//! Kinetic Effect 10 (`MagicReplace` & `Replace` Content Transition with De Casteljau Node Harmonization
//! & `vitrea::tessellate_meniscus_bridge` Liquid Meniscus Bridge — `SPEC-016 §4.2` #10).

use crate::core::bezier_ops::{Node, NodeId, NodeType};
use crate::core::color_layers::CanvasColorSublayer;
use crate::core::coordinates::FontPoint;
use crate::core::glyph_model::{ContourId, GlyphContour};
use crate::core::icon_studio::{
    KineticLayerScope, KineticTimelineState, ReplaceFallback, SymbolLayerTag,
};
use crate::core::kinetic_draw_trim::{contour_total_arc_length, trim_contour_by_arc_ratio};
use crate::core::kinetic_symbols::{
    apply_layer_kinetic_transform, compute_layer_local_phase, resolve_layer_pivot_upm,
    LayerKineticTransform,
};
use crate::core::math_consts::{
    CUBIC_ONE_THIRD, CUBIC_TWO_THIRDS, FIT_CURVE_MIN_CHORD_LEN_EPS, ICON_STUDIO_MAX_COORD_ABS_UPM,
    KINETIC_APPEAR_BASE_SCALE, KINETIC_APPEAR_DELTA_SCALE, KINETIC_DISAPPEAR_DELTA_SCALE,
    KINETIC_SMOOTHSTEP_C2, LIQUID_GLASS_DEFAULT_DOME_RADIUS_UPM, LIQUID_GLASS_DEFAULT_SMOOTH_K_UPM,
    MAGIC_REPLACE_CUBIC_SEGMENTS, MENISCUS_BASE_NODE_ID, MENISCUS_CORNER_R_FRAC,
    MENISCUS_HALF_SWEEP_DEG, MENISCUS_HULL_STEPS, MENISCUS_LIFT_GAIN, MENISCUS_MAX_PAIR_DIST_UPM,
    MENISCUS_MAX_PHASE, MENISCUS_MIN_ALPHA, MENISCUS_MIN_PHASE, MENISCUS_R_IN_FRAC,
    MENISCUS_R_IN_MIN_UPM, MENISCUS_R_OUT_FRAC, MENISCUS_R_OUT_MIN_UPM, MENISCUS_SECTOR_DEG,
};
use crate::core::selection_info::CanvasAnchor;
use crate::core::stem_gauge_and_balance::compute_contour_area_and_centroid;

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

fn sanitize_contour(c: &GlyphContour) -> GlyphContour {
    let nodes = c
        .nodes
        .iter()
        .map(|n| {
            Node::new(
                n.id,
                sanitize_coord(n.point.x.0),
                sanitize_coord(n.point.y.0),
                n.node_type,
            )
        })
        .collect();
    let mut out = GlyphContour::new(c.id, nodes);
    out.closed = c.closed;
    out.visible = c.visible;
    out
}

fn append_harmonized_cubic_segment(
    nodes: &mut Vec<Node>,
    next_nid: &mut u64,
    sub: &GlyphContour,
    include_p0: bool,
    include_p3: bool,
) {
    let p0 = sub
        .nodes
        .first()
        .map_or(FontPoint::new(0.0, 0.0), |n| n.point);
    let p3 = sub.nodes.last().map_or(p0, |n| n.point);
    let (p1, p2) = if sub.nodes.len() == 4
        && sub.nodes[1].node_type == NodeType::OffCurve
        && sub.nodes[2].node_type == NodeType::OffCurve
    {
        (sub.nodes[1].point, sub.nodes[2].point)
    } else {
        let (dx, dy) = (p3.x.0 - p0.x.0, p3.y.0 - p0.y.0);
        (
            FontPoint::new(p0.x.0 + dx * CUBIC_ONE_THIRD, p0.y.0 + dy * CUBIC_ONE_THIRD),
            FontPoint::new(
                p0.x.0 + dx * CUBIC_TWO_THIRDS,
                p0.y.0 + dy * CUBIC_TWO_THIRDS,
            ),
        )
    };
    let mut push = |pt: FontPoint, nt: NodeType| {
        nodes.push(Node::new(NodeId(*next_nid), pt.x.0, pt.y.0, nt));
        *next_nid += 1;
    };
    if include_p0 {
        push(p0, NodeType::Corner);
    }
    push(p1, NodeType::OffCurve);
    push(p2, NodeType::OffCurve);
    if include_p3 {
        push(p3, NodeType::Corner);
    }
}

