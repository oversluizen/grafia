//! Glyphs-Style 2-Column Structured Glyph / Icon Info Card (`Zone D2`) and Workspace Switcher
//! for `MockFontHost` (`SPEC-018 §3.7`).

use super::presets::StudioThemePreset;
use super::sonoma_top_bar::{section_header_text, StudioSubWorkspace, StudioWorkspaceMode};
use super::toolbar_specs::{
    GLASS_VALUE_FONT_PT, INFO_GRID_SPACING_X_DP, INFO_GRID_SPACING_Y_DP, TOOL_GRID_SPACING_X_DP,
    TOOL_GRID_SPACING_Y_DP, TOOL_ICON_SPECS,
};
use super::{DemoPreset, MockFontHost, ShowcaseChapter, METRIC_STEP_UPM};
use egui::{Grid, RichText, Ui};
use egui_macview::{MacSegmentedControl, MacSymbol, MacTheme};
use grafia::adapters::{CanvasTool, GrafiaCanvasState};
use grafia::core::{SelectionInfo, SelectionInfoAction};
use grafia::tokens::GrafiaTheme;

pub fn render_tool_category_grid(
    ui: &mut Ui,
    cat_idx: usize,
    tools: &[CanvasTool],
    state: &mut GrafiaCanvasState,
) {
    Grid::new(("tool_cat_subgrid", cat_idx))
        .num_columns(2)
        .spacing([TOOL_GRID_SPACING_X_DP, TOOL_GRID_SPACING_Y_DP])
        .show(ui, |g_ui| {
            for (i, &tool) in tools.iter().enumerate() {
                let icon = TOOL_ICON_SPECS
                    .iter()
                    .find(|(t, _, _)| *t == tool)
                    .map_or("", |(_, ic, _)| *ic);
                let (nl_name, nl_tip) = super::tool_dutch_guide(tool);
                if g_ui
                    .selectable_label(state.active_tool == tool, format!("{icon} {nl_name}"))
                    .on_hover_text(nl_tip)
                    .clicked()
                {
                    state.activate_tool(tool);
                    g_ui.close();
                }
                if i % 2 == 1 {
                    g_ui.end_row();
                }
            }
        });
}

fn mono_val(text: impl Into<String>, tm: &MacTheme) -> RichText {
    RichText::new(text.into())
        .monospace()
        .size(GLASS_VALUE_FONT_PT)
        .color(tm.label())
        .strong()
}

impl MockFontHost {
    pub fn render_zone_c_overlays_and_windows(
        &mut self,
        ui: &mut Ui,
        st: &mut GrafiaCanvasState,
        theme: &mut GrafiaTheme,
        show_preview_panel: &mut bool,
        tm: &MacTheme,
    ) {
        ui.menu_button("\u{f424}", |ui| {
            super::inspector_ui::render_overlays_popover_contents(ui, st, tm);
        })
        .response
        .on_hover_text("Canvas Overlays: Hulplijnen, Krommingskam, Tunni-lijnen & Optica");
        let mut active = [
            *show_preview_panel,
            self.show_mobile_thumb_hud,
            self.thumb_dock_right,
            self.show_right_inspector,
            self.is_fullscreen,
        ];
        if let Some(i) = MacSegmentedControl::new(
            &[
                "\u{f7ff}",
                "\u{f7c7}",
                "L/R",
                MacSymbol::SidebarLeading.as_str(),
                "\u{f386}",
            ],
            0,
        )
        .id_salt("zone_c_util_seg")
        .theme(tm)
        .show_multi(ui, &mut active)
        {
            match i {
                0 => *show_preview_panel = active[0],
                1 => self.set_mobile_thumb_hud_enabled(active[1], st),
                2 => self.thumb_dock_right = active[2],
                3 => self.show_right_inspector = active[3],
                _ => {
                    self.is_fullscreen = !self.is_fullscreen;
                    if self.is_fullscreen {
                        self.show_coach_banner = false;
                    }
                }
            }
        }
        MacSegmentedControl::new(&["\u{f4B6}", "\u{f37d}"], 0)
            .id_salt("zone_c_theme_fit_seg")
            .theme(tm)
            .show_momentary(ui, |idx| {
                if idx == 0 {
                    let cur = StudioThemePreset::ALL
                        .iter()
                        .position(|&p| p == self.studio_theme)
                        .unwrap_or(0);
                    let next = StudioThemePreset::ALL[(cur + 1) % StudioThemePreset::ALL.len()];
                    self.apply_studio_theme(next, st, theme);
                } else {
                    self.fit_canvas_viewport(st);
                }
            });
    }

