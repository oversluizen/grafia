//! Curve manipulation, De Casteljau subdivision, segment hit testing, and topological healing.
//!
//! Normative STD-ARCH: Zero magic numbers, strict typed models, curve preservation.

use crate::core::bezier_ops::{Node, NodeId, NodeType};
use crate::core::coordinates::FontPoint;
use crate::core::handle_constraints::fit_cubic_to_segment_chain;
use crate::tokens::{CURVE_SOLVE_ACCURACY, PARAM_CLAMP_MAX, PARAM_CLAMP_MIN};
use kurbo::{CubicBez, Line, ParamCurve, ParamCurveNearest, Point};

/// A geometric segment extracted from a glyph contour.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ContourSegment {
    Line {
        start_idx: usize,
        end_idx: usize,
        line: Line,
    },
    Cubic {
        start_idx: usize,
        h1_idx: usize,
        h2_idx: usize,
        end_idx: usize,
        cubic: CubicBez,
    },
}

/// Extracts all geometric segments from a contour's node list.
#[must_use]
pub fn extract_segments(nodes: &[Node], closed: bool) -> Vec<ContourSegment> {
    let mut segments = Vec::new();
    if nodes.len() < 2 {
        return segments;
    }

    let on_curve_indices: Vec<usize> = nodes
        .iter()
        .enumerate()
        .filter(|(_, n)| n.node_type != NodeType::OffCurve)
        .map(|(i, _)| i)
        .collect();

    if on_curve_indices.is_empty() {
        return segments;
    }

    let num_on_curve = on_curve_indices.len();
    let num_segments = if closed {
        num_on_curve
    } else {
        num_on_curve.saturating_sub(1)
    };

    for s in 0..num_segments {
        let start_idx = on_curve_indices[s];
        let end_idx = on_curve_indices[(s + 1) % num_on_curve];

        let p_start = Point::new(nodes[start_idx].point.x.0, nodes[start_idx].point.y.0);
        let p_end = Point::new(nodes[end_idx].point.x.0, nodes[end_idx].point.y.0);

        let mut off_indices = Vec::new();
        let mut curr = (start_idx + 1) % nodes.len();
        while curr != end_idx {
            if nodes[curr].node_type == NodeType::OffCurve {
                off_indices.push(curr);
            }
            curr = (curr + 1) % nodes.len();
        }

        if off_indices.len() == 2 {
            let h1_idx = off_indices[0];
            let h2_idx = off_indices[1];
            let p_h1 = Point::new(nodes[h1_idx].point.x.0, nodes[h1_idx].point.y.0);
            let p_h2 = Point::new(nodes[h2_idx].point.x.0, nodes[h2_idx].point.y.0);
            segments.push(ContourSegment::Cubic {
                start_idx,
                h1_idx,
                h2_idx,
                end_idx,
                cubic: CubicBez::new(p_start, p_h1, p_h2, p_end),
            });
        } else {
            segments.push(ContourSegment::Line {
                start_idx,
                end_idx,
                line: Line::new(p_start, p_end),
            });
        }
    }

    segments
}

/// Hit test result against a contour segment.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SegmentHit {
    pub contour_idx: usize,
    pub segment: ContourSegment,
    pub t: f64,
    pub point: FontPoint,
    pub distance_font: f64,
}

/// Hit tests a point against segments of all contours within `tolerance_font`.
#[must_use]
pub fn hit_test_segments(
    contours: &[Vec<Node>],
    closed_flags: &[bool],
    target: FontPoint,
    tolerance_font: f64,
) -> Option<SegmentHit> {
    let target_pt = Point::new(target.x.0, target.y.0);
    let mut best_hit: Option<SegmentHit> = None;
    let mut min_dist = tolerance_font;

    for (c_idx, contour) in contours.iter().enumerate() {
        let is_closed = closed_flags.get(c_idx).copied().unwrap_or(true);
        let segments = extract_segments(contour, is_closed);
        for seg in segments {
            let (dist, t, pt) = match seg {
                ContourSegment::Line { line, .. } => {
                    let solve = line.nearest(target_pt, CURVE_SOLVE_ACCURACY);
                    let d = solve.distance_sq.sqrt();
                    let p = line.eval(solve.t);
                    (d, solve.t, FontPoint::new(p.x, p.y))
                }
                ContourSegment::Cubic { cubic, .. } => {
                    let solve = cubic.nearest(target_pt, CURVE_SOLVE_ACCURACY);
                    let d = solve.distance_sq.sqrt();
                    let p = cubic.eval(solve.t);
                    (d, solve.t, FontPoint::new(p.x, p.y))
                }
            };

            if dist <= min_dist {
                min_dist = dist;
                best_hit = Some(SegmentHit {
                    contour_idx: c_idx,
                    segment: seg,
                    t,
                    point: pt,
                    distance_font: dist,
                });
            }
        }
    }

    best_hit
}

