//! `SPEC-017 §2.9` & `SPEC-018` Icon Studio Vector-, Kleurlagen- & Kinetische Animatie-Werkbank UI.

use super::icon_studio_showcase::{
    DEMO_DT_MAX_SEC, DEMO_DT_MIN_SEC, DEMO_OBLIQUE_DEG, DEMO_SYMMETRY_CENTER,
};
use super::{DemoPreset, MockFontHost, ShowcaseChapter};
use egui::{ColorImage, Image, RichText, ScrollArea, Slider, TextureOptions, Ui, Vec2};
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
        ScrollArea::horizontal()
            .id_salt("icon_studio_row1_scroll")
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    self.render_icon_templates_and_sublayers(ui, st);
                    ui.separator();
                    self.render_sublayer_style_and_color_modes(ui, st);
                });
            });
        ui.separator();
        ScrollArea::horizontal()
            .id_salt("icon_studio_row2_scroll")
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    self.render_icon_geometry_and_optical_controls(ui, st);
                    ui.separator();
                    self.render_kinetic_timeline_controls(ui, st);
                });
            });
    }

    fn render_icon_templates_and_sublayers(&mut self, ui: &mut Ui, st: &mut GrafiaCanvasState) {
        if ui.button(RichText::new("+ Nieuw Icoon").strong()).clicked() {
            self.apply_blank_icon_artboard_preset(st);
        }
        for (lbl, preset) in [
            ("Signaal", DemoPreset::KineticWifiBounceAndColorModes),
            ("Knockout-Slash", DemoPreset::KineticMicrophoneKnockoutHalo),
            (
                "Vlakken & VarWidth",
                DemoPreset::IconPlanarShapeBuilderMerge,
            ),
            ("6x Rozet", DemoPreset::IconRadialSymmetryAndOblique),
        ] {
            if ui.button(lbl).clicked() {
                self.apply_icon_studio_preset(preset, st);
            }
        }
        if !st.color_sublayers.is_empty() {
            ui.separator();
            let sel = st.selected_color_sublayer_index();
            let sub_names: Vec<(usize, String)> = st
                .color_sublayers
                .iter()
                .enumerate()
                .map(|(i, s)| (i, format!("{}: {}", i + 1, s.name)))
                .collect();
            for (idx, lbl) in sub_names {
                if ui.selectable_label(sel == Some(idx), lbl).clicked() {
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
                for (lbl, d) in [("Z-", -1), ("Z+", 1)] {
                    if ui.button(lbl).clicked() {
                        let _ = st.reorder_selected_color_sublayer(d);
                    }
                }
            }
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
            for (lvl, lbl) in [
                (SymbolHierarchyLevel::Primary, "1°"),
                (SymbolHierarchyLevel::Secondary, "2°"),
                (SymbolHierarchyLevel::Tertiary, "3°"),
            ] {
                if ui
                    .selectable_label(cur_tag.hierarchy_level == lvl, lbl)
                    .clicked()
                {
                    let _ = st.set_selected_sublayer_hierarchy_level(lvl);
                }
            }
            let grp = cur_tag.layer_group.clamp(1, 3);
            if ui.button(format!("Grp {grp}")).clicked() {
                if let Some(t) = st.icon_studio.symbol_layer_tags.get_mut(sel_idx) {
                    t.layer_group = (grp % 3) + 1;
                    st.bump_content_generation();
                }
            }
            let (dir_lbl, next_dir) = match cur_tag.draw_direction {
                DrawDirection::StartToEnd => ("Start→Eind", DrawDirection::CenterOut),
                DrawDirection::CenterOut => ("Midden↔Uit", DrawDirection::EndToStart),
                DrawDirection::EndToStart => ("Eind→Start", DrawDirection::StartToEnd),
            };
            if ui.button(dir_lbl).clicked() {
                let _ = st.set_selected_sublayer_draw_direction(next_dir);
            }
            Self::render_selected_sublayer_stroke_and_halo(
                ui,
                st,
                sel_idx,
                cur_tag.knockout_margin_upm,
            );
            ui.separator();
        }
        for idx in 0..4 {
            let (lbl, mode) = SymbolColorRenderingMode::preset_by_index(idx);
            let on =
                std::mem::discriminant(&st.icon_studio.color_mode) == std::mem::discriminant(&mode);
            if ui.selectable_label(on, lbl).clicked() {
                st.set_symbol_color_rendering_mode(mode);
            }
        }
    }

    fn render_selected_sublayer_stroke_and_halo(
        ui: &mut Ui,
        st: &mut GrafiaCanvasState,
        sel_idx: usize,
        cur_ko: f64,
    ) {
        if let Some(cid) = st
            .color_sublayers
            .get(sel_idx)
            .and_then(|s| s.contours.first().map(|c| c.id))
        {
            let cur_w = st.contour_strokes.get(&cid).map_or(0.0, |s| s.width);
            if ui
                .selectable_label(cur_w > 0.0, format!("Streek {cur_w:.0}"))
                .clicked()
            {
                let next_w = STROKE_CYCLE_WIDTHS_UPM
                    .iter()
                    .copied()
                    .find(|&w| w > cur_w + 1.0)
                    .unwrap_or(0.0);
                st.set_contour_stroke(
                    cid,
                    ContourStrokeStyle {
                        width: next_w,
                        height: next_w,
                        position: StrokePosition::Center,
                        cap_start: LineCapStyle::Round,
                        cap_end: LineCapStyle::Round,
                        join: LineJoinStyle::Round,
                    },
                );
            }
        }
        let mut ko = cur_ko;
        if ui
            .add(Slider::new(&mut ko, 0.0..=KNOCKOUT_SLIDER_MAX_UPM).text("Halo"))
            .changed()
        {
            let _ = st.set_selected_sublayer_knockout_margin(ko);
        }
    }

    fn render_icon_geometry_and_optical_controls(
        &mut self,
        ui: &mut Ui,
        st: &mut GrafiaCanvasState,
    ) {
        if st.icon_studio.variable_width_strokes.is_empty() {
            if ui.button("+ VarWidth").clicked() {
                if let Some(c) = st.glyph.contours.last() {
                    st.set_variable_width_stroke(Self::default_demo_var_width_spec(c.id));
                }
            }
        } else if ui.button("Bake Width").clicked() {
            let _ = st.bake_variable_width_strokes();
        }
        let folds = st.icon_studio.radial_symmetry.fold_count;
        if ui
            .selectable_label(folds > 1, format!("Sym: {folds}x"))
            .clicked()
        {
            st.icon_studio.radial_symmetry.fold_count =
                if folds == 1 { 4 } else { 7 - folds.min(6) };
            st.icon_studio.radial_symmetry.center = DEMO_SYMMETRY_CENTER;
            st.bump_content_generation();
        }
        if folds > 1 && ui.button("Bake Sym").clicked() {
            let _ = st.bake_radial_symmetry();
        }
        if ui.button("Oblique +12°").clicked() {
            let _ = st.apply_optical_oblique(DEMO_OBLIQUE_DEG, true);
        }
        if ui.button("Clean 5-Staps").clicked() {
            let _ = st.normalize_icon_contours(Some(1.0));
        }
        if ui.button("Bake COLRv1").clicked() && st.bake_liquid_glass_to_colrv1() {
            st.view_mode = CanvasViewMode::PreviewFilled;
        }
        if ui.button("SVG Roundtrip").clicked() {
            let svg = st.export_kinetic_symbol_to_svg();
            let _ = st.import_kinetic_symbol_from_svg(&svg);
        }
        if let Some(rep) = st.compute_active_icon_optical_balance() {
            let d = rep.delta_upm.x.0.hypot(rep.delta_upm.y.0);
            if ui.button(format!("Optisch ⊕ (Δ{d:.1})")).clicked() {
                let _ = st.align_icon_to_optical_center();
            }
        }
        self.render_crispness_24px_badge(ui, st);
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
