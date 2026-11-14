//! Contour start-node ordering, winding direction correction, and unslanted curve-flattening operations.

use crate::core::bezier_ops::{compute_signed_area, Node, NodeId, NodeType};
use crate::core::coordinates::FontPoint;
use crate::core::curve_manipulation::{extract_segments, ContourSegment};
use crate::core::lasso_and_zoom::point_in_polygon;
use crate::core::math_consts::{CURVE_FLATTEN_TOLERANCE, PATH_COORD_EPS_UPM};
use crate::core::reverse_contour_nodes;
use crate::core::transform_ops::unslant_point;

pub fn make_node_first_or_reverse_open(
    contour: &mut [Node],
    closed: bool,
    selected: &[NodeId],
) -> bool {
    if contour.len() < 2 {
        return false;
    }
    let Some(pos) = contour
        .iter()
        .position(|n| selected.contains(&n.id) && n.node_type != NodeType::OffCurve)
    else {
        return false;
    };
    if closed {
        if pos > 0 {
            contour.rotate_left(pos);
            return true;
        }
        false
    } else if pos + 1 == contour.len() {
        contour.reverse();
        if let Some(first) = contour.first_mut() {
            if first.node_type == NodeType::Smooth {
                first.node_type = NodeType::Corner;
            }
        }
        true
    } else {
        false
    }
}

#[must_use]
pub fn flatten_contour_points(
    contour: &[Node],
    closed: bool,
    italic_angle_deg: f64,
    half_x_height: f64,
) -> Vec<FontPoint> {
    if contour.is_empty() {
        return Vec::new();
    }
    let mut path = kurbo::BezPath::new();
    let unslanted = |n: &Node| {
        let p = unslant_point(n.point, italic_angle_deg, half_x_height);
        kurbo::Point::new(p.x.0, p.y.0)
    };
    path.move_to(unslanted(&contour[0]));
    for seg in extract_segments(contour, closed) {
        match seg {
            ContourSegment::Line { end_idx, .. } => {
                path.line_to(unslanted(&contour[end_idx]));
            }
            ContourSegment::Cubic {
                h1_idx,
                h2_idx,
                end_idx,
                ..
            } => {
                path.curve_to(
                    unslanted(&contour[h1_idx]),
                    unslanted(&contour[h2_idx]),
                    unslanted(&contour[end_idx]),
                );
            }
        }
    }
    if closed {
        path.close_path();
    }
    let mut pts = Vec::new();
    kurbo::flatten(path.iter(), CURVE_FLATTEN_TOLERANCE, |el| match el {
        kurbo::PathEl::MoveTo(p) | kurbo::PathEl::LineTo(p) => pts.push(FontPoint::new(p.x, p.y)),
        _ => {}
    });
    pts
}

fn rotate_contour_to_lowest_leftmost(
    contour: &mut [Node],
    italic_angle_deg: f64,
    half_x_height: f64,
) {
    let mut best_idx = 0usize;
    let mut best_pt = FontPoint::new(f64::INFINITY, f64::INFINITY);
    for (idx, nd) in contour.iter().enumerate() {
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
        contour.rotate_left(best_idx);
    }
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
                    && (point_in_polygon(test_pt, &polys[j])
                        || point_in_polygon(centroid, &polys[j]))
            })
            .count();
        let area = compute_signed_area(&polys[i]);
        let want_ccw = depth.is_multiple_of(2);
        if (want_ccw && area < 0.0) || (!want_ccw && area > 0.0) {
            reverse_contour_nodes(&mut contours[i], true);
        }
        rotate_contour_to_lowest_leftmost(&mut contours[i], italic_angle_deg, half_x_height);
    }
    contours != before
}
