//! Segment representation, pairwise Newton-Raphson intersection solver, and cycle reconstruction
//! for `BooleanPathEngine` (`SPEC-005 §2.7`).

use crate::core::bezier_ops::{Node, NodeId, NodeType};
use crate::core::coordinates::FontPoint;
use crate::core::curve_manipulation::{extract_segments, ContourSegment};
use crate::core::knife_ops::intersect_finite_segments;
use crate::tokens::{
    BOOL_COLLINEAR_CROSS_EPS, BOOL_CUBIC_SUBDIV_STEPS, BOOL_NEWTON_ITERS,
    BOOL_TANGENT_COS_SMOOTH_MIN, BOOL_VERTEX_SNAP_EPS_UPM, COLLINEAR_MIN_LEN_EPSILON,
    CURVE_SOLVE_ACCURACY, SOLVER_DET_EPSILON,
};
use kurbo::{CubicBez, Line, ParamCurve, ParamCurveDeriv, ParamCurveNearest, Point};

#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) enum BoolSeg {
    Line(Line),
    Cubic(CubicBez),
}

impl BoolSeg {
    pub(crate) fn eval(self, t: f64) -> FontPoint {
        let p = match self {
            Self::Line(l) => l.eval(t),
            Self::Cubic(c) => c.eval(t),
        };
        FontPoint::new(p.x, p.y)
    }

    pub(crate) fn deriv_vec(self, t: f64) -> (f64, f64) {
        match self {
            Self::Line(l) => (l.p1.x - l.p0.x, l.p1.y - l.p0.y),
            Self::Cubic(c) => {
                let d = c.deriv().eval(t);
                if d.x.hypot(d.y) > COLLINEAR_MIN_LEN_EPSILON {
                    (d.x, d.y)
                } else {
                    (c.p3.x - c.p0.x, c.p3.y - c.p0.y)
                }
            }
        }
    }

    pub(crate) fn unit_tangent(self, t: f64) -> (f64, f64) {
        let (dx, dy) = self.deriv_vec(t);
        let len = dx.hypot(dy).max(COLLINEAR_MIN_LEN_EPSILON);
        (dx / len, dy / len)
    }

    pub(crate) fn subsegment(self, t0: f64, t1: f64) -> Self {
        match self {
            Self::Line(l) => Self::Line(l.subsegment(t0..t1)),
            Self::Cubic(c) => Self::Cubic(c.subsegment(t0..t1)),
        }
    }

    pub(crate) fn reversed(self) -> Self {
        match self {
            Self::Line(l) => Self::Line(Line::new(l.p1, l.p0)),
            Self::Cubic(c) => Self::Cubic(CubicBez::new(c.p3, c.p2, c.p1, c.p0)),
        }
    }

    pub(crate) fn nearest_t_and_dist(self, pt: FontPoint) -> (f64, f64) {
        let target = Point::new(pt.x.0, pt.y.0);
        let n = match self {
            Self::Line(l) => l.nearest(target, CURVE_SOLVE_ACCURACY),
            Self::Cubic(c) => c.nearest(target, CURVE_SOLVE_ACCURACY),
        };
        (n.t, n.distance_sq.sqrt())
    }

    pub(crate) fn with_endpoints(self, p0: FontPoint, p3: FontPoint) -> Self {
        let (q0, q3) = (Point::new(p0.x.0, p0.y.0), Point::new(p3.x.0, p3.y.0));
        match self {
            Self::Line(_) => Self::Line(Line::new(q0, q3)),
            Self::Cubic(c) => Self::Cubic(CubicBez::new(q0, c.p1, c.p2, q3)),
        }
    }
}

pub(crate) fn extract_bool_segs(nodes: &[Node]) -> Vec<BoolSeg> {
    extract_segments(nodes, true)
        .into_iter()
        .map(|s| match s {
            ContourSegment::Line { line, .. } => BoolSeg::Line(line),
            ContourSegment::Cubic { cubic, .. } => BoolSeg::Cubic(cubic),
        })
        .collect()
}

