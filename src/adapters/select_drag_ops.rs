//! Node displacement, smooth/corner toggling, and `Alt/Option + Drag` contour duplication for `SelectTool`.

use crate::adapters::state::GrafiaCanvasState;
use crate::core::{
    apply_ctrl_alt_interpolate_nudge, apply_ctrl_alt_mirrored_handle_drag,
    enforce_smooth_collinearity_on_handle_drag, expanded_nodes_with_attached_handles,
    slide_on_curve_node_along_contour, slide_smooth_line_node_unlimited,
    slide_smooth_node_on_cubic_path_shift_middle, snap_vector_45_degrees,
    toggle_selected_on_curve_smooth, FontPoint, NodeId, NodeType,
};
use crate::interaction::SelectToolState;

/// Duplicates any contour(s) containing selected nodes when `Alt/Option + Drag` starts on a
/// multi-node selection (`optionDragDuplicate` / `_duplicateSelection`, `SPEC-002 §3.1.8`).
pub fn duplicate_selected_contours_for_alt_drag(state: &mut GrafiaCanvasState) {
    for (id, pt, nt) in state.drag_start_node_types.clone() {
        if let Some(node) = state.find_node_mut(id) {
            node.point = pt;
            node.node_type = nt;
        }
    }
    let closed_flags = state.closed_flags();
    let mut new_selected: Vec<NodeId> = Vec::new();
    let mut duplicated_contours: Vec<(Vec<crate::core::Node>, bool)> = Vec::new();

    for (c_idx, contour) in state.contours.iter().enumerate() {
        if contour.iter().any(|n| state.selected_nodes.contains(&n.id)) {
            let mut cloned_nodes = Vec::with_capacity(contour.len());
            for n in contour {
                let new_id = NodeId(state.next_node_id);
                state.next_node_id += 1;
                new_selected.push(new_id);
                cloned_nodes.push(crate::core::Node::new(
                    new_id,
                    n.point.x.0,
                    n.point.y.0,
                    n.node_type,
                ));
            }
            duplicated_contours.push((cloned_nodes, closed_flags[c_idx]));
        }
    }
    if duplicated_contours.is_empty() {
        return;
    }
    for (nodes, is_closed) in duplicated_contours {
        let new_c_idx = state.contours.len();
        state.contours.push(nodes.clone());
        let mut gc = crate::core::GlyphContour::new(crate::core::ContourId(new_c_idx), nodes);
        gc.closed = is_closed;
        state.glyph.contours.push(gc);
        state.active_contour_closed = is_closed;
    }
    state.selected_nodes = new_selected.clone();
    state.sync_paths_from_contours();
    state.record_drag_start();
    if let SelectToolState::DraggingNodes {
        start_pt,
        current_pt,
        modifiers,
        ..
    } = state.select_state.clone()
    {
        state.select_state = SelectToolState::DraggingNodes {
            selected: new_selected,
            start_pt,
            current_pt,
            modifiers,
        };
    }
}

impl GrafiaCanvasState {
    pub fn try_begin_alt_select_node_press(
        &mut self,
        scr_pt: crate::core::ScreenPoint,
        font_pt: FontPoint,
        mods: crate::interaction::Modifiers,
    ) -> bool {
        if !(mods.option && !mods.control) {
            return false;
        }
        let Some(node_id) = self.hit_test_node(scr_pt) else {
            return false;
        };
        let is_multi_sel_hit =
            self.selected_nodes.len() >= 2 && self.selected_nodes.contains(&node_id);
        let closed_flags = self.closed_flags();
        let has_attached_bcp =
            expanded_nodes_with_attached_handles(&self.contours, &closed_flags, &[node_id], true)
                .len()
                > 1;
        if is_multi_sel_hit
            || has_attached_bcp
            || !self
                .find_node(node_id)
                .is_some_and(|n| n.node_type != NodeType::OffCurve)
        {
            return false;
        }
        let before = self.snapshot();
        if !self.selected_nodes.contains(&node_id) {
            self.selected_nodes = vec![node_id];
        }
        self.active_drag_before_snapshot = Some(before);
        self.record_drag_start();
        toggle_selected_on_curve_smooth(&mut self.contours, &closed_flags, &[node_id]);
        self.sync_paths_from_contours();
        self.select_state = SelectToolState::DraggingNodes {
            selected: self.selected_nodes.clone(),
            start_pt: font_pt,
            current_pt: font_pt,
            modifiers: mods,
        };
        self.assert_invariants();
        true
    }

    pub fn toggle_node_smooth_corner(&mut self, id: NodeId) {
        let before = self.snapshot();
        let closed_flags = self.closed_flags();
        toggle_selected_on_curve_smooth(&mut self.contours, &closed_flags, &[id]);
        self.sync_paths_from_contours();
        self.record_mutation("Toggle Smooth/Corner", before);
        self.assert_invariants();
    }