    pub fn activate_sub_workspace(
        &mut self,
        sub: StudioSubWorkspace,
        state: &mut GrafiaCanvasState,
        theme: &mut GrafiaTheme,
    ) {
        self.active_workspace = sub;
        self.switch_studio_workspace(sub.studio_mode(), state);
        self.apply_preset(sub.default_preset(), state, theme);
        self.initial_canvas_fitted = false;
    }

    pub fn switch_studio_workspace(
        &mut self,
        mode: StudioWorkspaceMode,
        state: &mut GrafiaCanvasState,
    ) {
        if self.studio_mode == mode {
            return;
        }
        self.studio_mode = mode;
        if mode == StudioWorkspaceMode::LetterStudio {
            if matches!(
                self.active_chapter,
                ShowcaseChapter::IconStudioEnAnimatie | ShowcaseChapter::HostStekkersEnHardware
            ) {
                self.active_chapter = ShowcaseChapter::BasisTekenen;
            }
            state.icon_studio.kinetic.playing = false;
            state.icon_studio.kinetic.phase_t = 0.0;
            state.icon_studio.kinetic.playback_mode = grafia::core::KineticPlaybackMode::OneShot;
            state.icon_studio.radial_symmetry.fold_count = 1;
            state.icon_studio.variable_width_strokes.clear();
            state.show_metrics = true;
            state.set_icon_keyline_grid_enabled(false);
            if !state.color_sublayers.is_empty()
                || state.glyph.name.contains('.')
                || state.glyph.width > 700.0
            {
                state.load_glyph_archetype(state.active_archetype);
            }
            state.show_selection_info_hud = true;
            self.fit_canvas_viewport(state);
        } else {
            self.active_chapter = ShowcaseChapter::IconStudioEnAnimatie;
            state.text_stream = grafia::core::CanvasTextStream::new();
            state.set_companion_master_layers(Vec::new());
            state.show_master_compatibility = false;
            state.set_postscript_hints(Vec::new());
            state.set_truetype_instructions(Vec::new());
            state.set_pixel_preview_ppem(None);
            state.show_metrics = false;
            if state.color_sublayers.is_empty() || !state.glyph.name.contains('.') {
                self.apply_icon_studio_preset(DemoPreset::KineticWifiBounceAndColorModes, state);
            } else {
                (state.glyph.width, state.glyph.height) = (800.0, 800.0);
                state.set_icon_keyline_grid_enabled(true);
                self.fit_canvas_viewport(state);
            }
        }
    }

