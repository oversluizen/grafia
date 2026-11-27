#![cfg_attr(not(test), windows_subsystem = "windows")]

//! Interactive Grafia Vector Canvas & Hexagonal Host-Port ("Stekkers") Guided Showcase.

pub mod demo_host_shell;

pub use demo_host_shell::toolbar_specs::{
    configure_demo_fonts, fit_demo_canvas_viewport, install_crash_logger, ALL_ACTION_ICONS,
    APP_TITLE, DEMO_GUIDE_X_UPM, DEMO_GUIDE_Y_UPM, METRICS_ORIGIN_SPECS, MIN_WINDOW_HEIGHT_DP,
    MIN_WINDOW_WIDTH_DP, ORIGIN_ANCHOR_GRID_ROWS, TOOL_ICON_SPECS, WINDOW_HEIGHT_DP,
    WINDOW_WIDTH_DP,
};
use demo_host_shell::{
    demo_panels::{render_left_studio_sidebar, render_top_toolbar},
    MockFontHost,
};
use eframe::egui::{self, CentralPanel, Key as K, Panel, Separator};
use grafia::adapters::{
    CanvasViewMode, CanvasViewMode as M, GrafiaCanvasState, GrafiaCanvasWidget,
};
use grafia::tokens::GrafiaTheme;

