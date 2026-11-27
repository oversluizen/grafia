//! `SPEC-009 §2.1` Clothoid Euler/Spiro spline solver (`NodeType::EulerSpiro`).
//!
//! Solves 2-parameter spiral arcs $\theta(u) = a_0 + k_0 u + \frac{1}{2}k_1 u^2$ on $u \in [-0.5, 0.5]$
//! via 5-point Gauss-Legendre quadrature and damped Newton-Raphson $G^2$ curvature matching,
//! then fits cubic Bézier control points matching endpoint tangents and clothoid midpoint.

use crate::core::bezier_ops::{Node, NodeType};
use crate::core::canvas_entities::HobbyDirConstraint;
use crate::core::hobby_spline::{
    constrained_anchor_world_angle, evaluate_hobby_chain_to_cubics, extract_anchor_spans,
    hobby_velocity_rho, solve_linear_system, wrap_angle_pi, SolvedSplineSegment,
};
use crate::core::math_consts::{
    EULER_CHAIN_NEWTON_ITERS, EULER_CONVERGENCE_TOL, EULER_DAMPING_STEP, EULER_FD_STEP,
    EULER_GL5_NODES, EULER_GL5_WEIGHTS, EULER_K1_EIGHTH, EULER_K1_NEWTON_ITERS,
    EULER_MID_EIGHT_THIRDS, EULER_PAR_DET_EPS, HOBBY_HANDLE_DIVISOR, HOBBY_MIN_CHORD_EPS,
    HOBBY_SOLVER_PIVOT_EPS, HOBBY_TENSION_DEFAULT,
};
use kurbo::{CubicBez, Point};

#[derive(Debug, Clone, Copy)]
struct ClothoidArc {
    a0: f64,
    k0: f64,
    k1: f64,
    arc_len: f64,
    kappa_out: f64,
    kappa_in: f64,
}

fn solve_clothoid_arc(theta: f64, phi: f64, chord_len: f64) -> ClothoidArc {
    debug_assert!(theta.is_finite() && phi.is_finite());
    if chord_len <= HOBBY_MIN_CHORD_EPS {
        return ClothoidArc {
            a0: 0.0,
            k0: 0.0,
            k1: 0.0,
            arc_len: 0.0,
            kappa_out: 0.0,
            kappa_in: 0.0,
        };
    }
    let k0 = -(theta + phi);
    let half_diff = 0.5 * (theta - phi);
    let mut k1 = 2.0 * HOBBY_HANDLE_DIVISOR * (theta - phi);

    for _ in 0..EULER_K1_NEWTON_ITERS {
        let a0 = half_diff - EULER_K1_EIGHTH * k1;
        let mut f_val = 0.0;
        let mut df_val = 0.0;
        for (u, w) in EULER_GL5_NODES.into_iter().zip(EULER_GL5_WEIGHTS) {
            let ang = a0 + k0 * u + 0.5 * k1 * u * u;
            let d_ang_dk1 = 0.5 * u * u - EULER_K1_EIGHTH;
            f_val += w * ang.sin();
            df_val += w * ang.cos() * d_ang_dk1;
        }
        if df_val.abs() <= HOBBY_SOLVER_PIVOT_EPS {
            break;
        }
        let step = f_val / df_val;
        k1 -= step;
        if step.abs() <= EULER_CONVERGENCE_TOL {
            break;
        }
    }

    let a0 = half_diff - EULER_K1_EIGHTH * k1;
    let mut cos_int = 0.0;
    for (u, w) in EULER_GL5_NODES.into_iter().zip(EULER_GL5_WEIGHTS) {
        let ang = a0 + k0 * u + 0.5 * k1 * u * u;
        cos_int += w * ang.cos();
    }
    let safe_cos = if cos_int.abs() <= HOBBY_MIN_CHORD_EPS {
        1.0
    } else {
        cos_int.abs()
    };
    let arc_len = chord_len / safe_cos;
    let kappa_out = (k0 - 0.5 * k1) / arc_len;
    let kappa_in = (k0 + 0.5 * k1) / arc_len;
    ClothoidArc {
        a0,
        k0,
        k1,
        arc_len,
        kappa_out,
        kappa_in,
    }
}

