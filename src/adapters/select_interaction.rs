//! Select tool (`V` / `SelectTool`) interaction handlers for `GrafiaCanvasState` (`SPEC-002 §3`).

pub use crate::adapters::select_drag_ops::duplicate_selected_contours_for_alt_drag;
use crate::adapters::state::GrafiaCanvasState;
use crate::core::{
    convert_line_segment_to_cubic_with_grid, hit_test_segments, ContourSegment, FontPoint, NodeId,
    NodeType, ScreenPoint,
};
use crate::interaction::{Modifiers, SelectToolState};
use crate::render::ProjectedNode;
use crate::tokens::{DOUBLE_CLICK_MAX_DIST_DP, SEGMENT_HIT_TOLERANCE_DP};
use kurbo::Point;

pub fn handle_select_press(
    state: &mut GrafiaCanvasState,
    scr_pt: ScreenPoint,
    font_pt: FontPoint,
    mods: Modifiers,
    is_double: bool,
    is_triple: bool,
) -> bool {
    if state.try_begin_alt_select_node_press(scr_pt, font_pt, mods) {
        return true;
    }
    let (is_shift, is_alt) = (mods.shift, mods.option);
    let before = state.snapshot();
    state.active_drag_before_snapshot = Some(before.clone());
    state.pending_segment_click = None;
    state.alt_marquee_pending_start = None;
    state.alt_duplicate_pending = false;

    if is_triple {
        state.selected_nodes = state
            .contours
            .iter()
            .flat_map(|c| c.iter().map(|n| n.id))
            .collect();
        state.select_state = SelectToolState::Idle;
        state.active_drag_before_snapshot = None;
        return true;
    }

    if is_double {
        let hit_node = state.hit_test_node(scr_pt);
        if let Some(node_id) = hit_node {
            if state
                .find_node(node_id)
                .is_some_and(|n| n.node_type != NodeType::OffCurve)
            {
                state.toggle_node_smooth_corner(node_id);
                state.active_drag_before_snapshot = None;
                return true;
            }
        }
        let tol_font = (DOUBLE_CLICK_MAX_DIST_DP as f64) / (state.viewport.zoom as f64);
        let closed_flags = state.closed_flags();
        let target_contour = hit_node
            .and_then(|nid| {
                state
                    .contours
                    .iter()
                    .position(|c| c.iter().any(|n| n.id == nid))
            })
            .or_else(|| {
                hit_test_segments(&state.contours, &closed_flags, font_pt, tol_font)
                    .map(|h| h.contour_idx)
            });
        if let Some(c_idx) = target_contour {
            let contour_ids: Vec<NodeId> = state.contours[c_idx].iter().map(|n| n.id).collect();
            if is_shift {
                if contour_ids
                    .iter()
                    .all(|id| state.selected_nodes.contains(id))
                {
                    state.selected_nodes.retain(|id| !contour_ids.contains(id));
                } else {
                    for id in contour_ids {
                        if !state.selected_nodes.contains(&id) {
                            state.selected_nodes.push(id);
                        }
                    }
                }
            } else {
                state.selected_nodes = contour_ids;
            }
            state.select_state = SelectToolState::Idle;
            state.active_drag_before_snapshot = None;
            return true;
        }
        if state.double_click_component_at_upm(font_pt) {
            (state.select_state, state.active_drag_before_snapshot) = (SelectToolState::Idle, None);
            return true;
        }
    }

    if let Some(node_id) = state.hit_test_node(scr_pt) {
        if is_shift {
            if state.selected_nodes.contains(&node_id) {
                state.selected_nodes.retain(|&id| id != node_id);
            } else {
                state.selected_nodes.push(node_id);
            }
        } else if !state.selected_nodes.contains(&node_id) {
            state.selected_nodes = vec![node_id];
        }
        state.record_drag_start();
        state.alt_duplicate_pending = is_alt && !mods.control && state.selected_nodes.len() >= 2;
        state.select_state = SelectToolState::DraggingNodes {
            selected: state.selected_nodes.clone(),
            start_pt: font_pt,
            current_pt: font_pt,
            modifiers: mods,
        };
        return true;
    }

    let tol_font = (SEGMENT_HIT_TOLERANCE_DP as f64) / (state.viewport.zoom as f64);
    let closed_flags = state.closed_flags();
    let grid_step = state.effective_grid_step();
    if let Some(hit) = hit_test_segments(&state.contours, &closed_flags, font_pt, tol_font) {
        if is_alt
            && convert_line_segment_to_cubic_with_grid(
                &mut state.contours[hit.contour_idx],
                &hit.segment,
                &mut state.next_node_id,
                grid_step,
            )
            .is_some()
        {
            state.sync_paths_from_contours();
            state.record_mutation("Convert Line to Cubic", before);
            state.active_drag_before_snapshot = None;
            state.assert_invariants();
            return true;
        }

        let (start_idx, end_idx) = match hit.segment {
            ContourSegment::Line {
                start_idx, end_idx, ..
            }
            | ContourSegment::Cubic {
                start_idx, end_idx, ..
            } => (start_idx, end_idx),
        };
        let p0_id = state.contours[hit.contour_idx][start_idx].id;
        let p3_id = state.contours[hit.contour_idx][end_idx].id;
        if state.selected_nodes.contains(&p0_id) && state.selected_nodes.contains(&p3_id) {
            if is_shift {
                state.marquee_pre_selection = state.selected_nodes.clone();
                state.pending_segment_click = Some((hit.contour_idx, hit.segment, mods));
            }
            state.record_drag_start();
            state.alt_duplicate_pending =
                is_alt && !mods.control && state.selected_nodes.len() >= 2;
            state.select_state = SelectToolState::DraggingNodes {
                selected: state.selected_nodes.clone(),
                start_pt: font_pt,
                current_pt: font_pt,
                modifiers: mods,
            };
            return true;
        }

        state.marquee_pre_selection = state.selected_nodes.clone();
        state.marquee_modifiers = mods;
        if !is_shift {
            state.selected_nodes.clear();
        }
        state.pending_segment_click = Some((hit.contour_idx, hit.segment, mods));

        match hit.segment {
            ContourSegment::Line { .. } => {
                state.active_drag_before_snapshot = None;
                state.select_state = SelectToolState::MarqueeSelecting {
                    start_screen: scr_pt,
                    current_screen: scr_pt,
                };
                return true;
            }
            ContourSegment::Cubic { h1_idx, h2_idx, .. } => {
                let (initial_h1, initial_h2) = (
                    state.contours[hit.contour_idx][h1_idx].point,
                    state.contours[hit.contour_idx][h2_idx].point,
                );
                state.record_drag_start();
                state.select_state = SelectToolState::DraggingSegment {
                    contour_idx: hit.contour_idx,
                    segment: hit.segment,
                    t: hit.t,
                    start_pt: font_pt,
                    initial_h1,
                    initial_h2,
                };
                return true;
            }
        }
    }

    if state.try_select_v070_anchor_or_component_on_press(font_pt, is_shift) {
        return true;
    }

    state.marquee_pre_selection = if is_shift {
        state.selected_nodes.clone()
    } else {
        Vec::new()
    };
    state.marquee_modifiers = mods;
    if !is_shift {
        state.selected_nodes.clear();
        state.clear_v070_selections();
    }
    state.active_drag_before_snapshot = None;
    if is_alt {
        state.alt_marquee_pending_start = Some(scr_pt);
        state.select_state = SelectToolState::Idle;
    } else {
        state.select_state = SelectToolState::MarqueeSelecting {
            start_screen: scr_pt,
            current_screen: scr_pt,
        };
    }
    true
}

