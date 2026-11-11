//! Normalized canvas gestures from mouse, keyboard, or stylus hardware.

use crate::core::coordinates::{FontPoint, ScreenPoint};
use crate::core::NodeId;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct Modifiers {
    pub shift: bool,
    pub option: bool,
    pub command: bool,
    pub control: bool,
    pub space: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum CanvasGesture {
    ClickNode {
        id: NodeId,
        modifiers: Modifiers,
    },
    ClickEmptySpace {
        at: FontPoint,
        modifiers: Modifiers,
    },
    DragStart {
        origin: ScreenPoint,
        modifiers: Modifiers,
    },
    DragMove {
        current: ScreenPoint,
        delta: (f32, f32),
        modifiers: Modifiers,
    },
    DragEnd,
    Escape,
}
