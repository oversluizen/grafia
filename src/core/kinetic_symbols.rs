//! 10-Mode Kinetic Symbol Engine (`SPEC-016 §4.1–§4.2` & `§2.4.2`).

use crate::core::bezier_ops::Node;
use crate::core::coordinates::FontPoint;
use crate::core::glyph_model::GlyphContour;
use crate::core::icon_studio::{
    BounceDirection, KineticLayerScope, KineticSymbolEffect, ScaleDirection, SymbolLayerTag,
    WiggleAxis,
};
use crate::core::kinetic_draw_trim::trim_contour_by_arc_ratio;
use crate::core::math_consts::{
    DEG_TO_RAD_F64, FIT_CURVE_MIN_CHORD_LEN_EPS, ICON_STUDIO_MAX_COORD_ABS_UPM,
    KINETIC_APPEAR_BASE_SCALE, KINETIC_APPEAR_DELTA_SCALE, KINETIC_BOUNCE_DAMPING,
    KINETIC_BOUNCE_TRANSLATION_UPM, KINETIC_BREATHE_PULSE_AMP, KINETIC_BREATHE_SCALE_AMP,
    KINETIC_BY_LAYER_STAGGER_FRAC, KINETIC_DISAPPEAR_DELTA_SCALE, KINETIC_PULSE_AMPLITUDE,
    KINETIC_SCALE_AMPLITUDE, KINETIC_SMOOTHSTEP_C2, KINETIC_VAR_COLOR_DIM_ALPHA_FACTOR,
    KINETIC_WIGGLE_CYCLES_RAD, KINETIC_WIGGLE_ROT_MAX_DEG, KINETIC_WIGGLE_TRANS_MAX_UPM,
    LIQUID_GLASS_DEFAULT_DOME_RADIUS_UPM, PI_F64, TAU_F64,
};
use crate::core::selection_info::CanvasAnchor;
use crate::core::stem_gauge_and_balance::compute_contour_area_and_centroid;

/// Unified per-layer kinetic transformation kernel evaluated for Effects 1–9 (`SPEC-016 §2.4.2`).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LayerKineticTransform {
    pub pivot_upm: FontPoint,
    pub translation_upm: FontPoint,
    pub rotation_rad: f64,
    pub scale: (f64, f64),
    pub opacity_factor: f32,
    pub trim_arc_range: Option<(f64, f64)>,
    pub trim_reversed: bool,
    pub viscous_velocity_y: f32,
}

impl Default for LayerKineticTransform {
    fn default() -> Self {
        Self {
            pivot_upm: FontPoint::new(0.0, 0.0),
            translation_upm: FontPoint::new(0.0, 0.0),
            rotation_rad: 0.0,
            scale: (1.0, 1.0),
            opacity_factor: 1.0,
            trim_arc_range: None,
            trim_reversed: false,
            viscous_velocity_y: 0.0,
        }
    }
}

fn sanitize_coord(v: f64) -> f64 {
    if v.is_finite() {
        v.clamp(
            -ICON_STUDIO_MAX_COORD_ABS_UPM,
            ICON_STUDIO_MAX_COORD_ABS_UPM,
        )
    } else {
        0.0
    }
}

fn sanitize_pt(pt: FontPoint) -> FontPoint {
    FontPoint::new(sanitize_coord(pt.x.0), sanitize_coord(pt.y.0))
}

/// Computes local layer phase $\tau_g(t) \in [0.0, 1.0]$ for layer group `group_idx` out of `total_groups` (`SPEC-016 §4.2`).
#[must_use]
pub fn compute_layer_local_phase(
    global_t: f64,
    scope: KineticLayerScope,
    group_idx: usize,
    total_groups: usize,
) -> f64 {
    let t = if global_t.is_finite() { global_t } else { 0.0 }.clamp(0.0, 1.0);
    if scope == KineticLayerScope::WholeSymbol || total_groups <= 1 {
        return t;
    }
    let g = group_idx.min(total_groups - 1) as f64;
    let denom_g = (total_groups - 1).max(1) as f64;
    let offset = g * (KINETIC_BY_LAYER_STAGGER_FRAC / denom_g);
    let win = (1.0 - KINETIC_BY_LAYER_STAGGER_FRAC).max(FIT_CURVE_MIN_CHORD_LEN_EPS);
    ((t - offset) / win).clamp(0.0, 1.0)
}

