//! `SPEC-007` (`grafia v0.6.0`) tokens: Bottom Preview Strip with Separable 2-Pass Gaussian Blur,
//! 5-Mode Contextual `SelectionInfo` HUD, 20 dp Top/Left Canvas Rulers (`Cmd+R`),
//! Background Reference Layer (`Cmd+B`, `Cmd+J`, `Cmd+K`), and Default Glyph Anchors (`Cmd+U`).

use crate::tokens::{GrafiaTheme, RgbaColorToken};

/// Minimum Gaussian blur radius in pixels (`0.0 px` = sharp bypass).
pub const PREVIEW_BLUR_MIN_RADIUS_PX: f32 = 0.0;

/// Maximum Gaussian blur radius in pixels (`20.0 px`).
pub const PREVIEW_BLUR_MAX_RADIUS_PX: f32 = 20.0;

/// Default active Gaussian blur radius in pixels (`4.5 px`) when toggled on.
pub const PREVIEW_BLUR_DEFAULT_RADIUS_PX: f32 = 4.5;

/// Threshold (`0.1 px`) above which the separable 2-pass Gaussian blur filter is active.
pub const PREVIEW_BLUR_ACTIVE_THRESHOLD_PX: f32 = 0.1;

/// Default vertical extra spacing multiplier (`1.25`) for the bottom preview strip.
pub const PREVIEW_DEFAULT_EXTRA_SPACING: f64 = 1.25;

/// Vertical scale factor (`0.8`) when either `more_space_above` or `more_space_below` is enabled.
pub const PREVIEW_ONE_EXTRA_SPACE_SCALE: f64 = 0.8;

/// Vertical scale factor (`0.7`) when both `more_space_above` and `more_space_below` are enabled.
pub const PREVIEW_BOTH_EXTRA_SPACE_SCALE: f64 = 0.7;

/// Thickness in `dp` (`20.0 dp`) of the top horizontal and left vertical canvas rulers.
pub const RULER_THICKNESS_DP: f32 = 20.0;

/// Multiplier (`3.0`) converting Gaussian $\sigma$ to discrete kernel support radius $R = \lceil 3\sigma \rceil$.
pub const GAUSSIAN_SIGMA_RADIUS_MULTIPLIER: f32 = 3.0;

/// Minimum kernel weight sum (`1e-6`) guard for 1D Gaussian normalization.
pub const GAUSSIAN_MIN_WEIGHT_SUM: f32 = 1e-6;

/// Number of color channels (`4`) per RGBA pixel.
pub const RGBA_CHANNEL_COUNT: usize = 4;

/// Maximum `u8` channel value (`255.0`) as `f32`.
pub const RGBA_MAX_U8_F32: f32 = 255.0;

/// Minimum radial handle length (`1e-9 UPM`) before falling back to horizontal unit direction.
pub const HANDLE_RADIAL_LEN_EPS_UPM: f64 = 1e-9;

/// Minimum bounding span (`1e-6 UPM`) for multi-node bounding-box scaling.
pub const MULTI_SCALE_SPAN_EPS_UPM: f64 = 1e-6;

/// Horizontal guide angle (`0.0` deg) spawned when dragging from the top ruler.
pub const RULER_GUIDE_HORIZONTAL_ANGLE_DEG: f64 = 0.0;

/// Vertical guide angle (`90.0` deg) spawned when dragging from the left ruler.
pub const RULER_GUIDE_VERTICAL_ANGLE_DEG: f64 = 90.0;

/// Candidate `1–2–5` major UPM intervals for zoom-adaptive canvas rulers (`SPEC-007 §2.3.1`).
pub const RULER_CANDIDATE_MAJOR_STEPS_UPM: [f64; 10] = [
    5.0, 10.0, 20.0, 50.0, 100.0, 250.0, 500.0, 1000.0, 2500.0, 5000.0,
];

/// Minimum screen-space distance (`52.0 dp`) between major ruler ticks before stepping up.
pub const RULER_MIN_MAJOR_SPACING_DP: f64 = 52.0;

