//! `SPEC-011 §2.6.2 & §2.7`: Handle-Ratio Percentage HUD (`compute_segment_handle_ratios`)
//! and Diagonal Leg / Tangent Extension & Virtual Apex Snapping (`compute_tangent_extension_snaps`).

use crate::core::bezier_ops::NodeId;
use crate::core::coordinates::FontPoint;
use crate::core::curve_manipulation::{extract_segments, ContourSegment};
use crate::core::fit_and_stroke::solve_cubic_tangent_triangle;
use crate::core::glyph_model::GlyphContour;
use crate::core::math_consts::{
    DIAGONAL_SNAP_MIN_AXIS_DEV_DEG, PERCENT_SCALE_FACTOR, SNAP_LINE_INTERSECT_DET_EPS,
};
use crate::core::nudge_ops::project_point_onto_infinite_line;
use crate::core::outline_audit::min_axis_deviation_deg;
use serde::{Deserialize, Serialize};

/// Handle-length percentages `(ratio_in, ratio_out)` relative to tangent intersection apex `T` (`SPEC-011 §2.6`).
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct SegmentHandleRatioHud {
    pub contour_idx: usize,
    pub seg_idx: usize,
    pub apex: FontPoint,
    pub ratio_in: f64,
    pub ratio_out: f64,
}

/// Snap target classification for diagonal leg ray extensions and virtual apex intersections (`SPEC-011 §2.7`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TangentExtensionSnapKind {
    RayExtension {
        contour_idx: usize,
        seg_idx: usize,
    },
    VirtualApex {
        contour_idx_a: usize,
        seg_idx_a: usize,
        contour_idx_b: usize,
        seg_idx_b: usize,
    },
}

/// Result of `compute_tangent_extension_snaps` (`SPEC-011 §2.7`).
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct TangentExtensionSnap {
    pub snap_point: FontPoint,
    pub distance_upm: f64,
    pub kind: TangentExtensionSnapKind,
}

/// Computes handle-length percentages `(ratio_in, ratio_out)` relative to tangent intersection apex `T` (`SPEC-011 §2.6.2`).
#[must_use]
pub fn compute_segment_handle_ratios_in_contours(
    contours: &[GlyphContour],
) -> Vec<SegmentHandleRatioHud> {
    let mut out = Vec::new();
    for (contour_idx, contour) in contours.iter().enumerate() {
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
                if let Some((apex, t_out, t_in)) = solve_cubic_tangent_triangle(p0, p1, p2, p3) {
                    out.push(SegmentHandleRatioHud {
                        contour_idx,
                        seg_idx,
                        apex,
                        ratio_in: PERCENT_SCALE_FACTOR * t_in,
                        ratio_out: PERCENT_SCALE_FACTOR * t_out,
                    });
                }
            }
        }
    }
    out
}

/// Computes diagonal ray-extension and virtual apex snap candidates within `snap_radius_upm` (`SPEC-011 §2.7`).
#[must_use]
pub fn compute_tangent_extension_snaps_in_contours(
    contours: &[GlyphContour],
    query_pt: FontPoint,
    snap_radius_upm: f64,
) -> Option<TangentExtensionSnap> {
    debug_assert!(snap_radius_upm >= 0.0);
    let mut diag_lines: Vec<(usize, usize, FontPoint, FontPoint)> = Vec::new();
    for (contour_idx, contour) in contours.iter().enumerate() {
        let segments = extract_segments(&contour.nodes, contour.closed);
        for (seg_idx, seg) in segments.iter().enumerate() {
            if let ContourSegment::Line {
                start_idx, end_idx, ..
            } = *seg
            {
                let (a, b) = (contour.nodes[start_idx].point, contour.nodes[end_idx].point);
                let (dx, dy) = (b.x.0 - a.x.0, b.y.0 - a.y.0);
                if dx.hypot(dy) > 1.0
                    && min_axis_deviation_deg(dx, dy, 0.0) > DIAGONAL_SNAP_MIN_AXIS_DEV_DEG
                {
                    diag_lines.push((contour_idx, seg_idx, a, b));
                }
            }
        }
    }

    let mut best_apex: Option<TangentExtensionSnap> = None;
    for m in 0..diag_lines.len() {
        for n in (m + 1)..diag_lines.len() {
            let (ca, sa, a0, a1) = diag_lines[m];
            let (cb, sb, b0, b1) = diag_lines[n];
            if let Some(v_pt) = infinite_line_intersection(a0, a1, b0, b1) {
                let d = v_pt.distance_to(query_pt);
                if d <= snap_radius_upm && best_apex.is_none_or(|prev| d < prev.distance_upm) {
                    best_apex = Some(TangentExtensionSnap {
                        snap_point: v_pt,
                        distance_upm: d,
                        kind: TangentExtensionSnapKind::VirtualApex {
                            contour_idx_a: ca,
                            seg_idx_a: sa,
                            contour_idx_b: cb,
                            seg_idx_b: sb,
                        },
                    });
                }
            }
        }
    }
    if best_apex.is_some() {
        return best_apex;
    }

    let mut best_ray: Option<TangentExtensionSnap> = None;
    for &(contour_idx, seg_idx, a, b) in &diag_lines {
        let proj = project_point_onto_infinite_line(query_pt, a, b);
        let d = proj.distance_to(query_pt);
        if d <= snap_radius_upm && best_ray.is_none_or(|prev| d < prev.distance_upm) {
            best_ray = Some(TangentExtensionSnap {
                snap_point: proj,
                distance_upm: d,
                kind: TangentExtensionSnapKind::RayExtension {
                    contour_idx,
                    seg_idx,
                },
            });
        }
    }
    best_ray
}

