//! `SPEC-009` (`grafia v0.8.0` Part B) `GrafiaKeymap` runtime binding and dispatch (`state.apply_keymap`).

use crate::adapters::state::{CanvasTool, GrafiaCanvasState};
use crate::core::{NodeId, NodeType, ScreenPoint};
use crate::tokens::{
    AutoTraceConfig, CanvasActionId, GrafiaKeymap, KeyChord, ZOOM_STEP_IN_FACTOR,
    ZOOM_STEP_OUT_FACTOR,
};
use egui::{Event, Key, Rect};

#[derive(Debug, Clone, Copy)]
pub(crate) struct FramePointerFlags {
    pub has_ptr_btn: bool,
    pub has_ptr_move: bool,
    pub has_touch_start: bool,
    pub from_touch: bool,
}

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

    pub(crate) fn should_ignore_companion_touch_release(
        &self,
        time: f64,
        has_touch_start: bool,
    ) -> bool {
        let ts = &self.drag_transients.touch_state;
        !ts.primary_press_active
            && !has_touch_start
            && ts.last_raw_press_was_touch
            && time > 0.0
            && ts.last_touch_event_time_sec > 0.0
            && (time - ts.last_touch_event_time_sec).abs()
                <= crate::core::math_consts::TOUCH_COMPANION_MOUSE_DEDUP_SEC
    }

    pub(crate) fn deduplicate_companion_touch_press(
        &mut self,
        pos: egui::Pos2,
        time: f64,
        from_touch: bool,
        has_touch_start: bool,
    ) -> bool {
        if self.is_palm_companion_pointer_event(pos, Some(true)) {
            return true;
        }
        let now_touch = self.active_input_device() == crate::core::InputDeviceKind::TouchFinger
            || from_touch
            || has_touch_start;
        let ts = &self.drag_transients.touch_state;
        let (prev_t, prev_touch, active, last_touch_t) = (
            ts.last_raw_press_time_sec,
            ts.last_raw_press_was_touch,
            ts.primary_press_active,
            ts.last_touch_event_time_sec,
        );
        let max_dt = if active {
            crate::core::math_consts::TOUCH_ACTIVE_PRESS_DEDUP_SEC
        } else {
            crate::core::math_consts::TOUCH_SAME_PRESS_DEDUP_SEC
        };
        let same_or_active_dup = prev_t > 0.0 && (time - prev_t).abs() <= max_dt;
        let post_touch_compat_dup = !from_touch
            && !has_touch_start
            && prev_touch
            && last_touch_t > 0.0
            && (time - last_touch_t).abs()
                <= crate::core::math_consts::TOUCH_COMPANION_MOUSE_DEDUP_SEC;
        if now_touch && time > 0.0 && (same_or_active_dup || post_touch_compat_dup) {
            if prev_touch {
                return true;
            }
            if let Some(before) = self.drag_transients.active_drag_before_snapshot.take() {
                self.restore_snapshot(&before);
            }
            self.drag_transients.last_click_time_pos = None;
            self.drag_transients.last_double_click_time_pos = None;
        }
        let ts_mut = &mut self.drag_transients.touch_state;
        ts_mut.last_raw_press_time_sec = time;
        ts_mut.last_raw_press_was_touch = now_touch;
        ts_mut.primary_press_active = true;
        ts_mut.native_pointer_drag_active =
            !from_touch && !has_touch_start && ts_mut.contacts.is_empty();
        ts_mut.native_pointer_raw_pos = Some(pos);
        false
    }

    pub(crate) fn finish_pen_pointer_release(&mut self) -> bool {
        self.pen_state = crate::interaction::PenToolState::Idle;
        self.active_snappers.clear();
        self.pen_transients.drag_last_pt = None;
        self.pen_transients.handle_delta = (0.0, 0.0);
        self.pen_transients.resuming_corner_line_end = false;
        self.pen_transients.join_b_had_out_handles = false;
        self.pen_transients.join_b_was_smooth = false;
        if self
            .active_open_contour
            .is_some_and(|c| self.is_contour_closed(c))
        {
            if let Some(c_idx) = self.active_open_contour.take() {
                self.try_reflect_contour_at_index(c_idx);
            }
        }
        if let Some(before) = self.drag_transients.active_drag_before_snapshot.take() {
            self.record_mutation("Pen Draw", before);
        }
        self.assert_invariants();
        true
    }

    pub(crate) fn sync_frame_touch_and_device_state(
        &mut self,
        events: &[Event],
        ui: &egui::Ui,
        canvas_rect: Rect,
        time: f64,
    ) -> FramePointerFlags {
        let ui_any_touches = ui.input(|i| i.any_touches());
        let mut has_touch_ev = false;
        let mut has_touch_start = false;
        let mut frame_started_ids = Vec::new();
        for e in events {
            if let Event::Touch { id, phase, pos, .. } = e {
                if *phase == egui::TouchPhase::Start && !canvas_rect.contains(*pos) {
                    continue;
                }
                let is_known_same_frame = frame_started_ids.contains(&id.0);
                if is_known_same_frame || !self.classify_palm_touch_event(id.0, *phase, *pos) {
                    has_touch_ev = true;
                    if *phase == egui::TouchPhase::Start {
                        has_touch_start = true;
                        frame_started_ids.push(id.0);
                    }
                    if time > 0.0 {
                        self.drag_transients.touch_state.last_touch_event_time_sec = time;
                    }
                }
            }
        }
        let any_touches = (!self.drag_transients.touch_state.native_pointer_drag_active
            && self
                .drag_transients
                .touch_state
                .rejected_palm_contacts
                .is_empty()
            && ui_any_touches)
            || has_touch_ev;
        self.drag_transients.touch_state.egui_any_touches_in_frame = any_touches;
        if any_touches {
            self.set_active_input_device(crate::core::InputDeviceKind::TouchFinger);
        }
        if let Some(pos) = ui.input(|i| i.pointer.latest_pos()) {
            if canvas_rect.contains(pos) && !self.is_palm_companion_pointer_event(pos, None) {
                self.last_pointer_screen_pos = Some(ScreenPoint::new(pos.x, pos.y));
            }
        }
        let has_ptr_btn = events.iter().any(|e| {
            matches!(e, Event::PointerButton { pos, .. } if !self.is_palm_companion_pointer_event(*pos, None))
        });
        let has_ptr_move = events.iter().any(|e| {
            matches!(e, Event::PointerMoved(pos) if !self.is_palm_companion_pointer_event(*pos, None))
        });
        FramePointerFlags {
            has_ptr_btn,
            has_ptr_move,
            has_touch_start,
            from_touch: false,
        }
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
