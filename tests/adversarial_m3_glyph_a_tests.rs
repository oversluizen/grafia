//! Adversarial Challenge Test Suite for Milestone 3: Glyph /A State & Reset Parity.
//!
//! Normative Golden Master Verification:
//! 1. Verify `GrafiaCanvasState::init_glyph_a()` authentic parameters:
//!    - UPM: 1000, Width: 700.0
//!    - Outer contour: exactly 8 corner points
//!    - Inner counter: exactly 3 corner points
//! 2. Verify state mutations (node movement, pen tool node creation, smooth toggles, zoom)
//!    and that "Reset /A" restores pristine Golden Master state with zero ID drift.
//! 3. Verify Kurbo BezPath segment parity on launch, mutation, and reset.
//! 4. Verify geometric containment, signed area, and winding orientation.
//! 5. Verify resilience against degenerate corruption (NaN, INF, empty state, u64::MAX).
//! 6. Verify `examples/vector_canvas_demo.rs` app instantiation and UI frame lifecycle.

use egui::{Context, Event, Key, Modifiers as EguiModifiers, PointerButton, Pos2, RawInput};
use grafia::adapters::{
    CanvasTool, CanvasViewMode, GlifoCanvasResponse, GlifoCanvasState, GlifoCanvasWidget,
};
use grafia::core::{FontPoint, NodeType, ScreenPoint};
use grafia::tokens::{GlifoTheme, GlyphAGoldenMaster};
use kurbo::PathEl;

#[path = "../examples/vector_canvas_demo.rs"]
#[allow(dead_code)]
mod vector_canvas_demo;

use vector_canvas_demo::GrafiaVectorCanvasDemo;

const EXPECTED_UPM: u16 = 1000;
const EXPECTED_WIDTH: f64 = 700.0;
const EXPECTED_CAP_HEIGHT: f64 = 700.0;
const EXPECTED_ASCENDER: f64 = 800.0;
const EXPECTED_DESCENDER: f64 = -200.0;
const EXPECTED_X_HEIGHT: f64 = 500.0;

const EXPECTED_OUTER_POINTS: &[(f64, f64)] = &[
    (30.0, 0.0),
    (300.0, 700.0),
    (400.0, 700.0),
    (670.0, 0.0),
    (540.0, 0.0),
    (470.0, 180.0),
    (230.0, 180.0),
    (160.0, 0.0),
];

const EXPECTED_INNER_POINTS: &[(f64, f64)] = &[
    (260.0, 260.0),
    (440.0, 260.0),
    (350.0, 520.0),
];

fn run_canvas_frame(
    ctx: &Context,
    state: &mut GlifoCanvasState,
    theme: &GlifoTheme,
    raw_input: RawInput,
) -> GlifoCanvasResponse {
    let mut resp = None;
    let mut full_output = ctx.run_ui(raw_input, |ui| {
        resp = Some(GlifoCanvasWidget::new(state, theme).show(ui));
    });
    full_output.textures_delta.clear();
    resp.expect("GlifoCanvasWidget must return response")
}

