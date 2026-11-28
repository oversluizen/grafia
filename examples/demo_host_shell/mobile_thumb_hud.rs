//! `SPEC-014` & `SPEC-015` Bimanual Mobile & Tablet Thumb HUD (`MacContextThumbDial` + `MacEdgeThumbwheel`)
//! for `vector_canvas_demo` (`Stap 19.1`).

use super::toolbar_specs::{MAX_VIEWPORT_ZOOM, MIN_VIEWPORT_ZOOM, TOOL_ICON_SPECS};
use super::{MockFontHost, StudioWorkspaceMode};
use egui::{Event, Id, Order, Pos2, Rect, Ui, UiBuilder, Vec2};
use egui_macview::{
    MacAppearance, MacContextThumbDial, MacEdgeThumbwheel, MacSymbol, MacTheme,
    MacThumbArcPlacement, MacThumbContextPreset, MacThumbDockZone, MacThumbIntent,
    MacThumbIntentMask,
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

#[rustfmt::skip]
fn compute_mobile_hud_hit_rects(canvas_rect: Rect, s: f32, dock_right: bool) -> (Rect, Rect) {
    let tm = MacTheme::new(MacAppearance::Aqua);
    let m = tm.thumb_dial();
    let pad = m.dock_margin.points() * s;
    let side = (m.skirt_outer_radius.points() + m.bevel_width_pt.points()) * 2.0 * s;
    let dx = if dock_right { canvas_rect.max.x - side - pad } else { canvas_rect.min.x + pad };
    let dial_r = Rect::from_min_size(Pos2::new(dx, canvas_rect.max.y - side - pad), Vec2::splat(side));
    let (ww, wh) = ((side * 0.78).round(), (m.edge_wheel_width.points() * 1.35 * s).round());
    let wx = (dial_r.center().x - ww * 0.5).clamp(canvas_rect.min.x + pad, (canvas_rect.max.x - ww - pad).max(canvas_rect.min.x + pad));
    let wy = (dial_r.min.y - wh - pad * 0.55).max(canvas_rect.min.y + pad);
    (dial_r, Rect::from_min_size(Pos2::new(wx, wy), Vec2::new(ww, wh)))
}

#[rustfmt::skip]
fn resolve_active_wheel_icon(state: &GrafiaCanvasState, is_icon_studio: bool, (kin, krn, ctx): (bool, bool, bool)) -> &'static str {
    if kin {
        if state.glyph.name.contains("mic") { "\u{f7c9}" } else if state.glyph.name.contains("orbit") { "\u{f544}" } else { "\u{f7ff}" }
    } else if krn {
        "\u{f892}"
    } else if is_icon_studio && !state.color_sublayers.is_empty() {
        "\u{f77e}"
    } else if ctx {
        TOOL_ICON_SPECS.iter().find(|(t, _, _)| *t == state.active_tool).map_or("\u{f6e5}", |(_, ic, _)| *ic)
    } else {
        MacSymbol::Search.as_str()
    }
}

#[rustfmt::skip]
fn map_tool_to_thumb_preset(tool: CanvasTool) -> MacThumbContextPreset {
    match tool {
        CanvasTool::Pen | CanvasTool::Pencil => MacThumbContextPreset::Pen,
        CanvasTool::Primitives | CanvasTool::Scale | CanvasTool::Rotate => MacThumbContextPreset::Primitives,
        CanvasTool::Knife | CanvasTool::ShapeBuilder => MacThumbContextPreset::Knife,
        _ => MacThumbContextPreset::Select,
    }
}

impl MockFontHost {
    #[rustfmt::skip]
    pub fn set_mobile_thumb_hud_enabled(&mut self, enabled: bool, state: &mut GrafiaCanvasState) {
        self.show_mobile_thumb_hud = enabled;
        let mut prefs = state.preferences.clone();
        (prefs.show_touch_precision_loupe, prefs.touch_finger_offset_dp) = (false, 0.0);
        state.apply_preferences(&prefs);
        if enabled {
            state.set_active_input_device(InputDeviceKind::TouchFinger);
            self.push_log("Bimanueel Schermrand Thumb Dial & Edge Wheel actief");
        } else {
            state.set_active_input_device(InputDeviceKind::Mouse);
            let ts = &mut state.drag_transients.touch_state;
            (ts.touch_shift_latched, ts.touch_alt_latched, ts.touch_cmd_latched, ts.touch_space_pan_latched) = (false, false, false, false);
            self.thumb_latched_mask = MacThumbIntentMask::NONE;
        }
    }

