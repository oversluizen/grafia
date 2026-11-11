//! `SPEC-005` (`glifo v0.4.0`) tokens: Specialized Edit Tools (`KnifeTool`, `PrimitivesTool`,
//! `MeasurementTool`, `AnnotationTool`, `PencilTool`, `PixelTool`), Coordinate & Segment Delta
//! Overlays, and Canvas Annotations (`CanvasAnnotation`).
//!
//! Normative token definitions and specification constants
//! (`SPEC-005 §1 & §3`).

use crate::tokens::{GlifoTheme, RgbaColorToken};

// ============================================================================
// 1. KnifeTool (`CanvasTool` — Shortcut `E` / `Shift+E`)
// ============================================================================

/// Knife line stroke color in Light Mode (`NSColor::grayColor` @ `spec_ref`).
pub const KNIFE_LINE_COLOR_LIGHT: RgbaColorToken = RgbaColorToken::new(128, 128, 128, 230);
/// Knife line stroke color in Dark Mode.
pub const KNIFE_LINE_COLOR_DARK: RgbaColorToken = RgbaColorToken::new(175, 175, 182, 235);

/// Knife intersection circle fill in Light Mode (`handle_rect` @ `spec_ref`).
pub const KNIFE_INTERSECTION_FILL_LIGHT: RgbaColorToken = RgbaColorToken::new(115, 115, 120, 255);
/// Knife intersection circle fill in Dark Mode.
pub const KNIFE_INTERSECTION_FILL_DARK: RgbaColorToken = RgbaColorToken::new(200, 200, 208, 255);

/// Knife line stroke width in `dp` (`1.0 / scale` @ `spec_ref`).
pub const KNIFE_LINE_STROKE_WIDTH_DP: f32 = 1.25;
/// Knife intersection dot radius in `dp` (`handle_rect` @ `spec_ref`).
pub const KNIFE_INTERSECTION_RADIUS_DP: f32 = 3.5;
/// Minimum knife drag distance in UPM (`0.5 UPM` @ `mouseUp:` `spec_ref`).
pub const KNIFE_MIN_DRAG_UPM: f64 = 0.5;
/// Parametric intersection deduplication epsilon along the knife segment.
pub const KNIFE_PARAM_DEDUP_EPS: f64 = 1e-4;
/// Subdivision clamp minimum along a contour segment (`0.001`).
pub const KNIFE_SEG_T_MIN: f64 = 0.001;
/// Subdivision clamp maximum along a contour segment (`0.999`).
pub const KNIFE_SEG_T_MAX: f64 = 0.999;
/// Subdivision sampling count per cubic segment for line-cubic intersection.
pub const KNIFE_CUBIC_SUBDIVISIONS: usize = 32;

// ============================================================================
// 2. PrimitivesTool (`CanvasTool` / `Circle` — `F` / `Shift+F`)
// ============================================================================

/// Minimum primitive width/height (`minSize = 5.0` UPM @ `spec_ref`).
pub const PRIMITIVE_MIN_SIZE_UPM: f64 = 5.0;
/// Default primitive dialog width (`100.0` UPM @ `makePrimitiveWithDialog:` `spec_ref`).
pub const PRIMITIVE_DEFAULT_DIALOG_WIDTH_UPM: f64 = 100.0;
/// Default primitive dialog height (`100.0` UPM @ `makePrimitiveWithDialog:` `spec_ref`).
pub const PRIMITIVE_DEFAULT_DIALOG_HEIGHT_UPM: f64 = 100.0;
/// Cubic Bézier circle approximation constant $\kappa = \frac{4}{3}(\sqrt{2}-1) \approx 0.552284749831$.
pub const PRIMITIVE_CIRCLE_KAPPA: f64 = 0.552_284_749_831;
/// Primitive preview stroke width in `dp` (`1.0 / scale` @ `spec_ref`).
pub const PRIMITIVE_PREVIEW_STROKE_WIDTH_DP: f32 = 1.25;
/// Primitive preview stroke color in Light Mode (`NSColor::textColor` @ `spec_ref`).
pub const PRIMITIVE_PREVIEW_STROKE_LIGHT: RgbaColorToken = RgbaColorToken::new(30, 30, 34, 225);
/// Primitive preview stroke color in Dark Mode.
pub const PRIMITIVE_PREVIEW_STROKE_DARK: RgbaColorToken = RgbaColorToken::new(235, 235, 242, 230);
/// Primitive dimension badge offset in `dp`.
pub const PRIMITIVE_BADGE_OFFSET_DP: f32 = 12.0;

