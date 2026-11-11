//! Pointer interaction dispatch for `SPEC-005` (`glifo v0.4.0`) tools:
//! `Knife`, `Primitives`, `Measurement`, `Annotation`, `Pencil`, and `Pixel`.

use crate::adapters::state::{CanvasTool, GlifoCanvasState};
use crate::adapters::v040_tools::{
    AnnotationDragMode, KnifeDragState, MeasurementDragState, PencilDragState, PixelDragState,
    PrimitiveDragState,
};
use crate::core::{
    bresenham_pixel_cells, build_rectangle_contour_nodes, compute_knife_intersections,
    constrain_knife_endpoint, find_pixel_cell_contour_index, fit_freehand_contour_nodes,
    hit_test_annotation_body, hit_test_annotation_handle, knife_add_nodes_in_layer,
    knife_cut_paths_in_layer, pixel_cell_bounds_upm, snap_to_pixel_cell, AnnotationId,
    CanvasAnnotation, ContourId, FontPoint, GlyphContour, ScreenPoint, ToolDialogRequest,
};
use crate::interaction::Modifiers;
use crate::snapping::SnapEngine;
use crate::tokens::{
    KNIFE_MIN_DRAG_UPM, MEASUREMENT_SNAP_RADIUS_DP, MIN_VIEWPORT_ZOOM, PENCIL_CLOSE_RADIUS_DP,
    PENCIL_MIN_SAMPLE_DIST_DP, PRIMITIVE_DEFAULT_DIALOG_HEIGHT_UPM,
    PRIMITIVE_DEFAULT_DIALOG_WIDTH_UPM, PRIMITIVE_MIN_SIZE_UPM,
};

impl GlifoCanvasState {
    #[must_use]
    pub const fn is_v040_tool(&self) -> bool {
        matches!(
            self.active_tool,
            CanvasTool::Knife | CanvasTool::Primitives | CanvasTool::Measurement
                | CanvasTool::Annotation | CanvasTool::Pencil | CanvasTool::Pixel
        )
    }

    pub fn on_v040_pointer_press(&mut self, scr_pt: ScreenPoint, font_pt: FontPoint, mods: Modifiers) -> bool {
        match self.active_tool {
            CanvasTool::Knife => {
                self.v040.knife = KnifeDragState {
                    active: true, start_upm: font_pt, current_upm: font_pt,
                    intersections: Vec::new(), alt_add_nodes: mods.option,
                };
                true
            }
            CanvasTool::Primitives => {
                self.selected_nodes.clear();
                let start = self.snap_point_for_v040(font_pt, mods.control);
                self.v040.primitive_drag = PrimitiveDragState {
                    active: true, start_upm: start, current_upm: start, last_raw_upm: font_pt,
                    shift: mods.shift, alt: mods.option,
                    round_to_grid: self.round_to_grid, grid_step_upm: self.grid_step_upm,
                };
                true
            }
            CanvasTool::Measurement => {
                let start = self.try_snap_measurement_node(scr_pt).unwrap_or(font_pt);
                self.v040.measurement = MeasurementDragState {
                    active: true, start_upm: start, current_upm: start, intersections: vec![start],
                };
                true
            }
            CanvasTool::Annotation => self.on_annotation_press(scr_pt, font_pt, mods),
            CanvasTool::Pencil => {
                self.v040.pencil = PencilDragState { active: true, points_upm: vec![font_pt] };
                true
            }
            CanvasTool::Pixel => self.on_pixel_press(font_pt, mods),
            _ => false,
        }
    }