fn clothoid_to_cubic(
    p_start: Point,
    p_end: Point,
    psi: f64,
    chord_len: f64,
    theta: f64,
    phi: f64,
    arc: ClothoidArc,
) -> CubicBez {
    if chord_len <= HOBBY_MIN_CHORD_EPS {
        return CubicBez::new(p_start, p_start, p_end, p_end);
    }
    let out_ang = psi + theta;
    let in_ang = psi - phi;
    let t0 = (out_ang.cos(), out_ang.sin());
    let t1 = (in_ang.cos(), in_ang.sin());

    let mut mid_local_x = 0.0;
    let mut mid_local_y = 0.0;
    for (xi, w) in EULER_GL5_NODES.into_iter().zip(EULER_GL5_WEIGHTS) {
        let u = 0.5 * (xi - 0.5);
        let weight = 0.5 * w;
        let ang = arc.a0 + arc.k0 * u + 0.5 * arc.k1 * u * u;
        mid_local_x += weight * arc.arc_len * ang.cos();
        mid_local_y += weight * arc.arc_len * ang.sin();
    }

    let (cp, sp) = (psi.cos(), psi.sin());
    let mid_world_x = p_start.x + mid_local_x * cp - mid_local_y * sp;
    let mid_world_y = p_start.y + mid_local_x * sp + mid_local_y * cp;

    let chord_mid_x = 0.5 * (p_start.x + p_end.x);
    let chord_mid_y = 0.5 * (p_start.y + p_end.y);
    let rhs_x = EULER_MID_EIGHT_THIRDS * (mid_world_x - chord_mid_x);
    let rhs_y = EULER_MID_EIGHT_THIRDS * (mid_world_y - chord_mid_y);

    let det = -t0.0 * t1.1 + t0.1 * t1.0;
    let (r1, r2) = if det.abs() > EULER_PAR_DET_EPS {
        let cand_r1 = (-rhs_x * t1.1 + rhs_y * t1.0) / det;
        let cand_r2 = (t0.0 * rhs_y - t0.1 * rhs_x) / det;
        if cand_r1 > 0.0 && cand_r2 > 0.0 {
            (cand_r1, cand_r2)
        } else {
            let l1 = chord_len * hobby_velocity_rho(theta, phi) / HOBBY_HANDLE_DIVISOR;
            let l2 = chord_len * hobby_velocity_rho(phi, theta) / HOBBY_HANDLE_DIVISOR;
            (l1, l2)
        }
    } else {
        let l1 = chord_len * hobby_velocity_rho(theta, phi) / HOBBY_HANDLE_DIVISOR;
        let l2 = chord_len * hobby_velocity_rho(phi, theta) / HOBBY_HANDLE_DIVISOR;
        (l1, l2)
    };

    let h1 = Point::new(p_start.x + r1 * t0.0, p_start.y + r1 * t0.1);
    let h2 = Point::new(p_end.x - r2 * t1.0, p_end.y - r2 * t1.1);
    CubicBez::new(p_start, h1, h2, p_end)
}

#[derive(Clone, Copy)]
struct EulerSpanSpec {
    start_a: usize,
    end_a: usize,
    start_idx: usize,
    end_idx: usize,
    p_start: Point,
    p_end: Point,
    chord_len: f64,
    psi: f64,
}

struct EulerAnchorAngleInit {
    anchor_angles: Vec<f64>,
    free_anchors: Vec<usize>,
    inc_euler: Vec<Option<usize>>,
    out_euler: Vec<Option<usize>>,
}

