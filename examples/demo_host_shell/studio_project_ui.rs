//! `SPEC-019` Fase 1 & 2 (`M1.1`..`M2.4`): Working Font & Icon Studio Document Engine,
//! `Font & Icoon Overzichtsraster` (*Studio Font View*), Multi-Tab Canvas Bar, and W3C `.svg` I/O.

use super::sonoma_top_bar::{
    liquid_glass_capsule_frame, section_header_text, StudioSubWorkspace, StudioWorkspaceMode,
};
use super::{DemoPreset, MockFontHost};
use egui::{Button, Grid, RichText, ScrollArea, Ui, Vec2};
use egui_macview::{MacSegmentedControl, MacSymbol, MacTheme};
use grafia::adapters::{CanvasTool, GrafiaCanvasState};
use grafia::core::Glyph;
use grafia::tokens::GrafiaTheme;

const GRID_CARD_W_DP: f32 = 132.0;
const GRID_CARD_H_DP: f32 = 76.0;
const GRID_COLS_MIN: usize = 3;
const GRID_COLS_MAX: usize = 10;
const GRID_COL_STEP_DP: f32 = 144.0;
const GRID_SPACING_DP: f32 = 8.0;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum FontGridFilter {
    #[default]
    All,
    Letters,
    Digits,
    Icons,
}

