//! Segment-level operations: cubic belly drag (`dragCurve`), extremum solver,
//! paired OffCurve handle removal, proportional handle tension, and contour reversal.

use crate::core::bezier_ops::{Node, NodeId, NodeType};
use crate::core::coordinates::FontPoint;
use crate::core::curve_manipulation::{extract_segments, ContourSegment};
use crate::core::handle_constraints::{
    enforce_smooth_collinearity_on_handle_drag, fit_cubic_to_segment_chain,
};
use crate::tokens::{
    BERNSTEIN_CUBIC_COEFF, COLLINEAR_MIN_LEN_EPSILON, CURVE_SOLVE_ACCURACY,
    DRAG_CURVE_MIN_HANDLE_LEN, DRAG_CURVE_T_MAX, DRAG_CURVE_T_MIN, EXTREMUM_SNAP_MAX_DELTA_T,
    EXTREMUM_T_MAX, EXTREMUM_T_MIN, SEGMENT_DRAG_FREE_WEIGHT_FACTOR,
    SEGMENT_DRAG_MAX_HANDLE_LEN_UPM, SOLVER_DET_EPSILON, SPLIT_RATIO_MAX, SPLIT_RATIO_MIN,
    TENSION_SCALE_MAX, TENSION_SCALE_MIN,
};
use kurbo::{CubicBez, ParamCurve, ParamCurveNearest, Point};

/// Removes an `OffCurve` BCP handle AND its partner `OffCurve` handle in the same cubic segment,
/// converting that cubic segment back into a pure straight `Line` and resetting its two adjacent
/// `OnCurve` endpoints from `Smooth` to `Corner` (`SPEC-002 §3.1.5`).
pub fn remove_off_curve_segment_handles(
    contour: &mut Vec<Node>,
    closed: bool,
    target_off_curve_id: NodeId,
) -> bool {
    let segments = extract_segments(contour, closed);
    for seg in segments {
        if let ContourSegment::Cubic { start_idx, h1_idx, h2_idx, end_idx, .. } = seg {
            let h1_id = contour[h1_idx].id;
            let h2_id = contour[h2_idx].id;
            if h1_id == target_off_curve_id || h2_id == target_off_curve_id {
                let start_id = contour[start_idx].id;
                let end_id = contour[end_idx].id;
                for nd in contour.iter_mut() {
                    if (nd.id == start_id || nd.id == end_id) && nd.node_type == NodeType::Smooth {
                        nd.node_type = NodeType::Corner;
                    }
                }
                contour.retain(|nd| nd.id != h1_id && nd.id != h2_id);
                return true;
            }
        }
    }

    let len_before = contour.len();
    contour.retain(|nd| nd.id != target_off_curve_id);
    contour.len() != len_before
}

fn project_onto_infinite_line(target: FontPoint, origin: FontPoint, line_pt: FontPoint) -> FontPoint {
    let vx = line_pt.x.0 - origin.x.0;
    let vy = line_pt.y.0 - origin.y.0;
    let len_sq = vx * vx + vy * vy;
    if len_sq <= SOLVER_DET_EPSILON {
        return target;
    }
    let tx = target.x.0 - origin.x.0;
    let ty = target.y.0 - origin.y.0;
    let scale = (tx * vx + ty * vy) / len_sq;
    FontPoint::new(origin.x.0 + scale * vx, origin.y.0 + scale * vy)
}

fn clamp_handle_length(anchor: FontPoint, handle: FontPoint, max_len: f64) -> FontPoint {
    let dist = anchor.distance_to(handle);
    if dist > max_len && dist > SOLVER_DET_EPSILON {
        let s = max_len / dist;
        FontPoint::new(anchor.x.0 + (handle.x.0 - anchor.x.0) * s, anchor.y.0 + (handle.y.0 - anchor.y.0) * s)
    } else {
        handle
    }
}

