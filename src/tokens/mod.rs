//! Layout and rendering tokens loaded from JSON databases.

pub mod archetypes_tokens;
pub mod cursor_tokens;
pub mod v020_tokens;
pub mod v030_tokens;
pub mod v040_tokens;
pub use archetypes_tokens::{
    load_archetype_preset, ArchetypeContourPreset, ArchetypeGlyphPreset, ArchetypeNodePreset,
    GlyphAGoldenMaster, GlyphArchetype, GlyphPointToken,
};
pub use cursor_tokens::*;
pub use v020_tokens::*;
pub use v030_tokens::*;
pub use v040_tokens::*;

use serde::{Deserialize, Serialize};

/// Parses a hex color string into an `egui::Color32`.
///
/// Supports 3-digit (`#RGB`), 4-digit (`#RGBA`), 6-digit (`#RRGGBB`),
/// and 8-digit (`#RRGGBBAA`) hex representations, with or without a leading `#`.
#[must_use]
pub fn hex_to_color32(hex: &str) -> egui::Color32 {
    let clean = hex.trim().trim_start_matches('#');
    if !(clean.is_ascii() && clean.bytes().all(|b| b.is_ascii_hexdigit())) {
        return egui::Color32::from_rgb(0, 0, 0);
    }
    match clean.len() {
        3 => {
            let r = u8::from_str_radix(&clean[0..1].repeat(2), 16).unwrap_or(0);
            let g = u8::from_str_radix(&clean[1..2].repeat(2), 16).unwrap_or(0);
            let b = u8::from_str_radix(&clean[2..3].repeat(2), 16).unwrap_or(0);
            egui::Color32::from_rgb(r, g, b)
        }
        4 => {
            let r = u8::from_str_radix(&clean[0..1].repeat(2), 16).unwrap_or(0);
            let g = u8::from_str_radix(&clean[1..2].repeat(2), 16).unwrap_or(0);
            let b = u8::from_str_radix(&clean[2..3].repeat(2), 16).unwrap_or(0);
            let a = u8::from_str_radix(&clean[3..4].repeat(2), 16).unwrap_or(255);
            egui::Color32::from_rgba_unmultiplied(r, g, b, a)
        }
        6 => {
            let r = u8::from_str_radix(&clean[0..2], 16).unwrap_or(0);
            let g = u8::from_str_radix(&clean[2..4], 16).unwrap_or(0);
            let b = u8::from_str_radix(&clean[4..6], 16).unwrap_or(0);
            egui::Color32::from_rgb(r, g, b)
        }
        8 => {
            let r = u8::from_str_radix(&clean[0..2], 16).unwrap_or(0);
            let g = u8::from_str_radix(&clean[2..4], 16).unwrap_or(0);
            let b = u8::from_str_radix(&clean[4..6], 16).unwrap_or(0);
            let a = u8::from_str_radix(&clean[6..8], 16).unwrap_or(255);
            egui::Color32::from_rgba_unmultiplied(r, g, b, a)
        }
        _ => egui::Color32::from_rgb(0, 0, 0),
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct EditCanvasTokens {
    pub node_size_dp: f32,
    pub bcp_handle_size_dp: f32,
    pub bcp_line_width_dp: f32,
    pub contour_stroke_width_dp: f32,
    pub corner_node_color: String,
    pub smooth_node_color: String,
    pub bcp_handle_color: String,
    pub bcp_line_color: String,
    pub selected_glow_color: String,
    pub rubberband_fill_color: String,
    pub rubberband_stroke_color: String,
    pub rubberband_stroke_width_dp: f32,
    #[serde(default = "default_local_guide_rgba")]
    pub local_guide_color: RgbaColorToken,
    #[serde(default = "default_global_guide_rgba")]
    pub global_guide_color: RgbaColorToken,
    #[serde(default = "default_alignment_zone_rgba")]
    pub alignment_zone_fill: RgbaColorToken,
    #[serde(default = "default_snap_indicator_rgba")]
    pub snap_indicator_color: RgbaColorToken,
    #[serde(default = "default_grid_main_rgba")]
    pub grid_main_color: RgbaColorToken,
    #[serde(default = "default_grid_sub_rgba")]
    pub grid_sub_color: RgbaColorToken,
}

const fn default_local_guide_rgba() -> RgbaColorToken {
    LOCAL_GUIDE_COLOR_DEFAULT
}
const fn default_global_guide_rgba() -> RgbaColorToken {
    GLOBAL_GUIDE_COLOR_DEFAULT
}
const fn default_alignment_zone_rgba() -> RgbaColorToken {
    ALIGNMENT_ZONE_FILL_DEFAULT
}
const fn default_snap_indicator_rgba() -> RgbaColorToken {
    SNAP_INDICATOR_COLOR_DEFAULT
}
const fn default_grid_main_rgba() -> RgbaColorToken {
    GRID_MAIN_COLOR_DEFAULT
}
const fn default_grid_sub_rgba() -> RgbaColorToken {
    GRID_SUB_COLOR_DEFAULT
}

impl EditCanvasTokens {
    #[must_use]
    pub fn corner_node_color32(&self) -> egui::Color32 {
        hex_to_color32(&self.corner_node_color)
    }

    #[must_use]
    pub fn smooth_node_color32(&self) -> egui::Color32 {
        hex_to_color32(&self.smooth_node_color)
    }

    #[must_use]
    pub fn bcp_handle_color32(&self) -> egui::Color32 {
        hex_to_color32(&self.bcp_handle_color)
    }

    #[must_use]
    pub fn bcp_line_color32(&self) -> egui::Color32 {
        hex_to_color32(&self.bcp_line_color)
    }

    #[must_use]
    pub fn selected_glow_color32(&self) -> egui::Color32 {
        hex_to_color32(&self.selected_glow_color)
    }

    #[must_use]
    pub fn rubberband_fill_color32(&self) -> egui::Color32 {
        hex_to_color32(&self.rubberband_fill_color)
    }

    #[must_use]
    pub fn rubberband_stroke_color32(&self) -> egui::Color32 {
        hex_to_color32(&self.rubberband_stroke_color)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PreviewCanvasTokens {
    pub fill_color: String,
    pub baseline_color: String,
    pub baseline_stroke_width_dp: f32,
}

impl PreviewCanvasTokens {
    #[must_use]
    pub fn fill_color32(&self) -> egui::Color32 {
        hex_to_color32(&self.fill_color)
    }

    #[must_use]
    pub fn baseline_color32(&self) -> egui::Color32 {
        hex_to_color32(&self.baseline_color)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ShapeCanvasTokens {
    pub row_height_dp: f32,
    pub header_height_dp: f32,
    pub thumbnail_width_dp: f32,
    pub thumbnail_height_dp: f32,
    pub thumbnail_padding_dp: f32,
    pub accent_selection_color: String,
    pub selection_corner_radius_dp: f32,
    pub text_active_color: String,
    pub text_inactive_color: String,
    pub winding_icon_size_dp: f32,
}

impl ShapeCanvasTokens {
    #[must_use]
    pub fn accent_selection_color32(&self) -> egui::Color32 {
        hex_to_color32(&self.accent_selection_color)
    }

    #[must_use]
    pub fn text_active_color32(&self) -> egui::Color32 {
        hex_to_color32(&self.text_active_color)
    }

    #[must_use]
    pub fn text_inactive_color32(&self) -> egui::Color32 {
        hex_to_color32(&self.text_inactive_color)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SnappingTokens {
    pub snap_threshold_dp: f32,
    pub angle_constraint_step_deg: f32,
}

pub const MIN_VIEWPORT_ZOOM: f32 = 0.01;
pub const MAX_VIEWPORT_ZOOM: f32 = 50.0;
pub const ZOOM_STEP_IN_FACTOR: f32 = std::f32::consts::SQRT_2;
pub const ZOOM_STEP_OUT_FACTOR: f32 = 0.70710677;
pub const WHEEL_ZOOM_SENSITIVITY: f32 = 0.0015;
pub const FULL_CIRCLE_DEG: f64 = 360.0;
pub const DEFAULT_ANGLE_SNAP_DEG: f64 = 45.0;
pub const CURVE_FLATTEN_TOLERANCE: f64 = 0.25;
pub const DEFAULT_CANVAS_WIDTH_DP: f32 = 800.0;
pub const DEFAULT_CANVAS_HEIGHT_DP: f32 = 600.0;

pub const DEFAULT_ASCENDER_UPM: f64 = 800.0;
pub const DEFAULT_CAP_HEIGHT_UPM: f64 = 700.0;
pub const DEFAULT_X_HEIGHT_UPM: f64 = 500.0;
pub const DEFAULT_BASELINE_UPM: f64 = 0.0;
pub const DEFAULT_DESCENDER_UPM: f64 = -200.0;

pub const START_NODE_ARROW_OFFSET_DP: f32 = 6.5;
pub const START_NODE_ARROW_LEN_DP: f32 = 13.0;
pub const START_NODE_ARROW_HALF_WIDTH_DP: f32 = 6.0;
pub const START_NODE_ARROW_OUTLINE_WIDTH_DP: f32 = 1.25;
pub const OPEN_ENDPOINT_TICK_HALF_LEN_DP: f32 = 5.0;
pub const METRICS_LABEL_FONT_SIZE_DP: f32 = 10.0;
pub const METRICS_LABEL_OFFSET_X_DP: f32 = 6.0;
pub const METRICS_LABEL_OFFSET_Y_DP: f32 = 2.0;

pub const EXTREMUM_T_MIN: f64 = 0.01;
pub const EXTREMUM_T_MAX: f64 = 0.99;
pub const EXTREMUM_SNAP_MAX_DELTA_T: f64 = 0.25;
pub const DRAG_CURVE_T_MIN: f64 = 0.05;
pub const DRAG_CURVE_T_MAX: f64 = 0.95;
pub const DRAG_CURVE_MIN_HANDLE_LEN: f64 = 0.9;
pub const SEGMENT_DRAG_FREE_WEIGHT_FACTOR: f64 = 2.65;
pub const SEGMENT_DRAG_MAX_HANDLE_LEN_UPM: f64 = 6000.0;
pub const TENSION_SCALE_MIN: f64 = 0.05;
pub const TENSION_SCALE_MAX: f64 = 10.0;

pub const DEFAULT_GRID_STEP_UPM: f64 = 1.0;
pub const NUDGE_STEP_UPM: f64 = 1.0;
pub const NUDGE_STEP_SHIFT_UPM: f64 = 10.0;
pub const NUDGE_STEP_CMD_UPM: f64 = 100.0;
pub const NUDGE_SPAN_MIN: f64 = 0.1;
pub const NUDGE_PREV_SPAN_Y_MIN: f64 = 1.0;
pub const NUDGE_RATIO_CLAMP_MIN: f64 = -1.45;
pub const NUDGE_RATIO_CLAMP_MAX: f64 = 1.45;
pub const SHIFT_MIDDLE_MAX_T: f64 = 0.98;
pub const SHIFT_MIDDLE_TANGENT_FACTOR: f64 = 1.5;
pub const SEGMENT_HIT_TOLERANCE_DP: f32 = 6.0;
pub const DOUBLE_CLICK_MAX_DELAY_SEC: f64 = 0.4;
pub const DOUBLE_CLICK_MAX_DIST_DP: f32 = 8.0;
pub const PARAM_CLAMP_MIN: f64 = 0.001;
pub const PARAM_CLAMP_MAX: f64 = 0.999;
pub const CUBIC_HEAL_RATIO: f64 = 0.6666666666666666;
pub const CUBIC_ONE_THIRD: f64 = 1.0 / 3.0;
pub const CUBIC_TWO_THIRDS: f64 = 2.0 / 3.0;
pub const CUBIC_HEAL_MAX_SCALE: f64 = 3.0;
pub const BERNSTEIN_CUBIC_COEFF: f64 = 3.0;
pub const BERNSTEIN_SECOND_DERIV_COEFF: f64 = 6.0;
pub const SOLVER_DET_EPSILON: f64 = 1e-9;
pub const SPLIT_RATIO_MIN: f64 = 0.05;
pub const SPLIT_RATIO_MAX: f64 = 0.95;
pub const FIT_SAMPLES_PER_SEGMENT: usize = 8;
pub const NEWTON_REPARAM_ITERATIONS: usize = 4;
pub const COLLINEAR_MIN_LEN_EPSILON: f64 = 1e-6;
pub const GHOST_NODE_STROKE_WIDTH_DP: f32 = 1.5;
pub const AUTOFIT_MARGIN_RATIO: f32 = 0.65;
pub const INITIAL_SAFE_NODE_ID: u64 = 1000;
pub const CURVE_SOLVE_ACCURACY: f64 = 1e-4;

pub const LOCATOR_DEFAULT_WIDTH_DP: f32 = 48.0;
pub const LOCATOR_DEFAULT_HEIGHT_DP: f32 = 36.0;
pub const LOCATOR_DEFAULT_PADDING_DP: f32 = 2.0;
pub const LOCATOR_OPEN_CONTOUR_STROKE_WIDTH_DP: f32 = 1.5;
pub const LOCATOR_CONTEXT_ALPHA: u8 = 31;
pub const LOCATOR_CELL_CORNER_RADIUS_DP: f32 = 4.0;
pub const LOCATOR_CELL_BORDER_WIDTH_DP: f32 = 1.0;
pub const LOCATOR_CELL_BG_ALPHA: u8 = 15;
pub const LOCATOR_CELL_BORDER_ALPHA: u8 = 35;

#[must_use]
pub fn locator_cell_bg_color() -> egui::Color32 {
    egui::Color32::from_black_alpha(LOCATOR_CELL_BG_ALPHA)
}

#[must_use]
pub fn locator_cell_border_color() -> egui::Color32 {
    egui::Color32::from_black_alpha(LOCATOR_CELL_BORDER_ALPHA)
}

#[must_use]
pub fn locator_default_context_color() -> egui::Color32 {
    egui::Color32::from_black_alpha(LOCATOR_CONTEXT_ALPHA)
}

#[must_use]
pub fn locator_default_focus_color() -> egui::Color32 {
    hex_to_color32("#007AFF")
}

#[must_use]
pub fn locator_default_mask_color() -> egui::Color32 {
    hex_to_color32("#FF3B30")
}

pub const GOLDEN_MASTER_WINDOW_WIDTH_DP: f32 = 1223.0;
pub const GOLDEN_MASTER_WINDOW_HEIGHT_DP: f32 = 811.0;
pub const GOLDEN_MASTER_SIDEBAR_WIDTH_DP: f32 = 250.0;
pub const GOLDEN_MASTER_CANVAS_WIDTH_DP: f32 = 735.0;
pub const GOLDEN_MASTER_CANVAS_EDIT_HEIGHT_DP: f32 = 617.0;
pub const GOLDEN_MASTER_CANVAS_PREVIEW_HEIGHT_DP: f32 = 163.0;
pub const GOLDEN_MASTER_INFOBOX_WIDTH_DP: f32 = 237.0;
pub const GOLDEN_MASTER_SPLITTER_WIDTH_DP: f32 = 1.0;
pub const GOLDEN_MASTER_INFOBOX_DIVIDER_Y_DP: f32 = 781.0;
pub const GOLDEN_MASTER_ZOOM: f32 = 0.20425;
pub const GOLDEN_MASTER_PAN_X: f32 = 550.2;
pub const GOLDEN_MASTER_PAN_Y: f32 = 502.0;
pub const GOLDEN_MASTER_CALIBRATED_ZOOM: f32 = 0.200;
pub const GOLDEN_MASTER_FULL_PAN_X: f32 = 548.0;
pub const GOLDEN_MASTER_FULL_PAN_Y: f32 = 501.0;
pub const GOLDEN_MASTER_LOCAL_PAN_X: f32 = 297.0;
pub const GOLDEN_MASTER_LOCAL_PAN_Y: f32 = 501.0;

/// Converts an `egui::Color32` to a `tiny_skia::Color`.
#[must_use]
pub fn color32_to_tiny_skia(color: egui::Color32) -> tiny_skia::Color {
    let [r, g, b, a] = color.to_srgba_unmultiplied();
    tiny_skia::Color::from_rgba8(r, g, b, a)
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct GrafiaTheme {
    pub theme_name: String,
    pub edit_canvas: EditCanvasTokens,
    pub preview_canvas: PreviewCanvasTokens,
    pub shape_canvas: ShapeCanvasTokens,
    pub snapping: SnappingTokens,
}

pub type GlifoTheme = GrafiaTheme;

impl GrafiaTheme {
    /// Loads default theme embedded from `tokens/glifo_theme_sonoma.json`.
    #[must_use]
    pub fn sonoma_default() -> Self {
        let json_str = include_str!("../../tokens/glifo_theme_sonoma.json");
        serde_json::from_str(json_str).expect("glifo_theme_sonoma.json must be valid JSON")
    }

    #[must_use]
    pub fn canvas_background_color32(&self) -> egui::Color32 {
        hex_to_color32(if self.is_dark() { DARK_CANVAS_BACKGROUND_HEX } else { "#FFFFFF" })
    }

    #[must_use]
    pub fn sidebar_background_color32(&self) -> egui::Color32 {
        hex_to_color32(if self.is_dark() { DARK_SIDEBAR_BACKGROUND_HEX } else { "#F6F6F6" })
    }

    #[must_use]
    pub fn infobox_background_color32(&self) -> egui::Color32 {
        hex_to_color32(if self.is_dark() { DARK_INFOBOX_BACKGROUND_HEX } else { "#ECECEC" })
    }

    #[must_use]
    pub fn splitter_color32(&self) -> egui::Color32 {
        hex_to_color32(if self.is_dark() { DARK_SPLITTER_HEX } else { "#E1E1E1" })
    }

    #[must_use]
    pub fn start_node_arrow_color32(&self) -> egui::Color32 {
        hex_to_color32("#0066FFDC")
    }

    #[must_use]
    pub fn metrics_guide_color32(&self) -> egui::Color32 {
        hex_to_color32(if self.is_dark() { DARK_METRICS_GUIDE_HEX } else { "#EAEAEA" })
    }

    #[must_use]
    pub fn metrics_label_color32(&self) -> egui::Color32 {
        hex_to_color32(if self.is_dark() { DARK_METRICS_LABEL_HEX } else { "#8E8E93" })
    }

    #[must_use]
    pub fn open_endpoint_color32(&self) -> egui::Color32 {
        hex_to_color32(if self.is_dark() { DARK_OPEN_ENDPOINT_HEX } else { "#007AFF" })
    }
}
