//! `SPEC-007` (`grafia v0.6.0`) pure 2D VC-Contract (`SelectionInfoAction`),
//! HUD 3x3 Transform-Grid hit-testing, and foreground-to-background selection projection (`align_selection_to_background`).

use crate::adapters::state::GrafiaCanvasState;
use crate::core::{
    align_oncurve_nodes_to_background, toggle_selected_on_curve_smooth, FontPoint, GuideAlignment,
    NodeId, NodeType, ScreenPoint, SelectionInfo, SelectionInfoAction, TransformGridAnchor,
};
use crate::render::v060_hud::{
    compute_hud_proportional_icon_rect, compute_hud_rect, compute_hud_transform_grid_dot_positions,
    should_render_selection_info_hud,
};
use crate::snapping::{Guide, GuideId, SelectedGuideRef};
use crate::tokens::HUD_GRID_DOT_HIT_RADIUS_DP;

impl GrafiaCanvasState {
    /// Applies a unified pure 2D `SelectionInfoAction` from the host UI / HUD controller (`SPEC-007 §2.2`).
    pub fn apply_selection_info_action(&mut self, action: SelectionInfoAction) -> bool {
        match action {
            SelectionInfoAction::SetLsb(v) => {
                if !v.is_finite() {
                    return false;
                }
                self.set_lsb(v);
                true
            }
            SelectionInfoAction::SetRsb(v) => {
                if !v.is_finite() {
                    return false;
                }
                self.set_rsb(v);
                true
            }
            SelectionInfoAction::SetAdvanceWidth(v) => {
                if !v.is_finite() {
                    return false;
                }
                self.set_advance_width(v);
                true
            }
            SelectionInfoAction::CenterInWidth => {
                self.center_glyph_in_width();
                true
            }
            SelectionInfoAction::SetOnCurvePosition(pt) => {
                if self.selected_nodes.len() != 1
                    || !pt.x.0.is_finite()
                    || !pt.y.0.is_finite()
                    || !self
                        .find_node(self.selected_nodes[0])
                        .is_some_and(|n| n.node_type != NodeType::OffCurve)
                {
                    return false;
                }
                self.set_selected_oncurve_position(pt);
                true
            }
            SelectionInfoAction::ToggleOnCurveSmooth => {
                if self.selected_nodes.is_empty() {
                    return false;
                }
                let before = self.snapshot();
                let closed_flags = self.closed_flags();
                let changed = toggle_selected_on_curve_smooth(
                    &mut self.contours,
                    &closed_flags,
                    &self.selected_nodes,
                );
                if changed {
                    self.sync_paths_from_contours();
                    self.record_mutation("Toggle Smooth", before);
                    self.assert_invariants();
                }
                changed
            }
            SelectionInfoAction::SetOffCurvePosition(pt) => {
                if self.selected_nodes.len() != 1
                    || !pt.x.0.is_finite()
                    || !pt.y.0.is_finite()
                    || !self
                        .find_node(self.selected_nodes[0])
                        .is_some_and(|n| n.node_type == NodeType::OffCurve)
                {
                    return false;
                }
                self.set_selected_offcurve_position(pt);
                true
            }
            SelectionInfoAction::SetOffCurveDelta { dx, dy } => {
                if self.selected_nodes.len() != 1
                    || !dx.is_finite()
                    || !dy.is_finite()
                    || !self
                        .find_node(self.selected_nodes[0])
                        .is_some_and(|n| n.node_type == NodeType::OffCurve)
                {
                    return false;
                }
                self.set_selected_offcurve_delta(dx, dy);
                true
            }
            SelectionInfoAction::SetHandleLength(len) => {
                if self.selected_nodes.len() != 1
                    || !len.is_finite()
                    || !self
                        .find_node(self.selected_nodes[0])
                        .is_some_and(|n| n.node_type == NodeType::OffCurve)
                {
                    return false;
                }
                self.set_selected_handle_length(len);
                true
            }
            SelectionInfoAction::SetMultiAnchor(anchor) => {
                self.set_multi_selection_anchor(anchor);
                true
            }
            SelectionInfoAction::SetMultiProportional(prop) => {
                self.set_multi_selection_proportional(prop);
                true
            }
            SelectionInfoAction::SetMultiOrigin(origin) => {
                if self.selected_nodes.len() < 2
                    || !origin.x.0.is_finite()
                    || !origin.y.0.is_finite()
                {
                    return false;
                }
                self.set_multi_selection_origin(origin);
                true
            }
            SelectionInfoAction::SetMultiSize { width, height } => {
                if self.selected_nodes.len() < 2
                    || (width.is_none() && height.is_none())
                    || width.is_some_and(|w| !w.is_finite())
                    || height.is_some_and(|h| !h.is_finite())
                {
                    return false;
                }
                self.set_multi_selection_size(width, height);
                true
            }
            SelectionInfoAction::SetGuideOrigin(origin) => {
                if !origin.x.0.is_finite()
                    || !origin.y.0.is_finite()
                    || !self.selected_guide_matches(|g| !g.locked)
                {
                    return false;
                }
                self.set_selected_guide_origin(origin);
                true
            }
            SelectionInfoAction::SetGuideAngle(angle) => {
                if !angle.is_finite()
                    || !self.selected_guide_matches(|g| !g.locked && !g.lock_angle)
                {
                    return false;
                }
                self.set_selected_guide_angle(angle);
                true
            }
            SelectionInfoAction::SetGuideAlignment(align) => {
                if !self.selected_guide_matches(|_| true) {
                    return false;
                }
                self.set_selected_guide_alignment(align);
                true
            }
            SelectionInfoAction::SetGuideLockAngle(lock) => {
                if !self.selected_guide_matches(|_| true) {
                    return false;
                }
                self.set_selected_guide_lock_angle(lock);
                true
            }
            SelectionInfoAction::SetGuideMeasurement(show) => {
                if !self.selected_guide_matches(|_| true) {
                    return false;
                }
                self.set_selected_guide_measurement(show);
                true
            }
            other => self.apply_v070_selection_info_action(other),
        }
    }