/// Deforms an unselected cubic segment (`dragCurve`, `SPEC-002 §3.1.7`):
/// - Free-angle branch uses linear weight factor $2.65(1-t)$ and $2.65t$ (`spec_ref`).
/// - Angle-locked branch projects onto tangent lines and clamps max handle length to `6000.0` UPM (`spec_ref`).
#[allow(clippy::too_many_arguments)]
pub fn apply_cubic_segment_belly_drag(
    contour: &mut [Node],
    closed: bool,
    segment: ContourSegment,
    t: f64,
    initial_h1: FontPoint,
    initial_h2: FontPoint,
    dx: f64,
    dy: f64,
) {
    apply_cubic_segment_belly_drag_with_alt(contour, closed, segment, t, initial_h1, initial_h2, dx, dy, false);
}

/// Same as [`apply_cubic_segment_belly_drag`] with explicit `alt_held` flag.
#[allow(clippy::too_many_arguments)]
pub fn apply_cubic_segment_belly_drag_with_alt(
    contour: &mut [Node],
    closed: bool,
    segment: ContourSegment,
    t: f64,
    initial_h1: FontPoint,
    initial_h2: FontPoint,
    dx: f64,
    dy: f64,
    alt_held: bool,
) {
    let ContourSegment::Cubic { start_idx, h1_idx, h2_idx, end_idx, .. } = segment else { return; };
    if h1_idx >= contour.len() || h2_idx >= contour.len() {
        return;
    }

    let u = t.clamp(DRAG_CURVE_T_MIN, DRAG_CURVE_T_MAX);
    let om = 1.0 - u;
    let denom = BERNSTEIN_CUBIC_COEFF * u * om * (om * om + u * u);
    if denom.abs() <= SOLVER_DET_EPSILON {
        return;
    }

    let p0 = contour[start_idx].point;
    let p3 = contour[end_idx].point;
    let s0 = contour[start_idx].node_type == NodeType::Smooth && !alt_held && initial_h1.distance_to(p0) >= DRAG_CURVE_MIN_HANDLE_LEN;
    let s3 = contour[end_idx].node_type == NodeType::Smooth && !alt_held && initial_h2.distance_to(p3) >= DRAG_CURVE_MIN_HANDLE_LEN;

    let w1 = om / denom;
    let w2 = u / denom;
    let t1 = FontPoint::new(initial_h1.x.0 + dx * w1, initial_h1.y.0 + dy * w1);
    let t2 = FontPoint::new(initial_h2.x.0 + dx * w2, initial_h2.y.0 + dy * w2);

    let (h1_new, h2_new) = match (s0, s3) {
        (false, false) => {
            let wf1 = SEGMENT_DRAG_FREE_WEIGHT_FACTOR * om;
            let wf2 = SEGMENT_DRAG_FREE_WEIGHT_FACTOR * u;
            (
                FontPoint::new(initial_h1.x.0 + dx * wf1, initial_h1.y.0 + dy * wf1),
                FontPoint::new(initial_h2.x.0 + dx * wf2, initial_h2.y.0 + dy * wf2),
            )
        }
        (true, true) => (
            clamp_handle_length(p0, project_onto_infinite_line(t1, p0, initial_h1), SEGMENT_DRAG_MAX_HANDLE_LEN_UPM),
            clamp_handle_length(p3, project_onto_infinite_line(t2, p3, initial_h2), SEGMENT_DRAG_MAX_HANDLE_LEN_UPM),
        ),
        (true, false) => {
            let h1_proj = clamp_handle_length(p0, project_onto_infinite_line(t1, p0, initial_h1), SEGMENT_DRAG_MAX_HANDLE_LEN_UPM);
            let comp = om / (2.0 * u);
            let h2_comp = FontPoint::new(t2.x.0 + comp * (t1.x.0 - h1_proj.x.0), t2.y.0 + comp * (t1.y.0 - h1_proj.y.0));
            (h1_proj, clamp_handle_length(p3, h2_comp, SEGMENT_DRAG_MAX_HANDLE_LEN_UPM))
        }
        (false, true) => {
            let h2_proj = clamp_handle_length(p3, project_onto_infinite_line(t2, p3, initial_h2), SEGMENT_DRAG_MAX_HANDLE_LEN_UPM);
            let comp = u / (2.0 * om);
            let h1_comp = FontPoint::new(t1.x.0 + comp * (t2.x.0 - h2_proj.x.0), t1.y.0 + comp * (t2.y.0 - h2_proj.y.0));
            (clamp_handle_length(p0, h1_comp, SEGMENT_DRAG_MAX_HANDLE_LEN_UPM), h2_proj)
        }
    };

    contour[h1_idx].point = h1_new;
    contour[h2_idx].point = h2_new;

    if !alt_held {
        let h1_id = contour[h1_idx].id;
        let h2_id = contour[h2_idx].id;
        enforce_smooth_collinearity_on_handle_drag(contour, closed, h1_id, false);
        enforce_smooth_collinearity_on_handle_drag(contour, closed, h2_id, false);
    }
}

