//! `SPEC-012 §2.3`: Live Curvature Comb Spines (`compute_contours_curvature_comb_spines`),
//! Normalized Curvature Heatmap Envelopes (`CurvatureCombStyle::HeatmapEnvelope`),
//! and Exact Cubic Inflection Point Solver (`find_cubic_inflection_points`).

use crate::core::coordinates::FontPoint;
use crate::core::curve_harmony::{cubic_derivatives, cubic_signed_curvature};
use crate::core::curve_manipulation::{extract_segments, ContourSegment};
use crate::core::glyph_model::GlyphContour;
use crate::core::math_consts::{
    BERNSTEIN_CUBIC_COEFF, CURVE_ROOT_DEDUP_EPS, QUAD_DISC_FOUR, SOLVER_DET_EPSILON,
    TRAJECTORY_INTERIOR_EPS,
};
use kurbo::{CubicBez, ParamCurve, Point};
use serde::{Deserialize, Serialize};

/// Single normal spine of a Pass-4 Curvature Comb (`SPEC-012 §2.3.1`).
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct CurvatureCombSpine {
    pub base_point: FontPoint,
    pub tip_point: FontPoint,
    pub signed_curvature: f64,
}

/// Rendering style for the Pass-4 curvature diagnostic overlay (`SPEC-012 §2.3.3`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum CurvatureCombStyle {
    #[default]
    SpinesOnly,
    HeatmapEnvelope,
}

/// Exact cubic inflection point marker where $x'(t)y''(t) - y'(t)x''(t) = 0$ for $t \in (0, 1)$ (`SPEC-012 §2.3.2`).
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct InflectionMarker {
    pub contour_idx: usize,
    pub seg_idx: usize,
    pub t: f64,
    pub point: FontPoint,
}

/// Normalized curvature heatmap sample with intensity $|\kappa(t)| / \kappa_{\max} \in [0, 1]$ and polarity sign (`SPEC-012 §2.3.3`).
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct CurvatureHeatmapSample {
    pub base_point: FontPoint,
    pub tip_point: FontPoint,
    pub signed_curvature: f64,
    pub normalized_intensity: f64,
    pub polarity_sign: i8,
}

/// Closed curvature heatmap envelope and inflection markers for a contour (`SPEC-012 §2.3.3`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CurvatureHeatmapEnvelope {
    pub contour_idx: usize,
    pub samples: Vec<CurvatureHeatmapSample>,
    pub closed_polygon: Vec<FontPoint>,
    pub inflections: Vec<InflectionMarker>,
}

/// Solves $x'(t)y''(t) - y'(t)x''(t) = 0$ for $t \in (0, 1)$ on a cubic Bézier segment (`SPEC-012 §2.3.2`).
#[must_use]
pub fn find_cubic_inflection_points(
    contour_idx: usize,
    seg_idx: usize,
    p0: FontPoint,
    p1: FontPoint,
    p2: FontPoint,
    p3: FontPoint,
) -> Vec<InflectionMarker> {
    let (ax, ay) = (p1.x.0 - p0.x.0, p1.y.0 - p0.y.0);
    let (bx, by) = (
        p2.x.0 - 2.0 * p1.x.0 + p0.x.0,
        p2.y.0 - 2.0 * p1.y.0 + p0.y.0,
    );
    let cx = p3.x.0 - BERNSTEIN_CUBIC_COEFF * p2.x.0 + BERNSTEIN_CUBIC_COEFF * p1.x.0 - p0.x.0;
    let cy = p3.y.0 - BERNSTEIN_CUBIC_COEFF * p2.y.0 + BERNSTEIN_CUBIC_COEFF * p1.y.0 - p0.y.0;
    let qa = bx * cy - by * cx;
    let qb = ax * cy - ay * cx;
    let qc = ax * by - ay * bx;

    let mut roots = Vec::new();
    if qa.abs() <= SOLVER_DET_EPSILON {
        if qb.abs() > SOLVER_DET_EPSILON {
            roots.push(-qc / qb);
        }
    } else {
        let disc = qb * qb - QUAD_DISC_FOUR * qa * qc;
        if disc >= 0.0 {
            let s = disc.sqrt();
            roots.push((-qb - s) / (2.0 * qa));
            roots.push((-qb + s) / (2.0 * qa));
        }
    }
    roots.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    let bez = CubicBez::new(
        Point::new(p0.x.0, p0.y.0),
        Point::new(p1.x.0, p1.y.0),
        Point::new(p2.x.0, p2.y.0),
        Point::new(p3.x.0, p3.y.0),
    );
    let mut out: Vec<InflectionMarker> = Vec::new();
    for t in roots {
        if t <= TRAJECTORY_INTERIOR_EPS
            || t >= 1.0 - TRAJECTORY_INTERIOR_EPS
            || out.iter().any(|m| (m.t - t).abs() <= CURVE_ROOT_DEDUP_EPS)
        {
            continue;
        }
        let k_before = cubic_signed_curvature(p0, p1, p2, p3, (t - CURVE_ROOT_DEDUP_EPS).max(0.0));
        let k_after = cubic_signed_curvature(p0, p1, p2, p3, (t + CURVE_ROOT_DEDUP_EPS).min(1.0));
        if k_before * k_after < 0.0 {
            let pt = bez.eval(t);
            out.push(InflectionMarker {
                contour_idx,
                seg_idx,
                t,
                point: FontPoint::new(pt.x, pt.y),
            });
        }
    }
    out
}

