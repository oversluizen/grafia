//! Keyboard shortcut dispatching for canvas navigation and manipulation (SPEC-002 §3.3 & §4).

use crate::adapters::keyboard_edit::{delete_selected_nodes_or_guide, handle_arrow_nudge};
use crate::adapters::state::{CanvasTool, CanvasViewMode, GrafiaCanvasState};
use crate::core::{
    cycle_selection_tab, toggle_selected_on_curve_smooth, NodeId, NodeType, ScreenPoint,
};
use crate::tokens::{ZOOM_STEP_IN_FACTOR, ZOOM_STEP_OUT_FACTOR};
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
    let is_ctrl = state.drag_transients.last_modifiers.control;
    if let Event::Key {
        key,
        pressed: true,
        modifiers,
        ..
    } = event
    {
        if !modifiers.command
            && !modifiers.ctrl
            && !modifiers.alt
            && !is_cmd
            && !is_ctrl
            && !is_alt
            && state.handle_truetype_instructor_key(*key)
        {
            return true;
        }
    }
    if crate::adapters::component_and_layer_controller::dispatch_component_and_layer_keyboard_shortcut(
        state, event, is_shift, is_alt, is_cmd, is_ctrl,
    ) || crate::adapters::text_stream_interaction::dispatch_text_stream_keyboard_shortcut(
        state, event, is_shift, is_alt, is_cmd, is_ctrl,
    ) || crate::adapters::background_and_ruler_ops::dispatch_layer_and_ruler_keyboard_shortcut(
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
        } => handle_space_pressed(state, *modifiers, is_shift, is_cmd, is_alt),
        Event::Key {
            key: Key::Space,
            pressed: false,
            modifiers,
            ..
        } => handle_space_released(state, *modifiers, is_shift, is_cmd, is_alt),
        Event::Key {
            key: key @ (Key::ArrowLeft | Key::ArrowRight | Key::ArrowUp | Key::ArrowDown),
            pressed: true,
            modifiers,
            ..
        } => handle_arrow_nudge(state, *key, *modifiers, is_shift, is_cmd, is_alt),
        Event::Key {
            key,
            pressed: true,
            modifiers,
            ..
        } => handle_general_key_press(
            state,
            event,
            *key,
            *modifiers,
            KeyModifierFlags {
                shift: is_shift,
                alt: is_alt,
                cmd: is_cmd,
                ctrl: is_ctrl,
            },
            canvas_rect,
        ),
        _ => false,
    }
}

fn handle_space_pressed(
    state: &mut GrafiaCanvasState,
    modifiers: egui::Modifiers,
    is_shift: bool,
    is_cmd: bool,
    is_alt: bool,
) -> bool {
    state.drag_transients.last_modifiers.shift = modifiers.shift || is_shift;
    state.drag_transients.last_modifiers.option = modifiers.alt || is_alt;
    state.drag_transients.last_modifiers.command = modifiers.command || is_cmd;
    state.drag_transients.last_modifiers.control = modifiers.ctrl;
    state.drag_transients.last_modifiers.space = true;
    if state.active_tool == CanvasTool::Pen
        && state.pen_state != crate::interaction::PenToolState::Idle
    {
        return false;
    }
    if matches!(
        state.select_state,
        crate::interaction::SelectToolState::MarqueeSelecting { .. }
    ) {
        state.drag_transients.marquee_modifiers.space = true;
        return true;
    }
    if modifiers.command || modifiers.ctrl || is_cmd {
        if state.drag_transients.temp_tool_before_override.is_none() {
            state.drag_transients.temp_tool_before_override = Some(state.active_tool);
        }
        state.drag_transients.temp_zoom_via_key_space = true;
        state.active_tool = CanvasTool::Zoom;
        if let Some(prev_vm) = state.drag_transients.prev_view_mode.take() {
            state.view_mode = prev_vm;
        }
        return true;
    }
    if state.drag_transients.prev_view_mode.is_none() {
        state.drag_transients.prev_view_mode = Some(state.view_mode);
    }
    state.view_mode = CanvasViewMode::PreviewFilled;
    true
}

