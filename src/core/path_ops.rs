//! `SPEC-003 §2.6` Path Operations: `add_extremes_to_contours` and extremum curve subdivision.

use crate::core::bezier_ops::{Node, NodeId, NodeType};
use crate::core::coordinates::FontPoint;
use crate::core::curve_manipulation::{extract_segments, ContourSegment};
use crate::core::math_consts::{
    BERNSTEIN_CUBIC_COEFF, COLLINEAR_MIN_LEN_EPSILON, CURVE_ROOT_DEDUP_EPS, CURVE_ROOT_T_MAX,
    CURVE_ROOT_T_MIN, EXTREMUM_MIN_ENDPOINT_DIST_UPM, EXTREMUM_SMOOTH_ABSORB_DIST_UPM,
    QUAD_DISC_FOUR, SOLVER_DET_EPSILON,
};
use crate::core::tidy_align_connect::tidy_up_contours;
use kurbo::{CubicBez, ParamCurve, Point};

pub(crate) fn round_grid_pt(pt: FontPoint, grid: f64) -> FontPoint {
    if grid > 0.0 {
        FontPoint::new(
            (pt.x.0 / grid).round() * grid,
            (pt.y.0 / grid).round() * grid,
        )
    } else {
        pt
    }
}

pub(crate) fn alloc_node(next_id: &mut u64, pt: FontPoint, nt: NodeType) -> Node {
    let id = NodeId(*next_id);
    *next_id += 1;
    Node::new(id, pt.x.0, pt.y.0, nt)
}

pub(crate) fn solve_quadratic_roots_in_range(
    a: f64,
    b: f64,
    c: f64,
    t_min: f64,
    t_max: f64,
) -> Vec<f64> {
    let mut roots = Vec::new();
    if a.abs() <= SOLVER_DET_EPSILON {
        if b.abs() > SOLVER_DET_EPSILON {
            let t = -c / b;
            if t > t_min && t < t_max {
                roots.push(t);
            }
        }
        return roots;
    }
    let disc = b * b - QUAD_DISC_FOUR * a * c;
    if disc < 0.0 {
        return roots;
    }
    let sqrt_d = disc.sqrt();
    for t in [(-b - sqrt_d) / (2.0 * a), (-b + sqrt_d) / (2.0 * a)] {
        if t > t_min && t < t_max {
            roots.push(t);
        }
    }
    roots.sort_by(|x, y| x.partial_cmp(y).unwrap_or(std::cmp::Ordering::Equal));
    roots.dedup_by(|x, y| (*x - *y).abs() < CURVE_ROOT_DEDUP_EPS);
    roots
}

fn cubic_derivative_roots_1d(p0: f64, p1: f64, p2: f64, p3: f64) -> Vec<f64> {
    let a = (p3 - p0) + (p1 - p2) * BERNSTEIN_CUBIC_COEFF;
    let b = ((p2 - p1 * 2.0) + p0) * 2.0;
    solve_quadratic_roots_in_range(a, b, p1 - p0, CURVE_ROOT_T_MIN, CURVE_ROOT_T_MAX)
}

fn absorb_previous_smooth_into_cubic(cubic: &mut CubicBez, rebuilt: &[Node]) {
    let Some(last_node) = rebuilt.last() else {
        return;
    };
    let new_p0 = last_node.point;
    let prev_h2 = if rebuilt.len() >= 2 {
        rebuilt[rebuilt.len() - 2].point
    } else {
        new_p0
    };
    let (dx, dy) = (new_p0.x.0 - prev_h2.x.0, new_p0.y.0 - prev_h2.y.0);
    let (tx, ty) = if dx.abs() >= dy.abs() {
        (if dx >= 0.0 { 1.0 } else { -1.0 }, 0.0)
    } else {
        (0.0, if dy >= 0.0 { 1.0 } else { -1.0 })
    };
    let old_p0 = FontPoint::new(cubic.p0.x, cubic.p0.y);
    let old_p3 = FontPoint::new(cubic.p3.x, cubic.p3.y);
    let ratio =
        old_p3.distance_to(new_p0) / old_p3.distance_to(old_p0).max(COLLINEAR_MIN_LEN_EPSILON);
    let h1_len = FontPoint::new(cubic.p1.x, cubic.p1.y).distance_to(old_p0) * ratio;
    cubic.p0 = Point::new(new_p0.x.0, new_p0.y.0);
    cubic.p1 = Point::new(new_p0.x.0 + tx * h1_len, new_p0.y.0 + ty * h1_len);
    cubic.p2 = Point::new(
        old_p3.x.0 + (cubic.p2.x - old_p3.x.0) * ratio,
        old_p3.y.0 + (cubic.p2.y - old_p3.y.0) * ratio,
    );
}

