//! Slank Rechter Frosted Liquid Glass Icoon-Grid Palet (`4×` vierkante glazen icoon-tegels
//! per rij met Nederlandse S-Pen / muis-hover uitlegballonnetjes, `SPEC-018 §3.7`).

use super::sonoma_top_bar::{liquid_glass_capsule_frame, section_header_text, StudioSubWorkspace};
use super::toolbar_specs::{
    DEMO_GUIDE_X_UPM, DEMO_GUIDE_Y_UPM, DEMO_ROTATE_STEP_DEG, DEMO_SCALE_UP_FACTOR,
    DEMO_SLANT_STEP_DEG, GRID_SPACING_DP, METRICS_ORIGIN_SPECS, ORIGIN_ANCHOR_GRID_ROWS,
    SIDEBAR_TOP_SPACE_DP,
};
use super::{DemoPreset, MockFontHost};
use egui::{Button, Grid, ScrollArea, Separator, Ui};
use egui_macview::{MacSymbol, MacTheme};
use grafia::adapters::GrafiaCanvasState;
use grafia::core::{FontPoint, TransformGridAnchor, TransformOriginSpec};

const ICON_TILE_W_DP: f32 = 34.0;
const ICON_TILE_H_DP: f32 = 26.0;

#[rustfmt::skip]
pub(crate) fn icon_tile(ui: &mut Ui, icon: &str, tip: &str) -> bool {
    ui.add_sized([ICON_TILE_W_DP, ICON_TILE_H_DP], Button::new(icon)).on_hover_text(tip).clicked()
}

#[rustfmt::skip]
pub(crate) fn icon_tile_toggle(ui: &mut Ui, active: bool, icon: &str, tip: &str) -> bool {
    ui.add_sized([ICON_TILE_W_DP, ICON_TILE_H_DP], Button::selectable(active, icon)).on_hover_text(tip).clicked()
}

#[rustfmt::skip]
pub fn render_overlays_popover_contents(ui: &mut Ui, state: &mut GrafiaCanvasState, tm: &MacTheme) {
    ui.label(section_header_text("1. HULPLIJNEN & RASTER", tm));
    ui.horizontal_wrapped(|ui| {
        if icon_tile_toggle(ui, state.show_rulers, "\u{f7ca}", "20dp Linialen [⌘R]") { state.show_rulers = !state.show_rulers; }
        if icon_tile_toggle(ui, state.show_metrics, "\u{f52a}", "Typo-Metriek Lijnen") { state.show_metrics = !state.show_metrics; }
        if icon_tile_toggle(ui, state.show_guides, "\u{f424}", "Hulplijnen [⌘;]") { state.show_guides = !state.show_guides; }
        if icon_tile_toggle(ui, state.show_background, "\u{f80d}", "Achtergrondlaag [⌘B]") { state.show_background = !state.show_background; }
        if icon_tile_toggle(ui, state.show_selection_info_hud, "\u{f853}", "Zwevende Glyphs-4 Canvas Info-Box") { state.show_selection_info_hud = !state.show_selection_info_hud; }
    });
    ui.add(Separator::default().horizontal());
    ui.label(section_header_text("2. KROMMING, AUDIT & OPTICA", tm));
    MockFontHost::render_harmony_controls(ui, state);
}

impl MockFontHost {
    pub fn render_host_inspector(&mut self, ui: &mut Ui, state: &mut GrafiaCanvasState) {
        let tm = self.active_mac_theme();
        ScrollArea::vertical()
            .id_salt("right_frosted_icon_palette_scroll")
            .show(ui, |ui| {
                ui.add_space(SIDEBAR_TOP_SPACE_DP);
                liquid_glass_capsule_frame(&tm).show(ui, |ui| {
                    self.render_structured_info_card(ui, state, &tm);
                });
                ui.add_space(SIDEBAR_TOP_SPACE_DP);
                liquid_glass_capsule_frame(&tm).show(ui, |ui| {
                    Self::render_icon_transform_and_origin_card(ui, state, &tm);
                });
                ui.add_space(SIDEBAR_TOP_SPACE_DP);
                liquid_glass_capsule_frame(&tm).show(ui, |ui| {
                    self.render_workspace_specific_icon_grid(ui, state, &tm);
                });
            });
    }

