//! `SPEC-003` (`v0.2.0`) `GrafiaCanvasState` methods for 8-Knob Bounding Box, `RotateTool`,
//! `ScaleTool`, `LassoTool`, `ZoomTool`, Clipboard, Path Operations, and `TransformPanel`.

use crate::adapters::state::{CanvasTool, GrafiaCanvasState};
use crate::core::{
    add_extremes_to_contours, add_inflection_points_to_contours, align_selected_nodes,
    close_open_contours_in_layer, compute_layer_bounds, compute_selection_bounds,
    connect_selected_open_endpoints_in_contours, correct_path_direction_layer,
    cut_open_delete_nodes_in_contour, extract_clipboard_contours, hit_test_bbox_knobs,
    make_node_first_v020, paste_clipboard_into_contours, tidy_up_contours, BBoxAction, FontPoint,
    GlyphContour, MetricsOrigin, NodeId, NodeType, ScreenPoint, SelectionBounds,
    TransformGridAnchor, TransformOriginSpec,
};
use crate::interaction::{PenToolState, SelectToolState};
use crate::tokens::{DEFAULT_ASCENDER_UPM, DEFAULT_CAP_HEIGHT_UPM, DEFAULT_X_HEIGHT_UPM};

#[derive(Debug, Clone, PartialEq, Default)]
pub enum V020DragState {
    #[default]
    Idle,
    BBoxDragging {
        action: BBoxAction,
        start_font: FontPoint,
        current_font: FontPoint,
        shadow_bounds: SelectionBounds,
    },
    TransformOriginDragging {
        start_scr: ScreenPoint,
    },
    TransformPendingOrDragging {
        is_rotate: bool,
        start_scr: ScreenPoint,
        current_scr: ScreenPoint,
        start_font: FontPoint,
        current_font: FontPoint,
        active: bool,
    },
    LassoDragging {
        points_scr: Vec<ScreenPoint>,
        points_font: Vec<FontPoint>,
        initial_selected: Vec<NodeId>,
    },
    ZoomDragging {
        start_scr: ScreenPoint,
        current_scr: ScreenPoint,
    },
}

impl GrafiaCanvasState {
    #[must_use]
    pub fn effective_grid_step(&self) -> f64 {
        let subdiv = self.grid_subdivisions.max(1) as f64;
        if self.round_to_grid || self.grid_subdivisions >= crate::tokens::GRID_MIN_SUBDIVISIONS {
            self.grid_step_upm / subdiv
        } else {
            0.0
        }
    }

    #[must_use]
    pub fn selection_bounds(&self) -> Option<SelectionBounds> {
        let angle = if self.show_bounding_box_slanted {
            if self.selection_angle_deg.abs() > 0.0 {
                self.selection_angle_deg
            } else {
                self.italic_angle_deg
            }
        } else {
            0.0
        };
        compute_selection_bounds(
            &self.contours,
            &self.selected_nodes,
            angle,
            DEFAULT_X_HEIGHT_UPM * 0.5,
        )
    }

    #[must_use]
    pub fn layer_bounds(&self) -> Option<SelectionBounds> {
        let angle = if self.show_bounding_box_slanted {
            self.italic_angle_deg
        } else {
            0.0
        };
        compute_layer_bounds(&self.contours, angle, DEFAULT_X_HEIGHT_UPM * 0.5)
    }

    #[must_use]
    pub fn active_bbox_knobs(&self) -> Vec<u8> {
        if !self.show_bounding_box {
            return Vec::new();
        }
        self.selection_bounds()
            .map_or_else(Vec::new, |b| b.active_knobs(self.viewport.zoom))
    }

    #[must_use]
    pub fn bounding_box_pill_labels(&self) -> Option<(String, String)> {
        if !self.show_bounding_box {
            return None;
        }
        let b = self.selection_bounds()?;
        if !b.is_displayable(self.viewport.zoom) {
            return None;
        }
        Some((
            SelectionBounds::format_dimension_badge(b.width()),
            SelectionBounds::format_dimension_badge(b.height()),
        ))
    }

    #[must_use]
    pub fn hit_test_bbox_knob(
        &self,
        cursor_scr: ScreenPoint,
        is_ctrl: bool,
        is_alt: bool,
    ) -> Option<BBoxAction> {
        if !self.show_bounding_box {
            return None;
        }
        let b = self.selection_bounds()?;
        hit_test_bbox_knobs(&b, &self.viewport, cursor_scr, is_ctrl, is_alt)
    }

