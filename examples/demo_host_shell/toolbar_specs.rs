//! Toolbar, View-Mode, Transform-Origin, and Font Setup Specifications for `vector_canvas_demo`.

use super::DemoPreset;
use egui_macview::{setup_symbols_font, MacSymbol};
use grafia::adapters::CanvasTool;
use grafia::core::MetricsOrigin;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum StudioWorkspaceMode {
    #[default]
    LetterStudio,
    IconStudio,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum StudioSubWorkspace {
    #[default]
    LetterContourAndSpacing,
    LetterCagdAndStems,
    LetterMastersAndHinting,
    IconVectorAndGlass,
    IconKineticAndTouch,
}

#[rustfmt::skip]
impl StudioSubWorkspace {
    pub const LETTER_TABS: [Self; 3] = [Self::LetterContourAndSpacing, Self::LetterCagdAndStems, Self::LetterMastersAndHinting];
    pub const ICON_TABS: [Self; 2] = [Self::IconVectorAndGlass, Self::IconKineticAndTouch];
    #[must_use] pub const fn studio_mode(self) -> StudioWorkspaceMode {
        if matches!(self, Self::IconVectorAndGlass | Self::IconKineticAndTouch) { StudioWorkspaceMode::IconStudio } else { StudioWorkspaceMode::LetterStudio }
    }
    #[must_use] pub const fn tab_label(self) -> &'static str {
        match self {
            Self::LetterContourAndSpacing => "1. Contour", Self::LetterCagdAndStems => "2. Bochten",
            Self::LetterMastersAndHinting => "3. Masters", Self::IconVectorAndGlass => "4. Vector & Glas",
            Self::IconKineticAndTouch => "5. Kinetisch",
        }
    }
    #[must_use] pub const fn tools(self) -> [(CanvasTool, &'static str); 6] {
        use CanvasTool as T;
        match self {
            Self::LetterContourAndSpacing => [(T::Select, "\u{f61e}"), (T::Pen, "\u{f2bf}"), (T::Primitives, "\u{f80c}"), (T::Knife, "\u{f7c9}"), (T::Text, "\u{f892}"), (T::Annotation, "\u{f853}")],
            Self::LetterCagdAndStems => [(T::Select, "\u{f61e}"), (T::Pen, "\u{f2bf}"), (T::Pencil, "\u{f7cb}"), (T::Stem, "\u{f500}"), (T::Measurement, "\u{f7ca}"), (T::Trowel, "\u{f6f4}")],
            Self::LetterMastersAndHinting => [(T::Select, "\u{f61e}"), (T::CurvedTrajectory, "\u{f6e5}"), (T::Hoi, "\u{f899}"), (T::TrueTypeInstructor, "\u{f48a}"), (T::Pixel, "\u{f804}"), (T::Zoom, "\u{f785}")],
            Self::IconVectorAndGlass => [(T::Select, "\u{f61e}"), (T::Pen, "\u{f2bf}"), (T::Primitives, "\u{f80c}"), (T::ShapeBuilder, "\u{f618}"), (T::Rotate, "\u{f7c7}"), (T::Scale, "\u{f386}")],
            Self::IconKineticAndTouch => [(T::Select, "\u{f61e}"), (T::Pen, "\u{f2bf}"), (T::ShapeBuilder, "\u{f618}"), (T::Lasso, "\u{f6d4}"), (T::Measurement, "\u{f7ca}"), (T::Zoom, "\u{f785}")],
        }
    }
    #[must_use] pub const fn default_preset(self) -> DemoPreset {
        match self {
            Self::LetterContourAndSpacing => DemoPreset::TextStreamAovaKerning,
            Self::LetterCagdAndStems => DemoPreset::CurvatureCombSpikesAndHeatmap,
            Self::LetterMastersAndHinting => DemoPreset::MultiMasterCompatAndAutoMatch,
            Self::IconVectorAndGlass => DemoPreset::IconLiquidGlassAndBakeColrV1,
            Self::IconKineticAndTouch => DemoPreset::KineticWifiBounceAndColorModes,
        }
    }
    #[must_use] pub const fn from_tour_index(idx: usize) -> Self {
        match idx { 0..=5 => Self::LetterContourAndSpacing, 6..=17 => Self::LetterCagdAndStems, 18..=23 => Self::LetterMastersAndHinting, 24..=29 => Self::IconVectorAndGlass, _ => Self::IconKineticAndTouch }
    }
}

