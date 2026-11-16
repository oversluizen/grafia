//! `SPEC-011 §2.5 & §2.6.1`: Proactive Vector Outline Audit (`audit_layer_outlines`)
//! and Off-Axis Handle Warning (`compute_angled_handle_warnings`).

use crate::core::bezier_ops::{NodeId, NodeType};
use crate::core::coordinates::FontPoint;
use crate::core::curve_manipulation::{extract_segments, ContourSegment};
use crate::core::glyph_model::{ContourId, GlyphContour};
use crate::core::math_consts::{
    ANGLED_HANDLE_MAX_DEG, ANGLED_HANDLE_MIN_DEG, AUDIT_COLLINEAR_MAX_PERP_UPM,
    AUDIT_EXTREMUM_EXCURSION_MIN_UPM, AUDIT_QUASI_ORTHO_MAX_DEG, AUDIT_QUASI_ORTHO_MIN_DEG,
    AUDIT_STEM_MIN_LEN_UPM, CURVE_ROOT_DEDUP_EPS, CURVE_ROOT_T_MAX, CURVE_ROOT_T_MIN,
    DEG_TO_RAD_F64, QUAD_DISC_FOUR, RAD_TO_DEG_F64, SERVO_MIN_SPAN_UPM,
    SNAP_LINE_INTERSECT_DET_EPS, SOLVER_DET_EPSILON, TRAJECTORY_INTERIOR_EPS,
};
use kurbo::{CubicBez, ParamCurve, Point};
use serde::{Deserialize, Serialize};

/// Classification of the 4 classic vector outline defects on the 2D canvas (`SPEC-011 §2.5`).
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub enum OutlineAuditKind {
    MissingExtremum {
        contour_idx: usize,
        seg_idx: usize,
        t: f64,
        point: FontPoint,
    },
    QuasiOrthogonalStem {
        contour_idx: usize,
        seg_idx: usize,
        angle_deviation_deg: f64,
    },
    CrossedHandles {
        contour_idx: usize,
        seg_idx: usize,
    },
    CollinearCubic {
        contour_idx: usize,
        seg_idx: usize,
    },
}

/// Single finding emitted by `audit_layer_outlines` (`SPEC-011 §2.5`).
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct OutlineAuditFinding {
    pub contour_id: ContourId,
    pub kind: OutlineAuditKind,
}

/// Warning for an `OffCurve` handle attached to a `Smooth` extremum that deviates from orthogonal/italic axes (`SPEC-011 §2.6`).
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct AngledHandleWarning {
    pub contour_idx: usize,
    pub smooth_node_id: NodeId,
    pub handle_node_id: NodeId,
    pub deviation_deg: f64,
}

pub(crate) fn min_axis_deviation_deg(dx: f64, dy: f64, italic_angle_deg: f64) -> f64 {
    let len = dx.hypot(dy);
    if len <= SOLVER_DET_EPSILON {
        return 0.0;
    }
    let (ux, uy) = (dx / len, dy / len);
    let dev_horiz = uy.abs().clamp(0.0, 1.0).asin() * RAD_TO_DEG_F64;
    let dev_vert = ux.abs().clamp(0.0, 1.0).asin() * RAD_TO_DEG_F64;
    let mut best = dev_horiz.min(dev_vert);
    if italic_angle_deg.abs() > 0.0 {
        let rad = italic_angle_deg * DEG_TO_RAD_F64;
        let cross = (ux * rad.cos() - uy * rad.sin()).abs().clamp(0.0, 1.0);
        best = best.min(cross.asin() * RAD_TO_DEG_F64);
    }
    best
}

fn eval_1d_bernstein(c0: f64, c1: f64, c2: f64, c3: f64, t: f64) -> f64 {
    let bez = CubicBez::new(
        Point::new(c0, 0.0),
        Point::new(c1, 0.0),
        Point::new(c2, 0.0),
        Point::new(c3, 0.0),
    );
    bez.eval(t).x
}

