//! `SPEC-003` (`v0.2.0`) pointer and keyboard interaction handlers for 8-Knob Bounding Box,
//! `RotateTool` (`R`), `ScaleTool` (`S`), `LassoTool` (`Shift+V`), and `ZoomTool` (`Z`).

use crate::adapters::select_interaction::handle_select_press;
use crate::adapters::state::{CanvasTool, GlifoCanvasState};
use crate::adapters::v020_tools::V020DragState;
use crate::core::{
    apply_mode_nodes_transform, compute_bbox_rotate_or_slant_affine, compute_bbox_scale_affine,
    compute_marquee_zoom, compute_rotate_tool_affine, compute_scale_tool_affine, next_zoom_step,
    prev_zoom_step, select_nodes_in_lasso_polygon, BBoxAction, FontPoint, NodeId, NodeType,
    ScreenPoint,
};
use crate::interaction::{Modifiers, SelectToolState};
use crate::tokens::{
    DOUBLE_CLICK_MAX_DELAY_SEC, DOUBLE_CLICK_MAX_DIST_DP, LASSO_MIN_SAMPLE_STEP_DP,
    PEN_NODE_HIT_RADIUS_DP, SELECT_NODE_HIT_RADIUS_DP, TRANSFORM_CLICK_THRESHOLD_DP,
    TRANSFORM_MIN_PIVOT_DIST_UPM, TRANSFORM_ORIGIN_HIT_RADIUS_DP,
};
use egui::{Key, Rect};

impl GlifoCanvasState {
    pub fn check_triple_click(&mut self, scr_pt: ScreenPoint, current_time: f64) -> bool {
        if let Some((last_dbl_time, last_dbl_pos)) = self.last_double_click_time_pos {
            if (current_time - last_dbl_time).abs() <= DOUBLE_CLICK_MAX_DELAY_SEC
                && last_dbl_pos.distance_to(scr_pt) <= DOUBLE_CLICK_MAX_DIST_DP
            {
                self.last_double_click_time_pos = None;
                self.last_click_time_pos = None;
                return true;
            }
        }
        false
    }

    pub fn check_double_click(&mut self, scr_pt: ScreenPoint, current_time: f64) -> bool {
        if let Some((last_time, last_pos)) = self.last_click_time_pos {
            if (current_time - last_time).abs() <= DOUBLE_CLICK_MAX_DELAY_SEC
                && last_pos.distance_to(scr_pt) <= DOUBLE_CLICK_MAX_DIST_DP
            {
                self.last_click_time_pos = None;
                self.last_double_click_time_pos = Some((current_time, scr_pt));
                return true;
            }
        }
        self.last_click_time_pos = Some((current_time, scr_pt));
        false
    }

    #[must_use]
    pub fn hit_test_node_with_radius(&self, screen_pt: ScreenPoint, radius_dp: f32) -> Option<NodeId> {
        let mut best: Option<((u8, u32), NodeId)> = None;
        for (c_idx, contour) in self.contours.iter().enumerate() {
            let is_open = !self.is_contour_closed(c_idx);
            let len = contour.len();
            for (n_idx, node) in contour.iter().enumerate() {
                let dist = self.viewport.to_screen(node.point).distance_to(screen_pt);
                if dist >= radius_dp { continue; }
                let is_sel = self.selected_nodes.contains(&node.id);
                let is_open_ep = is_open && (n_idx == 0 || n_idx + 1 == len);
                let tier = match (node.node_type == NodeType::OffCurve, is_open_ep, is_sel) {
                    (true, _, true) => 0, (true, _, false) => 1,
                    (false, true, true) => 2, (false, true, false) => 3,
                    (false, false, true) => 4, (false, false, false) => 5,
                };
                let key = (tier, dist.to_bits());
                if best.is_none_or(|(b_key, _)| key < b_key) { best = Some((key, node.id)); }
            }
        }
        best.map(|(_, id)| id)
    }
    #[must_use]
    pub fn hit_test_node(&self, screen_pt: ScreenPoint) -> Option<NodeId> {
        self.hit_test_node_with_radius(screen_pt, SELECT_NODE_HIT_RADIUS_DP)
    }
    #[must_use]
    pub fn hit_test_pen_node(&self, screen_pt: ScreenPoint) -> Option<NodeId> {
        self.hit_test_node_with_radius(screen_pt, PEN_NODE_HIT_RADIUS_DP)
    }
}

