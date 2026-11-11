#![cfg_attr(not(test), windows_subsystem = "windows")]

//! Interactive Grafia Vector Canvas Demonstrator Example.
//!
//! Demonstrates:
//! - Golden Master glyph `/A` (UPM: 1000, Width: 700, 2 closed contours, 11 nodes).
//! - Three view mode projections: `EditWireframe`, `PreviewFilled`, `ShapeThumbnail`.
//! - Interactive tool switching between `SelectTool` (`V`) and `PenTool` (`P`).
//! - Multi-node dragging, 45° Shift-snapping, marquee selection, Option-click smooth/corner toggle.
//! - Pen tool anchor placement, handle extrusion, and contour closing.

use eframe::egui::{self, CentralPanel, Key, Panel, Separator};
use egui_macview::{setup_symbols_font, MacSymbol};
use grafia::adapters::{CanvasTool, CanvasViewMode, GrafiaCanvasState, GrafiaCanvasWidget};
use grafia::core::{FontPoint, MetricsOrigin, ShapeBooleanRole, TransformGridAnchor, TransformOriginSpec};
use grafia::tokens::{GrafiaTheme, GlyphArchetype};

const WINDOW_WIDTH_DP: f32 = 1223.0; const WINDOW_HEIGHT_DP: f32 = 811.0;
const MIN_WINDOW_WIDTH_DP: f32 = 640.0; const MIN_WINDOW_HEIGHT_DP: f32 = 480.0;
const ZOOM_IN_FACTOR: f32 = 1.25; const ZOOM_OUT_FACTOR: f32 = 0.8;
const ZOOM_PERCENT_BASE: f32 = 100.0; const SIDEBAR_TOP_SPACE_DP: f32 = 4.0;
const MIN_VIEWPORT_ZOOM: f32 = 0.01; const MAX_VIEWPORT_ZOOM: f32 = 50.0;
const GRID_SPACING_DP: f32 = 2.0; const DEMO_ROTATE_STEP_DEG: f64 = 15.0;
const DEMO_SCALE_UP_FACTOR: f64 = 1.1; const DEMO_SLANT_STEP_DEG: f64 = 12.0;
const DEMO_GUIDE_X_UPM: f64 = 300.0; const DEMO_GUIDE_Y_UPM: f64 = 250.0;
const APP_TITLE: &str = "Grafia Vector Canvas Demo";

/// View mode icon and tooltip table covering all 3 `CanvasViewMode` projections.
pub const VIEW_MODE_SPECS: [(CanvasViewMode, Key, &str, &str); 3] = [
    (CanvasViewMode::EditWireframe, Key::Num1, "\u{f7f8}", "Edit Wireframe View [1] — Bezier nodes, handles, metrics & guides"),
    (CanvasViewMode::PreviewFilled, Key::Num2, "\u{f7ff}", "Preview Filled View [2] — Filled glyph counter preview"),
    (CanvasViewMode::ShapeThumbnail, Key::Num3, "\u{f80d}", "Shape Thumbnail View [3] — Fit-to-viewport silhouette"),
];

/// Visual 3x3 Origin Anchor Grid rows (`NW [6], N [7], NE [8]`, `W [3], C [4], E [5]`, `SW [0], S [1], SE [2]`).
pub const ORIGIN_ANCHOR_GRID_ROWS: [[(u8, &str, &str); 3]; 3] = [
    [(6, "\u{f52f}", "Top-Left Origin [NW / Anchor 6]"), (7, "\u{f366}", "Top-Center Origin [N / Anchor 7]"), (8, "\u{f534}", "Top-Right Origin [NE / Anchor 8]")],
    [(3, "\u{f4fd}", "Middle-Left Origin [W / Anchor 3]"), (4, "\u{f7e1}", "Center Origin [C / Anchor 4]"), (5, "\u{f50a}", "Middle-Right Origin [E / Anchor 5]")],
    [(0, "\u{f4ef}", "Bottom-Left Origin [SW / Anchor 0]"), (1, "\u{f35d}", "Bottom-Center Origin [S / Anchor 1]"), (2, "\u{f4f4}", "Bottom-Right Origin [SE / Anchor 2]")],
];

