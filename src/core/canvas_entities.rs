//! Domain entities for Hobby/Euler spline constraints, virtual extra nodes, smart handles, and background images.

use crate::core::bezier_ops::NodeId;
use crate::core::coordinates::FontPoint;
use crate::core::host_requests::ComponentId;
use serde::{Deserialize, Serialize};

/// Directional tangent constraint at an interpolating `HobbyStar` or `EulerSpiro` node (`SPEC-009 §2.1.1`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum HobbyDirConstraint {
    #[default]
    Free,
    Horizontal,
    Vertical,
    SlantedItalic,
    FollowMetricLine,
}

/// Canonical deterministic address for a non-destructive interior segment-segment crossing (`SPEC-009 §2.4.1`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct ExtraNodeKey {
    pub contour_a: usize,
    pub seg_a: usize,
    pub contour_b: usize,
    pub seg_b: usize,
    pub crossing_rank: usize,
}

/// Non-destructive virtual crossing marker produced by `compute_layer_extra_nodes` (`SPEC-009 §2.4.1`).
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct VirtualExtraNode {
    pub key: ExtraNodeKey,
    pub position: FontPoint,
    pub t_a: f64,
    pub t_b: f64,
}

/// On-canvas 1D ray-projected parametric smart handle attached to a `CanvasComponent` (`SPEC-009 §2.3.1`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CanvasSmartHandle {
    pub component_id: ComponentId,
    pub axis_id: String,
    pub axis_name: String,
    pub anchor_upm: FontPoint,
    pub direction_unit: (f64, f64),
    pub scale_upm_per_unit: f64,
    pub min_val: f64,
    pub default_val: f64,
    pub max_val: f64,
    pub current_val: f64,
}

/// Reference raster background image placed in glyph UPM space (`SPEC-009 §2.1.3 & §2.5`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CanvasBackgroundImage {
    pub rgba_pixels: Vec<u8>,
    pub width_px: u32,
    pub height_px: u32,
    pub position: FontPoint,
    pub scale_x: f64,
    pub scale_y: f64,
    pub rotation_deg: f64,
    pub alpha: f32,
    pub locked: bool,
    pub selected: bool,
    pub crop_rect_px: Option<[u32; 4]>,
}

impl CanvasBackgroundImage {
    #[must_use]
    pub fn new(rgba_pixels: Vec<u8>, width_px: u32, height_px: u32) -> Self {
        Self {
            rgba_pixels,
            width_px,
            height_px,
            position: FontPoint::new(0.0, 0.0),
            scale_x: 1.0,
            scale_y: 1.0,
            rotation_deg: 0.0,
            alpha: 1.0,
            locked: false,
            selected: false,
            crop_rect_px: None,
        }
    }
}

/// Contextual `SelectionInfo` payload when a single `NodeType::HobbyStar` node is selected (`SPEC-009 §2.1.3`).
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct HobbyNodeInfo {
    pub node_id: NodeId,
    pub x: f64,
    pub y: f64,
    pub tension_in: f64,
    pub tension_out: f64,
    pub dir_constraint: HobbyDirConstraint,
}

/// Contextual `SelectionInfo` payload when `CanvasBackgroundImage` is selected (`SPEC-009 §2.1.3`).
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct BackgroundImageInfo {
    pub x: f64,
    pub y: f64,
    pub scale_x_pct: f64,
    pub scale_y_pct: f64,
    pub rotation_deg: f64,
    pub alpha: f32,
    pub locked: bool,
    pub crop_rect_px: Option<[u32; 4]>,
}

/// Partial transform/crop/opacity mutation payload for `CanvasBackgroundImage` (`SPEC-009 §2.1.3`).
#[derive(Debug, Clone, Copy, Default, PartialEq, Serialize, Deserialize)]
pub struct BackgroundImageMutation {
    pub position: Option<FontPoint>,
    pub scale_x: Option<f64>,
    pub scale_y: Option<f64>,
    pub rotation_deg: Option<f64>,
    pub alpha: Option<f32>,
    pub locked: Option<bool>,
    pub crop_rect_px: Option<Option<[u32; 4]>>,
}

/// Multi-layer batch path operation selector executed across active + editable companion master layers (`SPEC-009 §2.1.3`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum LayerPathOp {
    ReverseContours,
    CorrectPathDirection,
    TidyUpPaths,
    AddExtremes { force: bool },
    RemoveOverlap,
    RoundCoordinates,
}
