//! `SPEC-013 §2.1`: Interactive Structural `Stem` Tool, Anti-Parallel Wall Pairing,
//! Servo-Coupled Stem Pose & Thickness Editing, and Measurement Rainbow Cross-Section Quads.

use crate::core::bezier_ops::{Node, NodeId, NodeType};
use crate::core::contour_order_ops::flatten_contour_points;
use crate::core::coordinates::FontPoint;
use crate::core::curve_manipulation::{extract_segments, ContourSegment};
use crate::core::glyph_model::GlyphContour;
use crate::core::knife_ops::contour_intersections_with_segment;
use crate::core::lasso_and_zoom::point_in_polygon;
use crate::core::math_consts::{
    DEG_TO_RAD_F64, GUIDE_HALF_CIRCLE_DEG, GUIDE_PERPENDICULAR_DEG, RAD_TO_DEG_F64,
    SOLVER_DET_EPSILON, STEM_ANTIPARALLEL_COS_MAX, STEM_CUBIC_FLATNESS_TOL_UPM,
    STEM_DEDUP_DIST_UPM, STEM_HORIZ_MAX_DEG, STEM_MAX_WIDTH_UPM, STEM_MIN_EDIT_WIDTH_UPM,
    STEM_MIN_WIDTH_UPM, STEM_ORTHO_ANGLE_TOL_DEG, STEM_OVERLAP_MIN_UPM,
    STEM_PERP_COLLINEAR_TOL_UPM, STEM_RAY_INTERIOR_PROBE_UPM,
};
use crate::core::servo_and_nudge::servo_translate_node_in_contours;
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;

/// Orientation category of a detected structural stem (`SPEC-013 §2.1`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum StemOrientation {
    Horizontal,
    Vertical,
    Diagonal,
}

/// Wall anchoring mode when adjusting structural stem thickness (`SPEC-013 §2.1.2`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum StemAdjustmentMode {
    #[default]
    SymmetricCenter,
    LockLeft,
    LockRight,
}

/// Paired opposing contour walls forming a structural stem (`SPEC-013 §2.1.1`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DetectedStem {
    pub id: usize,
    pub contour_idx_a: usize,
    pub seg_idx_a: usize,
    pub contour_idx_b: usize,
    pub seg_idx_b: usize,
    pub p_left: FontPoint,
    pub p_right: FontPoint,
    pub width_upm: f64,
    pub angle_deg: f64,
    pub orientation: StemOrientation,
}

#[derive(Debug, Clone, Copy)]
struct WallCandidate {
    contour_idx: usize,
    seg_idx: usize,
    p0: FontPoint,
    p1: FontPoint,
    ux: f64,
    uy: f64,
    len: f64,
}

fn classify_stem_orientation(ux: f64, uy: f64) -> (f64, StemOrientation) {
    let angle_deg = (uy.atan2(ux) * RAD_TO_DEG_F64).rem_euclid(GUIDE_HALF_CIRCLE_DEG);
    let orientation = if angle_deg <= STEM_ORTHO_ANGLE_TOL_DEG || angle_deg >= STEM_HORIZ_MAX_DEG {
        StemOrientation::Horizontal
    } else if (angle_deg - GUIDE_PERPENDICULAR_DEG).abs() <= STEM_ORTHO_ANGLE_TOL_DEG {
        StemOrientation::Vertical
    } else {
        StemOrientation::Diagonal
    };
    (angle_deg, orientation)
}

