//! Attached BCP handle translation, Smooth $G^1$ collinearity enforcement, and Line-to-Cubic conversion (`SPEC-002 §2.1`).

use crate::core::bezier_ops::{align_collinear_opposite, Node, NodeId, NodeType};
use crate::core::coordinates::FontPoint;
use crate::core::curve_manipulation::ContourSegment;
use crate::tokens::{
    BERNSTEIN_CUBIC_COEFF, BERNSTEIN_SECOND_DERIV_COEFF, COLLINEAR_MIN_LEN_EPSILON,
    CUBIC_ONE_THIRD, CUBIC_TWO_THIRDS, FIT_SAMPLES_PER_SEGMENT, NEWTON_REPARAM_ITERATIONS,
    PARAM_CLAMP_MAX, PARAM_CLAMP_MIN, SOLVER_DET_EPSILON, SPLIT_RATIO_MIN,
};
use kurbo::{CubicBez, ParamCurve, Point, Vec2};

/// Expands `selected` node IDs to include directly attached `OffCurve` BCP handles of any
/// selected `OnCurve` node (unless `include_attached` is `false`, e.g. when `Alt` is held).
/// Guarantees zero duplicate `NodeId` entries so handles are never double-translated (`TC-1246`).
#[must_use]
pub fn expanded_nodes_with_attached_handles(
    contours: &[Vec<Node>],
    closed_flags: &[bool],
    selected: &[NodeId],
    include_attached: bool,
) -> Vec<NodeId> {
    let mut result: Vec<NodeId> = Vec::new();
    for &id in selected {
        if !result.contains(&id) {
            result.push(id);
        }
    }
    if !include_attached {
        return result;
    }

    for (c_idx, contour) in contours.iter().enumerate() {
        let n = contour.len();
        if n < 2 {
            continue;
        }
        let closed = closed_flags.get(c_idx).copied().unwrap_or(true);
        for (i, node) in contour.iter().enumerate() {
            if node.node_type == NodeType::OffCurve || !selected.contains(&node.id) {
                continue;
            }
            // Preceding neighbor (incoming BCP handle)
            if closed || i > 0 {
                let prev_idx = (i + n - 1) % n;
                if contour[prev_idx].node_type == NodeType::OffCurve {
                    let pid = contour[prev_idx].id;
                    if !result.contains(&pid) {
                        result.push(pid);
                    }
                }
            }
            // Succeeding neighbor (outgoing BCP handle)
            if closed || i + 1 < n {
                let next_idx = (i + 1) % n;
                if contour[next_idx].node_type == NodeType::OffCurve {
                    let nid = contour[next_idx].id;
                    if !result.contains(&nid) {
                        result.push(nid);
                    }
                }
            }
        }
    }
    result
}

/// Enforces $G^1$ collinearity on the opposite `OffCurve` handle across a `NodeType::Smooth`
/// anchor when a single `OffCurve` handle is dragged (`TC-1247`), or breaks the `Smooth`
/// connection to `NodeType::Corner` when `alt_held` is `true` (`TC-1248`).
pub fn enforce_smooth_collinearity_on_handle_drag(
    contour: &mut [Node],
    closed: bool,
    moved_handle_id: NodeId,
    alt_held: bool,
) {
    let n = contour.len();
    if n < 3 {
        return;
    }
    let Some(h_idx) = contour.iter().position(|nd| nd.id == moved_handle_id) else {
        return;
    };
    if contour[h_idx].node_type != NodeType::OffCurve {
        return;
    }

    // Determine whether the adjacent OnCurve anchor is immediately before or after h_idx
    let prev_candidate = if closed || h_idx > 0 {
        Some((h_idx + n - 1) % n)
    } else {
        None
    };
    let next_candidate = if closed || h_idx + 1 < n {
        Some((h_idx + 1) % n)
    } else {
        None
    };

    let anchor_and_opp = if let Some(p_idx) =
        prev_candidate.filter(|&i| contour[i].node_type != NodeType::OffCurve)
    {
        let opp = if closed || p_idx > 0 {
            Some((p_idx + n - 1) % n)
        } else {
            None
        };
        Some((p_idx, opp))
    } else if let Some(n_idx) =
        next_candidate.filter(|&i| contour[i].node_type != NodeType::OffCurve)
    {
        let opp = if closed || n_idx + 1 < n {
            Some((n_idx + 1) % n)
        } else {
            None
        };
        Some((n_idx, opp))
    } else {
        None
    };

    let Some((anchor_idx, opp_opt)) = anchor_and_opp else {
        return;
    };

    if contour[anchor_idx].node_type != NodeType::Smooth {
        return;
    }

    if alt_held {
        contour[anchor_idx].node_type = NodeType::Corner;
        return;
    }

    if let Some(opp_idx) = opp_opt {
        if opp_idx != h_idx && contour[opp_idx].node_type == NodeType::OffCurve {
            let new_opp = align_collinear_opposite(
                contour[anchor_idx].point,
                contour[opp_idx].point,
                contour[h_idx].point,
            );
            contour[opp_idx].point = new_opp;
        }
    }
}

