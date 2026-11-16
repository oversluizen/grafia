//! `SPEC-011 §2.1 & §2.8`: Parametric Cubic Curvature $\kappa(t)$,
//! Romer/Henestrosa $G^2$ & $G^3$ Clothoid Harmonization (`Ctrl+Shift+H`),
//! Live `genius_nodes` / `g3_nodes` Constraints, and 4-Level Continuity Classification.

use crate::core::bezier_ops::{Node, NodeId, NodeType};
use crate::core::coordinates::FontPoint;
use crate::core::curvature_heatmap::CurvatureCombStyle;
use crate::core::curved_trajectory::CurvedTrajectoryLink;
use crate::core::master_matching::MasterCompatibilityMode;
use crate::core::math_consts::{
    BERNSTEIN_CUBIC_COEFF, BERNSTEIN_SECOND_DERIV_COEFF, CLEANER_DEFAULT_MAX_LOOP_AREA_UPM2,
    CONTINUITY_G3_MAX_DKAPPA_DS_DIFF, G3_CLOTHOID_COEFF_NINE, G3_CLOTHOID_COEFF_SIX,
    G3_MAX_HANDLE_LEN_UPM, G3_MIN_HANDLE_LEN_UPM, HARMONIZE_MIN_PERP_DIST_UPM,
    HARMONIZE_RATIO_CLAMP_MAX, HARMONIZE_RATIO_CLAMP_MIN, SOLVER_DET_EPSILON,
    TROWEL_DEFAULT_TOLERANCE_UPM,
};
use crate::core::optical_rhythm::ActiveMirrorAxis;
use crate::core::smart_corners_brush::SmartCornerSpec;
use crate::core::stem_gauge_and_balance::SymmetryOverlayMode;
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;

/// Persistent and session state for `SPEC-011`..`SPEC-013` curve harmonization, structural stems, trajectories, brushes, and optical rhythm.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CurveHarmonyState {
    pub genius_nodes: BTreeSet<NodeId>,
    pub g3_nodes: BTreeSet<NodeId>,
    pub servo_scaling_enabled: bool,
    pub master_compatibility_mode: MasterCompatibilityMode,
    pub show_curvature_comb: bool,
    pub show_outline_audit: bool,
    pub smart_corners: Vec<SmartCornerSpec>,
    pub curvature_comb_style: CurvatureCombStyle,
    pub reference_stems_upm: Vec<f64>,
    pub stem_tolerance_upm: f64,
    pub symmetry_overlay_mode: SymmetryOverlayMode,
    pub show_measurement_rainbow: bool,
    pub curved_trajectories: Vec<CurvedTrajectoryLink>,
    pub quick_preview_on_drag: bool,
    pub round_overshoot_upm: Option<f64>,
    pub show_optical_rhythm_fill: bool,
    pub active_mirror_axis: ActiveMirrorAxis,
    pub trowel_fit_tolerance_upm: f64,
    pub cleaner_max_loop_area_upm2: f64,
    #[serde(default)]
    pub active_tangent_snap: Option<crate::core::diagonal_and_ratio_hud::TangentExtensionSnap>,
    #[serde(default)]
    pub show_stem_gauge: bool,
    #[serde(default)]
    pub show_visual_centroid: bool,
}

impl Default for CurveHarmonyState {
    fn default() -> Self {
        Self {
            genius_nodes: BTreeSet::new(),
            g3_nodes: BTreeSet::new(),
            servo_scaling_enabled: false,
            master_compatibility_mode: MasterCompatibilityMode::default(),
            show_curvature_comb: false,
            show_outline_audit: false,
            smart_corners: Vec::new(),
            curvature_comb_style: CurvatureCombStyle::default(),
            reference_stems_upm: Vec::new(),
            stem_tolerance_upm: 0.0,
            symmetry_overlay_mode: SymmetryOverlayMode::default(),
            show_measurement_rainbow: false,
            curved_trajectories: Vec::new(),
            quick_preview_on_drag: false,
            round_overshoot_upm: None,
            show_optical_rhythm_fill: false,
            active_mirror_axis: ActiveMirrorAxis::None,
            trowel_fit_tolerance_upm: TROWEL_DEFAULT_TOLERANCE_UPM,
            cleaner_max_loop_area_upm2: CLEANER_DEFAULT_MAX_LOOP_AREA_UPM2,
            active_tangent_snap: None,
            show_stem_gauge: false,
            show_visual_centroid: false,
        }
    }
}

