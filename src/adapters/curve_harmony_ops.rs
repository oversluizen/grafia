//! `SPEC-011` (`grafia v0.10.0`) adapter operations on `GrafiaCanvasState`:
//! $G^2 / G^3$ Curve Harmonization (`Ctrl+Shift+H`), Tangent Balance (Tunni) Lines,
//! Proportional Handle Servo, Interpolated Neighbour Nudge (`C`), Master Kinks/Crossings & Auto-Matching,
//! Proactive Vector Outline Audit, Handle Warnings/Ratios, Diagonal Apex Snaps, and Curvature Comb.

use crate::adapters::state::GrafiaCanvasState;
use crate::core::{
    audit_contours_outlines, auto_match_contours_to_companion, balance_tunni_in_contour,
    classify_node_continuity_in_contours, compute_angled_handle_warnings_in_contours,
    compute_curvature_combs_for_contours, compute_master_kinks_and_crossings_for_layers,
    compute_segment_handle_ratios_in_contours, compute_tangent_extension_snaps_in_contours,
    compute_tunni_overlays_for_contours, drag_tunni_in_contour,
    harmonize_smooth_join_g3_in_contour, harmonize_smooth_join_in_contour, power_nudge_contours,
    scale_tunni_in_contour, servo_translate_node_in_contours, AngledHandleWarning, ContinuityGrade,
    ContourId, CurvatureCombPoly, FontPoint, MasterCompatibilityMode, MasterKinkAndCrossingReport,
    NodeId, OutlineAuditFinding, SegmentHandleRatioHud, TangentExtensionSnap, TunniSegmentOverlay,
    TUNNI_HIT_RADIUS_DP,
};
use crate::tokens::MIN_VIEWPORT_ZOOM;

fn point_to_segment_distance_upm(pt: FontPoint, a: FontPoint, b: FontPoint) -> f64 {
    let dx = b.x.0 - a.x.0;
    let dy = b.y.0 - a.y.0;
    let len_sq = dx * dx + dy * dy;
    if len_sq <= 0.0 {
        return pt.distance_to(a);
    }
    let t = (((pt.x.0 - a.x.0) * dx + (pt.y.0 - a.y.0) * dy) / len_sq).clamp(0.0, 1.0);
    let proj = FontPoint::new(a.x.0 + t * dx, a.y.0 + t * dy);
    pt.distance_to(proj)
}

impl GrafiaCanvasState {
    /// Registers or unregisters `node_id` as a live $G^2$-constrained (`Genius`) smooth node (`SPEC-011 §2.1.3`).
    pub fn set_node_genius(&mut self, node_id: NodeId, enabled: bool) {
        if enabled {
            self.curve_harmony.genius_nodes.insert(node_id);
        } else {
            self.curve_harmony.genius_nodes.remove(&node_id);
        }
        self.bump_content_generation();
    }

    /// Harmonizes all selected smooth on-curve nodes (`Ctrl+Shift+H`, `SPEC-011 §2.1.2`).
    pub fn harmonize_selected_nodes(&mut self) {
        if self.selected_nodes.is_empty() {
            return;
        }
        let before = self.snapshot();
        let closed_flags = self.closed_flags();
        let selected = self.selected_nodes.clone();
        let mut changed = false;
        for (c_idx, nodes) in self.contours.iter_mut().enumerate() {
            let closed = closed_flags.get(c_idx).copied().unwrap_or(true);
            for p_idx in 0..nodes.len() {
                if selected.contains(&nodes[p_idx].id)
                    && harmonize_smooth_join_in_contour(nodes, closed, p_idx, false)
                {
                    changed = true;
                }
            }
        }
        if changed {
            self.sync_paths_from_contours();
            self.record_mutation("Harmonize Curves", before);
            self.assert_invariants();
        }
    }

