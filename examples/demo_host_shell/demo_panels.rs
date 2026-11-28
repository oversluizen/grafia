//! Left Liquid Glass Studio Sidebar (`CONTOUREN & LAGEN`, `TRANSFORMATIE 3×3`, `PAD-OPERATIES`,
//! and in `IconStudio`: `KLEURLAGEN & HIËRARCHIE` + `SYMMETRIE & OPTICA`, `SPEC-018 §3.7`).

pub use super::sonoma_top_bar::render_top_toolbar;

use super::sonoma_top_bar::{liquid_glass_capsule_frame, section_header_text, StudioWorkspaceMode};
use super::toolbar_specs::{
    BOTTOM_WATERFALL_SPLIT_RATIO, COMPACT_PHONE_BREAKPOINT_DP, DEMO_GUIDE_X_UPM, DEMO_GUIDE_Y_UPM,
    DEMO_ROTATE_STEP_DEG, DEMO_SCALE_UP_FACTOR, DEMO_SLANT_STEP_DEG, GLASS_CAPSULE_PAD_X_F32,
    GRID_SPACING_DP, INFO_GRID_SPACING_X_DP, METRICS_ORIGIN_SPECS, ORIGIN_ANCHOR_GRID_ROWS,
    PREVIEW_STRIP_MAX_WIDTH_DP, PREVIEW_STRIP_MIN_HEIGHT_DP, PREVIEW_STRIP_MIN_WIDTH_DP,
    SIDEBAR_TOP_SPACE_DP,
};
use super::MockFontHost;
use egui::{Grid, Layout, ScrollArea, Separator, Ui, Vec2};
use egui_macview::{MacSegmentedControl, MacSymbol, MacTheme};
use grafia::adapters::GrafiaCanvasState;
use grafia::core::{FontPoint, ShapeBooleanRole, TransformGridAnchor, TransformOriginSpec};
use grafia::tokens::GrafiaTheme;

fn tip_btn(ui: &mut Ui, icon: &str, tip: &str) -> bool {
    ui.button(icon).on_hover_text(tip).clicked()
}

fn tip_toggle(ui: &mut Ui, active: bool, icon: &str, tip: &str) -> bool {
    ui.selectable_label(active, icon)
        .on_hover_text(tip)
        .clicked()
}

pub fn render_left_studio_sidebar(
    ui: &mut Ui,
    host: &mut MockFontHost,
    state: &mut GrafiaCanvasState,
    _theme: &mut GrafiaTheme,
) {
    let tm = host.active_mac_theme();
    ScrollArea::vertical()
        .id_salt("left_studio_sidebar_scroll")
        .show(ui, |ui| {
            ui.add_space(SIDEBAR_TOP_SPACE_DP);
            if host.studio_mode == StudioWorkspaceMode::IconStudio {
                liquid_glass_capsule_frame(&tm).show(ui, |ui| {
                    ui.label(section_header_text("KLEURLAGEN & SYMMETRIE", &tm));
                    host.render_icon_studio_and_kinetic_bar(ui, state);
                });
                ui.add_space(SIDEBAR_TOP_SPACE_DP);
            }
            render_left_sidebar_with_theme(ui, state, &tm);
        });
}

pub fn render_left_sidebar(ui: &mut Ui, state: &mut GrafiaCanvasState) {
    let tm = MacTheme::default();
    render_left_sidebar_with_theme(ui, state, &tm);
}

fn render_left_sidebar_with_theme(ui: &mut Ui, state: &mut GrafiaCanvasState, tm: &MacTheme) {
    liquid_glass_capsule_frame(tm).show(ui, |ui| {
        render_left_sidebar_shapes(ui, state, tm);
    });
    ui.add_space(SIDEBAR_TOP_SPACE_DP);
    liquid_glass_capsule_frame(tm).show(ui, |ui| {
        render_left_sidebar_transform(ui, state, tm);
    });
    ui.add_space(SIDEBAR_TOP_SPACE_DP);
    liquid_glass_capsule_frame(tm).show(ui, |ui| {
        render_left_sidebar_path_ops(ui, state, tm);
    });
}