fn handle_space_released(
    state: &mut GrafiaCanvasState,
    modifiers: egui::Modifiers,
    is_shift: bool,
    is_cmd: bool,
    is_alt: bool,
) -> bool {
    state.drag_transients.last_modifiers.shift = modifiers.shift || is_shift;
    state.drag_transients.last_modifiers.option = modifiers.alt || is_alt;
    state.drag_transients.last_modifiers.command = modifiers.command || is_cmd;
    state.drag_transients.last_modifiers.control = modifiers.ctrl;
    state.drag_transients.last_modifiers.space = false;
    state.drag_transients.marquee_modifiers.space = false;
    if state.active_tool == CanvasTool::Pen
        && state.pen_state != crate::interaction::PenToolState::Idle
    {
        return false;
    }
    if state.drag_transients.temp_zoom_via_key_space || state.active_tool == CanvasTool::Zoom {
        state.drag_transients.temp_zoom_via_key_space = false;
        if let Some(prev) = state.drag_transients.temp_tool_before_override.take() {
            state.active_tool = prev;
            return true;
        }
    }
    state.view_mode = state
        .drag_transients
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

#[derive(Debug, Clone, Copy)]
pub(crate) struct KeyModifierFlags {
    pub shift: bool,
    pub alt: bool,
    pub cmd: bool,
    pub ctrl: bool,
}

fn handle_general_key_press(
    state: &mut GrafiaCanvasState,
    event: &Event,
    key: Key,
    modifiers: egui::Modifiers,
    flags: KeyModifierFlags,
    canvas_rect: Rect,
) -> bool {
    let KeyModifierFlags {
        shift: is_shift,
        alt: is_alt,
        cmd: is_cmd,
        ctrl: is_ctrl,
    } = flags;
    let cmd_active = modifiers.command || modifiers.ctrl || is_cmd;
    let shift_active = modifiers.shift || is_shift;
    let alt_active = modifiers.alt || is_alt;

    match crate::adapters::keymap_and_touch_ops::dispatch_keymap_event(
        state,
        event,
        is_shift,
        is_alt,
        is_cmd,
        is_ctrl,
        canvas_rect,
    ) {
        crate::adapters::keymap_and_touch_ops::KeymapDispatchOutcome::Handled(ch) => return ch,
        crate::adapters::keymap_and_touch_ops::KeymapDispatchOutcome::SuppressedReboundDefault => {
            return false;
        }
        crate::adapters::keymap_and_touch_ops::KeymapDispatchOutcome::Unmatched => {}
    }
    if crate::adapters::drawing_tool_state::dispatch_drawing_tool_keyboard_shortcut(
        state,
        key,
        shift_active,
        alt_active,
        cmd_active,
    ) || crate::adapters::contour_edit_interaction::dispatch_transform_and_clipboard_shortcut(
        state,
        key,
        shift_active,
        alt_active,
        cmd_active,
    ) {
        return true;
    }
    if cmd_active {
        handle_cmd_shortcut_key(state, key, shift_active, canvas_rect)
    } else {
        handle_unmodified_shortcut_key(state, key, shift_active, alt_active)
    }
}

fn handle_cmd_shortcut_key(
    state: &mut GrafiaCanvasState,
    key: Key,
    shift_active: bool,
    canvas_rect: Rect,
) -> bool {
    let center = ScreenPoint::new(canvas_rect.center().x, canvas_rect.center().y);
    match key {
        Key::A => {
            let on_curve_ids: Vec<NodeId> = state
                .contours
                .iter()
                .flatten()
                .filter(|n| n.node_type != NodeType::OffCurve)
                .map(|n| n.id)
                .collect();
            let all_on = !on_curve_ids.is_empty()
                && on_curve_ids
                    .iter()
                    .all(|id| state.selected_nodes.contains(id));
            state.selected_nodes = if all_on {
                state.contours.iter().flatten().map(|n| n.id).collect()
            } else {
                on_curve_ids
            };
            state.assert_invariants();
            true
        }
        Key::D => {
            state.selected_nodes.clear();
            state.drawing_tool_state.selected_annotations.clear();
            state.clear_anchor_and_component_selections();
            state.assert_invariants();
            true
        }
        Key::R if shift_active => state.reverse_selected_or_all_contours(),
        Key::F if shift_active => state.make_selected_node_first(),
        Key::Equals | Key::Plus => {
            state.viewport.zoom_at_cursor(center, ZOOM_STEP_IN_FACTOR);
            true
        }
        Key::Minus => {
            state.viewport.zoom_at_cursor(center, ZOOM_STEP_OUT_FACTOR);
            true
        }
        Key::Num0 => {
            state.fit_viewport_to_glyph(canvas_rect.width(), canvas_rect.height());
            true
        }
        Key::Z if !shift_active => state.undo(),
        Key::Z if shift_active => state.redo(),
        Key::Y => state.redo(),
        _ => false,
    }
}

fn handle_unmodified_shortcut_key(
    state: &mut GrafiaCanvasState,
    key: Key,
    shift_active: bool,
    alt_active: bool,
) -> bool {
    match key {
        Key::V => {
            state.active_tool = CanvasTool::Select;
            state.pen_state = crate::interaction::PenToolState::Idle;
            state.hovered_segment_hit = None;
            state.hovered_pen_close_target = None;
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
        Key::P => {
            state.active_tool = CanvasTool::Pen;
            state.select_state = crate::interaction::SelectToolState::Idle;
            true
        }
        Key::Escape => {
            state.selected_nodes.clear();
            state.selected_guide = None;
            state.drawing_tool_state.selected_annotations.clear();
            state.clear_anchor_and_component_selections();
            state.drawing_tool_state.reset_in_flight_drags();
            state.select_state = crate::interaction::SelectToolState::Idle;
            state.pen_state = crate::interaction::PenToolState::Idle;
            state.active_open_contour = None;
            state.hovered_segment_hit = None;
            state.hovered_pen_close_target = None;
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
            if toggle_selected_on_curve_smooth(&mut state.contours, &closed_flags, &selected_copy) {
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
        _ => false,
    }
}
