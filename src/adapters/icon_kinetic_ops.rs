//! Adapter operations for `SPEC-016` Radial Symmetry, Stem-Preserving Optical Oblique,
//! 5-Stage Multi-Scale Icon Normalizer, and 10-Mode Kinetic Symbol Evaluation.

use crate::adapters::headless::kurbo_to_tiny_skia_path;
use crate::adapters::state::GrafiaCanvasState;
use crate::core::math_consts::{
    FIT_CURVE_MIN_CHORD_LEN_EPS, ROTARY_CORNER_RADIUS_STEP_UPM, VARIABLE_WIDTH_MAX_UPM,
};
use crate::core::{
    audit_and_normalize_icon_contours, bake_liquid_glass_to_colrv1_sublayers,
    compute_optical_oblique_contours, compute_radial_symmetry_contours,
    contour_to_bezpath_with_splines, contour_total_arc_length,
    evaluate_kinetic_color_sublayers_with_paths, evaluate_kinetic_glyph_contours,
    evaluate_smart_corner_contours, evaluate_variable_width_at_t, expand_single_contour_stroke,
    expand_variable_width_contour, resolve_sublayers_for_color_mode, CanvasColorSublayer,
    CanvasIconStudioScene, ContourId, FontPoint, GlyphContour, RotaryTelemetryInfo,
    VariableWidthStrokeSpec, WidthNodeSample,
};
use crate::tokens::{color32_to_tiny_skia, GrafiaTheme, CURVE_FLATTEN_TOLERANCE};
use egui::{Painter, Pos2, Shape, Stroke};
use kurbo::PathEl;
use tiny_skia::{Paint, Pixmap, Stroke as TsStroke, Transform};

impl GrafiaCanvasState {
    /// Replaces `self.icon_studio` with an Undo/Redo transaction (`SPEC-016 §2.1`).
    pub fn set_icon_studio_scene(&mut self, scene: CanvasIconStudioScene) {
        let before = self.snapshot();
        self.icon_studio = scene.sanitized();
        self.record_mutation("Update Icon Studio Scene", before);
        self.assert_invariants();
    }

    /// Advances the kinetic timeline by `dt_sec` when `icon_studio.kinetic.playing == true` (`SPEC-016 §2.2`).
    pub fn step_kinetic_timeline(&mut self, dt_sec: f64) -> bool {
        let changed = self.icon_studio.kinetic.step_timeline(dt_sec);
        if changed {
            self.bump_content_generation();
        }
        changed
    }

    /// Destructively expands `icon_studio.radial_symmetry` into editable `GlyphContour`s (`SPEC-016 §3.3.1`).
    pub fn bake_radial_symmetry(&mut self) -> bool {
        if self.glyph.contours.is_empty() || self.icon_studio.radial_symmetry.fold_count <= 1 {
            return false;
        }
        let mut next_nid = self.next_node_id.max(1);
        let base = evaluate_smart_corner_contours(
            &self.glyph.contours,
            &self.curve_harmony.smart_corners,
            &mut next_nid,
        );
        let new_c = compute_radial_symmetry_contours(
            &base,
            &self.icon_studio.radial_symmetry,
            &mut next_nid,
        );
        if new_c.is_empty() {
            return false;
        }
        let before = self.snapshot();
        self.icon_studio.radial_symmetry.fold_count = 1;
        self.curve_harmony.smart_corners.clear();
        self.finish_icon_contour_replacement(new_c, next_nid, "Bake Radial Symmetry", before)
    }

    /// Applies stem-preserving optical oblique (or naive horizontal shear) by `slant_deg` (`SPEC-016 §3.3.2`).
    pub fn apply_optical_oblique(&mut self, slant_deg: f64, preserve_stem_width: bool) -> bool {
        if !slant_deg.is_finite()
            || slant_deg.abs() <= FIT_CURVE_MIN_CHORD_LEN_EPS
            || self.glyph.contours.is_empty()
        {
            return false;
        }
        let mut next_nid = self.next_node_id.max(1);
        let new_c = compute_optical_oblique_contours(
            &self.glyph.contours,
            slant_deg,
            preserve_stem_width,
            &mut next_nid,
        );
        (!new_c.is_empty()) && self.apply_icon_contour_mutation(new_c, next_nid, "Optical Oblique")
    }

