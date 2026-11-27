//! `SPEC-009` (`grafia v0.8.0` Part A) pointer press, drag, and release handlers for
//! `CanvasSmartHandle` ray-projected sliders and `VirtualExtraNode` crossing markers.

use crate::adapters::state::{CanvasTool, GrafiaCanvasState};
use crate::core::extra_nodes::{compute_layer_extra_nodes, hit_test_extra_nodes};
use crate::core::math_consts::{
    MIN_VIEWPORT_ZOOM, SMART_CORNER_PIN_BASE_OFFSET_UPM, SMART_HANDLE_HIT_RADIUS_DP,
    TUNNI_HIT_RADIUS_DP,
};
use crate::core::smart_handles::{
    hit_test_smart_handles_with_radius, project_pointer_onto_smart_handle,
};
use crate::core::{
    compute_radial_symmetry_contours, compute_smart_corner_radius_pins,
    expand_non_destructive_icon_contours, hit_test_smart_corner_radius_pin, CanvasHostRequest,
    FontPoint, GlyphContour, ScreenPoint, SmartCornerRadiusPin, SmartCornerSpec,
};

pub fn handle_smart_handle_pointer_press(
    state: &mut GrafiaCanvasState,
    scr_pt: ScreenPoint,
    _font_pt: FontPoint,
) -> Option<bool> {
    let vp = state.active_slot_viewport();
    let hit_r = state.scale_touch_hit_radius_dp(SMART_HANDLE_HIT_RADIUS_DP);
    if let Some(h_idx) = hit_test_smart_handles_with_radius(
        &state.smart_handles,
        &state.components,
        &vp,
        scr_pt,
        hit_r,
    ) {
        state.drag_transients.active_drag_before_snapshot = Some(state.snapshot());
        state.drag_transients.active_smart_handle_drag = Some(h_idx);
        return Some(true);
    }

    if state.show_extra_nodes
        && matches!(state.active_tool, CanvasTool::Select | CanvasTool::Lasso)
        && state.hit_test_node(scr_pt).is_none()
    {
        let extras = compute_layer_extra_nodes(&state.glyph.contours);
        if let Some(key) = hit_test_extra_nodes(&extras, &vp, scr_pt) {
            state.selected_extra_node = Some(key);
            state
                .drag_transients
                .host_requests
                .push(CanvasHostRequest::SelectOrMutateExtraNode { key });
            state.bump_content_generation();
            return Some(true);
        }
    }
    None
}

pub fn handle_smart_handle_pointer_move(
    state: &mut GrafiaCanvasState,
    _scr_pt: ScreenPoint,
    font_pt: FontPoint,
) -> Option<bool> {
    let h_idx = state.drag_transients.active_smart_handle_drag?;
    let vp = state.active_slot_viewport();
    if let Some(handle) = state.smart_handles.get(h_idx).cloned() {
        let new_val = project_pointer_onto_smart_handle(&handle, font_pt, &vp);
        state.smart_handles[h_idx].current_val = new_val;
        state
            .drag_transients
            .host_requests
            .push(CanvasHostRequest::UpdateSmartComponentAxis {
                component_id: handle.component_id,
                axis_id: handle.axis_id,
                new_value: new_val,
            });
        state.bump_content_generation();
    }
    Some(true)
}

pub fn handle_smart_handle_pointer_release(state: &mut GrafiaCanvasState) -> Option<bool> {
    if state
        .drag_transients
        .active_smart_handle_drag
        .take()
        .is_some()
    {
        if let Some(before) = state.drag_transients.active_drag_before_snapshot.take() {
            state.record_mutation("Smart Component Axis", before);
        }
        state.assert_invariants();
        return Some(true);
    }
    None
}

impl GrafiaCanvasState {
    /// Computes on-canvas `SmartCornerRadiusPin` handles for selected or active `Corner` nodes (`SPEC-017 §2.7`).
    #[must_use]
    pub fn compute_smart_corner_radius_pins(&self) -> Vec<SmartCornerRadiusPin> {
        compute_smart_corner_radius_pins(
            &self.glyph.contours,
            &self.selected_nodes,
            &self.curve_harmony.smart_corners,
        )
    }

    /// Hit-tests `SmartCornerRadiusPin` handles and begins an interactive corner-radius drag (`SPEC-017 §2.7`).
    pub fn try_begin_smart_corner_pin_drag(
        &mut self,
        _scr: ScreenPoint,
        font_pt: FontPoint,
    ) -> bool {
        let pins = self.compute_smart_corner_radius_pins();
        if pins.is_empty() {
            return false;
        }
        let hit_r = f64::from(
            self.scale_touch_hit_radius_dp(TUNNI_HIT_RADIUS_DP)
                / self.viewport.zoom.max(MIN_VIEWPORT_ZOOM),
        );
        let Some(node_id) = hit_test_smart_corner_radius_pin(&pins, font_pt, hit_r) else {
            return false;
        };
        self.drag_transients.active_drag_before_snapshot = Some(self.snapshot());
        self.drag_transients.active_smart_corner_pin_drag = Some(node_id);
        true
    }

