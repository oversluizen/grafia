//! Adapter operations and pointer interactions for `SPEC-016` (`grafia v0.14.0`):
//! `CanvasIconStudioScene`, `CanvasTool::ShapeBuilder`, and Variable-Width Strokes.

use crate::adapters::state::GrafiaCanvasState;
use crate::core::math_consts::{
    FIT_CURVE_MIN_CHORD_LEN_EPS, MIN_VIEWPORT_ZOOM, TUNNI_HIT_RADIUS_DP, VARIABLE_WIDTH_MAX_UPM,
};
use crate::core::{
    apply_shape_builder_regions_to_contours, compute_radial_symmetry_contours,
    compute_variable_width_handle_pins, evaluate_smart_corner_contours,
    expand_variable_width_contour, hit_test_shape_builder_region,
    hit_test_variable_width_handle_pin, ContourId, FontPoint, GlyphContour, NodeId, ScreenPoint,
    ShapeBuilderOp, VariableWidthHandlePin, VariableWidthStrokeSpec,
};
use crate::interaction::Modifiers;
use crate::render::canvas_render_cache::cached_shape_builder_regions;
use std::collections::{HashMap, HashSet};

impl GrafiaCanvasState {
    pub(crate) fn finish_icon_contour_replacement(
        &mut self,
        new_contours: Vec<GlyphContour>,
        next_nid: u64,
        undo_label: &'static str,
        before: crate::interaction::GlyphSnapshot,
    ) -> bool {
        let mut old_to_new: HashMap<ContourId, ContourId> = HashMap::new();
        for old_c in &self.glyph.contours {
            let old_nids: HashSet<NodeId> = old_c.nodes.iter().map(|n| n.id).collect();
            if old_nids.is_empty() {
                continue;
            }
            if let Some(new_c) = new_contours
                .iter()
                .find(|nc| nc.nodes.iter().any(|n| old_nids.contains(&n.id)))
            {
                old_to_new.insert(old_c.id, new_c.id);
            }
        }
        self.next_node_id = self.next_node_id.max(next_nid);
        self.contours = new_contours.iter().map(|c| c.nodes.clone()).collect();
        self.active_contour_closed = new_contours.last().is_none_or(|c| c.closed);
        self.glyph.contours = new_contours;
        let nids: HashSet<NodeId> = self.contours.iter().flatten().map(|n| n.id).collect();
        let cids: HashSet<ContourId> = self.glyph.contours.iter().map(|c| c.id).collect();
        self.selected_nodes.retain(|id| nids.contains(id));
        self.curve_harmony
            .smart_corners
            .retain(|sc| nids.contains(&sc.node_id));
        self.curve_harmony
            .genius_nodes
            .retain(|id| nids.contains(id));
        self.curve_harmony.g3_nodes.retain(|id| nids.contains(id));
        if !old_to_new.is_empty() {
            let prev_strokes = std::mem::take(&mut self.contour_strokes);
            for (old_cid, style) in prev_strokes {
                if let Some(&new_cid) = old_to_new.get(&old_cid) {
                    self.contour_strokes.insert(new_cid, style);
                }
            }
            self.icon_studio.variable_width_strokes.retain_mut(|vw| {
                match old_to_new.get(&vw.contour_id) {
                    Some(&new_cid) => {
                        vw.contour_id = new_cid;
                        true
                    }
                    None => false,
                }
            });
            if self.color_sublayers.is_empty() && !self.icon_studio.symbol_layer_tags.is_empty() {
                let prev_tags = std::mem::take(&mut self.icon_studio.symbol_layer_tags);
                let mut new_tags =
                    vec![crate::core::SymbolLayerTag::default(); self.glyph.contours.len()];
                for (old_idx, tag) in prev_tags.into_iter().enumerate() {
                    if let Some(&new_cid) = old_to_new.get(&ContourId(old_idx)) {
                        if new_cid.0 < new_tags.len() {
                            new_tags[new_cid.0] = tag;
                        }
                    }
                }
                self.icon_studio.symbol_layer_tags = new_tags;
            }
        } else {
            self.contour_strokes.retain(|cid, _| cids.contains(cid));
            self.icon_studio
                .variable_width_strokes
                .retain(|vw| cids.contains(&vw.contour_id));
        }
        self.sync_paths_from_contours();
        self.reconcile_color_sublayers_after_contour_topology_edit();
        self.record_mutation(undo_label, before);
        self.assert_invariants();
        true
    }