/// Metrics origin button labels and tooltips for the Transform Panel.
pub const METRICS_ORIGIN_SPECS: [(MetricsOrigin, &str, &str); 3] = [(MetricsOrigin::Baseline, "Base", "Transform Origin at Baseline (Y = 0 UPM)"), (MetricsOrigin::HalfXHeight, "xH/2", "Transform Origin at Half x-Height"), (MetricsOrigin::CapHeight, "Cap", "Transform Origin at Cap Height")];

/// Tool icon and tooltip table covering all 12 `CanvasTool` variants.
pub const TOOL_ICON_SPECS: [(CanvasTool, &str, &str); 12] = [
    (CanvasTool::Select, "\u{f61e}", "Select Tool [V] — Drag nodes/segments (Shift: 45° snap, Alt: smooth/corner)"),
    (CanvasTool::Lasso, "\u{f6d4}", "Lasso Tool [Shift+V] — Freeform polygon selection (Alt: OnCurve only, Shift: XOR)"),
    (CanvasTool::Pen, "\u{f2bf}", "Pen Tool [P] — Click to place anchor, drag handles, click start node to close"),
    (CanvasTool::Rotate, "\u{f7c7}", "Rotate Tool [R] — Click to relocate origin, drag to rotate (Shift: 45° snap)"),
    (CanvasTool::Scale, "\u{f386}", "Scale Tool [S] — Click to relocate origin, drag to scale (Shift: constrain)"),
    (CanvasTool::Zoom, MacSymbol::Search.as_str(), "Zoom Tool [Z] — Click or marquee drag to zoom in (Alt: zoom out)"),
    (CanvasTool::Knife, "\u{f7c9}", "Knife Tool [E] — Drag across contours to slice (Alt: insert nodes, Shift: 0°/90°)"),
    (CanvasTool::Primitives, "\u{f80c}", "Primitives Tool [F] — Drag Rect/Circle (Shift+F: cycle, Shift: square, Alt: center)"),
    (CanvasTool::Measurement, "\u{f7ca}", "Measurement Tool [L] — Drag ruler to inspect intersections, segment lengths & angles"),
    (CanvasTool::Annotation, "\u{f853}", "Annotation Tool [A] — Place Text/Circle/Arrow/Plus/Minus (Shift+A: cycle sub-tool)"),
    (CanvasTool::Pencil, "\u{f7cb}", "Pencil Tool [B] — Freehand sketch fitted to smooth cubic Bezier segments"),
    (CanvasTool::Pixel, "\u{f804}", "Pixel Tool [X] — Click/drag grid cells to stamp or erase pixel squares"),
];

/// All standalone action icon codepoints rendered in the toolbar and sidebar.
pub const ALL_ACTION_ICONS: [&str; 22] = [
    "\u{f618}", MacSymbol::SidebarLeading.as_str(), "\u{f37d}", "\u{f544}", "\u{f549}", "\u{f7c5}", "\u{f7c7}", "\u{f500}", "\u{f52a}", "\u{f386}", "\u{f7da}",
    "\u{f4e3}", "\u{f53d}", "\u{f6f4}", "\u{f48a}", "\u{f899}", "\u{f892}", "\u{f80a}", "\u{f6e5}", "\u{f77e}", "\u{f424}", "\u{f4c8}",
];

/// Top-level demonstrator application embedding `GrafiaCanvasWidget`.
pub struct GrafiaVectorCanvasDemo { state: GrafiaCanvasState, theme: GrafiaTheme, show_preview_panel: bool, preview_config: grafia::render::PreviewPanelConfig }

impl Default for GrafiaVectorCanvasDemo { fn default() -> Self { Self::new() } }

impl GrafiaVectorCanvasDemo {
    #[must_use]
    pub fn new() -> Self { Self { state: GrafiaCanvasState::init_glyph_a(), theme: GrafiaTheme::sonoma_default(), show_preview_panel: true, preview_config: grafia::render::PreviewPanelConfig::default() } }