/// Number of minor subdivisions (`5.0`) per major ruler interval (`minor = major / 5.0`).
pub const RULER_MINOR_SUBDIVISIONS: f64 = 5.0;

/// Minimum zoom clamp (`0.01`) for ruler step computation.
pub const RULER_MIN_ZOOM_CLAMP: f32 = 0.01;

/// Floating Grey Info Box HUD width (`480.0 dp`).
pub const HUD_BOX_WIDTH_DP: f32 = 480.0;

/// Floating Grey Info Box HUD height (`40.0 dp`).
pub const HUD_BOX_HEIGHT_DP: f32 = 40.0;

/// Floating Grey Info Box HUD bottom margin (`14.0 dp`).
pub const HUD_BOTTOM_MARGIN_DP: f32 = 14.0;

/// Floating Grey Info Box HUD corner radius (`6.0 dp`).
pub const HUD_CORNER_RADIUS_DP: f32 = 6.0;

/// Horizontal text padding (`14.0 dp`) inside the Grey Info Box HUD.
pub const HUD_TEXT_PAD_X_DP: f32 = 14.0;

/// Horizontal text offset (`68.0 dp`) inside the Grey Info Box HUD in `MultiNode` mode (after 3x3 grid & lock icon).
pub const HUD_MULTI_TEXT_OFFSET_X_DP: f32 = 68.0;

/// Line 1 vertical offset (`8.0 dp`) inside the Grey Info Box HUD.
pub const HUD_LINE1_OFFSET_Y_DP: f32 = 8.0;

/// Line 2 vertical offset (`23.0 dp`) inside the Grey Info Box HUD.
pub const HUD_LINE2_OFFSET_Y_DP: f32 = 23.0;

/// Primary font size (`11.0 dp`) inside the Grey Info Box HUD (`egui`).
pub const HUD_FONT_SIZE_PRIMARY_DP: f32 = 11.0;

/// Secondary font size (`9.5 dp`) inside the Grey Info Box HUD (`egui`).
pub const HUD_FONT_SIZE_SECONDARY_DP: f32 = 9.5;

/// Top-left dot X offset (`12.0 dp`) of the 3x3 Transform-Grid inside the Grey Info Box HUD.
pub const HUD_GRID_ORIGIN_OFFSET_X_DP: f32 = 12.0;

/// Top-left dot Y offset (`11.0 dp`) of the 3x3 Transform-Grid inside the Grey Info Box HUD.
pub const HUD_GRID_ORIGIN_OFFSET_Y_DP: f32 = 11.0;

/// Step (`9.0 dp`) between adjacent dots in the 3x3 Transform-Grid inside the Grey Info Box HUD.
pub const HUD_GRID_STEP_DP: f32 = 9.0;

/// Radius (`2.2 dp`) of an inactive dot in the 3x3 Transform-Grid.
pub const HUD_GRID_DOT_RADIUS_DP: f32 = 2.2;

/// Radius (`3.8 dp`) of the active `origin_anchor` dot in the 3x3 Transform-Grid.
pub const HUD_GRID_DOT_ACTIVE_RADIUS_DP: f32 = 3.8;

/// Hit-test radius (`4.4 dp`) for clicking a dot in the 3x3 Transform-Grid on the canvas.
pub const HUD_GRID_DOT_HIT_RADIUS_DP: f32 = 4.4;

/// X offset (`42.0 dp`) of the proportional scale icon inside the Grey Info Box HUD.
pub const HUD_PROP_ICON_OFFSET_X_DP: f32 = 42.0;

/// Y offset (`13.0 dp`) of the proportional scale icon inside the Grey Info Box HUD.
pub const HUD_PROP_ICON_OFFSET_Y_DP: f32 = 13.0;

/// Size (`14.0 dp`) of the proportional scale icon box inside the Grey Info Box HUD.
pub const HUD_PROP_ICON_SIZE_DP: f32 = 14.0;

/// Grey Info Box HUD background fill in Light Mode (`RGBA(236, 236, 240, 242)`).
pub const HUD_BG_FILL_LIGHT: RgbaColorToken = RgbaColorToken::new(236, 236, 240, 242);
/// Grey Info Box HUD background fill in Dark Mode (`RGBA(44, 44, 48, 242)`).
pub const HUD_BG_FILL_DARK: RgbaColorToken = RgbaColorToken::new(44, 44, 48, 242);

