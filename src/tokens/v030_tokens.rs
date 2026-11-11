//! `SPEC-004` (`glifo v0.3.0`) tokens: Local & Global Guides (`CanvasGuide`), Magnetic `SnapEngine`
//! (`SnapEngine`), and Alignment Zone Bands & Badges (`_drawAlignmentIndicator:`).
//!
//! Normative token definitions and specification constants
//! verified against deterministic golden-master vectors (`SPEC-004 §2`).

use serde::{Deserialize, Serialize};

/// Simple 8-bit RGBA color tuple used for programmatic inspection & theme verification (`SPEC-004 §2.1`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct RgbaColorToken {
    pub r: u8,
    pub g: u8,
    pub b: u8,
    pub a: u8,
}

impl RgbaColorToken {
    #[must_use]
    pub const fn new(r: u8, g: u8, b: u8, a: u8) -> Self {
        Self { r, g, b, a }
    }

    #[must_use]
    pub fn to_color32(self) -> egui::Color32 {
        egui::Color32::from_rgba_unmultiplied(self.r, self.g, self.b, self.a)
    }

    #[must_use]
    pub fn to_tiny_skia_color(self) -> tiny_skia::Color {
        tiny_skia::Color::from_rgba8(self.r, self.g, self.b, self.a)
    }
}

/// `_LocalGuideColorDefault` (`spec_ref`): `rgba(62, 76, 207, 179)` (`#3E4CCFB3`).
pub const LOCAL_GUIDE_COLOR_DEFAULT: RgbaColorToken = RgbaColorToken::new(62, 76, 207, 179);

/// `_GlobalGuideColorDefault` (`spec_ref`): `rgba(207, 79, 62, 179)` (`#CF4F3EB3`).
pub const GLOBAL_GUIDE_COLOR_DEFAULT: RgbaColorToken = RgbaColorToken::new(207, 79, 62, 179);

/// `_ZonesColorDefault` (`spec_ref`): `rgba(206, 167, 124, 51)` (`#CEA77C33`).
pub const ALIGNMENT_ZONE_FILL_DEFAULT: RgbaColorToken = RgbaColorToken::new(206, 167, 124, 51);

/// `_MetricsColorDefault` (`spec_ref`): `rgba(206, 167, 124, 255)` (`#CEA77CFF`).
pub const METRICS_LINE_COLOR_DEFAULT: RgbaColorToken = RgbaColorToken::new(206, 167, 124, 255);

/// Active magnetic snapper stroke/fill (`spec_ref`): `rgba(153, 51, 51, 255)` (`#993333FF`).
pub const SNAP_INDICATOR_COLOR_DEFAULT: RgbaColorToken = RgbaColorToken::new(153, 51, 51, 255);

/// Alignment badge fill for `Mode 1, 3, 4` (`_ZonesColor` w/ `alpha: 0.5` @ `spec_ref`): `rgba(206, 167, 124, 128)` (`#CEA77C80`).
pub const ALIGNMENT_BADGE_METRIC_FILL: RgbaColorToken = RgbaColorToken::new(206, 167, 124, 128);

/// Alignment badge fill for `Mode 2` Local Guide (`_LocalGuideColor` w/ `alpha: 0.4` @ `spec_ref`): `rgba(62, 76, 207, 102)` (`#3E4CCF66`).
pub const ALIGNMENT_BADGE_LOCAL_GUIDE_FILL: RgbaColorToken = RgbaColorToken::new(62, 76, 207, 102);

/// Alignment badge fill for `Mode 2` Global Guide (`_GlobalGuideColor` w/ `alpha: 0.4` @ `spec_ref`): `rgba(207, 79, 62, 102)` (`#CF4F3E66`).
pub const ALIGNMENT_BADGE_GLOBAL_GUIDE_FILL: RgbaColorToken = RgbaColorToken::new(207, 79, 62, 102);

