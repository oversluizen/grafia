//! Host-Port ("Stekkers") request, spacing, and kerning event pump for `MockFontHost`.

use super::icon_studio_showcase::{demo_kinetic_effect_by_idx, DEMO_KINETIC_DEFAULT_PHASE};
use super::sonoma_top_bar::{section_header_text, StudioWorkspaceMode};
use super::toolbar_specs::{
    PREVIEW_HEADER_CHROME_DP, PREVIEW_HEIGHT_L_DP, PREVIEW_HEIGHT_MATCH_TOL_DP,
    PREVIEW_HEIGHT_M_DP, PREVIEW_HEIGHT_S_DP, PREVIEW_STRIP_MAX_HEIGHT_DP,
    PREVIEW_STRIP_MIN_HEIGHT_DP,
};
use super::MockFontHost;
use egui::{Image, ScrollArea, Separator, Slider, Ui, Vec2};
use egui_macview::MacTheme;
use grafia::adapters::GrafiaCanvasState;
use grafia::core::{
    CanvasHostRequest, KineticPlaybackMode, TextStreamEditKind, TextStreamEditRequest,
};
use grafia::tokens::{PREVIEW_BLUR_MAX_RADIUS_PX, PREVIEW_BLUR_MIN_RADIUS_PX};

impl MockFontHost {
    pub fn poll_host_ports(&mut self, state: &mut GrafiaCanvasState) {
        for req in state.drain_host_requests() {
            self.handle_canvas_host_request(state, req);
        }
        for sp in state.drain_spacing_deltas() {
            self.glyph_library
                .insert(state.glyph.name.clone(), state.glyph.clone());
            self.push_log(format!(
                "HostPort: SpacingDelta(slot={}, dLSB={:+}, dRSB={:+})",
                sp.slot_index, sp.lsb_delta_upm, sp.rsb_delta_upm
            ));
        }
        for kd in state.drain_kerning_deltas() {
            self.kerning_pairs.insert(
                (kd.left_glyph.clone(), kd.right_glyph.clone()),
                kd.new_kern_upm,
            );
            self.push_log(format!(
                "HostPort: KerningDelta({},{}: {:+} -> {})",
                kd.left_glyph, kd.right_glyph, kd.delta_upm, kd.new_kern_upm
            ));
        }
        for hap in state.drain_haptic_events() {
            self.push_log(format!("HapticPort: {hap:?}"));
        }
    }

    fn handle_canvas_host_request(
        &mut self,
        state: &mut GrafiaCanvasState,
        req: CanvasHostRequest,
    ) {
        match req {
            CanvasHostRequest::ResetDefaultAnchors { glyph_name } => {
                let anchors = self.default_anchors_for(&glyph_name, state.glyph.width);
                let marks = self.default_ghost_marks();
                let (na, nm) = (anchors.len(), marks.len());
                state.set_anchors_with_undo(anchors);
                state.set_ghost_mark_paths(marks);
                state.show_mark_cloud = true;
                self.push_log(format!(
                    "HostPort: ResetDefaultAnchors({glyph_name}) -> {na} anchors + {nm} ghost marks injected"
                ));
            }
            CanvasHostRequest::EditTextStream(edit_req) => {
                self.handle_edit_text_stream_request(state, edit_req);
            }
            CanvasHostRequest::OpenComponentSource { component_name, .. } => {
                if let Some(src) = self.glyph_library.get(&component_name).cloned() {
                    let before = state.snapshot();
                    state.glyph = src.clone();
                    state.contours = src.contours.iter().map(|c| c.nodes.clone()).collect();
                    state.components.clear();
                    state.selected_nodes.clear();
                    state.set_anchors(self.default_anchors_for(&component_name, src.width));
                    state.sync_paths_from_contours();
                    state.record_mutation("Open Component Source", before);
                    state.assert_invariants();
                }
                self.push_log(format!("HostPort: OpenComponentSource({component_name})"));
            }
            CanvasHostRequest::UpdateSmartComponentAxis {
                axis_id, new_value, ..
            } => {
                self.push_log(format!("HostPort: SmartAxis({axis_id}={new_value:.1})"));
            }
            CanvasHostRequest::SelectOrMutateExtraNode { key } => {
                self.push_log(format!("HostPort: ExtraNode({key:?})"));
            }
            CanvasHostRequest::MutateColorSublayer {
                sublayer_index,
                mutation,
            } => {
                self.push_log(format!(
                    "HostPort: ColorSublayer({sublayer_index}, {mutation:?})"
                ));
            }
            CanvasHostRequest::MutatePostScriptHint(m) => {
                self.push_log(format!("HostPort: PostScriptHint({m:?})"));
            }
            CanvasHostRequest::MutateTrueTypeInstruction(m) => {
                self.push_log(format!("HostPort: TrueTypeInstruction({m:?})"));
            }
            CanvasHostRequest::MutateHoiLayerNode {
                layer_index,
                contour_index,
                node_id,
                new_pos,
            } => {
                self.push_log(format!(
                    "HostPort: HoiLayerNode(L{layer_index}:C{contour_index}:{node_id:?} -> {:.1},{:.1})",
                    new_pos.x.0, new_pos.y.0
                ));
            }
            CanvasHostRequest::PropagateStemDelta {
                orientation,
                old_width_upm,
                new_width_upm,
                angle_delta_deg,
            } => {
                self.push_log(format!(
                    "HostPort: PropagateStemDelta({orientation:?}: {old_width_upm:.1} -> {new_width_upm:.1}, dAngle={angle_delta_deg:+.1}°)"
                ));
            }
        }
    }

