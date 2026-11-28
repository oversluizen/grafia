//! `SPEC-017 §2.9` & `SPEC-018` Icon Studio Vector-, Kleurlagen- & Kinetische Animatie-Werkbank UI.

use super::icon_studio_showcase::{
    DEMO_DT_MAX_SEC, DEMO_DT_MIN_SEC, DEMO_OBLIQUE_DEG, DEMO_SYMMETRY_CENTER,
};
use super::inspector_ui::{icon_tile, icon_tile_toggle};
use super::toolbar_specs::GRID_SPACING_DP;
use super::{DemoPreset, MockFontHost, ShowcaseChapter};
use egui::{ColorImage, Grid, Image, Slider, TextureOptions, Ui, Vec2};
use egui_macview::MacSymbol;
use grafia::adapters::{CanvasTool, CanvasViewMode, GrafiaCanvasState};
use grafia::core::{
    ContourStrokeStyle, DrawDirection, LineCapStyle, LineJoinStyle, StrokePosition,
    SymbolColorRenderingMode, SymbolHierarchyLevel,
};

const CRISPNESS_DIM_PX: u32 = 24;
const CRISPNESS_DISPLAY_DP: f32 = 22.0;
const STROKE_CYCLE_WIDTHS_UPM: [f64; 3] = [0.0, 24.0, 40.0];
const KNOCKOUT_SLIDER_MAX_UPM: f64 = 48.0;

impl MockFontHost {
    pub fn render_icon_studio_and_kinetic_bar(&mut self, ui: &mut Ui, st: &mut GrafiaCanvasState) {
        if st.icon_studio.kinetic.playing {
            let dt = ui
                .input(|i| i.stable_dt)
                .clamp(DEMO_DT_MIN_SEC, DEMO_DT_MAX_SEC);
            st.step_kinetic_timeline(f64::from(dt));
            ui.ctx().request_repaint();
        }
        let is = &st.icon_studio;
        if self.studio_mode != super::StudioWorkspaceMode::IconStudio
            && self.active_chapter != ShowcaseChapter::IconStudioEnAnimatie
            && st.active_tool != CanvasTool::ShapeBuilder
            && is.variable_width_strokes.is_empty()
            && is.radial_symmetry.fold_count <= 1
            && !is.kinetic.is_active()
        {
            return;
        }
        self.render_icon_templates_and_sublayers(ui, st);
        ui.separator();
        self.render_sublayer_style_and_color_modes(ui, st);
        ui.separator();
        self.render_icon_geometry_and_optical_controls(ui, st);
    }

    fn render_icon_templates_and_sublayers(&mut self, ui: &mut Ui, st: &mut GrafiaCanvasState) {
        #[rustfmt::skip]
        const TMPL_TILES: [(&str, &str, Option<DemoPreset>); 4] = [
            (MacSymbol::Plus.as_str(), "Nieuw 800×800 Keyline Icoon-Artboard [⌘N]", None),
            ("\u{f7ff}", "Wi-Fi Signaal & 4 Apple Kleurmodi (Case 31)", Some(DemoPreset::KineticWifiBounceAndColorModes)),
            ("\u{f7c9}", "Microfoon Knockout-Slash 22 UPM Halo (Case 33)", Some(DemoPreset::KineticMicrophoneKnockoutHalo)),
            ("\u{f618}", "Planaire Vormenbouwer & VarWidth (Case 26)", Some(DemoPreset::IconPlanarShapeBuilderMerge)),
        ];
        Grid::new("icon_studio_templates_grid_4col")
            .num_columns(4)
            .spacing([GRID_SPACING_DP * 2.0, GRID_SPACING_DP * 2.0])
            .show(ui, |g_ui| {
                for (ic, tip, preset) in TMPL_TILES {
                    if icon_tile(g_ui, ic, tip) {
                        if let Some(p) = preset {
                            self.apply_icon_studio_preset(p, st);
                        } else {
                            self.apply_blank_icon_artboard_preset(st);
                        }
                    }
                }
                g_ui.end_row();
            });
        if !st.color_sublayers.is_empty() {
            let sel = st.selected_color_sublayer_index();
            ui.horizontal_wrapped(|ui| {
                for idx in 0..st.color_sublayers.len() {
                    let tip = st
                        .color_sublayers
                        .get(idx)
                        .map_or_else(String::new, |s| format!("Sublaag {} · {}", idx + 1, s.name));
                    if icon_tile_toggle(ui, sel == Some(idx), &format!("L{}", idx + 1), &tip) {
                        st.select_color_sublayer(Some(idx));
                        if let Some(sub) = st.color_sublayers.get(idx) {
                            st.selected_nodes = sub
                                .contours
                                .iter()
                                .flat_map(|c| c.nodes.iter().map(|n| n.id))
                                .collect();
                        }
                    }
                }
                if sel.is_some() {
                    if icon_tile(
                        ui,
                        "↓",
                        "Z-Order Omlaag [⌘[] — Verplaats kleurlaag naar achteren",
                    ) {
                        let _ = st.reorder_selected_color_sublayer(-1);
                    }
                    if icon_tile(
                        ui,
                        "↑",
                        "Z-Order Omhoog [⌘]] — Verplaats kleurlaag naar voren",
                    ) {
                        let _ = st.reorder_selected_color_sublayer(1);
                    }
                }
            });
        }
    }