/// Grey Info Box HUD border stroke in Light Mode (`RGBA(198, 198, 204, 255)`).
pub const HUD_BORDER_STROKE_LIGHT: RgbaColorToken = RgbaColorToken::new(198, 198, 204, 255);
/// Grey Info Box HUD border stroke in Dark Mode (`RGBA(72, 72, 78, 255)`).
pub const HUD_BORDER_STROKE_DARK: RgbaColorToken = RgbaColorToken::new(72, 72, 78, 255);

/// Grey Info Box HUD primary text color in Light Mode (`RGBA(28, 28, 32, 255)`).
pub const HUD_TEXT_PRIMARY_LIGHT: RgbaColorToken = RgbaColorToken::new(28, 28, 32, 255);
/// Grey Info Box HUD primary text color in Dark Mode (`RGBA(235, 235, 240, 255)`).
pub const HUD_TEXT_PRIMARY_DARK: RgbaColorToken = RgbaColorToken::new(235, 235, 240, 255);

/// Grey Info Box HUD secondary text color in Light Mode (`RGBA(92, 92, 100, 255)`).
pub const HUD_TEXT_SECONDARY_LIGHT: RgbaColorToken = RgbaColorToken::new(92, 92, 100, 255);
/// Grey Info Box HUD secondary text color in Dark Mode (`RGBA(168, 168, 178, 255)`).
pub const HUD_TEXT_SECONDARY_DARK: RgbaColorToken = RgbaColorToken::new(168, 168, 178, 255);

/// Inactive 3x3 Transform-Grid dot fill in Light Mode (`RGBA(148, 148, 156, 255)`).
pub const HUD_DOT_INACTIVE_LIGHT: RgbaColorToken = RgbaColorToken::new(148, 148, 156, 255);
/// Inactive 3x3 Transform-Grid dot fill in Dark Mode (`RGBA(118, 118, 128, 255)`).
pub const HUD_DOT_INACTIVE_DARK: RgbaColorToken = RgbaColorToken::new(118, 118, 128, 255);

/// Active 3x3 Transform-Grid dot & proportional lock highlight fill (`RGBA(0, 122, 255, 255)`).
pub const HUD_ACCENT_FILL: RgbaColorToken = RgbaColorToken::new(0, 122, 255, 255);

/// Major tick length (`8.0 dp`) on canvas rulers.
pub const RULER_MAJOR_TICK_LEN_DP: f32 = 8.0;

/// Minor tick length (`4.0 dp`) on canvas rulers.
pub const RULER_MINOR_TICK_LEN_DP: f32 = 4.0;

/// Font size (`8.5 dp`) for coordinate labels on canvas rulers.
pub const RULER_LABEL_FONT_SIZE_DP: f32 = 8.5;

/// Padding offset (`3.0 dp`) for coordinate labels on canvas rulers.
pub const RULER_LABEL_OFFSET_DP: f32 = 3.0;

/// Default preview strip render width (`800 px`).
pub const DEFAULT_PREVIEW_STRIP_WIDTH_PX: u32 = 800;

/// Default preview strip render height (`120 px`).
pub const DEFAULT_PREVIEW_STRIP_HEIGHT_PX: u32 = 120;

/// Preview normal background fill in Light Mode (`RGBA(250, 248, 245, 255)`).
pub const PREVIEW_BG_NORMAL_LIGHT: RgbaColorToken = RgbaColorToken::new(250, 248, 245, 255);
/// Preview normal background fill in Dark Mode (`RGBA(30, 30, 32, 255)`).
pub const PREVIEW_BG_NORMAL_DARK: RgbaColorToken = RgbaColorToken::new(30, 30, 32, 255);

/// Preview normal foreground fill in Light Mode (`RGBA(24, 24, 26, 255)`).
pub const PREVIEW_FG_NORMAL_LIGHT: RgbaColorToken = RgbaColorToken::new(24, 24, 26, 255);
/// Preview normal foreground fill in Dark Mode (`RGBA(235, 235, 240, 255)`).
pub const PREVIEW_FG_NORMAL_DARK: RgbaColorToken = RgbaColorToken::new(235, 235, 240, 255);