fn find_valid_extremum_roots(
    cubic: CubicBez,
    end_node_type: NodeType,
    has_next_seg: bool,
    force: bool,
    absorb_prev_smooth: &mut bool,
) -> Vec<f64> {
    let mut roots = cubic_derivative_roots_1d(cubic.p0.x, cubic.p1.x, cubic.p2.x, cubic.p3.x);
    roots.extend(cubic_derivative_roots_1d(
        cubic.p0.y, cubic.p1.y, cubic.p2.y, cubic.p3.y,
    ));
    roots.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    roots.dedup_by(|a, b| (*a - *b).abs() < CURVE_ROOT_DEDUP_EPS);

    let (p0_pt, p3_pt) = (
        FontPoint::new(cubic.p0.x, cubic.p0.y),
        FontPoint::new(cubic.p3.x, cubic.p3.y),
    );
    let mut valid_roots = Vec::new();
    for t in roots {
        let m = cubic.eval(t);
        let m_pt = FontPoint::new(m.x, m.y);
        let (d0, d3) = (m_pt.distance_to(p0_pt), m_pt.distance_to(p3_pt));
        if d0 < EXTREMUM_MIN_ENDPOINT_DIST_UPM || d3 < EXTREMUM_MIN_ENDPOINT_DIST_UPM {
            continue;
        }
        if !force && d3 < EXTREMUM_SMOOTH_ABSORB_DIST_UPM {
            if end_node_type == NodeType::Corner {
                continue;
            } else if end_node_type == NodeType::Smooth && has_next_seg {
                *absorb_prev_smooth = true;
            }
        }
        valid_roots.push(t);
    }
    valid_roots
}

fn emit_subdivided_extremum_cubics(
    cubic: CubicBez,
    valid_roots: &[f64],
    flags: (bool, bool),
    end_node: &Node,
    grid_step_upm: f64,
    next_node_id: &mut u64,
    rebuilt: &mut Vec<Node>,
) {
    let (absorb_prev_smooth, append_end) = flags;
    let mut stops = vec![0.0];
    stops.extend(valid_roots.iter().copied());
    if !absorb_prev_smooth {
        stops.push(1.0);
    }
    let mut sub_cubics: Vec<CubicBez> = stops
        .windows(2)
        .map(|w| cubic.subsegment(w[0]..w[1]))
        .collect();
    for k in 0..sub_cubics.len() {
        if k + 1 < sub_cubics.len() || absorb_prev_smooth {
            let m_pt = round_grid_pt(
                FontPoint::new(sub_cubics[k].p3.x, sub_cubics[k].p3.y),
                grid_step_upm,
            );
            sub_cubics[k].p3 = Point::new(m_pt.x.0, m_pt.y.0);
            let r0 = sub_cubics[k].p2;
            let r1 = if k + 1 < sub_cubics.len() {
                sub_cubics[k + 1].p1
            } else {
                r0
            };
            if (r1.x - r0.x).abs() >= (r1.y - r0.y).abs() {
                sub_cubics[k].p2.y = m_pt.y.0;
                if k + 1 < sub_cubics.len() {
                    sub_cubics[k + 1].p0 = Point::new(m_pt.x.0, m_pt.y.0);
                    sub_cubics[k + 1].p1.y = m_pt.y.0;
                }
            } else {
                sub_cubics[k].p2.x = m_pt.x.0;
                if k + 1 < sub_cubics.len() {
                    sub_cubics[k + 1].p0 = Point::new(m_pt.x.0, m_pt.y.0);
                    sub_cubics[k + 1].p1.x = m_pt.x.0;
                }
            }
        }
    }
    for (k, sc) in sub_cubics.iter().enumerate() {
        rebuilt.push(alloc_node(
            next_node_id,
            round_grid_pt(FontPoint::new(sc.p1.x, sc.p1.y), grid_step_upm),
            NodeType::OffCurve,
        ));
        rebuilt.push(alloc_node(
            next_node_id,
            round_grid_pt(FontPoint::new(sc.p2.x, sc.p2.y), grid_step_upm),
            NodeType::OffCurve,
        ));
        if k + 1 < sub_cubics.len() || absorb_prev_smooth {
            rebuilt.push(alloc_node(
                next_node_id,
                round_grid_pt(FontPoint::new(sc.p3.x, sc.p3.y), grid_step_upm),
                NodeType::Smooth,
            ));
        } else if append_end {
            rebuilt.push(end_node.clone());
        }
    }
}