pub const WINDOW_WIDTH_DP: f32 = 1280.0;
pub const WINDOW_HEIGHT_DP: f32 = 840.0;
pub const MIN_WINDOW_WIDTH_DP: f32 = 640.0;
pub const MIN_WINDOW_HEIGHT_DP: f32 = 480.0;
pub const DEMO_CANVAS_CENTER_X_DP: f32 = 544.0;
pub const DEMO_CANVAS_CENTER_Y_DP: f32 = 382.0;
pub const DEMO_DEFAULT_ZOOM: f32 = 0.48;

/// Centers the active glyph inside the central canvas panel of the 1280x840 demo layout.
pub fn fit_demo_canvas_viewport(state: &mut grafia::adapters::GrafiaCanvasState) {
    let glyph_h = (state.glyph.height as f32).max(1.0);
    let glyph_w = (state.glyph.width as f32).max(1.0);
    let zoom = DEMO_DEFAULT_ZOOM;
    let pan_x = DEMO_CANVAS_CENTER_X_DP - (glyph_w * zoom) * 0.5;
    let pan_y = DEMO_CANVAS_CENTER_Y_DP + (glyph_h * zoom) * 0.5;
    state.viewport.zoom = zoom;
    state.viewport.pan_offset = grafia::core::ScreenPoint::new(pan_x, pan_y);
}
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
pub const EVENT_LOG_MAX_HEIGHT_DP: f32 = 110.0;
pub const PREVIEW_STRIP_MIN_WIDTH_DP: f32 = 64.0;
pub const PREVIEW_STRIP_MAX_WIDTH_DP: f32 = 2048.0;
pub const PREVIEW_HEIGHT_S_DP: f32 = 120.0;
pub const PREVIEW_HEIGHT_M_DP: f32 = 180.0;
pub const PREVIEW_HEIGHT_L_DP: f32 = 300.0;
pub const PREVIEW_HEIGHT_MATCH_TOL_DP: f32 = 16.0;
pub const PREVIEW_HEADER_CHROME_DP: f32 = 56.0;
pub const PREVIEW_STRIP_MIN_HEIGHT_DP: f32 = 64.0;
pub const PREVIEW_STRIP_MAX_HEIGHT_DP: f32 = 420.0;
pub const PREVIEW_PANEL_MIN_DP: f32 = 110.0;
pub const PREVIEW_PANEL_MAX_DP: f32 = 480.0;
pub const BOTTOM_WATERFALL_SPLIT_RATIO: f32 = 0.68;
pub const COMPACT_PHONE_BREAKPOINT_DP: f32 = 600.0;
pub const GLASS_CAPSULE_CORNER_RADIUS_U8: u8 = 9;
pub const GLASS_CAPSULE_PAD_X_F32: f32 = 8.0;
pub const GLASS_CAPSULE_PAD_Y_F32: f32 = 4.0;
pub const GLASS_HEADER_FONT_PT: f32 = 9.5;
pub const GLASS_VALUE_FONT_PT: f32 = 11.5;
pub const CHAPTER_POPUP_WIDTH_DP: f32 = 76.0;
pub const CASE_POPUP_WIDTH_DP: f32 = 148.0;
pub const KINETIC_POPUP_WIDTH_DP: f32 = 136.0;
pub const TOOL_GRID_SPACING_X_DP: f32 = 8.0;
pub const TOOL_GRID_SPACING_Y_DP: f32 = 4.0;
pub const INFO_GRID_SPACING_X_DP: f32 = 12.0;
pub const INFO_GRID_SPACING_Y_DP: f32 = 4.0;
pub const MOBILE_RESPONSIVE_BREAKPOINT_WIDTH_DP: f32 = 900.0;
pub const MIN_FIT_CANVAS_DIM_DP: f32 = 64.0;
pub const CANVAS_RESIZE_EPSILON_SQ_DP: f32 = 16.0;
pub const PRESETS_MENU_MAX_HEIGHT_DP: f32 = 320.0;
pub const TOUCH_LOUPE_FINGER_OFFSET_DP: f32 = 44.0;
pub const APP_TITLE: &str = "Grafia Vector Canvas — Editorial Studio & Host-Port Showcase";

