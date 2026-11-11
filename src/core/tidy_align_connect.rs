//! `SPEC-003 §2.6.5–§2.6.7` `tidy_up_contours`, `checkConnections`, duplicate-node warning circles,
//! `align_selected_nodes` (`_GSAlignedPointForBounds` + `_alignOtherNodes:`), and `connect_selected_open_endpoints`.

use crate::core::bezier_ops::{Node, NodeId, NodeType};
use crate::core::coordinates::FontPoint;
use crate::core::curve_manipulation::{extract_segments, ContourSegment};
use crate::core::handle_constraints::enforce_smooth_collinearity_on_handle_drag;
use crate::core::path_ops::round_grid_pt;
use crate::core::transform_ops::{compute_selection_bounds, TransformGridAnchor};
use crate::tokens::{
    COLLINEAR_MIN_LEN_EPSILON, CONNECT_MERGE_DIST_UPM, GS_COLINEAR_PERP_FACTOR, PATH_COORD_EPS_UPM,
    TIDY_COLLINEAR_SLOPE_EPS, TIDY_FINE_GRID_THRESHOLD, TIDY_MERGE_ONCURVE_COARSE_UPM,
    TIDY_MERGE_ONCURVE_FINE_UPM, TIDY_MINI_CONTOUR_COARSE_UPM, TIDY_MINI_CONTOUR_FINE_UPM,
    TIDY_ZERO_HANDLE_EPS_UPM, WARNING_OFFCURVE_DIST_UPM,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DuplicateWarningKind {
    CoincidentOnCurve,
    CoincidentOffCurve,
}

#[must_use]
pub fn find_duplicate_warning_nodes(
    contours: &[Vec<Node>],
    closed_flags: &[bool],
    actively_dragging: bool,
) -> Vec<(NodeId, FontPoint, DuplicateWarningKind)> {
    if actively_dragging {
        return Vec::new();
    }
    let mut warnings = Vec::new();
    for (c_idx, contour) in contours.iter().enumerate() {
        let closed = closed_flags.get(c_idx).copied().unwrap_or(true);
        let on_nodes: Vec<&Node> = contour.iter().filter(|n| n.node_type != NodeType::OffCurve).collect();
        if on_nodes.len() >= 2 {
            let pair_count = if closed { on_nodes.len() } else { on_nodes.len() - 1 };
            for i in 0..pair_count {
                let (a, b) = (on_nodes[i], on_nodes[(i + 1) % on_nodes.len()]);
                if a.point.distance_to(b.point) < CONNECT_MERGE_DIST_UPM {
                    warnings.push((b.id, b.point, DuplicateWarningKind::CoincidentOnCurve));
                }
            }
        }
        for seg in extract_segments(contour, closed) {
            if let ContourSegment::Cubic { h1_idx, h2_idx, .. } = seg {
                let (h1, h2) = (&contour[h1_idx], &contour[h2_idx]);
                if h1.point.distance_to(h2.point) < WARNING_OFFCURVE_DIST_UPM {
                    warnings.push((h2.id, h2.point, DuplicateWarningKind::CoincidentOffCurve));
                }
            }
        }
    }
    warnings
}

fn gs_colinear_lines(a: FontPoint, b: FontPoint, c: FontPoint) -> bool {
    let (l1, l2, l_ac) = (a.distance_to(b), b.distance_to(c), a.distance_to(c));
    if l1 <= COLLINEAR_MIN_LEN_EPSILON || l2 <= COLLINEAR_MIN_LEN_EPSILON || l1 >= l_ac || l2 >= l_ac {
        return false;
    }
    let cross = ((c.x.0 - a.x.0) * (a.y.0 - b.y.0) - (a.x.0 - b.x.0) * (c.y.0 - a.y.0)).abs();
    (cross / l_ac) < GS_COLINEAR_PERP_FACTOR * l1.min(l2).sqrt()
}

pub fn tidy_up_contours(
    contours: &mut Vec<Vec<Node>>,
    closed_flags: &mut Vec<bool>,
    selected_nodes: &[NodeId],
    grid_step_upm: f64,
) -> bool {
    let before_c = contours.clone();
    let before_f = closed_flags.clone();
    let check_sel = !selected_nodes.is_empty();
    let mini_thresh = if grid_step_upm < TIDY_FINE_GRID_THRESHOLD {
        TIDY_MINI_CONTOUR_FINE_UPM
    } else {
        TIDY_MINI_CONTOUR_COARSE_UPM
    };
    let eps_on = if grid_step_upm < TIDY_FINE_GRID_THRESHOLD {
        TIDY_MERGE_ONCURVE_FINE_UPM
    } else {
        TIDY_MERGE_ONCURVE_COARSE_UPM
    };

    let mut kept_c = Vec::new();
    let mut kept_f = Vec::new();
    for (c_idx, mut contour) in contours.drain(..).enumerate() {
        let mut closed = closed_flags.get(c_idx).copied().unwrap_or(true);
        let in_scope = !check_sel || contour.iter().any(|n| selected_nodes.contains(&n.id));
        if !in_scope {
            kept_c.push(contour);
            kept_f.push(closed);
            continue;
        }
        let on_count = contour.iter().filter(|n| n.node_type != NodeType::OffCurve).count();
        let min_x = contour.iter().map(|n| n.point.x.0).fold(f64::INFINITY, f64::min);
        let max_x = contour.iter().map(|n| n.point.x.0).fold(f64::NEG_INFINITY, f64::max);
        let min_y = contour.iter().map(|n| n.point.y.0).fold(f64::INFINITY, f64::min);
        let max_y = contour.iter().map(|n| n.point.y.0).fold(f64::NEG_INFINITY, f64::max);
        if on_count < 2 || ((max_x - min_x) < mini_thresh && (max_y - min_y) < mini_thresh) {
            continue;
        }

        // Retract zero-length or collinear cubic handles -> straight Line
        let segs = extract_segments(&contour, closed);
        let mut remove_ids = Vec::new();
        for seg in &segs {
            if let ContourSegment::Cubic { start_idx, h1_idx, h2_idx, end_idx, .. } = *seg {
                let p0 = contour[start_idx].point;
                let h1 = contour[h1_idx].point;
                let h2 = contour[h2_idx].point;
                let p3 = contour[end_idx].point;
                let zero_handles = p0.distance_to(h1) < TIDY_ZERO_HANDLE_EPS_UPM
                    && p3.distance_to(h2) < TIDY_ZERO_HANDLE_EPS_UPM;
                let col_handles = gs_colinear_lines(p0, h1, p3) && gs_colinear_lines(p0, h2, p3);
                if zero_handles || col_handles {
                    remove_ids.push(contour[h1_idx].id);
                    remove_ids.push(contour[h2_idx].id);
                }
            }
        }
        if !remove_ids.is_empty() {
            contour.retain(|n| !remove_ids.contains(&n.id));
        }

        // Merge coincident consecutive OnCurve nodes connected by straight Line
        let mut i = 0;
        while i < contour.len() {
            let on_c = contour.iter().filter(|n| n.node_type != NodeType::OffCurve).count();
            if on_c <= 2 {
                break;
            }
            let next_i = if i + 1 < contour.len() {
                Some(i + 1)
            } else if closed && contour.len() >= 2 {
                Some(0)
            } else {
                None
            };
            let Some(ni) = next_i else { break; };
            if ni != i
                && contour[i].node_type != NodeType::OffCurve
                && contour[ni].node_type != NodeType::OffCurve
                && contour[i].point.distance_to(contour[ni].point) < eps_on
            {
                let rem_idx = if ni == 0 { i } else { ni };
                contour.remove(rem_idx);
                if rem_idx <= i && i > 0 {
                    i -= 1;
                }
            } else {
                i += 1;
            }
        }

        // Collinear interior Line node removal
        let mut idx = 0;
        while idx < contour.len() && contour.len() >= 3 {
            if !closed && (idx == 0 || idx + 1 >= contour.len()) {
                idx += 1;
                continue;
            }
            let prev_i = if idx > 0 { idx - 1 } else { contour.len() - 1 };
            let next_i = (idx + 1) % contour.len();
            if contour[prev_i].node_type != NodeType::OffCurve
                && contour[idx].node_type != NodeType::OffCurve
                && contour[next_i].node_type != NodeType::OffCurve
            {
                let a = contour[prev_i].point;
                let b = contour[idx].point;
                let c = contour[next_i].point;
                let dx1 = b.x.0 - a.x.0;
                let dy1 = b.y.0 - a.y.0;
                let dx2 = c.x.0 - b.x.0;
                let dy2 = c.y.0 - b.y.0;
                let forward = dx1 * dx2 + dy1 * dy2 > 0.0;
                let same_x = dx1.abs() < PATH_COORD_EPS_UPM && dx2.abs() < PATH_COORD_EPS_UPM;
                let same_y = dy1.abs() < PATH_COORD_EPS_UPM && dy2.abs() < PATH_COORD_EPS_UPM;
                let same_slope = dx1.abs() >= PATH_COORD_EPS_UPM
                    && dx2.abs() >= PATH_COORD_EPS_UPM
                    && ((dy1 / dx1) - (dy2 / dx2)).abs() < TIDY_COLLINEAR_SLOPE_EPS;
                if forward && (same_x || same_y || same_slope) {
                    contour.remove(idx);
                    continue;
                }
            }
            idx += 1;
        }

        // Open-contour auto-close (< 1.0 UPM)
        let rem_on = contour.iter().filter(|n| n.node_type != NodeType::OffCurve).count();
        if rem_on < 2 {
            continue;
        }
        if !closed && rem_on >= 3 {
            let p_first = contour.first().unwrap().point;
            let p_last = contour.last().unwrap().point;
            if p_first.distance_to(p_last) < CONNECT_MERGE_DIST_UPM {
                contour.pop();
                closed = true;
            }
        }

        // checkConnections (collinear_lines)
        let len = contour.len();
        if len >= 3 {
            let mut next_types: Vec<NodeType> = contour.iter().map(|n| n.node_type).collect();
            for k in 0..len {
                if contour[k].node_type == NodeType::OffCurve {
                    continue;
                }
                if !closed && (k == 0 || k + 1 == len) {
                    next_types[k] = NodeType::Corner;
                    continue;
                }
                let prev_k = if k > 0 { k - 1 } else { len - 1 };
                let next_k = (k + 1) % len;
                let has_off = contour[prev_k].node_type == NodeType::OffCurve || contour[next_k].node_type == NodeType::OffCurve;
                if has_off && gs_colinear_lines(contour[prev_k].point, contour[k].point, contour[next_k].point) {
                    next_types[k] = NodeType::Smooth;
                    if contour[prev_k].node_type == NodeType::OffCurve && contour[next_k].node_type != NodeType::OffCurve {
                        contour[prev_k].point = round_grid_pt(crate::core::align_collinear_opposite(contour[k].point, contour[prev_k].point, contour[next_k].point), grid_step_upm);
                    } else if contour[next_k].node_type == NodeType::OffCurve && contour[prev_k].node_type != NodeType::OffCurve {
                        contour[next_k].point = round_grid_pt(crate::core::align_collinear_opposite(contour[k].point, contour[next_k].point, contour[prev_k].point), grid_step_upm);
                    }
                } else {
                    next_types[k] = NodeType::Corner;
                }
            }
            for (k, nt) in next_types.into_iter().enumerate() {
                contour[k].node_type = nt;
            }
        }

        kept_c.push(contour);
        kept_f.push(closed);
    }

    *contours = kept_c;
    *closed_flags = kept_f;
    *contours != before_c || *closed_flags != before_f
}

pub fn align_selected_nodes(
    contours: &mut [Vec<Node>],
    closed_flags: &[bool],
    selected_nodes: &[NodeId],
    anchor: TransformGridAnchor,
    grid_step_upm: f64,
) -> bool {
    if selected_nodes.len() < 2 { return false; }
    let before = contours.to_vec();

    // Branch A: 2 nodes (1 OnCurve + 1 adjacent OffCurve on the same contour)
    if selected_nodes.len() == 2 {
        for (c_idx, contour) in contours.iter_mut().enumerate() {
            let (closed, len) = (closed_flags.get(c_idx).copied().unwrap_or(true), contour.len());
            let sel_pos: Vec<usize> = contour.iter().enumerate().filter(|(_, n)| selected_nodes.contains(&n.id)).map(|(i, _)| i).collect();
            if sel_pos.len() == 2 {
                let (i0, i1) = (sel_pos[0], sel_pos[1]);
                let adj = i0.abs_diff(i1) == 1 || (closed && len >= 2 && ((i0 == 0 && i1 + 1 == len) || (i1 == 0 && i0 + 1 == len)));
                let (on_opt, off_opt) = match (contour[i0].node_type, contour[i1].node_type) {
                    (NodeType::OffCurve, NodeType::Corner | NodeType::Smooth) => (Some(i1), Some(i0)),
                    (NodeType::Corner | NodeType::Smooth, NodeType::OffCurve) => (Some(i0), Some(i1)),
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
    let Some(bounds) = compute_selection_bounds(contours, selected_nodes, 0.0, 0.0) else { return false; };
    let aligned_pt = round_grid_pt(bounds.aligned_point(anchor.index()), grid_step_upm);
    let align_y = bounds.width() > bounds.height();
    for (c_idx, contour) in contours.iter_mut().enumerate() {
        let (closed, shadow, len) = (closed_flags.get(c_idx).copied().unwrap_or(true), contour.clone(), contour.len());
        for node in contour.iter_mut() {
            if selected_nodes.contains(&node.id) {
                if align_y { node.point.y = aligned_pt.y; } else { node.point.x = aligned_pt.x; }
            }
        }
        for i in 0..len {
            if contour[i].node_type != NodeType::OffCurve || selected_nodes.contains(&contour[i].id) { continue; }
            let prev = if i > 0 { Some(i - 1) } else if closed { Some(len - 1) } else { None };
            let next = if i + 1 < len { Some(i + 1) } else if closed { Some(0) } else { None };
            let anchor_idx = prev.filter(|&p| contour[p].node_type != NodeType::OffCurve).or_else(|| next.filter(|&n| contour[n].node_type != NodeType::OffCurve));
            if let Some(a_idx) = anchor_idx {
                if selected_nodes.contains(&contour[a_idx].id) {
                    if align_y { contour[i].point.y.0 += contour[a_idx].point.y.0 - shadow[a_idx].point.y.0; }
                    else { contour[i].point.x.0 += contour[a_idx].point.x.0 - shadow[a_idx].point.x.0; }
                }
                if contour[a_idx].node_type == NodeType::Smooth {
                    let opp = if prev == Some(a_idx) { if a_idx > 0 { Some(a_idx - 1) } else if closed { Some(len - 1) } else { None } }
                    else if a_idx + 1 < len { Some(a_idx + 1) } else if closed { Some(0) } else { None };
                    if let Some(b_idx) = opp.filter(|&b| selected_nodes.contains(&contour[b].id)) {
                        contour[i].point = crate::core::align_collinear_opposite(contour[a_idx].point, contour[i].point, contour[b_idx].point);
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
    let on_c = contour.iter().filter(|n| n.node_type != NodeType::OffCurve).count();
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
        let paired = endpoints.as_chunks::<2>().0.iter().all(|&[a, b]| a.0 == b.0 && a.1 != b.1);
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
