//! Interactive Spline (`HobbyStar`), `CanvasSmartHandle`, and `CanvasBackgroundImage` Auto-Trace Presets.

use super::MockFontHost;
use grafia::adapters::GrafiaCanvasState;
use grafia::core::{
    Affine2D, AutoTraceConfig, CanvasBackgroundImage, CanvasComponent, CanvasSmartHandle,
    ComponentId, FontPoint,
};
use grafia::tokens::GlyphArchetype;

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

    pub fn render_tablet_and_harmony_bar(ui: &mut egui::Ui, state: &mut GrafiaCanvasState) {
        ui.horizontal_wrapped(|ui| {
            ui.strong("Tablet & Harmony:");
            let ts = &mut state.drag_transients.touch_state;
            if ui
                .selectable_label(ts.touch_shift_latched, "[Shift]")
                .clicked()
            {
                ts.touch_shift_latched = !ts.touch_shift_latched;
            }
            if ui
                .selectable_label(ts.touch_alt_latched, "[Alt/Opt]")
                .clicked()
            {
                ts.touch_alt_latched = !ts.touch_alt_latched;
            }
            if ui
                .selectable_label(ts.touch_cmd_latched, "[Cmd/Ctrl]")
                .clicked()
            {
                ts.touch_cmd_latched = !ts.touch_cmd_latched;
            }
            if ui
                .selectable_label(ts.touch_space_pan_latched, "[Pan (Space)]")
                .clicked()
            {
                ts.touch_space_pan_latched = !ts.touch_space_pan_latched;
            }
            let loupe_on = state.preferences.show_touch_precision_loupe;
            if ui.selectable_label(loupe_on, "[Touch Loupe]").clicked() {
                let mut prefs = state.preferences.clone();
                prefs.show_touch_precision_loupe = !loupe_on;
                prefs.touch_finger_offset_dp = if prefs.show_touch_precision_loupe {
                    44.0
                } else {
                    0.0
                };
                state.apply_preferences(&prefs);
            }
            ui.separator();
            if ui
                .selectable_label(state.show_tunni_lines, "[Tunni Lines]")
                .clicked()
            {
                state.toggle_show_tunni_lines();
            }
            if ui
                .selectable_label(state.curve_harmony.show_curvature_comb, "[Curvature Comb]")
                .clicked()
            {
                state.toggle_show_curvature_comb();
            }
            if ui
                .selectable_label(state.curve_harmony.show_outline_audit, "[Audit Red Arrows]")
                .clicked()
            {
                state.curve_harmony.show_outline_audit = !state.curve_harmony.show_outline_audit;
            }
            if ui.button("[Harmonize G2]").clicked() {
                state.harmonize_selected_nodes();
            }
            if ui.button("[Harmonize G3]").clicked() {
                state.harmonize_selected_nodes_g3();
            }
            if ui
                .selectable_label(state.power_nudge_enabled, "[Power Nudge (C)]")
                .clicked()
            {
                state.toggle_power_nudge();
            }
            if ui.button("[Auto-Match Master]").clicked() {
                let _ = state.auto_match_master_compatibility(1);
            }
        });
    }

    pub fn render_status_bar_contents(ui: &mut egui::Ui, state: &GrafiaCanvasState) {
        ui.horizontal(|ui| {
            let tool_str = super::toolbar_specs::TOOL_ICON_SPECS
                .iter()
                .find(|(t, _, _)| *t == state.active_tool)
                .map_or("", |(_, _, tip)| *tip);
            ui.label(tool_str);
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
