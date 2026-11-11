//! `BooleanPathEngine` Boolean Path Operations (`SPEC-005 §2.7`):
//! `remove_overlap_in_contours` (`Cmd+Shift+O`), `subtract_contours`, and `intersect_contours`.
//!
//! Grounded 1-to-1 in `BooleanPathEngine::removeOverlapSegments:error:` (`spec_ref`),
//! `BooleanPathEngine::removeInnerSegments:` (`spec_ref`), `BooleanPathEngine::removeCornersFromPaths:`
//! (`spec_ref`), `subtractPaths:from:error:` (`spec_ref`), and `intersectPaths:with:error:` (`spec_ref`).

use crate::core::bezier_ops::{compute_signed_area, Node, NodeId, NodeType};
use crate::core::coordinates::FontPoint;
use crate::core::curve_manipulation::{extract_segments, ContourSegment};
use crate::core::knife_ops::intersect_finite_segments;
use crate::core::lasso_and_zoom::point_in_polygon;
use crate::core::path_ops::correct_path_direction_layer;
use crate::core::segment_ops::reverse_contour_nodes;
use crate::tokens::{
    BOOL_COLLINEAR_CROSS_EPS, BOOL_CUBIC_SUBDIV_STEPS, BOOL_CYCLE_CLOSE_BONUS, BOOL_NEWTON_ITERS,
    BOOL_PARAM_EPS, BOOL_PROBE_MAX_LEN_FRACTION, BOOL_PROBE_OFFSET_UPM,
    BOOL_TANGENT_COS_SMOOTH_MIN, BOOL_VERTEX_SNAP_EPS_UPM, COLLINEAR_MIN_LEN_EPSILON,
    CURVE_SOLVE_ACCURACY, DEFAULT_X_HEIGHT_UPM, SOLVER_DET_EPSILON,
};
use kurbo::{CubicBez, Line, ParamCurve, ParamCurveDeriv, ParamCurveNearest, Point};

fn sample_contour_fine(nodes: &[Node]) -> Vec<FontPoint> {
    let mut pts = Vec::new();
    let n = BOOL_CUBIC_SUBDIV_STEPS * 2;
    for seg in extract_segments(nodes, true) {
        match seg {
            ContourSegment::Line { line, .. } => pts.push(FontPoint::new(line.p0.x, line.p0.y)),
            ContourSegment::Cubic { cubic, .. } => {
                for step in 0..n {
                    let p = cubic.eval(step as f64 / n as f64);
                    pts.push(FontPoint::new(p.x, p.y));
                }
            }
        }
    }
    pts
}

