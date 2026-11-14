//! Empirically verified `Ctrl+Alt` (`setInterpolate:withPoint:` & `setMirrored:`),
//! `Cmd+Ctrl` (`moveNodeOnPath:` / `shiftMiddle:`), and `Alt` (`nearest_point_on_line`)
//! node/handle manipulation operators (`SPEC-002 §3.1.3–3.1.4`).

use crate::core::math_consts::{
    BERNSTEIN_CUBIC_COEFF, COLLINEAR_MIN_LEN_EPSILON, NUDGE_PREV_SPAN_Y_MIN, NUDGE_RATIO_CLAMP_MAX,
    NUDGE_RATIO_CLAMP_MIN, NUDGE_SPAN_MIN, SHIFT_MIDDLE_MAX_T, SHIFT_MIDDLE_TANGENT_FACTOR,
    SOLVER_DET_EPSILON,
};
use crate::core::{FontPoint, Node, NodeId, NodeType};

fn wrap_index(idx: isize, len: usize, closed: bool) -> Option<usize> {
    if len == 0 {
        return None;
    }
    if closed {
        Some(idx.rem_euclid(len as isize) as usize)
    } else {
        usize::try_from(idx).ok().filter(|&u| u < len)
    }
}

fn lerp_pt(a: FontPoint, b: FontPoint, t: f64) -> FontPoint {
    let u = 1.0 - t;
    FontPoint::new(u * a.x.0 + t * b.x.0, u * a.y.0 + t * b.y.0)
}

fn pick_far_coord(a: f64, b: f64, p3: f64) -> f64 {
    if (a - p3).abs() >= (b - p3).abs() {
        a
    } else {
        b
    }
}

fn interp_coord(val: f64, cur: f64, p3: f64, span: f64, min_span: f64, delta: f64) -> f64 {
    if span.abs() > min_span {
        let ratio = (1.0 - (val - p3) / span).clamp(NUDGE_RATIO_CLAMP_MIN, NUDGE_RATIO_CLAMP_MAX);
        val + ratio * delta
    } else if (val - p3).abs() > NUDGE_SPAN_MIN {
        val
    } else {
        cur
    }
}

/// Projects `pt` perpendicularly onto the infinite line passing through `line_a` and `line_b`
/// (`nearest_point_on_line`).
#[must_use]
pub fn project_point_onto_infinite_line(
    pt: FontPoint,
    line_a: FontPoint,
    line_b: FontPoint,
) -> FontPoint {
    let vx = line_b.x.0 - line_a.x.0;
    let vy = line_b.y.0 - line_a.y.0;
    let len_sq = vx * vx + vy * vy;
    if len_sq <= SOLVER_DET_EPSILON {
        return line_a;
    }
    let u = ((pt.x.0 - line_a.x.0) * vx + (pt.y.0 - line_a.y.0) * vy) / len_sq;
    FontPoint::new(line_a.x.0 + u * vx, line_a.y.0 + u * vy)
}

fn align_collinear_away_from_opposite(
    handle_pt: FontPoint,
    anchor_pt: FontPoint,
    opposite_handle_pt: FontPoint,
) -> FontPoint {
    let vx = anchor_pt.x.0 - opposite_handle_pt.x.0;
    let vy = anchor_pt.y.0 - opposite_handle_pt.y.0;
    let v_len = vx.hypot(vy);
    let arm_len = handle_pt.distance_to(anchor_pt);
    if v_len <= COLLINEAR_MIN_LEN_EPSILON {
        return handle_pt;
    }
    FontPoint::new(
        anchor_pt.x.0 + (vx / v_len) * arm_len,
        anchor_pt.y.0 + (vy / v_len) * arm_len,
    )
}