    /// Updates an in-flight `SmartCornerRadiusPin` drag to `font_pt` (`SPEC-017 §2.7`).
    pub fn try_update_smart_corner_pin_drag(&mut self, font_pt: FontPoint) -> bool {
        let Some(node_id) = self.drag_transients.active_smart_corner_pin_drag else {
            return false;
        };
        let pins = self.compute_smart_corner_radius_pins();
        let Some(pin) = pins.iter().find(|p| p.node_id == node_id) else {
            return false;
        };
        let (dx, dy) = (
            font_pt.x.0 - pin.vertex_pt.x.0,
            font_pt.y.0 - pin.vertex_pt.y.0,
        );
        let proj = dx * pin.bisector_unit.0 + dy * pin.bisector_unit.1;
        let new_r = (proj - SMART_CORNER_PIN_BASE_OFFSET_UPM).clamp(0.0, pin.max_radius_upm);
        if let Some(ex) = self
            .curve_harmony
            .smart_corners
            .iter_mut()
            .find(|s| s.node_id == node_id)
        {
            ex.radius_upm = new_r;
        } else {
            self.curve_harmony.smart_corners.push(SmartCornerSpec {
                node_id,
                radius_upm: new_r,
            });
        }
        self.bump_content_generation();
        true
    }

    /// Commits an in-flight `SmartCornerRadiusPin` drag onto the Undo/Redo stack (`SPEC-017 §2.7`).
    pub fn try_finish_smart_corner_pin_release(&mut self) -> bool {
        if self
            .drag_transients
            .active_smart_corner_pin_drag
            .take()
            .is_none()
        {
            return false;
        }
        if let Some(before) = self.drag_transients.active_drag_before_snapshot.take() {
            self.record_mutation("Adjust Smart Corner Radius", before);
        }
        self.assert_invariants();
        true
    }

    /// Evaluates active glyph contours with non-destructive `SmartCornerSpec`, `ContourStrokeStyle`, and `VariableWidthStrokeSpec` (`SPEC-017 §2.7`).
    #[must_use]
    pub fn evaluate_active_filled_preview_contours(&self) -> Vec<GlyphContour> {
        let mut next_nid = self.next_node_id.max(1);
        let mut out = expand_non_destructive_icon_contours(
            &self.glyph.contours,
            &self.curve_harmony.smart_corners,
            &self.contour_strokes,
            &self.icon_studio.variable_width_strokes,
            &mut next_nid,
        );
        if self.icon_studio.radial_symmetry.fold_count > 1 {
            out = compute_radial_symmetry_contours(
                &out,
                &self.icon_studio.radial_symmetry,
                &mut next_nid,
            );
        }
        out
    }

    /// Returns `true` when non-destructive icon or curve effects (`SmartCornerSpec`, `ContourStrokeStyle`,
    /// `VariableWidthStrokeSpec`, `radial_symmetry`, `kinetic.is_active()`, or `knockout_margin_upm`) are active (`SPEC-017 §2.8b.4`).
    #[must_use]
    pub fn has_active_non_destructive_preview_effects(&self) -> bool {
        !self.curve_harmony.smart_corners.is_empty()
            || !self.contour_strokes.is_empty()
            || !self.icon_studio.variable_width_strokes.is_empty()
            || self.icon_studio.radial_symmetry.fold_count > 1
            || self.icon_studio.kinetic.is_active()
            || self
                .icon_studio
                .symbol_layer_tags
                .iter()
                .any(|t| t.knockout_margin_upm > 0.0)
    }

    /// Evaluates active preview silhouette contours `(GlyphContour, opacity)` across all non-destructive
    /// icon passes (`SmartCornerSpec`, open strokes, radial symmetry, knockout masks, and kinetic/VariableValue, `SPEC-017 §2.8b.4`).
    #[must_use]
    pub fn evaluate_active_preview_silhouette_contours(&self) -> Vec<(GlyphContour, f32)> {
        let has_knockout = self
            .icon_studio
            .symbol_layer_tags
            .iter()
            .any(|t| t.knockout_margin_upm > 0.0);
        if !self.color_sublayers.is_empty()
            && (self.icon_studio.kinetic.is_active() || has_knockout)
        {
            return self
                .evaluate_active_color_sublayers()
                .into_iter()
                .filter(|s| s.visible)
                .flat_map(|s| {
                    let alpha = s.opacity.clamp(0.0, 1.0);
                    s.contours.into_iter().map(move |c| (c, alpha))
                })
                .collect();
        }
        if self.icon_studio.kinetic.is_active() {
            return self.evaluate_active_kinetic_contours();
        }
        self.evaluate_active_filled_preview_contours()
            .into_iter()
            .map(|c| (c, 1.0))
            .collect()
    }
}
