//! Single-Row macOS Sonoma / AppKit Toolbar built with `egui-macview` (`MacSegmentedControl` & `MacPopUpButton`)
//! for `vector_canvas_demo` (`SPEC-017 §2.9` & `SPEC-018 §1.1`).

use super::super::VIEW_MODE_SPECS;
use super::icon_studio_showcase::{demo_kinetic_effect_by_idx, DEMO_KINETIC_DEFAULT_PHASE};
use super::presets::StudioThemePreset;
use super::toolbar_specs::TOOL_ICON_SPECS;
use super::{DemoPreset, MockFontHost, ShowcaseChapter};
use egui::{ScrollArea, Separator, Ui};
use egui_macview::{MacPopUpButton, MacSegmentedControl, MacSymbol, MacTheme};
use grafia::adapters::{CanvasTool, GrafiaCanvasState};
use grafia::core::PrimitiveSubTool;
use grafia::tokens::GrafiaTheme;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum StudioWorkspaceMode {
    #[default]
    LetterStudio,
    IconStudio,
}

#[rustfmt::skip]
const CORE_STUDIO_TOOLS: [(CanvasTool, &str, &str); 8] = [
    (CanvasTool::Select, "\u{f61e}", "Selecteren (V)"), (CanvasTool::Pen, "\u{f2bf}", "Bézier-Pen (P)"),
    (CanvasTool::Primitives, "\u{f80c}", "Rechthoek / Cirkel (F)"), (CanvasTool::ShapeBuilder, "\u{f618}", "Vlakken Smelten"),
    (CanvasTool::Knife, "\u{f7c9}", "Mes (E)"), (CanvasTool::Pencil, "\u{f7cb}", "Vrije Schetspen (B)"),
    (CanvasTool::Measurement, "\u{f7ca}", "Meetlat (L)"), (CanvasTool::Text, "\u{f892}", "Tekst & Kerning (T)"),
];

#[rustfmt::skip]
const KINETIC_EFFECT_LABELS: [&str; 10] = [
    "Animatie: Pulse", "Animatie: Bounce", "Animatie: VarColor", "Animatie: Scale", "Animatie: DrawOn",
    "Animatie: DrawOff", "Animatie: Wiggle", "Animatie: Rotate", "Animatie: Breathe", "Animatie: MagicReplace",
];

#[rustfmt::skip]
pub const TOOL_CATEGORIES: [(&str, &[CanvasTool]); 4] = [
    ("1. Selectie & Transformatie", &[CanvasTool::Select, CanvasTool::Lasso, CanvasTool::Rotate, CanvasTool::Scale, CanvasTool::Zoom, CanvasTool::Text]),
    ("2. Vector & Vlak Constructie", &[CanvasTool::Pen, CanvasTool::Primitives, CanvasTool::Pencil, CanvasTool::Knife, CanvasTool::ShapeBuilder, CanvasTool::Pixel]),
    ("3. CAGD, Stems & Borstels", &[CanvasTool::Measurement, CanvasTool::Stem, CanvasTool::Trowel, CanvasTool::Cleaner]),
    ("4. Masters, Hinting & Notitie", &[CanvasTool::TrueTypeInstructor, CanvasTool::Hoi, CanvasTool::CurvedTrajectory, CanvasTool::Annotation]),
];

impl MockFontHost {
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
                state.glyph.width = 800.0;
                state.glyph.height = 800.0;
                state.set_icon_keyline_grid_enabled(true);
                self.fit_canvas_viewport(state);
            }
        }
    }
}

pub fn render_top_toolbar(
    ui: &mut Ui,
    state: &mut GrafiaCanvasState,
    theme: &mut GrafiaTheme,
    host: &mut MockFontHost,
    show_preview_panel: &mut bool,
) {
    let mac_theme = host.active_mac_theme();
    ScrollArea::horizontal()
        .id_salt("top_toolbar_scroll")
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                let nav_icon = MacSymbol::SidebarLeading.as_str();
                let mut sb_on = [host.show_left_sidebar];
                if MacSegmentedControl::new(&[nav_icon], 0)
                    .id_salt("sonoma_left_nav_toggle")
                    .theme(&mac_theme)
                    .show_multi(ui, &mut sb_on)
                    .is_some()
                {
                    host.show_left_sidebar = sb_on[0];
                }
                let ws_idx = usize::from(host.studio_mode == StudioWorkspaceMode::IconStudio);
                MacSegmentedControl::new(&["\u{f892} Letter", "\u{f77e} Icoon"], ws_idx)
                    .id_salt("sonoma_ws_seg")
                    .theme(&mac_theme)
                    .show(ui, |idx| {
                        let m = if idx == 1 {
                            StudioWorkspaceMode::IconStudio
                        } else {
                            StudioWorkspaceMode::LetterStudio
                        };
                        host.switch_studio_workspace(m, state);
                    });
                ui.add(Separator::default().vertical());
                render_appkit_tools_segmented(ui, state, &mac_theme);
                ui.add(Separator::default().vertical());
                render_appkit_view_mode_segmented(ui, state, &mac_theme);
                ui.add(Separator::default().vertical());
                render_appkit_scene_popups(ui, state, theme, host, &mac_theme);
                ui.add(Separator::default().vertical());
                render_appkit_utility_strip(ui, state, theme, host, show_preview_panel, &mac_theme);
            });
        });
}

