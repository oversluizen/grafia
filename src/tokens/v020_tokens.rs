//! `SPEC-003` (`glifo v0.2.0`) tokens: 8-Knob Bounding Box, `RotateTool`, `ScaleTool`,
//! `LassoTool`, `ZoomTool`, Clipboard, Path Operations, and `TransformPanel` constants & colors.
//!
//! Normative token definitions and specification constants
//! verified against deterministic golden-master vectors (`SPEC-003 §2–§6`).

/// Minimum screen span (`w * zoom > 4.0` or `h * zoom > 4.0`) for displaying the Selection Bounding Box (`spec_ref`).
pub const BBOX_MIN_SCREEN_SPAN_DP: f32 = 4.0;

/// Base bounding-box knob handle size in `dp` (`5.0 * zoom.powf(0.1)` @ `spec_ref`).
pub const BBOX_BASE_HANDLE_SIZE_DP: f32 = 5.0;

/// Zoom exponent for bounding-box handle scaling (`zoom.powf(0.1)`).
pub const BBOX_HANDLE_ZOOM_EXP: f32 = 0.1;

/// Outward offset multiplier (`1.5 * handle_size_dp`) placing the 8 knobs outside the dashed box (`spec_ref`).
pub const BBOX_KNOB_OUTWARD_OFFSET_FACTOR: f32 = 1.5;

/// Outer hit-test box half-extent factor (`1.5 * base_dp` = `7.5 dp` radius / `15.0 x 15.0 dp` square @ `spec_ref`).
pub const BBOX_KNOB_HIT_HALF_EXTENT_DP: f32 = 7.5;

/// Inner ray-projection threshold $t \le 0.7$ below which knob hit-testing misses and falls through to nodes (`spec_ref`).
pub const BBOX_RAY_FALLTHROUGH_MAX_T: f64 = 0.7;

/// Scale ray-projection upper threshold $0.7 < t \le 1.7$ (`spec_ref`).
pub const BBOX_RAY_SCALE_MAX_T: f64 = 1.7;

/// Degenerate 1-axis width/height threshold in UPM (`w <= 1.0` -> `[1, 7]`; `h <= 1.0` -> `[3, 5]` @ `spec_ref`).
pub const BBOX_FLAT_AXIS_THRESHOLD_UPM: f64 = 1.0;

/// Center-pivot drag multiplier (`2.0`) when `Alt/Option` (`!Ctrl`) is held during a bounding-box drag (`spec_ref`).
pub const BBOX_ALT_CENTER_SCALE_MULTIPLIER: f64 = 2.0;

/// Dashed bounding-box stroke width and dash length in `dp` (`1.75 dp`).
pub const BBOX_DASH_STROKE_WIDTH_DP: f32 = 1.75;

/// Circular bounding-box knob stroke width in `dp` (`0.8 dp`).
pub const BBOX_KNOB_STROKE_WIDTH_DP: f32 = 0.8;

/// Live Width/Height capsule pill font size in `dp` (`[NSFont systemFontOfSize:11.0]` @ `spec_ref`).
pub const BBOX_PILL_FONT_SIZE_DP: f32 = 11.0;

/// Live Width/Height capsule pill minimum height in `dp` (`12.0 dp` @ `spec_ref`).
pub const BBOX_PILL_MIN_HEIGHT_DP: f32 = 12.0;

/// Live Width/Height capsule pill horizontal padding (`+7.0 dp` @ `spec_ref`).
pub const BBOX_PILL_PAD_X_DP: f32 = 7.0;

/// Approximate character advance width in `dp` for sizing the capsule pill in headless/egui rendering.
pub const BBOX_PILL_CHAR_WIDTH_DP: f32 = 6.5;

/// Screen offset (`6.0 dp`) from the bounding box bottom/right edge to the Width/Height pill badge (`spec_ref`).
pub const BBOX_PILL_OFFSET_DP: f32 = 6.0;