    /// Renders the complete interactive Grafia Vector Canvas Demo UI inside `ui`.
    pub fn render_ui(&mut self, ui: &mut egui::Ui) {
        for (mode, key, _, _) in VIEW_MODE_SPECS {
            if ui.input(|i| i.key_pressed(key)) { self.state.view_mode = mode; }
        }

        // Top Toolbar (Compact Icon Bar with Descriptive Tooltips)
        Panel::top("top_toolbar").show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.heading("grafia");
                ui.add(Separator::default().vertical());
                for (mode, _, icon, tip) in VIEW_MODE_SPECS {
                    if ui.selectable_label(self.state.view_mode == mode, icon).on_hover_text(tip).clicked() { self.state.view_mode = mode; }
                }
                ui.add(Separator::default().vertical());
                for (tool, icon, tip) in TOOL_ICON_SPECS {
                    if ui.selectable_label(self.state.active_tool == tool, icon).on_hover_text(tip).clicked() { self.state.activate_tool(tool); }
                }
                ui.add(Separator::default().vertical());
                if ui.selectable_label(self.state.show_bounding_box, "\u{f618}").on_hover_text("Toggle Selection Bounding Box — 8-knob scale, rotate & slant frame").clicked() { self.state.show_bounding_box = !self.state.show_bounding_box; }
                if ui.selectable_label(self.show_preview_panel, MacSymbol::SidebarLeading.as_str()).on_hover_text("Toggle Bottom Live Preview Panel").clicked() { self.show_preview_panel = !self.show_preview_panel; }
                ui.add(Separator::default().vertical());
                if ui.button(MacSymbol::Minus.as_str()).on_hover_text("Zoom Out [Cmd+-]").clicked() { self.state.viewport.zoom = (self.state.viewport.zoom * ZOOM_OUT_FACTOR).max(MIN_VIEWPORT_ZOOM); }
                ui.label(format!("{:.0}%", self.state.viewport.zoom * ZOOM_PERCENT_BASE));
                if ui.button(MacSymbol::Plus.as_str()).on_hover_text("Zoom In [Cmd+=]").clicked() { self.state.viewport.zoom = (self.state.viewport.zoom * ZOOM_IN_FACTOR).min(MAX_VIEWPORT_ZOOM); }
                if ui.button("\u{f37d}").on_hover_text("Fit Glyph in Viewport [Cmd+0]").clicked() { self.state.fit_viewport_to_glyph(WINDOW_WIDTH_DP, WINDOW_HEIGHT_DP); }
                ui.add(Separator::default().vertical());
                for arch in GlyphArchetype::all() {
                    if ui.selectable_label(self.state.active_archetype == arch, arch.button_label()).on_hover_text(format!("Load Golden Master Glyph {}", arch.button_label())).clicked() { self.state.load_glyph_archetype(arch); }
                }
                ui.add(Separator::default().vertical());
                if ui.add_enabled(self.state.history.can_undo(), egui::Button::new("\u{f544}")).on_hover_text("Undo [Cmd+Z]").on_disabled_hover_text("Undo [Cmd+Z]").clicked() { self.state.undo(); }
                if ui.add_enabled(self.state.history.can_redo(), egui::Button::new("\u{f549}")).on_hover_text("Redo [Cmd+Shift+Z]").on_disabled_hover_text("Redo [Cmd+Shift+Z]").clicked() { self.state.redo(); }
                if ui.button(MacSymbol::ArrowCounterClockwise.as_str()).on_hover_text("Reset Canvas to Golden Master /A").clicked() { self.state = GrafiaCanvasState::init_glyph_a(); }
            });
        });

        // Bottom Status Bar
        Panel::bottom("status_bar").show(ui, |ui| {
            ui.horizontal(|ui| {
                let tool_str = TOOL_ICON_SPECS.iter().find(|(t, _, _)| *t == self.state.active_tool).map_or("", |(_, _, tip)| *tip);
                ui.label(tool_str);
                ui.add(Separator::default().vertical());
                let total_nodes: usize = self.state.contours.iter().map(|c| c.len()).sum();
                ui.label(format!("Contours: {} | Nodes: {total_nodes} (Selected: {})", self.state.contours.len(), self.state.selected_nodes.len()));
            });
        });

        // Left Shapes & Inspector Sidebar (Declared BEFORE glyph_preview_panel so it spans full height)
        Panel::left("shapes_sidebar").show(ui, |ui| {
            ui.add_space(SIDEBAR_TOP_SPACE_DP);
            ui.strong(format!("Shapes ({})", self.state.glyph.contours.len()));
            ui.add(Separator::default().horizontal());

            for contour in &self.state.glyph.contours {
                ui.push_id(contour.id.0, |ui| {
                    ui.horizontal(|ui| {
                        let locator_clicked = ui.add(grafia::locator::ShapeLocatorWidget::new(&self.state.glyph, contour.id)).on_hover_text("Click to select contour").clicked();
                        let is_selected = !contour.nodes.is_empty() && contour.nodes.iter().any(|n| self.state.selected_nodes.contains(&n.id));
                        let role_str = match contour.role { ShapeBooleanRole::Normal => "Normal", ShapeBooleanRole::Mask => "Mask", ShapeBooleanRole::Add => "Add", ShapeBooleanRole::Intersect => "Intersect" };
                        let winding_str = if contour.closed { grafia::locator::contour_winding_label(&contour.nodes) } else { "Open" };
                        let label_text = format!("Path {}\n{} pt • {role_str}, {winding_str}", contour.id.0 + 1, contour.nodes.len());
                        if locator_clicked || ui.selectable_label(is_selected, label_text).on_hover_text("Click to select contour nodes").clicked() { self.state.selected_nodes = contour.nodes.iter().map(|n| n.id).collect(); }
                    });
                });
            }

            ui.add(Separator::default().horizontal());
            ui.strong("Transform Panel");
            ui.horizontal(|ui| {
                egui::Grid::new("origin_anchor_grid").spacing([GRID_SPACING_DP, GRID_SPACING_DP]).show(ui, |g_ui| {
                    for row in ORIGIN_ANCHOR_GRID_ROWS {
                        for (idx, icon, tip) in row {
                            let anchor = TransformGridAnchor::from_index(idx);
                            let active = self.state.transform_origin_mode == TransformOriginSpec::BoundingBoxOrigin(anchor);
                            if g_ui.selectable_label(active, icon).on_hover_text(tip).clicked() { self.state.set_transform_grid_anchor(anchor); }
                        }
                        g_ui.end_row();
                    }
                });
                ui.add(Separator::default().vertical());
                ui.vertical(|v_ui| {
                    v_ui.style_mut().wrap_mode = Some(egui::TextWrapMode::Extend);
                    for (m, lbl, tip) in METRICS_ORIGIN_SPECS {
                        let active = self.state.transform_origin_mode == TransformOriginSpec::MetricsOrigin(m);
                        if v_ui.selectable_label(active, lbl).on_hover_text(tip).clicked() { self.state.set_metrics_origin(m); }
                    }
                });
            });
            ui.horizontal(|ui| {
                if ui.button("\u{f7c5}").on_hover_text("Rotate -15° CCW around active Transform Origin").clicked() { self.state.rotate_by_degrees(-DEMO_ROTATE_STEP_DEG); }
                if ui.button("\u{f7c7}").on_hover_text("Rotate +15° CW around active Transform Origin").clicked() { self.state.rotate_by_degrees(DEMO_ROTATE_STEP_DEG); }
                if ui.button("\u{f500}").on_hover_text("Flip Horizontal across active Transform Origin").clicked() { self.state.flip_horizontal(); }
                if ui.button("\u{f52a}").on_hover_text("Flip Vertical across active Transform Origin").clicked() { self.state.flip_vertical(); }
                if ui.button("\u{f386}").on_hover_text("Scale 110% around active Transform Origin").clicked() { self.state.scale_by_factor(DEMO_SCALE_UP_FACTOR, DEMO_SCALE_UP_FACTOR); }
                if ui.button("\u{f7da}").on_hover_text("Slant +12° Horizontal Shear around active Transform Origin").clicked() { self.state.slant_by_degrees(DEMO_SLANT_STEP_DEG, true); }
            });

            ui.add(Separator::default().horizontal());
            ui.strong("Path Operations");
            ui.horizontal_wrapped(|ui| {
                if ui.button("\u{f4e3}").on_hover_text("Reverse Contours — Flip winding order of selected or all contours").clicked() { self.state.reverse_selected_or_all_contours(); }
                if ui.button("\u{f53d}").on_hover_text("Make Node First — Promote selected on-curve node to contour start").clicked() { self.state.make_node_first(); }
                if ui.button("\u{f6f4}").on_hover_text("Correct Path Direction [Cmd+Shift+R] — Outer CCW, inner counters CW").clicked() { self.state.correct_path_direction(); }
                if ui.button("\u{f48a}").on_hover_text("Add Extremes — Insert on-curve nodes at horizontal & vertical extrema").clicked() { self.state.add_extremes_action(); }
                if ui.button("\u{f899}").on_hover_text("Add Inflections — Split cubic segments at zero-curvature inflection points").clicked() { self.state.add_inflection_points(); }
                if ui.button("\u{f892}").on_hover_text("Tidy Up Paths [Cmd+Opt+Shift+T] — Clean redundant nodes & collinear handles").clicked() { self.state.tidy_up_paths(); }
                if ui.button("\u{f80a}").on_hover_text("Align Selection [Cmd+Shift+A] — Align 2 nodes on axis or 3+ to bbox center").clicked() { self.state.align_selection(); }
                if ui.button("\u{f6e5}").on_hover_text("Connect Open Endpoints — Join two selected open contour endpoints").clicked() { self.state.connect_selected_open_endpoints(); }
                if ui.button("\u{f80c}").on_hover_text("Remove Overlap [Cmd+Shift+O] — Boolean union of overlapping closed contours").clicked() { self.state.remove_overlap(); }
            });

            ui.add(Separator::default().horizontal());
            ui.strong("Guides & Snapping");
            ui.horizontal_wrapped(|ui| {
                let g_pt = FontPoint::new(DEMO_GUIDE_X_UPM, DEMO_GUIDE_Y_UPM);
                if ui.button(MacSymbol::Plus.as_str()).on_hover_text("Add Local Guide from selection or default point").clicked() { self.state.add_guide_from_selection(false, g_pt); }
                if ui.button("\u{f77e}").on_hover_text("Add Global Master Guide across all glyphs").clicked() { self.state.add_guide_from_selection(true, g_pt); }
                if ui.selectable_label(self.state.show_guides, "\u{f424}").on_hover_text("Show / Hide Guides [Cmd+Shift+L]").clicked() { self.state.show_guides = !self.state.show_guides; }
                if ui.selectable_label(self.state.lock_guides, "\u{f4c8}").on_hover_text("Lock / Unlock Guides").clicked() { self.state.lock_guides = !self.state.lock_guides; }
                if let Some(sel) = self.state.selected_guide {
                    let cur_m = self.state.find_guide(sel.id).is_some_and(|g| g.show_measurement);
                    if ui.selectable_label(cur_m, "\u{f7ca}").on_hover_text("Toggle Guide Measurement [M]").clicked() { self.state.set_guide_show_measurement(sel.id, !cur_m); }
                }
            });
        });

        // Bottom Live Glyph Preview Panel (Strictly under central Edit Canvas)
        if self.show_preview_panel {
            Panel::bottom("glyph_preview_panel").show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.strong("Preview");
                    ui.add(Separator::default().vertical());
                    if ui.selectable_label(self.preview_config.inverted, "Invert").on_hover_text("Invert foreground/background polarity").clicked() { self.preview_config.inverted = !self.preview_config.inverted; }
                    if ui.selectable_label(self.preview_config.flipped, "Flip").on_hover_text("Flip preview upside-down for rhythm inspection").clicked() { self.preview_config.flipped = !self.preview_config.flipped; }
                    if ui.selectable_label(self.preview_config.show_baseline, "Baseline").on_hover_text("Show baseline and metric hairlines").clicked() { self.preview_config.show_baseline = !self.preview_config.show_baseline; }
                });
                ui.add(grafia::render::GrafiaPreviewWidget::new(&self.state).with_config(self.preview_config));
            });
        }

        CentralPanel::default().show(ui, |ui| {
            GrafiaCanvasWidget::new(&mut self.state, &self.theme).with_size(ui.available_size()).show(ui);
        });
    }
}