fn init_euler_anchor_angles(
    nodes: &[Node],
    anchors: &[usize],
    spans: &[(usize, usize, Vec<usize>)],
    euler_spans: &[EulerSpanSpec],
    hobby_seed: &[SolvedSplineSegment],
    view: (bool, f64),
    metrics_y: &[f64],
) -> EulerAnchorAngleInit {
    let (closed, italic_angle_deg) = view;
    let m = anchors.len();
    let mut anchor_angles = vec![0.0; m];
    let mut anchor_fixed = vec![true; m];
    let mut inc_euler = vec![None; m];
    let mut out_euler = vec![None; m];
    for (es_idx, es) in euler_spans.iter().enumerate() {
        out_euler[es.start_a] = Some(es_idx);
        inc_euler[es.end_a] = Some(es_idx);
    }

    for a_pos in 0..m {
        if inc_euler[a_pos].is_none() && out_euler[a_pos].is_none() {
            continue;
        }
        if let Some(fixed_ang) = constrained_anchor_world_angle(
            nodes,
            anchors,
            spans,
            a_pos,
            closed,
            italic_angle_deg,
            metrics_y,
        ) {
            anchor_angles[a_pos] = fixed_ang;
            anchor_fixed[a_pos] = true;
        } else {
            let a_idx = anchors[a_pos];
            let is_free_joint = matches!(
                nodes[a_idx].node_type,
                NodeType::EulerSpiro {
                    dir_constraint: HobbyDirConstraint::Free
                } | NodeType::Smooth
            );
            anchor_fixed[a_pos] = !is_free_joint;
            if let Some(es_idx) = out_euler[a_pos] {
                let es = &euler_spans[es_idx];
                anchor_angles[a_pos] = hobby_seed
                    .iter()
                    .find(|h| h.start_idx == es.start_idx && h.end_idx == es.end_idx)
                    .map_or(es.psi, |hs| wrap_angle_pi(es.psi + hs.theta));
            } else if let Some(es_idx) = inc_euler[a_pos] {
                let es = &euler_spans[es_idx];
                anchor_angles[a_pos] = hobby_seed
                    .iter()
                    .find(|h| h.start_idx == es.start_idx && h.end_idx == es.end_idx)
                    .map_or(es.psi, |hs| wrap_angle_pi(es.psi - hs.phi));
            }
        }
    }

    let free_anchors: Vec<usize> = (0..m)
        .filter(|&a_pos| {
            !anchor_fixed[a_pos] && (inc_euler[a_pos].is_some() || out_euler[a_pos].is_some())
        })
        .collect();
    EulerAnchorAngleInit {
        anchor_angles,
        free_anchors,
        inc_euler,
        out_euler,
    }
}

fn refine_euler_angles_newton(
    anchor_angles: &mut [f64],
    free_anchors: &[usize],
    euler_spans: &[EulerSpanSpec],
    inc_euler: &[Option<usize>],
    out_euler: &[Option<usize>],
) {
    let eval_residuals = |angs: &[f64]| -> Vec<f64> {
        let arcs: Vec<ClothoidArc> = euler_spans
            .iter()
            .map(|es| {
                let th = wrap_angle_pi(angs[es.start_a] - es.psi);
                let ph = wrap_angle_pi(es.psi - angs[es.end_a]);
                solve_clothoid_arc(th, ph, es.chord_len)
            })
            .collect();
        free_anchors
            .iter()
            .map(|&a_pos| match (inc_euler[a_pos], out_euler[a_pos]) {
                (Some(p), Some(s)) => arcs[p].kappa_in - arcs[s].kappa_out,
                (None, Some(s)) => arcs[s].kappa_out,
                (Some(p), None) => arcs[p].kappa_in,
                (None, None) => 0.0,
            })
            .collect()
    };

    let nf = free_anchors.len();
    for _ in 0..EULER_CHAIN_NEWTON_ITERS {
        let r0 = eval_residuals(anchor_angles);
        let max_r = r0.iter().fold(0.0_f64, |acc, v| acc.max(v.abs()));
        if max_r <= EULER_CONVERGENCE_TOL {
            break;
        }
        let zero_row = vec![0.0; nf];
        let mut jac = vec![zero_row; nf];
        for (col, &a_pos) in free_anchors.iter().enumerate() {
            let mut perturbed = anchor_angles.to_vec();
            perturbed[a_pos] += EULER_FD_STEP;
            let r1 = eval_residuals(&perturbed);
            for row in 0..nf {
                jac[row][col] = (r1[row] - r0[row]) / EULER_FD_STEP;
            }
        }
        let mut neg_r: Vec<f64> = r0.iter().map(|v| -*v).collect();
        let Some(delta) = solve_linear_system(&mut jac, &mut neg_r) else {
            break;
        };
        let mut max_step = 0.0_f64;
        for (col, &a_pos) in free_anchors.iter().enumerate() {
            let step = EULER_DAMPING_STEP * delta[col];
            anchor_angles[a_pos] = wrap_angle_pi(anchor_angles[a_pos] + step);
            max_step = max_step.max(step.abs());
        }
        if max_step <= EULER_CONVERGENCE_TOL {
            break;
        }
    }
}

