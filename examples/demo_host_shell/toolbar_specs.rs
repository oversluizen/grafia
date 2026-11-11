//! Toolbar, View-Mode, Transform-Origin, and Font Setup Specifications for `vector_canvas_demo`.

use egui_macview::{setup_symbols_font, MacSymbol};
use grafia::adapters::CanvasTool;
use grafia::core::MetricsOrigin;

pub const WINDOW_WIDTH_DP: f32 = 1280.0;
pub const WINDOW_HEIGHT_DP: f32 = 840.0;
pub const MIN_WINDOW_WIDTH_DP: f32 = 640.0;
pub const MIN_WINDOW_HEIGHT_DP: f32 = 480.0;
pub const ZOOM_IN_FACTOR: f32 = 1.25;
pub const ZOOM_OUT_FACTOR: f32 = 0.8;
pub const ZOOM_PERCENT_BASE: f32 = 100.0;
pub const SIDEBAR_TOP_SPACE_DP: f32 = 4.0;
pub const MIN_VIEWPORT_ZOOM: f32 = 0.01;
pub const MAX_VIEWPORT_ZOOM: f32 = 50.0;
pub const GRID_SPACING_DP: f32 = 2.0;
pub const DEMO_ROTATE_STEP_DEG: f64 = 15.0;
pub const DEMO_SCALE_UP_FACTOR: f64 = 1.1;
pub const DEMO_SLANT_STEP_DEG: f64 = 12.0;
pub const DEMO_GUIDE_X_UPM: f64 = 300.0;
pub const DEMO_GUIDE_Y_UPM: f64 = 250.0;
pub const APP_TITLE: &str = "Grafia Vector Canvas — Host-Port (Stekkers) Testbank";

/// Installs a deterministic panic hook writing `grafia_canvas_crash.log` beside the executable.
pub fn install_crash_logger() {
    std::panic::set_hook(Box::new(|info| {
        if let Ok(exe_path) = std::env::current_exe() {
            let _ = std::fs::write(
                exe_path.with_file_name("grafia_canvas_crash.log"),
                format!("PANIC: {info}\n"),
            );
        }
    }));
}

/// Visual 3x3 Origin Anchor Grid rows (`NW [6], N [7], NE [8]`, `W [3], C [4], E [5]`, `SW [0], S [1], SE [2]`).
pub const ORIGIN_ANCHOR_GRID_ROWS: [[(u8, &str, &str); 3]; 3] = [
    [
        (6, "\u{f52f}", "Top-Left Origin [NW / Anchor 6]"),
        (7, "\u{f366}", "Top-Center Origin [N / Anchor 7]"),
        (8, "\u{f534}", "Top-Right Origin [NE / Anchor 8]"),
    ],
    [
        (3, "\u{f4fd}", "Middle-Left Origin [W / Anchor 3]"),
        (4, "\u{f7e1}", "Center Origin [C / Anchor 4]"),
        (5, "\u{f50a}", "Middle-Right Origin [E / Anchor 5]"),
    ],
    [
        (0, "\u{f4ef}", "Bottom-Left Origin [SW / Anchor 0]"),
        (1, "\u{f35d}", "Bottom-Center Origin [S / Anchor 1]"),
        (2, "\u{f4f4}", "Bottom-Right Origin [SE / Anchor 2]"),
    ],
];

/// Metrics origin button labels and tooltips for the Transform Panel.
pub const METRICS_ORIGIN_SPECS: [(MetricsOrigin, &str, &str); 3] = [
    (
        MetricsOrigin::Baseline,
        "Base",
        "Transform Origin at Baseline (Y = 0 UPM)",
    ),
    (
        MetricsOrigin::HalfXHeight,
        "xH/2",
        "Transform Origin at Half x-Height",
    ),
    (
        MetricsOrigin::CapHeight,
        "Cap",
        "Transform Origin at Cap Height",
    ),
];