fn interpolate_adjacent_cubic_handles(
    contour: &mut [Node],
    shadow: &[Node],
    closed: bool,
    selected_nodes: &[NodeId],
    i: usize,
    delta: (f64, f64),
    dir: isize,
) {
    let n = shadow.len();
    let (dx, dy) = delta;
    let p3 = shadow[i].point;
    let (Some(h_near_idx), Some(h_far_idx), Some(opp_anchor_idx)) = (
        wrap_index(i as isize + dir, n, closed),
        wrap_index(i as isize + 2 * dir, n, closed),
        wrap_index(i as isize + 3 * dir, n, closed),
    ) else {
        return;
    };
    if shadow[h_near_idx].node_type != NodeType::OffCurve
        || shadow[h_far_idx].node_type != NodeType::OffCurve
        || shadow[opp_anchor_idx].node_type == NodeType::OffCurve
        || selected_nodes.contains(&shadow[opp_anchor_idx].id)
    {
        return;
    }
    let h_far = shadow[h_far_idx].point;
    let opp = shadow[opp_anchor_idx].point;
    let span_x = pick_far_coord(opp.x.0, h_far.x.0, p3.x.0) - p3.x.0;
    let span_y = pick_far_coord(opp.y.0, h_far.y.0, p3.y.0) - p3.y.0;
    let min_span_y = if dir > 0 {
        NUDGE_SPAN_MIN
    } else {
        NUDGE_PREV_SPAN_Y_MIN
    };

    for hk_idx in [h_near_idx, h_far_idx] {
        if !selected_nodes.contains(&shadow[hk_idx].id) {
            let hk = shadow[hk_idx].point;
            let cur = contour[hk_idx].point;
            let nx = interp_coord(hk.x.0, cur.x.0, p3.x.0, span_x, NUDGE_SPAN_MIN, dx);
            let ny = interp_coord(hk.y.0, cur.y.0, p3.y.0, span_y, min_span_y, dy);
            contour[hk_idx].point = FontPoint::new(nx, ny);
        }
    }
    if (span_x.abs() > NUDGE_SPAN_MIN || span_y.abs() > NUDGE_PREV_SPAN_Y_MIN)
        && contour[opp_anchor_idx].node_type == NodeType::Smooth
        && !selected_nodes.contains(&shadow[h_far_idx].id)
    {
        if let Some(beyond_idx) = wrap_index(i as isize + 4 * dir, n, closed) {
            if beyond_idx != h_near_idx && contour[beyond_idx].node_type == NodeType::OffCurve {
                contour[h_far_idx].point = align_collinear_away_from_opposite(
                    contour[h_far_idx].point,
                    contour[opp_anchor_idx].point,
                    contour[beyond_idx].point,
                );
            }
        }
    }
}

/// Applies `setInterpolate:withPoint:` (Nudge) on unselected `OffCurve` handles of cubic segments
/// adjacent to any selected `OnCurve` node (`Ctrl + Alt` + Drag or `Ctrl + Alt` + Arrows).
pub fn apply_ctrl_alt_interpolate_nudge(
    contours: &mut [Vec<Node>],
    shadow_contours: &[Vec<Node>],
    closed_flags: &[bool],
    selected_nodes: &[NodeId],
    dx: f64,
    dy: f64,
) {
    for (c_idx, contour) in contours.iter_mut().enumerate() {
        let Some(shadow) = shadow_contours.get(c_idx) else {
            continue;
        };
        let n = shadow.len();
        if n < 4 || contour.len() != n {
            continue;
        }
        let closed = closed_flags.get(c_idx).copied().unwrap_or(true);

        for i in 0..n {
            if shadow[i].node_type == NodeType::OffCurve || !selected_nodes.contains(&shadow[i].id)
            {
                continue;
            }
            for dir in [1, -1] {
                interpolate_adjacent_cubic_handles(
                    contour,
                    shadow,
                    closed,
                    selected_nodes,
                    i,
                    (dx, dy),
                    dir,
                );
            }
        }
    }
}

/// Applies `setMirrored:withCenterPoint:oppositePoint:` when ONLY `OffCurve` handles are selected
/// and dragged with `Ctrl + Alt`: mirrors the opposite `OffCurve` handle across an unselected
/// `Smooth` anchor in both angle (180°) and length (`oppHandle' = 2 * anchor - draggedHandle'`).
pub fn apply_ctrl_alt_mirrored_handle_drag(
    contour: &mut [Node],
    closed: bool,
    moved_handle_id: NodeId,
    selected_nodes: &[NodeId],
) {
    let n = contour.len();
    if n < 3 {
        return;
    }
    let Some(h_idx) = contour.iter().position(|node| node.id == moved_handle_id) else {
        return;
    };
    if contour[h_idx].node_type != NodeType::OffCurve {
        return;
    }
    let dragged_pt = contour[h_idx].point;

    for (cand_anchor_off, cand_opp_off) in [(-1isize, -2isize), (1isize, 2isize)] {
        let Some(anchor_idx) = wrap_index(h_idx as isize + cand_anchor_off, n, closed) else {
            continue;
        };
        if contour[anchor_idx].node_type != NodeType::Smooth
            || selected_nodes.contains(&contour[anchor_idx].id)
        {
            continue;
        }
        let Some(opp_idx) = wrap_index(h_idx as isize + cand_opp_off, n, closed) else {
            continue;
        };
        if opp_idx != h_idx
            && contour[opp_idx].node_type == NodeType::OffCurve
            && !selected_nodes.contains(&contour[opp_idx].id)
        {
            let anchor_pt = contour[anchor_idx].point;
            contour[opp_idx].point = FontPoint::new(
                2.0 * anchor_pt.x.0 - dragged_pt.x.0,
                2.0 * anchor_pt.y.0 - dragged_pt.y.0,
            );
            return;
        }
    }
}