    /// Harmonizes handle lengths at smooth node `node_id` while keeping its on-curve anchor fixed (`SPEC-011 §2.1.2`).
    pub fn harmonize_handles_keep_node(&mut self, node_id: NodeId) {
        let before = self.snapshot();
        let closed_flags = self.closed_flags();
        let mut changed = false;
        for (c_idx, nodes) in self.contours.iter_mut().enumerate() {
            let closed = closed_flags.get(c_idx).copied().unwrap_or(true);
            if let Some(p_idx) = nodes.iter().position(|n| n.id == node_id) {
                if harmonize_smooth_join_in_contour(nodes, closed, p_idx, true) {
                    changed = true;
                }
            }
        }
        if changed {
            self.sync_paths_from_contours();
            self.record_mutation("Harmonize Handles", before);
            self.assert_invariants();
        }
    }

    /// Toggles display and interaction of Tangent Balance (Tunni) lines (`SPEC-011 §2.2`).
    pub fn toggle_show_tunni_lines(&mut self) {
        self.show_tunni_lines = !self.show_tunni_lines;
        self.bump_content_generation();
    }

    /// Computes all active `TunniSegmentOverlay` items across the foreground glyph contours (`SPEC-011 §2.2`).
    #[must_use]
    pub fn compute_tunni_overlays(&self) -> Vec<TunniSegmentOverlay> {
        compute_tunni_overlays_for_contours(&self.glyph.contours)
    }

    /// Hit-tests the Tangent Balance (Tunni) lines at `pt` in UPM space within `6.0 dp` (`SPEC-011 §2.2.2`).
    #[must_use]
    pub fn hit_test_tunni_line_at_upm(&self, pt: FontPoint) -> Option<(ContourId, usize)> {
        if !self.show_tunni_lines {
            return None;
        }
        let zoom = self.viewport.zoom.max(MIN_VIEWPORT_ZOOM);
        let tol_upm = f64::from(TUNNI_HIT_RADIUS_DP / zoom);
        let mut best: Option<((ContourId, usize), f64)> = None;
        for ov in self.compute_tunni_overlays() {
            let d = point_to_segment_distance_upm(pt, ov.p1, ov.p2);
            if d <= tol_upm && best.is_none_or(|(_, bd)| d < bd) {
                best = Some(((ov.contour_id, ov.segment_index), d));
            }
        }
        best.map(|(hit, _)| hit)
    }

    /// Scales both off-curve handles of `(contour_id, segment_index)` along their tangent rays to `ratio` clamped in `[0.20, 0.95]` (`SPEC-011 §2.2.2`).
    pub fn scale_segment_tunni_ratio(
        &mut self,
        contour_id: ContourId,
        segment_index: usize,
        ratio: f64,
    ) {
        let before = self.snapshot();
        if let Some(c_idx) = self.glyph.contours.iter().position(|c| c.id == contour_id) {
            if scale_tunni_in_contour(&mut self.glyph.contours[c_idx], segment_index, ratio) {
                self.contours[c_idx] = self.glyph.contours[c_idx].nodes.clone();
                self.sync_paths_from_contours();
                self.record_mutation("Scale Tunni Ratio", before);
                self.assert_invariants();
            }
        }
    }

    /// Drags the Tunni line of `(contour_id, segment_index)` toward `target_upm`, clamping handle ratios in `[0.20, 0.95]` (`SPEC-011 §2.2.2`).
    pub fn drag_tunni_line_to_upm(
        &mut self,
        contour_id: ContourId,
        segment_index: usize,
        target_upm: FontPoint,
    ) {
        let before = self.snapshot();
        if let Some(c_idx) = self.glyph.contours.iter().position(|c| c.id == contour_id) {
            if drag_tunni_in_contour(&mut self.glyph.contours[c_idx], segment_index, target_upm) {
                self.contours[c_idx] = self.glyph.contours[c_idx].nodes.clone();
                self.sync_paths_from_contours();
                self.record_mutation("Drag Tunni Line", before);
                self.assert_invariants();
            }
        }
    }