    pub fn on_v040_pointer_move(&mut self, scr_pt: ScreenPoint, font_pt: FontPoint, mods: Modifiers) -> bool {
        match self.active_tool {
            CanvasTool::Knife if self.v040.knife.active => {
                let end = constrain_knife_endpoint(self.v040.knife.start_upm, font_pt, mods.shift);
                self.v040.knife.current_upm = end;
                self.v040.knife.alt_add_nodes = mods.option;
                let closed_flags = self.closed_flags();
                self.v040.knife.intersections = compute_knife_intersections(
                    &self.contours, &closed_flags, self.v040.knife.start_upm, end,
                );
                true
            }
            CanvasTool::Primitives if self.v040.primitive_drag.active => {
                if self.prev_view_mode.is_some() {
                    let dx = font_pt.x.0 - self.v040.primitive_drag.last_raw_upm.x.0;
                    let dy = font_pt.y.0 - self.v040.primitive_drag.last_raw_upm.y.0;
                    self.v040.primitive_drag.start_upm = FontPoint::new(
                        self.v040.primitive_drag.start_upm.x.0 + dx,
                        self.v040.primitive_drag.start_upm.y.0 + dy,
                    );
                }
                self.v040.primitive_drag.last_raw_upm = font_pt;
                self.v040.primitive_drag.current_upm = self.snap_point_for_v040(font_pt, mods.control);
                self.v040.primitive_drag.shift = mods.shift;
                self.v040.primitive_drag.alt = mods.option;
                self.v040.primitive_drag.round_to_grid = self.round_to_grid;
                self.v040.primitive_drag.grid_step_upm = self.grid_step_upm;
                true
            }
            CanvasTool::Measurement if self.v040.measurement.active => {
                self.v040.measurement.current_upm = self.try_snap_measurement_node(scr_pt).unwrap_or_else(|| {
                    constrain_knife_endpoint(self.v040.measurement.start_upm, font_pt, mods.shift)
                });
                self.recompute_measurement_intersections();
                true
            }
            CanvasTool::Annotation => self.on_annotation_move(font_pt, mods),
            CanvasTool::Pencil if self.v040.pencil.active => {
                let min_dist = f64::from(PENCIL_MIN_SAMPLE_DIST_DP) / f64::from(self.viewport.zoom.max(MIN_VIEWPORT_ZOOM));
                if self.v040.pencil.points_upm.last().is_none_or(|last| (font_pt.x.0 - last.x.0).hypot(font_pt.y.0 - last.y.0) >= min_dist) {
                    self.v040.pencil.points_upm.push(font_pt);
                }
                true
            }
            CanvasTool::Pixel if self.v040.pixel.active => self.on_pixel_move(font_pt),
            _ => false,
        }
    }

    pub fn on_v040_pointer_release(&mut self, font_pt: FontPoint, mods: Modifiers) -> bool {
        match self.active_tool {
            CanvasTool::Knife if self.v040.knife.active => self.finish_knife_drag(font_pt, mods),
            CanvasTool::Primitives if self.v040.primitive_drag.active => self.finish_primitive_drag(mods),
            CanvasTool::Measurement if self.v040.measurement.active => {
                self.v040.measurement.active = false;
                self.v040.measurement.intersections.clear();
                true
            }
            CanvasTool::Annotation => {
                self.v040.annotation_drag = AnnotationDragMode::Idle;
                if let Some(before) = self.active_drag_before_snapshot.take() {
                    self.record_mutation("Edit Annotation", before);
                }
                true
            }
            CanvasTool::Pencil if self.v040.pencil.active => self.finish_pencil_drag(font_pt),
            CanvasTool::Pixel if self.v040.pixel.active => {
                self.v040.pixel.active = false;
                self.v040.pixel.last_cell = None;
                if let Some(before) = self.active_drag_before_snapshot.take() {
                    self.record_mutation("Pixel Tool", before);
                }
                true
            }
            _ => false,
        }
    }

    fn snap_point_for_v040(&mut self, raw: FontPoint, disable_snap: bool) -> FontPoint {
        let res = SnapEngine::snap_point(
            raw, &self.contours, &[], &self.metrics, self.glyph.width, self.italic_angle_deg,
            self.show_guides, &self.guides, &self.global_guides, self.viewport.zoom, disable_snap,
        );
        self.active_snappers = res.active_snappers;
        res.snapped_point
    }

    fn try_snap_measurement_node(&self, scr_pt: ScreenPoint) -> Option<FontPoint> {
        let hit_r = f64::from(MEASUREMENT_SNAP_RADIUS_DP);
        for node in self.contours.iter().flatten() {
            let n_scr = self.viewport.to_screen(node.point);
            if f64::from(scr_pt.x.0 - n_scr.x.0).hypot(f64::from(scr_pt.y.0 - n_scr.y.0)) <= hit_r {
                return Some(node.point);
            }
        }
        None
    }