/// Converts a straight `ContourSegment::Line` into a `ContourSegment::Cubic` by inserting
/// two `NodeType::OffCurve` BCP handles at $1/3$ and $2/3$ along the line (`TC-1249`).
pub fn convert_line_segment_to_cubic(
    contour: &mut Vec<Node>,
    segment: &ContourSegment,
    next_node_id: &mut u64,
) -> Option<(NodeId, NodeId)> {
    convert_line_segment_to_cubic_with_grid(contour, segment, next_node_id, 0.0)
}

/// Converts a straight `ContourSegment::Line` into a `ContourSegment::Cubic` at $1/3$ and $2/3$
/// (`convertLineToCurve:pathIndex:nodeIndex:theEvent:` @ `CanvasEngine spec_ref` / `spec_ref`),
/// immediately rounding both inserted `OffCurve` handles to `grid_step_upm` when `grid_step_upm > 0.0`.
pub fn convert_line_segment_to_cubic_with_grid(
    contour: &mut Vec<Node>,
    segment: &ContourSegment,
    next_node_id: &mut u64,
    grid_step_upm: f64,
) -> Option<(NodeId, NodeId)> {
    let ContourSegment::Line {
        start_idx,
        end_idx,
        line,
    } = *segment
    else {
        return None;
    };
    let max_existing = contour.iter().map(|n| n.id.0).max().unwrap_or(0);
    if *next_node_id <= max_existing {
        *next_node_id = max_existing + 1;
    }
    let p1 = FontPoint::new(line.eval(CUBIC_ONE_THIRD).x, line.eval(CUBIC_ONE_THIRD).y)
        .round_to_grid_step(grid_step_upm);
    let p2 = FontPoint::new(line.eval(CUBIC_TWO_THIRDS).x, line.eval(CUBIC_TWO_THIRDS).y)
        .round_to_grid_step(grid_step_upm);
    let h1_id = NodeId(*next_node_id);
    *next_node_id += 1;
    let h2_id = NodeId(*next_node_id);
    *next_node_id += 1;
    let h1 = Node::new(h1_id, p1.x.0, p1.y.0, NodeType::OffCurve);
    let h2 = Node::new(h2_id, p2.x.0, p2.y.0, NodeType::OffCurve);
    let insert_pos = if end_idx > start_idx {
        end_idx
    } else {
        contour.len()
    };
    contour.insert(insert_pos, h1);
    contour.insert(insert_pos + 1, h2);
    Some((h1_id, h2_id))
}

/// Cycles selection forward (`reverse == false`) or backward (`reverse == true`) along the contour (`TC-1250`).
#[must_use]
pub fn cycle_selection_tab(
    contours: &[Vec<Node>],
    selected: &[NodeId],
    reverse: bool,
) -> Option<NodeId> {
    let target = *selected.first()?;
    for contour in contours {
        let n = contour.len();
        if n == 0 {
            continue;
        }
        if let Some(idx) = contour.iter().position(|nd| nd.id == target) {
            let next_idx = if reverse {
                (idx + n - 1) % n
            } else {
                (idx + 1) % n
            };
            return Some(contour[next_idx].id);
        }
    }
    None
}

/// Toggles selected `OnCurve` nodes between `NodeType::Corner` and `NodeType::Smooth` (`TC-1250`).
pub fn toggle_selected_on_curve_smooth(
    contours: &mut [Vec<Node>],
    closed_flags: &[bool],
    selected: &[NodeId],
) -> bool {
    let mut changed = false;
    for (c_idx, contour) in contours.iter_mut().enumerate() {
        let n = contour.len();
        let closed = closed_flags.get(c_idx).copied().unwrap_or(true);
        for i in 0..n {
            if !selected.contains(&contour[i].id) || contour[i].node_type == NodeType::OffCurve {
                continue;
            }
            contour[i].node_type = match contour[i].node_type {
                NodeType::Corner => NodeType::Smooth,
                NodeType::Smooth => NodeType::Corner,
                NodeType::OffCurve => NodeType::OffCurve,
            };
            changed = true;
            if contour[i].node_type == NodeType::Smooth && n >= 3 {
                let prev_idx = (i + n - 1) % n;
                let next_idx = (i + 1) % n;
                if (closed || (i > 0 && i + 1 < n))
                    && contour[prev_idx].node_type == NodeType::OffCurve
                    && contour[next_idx].node_type == NodeType::OffCurve
                {
                    let aligned = align_collinear_opposite(
                        contour[i].point,
                        contour[prev_idx].point,
                        contour[next_idx].point,
                    );
                    contour[prev_idx].point = aligned;
                }
            }
        }
    }
    changed
}

