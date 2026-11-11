//! `SPEC-008` (`grafia v0.7.0`) adapter operations:
//! Background Layer aliases, 2D `CanvasAnchor` & `GhostMarkPath`, 2D `CanvasComponent` & Decompose (`Cmd+Shift+D`),
//! 8-Step Fit Curve (`Ctrl+Opt+1..8`), and Live Contour Stroke & Expand Outline.

use crate::adapters::state::GrafiaCanvasState;
use crate::core::{
    apply_fit_curve_delta_to_contours, apply_fit_curve_ratio_to_contours,
    compute_mark_cloud_contours, expand_single_contour_stroke, expand_stroked_contours_preview,
    hit_test_component_upm, normalize_cubic_handles_in_contours, CanvasAnchor, CanvasComponent,
    CanvasHostRequest, ContourId, ContourStrokeStyle, FontPoint, GhostMarkPath, GlyphContour,
    NodeId,
};
use crate::snapping::SnapEngine;
use crate::tokens::{ANCHOR_HIT_RADIUS_DP, DECOMPOSE_NODE_ID_OFFSET, MIN_VIEWPORT_ZOOM};
use std::collections::HashSet;

impl GrafiaCanvasState {
    /// Alias for `swap_foreground_and_background` (`Cmd+K`, `SPEC-008 §2.1`).
    pub fn swap_foreground_with_background(&mut self) {
        self.swap_foreground_and_background();
    }

    /// Alias for `toggle_editing_background_layer` (`Cmd+B`, `SPEC-008 §2.1`).
    pub fn toggle_background_editing(&mut self) {
        self.toggle_editing_background_layer();
    }

    /// Translates a single node `node_id` in the active layer by `delta` in UPM space (`SPEC-008 §2.1`).
    pub fn translate_node_upm(&mut self, node_id: NodeId, delta: FontPoint) -> bool {
        let before = self.snapshot();
        let mut found = false;
        for contour in &mut self.contours {
            for node in contour {
                if node.id == node_id {
                    node.point.x.0 += delta.x.0;
                    node.point.y.0 += delta.y.0;
                    found = true;
                }
            }
        }
        if found {
            self.sync_paths_from_contours();
            self.record_mutation("Translate Node", before);
            self.assert_invariants();
        }
        found
    }

    /// Replaces the active glyph's `CanvasAnchor` list (`SPEC-008 §2.2`).
    pub fn set_anchors(&mut self, anchors: Vec<CanvasAnchor>) {
        for a in &anchors {
            if a.id.0 >= self.next_anchor_id {
                self.next_anchor_id = a.id.0 + 1;
            }
        }
        self.anchors = anchors;
        self.glyph.anchors = self.anchors.clone();
        self.bump_content_generation();
    }

    /// Sets the `GhostMarkPath` mark preview cloud templates (`SPEC-008 §2.2`).
    pub fn set_ghost_mark_paths(&mut self, paths: Vec<GhostMarkPath>) {
        self.ghost_paths = paths;
        self.bump_content_generation();
    }

    /// Hit-tests and selects a `CanvasAnchor` at `pt` in UPM coordinates (`SPEC-008 §2.2.2`).
    pub fn select_anchor_at_upm(&mut self, pt: FontPoint, extend: bool) -> bool {
        let zoom = self.viewport.zoom.max(MIN_VIEWPORT_ZOOM);
        let hit_radius_upm = f64::from(ANCHOR_HIT_RADIUS_DP / zoom);
        let mut best: Option<(usize, f64)> = None;
        for (idx, a) in self.anchors.iter().enumerate() {
            let d = a.position.distance_to(pt);
            if d <= hit_radius_upm && best.is_none_or(|(_, bd)| d < bd) {
                best = Some((idx, d));
            }
        }
        if let Some((hit_idx, _)) = best {
            self.selected_nodes.clear();
            self.selected_guide = None;
            if !extend {
                for a in &mut self.anchors {
                    a.selected = false;
                }
            }
            self.anchors[hit_idx].selected = true;
            self.glyph.anchors = self.anchors.clone();
            true
        } else {
            if !extend {
                for a in &mut self.anchors {
                    a.selected = false;
                }
                self.glyph.anchors = self.anchors.clone();
            }
            false
        }
    }

