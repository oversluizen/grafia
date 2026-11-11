//! Pointer handlers for `SPEC-005` (`grafia v0.4.0`) `Annotation`, `Pencil`, and `Pixel` tools.

use crate::adapters::state::GrafiaCanvasState;
use crate::adapters::v040_tools::{AnnotationDragMode, PixelDragState};
use crate::core::{
    bresenham_pixel_cells, build_rectangle_contour_nodes, constrain_knife_endpoint,
    find_pixel_cell_contour_index, fit_freehand_contour_nodes, hit_test_annotation_body,
    hit_test_annotation_handle, pixel_cell_bounds_upm, snap_to_pixel_cell, AnnotationId,
    CanvasAnnotation, ContourId, FontPoint, GlyphContour, ScreenPoint,
};
use crate::interaction::Modifiers;
use crate::tokens::{
    ANNOTATION_PASTE_OFFSET_UPM, MIN_VIEWPORT_ZOOM, PENCIL_CLOSE_RADIUS_DP,
    PIXEL_CUSTOM_GRID_MIN_UPM,
};

impl GrafiaCanvasState {
    /// Selects all `CanvasAnnotation` items on the active glyph (`AnnotationTool::selectAll:` at `spec_ref`).
    pub fn select_all_annotations(&mut self) {
        self.v040.selected_annotations = self.glyph.annotations.iter().map(|a| a.id).collect();
    }

    /// Deletes all currently selected `CanvasAnnotation` items (`AnnotationTool::deleteBackward:` at `spec_ref`).
    pub fn delete_selected_annotations(&mut self) -> bool {
        if self.v040.selected_annotations.is_empty() {
            return false;
        }
        let before = self.snapshot();
        self.glyph
            .annotations
            .retain(|a| !self.v040.selected_annotations.contains(&a.id));
        self.v040.selected_annotations.clear();
        self.record_mutation("Delete Annotations", before);
        true
    }

    /// Copies all selected `CanvasAnnotation` items to the annotation clipboard (`Cmd/Ctrl + C`).
    pub fn copy_selected_annotations(&mut self) -> bool {
        let copied: Vec<CanvasAnnotation> = self
            .glyph
            .annotations
            .iter()
            .filter(|a| self.v040.selected_annotations.contains(&a.id))
            .cloned()
            .collect();
        let ok = !copied.is_empty();
        if ok {
            self.v040.clipboard_annotations = copied;
        }
        ok
    }

    /// Cuts all selected `CanvasAnnotation` items to the annotation clipboard (`Cmd/Ctrl + X`).
    pub fn cut_selected_annotations(&mut self) -> bool {
        self.copy_selected_annotations() && self.delete_selected_annotations()
    }

    /// Pastes `clipboard_annotations` into the active glyph with `(+10, +10)` UPM offset (`Cmd/Ctrl + V`).
    pub fn paste_clipboard_annotations(&mut self) -> bool {
        if self.v040.clipboard_annotations.is_empty() {
            return false;
        }
        let before = self.snapshot();
        let mut new_selected = Vec::with_capacity(self.v040.clipboard_annotations.len());
        for clip_ann in &mut self.v040.clipboard_annotations {
            clip_ann.position = FontPoint::new(
                clip_ann.position.x.0 + ANNOTATION_PASTE_OFFSET_UPM,
                clip_ann.position.y.0 + ANNOTATION_PASTE_OFFSET_UPM,
            );
            let id = AnnotationId(self.v040.next_annotation_id);
            self.v040.next_annotation_id += 1;
            let mut pasted = clip_ann.clone();
            pasted.id = id;
            self.glyph.annotations.push(pasted);
            new_selected.push(id);
        }
        self.v040.selected_annotations = new_selected;
        self.record_mutation("Paste Annotations", before);
        true
    }