/// Resamples `contour` into a harmonized cubic Bézier chain with `seg_count` segments (`SPEC-016 §4.2` #10).
#[must_use]
pub fn harmonize_contour_cubic_segments(
    contour: &GlyphContour,
    seg_count: usize,
    force_closed: bool,
) -> GlyphContour {
    let clean = sanitize_contour(contour);
    let m = seg_count.max(3);
    if clean.nodes.len() < 2 || contour_total_arc_length(&clean) <= FIT_CURVE_MIN_CHORD_LEN_EPS {
        let p = clean
            .nodes
            .first()
            .map_or(FontPoint::new(0.0, 0.0), |n| n.point);
        let total = if force_closed { 3 * m } else { 3 * m + 1 };
        let nodes = (0..total)
            .map(|i| {
                let nt = if i % 3 == 0 {
                    NodeType::Corner
                } else {
                    NodeType::OffCurve
                };
                Node::new(NodeId((i as u64) + 1), p.x.0, p.y.0, nt)
            })
            .collect();
        let mut deg = GlyphContour::new(clean.id, nodes);
        deg.closed = force_closed;
        deg.visible = clean.visible;
        return deg;
    }
    let mut nodes = Vec::with_capacity(3 * m + 1);
    let mut next_nid = 1_u64;
    for k in 0..m {
        let (r0, r1) = ((k as f64) / (m as f64), ((k + 1) as f64) / (m as f64));
        let sub = trim_contour_by_arc_ratio(&clean, r0, r1, false);
        append_harmonized_cubic_segment(
            &mut nodes,
            &mut next_nid,
            &sub,
            k == 0 || force_closed,
            !force_closed,
        );
    }
    let mut out = GlyphContour::new(clean.id, nodes);
    out.closed = force_closed;
    out.visible = clean.visible;
    out
}

/// Smoothly morphs `src` into `dst` at local phase `tau` in `[0.0, 1.0]` (`SPEC-016 §4.2` #10).
#[must_use]
pub fn interpolate_magic_replace_contour_pair(
    src: &GlyphContour,
    dst: &GlyphContour,
    tau: f64,
    out_id: ContourId,
) -> GlyphContour {
    let t = if tau.is_finite() {
        tau.clamp(0.0, 1.0)
    } else {
        0.0
    };
    if t <= FIT_CURVE_MIN_CHORD_LEN_EPS {
        let mut c = sanitize_contour(src);
        c.id = out_id;
        return c;
    }
    if t >= 1.0 - FIT_CURVE_MIN_CHORD_LEN_EPS {
        let mut c = sanitize_contour(dst);
        c.id = out_id;
        return c;
    }
    let s = t * t * (KINETIC_SMOOTHSTEP_C2 - 2.0 * t);
    let closed = src.closed && dst.closed;
    let h_src = harmonize_contour_cubic_segments(src, MAGIC_REPLACE_CUBIC_SEGMENTS, closed);
    let h_dst = harmonize_contour_cubic_segments(dst, MAGIC_REPLACE_CUBIC_SEGMENTS, closed);
    let len = h_src.nodes.len().min(h_dst.nodes.len());
    let nodes = (0..len)
        .map(|i| {
            let (a, b) = (&h_src.nodes[i], &h_dst.nodes[i]);
            let x = (1.0 - s) * a.point.x.0 + s * b.point.x.0;
            let y = (1.0 - s) * a.point.y.0 + s * b.point.y.0;
            Node::new(a.id, sanitize_coord(x), sanitize_coord(y), a.node_type)
        })
        .collect();
    let mut out = GlyphContour::new(out_id, nodes);
    out.closed = closed;
    out.visible = src.visible || dst.visible;
    out
}

