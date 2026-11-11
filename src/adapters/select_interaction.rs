//! Select tool (`V` / `SelectTool`) interaction handlers for `GlifoCanvasState` (`SPEC-002 §3`).

use crate::adapters::state::GlifoCanvasState;
use crate::core::{
    apply_ctrl_alt_interpolate_nudge, apply_ctrl_alt_mirrored_handle_drag,
    convert_line_segment_to_cubic_with_grid, enforce_smooth_collinearity_on_handle_drag,
    expanded_nodes_with_attached_handles, hit_test_segments, slide_on_curve_node_along_contour,
    slide_smooth_line_node_unlimited, slide_smooth_node_on_cubic_path_shift_middle,
    snap_vector_45_degrees, toggle_selected_on_curve_smooth, ContourSegment, FontPoint, NodeId,
    NodeType, ScreenPoint,
};
use crate::interaction::{Modifiers, SelectToolState};
use crate::render::ProjectedNode;
use crate::tokens::{DOUBLE_CLICK_MAX_DIST_DP, GlifoTheme, SEGMENT_HIT_TOLERANCE_DP};
use kurbo::Point;

pub fn handle_select_press(
    state: &mut GlifoCanvasState,
    scr_pt: ScreenPoint,
    font_pt: FontPoint,
    mods: Modifiers,
    is_double: bool,
    is_triple: bool,
) -> bool {
    if state.try_begin_alt_select_node_press(scr_pt, font_pt, mods) {
        return true;
    }
    let (is_shift, is_alt) = (mods.shift, mods.option);
    let before = state.snapshot();
    state.active_drag_before_snapshot = Some(before.clone());
    state.pending_segment_click = None;
    state.alt_marquee_pending_start = None;
    state.alt_duplicate_pending = false;

    if is_triple {
        state.selected_nodes = state.contours.iter().flat_map(|c| c.iter().map(|n| n.id)).collect();
        state.select_state = SelectToolState::Idle;
        state.active_drag_before_snapshot = None;
        return true;
    }

    if is_double {
        let hit_node = state.hit_test_node(scr_pt);
        if let Some(node_id) = hit_node {
            if state.find_node(node_id).is_some_and(|n| n.node_type != NodeType::OffCurve) {
                state.toggle_node_smooth_corner(node_id);
                state.active_drag_before_snapshot = None;
                return true;
            }
        }
        let tol_font = (DOUBLE_CLICK_MAX_DIST_DP as f64) / (state.viewport.zoom as f64);
        let closed_flags = state.closed_flags();
        let target_contour = hit_node
            .and_then(|nid| state.contours.iter().position(|c| c.iter().any(|n| n.id == nid)))
            .or_else(|| hit_test_segments(&state.contours, &closed_flags, font_pt, tol_font).map(|h| h.contour_idx));
        if let Some(c_idx) = target_contour {
            let contour_ids: Vec<NodeId> = state.contours[c_idx].iter().map(|n| n.id).collect();
            if is_shift {
                if contour_ids.iter().all(|id| state.selected_nodes.contains(id)) {
                    state.selected_nodes.retain(|id| !contour_ids.contains(id));
                } else {
                    for id in contour_ids {
                        if !state.selected_nodes.contains(&id) {
                            state.selected_nodes.push(id);
                        }
                    }
                }
            } else {
                state.selected_nodes = contour_ids;
            }
            state.select_state = SelectToolState::Idle;
            state.active_drag_before_snapshot = None;
            return true;
        }
    }

    if let Some(node_id) = state.hit_test_node(scr_pt) {
        if is_shift {
            if state.selected_nodes.contains(&node_id) {
                state.selected_nodes.retain(|&id| id != node_id);
            } else {
                state.selected_nodes.push(node_id);
            }
        } else if !state.selected_nodes.contains(&node_id) {
            state.selected_nodes = vec![node_id];
        }
        state.record_drag_start();
        state.alt_duplicate_pending = is_alt && !mods.control && state.selected_nodes.len() >= 2;
        state.select_state = SelectToolState::DraggingNodes { selected: state.selected_nodes.clone(), start_pt: font_pt, current_pt: font_pt, modifiers: mods };
        return true;
    }

    let tol_font = (SEGMENT_HIT_TOLERANCE_DP as f64) / (state.viewport.zoom as f64);
    let closed_flags = state.closed_flags();
    let grid_step = state.effective_grid_step();
    if let Some(hit) = hit_test_segments(&state.contours, &closed_flags, font_pt, tol_font) {
        if is_alt
            && convert_line_segment_to_cubic_with_grid(
                &mut state.contours[hit.contour_idx],
                &hit.segment,
                &mut state.next_node_id,
                grid_step,
            )
            .is_some()
        {
            state.sync_paths_from_contours();
            state.record_mutation("Convert Line to Cubic", before);
            state.active_drag_before_snapshot = None;
            state.assert_invariants();
            return true;
        }

        let (start_idx, end_idx) = match hit.segment {
            ContourSegment::Line { start_idx, end_idx, .. } | ContourSegment::Cubic { start_idx, end_idx, .. } => (start_idx, end_idx),
        };
        let p0_id = state.contours[hit.contour_idx][start_idx].id;
        let p3_id = state.contours[hit.contour_idx][end_idx].id;
        if state.selected_nodes.contains(&p0_id) && state.selected_nodes.contains(&p3_id) {
            if is_shift {
                state.marquee_pre_selection = state.selected_nodes.clone();
                state.pending_segment_click = Some((hit.contour_idx, hit.segment, mods));
            }
            state.record_drag_start();
            state.alt_duplicate_pending = is_alt && !mods.control && state.selected_nodes.len() >= 2;
            state.select_state = SelectToolState::DraggingNodes { selected: state.selected_nodes.clone(), start_pt: font_pt, current_pt: font_pt, modifiers: mods };
            return true;
        }

        state.marquee_pre_selection = state.selected_nodes.clone();
        state.marquee_modifiers = mods;
        if !is_shift { state.selected_nodes.clear(); }
        state.pending_segment_click = Some((hit.contour_idx, hit.segment, mods));

        match hit.segment {
            ContourSegment::Line { .. } => {
                state.active_drag_before_snapshot = None;
                state.select_state = SelectToolState::MarqueeSelecting { start_screen: scr_pt, current_screen: scr_pt };
                return true;
            }
            ContourSegment::Cubic { h1_idx, h2_idx, .. } => {
                let (initial_h1, initial_h2) = (state.contours[hit.contour_idx][h1_idx].point, state.contours[hit.contour_idx][h2_idx].point);
                state.record_drag_start();
                state.select_state = SelectToolState::DraggingSegment { contour_idx: hit.contour_idx, segment: hit.segment, t: hit.t, start_pt: font_pt, initial_h1, initial_h2 };
                return true;
            }
        }
    }

    state.marquee_pre_selection = if is_shift { state.selected_nodes.clone() } else { Vec::new() };
    state.marquee_modifiers = mods;
    if !is_shift { state.selected_nodes.clear(); }
    state.active_drag_before_snapshot = None;
    if is_alt {
        state.alt_marquee_pending_start = Some(scr_pt);
        state.select_state = SelectToolState::Idle;
    } else {
        state.select_state = SelectToolState::MarqueeSelecting { start_screen: scr_pt, current_screen: scr_pt };
    }
    true
}

