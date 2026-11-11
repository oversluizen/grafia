//! `SPEC-006` (`grafia v0.5.0`) `GrafiaCanvasState` methods for Multi-Glyph Horizontal Text Strip
//! (`CanvasTextStream`), `TextTool` (`T`), Interactive Spacing (`LSB` / `RSB` / Contour Shift),
//! Kerning Delta Emission, and Text-Mode Measurement Scanline.

use crate::adapters::state::{CanvasTool, GrafiaCanvasState};
use crate::adapters::v020_tools::V020DragState;
use crate::adapters::v030_guides::GuideDragState;
pub use crate::adapters::v050_keys::{
    dispatch_v050_keyboard_shortcut, handle_v050_pointer_move, handle_v050_pointer_press,
    handle_v050_pointer_release,
};
use crate::core::{
    compute_glyph_lsb_and_rsb, CanvasHostRequest, ContourId, FontPoint, GlyphSlot, KerningDelta,
    KerningIndicatorQuad, NodeId, NodeType, SlotScanlineMeasurement, SpacingDelta,
    TextStreamEditKind, TextStreamEditRequest,
};
use crate::interaction::{CommandHistory, PenToolState, SelectToolState};
use std::collections::HashSet;

impl GrafiaCanvasState {
    /// Normalizes a `Glyph` inside a slot so all `ContourId`s and `NodeId`s are unique and valid.
    pub(crate) fn normalize_slot_glyph(&mut self, glyph: &mut crate::core::Glyph) {
        glyph.upm = glyph.upm.max(1);
        if glyph.height <= 0.0 {
            glyph.height = self.metrics.cap_height;
        }
        let mut seen_ids = HashSet::new();
        let max_existing = glyph
            .contours
            .iter()
            .flat_map(|gc| gc.nodes.iter().map(|n| n.id.0 + 1))
            .max()
            .unwrap_or(1);
        let mut next_id = self.next_node_id.max(max_existing);
        for (idx, gc) in glyph.contours.iter_mut().enumerate() {
            gc.id = ContourId(idx);
            for n in &mut gc.nodes {
                if n.id.0 == 0 || !seen_ids.insert(n.id) {
                    n.id = NodeId(next_id);
                    seen_ids.insert(n.id);
                    next_id += 1;
                }
            }
            let on_curve_count = gc
                .nodes
                .iter()
                .filter(|n| n.node_type != NodeType::OffCurve)
                .count();
            if on_curve_count <= 1 {
                gc.closed = false;
                gc.nodes.retain(|n| n.node_type != NodeType::OffCurve);
            } else if !gc.closed {
                while gc
                    .nodes
                    .first()
                    .is_some_and(|n| n.node_type == NodeType::OffCurve)
                {
                    gc.nodes.remove(0);
                }
                while gc
                    .nodes
                    .last()
                    .is_some_and(|n| n.node_type == NodeType::OffCurve)
                {
                    gc.nodes.pop();
                }
            }
        }
        self.next_node_id = next_id;
    }

    pub(crate) fn load_slot_glyph_into_active(&mut self, slot_index: usize) {
        let Some(slot) = self.text_stream.slots.get(slot_index) else {
            return;
        };
        let glyph = slot.glyph.clone();
        self.viewport.upm = glyph.upm.max(1);
        self.contours = glyph.contours.iter().map(|gc| gc.nodes.clone()).collect();
        self.active_contour_closed = glyph.contours.last().is_some_and(|gc| gc.closed);
        for a in &glyph.annotations {
            self.v040.next_annotation_id = self.v040.next_annotation_id.max(a.id.0 + 1);
        }
        self.v040.selected_annotations.clear();
        self.background_contours = glyph.background_contours.clone();
        self.anchors = glyph.anchors.clone();
        self.glyph = glyph;
        self.selected_nodes.clear();
        self.select_state = SelectToolState::Idle;
        self.pen_state = PenToolState::Idle;
        self.v020_drag = V020DragState::Idle;
        self.guide_drag = GuideDragState::Idle;
        self.v040.reset_in_flight_drags();
        self.active_snappers.clear();
        self.active_drag_before_snapshot = None;
        self.pending_pen_bcp_out = None;
        self.pending_pen_first_bcp_in = None;
        self.hovered_pen_close_target = None;
        self.hovered_pen_close_node = None;
        self.hovered_segment_hit = None;
        self.pending_segment_click = None;
        self.alt_marquee_pending_start = None;
        self.alt_duplicate_pending = false;
        self.active_open_contour = (0..self.contours.len())
            .rev()
            .find(|&i| !self.is_contour_closed(i));
        let suppress = self.suppress_text_stream_sync;
        self.suppress_text_stream_sync = true;
        self.sync_paths_from_contours();
        self.suppress_text_stream_sync = suppress;
    }