/// Resolves the effective pivot $\mathbf{c}_g$ for a layer (`SPEC-016 §4.2`).
#[must_use]
pub fn resolve_layer_pivot_upm(
    contours: &[GlyphContour],
    tag: Option<&SymbolLayerTag>,
    anchors: &[CanvasAnchor],
) -> FontPoint {
    if let Some(tg) = tag {
        if let Some(aid) = tg.motion_anchor_id {
            if let Some(a) = anchors.iter().find(|a| a.id == aid) {
                return sanitize_pt(a.position);
            }
        }
        if let Some(cp) = tg.custom_pivot_upm {
            return sanitize_pt(cp);
        }
    }
    let (mut sum_w, mut sum_x, mut sum_y) = (0.0_f64, 0.0_f64, 0.0_f64);
    for c in contours {
        let (area, cent) = compute_contour_area_and_centroid(&c.nodes);
        let w = area.abs();
        if w > FIT_CURVE_MIN_CHORD_LEN_EPS && cent.x.0.is_finite() && cent.y.0.is_finite() {
            sum_w += w;
            sum_x += w * cent.x.0;
            sum_y += w * cent.y.0;
        }
    }
    if sum_w > FIT_CURVE_MIN_CHORD_LEN_EPS {
        return sanitize_pt(FontPoint::new(sum_x / sum_w, sum_y / sum_w));
    }
    let (mut count, mut nx, mut ny) = (0usize, 0.0_f64, 0.0_f64);
    for n in contours.iter().flat_map(|c| &c.nodes) {
        if n.point.x.0.is_finite() && n.point.y.0.is_finite() {
            nx += n.point.x.0;
            ny += n.point.y.0;
            count += 1;
        }
    }
    if count > 0 {
        sanitize_pt(FontPoint::new(nx / (count as f64), ny / (count as f64)))
    } else {
        FontPoint::new(0.0, 0.0)
    }
}

fn eval_variable_color_opacity(
    t: f64,
    cumulative: bool,
    reversing: bool,
    dim_inactive: bool,
    rank: (usize, usize),
    tag: Option<&SymbolLayerTag>,
) -> f32 {
    if tag.is_some_and(|tg| !tg.variable_color_enabled) {
        return 1.0;
    }
    let inactive = if dim_inactive {
        KINETIC_VAR_COLOR_DIM_ALPHA_FACTOR
    } else {
        0.0
    };
    let p = if reversing {
        1.0 - (2.0 * t - 1.0).abs()
    } else {
        t
    };
    let (j, m) = (rank.0, rank.1.max(1));
    if cumulative {
        return if j <= ((p * (m as f64)).floor() as usize).min(m - 1) {
            1.0
        } else {
            inactive
        };
    }
    if m <= 1 {
        return 1.0;
    }
    let slot_w = 1.0 / (m as f64);
    let center_j = ((j.min(m - 1) as f64) + 0.5) * slot_w;
    let u = ((p - center_j).abs() / slot_w).clamp(0.0, 1.0);
    let w = 0.5 * (1.0 + (PI_F64 * u).cos());
    (inactive + (1.0 - inactive) * (w as f32)).clamp(0.0, 1.0)
}

fn apply_wiggle_transform(tf: &mut LayerKineticTransform, axis: WiggleAxis, tau: f64) {
    let w = (KINETIC_WIGGLE_CYCLES_RAD * tau).sin() * (-2.0 * tau).exp();
    match axis {
        WiggleAxis::Clockwise => {
            tf.rotation_rad = -(KINETIC_WIGGLE_ROT_MAX_DEG * DEG_TO_RAD_F64) * w;
        }
        WiggleAxis::CounterClockwise => {
            tf.rotation_rad = (KINETIC_WIGGLE_ROT_MAX_DEG * DEG_TO_RAD_F64) * w;
        }
        WiggleAxis::Left | WiggleAxis::Backward => {
            tf.translation_upm = FontPoint::new(-KINETIC_WIGGLE_TRANS_MAX_UPM * w, 0.0);
        }
        WiggleAxis::Right | WiggleAxis::Forward => {
            tf.translation_upm = FontPoint::new(KINETIC_WIGGLE_TRANS_MAX_UPM * w, 0.0);
        }
        WiggleAxis::Up => {
            tf.translation_upm = FontPoint::new(0.0, KINETIC_WIGGLE_TRANS_MAX_UPM * w);
        }
        WiggleAxis::Down => {
            tf.translation_upm = FontPoint::new(0.0, -KINETIC_WIGGLE_TRANS_MAX_UPM * w);
        }
        WiggleAxis::CustomAngle(deg) => {
            let safe_deg = if deg.is_finite() { deg } else { 0.0 };
            let (sin_p, cos_p) = (safe_deg * DEG_TO_RAD_F64).sin_cos();
            let disp = KINETIC_WIGGLE_TRANS_MAX_UPM * w;
            tf.translation_upm = FontPoint::new(disp * cos_p, disp * sin_p);
        }
    }
}

