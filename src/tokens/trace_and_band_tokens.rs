//! Numeric and color tokens for `SPEC-009` (`grafia v0.8.0` Part B):
//! Bitmap & Camera Auto-Trace (`AutoTraceConfig`), `GuideThickness`, `EditCanvasTokens` visual widths,
//! `GrafiaPreferences` defaults & clamp bounds, and touch precision loupe geometry.

use crate::tokens::{
    hex_to_color32, GrafiaTheme, RgbaColorToken, DARK_CANVAS_BACKGROUND_HEX,
    DARK_INFOBOX_BACKGROUND_HEX, DARK_METRICS_GUIDE_HEX, DARK_METRICS_LABEL_HEX,
    DARK_OPEN_ENDPOINT_HEX, DARK_SIDEBAR_BACKGROUND_HEX, DARK_SPLITTER_HEX,
};

// ============================================================================
// Auto-Trace Constants (`SPEC-009 §2.5`)
// ============================================================================

pub const AUTO_TRACE_DEFAULT_LUMINANCE: u8 = 128;
pub const AUTO_TRACE_MIN_LUMINANCE: u8 = 1;
pub const AUTO_TRACE_MAX_LUMINANCE: u8 = 254;
pub const AUTO_TRACE_DEFAULT_WINDOW_RADIUS_PX: u16 = 24;
pub const AUTO_TRACE_DEFAULT_BIAS_K: f32 = 0.18;
pub const AUTO_TRACE_SAUVOLA_DYNAMIC_RANGE: f64 = 128.0;
pub const AUTO_TRACE_MIN_SPECKLE_AREA_DEFAULT_PX2: f64 = 9.0;
pub const AUTO_TRACE_RDP_TOLERANCE_DEFAULT_PX: f64 = 0.85;
pub const AUTO_TRACE_CORNER_ANGLE_DEFAULT_DEG: f64 = 125.0;
pub const AUTO_TRACE_FIT_ERROR_DEFAULT_UPM: f64 = 2.5;

pub const LUMA_RED_COEFF: f64 = 0.299;
pub const LUMA_GREEN_COEFF: f64 = 0.587;
pub const LUMA_BLUE_COEFF: f64 = 0.114;
pub const BYTE_MAX_F64: f64 = 255.0;

pub const HOMOGRAPHY_SYSTEM_DIM: usize = 8;
pub const HOMOGRAPHY_PIVOT_EPS: f64 = 1e-11;
pub const HOMOGRAPHY_DENOM_EPS: f64 = 1e-9;

pub const MARCHING_SQUARES_ISO_EPS: f64 = 1e-6;
pub const MARCHING_SQUARES_QUANT_SCALE: f64 = 4096.0;
pub const TRACE_MIN_LOOP_VERTICES: usize = 3;
pub const TRACE_DEDUP_DIST_EPS_PX: f64 = 1e-4;
pub const TRACE_STRAIGHT_CHORD_TOL_FACTOR: f64 = 0.35;
pub const TRACE_BEZIER_FIT_MAX_DEPTH: u8 = 8;
pub const TRACE_NEWTON_ITERS: usize = 4;
pub const TRACE_CHORD_EPS: f64 = 1e-8;
pub const TRACE_HANDLE_MIN_FRACTION: f64 = 0.02;
pub const TRACE_HANDLE_ONE_THIRD: f64 = 1.0 / 3.0;
pub const TRACE_STRAIGHT_ANGLE_DEG: f64 = 180.0;
pub const TRACE_MIN_CORNER_ARM_PX: f64 = 1.5;
pub const TRACE_CORNER_NEIGHBOR_STRIDE: usize = 2;
pub const ADAPTIVE_BIAS_MIN_F64: f64 = 0.01;
pub const ADAPTIVE_BIAS_MAX_F64: f64 = 0.95;
pub const ADAPTIVE_DARK_ANCHOR_RATIO: f64 = 0.15;
pub const MARCHING_SADDLE_QUARTER: f64 = 0.25;
pub const TRACE_MIN_RDP_TOL_PX: f64 = 0.1;

// ============================================================================
// EditCanvasTokens & GuideThickness Constants (`SPEC-009 §2.6.1 & §2.6.2`)
// ============================================================================

