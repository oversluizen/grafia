//! `SPEC-004` (`grafia v0.3.0`) `GrafiaCanvasState` methods for Local & Global Guides (`CanvasGuide`),
//! Magnetic `SnapEngine` (`SnapEngine`), and Alignment Zone Badges.

use crate::adapters::state::GrafiaCanvasState;
use crate::core::{FontPoint, ScreenPoint};
use crate::snapping::{
    smart_two_node_guide_geometry, Guide, GuideHitKind, GuideId, SelectedGuideRef,
};

#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub enum GuideDragState {
    #[default]
    Idle,
    DraggingKnob {
        guide_ref: SelectedGuideRef,
        start_font: FontPoint,
        initial_pos: FontPoint,
        alt_duplicated: bool,
    },
    RotatingLine {
        guide_ref: SelectedGuideRef,
        initial_angle_deg: f64,
    },
}

impl GrafiaCanvasState {
    pub fn add_guide_from_selection(
        &mut self,
        is_global: bool,
        fallback_pos: FontPoint,
    ) -> GuideId {
        let before = self.snapshot();
        let (pos, angle_deg) = match self.selected_nodes.len() {
            1 => (
                self.find_node(self.selected_nodes[0])
                    .map_or(fallback_pos, |n| n.point),
                0.0,
            ),
            2 => {
                let p0 = self
                    .find_node(self.selected_nodes[0])
                    .map_or(fallback_pos, |n| n.point);
                let p1 = self
                    .find_node(self.selected_nodes[1])
                    .map_or(fallback_pos, |n| n.point);
                smart_two_node_guide_geometry(p0, p1)
            }
            _ => (fallback_pos, 0.0),
        };
        let id = GuideId(self.next_guide_id);
        self.next_guide_id += 1;
        let guide = Guide::new(id, pos, angle_deg, is_global);
        if is_global {
            self.global_guides.push(guide);
        } else {
            self.guides.push(guide);
        }
        self.show_guides = true;
        self.selected_nodes.clear();
        self.selected_guide = Some(SelectedGuideRef { id, is_global });
        self.record_mutation(
            if is_global {
                "Add Global Guide"
            } else {
                "Add Guide"
            },
            before,
        );
        self.assert_invariants();
        id
    }

    #[must_use]
    pub fn find_guide(&self, id: GuideId) -> Option<&Guide> {
        self.guides
            .iter()
            .chain(self.global_guides.iter())
            .find(|g| g.id == id)
    }

    pub fn find_guide_mut(&mut self, id: GuideId) -> Option<&mut Guide> {
        if let Some(g) = self.guides.iter_mut().find(|g| g.id == id) {
            return Some(g);
        }
        self.global_guides.iter_mut().find(|g| g.id == id)
    }

    pub fn set_guide_angle(&mut self, id: GuideId, angle_deg: f64) -> bool {
        if self.lock_guides {
            return false;
        }
        let before = self.snapshot();
        let changed = self
            .find_guide_mut(id)
            .is_some_and(|g| g.set_angle(angle_deg));
        if changed {
            self.record_mutation("Set Guide Angle", before);
            self.assert_invariants();
        }
        changed
    }

    pub fn set_guide_show_measurement(&mut self, id: GuideId, show: bool) -> bool {
        let before = self.snapshot();
        if let Some(g) = self.find_guide_mut(id) {
            if g.show_measurement != show {
                g.show_measurement = show;
                self.record_mutation("Toggle Guide Measurement", before);
                self.assert_invariants();
                return true;
            }
        }
        false
    }

    pub fn double_click_guide_knob(&mut self, id: GuideId) -> bool {
        if self.lock_guides {
            return false;
        }
        let before = self.snapshot();
        let changed = self
            .find_guide_mut(id)
            .is_some_and(Guide::flip_perpendicular);
        if changed {
            self.record_mutation("Flip Guide 90°", before);
            self.assert_invariants();
        }
        changed
    }