/// Tool icon and tooltip table covering all 13 `CanvasTool` variants.
pub const TOOL_ICON_SPECS: [(CanvasTool, &str, &str); 13] = [
    (
        CanvasTool::Select,
        "\u{f61e}",
        "Select Tool [V] — Drag nodes/segments (Shift: 45° snap, Alt: smooth/corner)",
    ),
    (
        CanvasTool::Lasso,
        "\u{f6d4}",
        "Lasso Tool [Shift+V] — Freeform polygon selection (Alt: OnCurve only, Shift: XOR)",
    ),
    (
        CanvasTool::Pen,
        "\u{f2bf}",
        "Pen Tool [P] — Click to place anchor, drag handles, click start node to close",
    ),
    (
        CanvasTool::Rotate,
        "\u{f7c7}",
        "Rotate Tool [R] — Click to relocate origin, drag to rotate (Shift: 45° snap)",
    ),
    (
        CanvasTool::Scale,
        "\u{f386}",
        "Scale Tool [S] — Click to relocate origin, drag to scale (Shift: constrain)",
    ),
    (
        CanvasTool::Zoom,
        MacSymbol::Search.as_str(),
        "Zoom Tool [Z] — Click or marquee drag to zoom in (Alt: zoom out)",
    ),
    (
        CanvasTool::Knife,
        "\u{f7c9}",
        "Knife Tool [E] — Drag across contours to slice (Alt: insert nodes, Shift: 0°/90°)",
    ),
    (
        CanvasTool::Primitives,
        "\u{f80c}",
        "Primitives Tool [F] — Drag Rect/Circle (Shift+F: cycle, Shift: square, Alt: center)",
    ),
    (
        CanvasTool::Measurement,
        "\u{f7ca}",
        "Measurement Tool [L] — Drag ruler to inspect intersections, segment lengths & angles",
    ),
    (
        CanvasTool::Annotation,
        "\u{f853}",
        "Annotation Tool [A] — Place Text/Circle/Arrow/Plus/Minus (Shift+A: cycle sub-tool)",
    ),
    (
        CanvasTool::Pencil,
        "\u{f7cb}",
        "Pencil Tool [B] — Freehand sketch fitted to smooth cubic Bezier segments",
    ),
    (
        CanvasTool::Pixel,
        "\u{f804}",
        "Pixel Tool [X] — Click/drag grid cells to stamp or erase pixel squares",
    ),
    (
        CanvasTool::Text,
        "\u{f892}",
        "Text Tool [T] — Multi-glyph horizontal strip, interactive spacing & kerning",
    ),
];

/// All standalone action icon codepoints rendered in the toolbar and sidebar.
pub const ALL_ACTION_ICONS: [&str; 22] = [
    "\u{f618}",
    MacSymbol::SidebarLeading.as_str(),
    "\u{f37d}",
    "\u{f544}",
    "\u{f549}",
    "\u{f7c5}",
    "\u{f7c7}",
    "\u{f500}",
    "\u{f52a}",
    "\u{f386}",
    "\u{f7da}",
    "\u{f4e3}",
    "\u{f53d}",
    "\u{f6f4}",
    "\u{f48a}",
    "\u{f899}",
    "\u{f892}",
    "\u{f80a}",
    "\u{f6e5}",
    "\u{f77e}",
    "\u{f424}",
    "\u{f4c8}",
];

/// Deterministically initializes `egui` fonts and registers `egui-macview` Cupertino vector icons.
pub fn configure_demo_fonts(ctx: &egui::Context) {
    let mut fonts = egui::FontDefinitions::default();
    let local_app_data = std::env::var("LOCALAPPDATA").unwrap_or_default();
    let udir = std::path::Path::new(&local_app_data).join("Microsoft\\Windows\\Fonts");
    let sdir = std::path::Path::new("C:\\Windows\\Fonts");
    let sys_fonts = [
        ("segoe_ui", "segoeui.ttf", egui::FontFamily::Proportional),
        ("inter", "Inter-Regular.ttf", egui::FontFamily::Proportional),
        (
            "roboto",
            "Roboto-Regular.ttf",
            egui::FontFamily::Proportional,
        ),
        ("consolas", "consola.ttf", egui::FontFamily::Monospace),
        (
            "jetbrains_mono",
            "JetBrainsMono-Regular.ttf",
            egui::FontFamily::Monospace,
        ),
    ];
    for (name, file, family) in sys_fonts {
        let maybe_bytes = std::fs::read(udir.join(file))
            .or_else(|_| std::fs::read(sdir.join(file)))
            .ok()
            .filter(|b| {
                b.get(..4)
                    .is_some_and(|h| matches!(h, [0, 1, 0, 0] | b"OTTO" | b"true" | b"ttcf"))
            });
        if let Some(b) = maybe_bytes {
            fonts.font_data.insert(
                name.to_owned(),
                std::sync::Arc::new(egui::FontData::from_owned(b)),
            );
            fonts
                .families
                .entry(family)
                .or_default()
                .insert(0, name.to_owned());
        }
    }
    setup_symbols_font(&mut fonts);
    let mut cup_bytes = egui_macview::CUPERTINO_ICONS_FONT_BYTES.to_vec();
    for range in [5558..5570, 5646..5658, 5732..5744, 5818..5830, 8444..8446] {
        cup_bytes[range].fill(0);
    }
    fonts.font_data.insert(
        egui_macview::CUPERTINO_ICONS_FONT_NAME.to_owned(),
        std::sync::Arc::new(egui::FontData::from_owned(cup_bytes)),
    );
    for fam in [egui::FontFamily::Proportional, egui::FontFamily::Monospace] {
        let list = fonts.families.entry(fam).or_default();
        list.retain(|f| f != egui_macview::CUPERTINO_ICONS_FONT_NAME);
        list.insert(0, egui_macview::CUPERTINO_ICONS_FONT_NAME.to_owned());
    }
    ctx.set_fonts(fonts);
}
