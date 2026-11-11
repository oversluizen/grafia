//! `SPEC-004` (`grafia v0.3.0`) guide measurement rays, alignment zone badge classification,
//! magnetic snapping evaluation, and guide knob/line drag updates.

use crate::adapters::state::GrafiaCanvasState;
use crate::adapters::v030_guides::GuideDragState;
use crate::core::FontPoint;
use crate::snapping::{
    calculate_guide_intersections, classify_node_alignment, normalize_guide_angle,
    AlignmentBadgeMode, GuideId, GuideMeasurementRay, SelectedGuideRef, SnapEngine, SnapResult,
};
use crate::tokens::{
    DEFAULT_ANGLE_SNAP_DEG, GUIDE_ITALIC_SNAP_TOLERANCE_DEG, GUIDE_PERPENDICULAR_DEG,
    RAD_TO_DEG_F64,
};

impl GrafiaCanvasState {
    #[must_use]
    pub fn guide_measurement_ray(&self, id: GuideId) -> Option<GuideMeasurementRay> {
        let g = self.find_guide(id)?;
        if !g.show_measurement {
            return None;
        }
        Some(calculate_guide_intersections(
            g,
            &self.contours,
            &self.closed_flags(),
        ))
    }

    #[must_use]
    pub fn all_measurement_rays(&self) -> Vec<GuideMeasurementRay> {
        if !self.show_guides {
            return Vec::new();
        }
        let flags = self.closed_flags();
        self.guides
            .iter()
            .chain(self.global_guides.iter())
            .filter(|g| g.show_measurement)
            .map(|g| calculate_guide_intersections(g, &self.contours, &flags))
            .collect()
    }

    #[must_use]
    pub fn classify_node_alignment(&self, pt: FontPoint) -> Option<AlignmentBadgeMode> {
        if self.show_metrics {
            classify_node_alignment(
                pt,
                &self.metrics,
                self.glyph.width,
                self.italic_angle_deg,
                self.grid_step_upm,
                self.show_guides,
                &self.guides,
                &self.global_guides,
            )
        } else if self.show_guides {
            classify_node_alignment(
                pt,
                &self.metrics,
                self.glyph.width,
                self.italic_angle_deg,
                self.grid_step_upm,
                true,
                &self.guides,
                &self.global_guides,
            )
            .filter(|m| {
                matches!(
                    m,
                    AlignmentBadgeMode::OnLocalGuide | AlignmentBadgeMode::OnGlobalGuide
                )
            })
        } else {
            None
        }
    }

    #[must_use]
    pub fn evaluate_magnetic_snap(
        &self,
        query: FontPoint,
        excluded_nodes: &[crate::core::NodeId],
        ctrl_bypass: bool,
    ) -> SnapResult {
        SnapEngine::snap_point(
            query,
            &self.contours,
            excluded_nodes,
            &self.metrics,
            self.glyph.width,
            self.italic_angle_deg,
            self.show_guides,
            &self.guides,
            &self.global_guides,
            self.viewport.zoom,
            ctrl_bypass,
        )
    }

    pub fn resolve_node_drag_displacement(
        &mut self,
        raw_dx: f64,
        raw_dy: f64,
        shift_snap: bool,
        alt_for_move: bool,
        ctrl_for_move: bool,
        ctrl_snap_bypass: bool,
    ) -> (f64, f64) {
        if shift_snap || alt_for_move || ctrl_for_move || ctrl_snap_bypass {
            self.active_snappers.clear();
            return (raw_dx, raw_dy);
        }
        let Some(&lead_id) = self.selected_nodes.first() else {
            self.active_snappers.clear();
            return (raw_dx, raw_dy);
        };
        let Some(&(_, lead_start)) = self
            .drag_start_font_positions
            .iter()
            .find(|&&(id, _)| id == lead_id)
        else {
            self.active_snappers.clear();
            return (raw_dx, raw_dy);
        };
        let excluded: Vec<crate::core::NodeId> = self
            .drag_start_font_positions
            .iter()
            .map(|&(id, _)| id)
            .collect();
        let query = FontPoint::new(lead_start.x.0 + raw_dx, lead_start.y.0 + raw_dy);
        let res = self.evaluate_magnetic_snap(query, &excluded, false);
        self.active_snappers = res.active_snappers;
        (raw_dx + res.delta.0, raw_dy + res.delta.1)
    }