fn collect_wall_candidates(contours: &[GlyphContour]) -> Vec<WallCandidate> {
    let mut out = Vec::new();
    for (c_idx, contour) in contours.iter().enumerate() {
        if !contour.visible || contour.nodes.len() < 2 {
            continue;
        }
        let segs = extract_segments(&contour.nodes, contour.closed);
        for (s_idx, seg) in segs.iter().enumerate() {
            let (p0, p1, is_flat) = match *seg {
                ContourSegment::Line { line, .. } => (
                    FontPoint::new(line.p0.x, line.p0.y),
                    FontPoint::new(line.p1.x, line.p1.y),
                    true,
                ),
                ContourSegment::Cubic {
                    start_idx,
                    h1_idx,
                    h2_idx,
                    end_idx,
                    ..
                } => {
                    let a = contour.nodes[start_idx].point;
                    let b = contour.nodes[end_idx].point;
                    let h1 = contour.nodes[h1_idx].point;
                    let h2 = contour.nodes[h2_idx].point;
                    let dx = b.x.0 - a.x.0;
                    let dy = b.y.0 - a.y.0;
                    let len = dx.hypot(dy);
                    if len <= SOLVER_DET_EPSILON {
                        continue;
                    }
                    let nx = -dy / len;
                    let ny = dx / len;
                    let d1 = ((h1.x.0 - a.x.0) * nx + (h1.y.0 - a.y.0) * ny).abs();
                    let d2 = ((h2.x.0 - a.x.0) * nx + (h2.y.0 - a.y.0) * ny).abs();
                    (
                        a,
                        b,
                        d1 <= STEM_CUBIC_FLATNESS_TOL_UPM && d2 <= STEM_CUBIC_FLATNESS_TOL_UPM,
                    )
                }
            };
            if !is_flat {
                continue;
            }
            let dx = p1.x.0 - p0.x.0;
            let dy = p1.y.0 - p0.y.0;
            let len = dx.hypot(dy);
            if len >= STEM_OVERLAP_MIN_UPM {
                out.push(WallCandidate {
                    contour_idx: c_idx,
                    seg_idx: s_idx,
                    p0,
                    p1,
                    ux: dx / len,
                    uy: dy / len,
                    len,
                });
            }
        }
    }
    out
}

fn is_point_inside_contours(pt: FontPoint, polys: &[Vec<FontPoint>]) -> bool {
    let mut inside = false;
    for poly in polys {
        if poly.len() >= 3 && point_in_polygon(pt, poly) {
            inside = !inside;
        }
    }
    inside
}

fn has_intervening_wall_hit(contours: &[GlyphContour], pa: FontPoint, pb: FontPoint) -> bool {
    let span = pa.distance_to(pb);
    if span <= STEM_RAY_INTERIOR_PROBE_UPM * 2.0 {
        return false;
    }
    let ux = (pb.x.0 - pa.x.0) / span;
    let uy = (pb.y.0 - pa.y.0) / span;
    let p_start = FontPoint::new(
        pa.x.0 + STEM_RAY_INTERIOR_PROBE_UPM * ux,
        pa.y.0 + STEM_RAY_INTERIOR_PROBE_UPM * uy,
    );
    let p_end = FontPoint::new(
        pb.x.0 - STEM_RAY_INTERIOR_PROBE_UPM * ux,
        pb.y.0 - STEM_RAY_INTERIOR_PROBE_UPM * uy,
    );
    contours.iter().any(|c| {
        c.visible
            && !contour_intersections_with_segment(&c.nodes, c.closed, p_start, p_end).is_empty()
    })
}

fn try_pair_wall_candidates(
    contours: &[GlyphContour],
    polys: &[Vec<FontPoint>],
    wa: WallCandidate,
    wb: WallCandidate,
) -> Option<(f64, DetectedStem)> {
    if wa.ux * wb.ux + wa.uy * wb.uy > STEM_ANTIPARALLEL_COS_MAX {
        return None;
    }
    let (nx, ny) = (-wa.uy, wa.ux);
    let signed_perp = (wb.p0.x.0 - wa.p0.x.0) * nx + (wb.p0.y.0 - wa.p0.y.0) * ny;
    let width_upm = signed_perp.abs();
    if !(STEM_MIN_WIDTH_UPM..=STEM_MAX_WIDTH_UPM).contains(&width_upm) {
        return None;
    }
    let sb0 = (wb.p0.x.0 - wa.p0.x.0) * wa.ux + (wb.p0.y.0 - wa.p0.y.0) * wa.uy;
    let sb1 = (wb.p1.x.0 - wa.p0.x.0) * wa.ux + (wb.p1.y.0 - wa.p0.y.0) * wa.uy;
    let s_min = sb0.min(sb1).max(0.0);
    let s_max = sb0.max(sb1).min(wa.len);
    let overlap = s_max - s_min;
    if overlap < STEM_OVERLAP_MIN_UPM {
        return None;
    }
    let s_mid = 0.5 * (s_min + s_max);
    let pa = FontPoint::new(wa.p0.x.0 + s_mid * wa.ux, wa.p0.y.0 + s_mid * wa.uy);
    let pb = FontPoint::new(pa.x.0 + signed_perp * nx, pa.y.0 + signed_perp * ny);
    let mid = FontPoint::new(0.5 * (pa.x.0 + pb.x.0), 0.5 * (pa.y.0 + pb.y.0));
    if (!polys.is_empty() && !is_point_inside_contours(mid, polys))
        || has_intervening_wall_hit(contours, pa, pb)
    {
        return None;
    }
    let (angle_deg, orientation) = classify_stem_orientation(wa.ux, wa.uy);
    let swap = match orientation {
        StemOrientation::Horizontal => pa.y.0 > pb.y.0,
        StemOrientation::Vertical | StemOrientation::Diagonal => pa.x.0 > pb.x.0,
    };
    let (first, second, p_left, p_right) = if swap {
        (wb, wa, pb, pa)
    } else {
        (wa, wb, pa, pb)
    };
    Some((
        overlap,
        DetectedStem {
            id: 0,
            contour_idx_a: first.contour_idx,
            seg_idx_a: first.seg_idx,
            contour_idx_b: second.contour_idx,
            seg_idx_b: second.seg_idx,
            p_left,
            p_right,
            width_upm,
            angle_deg,
            orientation,
        },
    ))
}