#[rustfmt::skip]
impl FontGridFilter {
    pub const LABELS: [&'static str; 4] = ["Alles", "Letters A–Z", "Cijfers 0–9", "Iconen ❖"];
    #[must_use]
    pub fn matches(self, name: &str) -> bool {
        let is_icon = name.contains('.');
        let is_digit = name.len() == 1 && name.chars().next().is_some_and(|c| c.is_ascii_digit());
        let is_letter = name.len() == 1 && name.chars().next().is_some_and(|c| c.is_ascii_alphabetic());
        match self { Self::All => true, Self::Letters => is_letter, Self::Digits => is_digit, Self::Icons => is_icon }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StudioCanvasTab {
    pub title: String,
    pub glyph_key: String,
    pub is_icon: bool,
    pub is_word_stream: bool,
}

#[rustfmt::skip]
impl StudioCanvasTab {
    #[must_use]
    pub fn glyph(key: impl Into<String>) -> Self {
        let k = key.into();
        let is_icon = k.contains('.');
        let title = if is_icon { format!("❖ {k}") } else { format!("/{k}") };
        Self { title, glyph_key: k, is_icon, is_word_stream: false }
    }
    #[must_use]
    pub fn word_stream(word: impl Into<String>) -> Self {
        let w = word.into();
        Self { title: format!("\"{w}\""), glyph_key: w, is_icon: false, is_word_stream: true }
    }
}

impl MockFontHost {
    #[rustfmt::skip]
    pub fn populate_extended_studio_catalog(&mut self) {
        for ch in 'A'..='Z' {
            let k = ch.to_string();
            self.glyph_library.entry(k.clone()).or_insert_with(|| {
                let c = Self::make_rect_contour(1, (ch as u64) * 10, 90.0, 0.0, 470.0, 700.0);
                Self::make_box_glyph(&k, 560.0, vec![c])
            });
        }
        for ch in '0'..='9' {
            let k = ch.to_string();
            self.glyph_library.entry(k.clone()).or_insert_with(|| {
                let c = Self::make_rect_contour(1, (ch as u64) * 10, 100.0, 0.0, 440.0, 680.0);
                Self::make_box_glyph(&k, 540.0, vec![c])
            });
        }
        for icon_name in ["wifi.signal", "mic.slash", "play.circle", "orbit.sensor", "rosette.glass"] {
            self.glyph_library.entry(icon_name.to_owned()).or_insert_with(|| {
                let c = Self::make_rect_contour(1, 500, 160.0, 160.0, 640.0, 640.0);
                let mut g = Self::make_box_glyph(icon_name, 800.0, vec![c]);
                g.height = 800.0;
                g
            });
        }
    }

    pub fn sync_active_glyph_to_library(&mut self, state: &GrafiaCanvasState) {
        if !state.glyph.name.is_empty() && state.text_stream.slots.len() <= 1 {
            self.glyph_library
                .insert(state.glyph.name.clone(), state.glyph.clone());
        }
    }

    pub fn open_or_activate_tab(
        &mut self,
        tab: StudioCanvasTab,
        state: &mut GrafiaCanvasState,
        theme: &mut GrafiaTheme,
    ) {
        self.sync_active_glyph_to_library(state);
        let idx = self
            .open_tabs
            .iter()
            .position(|t| t.glyph_key == tab.glyph_key && t.is_word_stream == tab.is_word_stream)
            .unwrap_or_else(|| {
                self.open_tabs.push(tab);
                self.open_tabs.len() - 1
            });
        self.active_tab_index = idx;
        self.show_font_grid_view = false;
        self.load_tab_onto_canvas(idx, state, theme);
    }

    #[rustfmt::skip]
    pub fn load_tab_onto_canvas(&mut self, idx: usize, state: &mut GrafiaCanvasState, theme: &mut GrafiaTheme) {
        let Some(tab) = self.open_tabs.get(idx).cloned() else {
            return;
        };
        if tab.is_word_stream {
            (self.studio_mode, self.active_workspace, self.text_buffer) = (StudioWorkspaceMode::LetterStudio, StudioSubWorkspace::LetterContourAndSpacing, tab.glyph_key);
            let slots = self.build_slots_for_text(&self.text_buffer);
            state.activate_tool(CanvasTool::Text);
            state.set_text_stream_slots_with_caret(slots, 0, 1);
            (state.show_text_measurement_line, self.initial_canvas_fitted) = (true, false);
            self.fit_canvas_viewport(state);
            return;
        }
        if tab.is_icon {
            self.studio_mode = StudioWorkspaceMode::IconStudio;
            if self.active_workspace.studio_mode() != StudioWorkspaceMode::IconStudio {
                self.active_workspace = StudioSubWorkspace::IconVectorAndGlass;
            }
            let preset = match tab.glyph_key.as_str() {
                "mic.slash" => Some(DemoPreset::KineticMicrophoneKnockoutHalo),
                "play.circle" => Some(DemoPreset::IconKeylineArtboardAndOpticalPlay),
                "orbit.sensor" => Some(DemoPreset::KineticOrbitMotionPathAndSensor),
                "rosette.glass" => Some(DemoPreset::IconRadialSymmetryAndOblique),
                "wifi.signal" => Some(DemoPreset::KineticWifiBounceAndColorModes),
                _ => None,
            };
            if let Some(p) = preset {
                self.apply_preset(p, state, theme);
            } else if let Some(g) = self.glyph_library.get(&tab.glyph_key).cloned() {
                self.load_library_glyph_direct(g, true, state);
            }
        } else if let Some(g) = self.glyph_library.get(&tab.glyph_key).cloned() {
            self.studio_mode = StudioWorkspaceMode::LetterStudio;
            if self.active_workspace.studio_mode() != StudioWorkspaceMode::LetterStudio {
                self.active_workspace = StudioSubWorkspace::LetterContourAndSpacing;
            }
            self.load_library_glyph_direct(g, false, state);
        }
        self.initial_canvas_fitted = false;
    }

    fn load_library_glyph_direct(&self, g: Glyph, is_icon: bool, state: &mut GrafiaCanvasState) {
        state.text_stream = grafia::core::CanvasTextStream::new();
        state.select_color_sublayer(None);
        state.color_sublayers.clear();
        state.contour_strokes.clear();
        state.show_metrics = !is_icon;
        state.set_icon_keyline_grid_enabled(is_icon);
        let metrics = state.metrics.clone();
        state.load_glyph(g, metrics);
        state.show_selection_info_hud = true;
        self.fit_canvas_viewport(state);
    }

    #[rustfmt::skip]
    pub fn create_new_glyph_or_icon(&mut self, is_icon: bool, state: &mut GrafiaCanvasState, theme: &mut GrafiaTheme) -> String {
        self.sync_active_glyph_to_library(state);
        let mut n = 1usize;
        let key = loop {
            let cand = if is_icon { format!("icon.custom{n}") } else { format!("glyph{n}") };
            if !self.glyph_library.contains_key(&cand) {
                break cand;
            }
            n += 1;
        };
        let (w, h, c) = if is_icon {
            (800.0, 800.0, Self::make_rect_contour(1, 900 + n as u64 * 10, 180.0, 180.0, 620.0, 620.0))
        } else {
            (560.0, 1000.0, Self::make_rect_contour(1, 900 + n as u64 * 10, 90.0, 0.0, 470.0, 700.0))
        };
        let mut g = Self::make_box_glyph(&key, w, vec![c]);
        g.height = h;
        self.glyph_library.insert(key.clone(), g);
        self.open_or_activate_tab(StudioCanvasTab::glyph(key.clone()), state, theme);
        self.push_log(format!("StudioProject: Nieuw item '{key}' aangemaakt en geopend"));
        key
    }

    pub fn export_active_to_svg_buffer(&mut self, state: &GrafiaCanvasState) -> String {
        let svg = state.export_kinetic_symbol_to_svg();
        self.last_exported_svg = svg.clone();
        self.push_log(format!(
            "StudioProject: '{}' geëxporteerd naar W3C SVG ({} bytes)",
            state.glyph.name,
            svg.len()
        ));
        svg
    }

    #[rustfmt::skip]
    pub fn render_multi_tab_canvas_bar(&mut self, ui: &mut Ui, state: &mut GrafiaCanvasState, theme: &mut GrafiaTheme) {
        let tm = self.active_mac_theme();
        ui.horizontal(|ui| {
            liquid_glass_capsule_frame(&tm).show(ui, |ui| {
                let mut g_state = [self.show_font_grid_view];
                if MacSegmentedControl::new(&["\u{f804} Overzicht"], 0).id_salt("studio_font_grid_toggle").theme(&tm).show_multi(ui, &mut g_state).is_some() {
                    self.sync_active_glyph_to_library(state);
                    self.show_font_grid_view = g_state[0];
                }
                let (mut clicked_tab, mut closed_tab) = (None, None);
                for (i, tab) in self.open_tabs.iter().enumerate() {
                    let is_act = !self.show_font_grid_view && i == self.active_tab_index;
                    if ui.selectable_label(is_act, &tab.title).on_hover_text("Wissel naar dit geopende studio-tabblad").clicked() {
                        clicked_tab = Some(i);
                    }
                    if self.open_tabs.len() > 1 && ui.small_button("×").on_hover_text("Sluit tabblad (wijzigingen blijven bewaard in project)").clicked() {
                        closed_tab = Some(i);
                    }
                }
                if let Some(i) = clicked_tab {
                    self.sync_active_glyph_to_library(state);
                    (self.active_tab_index, self.show_font_grid_view) = (i, false);
                    self.load_tab_onto_canvas(i, state, theme);
                }
                if let Some(i) = closed_tab {
                    self.sync_active_glyph_to_library(state);
                    self.open_tabs.remove(i);
                    self.active_tab_index = self.active_tab_index.min(self.open_tabs.len().saturating_sub(1));
                    self.load_tab_onto_canvas(self.active_tab_index, state, theme);
                }
                if ui.button(MacSymbol::Plus.as_str()).on_hover_text("Nieuwe Letter of Nieuw 800×800 Icoon toevoegen aan project").clicked() {
                    let _ = self.create_new_glyph_or_icon(self.studio_mode == StudioWorkspaceMode::IconStudio, state, theme);
                }
            });
        });
    }

    #[rustfmt::skip]
    pub fn render_font_and_icon_grid_overview(&mut self, ui: &mut Ui, state: &mut GrafiaCanvasState, theme: &mut GrafiaTheme) {
        let tm = self.active_mac_theme();
        liquid_glass_capsule_frame(&tm).show(ui, |ui| {
            self.render_font_grid_header_bar(ui, state, theme, &tm);
            ui.separator();
            let keys: Vec<String> = self.glyph_library.keys().filter(|k| self.font_grid_filter.matches(k)).cloned().collect();
            let cols = ((ui.available_width() / GRID_COL_STEP_DP).floor() as usize).clamp(GRID_COLS_MIN, GRID_COLS_MAX);
            ScrollArea::vertical().id_salt("studio_font_grid_scroll").show(ui, |s_ui| {
                Grid::new("studio_font_and_icon_tiles_grid").num_columns(cols).spacing([GRID_SPACING_DP, GRID_SPACING_DP]).show(s_ui, |g_ui| {
                    let mut open_key = None;
                    for (idx, k) in keys.iter().enumerate() {
                        if let Some(g) = self.glyph_library.get(k) {
                            let pts: usize = g.contours.iter().map(|c| c.nodes.len()).sum();
                            let title = if k.contains('.') { format!("❖ {k}\n{:.0}×{:.0} · {pts} pt", g.width, g.height) } else { format!("/{k}\nADV {:.0} · {pts} pt", g.width) };
                            let btn = Button::selectable(state.glyph.name == *k, RichText::new(title).strong());
                            if g_ui.add_sized(Vec2::new(GRID_CARD_W_DP, GRID_CARD_H_DP), btn).on_hover_text("Klik om te openen op het Vector-Canvas").clicked() {
                                open_key = Some(k.clone());
                            }
                        }
                        if idx % cols == cols - 1 {
                            g_ui.end_row();
                        }
                    }
                    if let Some(k) = open_key {
                        self.open_or_activate_tab(StudioCanvasTab::glyph(k), state, theme);
                    }
                });
            });
        });
    }

    #[rustfmt::skip]
    fn render_font_grid_header_bar(&mut self, ui: &mut Ui, state: &mut GrafiaCanvasState, theme: &mut GrafiaTheme, tm: &MacTheme) {
        ui.horizontal_wrapped(|ui| {
            ui.label(section_header_text(&format!("STUDIO PROJECT OVERZICHT ({} ITEMS IN BIBLIOTHEEK)", self.glyph_library.len()), tm));
            let cur_f = match self.font_grid_filter { FontGridFilter::All => 0, FontGridFilter::Letters => 1, FontGridFilter::Digits => 2, FontGridFilter::Icons => 3 };
            MacSegmentedControl::new(&FontGridFilter::LABELS, cur_f).id_salt("font_grid_filter_seg").theme(tm).show(ui, |idx| {
                self.font_grid_filter = match idx { 1 => FontGridFilter::Letters, 2 => FontGridFilter::Digits, 3 => FontGridFilter::Icons, _ => FontGridFilter::All };
            });
            if ui.button("+ Nieuwe Letter").on_hover_text("Maak een nieuwe 1000 UPM letter-glyph aan").clicked() {
                let _ = self.create_new_glyph_or_icon(false, state, theme);
            }
            if ui.button("+ Nieuw 800×800 Icoon").on_hover_text("Maak een nieuw 800×800 Keyline vector-icoon aan").clicked() {
                let _ = self.create_new_glyph_or_icon(true, state, theme);
            }
            if ui.button("⬇ Exporteer W3C SVG").on_hover_text("Exporteer het actieve item naar W3C SVG").clicked() {
                let _ = self.export_active_to_svg_buffer(state);
            }
        });
    }
}
