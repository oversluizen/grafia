//! `SPEC-006` (`grafia v0.5.0`) tokens: Multi-Glyph Horizontal Text Strip (`CanvasTextStream`),
//! Interactive Spacing (`LSB` / `RSB` / Contour Shift), Kerning Baseline Highlight Wedges,
//! and Text-Mode Measurement Scanline.

use crate::tokens::{GrafiaTheme, RgbaColorToken};

/// Default zoom-times-UPM threshold (`100.0`) for displaying live metrics numbers in text mode.
pub const TEXT_MODE_NUMBERS_THRESHOLD_DEFAULT: f64 = 100.0;

/// Default vertical scanline position in UPM (`200.0`) for the text-mode measurement line.
pub const TEXT_MEASUREMENT_LINE_DEFAULT_Y_UPM: f64 = 200.0;

/// Low step increment (`1` UPM) for keyboard spacing adjustments (`Ctrl/Cmd + Left/Right`).
pub const SPACING_INCREMENT_LOW_UPM: i32 = 1;

/// High step increment (`10` UPM) for keyboard spacing adjustments (`Ctrl/Cmd + Shift + Left/Right`).
pub const SPACING_INCREMENT_HIGH_UPM: i32 = 10;

/// Low step increment (`1` UPM) for keyboard kerning adjustments (`Ctrl/Cmd + Opt + Left/Right`).
pub const KERNING_INCREMENT_LOW_UPM: i32 = 1;

/// High step increment (`10` UPM) for keyboard kerning adjustments (`Ctrl/Cmd + Opt + Shift + Left/Right`).
pub const KERNING_INCREMENT_HIGH_UPM: i32 = 10;

/// Minimum pointer drag distance in `dp` (`2.0 dp`) before sweeping a text selection range.
pub const TEXT_SELECTION_DRAG_THRESHOLD_DP: f32 = 2.0;

/// Screen-space vertical hit radius (`8.0 dp`) for grabbing the text-mode measurement scanline.
pub const TEXT_MEASUREMENT_LINE_HIT_RADIUS_DP: f32 = 8.0;

/// Font-space vertical hit tolerance (`15.0 UPM`) for grabbing the text-mode measurement scanline.
pub const TEXT_MEASUREMENT_LINE_HIT_TOL_UPM: f64 = 15.0;

/// Negative kerning highlight fill in Light Mode (`RGBA(28, 158, 209, 51)` — translucent cyan/blue).
pub const KERNING_NEGATIVE_FILL_LIGHT: RgbaColorToken = RgbaColorToken::new(28, 158, 209, 51);

/// Negative kerning highlight fill in Dark Mode (`RGBA(28, 158, 209, 51)` — translucent cyan/blue).
pub const KERNING_NEGATIVE_FILL_DARK: RgbaColorToken = RgbaColorToken::new(28, 158, 209, 51);

/// Positive kerning highlight fill in Light Mode (`RGBA(242, 219, 51, 51)` — translucent amber/yellow).
pub const KERNING_POSITIVE_FILL_LIGHT: RgbaColorToken = RgbaColorToken::new(242, 219, 51, 51);

/// Positive kerning highlight fill in Dark Mode (`RGBA(242, 219, 51, 51)` — translucent amber/yellow).
pub const KERNING_POSITIVE_FILL_DARK: RgbaColorToken = RgbaColorToken::new(242, 219, 51, 51);

/// Passive neighbor glyph silhouette fill in Light Mode (`RGBA(28, 28, 30, 255)`).
pub const PASSIVE_GLYPH_FILL_LIGHT: RgbaColorToken = RgbaColorToken::new(28, 28, 30, 255);

/// Passive neighbor glyph silhouette fill in Dark Mode (`RGBA(235, 235, 240, 255)`).
pub const PASSIVE_GLYPH_FILL_DARK: RgbaColorToken = RgbaColorToken::new(235, 235, 240, 255);

