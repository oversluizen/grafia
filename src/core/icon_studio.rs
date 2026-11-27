//! Cohesive Vector Icon Studio, Variable-Width Stroke, `vitrea` Liquid-Glass & 10-Mode Kinetic Symbol Scene (`SPEC-016 §2`).

use crate::core::coordinates::FontPoint;
use crate::core::fit_and_stroke::LineCapStyle;
use crate::core::glyph_model::{ContourId, GlyphContour};
use crate::core::icon_color_modes::{
    DrawDirection, SymbolColorRenderingMode, SymbolHierarchyLevel,
};
use crate::core::icon_knockout_mask::KineticPlaybackMode;
use crate::core::math_consts::{
    KINETIC_MAX_SPEED_MULTIPLIER, KINETIC_MIN_SPEED_MULTIPLIER, LIQUID_GLASS_DEFAULT_DISPERSION,
    LIQUID_GLASS_DEFAULT_DOME_RADIUS_UPM, LIQUID_GLASS_DEFAULT_IOR,
    LIQUID_GLASS_DEFAULT_REFRACTION_PX, LIQUID_GLASS_DEFAULT_SMOOTH_K_UPM,
    LIQUID_GLASS_DEFAULT_SPECULAR, LIQUID_GLASS_DEFAULT_TINT_RGBA, LIQUID_GLASS_MAX_DISPERSION,
    LIQUID_GLASS_MAX_DOME_RADIUS_UPM, LIQUID_GLASS_MAX_IOR, LIQUID_GLASS_MAX_SMOOTH_K_UPM,
    LIQUID_GLASS_MAX_SPECULAR, LIQUID_GLASS_MIN_DOME_RADIUS_UPM, LIQUID_GLASS_MIN_IOR,
    LIQUID_GLASS_MIN_SMOOTH_K_UPM, RADIAL_SYMMETRY_MIN_FOLDS,
};
use crate::core::selection_info::AnchorId;
use serde::{Deserialize, Serialize};

/// Layer phase propagation scope for kinetic symbol animations (`SPEC-016 §4.2`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum KineticLayerScope {
    #[default]
    ByLayer,
    WholeSymbol,
}

/// Direction polarity for `Bounce`, `Appear`, and `Disappear` kinetic symbol effects (`SPEC-016 §4.2`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum BounceDirection {
    #[default]
    Up,
    Down,
}

/// Direction polarity for `Scale` kinetic symbol effects (`SPEC-016 §4.2`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum ScaleDirection {
    #[default]
    Up,
    Down,
}

/// Oscillation axis or rotational polarity for `Wiggle` kinetic symbol effects (`SPEC-016 §4.2`).
#[derive(Debug, Clone, Copy, PartialEq, Default, Serialize, Deserialize)]
pub enum WiggleAxis {
    #[default]
    Clockwise,
    CounterClockwise,
    Left,
    Right,
    Up,
    Down,
    Forward,
    Backward,
    CustomAngle(f64),
}

/// Fallback transition mode for unmatched sublayers during `MagicReplace` / `Replace` (`SPEC-016 §4.2`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum ReplaceFallback {
    #[default]
    DownUp,
    UpUp,
    OffUp,
}

/// All 10 layered vector symbol animation effects and their sub-modes (`SPEC-016 §4.2`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum KineticSymbolEffect {
    Pulse {
        scope: KineticLayerScope,
    },
    Bounce {
        direction: BounceDirection,
        scope: KineticLayerScope,
    },
    VariableColor {
        cumulative: bool,
        reversing: bool,
        dim_inactive: bool,
    },
    Scale {
        direction: ScaleDirection,
        scope: KineticLayerScope,
    },
    Appear {
        direction: BounceDirection,
        draw_on: bool,
        scope: KineticLayerScope,
    },
    Disappear {
        direction: BounceDirection,
        draw_off: bool,
        scope: KineticLayerScope,
    },
    Wiggle {
        axis: WiggleAxis,
        scope: KineticLayerScope,
    },
    Rotate {
        clockwise: bool,
        scope: KineticLayerScope,
    },
    Breathe {
        pulse_opacity: bool,
        scope: KineticLayerScope,
    },
    MagicReplace {
        fallback: ReplaceFallback,
        scope: KineticLayerScope,
        target_contours: Vec<GlyphContour>,
    },
}

impl Default for KineticSymbolEffect {
    fn default() -> Self {
        Self::Bounce {
            direction: BounceDirection::Up,
            scope: KineticLayerScope::ByLayer,
        }
    }
}

/// Pure phase-driven timeline state for 60/120 fps kinetic icon evaluation (`SPEC-016 §2.2`, `SPEC-017 §2.6`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct KineticTimelineState {
    pub effect: KineticSymbolEffect,
    pub phase_t: f64,
    pub speed_multiplier: f64,
    pub playing: bool,
    pub looping: bool,
    #[serde(default)]
    pub playback_mode: KineticPlaybackMode,
}

