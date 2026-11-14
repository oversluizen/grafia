//! `SPEC-003 §2.6.6–§2.6.7` `align_selected_nodes` (`_GSAlignedPointForBounds` + `_alignOtherNodes:`),
//! `connect_selected_open_endpoints_in_contours`, and `close_open_contours_in_layer`.

use crate::core::bezier_ops::{align_collinear_opposite, Node, NodeId, NodeType};
use crate::core::coordinates::FontPoint;
use crate::core::handle_constraints::enforce_smooth_collinearity_on_handle_drag;
use crate::core::math_consts::CONNECT_MERGE_DIST_UPM;
use crate::core::path_ops::round_grid_pt;
use crate::core::transform_ops::{compute_selection_bounds, TransformGridAnchor};

fn try_align_adjacent_on_off_pair(
    contours: &mut [Vec<Node>],
    closed_flags: &[bool],
    selected_nodes: &[NodeId],
) -> bool {
    for (c_idx, contour) in contours.iter_mut().enumerate() {
        let closed = closed_flags.get(c_idx).copied().unwrap_or(true);
        let len = contour.len();
        let sel_pos: Vec<usize> = contour
            .iter()
            .enumerate()
            .filter(|(_, n)| selected_nodes.contains(&n.id))
            .map(|(i, _)| i)
            .collect();
        if sel_pos.len() != 2 {
            continue;
        }
        let (i0, i1) = (sel_pos[0], sel_pos[1]);
        let adj = i0.abs_diff(i1) == 1
            || (closed && len >= 2 && ((i0 == 0 && i1 + 1 == len) || (i1 == 0 && i0 + 1 == len)));
        if !adj {
            continue;
        }
        let pair = match (contour[i0].node_type, contour[i1].node_type) {
            (NodeType::OffCurve, NodeType::Corner | NodeType::Smooth) => Some((i1, i0)),
            (NodeType::Corner | NodeType::Smooth, NodeType::OffCurve) => Some((i0, i1)),
            _ => None,
        };
        if let Some((on_idx, off_idx)) = pair {
            let (a_pt, h_pt) = (contour[on_idx].point, contour[off_idx].point);
            if (h_pt.x.0 - a_pt.x.0).abs() <= (h_pt.y.0 - a_pt.y.0).abs() {
                contour[off_idx].point.x = a_pt.x;
            } else {
                contour[off_idx].point.y = a_pt.y;
            }
            let off_id = contour[off_idx].id;
            enforce_smooth_collinearity_on_handle_drag(contour, closed, off_id, false);
            return true;
        }
    }
    false
}

fn neighbor_indices(idx: usize, len: usize, closed: bool) -> (Option<usize>, Option<usize>) {
    let prev = if idx > 0 {
        Some(idx - 1)
    } else if closed {
        Some(len - 1)
    } else {
        None
    };
    let next = if idx + 1 < len {
        Some(idx + 1)
    } else if closed {
        Some(0)
    } else {
        None
    };
    (prev, next)
}

fn align_contour_nodes_to_target(
    contour: &mut [Node],
    closed: bool,
    selected_nodes: &[NodeId],
    aligned_pt: FontPoint,
    align_y: bool,
) {
    let shadow = contour.to_vec();
    let len = contour.len();
    for node in contour.iter_mut() {
        if selected_nodes.contains(&node.id) {
            if align_y {
                node.point.y = aligned_pt.y;
            } else {
                node.point.x = aligned_pt.x;
            }
        }
    }
    for i in 0..len {
        if contour[i].node_type != NodeType::OffCurve || selected_nodes.contains(&contour[i].id) {
            continue;
        }
        let (prev, next) = neighbor_indices(i, len, closed);
        let anchor_idx = prev
            .filter(|&p| contour[p].node_type != NodeType::OffCurve)
            .or_else(|| next.filter(|&n| contour[n].node_type != NodeType::OffCurve));
        let Some(a_idx) = anchor_idx else {
            continue;
        };
        if selected_nodes.contains(&contour[a_idx].id) {
            if align_y {
                contour[i].point.y.0 += contour[a_idx].point.y.0 - shadow[a_idx].point.y.0;
            } else {
                contour[i].point.x.0 += contour[a_idx].point.x.0 - shadow[a_idx].point.x.0;
            }
        }
        if contour[a_idx].node_type == NodeType::Smooth {
            let (a_prev, a_next) = neighbor_indices(a_idx, len, closed);
            let opp = if prev == Some(a_idx) { a_prev } else { a_next };
            if let Some(b_idx) = opp.filter(|&b| selected_nodes.contains(&contour[b].id)) {
                contour[i].point = align_collinear_opposite(
                    contour[a_idx].point,
                    contour[i].point,
                    contour[b_idx].point,
                );
            }
        }
    }
}