    #[rustfmt::skip]
    pub fn take_mobile_hud_guarded_events(&self, ui: &Ui, avail: Vec2) -> Vec<Event> {
        if !self.show_mobile_thumb_hud { return Vec::new(); }
        let canvas_rect = Rect::from_min_size(ui.cursor().min, avail);
        let (dial_r, wheel_r) = compute_mobile_hud_hit_rects(canvas_rect, effective_dial_scale(canvas_rect), self.thumb_dock_right);
        ui.ctx().input_mut(|i| {
            let mut stashed = Vec::new();
            i.events.retain(|ev| {
                let in_hud = match ev {
                    Event::PointerButton { pos, .. } | Event::PointerMoved(pos) | Event::Touch { pos, .. } => dial_r.contains(*pos) || wheel_r.contains(*pos),
                    _ => false,
                };
                if in_hud { stashed.push(ev.clone()); }
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
        let builder = UiBuilder::new()
            .id_salt("grafia_thumb_pie_scope")
            .max_rect(canvas_rect);
        egui::Area::new(Id::NULL.with("grafia_thumb_pie_menu_area"))
            .order(Order::Foreground)
            .fixed_pos(canvas_rect.min)
            .show(ui.ctx(), |area_ui| {
                area_ui.set_clip_rect(canvas_rect);
                area_ui.scope_builder(builder, |hud_ui| {
                    hud_ui.set_clip_rect(canvas_rect);
                    self.render_paired_thumb_dial_and_drum(
                        hud_ui,
                        canvas_rect,
                        state,
                        &mac_theme,
                        scale,
                    );
                });
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
        if !(ts.touch_shift_latched
            || ts.touch_alt_latched
            || ts.touch_cmd_latched
            || ts.touch_space_pan_latched)
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

    #[rustfmt::skip]
    fn apply_thumb_dial_response(state: &mut GrafiaCanvasState, resp: &egui_macview::MacContextThumbResponse) {
        let eff = resp.effective_mask;
        let ts = &mut state.drag_transients.touch_state;
        ts.touch_shift_latched = eff.contains(MacThumbIntent::Constrain);
        ts.touch_alt_latched = eff.contains(MacThumbIntent::Alter);
        ts.touch_cmd_latched = eff.contains(MacThumbIntent::Glide);
        ts.touch_space_pan_latched = eff.contains(MacThumbIntent::MovePan);
        if resp.undo_clicked { let _ = state.undo(); }
        if resp.redo_clicked { let _ = state.redo(); }
        if resp.rotary_delta_ticks.abs() > f32::EPSILON {
            let _ = state.step_rotary_encoder(CanvasRotaryChannel::ContextPrimary, f64::from(resp.rotary_delta_ticks));
        }
        if resp.nudge_delta.length_sq() > 0.0 {
            Self::apply_joystick_nudge_or_pan(state, resp.nudge_delta, eff);
        }
    }

    #[rustfmt::skip]
    fn apply_joystick_nudge_or_pan(state: &mut GrafiaCanvasState, nudge_dp: egui::Vec2, eff: egui_macview::MacThumbIntentMask) {
        if state.selected_nodes.is_empty() || eff.contains(MacThumbIntent::MovePan) {
            state.viewport.pan_offset.x.0 -= nudge_dp.x;
            state.viewport.pan_offset.y.0 -= nudge_dp.y;
            state.bump_content_generation();
            return;
        }
        let z = f64::from(state.viewport.zoom.max(MIN_VIEWPORT_ZOOM));
        let (dx, dy) = (f64::from(nudge_dp.x) / z, f64::from(-nudge_dp.y) / z);
        for id in expanded_nodes_with_attached_handles(&state.contours, &state.closed_flags(), &state.selected_nodes, !eff.contains(MacThumbIntent::Alter)) {
            if let Some(n) = state.find_node_mut(id) {
                (n.point.x.0, n.point.y.0) = (n.point.x.0 + dx, n.point.y.0 + dy);
            }
        }
        state.sync_paths_from_contours();
        state.assert_invariants();
    }

    #[rustfmt::skip]
    fn render_paired_edge_thumbwheel(&mut self, ui: &mut Ui, canvas_rect: Rect, state: &mut GrafiaCanvasState, mac_theme: &MacTheme, scale: f32) {
        let (_, wheel_r) = compute_mobile_hud_hit_rects(canvas_rect, scale, self.thumb_dock_right);
        let is_ic = self.studio_mode == StudioWorkspaceMode::IconStudio;
        let kin = self.active_workspace == super::StudioSubWorkspace::IconKineticAndTouch || (is_ic && state.icon_studio.kinetic.is_active());
        let krn = !kin && state.active_tool == CanvasTool::Text && !state.text_stream.slots.is_empty();
        let ctx = !kin && !krn && (!state.selected_nodes.is_empty() || matches!(state.active_tool, CanvasTool::Stem | CanvasTool::Trowel | CanvasTool::Cleaner));
        let (id, range, cur) = if kin {
            let v = match state.icon_studio.kinetic.playback_mode {
                KineticPlaybackMode::VariableValue(val) => (val as f32) * PERCENT_FACTOR,
                _ => (state.icon_studio.kinetic.phase_t as f32) * PERCENT_FACTOR,
            };
            ("grafia_ew_k", 0.0..=PERCENT_FACTOR, v.clamp(0.0, PERCENT_FACTOR))
        } else if krn || ctx {
            let r = state.controller_telemetry().primary_rotary.normalized_ratio as f32;
            ("grafia_ew_c", 0.0..=PERCENT_FACTOR, (r * PERCENT_FACTOR).clamp(0.0, PERCENT_FACTOR))
        } else {
            let v = (state.viewport.zoom * PERCENT_FACTOR).clamp(EDGE_WHEEL_MIN_PCT, EDGE_WHEEL_MAX_PCT);
            ("grafia_ew_z", EDGE_WHEEL_MIN_PCT..=EDGE_WHEEL_MAX_PCT, v)
        };
        self.edge_wheel_value_pct = cur;
        let icon = resolve_active_wheel_icon(state, is_ic, (kin, krn, ctx));
        let builder = UiBuilder::new().id_salt("grafia_smart_wheel_scope").max_rect(wheel_r);
        let w_resp = ui.scope_builder(builder, |w_ui| {
            let r = MacEdgeThumbwheel::new(id, &mut self.edge_wheel_value_pct, range)
                .wide_drum_width(wheel_r.width())
                .scale(scale)
                .theme(mac_theme)
                .show(w_ui);
            let med_sz = Vec2::new((r.response.rect.height() * 1.15).max(18.0), (r.response.rect.height() - 4.0).max(14.0));
            let med_r = Rect::from_center_size(r.response.rect.center(), med_sz);
            let cr = egui::CornerRadius::same((med_sz.y * 0.45) as u8);
            w_ui.painter().rect(med_r, cr, mac_theme.control_background(), egui::Stroke::new(1.0, r.effective_glass_accent), egui::StrokeKind::Inside);
            w_ui.painter().text(med_r.center(), egui::Align2::CENTER_CENTER, icon, egui::FontId::proportional((med_sz.y * 0.62).clamp(10.0, 15.0)), mac_theme.label());
            r
        }).inner;
        let tel = state.controller_telemetry();
        let tip = if kin { format!("Horizontaal Context-Wiel — Kinetische Fase t: {:.0}%", self.edge_wheel_value_pct) }
            else if krn { "Horizontaal Context-Wiel — Live Kerning & Spatiëring (±1 / ±10 UPM)".to_owned() }
            else if ctx { format!("Horizontaal Context-Wiel — {}: {}", tel.primary_rotary.parameter_name, tel.primary_rotary.formatted_value) }
            else { format!("Horizontaal Context-Wiel — Canvas Zoom: {:.0}%", self.edge_wheel_value_pct) };
        w_resp.response.on_hover_text(tip);
        if w_resp.value_delta.abs() > f32::EPSILON || w_resp.delta_ticks.abs() > f32::EPSILON {
            Self::apply_smart_context_wheel_delta(state, canvas_rect, self.edge_wheel_value_pct, w_resp.delta_ticks, (kin, krn, ctx));
        }
    }

    #[rustfmt::skip]
    fn apply_smart_context_wheel_delta(
        state: &mut GrafiaCanvasState,
        canvas_rect: Rect,
        wheel_pct: f32,
        delta_ticks: f32,
        (kin, krn, ctx): (bool, bool, bool),
    ) {
        let step = if delta_ticks.abs() > f32::EPSILON { f64::from(delta_ticks) } else { 1.0 };
        if kin {
            let norm = f64::from((wheel_pct / PERCENT_FACTOR).clamp(0.0, 1.0));
            if matches!(state.icon_studio.kinetic.playback_mode, KineticPlaybackMode::VariableValue(_)) {
                state.set_kinetic_playback_mode(KineticPlaybackMode::VariableValue(norm));
            } else {
                state.icon_studio.kinetic.phase_t = norm;
                state.bump_content_generation();
            }
        } else if krn {
            state.step_rotary_encoder(CanvasRotaryChannel::KerningOrSpacing, step);
        } else if ctx {
            state.step_rotary_encoder(CanvasRotaryChannel::ContextPrimary, step);
        } else {
            let old_z = state.viewport.zoom.max(MIN_VIEWPORT_ZOOM);
            let new_z = (wheel_pct / PERCENT_FACTOR).clamp(MIN_VIEWPORT_ZOOM, MAX_VIEWPORT_ZOOM);
            let c = canvas_rect.center();
            state.viewport.zoom_at_cursor(grafia::ScreenPoint::new(c.x, c.y), new_z / old_z);
        }
    }
}