fn infinite_line_intersection(
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
    Some(FontPoint::new(a0.x.0 + u * r_x, a0.y.0 + u * r_y))
}

#[derive(Debug, Clone, Copy)]
struct DiagLineEntry {
    contour_idx: usize,
    seg_idx: usize,
    a: FontPoint,
    b: FontPoint,
    incident: bool,
}

fn collect_drag_diag_lines(
    contours: &[GlyphContour],
    dragged_node_id: NodeId,
) -> Vec<DiagLineEntry> {
    let mut diag_lines = Vec::new();
    for (contour_idx, contour) in contours.iter().enumerate() {
        let segments = extract_segments(&contour.nodes, contour.closed);
        for (seg_idx, seg) in segments.iter().enumerate() {
            if let ContourSegment::Line {
                start_idx, end_idx, ..
            } = *seg
            {
                let (a, b) = (contour.nodes[start_idx].point, contour.nodes[end_idx].point);
                let (dx, dy) = (b.x.0 - a.x.0, b.y.0 - a.y.0);
                if dx.hypot(dy) > 1.0
                    && min_axis_deviation_deg(dx, dy, 0.0) > DIAGONAL_SNAP_MIN_AXIS_DEV_DEG
                {
                    let incident = contour.nodes[start_idx].id == dragged_node_id
                        || contour.nodes[end_idx].id == dragged_node_id;
                    diag_lines.push(DiagLineEntry {
                        contour_idx,
                        seg_idx,
                        a,
                        b,
                        incident,
                    });
                }
            }
        }
    }
    diag_lines
}

/// Evaluates diagonal `VirtualApex` and non-incident `RayExtension` snaps during a single-node drag (`SPEC-011 §2.7`).
#[must_use]
pub fn compute_drag_tangent_extension_snap(
    contours: &[GlyphContour],
    query_pt: FontPoint,
    snap_radius_upm: f64,
    dragged_node_id: NodeId,
    lead_start: FontPoint,
) -> Option<TangentExtensionSnap> {
    let diag_lines = collect_drag_diag_lines(contours, dragged_node_id);
    let mut best_apex: Option<TangentExtensionSnap> = None;
    for m in 0..diag_lines.len() {
        for n in (m + 1)..diag_lines.len() {
            let lm = diag_lines[m];
            let ln = diag_lines[n];
            if lm.incident && ln.incident {
                continue;
            }
            if let Some(v_pt) = infinite_line_intersection(lm.a, lm.b, ln.a, ln.b) {
                let d = v_pt.distance_to(query_pt);
                if d <= snap_radius_upm
                    && v_pt.distance_to(lead_start) > snap_radius_upm
                    && best_apex.is_none_or(|prev| d < prev.distance_upm)
                {
                    best_apex = Some(TangentExtensionSnap {
                        snap_point: v_pt,
                        distance_upm: d,
                        kind: TangentExtensionSnapKind::VirtualApex {
                            contour_idx_a: lm.contour_idx,
                            seg_idx_a: lm.seg_idx,
                            contour_idx_b: ln.contour_idx,
                            seg_idx_b: ln.seg_idx,
                        },
                    });
                }
            }
        }
    }
    if best_apex.is_some() {
        return best_apex;
    }
    let mut best_ray: Option<TangentExtensionSnap> = None;
    for line in &diag_lines {
        if line.incident {
            continue;
        }
        let proj = project_point_onto_infinite_line(query_pt, line.a, line.b);
        let d = proj.distance_to(query_pt);
        if d <= snap_radius_upm
            && proj.distance_to(lead_start) > snap_radius_upm
            && best_ray.is_none_or(|prev| d < prev.distance_upm)
        {
            best_ray = Some(TangentExtensionSnap {
                snap_point: proj,
                distance_upm: d,
                kind: TangentExtensionSnapKind::RayExtension {
                    contour_idx: line.contour_idx,
                    seg_idx: line.seg_idx,
                },
            });
        }
    }
    best_ray
}

/// Resolves the `[leg_endpoint, snap_point]` ray segments for rendering `active_tangent_snap` in Pass 4 (`SPEC-011 §2.7`).
#[must_use]
pub fn resolve_tangent_extension_ray_endpoints(
    contours: &[GlyphContour],
    snap: TangentExtensionSnap,
) -> Vec<(FontPoint, FontPoint)> {
    let pick_anchor = |c_idx: usize, s_idx: usize| -> Option<FontPoint> {
        let c = contours.get(c_idx)?;
        let segs = extract_segments(&c.nodes, c.closed);
        let ContourSegment::Line {
            start_idx, end_idx, ..
        } = *segs.get(s_idx)?
        else {
            return None;
        };
        let (a, b) = (c.nodes[start_idx].point, c.nodes[end_idx].point);
        if a.distance_to(snap.snap_point) > b.distance_to(snap.snap_point) {
            Some(a)
        } else {
            Some(b)
        }
    };
    let mut rays = Vec::new();
    match snap.kind {
        TangentExtensionSnapKind::RayExtension {
            contour_idx,
            seg_idx,
        } => {
            if let Some(p) = pick_anchor(contour_idx, seg_idx) {
                rays.push((p, snap.snap_point));
            }
        }
        TangentExtensionSnapKind::VirtualApex {
            contour_idx_a,
            seg_idx_a,
            contour_idx_b,
            seg_idx_b,
        } => {
            if let Some(pa) = pick_anchor(contour_idx_a, seg_idx_a) {
                rays.push((pa, snap.snap_point));
            }
            if let Some(pb) = pick_anchor(contour_idx_b, seg_idx_b) {
                rays.push((pb, snap.snap_point));
            }
        }
    }
    rays
}
