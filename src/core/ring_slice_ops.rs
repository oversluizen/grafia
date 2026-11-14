//! Closed-ring merging and splitting helpers for knife path-slicing operations.

use crate::core::bezier_ops::{Node, NodeId, NodeType};
use crate::core::coordinates::FontPoint;
use crate::core::math_consts::KNIFE_PARAM_DEDUP_EPS;
use crate::core::segment_slide::reverse_contour_nodes;

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
        .find(|p| p.distance_to(pt_a) > KNIFE_PARAM_DEDUP_EPS)
        .unwrap_or(pt_a);
    let cross_a = dx * (next_a.y.0 - pt_a.y.0) - dy * (next_a.x.0 - pt_a.x.0);

    let mut oriented_b = ring_b.to_vec();
    let pt_b = oriented_b[idx_b].point;
    let next_b = (1..oriented_b.len())
        .map(|k| oriented_b[(idx_b + k) % oriented_b.len()].point)
        .find(|p| p.distance_to(pt_b) > KNIFE_PARAM_DEDUP_EPS)
        .unwrap_or(pt_b);
    let cross_b = dx * (next_b.y.0 - pt_b.y.0) - dy * (next_b.x.0 - pt_b.x.0);
    if cross_a * cross_b > 0.0 {
        reverse_contour_nodes(&mut oriented_b, true);
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
