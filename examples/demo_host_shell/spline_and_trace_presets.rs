//! Interactive Spline (`HobbyStar`), `CanvasSmartHandle`, and `CanvasBackgroundImage` Auto-Trace Presets.

use super::toolbar_specs::{
    CANVAS_RESIZE_EPSILON_SQ_DP, MIN_FIT_CANVAS_DIM_DP, MOBILE_RESPONSIVE_BREAKPOINT_WIDTH_DP,
    TOUCH_LOUPE_FINGER_OFFSET_DP,
};
use super::MockFontHost;
use egui::ScrollArea;
use grafia::adapters::GrafiaCanvasState;
use grafia::core::{
    Affine2D, AutoTraceConfig, CanvasBackgroundImage, CanvasComponent, CanvasSmartHandle,
    ComponentId, FontPoint,
};
use grafia::tokens::{GlyphArchetype, GrafiaTheme};

const SMART_HANDLE_ANCHOR_X_UPM: f64 = 120.0;
const SMART_HANDLE_ANCHOR_Y_UPM: f64 = 350.0;
const SMART_HANDLE_MIN_VAL: f64 = 40.0;
const SMART_HANDLE_DEFAULT_VAL: f64 = 90.0;
const SMART_HANDLE_MAX_VAL: f64 = 160.0;
const COMP_TRANSLATE_X_UPM: f64 = 300.0;
const COMP_TRANSLATE_Y_UPM: f64 = 560.0;
const TRACE_IMG_DIM_PX: u32 = 48;
const TRACE_RING_MIN_PX: u32 = 10;
const TRACE_RING_MAX_PX: u32 = 38;
const TRACE_IMG_SCALE_UPM: f64 = 12.0;
const TRACE_IMG_ALPHA: f32 = 0.55;
const TRACE_FG_R: u8 = 20;
const TRACE_FG_G: u8 = 25;
const TRACE_FG_B: u8 = 35;
const TRACE_FG_A: u8 = 255;

impl MockFontHost {
    pub fn apply_hobby_and_smart_handles_preset(&mut self, state: &mut GrafiaCanvasState) {
        if state.active_archetype != GlyphArchetype::O {
            state.load_glyph_archetype(GlyphArchetype::O);
            self.fit_canvas_viewport(state);
            state.glyph.name = "O".into();
        }
        let converted = if state.convert_selected_cubic_nodes_to_hobby() {
            "HobbyStar"
        } else {
            let _ = state.convert_selected_hobby_nodes_to_cubic();
            "Cubic"
        };
        if state.components.is_empty() {
            let comp_contours = self
                .glyph_library
                .get("acutecomb")
                .map_or_else(Vec::new, |g| g.contours.clone());
            state.set_components(vec![CanvasComponent {
                id: ComponentId(1),
                component_name: "acutecomb".into(),
                contours: comp_contours,
                transform: Affine2D::translation(COMP_TRANSLATE_X_UPM, COMP_TRANSLATE_Y_UPM),
                locked: false,
                automatic_alignment: false,
                selected: true,
            }]);
        }
        state.set_smart_handles(vec![CanvasSmartHandle {
            component_id: ComponentId(1),
            axis_id: "wght".into(),
            axis_name: "StemWeight".into(),
            anchor_upm: FontPoint::new(SMART_HANDLE_ANCHOR_X_UPM, SMART_HANDLE_ANCHOR_Y_UPM),
            direction_unit: (1.0, 0.0),
            scale_upm_per_unit: 2.0,
            min_val: SMART_HANDLE_MIN_VAL,
            default_val: SMART_HANDLE_DEFAULT_VAL,
            max_val: SMART_HANDLE_MAX_VAL,
            current_val: SMART_HANDLE_DEFAULT_VAL,
        }]);
        state.show_extra_nodes = true;
        self.push_log(format!(
            "Preset: Converted /O to {converted} + active SmartHandle slider"
        ));
    }