    fn render_icon_transform_and_origin_card(
        ui: &mut Ui,
        state: &mut GrafiaCanvasState,
        tm: &MacTheme,
    ) {
        ui.label(section_header_text("OORSPRONG (3×3) & TRANSFORMATIE", tm));
        ui.horizontal(|ui| {
            Grid::new("insp_origin_anchor_grid")
                .spacing([GRID_SPACING_DP, GRID_SPACING_DP])
                .show(ui, |g_ui| {
                    for row in ORIGIN_ANCHOR_GRID_ROWS {
                        for (idx, icon, tip) in row {
                            let anchor = TransformGridAnchor::from_index(idx);
                            let active = state.transform_origin_mode
                                == TransformOriginSpec::BoundingBoxOrigin(anchor);
                            if g_ui
                                .selectable_label(active, icon)
                                .on_hover_text(tip)
                                .clicked()
                            {
                                state.set_transform_grid_anchor(anchor);
                            }
                        }
                        g_ui.end_row();
                    }
                });
            ui.add(Separator::default().vertical());
            ui.vertical(|v_ui| {
                for (m, lbl, tip) in METRICS_ORIGIN_SPECS {
                    let active =
                        state.transform_origin_mode == TransformOriginSpec::MetricsOrigin(m);
                    if v_ui
                        .selectable_label(active, lbl)
                        .on_hover_text(tip)
                        .clicked()
                    {
                        state.set_metrics_origin(m);
                    }
                }
            });
        });
        Self::render_transform_8_tiles(ui, state);
    }

    fn render_transform_8_tiles(ui: &mut Ui, state: &mut GrafiaCanvasState) {
        #[rustfmt::skip]
        const TILES: [(&str, &str); 8] = [
            ("\u{f7c5}", "Roteer −15° CCW [⌘⇧L] — Draai selectie rond het 3×3 oorsprongspunt"),
            ("\u{f7c7}", "Roteer +15° CW [⌘R] — Draai selectie +15° rond het 3×3 oorsprongspunt"),
            ("\u{f500}", "Spiegel Horizontaal [⌘⇧H] — Spiegel over de verticale as"),
            ("\u{f52a}", "Spiegel Verticaal [⌘⇧V] — Spiegel over de horizontale as"),
            ("\u{f386}", "Proportioneel Schalen 110% [S] — Vergroot selectie met 10%"),
            ("\u{f7da}", "Optisch Cursiveren +12° [⌘⇧I] — Schuintrekken met extrema-behoud"),
            ("\u{f80c}", "Overlap Verwijderen [⌘⇧O] — Smelt overlappende contouren samen"),
            ("\u{f48a}", "Extrema Toevoegen [⌘⇧E] — Plaats ankerpunten op H/V uitersten"),
        ];
        Grid::new("insp_transform_icon_grid_4col")
            .num_columns(4)
            .spacing([GRID_SPACING_DP * 2.0, GRID_SPACING_DP * 2.0])
            .show(ui, |g_ui| {
                for (i, (ic, tip)) in TILES.into_iter().enumerate() {
                    if icon_tile(g_ui, ic, tip) {
                        let _ = match i {
                            0 => state.rotate_by_degrees(-DEMO_ROTATE_STEP_DEG),
                            1 => state.rotate_by_degrees(DEMO_ROTATE_STEP_DEG),
                            2 => state.flip_horizontal(),
                            3 => state.flip_vertical(),
                            4 => state.scale_by_factor(DEMO_SCALE_UP_FACTOR, DEMO_SCALE_UP_FACTOR),
                            5 => state.slant_by_degrees(DEMO_SLANT_STEP_DEG, true),
                            6 => state.remove_overlap(),
                            _ => state.add_extremes_action(),
                        };
                    }
                    if i % 4 == 3 {
                        g_ui.end_row();
                    }
                }
            });
    }

    fn render_workspace_specific_icon_grid(
        &mut self,
        ui: &mut Ui,
        state: &mut GrafiaCanvasState,
        tm: &MacTheme,
    ) {
        let hdr = match self.active_workspace {
            StudioSubWorkspace::LetterContourAndSpacing => "1. PAD-CHIRURGIE, ANKERS & COMPONENTEN",
            StudioSubWorkspace::LetterCagdAndStems => "2. CAGD, TUNNI, KAM & STAMDIKTE",
            StudioSubWorkspace::LetterMastersAndHinting => "3. MASTERS, HINTING & AUTO-TRACE",
            StudioSubWorkspace::IconVectorAndGlass => "4. 800×800 KEYLINES, VLAKKEN & GLAS",
            StudioSubWorkspace::IconKineticAndTouch => "5. KINETISCHE SF-SYMBOLEN & DUIMWIEL",
        };
        ui.label(section_header_text(hdr, tm));
        match self.active_workspace {
            StudioSubWorkspace::LetterContourAndSpacing => {
                Self::render_ws1_contour_icon_grid(ui, state)
            }
            StudioSubWorkspace::LetterCagdAndStems => Self::render_harmony_controls(ui, state),
            StudioSubWorkspace::LetterMastersAndHinting => {
                self.render_ws3_masters_icon_grid(ui, state)
            }
            StudioSubWorkspace::IconVectorAndGlass | StudioSubWorkspace::IconKineticAndTouch => {
                self.render_icon_studio_and_kinetic_bar(ui, state);
            }
        }
    }