    pub fn activate_tool(&mut self, tool: CanvasTool) {
        if tool == CanvasTool::Text && self.active_tool != CanvasTool::Text {
            self.last_select_tool = self.active_tool;
        }
        let was_transform = matches!(self.active_tool, CanvasTool::Rotate | CanvasTool::Scale);
        let is_transform = matches!(tool, CanvasTool::Rotate | CanvasTool::Scale);
        self.active_tool = tool;
        self.select_state = SelectToolState::Idle;
        self.pen_state = PenToolState::Idle;
        self.v020_drag = V020DragState::Idle;
        self.v040.reset_in_flight_drags();
        self.text_drag_anchor_caret = None;
        self.text_measurement_dragging = false;
        if is_transform && !was_transform {
            self.transform_origin_custom = false;
            self.update_transform_origin();
        }
    }

    pub fn update_transform_origin(&mut self) {
        self.transform_origin = self.resolve_transform_origin();
    }

    #[must_use]
    pub fn resolve_transform_origin(&self) -> FontPoint {
        if self.transform_origin_custom {
            return self.transform_origin;
        }
        match self.transform_origin_mode {
            TransformOriginSpec::Custom(pt) => pt,
            TransformOriginSpec::MetricsOrigin(m) => {
                let adv_w = if self.glyph.width > 0.0 {
                    self.glyph.width
                } else {
                    DEFAULT_X_HEIGHT_UPM
                };
                let cap_h = if self.glyph.height > 0.0 {
                    self.glyph.height
                } else {
                    DEFAULT_CAP_HEIGHT_UPM
                };
                m.resolve_pivot(
                    adv_w,
                    DEFAULT_ASCENDER_UPM,
                    cap_h,
                    DEFAULT_X_HEIGHT_UPM,
                    self.italic_angle_deg,
                )
            }
            TransformOriginSpec::BoundingBoxCenter | TransformOriginSpec::BoundingBoxOrigin(_) => {
                let anchor = match self.transform_origin_mode {
                    TransformOriginSpec::BoundingBoxOrigin(a) => a,
                    _ => self.transform_grid_anchor,
                };
                let ref_bounds = if !self.selected_nodes.is_empty() {
                    self.selection_bounds()
                } else {
                    self.layer_bounds()
                };
                ref_bounds.map_or(FontPoint::new(0.0, 0.0), |b| {
                    b.aligned_point(anchor.index())
                })
            }
        }
    }

    pub fn set_transform_grid_anchor(&mut self, anchor: TransformGridAnchor) {
        self.transform_grid_anchor = anchor;
        self.transform_origin_mode = TransformOriginSpec::BoundingBoxOrigin(anchor);
        self.transform_origin_custom = false;
        self.update_transform_origin();
    }

    pub fn set_metrics_origin(&mut self, origin: MetricsOrigin) {
        self.transform_origin_mode = TransformOriginSpec::MetricsOrigin(origin);
        self.transform_origin_custom = false;
        self.update_transform_origin();
    }

    pub(crate) fn sync_contours_and_closed_flags(&mut self, flags: &[bool]) {
        self.glyph.contours = self
            .contours
            .iter()
            .enumerate()
            .map(|(idx, nodes)| {
                let mut gc = GlyphContour::new(crate::core::ContourId(idx), nodes.clone());
                gc.closed = flags.get(idx).copied().unwrap_or(true);
                gc
            })
            .collect();
        self.active_contour_closed = flags.last().copied().unwrap_or(false);
        self.active_open_contour = flags.iter().rposition(|&c| !c);
        self.sync_paths_from_contours();
    }

    pub fn copy_selection(&mut self) -> bool {
        let extracted =
            extract_clipboard_contours(&self.contours, &self.closed_flags(), &self.selected_nodes);
        if extracted.is_empty() {
            return false;
        }
        self.clipboard_contours = extracted;
        true
    }

    pub fn paste_clipboard(&mut self) -> bool {
        if self.clipboard_contours.is_empty() {
            return false;
        }
        let before = self.snapshot();
        let mut flags = self.closed_flags();
        let mut clip = self.clipboard_contours.clone();
        if !paste_clipboard_into_contours(
            &mut self.contours,
            &mut flags,
            &mut self.selected_nodes,
            &mut clip,
            &mut self.next_node_id,
        ) {
            return false;
        }
        self.clipboard_contours = clip;
        self.sync_contours_and_closed_flags(&flags);
        self.record_mutation("Paste", before);
        self.assert_invariants();
        true
    }