    /// Atomically replaces the active layer's contours, prunes orphaned attachments, and records Undo/Redo.
    pub fn apply_icon_contour_mutation(
        &mut self,
        new_contours: Vec<GlyphContour>,
        next_nid: u64,
        undo_label: &'static str,
    ) -> bool {
        let before = self.snapshot();
        self.finish_icon_contour_replacement(new_contours, next_nid, undo_label, before)
    }

    /// Atomically applies `ShapeBuilderOp::Merge` or `ShapeBuilderOp::Erase` across `region_ids` (`SPEC-016 §3.1`).
    pub fn apply_shape_builder_operation(
        &mut self,
        region_ids: &[u32],
        op: ShapeBuilderOp,
    ) -> bool {
        let mut next_nid = self.next_node_id.max(1);
        let Some(new_c) = apply_shape_builder_regions_to_contours(
            &self.glyph.contours,
            region_ids,
            op,
            &mut next_nid,
        ) else {
            return false;
        };
        let label = match op {
            ShapeBuilderOp::Merge => "Shape Builder Merge",
            ShapeBuilderOp::Erase => "Shape Builder Erase",
        };
        self.apply_icon_contour_mutation(new_c, next_nid, label)
    }

    pub(crate) fn on_shape_builder_pointer_press(
        &mut self,
        scr_pt: ScreenPoint,
        font_pt: FontPoint,
        mods: Modifiers,
    ) -> bool {
        let hit_node = self.hit_test_node(scr_pt);
        let hit_offcurve = hit_node.is_some_and(|id| {
            self.find_node(id)
                .is_some_and(|n| n.node_type == crate::core::NodeType::OffCurve)
        });
        if hit_offcurve {
            return crate::adapters::select_interaction::handle_select_press(
                self, scr_pt, font_pt, mods, false, false,
            );
        }
        if self.try_begin_variable_width_pin_drag(scr_pt, font_pt)
            || self.try_begin_smart_corner_pin_drag(scr_pt, font_pt)
        {
            return true;
        }
        if hit_node.is_some() {
            return crate::adapters::select_interaction::handle_select_press(
                self, scr_pt, font_pt, mods, false, false,
            );
        }
        let regs = cached_shape_builder_regions(self);
        let hit = hit_test_shape_builder_region(&regs, font_pt);
        self.drawing_tool_state.shape_builder =
            crate::adapters::drawing_tool_state::ShapeBuilderDragState {
                active: true,
                erase_mode: mods.option,
                visited_region_ids: hit.into_iter().collect(),
                hovered_region_id: hit,
                active_width_pin_drag: None,
            };
        true
    }

    pub(crate) fn on_shape_builder_pointer_move(
        &mut self,
        font_pt: FontPoint,
        erase_mode: bool,
    ) -> bool {
        let regs = cached_shape_builder_regions(self);
        let hit = hit_test_shape_builder_region(&regs, font_pt);
        let sb = &mut self.drawing_tool_state.shape_builder;
        let hover_changed = sb.hovered_region_id != hit;
        sb.hovered_region_id = hit;
        if sb.active {
            sb.erase_mode |= erase_mode;
            if let Some(rid) = hit {
                if !sb.visited_region_ids.contains(&rid) {
                    sb.visited_region_ids.push(rid);
                }
            }
            true
        } else {
            sb.erase_mode = erase_mode;
            hover_changed
        }
    }