pub(crate) fn cubic_derivatives(
    p0: FontPoint,
    p1: FontPoint,
    p2: FontPoint,
    p3: FontPoint,
    t: f64,
) -> ((f64, f64), (f64, f64), (f64, f64)) {
    let (u, c3, c6) = (1.0 - t, BERNSTEIN_CUBIC_COEFF, BERNSTEIN_SECOND_DERIV_COEFF);
    let dx = c3 * u * u * (p1.x.0 - p0.x.0)
        + c6 * u * t * (p2.x.0 - p1.x.0)
        + c3 * t * t * (p3.x.0 - p2.x.0);
    let dy = c3 * u * u * (p1.y.0 - p0.y.0)
        + c6 * u * t * (p2.y.0 - p1.y.0)
        + c3 * t * t * (p3.y.0 - p2.y.0);
    let ddx = c6 * u * (p2.x.0 - 2.0 * p1.x.0 + p0.x.0) + c6 * t * (p3.x.0 - 2.0 * p2.x.0 + p1.x.0);
    let ddy = c6 * u * (p2.y.0 - 2.0 * p1.y.0 + p0.y.0) + c6 * t * (p3.y.0 - 2.0 * p2.y.0 + p1.y.0);
    let dddx = c6 * (p3.x.0 - c3 * p2.x.0 + c3 * p1.x.0 - p0.x.0);
    let dddy = c6 * (p3.y.0 - c3 * p2.y.0 + c3 * p1.y.0 - p0.y.0);
    ((dx, dy), (ddx, ddy), (dddx, dddy))
}

/// Evaluates the signed parametric curvature $\kappa(t)$ of cubic Bézier $(P_0, P_1, P_2, P_3)$ at $t \in [0, 1]$ (`SPEC-011 §2.1`).
#[must_use]
pub fn cubic_signed_curvature(
    p0: FontPoint,
    p1: FontPoint,
    p2: FontPoint,
    p3: FontPoint,
    t: f64,
) -> f64 {
    debug_assert!(t.is_finite());
    let ((dx, dy), (ddx, ddy), _) = cubic_derivatives(p0, p1, p2, p3, t);
    let speed = dx.hypot(dy);
    if speed <= SOLVER_DET_EPSILON {
        return 0.0;
    }
    (dx * ddy - dy * ddx) / (speed * speed * speed)
}

