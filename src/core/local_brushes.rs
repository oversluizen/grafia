//! `SPEC-013 §2.3`: Local `Trowel` (Windowed Contour Smoothing & Node Reduction)
//! and `Cleaner` (Micro-Loop, Collinear Redundancy & Stray-Spike Healer) Brushes.

use crate::core::bezier_ops::{Node, NodeId, NodeType};
use crate::core::boolean_ops::contour_signed_area;
use crate::core::coordinates::FontPoint;
use crate::core::knife_ops::intersect_finite_segments;
use crate::core::math_consts::{
    CLEANER_DUPLICATE_DIST_UPM, CLEANER_MAX_LOOP_WINDOW_NODES, CLEANER_SPIKE_COS_THRESHOLD,
    SOLVER_DET_EPSILON, TIDY_COLLINEAR_SLOPE_EPS, TROWEL_HANDLE_FRACTION, TROWEL_MIN_TOLERANCE_UPM,
};
use crate::core::primitives_ops::simplify_polyline_rdp;

fn find_contiguous_inside_window(
    nodes: &[Node],
    center: FontPoint,
    radius: f64,
) -> Option<(usize, usize)> {
    let mut first = None;
    let mut last = None;
    for (i, n) in nodes.iter().enumerate() {
        if n.point.distance_to(center) <= radius {
            if first.is_none() {
                first = Some(i);
            }
            last = Some(i);
        } else if first.is_some() {
            break;
        }
    }
    match (first, last) {
        (Some(s), Some(e)) if e >= s + 2 => Some((s, e)),
        _ => None,
    }
}

fn refit_inside_window(
    window: &[Node],
    fit_tolerance_upm: f64,
    next_node_id: &mut u64,
) -> Option<Vec<Node>> {
    if window.len() < 3 {
        return None;
    }
    let first = window.first()?.clone();
    let last = window.last()?.clone();
    if first.node_type == NodeType::OffCurve || last.node_type == NodeType::OffCurve {
        return None;
    }
    let pts: Vec<FontPoint> = window.iter().map(|n| n.point).collect();
    let tol = fit_tolerance_upm.max(TROWEL_MIN_TOLERANCE_UPM);
    let rdp = simplify_polyline_rdp(&pts, tol);
    if rdp.len() <= 2 {
        return Some(vec![first, last]);
    }
    if window.len() >= 5 {
        let p0 = first.point;
        let p3 = last.point;
        let chord = p0.distance_to(p3);
        if chord > SOLVER_DET_EPSILON {
            let p1_ref = window[1].point;
            let p2_ref = window[window.len() - 2].point;
            let d1 = p0.distance_to(p1_ref).max(SOLVER_DET_EPSILON);
            let d2 = p3.distance_to(p2_ref).max(SOLVER_DET_EPSILON);
            let h_len = chord * TROWEL_HANDLE_FRACTION;
            let h1 = FontPoint::new(
                p0.x.0 + (p1_ref.x.0 - p0.x.0) / d1 * h_len,
                p0.y.0 + (p1_ref.y.0 - p0.y.0) / d1 * h_len,
            );
            let h2 = FontPoint::new(
                p3.x.0 + (p2_ref.x.0 - p3.x.0) / d2 * h_len,
                p3.y.0 + (p2_ref.y.0 - p3.y.0) / d2 * h_len,
            );
            let id1 = NodeId(*next_node_id);
            *next_node_id += 1;
            let id2 = NodeId(*next_node_id);
            *next_node_id += 1;
            return Some(vec![
                first,
                Node::new(id1, h1.x.0, h1.y.0, NodeType::OffCurve),
                Node::new(id2, h2.x.0, h2.y.0, NodeType::OffCurve),
                last,
            ]);
        }
    }
    if rdp.len() < window.len() {
        let mut out = Vec::with_capacity(rdp.len());
        out.push(first);
        for pt in &rdp[1..rdp.len() - 1] {
            let id = NodeId(*next_node_id);
            *next_node_id += 1;
            out.push(Node::new(id, pt.x.0, pt.y.0, NodeType::Corner));
        }
        out.push(last);
        return Some(out);
    }
    None
}