/// Rounding factor (`10.0`) for `GSProxy::roundFloat:error:10.0` (`1` decimal place).
pub const BBOX_PILL_ROUND_FACTOR: f64 = 10.0;

/// Maximum dynamic marquee slant angle in degrees (`[-60.0, +60.0]` @ `spec_ref`).
pub const MARQUEE_MAX_SLANT_DEG: f64 = 60.0;

/// Maximum tangent clamp for dynamic marquee slant (`tan(60°) = sqrt(3)`).
pub const MARQUEE_MAX_SLANT_TAN: f64 = 1.732_050_807_568_877_2;

/// Snap tolerance in degrees (`2.0°`) to `italic_angle_deg` during `Ctrl` marquee slant (`spec_ref`).
pub const MARQUEE_ITALIC_SNAP_TOL_DEG: f64 = 2.0;

/// Hit radius in `dp` (`3.0 dp`) to grab and drag `transform_origin` directly in `RotateTool` / `ScaleTool` (`spec_ref`).
pub const TRANSFORM_ORIGIN_HIT_RADIUS_DP: f32 = 3.0;

/// Maximum pointer movement in `dp` (`2.0 dp`) treated as a click that relocates `transform_origin` (`spec_ref`).
pub const TRANSFORM_CLICK_THRESHOLD_DP: f32 = 2.0;

/// Minimum pivot distance in UPM (`5.0 UPM`) required to activate rotation or scaling (`spec_ref`, `spec_ref`).
pub const TRANSFORM_MIN_PIVOT_DIST_UPM: f64 = 5.0;

/// Angular snap increment in degrees (`45.0°`) when `Shift` is held during rotation (`spec_ref`).
pub const TRANSFORM_ROTATE_SHIFT_STEP_DEG: f64 = 45.0;

/// `ScaleTool` `Shift` Zone 1 upper threshold (`|sin(theta)| < 0.2` -> horizontal-only scale `sy = 1.0` @ `spec_ref`).
pub const SCALE_SHIFT_SIN_HORIZ_MAX: f64 = 0.2;

/// `ScaleTool` `Shift` Zone 2 lower threshold (`|sin(theta)| > 0.86` -> vertical-only scale `sx = 1.0` @ `spec_ref`).
pub const SCALE_SHIFT_SIN_VERT_MIN: f64 = 0.86;

/// Minimum scale factor magnitude (`0.01`) in `ScaleTool` and `TransformPanel` (`spec_ref`, `spec_ref`).
pub const SCALE_MIN_FACTOR: f64 = 0.01;

/// Maximum slant angle magnitude (`89.0°`) in `TransformPanel` (`spec_ref`).
pub const SLANT_MAX_ANGLE_DEG: f64 = 89.0;

/// Background circle radius (`8.0 dp`, diameter `16.0 dp`) of the on-canvas `transform_origin` reticle (`spec_ref`).
pub const ORIGIN_RETICLE_BG_RADIUS_DP: f32 = 8.0;

/// Stroke width (`1.2 dp`) of the `transform_origin` circle and active drag ray line (`spec_ref`).
pub const ORIGIN_RETICLE_STROKE_WIDTH_DP: f32 = 1.2;

/// Inner start offset (`2.0 dp`) and outer end offset (`5.0 dp`) for the 4 `transform_origin` crosshair ticks (`spec_ref`).
pub const ORIGIN_RETICLE_TICK_INNER_DP: f32 = 2.0;
pub const ORIGIN_RETICLE_TICK_OUTER_DP: f32 = 5.0;

/// Half-size (`1.5 dp`, `3.0 x 3.0 dp` square) of the `transform_origin` center square (`spec_ref`).
pub const ORIGIN_RETICLE_CENTER_HALF_DP: f32 = 1.5;

/// Minimum screen step (`2.0 dp`) between sampled vertices in `LassoTool` (`addPointToDragPath:` @ `spec_ref`).
pub const LASSO_MIN_SAMPLE_STEP_DP: f32 = 2.0;