    fn render_sublayer_style_and_color_modes(&self, ui: &mut Ui, st: &mut GrafiaCanvasState) {
        if let Some(sel_idx) = st.selected_color_sublayer_index() {
            let cur_tag = st
                .icon_studio
                .symbol_layer_tags
                .get(sel_idx)
                .copied()
                .unwrap_or_default();
            #[rustfmt::skip]
            const HIER_TILES: [(SymbolHierarchyLevel, &str, &str); 3] = [
                (SymbolHierarchyLevel::Primary, "1°", "Primaire Hiërarchielaag (100% Dekking)"),
                (SymbolHierarchyLevel::Secondary, "2°", "Secundaire Hiërarchielaag (50% Dekking)"),
                (SymbolHierarchyLevel::Tertiary, "3°", "Tertiaire Hiërarchielaag (25% Dekking)"),
            ];
            Grid::new("icon_sublayer_tags_grid_4col")
                .num_columns(4)
                .spacing([GRID_SPACING_DP * 2.0, GRID_SPACING_DP * 2.0])
                .show(ui, |g_ui| {
                    for (lvl, lbl, tip) in HIER_TILES {
                        if icon_tile_toggle(g_ui, cur_tag.hierarchy_level == lvl, lbl, tip) {
                            let _ = st.set_selected_sublayer_hierarchy_level(lvl);
                        }
                    }
                    let (dir_icon, dir_tip, next_dir) = match cur_tag.draw_direction {
                        DrawDirection::StartToEnd => {
                            ("→", "Tekenrichting: Start → Eind", DrawDirection::CenterOut)
                        }
                        DrawDirection::CenterOut => (
                            "↔",
                            "Tekenrichting: Midden ↔ Uit (SF Symbols 7)",
                            DrawDirection::EndToStart,
                        ),
                        DrawDirection::EndToStart => (
                            "←",
                            "Tekenrichting: Eind → Start",
                            DrawDirection::StartToEnd,
                        ),
                    };
                    if icon_tile(g_ui, dir_icon, dir_tip) {
                        let _ = st.set_selected_sublayer_draw_direction(next_dir);
                    }
                    g_ui.end_row();
                });
            Self::render_selected_sublayer_stroke_and_halo(
                ui,
                st,
                sel_idx,
                cur_tag.knockout_margin_upm,
            );
        }
        Self::render_color_mode_4_tiles(ui, st);
    }

    fn render_color_mode_4_tiles(ui: &mut Ui, st: &mut GrafiaCanvasState) {
        #[rustfmt::skip]
        const CM_TILES: [(&str, &str); 4] = [
            ("\u{f7f8}", "Monochrome Kleurmodus — Uniforme tintkleur"),
            ("\u{f7ff}", "Hierarchical Kleurmodus — 100% / 50% / 25% dekking"),
            ("\u{f80d}", "Palette Kleurmodus — 3-kleuren triade voor 1°/2°/3°"),
            ("\u{f77e}", "Multicolor & COLRv1 Kleurmodus — Originele verlopen"),
        ];
        Grid::new("icon_color_modes_grid_4col")
            .num_columns(4)
            .spacing([GRID_SPACING_DP * 2.0, GRID_SPACING_DP * 2.0])
            .show(ui, |g_ui| {
                for (idx, (ic, tip)) in CM_TILES.into_iter().enumerate() {
                    let (_, mode) = SymbolColorRenderingMode::preset_by_index(idx);
                    let on = std::mem::discriminant(&st.icon_studio.color_mode)
                        == std::mem::discriminant(&mode);
                    if icon_tile_toggle(g_ui, on, ic, tip) {
                        st.set_symbol_color_rendering_mode(mode);
                    }
                }
                g_ui.end_row();
            });
    }

