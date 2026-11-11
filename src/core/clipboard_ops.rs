//! `SPEC-003 §2.5` Canvas Outline Clipboard (`Cmd/Ctrl+C`, `Cmd/Ctrl+X`, `Cmd/Ctrl+V`,
//! `selectedObjects`, `pasteDict:` collision offset, and `pasteInsertPath:` open-contour splice).

use crate::core::bezier_ops::{Node, NodeId, NodeType};
use crate::core::coordinates::FontPoint;
use crate::tokens::{CLIPBOARD_COLLISION_OFFSET_UPM, PATH_COORD_EPS_UPM};

#[derive(Debug, Clone, PartialEq)]
pub struct ClipboardContour {
    pub nodes: Vec<Node>,
    pub closed: bool,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct CanvasClipboard {
    pub contours: Vec<ClipboardContour>,
}

#[must_use]
pub fn extract_clipboard_contours(
    contours: &[Vec<Node>],
    closed_flags: &[bool],
    selected_nodes: &[NodeId],
) -> Vec<ClipboardContour> {
    if selected_nodes.is_empty() {
        return Vec::new();
    }
    let mut out = Vec::new();
    for (c_idx, contour) in contours.iter().enumerate() {
        let closed = closed_flags.get(c_idx).copied().unwrap_or(true);
        let on_curve_ids: Vec<NodeId> = contour
            .iter()
            .filter(|n| n.node_type != NodeType::OffCurve)
            .map(|n| n.id)
            .collect();
        let n_on = on_curve_ids.len();
        let n_sel_on = on_curve_ids
            .iter()
            .filter(|id| selected_nodes.contains(id))
            .count();
        if n_sel_on == 0 {
            continue;
        }
        if n_sel_on == n_on {
            out.push(ClipboardContour {
                nodes: contour.clone(),
                closed,
            });
            continue;
        }
        let mut working = contour.clone();
        if closed {
            if let Some(unsel_pos) = working
                .iter()
                .position(|n| n.node_type != NodeType::OffCurve && !selected_nodes.contains(&n.id))
            {
                working.rotate_left(unsel_pos);
            }
        }
        let on_indices: Vec<usize> = working
            .iter()
            .enumerate()
            .filter(|(_, n)| n.node_type != NodeType::OffCurve)
            .map(|(idx, _)| idx)
            .collect();
        let mut sub_path: Vec<Node> = Vec::new();
        for (k, &on_idx) in on_indices.iter().enumerate() {
            let node = &working[on_idx];
            if selected_nodes.contains(&node.id) {
                if sub_path.is_empty() {
                    let mut start_node = node.clone();
                    start_node.node_type = NodeType::Corner;
                    sub_path.push(start_node);
                } else {
                    sub_path.push(node.clone());
                }
                let next_on_opt = if k + 1 < on_indices.len() {
                    Some(on_indices[k + 1])
                } else if closed && !on_indices.is_empty() {
                    Some(on_indices[0])
                } else {
                    None
                };
                if let Some(next_on_idx) = next_on_opt {
                    if selected_nodes.contains(&working[next_on_idx].id) {
                        let mut step = (on_idx + 1) % working.len();
                        while step != next_on_idx {
                            if working[step].node_type == NodeType::OffCurve {
                                sub_path.push(working[step].clone());
                            }
                            step = (step + 1) % working.len();
                        }
                    } else if !sub_path.is_empty() {
                        finalize_open_subpath(&mut sub_path, &mut out);
                    }
                } else if !sub_path.is_empty() {
                    finalize_open_subpath(&mut sub_path, &mut out);
                }
            } else if !sub_path.is_empty() {
                finalize_open_subpath(&mut sub_path, &mut out);
            }
        }
        if !sub_path.is_empty() {
            finalize_open_subpath(&mut sub_path, &mut out);
        }
    }
    out
}

fn finalize_open_subpath(sub_path: &mut Vec<Node>, out: &mut Vec<ClipboardContour>) {
    while sub_path.last().is_some_and(|n| n.node_type == NodeType::OffCurve) {
        sub_path.pop();
    }
    if let Some(first) = sub_path.first_mut() {
        first.node_type = NodeType::Corner;
    }
    if let Some(last) = sub_path.last_mut() {
        last.node_type = NodeType::Corner;
    }
    if !sub_path.is_empty() {
        out.push(ClipboardContour {
            nodes: std::mem::take(sub_path),
            closed: false,
        });
    }
}

fn contour_matches_coords(a: &[Node], b: &[Node]) -> bool {
    a.len() == b.len()
        && a.iter().zip(b.iter()).all(|(na, nb)| {
            (na.point.x.0 - nb.point.x.0).abs() < PATH_COORD_EPS_UPM
                && (na.point.y.0 - nb.point.y.0).abs() < PATH_COORD_EPS_UPM
        })
}

pub fn paste_clipboard_into_contours(
    contours: &mut Vec<Vec<Node>>,
    closed_flags: &mut Vec<bool>,
    selected_nodes: &mut Vec<NodeId>,
    clipboard: &mut [ClipboardContour],
    next_node_id: &mut u64,
) -> bool {
    if clipboard.is_empty() {
        return false;
    }
    // Mode B (`pasteInsertPath:`): 1 open contour on clipboard and >= 2 selected OnCurve nodes on 1 contour
    if clipboard.len() == 1 && !clipboard[0].closed && clipboard[0].nodes.len() >= 2 {
        let mut target_contours = Vec::new();
        for (c_idx, contour) in contours.iter().enumerate() {
            let sel_on: Vec<usize> = contour
                .iter()
                .enumerate()
                .filter(|(_, n)| n.node_type != NodeType::OffCurve && selected_nodes.contains(&n.id))
                .map(|(i, _)| i)
                .collect();
            if !sel_on.is_empty() {
                target_contours.push((c_idx, sel_on));
            }
        }
        if target_contours.len() == 1 && target_contours[0].1.len() >= 2 {
            let c_idx = target_contours[0].0;
            let closed = closed_flags.get(c_idx).copied().unwrap_or(true);
            let mut working = contours[c_idx].clone();
            if closed {
                if let Some(unsel_pos) = working
                    .iter()
                    .position(|n| n.node_type != NodeType::OffCurve && !selected_nodes.contains(&n.id))
                {
                    working.rotate_left(unsel_pos);
                }
            }
            let on_nodes: Vec<(usize, bool)> = working
                .iter()
                .enumerate()
                .filter(|(_, n)| n.node_type != NodeType::OffCurve)
                .map(|(i, n)| (i, selected_nodes.contains(&n.id)))
                .collect();
            let first_on_pos = on_nodes.iter().position(|(_, s)| *s).unwrap();
            let last_on_pos = on_nodes.iter().rposition(|(_, s)| *s).unwrap();
            let is_contiguous = on_nodes[first_on_pos..=last_on_pos].iter().all(|(_, s)| *s);
            if is_contiguous {
                let first_idx = on_nodes[first_on_pos].0;
                let last_idx = on_nodes[last_on_pos].0;
                let start_pt = working[first_idx].point;
                let end_pt = working[last_idx].point;
                let mut clip_nodes = clipboard[0].nodes.clone();
                let c_first = clip_nodes.first().unwrap().point;
                let c_last = clip_nodes.last().unwrap().point;
                let d_fwd = start_pt.distance_to(c_first) + end_pt.distance_to(c_last);
                let d_rev = start_pt.distance_to(c_last) + end_pt.distance_to(c_first);
                if d_rev < d_fwd {
                    clip_nodes.reverse();
                }
                let mut new_ids = Vec::with_capacity(clip_nodes.len());
                for n in &mut clip_nodes {
                    n.id = NodeId(*next_node_id);
                    *next_node_id += 1;
                    new_ids.push(n.id);
                }
                working.splice(first_idx..=last_idx, clip_nodes);
                contours[c_idx] = working;
                *selected_nodes = new_ids;
                return true;
            }
        }
    }

    // Mode A (`pasteDict:`): Standard contour paste with (+10, +10) collision offset if identical contour exists
    let has_collision = clipboard.iter().any(|cc| {
        contours
            .iter()
            .any(|existing| contour_matches_coords(existing, &cc.nodes))
    });
    if has_collision {
        for cc in clipboard.iter_mut() {
            for n in &mut cc.nodes {
                n.point = FontPoint::new(
                    n.point.x.0 + CLIPBOARD_COLLISION_OFFSET_UPM,
                    n.point.y.0 + CLIPBOARD_COLLISION_OFFSET_UPM,
                );
            }
        }
    }
    let mut newly_pasted = Vec::new();
    for cc in clipboard.iter() {
        let mut cloned = Vec::with_capacity(cc.nodes.len());
        for n in &cc.nodes {
            let id = NodeId(*next_node_id);
            *next_node_id += 1;
            newly_pasted.push(id);
            cloned.push(Node::new(id, n.point.x.0, n.point.y.0, n.node_type));
        }
        contours.push(cloned);
        closed_flags.push(cc.closed);
    }
    *selected_nodes = newly_pasted;
    true
}

pub fn make_node_first_v020(contour: &mut [Node], closed: bool, selected: &[NodeId]) -> bool {
    if contour.len() < 2 {
        return false;
    }
    let Some(pos) = contour
        .iter()
        .position(|n| selected.contains(&n.id) && n.node_type != NodeType::OffCurve)
    else {
        return false;
    };
    if closed {
        if pos > 0 {
            contour.rotate_left(pos);
            return true;
        }
        false
    } else if pos + 1 == contour.len() {
        contour.reverse();
        if let Some(first) = contour.first_mut() {
            if first.node_type == NodeType::Smooth {
                first.node_type = NodeType::Corner;
            }
        }
        true
    } else {
        false
    }
}

pub fn flatten_contour_points(
    contour: &[Node],
    closed: bool,
    italic_angle_deg: f64,
    half_x_height: f64,
) -> Vec<FontPoint> {
    if contour.is_empty() {
        return Vec::new();
    }
    let mut path = kurbo::BezPath::new();
    let unslanted = |n: &Node| {
        let p = crate::core::transform_ops::unslant_point(n.point, italic_angle_deg, half_x_height);
        kurbo::Point::new(p.x.0, p.y.0)
    };
    path.move_to(unslanted(&contour[0]));
    for seg in crate::core::curve_manipulation::extract_segments(contour, closed) {
        match seg {
            crate::core::curve_manipulation::ContourSegment::Line { end_idx, .. } => {
                path.line_to(unslanted(&contour[end_idx]));
            }
            crate::core::curve_manipulation::ContourSegment::Cubic { h1_idx, h2_idx, end_idx, .. } => {
                path.curve_to(
                    unslanted(&contour[h1_idx]),
                    unslanted(&contour[h2_idx]),
                    unslanted(&contour[end_idx]),
                );
            }
        }
    }
    if closed {
        path.close_path();
    }
    let mut pts = Vec::new();
    kurbo::flatten(path.iter(), crate::tokens::CURVE_FLATTEN_TOLERANCE, |el| match el {
        kurbo::PathEl::MoveTo(p) | kurbo::PathEl::LineTo(p) => pts.push(FontPoint::new(p.x, p.y)),
        _ => {}
    });
    pts
}