/// Evaluates the arc-length rate of change of curvature $d\kappa / ds$ at $t \in [0, 1]$ (`SPEC-011 §2.8`).
#[must_use]
pub fn cubic_dkappa_ds(p0: FontPoint, p1: FontPoint, p2: FontPoint, p3: FontPoint, t: f64) -> f64 {
    debug_assert!(t.is_finite());
    let ((dx, dy), (ddx, ddy), (dddx, dddy)) = cubic_derivatives(p0, p1, p2, p3, t);
    let v2 = dx * dx + dy * dy;
    if v2 <= SOLVER_DET_EPSILON {
        return 0.0;
    }
    let cross12 = dx * ddy - dy * ddx;
    let cross13 = dx * dddy - dy * dddx;
    let dot12 = dx * ddx + dy * ddy;
    (v2 * cross13 - BERNSTEIN_CUBIC_COEFF * cross12 * dot12) / (v2 * v2 * v2)
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct CubicJoinIndices {
    pub q0: usize,
    pub q1: usize,
    pub q2: usize,
    pub p: usize,
    pub r1: usize,
    pub r2: usize,
    pub r3: usize,
}

pub(crate) fn resolve_cubic_join_indices(
    nodes: &[Node],
    closed: bool,
    p_idx: usize,
) -> Option<CubicJoinIndices> {
    let n = nodes.len();
    if n < 7 || p_idx >= n || nodes[p_idx].node_type == NodeType::OffCurve {
        return None;
    }
    let idx = |off: isize| ((p_idx as isize + off).rem_euclid(n as isize)) as usize;
    let [q0, q1, q2, r1, r2, r3] = if closed {
        [idx(-3), idx(-2), idx(-1), idx(1), idx(2), idx(3)]
    } else if p_idx >= 3 && p_idx + 3 < n {
        [
            p_idx - 3,
            p_idx - 2,
            p_idx - 1,
            p_idx + 1,
            p_idx + 2,
            p_idx + 3,
        ]
    } else {
        return None;
    };
    let is_off = |i: usize| nodes[i].node_type == NodeType::OffCurve;
    if is_off(q0) || !is_off(q1) || !is_off(q2) || !is_off(r1) || !is_off(r2) || is_off(r3) {
        return None;
    }
    Some(CubicJoinIndices {
        q0,
        q1,
        q2,
        p: p_idx,
        r1,
        r2,
        r3,
    })
}

/// Harmonizes a smooth cubic join at `p_idx` in `nodes` (`keep_anchor == false` moves $P$; `keep_anchor == true` adjusts handles).
pub fn harmonize_smooth_join_in_contour(
    nodes: &mut [Node],
    closed: bool,
    p_idx: usize,
    keep_anchor: bool,
) -> bool {
    let Some(j) = resolve_cubic_join_indices(nodes, closed, p_idx) else {
        return false;
    };
    if nodes[j.p].node_type != NodeType::Smooth {
        return false;
    }
    let p = nodes[j.p].point;
    let q2 = nodes[j.q2].point;
    let r1 = nodes[j.r1].point;
    let mut tx = r1.x.0 - q2.x.0;
    let mut ty = r1.y.0 - q2.y.0;
    if tx.hypot(ty) <= HARMONIZE_MIN_PERP_DIST_UPM {
        tx = p.x.0 - q2.x.0;
        ty = p.y.0 - q2.y.0;
    }
    let t_len = tx.hypot(ty);
    if t_len <= HARMONIZE_MIN_PERP_DIST_UPM {
        return false;
    }
    let (ux, uy) = (tx / t_len, ty / t_len);
    let (nx, ny) = (-uy, ux);
    let a_old = p.distance_to(q2).max(HARMONIZE_MIN_PERP_DIST_UPM);
    let b_old = p.distance_to(r1).max(HARMONIZE_MIN_PERP_DIST_UPM);
    let q2_col = FontPoint::new(p.x.0 - a_old * ux, p.y.0 - a_old * uy);
    let r1_col = FontPoint::new(p.x.0 + b_old * ux, p.y.0 + b_old * uy);
    let d_in = ((nodes[j.q1].point.x.0 - p.x.0) * nx + (nodes[j.q1].point.y.0 - p.y.0) * ny).abs();
    let d_out = ((nodes[j.r2].point.x.0 - p.x.0) * nx + (nodes[j.r2].point.y.0 - p.y.0) * ny).abs();
    if d_in <= HARMONIZE_MIN_PERP_DIST_UPM || d_out <= HARMONIZE_MIN_PERP_DIST_UPM {
        return false;
    }
    let p0 = (d_in / d_out)
        .sqrt()
        .clamp(HARMONIZE_RATIO_CLAMP_MIN, HARMONIZE_RATIO_CLAMP_MAX);
    if keep_anchor {
        let prod = a_old * b_old;
        let (a_new, b_new) = ((prod * p0).sqrt(), (prod / p0).sqrt());
        nodes[j.q2].point = FontPoint::new(p.x.0 - a_new * ux, p.y.0 - a_new * uy);
        nodes[j.r1].point = FontPoint::new(p.x.0 + b_new * ux, p.y.0 + b_new * uy);
    } else {
        nodes[j.q2].point = q2_col;
        nodes[j.r1].point = r1_col;
        let denom = 1.0 + p0;
        nodes[j.p].point = FontPoint::new(
            (q2_col.x.0 + p0 * r1_col.x.0) / denom,
            (q2_col.y.0 + p0 * r1_col.y.0) / denom,
        );
    }
    true
}

/// Solves for $G^3$ (`G3Clothoid`) continuity at smooth join `p_idx` so both $\Delta\kappa = 0$ and $\Delta(d\kappa/ds) = 0$.
pub fn harmonize_smooth_join_g3_in_contour(
    nodes: &mut [Node],
    closed: bool,
    p_idx: usize,
    fixed_in_handle: Option<bool>,
) -> bool {
    let Some(j) = resolve_cubic_join_indices(nodes, closed, p_idx) else {
        return false;
    };
    if nodes[j.p].node_type != NodeType::Smooth {
        return false;
    }
    let p = nodes[j.p].point;
    let q2 = nodes[j.q2].point;
    let r1 = nodes[j.r1].point;
    let (dx, dy) = match fixed_in_handle {
        Some(true) => (p.x.0 - q2.x.0, p.y.0 - q2.y.0),
        Some(false) => (r1.x.0 - p.x.0, r1.y.0 - p.y.0),
        None => (r1.x.0 - q2.x.0, r1.y.0 - q2.y.0),
    };
    let len = dx.hypot(dy).max(HARMONIZE_MIN_PERP_DIST_UPM);
    let (ux, uy, nx, ny) = (dx / len, dy / len, -dy / len, dx / len);
    let to_local = |pt: FontPoint| {
        (
            (pt.x.0 - p.x.0) * ux + (pt.y.0 - p.y.0) * uy,
            (pt.x.0 - p.x.0) * nx + (pt.y.0 - p.y.0) * ny,
        )
    };
    let (_, eta_q0) = to_local(nodes[j.q0].point);
    let (xi_q1, eta_q1) = to_local(nodes[j.q1].point);
    let (xi_r2, eta_r2) = to_local(nodes[j.r2].point);
    let (_, eta_r3) = to_local(nodes[j.r3].point);
    if eta_q1.abs() <= HARMONIZE_MIN_PERP_DIST_UPM || eta_r2.abs() <= HARMONIZE_MIN_PERP_DIST_UPM {
        return false;
    }
    let p0 = (eta_q1.abs() / eta_r2.abs())
        .sqrt()
        .clamp(HARMONIZE_RATIO_CLAMP_MIN, HARMONIZE_RATIO_CLAMP_MAX);
    let (a_new, b_new) = solve_g3_handle_lengths(
        p,
        q2,
        r1,
        p0,
        [eta_q0, xi_q1, eta_q1, xi_r2, eta_r2, eta_r3],
        fixed_in_handle,
    );
    nodes[j.q2].point = FontPoint::new(p.x.0 - a_new * ux, p.y.0 - a_new * uy);
    nodes[j.r1].point = FontPoint::new(p.x.0 + b_new * ux, p.y.0 + b_new * uy);

    let (q0p, q1p, q2p, r1p, r2p, r3p) = (
        nodes[j.q0].point,
        nodes[j.q1].point,
        nodes[j.q2].point,
        nodes[j.r1].point,
        nodes[j.r2].point,
        nodes[j.r3].point,
    );
    let dk_in = cubic_dkappa_ds(q0p, q1p, q2p, p, 1.0);
    let dk_out = cubic_dkappa_ds(p, r1p, r2p, r3p, 0.0);
    if (dk_in - dk_out).abs() > CONTINUITY_G3_MAX_DKAPPA_DS_DIFF * 0.5 {
        let p0_3 = p0 * p0 * p0;
        let term_in = (-eta_q0
            - G3_CLOTHOID_COEFF_NINE * eta_q1
            - (G3_CLOTHOID_COEFF_SIX * eta_q1 * xi_q1) / a_new)
            / p0_3;
        let xi_r2_t = b_new * (eta_r3 + G3_CLOTHOID_COEFF_NINE * eta_r2 - term_in)
            / (G3_CLOTHOID_COEFF_SIX * eta_r2);
        if xi_r2_t.is_finite() {
            nodes[j.r2].point = FontPoint::new(
                p.x.0 + xi_r2_t * ux + eta_r2 * nx,
                p.y.0 + xi_r2_t * uy + eta_r2 * ny,
            );
        }
    }
    true
}

fn solve_g3_handle_lengths(
    p: FontPoint,
    q2: FontPoint,
    r1: FontPoint,
    p0: f64,
    loc: [f64; 6],
    fixed_in_handle: Option<bool>,
) -> (f64, f64) {
    let [eta_q0, xi_q1, eta_q1, xi_r2, eta_r2, eta_r3] = loc;
    let clamp_len = |d: f64| d.clamp(G3_MIN_HANDLE_LEN_UPM, G3_MAX_HANDLE_LEN_UPM);
    match fixed_in_handle {
        Some(true) => {
            let a = clamp_len(p.distance_to(q2));
            (a, a / p0)
        }
        Some(false) => {
            let b = clamp_len(p.distance_to(r1));
            (p0 * b, b)
        }
        None => {
            let p0_3 = p0 * p0 * p0;
            let lhs = G3_CLOTHOID_COEFF_SIX * eta_r2 * xi_r2
                - (G3_CLOTHOID_COEFF_SIX * eta_q1 * xi_q1) / (p0_3 * p0);
            let rhs = (eta_r3 + G3_CLOTHOID_COEFF_NINE * eta_r2)
                + (eta_q0 + G3_CLOTHOID_COEFF_NINE * eta_q1) / p0_3;
            if lhs.abs() > SOLVER_DET_EPSILON && (lhs / rhs) > 0.0 {
                let b = clamp_len(lhs / rhs);
                (p0 * b, b)
            } else {
                let prod = p.distance_to(q2).max(G3_MIN_HANDLE_LEN_UPM)
                    * p.distance_to(r1).max(G3_MIN_HANDLE_LEN_UPM);
                ((prod * p0).sqrt(), (prod / p0).sqrt())
            }
        }
    }
}
