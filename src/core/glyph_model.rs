//! Hierarchical glyph data model and contour identity representations.
//!
//! Normative: SPEC-002 Deel I §5 (Shape Hierarchy Model).

use crate::core::annotations::CanvasAnnotation;
use crate::core::bezier_ops::Node;
use crate::core::selection_info::CanvasAnchor;
use serde::{Deserialize, Serialize};

/// Type-safe contour identifier.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct ContourId(pub usize);

impl From<usize> for ContourId {
    fn from(val: usize) -> Self {
        Self(val)
    }
}

impl From<u64> for ContourId {
    fn from(val: u64) -> Self {
        Self(val as usize)
    }
}

/// Type alias for `ContourId` conforming to SPEC-002 §5.1.
pub type ShapeId = ContourId;

/// Boolean role for live shape and mask evaluations (SPEC-002 §5.1).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum ShapeBooleanRole {
    #[default]
    Normal,
    Mask,
    Add,
    Intersect,
}

/// A single vector contour (or shape) within a glyph hierarchy.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct GlyphContour {
    pub id: ContourId,
    pub nodes: Vec<Node>,
    pub role: ShapeBooleanRole,
    pub visible: bool,
    pub locked: bool,
    #[serde(default = "default_closed_true")]
    pub closed: bool,
}

fn default_closed_true() -> bool {
    true
}

impl GlyphContour {
    #[must_use]
    pub fn new(id: ContourId, nodes: Vec<Node>) -> Self {
        Self {
            id,
            nodes,
            role: ShapeBooleanRole::Normal,
            visible: true,
            locked: false,
            closed: true,
        }
    }
}

/// Type alias for `GlyphContour` conforming to SPEC-002 §5.1.
pub type Shape = GlyphContour;

/// High-level glyph structure containing outline contours and typographic metrics.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct Glyph {
    pub name: String,
    pub upm: u16,
    pub width: f64,
    pub height: f64,
    pub contours: Vec<GlyphContour>,
    #[serde(default)]
    pub annotations: Vec<CanvasAnnotation>,
    #[serde(default)]
    pub background_contours: Vec<GlyphContour>,
    #[serde(default)]
    pub anchors: Vec<CanvasAnchor>,
}

/// Type alias for `Glyph` conforming to SPEC-002 §5.1.
pub type GlyphHierarchy = Glyph;

impl Glyph {
    #[must_use]
    pub fn new(name: impl Into<String>, upm: u16) -> Self {
        Self {
            name: name.into(),
            upm,
            width: 0.0,
            height: 0.0,
            contours: Vec::new(),
            annotations: Vec::new(),
            background_contours: Vec::new(),
            anchors: Vec::new(),
        }
    }

    /// Returns `true` if the glyph contains an anchor with the given `name`.
    #[must_use]
    pub fn has_anchor(&self, name: &str) -> bool {
        self.anchors.iter().any(|a| a.name == name)
    }

    /// Returns a list of all active contour IDs in order.
    #[must_use]
    pub fn contour_ids(&self) -> Vec<ContourId> {
        self.contours.iter().map(|c| c.id).collect()
    }
}
