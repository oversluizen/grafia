//! `SPEC-003` (`v0.2.0`) 8-Knob Selection Bounding Box, 9-point `TransformGridAnchor`,
//! 5-mode `MetricsOrigin`, 3-zone ray hit-testing, and 3-pass `_modeNodes:` transform math.

use crate::core::bezier_ops::{Node, NodeId};
use crate::core::coordinates::{FontPoint, ScreenPoint, ViewportTransform};
pub use crate::core::lasso_and_zoom::{
    apply_mode_nodes_transform, compute_rotate_tool_affine, compute_scale_tool_affine,
    MetricsOrigin, TransformOriginSpec,
};
use crate::tokens::{
    BBOX_ALT_CENTER_SCALE_MULTIPLIER, BBOX_BASE_HANDLE_SIZE_DP, BBOX_FLAT_AXIS_THRESHOLD_UPM,
    BBOX_HANDLE_ZOOM_EXP, BBOX_KNOB_HIT_HALF_EXTENT_DP, BBOX_KNOB_OUTWARD_OFFSET_FACTOR,
    BBOX_MIN_SCREEN_SPAN_DP, BBOX_PILL_ROUND_FACTOR, BBOX_RAY_FALLTHROUGH_MAX_T,
    BBOX_RAY_SCALE_MAX_T, COLLINEAR_MIN_LEN_EPSILON, DEG_TO_RAD_F64, PATH_COORD_EPS_UPM,
    RAD_TO_DEG_F64, SOLVER_DET_EPSILON, TRANSFORM_MIN_PIVOT_DIST_UPM,
    TRANSFORM_ROTATE_SHIFT_STEP_DEG, ZOOM_TOOL_MIN_SCALE,
};
use kurbo::{Affine, Point};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[repr(u8)]
pub enum TransformGridAnchor {
    BottomLeft = 0,
    BottomCenter = 1,
    BottomRight = 2,
    MiddleLeft = 3,
    #[default]
    Center = 4,
    MiddleRight = 5,
    TopLeft = 6,
    TopCenter = 7,
    TopRight = 8,
}

