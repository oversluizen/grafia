//! `SPEC-009` (`grafia v0.8.0` Part B) `GrafiaCanvasState` methods for `trace_active_background_image`,
//! `apply_preferences`, wheel/zoom preference routing, and multi-touch / finger-offset / 2-finger tap Undo.

use crate::adapters::state::GrafiaCanvasState;
use crate::core::{trace_background_image, ContourId, GlyphContour, NodeId, ScreenPoint};
use crate::tokens::{
    AutoTraceConfig, GrafiaPreferences, ALPHA_MAX_F32, ALPHA_MIN_F32, PREF_MAX_RADIUS_DP,
    PREF_MIN_FREEHAND_SMOOTHING_PX, PREF_MIN_RADIUS_DP, PREF_MIN_STEP_UPM, STROKE_WIDTH_MAX_DP,
    STROKE_WIDTH_MIN_DP, TOUCH_MIN_PINCH_DIST_DP, TOUCH_TWO_FINGER_TAP_MAX_MOVE_DP,
    WHEEL_ZOOM_SENSITIVITY,
};
use egui::{Event, Modifiers as EguiModifiers, Pos2, Rect, TouchPhase};

fn sanitize_positive_f64(val: f64, fallback: f64) -> f64 {
    if val.is_finite() && val >= PREF_MIN_STEP_UPM {
        val
    } else {
        fallback
    }
}

fn sanitize_f32_non_neg(val: f64, fallback: f64) -> f64 {
    if val.is_finite() {
        val.max(0.0)
    } else {
        fallback
    }
}

fn clamp_pref_radius(v: f32, d: f32) -> f32 {
    if v.is_finite() {
        v.clamp(PREF_MIN_RADIUS_DP, PREF_MAX_RADIUS_DP)
    } else {
        d
    }
}

fn sanitize_preferences(prefs: &GrafiaPreferences) -> GrafiaPreferences {
    let def = GrafiaPreferences::default();
    let pos = sanitize_positive_f64;
    let grid_step = sanitize_f32_non_neg(prefs.grid_step_upm, def.grid_step_upm);
    GrafiaPreferences {
        nudge_step_small_upm: pos(prefs.nudge_step_small_upm, def.nudge_step_small_upm),
        nudge_step_medium_upm: pos(prefs.nudge_step_medium_upm, def.nudge_step_medium_upm),
        nudge_step_large_upm: pos(prefs.nudge_step_large_upm, def.nudge_step_large_upm),
        spacing_step_low_upm: pos(prefs.spacing_step_low_upm, def.spacing_step_low_upm),
        spacing_step_high_upm: pos(prefs.spacing_step_high_upm, def.spacing_step_high_upm),
        kerning_step_low_upm: pos(prefs.kerning_step_low_upm, def.kerning_step_low_upm),
        kerning_step_high_upm: pos(prefs.kerning_step_high_upm, def.kerning_step_high_upm),
        spacing_increment_low_upm: pos(
            prefs.spacing_increment_low_upm,
            def.spacing_increment_low_upm,
        ),
        spacing_increment_high_upm: pos(
            prefs.spacing_increment_high_upm,
            def.spacing_increment_high_upm,
        ),
        kerning_increment_low_upm: pos(
            prefs.kerning_increment_low_upm,
            def.kerning_increment_low_upm,
        ),
        kerning_increment_high_upm: pos(
            prefs.kerning_increment_high_upm,
            def.kerning_increment_high_upm,
        ),
        grid_step_upm: grid_step,
        grid_subdivisions: prefs.grid_subdivisions.max(1),
        round_to_grid: prefs.round_to_grid && grid_step > 0.0,
        snap_radius_dp: clamp_pref_radius(prefs.snap_radius_dp, def.snap_radius_dp),
        snap_targets: prefs.snap_targets,
        hit_test_radius_dp: clamp_pref_radius(prefs.hit_test_radius_dp, def.hit_test_radius_dp),
        segment_hit_tolerance_dp: clamp_pref_radius(
            prefs.segment_hit_tolerance_dp,
            def.segment_hit_tolerance_dp,
        ),
        text_mode_numbers_threshold: sanitize_f32_non_neg(
            prefs.text_mode_numbers_threshold,
            def.text_mode_numbers_threshold,
        ),
        scroll_wheel_zooms: prefs.scroll_wheel_zooms,
        invert_zoom_direction: prefs.invert_zoom_direction,
        fill_active_glyph_while_editing: prefs.fill_active_glyph_while_editing,
        passive_neighbor_opacity: if prefs.passive_neighbor_opacity.is_finite() {
            prefs
                .passive_neighbor_opacity
                .clamp(ALPHA_MIN_F32, ALPHA_MAX_F32)
        } else {
            def.passive_neighbor_opacity
        },
        italic_angle_compensates_bbox_and_nudge: prefs.italic_angle_compensates_bbox_and_nudge,
        freehand_smoothing_tolerance_px: sanitize_f32_non_neg(
            prefs.freehand_smoothing_tolerance_px,
            def.freehand_smoothing_tolerance_px,
        )
        .max(PREF_MIN_FREEHAND_SMOOTHING_PX),
        touch_finger_offset_dp: if prefs.touch_finger_offset_dp.is_finite() {
            prefs.touch_finger_offset_dp
        } else {
            0.0
        },
        show_touch_precision_loupe: prefs.show_touch_precision_loupe,
        two_finger_pan_zoom_enabled: prefs.two_finger_pan_zoom_enabled,
        two_finger_tap_undo_enabled: prefs.two_finger_tap_undo_enabled,
    }
}

