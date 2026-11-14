//! `SPEC-009` (`grafia v0.8.0` Part A) tokens, data contracts, constants, and theme colors:
//! Tension-Controlled Hobby Splines (`NodeType::HobbyStar`) & Euler/Spiro Clothoid Splines (`NodeType::EulerSpiro`),
//! 8+4 Bounding-Box Outer-Corner Rotation & Shear, On-Canvas Parametric Smart Handles (`CanvasSmartHandle`),
//! Self-Intersection Virtual `ExtraNode`s, `CanvasBackgroundImage`, Curve Conversions, and All-Layers Path Ops.

pub use crate::core::canvas_entities::{
    BackgroundImageInfo, BackgroundImageMutation, CanvasBackgroundImage, CanvasSmartHandle,
    ExtraNodeKey, HobbyDirConstraint, HobbyNodeInfo, LayerPathOp, VirtualExtraNode,
};
use crate::tokens::GrafiaTheme;

// ============================================================================
// Hobby & Euler/Spiro Spline Solver Constants
// ============================================================================

pub const HOBBY_TENSION_MIN: f64 = 0.75;
pub const HOBBY_TENSION_MAX: f64 = 4.0;
pub const HOBBY_TENSION_DEFAULT: f64 = 1.0;
pub const HOBBY_RHO_SQRT2: f64 = std::f64::consts::SQRT_2;
pub const HOBBY_RHO_SIN_SIXTEENTH: f64 = 0.0625;
pub const HOBBY_RHO_COS_THETA_WEIGHT: f64 = 0.618_033_988_749_894_9;
pub const HOBBY_RHO_COS_PHI_WEIGHT: f64 = 0.381_966_011_250_105_1;
pub const HOBBY_HANDLE_DIVISOR: f64 = 3.0;
pub const HOBBY_MOCK_CURVATURE_FACTOR: f64 = 3.0;
pub const HOBBY_MIN_CHORD_EPS: f64 = 1e-9;
pub const HOBBY_SOLVER_PIVOT_EPS: f64 = 1e-12;
pub const HOBBY_RHO_MIN_CLAMP: f64 = 0.05;
pub const HOBBY_RHO_MAX_CLAMP: f64 = 4.0;

/// 5-point Gauss-Legendre quadrature nodes and weights mapped onto `[-0.5, 0.5]`.
pub const EULER_GL5_NODES: [f64; 5] = [
    -0.453_089_922_969_332,
    -0.269_234_655_052_841_55,
    0.0,
    0.269_234_655_052_841_55,
    0.453_089_922_969_332,
];
pub const EULER_GL5_WEIGHTS: [f64; 5] = [
    0.118_463_442_528_094_54,
    0.239_314_335_249_683_24,
    0.284_444_444_444_444_44,
    0.239_314_335_249_683_24,
    0.118_463_442_528_094_54,
];
pub const EULER_K1_NEWTON_ITERS: usize = 6;
pub const EULER_CHAIN_NEWTON_ITERS: usize = 16;
pub const EULER_FD_STEP: f64 = 1e-6;
pub const EULER_DAMPING_STEP: f64 = 0.8;
pub const EULER_CONVERGENCE_TOL: f64 = 1e-10;
pub const EULER_K1_EIGHTH: f64 = 0.125;
pub const EULER_END_EIGHTH: f64 = 0.125;
pub const EULER_MID_EIGHT_THIRDS: f64 = 2.666_666_666_666_666_5;

/// Screen-space geometry for `HobbyStar` (`6.0 dp` nominal star) and `EulerSpiro` (`6.0 dp` concentric ring).
pub const HOBBY_STAR_OUTER_RADIUS_DP: f32 = 4.2;
pub const HOBBY_STAR_INNER_RADIUS_DP: f32 = 1.9;
pub const HOBBY_STAR_VERTICES: usize = 10;
pub const HOBBY_STAR_HALF_STEP_RAD: f32 = 0.628_318_55;
pub const HOBBY_STAR_TOP_ANGLE_RAD: f32 = -std::f32::consts::FRAC_PI_2;
pub const EULER_SPIRO_OUTER_RADIUS_DP: f32 = 3.6;
pub const EULER_SPIRO_INNER_RADIUS_DP: f32 = 1.5;
pub const EULER_SPIRO_RING_WIDTH_DP: f32 = 1.2;

