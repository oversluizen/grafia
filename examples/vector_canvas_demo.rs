#![cfg_attr(not(test), windows_subsystem = "windows")]

//! Interactive Grafia Vector Canvas & Hexagonal Host-Port ("Stekkers") Demonstrator.
//! Wires `set_transform_grid_anchor`, `set_metrics_origin`, `align_selection`,
//! `connect_selected_open_endpoints`, `add_inflection_points`, `correct_path_direction`,
//! `rotate_by_degrees`, `scale_by_factor`, and `slant_by_degrees`.

pub mod demo_host_shell;

pub use demo_host_shell::toolbar_specs::{
    configure_demo_fonts, install_crash_logger, ALL_ACTION_ICONS, APP_TITLE, DEMO_GUIDE_X_UPM,
    DEMO_GUIDE_Y_UPM, METRICS_ORIGIN_SPECS, MIN_WINDOW_HEIGHT_DP, MIN_WINDOW_WIDTH_DP,
    ORIGIN_ANCHOR_GRID_ROWS, TOOL_ICON_SPECS, WINDOW_HEIGHT_DP, WINDOW_WIDTH_DP,
};
use demo_host_shell::{
    demo_panels::{render_left_sidebar, render_top_toolbar},
    MockFontHost,
};
use eframe::egui::{self, CentralPanel, Panel, Separator};
use grafia::adapters::{CanvasViewMode, GrafiaCanvasState, GrafiaCanvasWidget};
use grafia::tokens::GrafiaTheme;

/// View mode icon and tooltip table covering all 3 `CanvasViewMode` projections.
pub const VIEW_MODE_SPECS: [(CanvasViewMode, egui::Key, &str, &str); 3] = [
    (
        CanvasViewMode::EditWireframe,
        egui::Key::Num1,
        "\u{f7f8}",
        "Edit Contour View [1] — Bezier nodes, handles, metrics & guides",
    ),
    (
        CanvasViewMode::PreviewFilled,
        egui::Key::Num2,
        "\u{f7ff}",
        "Preview Filled View [2] — Filled glyph counter preview",
    ),
    (
        CanvasViewMode::ShapeThumbnail,
        egui::Key::Num3,
        "\u{f80d}",
        "Shape Thumbnail View [3] — Fit-to-viewport silhouette",
    ),
];

/// Top-level demonstrator application embedding `GrafiaCanvasWidget` and `MockFontHost`.
pub struct GrafiaVectorCanvasDemo {
    pub state: GrafiaCanvasState,
    pub theme: GrafiaTheme,
    pub show_preview_panel: bool,
    pub host: MockFontHost,
}

impl Default for GrafiaVectorCanvasDemo {
    fn default() -> Self {
        Self::new()
    }
}

impl GrafiaVectorCanvasDemo {
    #[must_use]
    pub fn new() -> Self {
        Self {
            state: GrafiaCanvasState::init_glyph_a(),
            theme: GrafiaTheme::sonoma_default(),
            show_preview_panel: true,
            host: MockFontHost::new(),
        }
    }

    /// Renders the complete interactive Grafia Vector Canvas Demo UI inside `ui`.
    pub fn render_ui(&mut self, ui: &mut egui::Ui) {
        self.host.poll_host_ports(&mut self.state);
        for (mode, key, _, _) in VIEW_MODE_SPECS {
            if ui.input(|i| i.key_pressed(key)) {
                self.state.view_mode = mode;
            }
        }

        Panel::top("top_toolbar").show(ui, |ui| {
            render_top_toolbar(
                ui,
                &mut self.state,
                &mut self.theme,
                &mut self.host,
                &mut self.show_preview_panel,
            );
            ui.add(Separator::default().horizontal());
            self.host
                .render_preset_bar(ui, &mut self.state, &mut self.theme);
        });

        Panel::bottom("status_bar").show(ui, |ui| {
            ui.horizontal(|ui| {
                let tool_str = TOOL_ICON_SPECS
                    .iter()
                    .find(|(t, _, _)| *t == self.state.active_tool)
                    .map_or("", |(_, _, tip)| *tip);
                ui.label(tool_str);
                ui.add(Separator::default().vertical());
                let total_nodes: usize = self.state.contours.iter().map(Vec::len).sum();
                ui.label(format!(
                    "Contours: {} | Nodes: {total_nodes} (Selected: {}) | Anchors: {} | Components: {}",
                    self.state.contours.len(),
                    self.state.selected_nodes.len(),
                    self.state.anchors.len(),
                    self.state.components.len()
                ));
            });
        });

        Panel::left("shapes_sidebar").show(ui, |ui| {
            render_left_sidebar(ui, &mut self.state);
        });

        Panel::right("host_inspector_panel").show(ui, |ui| {
            self.host.render_host_inspector(ui, &mut self.state);
        });
        if self.show_preview_panel {
            Panel::bottom("glyph_preview_panel").show(ui, |ui| {
                self.host.render_preview_strip_panel(ui, &self.state);
            });
        }
        CentralPanel::default().show(ui, |ui| {
            GrafiaCanvasWidget::new(&mut self.state, &self.theme)
                .with_size(ui.available_size())
                .show(ui);
        });
        self.host.poll_host_ports(&mut self.state);
    }
}