pub fn apply_marquee_selection(state: &mut GlifoCanvasState, theme: &GlifoTheme, is_alt_now: bool) {
    let only_on_curve = state.marquee_modifiers.option || is_alt_now;
    let xor_toggle = state.marquee_modifiers.shift;
    let ctrl_slant = state.marquee_modifiers.control && !state.marquee_modifiers.command;
    if let SelectToolState::MarqueeSelecting { start_screen, current_screen } = state.select_state {
        if ctrl_slant {
            state.show_bounding_box_slanted = true;
            let (p0, p1) = (state.viewport.to_font(start_screen), state.viewport.to_font(current_screen));
            let mut deg = (p1.x.0 - p0.x.0).atan2((p1.y.0 - p0.y.0).abs().max(1.0)).to_degrees()
                .clamp(-crate::tokens::MARQUEE_MAX_SLANT_DEG, crate::tokens::MARQUEE_MAX_SLANT_DEG);
            if (deg - state.italic_angle_deg).abs() < crate::tokens::MARQUEE_ITALIC_SNAP_TOL_DEG {
                deg = state.italic_angle_deg;
            } else if state.marquee_modifiers.shift {
                deg = deg.round();
            }
            state.selection_angle_deg = deg;
        } else if only_on_curve && state.italic_angle_deg.abs() > 0.0 {
            state.selection_angle_deg = state.italic_angle_deg;
            state.show_bounding_box_slanted = true;
        } else {
            state.selection_angle_deg = 0.0;
            state.show_bounding_box_slanted = false;
        }
        if (ctrl_slant || only_on_curve) && state.selection_angle_deg.abs() > 0.0 {
            state.selected_nodes = crate::core::select_nodes_in_slanted_marquee(
                &state.contours, &state.viewport, &theme.edit_canvas, start_screen, current_screen, state.selection_angle_deg, &state.marquee_pre_selection, xor_toggle, only_on_curve,
            );
            return;
        }
    }
    let Some(rect) = state.select_state.marquee_rect() else { return; };
    let mut hits: Vec<NodeId> = Vec::new();
    for contour in &state.contours {
        for node in contour {
            if only_on_curve && node.node_type == NodeType::OffCurve {
                continue;
            }
            let proj = ProjectedNode::new(node.point, node.node_type, false, &state.viewport, &theme.edit_canvas);
            let center = Point::new(proj.screen_pos.x.0 as f64, proj.screen_pos.y.0 as f64);
            let inter = rect.intersect(proj.bounds);
            if (rect.contains(center) || (inter.width() > 0.0 && inter.height() > 0.0)) && !hits.contains(&node.id) {
                hits.push(node.id);
            }
        }
    }
    if xor_toggle {
        let mut next_sel: Vec<NodeId> = state
            .marquee_pre_selection
            .iter()
            .copied()
            .filter(|id| !hits.contains(id))
            .collect();
        for id in hits {
            if !state.marquee_pre_selection.contains(&id) {
                next_sel.push(id);
            }
        }
        state.selected_nodes = next_sel;
    } else {
        state.selected_nodes = hits;
    }
}