fn solve_1d_extremum_roots_clean(c0: f64, c1: f64, c2: f64, c3: f64) -> Vec<f64> {
    let (d0, d1, d2) = (c1 - c0, c2 - c1, c3 - c2);
    let a = d0 - 2.0 * d1 + d2;
    let b = 2.0 * (d1 - d0);
    let c = d0;
    let mut candidates = Vec::new();
    if a.abs() <= SOLVER_DET_EPSILON {
        if b.abs() > SOLVER_DET_EPSILON {
            candidates.push(-c / b);
        }
    } else {
        let disc = b * b - QUAD_DISC_FOUR * a * c;
        if disc >= 0.0 {
            let sq = disc.sqrt();
            candidates.push((-b - sq) / (2.0 * a));
            candidates.push((-b + sq) / (2.0 * a));
        }
    }
    let lo_bound = c0.min(c3) - AUDIT_EXTREMUM_EXCURSION_MIN_UPM;
    let hi_bound = c0.max(c3) + AUDIT_EXTREMUM_EXCURSION_MIN_UPM;
    let mut valid: Vec<f64> = Vec::new();
    for t in candidates {
        if t > CURVE_ROOT_T_MIN && t < CURVE_ROOT_T_MAX {
            let val = eval_1d_bernstein(c0, c1, c2, c3, t);
            if (val < lo_bound || val > hi_bound)
                && valid
                    .iter()
                    .all(|&prev| (prev - t).abs() > CURVE_ROOT_DEDUP_EPS)
            {
                valid.push(t);
            }
        }
    }
    valid
}

pub(crate) fn open_segments_interior_intersection(
    a0: FontPoint,
    a1: FontPoint,
    b0: FontPoint,
    b1: FontPoint,
) -> Option<FontPoint> {
    let (r_x, r_y) = (a1.x.0 - a0.x.0, a1.y.0 - a0.y.0);
    let (s_x, s_y) = (b1.x.0 - b0.x.0, b1.y.0 - b0.y.0);
    let det = r_x * s_y - r_y * s_x;
    if det.abs() <= SNAP_LINE_INTERSECT_DET_EPS {
        return None;
    }
    let (qp_x, qp_y) = (b0.x.0 - a0.x.0, b0.y.0 - a0.y.0);
    let u = (qp_x * s_y - qp_y * s_x) / det;
    let v = (qp_x * r_y - qp_y * r_x) / det;
    let (lo, hi) = (TRAJECTORY_INTERIOR_EPS, 1.0 - TRAJECTORY_INTERIOR_EPS);
    if u > lo && u < hi && v > lo && v < hi {
        Some(FontPoint::new(a0.x.0 + u * r_x, a0.y.0 + u * r_y))
    } else {
        None
    }
}

/// Audits all contours in the layer for `MissingExtremum`, `QuasiOrthogonalStem`, `CrossedHandles`, and `CollinearCubic` (`SPEC-011 §2.5`).
#[must_use]
pub fn audit_contours_outlines(
    contours: &[GlyphContour],
    italic_angle_deg: f64,
) -> Vec<OutlineAuditFinding> {
    debug_assert!(italic_angle_deg.is_finite());
    let mut findings = Vec::new();
    let tan_italic = (italic_angle_deg * DEG_TO_RAD_F64).tan();
    for (contour_idx, contour) in contours.iter().enumerate() {
        let segments = extract_segments(&contour.nodes, contour.closed);
        for (seg_idx, seg) in segments.iter().enumerate() {
            match *seg {
                ContourSegment::Line {
                    start_idx, end_idx, ..
                } => {
                    let (p0, p3) = (contour.nodes[start_idx].point, contour.nodes[end_idx].point);
                    let (dx, dy) = (p3.x.0 - p0.x.0, p3.y.0 - p0.y.0);
                    if dx.hypot(dy) >= AUDIT_STEM_MIN_LEN_UPM {
                        let dev = min_axis_deviation_deg(dx, dy, italic_angle_deg);
                        if (AUDIT_QUASI_ORTHO_MIN_DEG..=AUDIT_QUASI_ORTHO_MAX_DEG).contains(&dev) {
                            findings.push(OutlineAuditFinding {
                                contour_id: contour.id,
                                kind: OutlineAuditKind::QuasiOrthogonalStem {
                                    contour_idx,
                                    seg_idx,
                                    angle_deviation_deg: dev,
                                },
                            });
                        }
                    }
                }
                ContourSegment::Cubic {
                    start_idx,
                    h1_idx,
                    h2_idx,
                    end_idx,
                    ..
                } => {
                    let pts = [
                        contour.nodes[start_idx].point,
                        contour.nodes[h1_idx].point,
                        contour.nodes[h2_idx].point,
                        contour.nodes[end_idx].point,
                    ];
                    audit_single_cubic_segment(
                        contour.id,
                        contour_idx,
                        seg_idx,
                        pts,
                        tan_italic,
                        &mut findings,
                    );
                }
            }
        }
    }
    findings
}