    /// Returns the effective grid step in UPM for `PixelTool` (`X`).
    #[must_use]
    pub fn effective_pixel_step_upm(&self) -> f64 {
        if self.grid_step_upm >= PIXEL_CUSTOM_GRID_MIN_UPM {
            self.grid_step_upm
        } else {
            self.v040.pixel_size_upm.max(1.0)
        }
    }
    pub(crate) fn on_annotation_press(
        &mut self,
        scr_pt: ScreenPoint,
        font_pt: FontPoint,
        mods: Modifiers,
    ) -> bool {
        let before = self.snapshot();
        if let Some(id) =
            hit_test_annotation_handle(&self.glyph.annotations, &self.viewport, scr_pt)
        {
            self.select_annotation_id(id, mods.shift);
            if let Some(ann) = self.glyph.annotations.iter().find(|a| a.id == id) {
                self.active_drag_before_snapshot = Some(before);
                self.v040.annotation_drag = AnnotationDragMode::DragHandle {
                    id,
                    start_upm: font_pt,
                    initial_kind: ann.kind.clone(),
                };
            }
            return true;
        }
        if let Some(id) = hit_test_annotation_body(&self.glyph.annotations, &self.viewport, scr_pt)
        {
            self.select_annotation_id(id, mods.shift);
            let initial_positions = self
                .glyph
                .annotations
                .iter()
                .filter(|a| self.v040.selected_annotations.contains(&a.id))
                .map(|a| (a.id, a.position))
                .collect();
            self.active_drag_before_snapshot = Some(before);
            self.v040.annotation_drag = AnnotationDragMode::MoveBody {
                start_upm: font_pt,
                initial_positions,
            };
            return true;
        }
        if !mods.shift {
            self.v040.selected_annotations.clear();
        }
        let id = AnnotationId(self.v040.next_annotation_id);
        self.v040.next_annotation_id += 1;
        let ann = CanvasAnnotation::new_for_sub_tool(id, font_pt, self.v040.annotation_sub_tool);
        self.glyph.annotations.push(ann);
        self.v040.selected_annotations.push(id);
        self.active_drag_before_snapshot = Some(before);
        self.v040.annotation_drag = AnnotationDragMode::MoveBody {
            start_upm: font_pt,
            initial_positions: vec![(id, font_pt)],
        };
        true
    }

    fn select_annotation_id(&mut self, id: AnnotationId, shift: bool) {
        if !shift && !self.v040.selected_annotations.contains(&id) {
            self.v040.selected_annotations = vec![id];
        } else if !self.v040.selected_annotations.contains(&id) {
            self.v040.selected_annotations.push(id);
        }
    }

    pub(crate) fn on_annotation_move(&mut self, font_pt: FontPoint, mods: Modifiers) -> bool {
        match self.v040.annotation_drag.clone() {
            AnnotationDragMode::Idle => false,
            AnnotationDragMode::MoveBody {
                start_upm,
                initial_positions,
            } => {
                let target = constrain_knife_endpoint(start_upm, font_pt, mods.shift);
                let (dx, dy) = (target.x.0 - start_upm.x.0, target.y.0 - start_upm.y.0);
                for (id, init_pos) in initial_positions {
                    if let Some(ann) = self.glyph.annotations.iter_mut().find(|a| a.id == id) {
                        ann.position = FontPoint::new(init_pos.x.0 + dx, init_pos.y.0 + dy);
                    }
                }
                true
            }
            AnnotationDragMode::DragHandle {
                id,
                start_upm,
                initial_kind,
            } => {
                if let Some(ann) = self.glyph.annotations.iter_mut().find(|a| a.id == id) {
                    ann.apply_handle_drag(&initial_kind, font_pt, start_upm);
                }
                true
            }
        }
    }