impl GrafiaCanvasState {
    /// Traces `self.background_image` into foreground contours as a single Undo step `"Trace Background Image"` (`SPEC-009 §2.5.2`).
    pub fn trace_active_background_image(&mut self, config: &AutoTraceConfig) -> bool {
        let Some(bg) = self.background_image.clone() else {
            return false;
        };
        let traced = trace_background_image(&bg, config);
        if traced.is_empty() {
            return false;
        }
        let before = self.snapshot();
        let base_c_idx = self.contours.len();
        for (offset, mut gc) in traced.into_iter().enumerate() {
            for node in &mut gc.nodes {
                node.id = NodeId(self.next_node_id);
                self.next_node_id += 1;
            }
            gc.id = ContourId(base_c_idx + offset + 1);
            self.contours.push(gc.nodes.clone());
            let mut new_gc = GlyphContour::new(gc.id, gc.nodes);
            new_gc.closed = true;
            new_gc.visible = true;
            self.glyph.contours.push(new_gc);
        }
        self.active_contour_closed = true;
        self.active_open_contour = None;
        self.sync_paths_from_contours();
        self.record_mutation("Trace Background Image", before);
        self.assert_invariants();
        true
    }

    /// Applies typed host `GrafiaPreferences` to canvas editing, snapping, grid, rendering, and touch ergonomics (`SPEC-009 §2.6.3`).
    pub fn apply_preferences(&mut self, prefs: &GrafiaPreferences) {
        let sanitized = sanitize_preferences(prefs);
        self.grid_step_upm = sanitized.grid_step_upm;
        self.grid_subdivisions = sanitized.grid_subdivisions;
        self.round_to_grid = sanitized.round_to_grid;
        self.text_transients.text_mode_numbers_threshold = sanitized.text_mode_numbers_threshold;
        self.theme.snapping.snap_threshold_dp = sanitized.snap_radius_dp;
        self.theme.edit_canvas.guide_stroke_width_dp = self
            .theme
            .edit_canvas
            .guide_stroke_width_dp
            .clamp(STROKE_WIDTH_MIN_DP, STROKE_WIDTH_MAX_DP);
        self.theme.edit_canvas.metric_line_width_dp = self
            .theme
            .edit_canvas
            .metric_line_width_dp
            .clamp(STROKE_WIDTH_MIN_DP, STROKE_WIDTH_MAX_DP);
        self.preferences = sanitized;
        self.bump_content_generation();
    }

    /// Returns the effective Pen tool node hit-test radius in `dp` (`TC-MATURITY-007`).
    #[must_use]
    pub fn effective_pen_node_hit_radius_dp(&self) -> f32 {
        let r = self.preferences.hit_test_radius_dp;
        let base = if r.is_finite() && (r - crate::tokens::SELECT_NODE_HIT_RADIUS_DP).abs() > 1e-5 {
            r.clamp(PREF_MIN_RADIUS_DP, PREF_MAX_RADIUS_DP)
        } else {
            crate::tokens::PEN_NODE_HIT_RADIUS_DP
        };
        if self.controller_ports.active_input_device == crate::core::InputDeviceKind::TouchFinger {
            base * crate::core::math_consts::TOUCH_HIT_RADIUS_INFLATION_FACTOR
        } else {
            base
        }
    }