pub(crate) fn intersect_pair(a: BoolSeg, b: BoolSeg) -> Vec<(f64, f64)> {
    let mut hits = Vec::new();
    for tb in [0.0, 1.0] {
        let (ta, dist) = a.nearest_t_and_dist(b.eval(tb));
        if dist <= BOOL_VERTEX_SNAP_EPS_UPM {
            hits.push((ta.clamp(0.0, 1.0), tb));
        }
    }
    for ta in [0.0, 1.0] {
        let (tb, dist) = b.nearest_t_and_dist(a.eval(ta));
        if dist <= BOOL_VERTEX_SNAP_EPS_UPM {
            hits.push((ta, tb.clamp(0.0, 1.0)));
        }
    }
    if a.eval(0.5).distance_to(b.eval(0.5)) <= BOOL_VERTEX_SNAP_EPS_UPM && hits.len() >= 2 {
        return hits;
    }
    if let (BoolSeg::Line(la), BoolSeg::Line(lb)) = (a, b) {
        if let Some((u, v, _)) = intersect_finite_segments(
            FontPoint::new(la.p0.x, la.p0.y),
            FontPoint::new(la.p1.x, la.p1.y),
            FontPoint::new(lb.p0.x, lb.p0.y),
            FontPoint::new(lb.p1.x, lb.p1.y),
        ) {
            hits.push((u, v));
        }
        return hits;
    }
    let na = if matches!(a, BoolSeg::Cubic(_)) {
        BOOL_CUBIC_SUBDIV_STEPS
    } else {
        1
    };
    let nb = if matches!(b, BoolSeg::Cubic(_)) {
        BOOL_CUBIC_SUBDIV_STEPS
    } else {
        1
    };
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
                    if det.abs() <= SOLVER_DET_EPSILON {
                        break;
                    }
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

pub(crate) fn snap_vertex(vertices: &mut Vec<FontPoint>, pt: FontPoint) -> usize {
    if let Some((idx, _)) = vertices
        .iter()
        .enumerate()
        .find(|(_, v)| v.distance_to(pt) <= BOOL_VERTEX_SNAP_EPS_UPM)
    {
        idx
    } else {
        vertices.push(pt);
        vertices.len() - 1
    }
}

fn merge_collinear_lines(s1: BoolSeg, s2: BoolSeg) -> Option<BoolSeg> {
    if let (BoolSeg::Line(l1), BoolSeg::Line(l2)) = (s1, s2) {
        let ((dx1, dy1), (dx2, dy2)) = (
            (l1.p1.x - l1.p0.x, l1.p1.y - l1.p0.y),
            (l2.p1.x - l2.p0.x, l2.p1.y - l2.p0.y),
        );
        let (len1, len2) = (dx1.hypot(dy1), dx2.hypot(dy2));
        if len1 > COLLINEAR_MIN_LEN_EPSILON
            && len2 > COLLINEAR_MIN_LEN_EPSILON
            && (dx1 * dy2 - dy1 * dx2).abs() <= BOOL_COLLINEAR_CROSS_EPS * len1 * len2
            && (dx1 * dx2 + dy1 * dy2) > 0.0
        {
            return Some(BoolSeg::Line(Line::new(l1.p0, l2.p1)));
        }
    }
    None
}

pub(crate) fn cycle_to_nodes(cycle: &mut Vec<BoolSeg>, next_node_id: &mut u64) -> Vec<Node> {
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
        let is_smooth = matches!((prev, curr), (BoolSeg::Cubic(_), BoolSeg::Cubic(_)))
            && tx1 * tx2 + ty1 * ty2 >= BOOL_TANGENT_COS_SMOOTH_MIN;
        let p0 = curr.eval(0.0);
        nodes.push(Node::new(
            NodeId(*next_node_id),
            p0.x.0,
            p0.y.0,
            if is_smooth {
                NodeType::Smooth
            } else {
                NodeType::Corner
            },
        ));
        *next_node_id += 1;
        if let BoolSeg::Cubic(c) = curr {
            nodes.push(Node::new(
                NodeId(*next_node_id),
                c.p1.x,
                c.p1.y,
                NodeType::OffCurve,
            ));
            nodes.push(Node::new(
                NodeId(*next_node_id + 1),
                c.p2.x,
                c.p2.y,
                NodeType::OffCurve,
            ));
            *next_node_id += 2;
        }
    }
    nodes
}