    fn handle_edit_text_stream_request(
        &mut self,
        state: &mut GrafiaCanvasState,
        edit_req: TextStreamEditRequest,
    ) {
        let mut chars: Vec<char> = if !state.text_stream.slots.is_empty()
            && state.text_stream.slots.len() != self.text_buffer.chars().count()
        {
            state
                .text_stream
                .slots
                .iter()
                .map(|s| s.glyph.name.chars().next().unwrap_or('?'))
                .collect()
        } else {
            self.text_buffer.chars().collect()
        };
        let mut caret = edit_req.caret_index.min(chars.len());
        if let Some((s0, s1)) = edit_req.selection_range {
            let (lo, hi) = (s0.min(s1).min(chars.len()), s0.max(s1).min(chars.len()));
            if lo < hi {
                chars.drain(lo..hi);
                caret = lo;
            }
        }
        match &edit_req.edit {
            TextStreamEditKind::InsertText(t) => {
                for ch in t.chars() {
                    chars.insert(caret, ch);
                    caret += 1;
                }
            }
            TextStreamEditKind::Backspace if edit_req.selection_range.is_none() && caret > 0 => {
                caret -= 1;
                chars.remove(caret);
            }
            TextStreamEditKind::DeleteForward
                if edit_req.selection_range.is_none() && caret < chars.len() =>
            {
                chars.remove(caret);
            }
            _ => {}
        }
        self.text_buffer = chars.iter().collect();
        let slots = self.build_slots_for_text(&self.text_buffer);
        let active = caret.saturating_sub(1).min(slots.len().saturating_sub(1));
        state.set_text_stream_slots_with_caret(slots, active, caret);
        self.push_log(format!(
            "HostPort: EditTextStream({:?}) -> \"{}\"",
            edit_req.edit, self.text_buffer
        ));
    }

    pub(crate) fn render_kinetic_timeline_controls(
        &mut self,
        ui: &mut Ui,
        st: &mut GrafiaCanvasState,
    ) {
        let playing = st.icon_studio.kinetic.playing;
        let lbl = if playing { "⏸ Pause" } else { "▶ Play" };
        if ui.selectable_label(playing, lbl).clicked() {
            st.icon_studio.kinetic.playing = !playing;
            st.bump_content_generation();
        }
        let cur_pm = st.icon_studio.kinetic.playback_mode;
        let rep = KineticPlaybackMode::Repeating {
            repeat_delay_phase: 0.15,
        };
        for (lbl, mode) in [
            ("1× Veer", KineticPlaybackMode::OneShot),
            ("Herhaal", rep),
            ("Continu", KineticPlaybackMode::Continuous),
            ("Sensor %", KineticPlaybackMode::VariableValue(0.65)),
        ] {
            let on = std::mem::discriminant(&cur_pm) == std::mem::discriminant(&mode);
            if ui.selectable_label(on, lbl).clicked() {
                st.set_kinetic_playback_mode(mode);
            }
        }
        Self::render_kinetic_effect_and_phase_controls(ui, st);
    }

