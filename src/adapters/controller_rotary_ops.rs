//! `SPEC-015 §3`: Port 2 (`CanvasRotaryChannel` / `step_rotary_encoder`) and Primary Rotary Telemetry
//! for Loupedeck CT dials, Wacom Intuos 4 Touch Ring, and on-screen radial wheels.

use crate::adapters::state::{CanvasTool, GrafiaCanvasState};
use crate::core::math_consts::{
    BRUSH_MAX_RADIUS_DP, BRUSH_MIN_RADIUS_DP, HOBBY_TENSION_MAX, HOBBY_TENSION_MIN,
    MAX_VIEWPORT_ZOOM, MIN_VIEWPORT_ZOOM, PERCENT_SCALE_FACTOR, ROTARY_ANGLE_STEP_DEG,
    ROTARY_BRUSH_RADIUS_STEP_DP, ROTARY_CORNER_MAX_RADIUS_UPM, ROTARY_CORNER_MIN_RADIUS_UPM,
    ROTARY_CORNER_RADIUS_STEP_UPM, ROTARY_HOBBY_TENSION_STEP, ROTARY_ITALIC_MAX_ABS_DEG,
    ROTARY_STEM_STEP_UPM, ROTARY_TUNNI_RATIO_STEP, ROTARY_ZOOM_STEP_FRAC, SOLVER_DET_EPSILON,
    TUNNI_MAX_RATIO, TUNNI_MIN_RATIO,
};
use crate::core::{
    CanvasHapticEvent, CanvasRotaryChannel, NodeType, RotaryTelemetryInfo, ScreenPoint,
    SmartCornerSpec, StemAdjustmentMode,
};

impl GrafiaCanvasState {
    /// Steps a continuous hardware or virtual rotary encoder channel by `delta_ticks` (`SPEC-015 §3`).
    pub fn step_rotary_encoder(&mut self, channel: CanvasRotaryChannel, delta_ticks: f64) -> bool {
        if !delta_ticks.is_finite() || delta_ticks.abs() <= SOLVER_DET_EPSILON {
            return false;
        }
        let (changed, clamped) = match channel {
            CanvasRotaryChannel::ContextPrimary => self.step_context_primary_rotary(delta_ticks),
            CanvasRotaryChannel::ViewportZoom => self.step_viewport_zoom_rotary(delta_ticks),
            CanvasRotaryChannel::CanvasRotationOrSlant => {
                self.step_rotation_or_slant_rotary(delta_ticks)
            }
            CanvasRotaryChannel::HistoryScrub => self.step_history_scrub_rotary(delta_ticks),
            CanvasRotaryChannel::CurveTensionOrRadius => {
                self.step_curve_tension_or_radius_rotary(delta_ticks)
            }
            CanvasRotaryChannel::KerningOrSpacing => {
                self.step_kerning_or_spacing_rotary(delta_ticks)
            }
        };
        if changed {
            self.push_haptic_event(CanvasHapticEvent::RotaryDetentTick { channel });
        }
        if clamped {
            self.push_haptic_event(CanvasHapticEvent::BoundaryClamped);
        }
        changed
    }

    fn step_context_primary_rotary(&mut self, delta_ticks: f64) -> (bool, bool) {
        if self.active_tool == CanvasTool::Stem {
            let stems = self.detect_structural_stems();
            if let Some(first) = stems.first() {
                let ok = self.adjust_structural_stem(
                    first.id,
                    delta_ticks * ROTARY_STEM_STEP_UPM,
                    StemAdjustmentMode::SymmetricCenter,
                    0.0,
                    false,
                );
                return (ok, !ok);
            }
        }
        if matches!(self.active_tool, CanvasTool::Trowel | CanvasTool::Cleaner) {
            let old_r = self.drawing_tool_state.controller_ports.brush_radius_dp;
            let raw_r = old_r + (delta_ticks as f32) * ROTARY_BRUSH_RADIUS_STEP_DP;
            let new_r = raw_r.clamp(BRUSH_MIN_RADIUS_DP, BRUSH_MAX_RADIUS_DP);
            let clamped = (new_r - raw_r).abs() > f32::EPSILON;
            self.drawing_tool_state.controller_ports.brush_radius_dp = new_r;
            self.bump_content_generation();
            return ((new_r - old_r).abs() > f32::EPSILON, clamped);
        }
        let (changed, clamped) = self.step_curve_tension_or_radius_rotary(delta_ticks);
        if changed || clamped {
            return (changed, clamped);
        }
        self.step_viewport_zoom_rotary(delta_ticks)
    }