/// Alignment badge outline stroke (`colorWithWhite:0.55 alpha:0.8` @ `spec_ref`): `rgba(140, 140, 140, 204)` (`#8C8C8CCC`).
pub const ALIGNMENT_BADGE_STROKE_COLOR: RgbaColorToken = RgbaColorToken::new(140, 140, 140, 204);

/// `_GridMainColorDefault` (`ThemeColor` @ `spec_ref`): `rgba(128, 115, 102, 102)` (`#80736666`, `(0.50, 0.45, 0.40, 0.40)`).
pub const GRID_MAIN_COLOR_DEFAULT: RgbaColorToken = RgbaColorToken::new(128, 115, 102, 102);

/// `_GridSubColorDefault` (`ThemeColor` @ `spec_ref`): `rgba(64, 89, 89, 64)` (`#40595940`, `(0.25, 0.35, 0.35, 0.25)`).
pub const GRID_SUB_COLOR_DEFAULT: RgbaColorToken = RgbaColorToken::new(64, 89, 89, 64);

/// Minimum screen-space step in `dp` (`8.0 dp` @ `drawGrid` `spec_ref`) required to render the main grid.
pub const GRID_MAIN_MIN_STEP_DP: f64 = 8.0;

/// Minimum screen-space step in `dp` (`4.0 dp` @ `drawGrid` `spec_ref`) required to render the subdivision grid.
pub const GRID_SUB_MIN_STEP_DP: f64 = 4.0;

/// Minimum `grid_subdivisions` count (`2` @ `drawGrid` `spec_ref`) required to activate subdivision grid lines.
pub const GRID_MIN_SUBDIVISIONS: u32 = 2;

/// Maximum valid `slopeRise` / `slopeRun` component (`100_000` @ `drawGrid` `spec_ref`) for sloped grids.
pub const GRID_MAX_SLOPE_COMPONENT: i32 = 100_000;

/// Main grid orthogonal stroke width (`0.5 dp` @ `drawGrid` `spec_ref`) and sloped vertical stroke width (`0.7 dp` @ `spec_ref`).
pub const GRID_MAIN_LINE_WIDTH_DP: f32 = 0.5;
pub const GRID_MAIN_SLOPED_LINE_WIDTH_DP: f32 = 0.7;

/// Subdivision grid orthogonal stroke width (`0.4 dp` @ `spec_ref`) and sloped vertical stroke width (`0.6 dp` @ `spec_ref`).
pub const GRID_SUB_LINE_WIDTH_DP: f32 = 0.4;
pub const GRID_SUB_SLOPED_LINE_WIDTH_DP: f32 = 0.6;

/// Maximum grid lines drawn per axis (`2048`) to guard against pathological viewport bounds.
pub const GRID_MAX_LINES_PER_AXIS: i64 = 2048;

/// Guide rotation `Shift` snap tolerance in degrees (`6.0 deg`) around `90.0 - italic_angle_deg`.
pub const GUIDE_ITALIC_SNAP_TOLERANCE_DEG: f64 = 6.0;

/// Perpendicular angle offset (`90.0 deg`) used for vertical/italic guide alignment.
pub const GUIDE_PERPENDICULAR_DEG: f64 = 90.0;

/// Infinite guide half-extent in UPM (`200_000.0` @ `beginPointForLayerWidth:` `spec_ref`).
pub const GUIDE_INFINITE_HALF_EXTENT_UPM: f64 = 200_000.0;

/// Half-circle angle in degrees (`180.0`) used for `(-180.0, 180.0]` normalization (`setAngle:` @ `spec_ref`).
pub const GUIDE_HALF_CIRCLE_DEG: f64 = 180.0;

/// Minimum angle change epsilon (`1e-5`) in `CanvasGuide::setAngle:` (`spec_ref`).
pub const GUIDE_ANGLE_CHANGE_EPS: f64 = 1e-5;