    /// Equalizes both handle ratios of `(contour_id, segment_index)` to their arithmetic mean so $[P_1, P_2] \parallel [P_0, P_3]$ (`SPEC-011 §2.2.2`).
    pub fn balance_segment_tunni(&mut self, contour_id: ContourId, segment_index: usize) {
        let before = self.snapshot();
        if let Some(c_idx) = self.glyph.contours.iter().position(|c| c.id == contour_id) {
            if balance_tunni_in_contour(&mut self.glyph.contours[c_idx], segment_index) {
                self.contours[c_idx] = self.glyph.contours[c_idx].nodes.clone();
                self.sync_paths_from_contours();
                self.record_mutation("Balance Tunni Line", before);
                self.assert_invariants();
            }
        }
    }

    /// Toggles Interpolated Neighbour Nudge (`power_nudge_enabled`, Shortcut `C`, `SPEC-011 §2.3.2`).
    pub fn toggle_power_nudge(&mut self) {
        self.power_nudge_enabled = !self.power_nudge_enabled;
    }

    /// Translates on-curve node `node_id` by `delta` with Proportional Handle Servo scaling (`SPEC-011 §2.3.1`).
    pub fn servo_translate_on_curve_node(&mut self, node_id: NodeId, delta: FontPoint) {
        if !delta.x.0.is_finite() || !delta.y.0.is_finite() {
            return;
        }
        let before = self.snapshot();
        let closed_flags = self.closed_flags();
        if servo_translate_node_in_contours(&mut self.contours, &closed_flags, node_id, delta) {
            self.sync_paths_from_contours();
            self.record_mutation("Servo Translate Node", before);
            self.assert_invariants();
        }
    }

    /// Translates selected nodes by `delta` and linearly interpolates intermediate unselected contour nodes (`SPEC-011 §2.3.2`).
    pub fn power_nudge_translate_selected(&mut self, delta: FontPoint) {
        if !delta.x.0.is_finite() || !delta.y.0.is_finite() || self.selected_nodes.is_empty() {
            return;
        }
        let before = self.snapshot();
        let closed_flags = self.closed_flags();
        let selected = self.selected_nodes.clone();
        if power_nudge_contours(&mut self.contours, &closed_flags, &selected, delta) {
            self.sync_paths_from_contours();
            self.record_mutation("Power Nudge", before);
            self.assert_invariants();
        }
    }

    /// Sets the `MasterCompatibilityMode` (`OriginAndAnchorRays` vs. `FullNodeTrajectories`, `SPEC-011 §2.4.1`).
    pub fn set_master_compatibility_mode(&mut self, mode: MasterCompatibilityMode) {
        self.curve_harmony.master_compatibility_mode = mode;
        self.bump_content_generation();
    }

    /// Evaluates full cross-master node trajectories, trajectory crossings, and smooth-node kinks (`SPEC-011 §2.4`).
    #[must_use]
    pub fn compute_master_kinks_and_crossings(&self) -> MasterKinkAndCrossingReport {
        compute_master_kinks_and_crossings_for_layers(
            &self.glyph.contours,
            &self.companion_master_layers,
        )
    }

    /// Automatically aligns contour order, winding parity, and cyclic `Node 0` start nodes to `companion_layer_id` (`SPEC-011 §2.4.4`).
    pub fn auto_match_master_compatibility(&mut self, companion_layer_id: u32) -> bool {
        let Some(comp) = self
            .companion_master_layers
            .iter()
            .find(|l| l.id == companion_layer_id)
            .cloned()
        else {
            return false;
        };
        let before = self.snapshot();
        let mut matched = self.glyph.contours.clone();
        let Some(changed) = auto_match_contours_to_companion(&mut matched, &comp) else {
            return false;
        };
        if changed {
            self.contours = matched.iter().map(|c| c.nodes.clone()).collect();
            self.active_contour_closed = matched.last().is_none_or(|c| c.closed);
            self.glyph.contours = matched;
            self.sync_paths_from_contours();
            self.record_mutation("Auto-Match Master Compatibility", before);
            self.assert_invariants();
        }
        true
    }

    /// Audits the active layer's outlines for `MissingExtremum`, `QuasiOrthogonalStem`, `CrossedHandles`, and `CollinearCubic` (`SPEC-011 §2.5`).
    #[must_use]
    pub fn audit_layer_outlines(&self) -> Vec<OutlineAuditFinding> {
        audit_contours_outlines(&self.glyph.contours, self.italic_angle_deg)
    }