impl eframe::App for GrafiaVectorCanvasDemo {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) { self.render_ui(ui); }
}

/// Deterministically initializes `egui` fonts and registers `egui-macview` Cupertino vector icons
/// without blocking the main UI thread on background system-font enumeration.
pub fn configure_demo_fonts(ctx: &egui::Context) {
    let mut fonts = egui::FontDefinitions::default();
    let udir = std::path::Path::new(&std::env::var("LOCALAPPDATA").unwrap_or_default()).join("Microsoft\\Windows\\Fonts");
    let sdir = std::path::Path::new("C:\\Windows\\Fonts");
    let sys_fonts = [
        ("segoe_ui", "segoeui.ttf", egui::FontFamily::Proportional), ("inter", "Inter-Regular.ttf", egui::FontFamily::Proportional),
        ("roboto", "Roboto-Regular.ttf", egui::FontFamily::Proportional), ("consolas", "consola.ttf", egui::FontFamily::Monospace),
        ("jetbrains_mono", "JetBrainsMono-Regular.ttf", egui::FontFamily::Monospace),
    ];
    for (name, file, family) in sys_fonts {
        if let Some(b) = std::fs::read(udir.join(file)).or_else(|_| std::fs::read(sdir.join(file))).ok().filter(|b| b.get(..4).is_some_and(|h| matches!(h, [0, 1, 0, 0] | b"OTTO" | b"true" | b"ttcf"))) {
            fonts.font_data.insert(name.to_owned(), std::sync::Arc::new(egui::FontData::from_owned(b)));
            fonts.families.entry(family).or_default().insert(0, name.to_owned());
        }
    }
    setup_symbols_font(&mut fonts);
    let mut cup_bytes = egui_macview::CUPERTINO_ICONS_FONT_BYTES.to_vec();
    for range in [5558..5570, 5646..5658, 5732..5744, 5818..5830, 8444..8446] { cup_bytes[range].fill(0); }
    fonts.font_data.insert(egui_macview::CUPERTINO_ICONS_FONT_NAME.to_owned(), std::sync::Arc::new(egui::FontData::from_owned(cup_bytes)));
    for fam in [egui::FontFamily::Proportional, egui::FontFamily::Monospace] {
        let list = fonts.families.entry(fam).or_default();
        list.retain(|f| f != egui_macview::CUPERTINO_ICONS_FONT_NAME);
        list.insert(0, egui_macview::CUPERTINO_ICONS_FONT_NAME.to_owned());
    }
    ctx.set_fonts(fonts);
}