    /// Projects every selected `OnCurve` node (and translates its adjacent `OffCurve` handles)
    /// onto the nearest point on `self.background_contours` (`Line` and `Cubic` segments).
    pub fn align_selection_to_background(&mut self) -> usize {
        if self.selected_nodes.is_empty() || self.background_contours.is_empty() {
            return 0;
        }
        let selected_oncurve_ids: Vec<NodeId> = self
            .selected_nodes
            .iter()
            .copied()
            .filter(|&id| {
                self.find_node(id)
                    .is_some_and(|n| n.node_type != NodeType::OffCurve)
            })
            .collect();
        if selected_oncurve_ids.is_empty() {
            return 0;
        }

        let before = self.snapshot();
        let closed_flags = self.closed_flags();
        let aligned_count = align_oncurve_nodes_to_background(
            &mut self.contours,
            &closed_flags,
            &selected_oncurve_ids,
            &self.background_contours,
        );
        if aligned_count > 0 {
            self.sync_paths_from_contours();
            self.record_mutation("Align Selection to Background", before);
            self.assert_invariants();
        }
        aligned_count
    }

    /// Hit-tests the 9 dots of the 3x3 Transform-Grid inside the bottom Grey Info Box HUD.
    #[must_use]
    pub fn hit_test_hud_transform_grid_anchor(
        &self,
        scr_pt: ScreenPoint,
        canvas_rect: egui::Rect,
    ) -> Option<TransformGridAnchor> {
        if !should_render_selection_info_hud(self)
            || !matches!(self.selection_info(), SelectionInfo::MultiNode(_))
        {
            return None;
        }
        let hud_rect = compute_hud_rect(canvas_rect);
        let dots = compute_hud_transform_grid_dot_positions(hud_rect);
        dots.into_iter()
            .find(|(_, pt)| scr_pt.distance_to(*pt) <= HUD_GRID_DOT_HIT_RADIUS_DP)
            .map(|(anchor, _)| anchor)
    }

    /// Hit-tests the proportional scale icon inside the bottom Grey Info Box HUD.
    #[must_use]
    pub fn hit_test_hud_proportional_toggle(
        &self,
        scr_pt: ScreenPoint,
        canvas_rect: egui::Rect,
    ) -> bool {
        if !should_render_selection_info_hud(self)
            || !matches!(self.selection_info(), SelectionInfo::MultiNode(_))
        {
            return false;
        }
        let hud_rect = compute_hud_rect(canvas_rect);
        let prop_rect = compute_hud_proportional_icon_rect(hud_rect);
        prop_rect.contains(egui::Pos2::new(scr_pt.x.0, scr_pt.y.0))
    }

