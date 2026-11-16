//! `SPEC-011 §2.3 & §2.8.1`: Proportional Handle Servo (`servo_translate_on_curve_node`),
//! Interpolated Neighbour Nudge (`power_nudge_translate_selected`, Shortcut `C`),
//! and 4-Level Continuity Classification (`classify_node_continuity_in_contours`).

use crate::core::bezier_ops::{Node, NodeId, NodeType};
use crate::core::coordinates::FontPoint;
use crate::core::curve_harmony::{
    cubic_dkappa_ds, cubic_signed_curvature, harmonize_smooth_join_g3_in_contour,
    harmonize_smooth_join_in_contour, resolve_cubic_join_indices, CubicJoinIndices,
};
use crate::core::curve_manipulation::{extract_segments, ContourSegment};
use crate::core::math_consts::{
    CONTINUITY_G1_MAX_ANGLE_DEG, CONTINUITY_G2_MAX_KAPPA_DIFF, CONTINUITY_G3_MAX_DKAPPA_DS_DIFF,
    HARMONIZE_MIN_PERP_DIST_UPM, HARMONIZE_RATIO_CLAMP_MAX, HARMONIZE_RATIO_CLAMP_MIN,
    RAD_TO_DEG_F64, SERVO_MIN_SPAN_UPM, SOLVER_DET_EPSILON,
};
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;

/// Real-time 4-level geometric continuity grade at an on-curve join node (`SPEC-011 §2.8`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum ContinuityGrade {
    G0Corner,
    G1Tangent,
    G2Curvature,
    G3Clothoid,
}

/// Classifies the geometric continuity grade (`G0Corner` .. `G3Clothoid`) at `node_id` (`SPEC-011 §2.8`).
#[must_use]
pub fn classify_node_continuity_in_contours(
    contours: &[Vec<Node>],
    closed_flags: &[bool],
    node_id: NodeId,
) -> ContinuityGrade {
    for (c_idx, nodes) in contours.iter().enumerate() {
        let closed = closed_flags.get(c_idx).copied().unwrap_or(true);
        let Some(p_idx) = nodes.iter().position(|n| n.id == node_id) else {
            continue;
        };
        let Some(j) = resolve_cubic_join_indices(nodes, closed, p_idx) else {
            return classify_non_cubic_join_continuity(nodes, closed, p_idx);
        };
        let (p, q2, r1) = (nodes[j.p].point, nodes[j.q2].point, nodes[j.r1].point);
        let (v_in, v_out) = (
            (p.x.0 - q2.x.0, p.y.0 - q2.y.0),
            (r1.x.0 - p.x.0, r1.y.0 - p.y.0),
        );
        let (l_in, l_out) = (v_in.0.hypot(v_in.1), v_out.0.hypot(v_out.1));
        if l_in <= HARMONIZE_MIN_PERP_DIST_UPM || l_out <= HARMONIZE_MIN_PERP_DIST_UPM {
            return ContinuityGrade::G0Corner;
        }
        let dot = ((v_in.0 * v_out.0 + v_in.1 * v_out.1) / (l_in * l_out)).clamp(-1.0, 1.0);
        if dot.acos() * RAD_TO_DEG_F64 > CONTINUITY_G1_MAX_ANGLE_DEG {
            return ContinuityGrade::G0Corner;
        }
        let k_in = cubic_signed_curvature(nodes[j.q0].point, nodes[j.q1].point, q2, p, 1.0);
        let k_out = cubic_signed_curvature(p, r1, nodes[j.r2].point, nodes[j.r3].point, 0.0);
        if (k_in - k_out).abs() > CONTINUITY_G2_MAX_KAPPA_DIFF {
            return ContinuityGrade::G1Tangent;
        }
        let dk_in = cubic_dkappa_ds(nodes[j.q0].point, nodes[j.q1].point, q2, p, 1.0);
        let dk_out = cubic_dkappa_ds(p, r1, nodes[j.r2].point, nodes[j.r3].point, 0.0);
        if (dk_in - dk_out).abs() <= CONTINUITY_G3_MAX_DKAPPA_DS_DIFF {
            return ContinuityGrade::G3Clothoid;
        }
        return ContinuityGrade::G2Curvature;
    }
    ContinuityGrade::G0Corner
}

