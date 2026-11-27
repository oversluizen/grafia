//! 1-Click `COLRv1` Vector Baker (`SPEC-016 §5.2`, `ISO/IEC 14496-22` `COLRv1`).
//!
//! Converts an active contour set and `LiquidGlassRenderConfig` into 3 pure-vector
//! `CanvasColorSublayer` entries per contour group:
//! 1. **`LinearGradient` Refractive Dome Body** aligned along the primary light axis,
//! 2. **`RadialGradient` Schlick-Fresnel Rim Glow** centered on the contour bounding box,
//! 3. **`LinearGradient` Specular Crest Highlight** concentrated near the top-left dome crest.

use crate::core::color_layers::{
    CanvasColorSublayer, ColorLayerPaint, GradientSpread, GradientStop,
};
use crate::core::coordinates::FontPoint;
use crate::core::glyph_model::GlyphContour;
use crate::core::icon_knockout_mask::KineticPlaybackMode;
use crate::core::icon_studio::{
    CanvasIconStudioScene, KineticSymbolEffect, LiquidGlassRenderConfig, WiggleAxis,
};
use crate::core::math_consts::{
    COLRV1_BAKE_CREST_ALPHA_BASE_F32, COLRV1_BAKE_CREST_OPACITY, COLRV1_BAKE_CREST_SPAN_FRAC,
    COLRV1_BAKE_CREST_STOP_FADE, COLRV1_BAKE_CREST_STOP_PEAK, COLRV1_BAKE_DARK_PENALTY_U8,
    COLRV1_BAKE_INNER_RADIUS_FRAC, COLRV1_BAKE_LIGHT_BONUS_U8, COLRV1_BAKE_MIN_SPAN_UPM,
    COLRV1_BAKE_RIM_ALPHA_U8, COLRV1_BAKE_RIM_OPACITY, COLRV1_BAKE_RIM_STOP_EDGE,
    COLRV1_BAKE_RIM_STOP_INNER, COLRV1_BAKE_STOP_MID, ICON_STUDIO_MAX_COORD_ABS_UPM,
    KINETIC_MAX_SPEED_MULTIPLIER, KINETIC_MIN_SPEED_MULTIPLIER, KINETIC_REPEAT_DELAY_MAX_PHASE,
    KNOCKOUT_MARGIN_MAX_UPM, RADIAL_SYMMETRY_MAX_FOLDS, RADIAL_SYMMETRY_MIN_FOLDS,
    VARIABLE_WIDTH_MAX_UPM,
};
use crate::core::reference_frame::RectUpm;

fn valid_coord(pt: FontPoint) -> bool {
    pt.x.0.is_finite()
        && pt.y.0.is_finite()
        && pt.x.0.abs() <= ICON_STUDIO_MAX_COORD_ABS_UPM
        && pt.y.0.abs() <= ICON_STUDIO_MAX_COORD_ABS_UPM
}

