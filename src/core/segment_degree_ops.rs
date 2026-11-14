//! `SPEC-009 §2.1.3` Off-curve handle insertion/removal and compatible foreground-background
//! linear interpolation (`interpolate_contours_linear`).

use crate::core::bezier_ops::{Node, NodeId, NodeType};
use crate::core::coordinates::FontPoint;
use crate::core::glyph_model::GlyphContour;
use crate::core::hobby_spline::extract_anchor_spans;
use crate::core::math_consts::{
    INTERPOLATE_T_MAX, INTERPOLATE_T_MIN, ONE_THIRD_F64, TWO_THIRDS_F64,
};

fn alloc_id(next_id: &mut u64) -> NodeId {
    let id = NodeId(*next_id);
    *next_id += 1;
    id
}

pub fn add_offcurve_in_contour(
    nodes: &mut Vec<Node>,
    closed: bool,
    selected: &[NodeId],
    next_id: &mut u64,
) -> bool {
    let (anchors, spans) = extract_anchor_spans(nodes, closed);
    if anchors.is_empty() {
        return false;
    }
    let is_sel = |id: NodeId| selected.is_empty() || selected.contains(&id);
    let mut changed = false;
    let mut rebuilt = Vec::new();

    for (a_pos, &a_idx) in anchors.iter().enumerate() {
        rebuilt.push(nodes[a_idx].clone());
        if let Some((_, e_pos, off)) = spans.get(a_pos) {
            let e_idx = anchors[*e_pos];
            let both_sel = is_sel(nodes[a_idx].id) && is_sel(nodes[e_idx].id);
            let p0 = nodes[a_idx].point;
            let p1 = nodes[e_idx].point;
            if both_sel
                && off.is_empty()
                && !matches!(
                    nodes[a_idx].node_type,
                    NodeType::HobbyStar { .. } | NodeType::EulerSpiro { .. }
                )
                && !matches!(
                    nodes[e_idx].node_type,
                    NodeType::HobbyStar { .. } | NodeType::EulerSpiro { .. }
                )
            {
                let h1x = TWO_THIRDS_F64 * p0.x.0 + ONE_THIRD_F64 * p1.x.0;
                let h1y = TWO_THIRDS_F64 * p0.y.0 + ONE_THIRD_F64 * p1.y.0;
                let h2x = ONE_THIRD_F64 * p0.x.0 + TWO_THIRDS_F64 * p1.x.0;
                let h2y = ONE_THIRD_F64 * p0.y.0 + TWO_THIRDS_F64 * p1.y.0;
                rebuilt.push(Node::new(alloc_id(next_id), h1x, h1y, NodeType::OffCurve));
                rebuilt.push(Node::new(alloc_id(next_id), h2x, h2y, NodeType::OffCurve));
                changed = true;
            } else if both_sel && off.len() == 1 {
                let q1 = nodes[off[0]].point;
                let h1x = ONE_THIRD_F64 * p0.x.0 + TWO_THIRDS_F64 * q1.x.0;
                let h1y = ONE_THIRD_F64 * p0.y.0 + TWO_THIRDS_F64 * q1.y.0;
                let h2x = TWO_THIRDS_F64 * q1.x.0 + ONE_THIRD_F64 * p1.x.0;
                let h2y = TWO_THIRDS_F64 * q1.y.0 + ONE_THIRD_F64 * p1.y.0;
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

pub fn remove_offcurve_in_contour(
    nodes: &mut Vec<Node>,
    closed: bool,
    selected: &[NodeId],
) -> bool {
    let (anchors, spans) = extract_anchor_spans(nodes, closed);
    if anchors.is_empty() {
        return false;
    }
    let is_sel = |id: NodeId| selected.is_empty() || selected.contains(&id);
    let mut strip_span = vec![false; spans.len()];
    let mut reset_corner = vec![false; anchors.len()];
    let mut changed = false;

    for (sp_i, (s_pos, e_pos, off)) in spans.iter().enumerate() {
        if off.is_empty() {
            continue;
        }
        let s_idx = anchors[*s_pos];
        let e_idx = anchors[*e_pos];
        let end_sel = is_sel(nodes[s_idx].id) && is_sel(nodes[e_idx].id);
        let handle_sel = off.iter().any(|&oi| selected.contains(&nodes[oi].id));
        if end_sel || handle_sel {
            strip_span[sp_i] = true;
            reset_corner[*s_pos] = true;
            reset_corner[*e_pos] = true;
            changed = true;
        }
    }
    if !changed {
        return false;
    }

    let mut rebuilt = Vec::new();
    for (a_pos, &a_idx) in anchors.iter().enumerate() {
        let mut nd = nodes[a_idx].clone();
        if reset_corner[a_pos] && nd.node_type == NodeType::Smooth {
            nd.node_type = NodeType::Corner;
        }
        rebuilt.push(nd);
        if let Some((_, _, off)) = spans.get(a_pos) {
            if !strip_span[a_pos] {
                for &oi in off {
                    rebuilt.push(nodes[oi].clone());
                }
            }
        }
    }
    *nodes = rebuilt;
    true
}

pub fn interpolate_contours_linear(
    fg: &mut [Vec<Node>],
    fg_closed: &[bool],
    bg: &[GlyphContour],
    t: f64,
) -> bool {
    if fg.is_empty() || fg.len() != bg.len() {
        return false;
    }
    for (i, fg_c) in fg.iter().enumerate() {
        let closed_i = fg_closed.get(i).copied().unwrap_or(true);
        if closed_i != bg[i].closed || fg_c.len() != bg[i].nodes.len() {
            return false;
        }
    }
    let w = t.clamp(INTERPOLATE_T_MIN, INTERPOLATE_T_MAX);
    let one_minus_w = 1.0 - w;
    for (fg_c, bg_c) in fg.iter_mut().zip(bg.iter()) {
        for (fn_node, bn_node) in fg_c.iter_mut().zip(bg_c.nodes.iter()) {
            let nx = one_minus_w * fn_node.point.x.0 + w * bn_node.point.x.0;
            let ny = one_minus_w * fn_node.point.y.0 + w * bn_node.point.y.0;
            fn_node.point = FontPoint::new(nx, ny);
        }
    }
    true
}
