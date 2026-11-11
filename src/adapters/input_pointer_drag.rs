//! Pointer move and release handlers for `InputDispatcher`.

use crate::adapters::input_dispatcher::InputDispatcher;
use crate::adapters::pen_interaction::handle_pen_move;
use crate::adapters::select_interaction::{
    apply_marquee_selection, apply_segment_click_selection,
    duplicate_selected_contours_for_alt_drag,
};
use crate::adapters::state::CanvasTool;
use crate::core::{
    apply_cubic_segment_belly_drag_with_alt, snap_vector_45_degrees, FontPoint, ScreenPoint,
};
use crate::interaction::{Modifiers, PenToolState, SelectToolState};

impl<'a> InputDispatcher<'a> {
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn handle_pointer_move(
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
            self.state,
            scr_pt,
            font_pt,
            is_shift,
            is_alt,
            is_ctrl,
            is_space || self.state.marquee_modifiers.space,
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
                self.state.select_state = SelectToolState::PanningViewport {
                    last_screen: scr_pt,
                };
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
                    apply_marquee_selection(self.state, is_alt);
                }
                moved_changed = true;
            }
            SelectToolState::DraggingNodes {
                selected,
                start_pt,
                modifiers,
                ..
            } => {
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
                    && (alt_for_move
                        || (self.state.active_tool == CanvasTool::Select && cmd_for_move));
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

        if handle_pen_move(
            self.state,
            scr_pt,
            font_pt,
            is_shift,
            is_alt,
            is_space,
            self.canvas_rect,
        ) {
            moved_changed = true;
        }
        moved_changed
    }

    pub(crate) fn handle_pointer_release(
        &mut self,
        font_pt: FontPoint,
        is_shift: bool,
        is_alt: bool,
        is_ctrl: bool,
    ) -> bool {
        let scr_pt = self
            .state
            .last_pointer_screen_pos
            .unwrap_or_else(|| ScreenPoint::new(0.0, 0.0));
        if crate::adapters::v060_layers::handle_v060_ruler_pointer_release(
            self.state, scr_pt, is_alt,
        ) {
            return true;
        }
        if let Some(handled) =
            crate::adapters::v050_interaction::handle_v050_pointer_release(self.state)
        {
            return handled;
        }
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
            apply_marquee_selection(self.state, is_alt);
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
                crate::adapters::contour_join::try_join_or_close_open_contours_on_select_release(
                    self.state,
                );
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
            if self
                .state
                .active_open_contour
                .is_some_and(|c| self.state.is_contour_closed(c))
            {
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