impl CanvasIconStudioScene {
    /// Validates all `SPEC-016 §2.3` state invariants.
    pub fn validate_invariants(&self) -> Result<(), String> {
        let kin_ok = (0.0..=1.0).contains(&self.kinetic.phase_t)
            && (KINETIC_MIN_SPEED_MULTIPLIER..=KINETIC_MAX_SPEED_MULTIPLIER)
                .contains(&self.kinetic.speed_multiplier)
            && match &self.kinetic.effect {
                KineticSymbolEffect::Wiggle {
                    axis: WiggleAxis::CustomAngle(deg),
                    ..
                } => deg.is_finite(),
                KineticSymbolEffect::MagicReplace {
                    target_contours, ..
                } => target_contours
                    .iter()
                    .all(|c| c.nodes.iter().all(|n| valid_coord(n.point))),
                _ => true,
            };
        let sym_ok = (RADIAL_SYMMETRY_MIN_FOLDS..=RADIAL_SYMMETRY_MAX_FOLDS)
            .contains(&self.radial_symmetry.fold_count)
            && valid_coord(self.radial_symmetry.center)
            && self.liquid_glass.is_valid();
        let vw_ok = self.variable_width_strokes.iter().all(|vw| {
            let mut prev_t = -1.0_f64;
            vw.samples.iter().all(|s| {
                let ok = (0.0..=1.0).contains(&s.arc_ratio_t)
                    && s.arc_ratio_t >= prev_t
                    && (0.0..=VARIABLE_WIDTH_MAX_UPM).contains(&s.width_left_upm)
                    && (0.0..=VARIABLE_WIDTH_MAX_UPM).contains(&s.width_right_upm);
                prev_t = s.arc_ratio_t;
                ok
            })
        });
        let tags_ok = self.symbol_layer_tags.iter().all(|t| {
            t.layer_group >= 1
                && t.custom_pivot_upm.is_none_or(valid_coord)
                && (0.0..=KNOCKOUT_MARGIN_MAX_UPM).contains(&t.knockout_margin_upm)
                && (-1.0..=1.0).contains(&t.phase_offset)
        });
        let mode_ok = match self.kinetic.playback_mode {
            KineticPlaybackMode::Repeating { repeat_delay_phase } => {
                (0.0..=KINETIC_REPEAT_DELAY_MAX_PHASE).contains(&repeat_delay_phase)
            }
            KineticPlaybackMode::VariableValue(v) => (0.0..=1.0).contains(&v),
            _ => true,
        };
        (kin_ok && sym_ok && vw_ok && tags_ok && mode_ok)
            .then_some(())
            .ok_or_else(|| "Invariant violated: invalid CanvasIconStudioScene state".into())
    }

    /// Sanitizes an external/deserialized `CanvasIconStudioScene` so all fields satisfy `validate_invariants()`.
    #[must_use]
    pub fn sanitized(mut self) -> Self {
        let f64_or = |v: f64, def: f64| if v.is_finite() { v } else { def };
        self.kinetic.phase_t = f64_or(self.kinetic.phase_t, 0.0).clamp(0.0, 1.0);
        self.kinetic.speed_multiplier = f64_or(self.kinetic.speed_multiplier, 1.0)
            .clamp(KINETIC_MIN_SPEED_MULTIPLIER, KINETIC_MAX_SPEED_MULTIPLIER);
        self.kinetic.playback_mode = self.kinetic.playback_mode.sanitized();
        match &mut self.kinetic.effect {
            KineticSymbolEffect::Wiggle {
                axis: WiggleAxis::CustomAngle(deg),
                ..
            } if !deg.is_finite() => *deg = 0.0,
            KineticSymbolEffect::MagicReplace {
                target_contours, ..
            } => {
                for c in target_contours {
                    c.nodes.retain(|n| valid_coord(n.point));
                }
            }
            _ => {}
        }
        self.radial_symmetry.fold_count = self
            .radial_symmetry
            .fold_count
            .clamp(RADIAL_SYMMETRY_MIN_FOLDS, RADIAL_SYMMETRY_MAX_FOLDS);
        if !valid_coord(self.radial_symmetry.center) {
            self.radial_symmetry.center = FontPoint::new(0.0, 0.0);
        }
        self.liquid_glass = self.liquid_glass.sanitized();
        for vw in &mut self.variable_width_strokes {
            vw.samples.retain(|s| {
                s.arc_ratio_t.is_finite()
                    && s.width_left_upm.is_finite()
                    && s.width_right_upm.is_finite()
            });
            for s in &mut vw.samples {
                s.arc_ratio_t = s.arc_ratio_t.clamp(0.0, 1.0);
                s.width_left_upm = s.width_left_upm.clamp(0.0, VARIABLE_WIDTH_MAX_UPM);
                s.width_right_upm = s.width_right_upm.clamp(0.0, VARIABLE_WIDTH_MAX_UPM);
            }
            vw.samples
                .sort_by(|a, b| a.arc_ratio_t.total_cmp(&b.arc_ratio_t));
        }
        for tag in &mut self.symbol_layer_tags {
            tag.layer_group = tag.layer_group.max(1);
            tag.custom_pivot_upm = tag.custom_pivot_upm.filter(|&p| valid_coord(p));
            tag.knockout_margin_upm =
                f64_or(tag.knockout_margin_upm, 0.0).clamp(0.0, KNOCKOUT_MARGIN_MAX_UPM);
            tag.phase_offset = f64_or(tag.phase_offset, 0.0).clamp(-1.0, 1.0);
        }
        self
    }
}

