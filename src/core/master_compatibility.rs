//! `SPEC-008 §2.6`: `CompanionMasterLayer`, Master Compatibility (`Ctrl+Opt+Cmd+N`),
//! and `SelectAllLayers` multi-layer node selection & translation.

use crate::core::bezier_ops::{Node, NodeId, NodeType};
use crate::core::coordinates::FontPoint;
use crate::core::glyph_model::GlyphContour;
use crate::core::selection_info::CanvasAnchor;
use serde::{Deserialize, Serialize};

/// Companion master layer displayed alongside the active glyph layer (`SPEC-008 §2.6.1`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CompanionMasterLayer {
    pub id: u32,
    pub name: String,
    pub color_rgba: [u8; 4],
    pub contours: Vec<Vec<Node>>,
    pub closed_flags: Vec<bool>,
    pub anchors: Vec<CanvasAnchor>,
    pub editable: bool,
}

/// Master interpolation compatibility report between the active layer and all `CompanionMasterLayer`s (`SPEC-008 §2.6.2`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MasterCompatibilityReport {
    pub is_compatible: bool,
    pub start_node_vectors: Vec<(FontPoint, FontPoint)>,
    pub anchor_vectors: Vec<(FontPoint, FontPoint)>,
    pub incompatible_contour_indices: Vec<usize>,
}

/// Evaluates interpolation compatibility across the active layer and all `companion_layers` (`SPEC-008 §2.6.2`).
#[must_use]
pub fn compute_master_compatibility_report(
    active_contours: &[GlyphContour],
    active_anchors: &[CanvasAnchor],
    companion_layers: &[CompanionMasterLayer],
) -> MasterCompatibilityReport {
    let mut start_node_vectors = Vec::new();
    let mut anchor_vectors = Vec::new();
    let mut incompatible_contour_indices = Vec::new();
    let mut all_compatible = true;

    for comp in companion_layers {
        if comp.contours.len() != active_contours.len() {
            all_compatible = false;
        }
        let max_c = active_contours.len().max(comp.contours.len());
        for ci in 0..max_c {
            let Some(act_c) = active_contours.get(ci) else {
                if !incompatible_contour_indices.contains(&ci) {
                    incompatible_contour_indices.push(ci);
                }
                all_compatible = false;
                continue;
            };
            let Some(comp_nodes) = comp.contours.get(ci) else {
                if !incompatible_contour_indices.contains(&ci) {
                    incompatible_contour_indices.push(ci);
                }
                all_compatible = false;
                continue;
            };
            let comp_closed = comp.closed_flags.get(ci).copied().unwrap_or(false);
            let types_match = act_c.nodes.len() == comp_nodes.len()
                && act_c.closed == comp_closed
                && act_c.nodes.iter().zip(comp_nodes.iter()).all(|(a, b)| {
                    (a.node_type == NodeType::OffCurve) == (b.node_type == NodeType::OffCurve)
                });
            if !types_match {
                if !incompatible_contour_indices.contains(&ci) {
                    incompatible_contour_indices.push(ci);
                }
                all_compatible = false;
            }
            if let (Some(a0), Some(b0)) = (act_c.nodes.first(), comp_nodes.first()) {
                start_node_vectors.push((a0.point, b0.point));
            }
        }

        if comp.anchors.len() != active_anchors.len()
            || !comp
                .anchors
                .iter()
                .all(|ca| active_anchors.iter().any(|aa| aa.name == ca.name))
        {
            all_compatible = false;
        }
        for act_a in active_anchors {
            if let Some(comp_a) = comp.anchors.iter().find(|ca| ca.name == act_a.name) {
                anchor_vectors.push((act_a.position, comp_a.position));
            } else {
                all_compatible = false;
            }
        }
    }

    MasterCompatibilityReport {
        is_compatible: all_compatible && incompatible_contour_indices.is_empty(),
        start_node_vectors,
        anchor_vectors,
        incompatible_contour_indices,
    }
}

/// Collects `NodeId`s in the active layer and `(layer_id, NodeId)` pairs in editable `CompanionMasterLayer`s
/// that fall inside the axis-aligned UPM bounding box `[min, max]` (`SPEC-008 §2.6.3`).
#[must_use]
pub fn select_nodes_in_rect_across_layers(
    active_contours: &[GlyphContour],
    companion_layers: &[CompanionMasterLayer],
    select_all_layers_mode: bool,
    min: FontPoint,
    max: FontPoint,
) -> (Vec<NodeId>, Vec<(u32, NodeId)>) {
    let min_x = min.x.0.min(max.x.0);
    let max_x = min.x.0.max(max.x.0);
    let min_y = min.y.0.min(max.y.0);
    let max_y = min.y.0.max(max.y.0);
    let in_rect =
        |pt: FontPoint| pt.x.0 >= min_x && pt.x.0 <= max_x && pt.y.0 >= min_y && pt.y.0 <= max_y;

    let mut active_selected = Vec::new();
    for c in active_contours {
        for n in &c.nodes {
            if in_rect(n.point) {
                active_selected.push(n.id);
            }
        }
    }

    let mut companion_selected = Vec::new();
    if select_all_layers_mode {
        for layer in companion_layers.iter().filter(|l| l.editable) {
            for c in &layer.contours {
                for n in c {
                    if in_rect(n.point) {
                        companion_selected.push((layer.id, n.id));
                    }
                }
            }
        }
    }

    (active_selected, companion_selected)
}
