//! Gated Input Dispatcher for keyboard shortcuts, viewport navigation, and pointer event isolation.

use crate::adapters::keyboard::dispatch_keyboard_event;
use crate::adapters::pen_interaction::{handle_pen_move, handle_pen_press};
use crate::adapters::select_interaction::{
    apply_marquee_selection, apply_segment_click_selection,
    duplicate_selected_contours_for_alt_drag, handle_select_press,
};
use crate::adapters::state::{CanvasTool, GlifoCanvasState};
use crate::core::{
    apply_cubic_segment_belly_drag_with_alt, snap_vector_45_degrees, FontPoint, ScreenPoint,
};
use crate::interaction::{Modifiers, PenToolState, SelectToolState};
use crate::tokens::{GlifoTheme, WHEEL_ZOOM_SENSITIVITY};
use egui::{Event, PointerButton, Rect};

/// Dispatches input events to canvas tools and state machines with boundary gating.
pub struct InputDispatcher<'a> {
    pub state: &'a mut GlifoCanvasState,
    pub theme: &'a GlifoTheme,
    pub canvas_rect: Rect,
}

impl<'a> InputDispatcher<'a> {
    #[must_use]
    pub fn new(state: &'a mut GlifoCanvasState, theme: &'a GlifoTheme, canvas_rect: Rect) -> Self {
        Self { state, theme, canvas_rect }
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

        for event in events {
            if dispatch_keyboard_event(self.state, event, is_shift, is_cmd, is_alt, self.canvas_rect) {
                changed = true;
            }
            if self.dispatch_pointer_event(event, is_shift, is_alt, is_cmd, is_ctrl, is_space, time) {
                changed = true;
            }
        }
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
            Event::MouseWheel { delta, modifiers, .. } => {
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
            Event::PointerButton { pos, button: PointerButton::Primary, pressed: true, modifiers } => {
                if !self.canvas_rect.contains(*pos) {
                    return false;
                }
                let scr_pt = ScreenPoint::new(pos.x, pos.y);
                self.state.last_pointer_screen_pos = Some(scr_pt);
                let font_pt = self.state.viewport.to_font(scr_pt);

                let alt_active = modifiers.alt || is_alt;
                let shift_active = modifiers.shift || is_shift;
                let cmd_active = modifiers.command || is_cmd;
                let ctrl_active = modifiers.ctrl || is_ctrl;

                if is_space && !cmd_active && !ctrl_active && self.state.pen_state == PenToolState::Idle {
                    self.state.select_state = SelectToolState::PanningViewport { last_screen: scr_pt };
                    return true;
                }

                let is_triple = self.state.check_triple_click(scr_pt, time);
                let is_double = if is_triple { false } else { self.state.check_double_click(scr_pt, time) };

                let mods = Modifiers {
                    shift: shift_active,
                    option: alt_active,
                    command: cmd_active,
                    control: ctrl_active,
                    space: is_space,
                };
                self.state.last_modifiers = mods;
                if matches!(self.state.active_tool, CanvasTool::Knife | CanvasTool::Pencil)
                    && cmd_active
                    && !ctrl_active
                    && !alt_active
                    && !is_space
                    && !self.state.temp_zoom_via_key_space
                {
                    self.state.temp_tool_before_override = Some(self.state.active_tool);
                    self.state.active_tool = CanvasTool::Select;
                    return handle_select_press(self.state, scr_pt, font_pt, mods, is_double, is_triple);
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
                    return handle_select_press(self.state, scr_pt, font_pt, mods, is_double, is_triple);
                } else if self.state.active_tool == CanvasTool::Pen {
                    return handle_pen_press(self.state, scr_pt, font_pt, shift_active);
                }
                false
            }
            Event::PointerMoved(pos) => {
                let scr_pt = ScreenPoint::new(pos.x, pos.y);
                self.state.last_pointer_screen_pos = Some(scr_pt);
                let font_pt = self.state.viewport.to_font(scr_pt);
                self.handle_pointer_move(scr_pt, font_pt, is_shift, is_alt, is_cmd, is_ctrl, is_space)
            }
            Event::PointerButton { pos, button: PointerButton::Primary, pressed: false, modifiers } => {
                let font_pt = self.state.viewport.to_font(ScreenPoint::new(pos.x, pos.y));
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

    #[allow(clippy::too_many_arguments)]
    fn handle_pointer_move(
        &mut self,
        scr_pt: ScreenPoint,
        font_pt: FontPoint,
        is_shift: bool,
        is_alt: bool,
        is_cmd: bool,
        is_ctrl: bool,
        is_space: bool,
    ) -> bool {
        if self.state.is_v040_tool() {
            let mods = Modifiers {
                shift: is_shift,
                option: is_alt,
                command: is_cmd,
                control: is_ctrl,
                space: is_space,
            };
            return self.state.on_v040_pointer_move(scr_pt, font_pt, mods);
        }
        if let Some(handled) = crate::adapters::v020_interaction::handle_v020_pointer_move(
            self.state, scr_pt, font_pt, is_shift, is_alt, is_ctrl, is_space || self.state.marquee_modifiers.space,
        ) {
            return handled;
        }
        let mut moved_changed = false;
        if let Some(start_screen) = self.state.alt_marquee_pending_start.take() {
            self.state.select_state = SelectToolState::MarqueeSelecting {
                start_screen,
                current_screen: scr_pt,
            };
        }
        match self.state.select_state.clone() {
            SelectToolState::PanningViewport { last_screen } => {
                self.state.viewport.pan_offset.x.0 += scr_pt.x.0 - last_screen.x.0;
                self.state.viewport.pan_offset.y.0 += scr_pt.y.0 - last_screen.y.0;
                self.state.select_state = SelectToolState::PanningViewport { last_screen: scr_pt };
                return true;
            }
            SelectToolState::MarqueeSelecting { start_screen, .. } => {
                if start_screen.distance_to(scr_pt) > 1.0 {
                    self.state.pending_segment_click = None;
                }
                self.state.select_state = SelectToolState::MarqueeSelecting {
                    start_screen,
                    current_screen: scr_pt,
                };
                if self.state.pending_segment_click.is_none() {
                    apply_marquee_selection(self.state, self.theme, is_alt);
                }
                moved_changed = true;
            }
            SelectToolState::DraggingNodes { selected, start_pt, modifiers, .. } => {
                let dx = font_pt.x.0 - start_pt.x.0;
                let dy = font_pt.y.0 - start_pt.y.0;
                self.state.select_state = SelectToolState::DraggingNodes {
                    selected,
                    start_pt,
                    current_pt: font_pt,
                    modifiers,
                };
                if dx.hypot(dy) > 0.0 {
                    self.state.pending_segment_click = None;
                    if self.state.alt_duplicate_pending {
                        self.state.alt_duplicate_pending = false;
                        if !(is_ctrl || modifiers.control) {
                            duplicate_selected_contours_for_alt_drag(self.state);
                        }
                    }
                }
                let alt_for_move = is_alt || modifiers.option;
                let cmd_for_move = is_cmd || modifiers.command;
                let ctrl_for_move = (is_ctrl || modifiers.control)
                    && (alt_for_move || (self.state.active_tool == CanvasTool::Select && cmd_for_move));
                let shift_for_move = is_shift || modifiers.shift;
                let ctrl_snap_bypass = is_ctrl || modifiers.control;
                let (eff_dx, eff_dy) = self.state.resolve_node_drag_displacement(
                    dx,
                    dy,
                    shift_for_move,
                    alt_for_move,
                    ctrl_for_move,
                    ctrl_snap_bypass,
                );
                self.state.apply_node_displacement_with_modifiers_and_ctrl(
                    eff_dx,
                    eff_dy,
                    shift_for_move,
                    alt_for_move,
                    ctrl_for_move,
                );
                moved_changed = true;
            }
            SelectToolState::DraggingSegment {
                contour_idx,
                segment,
                t,
                start_pt,
                initial_h1,
                initial_h2,
            } => {
                let raw_dx = font_pt.x.0 - start_pt.x.0;
                let raw_dy = font_pt.y.0 - start_pt.y.0;
                if raw_dx.hypot(raw_dy) > 0.0 {
                    self.state.pending_segment_click = None;
                }
                for (id, pt, nt) in self.state.drag_start_node_types.clone() {
                    if let Some(node) = self.state.find_node_mut(id) {
                        node.point = pt;
                        node.node_type = nt;
                    }
                }
                let (dx, dy) = if is_shift {
                    snap_vector_45_degrees(raw_dx, raw_dy)
                } else {
                    (raw_dx, raw_dy)
                };
                let closed = self.state.is_contour_closed(contour_idx);
                if let Some(contour) = self.state.contours.get_mut(contour_idx) {
                    apply_cubic_segment_belly_drag_with_alt(
                        contour, closed, segment, t, initial_h1, initial_h2, dx, dy, is_alt,
                    );
                }
                self.state.sync_paths_from_contours();
                moved_changed = true;
            }
            SelectToolState::Idle => {}
        }

        if handle_pen_move(self.state, scr_pt, font_pt, is_shift, is_alt, is_space, self.canvas_rect) {
            moved_changed = true;
        }
        moved_changed
    }

    fn handle_pointer_release(
        &mut self,
        font_pt: FontPoint,
        is_shift: bool,
        is_alt: bool,
        is_ctrl: bool,
    ) -> bool {
        if self.state.is_v040_tool() {
            let mods = Modifiers {
                shift: is_shift,
                option: is_alt,
                command: self.state.last_modifiers.command,
                control: is_ctrl,
                space: self.state.last_modifiers.space,
            };
            return self.state.on_v040_pointer_release(font_pt, mods);
        }
        if let Some(handled) = crate::adapters::v020_interaction::handle_v020_pointer_release(
            self.state,
            is_shift,
            is_alt,
            is_ctrl,
            self.canvas_rect,
        ) {
            return handled;
        }
        self.state.alt_marquee_pending_start = None;
        self.state.alt_duplicate_pending = false;
        if let SelectToolState::PanningViewport { .. } = self.state.select_state {
            self.state.select_state = SelectToolState::Idle;
            self.state.assert_invariants();
            return true;
        }
        if let Some((c_idx, seg, mods)) = self.state.pending_segment_click.take() {
            apply_segment_click_selection(self.state, c_idx, seg, mods);
            self.state.select_state = SelectToolState::Idle;
            self.state.active_drag_before_snapshot = None;
            self.state.assert_invariants();
            return true;
        }
        if let SelectToolState::MarqueeSelecting { .. } = self.state.select_state {
            apply_marquee_selection(self.state, self.theme, is_alt);
            self.state.marquee_modifiers.space = false;
            self.state.select_state = SelectToolState::Idle;
            self.state.assert_invariants();
            return true;
        }
        if matches!(
            self.state.select_state,
            SelectToolState::DraggingNodes { .. } | SelectToolState::DraggingSegment { .. }
        ) {
            if matches!(
                self.state.select_state,
                SelectToolState::DraggingNodes { start_pt, current_pt, .. } if start_pt != current_pt
            ) {
                crate::adapters::contour_join::try_join_or_close_open_contours_on_select_release(self.state);
            }
            self.state.select_state = SelectToolState::Idle;
            self.state.active_snappers.clear();
            if let Some(before) = self.state.active_drag_before_snapshot.take() {
                self.state.record_mutation("Drag Edit", before);
            }
            self.state.assert_invariants();
            return true;
        }
        if matches!(
            self.state.pen_state,
            PenToolState::DraggingHandles { .. } | PenToolState::PlacingAnchor { .. }
        ) {
            self.state.pen_state = PenToolState::Idle;
            self.state.active_snappers.clear();
            self.state.pen_drag_last_pt = None;
            self.state.pen_handle_delta = (0.0, 0.0);
            self.state.pen_resuming_corner_line_end = false;
            self.state.pen_join_b_had_out_handles = false;
            self.state.pen_join_b_was_smooth = false;
            if self.state.active_open_contour.is_some_and(|c| self.state.is_contour_closed(c)) {
                self.state.active_open_contour = None;
            }
            if let Some(before) = self.state.active_drag_before_snapshot.take() {
                self.state.record_mutation("Pen Draw", before);
            }
            self.state.assert_invariants();
            return true;
        }
        false
    }
}