    fn render_kinetic_effect_and_phase_controls(ui: &mut Ui, st: &mut GrafiaCanvasState) {
        let names = "Pulse,Bounce,VarColor,Scale,DrawOn,DrawOff,Wiggle,Rotate,Breathe,MagicReplace";
        for (idx, lbl) in names.split(',').enumerate() {
            let eff = demo_kinetic_effect_by_idx(idx);
            let same = std::mem::discriminant(&st.icon_studio.kinetic.effect)
                == std::mem::discriminant(&eff);
            if ui.selectable_label(same, lbl).clicked() {
                st.icon_studio.kinetic.effect = eff;
                let t = st.icon_studio.kinetic.phase_t;
                st.icon_studio.kinetic.phase_t = t.max(DEMO_KINETIC_DEFAULT_PHASE);
                st.bump_content_generation();
            }
        }
        if let KineticPlaybackMode::VariableValue(mut val) = st.icon_studio.kinetic.playback_mode {
            if ui
                .add(Slider::new(&mut val, 0.0..=1.0).text("Sensor"))
                .changed()
            {
                st.set_kinetic_playback_mode(KineticPlaybackMode::VariableValue(val));
            }
        } else if ui
            .add(Slider::new(&mut st.icon_studio.kinetic.phase_t, 0.0..=1.0).text("Fase t"))
            .changed()
        {
            st.icon_studio.kinetic.playing = false;
            st.bump_content_generation();
        }
    }

    pub(crate) fn render_preview_size_presets(&mut self, ui: &mut Ui) {
        for (lbl, h_dp) in [
            ("S", PREVIEW_HEIGHT_S_DP),
            ("M", PREVIEW_HEIGHT_M_DP),
            ("L", PREVIEW_HEIGHT_L_DP),
        ] {
            let is_cur = (self.preview_panel_height_dp - h_dp).abs() < PREVIEW_HEIGHT_MATCH_TOL_DP;
            if ui.selectable_label(is_cur, lbl).clicked() {
                self.preview_panel_height_dp = h_dp;
            }
        }
    }

    pub(crate) fn render_waterfall_and_kinetic_card(
        &mut self,
        ui: &mut Ui,
        state: &mut GrafiaCanvasState,
        tm: &MacTheme,
        target_w_px: u32,
    ) {
        ScrollArea::horizontal()
            .id_salt("preview_strip_ctrl_scroll")
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    let is_ic = self.studio_mode == StudioWorkspaceMode::IconStudio;
                    let hdr = if is_ic {
                        "KINETISCHE TIJDLIJN & SPECIMEN"
                    } else {
                        "LIVE SPECIMEN WATERFALL"
                    };
                    ui.label(section_header_text(hdr, tm));
                    ui.add(Separator::default().vertical());
                    self.render_preview_size_presets(ui);
                    ui.add(Separator::default().vertical());
                    if is_ic {
                        self.render_kinetic_timeline_controls(ui, state);
                    } else {
                        let r = PREVIEW_BLUR_MIN_RADIUS_PX..=PREVIEW_BLUR_MAX_RADIUS_PX;
                        let _ = ui
                            .add(Slider::new(&mut self.preview_cfg.blur_radius_px, r).text("Blur"));
                        ui.checkbox(&mut self.preview_cfg.flipped, "Spiegel");
                        ui.checkbox(&mut self.preview_cfg.inverted, "Inverteer");
                    }
                });
            });
        self.render_waterfall_texture(ui, state, target_w_px);
    }

    pub(crate) fn render_waterfall_texture(
        &mut self,
        ui: &mut Ui,
        state: &GrafiaCanvasState,
        target_w_px: u32,
    ) {
        let avail_h = (self.preview_panel_height_dp - PREVIEW_HEADER_CHROME_DP)
            .clamp(PREVIEW_STRIP_MIN_HEIGHT_DP, PREVIEW_STRIP_MAX_HEIGHT_DP);
        self.preview_cfg.width_px = target_w_px;
        self.preview_cfg.height_px = avail_h.round() as u32;
        let (tex, _) = self
            .preview_cache
            .get_or_update_texture(ui.ctx(), state, &self.preview_cfg);
        let sz = Vec2::new(
            self.preview_cfg.width_px as f32,
            self.preview_cfg.height_px as f32,
        );
        ui.add(Image::from_texture((tex.id(), sz)));
    }
}
