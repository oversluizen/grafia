//! `SPEC-009 §2.1` Tension-Controlled Hobby Spline (`NodeType::HobbyStar`) and Clothoid Euler/Spiro
//! (`NodeType::EulerSpiro`) solvers producing dynamic cubic Bézier control points.

use crate::core::bezier_ops::{Node, NodeType};
use crate::core::math_consts::{
    HOBBY_HANDLE_DIVISOR, HOBBY_MIN_CHORD_EPS, HOBBY_MOCK_CURVATURE_FACTOR,
    HOBBY_RHO_COS_PHI_WEIGHT, HOBBY_RHO_COS_THETA_WEIGHT, HOBBY_RHO_MAX_CLAMP, HOBBY_RHO_MIN_CLAMP,
    HOBBY_RHO_SIN_SIXTEENTH, HOBBY_RHO_SQRT2, HOBBY_SOLVER_PIVOT_EPS, HOBBY_TENSION_DEFAULT,
    HOBBY_TENSION_MAX, HOBBY_TENSION_MIN,
};
pub(crate) use crate::core::spline_support::{
    constrained_anchor_world_angle, extract_anchor_spans, solve_linear_system, wrap_angle_pi,
};
use kurbo::{CubicBez, Point};

/// Solved cubic segment metadata emitted for a contiguous `HobbyStar` or `EulerSpiro` span.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SolvedSplineSegment {
    pub start_idx: usize,
    pub end_idx: usize,
    pub cubic: CubicBez,
    pub theta: f64,
    pub phi: f64,
    pub mock_curvature_out: f64,
    pub mock_curvature_in: f64,
}

/// John D. Hobby velocity function $\rho(\theta, \phi)$ (`SPEC-009 §1`).
#[must_use]
pub fn hobby_velocity_rho(theta: f64, phi: f64) -> f64 {
    debug_assert!(theta.is_finite() && phi.is_finite());
    let st = theta.sin();
    let ct = theta.cos();
    let sp = phi.sin();
    let cp = phi.cos();
    let num = 2.0
        + HOBBY_RHO_SQRT2
            * (st - HOBBY_RHO_SIN_SIXTEENTH * sp)
            * (sp - HOBBY_RHO_SIN_SIXTEENTH * st)
            * (ct - cp);
    let den = 1.0 + HOBBY_RHO_COS_THETA_WEIGHT * ct + HOBBY_RHO_COS_PHI_WEIGHT * cp;
    if den.abs() <= HOBBY_SOLVER_PIVOT_EPS {
        return 1.0;
    }
    (num / den).clamp(HOBBY_RHO_MIN_CLAMP, HOBBY_RHO_MAX_CLAMP)
}

#[derive(Clone, Copy)]
struct HobbySpanSpec {
    start_a: usize,
    end_a: usize,
    start_idx: usize,
    end_idx: usize,
    p_start: Point,
    p_end: Point,
    chord_len: f64,
    psi: f64,
    alpha: f64,
    beta: f64,
}

fn collect_hobby_spans(
    nodes: &[Node],
    anchors: &[usize],
    spans: &[(usize, usize, Vec<usize>)],
) -> Vec<HobbySpanSpec> {
    let mut hobby_spans = Vec::new();
    for (s_pos, e_pos, off) in spans {
        if !off.is_empty() {
            continue;
        }
        let s_idx = anchors[*s_pos];
        let e_idx = anchors[*e_pos];
        let s_hobby = matches!(nodes[s_idx].node_type, NodeType::HobbyStar { .. });
        let e_hobby = matches!(nodes[e_idx].node_type, NodeType::HobbyStar { .. });
        if !s_hobby && !e_hobby {
            continue;
        }
        let alpha = match nodes[s_idx].node_type {
            NodeType::HobbyStar { tension_out, .. } => {
                tension_out.clamp(HOBBY_TENSION_MIN, HOBBY_TENSION_MAX)
            }
            _ => HOBBY_TENSION_DEFAULT,
        };
        let beta = match nodes[e_idx].node_type {
            NodeType::HobbyStar { tension_in, .. } => {
                tension_in.clamp(HOBBY_TENSION_MIN, HOBBY_TENSION_MAX)
            }
            _ => HOBBY_TENSION_DEFAULT,
        };
        let p_start = Point::new(nodes[s_idx].point.x.0, nodes[s_idx].point.y.0);
        let p_end = Point::new(nodes[e_idx].point.x.0, nodes[e_idx].point.y.0);
        let (dx, dy) = (p_end.x - p_start.x, p_end.y - p_start.y);
        hobby_spans.push(HobbySpanSpec {
            start_a: *s_pos,
            end_a: *e_pos,
            start_idx: s_idx,
            end_idx: e_idx,
            p_start,
            p_end,
            chord_len: dx.hypot(dy),
            psi: dy.atan2(dx),
            alpha,
            beta,
        });
    }
    hobby_spans
}