/// Guide line stroke width in `dp` (`1.0 dp`).
pub const GUIDE_LINE_STROKE_WIDTH_DP: f32 = 1.0;

/// Selected guide line stroke width in `dp` (`1.5 dp`).
pub const GUIDE_SELECTED_LINE_STROKE_WIDTH_DP: f32 = 1.5;

/// Unselected guide knob radius in `dp` (`3.5 dp`, diameter `7.0 dp`).
pub const GUIDE_KNOB_RADIUS_DP: f32 = 3.5;

/// Selected guide knob radius in `dp` (`4.5 dp`, diameter `9.0 dp`).
pub const GUIDE_SELECTED_KNOB_RADIUS_DP: f32 = 4.5;

/// Guide knob hit-test radius in `dp` (`7.0 dp`).
pub const GUIDE_KNOB_HIT_RADIUS_DP: f32 = 7.0;

/// Guide line hit-test tolerance in `dp` (`5.0 dp`).
pub const GUIDE_LINE_HIT_TOLERANCE_DP: f32 = 5.0;

/// Selected guide directional tick length in `dp` (`6.0 dp`).
pub const GUIDE_SELECTED_TICK_LEN_DP: f32 = 6.0;

/// Padlock icon body half-width (`3.5 dp`), body height (`5.0 dp`), and shackle radius (`2.2 dp`) (`spec_ref`).
pub const GUIDE_LOCK_BODY_HALF_W_DP: f32 = 3.5;
pub const GUIDE_LOCK_BODY_H_DP: f32 = 5.0;
pub const GUIDE_LOCK_SHACKLE_RADIUS_DP: f32 = 2.2;
pub const GUIDE_LOCK_STROKE_WIDTH_DP: f32 = 1.2;

/// Measurement guide intersection dot radius in `dp` (`3.0 dp`).
pub const GUIDE_MEASUREMENT_DOT_RADIUS_DP: f32 = 3.0;

/// Measurement guide perpendicular tick half-length in `dp` (`4.5 dp`).
pub const GUIDE_MEASUREMENT_TICK_HALF_DP: f32 = 4.5;

/// Measurement guide pill vertical offset in `dp` (`10.0 dp`).
pub const GUIDE_MEASUREMENT_PILL_OFFSET_DP: f32 = 10.0;

/// Default magnetic snap threshold in `dp` (`5.0 dp` @ `SnapEngine`).
pub const DEFAULT_SNAP_THRESHOLD_DP: f32 = 5.0;

/// Maximum number of simultaneous active snappers (`2` @ `spec_ref`).
pub const MAX_ACTIVE_SNAPPERS: usize = 2;

/// Active magnetic snapper stroke width in `dp` (`1.2 dp` @ `spec_ref`).
pub const SNAP_INDICATOR_STROKE_WIDTH_DP: f32 = 1.2;

/// Active magnetic snapper endpoint circle radius in `dp` (`4.0 dp`, diameter `8.0 dp` @ `spec_ref`).
pub const SNAP_INDICATOR_CIRCLE_RADIUS_DP: f32 = 4.0;

/// Parallel line determinant epsilon (`1e-7`) for `intersect_line_line`.
pub const SNAP_LINE_INTERSECT_DET_EPS: f64 = 1e-7;

/// Metric line & LSB/RSB proximity tolerance in UPM (`0.6 UPM` @ `FontMasterMetrics::isOnMetrics:...`).
pub const METRIC_BADGE_TOLERANCE_UPM: f64 = 0.6;

/// Guide proximity tolerance clamp bounds (`clamp(0.5 * grid, 0.4, 2.0)` @ `spec_ref..spec_ref`).
pub const GUIDE_BADGE_TOL_MIN_UPM: f64 = 0.4;
pub const GUIDE_BADGE_TOL_MAX_UPM: f64 = 2.0;
pub const GUIDE_BADGE_TOL_DEFAULT_UPM: f64 = 1.0;