/// Solid stroke width (`1.5 dp`) of the `LassoTool` selection polygon (`spec_ref`).
pub const LASSO_STROKE_WIDTH_DP: f32 = 1.5;

/// Minimum `w + h` screen threshold (`10.0 dp`) for `ZoomTool` marquee zoom vs. click zoom (`spec_ref`).
pub const ZOOM_MARQUEE_MIN_SUM_DP: f32 = 10.0;

/// Minimum zoom scale (`0.008`) and maximum zoom scale (`40.0`) in `ZoomTool` (`ZOOM_STEPS` @ `spec_ref`).
pub const ZOOM_TOOL_MIN_SCALE: f32 = 0.008;
pub const ZOOM_TOOL_MAX_SCALE: f32 = 40.0;

/// Multiplier thresholds (`1.08` for `nextZoomStep:`, `0.92` for `previousZoomStep:` @ `spec_ref`, `spec_ref`).
pub const ZOOM_STEP_NEXT_THRESHOLD: f32 = 1.08;
pub const ZOOM_STEP_PREV_THRESHOLD: f32 = 0.92;

/// Authentic 23-entry `ZOOM_STEPS` table (`CanvasEngine spec_ref`).
pub const ZOOM_STEPS: [f32; 23] = [
    0.008, 0.01, 0.012, 0.018, 0.024, 0.034, 0.05, 0.075, 0.1, 0.16, 0.24, 0.34, 0.5, 0.66, 1.0,
    1.5, 2.0, 3.0, 4.0, 6.0, 10.0, 20.0, 40.0,
];

/// Collision offset (`+10.0 UPM`) applied by `pasteDict:` when an identical contour already exists at `(0, 0)` (`spec_ref`).
pub const CLIPBOARD_COLLISION_OFFSET_UPM: f64 = 10.0;

/// Coordinate comparison tolerance (`0.001 UPM`) for clipboard collision detection and `findMinNode` tie-breaking (`spec_ref`).
pub const PATH_COORD_EPS_UPM: f64 = 0.001;

/// Minimum parameter $t \in (0.001, 0.999)$ for `add_extremes` and `add_inflection_points` (`spec_ref`, `spec_ref`).
pub const CURVE_ROOT_T_MIN: f64 = 0.001;
pub const CURVE_ROOT_T_MAX: f64 = 0.999;

/// Root deduplication epsilon ($\Delta t < 10^{-4}$).
pub const CURVE_ROOT_DEDUP_EPS: f64 = 0.0001;

/// Minimum distance (`0.8 UPM`) from segment endpoints $P_0, P_3$ for inserting an extremum (`spec_ref`).
pub const EXTREMUM_MIN_ENDPOINT_DIST_UPM: f64 = 0.8;

/// Near-endpoint smooth absorption distance threshold (`30.0 UPM`) when `force == false` in `add_extremes` (`spec_ref`).
pub const EXTREMUM_SMOOTH_ABSORB_DIST_UPM: f64 = 30.0;

/// Minimum distance (`20.0 UPM`) from segment endpoints $P_0, P_3$ for inserting an inflection point (`spec_ref`).
pub const INFLECTION_MIN_ENDPOINT_DIST_UPM: f64 = 20.0;

/// Minimum control-point chord height (`0.5 UPM`) required before searching for cubic inflection points (`spec_ref`).
pub const INFLECTION_MIN_CHORD_HEIGHT_UPM: f64 = 0.5;

/// Degenerate mini-contour bounding-box threshold (`0.2 UPM` fine grid, `2.0 UPM` standard grid @ `spec_ref`).
pub const TIDY_FINE_GRID_THRESHOLD: f64 = 0.99;
pub const TIDY_MINI_CONTOUR_FINE_UPM: f64 = 0.2;
pub const TIDY_MINI_CONTOUR_COARSE_UPM: f64 = 2.0;