    /// Synchronizes the active editable `self.glyph` and `self.contours` back into `self.text_stream.slots`,
    /// propagating edits to any repeated occurrences of the same `glyph.name` and recomputing origins.
    pub fn sync_active_glyph_to_text_stream(&mut self) {
        if self.suppress_text_stream_sync || self.text_stream.slots.is_empty() {
            return;
        }
        let active_idx = self
            .text_stream
            .active_slot_index
            .min(self.text_stream.slots.len() - 1);
        self.text_stream.active_slot_index = active_idx;
        let updated_glyph = self.export_glyph();
        let active_name = updated_glyph.name.clone();
        self.text_stream.slots[active_idx].glyph = updated_glyph.clone();
        if !active_name.is_empty() {
            for (idx, slot) in self.text_stream.slots.iter_mut().enumerate() {
                if idx != active_idx && slot.glyph.name == active_name {
                    slot.glyph = updated_glyph.clone();
                }
            }
        }
        self.text_stream.recompute_origins();
    }

    /// Replaces the multi-glyph horizontal text strip (`CanvasTextStream`) and activates `active_index`.
    pub fn set_text_stream_slots(&mut self, mut slots: Vec<GlyphSlot>, active_index: usize) {
        for slot in &mut slots {
            self.normalize_slot_glyph(&mut slot.glyph);
        }
        self.text_stream.slots = slots;
        self.text_stream.selection_range = None;
        self.text_stream.pending_spacing_deltas.clear();
        self.text_stream.pending_kerning_deltas.clear();
        if !self.text_stream.slots.is_empty() {
            let idx = active_index.min(self.text_stream.slots.len() - 1);
            self.text_stream.active_slot_index = idx;
            self.text_stream.caret_index = idx;
            self.text_stream.recompute_origins();
            self.load_slot_glyph_into_active(idx);
            self.history = CommandHistory::new();
        } else {
            self.text_stream.active_slot_index = 0;
            self.text_stream.caret_index = 0;
        }
        self.bump_content_generation();
        self.assert_invariants();
    }

    /// Replaces the multi-glyph horizontal text strip (`CanvasTextStream`), sets `active_index` and `caret_index`,
    /// and records an atomic `"Edit Text Stream"` Undo snapshot (`SPEC-006 §2.2`).
    pub fn set_text_stream_slots_with_caret(
        &mut self,
        mut slots: Vec<GlyphSlot>,
        active_index: usize,
        caret_index: usize,
    ) {
        self.sync_active_glyph_to_text_stream();
        let before = self.snapshot();
        for slot in &mut slots {
            self.normalize_slot_glyph(&mut slot.glyph);
        }
        self.text_stream.slots = slots;
        self.text_stream.selection_range = None;
        if !self.text_stream.slots.is_empty() {
            let n = self.text_stream.slots.len();
            let idx = active_index.min(n - 1);
            let caret = caret_index.min(n);
            self.text_stream.active_slot_index = idx;
            self.text_stream.caret_index = caret;
            self.text_stream.recompute_origins();
            self.load_slot_glyph_into_active(idx);
        } else {
            self.text_stream.active_slot_index = 0;
            self.text_stream.caret_index = 0;
        }
        self.record_mutation("Edit Text Stream", before);
        self.assert_invariants();
    }

    /// Alias for `set_caret_index` (`SPEC-006 §2.2`).
    pub fn set_text_caret_index(&mut self, caret_index: usize) {
        self.set_caret_index(caret_index);
    }

    /// Switches the active editable glyph slot in `text_stream` to `slot_index`.
    pub fn switch_active_slot(&mut self, slot_index: usize) {
        if self.text_stream.slots.is_empty() {
            return;
        }
        self.sync_active_glyph_to_text_stream();
        let target = slot_index.min(self.text_stream.slots.len() - 1);
        self.text_stream.active_slot_index = target;
        self.text_stream.caret_index = target;
        self.load_slot_glyph_into_active(target);
        self.assert_invariants();
    }

    /// Steps the active editable glyph slot by `delta` (`-1` for `Home`, `+1` for `End`).
    pub fn step_active_slot(&mut self, delta: isize) -> bool {
        if self.text_stream.slots.is_empty() {
            return false;
        }
        let max_idx = (self.text_stream.slots.len() - 1) as isize;
        let cur = self.text_stream.active_slot_index as isize;
        let next = (cur + delta).clamp(0, max_idx) as usize;
        self.switch_active_slot(next);
        self.text_stream.caret_index = next;
        self.text_stream.selection_range = None;
        self.assert_invariants();
        true
    }

