//! Keyboard shortcut dispatching for canvas navigation and manipulation (SPEC-002 §3.3 & §4).

use crate::adapters::state::{CanvasTool, CanvasViewMode, GlifoCanvasState};
use crate::core::{
    cycle_selection_tab, enforce_smooth_collinearity_on_handle_drag,
    expanded_nodes_with_attached_handles,
    toggle_selected_on_curve_smooth, NodeId, NodeType, ScreenPoint,
};
use crate::tokens::{
    NUDGE_STEP_CMD_UPM, NUDGE_STEP_SHIFT_UPM, NUDGE_STEP_UPM, ZOOM_STEP_IN_FACTOR,
    ZOOM_STEP_OUT_FACTOR,
};
use egui::{Event, Key, Rect};

fn prune_empty_contours(state: &mut GlifoCanvasState) {
    let old_closed = state.closed_flags();
    let old_meta: Vec<(crate::core::ShapeBooleanRole, bool)> = (0..state.contours.len())
        .map(|i| state.glyph.contours.get(i).map_or((crate::core::ShapeBooleanRole::Normal, true), |gc| (gc.role, gc.visible)))
        .collect();
    let mut kept_contours = Vec::with_capacity(state.contours.len());
    let mut kept_closed = Vec::with_capacity(state.contours.len());
    let mut kept_meta = Vec::with_capacity(state.contours.len());
    let mut new_active_open = None;

    for (old_idx, contour) in state.contours.drain(..).enumerate() {
        if !contour.is_empty() {
            let new_idx = kept_contours.len();
            let on_curve_count = contour.iter().filter(|n| n.node_type != NodeType::OffCurve).count();
            let is_closed = old_closed.get(old_idx).copied().unwrap_or(true) && on_curve_count > 1;
            if state.active_open_contour == Some(old_idx) && !is_closed {
                new_active_open = Some(new_idx);
            }
            kept_contours.push(contour);
            kept_closed.push(is_closed);
            kept_meta.push(old_meta.get(old_idx).copied().unwrap_or((crate::core::ShapeBooleanRole::Normal, true)));
        }
    }

    state.contours = kept_contours;
    state.active_open_contour = new_active_open.or_else(|| kept_closed.iter().rposition(|&closed| !closed));
    state.active_contour_closed = kept_closed.last().copied().unwrap_or(false);
    state.glyph.contours = state
        .contours
        .iter()
        .enumerate()
        .map(|(idx, nodes)| {
            let mut gc = crate::core::GlyphContour::new(crate::core::ContourId(idx), nodes.clone());
            gc.closed = kept_closed[idx];
            gc.role = kept_meta[idx].0;
            gc.visible = kept_meta[idx].1;
            gc
        })
        .collect();
}

/// Processes keyboard events for view mode overrides, tool switching, nudging, zoom, and undo/redo.
pub fn dispatch_keyboard_event(
    state: &mut GlifoCanvasState,
    event: &Event,
    is_shift: bool,
    is_cmd: bool,
    is_alt: bool,
    canvas_rect: Rect,
) -> bool {
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
            if state.active_tool == CanvasTool::Pen && state.pen_state != crate::interaction::PenToolState::Idle {
                return false;
            }
            if matches!(state.select_state, crate::interaction::SelectToolState::MarqueeSelecting { .. }) {
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
            if state.active_tool == CanvasTool::Pen && state.pen_state != crate::interaction::PenToolState::Idle {
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
            if matches!(state.select_state, crate::interaction::SelectToolState::PanningViewport { .. }) {
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
            if state.selected_nodes.is_empty() {
                return state.nudge_selected_guide(dx, dy);
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
                if state.find_node(sel_id).is_some_and(|n| n.node_type == NodeType::OffCurve) {
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
                        && on_curve_ids.iter().all(|id| state.selected_nodes.contains(id));
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
                    if let Some(next_id) = cycle_selection_tab(
                        &state.contours,
                        &state.selected_nodes,
                        shift_active,
                    ) {
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
                    if toggle_selected_on_curve_smooth(&mut state.contours, &closed_flags, &selected_copy) {
                        state.sync_paths_from_contours();
                        state.record_mutation("Toggle Smooth", before);
                        state.assert_invariants();
                        return true;
                    }
                    false
                }
                Key::Backspace | Key::Delete => {
                    if state.selected_nodes.is_empty() {
                        return state.delete_selected_guide();
                    }
                    let before = state.snapshot();
                    let cut_open = shift_active || alt_active;
                    if cut_open {
                        for c_idx in 0..state.contours.len() {
                            let has_sel = state.contours[c_idx].iter().any(|n| state.selected_nodes.contains(&n.id));
                            if has_sel {
                                let is_closed = state.is_contour_closed(c_idx);
                                crate::core::cut_open_delete_nodes_in_contour(
                                    &mut state.contours[c_idx],
                                    is_closed,
                                    &state.selected_nodes,
                                );
                                if !state.contours[c_idx].is_empty() {
                                    state.set_contour_closed(c_idx, false);
                                    state.active_open_contour = Some(c_idx);
                                }
                            }
                        }
                    } else {
                        for c_idx in 0..state.contours.len() {
                            let has_sel = state.contours[c_idx].iter().any(|n| state.selected_nodes.contains(&n.id));
                            if has_sel {
                                let is_closed = state.is_contour_closed(c_idx);
                                crate::core::heal_delete_nodes(&mut state.contours[c_idx], is_closed, &state.selected_nodes, &mut state.next_node_id);
                                if state.contours[c_idx].len() <= 1 {
                                    state.set_contour_closed(c_idx, false);
                                }
                            }
                        }
                    }
                    state.selected_nodes.clear();
                    state.hovered_segment_hit = None;
                    state.hovered_pen_close_target = None;
                    state.hovered_pen_close_node = None;
                    state.pending_segment_click = None;
                    state.pending_pen_bcp_out = None;
                    state.pending_pen_first_bcp_in = None;
                    prune_empty_contours(state);
                    state.sync_paths_from_contours();
                    state.record_mutation(if cut_open { "Cut Contour Open" } else { "Delete Nodes" }, before);
                    state.assert_invariants();
                    true
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
