//! `SPEC-014` & `SPEC-015` Bimanual Mobile & Tablet Thumb HUD (`MacContextThumbDial` + `MacEdgeThumbwheel`)
//! for `vector_canvas_demo` (`Stap 19.1`).

use super::toolbar_specs::{MAX_VIEWPORT_ZOOM, MIN_VIEWPORT_ZOOM};
use super::{MockFontHost, StudioWorkspaceMode};
use egui::{Event, Id, Order, Pos2, Rect, Ui, UiBuilder, Vec2};
use egui_macview::{
    MacAppearance, MacContextThumbDial, MacEdgeThumbwheel, MacTheme, MacThumbArcPlacement,
    MacThumbContextPreset, MacThumbDockZone, MacThumbIntent, MacThumbIntentMask,
};
use grafia::adapters::{CanvasTool, GrafiaCanvasState};
use grafia::core::{
    expanded_nodes_with_attached_handles, CanvasRotaryChannel, InputDeviceKind, KineticPlaybackMode,
};

const NARROW_PHONE_WIDTH_DP: f32 = 480.0;
const NARROW_PHONE_DIAL_SCALE: f32 = 0.84;
const STANDARD_DIAL_SCALE: f32 = 0.94;
const PAIRED_HEIGHT_DIVISOR_DP: f32 = 262.0;
const PAIRED_MIN_SCALE: f32 = 0.72;
const PAIRED_MAX_SCALE: f32 = 1.0;
const EDGE_WHEEL_MIN_PCT: f32 = 10.0;
const EDGE_WHEEL_MAX_PCT: f32 = 400.0;
const PERCENT_FACTOR: f32 = 100.0;

fn effective_dial_scale(canvas_rect: Rect) -> f32 {
    let max_p = ((canvas_rect.height() - 20.0) / PAIRED_HEIGHT_DIVISOR_DP)
        .clamp(PAIRED_MIN_SCALE, PAIRED_MAX_SCALE);
    let base = if canvas_rect.width() < NARROW_PHONE_WIDTH_DP {
        NARROW_PHONE_DIAL_SCALE
    } else {
        STANDARD_DIAL_SCALE
    };
    base.min(max_p)
}

fn compute_mobile_hud_hit_rects(canvas_rect: Rect, s: f32, dock_right: bool) -> (Rect, Rect) {
    let tm = MacTheme::new(MacAppearance::Aqua);
    let m = tm.thumb_dial();
    let pad = m.dock_margin.points() * s;
    let side = (m.skirt_outer_radius.points() + m.bevel_width_pt.points()) * 2.0 * s;
    let dial_x = if dock_right {
        canvas_rect.max.x - side - pad
    } else {
        canvas_rect.min.x + pad
    };
    let dial_r = Rect::from_min_size(
        Pos2::new(dial_x, canvas_rect.max.y - side - pad),
        Vec2::splat(side),
    );
    let (ww, wh) = (
        m.edge_wheel_width.points() * s,
        m.edge_wheel_height.points() * s,
    );
    let wheel_r = Rect::from_min_size(
        Pos2::new(
            canvas_rect.max.x - ww - pad,
            canvas_rect.min.y + pad * 2.0 + m.undo_bar_height.points(),
        ),
        Vec2::new(ww, wh),
    );
    (dial_r, wheel_r)
}

fn map_tool_to_thumb_preset(tool: CanvasTool) -> MacThumbContextPreset {
    match tool {
        CanvasTool::Pen | CanvasTool::Pencil => MacThumbContextPreset::Pen,
        CanvasTool::Primitives | CanvasTool::Scale | CanvasTool::Rotate => {
            MacThumbContextPreset::Primitives
        }
        CanvasTool::Knife | CanvasTool::ShapeBuilder => MacThumbContextPreset::Knife,
        _ => MacThumbContextPreset::Select,
    }
}

impl MockFontHost {
    pub fn set_mobile_thumb_hud_enabled(&mut self, enabled: bool, state: &mut GrafiaCanvasState) {
        self.show_mobile_thumb_hud = enabled;
        let mut prefs = state.preferences.clone();
        prefs.show_touch_precision_loupe = false;
        prefs.touch_finger_offset_dp = 0.0;
        state.apply_preferences(&prefs);
        if enabled {
            state.set_active_input_device(InputDeviceKind::TouchFinger);
            self.push_log("Bimanueel Schermrand Thumb Dial & Edge Wheel actief");
        } else {
            state.set_active_input_device(InputDeviceKind::Mouse);
            let ts = &mut state.drag_transients.touch_state;
            (ts.touch_shift_latched, ts.touch_alt_latched) = (false, false);
            (ts.touch_cmd_latched, ts.touch_space_pan_latched) = (false, false);
            self.thumb_latched_mask = MacThumbIntentMask::NONE;
        }
    }

