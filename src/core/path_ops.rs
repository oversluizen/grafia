//! `SPEC-003 §2.6` Path Operations: `make_node_first_v020`, `correct_path_direction_layer`,
//! `add_extremes_to_contours`, and `add_inflection_points_to_contours`.

use crate::core::bezier_ops::{compute_signed_area, Node, NodeId, NodeType};
use crate::core::clipboard_ops::flatten_contour_points;
pub use crate::core::clipboard_ops::make_node_first_v020;
use crate::core::coordinates::FontPoint;
use crate::core::curve_manipulation::{extract_segments, ContourSegment};
use crate::core::lasso_and_zoom::point_in_polygon;
use crate::core::segment_ops::reverse_contour_nodes;
use crate::core::tidy_align_connect::tidy_up_contours;
use crate::core::transform_ops::unslant_point;
use crate::tokens::{
    COLLINEAR_MIN_LEN_EPSILON, CURVE_ROOT_DEDUP_EPS, CURVE_ROOT_T_MAX, CURVE_ROOT_T_MIN,
    EXTREMUM_MIN_ENDPOINT_DIST_UPM, EXTREMUM_SMOOTH_ABSORB_DIST_UPM,
    INFLECTION_MIN_CHORD_HEIGHT_UPM, INFLECTION_MIN_ENDPOINT_DIST_UPM, PATH_COORD_EPS_UPM,
    SOLVER_DET_EPSILON,
};
use kurbo::{CubicBez, ParamCurve, Point};

pub(crate) fn round_grid_pt(pt: FontPoint, grid: f64) -> FontPoint {
    if grid > 0.0 {
        FontPoint::new((pt.x.0 / grid).round() * grid, (pt.y.0 / grid).round() * grid)
    } else {
        pt
    }
}

fn alloc_node(next_id: &mut u64, pt: FontPoint, nt: NodeType) -> Node {
    let id = NodeId(*next_id);
    *next_id += 1;
    Node::new(id, pt.x.0, pt.y.0, nt)
}

pub fn correct_path_direction_layer(
    contours: &mut [Vec<Node>],
    closed_flags: &[bool],
    italic_angle_deg: f64,
    half_x_height: f64,
) -> bool {
    let n = contours.len();
    if n == 0 {
        return false;
    }
    let before: Vec<Vec<Node>> = contours.to_vec();
    let polys: Vec<Vec<FontPoint>> = (0..n)
        .map(|i| {
            if closed_flags.get(i).copied().unwrap_or(true) {
                flatten_contour_points(&contours[i], true, italic_angle_deg, half_x_height)
            } else {
                Vec::new()
            }
        })
        .collect();
    let bbox_areas: Vec<f64> = polys
        .iter()
        .map(|poly| {
            if poly.is_empty() {
                0.0
            } else {
                let min_x = poly.iter().map(|p| p.x.0).fold(f64::INFINITY, f64::min);
                let max_x = poly.iter().map(|p| p.x.0).fold(f64::NEG_INFINITY, f64::max);
                let min_y = poly.iter().map(|p| p.y.0).fold(f64::INFINITY, f64::min);
                let max_y = poly.iter().map(|p| p.y.0).fold(f64::NEG_INFINITY, f64::max);
                (max_x - min_x) * (max_y - min_y)
            }
        })
        .collect();

    for i in 0..n {
        if !closed_flags.get(i).copied().unwrap_or(true) || contours[i].len() < 2 {
            continue;
        }
        let Some(test_pt) = contours[i]
            .iter()
            .find(|nd| nd.node_type != NodeType::OffCurve)
            .map(|nd| unslant_point(nd.point, italic_angle_deg, half_x_height))
        else {
            continue;
        };
        let n_poly = polys[i].len().max(1) as f64;
        let centroid = FontPoint::new(
            polys[i].iter().map(|p| p.x.0).sum::<f64>() / n_poly,
            polys[i].iter().map(|p| p.y.0).sum::<f64>() / n_poly,
        );
        let depth = (0..n)
            .filter(|&j| {
                i != j
                    && closed_flags.get(j).copied().unwrap_or(true)
                    && bbox_areas[j] > bbox_areas[i]
                    && (point_in_polygon(test_pt, &polys[j]) || point_in_polygon(centroid, &polys[j]))
            })
            .count();
        let area = compute_signed_area(&polys[i]);
        let want_ccw = depth.is_multiple_of(2);
        if (want_ccw && area < 0.0) || (!want_ccw && area > 0.0) {
            reverse_contour_nodes(&mut contours[i], true);
        }
        let mut best_idx = 0usize;
        let mut best_pt = FontPoint::new(f64::INFINITY, f64::INFINITY);
        for (idx, nd) in contours[i].iter().enumerate() {
            if nd.node_type == NodeType::OffCurve {
                continue;
            }
            let p = unslant_point(nd.point, italic_angle_deg, half_x_height);
            if p.y.0 - best_pt.y.0 < -PATH_COORD_EPS_UPM
                || ((p.y.0 - best_pt.y.0).abs() <= PATH_COORD_EPS_UPM && p.x.0 < best_pt.x.0)
            {
                best_pt = p;
                best_idx = idx;
            }
        }
        if best_idx > 0 {
            contours[i].rotate_left(best_idx);
        }
    }
    contours != before
}