fn solve_1d_cubic_deriv_roots(p0: f64, p1: f64, p2: f64, p3: f64) -> Vec<f64> {
    let a = -p0 + BERNSTEIN_CUBIC_COEFF * p1 - BERNSTEIN_CUBIC_COEFF * p2 + p3;
    let b = p0 - 2.0 * p1 + p2;
    let c = p1 - p0;
    let mut roots = Vec::new();

    if a.abs() <= SOLVER_DET_EPSILON {
        if b.abs() > SOLVER_DET_EPSILON {
            let t = -c / (2.0 * b);
            if t > EXTREMUM_T_MIN && t < EXTREMUM_T_MAX {
                roots.push(t);
            }
        }
        return roots;
    }

    let disc = b * b - a * c;
    if disc >= 0.0 {
        let q = disc.sqrt();
        for t in [(-b - q) / a, (-b + q) / a] {
            if t > EXTREMUM_T_MIN && t < EXTREMUM_T_MAX {
                roots.push(t);
            }
        }
    }
    roots
}

/// Finds the horizontal or vertical extremum ($dx/dt = 0$ or $dy/dt = 0$, $t \in (0.01, 0.99)$)
/// on `cubic` nearest to `ref_t` when $|\Delta t| \le \text{EXTREMUM\_SNAP\_MAX\_DELTA\_T}$ (`0.25`,
/// matching `CanvasTool::alignPathTime:path:`).
#[must_use]
pub fn find_nearest_cubic_extremum(cubic: CubicBez, ref_t: f64) -> (f64, FontPoint) {
    let mut candidates = solve_1d_cubic_deriv_roots(cubic.p0.x, cubic.p1.x, cubic.p2.x, cubic.p3.x);
    candidates.extend(solve_1d_cubic_deriv_roots(
        cubic.p0.y, cubic.p1.y, cubic.p2.y, cubic.p3.y,
    ));

    let best_t = candidates
        .into_iter()
        .filter(|t| (t - ref_t).abs() <= EXTREMUM_SNAP_MAX_DELTA_T)
        .min_by(|a, b| {
            (a - ref_t)
                .abs()
                .partial_cmp(&(b - ref_t).abs())
                .unwrap_or(std::cmp::Ordering::Equal)
        })
        .unwrap_or(ref_t);

    let pt = cubic.eval(best_t);
    (best_t, FontPoint::new(pt.x, pt.y))
}