/// Standard Latin default alignment zone size (`16.0 UPM` above Cap/x-Height, `-16.0 UPM` below Baseline/Descender).
pub const DEFAULT_ALIGNMENT_ZONE_SIZE_UPM: f64 = 16.0;

/// Alignment badge dimensions in `dp` (`_drawAlignmentIndicator:` @ `spec_ref..spec_ref`):
/// - Mode 1 (`OnMetricLine` diamond): unselected `14.0 dp`, selected `16.0 dp` (extends visibly outside the `6.0 dp` node body)
/// - Mode 2, 3, 4: unselected `12.5 dp`, selected `15.0 dp`
pub const BADGE_MODE1_UNSELECTED_SIZE_DP: f32 = 14.0;
pub const BADGE_MODE1_SELECTED_SIZE_DP: f32 = 16.0;
pub const BADGE_MODE234_UNSELECTED_SIZE_DP: f32 = 12.5;
pub const BADGE_MODE234_SELECTED_SIZE_DP: f32 = 15.0;
pub const BADGE_VERTICAL_RECT_HALF_W_DP: f32 = 3.5;
pub const BADGE_VERTICAL_RECT_EXTRA_H_DP: f32 = 3.5;
pub const BADGE_STROKE_WIDTH_DP: f32 = 1.0;
pub const BADGE_ZOOM_SCALE_MIN: f32 = 0.85;
pub const BADGE_ZOOM_SCALE_MAX: f32 = 1.35;

#[must_use]
pub fn local_guide_color32() -> egui::Color32 {
    LOCAL_GUIDE_COLOR_DEFAULT.to_color32()
}

#[must_use]
pub fn global_guide_color32() -> egui::Color32 {
    GLOBAL_GUIDE_COLOR_DEFAULT.to_color32()
}

#[must_use]
pub fn alignment_zone_fill_color32() -> egui::Color32 {
    ALIGNMENT_ZONE_FILL_DEFAULT.to_color32()
}

#[must_use]
pub fn snap_indicator_color32() -> egui::Color32 {
    SNAP_INDICATOR_COLOR_DEFAULT.to_color32()
}

#[must_use]
pub fn alignment_badge_metric_fill_color32() -> egui::Color32 {
    ALIGNMENT_BADGE_METRIC_FILL.to_color32()
}

#[must_use]
pub fn alignment_badge_local_guide_fill_color32() -> egui::Color32 {
    ALIGNMENT_BADGE_LOCAL_GUIDE_FILL.to_color32()
}

#[must_use]
pub fn alignment_badge_global_guide_fill_color32() -> egui::Color32 {
    ALIGNMENT_BADGE_GLOBAL_GUIDE_FILL.to_color32()
}

#[must_use]
pub fn alignment_badge_stroke_color32() -> egui::Color32 {
    ALIGNMENT_BADGE_STROKE_COLOR.to_color32()
}

/// 3-way appearance preference designed for HostApp Preferences (`Light`, `Dark`, `System`).
///
/// - `AppearanceMode::Light`: Explicit user choice — always resolves to `GlifoTheme::sonoma_default()`.
/// - `AppearanceMode::Dark`: Explicit user choice — always resolves to `GlifoTheme::sonoma_dark()`.
/// - `AppearanceMode::System`: System preference — resolves to `GlifoTheme::sonoma_dark()` when the
///   host OS / `egui` environment is in Dark Mode (`system_dark_mode == true`) and to
///   `GlifoTheme::sonoma_default()` when in Light Mode (`system_dark_mode == false`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum AppearanceMode {
    #[default]
    Light,
    Dark,
    System,
}

impl AppearanceMode {
    /// Resolves whether dark mode should be active given the current OS/system dark mode state.
    #[must_use]
    pub const fn resolves_to_dark(self, system_is_dark: bool) -> bool {
        match self {
            Self::Light => false,
            Self::Dark => true,
            Self::System => system_is_dark,
        }
    }
}

