//! `SPEC-007` (`grafia v0.6.0`) multi-node selection mutators (`TransformGridAnchor`, origin, scale)
//! and bounding-box / transform pointer release handling.

use crate::adapters::bbox_selection_tools::BBoxDragState;
use crate::adapters::contour_edit_interaction::{
    finish_transform_tool_release, finish_zoom_drag_release, TransformReleaseCoords,
    TransformReleaseFlags,
};
use crate::adapters::state::GrafiaCanvasState;
use crate::core::math_consts::{DEFAULT_PALETTE_COLORS_RGBA, RADIAL_SYMMETRY_MAX_FOLDS};
use crate::core::{
    compute_selection_bounds, expanded_nodes_with_attached_handles, gradient_stops_slice,
    select_nodes_in_lasso_polygon, BBoxAction, CanvasColorSublayer, ColorLayerPaint, FontPoint,
    GlyphContour, NodeId, SymbolLayerTag, TransformGridAnchor,
};
use crate::tokens::MULTI_SCALE_SPAN_EPS_UPM;
use egui::Rect;
use std::collections::HashSet;

impl GrafiaCanvasState {
    /// Selects all nodes across all contours in the active layer.
    pub fn select_all_nodes(&mut self) {
        self.selected_guide = None;
        self.selected_nodes = self
            .contours
            .iter()
            .flat_map(|c| c.iter().map(|n| n.id))
            .collect();
        self.assert_invariants();
    }

    /// Sets the 9-point `TransformGridAnchor` reference for multi-node selection bounds.
    pub fn set_multi_selection_anchor(&mut self, anchor: TransformGridAnchor) {
        let before = self.snapshot();
        self.transform_grid_anchor = anchor;
        self.record_mutation("Set Transform Anchor", before);
        self.assert_invariants();
    }

    /// Sets whether `set_multi_selection_size` preserves aspect ratio (`proportional_scale`).
    pub fn set_multi_selection_proportional(&mut self, proportional: bool) {
        let before = self.snapshot();
        self.multi_selection_proportional = proportional;
        self.record_mutation("Set Proportional Scale", before);
        self.assert_invariants();
    }

    /// Translates all selected nodes so the active `TransformGridAnchor` origin moves to `new_origin`.
    pub fn set_multi_selection_origin(&mut self, new_origin: FontPoint) {
        if self.selected_nodes.len() < 2
            || !new_origin.x.0.is_finite()
            || !new_origin.y.0.is_finite()
        {
            return;
        }
        let Some(bounds) = compute_selection_bounds(
            &self.contours,
            &self.selected_nodes,
            0.0,
            self.metrics.x_height * 0.5,
        ) else {
            return;
        };
        let cur_origin = bounds.aligned_point(self.transform_grid_anchor.index());
        let (dx, dy) = (
            new_origin.x.0 - cur_origin.x.0,
            new_origin.y.0 - cur_origin.y.0,
        );
        let before = self.snapshot();
        let closed_flags = self.closed_flags();
        let sel_set: HashSet<NodeId> = expanded_nodes_with_attached_handles(
            &self.contours,
            &closed_flags,
            &self.selected_nodes,
            true,
        )
        .into_iter()
        .collect();
        for n in self
            .contours
            .iter_mut()
            .flatten()
            .filter(|n| sel_set.contains(&n.id))
        {
            n.point.x.0 += dx;
            n.point.y.0 += dy;
        }
        self.sync_paths_from_contours();
        self.record_mutation("Move Selection Origin", before);
        self.assert_invariants();
    }

    /// Scales all selected nodes relative to the active `TransformGridAnchor` origin (`SPEC-007 §2.2.4`).
    pub fn set_multi_selection_size(&mut self, new_width: Option<f64>, new_height: Option<f64>) {
        if self.selected_nodes.len() < 2
            || (new_width.is_none() && new_height.is_none())
            || new_width.is_some_and(|w| !w.is_finite())
            || new_height.is_some_and(|h| !h.is_finite())
        {
            return;
        }
        let Some(bounds) = compute_selection_bounds(
            &self.contours,
            &self.selected_nodes,
            0.0,
            self.metrics.x_height * 0.5,
        ) else {
            return;
        };
        let (cur_w, cur_h) = (bounds.width(), bounds.height());
        let mut sx = new_width.map_or(1.0, |w| {
            if cur_w > MULTI_SCALE_SPAN_EPS_UPM {
                w / cur_w
            } else {
                1.0
            }
        });
        let mut sy = new_height.map_or(1.0, |h| {
            if cur_h > MULTI_SCALE_SPAN_EPS_UPM {
                h / cur_h
            } else {
                1.0
            }
        });
        if self.multi_selection_proportional {
            match (new_width.is_some(), new_height.is_some()) {
                (true, false) => sy = sx,
                (false, true) => sx = sy,
                _ => {}
            }
        }
        if !sx.is_finite() || !sy.is_finite() {
            return;
        }
        let pivot = bounds.aligned_point(self.transform_grid_anchor.index());
        let before = self.snapshot();
        let closed_flags = self.closed_flags();
        let sel_set: HashSet<NodeId> = expanded_nodes_with_attached_handles(
            &self.contours,
            &closed_flags,
            &self.selected_nodes,
            true,
        )
        .into_iter()
        .collect();
        for n in self
            .contours
            .iter_mut()
            .flatten()
            .filter(|n| sel_set.contains(&n.id))
        {
            n.point.x.0 = pivot.x.0 + (n.point.x.0 - pivot.x.0) * sx;
            n.point.y.0 = pivot.y.0 + (n.point.y.0 - pivot.y.0) * sy;
        }
        self.sync_paths_from_contours();
        self.record_mutation("Scale Selection", before);
        self.assert_invariants();
    }

