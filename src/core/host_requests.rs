//! Host application request contracts, auto-trace settings, snap masks, and touch interaction state.

use crate::core::canvas_entities::ExtraNodeKey;
use crate::core::color_layers::ColorSublayerMutation;
use crate::core::hinting_and_hoi::{PostScriptHintMutation, TrueTypeInstructionMutation};
use crate::core::math_consts::{
    AUTO_TRACE_CORNER_ANGLE_DEFAULT_DEG, AUTO_TRACE_DEFAULT_LUMINANCE,
    AUTO_TRACE_FIT_ERROR_DEFAULT_UPM, AUTO_TRACE_MIN_SPECKLE_AREA_DEFAULT_PX2,
    AUTO_TRACE_RDP_TOLERANCE_DEFAULT_PX,
};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

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

/// Outbound Host-Port event emitted by `grafia` to the Host Application (`SPEC-006`..`SPEC-009`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum CanvasHostRequest {
    OpenComponentSource {
        id: ComponentId,
        component_name: String,
    },
    ResetDefaultAnchors {
        glyph_name: String,
    },
    EditTextStream(TextStreamEditRequest),
    UpdateSmartComponentAxis {
        component_id: ComponentId,
        axis_id: String,
        new_value: f64,
    },
    SelectOrMutateExtraNode {
        key: ExtraNodeKey,
    },
    MutateColorSublayer {
        sublayer_index: usize,
        mutation: ColorSublayerMutation,
    },
    MutatePostScriptHint(PostScriptHintMutation),
    MutateTrueTypeInstruction(TrueTypeInstructionMutation),
    MutateHoiLayerNode {
        layer_index: usize,
        contour_index: usize,
        node_id: crate::core::NodeId,
        new_pos: crate::core::FontPoint,
    },
    PropagateStemDelta {
        orientation: crate::core::StemOrientation,
        old_width_upm: f64,
        new_width_upm: f64,
        angle_delta_deg: f64,
    },
}

/// Active multi-touch / stylus tracking state for `GrafiaPreferences` touch ergonomics (`SPEC-009 §2.6.3`).
#[derive(Debug, Clone, Default)]
pub struct TouchInteractionState {
    pub contacts: BTreeMap<u64, (egui::Pos2, egui::Pos2)>,
    pub two_finger_peak: bool,
    pub max_contact_excursion_dp: f32,
    pub active_touch_raw_pos: Option<egui::Pos2>,
    pub egui_any_touches_in_frame: bool,
    pub touch_shift_latched: bool,
    pub touch_alt_latched: bool,
    pub touch_cmd_latched: bool,
    pub touch_space_pan_latched: bool,
}

/// Binarization mode for bitmap & shadowed-camera auto-tracing (`SPEC-009 §2.5.1`).
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub enum AutoTraceThresholdMode {
    GlobalLuminance(u8),
    AdaptiveLocal { window_radius_px: u16, bias_k: f32 },
}

impl Default for AutoTraceThresholdMode {
    fn default() -> Self {
        Self::GlobalLuminance(AUTO_TRACE_DEFAULT_LUMINANCE)
    }
}

/// Configuration for `trace_background_image` and `trace_active_background_image` (`SPEC-009 §2.5.1`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AutoTraceConfig {
    pub threshold_mode: AutoTraceThresholdMode,
    pub perspective_quad_px: Option<[(f64, f64); 4]>,
    pub invert_polarity: bool,
    pub min_speckle_area_px2: f64,
    pub rdp_tolerance_px: f64,
    pub corner_angle_deg: f64,
    pub fit_error_upm: f64,
    pub snap_extrema: bool,
}

impl Default for AutoTraceConfig {
    fn default() -> Self {
        Self {
            threshold_mode: AutoTraceThresholdMode::default(),
            perspective_quad_px: None,
            invert_polarity: false,
            min_speckle_area_px2: AUTO_TRACE_MIN_SPECKLE_AREA_DEFAULT_PX2,
            rdp_tolerance_px: AUTO_TRACE_RDP_TOLERANCE_DEFAULT_PX,
            corner_angle_deg: AUTO_TRACE_CORNER_ANGLE_DEFAULT_DEG,
            fit_error_upm: AUTO_TRACE_FIT_ERROR_DEFAULT_UPM,
            snap_extrema: true,
        }
    }
}

/// Magnetic snap target category mask (`SPEC-009 §2.6.3`, `SPEC-010 §2.3`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct SnapTargetMask {
    pub guides: bool,
    pub metrics: bool,
    pub grid: bool,
    pub nodes: bool,
    pub background: bool,
    pub italic_angle: bool,
    #[serde(default = "default_snap_reference_frame")]
    pub reference_frame: bool,
}

const fn default_snap_reference_frame() -> bool {
    true
}

impl SnapTargetMask {
    #[must_use]
    pub const fn all() -> Self {
        Self {
            guides: true,
            metrics: true,
            grid: true,
            nodes: true,
            background: true,
            italic_angle: true,
            reference_frame: true,
        }
    }

    #[must_use]
    pub const fn none() -> Self {
        Self {
            guides: false,
            metrics: false,
            grid: false,
            nodes: false,
            background: false,
            italic_angle: false,
            reference_frame: false,
        }
    }
}

impl Default for SnapTargetMask {
    fn default() -> Self {
        Self::all()
    }
}
