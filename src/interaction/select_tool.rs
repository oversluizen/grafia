//! Select tool state machine (V key).

use crate::core::coordinates::{FontPoint, ScreenPoint};
use crate::core::curve_manipulation::ContourSegment;
use crate::core::NodeId;
use crate::interaction::gestures::Modifiers;
use kurbo::Rect;

#[derive(Debug, Clone, PartialEq, Default)]
pub enum SelectToolState {
    #[default]
    Idle,
    DraggingNodes {
        selected: Vec<NodeId>,
        start_pt: FontPoint,
        current_pt: FontPoint,
        modifiers: Modifiers,
    },
    DraggingSegment {
        contour_idx: usize,
        segment: ContourSegment,
        t: f64,
        start_pt: FontPoint,
        initial_h1: FontPoint,
        initial_h2: FontPoint,
    },
    PanningViewport {
        last_screen: ScreenPoint,
    },
    MarqueeSelecting {
        start_screen: ScreenPoint,
        current_screen: ScreenPoint,
    },
}

impl SelectToolState {
    #[must_use]
    pub fn marquee_rect(&self) -> Option<Rect> {
        if let Self::MarqueeSelecting {
            start_screen,
            current_screen,
        } = self
        {
            let min_x = (start_screen.x.0).min(current_screen.x.0) as f64;
            let max_x = (start_screen.x.0).max(current_screen.x.0) as f64;
            let min_y = (start_screen.y.0).min(current_screen.y.0) as f64;
            let max_y = (start_screen.y.0).max(current_screen.y.0) as f64;
            Some(Rect::new(min_x, min_y, max_x, max_y))
        } else {
            None
        }
    }
}
