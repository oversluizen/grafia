//! `SPEC-003` (`v0.2.0`) pointer and keyboard interaction handlers for 8-Knob Bounding Box,
//! `RotateTool` (`R`), `ScaleTool` (`S`), `LassoTool` (`Shift+V`), and `ZoomTool` (`Z`).

pub use crate::adapters::bbox_drag_ops::handle_bbox_and_transform_pointer_move;
use crate::adapters::bbox_selection_tools::BBoxDragState;
pub use crate::adapters::multi_selection_ops::handle_bbox_and_transform_pointer_release;
use crate::adapters::select_interaction::handle_select_press;
use crate::adapters::state::{CanvasTool, GrafiaCanvasState};
use crate::core::{
    compute_marquee_zoom, next_zoom_step, prev_zoom_step, FontPoint, NodeId, NodeType, ScreenPoint,
};
use crate::interaction::{Modifiers, SelectToolState};
use crate::tokens::{
    DOUBLE_CLICK_MAX_DELAY_SEC, DOUBLE_CLICK_MAX_DIST_DP, TRANSFORM_CLICK_THRESHOLD_DP,
    TRANSFORM_ORIGIN_HIT_RADIUS_DP,
};
use egui::{Key, Rect};

pub fn restore_temp_tool_override(state: &mut GrafiaCanvasState) {
    if state.drag_transients.temp_zoom_via_key_space {
        return;
    }
    if state.active_tool == CanvasTool::Measurement
        && state.drag_transients.last_modifiers.command
        && state.drag_transients.last_modifiers.control
        && state.drag_transients.last_modifiers.option
        && !state.drag_transients.last_modifiers.space
    {
        return;
    }
    if let Some(prev_tool) = state.drag_transients.temp_tool_before_override.take() {
        state.active_tool = prev_tool;
        if matches!(prev_tool, CanvasTool::Rotate | CanvasTool::Scale) {
            state.transform_origin_custom = false;
            state.update_transform_origin();
        }
    }
}

impl GrafiaCanvasState {
    pub fn check_triple_click(&mut self, scr_pt: ScreenPoint, current_time: f64) -> bool {
        if let Some((last_dbl_time, last_dbl_pos)) = self.drag_transients.last_double_click_time_pos
        {
            if (current_time - last_dbl_time).abs() <= DOUBLE_CLICK_MAX_DELAY_SEC
                && last_dbl_pos.distance_to(scr_pt) <= DOUBLE_CLICK_MAX_DIST_DP
            {
                self.drag_transients.last_double_click_time_pos = None;
                self.drag_transients.last_click_time_pos = None;
                return true;
            }
        }
        false
    }

    pub fn check_double_click(&mut self, scr_pt: ScreenPoint, current_time: f64) -> bool {
        if let Some((last_time, last_pos)) = self.drag_transients.last_click_time_pos {
            if (current_time - last_time).abs() <= DOUBLE_CLICK_MAX_DELAY_SEC
                && last_pos.distance_to(scr_pt) <= DOUBLE_CLICK_MAX_DIST_DP
            {
                self.drag_transients.last_click_time_pos = None;
                self.drag_transients.last_double_click_time_pos = Some((current_time, scr_pt));
                return true;
            }
        }
        self.drag_transients.last_click_time_pos = Some((current_time, scr_pt));
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
        self.hit_test_node_with_radius(screen_pt, self.preferences.hit_test_radius_dp)
    }
}