    /// Places the text caret at `caret_index` (`0..=slots.len()`) and activates the glyph to its right.
    pub fn set_caret_index(&mut self, caret_index: usize) {
        let n = self.text_stream.slots.len();
        let clamped = caret_index.min(n);
        self.text_stream.caret_index = clamped;
        self.text_stream.selection_range = None;
        if n > 0 {
            let active = clamped.min(n - 1);
            self.switch_active_slot(active);
            self.text_stream.caret_index = clamped;
        }
        self.assert_invariants();
    }

    /// Resolves and places the text caret at `pt_upm`, extending `selection_range` when `extend_selection` is true.
    pub fn place_text_caret_at_upm(&mut self, pt_upm: FontPoint, extend_selection: bool) {
        let resolved = self.text_stream.resolve_caret_index_at_upm(pt_upm);
        let n = self.text_stream.slots.len();
        if extend_selection {
            let anchor = self.text_stream.caret_index.min(n);
            let start = anchor.min(resolved);
            let end = anchor.max(resolved);
            let range = if start < end {
                Some((start, end))
            } else {
                None
            };
            if n > 0 {
                let active = resolved.min(n - 1);
                self.switch_active_slot(active);
            }
            self.text_stream.caret_index = resolved;
            self.text_stream.selection_range = range;
            self.assert_invariants();
        } else {
            self.set_caret_index(resolved);
        }
    }

    /// Switches to the slot double-clicked at `pt_upm` and activates `CanvasTool::Select` (`SPEC-006 §2.1.2`).
    pub fn handle_double_click_upm(&mut self, pt_upm: FontPoint) -> bool {
        let Some(slot_idx) = self.text_stream.resolve_slot_at_upm(pt_upm) else {
            return false;
        };
        self.switch_active_slot(slot_idx);
        self.text_stream.caret_index = slot_idx;
        self.text_stream.selection_range = None;
        self.activate_tool(CanvasTool::Select);
        self.assert_invariants();
        true
    }

    /// Returns the active glyph's Left Sidebearing (`LSB`) in UPM.
    #[must_use]
    pub fn active_glyph_lsb(&self) -> f64 {
        compute_glyph_lsb_and_rsb(&self.contours, self.glyph.width).0
    }

    /// Returns the active glyph's Right Sidebearing (`RSB`) in UPM.
    #[must_use]
    pub fn active_glyph_rsb(&self) -> f64 {
        compute_glyph_lsb_and_rsb(&self.contours, self.glyph.width).1
    }

    /// Drains all pending `SpacingDelta` events emitted for the Host Font Model Backend.
    pub fn drain_spacing_deltas(&mut self) -> Vec<SpacingDelta> {
        std::mem::take(&mut self.text_stream.pending_spacing_deltas)
    }

    /// Drains all pending `KerningDelta` events emitted for the Host Font Model Backend.
    pub fn drain_kerning_deltas(&mut self) -> Vec<KerningDelta> {
        std::mem::take(&mut self.text_stream.pending_kerning_deltas)
    }

    /// Computes all kerning baseline highlight quads for adjacent kerned slots in `text_stream`.
    #[must_use]
    pub fn compute_kerning_indicator_quads(&self) -> Vec<KerningIndicatorQuad> {
        self.text_stream.compute_kerning_indicator_quads(
            self.metrics.descender,
            self.metrics.ascender,
            self.metrics.x_height,
            self.italic_angle_deg,
        )
    }

    /// Computes horizontal scanline intersections and sidebearing distances at `text_measurement_y_upm`.
    #[must_use]
    pub fn compute_text_scanline_measurements(&self) -> Vec<SlotScanlineMeasurement> {
        self.text_stream.compute_scanline_measurements(
            self.text_measurement_y_upm,
            self.metrics.x_height,
            self.italic_angle_deg,
        )
    }

    /// Updates the vertical position (`y_upm`) of the text-mode measurement scanline.
    pub fn set_text_measurement_y(&mut self, y_upm: f64) {
        if y_upm.is_finite() {
            self.text_measurement_y_upm = y_upm;
        }
        self.assert_invariants();
    }

    /// Returns `true` when `upm * zoom > text_mode_numbers_threshold` (`SPEC-006 §2.5.1`).
    #[must_use]
    pub fn should_draw_text_mode_numbers(&self) -> bool {
        f64::from(self.viewport.upm) * f64::from(self.viewport.zoom)
            > self.text_mode_numbers_threshold
    }