    /// Runs the 5-stage *"Clean for Font & Glass"* multi-scale icon normalizer on the active layer (`SPEC-016 §3.4`).
    pub fn normalize_icon_contours(&mut self, grid_step_opt: Option<f64>) -> bool {
        if self.glyph.contours.is_empty() {
            return false;
        }
        let mut next_nid = self.next_node_id.max(1);
        let new_c =
            audit_and_normalize_icon_contours(&self.glyph.contours, grid_step_opt, &mut next_nid);
        (!new_c.is_empty())
            && self.apply_icon_contour_mutation(new_c, next_nid, "Normalize Icon Contours")
    }

    /// Evaluates `self.color_sublayers` at the current `icon_studio.kinetic` phase and `icon_studio.color_mode` (`SPEC-016 §4.2`, `SPEC-017 §2.3 & §2.6`).
    #[must_use]
    pub fn evaluate_active_kinetic_sublayers(&self) -> Vec<CanvasColorSublayer> {
        let is = &self.icon_studio;
        let resolved = resolve_sublayers_for_color_mode(
            &self.color_sublayers,
            &is.symbol_layer_tags,
            is.color_mode,
        );
        evaluate_kinetic_color_sublayers_with_paths(
            &resolved,
            &is.symbol_layer_tags,
            &self.anchors,
            &is.kinetic,
            &self.glyph.contours,
        )
    }

    /// Evaluates `self.glyph.contours` at the current `icon_studio.kinetic` phase (`SPEC-016 §4.2`, `SPEC-017 §2.4`).
    #[must_use]
    pub fn evaluate_active_kinetic_contours(&self) -> Vec<(GlyphContour, f32)> {
        let is = &self.icon_studio;
        let raw = evaluate_kinetic_glyph_contours(
            &self.glyph.contours,
            &is.symbol_layer_tags,
            &self.anchors,
            &is.kinetic,
        );
        if is.variable_width_strokes.is_empty() && self.contour_strokes.is_empty() {
            return raw;
        }
        let mut next_nid = self.next_node_id.max(1);
        let mut out = Vec::with_capacity(raw.len());
        for (idx, (eval_c, alpha)) in raw.into_iter().enumerate() {
            if eval_c.nodes.len() >= 2 {
                if let Some(spec) = is
                    .variable_width_strokes
                    .iter()
                    .find(|s| s.contour_id == eval_c.id)
                {
                    let orig_len = self
                        .glyph
                        .contours
                        .get(idx)
                        .map_or(0.0, contour_total_arc_length);
                    let eval_len = contour_total_arc_length(&eval_c);
                    let ratio = if orig_len > FIT_CURVE_MIN_CHORD_LEN_EPS {
                        (eval_len / orig_len).clamp(0.0, 1.0)
                    } else {
                        1.0
                    };
                    let mut samples = Vec::with_capacity(spec.samples.len().max(2) + 2);
                    for u in [0.0, 0.5, 1.0] {
                        let (wl, wr) = evaluate_variable_width_at_t(spec, u * ratio);
                        samples.push(WidthNodeSample {
                            arc_ratio_t: u,
                            width_left_upm: wl,
                            width_right_upm: wr,
                        });
                    }
                    let sub_spec = VariableWidthStrokeSpec {
                        contour_id: eval_c.id,
                        samples,
                        cap_style: spec.cap_style,
                    };
                    let expanded = expand_variable_width_contour(&eval_c, &sub_spec, &mut next_nid);
                    if !expanded.is_empty() {
                        for mut c in expanded {
                            c.id = ContourId(out.len());
                            out.push((c, alpha));
                        }
                        continue;
                    }
                }
                if let Some(&st) = self
                    .contour_strokes
                    .get(&eval_c.id)
                    .filter(|s| s.width > 0.0 && s.height > 0.0)
                {
                    let mut next_cid = eval_c.id.0;
                    let expanded =
                        expand_single_contour_stroke(&eval_c, st, &mut next_cid, &mut next_nid);
                    if !expanded.is_empty() {
                        for mut c in expanded {
                            c.id = ContourId(out.len());
                            out.push((c, alpha));
                        }
                        continue;
                    }
                }
            }
            out.push((eval_c, alpha));
        }
        out
    }