/// Scales the partner handle on the same cubic segment proportionally when `Ctrl + Alt`
/// is held during handle dragging (`SPEC-002 §3.1.4`).
pub fn apply_proportional_tension_handle_drag(
    contour: &mut [Node],
    closed: bool,
    moved_handle_id: NodeId,
    old_handle_pos: FontPoint,
    new_handle_pos: FontPoint,
) {
    let segments = extract_segments(contour, closed);
    for seg in segments {
        let ContourSegment::Cubic {
            start_idx,
            h1_idx,
            h2_idx,
            end_idx,
            ..
        } = seg
        else {
            continue;
        };
        let (anchor_idx, partner_h_idx, partner_anchor_idx) =
            if contour[h1_idx].id == moved_handle_id {
                (start_idx, h2_idx, end_idx)
            } else if contour[h2_idx].id == moved_handle_id {
                (end_idx, h1_idx, start_idx)
            } else {
                continue;
            };

        let anchor_pt = contour[anchor_idx].point;
        let old_len = old_handle_pos.distance_to(anchor_pt);
        let new_len = new_handle_pos.distance_to(anchor_pt);
        if old_len <= COLLINEAR_MIN_LEN_EPSILON {
            return;
        }
        let ratio = (new_len / old_len).clamp(TENSION_SCALE_MIN, TENSION_SCALE_MAX);
        let p_anchor = contour[partner_anchor_idx].point;
        let p_handle = contour[partner_h_idx].point;
        let vx = p_handle.x.0 - p_anchor.x.0;
        let vy = p_handle.y.0 - p_anchor.y.0;
        contour[partner_h_idx].point = FontPoint::new(p_anchor.x.0 + vx * ratio, p_anchor.y.0 + vy * ratio);
        let partner_id = contour[partner_h_idx].id;
        enforce_smooth_collinearity_on_handle_drag(contour, closed, partner_id, false);
        return;
    }
}

/// Reverses the winding direction of a contour while keeping the first `OnCurve` node as
/// the starting anchor on closed contours (`SPEC-002 §5.3`).
pub fn reverse_contour_nodes(contour: &mut [Node], closed: bool) {
    if contour.len() <= 1 {
        return;
    }
    if !closed {
        contour.reverse();
        return;
    }
    let first_on_id = contour
        .iter()
        .find(|n| n.node_type != NodeType::OffCurve)
        .map(|n| n.id);
    contour.reverse();
    if let Some(target_id) = first_on_id {
        if let Some(pos) = contour.iter().position(|n| n.id == target_id) {
            contour.rotate_left(pos);
        }
    }
}

/// Rotates a closed contour so that the first selected `OnCurve` node becomes `contour[0]`
/// (`Make Node First` / `Set Start Node`, `SPEC-002 §5.3`).
pub fn make_selected_on_curve_node_first(
    contour: &mut [Node],
    closed: bool,
    selected: &[NodeId],
) -> bool {
    if !closed || contour.len() < 2 {
        return false;
    }
    if let Some(pos) = contour
        .iter()
        .position(|n| selected.contains(&n.id) && n.node_type != NodeType::OffCurve)
    {
        if pos > 0 {
            contour.rotate_left(pos);
            return true;
        }
    }
    false
}