    fn step_curve_tension_or_radius_rotary(&mut self, delta_ticks: f64) -> (bool, bool) {
        if let Some(&sel_id) = self.selected_nodes.first() {
            if let Some(existing) = self
                .smart_corners
                .iter()
                .find(|s| s.node_id == sel_id)
                .copied()
            {
                let raw = existing.radius_upm + delta_ticks * ROTARY_CORNER_RADIUS_STEP_UPM;
                let next = raw.clamp(ROTARY_CORNER_MIN_RADIUS_UPM, ROTARY_CORNER_MAX_RADIUS_UPM);
                let clamped = (next - raw).abs() > SOLVER_DET_EPSILON;
                self.set_smart_corner(SmartCornerSpec {
                    node_id: sel_id,
                    radius_upm: next,
                });
                return (
                    (next - existing.radius_upm).abs() > SOLVER_DET_EPSILON,
                    clamped,
                );
            }
            if let Some(node) = self.find_node(sel_id) {
                if node.node_type == NodeType::Corner {
                    let raw = delta_ticks * ROTARY_CORNER_RADIUS_STEP_UPM;
                    let next =
                        raw.clamp(ROTARY_CORNER_MIN_RADIUS_UPM, ROTARY_CORNER_MAX_RADIUS_UPM);
                    let clamped = (next - raw).abs() > SOLVER_DET_EPSILON;
                    self.set_smart_corner(SmartCornerSpec {
                        node_id: sel_id,
                        radius_upm: next,
                    });
                    return (next.abs() > SOLVER_DET_EPSILON, clamped);
                }
                if let NodeType::HobbyStar {
                    tension_in,
                    tension_out,
                    dir_constraint,
                } = node.node_type
                {
                    let raw_in = tension_in + delta_ticks * ROTARY_HOBBY_TENSION_STEP;
                    let raw_out = tension_out + delta_ticks * ROTARY_HOBBY_TENSION_STEP;
                    let next_in = raw_in.clamp(HOBBY_TENSION_MIN, HOBBY_TENSION_MAX);
                    let next_out = raw_out.clamp(HOBBY_TENSION_MIN, HOBBY_TENSION_MAX);
                    let clamped = (next_in - raw_in).abs() > SOLVER_DET_EPSILON
                        || (next_out - raw_out).abs() > SOLVER_DET_EPSILON;
                    let changed = self.apply_selection_info_action(
                        crate::core::SelectionInfoAction::SetHobbyNode {
                            tension_in: next_in,
                            tension_out: next_out,
                            dir_constraint,
                        },
                    );
                    return (changed, clamped);
                }
            }
        }
        let overlays = self.compute_tunni_overlays();
        if let Some(ov) = overlays.first() {
            let cur_r = 0.5 * (ov.ratio_p1 + ov.ratio_p2);
            let raw_r = cur_r + delta_ticks * ROTARY_TUNNI_RATIO_STEP;
            let next_r = raw_r.clamp(TUNNI_MIN_RATIO, TUNNI_MAX_RATIO);
            let clamped = (next_r - raw_r).abs() > SOLVER_DET_EPSILON;
            self.scale_segment_tunni_ratio(ov.contour_id, ov.segment_index, next_r);
            return ((next_r - cur_r).abs() > SOLVER_DET_EPSILON, clamped);
        }
        (false, false)
    }

    fn step_viewport_zoom_rotary(&mut self, delta_ticks: f64) -> (bool, bool) {
        let old_z = self.viewport.zoom;
        let factor = (1.0 + (delta_ticks as f32) * ROTARY_ZOOM_STEP_FRAC).clamp(0.5, 2.0);
        let raw_z = old_z * factor;
        let new_z = raw_z.clamp(MIN_VIEWPORT_ZOOM, MAX_VIEWPORT_ZOOM);
        let clamped = (new_z - raw_z).abs() > f32::EPSILON;
        let pivot = self
            .last_pointer_screen_pos
            .unwrap_or_else(|| ScreenPoint::new(0.0, 0.0));
        self.viewport
            .zoom_at_cursor(pivot, new_z / old_z.max(MIN_VIEWPORT_ZOOM));
        ((self.viewport.zoom - old_z).abs() > f32::EPSILON, clamped)
    }

    fn step_rotation_or_slant_rotary(&mut self, delta_ticks: f64) -> (bool, bool) {
        let deg = delta_ticks * ROTARY_ANGLE_STEP_DEG;
        if !self.selected_nodes.is_empty() {
            self.rotate_by_degrees(deg);
            return (true, false);
        }
        let old_it = self.italic_angle_deg;
        let raw_it = old_it + deg;
        let new_it = raw_it.clamp(-ROTARY_ITALIC_MAX_ABS_DEG, ROTARY_ITALIC_MAX_ABS_DEG);
        let clamped = (new_it - raw_it).abs() > SOLVER_DET_EPSILON;
        self.italic_angle_deg = new_it;
        self.bump_content_generation();
        ((new_it - old_it).abs() > SOLVER_DET_EPSILON, clamped)
    }

    fn step_history_scrub_rotary(&mut self, delta_ticks: f64) -> (bool, bool) {
        let steps = delta_ticks.round().abs().max(1.0) as usize;
        let mut any = false;
        let mut clamped = false;
        for _ in 0..steps {
            let ok = if delta_ticks < 0.0 {
                self.undo()
            } else {
                self.redo()
            };
            if ok {
                any = true;
            } else {
                clamped = true;
                break;
            }
        }
        (any, clamped)
    }