// ============================================================================
// Curve Conversion, BBox 8+4, Smart Handles & ExtraNodes Constants
// ============================================================================

pub const QUAD_APPROX_TOLERANCE_UPM: f64 = 0.5;
pub const QUAD_MAX_SUBDIV_DEPTH: u8 = 8;
pub const QUAD_THREE_QUARTERS: f64 = 0.75;
pub const QUAD_QUARTER: f64 = 0.25;
pub const QUAD_SAMPLE_TS: [f64; 7] = [0.125, 0.25, 0.375, 0.5, 0.625, 0.75, 0.875];
pub const ONE_THIRD_F64: f64 = 1.0 / 3.0;
pub const TWO_THIRDS_F64: f64 = 2.0 / 3.0;
pub const INTERPOLATE_T_MIN: f64 = -0.5;
pub const INTERPOLATE_T_MAX: f64 = 1.5;
pub const PERCENT_SCALE_FACTOR: f64 = 100.0;
pub const ALPHA_MIN_F32: f32 = 0.0;
pub const ALPHA_MAX_F32: f32 = 1.0;

pub const BBOX_ROTATE_OUTER_MIN_DP: f32 = 6.0;
pub const BBOX_ROTATE_OUTER_MAX_DP: f32 = 18.0;
pub const BBOX_ROTATE_SHIFT_STEP_DEG: f64 = 15.0;
pub const BBOX_ROTATE_PILL_OFFSET_X_DP: f32 = 14.0;
pub const BBOX_ROTATE_PILL_OFFSET_Y_DP: f32 = -16.0;

pub const SMART_HANDLE_DIAMOND_RADIUS_DP: f32 = 5.5;
pub const SMART_HANDLE_HIT_RADIUS_DP: f32 = 8.0;
pub const SMART_HANDLE_DEFAULT_SNAP_DP: f32 = 4.0;
pub const SMART_HANDLE_TRACK_WIDTH_DP: f32 = 1.25;
pub const SMART_HANDLE_TICK_RADIUS_DP: f32 = 2.2;
pub const SMART_HANDLE_PILL_OFFSET_Y_DP: f32 = -14.0;

pub const EXTRA_NODE_INTERIOR_T_MIN: f64 = 1e-4;
pub const EXTRA_NODE_INTERIOR_T_MAX: f64 = 0.9999;
pub const EXTRA_NODE_DEDUP_T_EPS: f64 = 1e-3;
pub const EXTRA_NODE_DEDUP_DIST_UPM: f64 = 0.05;
pub const EXTRA_NODE_DIAMOND_RADIUS_DP: f32 = 5.0;
pub const EXTRA_NODE_STROKE_WIDTH_DP: f32 = 1.4;
pub const EXTRA_NODE_HIT_RADIUS_DP: f32 = 7.0;