    pub fn cut_selection(&mut self) -> bool {
        if !self.copy_selection() {
            return false;
        }
        let before = self.snapshot();
        let mut flags = self.closed_flags();
        for (contour, is_closed) in self.contours.iter_mut().zip(flags.iter_mut()) {
            if contour.iter().any(|n| self.selected_nodes.contains(&n.id)) {
                cut_open_delete_nodes_in_contour(contour, *is_closed, &self.selected_nodes);
                *is_closed = false;
            }
        }
        let mut kept_c = Vec::new();
        let mut kept_f = Vec::new();
        for (c, f) in self.contours.drain(..).zip(flags) {
            if !c.is_empty() {
                let on_c = c
                    .iter()
                    .filter(|n| n.node_type != NodeType::OffCurve)
                    .count();
                kept_c.push(c);
                kept_f.push(f && on_c > 1);
            }
        }
        self.contours = kept_c;
        self.selected_nodes.clear();
        self.sync_contours_and_closed_flags(&kept_f);
        self.record_mutation("Cut", before);
        self.assert_invariants();
        true
    }

    pub fn reverse_selected_contours(&mut self) -> bool {
        self.reverse_selected_or_all_contours()
    }

    pub fn make_node_first(&mut self) -> bool {
        if self.selected_nodes.is_empty() {
            return false;
        }
        let before = self.snapshot();
        let flags = self.closed_flags();
        let mut changed = false;
        for (c_idx, contour) in self.contours.iter_mut().enumerate() {
            if make_node_first_v020(contour, flags[c_idx], &self.selected_nodes) {
                changed = true;
            }
        }
        if changed {
            self.sync_paths_from_contours();
            self.record_mutation("Make Node First", before);
            self.assert_invariants();
        }
        changed
    }

    pub fn correct_path_direction(&mut self) -> bool {
        let before = self.snapshot();
        let flags = self.closed_flags();
        if !correct_path_direction_layer(
            &mut self.contours,
            &flags,
            self.italic_angle_deg,
            DEFAULT_X_HEIGHT_UPM * 0.5,
        ) {
            return false;
        }
        self.sync_paths_from_contours();
        self.record_mutation("Correct Path Direction", before);
        self.assert_invariants();
        true
    }

    fn retain_valid_selection(&mut self) {
        let valid_ids: std::collections::HashSet<NodeId> = self
            .contours
            .iter()
            .flat_map(|c| c.iter().map(|n| n.id))
            .collect();
        self.selected_nodes.retain(|id| valid_ids.contains(id));
    }

    fn apply_contour_op(
        &mut self,
        label: &'static str,
        sync_flags: bool,
        op: impl FnOnce(&mut Vec<Vec<crate::core::Node>>, &mut Vec<bool>, &[NodeId], &mut u64) -> bool,
    ) -> bool {
        let before = self.snapshot();
        let mut flags = self.closed_flags();
        if !op(
            &mut self.contours,
            &mut flags,
            &self.selected_nodes,
            &mut self.next_node_id,
        ) {
            return false;
        }
        self.retain_valid_selection();
        if sync_flags {
            self.sync_contours_and_closed_flags(&flags);
        } else {
            self.sync_paths_from_contours();
        }
        self.record_mutation(label, before);
        self.assert_invariants();
        true
    }

    pub fn add_extremes(&mut self, force: bool) -> bool {
        let grid = self.effective_grid_step();
        self.apply_contour_op("Add Extremes", true, |c, f, s, id| {
            add_extremes_to_contours(c, f, s, force, id, grid)
        })
    }

    pub fn add_extremes_action(&mut self) -> bool {
        self.add_extremes(false)
    }

    pub fn add_inflection_points(&mut self) -> bool {
        let grid = self.effective_grid_step();
        self.apply_contour_op("Add Inflection Points", false, |c, f, s, id| {
            add_inflection_points_to_contours(c, f, s, id, grid)
        })
    }

    pub fn tidy_up_paths(&mut self) -> bool {
        let step = self.grid_step_upm;
        self.apply_contour_op("Tidy Up Paths", true, |c, f, s, _| {
            tidy_up_contours(c, f, s, step)
        })
    }

    pub fn align_selection(&mut self) -> bool {
        let anchor = self.transform_grid_anchor;
        let grid = self.effective_grid_step();
        self.apply_contour_op("Align Selection", false, |c, f, s, _| {
            align_selected_nodes(c, f, s, anchor, grid)
        })
    }

    pub fn connect_selected_open_endpoints(&mut self) -> bool {
        self.apply_contour_op("Connect Open Endpoints", true, |c, f, s, _| {
            connect_selected_open_endpoints_in_contours(c, f, s)
        })
    }

    pub fn close_open_paths(&mut self) -> bool {
        self.apply_contour_op("Close Open Paths", true, |c, f, s, _| {
            close_open_contours_in_layer(c, f, s)
        })
    }
}
