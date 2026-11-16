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

#[derive(Debug, Clone, Copy)]
struct SegmentBellyDragTarget {
    contour_idx: usize,
    segment: crate::core::ContourSegment,
    t: f64,
}

#[derive(Debug, Clone, Copy)]
struct SegmentBellyInitialPoints {
    start_pt: FontPoint,
    initial_h1: FontPoint,
    initial_h2: FontPoint,
}

impl<'a> InputDispatcher<'a> {
    pub(crate) fn handle_pointer_move(
        &mut self,
        scr_pt: ScreenPoint,
        font_pt: FontPoint,
        mods: Modifiers,
    ) -> bool {
        if self.state.try_update_gradient_handle_drag(font_pt) {
            return true;
        }
        if self.state.is_drawing_or_measurement_tool() {
            return self
                .state
                .on_drawing_tool_pointer_move(scr_pt, font_pt, mods);
        }
        if let Some(handled) =
            crate::adapters::smart_handle_interaction::handle_smart_handle_pointer_move(
                self.state, scr_pt, font_pt,
            )
        {
            return handled;
        }
        if let Some(handled) =
            crate::adapters::contour_edit_interaction::handle_bbox_and_transform_pointer_move(
                self.state,
                scr_pt,
                font_pt,
                mods.shift,
                mods.option,
                mods.control,
                mods.space || self.state.drag_transients.marquee_modifiers.space,
            )
        {
            return handled;
        }
        if let Some(start_screen) = self.state.drag_transients.alt_marquee_pending_start.take() {
            self.state.select_state = SelectToolState::MarqueeSelecting {
                start_screen,
                current_screen: scr_pt,
            };
        }
        if matches!(
            self.state.select_state,
            SelectToolState::PanningViewport { .. }
        ) {
            if let SelectToolState::PanningViewport { last_screen } = self.state.select_state {
                self.state.viewport.pan_offset.x.0 += scr_pt.x.0 - last_screen.x.0;
                self.state.viewport.pan_offset.y.0 += scr_pt.y.0 - last_screen.y.0;
                self.state.select_state = SelectToolState::PanningViewport {
                    last_screen: scr_pt,
                };
            }
            return true;
        }
        let mut moved_changed = self.handle_select_state_pointer_move(scr_pt, font_pt, mods);
        if handle_pen_move(
            self.state,
            scr_pt,
            font_pt,
            mods.shift,
            mods.option,
            mods.space,
            self.canvas_rect,
        ) {
            moved_changed = true;
        }
        moved_changed
    }

    fn handle_select_state_pointer_move(
        &mut self,
        scr_pt: ScreenPoint,
        font_pt: FontPoint,
        mods: Modifiers,
    ) -> bool {
        match self.state.select_state.clone() {
            SelectToolState::MarqueeSelecting { start_screen, .. } => {
                if start_screen.distance_to(scr_pt) > 1.0 {
                    self.state.drag_transients.pending_segment_click = None;
                }
                self.state.select_state = SelectToolState::MarqueeSelecting {
                    start_screen,
                    current_screen: scr_pt,
                };
                if self.state.drag_transients.pending_segment_click.is_none() {
                    apply_marquee_selection(self.state, mods.option);
                }
                true
            }
            SelectToolState::DraggingNodes {
                selected,
                start_pt,
                modifiers,
                ..
            } => {
                self.handle_dragging_nodes_move(font_pt, mods, selected, start_pt, modifiers);
                true
            }
            SelectToolState::DraggingSegment {
                contour_idx,
                segment,
                t,
                start_pt,
                initial_h1,
                initial_h2,
            } => {
                self.handle_segment_belly_drag_move(
                    font_pt,
                    mods,
                    SegmentBellyDragTarget {
                        contour_idx,
                        segment,
                        t,
                    },
                    SegmentBellyInitialPoints {
                        start_pt,
                        initial_h1,
                        initial_h2,
                    },
                );
                true
            }
            SelectToolState::DraggingTunniLine {
                contour_id,
                segment_index,
            } => {
                self.state
                    .update_active_tunni_drag(contour_id, segment_index, font_pt);
                true
            }
            SelectToolState::PanningViewport { .. } | SelectToolState::Idle => false,
        }
    }

    fn handle_dragging_nodes_move(
        &mut self,
        font_pt: FontPoint,
        mods: Modifiers,
        selected: Vec<crate::core::NodeId>,
        start_pt: FontPoint,
        modifiers: Modifiers,
    ) {
        let (dx, dy) = (font_pt.x.0 - start_pt.x.0, font_pt.y.0 - start_pt.y.0);
        self.state.select_state = SelectToolState::DraggingNodes {
            selected,
            start_pt,
            current_pt: font_pt,
            modifiers,
        };
        if dx.hypot(dy) > 0.0 {
            self.state.drag_transients.pending_segment_click = None;
            if self.state.drag_transients.alt_duplicate_pending {
                self.state.drag_transients.alt_duplicate_pending = false;
                if !(mods.control || modifiers.control) {
                    duplicate_selected_contours_for_alt_drag(self.state);
                }
            }
        }
        let alt_for_move = mods.option || modifiers.option;
        let cmd_for_move = mods.command || modifiers.command;
        let ctrl_for_move = (mods.control || modifiers.control)
            && (alt_for_move || (self.state.active_tool == CanvasTool::Select && cmd_for_move));
        let shift_for_move = mods.shift || modifiers.shift;
        let ctrl_snap_bypass = mods.control || modifiers.control;
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
    }