fn render_left_sidebar_shapes(ui: &mut Ui, state: &mut GrafiaCanvasState, tm: &MacTheme) {
    ui.label(section_header_text(
        &format!("CONTOUREN & LAGEN ({})", state.glyph.contours.len()),
        tm,
    ));
    for contour in &state.glyph.contours {
        ui.push_id(contour.id.0, |ui| {
            ui.horizontal(|ui| {
                let locator_clicked = ui
                    .add(grafia::locator::ShapeLocatorWidget::new(
                        &state.glyph,
                        contour.id,
                    ))
                    .on_hover_text("Selecteer contour")
                    .clicked();
                let is_selected = !contour.nodes.is_empty()
                    && contour
                        .nodes
                        .iter()
                        .any(|n| state.selected_nodes.contains(&n.id));
                let role_str = match contour.role {
                    ShapeBooleanRole::Normal => "Vulling",
                    ShapeBooleanRole::Mask => "Uitsparing",
                    ShapeBooleanRole::Add => "Samenvoegen",
                    ShapeBooleanRole::Intersect => "Doorsnede",
                };
                let winding_str = if contour.closed {
                    grafia::locator::contour_winding_label(&contour.nodes)
                } else {
                    "Open"
                };
                let label_text = format!(
                    "Pad {} · {} pt ({role_str}, {winding_str})",
                    contour.id.0 + 1,
                    contour.nodes.len()
                );
                if locator_clicked
                    || tip_toggle(ui, is_selected, &label_text, "Selecteer contour-knopen")
                {
                    state.selected_nodes = contour.nodes.iter().map(|n| n.id).collect();
                }
            });
        });
    }
}

fn render_left_sidebar_transform(ui: &mut Ui, state: &mut GrafiaCanvasState, tm: &MacTheme) {
    ui.label(section_header_text("TRANSFORMATIE & OORSPRONG (3×3)", tm));
    ui.horizontal(|ui| {
        Grid::new("origin_anchor_grid")
            .spacing([GRID_SPACING_DP, GRID_SPACING_DP])
            .show(ui, |g_ui| {
                for row in ORIGIN_ANCHOR_GRID_ROWS {
                    for (idx, icon, tip) in row {
                        let anchor = TransformGridAnchor::from_index(idx);
                        let active = state.transform_origin_mode
                            == TransformOriginSpec::BoundingBoxOrigin(anchor);
                        if tip_toggle(g_ui, active, icon, tip) {
                            state.set_transform_grid_anchor(anchor);
                        }
                    }
                    g_ui.end_row();
                }
            });
        ui.add(Separator::default().vertical());
        ui.vertical(|v_ui| {
            v_ui.style_mut().wrap_mode = Some(egui::TextWrapMode::Extend);
            for (m, lbl, tip) in METRICS_ORIGIN_SPECS {
                let active = state.transform_origin_mode == TransformOriginSpec::MetricsOrigin(m);
                if tip_toggle(v_ui, active, lbl, tip) {
                    state.set_metrics_origin(m);
                }
            }
        });
    });
    ui.horizontal(|ui| {
        if tip_btn(ui, "\u{f7c5}", "Roteer -15° CCW") {
            state.rotate_by_degrees(-DEMO_ROTATE_STEP_DEG);
        }
        if tip_btn(ui, "\u{f7c7}", "Roteer +15° CW") {
            state.rotate_by_degrees(DEMO_ROTATE_STEP_DEG);
        }
        if tip_btn(ui, "\u{f500}", "Spiegel Horizontaal") {
            state.flip_horizontal();
        }
        if tip_btn(ui, "\u{f52a}", "Spiegel Verticaal") {
            state.flip_vertical();
        }
        if tip_btn(ui, "\u{f386}", "Schaal 110%") {
            state.scale_by_factor(DEMO_SCALE_UP_FACTOR, DEMO_SCALE_UP_FACTOR);
        }
        if tip_btn(ui, "\u{f7da}", "Cursivering +12°") {
            state.slant_by_degrees(DEMO_SLANT_STEP_DEG, true);
        }
    });
}

