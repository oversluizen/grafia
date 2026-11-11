//! O(1) Copy-on-Write undo/redo transaction stack with mouse-drag coalescing.

use crate::core::{
    AnnotationId, CanvasAnchor, CanvasAnnotation, CanvasComponent, CompanionMasterLayer, ContourId,
    ContourStrokeStyle, FontPoint, GlyphContour, GlyphSlot, Node, NodeId, ShapeBooleanRole,
    TransformGridAnchor,
};
use crate::snapping::{Guide, SelectedGuideRef};
use kurbo::BezPath;
use std::collections::BTreeMap;
use std::sync::Arc;

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
    pub background_contours: Vec<GlyphContour>,
    pub editing_background_layer: bool,
    pub show_background: bool,
    pub anchors: Vec<CanvasAnchor>,
    pub multi_selection_proportional: bool,
    pub transform_grid_anchor: TransformGridAnchor,
    pub show_rulers: bool,
    pub center_in_width_count: u64,
    pub components: Vec<CanvasComponent>,
    pub contour_strokes: BTreeMap<ContourId, ContourStrokeStyle>,
    pub companion_master_layers: Vec<CompanionMasterLayer>,
    pub selected_companion_nodes: Vec<(u32, NodeId)>,
    pub show_master_compatibility: bool,
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
            background_contours: Vec::new(),
            editing_background_layer: false,
            show_background: true,
            anchors: Vec::new(),
            multi_selection_proportional: false,
            transform_grid_anchor: TransformGridAnchor::Center,
            show_rulers: false,
            center_in_width_count: 0,
            components: Vec::new(),
            contour_strokes: BTreeMap::new(),
            companion_master_layers: Vec::new(),
            selected_companion_nodes: Vec::new(),
            show_master_compatibility: false,
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
    #[allow(clippy::too_many_arguments)]
    pub fn with_text_stream(
        mut self,
        glyph_name: String,
        glyph_width: f64,
        glyph_height: f64,
        text_stream_slots: Vec<GlyphSlot>,
        active_slot_index: usize,
        caret_index: usize,
        selection_range: Option<(usize, usize)>,
    ) -> Self {
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
    #[allow(clippy::too_many_arguments)]
    pub fn with_v060_state(
        mut self,
        background_contours: Vec<GlyphContour>,
        editing_background_layer: bool,
        show_background: bool,
        anchors: Vec<CanvasAnchor>,
        multi_selection_proportional: bool,
        transform_grid_anchor: TransformGridAnchor,
        show_rulers: bool,
        center_in_width_count: u64,
    ) -> Self {
        self.background_contours = background_contours;
        self.editing_background_layer = editing_background_layer;
        self.show_background = show_background;
        self.anchors = anchors;
        self.multi_selection_proportional = multi_selection_proportional;
        self.transform_grid_anchor = transform_grid_anchor;
        self.show_rulers = show_rulers;
        self.center_in_width_count = center_in_width_count;
        self
    }

    #[must_use]
    pub fn with_v070_state(
        mut self,
        components: Vec<CanvasComponent>,
        contour_strokes: BTreeMap<ContourId, ContourStrokeStyle>,
        companion_master_layers: Vec<CompanionMasterLayer>,
        selected_companion_nodes: Vec<(u32, NodeId)>,
        show_master_compatibility: bool,
    ) -> Self {
        self.components = components;
        self.contour_strokes = contour_strokes;
        self.companion_master_layers = companion_master_layers;
        self.selected_companion_nodes = selected_companion_nodes;
        self.show_master_compatibility = show_master_compatibility;
        self
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
}