    fn record_spacing_delta(
        &mut self,
        lsb: i32,
        rsb: i32,
        width: i32,
        before: crate::interaction::GlyphSnapshot,
    ) -> bool {
        self.sync_paths_from_contours();
        self.text_stream.pending_spacing_deltas.push(SpacingDelta {
            slot_index: self.text_stream.active_slot_index,
            glyph_name: self.glyph.name.clone(),
            lsb_delta_upm: lsb,
            rsb_delta_upm: rsb,
            width_delta_upm: width,
        });
        self.record_mutation("Change Metrics", before);
        self.assert_invariants();
        true
    }

    /// Adjusts the active glyph's Left Sidebearing (`LSB`) by `delta_upm` (`Ctrl + Left/Right`).
    pub fn adjust_active_lsb(&mut self, delta_upm: i32) -> bool {
        let before = self.snapshot();
        let dx = f64::from(delta_upm);
        self.shift_all_glyph_elements_x(dx);
        self.glyph.width = (self.glyph.width + dx).max(0.0);
        self.record_spacing_delta(delta_upm, 0, delta_upm, before)
    }

    /// Adjusts the active glyph's Right Sidebearing (`RSB`) by `delta_upm` (`Cmd + Left/Right`).
    pub fn adjust_active_rsb(&mut self, delta_upm: i32) -> bool {
        let before = self.snapshot();
        self.glyph.width = (self.glyph.width + f64::from(delta_upm)).max(0.0);
        self.record_spacing_delta(0, delta_upm, delta_upm, before)
    }

    /// Shifts all contours horizontally by `delta_upm` within a constant `advance_width` (`Cmd + Ctrl + Left/Right`).
    pub fn shift_active_contours_in_width(&mut self, delta_upm: i32) -> bool {
        let before = self.snapshot();
        self.shift_all_glyph_elements_x(f64::from(delta_upm));
        self.record_spacing_delta(delta_upm, -delta_upm, 0, before)
    }

    fn adjust_kerning_at_slot(&mut self, right_slot_idx: usize, delta_upm: i32) -> bool {
        if right_slot_idx == 0 || right_slot_idx >= self.text_stream.slots.len() {
            return false;
        }
        let before = self.snapshot();
        self.text_stream.slots[right_slot_idx].kern_with_prev += delta_upm;
        let new_kern = self.text_stream.slots[right_slot_idx].kern_with_prev;
        let left_glyph = self.text_stream.slots[right_slot_idx - 1]
            .glyph
            .name
            .clone();
        let right_glyph = self.text_stream.slots[right_slot_idx].glyph.name.clone();
        self.text_stream.recompute_origins();
        self.text_stream.pending_kerning_deltas.push(KerningDelta {
            left_slot_index: right_slot_idx - 1,
            right_slot_index: right_slot_idx,
            left_glyph,
            right_glyph,
            delta_upm,
            new_kern_upm: new_kern,
        });
        self.record_mutation("Change Metrics", before);
        self.assert_invariants();
        true
    }

    /// Adjusts the left kerning pair `(prev_glyph, active_glyph)` by `delta_upm` (`Ctrl + Opt + Left/Right`).
    pub fn adjust_left_kerning(&mut self, delta_upm: i32) -> bool {
        self.adjust_kerning_at_slot(self.text_stream.active_slot_index, delta_upm)
    }

    /// Adjusts the right kerning pair `(active_glyph, next_glyph)` by `delta_upm` (`Cmd + Opt + Left/Right`).
    pub fn adjust_right_kerning(&mut self, delta_upm: i32) -> bool {
        self.adjust_kerning_at_slot(self.text_stream.active_slot_index + 1, delta_upm)
    }

    fn emit_text_edit_request(&mut self, edit: TextStreamEditKind) -> bool {
        self.host_requests
            .push(CanvasHostRequest::EditTextStream(TextStreamEditRequest {
                caret_index: self.text_stream.caret_index,
                selection_range: self.text_stream.selection_range,
                edit,
            }));
        self.assert_invariants();
        true
    }

    /// Emits `CanvasHostRequest::EditTextStream` with `TextStreamEditKind::Backspace` (`SPEC-006 §2.2`).
    pub fn delete_text_backward_at_caret(&mut self) -> bool {
        self.emit_text_edit_request(TextStreamEditKind::Backspace)
    }

    /// Emits `CanvasHostRequest::EditTextStream` with `TextStreamEditKind::DeleteForward` (`SPEC-006 §2.2`).
    pub fn delete_text_forward_at_caret(&mut self) -> bool {
        self.emit_text_edit_request(TextStreamEditKind::DeleteForward)
    }

    /// Emits `CanvasHostRequest::EditTextStream` for `Backspace` (`backward == true`) or `Delete` (`backward == false`).
    pub fn delete_text_stream_slots(&mut self, backward: bool) -> bool {
        if backward {
            self.delete_text_backward_at_caret()
        } else {
            self.delete_text_forward_at_caret()
        }
    }
}