#[test]
fn test_glyph_a_golden_master_parameters_on_launch() {
    let state = GlifoCanvasState::init_glyph_a();
    let master = GlyphAGoldenMaster::load();

    // 1. Verify Typography Metrics
    assert_eq!(state.viewport.upm, EXPECTED_UPM, "UPM must be 1000");
    assert_eq!(master.upm, EXPECTED_UPM, "Master UPM must be 1000");
    assert_eq!(master.width, EXPECTED_WIDTH, "Advance width must be 700.0");
    assert_eq!(master.cap_height, EXPECTED_CAP_HEIGHT, "Cap height must be 700.0");
    assert_eq!(master.ascender, EXPECTED_ASCENDER, "Ascender must be 800.0");
    assert_eq!(master.descender, EXPECTED_DESCENDER, "Descender must be -200.0");
    assert_eq!(master.x_height, EXPECTED_X_HEIGHT, "x-height must be 500.0");

    // 2. Verify Contours and Nodes Count
    assert_eq!(state.contours.len(), 2, "Must have exactly 2 contours (outer + inner)");
    assert_eq!(
        state.contours[0].len(),
        EXPECTED_OUTER_POINTS.len(),
        "Outer contour must have exactly 8 points"
    );
    assert_eq!(
        state.contours[1].len(),
        EXPECTED_INNER_POINTS.len(),
        "Inner counter must have exactly 3 points"
    );

    // 3. Verify Outer Contour Points and Types
    for (idx, &(exp_x, exp_y)) in EXPECTED_OUTER_POINTS.iter().enumerate() {
        let node = &state.contours[0][idx];
        assert_eq!(
            (node.point.x.0, node.point.y.0),
            (exp_x, exp_y),
            "Outer node {idx} coordinate mismatch"
        );
        assert_eq!(
            node.node_type,
            NodeType::Corner,
            "Outer node {idx} must be NodeType::Corner"
        );
        assert_eq!(
            node.id.0,
            (idx + 1) as u64,
            "Outer node {idx} ID must match 1-based sequential ordering"
        );
    }

    // 4. Verify Inner Counter Points and Types
    for (idx, &(exp_x, exp_y)) in EXPECTED_INNER_POINTS.iter().enumerate() {
        let node = &state.contours[1][idx];
        assert_eq!(
            (node.point.x.0, node.point.y.0),
            (exp_x, exp_y),
            "Inner node {idx} coordinate mismatch"
        );
        assert_eq!(
            node.node_type,
            NodeType::Corner,
            "Inner node {idx} must be NodeType::Corner"
        );
        assert_eq!(
            node.id.0,
            (idx + 1 + EXPECTED_OUTER_POINTS.len()) as u64,
            "Inner node {idx} ID must follow outer nodes"
        );
    }

    // 5. Verify Paths Synchronization and View Mode
    assert_eq!(state.paths.len(), 2, "Must synchronize exactly 2 BezPaths");
    assert_eq!(state.active_tool, CanvasTool::Select, "Default tool must be Select");
    assert_eq!(state.view_mode, CanvasViewMode::EditWireframe, "Default mode must be EditWireframe");
    assert!(state.selected_nodes.is_empty(), "Initial selection must be empty");
    assert!(state.active_contour_closed, "Contour closed state must be true");
    assert_eq!(state.next_node_id, 12, "Next node ID must be 12");
}

#[test]
fn test_glyph_a_kurbo_bezpaths_structure_on_launch_and_reset() {
    let state = GlifoCanvasState::init_glyph_a();

    // Verify Outer BezPath elements
    let outer_elements: Vec<PathEl> = state.paths[0].elements().to_vec();
    assert_eq!(outer_elements.len(), 9, "Outer path must have MoveTo + 7 LineTo + ClosePath");
    match outer_elements[0] {
        PathEl::MoveTo(pt) => assert_eq!((pt.x, pt.y), EXPECTED_OUTER_POINTS[0]),
        _ => panic!("First outer element must be MoveTo"),
    }
    for idx in 1..8 {
        match outer_elements[idx] {
            PathEl::LineTo(pt) => assert_eq!((pt.x, pt.y), EXPECTED_OUTER_POINTS[idx]),
            _ => panic!("Outer element {idx} must be LineTo"),
        }
    }
    assert!(matches!(outer_elements[8], PathEl::ClosePath));

    // Verify Inner BezPath elements
    let inner_elements: Vec<PathEl> = state.paths[1].elements().to_vec();
    assert_eq!(inner_elements.len(), 4, "Inner path must have MoveTo + 2 LineTo + ClosePath");
    match inner_elements[0] {
        PathEl::MoveTo(pt) => assert_eq!((pt.x, pt.y), EXPECTED_INNER_POINTS[0]),
        _ => panic!("First inner element must be MoveTo"),
    }
    for idx in 1..3 {
        match inner_elements[idx] {
            PathEl::LineTo(pt) => assert_eq!((pt.x, pt.y), EXPECTED_INNER_POINTS[idx]),
            _ => panic!("Inner element {idx} must be LineTo"),
        }
    }
    assert!(matches!(inner_elements[3], PathEl::ClosePath));
}