/// Subdivides a segment using `kurbo::ParamCurveSubdivide` with 100% curve preservation (TC-1238).
pub fn split_contour_segment(
    contour: &mut Vec<Node>,
    segment: ContourSegment,
    t: f64,
    next_node_id: &mut u64,
) -> NodeId {
    let t_clamped = t.clamp(PARAM_CLAMP_MIN, PARAM_CLAMP_MAX);
    match segment {
        ContourSegment::Line {
            start_idx,
            end_idx,
            line,
        } => {
            let split_pt = line.eval(t_clamped);
            let new_id = NodeId(*next_node_id);
            *next_node_id += 1;
            let new_node = Node::new(new_id, split_pt.x, split_pt.y, NodeType::Corner);

            let insert_pos = if end_idx > start_idx {
                end_idx
            } else {
                contour.len()
            };
            contour.insert(insert_pos, new_node);
            new_id
        }
        ContourSegment::Cubic {
            h1_idx,
            h2_idx,
            cubic,
            ..
        } => {
            let c1 = cubic.subsegment(0.0..t_clamped);
            let c2 = cubic.subsegment(t_clamped..1.0);

            contour[h1_idx].point = FontPoint::new(c1.p1.x, c1.p1.y);
            contour[h2_idx].point = FontPoint::new(c2.p2.x, c2.p2.y);

            let in_handle_id = NodeId(*next_node_id);
            *next_node_id += 1;
            let in_handle = Node::new(in_handle_id, c1.p2.x, c1.p2.y, NodeType::OffCurve);

            let split_id = NodeId(*next_node_id);
            *next_node_id += 1;
            let split_node = Node::new(split_id, c1.p3.x, c1.p3.y, NodeType::Smooth);

            let out_handle_id = NodeId(*next_node_id);
            *next_node_id += 1;
            let out_handle = Node::new(out_handle_id, c2.p1.x, c2.p1.y, NodeType::OffCurve);

            let insert_pos = h1_idx + 1;
            contour.insert(insert_pos, in_handle);
            contour.insert(insert_pos + 1, split_node);
            contour.insert(insert_pos + 2, out_handle);

            split_id
        }
    }
}

fn sanitize_contour_handles(contour: &mut Vec<Node>, closed: bool) {
    if !closed {
        while contour
            .first()
            .is_some_and(|nd| nd.node_type == NodeType::OffCurve)
        {
            contour.remove(0);
        }
        while contour
            .last()
            .is_some_and(|nd| nd.node_type == NodeType::OffCurve)
        {
            contour.pop();
        }
    }
    if contour
        .iter()
        .filter(|nd| nd.node_type != NodeType::OffCurve)
        .count()
        <= 1
    {
        contour.retain(|nd| nd.node_type != NodeType::OffCurve);
    }
}