    pub fn delete_selected_guide(&mut self) -> bool {
        if self.lock_guides {
            return false;
        }
        let Some(sel) = self.selected_guide else {
            return false;
        };
        let before = self.snapshot();
        let len_before = self.guides.len() + self.global_guides.len();
        self.guides.retain(|g| g.id != sel.id || g.locked);
        self.global_guides.retain(|g| g.id != sel.id || g.locked);
        if self.guides.len() + self.global_guides.len() < len_before {
            self.selected_guide = None;
            self.record_mutation("Delete Guide", before);
            self.assert_invariants();
            return true;
        }
        false
    }

    pub fn nudge_selected_guide(&mut self, dx: f64, dy: f64) -> bool {
        if self.lock_guides {
            return false;
        }
        let Some(sel) = self.selected_guide else {
            return false;
        };
        let before = self.snapshot();
        if let Some(g) = self.find_guide_mut(sel.id) {
            if g.locked {
                return false;
            }
            g.position.x.0 += dx;
            g.position.y.0 += dy;
            self.record_mutation("Nudge Guide", before);
            self.assert_invariants();
            return true;
        }
        false
    }

    pub fn cycle_guide_tab(&mut self, reverse: bool) -> bool {
        if !self.show_guides {
            return false;
        }
        let all: Vec<SelectedGuideRef> = self
            .guides
            .iter()
            .chain(self.global_guides.iter())
            .map(|g| SelectedGuideRef {
                id: g.id,
                is_global: g.is_global,
            })
            .collect();
        if all.is_empty() {
            return false;
        }
        let next = if let Some(cur) = self.selected_guide {
            if let Some(idx) = all.iter().position(|&r| r == cur) {
                let next_idx = if reverse {
                    (idx + all.len() - 1) % all.len()
                } else {
                    (idx + 1) % all.len()
                };
                all[next_idx]
            } else {
                all[0]
            }
        } else {
            all[0]
        };
        self.selected_guide = Some(next);
        self.assert_invariants();
        true
    }

    #[must_use]
    pub fn hit_test_guides(&self, cursor_scr: ScreenPoint) -> Option<GuideHitKind> {
        if !self.show_guides {
            return None;
        }
        for g in self.guides.iter().chain(self.global_guides.iter()) {
            if let Some(GuideHitKind::Knob(r)) = g.hit_test_screen(cursor_scr, &self.viewport) {
                return Some(GuideHitKind::Knob(r));
            }
        }
        for g in self.guides.iter().chain(self.global_guides.iter()) {
            if let Some(hit) = g.hit_test_screen(cursor_scr, &self.viewport) {
                return Some(hit);
            }
        }
        None
    }

    pub fn try_begin_guide_press(
        &mut self,
        scr_pt: ScreenPoint,
        font_pt: FontPoint,
        is_double: bool,
    ) -> Option<bool> {
        let Some(hit) = self.hit_test_guides(scr_pt) else {
            self.selected_guide = None;
            return None;
        };
        self.selected_nodes.clear();
        match hit {
            GuideHitKind::Knob(gref) => {
                self.selected_guide = Some(gref);
                if is_double {
                    self.double_click_guide_knob(gref.id);
                    return Some(true);
                }
                if !self.lock_guides {
                    if let Some(g) = self.find_guide(gref.id) {
                        if !g.locked {
                            let initial_pos = g.position;
                            self.active_drag_before_snapshot = Some(self.snapshot());
                            self.guide_drag = GuideDragState::DraggingKnob {
                                guide_ref: gref,
                                start_font: font_pt,
                                initial_pos,
                                alt_duplicated: false,
                            };
                        }
                    }
                }
                Some(true)
            }
            GuideHitKind::Line(gref) => {
                self.selected_guide = Some(gref);
                if !self.lock_guides {
                    if let Some(g) = self.find_guide(gref.id) {
                        if !g.locked && !g.lock_angle {
                            let initial_angle_deg = g.angle_deg;
                            self.active_drag_before_snapshot = Some(self.snapshot());
                            self.guide_drag = GuideDragState::RotatingLine {
                                guide_ref: gref,
                                initial_angle_deg,
                            };
                        }
                    }
                }
                Some(true)
            }
        }
    }
}
