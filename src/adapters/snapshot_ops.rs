//! Snapshot, Undo/Redo, Host Bridge, and preset helper methods for `GrafiaCanvasState`.

use crate::adapters::bbox_selection_tools::BBoxDragState;
use crate::adapters::guide_management_ops::GuideDragState;
use crate::adapters::state::GrafiaCanvasState;
use crate::core::{ContourId, Glyph, GlyphContour, NodeId, NodeType, ShapeBooleanRole};
use crate::interaction::{CommandHistory, GlyphSnapshot, PenToolState, SelectToolState};
use crate::snapping::GlyphMetrics;
use std::collections::HashSet;
use std::sync::Arc;

impl GrafiaCanvasState {
    /// Loads any external `Glyph` and `GlyphMetrics` into the canvas state, synchronizing
    /// `self.contours`, `self.paths`, and `self.glyph`, resetting selection/history, and
    /// verifying all structural invariants.
    pub fn load_glyph(&mut self, mut glyph: Glyph, metrics: GlyphMetrics) {
        self.text_stream = crate::core::CanvasTextStream::new();
        let safe_upm = glyph.upm.max(1);
        glyph.upm = safe_upm;
        if glyph.height <= 0.0 {
            glyph.height = metrics.cap_height;
        }
        self.sanitize_loaded_glyph_contours(&mut glyph);
        self.viewport.upm = safe_upm;
        self.metrics = metrics;
        self.contours = glyph.contours.iter().map(|gc| gc.nodes.clone()).collect();
        self.active_contour_closed = glyph.contours.last().is_some_and(|gc| gc.closed);
        for a in &glyph.annotations {
            if a.id.0 >= self.drawing_tool_state.next_annotation_id {
                self.drawing_tool_state.next_annotation_id = a.id.0 + 1;
            }
        }
        self.drawing_tool_state.selected_annotations.clear();
        self.background_contours = glyph.background_contours.clone();
        self.anchors = glyph.anchors.clone();
        self.postscript_hints = glyph.postscript_hints.clone();
        self.selected_postscript_hint = None;
        self.truetype_instructions = glyph.truetype_instructions.clone();
        self.selected_truetype_instruction = None;
        self.drag_transients.hoi_layer_sets = glyph.hoi_layer_sets.clone();
        self.drag_transients.hoi_sibling_contours = glyph.hoi_sibling_contours.clone();
        self.backdrop_ghost_contours = glyph.backdrop_ghost_contours.clone();
        self.smart_corners.clear();
        self.curve_harmony.smart_corners.clear();
        self.editing_background_layer = false;
        self.glyph = glyph;
        self.selected_nodes.clear();
        self.history = CommandHistory::new();
        self.reset_transient_interaction_states();
        self.sync_paths_from_contours();
        self.assert_invariants();
    }

    fn sanitize_loaded_glyph_contours(&mut self, glyph: &mut Glyph) {
        let mut seen_ids = HashSet::new();
        let max_existing = glyph
            .contours
            .iter()
            .flat_map(|gc| gc.nodes.iter().map(|n| n.id.0 + 1))
            .max()
            .unwrap_or(1);
        let mut next_id = self.next_node_id.max(max_existing);
        for (idx, gc) in glyph.contours.iter_mut().enumerate() {
            gc.id = ContourId(idx);
            for n in &mut gc.nodes {
                if n.id.0 == 0 || !seen_ids.insert(n.id) {
                    n.id = NodeId(next_id);
                    seen_ids.insert(n.id);
                    next_id += 1;
                }
            }
            let on_curve_count = gc
                .nodes
                .iter()
                .filter(|n| n.node_type != NodeType::OffCurve)
                .count();
            if on_curve_count <= 1 {
                gc.closed = false;
                gc.nodes.retain(|n| n.node_type != NodeType::OffCurve);
            } else if !gc.closed {
                while gc
                    .nodes
                    .first()
                    .is_some_and(|n| n.node_type == NodeType::OffCurve)
                {
                    gc.nodes.remove(0);
                }
                while gc
                    .nodes
                    .last()
                    .is_some_and(|n| n.node_type == NodeType::OffCurve)
                {
                    gc.nodes.pop();
                }
            }
        }
        self.next_node_id = next_id;
    }