/// View mode icon and tooltip table covering all 4 `CanvasViewMode` projections.
pub const VIEW_MODE_SPECS: [(CanvasViewMode, K, &str, &str); 4] = [
    (M::EditWireframe, K::Num1, "\u{f7f8}", "Edit [1]"),
    (M::PreviewFilled, K::Num2, "\u{f7ff}", "Filled [2]"),
    (M::ShapeThumbnail, K::Num3, "\u{f80d}", "Thumb [3]"),
    (M::LiquidGlass, K::Num4, "\u{f77e}", "Glass [4]"),
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
        let mut state = GrafiaCanvasState::init_glyph_a();
        state.show_selection_info_hud = true;
        fit_demo_canvas_viewport(&mut state);
        Self {
            state,
            theme: GrafiaTheme::sonoma_default(),
            show_preview_panel: false,
            host: MockFontHost::new(),
        }
    }

    /// Renders the complete interactive Grafia Vector Canvas Demo UI inside `ui`.
    pub fn render_ui(&mut self, ui: &mut egui::Ui) {
        self.host.active_mac_theme().apply_to_egui(ui.ctx());
        self.host.poll_host_ports(&mut self.state);
        for (mode, key, _, _) in VIEW_MODE_SPECS {
            if ui.input(|i| i.key_pressed(key)) {
                self.state.view_mode = mode;
            }
        }
        let w = ui.ctx().content_rect().width();
        self.host
            .update_responsive_layout(w, &mut self.show_preview_panel, &mut self.state);
        if self.state.icon_studio.kinetic.playing {
            let dt = f64::from(ui.input(|i| i.stable_dt).clamp(0.001, 0.05));
            self.state.step_kinetic_timeline(dt);
            ui.ctx().request_repaint();
        }
        Panel::top("top_toolbar").show(ui, |ui| {
            let pr = &mut self.show_preview_panel;
            render_top_toolbar(ui, &mut self.state, &mut self.theme, &mut self.host, pr);
            if self.host.show_coach_banner {
                ui.add(Separator::default().horizontal());
                self.host
                    .render_coach_banner(ui, &mut self.state, &mut self.theme);
            }
        });
        Panel::bottom("status_bar").show(ui, |ui| {
            MockFontHost::render_status_bar_contents(ui, &self.state)
        });
        if self.show_preview_panel {
            let ph = self.host.preview_panel_height_dp;
            Panel::bottom("glyph_preview_panel")
                .resizable(true)
                .default_size(ph)
                .size_range(110.0..=480.0)
                .show(ui, |ui| {
                    self.host.render_preview_strip_panel(ui, &self.state)
                });
        }
        if self.host.show_left_sidebar {
            Panel::left("shapes_sidebar").show(ui, |ui| {
                render_left_studio_sidebar(ui, &mut self.host, &mut self.state, &mut self.theme)
            });
        }
        if self.host.show_right_inspector {
            Panel::right("host_inspector_panel").show(ui, |ui| {
                self.host.render_host_inspector(ui, &mut self.state)
            });
        }
        CentralPanel::default().show(ui, |ui| {
            let avail = ui.available_size();
            self.host.record_and_fit_canvas_size(avail, &mut self.state);
            let stashed = self.host.take_mobile_hud_guarded_events(ui, avail);
            let r = GrafiaCanvasWidget::new(&mut self.state, &self.theme)
                .with_size(avail)
                .show(ui);
            MockFontHost::restore_mobile_hud_guarded_events(ui, stashed);
            self.host
                .render_mobile_thumb_hud_overlay(ui, r.rect, &mut self.state);
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
#[cfg(not(target_arch = "wasm32"))]
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

#[cfg(all(not(target_arch = "wasm32"), not(target_os = "linux")))]
pub fn main() -> eframe::Result<()> {
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

#[cfg(all(not(target_arch = "wasm32"), target_os = "linux"))]
pub fn main() {}

#[cfg(target_arch = "wasm32")]
pub fn main() {
    use wasm_bindgen::JsCast as _;
    wasm_bindgen_futures::spawn_local(async {
        let Some(canvas) = web_sys::window()
            .and_then(|w| w.document())
            .and_then(|d| d.get_element_by_id("grafia_canvas"))
            .and_then(|el| el.dyn_into::<web_sys::HtmlCanvasElement>().ok())
        else {
            return;
        };
        let _ = eframe::WebRunner::new()
            .start(
                canvas,
                eframe::WebOptions::default(),
                Box::new(|cc| {
                    configure_demo_fonts(&cc.egui_ctx);
                    Ok(Box::new(GrafiaVectorCanvasDemo::new()))
                }),
            )
            .await;
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use demo_host_shell::DemoPreset;
    use grafia::core::FontPoint;

    const TEST_ICON_FONT_SIZE_PT: f32 = 14.0;

    #[cfg(not(target_arch = "wasm32"))]
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
        let run_frame = |app: &mut GrafiaVectorCanvasDemo, rect: egui::Rect| {
            let input = egui::RawInput {
                screen_rect: Some(rect),
                ..Default::default()
            };
            let mut out = ctx.run_ui(input, |ui| app.render_ui(ui));
            out.textures_delta.clear();
            out
        };
        let mut app = GrafiaVectorCanvasDemo::new();
        let win_sz = egui::Vec2::new(WINDOW_WIDTH_DP, WINDOW_HEIGHT_DP);
        let screen_rect = egui::Rect::from_min_size(egui::Pos2::ZERO, win_sz);
        app.state
            .add_guide_from_selection(false, FontPoint::new(DEMO_GUIDE_X_UPM, DEMO_GUIDE_Y_UPM));

        for (preset, _) in DemoPreset::ALL {
            assert!(!demo_host_shell::coach_card(preset).title.is_empty());
            app.host
                .apply_preset(preset, &mut app.state, &mut app.theme);
            let out = run_frame(&mut app, screen_rect);
            assert!(!ctx.tessellate(out.shapes, out.pixels_per_point).is_empty());
            assert!(app.state.validate_invariants().is_ok());
        }
        assert_eq!(DemoPreset::ALL.len(), 36);
        assert!(app.host.show_left_sidebar && app.state.show_selection_info_hud);
        for ch in demo_host_shell::ShowcaseChapter::ALL {
            assert_eq!(ch.presets().len(), 6);
        }
        for th in demo_host_shell::StudioThemePreset::ALL {
            app.host
                .apply_studio_theme(th, &mut app.state, &mut app.theme);
            let _ = run_frame(&mut app, screen_rect);
            assert_eq!(
                ctx.global_style().visuals.panel_fill,
                app.host.active_mac_theme().window_background()
            );
        }
        let renders_before = app.host.preview_cache.render_count();
        let _ = run_frame(&mut app, screen_rect);
        assert_eq!(app.host.preview_cache.render_count(), renders_before);
        app.host.step_tour_next(&mut app.state, &mut app.theme);
        app.host.step_tour_prev(&mut app.state, &mut app.theme);
        app.host.apply_preset(
            DemoPreset::TextStreamAovaKerning,
            &mut app.state,
            &mut app.theme,
        );
        app.state.insert_text_at_caret("S");
        app.host.poll_host_ports(&mut app.state);
        assert_eq!(app.host.text_buffer, "AOSVA");
        app.state.delete_text_backward_at_caret();
        app.host.poll_host_ports(&mut app.state);
        assert_eq!(app.host.text_buffer, "AOVA");
        let lsb_act = grafia::core::SelectionInfoAction::SetLsb(55.0);
        assert!(app.state.apply_selection_info_action(lsb_act));
        app.host.poll_host_ports(&mut app.state);
        assert!(app.state.validate_invariants().is_ok());

        let mut mob = GrafiaVectorCanvasDemo::new();
        let _ = run_frame(
            &mut mob,
            egui::Rect::from_min_size(egui::Pos2::ZERO, egui::Vec2::new(390.0, 844.0)),
        );
        assert!(!mob.host.show_left_sidebar && mob.host.show_mobile_thumb_hud);
        assert_eq!(mob.state.preferences.touch_finger_offset_dp, 0.0);
        assert_eq!(demo_host_shell::sonoma_top_bar::TOOL_CATEGORIES.len(), 4);
        let c_sz = mob.host.last_canvas_size.expect("mobile size");
        assert!(c_sz.x >= 350.0 && c_sz.y >= 650.0);
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
            let fams = ui.ctx().fonts_mut(|f| f.definitions().families.clone());
            let cup = egui_macview::CUPERTINO_ICONS_FONT_NAME;
            assert_eq!(fams[&egui::FontFamily::Proportional][0].as_str(), cup);
            assert_eq!(fams[&egui::FontFamily::Monospace][0].as_str(), cup);
            assert_eq!(METRICS_ORIGIN_SPECS.map(|s| s.1), ["Base", "xH/2", "Cap"]);
            let font_id = egui::FontId::proportional(TEST_ICON_FONT_SIZE_PT);
            let layout_str = |s: String| {
                ui.painter()
                    .layout_no_wrap(s, font_id.clone(), egui::Color32::WHITE)
            };
            let tofu_uv = layout_str("\u{fffe}".to_owned()).rows[0].glyphs[0].uv_rect;
            for ch in ('0'..='9').chain('A'..='Z').chain('a'..='z') {
                assert_ne!(
                    layout_str(ch.to_string()).rows[0].glyphs[0].uv_rect,
                    tofu_uv
                );
            }
            for icon in TOOL_ICON_SPECS
                .iter()
                .map(|s| s.1)
                .chain(VIEW_MODE_SPECS.iter().map(|s| s.2))
                .chain(ORIGIN_ANCHOR_GRID_ROWS.iter().flatten().map(|r| r.1))
                .chain(egui_macview::MacSymbol::ALL.iter().map(|s| s.as_str()))
                .chain(ALL_ACTION_ICONS)
            {
                let galley = layout_str(icon.to_owned());
                assert!(galley.size().x > 0.0 && galley.rows[0].glyphs[0].uv_rect != tofu_uv);
            }
        });
        out.textures_delta.clear();
    }
}
