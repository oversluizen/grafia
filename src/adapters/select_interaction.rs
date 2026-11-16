//! Select tool (`V` / `SelectTool`) interaction handlers for `GrafiaCanvasState` (`SPEC-002 §3`).

pub use crate::adapters::select_drag_ops::{
    apply_segment_click_selection, duplicate_selected_contours_for_alt_drag,
};
use crate::adapters::state::GrafiaCanvasState;
use crate::core::{
    convert_line_segment_to_cubic_with_grid, hit_test_segments, ContourSegment, FontPoint, NodeId,
    NodeType, ScreenPoint,
};
use crate::interaction::{Modifiers, SelectToolState};
use crate::render::ProjectedNode;
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
    state.drag_transients.active_drag_before_snapshot = Some(before.clone());
    state.drag_transients.pending_segment_click = None;
    state.drag_transients.alt_marquee_pending_start = None;
    state.drag_transients.alt_duplicate_pending = false;

    if is_triple {
        state.selected_nodes = state.contours.iter().flatten().map(|n| n.id).collect();
        state.select_state = SelectToolState::Idle;
        state.drag_transients.active_drag_before_snapshot = None;
        return true;
    }

    if (is_double && handle_select_double_click(state, scr_pt, font_pt, is_shift))
        || state.try_begin_tunni_press(font_pt, false)
    {
        return true;
    }

    if let Some(node_id) = state.hit_test_node(scr_pt) {
        let is_off = state
            .find_node(node_id)
            .is_some_and(|n| n.node_type == NodeType::OffCurve);
        if is_shift {
            if state.selected_nodes.contains(&node_id) {
                state.selected_nodes.retain(|&id| id != node_id);
            } else {
                state.selected_nodes.push(node_id);
            }
        } else if !state.selected_nodes.contains(&node_id) || (is_off && !(mods.control && is_alt))
        {
            state.selected_nodes = vec![node_id];
        }
        state.record_drag_start();
        state.drag_transients.alt_duplicate_pending =
            is_alt && !mods.control && state.selected_nodes.len() >= 2;
        state.select_state = SelectToolState::DraggingNodes {
            selected: state.selected_nodes.clone(),
            start_pt: font_pt,
            current_pt: font_pt,
            modifiers: mods,
        };
        return true;
    }

    if handle_select_segment_hit(state, scr_pt, font_pt, mods, before)
        || state.try_select_anchor_or_component_on_press(font_pt, is_shift)
    {
        return true;
    }

    if state.try_click_select_postscript_hint(scr_pt)
        || state.try_click_select_truetype_instruction(scr_pt)
    {
        state.select_state = SelectToolState::Idle;
        state.drag_transients.active_drag_before_snapshot = None;
        return true;
    }

    if !is_shift {
        state.selected_nodes.clear();
        state.clear_anchor_and_component_selections();
    }
    state.drag_transients.marquee_pre_selection = state.selected_nodes.clone();
    state.drag_transients.marquee_modifiers = mods;
    state.drag_transients.active_drag_before_snapshot = None;
    state.drag_transients.alt_marquee_pending_start = is_alt.then_some(scr_pt);
    state.select_state = if is_alt {
        SelectToolState::Idle
    } else {
        SelectToolState::MarqueeSelecting {
            start_screen: scr_pt,
            current_screen: scr_pt,
        }
    };
    true
}

fn try_convert_hit_line_segment_to_cubic(
    state: &mut GrafiaCanvasState,
    contour_idx: usize,
    segment: &ContourSegment,
    before: crate::interaction::GlyphSnapshot,
) -> bool {
    let grid_step = state.effective_grid_step();
    let Some((h1_id, h2_id)) = convert_line_segment_to_cubic_with_grid(
        &mut state.contours[contour_idx],
        segment,
        &mut state.next_node_id,
        grid_step,
    ) else {
        return false;
    };
    state.selected_nodes = vec![h1_id, h2_id];
    state.sync_paths_from_contours();
    state.record_mutation("Convert Line to Cubic", before);
    state.select_state = SelectToolState::Idle;
    state.drag_transients.active_drag_before_snapshot = None;
    state.assert_invariants();
    true
}

fn handle_select_double_click(
    state: &mut GrafiaCanvasState,
    scr_pt: ScreenPoint,
    font_pt: FontPoint,
    is_shift: bool,
) -> bool {
    let is_touch = state.active_input_device() == crate::core::InputDeviceKind::TouchFinger;
    if let Some(node_id) = state.hit_test_oncurve_node_for_double_click(scr_pt) {
        state.toggle_node_smooth_corner(node_id);
        state.drag_transients.active_drag_before_snapshot = None;
        return true;
    }
    let hit_node = state.hit_test_node(scr_pt);
    if state.try_begin_tunni_press(font_pt, true) {
        return true;
    }
    let tol_dp = state
        .effective_segment_hit_tolerance_dp()
        .max(state.effective_double_click_slop_dp());
    let tol_font = f64::from(tol_dp) / f64::from(state.viewport.zoom);
    let seg_hit = hit_test_segments(&state.contours, &state.closed_flags(), font_pt, tol_font);
    if is_touch && !is_shift {
        if let Some(hit) = seg_hit.filter(|h| matches!(h.segment, ContourSegment::Line { .. })) {
            let before = state
                .drag_transients
                .active_drag_before_snapshot
                .take()
                .unwrap_or_else(|| state.snapshot());
            if try_convert_hit_line_segment_to_cubic(state, hit.contour_idx, &hit.segment, before) {
                return true;
            }
        }
    }
    let target_contour = hit_node
        .and_then(|nid| {
            state
                .contours
                .iter()
                .position(|c| c.iter().any(|n| n.id == nid))
        })
        .or_else(|| seg_hit.map(|h| h.contour_idx));
    if let Some(c_idx) = target_contour {
        let contour_ids: Vec<NodeId> = state.contours[c_idx].iter().map(|n| n.id).collect();
        if !is_shift {
            state.selected_nodes = contour_ids;
        } else if contour_ids
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
        state.select_state = SelectToolState::Idle;
        state.drag_transients.active_drag_before_snapshot = None;
        return true;
    }
    if state.double_click_component_at_upm(font_pt) {
        state.select_state = SelectToolState::Idle;
        state.drag_transients.active_drag_before_snapshot = None;
        return true;
    }
    false
}

