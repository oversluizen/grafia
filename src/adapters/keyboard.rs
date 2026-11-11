//! Keyboard shortcut dispatching for canvas navigation and manipulation (SPEC-002 §3.3 & §4).

use crate::adapters::keyboard_edit::delete_selected_nodes_or_guide;
use crate::adapters::state::{CanvasTool, CanvasViewMode, GrafiaCanvasState};
use crate::core::{
    cycle_selection_tab, enforce_smooth_collinearity_on_handle_drag,
    expanded_nodes_with_attached_handles, toggle_selected_on_curve_smooth, NodeId, NodeType,
    ScreenPoint,
};
use crate::tokens::{
    NUDGE_STEP_CMD_UPM, NUDGE_STEP_SHIFT_UPM, NUDGE_STEP_UPM, ZOOM_STEP_IN_FACTOR,
    ZOOM_STEP_OUT_FACTOR,
};
use egui::{Event, Key, Rect};

/// Processes keyboard events for view mode overrides, tool switching, nudging, zoom, and undo/redo.
pub fn dispatch_keyboard_event(
    state: &mut GrafiaCanvasState,
    event: &Event,
    is_shift: bool,
    is_cmd: bool,
    is_alt: bool,
    canvas_rect: Rect,
) -> bool {
    let is_ctrl = state.last_modifiers.control;
    if crate::adapters::v070_layers_vc::dispatch_v070_keyboard_shortcut(
        state, event, is_shift, is_alt, is_cmd, is_ctrl,
    ) || crate::adapters::v050_interaction::dispatch_v050_keyboard_shortcut(
        state, event, is_shift, is_alt, is_cmd, is_ctrl,
    ) || crate::adapters::v060_layers::dispatch_v060_keyboard_shortcut(
        state, event, is_shift, is_alt, is_cmd, is_ctrl,
    ) {
        return true;
    }
    match event {
        Event::Key {
            key: Key::Space,
            pressed: true,
            repeat: false,
            modifiers,
            ..
        } => {
            state.last_modifiers.shift = modifiers.shift || is_shift;
            state.last_modifiers.option = modifiers.alt || is_alt;
            state.last_modifiers.command = modifiers.command || is_cmd;
            state.last_modifiers.control = modifiers.ctrl;
            state.last_modifiers.space = true;
            if state.active_tool == CanvasTool::Pen
                && state.pen_state != crate::interaction::PenToolState::Idle
            {
                return false;
            }
            if matches!(
                state.select_state,
                crate::interaction::SelectToolState::MarqueeSelecting { .. }
            ) {
                state.marquee_modifiers.space = true;
                return true;
            }
            if modifiers.command || modifiers.ctrl || is_cmd {
                if state.temp_tool_before_override.is_none() {
                    state.temp_tool_before_override = Some(state.active_tool);
                }
                state.temp_zoom_via_key_space = true;
                state.active_tool = CanvasTool::Zoom;
                if let Some(prev_vm) = state.prev_view_mode.take() {
                    state.view_mode = prev_vm;
                }
                return true;
            }
            if state.prev_view_mode.is_none() {
                state.prev_view_mode = Some(state.view_mode);
            }
            state.view_mode = CanvasViewMode::PreviewFilled;
            true
        }
        Event::Key {
            key: Key::Space,
            pressed: false,
            modifiers,
            ..
        } => {
            state.last_modifiers.shift = modifiers.shift || is_shift;
            state.last_modifiers.option = modifiers.alt || is_alt;
            state.last_modifiers.command = modifiers.command || is_cmd;
            state.last_modifiers.control = modifiers.ctrl;
            state.last_modifiers.space = false;
            state.marquee_modifiers.space = false;
            if state.active_tool == CanvasTool::Pen
                && state.pen_state != crate::interaction::PenToolState::Idle
            {
                return false;
            }
            if state.temp_zoom_via_key_space || state.active_tool == CanvasTool::Zoom {
                state.temp_zoom_via_key_space = false;
                if let Some(prev) = state.temp_tool_before_override.take() {
                    state.active_tool = prev;
                    return true;
                }
            }
            state.view_mode = state
                .prev_view_mode
                .take()
                .unwrap_or(CanvasViewMode::EditWireframe);
            if matches!(
                state.select_state,
                crate::interaction::SelectToolState::PanningViewport { .. }
            ) {
                state.select_state = crate::interaction::SelectToolState::Idle;
            }
            true
        }
        Event::Key {
            key: key @ (Key::ArrowLeft | Key::ArrowRight | Key::ArrowUp | Key::ArrowDown),
            pressed: true,
            modifiers,
            ..
        } => {
            let alt_held = modifiers.alt || is_alt;
            let ctrl_held = modifiers.ctrl;
            let step = if (modifiers.command || is_cmd) && !ctrl_held {
                NUDGE_STEP_CMD_UPM
            } else if modifiers.shift || is_shift {
                NUDGE_STEP_SHIFT_UPM
            } else if ctrl_held && !alt_held {
                NUDGE_STEP_CMD_UPM
            } else {
                NUDGE_STEP_UPM
            };
            let (dx, dy) = match key {
                Key::ArrowLeft => (-step, 0.0),
                Key::ArrowRight => (step, 0.0),
                Key::ArrowUp => (0.0, step),
                Key::ArrowDown => (0.0, -step),
                _ => (0.0, 0.0),
            };
            if state.select_all_layers_mode && !state.selected_companion_nodes.is_empty() {
                state.translate_selected_across_all_layers(crate::core::FontPoint::new(dx, dy));
                return true;
            }
            if state.selected_nodes.is_empty() {
                return state.nudge_selected_guide(dx, dy)
                    || state.nudge_selected_anchors_or_components(dx, dy);
            }
            let before = state.snapshot();
            if ctrl_held && alt_held {
                state.record_drag_start();
                state.apply_node_displacement_with_modifiers_and_ctrl(dx, dy, false, true, true);
                state.record_mutation("Nudge Nodes", before);
                state.assert_invariants();
                return true;
            }
            let closed_flags = state.closed_flags();
            let expanded_ids = expanded_nodes_with_attached_handles(
                &state.contours,
                &closed_flags,
                &state.selected_nodes,
                !alt_held,
            );
            for id in expanded_ids {
                if let Some(node) = state.find_node_mut(id) {
                    node.point.x.0 += dx;
                    node.point.y.0 += dy;
                }
            }
            if state.selected_nodes.len() == 1 {
                let sel_id = state.selected_nodes[0];
                if state
                    .find_node(sel_id)
                    .is_some_and(|n| n.node_type == NodeType::OffCurve)
                {
                    for (c_idx, contour) in state.contours.iter_mut().enumerate() {
                        enforce_smooth_collinearity_on_handle_drag(
                            contour,
                            closed_flags[c_idx],
                            sel_id,
                            alt_held,
                        );
                    }
                }
            }
            state.sync_paths_from_contours();
            state.record_mutation("Nudge Nodes", before);
            state.assert_invariants();
            true
        }
        Event::Key {
            key,
            pressed: true,
            modifiers,
            ..
        } => {
            let cmd_active = modifiers.command || modifiers.ctrl || is_cmd;
            let shift_active = modifiers.shift || is_shift;
            let alt_active = modifiers.alt || is_alt;

            if crate::adapters::v040_tools::dispatch_v040_keyboard_shortcut(
                state,
                *key,
                shift_active,
                alt_active,
                cmd_active,
            ) || crate::adapters::v020_interaction::dispatch_v020_keyboard_shortcut(
                state,
                *key,
                shift_active,
                alt_active,
                cmd_active,
            ) {
                return true;
            }

            match key {
                Key::A if cmd_active => {
                    let on_curve_ids: Vec<NodeId> = state
                        .contours
                        .iter()
                        .flat_map(|c| c.iter())
                        .filter(|n| n.node_type != NodeType::OffCurve)
                        .map(|n| n.id)
                        .collect();
                    let all_on_selected = !on_curve_ids.is_empty()
                        && on_curve_ids
                            .iter()
                            .all(|id| state.selected_nodes.contains(id));
                    if all_on_selected {
                        state.selected_nodes = state
                            .contours
                            .iter()
                            .flat_map(|c| c.iter().map(|n| n.id))
                            .collect();
                    } else {
                        state.selected_nodes = on_curve_ids;
                    }
                    state.assert_invariants();
                    true
                }
                Key::D if cmd_active => {
                    state.selected_nodes.clear();
                    state.v040.selected_annotations.clear();
                    state.clear_v070_selections();
                    state.assert_invariants();
                    true
                }
                Key::R if cmd_active && shift_active => state.reverse_selected_or_all_contours(),
                Key::F if cmd_active && shift_active => state.make_selected_node_first(),
                Key::Equals | Key::Plus if cmd_active => {
                    let center = ScreenPoint::new(canvas_rect.center().x, canvas_rect.center().y);
                    state.viewport.zoom_at_cursor(center, ZOOM_STEP_IN_FACTOR);
                    true
                }
                Key::Minus if cmd_active => {
                    let center = ScreenPoint::new(canvas_rect.center().x, canvas_rect.center().y);
                    state.viewport.zoom_at_cursor(center, ZOOM_STEP_OUT_FACTOR);
                    true
                }
                Key::Num0 if cmd_active => {
                    state.fit_viewport_to_glyph(canvas_rect.width(), canvas_rect.height());
                    true
                }
                Key::V if !cmd_active => {
                    state.active_tool = CanvasTool::Select;
                    state.pen_state = crate::interaction::PenToolState::Idle;
                    state.hovered_segment_hit = None;
                    state.hovered_pen_close_target = None;
                    state.hovered_pen_close_node = None;
                    state.pending_pen_bcp_out = None;
                    state.pending_pen_first_bcp_in = None;
                    if state
                        .active_open_contour
                        .is_some_and(|c| state.is_contour_closed(c))
                    {
                        state.active_open_contour = None;
                    }
                    true
                }
                Key::P if !cmd_active => {
                    state.active_tool = CanvasTool::Pen;
                    state.select_state = crate::interaction::SelectToolState::Idle;
                    true
                }
                Key::Escape => {
                    state.selected_nodes.clear();
                    state.selected_guide = None;
                    state.v040.selected_annotations.clear();
                    state.clear_v070_selections();
                    state.v040.reset_in_flight_drags();
                    state.select_state = crate::interaction::SelectToolState::Idle;
                    state.pen_state = crate::interaction::PenToolState::Idle;
                    state.active_open_contour = None;
                    state.hovered_segment_hit = None;
                    state.hovered_pen_close_target = None;
                    state.hovered_pen_close_node = None;
                    state.pending_pen_bcp_out = None;
                    state.pending_pen_first_bcp_in = None;
                    true
                }
                Key::Tab => {
                    if state.selected_nodes.is_empty() && state.selected_guide.is_some() {
                        return state.cycle_guide_tab(shift_active);
                    }
                    if let Some(next_id) =
                        cycle_selection_tab(&state.contours, &state.selected_nodes, shift_active)
                    {
                        state.selected_nodes = vec![next_id];
                        state.assert_invariants();
                        return true;
                    }
                    state.cycle_guide_tab(shift_active)
                }
                Key::Enter => {
                    if state.selected_nodes.is_empty() {
                        return false;
                    }
                    let before = state.snapshot();
                    let closed_flags = state.closed_flags();
                    let selected_copy = state.selected_nodes.clone();
                    if toggle_selected_on_curve_smooth(
                        &mut state.contours,
                        &closed_flags,
                        &selected_copy,
                    ) {
                        state.sync_paths_from_contours();
                        state.record_mutation("Toggle Smooth", before);
                        state.assert_invariants();
                        return true;
                    }
                    false
                }
                Key::Backspace | Key::Delete => {
                    delete_selected_nodes_or_guide(state, shift_active, alt_active)
                }
                Key::Z if cmd_active && !shift_active => state.undo(),
                Key::Z if cmd_active && shift_active => state.redo(),
                Key::Y if cmd_active => state.redo(),
                _ => false,
            }
        }
        _ => false,
    }
}
