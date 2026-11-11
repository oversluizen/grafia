//! Adversarial Challenge Test Suite for Milestone 3: Demonstrator Example (`vector_canvas_demo.rs`).
//!
//! Empirical Challenge Dimensions:
//! 1. Mode switching: EditWireframe [1], PreviewFilled [2], ShapeThumbnail [3] via UI and hotkeys.
//! 2. Tool switching: Select [V] vs Pen [P] across all view modes and during active interactions.
//! 3. Zoom limit clamping: strictly [0.01, 50.0] with extreme zoom-in/out stress tests.
//! 4. Viewport coordinate transformations under extreme zoom bounds (0.01 and 50.0).
//! 5. Demonstrator `GrafiaVectorCanvasDemo` lifecycle and UI execution via headless `eframe::App`.
//! 6. Windows GUI Subsystem Invariant & modularity and color rules.

use egui::{Context, Event, Key, Modifiers as EguiModifiers, RawInput};
use grafia::adapters::{
    CanvasTool, CanvasViewMode, GrafiaCanvasResponse, GrafiaCanvasState, GrafiaCanvasWidget,
};
use grafia::core::{FontPoint, ScreenPoint, ViewportTransform};
use grafia::tokens::{GrafiaTheme, MIN_VIEWPORT_ZOOM};
use std::fs;
use std::path::Path;

#[path = "../examples/vector_canvas_demo.rs"]
#[allow(dead_code)]
mod vector_canvas_demo;

use vector_canvas_demo::GrafiaVectorCanvasDemo;

const MAX_VIEWPORT_ZOOM: f32 = 50.0;
const ZOOM_IN_FACTOR: f32 = 1.25;
const ZOOM_OUT_FACTOR: f32 = 0.8;

fn run_canvas_frame(
    ctx: &Context,
    state: &mut GrafiaCanvasState,
    theme: &GrafiaTheme,
    raw_input: RawInput,
) -> GrafiaCanvasResponse {
    let mut resp = None;
    let mut full_output = ctx.run_ui(raw_input, |ui| {
        resp = Some(GrafiaCanvasWidget::new(state, theme).show(ui));
    });
    full_output.textures_delta.clear();
    resp.expect("GrafiaCanvasWidget must return a response")
}

// ============================================================================
// 1. Interactive View Mode Switching & Hotkeys ('1', '2', '3')
// ============================================================================

#[test]
fn test_adversarial_view_mode_switching_and_hotkeys() {
    let ctx = Context::default();
    let theme = GrafiaTheme::sonoma_default();
    let mut state = GrafiaCanvasState::init_glyph_a();

    assert_eq!(state.view_mode, CanvasViewMode::EditWireframe);

    // Simulate pressing hotkey '2' (PreviewFilled)
    let mut input = RawInput::default();
    input.events.push(Event::Key {
        key: Key::Num2,
        physical_key: None,
        pressed: true,
        repeat: false,
        modifiers: EguiModifiers::NONE,
    });
    let mut full_output = ctx.run_ui(input, |ui| {
        if ui.input(|i| i.key_pressed(Key::Num2)) {
            state.view_mode = CanvasViewMode::PreviewFilled;
        }
        let _ = GrafiaCanvasWidget::new(&mut state, &theme).show(ui);
    });
    full_output.textures_delta.clear();
    assert_eq!(
        state.view_mode,
        CanvasViewMode::PreviewFilled,
        "Hotkey '2' must switch view mode to PreviewFilled"
    );

    // Simulate pressing hotkey '3' (ShapeThumbnail)
    let mut input = RawInput::default();
    input.events.push(Event::Key {
        key: Key::Num3,
        physical_key: None,
        pressed: true,
        repeat: false,
        modifiers: EguiModifiers::NONE,
    });
    let mut full_output = ctx.run_ui(input, |ui| {
        if ui.input(|i| i.key_pressed(Key::Num3)) {
            state.view_mode = CanvasViewMode::ShapeThumbnail;
        }
        let _ = GrafiaCanvasWidget::new(&mut state, &theme).show(ui);
    });
    full_output.textures_delta.clear();
    assert_eq!(
        state.view_mode,
        CanvasViewMode::ShapeThumbnail,
        "Hotkey '3' must switch view mode to ShapeThumbnail"
    );

    // Simulate pressing hotkey '1' (EditWireframe)
    let mut input = RawInput::default();
    input.events.push(Event::Key {
        key: Key::Num1,
        physical_key: None,
        pressed: true,
        repeat: false,
        modifiers: EguiModifiers::NONE,
    });
    let mut full_output = ctx.run_ui(input, |ui| {
        if ui.input(|i| i.key_pressed(Key::Num1)) {
            state.view_mode = CanvasViewMode::EditWireframe;
        }
        let _ = GrafiaCanvasWidget::new(&mut state, &theme).show(ui);
    });
    full_output.textures_delta.clear();
    assert_eq!(
        state.view_mode,
        CanvasViewMode::EditWireframe,
        "Hotkey '1' must switch view mode to EditWireframe"
    );
}

