//! `SPEC-012 §2.4 .. §2.6`: Live Orthogonal Stem Thickness Band & Reference Stem Gauge,
//! Closed-Form Green's Theorem Counter Area & Visual Centroid $\oplus$,
//! and Lamé Superellipse / Squircle Generator & Live Optical Symmetry Ghost Overlays.

use crate::core::bezier_ops::{Node, NodeId, NodeType};
use crate::core::boolean_ops::contour_signed_area;
use crate::core::coordinates::FontPoint;
use crate::core::curve_harmony::cubic_derivatives;
use crate::core::curve_manipulation::{extract_segments, ContourSegment};
use crate::core::glyph_model::{ContourId, GlyphContour};
use crate::core::knife_ops::contour_intersections_with_segment;
use crate::core::math_consts::{
    CIRCLE_BEZIER_KAPPA, COLLINEAR_MIN_LEN_EPSILON, EULER_GL5_NODES, EULER_GL5_WEIGHTS,
    GREEN_CENTROID_THREE, MEASUREMENT_SIDEBEARING_EXTENT_UPM, PATH_COORD_EPS_UPM,
    SOLVER_DET_EPSILON, SUPERELLIPSE_KAPPA_DENOM, SUPERELLIPSE_MAX_KAPPA, SUPERELLIPSE_MAX_N,
    SUPERELLIPSE_MIN_N,
};
use kurbo::ParamCurve;
use serde::{Deserialize, Serialize};

/// Classification of an orthogonal stem thickness measurement against reference stems (`SPEC-012 §2.4.2`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum StemGaugeMatch {
    Matched,
    Thinner,
    Thicker,
}

/// Result of casting an inward unit normal ray across a filled stem (`SPEC-012 §2.4`).
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct StemThicknessMeasurement {
    pub p_entry: FontPoint,
    pub p_exit: FontPoint,
    pub thickness_upm: f64,
    pub nearest_reference_upm: Option<f64>,
    pub gauge_match: StemGaugeMatch,
}

/// Green's theorem area and centroid entry for a single closed contour (`SPEC-012 §2.5`).
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct ContourAreaCentroid {
    pub contour_id: ContourId,
    pub signed_area_upm2: f64,
    pub centroid: FontPoint,
    pub is_counter: bool,
}

/// Layer-wide Green's theorem stem/counter area balance and visual centroid report (`SPEC-012 §2.5.2`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CounterBalanceReport {
    pub contours: Vec<ContourAreaCentroid>,
    pub net_black_area_upm2: f64,
    pub total_counter_area_upm2: f64,
    pub black_to_white_ratio: f64,
    pub overall_centroid: FontPoint,
}

/// Live optical symmetry ghost overlay mode (`SPEC-012 §2.6.2`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum SymmetryOverlayMode {
    #[default]
    Off,
    HorizontalAxis,
    VerticalAxis,
    PointCenter,
}

fn find_nearest_ray_hit(
    contours: &[GlyphContour],
    p_entry: FontPoint,
    nx: f64,
    ny: f64,
) -> Option<(FontPoint, f64)> {
    let ray_end = FontPoint::new(
        p_entry.x.0 + MEASUREMENT_SIDEBEARING_EXTENT_UPM * nx,
        p_entry.y.0 + MEASUREMENT_SIDEBEARING_EXTENT_UPM * ny,
    );
    let mut best_dist = f64::INFINITY;
    for c in contours {
        for (_u, hit_pt) in contour_intersections_with_segment(&c.nodes, c.closed, p_entry, ray_end)
        {
            let s = (hit_pt.x.0 - p_entry.x.0) * nx + (hit_pt.y.0 - p_entry.y.0) * ny;
            if s > PATH_COORD_EPS_UPM && s < best_dist {
                best_dist = s;
            }
        }
    }
    if best_dist.is_finite() {
        let p_exit = FontPoint::new(p_entry.x.0 + best_dist * nx, p_entry.y.0 + best_dist * ny);
        Some((p_exit, best_dist))
    } else {
        None
    }
}