fn evaluate_unmatched_fallback_transform(
    fallback: ReplaceFallback,
    is_source: bool,
    tau: f64,
    pivot: FontPoint,
) -> (LayerKineticTransform, bool) {
    let t = if tau.is_finite() {
        tau.clamp(0.0, 1.0)
    } else {
        0.0
    };
    let mut tf = LayerKineticTransform {
        pivot_upm: pivot,
        ..Default::default()
    };
    let half_cut =
        fallback != ReplaceFallback::OffUp && ((is_source && t >= 0.5) || (!is_source && t <= 0.5));
    if half_cut {
        tf.opacity_factor = 0.0;
        return (tf, false);
    }
    let u = if fallback == ReplaceFallback::OffUp {
        t
    } else if is_source {
        (t * 2.0).clamp(0.0, 1.0)
    } else {
        ((t - 0.5) * 2.0).clamp(0.0, 1.0)
    };
    let s = u * u * (KINETIC_SMOOTHSTEP_C2 - 2.0 * u);
    let (alpha, scale) = if is_source {
        let sc = match fallback {
            ReplaceFallback::DownUp => 1.0 - KINETIC_DISAPPEAR_DELTA_SCALE * s,
            ReplaceFallback::UpUp => 1.0 + KINETIC_DISAPPEAR_DELTA_SCALE * s,
            ReplaceFallback::OffUp => 1.0,
        };
        (((1.0 - s) as f32).clamp(0.0, 1.0), sc)
    } else {
        (
            (s as f32).clamp(0.0, 1.0),
            KINETIC_APPEAR_BASE_SCALE + KINETIC_APPEAR_DELTA_SCALE * s,
        )
    };
    tf.opacity_factor = alpha;
    tf.scale = (scale, scale);
    (tf, alpha > 0.0)
}

fn build_meniscus_bridge_contour(
    c0: FontPoint,
    c1: FontPoint,
    alpha_pair: (f32, f32),
    tau: f64,
    out_id: ContourId,
) -> Option<(GlyphContour, f32)> {
    if !(MENISCUS_MIN_PHASE..=MENISCUS_MAX_PHASE).contains(&tau)
        || !c0.x.0.is_finite()
        || !c0.y.0.is_finite()
        || !c1.x.0.is_finite()
        || !c1.y.0.is_finite()
    {
        return None;
    }
    let dist = c0.distance_to(c1) as f32;
    if !(2.0..=MENISCUS_MAX_PAIR_DIST_UPM).contains(&dist) {
        return None;
    }
    let mid = vitrea::GlamVec2::new(
        ((c0.x.0 + c1.x.0) * 0.5) as f32,
        ((c0.y.0 + c1.y.0) * 0.5) as f32,
    );
    let lift = (MENISCUS_LIFT_GAIN * tau * (1.0 - tau)).clamp(0.0, 1.0) as f32;
    let mesh = vitrea::tessellate_meniscus_bridge(
        mid,
        (
            (dist * MENISCUS_R_IN_FRAC).max(MENISCUS_R_IN_MIN_UPM),
            (dist * MENISCUS_R_OUT_FRAC).max(MENISCUS_R_OUT_MIN_UPM),
            (LIQUID_GLASS_DEFAULT_DOME_RADIUS_UPM as f32) * MENISCUS_CORNER_R_FRAC,
            LIQUID_GLASS_DEFAULT_SMOOTH_K_UPM as f32,
        ),
        (-MENISCUS_SECTOR_DEG, MENISCUS_HALF_SWEEP_DEG, lift),
        (MENISCUS_SECTOR_DEG, MENISCUS_HALF_SWEEP_DEG, lift),
    );
    if mesh.positions.len() <= 3 {
        return None;
    }
    let ring = &mesh.positions[1..];
    let stride = (ring.len() / MENISCUS_HULL_STEPS).max(1);
    let nodes: Vec<Node> = ring
        .iter()
        .step_by(stride)
        .enumerate()
        .map(|(i, p)| {
            Node::new(
                NodeId(MENISCUS_BASE_NODE_ID + (i as u64)),
                sanitize_coord(f64::from(p.x)),
                sanitize_coord(f64::from(p.y)),
                NodeType::Corner,
            )
        })
        .collect();
    (nodes.len() >= 3).then(|| {
        let alpha = ((alpha_pair.0 + alpha_pair.1) * 0.5 * lift).clamp(MENISCUS_MIN_ALPHA, 1.0);
        (GlyphContour::new(out_id, nodes), alpha)
    })
}