fn slide_middle_toward_next(
    contour: &mut [Node],
    indices: [usize; 5],
    pts: [FontPoint; 6],
    target: FontPoint,
    d1: f64,
    d2: f64,
) -> bool {
    let [p1_i, p2_i, i, q1_i, q2_i] = indices;
    let [p0, p1, p3, q1, q2, q3] = pts;
    if d2 <= COLLINEAR_MIN_LEN_EPSILON {
        return false;
    }
    let proj = project_point_onto_infinite_line(target, p3, q1);
    let s = p3.distance_to(proj) / (BERNSTEIN_CUBIC_COEFF * d2);
    if s <= 0.0 || s >= SHIFT_MIDDLE_MAX_T {
        return true;
    }
    let (a0, a1, a2) = (lerp_pt(p3, q1, s), lerp_pt(q1, q2, s), lerp_pt(q2, q3, s));
    let (b0, b1) = (lerp_pt(a0, a1, s), lerp_pt(a1, a2, s));
    let p3_new = lerp_pt(b0, b1, s);
    let (vx, vy) = (b1.x.0 - b0.x.0, b1.y.0 - b0.y.0);
    let l_len = vx.hypot(vy);
    if l_len <= COLLINEAR_MIN_LEN_EPSILON {
        return true;
    }
    let (tau_x, tau_y) = (vx / l_len, vy / l_len);
    let d_new = d1 + SHIFT_MIDDLE_TANGENT_FACTOR * (l_len + d2) * s;
    contour[i].point = p3_new;
    contour[q1_i].point = b1;
    contour[q2_i].point = a2;
    contour[p2_i].point = FontPoint::new(p3_new.x.0 - d_new * tau_x, p3_new.y.0 - d_new * tau_y);
    if d1 > COLLINEAR_MIN_LEN_EPSILON {
        let ratio = d_new / d1;
        contour[p1_i].point = FontPoint::new(
            p0.x.0 + ratio * (p1.x.0 - p0.x.0),
            p0.y.0 + ratio * (p1.y.0 - p0.y.0),
        );
    }
    true
}

fn slide_middle_toward_prev(
    contour: &mut [Node],
    indices: [usize; 5],
    pts: [FontPoint; 6],
    target: FontPoint,
    d1: f64,
    d2: f64,
) -> bool {
    let [p1_i, p2_i, i, q1_i, q2_i] = indices;
    let [p0, p1, p2, p3, q2, q3] = pts;
    if d1 <= COLLINEAR_MIN_LEN_EPSILON {
        return false;
    }
    let proj = project_point_onto_infinite_line(target, p3, p2);
    let abs_t = p3.distance_to(proj) / (BERNSTEIN_CUBIC_COEFF * d1);
    if abs_t <= 0.0 || abs_t >= SHIFT_MIDDLE_MAX_T {
        return true;
    }
    let u = 1.0 - abs_t;
    let (a0, a1, a2) = (lerp_pt(p0, p1, u), lerp_pt(p1, p2, u), lerp_pt(p2, p3, u));
    let (b0, b1) = (lerp_pt(a0, a1, u), lerp_pt(a1, a2, u));
    let p3_new = lerp_pt(b0, b1, u);
    let (vx, vy) = (b1.x.0 - b0.x.0, b1.y.0 - b0.y.0);
    let l_len = vx.hypot(vy);
    if l_len <= COLLINEAR_MIN_LEN_EPSILON {
        return true;
    }
    let (tau_x, tau_y) = (vx / l_len, vy / l_len);
    let d_new = d2 + SHIFT_MIDDLE_TANGENT_FACTOR * (l_len + d1) * abs_t;
    contour[i].point = p3_new;
    contour[p1_i].point = a0;
    contour[p2_i].point = b0;
    contour[q1_i].point = FontPoint::new(p3_new.x.0 + d_new * tau_x, p3_new.y.0 + d_new * tau_y);
    if d2 > COLLINEAR_MIN_LEN_EPSILON {
        let ratio = d_new / d2;
        contour[q2_i].point = FontPoint::new(
            q3.x.0 + ratio * (q2.x.0 - q3.x.0),
            q3.y.0 + ratio * (q2.y.0 - q3.y.0),
        );
    }
    true
}

