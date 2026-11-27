//! `SPEC-009 §2.6.4`: Typed `CanvasActionId`, `KeyChord`, `GrafiaKeymap`, and `TouchInteractionState`.

use crate::adapters::CanvasTool;
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::collections::BTreeMap;

/// Rebindable canvas action identifier (`SPEC-009 §2.6.4`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum CanvasActionId {
    SelectTool,
    PenTool,
    RotateTool,
    ScaleTool,
    LassoTool,
    ZoomTool,
    KnifeTool,
    PrimitivesTool,
    CyclePrimitivesSubTool,
    MeasurementTool,
    AnnotationTool,
    CycleAnnotationSubTool,
    PencilTool,
    PixelTool,
    TextTool,
    Undo,
    Redo,
    ZoomIn,
    ZoomOut,
    ZoomToFit,
    SelectAll,
    DeselectAll,
    ReverseContours,
    MakeNodeFirst,
    RemoveOverlap,
    ToggleBoundingBox,
    SaveViewportBookmark,
    ToggleViewportBookmark,
    SwitchActiveBackgroundLayer,
    SwapForegroundWithBackground,
    TraceBackgroundImage,
}

impl CanvasActionId {
    #[must_use]
    pub const fn from_tool(tool: CanvasTool) -> Self {
        match tool {
            CanvasTool::Select => Self::SelectTool,
            CanvasTool::Pen => Self::PenTool,
            CanvasTool::Rotate => Self::RotateTool,
            CanvasTool::Scale => Self::ScaleTool,
            CanvasTool::Lasso => Self::LassoTool,
            CanvasTool::Zoom => Self::ZoomTool,
            CanvasTool::Knife => Self::KnifeTool,
            CanvasTool::Primitives => Self::PrimitivesTool,
            CanvasTool::Measurement => Self::MeasurementTool,
            CanvasTool::Annotation => Self::AnnotationTool,
            CanvasTool::Pencil => Self::PencilTool,
            CanvasTool::Pixel => Self::PixelTool,
            CanvasTool::Text => Self::TextTool,
            CanvasTool::TrueTypeInstructor
            | CanvasTool::Hoi
            | CanvasTool::Stem
            | CanvasTool::CurvedTrajectory
            | CanvasTool::Trowel
            | CanvasTool::Cleaner
            | CanvasTool::ShapeBuilder => Self::SelectTool,
        }
    }

    #[must_use]
    pub const fn to_tool(self) -> Option<CanvasTool> {
        match self {
            Self::SelectTool => Some(CanvasTool::Select),
            Self::PenTool => Some(CanvasTool::Pen),
            Self::RotateTool => Some(CanvasTool::Rotate),
            Self::ScaleTool => Some(CanvasTool::Scale),
            Self::LassoTool => Some(CanvasTool::Lasso),
            Self::ZoomTool => Some(CanvasTool::Zoom),
            Self::KnifeTool => Some(CanvasTool::Knife),
            Self::PrimitivesTool => Some(CanvasTool::Primitives),
            Self::MeasurementTool => Some(CanvasTool::Measurement),
            Self::AnnotationTool => Some(CanvasTool::Annotation),
            Self::PencilTool => Some(CanvasTool::Pencil),
            Self::PixelTool => Some(CanvasTool::Pixel),
            Self::TextTool => Some(CanvasTool::Text),
            _ => None,
        }
    }
}

/// Keyboard chord `{ key, cmd, ctrl, alt, shift }` (`SPEC-009 §2.6.4`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct KeyChord {
    pub key: egui::Key,
    pub cmd: bool,
    pub ctrl: bool,
    pub alt: bool,
    pub shift: bool,
}

impl KeyChord {
    #[must_use]
    pub const fn new(key: egui::Key, cmd: bool, ctrl: bool, alt: bool, shift: bool) -> Self {
        Self {
            key,
            cmd,
            ctrl,
            alt,
            shift,
        }
    }

    #[must_use]
    pub const fn plain(key: egui::Key) -> Self {
        Self::new(key, false, false, false, false)
    }
}

#[derive(Serialize, Deserialize)]
struct KeyChordWire {
    key: String,
    cmd: bool,
    ctrl: bool,
    alt: bool,
    shift: bool,
}

