//! Node deletion, open-contour cut, and empty-contour pruning for keyboard shortcuts.

use crate::adapters::state::GrafiaCanvasState;
use crate::core::{
    enforce_smooth_collinearity_on_handle_drag, expanded_nodes_with_attached_handles, NodeType,
};
use egui::Key;

pub(crate) fn handle_arrow_nudge(
    state: &mut GrafiaCanvasState,
    key: Key,
    modifiers: egui::Modifiers,
    is_shift: bool,
    is_cmd: bool,
    is_alt: bool,
) -> bool {
    let alt_held = modifiers.alt || is_alt;
    let ctrl_held = modifiers.ctrl;
    let step = if (modifiers.command || is_cmd) && !ctrl_held {
        state.preferences.nudge_step_large_upm
    } else if modifiers.shift || is_shift {
        state.preferences.nudge_step_medium_upm
    } else if ctrl_held && !alt_held {
        state.preferences.nudge_step_large_upm
    } else {
        state.preferences.nudge_step_small_upm
    };
    let (dx, dy) = match key {
        Key::ArrowLeft => (-step, 0.0),
        Key::ArrowRight => (step, 0.0),
        Key::ArrowUp => (0.0, step),
        Key::ArrowDown => (0.0, -step),
        _ => (0.0, 0.0),
    };
    if state.select_all_layers_mode && !state.drag_transients.selected_companion_nodes.is_empty() {
        state.translate_selected_across_all_layers(crate::core::FontPoint::new(dx, dy));
        return true;
    }
    if state.selected_nodes.is_empty() {
        return state.nudge_selected_guide(dx, dy)
            || state.nudge_selected_anchors_or_components(dx, dy);
    }
    let before = state.snapshot();
    if ctrl_held && alt_held {
        state.record_drag_start();
        state.apply_node_displacement_with_modifiers_and_ctrl(dx, dy, false, true, true);
        state.record_mutation("Nudge Nodes", before);
        state.assert_invariants();
        return true;
    }
    let closed_flags = state.closed_flags();
    let expanded_ids = expanded_nodes_with_attached_handles(
        &state.contours,
        &closed_flags,
        &state.selected_nodes,
        !alt_held,
    );
    for id in expanded_ids {
        if let Some(node) = state.find_node_mut(id) {
            node.point.x.0 += dx;
            node.point.y.0 += dy;
        }
    }
    if state.selected_nodes.len() == 1 {
        let sel_id = state.selected_nodes[0];
        if state
            .find_node(sel_id)
            .is_some_and(|n| n.node_type == NodeType::OffCurve)
        {
            for (c_idx, contour) in state.contours.iter_mut().enumerate() {
                enforce_smooth_collinearity_on_handle_drag(
                    contour,
                    closed_flags[c_idx],
                    sel_id,
                    alt_held,
                );
            }
        }
    }
    state.sync_paths_from_contours();
    state.record_mutation("Nudge Nodes", before);
    state.assert_invariants();
    true
}

pub(crate) fn prune_empty_contours(state: &mut GrafiaCanvasState) {
    let old_closed = state.closed_flags();
    let old_meta: Vec<(crate::core::ShapeBooleanRole, bool)> = (0..state.contours.len())
        .map(|i| {
            state
                .glyph
                .contours
                .get(i)
                .map_or((crate::core::ShapeBooleanRole::Normal, true), |gc| {
                    (gc.role, gc.visible)
                })
        })
        .collect();
    let mut kept_contours = Vec::with_capacity(state.contours.len());
    let mut kept_closed = Vec::with_capacity(state.contours.len());
    let mut kept_meta = Vec::with_capacity(state.contours.len());
    let mut new_active_open = None;

    for (old_idx, contour) in state.contours.drain(..).enumerate() {
        if !contour.is_empty() {
            let new_idx = kept_contours.len();
            let on_curve_count = contour
                .iter()
                .filter(|n| n.node_type != NodeType::OffCurve)
                .count();
            let is_closed = old_closed.get(old_idx).copied().unwrap_or(true) && on_curve_count > 1;
            if state.active_open_contour == Some(old_idx) && !is_closed {
                new_active_open = Some(new_idx);
            }
            kept_contours.push(contour);
            kept_closed.push(is_closed);
            kept_meta.push(
                old_meta
                    .get(old_idx)
                    .copied()
                    .unwrap_or((crate::core::ShapeBooleanRole::Normal, true)),
            );
        }
    }

    state.contours = kept_contours;
    state.active_open_contour =
        new_active_open.or_else(|| kept_closed.iter().rposition(|&closed| !closed));
    state.active_contour_closed = kept_closed.last().copied().unwrap_or(false);
    state.glyph.contours = state
        .contours
        .iter()
        .enumerate()
        .map(|(idx, nodes)| {
            let mut gc = crate::core::GlyphContour::new(crate::core::ContourId(idx), nodes.clone());
            gc.closed = kept_closed[idx];
            gc.role = kept_meta[idx].0;
            gc.visible = kept_meta[idx].1;
            gc
        })
        .collect();
}

pub(crate) fn delete_selected_nodes_or_guide(
    state: &mut GrafiaCanvasState,
    shift_active: bool,
    alt_active: bool,
) -> bool {
    if state.selected_nodes.is_empty() {
        if state.delete_selected_postscript_hint() {
            return true;
        }
        return state.delete_selected_guide();
    }
    let before = state.snapshot();
    let cut_open = shift_active || alt_active;
    if cut_open {
        for c_idx in 0..state.contours.len() {
            let has_sel = state.contours[c_idx]
                .iter()
                .any(|n| state.selected_nodes.contains(&n.id));
            if has_sel {
                let is_closed = state.is_contour_closed(c_idx);
                crate::core::cut_open_delete_nodes_in_contour(
                    &mut state.contours[c_idx],
                    is_closed,
                    &state.selected_nodes,
                );
                if !state.contours[c_idx].is_empty() {
                    state.set_contour_closed(c_idx, false);
                    state.active_open_contour = Some(c_idx);
                }
            }
        }
    } else {
        for c_idx in 0..state.contours.len() {
            let has_sel = state.contours[c_idx]
                .iter()
                .any(|n| state.selected_nodes.contains(&n.id));
            if has_sel {
                let is_closed = state.is_contour_closed(c_idx);
                crate::core::heal_delete_nodes(
                    &mut state.contours[c_idx],
                    is_closed,
                    &state.selected_nodes,
                    &mut state.next_node_id,
                );
                if state.contours[c_idx].len() <= 1 {
                    state.set_contour_closed(c_idx, false);
                }
            }
        }
    }
    state.selected_nodes.clear();
    state.hovered_segment_hit = None;
    state.hovered_pen_close_target = None;
    state.drag_transients.pending_segment_click = None;
    state.pending_pen_bcp_out = None;
    state.pending_pen_first_bcp_in = None;
    prune_empty_contours(state);
    state.sync_paths_from_contours();
    state.record_mutation(
        if cut_open {
            "Cut Contour Open"
        } else {
            "Delete Nodes"
        },
        before,
    );
    state.assert_invariants();
    true
}
