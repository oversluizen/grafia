//! `SPEC-012` (`grafia v0.11.0`) adapter operations on `GrafiaCanvasState`:
//! Non-Destructive Smart Corners & Ink Traps, Pressure & Tilt Calligraphic Brush,
//! Curvature Comb Spines & Heatmap Envelopes, Orthogonal Stem Thickness Gauge,
//! Green's Theorem Counter Balance & Centroid, and Live Optical Symmetry Ghost Overlays.

use crate::adapters::state::GrafiaCanvasState;
use crate::core::{
    compute_contours_counter_balance, compute_contours_curvature_comb_spines,
    compute_contours_curvature_heatmap_envelopes, compute_contours_inflection_markers,
    compute_contours_symmetry_ghost, compute_radial_symmetry_contours,
    evaluate_smart_corner_contours, fit_calligraphic_brush_stroke,
    measure_contour_orthogonal_stem_thickness, CalligraphicNibSpec, CalligraphicSample, ContourId,
    CounterBalanceReport, CurvatureCombSpine, CurvatureCombStyle, CurvatureHeatmapEnvelope,
    GlyphContour, InflectionMarker, Node, NodeId, SmartCornerSpec, StemThicknessMeasurement,
    SymmetryOverlayMode,
};

impl GrafiaCanvasState {
    fn next_safe_node_id_seed(&self) -> u64 {
        let max_existing = self
            .contours
            .iter()
            .flatten()
            .map(|n| n.id.0 + 1)
            .max()
            .unwrap_or(1);
        self.next_node_id.max(max_existing)
    }

    /// Returns the active non-destructive `SmartCornerSpec` slices from the canonical `curve_harmony` domain state (`SPEC-014 §2.7`).
    #[must_use]
    pub fn smart_corners(&self) -> &[SmartCornerSpec] {
        &self.curve_harmony.smart_corners
    }

    /// Attaches or updates a non-destructive `SmartCornerSpec` (`radius_upm > 0.0` fillet, `radius_upm < 0.0` ink trap, `SPEC-012 §2.1`).
    pub fn set_smart_corner(&mut self, spec: SmartCornerSpec) {
        if !spec.radius_upm.is_finite() {
            return;
        }
        let before = self.snapshot();
        if let Some(existing) = self
            .curve_harmony
            .smart_corners
            .iter_mut()
            .find(|s| s.node_id == spec.node_id)
        {
            existing.radius_upm = spec.radius_upm;
        } else {
            self.curve_harmony.smart_corners.push(spec);
        }
        self.record_mutation("Set Smart Corner", before);
        self.bump_content_generation();
        self.assert_invariants();
    }

    /// Removes any non-destructive `SmartCornerSpec` attached to `node_id` (`SPEC-012 §2.1`).
    pub fn remove_smart_corner(&mut self, node_id: NodeId) {
        let before = self.snapshot();
        self.curve_harmony
            .smart_corners
            .retain(|s| s.node_id != node_id);
        self.record_mutation("Remove Smart Corner", before);
        self.bump_content_generation();
        self.assert_invariants();
    }

    /// Evaluates all `SmartCornerSpec` fillets and ink-trap notches non-destructively (`SPEC-012 §2.1.4`).
    #[must_use]
    pub fn compute_smart_corner_preview_contours(&self) -> Vec<GlyphContour> {
        let mut preview_next_id = self.next_safe_node_id_seed();
        let cornered = evaluate_smart_corner_contours(
            &self.glyph.contours,
            &self.curve_harmony.smart_corners,
            &mut preview_next_id,
        );
        if self.icon_studio.radial_symmetry.fold_count > 1 {
            compute_radial_symmetry_contours(
                &cornered,
                &self.icon_studio.radial_symmetry,
                &mut preview_next_id,
            )
        } else {
            cornered
        }
    }

    /// Destructively commits all active Smart Corner fillets and Ink Trap notches into `self.glyph.contours` inside an Undo transaction (`SPEC-012 §2.1.4`).
    pub fn bake_smart_corners(&mut self) {
        if self.curve_harmony.smart_corners.is_empty() {
            return;
        }
        let before = self.snapshot();
        let mut next_id = self.next_safe_node_id_seed();
        let baked = evaluate_smart_corner_contours(
            &self.glyph.contours,
            &self.curve_harmony.smart_corners,
            &mut next_id,
        );
        self.next_node_id = next_id;
        self.contours = baked.iter().map(|c| c.nodes.clone()).collect();
        self.active_contour_closed = baked.last().is_none_or(|c| c.closed);
        self.glyph.contours = baked;
        self.curve_harmony.smart_corners.clear();
        self.selected_nodes
            .retain(|id| self.contours.iter().flatten().any(|n| n.id == *id));
        self.sync_paths_from_contours();
        self.record_mutation("Bake Smart Corners", before);
        self.assert_invariants();
    }