/// Multi-glyph stream text caret stroke color in Light Mode (`RGBA(30, 30, 32, 240)`).
pub const TEXT_CARET_COLOR_LIGHT: RgbaColorToken = RgbaColorToken::new(30, 30, 32, 240);

/// Multi-glyph stream text caret stroke color in Dark Mode (`RGBA(240, 240, 245, 240)`).
pub const TEXT_CARET_COLOR_DARK: RgbaColorToken = RgbaColorToken::new(240, 240, 245, 240);

/// Multi-slot text selection highlight fill in Light Mode (`RGBA(46, 117, 219, 55)`).
pub const TEXT_SELECTION_FILL_LIGHT: RgbaColorToken = RgbaColorToken::new(46, 117, 219, 55);

/// Multi-slot text selection highlight fill in Dark Mode (`RGBA(74, 144, 242, 70)`).
pub const TEXT_SELECTION_FILL_DARK: RgbaColorToken = RgbaColorToken::new(74, 144, 242, 70);

/// Horizontal text-mode measurement scanline stroke in Light Mode (`RGBA(120, 120, 128, 200)`).
pub const TEXT_MEASUREMENT_LINE_COLOR_LIGHT: RgbaColorToken =
    RgbaColorToken::new(120, 120, 128, 200);

/// Horizontal text-mode measurement scanline stroke in Dark Mode (`RGBA(165, 165, 175, 200)`).
pub const TEXT_MEASUREMENT_LINE_COLOR_DARK: RgbaColorToken =
    RgbaColorToken::new(165, 165, 175, 200);

/// Scanline contour/sidebearing intersection knob fill in Light Mode (`RGBA(90, 90, 98, 235)`).
pub const TEXT_MEASUREMENT_KNOB_FILL_LIGHT: RgbaColorToken = RgbaColorToken::new(90, 90, 98, 235);

/// Scanline contour/sidebearing intersection knob fill in Dark Mode (`RGBA(190, 190, 200, 235)`).
pub const TEXT_MEASUREMENT_KNOB_FILL_DARK: RgbaColorToken = RgbaColorToken::new(190, 190, 200, 235);

/// Live LSB/RSB/width/kerning numeric label color in Light Mode (`RGBA(100, 100, 108, 235)`).
pub const TEXT_METRICS_NUMBER_COLOR_LIGHT: RgbaColorToken = RgbaColorToken::new(100, 100, 108, 235);

/// Live LSB/RSB/width/kerning numeric label color in Dark Mode (`RGBA(175, 175, 185, 235)`).
pub const TEXT_METRICS_NUMBER_COLOR_DARK: RgbaColorToken = RgbaColorToken::new(175, 175, 185, 235);

/// Text caret vertical stroke width in `dp` (`2.0 dp`).
pub const TEXT_CARET_STROKE_WIDTH_DP: f32 = 2.0;

/// Horizontal text-mode scanline stroke width in `dp` (`1.0 dp`).
pub const TEXT_MEASUREMENT_LINE_STROKE_WIDTH_DP: f32 = 1.0;

/// Scanline intersection knob radius in `dp` (`3.0 dp`).
pub const TEXT_MEASUREMENT_KNOB_RADIUS_DP: f32 = 3.0;

/// Font size in `dp` (`9.5 dp`) for live text-mode numeric labels.
pub const TEXT_METRICS_NUMBER_FONT_SIZE_DP: f32 = 9.5;

/// Vertical offset in `dp` (`8.0 dp`) for scanline and baseline numeric labels.
pub const TEXT_METRICS_NUMBER_OFFSET_DP: f32 = 8.0;

/// Subdivision count per cubic/quad segment for horizontal scanline intersection.
pub const TEXT_SCANLINE_CUBIC_SUBDIVISIONS: usize = 32;

/// Deduplication tolerance in UPM (`0.05 UPM`) for horizontal scanline intersections.
pub const TEXT_SCANLINE_DEDUP_EPS_UPM: f64 = 0.05;