    /// Cycles selection to the next (`reverse == false`) or previous (`reverse == true`) `CanvasAnchor` (`Tab` / `Shift+Tab`).
    pub fn cycle_selected_anchor(&mut self, reverse: bool) -> bool {
        if self.anchors.is_empty() {
            return false;
        }
        let len = self.anchors.len();
        let cur = self.anchors.iter().position(|a| a.selected);
        let next_idx = match cur {
            Some(i) if reverse => (i + len - 1) % len,
            Some(i) => (i + 1) % len,
            None if reverse => len - 1,
            None => 0,
        };
        for (i, a) in self.anchors.iter_mut().enumerate() {
            a.selected = i == next_idx;
        }
        self.glyph.anchors = self.anchors.clone();
        true
    }

    /// Drags all selected `CanvasAnchor`s so the first selected anchor moves to `pt` (with optional guide/metric snapping).
    pub fn drag_selected_anchors_to_upm(&mut self, pt: FontPoint, bypass_snap: bool) {
        let Some(lead_pos) = self.anchors.iter().find(|a| a.selected).map(|a| a.position) else {
            return;
        };
        let target_pt = if bypass_snap {
            pt
        } else {
            let res = SnapEngine::snap_point(
                pt,
                &[],
                &[],
                &self.metrics,
                self.glyph.width,
                self.italic_angle_deg,
                self.show_guides,
                &self.guides,
                &self.global_guides,
                self.viewport.zoom,
                false,
            );
            self.active_snappers = res.active_snappers;
            res.snapped_point
        };
        let dx = target_pt.x.0 - lead_pos.x.0;
        let dy = target_pt.y.0 - lead_pos.y.0;
        let before = self.snapshot();
        for a in self.anchors.iter_mut().filter(|a| a.selected) {
            a.position.x.0 += dx;
            a.position.y.0 += dy;
        }
        self.glyph.anchors = self.anchors.clone();
        self.record_mutation("Move Anchor", before);
    }

    /// Deletes all currently selected `CanvasAnchor`s with Undo support (`Delete` / `Backspace`).
    pub fn delete_selected_anchors(&mut self) -> bool {
        if !self.anchors.iter().any(|a| a.selected) {
            return false;
        }
        let before = self.snapshot();
        self.anchors.retain(|a| !a.selected);
        self.glyph.anchors = self.anchors.clone();
        self.record_mutation("Delete Anchors", before);
        true
    }

    /// Computes the active `GhostMarkPath` mark preview cloud contours for all selected anchors (`SPEC-008 §2.2.2`).
    #[must_use]
    pub fn compute_active_mark_cloud_contours(&self) -> Vec<GlyphContour> {
        compute_mark_cloud_contours(&self.anchors, &self.ghost_paths, self.show_mark_cloud)
    }

    /// Replaces the 2D pre-resolved `CanvasComponent` list (`SPEC-008 §2.3`).
    pub fn set_components(&mut self, components: Vec<CanvasComponent>) {
        self.components = components;
        self.bump_content_generation();
    }

    /// Hit-tests and selects an unlocked `CanvasComponent` at `pt` in UPM coordinates (`SPEC-008 §2.3.2`).
    pub fn select_component_at_upm(&mut self, pt: FontPoint, extend: bool) -> bool {
        let hit_idx = self
            .components
            .iter()
            .enumerate()
            .rev()
            .find(|(_, c)| !c.locked && hit_test_component_upm(c, pt))
            .map(|(i, _)| i);
        if let Some(idx) = hit_idx {
            if !extend {
                for c in &mut self.components {
                    c.selected = false;
                }
            }
            self.components[idx].selected = true;
            true
        } else {
            if !extend {
                for c in &mut self.components {
                    c.selected = false;
                }
            }
            false
        }
    }