    pub(crate) fn finish_pencil_drag(&mut self, font_pt: FontPoint) -> bool {
        if self
            .v040
            .pencil
            .points_upm
            .last()
            .is_none_or(|last| (font_pt.x.0 - last.x.0).hypot(font_pt.y.0 - last.y.0) > 0.5)
        {
            self.v040.pencil.points_upm.push(font_pt);
        }
        let pts = std::mem::take(&mut self.v040.pencil.points_upm);
        self.v040.pencil.active = false;
        if pts.len() < 2 {
            return false;
        }
        let close_tol = f64::from(PENCIL_CLOSE_RADIUS_DP)
            / f64::from(self.viewport.zoom.max(MIN_VIEWPORT_ZOOM));
        let (first, last) = (pts[0], *pts.last().unwrap());
        let closed =
            pts.len() >= 3 && (first.x.0 - last.x.0).hypot(first.y.0 - last.y.0) <= close_tol;
        let before = self.snapshot();
        let nodes = fit_freehand_contour_nodes(&pts, closed, &mut self.next_node_id);
        if nodes.is_empty() {
            return false;
        }
        self.selected_nodes = nodes.iter().map(|n| n.id).collect();
        let c_idx = self.contours.len();
        self.contours.push(nodes.clone());
        let mut gc = GlyphContour::new(ContourId(c_idx), nodes);
        gc.closed = closed;
        self.glyph.contours.push(gc);
        self.active_contour_closed = closed;
        self.sync_paths_from_contours();
        self.record_mutation("Draw Freehand Path", before);
        self.assert_invariants();
        true
    }

    pub(crate) fn on_pixel_press(&mut self, font_pt: FontPoint, mods: Modifiers) -> bool {
        let step = self.effective_pixel_step_upm();
        let cell = snap_to_pixel_cell(font_pt, step);
        self.active_drag_before_snapshot = Some(self.snapshot());
        let exists =
            find_pixel_cell_contour_index(&self.contours, &self.closed_flags(), cell, step)
                .is_some();
        self.v040.pixel = PixelDragState {
            active: true,
            erase_mode: exists,
            last_cell: Some(cell),
        };
        if mods.shift {
            if let Some(prev_cell) = self.v040.last_pixel_cell {
                for c in bresenham_pixel_cells(prev_cell, cell) {
                    self.apply_pixel_cell(c, step, exists);
                }
                self.v040.last_pixel_cell = Some(cell);
                return true;
            }
        }
        self.apply_pixel_cell(cell, step, exists);
        self.v040.last_pixel_cell = Some(cell);
        true
    }

    pub(crate) fn on_pixel_move(&mut self, font_pt: FontPoint) -> bool {
        let step = self.effective_pixel_step_upm();
        let cell = snap_to_pixel_cell(font_pt, step);
        if self.v040.pixel.last_cell == Some(cell) {
            return false;
        }
        self.v040.pixel.last_cell = Some(cell);
        self.v040.last_pixel_cell = Some(cell);
        self.apply_pixel_cell(cell, step, self.v040.pixel.erase_mode);
        true
    }

    fn apply_pixel_cell(&mut self, cell: (i64, i64), step: f64, erase: bool) {
        let existing =
            find_pixel_cell_contour_index(&self.contours, &self.closed_flags(), cell, step);
        if erase {
            if let Some(idx) = existing {
                self.contours.remove(idx);
                if idx < self.glyph.contours.len() {
                    self.glyph.contours.remove(idx);
                }
                self.sync_paths_from_contours();
                self.assert_invariants();
            }
        } else if existing.is_none() {
            let (min_x, min_y, max_x, max_y) = pixel_cell_bounds_upm(cell, step);
            let nodes =
                build_rectangle_contour_nodes(min_x, min_y, max_x, max_y, &mut self.next_node_id);
            let c_idx = self.contours.len();
            self.contours.push(nodes.clone());
            self.glyph
                .contours
                .push(GlyphContour::new(ContourId(c_idx), nodes));
            self.active_contour_closed = true;
            self.sync_paths_from_contours();
            self.assert_invariants();
        }
    }

    pub(crate) fn rebuild_glyph_contours(&mut self, closed_flags: &[bool]) {
        self.glyph.contours = self
            .contours
            .iter()
            .enumerate()
            .map(|(idx, nodes)| {
                let mut gc = GlyphContour::new(ContourId(idx), nodes.clone());
                gc.closed = closed_flags.get(idx).copied().unwrap_or(true);
                gc
            })
            .collect();
        self.active_contour_closed = closed_flags.last().copied().unwrap_or(true);
    }
}
