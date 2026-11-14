//! `SPEC-009 §2.2` 8+4 Selection Bounding Box outer-corner rotation (`[6.0 dp, 18.0 dp]` ring,
//! `Shift` `15°` quantization) and `Ctrl+Alt` mid-edge shear math.

use crate::core::bezier_ops::{Node, NodeId, NodeType};
use crate::core::coordinates::{FontPoint, ScreenPoint, ViewportTransform};
use crate::core::hobby_spline::wrap_angle_pi;
use crate::core::math_consts::{
    BBOX_KNOB_HIT_HALF_EXTENT_DP, BBOX_ROTATE_OUTER_MAX_DP, BBOX_ROTATE_OUTER_MIN_DP,
    BBOX_ROTATE_SHIFT_STEP_DEG, BBOX_SHEAR_MIN_SPAN_UPM, DEG_TO_RAD_F64, RAD_TO_DEG_F64,
    SOLVER_DET_EPSILON, TRANSFORM_MIN_PIVOT_DIST_UPM,
};
use crate::core::transform_ops::{unslant_point, SelectionBounds};
use kurbo::{Affine, Point};

const CORNER_KNOBS: [u8; 4] = [0, 2, 6, 8];

#[must_use]
pub fn hit_test_bbox_outer_corner_rotation(
    bounds: &SelectionBounds,
    viewport: &ViewportTransform,
    cursor_scr: ScreenPoint,
) -> Option<u8> {
    if !bounds.is_displayable(viewport.zoom) {
        return None;
    }
    let active = bounds.active_knobs(viewport.zoom);
    let max_perp = f64::from(BBOX_KNOB_HIT_HALF_EXTENT_DP * 2.0);
    let min_rad = f64::from(BBOX_ROTATE_OUTER_MIN_DP);
    let max_rad = f64::from(BBOX_ROTATE_OUTER_MAX_DP);

    for k in CORNER_KNOBS {
        if !active.contains(&k) {
            continue;
        }
        let p_box = bounds.box_screen_point(k, viewport);
        let p_knob = bounds.knob_screen_center(k, viewport);
        let vx = f64::from(p_knob.x.0 - p_box.x.0);
        let vy = f64::from(p_knob.y.0 - p_box.y.0);
        let v_len = vx.hypot(vy);
        if v_len <= SOLVER_DET_EPSILON {
            continue;
        }
        let ux = vx / v_len;
        let uy = vy / v_len;
        let dx = f64::from(cursor_scr.x.0 - p_knob.x.0);
        let dy = f64::from(cursor_scr.y.0 - p_knob.y.0);
        let radial = dx * ux + dy * uy;
        let perp = (dx * uy - dy * ux).abs();
        if radial >= min_rad && radial <= max_rad && perp <= max_perp {
            return Some(k);
        }
    }
    None
}

#[must_use]
pub fn compute_bbox_rotation_deg_and_affine(
    pivot: FontPoint,
    start_font: FontPoint,
    curr_font: FontPoint,
    is_shift: bool,
) -> (f64, Affine) {
    if start_font.distance_to(pivot) < TRANSFORM_MIN_PIVOT_DIST_UPM {
        return (0.0, Affine::IDENTITY);
    }
    let a0 = (start_font.y.0 - pivot.y.0).atan2(start_font.x.0 - pivot.x.0);
    let a1 = (curr_font.y.0 - pivot.y.0).atan2(curr_font.x.0 - pivot.x.0);
    let mut d_deg = wrap_angle_pi(a1 - a0) * RAD_TO_DEG_F64;
    if is_shift {
        d_deg = (d_deg / BBOX_ROTATE_SHIFT_STEP_DEG).round() * BBOX_ROTATE_SHIFT_STEP_DEG;
    }
    let aff = Affine::rotate_about(d_deg * DEG_TO_RAD_F64, Point::new(pivot.x.0, pivot.y.0));
    (d_deg, aff)
}

#[must_use]
pub fn compute_bbox_shear_affine(
    bounds: &SelectionBounds,
    k: u8,
    start_font: FontPoint,
    curr_font: FontPoint,
) -> Affine {
    let pivot = bounds.aligned_point(8 - k);
    let edge_pt = bounds.aligned_point(k);
    let s_u = unslant_point(start_font, bounds.angle_deg, bounds.half_x_height);
    let c_u = unslant_point(curr_font, bounds.angle_deg, bounds.half_x_height);
    let shear = if matches!(k, 1 | 7) {
        let span_y = edge_pt.y.0 - pivot.y.0;
        if span_y.abs() <= BBOX_SHEAR_MIN_SPAN_UPM {
            return Affine::IDENTITY;
        }
        let gamma_x = (c_u.x.0 - s_u.x.0) / span_y;
        Affine::new([1.0, 0.0, gamma_x, 1.0, 0.0, 0.0])
    } else if matches!(k, 3 | 5) {
        let span_x = edge_pt.x.0 - pivot.x.0;
        if span_x.abs() <= BBOX_SHEAR_MIN_SPAN_UPM {
            return Affine::IDENTITY;
        }
        let gamma_y = (c_u.y.0 - s_u.y.0) / span_x;
        Affine::new([1.0, gamma_y, 0.0, 1.0, 0.0, 0.0])
    } else {
        return Affine::IDENTITY;
    };
    Affine::translate((pivot.x.0, pivot.y.0)) * shear * Affine::translate((-pivot.x.0, -pivot.y.0))
}

pub fn apply_rigid_or_affine_coupled_transform(
    contours: &mut [Vec<Node>],
    shadow_contours: &[Vec<Node>],
    closed_flags: &[bool],
    selected_nodes: &[NodeId],
    affine: Affine,
    grid_step_upm: f64,
) {
    let round_pt = |pt: FontPoint| -> FontPoint {
        if grid_step_upm > 0.0 {
            FontPoint::new(
                (pt.x.0 / grid_step_upm).round() * grid_step_upm,
                (pt.y.0 / grid_step_upm).round() * grid_step_upm,
            )
        } else {
            pt
        }
    };
    for (c_idx, contour) in contours.iter_mut().enumerate() {
        let Some(shadow) = shadow_contours.get(c_idx) else {
            continue;
        };
        let len = contour.len();
        let closed = closed_flags.get(c_idx).copied().unwrap_or(true);
        for i in 0..len {
            let direct_sel = selected_nodes.contains(&contour[i].id);
            let attached_to_sel = if !direct_sel && contour[i].node_type == NodeType::OffCurve {
                let prev = if i > 0 {
                    Some(i - 1)
                } else if closed && len >= 2 {
                    Some(len - 1)
                } else {
                    None
                };
                let next = if i + 1 < len {
                    Some(i + 1)
                } else if closed && len >= 2 {
                    Some(0)
                } else {
                    None
                };
                let anchor_idx = prev
                    .filter(|&p| contour[p].node_type != NodeType::OffCurve)
                    .or_else(|| next.filter(|&n| contour[n].node_type != NodeType::OffCurve));
                anchor_idx.is_some_and(|a| selected_nodes.contains(&contour[a].id))
            } else {
                false
            };
            if direct_sel || attached_to_sel {
                let sp = shadow[i].point;
                let tp = affine * Point::new(sp.x.0, sp.y.0);
                contour[i].point = round_pt(FontPoint::new(tp.x, tp.y));
            } else {
                contour[i].point = shadow[i].point;
            }
        }
    }
}
