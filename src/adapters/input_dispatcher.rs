//! Gated Input Dispatcher for keyboard shortcuts, viewport navigation, and pointer event isolation.

use crate::adapters::keyboard::dispatch_keyboard_event;
use crate::adapters::pen_interaction::handle_pen_press;
use crate::adapters::select_interaction::handle_select_press;
use crate::adapters::state::{CanvasTool, GrafiaCanvasState};
use crate::core::ScreenPoint;
use crate::interaction::{Modifiers, PenToolState, SelectToolState};
use crate::tokens::{GrafiaTheme, WHEEL_ZOOM_SENSITIVITY};
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
        self.state.last_modifiers = Modifiers {
            shift: is_shift,
            option: is_alt,
            command: is_cmd,
            control: is_ctrl,
            space: is_space,
        };
        crate::adapters::v040_tools::sync_temp_measurement_modifier_override(
            self.state, is_cmd, is_ctrl, is_alt, is_space,
        );
        if let Some(pos) = ui.input(|i| i.pointer.latest_pos()) {
            if self.canvas_rect.contains(pos) {
                self.state.last_pointer_screen_pos = Some(ScreenPoint::new(pos.x, pos.y));
            }
        }
        let mut changed = false;
        self.state.text_tool_activated_in_frame = false;
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
            if self.dispatch_pointer_event(event, is_shift, is_alt, is_cmd, is_ctrl, is_space, time)
            {
                changed = true;
            }
        }
        self.state.text_tool_activated_in_frame = false;
        changed
    }

    #[allow(clippy::too_many_arguments)]
    fn dispatch_pointer_event(
        &mut self,
        event: &Event,
        is_shift: bool,
        is_alt: bool,
        is_cmd: bool,
        is_ctrl: bool,
        is_space: bool,
        time: f64,
    ) -> bool {
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
            } => {
                let zoom_mod = modifiers.command
                    || modifiers.ctrl
                    || modifiers.alt
                    || is_cmd
                    || is_ctrl
                    || is_alt;
                if zoom_mod {
                    let cursor_scr = self.state.last_pointer_screen_pos.unwrap_or_else(|| {
                        ScreenPoint::new(self.canvas_rect.center().x, self.canvas_rect.center().y)
                    });
                    let step = if delta.y.abs() <= 2.0 {
                        delta.y * 0.5 * 0.5
                    } else {
                        delta.y * WHEEL_ZOOM_SENSITIVITY
                    };
                    let factor = (1.0 + step).clamp(0.5, 2.0);
                    self.state.viewport.zoom_at_cursor(cursor_scr, factor);
                } else {
                    self.state.viewport.pan_offset.x.0 += delta.x;
                    self.state.viewport.pan_offset.y.0 += delta.y;
                }
                true
            }
            Event::PointerButton {
                pos,
                button: PointerButton::Primary,
                pressed: true,
                modifiers,
            } => {
                if !self.canvas_rect.contains(*pos) {
                    return false;
                }
                let scr_pt = ScreenPoint::new(pos.x, pos.y);
                self.state.last_pointer_screen_pos = Some(scr_pt);
                let strip_font_pt = self.state.viewport.to_font(scr_pt);

                let (alt_active, shift_active) =
                    (modifiers.alt || is_alt, modifiers.shift || is_shift);
                let (cmd_active, ctrl_active) =
                    (modifiers.command || is_cmd, modifiers.ctrl || is_ctrl);

                if is_space
                    && !cmd_active
                    && !ctrl_active
                    && self.state.pen_state == PenToolState::Idle
                {
                    self.state.select_state = SelectToolState::PanningViewport {
                        last_screen: scr_pt,
                    };
                    return true;
                }

                let is_triple = self.state.check_triple_click(scr_pt, time);
                let is_double = if is_triple {
                    false
                } else {
                    self.state.check_double_click(scr_pt, time)
                };

                let mods = Modifiers {
                    shift: shift_active,
                    option: alt_active,
                    command: cmd_active,
                    control: ctrl_active,
                    space: is_space,
                };
                self.state.last_modifiers = mods;
                if self
                    .state
                    .handle_v060_hud_pointer_press(scr_pt, self.canvas_rect)
                {
                    return true;
                }
                if crate::adapters::v060_layers::handle_v060_ruler_pointer_press(
                    self.state,
                    scr_pt,
                    self.canvas_rect,
                ) {
                    return true;
                }
                if let Some(handled) = crate::adapters::v050_interaction::handle_v050_pointer_press(
                    self.state,
                    scr_pt,
                    strip_font_pt,
                    mods,
                    is_double,
                ) {
                    return handled;
                }
                let font_pt = self.state.active_slot_viewport().to_font(scr_pt);
                if matches!(
                    self.state.active_tool,
                    CanvasTool::Knife | CanvasTool::Pencil
                ) && cmd_active
                    && !ctrl_active
                    && !alt_active
                    && !is_space
                    && !self.state.temp_zoom_via_key_space
                {
                    self.state.temp_tool_before_override = Some(self.state.active_tool);
                    self.state.active_tool = CanvasTool::Select;
                    return handle_select_press(
                        self.state, scr_pt, font_pt, mods, is_double, is_triple,
                    );
                }
                if self.state.is_v040_tool() {
                    return self.state.on_v040_pointer_press(scr_pt, font_pt, mods);
                }
                if let Some(handled) = crate::adapters::v020_interaction::handle_v020_pointer_press(
                    self.state, scr_pt, font_pt, mods, is_double, is_triple,
                ) {
                    return handled;
                }

                if self.state.active_tool == CanvasTool::Select
                    || (self.state.active_tool == CanvasTool::Pen && cmd_active)
                {
                    return handle_select_press(
                        self.state, scr_pt, font_pt, mods, is_double, is_triple,
                    );
                } else if self.state.active_tool == CanvasTool::Pen {
                    return handle_pen_press(self.state, scr_pt, font_pt, shift_active);
                }
                false
            }
            Event::PointerMoved(pos) => {
                let scr_pt = ScreenPoint::new(pos.x, pos.y);
                self.state.last_pointer_screen_pos = Some(scr_pt);
                let strip_font_pt = self.state.viewport.to_font(scr_pt);
                if let Some(handled) = crate::adapters::v050_interaction::handle_v050_pointer_move(
                    self.state,
                    scr_pt,
                    strip_font_pt,
                ) {
                    return handled;
                }
                let font_pt = self.state.active_slot_viewport().to_font(scr_pt);
                self.handle_pointer_move(
                    scr_pt, font_pt, is_shift, is_alt, is_cmd, is_ctrl, is_space,
                )
            }
            Event::PointerButton {
                pos,
                button: PointerButton::Primary,
                pressed: false,
                modifiers,
            } => {
                let scr_pt = ScreenPoint::new(pos.x, pos.y);
                self.state.last_pointer_screen_pos = Some(scr_pt);
                let font_pt = self.state.active_slot_viewport().to_font(scr_pt);
                let res = self.handle_pointer_release(
                    font_pt,
                    is_shift || modifiers.shift,
                    is_alt || modifiers.alt,
                    is_ctrl || modifiers.ctrl,
                );
                crate::adapters::v020_interaction::restore_temp_tool_override(self.state);
                res
            }
            _ => false,
        }
    }
}
