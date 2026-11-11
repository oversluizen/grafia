//! `SPEC-008 §2.2 & §2.3`: 2D `CanvasAnchor` Mark Preview Cloud (`GhostMarkPath`),
//! 2D Pre-Resolved `CanvasComponent`, `Affine2D`, Decompose (`Cmd+Shift+D`),
//! `CanvasHostRequest`, and VC selection telemetry (`AnchorSelectionInfo`, `ComponentSelectionInfo`).

use crate::core::bezier_ops::{Node, NodeId};
use crate::core::coordinates::FontPoint;
use crate::core::glyph_model::{ContourId, GlyphContour};
use crate::core::lasso_and_zoom::point_in_polygon;
use crate::core::segment_ops::reverse_contour_nodes;
use crate::core::selection_info::{AnchorId, CanvasAnchor};
pub use crate::tokens::{
    CanvasHostRequest, ComponentId, TextStreamEditKind, TextStreamEditRequest,
};
use serde::{Deserialize, Serialize};

/// Pre-resolved mark outline cloud associated with an anchor name (`SPEC-008 §2.2.1`).
///
/// Contours are expressed in mark-local coordinates relative to `(0, 0)` at the matching
/// `_anchor` attachment point, so translating by `anchor.position` yields the live mark cloud.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct GhostMarkPath {
    pub anchor_name: String,
    pub contours: Vec<GlyphContour>,
}

/// 2D affine transform matrix $\begin{bmatrix} a & c & t_x \\ b & d & t_y \\ 0 & 0 & 1 \end{bmatrix}$ (`SPEC-008 §2.3.1`).
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Affine2D {
    pub a: f64,
    pub b: f64,
    pub c: f64,
    pub d: f64,
    pub tx: f64,
    pub ty: f64,
}

impl Default for Affine2D {
    fn default() -> Self {
        Self::identity()
    }
}

impl Affine2D {
    /// Returns the 2D identity transform (`a = 1, d = 1`, all other entries `0`).
    #[must_use]
    pub const fn identity() -> Self {
        Self {
            a: 1.0,
            b: 0.0,
            c: 0.0,
            d: 1.0,
            tx: 0.0,
            ty: 0.0,
        }
    }

    /// Returns a pure 2D translation transform by `(tx, ty)`.
    #[must_use]
    pub const fn translation(tx: f64, ty: f64) -> Self {
        Self {
            a: 1.0,
            b: 0.0,
            c: 0.0,
            d: 1.0,
            tx,
            ty,
        }
    }

    /// Computes the $2 \times 2$ linear determinant $ad - bc$.
    #[must_use]
    pub fn determinant(&self) -> f64 {
        self.a * self.d - self.b * self.c
    }

    /// Applies the 2D affine matrix to a `FontPoint` in UPM space:
    /// $(x', y') = (a x + c y + t_x,\; b x + d y + t_y)$.
    #[must_use]
    pub fn transform_point(&self, pt: FontPoint) -> FontPoint {
        FontPoint::new(
            self.a * pt.x.0 + self.c * pt.y.0 + self.tx,
            self.b * pt.x.0 + self.d * pt.y.0 + self.ty,
        )
    }

    /// Transforms a `GlyphContour` by this affine matrix, assigning fresh `ContourId` and `NodeId`s,
    /// and reversing node order when `self.determinant() < 0.0` so winding orientation is preserved (`SPEC-008 §2.3.2`).
    pub fn transform_contour(
        &self,
        contour: &GlyphContour,
        new_id: ContourId,
        next_node_id: &mut u64,
    ) -> GlyphContour {
        let mut nodes: Vec<Node> = contour
            .nodes
            .iter()
            .map(|n| {
                let id = NodeId(*next_node_id);
                *next_node_id += 1;
                let tp = self.transform_point(n.point);
                Node::new(id, tp.x.0, tp.y.0, n.node_type)
            })
            .collect();
        if self.determinant() < 0.0 {
            reverse_contour_nodes(&mut nodes, contour.closed);
        }
        let mut out = GlyphContour::new(new_id, nodes);
        out.closed = contour.closed;
        out
    }
}

/// Pre-resolved 2D component reference (`SPEC-008 §2.3.1`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CanvasComponent {
    pub id: ComponentId,
    pub component_name: String,
    pub contours: Vec<GlyphContour>,
    pub transform: Affine2D,
    pub locked: bool,
    pub automatic_alignment: bool,
    pub selected: bool,
}

impl CanvasComponent {
    /// Returns all contours of this component transformed into glyph UPM coordinates.
    #[must_use]
    pub fn transformed_contours(&self) -> Vec<GlyphContour> {
        let mut dummy_id = 1;
        self.contours
            .iter()
            .enumerate()
            .map(|(idx, c)| {
                self.transform
                    .transform_contour(c, ContourId(idx), &mut dummy_id)
            })
            .collect()
    }
}

/// VC-stekker telemetry payload for an active selected `CanvasAnchor` (`SPEC-008`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AnchorSelectionInfo {
    pub id: AnchorId,
    pub name: String,
    pub x: f64,
    pub y: f64,
}

/// VC-stekker telemetry payload for an active selected `CanvasComponent` (`SPEC-008`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ComponentSelectionInfo {
    pub id: ComponentId,
    pub component_name: String,
    pub transform: Affine2D,
    pub locked: bool,
    pub automatic_alignment: bool,
}

/// Translates matching `GhostMarkPath` contours to the positions of all currently selected `CanvasAnchor`s (`SPEC-008 §2.2.2`).
#[must_use]
pub fn compute_mark_cloud_contours(
    anchors: &[CanvasAnchor],
    ghost_paths: &[GhostMarkPath],
    show_mark_cloud: bool,
) -> Vec<GlyphContour> {
    if !show_mark_cloud {
        return Vec::new();
    }
    let mut out = Vec::new();
    let mut next_cid = 10_000usize;
    for anchor in anchors.iter().filter(|a| a.selected) {
        let base_name = anchor.name.trim_start_matches('_');
        for gp in ghost_paths {
            if gp.anchor_name.trim_start_matches('_') == base_name {
                let tx = Affine2D::translation(anchor.position.x.0, anchor.position.y.0);
                let mut nid = 50_000u64;
                for c in &gp.contours {
                    out.push(tx.transform_contour(c, ContourId(next_cid), &mut nid));
                    next_cid += 1;
                }
            }
        }
    }
    out
}

/// Hit-tests a `CanvasComponent` at `pt` in UPM coordinates using its transformed contours and bounding box.
#[must_use]
pub fn hit_test_component_upm(component: &CanvasComponent, pt: FontPoint) -> bool {
    let transformed = component.transformed_contours();
    if transformed.is_empty() {
        return false;
    }
    let mut min_x = f64::INFINITY;
    let mut max_x = f64::NEG_INFINITY;
    let mut min_y = f64::INFINITY;
    let mut max_y = f64::NEG_INFINITY;
    for c in &transformed {
        let poly: Vec<FontPoint> = c.nodes.iter().map(|n| n.point).collect();
        if poly.len() >= 3 && point_in_polygon(pt, &poly) {
            return true;
        }
        for n in &c.nodes {
            min_x = min_x.min(n.point.x.0);
            max_x = max_x.max(n.point.x.0);
            min_y = min_y.min(n.point.y.0);
            max_y = max_y.max(n.point.y.0);
        }
    }
    pt.x.0 >= min_x && pt.x.0 <= max_x && pt.y.0 >= min_y && pt.y.0 <= max_y
}