pub fn apply_segment_click_selection(
    state: &mut GlifoCanvasState,
    c_idx: usize,
    seg: ContourSegment,
    mods: Modifiers,
) {
    let Some(contour) = state.contours.get(c_idx) else { return; };
    let target_ids: Vec<NodeId> = match seg {
        ContourSegment::Line { start_idx, end_idx, .. } => vec![contour[start_idx].id, contour[end_idx].id],
        ContourSegment::Cubic { start_idx, h1_idx, h2_idx, end_idx, .. } => {
            vec![contour[start_idx].id, contour[h1_idx].id, contour[h2_idx].id, contour[end_idx].id]
        }
    };
    if mods.shift {
        let mut sel = state.marquee_pre_selection.clone();
        if target_ids.iter().all(|id| sel.contains(id)) {
            sel.retain(|id| !target_ids.contains(id));
        } else {
            for id in target_ids {
                if !sel.contains(&id) {
                    sel.push(id);
                }
            }
        }
        state.selected_nodes = sel;
    } else {
        state.selected_nodes = target_ids;
    }
}

/// Duplicates any contour(s) containing selected nodes when `Alt/Option + Drag` starts on a
/// multi-node selection (`optionDragDuplicate` / `_duplicateSelection`, `SPEC-002 §3.1.8`).
pub fn duplicate_selected_contours_for_alt_drag(state: &mut GlifoCanvasState) {
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
                cloned_nodes.push(crate::core::Node::new(new_id, n.point.x.0, n.point.y.0, n.node_type));
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
    if let SelectToolState::DraggingNodes { start_pt, current_pt, modifiers, .. } = state.select_state.clone() {
        state.select_state = SelectToolState::DraggingNodes { selected: new_selected, start_pt, current_pt, modifiers };
    }
}