fn classify_non_cubic_join_continuity(
    nodes: &[Node],
    closed: bool,
    p_idx: usize,
) -> ContinuityGrade {
    let n = nodes.len();
    if n < 3 || p_idx >= n || nodes[p_idx].node_type == NodeType::OffCurve {
        return ContinuityGrade::G0Corner;
    }
    let (prev_idx, next_idx) = if closed {
        ((p_idx + n - 1) % n, (p_idx + 1) % n)
    } else if p_idx > 0 && p_idx + 1 < n {
        (p_idx - 1, p_idx + 1)
    } else {
        return ContinuityGrade::G0Corner;
    };
    let p = nodes[p_idx].point;
    let q = nodes[prev_idx].point;
    let r = nodes[next_idx].point;
    let (v_in, v_out) = (
        (p.x.0 - q.x.0, p.y.0 - q.y.0),
        (r.x.0 - p.x.0, r.y.0 - p.y.0),
    );
    let (l_in, l_out) = (v_in.0.hypot(v_in.1), v_out.0.hypot(v_out.1));
    if l_in <= HARMONIZE_MIN_PERP_DIST_UPM || l_out <= HARMONIZE_MIN_PERP_DIST_UPM {
        return ContinuityGrade::G0Corner;
    }
    let dot = ((v_in.0 * v_out.0 + v_in.1 * v_out.1) / (l_in * l_out)).clamp(-1.0, 1.0);
    if dot.acos() * RAD_TO_DEG_F64 <= CONTINUITY_G1_MAX_ANGLE_DEG {
        ContinuityGrade::G1Tangent
    } else {
        ContinuityGrade::G0Corner
    }
}

/// Translates `node_id` by `delta` and proportionally scales off-curve handles on adjacent cubic segments (`SPEC-011 §2.3.1`).
pub fn servo_translate_node_in_contours(
    contours: &mut [Vec<Node>],
    closed_flags: &[bool],
    node_id: NodeId,
    delta: FontPoint,
) -> bool {
    debug_assert!(delta.x.0.is_finite() && delta.y.0.is_finite());
    for (c_idx, nodes) in contours.iter_mut().enumerate() {
        let closed = closed_flags.get(c_idx).copied().unwrap_or(true);
        let Some(target_idx) = nodes
            .iter()
            .position(|n| n.id == node_id && n.node_type != NodeType::OffCurve)
        else {
            continue;
        };
        let p0_old = nodes[target_idx].point;
        let p0_new = FontPoint::new(p0_old.x.0 + delta.x.0, p0_old.y.0 + delta.y.0);
        let segments = extract_segments(nodes, closed);
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
            if start_idx == target_idx && end_idx != target_idx {
                let p3 = nodes[end_idx].point;
                scale_cubic_handles_servo(nodes, h1_idx, h2_idx, [p0_old, p0_new, p3], delta, true);
            } else if end_idx == target_idx && start_idx != target_idx {
                let p3 = nodes[start_idx].point;
                scale_cubic_handles_servo(nodes, h2_idx, h1_idx, [p0_old, p0_new, p3], delta, true);
            }
        }
        nodes[target_idx].point = p0_new;
        return true;
    }
    false
}

