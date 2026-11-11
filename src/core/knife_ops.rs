//! `CanvasTool` (`E` / `Shift+E`) contour intersection, node insertion, and path slicing (`SPEC-005 §2.1`).
//!
//! Extracted 1-to-1 from `CanvasTool` (`SPEC-005`) in Reference Vector (``).

use crate::core::{
    extract_segments, hit_test_segments, reverse_contour_nodes, split_contour_segment,
    ContourSegment, FontPoint, Node, NodeId, NodeType,
};
use crate::tokens::{
    KNIFE_CUBIC_SUBDIVISIONS, KNIFE_MIN_DRAG_UPM, KNIFE_PARAM_DEDUP_EPS, KNIFE_SEG_T_MAX,
    KNIFE_SEG_T_MIN, SNAP_LINE_INTERSECT_DET_EPS,
};
use kurbo::ParamCurve;

/// Applies the `Shift` axis constraint (SPEC-005).
#[must_use]
pub fn constrain_knife_endpoint(start: FontPoint, raw: FontPoint, shift: bool) -> FontPoint {
    if !shift {
        return raw;
    }
    let dx = (raw.x.0 - start.x.0).abs();
    let dy = (raw.y.0 - start.y.0).abs();
    if dx > dy {
        FontPoint::new(raw.x.0, start.y.0)
    } else {
        FontPoint::new(start.x.0, raw.y.0)
    }
}

/// Finite line-segment to finite line-segment intersection returning `(u_line_a, v_line_b, point)`.
#[must_use]
pub fn intersect_finite_segments(
    a0: FontPoint,
    a1: FontPoint,
    b0: FontPoint,
    b1: FontPoint,
) -> Option<(f64, f64, FontPoint)> {
    let r_x = a1.x.0 - a0.x.0;
    let r_y = a1.y.0 - a0.y.0;
    let s_x = b1.x.0 - b0.x.0;
    let s_y = b1.y.0 - b0.y.0;
    let det = r_x * s_y - r_y * s_x;
    if det.abs() <= SNAP_LINE_INTERSECT_DET_EPS {
        return None;
    }
    let qp_x = b0.x.0 - a0.x.0;
    let qp_y = b0.y.0 - a0.y.0;
    let u = (qp_x * s_y - qp_y * s_x) / det;
    let v = (qp_x * r_y - qp_y * r_x) / det;
    let range = -KNIFE_PARAM_DEDUP_EPS..=(1.0 + KNIFE_PARAM_DEDUP_EPS);
    if range.contains(&u) && range.contains(&v) {
        let u_clamped = u.clamp(0.0, 1.0);
        let v_clamped = v.clamp(0.0, 1.0);
        Some((
            u_clamped,
            v_clamped,
            FontPoint::new(a0.x.0 + u_clamped * r_x, a0.y.0 + u_clamped * r_y),
        ))
    } else {
        None
    }
}

