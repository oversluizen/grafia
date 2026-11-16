//! `SPEC-009` (`grafia v0.8.0` Part A) pointer press, drag, and release handlers for
//! `CanvasSmartHandle` ray-projected sliders and `VirtualExtraNode` crossing markers.

use crate::adapters::state::{CanvasTool, GrafiaCanvasState};
use crate::core::extra_nodes::{compute_layer_extra_nodes, hit_test_extra_nodes};
use crate::core::smart_handles::{hit_test_smart_handles, project_pointer_onto_smart_handle};
use crate::core::{CanvasHostRequest, FontPoint, ScreenPoint};

pub fn handle_smart_handle_pointer_press(
    state: &mut GrafiaCanvasState,
    scr_pt: ScreenPoint,
    _font_pt: FontPoint,
) -> Option<bool> {
    let vp = state.active_slot_viewport();
    if let Some(h_idx) =
        hit_test_smart_handles(&state.smart_handles, &state.components, &vp, scr_pt)
    {
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
