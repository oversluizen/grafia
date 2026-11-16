//! `SPEC-009` (`grafia v0.8.0` Part A) `GrafiaCanvasState` methods for Hobby/Euler conversions,
//! Quadratic/Cubic degree conversions, off-curve insertion/removal, multi-layer batch path ops,
//! compatible background interpolation, viewport bookmarks, and `v0.8.0` `SelectionInfo` actions.

use crate::adapters::state::GrafiaCanvasState;
use crate::core::curve_conversions::{
    convert_contour_to_cubic, convert_contour_to_quadratic, convert_cubic_to_hobby_in_contour,
    convert_hobby_or_euler_in_contour,
};
use crate::core::extra_nodes::{compute_layer_extra_nodes, hit_test_extra_nodes};
use crate::core::segment_degree_ops::{
    add_offcurve_in_contour, interpolate_contours_linear, remove_offcurve_in_contour,
};
use crate::core::{
    add_extremes_to_contours, correct_path_direction_layer, remove_overlap_in_contours,
    reverse_contour_nodes, tidy_up_contours, CanvasHostRequest, FontPoint, Node, NodeId, NodeType,
    ScreenPoint, SelectionInfoAction,
};
use crate::tokens::{
    CanvasBackgroundImage, CanvasSmartHandle, ExtraNodeKey, LayerPathOp, VirtualExtraNode,
    ALPHA_MAX_F32, ALPHA_MIN_F32, DEFAULT_ASCENDER_UPM, DEFAULT_BASELINE_UPM,
    DEFAULT_CAP_HEIGHT_UPM, DEFAULT_DESCENDER_UPM, DEFAULT_X_HEIGHT_UPM, HOBBY_TENSION_MAX,
    HOBBY_TENSION_MIN, PATH_COORD_EPS_UPM,
};

fn run_layer_path_op(
    op: LayerPathOp,
    contours: &mut Vec<Vec<Node>>,
    closed_flags: &mut Vec<bool>,
    selected_nodes: &[NodeId],
    next_node_id: &mut u64,
    italic_angle_deg: f64,
    grid_step_upm: f64,
) -> bool {
    match op {
        LayerPathOp::ReverseContours => {
            let mut changed = false;
            for (ci, contour) in contours.iter_mut().enumerate() {
                let targeted = selected_nodes.is_empty()
                    || contour.iter().any(|n| selected_nodes.contains(&n.id));
                if targeted && !contour.is_empty() {
                    let closed = closed_flags.get(ci).copied().unwrap_or(true);
                    reverse_contour_nodes(contour, closed);
                    changed = true;
                }
            }
            changed
        }
        LayerPathOp::CorrectPathDirection => correct_path_direction_layer(
            contours,
            closed_flags,
            italic_angle_deg,
            DEFAULT_X_HEIGHT_UPM * 0.5,
        ),
        LayerPathOp::TidyUpPaths => {
            tidy_up_contours(contours, closed_flags, selected_nodes, grid_step_upm)
        }
        LayerPathOp::AddExtremes { force } => add_extremes_to_contours(
            contours,
            closed_flags,
            selected_nodes,
            force,
            next_node_id,
            0.0,
        ),
        LayerPathOp::RemoveOverlap => {
            remove_overlap_in_contours(contours, closed_flags, selected_nodes, next_node_id)
        }
        LayerPathOp::RoundCoordinates => {
            let mut changed = false;
            for contour in contours.iter_mut() {
                for n in contour.iter_mut() {
                    if !selected_nodes.is_empty() && !selected_nodes.contains(&n.id) {
                        continue;
                    }
                    let (rx, ry) = (n.point.x.0.round(), n.point.y.0.round());
                    if (rx - n.point.x.0).abs() > PATH_COORD_EPS_UPM
                        || (ry - n.point.y.0).abs() > PATH_COORD_EPS_UPM
                    {
                        n.point = FontPoint::new(rx, ry);
                        changed = true;
                    }
                }
            }
            changed
        }
    }
}

impl GrafiaCanvasState {
    fn metric_lines_upm_array(&self) -> [f64; 5] {
        [
            DEFAULT_BASELINE_UPM,
            DEFAULT_X_HEIGHT_UPM,
            DEFAULT_CAP_HEIGHT_UPM,
            DEFAULT_ASCENDER_UPM,
            DEFAULT_DESCENDER_UPM,
        ]
    }

    fn apply_per_contour_op(
        &mut self,
        label: &'static str,
        filter_by_selection: bool,
        mut op: impl FnMut(&mut Vec<Node>, bool, &[NodeId], f64, &[f64], &mut u64) -> bool,
    ) -> bool {
        let before = self.snapshot();
        let flags = self.closed_flags();
        let sel = self.selected_nodes.clone();
        let italic = self.italic_angle_deg;
        let metrics = self.metric_lines_upm_array();
        let mut next_id = self.next_node_id;
        let mut changed = false;
        for (ci, contour) in self.contours.iter_mut().enumerate() {
            if filter_by_selection
                && !sel.is_empty()
                && !contour.iter().any(|n| sel.contains(&n.id))
            {
                continue;
            }
            let closed = flags.get(ci).copied().unwrap_or(true);
            if op(contour, closed, &sel, italic, &metrics, &mut next_id) {
                changed = true;
            }
        }
        if !changed {
            return false;
        }
        self.next_node_id = next_id;
        self.sync_paths_from_contours();
        self.record_mutation(label, before);
        self.assert_invariants();
        true
    }

