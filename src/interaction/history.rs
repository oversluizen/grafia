//! O(1) Copy-on-Write undo/redo transaction stack with mouse-drag coalescing.

use crate::core::{
    AnnotationId, CanvasAnchor, CanvasAnnotation, CanvasColorSublayer, CanvasComponent,
    CompanionMasterLayer, ContourId, ContourStrokeStyle, CustomReferenceFrame, FontPoint,
    GlyphContour, GlyphSlot, HoiCubicLayerSet, HoiSiblingContours, Node, NodeId, PostScriptHint,
    ShapeBooleanRole, TransformGridAnchor, TrueTypeVisualInstruction, WritingDirection,
};
use crate::snapping::{Guide, SelectedGuideRef};
use crate::tokens::{CanvasBackgroundImage, CanvasSmartHandle, ExtraNodeKey};
use kurbo::BezPath;
use std::collections::BTreeMap;
use std::sync::Arc;

#[derive(Debug, Clone, PartialEq)]
pub struct SnapshotLayerAttachments {
    pub background_contours: Vec<GlyphContour>,
    pub editing_background_layer: bool,
    pub show_background: bool,
    pub anchors: Vec<CanvasAnchor>,
    pub components: Vec<CanvasComponent>,
    pub contour_strokes: BTreeMap<ContourId, ContourStrokeStyle>,
    pub companion_master_layers: Vec<CompanionMasterLayer>,
    pub selected_companion_nodes: Vec<(u32, NodeId)>,
    pub show_master_compatibility: bool,
    pub background_image: Option<CanvasBackgroundImage>,
    pub smart_handles: Vec<CanvasSmartHandle>,
    pub selected_extra_node: Option<ExtraNodeKey>,
    pub backdrop_ghost_contours: Vec<GlyphContour>,
}