fn finalize_closed_smooth_absorption(rebuilt: &mut Vec<Node>, grid_step_upm: f64) {
    let Some(new_p0_node) = rebuilt.pop() else {
        return;
    };
    let Some(prev_h2_node) = rebuilt.last() else {
        return;
    };
    let new_p0 = new_p0_node.point;
    let prev_h2 = prev_h2_node.point;
    let (dx, dy) = (new_p0.x.0 - prev_h2.x.0, new_p0.y.0 - prev_h2.y.0);
    let (tx, ty) = if dx.abs() >= dy.abs() {
        (if dx >= 0.0 { 1.0 } else { -1.0 }, 0.0)
    } else {
        (0.0, if dy >= 0.0 { 1.0 } else { -1.0 })
    };
    let old_p0 = rebuilt[0].point;
    rebuilt[0] = new_p0_node;
    if rebuilt[1].node_type == NodeType::OffCurve && rebuilt[2].node_type == NodeType::OffCurve {
        let old_p3 = rebuilt[3].point;
        let ratio =
            old_p3.distance_to(new_p0) / old_p3.distance_to(old_p0).max(COLLINEAR_MIN_LEN_EPSILON);
        let h1_len = rebuilt[1].point.distance_to(old_p0) * ratio;
        rebuilt[1].point = round_grid_pt(
            FontPoint::new(new_p0.x.0 + tx * h1_len, new_p0.y.0 + ty * h1_len),
            grid_step_upm,
        );
        rebuilt[2].point = round_grid_pt(
            FontPoint::new(
                old_p3.x.0 + (rebuilt[2].point.x.0 - old_p3.x.0) * ratio,
                old_p3.y.0 + (rebuilt[2].point.y.0 - old_p3.y.0) * ratio,
            ),
            grid_step_upm,
        );
    }
}