pub const DEFAULT_GUIDE_STROKE_WIDTH_DP: f32 = 1.0;
pub const DEFAULT_METRIC_LINE_WIDTH_DP: f32 = 1.0;
pub const STROKE_WIDTH_MIN_DP: f32 = 0.5;
pub const STROKE_WIDTH_MAX_DP: f32 = 4.0;
pub const GUIDE_BAND_ALPHA_SCALE: f32 = 0.24;
pub const GUIDE_CLIP_MARGIN_DP: f32 = 64.0;
pub const GUIDE_CLIP_RAY_EPS_DP: f32 = 1e-6;
pub const OFFCURVE_CROSS_ARM_SCALE: f32 = 0.65;
pub const OFFCURVE_CROSS_STROKE_WIDTH_DP: f32 = 1.3;
pub const FULL_CROSSHAIR_STROKE_WIDTH_DP: f32 = 1.0;
pub const FULL_CROSSHAIR_ALPHA: u8 = 110;
pub const EDIT_ACTIVE_FILL_ALPHA: u8 = 38;

// ============================================================================
// GrafiaPreferences Defaults & Touch Loupe Constants (`SPEC-009 §2.6.3`)
// ============================================================================

pub const PREF_DEFAULT_NUDGE_SMALL_UPM: f64 = 1.0;
pub const PREF_DEFAULT_NUDGE_MEDIUM_UPM: f64 = 10.0;
pub const PREF_DEFAULT_NUDGE_LARGE_UPM: f64 = 100.0;
pub const PREF_MIN_STEP_UPM: f64 = 0.01;
pub const PREF_DEFAULT_SPACING_LOW_UPM: f64 = 1.0;
pub const PREF_DEFAULT_SPACING_HIGH_UPM: f64 = 10.0;
pub const PREF_DEFAULT_KERNING_LOW_UPM: f64 = 1.0;
pub const PREF_DEFAULT_KERNING_HIGH_UPM: f64 = 10.0;
pub const PREF_DEFAULT_GRID_STEP_UPM: f64 = 1.0;
pub const PREF_DEFAULT_GRID_SUBDIVISIONS: u32 = 1;
pub const PREF_DEFAULT_SNAP_RADIUS_DP: f32 = 6.0;
pub const PREF_DEFAULT_HIT_TEST_RADIUS_DP: f32 = 6.0;
pub const PREF_MIN_RADIUS_DP: f32 = 1.0;
pub const PREF_MAX_RADIUS_DP: f32 = 32.0;
pub const PREF_DEFAULT_TEXT_NUMBERS_THRESHOLD: f64 = 100.0;
pub const PREF_DEFAULT_PASSIVE_NEIGHBOR_OPACITY: f32 = 0.85;
pub const PREF_DEFAULT_FREEHAND_SMOOTHING_PX: f64 = 0.85;
pub const PREF_MIN_FREEHAND_SMOOTHING_PX: f64 = 0.05;
pub const PREF_DEFAULT_TOUCH_OFFSET_DP: f32 = 0.0;

pub const TOUCH_LOUPE_MAGNIFICATION: f32 = 2.5;
pub const TOUCH_LOUPE_RADIUS_DP: f32 = 28.0;
pub const TOUCH_LOUPE_CENTER_LIFT_DP: f32 = 52.0;
pub const TOUCH_LOUPE_RING_WIDTH_DP: f32 = 2.0;
pub const TOUCH_LOUPE_CROSSHAIR_HALF_DP: f32 = 8.0;
pub const TOUCH_LOUPE_CROSSHAIR_WIDTH_DP: f32 = 1.2;
pub const TOUCH_TWO_FINGER_TAP_MAX_MOVE_DP: f32 = 12.0;
pub const TOUCH_MIN_PINCH_DIST_DP: f32 = 8.0;

// ============================================================================
// GrafiaTheme v0.8.0 Part B Default Color Tokens (`SPEC-009 §2.6.2`)
// ============================================================================

pub const SMART_HANDLE_COLOR_LIGHT: RgbaColorToken = RgbaColorToken::new(209, 36, 163, 255);
pub const SMART_HANDLE_COLOR_DARK: RgbaColorToken = RgbaColorToken::new(240, 82, 196, 255);

pub const PS_HINT_STEM_FILL_LIGHT: RgbaColorToken = RgbaColorToken::new(66, 133, 244, 48);
pub const PS_HINT_STEM_FILL_DARK: RgbaColorToken = RgbaColorToken::new(100, 162, 255, 58);

pub const PS_HINT_GHOST_FILL_LIGHT: RgbaColorToken = RgbaColorToken::new(142, 142, 147, 42);
pub const PS_HINT_GHOST_FILL_DARK: RgbaColorToken = RgbaColorToken::new(174, 174, 178, 52);

