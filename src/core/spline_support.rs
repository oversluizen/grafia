//! Shared anchor-span extraction, directional tangent constraints, linear solver, and `BezPath`
//! projection for `NodeType::HobbyStar` and `NodeType::EulerSpiro` (`SPEC-009 §2.1`).

use crate::core::bezier_ops::{Node, NodeType};
use crate::core::canvas_entities::HobbyDirConstraint;
use crate::core::hobby_spline::{
    evaluate_euler_spiro_chain_to_cubics, evaluate_hobby_chain_to_cubics,
};
use crate::core::math_consts::{
    DEG_TO_RAD_F64, HOBBY_MIN_CHORD_EPS, HOBBY_SOLVER_PIVOT_EPS, METRIC_LINE_SNAP_TOL_UPM, PI_F64,
    TAU_F64,
};
use kurbo::{BezPath, Point};

#[must_use]
pub(crate) fn wrap_angle_pi(mut angle: f64) -> f64 {
    debug_assert!(angle.is_finite());
    while angle > PI_F64 {
        angle -= TAU_F64;
    }
    while angle < -PI_F64 {
        angle += TAU_F64;
    }
    angle
}

pub(crate) fn solve_linear_system(a: &mut [Vec<f64>], b: &mut [f64]) -> Option<Vec<f64>> {
    debug_assert_eq!(a.len(), b.len());
    let n = b.len();
    if n == 0 {
        return Some(Vec::new());
    }
    for col in 0..n {
        let mut pivot_row = col;
        let mut max_abs = a[col][col].abs();
        for (r, row_vals) in a.iter().enumerate().skip(col + 1) {
            let v = row_vals[col].abs();
            if v > max_abs {
                max_abs = v;
                pivot_row = r;
            }
        }
        if max_abs <= HOBBY_SOLVER_PIVOT_EPS {
            return None;
        }
        if pivot_row != col {
            a.swap(col, pivot_row);
            b.swap(col, pivot_row);
        }
        let diag = a[col][col];
        let pivot_tail: Vec<f64> = a[col][(col + 1)..n].to_vec();
        for r in (col + 1)..n {
            let factor = a[r][col] / diag;
            if factor.abs() <= HOBBY_SOLVER_PIVOT_EPS {
                continue;
            }
            a[r][col] = 0.0;
            for (rc, &pv) in a[r][(col + 1)..n].iter_mut().zip(pivot_tail.iter()) {
                *rc -= factor * pv;
            }
            b[r] -= factor * b[col];
        }
    }
    let mut x = vec![0.0; n];
    for i in (0..n).rev() {
        let mut sum = b[i];
        for (j, &xj) in x.iter().enumerate().skip(i + 1) {
            sum -= a[i][j] * xj;
        }
        let diag = a[i][i];
        if diag.abs() <= HOBBY_SOLVER_PIVOT_EPS {
            return None;
        }
        x[i] = sum / diag;
    }
    Some(x)
}

pub(crate) type AnchorSpan = (usize, usize, Vec<usize>);

pub(crate) fn extract_anchor_spans(nodes: &[Node], closed: bool) -> (Vec<usize>, Vec<AnchorSpan>) {
    let anchors: Vec<usize> = nodes
        .iter()
        .enumerate()
        .filter(|(_, n)| n.node_type != NodeType::OffCurve)
        .map(|(i, _)| i)
        .collect();
    let m = anchors.len();
    if m < 2 {
        return (anchors, Vec::new());
    }
    let num_spans = if closed { m } else { m - 1 };
    let total = nodes.len();
    let mut spans = Vec::with_capacity(num_spans);
    for s in 0..num_spans {
        let e = (s + 1) % m;
        let start_idx = anchors[s];
        let end_idx = anchors[e];
        let mut off = Vec::new();
        let mut curr = (start_idx + 1) % total;
        while curr != end_idx {
            if nodes[curr].node_type == NodeType::OffCurve {
                off.push(curr);
            }
            curr = (curr + 1) % total;
        }
        spans.push((s, e, off));
    }
    (anchors, spans)
}