    fn render_ws1_contour_icon_grid(ui: &mut Ui, state: &mut GrafiaCanvasState) {
        #[rustfmt::skip]
        const WS1_TILES: [(&str, &str); 8] = [
            ("\u{f6f4}", "Corrigeer Padrichting [⌘⇧R] — Buitenrand CCW, binnen-counter CW"),
            ("\u{f4e3}", "Keer Padrichting Om [⌃⌥⌘R] — Draai wikkelrichting (CW ↔ CCW) om"),
            ("\u{f53d}", "Maak Knoop Startpunt — Stel geselecteerde knoop in als index 0"),
            ("\u{f892}", "Ruim Paden Op [⌘⌥⇧T] — Verwijder dubbele knopen en trek handles strak"),
            ("\u{f899}", "Buigpunten Toevoegen — Splits kubische S-bochten op inflectiepunt κ=0"),
            ("\u{f80a}", "Lijn Selectie Uit [⌘⇧A] — Lijn geselecteerde knopen H/V uit"),
            ("\u{f6e5}", "Verbind Open Eindpunten [⌘J] — Sluit twee open eindknopen"),
            ("\u{f77e}", "Herstel Standaard Ankers [⌘U] — Injecteer top/bottom/ogonek + wolk"),
        ];
        let g_pt = FontPoint::new(DEMO_GUIDE_X_UPM, DEMO_GUIDE_Y_UPM);
        Grid::new("ws1_path_ops_icon_grid_4col")
            .num_columns(4)
            .spacing([GRID_SPACING_DP * 2.0, GRID_SPACING_DP * 2.0])
            .show(ui, |g_ui| {
                for (i, (ic, tip)) in WS1_TILES.into_iter().enumerate() {
                    if icon_tile(g_ui, ic, tip) {
                        if i == 7 {
                            state.reset_default_anchors();
                        } else {
                            let _ = match i {
                                0 => state.correct_path_direction(),
                                1 => state.reverse_selected_or_all_contours(),
                                2 => state.make_node_first(),
                                3 => state.tidy_up_paths(),
                                4 => state.add_inflection_points(),
                                5 => state.align_selection(),
                                _ => state.connect_selected_open_endpoints(),
                            };
                        }
                    }
                    if i % 4 == 3 {
                        g_ui.end_row();
                    }
                }
                if icon_tile(
                    g_ui,
                    MacSymbol::Plus.as_str(),
                    "Lokale Hulplijn Toevoegen [⌘G]",
                ) {
                    state.add_guide_from_selection(false, g_pt);
                }
                if icon_tile_toggle(
                    g_ui,
                    state.show_guides,
                    "\u{f424}",
                    "Toon / Verberg Hulplijnen [⌘;]",
                ) {
                    state.show_guides = !state.show_guides;
                }
                if icon_tile_toggle(
                    g_ui,
                    state.lock_guides,
                    "\u{f4c8}",
                    "Vergrendel Hulplijnen [⌘⌥;]",
                ) {
                    state.lock_guides = !state.lock_guides;
                }
                if icon_tile_toggle(
                    g_ui,
                    state.show_background,
                    "\u{f80d}",
                    "Referentie-Achtergrondlaag [⌘B]",
                ) {
                    if state.background_contours.is_empty() {
                        state.copy_selection_to_background();
                    }
                    state.show_background = !state.show_background;
                }
                g_ui.end_row();
            });
    }