fn collect_euler_span_specs(
    nodes: &[Node],
    anchors: &[usize],
    spans: &[(usize, usize, Vec<usize>)],
) -> Vec<EulerSpanSpec> {
    let mut euler_spans = Vec::new();
    for (s_pos, e_pos, off) in spans {
        if !off.is_empty() {
            continue;
        }
        let (s_idx, e_idx) = (anchors[*s_pos], anchors[*e_pos]);
        let has_hobby = matches!(nodes[s_idx].node_type, NodeType::HobbyStar { .. })
            || matches!(nodes[e_idx].node_type, NodeType::HobbyStar { .. });
        let has_euler = matches!(nodes[s_idx].node_type, NodeType::EulerSpiro { .. })
            || matches!(nodes[e_idx].node_type, NodeType::EulerSpiro { .. });
        if !has_hobby && has_euler {
            let p_start = Point::new(nodes[s_idx].point.x.0, nodes[s_idx].point.y.0);
            let p_end = Point::new(nodes[e_idx].point.x.0, nodes[e_idx].point.y.0);
            let (dx, dy) = (p_end.x - p_start.x, p_end.y - p_start.y);
            euler_spans.push(EulerSpanSpec {
                start_a: *s_pos,
                end_a: *e_pos,
                start_idx: s_idx,
                end_idx: e_idx,
                p_start,
                p_end,
                chord_len: dx.hypot(dy),
                psi: dy.atan2(dx),
            });
        }
    }
    euler_spans
}

#[must_use]
pub(crate) fn solve_euler_spiro_chain(
    nodes: &[Node],
    closed: bool,
    italic_angle_deg: f64,
    metrics_y: &[f64],
) -> Vec<SolvedSplineSegment> {
    let (anchors, spans) = extract_anchor_spans(nodes, closed);
    if anchors.len() < 2 {
        return Vec::new();
    }
    let euler_spans = collect_euler_span_specs(nodes, &anchors, &spans);
    if euler_spans.is_empty() {
        return Vec::new();
    }

    let seed_nodes: Vec<Node> = nodes
        .iter()
        .map(|n| {
            let mut copy = n.clone();
            if let NodeType::EulerSpiro { dir_constraint } = n.node_type {
                copy.node_type = NodeType::HobbyStar {
                    tension_in: HOBBY_TENSION_DEFAULT,
                    tension_out: HOBBY_TENSION_DEFAULT,
                    dir_constraint,
                };
            }
            copy
        })
        .collect();
    let hobby_seed =
        evaluate_hobby_chain_to_cubics(&seed_nodes, closed, italic_angle_deg, metrics_y);
    let mut init = init_euler_anchor_angles(
        nodes,
        &anchors,
        &spans,
        &euler_spans,
        &hobby_seed,
        (closed, italic_angle_deg),
        metrics_y,
    );
    if !init.free_anchors.is_empty() {
        refine_euler_angles_newton(
            &mut init.anchor_angles,
            &init.free_anchors,
            &euler_spans,
            &init.inc_euler,
            &init.out_euler,
        );
    }

    euler_spans
        .iter()
        .map(|es| {
            let theta = wrap_angle_pi(init.anchor_angles[es.start_a] - es.psi);
            let phi = wrap_angle_pi(es.psi - init.anchor_angles[es.end_a]);
            let arc = solve_clothoid_arc(theta, phi, es.chord_len);
            let cubic =
                clothoid_to_cubic(es.p_start, es.p_end, es.psi, es.chord_len, theta, phi, arc);
            SolvedSplineSegment {
                start_idx: es.start_idx,
                end_idx: es.end_idx,
                cubic,
                theta,
                phi,
                mock_curvature_out: arc.kappa_out,
                mock_curvature_in: arc.kappa_in,
            }
        })
        .collect()
}
