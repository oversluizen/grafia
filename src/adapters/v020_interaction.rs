//! `SPEC-003` (`v0.2.0`) pointer and keyboard interaction handlers for 8-Knob Bounding Box,
//! `RotateTool` (`R`), `ScaleTool` (`S`), `LassoTool` (`Shift+V`), and `ZoomTool` (`Z`).

use crate::adapters::select_interaction::handle_select_press;
use crate::adapters::state::{CanvasTool, GrafiaCanvasState};
pub use crate::adapters::v020_drag_ops::{
    handle_v020_pointer_move, handle_v020_pointer_release, restore_temp_tool_override,
};
use crate::adapters::v020_tools::V020DragState;
use crate::core::{FontPoint, NodeId, NodeType, ScreenPoint};
use crate::interaction::Modifiers;
use crate::tokens::{
    DOUBLE_CLICK_MAX_DELAY_SEC, DOUBLE_CLICK_MAX_DIST_DP, PEN_NODE_HIT_RADIUS_DP,
    SELECT_NODE_HIT_RADIUS_DP, TRANSFORM_ORIGIN_HIT_RADIUS_DP,
};
use egui::Key;

impl GrafiaCanvasState {
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
    pub fn hit_test_node_with_radius(
        &self,
        screen_pt: ScreenPoint,
        radius_dp: f32,
    ) -> Option<NodeId> {
        let vp = self.active_slot_viewport();
        let mut best: Option<((u8, u32), NodeId)> = None;
        for (c_idx, contour) in self.contours.iter().enumerate() {
            let is_open = !self.is_contour_closed(c_idx);
            let len = contour.len();
            for (n_idx, node) in contour.iter().enumerate() {
                let dist = vp.to_screen(node.point).distance_to(screen_pt);
                if dist >= radius_dp {
                    continue;
                }
                let is_sel = self.selected_nodes.contains(&node.id);
                let is_open_ep = is_open && (n_idx == 0 || n_idx + 1 == len);
                let tier = match (node.node_type == NodeType::OffCurve, is_open_ep, is_sel) {
                    (true, _, true) => 0,
                    (true, _, false) => 1,
                    (false, true, true) => 2,
                    (false, true, false) => 3,
                    (false, false, true) => 4,
                    (false, false, false) => 5,
                };
                let key = (tier, dist.to_bits());
                if best.is_none_or(|(b_key, _)| key < b_key) {
                    best = Some((key, node.id));
                }
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
    state: &mut GrafiaCanvasState,
    key: Key,
    shift_active: bool,
    alt_active: bool,
    cmd_active: bool,
) -> bool {
    match key {
        Key::B if cmd_active && alt_active && shift_active => {
            state.show_bounding_box = !state.show_bounding_box;
            true
        }
        Key::L if cmd_active && shift_active && !alt_active => {
            state.show_guides = !state.show_guides;
            true
        }
        Key::V if shift_active && !cmd_active => {
            state.activate_tool(CanvasTool::Lasso);
            true
        }
        Key::R if !cmd_active && !shift_active => {
            state.activate_tool(CanvasTool::Rotate);
            true
        }
        Key::S if !cmd_active && !shift_active => {
            state.activate_tool(CanvasTool::Scale);
            true
        }
        Key::Z if !cmd_active && !shift_active => {
            state.activate_tool(CanvasTool::Zoom);
            true
        }
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
    state: &mut GrafiaCanvasState,
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
        return Some(handle_select_press(
            state, scr_pt, font_pt, mods, is_double, is_triple,
        ));
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
                state.transform_origin = if !mods.control && step > 0.0 {
                    font_pt.round_to_grid_step(step)
                } else {
                    font_pt
                };
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
                return Some(handle_select_press(
                    state, scr_pt, font_pt, mods, is_double, is_triple,
                ));
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
            state.v020_drag = V020DragState::ZoomDragging {
                start_scr: scr_pt,
                current_scr: scr_pt,
            };
            Some(true)
        }
        _ => None,
    }
}
