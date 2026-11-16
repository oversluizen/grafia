//! Color sublayer (`CPAL`/`COLR`), gradient handle drag, and reference frame state adapters (`SPEC-010 §2.1 & §2.3`).

use crate::adapters::state::{CanvasTool, GrafiaCanvasState};
use crate::core::{
    apply_gradient_handle_drag, gradient_stops_slice, hit_test_gradient_handle,
    CanvasColorSublayer, CanvasHostRequest, ColorLayerPaint, ColorSublayerMutation,
    CustomReferenceFrame, FontPoint, GradientHandleKind, ScreenPoint, SelectionInfo,
};
use crate::tokens::PERCENT_SCALE_FACTOR;

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