/// Detects paired opposing structural stems across `contours` (`SPEC-013 §2.1.1`).
#[must_use]
pub fn detect_structural_stems_in_contours(contours: &[GlyphContour]) -> Vec<DetectedStem> {
    let walls = collect_wall_candidates(contours);
    let polys: Vec<Vec<FontPoint>> = contours
        .iter()
        .filter(|c| c.visible && c.closed && c.nodes.len() >= 3)
        .map(|c| flatten_contour_points(&c.nodes, true, 0.0, 0.0))
        .collect();

    let mut raw_stems: Vec<(f64, DetectedStem)> = Vec::new();
    for i in 0..walls.len() {
        for j in (i + 1)..walls.len() {
            if let Some(pair) = try_pair_wall_candidates(contours, &polys, walls[i], walls[j]) {
                raw_stems.push(pair);
            }
        }
    }

    raw_stems.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap_or(std::cmp::Ordering::Equal));
    let mut deduplicated: Vec<DetectedStem> = Vec::new();
    for (_ov, mut cand) in raw_stems {
        let is_dup = deduplicated.iter().any(|existing| {
            existing.orientation == cand.orientation
                && (existing.width_upm - cand.width_upm).abs() <= STEM_DEDUP_DIST_UPM
                && ((existing.contour_idx_a == cand.contour_idx_a
                    && existing.seg_idx_a == cand.seg_idx_a)
                    || (existing.contour_idx_b == cand.contour_idx_b
                        && existing.seg_idx_b == cand.seg_idx_b))
        });
        if !is_dup {
            cand.id = deduplicated.len();
            deduplicated.push(cand);
        }
    }
    deduplicated
}

fn collect_collinear_wall_node_ids(nodes: &[Node], closed: bool, seg_idx: usize) -> Vec<NodeId> {
    let segs = extract_segments(nodes, closed);
    let Some(seg) = segs.get(seg_idx) else {
        return Vec::new();
    };
    let (s_idx, e_idx) = match *seg {
        ContourSegment::Line {
            start_idx, end_idx, ..
        }
        | ContourSegment::Cubic {
            start_idx, end_idx, ..
        } => (start_idx, end_idx),
    };
    let p0 = nodes[s_idx].point;
    let p1 = nodes[e_idx].point;
    let dx = p1.x.0 - p0.x.0;
    let dy = p1.y.0 - p0.y.0;
    let len = dx.hypot(dy);
    if len <= SOLVER_DET_EPSILON {
        return vec![nodes[s_idx].id, nodes[e_idx].id];
    }
    let nx = -dy / len;
    let ny = dx / len;
    let mut ids = BTreeSet::new();
    ids.insert(nodes[s_idx].id);
    ids.insert(nodes[e_idx].id);
    for n in nodes {
        if n.node_type != NodeType::OffCurve {
            let perp = ((n.point.x.0 - p0.x.0) * nx + (n.point.y.0 - p0.y.0) * ny).abs();
            if perp <= STEM_PERP_COLLINEAR_TOL_UPM {
                ids.insert(n.id);
            }
        }
    }
    ids.into_iter().collect()
}