fn apply_wiggle_rotate_or_breathe(
    tf: &mut LayerKineticTransform,
    effect: &KineticSymbolEffect,
    tau: f64,
) {
    match *effect {
        KineticSymbolEffect::Wiggle { axis, .. } => apply_wiggle_transform(tf, axis, tau),
        KineticSymbolEffect::Rotate { clockwise, .. } => {
            let s = tau * tau * (KINETIC_SMOOTHSTEP_C2 - 2.0 * tau);
            let sign = if clockwise { -1.0 } else { 1.0 };
            tf.rotation_rad = sign * TAU_F64 * s;
        }
        KineticSymbolEffect::Breathe { pulse_opacity, .. } => {
            let wave = 0.5 * (1.0 - (TAU_F64 * tau).cos());
            let factor = 1.0 + KINETIC_BREATHE_SCALE_AMP * wave;
            tf.scale = (factor, factor);
            if pulse_opacity {
                tf.opacity_factor =
                    (1.0 - KINETIC_BREATHE_PULSE_AMP * (wave as f32)).clamp(0.0, 1.0);
            }
        }
        _ => {}
    }
}

fn apply_appear_or_disappear_transform(
    tf: &mut LayerKineticTransform,
    tau: f64,
    direction: BounceDirection,
    is_appear: bool,
    draw_trim: bool,
    tag: Option<&SymbolLayerTag>,
) {
    let s = tau * tau * (KINETIC_SMOOTHSTEP_C2 - 2.0 * tau);
    let draw_dir = tag.map_or(crate::core::DrawDirection::StartToEnd, |tg| {
        if tg.draw_direction == crate::core::DrawDirection::StartToEnd && tg.draw_order_reversed {
            crate::core::DrawDirection::EndToStart
        } else {
            tg.draw_direction
        }
    });
    let up = direction == BounceDirection::Up;
    let (alpha, factor, range) = if is_appear {
        let f = if up {
            KINETIC_APPEAR_BASE_SCALE + KINETIC_APPEAR_DELTA_SCALE * s
        } else {
            1.0 + KINETIC_APPEAR_DELTA_SCALE * (1.0 - s)
        };
        (s as f32, f, draw_dir.appear_arc_range(s))
    } else {
        let f = 1.0 + (if up { 1.0 } else { -1.0 }) * KINETIC_DISAPPEAR_DELTA_SCALE * s;
        ((1.0 - s) as f32, f, draw_dir.disappear_arc_range(s))
    };
    tf.opacity_factor = alpha.clamp(0.0, 1.0);
    tf.scale = (factor, factor);
    tf.trim_arc_range = draw_trim.then_some(range);
}

fn apply_bounce_transform(tf: &mut LayerKineticTransform, direction: BounceDirection, tau: f64) {
    let dir = if direction == BounceDirection::Up {
        1.0
    } else {
        -1.0
    };
    let decay = (-KINETIC_BOUNCE_DAMPING * tau).exp();
    let ang = TAU_F64 * tau;
    let u = dir * ang.sin() * decay;
    let du = dir * decay * (TAU_F64 * ang.cos() - KINETIC_BOUNCE_DAMPING * ang.sin());
    tf.translation_upm = FontPoint::new(0.0, KINETIC_BOUNCE_TRANSLATION_UPM * u);
    tf.viscous_velocity_y = (du * KINETIC_BOUNCE_TRANSLATION_UPM) as f32;
}