    pub fn set_background_image(&mut self, image: Option<CanvasBackgroundImage>) {
        let before = self.snapshot();
        self.background_image = image;
        self.record_mutation("Set Background Image", before);
    }

    pub fn set_smart_handles(&mut self, handles: Vec<CanvasSmartHandle>) {
        self.smart_handles = handles;
        self.bump_content_generation();
    }

    pub fn convert_selected_hobby_nodes_to_cubic(&mut self) -> bool {
        self.apply_per_contour_op(
            "Convert Hobby/Euler Splines",
            false,
            |c, cl, sel, it, m, nid| {
                convert_hobby_or_euler_in_contour(c, cl, sel, (true, false), it, m, nid)
            },
        )
    }

    pub fn convert_selected_euler_spiro_nodes_to_cubic(&mut self) -> bool {
        self.apply_per_contour_op(
            "Convert Hobby/Euler Splines",
            false,
            |c, cl, sel, it, m, nid| {
                convert_hobby_or_euler_in_contour(c, cl, sel, (false, true), it, m, nid)
            },
        )
    }

    pub fn convert_selected_cubic_nodes_to_hobby(&mut self) -> bool {
        self.apply_per_contour_op("Convert Cubic to Hobby", false, |c, cl, sel, _, _, _| {
            convert_cubic_to_hobby_in_contour(c, cl, sel)
        })
    }

    pub fn convert_selected_contours_to_quadratic(&mut self) -> bool {
        self.apply_per_contour_op("Convert to Quadratic", true, |c, cl, _, it, m, nid| {
            convert_contour_to_quadratic(c, cl, it, m, nid)
        })
    }

    pub fn convert_selected_contours_to_cubic(&mut self) -> bool {
        self.apply_per_contour_op("Convert to Cubic", true, |c, cl, _, it, m, nid| {
            convert_contour_to_cubic(c, cl, it, m, nid)
        })
    }

    pub fn add_offcurve_to_selected_segments(&mut self) -> bool {
        self.apply_per_contour_op("Add Off-Curve Handles", false, |c, cl, sel, _, _, nid| {
            add_offcurve_in_contour(c, cl, sel, nid)
        })
    }

    pub fn remove_offcurve_from_selected_segments(&mut self) -> bool {
        self.apply_per_contour_op("Remove Off-Curve Handles", false, |c, cl, sel, _, _, _| {
            remove_offcurve_in_contour(c, cl, sel)
        })
    }

    pub fn apply_path_op_across_all_layers(&mut self, op: LayerPathOp) -> bool {
        let before = self.snapshot();
        let mut active_flags = self.closed_flags();
        let sel = self.selected_nodes.clone();
        let italic = self.italic_angle_deg;
        let step = self.grid_step_upm;
        let mut next_id = self.next_node_id;

        let mut any_changed = run_layer_path_op(
            op,
            &mut self.contours,
            &mut active_flags,
            &sel,
            &mut next_id,
            italic,
            step,
        );
        for layer in &mut self.companion_master_layers {
            if !layer.editable {
                continue;
            }
            while layer.closed_flags.len() < layer.contours.len() {
                layer.closed_flags.push(true);
            }
            if run_layer_path_op(
                op,
                &mut layer.contours,
                &mut layer.closed_flags,
                &[],
                &mut next_id,
                italic,
                step,
            ) {
                any_changed = true;
            }
        }
        if !any_changed {
            return false;
        }
        self.next_node_id = next_id;
        self.sync_contours_and_closed_flags(&active_flags);
        self.record_mutation("Path Op Across All Layers", before);
        self.assert_invariants();
        true
    }

    pub fn interpolate_with_background(&mut self, t: f64) -> bool {
        let before = self.snapshot();
        let flags = self.closed_flags();
        if !interpolate_contours_linear(&mut self.contours, &flags, &self.background_contours, t) {
            return false;
        }
        self.sync_paths_from_contours();
        self.record_mutation("Interpolate with Background", before);
        self.assert_invariants();
        true
    }

    pub fn save_viewport_bookmark(&mut self) {
        self.drag_transients.saved_viewport = Some(self.viewport);
    }

    pub fn toggle_saved_viewport_bookmark(&mut self) -> bool {
        let Some(saved) = self.drag_transients.saved_viewport else {
            return false;
        };
        let prev = self.viewport;
        self.viewport.zoom = saved.zoom;
        self.viewport.pan_offset = saved.pan_offset;
        self.drag_transients.saved_viewport = Some(prev);
        true
    }