impl TransformGridAnchor {
    #[must_use]
    pub const fn from_index(idx: u8) -> Self {
        match idx {
            0 => Self::BottomLeft,
            1 => Self::BottomCenter,
            2 => Self::BottomRight,
            3 => Self::MiddleLeft,
            5 => Self::MiddleRight,
            6 => Self::TopLeft,
            7 => Self::TopCenter,
            8 => Self::TopRight,
            _ => Self::Center,
        }
    }
    #[must_use]
    pub const fn index(self) -> u8 {
        self as u8
    }
    #[must_use]
    pub const fn opposite(self) -> Self {
        Self::from_index(8 - (self as u8))
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BBoxAction {
    Scale(u8),
    Rotate(u8),
    Slant(u8),
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SelectionBounds {
    pub min_x: f64,
    pub max_x: f64,
    pub min_y: f64,
    pub max_y: f64,
    pub angle_deg: f64,
    pub half_x_height: f64,
}

impl SelectionBounds {
    #[must_use]
    pub fn width(&self) -> f64 {
        self.max_x - self.min_x
    }
    #[must_use]
    pub fn height(&self) -> f64 {
        self.max_y - self.min_y
    }
    #[must_use]
    pub fn is_displayable(&self, zoom: f32) -> bool {
        (self.width() as f32) * zoom > BBOX_MIN_SCREEN_SPAN_DP
            || (self.height() as f32) * zoom > BBOX_MIN_SCREEN_SPAN_DP
    }
    #[must_use]
    pub fn active_knobs(&self, zoom: f32) -> Vec<u8> {
        if !self.is_displayable(zoom) {
            return Vec::new();
        }
        let (w, h) = (self.width(), self.height());
        if w <= BBOX_FLAT_AXIS_THRESHOLD_UPM && (h as f32) * zoom > BBOX_MIN_SCREEN_SPAN_DP {
            vec![1, 7]
        } else if h <= BBOX_FLAT_AXIS_THRESHOLD_UPM && (w as f32) * zoom > BBOX_MIN_SCREEN_SPAN_DP {
            vec![3, 5]
        } else {
            vec![6, 7, 8, 3, 5, 0, 1, 2]
        }
    }
    #[must_use]
    pub fn aligned_point_unslanted(&self, index: u8) -> FontPoint {
        let (w, h) = (self.width(), self.height());
        let x = match index {
            0 | 3 | 6 => self.min_x,
            2 | 5 | 8 => self.max_x,
            _ => self.min_x + 0.5 * w,
        };
        let y = match index {
            0..=2 => self.min_y,
            6..=8 => self.max_y,
            _ => self.min_y + 0.5 * h,
        };
        FontPoint::new(x, y)
    }
    #[must_use]
    pub fn aligned_point(&self, index: u8) -> FontPoint {
        slant_point(
            self.aligned_point_unslanted(index),
            self.angle_deg,
            self.half_x_height,
        )
    }
    #[must_use]
    pub fn box_screen_point(&self, index: u8, viewport: &ViewportTransform) -> ScreenPoint {
        viewport.to_screen(self.aligned_point(index))
    }
    #[must_use]
    pub fn knob_screen_center(&self, index: u8, viewport: &ViewportTransform) -> ScreenPoint {
        let zoom = viewport.zoom.max(ZOOM_TOOL_MIN_SCALE);
        let handle_size_dp = BBOX_BASE_HANDLE_SIZE_DP * zoom.powf(BBOX_HANDLE_ZOOM_EXP);
        let delta_out = f64::from((BBOX_KNOB_OUTWARD_OFFSET_FACTOR * handle_size_dp) / zoom);
        let expanded = Self {
            min_x: self.min_x - delta_out,
            max_x: self.max_x + delta_out,
            min_y: self.min_y - delta_out,
            max_y: self.max_y + delta_out,
            angle_deg: self.angle_deg,
            half_x_height: self.half_x_height,
        };
        viewport.to_screen(expanded.aligned_point(index))
    }
    #[must_use]
    pub fn format_dimension_badge(val: f64) -> String {
        let rounded = (val * BBOX_PILL_ROUND_FACTOR).round() / BBOX_PILL_ROUND_FACTOR;
        if (rounded - rounded.round()).abs() < PATH_COORD_EPS_UPM {
            format!("{:.0}", rounded.round())
        } else {
            format!("{rounded:.1}")
        }
    }
}

#[must_use]
pub fn unslant_point(pt: FontPoint, angle_deg: f64, half_x_height: f64) -> FontPoint {
    if angle_deg.abs() <= SOLVER_DET_EPSILON {
        return pt;
    }
    FontPoint::new(
        pt.x.0 - (pt.y.0 - half_x_height) * (angle_deg * DEG_TO_RAD_F64).tan(),
        pt.y.0,
    )
}

#[must_use]
pub fn slant_point(pt: FontPoint, angle_deg: f64, half_x_height: f64) -> FontPoint {
    if angle_deg.abs() <= SOLVER_DET_EPSILON {
        return pt;
    }
    FontPoint::new(
        pt.x.0 + (pt.y.0 - half_x_height) * (angle_deg * DEG_TO_RAD_F64).tan(),
        pt.y.0,
    )
}

#[must_use]
pub fn compute_selection_bounds(
    contours: &[Vec<Node>],
    selected: &[NodeId],
    angle_deg: f64,
    half_x_height: f64,
) -> Option<SelectionBounds> {
    if selected.len() < 2 {
        return None;
    }
    let (mut min_x, mut max_x, mut min_y, mut max_y) = (
        f64::INFINITY,
        f64::NEG_INFINITY,
        f64::INFINITY,
        f64::NEG_INFINITY,
    );
    let mut count = 0usize;
    for contour in contours {
        for node in contour {
            if selected.contains(&node.id) {
                let p = unslant_point(node.point, angle_deg, half_x_height);
                min_x = min_x.min(p.x.0);
                max_x = max_x.max(p.x.0);
                min_y = min_y.min(p.y.0);
                max_y = max_y.max(p.y.0);
                count += 1;
            }
        }
    }
    (count >= 2).then_some(SelectionBounds {
        min_x,
        max_x,
        min_y,
        max_y,
        angle_deg,
        half_x_height,
    })
}

#[must_use]
pub fn compute_layer_bounds(
    contours: &[Vec<Node>],
    angle_deg: f64,
    half_x_height: f64,
) -> Option<SelectionBounds> {
    let (mut min_x, mut max_x, mut min_y, mut max_y) = (
        f64::INFINITY,
        f64::NEG_INFINITY,
        f64::INFINITY,
        f64::NEG_INFINITY,
    );
    let mut any = false;
    for contour in contours {
        for node in contour {
            let p = unslant_point(node.point, angle_deg, half_x_height);
            min_x = min_x.min(p.x.0);
            max_x = max_x.max(p.x.0);
            min_y = min_y.min(p.y.0);
            max_y = max_y.max(p.y.0);
            any = true;
        }
    }
    any.then_some(SelectionBounds {
        min_x,
        max_x,
        min_y,
        max_y,
        angle_deg,
        half_x_height,
    })
}

#[must_use]
pub fn hit_test_bbox_knobs(
    bounds: &SelectionBounds,
    viewport: &ViewportTransform,
    cursor_scr: ScreenPoint,
    is_ctrl: bool,
    is_alt: bool,
) -> Option<BBoxAction> {
    let half_box = BBOX_KNOB_HIT_HALF_EXTENT_DP * 2.0;
    for k in bounds.active_knobs(viewport.zoom) {
        let p_box = bounds.box_screen_point(k, viewport);
        let p_knob = bounds.knob_screen_center(k, viewport);
        if (cursor_scr.x.0 - p_knob.x.0).abs() > half_box
            || (cursor_scr.y.0 - p_knob.y.0).abs() > half_box
        {
            continue;
        }
        let (vx, vy) = (
            f64::from(p_knob.x.0 - p_box.x.0),
            f64::from(p_knob.y.0 - p_box.y.0),
        );
        let len_sq = vx * vx + vy * vy;
        if len_sq <= SOLVER_DET_EPSILON {
            continue;
        }
        let (qx, qy) = (
            f64::from(cursor_scr.x.0 - p_box.x.0),
            f64::from(cursor_scr.y.0 - p_box.y.0),
        );
        let t = (qx * vx + qy * vy) / len_sq;
        if t <= BBOX_RAY_FALLTHROUGH_MAX_T {
            continue;
        } else if t <= BBOX_RAY_SCALE_MAX_T {
            return Some(BBoxAction::Scale(k));
        } else if k == 1 || (is_ctrl && is_alt && matches!(k, 3 | 5 | 7)) {
            return Some(BBoxAction::Slant(k));
        } else {
            return Some(BBoxAction::Rotate(k));
        }
    }
    None
}

#[must_use]
pub fn compute_bbox_scale_affine(
    bounds: &SelectionBounds,
    k: u8,
    start_font: FontPoint,
    curr_font: FontPoint,
    is_shift: bool,
    is_alt: bool,
    is_ctrl: bool,
) -> Affine {
    let alt_center = is_alt && !is_ctrl;
    let pivot = bounds.aligned_point(if alt_center { 4 } else { 8 - k });
    let mult = if alt_center {
        BBOX_ALT_CENTER_SCALE_MULTIPLIER
    } else {
        1.0
    };
    let s_u = unslant_point(start_font, bounds.angle_deg, bounds.half_x_height);
    let c_u = unslant_point(curr_font, bounds.angle_deg, bounds.half_x_height);
    let (dx, dy) = ((c_u.x.0 - s_u.x.0) * mult, (c_u.y.0 - s_u.y.0) * mult);
    let (w, h) = (bounds.width(), bounds.height());
    let mut sx = if matches!(k, 0 | 3 | 6) && w > COLLINEAR_MIN_LEN_EPSILON {
        (w - dx) / w
    } else if matches!(k, 2 | 5 | 8) && w > COLLINEAR_MIN_LEN_EPSILON {
        (w + dx) / w
    } else {
        1.0
    };
    let mut sy = if matches!(k, 0..=2) && h > COLLINEAR_MIN_LEN_EPSILON {
        (h - dy) / h
    } else if matches!(k, 6..=8) && h > COLLINEAR_MIN_LEN_EPSILON {
        (h + dy) / h
    } else {
        1.0
    };
    if is_shift {
        match k {
            0 | 2 | 6 | 8 => {
                let s = sx.max(sy);
                sx = s;
                sy = s;
            }
            3 | 5 => sy = sx,
            1 | 7 => sx = sy,
            _ => {}
        }
    }
    Affine::translate((pivot.x.0, pivot.y.0))
        * Affine::scale_non_uniform(sx, sy)
        * Affine::translate((-pivot.x.0, -pivot.y.0))
}

#[must_use]
pub fn compute_bbox_rotate_or_slant_affine(
    bounds: &SelectionBounds,
    action: BBoxAction,
    start_font: FontPoint,
    curr_font: FontPoint,
    is_shift: bool,
    is_alt: bool,
    is_ctrl: bool,
) -> Affine {
    let k = match action {
        BBoxAction::Scale(idx) | BBoxAction::Rotate(idx) | BBoxAction::Slant(idx) => idx,
    };
    let pivot = bounds.aligned_point(if is_alt && !is_ctrl { 4 } else { 8 - k });
    if start_font.distance_to(pivot) < TRANSFORM_MIN_PIVOT_DIST_UPM {
        return Affine::IDENTITY;
    }
    let a0 = (start_font.y.0 - pivot.y.0).atan2(start_font.x.0 - pivot.x.0);
    let a1 = (curr_font.y.0 - pivot.y.0).atan2(curr_font.x.0 - pivot.x.0);
    let mut d_deg = (a1 - a0) * RAD_TO_DEG_F64;
    match action {
        BBoxAction::Rotate(_) => {
            if is_shift {
                d_deg = (d_deg / TRANSFORM_ROTATE_SHIFT_STEP_DEG).round()
                    * TRANSFORM_ROTATE_SHIFT_STEP_DEG;
            }
            Affine::rotate_about(d_deg * DEG_TO_RAD_F64, Point::new(pivot.x.0, pivot.y.0))
        }
        BBoxAction::Slant(_) => {
            let shear = if matches!(k, 1 | 7) {
                Affine::new([1.0, 0.0, (-d_deg * DEG_TO_RAD_F64).tan(), 1.0, 0.0, 0.0])
            } else {
                Affine::new([1.0, (d_deg * DEG_TO_RAD_F64).tan(), 0.0, 1.0, 0.0, 0.0])
            };
            Affine::translate((pivot.x.0, pivot.y.0))
                * shear
                * Affine::translate((-pivot.x.0, -pivot.y.0))
        }
        BBoxAction::Scale(_) => Affine::IDENTITY,
    }
}