/// Reference Vector (``) Dark Mode canvas background (`NSColor::darkContentBackgroundColor` @ `spec_ref`).
pub const DARK_CANVAS_BACKGROUND_HEX: &str = "#1E1E1E";

/// Sonoma Dark sidebar (`Shapes`) background.
pub const DARK_SIDEBAR_BACKGROUND_HEX: &str = "#262626";

/// Sonoma Dark InfoBox / HUD background.
pub const DARK_INFOBOX_BACKGROUND_HEX: &str = "#2C2C2E";

/// Sonoma Dark splitter / separator color.
pub const DARK_SPLITTER_HEX: &str = "#38383A";

/// Sonoma Dark metrics guide line color.
pub const DARK_METRICS_GUIDE_HEX: &str = "#555558";

/// Sonoma Dark metrics label text color.
pub const DARK_METRICS_LABEL_HEX: &str = "#98989D";

/// Sonoma Dark open contour endpoint tick color.
pub const DARK_OPEN_ENDPOINT_HEX: &str = "#0A84FF";

impl super::GlifoTheme {
    /// Loads the Reference Vector (``) Sonoma Dark Mode theme from `tokens/glifo_theme_sonoma_dark.json`.
    #[must_use]
    pub fn sonoma_dark() -> Self {
        let json_str = include_str!("../../tokens/glifo_theme_sonoma_dark.json");
        serde_json::from_str(json_str).expect("glifo_theme_sonoma_dark.json must be valid JSON")
    }

    /// Resolves the appropriate `GlifoTheme` (`sonoma_default` or `sonoma_dark`) for the given
    /// 3-way `AppearanceMode` (`Light`, `Dark`, `System`) and host OS `system_is_dark` flag.
    #[must_use]
    pub fn for_appearance(mode: AppearanceMode, system_is_dark: bool) -> Self {
        if mode.resolves_to_dark(system_is_dark) {
            Self::sonoma_dark()
        } else {
            Self::sonoma_default()
        }
    }

    /// Returns `true` when this theme is a Dark Mode theme.
    #[must_use]
    pub fn is_dark(&self) -> bool {
        self.theme_name.contains("Dark")
            || self.preview_canvas.fill_color.eq_ignore_ascii_case("#FFFFFF")
    }

    #[must_use]
    pub fn contour_stroke_color32(&self) -> egui::Color32 {
        self.preview_canvas.fill_color32()
    }

    #[must_use]
    pub fn preview_fill_color32(&self) -> egui::Color32 {
        self.preview_canvas.fill_color32()
    }

    #[must_use]
    pub fn corner_node_color32(&self) -> egui::Color32 {
        self.edit_canvas.corner_node_color32()
    }

    #[must_use]
    pub fn smooth_node_color32(&self) -> egui::Color32 {
        self.edit_canvas.smooth_node_color32()
    }

    #[must_use]
    pub fn bcp_handle_color32(&self) -> egui::Color32 {
        self.edit_canvas.bcp_handle_color32()
    }
}

#[must_use]
pub fn locator_cell_bg_color_for_mode(is_dark: bool) -> egui::Color32 {
    if is_dark {
        egui::Color32::from_white_alpha(super::LOCATOR_CELL_BG_ALPHA)
    } else {
        super::locator_cell_bg_color()
    }
}

#[must_use]
pub fn locator_cell_border_color_for_mode(is_dark: bool) -> egui::Color32 {
    if is_dark {
        egui::Color32::from_white_alpha(super::LOCATOR_CELL_BORDER_ALPHA)
    } else {
        super::locator_cell_border_color()
    }
}

#[must_use]
pub fn locator_context_color_for_mode(is_dark: bool) -> egui::Color32 {
    if is_dark {
        egui::Color32::from_white_alpha(super::LOCATOR_CONTEXT_ALPHA)
    } else {
        super::locator_default_context_color()
    }
}