    /// Translates all selected unlocked components that do not have `automatic_alignment == true` by `delta` (`SPEC-008 §2.3.2`).
    pub fn translate_selected_components(&mut self, delta: FontPoint) -> bool {
        if !self
            .components
            .iter()
            .any(|c| c.selected && !c.locked && !c.automatic_alignment)
        {
            return false;
        }
        let before = self.snapshot();
        for c in self
            .components
            .iter_mut()
            .filter(|c| c.selected && !c.locked && !c.automatic_alignment)
        {
            c.transform.tx += delta.x.0;
            c.transform.ty += delta.y.0;
        }
        self.record_mutation("Translate Components", before);
        true
    }

    /// Emits `CanvasHostRequest::OpenComponentSource` when a component is double-clicked at `pt` (`SPEC-008 §2.3.2`).
    pub fn double_click_component_at_upm(&mut self, pt: FontPoint) -> bool {
        if let Some(comp) = self
            .components
            .iter()
            .rev()
            .find(|c| hit_test_component_upm(c, pt))
        {
            self.host_requests
                .push(CanvasHostRequest::OpenComponentSource {
                    id: comp.id,
                    component_name: comp.component_name.clone(),
                });
            true
        } else {
            false
        }
    }

    /// Decomposes all selected unlocked `CanvasComponent`s into editable `GlyphContour`s (`Cmd+Shift+D`, `SPEC-008 §2.3.2`).
    pub fn decompose_selected_components(&mut self) -> bool {
        if !self.components.iter().any(|c| c.selected && !c.locked) {
            return false;
        }
        let before = self.snapshot();
        let mut next_nid = self.next_node_id.max(DECOMPOSE_NODE_ID_OFFSET);
        let mut next_cid = self.glyph.contours.len();
        let mut remaining = Vec::with_capacity(self.components.len());
        let mut decomposed_node_ids = Vec::new();
        for comp in std::mem::take(&mut self.components) {
            if comp.selected && !comp.locked {
                for c in &comp.contours {
                    let tc =
                        comp.transform
                            .transform_contour(c, ContourId(next_cid), &mut next_nid);
                    next_cid += 1;
                    decomposed_node_ids.extend(tc.nodes.iter().map(|n| n.id));
                    self.contours.push(tc.nodes.clone());
                    self.glyph.contours.push(tc);
                }
            } else {
                remaining.push(comp);
            }
        }
        self.components = remaining;
        self.next_node_id = next_nid;
        self.selected_nodes = decomposed_node_ids;
        self.active_contour_closed = self.glyph.contours.last().is_none_or(|c| c.closed);
        self.sync_paths_from_contours();
        self.record_mutation("Decompose Components", before);
        self.assert_invariants();
        true
    }

    /// Drains all pending `CanvasHostRequest` events (`SPEC-008 §2.3.2`).
    pub fn drain_host_requests(&mut self) -> Vec<CanvasHostRequest> {
        std::mem::take(&mut self.host_requests)
    }

    /// Applies column `col` (`0..=7`) of the 8-step Fit Curve ladder (`Ctrl+Opt+1..8`, `SPEC-008 §2.4.2`).
    pub fn apply_fit_curve_column(&mut self, col: usize) -> bool {
        let ratio = self.fit_curve_config.ratio_for_column(col);
        self.fit_selected_curves(ratio)
    }

    /// Applies a uniform Fit Curve ratio `ratio` to all selected cubic segments (`SPEC-008 §2.4.2`).
    pub fn fit_selected_curves(&mut self, ratio: f64) -> bool {
        let before = self.snapshot();
        if apply_fit_curve_ratio_to_contours(&mut self.glyph.contours, &self.selected_nodes, ratio)
        {
            self.contours = self
                .glyph
                .contours
                .iter()
                .map(|c| c.nodes.clone())
                .collect();
            self.sync_paths_from_contours();
            self.record_mutation("Fit Curve", before);
            self.assert_invariants();
            true
        } else {
            self.sync_paths_from_contours();
            false
        }
    }