/// Casts an inward unit normal ray from `(contour_idx, seg_idx, t)` and classifies stem thickness (`SPEC-012 §2.4`).
#[must_use]
pub fn measure_contour_orthogonal_stem_thickness(
    contours: &[GlyphContour],
    contour_idx: usize,
    seg_idx: usize,
    t: f64,
    reference_stems_upm: &[f64],
    stem_tolerance_upm: f64,
) -> Option<StemThicknessMeasurement> {
    let contour = contours.get(contour_idx)?;
    let segs = extract_segments(&contour.nodes, contour.closed);
    let seg = segs.get(seg_idx)?;
    let t_clamped = t.clamp(0.0, 1.0);
    let (p_entry, tx, ty) = match *seg {
        ContourSegment::Line { line, .. } => {
            let pt = line.eval(t_clamped);
            (
                FontPoint::new(pt.x, pt.y),
                line.p1.x - line.p0.x,
                line.p1.y - line.p0.y,
            )
        }
        ContourSegment::Cubic {
            start_idx,
            h1_idx,
            h2_idx,
            end_idx,
            cubic,
        } => {
            let pt = cubic.eval(t_clamped);
            let ((dx, dy), _, _) = cubic_derivatives(
                contour.nodes[start_idx].point,
                contour.nodes[h1_idx].point,
                contour.nodes[h2_idx].point,
                contour.nodes[end_idx].point,
                t_clamped,
            );
            (FontPoint::new(pt.x, pt.y), dx, dy)
        }
    };
    let t_len = tx.hypot(ty);
    if t_len <= SOLVER_DET_EPSILON {
        return None;
    }
    let (ux, uy) = (tx / t_len, ty / t_len);
    let dominant_sign = contours
        .iter()
        .filter(|c| c.closed && c.nodes.len() >= 3)
        .map(|c| contour_signed_area(&c.nodes))
        .max_by(|a, b| {
            a.abs()
                .partial_cmp(&b.abs())
                .unwrap_or(std::cmp::Ordering::Equal)
        })
        .map_or(1.0, f64::signum);
    let (nx, ny) = if dominant_sign >= 0.0 {
        (-uy, ux)
    } else {
        (uy, -ux)
    };
    let (p_exit, thickness_upm) = find_nearest_ray_hit(contours, p_entry, nx, ny)
        .or_else(|| find_nearest_ray_hit(contours, p_entry, -nx, -ny))?;
    let (nearest_reference_upm, gauge_match) =
        classify_stem_gauge_match(thickness_upm, reference_stems_upm, stem_tolerance_upm);
    Some(StemThicknessMeasurement {
        p_entry,
        p_exit,
        thickness_upm,
        nearest_reference_upm,
        gauge_match,
    })
}

fn classify_stem_gauge_match(
    thickness_upm: f64,
    reference_stems_upm: &[f64],
    stem_tolerance_upm: f64,
) -> (Option<f64>, StemGaugeMatch) {
    let tol = stem_tolerance_upm.abs();
    let nearest_reference_upm = reference_stems_upm
        .iter()
        .copied()
        .filter(|s| s.is_finite())
        .min_by(|a, b| {
            (thickness_upm - *a)
                .abs()
                .partial_cmp(&(thickness_upm - *b).abs())
                .unwrap_or(std::cmp::Ordering::Equal)
        });
    let gauge_match = if let Some(ref_stem) = nearest_reference_upm {
        if (thickness_upm - ref_stem).abs() <= tol {
            StemGaugeMatch::Matched
        } else if thickness_upm < ref_stem - tol {
            StemGaugeMatch::Thinner
        } else {
            StemGaugeMatch::Thicker
        }
    } else {
        StemGaugeMatch::Matched
    };
    (nearest_reference_upm, gauge_match)
}

