//! `SPEC-011 §2.2 & §2.9`: Interactive Tangent Balance (Tunni) Lines (`[0.20, 0.95]`)
//! and Live 120 fps `CurvatureComb` (`Krommingskam`) Evaluation.

use crate::core::bezier_ops::NodeId;
use crate::core::coordinates::FontPoint;
use crate::core::curve_harmony::{cubic_derivatives, cubic_dkappa_ds, cubic_signed_curvature};
use crate::core::curve_manipulation::{extract_segments, ContourSegment};
use crate::core::fit_and_stroke::solve_cubic_tangent_triangle;
use crate::core::glyph_model::{ContourId, GlyphContour};
use crate::core::math_consts::{
    COMB_ACTIVE_SAMPLES, COMB_DEFAULT_SAMPLES, COMB_DIP_RATIO_THRESHOLD, COMB_SCALE_UPM,
    CONTINUITY_G3_MAX_DKAPPA_DS_DIFF, SOLVER_DET_EPSILON, TUNNI_MAX_RATIO, TUNNI_MIN_RATIO,
};
use kurbo::{CubicBez, ParamCurve, Point};
use serde::{Deserialize, Serialize};

/// Tangent Balance (Tunni) geometry for a single cubic Bézier segment `(P0, P1, P2, P3)` (`SPEC-011 §2.2`).
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct TunniSegmentOverlay {
    pub contour_id: ContourId,
    pub segment_index: usize,
    pub p1: FontPoint,
    pub p2: FontPoint,
    pub tunni_point: FontPoint,
    pub apex: FontPoint,
    pub ratio_p1: f64,
    pub ratio_p2: f64,
}

/// Single sample tooth on a live `CurvatureComb` (`Krommingskam`) overlay (`SPEC-011 §2.9`).
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct CurvatureCombTooth {
    pub t: f64,
    pub base: FontPoint,
    pub tip: FontPoint,
    pub signed_curvature: f64,
    pub dkappa_ds: f64,
}

/// Curvature comb polyline, teeth, inflection markers, and curvature-rate dip locations (`SPEC-011 §2.9`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CurvatureCombPoly {
    pub contour_id: ContourId,
    pub segment_index: usize,
    pub teeth: Vec<CurvatureCombTooth>,
    pub inflections: Vec<FontPoint>,
    pub dkappa_dip_points: Vec<FontPoint>,
}

/// Computes `TunniSegmentOverlay` entries for all cubic segments with forward-intersecting tangent rays (`SPEC-011 §2.2`).
#[must_use]
pub fn compute_tunni_overlays_for_contours(contours: &[GlyphContour]) -> Vec<TunniSegmentOverlay> {
    let mut out = Vec::new();
    for contour in contours {
        let segments = extract_segments(&contour.nodes, contour.closed);
        for (seg_idx, seg) in segments.iter().enumerate() {
            if let ContourSegment::Cubic {
                start_idx,
                h1_idx,
                h2_idx,
                end_idx,
                ..
            } = *seg
            {
                let (p0, p1, p2, p3) = (
                    contour.nodes[start_idx].point,
                    contour.nodes[h1_idx].point,
                    contour.nodes[h2_idx].point,
                    contour.nodes[end_idx].point,
                );
                if let Some((apex, r1, r2)) = solve_cubic_tangent_triangle(p0, p1, p2, p3) {
                    let tunni_point =
                        FontPoint::new(0.5 * (p1.x.0 + p2.x.0), 0.5 * (p1.y.0 + p2.y.0));
                    out.push(TunniSegmentOverlay {
                        contour_id: contour.id,
                        segment_index: seg_idx,
                        p1,
                        p2,
                        tunni_point,
                        apex,
                        ratio_p1: r1,
                        ratio_p2: r2,
                    });
                }
            }
        }
    }
    out
}