/// Applies the `Trowel` local smoothing brush to all sub-chains inside $\mathcal{B}(c, R_{\text{upm}})$,
/// keeping every node outside $\mathcal{B}(c, R_{\text{upm}})$ 100% bit-identical (`SPEC-013 §2.3.1`).
pub fn apply_trowel_brush_in_contours(
    contours: &mut [Vec<Node>],
    closed_flags: &[bool],
    center_upm: FontPoint,
    radius_upm: f64,
    fit_tolerance_upm: f64,
    next_node_id: &mut u64,
) -> bool {
    debug_assert!(radius_upm.is_finite() && fit_tolerance_upm.is_finite());
    if !radius_upm.is_finite() || radius_upm <= 0.0 || !fit_tolerance_upm.is_finite() {
        return false;
    }
    let mut changed = false;
    for (c_idx, nodes) in contours.iter_mut().enumerate() {
        let closed = closed_flags.get(c_idx).copied().unwrap_or(true);
        let min_len = if closed { 3 } else { 2 };
        if nodes.len() <= min_len {
            continue;
        }
        let mut rot = 0usize;
        if closed
            && nodes
                .first()
                .is_some_and(|n| n.point.distance_to(center_upm) <= radius_upm)
            && nodes
                .last()
                .is_some_and(|n| n.point.distance_to(center_upm) <= radius_upm)
        {
            if let Some(out_idx) = nodes
                .iter()
                .position(|n| n.point.distance_to(center_upm) > radius_upm)
            {
                nodes.rotate_left(out_idx);
                rot = out_idx;
            }
        }
        while let Some((s, e)) = find_contiguous_inside_window(nodes, center_upm, radius_upm) {
            let remaining = nodes.len() - (e - s + 1);
            let mut spliced = false;
            if let Some(replacement) =
                refit_inside_window(&nodes[s..=e], fit_tolerance_upm, next_node_id)
            {
                if remaining + replacement.len() >= min_len && replacement.len() < (e - s + 1) {
                    nodes.splice(s..=e, replacement);
                    changed = true;
                    spliced = true;
                }
            }
            if !spliced {
                break;
            }
        }
        if rot > 0 && !changed {
            let n = nodes.len();
            nodes.rotate_right(rot % n);
        }
    }
    changed
}

fn prune_duplicates_in_disc(
    nodes: &mut Vec<Node>,
    closed: bool,
    center: FontPoint,
    radius: f64,
    min_len: usize,
) -> bool {
    let mut any = false;
    let mut i = 0;
    while i < nodes.len() && nodes.len() > min_len {
        if nodes[i].point.distance_to(center) <= radius {
            let n = nodes.len();
            let prev_idx = if i > 0 {
                Some(i - 1)
            } else if closed {
                Some(n - 1)
            } else {
                None
            };
            let is_dup = prev_idx.is_some_and(|p| {
                nodes[p].point.distance_to(center) <= radius
                    && nodes[i].point.distance_to(nodes[p].point) <= CLEANER_DUPLICATE_DIST_UPM
            });
            if is_dup {
                nodes.remove(i);
                any = true;
                continue;
            }
        }
        i += 1;
    }
    any
}

fn try_prune_one_micro_loop(
    nodes: &mut Vec<Node>,
    closed: bool,
    center: FontPoint,
    radius: f64,
    max_area: f64,
    min_len: usize,
) -> bool {
    let max_w = CLEANER_MAX_LOOP_WINDOW_NODES.min(nodes.len().saturating_sub(min_len));
    for w in (2..=max_w).rev() {
        let n = nodes.len();
        let limit = if closed { n } else { n.saturating_sub(w + 1) };
        for start in 0..limit {
            if !closed && (start == 0 || start + w >= n) {
                continue;
            }
            let win_indices: Vec<usize> = (0..w).map(|k| (start + k) % n).collect();
            if !win_indices
                .iter()
                .all(|&idx| nodes[idx].point.distance_to(center) <= radius)
            {
                continue;
            }
            let prev_idx = (start + n - 1) % n;
            let next_idx = (start + w) % n;
            let mut sub_nodes: Vec<Node> =
                win_indices.iter().map(|&idx| nodes[idx].clone()).collect();
            let area = contour_signed_area(&sub_nodes).abs();
            let mut x_min = f64::INFINITY;
            let mut x_max = f64::NEG_INFINITY;
            let mut y_min = f64::INFINITY;
            let mut y_max = f64::NEG_INFINITY;
            for nd in &sub_nodes {
                x_min = x_min.min(nd.point.x.0);
                x_max = x_max.max(nd.point.x.0);
                y_min = y_min.min(nd.point.y.0);
                y_max = y_max.max(nd.point.y.0);
            }
            let bbox_area = (x_max - x_min) * (y_max - y_min);
            sub_nodes.insert(0, nodes[prev_idx].clone());
            sub_nodes.push(nodes[next_idx].clone());
            let mut self_crosses = false;
            for a in 0..sub_nodes.len().saturating_sub(1) {
                for b in (a + 2)..sub_nodes.len().saturating_sub(1) {
                    if intersect_finite_segments(
                        sub_nodes[a].point,
                        sub_nodes[a + 1].point,
                        sub_nodes[b].point,
                        sub_nodes[b + 1].point,
                    )
                    .is_some()
                    {
                        self_crosses = true;
                    }
                }
            }
            if (self_crosses && area <= max_area * 2.0)
                || (bbox_area > 0.0 && bbox_area <= max_area)
            {
                let mut remove_sorted = win_indices;
                remove_sorted.sort_unstable();
                for &idx in remove_sorted.iter().rev() {
                    if nodes.len() > min_len {
                        nodes.remove(idx);
                    }
                }
                return true;
            }
        }
    }
    false
}