    /// Returns the effective contour segment hit-test tolerance in `dp` (`TC-MATURITY-007`, `TC-V0130-011`).
    #[must_use]
    pub fn effective_segment_hit_tolerance_dp(&self) -> f32 {
        let base = clamp_pref_radius(
            self.preferences.segment_hit_tolerance_dp,
            crate::tokens::SEGMENT_HIT_TOLERANCE_DP,
        );
        self.scale_touch_hit_radius_dp(base)
    }

    /// Returns the effective kerning keyboard step in UPM (`high == true` for `Shift`).
    #[must_use]
    pub fn effective_kerning_step_upm(&self, high: bool) -> f64 {
        let (inc, step, def) = if high {
            (
                self.preferences.kerning_increment_high_upm,
                self.preferences.kerning_step_high_upm,
                crate::tokens::PREF_DEFAULT_KERNING_HIGH_UPM,
            )
        } else {
            (
                self.preferences.kerning_increment_low_upm,
                self.preferences.kerning_step_low_upm,
                crate::tokens::PREF_DEFAULT_KERNING_LOW_UPM,
            )
        };
        let chosen = if (inc - def).abs() > 1e-9 { inc } else { step };
        sanitize_positive_f64(chosen, def)
    }

    /// Returns the effective spacing (`LSB`/`RSB`/`TSB`/`BSB`) keyboard step in UPM (`high == true` for `Shift`).
    #[must_use]
    pub fn effective_spacing_step_upm(&self, high: bool) -> f64 {
        let (inc, step, def) = if high {
            (
                self.preferences.spacing_increment_high_upm,
                self.preferences.spacing_step_high_upm,
                crate::tokens::PREF_DEFAULT_SPACING_HIGH_UPM,
            )
        } else {
            (
                self.preferences.spacing_increment_low_upm,
                self.preferences.spacing_step_low_upm,
                crate::tokens::PREF_DEFAULT_SPACING_LOW_UPM,
            )
        };
        let chosen = if (inc - def).abs() > 1e-9 { inc } else { step };
        sanitize_positive_f64(chosen, def)
    }

    #[must_use]
    pub(crate) fn map_pointer_pos_with_touch_offset(&self, raw: Pos2) -> ScreenPoint {
        if (!self.drag_transients.touch_state.contacts.is_empty()
            || self.drag_transients.touch_state.egui_any_touches_in_frame
            || self.controller_ports.active_input_device
                == crate::core::InputDeviceKind::TouchFinger)
            && self.preferences.touch_finger_offset_dp != 0.0
        {
            ScreenPoint::new(raw.x, raw.y - self.preferences.touch_finger_offset_dp)
        } else {
            ScreenPoint::new(raw.x, raw.y)
        }
    }

    pub(crate) fn handle_mouse_wheel_with_prefs(
        &mut self,
        delta: egui::Vec2,
        modifiers: EguiModifiers,
        is_cmd: bool,
        is_ctrl: bool,
        is_alt: bool,
        canvas_rect: Rect,
    ) -> bool {
        let mod_held =
            modifiers.command || modifiers.ctrl || modifiers.alt || is_cmd || is_ctrl || is_alt;
        let should_zoom = if self.preferences.scroll_wheel_zooms {
            true
        } else {
            mod_held
        };
        if should_zoom {
            let cursor_scr = self.last_pointer_screen_pos.unwrap_or_else(|| {
                ScreenPoint::new(canvas_rect.center().x, canvas_rect.center().y)
            });
            let signed_dy = if self.preferences.invert_zoom_direction {
                -delta.y
            } else {
                delta.y
            };
            let step = if signed_dy.abs() <= 2.0 {
                signed_dy * 0.5 * 0.5
            } else {
                signed_dy * WHEEL_ZOOM_SENSITIVITY
            };
            let factor = (1.0 + step).clamp(0.5, 2.0);
            self.viewport.zoom_at_cursor(cursor_scr, factor);
        } else {
            self.viewport.pan_offset.x.0 += delta.x;
            self.viewport.pan_offset.y.0 += delta.y;
        }
        true
    }

