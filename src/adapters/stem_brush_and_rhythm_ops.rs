//! `SPEC-013` (`grafia v0.12.0`) adapter operations on `GrafiaCanvasState`:
//! Interactive Structural `Stem` Tool & Measurement Rainbow,
//! Non-Linear Curved Master Trajectories (`CurvedTrajectoryLink`),
//! Local `Trowel` & `Cleaner` Brushes,
//! `QuickPreviewOnDrag` & Subtle Metric Overshoot Bands,
//! and Scanline Optical Inter-Shape Rhythm Integrals & Clean 1-Axis Live Mirror.

use crate::adapters::state::GrafiaCanvasState;
use crate::core::{
    adjust_structural_stem_in_contours, apply_cleaner_brush_in_contours,
    apply_trowel_brush_in_contours, detect_structural_stems_in_contours,
    evaluate_curved_master_contour_nodes, reflect_contour_across_mirror_axis, ActiveMirrorAxis,
    CanvasHostRequest, ContourId, CurvedTrajectoryLink, DetectedStem, FontPoint, GlyphContour,
    Node, NodeId, StemAdjustmentMode,
};

impl GrafiaCanvasState {
    pub(crate) fn next_safe_node_id_seed_ext(&self) -> u64 {
        let max_existing = self
            .contours
            .iter()
            .flatten()
            .map(|n| n.id.0 + 1)
            .max()
            .unwrap_or(1);
        self.next_node_id.max(max_existing)
    }

    /// Detects paired opposing structural stems across the active layer (`SPEC-013 §2.1.1`).
    #[must_use]
    pub fn detect_structural_stems(&self) -> Vec<DetectedStem> {
        detect_structural_stems_in_contours(&self.glyph.contours)
    }

    /// Adjusts a detected structural stem's perpendicular separation and angle while servo-scaling connected handles (`SPEC-013 §2.1.2`).
    pub fn adjust_structural_stem(
        &mut self,
        stem_id: usize,
        width_delta_upm: f64,
        mode: StemAdjustmentMode,
        angle_delta_deg: f64,
        propagate_across_glyphs: bool,
    ) -> bool {
        let stems = self.detect_structural_stems();
        let Some(stem) = stems.iter().find(|s| s.id == stem_id).cloned() else {
            return false;
        };
        let before = self.snapshot();
        let closed_flags = self.closed_flags();
        let Some((old_width_upm, new_width_upm)) = adjust_structural_stem_in_contours(
            &mut self.contours,
            &closed_flags,
            &stem,
            width_delta_upm,
            mode,
            angle_delta_deg,
        ) else {
            return false;
        };
        self.sync_paths_from_contours();
        if propagate_across_glyphs {
            self.drag_transients
                .host_requests
                .push(CanvasHostRequest::PropagateStemDelta {
                    orientation: stem.orientation,
                    old_width_upm,
                    new_width_upm,
                    angle_delta_deg,
                });
        }
        self.record_mutation("Adjust Stem", before);
        self.assert_invariants();
        true
    }

    /// Enables or disables the Pass-2 interior Measurement Rainbow heatmap (`SPEC-013 §2.1.3`).
    pub fn set_show_measurement_rainbow(&mut self, enabled: bool) {
        self.curve_harmony.show_measurement_rainbow = enabled;
        self.bump_content_generation();
    }

    /// Toggles the Pass-2 interior Measurement Rainbow heatmap (`SPEC-013 §2.1.3`).
    pub fn toggle_show_measurement_rainbow(&mut self) {
        self.curve_harmony.show_measurement_rainbow = !self.curve_harmony.show_measurement_rainbow;
        self.bump_content_generation();
    }

    /// Registers or updates a non-linear `CurvedTrajectoryLink` inside an atomic Undo entry (`SPEC-013 §2.2`).
    pub fn set_curved_trajectory_link(&mut self, link: CurvedTrajectoryLink) {
        if !link.control1.x.0.is_finite()
            || !link.control1.y.0.is_finite()
            || link
                .control2
                .is_some_and(|c| !c.x.0.is_finite() || !c.y.0.is_finite())
        {
            return;
        }
        let before = self.snapshot();
        if let Some(existing) = self.curve_harmony.curved_trajectories.iter_mut().find(|l| {
            l.node_id == link.node_id && l.companion_layer_index == link.companion_layer_index
        }) {
            *existing = link;
        } else {
            self.curve_harmony.curved_trajectories.push(link);
        }
        self.record_mutation("Edit Master Trajectory", before);
        self.bump_content_generation();
        self.assert_invariants();
    }