#[rustfmt::skip]
pub const CORE_STUDIO_TOOLS: [(CanvasTool, &str, &str); 6] = [
    (CanvasTool::Select, "\u{f61e}", "Selecteren (V)"),
    (CanvasTool::Pen, "\u{f2bf}", "Bézier-Pen (P)"),
    (CanvasTool::Primitives, "\u{f80c}", "Vormen (F)"),
    (CanvasTool::Knife, "\u{f7c9}", "Mes (E)"),
    (CanvasTool::ShapeBuilder, "\u{f618}", "Shape Builder"),
    (CanvasTool::Text, "\u{f892}", "Tekst & Kerning (T)"),
];

#[rustfmt::skip]
pub const KINETIC_EFFECT_LABELS: [&str; 10] = [
    "Pulse", "Bounce", "VarColor", "Scale", "DrawOn",
    "DrawOff", "Wiggle", "Rotate", "Breathe", "MagicReplace",
];

#[rustfmt::skip]
pub const TOOL_CATEGORIES: [(&str, &[CanvasTool]); 4] = [
    ("SELECTIE & TRANSFORMATIE", &[CanvasTool::Select, CanvasTool::Lasso, CanvasTool::Rotate, CanvasTool::Scale, CanvasTool::Zoom, CanvasTool::Text]),
    ("TEKENEN & CONSTRUCTIE", &[CanvasTool::Pen, CanvasTool::Primitives, CanvasTool::Pencil, CanvasTool::Knife, CanvasTool::ShapeBuilder, CanvasTool::Pixel]),
    ("BOCHTEN, STEMS & BORSTELS", &[CanvasTool::Measurement, CanvasTool::Stem, CanvasTool::Trowel, CanvasTool::Cleaner]),
    ("MASTERS, HINTING & NOTITIE", &[CanvasTool::TrueTypeInstructor, CanvasTool::Hoi, CanvasTool::CurvedTrajectory, CanvasTool::Annotation]),
];

/// Installs a deterministic panic hook writing `grafia_canvas_crash.log` beside the executable.
pub fn install_crash_logger() {
    #[cfg(not(target_arch = "wasm32"))]
    std::panic::set_hook(Box::new(|info| {
        if let Ok(exe_path) = std::env::current_exe() {
            let _ = std::fs::write(
                exe_path.with_file_name("grafia_canvas_crash.log"),
                format!("PANIC: {info}\n"),
            );
        }
    }));
}

#[rustfmt::skip]
pub const ORIGIN_ANCHOR_GRID_ROWS: [[(u8, &str, &str); 3]; 3] = [
    [(6, "\u{f52f}", "Top-Left [NW]"), (7, "\u{f366}", "Top-Center [N]"), (8, "\u{f534}", "Top-Right [NE]")],
    [(3, "\u{f4fd}", "Middle-Left [W]"), (4, "\u{f7e1}", "Center [C]"), (5, "\u{f50a}", "Middle-Right [E]")],
    [(0, "\u{f4ef}", "Bottom-Left [SW]"), (1, "\u{f35d}", "Bottom-Center [S]"), (2, "\u{f4f4}", "Bottom-Right [SE]")],
];

