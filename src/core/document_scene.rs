//! Cohesive host document scene aggregate (`CanvasDocumentScene` — `SPEC-014 §2.4`).

use crate::core::{
    CanvasAnchor, CanvasBackgroundImage, CanvasColorSublayer, CanvasComponent, CanvasSmartHandle,
    CompanionMasterLayer, ContourId, ContourStrokeStyle, CustomReferenceFrame, Glyph, GlyphContour,
    HoiCubicLayerSet, HoiSiblingContours, PostScriptHint, TrueTypeVisualInstruction,
    WritingDirection,
};
use crate::snapping::{GlyphMetrics, Guide};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// Local and global guide collection and visibility/lock state for a glyph document scene (`SPEC-014 §2.4`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CanvasGuideSet {
    pub local_guides: Vec<Guide>,
    pub global_guides: Vec<Guide>,
    pub show_guides: bool,
    pub lock_guides: bool,
}

impl Default for CanvasGuideSet {
    fn default() -> Self {
        Self {
            local_guides: Vec::new(),
            global_guides: Vec::new(),
            show_guides: true,
            lock_guides: false,
        }
    }
}

/// Auxiliary layer attachments (background layer, anchors, components, strokes, smart handles, backdrop) for a glyph scene (`SPEC-014 §2.4`).
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct CanvasLayerAttachments {
    pub background_contours: Vec<GlyphContour>,
    pub anchors: Vec<CanvasAnchor>,
    pub components: Vec<CanvasComponent>,
    pub contour_strokes: BTreeMap<ContourId, ContourStrokeStyle>,
    pub companion_master_layers: Vec<CompanionMasterLayer>,
    pub smart_handles: Vec<CanvasSmartHandle>,
    pub background_image: Option<CanvasBackgroundImage>,
    pub backdrop_ghost_contours: Vec<GlyphContour>,
}

/// PostScript hints, TrueType visual instructions, HOI control sets, and pixel preview PPEM for a glyph scene (`SPEC-014 §2.4`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CanvasHintingAndHoiScene {
    pub postscript_hints: Vec<PostScriptHint>,
    pub truetype_instructions: Vec<TrueTypeVisualInstruction>,
    pub truetype_instructor_ppem: u16,
    pub hoi_layer_sets: Vec<HoiCubicLayerSet>,
    pub hoi_sibling_contours: Option<HoiSiblingContours>,
    pub show_pixel_preview_ppem: Option<u16>,
}

impl Default for CanvasHintingAndHoiScene {
    fn default() -> Self {
        Self {
            postscript_hints: Vec::new(),
            truetype_instructions: Vec::new(),
            truetype_instructor_ppem: crate::core::math_consts::TT_DEFAULT_PPEM,
            hoi_layer_sets: Vec::new(),
            hoi_sibling_contours: None,
            show_pixel_preview_ppem: None,
        }
    }
}

/// Complete, cohesive host document scene aggregate for atomic loading and exporting on `GrafiaCanvasState` (`SPEC-014 §2.4`).
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct CanvasDocumentScene {
    pub glyph: Glyph,
    pub metrics: GlyphMetrics,
    pub guides: CanvasGuideSet,
    pub layers: CanvasLayerAttachments,
    pub color_layers: Vec<CanvasColorSublayer>,
    pub hinting: CanvasHintingAndHoiScene,
    pub reference_frame: Option<CustomReferenceFrame>,
    pub writing_direction: WritingDirection,
}