    fn finish_knife_drag(&mut self, font_pt: FontPoint, mods: Modifiers) -> bool {
        let start = self.v040.knife.start_upm;
        let end = constrain_knife_endpoint(start, font_pt, mods.shift);
        let alt = mods.option || self.v040.knife.alt_add_nodes;
        self.v040.knife = KnifeDragState::default();
        if (end.x.0 - start.x.0).hypot(end.y.0 - start.y.0) <= KNIFE_MIN_DRAG_UPM { return false; }
        let before = self.snapshot();
        if alt {
            let closed_flags = self.closed_flags();
            let added = knife_add_nodes_in_layer(&mut self.contours, &closed_flags, start, end, &mut self.next_node_id);
            if !added.is_empty() {
                self.selected_nodes = added;
                self.rebuild_glyph_contours(&closed_flags);
                self.sync_paths_from_contours();
                self.record_mutation("Add Nodes with Knife", before);
                self.assert_invariants();
                return true;
            }
            false
        } else {
            let mut closed_flags = self.closed_flags();
            if knife_cut_paths_in_layer(&mut self.contours, &mut closed_flags, start, end, &mut self.next_node_id) {
                self.selected_nodes.clear();
                self.rebuild_glyph_contours(&closed_flags);
                self.sync_paths_from_contours();
                self.record_mutation("Cut Paths", before);
                self.assert_invariants();
                return true;
            }
            false
        }
    }

    fn finish_primitive_drag(&mut self, mods: Modifiers) -> bool {
        self.v040.primitive_drag.shift = mods.shift;
        self.v040.primitive_drag.alt = mods.option;
        let (min_x, min_y, max_x, max_y) = self.v040.primitive_drag.preview_rect_upm();
        let start_upm = self.v040.primitive_drag.start_upm;
        self.v040.primitive_drag = PrimitiveDragState::default();
        self.active_snappers.clear();
        let (w, h) = (max_x - min_x, max_y - min_y);
        if w >= PRIMITIVE_MIN_SIZE_UPM && h >= PRIMITIVE_MIN_SIZE_UPM {
            self.insert_primitive(self.v040.primitive_sub_tool, FontPoint::new(min_x, min_y), w, h)
        } else {
            self.v040.last_dialog_request = Some(ToolDialogRequest::Primitive {
                position_upm: start_upm, sub_tool: self.v040.primitive_sub_tool,
                default_width: PRIMITIVE_DEFAULT_DIALOG_WIDTH_UPM, default_height: PRIMITIVE_DEFAULT_DIALOG_HEIGHT_UPM,
            });
            true
        }
    }

    fn on_annotation_press(&mut self, scr_pt: ScreenPoint, font_pt: FontPoint, mods: Modifiers) -> bool {
        let before = self.snapshot();
        if let Some(id) = hit_test_annotation_handle(&self.glyph.annotations, &self.viewport, scr_pt) {
            self.select_annotation_id(id, mods.shift);
            if let Some(ann) = self.glyph.annotations.iter().find(|a| a.id == id) {
                self.active_drag_before_snapshot = Some(before);
                self.v040.annotation_drag = AnnotationDragMode::DragHandle { id, start_upm: font_pt, initial_kind: ann.kind.clone() };
            }
            return true;
        }
        if let Some(id) = hit_test_annotation_body(&self.glyph.annotations, &self.viewport, scr_pt) {
            self.select_annotation_id(id, mods.shift);
            let initial_positions = self.glyph.annotations.iter()
                .filter(|a| self.v040.selected_annotations.contains(&a.id))
                .map(|a| (a.id, a.position)).collect();
            self.active_drag_before_snapshot = Some(before);
            self.v040.annotation_drag = AnnotationDragMode::MoveBody { start_upm: font_pt, initial_positions };
            return true;
        }
        if !mods.shift { self.v040.selected_annotations.clear(); }
        let id = AnnotationId(self.v040.next_annotation_id);
        self.v040.next_annotation_id += 1;
        let ann = CanvasAnnotation::new_for_sub_tool(id, font_pt, self.v040.annotation_sub_tool);
        self.glyph.annotations.push(ann);
        self.v040.selected_annotations.push(id);
        self.active_drag_before_snapshot = Some(before);
        self.v040.annotation_drag = AnnotationDragMode::MoveBody { start_upm: font_pt, initial_positions: vec![(id, font_pt)] };
        true
    }

    fn select_annotation_id(&mut self, id: AnnotationId, shift: bool) {
        if !shift && !self.v040.selected_annotations.contains(&id) {
            self.v040.selected_annotations = vec![id];
        } else if !self.v040.selected_annotations.contains(&id) {
            self.v040.selected_annotations.push(id);
        }
    }