    /// Computes warnings for `OffCurve` handles on `Smooth` extrema that deviate slightly from orthogonal/italic axes (`SPEC-011 §2.6.1`).
    #[must_use]
    pub fn compute_angled_handle_warnings(&self) -> Vec<AngledHandleWarning> {
        compute_angled_handle_warnings_in_contours(&self.glyph.contours, self.italic_angle_deg)
    }

    /// Computes handle-length percentages `(ratio_in, ratio_out)` relative to tangent intersection apex `T` (`SPEC-011 §2.6.2`).
    #[must_use]
    pub fn compute_segment_handle_ratios(&self) -> Vec<SegmentHandleRatioHud> {
        compute_segment_handle_ratios_in_contours(&self.glyph.contours)
    }

    /// Computes diagonal leg ray-extension and virtual apex snap candidates within `snap_radius_upm` (`SPEC-011 §2.7`).
    #[must_use]
    pub fn compute_tangent_extension_snaps(
        &self,
        query_pt: FontPoint,
        snap_radius_upm: f64,
    ) -> Option<TangentExtensionSnap> {
        compute_tangent_extension_snaps_in_contours(&self.glyph.contours, query_pt, snap_radius_upm)
    }

    /// Classifies geometric continuity (`G0Corner` .. `G3Clothoid`) at `node_id` (`SPEC-011 §2.8.1`).
    #[must_use]
    pub fn classify_node_continuity(&self, node_id: NodeId) -> ContinuityGrade {
        classify_node_continuity_in_contours(&self.contours, &self.closed_flags(), node_id)
    }

    /// Enables or disables live $G^3$ (`G3Clothoid`) continuity lock on smooth node `node_id` (`SPEC-011 §2.8.2`).
    pub fn set_node_g3_lock(&mut self, node_id: NodeId, enabled: bool) {
        if enabled {
            self.curve_harmony.g3_nodes.insert(node_id);
        } else {
            self.curve_harmony.g3_nodes.remove(&node_id);
        }
        self.bump_content_generation();
    }

    /// Harmonizes selected smooth nodes to $G^3$ (`G3Clothoid`) continuity (`SPEC-011 §2.8.2`).
    pub fn harmonize_selected_nodes_g3(&mut self) {
        if self.selected_nodes.is_empty() {
            return;
        }
        let before = self.snapshot();
        let closed_flags = self.closed_flags();
        let selected = self.selected_nodes.clone();
        let mut changed = false;
        for (c_idx, nodes) in self.contours.iter_mut().enumerate() {
            let closed = closed_flags.get(c_idx).copied().unwrap_or(true);
            for p_idx in 0..nodes.len() {
                if selected.contains(&nodes[p_idx].id)
                    && harmonize_smooth_join_g3_in_contour(nodes, closed, p_idx, None)
                {
                    changed = true;
                }
            }
        }
        if changed {
            self.sync_paths_from_contours();
            self.record_mutation("Harmonize G3 Clothoid", before);
            self.assert_invariants();
        }
    }

    /// Toggles the live 120 fps `CurvatureComb` (`Krommingskam`) overlay (`SPEC-011 §2.9`).
    pub fn toggle_show_curvature_comb(&mut self) {
        self.curve_harmony.show_curvature_comb = !self.curve_harmony.show_curvature_comb;
        self.bump_content_generation();
    }

    /// Explicitly sets `show_curvature_comb` (`SPEC-011 §2.9`).
    pub fn set_show_curvature_comb(&mut self, enabled: bool) {
        self.curve_harmony.show_curvature_comb = enabled;
        self.bump_content_generation();
    }

