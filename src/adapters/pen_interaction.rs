//! Pen tool (`P`) interaction logic: persistent cubic Bézier handle creation,
//! `Alt` cusp-corner breaking, `Shift` extremum segment snapping, and open-path resume/close.

use crate::adapters::contour_join::{
    alloc_off_curve, join_open_contours_in_pen, lerp_one_third, sync_pen_drag_handles_into_contour,
};
pub use crate::adapters::cursor_overlay::find_open_contour_close_or_resume_target;
use crate::adapters::state::{CanvasTool, GrafiaCanvasState};
use crate::core::{
    find_nearest_cubic_extremum, hit_test_segments, snap_vector_45_degrees, split_contour_segment,
    ContourSegment, FontPoint, Node, NodeId, NodeType, ScreenPoint,
};
use crate::interaction::PenToolState;
use crate::tokens::{DRAG_THRESHOLD_DP, SEGMENT_HIT_TOLERANCE_DP};

fn snap_extremum_if_shift(hit: &mut crate::core::SegmentHit, is_shift: bool) {
    if is_shift {
        if let ContourSegment::Cubic { cubic, .. } = hit.segment {
            let (ext_t, ext_pt) = find_nearest_cubic_extremum(cubic, hit.t);
            hit.t = ext_t;
            hit.point = ext_pt;
        }
    }
}

fn reset_pen_drag_transients(state: &mut GrafiaCanvasState) {
    state.pen_state = PenToolState::Idle;
    state.pen_drag_last_pt = None;
    state.pen_handle_delta = (0.0, 0.0);
    state.pen_alt_frozen_bcp_in = None;
    state.pen_seg_has_explicit_h1 = false;
    state.pen_resuming_corner_line_end = false;
    state.pen_join_b_had_out_handles = false;
    state.pen_join_b_was_smooth = false;
    state.active_drag_before_snapshot = None;
}