// ============================================================================
// 3. MeasurementTool (`CanvasTool` — `L` & `Cmd+Ctrl+Opt` `spec_ref`)
// ============================================================================

/// Measurement ray stroke color in Light Mode (`selectedControlColor` @ `spec_ref`).
pub const MEASUREMENT_LINE_COLOR_LIGHT: RgbaColorToken = RgbaColorToken::new(46, 117, 219, 230);
/// Measurement ray stroke color in Dark Mode.
pub const MEASUREMENT_LINE_COLOR_DARK: RgbaColorToken = RgbaColorToken::new(82, 154, 250, 240);

/// `OnCurve` node coordinate label color in Light Mode (Blue @ `spec_ref`).
pub const MEASUREMENT_ONCURVE_LABEL_LIGHT: RgbaColorToken = RgbaColorToken::new(26, 102, 204, 245);
/// `OnCurve` node coordinate label color in Dark Mode.
pub const MEASUREMENT_ONCURVE_LABEL_DARK: RgbaColorToken = RgbaColorToken::new(96, 172, 255, 250);

/// `OffCurve` handle coordinate label color in Light Mode (Green @ `spec_ref`).
pub const MEASUREMENT_OFFCURVE_LABEL_LIGHT: RgbaColorToken = RgbaColorToken::new(38, 140, 64, 240);
/// `OffCurve` handle coordinate label color in Dark Mode.
pub const MEASUREMENT_OFFCURVE_LABEL_DARK: RgbaColorToken = RgbaColorToken::new(88, 204, 116, 245);

/// Segment midpoint $(\Delta X, \Delta Y)$ delta label color in Light Mode (Red @ `spec_ref..spec_ref`).
pub const MEASUREMENT_SEGMENT_DELTA_LIGHT: RgbaColorToken = RgbaColorToken::new(191, 51, 51, 235);
/// Segment midpoint $(\Delta X, \Delta Y)$ delta label color in Dark Mode.
pub const MEASUREMENT_SEGMENT_DELTA_DARK: RgbaColorToken = RgbaColorToken::new(245, 98, 98, 245);

/// Measurement ray stroke width in `dp`.
pub const MEASUREMENT_LINE_STROKE_WIDTH_DP: f32 = 1.25;
/// Measurement intersection dot radius in `dp` (`3.5 dp`).
pub const MEASUREMENT_DOT_RADIUS_DP: f32 = 3.5;
/// Measurement node/anchor snap radius in `dp` (`elementAtPoint:atLayer:` @ `spec_ref`).
pub const MEASUREMENT_SNAP_RADIUS_DP: f32 = 8.0;
/// Measurement intersection point deduplication distance in UPM (`0.05 UPM`).
pub const MEASUREMENT_POINT_DEDUP_UPM: f64 = 0.05;
/// Measurement label font size in `dp` (`9.5 dp`).
pub const MEASUREMENT_LABEL_FONT_SIZE_DP: f32 = 9.5;
/// Measurement coordinate label offset from node in `dp` (`6.0 dp`).
pub const MEASUREMENT_LABEL_OFFSET_DP: f32 = 6.0;
/// Measurement angle badge offset from drag end in `dp` (`16.0 dp`).
pub const MEASUREMENT_ANGLE_BADGE_OFFSET_DP: f32 = 16.0;
/// Sidebearing vertical ray half-extent in UPM (`100_000.0 UPM`).
pub const MEASUREMENT_SIDEBEARING_EXTENT_UPM: f64 = 100_000.0;
/// Decimal rounding factor (`10.0`) for `0.1` UPM measurement badges.
pub const MEASUREMENT_DEC_ROUND_FACTOR: f64 = 10.0;