/// Zero-length cubic handle retraction threshold (`0.15 UPM` @ `spec_ref`).
pub const TIDY_ZERO_HANDLE_EPS_UPM: f64 = 0.15;

/// Coincident consecutive `OnCurve` merge threshold (`0.01 UPM` fine grid, `0.6 UPM` standard grid @ `spec_ref`).
pub const TIDY_MERGE_ONCURVE_FINE_UPM: f64 = 0.01;
pub const TIDY_MERGE_ONCURVE_COARSE_UPM: f64 = 0.6;

/// Slope difference tolerance (`0.01`) for collinear interior line-node removal (`findRequiredNodeIndexes:grid:` @ `spec_ref`).
pub const TIDY_COLLINEAR_SLOPE_EPS: f64 = 0.01;

/// Distance threshold (`1.0 UPM`) for open-contour auto-close, `connect_selected_open_endpoints`, and red warning circles (`spec_ref`, `spec_ref`).
pub const CONNECT_MERGE_DIST_UPM: f64 = 1.0;

/// Coincident `OffCurve` handle distance threshold (`0.1 UPM`) for yellow warning circles (`spec_ref`).
pub const WARNING_OFFCURVE_DIST_UPM: f64 = 0.1;

/// Perpendicular distance multiplier (`0.25`) in `collinear_lines(A, B, C)` (`d_perp < 0.25 * sqrt(min(L1, L2))` @ `spec_ref`).
pub const GS_COLINEAR_PERP_FACTOR: f64 = 0.25;

/// Radius increment (`+4.5 dp` on `node_size_dp * 0.5`) for red coincident `OnCurve` warning circles (`spec_ref`).
pub const WARNING_ONCURVE_RADIUS_EXTRA_DP: f32 = 4.5;

/// Radius increment (`+4.5 dp` on `bcp_handle_size_dp * 0.5`) for yellow coincident `OffCurve` warning circles (`spec_ref`).
pub const WARNING_OFFCURVE_RADIUS_EXTRA_DP: f32 = 4.5;

/// Bitmap 3x5 glyph rows for headless rendering of numeric pill badges (`0-9`, `.`, `-`).
pub const PILL_DIGIT_FONT_WIDTH_PX: u32 = 3;
pub const PILL_DIGIT_FONT_HEIGHT_PX: u32 = 5;
pub const PILL_DIGIT_ADVANCE_PX: f32 = 4.0;

#[must_use]
pub const fn pill_digit_bitmap_3x5(ch: char) -> [u8; 5] {
    match ch {
        '0' => [0b111, 0b101, 0b101, 0b101, 0b111],
        '1' => [0b010, 0b110, 0b010, 0b010, 0b111],
        '2' => [0b111, 0b001, 0b111, 0b100, 0b111],
        '3' => [0b111, 0b001, 0b111, 0b001, 0b111],
        '4' => [0b101, 0b101, 0b111, 0b001, 0b001],
        '5' => [0b111, 0b100, 0b111, 0b001, 0b111],
        '6' => [0b111, 0b100, 0b111, 0b101, 0b111],
        '7' => [0b111, 0b001, 0b010, 0b010, 0b010],
        '8' => [0b111, 0b101, 0b111, 0b101, 0b111],
        '9' => [0b111, 0b101, 0b111, 0b001, 0b111],
        '.' => [0b000, 0b000, 0b000, 0b000, 0b010],
        '-' => [0b000, 0b000, 0b111, 0b000, 0b000],
        _ => [0; 5],
    }
}

/// Degrees-to-radians and radians-to-degrees conversion factors.
pub const DEG_TO_RAD_F64: f64 = std::f64::consts::PI / 180.0;
pub const RAD_TO_DEG_F64: f64 = 180.0 / std::f64::consts::PI;
pub const RIGHT_ANGLE_DEG_F64: f64 = 90.0;
pub const FULL_CIRCLE_DEG_F64: f64 = 360.0;