/// Builds deterministic `eframe::NativeOptions` resilient against hybrid-GPU + virtual-adapter
/// `wgpu` debug-layer TDRs (`LiveKernelEvent 141`) and main-thread font enumeration stalls.
#[must_use]
pub fn build_native_options() -> eframe::NativeOptions {
    let renderer = match std::env::var("GRAFIA_RENDERER").as_deref() { Ok("wgpu") => eframe::Renderer::Wgpu, _ => eframe::Renderer::Glow };
    let mut wgpu_setup = eframe::egui_wgpu::WgpuSetupCreateNew::without_display_handle();
    wgpu_setup.instance_descriptor.flags = eframe::wgpu::InstanceFlags::empty();
    let viewport = egui::ViewportBuilder::default().with_inner_size([WINDOW_WIDTH_DP, WINDOW_HEIGHT_DP]).with_min_inner_size([MIN_WINDOW_WIDTH_DP, MIN_WINDOW_HEIGHT_DP]).with_active(true).with_visible(true).with_title(APP_TITLE);
    let wgpu_options = eframe::egui_wgpu::WgpuConfiguration { wgpu_setup: eframe::egui_wgpu::WgpuSetup::CreateNew(wgpu_setup), ..Default::default() };
    eframe::NativeOptions { viewport, renderer, wgpu_options, system_font_fallback: false, persist_window: false, ..Default::default() }
}

