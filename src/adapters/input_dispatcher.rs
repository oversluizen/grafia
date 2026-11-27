//! Gated Input Dispatcher for keyboard shortcuts, viewport navigation, and pointer event isolation.

use crate::adapters::keyboard::dispatch_keyboard_event;
use crate::adapters::keymap_and_touch_ops::FramePointerFlags;
use crate::adapters::pen_interaction::handle_pen_press;
use crate::adapters::select_interaction::handle_select_press;
use crate::adapters::state::{CanvasTool, GrafiaCanvasState};
use crate::core::ScreenPoint;
use crate::interaction::{Modifiers, PenToolState, SelectToolState};
use crate::tokens::GrafiaTheme;
use egui::{Event, PointerButton, Rect};

/// Dispatches input events to canvas tools and state machines with boundary gating.
pub struct InputDispatcher<'a> {
    pub state: &'a mut GrafiaCanvasState,
    pub canvas_rect: Rect,
}

impl<'a> InputDispatcher<'a> {
    #[must_use]
    pub fn new(state: &'a mut GrafiaCanvasState, _theme: &GrafiaTheme, canvas_rect: Rect) -> Self {
        Self { state, canvas_rect }
    }

    #[must_use]
    pub fn for_canvas(state: &'a mut GrafiaCanvasState, canvas_rect: Rect) -> Self {
        Self { state, canvas_rect }
    }

    /// Dispatches all pending frame events and returns whether the glyph was mutated.
    pub fn dispatch_events(&mut self, events: &[Event], ui: &egui::Ui) -> bool {
        let ts = &self.state.drag_transients.touch_state;
        let is_shift = ui.input(|i| i.modifiers.shift) || ts.touch_shift_latched;
        let is_alt = ui.input(|i| i.modifiers.alt) || ts.touch_alt_latched;
        let is_cmd = ui.input(|i| i.modifiers.command) || ts.touch_cmd_latched;
        let is_ctrl = ui.input(|i| i.modifiers.ctrl) || ts.touch_cmd_latched;
        let is_space = ui.input(|i| i.key_down(egui::Key::Space)) || ts.touch_space_pan_latched;
        let time = ui.input(|i| i.time);
        let flags =
            self.state
                .sync_frame_touch_and_device_state(events, ui, self.canvas_rect, time);
        let raw_mods = Modifiers {
            shift: is_shift,
            option: is_alt,
            command: is_cmd,
            control: is_ctrl,
            space: is_space,
        };
        let frame_mods = self
            .state
            .resolve_modifiers_with_intents(raw_mods, is_space);
        self.state.drag_transients.last_modifiers = frame_mods;
        crate::adapters::drawing_tool_state::sync_temp_measurement_modifier_override(
            self.state, is_cmd, is_ctrl, is_alt, is_space,
        );
        let mut changed = false;
        self.state.text_transients.text_tool_activated_in_frame = false;
        for event in events {
            if dispatch_keyboard_event(
                self.state,
                event,
                is_shift,
                is_cmd,
                is_alt,
                self.canvas_rect,
            ) {
                changed = true;
            }
            if self.dispatch_pointer_event(event, frame_mods, time, flags) {
                changed = true;
            }
        }
        self.state.text_transients.text_tool_activated_in_frame = false;
        changed
    }

    fn dispatch_touch_if_present(
        &mut self,
        event: &Event,
        frame_mods: Modifiers,
        time: f64,
        flags: FramePointerFlags,
    ) -> Option<bool> {
        if let Event::Touch { id, phase, pos, .. } = event {
            if *phase == egui::TouchPhase::Start && !self.canvas_rect.contains(*pos) {
                return Some(false);
            }
            if self.state.classify_palm_touch_event(id.0, *phase, *pos) {
                return Some(false);
            }
            if time > 0.0 {
                self.state
                    .drag_transients
                    .touch_state
                    .last_touch_event_time_sec = time;
            }
        }
        let handled = self.state.handle_touch_event(event)?;
        if handled || self.state.drag_transients.touch_state.two_finger_peak {
            return Some(handled);
        }
        if let Event::Touch { phase, pos, .. } = event {
            let mut touch_flags = flags;
            touch_flags.from_touch = true;
            return Some(self.synthesize_single_touch_pointer(
                *phase,
                *pos,
                frame_mods,
                time,
                touch_flags,
            ));
        }
        Some(false)
    }