    /// Handles primary pointer press on the interactive 3x3 Transform-Grid and proportional icon in the HUD.
    pub fn handle_v060_hud_pointer_press(
        &mut self,
        scr_pt: ScreenPoint,
        canvas_rect: egui::Rect,
    ) -> bool {
        if let Some(anchor) = self.hit_test_hud_transform_grid_anchor(scr_pt, canvas_rect) {
            return self.apply_selection_info_action(SelectionInfoAction::SetMultiAnchor(anchor));
        }
        if self.hit_test_hud_proportional_toggle(scr_pt, canvas_rect) {
            let next_prop = !self.multi_selection_proportional;
            return self
                .apply_selection_info_action(SelectionInfoAction::SetMultiProportional(next_prop));
        }
        false
    }

    /// Adds a local guide at `origin` with `angle_deg`, selects it, and records an Undo snapshot.
    pub fn add_local_guide(&mut self, origin: FontPoint, angle_deg: f64) -> GuideId {
        let before = self.snapshot();
        let id = GuideId(self.next_guide_id);
        self.next_guide_id += 1;
        self.guides.push(Guide::new(id, origin, angle_deg, false));
        self.show_guides = true;
        self.selected_nodes.clear();
        self.selected_guide = Some(SelectedGuideRef {
            id,
            is_global: false,
        });
        self.record_mutation("Add Guide", before);
        self.assert_invariants();
        id
    }

    fn selected_guide_matches(&self, pred: impl Fn(&Guide) -> bool) -> bool {
        let Some(gref) = self.selected_guide else {
            return false;
        };
        let list = if gref.is_global {
            &self.global_guides
        } else {
            &self.guides
        };
        list.iter().any(|g| g.id == gref.id && pred(g))
    }

    fn mutate_selected_guide(&mut self, label: &'static str, op: impl FnOnce(&mut Guide)) {
        let Some(gref) = self.selected_guide else {
            return;
        };
        let before = self.snapshot();
        let list = if gref.is_global {
            &mut self.global_guides
        } else {
            &mut self.guides
        };
        if let Some(g) = list.iter_mut().find(|g| g.id == gref.id) {
            op(g);
        }
        self.record_mutation(label, before);
        self.assert_invariants();
    }
    /// Selects a local guide by `id` (clearing `selected_nodes` so `SelectionInfo::Guide` is active).
    pub fn select_local_guide(&mut self, id: GuideId) {
        if self.guides.iter().any(|g| g.id == id) {
            self.selected_nodes.clear();
            self.selected_guide = Some(SelectedGuideRef {
                id,
                is_global: false,
            });
        }
        self.assert_invariants();
    }

    /// Sets the `GuideAlignment` (`Lsb`, `Center`, `Rsb`) of the selected guide and aligns its X position.
    pub fn set_selected_guide_alignment(&mut self, alignment: GuideAlignment) {
        let target_x = match alignment {
            GuideAlignment::Lsb => 0.0,
            GuideAlignment::Center => self.glyph.width * 0.5,
            GuideAlignment::Rsb => self.glyph.width,
        };
        self.mutate_selected_guide("Set Guide Alignment", |g| {
            g.alignment = alignment;
            if !g.locked {
                g.position.x.0 = target_x;
            }
        });
    }

    /// Sets the origin (`FontPoint`) of the currently selected guide.
    pub fn set_selected_guide_origin(&mut self, new_origin: FontPoint) {
        if !new_origin.x.0.is_finite() || !new_origin.y.0.is_finite() {
            return;
        }
        self.mutate_selected_guide("Move Guide", |g| {
            if !g.locked {
                g.position = new_origin;
            }
        });
    }

    /// Sets the angle in degrees of the currently selected guide (if not angle-locked).
    pub fn set_selected_guide_angle(&mut self, new_angle_deg: f64) {
        self.mutate_selected_guide("Rotate Guide", |g| {
            let _ = g.set_angle(new_angle_deg);
        });
    }

    /// Sets the `lock_angle` flag on the currently selected guide.
    pub fn set_selected_guide_lock_angle(&mut self, lock: bool) {
        self.mutate_selected_guide("Lock Guide Angle", |g| {
            g.lock_angle = lock;
        });
    }

    /// Sets the `show_measurement` flag on the currently selected guide.
    pub fn set_selected_guide_measurement(&mut self, show: bool) {
        self.mutate_selected_guide("Toggle Guide Measurement", |g| {
            g.show_measurement = show;
        });
    }
}
