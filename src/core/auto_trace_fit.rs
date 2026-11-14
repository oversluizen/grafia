//! Polygon simplification, corner chamfer collapse, ray-cast winding classification,
//! and extremum insertion for `SPEC-009 §2.5` Bitmap & Camera Auto-Trace (`< 399` eSLOC).

use crate::core::auto_trace_bezier::{
    fit_cubic_chain, included_angle_deg, is_collinear_chain, unit_vec,
};
use crate::core::host_requests::AutoTraceConfig;
use crate::core::math_consts::{
    DEG_TO_RAD_F64, SOLVER_DET_EPSILON, TRACE_DEDUP_DIST_EPS_PX, TRACE_MIN_CORNER_ARM_PX,
    TRACE_MIN_LOOP_VERTICES, TRACE_STRAIGHT_CHORD_TOL_FACTOR,
};
use crate::core::{
    add_extremes_to_contours, contour_signed_area, point_in_polygon, reverse_contour_nodes,
    simplify_polyline_rdp, ContourId, FontPoint, GlyphContour, Node, NodeId, NodeType,
};
use kurbo::Point;

#[must_use]
pub(crate) fn poly_signed_area(pts: &[(f64, f64)]) -> f64 {
    if pts.len() < TRACE_MIN_LOOP_VERTICES {
        return 0.0;
    }
    let mut acc = 0.0;
    for i in 0..pts.len() {
        let (x0, y0) = pts[i];
        let (x1, y1) = pts[(i + 1) % pts.len()];
        acc += x0 * y1 - x1 * y0;
    }
    acc * 0.5
}

#[must_use]
pub(crate) fn simplify_closed_loop_px(loop_px: &[(f64, f64)], tol_px: f64) -> Vec<(f64, f64)> {
    let mut clean: Vec<(f64, f64)> = Vec::with_capacity(loop_px.len());
    for &pt in loop_px {
        let keep = clean
            .last()
            .is_none_or(|prev| (pt.0 - prev.0).hypot(pt.1 - prev.1) > TRACE_DEDUP_DIST_EPS_PX);
        if keep {
            clean.push(pt);
        }
    }
    if clean.len() > 1 {
        let first = clean[0];
        let last = clean[clean.len() - 1];
        if (first.0 - last.0).hypot(first.1 - last.1) <= TRACE_DEDUP_DIST_EPS_PX {
            clean.pop();
        }
    }
    if clean.len() <= TRACE_MIN_LOOP_VERTICES {
        return clean;
    }

    let mut min_idx = 0;
    let mut max_idx = 0;
    for (idx, &(x, y)) in clean.iter().enumerate() {
        if (x, y) < clean[min_idx] {
            min_idx = idx;
        }
        if (x, y) > clean[max_idx] {
            max_idx = idx;
        }
    }
    if min_idx == max_idx {
        max_idx = (min_idx + clean.len() / 2) % clean.len();
    }
    let n = clean.len();
    let mut chain_a = Vec::new();
    let mut cur = min_idx;
    loop {
        chain_a.push(FontPoint::new(clean[cur].0, clean[cur].1));
        if cur == max_idx {
            break;
        }
        cur = (cur + 1) % n;
    }
    let mut chain_b = Vec::new();
    cur = max_idx;
    loop {
        chain_b.push(FontPoint::new(clean[cur].0, clean[cur].1));
        if cur == min_idx {
            break;
        }
        cur = (cur + 1) % n;
    }
    let simp_a = simplify_polyline_rdp(&chain_a, tol_px);
    let simp_b = simplify_polyline_rdp(&chain_b, tol_px);
    let mut merged: Vec<(f64, f64)> = Vec::with_capacity(simp_a.len() + simp_b.len());
    for p in simp_a.iter().take(simp_a.len().saturating_sub(1)) {
        merged.push((p.x.0, p.y.0));
    }
    for p in simp_b.iter().take(simp_b.len().saturating_sub(1)) {
        merged.push((p.x.0, p.y.0));
    }
    collapse_corner_chamfers_px(&merged, tol_px * 2.0 + TRACE_MIN_CORNER_ARM_PX)
}

