//! `SPEC-013 §2.1.3 & §2.2`: Non-Linear Quadratic $B_2(t)$ & Cubic $B_3(t)$ Cross-Master Point Trajectories (`CurvedTrajectoryLink`)
//! and Measurement Rainbow Cross-Section Quads (`compute_measurement_rainbow_quads`).

use crate::core::bezier_ops::{Node, NodeId};
use crate::core::coordinates::FontPoint;
use crate::core::curve_manipulation::extract_segments;
use crate::core::glyph_model::GlyphContour;
use crate::core::math_consts::{
    RAINBOW_SAMPLES_PER_SEG, RAINBOW_THICKNESS_MAX_UPM, RAINBOW_THICKNESS_MIN_UPM,
    SOLVER_DET_EPSILON, STEM_MAX_WIDTH_UPM, STEM_MIN_EDIT_WIDTH_UPM,
};
use crate::core::stem_gauge_and_balance::measure_contour_orthogonal_stem_thickness;
use kurbo::{CubicBez, ParamCurve, Point};
use serde::{Deserialize, Serialize};

/// Non-linear quadratic ($B_2$) or cubic ($B_3$) cross-master point migration trajectory (`SPEC-013 §2.2.1`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CurvedTrajectoryLink {
    pub node_id: NodeId,
    pub companion_layer_index: usize,
    pub control1: FontPoint,
    pub control2: Option<FontPoint>,
}

/// Evaluates an intermediate master node position at $t \in [0, 1]$ along `link` (quadratic $B_2$ or cubic $B_3$),
/// falling back to linear interpolation when `link` is `None` (`SPEC-013 §2.2.1`).
#[must_use]
pub fn evaluate_curved_master_point(
    p_a: FontPoint,
    p_b: FontPoint,
    link: Option<&CurvedTrajectoryLink>,
    t: f64,
) -> FontPoint {
    debug_assert!(t.is_finite());
    let tc = if t.is_finite() {
        t.clamp(0.0, 1.0)
    } else {
        0.0
    };
    let u = 1.0 - tc;
    let Some(l) = link else {
        return FontPoint::new(u * p_a.x.0 + tc * p_b.x.0, u * p_a.y.0 + tc * p_b.y.0);
    };
    if let Some(c2) = l.control2 {
        let cubic = CubicBez::new(
            Point::new(p_a.x.0, p_a.y.0),
            Point::new(l.control1.x.0, l.control1.y.0),
            Point::new(c2.x.0, c2.y.0),
            Point::new(p_b.x.0, p_b.y.0),
        );
        let pt = cubic.eval(tc);
        FontPoint::new(pt.x, pt.y)
    } else {
        let w0 = u * u;
        let w1 = 2.0 * u * tc;
        let w2 = tc * tc;
        FontPoint::new(
            w0 * p_a.x.0 + w1 * l.control1.x.0 + w2 * p_b.x.0,
            w0 * p_a.y.0 + w1 * l.control1.y.0 + w2 * p_b.y.0,
        )
    }
}

/// Evaluates a full contour at normalized master parameter $t \in [0, 1]$ between `fg_nodes` and `bg_nodes` (`SPEC-013 §2.2`).
#[must_use]
pub fn evaluate_curved_master_contour_nodes(
    fg_nodes: &[Node],
    bg_nodes: &[Node],
    companion_layer_index: usize,
    links: &[CurvedTrajectoryLink],
    t: f64,
) -> Option<Vec<Node>> {
    if fg_nodes.len() != bg_nodes.len() {
        return None;
    }
    let mut out = Vec::with_capacity(fg_nodes.len());
    for (na, nb) in fg_nodes.iter().zip(bg_nodes.iter()) {
        let link = links
            .iter()
            .find(|l| l.node_id == na.id && l.companion_layer_index == companion_layer_index);
        let pt = evaluate_curved_master_point(na.point, nb.point, link, t);
        out.push(Node::new(na.id, pt.x.0, pt.y.0, na.node_type));
    }
    Some(out)
}

/// Computes interior cross-section quads and normalized thickness ratios $r \in [0, 1]$ for `show_measurement_rainbow` (`SPEC-013 §2.1.3`).
#[must_use]
pub fn compute_measurement_rainbow_quads(contours: &[GlyphContour]) -> Vec<([FontPoint; 4], f64)> {
    let mut raw: Vec<([FontPoint; 4], f64)> = Vec::new();
    let mut d_min = f64::INFINITY;
    let mut d_max = f64::NEG_INFINITY;
    let dominant_sign = contours
        .iter()
        .filter(|c| c.visible && c.closed && c.nodes.len() >= 3)
        .map(|c| crate::core::contour_signed_area(&c.nodes))
        .max_by(|a, b| {
            a.abs()
                .partial_cmp(&b.abs())
                .unwrap_or(std::cmp::Ordering::Equal)
        })
        .map_or(1.0, f64::signum);

    for (c_idx, contour) in contours.iter().enumerate() {
        if !contour.visible || !contour.closed || contour.nodes.len() < 3 {
            continue;
        }
        let area = crate::core::contour_signed_area(&contour.nodes);
        if area.abs() > SOLVER_DET_EPSILON && area.signum() != dominant_sign {
            continue;
        }
        let segs = extract_segments(&contour.nodes, contour.closed);
        for s_idx in 0..segs.len() {
            let n_samples = RAINBOW_SAMPLES_PER_SEG.max(1);
            for k in 0..n_samples {
                let t0 = (k as f64) / (n_samples as f64);
                let t1 = ((k + 1) as f64) / (n_samples as f64);
                let Some(m0) =
                    measure_contour_orthogonal_stem_thickness(contours, c_idx, s_idx, t0, &[], 0.0)
                else {
                    continue;
                };
                let Some(m1) =
                    measure_contour_orthogonal_stem_thickness(contours, c_idx, s_idx, t1, &[], 0.0)
                else {
                    continue;
                };
                if m0.p_exit.distance_to(m1.p_exit) > STEM_MAX_WIDTH_UPM * 0.5 {
                    continue;
                }
                let avg_d = 0.5 * (m0.thickness_upm + m1.thickness_upm);
                if avg_d <= 0.0 || avg_d > RAINBOW_THICKNESS_MAX_UPM * 2.0 {
                    continue;
                }
                d_min = d_min.min(avg_d);
                d_max = d_max.max(avg_d);
                raw.push(([m0.p_entry, m1.p_entry, m1.p_exit, m0.p_exit], avg_d));
            }
        }
    }

    let span = d_max - d_min;
    raw.into_iter()
        .map(|(quad, d)| {
            let ratio = if span > STEM_MIN_EDIT_WIDTH_UPM {
                ((d - d_min) / span).clamp(0.0, 1.0)
            } else {
                ((d - RAINBOW_THICKNESS_MIN_UPM)
                    / (RAINBOW_THICKNESS_MAX_UPM - RAINBOW_THICKNESS_MIN_UPM))
                    .clamp(0.0, 1.0)
            };
            (quad, ratio)
        })
        .collect()
}