/// Evaluates exact Green's theorem signed area $A$ and visual centroid $(C_x, C_y)$ of a closed contour (`SPEC-012 §2.5.1`).
#[must_use]
pub fn compute_contour_area_and_centroid(nodes: &[Node]) -> (f64, FontPoint) {
    if nodes.is_empty() {
        return (0.0, FontPoint::new(0.0, 0.0));
    }
    let segs = extract_segments(nodes, true);
    if segs.is_empty() {
        let count = nodes.len() as f64;
        let sx: f64 = nodes.iter().map(|n| n.point.x.0).sum();
        let sy: f64 = nodes.iter().map(|n| n.point.y.0).sum();
        return (0.0, FontPoint::new(sx / count, sy / count));
    }
    let mut i_area = 0.0;
    let mut i_cx = 0.0;
    let mut i_cy = 0.0;
    for seg in segs {
        for (node_offset, weight) in EULER_GL5_NODES.iter().zip(EULER_GL5_WEIGHTS.iter()) {
            let t = 0.5 + *node_offset;
            let (x, y, dx, dy) = match seg {
                ContourSegment::Line { line, .. } => {
                    let pt = line.eval(t);
                    (pt.x, pt.y, line.p1.x - line.p0.x, line.p1.y - line.p0.y)
                }
                ContourSegment::Cubic {
                    start_idx,
                    h1_idx,
                    h2_idx,
                    end_idx,
                    cubic,
                } => {
                    let pt = cubic.eval(t);
                    let ((dx, dy), _, _) = cubic_derivatives(
                        nodes[start_idx].point,
                        nodes[h1_idx].point,
                        nodes[h2_idx].point,
                        nodes[end_idx].point,
                        t,
                    );
                    (pt.x, pt.y, dx, dy)
                }
            };
            let cross = x * dy - y * dx;
            i_area += *weight * cross;
            i_cx += *weight * x * cross;
            i_cy += *weight * y * cross;
        }
    }
    let signed_area = 0.5 * i_area;
    let denom = GREEN_CENTROID_THREE * signed_area;
    let centroid = if denom.abs() > SOLVER_DET_EPSILON {
        FontPoint::new(i_cx / denom, i_cy / denom)
    } else {
        let count = nodes.len() as f64;
        let sx: f64 = nodes.iter().map(|n| n.point.x.0).sum();
        let sy: f64 = nodes.iter().map(|n| n.point.y.0).sum();
        FontPoint::new(sx / count, sy / count)
    };
    (signed_area, centroid)
}

/// Computes the layer-wide `CounterBalanceReport` across `contours` (`SPEC-012 §2.5.2`).
#[must_use]
pub fn compute_contours_counter_balance(contours: &[GlyphContour]) -> CounterBalanceReport {
    let mut raw: Vec<(ContourId, f64, FontPoint)> = Vec::with_capacity(contours.len());
    for c in contours {
        if c.closed && c.nodes.len() >= 3 {
            let (area, centroid) = compute_contour_area_and_centroid(&c.nodes);
            raw.push((c.id, area, centroid));
        }
    }
    let dominant_sign = raw
        .iter()
        .max_by(|a, b| {
            a.1.abs()
                .partial_cmp(&b.1.abs())
                .unwrap_or(std::cmp::Ordering::Equal)
        })
        .map_or(1.0, |(_, a, _)| a.signum());

    let mut entries = Vec::with_capacity(raw.len());
    let mut outer_area = 0.0;
    let mut total_counter_area_upm2 = 0.0;
    let mut wx = 0.0;
    let mut wy = 0.0;
    let mut w_sum = 0.0;

    for (contour_id, signed_area_upm2, centroid) in raw {
        let abs_a = signed_area_upm2.abs();
        let is_counter = abs_a > SOLVER_DET_EPSILON && signed_area_upm2.signum() != dominant_sign;
        if is_counter {
            total_counter_area_upm2 += abs_a;
            wx -= abs_a * centroid.x.0;
            wy -= abs_a * centroid.y.0;
            w_sum -= abs_a;
        } else {
            outer_area += abs_a;
            wx += abs_a * centroid.x.0;
            wy += abs_a * centroid.y.0;
            w_sum += abs_a;
        }
        entries.push(ContourAreaCentroid {
            contour_id,
            signed_area_upm2,
            centroid,
            is_counter,
        });
    }
    let net_black_area_upm2 = (outer_area - total_counter_area_upm2).max(0.0);
    let black_to_white_ratio = if total_counter_area_upm2 > COLLINEAR_MIN_LEN_EPSILON {
        net_black_area_upm2 / total_counter_area_upm2
    } else {
        0.0
    };
    let overall_centroid = if w_sum.abs() > SOLVER_DET_EPSILON {
        FontPoint::new(wx / w_sum, wy / w_sum)
    } else {
        entries
            .first()
            .map_or(FontPoint::new(0.0, 0.0), |e| e.centroid)
    };
    CounterBalanceReport {
        contours: entries,
        net_black_area_upm2,
        total_counter_area_upm2,
        black_to_white_ratio,
        overall_centroid,
    }
}