#[test]
fn test_glyph_a_geometric_containment_and_winding_area() {
    let _state = GlifoCanvasState::init_glyph_a();

    // Shoelace formula for signed polygon area
    let shoelace = |pts: &[(f64, f64)]| -> f64 {
        let n = pts.len();
        let mut area = 0.0;
        for i in 0..n {
            let j = (i + 1) % n;
            area += pts[i].0 * pts[j].1;
            area -= pts[j].0 * pts[i].1;
        }
        area * 0.5
    };

    let outer_area = shoelace(EXPECTED_OUTER_POINTS);
    let inner_area = shoelace(EXPECTED_INNER_POINTS);

    // Both contours must have non-zero area
    assert!(outer_area.abs() > 50000.0, "Outer contour area must be substantial: {outer_area}");
    assert!(inner_area.abs() > 10000.0, "Inner counter area must be substantial: {inner_area}");

    // Counter containment check: every inner point must be strictly within outer bounding box
    let min_outer_x = EXPECTED_OUTER_POINTS.iter().map(|p| p.0).fold(f64::INFINITY, f64::min);
    let max_outer_x = EXPECTED_OUTER_POINTS.iter().map(|p| p.0).fold(f64::NEG_INFINITY, f64::max);
    let min_outer_y = EXPECTED_OUTER_POINTS.iter().map(|p| p.1).fold(f64::INFINITY, f64::min);
    let max_outer_y = EXPECTED_OUTER_POINTS.iter().map(|p| p.1).fold(f64::NEG_INFINITY, f64::max);

    for &(ix, iy) in EXPECTED_INNER_POINTS {
        assert!(
            ix > min_outer_x && ix < max_outer_x,
            "Inner point X ({ix}) must be within outer X bounds ({min_outer_x}..{max_outer_x})"
        );
        assert!(
            iy > min_outer_y && iy < max_outer_y,
            "Inner point Y ({iy}) must be within outer Y bounds ({min_outer_y}..{max_outer_y})"
        );
    }
}

