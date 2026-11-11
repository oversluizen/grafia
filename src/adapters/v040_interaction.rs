//! Pointer interaction dispatch for `SPEC-005` (`grafia v0.4.0`) tools:
//! `Knife`, `Primitives`, `Measurement`, `Annotation`, `Pencil`, and `Pixel`.

use crate::adapters::state::{CanvasTool, GrafiaCanvasState};
use crate::adapters::v040_tools::{
    AnnotationDragMode, KnifeDragState, MeasurementDragState, PencilDragState, PrimitiveDragState,
};
use crate::core::{
    compute_knife_intersections, constrain_knife_endpoint, knife_add_nodes_in_layer,
    knife_cut_paths_in_layer, FontPoint, ScreenPoint, ToolDialogRequest,
};
use crate::interaction::Modifiers;
use crate::snapping::SnapEngine;
use crate::tokens::{
    KNIFE_MIN_DRAG_UPM, MEASUREMENT_SNAP_RADIUS_DP, MIN_VIEWPORT_ZOOM, PENCIL_MIN_SAMPLE_DIST_DP,
    PRIMITIVE_DEFAULT_DIALOG_HEIGHT_UPM, PRIMITIVE_DEFAULT_DIALOG_WIDTH_UPM,
    PRIMITIVE_MIN_SIZE_UPM,
};

impl GrafiaCanvasState {
    #[must_use]
    pub const fn is_v040_tool(&self) -> bool {
        matches!(
            self.active_tool,
            CanvasTool::Knife
                | CanvasTool::Primitives
                | CanvasTool::Measurement
                | CanvasTool::Annotation
                | CanvasTool::Pencil
                | CanvasTool::Pixel
        )
    }

    pub fn on_v040_pointer_press(
        &mut self,
        scr_pt: ScreenPoint,
        font_pt: FontPoint,
        mods: Modifiers,
    ) -> bool {
        match self.active_tool {
            CanvasTool::Knife => {
                self.v040.knife = KnifeDragState {
                    active: true,
                    start_upm: font_pt,
                    current_upm: font_pt,
                    intersections: Vec::new(),
                    alt_add_nodes: mods.option,
                };
                true
            }
            CanvasTool::Primitives => {
                self.selected_nodes.clear();
                let start = self.snap_point_for_v040(font_pt, mods.control);
                self.v040.primitive_drag = PrimitiveDragState {
                    active: true,
                    start_upm: start,
                    current_upm: start,
                    last_raw_upm: font_pt,
                    shift: mods.shift,
                    alt: mods.option,
                    round_to_grid: self.round_to_grid,
                    grid_step_upm: self.grid_step_upm,
                };
                true
            }
            CanvasTool::Measurement => {
                let start = self.try_snap_measurement_node(scr_pt).unwrap_or(font_pt);
                self.v040.measurement = MeasurementDragState {
                    active: true,
                    start_upm: start,
                    current_upm: start,
                    intersections: vec![start],
                };
                true
            }
            CanvasTool::Annotation => self.on_annotation_press(scr_pt, font_pt, mods),
            CanvasTool::Pencil => {
                self.v040.pencil = PencilDragState {
                    active: true,
                    points_upm: vec![font_pt],
                };
                true
            }
            CanvasTool::Pixel => self.on_pixel_press(font_pt, mods),
            _ => false,
        }
    }

    pub fn on_v040_pointer_move(
        &mut self,
        scr_pt: ScreenPoint,
        font_pt: FontPoint,
        mods: Modifiers,
    ) -> bool {
        match self.active_tool {
            CanvasTool::Knife if self.v040.knife.active => {
                let end = constrain_knife_endpoint(self.v040.knife.start_upm, font_pt, mods.shift);
                self.v040.knife.current_upm = end;
                self.v040.knife.alt_add_nodes = mods.option;
                let closed_flags = self.closed_flags();
                self.v040.knife.intersections = compute_knife_intersections(
                    &self.contours,
                    &closed_flags,
                    self.v040.knife.start_upm,
                    end,
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
                self.v040.primitive_drag.current_upm =
                    self.snap_point_for_v040(font_pt, mods.control);
                self.v040.primitive_drag.shift = mods.shift;
                self.v040.primitive_drag.alt = mods.option;
                self.v040.primitive_drag.round_to_grid = self.round_to_grid;
                self.v040.primitive_drag.grid_step_upm = self.grid_step_upm;
                true
            }
            CanvasTool::Measurement if self.v040.measurement.active => {
                self.v040.measurement.current_upm =
                    self.try_snap_measurement_node(scr_pt).unwrap_or_else(|| {
                        constrain_knife_endpoint(
                            self.v040.measurement.start_upm,
                            font_pt,
                            mods.shift,
                        )
                    });
                self.recompute_measurement_intersections();
                true
            }
            CanvasTool::Annotation => self.on_annotation_move(font_pt, mods),
            CanvasTool::Pencil if self.v040.pencil.active => {
                let min_dist = f64::from(PENCIL_MIN_SAMPLE_DIST_DP)
                    / f64::from(self.viewport.zoom.max(MIN_VIEWPORT_ZOOM));
                if self.v040.pencil.points_upm.last().is_none_or(|last| {
                    (font_pt.x.0 - last.x.0).hypot(font_pt.y.0 - last.y.0) >= min_dist
                }) {
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
            CanvasTool::Primitives if self.v040.primitive_drag.active => {
                self.finish_primitive_drag(mods)
            }
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
            raw,
            &self.contours,
            &[],
            &self.metrics,
            self.glyph.width,
            self.italic_angle_deg,
            self.show_guides,
            &self.guides,
            &self.global_guides,
            self.viewport.zoom,
            disable_snap,
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
        if (end.x.0 - start.x.0).hypot(end.y.0 - start.y.0) <= KNIFE_MIN_DRAG_UPM {
            return false;
        }
        let before = self.snapshot();
        if alt {
            let closed_flags = self.closed_flags();
            let added = knife_add_nodes_in_layer(
                &mut self.contours,
                &closed_flags,
                start,
                end,
                &mut self.next_node_id,
            );
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
            if knife_cut_paths_in_layer(
                &mut self.contours,
                &mut closed_flags,
                start,
                end,
                &mut self.next_node_id,
            ) {
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
            self.insert_primitive(
                self.v040.primitive_sub_tool,
                FontPoint::new(min_x, min_y),
                w,
                h,
            )
        } else {
            self.v040.last_dialog_request = Some(ToolDialogRequest::Primitive {
                position_upm: start_upm,
                sub_tool: self.v040.primitive_sub_tool,
                default_width: PRIMITIVE_DEFAULT_DIALOG_WIDTH_UPM,
                default_height: PRIMITIVE_DEFAULT_DIALOG_HEIGHT_UPM,
            });
            true
        }
    }
}