    pub(crate) fn on_shape_builder_pointer_release(
        &mut self,
        font_pt: FontPoint,
        erase_mode: bool,
    ) -> bool {
        let _ = self.on_shape_builder_pointer_move(font_pt, erase_mode);
        let sb = std::mem::take(&mut self.drawing_tool_state.shape_builder);
        let op = if sb.erase_mode || erase_mode {
            ShapeBuilderOp::Erase
        } else {
            ShapeBuilderOp::Merge
        };
        (!sb.visited_region_ids.is_empty())
            && self.apply_shape_builder_operation(&sb.visited_region_ids, op)
    }

    /// Attaches or updates a `VariableWidthStrokeSpec` on `icon_studio.variable_width_strokes` (`SPEC-016 §3.2`).
    pub fn set_variable_width_stroke(&mut self, spec: VariableWidthStrokeSpec) {
        let before = self.snapshot();
        if let Some(ex) = self
            .icon_studio
            .variable_width_strokes
            .iter_mut()
            .find(|vw| vw.contour_id == spec.contour_id)
        {
            *ex = spec;
        } else {
            self.icon_studio.variable_width_strokes.push(spec);
        }
        self.icon_studio = self.icon_studio.clone().sanitized();
        self.record_mutation("Set Variable-Width Stroke", before);
        self.assert_invariants();
    }

    pub(crate) fn append_variable_width_stroke_previews(&self, out: &mut Vec<GlyphContour>) {
        let mut next_nid = self.next_node_id.max(1);
        let cornered = evaluate_smart_corner_contours(
            &self.glyph.contours,
            &self.curve_harmony.smart_corners,
            &mut next_nid,
        );
        for gc in &cornered {
            if !gc.visible {
                continue;
            }
            if let Some(spec) = self
                .icon_studio
                .variable_width_strokes
                .iter()
                .find(|s| s.contour_id == gc.id)
            {
                for mut c in expand_variable_width_contour(gc, spec, &mut next_nid) {
                    c.id = ContourId(out.len());
                    out.push(c);
                }
            }
        }
        if self.icon_studio.radial_symmetry.fold_count > 1 {
            let copies = compute_radial_symmetry_contours(
                &cornered,
                &self.icon_studio.radial_symmetry,
                &mut next_nid,
            );
            let skip = cornered.iter().filter(|c| c.visible).count();
            for mut c in copies.into_iter().skip(skip) {
                c.id = ContourId(out.len());
                out.push(c);
            }
        }
    }

    /// Computes visual width-handle crossbar pins for all active `VariableWidthStrokeSpec`s (`SPEC-016 §3.2`).
    #[must_use]
    pub fn compute_variable_width_handle_pins(&self) -> Vec<VariableWidthHandlePin> {
        compute_variable_width_handle_pins(
            &self.glyph.contours,
            &self.icon_studio.variable_width_strokes,
        )
    }

    /// Uniformly adjusts the left/right widths of all active `VariableWidthStrokeSpec`s by `delta_upm` (`SPEC-016 §3.2`).
    pub fn scale_active_variable_width_stroke(&mut self, delta_upm: f64) -> bool {
        if !delta_upm.is_finite()
            || delta_upm.abs() <= FIT_CURVE_MIN_CHORD_LEN_EPS
            || self.icon_studio.variable_width_strokes.is_empty()
        {
            return false;
        }
        let before = self.snapshot();
        let mut changed = false;
        for s in self
            .icon_studio
            .variable_width_strokes
            .iter_mut()
            .flat_map(|sp| &mut sp.samples)
        {
            let nl = (s.width_left_upm + delta_upm).clamp(0.0, VARIABLE_WIDTH_MAX_UPM);
            let nr = (s.width_right_upm + delta_upm).clamp(0.0, VARIABLE_WIDTH_MAX_UPM);
            if (nl - s.width_left_upm).abs() > FIT_CURVE_MIN_CHORD_LEN_EPS
                || (nr - s.width_right_upm).abs() > FIT_CURVE_MIN_CHORD_LEN_EPS
            {
                s.width_left_upm = nl;
                s.width_right_upm = nr;
                changed = true;
            }
        }
        if changed {
            self.record_mutation("Scale Variable-Width Stroke", before);
            self.assert_invariants();
        }
        changed
    }