    /// Evaluates the live `CurvatureCombPoly` overlay for all cubic segments in the active layer (`SPEC-011 §2.9`).
    #[must_use]
    pub fn compute_curvature_comb_overlay(&self) -> Vec<CurvatureCombPoly> {
        let hovered = if let crate::interaction::SelectToolState::DraggingTunniLine {
            contour_id,
            segment_index,
        } = self.select_state
        {
            self.glyph
                .contours
                .iter()
                .position(|c| c.id == contour_id)
                .map(|c_idx| (c_idx, segment_index))
        } else {
            let drag_seg = match self.select_state {
                crate::interaction::SelectToolState::DraggingSegment {
                    contour_idx,
                    segment,
                    ..
                } => Some((contour_idx, segment)),
                _ => None,
            };
            let active_seg = self
                .hovered_segment_hit
                .map(|h| (h.contour_idx, h.segment))
                .or(drag_seg);
            active_seg.and_then(|(c_idx, target_seg)| {
                let contour = self.glyph.contours.get(c_idx)?;
                let seg_idx = crate::core::extract_segments(&contour.nodes, contour.closed)
                    .iter()
                    .position(|s| s == &target_seg)?;
                Some((c_idx, seg_idx))
            })
        };
        compute_curvature_combs_for_contours(&self.glyph.contours, &self.selected_nodes, hovered)
    }

    pub(crate) fn try_apply_harmony_arrow_nudge(
        &mut self,
        dx: f64,
        dy: f64,
        ctrl_held: bool,
        alt_held: bool,
    ) -> bool {
        if ctrl_held || alt_held {
            return false;
        }
        if self.power_nudge_enabled {
            self.power_nudge_translate_selected(FontPoint::new(dx, dy));
            return true;
        }
        if self.curve_harmony.servo_scaling_enabled && self.selected_nodes.len() == 1 {
            let sel_id = self.selected_nodes[0];
            if self
                .find_node(sel_id)
                .is_some_and(|n| n.node_type != crate::core::NodeType::OffCurve)
            {
                self.servo_translate_on_curve_node(sel_id, FontPoint::new(dx, dy));
                return true;
            }
        }
        false
    }

    pub(crate) fn try_begin_tunni_press(&mut self, font_pt: FontPoint, is_double: bool) -> bool {
        if !self.show_tunni_lines {
            return false;
        }
        let zoom = self.viewport.zoom.max(MIN_VIEWPORT_ZOOM);
        let tol_upm = f64::from(TUNNI_HIT_RADIUS_DP / zoom);
        let mut best: Option<((ContourId, usize), f64)> = None;
        for ov in self.compute_tunni_overlays() {
            let d = point_to_segment_distance_upm(font_pt, ov.p1, ov.p2);
            if d <= tol_upm && best.is_none_or(|(_, bd)| d < bd) {
                best = Some(((ov.contour_id, ov.segment_index), d));
            }
        }
        let Some(((contour_id, segment_index), tunni_d)) = best else {
            return false;
        };
        let min_node_d = self
            .contours
            .iter()
            .flatten()
            .map(|n| n.point.distance_to(font_pt))
            .fold(f64::INFINITY, f64::min);
        if min_node_d <= tunni_d {
            return false;
        }
        if is_double {
            self.balance_segment_tunni(contour_id, segment_index);
            self.select_state = crate::interaction::SelectToolState::Idle;
            self.drag_transients.active_drag_before_snapshot = None;
        } else {
            self.record_drag_start();
            self.select_state = crate::interaction::SelectToolState::DraggingTunniLine {
                contour_id,
                segment_index,
            };
        }
        true
    }

    pub(crate) fn update_active_tunni_drag(
        &mut self,
        contour_id: ContourId,
        segment_index: usize,
        font_pt: FontPoint,
    ) {
        if let Some(c_idx) = self.glyph.contours.iter().position(|c| c.id == contour_id) {
            if let Some(start_nodes) = self.drag_transients.drag_start_contours.get(c_idx) {
                self.glyph.contours[c_idx].nodes = start_nodes.clone();
            }
            if drag_tunni_in_contour(&mut self.glyph.contours[c_idx], segment_index, font_pt) {
                self.contours[c_idx] = self.glyph.contours[c_idx].nodes.clone();
                self.sync_paths_from_contours();
            }
        }
    }
}