    /// Fits a closed cubic Bézier calligraphic stroke from `samples` and appends it to the active layer inside an Undo transaction (`SPEC-012 §2.2`).
    pub fn apply_calligraphic_brush_stroke(
        &mut self,
        nib: CalligraphicNibSpec,
        samples: &[CalligraphicSample],
    ) -> Option<ContourId> {
        let mut fitted = fit_calligraphic_brush_stroke(nib, samples)?;
        let before = self.snapshot();
        let mut next_id = self.next_safe_node_id_seed();
        for node in &mut fitted.nodes {
            node.id = NodeId(next_id);
            next_id += 1;
        }
        let cid = ContourId(self.glyph.contours.len());
        fitted.id = cid;
        fitted.closed = true;
        let mirrored_opt = crate::core::reflect_contour_across_mirror_axis(
            &fitted,
            self.curve_harmony.active_mirror_axis,
            &mut next_id,
        );
        self.next_node_id = next_id;
        self.contours.push(fitted.nodes.clone());
        self.active_contour_closed = true;
        self.glyph.contours.push(fitted);
        if let Some(mut mirrored) = mirrored_opt {
            mirrored.id = ContourId(self.glyph.contours.len());
            self.contours.push(mirrored.nodes.clone());
            self.glyph.contours.push(mirrored);
        }
        self.sync_paths_from_contours();
        self.record_mutation("Calligraphic Brush Stroke", before);
        self.assert_invariants();
        Some(cid)
    }

    /// Configures the `CurvatureCombStyle` (`SpinesOnly` vs. `HeatmapEnvelope`, `SPEC-012 §2.3.3`).
    pub fn set_curvature_comb_style(&mut self, style: CurvatureCombStyle) {
        self.curve_harmony.curvature_comb_style = style;
        self.bump_content_generation();
    }

    /// Evaluates analytical curvature comb spines across all contours in the active layer (`SPEC-012 §2.3.1`).
    #[must_use]
    pub fn compute_curvature_comb_spines(
        &self,
        samples_per_segment: usize,
        scale_upm2: f64,
    ) -> Vec<Vec<CurvatureCombSpine>> {
        compute_contours_curvature_comb_spines(
            &self.glyph.contours,
            samples_per_segment,
            scale_upm2,
        )
    }

    /// Computes all exact interior cubic inflection markers across the active layer (`SPEC-012 §2.3.2`).
    #[must_use]
    pub fn compute_layer_inflection_markers(&self) -> Vec<InflectionMarker> {
        compute_contours_inflection_markers(&self.glyph.contours)
    }

    /// Computes closed normalized curvature heatmap envelopes and inflection markers across the active layer (`SPEC-012 §2.3.3`).
    #[must_use]
    pub fn compute_curvature_heatmap_envelopes(
        &self,
        samples_per_segment: usize,
        scale_upm2: f64,
    ) -> Vec<CurvatureHeatmapEnvelope> {
        compute_contours_curvature_heatmap_envelopes(
            &self.glyph.contours,
            samples_per_segment,
            scale_upm2,
        )
    }

    /// Configures reference stem widths and matching tolerance in UPM (`SPEC-012 §2.4.2`).
    pub fn set_reference_stems(&mut self, stems_upm: Vec<f64>, tolerance_upm: f64) {
        self.curve_harmony.reference_stems_upm =
            stems_upm.into_iter().filter(|s| s.is_finite()).collect();
        self.curve_harmony.stem_tolerance_upm = if tolerance_upm.is_finite() {
            tolerance_upm.abs()
        } else {
            0.0
        };
        self.bump_content_generation();
    }

    /// Casts an inward unit normal ray from `(contour_idx, seg_idx, t)` and classifies stem thickness against `reference_stems_upm` (`SPEC-012 §2.4`).
    #[must_use]
    pub fn measure_orthogonal_stem_thickness(
        &self,
        contour_idx: usize,
        seg_idx: usize,
        t: f64,
    ) -> Option<StemThicknessMeasurement> {
        measure_contour_orthogonal_stem_thickness(
            &self.glyph.contours,
            contour_idx,
            seg_idx,
            t,
            &self.curve_harmony.reference_stems_upm,
            self.curve_harmony.stem_tolerance_upm,
        )
    }