pub fn align_selected_nodes(
    contours: &mut [Vec<Node>],
    closed_flags: &[bool],
    selected_nodes: &[NodeId],
    anchor: TransformGridAnchor,
    grid_step_upm: f64,
) -> bool {
    debug_assert!(grid_step_upm.is_finite());
    if selected_nodes.len() < 2 {
        return false;
    }
    let before = contours.to_vec();
    if selected_nodes.len() == 2
        && try_align_adjacent_on_off_pair(contours, closed_flags, selected_nodes)
    {
        return contours != before;
    }
    let Some(bounds) = compute_selection_bounds(contours, selected_nodes, 0.0, 0.0) else {
        return false;
    };
    let aligned_pt = round_grid_pt(bounds.aligned_point(anchor.index()), grid_step_upm);
    let align_y = bounds.width() > bounds.height();
    for (c_idx, contour) in contours.iter_mut().enumerate() {
        let closed = closed_flags.get(c_idx).copied().unwrap_or(true);
        align_contour_nodes_to_target(contour, closed, selected_nodes, aligned_pt, align_y);
    }
    contours != before
}

fn close_open_contour_endpoints(contour: &mut Vec<Node>) {
    let (Some(first_node), Some(last_node)) = (contour.first(), contour.last()) else {
        return;
    };
    let (p_first, p_last) = (first_node.point, last_node.point);
    let on_c = contour
        .iter()
        .filter(|n| n.node_type != NodeType::OffCurve)
        .count();
    if p_first.distance_to(p_last) < CONNECT_MERGE_DIST_UPM && on_c > 2 {
        contour.pop();
        if let Some(first) = contour.first_mut() {
            first.node_type = NodeType::Corner;
        }
    } else {
        if let Some(first) = contour.first_mut() {
            first.node_type = NodeType::Corner;
        }
        if let Some(last) = contour.last_mut() {
            last.node_type = NodeType::Corner;
        }
    }
}

pub fn connect_selected_open_endpoints_in_contours(
    contours: &mut Vec<Vec<Node>>,
    closed_flags: &mut Vec<bool>,
    selected_nodes: &[NodeId],
) -> bool {
    debug_assert_eq!(contours.len(), closed_flags.len());
    let mut endpoints = Vec::new();
    for (c_idx, contour) in contours.iter().enumerate() {
        if closed_flags.get(c_idx).copied().unwrap_or(true) || contour.len() < 2 {
            continue;
        }
        let (Some(first), Some(last)) = (contour.first(), contour.last()) else {
            continue;
        };
        if selected_nodes.contains(&first.id) {
            endpoints.push((c_idx, true, first.id));
        }
        if selected_nodes.contains(&last.id) {
            endpoints.push((c_idx, false, last.id));
        }
    }
    if endpoints.len() > 2 && endpoints.len().is_multiple_of(2) {
        let paired = endpoints
            .as_chunks::<2>()
            .0
            .iter()
            .all(|&[a, b]| a.0 == b.0 && a.1 != b.1);
        if paired {
            for &[a, _] in endpoints.as_chunks::<2>().0 {
                close_open_contour_endpoints(&mut contours[a.0]);
                closed_flags[a.0] = true;
            }
            return true;
        }
    }
    if endpoints.len() != 2 {
        return false;
    }
    let (ca, is_first_a, _) = endpoints[0];
    let (cb, is_first_b, _) = endpoints[1];
    if ca == cb {
        close_open_contour_endpoints(&mut contours[ca]);
        closed_flags[ca] = true;
        return true;
    }
    if is_first_a {
        contours[ca].reverse();
    }
    if !is_first_b {
        contours[cb].reverse();
    }
    let (Some(pa_node), Some(pb_node)) = (contours[ca].last(), contours[cb].first()) else {
        return false;
    };
    let (pa, pb) = (pa_node.point, pb_node.point);
    if let Some(last_a) = contours[ca].last_mut() {
        last_a.node_type = NodeType::Corner;
    }
    if let Some(first_b) = contours[cb].first_mut() {
        first_b.node_type = NodeType::Corner;
    }
    let mut b_nodes = contours[cb].clone();
    if pa.distance_to(pb) < CONNECT_MERGE_DIST_UPM && !b_nodes.is_empty() {
        b_nodes.remove(0);
    }
    contours[ca].extend(b_nodes);
    contours.remove(cb);
    closed_flags.remove(cb);
    true
}

pub fn close_open_contours_in_layer(
    contours: &mut [Vec<Node>],
    closed_flags: &mut [bool],
    selected_nodes: &[NodeId],
) -> bool {
    let check_sel = !selected_nodes.is_empty();
    let mut changed = false;
    for (c_idx, contour) in contours.iter_mut().enumerate() {
        if closed_flags.get(c_idx).copied().unwrap_or(true) || contour.len() < 2 {
            continue;
        }
        if !check_sel || contour.iter().any(|n| selected_nodes.contains(&n.id)) {
            closed_flags[c_idx] = true;
            changed = true;
        }
    }
    changed
}
