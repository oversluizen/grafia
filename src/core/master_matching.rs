//! `SPEC-011 §2.4`: `MasterCompatibilityMode`, Trajectory Crossing Detection (`TrajectoryCrossing`),
//! Smooth-Node Interpolation Kink Detector (`KinkWarning`), and Automatic Cross-Master
//! Contour Order, Winding & Cyclic Start-Node Matching (`auto_match_master_compatibility`).

use crate::core::bezier_ops::{compute_signed_area, Node, NodeId, NodeType};
use crate::core::coordinates::FontPoint;
use crate::core::glyph_model::{ContourId, GlyphContour};
use crate::core::master_compatibility::CompanionMasterLayer;
use crate::core::math_consts::{
    KINK_MIN_ANGLE_DEG, KINK_MIN_RATIO_DIFF, RAD_TO_DEG_F64, SOLVER_DET_EPSILON,
};
use crate::core::outline_audit::open_segments_interior_intersection;
use crate::core::segment_slide::reverse_contour_nodes;
use serde::{Deserialize, Serialize};

/// Overlay granularity for cross-master compatibility diagnostics (`SPEC-011 §2.4.1`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum MasterCompatibilityMode {
    #[default]
    OriginAndAnchorRays,
    FullNodeTrajectories,
}

/// Cross-master trajectory intersection indicating a shifted start node or reversed winding (`SPEC-011 §2.4.2`).
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct TrajectoryCrossing {
    pub contour_idx: usize,
    pub companion_layer_id: u32,
    pub node_index_a: usize,
    pub node_index_b: usize,
    pub intersection: FontPoint,
}

/// Smooth-node interpolation kink warning when tangent angle and handle ratio diverge across masters (`SPEC-011 §2.4.3`).
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct KinkWarning {
    pub contour_idx: usize,
    pub node_id: NodeId,
    pub companion_layer_id: u32,
    pub severity: f64,
}

/// Combined report of full node trajectories, crossings, and smooth-node kinks (`SPEC-011 §2.4`).
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct MasterKinkAndCrossingReport {
    pub node_trajectories: Vec<(FontPoint, FontPoint)>,
    pub crossings: Vec<TrajectoryCrossing>,
    pub kink_warnings: Vec<KinkWarning>,
}

/// Computes `MasterKinkAndCrossingReport` across `active_contours` and all `companion_layers` (`SPEC-011 §2.4`).
#[must_use]
pub fn compute_master_kinks_and_crossings_for_layers(
    active_contours: &[GlyphContour],
    companion_layers: &[CompanionMasterLayer],
) -> MasterKinkAndCrossingReport {
    let mut node_trajectories = Vec::new();
    let mut crossings = Vec::new();
    let mut kink_warnings = Vec::new();

    for comp in companion_layers {
        for (ci, act_c) in active_contours.iter().enumerate() {
            let Some(comp_nodes) = comp.contours.get(ci) else {
                continue;
            };
            let comp_closed = comp.closed_flags.get(ci).copied().unwrap_or(true);
            if act_c.nodes.len() != comp_nodes.len() || act_c.closed != comp_closed {
                continue;
            }
            let n = act_c.nodes.len();
            for (act_n, comp_n) in act_c.nodes.iter().zip(comp_nodes.iter()) {
                node_trajectories.push((act_n.point, comp_n.point));
            }
            for j in 0..n {
                for k in (j + 1)..n {
                    if let Some(intersection) = open_segments_interior_intersection(
                        act_c.nodes[j].point,
                        comp_nodes[j].point,
                        act_c.nodes[k].point,
                        comp_nodes[k].point,
                    ) {
                        crossings.push(TrajectoryCrossing {
                            contour_idx: ci,
                            companion_layer_id: comp.id,
                            node_index_a: j,
                            node_index_b: k,
                            intersection,
                        });
                    }
                }
            }
            detect_smooth_node_kinks(
                ci,
                comp.id,
                &act_c.nodes,
                comp_nodes,
                act_c.closed,
                &mut kink_warnings,
            );
        }
    }

    MasterKinkAndCrossingReport {
        node_trajectories,
        crossings,
        kink_warnings,
    }
}