fn collect_valid_baked_contours(contours: &[GlyphContour]) -> Vec<GlyphContour> {
    let mut out = Vec::new();
    for c in contours {
        if !c.visible || !c.closed || c.nodes.len() < 3 {
            continue;
        }
        let mut clean = c.clone();
        clean.nodes.retain(|n| {
            n.point.x.0.is_finite()
                && n.point.y.0.is_finite()
                && n.point.x.0.abs() <= ICON_STUDIO_MAX_COORD_ABS_UPM
                && n.point.y.0.abs() <= ICON_STUDIO_MAX_COORD_ABS_UPM
        });
        if clean.nodes.len() >= 3 {
            out.push(clean);
        }
    }
    out
}

fn compute_contours_bbox_upm(contours: &[GlyphContour]) -> RectUpm {
    let (mut min_x, mut min_y) = (f64::INFINITY, f64::INFINITY);
    let (mut max_x, mut max_y) = (f64::NEG_INFINITY, f64::NEG_INFINITY);
    for c in contours {
        for n in &c.nodes {
            min_x = min_x.min(n.point.x.0);
            min_y = min_y.min(n.point.y.0);
            max_x = max_x.max(n.point.x.0);
            max_y = max_y.max(n.point.y.0);
        }
    }
    if !min_x.is_finite() || !max_x.is_finite() {
        return RectUpm::new(0.0, 0.0, COLRV1_BAKE_MIN_SPAN_UPM, COLRV1_BAKE_MIN_SPAN_UPM);
    }
    if (max_x - min_x) < COLRV1_BAKE_MIN_SPAN_UPM {
        max_x = min_x + COLRV1_BAKE_MIN_SPAN_UPM;
    }
    if (max_y - min_y) < COLRV1_BAKE_MIN_SPAN_UPM {
        max_y = min_y + COLRV1_BAKE_MIN_SPAN_UPM;
    }
    RectUpm::new(min_x, min_y, max_x, max_y)
}

fn build_fresnel_rim_sublayer(
    contours: Vec<GlyphContour>,
    bbox: RectUpm,
    tint_rgba: [u8; 4],
    light_rgba: [u8; 4],
) -> CanvasColorSublayer {
    let (cx, cy) = (
        0.5 * (bbox.min_x + bbox.max_x),
        0.5 * (bbox.min_y + bbox.max_y),
    );
    let r_outer = (0.5 * (bbox.max_x - bbox.min_x).hypot(bbox.max_y - bbox.min_y))
        .max(COLRV1_BAKE_MIN_SPAN_UPM);
    let r_inner = (r_outer * COLRV1_BAKE_INNER_RADIUS_FRAC).max(1.0);
    let [tr, tg, tb, ta] = tint_rgba;
    CanvasColorSublayer {
        id: 2,
        name: "LiquidGlass.FresnelRim".into(),
        contours,
        paint: ColorLayerPaint::RadialGradient {
            c0: FontPoint::new(cx, cy),
            r0: r_inner,
            c1: FontPoint::new(cx, cy),
            r1: r_outer,
            stops: vec![
                GradientStop::new(0.0, [tr, tg, tb, 0]),
                GradientStop::new(COLRV1_BAKE_RIM_STOP_INNER, [tr, tg, tb, 0]),
                GradientStop::new(
                    COLRV1_BAKE_RIM_STOP_EDGE,
                    [
                        light_rgba[0],
                        light_rgba[1],
                        u8::MAX,
                        COLRV1_BAKE_RIM_ALPHA_U8,
                    ],
                ),
                GradientStop::new(1.0, [u8::MAX, u8::MAX, u8::MAX, ta]),
            ],
            spread: GradientSpread::Pad,
        },
        opacity: COLRV1_BAKE_RIM_OPACITY,
        visible: true,
        locked: false,
    }
}