/// Generates a closed 12-node superellipse / squircle contour for Lamé exponent $n \in [2.0, 20.0]$ (`SPEC-012 §2.6.1`).
#[must_use]
pub fn build_superellipse_contour_nodes(
    rect: (FontPoint, FontPoint),
    exponent_n: f64,
) -> Vec<Node> {
    let (p_min, p_max) = rect;
    let x_min = p_min.x.0.min(p_max.x.0);
    let x_max = p_min.x.0.max(p_max.x.0);
    let y_min = p_min.y.0.min(p_max.y.0);
    let y_max = p_min.y.0.max(p_max.y.0);
    let cx = 0.5 * (x_min + x_max);
    let cy = 0.5 * (y_min + y_max);
    let a = 0.5 * (x_max - x_min);
    let b = 0.5 * (y_max - y_min);
    let n = if exponent_n.is_finite() {
        exponent_n.clamp(SUPERELLIPSE_MIN_N, SUPERELLIPSE_MAX_N)
    } else {
        SUPERELLIPSE_MIN_N
    };
    let diag = 2.0_f64.powf(-1.0 / n);
    let k = ((2.0 * diag - 1.0) / SUPERELLIPSE_KAPPA_DENOM)
        .clamp(CIRCLE_BEZIER_KAPPA, SUPERELLIPSE_MAX_KAPPA);
    let (kx, ky) = (k * a, k * b);
    vec![
        Node::new(NodeId(1), cx, y_min, NodeType::Smooth),
        Node::new(NodeId(2), cx + kx, y_min, NodeType::OffCurve),
        Node::new(NodeId(3), x_max, cy - ky, NodeType::OffCurve),
        Node::new(NodeId(4), x_max, cy, NodeType::Smooth),
        Node::new(NodeId(5), x_max, cy + ky, NodeType::OffCurve),
        Node::new(NodeId(6), cx + kx, y_max, NodeType::OffCurve),
        Node::new(NodeId(7), cx, y_max, NodeType::Smooth),
        Node::new(NodeId(8), cx - kx, y_max, NodeType::OffCurve),
        Node::new(NodeId(9), x_min, cy + ky, NodeType::OffCurve),
        Node::new(NodeId(10), x_min, cy, NodeType::Smooth),
        Node::new(NodeId(11), x_min, cy - ky, NodeType::OffCurve),
        Node::new(NodeId(12), cx - kx, y_min, NodeType::OffCurve),
    ]
}

/// Computes reflected or $180^\circ$-point-inverted ghost contours for `mode` (`SPEC-012 §2.6.2`).
#[must_use]
pub fn compute_contours_symmetry_ghost(
    contours: &[GlyphContour],
    mode: SymmetryOverlayMode,
) -> Vec<Vec<Node>> {
    if mode == SymmetryOverlayMode::Off || contours.is_empty() {
        return Vec::new();
    }
    let mut x_min = f64::INFINITY;
    let mut x_max = f64::NEG_INFINITY;
    let mut y_min = f64::INFINITY;
    let mut y_max = f64::NEG_INFINITY;
    for n in contours.iter().flat_map(|c| c.nodes.iter()) {
        x_min = x_min.min(n.point.x.0);
        x_max = x_max.max(n.point.x.0);
        y_min = y_min.min(n.point.y.0);
        y_max = y_max.max(n.point.y.0);
    }
    if !x_min.is_finite() || !y_min.is_finite() {
        return Vec::new();
    }
    let two_mx = x_min + x_max;
    let two_my = y_min + y_max;
    contours
        .iter()
        .map(|c| {
            c.nodes
                .iter()
                .map(|n| {
                    let mut m = n.clone();
                    m.point = match mode {
                        SymmetryOverlayMode::Off => n.point,
                        SymmetryOverlayMode::HorizontalAxis => {
                            FontPoint::new(n.point.x.0, two_my - n.point.y.0)
                        }
                        SymmetryOverlayMode::VerticalAxis => {
                            FontPoint::new(two_mx - n.point.x.0, n.point.y.0)
                        }
                        SymmetryOverlayMode::PointCenter => {
                            FontPoint::new(two_mx - n.point.x.0, two_my - n.point.y.0)
                        }
                    };
                    m
                })
                .collect()
        })
        .collect()
}