    pub fn apply_auto_trace_bg_image_preset(&mut self, state: &mut GrafiaCanvasState) {
        if state.background_image.is_none() {
            let mut pixels = vec![0_u8; (TRACE_IMG_DIM_PX * TRACE_IMG_DIM_PX * 4) as usize];
            for y in TRACE_RING_MIN_PX..TRACE_RING_MAX_PX {
                for x in TRACE_RING_MIN_PX..TRACE_RING_MAX_PX {
                    let i = ((y * TRACE_IMG_DIM_PX + x) * 4) as usize;
                    pixels[i] = TRACE_FG_R;
                    pixels[i + 1] = TRACE_FG_G;
                    pixels[i + 2] = TRACE_FG_B;
                    pixels[i + 3] = TRACE_FG_A;
                }
            }
            let mut bg = CanvasBackgroundImage::new(pixels, TRACE_IMG_DIM_PX, TRACE_IMG_DIM_PX);
            bg.scale_x = TRACE_IMG_SCALE_UPM;
            bg.scale_y = TRACE_IMG_SCALE_UPM;
            bg.alpha = TRACE_IMG_ALPHA;
            bg.selected = true;
            state.set_background_image(Some(bg));
            self.push_log("Preset: Placed Background Image (Click again to Auto-Trace)");
        } else {
            let traced = state.trace_active_background_image(&AutoTraceConfig::default());
            state.set_background_image(None);
            self.push_log(format!(
                "Preset: Auto-Traced bitmap into Foreground Contours (changed={traced})"
            ));
        }
    }

    pub fn update_responsive_layout(
        &mut self,
        screen_w: f32,
        show_preview_panel: &mut bool,
        state: &mut GrafiaCanvasState,
    ) {
        if screen_w > 0.0
            && screen_w < MOBILE_RESPONSIVE_BREAKPOINT_WIDTH_DP
            && !self.responsive_layout_initialized
        {
            self.responsive_layout_initialized = true;
            self.show_left_sidebar = false;
            self.show_right_inspector = false;
            self.show_coach_banner = false;
            *show_preview_panel = false;
            self.set_mobile_thumb_hud_enabled(true, state);
        }
    }

    pub fn record_and_fit_canvas_size(&mut self, avail: egui::Vec2, state: &mut GrafiaCanvasState) {
        if avail.x > MIN_FIT_CANVAS_DIM_DP && avail.y > MIN_FIT_CANVAS_DIM_DP {
            let drag_busy = state.is_interactive_drag_active()
                || state.drag_transients.touch_state.primary_press_active
                || !state.drag_transients.touch_state.contacts.is_empty();
            if drag_busy && self.initial_canvas_fitted {
                return;
            }
            let resized = self
                .last_canvas_size
                .is_some_and(|p| (p - avail).length_sq() > CANVAS_RESIZE_EPSILON_SQ_DP);
            self.last_canvas_size = Some(avail);
            if !self.initial_canvas_fitted || resized {
                state.fit_viewport_to_glyph(avail.x, avail.y);
                state.viewport.pan_offset.x.0 += self.last_canvas_origin.x;
                state.viewport.pan_offset.y.0 += self.last_canvas_origin.y;
                self.initial_canvas_fitted = true;
            }
        }
    }

    pub fn render_preset_bar(
        &mut self,
        ui: &mut egui::Ui,
        state: &mut GrafiaCanvasState,
        theme: &mut GrafiaTheme,
        _show_preview_panel: &mut bool,
    ) {
        if self.studio_mode == super::StudioWorkspaceMode::LetterStudio {
            ScrollArea::horizontal()
                .id_salt("preset_and_harmony_bar_scroll")
                .show(ui, |ui| {
                    ui.horizontal(|ui| {
                        self.render_chapter_and_preset_buttons(ui, state, theme);
                        ui.separator();
                        self.render_active_letter_tab_context_controls(ui, state);
                    });
                });
        } else {
            self.render_icon_studio_and_kinetic_bar(ui, state);
        }
    }

    fn render_active_letter_tab_context_controls(
        &self,
        ui: &mut egui::Ui,
        state: &mut GrafiaCanvasState,
    ) {
        use super::ShowcaseChapter as Ch;
        match self.active_letter_tab() {
            Ch::BasisTekenen => Self::render_tab_1_tekenen_controls(ui, state),
            Ch::CurvenEnAudit => Self::render_tablet_and_harmony_bar(ui, state),
            Ch::KalligrafieEnStems => Self::render_tab_3_stems_controls(ui, state),
            _ => Self::render_tab_4_tekst_masters_controls(ui, state),
        }
    }

