//! Gated Input Dispatcher for keyboard shortcuts, viewport navigation, and pointer event isolation.

use crate::adapters::keyboard::dispatch_keyboard_event;
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
        let is_shift = ui.input(|i| i.modifiers.shift);
        let is_alt = ui.input(|i| i.modifiers.alt);
        let is_cmd = ui.input(|i| i.modifiers.command);
        let is_ctrl = ui.input(|i| i.modifiers.ctrl);
        let is_space = ui.input(|i| i.key_down(egui::Key::Space));
        let time = ui.input(|i| i.time);
        self.state
            .drag_transients
            .touch_state
            .egui_any_touches_in_frame = ui.input(|i| i.any_touches());
        let frame_mods = Modifiers {
            shift: is_shift,
            option: is_alt,
            command: is_cmd,
            control: is_ctrl,
            space: is_space,
        };
        self.state.drag_transients.last_modifiers = frame_mods;
        crate::adapters::drawing_tool_state::sync_temp_measurement_modifier_override(
            self.state, is_cmd, is_ctrl, is_alt, is_space,
        );
        if let Some(pos) = ui.input(|i| i.pointer.latest_pos()) {
            if self.canvas_rect.contains(pos) {
                self.state.last_pointer_screen_pos = Some(ScreenPoint::new(pos.x, pos.y));
            }
        }
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
            if self.dispatch_pointer_event(event, frame_mods, time) {
                changed = true;
            }
        }
        self.state.text_transients.text_tool_activated_in_frame = false;
        changed
    }

    fn dispatch_pointer_event(&mut self, event: &Event, frame_mods: Modifiers, time: f64) -> bool {
        if let Some(handled) = self.state.handle_touch_event(event) {
            return handled;
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
            } => self.handle_primary_pointer_press(*pos, *modifiers, frame_mods, time),
            Event::PointerMoved(pos) => {
                let scr_pt = self.state.map_pointer_pos_with_touch_offset(*pos);
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
            Event::PointerButton {
                pos,
                button: PointerButton::Primary,
                pressed: false,
                modifiers,
            } => {
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

    fn handle_primary_pointer_press(
        &mut self,
        pos: egui::Pos2,
        modifiers: egui::Modifiers,
        frame_mods: Modifiers,
        time: f64,
    ) -> bool {
        let scr_pt = self.state.map_pointer_pos_with_touch_offset(pos);
        let adj_pos = egui::Pos2::new(scr_pt.x.0, scr_pt.y.0);
        if !self.canvas_rect.contains(pos) && !self.canvas_rect.contains(adj_pos) {
            return false;
        }
        self.state.last_pointer_screen_pos = Some(scr_pt);
        self.state.cursor_screen_pos = Some(scr_pt);
        let strip_font_pt = self.state.viewport.to_font(scr_pt);

        let mods = Modifiers {
            shift: modifiers.shift || frame_mods.shift,
            option: modifiers.alt || frame_mods.option,
            command: modifiers.command || frame_mods.command,
            control: modifiers.ctrl || frame_mods.control,
            space: frame_mods.space,
        };
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