    fn render_ws3_masters_icon_grid(&mut self, ui: &mut Ui, state: &mut GrafiaCanvasState) {
        #[rustfmt::skip]
        const WS3_TILES: [(&str, &str, Option<DemoPreset>); 8] = [
            ("\u{f6e5}", "Multi-Master Compatibiliteitsstralen [⌘⌥N]", Some(DemoPreset::MultiMasterCompatAndAutoMatch)),
            ("\u{f6f4}", "Auto-Match Master Volgorde & Startknopen", None),
            ("\u{f899}", "Hogere-Orde Interpolatie (HOI) Diamanten [H]", Some(DemoPreset::HigherOrderInterpolationHoi)),
            ("\u{f7c7}", "Gebogen Master-Trajecten voor Rotatie", Some(DemoPreset::CurvedMasterTrajectories)),
            ("\u{f48a}", "TrueType Visual Instructor & 16 PPEM Raster [I]", Some(DemoPreset::TrueTypeVisualInstructor16Ppem)),
            ("\u{f500}", "PostScript Stam- & Ghost-Hints", Some(DemoPreset::PostScriptStemAndGhostHints)),
            ("\u{f804}", "Scan-Onderlegger & Auto-Trace naar Bézier", None),
            ("\u{f80c}", "Hoeken Vastbakken (Bake Smart Corners)", None),
        ];
        let mut dummy_theme = state.theme.clone();
        Grid::new("ws3_masters_icon_grid_4col")
            .num_columns(4)
            .spacing([GRID_SPACING_DP * 2.0, GRID_SPACING_DP * 2.0])
            .show(ui, |g_ui| {
                for (i, (ic, tip, preset)) in WS3_TILES.into_iter().enumerate() {
                    if icon_tile(g_ui, ic, tip) {
                        if let Some(p) = preset {
                            self.apply_preset(p, state, &mut dummy_theme);
                        } else if i == 1 {
                            let _ = state.auto_match_master_compatibility(1);
                        } else if i == 6 {
                            self.apply_auto_trace_bg_image_preset(state);
                        } else {
                            state.bake_smart_corners();
                        }
                    }
                    if i % 4 == 3 {
                        g_ui.end_row();
                    }
                }
            });
    }

    #[rustfmt::skip]
    pub(crate) fn render_harmony_controls(ui: &mut Ui, state: &mut GrafiaCanvasState) {
        Grid::new("ws2_cagd_icon_grid_4col").num_columns(4).spacing([GRID_SPACING_DP * 2.0, GRID_SPACING_DP * 2.0]).show(ui, |g_ui| {
            if icon_tile(g_ui, "G²", "G² Krommingscontinuïteit (Genius ★) [⌥⌘H]") { state.harmonize_selected_nodes(); }
            if icon_tile(g_ui, "G³", "G³ Clothoïde / Euler-Spiraal (❖) [⌃⌥⌘H]") { state.harmonize_selected_nodes_g3(); }
            if icon_tile_toggle(g_ui, state.show_tunni_lines, "\u{f6e5}", "Blauwe Tunni-Hefboomlijnen & Ratio-Badges") { state.toggle_show_tunni_lines(); }
            if icon_tile_toggle(g_ui, state.curve_harmony.show_curvature_comb, "\u{f899}", "Krommingskam (Curvature Comb) [⌘⌥C]") { state.toggle_show_curvature_comb(); }
            g_ui.end_row();
            if icon_tile_toggle(g_ui, state.curve_harmony.show_outline_audit, "\u{f48a}", "Rode-Pijlen Contour-Audit [⌘⇧D]") { state.curve_harmony.show_outline_audit = !state.curve_harmony.show_outline_audit; }
            if icon_tile_toggle(g_ui, state.power_nudge_enabled, "\u{f386}", "Power Nudge & Servo-Handles [⌃⌘]") { state.toggle_power_nudge(); }
            let sym_on = state.curve_harmony.symmetry_overlay_mode != grafia::SymmetryOverlayMode::Off;
            if icon_tile_toggle(g_ui, sym_on, "\u{f500}", "Spiegel-As Symmetrie-Overlay") {
                state.set_symmetry_overlay_mode(if sym_on { grafia::SymmetryOverlayMode::Off } else { grafia::SymmetryOverlayMode::VerticalAxis });
            }
            if icon_tile_toggle(g_ui, state.curve_harmony.show_measurement_rainbow, "\u{f7ca}", "Continue Dikte-Regenboog (Measurement Rainbow)") { state.toggle_show_measurement_rainbow(); }
            g_ui.end_row();
            if icon_tile_toggle(g_ui, state.curve_harmony.show_optical_rhythm_fill, "\u{f7ff}", "Optische Ritme-Vulling & Overshoot-Band") { state.toggle_show_optical_rhythm_fill(); }
            if icon_tile_toggle(g_ui, state.curve_harmony.show_stem_gauge, "\u{f52a}", "Structurele Stamdikte-Meter") { state.toggle_show_stem_gauge(); }
            if icon_tile_toggle(g_ui, state.curve_harmony.show_visual_centroid, "⊕", "Optisch Zwaartepunt (Visual Centroid ⊕)") { state.toggle_show_visual_centroid(); }
            if icon_tile(g_ui, "★", "Converteer Kubisch ↔ Hobby-Spline") && !state.convert_selected_cubic_nodes_to_hobby() {
                let _ = state.convert_selected_hobby_nodes_to_cubic();
            }
            g_ui.end_row();
        });
    }
}