    pub fn take_mobile_hud_guarded_events(&self, ui: &Ui, avail: Vec2) -> Vec<Event> {
        if !self.show_mobile_thumb_hud {
            return Vec::new();
        }
        let canvas_rect = Rect::from_min_size(ui.cursor().min, avail);
        let (dial_r, wheel_r) = compute_mobile_hud_hit_rects(
            canvas_rect,
            effective_dial_scale(canvas_rect),
            self.thumb_dock_right,
        );
        ui.ctx().input_mut(|i| {
            let mut stashed = Vec::new();
            i.events.retain(|ev| {
                let in_hud = match ev {
                    Event::PointerButton { pos, .. }
                    | Event::PointerMoved(pos)
                    | Event::Touch { pos, .. } => dial_r.contains(*pos) || wheel_r.contains(*pos),
                    _ => false,
                };
                if in_hud {
                    stashed.push(ev.clone());
                }
                !in_hud
            });
            stashed
        })
    }

    pub fn restore_mobile_hud_guarded_events(ui: &Ui, stashed: Vec<Event>) {
        if !stashed.is_empty() {
            ui.ctx().input_mut(|i| i.events.extend(stashed));
        }
    }

    pub fn render_mobile_thumb_hud_overlay(
        &mut self,
        ui: &mut Ui,
        canvas_rect: Rect,
        state: &mut GrafiaCanvasState,
    ) {
        if !self.show_mobile_thumb_hud {
            return;
        }
        let mac_theme = self.active_mac_theme();
        let scale = effective_dial_scale(canvas_rect);
        egui::Area::new(Id::NULL.with("grafia_thumb_pie_menu_area"))
            .order(Order::Foreground)
            .fixed_pos(canvas_rect.min)
            .show(ui.ctx(), |area_ui| {
                area_ui.set_clip_rect(canvas_rect);
                area_ui.scope_builder(
                    UiBuilder::new()
                        .id_salt("grafia_thumb_pie_scope")
                        .max_rect(canvas_rect),
                    |hud_ui| {
                        hud_ui.set_clip_rect(canvas_rect);
                        self.render_paired_thumb_dial_and_drum(
                            hud_ui,
                            canvas_rect,
                            state,
                            &mac_theme,
                            scale,
                        );
                    },
                );
            });
    }

    fn render_paired_thumb_dial_and_drum(
        &mut self,
        ui: &mut Ui,
        canvas_rect: Rect,
        state: &mut GrafiaCanvasState,
        mac_theme: &MacTheme,
        scale: f32,
    ) {
        let ts = &state.drag_transients.touch_state;
        if !ts.touch_shift_latched
            && !ts.touch_alt_latched
            && !ts.touch_cmd_latched
            && !ts.touch_space_pan_latched
        {
            self.thumb_latched_mask = MacThumbIntentMask::NONE;
        }
        let dock_zone = if self.thumb_dock_right {
            MacThumbDockZone::BottomRight
        } else {
            MacThumbDockZone::BottomLeft
        };
        let tel = state.controller_telemetry();
        self.thumb_rotary_norm = (tel.primary_rotary.normalized_ratio as f32).clamp(0.0, 1.0);
        let dial_resp = MacContextThumbDial::new(
            "grafia_mobile_thumb_dial",
            &mut self.thumb_latched_mask,
            &mut self.thumb_rotary_norm,
        )
        .layer(&mut self.thumb_dial_layer)
        .undo_arc_placement(MacThumbArcPlacement::TopCrown)
        .preset(map_tool_to_thumb_preset(state.active_tool))
        .rotary_parameter(
            tel.primary_rotary.parameter_name,
            &tel.primary_rotary.formatted_value,
        )
        .can_undo(state.history.can_undo())
        .can_redo(state.history.can_redo())
        .dock_in_rect(canvas_rect, dock_zone)
        .scale(scale)
        .theme(mac_theme)
        .show(ui);
        Self::apply_thumb_dial_response(state, &dial_resp);
        self.render_paired_edge_thumbwheel(ui, canvas_rect, state, mac_theme, scale);
    }

