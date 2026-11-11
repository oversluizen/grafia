//! Open-contour joining and closing operations for `PenTool` (`P`) and `SelectTool` (`V`)
//! (`SPEC-002 §3.1.8 & §3.2.4`).

use crate::adapters::state::GlifoCanvasState;
use crate::core::{FontPoint, Node, NodeId, NodeType};
use crate::interaction::PenToolState;
use crate::tokens::{COLLINEAR_MIN_LEN_EPSILON, CUBIC_ONE_THIRD, SELECT_NODE_HIT_RADIUS_DP};

#[must_use]
pub fn alloc_off_curve(state: &mut GlifoCanvasState, pt: FontPoint) -> Node {
    let id = NodeId(state.next_node_id);
    state.next_node_id += 1;
    Node::new(id, pt.x.0, pt.y.0, NodeType::OffCurve)
}

#[must_use]
pub fn lerp_one_third(from: FontPoint, to: FontPoint) -> FontPoint {
    FontPoint::new(
        from.x.0 + (to.x.0 - from.x.0) * CUBIC_ONE_THIRD,
        from.y.0 + (to.y.0 - from.y.0) * CUBIC_ONE_THIRD,
    )
}

/// Joins the active open contour `a_idx` to the hit endpoint `target_node` (`P0` if `is_b_start`,
/// or `P_last` if `!is_b_start`) of another open contour `b_idx` (`b_idx != a_idx`) in `PenTool`.
pub fn join_open_contours_in_pen(
    state: &mut GlifoCanvasState,
    a_idx: usize,
    b_idx: usize,
    target_node: Node,
    is_b_start: bool,
    font_pt: FontPoint,
) {
    let a_out = state.pending_pen_bcp_out.filter(|&(c, _)| c == a_idx).map(|(_, pt)| pt);
    let a_first_in = state.pending_pen_first_bcp_in.filter(|&(c, _)| c == a_idx).map(|(_, pt)| pt);
    let b_out = state.pending_pen_bcp_out.filter(|&(c, _)| c == b_idx).map(|(_, pt)| pt);
    let b_first_in = state.pending_pen_first_bcp_in.filter(|&(c, _)| c == b_idx).map(|(_, pt)| pt);
    state.pending_pen_bcp_out = None;
    state.pending_pen_first_bcp_in = None;

    let (b_join_in, new_end_out) = if is_b_start {
        (b_first_in, b_out)
    } else {
        state.contours[b_idx].reverse();
        (b_out, b_first_in)
    };

    let b_had_out_handles = state.contours[b_idx]
        .get(1)
        .is_some_and(|n| n.node_type == NodeType::OffCurve);
    let b_was_smooth = state.contours[b_idx]
        .first()
        .is_some_and(|n| n.node_type == NodeType::Smooth);

    let a_last = state.contours[a_idx]
        .last()
        .map_or(target_node.point, |n| n.point);
    state.pen_seg_has_explicit_h1 = a_out.is_some();

    if a_out.is_some() || b_join_in.is_some() {
        let h1_pt = a_out.unwrap_or_else(|| lerp_one_third(a_last, b_join_in.unwrap()));
        let h2_pt =
            b_join_in.unwrap_or_else(|| lerp_one_third(target_node.point, a_out.unwrap()));
        let h1 = alloc_off_curve(state, h1_pt);
        let h2 = alloc_off_curve(state, h2_pt);
        state.contours[a_idx].push(h1);
        state.contours[a_idx].push(h2);
        if !b_had_out_handles && state.contours[b_idx].len() > 1 {
            if let Some(first_b) = state.contours[b_idx].first_mut() {
                first_b.node_type = NodeType::Corner;
            }
        }
    } else if let Some(first_b) = state.contours[b_idx].first_mut() {
        first_b.node_type = NodeType::Corner;
    }

    let b_nodes = state.contours[b_idx].clone();
    state.contours[a_idx].extend(b_nodes);
    state.contours.remove(b_idx);
    if b_idx < state.glyph.contours.len() {
        state.glyph.contours.remove(b_idx);
    }
    let merged_idx = if b_idx < a_idx { a_idx - 1 } else { a_idx };

    state.pending_pen_first_bcp_in = a_first_in.map(|pt| (merged_idx, pt));
    state.pending_pen_bcp_out = new_end_out.map(|pt| (merged_idx, pt));
    state.active_open_contour = Some(merged_idx);
    state.set_contour_closed(merged_idx, false);
    state.selected_nodes = vec![target_node.id];
    state.pen_state = PenToolState::PlacingAnchor {
        anchor: target_node.point,
    };
    state.pen_drag_last_pt = Some(font_pt);
    state.pen_handle_delta = (0.0, 0.0);
    state.pen_alt_frozen_bcp_in = b_join_in.or(Some(target_node.point));
    state.pen_resuming_corner_line_end = false;
    state.pen_join_b_had_out_handles = b_had_out_handles;
    state.pen_join_b_was_smooth = b_was_smooth;
    state.sync_paths_from_contours();
    state.assert_invariants();
}