// ============================================================================
// 4. AnnotationTool & `CanvasAnnotation` (`A` / `Shift+A`)
// ============================================================================

/// Default `Circle` annotation radius (`width = 100.0` UPM @ `spec_ref`).
pub const ANNOTATION_DEFAULT_CIRCLE_RADIUS_UPM: f64 = 100.0;
/// Minimum `Circle` annotation radius (`10.0` UPM).
pub const ANNOTATION_MIN_CIRCLE_RADIUS_UPM: f64 = 10.0;
/// Default `Text` annotation box width (`100.0` UPM).
pub const ANNOTATION_DEFAULT_TEXT_WIDTH_UPM: f64 = 100.0;
/// Minimum `Text` annotation box width (`40.0` UPM).
pub const ANNOTATION_MIN_TEXT_WIDTH_UPM: f64 = 40.0;
/// Default `Text` annotation box height in UPM (`40.0` UPM).
pub const ANNOTATION_DEFAULT_TEXT_HEIGHT_UPM: f64 = 40.0;
/// Default `Arrow` annotation angle (`45.0` deg).
pub const ANNOTATION_DEFAULT_ARROW_ANGLE_DEG: f64 = 45.0;
/// Default `Arrow` annotation length (`100.0` UPM).
pub const ANNOTATION_DEFAULT_ARROW_LEN_UPM: f64 = 100.0;
/// Arrowhead wing length in `dp` (`9.0 dp`).
pub const ANNOTATION_ARROW_HEAD_LEN_DP: f32 = 9.0;
/// Arrowhead wing half-angle in radians (`0.45 rad` ~ `25.8 deg`).
pub const ANNOTATION_ARROW_HEAD_ANGLE_RAD: f64 = 0.45;
/// `Plus` / `Minus` annotation half-arm length in `dp` (`8.0 dp`, total span `16.0 dp`).
pub const ANNOTATION_SYMBOL_HALF_ARM_DP: f32 = 8.0;
/// Annotation control handle radius in `dp` (`4.0 dp`).
pub const ANNOTATION_HANDLE_RADIUS_DP: f32 = 4.0;
/// Annotation control handle hit-test radius in `dp` (`8.0 dp`).
pub const ANNOTATION_HANDLE_HIT_RADIUS_DP: f32 = 8.0;
/// Annotation body hit-test tolerance in `dp` (`10.0 dp`).
pub const ANNOTATION_BODY_HIT_RADIUS_DP: f32 = 10.0;
/// Annotation paste collision offset in UPM (`10.0 UPM`).
pub const ANNOTATION_PASTE_OFFSET_UPM: f64 = 10.0;
/// Annotation stroke width in `dp` (`1.5 dp`).
pub const ANNOTATION_STROKE_WIDTH_DP: f32 = 1.5;
/// Annotation text font size in `dp` (`11.0 dp`).
pub const ANNOTATION_FONT_SIZE_DP: f32 = 11.0;

/// Unselected annotation stroke color in Light Mode (`AnnotationTool::drawForegroundInRect:`).
pub const ANNOTATION_STROKE_LIGHT: RgbaColorToken = RgbaColorToken::new(204, 77, 64, 225);
/// Unselected annotation stroke color in Dark Mode.
pub const ANNOTATION_STROKE_DARK: RgbaColorToken = RgbaColorToken::new(242, 108, 94, 235);

/// Selected annotation stroke/handle color in Light Mode.
pub const ANNOTATION_SELECTED_LIGHT: RgbaColorToken = RgbaColorToken::new(230, 42, 32, 255);
/// Selected annotation stroke/handle color in Dark Mode.
pub const ANNOTATION_SELECTED_DARK: RgbaColorToken = RgbaColorToken::new(255, 92, 82, 255);

