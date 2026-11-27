//! Color sublayer (`CPAL`/`COLR`), gradient handle drag, and reference frame state adapters (`SPEC-010 §2.1 & §2.3`).

use crate::adapters::state::{CanvasTool, GrafiaCanvasState};
use crate::core::{
    apply_gradient_handle_drag, apply_knockout_halo_masks_to_sublayers,
    compute_radial_symmetry_contours, contour_to_bezpath_with_splines,
    expand_non_destructive_icon_contours, gradient_stops_slice, hit_test_gradient_handle,
    point_in_polygon, resolve_sublayers_for_color_mode, CanvasColorSublayer, CanvasHostRequest,
    ColorLayerPaint, ColorSublayerMutation, CustomReferenceFrame, FontPoint, GradientHandleKind,
    NodeId, ScreenPoint, SelectionInfo, SymbolColorRenderingMode, SymbolHierarchyLevel,
    SymbolLayerTag,
};
use crate::tokens::{CURVE_FLATTEN_TOLERANCE, PERCENT_SCALE_FACTOR};
use kurbo::PathEl;
use std::collections::HashSet;

/// Active pointer drag state for an on-canvas gradient endpoint, radius, or stop diamond handle.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GradientDragState {
    pub sublayer_index: usize,
    pub handle: GradientHandleKind,
}