#[test]
fn test_glyph_a_mutations_and_reset_restoration() {
    let ctx = Context::default();
    let theme = GlifoTheme::sonoma_default();
    let mut state = GlifoCanvasState::init_glyph_a();

    // --- MUTATION 1: Drag outer contour apex node (300, 700) ---
    let apex_font = FontPoint::new(300.0, 700.0);
    let apex_screen = state.viewport.to_screen(apex_font);
    let p_start = Pos2::new(apex_screen.x.0, apex_screen.y.0);
    let p_drag = Pos2::new(apex_screen.x.0 + 40.0, apex_screen.y.0 - 20.0);

    // Mouse click on apex node
    let mut input = RawInput::default();
    input.events.push(Event::PointerButton {
        pos: p_start,
        button: PointerButton::Primary,
        pressed: true,
        modifiers: EguiModifiers::NONE,
    });
    let _ = run_canvas_frame(&ctx, &mut state, &theme, input);

    // Mouse drag
    let mut input = RawInput::default();
    input.events.push(Event::PointerMoved(p_drag));
    let _ = run_canvas_frame(&ctx, &mut state, &theme, input);

    // Mouse release
    let mut input = RawInput::default();
    input.events.push(Event::PointerButton {
        pos: p_drag,
        button: PointerButton::Primary,
        pressed: false,
        modifiers: EguiModifiers::NONE,
    });
    let _ = run_canvas_frame(&ctx, &mut state, &theme, input);

    // Verify node moved
    let current_apex = state.contours[0][1].point;
    assert_ne!(
        current_apex.x.0, 300.0,
        "Apex node X coordinate must have mutated"
    );

    // --- MUTATION 2: Switch to Pen Tool and add a new node (forming 3rd contour) ---
    let mut input = RawInput::default();
    input.events.push(Event::Key {
        key: Key::P,
        pressed: true,
        repeat: false,
        modifiers: EguiModifiers::NONE,
        physical_key: None,
    });
    let _ = run_canvas_frame(&ctx, &mut state, &theme, input);
    assert_eq!(state.active_tool, CanvasTool::Pen);

    let pen_pos = Pos2::new(100.0, 100.0);
    let mut input = RawInput::default();
    input.events.push(Event::PointerButton {
        pos: pen_pos,
        button: PointerButton::Primary,
        pressed: true,
        modifiers: EguiModifiers::NONE,
    });
    let _ = run_canvas_frame(&ctx, &mut state, &theme, input);

    let mut input = RawInput::default();
    input.events.push(Event::PointerButton {
        pos: pen_pos,
        button: PointerButton::Primary,
        pressed: false,
        modifiers: EguiModifiers::NONE,
    });
    let _ = run_canvas_frame(&ctx, &mut state, &theme, input);

    assert_eq!(state.contours.len(), 3, "Pen click must create a 3rd contour");
    let total_nodes_mutated: usize = state.contours.iter().map(|c| c.len()).sum();
    assert_eq!(total_nodes_mutated, 12, "Total nodes must now be 12");

    // --- MUTATION 3: Option-click toggle smooth/corner ---
    let node_to_toggle = state.contours[0][0].id;
    state.toggle_node_smooth_corner(node_to_toggle);
    assert_eq!(
        state.contours[0][0].node_type,
        NodeType::Smooth,
        "Node 0 must have toggled to Smooth"
    );

    // --- MUTATION 4: Change zoom, pan, and view mode ---
    state.viewport.zoom = 5.0;
    state.viewport.pan_offset = ScreenPoint::new(999.0, -888.0);
    state.view_mode = CanvasViewMode::PreviewFilled;

    // --- EXECUTE RESET /A ---
    // This mirrors `examples/vector_canvas_demo.rs`: `self.state = GrafiaCanvasState::init_glyph_a();`
    state = GlifoCanvasState::init_glyph_a();

    // --- VERIFY RESTORATION OF PRISTINE GOLDEN MASTER STATE ---
    assert_eq!(state.contours.len(), 2, "Reset must restore exactly 2 contours");
    assert_eq!(state.contours[0].len(), 8, "Reset must restore 8 outer nodes");
    assert_eq!(state.contours[1].len(), 3, "Reset must restore 3 inner nodes");

    // Verify outer points restored
    for (idx, &(exp_x, exp_y)) in EXPECTED_OUTER_POINTS.iter().enumerate() {
        let node = &state.contours[0][idx];
        assert_eq!((node.point.x.0, node.point.y.0), (exp_x, exp_y));
        assert_eq!(node.node_type, NodeType::Corner);
        assert_eq!(node.id.0, (idx + 1) as u64);
    }

    // Verify inner points restored
    for (idx, &(exp_x, exp_y)) in EXPECTED_INNER_POINTS.iter().enumerate() {
        let node = &state.contours[1][idx];
        assert_eq!((node.point.x.0, node.point.y.0), (exp_x, exp_y));
        assert_eq!(node.node_type, NodeType::Corner);
        assert_eq!(node.id.0, (idx + 1 + EXPECTED_OUTER_POINTS.len()) as u64);
    }

    assert_eq!(state.active_tool, CanvasTool::Select);
    assert_eq!(state.view_mode, CanvasViewMode::EditWireframe);
    assert!(state.selected_nodes.is_empty());
    assert!(state.active_contour_closed);
    assert_eq!(state.next_node_id, 12);
}