/// Preview inverted background fill in Light Mode (`RGBA(24, 24, 26, 255)`).
pub const PREVIEW_BG_INVERTED_LIGHT: RgbaColorToken = RgbaColorToken::new(24, 24, 26, 255);
/// Preview inverted background fill in Dark Mode (`RGBA(250, 248, 245, 255)`).
pub const PREVIEW_BG_INVERTED_DARK: RgbaColorToken = RgbaColorToken::new(250, 248, 245, 255);

/// Preview inverted foreground fill in Light Mode (`RGBA(235, 235, 240, 255)`).
pub const PREVIEW_FG_INVERTED_LIGHT: RgbaColorToken = RgbaColorToken::new(235, 235, 240, 255);
/// Preview inverted foreground fill in Dark Mode (`RGBA(24, 24, 26, 255)`).
pub const PREVIEW_FG_INVERTED_DARK: RgbaColorToken = RgbaColorToken::new(24, 24, 26, 255);

/// Canvas ruler bar fill in Light Mode (`RGBA(242, 240, 236, 245)`).
pub const RULER_BG_FILL_LIGHT: RgbaColorToken = RgbaColorToken::new(242, 240, 236, 245);
/// Canvas ruler bar fill in Dark Mode (`RGBA(38, 38, 42, 245)`).
pub const RULER_BG_FILL_DARK: RgbaColorToken = RgbaColorToken::new(38, 38, 42, 245);

/// Canvas ruler border stroke in Light Mode (`RGBA(205, 203, 198, 255)`).
pub const RULER_BORDER_STROKE_LIGHT: RgbaColorToken = RgbaColorToken::new(205, 203, 198, 255);
/// Canvas ruler border stroke in Dark Mode (`RGBA(62, 62, 68, 255)`).
pub const RULER_BORDER_STROKE_DARK: RgbaColorToken = RgbaColorToken::new(62, 62, 68, 255);

/// Canvas ruler tick/label color in Light Mode (`RGBA(115, 115, 122, 255)`).
pub const RULER_TICK_COLOR_LIGHT: RgbaColorToken = RgbaColorToken::new(115, 115, 122, 255);
/// Canvas ruler tick/label color in Dark Mode (`RGBA(165, 165, 175, 255)`).
pub const RULER_TICK_COLOR_DARK: RgbaColorToken = RgbaColorToken::new(165, 165, 175, 255);

/// Live cursor tracking hairline on rulers in Light Mode (`RGBA(46, 117, 219, 220)`).
pub const RULER_HAIRLINE_COLOR_LIGHT: RgbaColorToken = RgbaColorToken::new(46, 117, 219, 220);
/// Live cursor tracking hairline on rulers in Dark Mode (`RGBA(74, 144, 242, 230)`).
pub const RULER_HAIRLINE_COLOR_DARK: RgbaColorToken = RgbaColorToken::new(74, 144, 242, 230);

/// Background reference layer contour stroke in Light Mode (`RGBA(185, 115, 115, 170)`).
pub const BACKGROUND_LAYER_STROKE_LIGHT: RgbaColorToken = RgbaColorToken::new(185, 115, 115, 170);
/// Background reference layer contour stroke in Dark Mode (`RGBA(215, 130, 130, 175)`).
pub const BACKGROUND_LAYER_STROKE_DARK: RgbaColorToken = RgbaColorToken::new(215, 130, 130, 175);