    fn render_tab_1_tekenen_controls(ui: &mut egui::Ui, state: &mut GrafiaCanvasState) {
        if ui.button("Overlap Verwijderen").clicked() {
            state.remove_overlap();
        }
        if ui.button("Extrema Toevoegen").clicked() {
            state.add_extremes_action();
        }
        let loupe_on = state.preferences.show_touch_precision_loupe;
        if ui.selectable_label(loupe_on, "Precisie-Loep").clicked() {
            let mut prefs = state.preferences.clone();
            prefs.show_touch_precision_loupe = !loupe_on;
            prefs.touch_finger_offset_dp = if !loupe_on {
                TOUCH_LOUPE_FINGER_OFFSET_DP
            } else {
                0.0
            };
            state.apply_preferences(&prefs);
        }
    }

    fn render_tab_3_stems_controls(ui: &mut egui::Ui, state: &mut GrafiaCanvasState) {
        use grafia::adapters::CanvasTool as T;
        for (tool, lbl) in [
            (T::Stem, "Tool: Stamdikte"),
            (T::Trowel, "Tool: Troffel"),
            (T::Cleaner, "Tool: Gum-Borstel"),
        ] {
            if ui
                .selectable_label(state.active_tool == tool, lbl)
                .clicked()
            {
                state.activate_tool(tool);
            }
        }
        let ch = &state.curve_harmony;
        let (sg, rb, vc) = (
            ch.show_stem_gauge,
            ch.show_measurement_rainbow,
            ch.show_visual_centroid,
        );
        if ui.selectable_label(sg, "Diktemeter").clicked() {
            state.toggle_show_stem_gauge();
        }
        if ui.selectable_label(rb, "Regenboog").clicked() {
            state.toggle_show_measurement_rainbow();
        }
        if ui.selectable_label(vc, "Zwaartepunt ⊕").clicked() {
            state.toggle_show_visual_centroid();
        }
        if !state.smart_corners().is_empty() && ui.button("Hoeken Vastbakken").clicked() {
            state.bake_smart_corners();
        }
    }

    fn render_tab_4_tekst_masters_controls(ui: &mut egui::Ui, state: &mut GrafiaCanvasState) {
        use grafia::adapters::CanvasTool as T;
        for (tool, lbl) in [
            (T::Text, "Tool: Tekst (T)"),
            (T::CurvedTrajectory, "Tool: Master-Baan"),
            (T::TrueTypeInstructor, "Tool: Hinting (I)"),
            (T::Hoi, "Tool: HOI (H)"),
        ] {
            if ui
                .selectable_label(state.active_tool == tool, lbl)
                .clicked()
            {
                state.activate_tool(tool);
            }
        }
        if state.show_master_compatibility && ui.button("Match Masters").clicked() {
            let _ = state.auto_match_master_compatibility(1);
        }
    }

    pub fn render_tablet_and_harmony_bar(ui: &mut egui::Ui, state: &mut GrafiaCanvasState) {
        if ui
            .selectable_label(state.show_tunni_lines, "Tunni-Lijnen")
            .clicked()
        {
            state.toggle_show_tunni_lines();
        }
        if ui
            .selectable_label(state.curve_harmony.show_curvature_comb, "Krommingskam")
            .clicked()
        {
            state.toggle_show_curvature_comb();
        }
        if ui
            .selectable_label(state.curve_harmony.show_outline_audit, "Fout-Audit")
            .clicked()
        {
            state.curve_harmony.show_outline_audit = !state.curve_harmony.show_outline_audit;
        }
        if ui.button("G2 Glad").clicked() {
            state.harmonize_selected_nodes();
        }
        if ui.button("G3 Clothoïde").clicked() {
            state.harmonize_selected_nodes_g3();
        }
        if ui
            .selectable_label(state.power_nudge_enabled, "Power Nudge")
            .clicked()
        {
            state.toggle_power_nudge();
        }
    }

    pub fn render_status_bar_contents(ui: &mut egui::Ui, state: &GrafiaCanvasState) {
        ui.horizontal(|ui| {
            let (nl_name, nl_how) = super::tool_dutch_guide(state.active_tool);
            ui.label(format!("{nl_name} — {nl_how}"));
            ui.add(egui::Separator::default().vertical());
            let total_nodes: usize = state.contours.iter().map(Vec::len).sum();
            ui.label(format!(
                "Contours: {} | Nodes: {total_nodes} (Selected: {}) | Anchors: {} | Components: {}",
                state.contours.len(),
                state.selected_nodes.len(),
                state.anchors.len(),
                state.components.len()
            ));
        });
    }
}