    fn reset_transient_interaction_states(&mut self) {
        self.select_state = SelectToolState::Idle;
        self.pen_state = PenToolState::Idle;
        self.bbox_drag = BBoxDragState::Idle;
        self.drag_transients.guide_drag = GuideDragState::Idle;
        self.drawing_tool_state.reset_in_flight_drags();
        self.active_snappers.clear();
        self.curve_harmony.active_tangent_snap = None;
        self.drag_transients.active_drag_before_snapshot = None;
        self.pending_pen_bcp_out = None;
        self.pending_pen_first_bcp_in = None;
        self.hovered_pen_close_target = None;
        self.hovered_segment_hit = None;
        self.drag_transients.pending_segment_click = None;
        self.drag_transients.alt_marquee_pending_start = None;
        self.drag_transients.alt_duplicate_pending = false;
        self.drag_transients.active_gradient_drag = None;
        self.active_open_contour = (0..self.contours.len())
            .rev()
            .find(|&i| !self.is_contour_closed(i));
    }

    /// Exports an up-to-date `Glyph` value reflecting all current `contours`, `closed` flags,
    /// boolean `role`s, `visible` flags, `locked` flags, and `annotations`.
    #[must_use]
    pub fn export_glyph(&self) -> Glyph {
        let contours = self
            .contours
            .iter()
            .enumerate()
            .map(|(idx, nodes)| {
                let existing = self.glyph.contours.get(idx);
                GlyphContour {
                    id: ContourId(idx),
                    nodes: nodes.clone(),
                    role: existing.map_or(ShapeBooleanRole::Normal, |gc| gc.role),
                    visible: existing.is_none_or(|gc| gc.visible),
                    locked: existing.is_some_and(|gc| gc.locked),
                    closed: self.is_contour_closed(idx),
                }
            })
            .collect();
        Glyph {
            name: self.glyph.name.clone(),
            upm: self.glyph.upm,
            width: self.glyph.width,
            height: self.glyph.height,
            contours,
            annotations: self.glyph.annotations.clone(),
            background_contours: self.background_contours.clone(),
            anchors: self.anchors.clone(),
            postscript_hints: self.postscript_hints.clone(),
            truetype_instructions: self.truetype_instructions.clone(),
            hoi_layer_sets: self.drag_transients.hoi_layer_sets.clone(),
            hoi_sibling_contours: self.drag_transients.hoi_sibling_contours.clone(),
            backdrop_ghost_contours: self.backdrop_ghost_contours.clone(),
        }
    }

    #[must_use]
    pub fn snapshot(&self) -> GlyphSnapshot {
        let meta = (0..self.contours.len())
            .map(|i| {
                self.glyph
                    .contours
                    .get(i)
                    .map_or((crate::core::ShapeBooleanRole::Normal, true), |gc| {
                        (gc.role, gc.visible)
                    })
            })
            .collect();
        GlyphSnapshot::from_state(
            &self.contours,
            self.closed_flags(),
            Arc::from(self.paths.clone().into_boxed_slice()),
            &self.selected_nodes,
        )
        .with_contour_meta(meta)
        .with_pending_pen_handles(self.pending_pen_bcp_out, self.pending_pen_first_bcp_in)
        .with_guides(
            self.guides.clone(),
            self.global_guides.clone(),
            self.selected_guide,
        )
        .with_annotations(
            self.glyph.annotations.clone(),
            self.drawing_tool_state.selected_annotations.clone(),
        )
        .with_text_stream(
            self.glyph.name.clone(),
            (self.glyph.width, self.glyph.height),
            self.text_stream.slots.clone(),
            self.text_stream.active_slot_index,
            self.text_stream.caret_index,
            self.text_stream.selection_range,
        )
        .with_background_and_ruler_state(
            self.background_contours.clone(),
            (self.editing_background_layer, self.show_background),
            self.anchors.clone(),
            self.multi_selection_proportional,
            self.transform_grid_anchor,
            self.show_rulers,
        )
        .with_components_and_strokes_state(
            self.components.clone(),
            self.contour_strokes.clone(),
            self.companion_master_layers.clone(),
            self.drag_transients.selected_companion_nodes.clone(),
            self.show_master_compatibility,
        )
        .with_image_and_smart_handle_state(
            crate::interaction::history::ImageAndSmartHandleSnapshot {
                background_image: self.background_image.clone(),
                smart_handles: self.smart_handles.clone(),
                selected_extra_node: self.selected_extra_node,
            },
        )
        .with_color_layers_and_frame_state(
            self.color_sublayers.clone(),
            self.drag_transients.selected_color_sublayer,
            self.drag_transients.selected_gradient_stop,
            self.custom_reference_frame.clone(),
            self.text_stream.writing_direction,
        )
        .with_hinting_and_hoi_state(self.build_hinting_and_hoi_snapshot())
    }

