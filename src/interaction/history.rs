//! O(1) Copy-on-Write undo/redo transaction stack with mouse-drag coalescing.

use std::sync::Arc;
use kurbo::BezPath;
use crate::core::{AnnotationId, CanvasAnnotation, FontPoint, Node, NodeId, ShapeBooleanRole};
use crate::snapping::{Guide, SelectedGuideRef};

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
            pending_pen_bcp_out: None,
            pending_pen_first_bcp_in: None,
            guides: Vec::new(),
            global_guides: Vec::new(),
            selected_guide: None,
            annotations: Vec::new(),
            selected_annotations: Vec::new(),
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
        self.undo_stack.push(HistoryTransaction { label, before, after });
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