pub const HOI_DIAMOND_COLOR_LIGHT: RgbaColorToken = RgbaColorToken::new(217, 95, 14, 255);
pub const HOI_DIAMOND_COLOR_DARK: RgbaColorToken = RgbaColorToken::new(255, 145, 77, 255);

pub const TOUCH_LOUPE_FILL_LIGHT: RgbaColorToken = RgbaColorToken::new(250, 250, 252, 224);
pub const TOUCH_LOUPE_FILL_DARK: RgbaColorToken = RgbaColorToken::new(38, 38, 42, 224);
pub const TOUCH_LOUPE_RING_LIGHT: RgbaColorToken = RgbaColorToken::new(43, 120, 228, 235);
pub const TOUCH_LOUPE_RING_DARK: RgbaColorToken = RgbaColorToken::new(86, 147, 224, 240);

pub const REF_FRAME_OUTER_BOX_LIGHT: RgbaColorToken = RgbaColorToken::new(140, 148, 160, 175);
pub const REF_FRAME_OUTER_BOX_DARK: RgbaColorToken = RgbaColorToken::new(130, 140, 156, 175);
pub const REF_FRAME_INNER_BOX_LIGHT: RgbaColorToken = RgbaColorToken::new(72, 132, 224, 165);
pub const REF_FRAME_INNER_BOX_DARK: RgbaColorToken = RgbaColorToken::new(98, 158, 244, 175);
pub const REF_FRAME_SUBDIV_LIGHT: RgbaColorToken = RgbaColorToken::new(72, 132, 224, 96);
pub const REF_FRAME_SUBDIV_DARK: RgbaColorToken = RgbaColorToken::new(98, 158, 244, 110);
pub const REF_FRAME_KEYLINE_LIGHT: RgbaColorToken = RgbaColorToken::new(168, 92, 214, 140);
pub const REF_FRAME_KEYLINE_DARK: RgbaColorToken = RgbaColorToken::new(196, 124, 240, 150);
pub const GRADIENT_HANDLE_RING_LIGHT: RgbaColorToken = RgbaColorToken::new(32, 36, 44, 230);
pub const GRADIENT_HANDLE_RING_DARK: RgbaColorToken = RgbaColorToken::new(240, 242, 248, 235);
pub const BACKDROP_GHOST_FILL_LIGHT: RgbaColorToken = RgbaColorToken::new(148, 156, 168, 52);
pub const BACKDROP_GHOST_FILL_DARK: RgbaColorToken = RgbaColorToken::new(132, 142, 158, 62);
pub const PIXEL_PREVIEW_CELL_FILL_LIGHT: RgbaColorToken = RgbaColorToken::new(72, 132, 224, 48);
pub const PIXEL_PREVIEW_CELL_FILL_DARK: RgbaColorToken = RgbaColorToken::new(104, 164, 248, 58);
pub const PIXEL_PREVIEW_GRID_LINE_LIGHT: RgbaColorToken = RgbaColorToken::new(140, 148, 160, 42);
pub const PIXEL_PREVIEW_GRID_LINE_DARK: RgbaColorToken = RgbaColorToken::new(130, 140, 156, 48);
pub const TT_GRID_FIT_OUTLINE_LIGHT: RgbaColorToken = RgbaColorToken::new(28, 148, 84, 220);
pub const TT_GRID_FIT_OUTLINE_DARK: RgbaColorToken = RgbaColorToken::new(64, 204, 120, 225);
pub const TT_INSTRUCTION_COLOR_LIGHT: RgbaColorToken = RgbaColorToken::new(186, 68, 214, 235);
pub const TT_INSTRUCTION_COLOR_DARK: RgbaColorToken = RgbaColorToken::new(216, 108, 244, 240);
pub const HOI_PREV_DIAMOND_LIGHT: RgbaColorToken = RgbaColorToken::new(52, 132, 228, 230);
pub const HOI_PREV_DIAMOND_DARK: RgbaColorToken = RgbaColorToken::new(92, 164, 248, 235);
pub const HOI_NEXT_DIAMOND_LIGHT: RgbaColorToken = RgbaColorToken::new(36, 168, 108, 230);
pub const HOI_NEXT_DIAMOND_DARK: RgbaColorToken = RgbaColorToken::new(76, 208, 140, 235);

#[must_use]
pub fn rgba_u8_to_color32(rgba: [u8; 4]) -> egui::Color32 {
    egui::Color32::from_rgba_unmultiplied(rgba[0], rgba[1], rgba[2], rgba[3])
}