    /// Auto-adopts newly drawn `contours` into `self.color_sublayers` + `self.icon_studio.symbol_layer_tags`
    /// when `self.color_sublayers` is active or when the Icon Keyline Artboard is active (`SPEC-017 §2.8b & §2.9`).
    pub fn try_auto_adopt_drawn_contours_into_sublayers(&mut self, contours: &[GlyphContour]) {
        let is_icon_artboard = self
            .custom_reference_frame
            .as_ref()
            .is_some_and(|f| f.show_diagonals);
        if (self.color_sublayers.is_empty() && !is_icon_artboard) || contours.is_empty() {
            return;
        }
        let mut next_sub_id = self.color_sublayers.iter().map(|s| s.id).max().unwrap_or(0) + 1;
        for gc in contours {
            let already_present = self.color_sublayers.iter().any(|s| {
                s.contours.iter().any(|sc| {
                    sc.id == gc.id
                        || (!sc.nodes.is_empty()
                            && sc
                                .nodes
                                .iter()
                                .any(|sn| gc.nodes.iter().any(|gn| gn.id == sn.id)))
                })
            });
            if already_present {
                continue;
            }
            let new_idx = self.color_sublayers.len();
            let rgba = DEFAULT_PALETTE_COLORS_RGBA[new_idx % DEFAULT_PALETTE_COLORS_RGBA.len()];
            self.color_sublayers.push(CanvasColorSublayer {
                id: next_sub_id,
                name: format!("Layer{}", new_idx + 1),
                contours: vec![gc.clone()],
                paint: ColorLayerPaint::Solid { rgba },
                opacity: 1.0,
                visible: true,
                locked: false,
            });
            next_sub_id += 1;
            while self.icon_studio.symbol_layer_tags.len() <= new_idx {
                let grp = ((self.icon_studio.symbol_layer_tags.len() as u8) + 1)
                    .clamp(1, RADIAL_SYMMETRY_MAX_FOLDS);
                self.icon_studio.symbol_layer_tags.push(SymbolLayerTag {
                    layer_group: grp,
                    ..Default::default()
                });
            }
            self.drag_transients.selected_color_sublayer = Some(new_idx);
        }
    }

    /// Prunes color sublayers whose contour nodes were all deleted from `self.glyph.contours` (`SPEC-017 §2.8b`).
    pub fn prune_deleted_contours_from_color_sublayers(&mut self) {
        if self.color_sublayers.is_empty() {
            return;
        }
        let live_nids: HashSet<NodeId> = self
            .glyph
            .contours
            .iter()
            .flat_map(|c| c.nodes.iter().map(|n| n.id))
            .collect();
        for sub in &mut self.color_sublayers {
            sub.contours
                .retain(|sc| sc.nodes.iter().any(|n| live_nids.contains(&n.id)));
        }
        let mut idx = 0;
        while idx < self.color_sublayers.len() {
            let is_mask = matches!(
                self.color_sublayers[idx].paint,
                ColorLayerPaint::ClippingMask
            );
            if self.color_sublayers[idx].contours.is_empty() && !is_mask {
                self.color_sublayers.remove(idx);
                if idx < self.icon_studio.symbol_layer_tags.len() {
                    self.icon_studio.symbol_layer_tags.remove(idx);
                }
            } else {
                idx += 1;
            }
        }
        if self
            .drag_transients
            .selected_color_sublayer
            .is_some_and(|i| i >= self.color_sublayers.len())
        {
            self.drag_transients.selected_color_sublayer =
                (!self.color_sublayers.is_empty()).then_some(self.color_sublayers.len() - 1);
        }
        if self
            .drag_transients
            .selected_gradient_stop
            .is_some_and(|(s_idx, st_idx)| {
                self.color_sublayers
                    .get(s_idx)
                    .and_then(|s| gradient_stops_slice(&s.paint))
                    .is_none_or(|stops| st_idx >= stops.len())
            })
        {
            self.drag_transients.selected_gradient_stop = None;
        }
    }