fn solve_quadratic_roots_in_range(a: f64, b: f64, c: f64, t_min: f64, t_max: f64) -> Vec<f64> {
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
    let disc = b * b - 2.0 * 2.0 * a * c;
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
    let a = (p3 - p0) + (p1 - p2) * (2.0 + 1.0);
    let b = ((p2 - p1 * 2.0) + p0) * 2.0;
    solve_quadratic_roots_in_range(a, b, p1 - p0, CURVE_ROOT_T_MIN, CURVE_ROOT_T_MAX)
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
    let check_sel = !selected_nodes.is_empty();

    for (c_idx, contour) in contours.iter_mut().enumerate() {
        let closed = closed_flags.get(c_idx).copied().unwrap_or(true);
        let segs = extract_segments(contour, closed);
        if segs.is_empty() {
            continue;
        }
        let mut rebuilt: Vec<Node> = Vec::new();
        let mut absorb_prev_smooth = false;
        let mut contour_changed = false;

        for (s_idx, seg) in segs.iter().enumerate() {
            match *seg {
                ContourSegment::Line { start_idx, end_idx, .. } => {
                    if s_idx == 0 {
                        rebuilt.push(contour[start_idx].clone());
                    }
                    if !closed || s_idx + 1 < segs.len() {
                        rebuilt.push(contour[end_idx].clone());
                    }
                    absorb_prev_smooth = false;
                }
                ContourSegment::Cubic { start_idx, h1_idx, h2_idx, end_idx, mut cubic } => {
                    if s_idx == 0 {
                        rebuilt.push(contour[start_idx].clone());
                    }
                    if absorb_prev_smooth {
                        let new_p0 = rebuilt.last().unwrap().point;
                        let prev_h2 = if rebuilt.len() >= 2 { rebuilt[rebuilt.len() - 2].point } else { new_p0 };
                        let (dx, dy) = (new_p0.x.0 - prev_h2.x.0, new_p0.y.0 - prev_h2.y.0);
                        let (tx, ty) = if dx.abs() >= dy.abs() { (if dx >= 0.0 { 1.0 } else { -1.0 }, 0.0) } else { (0.0, if dy >= 0.0 { 1.0 } else { -1.0 }) };
                        let old_p0 = FontPoint::new(cubic.p0.x, cubic.p0.y);
                        let old_p3 = FontPoint::new(cubic.p3.x, cubic.p3.y);
                        let ratio = old_p3.distance_to(new_p0)
                            / old_p3.distance_to(old_p0).max(COLLINEAR_MIN_LEN_EPSILON);
                        let h1_len = FontPoint::new(cubic.p1.x, cubic.p1.y).distance_to(old_p0) * ratio;
                        cubic.p0 = Point::new(new_p0.x.0, new_p0.y.0);
                        cubic.p1 = Point::new(new_p0.x.0 + tx * h1_len, new_p0.y.0 + ty * h1_len);
                        cubic.p2 = Point::new(
                            old_p3.x.0 + (cubic.p2.x - old_p3.x.0) * ratio,
                            old_p3.y.0 + (cubic.p2.y - old_p3.y.0) * ratio,
                        );
                        absorb_prev_smooth = false;
                    }
                    let in_scope = !check_sel
                        || [start_idx, h1_idx, h2_idx, end_idx]
                            .iter()
                            .any(|&i| selected_nodes.contains(&contour[i].id));
                    let mut roots = Vec::new();
                    if in_scope {
                        roots.extend(cubic_derivative_roots_1d(cubic.p0.x, cubic.p1.x, cubic.p2.x, cubic.p3.x));
                        roots.extend(cubic_derivative_roots_1d(cubic.p0.y, cubic.p1.y, cubic.p2.y, cubic.p3.y));
                        roots.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
                        roots.dedup_by(|a, b| (*a - *b).abs() < CURVE_ROOT_DEDUP_EPS);
                    }
                    let (p0_pt, p3_pt) = (FontPoint::new(cubic.p0.x, cubic.p0.y), FontPoint::new(cubic.p3.x, cubic.p3.y));
                    let mut valid_roots = Vec::new();
                    for t in roots {
                        let m = cubic.eval(t);
                        let m_pt = FontPoint::new(m.x, m.y);
                        let (d0, d3) = (m_pt.distance_to(p0_pt), m_pt.distance_to(p3_pt));
                        if d0 < EXTREMUM_MIN_ENDPOINT_DIST_UPM || d3 < EXTREMUM_MIN_ENDPOINT_DIST_UPM {
                            continue;
                        }
                        if !force && d3 < EXTREMUM_SMOOTH_ABSORB_DIST_UPM {
                            if contour[end_idx].node_type == NodeType::Corner {
                                continue;
                            } else if contour[end_idx].node_type == NodeType::Smooth && (s_idx + 1 < segs.len() || closed) {
                                absorb_prev_smooth = true;
                            }
                        }
                        valid_roots.push(t);
                    }

                    if valid_roots.is_empty() {
                        rebuilt.push(Node::new(contour[h1_idx].id, cubic.p1.x, cubic.p1.y, NodeType::OffCurve));
                        rebuilt.push(Node::new(contour[h2_idx].id, cubic.p2.x, cubic.p2.y, NodeType::OffCurve));
                        if !closed || s_idx + 1 < segs.len() {
                            rebuilt.push(contour[end_idx].clone());
                        }
                    } else {
                        contour_changed = true;
                        let mut stops = vec![0.0];
                        stops.extend(valid_roots.iter().copied());
                        if !absorb_prev_smooth {
                            stops.push(1.0);
                        }
                        let mut sub_cubics: Vec<CubicBez> = stops.windows(2).map(|w| cubic.subsegment(w[0]..w[1])).collect();
                        for k in 0..sub_cubics.len() {
                            if k + 1 < sub_cubics.len() || absorb_prev_smooth {
                                let m_pt = round_grid_pt(FontPoint::new(sub_cubics[k].p3.x, sub_cubics[k].p3.y), grid_step_upm);
                                sub_cubics[k].p3 = Point::new(m_pt.x.0, m_pt.y.0);
                                let r0 = sub_cubics[k].p2;
                                let r1 = if k + 1 < sub_cubics.len() { sub_cubics[k + 1].p1 } else { r0 };
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
                            rebuilt.push(alloc_node(next_node_id, round_grid_pt(FontPoint::new(sc.p1.x, sc.p1.y), grid_step_upm), NodeType::OffCurve));
                            rebuilt.push(alloc_node(next_node_id, round_grid_pt(FontPoint::new(sc.p2.x, sc.p2.y), grid_step_upm), NodeType::OffCurve));
                            if k + 1 < sub_cubics.len() || absorb_prev_smooth {
                                rebuilt.push(alloc_node(next_node_id, round_grid_pt(FontPoint::new(sc.p3.x, sc.p3.y), grid_step_upm), NodeType::Smooth));
                            } else if !closed || s_idx + 1 < segs.len() {
                                rebuilt.push(contour[end_idx].clone());
                            }
                        }
                    }
                }
            }
        }
        if absorb_prev_smooth && closed && rebuilt.len() >= 4 {
            let new_p0_node = rebuilt.pop().unwrap();
            let new_p0 = new_p0_node.point;
            let prev_h2 = rebuilt.last().unwrap().point;
            let (dx, dy) = (new_p0.x.0 - prev_h2.x.0, new_p0.y.0 - prev_h2.y.0);
            let (tx, ty) = if dx.abs() >= dy.abs() { (if dx >= 0.0 { 1.0 } else { -1.0 }, 0.0) } else { (0.0, if dy >= 0.0 { 1.0 } else { -1.0 }) };
            let old_p0 = rebuilt[0].point;
            rebuilt[0] = new_p0_node;
            if rebuilt[1].node_type == NodeType::OffCurve && rebuilt[2].node_type == NodeType::OffCurve {
                let old_p3 = rebuilt[3].point;
                let ratio = old_p3.distance_to(new_p0) / old_p3.distance_to(old_p0).max(COLLINEAR_MIN_LEN_EPSILON);
                let h1_len = rebuilt[1].point.distance_to(old_p0) * ratio;
                rebuilt[1].point = round_grid_pt(FontPoint::new(new_p0.x.0 + tx * h1_len, new_p0.y.0 + ty * h1_len), grid_step_upm);
                rebuilt[2].point = round_grid_pt(FontPoint::new(old_p3.x.0 + (rebuilt[2].point.x.0 - old_p3.x.0) * ratio, old_p3.y.0 + (rebuilt[2].point.y.0 - old_p3.y.0) * ratio), grid_step_upm);
            }
        }
        if contour_changed {
            *contour = rebuilt;
            any_changed = true;
        }
    }
    if any_changed && !check_sel {
        let _ = tidy_up_contours(contours, closed_flags, selected_nodes, grid_step_upm);
    }
    any_changed
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
            match *seg {
                ContourSegment::Line { start_idx, end_idx, .. } => {
                    if s_idx == 0 {
                        rebuilt.push(contour[start_idx].clone());
                    }
                    if !closed || s_idx + 1 < segs.len() {
                        rebuilt.push(contour[end_idx].clone());
                    }
                }
                ContourSegment::Cubic { start_idx, h1_idx, h2_idx, end_idx, cubic } => {
                    if s_idx == 0 {
                        rebuilt.push(contour[start_idx].clone());
                    }
                    let in_scope = !check_sel
                        || [start_idx, h1_idx, h2_idx, end_idx]
                            .iter()
                            .any(|&i| selected_nodes.contains(&contour[i].id));
                    let mut valid_roots = Vec::new();
                    if in_scope {
                        let (chord_dx, chord_dy) = (cubic.p3.x - cubic.p0.x, cubic.p3.y - cubic.p0.y);
                        let chord_len = chord_dx.hypot(chord_dy).max(COLLINEAR_MIN_LEN_EPSILON);
                        let h1_perp = ((cubic.p1.x - cubic.p0.x) * chord_dy - (cubic.p1.y - cubic.p0.y) * chord_dx).abs() / chord_len;
                        let h2_perp = ((cubic.p2.x - cubic.p0.x) * chord_dy - (cubic.p2.y - cubic.p0.y) * chord_dx).abs() / chord_len;
                        if h1_perp >= INFLECTION_MIN_CHORD_HEIGHT_UPM || h2_perp >= INFLECTION_MIN_CHORD_HEIGHT_UPM {
                            let (ax, ay) = (cubic.p1.x - cubic.p0.x, cubic.p1.y - cubic.p0.y);
                            let (bx, by) = (cubic.p2.x - cubic.p1.x - ax, cubic.p2.y - cubic.p1.y - ay);
                            let cx = cubic.p3.x - cubic.p0.x - (2.0 + 1.0) * ax - (2.0 + 1.0) * bx;
                            let cy = cubic.p3.y - cubic.p0.y - (2.0 + 1.0) * ay - (2.0 + 1.0) * by;
                            let (qa, qb, qc) = (bx * cy - by * cx, ax * cy - ay * cx, ax * by - ay * bx);
                            let (p0_pt, p3_pt) = (FontPoint::new(cubic.p0.x, cubic.p0.y), FontPoint::new(cubic.p3.x, cubic.p3.y));
                            for t in solve_quadratic_roots_in_range(qa, qb, qc, CURVE_ROOT_T_MIN, CURVE_ROOT_T_MAX) {
                                let m = cubic.eval(t);
                                let m_pt = FontPoint::new(m.x, m.y);
                                if m_pt.distance_to(p0_pt) > INFLECTION_MIN_ENDPOINT_DIST_UPM
                                    && m_pt.distance_to(p3_pt) > INFLECTION_MIN_ENDPOINT_DIST_UPM
                                {
                                    valid_roots.push(t);
                                }
                            }
                        }
                    }
                    if valid_roots.is_empty() {
                        rebuilt.push(contour[h1_idx].clone());
                        rebuilt.push(contour[h2_idx].clone());
                        if !closed || s_idx + 1 < segs.len() {
                            rebuilt.push(contour[end_idx].clone());
                        }
                    } else {
                        contour_changed = true;
                        let mut stops = vec![0.0];
                        stops.extend(valid_roots.iter().copied());
                        stops.push(1.0);
                        let sub_cubics: Vec<CubicBez> = stops.windows(2).map(|w| cubic.subsegment(w[0]..w[1])).collect();
                        for (k, sc) in sub_cubics.iter().enumerate() {
                            rebuilt.push(alloc_node(next_node_id, round_grid_pt(FontPoint::new(sc.p1.x, sc.p1.y), grid_step_upm), NodeType::OffCurve));
                            rebuilt.push(alloc_node(next_node_id, round_grid_pt(FontPoint::new(sc.p2.x, sc.p2.y), grid_step_upm), NodeType::OffCurve));
                            if k + 1 < sub_cubics.len() {
                                rebuilt.push(alloc_node(next_node_id, round_grid_pt(FontPoint::new(sc.p3.x, sc.p3.y), grid_step_upm), NodeType::Smooth));
                            } else if !closed || s_idx + 1 < segs.len() {
                                rebuilt.push(contour[end_idx].clone());
                            }
                        }
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