    #[must_use]
    pub fn active_bbox_rotation_pill_label(&self) -> Option<String> {
        self.active_bbox_rotation_deg
            .map(|deg| format!("∠ {deg:+.1}°"))
    }

    #[must_use]
    pub fn compute_active_extra_nodes(&self) -> Vec<VirtualExtraNode> {
        compute_layer_extra_nodes(&self.glyph.contours)
    }

    pub fn select_extra_node_at_screen(&mut self, scr_pt: ScreenPoint) -> Option<ExtraNodeKey> {
        let vp = self.active_slot_viewport();
        let extras = self.compute_active_extra_nodes();
        let key = hit_test_extra_nodes(&extras, &vp, scr_pt)?;
        self.selected_extra_node = Some(key);
        self.drag_transients
            .host_requests
            .push(CanvasHostRequest::SelectOrMutateExtraNode { key });
        Some(key)
    }

    pub(crate) fn apply_spline_and_image_selection_info_action(
        &mut self,
        action: SelectionInfoAction,
    ) -> bool {
        match action {
            SelectionInfoAction::SetHobbyNode {
                tension_in,
                tension_out,
                dir_constraint,
            } => self.apply_set_hobby_node_action(tension_in, tension_out, dir_constraint),
            SelectionInfoAction::MutateBackgroundImage(m) => {
                self.apply_mutate_background_image_action(m)
            }
            SelectionInfoAction::ResetBackgroundImageTransform => {
                if self.background_image.is_none() {
                    return false;
                }
                let before = self.snapshot();
                if let Some(bg) = self.background_image.as_mut() {
                    bg.position = FontPoint::new(0.0, 0.0);
                    bg.scale_x = 1.0;
                    bg.scale_y = 1.0;
                    bg.rotation_deg = 0.0;
                    bg.crop_rect_px = None;
                }
                self.record_mutation("Reset Background Image", before);
                true
            }
            SelectionInfoAction::MutateGradientStop { offset_pct, rgba } => {
                self.apply_gradient_stop_selection_action(offset_pct, rgba)
            }
            SelectionInfoAction::MutatePostScriptHint {
                origin_upm,
                width_upm,
                top_ghost,
            } => self.apply_postscript_hint_selection_action(origin_upm, width_upm, top_ghost),
            SelectionInfoAction::MutateTrueTypeInstruction {
                kind,
                ppem,
                round_mode,
                stem_index,
                zone_index,
            } => self.apply_truetype_instruction_selection_action(
                kind, ppem, round_mode, stem_index, zone_index,
            ),
            SelectionInfoAction::SetHoiTiming {
                c1_x,
                c1_y,
                c2_x,
                c2_y,
            } => self.apply_hoi_timing_selection_action(c1_x, c1_y, c2_x, c2_y),
            SelectionInfoAction::MutateHoiNode {
                layer_index,
                contour_index,
                node_id,
                new_pos,
                c1,
                c2,
            } => self.apply_mutate_hoi_node_selection_action(
                layer_index,
                contour_index,
                node_id,
                new_pos,
                c1,
                c2,
            ),
            _ => false,
        }
    }

    fn apply_mutate_background_image_action(
        &mut self,
        m: crate::tokens::BackgroundImageMutation,
    ) -> bool {
        let Some(mut bg) = self.background_image.clone() else {
            return false;
        };
        let before = self.snapshot();
        if let Some(pos) = m.position {
            bg.position = pos;
        }
        if let Some(sx) = m.scale_x {
            bg.scale_x = sx;
        }
        if let Some(sy) = m.scale_y {
            bg.scale_y = sy;
        }
        if let Some(rot) = m.rotation_deg {
            bg.rotation_deg = rot;
        }
        if let Some(a) = m.alpha {
            bg.alpha = a.clamp(ALPHA_MIN_F32, ALPHA_MAX_F32);
        }
        if let Some(locked) = m.locked {
            bg.locked = locked;
        }
        if let Some(crop) = m.crop_rect_px {
            bg.crop_rect_px = crop;
        }
        self.background_image = Some(bg);
        self.record_mutation("Mutate Background Image", before);
        true
    }

    fn apply_set_hobby_node_action(
        &mut self,
        tension_in: f64,
        tension_out: f64,
        dir_constraint: crate::tokens::HobbyDirConstraint,
    ) -> bool {
        if self.selected_nodes.len() != 1 {
            return false;
        }
        let sel_id = self.selected_nodes[0];
        let before = self.snapshot();
        let t_in = tension_in.clamp(HOBBY_TENSION_MIN, HOBBY_TENSION_MAX);
        let t_out = tension_out.clamp(HOBBY_TENSION_MIN, HOBBY_TENSION_MAX);
        let Some(node) = self.find_node_mut(sel_id) else {
            return false;
        };
        if !matches!(node.node_type, NodeType::HobbyStar { .. }) {
            return false;
        }
        node.node_type = NodeType::HobbyStar {
            tension_in: t_in,
            tension_out: t_out,
            dir_constraint,
        };
        self.sync_paths_from_contours();
        self.record_mutation("Set Hobby Node", before);
        self.assert_invariants();
        true
    }
}