    fn handle_segment_belly_drag_move(
        &mut self,
        font_pt: FontPoint,
        mods: Modifiers,
        seg_hit: SegmentBellyDragTarget,
        initial_pts: SegmentBellyInitialPoints,
    ) {
        let SegmentBellyDragTarget {
            contour_idx,
            segment,
            t,
        } = seg_hit;
        let SegmentBellyInitialPoints {
            start_pt,
            initial_h1,
            initial_h2,
        } = initial_pts;
        let raw_dx = font_pt.x.0 - start_pt.x.0;
        let raw_dy = font_pt.y.0 - start_pt.y.0;
        if raw_dx.hypot(raw_dy) > 0.0 {
            self.state.drag_transients.pending_segment_click = None;
        }
        for (id, pt, nt) in self.state.drag_transients.drag_start_node_types.clone() {
            if let Some(node) = self.state.find_node_mut(id) {
                node.point = pt;
                node.node_type = nt;
            }
        }
        let (dx, dy) = if mods.shift {
            snap_vector_45_degrees(raw_dx, raw_dy)
        } else {
            (raw_dx, raw_dy)
        };
        let closed = self.state.is_contour_closed(contour_idx);
        if let Some(contour) = self.state.contours.get_mut(contour_idx) {
            apply_cubic_segment_belly_drag_with_alt(
                contour,
                closed,
                segment,
                t,
                (initial_h1, initial_h2),
                (dx, dy),
                mods.option,
            );
        }
        self.state.sync_paths_from_contours();
    }

    pub(crate) fn handle_pointer_release(
        &mut self,
        font_pt: FontPoint,
        is_shift: bool,
        is_alt: bool,
        is_ctrl: bool,
    ) -> bool {
        if self.state.try_finish_gradient_handle_release() {
            return true;
        }
        let scr_pt = self
            .state
            .last_pointer_screen_pos
            .unwrap_or_else(|| ScreenPoint::new(0.0, 0.0));
        if crate::adapters::background_and_ruler_ops::handle_ruler_pointer_release(
            self.state, scr_pt, is_alt,
        ) {
            return true;
        }
        if let Some(handled) =
            crate::adapters::text_stream_interaction::handle_text_stream_pointer_release(self.state)
        {
            return handled;
        }
        if self.state.is_drawing_or_measurement_tool() {
            let mods = Modifiers {
                shift: is_shift,
                option: is_alt,
                command: self.state.drag_transients.last_modifiers.command,
                control: is_ctrl,
                space: self.state.drag_transients.last_modifiers.space,
            };
            return self.state.on_drawing_tool_pointer_release(font_pt, mods);
        }
        if let Some(handled) =
            crate::adapters::smart_handle_interaction::handle_smart_handle_pointer_release(
                self.state,
            )
        {
            return handled;
        }
        if let Some(handled) =
            crate::adapters::contour_edit_interaction::handle_bbox_and_transform_pointer_release(
                self.state,
                is_shift,
                is_alt,
                is_ctrl,
                self.canvas_rect,
            )
        {
            return handled;
        }
        self.finish_select_or_pen_pointer_release(is_alt)
    }

    fn finish_select_or_pen_pointer_release(&mut self, is_alt: bool) -> bool {
        self.state.drag_transients.alt_marquee_pending_start = None;
        self.state.drag_transients.alt_duplicate_pending = false;
        if let SelectToolState::PanningViewport { .. } = self.state.select_state {
            self.state.select_state = SelectToolState::Idle;
            self.state.assert_invariants();
            return true;
        }
        if let Some((c_idx, seg, mods)) = self.state.drag_transients.pending_segment_click.take() {
            apply_segment_click_selection(self.state, c_idx, seg, mods);
            self.state.select_state = SelectToolState::Idle;
            self.state.drag_transients.active_drag_before_snapshot = None;
            self.state.assert_invariants();
            return true;
        }
        if let SelectToolState::MarqueeSelecting { .. } = self.state.select_state {
            apply_marquee_selection(self.state, is_alt);
            self.state.drag_transients.marquee_modifiers.space = false;
            self.state.select_state = SelectToolState::Idle;
            self.state.assert_invariants();
            return true;
        }
        if matches!(
            self.state.select_state,
            SelectToolState::DraggingNodes { .. }
                | SelectToolState::DraggingSegment { .. }
                | SelectToolState::DraggingTunniLine { .. }
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
            self.state.curve_harmony.active_tangent_snap = None;
            let before_opt = self
                .state
                .drag_transients
                .active_drag_before_snapshot
                .take();
            if let Some(before) = before_opt {
                self.state.record_mutation("Drag Edit", before);
            }
            self.state.assert_invariants();
            return true;
        }
        if matches!(
            self.state.pen_state,
            PenToolState::DraggingHandles { .. } | PenToolState::PlacingAnchor { .. }
        ) {
            return self.finish_pen_pointer_release();
        }
        false
    }

    fn finish_pen_pointer_release(&mut self) -> bool {
        self.state.pen_state = PenToolState::Idle;
        self.state.active_snappers.clear();
        self.state.pen_transients.drag_last_pt = None;
        self.state.pen_transients.handle_delta = (0.0, 0.0);
        self.state.pen_transients.resuming_corner_line_end = false;
        self.state.pen_transients.join_b_had_out_handles = false;
        self.state.pen_transients.join_b_was_smooth = false;
        if self
            .state
            .active_open_contour
            .is_some_and(|c| self.state.is_contour_closed(c))
        {
            if let Some(c_idx) = self.state.active_open_contour.take() {
                self.state.try_reflect_contour_at_index(c_idx);
            }
        }
        if let Some(before) = self
            .state
            .drag_transients
            .active_drag_before_snapshot
            .take()
        {
            self.state.record_mutation("Pen Draw", before);
        }
        self.state.assert_invariants();
        true
    }
}