fn handle_select_segment_hit(
    state: &mut GrafiaCanvasState,
    scr_pt: ScreenPoint,
    font_pt: FontPoint,
    mods: Modifiers,
    before: crate::interaction::GlyphSnapshot,
) -> bool {
    let (is_shift, is_alt) = (mods.shift, mods.option);
    let tol_font =
        f64::from(state.effective_segment_hit_tolerance_dp()) / f64::from(state.viewport.zoom);
    let Some(hit) = hit_test_segments(&state.contours, &state.closed_flags(), font_pt, tol_font)
    else {
        return false;
    };
    if is_alt && try_convert_hit_line_segment_to_cubic(state, hit.contour_idx, &hit.segment, before)
    {
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
    let c_nodes = &state.contours[hit.contour_idx];
    if state.selected_nodes.contains(&c_nodes[start_idx].id)
        && state.selected_nodes.contains(&c_nodes[end_idx].id)
    {
        if is_shift {
            state.drag_transients.marquee_pre_selection = state.selected_nodes.clone();
            state.drag_transients.pending_segment_click =
                Some((hit.contour_idx, hit.segment, mods));
        }
        state.record_drag_start();
        state.drag_transients.alt_duplicate_pending =
            is_alt && !mods.control && state.selected_nodes.len() >= 2;
        state.select_state = SelectToolState::DraggingNodes {
            selected: state.selected_nodes.clone(),
            start_pt: font_pt,
            current_pt: font_pt,
            modifiers: mods,
        };
        return true;
    }
    state.drag_transients.marquee_pre_selection = state.selected_nodes.clone();
    state.drag_transients.marquee_modifiers = mods;
    if !is_shift {
        state.selected_nodes.clear();
    }
    state.drag_transients.pending_segment_click = Some((hit.contour_idx, hit.segment, mods));
    start_segment_or_line_marquee_drag(state, hit, scr_pt, font_pt);
    true
}

fn start_segment_or_line_marquee_drag(
    state: &mut GrafiaCanvasState,
    hit: crate::core::SegmentHit,
    scr_pt: ScreenPoint,
    font_pt: FontPoint,
) {
    match hit.segment {
        ContourSegment::Line { .. } => {
            state.drag_transients.active_drag_before_snapshot = None;
            state.select_state = SelectToolState::MarqueeSelecting {
                start_screen: scr_pt,
                current_screen: scr_pt,
            };
        }
        ContourSegment::Cubic { h1_idx, h2_idx, .. } => {
            let initial_h1 = state.contours[hit.contour_idx][h1_idx].point;
            let initial_h2 = state.contours[hit.contour_idx][h2_idx].point;
            state.record_drag_start();
            state.select_state = SelectToolState::DraggingSegment {
                contour_idx: hit.contour_idx,
                segment: hit.segment,
                t: hit.t,
                start_pt: font_pt,
                initial_h1,
                initial_h2,
            };
        }
    }
}

pub fn apply_marquee_selection(state: &mut GrafiaCanvasState, is_alt_now: bool) {
    let only_on_curve = state.drag_transients.marquee_modifiers.option || is_alt_now;
    let xor_toggle = state.drag_transients.marquee_modifiers.shift;
    let ctrl_slant = state.drag_transients.marquee_modifiers.control
        && !state.drag_transients.marquee_modifiers.command;
    if update_slanted_marquee_selection(state, ctrl_slant, only_on_curve, xor_toggle) {
        return;
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
            .drag_transients
            .marquee_pre_selection
            .iter()
            .copied()
            .filter(|id| !hits.contains(id))
            .collect();
        for id in hits {
            if !state.drag_transients.marquee_pre_selection.contains(&id) {
                next_sel.push(id);
            }
        }
        state.selected_nodes = next_sel;
    } else {
        state.selected_nodes = hits;
    }
    state.sync_companion_marquee_selection(rect);
}

fn update_slanted_marquee_selection(
    state: &mut GrafiaCanvasState,
    ctrl_slant: bool,
    only_on_curve: bool,
    xor_toggle: bool,
) -> bool {
    let SelectToolState::MarqueeSelecting {
        start_screen,
        current_screen,
    } = state.select_state
    else {
        return false;
    };
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
        } else if state.drag_transients.marquee_modifiers.shift {
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
            start_screen,
            current_screen,
            state.selection_angle_deg,
            &state.drag_transients.marquee_pre_selection,
            (xor_toggle, only_on_curve),
        );
        return true;
    }
    false
}