#[must_use]
pub(crate) fn constrained_anchor_world_angle(
    nodes: &[Node],
    anchors: &[usize],
    spans: &[AnchorSpan],
    a_pos: usize,
    closed: bool,
    italic_angle_deg: f64,
    metrics_y: &[f64],
) -> Option<f64> {
    let a_idx = anchors[a_pos];
    let p_curr = nodes[a_idx].point;
    let constraint = match nodes[a_idx].node_type {
        NodeType::HobbyStar { dir_constraint, .. } | NodeType::EulerSpiro { dir_constraint } => {
            dir_constraint
        }
        NodeType::Smooth => {
            for (s, e, off) in spans {
                if *e == a_pos && !off.is_empty() {
                    let h = nodes[*off.last()?].point;
                    let (dx, dy) = (p_curr.x.0 - h.x.0, p_curr.y.0 - h.y.0);
                    if dx.hypot(dy) > HOBBY_MIN_CHORD_EPS {
                        return Some(dy.atan2(dx));
                    }
                }
                if *s == a_pos && !off.is_empty() {
                    let h = nodes[off[0]].point;
                    let (dx, dy) = (h.x.0 - p_curr.x.0, h.y.0 - p_curr.y.0);
                    if dx.hypot(dy) > HOBBY_MIN_CHORD_EPS {
                        return Some(dy.atan2(dx));
                    }
                }
            }
            HobbyDirConstraint::Free
        }
        NodeType::Corner | NodeType::OffCurve => HobbyDirConstraint::Free,
    };

    let axis = match constraint {
        HobbyDirConstraint::Free => return None,
        HobbyDirConstraint::Horizontal => (1.0, 0.0),
        HobbyDirConstraint::Vertical => (0.0, 1.0),
        HobbyDirConstraint::SlantedItalic => {
            let rad = italic_angle_deg * DEG_TO_RAD_F64;
            (rad.sin(), rad.cos())
        }
        HobbyDirConstraint::FollowMetricLine => {
            let _snapped = metrics_y
                .iter()
                .any(|&my| (p_curr.y.0 - my).abs() <= METRIC_LINE_SNAP_TOL_UPM);
            (1.0, 0.0)
        }
    };

    let m = anchors.len();
    let prev_pt = if a_pos > 0 {
        Some(nodes[anchors[a_pos - 1]].point)
    } else if closed && m >= 2 {
        Some(nodes[anchors[m - 1]].point)
    } else {
        None
    };
    let next_pt = if a_pos + 1 < m {
        Some(nodes[anchors[a_pos + 1]].point)
    } else if closed && m >= 2 {
        Some(nodes[anchors[0]].point)
    } else {
        None
    };

    let flow = match (prev_pt, next_pt) {
        (Some(prv), Some(nxt)) => (nxt.x.0 - prv.x.0, nxt.y.0 - prv.y.0),
        (None, Some(nxt)) => (nxt.x.0 - p_curr.x.0, nxt.y.0 - p_curr.y.0),
        (Some(prv), None) => (p_curr.x.0 - prv.x.0, p_curr.y.0 - prv.y.0),
        (None, None) => (1.0, 0.0),
    };
    let mut dot = flow.0 * axis.0 + flow.1 * axis.1;
    if dot.abs() <= HOBBY_MIN_CHORD_EPS {
        if let Some(nxt) = next_pt {
            dot = (nxt.x.0 - p_curr.x.0) * axis.0 + (nxt.y.0 - p_curr.y.0) * axis.1;
        }
    }
    let dir = if dot < 0.0 { (-axis.0, -axis.1) } else { axis };
    Some(dir.1.atan2(dir.0))
}

fn contour_to_bezpath_non_spline(nodes: &[Node], closed: bool) -> BezPath {
    let mut path = BezPath::new();
    let pt_of = |n: &Node| Point::new(n.point.x.0, n.point.y.0);
    path.move_to(pt_of(&nodes[0]));
    let mut i = 1;
    while i < nodes.len() {
        if nodes[i].node_type == NodeType::OffCurve {
            if i + 2 < nodes.len() && nodes[i + 1].node_type == NodeType::OffCurve {
                path.curve_to(pt_of(&nodes[i]), pt_of(&nodes[i + 1]), pt_of(&nodes[i + 2]));
                i += 3;
            } else if closed
                && i + 1 == nodes.len() - 1
                && nodes[i + 1].node_type == NodeType::OffCurve
            {
                path.curve_to(pt_of(&nodes[i]), pt_of(&nodes[i + 1]), pt_of(&nodes[0]));
                i += 2;
            } else if i + 1 < nodes.len() {
                path.quad_to(pt_of(&nodes[i]), pt_of(&nodes[i + 1]));
                i += 2;
            } else if closed && i == nodes.len() - 1 {
                path.quad_to(pt_of(&nodes[i]), pt_of(&nodes[0]));
                i += 1;
            } else {
                path.line_to(pt_of(&nodes[i]));
                i += 1;
            }
        } else {
            path.line_to(pt_of(&nodes[i]));
            i += 1;
        }
    }
    if closed {
        path.close_path();
    }
    path
}

/// Converts a mixed `Node` contour into an evaluated `kurbo::BezPath`.
#[must_use]
pub fn contour_to_bezpath_with_splines(
    nodes: &[Node],
    closed: bool,
    italic_angle_deg: f64,
    metrics_y: &[f64],
) -> BezPath {
    if nodes.is_empty() {
        return BezPath::new();
    }
    let has_splines = nodes.iter().any(|n| {
        matches!(
            n.node_type,
            NodeType::HobbyStar { .. } | NodeType::EulerSpiro { .. }
        )
    });
    if !has_splines {
        return contour_to_bezpath_non_spline(nodes, closed);
    }

    let mut path = BezPath::new();
    let pt_of = |n: &Node| Point::new(n.point.x.0, n.point.y.0);
    let (anchors, spans) = extract_anchor_spans(nodes, closed);
    if anchors.is_empty() {
        path.move_to(pt_of(&nodes[0]));
        return path;
    }
    let hobby_segs = evaluate_hobby_chain_to_cubics(nodes, closed, italic_angle_deg, metrics_y);
    let euler_segs =
        evaluate_euler_spiro_chain_to_cubics(nodes, closed, italic_angle_deg, metrics_y);

    path.move_to(pt_of(&nodes[anchors[0]]));
    let num_spans = spans.len();
    for (span_i, (s_pos, e_pos, off)) in spans.iter().enumerate() {
        let s_idx = anchors[*s_pos];
        let e_idx = anchors[*e_pos];
        let p_end = pt_of(&nodes[e_idx]);
        if off.len() >= 2 {
            path.curve_to(pt_of(&nodes[off[0]]), pt_of(&nodes[off[1]]), p_end);
        } else if off.len() == 1 {
            path.quad_to(pt_of(&nodes[off[0]]), p_end);
        } else if let Some(seg) = hobby_segs
            .iter()
            .chain(euler_segs.iter())
            .find(|seg| seg.start_idx == s_idx && seg.end_idx == e_idx)
        {
            path.curve_to(seg.cubic.p1, seg.cubic.p2, seg.cubic.p3);
        } else if !(closed && span_i + 1 == num_spans) {
            path.line_to(p_end);
        }
    }
    if closed {
        path.close_path();
    }
    path
}