// ============================================================================
// 5. PencilTool (`PenTool` — Shortcut `B`)
// ============================================================================

/// Minimum freehand sample distance in `dp` (`2.0 dp` @ `PenTool::mouseDragged:` `spec_ref`).
pub const PENCIL_MIN_SAMPLE_DIST_DP: f32 = 2.5;
/// Auto-close radius in `dp` when freehand stroke finishes near its start point (`10.0 dp`).
pub const PENCIL_CLOSE_RADIUS_DP: f32 = 10.0;
/// Ramer-Douglas-Peucker simplification epsilon in UPM (`3.0 UPM`).
pub const PENCIL_RDP_EPSILON_UPM: f64 = 3.0;
/// Catmull-Rom / tangent handle scale factor (`1.0 / 3.0`) for smooth cubic fitting.
pub const PENCIL_TANGENT_HANDLE_FACTOR: f64 = 0.333_333_333_333;
/// Pencil trail stroke width in `dp` (`1.5 dp`).
pub const PENCIL_TRAIL_STROKE_WIDTH_DP: f32 = 1.5;
/// Pencil sample dot radius in `dp` (`2.0 dp`).
pub const PENCIL_SAMPLE_DOT_RADIUS_DP: f32 = 2.0;

/// Pencil live trail stroke color in Light Mode.
pub const PENCIL_TRAIL_STROKE_LIGHT: RgbaColorToken = RgbaColorToken::new(55, 55, 62, 215);
/// Pencil live trail stroke color in Dark Mode.
pub const PENCIL_TRAIL_STROKE_DARK: RgbaColorToken = RgbaColorToken::new(212, 212, 220, 220);

// ============================================================================
// 6. PixelTool (`PixelTool` — Shortcut `X`)
// ============================================================================

/// Default pixel cell size in UPM when `grid_step_upm <= 1.0` (`50.0 UPM`).
pub const PIXEL_DEFAULT_STEP_UPM: f64 = 50.0;
/// Minimum custom `grid_step_upm` threshold (`1.5 UPM`) above which `grid_step_upm` is used as pixel step.
pub const PIXEL_CUSTOM_GRID_MIN_UPM: f64 = 1.5;
/// Pixel cell match tolerance in UPM (`0.5 UPM`).
pub const PIXEL_CELL_MATCH_TOL_UPM: f64 = 0.5;
/// Pixel hover preview stroke width in `dp` (`1.0 dp`).
pub const PIXEL_PREVIEW_STROKE_WIDTH_DP: f32 = 1.0;

/// Pixel hover preview stroke color in Light Mode.
pub const PIXEL_PREVIEW_STROKE_LIGHT: RgbaColorToken = RgbaColorToken::new(115, 115, 125, 150);
/// Pixel hover preview stroke color in Dark Mode.
pub const PIXEL_PREVIEW_STROKE_DARK: RgbaColorToken = RgbaColorToken::new(165, 165, 175, 155);

// ============================================================================
// 7. Boolean `BooleanPathEngine` (`Remove Overlap` / `Subtract` / `Intersect`)
// ============================================================================

/// Initial uniform subdivision count per cubic Bézier for curve-curve intersection detection.
pub const BOOL_CUBIC_SUBDIV_STEPS: usize = 24;
/// Newton-Raphson refinement iterations for cubic-cubic / cubic-line intersections.
pub const BOOL_NEWTON_ITERS: usize = 6;
/// Parametric endpoint clamp epsilon for boolean segment splitting.
pub const BOOL_PARAM_EPS: f64 = 1e-4;
/// Canonical vertex snapping tolerance in UPM (`_intersectSegments:point:` @ `spec_ref`).
pub const BOOL_VERTEX_SNAP_EPS_UPM: f64 = 0.05;
/// Left/right normal probe offset in UPM for `removeInnerSegments:` (`spec_ref`).
pub const BOOL_PROBE_OFFSET_UPM: f64 = 0.05;
/// Normalized cross-product threshold for merging collinear `Line` segments (`removeCornersFromPaths:` @ `spec_ref`).
pub const BOOL_COLLINEAR_CROSS_EPS: f64 = 1e-3;
/// Cosine threshold (`cos(theta) >= 0.995`) for preserving `NodeType::Smooth` across cubic junctions.
pub const BOOL_TANGENT_COS_SMOOTH_MIN: f64 = 0.995;
/// Maximum probe offset as a fraction of segment length (`0.2`).
pub const BOOL_PROBE_MAX_LEN_FRACTION: f64 = 0.2;
/// Angular score bonus (`10.0`) when closing a directed boundary cycle back to its origin vertex.
pub const BOOL_CYCLE_CLOSE_BONUS: f64 = 10.0;