fn collapse_corner_chamfers_px(pts: &[(f64, f64)], chamfer_max_px: f64) -> Vec<(f64, f64)> {
    if pts.len() <= 4 {
        return pts.to_vec();
    }
    let n = pts.len();
    let mut out = Vec::with_capacity(n);
    let mut skip_next = false;
    for i in 0..n {
        if skip_next {
            skip_next = false;
            continue;
        }
        let j = (i + 1) % n;
        let edge_len = (pts[j].0 - pts[i].0).hypot(pts[j].1 - pts[i].1);
        let prev = pts[(i + n - 1) % n];
        let next = pts[(j + 1) % n];
        let prev_len = (pts[i].0 - prev.0).hypot(pts[i].1 - prev.1);
        let next_len = (next.0 - pts[j].0).hypot(next.1 - pts[j].1);
        if edge_len <= chamfer_max_px
            && prev_len > chamfer_max_px * 2.0
            && next_len > chamfer_max_px * 2.0
        {
            let corner = intersect_lines_or_mid(prev, pts[i], pts[j], next);
            out.push(corner);
            if j == 0 && !out.is_empty() {
                out.remove(0);
            } else {
                skip_next = true;
            }
        } else {
            out.push(pts[i]);
        }
    }
    out
}

fn intersect_lines_or_mid(
    a0: (f64, f64),
    a1: (f64, f64),
    b0: (f64, f64),
    b1: (f64, f64),
) -> (f64, f64) {
    let dax = a1.0 - a0.0;
    let day = a1.1 - a0.1;
    let dbx = b1.0 - b0.0;
    let dby = b1.1 - b0.1;
    let det = dax * dby - day * dbx;
    let mid = ((a1.0 + b0.0) * 0.5, (a1.1 + b0.1) * 0.5);
    if det.abs() <= SOLVER_DET_EPSILON {
        return mid;
    }
    let t = ((b0.0 - a0.0) * dby - (b0.1 - a0.1) * dbx) / det;
    let ix = a0.0 + t * dax;
    let iy = a0.1 + t * day;
    let max_hop = (a1.0 - b0.0).hypot(a1.1 - b0.1) * 2.0 + 2.0;
    if (ix - mid.0).hypot(iy - mid.1) <= max_hop {
        (ix, iy)
    } else {
        mid
    }
}

fn extract_chain_between_corners(pts: &[Point], s: usize, e: usize) -> Vec<Point> {
    let n = pts.len();
    let mut chain = Vec::new();
    let mut cur = s;
    loop {
        chain.push(pts[cur]);
        cur = (cur + 1) % n;
        if cur == e {
            chain.push(pts[e]);
            break;
        }
    }
    chain
}

fn fit_polygon_to_contour_nodes(
    pts: &[Point],
    config: &AutoTraceConfig,
    line_tol: f64,
    fit_err: f64,
    next_node_id: &mut u64,
) -> Vec<Node> {
    let n = pts.len();
    let mut corners: Vec<usize> = (0..n)
        .filter(|&i| {
            included_angle_deg(pts[(i + n - 1) % n], pts[i], pts[(i + 1) % n])
                < config.corner_angle_deg
        })
        .collect();
    let smooth_loop = corners.is_empty();
    if smooth_loop {
        corners.push(0);
        if n >= 4 {
            corners.push(n / 2);
        }
    }

    let mut nodes: Vec<Node> = Vec::new();
    for k in 0..corners.len() {
        let (s, e) = (corners[k], corners[(k + 1) % corners.len()]);
        let chain = extract_chain_between_corners(pts, s, e);
        let start_type = if smooth_loop {
            NodeType::Smooth
        } else {
            NodeType::Corner
        };
        if !smooth_loop && is_collinear_chain(&chain, line_tol) {
            let id = NodeId(*next_node_id);
            *next_node_id += 1;
            nodes.push(Node::new(id, chain[0].x, chain[0].y, start_type));
        } else {
            let tan_l = if smooth_loop {
                unit_vec(
                    pts[(s + 1) % n].x - pts[(s + n - 1) % n].x,
                    pts[(s + 1) % n].y - pts[(s + n - 1) % n].y,
                )
            } else {
                unit_vec(chain[1].x - chain[0].x, chain[1].y - chain[0].y)
            };
            let tan_r = if smooth_loop {
                let dir = unit_vec(
                    pts[(e + 1) % n].x - pts[(e + n - 1) % n].x,
                    pts[(e + 1) % n].y - pts[(e + n - 1) % n].y,
                );
                (-dir.0, -dir.1)
            } else {
                let m = chain.len();
                unit_vec(
                    chain[m - 2].x - chain[m - 1].x,
                    chain[m - 2].y - chain[m - 1].y,
                )
            };
            let mut segs = Vec::new();
            fit_cubic_chain(&chain, tan_l, tan_r, fit_err * fit_err, 0, &mut segs);
            for (seg_idx, seg) in segs.iter().enumerate() {
                let on_type = if seg_idx == 0 {
                    start_type
                } else {
                    NodeType::Smooth
                };
                let (on_id, h1_id, h2_id) = (
                    NodeId(*next_node_id),
                    NodeId(*next_node_id + 1),
                    NodeId(*next_node_id + 2),
                );
                *next_node_id += 3;
                nodes.push(Node::new(on_id, seg.p0.x, seg.p0.y, on_type));
                nodes.push(Node::new(h1_id, seg.p1.x, seg.p1.y, NodeType::OffCurve));
                nodes.push(Node::new(h2_id, seg.p2.x, seg.p2.y, NodeType::OffCurve));
            }
        }
    }
    nodes
}

