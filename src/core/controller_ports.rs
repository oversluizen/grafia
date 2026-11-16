//! `SPEC-015`: 100% System-Agnostic Controller, Semantic Intent, Rotary Encoder,
//! OLED/HUD Telemetry, Haptic Feedback, and Mobile/Stylus Precision Port data contracts.

use crate::adapters::CanvasTool;
use crate::core::coordinates::{FontPoint, ScreenPoint};
use crate::core::math_consts::BRUSH_DEFAULT_RADIUS_DP;
use crate::core::reference_frame::SnapTargetKind;
use serde::{Deserialize, Serialize};
use std::ops::{BitOr, BitOrAssign};

/// Semantic geometric modifier intents decoupled from physical keyboard scancodes (`SPEC-015 §2`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum CanvasIntent {
    ConstrainAngle,
    LockAspectRatio,
    OriginFromCenter,
    BreakCuspHandle,
    SlideAlongCurve,
    DuplicateOnDrag,
    ShearTransform,
    KnifeAddNodesOnly,
    MoveOriginOrPan,
}

impl CanvasIntent {
    pub const ALL: [Self; 9] = [
        Self::ConstrainAngle,
        Self::LockAspectRatio,
        Self::OriginFromCenter,
        Self::BreakCuspHandle,
        Self::SlideAlongCurve,
        Self::DuplicateOnDrag,
        Self::ShearTransform,
        Self::KnifeAddNodesOnly,
        Self::MoveOriginOrPan,
    ];

    #[must_use]
    pub const fn bit(self) -> u16 {
        match self {
            Self::ConstrainAngle => 1 << 0,
            Self::LockAspectRatio => 1 << 1,
            Self::OriginFromCenter => 1 << 2,
            Self::BreakCuspHandle => 1 << 3,
            Self::SlideAlongCurve => 1 << 4,
            Self::DuplicateOnDrag => 1 << 5,
            Self::ShearTransform => 1 << 6,
            Self::KnifeAddNodesOnly => 1 << 7,
            Self::MoveOriginOrPan => 1 << 8,
        }
    }
}

/// Bitmask set of active [`CanvasIntent`] flags (`SPEC-015 §2`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct CanvasIntentMask(pub u16);

impl CanvasIntentMask {
    pub const NONE: Self = Self(0);

    #[must_use]
    pub const fn empty() -> Self {
        Self(0)
    }

    #[must_use]
    pub const fn from_intent(intent: CanvasIntent) -> Self {
        Self(intent.bit())
    }

    #[must_use]
    pub const fn with(self, intent: CanvasIntent) -> Self {
        Self(self.0 | intent.bit())
    }

    #[must_use]
    pub const fn contains(self, intent: CanvasIntent) -> bool {
        (self.0 & intent.bit()) != 0
    }

    #[must_use]
    pub const fn is_empty(self) -> bool {
        self.0 == 0
    }

    pub fn insert(&mut self, intent: CanvasIntent) {
        self.0 |= intent.bit();
    }

    pub fn remove(&mut self, intent: CanvasIntent) {
        self.0 &= !intent.bit();
    }

    #[must_use]
    pub const fn union(self, other: Self) -> Self {
        Self(self.0 | other.0)
    }
}

impl From<CanvasIntent> for CanvasIntentMask {
    fn from(intent: CanvasIntent) -> Self {
        Self::from_intent(intent)
    }
}

impl BitOr for CanvasIntentMask {
    type Output = Self;
    fn bitor(self, rhs: Self) -> Self::Output {
        Self(self.0 | rhs.0)
    }
}

impl BitOr<CanvasIntent> for CanvasIntentMask {
    type Output = Self;
    fn bitor(self, rhs: CanvasIntent) -> Self::Output {
        self.with(rhs)
    }
}

impl BitOrAssign for CanvasIntentMask {
    fn bitor_assign(&mut self, rhs: Self) {
        self.0 |= rhs.0;
    }
}

impl BitOrAssign<CanvasIntent> for CanvasIntentMask {
    fn bitor_assign(&mut self, rhs: CanvasIntent) {
        self.insert(rhs);
    }
}