///Determinant / span epsilon (`1e-7`) for horizontal scanline crossing checks.
pub const TEXT_SCANLINE_T_EPS: f64 = 1e-7;

/// Demo kerning value in UPM (`-40`) between `/A` and `/o` in the default multi-glyph demo strip.
pub const DEMO_STRIP_KERN_AO_UPM: i32 = -40;

/// Demo kerning value in UPM (`25`) between `/s` and `/A` in the default multi-glyph demo strip.
pub const DEMO_STRIP_KERN_SA_UPM: i32 = 25;

/// Tolerance in UPM (`1e-5`) for cumulative slot origin invariant validation.
pub const TEXT_STREAM_ORIGIN_INVARIANT_EPS_UPM: f64 = 1e-5;

#[must_use]
pub const fn default_passive_glyph_fill_rgba() -> RgbaColorToken {
    PASSIVE_GLYPH_FILL_LIGHT
}

#[must_use]
pub const fn default_kerning_negative_fill_rgba() -> RgbaColorToken {
    KERNING_NEGATIVE_FILL_LIGHT
}

#[must_use]
pub const fn default_kerning_positive_fill_rgba() -> RgbaColorToken {
    KERNING_POSITIVE_FILL_LIGHT
}

#[must_use]
pub const fn default_text_caret_color_rgba() -> RgbaColorToken {
    TEXT_CARET_COLOR_LIGHT
}

#[must_use]
pub const fn default_text_selection_fill_rgba() -> RgbaColorToken {
    TEXT_SELECTION_FILL_LIGHT
}

#[must_use]
pub const fn default_text_measurement_line_color_rgba() -> RgbaColorToken {
    TEXT_MEASUREMENT_LINE_COLOR_LIGHT
}

#[must_use]
pub const fn default_text_measurement_knob_fill_rgba() -> RgbaColorToken {
    TEXT_MEASUREMENT_KNOB_FILL_LIGHT
}

#[must_use]
pub const fn default_text_metrics_number_color_rgba() -> RgbaColorToken {
    TEXT_METRICS_NUMBER_COLOR_LIGHT
}

impl GrafiaTheme {
    #[must_use]
    pub fn passive_glyph_fill_color32(&self) -> egui::Color32 {
        (if self.is_dark() {
            PASSIVE_GLYPH_FILL_DARK
        } else {
            self.passive_glyph_fill
        })
        .to_color32()
    }

    #[must_use]
    pub fn kerning_negative_fill_color32(&self) -> egui::Color32 {
        self.kerning_negative_fill.to_color32()
    }

    #[must_use]
    pub fn kerning_positive_fill_color32(&self) -> egui::Color32 {
        self.kerning_positive_fill.to_color32()
    }

    #[must_use]
    pub fn text_caret_color32(&self) -> egui::Color32 {
        (if self.is_dark() {
            TEXT_CARET_COLOR_DARK
        } else {
            self.text_caret_color
        })
        .to_color32()
    }

    #[must_use]
    pub fn text_selection_fill_color32(&self) -> egui::Color32 {
        (if self.is_dark() {
            TEXT_SELECTION_FILL_DARK
        } else {
            self.text_selection_fill
        })
        .to_color32()
    }

    #[must_use]
    pub fn text_measurement_line_color32(&self) -> egui::Color32 {
        (if self.is_dark() {
            TEXT_MEASUREMENT_LINE_COLOR_DARK
        } else {
            self.text_measurement_line_color
        })
        .to_color32()
    }

    #[must_use]
    pub fn text_measurement_knob_fill_color32(&self) -> egui::Color32 {
        (if self.is_dark() {
            TEXT_MEASUREMENT_KNOB_FILL_DARK
        } else {
            self.text_measurement_knob_fill
        })
        .to_color32()
    }

    #[must_use]
    pub fn text_metrics_number_color32(&self) -> egui::Color32 {
        (if self.is_dark() {
            TEXT_METRICS_NUMBER_COLOR_DARK
        } else {
            self.text_metrics_number_color
        })
        .to_color32()
    }
}