fn prune_collinear_and_spikes_in_disc(
    nodes: &mut Vec<Node>,
    closed: bool,
    center: FontPoint,
    radius: f64,
    min_len: usize,
) -> bool {
    let mut any = false;
    let mut i = 0;
    while i < nodes.len() && nodes.len() > min_len {
        let n = nodes.len();
        if (!closed && (i == 0 || i + 1 == n))
            || nodes[i].node_type == NodeType::OffCurve
            || nodes[i].point.distance_to(center) > radius
        {
            i += 1;
            continue;
        }
        let prev = nodes[(i + n - 1) % n].clone();
        let curr = nodes[i].clone();
        let next = nodes[(i + 1) % n].clone();
        if prev.node_type != NodeType::OffCurve && next.node_type != NodeType::OffCurve {
            let v1x = curr.point.x.0 - prev.point.x.0;
            let v1y = curr.point.y.0 - prev.point.y.0;
            let v2x = next.point.x.0 - curr.point.x.0;
            let v2y = next.point.y.0 - curr.point.y.0;
            let l1 = v1x.hypot(v1y);
            let l2 = v2x.hypot(v2y);
            if l1 > SOLVER_DET_EPSILON && l2 > SOLVER_DET_EPSILON {
                let cross = ((v1x * v2y - v1y * v2x) / (l1 * l2)).abs();
                let dot = (v1x * v2x + v1y * v2y) / (l1 * l2);
                if cross <= TIDY_COLLINEAR_SLOPE_EPS || dot <= CLEANER_SPIKE_COS_THRESHOLD {
                    nodes.remove(i);
                    any = true;
                    continue;
                }
            }
        }
        i += 1;
    }
    any
}

/// Applies the `Cleaner` brush to prune self-intersecting micro-loops ($|A| < \text{max\_loop\_area\_upm2}$),
/// collinear redundant nodes, and duplicate nodes inside $\mathcal{B}(c, R_{\text{upm}})$ (`SPEC-013 §2.3.2`).
pub fn apply_cleaner_brush_in_contours(
    contours: &mut [Vec<Node>],
    closed_flags: &[bool],
    center_upm: FontPoint,
    radius_upm: f64,
    max_loop_area_upm2: f64,
) -> bool {
    debug_assert!(radius_upm.is_finite() && max_loop_area_upm2.is_finite());
    if !radius_upm.is_finite() || radius_upm <= 0.0 || !max_loop_area_upm2.is_finite() {
        return false;
    }
    let max_area = max_loop_area_upm2.abs();
    let mut changed = false;
    for (c_idx, nodes) in contours.iter_mut().enumerate() {
        let closed = closed_flags.get(c_idx).copied().unwrap_or(true);
        let min_len = if closed { 3 } else { 2 };
        if nodes.len() <= min_len {
            continue;
        }
        let mut rot = 0usize;
        if closed
            && nodes
                .first()
                .is_some_and(|n| n.point.distance_to(center_upm) <= radius_upm)
            && nodes
                .last()
                .is_some_and(|n| n.point.distance_to(center_upm) <= radius_upm)
        {
            if let Some(out_idx) = nodes
                .iter()
                .position(|n| n.point.distance_to(center_upm) > radius_upm)
            {
                nodes.rotate_left(out_idx);
                rot = out_idx;
            }
        }
        if prune_duplicates_in_disc(nodes, closed, center_upm, radius_upm, min_len) {
            changed = true;
        }
        while try_prune_one_micro_loop(nodes, closed, center_upm, radius_upm, max_area, min_len) {
            changed = true;
        }
        if prune_collinear_and_spikes_in_disc(nodes, closed, center_upm, radius_upm, min_len) {
            changed = true;
        }
        if rot > 0 && !changed {
            let n = nodes.len();
            nodes.rotate_right(rot % n);
        }
    }
    changed
}