/// Deletes selected nodes with context-aware curve healing (SPEC-002 §3.1, DEF-GRAFIA-003, TC-1244).
pub fn heal_delete_nodes(
    contour: &mut Vec<Node>,
    closed: bool,
    to_delete: &[NodeId],
    next_node_id: &mut u64,
) {
    if contour.len() <= 2 {
        contour.retain(|n| !to_delete.contains(&n.id));
        sanitize_contour_handles(contour, true);
        return;
    }

    let max_existing = contour.iter().map(|n| n.id.0).max().unwrap_or(0);
    if *next_node_id <= max_existing {
        *next_node_id = max_existing + 1;
    }

    for &id in to_delete {
        if let Some(del_idx) = contour.iter().position(|n| n.id == id) {
            if contour[del_idx].node_type == NodeType::OffCurve {
                continue;
            }

            let n = contour.len();

            let mut prev_on = None;
            for offset in 1..n {
                if !closed && offset > del_idx {
                    break;
                }
                let idx = (del_idx + n - offset) % n;
                if contour[idx].node_type != NodeType::OffCurve
                    && !to_delete.contains(&contour[idx].id)
                {
                    prev_on = Some(idx);
                    break;
                }
            }

            let mut next_on = None;
            for offset in 1..n {
                if !closed && del_idx + offset >= n {
                    break;
                }
                let idx = (del_idx + offset) % n;
                if contour[idx].node_type != NodeType::OffCurve
                    && !to_delete.contains(&contour[idx].id)
                {
                    next_on = Some(idx);
                    break;
                }
            }

            if let (Some(p_idx), Some(n_idx)) = (prev_on, next_on) {
                if p_idx != n_idx {
                    let mut sub_nodes = vec![contour[p_idx].clone()];
                    let mut indices_to_remove = Vec::new();
                    let mut curr = (p_idx + 1) % n;
                    while curr != n_idx {
                        sub_nodes.push(contour[curr].clone());
                        indices_to_remove.push(curr);
                        curr = (curr + 1) % n;
                    }
                    sub_nodes.push(contour[n_idx].clone());

                    let has_curve_in_chain = sub_nodes[1..sub_nodes.len() - 1]
                        .iter()
                        .any(|nd| nd.node_type == NodeType::OffCurve);

                    // DEF-GRAFIA-003: When all segments in the deleted chain are straight lines, keep a straight line (0 OffCurve dots)
                    if !has_curve_in_chain {
                        indices_to_remove.sort_by(|a, b| b.cmp(a));
                        for r_idx in &indices_to_remove {
                            contour.remove(*r_idx);
                        }
                        continue;
                    }

                    let p_id = contour[p_idx].id;
                    let sub_segments = extract_segments(&sub_nodes, false);
                    let (h1_pt, h2_pt) = fit_cubic_to_segment_chain(&sub_segments);

                    let h1_id = NodeId(*next_node_id);
                    *next_node_id += 1;
                    let h2_id = NodeId(*next_node_id);
                    *next_node_id += 1;

                    let h1 = Node::new(h1_id, h1_pt.x.0, h1_pt.y.0, NodeType::OffCurve);
                    let h2 = Node::new(h2_id, h2_pt.x.0, h2_pt.y.0, NodeType::OffCurve);

                    indices_to_remove.sort_by(|a, b| b.cmp(a));
                    for r_idx in &indices_to_remove {
                        contour.remove(*r_idx);
                    }

                    let updated_p_idx =
                        contour.iter().position(|node| node.id == p_id).unwrap_or(0);
                    contour.insert(updated_p_idx + 1, h1);
                    contour.insert(updated_p_idx + 2, h2);
                    continue;
                }
            }

            contour.retain(|node| node.id != id);
            sanitize_contour_handles(contour, closed);
        }
    }

    for &id in to_delete {
        if contour
            .iter()
            .any(|n| n.id == id && n.node_type == NodeType::OffCurve)
        {
            crate::core::segment_ops::remove_off_curve_segment_handles(contour, closed, id);
        }
    }

    sanitize_contour_handles(contour, closed);
}

/// Deletes selected nodes without healing, cutting a closed contour open (TC-1245).
pub fn cut_open_delete_nodes(contour: &mut Vec<Node>, to_delete: &[NodeId]) {
    cut_open_delete_nodes_in_contour(contour, true, to_delete);
}

/// Deletes selected nodes without healing on a contour with explicit `closed` state (TC-1245).
pub fn cut_open_delete_nodes_in_contour(
    contour: &mut Vec<Node>,
    closed: bool,
    to_delete: &[NodeId],
) {
    if contour.is_empty() {
        return;
    }

    let original_cubics: Vec<(NodeId, NodeId, NodeId, NodeId)> = extract_segments(contour, closed)
        .into_iter()
        .filter_map(|seg| match seg {
            ContourSegment::Cubic {
                start_idx,
                h1_idx,
                h2_idx,
                end_idx,
                ..
            } => Some((
                contour[start_idx].id,
                contour[h1_idx].id,
                contour[h2_idx].id,
                contour[end_idx].id,
            )),
            ContourSegment::Line { .. } => None,
        })
        .collect();

    if closed {
        let cut_pos = contour
            .iter()
            .position(|n| to_delete.contains(&n.id) && n.node_type != NodeType::OffCurve)
            .or_else(|| contour.iter().position(|n| to_delete.contains(&n.id)));

        if let Some(cut_idx) = cut_pos {
            let n = contour.len();
            if let Some(next_idx) = (1..n).map(|off| (cut_idx + off) % n).find(|&idx| {
                contour[idx].node_type != NodeType::OffCurve
                    && !to_delete.contains(&contour[idx].id)
            }) {
                contour.rotate_left(next_idx);
            }
        }
    }

    contour.retain(|n| !to_delete.contains(&n.id));
    sanitize_contour_handles(contour, false);
    if contour.is_empty() {
        return;
    }

    let mut rebuilt = Vec::with_capacity(contour.len());
    let mut i = 0;
    while i < contour.len() {
        let start_node = contour[i].clone();
        rebuilt.push(start_node.clone());
        let mut j = i + 1;
        while j < contour.len() && contour[j].node_type == NodeType::OffCurve {
            j += 1;
        }
        if j < contour.len() && j - (i + 1) == 2 {
            let h1 = &contour[i + 1];
            let h2 = &contour[i + 2];
            let end_node = &contour[j];
            if original_cubics.contains(&(start_node.id, h1.id, h2.id, end_node.id)) {
                rebuilt.push(h1.clone());
                rebuilt.push(h2.clone());
            }
        }
        i = j;
    }
    *contour = rebuilt;
}