    pub(crate) fn handle_touch_event(&mut self, event: &Event) -> Option<bool> {
        let Event::Touch { id, phase, pos, .. } = event else {
            return None;
        };
        let key = id.0;
        self.drag_transients.touch_state.native_pointer_drag_active = false;
        self.drag_transients.touch_state.native_pointer_raw_pos = None;
        match phase {
            TouchPhase::Start => {
                if self.drag_transients.touch_state.contacts.is_empty() {
                    self.drag_transients.touch_state.active_primary_touch_id = Some(key);
                }
                self.drag_transients
                    .touch_state
                    .contacts
                    .insert(key, (*pos, *pos));
                self.drag_transients.touch_state.active_touch_raw_pos = Some(*pos);
                if self.drag_transients.touch_state.contacts.len() == 2 {
                    if let Some(before) = self.drag_transients.active_drag_before_snapshot.take() {
                        self.restore_snapshot(&before);
                    }
                    self.drag_transients.touch_state.two_finger_peak = true;
                    self.select_state = crate::interaction::SelectToolState::Idle;
                    self.pen_state = crate::interaction::PenToolState::Idle;
                    self.drawing_tool_state.reset_in_flight_drags();
                    self.active_snappers.clear();
                    self.curve_harmony.active_tangent_snap = None;
                }
                let eff = self.map_pointer_pos_with_touch_offset(*pos);
                self.cursor_screen_pos = Some(eff);
                self.last_pointer_screen_pos = Some(eff);
                Some(false)
            }
            TouchPhase::Move => Some(self.handle_touch_move(key, *pos)),
            TouchPhase::End | TouchPhase::Cancel => {
                self.drag_transients.touch_state.contacts.remove(&key);
                if self.drag_transients.touch_state.contacts.is_empty() {
                    let ts = &mut self.drag_transients.touch_state;
                    ts.active_touch_raw_pos = None;
                    ts.active_primary_touch_id = None;
                    ts.primary_press_active = false;
                    ts.native_pointer_drag_active = false;
                    ts.native_pointer_raw_pos = None;
                    ts.rejected_palm_contacts.clear();
                    let was_tap = *phase == TouchPhase::End
                        && self.drag_transients.touch_state.two_finger_peak
                        && self.drag_transients.touch_state.max_contact_excursion_dp
                            <= TOUCH_TWO_FINGER_TAP_MAX_MOVE_DP
                        && self.preferences.two_finger_tap_undo_enabled;
                    self.drag_transients.touch_state.two_finger_peak = false;
                    self.drag_transients.touch_state.max_contact_excursion_dp = 0.0;
                    if was_tap {
                        return Some(self.undo());
                    }
                }
                Some(false)
            }
        }
    }

    fn handle_touch_move(&mut self, key: u64, pos: Pos2) -> bool {
        let prev_contacts: Vec<Pos2> = self
            .drag_transients
            .touch_state
            .contacts
            .values()
            .map(|(_, cur)| *cur)
            .collect();
        let ts = &mut self.drag_transients.session_config.touch_state;
        if let Some((start_p, cur_p)) = ts.contacts.get_mut(&key) {
            let exc = start_p.distance(pos);
            if exc > ts.max_contact_excursion_dp {
                ts.max_contact_excursion_dp = exc;
            }
            *cur_p = pos;
        }
        ts.active_touch_raw_pos = Some(pos);
        let new_contacts: Vec<Pos2> = self
            .drag_transients
            .touch_state
            .contacts
            .values()
            .map(|(_, cur)| *cur)
            .collect();
        if prev_contacts.len() == 2
            && new_contacts.len() == 2
            && self.preferences.two_finger_pan_zoom_enabled
        {
            let old_mid = Pos2::new(
                (prev_contacts[0].x + prev_contacts[1].x) * 0.5,
                (prev_contacts[0].y + prev_contacts[1].y) * 0.5,
            );
            let new_mid = Pos2::new(
                (new_contacts[0].x + new_contacts[1].x) * 0.5,
                (new_contacts[0].y + new_contacts[1].y) * 0.5,
            );
            self.viewport.pan_offset.x.0 += new_mid.x - old_mid.x;
            self.viewport.pan_offset.y.0 += new_mid.y - old_mid.y;
            let old_d = prev_contacts[0].distance(prev_contacts[1]);
            let new_d = new_contacts[0].distance(new_contacts[1]);
            if old_d >= TOUCH_MIN_PINCH_DIST_DP && new_d >= TOUCH_MIN_PINCH_DIST_DP {
                let ratio = (new_d / old_d).clamp(0.5, 2.0);
                self.viewport
                    .zoom_at_cursor(ScreenPoint::new(new_mid.x, new_mid.y), ratio);
            }
            return true;
        }
        let eff = self.map_pointer_pos_with_touch_offset(pos);
        self.cursor_screen_pos = Some(eff);
        self.last_pointer_screen_pos = Some(eff);
        false
    }
}