pub fn apply_marquee_selection(state: &mut GrafiaCanvasState, is_alt_now: bool) {
    let only_on_curve = state.marquee_modifiers.option || is_alt_now;
    let xor_toggle = state.marquee_modifiers.shift;
    let ctrl_slant = state.marquee_modifiers.control && !state.marquee_modifiers.command;
    if let SelectToolState::MarqueeSelecting {
        start_screen,
        current_screen,
    } = state.select_state
    {
        if ctrl_slant {
            state.show_bounding_box_slanted = true;
            let (p0, p1) = (
                state.viewport.to_font(start_screen),
                state.viewport.to_font(current_screen),
            );
            let mut deg = (p1.x.0 - p0.x.0)
                .atan2((p1.y.0 - p0.y.0).abs().max(1.0))
                .to_degrees()
                .clamp(
                    -crate::tokens::MARQUEE_MAX_SLANT_DEG,
                    crate::tokens::MARQUEE_MAX_SLANT_DEG,
                );
            if (deg - state.italic_angle_deg).abs() < crate::tokens::MARQUEE_ITALIC_SNAP_TOL_DEG {
                deg = state.italic_angle_deg;
            } else if state.marquee_modifiers.shift {
                deg = deg.round();
            }
            state.selection_angle_deg = deg;
        } else if only_on_curve && state.italic_angle_deg.abs() > 0.0 {
            state.selection_angle_deg = state.italic_angle_deg;
            state.show_bounding_box_slanted = true;
        } else {
            state.selection_angle_deg = 0.0;
            state.show_bounding_box_slanted = false;
        }
        if (ctrl_slant || only_on_curve) && state.selection_angle_deg.abs() > 0.0 {
            state.selected_nodes = crate::core::select_nodes_in_slanted_marquee(
                &state.contours,
                &state.viewport,
                &state.theme.edit_canvas,
                start_screen,
                current_screen,
                state.selection_angle_deg,
                &state.marquee_pre_selection,
                xor_toggle,
                only_on_curve,
            );
            return;
        }
    }
    let Some(rect) = state.select_state.marquee_rect() else {
        return;
    };
    let mut hits: Vec<NodeId> = Vec::new();
    for contour in &state.contours {
        for node in contour {
            if only_on_curve && node.node_type == NodeType::OffCurve {
                continue;
            }
            let proj = ProjectedNode::new(
                node.point,
                node.node_type,
                false,
                &state.viewport,
                &state.theme.edit_canvas,
            );
            let center = Point::new(proj.screen_pos.x.0 as f64, proj.screen_pos.y.0 as f64);
            let inter = rect.intersect(proj.bounds);
            if (rect.contains(center) || (inter.width() > 0.0 && inter.height() > 0.0))
                && !hits.contains(&node.id)
            {
                hits.push(node.id);
            }
        }
    }
    if xor_toggle {
        let mut next_sel: Vec<NodeId> = state
            .marquee_pre_selection
            .iter()
            .copied()
            .filter(|id| !hits.contains(id))
            .collect();
        for id in hits {
            if !state.marquee_pre_selection.contains(&id) {
                next_sel.push(id);
            }
        }
        state.selected_nodes = next_sel;
    } else {
        state.selected_nodes = hits;
    }
    state.sync_companion_marquee_selection(rect);
}

pub fn apply_segment_click_selection(
    state: &mut GrafiaCanvasState,
    c_idx: usize,
    seg: ContourSegment,
    mods: Modifiers,
) {
    let Some(contour) = state.contours.get(c_idx) else {
        return;
    };
    let target_ids: Vec<NodeId> = match seg {
        ContourSegment::Line {
            start_idx, end_idx, ..
        } => vec![contour[start_idx].id, contour[end_idx].id],
        ContourSegment::Cubic {
            start_idx,
            h1_idx,
            h2_idx,
            end_idx,
            ..
        } => {
            vec![
                contour[start_idx].id,
                contour[h1_idx].id,
                contour[h2_idx].id,
                contour[end_idx].id,
            ]
        }
    };
    if mods.shift {
        let mut sel = state.marquee_pre_selection.clone();
        if target_ids.iter().all(|id| sel.contains(id)) {
            sel.retain(|id| !target_ids.contains(id));
        } else {
            for id in target_ids {
                if !sel.contains(&id) {
                    sel.push(id);
                }
            }
        }
        state.selected_nodes = sel;
    } else {
        state.selected_nodes = target_ids;
    }
}