pub fn dispatch_transform_and_clipboard_shortcut(
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

pub fn handle_bbox_and_transform_pointer_press(
    state: &mut GrafiaCanvasState,
    scr_pt: ScreenPoint,
    font_pt: FontPoint,
    mods: Modifiers,
    is_double: bool,
    is_triple: bool,
) -> Option<bool> {
    if mods.space && (mods.command || mods.control) && state.active_tool != CanvasTool::Zoom {
        if state.drag_transients.temp_tool_before_override.is_none() {
            state.drag_transients.temp_tool_before_override = Some(state.active_tool);
        }
        state.active_tool = CanvasTool::Zoom;
    }
    if matches!(
        state.active_tool,
        CanvasTool::Rotate | CanvasTool::Scale | CanvasTool::Lasso | CanvasTool::Zoom
    ) && mods.command
        && !mods.space
        && !state.drag_transients.temp_zoom_via_key_space
    {
        state.drag_transients.temp_tool_before_override = Some(state.active_tool);
        state.active_tool = CanvasTool::Select;
        return Some(handle_select_press(
            state, scr_pt, font_pt, mods, is_double, is_triple,
        ));
    }
    if matches!(state.active_tool, CanvasTool::Select | CanvasTool::Lasso) {
        if let Some(handled) =
            handle_select_or_lasso_bbox_and_guide_press(state, scr_pt, font_pt, mods, is_double)
        {
            return Some(handled);
        }
    }
    match state.active_tool {
        CanvasTool::Rotate | CanvasTool::Scale => {
            start_rotate_or_scale_press(state, scr_pt, font_pt, mods);
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
            state.show_bounding_box_slanted |= mods.control;
            state.bbox_drag = BBoxDragState::LassoDragging {
                points_scr: vec![scr_pt],
                points_font: vec![font_pt],
                initial_selected,
            };
            Some(true)
        }
        CanvasTool::Zoom => {
            state.bbox_drag = BBoxDragState::ZoomDragging {
                start_scr: scr_pt,
                current_scr: scr_pt,
            };
            Some(true)
        }
        _ => None,
    }
}

fn start_rotate_or_scale_press(
    state: &mut GrafiaCanvasState,
    scr_pt: ScreenPoint,
    font_pt: FontPoint,
    mods: Modifiers,
) {
    let origin_scr = state.viewport.to_screen(state.transform_origin);
    if scr_pt.distance_to(origin_scr) < TRANSFORM_ORIGIN_HIT_RADIUS_DP {
        let step = state.effective_grid_step();
        state.transform_origin = if !mods.control && step > 0.0 {
            font_pt.round_to_grid_step(step)
        } else {
            font_pt
        };
        state.transform_origin_custom = true;
        state.bbox_drag = BBoxDragState::TransformOriginDragging { start_scr: scr_pt };
    } else {
        state.bbox_drag = BBoxDragState::TransformPendingOrDragging {
            is_rotate: state.active_tool == CanvasTool::Rotate,
            start_scr: scr_pt,
            current_scr: scr_pt,
            start_font: font_pt,
            current_font: font_pt,
            active: false,
        };
    }
}

fn handle_select_or_lasso_bbox_and_guide_press(
    state: &mut GrafiaCanvasState,
    scr_pt: ScreenPoint,
    font_pt: FontPoint,
    mods: Modifiers,
    is_double: bool,
) -> Option<bool> {
    if let Some(action) = state.hit_test_bbox_knob(scr_pt, mods.control, mods.option) {
        if let Some(shadow_bounds) = state.selection_bounds() {
            state.drag_transients.active_drag_before_snapshot = Some(state.snapshot());
            state.record_drag_start();
            state.active_bbox_rotation_deg =
                matches!(action, crate::core::BBoxAction::Rotate(_)).then_some(0.0);
            state.bbox_drag = BBoxDragState::BBoxDragging {
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
    None
}

pub(crate) fn pan_marquee_while_space_held(
    state: &mut GrafiaCanvasState,
    scr_pt: ScreenPoint,
    is_alt: bool,
) -> bool {
    let SelectToolState::MarqueeSelecting {
        start_screen,
        current_screen,
    } = state.select_state
    else {
        return false;
    };
    let dx = scr_pt.x.0 - current_screen.x.0;
    let dy = scr_pt.y.0 - current_screen.y.0;
    state.select_state = SelectToolState::MarqueeSelecting {
        start_screen: ScreenPoint::new(start_screen.x.0 + dx, start_screen.y.0 + dy),
        current_screen: scr_pt,
    };
    crate::adapters::select_interaction::apply_marquee_selection(state, is_alt);
    true
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct TransformReleaseFlags {
    pub is_rotate: bool,
    pub active: bool,
    pub is_ctrl: bool,
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct TransformReleaseCoords {
    pub start_scr: ScreenPoint,
    pub current_scr: ScreenPoint,
    pub start_font: FontPoint,
}

pub(crate) fn finish_transform_tool_release(
    state: &mut GrafiaCanvasState,
    flags: TransformReleaseFlags,
    coords: TransformReleaseCoords,
) {
    let TransformReleaseFlags {
        is_rotate,
        active,
        is_ctrl,
    } = flags;
    let TransformReleaseCoords {
        start_scr,
        current_scr,
        start_font,
    } = coords;
    if active {
        if let Some(before) = state.drag_transients.active_drag_before_snapshot.take() {
            let label = if is_rotate {
                "Rotate Tool"
            } else {
                "Scale Tool"
            };
            state.record_mutation(label, before);
        }
    } else if current_scr.distance_to(start_scr) <= TRANSFORM_CLICK_THRESHOLD_DP {
        let step = state.effective_grid_step();
        state.transform_origin = if !is_ctrl && step > 0.0 {
            start_font.round_to_grid_step(step)
        } else {
            start_font
        };
        state.transform_origin_custom = true;
    }
    state.assert_invariants();
}

pub(crate) fn finish_zoom_drag_release(
    state: &mut GrafiaCanvasState,
    start_scr: ScreenPoint,
    current_scr: ScreenPoint,
    canvas_rect: Rect,
    is_alt: bool,
) -> bool {
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
    true
}