/// Adjusts a detected structural stem's thickness and angle while servo-scaling connected curve handles (`SPEC-013 §2.1.2`).
pub fn adjust_structural_stem_in_contours(
    contours: &mut [Vec<Node>],
    closed_flags: &[bool],
    stem: &DetectedStem,
    width_delta_upm: f64,
    mode: StemAdjustmentMode,
    angle_delta_deg: f64,
) -> Option<(f64, f64)> {
    debug_assert!(width_delta_upm.is_finite() && angle_delta_deg.is_finite());
    if !width_delta_upm.is_finite() || !angle_delta_deg.is_finite() {
        return None;
    }
    let old_width_upm = stem.width_upm;
    let new_width_upm = (old_width_upm + width_delta_upm).max(STEM_MIN_EDIT_WIDTH_UPM);
    let eff_delta = new_width_upm - old_width_upm;
    if eff_delta.abs() <= SOLVER_DET_EPSILON && angle_delta_deg.abs() <= SOLVER_DET_EPSILON {
        return None;
    }
    let vx = stem.p_right.x.0 - stem.p_left.x.0;
    let vy = stem.p_right.y.0 - stem.p_left.y.0;
    let v_len = vx.hypot(vy);
    if v_len <= SOLVER_DET_EPSILON {
        return None;
    }
    let (nx, ny) = (vx / v_len, vy / v_len);
    let (shift_a, shift_b) = match mode {
        StemAdjustmentMode::SymmetricCenter => (-0.5 * eff_delta, 0.5 * eff_delta),
        StemAdjustmentMode::LockLeft => (0.0, eff_delta),
        StemAdjustmentMode::LockRight => (-eff_delta, 0.0),
    };

    let closed_a = closed_flags
        .get(stem.contour_idx_a)
        .copied()
        .unwrap_or(true);
    let closed_b = closed_flags
        .get(stem.contour_idx_b)
        .copied()
        .unwrap_or(true);
    let ids_a = contours
        .get(stem.contour_idx_a)
        .map(|c| collect_collinear_wall_node_ids(c, closed_a, stem.seg_idx_a))
        .unwrap_or_default();
    let ids_b = contours
        .get(stem.contour_idx_b)
        .map(|c| collect_collinear_wall_node_ids(c, closed_b, stem.seg_idx_b))
        .unwrap_or_default();

    let delta_a = FontPoint::new(shift_a * nx, shift_a * ny);
    let delta_b = FontPoint::new(shift_b * nx, shift_b * ny);
    for id in &ids_a {
        let _ = servo_translate_node_in_contours(contours, closed_flags, *id, delta_a);
    }
    for id in &ids_b {
        if !ids_a.contains(id) {
            let _ = servo_translate_node_in_contours(contours, closed_flags, *id, delta_b);
        }
    }

    if angle_delta_deg.abs() > SOLVER_DET_EPSILON {
        let pivot = FontPoint::new(
            0.5 * (stem.p_left.x.0 + stem.p_right.x.0),
            0.5 * (stem.p_left.y.0 + stem.p_right.y.0),
        );
        let rad = angle_delta_deg * DEG_TO_RAD_F64;
        let (sin_a, cos_a) = rad.sin_cos();
        for id in ids_a.iter().chain(ids_b.iter()) {
            if let Some(pt) = contours
                .iter()
                .flatten()
                .find(|n| n.id == *id)
                .map(|n| n.point)
            {
                let rx = pt.x.0 - pivot.x.0;
                let ry = pt.y.0 - pivot.y.0;
                let rot_delta = FontPoint::new(
                    pivot.x.0 + rx * cos_a - ry * sin_a - pt.x.0,
                    pivot.y.0 + rx * sin_a + ry * cos_a - pt.y.0,
                );
                let _ = servo_translate_node_in_contours(contours, closed_flags, *id, rot_delta);
            }
        }
    }
    Some((old_width_upm, new_width_upm))
}
