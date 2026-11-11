//! `SPEC-008` (`grafia v0.7.0`) tokens: Background Layer (`Cmd+B`, `Cmd+J`, `Cmd+K`),
//! 2D `CanvasAnchor` & Mark Cloud (`GhostMarkPath`), 2D Pre-Resolved `CanvasComponent`
//! & Decompose (`Cmd+Shift+D`), 8-Step Fit Curve (`Ctrl+Opt+1..8`), Live Contour Stroke
//! & Expand Outline, and `CompanionMasterLayer` / Master Compatibility (`Ctrl+Opt+Cmd+N`).

use crate::tokens::{GrafiaTheme, RgbaColorToken};
use serde::{Deserialize, Serialize};

/// Stable identifier for a 2D pre-resolved `CanvasComponent` (`SPEC-008 §2.3.1`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct ComponentId(pub u64);

/// Kind of text-stream edit requested while in `CanvasTool::Text` (`SPEC-006 §2.2`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum TextStreamEditKind {
    InsertText(String),
    Backspace,
    DeleteForward,
}

/// Outbound Host-Port request emitted when typing or deleting in `CanvasTool::Text` (`SPEC-006 §2.2`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TextStreamEditRequest {
    pub caret_index: usize,
    pub selection_range: Option<(usize, usize)>,
    pub edit: TextStreamEditKind,
}

/// Outbound Host-Port event emitted by `grafia` to the Host Application (`SPEC-006`, `SPEC-007`, `SPEC-008`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum CanvasHostRequest {
    OpenComponentSource {
        id: ComponentId,
        component_name: String,
    },
    ResetDefaultAnchors {
        glyph_name: String,
    },
    EditTextStream(TextStreamEditRequest),
}

/// Stroke width (`1.0 dp`) for background reference layer contours (`SPEC-008 §3`).
pub const BACKGROUND_STROKE_WIDTH_DP: f32 = 1.0;

/// Base ID offset (`100`) when cloning contours into the background layer.
pub const BACKGROUND_CONTOUR_ID_OFFSET: usize = 100;

/// Base NodeId offset (`10_000`) when cloning or decomposing contours.
pub const DECOMPOSE_NODE_ID_OFFSET: u64 = 10_000;

/// Unselected anchor diamond half-size (`4.5 dp`) (`SPEC-008 §3`).
pub const ANCHOR_DIAMOND_RADIUS_DP: f32 = 4.5;

/// Selected anchor diamond half-size (`6.0 dp`) (`SPEC-008 §3`).
pub const ANCHOR_DIAMOND_SELECTED_RADIUS_DP: f32 = 6.0;

/// Screen-space hit-test radius (`7.0 dp`) for glyph anchors (`SPEC-008 §3`).
pub const ANCHOR_HIT_RADIUS_DP: f32 = 7.0;

/// Font size (`9.0 dp`) for glyph anchor name labels.
pub const ANCHOR_LABEL_FONT_SIZE_DP: f32 = 9.0;

/// Screen-space offset (`6.0 dp`) for glyph anchor name labels.
pub const ANCHOR_LABEL_OFFSET_DP: f32 = 6.0;

/// Default minimum Fit Curve handle ratio (`0.57` = `57%`, Column 1 / `Ctrl+Opt+1`).
pub const FIT_CURVE_DEFAULT_MIN_RATIO: f64 = 0.57;

/// Default maximum Fit Curve handle ratio (`0.92` = `92%`, Column 8 / `Ctrl+Opt+8`).
pub const FIT_CURVE_DEFAULT_MAX_RATIO: f64 = 0.92;

/// Maximum chord multiple (`2.0`) for tangent intersection $T$ (`\|T - P_0\| < 2.0 * \|P_3 - P_0\|`).
pub const FIT_CURVE_MAX_INTERSECTION_CHORD_MULTIPLE: f64 = 2.0;

