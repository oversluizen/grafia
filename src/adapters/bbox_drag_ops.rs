//! `SPEC-003` (`v0.2.0`) pointer drag move, temporary tool override restoration, and pointer release handlers.

use crate::adapters::bbox_selection_tools::BBoxDragState;
use crate::adapters::contour_edit_interaction::pan_marquee_while_space_held;
use crate::adapters::state::GrafiaCanvasState;
use crate::core::{
    apply_mode_nodes_transform, compute_bbox_rotate_or_slant_affine, compute_bbox_scale_affine,
    compute_rotate_tool_affine, compute_scale_tool_affine, select_nodes_in_lasso_polygon,
    BBoxAction, FontPoint, NodeId, ScreenPoint,
};
use crate::tokens::{
    LASSO_MIN_SAMPLE_STEP_DP, TRANSFORM_CLICK_THRESHOLD_DP, TRANSFORM_MIN_PIVOT_DIST_UPM,
};

pub use crate::adapters::contour_edit_interaction::{
    handle_bbox_and_transform_pointer_release, restore_temp_tool_override,
};

#[derive(Debug, Clone, Copy)]
struct DragModifierFlags {
    shift: bool,
    alt: bool,
    ctrl: bool,
}

struct LassoDragBuffers {
    points_scr: Vec<ScreenPoint>,
    points_font: Vec<FontPoint>,
    initial_selected: Vec<NodeId>,
}

pub fn handle_bbox_and_transform_pointer_move(
    state: &mut GrafiaCanvasState,
    scr_pt: ScreenPoint,
    font_pt: FontPoint,
    is_shift: bool,
    is_alt: bool,
    is_ctrl: bool,
    is_space: bool,
) -> Option<bool> {
    if is_space && pan_marquee_while_space_held(state, scr_pt, is_alt) {
        return Some(true);
    }
    if state.drag_transients.guide_drag
        != crate::adapters::guide_management_ops::GuideDragState::Idle
    {
        return Some(state.update_guide_drag(font_pt, is_shift, is_alt, is_ctrl));
    }
    let mods = DragModifierFlags {
        shift: is_shift,
        alt: is_alt,
        ctrl: is_ctrl,
    };
    match state.bbox_drag.clone() {
        BBoxDragState::Idle => None,
        BBoxDragState::BBoxDragging {
            action,
            start_font,
            shadow_bounds,
            ..
        } => {
            let res =
                handle_bbox_drag_move(state, action, start_font, font_pt, shadow_bounds, mods);
            Some(res)
        }
        BBoxDragState::TransformOriginDragging { .. } => {
            let step = state.effective_grid_step();
            state.transform_origin = if !is_ctrl && step > 0.0 {
                font_pt.round_to_grid_step(step)
            } else {
                font_pt
            };
            state.transform_origin_custom = true;
            Some(true)
        }
        BBoxDragState::TransformPendingOrDragging {
            is_rotate,
            start_scr,
            start_font,
            active,
            ..
        } => Some(handle_transform_tool_drag_move(
            state,
            is_rotate,
            (start_scr, scr_pt),
            (start_font, font_pt),
            active,
            mods,
        )),
        BBoxDragState::LassoDragging {
            points_scr,
            points_font,
            initial_selected,
        } => {
            let lasso = LassoDragBuffers {
                points_scr,
                points_font,
                initial_selected,
            };
            update_lasso_drag_move(state, (scr_pt, font_pt), lasso, mods);
            Some(true)
        }
        BBoxDragState::ZoomDragging { start_scr, .. } => {
            let current_scr = scr_pt;
            state.bbox_drag = BBoxDragState::ZoomDragging {
                start_scr,
                current_scr,
            };
            Some(true)
        }
    }
}

fn update_lasso_drag_move(
    state: &mut GrafiaCanvasState,
    pts: (ScreenPoint, FontPoint),
    lasso: LassoDragBuffers,
    mods: DragModifierFlags,
) {
    let (scr_pt, font_pt) = pts;
    let LassoDragBuffers {
        mut points_scr,
        mut points_font,
        initial_selected,
    } = lasso;
    let DragModifierFlags {
        shift: is_shift,
        alt: is_alt,
        ctrl: is_ctrl,
    } = mods;
    if points_scr
        .last()
        .is_none_or(|p| p.distance_to(scr_pt) > LASSO_MIN_SAMPLE_STEP_DP)
    {
        points_scr.push(scr_pt);
        points_font.push(font_pt);
    }
    state.selected_nodes = select_nodes_in_lasso_polygon(
        &state.contours,
        &points_font,
        &initial_selected,
        is_shift,
        is_alt,
    );
    state.show_bounding_box_slanted |= is_ctrl;
    state.bbox_drag = BBoxDragState::LassoDragging {
        points_scr,
        points_font,
        initial_selected,
    };
}

fn resolve_bbox_rotate_pivot(
    state: &GrafiaCanvasState,
    shadow_bounds: &crate::core::SelectionBounds,
    k: u8,
    is_alt: bool,
    is_ctrl: bool,
) -> FontPoint {
    if state.transform_origin_custom {
        return state.transform_origin;
    }
    match state.transform_origin_mode {
        crate::core::TransformOriginSpec::BoundingBoxOrigin(anchor) => {
            shadow_bounds.aligned_point(anchor.index())
        }
        crate::core::TransformOriginSpec::Custom(pt) => pt,
        crate::core::TransformOriginSpec::MetricsOrigin(_) => state.resolve_transform_origin(),
        crate::core::TransformOriginSpec::BoundingBoxCenter => {
            shadow_bounds.aligned_point(if is_alt && !is_ctrl { 4 } else { 8 - k })
        }
    }
}