fn assemble_hobby_linear_system(
    nodes: &[Node],
    hobby_spans: &[HobbySpanSpec],
    fixed_dir: &[Option<f64>],
    inc_span: &[Option<usize>],
    out_span: &[Option<usize>],
) -> (Vec<Vec<f64>>, Vec<f64>) {
    let dim = 2 * hobby_spans.len();
    let zero_row = vec![0.0; dim];
    let mut mat = vec![zero_row; dim];
    let mut rhs = vec![0.0; dim];

    for (j, hs) in hobby_spans.iter().enumerate() {
        let row_theta = 2 * j;
        let row_phi = 2 * j + 1;
        let d_j = hs.chord_len.max(HOBBY_MIN_CHORD_EPS);

        if let Some(gamma_out) = fixed_dir[hs.start_a] {
            mat[row_theta][2 * j] = 1.0;
            rhs[row_theta] = wrap_angle_pi(gamma_out - hs.psi);
        } else if let Some(p) = inc_span[hs.start_a] {
            let start_node_type = nodes[hs.start_idx].node_type;
            if matches!(
                start_node_type,
                NodeType::HobbyStar { .. } | NodeType::Smooth
            ) {
                mat[row_theta][2 * j] = 1.0;
                mat[row_theta][2 * p + 1] = 1.0;
                rhs[row_theta] = -wrap_angle_pi(hs.psi - hobby_spans[p].psi);
            } else {
                mat[row_theta][2 * j] = -(HOBBY_MOCK_CURVATURE_FACTOR * hs.beta - 1.0);
                mat[row_theta][2 * j + 1] = 1.0;
            }
        } else {
            mat[row_theta][2 * j] = -(HOBBY_MOCK_CURVATURE_FACTOR * hs.beta - 1.0);
            mat[row_theta][2 * j + 1] = 1.0;
        }

        if let Some(gamma_in) = fixed_dir[hs.end_a] {
            mat[row_phi][2 * j + 1] = 1.0;
            rhs[row_phi] = wrap_angle_pi(hs.psi - gamma_in);
        } else if let Some(s) = out_span[hs.end_a] {
            let end_node_type = nodes[hs.end_idx].node_type;
            if matches!(end_node_type, NodeType::HobbyStar { .. } | NodeType::Smooth) {
                let next_hs = &hobby_spans[s];
                let d_s = next_hs.chord_len.max(HOBBY_MIN_CHORD_EPS);
                let c_in = 2.0 * hs.beta * hs.beta / (d_j * hs.alpha);
                let c_out = 2.0 * next_hs.alpha * next_hs.alpha / (d_s * next_hs.beta);
                mat[row_phi][2 * j] += c_in;
                mat[row_phi][2 * j + 1] -= c_in * (HOBBY_MOCK_CURVATURE_FACTOR * hs.alpha - 1.0);
                mat[row_phi][2 * s] += c_out * (HOBBY_MOCK_CURVATURE_FACTOR * next_hs.beta - 1.0);
                mat[row_phi][2 * s + 1] -= c_out;
            } else {
                mat[row_phi][2 * j] = 1.0;
                mat[row_phi][2 * j + 1] = -(HOBBY_MOCK_CURVATURE_FACTOR * hs.alpha - 1.0);
            }
        } else {
            mat[row_phi][2 * j] = 1.0;
            mat[row_phi][2 * j + 1] = -(HOBBY_MOCK_CURVATURE_FACTOR * hs.alpha - 1.0);
        }
    }
    (mat, rhs)
}