/// Scales both handles of `segment_index` in `contour` to `ratio` clamped into `[0.20, 0.95]` (`SPEC-011 §2.2`).
pub fn scale_tunni_in_contour(
    contour: &mut GlyphContour,
    segment_index: usize,
    ratio: f64,
) -> bool {
    let segments = extract_segments(&contour.nodes, contour.closed);
    let Some(ContourSegment::Cubic {
        start_idx,
        h1_idx,
        h2_idx,
        end_idx,
        ..
    }) = segments.get(segment_index).copied()
    else {
        return false;
    };
    let (p0, p1, p2, p3) = (
        contour.nodes[start_idx].point,
        contour.nodes[h1_idx].point,
        contour.nodes[h2_idx].point,
        contour.nodes[end_idx].point,
    );
    let Some((apex, _, _)) = solve_cubic_tangent_triangle(p0, p1, p2, p3) else {
        return false;
    };
    let r = ratio.clamp(TUNNI_MIN_RATIO, TUNNI_MAX_RATIO);
    contour.nodes[h1_idx].point = FontPoint::new(
        p0.x.0 + r * (apex.x.0 - p0.x.0),
        p0.y.0 + r * (apex.y.0 - p0.y.0),
    );
    contour.nodes[h2_idx].point = FontPoint::new(
        p3.x.0 + r * (apex.x.0 - p3.x.0),
        p3.y.0 + r * (apex.y.0 - p3.y.0),
    );
    true
}

/// Drags the Tunni line of `segment_index` so its midpoint moves toward `target_upm` (`SPEC-011 §2.2`).
pub fn drag_tunni_in_contour(
    contour: &mut GlyphContour,
    segment_index: usize,
    target_upm: FontPoint,
) -> bool {
    let segments = extract_segments(&contour.nodes, contour.closed);
    let Some(ContourSegment::Cubic {
        start_idx,
        h1_idx,
        h2_idx,
        end_idx,
        ..
    }) = segments.get(segment_index).copied()
    else {
        return false;
    };
    let (p0, p1, p2, p3) = (
        contour.nodes[start_idx].point,
        contour.nodes[h1_idx].point,
        contour.nodes[h2_idx].point,
        contour.nodes[end_idx].point,
    );
    let Some((apex, t1, t2)) = solve_cubic_tangent_triangle(p0, p1, p2, p3) else {
        return false;
    };
    let chord_mid = FontPoint::new(0.5 * (p0.x.0 + p3.x.0), 0.5 * (p0.y.0 + p3.y.0));
    let (vx, vy) = (apex.x.0 - chord_mid.x.0, apex.y.0 - chord_mid.y.0);
    let v_len2 = vx * vx + vy * vy;
    if v_len2 <= SOLVER_DET_EPSILON {
        return false;
    }
    let proj =
        ((target_upm.x.0 - chord_mid.x.0) * vx + (target_upm.y.0 - chord_mid.y.0) * vy) / v_len2;
    let mean_t = 0.5 * (t1 + t2);
    let scale = if mean_t > SOLVER_DET_EPSILON {
        proj / mean_t
    } else {
        1.0
    };
    let r1 = (t1 * scale).clamp(TUNNI_MIN_RATIO, TUNNI_MAX_RATIO);
    let r2 = (t2 * scale).clamp(TUNNI_MIN_RATIO, TUNNI_MAX_RATIO);
    contour.nodes[h1_idx].point = FontPoint::new(
        p0.x.0 + r1 * (apex.x.0 - p0.x.0),
        p0.y.0 + r1 * (apex.y.0 - p0.y.0),
    );
    contour.nodes[h2_idx].point = FontPoint::new(
        p3.x.0 + r2 * (apex.x.0 - p3.x.0),
        p3.y.0 + r2 * (apex.y.0 - p3.y.0),
    );
    true
}

/// Equalizes both handle ratios of `segment_index` to their mean $\bar{t} \in [0.20, 0.95]$ (`SPEC-011 §2.2`).
pub fn balance_tunni_in_contour(contour: &mut GlyphContour, segment_index: usize) -> bool {
    let segments = extract_segments(&contour.nodes, contour.closed);
    let Some(ContourSegment::Cubic {
        start_idx,
        h1_idx,
        h2_idx,
        end_idx,
        ..
    }) = segments.get(segment_index).copied()
    else {
        return false;
    };
    let (p0, p1, p2, p3) = (
        contour.nodes[start_idx].point,
        contour.nodes[h1_idx].point,
        contour.nodes[h2_idx].point,
        contour.nodes[end_idx].point,
    );
    let Some((_, t1, t2)) = solve_cubic_tangent_triangle(p0, p1, p2, p3) else {
        return false;
    };
    scale_tunni_in_contour(contour, segment_index, 0.5 * (t1 + t2))
}

