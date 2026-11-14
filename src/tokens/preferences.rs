//! `SPEC-009` (`grafia v0.8.0` Part B) typed contracts:
//! `AutoTraceThresholdMode`, `AutoTraceConfig`, `OffCurveHandleShape`, `SnapTargetMask`,
//! and `GrafiaPreferences` (all 24 normative host fields).

pub use crate::core::host_requests::{
    AutoTraceConfig, AutoTraceThresholdMode, SnapTargetMask, TouchInteractionState,
};
pub use crate::tokens::keymap::{CanvasActionId, GrafiaKeymap, KeyChord};

use crate::tokens::trace_and_band_tokens::{
    PREF_DEFAULT_FREEHAND_SMOOTHING_PX, PREF_DEFAULT_GRID_STEP_UPM, PREF_DEFAULT_GRID_SUBDIVISIONS,
    PREF_DEFAULT_HIT_TEST_RADIUS_DP, PREF_DEFAULT_KERNING_HIGH_UPM, PREF_DEFAULT_KERNING_LOW_UPM,
    PREF_DEFAULT_NUDGE_LARGE_UPM, PREF_DEFAULT_NUDGE_MEDIUM_UPM, PREF_DEFAULT_NUDGE_SMALL_UPM,
    PREF_DEFAULT_PASSIVE_NEIGHBOR_OPACITY, PREF_DEFAULT_SNAP_RADIUS_DP,
    PREF_DEFAULT_SPACING_HIGH_UPM, PREF_DEFAULT_SPACING_LOW_UPM,
    PREF_DEFAULT_TEXT_NUMBERS_THRESHOLD, PREF_DEFAULT_TOUCH_OFFSET_DP,
};
use serde::{Deserialize, Serialize};

/// Off-curve Bézier control-point handle shape (`SPEC-009 §2.6.2`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum OffCurveHandleShape {
    #[default]
    Circle,
    Square,
    Cross,
}

const fn default_segment_hit_tol_dp() -> f32 {
    crate::tokens::SEGMENT_HIT_TOLERANCE_DP
}
const fn default_kerning_low_upm() -> f64 {
    PREF_DEFAULT_KERNING_LOW_UPM
}
const fn default_kerning_high_upm() -> f64 {
    PREF_DEFAULT_KERNING_HIGH_UPM
}
const fn default_spacing_low_upm() -> f64 {
    PREF_DEFAULT_SPACING_LOW_UPM
}
const fn default_spacing_high_upm() -> f64 {
    PREF_DEFAULT_SPACING_HIGH_UPM
}

/// Unified typed host configuration applied via `state.apply_preferences(&GrafiaPreferences)` (`SPEC-009 §2.6.3`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct GrafiaPreferences {
    pub nudge_step_small_upm: f64,
    pub nudge_step_medium_upm: f64,
    pub nudge_step_large_upm: f64,
    pub spacing_step_low_upm: f64,
    pub spacing_step_high_upm: f64,
    pub kerning_step_low_upm: f64,
    pub kerning_step_high_upm: f64,
    #[serde(default = "default_spacing_low_upm")]
    pub spacing_increment_low_upm: f64,
    #[serde(default = "default_spacing_high_upm")]
    pub spacing_increment_high_upm: f64,
    #[serde(default = "default_kerning_low_upm")]
    pub kerning_increment_low_upm: f64,
    #[serde(default = "default_kerning_high_upm")]
    pub kerning_increment_high_upm: f64,
    pub grid_step_upm: f64,
    pub grid_subdivisions: u32,
    pub round_to_grid: bool,
    pub snap_radius_dp: f32,
    pub snap_targets: SnapTargetMask,
    pub hit_test_radius_dp: f32,
    #[serde(default = "default_segment_hit_tol_dp")]
    pub segment_hit_tolerance_dp: f32,
    pub text_mode_numbers_threshold: f64,
    pub scroll_wheel_zooms: bool,
    pub invert_zoom_direction: bool,
    pub fill_active_glyph_while_editing: bool,
    pub passive_neighbor_opacity: f32,
    pub italic_angle_compensates_bbox_and_nudge: bool,
    pub freehand_smoothing_tolerance_px: f64,
    pub touch_finger_offset_dp: f32,
    pub show_touch_precision_loupe: bool,
    pub two_finger_pan_zoom_enabled: bool,
    pub two_finger_tap_undo_enabled: bool,
}

impl Default for GrafiaPreferences {
    fn default() -> Self {
        Self {
            nudge_step_small_upm: PREF_DEFAULT_NUDGE_SMALL_UPM,
            nudge_step_medium_upm: PREF_DEFAULT_NUDGE_MEDIUM_UPM,
            nudge_step_large_upm: PREF_DEFAULT_NUDGE_LARGE_UPM,
            spacing_step_low_upm: PREF_DEFAULT_SPACING_LOW_UPM,
            spacing_step_high_upm: PREF_DEFAULT_SPACING_HIGH_UPM,
            kerning_step_low_upm: PREF_DEFAULT_KERNING_LOW_UPM,
            kerning_step_high_upm: PREF_DEFAULT_KERNING_HIGH_UPM,
            spacing_increment_low_upm: PREF_DEFAULT_SPACING_LOW_UPM,
            spacing_increment_high_upm: PREF_DEFAULT_SPACING_HIGH_UPM,
            kerning_increment_low_upm: PREF_DEFAULT_KERNING_LOW_UPM,
            kerning_increment_high_upm: PREF_DEFAULT_KERNING_HIGH_UPM,
            grid_step_upm: PREF_DEFAULT_GRID_STEP_UPM,
            grid_subdivisions: PREF_DEFAULT_GRID_SUBDIVISIONS,
            round_to_grid: true,
            snap_radius_dp: PREF_DEFAULT_SNAP_RADIUS_DP,
            snap_targets: SnapTargetMask::all(),
            hit_test_radius_dp: PREF_DEFAULT_HIT_TEST_RADIUS_DP,
            segment_hit_tolerance_dp: crate::tokens::SEGMENT_HIT_TOLERANCE_DP,
            text_mode_numbers_threshold: PREF_DEFAULT_TEXT_NUMBERS_THRESHOLD,
            scroll_wheel_zooms: true,
            invert_zoom_direction: false,
            fill_active_glyph_while_editing: false,
            passive_neighbor_opacity: PREF_DEFAULT_PASSIVE_NEIGHBOR_OPACITY,
            italic_angle_compensates_bbox_and_nudge: true,
            freehand_smoothing_tolerance_px: PREF_DEFAULT_FREEHAND_SMOOTHING_PX,
            touch_finger_offset_dp: PREF_DEFAULT_TOUCH_OFFSET_DP,
            show_touch_precision_loupe: false,
            two_finger_pan_zoom_enabled: true,
            two_finger_tap_undo_enabled: true,
        }
    }
}