/// Number of intervals (`7.0`) across the 8 Fit Curve steps (`col / 7.0` for `col in 0..=7`).
pub const FIT_CURVE_STEP_DIVISOR: f64 = 7.0;

/// Maximum column index (`7`) in the 8-step Fit Curve ladder.
pub const FIT_CURVE_MAX_COLUMN_INDEX: usize = 7;

/// Relative step delta fraction (`0.125` = `1/8` of `(max_ratio - min_ratio)`) for `fit_curve_step_delta`.
pub const FIT_CURVE_STEP_DELTA_FRACTION: f64 = 0.125;

/// Minimum span clamp (`0.125`) when computing `fit_curve_step_delta`.
pub const FIT_CURVE_MIN_SPAN_RATIO: f64 = 0.125;

/// Minimum allowed `min_ratio` (`0.0`) in `FitCurveConfig`.
pub const FIT_CURVE_CONFIG_MIN_BOUND: f64 = 0.0;

/// Maximum allowed `min_ratio` (`1.0`) in `FitCurveConfig`.
pub const FIT_CURVE_CONFIG_MAX_MIN_BOUND: f64 = 1.0;

/// Minimum allowed handle ratio (`0.05`) when clamping Fit Curve ratios.
pub const FIT_CURVE_MIN_ALLOWED_RATIO: f64 = 0.05;

/// Maximum allowed handle ratio (`1.50`) when clamping Fit Curve ratios.
pub const FIT_CURVE_MAX_ALLOWED_RATIO: f64 = 1.50;

/// Determinant epsilon (`1e-7`) for parallel or divergent tangent lines in Fit Curve.
pub const FIT_CURVE_PARALLEL_DET_EPS: f64 = 1e-7;

/// Minimum chord length (`1e-6 UPM`) guard for Fit Curve and stroke offset math.
pub const FIT_CURVE_MIN_CHORD_LEN_EPS: f64 = 1e-6;

/// Join coincidence tolerance (`1e-4 UPM`) when two offset rails meet smoothly.
pub const STROKE_JOIN_COINCIDENT_EPS_UPM: f64 = 1e-4;

/// Default miter limit (`4.0`) for `LineJoinStyle::Miter` in live contour stroke expansion.
pub const STROKE_MITER_LIMIT: f64 = 4.0;

/// Default stroke width and height (`20.0 UPM`) for `ContourStrokeStyle::default()`.
pub const DEFAULT_CONTOUR_STROKE_WIDTH_UPM: f64 = 20.0;

/// Kappa (`0.5522847498307936`) for quarter-circle cubic Bézier approximation in round caps/joins.
pub const STROKE_ROUND_KAPPA: f64 = 0.552_284_749_830_793_6;

/// Stroke width (`1.25 dp`) for `CompanionMasterLayer` outlines (`SPEC-008 §2.6`).
pub const COMPANION_LAYER_STROKE_WIDTH_DP: f32 = 1.25;

/// Stroke width (`1.25 dp`) for Master Compatibility start-node and anchor rays (`SPEC-008 §2.6`).
pub const MASTER_COMPAT_RAY_STROKE_WIDTH_DP: f32 = 1.25;

/// Stroke width (`2.25 dp`) for incompatible contour highlight outlines (`SPEC-008 §2.6`).
pub const MASTER_INCOMPAT_STROKE_WIDTH_DP: f32 = 2.25;

/// Endpoint dot radius (`3.0 dp`) for Master Compatibility rays (`SPEC-008 §2.6`).
pub const MASTER_COMPAT_DOT_RADIUS_DP: f32 = 3.0;

/// Glyph anchor diamond fill in Light Mode (`RGBA(196, 84, 52, 235)`).
pub const ANCHOR_FILL_LIGHT: RgbaColorToken = RgbaColorToken::new(196, 84, 52, 235);
/// Glyph anchor diamond fill in Dark Mode (`RGBA(232, 112, 78, 240)`).
pub const ANCHOR_FILL_DARK: RgbaColorToken = RgbaColorToken::new(232, 112, 78, 240);