#[must_use]
pub fn bbox_dashed_stroke_color32() -> egui::Color32 {
    egui::Color32::from_rgba_unmultiplied(128, 128, 128, 191)
}

#[must_use]
pub fn bbox_knob_fill_color32() -> egui::Color32 {
    egui::Color32::from_rgba_unmultiplied(102, 102, 102, 71)
}

#[must_use]
pub fn bbox_knob_stroke_color32() -> egui::Color32 {
    egui::Color32::from_rgba_unmultiplied(102, 102, 102, 84)
}

#[must_use]
pub fn bbox_pill_bg_color32() -> egui::Color32 {
    egui::Color32::from_rgba_unmultiplied(153, 153, 153, 230)
}

#[must_use]
pub fn bbox_pill_text_color32() -> egui::Color32 {
    egui::Color32::from_rgba_unmultiplied(255, 255, 255, 255)
}

#[must_use]
pub fn origin_bg_fill_color32() -> egui::Color32 {
    egui::Color32::from_rgba_unmultiplied(0, 0, 0, 51)
}

#[must_use]
pub fn origin_bg_stroke_color32() -> egui::Color32 {
    egui::Color32::from_rgba_unmultiplied(128, 128, 128, 255)
}

#[must_use]
pub fn origin_tick_color32() -> egui::Color32 {
    egui::Color32::from_rgba_unmultiplied(85, 85, 85, 255)
}

#[must_use]
pub fn lasso_fill_color32() -> egui::Color32 {
    egui::Color32::from_rgba_unmultiplied(0, 122, 255, 84)
}

#[must_use]
pub fn lasso_stroke_color32() -> egui::Color32 {
    egui::Color32::from_rgba_unmultiplied(0, 122, 255, 255)
}

#[must_use]
pub fn zoom_marquee_fill_color32() -> egui::Color32 {
    egui::Color32::from_rgba_unmultiplied(128, 128, 128, 84)
}

#[must_use]
pub fn zoom_marquee_stroke_color32() -> egui::Color32 {
    egui::Color32::from_rgba_unmultiplied(128, 128, 128, 255)
}

#[must_use]
pub fn duplicate_line_node_color32() -> egui::Color32 {
    egui::Color32::from_rgba_unmultiplied(229, 57, 53, 153)
}

#[must_use]
pub fn duplicate_curve_node_color32() -> egui::Color32 {
    egui::Color32::from_rgba_unmultiplied(253, 216, 53, 153)
}

pub const PREVIEW_PANEL_DEFAULT_HEIGHT_DP: f32 = 163.0;
pub const PREVIEW_PANEL_MIN_HEIGHT_DP: f32 = 60.0;
pub const PREVIEW_PANEL_PADDING_DP: f32 = 8.0;
pub const PREVIEW_PANEL_BASELINE_STROKE_WIDTH_DP: f32 = 1.0;
pub const PREVIEW_PANEL_AUTOFIT_MARGIN: f64 = 0.85;

#[must_use]
pub fn preview_panel_bg_normal() -> egui::Color32 {
    egui::Color32::from_rgb(255, 255, 255)
}

#[must_use]
pub fn preview_panel_bg_inverted() -> egui::Color32 {
    egui::Color32::from_rgb(26, 26, 26)
}

#[must_use]
pub fn preview_panel_fill_normal() -> egui::Color32 {
    egui::Color32::from_rgb(0, 0, 0)
}

#[must_use]
pub fn preview_panel_fill_inverted() -> egui::Color32 {
    egui::Color32::from_rgb(255, 255, 255)
}

#[must_use]
pub fn preview_panel_baseline_color() -> egui::Color32 {
    egui::Color32::from_rgba_unmultiplied(128, 128, 128, 100)
}

#[must_use]
pub fn preview_panel_border_color() -> egui::Color32 {
    egui::Color32::from_rgba_unmultiplied(128, 128, 128, 80)
}