/// Evaluates `KineticSymbolEffect::MagicReplace` across `contours` and `target_contours` (`SPEC-016 §4.2` #10).
#[must_use]
pub fn evaluate_magic_replace_glyph_contours(
    contours: &[GlyphContour],
    target_contours: &[GlyphContour],
    fallback: ReplaceFallback,
    scope: KineticLayerScope,
    tags: &[SymbolLayerTag],
    anchors: &[CanvasAnchor],
    global_t: f64,
) -> Vec<(GlyphContour, f32)> {
    let max_len = contours.len().max(target_contours.len());
    if max_len == 0 {
        return Vec::new();
    }
    let t = if global_t.is_finite() {
        global_t.clamp(0.0, 1.0)
    } else {
        0.0
    };
    let mut out: Vec<(GlyphContour, f32)> = Vec::with_capacity(max_len + 1);
    for idx in 0..max_len {
        let tau = compute_layer_local_phase(t, scope, idx, max_len);
        match (contours.get(idx), target_contours.get(idx)) {
            (Some(src), Some(dst)) => {
                out.push((
                    interpolate_magic_replace_contour_pair(src, dst, tau, ContourId(out.len())),
                    1.0,
                ));
            }
            (Some(src), None) => {
                let piv =
                    resolve_layer_pivot_upm(std::slice::from_ref(src), tags.get(idx), anchors);
                let (tf, vis) = evaluate_unmatched_fallback_transform(fallback, true, tau, piv);
                if vis || t <= FIT_CURVE_MIN_CHORD_LEN_EPS {
                    let mut c = apply_layer_kinetic_transform(src, &tf);
                    c.id = ContourId(out.len());
                    out.push((c, tf.opacity_factor));
                }
            }
            (None, Some(dst)) => {
                let piv =
                    resolve_layer_pivot_upm(std::slice::from_ref(dst), tags.get(idx), anchors);
                let (tf, vis) = evaluate_unmatched_fallback_transform(fallback, false, tau, piv);
                if vis || t >= 1.0 - FIT_CURVE_MIN_CHORD_LEN_EPS {
                    let mut c = apply_layer_kinetic_transform(dst, &tf);
                    c.id = ContourId(out.len());
                    out.push((c, tf.opacity_factor));
                }
            }
            (None, None) => {}
        }
    }
    if let Some((first, second)) = out.first().zip(
        out.get(1)
            .map(|(c, a)| (&c.nodes, *a))
            .or_else(|| target_contours.get(1).map(|c| (&c.nodes, 1.0))),
    ) {
        let (_, c0) = compute_contour_area_and_centroid(&first.0.nodes);
        let (_, c1) = compute_contour_area_and_centroid(second.0);
        if let Some(b) =
            build_meniscus_bridge_contour(c0, c1, (first.1, second.1), t, ContourId(out.len()))
        {
            out.push(b);
        }
    }
    out
}

/// Evaluates `KineticSymbolEffect::MagicReplace` across `CanvasColorSublayer`s (`SPEC-016 §4.2` #10).
#[must_use]
pub fn evaluate_magic_replace_color_sublayers(
    sublayers: &[CanvasColorSublayer],
    target_contours: &[GlyphContour],
    fallback: ReplaceFallback,
    scope: KineticLayerScope,
    tags: &[SymbolLayerTag],
    anchors: &[CanvasAnchor],
    kinetic: &KineticTimelineState,
) -> Vec<CanvasColorSublayer> {
    let total = sublayers.len().max(1);
    sublayers
        .iter()
        .enumerate()
        .map(|(i, sub)| {
            let tau = compute_layer_local_phase(kinetic.phase_t, scope, i, total);
            let mut eval_sub = sub.clone();
            if let (Some(src), Some(dst)) = (sub.contours.first(), target_contours.get(i)) {
                eval_sub.contours = vec![interpolate_magic_replace_contour_pair(
                    src,
                    dst,
                    tau,
                    ContourId(0),
                )];
            } else {
                let piv = resolve_layer_pivot_upm(&sub.contours, tags.get(i), anchors);
                let (tf, _) = evaluate_unmatched_fallback_transform(fallback, true, tau, piv);
                eval_sub.contours = sub
                    .contours
                    .iter()
                    .map(|c| apply_layer_kinetic_transform(c, &tf))
                    .collect();
                eval_sub.opacity = (sub.opacity * tf.opacity_factor).clamp(0.0, 1.0);
            }
            eval_sub
        })
        .collect()
}