    pub(crate) fn paint_kinetic_silhouette_egui(
        &self,
        painter: &Painter,
        fill_color: egui::Color32,
    ) -> bool {
        if !self.icon_studio.kinetic.is_active() {
            return false;
        }
        let vp = self.active_slot_viewport();
        for (gc, alpha) in self.evaluate_active_kinetic_contours() {
            if !gc.visible || gc.nodes.len() < 2 {
                continue;
            }
            let col = fill_color.gamma_multiply(alpha.clamp(0.0, 1.0));
            if gc.closed {
                let mesh = crate::render::tessellate_closed_contours_nonzero(
                    std::slice::from_ref(&gc.nodes),
                    &[true],
                    &vp,
                    col,
                );
                if !mesh.is_empty() {
                    painter.add(Shape::mesh(mesh));
                }
            } else {
                let bez = contour_to_bezpath_with_splines(&gc.nodes, false, 0.0, &[]);
                let mut pts = Vec::new();
                kurbo::flatten(bez, CURVE_FLATTEN_TOLERANCE, |el| {
                    if let PathEl::MoveTo(p) | PathEl::LineTo(p) = el {
                        let s = vp.to_screen(FontPoint::new(p.x, p.y));
                        pts.push(Pos2::new(s.x.0, s.y.0));
                    }
                });
                if pts.len() >= 2 {
                    painter.add(Shape::line(pts, Stroke::new(2.0, col)));
                }
            }
        }
        true
    }

    pub(crate) fn paint_kinetic_wireframe_egui(
        &self,
        painter: &Painter,
        theme: &GrafiaTheme,
    ) -> bool {
        if !self.icon_studio.kinetic.is_active() {
            return false;
        }
        let vp = self.active_slot_viewport();
        let base_col = theme.preview_canvas.fill_color32();
        let sw = theme.edit_canvas.contour_stroke_width_dp;
        for (gc, alpha) in self.evaluate_active_kinetic_contours() {
            if !gc.visible || gc.nodes.len() < 2 {
                continue;
            }
            let bez = contour_to_bezpath_with_splines(&gc.nodes, gc.closed, 0.0, &[]);
            let mut pts = Vec::new();
            kurbo::flatten(bez.iter(), CURVE_FLATTEN_TOLERANCE, |el| {
                if let PathEl::MoveTo(p) | PathEl::LineTo(p) = el {
                    let s = vp.to_screen(FontPoint::new(p.x, p.y));
                    pts.push(Pos2::new(s.x.0, s.y.0));
                }
            });
            if pts.len() >= 2 {
                let st = Stroke::new(sw, base_col.gamma_multiply(alpha.clamp(0.0, 1.0)));
                painter.add(if gc.closed {
                    Shape::closed_line(pts, st)
                } else {
                    Shape::line(pts, st)
                });
            }
        }
        true
    }