/// Evaluates the live 120 fps `CurvatureCombPoly` overlays for all cubic segments (`SPEC-011 §2.9`).
#[must_use]
pub fn compute_curvature_combs_for_contours(
    contours: &[GlyphContour],
    selected_nodes: &[NodeId],
    hovered_seg: Option<(usize, usize)>,
) -> Vec<CurvatureCombPoly> {
    let mut out = Vec::new();
    for (c_idx, contour) in contours.iter().enumerate() {
        let segments = extract_segments(&contour.nodes, contour.closed);
        for (seg_idx, seg) in segments.iter().enumerate() {
            let ContourSegment::Cubic {
                start_idx,
                h1_idx,
                h2_idx,
                end_idx,
                ..
            } = *seg
            else {
                continue;
            };
            let ids = [
                contour.nodes[start_idx].id,
                contour.nodes[h1_idx].id,
                contour.nodes[h2_idx].id,
                contour.nodes[end_idx].id,
            ];
            let is_active = hovered_seg == Some((c_idx, seg_idx))
                || ids.iter().any(|id| selected_nodes.contains(id));
            let n_samples = if is_active {
                COMB_ACTIVE_SAMPLES
            } else {
                COMB_DEFAULT_SAMPLES
            };
            let pts = [
                contour.nodes[start_idx].point,
                contour.nodes[h1_idx].point,
                contour.nodes[h2_idx].point,
                contour.nodes[end_idx].point,
            ];
            out.push(build_single_segment_comb(
                contour.id, seg_idx, pts, n_samples,
            ));
        }
    }
    out
}

fn build_single_segment_comb(
    contour_id: ContourId,
    segment_index: usize,
    pts: [FontPoint; 4],
    n_samples: usize,
) -> CurvatureCombPoly {
    let [p0, p1, p2, p3] = pts;
    let bez = CubicBez::new(
        Point::new(p0.x.0, p0.y.0),
        Point::new(p1.x.0, p1.y.0),
        Point::new(p2.x.0, p2.y.0),
        Point::new(p3.x.0, p3.y.0),
    );
    let mut teeth = Vec::with_capacity(n_samples);
    let mut inflections = Vec::new();
    let mut dkappa_dip_points = Vec::new();
    let denom = (n_samples.saturating_sub(1)).max(1) as f64;
    for i in 0..n_samples {
        let t = (i as f64) / denom;
        let b = bez.eval(t);
        let base = FontPoint::new(b.x, b.y);
        let ((dx, dy), _, _) = cubic_derivatives(p0, p1, p2, p3, t);
        let speed = dx.hypot(dy);
        let (nx, ny) = if speed > SOLVER_DET_EPSILON {
            (-dy / speed, dx / speed)
        } else {
            (0.0, 0.0)
        };
        let kappa = cubic_signed_curvature(p0, p1, p2, p3, t);
        let dkappa = cubic_dkappa_ds(p0, p1, p2, p3, t);
        let tip = FontPoint::new(
            base.x.0 + COMB_SCALE_UPM * kappa * nx,
            base.y.0 + COMB_SCALE_UPM * kappa * ny,
        );
        teeth.push(CurvatureCombTooth {
            t,
            base,
            tip,
            signed_curvature: kappa,
            dkappa_ds: dkappa,
        });
    }
    for i in 0..teeth.len().saturating_sub(1) {
        let (k0, k1) = (teeth[i].signed_curvature, teeth[i + 1].signed_curvature);
        if (k0 > 0.0 && k1 < 0.0) || (k0 < 0.0 && k1 > 0.0) {
            let frac = k0.abs() / (k0.abs() + k1.abs()).max(SOLVER_DET_EPSILON);
            let t_inf = teeth[i].t + frac * (teeth[i + 1].t - teeth[i].t);
            let pt = bez.eval(t_inf);
            inflections.push(FontPoint::new(pt.x, pt.y));
        }
    }
    for i in 1..teeth.len().saturating_sub(1) {
        let neighbor_mean = 0.5 * (teeth[i - 1].dkappa_ds.abs() + teeth[i + 1].dkappa_ds.abs());
        if teeth[i].dkappa_ds.abs()
            > COMB_DIP_RATIO_THRESHOLD * neighbor_mean.max(CONTINUITY_G3_MAX_DKAPPA_DS_DIFF)
        {
            dkappa_dip_points.push(teeth[i].base);
        }
    }
    CurvatureCombPoly {
        contour_id,
        segment_index,
        teeth,
        inflections,
        dkappa_dip_points,
    }
}