    fn apply_thumb_dial_response(
        state: &mut GrafiaCanvasState,
        resp: &egui_macview::MacContextThumbResponse,
    ) {
        let eff = resp.effective_mask;
        let ts = &mut state.drag_transients.touch_state;
        ts.touch_shift_latched = eff.contains(MacThumbIntent::Constrain);
        ts.touch_alt_latched = eff.contains(MacThumbIntent::Alter);
        ts.touch_cmd_latched = eff.contains(MacThumbIntent::Glide);
        ts.touch_space_pan_latched = eff.contains(MacThumbIntent::MovePan);
        if resp.undo_clicked {
            let _ = state.undo();
        }
        if resp.redo_clicked {
            let _ = state.redo();
        }
        if resp.rotary_delta_ticks.abs() > f32::EPSILON {
            let _ = state.step_rotary_encoder(
                CanvasRotaryChannel::ContextPrimary,
                f64::from(resp.rotary_delta_ticks),
            );
        }
        if resp.nudge_delta.length_sq() > 0.0 {
            Self::apply_joystick_nudge_or_pan(state, resp.nudge_delta, eff);
        }
    }

    fn apply_joystick_nudge_or_pan(
        state: &mut GrafiaCanvasState,
        nudge_dp: egui::Vec2,
        eff: egui_macview::MacThumbIntentMask,
    ) {
        if state.selected_nodes.is_empty() || eff.contains(MacThumbIntent::MovePan) {
            state.viewport.pan_offset.x.0 -= nudge_dp.x;
            state.viewport.pan_offset.y.0 -= nudge_dp.y;
            state.bump_content_generation();
            return;
        }
        let z = f64::from(state.viewport.zoom.max(MIN_VIEWPORT_ZOOM));
        let (dx, dy) = (f64::from(nudge_dp.x) / z, f64::from(-nudge_dp.y) / z);
        let inc_bcp = !eff.contains(MacThumbIntent::Alter);
        for id in expanded_nodes_with_attached_handles(
            &state.contours,
            &state.closed_flags(),
            &state.selected_nodes,
            inc_bcp,
        ) {
            if let Some(n) = state.find_node_mut(id) {
                (n.point.x.0, n.point.y.0) = (n.point.x.0 + dx, n.point.y.0 + dy);
            }
        }
        state.sync_paths_from_contours();
        state.assert_invariants();
    }

    fn render_paired_edge_thumbwheel(
        &mut self,
        ui: &mut Ui,
        canvas_rect: Rect,
        state: &mut GrafiaCanvasState,
        mac_theme: &MacTheme,
        scale: f32,
    ) {
        let scrub_kin = self.studio_mode == StudioWorkspaceMode::IconStudio
            && state.icon_studio.kinetic.is_active();
        let (id, range, cur) = if scrub_kin {
            let v = match state.icon_studio.kinetic.playback_mode {
                KineticPlaybackMode::VariableValue(val) => (val as f32) * PERCENT_FACTOR,
                _ => (state.icon_studio.kinetic.phase_t as f32) * PERCENT_FACTOR,
            };
            (
                "grafia_ew_k",
                0.0..=PERCENT_FACTOR,
                v.clamp(0.0, PERCENT_FACTOR),
            )
        } else {
            let v = (state.viewport.zoom * PERCENT_FACTOR)
                .clamp(EDGE_WHEEL_MIN_PCT, EDGE_WHEEL_MAX_PCT);
            ("grafia_ew_z", EDGE_WHEEL_MIN_PCT..=EDGE_WHEEL_MAX_PCT, v)
        };
        self.edge_wheel_value_pct = cur;
        let w_resp = MacEdgeThumbwheel::new(id, &mut self.edge_wheel_value_pct, range)
            .dock_right_edge(canvas_rect)
            .scale(scale)
            .theme(mac_theme)
            .show(ui);
        if w_resp.value_delta.abs() <= f32::EPSILON {
            return;
        }
        if scrub_kin {
            let norm = f64::from((self.edge_wheel_value_pct / PERCENT_FACTOR).clamp(0.0, 1.0));
            if matches!(
                state.icon_studio.kinetic.playback_mode,
                KineticPlaybackMode::VariableValue(_)
            ) {
                state.set_kinetic_playback_mode(KineticPlaybackMode::VariableValue(norm));
            } else {
                state.icon_studio.kinetic.phase_t = norm;
                state.bump_content_generation();
            }
        } else {
            let old_z = state.viewport.zoom.max(MIN_VIEWPORT_ZOOM);
            let new_z = (self.edge_wheel_value_pct / PERCENT_FACTOR)
                .clamp(MIN_VIEWPORT_ZOOM, MAX_VIEWPORT_ZOOM);
            let c = canvas_rect.center();
            let p = grafia::ScreenPoint::new(c.x, c.y);
            state.viewport.zoom_at_cursor(p, new_z / old_z);
        }
    }
}
