//! `SPEC-003 §2.1.7, §2.3 & §2.4` `LassoTool` point-in-polygon selection, slanted marquee
//! parallelogram geometry, and `ZoomTool` 23-step discrete & marquee zoom math.

use crate::core::bezier_ops::{Node, NodeId, NodeType};
use crate::core::coordinates::{FontPoint, ScreenPoint, ViewportTransform};
use crate::core::transform_ops::TransformGridAnchor;
use crate::tokens::{
    EditCanvasTokens, DEG_TO_RAD_F64, MARQUEE_MAX_SLANT_TAN, SOLVER_DET_EPSILON,
    ZOOM_MARQUEE_MIN_SUM_DP, ZOOM_STEPS, ZOOM_STEP_NEXT_THRESHOLD, ZOOM_STEP_PREV_THRESHOLD,
    ZOOM_TOOL_MAX_SCALE, ZOOM_TOOL_MIN_SCALE,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[repr(u8)]
pub enum MetricsOrigin {
    #[default]
    Baseline = 0,
    Ascender = 1,
    CapHeight = 2,
    HalfXHeight = 3,
    XHeight = 4,
}

impl MetricsOrigin {
    #[must_use]
    pub const fn from_index(idx: u8) -> Self {
        match idx {
            1 => Self::Ascender,
            2 => Self::CapHeight,
            3 => Self::HalfXHeight,
            4 => Self::XHeight,
            _ => Self::Baseline,
        }
    }
    #[must_use]
    pub fn resolve_pivot(
        self,
        advance_width: f64,
        ascender: f64,
        cap_height: f64,
        x_height: f64,
        italic_angle_deg: f64,
    ) -> FontPoint {
        let y = match self {
            Self::Baseline => 0.0,
            Self::Ascender => ascender,
            Self::CapHeight => cap_height,
            Self::HalfXHeight => 0.5 * x_height,
            Self::XHeight => x_height,
        };
        FontPoint::new(
            0.5 * advance_width + y * (italic_angle_deg * DEG_TO_RAD_F64).tan(),
            y,
        )
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub enum TransformOriginSpec {
    #[default]
    BoundingBoxCenter,
    BoundingBoxOrigin(TransformGridAnchor),
    MetricsOrigin(MetricsOrigin),
    Custom(FontPoint),
}

#[must_use]
pub fn point_in_polygon(pt: FontPoint, poly: &[FontPoint]) -> bool {
    if poly.len() < 3 {
        return false;
    }
    let mut inside = false;
    let mut j = poly.len() - 1;
    for i in 0..poly.len() {
        let pi = poly[i];
        let pj = poly[j];
        let intersects = ((pi.y.0 > pt.y.0) != (pj.y.0 > pt.y.0))
            && (pt.x.0
                < (pj.x.0 - pi.x.0) * (pt.y.0 - pi.y.0)
                    / (pj.y.0 - pi.y.0 + SOLVER_DET_EPSILON.copysign(pj.y.0 - pi.y.0))
                    + pi.x.0);
        if intersects {
            inside = !inside;
        }
        j = i;
    }
    inside
}

#[must_use]
pub fn select_nodes_in_lasso_polygon(
    contours: &[Vec<Node>],
    poly: &[FontPoint],
    initial_selected: &[NodeId],
    is_shift: bool,
    is_alt: bool,
) -> Vec<NodeId> {
    let mut hits = Vec::new();
    for contour in contours {
        for node in contour {
            if is_alt && node.node_type == NodeType::OffCurve {
                continue;
            }
            if point_in_polygon(node.point, poly) && !hits.contains(&node.id) {
                hits.push(node.id);
            }
        }
    }
    if is_shift {
        let mut next: Vec<NodeId> = initial_selected
            .iter()
            .copied()
            .filter(|id| !hits.contains(id))
            .collect();
        for id in hits {
            if !initial_selected.contains(&id) {
                next.push(id);
            }
        }
        next
    } else {
        hits
    }
}

#[must_use]
pub fn slanted_marquee_parallelogram_screen(
    start: ScreenPoint,
    current: ScreenPoint,
    angle_deg: f64,
) -> [ScreenPoint; 4] {
    let k = (angle_deg * DEG_TO_RAD_F64)
        .tan()
        .clamp(-MARQUEE_MAX_SLANT_TAN, MARQUEE_MAX_SLANT_TAN) as f32;
    let dy = current.y.0 - start.y.0;
    [
        start,
        ScreenPoint::new(current.x.0 - dy * k, start.y.0),
        current,
        ScreenPoint::new(start.x.0 + dy * k, current.y.0),
    ]
}

#[must_use]
#[allow(clippy::too_many_arguments)]
pub fn select_nodes_in_slanted_marquee(
    contours: &[Vec<Node>],
    viewport: &ViewportTransform,
    _theme: &EditCanvasTokens,
    start: ScreenPoint,
    current: ScreenPoint,
    angle_deg: f64,
    initial_selected: &[NodeId],
    is_shift: bool,
    is_alt: bool,
) -> Vec<NodeId> {
    let quad_scr = slanted_marquee_parallelogram_screen(start, current, angle_deg);
    let quad_font: Vec<FontPoint> = quad_scr.iter().map(|&s| viewport.to_font(s)).collect();
    select_nodes_in_lasso_polygon(contours, &quad_font, initial_selected, is_shift, is_alt)
}

#[must_use]
pub fn next_zoom_step(current_zoom: f32) -> f32 {
    let thresh = current_zoom * ZOOM_STEP_NEXT_THRESHOLD;
    ZOOM_STEPS
        .iter()
        .copied()
        .find(|&s| s > thresh)
        .unwrap_or(ZOOM_TOOL_MAX_SCALE)
}

#[must_use]
pub fn prev_zoom_step(current_zoom: f32) -> f32 {
    let thresh = current_zoom * ZOOM_STEP_PREV_THRESHOLD;
    ZOOM_STEPS
        .iter()
        .rev()
        .copied()
        .find(|&s| s < thresh)
        .unwrap_or(ZOOM_TOOL_MIN_SCALE)
}

#[must_use]
pub fn compute_marquee_zoom(
    current_zoom: f32,
    start_scr: ScreenPoint,
    end_scr: ScreenPoint,
    canvas_w: f32,
    canvas_h: f32,
    is_alt: bool,
) -> Option<(f32, ScreenPoint)> {
    let w = (end_scr.x.0 - start_scr.x.0).abs();
    let h = (end_scr.y.0 - start_scr.y.0).abs();
    if w + h < ZOOM_MARQUEE_MIN_SUM_DP || w <= 1.0 || h <= 1.0 {
        return None;
    }
    let center_scr = ScreenPoint::new(
        (start_scr.x.0 + end_scr.x.0) * 0.5,
        (start_scr.y.0 + end_scr.y.0) * 0.5,
    );
    let factor = if is_alt {
        (w / canvas_w.max(1.0)).min(h / canvas_h.max(1.0))
    } else {
        (canvas_w.max(1.0) / w).min(canvas_h.max(1.0) / h)
    };
    let new_zoom = (current_zoom * factor).clamp(ZOOM_TOOL_MIN_SCALE, ZOOM_TOOL_MAX_SCALE);
    Some((new_zoom, center_scr))
}

#[must_use]
pub fn compute_rotate_tool_affine(
    origin: FontPoint,
    start_font: FontPoint,
    curr_font: FontPoint,
    is_shift: bool,
) -> kurbo::Affine {
    use crate::tokens::{
        RAD_TO_DEG_F64, TRANSFORM_MIN_PIVOT_DIST_UPM, TRANSFORM_ROTATE_SHIFT_STEP_DEG,
    };
    if start_font.distance_to(origin) <= TRANSFORM_MIN_PIVOT_DIST_UPM {
        return kurbo::Affine::IDENTITY;
    }
    let a0 = (start_font.y.0 - origin.y.0).atan2(start_font.x.0 - origin.x.0);
    let a1 = (curr_font.y.0 - origin.y.0).atan2(curr_font.x.0 - origin.x.0);
    let mut d_deg = (a1 - a0) * RAD_TO_DEG_F64;
    if is_shift {
        d_deg = (d_deg / TRANSFORM_ROTATE_SHIFT_STEP_DEG).round() * TRANSFORM_ROTATE_SHIFT_STEP_DEG;
    }
    kurbo::Affine::rotate_about(
        d_deg * DEG_TO_RAD_F64,
        kurbo::Point::new(origin.x.0, origin.y.0),
    )
}

#[must_use]
pub fn compute_scale_tool_affine(
    origin: FontPoint,
    start_font: FontPoint,
    curr_font: FontPoint,
    is_shift: bool,
) -> kurbo::Affine {
    use crate::tokens::{
        SCALE_MIN_FACTOR, SCALE_SHIFT_SIN_HORIZ_MAX, SCALE_SHIFT_SIN_VERT_MIN,
        TRANSFORM_MIN_PIVOT_DIST_UPM,
    };
    let (ax, ay) = (start_font.x.0 - origin.x.0, start_font.y.0 - origin.y.0);
    if ax.abs() <= TRANSFORM_MIN_PIVOT_DIST_UPM || ay.abs() <= TRANSFORM_MIN_PIVOT_DIST_UPM {
        return kurbo::Affine::IDENTITY;
    }
    let (mut sx, mut sy) = (
        (curr_font.x.0 - origin.x.0) / ax,
        (curr_font.y.0 - origin.y.0) / ay,
    );
    if is_shift {
        let u = (curr_font.y.0 - start_font.y.0)
            .atan2(curr_font.x.0 - start_font.x.0)
            .sin()
            .abs();
        if u < SCALE_SHIFT_SIN_HORIZ_MAX {
            sy = 1.0;
        } else if u > SCALE_SHIFT_SIN_VERT_MIN {
            sx = 1.0;
        } else {
            let m = if sx <= 1.0 && sy <= 1.0 {
                sx.abs().min(sy.abs())
            } else {
                sx.abs().max(sy.abs())
            };
            sx = sx.signum() * m;
            sy = sy.signum() * m;
        }
    }
    if sx.abs() < SCALE_MIN_FACTOR {
        sx = (if sx >= 0.0 { 1.0 } else { -1.0 }) * SCALE_MIN_FACTOR;
    }
    if sy.abs() < SCALE_MIN_FACTOR {
        sy = (if sy >= 0.0 { 1.0 } else { -1.0 }) * SCALE_MIN_FACTOR;
    }
    kurbo::Affine::translate((origin.x.0, origin.y.0))
        * kurbo::Affine::scale_non_uniform(sx, sy)
        * kurbo::Affine::translate((-origin.x.0, -origin.y.0))
}

pub fn apply_mode_nodes_transform(
    contours: &mut [Vec<Node>],
    shadow_contours: &[Vec<Node>],
    closed_flags: &[bool],
    selected_nodes: &[NodeId],
    affine: kurbo::Affine,
    is_ctrl_alt: bool,
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
        for (i, node) in contour.iter_mut().enumerate() {
            if selected_nodes.contains(&node.id) {
                let sp = shadow[i].point;
                let tp = affine * kurbo::Point::new(sp.x.0, sp.y.0);
                node.point = round_pt(FontPoint::new(tp.x, tp.y));
            } else {
                node.point = shadow[i].point;
            }
        }
    }
    if is_ctrl_alt {
        crate::core::nudge_ops::apply_ctrl_alt_interpolate_nudge(
            contours,
            shadow_contours,
            closed_flags,
            selected_nodes,
            0.0,
            0.0,
        );
        return;
    }
    for (c_idx, contour) in contours.iter_mut().enumerate() {
        let Some(shadow) = shadow_contours.get(c_idx) else {
            continue;
        };
        let len = contour.len();
        if len < 2 {
            continue;
        }
        let closed = closed_flags.get(c_idx).copied().unwrap_or(true);
        for i in 0..len {
            if contour[i].node_type != NodeType::OffCurve || selected_nodes.contains(&contour[i].id)
            {
                continue;
            }
            let prev = if i > 0 {
                Some(i - 1)
            } else if closed {
                Some(len - 1)
            } else {
                None
            };
            let next = if i + 1 < len {
                Some(i + 1)
            } else if closed {
                Some(0)
            } else {
                None
            };
            let anchor_idx = prev
                .filter(|&p| contour[p].node_type != NodeType::OffCurve)
                .or_else(|| next.filter(|&n| contour[n].node_type != NodeType::OffCurve));
            let Some(a_idx) = anchor_idx else {
                continue;
            };
            if selected_nodes.contains(&contour[a_idx].id) {
                let da_x = contour[a_idx].point.x.0 - shadow[a_idx].point.x.0;
                let da_y = contour[a_idx].point.y.0 - shadow[a_idx].point.y.0;
                contour[i].point = round_pt(FontPoint::new(
                    shadow[i].point.x.0 + da_x,
                    shadow[i].point.y.0 + da_y,
                ));
            }
            if contour[a_idx].node_type == NodeType::Smooth {
                let opp_idx = if prev == Some(a_idx) {
                    if a_idx > 0 {
                        Some(a_idx - 1)
                    } else if closed {
                        Some(len - 1)
                    } else {
                        None
                    }
                } else if a_idx + 1 < len {
                    Some(a_idx + 1)
                } else if closed {
                    Some(0)
                } else {
                    None
                };
                if let Some(b_idx) = opp_idx.filter(|&b| selected_nodes.contains(&contour[b].id)) {
                    let aligned = crate::core::align_collinear_opposite(
                        contour[a_idx].point,
                        contour[i].point,
                        contour[b_idx].point,
                    );
                    contour[i].point = round_pt(aligned);
                }
            }
        }
    }
}