    /// Evaluates Green's theorem stem/counter area balance and visual centroid $\oplus$ for the active layer (`SPEC-012 §2.5.2`).
    #[must_use]
    pub fn compute_layer_counter_balance(&self) -> CounterBalanceReport {
        compute_contours_counter_balance(&self.glyph.contours)
    }

    /// Sets the live optical symmetry ghost overlay mode (`SPEC-012 §2.6.2`).
    pub fn set_symmetry_overlay_mode(&mut self, mode: SymmetryOverlayMode) {
        self.curve_harmony.symmetry_overlay_mode = mode;
        self.bump_content_generation();
    }

    /// Enables or disables the live orthogonal stem thickness gauge overlay under the cursor (`SPEC-012 §2.4.4`).
    pub fn set_show_stem_gauge(&mut self, enabled: bool) {
        self.curve_harmony.show_stem_gauge = enabled;
        self.bump_content_generation();
    }

    /// Toggles the live orthogonal stem thickness gauge overlay and returns the new state (`SPEC-012 §2.4.4`).
    pub fn toggle_show_stem_gauge(&mut self) -> bool {
        self.curve_harmony.show_stem_gauge = !self.curve_harmony.show_stem_gauge;
        self.bump_content_generation();
        self.curve_harmony.show_stem_gauge
    }

    /// Enables or disables the Green's theorem visual centroid crosshair $\oplus$ overlay (`SPEC-012 §2.5.2`).
    pub fn set_show_visual_centroid(&mut self, enabled: bool) {
        self.curve_harmony.show_visual_centroid = enabled;
        self.bump_content_generation();
    }

    /// Toggles the Green's theorem visual centroid crosshair $\oplus$ overlay and returns the new state (`SPEC-012 §2.5.2`).
    pub fn toggle_show_visual_centroid(&mut self) -> bool {
        self.curve_harmony.show_visual_centroid = !self.curve_harmony.show_visual_centroid;
        self.bump_content_generation();
        self.curve_harmony.show_visual_centroid
    }

    /// Computes reflected or $180^\circ$-point-inverted ghost contours for the active `SymmetryOverlayMode` (`SPEC-012 §2.6.2`).
    #[must_use]
    pub fn compute_symmetry_ghost_contours(&self) -> Vec<Vec<Node>> {
        compute_contours_symmetry_ghost(
            &self.glyph.contours,
            self.curve_harmony.symmetry_overlay_mode,
        )
    }

    pub fn load_preset_a(&mut self) {
        self.load_glyph_archetype(crate::tokens::GlyphArchetype::A);
    }
    pub fn load_preset_n(&mut self) {
        self.load_glyph_archetype(crate::tokens::GlyphArchetype::N);
    }
    pub fn load_preset_o(&mut self) {
        self.load_glyph_archetype(crate::tokens::GlyphArchetype::O);
    }
    pub fn load_preset_s(&mut self) {
        self.load_glyph_archetype(crate::tokens::GlyphArchetype::S);
    }

    pub fn reverse_selected_or_all_contours(&mut self) -> bool {
        if self.contours.is_empty() {
            return false;
        }
        let before = self.snapshot();
        let closed_flags = self.closed_flags();
        let any_sel = !self.selected_nodes.is_empty();
        for (c_idx, contour) in self.contours.iter_mut().enumerate() {
            if !any_sel || contour.iter().any(|n| self.selected_nodes.contains(&n.id)) {
                crate::core::reverse_contour_nodes(contour, closed_flags[c_idx]);
            }
        }
        self.pending_pen_bcp_out = None;
        self.pending_pen_first_bcp_in = None;
        self.hovered_pen_close_target = None;
        self.sync_paths_from_contours();
        self.record_mutation("Reverse Contour", before);
        self.assert_invariants();
        true
    }

    pub fn make_selected_node_first(&mut self) -> bool {
        self.make_node_first()
    }