pub(crate) fn merge_two_closed_rings_at_cut(
    ring_a: &[Node],
    idx_a: usize,
    ring_b: &[Node],
    idx_b: usize,
    knife_start: FontPoint,
    knife_end: FontPoint,
    next_node_id: &mut u64,
) -> Vec<Node> {
    let (dx, dy) = (
        knife_end.x.0 - knife_start.x.0,
        knife_end.y.0 - knife_start.y.0,
    );
    let pt_a = ring_a[idx_a].point;
    let next_a = (1..ring_a.len())
        .map(|k| ring_a[(idx_a + k) % ring_a.len()].point)
        .find(|p| p.distance_to(pt_a) > crate::tokens::KNIFE_PARAM_DEDUP_EPS)
        .unwrap_or(pt_a);
    let cross_a = dx * (next_a.y.0 - pt_a.y.0) - dy * (next_a.x.0 - pt_a.x.0);

    let mut oriented_b = ring_b.to_vec();
    let pt_b = oriented_b[idx_b].point;
    let next_b = (1..oriented_b.len())
        .map(|k| oriented_b[(idx_b + k) % oriented_b.len()].point)
        .find(|p| p.distance_to(pt_b) > crate::tokens::KNIFE_PARAM_DEDUP_EPS)
        .unwrap_or(pt_b);
    let cross_b = dx * (next_b.y.0 - pt_b.y.0) - dy * (next_b.x.0 - pt_b.x.0);
    if cross_a * cross_b > 0.0 {
        crate::core::reverse_contour_nodes(&mut oriented_b, true);
    }
    let ib = oriented_b
        .iter()
        .position(|n| n.id == ring_b[idx_b].id)
        .unwrap_or(0);

    let mut merged = Vec::with_capacity(ring_a.len() + oriented_b.len() + 2);
    for (ring, start_idx) in [(ring_a, idx_a), (oriented_b.as_slice(), ib)] {
        let len = ring.len();
        for k in 0..len {
            let mut node = ring[(start_idx + k) % len].clone();
            if k == 0 {
                node.node_type = NodeType::Corner;
            }
            merged.push(node);
        }
        let mut dup = ring[start_idx].clone();
        dup.id = NodeId(*next_node_id);
        *next_node_id += 1;
        dup.node_type = NodeType::Corner;
        merged.push(dup);
    }
    merged
}

pub(crate) fn split_closed_ring_at_indices(
    ring: &[Node],
    idx_a: usize,
    idx_b: usize,
    next_node_id: &mut u64,
) -> (Vec<Node>, Vec<Node>) {
    let len = ring.len();
    let mut half1 = Vec::new();
    let mut curr = idx_a;
    loop {
        half1.push(ring[curr].clone());
        if curr == idx_b {
            break;
        }
        curr = (curr + 1) % len;
    }
    if let Some(first) = half1.first_mut() {
        first.node_type = NodeType::Corner;
    }
    if let Some(last) = half1.last_mut() {
        last.node_type = NodeType::Corner;
    }

    let mut half2 = Vec::new();
    let mut curr2 = idx_b;
    loop {
        half2.push(ring[curr2].clone());
        if curr2 == idx_a {
            break;
        }
        curr2 = (curr2 + 1) % len;
    }
    if let Some(first) = half2.first_mut() {
        first.id = NodeId(*next_node_id);
        *next_node_id += 1;
        first.node_type = NodeType::Corner;
    }
    if let Some(last) = half2.last_mut() {
        last.id = NodeId(*next_node_id);
        *next_node_id += 1;
        last.node_type = NodeType::Corner;
    }
    (half1, half2)
}