/// Synchronizes dragged `PenTool` handles `(bcp_in, bcp_out)` into contour `c_idx`.
pub fn sync_pen_drag_handles_into_contour(
    state: &mut GlifoCanvasState,
    c_idx: usize,
    anchor: FontPoint,
    bcp_in: FontPoint,
    bcp_out: FontPoint,
    target_type: NodeType,
    is_closing: bool,
) {
    let Some(&sel_id) = state.selected_nodes.first() else {
        return;
    };
    if c_idx >= state.contours.len() {
        return;
    }
    let Some(mut node_idx) = state.contours[c_idx].iter().position(|n| n.id == sel_id) else {
        return;
    };
    state.contours[c_idx][node_idx].node_type = target_type;

    if is_closing && node_idx == 0 {
        let len = state.contours[c_idx].len();
        if len >= 3
            && state.contours[c_idx][len - 1].node_type == NodeType::OffCurve
            && state.contours[c_idx][len - 2].node_type == NodeType::OffCurve
        {
            state.contours[c_idx][len - 1].point = bcp_in;
            if !state.pen_seg_has_explicit_h1 && len >= 4 {
                let prev_on = state.contours[c_idx][len - 3].point;
                state.contours[c_idx][len - 2].point = lerp_one_third(prev_on, bcp_in);
            }
        } else if len >= 2 {
            let prev_on = state.contours[c_idx][len - 1].point;
            let h1 = alloc_off_curve(state, lerp_one_third(prev_on, bcp_in));
            let h2 = alloc_off_curve(state, bcp_in);
            state.contours[c_idx].push(h1);
            state.contours[c_idx].push(h2);
        }

        let (seg0_had_handles, start_was_smooth) = state
            .active_drag_before_snapshot
            .as_ref()
            .and_then(|s| s.contours.get(c_idx))
            .map_or((false, false), |c| {
                (
                    c.get(1).is_some_and(|n| n.node_type == NodeType::OffCurve),
                    c.first().is_some_and(|n| n.node_type == NodeType::Smooth),
                )
            });
        if state.contours[c_idx].len() >= 4
            && state.contours[c_idx][1].node_type == NodeType::OffCurve
            && state.contours[c_idx][2].node_type == NodeType::OffCurve
        {
            if !start_was_smooth {
                state.contours[c_idx][1].point = bcp_out;
                if !seg0_had_handles {
                    let next_on = state.contours[c_idx][3].point;
                    state.contours[c_idx][2].point = lerp_one_third(next_on, bcp_out);
                }
            }
        } else if state.contours[c_idx].len() >= 2
            && state.contours[c_idx][1].node_type != NodeType::OffCurve
        {
            let next_on = state.contours[c_idx][1].point;
            let h1 = alloc_off_curve(state, bcp_out);
            let h2 = alloc_off_curve(state, lerp_one_third(next_on, bcp_out));
            state.contours[c_idx].insert(1, h1);
            state.contours[c_idx].insert(2, h2);
        }
        return;
    }

    if node_idx == 0 {
        state.pending_pen_first_bcp_in =
            (bcp_in.distance_to(anchor) > COLLINEAR_MIN_LEN_EPSILON).then_some((c_idx, bcp_in));
    } else if bcp_in.distance_to(anchor) > COLLINEAR_MIN_LEN_EPSILON {
        if node_idx >= 2
            && state.contours[c_idx][node_idx - 1].node_type == NodeType::OffCurve
            && state.contours[c_idx][node_idx - 2].node_type == NodeType::OffCurve
        {
            state.contours[c_idx][node_idx - 1].point = bcp_in;
            if !state.pen_seg_has_explicit_h1 && node_idx >= 3 {
                let prev_on = state.contours[c_idx][node_idx - 3].point;
                state.contours[c_idx][node_idx - 2].point = lerp_one_third(prev_on, bcp_in);
            }
        } else if state.contours[c_idx][node_idx - 1].node_type != NodeType::OffCurve {
            let prev_on = state.contours[c_idx][node_idx - 1].point;
            let h1 = alloc_off_curve(state, lerp_one_third(prev_on, bcp_in));
            let h2 = alloc_off_curve(state, bcp_in);
            state.contours[c_idx].insert(node_idx, h1);
            state.contours[c_idx].insert(node_idx + 1, h2);
            node_idx += 2;
        }
    }

    if node_idx + 1 < state.contours[c_idx].len() {
        if bcp_out.distance_to(anchor) > COLLINEAR_MIN_LEN_EPSILON {
            if node_idx + 3 < state.contours[c_idx].len()
                && state.contours[c_idx][node_idx + 1].node_type == NodeType::OffCurve
                && state.contours[c_idx][node_idx + 2].node_type == NodeType::OffCurve
            {
                if !state.pen_join_b_was_smooth {
                    state.contours[c_idx][node_idx + 1].point = bcp_out;
                    if !state.pen_join_b_had_out_handles {
                        let next_on = state.contours[c_idx][node_idx + 3].point;
                        state.contours[c_idx][node_idx + 2].point =
                            lerp_one_third(next_on, bcp_out);
                    }
                }
            } else if state.contours[c_idx][node_idx + 1].node_type != NodeType::OffCurve {
                let next_on = state.contours[c_idx][node_idx + 1].point;
                let h1 = alloc_off_curve(state, bcp_out);
                let h2 = alloc_off_curve(state, lerp_one_third(next_on, bcp_out));
                state.contours[c_idx].insert(node_idx + 1, h1);
                state.contours[c_idx].insert(node_idx + 2, h2);
            }
        }
    } else if bcp_out.distance_to(anchor) > COLLINEAR_MIN_LEN_EPSILON {
        state.pending_pen_bcp_out = Some((c_idx, bcp_out));
    }
}