fn scale_cubic_handles_servo(
    nodes: &mut [Node],
    near_h_idx: usize,
    far_h_idx: usize,
    anchors: [FontPoint; 3],
    delta: FontPoint,
    include_far_handle: bool,
) {
    let [p0_old, p0_new, p3] = anchors;
    let span_x_old = p0_old.x.0 - p3.x.0;
    let span_y_old = p0_old.y.0 - p3.y.0;
    let span_x_new = p0_new.x.0 - p3.x.0;
    let span_y_new = p0_new.y.0 - p3.y.0;

    let indices = if include_far_handle {
        vec![near_h_idx, far_h_idx]
    } else {
        vec![near_h_idx]
    };
    for idx in indices {
        let h = nodes[idx].point;
        let nx = if span_x_old.abs() > SERVO_MIN_SPAN_UPM {
            p3.x.0 + (h.x.0 - p3.x.0) * (span_x_new / span_x_old)
        } else if idx == near_h_idx {
            h.x.0 + delta.x.0
        } else {
            h.x.0
        };
        let ny = if span_y_old.abs() > SERVO_MIN_SPAN_UPM {
            p3.y.0 + (h.y.0 - p3.y.0) * (span_y_new / span_y_old)
        } else if idx == near_h_idx {
            h.y.0 + delta.y.0
        } else {
            h.y.0
        };
        nodes[idx].point = FontPoint::new(nx, ny);
    }
}

/// Translates selected nodes by `delta` and linearly interpolates unselected contour nodes between
/// the selection's pre-move interval and the stationary contour extrema (`SPEC-011 §2.3.2`).
pub fn power_nudge_contours(
    contours: &mut [Vec<Node>],
    closed_flags: &[bool],
    selected_nodes: &[NodeId],
    delta: FontPoint,
) -> bool {
    debug_assert!(delta.x.0.is_finite() && delta.y.0.is_finite());
    if selected_nodes.is_empty()
        || (delta.x.0.abs() <= SOLVER_DET_EPSILON && delta.y.0.abs() <= SOLVER_DET_EPSILON)
    {
        return false;
    }
    let mut any_changed = false;
    for (c_idx, nodes) in contours.iter_mut().enumerate() {
        let _closed = closed_flags.get(c_idx).copied().unwrap_or(true);
        let sel_indices: Vec<usize> = nodes
            .iter()
            .enumerate()
            .filter_map(|(i, n)| selected_nodes.contains(&n.id).then_some(i))
            .collect();
        if sel_indices.is_empty() {
            continue;
        }
        let mut x_min = f64::INFINITY;
        let mut x_max = f64::NEG_INFINITY;
        let mut y_min = f64::INFINITY;
        let mut y_max = f64::NEG_INFINITY;
        for n in nodes.iter() {
            x_min = x_min.min(n.point.x.0);
            x_max = x_max.max(n.point.x.0);
            y_min = y_min.min(n.point.y.0);
            y_max = y_max.max(n.point.y.0);
        }
        let mut sx_min = f64::INFINITY;
        let mut sx_max = f64::NEG_INFINITY;
        let mut sy_min = f64::INFINITY;
        let mut sy_max = f64::NEG_INFINITY;
        for &si in &sel_indices {
            let p = nodes[si].point;
            sx_min = sx_min.min(p.x.0);
            sx_max = sx_max.max(p.x.0);
            sy_min = sy_min.min(p.y.0);
            sy_max = sy_max.max(p.y.0);
        }

        for (i, n) in nodes.iter_mut().enumerate() {
            if sel_indices.contains(&i) {
                n.point.x.0 += delta.x.0;
                n.point.y.0 += delta.y.0;
                continue;
            }
            let wx = interp_weight_1d(n.point.x.0, x_min, x_max, sx_min, sx_max);
            let wy = interp_weight_1d(n.point.y.0, y_min, y_max, sy_min, sy_max);
            n.point.x.0 += wx * delta.x.0;
            n.point.y.0 += wy * delta.y.0;
        }
        any_changed = true;
    }
    any_changed
}