impl Default for KineticTimelineState {
    fn default() -> Self {
        Self {
            effect: KineticSymbolEffect::default(),
            phase_t: 0.0,
            speed_multiplier: 1.0,
            playing: false,
            looping: true,
            playback_mode: KineticPlaybackMode::Continuous,
        }
    }
}

impl KineticTimelineState {
    /// Returns the effective normalized evaluation phase $t_{\text{eff}} \in [0.0, 1.0]$ (`SPEC-017 §2.6`).
    #[must_use]
    pub fn effective_phase_t(&self) -> f64 {
        self.playback_mode.effective_phase(self.phase_t)
    }

    /// Returns `true` when kinetic evaluation is active (playing, non-zero `phase_t`, or `VariableValue`).
    #[must_use]
    pub fn is_active(&self) -> bool {
        self.playing || self.phase_t > 0.0 || self.playback_mode.is_always_active()
    }

    /// Advances `phase_t` by `dt_sec * speed_multiplier` when `playing == true` (`SPEC-016 §2.2`, `SPEC-017 §2.6`).
    pub fn step_timeline(&mut self, dt_sec: f64) -> bool {
        if !self.playing || !dt_sec.is_finite() || dt_sec <= 0.0 {
            return false;
        }
        let speed = self
            .speed_multiplier
            .clamp(KINETIC_MIN_SPEED_MULTIPLIER, KINETIC_MAX_SPEED_MULTIPLIER);
        let next = self.phase_t + dt_sec * speed;
        if !next.is_finite() {
            return false;
        }
        if self.looping && self.playback_mode != KineticPlaybackMode::OneShot {
            let rem = next.rem_euclid(1.0);
            self.phase_t = if rem.is_finite() {
                rem.clamp(0.0, 1.0)
            } else {
                0.0
            };
        } else {
            self.phase_t = next.clamp(0.0, 1.0);
            self.playing = next < 1.0;
        }
        true
    }
}

/// Physical 3D Liquid-Glass optical configuration backed by `vitrea::LiquidGlassOpticalParams` (`SPEC-016 §5.1`).
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct LiquidGlassRenderConfig {
    pub dome_radius_upm: f64,
    pub smooth_union_k_upm: f64,
    pub ior_base: f32,
    pub chromatic_dispersion: f32,
    pub specular_intensity: f32,
    pub tint_rgba: [u8; 4],
}

impl Default for LiquidGlassRenderConfig {
    fn default() -> Self {
        Self {
            dome_radius_upm: LIQUID_GLASS_DEFAULT_DOME_RADIUS_UPM,
            smooth_union_k_upm: LIQUID_GLASS_DEFAULT_SMOOTH_K_UPM,
            ior_base: LIQUID_GLASS_DEFAULT_IOR,
            chromatic_dispersion: LIQUID_GLASS_DEFAULT_DISPERSION,
            specular_intensity: LIQUID_GLASS_DEFAULT_SPECULAR,
            tint_rgba: LIQUID_GLASS_DEFAULT_TINT_RGBA,
        }
    }
}

impl LiquidGlassRenderConfig {
    /// Converts this Liquid-Glass configuration into a `vitrea::LiquidGlassOpticalParams` value (`SPEC-016 §2.4`).
    #[must_use]
    pub fn to_vitrea_optics(&self) -> vitrea::LiquidGlassOpticalParams {
        let r0 = (self.ior_base - 1.0) / (self.ior_base + 1.0);
        vitrea::LiquidGlassOpticalParams {
            bevel_width_pt: (self.dome_radius_upm as f32).max(1.0),
            dome_thickness_pt: LIQUID_GLASS_DEFAULT_REFRACTION_PX,
            meniscus_merge_k_pt: (self.smooth_union_k_upm as f32).max(1.0),
            ior_glass: self.ior_base,
            ior_dispersion_rgb: [
                self.ior_base - self.chromatic_dispersion,
                self.ior_base,
                self.ior_base + self.chromatic_dispersion,
            ],
            fresnel_f0: r0 * r0 * self.specular_intensity,
            ..vitrea::LiquidGlassOpticalParams::default()
        }
    }

    pub(crate) fn is_valid(&self) -> bool {
        (LIQUID_GLASS_MIN_DOME_RADIUS_UPM..=LIQUID_GLASS_MAX_DOME_RADIUS_UPM)
            .contains(&self.dome_radius_upm)
            && (LIQUID_GLASS_MIN_SMOOTH_K_UPM..=LIQUID_GLASS_MAX_SMOOTH_K_UPM)
                .contains(&self.smooth_union_k_upm)
            && (LIQUID_GLASS_MIN_IOR..=LIQUID_GLASS_MAX_IOR).contains(&self.ior_base)
            && (0.0..=LIQUID_GLASS_MAX_DISPERSION).contains(&self.chromatic_dispersion)
            && (0.0..=LIQUID_GLASS_MAX_SPECULAR).contains(&self.specular_intensity)
    }