/// Computes all intersections between the finite knife segment `(start, end)` and a single contour.
/// Returns `(u_along_knife, intersection_point)` sorted by `u_along_knife`.
#[must_use]
pub fn contour_intersections_with_segment(
    nodes: &[Node],
    closed: bool,
    start: FontPoint,
    end: FontPoint,
) -> Vec<(f64, FontPoint)> {
    let mut hits: Vec<(f64, FontPoint)> = Vec::new();
    for seg in extract_segments(nodes, closed) {
        match seg {
            ContourSegment::Line { line, .. } => {
                let b0 = FontPoint::new(line.p0.x, line.p0.y);
                let b1 = FontPoint::new(line.p1.x, line.p1.y);
                if let Some((u, _v, pt)) = intersect_finite_segments(start, end, b0, b1) {
                    hits.push((u, pt));
                }
            }
            ContourSegment::Cubic { cubic, .. } => {
                let n = KNIFE_CUBIC_SUBDIVISIONS;
                for step in 0..n {
                    let t0 = step as f64 / n as f64;
                    let t1 = (step + 1) as f64 / n as f64;
                    let p0 = cubic.eval(t0);
                    let p1 = cubic.eval(t1);
                    let b0 = FontPoint::new(p0.x, p0.y);
                    let b1 = FontPoint::new(p1.x, p1.y);
                    if let Some((u, v_sub, _)) = intersect_finite_segments(start, end, b0, b1) {
                        let t_exact = (t0 + v_sub * (t1 - t0)).clamp(0.0, 1.0);
                        let p_exact = cubic.eval(t_exact);
                        hits.push((u, FontPoint::new(p_exact.x, p_exact.y)));
                    }
                }
            }
        }
    }
    hits.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap_or(std::cmp::Ordering::Equal));
    let mut deduped: Vec<(f64, FontPoint)> = Vec::with_capacity(hits.len());
    for (u, pt) in hits {
        if deduped.last().is_none_or(|(prev_u, prev_pt)| {
            (u - *prev_u).abs() > KNIFE_PARAM_DEDUP_EPS
                && pt.distance_to(*prev_pt) > KNIFE_MIN_DRAG_UPM
        }) {
            deduped.push((u, pt));
        }
    }
    deduped
}

fn resolve_or_insert_cut_node(
    contour: &mut Vec<Node>,
    closed: bool,
    pt: FontPoint,
    next_node_id: &mut u64,
) -> Option<NodeId> {
    if let Some(existing) = contour
        .iter()
        .find(|n| n.node_type != NodeType::OffCurve && n.point.distance_to(pt) <= KNIFE_MIN_DRAG_UPM)
    {
        return Some(existing.id);
    }
    let seg_hit = hit_test_segments(std::slice::from_ref(contour), &[closed], pt, 2.0)?;
    let t_clamped = seg_hit.t.clamp(KNIFE_SEG_T_MIN, KNIFE_SEG_T_MAX);
    Some(split_contour_segment(
        contour,
        seg_hit.segment,
        t_clamped,
        next_node_id,
    ))
}

/// Computes all intersections between `(start, end)` and all contours in the layer, sorted along the knife line.
#[must_use]
pub fn compute_knife_intersections(
    contours: &[Vec<Node>],
    closed_flags: &[bool],
    start: FontPoint,
    end: FontPoint,
) -> Vec<FontPoint> {
    let mut all_hits: Vec<(f64, FontPoint)> = Vec::new();
    for (c_idx, nodes) in contours.iter().enumerate() {
        let closed = closed_flags.get(c_idx).copied().unwrap_or(true);
        all_hits.extend(contour_intersections_with_segment(nodes, closed, start, end));
    }
    all_hits.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap_or(std::cmp::Ordering::Equal));
    all_hits.into_iter().map(|(_, pt)| pt).collect()
}

/// Inserts `OnCurve` nodes at all intersections along `(start, end)` without splitting contours
/// (`CanvasTool::addNodesInLayer:forPoint:endPoint:` at `spec_ref`).
pub fn knife_add_nodes_in_layer(
    contours: &mut [Vec<Node>],
    closed_flags: &[bool],
    start: FontPoint,
    end: FontPoint,
    next_node_id: &mut u64,
) -> Vec<NodeId> {
    let mut inserted = Vec::new();
    for (c_idx, contour) in contours.iter_mut().enumerate() {
        let closed = closed_flags.get(c_idx).copied().unwrap_or(true);
        let hits = contour_intersections_with_segment(contour, closed, start, end);
        for (_, pt) in hits {
            if let Some(id) = resolve_or_insert_cut_node(contour, closed, pt, next_node_id) {
                if !inserted.contains(&id) {
                    inserted.push(id);
                }
            }
        }
    }
    inserted
}