pub fn dispatch_v020_keyboard_shortcut(
    state: &mut GlifoCanvasState,
    key: Key,
    shift_active: bool,
    alt_active: bool,
    cmd_active: bool,
) -> bool {
    match key {
        Key::B if cmd_active && alt_active && shift_active => { state.show_bounding_box = !state.show_bounding_box; true }
        Key::L if cmd_active && shift_active && !alt_active => { state.show_guides = !state.show_guides; true }
        Key::V if shift_active && !cmd_active => { state.activate_tool(CanvasTool::Lasso); true }
        Key::R if !cmd_active && !shift_active => { state.activate_tool(CanvasTool::Rotate); true }
        Key::S if !cmd_active && !shift_active => { state.activate_tool(CanvasTool::Scale); true }
        Key::Z if !cmd_active && !shift_active => { state.activate_tool(CanvasTool::Zoom); true }
        Key::C if cmd_active && !shift_active => state.copy_selection(),
        Key::X if cmd_active && !shift_active => state.cut_selection(),
        Key::V if cmd_active && !shift_active => state.paste_clipboard(),
        Key::T if cmd_active && shift_active => state.tidy_up_paths(),
        Key::A if cmd_active && shift_active => state.align_selection(),
        Key::R if cmd_active && shift_active => state.correct_path_direction(),
        _ => false,
    }
}

pub fn handle_v020_pointer_press(
    state: &mut GlifoCanvasState,
    scr_pt: ScreenPoint,
    font_pt: FontPoint,
    mods: Modifiers,
    is_double: bool,
    is_triple: bool,
) -> Option<bool> {
    if mods.space && (mods.command || mods.control) && state.active_tool != CanvasTool::Zoom {
        if state.temp_tool_before_override.is_none() {
            state.temp_tool_before_override = Some(state.active_tool);
        }
        state.active_tool = CanvasTool::Zoom;
    }

    if matches!(
        state.active_tool,
        CanvasTool::Rotate | CanvasTool::Scale | CanvasTool::Lasso | CanvasTool::Zoom
    ) && mods.command
        && !mods.space
        && !state.temp_zoom_via_key_space
    {
        state.temp_tool_before_override = Some(state.active_tool);
        state.active_tool = CanvasTool::Select;
        return Some(handle_select_press(state, scr_pt, font_pt, mods, is_double, is_triple));
    }

    if matches!(state.active_tool, CanvasTool::Select | CanvasTool::Lasso) {
        if let Some(action) = state.hit_test_bbox_knob(scr_pt, mods.control, mods.option) {
            if let Some(shadow_bounds) = state.selection_bounds() {
                state.active_drag_before_snapshot = Some(state.snapshot());
                state.record_drag_start();
                state.v020_drag = V020DragState::BBoxDragging {
                    action,
                    start_font: font_pt,
                    current_font: font_pt,
                    shadow_bounds,
                };
                return Some(true);
            }
        }
        if state.hit_test_node(scr_pt).is_none() {
            if let Some(handled) = state.try_begin_guide_press(scr_pt, font_pt, is_double) {
                return Some(handled);
            }
        } else {
            state.selected_guide = None;
        }
    }

    match state.active_tool {
        CanvasTool::Rotate | CanvasTool::Scale => {
            let origin_scr = state.viewport.to_screen(state.transform_origin);
            if scr_pt.distance_to(origin_scr) < TRANSFORM_ORIGIN_HIT_RADIUS_DP {
                let step = state.effective_grid_step();
                state.transform_origin = if !mods.control && step > 0.0 { font_pt.round_to_grid_step(step) } else { font_pt };
                state.transform_origin_custom = true;
                state.v020_drag = V020DragState::TransformOriginDragging { start_scr: scr_pt };
            } else {
                state.v020_drag = V020DragState::TransformPendingOrDragging {
                    is_rotate: state.active_tool == CanvasTool::Rotate,
                    start_scr: scr_pt,
                    current_scr: scr_pt,
                    start_font: font_pt,
                    current_font: font_pt,
                    active: false,
                };
            }
            Some(true)
        }
        CanvasTool::Lasso => {
            if state.hit_test_node(scr_pt).is_some() || is_triple {
                return Some(handle_select_press(state, scr_pt, font_pt, mods, is_double, is_triple));
            }
            let initial_selected = state.selected_nodes.clone();
            if !mods.shift {
                state.selected_nodes.clear();
            }
            if mods.control {
                state.show_bounding_box_slanted = true;
            }
            state.v020_drag = V020DragState::LassoDragging {
                points_scr: vec![scr_pt],
                points_font: vec![font_pt],
                initial_selected,
            };
            Some(true)
        }
        CanvasTool::Zoom => {
            state.v020_drag = V020DragState::ZoomDragging { start_scr: scr_pt, current_scr: scr_pt };
            Some(true)
        }
        _ => None,
    }
}