    /// Removes a non-linear `CurvedTrajectoryLink` inside an atomic Undo entry (`SPEC-013 §2.2`).
    pub fn remove_curved_trajectory_link(&mut self, node_id: NodeId, companion_layer_index: usize) {
        let before = self.snapshot();
        self.curve_harmony.curved_trajectories.retain(|l| {
            !(l.node_id == node_id && l.companion_layer_index == companion_layer_index)
        });
        self.record_mutation("Edit Master Trajectory", before);
        self.bump_content_generation();
        self.assert_invariants();
    }

    /// Evaluates a contour along active `CurvedTrajectoryLink`s at parameter $t \in [0, 1]$ (`SPEC-013 §2.2`).
    #[must_use]
    pub fn evaluate_curved_master_contour(
        &self,
        contour_idx: usize,
        companion_layer_index: usize,
        t: f64,
    ) -> Option<Vec<Node>> {
        let fg = self.contours.get(contour_idx)?;
        let bg = self
            .companion_master_layers
            .get(companion_layer_index)?
            .contours
            .get(contour_idx)?;
        evaluate_curved_master_contour_nodes(
            fg,
            bg,
            companion_layer_index,
            &self.curve_harmony.curved_trajectories,
            t,
        )
    }

    /// Applies the local `Trowel` smoothing brush inside $\mathcal{B}(c, R_{\text{upm}})$ (`SPEC-013 §2.3.1`).
    pub fn apply_trowel_brush_at_upm(
        &mut self,
        center_upm: FontPoint,
        radius_upm: f64,
        fit_tolerance_upm: f64,
    ) -> bool {
        let before = self.snapshot();
        let closed_flags = self.closed_flags();
        let mut next_id = self.next_safe_node_id_seed_ext();
        let changed = apply_trowel_brush_in_contours(
            &mut self.contours,
            &closed_flags,
            center_upm,
            radius_upm,
            fit_tolerance_upm,
            &mut next_id,
        );
        if !changed {
            return false;
        }
        self.next_node_id = next_id;
        self.selected_nodes
            .retain(|id| self.contours.iter().flatten().any(|n| n.id == *id));
        self.sync_paths_from_contours();
        self.record_mutation("Trowel Smooth", before);
        self.assert_invariants();
        true
    }

    /// Applies the local `Cleaner` micro-loop and duplicate-node healing brush inside $\mathcal{B}(c, R_{\text{upm}})$ (`SPEC-013 §2.3.2`).
    pub fn apply_cleaner_brush_at_upm(
        &mut self,
        center_upm: FontPoint,
        radius_upm: f64,
        max_loop_area_upm2: f64,
    ) -> bool {
        let before = self.snapshot();
        let closed_flags = self.closed_flags();
        let changed = apply_cleaner_brush_in_contours(
            &mut self.contours,
            &closed_flags,
            center_upm,
            radius_upm,
            max_loop_area_upm2,
        );
        if !changed {
            return false;
        }
        self.selected_nodes
            .retain(|id| self.contours.iter().flatten().any(|n| n.id == *id));
        self.sync_paths_from_contours();
        self.record_mutation("Clean Contour", before);
        self.assert_invariants();
        true
    }

    /// Enables or disables `QuickPreviewOnDrag` wireframe suppression during active drag (`SPEC-013 §2.4.1`).
    pub fn set_quick_preview_on_drag(&mut self, enabled: bool) {
        self.curve_harmony.quick_preview_on_drag = enabled;
        self.bump_content_generation();
    }

    /// Toggles `QuickPreviewOnDrag` (`SPEC-013 §2.4.1`).
    pub fn toggle_quick_preview_on_drag(&mut self) {
        self.curve_harmony.quick_preview_on_drag = !self.curve_harmony.quick_preview_on_drag;
        self.bump_content_generation();
    }

    /// Configures the subtle metric overshoot ribbon height in UPM (`SPEC-013 §2.4.2`).
    pub fn set_round_overshoot_upm(&mut self, overshoot_upm: Option<f64>) {
        self.curve_harmony.round_overshoot_upm =
            overshoot_upm.filter(|v| v.is_finite() && *v > 0.0);
        self.bump_content_generation();
    }