    pub fn restore_snapshot(&mut self, snap: &GlyphSnapshot) {
        self.restore_snapshot_layers_and_metrics(snap);
        if !snap.contours.is_empty() || snap.paths.is_empty() {
            self.contours = snap.contours.clone();
            self.selected_nodes = snap.selected_nodes.clone();
            self.active_contour_closed = snap.closed_flags.last().copied().unwrap_or(false);
            self.glyph.contours = self
                .contours
                .iter()
                .enumerate()
                .map(|(idx, nodes)| {
                    let mut gc =
                        crate::core::GlyphContour::new(crate::core::ContourId(idx), nodes.clone());
                    gc.closed = snap.closed_flags.get(idx).copied().unwrap_or(true);
                    let (role, visible) = snap
                        .contour_meta
                        .get(idx)
                        .copied()
                        .unwrap_or((crate::core::ShapeBooleanRole::Normal, true));
                    gc.role = role;
                    gc.visible = visible;
                    gc
                })
                .collect();
            self.sync_paths_from_contours();
        } else {
            self.paths = snap.paths.to_vec();
        }
        self.guides = snap.guides.clone();
        self.global_guides = snap.global_guides.clone();
        self.selected_guide = snap.selected_guide;
        for g in self.guides.iter().chain(self.global_guides.iter()) {
            self.drag_transients.next_guide_id = self.drag_transients.next_guide_id.max(g.id.0 + 1);
        }
        self.glyph.annotations = snap.annotations.clone();
        self.drawing_tool_state.selected_annotations = snap.selected_annotations.clone();
        for a in &self.glyph.annotations {
            self.drawing_tool_state.next_annotation_id =
                self.drawing_tool_state.next_annotation_id.max(a.id.0 + 1);
        }
        self.reset_transient_interaction_states();
        self.pen_transients.drag_last_pt = None;
        self.pen_transients.handle_delta = (0.0, 0.0);
        self.pending_pen_bcp_out = snap
            .pending_pen_bcp_out
            .filter(|&(c, _)| c < self.contours.len() && !self.is_contour_closed(c));
        self.pending_pen_first_bcp_in = snap
            .pending_pen_first_bcp_in
            .filter(|&(c, _)| c < self.contours.len() && !self.is_contour_closed(c));
        self.bump_content_generation();
    }