    pub(crate) fn render_structured_info_card(
        &mut self,
        ui: &mut Ui,
        state: &mut GrafiaCanvasState,
        tm: &MacTheme,
    ) {
        let is_icon = self.studio_mode == StudioWorkspaceMode::IconStudio;
        ui.label(section_header_text(
            if is_icon {
                "ICOON & SUBLAAG GEGEVENS"
            } else {
                "GLYPH & KNOOP GEGEVENS"
            },
            tm,
        ));
        let total_nodes: usize = state.contours.iter().map(Vec::len).sum();
        Grid::new("structured_glyph_info_2col_grid")
            .num_columns(2)
            .spacing([INFO_GRID_SPACING_X_DP, INFO_GRID_SPACING_Y_DP])
            .striped(true)
            .show(ui, |g_ui| {
                let row = |g_ui: &mut Ui, lbl: &str, val: String| {
                    g_ui.label(section_header_text(lbl, tm));
                    g_ui.label(mono_val(val, tm));
                    g_ui.end_row();
                };
                if is_icon {
                    row(
                        g_ui,
                        "ARTBOARD",
                        format!("{:.0} × {:.0} UPM", state.glyph.width, state.glyph.height),
                    );
                    let sub_txt = state
                        .selected_color_sublayer_index()
                        .and_then(|i| {
                            state
                                .color_sublayers
                                .get(i)
                                .map(|s| (i + 1, s.name.as_str()))
                        })
                        .map_or_else(
                            || format!("{} sublagen", state.color_sublayers.len()),
                            |(n, name)| format!("{n} · {name}"),
                        );
                    row(g_ui, "ACTIEVE LAAG", sub_txt);
                    let folds = state.icon_studio.radial_symmetry.fold_count;
                    row(
                        g_ui,
                        "SYMMETRIE / FASE",
                        format!("{folds}× · t={:.2}", state.icon_studio.kinetic.phase_t),
                    );
                } else {
                    row(g_ui, "GLYPH", format!("/{}", state.glyph.name));
                    row(
                        g_ui,
                        "BREEDTE (ADV)",
                        format!("{:.0} UPM", state.glyph.width),
                    );
                    if let SelectionInfo::GlyphBase(gb) = state.selection_info() {
                        row(g_ui, "LSB / RSB", format!("{:.0} / {:.0}", gb.lsb, gb.rsb));
                    }
                }
                row(
                    g_ui,
                    "CONTOUREN / KNOPEN",
                    format!(
                        "{} / {total_nodes} ({} sel.)",
                        state.contours.len(),
                        state.selected_nodes.len()
                    ),
                );
                row(g_ui, "SELECTIE POS", Self::format_selection_value(state));
            });
        if !is_icon {
            Self::render_quick_metric_steppers(ui, state);
        }
    }

    #[rustfmt::skip]
    fn format_selection_value(state: &GrafiaCanvasState) -> String {
        match state.selection_info() {
            SelectionInfo::SingleOnCurve(n) => format!("X {:.1}  Y {:.1}", n.x, n.y),
            SelectionInfo::SingleOffCurve(h) => format!("Δ({:.1}, {:.1}) L={:.1}", h.delta_x, h.delta_y, h.length),
            SelectionInfo::MultiNode(m) => format!("{:.0}×{:.0} @ ({:.0}, {:.0})", m.width, m.height, m.origin_x, m.origin_y),
            SelectionInfo::GlyphBase(g) => format!("{:.0}×{:.0} @ ({:.0}, {:.0})", g.bbox_w, g.bbox_h, g.bbox_x, g.bbox_y),
            SelectionInfo::Guide(gd) => format!("({:.0}, {:.0}) {:.1}°", gd.x, gd.y, gd.angle_deg),
            SelectionInfo::HobbyNode(h) => format!("X {:.1}  Y {:.1}", h.x, h.y),
            SelectionInfo::BackgroundImage(bg) => format!("{:.0}%×{:.0}%", bg.scale_x_pct, bg.scale_y_pct),
            _ => "Geen selectie".to_owned(),
        }
    }

    #[rustfmt::skip]
    fn render_quick_metric_steppers(ui: &mut Ui, state: &mut GrafiaCanvasState) {
        if let SelectionInfo::GlyphBase(g) = state.selection_info() {
            ui.horizontal_wrapped(|ui| {
                use SelectionInfoAction as Act;
                for (icon, tip, act) in [
                    ("\u{f4fd}−", "LSB −10 UPM [⌃⌥←] — Verklein linker zijruimte", Act::SetLsb(g.lsb - METRIC_STEP_UPM)),
                    ("\u{f4fd}+", "LSB +10 UPM [⌃⌥→] — Vergroot linker zijruimte", Act::SetLsb(g.lsb + METRIC_STEP_UPM)),
                    ("\u{f7e1}", "Centreer in Breedte [⌘⇧C] — Verdeel LSB en RSB symmetrisch", Act::CenterInWidth),
                    ("\u{f50a}−", "RSB −10 UPM [⌘⌥←] — Verklein rechter zijruimte", Act::SetRsb(g.rsb - METRIC_STEP_UPM)),
                    ("\u{f50a}+", "RSB +10 UPM [⌘⌥→] — Vergroot rechter zijruimte", Act::SetRsb(g.rsb + METRIC_STEP_UPM)),
                ] {
                    if ui.button(icon).on_hover_text(tip).clicked() {
                        let _ = state.apply_selection_info_action(act);
                    }
                }
            });
        }
    }
}
