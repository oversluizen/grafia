//! `SPEC-007` (`grafia v0.6.0`) 5-Mode Contextual `SelectionInfo` HUD inspection and mutators
//! (`GlyphBase`, `SingleOnCurve`, `SingleOffCurve`, `MultiNode`, `Guide`).

use crate::adapters::state::GrafiaCanvasState;
use crate::core::{
    collect_adjacent_offcurve_positions, compute_glyph_lsb_and_rsb,
    compute_radially_scaled_handle_pos, compute_selection_bounds, find_first_offcurve_node_id,
    find_first_oncurve_with_adjacent_offcurve, resolve_tethered_oncurve_for_offcurve,
    translate_oncurve_with_adjacent_handles, update_offcurve_with_smooth_constraint, FontPoint,
    GlyphBaseInfo, GuideInfo, MultiNodeInfo, NodeId, NodeType, SelectionInfo, SingleOffCurveInfo,
    SingleOnCurveInfo, SpacingDelta,
};

impl GrafiaCanvasState {
    /// Computes the current 5-mode contextual `SelectionInfo` (`SPEC-007 §2.2`).
    #[must_use]
    pub fn selection_info(&self) -> SelectionInfo {
        if self.selected_nodes.is_empty() {
            if let Some(gref) = self.selected_guide {
                let g_opt = if gref.is_global {
                    self.global_guides.iter().find(|g| g.id == gref.id)
                } else {
                    self.guides.iter().find(|g| g.id == gref.id)
                };
                if let Some(g) = g_opt {
                    return SelectionInfo::Guide(GuideInfo {
                        guide_ref: gref,
                        x: g.position.x.0,
                        y: g.position.y.0,
                        angle_deg: g.angle_deg,
                        alignment: g.alignment,
                        lock_angle: g.lock_angle,
                        show_measurement: g.show_measurement,
                    });
                }
            }
            return SelectionInfo::GlyphBase(self.build_glyph_base_info());
        }

        if self.selected_nodes.len() == 1 {
            let sel_id = self.selected_nodes[0];
            let closed_flags = self.closed_flags();
            if let Some(node) = self.find_node(sel_id) {
                if node.node_type != NodeType::OffCurve {
                    return SelectionInfo::SingleOnCurve(SingleOnCurveInfo {
                        node_id: node.id,
                        x: node.point.x.0,
                        y: node.point.y.0,
                        node_type: node.node_type,
                    });
                }
                if let Some((off_n, on_n)) =
                    resolve_tethered_oncurve_for_offcurve(&self.contours, &closed_flags, sel_id)
                {
                    let dx = off_n.point.x.0 - on_n.point.x.0;
                    let dy = off_n.point.y.0 - on_n.point.y.0;
                    return SelectionInfo::SingleOffCurve(SingleOffCurveInfo {
                        node_id: off_n.id,
                        anchor_node_id: on_n.id,
                        tethered_oncurve_id: on_n.id,
                        x: off_n.point.x.0,
                        y: off_n.point.y.0,
                        delta_x: dx,
                        delta_y: dy,
                        length: dx.hypot(dy),
                    });
                }
            }
        }

        let total_layer_node_count = self.contours.iter().map(Vec::len).sum();
        if let Some(bounds) = compute_selection_bounds(
            &self.contours,
            &self.selected_nodes,
            0.0,
            self.metrics.x_height * 0.5,
        ) {
            let origin = bounds.aligned_point(self.transform_grid_anchor.index());
            return SelectionInfo::MultiNode(MultiNodeInfo {
                origin_anchor: self.transform_grid_anchor,
                origin_x: origin.x.0,
                origin_y: origin.y.0,
                width: bounds.width(),
                height: bounds.height(),
                proportional_scale: self.multi_selection_proportional,
                selected_node_count: self.selected_nodes.len(),
                total_layer_node_count,
            });
        }

        SelectionInfo::GlyphBase(self.build_glyph_base_info())
    }

