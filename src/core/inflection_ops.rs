//! `SPEC-003 §2.6.4` Inflection point insertion (`add_inflection_points_to_contours`).

use crate::core::bezier_ops::{Node, NodeId, NodeType};
use crate::core::coordinates::FontPoint;
use crate::core::curve_manipulation::{extract_segments, ContourSegment};
use crate::core::math_consts::{
    BERNSTEIN_CUBIC_COEFF, COLLINEAR_MIN_LEN_EPSILON, CURVE_ROOT_T_MAX, CURVE_ROOT_T_MIN,
    INFLECTION_MIN_CHORD_HEIGHT_UPM, INFLECTION_MIN_ENDPOINT_DIST_UPM,
};
use crate::core::path_ops::{alloc_node, round_grid_pt, solve_quadratic_roots_in_range};
use kurbo::{CubicBez, ParamCurve};

fn find_cubic_inflection_roots(cubic: CubicBez) -> Vec<f64> {
    let (chord_dx, chord_dy) = (cubic.p3.x - cubic.p0.x, cubic.p3.y - cubic.p0.y);
    let chord_len = chord_dx.hypot(chord_dy).max(COLLINEAR_MIN_LEN_EPSILON);
    let h1_perp = ((cubic.p1.x - cubic.p0.x) * chord_dy - (cubic.p1.y - cubic.p0.y) * chord_dx)
        .abs()
        / chord_len;
    let h2_perp = ((cubic.p2.x - cubic.p0.x) * chord_dy - (cubic.p2.y - cubic.p0.y) * chord_dx)
        .abs()
        / chord_len;
    if h1_perp < INFLECTION_MIN_CHORD_HEIGHT_UPM && h2_perp < INFLECTION_MIN_CHORD_HEIGHT_UPM {
        return Vec::new();
    }
    let (ax, ay) = (cubic.p1.x - cubic.p0.x, cubic.p1.y - cubic.p0.y);
    let (bx, by) = (cubic.p2.x - cubic.p1.x - ax, cubic.p2.y - cubic.p1.y - ay);
    let cx = cubic.p3.x - cubic.p0.x - BERNSTEIN_CUBIC_COEFF * ax - BERNSTEIN_CUBIC_COEFF * bx;
    let cy = cubic.p3.y - cubic.p0.y - BERNSTEIN_CUBIC_COEFF * ay - BERNSTEIN_CUBIC_COEFF * by;
    let (qa, qb, qc) = (bx * cy - by * cx, ax * cy - ay * cx, ax * by - ay * bx);
    let (p0_pt, p3_pt) = (
        FontPoint::new(cubic.p0.x, cubic.p0.y),
        FontPoint::new(cubic.p3.x, cubic.p3.y),
    );
    let mut valid_roots = Vec::new();
    for t in solve_quadratic_roots_in_range(qa, qb, qc, CURVE_ROOT_T_MIN, CURVE_ROOT_T_MAX) {
        let m = cubic.eval(t);
        let m_pt = FontPoint::new(m.x, m.y);
        if m_pt.distance_to(p0_pt) > INFLECTION_MIN_ENDPOINT_DIST_UPM
            && m_pt.distance_to(p3_pt) > INFLECTION_MIN_ENDPOINT_DIST_UPM
        {
            valid_roots.push(t);
        }
    }
    valid_roots
}

fn emit_inflection_sub_cubics(
    cubic: CubicBez,
    valid_roots: &[f64],
    append_end: bool,
    end_node: &Node,
    grid_step_upm: f64,
    next_node_id: &mut u64,
    rebuilt: &mut Vec<Node>,
) {
    let mut stops = vec![0.0];
    stops.extend(valid_roots.iter().copied());
    stops.push(1.0);
    let sub_cubics: Vec<CubicBez> = stops
        .windows(2)
        .map(|w| cubic.subsegment(w[0]..w[1]))
        .collect();
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
        if k + 1 < sub_cubics.len() {
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

pub fn add_inflection_points_to_contours(
    contours: &mut [Vec<Node>],
    closed_flags: &[bool],
    selected_nodes: &[NodeId],
    next_node_id: &mut u64,
    grid_step_upm: f64,
) -> bool {
    let check_sel = !selected_nodes.is_empty();
    let mut any_changed = false;
    for (c_idx, contour) in contours.iter_mut().enumerate() {
        let closed = closed_flags.get(c_idx).copied().unwrap_or(true);
        let segs = extract_segments(contour, closed);
        if segs.is_empty() {
            continue;
        }
        let mut rebuilt = Vec::new();
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
                }
                ContourSegment::Cubic {
                    start_idx,
                    h1_idx,
                    h2_idx,
                    end_idx,
                    cubic,
                } => {
                    if s_idx == 0 {
                        rebuilt.push(contour[start_idx].clone());
                    }
                    let in_scope = !check_sel
                        || [start_idx, h1_idx, h2_idx, end_idx]
                            .iter()
                            .any(|&i| selected_nodes.contains(&contour[i].id));
                    let valid_roots = if in_scope {
                        find_cubic_inflection_roots(cubic)
                    } else {
                        Vec::new()
                    };
                    if valid_roots.is_empty() {
                        rebuilt.push(contour[h1_idx].clone());
                        rebuilt.push(contour[h2_idx].clone());
                        if append_end {
                            rebuilt.push(contour[end_idx].clone());
                        }
                    } else {
                        contour_changed = true;
                        emit_inflection_sub_cubics(
                            cubic,
                            &valid_roots,
                            append_end,
                            &contour[end_idx],
                            grid_step_upm,
                            next_node_id,
                            &mut rebuilt,
                        );
                    }
                }
            }
        }
        if contour_changed {
            *contour = rebuilt;
            any_changed = true;
        }
    }
    any_changed
}