    fn step_kerning_or_spacing_rotary(&mut self, delta_ticks: f64) -> (bool, bool) {
        let step = delta_ticks * self.effective_kerning_step_upm(false);
        if self.active_tool == CanvasTool::Text && !self.text_stream.slots.is_empty() {
            let k_step = step.round() as i32;
            let k_step = if k_step == 0 {
                if delta_ticks < 0.0 {
                    -1
                } else {
                    1
                }
            } else {
                k_step
            };
            if self.text_stream.active_slot_index > 0 {
                return (self.adjust_left_kerning(k_step), false);
            }
            if self.text_stream.slots.len() > 1 {
                return (self.adjust_right_kerning(k_step), false);
            }
        }
        let (_, cur_rsb) = crate::core::compute_glyph_lsb_and_rsb(&self.contours, self.glyph.width);
        self.set_rsb(cur_rsb + step);
        (true, false)
    }

    pub(crate) fn compute_primary_rotary_telemetry(&self) -> RotaryTelemetryInfo {
        if self.active_tool == CanvasTool::Stem {
            if let Some(stem) = self.detect_structural_stems().first() {
                return RotaryTelemetryInfo {
                    parameter_name: "Stem Width",
                    formatted_value: format!("{:.0} UPM", stem.width_upm),
                    normalized_ratio: (stem.width_upm / ROTARY_CORNER_MAX_RADIUS_UPM)
                        .clamp(0.0, 1.0),
                };
            }
        }
        if matches!(self.active_tool, CanvasTool::Trowel | CanvasTool::Cleaner) {
            let r = self.effective_brush_radius_dp();
            return RotaryTelemetryInfo {
                parameter_name: "Brush Radius",
                formatted_value: format!("{r:.0} dp"),
                normalized_ratio: f64::from(
                    (r - BRUSH_MIN_RADIUS_DP) / (BRUSH_MAX_RADIUS_DP - BRUSH_MIN_RADIUS_DP),
                )
                .clamp(0.0, 1.0),
            };
        }
        if let Some(&sel_id) = self.selected_nodes.first() {
            if let Some(sc) = self.smart_corners.iter().find(|s| s.node_id == sel_id) {
                let span = ROTARY_CORNER_MAX_RADIUS_UPM - ROTARY_CORNER_MIN_RADIUS_UPM;
                return RotaryTelemetryInfo {
                    parameter_name: "Corner Radius",
                    formatted_value: format!("{:+.0} UPM", sc.radius_upm),
                    normalized_ratio: ((sc.radius_upm - ROTARY_CORNER_MIN_RADIUS_UPM) / span)
                        .clamp(0.0, 1.0),
                };
            }
            if let Some(node) = self.find_node(sel_id) {
                if node.node_type == NodeType::Corner {
                    let span = ROTARY_CORNER_MAX_RADIUS_UPM - ROTARY_CORNER_MIN_RADIUS_UPM;
                    return RotaryTelemetryInfo {
                        parameter_name: "Corner Radius",
                        formatted_value: "+0 UPM".to_string(),
                        normalized_ratio: ((-ROTARY_CORNER_MIN_RADIUS_UPM) / span).clamp(0.0, 1.0),
                    };
                }
                if let NodeType::HobbyStar { tension_in, .. } = node.node_type {
                    let span = HOBBY_TENSION_MAX - HOBBY_TENSION_MIN;
                    return RotaryTelemetryInfo {
                        parameter_name: "Hobby Tension",
                        formatted_value: format!("{tension_in:.2}"),
                        normalized_ratio: ((tension_in - HOBBY_TENSION_MIN) / span).clamp(0.0, 1.0),
                    };
                }
            }
        }
        if let Some(ov) = self.compute_tunni_overlays().first() {
            let r = 0.5 * (ov.ratio_p1 + ov.ratio_p2);
            return RotaryTelemetryInfo {
                parameter_name: "Tunni Ratio",
                formatted_value: format!("{:.0}%", r * PERCENT_SCALE_FACTOR),
                normalized_ratio: ((r - TUNNI_MIN_RATIO) / (TUNNI_MAX_RATIO - TUNNI_MIN_RATIO))
                    .clamp(0.0, 1.0),
            };
        }
        RotaryTelemetryInfo {
            parameter_name: "Zoom",
            formatted_value: format!(
                "{:.0}%",
                f64::from(self.viewport.zoom) * PERCENT_SCALE_FACTOR
            ),
            normalized_ratio: f64::from(
                (self.viewport.zoom - MIN_VIEWPORT_ZOOM) / (MAX_VIEWPORT_ZOOM - MIN_VIEWPORT_ZOOM),
            )
            .clamp(0.0, 1.0),
        }
    }
}