    fn build_glyph_base_info(&self) -> GlyphBaseInfo {
        let (lsb, rsb) = compute_glyph_lsb_and_rsb(&self.contours, self.glyph.width);
        let mut min_x = f64::INFINITY;
        let mut min_y = f64::INFINITY;
        let mut max_x = f64::NEG_INFINITY;
        let mut max_y = f64::NEG_INFINITY;
        for c in &self.contours {
            for n in c {
                min_x = min_x.min(n.point.x.0);
                min_y = min_y.min(n.point.y.0);
                max_x = max_x.max(n.point.x.0);
                max_y = max_y.max(n.point.y.0);
            }
        }
        let (bbox_x, bbox_y, bbox_w, bbox_h) = if min_x.is_finite() && min_y.is_finite() {
            (
                min_x,
                min_y,
                (max_x - min_x).max(0.0),
                (max_y - min_y).max(0.0),
            )
        } else {
            (0.0, 0.0, 0.0, 0.0)
        };
        let i = self.text_stream.active_slot_index;
        let has_slots = !self.text_stream.slots.is_empty();
        let kern_with_prev = if has_slots && i > 0 {
            self.text_stream.slots.get(i).map(|s| s.kern_with_prev)
        } else {
            None
        };
        let kern_with_next = if has_slots {
            self.text_stream.slots.get(i + 1).map(|s| s.kern_with_prev)
        } else {
            None
        };
        GlyphBaseInfo {
            glyph_name: self.glyph.name.clone(),
            bbox_x,
            bbox_y,
            bbox_w,
            bbox_h,
            lsb,
            rsb,
            advance_width: self.glyph.width,
            active_slot_kerning_left: kern_with_prev,
            kern_with_prev,
            kern_with_next,
            transform_anchor: self.transform_grid_anchor,
            proportional_scale: self.multi_selection_proportional,
            contour_count: self.contours.len(),
            total_node_count: self.contours.iter().map(Vec::len).sum(),
        }
    }

    pub(crate) fn shift_all_glyph_elements_x(&mut self, dx: f64) {
        for contour in &mut self.contours {
            for node in contour {
                node.point.x.0 += dx;
            }
        }
        for guide in &mut self.guides {
            guide.position.x.0 += dx;
        }
        for ann in &mut self.glyph.annotations {
            ann.position.x.0 += dx;
        }
        for anchor in &mut self.anchors {
            anchor.position.x.0 += dx;
        }
    }

    /// Sets the active glyph's Left Sidebearing (`LSB`) in UPM while preserving `RSB` (`SPEC-007 §2.2.1`).
    pub fn set_lsb(&mut self, new_lsb: f64) {
        if !new_lsb.is_finite() {
            return;
        }
        let before = self.snapshot();
        let (cur_lsb, _) = compute_glyph_lsb_and_rsb(&self.contours, self.glyph.width);
        let dx = new_lsb - cur_lsb;
        let old_w = self.glyph.width;
        self.shift_all_glyph_elements_x(dx);
        self.glyph.width = (self.glyph.width + dx).max(0.0);
        self.sync_paths_from_contours();
        let d_lsb = dx.round() as i32;
        let d_w = (self.glyph.width - old_w).round() as i32;
        self.text_stream.pending_spacing_deltas.push(SpacingDelta {
            slot_index: self.text_stream.active_slot_index,
            glyph_name: self.glyph.name.clone(),
            lsb_delta_upm: d_lsb,
            rsb_delta_upm: 0,
            width_delta_upm: d_w,
        });
        self.record_mutation("Set LSB", before);
        self.assert_invariants();
    }

    /// Sets the active glyph's Right Sidebearing (`RSB`) in UPM while preserving `LSB` (`SPEC-007 §2.2.1`).
    pub fn set_rsb(&mut self, new_rsb: f64) {
        if !new_rsb.is_finite() {
            return;
        }
        let before = self.snapshot();
        let (_, cur_rsb) = compute_glyph_lsb_and_rsb(&self.contours, self.glyph.width);
        let dx = new_rsb - cur_rsb;
        let old_w = self.glyph.width;
        self.glyph.width = (self.glyph.width + dx).max(0.0);
        self.sync_paths_from_contours();
        let d_rsb = dx.round() as i32;
        let d_w = (self.glyph.width - old_w).round() as i32;
        self.text_stream.pending_spacing_deltas.push(SpacingDelta {
            slot_index: self.text_stream.active_slot_index,
            glyph_name: self.glyph.name.clone(),
            lsb_delta_upm: 0,
            rsb_delta_upm: d_rsb,
            width_delta_upm: d_w,
        });
        self.record_mutation("Set RSB", before);
        self.assert_invariants();
    }

    /// Sets the active glyph's `advance_width` in UPM (`SPEC-007 §2.2.1`).
    pub fn set_advance_width(&mut self, new_width: f64) {
        if !new_width.is_finite() {
            return;
        }
        let before = self.snapshot();
        let old_w = self.glyph.width;
        self.glyph.width = new_width.max(0.0);
        self.sync_paths_from_contours();
        let d_w = (self.glyph.width - old_w).round() as i32;
        self.text_stream.pending_spacing_deltas.push(SpacingDelta {
            slot_index: self.text_stream.active_slot_index,
            glyph_name: self.glyph.name.clone(),
            lsb_delta_upm: 0,
            rsb_delta_upm: d_w,
            width_delta_upm: d_w,
        });
        self.record_mutation("Set Advance Width", before);
        self.assert_invariants();
    }