impl Default for SnapshotLayerAttachments {
    fn default() -> Self {
        Self {
            background_contours: Vec::new(),
            editing_background_layer: false,
            show_background: true,
            anchors: Vec::new(),
            components: Vec::new(),
            contour_strokes: BTreeMap::new(),
            companion_master_layers: Vec::new(),
            selected_companion_nodes: Vec::new(),
            show_master_compatibility: false,
            background_image: None,
            smart_handles: Vec::new(),
            selected_extra_node: None,
            backdrop_ghost_contours: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct SnapshotHintingAndHoiState {
    pub color_sublayers: Vec<CanvasColorSublayer>,
    pub selected_color_sublayer: Option<usize>,
    pub selected_gradient_stop: Option<(usize, usize)>,
    pub custom_reference_frame: Option<CustomReferenceFrame>,
    pub postscript_hints: Vec<PostScriptHint>,
    pub selected_postscript_hint: Option<u64>,
    pub truetype_instructions: Vec<TrueTypeVisualInstruction>,
    pub selected_truetype_instruction: Option<u64>,
    pub hoi_layer_sets: Vec<HoiCubicLayerSet>,
    pub hoi_sibling_contours: Option<HoiSiblingContours>,
    pub curve_harmony: crate::core::CurveHarmonyState,
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct ImageAndSmartHandleSnapshot {
    pub background_image: Option<CanvasBackgroundImage>,
    pub smart_handles: Vec<CanvasSmartHandle>,
    pub selected_extra_node: Option<ExtraNodeKey>,
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct HintingAndHoiSnapshot {
    pub postscript_hints: Vec<PostScriptHint>,
    pub selected_postscript_hint: Option<u64>,
    pub truetype_instructions: Vec<TrueTypeVisualInstruction>,
    pub selected_truetype_instruction: Option<u64>,
    pub hoi_layer_sets: Vec<HoiCubicLayerSet>,
    pub hoi_sibling_contours: Option<HoiSiblingContours>,
    pub curve_harmony: crate::core::CurveHarmonyState,
}

#[derive(Debug, Clone, PartialEq)]
pub struct GlyphSnapshot {
    pub contours: Vec<Vec<Node>>,
    pub closed_flags: Vec<bool>,
    pub contour_meta: Vec<(ShapeBooleanRole, bool)>,
    pub paths: Arc<[BezPath]>,
    pub selected_nodes: Vec<NodeId>,
    pub pending_pen_bcp_out: Option<(usize, FontPoint)>,
    pub pending_pen_first_bcp_in: Option<(usize, FontPoint)>,
    pub guides: Vec<Guide>,
    pub global_guides: Vec<Guide>,
    pub selected_guide: Option<SelectedGuideRef>,
    pub annotations: Vec<CanvasAnnotation>,
    pub selected_annotations: Vec<AnnotationId>,
    pub has_glyph_metrics: bool,
    pub glyph_name: String,
    pub glyph_width: f64,
    pub glyph_height: f64,
    pub text_stream_slots: Vec<GlyphSlot>,
    pub active_slot_index: usize,
    pub caret_index: usize,
    pub selection_range: Option<(usize, usize)>,
    pub multi_selection_proportional: bool,
    pub transform_grid_anchor: TransformGridAnchor,
    pub show_rulers: bool,
    pub writing_direction: WritingDirection,
    pub layers: SnapshotLayerAttachments,
    pub hinting: SnapshotHintingAndHoiState,
}

impl Default for GlyphSnapshot {
    fn default() -> Self {
        Self {
            contours: Vec::new(),
            closed_flags: Vec::new(),
            contour_meta: Vec::new(),
            paths: Arc::from([]),
            selected_nodes: Vec::new(),
            pending_pen_bcp_out: None,
            pending_pen_first_bcp_in: None,
            guides: Vec::new(),
            global_guides: Vec::new(),
            selected_guide: None,
            annotations: Vec::new(),
            selected_annotations: Vec::new(),
            has_glyph_metrics: false,
            glyph_name: String::new(),
            glyph_width: 0.0,
            glyph_height: 0.0,
            text_stream_slots: Vec::new(),
            active_slot_index: 0,
            caret_index: 0,
            selection_range: None,
            multi_selection_proportional: false,
            transform_grid_anchor: TransformGridAnchor::Center,
            show_rulers: false,
            writing_direction: WritingDirection::default(),
            layers: SnapshotLayerAttachments::default(),
            hinting: SnapshotHintingAndHoiState::default(),
        }
    }
}

impl GlyphSnapshot {
    #[must_use]
    pub fn from_state(
        contours: &[Vec<Node>],
        closed_flags: Vec<bool>,
        paths: Arc<[BezPath]>,
        selected_nodes: &[NodeId],
    ) -> Self {
        let contour_meta = vec![(ShapeBooleanRole::Normal, true); contours.len()];
        Self {
            contours: contours.to_vec(),
            closed_flags,
            contour_meta,
            paths,
            selected_nodes: selected_nodes.to_vec(),
            ..Self::default()
        }
    }

    #[must_use]
    pub fn with_contour_meta(mut self, contour_meta: Vec<(ShapeBooleanRole, bool)>) -> Self {
        self.contour_meta = contour_meta;
        self
    }

    #[must_use]
    pub fn with_pending_pen_handles(
        mut self,
        pending_pen_bcp_out: Option<(usize, FontPoint)>,
        pending_pen_first_bcp_in: Option<(usize, FontPoint)>,
    ) -> Self {
        self.pending_pen_bcp_out = pending_pen_bcp_out;
        self.pending_pen_first_bcp_in = pending_pen_first_bcp_in;
        self
    }

    #[must_use]
    pub fn with_guides(
        mut self,
        guides: Vec<Guide>,
        global_guides: Vec<Guide>,
        selected_guide: Option<SelectedGuideRef>,
    ) -> Self {
        self.guides = guides;
        self.global_guides = global_guides;
        self.selected_guide = selected_guide;
        self
    }

    #[must_use]
    pub fn with_annotations(
        mut self,
        annotations: Vec<CanvasAnnotation>,
        selected_annotations: Vec<AnnotationId>,
    ) -> Self {
        self.annotations = annotations;
        self.selected_annotations = selected_annotations;
        self
    }

    #[must_use]
    pub fn with_text_stream(
        mut self,
        glyph_name: String,
        glyph_size: (f64, f64),
        text_stream_slots: Vec<GlyphSlot>,
        active_slot_index: usize,
        caret_index: usize,
        selection_range: Option<(usize, usize)>,
    ) -> Self {
        let (glyph_width, glyph_height) = glyph_size;
        self.has_glyph_metrics = true;
        self.glyph_name = glyph_name;
        self.glyph_width = glyph_width;
        self.glyph_height = glyph_height;
        self.text_stream_slots = text_stream_slots;
        self.active_slot_index = active_slot_index;
        self.caret_index = caret_index;
        self.selection_range = selection_range;
        self
    }

    #[must_use]
    pub fn with_background_and_ruler_state(
        mut self,
        background_contours: Vec<GlyphContour>,
        bg_flags: (bool, bool),
        anchors: Vec<CanvasAnchor>,
        multi_selection_proportional: bool,
        transform_grid_anchor: TransformGridAnchor,
        show_rulers: bool,
    ) -> Self {
        let (editing_background_layer, show_background) = bg_flags;
        self.layers.background_contours = background_contours;
        self.layers.editing_background_layer = editing_background_layer;
        self.layers.show_background = show_background;
        self.layers.anchors = anchors;
        self.multi_selection_proportional = multi_selection_proportional;
        self.transform_grid_anchor = transform_grid_anchor;
        self.show_rulers = show_rulers;
        self
    }

    #[must_use]
    pub fn with_components_and_strokes_state(
        mut self,
        components: Vec<CanvasComponent>,
        contour_strokes: BTreeMap<ContourId, ContourStrokeStyle>,
        companion_master_layers: Vec<CompanionMasterLayer>,
        selected_companion_nodes: Vec<(u32, NodeId)>,
        show_master_compatibility: bool,
    ) -> Self {
        self.layers.components = components;
        self.layers.contour_strokes = contour_strokes;
        self.layers.companion_master_layers = companion_master_layers;
        self.layers.selected_companion_nodes = selected_companion_nodes;
        self.layers.show_master_compatibility = show_master_compatibility;
        self
    }

    #[must_use]
    pub fn with_image_and_smart_handle_state(
        mut self,
        image_and_handles: ImageAndSmartHandleSnapshot,
    ) -> Self {
        self.layers.background_image = image_and_handles.background_image;
        self.layers.smart_handles = image_and_handles.smart_handles;
        self.layers.selected_extra_node = image_and_handles.selected_extra_node;
        self
    }

    #[must_use]
    pub fn with_color_layers_and_frame_state(
        mut self,
        color_sublayers: Vec<CanvasColorSublayer>,
        selected_color_sublayer: Option<usize>,
        selected_gradient_stop: Option<(usize, usize)>,
        custom_reference_frame: Option<CustomReferenceFrame>,
        writing_direction: WritingDirection,
    ) -> Self {
        self.hinting.color_sublayers = color_sublayers;
        self.hinting.selected_color_sublayer = selected_color_sublayer;
        self.hinting.selected_gradient_stop = selected_gradient_stop;
        self.hinting.custom_reference_frame = custom_reference_frame;
        self.writing_direction = writing_direction;
        self
    }

    #[must_use]
    pub fn with_hinting_and_hoi_state(mut self, hinting_and_hoi: HintingAndHoiSnapshot) -> Self {
        self.hinting.postscript_hints = hinting_and_hoi.postscript_hints;
        self.hinting.selected_postscript_hint = hinting_and_hoi.selected_postscript_hint;
        self.hinting.truetype_instructions = hinting_and_hoi.truetype_instructions;
        self.hinting.selected_truetype_instruction = hinting_and_hoi.selected_truetype_instruction;
        self.hinting.hoi_layer_sets = hinting_and_hoi.hoi_layer_sets;
        self.hinting.hoi_sibling_contours = hinting_and_hoi.hoi_sibling_contours;
        self.hinting.curve_harmony = hinting_and_hoi.curve_harmony;
        self
    }

    /// Returns `true` if any geometry, layer, text-stream, anchor position/name, component transform/lock,
    /// stroke, guide, or annotation field differs between `self` and `other` (ignoring pure selection or HUD flags).
    #[must_use]
    pub fn has_content_change(&self, other: &Self) -> bool {
        let anchors_changed = self.layers.anchors.len() != other.layers.anchors.len()
            || self
                .layers
                .anchors
                .iter()
                .zip(&other.layers.anchors)
                .any(|(a, b)| a.id != b.id || a.name != b.name || a.position != b.position);
        let components_changed = self.layers.components.len() != other.layers.components.len()
            || self
                .layers
                .components
                .iter()
                .zip(&other.layers.components)
                .any(|(a, b)| {
                    a.id != b.id
                        || a.component_name != b.component_name
                        || a.contours != b.contours
                        || a.transform != b.transform
                        || a.locked != b.locked
                        || a.automatic_alignment != b.automatic_alignment
                });
        self.contours != other.contours
            || self.closed_flags != other.closed_flags
            || self.contour_meta != other.contour_meta
            || self.pending_pen_bcp_out != other.pending_pen_bcp_out
            || self.pending_pen_first_bcp_in != other.pending_pen_first_bcp_in
            || self.guides != other.guides
            || self.global_guides != other.global_guides
            || self.annotations != other.annotations
            || self.glyph_name != other.glyph_name
            || self.glyph_width.to_bits() != other.glyph_width.to_bits()
            || self.glyph_height.to_bits() != other.glyph_height.to_bits()
            || self.text_stream_slots != other.text_stream_slots
            || self.layers.background_contours != other.layers.background_contours
            || self.layers.editing_background_layer != other.layers.editing_background_layer
            || self.layers.show_background != other.layers.show_background
            || anchors_changed
            || components_changed
            || self.layers.contour_strokes != other.layers.contour_strokes
            || self.layers.companion_master_layers != other.layers.companion_master_layers
            || self.layers.background_image != other.layers.background_image
            || self.layers.smart_handles != other.layers.smart_handles
            || self.layers.backdrop_ghost_contours != other.layers.backdrop_ghost_contours
            || self.hinting.color_sublayers != other.hinting.color_sublayers
            || self.hinting.custom_reference_frame != other.hinting.custom_reference_frame
            || self.writing_direction != other.writing_direction
            || self.hinting.postscript_hints != other.hinting.postscript_hints
            || self.hinting.truetype_instructions != other.hinting.truetype_instructions
            || self.hinting.hoi_layer_sets != other.hinting.hoi_layer_sets
            || self.hinting.hoi_sibling_contours != other.hinting.hoi_sibling_contours
            || self.hinting.curve_harmony != other.hinting.curve_harmony
    }
}

#[derive(Debug, Clone)]
pub struct HistoryTransaction {
    pub label: &'static str,
    pub before: GlyphSnapshot,
    pub after: GlyphSnapshot,
}

#[derive(Debug, Default)]
pub struct CommandHistory {
    undo_stack: Vec<HistoryTransaction>,
    redo_stack: Vec<HistoryTransaction>,
}

impl CommandHistory {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    pub fn record(&mut self, label: &'static str, before: GlyphSnapshot, after: GlyphSnapshot) {
        self.undo_stack.push(HistoryTransaction {
            label,
            before,
            after,
        });
        self.redo_stack.clear();
    }

    pub fn undo(&mut self, current: &mut GlyphSnapshot) -> bool {
        if let Some(tx) = self.undo_stack.pop() {
            *current = tx.before.clone();
            self.redo_stack.push(tx);
            true
        } else {
            false
        }
    }

    pub fn redo(&mut self, current: &mut GlyphSnapshot) -> bool {
        if let Some(tx) = self.redo_stack.pop() {
            *current = tx.after.clone();
            self.undo_stack.push(tx);
            true
        } else {
            false
        }
    }

    #[must_use]
    pub fn can_undo(&self) -> bool {
        !self.undo_stack.is_empty()
    }

    #[must_use]
    pub fn can_redo(&self) -> bool {
        !self.redo_stack.is_empty()
    }

    #[must_use]
    pub fn last_undo_label(&self) -> Option<&'static str> {
        self.undo_stack.last().map(|tx| tx.label)
    }
}