fn add_extremes_to_single_contour(
    contour: &mut Vec<Node>,
    closed: bool,
    selected_nodes: &[NodeId],
    force: bool,
    next_node_id: &mut u64,
    grid_step_upm: f64,
) -> bool {
    let segs = extract_segments(contour, closed);
    if segs.is_empty() {
        return false;
    }
    let mut rebuilt: Vec<Node> = Vec::new();
    let mut absorb_prev_smooth = false;
    let mut contour_changed = false;

    for (s_idx, seg) in segs.iter().enumerate() {
        let append_end = !closed || s_idx + 1 < segs.len();
        match *seg {
            ContourSegment::Line {
                start_idx, end_idx, ..
            } => {
                if s_idx == 0 {
                    rebuilt.push(contour[start_idx].clone());
                }
                if append_end {
                    rebuilt.push(contour[end_idx].clone());
                }
                absorb_prev_smooth = false;
            }
            ContourSegment::Cubic {
                start_idx,
                h1_idx,
                h2_idx,
                end_idx,
                mut cubic,
            } => {
                if s_idx == 0 {
                    rebuilt.push(contour[start_idx].clone());
                }
                if absorb_prev_smooth {
                    absorb_previous_smooth_into_cubic(&mut cubic, &rebuilt);
                    absorb_prev_smooth = false;
                }
                let has_next = s_idx + 1 < segs.len() || closed;
                let valid_roots = resolve_cubic_extremum_roots_in_scope(
                    contour,
                    CubicSegmentNodeIndices::new(start_idx, h1_idx, h2_idx, end_idx),
                    cubic,
                    selected_nodes,
                    (has_next, force),
                    &mut absorb_prev_smooth,
                );
                if valid_roots.is_empty() {
                    let (id1, id2) = (contour[h1_idx].id, contour[h2_idx].id);
                    rebuilt.push(Node::new(id1, cubic.p1.x, cubic.p1.y, NodeType::OffCurve));
                    rebuilt.push(Node::new(id2, cubic.p2.x, cubic.p2.y, NodeType::OffCurve));
                    if append_end {
                        rebuilt.push(contour[end_idx].clone());
                    }
                } else {
                    contour_changed = true;
                    let flags = (absorb_prev_smooth, append_end);
                    emit_subdivided_extremum_cubics(
                        cubic,
                        &valid_roots,
                        flags,
                        &contour[end_idx],
                        grid_step_upm,
                        next_node_id,
                        &mut rebuilt,
                    );
                }
            }
        }
    }
    if absorb_prev_smooth && closed && rebuilt.len() >= 4 {
        finalize_closed_smooth_absorption(&mut rebuilt, grid_step_upm);
    }
    if contour_changed {
        *contour = rebuilt;
    }
    contour_changed
}

#[derive(Debug, Clone, Copy)]
struct CubicSegmentNodeIndices {
    start_idx: usize,
    h1_idx: usize,
    h2_idx: usize,
    end_idx: usize,
}

impl CubicSegmentNodeIndices {
    const fn new(start_idx: usize, h1_idx: usize, h2_idx: usize, end_idx: usize) -> Self {
        Self {
            start_idx,
            h1_idx,
            h2_idx,
            end_idx,
        }
    }
}

fn resolve_cubic_extremum_roots_in_scope(
    contour: &[Node],
    indices: CubicSegmentNodeIndices,
    cubic: CubicBez,
    selected_nodes: &[NodeId],
    flags: (bool, bool),
    absorb_prev_smooth: &mut bool,
) -> Vec<f64> {
    let CubicSegmentNodeIndices {
        start_idx,
        h1_idx,
        h2_idx,
        end_idx,
    } = indices;
    let (has_next, force) = flags;
    let in_scope = selected_nodes.is_empty()
        || [start_idx, h1_idx, h2_idx, end_idx]
            .iter()
            .any(|&i| selected_nodes.contains(&contour[i].id));
    if in_scope {
        find_valid_extremum_roots(
            cubic,
            contour[end_idx].node_type,
            has_next,
            force,
            absorb_prev_smooth,
        )
    } else {
        Vec::new()
    }
}

pub fn add_extremes_to_contours(
    contours: &mut Vec<Vec<Node>>,
    closed_flags: &mut Vec<bool>,
    selected_nodes: &[NodeId],
    force: bool,
    next_node_id: &mut u64,
    grid_step_upm: f64,
) -> bool {
    let mut any_changed = false;
    for (c_idx, contour) in contours.iter_mut().enumerate() {
        let closed = closed_flags.get(c_idx).copied().unwrap_or(true);
        if add_extremes_to_single_contour(
            contour,
            closed,
            selected_nodes,
            force,
            next_node_id,
            grid_step_upm,
        ) {
            any_changed = true;
        }
    }
    if any_changed && selected_nodes.is_empty() {
        let _ = tidy_up_contours(contours, closed_flags, selected_nodes, grid_step_upm);
    }
    any_changed
}