/// Slices contours along `(start, end)` (`CanvasTool::cutPathsInLayer:forPoint:endPoint:` at `spec_ref`).
/// - Pair(s) of hits on closed contours -> splits or merges+splits across stems/counters (`reconnectNodesAtNode1:node2:`).
/// - Single hit on a closed contour -> inserts node and opens the contour at the hit node (`dividePathAtNode:`).
/// - Hits on an open contour -> splits the open contour at each hit node.
pub fn knife_cut_paths_in_layer(
    contours: &mut Vec<Vec<Node>>,
    closed_flags: &mut Vec<bool>,
    start: FontPoint,
    end: FontPoint,
    next_node_id: &mut u64,
) -> bool {
    let mut new_contours: Vec<Vec<Node>> = Vec::with_capacity(contours.len() + 2);
    let mut new_closed: Vec<bool> = Vec::with_capacity(closed_flags.len() + 2);
    let mut touched_closed: Vec<Vec<Node>> = Vec::new();
    let mut closed_hits: Vec<(f64, FontPoint, NodeId)> = Vec::new();
    let mut mutated = false;

    for (c_idx, orig_contour) in contours.iter().enumerate() {
        let is_closed = closed_flags.get(c_idx).copied().unwrap_or(true);
        let hits = contour_intersections_with_segment(orig_contour, is_closed, start, end);
        if hits.is_empty() {
            new_contours.push(orig_contour.clone());
            new_closed.push(is_closed);
            continue;
        }
        mutated = true;
        let mut working = orig_contour.clone();
        let mut cut_ids: Vec<(f64, FontPoint, NodeId)> = Vec::with_capacity(hits.len());
        for (u, pt) in &hits {
            if let Some(id) = resolve_or_insert_cut_node(&mut working, is_closed, *pt, next_node_id)
            {
                if !cut_ids.iter().any(|(_, _, cid)| *cid == id) {
                    cut_ids.push((*u, *pt, id));
                }
            }
        }

        if is_closed {
            closed_hits.extend(cut_ids);
            touched_closed.push(working);
        } else {
            // Open contour: split at each OnCurve cut_id into independent open sub-contours
            let mut pieces = vec![working];
            for (_, _, cid) in cut_ids {
                if let Some(p_idx) = pieces.iter().position(|p| {
                    p.iter().position(|n| n.id == cid).is_some_and(|i| {
                        i > 0 && i + 1 < p.len() && p[i].node_type != NodeType::OffCurve
                    })
                }) {
                    let piece = pieces.remove(p_idx);
                    let cut_idx = piece.iter().position(|n| n.id == cid).unwrap_or(0);
                    let mut part1 = piece[..=cut_idx].to_vec();
                    let mut part2 = piece[cut_idx..].to_vec();
                    if let Some(last) = part1.last_mut() {
                        last.node_type = NodeType::Corner;
                    }
                    if let Some(first) = part2.first_mut() {
                        first.id = NodeId(*next_node_id);
                        *next_node_id += 1;
                        first.node_type = NodeType::Corner;
                    }
                    let on1 = part1.iter().filter(|n| n.node_type != NodeType::OffCurve).count();
                    let on2 = part2.iter().filter(|n| n.node_type != NodeType::OffCurve).count();
                    if on1 >= 2 {
                        pieces.push(part1);
                    }
                    if on2 >= 2 {
                        pieces.push(part2);
                    }
                }
            }
            for piece in pieces {
                new_contours.push(piece);
                new_closed.push(false);
            }
        }
    }

    if !touched_closed.is_empty() {
        closed_hits.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap_or(std::cmp::Ordering::Equal));
        let (pairs, remainder) = closed_hits.as_chunks::<2>();
        for &[(_, _, id_a), (_, _, id_b)] in pairs {
            let pos_a = touched_closed.iter().position(|r| {
                r.iter().any(|n| n.id == id_a && n.node_type != NodeType::OffCurve)
            });
            let pos_b = touched_closed.iter().position(|r| {
                r.iter().any(|n| n.id == id_b && n.node_type != NodeType::OffCurve)
            });
            match (pos_a, pos_b) {
                (Some(r_a), Some(r_b)) if r_a == r_b => {
                    let ring = touched_closed.remove(r_a);
                    let ia = ring.iter().position(|n| n.id == id_a).unwrap_or(0);
                    let ib = ring.iter().position(|n| n.id == id_b).unwrap_or(0);
                    if ia != ib {
                        let (half1, half2) =
                            split_closed_ring_at_indices(&ring, ia, ib, next_node_id);
                        touched_closed.push(half1);
                        touched_closed.push(half2);
                    } else {
                        touched_closed.push(ring);
                    }
                }
                (Some(r_a), Some(r_b)) => {
                    let (hi, lo) = if r_a > r_b { (r_a, r_b) } else { (r_b, r_a) };
                    let ring_hi = touched_closed.remove(hi);
                    let ring_lo = touched_closed.remove(lo);
                    let (ring_a, ring_b) =
                        if r_a < r_b { (ring_lo, ring_hi) } else { (ring_hi, ring_lo) };
                    let ia = ring_a.iter().position(|n| n.id == id_a).unwrap_or(0);
                    let ib = ring_b.iter().position(|n| n.id == id_b).unwrap_or(0);
                    touched_closed.push(merge_two_closed_rings_at_cut(
                        &ring_a,
                        ia,
                        &ring_b,
                        ib,
                        start,
                        end,
                        next_node_id,
                    ));
                }
                _ => {}
            }
        }
        if let &[(_, _, id_rem)] = remainder {
            if let Some(r_rem) = touched_closed.iter().position(|r| {
                r.iter().any(|n| n.id == id_rem && n.node_type != NodeType::OffCurve)
            }) {
                let working = touched_closed.remove(r_rem);
                if let Some(cut_idx) = working
                    .iter()
                    .position(|n| n.id == id_rem && n.node_type != NodeType::OffCurve)
                {
                    let len = working.len();
                    let mut opened: Vec<Node> =
                        (0..len).map(|k| working[(cut_idx + k) % len].clone()).collect();
                    opened[0].node_type = NodeType::Corner;
                    let end_pt = opened[0].point;
                    let end_id = NodeId(*next_node_id);
                    *next_node_id += 1;
                    opened.push(Node::new(end_id, end_pt.x.0, end_pt.y.0, NodeType::Corner));
                    new_contours.push(opened);
                    new_closed.push(false);
                } else {
                    touched_closed.push(working);
                }
            }
        }
        for ring in touched_closed {
            new_contours.push(ring);
            new_closed.push(true);
        }
    }

    if mutated {
        *contours = new_contours;
        *closed_flags = new_closed;
    }
    mutated
}