pub fn handle_v020_pointer_move(
    state: &mut GlifoCanvasState,
    scr_pt: ScreenPoint,
    font_pt: FontPoint,
    is_shift: bool,
    is_alt: bool,
    is_ctrl: bool,
    is_space: bool,
) -> Option<bool> {
    if is_space {
        if let SelectToolState::MarqueeSelecting { start_screen, current_screen } = state.select_state {
            let (dx, dy) = (scr_pt.x.0 - current_screen.x.0, scr_pt.y.0 - current_screen.y.0);
            state.select_state = SelectToolState::MarqueeSelecting {
                start_screen: ScreenPoint::new(start_screen.x.0 + dx, start_screen.y.0 + dy),
                current_screen: scr_pt,
            };
            let theme = state.theme.clone();
            crate::adapters::select_interaction::apply_marquee_selection(state, &theme, is_alt);
            return Some(true);
        }
    }

    if state.guide_drag != crate::adapters::v030_guides::GuideDragState::Idle {
        return Some(state.update_guide_drag(font_pt, is_shift, is_alt, is_ctrl));
    }
    match state.v020_drag.clone() {
        V020DragState::Idle => None,
        V020DragState::BBoxDragging { action, start_font, shadow_bounds, .. } => {
            state.v020_drag = V020DragState::BBoxDragging { action, start_font, current_font: font_pt, shadow_bounds };
            state.contours = state.drag_start_contours.clone();
            let aff = match action {
                BBoxAction::Scale(k) => compute_bbox_scale_affine(&shadow_bounds, k, start_font, font_pt, is_shift, is_alt, is_ctrl),
                BBoxAction::Rotate(_) | BBoxAction::Slant(_) => compute_bbox_rotate_or_slant_affine(&shadow_bounds, action, start_font, font_pt, is_shift, is_alt, is_ctrl),
            };
            let (flags, sel) = (state.closed_flags(), state.selected_nodes.clone());
            let grid = if is_ctrl { 0.0 } else { state.effective_grid_step() };
            apply_mode_nodes_transform(&mut state.contours, &state.drag_start_contours, &flags, &sel, aff, is_ctrl && is_alt, grid);
            state.sync_paths_from_contours();
            Some(true)
        }
        V020DragState::TransformOriginDragging { .. } => {
            let step = state.effective_grid_step();
            state.transform_origin = if !is_ctrl && step > 0.0 { font_pt.round_to_grid_step(step) } else { font_pt };
            state.transform_origin_custom = true;
            Some(true)
        }
        V020DragState::TransformPendingOrDragging {
            is_rotate, start_scr, start_font, mut active, ..
        } => {
            let pivot_dist_ok = if is_rotate {
                start_font.distance_to(state.transform_origin) > TRANSFORM_MIN_PIVOT_DIST_UPM
            } else {
                (start_font.x.0 - state.transform_origin.x.0).abs() > TRANSFORM_MIN_PIVOT_DIST_UPM
                    && (start_font.y.0 - state.transform_origin.y.0).abs() > TRANSFORM_MIN_PIVOT_DIST_UPM
            };
            if !active
                && scr_pt.distance_to(start_scr) > TRANSFORM_CLICK_THRESHOLD_DP
                && pivot_dist_ok
            {
                state.active_drag_before_snapshot = Some(state.snapshot());
                state.record_drag_start();
                active = true;
            }
            state.v020_drag = V020DragState::TransformPendingOrDragging {
                is_rotate, start_scr, current_scr: scr_pt, start_font, current_font: font_pt, active,
            };
            if active {
                state.contours = state.drag_start_contours.clone();
                let aff = if is_rotate {
                    compute_rotate_tool_affine(state.transform_origin, start_font, font_pt, is_shift)
                } else {
                    compute_scale_tool_affine(state.transform_origin, start_font, font_pt, is_shift)
                };
                let flags = state.closed_flags();
                let target_ids: Vec<NodeId> = if !state.selected_nodes.is_empty() {
                    state.selected_nodes.clone()
                } else {
                    state.contours.iter().flat_map(|c| c.iter().map(|n| n.id)).collect()
                };
                let grid = if is_ctrl { 0.0 } else { state.effective_grid_step() };
                apply_mode_nodes_transform(&mut state.contours, &state.drag_start_contours, &flags, &target_ids, aff, is_ctrl && is_alt, grid);
                state.sync_paths_from_contours();
            }
            Some(true)
        }
        V020DragState::LassoDragging { mut points_scr, mut points_font, initial_selected } => {
            if points_scr.last().is_none_or(|p| p.distance_to(scr_pt) > LASSO_MIN_SAMPLE_STEP_DP) {
                points_scr.push(scr_pt);
                points_font.push(font_pt);
            }
            state.selected_nodes = select_nodes_in_lasso_polygon(&state.contours, &points_font, &initial_selected, is_shift, is_alt);
            if is_ctrl { state.show_bounding_box_slanted = true; }
            state.v020_drag = V020DragState::LassoDragging { points_scr, points_font, initial_selected };
            Some(true)
        }
        V020DragState::ZoomDragging { start_scr, .. } => {
            state.v020_drag = V020DragState::ZoomDragging { start_scr, current_scr: scr_pt };
            Some(true)
        }
    }
}