fn audit_single_cubic_segment(
    contour_id: ContourId,
    contour_idx: usize,
    seg_idx: usize,
    pts: [FontPoint; 4],
    tan_italic: f64,
    findings: &mut Vec<OutlineAuditFinding>,
) {
    let [p0, p1, p2, p3] = pts;
    let bez = CubicBez::new(
        Point::new(p0.x.0, p0.y.0),
        Point::new(p1.x.0, p1.y.0),
        Point::new(p2.x.0, p2.y.0),
        Point::new(p3.x.0, p3.y.0),
    );
    let xs0 = p0.x.0 - p0.y.0 * tan_italic;
    let xs1 = p1.x.0 - p1.y.0 * tan_italic;
    let xs2 = p2.x.0 - p2.y.0 * tan_italic;
    let xs3 = p3.x.0 - p3.y.0 * tan_italic;
    for t in solve_1d_extremum_roots_clean(xs0, xs1, xs2, xs3)
        .into_iter()
        .chain(solve_1d_extremum_roots_clean(
            p0.y.0, p1.y.0, p2.y.0, p3.y.0,
        ))
    {
        let pt = bez.eval(t);
        findings.push(OutlineAuditFinding {
            contour_id,
            kind: OutlineAuditKind::MissingExtremum {
                contour_idx,
                seg_idx,
                t,
                point: FontPoint::new(pt.x, pt.y),
            },
        });
    }
    if open_segments_interior_intersection(p0, p1, p3, p2).is_some() {
        findings.push(OutlineAuditFinding {
            contour_id,
            kind: OutlineAuditKind::CrossedHandles {
                contour_idx,
                seg_idx,
            },
        });
    }
    let chord_len = p0.distance_to(p3);
    if chord_len > SERVO_MIN_SPAN_UPM {
        let (cx, cy) = (p3.x.0 - p0.x.0, p3.y.0 - p0.y.0);
        let d1 = ((cx * (p1.y.0 - p0.y.0) - cy * (p1.x.0 - p0.x.0)).abs()) / chord_len;
        let d2 = ((cx * (p2.y.0 - p0.y.0) - cy * (p2.x.0 - p0.x.0)).abs()) / chord_len;
        if d1 <= AUDIT_COLLINEAR_MAX_PERP_UPM && d2 <= AUDIT_COLLINEAR_MAX_PERP_UPM {
            findings.push(OutlineAuditFinding {
                contour_id,
                kind: OutlineAuditKind::CollinearCubic {
                    contour_idx,
                    seg_idx,
                },
            });
        }
    }
}