    /// Sanitizes an external/deserialized `LiquidGlassRenderConfig` into valid optical bounds (`SPEC-016 §5.1`).
    #[must_use]
    pub fn sanitized(mut self) -> Self {
        let d = Self::default();
        let f64_or = |v: f64, def: f64| if v.is_finite() { v } else { def };
        let f32_or = |v: f32, def: f32| if v.is_finite() { v } else { def };
        self.dome_radius_upm = f64_or(self.dome_radius_upm, d.dome_radius_upm).clamp(
            LIQUID_GLASS_MIN_DOME_RADIUS_UPM,
            LIQUID_GLASS_MAX_DOME_RADIUS_UPM,
        );
        self.smooth_union_k_upm = f64_or(self.smooth_union_k_upm, d.smooth_union_k_upm)
            .clamp(LIQUID_GLASS_MIN_SMOOTH_K_UPM, LIQUID_GLASS_MAX_SMOOTH_K_UPM);
        self.ior_base =
            f32_or(self.ior_base, d.ior_base).clamp(LIQUID_GLASS_MIN_IOR, LIQUID_GLASS_MAX_IOR);
        self.chromatic_dispersion = f32_or(self.chromatic_dispersion, d.chromatic_dispersion)
            .clamp(0.0, LIQUID_GLASS_MAX_DISPERSION);
        self.specular_intensity = f32_or(self.specular_intensity, d.specular_intensity)
            .clamp(0.0, LIQUID_GLASS_MAX_SPECULAR);
        self
    }
}

/// $N$-fold rotational and dihedral kaleidoscope symmetry configuration (`SPEC-016 §3.3`).
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct RadialSymmetrySpec {
    pub fold_count: u8,
    pub dihedral_mirror: bool,
    pub center: FontPoint,
}

impl Default for RadialSymmetrySpec {
    fn default() -> Self {
        Self {
            fold_count: RADIAL_SYMMETRY_MIN_FOLDS,
            dihedral_mirror: false,
            center: FontPoint::new(0.0, 0.0),
        }
    }
}

/// Asymmetric left/right width sample at normalized contour arc length `arc_ratio_t` in `[0.0, 1.0]` (`SPEC-016 §3.2`).
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct WidthNodeSample {
    pub arc_ratio_t: f64,
    pub width_left_upm: f64,
    pub width_right_upm: f64,
}

/// Non-destructive variable-width skeleton stroke profile attached to `contour_id` (`SPEC-016 §3.2`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct VariableWidthStrokeSpec {
    pub contour_id: ContourId,
    pub samples: Vec<WidthNodeSample>,
    pub cap_style: LineCapStyle,
}

/// Symbol motion group, pivot anchor, variable-color, hierarchy tier, draw-order, and knockout margin metadata for a sublayer (`SPEC-016 §4.1`, `SPEC-017 §2.3–§2.5`).
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct SymbolLayerTag {
    pub layer_group: u8,
    pub motion_anchor_id: Option<AnchorId>,
    pub custom_pivot_upm: Option<FontPoint>,
    pub draw_order_reversed: bool,
    pub variable_color_enabled: bool,
    pub pulse_participates: bool,
    #[serde(default)]
    pub hierarchy_level: SymbolHierarchyLevel,
    #[serde(default)]
    pub draw_direction: DrawDirection,
    #[serde(default)]
    pub knockout_margin_upm: f64,
    #[serde(default)]
    pub phase_offset: f64,
    #[serde(default)]
    pub motion_path_contour_id: Option<ContourId>,
    #[serde(default)]
    pub orient_along_motion_path: bool,
}

impl Default for SymbolLayerTag {
    fn default() -> Self {
        Self {
            layer_group: 1,
            motion_anchor_id: None,
            custom_pivot_upm: None,
            draw_order_reversed: false,
            variable_color_enabled: true,
            pulse_participates: true,
            hierarchy_level: SymbolHierarchyLevel::Primary,
            draw_direction: DrawDirection::StartToEnd,
            knockout_margin_upm: 0.0,
            phase_offset: 0.0,
            motion_path_contour_id: None,
            orient_along_motion_path: false,
        }
    }
}

/// Cohesive aggregate for all `SPEC-016` / `SPEC-017` Vector Icon Studio, Kinetic Symbol, and Liquid-Glass state (`SPEC-016 §2.1`, `SPEC-017 §2.3`).
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct CanvasIconStudioScene {
    pub kinetic: KineticTimelineState,
    pub liquid_glass: LiquidGlassRenderConfig,
    pub radial_symmetry: RadialSymmetrySpec,
    pub variable_width_strokes: Vec<VariableWidthStrokeSpec>,
    pub symbol_layer_tags: Vec<SymbolLayerTag>,
    #[serde(default)]
    pub color_mode: SymbolColorRenderingMode,
}