    /// Steps selected cubic curve handles longer (`longer == true`) or shorter (`longer == false`) by `12.5%` of the Fit Curve range.
    pub fn fit_curve_step_delta(&mut self, longer: bool) -> bool {
        let cfg = self.fit_curve_config.sanitized();
        let delta = cfg.step_delta(longer);
        let before = self.snapshot();
        if apply_fit_curve_delta_to_contours(
            &mut self.glyph.contours,
            &self.selected_nodes,
            delta,
            cfg.min_ratio,
            cfg.max_ratio,
        ) {
            self.contours = self
                .glyph
                .contours
                .iter()
                .map(|c| c.nodes.clone())
                .collect();
            self.sync_paths_from_contours();
            self.record_mutation("Step Fit Curve", before);
            self.assert_invariants();
            true
        } else {
            self.sync_paths_from_contours();
            false
        }
    }

    /// Normalizes each selected cubic segment's handle ratios to their arithmetic mean $\bar{t} = 0.5(t_1 + t_2)$ (`SPEC-008 §2.4.3`).
    pub fn normalize_selected_cubic_handles(&mut self) -> bool {
        let before = self.snapshot();
        if normalize_cubic_handles_in_contours(&mut self.glyph.contours, &self.selected_nodes) {
            self.contours = self
                .glyph
                .contours
                .iter()
                .map(|c| c.nodes.clone())
                .collect();
            self.sync_paths_from_contours();
            self.record_mutation("Normalize Cubic Handles", before);
            self.assert_invariants();
            true
        } else {
            self.sync_paths_from_contours();
            false
        }
    }

    /// Attaches or updates a non-destructive `ContourStrokeStyle` on `contour_id` (`SPEC-008 §2.5.2`).
    pub fn set_contour_stroke(&mut self, contour_id: ContourId, style: ContourStrokeStyle) {
        let before = self.snapshot();
        self.contour_strokes.insert(contour_id, style);
        self.record_mutation("Set Contour Stroke", before);
    }

    /// Computes non-destructive expanded stroke preview contours for all stroked contours (`SPEC-008 §2.5.2`).
    #[must_use]
    pub fn compute_expanded_stroke_contours(&self) -> Vec<GlyphContour> {
        expand_stroked_contours_preview(&self.glyph.contours, &self.contour_strokes)
    }

    /// Destructively expands stroked contours into closed outlines and removes their stroke attributes (`SPEC-008 §2.5.3`).
    pub fn expand_selected_strokes(&mut self) -> bool {
        if self.contour_strokes.is_empty() {
            return false;
        }
        let before = self.snapshot();
        let sel_set: HashSet<NodeId> = self.selected_nodes.iter().copied().collect();
        let mut target_ids: HashSet<ContourId> = self
            .glyph
            .contours
            .iter()
            .filter(|gc| {
                self.contour_strokes
                    .get(&gc.id)
                    .is_some_and(|st| st.width > 0.0 && st.height > 0.0)
                    && (sel_set.is_empty() || gc.nodes.iter().any(|n| sel_set.contains(&n.id)))
            })
            .map(|gc| gc.id)
            .collect();
        if target_ids.is_empty() {
            target_ids = self
                .contour_strokes
                .iter()
                .filter(|(_, st)| st.width > 0.0 && st.height > 0.0)
                .map(|(&cid, _)| cid)
                .collect();
        }
        if target_ids.is_empty() {
            return false;
        }
        let mut next_cid = 0usize;
        let mut next_nid = self.next_node_id.max(DECOMPOSE_NODE_ID_OFFSET);
        let mut new_g_contours = Vec::new();
        for gc in &self.glyph.contours {
            if target_ids.contains(&gc.id) {
                if let Some(style) = self.contour_strokes.remove(&gc.id) {
                    let expanded =
                        expand_single_contour_stroke(gc, style, &mut next_cid, &mut next_nid);
                    new_g_contours.extend(expanded);
                    continue;
                }
            }
            let mut copy = gc.clone();
            copy.id = ContourId(next_cid);
            next_cid += 1;
            new_g_contours.push(copy);
        }
        self.next_node_id = next_nid;
        self.contours = new_g_contours.iter().map(|c| c.nodes.clone()).collect();
        self.active_contour_closed = new_g_contours.last().is_none_or(|c| c.closed);
        self.glyph.contours = new_g_contours;
        self.sync_paths_from_contours();
        self.record_mutation("Expand Stroke Outline", before);
        self.assert_invariants();
        true
    }
}
