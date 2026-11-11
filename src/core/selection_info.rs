//! `SPEC-007` (`grafia v0.6.0`) 5-mode contextual `SelectionInfo` data model,
//! `GuideAlignment`, `CanvasAnchor`, and cubic handle/anchor geometry helpers.

pub use crate::core::selection_info_ops::{
    align_oncurve_nodes_to_background, collect_adjacent_offcurve_positions,
    compute_radially_scaled_handle_pos, enforce_smooth_collinearity_around_oncurve,
    find_first_offcurve_node_id, find_first_oncurve_with_adjacent_offcurve,
    project_point_to_background_contours, resolve_tethered_oncurve_for_offcurve,
    translate_oncurve_with_adjacent_handles, update_offcurve_with_smooth_constraint,
};

use crate::core::{FontPoint, NodeId, NodeType, TransformGridAnchor};
use crate::snapping::SelectedGuideRef;
use serde::{Deserialize, Serialize};

/// Alignment reference for horizontal guide X-coordinate display and positioning (`SPEC-007 §2.2.5`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum GuideAlignment {
    #[default]
    Lsb,
    Center,
    Rsb,
}

/// Unique identifier for a 2D glyph anchor (`CanvasAnchor`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct AnchorId(pub u64);

/// A named 2D attachment anchor on a glyph (`top`, `bottom`, `ogonek`, etc.).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct CanvasAnchor {
    pub id: AnchorId,
    pub name: String,
    pub position: FontPoint,
    pub selected: bool,
}

impl CanvasAnchor {
    #[must_use]
    pub fn new(id: AnchorId, name: impl Into<String>, position: FontPoint) -> Self {
        Self {
            id,
            name: name.into(),
            position,
            selected: false,
        }
    }
}

/// Mode 1 (`SPEC-007 §2.2.1`): No nodes and no guide selected — pure 2D base glyph metrics & bounding box.
#[derive(Clone, Debug, PartialEq)]
pub struct GlyphBaseInfo {
    pub glyph_name: String,
    pub bbox_x: f64,
    pub bbox_y: f64,
    pub bbox_w: f64,
    pub bbox_h: f64,
    pub lsb: f64,
    pub rsb: f64,
    pub advance_width: f64,
    pub active_slot_kerning_left: Option<i32>,
    pub kern_with_prev: Option<i32>,
    pub kern_with_next: Option<i32>,
    pub transform_anchor: TransformGridAnchor,
    pub proportional_scale: bool,
    pub contour_count: usize,
    pub total_node_count: usize,
}

/// Mode 2 (`SPEC-007 §2.2.2`): Exactly 1 `OnCurve` node (`Corner` or `Smooth`) selected.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SingleOnCurveInfo {
    pub node_id: NodeId,
    pub x: f64,
    pub y: f64,
    pub node_type: NodeType,
}

/// Mode 3 (`SPEC-007 §2.2.3`): Exactly 1 `OffCurve` BCP handle selected.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SingleOffCurveInfo {
    pub node_id: NodeId,
    pub anchor_node_id: NodeId,
    pub tethered_oncurve_id: NodeId,
    pub x: f64,
    pub y: f64,
    pub delta_x: f64,
    pub delta_y: f64,
    pub length: f64,
}

/// Mode 4 (`SPEC-007 §2.2.4`): 2 or more nodes selected — bounding box & 9-point anchor.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct MultiNodeInfo {
    pub origin_anchor: TransformGridAnchor,
    pub origin_x: f64,
    pub origin_y: f64,
    pub width: f64,
    pub height: f64,
    pub proportional_scale: bool,
    pub selected_node_count: usize,
    pub total_layer_node_count: usize,
}

/// Mode 5 (`SPEC-007 §2.2.5`): A local or global guide is selected and no nodes are selected.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct GuideInfo {
    pub guide_ref: SelectedGuideRef,
    pub x: f64,
    pub y: f64,
    pub angle_deg: f64,
    pub alignment: GuideAlignment,
    pub lock_angle: bool,
    pub show_measurement: bool,
}

/// 5-mode contextual selection inspection contract (`SPEC-007 §2.2`).
#[derive(Clone, Debug, PartialEq)]
pub enum SelectionInfo {
    GlyphBase(GlyphBaseInfo),
    SingleOnCurve(SingleOnCurveInfo),
    SingleOffCurve(SingleOffCurveInfo),
    MultiNode(MultiNodeInfo),
    Guide(GuideInfo),
}

/// Unified pure 2D VC controller-in command enum for the 5-mode Selection Info HUD (`SPEC-007 §2.2`).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum SelectionInfoAction {
    SetLsb(f64),
    SetRsb(f64),
    SetAdvanceWidth(f64),
    CenterInWidth,
    SetOnCurvePosition(FontPoint),
    ToggleOnCurveSmooth,
    SetOffCurvePosition(FontPoint),
    SetOffCurveDelta {
        dx: f64,
        dy: f64,
    },
    SetHandleLength(f64),
    SetMultiAnchor(TransformGridAnchor),
    SetMultiProportional(bool),
    SetMultiOrigin(FontPoint),
    SetMultiSize {
        width: Option<f64>,
        height: Option<f64>,
    },
    SetGuideOrigin(FontPoint),
    SetGuideAngle(f64),
    SetGuideAlignment(GuideAlignment),
    SetGuideLockAngle(bool),
    SetGuideMeasurement(bool),
    SetAnchorPosition(FontPoint),
    SetAnchorName(String),
    ResetDefaultAnchors,
    SetComponentTransform(crate::core::anchors_components::Affine2D),
    SetComponentLocked(bool),
    SetComponentAutoAlignment(bool),
    DecomposeComponents,
}