impl GrafiaCanvasState {
    /// Replaces the ordered stack of color sublayers (`SPEC-010 §2.1`) and bumps `content_generation`.
    pub fn set_color_sublayers(&mut self, sublayers: Vec<CanvasColorSublayer>) {
        self.color_sublayers = sublayers;
        if self
            .drag_transients
            .selected_color_sublayer
            .is_some_and(|idx| idx >= self.color_sublayers.len())
        {
            self.drag_transients.selected_color_sublayer = None;
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
        self.drag_transients.active_gradient_drag = None;
        self.bump_content_generation();
        self.assert_invariants();
    }

    /// Selects a color sublayer by index (`None` deselects all color sublayers and gradient stops).
    pub fn select_color_sublayer(&mut self, index: Option<usize>) {
        self.drag_transients.selected_color_sublayer =
            index.filter(|&idx| idx < self.color_sublayers.len());
        self.drag_transients.selected_gradient_stop = None;
        self.drag_transients.active_gradient_drag = None;
        self.assert_invariants();
    }

    /// Returns the currently selected color sublayer index (`SPEC-017 §2.2`).
    #[must_use]
    pub fn selected_color_sublayer_index(&self) -> Option<usize> {
        self.drag_transients
            .selected_color_sublayer
            .filter(|&idx| idx < self.color_sublayers.len())
    }

    /// Synchronizes contour node geometry from `self.glyph.contours` into `self.color_sublayers`
    /// by matching `ContourId` and `NodeId` lineage (`SPEC-017 §2.2`).
    pub fn sync_color_sublayers_from_glyph_contours(&mut self) {
        if self.color_sublayers.is_empty() || self.glyph.contours.is_empty() {
            return;
        }
        for sub in &mut self.color_sublayers {
            for sc in &mut sub.contours {
                let sc_nids: HashSet<NodeId> = sc.nodes.iter().map(|n| n.id).collect();
                if let Some(gc) = self
                    .glyph
                    .contours
                    .iter()
                    .find(|c| c.id == sc.id && c.nodes.iter().any(|n| sc_nids.contains(&n.id)))
                    .or_else(|| {
                        self.glyph.contours.iter().find(|c| {
                            !sc_nids.is_empty() && c.nodes.iter().any(|n| sc_nids.contains(&n.id))
                        })
                    })
                {
                    sc.id = gc.id;
                    sc.nodes.clone_from(&gc.nodes);
                    sc.closed = gc.closed;
                }
            }
        }
    }

    /// Hit-tests `self.color_sublayers` in top-down Z-order at `font_pt` and selects the hit sublayer (`SPEC-017 §2.2`).
    pub fn try_select_color_sublayer_at_upm(&mut self, font_pt: FontPoint) -> bool {
        for (idx, sub) in self.color_sublayers.iter().enumerate().rev() {
            if !sub.visible || sub.locked {
                continue;
            }
            for gc in &sub.contours {
                if !gc.visible || gc.nodes.len() < 3 {
                    continue;
                }
                let bez = contour_to_bezpath_with_splines(&gc.nodes, true, 0.0, &[]);
                let mut poly = Vec::new();
                kurbo::flatten(bez.iter(), CURVE_FLATTEN_TOLERANCE, |el| {
                    if let PathEl::MoveTo(p) | PathEl::LineTo(p) = el {
                        poly.push(FontPoint::new(p.x, p.y));
                    }
                });
                if poly.len() >= 3 && point_in_polygon(font_pt, &poly) {
                    self.select_color_sublayer(Some(idx));
                    return true;
                }
            }
        }
        false
    }

    /// Moves the currently selected color sublayer forward (`delta > 0`) or backward (`delta < 0`) in Z-order,
    /// keeping `icon_studio.symbol_layer_tags` synchronized and recording an Undo transaction (`SPEC-017 §2.2`).
    pub fn reorder_selected_color_sublayer(&mut self, delta: i32) -> bool {
        let Some(cur_idx) = self.selected_color_sublayer_index() else {
            return false;
        };
        let len = self.color_sublayers.len();
        if len < 2 || delta == 0 {
            return false;
        }
        let target = (cur_idx as i32 + delta).clamp(0, (len - 1) as i32) as usize;
        if target == cur_idx {
            return false;
        }
        let before = self.snapshot();
        self.color_sublayers.swap(cur_idx, target);
        if self.icon_studio.symbol_layer_tags.len() >= len {
            self.icon_studio.symbol_layer_tags.swap(cur_idx, target);
        }
        self.drag_transients.selected_color_sublayer = Some(target);
        if let Some((s_idx, st_idx)) = self.drag_transients.selected_gradient_stop {
            if s_idx == cur_idx {
                self.drag_transients.selected_gradient_stop = Some((target, st_idx));
            }
        }
        self.record_mutation("Reorder Color Sublayer", before);
        self.assert_invariants();
        true
    }

    /// Sets the active Apple SF Symbols 4-mode color rendering mode with Undo/Redo (`SPEC-017 §2.3`).
    pub fn set_symbol_color_rendering_mode(&mut self, mode: SymbolColorRenderingMode) {
        if self.icon_studio.color_mode == mode {
            return;
        }
        let before = self.snapshot();
        self.icon_studio.color_mode = mode;
        self.record_mutation("Set Symbol Color Mode", before);
        self.assert_invariants();
    }

    /// Sets the `SymbolHierarchyLevel` (`Primary`, `Secondary`, `Tertiary`) of the selected color sublayer with Undo/Redo (`SPEC-017 §2.3`).
    pub fn set_selected_sublayer_hierarchy_level(&mut self, level: SymbolHierarchyLevel) -> bool {
        let Some(idx) = self.selected_color_sublayer_index() else {
            return false;
        };
        let cur_lvl = self
            .icon_studio
            .symbol_layer_tags
            .get(idx)
            .map_or(SymbolHierarchyLevel::Primary, |t| t.hierarchy_level);
        if cur_lvl == level && idx < self.icon_studio.symbol_layer_tags.len() {
            return false;
        }
        let before = self.snapshot();
        while self.icon_studio.symbol_layer_tags.len() <= idx {
            let group = (self.icon_studio.symbol_layer_tags.len() as u8)
                .saturating_add(1)
                .max(1);
            self.icon_studio.symbol_layer_tags.push(SymbolLayerTag {
                layer_group: group,
                ..Default::default()
            });
        }
        self.icon_studio.symbol_layer_tags[idx].hierarchy_level = level;
        self.record_mutation("Set Sublayer Hierarchy Level", before);
        self.assert_invariants();
        true
    }

    /// Evaluates `self.color_sublayers` applying `icon_studio.color_mode`, non-destructive `SmartCornerSpec` /
    /// open-stroke expansion (`ContourStrokeStyle` & `VariableWidthStrokeSpec`), and `icon_studio.kinetic` (`SPEC-017 §2.3 & §2.7`).
    #[must_use]
    pub fn evaluate_active_color_sublayers(&self) -> Vec<CanvasColorSublayer> {
        let mut subs = if self.icon_studio.kinetic.is_active() {
            self.evaluate_active_kinetic_sublayers()
        } else {
            resolve_sublayers_for_color_mode(
                &self.color_sublayers,
                &self.icon_studio.symbol_layer_tags,
                self.icon_studio.color_mode,
            )
        };
        let mut next_nid = self.next_node_id.max(1);
        if !self.curve_harmony.smart_corners.is_empty()
            || !self.contour_strokes.is_empty()
            || !self.icon_studio.variable_width_strokes.is_empty()
            || self.icon_studio.radial_symmetry.fold_count > 1
        {
            for sub in &mut subs {
                sub.contours = expand_non_destructive_icon_contours(
                    &sub.contours,
                    &self.curve_harmony.smart_corners,
                    &self.contour_strokes,
                    &self.icon_studio.variable_width_strokes,
                    &mut next_nid,
                );
                if self.icon_studio.radial_symmetry.fold_count > 1 {
                    sub.contours = compute_radial_symmetry_contours(
                        &sub.contours,
                        &self.icon_studio.radial_symmetry,
                        &mut next_nid,
                    );
                }
            }
        }
        apply_knockout_halo_masks_to_sublayers(
            &mut subs,
            &self.icon_studio.symbol_layer_tags,
            &mut next_nid,
        );
        subs
    }

    /// Selects a specific `(sublayer_index, stop_index)` gradient stop for HUD inspection and editing.
    pub fn select_gradient_stop(&mut self, sub_and_stop: Option<(usize, usize)>) {
        let valid = sub_and_stop.filter(|&(sub_idx, stop_idx)| {
            self.color_sublayers
                .get(sub_idx)
                .and_then(|s| gradient_stops_slice(&s.paint))
                .is_some_and(|stops| stop_idx < stops.len())
        });
        self.drag_transients.selected_gradient_stop = valid;
        if let Some((sub_idx, _)) = valid {
            self.drag_transients.selected_color_sublayer = Some(sub_idx);
            self.selected_nodes.clear();
            self.selected_guide = None;
        }
        self.assert_invariants();
    }

    /// Sets or clears the `CustomReferenceFrame` (`SPEC-010 §2.3`) and bumps `content_generation`.
    pub fn set_custom_reference_frame(&mut self, frame: Option<CustomReferenceFrame>) {
        self.custom_reference_frame = frame;
        self.bump_content_generation();
        self.assert_invariants();
    }

    /// Toggles or sets visibility of a color sublayer by index with Undo/Redo support.
    pub fn set_color_sublayer_visibility(&mut self, index: usize, visible: bool) -> bool {
        if index >= self.color_sublayers.len() {
            return false;
        }
        if self.color_sublayers[index].visible == visible {
            return false;
        }
        let before = self.snapshot();
        self.color_sublayers[index].visible = visible;
        self.drag_transients
            .host_requests
            .push(CanvasHostRequest::MutateColorSublayer {
                sublayer_index: index,
                mutation: ColorSublayerMutation::SetVisible(visible),
            });
        self.record_mutation("Change Color Sublayer Visibility", before);
        self.assert_invariants();
        true
    }

    /// Attempts to begin an interactive drag on a gradient handle of the selected color sublayer.
    pub fn try_begin_gradient_handle_press(&mut self, scr_pt: ScreenPoint) -> bool {
        if self.active_tool != CanvasTool::Select {
            return false;
        }
        let Some(sub_idx) = self.drag_transients.selected_color_sublayer else {
            return false;
        };
        let Some(sub) = self.color_sublayers.get(sub_idx) else {
            return false;
        };
        let vp = self.active_slot_viewport();
        let Some(handle) = hit_test_gradient_handle(&sub.paint, scr_pt, &vp) else {
            return false;
        };
        self.drag_transients.active_drag_before_snapshot = Some(self.snapshot());
        self.drag_transients.active_gradient_drag = Some(GradientDragState {
            sublayer_index: sub_idx,
            handle,
        });
        if let GradientHandleKind::StopTick(stop_idx) = handle {
            self.drag_transients.selected_gradient_stop = Some((sub_idx, stop_idx));
            self.selected_nodes.clear();
            self.selected_guide = None;
        } else {
            self.drag_transients.selected_gradient_stop = None;
        }
        true
    }

    /// Updates an in-flight gradient handle drag to `font_pt` in UPM coordinates.
    pub fn try_update_gradient_handle_drag(&mut self, font_pt: FontPoint) -> bool {
        let Some(drag) = self.drag_transients.active_gradient_drag else {
            return false;
        };
        let Some(sub) = self.color_sublayers.get_mut(drag.sublayer_index) else {
            return false;
        };
        if apply_gradient_handle_drag(&mut sub.paint, drag.handle, font_pt) {
            self.bump_content_generation();
        }
        true
    }

    /// Completes an in-flight gradient handle drag, emitting `CanvasHostRequest::MutateColorSublayer`
    /// and recording an atomic `"Change Color Gradient"` Undo transaction.
    pub fn try_finish_gradient_handle_release(&mut self) -> bool {
        let Some(drag) = self.drag_transients.active_gradient_drag.take() else {
            return false;
        };
        if let Some(before) = self.drag_transients.active_drag_before_snapshot.take() {
            let changed = before
                .hinting
                .color_sublayers
                .get(drag.sublayer_index)
                .zip(self.color_sublayers.get(drag.sublayer_index))
                .is_some_and(|(old_s, new_s)| old_s.paint != new_s.paint);
            if changed {
                if let Some(sub) = self.color_sublayers.get(drag.sublayer_index) {
                    self.drag_transients.host_requests.push(
                        CanvasHostRequest::MutateColorSublayer {
                            sublayer_index: drag.sublayer_index,
                            mutation: ColorSublayerMutation::UpdatePaint(sub.paint.clone()),
                        },
                    );
                }
                self.record_mutation("Change Color Gradient", before);
            }
        }
        self.assert_invariants();
        true
    }

    /// Resolves `SelectionInfo::GradientStop` when a gradient stop is selected.
    #[must_use]
    pub fn resolve_gradient_stop_selection_info(&self) -> Option<SelectionInfo> {
        let (sub_idx, stop_idx) = self.drag_transients.selected_gradient_stop?;
        let sub = self.color_sublayers.get(sub_idx)?;
        let stop = gradient_stops_slice(&sub.paint)?.get(stop_idx)?;
        Some(SelectionInfo::GradientStop {
            sublayer_index: sub_idx,
            stop_index: stop_idx,
            offset_pct: stop.offset * PERCENT_SCALE_FACTOR,
            rgba: stop.rgba,
        })
    }

    /// Applies `SelectionInfoAction::MutateGradientStop { offset_pct, rgba }` to the selected stop.
    pub fn apply_gradient_stop_selection_action(&mut self, offset_pct: f64, rgba: [u8; 4]) -> bool {
        let Some((sub_idx, stop_idx)) = self.drag_transients.selected_gradient_stop else {
            return false;
        };
        if !offset_pct.is_finite() {
            return false;
        }
        let has_stop = self
            .color_sublayers
            .get(sub_idx)
            .and_then(|s| gradient_stops_slice(&s.paint))
            .is_some_and(|stops| stop_idx < stops.len());
        if !has_stop {
            return false;
        }
        let before = self.snapshot();
        let new_offset = (offset_pct / PERCENT_SCALE_FACTOR).clamp(0.0, 1.0);
        if let Some(sub) = self.color_sublayers.get_mut(sub_idx) {
            mutate_gradient_stop_in_paint(&mut sub.paint, stop_idx, new_offset, rgba);
            let updated_paint = sub.paint.clone();
            self.drag_transients
                .host_requests
                .push(CanvasHostRequest::MutateColorSublayer {
                    sublayer_index: sub_idx,
                    mutation: ColorSublayerMutation::UpdatePaint(updated_paint),
                });
        }
        self.record_mutation("Change Color Gradient", before);
        self.assert_invariants();
        true
    }
}

fn mutate_gradient_stop_in_paint(
    paint: &mut ColorLayerPaint,
    stop_idx: usize,
    offset: f64,
    rgba: [u8; 4],
) {
    let stops = match paint {
        ColorLayerPaint::LinearGradient { stops, .. }
        | ColorLayerPaint::RadialGradient { stops, .. }
        | ColorLayerPaint::SweepGradient { stops, .. } => stops,
        ColorLayerPaint::Solid { .. }
        | ColorLayerPaint::Bitmap { .. }
        | ColorLayerPaint::ClippingMask => return,
    };
    if let Some(stop) = stops.get_mut(stop_idx) {
        stop.offset = offset;
        stop.rgba = rgba;
    }
}
