//! Segment representation, pairwise Newton-Raphson intersection solver, and cycle reconstruction
//! for `BooleanPathEngine` (`SPEC-005 §2.7`).

use crate::core::bezier_ops::{Node, NodeId, NodeType};
use crate::core::coordinates::FontPoint;
use crate::core::knife_ops::intersect_finite_segments;
use crate::core::math_consts::{
    BOOL_COLLINEAR_CROSS_EPS, BOOL_CUBIC_SUBDIV_STEPS, BOOL_NEWTON_ITERS,
    BOOL_TANGENT_COS_SMOOTH_MIN, BOOL_VERTEX_SNAP_EPS_UPM, COLLINEAR_MIN_LEN_EPSILON,
    CURVE_SOLVE_ACCURACY, ONE_THIRD_F64, SOLVER_DET_EPSILON, TWO_THIRDS_F64,
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

pub(crate) fn extract_bool_segs(nodes: &[Node], closed: bool) -> Vec<BoolSeg> {
    let bez = crate::core::spline_support::contour_to_bezpath_with_splines(nodes, closed, 0.0, &[]);
    let mut segs = Vec::new();
    let mut start_pt = None;
    let mut curr_pt = None;
    for el in bez.elements() {
        match *el {
            kurbo::PathEl::MoveTo(p) => {
                start_pt = Some(p);
                curr_pt = Some(p);
            }
            kurbo::PathEl::LineTo(p) => {
                if let Some(c) = curr_pt {
                    segs.push(BoolSeg::Line(Line::new(c, p)));
                }
                curr_pt = Some(p);
            }
            kurbo::PathEl::QuadTo(q1, p2) => {
                if let Some(p0) = curr_pt {
                    let h1 = Point::new(
                        ONE_THIRD_F64 * p0.x + TWO_THIRDS_F64 * q1.x,
                        ONE_THIRD_F64 * p0.y + TWO_THIRDS_F64 * q1.y,
                    );
                    let h2 = Point::new(
                        TWO_THIRDS_F64 * q1.x + ONE_THIRD_F64 * p2.x,
                        TWO_THIRDS_F64 * q1.y + ONE_THIRD_F64 * p2.y,
                    );
                    segs.push(BoolSeg::Cubic(CubicBez::new(p0, h1, h2, p2)));
                }
                curr_pt = Some(p2);
            }
            kurbo::PathEl::CurveTo(p1, p2, p3) => {
                if let Some(p0) = curr_pt {
                    segs.push(BoolSeg::Cubic(CubicBez::new(p0, p1, p2, p3)));
                }
                curr_pt = Some(p3);
            }
            kurbo::PathEl::ClosePath => {
                if let (Some(c), Some(s)) = (curr_pt, start_pt) {
                    if (c.x - s.x).hypot(c.y - s.y) > COLLINEAR_MIN_LEN_EPSILON {
                        segs.push(BoolSeg::Line(Line::new(c, s)));
                    }
                    curr_pt = Some(s);
                }
            }
        }
    }
    segs
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
        if let Some(&last_seg) = cycle.last() {
            if let Some(m) = merge_collinear_lines(last_seg, cycle[0]) {
                cycle[0] = m;
                cycle.pop();
            }
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