    /// Reconciles `self.color_sublayers` and `self.icon_studio.symbol_layer_tags` after topological contour
    /// edits (`ShapeBuilder`, `Remove Overlap`, `Knife`, `SPEC-017 §2.8b.4`).
    pub fn reconcile_color_sublayers_after_contour_topology_edit(&mut self) {
        if self.color_sublayers.is_empty() {
            return;
        }
        let live_nids: HashSet<NodeId> = self
            .glyph
            .contours
            .iter()
            .flat_map(|c| c.nodes.iter().map(|n| n.id))
            .collect();
        let mut claimed_cids: HashSet<crate::core::ContourId> = HashSet::new();
        for sub in &self.color_sublayers {
            for sc in &sub.contours {
                if sc.nodes.iter().any(|n| live_nids.contains(&n.id)) {
                    claimed_cids.insert(sc.id);
                }
            }
        }
        let mut unowned: std::collections::VecDeque<GlyphContour> = self
            .glyph
            .contours
            .iter()
            .filter(|c| !claimed_cids.contains(&c.id))
            .cloned()
            .collect();
        for sub in &mut self.color_sublayers {
            let has_surviving = sub
                .contours
                .iter()
                .any(|sc| sc.nodes.iter().any(|n| live_nids.contains(&n.id)));
            if !has_surviving {
                if let Some(fresh_c) = unowned.pop_front() {
                    claimed_cids.insert(fresh_c.id);
                    sub.contours = vec![fresh_c];
                }
            }
        }
        let mut seen_cids: HashSet<crate::core::ContourId> = HashSet::new();
        for sub in &mut self.color_sublayers {
            sub.contours.retain(|sc| {
                sc.nodes.iter().any(|n| live_nids.contains(&n.id)) && seen_cids.insert(sc.id)
            });
        }
        self.prune_deleted_contours_from_color_sublayers();
        let remaining_unowned: Vec<GlyphContour> = unowned.into_iter().collect();
        if !remaining_unowned.is_empty() {
            self.try_auto_adopt_drawn_contours_into_sublayers(&remaining_unowned);
        }
    }
}

pub fn handle_bbox_and_transform_pointer_release(
    state: &mut GrafiaCanvasState,
    is_shift: bool,
    is_alt: bool,
    is_ctrl: bool,
    canvas_rect: Rect,
) -> Option<bool> {
    if state.drag_transients.guide_drag
        != crate::adapters::guide_management_ops::GuideDragState::Idle
    {
        state.drag_transients.guide_drag =
            crate::adapters::guide_management_ops::GuideDragState::Idle;
        state.active_snappers.clear();
        if let Some(before) = state.drag_transients.active_drag_before_snapshot.take() {
            state.record_mutation("Guide Transform", before);
        }
        state.assert_invariants();
        return Some(true);
    }
    match std::mem::take(&mut state.bbox_drag) {
        BBoxDragState::Idle => None,
        BBoxDragState::BBoxDragging { action, .. } => {
            state.active_bbox_rotation_deg = None;
            let label = match action {
                BBoxAction::Scale(_) => "Bounding Box Transform",
                BBoxAction::Rotate(_) | BBoxAction::Slant(_) => "Transform Selection",
            };
            if let Some(before) = state.drag_transients.active_drag_before_snapshot.take() {
                state.record_mutation(label, before);
            }
            state.assert_invariants();
            Some(true)
        }
        BBoxDragState::TransformOriginDragging { .. } => {
            state.assert_invariants();
            Some(true)
        }
        BBoxDragState::TransformPendingOrDragging {
            is_rotate,
            start_scr,
            current_scr,
            start_font,
            active,
            ..
        } => {
            let flags = TransformReleaseFlags {
                is_rotate,
                active,
                is_ctrl,
            };
            let coords = TransformReleaseCoords {
                start_scr,
                current_scr,
                start_font,
            };
            finish_transform_tool_release(state, flags, coords);
            Some(true)
        }
        BBoxDragState::LassoDragging {
            points_font,
            initial_selected,
            ..
        } => {
            state.selected_nodes = select_nodes_in_lasso_polygon(
                &state.contours,
                &points_font,
                &initial_selected,
                is_shift,
                is_alt,
            );
            state.assert_invariants();
            Some(true)
        }
        BBoxDragState::ZoomDragging {
            start_scr,
            current_scr,
        } => {
            let res = finish_zoom_drag_release(state, start_scr, current_scr, canvas_rect, is_alt);
            Some(res)
        }
    }
}