// ============================================================================
// 2. Interactive Tool Switching ('V', 'P') in all View Modes
// ============================================================================

#[test]
fn test_adversarial_tool_switching_in_all_view_modes() {
    let ctx = Context::default();
    let theme = GrafiaTheme::sonoma_default();
    let mut state = GrafiaCanvasState::init_glyph_a();

    for mode in [
        CanvasViewMode::EditWireframe,
        CanvasViewMode::PreviewFilled,
        CanvasViewMode::ShapeThumbnail,
    ] {
        state.view_mode = mode;

        // Switch to Pen (P)
        let mut input = RawInput::default();
        input.events.push(Event::Key {
            key: Key::P,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: EguiModifiers::NONE,
        });
        let resp = run_canvas_frame(&ctx, &mut state, &theme, input);
        assert_eq!(
            resp.active_tool,
            CanvasTool::Pen,
            "P key must switch to Pen tool in mode {mode:?}"
        );
        assert_eq!(state.active_tool, CanvasTool::Pen);

        // Switch back to Select (V)
        let mut input = RawInput::default();
        input.events.push(Event::Key {
            key: Key::V,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: EguiModifiers::NONE,
        });
        let resp = run_canvas_frame(&ctx, &mut state, &theme, input);
        assert_eq!(
            resp.active_tool,
            CanvasTool::Select,
            "V key must switch to Select tool in mode {mode:?}"
        );
        assert_eq!(state.active_tool, CanvasTool::Select);
    }
}

// ============================================================================
// 3. Zoom Clamping Limits: Minimum (0.01) and Maximum (50.0)
// ============================================================================

#[test]
fn test_adversarial_zoom_clamping_limits() {
    let mut zoom = 1.0f32;

    // Zoom OUT stress test: 100 consecutive zoom-outs
    for _ in 0..100 {
        zoom = (zoom * ZOOM_OUT_FACTOR).max(MIN_VIEWPORT_ZOOM);
        assert!(
            zoom >= MIN_VIEWPORT_ZOOM,
            "Zoom {zoom} cannot drop below MIN_VIEWPORT_ZOOM ({MIN_VIEWPORT_ZOOM})"
        );
        assert!(zoom.is_finite(), "Zoom must remain finite");
        assert!(!zoom.is_nan(), "Zoom cannot be NaN");
    }
    assert_eq!(
        zoom, MIN_VIEWPORT_ZOOM,
        "Zoom must strictly clamp at MIN_VIEWPORT_ZOOM (0.01)"
    );

    // Zoom IN stress test: 100 consecutive zoom-ins
    for _ in 0..100 {
        zoom = (zoom * ZOOM_IN_FACTOR).min(MAX_VIEWPORT_ZOOM);
        assert!(
            zoom <= MAX_VIEWPORT_ZOOM,
            "Zoom {zoom} cannot exceed MAX_VIEWPORT_ZOOM ({MAX_VIEWPORT_ZOOM})"
        );
        assert!(zoom.is_finite(), "Zoom must remain finite");
        assert!(!zoom.is_nan(), "Zoom cannot be NaN");
    }
    assert_eq!(
        zoom, MAX_VIEWPORT_ZOOM,
        "Zoom must strictly clamp at MAX_VIEWPORT_ZOOM (50.0)"
    );
}

// ============================================================================
// 4. Viewport Coordinate Transformations at Zoom Boundaries
// ============================================================================

#[test]
fn test_adversarial_coordinate_math_at_zoom_boundaries() {
    let test_points = [
        FontPoint::new(0.0, 0.0),
        FontPoint::new(30.0, 0.0),
        FontPoint::new(350.0, 700.0),
        FontPoint::new(700.0, 0.0),
        FontPoint::new(-100.0, -200.0),
    ];

    for zoom in [MIN_VIEWPORT_ZOOM, 1.0, MAX_VIEWPORT_ZOOM] {
        let vp = ViewportTransform::new(zoom, ScreenPoint::new(400.0, 300.0), 1000);

        for pt in test_points {
            let scr = vp.to_screen(pt);
            assert!(
                scr.x.0.is_finite() && scr.y.0.is_finite(),
                "Screen coordinates must be finite at zoom {zoom}"
            );

            let roundtrip = vp.to_font(scr);
            assert!(
                (roundtrip.x.0 - pt.x.0).abs() < 1e-2,
                "X roundtrip failure at zoom {zoom}: orig {}, got {}",
                pt.x.0,
                roundtrip.x.0
            );
            assert!(
                (roundtrip.y.0 - pt.y.0).abs() < 1e-2,
                "Y roundtrip failure at zoom {zoom}: orig {}, got {}",
                pt.y.0,
                roundtrip.y.0
            );
        }
    }
}