pub fn handle_pen_press(
    state: &mut GrafiaCanvasState,
    scr_pt: ScreenPoint,
    font_pt: FontPoint,
    is_shift: bool,
) -> bool {
    let before = state.snapshot();
    state.active_drag_before_snapshot = Some(before.clone());
    state.hovered_segment_hit = None;
    state.hovered_pen_close_target = None;
    state.hovered_pen_close_node = None;
    state.pen_join_b_had_out_handles = false;
    state.pen_join_b_was_smooth = false;

    if let Some((c_idx, target_node, is_closing_start)) =
        find_open_contour_close_or_resume_target(state, scr_pt)
    {
        if let Some(a_idx) = state.active_open_contour.filter(|&a| {
            a != c_idx
                && a < state.contours.len()
                && !state.is_contour_closed(a)
                && !state.contours[a].is_empty()
        }) {
            join_open_contours_in_pen(state, a_idx, c_idx, target_node, is_closing_start, font_pt);
            return true;
        }

        let opt_prev_out = state
            .pending_pen_bcp_out
            .filter(|&(c, _)| c == c_idx)
            .map(|(_, pt)| pt);
        let opt_first_in = state
            .pending_pen_first_bcp_in
            .filter(|&(c, _)| c == c_idx)
            .map(|(_, pt)| pt);
        if is_closing_start {
            state.pending_pen_bcp_out = None;
            state.pending_pen_first_bcp_in = None;
            state.pen_seg_has_explicit_h1 = opt_prev_out.is_some();
            state.pen_resuming_corner_line_end = false;
            if let Some(prev_out) = opt_prev_out {
                let h1 = alloc_off_curve(state, prev_out);
                let h2 = alloc_off_curve(
                    state,
                    opt_first_in.unwrap_or_else(|| lerp_one_third(target_node.point, prev_out)),
                );
                state.contours[c_idx].push(h1);
                state.contours[c_idx].push(h2);
            } else {
                state.contours[c_idx][0].node_type = NodeType::Corner;
            }
            state.set_contour_closed(c_idx, true);
            state.pen_alt_frozen_bcp_in = opt_first_in.or(Some(target_node.point));
        } else {
            if opt_prev_out.is_none() {
                state.pending_pen_bcp_out = None;
            }
            let c_len = state.contours[c_idx].len();
            let has_existing_in = c_len >= 3
                && state.contours[c_idx][c_len - 2].node_type == NodeType::OffCurve
                && state.contours[c_idx][c_len - 3].node_type == NodeType::OffCurve;
            let existing_bcp_in = if has_existing_in {
                Some(state.contours[c_idx][c_len - 2].point)
            } else if c_len == 1 {
                opt_first_in
            } else {
                None
            };
            state.pen_alt_frozen_bcp_in = existing_bcp_in.or(Some(target_node.point));
            state.pen_seg_has_explicit_h1 = has_existing_in;
            state.pen_resuming_corner_line_end =
                c_len >= 2 && !has_existing_in && target_node.node_type == NodeType::Corner;
        }
        state.active_open_contour = Some(c_idx);
        state.selected_nodes = vec![target_node.id];
        state.pen_state = PenToolState::PlacingAnchor {
            anchor: target_node.point,
        };
        state.pen_drag_last_pt = Some(font_pt);
        state.pen_handle_delta = (0.0, 0.0);
        state.sync_paths_from_contours();
        return true;
    }

    if let Some(hid) = state.hit_test_pen_node(scr_pt) {
        state.selected_nodes = vec![hid];
        reset_pen_drag_transients(state);
        state.assert_invariants();
        return true;
    }

    let tol_font = (SEGMENT_HIT_TOLERANCE_DP as f64) / (state.viewport.zoom as f64);
    let closed_flags = state.closed_flags();
    if let Some(mut hit) = hit_test_segments(&state.contours, &closed_flags, font_pt, tol_font) {
        snap_extremum_if_shift(&mut hit, is_shift);
        let new_id = split_contour_segment(
            &mut state.contours[hit.contour_idx],
            hit.segment,
            hit.t,
            &mut state.next_node_id,
        );
        state.selected_nodes = vec![new_id];
        reset_pen_drag_transients(state);
        state.sync_paths_from_contours();
        state.record_mutation("Add Node on Segment", before);
        state.assert_invariants();
        return true;
    }

    let target_open = state
        .active_open_contour
        .filter(|&c| c < state.contours.len() && !state.is_contour_closed(c))
        .or_else(|| {
            let last_idx = state.contours.len().checked_sub(1)?;
            (!state.is_contour_closed(last_idx)
                && state.contours[last_idx]
                    .iter()
                    .any(|n| state.selected_nodes.contains(&n.id)))
            .then_some(last_idx)
        });

    let sub_grid_step = state.grid_step_upm / f64::from(state.grid_subdivisions.max(1));
    let snapped_pt = if state.last_modifiers.control {
        state.active_snappers.clear();
        font_pt
    } else {
        let res = state.evaluate_magnetic_snap(font_pt, &[], false);
        if !res.active_snappers.is_empty() {
            state.active_snappers = res.active_snappers;
            res.snapped_point
        } else {
            state.active_snappers.clear();
            font_pt.round_to_grid_step(sub_grid_step)
        }
    };
    let new_id = NodeId(state.next_node_id);
    state.next_node_id += 1;
    let node = Node::new(new_id, snapped_pt.x.0, snapped_pt.y.0, NodeType::Corner);
    state.pen_seg_has_explicit_h1 = false;
    state.pen_resuming_corner_line_end = false;

    if let Some(c_idx) = target_open {
        if let Some((pend_c, prev_out)) = state.pending_pen_bcp_out.take() {
            if pend_c == c_idx {
                state.pen_seg_has_explicit_h1 = true;
                let h1 = alloc_off_curve(state, prev_out);
                let h2 = alloc_off_curve(state, lerp_one_third(snapped_pt, prev_out));
                state.contours[c_idx].push(h1);
                state.contours[c_idx].push(h2);
            }
        }
        state.contours[c_idx].push(node);
        state.active_open_contour = Some(c_idx);
        state.set_contour_closed(c_idx, false);
    } else {
        let c_idx = state.contours.len();
        state.contours.push(vec![node.clone()]);
        let mut gc = crate::core::GlyphContour::new(crate::core::ContourId(c_idx), vec![node]);
        gc.closed = false;
        state.glyph.contours.push(gc);
        state.active_open_contour = Some(c_idx);
        state.set_contour_closed(c_idx, false);
        state.pending_pen_bcp_out = None;
        state.pending_pen_first_bcp_in = None;
    }

    state.selected_nodes = vec![new_id];
    state.pen_state = PenToolState::PlacingAnchor { anchor: snapped_pt };
    state.pen_drag_last_pt = Some(snapped_pt);
    state.pen_handle_delta = (0.0, 0.0);
    state.pen_alt_frozen_bcp_in = Some(snapped_pt);
    state.sync_paths_from_contours();
    true
}