impl GlifoCanvasState {
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
        for id in expanded_nodes_with_attached_handles(&self.contours, &closed_flags, &self.selected_nodes, true) {
            if let Some(node) = self.find_node(id) {
                self.drag_start_font_positions.push((id, node.point));
            }
        }
    }

    pub fn apply_node_displacement(&mut self, dx_font: f64, dy_font: f64, shift_snap: bool) {
        self.apply_node_displacement_with_modifiers_and_ctrl(dx_font, dy_font, shift_snap, false, false);
    }

    pub fn apply_node_displacement_with_modifiers(&mut self, dx_font: f64, dy_font: f64, shift_snap: bool, alt_held: bool) {
        self.apply_node_displacement_with_modifiers_and_ctrl(dx_font, dy_font, shift_snap, alt_held, false);
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
        let (snapped_dx, snapped_dy) = if shift_snap { snap_vector_45_degrees(dx_font, dy_font) } else { (dx_font, dy_font) };
        let selected_copy = self.selected_nodes.clone();
        let closed_flags = self.closed_flags();
        let shadow_contours = if self.drag_start_contours.is_empty() { self.contours.clone() } else { self.drag_start_contours.clone() };
        let has_on_curve_selected = selected_copy.iter().any(|&id| self.find_node(id).is_some_and(|n| n.node_type != NodeType::OffCurve));

        if ctrl_held && !alt_held && selected_copy.len() == 1 && has_on_curve_selected {
            let sel_id = selected_copy[0];
            let start_pt = self
                .drag_start_font_positions
                .iter()
                .find(|&&(id, _)| id == sel_id)
                .map_or_else(|| self.find_node(sel_id).unwrap().point, |&(_, p)| p);
            let target_pt = FontPoint::new(start_pt.x.0 + snapped_dx, start_pt.y.0 + snapped_dy);
            for (c_idx, contour) in self.contours.iter_mut().enumerate() {
                if slide_smooth_node_on_cubic_path_shift_middle(contour, &shadow_contours[c_idx], closed_flags[c_idx], sel_id, snapped_dx, snapped_dy)
                    || slide_on_curve_node_along_contour(contour, closed_flags[c_idx], sel_id, target_pt)
                {
                    break;
                }
            }
            self.sync_paths_from_contours();
            return;
        }

        if alt_held && !ctrl_held && selected_copy.len() == 1 && has_on_curve_selected {
            let sel_id = selected_copy[0];
            for (c_idx, contour) in self.contours.iter_mut().enumerate() {
                if slide_smooth_line_node_unlimited(contour, &shadow_contours[c_idx], closed_flags[c_idx], sel_id, snapped_dx, snapped_dy) {
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
                apply_ctrl_alt_interpolate_nudge(&mut self.contours, &shadow_contours, &closed_flags, &selected_copy, snapped_dx, snapped_dy);
            } else {
                for &sel_id in &selected_copy {
                    for (c_idx, contour) in self.contours.iter_mut().enumerate() {
                        apply_ctrl_alt_mirrored_handle_drag(contour, closed_flags[c_idx], sel_id, &selected_copy);
                    }
                }
            }
            self.sync_paths_from_contours();
            return;
        }

        if let [sel_id] = selected_copy.as_slice() {
            if self.find_node(*sel_id).is_some_and(|n| n.node_type == NodeType::OffCurve) {
                for (c_idx, contour) in self.contours.iter_mut().enumerate() {
                    enforce_smooth_collinearity_on_handle_drag(contour, closed_flags[c_idx], *sel_id, alt_held);
                }
            }
        }
        self.sync_paths_from_contours();
    }
}
