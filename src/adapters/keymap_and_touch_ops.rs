//! `SPEC-009` (`grafia v0.8.0` Part B) `GrafiaKeymap` runtime binding and dispatch (`state.apply_keymap`).

use crate::adapters::state::{CanvasTool, GrafiaCanvasState};
use crate::core::{NodeId, NodeType, ScreenPoint};
use crate::tokens::{
    AutoTraceConfig, CanvasActionId, GrafiaKeymap, KeyChord, ZOOM_STEP_IN_FACTOR,
    ZOOM_STEP_OUT_FACTOR,
};
use egui::{Event, Key, Rect};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum KeymapDispatchOutcome {
    Handled(bool),
    SuppressedReboundDefault,
    Unmatched,
}

impl GrafiaCanvasState {
    /// Applies a rebindable `GrafiaKeymap` to this canvas state (`SPEC-009 §2.6.4`).
    pub fn apply_keymap(&mut self, keymap: &GrafiaKeymap) {
        let mut clean = GrafiaKeymap {
            bindings: std::collections::BTreeMap::new(),
        };
        for (&action, &chord) in &keymap.bindings {
            clean.set_binding(action, chord);
        }
        self.drag_transients.keymap = clean;
        self.bump_content_generation();
    }
}

pub(crate) fn dispatch_keymap_event(
    state: &mut GrafiaCanvasState,
    event: &Event,
    is_shift: bool,
    is_alt: bool,
    is_cmd: bool,
    is_ctrl: bool,
    canvas_rect: Rect,
) -> KeymapDispatchOutcome {
    let Event::Key {
        key,
        pressed: true,
        modifiers,
        ..
    } = event
    else {
        return KeymapDispatchOutcome::Unmatched;
    };

    // Preserve context-specific Annotation Cmd+A/C/X/V and Measurement 'G'
    let cmd_any = modifiers.command || modifiers.ctrl || is_cmd || is_ctrl;
    let shift = modifiers.shift || is_shift;
    let alt = modifiers.alt || is_alt;
    if state.active_tool == CanvasTool::Annotation
        && cmd_any
        && !shift
        && !alt
        && matches!(key, Key::A | Key::C | Key::X | Key::V)
    {
        return KeymapDispatchOutcome::Unmatched;
    }
    if state.active_tool == CanvasTool::Measurement
        && state.drawing_tool_state.measurement.active
        && *key == Key::G
        && !cmd_any
        && !alt
    {
        return KeymapDispatchOutcome::Unmatched;
    }

    let chord = KeyChord {
        key: *key,
        cmd: cmd_any,
        ctrl: false,
        alt,
        shift,
    };
    let exact_chord = KeyChord {
        key: *key,
        cmd: modifiers.mac_cmd || (modifiers.command && !modifiers.ctrl) || is_cmd,
        ctrl: modifiers.ctrl || is_ctrl,
        alt,
        shift,
    };

    if let Some(action) = state
        .drag_transients
        .keymap
        .action_for_chord(exact_chord)
        .or_else(|| state.drag_transients.keymap.action_for_chord(chord))
    {
        return KeymapDispatchOutcome::Handled(execute_canvas_action(state, action, canvas_rect));
    }

    let defaults = GrafiaKeymap::default();
    if let Some(def_action) = defaults
        .action_for_chord(exact_chord)
        .or_else(|| defaults.action_for_chord(chord))
    {
        if state.drag_transients.keymap.chord_for(def_action) != defaults.chord_for(def_action) {
            return KeymapDispatchOutcome::SuppressedReboundDefault;
        }
    }

    KeymapDispatchOutcome::Unmatched
}