pub fn handle_pen_move(
    state: &mut GrafiaCanvasState,
    scr_pt: ScreenPoint,
    font_pt: FontPoint,
    is_shift: bool,
    is_alt: bool,
    is_space: bool,
    canvas_rect: egui::Rect,
) -> bool {
    match state.pen_state {
        PenToolState::PlacingAnchor { anchor } | PenToolState::DraggingHandles { anchor, .. } => {
            if is_space {
                let prev = state.pen_drag_last_pt.unwrap_or(anchor);
                let delta = (font_pt.x.0 - prev.x.0, font_pt.y.0 - prev.y.0);
                let new_anchor = FontPoint::new(anchor.x.0 + delta.0, anchor.y.0 + delta.1);
                let is_closing = state
                    .active_open_contour
                    .is_some_and(|c| state.is_contour_closed(c));
                let shift_pt = |pt: &mut FontPoint| {
                    pt.x.0 += delta.0;
                    pt.y.0 += delta.1;
                };
                if let Some(&first_id) = state.selected_nodes.first() {
                    for contour in &mut state.contours {
                        if let Some(pos) = contour.iter().position(|n| n.id == first_id) {
                            contour[pos].point = new_anchor;
                            if pos > 0 && contour[pos - 1].node_type == NodeType::OffCurve {
                                shift_pt(&mut contour[pos - 1].point);
                            } else if is_closing && pos == 0 {
                                let len = contour.len();
                                if len >= 3 && contour[len - 1].node_type == NodeType::OffCurve {
                                    shift_pt(&mut contour[len - 1].point);
                                }
                                if len >= 4 && contour[1].node_type == NodeType::OffCurve {
                                    shift_pt(&mut contour[1].point);
                                }
                            }
                        }
                    }
                }
                if let Some((_, ref mut p_out)) = state.pending_pen_bcp_out {
                    shift_pt(p_out);
                }
                if let Some((_, ref mut p_in)) = state.pending_pen_first_bcp_in {
                    shift_pt(p_in);
                }
                let bcp_out = FontPoint::new(
                    new_anchor.x.0 + state.pen_handle_delta.0,
                    new_anchor.y.0 + state.pen_handle_delta.1,
                );
                let bcp_in = state.pen_alt_frozen_bcp_in.map_or_else(
                    || {
                        FontPoint::new(
                            new_anchor.x.0 - state.pen_handle_delta.0,
                            new_anchor.y.0 - state.pen_handle_delta.1,
                        )
                    },
                    |frozen| FontPoint::new(frozen.x.0 + delta.0, frozen.y.0 + delta.1),
                );
                state.pen_alt_frozen_bcp_in = Some(bcp_in);
                state.pen_state = PenToolState::DraggingHandles {
                    anchor: new_anchor,
                    bcp_out,
                    bcp_in,
                    is_closing,
                };
                state.pen_drag_last_pt = Some(font_pt);
                state.sync_paths_from_contours();
                true
            } else {
                if matches!(state.pen_state, PenToolState::PlacingAnchor { .. }) {
                    let press_scr = state
                        .viewport
                        .to_screen(state.pen_drag_last_pt.unwrap_or(anchor));
                    if press_scr.distance_to(scr_pt) < DRAG_THRESHOLD_DP {
                        return false;
                    }
                }
                let mut delta = (font_pt.x.0 - anchor.x.0, font_pt.y.0 - anchor.y.0);
                if is_shift {
                    delta = snap_vector_45_degrees(delta.0, delta.1);
                }
                state.pen_handle_delta = delta;
                let c_idx = state
                    .active_open_contour
                    .filter(|&c| c < state.contours.len())
                    .unwrap_or_else(|| state.contours.len().saturating_sub(1));
                let is_closing = state.is_contour_closed(c_idx);
                let bcp_out = FontPoint::new(anchor.x.0 + delta.0, anchor.y.0 + delta.1);
                let (bcp_in, target_type) = if state.pen_resuming_corner_line_end {
                    (anchor, NodeType::Corner)
                } else if is_alt {
                    (
                        state.pen_alt_frozen_bcp_in.unwrap_or(anchor),
                        NodeType::Corner,
                    )
                } else {
                    let sym_in = FontPoint::new(anchor.x.0 - delta.0, anchor.y.0 - delta.1);
                    state.pen_alt_frozen_bcp_in = Some(sym_in);
                    (sym_in, NodeType::Smooth)
                };
                sync_pen_drag_handles_into_contour(
                    state,
                    c_idx,
                    anchor,
                    bcp_in,
                    bcp_out,
                    target_type,
                    is_closing,
                );
                state.pen_state = PenToolState::DraggingHandles {
                    anchor,
                    bcp_out,
                    bcp_in,
                    is_closing,
                };
                state.pen_drag_last_pt = Some(font_pt);
                state.sync_paths_from_contours();
                true
            }
        }
        PenToolState::Idle => {
            state.hovered_pen_close_target = None;
            state.hovered_pen_close_node = None;
            state.hovered_segment_hit = None;
            if state.active_tool == CanvasTool::Pen
                && state.select_state == crate::interaction::SelectToolState::Idle
                && canvas_rect.contains(egui::pos2(scr_pt.x.0, scr_pt.y.0))
            {
                let close_or_join_hit = find_open_contour_close_or_resume_target(state, scr_pt)
                    .filter(|&(c_idx, _, is_closing_start)| {
                        is_closing_start
                            || state.active_open_contour.is_some_and(|a| {
                                a != c_idx
                                    && a < state.contours.len()
                                    && !state.is_contour_closed(a)
                                    && !state.contours[a].is_empty()
                            })
                    });
                if let Some((c_idx, target_node, _)) = close_or_join_hit {
                    state.hovered_pen_close_target = Some((c_idx, target_node.id));
                    state.hovered_pen_close_node = Some(target_node.id);
                } else if state.hit_test_pen_node(scr_pt).is_none() {
                    let tol_font = (SEGMENT_HIT_TOLERANCE_DP as f64) / (state.viewport.zoom as f64);
                    let closed_flags = state.closed_flags();
                    let mut hit_opt =
                        hit_test_segments(&state.contours, &closed_flags, font_pt, tol_font);
                    if let Some(ref mut hit) = hit_opt {
                        snap_extremum_if_shift(hit, is_shift);
                    }
                    state.hovered_segment_hit = hit_opt;
                }
            }
            false
        }
    }
}