/// Universal rotary encoder channels for hardware dials (Loupedeck CT, Wacom Touch Ring) and virtual wheels (`SPEC-015 §3`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum CanvasRotaryChannel {
    ContextPrimary,
    ViewportZoom,
    CanvasRotationOrSlant,
    HistoryScrub,
    CurveTensionOrRadius,
    KerningOrSpacing,
}

impl CanvasRotaryChannel {
    pub const ALL: [Self; 6] = [
        Self::ContextPrimary,
        Self::ViewportZoom,
        Self::CanvasRotationOrSlant,
        Self::HistoryScrub,
        Self::CurveTensionOrRadius,
        Self::KerningOrSpacing,
    ];
}

/// Single semantic intent entry in [`ControllerTelemetryDescriptor`] (`SPEC-015 §4`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IntentTelemetryItem {
    pub intent: CanvasIntent,
    pub short_label: &'static str,
    pub shortcut_hint: &'static str,
    pub active: bool,
}

/// Live rotary parameter readout for OLED/LCD dial screens and radial HUDs (`SPEC-015 §4`).
#[derive(Debug, Clone, PartialEq)]
pub struct RotaryTelemetryInfo {
    pub parameter_name: &'static str,
    pub formatted_value: String,
    pub normalized_ratio: f64,
}

/// Complete telemetry snapshot exported by `GrafiaCanvasState::controller_telemetry()` (`SPEC-015 §4`).
#[derive(Debug, Clone, PartialEq)]
pub struct ControllerTelemetryDescriptor {
    pub active_tool: CanvasTool,
    pub available_intents: Vec<IntentTelemetryItem>,
    pub primary_rotary: RotaryTelemetryInfo,
}

/// Semantic haptic feedback events emitted by `GrafiaCanvasState` (`SPEC-015 §5`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum CanvasHapticEvent {
    RotaryDetentTick { channel: CanvasRotaryChannel },
    IntentLatchToggled { intent: CanvasIntent, latched: bool },
    SnapAlignment { kind: SnapTargetKind },
    ContourClosed,
    BoundaryClamped,
}

/// Physical input device classification for adaptive touch offsets and hit radii (`SPEC-015 §6`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum InputDeviceKind {
    #[default]
    Mouse,
    Stylus,
    TouchFinger,
}

/// Stylus hardware events (Apple Pencil Pro double-tap / squeeze, Wacom barrel button, `SPEC-015 §6`).
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub enum StylusAccessoryEvent {
    DoubleTap,
    Squeeze { pos: ScreenPoint },
    BarrelHold(bool),
}

/// Computed screen-space geometry for the edge-flipping magnification loupe (`SPEC-015 §6`).
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct PrecisionLoupeGeometry {
    pub target_screen_pos: ScreenPoint,
    pub target_upm_pos: FontPoint,
    pub lens_center_screen_pos: ScreenPoint,
    pub lens_radius_dp: f32,
    pub zoom_factor: f32,
    pub flipped_from_top_edge: bool,
}

/// Internal controller port session state (`SPEC-015`).
#[derive(Debug, Clone, PartialEq)]
pub struct ControllerPortState {
    pub external_intents: CanvasIntentMask,
    pub latched_intents: CanvasIntentMask,
    pub active_input_device: InputDeviceKind,
    pub last_drawing_tool: CanvasTool,
    pub barrel_hold_active: bool,
    pub brush_radius_dp: f32,
    pub pending_haptic_events: Vec<CanvasHapticEvent>,
}

impl ControllerPortState {
    pub fn push_haptic(&mut self, event: CanvasHapticEvent) {
        self.pending_haptic_events.push(event);
    }
}

impl Default for ControllerPortState {
    fn default() -> Self {
        Self {
            external_intents: CanvasIntentMask::NONE,
            latched_intents: CanvasIntentMask::NONE,
            active_input_device: InputDeviceKind::Mouse,
            last_drawing_tool: CanvasTool::Pen,
            barrel_hold_active: false,
            brush_radius_dp: BRUSH_DEFAULT_RADIUS_DP,
            pending_haptic_events: Vec::new(),
        }
    }
}