#[must_use]
pub fn ref_frame_outer_color32(is_dark: bool) -> egui::Color32 {
    if is_dark {
        REF_FRAME_OUTER_BOX_DARK.to_color32()
    } else {
        REF_FRAME_OUTER_BOX_LIGHT.to_color32()
    }
}

#[must_use]
pub fn ref_frame_inner_color32(is_dark: bool) -> egui::Color32 {
    if is_dark {
        REF_FRAME_INNER_BOX_DARK.to_color32()
    } else {
        REF_FRAME_INNER_BOX_LIGHT.to_color32()
    }
}

#[must_use]
pub fn ref_frame_subdiv_color32(is_dark: bool) -> egui::Color32 {
    if is_dark {
        REF_FRAME_SUBDIV_DARK.to_color32()
    } else {
        REF_FRAME_SUBDIV_LIGHT.to_color32()
    }
}

#[must_use]
pub fn ref_frame_keyline_color32(is_dark: bool) -> egui::Color32 {
    if is_dark {
        REF_FRAME_KEYLINE_DARK.to_color32()
    } else {
        REF_FRAME_KEYLINE_LIGHT.to_color32()
    }
}

#[must_use]
pub fn gradient_handle_ring_color32(is_dark: bool) -> egui::Color32 {
    if is_dark {
        GRADIENT_HANDLE_RING_DARK.to_color32()
    } else {
        GRADIENT_HANDLE_RING_LIGHT.to_color32()
    }
}

pub const fn default_guide_stroke_width_dp() -> f32 {
    DEFAULT_GUIDE_STROKE_WIDTH_DP
}

pub const fn default_metric_line_width_dp() -> f32 {
    DEFAULT_METRIC_LINE_WIDTH_DP
}

pub const fn default_smart_handle_color_rgba() -> RgbaColorToken {
    SMART_HANDLE_COLOR_LIGHT
}

pub const fn default_ps_hint_stem_fill_rgba() -> RgbaColorToken {
    PS_HINT_STEM_FILL_LIGHT
}

pub const fn default_ps_hint_ghost_fill_rgba() -> RgbaColorToken {
    PS_HINT_GHOST_FILL_LIGHT
}

pub const fn default_hoi_diamond_color_rgba() -> RgbaColorToken {
    HOI_DIAMOND_COLOR_LIGHT
}

pub const fn default_backdrop_ghost_fill_rgba() -> RgbaColorToken {
    BACKDROP_GHOST_FILL_LIGHT
}

pub const fn default_ref_frame_outer_rgba() -> RgbaColorToken {
    REF_FRAME_OUTER_BOX_LIGHT
}

pub const fn default_ref_frame_inner_rgba() -> RgbaColorToken {
    REF_FRAME_INNER_BOX_LIGHT
}

pub const fn default_ref_frame_subdiv_rgba() -> RgbaColorToken {
    REF_FRAME_SUBDIV_LIGHT
}

pub const fn default_ref_frame_keyline_rgba() -> RgbaColorToken {
    REF_FRAME_KEYLINE_LIGHT
}

#[must_use]
pub fn pixel_preview_cell_fill_color32(is_dark: bool) -> egui::Color32 {
    if is_dark {
        PIXEL_PREVIEW_CELL_FILL_DARK.to_color32()
    } else {
        PIXEL_PREVIEW_CELL_FILL_LIGHT.to_color32()
    }
}

#[must_use]
pub fn pixel_preview_grid_line_color32(is_dark: bool) -> egui::Color32 {
    if is_dark {
        PIXEL_PREVIEW_GRID_LINE_DARK.to_color32()
    } else {
        PIXEL_PREVIEW_GRID_LINE_LIGHT.to_color32()
    }
}

#[must_use]
pub fn tt_grid_fit_outline_color32(is_dark: bool) -> egui::Color32 {
    if is_dark {
        TT_GRID_FIT_OUTLINE_DARK.to_color32()
    } else {
        TT_GRID_FIT_OUTLINE_LIGHT.to_color32()
    }
}

#[must_use]
pub fn tt_instruction_color32(is_dark: bool) -> egui::Color32 {
    if is_dark {
        TT_INSTRUCTION_COLOR_DARK.to_color32()
    } else {
        TT_INSTRUCTION_COLOR_LIGHT.to_color32()
    }
}

#[must_use]
pub fn hoi_prev_diamond_color32(is_dark: bool) -> egui::Color32 {
    if is_dark {
        HOI_PREV_DIAMOND_DARK.to_color32()
    } else {
        HOI_PREV_DIAMOND_LIGHT.to_color32()
    }
}

