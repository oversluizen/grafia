//! `SPEC-009 §2.1.3` Explicit curve degree & spline conversions (`Hobby/Euler <-> Cubic`,
//! `Cubic <-> Quadratic`).

use crate::core::bezier_ops::{Node, NodeId, NodeType};
use crate::core::canvas_entities::HobbyDirConstraint;
use crate::core::hobby_spline::{
    evaluate_euler_spiro_chain_to_cubics, evaluate_hobby_chain_to_cubics, extract_anchor_spans,
};
use crate::core::math_consts::{
    HOBBY_TENSION_DEFAULT, ONE_THIRD_F64, QUAD_APPROX_TOLERANCE_UPM, QUAD_MAX_SUBDIV_DEPTH,
    QUAD_QUARTER, QUAD_SAMPLE_TS, QUAD_THREE_QUARTERS, TWO_THIRDS_F64,
};
use kurbo::{CubicBez, ParamCurve, Point, QuadBez};

fn alloc_id(next_id: &mut u64) -> NodeId {
    let id = NodeId(*next_id);
    *next_id += 1;
    id
}

fn approx_cubic_with_quads(cubic: CubicBez, depth: u8, out: &mut Vec<(Point, Point)>) {
    let q1 = Point::new(
        QUAD_THREE_QUARTERS * (cubic.p1.x + cubic.p2.x) - QUAD_QUARTER * (cubic.p0.x + cubic.p3.x),
        QUAD_THREE_QUARTERS * (cubic.p1.y + cubic.p2.y) - QUAD_QUARTER * (cubic.p0.y + cubic.p3.y),
    );
    let quad = QuadBez::new(cubic.p0, q1, cubic.p3);
    let mut max_err = 0.0_f64;
    for t in QUAD_SAMPLE_TS {
        let cp = cubic.eval(t);
        let qp = quad.eval(t);
        let err = (cp.x - qp.x).hypot(cp.y - qp.y);
        if err > max_err {
            max_err = err;
        }
    }
    if max_err <= QUAD_APPROX_TOLERANCE_UPM || depth >= QUAD_MAX_SUBDIV_DEPTH {
        out.push((q1, cubic.p3));
    } else {
        let left = cubic.subsegment(0.0..0.5);
        let right = cubic.subsegment(0.5..1.0);
        approx_cubic_with_quads(left, depth + 1, out);
        approx_cubic_with_quads(right, depth + 1, out);
    }
}

pub fn convert_hobby_or_euler_in_contour(
    nodes: &mut Vec<Node>,
    closed: bool,
    selected: &[NodeId],
    targets: (bool, bool),
    italic_angle_deg: f64,
    metrics_y: &[f64],
    next_id: &mut u64,
) -> bool {
    let (convert_hobby, convert_euler) = targets;
    let (anchors, spans) = extract_anchor_spans(nodes, closed);
    if anchors.is_empty() {
        return false;
    }
    let is_sel = |id: NodeId| selected.is_empty() || selected.contains(&id);
    let hobby_segs = if convert_hobby {
        evaluate_hobby_chain_to_cubics(nodes, closed, italic_angle_deg, metrics_y)
    } else {
        Vec::new()
    };
    let euler_segs = if convert_euler {
        evaluate_euler_spiro_chain_to_cubics(nodes, closed, italic_angle_deg, metrics_y)
    } else {
        Vec::new()
    };

    let mut changed = false;
    let mut rebuilt = Vec::with_capacity(nodes.len() + spans.len() * 2);
    let m = anchors.len();
    for (a_pos, &a_idx) in anchors.iter().enumerate() {
        let mut anchor_node = nodes[a_idx].clone();
        let sel_here = is_sel(anchor_node.id);
        let is_target_type = (convert_hobby
            && matches!(anchor_node.node_type, NodeType::HobbyStar { .. }))
            || (convert_euler && matches!(anchor_node.node_type, NodeType::EulerSpiro { .. }));
        if sel_here && is_target_type {
            let open_end = !closed && (a_pos == 0 || a_pos + 1 == m);
            anchor_node.node_type = if open_end {
                NodeType::Corner
            } else {
                NodeType::Smooth
            };
            changed = true;
        }
        rebuilt.push(anchor_node);

        if let Some((_, e_pos, off)) = spans.get(a_pos) {
            let e_idx = anchors[*e_pos];
            let end_sel = is_sel(nodes[e_idx].id);
            if off.is_empty() && (sel_here || end_sel) {
                let matched_hobby = hobby_segs
                    .iter()
                    .find(|s| s.start_idx == a_idx && s.end_idx == e_idx);
                let matched_euler = euler_segs
                    .iter()
                    .find(|s| s.start_idx == a_idx && s.end_idx == e_idx);
                if let Some(seg) = matched_hobby.or(matched_euler) {
                    rebuilt.push(Node::new(
                        alloc_id(next_id),
                        seg.cubic.p1.x,
                        seg.cubic.p1.y,
                        NodeType::OffCurve,
                    ));
                    rebuilt.push(Node::new(
                        alloc_id(next_id),
                        seg.cubic.p2.x,
                        seg.cubic.p2.y,
                        NodeType::OffCurve,
                    ));
                    changed = true;
                }
            } else {
                for &oi in off {
                    rebuilt.push(nodes[oi].clone());
                }
            }
        }
    }
    if changed {
        *nodes = rebuilt;
    }
    changed
}

