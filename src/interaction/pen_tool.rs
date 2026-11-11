//! Pen tool state machine (P key).

use crate::core::coordinates::FontPoint;

#[derive(Debug, Clone, PartialEq, Default)]
pub enum PenToolState {
    #[default]
    Idle,
    PlacingAnchor {
        anchor: FontPoint,
    },
    DraggingHandles {
        anchor: FontPoint,
        bcp_out: FontPoint,
        bcp_in: FontPoint,
        is_closing: bool,
    },
}

impl PenToolState {
    /// Computes symmetrical handles extruded from `anchor` by `delta`.
    #[must_use]
    pub fn compute_symmetric_handles(anchor: FontPoint, delta: (f64, f64), is_closing: bool) -> Self {
        let bcp_out = FontPoint::new(anchor.x.0 + delta.0, anchor.y.0 + delta.1);
        let bcp_in = FontPoint::new(anchor.x.0 - delta.0, anchor.y.0 - delta.1);
        Self::DraggingHandles {
            anchor,
            bcp_out,
            bcp_in,
            is_closing,
        }
    }
}