impl Serialize for KeyChord {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let wire = KeyChordWire {
            key: self.key.name().to_owned(),
            cmd: self.cmd,
            ctrl: self.ctrl,
            alt: self.alt,
            shift: self.shift,
        };
        wire.serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for KeyChord {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let wire = KeyChordWire::deserialize(deserializer)?;
        let key = egui::Key::from_name(&wire.key)
            .ok_or_else(|| serde::de::Error::custom("unknown egui::Key name"))?;
        Ok(Self {
            key,
            cmd: wire.cmd,
            ctrl: wire.ctrl,
            alt: wire.alt,
            shift: wire.shift,
        })
    }
}

/// Serializable, conflict-free keyboard shortcut map (`SPEC-009 §2.6.4`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GrafiaKeymap {
    pub bindings: BTreeMap<CanvasActionId, KeyChord>,
}

impl Default for GrafiaKeymap {
    fn default() -> Self {
        let mut map = Self {
            bindings: BTreeMap::new(),
        };
        for (act, key) in [
            (CanvasActionId::SelectTool, egui::Key::V),
            (CanvasActionId::PenTool, egui::Key::P),
            (CanvasActionId::RotateTool, egui::Key::R),
            (CanvasActionId::ScaleTool, egui::Key::S),
            (CanvasActionId::ZoomTool, egui::Key::Z),
            (CanvasActionId::KnifeTool, egui::Key::E),
            (CanvasActionId::PrimitivesTool, egui::Key::F),
            (CanvasActionId::MeasurementTool, egui::Key::L),
            (CanvasActionId::AnnotationTool, egui::Key::A),
            (CanvasActionId::PencilTool, egui::Key::B),
            (CanvasActionId::PixelTool, egui::Key::X),
            (CanvasActionId::TextTool, egui::Key::T),
        ] {
            map.set_binding(act, KeyChord::plain(key));
        }
        map.register_default_chord_bindings();
        map
    }
}

impl GrafiaKeymap {
    fn register_default_chord_bindings(&mut self) {
        use egui::Key;
        use CanvasActionId as Act;
        for (act, key, cmd, alt, shift) in [
            (Act::LassoTool, Key::V, false, false, true),
            (Act::CyclePrimitivesSubTool, Key::F, false, false, true),
            (Act::CycleAnnotationSubTool, Key::A, false, false, true),
            (Act::Undo, Key::Z, true, false, false),
            (Act::Redo, Key::Z, true, false, true),
            (Act::ZoomIn, Key::Equals, true, false, false),
            (Act::ZoomOut, Key::Minus, true, false, false),
            (Act::ZoomToFit, Key::Num0, true, false, false),
            (Act::SelectAll, Key::A, true, false, false),
            (Act::DeselectAll, Key::D, true, false, false),
            (Act::ReverseContours, Key::R, true, false, true),
            (Act::MakeNodeFirst, Key::F, true, false, true),
            (Act::RemoveOverlap, Key::O, true, false, true),
            (Act::ToggleBoundingBox, Key::B, true, true, true),
        ] {
            self.set_binding(act, KeyChord::new(key, cmd, false, alt, shift));
        }
    }

    pub fn set_binding(&mut self, action: CanvasActionId, chord: KeyChord) {
        self.bindings.retain(|_, existing| *existing != chord);
        self.bindings.insert(action, chord);
    }

    pub fn bind_tool(&mut self, tool: CanvasTool, chord: KeyChord) {
        self.set_binding(CanvasActionId::from_tool(tool), chord);
    }

    pub fn remove_binding(&mut self, action: CanvasActionId) -> Option<KeyChord> {
        self.bindings.remove(&action)
    }

    #[must_use]
    pub fn chord_for(&self, action: CanvasActionId) -> Option<KeyChord> {
        self.bindings.get(&action).copied()
    }

    #[must_use]
    pub fn action_for_chord(&self, chord: KeyChord) -> Option<CanvasActionId> {
        self.bindings
            .iter()
            .find_map(|(act, bound)| (*bound == chord).then_some(*act))
    }
}

pub use crate::core::host_requests::TouchInteractionState;