fn handle_bbox_drag_move(
    state: &mut GrafiaCanvasState,
    action: BBoxAction,
    start_font: FontPoint,
    font_pt: FontPoint,
    shadow_bounds: crate::core::SelectionBounds,
    mods: DragModifierFlags,
) -> bool {
    let DragModifierFlags {
        shift: is_shift,
        alt: is_alt,
        ctrl: is_ctrl,
    } = mods;
    state.bbox_drag = BBoxDragState::BBoxDragging {
        action,
        start_font,
        current_font: font_pt,
        shadow_bounds,
    };
    state.contours = state.drag_transients.drag_start_contours.clone();
    let aff = match action {
        BBoxAction::Scale(k) => {
            state.active_bbox_rotation_deg = None;
            compute_bbox_scale_affine(
                &shadow_bounds,
                k,
                start_font,
                font_pt,
                is_shift,
                is_alt,
                is_ctrl,
            )
        }
        BBoxAction::Rotate(k) => {
            let pivot = resolve_bbox_rotate_pivot(state, &shadow_bounds, k, is_alt, is_ctrl);
            let (deg, rot_aff) =
                crate::core::bbox_rotate_shear_ops::compute_bbox_rotation_deg_and_affine(
                    pivot, start_font, font_pt, is_shift,
                );
            state.active_bbox_rotation_deg = Some(deg);
            rot_aff
        }
        BBoxAction::Slant(k) => {
            state.active_bbox_rotation_deg = None;
            if is_ctrl && is_alt {
                crate::core::bbox_rotate_shear_ops::compute_bbox_shear_affine(
                    &shadow_bounds,
                    k,
                    start_font,
                    font_pt,
                )
            } else {
                compute_bbox_rotate_or_slant_affine(
                    &shadow_bounds,
                    action,
                    start_font,
                    font_pt,
                    is_shift,
                    is_alt,
                    is_ctrl,
                )
            }
        }
    };
    let (flags, sel) = (state.closed_flags(), state.selected_nodes.clone());
    let grid = if is_ctrl {
        0.0
    } else {
        state.effective_grid_step()
    };
    apply_bbox_action_transform(state, action, &flags, &sel, aff, is_ctrl && is_alt, grid);
    state.sync_paths_from_contours();
    true
}

fn apply_bbox_action_transform(
    state: &mut GrafiaCanvasState,
    action: BBoxAction,
    flags: &[bool],
    sel: &[NodeId],
    aff: kurbo::Affine,
    ctrl_alt: bool,
    grid: f64,
) {
    if matches!(action, BBoxAction::Scale(_)) {
        apply_mode_nodes_transform(
            &mut state.contours,
            &state.drag_transients.drag_start_contours,
            flags,
            sel,
            aff,
            ctrl_alt,
            grid,
        );
    } else {
        crate::core::bbox_rotate_shear_ops::apply_rigid_or_affine_coupled_transform(
            &mut state.contours,
            &state.drag_transients.drag_start_contours,
            flags,
            sel,
            aff,
            grid,
        );
    }
}

fn handle_transform_tool_drag_move(
    state: &mut GrafiaCanvasState,
    is_rotate: bool,
    scr_pts: (ScreenPoint, ScreenPoint),
    font_pts: (FontPoint, FontPoint),
    mut active: bool,
    mods: DragModifierFlags,
) -> bool {
    let (start_scr, scr_pt) = scr_pts;
    let (start_font, font_pt) = font_pts;
    let DragModifierFlags {
        shift: is_shift,
        alt: is_alt,
        ctrl: is_ctrl,
    } = mods;
    let pivot_dist_ok = if is_rotate {
        start_font.distance_to(state.transform_origin) > TRANSFORM_MIN_PIVOT_DIST_UPM
    } else {
        (start_font.x.0 - state.transform_origin.x.0).abs() > TRANSFORM_MIN_PIVOT_DIST_UPM
            && (start_font.y.0 - state.transform_origin.y.0).abs() > TRANSFORM_MIN_PIVOT_DIST_UPM
    };
    if !active && scr_pt.distance_to(start_scr) > TRANSFORM_CLICK_THRESHOLD_DP && pivot_dist_ok {
        state.drag_transients.active_drag_before_snapshot = Some(state.snapshot());
        state.record_drag_start();
        active = true;
    }
    state.bbox_drag = BBoxDragState::TransformPendingOrDragging {
        is_rotate,
        start_scr,
        current_scr: scr_pt,
        start_font,
        current_font: font_pt,
        active,
    };
    if active {
        state.contours = state.drag_transients.drag_start_contours.clone();
        let aff = if is_rotate {
            compute_rotate_tool_affine(state.transform_origin, start_font, font_pt, is_shift)
        } else {
            compute_scale_tool_affine(state.transform_origin, start_font, font_pt, is_shift)
        };
        let flags = state.closed_flags();
        let target_ids: Vec<NodeId> = if !state.selected_nodes.is_empty() {
            state.selected_nodes.clone()
        } else {
            state
                .contours
                .iter()
                .flat_map(|c| c.iter().map(|n| n.id))
                .collect()
        };
        let grid = if is_ctrl {
            0.0
        } else {
            state.effective_grid_step()
        };
        apply_mode_nodes_transform(
            &mut state.contours,
            &state.drag_transients.drag_start_contours,
            &flags,
            &target_ids,
            aff,
            is_ctrl && is_alt,
            grid,
        );
        state.sync_paths_from_contours();
    }
    true
}