pub fn convert_cubic_to_hobby_in_contour(
    nodes: &mut Vec<Node>,
    closed: bool,
    selected: &[NodeId],
) -> bool {
    let (anchors, spans) = extract_anchor_spans(nodes, closed);
    if anchors.is_empty() {
        return false;
    }
    let is_sel = |id: NodeId| selected.is_empty() || selected.contains(&id);
    let mut converted_anchor = vec![false; anchors.len()];
    let mut changed = false;

    for (a_pos, &a_idx) in anchors.iter().enumerate() {
        if is_sel(nodes[a_idx].id)
            && matches!(nodes[a_idx].node_type, NodeType::Corner | NodeType::Smooth)
        {
            nodes[a_idx].node_type = NodeType::HobbyStar {
                tension_in: HOBBY_TENSION_DEFAULT,
                tension_out: HOBBY_TENSION_DEFAULT,
                dir_constraint: HobbyDirConstraint::Free,
            };
            converted_anchor[a_pos] = true;
            changed = true;
        }
    }
    if !changed {
        return false;
    }

    let mut rebuilt = Vec::with_capacity(nodes.len());
    for (a_pos, &a_idx) in anchors.iter().enumerate() {
        rebuilt.push(nodes[a_idx].clone());
        if let Some((_, e_pos, off)) = spans.get(a_pos) {
            let e_idx = anchors[*e_pos];
            let s_hobby = matches!(nodes[a_idx].node_type, NodeType::HobbyStar { .. });
            let e_hobby = matches!(nodes[e_idx].node_type, NodeType::HobbyStar { .. });
            let strip = !off.is_empty()
                && s_hobby
                && e_hobby
                && (converted_anchor[a_pos] || converted_anchor[*e_pos]);
            if !strip {
                for &oi in off {
                    rebuilt.push(nodes[oi].clone());
                }
            }
        }
    }
    *nodes = rebuilt;
    true
}

pub fn convert_contour_to_quadratic(
    nodes: &mut Vec<Node>,
    closed: bool,
    italic_angle_deg: f64,
    metrics_y: &[f64],
    next_id: &mut u64,
) -> bool {
    let mut changed = convert_hobby_or_euler_in_contour(
        nodes,
        closed,
        &[],
        (true, true),
        italic_angle_deg,
        metrics_y,
        next_id,
    );
    let (anchors, spans) = extract_anchor_spans(nodes, closed);
    if anchors.is_empty() {
        return changed;
    }
    let pt_of = |idx: usize, ns: &[Node]| Point::new(ns[idx].point.x.0, ns[idx].point.y.0);
    let mut rebuilt = Vec::new();
    for (a_pos, &a_idx) in anchors.iter().enumerate() {
        rebuilt.push(nodes[a_idx].clone());
        if let Some((_, e_pos, off)) = spans.get(a_pos) {
            let e_idx = anchors[*e_pos];
            if off.len() >= 2 {
                let cubic = CubicBez::new(
                    pt_of(a_idx, nodes),
                    pt_of(off[0], nodes),
                    pt_of(off[1], nodes),
                    pt_of(e_idx, nodes),
                );
                let mut quads = Vec::new();
                approx_cubic_with_quads(cubic, 0, &mut quads);
                let q_len = quads.len();
                for (qi, (ctrl, end_pt)) in quads.into_iter().enumerate() {
                    rebuilt.push(Node::new(
                        alloc_id(next_id),
                        ctrl.x,
                        ctrl.y,
                        NodeType::OffCurve,
                    ));
                    if qi + 1 < q_len {
                        rebuilt.push(Node::new(
                            alloc_id(next_id),
                            end_pt.x,
                            end_pt.y,
                            NodeType::Smooth,
                        ));
                    }
                }
                changed = true;
            } else {
                for &oi in off {
                    rebuilt.push(nodes[oi].clone());
                }
            }
        }
    }
    if changed {
        *nodes = rebuilt;
    }
    changed
}

pub fn convert_contour_to_cubic(
    nodes: &mut Vec<Node>,
    closed: bool,
    italic_angle_deg: f64,
    metrics_y: &[f64],
    next_id: &mut u64,
) -> bool {
    let mut changed = convert_hobby_or_euler_in_contour(
        nodes,
        closed,
        &[],
        (true, true),
        italic_angle_deg,
        metrics_y,
        next_id,
    );
    let (anchors, spans) = extract_anchor_spans(nodes, closed);
    if anchors.is_empty() {
        return changed;
    }
    let mut rebuilt = Vec::new();
    for (a_pos, &a_idx) in anchors.iter().enumerate() {
        rebuilt.push(nodes[a_idx].clone());
        if let Some((_, e_pos, off)) = spans.get(a_pos) {
            let e_idx = anchors[*e_pos];
            if off.len() == 1 {
                let p0 = nodes[a_idx].point;
                let q1 = nodes[off[0]].point;
                let p2 = nodes[e_idx].point;
                let h1x = ONE_THIRD_F64 * p0.x.0 + TWO_THIRDS_F64 * q1.x.0;
                let h1y = ONE_THIRD_F64 * p0.y.0 + TWO_THIRDS_F64 * q1.y.0;
                let h2x = TWO_THIRDS_F64 * q1.x.0 + ONE_THIRD_F64 * p2.x.0;
                let h2y = TWO_THIRDS_F64 * q1.y.0 + ONE_THIRD_F64 * p2.y.0;
                rebuilt.push(Node::new(nodes[off[0]].id, h1x, h1y, NodeType::OffCurve));
                rebuilt.push(Node::new(alloc_id(next_id), h2x, h2y, NodeType::OffCurve));
                changed = true;
            } else {
                for &oi in off {
                    rebuilt.push(nodes[oi].clone());
                }
            }
        }
    }
    if changed {
        *nodes = rebuilt;
    }
    changed
}