impl eframe::App for GrafiaVectorCanvasDemo {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        self.render_ui(ui);
    }
}

/// Builds deterministic `eframe::NativeOptions` resilient against hybrid-GPU + virtual-adapter `wgpu` TDRs.
#[must_use]
pub fn build_native_options() -> eframe::NativeOptions {
    let renderer = match std::env::var("GRAFIA_RENDERER").as_deref() {
        Ok("wgpu") => eframe::Renderer::Wgpu,
        _ => eframe::Renderer::Glow,
    };
    let mut wgpu_setup = eframe::egui_wgpu::WgpuSetupCreateNew::without_display_handle();
    wgpu_setup.instance_descriptor.flags = eframe::wgpu::InstanceFlags::empty();
    let viewport = egui::ViewportBuilder::default()
        .with_inner_size([WINDOW_WIDTH_DP, WINDOW_HEIGHT_DP])
        .with_min_inner_size([MIN_WINDOW_WIDTH_DP, MIN_WINDOW_HEIGHT_DP])
        .with_active(true)
        .with_visible(true)
        .with_title(APP_TITLE);
    let wgpu_options = eframe::egui_wgpu::WgpuConfiguration {
        wgpu_setup: eframe::egui_wgpu::WgpuSetup::CreateNew(wgpu_setup),
        ..Default::default()
    };
    eframe::NativeOptions {
        viewport,
        renderer,
        wgpu_options,
        system_font_fallback: false,
        persist_window: false,
        ..Default::default()
    }
}