/// Selected glyph anchor diamond fill in Light Mode (`RGBA(255, 185, 40, 255)`).
pub const ANCHOR_SELECTED_FILL_LIGHT: RgbaColorToken = RgbaColorToken::new(255, 185, 40, 255);
/// Selected glyph anchor diamond fill in Dark Mode (`RGBA(255, 195, 55, 255)`).
pub const ANCHOR_SELECTED_FILL_DARK: RgbaColorToken = RgbaColorToken::new(255, 195, 55, 255);
/// Alias for default selected glyph anchor fill.
pub const ANCHOR_SELECTED_FILL: RgbaColorToken = ANCHOR_SELECTED_FILL_LIGHT;

/// Mark preview cloud (`GhostMarkPath`) fill in Light Mode (`RGBA(120, 110, 145, 72)`).
pub const MARK_CLOUD_FILL_LIGHT: RgbaColorToken = RgbaColorToken::new(120, 110, 145, 72);
/// Mark preview cloud (`GhostMarkPath`) fill in Dark Mode (`RGBA(165, 155, 195, 82)`).
pub const MARK_CLOUD_FILL_DARK: RgbaColorToken = RgbaColorToken::new(165, 155, 195, 82);

/// Unselected unlocked `CanvasComponent` fill in Light Mode (`RGBA(110, 115, 125, 140)`).
pub const COMPONENT_FILL_LIGHT: RgbaColorToken = RgbaColorToken::new(110, 115, 125, 140);
/// Unselected unlocked `CanvasComponent` fill in Dark Mode (`RGBA(150, 155, 168, 145)`).
pub const COMPONENT_FILL_DARK: RgbaColorToken = RgbaColorToken::new(150, 155, 168, 145);

/// Selected `CanvasComponent` fill in Light Mode (`RGBA(58, 120, 215, 165)`).
pub const COMPONENT_SELECTED_FILL_LIGHT: RgbaColorToken = RgbaColorToken::new(58, 120, 215, 165);
/// Selected `CanvasComponent` fill in Dark Mode (`RGBA(88, 148, 240, 170)`).
pub const COMPONENT_SELECTED_FILL_DARK: RgbaColorToken = RgbaColorToken::new(88, 148, 240, 170);

/// Locked `CanvasComponent` fill in Light Mode (`RGBA(145, 145, 150, 105)`).
pub const COMPONENT_LOCKED_FILL_LIGHT: RgbaColorToken = RgbaColorToken::new(145, 145, 150, 105);
/// Locked `CanvasComponent` fill in Dark Mode (`RGBA(120, 120, 128, 110)`).
pub const COMPONENT_LOCKED_FILL_DARK: RgbaColorToken = RgbaColorToken::new(120, 120, 128, 110);

/// Master compatibility ray color in Light Mode (`RGBA(40, 150, 95, 195)`).
pub const MASTER_COMPAT_RAY_COLOR_LIGHT: RgbaColorToken = RgbaColorToken::new(40, 150, 95, 195);
/// Master compatibility ray color in Dark Mode (`RGBA(65, 195, 125, 205)`).
pub const MASTER_COMPAT_RAY_COLOR_DARK: RgbaColorToken = RgbaColorToken::new(65, 195, 125, 205);

/// Master incompatibility contour highlight in Light Mode (`RGBA(225, 55, 55, 215)`).
pub const MASTER_INCOMPAT_HIGHLIGHT_LIGHT: RgbaColorToken = RgbaColorToken::new(225, 55, 55, 215);
/// Master incompatibility contour highlight in Dark Mode (`RGBA(245, 80, 80, 225)`).
pub const MASTER_INCOMPAT_HIGHLIGHT_DARK: RgbaColorToken = RgbaColorToken::new(245, 80, 80, 225);