pub const NODE_COORD_LABEL_FONT_SIZE_DP: f32 = 9.0;
pub const NODE_COORD_LABEL_OFFSET_X_DP: f32 = 6.0;
pub const NODE_COORD_LABEL_OFFSET_Y_DP: f32 = 4.0;
pub const COMPONENT_PIECE_FRAME_WIDTH_DP: f32 = 1.0;
pub const BG_IMAGE_SELECTION_WIDTH_DP: f32 = 1.25;
pub const RGBA_CHANNEL_MAX_F32: f32 = 255.0;
pub const CIRCLE_POLY_SEGMENTS: usize = 16;
pub const TAU_F32: f32 = std::f32::consts::TAU;
pub const PI_F64: f64 = std::f64::consts::PI;
pub const TAU_F64: f64 = std::f64::consts::TAU;
pub const METRIC_LINE_SNAP_TOL_UPM: f64 = 12.0;
pub const COLLINEAR_CROSS_TOL: f64 = 1e-4;
pub const EULER_PAR_DET_EPS: f64 = 1e-5;
pub const EXTRA_NODE_SOLVE_STEPS: usize = 64;
pub const EXTRA_NODE_BISECT_ITERS: usize = 28;
pub const EXTRA_NODE_HIT_DIST_EPS_SQ: f64 = 1e-6;
pub const BBOX_SHEAR_MIN_SPAN_UPM: f64 = 1e-6;
pub const SMART_HANDLE_MIN_SCALE_EPS: f64 = 1e-9;
pub const PILL_PADDING_X_DP: f32 = 5.0;
pub const PILL_PADDING_Y_DP: f32 = 3.0;
pub const PILL_GLYPH_STEP_X_DP: f32 = 4.0;
pub const PILL_GLYPH_H_DP: f32 = 5.0;
pub const PILL_CORNER_RADIUS_DP: f32 = 3.0;

#[must_use]
pub fn white_tint_with_alpha_color32(alpha: f32) -> egui::Color32 {
    let a = (alpha.clamp(ALPHA_MIN_F32, ALPHA_MAX_F32) * RGBA_CHANNEL_MAX_F32).round() as u8;
    egui::Color32::from_rgba_unmultiplied(u8::MAX, u8::MAX, u8::MAX, a)
}

impl GrafiaTheme {
    #[must_use]
    pub fn hobby_star_node_color32(&self) -> egui::Color32 {
        if self.is_dark() {
            egui::Color32::from_rgb(255, 196, 61)
        } else {
            egui::Color32::from_rgb(214, 134, 0)
        }
    }

    #[must_use]
    pub fn euler_spiro_node_color32(&self) -> egui::Color32 {
        if self.is_dark() {
            egui::Color32::from_rgb(76, 209, 196)
        } else {
            egui::Color32::from_rgb(13, 148, 136)
        }
    }

    #[must_use]
    pub fn smart_handle_fill_color32(&self) -> egui::Color32 {
        self.smart_handle_color.to_color32()
    }

    #[must_use]
    pub fn smart_handle_track_color32(&self) -> egui::Color32 {
        let [r, g, b, _] = self.smart_handle_color.to_color32().to_srgba_unmultiplied();
        let a = if self.is_dark() { 135 } else { 125 };
        egui::Color32::from_rgba_unmultiplied(r, g, b, a)
    }

    #[must_use]
    pub fn extra_node_stroke_color32(&self) -> egui::Color32 {
        self.hoi_diamond_color.to_color32()
    }

    #[must_use]
    pub fn extra_node_selected_fill_color32(&self) -> egui::Color32 {
        if self.is_dark() {
            egui::Color32::from_rgba_unmultiplied(255, 145, 77, 110)
        } else {
            egui::Color32::from_rgba_unmultiplied(217, 95, 14, 95)
        }
    }

    #[must_use]
    pub fn component_piece_frame_color32(&self) -> egui::Color32 {
        if self.is_dark() {
            egui::Color32::from_rgba_unmultiplied(120, 180, 255, 145)
        } else {
            egui::Color32::from_rgba_unmultiplied(45, 115, 210, 145)
        }
    }

    #[must_use]
    pub fn background_node_fill_color32(&self) -> egui::Color32 {
        if self.is_dark() {
            egui::Color32::from_rgba_unmultiplied(185, 155, 235, 175)
        } else {
            egui::Color32::from_rgba_unmultiplied(135, 95, 195, 175)
        }
    }

    #[must_use]
    pub fn node_coordinate_label_color32(&self) -> egui::Color32 {
        if self.is_dark() {
            egui::Color32::from_rgba_unmultiplied(220, 224, 232, 220)
        } else {
            egui::Color32::from_rgba_unmultiplied(48, 52, 62, 220)
        }
    }
}