/// Slides a selected `OnCurve` node along its adjacent `Line` or `Cubic` segments while preserving
/// curvature via De Casteljau subdivision (`SPEC-002 §3.1.3`).
pub fn slide_on_curve_node_along_contour(
    contour: &mut [Node],
    closed: bool,
    node_id: NodeId,
    target_pt: FontPoint,
) -> bool {
    let Some(node_idx) = contour.iter().position(|n| n.id == node_id) else {
        return false;
    };
    if contour[node_idx].node_type == NodeType::OffCurve {
        return false;
    }
    let segments = extract_segments(contour, closed);
    let seg_in = segments.iter().copied().find(|s| match s {
        ContourSegment::Line { end_idx, .. } | ContourSegment::Cubic { end_idx, .. } => *end_idx == node_idx,
    });
    let seg_out = segments.iter().copied().find(|s| match s {
        ContourSegment::Line { start_idx, .. } | ContourSegment::Cubic { start_idx, .. } => *start_idx == node_idx,
    });
    let target = Point::new(target_pt.x.0, target_pt.y.0);
    let eval_seg = |s: ContourSegment| -> (f64, f64) {
        let res = match s {
            ContourSegment::Line { line, .. } => line.nearest(target, CURVE_SOLVE_ACCURACY),
            ContourSegment::Cubic { cubic, .. } => cubic.nearest(target, CURVE_SOLVE_ACCURACY),
        };
        (res.distance_sq.sqrt(), res.t)
    };

    let in_cand = seg_in.map(eval_seg);
    let out_cand = seg_out.map(eval_seg);
    let use_in = match (in_cand, out_cand) {
        (Some((d_in, _)), Some((d_out, _))) => d_in <= d_out,
        (Some(_), None) => true,
        (None, Some(_)) => false,
        (None, None) => return false,
    };

    if use_in {
        let ( Some(s_in), Some((_, raw_t)) ) = (seg_in, in_cand) else { return false; };
        let u = raw_t.clamp(SPLIT_RATIO_MIN, 1.0);
        match s_in {
            ContourSegment::Line { line, .. } => {
                let p = line.eval(u);
                let old = contour[node_idx].point;
                contour[node_idx].point = FontPoint::new(p.x, p.y);
                if let Some(ContourSegment::Cubic { h1_idx, .. }) = seg_out {
                    contour[h1_idx].point.x.0 += p.x - old.x.0;
                    contour[h1_idx].point.y.0 += p.y - old.y.0;
                }
            }
            ContourSegment::Cubic { h1_idx, h2_idx, cubic, .. } => {
                let left = cubic.subsegment(0.0..u);
                let rem = cubic.subsegment(u..1.0);
                contour[h1_idx].point = FontPoint::new(left.p1.x, left.p1.y);
                contour[h2_idx].point = FontPoint::new(left.p2.x, left.p2.y);
                contour[node_idx].point = FontPoint::new(left.p3.x, left.p3.y);
                if (1.0 - u) > COLLINEAR_MIN_LEN_EPSILON {
                    if let Some(ContourSegment::Cubic { h1_idx: oh1, h2_idx: oh2, cubic: oc, .. }) = seg_out {
                        let chain = [
                            ContourSegment::Cubic { start_idx: 0, h1_idx: 1, h2_idx: 2, end_idx: 3, cubic: rem },
                            ContourSegment::Cubic { start_idx: 3, h1_idx: 4, h2_idx: 5, end_idx: 6, cubic: oc },
                        ];
                        let (fh1, fh2) = fit_cubic_to_segment_chain(&chain);
                        contour[oh1].point = fh1;
                        contour[oh2].point = fh2;
                    }
                }
            }
        }
    } else {
        let ( Some(s_out), Some((_, raw_t)) ) = (seg_out, out_cand) else { return false; };
        let v = raw_t.clamp(0.0, SPLIT_RATIO_MAX);
        match s_out {
            ContourSegment::Line { line, .. } => {
                let p = line.eval(v);
                let old = contour[node_idx].point;
                contour[node_idx].point = FontPoint::new(p.x, p.y);
                if let Some(ContourSegment::Cubic { h2_idx, .. }) = seg_in {
                    contour[h2_idx].point.x.0 += p.x - old.x.0;
                    contour[h2_idx].point.y.0 += p.y - old.y.0;
                }
            }
            ContourSegment::Cubic { h1_idx, h2_idx, cubic, .. } => {
                let rem = cubic.subsegment(0.0..v);
                let right = cubic.subsegment(v..1.0);
                contour[node_idx].point = FontPoint::new(right.p0.x, right.p0.y);
                contour[h1_idx].point = FontPoint::new(right.p1.x, right.p1.y);
                contour[h2_idx].point = FontPoint::new(right.p2.x, right.p2.y);
                if v > COLLINEAR_MIN_LEN_EPSILON {
                    if let Some(ContourSegment::Cubic { h1_idx: ih1, h2_idx: ih2, cubic: ic, .. }) = seg_in {
                        let chain = [
                            ContourSegment::Cubic { start_idx: 0, h1_idx: 1, h2_idx: 2, end_idx: 3, cubic: ic },
                            ContourSegment::Cubic { start_idx: 3, h1_idx: 4, h2_idx: 5, end_idx: 6, cubic: rem },
                        ];
                        let (fh1, fh2) = fit_cubic_to_segment_chain(&chain);
                        contour[ih1].point = fh1;
                        contour[ih2].point = fh2;
                    }
                }
            }
        }
    }
    true
}