    pub(crate) fn render_kinetic_wireframe_ts(
        &self,
        pixmap: &mut Pixmap,
        theme: &GrafiaTheme,
    ) -> bool {
        if !self.icon_studio.kinetic.is_active() {
            return false;
        }
        let vp = self.active_slot_viewport();
        let affine = kurbo::Affine::translate((vp.pan_offset.x.0 as f64, vp.pan_offset.y.0 as f64))
            * kurbo::Affine::scale_non_uniform(vp.zoom as f64, -(vp.zoom as f64));
        let stroke = TsStroke {
            width: theme.edit_canvas.contour_stroke_width_dp,
            ..Default::default()
        };
        let base_c32 = theme.preview_canvas.fill_color32();
        for (gc, alpha) in self.evaluate_active_kinetic_contours() {
            if !gc.visible || gc.nodes.len() < 2 {
                continue;
            }
            let bez = contour_to_bezpath_with_splines(&gc.nodes, gc.closed, 0.0, &[]);
            let screen_path = affine * bez;
            if let Some(ts_path) = kurbo_to_tiny_skia_path(&screen_path) {
                let mut kp = Paint::default();
                kp.set_color(color32_to_tiny_skia(
                    base_c32.gamma_multiply(alpha.clamp(0.0, 1.0)),
                ));
                kp.anti_alias = true;
                pixmap.stroke_path(&ts_path, &kp, &stroke, Transform::identity(), None);
            }
        }
        true
    }

    pub(crate) fn try_step_icon_studio_rotary(&mut self, delta_ticks: f64) -> Option<(bool, bool)> {
        if !self.selected_nodes.is_empty() || self.icon_studio.variable_width_strokes.is_empty() {
            return None;
        }
        let changed =
            self.scale_active_variable_width_stroke(delta_ticks * ROTARY_CORNER_RADIUS_STEP_UPM);
        Some((changed, !changed))
    }

    pub(crate) fn try_icon_studio_rotary_telemetry(&self) -> Option<RotaryTelemetryInfo> {
        if !self.selected_nodes.is_empty() {
            return None;
        }
        let s = self
            .icon_studio
            .variable_width_strokes
            .first()?
            .samples
            .first()?;
        let mean_w = 0.5 * (s.width_left_upm + s.width_right_upm);
        Some(RotaryTelemetryInfo {
            parameter_name: "Variable Width",
            formatted_value: format!("{mean_w:.0} UPM"),
            normalized_ratio: (mean_w / VARIABLE_WIDTH_MAX_UPM).clamp(0.0, 1.0),
        })
    }

    /// Destructively expands all active `VariableWidthStrokeSpec` contours into closed outlines (`SPEC-016 §3.2`).
    pub fn bake_variable_width_strokes(&mut self) -> bool {
        if self.icon_studio.variable_width_strokes.is_empty() {
            return false;
        }
        let mut next_nid = self.next_node_id.max(1);
        let mut new_contours: Vec<GlyphContour> = Vec::new();
        let mut any_expanded = false;
        for gc in &self.glyph.contours {
            if let Some(spec) = self
                .icon_studio
                .variable_width_strokes
                .iter()
                .find(|s| s.contour_id == gc.id)
            {
                let expanded = expand_variable_width_contour(gc, spec, &mut next_nid);
                if !expanded.is_empty() {
                    any_expanded = true;
                    for mut c in expanded {
                        c.id = ContourId(new_contours.len());
                        new_contours.push(c);
                    }
                    continue;
                }
            }
            let mut copy = gc.clone();
            copy.id = ContourId(new_contours.len());
            new_contours.push(copy);
        }
        if !any_expanded {
            return false;
        }
        let before = self.snapshot();
        self.icon_studio.variable_width_strokes.clear();
        self.selected_nodes.clear();
        self.finish_icon_contour_replacement(
            new_contours,
            next_nid,
            "Expand Variable-Width Stroke",
            before,
        )
    }

    /// Bakes active closed glyph contours into 3 pure-vector `COLRv1` `CanvasColorSublayer`s (`SPEC-016 §5.2`).
    pub fn bake_liquid_glass_to_colrv1(&mut self) -> bool {
        let baked = bake_liquid_glass_to_colrv1_sublayers(
            &self.glyph.contours,
            &self.icon_studio.liquid_glass,
        );
        if baked.is_empty() {
            return false;
        }
        let before = self.snapshot();
        self.set_color_sublayers(baked);
        self.select_color_sublayer(Some(0));
        self.record_mutation("Bake Liquid Glass to COLRv1", before);
        self.assert_invariants();
        true
    }
}