/// Evaluates the closed-form `LayerKineticTransform` for `effect` at global phase `global_t` (`SPEC-016 §4.2`).
#[must_use]
pub fn evaluate_layer_kinetic_transform(
    effect: &KineticSymbolEffect,
    global_t: f64,
    group_idx: usize,
    total_groups: usize,
    var_color_rank: Option<(usize, usize)>,
    tag: Option<&SymbolLayerTag>,
    pivot_upm: FontPoint,
) -> LayerKineticTransform {
    let mut tf = LayerKineticTransform {
        pivot_upm: sanitize_pt(pivot_upm),
        ..Default::default()
    };
    let t = if global_t.is_finite() {
        global_t.clamp(0.0, 1.0)
    } else {
        0.0
    };
    match *effect {
        KineticSymbolEffect::Pulse { scope } => {
            if scope == KineticLayerScope::WholeSymbol || tag.is_none_or(|tg| tg.pulse_participates)
            {
                let tau = compute_layer_local_phase(t, scope, group_idx, total_groups);
                let s = (PI_F64 * tau).sin();
                tf.opacity_factor =
                    (1.0 - KINETIC_PULSE_AMPLITUDE * ((s * s) as f32)).clamp(0.0, 1.0);
            }
        }
        KineticSymbolEffect::Bounce { direction, scope } => {
            let tau = compute_layer_local_phase(t, scope, group_idx, total_groups);
            apply_bounce_transform(&mut tf, direction, tau);
        }
        KineticSymbolEffect::VariableColor {
            cumulative,
            reversing,
            dim_inactive,
        } => {
            let rank = var_color_rank.unwrap_or((group_idx, total_groups.max(1)));
            tf.opacity_factor =
                eval_variable_color_opacity(t, cumulative, reversing, dim_inactive, rank, tag);
        }
        KineticSymbolEffect::Scale { direction, scope } => {
            let tau = compute_layer_local_phase(t, scope, group_idx, total_groups);
            let sign = if direction == ScaleDirection::Up {
                1.0
            } else {
                -1.0
            };
            let s = (PI_F64 * tau).sin();
            let factor = 1.0 + sign * KINETIC_SCALE_AMPLITUDE * s * s;
            tf.scale = (factor, factor);
        }
        KineticSymbolEffect::Appear {
            direction,
            draw_on: trim,
            scope,
        }
        | KineticSymbolEffect::Disappear {
            direction,
            draw_off: trim,
            scope,
        } => {
            let is_app = matches!(effect, KineticSymbolEffect::Appear { .. });
            let tau = compute_layer_local_phase(t, scope, group_idx, total_groups);
            apply_appear_or_disappear_transform(&mut tf, tau, direction, is_app, trim, tag);
        }
        KineticSymbolEffect::Wiggle { scope, .. }
        | KineticSymbolEffect::Rotate { scope, .. }
        | KineticSymbolEffect::Breathe { scope, .. } => {
            let tau = compute_layer_local_phase(t, scope, group_idx, total_groups);
            apply_wiggle_rotate_or_breathe(&mut tf, effect, tau);
        }
        _ => {}
    }
    tf
}

/// Applies `LayerKineticTransform` (including `vitrea::deform_viscous_droplet_point` $\det J = 1.0$) to `contour` (`SPEC-016 §2.4.2`).
#[must_use]
pub fn apply_layer_kinetic_transform(
    contour: &GlyphContour,
    tf: &LayerKineticTransform,
) -> GlyphContour {
    let trimmed_storage;
    let src = if let Some((r0, r1)) = tf.trim_arc_range {
        trimmed_storage = trim_contour_by_arc_ratio(contour, r0, r1, tf.trim_reversed);
        &trimmed_storage
    } else {
        contour
    };
    let (pivot, trans) = (sanitize_pt(tf.pivot_upm), sanitize_pt(tf.translation_upm));
    let (sin_r, cos_r) = if tf.rotation_rad.is_finite() {
        tf.rotation_rad.sin_cos()
    } else {
        (0.0, 1.0)
    };
    let (sx, sy) = (
        if tf.scale.0.is_finite() {
            tf.scale.0
        } else {
            1.0
        },
        if tf.scale.1.is_finite() {
            tf.scale.1
        } else {
            1.0
        },
    );
    let has_visc = tf.viscous_velocity_y.is_finite() && tf.viscous_velocity_y.abs() > 0.0;
    let center_v = vitrea::GlamVec2::new(pivot.x.0 as f32, pivot.y.0 as f32);
    let vel_v = vitrea::GlamVec2::new(0.0, tf.viscous_velocity_y);
    let (r_ref, gains) = (
        LIQUID_GLASS_DEFAULT_DOME_RADIUS_UPM as f32,
        (KINETIC_SCALE_AMPLITUDE as f32, 0.0),
    );

    let nodes: Vec<Node> = src
        .nodes
        .iter()
        .map(|n| {
            let sp = sanitize_pt(n.point);
            let (vx, vy) = if has_visc {
                let pt_v = vitrea::GlamVec2::new(sp.x.0 as f32, sp.y.0 as f32);
                let d =
                    vitrea::deform_viscous_droplet_point(pt_v, center_v, vel_v, r_ref, 0.0, gains);
                (f64::from(d.x), f64::from(d.y))
            } else {
                (sp.x.0, sp.y.0)
            };
            let (dx, dy) = ((vx - pivot.x.0) * sx, (vy - pivot.y.0) * sy);
            let rx = pivot.x.0 + dx * cos_r - dy * sin_r + trans.x.0;
            let ry = pivot.y.0 + dx * sin_r + dy * cos_r + trans.y.0;
            Node::new(n.id, rx, ry, n.node_type)
        })
        .collect();
    let mut out = GlyphContour::new(src.id, nodes);
    out.closed = src.closed;
    out.visible = src.visible;
    out
}