    pub(crate) fn build_hinting_and_hoi_snapshot(
        &self,
    ) -> crate::interaction::history::HintingAndHoiSnapshot {
        crate::interaction::history::HintingAndHoiSnapshot {
            postscript_hints: self.postscript_hints.clone(),
            selected_postscript_hint: self.selected_postscript_hint,
            truetype_instructions: self.truetype_instructions.clone(),
            selected_truetype_instruction: self.selected_truetype_instruction,
            hoi_layer_sets: self.drag_transients.hoi_layer_sets.clone(),
            hoi_sibling_contours: self.drag_transients.hoi_sibling_contours.clone(),
            curve_harmony: self.curve_harmony.clone(),
            icon_studio: self.icon_studio.clone(),
        }
    }
}

pub(crate) fn snap_extremum_if_shift(hit: &mut crate::core::SegmentHit, is_shift: bool) {
    if is_shift {
        if let crate::core::ContourSegment::Cubic { cubic, .. } = hit.segment {
            let (ext_t, ext_pt) = crate::core::find_nearest_cubic_extremum(cubic, hit.t);
            hit.t = ext_t;
            hit.point = ext_pt;
        }
    }
}

pub(crate) fn other_open_contour(state: &GrafiaCanvasState, c_idx: usize) -> Option<usize> {
    state.active_open_contour.filter(|&a| {
        a != c_idx
            && a < state.contours.len()
            && !state.is_contour_closed(a)
            && !state.contours[a].is_empty()
    })
}

pub(crate) fn update_pen_idle_hover(
    state: &mut GrafiaCanvasState,
    scr_pt: crate::core::ScreenPoint,
    font_pt: crate::core::FontPoint,
    is_shift: bool,
    canvas_rect: egui::Rect,
) {
    state.hovered_pen_close_target = None;
    state.hovered_segment_hit = None;
    if state.select_state == crate::interaction::SelectToolState::Idle
        && canvas_rect.contains(egui::pos2(scr_pt.x.0, scr_pt.y.0))
    {
        if state.active_tool == crate::adapters::state::CanvasTool::Pen {
            let close_or_join_hit =
                crate::adapters::cursor_overlay::find_open_contour_close_or_resume_target(
                    state, scr_pt,
                )
                .filter(|&(c_idx, _, is_closing_start)| {
                    is_closing_start || other_open_contour(state, c_idx).is_some()
                });
            if let Some((c_idx, target_node, _)) = close_or_join_hit {
                state.hovered_pen_close_target = Some((c_idx, target_node.id));
            } else if state.pen_hover_node_hit(scr_pt).is_none() {
                let tol_font = f64::from(state.effective_segment_hit_tolerance_dp())
                    / f64::from(state.viewport.zoom);
                let closed_flags = state.closed_flags();
                let mut hit_opt = crate::core::hit_test_segments(
                    &state.contours,
                    &closed_flags,
                    font_pt,
                    tol_font,
                );
                if let Some(ref mut hit) = hit_opt {
                    snap_extremum_if_shift(hit, is_shift);
                }
                state.hovered_segment_hit = hit_opt;
            }
        } else if state.curve_harmony.show_stem_gauge {
            let tol_font = (f64::from(state.effective_segment_hit_tolerance_dp())
                * crate::core::math_consts::STEM_GAUGE_HOVER_TOLERANCE_MULTIPLIER)
                / f64::from(state.viewport.zoom);
            let closed_flags = state.closed_flags();
            state.hovered_segment_hit =
                crate::core::hit_test_segments(&state.contours, &closed_flags, font_pt, tol_font);
        }
    }
}

pub fn apply_segment_click_selection(
    state: &mut GrafiaCanvasState,
    c_idx: usize,
    seg: crate::core::ContourSegment,
    mods: crate::interaction::Modifiers,
) {
    let Some(contour) = state.contours.get(c_idx) else {
        return;
    };
    let target_ids: Vec<NodeId> = match seg {
        crate::core::ContourSegment::Line {
            start_idx, end_idx, ..
        } => vec![contour[start_idx].id, contour[end_idx].id],
        crate::core::ContourSegment::Cubic {
            start_idx,
            h1_idx,
            h2_idx,
            end_idx,
            ..
        } => vec![
            contour[start_idx].id,
            contour[h1_idx].id,
            contour[h2_idx].id,
            contour[end_idx].id,
        ],
    };
    if mods.shift {
        let mut sel = state.drag_transients.marquee_pre_selection.clone();
        if target_ids.iter().all(|id| sel.contains(id)) {
            sel.retain(|id| !target_ids.contains(id));
        } else {
            for id in target_ids {
                if !sel.contains(&id) {
                    sel.push(id);
                }
            }
        }
        state.selected_nodes = sel;
    } else {
        state.selected_nodes = target_ids;
    }
}
