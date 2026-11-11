//! `SPEC-003 §2.6.6–§2.6.7` `align_selected_nodes` (`_GSAlignedPointForBounds` + `_alignOtherNodes:`),
//! `connect_selected_open_endpoints_in_contours`, and `close_open_contours_in_layer`.

use crate::core::bezier_ops::{Node, NodeId, NodeType};
use crate::core::handle_constraints::enforce_smooth_collinearity_on_handle_drag;
use crate::core::path_ops::round_grid_pt;
use crate::core::transform_ops::{compute_selection_bounds, TransformGridAnchor};
use crate::tokens::CONNECT_MERGE_DIST_UPM;

pub fn align_selected_nodes(
    contours: &mut [Vec<Node>],
    closed_flags: &[bool],
    selected_nodes: &[NodeId],
    anchor: TransformGridAnchor,
    grid_step_upm: f64,
) -> bool {
    if selected_nodes.len() < 2 {
        return false;
    }
    let before = contours.to_vec();

    // Branch A: 2 nodes (1 OnCurve + 1 adjacent OffCurve on the same contour)
    if selected_nodes.len() == 2 {
        for (c_idx, contour) in contours.iter_mut().enumerate() {
            let (closed, len) = (
                closed_flags.get(c_idx).copied().unwrap_or(true),
                contour.len(),
            );
            let sel_pos: Vec<usize> = contour
                .iter()
                .enumerate()
                .filter(|(_, n)| selected_nodes.contains(&n.id))
                .map(|(i, _)| i)
                .collect();
            if sel_pos.len() == 2 {
                let (i0, i1) = (sel_pos[0], sel_pos[1]);
                let adj = i0.abs_diff(i1) == 1
                    || (closed
                        && len >= 2
                        && ((i0 == 0 && i1 + 1 == len) || (i1 == 0 && i0 + 1 == len)));
                let (on_opt, off_opt) = match (contour[i0].node_type, contour[i1].node_type) {
                    (NodeType::OffCurve, NodeType::Corner | NodeType::Smooth) => {
                        (Some(i1), Some(i0))
                    }
                    (NodeType::Corner | NodeType::Smooth, NodeType::OffCurve) => {
                        (Some(i0), Some(i1))
                    }
                    _ => (None, None),
                };
                if adj {
                    if let (Some(on_idx), Some(off_idx)) = (on_opt, off_opt) {
                        let (a_pt, h_pt) = (contour[on_idx].point, contour[off_idx].point);
                        if (h_pt.x.0 - a_pt.x.0).abs() <= (h_pt.y.0 - a_pt.y.0).abs() {
                            contour[off_idx].point.x = a_pt.x;
                        } else {
                            contour[off_idx].point.y = a_pt.y;
                        }
                        let off_id = contour[off_idx].id;
                        enforce_smooth_collinearity_on_handle_drag(contour, closed, off_id, false);
                        return contours != before;
                    }
                }
            }
        }
    }

    // Branch B: Multi-Node Alignment to _GSAlignedPointForBounds(anchor, bounds)
    let Some(bounds) = compute_selection_bounds(contours, selected_nodes, 0.0, 0.0) else {
        return false;
    };
    let aligned_pt = round_grid_pt(bounds.aligned_point(anchor.index()), grid_step_upm);
    let align_y = bounds.width() > bounds.height();
    for (c_idx, contour) in contours.iter_mut().enumerate() {
        let (closed, shadow, len) = (
            closed_flags.get(c_idx).copied().unwrap_or(true),
            contour.clone(),
            contour.len(),
        );
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
            if contour[i].node_type != NodeType::OffCurve || selected_nodes.contains(&contour[i].id)
            {
                continue;
            }
            let prev = if i > 0 {
                Some(i - 1)
            } else if closed {
                Some(len - 1)
            } else {
                None
            };
            let next = if i + 1 < len {
                Some(i + 1)
            } else if closed {
                Some(0)
            } else {
                None
            };
            let anchor_idx = prev
                .filter(|&p| contour[p].node_type != NodeType::OffCurve)
                .or_else(|| next.filter(|&n| contour[n].node_type != NodeType::OffCurve));
            if let Some(a_idx) = anchor_idx {
                if selected_nodes.contains(&contour[a_idx].id) {
                    if align_y {
                        contour[i].point.y.0 += contour[a_idx].point.y.0 - shadow[a_idx].point.y.0;
                    } else {
                        contour[i].point.x.0 += contour[a_idx].point.x.0 - shadow[a_idx].point.x.0;
                    }
                }
                if contour[a_idx].node_type == NodeType::Smooth {
                    let opp = if prev == Some(a_idx) {
                        if a_idx > 0 {
                            Some(a_idx - 1)
                        } else if closed {
                            Some(len - 1)
                        } else {
                            None
                        }
                    } else if a_idx + 1 < len {
                        Some(a_idx + 1)
                    } else if closed {
                        Some(0)
                    } else {
                        None
                    };
                    if let Some(b_idx) = opp.filter(|&b| selected_nodes.contains(&contour[b].id)) {
                        contour[i].point = crate::core::align_collinear_opposite(
                            contour[a_idx].point,
                            contour[i].point,
                            contour[b_idx].point,
                        );
                    }
                }
            }
        }
    }
    contours != before
}

fn close_open_contour_endpoints(contour: &mut Vec<Node>) {
    let p_first = contour.first().unwrap().point;
    let p_last = contour.last().unwrap().point;
    let on_c = contour
        .iter()
        .filter(|n| n.node_type != NodeType::OffCurve)
        .count();
    if p_first.distance_to(p_last) < CONNECT_MERGE_DIST_UPM && on_c > 2 {
        contour.pop();
        contour.first_mut().unwrap().node_type = NodeType::Corner;
    } else {
        contour.first_mut().unwrap().node_type = NodeType::Corner;
        contour.last_mut().unwrap().node_type = NodeType::Corner;
    }
}

pub fn connect_selected_open_endpoints_in_contours(
    contours: &mut Vec<Vec<Node>>,
    closed_flags: &mut Vec<bool>,
    selected_nodes: &[NodeId],
) -> bool {
    let mut endpoints = Vec::new();
    for (c_idx, contour) in contours.iter().enumerate() {
        if closed_flags.get(c_idx).copied().unwrap_or(true) || contour.len() < 2 {
            continue;
        }
        let first_id = contour.first().unwrap().id;
        let last_id = contour.last().unwrap().id;
        if selected_nodes.contains(&first_id) {
            endpoints.push((c_idx, true, first_id));
        }
        if selected_nodes.contains(&last_id) {
            endpoints.push((c_idx, false, last_id));
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
    let pa = contours[ca].last().unwrap().point;
    let pb = contours[cb].first().unwrap().point;
    contours[ca].last_mut().unwrap().node_type = NodeType::Corner;
    contours[cb].first_mut().unwrap().node_type = NodeType::Corner;
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
