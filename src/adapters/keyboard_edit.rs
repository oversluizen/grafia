//! Node deletion, open-contour cut, and empty-contour pruning for keyboard shortcuts.

use crate::adapters::state::GrafiaCanvasState;
use crate::core::NodeType;

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
    state.hovered_pen_close_node = None;
    state.pending_segment_click = None;
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