/// Solves all `NodeType::HobbyStar` segments in `nodes` into `SolvedSplineSegment`s (`SPEC-009 §2.1.2`).
#[must_use]
pub fn evaluate_hobby_chain_to_cubics(
    nodes: &[Node],
    closed: bool,
    italic_angle_deg: f64,
    metrics_y: &[f64],
) -> Vec<SolvedSplineSegment> {
    let (anchors, spans) = extract_anchor_spans(nodes, closed);
    let m = anchors.len();
    if m < 2 {
        return Vec::new();
    }

    let hobby_spans = collect_hobby_spans(nodes, &anchors, &spans);
    let k = hobby_spans.len();
    if k == 0 {
        return Vec::new();
    }

    let mut inc_span = vec![None; m];
    let mut out_span = vec![None; m];
    for (j, hs) in hobby_spans.iter().enumerate() {
        out_span[hs.start_a] = Some(j);
        inc_span[hs.end_a] = Some(j);
    }
    let fixed_dir: Vec<Option<f64>> = (0..m)
        .map(|a_pos| {
            constrained_anchor_world_angle(
                nodes,
                &anchors,
                &spans,
                a_pos,
                closed,
                italic_angle_deg,
                metrics_y,
            )
        })
        .collect();

    let (mut mat, mut rhs) =
        assemble_hobby_linear_system(nodes, &hobby_spans, &fixed_dir, &inc_span, &out_span);
    let dim = 2 * k;
    let sol = solve_linear_system(&mut mat, &mut rhs).unwrap_or_else(|| vec![0.0; dim]);
    hobby_spans
        .iter()
        .enumerate()
        .map(|(j, hs)| {
            let theta = sol[2 * j];
            let phi = sol[2 * j + 1];
            let d = hs.chord_len;
            let cubic = if d <= HOBBY_MIN_CHORD_EPS {
                CubicBez::new(hs.p_start, hs.p_start, hs.p_end, hs.p_end)
            } else {
                let r_out = d * hobby_velocity_rho(theta, phi) / (HOBBY_HANDLE_DIVISOR * hs.alpha);
                let r_in = d * hobby_velocity_rho(phi, theta) / (HOBBY_HANDLE_DIVISOR * hs.beta);
                let out_ang = hs.psi + theta;
                let in_ang = hs.psi - phi;
                let h1 = Point::new(
                    hs.p_start.x + r_out * out_ang.cos(),
                    hs.p_start.y + r_out * out_ang.sin(),
                );
                let h2 = Point::new(
                    hs.p_end.x - r_in * in_ang.cos(),
                    hs.p_end.y - r_in * in_ang.sin(),
                );
                CubicBez::new(hs.p_start, h1, h2, hs.p_end)
            };
            let safe_d = d.max(HOBBY_MIN_CHORD_EPS);
            let mock_curvature_out = (2.0 * hs.alpha * hs.alpha / (safe_d * hs.beta))
                * (phi - (HOBBY_MOCK_CURVATURE_FACTOR * hs.beta - 1.0) * theta);
            let mock_curvature_in = (2.0 * hs.beta * hs.beta / (safe_d * hs.alpha))
                * (theta - (HOBBY_MOCK_CURVATURE_FACTOR * hs.alpha - 1.0) * phi);
            SolvedSplineSegment {
                start_idx: hs.start_idx,
                end_idx: hs.end_idx,
                cubic,
                theta,
                phi,
                mock_curvature_out,
                mock_curvature_in,
            }
        })
        .collect()
}

/// Solves all `NodeType::EulerSpiro` clothoid segments in `nodes` into `SolvedSplineSegment`s (`SPEC-009 §2.1.2`).
#[must_use]
pub fn evaluate_euler_spiro_chain_to_cubics(
    nodes: &[Node],
    closed: bool,
    italic_angle_deg: f64,
    metrics_y: &[f64],
) -> Vec<SolvedSplineSegment> {
    crate::core::euler_spiro::solve_euler_spiro_chain(nodes, closed, italic_angle_deg, metrics_y)
}