/// Computes `CurvatureCombSpine` sequences per contour (`SPEC-012 §2.3.1`).
#[must_use]
pub fn compute_contours_curvature_comb_spines(
    contours: &[GlyphContour],
    samples_per_segment: usize,
    scale_upm2: f64,
) -> Vec<Vec<CurvatureCombSpine>> {
    let n_seg = samples_per_segment.max(2);
    let denom = n_seg as f64;
    let mut out = Vec::with_capacity(contours.len());
    for contour in contours {
        let mut spines = Vec::new();
        for seg in extract_segments(&contour.nodes, contour.closed) {
            if let ContourSegment::Cubic {
                start_idx,
                h1_idx,
                h2_idx,
                end_idx,
                cubic,
            } = seg
            {
                let (p0, p1, p2, p3) = (
                    contour.nodes[start_idx].point,
                    contour.nodes[h1_idx].point,
                    contour.nodes[h2_idx].point,
                    contour.nodes[end_idx].point,
                );
                for k in 0..=n_seg {
                    let t = (k as f64) / denom;
                    let b = cubic.eval(t);
                    let base_point = FontPoint::new(b.x, b.y);
                    let ((dx, dy), _, _) = cubic_derivatives(p0, p1, p2, p3, t);
                    let speed = dx.hypot(dy);
                    let (nx, ny) = if speed > SOLVER_DET_EPSILON {
                        (-dy / speed, dx / speed)
                    } else {
                        (0.0, 0.0)
                    };
                    let signed_curvature = cubic_signed_curvature(p0, p1, p2, p3, t);
                    let disp = scale_upm2 * signed_curvature;
                    let tip_point =
                        FontPoint::new(base_point.x.0 + disp * nx, base_point.y.0 + disp * ny);
                    spines.push(CurvatureCombSpine {
                        base_point,
                        tip_point,
                        signed_curvature,
                    });
                }
            }
        }
        out.push(spines);
    }
    out
}

/// Computes all exact cubic `InflectionMarker` points across `contours` (`SPEC-012 §2.3.2`).
#[must_use]
pub fn compute_contours_inflection_markers(contours: &[GlyphContour]) -> Vec<InflectionMarker> {
    let mut out = Vec::new();
    for (c_idx, contour) in contours.iter().enumerate() {
        for (seg_idx, seg) in extract_segments(&contour.nodes, contour.closed)
            .into_iter()
            .enumerate()
        {
            if let ContourSegment::Cubic {
                start_idx,
                h1_idx,
                h2_idx,
                end_idx,
                ..
            } = seg
            {
                out.extend(find_cubic_inflection_points(
                    c_idx,
                    seg_idx,
                    contour.nodes[start_idx].point,
                    contour.nodes[h1_idx].point,
                    contour.nodes[h2_idx].point,
                    contour.nodes[end_idx].point,
                ));
            }
        }
    }
    out
}

/// Computes `CurvatureHeatmapEnvelope` polygons and inflection markers per contour (`SPEC-012 §2.3.3`).
#[must_use]
pub fn compute_contours_curvature_heatmap_envelopes(
    contours: &[GlyphContour],
    samples_per_segment: usize,
    scale_upm2: f64,
) -> Vec<CurvatureHeatmapEnvelope> {
    let all_spines =
        compute_contours_curvature_comb_spines(contours, samples_per_segment, scale_upm2);
    let mut out = Vec::with_capacity(contours.len());
    for (contour_idx, (contour, spines)) in contours.iter().zip(all_spines).enumerate() {
        let kappa_max = spines
            .iter()
            .map(|s| s.signed_curvature.abs())
            .fold(0.0_f64, f64::max);
        let mut samples = Vec::with_capacity(spines.len());
        for s in &spines {
            let normalized_intensity = if kappa_max > SOLVER_DET_EPSILON {
                (s.signed_curvature.abs() / kappa_max).clamp(0.0, 1.0)
            } else {
                0.0
            };
            let polarity_sign = if s.signed_curvature > SOLVER_DET_EPSILON {
                1
            } else if s.signed_curvature < -SOLVER_DET_EPSILON {
                -1
            } else {
                0
            };
            samples.push(CurvatureHeatmapSample {
                base_point: s.base_point,
                tip_point: s.tip_point,
                signed_curvature: s.signed_curvature,
                normalized_intensity,
                polarity_sign,
            });
        }
        let mut closed_polygon = Vec::with_capacity(spines.len() * 2);
        closed_polygon.extend(spines.iter().map(|s| s.base_point));
        closed_polygon.extend(spines.iter().rev().map(|s| s.tip_point));
        let inflections = compute_contours_inflection_markers(std::slice::from_ref(contour))
            .into_iter()
            .map(|mut m| {
                m.contour_idx = contour_idx;
                m
            })
            .collect();
        out.push(CurvatureHeatmapEnvelope {
            contour_idx,
            samples,
            closed_polygon,
            inflections,
        });
    }
    out
}