    fn dispatch_pointer_event(
        &mut self,
        event: &Event,
        frame_mods: Modifiers,
        time: f64,
        flags: FramePointerFlags,
    ) -> bool {
        if let Some(res) = self.dispatch_touch_if_present(event, frame_mods, time, flags) {
            return res;
        }
        if self.state.drag_transients.touch_state.two_finger_peak {
            return false;
        }
        match event {
            Event::Zoom(factor) => {
                let cursor_scr = self.state.last_pointer_screen_pos.unwrap_or_else(|| {
                    ScreenPoint::new(self.canvas_rect.center().x, self.canvas_rect.center().y)
                });
                self.state.viewport.zoom_at_cursor(cursor_scr, *factor);
                true
            }
            Event::MouseWheel {
                delta, modifiers, ..
            } => self.state.handle_mouse_wheel_with_prefs(
                *delta,
                *modifiers,
                frame_mods.command,
                frame_mods.control,
                frame_mods.option,
                self.canvas_rect,
            ),
            Event::PointerButton {
                pos,
                button: PointerButton::Primary,
                pressed: true,
                modifiers,
            } => self.handle_primary_pointer_press(*pos, *modifiers, frame_mods, time, flags),
            Event::PointerMoved(pos) => {
                if self.state.is_palm_companion_pointer_event(*pos, None) {
                    return false;
                }
                let ts = &mut self.state.drag_transients.touch_state;
                if ts.native_pointer_drag_active {
                    ts.native_pointer_raw_pos = Some(*pos);
                }
                self.dispatch_pointer_moved_at(*pos, frame_mods)
            }
            Event::PointerButton {
                pos,
                button: PointerButton::Primary,
                pressed: false,
                modifiers,
            } => {
                if self
                    .state
                    .is_palm_companion_pointer_event(*pos, Some(false))
                    || self
                        .state
                        .should_ignore_companion_touch_release(time, flags.has_touch_start)
                {
                    return false;
                }
                let scr_pt = self.state.map_pointer_pos_with_touch_offset(*pos);
                self.state.last_pointer_screen_pos = Some(scr_pt);
                let font_pt = self.state.active_slot_viewport().to_font(scr_pt);
                let res = self.handle_pointer_release(
                    font_pt,
                    frame_mods.shift || modifiers.shift,
                    frame_mods.option || modifiers.alt,
                    frame_mods.control || modifiers.ctrl,
                );
                crate::adapters::contour_edit_interaction::restore_temp_tool_override(self.state);
                res
            }
            _ => false,
        }
    }

    fn dispatch_pointer_moved_at(&mut self, pos: egui::Pos2, frame_mods: Modifiers) -> bool {
        let scr_pt = self.state.map_pointer_pos_with_touch_offset(pos);
        self.state.last_pointer_screen_pos = Some(scr_pt);
        self.state.cursor_screen_pos = Some(scr_pt);
        let strip_font_pt = self.state.viewport.to_font(scr_pt);
        if let Some(handled) =
            crate::adapters::text_stream_interaction::handle_text_stream_pointer_move(
                self.state,
                scr_pt,
                strip_font_pt,
            )
        {
            return handled;
        }
        let font_pt = self.state.active_slot_viewport().to_font(scr_pt);
        self.handle_pointer_move(scr_pt, font_pt, frame_mods)
    }

    fn synthesize_single_touch_pointer(
        &mut self,
        phase: egui::TouchPhase,
        pos: egui::Pos2,
        frame_mods: Modifiers,
        time: f64,
        flags: FramePointerFlags,
    ) -> bool {
        let FramePointerFlags {
            has_ptr_btn,
            has_ptr_move,
            ..
        } = flags;
        match phase {
            egui::TouchPhase::Start if !has_ptr_btn => self.handle_primary_pointer_press(
                pos,
                egui::Modifiers::NONE,
                frame_mods,
                time,
                flags,
            ),
            egui::TouchPhase::Move if !has_ptr_move => {
                self.dispatch_pointer_moved_at(pos, frame_mods)
            }
            egui::TouchPhase::Cancel => {
                self.state.cancel_in_flight_interaction_and_rollback();
                crate::adapters::contour_edit_interaction::restore_temp_tool_override(self.state);
                false
            }
            egui::TouchPhase::End if !has_ptr_btn => {
                let scr_pt = self.state.map_pointer_pos_with_touch_offset(pos);
                self.state.last_pointer_screen_pos = Some(scr_pt);
                let font_pt = self.state.active_slot_viewport().to_font(scr_pt);
                let res = self.handle_pointer_release(
                    font_pt,
                    frame_mods.shift,
                    frame_mods.option,
                    frame_mods.control,
                );
                crate::adapters::contour_edit_interaction::restore_temp_tool_override(self.state);
                res
            }
            _ => false,
        }
    }