    fn on_annotation_move(&mut self, font_pt: FontPoint, mods: Modifiers) -> bool {
        match self.v040.annotation_drag.clone() {
            AnnotationDragMode::Idle => false,
            AnnotationDragMode::MoveBody { start_upm, initial_positions } => {
                let target = constrain_knife_endpoint(start_upm, font_pt, mods.shift);
                let (dx, dy) = (target.x.0 - start_upm.x.0, target.y.0 - start_upm.y.0);
                for (id, init_pos) in initial_positions {
                    if let Some(ann) = self.glyph.annotations.iter_mut().find(|a| a.id == id) {
                        ann.position = FontPoint::new(init_pos.x.0 + dx, init_pos.y.0 + dy);
                    }
                }
                true
            }
            AnnotationDragMode::DragHandle { id, start_upm, initial_kind } => {
                if let Some(ann) = self.glyph.annotations.iter_mut().find(|a| a.id == id) {
                    ann.apply_handle_drag(&initial_kind, font_pt, start_upm);
                }
                true
            }
        }
    }

    fn finish_pencil_drag(&mut self, font_pt: FontPoint) -> bool {
        if self.v040.pencil.points_upm.last().is_none_or(|last| (font_pt.x.0 - last.x.0).hypot(font_pt.y.0 - last.y.0) > 0.5) {
            self.v040.pencil.points_upm.push(font_pt);
        }
        let pts = std::mem::take(&mut self.v040.pencil.points_upm);
        self.v040.pencil.active = false;
        if pts.len() < 2 { return false; }
        let close_tol = f64::from(PENCIL_CLOSE_RADIUS_DP) / f64::from(self.viewport.zoom.max(MIN_VIEWPORT_ZOOM));
        let (first, last) = (pts[0], *pts.last().unwrap());
        let closed = pts.len() >= 3 && (first.x.0 - last.x.0).hypot(first.y.0 - last.y.0) <= close_tol;
        let before = self.snapshot();
        let nodes = fit_freehand_contour_nodes(&pts, closed, &mut self.next_node_id);
        if nodes.is_empty() { return false; }
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

    fn on_pixel_press(&mut self, font_pt: FontPoint, mods: Modifiers) -> bool {
        let step = self.effective_pixel_step_upm();
        let cell = snap_to_pixel_cell(font_pt, step);
        self.active_drag_before_snapshot = Some(self.snapshot());
        let exists = find_pixel_cell_contour_index(&self.contours, &self.closed_flags(), cell, step).is_some();
        self.v040.pixel = PixelDragState { active: true, erase_mode: exists, last_cell: Some(cell) };
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

    fn on_pixel_move(&mut self, font_pt: FontPoint) -> bool {
        let step = self.effective_pixel_step_upm();
        let cell = snap_to_pixel_cell(font_pt, step);
        if self.v040.pixel.last_cell == Some(cell) { return false; }
        self.v040.pixel.last_cell = Some(cell);
        self.v040.last_pixel_cell = Some(cell);
        self.apply_pixel_cell(cell, step, self.v040.pixel.erase_mode);
        true
    }

    fn apply_pixel_cell(&mut self, cell: (i64, i64), step: f64, erase: bool) {
        let existing = find_pixel_cell_contour_index(&self.contours, &self.closed_flags(), cell, step);
        if erase {
            if let Some(idx) = existing {
                self.contours.remove(idx);
                if idx < self.glyph.contours.len() { self.glyph.contours.remove(idx); }
                self.sync_paths_from_contours();
                self.assert_invariants();
            }
        } else if existing.is_none() {
            let (min_x, min_y, max_x, max_y) = pixel_cell_bounds_upm(cell, step);
            let nodes = build_rectangle_contour_nodes(min_x, min_y, max_x, max_y, &mut self.next_node_id);
            let c_idx = self.contours.len();
            self.contours.push(nodes.clone());
            self.glyph.contours.push(GlyphContour::new(ContourId(c_idx), nodes));
            self.active_contour_closed = true;
            self.sync_paths_from_contours();
            self.assert_invariants();
        }
    }

    fn rebuild_glyph_contours(&mut self, closed_flags: &[bool]) {
        self.glyph.contours = self.contours.iter().enumerate().map(|(idx, nodes)| {
            let mut gc = GlyphContour::new(ContourId(idx), nodes.clone());
            gc.closed = closed_flags.get(idx).copied().unwrap_or(true);
            gc
        }).collect();
        self.active_contour_closed = closed_flags.last().copied().unwrap_or(true);
    }
}