fn detect_smooth_node_kinks(
    contour_idx: usize,
    companion_layer_id: u32,
    act_nodes: &[Node],
    comp_nodes: &[Node],
    closed: bool,
    kink_warnings: &mut Vec<KinkWarning>,
) {
    let n = act_nodes.len();
    if n < 3 {
        return;
    }
    for k in 0..n {
        if act_nodes[k].node_type != NodeType::Smooth || comp_nodes[k].node_type != NodeType::Smooth
        {
            continue;
        }
        let (prev_idx, next_idx) = if closed {
            ((k + n - 1) % n, (k + 1) % n)
        } else {
            if k == 0 || k + 1 >= n {
                continue;
            }
            (k - 1, k + 1)
        };
        if act_nodes[prev_idx].node_type != NodeType::OffCurve
            || act_nodes[next_idx].node_type != NodeType::OffCurve
            || comp_nodes[prev_idx].node_type != NodeType::OffCurve
            || comp_nodes[next_idx].node_type != NodeType::OffCurve
        {
            continue;
        }
        let Some((tau_a, r_a)) = smooth_tangent_and_ratio(
            act_nodes[prev_idx].point,
            act_nodes[k].point,
            act_nodes[next_idx].point,
        ) else {
            continue;
        };
        let Some((tau_b, r_b)) = smooth_tangent_and_ratio(
            comp_nodes[prev_idx].point,
            comp_nodes[k].point,
            comp_nodes[next_idx].point,
        ) else {
            continue;
        };
        let dot = (tau_a.0 * tau_b.0 + tau_a.1 * tau_b.1).clamp(-1.0, 1.0);
        let delta_theta_deg = dot.acos() * RAD_TO_DEG_F64;
        let ratio_diff = (r_a - r_b).abs();
        if delta_theta_deg > KINK_MIN_ANGLE_DEG && ratio_diff > KINK_MIN_RATIO_DIFF {
            kink_warnings.push(KinkWarning {
                contour_idx,
                node_id: act_nodes[k].id,
                companion_layer_id,
                severity: delta_theta_deg * ratio_diff,
            });
        }
    }
}

fn smooth_tangent_and_ratio(
    h_in: FontPoint,
    p: FontPoint,
    h_out: FontPoint,
) -> Option<((f64, f64), f64)> {
    let l_in = h_in.distance_to(p);
    let l_out = h_out.distance_to(p);
    let sum = l_in + l_out;
    let tx = h_out.x.0 - h_in.x.0;
    let ty = h_out.y.0 - h_in.y.0;
    let t_len = tx.hypot(ty);
    if sum <= SOLVER_DET_EPSILON || t_len <= SOLVER_DET_EPSILON {
        return None;
    }
    Some(((tx / t_len, ty / t_len), l_in / sum))
}

fn contour_bbox_center_and_size(nodes: &[Node]) -> ((f64, f64), (f64, f64)) {
    let mut min_x = f64::INFINITY;
    let mut max_x = f64::NEG_INFINITY;
    let mut min_y = f64::INFINITY;
    let mut max_y = f64::NEG_INFINITY;
    for n in nodes {
        min_x = min_x.min(n.point.x.0);
        max_x = max_x.max(n.point.x.0);
        min_y = min_y.min(n.point.y.0);
        max_y = max_y.max(n.point.y.0);
    }
    if !min_x.is_finite() {
        return ((0.0, 0.0), (0.0, 0.0));
    }
    (
        (0.5 * (min_x + max_x), 0.5 * (min_y + max_y)),
        (max_x - min_x, max_y - min_y),
    )
}