fn main() -> eframe::Result<()> {
    install_crash_logger();
    eframe::run_native(
        APP_TITLE,
        build_native_options(),
        Box::new(|cc| {
            configure_demo_fonts(&cc.egui_ctx);
            Ok(Box::new(GrafiaVectorCanvasDemo::new()))
        }),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use demo_host_shell::DemoPreset;
    use grafia::core::FontPoint;

    const TEST_ICON_FONT_SIZE_PT: f32 = 14.0;

    #[test]
    fn test_native_options_prevent_wgpu_tdr_and_system_font_main_thread_hang() {
        let options = build_native_options();
        assert_eq!(options.renderer, eframe::Renderer::Glow);
        assert!(!options.system_font_fallback);
        assert!(!options.persist_window);
    }

    #[test]
    fn test_vector_canvas_demo_all_archetypes_modes_tools_and_host_stekkers() {
        let ctx = egui::Context::default();
        configure_demo_fonts(&ctx);
        let mut app = GrafiaVectorCanvasDemo::new();
        let screen_rect = egui::Rect::from_min_size(
            egui::Pos2::ZERO,
            egui::Vec2::new(WINDOW_WIDTH_DP, WINDOW_HEIGHT_DP),
        );
        app.state
            .add_guide_from_selection(false, FontPoint::new(DEMO_GUIDE_X_UPM, DEMO_GUIDE_Y_UPM));

        for (preset, _) in DemoPreset::ALL {
            app.host
                .apply_preset(preset, &mut app.state, &mut app.theme);
            let raw_input = egui::RawInput {
                screen_rect: Some(screen_rect),
                ..Default::default()
            };
            let mut full_output = ctx.run_ui(raw_input, |ui| app.render_ui(ui));
            full_output.textures_delta.clear();
            assert!(!ctx
                .tessellate(full_output.shapes, full_output.pixels_per_point)
                .is_empty());
        }

        // Verify zero redundant preview strip rasterization on idle frame
        let renders_before_idle = app.host.preview_cache.render_count();
        let raw_idle = egui::RawInput {
            screen_rect: Some(screen_rect),
            ..Default::default()
        };
        let mut idle_output = ctx.run_ui(raw_idle, |ui| app.render_ui(ui));
        idle_output.textures_delta.clear();
        assert_eq!(app.host.preview_cache.render_count(), renders_before_idle);

        // Exercise TextStreamEditRequest + ResetDefaultAnchors + OpenComponentSource + SpacingDelta
        app.host
            .apply_preset(DemoPreset::TextStreamAova, &mut app.state, &mut app.theme);
        app.state.insert_text_at_caret("S");
        app.host.poll_host_ports(&mut app.state);
        assert_eq!(app.host.text_buffer, "AOSVA");
        app.state.delete_text_backward_at_caret();
        app.host.poll_host_ports(&mut app.state);
        assert_eq!(app.host.text_buffer, "AOVA");
        assert!(app
            .state
            .apply_selection_info_action(grafia::core::SelectionInfoAction::SetLsb(55.0)));
        app.host.poll_host_ports(&mut app.state);
        assert!(app
            .host
            .event_log
            .back()
            .is_some_and(|l| l.contains("SpacingDelta")));
        app.state = GrafiaCanvasState::init_glyph_a();
        app.host.apply_preset(
            DemoPreset::ComponentAndDecompose,
            &mut app.state,
            &mut app.theme,
        );
        assert!(app
            .state
            .double_click_component_at_upm(FontPoint::new(350.0, 620.0)));
        app.host.poll_host_ports(&mut app.state);
        assert_eq!(app.state.glyph.name, "acutecomb");
        assert!(app.state.validate_invariants().is_ok());
        assert!(app.state.undo());
        assert_eq!(app.state.glyph.name, "A");
    }

    #[test]
    fn test_configure_demo_fonts_registers_cupertino_icons_and_shapes_all_symbols() {
        assert_eq!(
            ORIGIN_ANCHOR_GRID_ROWS.map(|row| row.map(|r| r.0)),
            [[6, 7, 8], [3, 4, 5], [0, 1, 2]]
        );

        let ctx = egui::Context::default();
        configure_demo_fonts(&ctx);
        let mut out = ctx.run_ui(egui::RawInput::default(), |ui| {
            let defs = ui.ctx().fonts_mut(|f| f.definitions().clone());
            assert_eq!(
                defs.families[&egui::FontFamily::Proportional][0].as_str(),
                egui_macview::CUPERTINO_ICONS_FONT_NAME
            );
            assert_eq!(
                defs.families[&egui::FontFamily::Monospace][0].as_str(),
                egui_macview::CUPERTINO_ICONS_FONT_NAME
            );
            assert_eq!(METRICS_ORIGIN_SPECS.map(|s| s.1), ["Base", "xH/2", "Cap"]);
            let font_id = egui::FontId::proportional(TEST_ICON_FONT_SIZE_PT);
            let tofu_uv = ui
                .painter()
                .layout_no_wrap("\u{fffe}".to_owned(), font_id.clone(), egui::Color32::WHITE)
                .rows[0]
                .glyphs[0]
                .uv_rect;
            for ch in ('0'..='9').chain('A'..='Z').chain('a'..='z') {
                let uv = ui
                    .painter()
                    .layout_no_wrap(ch.to_string(), font_id.clone(), egui::Color32::WHITE)
                    .rows[0]
                    .glyphs[0]
                    .uv_rect;
                assert_ne!(uv, tofu_uv);
            }
            let all_icons = TOOL_ICON_SPECS
                .iter()
                .map(|s| s.1)
                .chain(VIEW_MODE_SPECS.iter().map(|s| s.2))
                .chain(ORIGIN_ANCHOR_GRID_ROWS.iter().flatten().map(|r| r.1))
                .chain(egui_macview::MacSymbol::ALL.iter().map(|s| s.as_str()))
                .chain(ALL_ACTION_ICONS);
            for icon in all_icons {
                let galley = ui.painter().layout_no_wrap(
                    icon.to_owned(),
                    font_id.clone(),
                    egui::Color32::WHITE,
                );
                assert!(galley.size().x > 0.0 && galley.size().y > 0.0);
                assert_ne!(galley.rows[0].glyphs[0].uv_rect, tofu_uv);
            }
        });
        out.textures_delta.clear();
    }
}