#[rustfmt::skip]
pub const METRICS_ORIGIN_SPECS: [(MetricsOrigin, &str, &str); 3] = [
    (MetricsOrigin::Baseline, "Base", "Origin at Baseline (Y = 0 UPM)"),
    (MetricsOrigin::HalfXHeight, "xH/2", "Origin at Half x-Height"),
    (MetricsOrigin::CapHeight, "Cap", "Origin at Cap Height"),
];

#[rustfmt::skip]
pub const TOOL_ICON_SPECS: [(CanvasTool, &str, &str); 20] = [
    (CanvasTool::Select, "\u{f61e}", "Select Tool [V] — Drag nodes/segments"),
    (CanvasTool::Lasso, "\u{f6d4}", "Lasso Tool [Shift+V] — Freeform selection"),
    (CanvasTool::Pen, "\u{f2bf}", "Pen Tool [P] — Cubic Bezier & spline pen"),
    (CanvasTool::Rotate, "\u{f7c7}", "Rotate Tool [R] — Relocate origin & rotate"),
    (CanvasTool::Scale, "\u{f386}", "Scale Tool [S] — Relocate origin & scale"),
    (CanvasTool::Zoom, MacSymbol::Search.as_str(), "Zoom Tool [Z] — Click or marquee zoom"),
    (CanvasTool::Knife, "\u{f7c9}", "Knife Tool [E] — Slice across contours"),
    (CanvasTool::Primitives, "\u{f80c}", "Primitives Tool [F] — Rect / Circle"),
    (CanvasTool::Measurement, "\u{f7ca}", "Measurement Tool [L] — Intersections & lengths"),
    (CanvasTool::Annotation, "\u{f853}", "Annotation Tool [A] — Canvas notes & arrows"),
    (CanvasTool::Pencil, "\u{f7cb}", "Pencil Tool [B] — Freehand Bezier sketch"),
    (CanvasTool::Pixel, "\u{f804}", "Pixel Tool [X] — Stamp pixel grid cells"),
    (CanvasTool::Text, "\u{f892}", "Text Tool [T] — Multi-glyph strip & kerning"),
    (CanvasTool::TrueTypeInstructor, "\u{f48a}", "TrueType Instructor [I] — Visual hinting"),
    (CanvasTool::Hoi, "\u{f899}", "HOI Tool [H] — Non-linear master interpolation"),
    (CanvasTool::Stem, "\u{f500}", "Structural Stem Tool — Paired stem width servo"),
    (CanvasTool::CurvedTrajectory, "\u{f6e5}", "Curved Master Trajectory Tool"),
    (CanvasTool::Trowel, "\u{f6f4}", "Trowel Brush Tool — Local contour smoothing"),
    (CanvasTool::Cleaner, "\u{f80a}", "Cleaner Brush Tool — Excise micro-loops"),
    (CanvasTool::ShapeBuilder, "\u{f618}", "Shape Builder Tool — Planar region merge"),
];

#[rustfmt::skip]
pub const ALL_ACTION_ICONS: [&str; 22] = [
    "\u{f618}", MacSymbol::SidebarLeading.as_str(), "\u{f37d}", "\u{f544}", "\u{f549}", "\u{f7c5}",
    "\u{f7c7}", "\u{f500}", "\u{f52a}", "\u{f386}", "\u{f7da}", "\u{f4e3}", "\u{f53d}", "\u{f6f4}",
    "\u{f48a}", "\u{f899}", "\u{f892}", "\u{f80a}", "\u{f6e5}", "\u{f77e}", "\u{f424}", "\u{f4c8}",
];

/// Deterministically initializes `egui` fonts and registers `egui-macview` Cupertino vector icons.
pub fn configure_demo_fonts(ctx: &egui::Context) {
    let mut fonts = egui::FontDefinitions::default();
    #[cfg(not(target_arch = "wasm32"))]
    {
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