fn orient_traced_polygons_by_nesting(polys_upm: &mut [Vec<FontPoint>]) -> Vec<bool> {
    let snapshot_polys = polys_upm.to_vec();
    let mut target_ccw = Vec::with_capacity(polys_upm.len());
    for (i, poly) in polys_upm.iter_mut().enumerate() {
        let probe = poly[0];
        let depth = snapshot_polys
            .iter()
            .enumerate()
            .filter(|(j, other)| *j != i && point_in_polygon(probe, other))
            .count();
        let want_ccw = depth % 2 == 0;
        let area = poly_signed_area(
            &poly
                .iter()
                .map(|p| (p.x.0, p.y.0))
                .collect::<Vec<(f64, f64)>>(),
        );
        if (want_ccw && area < 0.0) || (!want_ccw && area > 0.0) {
            poly.reverse();
        }
        target_ccw.push(want_ccw);
    }
    target_ccw
}

pub(crate) fn build_traced_contours_upm(
    loops_px: Vec<Vec<(f64, f64)>>,
    ch: u32,
    pos: FontPoint,
    scale_x: f64,
    scale_y: f64,
    rotation_deg: f64,
    config: &AutoTraceConfig,
) -> Vec<GlyphContour> {
    let rad = rotation_deg * DEG_TO_RAD_F64;
    let (cr, sr) = (rad.cos(), rad.sin());
    let to_upm = |px: f64, py: f64| -> FontPoint {
        let lx = px * scale_x;
        let ly = (f64::from(ch) - py) * scale_y;
        FontPoint::new(pos.x.0 + lx * cr - ly * sr, pos.y.0 + lx * sr + ly * cr)
    };

    let mut polys_upm: Vec<Vec<FontPoint>> = loops_px
        .into_iter()
        .filter(|lp| lp.len() >= TRACE_MIN_LOOP_VERTICES)
        .map(|lp| lp.into_iter().map(|(x, y)| to_upm(x, y)).collect())
        .collect();
    if polys_upm.is_empty() {
        return Vec::new();
    }

    let target_ccw = orient_traced_polygons_by_nesting(&mut polys_upm);
    let mut next_node_id = 1_u64;
    let fit_err = config.fit_error_upm.max(TRACE_STRAIGHT_CHORD_TOL_FACTOR);
    let line_tol = fit_err * TRACE_STRAIGHT_CHORD_TOL_FACTOR;
    let mut contours = Vec::with_capacity(polys_upm.len());

    for (c_idx, (poly, want_ccw)) in polys_upm.into_iter().zip(target_ccw).enumerate() {
        let pts: Vec<Point> = poly.iter().map(|p| Point::new(p.x.0, p.y.0)).collect();
        let mut nodes =
            fit_polygon_to_contour_nodes(&pts, config, line_tol, fit_err, &mut next_node_id);
        if nodes.len() >= TRACE_MIN_LOOP_VERTICES {
            let signed = contour_signed_area(&nodes);
            if (want_ccw && signed < 0.0) || (!want_ccw && signed > 0.0) {
                reverse_contour_nodes(&mut nodes, true);
            }
            contours.push((want_ccw, GlyphContour::new(ContourId(c_idx + 1), nodes)));
        }
    }

    let mut raw_vecs: Vec<Vec<Node>> = contours.iter().map(|(_, c)| c.nodes.clone()).collect();
    if config.snap_extrema && !raw_vecs.is_empty() {
        let mut flags = vec![true; raw_vecs.len()];
        let _ = add_extremes_to_contours(
            &mut raw_vecs,
            &mut flags,
            &[],
            false,
            &mut next_node_id,
            0.0,
        );
    }
    contours
        .into_iter()
        .zip(raw_vecs)
        .map(|((want_ccw, mut gc), mut nodes)| {
            let signed = contour_signed_area(&nodes);
            if (want_ccw && signed < 0.0) || (!want_ccw && signed > 0.0) {
                reverse_contour_nodes(&mut nodes, true);
            }
            gc.nodes = nodes;
            gc.closed = true;
            gc
        })
        .collect()
}