    /// Enables or disables the Pass-1 optical rhythm water-volume fill overlay (`SPEC-013 §2.5.1`).
    pub fn set_show_optical_rhythm_fill(&mut self, enabled: bool) {
        self.curve_harmony.show_optical_rhythm_fill = enabled;
        self.bump_content_generation();
    }

    /// Toggles the Pass-1 optical rhythm water-volume fill overlay (`SPEC-013 §2.5.1`).
    pub fn toggle_show_optical_rhythm_fill(&mut self) {
        self.curve_harmony.show_optical_rhythm_fill = !self.curve_harmony.show_optical_rhythm_fill;
        self.bump_content_generation();
    }

    /// Configures the clean 1-axis live mirror mode (`SPEC-013 §2.5.2`).
    pub fn set_active_mirror_axis(&mut self, axis: ActiveMirrorAxis) {
        self.curve_harmony.active_mirror_axis = axis;
        self.bump_content_generation();
    }

    /// Appends `contour` (and its reflection when `active_mirror_axis != ActiveMirrorAxis::None`) in one atomic Undo step (`SPEC-013 §2.5.2`).
    pub fn add_contour_with_active_mirror(&mut self, mut contour: GlyphContour) -> ContourId {
        let before = self.snapshot();
        let mut next_id = self.next_safe_node_id_seed_ext();
        for n in &mut contour.nodes {
            n.id = NodeId(next_id);
            next_id += 1;
        }
        let cid = ContourId(self.glyph.contours.len());
        contour.id = cid;
        let mirrored_opt = reflect_contour_across_mirror_axis(
            &contour,
            self.curve_harmony.active_mirror_axis,
            &mut next_id,
        );
        self.next_node_id = next_id;
        self.contours.push(contour.nodes.clone());
        self.active_contour_closed = contour.closed;
        self.glyph.contours.push(contour);
        if let Some(mut mirrored) = mirrored_opt {
            mirrored.id = ContourId(self.glyph.contours.len());
            self.contours.push(mirrored.nodes.clone());
            self.active_contour_closed = mirrored.closed;
            self.glyph.contours.push(mirrored);
        }
        self.sync_paths_from_contours();
        self.record_mutation("Add Mirrored Contour", before);
        self.assert_invariants();
        cid
    }

    pub(crate) fn try_reflect_contour_at_index(&mut self, c_idx: usize) {
        if let Some(gc) = self.glyph.contours.get(c_idx).cloned() {
            let mut next_id = self.next_safe_node_id_seed_ext();
            if let Some(mut mirrored) = reflect_contour_across_mirror_axis(
                &gc,
                self.curve_harmony.active_mirror_axis,
                &mut next_id,
            ) {
                self.next_node_id = next_id;
                mirrored.id = ContourId(self.glyph.contours.len());
                self.contours.push(mirrored.nodes.clone());
                self.glyph.contours.push(mirrored);
                self.sync_paths_from_contours();
            }
        }
    }

    pub(crate) fn on_stem_brush_or_trajectory_press(
        &mut self,
        scr_pt: crate::core::ScreenPoint,
        font_pt: FontPoint,
        mods: crate::interaction::Modifiers,
    ) -> Option<bool> {
        let zoom = self.viewport.zoom.max(crate::tokens::MIN_VIEWPORT_ZOOM);
        let r_upm = f64::from(self.effective_brush_radius_dp()) / f64::from(zoom);
        match self.active_tool {
            crate::adapters::state::CanvasTool::Trowel => {
                self.drag_transients.active_drag_before_snapshot = Some(self.snapshot());
                self.drawing_tool_state.brush_stroke_active = true;
                let _ = self.step_brush_stroke_in_place(font_pt, r_upm);
                Some(true)
            }
            crate::adapters::state::CanvasTool::Cleaner => {
                self.drag_transients.active_drag_before_snapshot = Some(self.snapshot());
                self.drawing_tool_state.brush_stroke_active = true;
                let _ = self.step_brush_stroke_in_place(font_pt, r_upm);
                Some(true)
            }
            crate::adapters::state::CanvasTool::Stem => {
                let stems = self.detect_structural_stems();
                if let Some(best) = stems.iter().min_by(|a, b| {
                    let ma = FontPoint::new(
                        0.5 * (a.p_left.x.0 + a.p_right.x.0),
                        0.5 * (a.p_left.y.0 + a.p_right.y.0),
                    );
                    let mb = FontPoint::new(
                        0.5 * (b.p_left.x.0 + b.p_right.x.0),
                        0.5 * (b.p_left.y.0 + b.p_right.y.0),
                    );
                    font_pt
                        .distance_to(ma)
                        .partial_cmp(&font_pt.distance_to(mb))
                        .unwrap_or(std::cmp::Ordering::Equal)
                }) {
                    let delta = if mods.option { -2.0 } else { 2.0 };
                    return Some(self.adjust_structural_stem(
                        best.id,
                        delta,
                        StemAdjustmentMode::SymmetricCenter,
                        0.0,
                        mods.shift,
                    ));
                }
                Some(false)
            }
            crate::adapters::state::CanvasTool::CurvedTrajectory => {
                Some(self.on_curved_trajectory_press(scr_pt, font_pt))
            }
            _ => None,
        }
    }