fn merge_two_closed_rings_at_cut(
    ring_a: &[Node],
    idx_a: usize,
    ring_b: &[Node],
    idx_b: usize,
    knife_start: FontPoint,
    knife_end: FontPoint,
    next_node_id: &mut u64,
) -> Vec<Node> {
    let (dx, dy) = (knife_end.x.0 - knife_start.x.0, knife_end.y.0 - knife_start.y.0);
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
    let ib = oriented_b.iter().position(|n| n.id == ring_b[idx_b].id).unwrap_or(0);

    let mut merged = Vec::with_capacity(ring_a.len() + oriented_b.len() + 2);
    for (ring, start_idx) in [(ring_a, idx_a), (oriented_b.as_slice(), ib)] {
        let len = ring.len();
        for k in 0..len {
            let mut node = ring[(start_idx + k) % len].clone();
            if k == 0 { node.node_type = NodeType::Corner; }
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

fn split_closed_ring_at_indices(
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
        if curr == idx_b { break; }
        curr = (curr + 1) % len;
    }
    if let Some(first) = half1.first_mut() { first.node_type = NodeType::Corner; }
    if let Some(last) = half1.last_mut() { last.node_type = NodeType::Corner; }

    let mut half2 = Vec::new();
    let mut curr2 = idx_b;
    loop {
        half2.push(ring[curr2].clone());
        if curr2 == idx_a { break; }
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