    fn handle_primary_pointer_press(
        &mut self,
        pos: egui::Pos2,
        modifiers: egui::Modifiers,
        frame_mods: Modifiers,
        time: f64,
        flags: FramePointerFlags,
    ) -> bool {
        let scr_pt = self.state.map_pointer_pos_with_touch_offset(pos);
        let adj_pos = egui::Pos2::new(scr_pt.x.0, scr_pt.y.0);
        if (!self.canvas_rect.contains(pos) && !self.canvas_rect.contains(adj_pos))
            || self.state.deduplicate_companion_touch_press(
                pos,
                time,
                flags.from_touch,
                flags.has_touch_start,
            )
        {
            return false;
        }
        self.state.last_pointer_screen_pos = Some(scr_pt);
        self.state.cursor_screen_pos = Some(scr_pt);
        let strip_font_pt = self.state.viewport.to_font(scr_pt);

        let raw_mods = Modifiers {
            shift: modifiers.shift || frame_mods.shift,
            option: modifiers.alt || frame_mods.option,
            command: modifiers.command || frame_mods.command,
            control: modifiers.ctrl || frame_mods.control,
            space: frame_mods.space,
        };
        let mods = self
            .state
            .resolve_modifiers_with_intents(raw_mods, raw_mods.space);
        if mods.space
            && !mods.command
            && !mods.control
            && self.state.pen_state == PenToolState::Idle
        {
            self.state.select_state = SelectToolState::PanningViewport {
                last_screen: scr_pt,
            };
            return true;
        }

        let is_triple = self.state.check_triple_click(scr_pt, time);
        let is_double = !is_triple && self.state.check_double_click(scr_pt, time);
        self.state.drag_transients.last_modifiers = mods;

        if self
            .state
            .handle_hud_pointer_press(scr_pt, self.canvas_rect)
            || crate::adapters::background_and_ruler_ops::handle_ruler_pointer_press(
                self.state,
                scr_pt,
                self.canvas_rect,
            )
        {
            return true;
        }
        if let Some(handled) =
            crate::adapters::text_stream_interaction::handle_text_stream_pointer_press(
                self.state,
                scr_pt,
                strip_font_pt,
                mods,
                is_double,
            )
        {
            return handled;
        }
        self.dispatch_primary_press_to_active_tool(scr_pt, mods, is_double, is_triple)
    }

    fn dispatch_primary_press_to_active_tool(
        &mut self,
        scr_pt: ScreenPoint,
        mods: Modifiers,
        is_double: bool,
        is_triple: bool,
    ) -> bool {
        let font_pt = self.state.active_slot_viewport().to_font(scr_pt);
        if matches!(
            self.state.active_tool,
            CanvasTool::Knife | CanvasTool::Pencil
        ) && mods.command
            && !mods.control
            && !mods.option
            && !mods.space
            && !self.state.drag_transients.temp_zoom_via_key_space
        {
            self.state.drag_transients.temp_tool_before_override = Some(self.state.active_tool);
            self.state.active_tool = CanvasTool::Select;
            return handle_select_press(self.state, scr_pt, font_pt, mods, is_double, is_triple);
        }
        if self.state.is_drawing_or_measurement_tool() {
            return self
                .state
                .on_drawing_tool_pointer_press(scr_pt, font_pt, mods);
        }
        if let Some(handled) =
            crate::adapters::smart_handle_interaction::handle_smart_handle_pointer_press(
                self.state, scr_pt, font_pt,
            )
        {
            return handled;
        }
        if self.state.try_begin_gradient_handle_press(scr_pt) {
            return true;
        }
        if let Some(handled) =
            crate::adapters::contour_edit_interaction::handle_bbox_and_transform_pointer_press(
                self.state, scr_pt, font_pt, mods, is_double, is_triple,
            )
        {
            return handled;
        }
        if self.state.active_tool == CanvasTool::TrueTypeInstructor {
            if self.state.try_click_select_truetype_instruction(scr_pt) {
                return true;
            }
            return handle_select_press(self.state, scr_pt, font_pt, mods, is_double, is_triple);
        }
        if self.state.active_tool == CanvasTool::Select
            || (self.state.active_tool == CanvasTool::Pen && mods.command)
        {
            return handle_select_press(self.state, scr_pt, font_pt, mods, is_double, is_triple);
        } else if self.state.active_tool == CanvasTool::Pen {
            return handle_pen_press(self.state, scr_pt, font_pt, mods.shift);
        }
        false
    }
}