#[must_use]
pub const fn default_preview_bg_normal_rgba() -> RgbaColorToken {
    PREVIEW_BG_NORMAL_LIGHT
}
#[must_use]
pub const fn default_preview_fg_normal_rgba() -> RgbaColorToken {
    PREVIEW_FG_NORMAL_LIGHT
}
#[must_use]
pub const fn default_preview_bg_inverted_rgba() -> RgbaColorToken {
    PREVIEW_BG_INVERTED_LIGHT
}
#[must_use]
pub const fn default_preview_fg_inverted_rgba() -> RgbaColorToken {
    PREVIEW_FG_INVERTED_LIGHT
}
#[must_use]
pub const fn default_ruler_bg_fill_rgba() -> RgbaColorToken {
    RULER_BG_FILL_LIGHT
}
#[must_use]
pub const fn default_ruler_border_stroke_rgba() -> RgbaColorToken {
    RULER_BORDER_STROKE_LIGHT
}
#[must_use]
pub const fn default_ruler_tick_color_rgba() -> RgbaColorToken {
    RULER_TICK_COLOR_LIGHT
}
#[must_use]
pub const fn default_ruler_hairline_color_rgba() -> RgbaColorToken {
    RULER_HAIRLINE_COLOR_LIGHT
}
#[must_use]
pub const fn default_background_layer_stroke_rgba() -> RgbaColorToken {
    BACKGROUND_LAYER_STROKE_LIGHT
}

/// Converts a 4-byte RGBA slice from the preview raster buffer into `egui::Color32`.
#[must_use]
pub fn preview_rgba_slice_to_color32(rgba: &[u8]) -> egui::Color32 {
    egui::Color32::from_rgba_unmultiplied(rgba[0], rgba[1], rgba[2], rgba[3])
}

impl GrafiaTheme {
    #[must_use]
    pub fn ruler_bg_fill_color32(&self) -> egui::Color32 {
        (if self.is_dark() {
            RULER_BG_FILL_DARK
        } else {
            self.ruler_bg_fill
        })
        .to_color32()
    }
    #[must_use]
    pub fn ruler_border_stroke_color32(&self) -> egui::Color32 {
        (if self.is_dark() {
            RULER_BORDER_STROKE_DARK
        } else {
            self.ruler_border_stroke
        })
        .to_color32()
    }
    #[must_use]
    pub fn ruler_tick_color32(&self) -> egui::Color32 {
        (if self.is_dark() {
            RULER_TICK_COLOR_DARK
        } else {
            self.ruler_tick_color
        })
        .to_color32()
    }
    #[must_use]
    pub fn ruler_hairline_color32(&self) -> egui::Color32 {
        (if self.is_dark() {
            RULER_HAIRLINE_COLOR_DARK
        } else {
            self.ruler_hairline_color
        })
        .to_color32()
    }
    #[must_use]
    pub fn background_layer_stroke_color32(&self) -> egui::Color32 {
        (if self.is_dark() {
            BACKGROUND_LAYER_STROKE_DARK
        } else {
            self.background_layer_stroke
        })
        .to_color32()
    }
    #[must_use]
    pub fn hud_bg_fill_color32(&self) -> egui::Color32 {
        (if self.is_dark() {
            HUD_BG_FILL_DARK
        } else {
            HUD_BG_FILL_LIGHT
        })
        .to_color32()
    }
    #[must_use]
    pub fn hud_border_stroke_color32(&self) -> egui::Color32 {
        (if self.is_dark() {
            HUD_BORDER_STROKE_DARK
        } else {
            HUD_BORDER_STROKE_LIGHT
        })
        .to_color32()
    }
    #[must_use]
    pub fn hud_text_primary_color32(&self) -> egui::Color32 {
        (if self.is_dark() {
            HUD_TEXT_PRIMARY_DARK
        } else {
            HUD_TEXT_PRIMARY_LIGHT
        })
        .to_color32()
    }
    #[must_use]
    pub fn hud_text_secondary_color32(&self) -> egui::Color32 {
        (if self.is_dark() {
            HUD_TEXT_SECONDARY_DARK
        } else {
            HUD_TEXT_SECONDARY_LIGHT
        })
        .to_color32()
    }
    #[must_use]
    pub fn hud_dot_inactive_color32(&self) -> egui::Color32 {
        (if self.is_dark() {
            HUD_DOT_INACTIVE_DARK
        } else {
            HUD_DOT_INACTIVE_LIGHT
        })
        .to_color32()
    }
    #[must_use]
    pub fn hud_accent_fill_color32(&self) -> egui::Color32 {
        HUD_ACCENT_FILL.to_color32()
    }
}