fn seg_endpoints_and_tangents(seg: &ContourSegment) -> (Point, Point, Vec2, Vec2) {
    match *seg {
        ContourSegment::Line { line, .. } => {
            let v = line.p1 - line.p0;
            (line.p0, line.p1, v, -v)
        }
        ContourSegment::Cubic { cubic, .. } => {
            let mut v1 = cubic.p1 - cubic.p0;
            if v1.hypot() <= COLLINEAR_MIN_LEN_EPSILON {
                v1 = cubic.p3 - cubic.p0;
            }
            let mut v2 = cubic.p2 - cubic.p3;
            if v2.hypot() <= COLLINEAR_MIN_LEN_EPSILON {
                v2 = cubic.p0 - cubic.p3;
            }
            (cubic.p0, cubic.p3, v1, v2)
        }
    }
}

/// Fits a single cubic Bézier `(h1, h2)` along the endpoint tangents of a chain of segments
/// using inverse De Casteljau handle-ratio initialization and Schneider's Newton-Raphson
/// least-squares solver (`SPEC-002 §3.1`, Reference `removeNodeCheckKeepShape:` parity).
#[must_use]
pub fn fit_cubic_to_segment_chain(segments: &[ContourSegment]) -> (FontPoint, FontPoint) {
    let (p0, _, v1_raw, _) = seg_endpoints_and_tangents(&segments[0]);
    let (_, p3, _, v2_raw) = seg_endpoints_and_tangents(&segments[segments.len() - 1]);
    let chord = (p3 - p0).hypot().max(1.0);
    let t1 = if v1_raw.hypot() > COLLINEAR_MIN_LEN_EPSILON {
        v1_raw / v1_raw.hypot()
    } else {
        (p3 - p0) / chord
    };
    let t2 = if v2_raw.hypot() > COLLINEAR_MIN_LEN_EPSILON {
        v2_raw / v2_raw.hypot()
    } else {
        (p0 - p3) / chord
    };

    let mut weights = vec![1.0_f64; segments.len()];
    for idx in 0..segments.len().saturating_sub(1) {
        let d_in = match segments[idx] {
            ContourSegment::Cubic { cubic, .. } => (cubic.p3 - cubic.p2).hypot(),
            ContourSegment::Line { line, .. } => (line.p1 - line.p0).hypot() * CUBIC_ONE_THIRD,
        };
        let d_out = match segments[idx + 1] {
            ContourSegment::Cubic { cubic, .. } => (cubic.p1 - cubic.p0).hypot(),
            ContourSegment::Line { line, .. } => (line.p1 - line.p0).hypot() * CUBIC_ONE_THIRD,
        };
        let ratio = if d_in > COLLINEAR_MIN_LEN_EPSILON && d_out > COLLINEAR_MIN_LEN_EPSILON {
            (d_out / d_in).clamp(SPLIT_RATIO_MIN, 1.0 / SPLIT_RATIO_MIN)
        } else {
            1.0
        };
        weights[idx + 1] = weights[idx] * ratio;
    }

    let weight_sum: f64 = weights.iter().sum::<f64>().max(COLLINEAR_MIN_LEN_EPSILON);
    let mut samples: Vec<(Point, f64)> = Vec::new();
    let mut u_acc = 0.0;
    for (seg, &w) in segments.iter().zip(&weights) {
        let span = w / weight_sum;
        for k in 0..FIT_SAMPLES_PER_SEGMENT {
            if u_acc == 0.0 && k == 0 {
                continue;
            }
            let local_t = (k as f64) / (FIT_SAMPLES_PER_SEGMENT as f64);
            let u = (u_acc + span * local_t).clamp(PARAM_CLAMP_MIN, PARAM_CLAMP_MAX);
            let pt = match *seg {
                ContourSegment::Line { line, .. } => line.eval(local_t),
                ContourSegment::Cubic { cubic, .. } => cubic.eval(local_t),
            };
            samples.push((pt, u));
        }
        if (u_acc..=u_acc + span).contains(&0.5) {
            let lt = ((0.5 - u_acc) / span).clamp(0.0, 1.0);
            let pt = match *seg {
                ContourSegment::Line { line, .. } => line.eval(lt),
                ContourSegment::Cubic { cubic, .. } => cubic.eval(lt),
            };
            samples.push((pt, 0.5));
        }
        u_acc += span;
    }

    if segments.len() >= 2 && t1.cross(t2).abs() <= COLLINEAR_MIN_LEN_EPSILON && t1.dot(t2) > 0.0 {
        let mid_chord = p0 + (p3 - p0) * 0.5;
        let arch_h = samples
            .iter()
            .map(|&(pt, _)| (pt - mid_chord).dot(t1))
            .fold(0.0_f64, f64::max);
        if arch_h > COLLINEAR_MIN_LEN_EPSILON {
            let alpha_arch = 2.0 * CUBIC_TWO_THIRDS * arch_h;
            let h1_u = p0 + t1 * alpha_arch;
            let h2_u = p3 + t2 * alpha_arch;
            return (
                FontPoint::new(h1_u.x, h1_u.y),
                FontPoint::new(h2_u.x, h2_u.y),
            );
        }
    }

    let mut alpha1 = chord * CUBIC_ONE_THIRD;
    let mut alpha2 = chord * CUBIC_ONE_THIRD;

    for _ in 0..=NEWTON_REPARAM_ITERATIONS {
        let mut c11 = 0.0;
        let mut c12 = 0.0;
        let mut c22 = 0.0;
        let mut x1 = 0.0;
        let mut x2 = 0.0;

        for &(d_pt, u) in &samples {
            let om = 1.0 - u;
            let b0 = om * om * om;
            let b1 = BERNSTEIN_CUBIC_COEFF * om * om * u;
            let b2 = BERNSTEIN_CUBIC_COEFF * om * u * u;
            let b3 = u * u * u;
            let a1 = t1 * b1;
            let a2 = t2 * b2;
            c11 += a1.dot(a1);
            c12 += a1.dot(a2);
            c22 += a2.dot(a2);

            let base_x = (b0 + b1) * p0.x + (b2 + b3) * p3.x;
            let base_y = (b0 + b1) * p0.y + (b2 + b3) * p3.y;
            let rhs = Vec2::new(d_pt.x - base_x, d_pt.y - base_y);
            x1 += a1.dot(rhs);
            x2 += a2.dot(rhs);
        }

        let det = c11 * c22 - c12 * c12;
        if det.abs() > SOLVER_DET_EPSILON {
            let a1_sol = (c22 * x1 - c12 * x2) / det;
            let a2_sol = (c11 * x2 - c12 * x1) / det;
            if a1_sol > COLLINEAR_MIN_LEN_EPSILON && a2_sol > COLLINEAR_MIN_LEN_EPSILON {
                alpha1 = a1_sol;
                alpha2 = a2_sol;
            }
        }

        let h1_pt = p0 + t1 * alpha1;
        let h2_pt = p3 + t2 * alpha2;
        let cand = CubicBez::new(p0, h1_pt, h2_pt, p3);
        for (d_pt, u) in &mut samples {
            let om = 1.0 - *u;
            let q = cand.eval(*u) - *d_pt;
            let d1 = (h1_pt - p0) * (BERNSTEIN_CUBIC_COEFF * om * om)
                + (h2_pt - h1_pt) * (BERNSTEIN_SECOND_DERIV_COEFF * om * *u)
                + (p3 - h2_pt) * (BERNSTEIN_CUBIC_COEFF * *u * *u);
            let d2 = ((h2_pt - h1_pt) - (h1_pt - p0)) * (BERNSTEIN_SECOND_DERIV_COEFF * om)
                + ((p3 - h2_pt) - (h2_pt - h1_pt)) * (BERNSTEIN_SECOND_DERIV_COEFF * *u);
            let denom = d1.dot(d1) + q.dot(d2);
            if denom.abs() > SOLVER_DET_EPSILON {
                *u = (*u - q.dot(d1) / denom).clamp(PARAM_CLAMP_MIN, PARAM_CLAMP_MAX);
            }
        }
    }

    let h1_final = p0 + t1 * alpha1;
    let h2_final = p3 + t2 * alpha2;
    (
        FontPoint::new(h1_final.x, h1_final.y),
        FontPoint::new(h2_final.x, h2_final.y),
    )
}