    fn restore_snapshot_layers_and_metrics(&mut self, snap: &GlyphSnapshot) {
        self.background_image = snap.layers.background_image.clone();
        self.smart_handles = snap.layers.smart_handles.clone();
        self.selected_extra_node = snap.layers.selected_extra_node;
        self.backdrop_ghost_contours = snap.layers.backdrop_ghost_contours.clone();
        self.drag_transients.active_smart_handle_drag = None;
        self.active_bbox_rotation_deg = None;
        self.components = snap.layers.components.clone();
        self.contour_strokes = snap.layers.contour_strokes.clone();
        self.companion_master_layers = snap.layers.companion_master_layers.clone();
        self.drag_transients.selected_companion_nodes =
            snap.layers.selected_companion_nodes.clone();
        self.show_master_compatibility = snap.layers.show_master_compatibility;
        self.drag_transients.pre_all_layers_select_snapshot = None;
        self.background_contours = snap.layers.background_contours.clone();
        self.glyph.background_contours = self.background_contours.clone();
        self.editing_background_layer = snap.layers.editing_background_layer;
        self.show_background = snap.layers.show_background;
        self.anchors = snap.layers.anchors.clone();
        self.glyph.anchors = self.anchors.clone();
        for a in &self.anchors {
            if a.id.0 >= self.drag_transients.next_anchor_id {
                self.drag_transients.next_anchor_id = a.id.0 + 1;
            }
        }
        self.multi_selection_proportional = snap.multi_selection_proportional;
        self.transform_grid_anchor = snap.transform_grid_anchor;
        self.show_rulers = snap.show_rulers;
        self.color_sublayers = snap.hinting.color_sublayers.clone();
        self.drag_transients.selected_color_sublayer = snap.hinting.selected_color_sublayer;
        self.drag_transients.selected_gradient_stop = snap.hinting.selected_gradient_stop;
        self.drag_transients.active_gradient_drag = None;
        self.custom_reference_frame = snap.hinting.custom_reference_frame.clone();
        self.text_stream.writing_direction = snap.writing_direction;
        self.postscript_hints = snap.hinting.postscript_hints.clone();
        self.selected_postscript_hint = snap.hinting.selected_postscript_hint;
        self.truetype_instructions = snap.hinting.truetype_instructions.clone();
        self.selected_truetype_instruction = snap.hinting.selected_truetype_instruction;
        self.drag_transients.hoi_layer_sets = snap.hinting.hoi_layer_sets.clone();
        self.drag_transients.hoi_sibling_contours = snap.hinting.hoi_sibling_contours.clone();
        self.curve_harmony = snap.hinting.curve_harmony.clone();
        self.smart_corners = self.curve_harmony.smart_corners.clone();
        self.glyph.postscript_hints = self.postscript_hints.clone();
        self.glyph.truetype_instructions = self.truetype_instructions.clone();
        self.glyph.hoi_layer_sets = self.drag_transients.hoi_layer_sets.clone();
        self.glyph.hoi_sibling_contours = self.drag_transients.hoi_sibling_contours.clone();
        if snap.has_glyph_metrics {
            self.glyph.name = snap.glyph_name.clone();
            self.glyph.width = snap.glyph_width;
            self.glyph.height = snap.glyph_height;
            self.text_stream.slots = snap.text_stream_slots.clone();
            self.text_stream.active_slot_index = snap.active_slot_index;
            self.text_stream.caret_index = snap.caret_index;
            self.text_stream.selection_range = snap.selection_range;
            self.text_stream.recompute_origins();
        }
    }

    pub fn record_mutation(&mut self, label: &'static str, before: GlyphSnapshot) {
        let after = self.snapshot();
        if before != after {
            let content_changed = before.has_content_change(&after);
            self.history.record(label, before, after);
            if content_changed {
                self.bump_content_generation();
            }
        }
    }

    pub fn undo(&mut self) -> bool {
        if let Some(before) = self.drag_transients.active_drag_before_snapshot.take() {
            let after = self.snapshot();
            if before != after {
                self.history.record("In-Flight Edit", before, after);
            }
        }
        let mut snap = self.snapshot();
        let ok = self.history.undo(&mut snap);
        if ok {
            self.restore_snapshot(&snap);
            self.assert_invariants();
        }
        ok
    }

    pub fn redo(&mut self) -> bool {
        let mut snap = self.snapshot();
        let ok = self.history.redo(&mut snap);
        if ok {
            self.restore_snapshot(&snap);
            self.assert_invariants();
        }
        ok
    }
}