fn render_appkit_tools_segmented(ui: &mut Ui, state: &mut GrafiaCanvasState, mac_theme: &MacTheme) {
    let sel_idx = CORE_STUDIO_TOOLS
        .iter()
        .position(|(t, _, _)| *t == state.active_tool)
        .unwrap_or(0);
    let icons: [&str; 8] = CORE_STUDIO_TOOLS.map(|(_, i, _)| i);
    MacSegmentedControl::new(&icons, sel_idx)
        .id_salt("sonoma_tools_seg")
        .theme(mac_theme)
        .show(ui, |idx| {
            let (tool, _, _) = CORE_STUDIO_TOOLS[idx];
            if tool == CanvasTool::Primitives && state.active_tool == CanvasTool::Primitives {
                state.drawing_tool_state.primitive_sub_tool =
                    if state.drawing_tool_state.primitive_sub_tool == PrimitiveSubTool::Rectangle {
                        PrimitiveSubTool::Circle
                    } else {
                        PrimitiveSubTool::Rectangle
                    };
            }
            state.activate_tool(tool);
        });
    ui.menu_button("\u{f608} Tools", |ui| {
        for (cat_idx, (cat_title, tools)) in TOOL_CATEGORIES.iter().enumerate() {
            if cat_idx > 0 {
                ui.separator();
            }
            ui.strong(*cat_title);
            egui::Grid::new(("tool_cat_grid", cat_idx))
                .num_columns(2)
                .spacing([8.0, 4.0])
                .show(ui, |g_ui| {
                    for (i, &tool) in tools.iter().enumerate() {
                        let icon = TOOL_ICON_SPECS
                            .iter()
                            .find(|(t, _, _)| *t == tool)
                            .map_or("", |(_, ic, _)| *ic);
                        let (nl_name, nl_tip) = super::tool_dutch_guide(tool);
                        let lbl = format!("{icon} {nl_name}");
                        if g_ui
                            .selectable_label(state.active_tool == tool, lbl)
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
    });
}

fn render_appkit_view_mode_segmented(ui: &mut Ui, st: &mut GrafiaCanvasState, tm: &MacTheme) {
    let specs = [
        VIEW_MODE_SPECS[0],
        VIEW_MODE_SPECS[1],
        VIEW_MODE_SPECS[3],
        VIEW_MODE_SPECS[2],
    ];
    let icons = specs.map(|s| s.2);
    let cur = specs.iter().position(|s| s.0 == st.view_mode).unwrap_or(0);
    MacSegmentedControl::new(&icons, cur)
        .id_salt("sonoma_view_seg")
        .theme(tm)
        .show(ui, |idx| st.view_mode = specs[idx].0);
}

fn render_appkit_scene_popups(
    ui: &mut Ui,
    st: &mut GrafiaCanvasState,
    theme: &mut GrafiaTheme,
    host: &mut MockFontHost,
    tm: &MacTheme,
) {
    let ch_labels: [&str; 6] = ShowcaseChapter::ALL.map(ShowcaseChapter::short_pill);
    let mut ch_idx = ShowcaseChapter::ALL
        .iter()
        .position(|&c| c == host.active_chapter)
        .unwrap_or(0);
    let prev_ch = ch_idx;
    MacPopUpButton::new(&mut ch_idx, &ch_labels)
        .id("sonoma_chapter_6_pop")
        .width(96.0)
        .theme(tm)
        .show(ui);
    if ch_idx != prev_ch {
        host.active_chapter = ShowcaseChapter::ALL[ch_idx];
        let first_preset = host.active_chapter.presets()[0];
        host.apply_preset(first_preset, st, theme);
    }
    let ch_presets = host.active_chapter.presets();
    let case_labels: [&str; 6] = ch_presets.map(|p| super::coach_card(p).short_button);
    let active_preset = DemoPreset::ALL[host.tour_index.min(DemoPreset::ALL.len() - 1)].0;
    let mut case_idx = ch_presets
        .iter()
        .position(|&p| p == active_preset)
        .unwrap_or(0);
    let prev_case = case_idx;
    MacPopUpButton::new(&mut case_idx, &case_labels)
        .id("sonoma_chapter_cases_6_pop")
        .width(196.0)
        .theme(tm)
        .show(ui);
    if case_idx != prev_case {
        host.apply_preset(ch_presets[case_idx], st, theme);
    }
    MacSegmentedControl::new(&["◀", "▶"], 0)
        .id_salt("sonoma_case_step_seg")
        .theme(tm)
        .show_momentary(ui, |idx| {
            if idx == 0 {
                host.step_tour_prev(st, theme);
            } else {
                host.step_tour_next(st, theme);
            }
        });
    if host.studio_mode == StudioWorkspaceMode::IconStudio {
        let mut eff_idx = (0..10)
            .find(|&i| {
                std::mem::discriminant(&st.icon_studio.kinetic.effect)
                    == std::mem::discriminant(&demo_kinetic_effect_by_idx(i))
            })
            .unwrap_or(1);
        let prev_eff = eff_idx;
        MacPopUpButton::new(&mut eff_idx, &KINETIC_EFFECT_LABELS)
            .id("sonoma_kin_pop")
            .width(142.0)
            .theme(tm)
            .show(ui);
        if eff_idx != prev_eff {
            st.icon_studio.kinetic.effect = demo_kinetic_effect_by_idx(eff_idx);
            st.icon_studio.kinetic.phase_t = st
                .icon_studio
                .kinetic
                .phase_t
                .max(DEMO_KINETIC_DEFAULT_PHASE);
            st.bump_content_generation();
        }
        let play_lbl = if st.icon_studio.kinetic.playing {
            "⏸"
        } else {
            "▶"
        };
        MacSegmentedControl::new(&[play_lbl], 0)
            .id_salt("sonoma_kinetic_play_btn")
            .theme(tm)
            .show_momentary(ui, |_| {
                st.icon_studio.kinetic.playing = !st.icon_studio.kinetic.playing;
                st.bump_content_generation();
            });
    }
}

fn toggle_demo_fullscreen(ui: &Ui, host: &mut MockFontHost) {
    host.is_fullscreen = !host.is_fullscreen;
    if host.is_fullscreen {
        host.show_coach_banner = false;
    }
    #[cfg(not(target_arch = "wasm32"))]
    ui.ctx()
        .send_viewport_cmd(egui::ViewportCommand::Fullscreen(host.is_fullscreen));
    #[cfg(target_arch = "wasm32")]
    {
        let _ = ui;
        if let Some(doc) = web_sys::window().and_then(|w| w.document()) {
            if doc.fullscreen_element().is_some() {
                doc.exit_fullscreen();
            } else if let Some(el) = doc.document_element() {
                let _ = el.request_fullscreen();
            }
        }
    }
}

fn render_appkit_utility_strip(
    ui: &mut Ui,
    state: &mut GrafiaCanvasState,
    theme: &mut GrafiaTheme,
    host: &mut MockFontHost,
    show_preview_panel: &mut bool,
    mac_theme: &MacTheme,
) {
    let mut th_idx = StudioThemePreset::ALL
        .iter()
        .position(|&p| p == host.studio_theme)
        .unwrap_or(0);
    let prev_th = th_idx;
    MacPopUpButton::new(&mut th_idx, &StudioThemePreset::LABELS)
        .id("sonoma_studio_theme_pop")
        .width(148.0)
        .theme(mac_theme)
        .show(ui);
    if th_idx != prev_th {
        host.apply_studio_theme(StudioThemePreset::ALL[th_idx], state, theme);
    }
    let mut active = [
        host.show_mobile_thumb_hud,
        host.thumb_dock_right,
        host.show_right_inspector,
        *show_preview_panel,
        host.show_coach_banner,
        host.is_fullscreen,
    ];
    if let Some(i) =
        MacSegmentedControl::new(&["\u{f7c7}", "L⇄R", "\u{f424}", "\u{f7ff}", "?", "⛶"], 0)
            .id_salt("sonoma_util_multi_seg")
            .theme(mac_theme)
            .show_multi(ui, &mut active)
    {
        match i {
            0 => host.set_mobile_thumb_hud_enabled(active[0], state),
            1 => host.thumb_dock_right = active[1],
            2 => host.show_right_inspector = active[2],
            3 => *show_preview_panel = active[3],
            4 => host.show_coach_banner = active[4],
            _ => toggle_demo_fullscreen(ui, host),
        }
    }
    MacSegmentedControl::new(&["\u{f37d}"], 0)
        .id_salt("sonoma_fit_btn")
        .theme(mac_theme)
        .show_momentary(ui, |_| host.fit_canvas_viewport(state));
}
