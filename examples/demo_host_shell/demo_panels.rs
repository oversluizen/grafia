//! Left Studio Navigator (`36-Case 6×6 Matrix`), `ShapeLocatorWidget`, Transform & Path Operations Sidebar Panel (`SPEC-018 §1.2`).

pub use super::sonoma_top_bar::render_top_toolbar;

use super::toolbar_specs::{
    DEMO_GUIDE_X_UPM, DEMO_GUIDE_Y_UPM, DEMO_ROTATE_STEP_DEG, DEMO_SCALE_UP_FACTOR,
    DEMO_SLANT_STEP_DEG, GRID_SPACING_DP, METRICS_ORIGIN_SPECS, ORIGIN_ANCHOR_GRID_ROWS,
    SIDEBAR_TOP_SPACE_DP,
};
use super::MockFontHost;
use egui::{Grid, ScrollArea, Separator, Ui};
use egui_macview::MacSymbol;
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
    theme: &mut GrafiaTheme,
) {
    ScrollArea::vertical()
        .id_salt("left_studio_sidebar_scroll")
        .show(ui, |ui| {
            ui.add_space(SIDEBAR_TOP_SPACE_DP);
            host.render_keynote_36_case_navigator(ui, state, theme);
            ui.add(Separator::default().horizontal());
            render_left_sidebar(ui, state);
        });
}

pub fn render_left_sidebar(ui: &mut Ui, state: &mut GrafiaCanvasState) {
    ui.add_space(SIDEBAR_TOP_SPACE_DP);
    render_left_sidebar_shapes(ui, state);
    ui.add(Separator::default().horizontal());
    render_left_sidebar_transform(ui, state);
    ui.add(Separator::default().horizontal());
    render_left_sidebar_path_ops(ui, state);
}

fn render_left_sidebar_shapes(ui: &mut Ui, state: &mut GrafiaCanvasState) {
    ui.strong(format!("Shapes ({})", state.glyph.contours.len()));
    ui.add(Separator::default().horizontal());
    for contour in &state.glyph.contours {
        ui.push_id(contour.id.0, |ui| {
            ui.horizontal(|ui| {
                let locator_clicked = ui
                    .add(grafia::locator::ShapeLocatorWidget::new(
                        &state.glyph,
                        contour.id,
                    ))
                    .on_hover_text("Click to select contour")
                    .clicked();
                let is_selected = !contour.nodes.is_empty()
                    && contour
                        .nodes
                        .iter()
                        .any(|n| state.selected_nodes.contains(&n.id));
                let role_str = match contour.role {
                    ShapeBooleanRole::Normal => "Normal",
                    ShapeBooleanRole::Mask => "Mask",
                    ShapeBooleanRole::Add => "Add",
                    ShapeBooleanRole::Intersect => "Intersect",
                };
                let winding_str = if contour.closed {
                    grafia::locator::contour_winding_label(&contour.nodes)
                } else {
                    "Open"
                };
                let label_text = format!(
                    "Path {}\n{} pt • {role_str}, {winding_str}",
                    contour.id.0 + 1,
                    contour.nodes.len()
                );
                if locator_clicked
                    || tip_toggle(
                        ui,
                        is_selected,
                        &label_text,
                        "Click to select contour nodes",
                    )
                {
                    state.selected_nodes = contour.nodes.iter().map(|n| n.id).collect();
                }
            });
        });
    }
}

fn render_left_sidebar_transform(ui: &mut Ui, state: &mut GrafiaCanvasState) {
    ui.strong("Transform Panel");
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
        if tip_btn(
            ui,
            "\u{f7c5}",
            "Rotate -15° CCW around active Transform Origin",
        ) {
            state.rotate_by_degrees(-DEMO_ROTATE_STEP_DEG);
        }
        if tip_btn(
            ui,
            "\u{f7c7}",
            "Rotate +15° CW around active Transform Origin",
        ) {
            state.rotate_by_degrees(DEMO_ROTATE_STEP_DEG);
        }
        if tip_btn(
            ui,
            "\u{f500}",
            "Flip Horizontal across active Transform Origin",
        ) {
            state.flip_horizontal();
        }
        if tip_btn(
            ui,
            "\u{f52a}",
            "Flip Vertical across active Transform Origin",
        ) {
            state.flip_vertical();
        }
        if tip_btn(ui, "\u{f386}", "Scale 110% around active Transform Origin") {
            state.scale_by_factor(DEMO_SCALE_UP_FACTOR, DEMO_SCALE_UP_FACTOR);
        }
        if tip_btn(
            ui,
            "\u{f7da}",
            "Slant +12° Horizontal Shear around active Transform Origin",
        ) {
            state.slant_by_degrees(DEMO_SLANT_STEP_DEG, true);
        }
    });
}

fn render_left_sidebar_path_ops(ui: &mut Ui, state: &mut GrafiaCanvasState) {
    ui.strong("Path Operations & Guides");
    ui.horizontal_wrapped(|ui| {
        if tip_btn(ui, "\u{f4e3}", "Reverse Contours") {
            state.reverse_selected_or_all_contours();
        }
        if tip_btn(ui, "\u{f53d}", "Make Node First") {
            state.make_node_first();
        }
        if tip_btn(ui, "\u{f6f4}", "Correct Path Direction [Cmd+Shift+R]") {
            state.correct_path_direction();
        }
        if tip_btn(ui, "\u{f48a}", "Add Extremes") {
            state.add_extremes_action();
        }
        if tip_btn(ui, "\u{f899}", "Add Inflections") {
            state.add_inflection_points();
        }
        if tip_btn(ui, "\u{f892}", "Tidy Up Paths [Cmd+Opt+Shift+T]") {
            state.tidy_up_paths();
        }
        if tip_btn(ui, "\u{f80a}", "Align Selection [Cmd+Shift+A]") {
            state.align_selection();
        }
        if tip_btn(ui, "\u{f6e5}", "Connect Open Endpoints") {
            state.connect_selected_open_endpoints();
        }
        if tip_btn(ui, "\u{f80c}", "Remove Overlap [Cmd+Shift+O]") {
            state.remove_overlap();
        }
        let g_pt = FontPoint::new(DEMO_GUIDE_X_UPM, DEMO_GUIDE_Y_UPM);
        if tip_btn(ui, MacSymbol::Plus.as_str(), "Add Local Guide") {
            state.add_guide_from_selection(false, g_pt);
        }
        if tip_btn(ui, "\u{f77e}", "Add Global Master Guide") {
            state.add_guide_from_selection(true, g_pt);
        }
        if tip_toggle(
            ui,
            state.show_guides,
            "\u{f424}",
            "Show / Hide Guides [Cmd+Shift+L]",
        ) {
            state.show_guides = !state.show_guides;
        }
        if tip_toggle(ui, state.lock_guides, "\u{f4c8}", "Lock / Unlock Guides") {
            state.lock_guides = !state.lock_guides;
        }
    });
}