fn execute_canvas_action(
    state: &mut GrafiaCanvasState,
    action: CanvasActionId,
    canvas_rect: Rect,
) -> bool {
    if let Some(tool) = action.to_tool() {
        if tool == CanvasTool::Primitives && state.active_tool == CanvasTool::Primitives {
            state.drawing_tool_state.primitive_sub_tool =
                state.drawing_tool_state.primitive_sub_tool.cycle_next();
        }
        if tool == CanvasTool::Text && state.active_tool != CanvasTool::Text {
            state.text_transients.last_select_tool = state.active_tool;
            state.text_transients.text_tool_activated_in_frame = true;
        }
        state.activate_tool(tool);
        return true;
    }
    match action {
        CanvasActionId::CyclePrimitivesSubTool => {
            state.drawing_tool_state.primitive_sub_tool =
                state.drawing_tool_state.primitive_sub_tool.cycle_next();
            state.activate_tool(CanvasTool::Primitives);
            true
        }
        CanvasActionId::CycleAnnotationSubTool => {
            state.drawing_tool_state.annotation_sub_tool =
                state.drawing_tool_state.annotation_sub_tool.cycle_next();
            state.activate_tool(CanvasTool::Annotation);
            true
        }
        CanvasActionId::Undo => state.undo(),
        CanvasActionId::Redo => state.redo(),
        CanvasActionId::ZoomIn | CanvasActionId::ZoomOut | CanvasActionId::ZoomToFit => {
            execute_viewport_zoom_action(state, action, canvas_rect)
        }
        CanvasActionId::SelectAll => execute_select_all_action(state),
        CanvasActionId::DeselectAll => {
            state.selected_nodes.clear();
            state.drawing_tool_state.selected_annotations.clear();
            state.clear_anchor_and_component_selections();
            state.assert_invariants();
            true
        }
        CanvasActionId::ReverseContours => state.correct_path_direction(),
        CanvasActionId::MakeNodeFirst => state.make_selected_node_first(),
        CanvasActionId::RemoveOverlap => state.remove_overlap(),
        CanvasActionId::ToggleBoundingBox => {
            state.show_bounding_box = !state.show_bounding_box;
            true
        }
        CanvasActionId::SaveViewportBookmark => {
            state.save_viewport_bookmark();
            true
        }
        CanvasActionId::ToggleViewportBookmark => state.toggle_saved_viewport_bookmark(),
        CanvasActionId::SwitchActiveBackgroundLayer => {
            state.toggle_editing_background_layer();
            true
        }
        CanvasActionId::SwapForegroundWithBackground => {
            state.swap_foreground_and_background();
            true
        }
        CanvasActionId::TraceBackgroundImage => {
            state.trace_active_background_image(&AutoTraceConfig::default())
        }
        _ => false,
    }
}

fn execute_viewport_zoom_action(
    state: &mut GrafiaCanvasState,
    action: CanvasActionId,
    canvas_rect: Rect,
) -> bool {
    match action {
        CanvasActionId::ZoomIn => {
            let c = ScreenPoint::new(canvas_rect.center().x, canvas_rect.center().y);
            state.viewport.zoom_at_cursor(c, ZOOM_STEP_IN_FACTOR);
            true
        }
        CanvasActionId::ZoomOut => {
            let c = ScreenPoint::new(canvas_rect.center().x, canvas_rect.center().y);
            state.viewport.zoom_at_cursor(c, ZOOM_STEP_OUT_FACTOR);
            true
        }
        CanvasActionId::ZoomToFit => {
            state.fit_viewport_to_glyph(canvas_rect.width(), canvas_rect.height());
            true
        }
        _ => false,
    }
}

fn execute_select_all_action(state: &mut GrafiaCanvasState) -> bool {
    let on_ids: Vec<NodeId> = state
        .contours
        .iter()
        .flat_map(|c| c.iter())
        .filter(|n| n.node_type != NodeType::OffCurve)
        .map(|n| n.id)
        .collect();
    let all_on = !on_ids.is_empty() && on_ids.iter().all(|id| state.selected_nodes.contains(id));
    state.selected_nodes = if all_on {
        state
            .contours
            .iter()
            .flat_map(|c| c.iter().map(|n| n.id))
            .collect()
    } else {
        on_ids
    };
    state.assert_invariants();
    true
}