/// Slides a `Smooth` `OnCurve` node between two cubic segments (`Cmd + Ctrl` + Drag,
/// `moveNodeOnPath:shadowLayer:offset:` / `shiftMiddle:`). Returns `true` if handled.
pub fn slide_smooth_node_on_cubic_path_shift_middle(
    contour: &mut [Node],
    shadow: &[Node],
    closed: bool,
    node_id: NodeId,
    dx: f64,
    dy: f64,
) -> bool {
    let n = shadow.len();
    let min_len = if closed { 6 } else { 7 };
    if n < min_len || contour.len() != n {
        return false;
    }
    let Some(i) = shadow.iter().position(|node| node.id == node_id) else {
        return false;
    };
    if shadow[i].node_type != NodeType::Smooth {
        return false;
    }
    let (Some(p0_i), Some(p1_i), Some(p2_i), Some(q1_i), Some(q2_i), Some(q3_i)) = (
        wrap_index(i as isize - 3, n, closed),
        wrap_index(i as isize - 2, n, closed),
        wrap_index(i as isize - 1, n, closed),
        wrap_index(i as isize + 1, n, closed),
        wrap_index(i as isize + 2, n, closed),
        wrap_index(i as isize + 3, n, closed),
    ) else {
        return false;
    };
    if shadow[p0_i].node_type == NodeType::OffCurve
        || shadow[p1_i].node_type != NodeType::OffCurve
        || shadow[p2_i].node_type != NodeType::OffCurve
        || shadow[q1_i].node_type != NodeType::OffCurve
        || shadow[q2_i].node_type != NodeType::OffCurve
        || shadow[q3_i].node_type == NodeType::OffCurve
    {
        return false;
    }

    let (p0, p1, p2, p3, q1, q2, q3) = (
        shadow[p0_i].point,
        shadow[p1_i].point,
        shadow[p2_i].point,
        shadow[i].point,
        shadow[q1_i].point,
        shadow[q2_i].point,
        shadow[q3_i].point,
    );
    let target = FontPoint::new(p3.x.0 + dx, p3.y.0 + dy);
    let (d1, d2) = (p3.distance_to(p2), q1.distance_to(p3));
    if p2.distance_to(target) >= q1.distance_to(target) {
        slide_middle_toward_next(
            contour,
            [p1_i, p2_i, i, q1_i, q2_i],
            [p0, p1, p3, q1, q2, q3],
            target,
            d1,
            d2,
        )
    } else {
        slide_middle_toward_prev(
            contour,
            [p1_i, p2_i, i, q1_i, q2_i],
            [p0, p1, p2, p3, q2, q3],
            target,
            d1,
            d2,
        )
    }
}

/// Projects a single `Smooth` `OnCurve` node without adjacent `OffCurve` handles onto the infinite
/// line through its two `OnCurve` neighbors $P_{\text{prev}}$ and $P_{\text{next}}$ when dragged with `Alt`.
pub fn slide_smooth_line_node_unlimited(
    contour: &mut [Node],
    shadow: &[Node],
    closed: bool,
    node_id: NodeId,
    dx: f64,
    dy: f64,
) -> bool {
    let n = shadow.len();
    if n < 3 || contour.len() != n {
        return false;
    }
    let Some(i) = shadow.iter().position(|node| node.id == node_id) else {
        return false;
    };
    if shadow[i].node_type != NodeType::Smooth {
        return false;
    }
    let (Some(prev_i), Some(next_i)) = (
        wrap_index(i as isize - 1, n, closed),
        wrap_index(i as isize + 1, n, closed),
    ) else {
        return false;
    };
    if shadow[prev_i].node_type == NodeType::OffCurve
        || shadow[next_i].node_type == NodeType::OffCurve
    {
        return false;
    }
    let target = FontPoint::new(shadow[i].point.x.0 + dx, shadow[i].point.y.0 + dy);
    contour[i].point =
        project_point_onto_infinite_line(target, shadow[prev_i].point, shadow[next_i].point);
    contour[i].node_type = NodeType::Smooth;
    true
}