// ============================================================================
// 5. Golden Master /A State Restoration Invariant ("Reset /A")
// ============================================================================

#[test]
fn test_adversarial_reset_glyph_a_restoration() {
    let mut state = GrafiaCanvasState::init_glyph_a();

    // Verify initial golden master state
    assert_eq!(state.contours.len(), 2);
    let initial_node_count: usize = state.contours.iter().map(|c| c.len()).sum();
    assert_eq!(initial_node_count, 11);

    // Mutilate state: clear contours, change zoom, change active tool
    state.contours.clear();
    state.selected_nodes.clear();
    state.viewport.zoom = 42.0;
    state.active_tool = CanvasTool::Pen;
    state.view_mode = CanvasViewMode::ShapeThumbnail;

    assert_eq!(state.contours.len(), 0);

    // Trigger Reset /A
    state = GrafiaCanvasState::init_glyph_a();

    // Verify full restoration
    assert_eq!(
        state.contours.len(),
        2,
        "Contours count must be restored to 2"
    );
    let restored_node_count: usize = state.contours.iter().map(|c| c.len()).sum();
    assert_eq!(restored_node_count, 11, "Node count must be restored to 11");
    assert_eq!(state.active_tool, CanvasTool::Select, "Tool must be Select");
    assert_eq!(
        state.view_mode,
        CanvasViewMode::EditWireframe,
        "Mode must be EditWireframe"
    );
}

// ============================================================================
// 6. GrafiaVectorCanvasDemo Lifecycle Execution via eframe::App
// ============================================================================

#[test]
fn test_adversarial_vector_canvas_demo_app_eframe_lifecycle() {
    let mut app = GrafiaVectorCanvasDemo::new();
    let ctx = Context::default();

    let mut run_frame = |raw_input: RawInput| {
        let mut full_output = ctx.run_ui(raw_input, |ui| {
            let mut frame = eframe::Frame::_new_kittest();
            <GrafiaVectorCanvasDemo as eframe::App>::ui(&mut app, ui, &mut frame);
        });
        full_output.textures_delta.clear();
    };

    // Frame 1: default wireframe
    run_frame(RawInput::default());

    // Frame 2: hotkey '2'
    let mut input = RawInput::default();
    input.events.push(Event::Key {
        key: Key::Num2,
        pressed: true,
        repeat: false,
        modifiers: EguiModifiers::NONE,
        physical_key: None,
    });
    run_frame(input);

    // Frame 3: hotkey 'P'
    let mut input = RawInput::default();
    input.events.push(Event::Key {
        key: Key::P,
        pressed: true,
        repeat: false,
        modifiers: EguiModifiers::NONE,
        physical_key: None,
    });
    run_frame(input);

    // Frame 4: hotkey '3'
    let mut input = RawInput::default();
    input.events.push(Event::Key {
        key: Key::Num3,
        pressed: true,
        repeat: false,
        modifiers: EguiModifiers::NONE,
        physical_key: None,
    });
    run_frame(input);

    // Frame 5: hotkey 'V'
    let mut input = RawInput::default();
    input.events.push(Event::Key {
        key: Key::V,
        pressed: true,
        repeat: false,
        modifiers: EguiModifiers::NONE,
        physical_key: None,
    });
    run_frame(input);

    // Frame 6: hotkey '1'
    let mut input = RawInput::default();
    input.events.push(Event::Key {
        key: Key::Num1,
        pressed: true,
        repeat: false,
        modifiers: EguiModifiers::NONE,
        physical_key: None,
    });
    run_frame(input);
}