struct EndpointSnapMatch {
    a_idx: usize,
    dragged_is_start: bool,
    dragged_pt: FontPoint,
    b_idx: usize,
    target_is_start: bool,
    target_id: NodeId,
    target_pt: FontPoint,
    dist: f32,
}

/// When an open-contour endpoint (`P0` or `P_last`) is dragged and released within
/// `SELECT_NODE_HIT_RADIUS_DP` of another open-contour endpoint in `SelectTool`:
/// - On the **same** open contour (with `>= 3` `OnCurve` points before merge): merges the two
///   endpoints into one `OnCurve` node and closes the contour (`closed = true`).
/// - On **another** open contour (`b_idx != a_idx`): merges the two endpoints at the target
///   position and joins both open contours into a single open contour (`closed = false`).
pub fn try_join_or_close_open_contours_on_select_release(state: &mut GlifoCanvasState) -> bool {
    let mut best_match: Option<EndpointSnapMatch> = None;

    for a_idx in 0..state.contours.len() {
        if state.is_contour_closed(a_idx) || state.contours[a_idx].is_empty() {
            continue;
        }
        let a_contour = &state.contours[a_idx];
        let a_first = &a_contour[0];
        let a_last = &a_contour[a_contour.len() - 1];
        let mut a_candidates = Vec::with_capacity(2);
        if a_first.node_type != NodeType::OffCurve && state.selected_nodes.contains(&a_first.id) {
            a_candidates.push((true, a_first.id, a_first.point));
        }
        if a_contour.len() > 1
            && a_last.node_type != NodeType::OffCurve
            && state.selected_nodes.contains(&a_last.id)
        {
            a_candidates.push((false, a_last.id, a_last.point));
        }

        for (dragged_is_start, dragged_id, dragged_pt) in a_candidates {
            let dragged_scr = state.viewport.to_screen(dragged_pt);
            for b_idx in 0..state.contours.len() {
                if state.is_contour_closed(b_idx) || state.contours[b_idx].is_empty() {
                    continue;
                }
                if a_idx == b_idx {
                    let on_count = state.contours[a_idx]
                        .iter()
                        .filter(|n| n.node_type != NodeType::OffCurve)
                        .count();
                    if on_count < 3 {
                        continue;
                    }
                }
                let b_contour = &state.contours[b_idx];
                let b_first = &b_contour[0];
                let b_last = &b_contour[b_contour.len() - 1];
                let mut b_candidates = vec![(true, b_first.id, b_first.point)];
                if b_contour.len() > 1 {
                    b_candidates.push((false, b_last.id, b_last.point));
                }

                for (target_is_start, target_id, target_pt) in b_candidates {
                    if target_id == dragged_id || state.selected_nodes.contains(&target_id) {
                        continue;
                    }
                    let dist = dragged_scr.distance_to(state.viewport.to_screen(target_pt));
                    if dist < SELECT_NODE_HIT_RADIUS_DP
                        && best_match.as_ref().is_none_or(|m| dist < m.dist)
                    {
                        best_match = Some(EndpointSnapMatch {
                            a_idx,
                            dragged_is_start,
                            dragged_pt,
                            b_idx,
                            target_is_start,
                            target_id,
                            target_pt,
                            dist,
                        });
                    }
                }
            }
        }
    }

    let Some(EndpointSnapMatch {
        a_idx,
        dragged_is_start,
        dragged_pt,
        b_idx,
        target_is_start,
        target_id,
        target_pt,
        ..
    }) = best_match
    else {
        return false;
    };

    let snap_dx = target_pt.x.0 - dragged_pt.x.0;
    let snap_dy = target_pt.y.0 - dragged_pt.y.0;
    if dragged_is_start {
        state.contours[a_idx][0].point = target_pt;
        if state.contours[a_idx]
            .get(1)
            .is_some_and(|n| n.node_type == NodeType::OffCurve && !state.selected_nodes.contains(&n.id))
        {
            state.contours[a_idx][1].point.x.0 += snap_dx;
            state.contours[a_idx][1].point.y.0 += snap_dy;
        }
    } else {
        let a_len = state.contours[a_idx].len();
        state.contours[a_idx][a_len - 1].point = target_pt;
        if a_len >= 2
            && state.contours[a_idx][a_len - 2].node_type == NodeType::OffCurve
            && !state.selected_nodes.contains(&state.contours[a_idx][a_len - 2].id)
        {
            state.contours[a_idx][a_len - 2].point.x.0 += snap_dx;
            state.contours[a_idx][a_len - 2].point.y.0 += snap_dy;
        }
    }

    if a_idx == b_idx {
        state.contours[a_idx].pop();
        let has_out = state.contours[a_idx]
            .get(1)
            .is_some_and(|n| n.node_type == NodeType::OffCurve);
        let has_in = state.contours[a_idx]
            .last()
            .is_some_and(|n| n.node_type == NodeType::OffCurve);
        if !has_out || !has_in {
            state.contours[a_idx][0].node_type = NodeType::Corner;
        }
        let surviving_id = state.contours[a_idx][0].id;
        state.set_contour_closed(a_idx, true);
        if state.active_open_contour == Some(a_idx) {
            state.active_open_contour = None;
        }
        state.pending_pen_bcp_out = None;
        state.pending_pen_first_bcp_in = None;
        state.hovered_pen_close_target = None;
        state.hovered_pen_close_node = None;
        state.selected_nodes = vec![surviving_id];
        state.sync_paths_from_contours();
        state.assert_invariants();
        return true;
    }

    if dragged_is_start {
        state.contours[a_idx].reverse();
    }
    if !target_is_start {
        state.contours[b_idx].reverse();
    }
    state.contours[a_idx].pop();

    let has_in_off = state.contours[a_idx]
        .last()
        .is_some_and(|n| n.node_type == NodeType::OffCurve);
    let has_out_off = state.contours[b_idx]
        .get(1)
        .is_some_and(|n| n.node_type == NodeType::OffCurve);
    if !has_in_off || !has_out_off {
        if let Some(first_b) = state.contours[b_idx].first_mut() {
            first_b.node_type = NodeType::Corner;
        }
    }

    let b_nodes = state.contours[b_idx].clone();
    state.contours[a_idx].extend(b_nodes);
    state.contours.remove(b_idx);
    if b_idx < state.glyph.contours.len() {
        state.glyph.contours.remove(b_idx);
    }
    let merged_idx = if b_idx < a_idx { a_idx - 1 } else { a_idx };
    state.set_contour_closed(merged_idx, false);
    state.active_open_contour = Some(merged_idx);
    state.pending_pen_bcp_out = None;
    state.pending_pen_first_bcp_in = None;
    state.hovered_pen_close_target = None;
    state.hovered_pen_close_node = None;
    state.selected_nodes = vec![target_id];
    state.sync_paths_from_contours();
    state.assert_invariants();
    true
}