pub fn restore_temp_tool_override(state: &mut GlifoCanvasState) {
    if state.temp_zoom_via_key_space {
        return;
    }
    if state.active_tool == CanvasTool::Measurement
        && state.last_modifiers.command
        && state.last_modifiers.control
        && state.last_modifiers.option
        && !state.last_modifiers.space
    {
        return;
    }
    if let Some(prev_tool) = state.temp_tool_before_override.take() {
        state.active_tool = prev_tool;
        if matches!(prev_tool, CanvasTool::Rotate | CanvasTool::Scale) {
            state.transform_origin_custom = false;
            state.update_transform_origin();
        }
    }
}

pub fn handle_v020_pointer_release(
    state: &mut GlifoCanvasState,
    is_shift: bool,
    is_alt: bool,
    is_ctrl: bool,
    canvas_rect: Rect,
) -> Option<bool> {
    if state.guide_drag != crate::adapters::v030_guides::GuideDragState::Idle {
        state.guide_drag = crate::adapters::v030_guides::GuideDragState::Idle;
        state.active_snappers.clear();
        if let Some(before) = state.active_drag_before_snapshot.take() {
            state.record_mutation("Guide Transform", before);
        }
        state.assert_invariants();
        return Some(true);
    }
    match std::mem::take(&mut state.v020_drag) {
        V020DragState::Idle => None,
        V020DragState::BBoxDragging { .. } => {
            if let Some(before) = state.active_drag_before_snapshot.take() {
                state.record_mutation("Bounding Box Transform", before);
            }
            state.assert_invariants();
            Some(true)
        }
        V020DragState::TransformOriginDragging { .. } => {
            state.assert_invariants();
            Some(true)
        }
        V020DragState::TransformPendingOrDragging {
            is_rotate, start_scr, current_scr, start_font, active, ..
        } => {
            if active {
                if let Some(before) = state.active_drag_before_snapshot.take() {
                    state.record_mutation(if is_rotate { "Rotate Tool" } else { "Scale Tool" }, before);
                }
            } else if current_scr.distance_to(start_scr) <= TRANSFORM_CLICK_THRESHOLD_DP {
                let step = state.effective_grid_step();
                state.transform_origin = if !is_ctrl && step > 0.0 { start_font.round_to_grid_step(step) } else { start_font };
                state.transform_origin_custom = true;
            }
            state.assert_invariants();
            Some(true)
        }
        V020DragState::LassoDragging { points_font, initial_selected, .. } => {
            state.selected_nodes = select_nodes_in_lasso_polygon(
                &state.contours, &points_font, &initial_selected, is_shift, is_alt,
            );
            state.assert_invariants();
            Some(true)
        }
        V020DragState::ZoomDragging { start_scr, current_scr } => {
            if let Some((new_zoom, center_scr)) = compute_marquee_zoom(
                state.viewport.zoom,
                start_scr,
                current_scr,
                canvas_rect.width(),
                canvas_rect.height(),
                is_alt,
            ) {
                let center_font = state.viewport.to_font(center_scr);
                state.viewport.zoom = new_zoom;
                let after_scr = state.viewport.to_screen(center_font);
                let view_center = ScreenPoint::new(canvas_rect.center().x, canvas_rect.center().y);
                state.viewport.pan_offset.x.0 += view_center.x.0 - after_scr.x.0;
                state.viewport.pan_offset.y.0 += view_center.y.0 - after_scr.y.0;
            } else {
                let anchor_font = state.viewport.to_font(start_scr);
                let next_z = if is_alt {
                    prev_zoom_step(state.viewport.zoom)
                } else {
                    next_zoom_step(state.viewport.zoom)
                };
                state.viewport.zoom = next_z;
                let after_scr = state.viewport.to_screen(anchor_font);
                state.viewport.pan_offset.x.0 += start_scr.x.0 - after_scr.x.0;
                state.viewport.pan_offset.y.0 += start_scr.y.0 - after_scr.y.0;
            }
            state.assert_invariants();
            Some(true)
        }
    }
}