    fn on_curved_trajectory_press(
        &mut self,
        scr_pt: crate::core::ScreenPoint,
        font_pt: FontPoint,
    ) -> bool {
        let vp = self.active_slot_viewport();
        let hit_r = crate::core::math_consts::TRAJECTORY_HANDLE_HIT_RADIUS_DP;
        if let Some((hit_link, is_c2)) =
            self.curve_harmony.curved_trajectories.iter().find_map(|l| {
                if let Some(c2) = l.control2 {
                    if vp.to_screen(c2).distance_to(scr_pt) <= hit_r {
                        return Some((l.clone(), true));
                    }
                }
                if vp.to_screen(l.control1).distance_to(scr_pt) <= hit_r {
                    return Some((l.clone(), false));
                }
                None
            })
        {
            self.drag_transients.active_drag_before_snapshot = Some(self.snapshot());
            self.drawing_tool_state.active_trajectory_drag =
                Some((hit_link.node_id, hit_link.companion_layer_index));
            self.drawing_tool_state.active_trajectory_drag_is_control2 = is_c2;
            return true;
        }
        if let Some(sel_id) = self
            .selected_nodes
            .first()
            .copied()
            .or_else(|| self.hit_test_node(scr_pt))
        {
            self.drag_transients.active_drag_before_snapshot = Some(self.snapshot());
            self.upsert_curved_trajectory_in_place(CurvedTrajectoryLink {
                node_id: sel_id,
                companion_layer_index: 0,
                control1: font_pt,
                control2: None,
            });
            self.drawing_tool_state.active_trajectory_drag = Some((sel_id, 0));
            self.drawing_tool_state.active_trajectory_drag_is_control2 = false;
            return true;
        }
        false
    }

    pub(crate) fn step_brush_stroke_in_place(&mut self, center_upm: FontPoint, r_upm: f64) -> bool {
        let closed_flags = self.closed_flags();
        let changed = match self.active_tool {
            crate::adapters::state::CanvasTool::Trowel => {
                let mut next_id = self.next_safe_node_id_seed_ext();
                let ch = apply_trowel_brush_in_contours(
                    &mut self.contours,
                    &closed_flags,
                    center_upm,
                    r_upm,
                    self.curve_harmony.trowel_fit_tolerance_upm,
                    &mut next_id,
                );
                if ch {
                    self.next_node_id = next_id;
                }
                ch
            }
            crate::adapters::state::CanvasTool::Cleaner => apply_cleaner_brush_in_contours(
                &mut self.contours,
                &closed_flags,
                center_upm,
                r_upm,
                self.curve_harmony.cleaner_max_loop_area_upm2,
            ),
            _ => false,
        };
        if changed {
            self.selected_nodes
                .retain(|id| self.contours.iter().flatten().any(|n| n.id == *id));
            self.sync_paths_from_contours();
        }
        changed
    }

    pub(crate) fn upsert_curved_trajectory_in_place(&mut self, link: CurvedTrajectoryLink) {
        if let Some(existing) = self.curve_harmony.curved_trajectories.iter_mut().find(|l| {
            l.node_id == link.node_id && l.companion_layer_index == link.companion_layer_index
        }) {
            *existing = link;
        } else {
            self.curve_harmony.curved_trajectories.push(link);
        }
        self.bump_content_generation();
    }
}