/// Bakes `contours` and `config` into 3 pure-vector `COLRv1` `CanvasColorSublayer` gradients (`SPEC-016 §5.2`).
#[must_use]
pub fn bake_liquid_glass_to_colrv1_sublayers(
    contours: &[GlyphContour],
    config: &LiquidGlassRenderConfig,
) -> Vec<CanvasColorSublayer> {
    let valid = collect_valid_baked_contours(contours);
    if valid.is_empty() {
        return Vec::new();
    }
    let cfg = config.sanitized();
    let bbox = compute_contours_bbox_upm(&valid);
    let (w, h) = (bbox.max_x - bbox.min_x, bbox.max_y - bbox.min_y);
    let [tr, tg, tb, ta] = cfg.tint_rgba;
    let light_rgba = [
        tr.saturating_add(COLRV1_BAKE_LIGHT_BONUS_U8),
        tg.saturating_add(COLRV1_BAKE_LIGHT_BONUS_U8),
        tb.saturating_add(COLRV1_BAKE_LIGHT_BONUS_U8),
        ta,
    ];
    let deep_rgba = [
        tr.saturating_sub(COLRV1_BAKE_DARK_PENALTY_U8),
        tg.saturating_sub(COLRV1_BAKE_DARK_PENALTY_U8),
        tb.saturating_sub(COLRV1_BAKE_DARK_PENALTY_U8),
        ta,
    ];
    let body_layer = CanvasColorSublayer {
        id: 1,
        name: "LiquidGlass.DomeBody".into(),
        contours: valid.clone(),
        paint: ColorLayerPaint::LinearGradient {
            p0: FontPoint::new(bbox.min_x, bbox.max_y),
            p1: FontPoint::new(bbox.max_x, bbox.min_y),
            stops: vec![
                GradientStop::new(0.0, light_rgba),
                GradientStop::new(COLRV1_BAKE_STOP_MID, cfg.tint_rgba),
                GradientStop::new(1.0, deep_rgba),
            ],
            spread: GradientSpread::Pad,
        },
        opacity: 1.0,
        visible: true,
        locked: false,
    };
    let rim_layer = build_fresnel_rim_sublayer(valid.clone(), bbox, cfg.tint_rgba, light_rgba);
    let spec_scale = (cfg.specular_intensity / 2.0).clamp(0.0, 1.0);
    let crest_alpha = (COLRV1_BAKE_CREST_ALPHA_BASE_F32 * spec_scale.max(0.5)).round() as u8;
    let crest_layer = CanvasColorSublayer {
        id: 3,
        name: "LiquidGlass.SpecularCrest".into(),
        contours: valid,
        paint: ColorLayerPaint::LinearGradient {
            p0: FontPoint::new(bbox.min_x, bbox.max_y),
            p1: FontPoint::new(
                bbox.min_x + w * COLRV1_BAKE_CREST_SPAN_FRAC,
                bbox.max_y - h * COLRV1_BAKE_CREST_SPAN_FRAC,
            ),
            stops: vec![
                GradientStop::new(0.0, [u8::MAX, u8::MAX, u8::MAX, crest_alpha]),
                GradientStop::new(
                    COLRV1_BAKE_CREST_STOP_PEAK,
                    [u8::MAX, u8::MAX, u8::MAX, crest_alpha],
                ),
                GradientStop::new(
                    COLRV1_BAKE_CREST_STOP_FADE,
                    [light_rgba[0], light_rgba[1], u8::MAX, 0],
                ),
                GradientStop::new(1.0, [light_rgba[0], light_rgba[1], u8::MAX, 0]),
            ],
            spread: GradientSpread::Pad,
        },
        opacity: COLRV1_BAKE_CREST_OPACITY,
        visible: true,
        locked: false,
    };
    vec![body_layer, rim_layer, crest_layer]
}