    pub fn record_drag_start(&mut self) {
        self.drag_start_font_positions.clear();
        self.drag_start_contours = self.contours.clone();
        self.drag_start_node_types = self
            .contours
            .iter()
            .flat_map(|c| c.iter().map(|n| (n.id, n.point, n.node_type)))
            .collect();
        let closed_flags = self.closed_flags();
        for id in expanded_nodes_with_attached_handles(
            &self.contours,
            &closed_flags,
            &self.selected_nodes,
            true,
        ) {
            if let Some(node) = self.find_node(id) {
                self.drag_start_font_positions.push((id, node.point));
            }
        }
    }

    pub fn apply_node_displacement(&mut self, dx_font: f64, dy_font: f64, shift_snap: bool) {
        self.apply_node_displacement_with_modifiers_and_ctrl(
            dx_font, dy_font, shift_snap, false, false,
        );
    }

    pub fn apply_node_displacement_with_modifiers(
        &mut self,
        dx_font: f64,
        dy_font: f64,
        shift_snap: bool,
        alt_held: bool,
    ) {
        self.apply_node_displacement_with_modifiers_and_ctrl(
            dx_font, dy_font, shift_snap, alt_held, false,
        );
    }

    pub fn apply_node_displacement_with_modifiers_and_ctrl(
        &mut self,
        dx_font: f64,
        dy_font: f64,
        shift_snap: bool,
        alt_held: bool,
        ctrl_held: bool,
    ) {
        for (id, pt, nt) in self.drag_start_node_types.clone() {
            if let Some(node) = self.find_node_mut(id) {
                node.point = pt;
                node.node_type = nt;
            }
        }
        let (snapped_dx, snapped_dy) = if shift_snap {
            snap_vector_45_degrees(dx_font, dy_font)
        } else {
            (dx_font, dy_font)
        };
        let selected_copy = self.selected_nodes.clone();
        let closed_flags = self.closed_flags();
        let shadow_contours = if self.drag_start_contours.is_empty() {
            self.contours.clone()
        } else {
            self.drag_start_contours.clone()
        };
        let has_on_curve_selected = selected_copy.iter().any(|&id| {
            self.find_node(id)
                .is_some_and(|n| n.node_type != NodeType::OffCurve)
        });

        if ctrl_held && !alt_held && selected_copy.len() == 1 && has_on_curve_selected {
            let sel_id = selected_copy[0];
            let start_pt = self
                .drag_start_font_positions
                .iter()
                .find(|&&(id, _)| id == sel_id)
                .map_or_else(|| self.find_node(sel_id).unwrap().point, |&(_, p)| p);
            let target_pt = FontPoint::new(start_pt.x.0 + snapped_dx, start_pt.y.0 + snapped_dy);
            for (c_idx, contour) in self.contours.iter_mut().enumerate() {
                if slide_smooth_node_on_cubic_path_shift_middle(
                    contour,
                    &shadow_contours[c_idx],
                    closed_flags[c_idx],
                    sel_id,
                    snapped_dx,
                    snapped_dy,
                ) || slide_on_curve_node_along_contour(
                    contour,
                    closed_flags[c_idx],
                    sel_id,
                    target_pt,
                ) {
                    break;
                }
            }
            self.sync_paths_from_contours();
            return;
        }

        if alt_held && !ctrl_held && selected_copy.len() == 1 && has_on_curve_selected {
            let sel_id = selected_copy[0];
            for (c_idx, contour) in self.contours.iter_mut().enumerate() {
                if slide_smooth_line_node_unlimited(
                    contour,
                    &shadow_contours[c_idx],
                    closed_flags[c_idx],
                    sel_id,
                    snapped_dx,
                    snapped_dy,
                ) {
                    self.sync_paths_from_contours();
                    return;
                }
            }
        }

        let decouple_handles = alt_held && !ctrl_held && selected_copy.len() == 1;
        for (id, start_pt) in self.drag_start_font_positions.clone() {
            if let Some(node) = self.find_node_mut(id) {
                node.point = if decouple_handles && !selected_copy.contains(&id) {
                    start_pt
                } else {
                    FontPoint::new(start_pt.x.0 + snapped_dx, start_pt.y.0 + snapped_dy)
                };
            }
        }

        if ctrl_held && alt_held {
            if has_on_curve_selected {
                apply_ctrl_alt_interpolate_nudge(
                    &mut self.contours,
                    &shadow_contours,
                    &closed_flags,
                    &selected_copy,
                    snapped_dx,
                    snapped_dy,
                );
            } else {
                for &sel_id in &selected_copy {
                    for (c_idx, contour) in self.contours.iter_mut().enumerate() {
                        apply_ctrl_alt_mirrored_handle_drag(
                            contour,
                            closed_flags[c_idx],
                            sel_id,
                            &selected_copy,
                        );
                    }
                }
            }
            self.sync_paths_from_contours();
            return;
        }

        if let [sel_id] = selected_copy.as_slice() {
            if self
                .find_node(*sel_id)
                .is_some_and(|n| n.node_type == NodeType::OffCurve)
            {
                for (c_idx, contour) in self.contours.iter_mut().enumerate() {
                    enforce_smooth_collinearity_on_handle_drag(
                        contour,
                        closed_flags[c_idx],
                        *sel_id,
                        alt_held,
                    );
                }
            }
        }
        self.sync_paths_from_contours();
    }
}