    /// Centers all contours horizontally within the current `advance_width` so `LSB == RSB` (`SPEC-007 §2.2.1`).
    pub fn center_glyph_in_width(&mut self) {
        let before = self.snapshot();
        let (lsb, rsb) = compute_glyph_lsb_and_rsb(&self.contours, self.glyph.width);
        let dx = (rsb - lsb) * 0.5;
        self.shift_all_glyph_elements_x(dx);
        self.center_in_width_count = self.center_in_width_count.wrapping_add(1);
        self.sync_paths_from_contours();
        let d_lsb = dx.round() as i32;
        self.text_stream.pending_spacing_deltas.push(SpacingDelta {
            slot_index: self.text_stream.active_slot_index,
            glyph_name: self.glyph.name.clone(),
            lsb_delta_upm: d_lsb,
            rsb_delta_upm: -d_lsb,
            width_delta_upm: 0,
        });
        self.record_mutation("Center in Width", before);
        self.assert_invariants();
    }

    /// Returns the first `OnCurve` node that has at least one adjacent `OffCurve` handle.
    #[must_use]
    pub fn first_oncurve_with_adjacent_offcurve(&self) -> Option<NodeId> {
        find_first_oncurve_with_adjacent_offcurve(&self.contours, &self.closed_flags())
    }

    /// Returns the positions of all `OffCurve` handles directly adjacent to `oncurve_id`.
    #[must_use]
    pub fn adjacent_offcurve_positions(&self, oncurve_id: NodeId) -> Vec<FontPoint> {
        collect_adjacent_offcurve_positions(&self.contours, &self.closed_flags(), oncurve_id)
    }

    /// Selects exclusively `node_id` (clearing any guide selection).
    pub fn select_only_node(&mut self, node_id: NodeId) {
        self.selected_guide = None;
        if self.find_node(node_id).is_some() {
            self.selected_nodes = vec![node_id];
        } else {
            self.selected_nodes.clear();
        }
        self.assert_invariants();
    }

    /// Moves the single selected `OnCurve` node to `new_pos`, translating its adjacent `OffCurve` handles by the same delta.
    pub fn set_selected_oncurve_position(&mut self, new_pos: FontPoint) {
        if self.selected_nodes.len() != 1 || !new_pos.x.0.is_finite() || !new_pos.y.0.is_finite() {
            return;
        }
        let sel_id = self.selected_nodes[0];
        let before = self.snapshot();
        let closed_flags = self.closed_flags();
        if translate_oncurve_with_adjacent_handles(
            &mut self.contours,
            &closed_flags,
            sel_id,
            new_pos,
        ) {
            self.sync_paths_from_contours();
            self.record_mutation("Move On-Curve Node", before);
            self.assert_invariants();
        }
    }

    /// Returns the first `OffCurve` handle `NodeId` in the active layer.
    #[must_use]
    pub fn first_offcurve_node_id(&self) -> Option<NodeId> {
        find_first_offcurve_node_id(&self.contours)
    }

    /// Moves the single selected `OffCurve` handle to absolute `new_pos`, preserving smooth collinearity.
    pub fn set_selected_offcurve_position(&mut self, new_pos: FontPoint) {
        if self.selected_nodes.len() != 1 || !new_pos.x.0.is_finite() || !new_pos.y.0.is_finite() {
            return;
        }
        let sel_id = self.selected_nodes[0];
        let before = self.snapshot();
        let closed_flags = self.closed_flags();
        if update_offcurve_with_smooth_constraint(
            &mut self.contours,
            &closed_flags,
            sel_id,
            new_pos,
        ) {
            self.sync_paths_from_contours();
            self.record_mutation("Move Off-Curve Handle", before);
            self.assert_invariants();
        }
    }

    /// Sets the relative `(dx, dy)` offset of the selected `OffCurve` handle from its tethered `OnCurve` anchor.
    pub fn set_selected_offcurve_delta(&mut self, dx: f64, dy: f64) {
        if self.selected_nodes.len() != 1 || !dx.is_finite() || !dy.is_finite() {
            return;
        }
        let sel_id = self.selected_nodes[0];
        let closed_flags = self.closed_flags();
        let Some((_, on_n)) =
            resolve_tethered_oncurve_for_offcurve(&self.contours, &closed_flags, sel_id)
        else {
            return;
        };
        self.set_selected_offcurve_position(FontPoint::new(
            on_n.point.x.0 + dx,
            on_n.point.y.0 + dy,
        ));
    }

    /// Radially scales the selected `OffCurve` handle to `new_length` along its current angle from its tethered `OnCurve` anchor.
    pub fn set_selected_handle_length(&mut self, new_length: f64) {
        if self.selected_nodes.len() != 1 || !new_length.is_finite() {
            return;
        }
        let sel_id = self.selected_nodes[0];
        let closed_flags = self.closed_flags();
        let Some((off_n, on_n)) =
            resolve_tethered_oncurve_for_offcurve(&self.contours, &closed_flags, sel_id)
        else {
            return;
        };
        let new_pos = compute_radially_scaled_handle_pos(on_n.point, off_n.point, new_length);
        self.set_selected_offcurve_position(new_pos);
    }
}