#[must_use]
pub const fn default_anchor_fill_rgba() -> RgbaColorToken {
    ANCHOR_FILL_LIGHT
}
#[must_use]
pub const fn default_anchor_selected_fill_rgba() -> RgbaColorToken {
    ANCHOR_SELECTED_FILL_LIGHT
}
#[must_use]
pub const fn default_mark_cloud_fill_rgba() -> RgbaColorToken {
    MARK_CLOUD_FILL_LIGHT
}
#[must_use]
pub const fn default_component_fill_rgba() -> RgbaColorToken {
    COMPONENT_FILL_LIGHT
}
#[must_use]
pub const fn default_component_selected_fill_rgba() -> RgbaColorToken {
    COMPONENT_SELECTED_FILL_LIGHT
}
#[must_use]
pub const fn default_component_locked_fill_rgba() -> RgbaColorToken {
    COMPONENT_LOCKED_FILL_LIGHT
}
#[must_use]
pub const fn default_master_compat_ray_color_rgba() -> RgbaColorToken {
    MASTER_COMPAT_RAY_COLOR_LIGHT
}
#[must_use]
pub const fn default_master_incompat_highlight_rgba() -> RgbaColorToken {
    MASTER_INCOMPAT_HIGHLIGHT_LIGHT
}

impl GrafiaTheme {
    #[must_use]
    pub fn anchor_fill_color32(&self, selected: bool) -> egui::Color32 {
        if selected {
            let tok = if self.is_dark() && self.anchor_selected_fill == ANCHOR_SELECTED_FILL_LIGHT {
                ANCHOR_SELECTED_FILL_DARK
            } else {
                self.anchor_selected_fill
            };
            tok.to_color32()
        } else {
            let tok = if self.is_dark() && self.anchor_fill == ANCHOR_FILL_LIGHT {
                ANCHOR_FILL_DARK
            } else {
                self.anchor_fill
            };
            tok.to_color32()
        }
    }

    #[must_use]
    pub fn mark_cloud_fill_color32(&self) -> egui::Color32 {
        let tok = if self.is_dark() && self.mark_cloud_fill == MARK_CLOUD_FILL_LIGHT {
            MARK_CLOUD_FILL_DARK
        } else {
            self.mark_cloud_fill
        };
        tok.to_color32()
    }

    #[must_use]
    pub fn component_fill_color32(&self, selected: bool, locked: bool) -> egui::Color32 {
        if locked {
            let tok = if self.is_dark() && self.component_locked_fill == COMPONENT_LOCKED_FILL_LIGHT
            {
                COMPONENT_LOCKED_FILL_DARK
            } else {
                self.component_locked_fill
            };
            tok.to_color32()
        } else if selected {
            let tok = if self.is_dark()
                && self.component_selected_fill == COMPONENT_SELECTED_FILL_LIGHT
            {
                COMPONENT_SELECTED_FILL_DARK
            } else {
                self.component_selected_fill
            };
            tok.to_color32()
        } else {
            let tok = if self.is_dark() && self.component_fill == COMPONENT_FILL_LIGHT {
                COMPONENT_FILL_DARK
            } else {
                self.component_fill
            };
            tok.to_color32()
        }
    }

    #[must_use]
    pub fn master_compat_ray_color32(&self) -> egui::Color32 {
        let tok = if self.is_dark() && self.master_compat_ray_color == MASTER_COMPAT_RAY_COLOR_LIGHT
        {
            MASTER_COMPAT_RAY_COLOR_DARK
        } else {
            self.master_compat_ray_color
        };
        tok.to_color32()
    }

    #[must_use]
    pub fn master_incompat_highlight_color32(&self) -> egui::Color32 {
        let tok = if self.is_dark()
            && self.master_incompat_highlight == MASTER_INCOMPAT_HIGHLIGHT_LIGHT
        {
            MASTER_INCOMPAT_HIGHLIGHT_DARK
        } else {
            self.master_incompat_highlight
        };
        tok.to_color32()
    }
}