fn interp_weight_1d(val: f64, c_min: f64, c_max: f64, s_min: f64, s_max: f64) -> f64 {
    if val >= s_min && val <= s_max {
        1.0
    } else if val < s_min {
        let span = s_min - c_min;
        if span > SERVO_MIN_SPAN_UPM {
            ((val - c_min) / span).clamp(0.0, 1.0)
        } else {
            0.0
        }
    } else {
        let span = c_max - s_max;
        if span > SERVO_MIN_SPAN_UPM {
            ((c_max - val) / span).clamp(0.0, 1.0)
        } else {
            0.0
        }
    }
}

/// Enforces live $G^2$ (`genius_nodes`) or $G^3$ (`g3_nodes`) continuity when an adjacent handle is translated.
pub fn enforce_genius_or_g3_on_handle_move(
    contours: &mut [Vec<Node>],
    closed_flags: &[bool],
    moved_node_id: NodeId,
    genius_nodes: &BTreeSet<NodeId>,
    g3_nodes: &BTreeSet<NodeId>,
) {
    if genius_nodes.is_empty() && g3_nodes.is_empty() {
        return;
    }
    for (c_idx, nodes) in contours.iter_mut().enumerate() {
        let closed = closed_flags.get(c_idx).copied().unwrap_or(true);
        let Some(m_idx) = nodes.iter().position(|n| n.id == moved_node_id) else {
            continue;
        };
        for p_idx in 0..nodes.len() {
            let pid = nodes[p_idx].id;
            let (is_g3, is_g2) = (g3_nodes.contains(&pid), genius_nodes.contains(&pid));
            if !is_g2 && !is_g3 {
                continue;
            }
            let Some(j) = resolve_cubic_join_indices(nodes, closed, p_idx) else {
                continue;
            };
            if m_idx == j.q2 || m_idx == j.r1 {
                let moved_in = m_idx == j.q2;
                if is_g3 {
                    let _ =
                        harmonize_smooth_join_g3_in_contour(nodes, closed, p_idx, Some(moved_in));
                } else {
                    enforce_live_g2_from_handle(nodes, j, moved_in);
                }
            } else if m_idx == j.q1 || m_idx == j.r2 {
                if is_g3 {
                    let _ = harmonize_smooth_join_g3_in_contour(nodes, closed, p_idx, None);
                } else {
                    let _ = harmonize_smooth_join_in_contour(nodes, closed, p_idx, true);
                }
            }
        }
    }
}

fn enforce_live_g2_from_handle(nodes: &mut [Node], j: CubicJoinIndices, moved_in: bool) {
    let p = nodes[j.p].point;
    let (dx, dy) = if moved_in {
        (p.x.0 - nodes[j.q2].point.x.0, p.y.0 - nodes[j.q2].point.y.0)
    } else {
        (nodes[j.r1].point.x.0 - p.x.0, nodes[j.r1].point.y.0 - p.y.0)
    };
    let fixed_len = dx.hypot(dy).max(HARMONIZE_MIN_PERP_DIST_UPM);
    let (ux, uy) = (dx / fixed_len, dy / fixed_len);
    let (nx, ny) = (-uy, ux);
    let d_in = ((nodes[j.q1].point.x.0 - p.x.0) * nx + (nodes[j.q1].point.y.0 - p.y.0) * ny).abs();
    let d_out = ((nodes[j.r2].point.x.0 - p.x.0) * nx + (nodes[j.r2].point.y.0 - p.y.0) * ny).abs();
    if d_in <= HARMONIZE_MIN_PERP_DIST_UPM || d_out <= HARMONIZE_MIN_PERP_DIST_UPM {
        return;
    }
    let p0 = (d_in / d_out)
        .sqrt()
        .clamp(HARMONIZE_RATIO_CLAMP_MIN, HARMONIZE_RATIO_CLAMP_MAX);
    if moved_in {
        nodes[j.r1].point =
            FontPoint::new(p.x.0 + (fixed_len / p0) * ux, p.y.0 + (fixed_len / p0) * uy);
    } else {
        nodes[j.q2].point =
            FontPoint::new(p.x.0 - (fixed_len * p0) * ux, p.y.0 - (fixed_len * p0) * uy);
    }
}