fn render_left_sidebar_path_ops(ui: &mut Ui, state: &mut GrafiaCanvasState, tm: &MacTheme) {
    ui.label(section_header_text("PAD-CHIRURGIE & HULPLIJNEN", tm));
    let g_pt = FontPoint::new(DEMO_GUIDE_X_UPM, DEMO_GUIDE_Y_UPM);
    #[rustfmt::skip]
    const PATH_OPS: [(&str, &str); 11] = [
        ("\u{f4e3}", "Keer Padrichting Om"), ("\u{f53d}", "Maak Knoop Startpunt"),
        ("\u{f6f4}", "Corrigeer Padrichting [Cmd+Shift+R]"), ("\u{f48a}", "Voeg Extrema Toe"),
        ("\u{f899}", "Voeg Buigpunten Toe"), ("\u{f892}", "Ruim Paden Op [Cmd+Opt+Shift+T]"),
        ("\u{f80a}", "Lijn Selectie Uit [Cmd+Shift+A]"), ("\u{f6e5}", "Verbind Open Eindpunten"),
        ("\u{f80c}", "Verwijder Overlap [Cmd+Shift+O]"), (MacSymbol::Plus.as_str(), "Lokale Hulplijn Toevoegen"),
        ("\u{f77e}", "Globale Master-Hulplijn Toevoegen"),
    ];
    ui.horizontal_wrapped(|ui| {
        for (i, (ic, tip)) in PATH_OPS.into_iter().enumerate() {
            if tip_btn(ui, ic, tip) {
                match i {
                    0 => {
                        let _ = state.reverse_selected_or_all_contours();
                    }
                    1 => {
                        let _ = state.make_node_first();
                    }
                    2 => {
                        let _ = state.correct_path_direction();
                    }
                    3 => {
                        let _ = state.add_extremes_action();
                    }
                    4 => {
                        let _ = state.add_inflection_points();
                    }
                    5 => {
                        let _ = state.tidy_up_paths();
                    }
                    6 => {
                        let _ = state.align_selection();
                    }
                    7 => {
                        let _ = state.connect_selected_open_endpoints();
                    }
                    8 => {
                        let _ = state.remove_overlap();
                    }
                    9 => {
                        let _ = state.add_guide_from_selection(false, g_pt);
                    }
                    _ => {
                        let _ = state.add_guide_from_selection(true, g_pt);
                    }
                }
            }
        }
        if tip_toggle(
            ui,
            state.show_guides,
            "\u{f424}",
            "Toon / Verberg Hulplijnen",
        ) {
            state.show_guides = !state.show_guides;
        }
        if tip_toggle(ui, state.lock_guides, "\u{f4c8}", "Vergrendel Hulplijnen") {
            state.lock_guides = !state.lock_guides;
        }
    });
}

impl MockFontHost {
    pub fn render_preview_strip_panel(&mut self, ui: &mut Ui, state: &mut GrafiaCanvasState) {
        let tm = self.active_mac_theme();
        let total_w = ui.available_width();
        let pad_w = GLASS_CAPSULE_PAD_X_F32 * 2.0;
        let clamp_w = |w: f32| {
            w.round()
                .clamp(PREVIEW_STRIP_MIN_WIDTH_DP, PREVIEW_STRIP_MAX_WIDTH_DP) as u32
        };
        if total_w < COMPACT_PHONE_BREAKPOINT_DP {
            liquid_glass_capsule_frame(&tm).show(ui, |ui| {
                self.render_mobile_tabbed_bottom_drawer(ui, state, &tm, clamp_w(total_w - pad_w));
            });
            return;
        }
        if self.show_right_inspector {
            liquid_glass_capsule_frame(&tm).show(ui, |g_ui| {
                self.render_waterfall_and_kinetic_card(g_ui, state, &tm, clamp_w(total_w - pad_w));
            });
            return;
        }
        let left_w = (total_w * BOTTOM_WATERFALL_SPLIT_RATIO).max(PREVIEW_STRIP_MIN_WIDTH_DP);
        let right_w = (total_w - left_w - INFO_GRID_SPACING_X_DP).max(PREVIEW_STRIP_MIN_WIDTH_DP);
        let card_h = (self.preview_panel_height_dp - INFO_GRID_SPACING_X_DP)
            .max(PREVIEW_STRIP_MIN_HEIGHT_DP);
        ui.horizontal_top(|ui| {
            ui.allocate_ui_with_layout(
                Vec2::new(left_w, card_h),
                Layout::top_down(egui::Align::Min),
                |l_ui| {
                    liquid_glass_capsule_frame(&tm).show(l_ui, |g_ui| {
                        self.render_waterfall_and_kinetic_card(
                            g_ui,
                            state,
                            &tm,
                            clamp_w(left_w - pad_w),
                        );
                    });
                },
            );
            ui.allocate_ui_with_layout(
                Vec2::new(right_w, card_h),
                Layout::top_down(egui::Align::Min),
                |r_ui| {
                    liquid_glass_capsule_frame(&tm).show(r_ui, |g_ui| {
                        self.render_structured_info_card(g_ui, state, &tm);
                    });
                },
            );
        });
    }

    fn render_mobile_tabbed_bottom_drawer(
        &mut self,
        ui: &mut Ui,
        state: &mut GrafiaCanvasState,
        tm: &MacTheme,
        target_w_px: u32,
    ) {
        ui.horizontal(|ui| {
            let tab_idx = usize::from(self.mobile_bottom_tab_info);
            MacSegmentedControl::new(&["\u{f7ff} Preview", "ℹ Info"], tab_idx)
                .id_salt("mobile_bottom_drawer_tab")
                .theme(tm)
                .show(ui, |idx| self.mobile_bottom_tab_info = idx == 1);
            ui.with_layout(Layout::right_to_left(egui::Align::Center), |r_ui| {
                self.render_preview_size_presets(r_ui);
            });
        });
        if self.mobile_bottom_tab_info {
            self.render_structured_info_card(ui, state, tm);
        } else {
            self.render_waterfall_texture(ui, state, target_w_px);
        }
    }
}