/// Computes the signed area of a closed contour by sampling cubic segments (`> 0.0` = CCW, `< 0.0` = CW).
#[must_use]
pub fn contour_signed_area(nodes: &[Node]) -> f64 {
    compute_signed_area(&sample_contour_fine(nodes))
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum BoolSeg {
    Line(Line),
    Cubic(CubicBez),
}

impl BoolSeg {
    fn eval(self, t: f64) -> FontPoint {
        let p = match self { Self::Line(l) => l.eval(t), Self::Cubic(c) => c.eval(t) };
        FontPoint::new(p.x, p.y)
    }

    fn deriv_vec(self, t: f64) -> (f64, f64) {
        match self {
            Self::Line(l) => (l.p1.x - l.p0.x, l.p1.y - l.p0.y),
            Self::Cubic(c) => {
                let d = c.deriv().eval(t);
                if d.x.hypot(d.y) > COLLINEAR_MIN_LEN_EPSILON { (d.x, d.y) } else { (c.p3.x - c.p0.x, c.p3.y - c.p0.y) }
            }
        }
    }

    fn unit_tangent(self, t: f64) -> (f64, f64) {
        let (dx, dy) = self.deriv_vec(t);
        let len = dx.hypot(dy).max(COLLINEAR_MIN_LEN_EPSILON);
        (dx / len, dy / len)
    }

    fn subsegment(self, t0: f64, t1: f64) -> Self {
        match self { Self::Line(l) => Self::Line(l.subsegment(t0..t1)), Self::Cubic(c) => Self::Cubic(c.subsegment(t0..t1)) }
    }

    fn reversed(self) -> Self {
        match self { Self::Line(l) => Self::Line(Line::new(l.p1, l.p0)), Self::Cubic(c) => Self::Cubic(CubicBez::new(c.p3, c.p2, c.p1, c.p0)) }
    }

    fn nearest_t_and_dist(self, pt: FontPoint) -> (f64, f64) {
        let target = Point::new(pt.x.0, pt.y.0);
        let n = match self { Self::Line(l) => l.nearest(target, CURVE_SOLVE_ACCURACY), Self::Cubic(c) => c.nearest(target, CURVE_SOLVE_ACCURACY) };
        (n.t, n.distance_sq.sqrt())
    }

    fn with_endpoints(self, p0: FontPoint, p3: FontPoint) -> Self {
        let (q0, q3) = (Point::new(p0.x.0, p0.y.0), Point::new(p3.x.0, p3.y.0));
        match self { Self::Line(_) => Self::Line(Line::new(q0, q3)), Self::Cubic(c) => Self::Cubic(CubicBez::new(q0, c.p1, c.p2, q3)) }
    }
}

fn extract_bool_segs(nodes: &[Node]) -> Vec<BoolSeg> {
    extract_segments(nodes, true)
        .into_iter()
        .map(|s| match s {
            ContourSegment::Line { line, .. } => BoolSeg::Line(line),
            ContourSegment::Cubic { cubic, .. } => BoolSeg::Cubic(cubic),
        })
        .collect()
}

fn intersect_pair(a: BoolSeg, b: BoolSeg) -> Vec<(f64, f64)> {
    let mut hits = Vec::new();
    for tb in [0.0, 1.0] {
        let (ta, dist) = a.nearest_t_and_dist(b.eval(tb));
        if dist <= BOOL_VERTEX_SNAP_EPS_UPM { hits.push((ta.clamp(0.0, 1.0), tb)); }
    }
    for ta in [0.0, 1.0] {
        let (tb, dist) = b.nearest_t_and_dist(a.eval(ta));
        if dist <= BOOL_VERTEX_SNAP_EPS_UPM { hits.push((ta, tb.clamp(0.0, 1.0))); }
    }
    if a.eval(0.5).distance_to(b.eval(0.5)) <= BOOL_VERTEX_SNAP_EPS_UPM && hits.len() >= 2 {
        return hits;
    }
    if let (BoolSeg::Line(la), BoolSeg::Line(lb)) = (a, b) {
        if let Some((u, v, _)) = intersect_finite_segments(FontPoint::new(la.p0.x, la.p0.y), FontPoint::new(la.p1.x, la.p1.y), FontPoint::new(lb.p0.x, lb.p0.y), FontPoint::new(lb.p1.x, lb.p1.y)) {
            hits.push((u, v));
        }
        return hits;
    }
    let na = if matches!(a, BoolSeg::Cubic(_)) { BOOL_CUBIC_SUBDIV_STEPS } else { 1 };
    let nb = if matches!(b, BoolSeg::Cubic(_)) { BOOL_CUBIC_SUBDIV_STEPS } else { 1 };
    for ia in 0..na {
        let (ta0, ta1) = (ia as f64 / na as f64, (ia + 1) as f64 / na as f64);
        let (pa0, pa1) = (a.eval(ta0), a.eval(ta1));
        for ib in 0..nb {
            let (tb0, tb1) = (ib as f64 / nb as f64, (ib + 1) as f64 / nb as f64);
            if let Some((u, v, _)) = intersect_finite_segments(pa0, pa1, b.eval(tb0), b.eval(tb1)) {
                let mut ta = (ta0 + u * (ta1 - ta0)).clamp(0.0, 1.0);
                let mut tb = (tb0 + v * (tb1 - tb0)).clamp(0.0, 1.0);
                for _ in 0..BOOL_NEWTON_ITERS {
                    let (qa, qb) = (a.eval(ta), b.eval(tb));
                    let (fx, fy) = (qa.x.0 - qb.x.0, qa.y.0 - qb.y.0);
                    let ((dax, day), (dbx, dby)) = (a.deriv_vec(ta), b.deriv_vec(tb));
                    let det = -dax * dby + day * dbx;
                    if det.abs() <= SOLVER_DET_EPSILON { break; }
                    ta = (ta + (fx * dby - fy * dbx) / det).clamp(0.0, 1.0);
                    tb = (tb + (fx * day - fy * dax) / det).clamp(0.0, 1.0);
                }
                if a.eval(ta).distance_to(b.eval(tb)) <= BOOL_VERTEX_SNAP_EPS_UPM * 2.0 {
                    hits.push((ta, tb));
                }
            }
        }
    }
    hits
}

fn snap_vertex(vertices: &mut Vec<FontPoint>, pt: FontPoint) -> usize {
    if let Some((idx, _)) = vertices.iter().enumerate().find(|(_, v)| v.distance_to(pt) <= BOOL_VERTEX_SNAP_EPS_UPM) {
        idx
    } else {
        vertices.push(pt);
        vertices.len() - 1
    }
}

fn merge_collinear_lines(s1: BoolSeg, s2: BoolSeg) -> Option<BoolSeg> {
    if let (BoolSeg::Line(l1), BoolSeg::Line(l2)) = (s1, s2) {
        let ((dx1, dy1), (dx2, dy2)) = ((l1.p1.x - l1.p0.x, l1.p1.y - l1.p0.y), (l2.p1.x - l2.p0.x, l2.p1.y - l2.p0.y));
        let (len1, len2) = (dx1.hypot(dy1), dx2.hypot(dy2));
        if len1 > COLLINEAR_MIN_LEN_EPSILON && len2 > COLLINEAR_MIN_LEN_EPSILON && (dx1 * dy2 - dy1 * dx2).abs() <= BOOL_COLLINEAR_CROSS_EPS * len1 * len2 && (dx1 * dx2 + dy1 * dy2) > 0.0 {
            return Some(BoolSeg::Line(Line::new(l1.p0, l2.p1)));
        }
    }
    None
}

fn cycle_to_nodes(cycle: &mut Vec<BoolSeg>, next_node_id: &mut u64) -> Vec<Node> {
    let mut i = 0;
    while i + 1 < cycle.len() && cycle.len() > 2 {
        if let Some(m) = merge_collinear_lines(cycle[i], cycle[i + 1]) {
            cycle[i] = m;
            cycle.remove(i + 1);
        } else {
            i += 1;
        }
    }
    if cycle.len() > 2 {
        if let Some(m) = merge_collinear_lines(*cycle.last().unwrap(), cycle[0]) {
            cycle[0] = m;
            cycle.pop();
        }
    }
    let mut nodes = Vec::with_capacity(cycle.len() * 3);
    let m = cycle.len();
    for k in 0..m {
        let (prev, curr) = (cycle[(k + m - 1) % m], cycle[k]);
        let ((tx1, ty1), (tx2, ty2)) = (prev.unit_tangent(1.0), curr.unit_tangent(0.0));
        let is_smooth = matches!((prev, curr), (BoolSeg::Cubic(_), BoolSeg::Cubic(_))) && tx1 * tx2 + ty1 * ty2 >= BOOL_TANGENT_COS_SMOOTH_MIN;
        let p0 = curr.eval(0.0);
        nodes.push(Node::new(NodeId(*next_node_id), p0.x.0, p0.y.0, if is_smooth { NodeType::Smooth } else { NodeType::Corner }));
        *next_node_id += 1;
        if let BoolSeg::Cubic(c) = curr {
            nodes.push(Node::new(NodeId(*next_node_id), c.p1.x, c.p1.y, NodeType::OffCurve));
            nodes.push(Node::new(NodeId(*next_node_id + 1), c.p2.x, c.p2.y, NodeType::OffCurve));
            *next_node_id += 2;
        }
    }
    nodes
}

fn execute_boolean_pipeline<F>(all_segs: &[BoolSeg], inside_pred: F, next_node_id: &mut u64) -> Vec<Vec<Node>>
where
    F: Fn(FontPoint) -> bool,
{
    let mut split_params: Vec<Vec<f64>> = vec![vec![0.0, 1.0]; all_segs.len()];
    for i in 0..all_segs.len() {
        for j in (i + 1)..all_segs.len() {
            for (ta, tb) in intersect_pair(all_segs[i], all_segs[j]) {
                split_params[i].push(ta);
                split_params[j].push(tb);
            }
        }
    }

    let mut vertices: Vec<FontPoint> = Vec::new();
    let mut kept: Vec<(usize, usize, BoolSeg)> = Vec::new();
    for (idx, &seg) in all_segs.iter().enumerate() {
        let params = &mut split_params[idx];
        params.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
        let mut clean_t = vec![0.0];
        for &t in params.iter() {
            let (tc, prev_t) = (t.clamp(0.0, 1.0), *clean_t.last().unwrap());
            if tc - prev_t > BOOL_PARAM_EPS && seg.eval(tc).distance_to(seg.eval(prev_t)) > BOOL_VERTEX_SNAP_EPS_UPM {
                clean_t.push(tc);
            }
        }
        let last_t = *clean_t.last().unwrap();
        if (1.0 - last_t) <= BOOL_PARAM_EPS || seg.eval(1.0).distance_to(seg.eval(last_t)) <= BOOL_VERTEX_SNAP_EPS_UPM {
            *clean_t.last_mut().unwrap() = 1.0;
        } else {
            clean_t.push(1.0);
        }
        for w in clean_t.windows(2) {
            let sub = seg.subsegment(w[0], w[1]);
            let (v0, v1) = (snap_vertex(&mut vertices, sub.eval(0.0)), snap_vertex(&mut vertices, sub.eval(1.0)));
            if v0 == v1 { continue; }
            let snapped = sub.with_endpoints(vertices[v0], vertices[v1]);
            let mid = snapped.eval(0.5);
            let (tx, ty) = snapped.unit_tangent(0.5);
            let delta = BOOL_PROBE_OFFSET_UPM.min(vertices[v0].distance_to(vertices[v1]) * BOOL_PROBE_MAX_LEN_FRACTION).max(BOOL_PARAM_EPS);
            let in_l = inside_pred(FontPoint::new(mid.x.0 - ty * delta, mid.y.0 + tx * delta));
            let in_r = inside_pred(FontPoint::new(mid.x.0 + ty * delta, mid.y.0 - tx * delta));
            let cand = if in_l && !in_r { Some((v0, v1, snapped)) } else if !in_l && in_r { Some((v1, v0, snapped.reversed())) } else { None };
            if let Some((sv, ev, cseg)) = cand {
                if !kept.iter().any(|&(ks, ke, kseg)| ks == sv && ke == ev && kseg.eval(0.5).distance_to(mid) <= BOOL_VERTEX_SNAP_EPS_UPM * 2.0) {
                    kept.push((sv, ev, cseg));
                }
            }
        }
    }

    let mut used = vec![false; kept.len()];
    let mut result_contours: Vec<Vec<Node>> = Vec::new();
    for start_idx in 0..kept.len() {
        if used[start_idx] { continue; }
        used[start_idx] = true;
        let (origin_vid, mut curr_vid, first_seg) = kept[start_idx];
        let mut prev_tan = first_seg.unit_tangent(1.0);
        let mut cycle = vec![first_seg];

        while curr_vid != origin_vid {
            let next_opt = (0..kept.len())
                .filter(|&ci| !used[ci] && kept[ci].0 == curr_vid)
                .max_by(|&a, &b| {
                    let score = |ci: usize| {
                        let (nx, ny) = kept[ci].2.unit_tangent(0.0);
                        let turn = (prev_tan.0 * ny - prev_tan.1 * nx).atan2(prev_tan.0 * nx + prev_tan.1 * ny);
                        if kept[ci].1 == origin_vid { turn + BOOL_CYCLE_CLOSE_BONUS } else { turn }
                    };
                    score(a).partial_cmp(&score(b)).unwrap_or(std::cmp::Ordering::Equal)
                });
            if let Some(next_idx) = next_opt {
                used[next_idx] = true;
                curr_vid = kept[next_idx].1;
                prev_tan = kept[next_idx].2.unit_tangent(1.0);
                cycle.push(kept[next_idx].2);
            } else {
                break;
            }
        }
        if curr_vid == origin_vid && cycle.len() >= 2 {
            let nodes = cycle_to_nodes(&mut cycle, next_node_id);
            if nodes.iter().filter(|n| n.node_type != NodeType::OffCurve).count() >= 2 {
                result_contours.push(nodes);
            }
        }
    }

    let closed = vec![true; result_contours.len()];
    let _ = correct_path_direction_layer(&mut result_contours, &closed, 0.0, DEFAULT_X_HEIGHT_UPM * 0.5);
    result_contours
}

fn build_signed_polys(contours: &[Vec<Node>]) -> Vec<(Vec<FontPoint>, i32)> {
    let polys: Vec<Vec<FontPoint>> = contours.iter().map(|c| sample_contour_fine(c)).collect();
    let areas: Vec<f64> = polys.iter().map(|p| compute_signed_area(p)).collect();
    (0..contours.len())
        .map(|i| {
            let test_pt = contours[i].iter().find(|n| n.node_type != NodeType::OffCurve).map_or(FontPoint::new(0.0, 0.0), |n| n.point);
            let is_enclosed_cw = areas[i] < 0.0
                && (0..contours.len()).any(|j| j != i && areas[j].abs() > areas[i].abs() && point_in_polygon(test_pt, &polys[j]));
            (polys[i].clone(), if is_enclosed_cw { -1 } else { 1 })
        })
        .collect()
}

fn eval_signed_winding(pt: FontPoint, polys: &[(Vec<FontPoint>, i32)]) -> i32 {
    polys.iter().filter(|(p, _)| point_in_polygon(pt, p)).map(|(_, s)| *s).sum()
}

/// Removes overlaps across closed contours in `contours` (`BooleanPathEngine::removeOverlapPaths:error:`),
/// leaving open contours untouched.
pub fn remove_overlap_in_contours(
    contours: &mut Vec<Vec<Node>>,
    closed_flags: &mut Vec<bool>,
    selected_nodes: &[NodeId],
    next_node_id: &mut u64,
) -> bool {
    let selected_closed: Vec<usize> = (0..contours.len())
        .filter(|&i| closed_flags.get(i).copied().unwrap_or(true) && contours[i].iter().any(|n| selected_nodes.contains(&n.id)))
        .collect();
    let use_subset = selected_closed.len() >= 2;
    let active_indices: Vec<usize> = (0..contours.len())
        .filter(|&i| closed_flags.get(i).copied().unwrap_or(true) && (!use_subset || selected_closed.contains(&i)))
        .collect();
    if active_indices.is_empty() { return false; }

    let mut working_closed: Vec<Vec<Node>> = active_indices.iter().map(|&i| contours[i].clone()).collect();
    let signed_polys = build_signed_polys(&working_closed);
    for (c, (_, sign)) in working_closed.iter_mut().zip(signed_polys.iter()) {
        let area = contour_signed_area(c);
        if (*sign > 0 && area < 0.0) || (*sign < 0 && area > 0.0) { reverse_contour_nodes(c, true); }
    }

    let contour_segs: Vec<Vec<BoolSeg>> = working_closed.iter().map(|c| extract_bool_segs(c)).collect();
    let has_cross_hit = (0..contour_segs.len()).any(|i| {
        ((i + 1)..contour_segs.len()).any(|j| contour_segs[i].iter().any(|&sa| contour_segs[j].iter().any(|&sb| !intersect_pair(sa, sb).is_empty())))
    });
    let has_self_hit = contour_segs.iter().any(|segs| {
        (0..segs.len()).any(|a| ((a + 1)..segs.len()).any(|b| {
            intersect_pair(segs[a], segs[b]).into_iter().any(|(ta, tb)| (ta > BOOL_PARAM_EPS && ta < 1.0 - BOOL_PARAM_EPS) || (tb > BOOL_PARAM_EPS && tb < 1.0 - BOOL_PARAM_EPS))
        }))
    });
    let has_contained_pos = (0..working_closed.len()).any(|i| {
        signed_polys[i].1 > 0 && working_closed[i].iter().find(|n| n.node_type != NodeType::OffCurve).is_some_and(|n| {
            (0..working_closed.len()).any(|j| j != i && signed_polys[j].1 > 0 && point_in_polygon(n.point, &signed_polys[j].0))
        })
    });
    if !has_cross_hit && !has_self_hit && !has_contained_pos { return false; }

    let all_segs: Vec<BoolSeg> = contour_segs.into_iter().flatten().collect();
    let merged = execute_boolean_pipeline(&all_segs, |pt| eval_signed_winding(pt, &signed_polys) > 0, next_node_id);
    if merged.is_empty() { return false; }

    let mut new_contours = Vec::new();
    let mut new_flags = Vec::new();
    let mut inserted_merged = false;
    for (i, orig) in contours.iter().enumerate() {
        if active_indices.contains(&i) {
            if !inserted_merged {
                inserted_merged = true;
                for m in &merged {
                    new_contours.push(m.clone());
                    new_flags.push(true);
                }
            }
        } else {
            new_contours.push(orig.clone());
            new_flags.push(closed_flags.get(i).copied().unwrap_or(true));
        }
    }
    *contours = new_contours;
    *closed_flags = new_flags;
    true
}

/// Subtracts `clip` closed contours from `subject` closed contours (`BooleanPathEngine::subtractPaths:from:error:`).
#[must_use]
pub fn subtract_contours(subject: &[Vec<Node>], clip: &[Vec<Node>], next_node_id: &mut u64) -> Vec<Vec<Node>> {
    let (sub_polys, clip_polys) = (build_signed_polys(subject), build_signed_polys(clip));
    let all_segs: Vec<BoolSeg> = subject.iter().chain(clip.iter()).flat_map(|c| extract_bool_segs(c)).collect();
    execute_boolean_pipeline(&all_segs, |pt| eval_signed_winding(pt, &sub_polys) > 0 && eval_signed_winding(pt, &clip_polys) <= 0, next_node_id)
}

/// Intersects `subject` closed contours with `clip` closed contours (`BooleanPathEngine::intersectPaths:with:error:`).
#[must_use]
pub fn intersect_contours(subject: &[Vec<Node>], clip: &[Vec<Node>], next_node_id: &mut u64) -> Vec<Vec<Node>> {
    let (sub_polys, clip_polys) = (build_signed_polys(subject), build_signed_polys(clip));
    let all_segs: Vec<BoolSeg> = subject.iter().chain(clip.iter()).flat_map(|c| extract_bool_segs(c)).collect();
    execute_boolean_pipeline(&all_segs, |pt| eval_signed_winding(pt, &sub_polys) > 0 && eval_signed_winding(pt, &clip_polys) > 0, next_node_id)
}