/// Inspects `OffCurve` handles attached to `Smooth` extrema that deviate slightly from orthogonal/italic axes (`SPEC-011 §2.6.1`).
#[must_use]
pub fn compute_angled_handle_warnings_in_contours(
    contours: &[GlyphContour],
    italic_angle_deg: f64,
) -> Vec<AngledHandleWarning> {
    let mut warnings = Vec::new();
    for (contour_idx, contour) in contours.iter().enumerate() {
        let n = contour.nodes.len();
        if n < 2 {
            continue;
        }
        for (i, node) in contour.nodes.iter().enumerate() {
            if node.node_type != NodeType::Smooth {
                continue;
            }
            let mut neighbors = Vec::with_capacity(2);
            if contour.closed {
                neighbors.push((i + n - 1) % n);
                neighbors.push((i + 1) % n);
            } else {
                if i > 0 {
                    neighbors.push(i - 1);
                }
                if i + 1 < n {
                    neighbors.push(i + 1);
                }
            }
            for nb_idx in neighbors {
                let h_node = &contour.nodes[nb_idx];
                if h_node.node_type != NodeType::OffCurve {
                    continue;
                }
                let dev = min_axis_deviation_deg(
                    h_node.point.x.0 - node.point.x.0,
                    h_node.point.y.0 - node.point.y.0,
                    italic_angle_deg,
                );
                if (ANGLED_HANDLE_MIN_DEG..=ANGLED_HANDLE_MAX_DEG).contains(&dev) {
                    warnings.push(AngledHandleWarning {
                        contour_idx,
                        smooth_node_id: node.id,
                        handle_node_id: h_node.id,
                        deviation_deg: dev,
                    });
                }
            }
        }
    }
    warnings
}

/// Resolves the target `FontPoint` and compact badge label (`"EXT"`, `"STEM"`, `"CROSS"`, `"FLAT"`) for an `OutlineAuditKind` finding (`SPEC-011 §2.5`).
#[must_use]
pub fn resolve_outline_audit_target(
    contours: &[GlyphContour],
    kind: OutlineAuditKind,
) -> Option<(FontPoint, &'static str)> {
    match kind {
        OutlineAuditKind::MissingExtremum { point, .. } => Some((point, "EXT")),
        OutlineAuditKind::QuasiOrthogonalStem {
            contour_idx,
            seg_idx,
            ..
        } => {
            let c = contours.get(contour_idx)?;
            let segs = extract_segments(&c.nodes, c.closed);
            let ContourSegment::Line {
                start_idx, end_idx, ..
            } = *segs.get(seg_idx)?
            else {
                return None;
            };
            let (a, b) = (c.nodes[start_idx].point, c.nodes[end_idx].point);
            Some((
                FontPoint::new(0.5 * (a.x.0 + b.x.0), 0.5 * (a.y.0 + b.y.0)),
                "STEM",
            ))
        }
        OutlineAuditKind::CrossedHandles {
            contour_idx,
            seg_idx,
        } => {
            let c = contours.get(contour_idx)?;
            let segs = extract_segments(&c.nodes, c.closed);
            let ContourSegment::Cubic {
                start_idx,
                h1_idx,
                h2_idx,
                end_idx,
                ..
            } = *segs.get(seg_idx)?
            else {
                return None;
            };
            let (p0, p1, p2, p3) = (
                c.nodes[start_idx].point,
                c.nodes[h1_idx].point,
                c.nodes[h2_idx].point,
                c.nodes[end_idx].point,
            );
            let target = open_segments_interior_intersection(p0, p1, p3, p2).unwrap_or_else(|| {
                FontPoint::new(0.5 * (p1.x.0 + p2.x.0), 0.5 * (p1.y.0 + p2.y.0))
            });
            Some((target, "CROSS"))
        }
        OutlineAuditKind::CollinearCubic {
            contour_idx,
            seg_idx,
        } => {
            let c = contours.get(contour_idx)?;
            let segs = extract_segments(&c.nodes, c.closed);
            let ContourSegment::Cubic {
                start_idx, end_idx, ..
            } = *segs.get(seg_idx)?
            else {
                return None;
            };
            let (p0, p3) = (c.nodes[start_idx].point, c.nodes[end_idx].point);
            Some((
                FontPoint::new(0.5 * (p0.x.0 + p3.x.0), 0.5 * (p0.y.0 + p3.y.0)),
                "FLAT",
            ))
        }
    }
}