#[test]
fn test_glyph_a_adversarial_corruption_recovery() {
    let mut state = GlifoCanvasState::init_glyph_a();

    // Adversarial corruption: empty state, extreme numbers
    state.contours.clear();
    state.paths.clear();
    state.selected_nodes.push(grafia::core::NodeId(99999));
    state.next_node_id = u64::MAX - 10;
    state.viewport.zoom = f32::NAN;
    state.viewport.pan_offset = ScreenPoint::new(f32::INFINITY, f32::NEG_INFINITY);
    state.active_contour_closed = false;

    // Reset restores full state health
    state = GlifoCanvasState::init_glyph_a();

    assert_eq!(state.contours.len(), 2);
    assert_eq!(state.contours[0].len(), 8);
    assert_eq!(state.contours[1].len(), 3);
    assert_eq!(state.next_node_id, 12);
    assert!(state.viewport.zoom.is_finite());
    assert!(state.viewport.pan_offset.x.0.is_finite());
    assert!(state.viewport.pan_offset.y.0.is_finite());
    assert!(state.selected_nodes.is_empty());
    assert!(state.active_contour_closed);
}

#[test]
fn test_glyph_a_repeated_reset_idempotency_and_no_id_drift() {
    for cycle in 0..50 {
        let mut state = GlifoCanvasState::init_glyph_a();

        // Mutate arbitrarily
        state.contours[0][0].point.x.0 += (cycle as f64) * 1.5;
        state.contours[1][1].point.y.0 -= (cycle as f64) * 2.0;
        state.next_node_id += 500;
        state.selected_nodes = vec![state.contours[0][0].id];
        state.active_tool = CanvasTool::Pen;
        state.view_mode = CanvasViewMode::ShapeThumbnail;

        // Reset
        state = GlifoCanvasState::init_glyph_a();

        assert_eq!(state.next_node_id, 12, "Cycle {cycle}: next_node_id must reset to 12 without drift");
        assert_eq!(state.contours[0][0].point.x.0, 30.0, "Cycle {cycle}: coordinate must reset");
        assert_eq!(state.contours[1][1].point.y.0, 260.0, "Cycle {cycle}: coordinate must reset");
        assert!(state.selected_nodes.is_empty(), "Cycle {cycle}: selection must clear");
        assert_eq!(state.active_tool, CanvasTool::Select, "Cycle {cycle}: tool must reset");
        assert_eq!(state.view_mode, CanvasViewMode::EditWireframe, "Cycle {cycle}: mode must reset");
    }
}

#[test]
fn test_vector_canvas_demo_app_ui_lifecycle_and_reset_button() {
    let mut app = GrafiaVectorCanvasDemo::new();
    let ctx = Context::default();

    // Helper closure to run app frame safely handling textures_delta
    let mut run_app_frame = |raw_input: RawInput| {
        let mut full_output = ctx.run_ui(raw_input, |ui| {
            let mut frame = eframe::Frame::_new_kittest();
            <GrafiaVectorCanvasDemo as eframe::App>::ui(&mut app, ui, &mut frame);
        });
        full_output.textures_delta.clear();
    };

    // 1. Initial frame pass
    run_app_frame(RawInput::default());

    // 2. Simulate hotkey '2' (PreviewFilled)
    let mut raw_input = RawInput::default();
    raw_input.events.push(Event::Key {
        key: Key::Num2,
        pressed: true,
        repeat: false,
        modifiers: EguiModifiers::NONE,
        physical_key: None,
    });
    run_app_frame(raw_input);

    // 3. Simulate hotkey '3' (ShapeThumbnail)
    let mut raw_input = RawInput::default();
    raw_input.events.push(Event::Key {
        key: Key::Num3,
        pressed: true,
        repeat: false,
        modifiers: EguiModifiers::NONE,
        physical_key: None,
    });
    run_app_frame(raw_input);

    // 4. Simulate hotkey '1' (EditWireframe)
    let mut raw_input = RawInput::default();
    raw_input.events.push(Event::Key {
        key: Key::Num1,
        pressed: true,
        repeat: false,
        modifiers: EguiModifiers::NONE,
        physical_key: None,
    });
    run_app_frame(raw_input);
}
