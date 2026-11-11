//! Cubic handle constraint and background-contour projection helpers for `SelectionInfo` (`SPEC-007`).

use crate::core::{
    align_collinear_opposite, enforce_smooth_collinearity_on_handle_drag, extract_segments,
    ContourSegment, FontPoint, GlyphContour, Node, NodeId, NodeType,
};
use crate::tokens::{CURVE_SOLVE_ACCURACY, HANDLE_RADIAL_LEN_EPS_UPM};
use kurbo::{ParamCurve, ParamCurveNearest, Point as KPoint};

fn adjacent_indices(len: usize, idx: usize, closed: bool) -> (Option<usize>, Option<usize>) {
    if len <= 1 {
        return (None, None);
    }
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

/// Finds the first `OnCurve` node that has at least one directly adjacent `OffCurve` handle.
#[must_use]
pub fn find_first_oncurve_with_adjacent_offcurve(
    contours: &[Vec<Node>],
    closed_flags: &[bool],
) -> Option<NodeId> {
    for (c_idx, contour) in contours.iter().enumerate() {
        let closed = closed_flags.get(c_idx).copied().unwrap_or(true);
        for (n_idx, node) in contour
            .iter()
            .enumerate()
            .filter(|(_, n)| n.node_type != NodeType::OffCurve)
        {
            let (prev, next) = adjacent_indices(contour.len(), n_idx, closed);
            if prev.is_some_and(|p| contour[p].node_type == NodeType::OffCurve)
                || next.is_some_and(|nx| contour[nx].node_type == NodeType::OffCurve)
            {
                return Some(node.id);
            }
        }
    }
    None
}

/// Returns the positions of all `OffCurve` handles directly adjacent to `oncurve_id`.
#[must_use]
pub fn collect_adjacent_offcurve_positions(
    contours: &[Vec<Node>],
    closed_flags: &[bool],
    oncurve_id: NodeId,
) -> Vec<FontPoint> {
    let mut out = Vec::new();
    for (c_idx, contour) in contours.iter().enumerate() {
        let closed = closed_flags.get(c_idx).copied().unwrap_or(true);
        if let Some(n_idx) = contour.iter().position(|n| n.id == oncurve_id) {
            let (prev, next) = adjacent_indices(contour.len(), n_idx, closed);
            if let Some(p) = prev.filter(|&p| contour[p].node_type == NodeType::OffCurve) {
                out.push(contour[p].point);
            }
            if let Some(nx) =
                next.filter(|&nx| Some(nx) != prev && contour[nx].node_type == NodeType::OffCurve)
            {
                out.push(contour[nx].point);
            }
            break;
        }
    }
    out
}

/// Finds the first `OffCurve` node across all contours.
#[must_use]
pub fn find_first_offcurve_node_id(contours: &[Vec<Node>]) -> Option<NodeId> {
    contours
        .iter()
        .flat_map(|c| c.iter())
        .find(|n| n.node_type == NodeType::OffCurve)
        .map(|n| n.id)
}

/// Resolves `(offcurve_node, tethered_oncurve_node)` for `offcurve_id`.
#[must_use]
pub fn resolve_tethered_oncurve_for_offcurve(
    contours: &[Vec<Node>],
    closed_flags: &[bool],
    offcurve_id: NodeId,
) -> Option<(Node, Node)> {
    for (c_idx, contour) in contours.iter().enumerate() {
        let closed = closed_flags.get(c_idx).copied().unwrap_or(true);
        if let Some(n_idx) = contour.iter().position(|n| n.id == offcurve_id) {
            let off_node = contour[n_idx].clone();
            let (prev, next) = adjacent_indices(contour.len(), n_idx, closed);
            if let Some(p) = prev.filter(|&p| contour[p].node_type != NodeType::OffCurve) {
                return Some((off_node, contour[p].clone()));
            }
            if let Some(nx) = next.filter(|&nx| contour[nx].node_type != NodeType::OffCurve) {
                return Some((off_node, contour[nx].clone()));
            }
            if let Some(any_on) = contour.iter().find(|n| n.node_type != NodeType::OffCurve) {
                return Some((off_node, any_on.clone()));
            }
            return Some((off_node.clone(), off_node));
        }
    }
    None
}

/// Enforces smooth collinearity on neighboring `Smooth` nodes and `n_idx` after moving `contour[n_idx]`.
pub fn enforce_smooth_collinearity_around_oncurve(
    contour: &mut [Node],
    closed: bool,
    n_idx: usize,
) {
    let (prev, next) = adjacent_indices(contour.len(), n_idx, closed);
    if let Some(p) = prev.filter(|&p| contour[p].node_type == NodeType::Smooth) {
        let (pp, _) = adjacent_indices(contour.len(), p, closed);
        if let Some(pp_idx) =
            pp.filter(|&i| i != n_idx && contour[i].node_type == NodeType::OffCurve)
        {
            contour[pp_idx].point = align_collinear_opposite(
                contour[p].point,
                contour[pp_idx].point,
                contour[n_idx].point,
            );
        }
    }
    if let Some(nx) =
        next.filter(|&nx| Some(nx) != prev && contour[nx].node_type == NodeType::Smooth)
    {
        let (_, nn) = adjacent_indices(contour.len(), nx, closed);
        if let Some(nn_idx) =
            nn.filter(|&i| i != n_idx && contour[i].node_type == NodeType::OffCurve)
        {
            contour[nn_idx].point = align_collinear_opposite(
                contour[nx].point,
                contour[nn_idx].point,
                contour[n_idx].point,
            );
        }
    }
    if contour[n_idx].node_type == NodeType::Smooth {
        if let (Some(p), Some(nx)) = (prev, next) {
            if p != nx
                && contour[p].node_type != NodeType::OffCurve
                && contour[nx].node_type == NodeType::OffCurve
            {
                contour[nx].point = align_collinear_opposite(
                    contour[n_idx].point,
                    contour[nx].point,
                    contour[p].point,
                );
            } else if p != nx
                && contour[nx].node_type != NodeType::OffCurve
                && contour[p].node_type == NodeType::OffCurve
            {
                contour[p].point = align_collinear_opposite(
                    contour[n_idx].point,
                    contour[p].point,
                    contour[nx].point,
                );
            }
        }
    }
}

/// Translates an `OnCurve` node to `new_pos` along with its directly adjacent `OffCurve` handles.
pub fn translate_oncurve_with_adjacent_handles(
    contours: &mut [Vec<Node>],
    closed_flags: &[bool],
    oncurve_id: NodeId,
    new_pos: FontPoint,
) -> bool {
    for (c_idx, contour) in contours.iter_mut().enumerate() {
        let closed = closed_flags.get(c_idx).copied().unwrap_or(true);
        if let Some(n_idx) = contour.iter().position(|n| n.id == oncurve_id) {
            if contour[n_idx].node_type == NodeType::OffCurve {
                return false;
            }
            let dx = new_pos.x.0 - contour[n_idx].point.x.0;
            let dy = new_pos.y.0 - contour[n_idx].point.y.0;
            contour[n_idx].point = new_pos;
            let (prev, next) = adjacent_indices(contour.len(), n_idx, closed);
            if let Some(p) = prev.filter(|&p| contour[p].node_type == NodeType::OffCurve) {
                contour[p].point.x.0 += dx;
                contour[p].point.y.0 += dy;
            }
            if let Some(nx) =
                next.filter(|&nx| Some(nx) != prev && contour[nx].node_type == NodeType::OffCurve)
            {
                contour[nx].point.x.0 += dx;
                contour[nx].point.y.0 += dy;
            }
            enforce_smooth_collinearity_around_oncurve(contour, closed, n_idx);
            return true;
        }
    }
    false
}

/// Updates an `OffCurve` handle to `new_pos` and enforces smooth collinearity on the opposite handle.
pub fn update_offcurve_with_smooth_constraint(
    contours: &mut [Vec<Node>],
    closed_flags: &[bool],
    offcurve_id: NodeId,
    new_pos: FontPoint,
) -> bool {
    for (c_idx, contour) in contours.iter_mut().enumerate() {
        let closed = closed_flags.get(c_idx).copied().unwrap_or(true);
        if let Some(n_idx) = contour.iter().position(|n| n.id == offcurve_id) {
            if contour[n_idx].node_type != NodeType::OffCurve {
                return false;
            }
            contour[n_idx].point = new_pos;
            enforce_smooth_collinearity_on_handle_drag(contour, closed, offcurve_id, false);
            return true;
        }
    }
    false
}

/// Computes the new `FontPoint` for an `OffCurve` handle scaled radially to `new_length` along its current angle.
#[must_use]
pub fn compute_radially_scaled_handle_pos(
    anchor_pt: FontPoint,
    handle_pt: FontPoint,
    new_length: f64,
) -> FontPoint {
    let dx = handle_pt.x.0 - anchor_pt.x.0;
    let dy = handle_pt.y.0 - anchor_pt.y.0;
    let cur_len = dx.hypot(dy);
    let clamped_len = new_length.max(0.0);
    let (ux, uy) = if cur_len > HANDLE_RADIAL_LEN_EPS_UPM {
        (dx / cur_len, dy / cur_len)
    } else {
        (1.0, 0.0)
    };
    FontPoint::new(
        anchor_pt.x.0 + ux * clamped_len,
        anchor_pt.y.0 + uy * clamped_len,
    )
}

/// Projects `pt` onto the mathematically nearest point across all `Line` and `Cubic` segments in `background_contours`.
#[must_use]
pub fn project_point_to_background_contours(
    pt: FontPoint,
    background_contours: &[GlyphContour],
) -> Option<FontPoint> {
    let query = KPoint::new(pt.x.0, pt.y.0);
    let mut best: Option<(f64, FontPoint)> = None;
    for gc in background_contours {
        if !gc.visible || gc.nodes.len() < 2 {
            continue;
        }
        for seg in extract_segments(&gc.nodes, gc.closed) {
            let (dist_sq, proj_kpt) = match seg {
                ContourSegment::Line { line, .. } => {
                    let nearest = line.nearest(query, CURVE_SOLVE_ACCURACY);
                    (nearest.distance_sq, line.eval(nearest.t))
                }
                ContourSegment::Cubic { cubic, .. } => {
                    let nearest = cubic.nearest(query, CURVE_SOLVE_ACCURACY);
                    (nearest.distance_sq, cubic.eval(nearest.t))
                }
            };
            if best.is_none_or(|(b_sq, _)| dist_sq < b_sq) {
                best = Some((dist_sq, FontPoint::new(proj_kpt.x, proj_kpt.y)));
            }
        }
    }
    best.map(|(_, fp)| fp)
}

/// Projects each `OnCurve` node in `selected_oncurve_ids` onto `background_contours`,
/// translating its directly adjacent `OffCurve` handles once and then enforcing smooth collinearity.
pub fn align_oncurve_nodes_to_background(
    contours: &mut [Vec<Node>],
    closed_flags: &[bool],
    selected_oncurve_ids: &[NodeId],
    background_contours: &[GlyphContour],
) -> usize {
    let mut aligned_count = 0usize;
    let mut moved_indices: Vec<(usize, usize)> = Vec::new();
    let mut moved_handles = std::collections::HashSet::new();
    for &node_id in selected_oncurve_ids {
        for (c_idx, contour) in contours.iter_mut().enumerate() {
            let closed = closed_flags.get(c_idx).copied().unwrap_or(true);
            if let Some(n_idx) = contour.iter().position(|n| n.id == node_id) {
                if contour[n_idx].node_type == NodeType::OffCurve {
                    break;
                }
                let cur_pt = contour[n_idx].point;
                let Some(proj_pt) =
                    project_point_to_background_contours(cur_pt, background_contours)
                else {
                    break;
                };
                let (dx, dy) = (proj_pt.x.0 - cur_pt.x.0, proj_pt.y.0 - cur_pt.y.0);
                contour[n_idx].point = proj_pt;
                let (prev, next) = adjacent_indices(contour.len(), n_idx, closed);
                if let Some(p) = prev.filter(|&p| contour[p].node_type == NodeType::OffCurve) {
                    if moved_handles.insert(contour[p].id) {
                        contour[p].point.x.0 += dx;
                        contour[p].point.y.0 += dy;
                    }
                }
                if let Some(nx) = next
                    .filter(|&nx| Some(nx) != prev && contour[nx].node_type == NodeType::OffCurve)
                {
                    if moved_handles.insert(contour[nx].id) {
                        contour[nx].point.x.0 += dx;
                        contour[nx].point.y.0 += dy;
                    }
                }
                moved_indices.push((c_idx, n_idx));
                aligned_count += 1;
                break;
            }
        }
    }
    for (c_idx, n_idx) in moved_indices {
        let closed = closed_flags.get(c_idx).copied().unwrap_or(true);
        enforce_smooth_collinearity_around_oncurve(&mut contours[c_idx], closed, n_idx);
    }
    aligned_count
}