    #[rustfmt::skip]
    fn render_selected_sublayer_stroke_and_halo(ui: &mut Ui, st: &mut GrafiaCanvasState, sel_idx: usize, cur_ko: f64) {
        if let Some(cid) = st.color_sublayers.get(sel_idx).and_then(|s| s.contours.first().map(|c| c.id)) {
            let cur_w = st.contour_strokes.get(&cid).map_or(0.0, |s| s.width);
            if ui.selectable_label(cur_w > 0.0, format!("Streek {cur_w:.0}")).clicked() {
                let next_w = STROKE_CYCLE_WIDTHS_UPM.iter().copied().find(|&w| w > cur_w + 1.0).unwrap_or(0.0);
                st.set_contour_stroke(cid, ContourStrokeStyle {
                    width: next_w, height: next_w, position: StrokePosition::Center,
                    cap_start: LineCapStyle::Round, cap_end: LineCapStyle::Round, join: LineJoinStyle::Round,
                });
            }
        }
        let mut ko = cur_ko;
        if ui.add(Slider::new(&mut ko, 0.0..=KNOCKOUT_SLIDER_MAX_UPM).text("Halo")).changed() {
            let _ = st.set_selected_sublayer_knockout_margin(ko);
        }
    }

    #[rustfmt::skip]
    fn render_icon_geometry_and_optical_controls(&mut self, ui: &mut Ui, st: &mut GrafiaCanvasState) {
        let folds = st.icon_studio.radial_symmetry.fold_count;
        Grid::new("icon_geom_optical_grid_4col").num_columns(4).spacing([GRID_SPACING_DP * 2.0, GRID_SPACING_DP * 2.0]).show(ui, |g_ui| {
            let vw_empty = st.icon_studio.variable_width_strokes.is_empty();
            let (vw_ic, vw_tip) = if vw_empty { ("\u{f500}", "Variabele Lijndikte-Pinnen (+VarWidth)") } else { ("\u{f80c}", "Lijndikte Vastbakken (Bake Variable Width)") };
            if icon_tile(g_ui, vw_ic, vw_tip) {
                if vw_empty {
                    if let Some(c) = st.glyph.contours.last() { st.set_variable_width_stroke(Self::default_demo_var_width_spec(c.id)); }
                } else {
                    let _ = st.bake_variable_width_strokes();
                }
            }
            if icon_tile_toggle(g_ui, folds > 1, "\u{f544}", "Radiale Symmetrie-Rozet (1× / 4× / 6×)") {
                st.icon_studio.radial_symmetry.fold_count = if folds == 1 { 4 } else { 7 - folds.min(6) };
                st.icon_studio.radial_symmetry.center = DEMO_SYMMETRY_CENTER;
                st.bump_content_generation();
            }
            if icon_tile(g_ui, "\u{f7da}", "Optisch Oblique +12° [⌘⇧I]") { let _ = st.apply_optical_oblique(DEMO_OBLIQUE_DEG, true); }
            if icon_tile(g_ui, "\u{f6f4}", "5-Staps Icoon-Normalisatie (Clean)") { let _ = st.normalize_icon_contours(Some(1.0)); }
            g_ui.end_row();
            if icon_tile(g_ui, "\u{f77e}", "Vitrea Liquid Glass → Bake COLRv1") && st.bake_liquid_glass_to_colrv1() {
                st.view_mode = CanvasViewMode::PreviewFilled;
            }
            if icon_tile(g_ui, "\u{f6e5}", "2-Weg W3C / Apple SF Symbol SVG Roundtrip") {
                let svg = st.export_kinetic_symbol_to_svg();
                let _ = st.import_kinetic_symbol_from_svg(&svg);
            }
            if icon_tile(g_ui, "⊕", "Optische Balans-Uitlijning (Play-Optica ⊕)") { let _ = st.align_icon_to_optical_center(); }
            if folds > 1 && icon_tile(g_ui, "\u{f53d}", "Symmetrie-Rozet Vastbakken (Bake Symmetry)") { let _ = st.bake_radial_symmetry(); }
            g_ui.end_row();
        });
        ui.horizontal(|ui| self.render_crispness_24px_badge(ui, st));
    }

    fn render_crispness_24px_badge(&mut self, ui: &mut Ui, st: &GrafiaCanvasState) {
        let (gen, dark) = (st.content_generation(), st.theme.is_dark());
        let hit = self
            .crispness_badge_cache
            .as_ref()
            .is_some_and(|(g, d, _)| *g == gen && *d == dark);
        if !hit {
            let rgba = st.render_icon_crispness_preview_rgba(CRISPNESS_DIM_PX);
            let dim = CRISPNESS_DIM_PX as usize;
            let img = ColorImage::from_rgba_unmultiplied([dim, dim], &rgba);
            if let Some((g, d, tex)) = &mut self.crispness_badge_cache {
                tex.set(img, TextureOptions::NEAREST);
                (*g, *d) = (gen, dark);
            } else {
                let tex =
                    ui.ctx()
                        .load_texture("icon_crispness_24px", img, TextureOptions::NEAREST);
                self.crispness_badge_cache = Some((gen, dark, tex));
            }
        }
        if let Some((_, _, tex)) = &self.crispness_badge_cache {
            ui.label("24px:");
            ui.add(Image::from_texture((
                tex.id(),
                Vec2::splat(CRISPNESS_DISPLAY_DP),
            )));
        }
    }
}