fn install_crash_logger() {
    std::panic::set_hook(Box::new(|info| {
        if let Ok(exe_path) = std::env::current_exe() {
            let _ = std::fs::write(exe_path.with_file_name("grafia_canvas_crash.log"), format!("PANIC: {info}\n"));
        }
    }));
}

fn main() -> eframe::Result<()> {
    install_crash_logger();
    eframe::run_native(APP_TITLE, build_native_options(), Box::new(|cc| {
        configure_demo_fonts(&cc.egui_ctx);
        Ok(Box::new(GrafiaVectorCanvasDemo::new()))
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    const TEST_ICON_FONT_SIZE_PT: f32 = 14.0; const MAX_EXAMPLE_ESLOC: usize = 300;
    const MAX_SINGLE_LINE_SEMICOLONS: usize = 2;

    #[test]
    fn test_native_options_prevent_wgpu_tdr_and_system_font_main_thread_hang() {
        let options = build_native_options();
        assert!(options.renderer == eframe::Renderer::Glow && !options.system_font_fallback && !options.persist_window);
    }

    #[test]
    fn test_vector_canvas_demo_all_archetypes_modes_and_tools_no_panic() {
        let ctx = egui::Context::default();
        configure_demo_fonts(&ctx);
        let mut app = GrafiaVectorCanvasDemo::new();
        let screen_rect = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::Vec2::new(WINDOW_WIDTH_DP, WINDOW_HEIGHT_DP));
        app.state.add_guide_from_selection(false, FontPoint::new(DEMO_GUIDE_X_UPM, DEMO_GUIDE_Y_UPM));

        for arch in GlyphArchetype::all() {
            app.state.load_glyph_archetype(arch);
            for (mode, _, _, _) in VIEW_MODE_SPECS {
                app.state.view_mode = mode;
                for (tool, _, _) in TOOL_ICON_SPECS {
                    app.state.activate_tool(tool);
                    let raw_input = egui::RawInput { screen_rect: Some(screen_rect), ..Default::default() };
                    let mut full_output = ctx.run_ui(raw_input, |ui| app.render_ui(ui));
                    full_output.textures_delta.clear();
                    assert!(!ctx.tessellate(full_output.shapes, full_output.pixels_per_point).is_empty());
                }
            }
        }
    }

    #[test]
    fn test_configure_demo_fonts_registers_cupertino_icons_and_shapes_all_symbols() {
        let src = include_str!("vector_canvas_demo.rs");
        let esloc = src.lines().filter(|l| !l.trim().is_empty() && !l.trim().starts_with("//")).count();
        assert!(esloc < MAX_EXAMPLE_ESLOC, "examples/vector_canvas_demo.rs eSLOC {esloc} must be < {MAX_EXAMPLE_ESLOC}");
        for (idx, line) in src.lines().enumerate() {
            let code = line.split("//").next().unwrap_or("").replace("; 3]", "]").replace("; 22]", "]");
            assert!(code.matches(';').count() <= MAX_SINGLE_LINE_SEMICOLONS, "Line {} has >2 semicolons: {code}", idx + 1);
        }
        assert_eq!(ORIGIN_ANCHOR_GRID_ROWS.map(|row| row.map(|r| r.0)), [[6, 7, 8], [3, 4, 5], [0, 1, 2]]);

        let ctx = egui::Context::default();
        configure_demo_fonts(&ctx);
        let mut out = ctx.run_ui(egui::RawInput::default(), |ui| {
            let defs = ui.ctx().fonts_mut(|f| f.definitions().clone());
            assert_eq!((defs.families[&egui::FontFamily::Proportional][0].as_str(), defs.families[&egui::FontFamily::Monospace][0].as_str()), (egui_macview::CUPERTINO_ICONS_FONT_NAME, egui_macview::CUPERTINO_ICONS_FONT_NAME));
            let sdir = std::path::Path::new("C:\\Windows\\Fonts");
            if sdir.join("segoeui.ttf").exists() { assert!(defs.families[&egui::FontFamily::Proportional].contains(&"segoe_ui".to_owned())); }
            if sdir.join("consola.ttf").exists() { assert!(defs.families[&egui::FontFamily::Monospace].contains(&"consolas".to_owned())); }
            assert_eq!(METRICS_ORIGIN_SPECS.map(|s| s.1), ["Base", "xH/2", "Cap"]);
            let cup_bytes: &[u8] = defs.font_data[egui_macview::CUPERTINO_ICONS_FONT_NAME].font.as_ref().as_ref();
            assert!(cup_bytes[5558..5570].iter().all(|&b| b == 0) && cup_bytes[5646..5658].iter().all(|&b| b == 0));
            let font_id = egui::FontId::proportional(TEST_ICON_FONT_SIZE_PT);
            let tofu_uv = ui.painter().layout_no_wrap("\u{fffe}".to_owned(), font_id.clone(), egui::Color32::WHITE).rows[0].glyphs[0].uv_rect;
            for ch in ('0'..='9').chain('A'..='Z').chain('a'..='z') {
                assert_ne!(ui.painter().layout_no_wrap(ch.to_string(), font_id.clone(), egui::Color32::WHITE).rows[0].glyphs[0].uv_rect, tofu_uv, "ASCII {ch:?} must shape via primary text font");
            }
            for icon in TOOL_ICON_SPECS.iter().map(|s| s.1).chain(VIEW_MODE_SPECS.iter().map(|s| s.2)).chain(ORIGIN_ANCHOR_GRID_ROWS.iter().flatten().map(|r| r.1)).chain(egui_macview::MacSymbol::ALL.iter().map(|s| s.as_str())).chain(ALL_ACTION_ICONS) {
                let galley = ui.painter().layout_no_wrap(icon.to_owned(), font_id.clone(), egui::Color32::WHITE);
                assert!(galley.size().x > 0.0 && galley.size().y > 0.0 && galley.rows[0].glyphs[0].uv_rect != tofu_uv, "Icon {icon:?} failed to shape");
            }
        });
        out.textures_delta.clear();
    }
}