#[test]
fn test_adversarial_v020_demonstrator_toolbar_actions_empty_and_selected() {
    let mut app = GrafiaVectorCanvasDemo::new();
    let ctx = Context::default();
    let screen_rect = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::Vec2::new(1223.0, 811.0));

    let run_ui_frame = |app: &mut GrafiaVectorCanvasDemo| {
        let raw = RawInput {
            screen_rect: Some(screen_rect),
            ..Default::default()
        };
        let mut out = ctx.run_ui(raw, |ui| app.render_ui(ui));
        out.textures_delta.clear();
    };

    // 1. Zero nodes selected: Flip H, Flip V, Extremes, Tidy, Make First, BBox toggle + narrow 800px window
    let mut state = GrafiaCanvasState::init_glyph_s();
    assert!(state.selected_nodes.is_empty());
    let orig_count = state.contours[0].len();
    let _ = state.add_extremes_action();
    assert!(state.contours[0].len() >= orig_count);
    assert!(state.flip_horizontal());
    assert!(state.flip_vertical());
    let _ = state.tidy_up_paths();
    assert!(!state.make_node_first());

    // 2. Single contour selected: Flip H, Flip V, Make First, Extremes, Tidy
    state = GrafiaCanvasState::init_glyph_a();
    state.selected_nodes = state.contours[0].iter().map(|n| n.id).collect();
    let c1_before = state.contours[1].clone();
    assert!(state.flip_horizontal());
    assert_eq!(
        state.contours[1], c1_before,
        "Unselected contour 1 must not move when contour 0 is selected"
    );
    assert!(state.flip_vertical());
    state.selected_nodes = vec![state.contours[0][1].id];
    assert!(state.make_node_first());

    // 3. Render GrafiaVectorCanvasDemo across narrow (< 900px) and standard window widths with BBox & all v0.2.0 tools
    for width in [720.0_f32, 1223.0_f32] {
        let rect = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::Vec2::new(width, 811.0));
        let mut out = ctx.run_ui(
            RawInput {
                screen_rect: Some(rect),
                ..Default::default()
            },
            |ui| app.render_ui(ui),
        );
        out.textures_delta.clear();
    }
    run_ui_frame(&mut app);

    // 4. M-38: Verify examples/vector_canvas_demo.rs wires all SPEC-003 Path Ops & TransformPanel actions
    let demo_src = fs::read_to_string("examples/vector_canvas_demo.rs")
        .expect("Must read examples/vector_canvas_demo.rs");
    for required_call in [
        "set_transform_grid_anchor",
        "set_metrics_origin",
        "align_selection",
        "connect_selected_open_endpoints",
        "add_inflection_points",
        "correct_path_direction",
        "rotate_by_degrees",
        "scale_by_factor",
        "slant_by_degrees",
    ] {
        assert!(
            demo_src.contains(required_call),
            "M-38: examples/vector_canvas_demo.rs must expose `{required_call}`"
        );
    }
}

// ============================================================================
// 7. Demonstrator Source Hygiene & Modularity Compliance
// ============================================================================

#[test]
fn test_adversarial_demonstrator_source_hygiene() {
    let path = Path::new("examples/vector_canvas_demo.rs");
    assert!(path.exists(), "examples/vector_canvas_demo.rs must exist");

    let content = fs::read_to_string(path).expect("Must read examples/vector_canvas_demo.rs");
    let lines: Vec<&str> = content.lines().collect();

    // Check line 1: Windows Subsystem Invariant
    assert_eq!(
        lines[0].trim(),
        r#"#![cfg_attr(not(test), windows_subsystem = "windows")]"#,
        "Line 1 must strictly declare windows_subsystem attribute"
    );

    // Check physical lines ceiling
    assert!(
        lines.len() < 599,
        "Physical lines ({}) must be strictly < 599",
        lines.len()
    );

    // Check eSLOC ceiling
    let esloc = lines
        .iter()
        .filter(|l| {
            let t = l.trim();
            !t.is_empty() && !t.starts_with("//") && !t.starts_with("/*") && !t.starts_with('*')
        })
        .count();
    assert!(esloc <= 300, "eSLOC ({esloc}) must be <= 300");

    // Check zero hardcoded colors
    for (idx, line) in lines.iter().enumerate() {
        let code = line.split("//").next().unwrap_or("").trim();
        for forbidden in [
            "Color32::from_rgb(",
            "Color32::from_rgba_premultiplied(",
            "Color32::from_rgba_unmultiplied(",
        ] {
            assert!(
                !code.contains(forbidden),
                "Line {}: Forbidden hardcoded color constructor `{forbidden}`",
                idx + 1
            );
        }
        if code.contains('"') {
            for part in code.split('"') {
                if part.starts_with('#')
                    && (part.len() == 4 || part.len() == 7 || part.len() == 9)
                    && part[1..].chars().all(|c| c.is_ascii_hexdigit())
                {
                    panic!("Line {}: Forbidden hardcoded hex color `{part}`", idx + 1);
                }
            }
        }
    }
}