impl GlifoTheme {
    #[must_use]
    pub fn knife_line_color32(&self) -> egui::Color32 {
        (if self.is_dark() { KNIFE_LINE_COLOR_DARK } else { KNIFE_LINE_COLOR_LIGHT }).to_color32()
    }

    #[must_use]
    pub fn knife_intersection_fill_color32(&self) -> egui::Color32 {
        (if self.is_dark() {
            KNIFE_INTERSECTION_FILL_DARK
        } else {
            KNIFE_INTERSECTION_FILL_LIGHT
        })
        .to_color32()
    }

    #[must_use]
    pub fn primitive_preview_stroke_color32(&self) -> egui::Color32 {
        (if self.is_dark() {
            PRIMITIVE_PREVIEW_STROKE_DARK
        } else {
            PRIMITIVE_PREVIEW_STROKE_LIGHT
        })
        .to_color32()
    }

    #[must_use]
    pub fn measurement_line_color32(&self) -> egui::Color32 {
        (if self.is_dark() {
            MEASUREMENT_LINE_COLOR_DARK
        } else {
            MEASUREMENT_LINE_COLOR_LIGHT
        })
        .to_color32()
    }

    #[must_use]
    pub fn measurement_oncurve_label_color32(&self) -> egui::Color32 {
        (if self.is_dark() {
            MEASUREMENT_ONCURVE_LABEL_DARK
        } else {
            MEASUREMENT_ONCURVE_LABEL_LIGHT
        })
        .to_color32()
    }

    #[must_use]
    pub fn measurement_offcurve_label_color32(&self) -> egui::Color32 {
        (if self.is_dark() {
            MEASUREMENT_OFFCURVE_LABEL_DARK
        } else {
            MEASUREMENT_OFFCURVE_LABEL_LIGHT
        })
        .to_color32()
    }

    #[must_use]
    pub fn measurement_segment_delta_color32(&self) -> egui::Color32 {
        (if self.is_dark() {
            MEASUREMENT_SEGMENT_DELTA_DARK
        } else {
            MEASUREMENT_SEGMENT_DELTA_LIGHT
        })
        .to_color32()
    }

    #[must_use]
    pub fn annotation_stroke_color32(&self, selected: bool) -> egui::Color32 {
        match (self.is_dark(), selected) {
            (false, false) => ANNOTATION_STROKE_LIGHT.to_color32(),
            (false, true) => ANNOTATION_SELECTED_LIGHT.to_color32(),
            (true, false) => ANNOTATION_STROKE_DARK.to_color32(),
            (true, true) => ANNOTATION_SELECTED_DARK.to_color32(),
        }
    }

    #[must_use]
    pub fn pencil_trail_stroke_color32(&self) -> egui::Color32 {
        (if self.is_dark() {
            PENCIL_TRAIL_STROKE_DARK
        } else {
            PENCIL_TRAIL_STROKE_LIGHT
        })
        .to_color32()
    }

    #[must_use]
    pub fn pixel_preview_stroke_color32(&self) -> egui::Color32 {
        (if self.is_dark() {
            PIXEL_PREVIEW_STROKE_DARK
        } else {
            PIXEL_PREVIEW_STROKE_LIGHT
        })
        .to_color32()
    }
}