    pub fn update_guide_drag(
        &mut self,
        font_pt: FontPoint,
        is_shift: bool,
        is_alt: bool,
        is_ctrl: bool,
    ) -> bool {
        match self.guide_drag {
            GuideDragState::Idle => false,
            GuideDragState::DraggingKnob {
                mut guide_ref,
                start_font,
                initial_pos,
                mut alt_duplicated,
            } => {
                if self.lock_guides {
                    return false;
                }
                let (raw_dx, raw_dy) = (font_pt.x.0 - start_font.x.0, font_pt.y.0 - start_font.y.0);
                if is_alt && !alt_duplicated && raw_dx.hypot(raw_dy) > 0.0 {
                    if let Some(orig) = self.find_guide(guide_ref.id).cloned() {
                        let new_id = GuideId(self.next_guide_id);
                        self.next_guide_id += 1;
                        let mut dup = orig;
                        dup.id = new_id;
                        dup.position = initial_pos;
                        if let Some(orig_mut) = self.find_guide_mut(guide_ref.id) {
                            orig_mut.position = initial_pos;
                        }
                        if dup.is_global {
                            self.global_guides.push(dup);
                        } else {
                            self.guides.push(dup);
                        }
                        guide_ref = SelectedGuideRef {
                            id: new_id,
                            is_global: guide_ref.is_global,
                        };
                        self.selected_guide = Some(guide_ref);
                        alt_duplicated = true;
                        self.guide_drag = GuideDragState::DraggingKnob {
                            guide_ref,
                            start_font,
                            initial_pos,
                            alt_duplicated,
                        };
                    }
                }
                let target = FontPoint::new(initial_pos.x.0 + raw_dx, initial_pos.y.0 + raw_dy);
                let snapped = if is_ctrl {
                    self.active_snappers.clear();
                    target
                } else {
                    let res = self.evaluate_magnetic_snap(target, &[], false);
                    self.active_snappers = res.active_snappers;
                    res.snapped_point
                };
                if let Some(g) = self.find_guide_mut(guide_ref.id) {
                    if !g.locked {
                        g.position = snapped;
                    }
                }
                true
            }
            GuideDragState::RotatingLine { guide_ref, .. } => {
                if self.lock_guides {
                    return false;
                }
                let italic = self.italic_angle_deg;
                if let Some(g) = self.find_guide_mut(guide_ref.id) {
                    if g.locked || g.lock_angle {
                        return false;
                    }
                    let (dx, dy) = (font_pt.x.0 - g.position.x.0, font_pt.y.0 - g.position.y.0);
                    if dx.hypot(dy) > 1.0 {
                        let mut deg = normalize_guide_angle(dy.atan2(dx) * RAD_TO_DEG_F64);
                        if is_shift {
                            let italic_up = normalize_guide_angle(GUIDE_PERPENDICULAR_DEG - italic);
                            let italic_down =
                                normalize_guide_angle(-GUIDE_PERPENDICULAR_DEG - italic);
                            if italic.abs() > 0.0
                                && normalize_guide_angle(deg - italic_up).abs()
                                    <= GUIDE_ITALIC_SNAP_TOLERANCE_DEG
                            {
                                deg = italic_up;
                            } else if italic.abs() > 0.0
                                && normalize_guide_angle(deg - italic_down).abs()
                                    <= GUIDE_ITALIC_SNAP_TOLERANCE_DEG
                            {
                                deg = italic_down;
                            } else {
                                let step = DEFAULT_ANGLE_SNAP_DEG;
                                deg = normalize_guide_angle((deg / step).round() * step);
                            }
                        }
                        g.set_angle(deg);
                    }
                }
                true
            }
        }
    }
}