/// Deterministically aligns `contours` to `companion` (`SPEC-011 §2.4.4`).
/// Returns `None` if topologically incompatible, or `Some(changed)` if aligned.
pub fn auto_match_contours_to_companion(
    contours: &mut Vec<GlyphContour>,
    companion: &CompanionMasterLayer,
) -> Option<bool> {
    let m = companion.contours.len();
    if contours.len() != m {
        return None;
    }
    debug_assert_eq!(contours.len(), m);
    let before_snapshot: Vec<(Vec<Node>, bool)> = contours
        .iter()
        .map(|c| (c.nodes.clone(), c.closed))
        .collect();

    let mut assigned = vec![false; m];
    let mut ordered: Vec<GlyphContour> = Vec::with_capacity(m);

    for j in 0..m {
        let ref_nodes = &companion.contours[j];
        let ref_closed = companion.closed_flags.get(j).copied().unwrap_or(true);
        let (ref_c, ref_s) = contour_bbox_center_and_size(ref_nodes);
        let mut best: Option<(usize, GlyphContour, (f64, f64))> = None;
        for (cand_idx, cand) in contours.iter().enumerate() {
            if assigned[cand_idx]
                || cand.nodes.len() != ref_nodes.len()
                || cand.closed != ref_closed
            {
                continue;
            }
            let mut aligned_cand = cand.clone();
            aligned_cand.id = ContourId(j);
            if align_contour_winding_and_start_node(&mut aligned_cand, ref_nodes).is_none() {
                continue;
            }
            let (cand_c, cand_s) = contour_bbox_center_and_size(&cand.nodes);
            let dc_x = cand_c.0 - ref_c.0;
            let dc_y = cand_c.1 - ref_c.1;
            let ds_x = cand_s.0 - ref_s.0;
            let ds_y = cand_s.1 - ref_s.1;
            let bbox_cost = dc_x * dc_x + dc_y * dc_y + ds_x * ds_x + ds_y * ds_y;
            let node_cost: f64 = aligned_cand
                .nodes
                .iter()
                .zip(ref_nodes.iter())
                .map(|(a, b)| {
                    let dx = a.point.x.0 - b.point.x.0;
                    let dy = a.point.y.0 - b.point.y.0;
                    dx * dx + dy * dy
                })
                .sum();
            let key = (bbox_cost, node_cost);
            if best.as_ref().is_none_or(|(_, _, b_key)| key < *b_key) {
                best = Some((cand_idx, aligned_cand, key));
            }
        }
        let (chosen_idx, matched_contour, _) = best?;
        assigned[chosen_idx] = true;
        ordered.push(matched_contour);
    }

    *contours = ordered;
    let changed = contours
        .iter()
        .zip(before_snapshot.iter())
        .any(|(c, (old_nodes, old_closed))| &c.nodes != old_nodes || c.closed != *old_closed);
    Some(changed)
}

fn align_contour_winding_and_start_node(
    contour: &mut GlyphContour,
    ref_nodes: &[Node],
) -> Option<()> {
    let n = contour.nodes.len();
    if n == 0 {
        return Some(());
    }
    if contour.closed {
        let pts_a: Vec<FontPoint> = contour.nodes.iter().map(|nd| nd.point).collect();
        let pts_b: Vec<FontPoint> = ref_nodes.iter().map(|nd| nd.point).collect();
        let area_a = compute_signed_area(&pts_a);
        let area_b = compute_signed_area(&pts_b);
        if area_a.abs() > SOLVER_DET_EPSILON
            && area_b.abs() > SOLVER_DET_EPSILON
            && area_a.signum() != area_b.signum()
        {
            reverse_contour_nodes(&mut contour.nodes, true);
        }
        let mut best_shift: Option<(usize, f64)> = None;
        for k in 0..n {
            if contour.nodes[k].node_type == NodeType::OffCurve {
                continue;
            }
            let pattern_ok = (0..n).all(|i| {
                (contour.nodes[(i + k) % n].node_type == NodeType::OffCurve)
                    == (ref_nodes[i].node_type == NodeType::OffCurve)
            });
            if !pattern_ok {
                continue;
            }
            let mut sq_dist = 0.0;
            for (i, ref_n) in ref_nodes.iter().enumerate().take(n) {
                let pa = contour.nodes[(i + k) % n].point;
                let qb = ref_n.point;
                let dx = pa.x.0 - qb.x.0;
                let dy = pa.y.0 - qb.y.0;
                sq_dist += dx * dx + dy * dy;
            }
            if best_shift.is_none_or(|(_, b_dist)| sq_dist < b_dist) {
                best_shift = Some((k, sq_dist));
            }
        }
        let (k_star, _) = best_shift?;
        if k_star > 0 {
            contour.nodes.rotate_left(k_star);
        }
    } else {
        let fwd_ok = (0..n).all(|i| {
            (contour.nodes[i].node_type == NodeType::OffCurve)
                == (ref_nodes[i].node_type == NodeType::OffCurve)
        });
        let mut rev_nodes = contour.nodes.clone();
        reverse_contour_nodes(&mut rev_nodes, false);
        let rev_ok = (0..n).all(|i| {
            (rev_nodes[i].node_type == NodeType::OffCurve)
                == (ref_nodes[i].node_type == NodeType::OffCurve)
        });
        if !fwd_ok && !rev_ok {
            return None;
        }
        let seq_cost = |seq: &[Node]| -> f64 {
            seq.iter()
                .zip(ref_nodes.iter())
                .map(|(a, b)| {
                    let dx = a.point.x.0 - b.point.x.0;
                    let dy = a.point.y.0 - b.point.y.0;
                    dx * dx + dy * dy
                })
                .sum()
        };
        if rev_ok && (!fwd_ok || seq_cost(&rev_nodes) < seq_cost(&contour.nodes)) {
            contour.nodes = rev_nodes;
        }
    }
    Some(())
}