    /// Updates `(width_left_upm, width_right_upm)` for a single `sample_index` on `contour_id` (`SPEC-016 §3.2`).
    pub fn update_variable_width_sample(
        &mut self,
        cid: ContourId,
        idx: usize,
        wl: f64,
        wr: f64,
    ) -> bool {
        if !wl.is_finite() || !wr.is_finite() {
            return false;
        }
        let (nl, nr) = (
            wl.clamp(0.0, VARIABLE_WIDTH_MAX_UPM),
            wr.clamp(0.0, VARIABLE_WIDTH_MAX_UPM),
        );
        let Some(pos) = self
            .icon_studio
            .variable_width_strokes
            .iter()
            .position(|s| s.contour_id == cid && idx < s.samples.len())
        else {
            return false;
        };
        let cur = self.icon_studio.variable_width_strokes[pos].samples[idx];
        if (cur.width_left_upm - nl).abs() <= FIT_CURVE_MIN_CHORD_LEN_EPS
            && (cur.width_right_upm - nr).abs() <= FIT_CURVE_MIN_CHORD_LEN_EPS
        {
            return false;
        }
        let before = self.snapshot();
        self.icon_studio.variable_width_strokes[pos].samples[idx].width_left_upm = nl;
        self.icon_studio.variable_width_strokes[pos].samples[idx].width_right_upm = nr;
        self.record_mutation("Adjust Variable-Width Sample", before);
        self.assert_invariants();
        true
    }

    /// Hit-tests `VariableWidthHandlePin`s and starts an interactive pin drag (`SPEC-016 §3.2`).
    pub fn try_begin_variable_width_pin_drag(
        &mut self,
        _scr: ScreenPoint,
        font_pt: FontPoint,
    ) -> bool {
        if self.icon_studio.variable_width_strokes.is_empty() {
            return false;
        }
        let hit_r = f64::from(
            self.scale_touch_hit_radius_dp(TUNNI_HIT_RADIUS_DP)
                / self.viewport.zoom.max(MIN_VIEWPORT_ZOOM),
        );
        let Some(target) = hit_test_variable_width_handle_pin(
            &self.compute_variable_width_handle_pins(),
            font_pt,
            hit_r,
        ) else {
            return false;
        };
        self.drag_transients.active_drag_before_snapshot = Some(self.snapshot());
        self.drawing_tool_state.shape_builder.active_width_pin_drag = Some(target);
        true
    }

    /// Updates an in-flight `VariableWidthHandlePin` drag to `font_pt` (`SPEC-016 §3.2`).
    pub fn try_update_variable_width_pin_drag(&mut self, font_pt: FontPoint) -> bool {
        let Some((cid, idx, is_left)) = self.drawing_tool_state.shape_builder.active_width_pin_drag
        else {
            return false;
        };
        let pins = self.compute_variable_width_handle_pins();
        let Some(pin) = pins
            .iter()
            .find(|p| p.contour_id == cid && p.sample_index == idx)
        else {
            return false;
        };
        let new_w = pin
            .spine_pt
            .distance_to(font_pt)
            .clamp(0.0, VARIABLE_WIDTH_MAX_UPM);
        let Some(spec) = self
            .icon_studio
            .variable_width_strokes
            .iter_mut()
            .find(|s| s.contour_id == cid)
        else {
            return false;
        };
        let Some(sample) = spec.samples.get_mut(idx) else {
            return false;
        };
        if is_left {
            sample.width_left_upm = new_w;
        } else {
            sample.width_right_upm = new_w;
        }
        self.bump_content_generation();
        true
    }

    /// Commits an in-flight `VariableWidthHandlePin` drag onto the Undo/Redo stack (`SPEC-016 §3.2`).
    pub fn try_finish_variable_width_pin_release(&mut self) -> bool {
        if self
            .drawing_tool_state
            .shape_builder
            .active_width_pin_drag
            .take()
            .is_none()
        {
            return false;
        }
        if let Some(before) = self.drag_transients.active_drag_before_snapshot.take() {
            self.record_mutation("Adjust Variable-Width Sample", before);
        }
        self.assert_invariants();
        true
    }
}