#[must_use]
pub fn hoi_next_diamond_color32(is_dark: bool) -> egui::Color32 {
    if is_dark {
        HOI_NEXT_DIAMOND_DARK.to_color32()
    } else {
        HOI_NEXT_DIAMOND_LIGHT.to_color32()
    }
}

#[must_use]
pub fn guide_band_fill_color32(guide_color: egui::Color32) -> egui::Color32 {
    let [r, g, b, a] = guide_color.to_srgba_unmultiplied();
    let scaled_a = (f32::from(a) * GUIDE_BAND_ALPHA_SCALE)
        .round()
        .clamp(0.0, 255.0) as u8;
    egui::Color32::from_rgba_unmultiplied(r, g, b, scaled_a)
}

#[must_use]
pub fn edit_active_fill_color32(fill_color: egui::Color32) -> egui::Color32 {
    let [r, g, b, _] = fill_color.to_srgba_unmultiplied();
    egui::Color32::from_rgba_unmultiplied(r, g, b, EDIT_ACTIVE_FILL_ALPHA)
}

#[must_use]
pub fn full_crosshair_color32(stroke_color: egui::Color32) -> egui::Color32 {
    let [r, g, b, _] = stroke_color.to_srgba_unmultiplied();
    egui::Color32::from_rgba_unmultiplied(r, g, b, FULL_CROSSHAIR_ALPHA)
}

#[must_use]
pub fn modulate_color32_opacity(color: egui::Color32, opacity: f32) -> egui::Color32 {
    let clamped = opacity.clamp(0.0, 1.0);
    if (clamped - 1.0).abs() <= f32::EPSILON {
        return color;
    }
    let [r, g, b, a] = color.to_srgba_unmultiplied();
    let next_a = (f32::from(a) * clamped).round().clamp(0.0, 255.0) as u8;
    egui::Color32::from_rgba_unmultiplied(r, g, b, next_a)
}

#[must_use]
pub fn touch_loupe_fill_color32(is_dark: bool) -> egui::Color32 {
    if is_dark {
        TOUCH_LOUPE_FILL_DARK.to_color32()
    } else {
        TOUCH_LOUPE_FILL_LIGHT.to_color32()
    }
}

#[must_use]
pub fn touch_loupe_ring_color32(is_dark: bool) -> egui::Color32 {
    if is_dark {
        TOUCH_LOUPE_RING_DARK.to_color32()
    } else {
        TOUCH_LOUPE_RING_LIGHT.to_color32()
    }
}

impl GrafiaTheme {
    /// Loads default theme embedded from `tokens/grafia_theme_sonoma.json`.
    #[must_use]
    pub fn sonoma_default() -> Self {
        let json_str = include_str!("../../tokens/grafia_theme_sonoma.json");
        serde_json::from_str(json_str).unwrap_or_default()
    }

    fn themed_hex(&self, dark: &str, light: &str) -> egui::Color32 {
        hex_to_color32(if self.is_dark() { dark } else { light })
    }

    #[must_use]
    pub fn canvas_background_color32(&self) -> egui::Color32 {
        self.themed_hex(DARK_CANVAS_BACKGROUND_HEX, "#FFFFFF")
    }

    #[must_use]
    pub fn sidebar_background_color32(&self) -> egui::Color32 {
        self.themed_hex(DARK_SIDEBAR_BACKGROUND_HEX, "#F6F6F6")
    }

    #[must_use]
    pub fn infobox_background_color32(&self) -> egui::Color32 {
        self.themed_hex(DARK_INFOBOX_BACKGROUND_HEX, "#ECECEC")
    }

    #[must_use]
    pub fn splitter_color32(&self) -> egui::Color32 {
        self.themed_hex(DARK_SPLITTER_HEX, "#E1E1E1")
    }

    #[must_use]
    pub fn start_node_arrow_color32(&self) -> egui::Color32 {
        hex_to_color32("#0066FFDC")
    }

    #[must_use]
    pub fn metrics_guide_color32(&self) -> egui::Color32 {
        self.themed_hex(DARK_METRICS_GUIDE_HEX, "#EAEAEA")
    }

    #[must_use]
    pub fn metrics_label_color32(&self) -> egui::Color32 {
        self.themed_hex(DARK_METRICS_LABEL_HEX, "#8E8E93")
    }

    #[must_use]
    pub fn open_endpoint_color32(&self) -> egui::Color32 {
        self.themed_hex(DARK_OPEN_ENDPOINT_HEX, "#007AFF")
    }
}
