//! Integration tests for `GlifoCanvasWidget` and `grafia::adapters` (STD-ARCH Verankeringswet).
//!
//! Tests:
//! 1. Tool switching: `V` for SelectTool, `P` for PenTool.
//! 2. Node hit-testing and selection: clicking a node selects it, dragging moves it,
//!    Shift constrains displacement to 45° angle snapping.
//! 3. Option-click toggles smooth / corner node types.
//! 4. Marquee drag selection creates a selection rectangle and selects intersecting nodes.
//! 5. Pen tool adds nodes on canvas click and creates symmetric handles on drag.
//! 6. View mode switching between `EditWireframe`, `PreviewFilled`, and `ShapeThumbnail`.

use egui::{Context, Event, Key, Modifiers as EguiModifiers, PointerButton, Pos2, RawInput, Vec2};
use grafia::adapters::{CanvasTool, CanvasViewMode, GlifoCanvasResponse, GlifoCanvasState, GlifoCanvasWidget};
use grafia::core::{FontPoint, NodeType};
use grafia::tokens::GlifoTheme;

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
    resp.expect("GlifoCanvasWidget must return a response")
}

#[test]
fn test_tool_switching_hotkeys_v_and_p() {
    let ctx = Context::default();
    let theme = GlifoTheme::sonoma_default();
    let mut state = GlifoCanvasState::init_glyph_a();

    assert_eq!(state.active_tool, CanvasTool::Select);

    // Press 'P' to switch to Pen tool
    let mut input = RawInput::default();
    input.events.push(Event::Key {
        key: Key::P,
        physical_key: None,
        pressed: true,
        repeat: false,
        modifiers: EguiModifiers::NONE,
    });
    let _ = run_canvas_frame(&ctx, &mut state, &theme, input);
    assert_eq!(state.active_tool, CanvasTool::Pen, "P key must switch to Pen tool");

    // Press 'V' to switch back to Select tool
    let mut input = RawInput::default();
    input.events.push(Event::Key {
        key: Key::V,
        physical_key: None,
        pressed: true,
        repeat: false,
        modifiers: EguiModifiers::NONE,
    });
    let _ = run_canvas_frame(&ctx, &mut state, &theme, input);
    assert_eq!(state.active_tool, CanvasTool::Select, "V key must switch to Select tool");
}

#[test]
fn test_node_hit_testing_selection_and_drag_with_shift_snap() {
    let ctx = Context::default();
    let theme = GlifoTheme::sonoma_default();
    let mut state = GlifoCanvasState::init_glyph_a();

    // Verify initial node 0 position (outer contour start at font (30, 0))
    let first_node = &state.contours[0][0];
    let node_id = first_node.id;
    let initial_font_pt = first_node.point;
    let screen_pt = state.viewport.to_screen(initial_font_pt);
    let click_pos = Pos2::new(screen_pt.x.0, screen_pt.y.0);

    // Initial state: no nodes selected
    assert!(state.selected_nodes.is_empty());

    // 1. Click on the node to select it
    let mut input = RawInput::default();
    input.events.push(Event::PointerButton {
        pos: click_pos,
        button: PointerButton::Primary,
        pressed: true,
        modifiers: EguiModifiers::NONE,
    });
    let _ = run_canvas_frame(&ctx, &mut state, &theme, input);

    assert_eq!(state.selected_nodes, vec![node_id], "Clicking node must select it");

    // 2. Drag the node with mouse move
    // Drag towards arbitrary angle (e.g. delta (+60, -35) px)
    let drag_pos = click_pos + Vec2::new(60.0, -35.0);
    let mut input = RawInput::default();
    input.events.push(Event::PointerMoved(drag_pos));
    let _ = run_canvas_frame(&ctx, &mut state, &theme, input);

    let moved_node = state.find_node(node_id).expect("Node must exist");
    assert_ne!(
        moved_node.point, initial_font_pt,
        "Dragging selected node must move its font coordinate"
    );

    // 3. Drag with Shift held: displacement must be snapped to 45° angle increments
    let mut state = GlifoCanvasState::init_glyph_a();
    state.selected_nodes = vec![node_id];
    let start_screen = state.viewport.to_screen(state.find_node(node_id).unwrap().point);
    let start_pos = Pos2::new(start_screen.x.0, start_screen.y.0);

    // Move to 30 degrees direction (e.g. dx = 86.6, dy = -50.0).
    // Snapping to 45 degrees should snap to 45° (or nearest multiple).
    let shift_modifiers = EguiModifiers {
        shift: true,
        ..Default::default()
    };
    let shift_target_pos = start_pos + Vec2::new(100.0, -20.0);

    // Mouse down at start
    let mut input = RawInput::default();
    input.events.push(Event::PointerButton {
        pos: start_pos,
        button: PointerButton::Primary,
        pressed: true,
        modifiers: shift_modifiers,
    });
    let _ = run_canvas_frame(&ctx, &mut state, &theme, input);

    // Mouse drag with Shift
    let mut input = RawInput::default();

    input.events.push(Event::PointerMoved(shift_target_pos));
    let _ = run_canvas_frame(&ctx, &mut state, &theme, input);

    // Release mouse
    let mut input = RawInput::default();
    input.events.push(Event::PointerButton {
        pos: shift_target_pos,
        button: PointerButton::Primary,
        pressed: false,
        modifiers: shift_modifiers,
    });
    let _ = run_canvas_frame(&ctx, &mut state, &theme, input);

    let snapped_node = state.find_node(node_id).expect("Node must exist");
    let font_dx = (snapped_node.point.x.0 - initial_font_pt.x.0).round();
    let font_dy = (snapped_node.point.y.0 - initial_font_pt.y.0).round();

    // Verify angle is a multiple of 45 degrees
    if font_dx.abs() > 1e-4 || font_dy.abs() > 1e-4 {
        let angle_deg = font_dy.atan2(font_dx).to_degrees().rem_euclid(360.0);
        let remainder = (angle_deg % 45.0).min(45.0 - (angle_deg % 45.0));
        assert!(
            remainder < 1.0,
            "Displacement vector with Shift must snap to 45° angle. Angle was {angle_deg}°, remainder {remainder}"
        );
    }
}

#[test]
fn test_option_click_toggles_smooth_corner_node_type() {
    let ctx = Context::default();
    let theme = GlifoTheme::sonoma_default();
    let mut state = GlifoCanvasState::init_glyph_a();

    let node_id = state.contours[0][0].id;
    assert_eq!(
        state.find_node(node_id).unwrap().node_type,
        NodeType::Corner,
        "Initial node must be Corner"
    );

    let screen_pt = state.viewport.to_screen(state.find_node(node_id).unwrap().point);
    let click_pos = Pos2::new(screen_pt.x.0, screen_pt.y.0);

    // Option-click on the node
    let alt_modifiers = EguiModifiers {
        alt: true,
        ..Default::default()
    };
    let mut input = RawInput::default();
    input.events.push(Event::PointerButton {
        pos: click_pos,
        button: PointerButton::Primary,
        pressed: true,
        modifiers: alt_modifiers,
    });
    let _ = run_canvas_frame(&ctx, &mut state, &theme, input);

    assert_eq!(
        state.find_node(node_id).unwrap().node_type,
        NodeType::Smooth,
        "Option-click must toggle Corner to Smooth"
    );

    // Option-click again: toggle back to Corner
    let mut input = RawInput::default();
    input.events.push(Event::PointerButton {
        pos: click_pos,
        button: PointerButton::Primary,
        pressed: true,
        modifiers: alt_modifiers,
    });
    let _ = run_canvas_frame(&ctx, &mut state, &theme, input);

    assert_eq!(
        state.find_node(node_id).unwrap().node_type,
        NodeType::Corner,
        "Option-click must toggle Smooth back to Corner"
    );
}

#[test]
fn test_marquee_drag_selection_selects_intersecting_nodes() {
    let ctx = Context::default();
    let theme = GlifoTheme::sonoma_default();
    let mut state = GlifoCanvasState::init_glyph_a();

    // Verify initial selection is empty
    assert!(state.selected_nodes.is_empty());

    // In glyph A, apex nodes are at font y=700 (nodes 1 and 2 of contour 0).
    // Let's drag a marquee box covering the top area of the glyph.
    let top_left = state.viewport.to_screen(FontPoint::new(200.0, 750.0));
    let bottom_right = state.viewport.to_screen(FontPoint::new(500.0, 650.0));

    let p_start = Pos2::new(top_left.x.0, top_left.y.0);
    let p_end = Pos2::new(bottom_right.x.0, bottom_right.y.0);

    // 1. Mouse down on empty space
    let mut input = RawInput::default();
    input.events.push(Event::PointerButton {
        pos: p_start,
        button: PointerButton::Primary,
        pressed: true,
        modifiers: EguiModifiers::NONE,
    });
    let _ = run_canvas_frame(&ctx, &mut state, &theme, input);

    // 2. Mouse move to drag marquee
    let mut input = RawInput::default();
    input.events.push(Event::PointerMoved(p_end));
    let _ = run_canvas_frame(&ctx, &mut state, &theme, input);

    // Marquee rect should be active during drag
    assert!(
        state.select_state.marquee_rect().is_some(),
        "Marquee rectangle must be active while dragging over empty space"
    );

    // 3. Mouse up: marquee selection finalizes
    let mut input = RawInput::default();
    input.events.push(Event::PointerButton {
        pos: p_end,
        button: PointerButton::Primary,
        pressed: false,
        modifiers: EguiModifiers::NONE,
    });
    let _ = run_canvas_frame(&ctx, &mut state, &theme, input);

    // Node (300, 700) and (400, 700) should be inside the marquee rect [200..500, 650..750]
    let node_300 = state.contours[0][1].id;
    let node_400 = state.contours[0][2].id;

    assert!(
        state.selected_nodes.contains(&node_300),
        "Marquee must select node at (300, 700)"
    );
    assert!(
        state.selected_nodes.contains(&node_400),
        "Marquee must select node at (400, 700)"
    );
    assert_eq!(
        state.selected_nodes.len(),
        2,
        "Marquee must select exactly 2 nodes (the apex nodes)"
    );
}

#[test]
fn test_pen_tool_adds_node_and_drags_symmetric_handles() {
    let ctx = Context::default();
    let theme = GlifoTheme::sonoma_default();
    let mut state = GlifoCanvasState::new(theme.clone());
    state.active_tool = CanvasTool::Pen;

    let _initial_contour_count = state.contours.len();

    // 1. Click at screen position (300, 300) to place an anchor
    let anchor_screen = Pos2::new(300.0, 300.0);
    let mut input = RawInput::default();
    input.events.push(Event::PointerButton {
        pos: anchor_screen,
        button: PointerButton::Primary,
        pressed: true,
        modifiers: EguiModifiers::NONE,
    });
    let _ = run_canvas_frame(&ctx, &mut state, &theme, input);

    // A node should be added
    let total_nodes: usize = state.contours.iter().map(|c| c.len()).sum();
    assert!(total_nodes >= 1, "Pen tool click must add an anchor node");

    // 2. Drag mouse to extrude handles
    let drag_pos = anchor_screen + Vec2::new(40.0, -30.0);
    let mut input = RawInput::default();
    input.events.push(Event::PointerMoved(drag_pos));
    let _ = run_canvas_frame(&ctx, &mut state, &theme, input);

    // Pen state should reflect DraggingHandles
    match state.pen_state {
        grafia::interaction::PenToolState::DraggingHandles {
            anchor,
            bcp_out,
            bcp_in,
            ..
        } => {
            let dx_out = bcp_out.x.0 - anchor.x.0;
            let dy_out = bcp_out.y.0 - anchor.y.0;
            let dx_in = bcp_in.x.0 - anchor.x.0;
            let dy_in = bcp_in.y.0 - anchor.y.0;

            // Symmetrical handles: dx_in == -dx_out, dy_in == -dy_out
            assert!(
                (dx_out + dx_in).abs() < 1e-4,
                "Pen tool handles must be symmetrical (x)"
            );
            assert!(
                (dy_out + dy_in).abs() < 1e-4,
                "Pen tool handles must be symmetrical (y)"
            );
        }
        ref other => panic!("Expected PenToolState::DraggingHandles, got {:?}", other),
    }

    // 3. Mouse up
    let mut input = RawInput::default();
    input.events.push(Event::PointerButton {
        pos: drag_pos,
        button: PointerButton::Primary,
        pressed: false,
        modifiers: EguiModifiers::NONE,
    });
    let _ = run_canvas_frame(&ctx, &mut state, &theme, input);
}

#[test]
fn test_view_mode_switching() {
    let ctx = Context::default();
    let theme = GlifoTheme::sonoma_default();
    let mut state = GlifoCanvasState::init_glyph_a();

    assert_eq!(state.view_mode, CanvasViewMode::EditWireframe);

    // Switch to PreviewFilled
    state.view_mode = CanvasViewMode::PreviewFilled;
    let resp = run_canvas_frame(&ctx, &mut state, &theme, RawInput::default());
    assert_eq!(resp.view_mode, CanvasViewMode::PreviewFilled);

    // Switch to ShapeThumbnail
    state.view_mode = CanvasViewMode::ShapeThumbnail;
    let resp = run_canvas_frame(&ctx, &mut state, &theme, RawInput::default());
    assert_eq!(resp.view_mode, CanvasViewMode::ShapeThumbnail);

    // Switch back to EditWireframe
    state.view_mode = CanvasViewMode::EditWireframe;
    let resp = run_canvas_frame(&ctx, &mut state, &theme, RawInput::default());
    assert_eq!(resp.view_mode, CanvasViewMode::EditWireframe);
}

#[test]
fn test_glifo_canvas_widget_zero_repaint_leak_at_rest() {
    let theme = GlifoTheme::sonoma_default();
    let mut state = GlifoCanvasState::init_glyph_a();

    for mode in [
        CanvasViewMode::EditWireframe,
        CanvasViewMode::PreviewFilled,
        CanvasViewMode::ShapeThumbnail,
    ] {
        let ctx = Context::default();
        state.view_mode = mode;
        let mut run_idle_frame = |time: f64| {
            let input = RawInput {
                time: Some(time),
                ..Default::default()
            };
            let mut out = ctx.run_ui(input, |ui| {
                let _ = GlifoCanvasWidget::new(&mut state, &theme).show(ui);
            });
            out.textures_delta.clear();
            out.viewport_output[&egui::ViewportId::ROOT].repaint_delay
        };
        // Frame 1 & 2: initial egui font/sizing pass warmup
        let _ = run_idle_frame(1.0);
        let _ = run_idle_frame(1.016);
        // Frame 3: steady-state at rest without input
        let repaint_delay = run_idle_frame(1.032);
        assert!(
            !repaint_delay.is_zero(),
            "GlifoCanvasWidget in {mode:?} at rest must NOT trigger a 0ns repaint loop (got {repaint_delay:?})"
        );
    }
}

// ============================================================================
// Adversarial Stress Tests (Milestone M2 Empirical Verification)
// ============================================================================

#[test]
fn test_adversarial_view_modes_empty_and_degenerate_canvas() {
    let ctx = Context::default();
    let theme = GlifoTheme::sonoma_default();

    // 1. Completely empty canvas (zero contours, zero paths, zero nodes)
    let mut empty_state = GlifoCanvasState::new(theme.clone());
    assert!(empty_state.contours.is_empty());
    assert!(empty_state.paths.is_empty());

    for mode in [
        CanvasViewMode::EditWireframe,
        CanvasViewMode::PreviewFilled,
        CanvasViewMode::ShapeThumbnail,
    ] {
        empty_state.view_mode = mode;
        let resp = run_canvas_frame(&ctx, &mut empty_state, &theme, RawInput::default());
        assert_eq!(resp.view_mode, mode, "Empty canvas must render in mode {mode:?} without panics");
    }

    // 2. Degenerate contours: zero-node, single-node, two-node open, and coincident points
    let mut degen_state = GlifoCanvasState::new(theme.clone());
    degen_state.contours = vec![
        // Contour 0: empty
        vec![],
        // Contour 1: single node
        vec![grafia::core::Node::new(grafia::core::NodeId(101), 100.0, 100.0, NodeType::Corner)],
        // Contour 2: two-node open segment
        vec![
            grafia::core::Node::new(grafia::core::NodeId(102), 200.0, 200.0, NodeType::Smooth),
            grafia::core::Node::new(grafia::core::NodeId(103), 300.0, 300.0, NodeType::Corner),
        ],
        // Contour 3: three coincident identical points (zero-area polygon)
        vec![
            grafia::core::Node::new(grafia::core::NodeId(104), 400.0, 400.0, NodeType::Corner),
            grafia::core::Node::new(grafia::core::NodeId(105), 400.0, 400.0, NodeType::Corner),
            grafia::core::Node::new(grafia::core::NodeId(106), 400.0, 400.0, NodeType::Corner),
        ],
    ];
    degen_state.sync_paths_from_contours();

    for mode in [
        CanvasViewMode::EditWireframe,
        CanvasViewMode::PreviewFilled,
        CanvasViewMode::ShapeThumbnail,
    ] {
        degen_state.view_mode = mode;
        let resp = run_canvas_frame(&ctx, &mut degen_state, &theme, RawInput::default());
        assert_eq!(resp.view_mode, mode, "Degenerate contours must render in mode {mode:?} without panics");
    }
}

#[test]
fn test_adversarial_extreme_zoom_and_pan_projections() {
    let ctx = Context::default();
    let theme = GlifoTheme::sonoma_default();
    let mut state = GlifoCanvasState::init_glyph_a();

    // Test zoom levels down to normative MIN_VIEWPORT_ZOOM (0.01) up to extreme zoom (100.0)
    let zoom_levels = [
        grafia::tokens::MIN_VIEWPORT_ZOOM,
        0.05_f32,
        0.5_f32,
        2.0_f32,
        10.0_f32,
        100.0_f32,
    ];
    let pan_offsets = [
        grafia::core::ScreenPoint::new(0.0, 0.0),
        grafia::core::ScreenPoint::new(800.0, 600.0),
        grafia::core::ScreenPoint::new(-2000.0, 2000.0),
        grafia::core::ScreenPoint::new(10_000.0, -10_000.0),
    ];

    for &zoom in &zoom_levels {
        for &pan in &pan_offsets {
            state.viewport.zoom = zoom;
            state.viewport.pan_offset = pan;

            // Verify coordinate roundtrip numerical stability within f32 precision budget
            let original_pt = FontPoint::new(300.0, 700.0);
            let screen_pt = state.viewport.to_screen(original_pt);
            let recovered_font = state.viewport.to_font(screen_pt);

            let dx = (recovered_font.x.0 - original_pt.x.0).abs();
            let dy = (recovered_font.y.0 - original_pt.y.0).abs();
            // At zoom=0.01 and pan=10,000, 1 ScreenDp = 100 FontUnits; f32 precision gives ~0.2 FontUnit tolerance
            let max_tolerable_font_delta = (1.0 / zoom as f64).max(0.5);
            assert!(
                dx <= max_tolerable_font_delta && dy <= max_tolerable_font_delta,
                "Coordinate roundtrip failed at zoom {zoom}, pan {pan:?}: dx={dx}, dy={dy}, tol={max_tolerable_font_delta}"
            );

            // Verify rendering in all 3 modes does not panic under extreme transforms
            for mode in [
                CanvasViewMode::EditWireframe,
                CanvasViewMode::PreviewFilled,
                CanvasViewMode::ShapeThumbnail,
            ] {
                state.view_mode = mode;
                let resp = run_canvas_frame(&ctx, &mut state, &theme, RawInput::default());
                assert_eq!(resp.view_mode, mode);
            }
        }
    }

    // Now test extreme sub-minimum zoom (e.g. 0.0001) to verify rendering never panics even under extreme underflow
    state.viewport.zoom = 0.0001;
    state.viewport.pan_offset = grafia::core::ScreenPoint::new(1_000_000.0, -1_000_000.0);
    for mode in [
        CanvasViewMode::EditWireframe,
        CanvasViewMode::PreviewFilled,
        CanvasViewMode::ShapeThumbnail,
    ] {
        state.view_mode = mode;
        let resp = run_canvas_frame(&ctx, &mut state, &theme, RawInput::default());
        assert_eq!(resp.view_mode, mode, "Rendering under extreme underflow zoom must never panic");
    }
}

#[test]
fn test_adversarial_kurbo_bezier_curve_flattening_and_thumbnail_projections() {
    use kurbo::{flatten, BezPath, Shape};
    let theme = GlifoTheme::sonoma_default();

    // 1. Stress-test kurbo BezPath curve flattening on diverse curves
    let mut path = BezPath::new();

    // Normal cubic segment
    path.move_to((100.0, 100.0));
    path.curve_to((150.0, 300.0), (350.0, 300.0), (400.0, 100.0));

    // Sharp cusp (control points collinear and reversing)
    path.curve_to((500.0, 100.0), (300.0, 100.0), (450.0, 200.0));

    // Collinear cubic (degenerate line)
    path.curve_to((460.0, 210.0), (470.0, 220.0), (480.0, 230.0));

    // Coincident cubic (all points equal)
    path.curve_to((480.0, 230.0), (480.0, 230.0), (480.0, 230.0));

    // Self-intersecting loop
    path.curve_to((600.0, 400.0), (300.0, 400.0), (500.0, 200.0));
    path.close_path();

    // Flatten with normative CURVE_FLATTEN_TOLERANCE using kurbo::flatten
    let mut flattened_points = Vec::new();
    flatten(
        path.iter(),
        grafia::tokens::CURVE_FLATTEN_TOLERANCE,
        |el| match el {
            kurbo::PathEl::MoveTo(p) | kurbo::PathEl::LineTo(p) => {
                flattened_points.push(p);
            }
            kurbo::PathEl::ClosePath => {}
            _ => panic!("Flattened path must only contain MoveTo, LineTo, or ClosePath"),
        },
    );

    assert!(!flattened_points.is_empty(), "Flattened points must not be empty");
    assert!(
        flattened_points.len() < 100_000,
        "Flattening must not cause memory explosion: got {} points",
        flattened_points.len()
    );

    for p in &flattened_points {
        assert!(p.x.is_finite() && p.y.is_finite(), "Flattened points must all be finite");
    }

    // 2. ShapeThumbnailProjection stress on edge-case paths
    // Case A: Empty path
    let empty_path = BezPath::new();
    let empty_thumb = grafia::render::ShapeThumbnailProjection::compute(
        &empty_path,
        &theme.shape_canvas,
        0,
        true,
    );
    assert!(empty_thumb.scale.is_finite());
    assert!(empty_thumb.offset_x.is_finite());
    assert!(empty_thumb.offset_y.is_finite());

    // Case B: Single point MoveTo
    let mut single_pt = BezPath::new();
    single_pt.move_to((250.0, 250.0));
    let single_thumb = grafia::render::ShapeThumbnailProjection::compute(
        &single_pt,
        &theme.shape_canvas,
        1,
        true,
    );
    assert!(single_thumb.scale.is_finite());
    assert!(single_thumb.offset_x.is_finite());
    assert!(single_thumb.offset_y.is_finite());

    // Case C: Massive path (1e6 units)
    let mut massive_path = BezPath::new();
    massive_path.move_to((0.0, 0.0));
    massive_path.line_to((1_000_000.0, 1_000_000.0));
    massive_path.close_path();
    let massive_thumb = grafia::render::ShapeThumbnailProjection::compute(
        &massive_path,
        &theme.shape_canvas,
        2,
        true,
    );
    assert!(massive_thumb.scale.is_finite() && massive_thumb.scale > 0.0);
    let bbox = massive_thumb.fitted_path.bounding_box();
    assert!(bbox.width() <= (theme.shape_canvas.thumbnail_width_dp as f64) + 0.1);
    assert!(bbox.height() <= (theme.shape_canvas.thumbnail_height_dp as f64) + 0.1);

    // Case D: Microscopic path (1e-6 units)
    let mut micro_path = BezPath::new();
    micro_path.move_to((100.0, 100.0));
    micro_path.line_to((100.000001, 100.000001));
    micro_path.close_path();
    let micro_thumb = grafia::render::ShapeThumbnailProjection::compute(
        &micro_path,
        &theme.shape_canvas,
        2,
        true,
    );
    assert!(micro_thumb.scale.is_finite() && micro_thumb.scale > 0.0);
}

#[test]
fn test_adversarial_hit_testing_boundary_alignment_with_projections() {
    let theme = GlifoTheme::sonoma_default();
    let state = GlifoCanvasState::init_glyph_a();

    // Node 0 of contour 0 (Corner node at (30.0, 0.0))
    let node_id = state.contours[0][0].id;
    let font_pt = state.contours[0][0].point;
    let screen_pt = state.viewport.to_screen(font_pt);
    let half_node = (theme.edit_canvas.node_size_dp * 0.5) as f64;

    let proj = grafia::render::ProjectedNode::new(
        font_pt,
        NodeType::Corner,
        false,
        &state.viewport,
        &theme.edit_canvas,
    );

    // Verify mathematical bounds alignment
    assert_eq!(proj.bounds.x0, screen_pt.x.0 as f64 - half_node);
    assert_eq!(proj.bounds.x1, screen_pt.x.0 as f64 + half_node);
    assert_eq!(proj.bounds.y0, screen_pt.y.0 as f64 - half_node);
    assert_eq!(proj.bounds.y1, screen_pt.y.0 as f64 + half_node);

    // 1. Center point hit
    assert_eq!(
        state.hit_test_node(screen_pt),
        Some(node_id),
        "Center of projected node must hit"
    );

    // 2. Visual projection boundary: just inside/outside all 4 edges (delta = half_node ± 0.05)
    let eps = 0.05;
    let proj_inside = [
        grafia::core::ScreenPoint::new(screen_pt.x.0 - (half_node as f32 - eps), screen_pt.y.0),
        grafia::core::ScreenPoint::new(screen_pt.x.0 + (half_node as f32 - eps), screen_pt.y.0),
        grafia::core::ScreenPoint::new(screen_pt.x.0, screen_pt.y.0 - (half_node as f32 - eps)),
        grafia::core::ScreenPoint::new(screen_pt.x.0, screen_pt.y.0 + (half_node as f32 - eps)),
    ];
    for (i, &pt) in proj_inside.iter().enumerate() {
        assert!(
            proj.contains_screen_pt(pt),
            "Point just inside visual edge {i} must be inside ProjectedNode bounds"
        );
    }
    let proj_outside = [
        grafia::core::ScreenPoint::new(screen_pt.x.0 - (half_node as f32 + eps), screen_pt.y.0),
        grafia::core::ScreenPoint::new(screen_pt.x.0 + (half_node as f32 + eps), screen_pt.y.0),
        grafia::core::ScreenPoint::new(screen_pt.x.0, screen_pt.y.0 - (half_node as f32 + eps)),
        grafia::core::ScreenPoint::new(screen_pt.x.0, screen_pt.y.0 + (half_node as f32 + eps)),
    ];
    for (i, &pt) in proj_outside.iter().enumerate() {
        assert!(
            !proj.contains_screen_pt(pt),
            "Point just outside visual edge {i} must NOT be inside ProjectedNode bounds"
        );
    }

    // 3. Interactive click hit-test boundary (NODE_HIT_TEST_RADIUS_DP = 7.5 dp)
    let hit_r = grafia::tokens::NODE_HIT_TEST_RADIUS_DP;
    let test_inside = [
        grafia::core::ScreenPoint::new(screen_pt.x.0 - (hit_r - eps), screen_pt.y.0),
        grafia::core::ScreenPoint::new(screen_pt.x.0 + (hit_r - eps), screen_pt.y.0),
        grafia::core::ScreenPoint::new(screen_pt.x.0, screen_pt.y.0 - (hit_r - eps)),
        grafia::core::ScreenPoint::new(screen_pt.x.0, screen_pt.y.0 + (hit_r - eps)),
    ];
    for (i, &pt) in test_inside.iter().enumerate() {
        assert_eq!(
            state.hit_test_node(pt),
            Some(node_id),
            "Point just inside hit radius {i} must hit node"
        );
    }

    let test_outside = [
        grafia::core::ScreenPoint::new(screen_pt.x.0 - (hit_r + eps), screen_pt.y.0),
        grafia::core::ScreenPoint::new(screen_pt.x.0 + (hit_r + eps), screen_pt.y.0),
        grafia::core::ScreenPoint::new(screen_pt.x.0, screen_pt.y.0 - (hit_r + eps)),
        grafia::core::ScreenPoint::new(screen_pt.x.0, screen_pt.y.0 + (hit_r + eps)),
    ];
    for (i, &pt) in test_outside.iter().enumerate() {
        assert_eq!(
            state.hit_test_node(pt),
            None,
            "Point just outside hit radius {i} must NOT hit node"
        );
    }

    // 4. OffCurve node vs Corner/Smooth node dimension analysis
    // In ProjectedNode::new, bounds always uses node_size_dp (7.0),
    // whereas pass_4_nodes renders OffCurve nodes with bcp_handle_size_dp (6.0).
    let offcurve_proj = grafia::render::ProjectedNode::new(
        font_pt,
        NodeType::OffCurve,
        false,
        &state.viewport,
        &theme.edit_canvas,
    );
    // Document exact metric: OffCurve projected bounds is 7x7 dp
    assert_eq!(offcurve_proj.bounds.width(), theme.edit_canvas.node_size_dp as f64);
    assert_eq!(offcurve_proj.bounds.height(), theme.edit_canvas.node_size_dp as f64);
}

fn point_in_triangle_2d(p: Pos2, a: Pos2, b: Pos2, c: Pos2) -> bool {
    let sign = |p1: Pos2, p2: Pos2, p3: Pos2| -> f32 {
        (p1.x - p3.x) * (p2.y - p3.y) - (p2.x - p3.x) * (p1.y - p3.y)
    };
    let d1 = sign(p, a, b);
    let d2 = sign(p, b, c);
    let d3 = sign(p, c, a);
    let has_neg = (d1 < 0.0) || (d2 < 0.0) || (d3 < 0.0);
    let has_pos = (d1 > 0.0) || (d2 > 0.0) || (d3 > 0.0);
    !(has_neg && has_pos)
}

fn mesh_covers_point(mesh: &egui::Mesh, pt: Pos2) -> bool {
    for &[i0, i1, i2] in mesh.indices.as_chunks::<3>().0 {
        let a = mesh.vertices[i0 as usize].pos;
        let b = mesh.vertices[i1 as usize].pos;
        let c = mesh.vertices[i2 as usize].pos;
        if point_in_triangle_2d(pt, a, b, c) {
            return true;
        }
    }
    false
}

#[test]
fn test_egui_canvas_painter_and_headless_v010_visual_affordance_parity() {
    use egui::Shape;
    use grafia::adapters::headless::rasterize_canvas_subregion;
    use grafia::render::{compute_open_contour_endpoint_ticks, compute_start_node_arrow_triangle};

    let ctx = Context::default();
    let theme = GlifoTheme::sonoma_default();

    // =========================================================================
    // 1. PreviewFilled Non-Zero Winding Shape::Mesh & Headless Parity
    // =========================================================================
    let mut state = GlifoCanvasState::init_glyph_a();
    state.view_mode = CanvasViewMode::PreviewFilled;

    let hole_font = FontPoint::new(300.0, 320.0);
    let hole_scr = state.viewport.to_screen(hole_font);
    let hole_pos = Pos2::new(hole_scr.x.0, hole_scr.y.0);
    let hx = hole_scr.x.0.round() as u32;
    let hy = hole_scr.y.0.round() as u32;

    let mut out_initial = ctx.run_ui(RawInput::default(), |ui| {
        let _ = GlifoCanvasWidget::new(&mut state, &theme).show(ui);
    });
    out_initial.textures_delta.clear();

    // Ensure no dead/redundant transparent Shape::Path is emitted in PreviewFilled
    let redundant_transparent_paths = out_initial
        .shapes
        .iter()
        .filter(|cs| {
            matches!(
                &cs.shape,
                Shape::Path(ps) if ps.fill == egui::Color32::TRANSPARENT
            )
        })
        .count();
    assert_eq!(
        redundant_transparent_paths, 0,
        "PreviewFilled must NOT emit dead transparent Shape::Path over unflattened control points"
    );

    let mesh_initial = out_initial
        .shapes
        .iter()
        .find_map(|cs| match &cs.shape {
            Shape::Mesh(m) => Some(m.as_ref()),
            _ => None,
        })
        .expect("PreviewFilled must emit a Non-Zero Winding Shape::Mesh");

    let pix_initial = rasterize_canvas_subregion(&state, &theme, 900, 700);
    let bg_c = theme.canvas_background_color32();
    let fill_c = theme.preview_canvas.fill_color32();
    let p_hole_init = pix_initial.pixel(hx, hy).expect("hole pixel inside bounds");

    assert!(
        !mesh_covers_point(mesh_initial, hole_pos),
        "Initial /A (CCW outer + CW inner -> winding 0): egui::Mesh must NOT cover counter hole (300, 320)"
    );
    assert_eq!(
        (p_hole_init.red(), p_hole_init.green(), p_hole_init.blue()),
        (bg_c.r(), bg_c.g(), bg_c.b()),
        "Initial /A headless pixel at (300, 320) must be background color"
    );

    // Reverse only inner contour 1 (so both contours wind CCW: +1 + 1 = +2 != 0)
    state.selected_nodes = vec![state.contours[1][0].id];
    assert!(state.reverse_selected_or_all_contours());

    let mut out_reversed = ctx.run_ui(RawInput::default(), |ui| {
        let _ = GlifoCanvasWidget::new(&mut state, &theme).show(ui);
    });
    out_reversed.textures_delta.clear();

    let mesh_reversed = out_reversed
        .shapes
        .iter()
        .find_map(|cs| match &cs.shape {
            Shape::Mesh(m) => Some(m.as_ref()),
            _ => None,
        })
        .expect("PreviewFilled after reverse must emit Shape::Mesh");
    let pix_reversed = rasterize_canvas_subregion(&state, &theme, 900, 700);
    let p_hole_rev = pix_reversed.pixel(hx, hy).expect("hole pixel inside bounds");

    assert!(
        mesh_covers_point(mesh_reversed, hole_pos),
        "After reversing inner contour 1 (winding +1 + 1 = +2): egui::Mesh MUST cover counter hole (300, 320)"
    );
    assert_eq!(
        (p_hole_rev.red(), p_hole_rev.green(), p_hole_rev.blue()),
        (fill_c.r(), fill_c.g(), fill_c.b()),
        "After reversing inner contour 1: headless pixel at (300, 320) must be filled solid"
    );

    // Self-intersecting figure-8 cubic contour in PreviewFilled: both lobes (winding +1 and -1)
    // must be filled identically by lyon FillRule::NonZero and tiny-skia FillRule::Winding.
    let mut fig8_state = GlifoCanvasState::new(theme.clone());
    fig8_state.viewport.pan_offset = grafia::core::ScreenPoint::new(0.0, 600.0);
    fig8_state.view_mode = CanvasViewMode::PreviewFilled;
    fig8_state.contours = vec![vec![
        grafia::core::Node::new(grafia::core::NodeId(1), 100.0, 100.0, NodeType::Corner),
        grafia::core::Node::new(grafia::core::NodeId(2), 500.0, 500.0, NodeType::Corner),
        grafia::core::Node::new(grafia::core::NodeId(3), 500.0, 100.0, NodeType::Corner),
        grafia::core::Node::new(grafia::core::NodeId(4), 100.0, 500.0, NodeType::Corner),
    ]];
    fig8_state.set_contour_closed(0, true);
    fig8_state.sync_paths_from_contours();
    let mut out_fig8 = ctx.run_ui(RawInput::default(), |ui| {
        let _ = GlifoCanvasWidget::new(&mut fig8_state, &theme).show(ui);
    });
    out_fig8.textures_delta.clear();
    let mesh_fig8 = out_fig8
        .shapes
        .iter()
        .find_map(|cs| match &cs.shape {
            Shape::Mesh(m) => Some(m.as_ref()),
            _ => None,
        })
        .expect("Figure-8 contour must emit Shape::Mesh");
    let pix_fig8 = rasterize_canvas_subregion(&fig8_state, &theme, 900, 700);
    for lobe_pt in [FontPoint::new(180.0, 300.0), FontPoint::new(420.0, 300.0)] {
        let scr = fig8_state.viewport.to_screen(lobe_pt);
        assert!(
            mesh_covers_point(mesh_fig8, Pos2::new(scr.x.0, scr.y.0)),
            "Figure-8 lobe at {lobe_pt:?} must be covered by Non-Zero Winding egui::Mesh"
        );
        let px = pix_fig8
            .pixel(scr.x.0.round() as u32, scr.y.0.round() as u32)
            .expect("figure-8 lobe inside pixmap");
        assert_eq!(
            (px.red(), px.green(), px.blue()),
            (fill_c.r(), fill_c.g(), fill_c.b()),
            "Figure-8 lobe at {lobe_pt:?} must be filled in headless.rs"
        );
    }

    // =========================================================================
    // 2. EditWireframe Start-Node Direction Arrow (Flip on Reverse, Move on Make Node First)
    // =========================================================================
    let mut state = GlifoCanvasState::init_glyph_a();
    state.view_mode = CanvasViewMode::EditWireframe;
    let arrow_color = theme.start_node_arrow_color32();

    let expected_tri_0 =
        compute_start_node_arrow_triangle(&state.contours[0], true, &state.viewport)
            .expect("Contour 0 must have start arrow triangle");

    let mut out_wf0 = ctx.run_ui(RawInput::default(), |ui| {
        let _ = GlifoCanvasWidget::new(&mut state, &theme).show(ui);
    });
    out_wf0.textures_delta.clear();
    let pix_wf0 = rasterize_canvas_subregion(&state, &theme, 900, 700);

    let find_arrow_polygons = |shapes: &[egui::epaint::ClippedShape]| -> Vec<[Pos2; 3]> {
        shapes
            .iter()
            .filter_map(|cs| match &cs.shape {
                Shape::Path(poly)
                    if poly.fill == arrow_color && poly.points.len() == 3 =>
                {
                    Some([poly.points[0], poly.points[1], poly.points[2]])
                }
                _ => None,
            })
            .collect()
    };

    let arrows_initial = find_arrow_polygons(&out_wf0.shapes);
    assert_eq!(
        arrows_initial.len(),
        2,
        "EditWireframe for /A must emit 2 start-node direction arrows (one per contour)"
    );
    assert!(
        (arrows_initial[0][0] - Pos2::new(expected_tri_0[0].x.0, expected_tri_0[0].y.0)).length()
            < 1e-3,
        "Initial contour 0 arrow tip must match compute_start_node_arrow_triangle"
    );
    let centroid_init = Pos2::new(
        (expected_tri_0[0].x.0 + expected_tri_0[1].x.0 + expected_tri_0[2].x.0) / 3.0,
        (expected_tri_0[0].y.0 + expected_tri_0[1].y.0 + expected_tri_0[2].y.0) / 3.0,
    );
    let p_arrow_init = pix_wf0
        .pixel(centroid_init.x.round() as u32, centroid_init.y.round() as u32)
        .expect("arrow centroid inside pixmap");
    assert!(
        p_arrow_init.blue() > 180 && p_arrow_init.red() < 50,
        "Initial start-node arrow centroid must be rendered with blue arrow_color in headless.rs, got {p_arrow_init:?}"
    );

    // Reverse contour 0 -> arrow tip flips along reversed outgoing tangent
    state.selected_nodes = vec![state.contours[0][0].id];
    assert!(state.reverse_selected_or_all_contours());
    let mut out_wf_rev = ctx.run_ui(RawInput::default(), |ui| {
        let _ = GlifoCanvasWidget::new(&mut state, &theme).show(ui);
    });
    out_wf_rev.textures_delta.clear();
    let pix_wf_rev = rasterize_canvas_subregion(&state, &theme, 900, 700);
    let arrows_rev = find_arrow_polygons(&out_wf_rev.shapes);
    assert!(
        (arrows_rev[0][0] - arrows_initial[0][0]).length() > 8.0,
        "Reversing contour 0 must flip the emitted start-node arrow tip position"
    );
    let centroid_rev = Pos2::new(
        (arrows_rev[0][0].x + arrows_rev[0][1].x + arrows_rev[0][2].x) / 3.0,
        (arrows_rev[0][0].y + arrows_rev[0][1].y + arrows_rev[0][2].y) / 3.0,
    );
    let p_arrow_rev = pix_wf_rev
        .pixel(centroid_rev.x.round() as u32, centroid_rev.y.round() as u32)
        .expect("reversed arrow centroid inside pixmap");
    assert!(
        p_arrow_rev.blue() > 180 && p_arrow_rev.red() < 50,
        "Reversed start-node arrow centroid must be rendered with blue arrow_color in headless.rs, got {p_arrow_rev:?}"
    );

    // Make node index 2 the new first node -> arrow moves to the new start node
    let new_first_id = state.contours[0][2].id;
    state.selected_nodes = vec![new_first_id];
    assert!(state.make_selected_node_first());
    let mut out_wf_first = ctx.run_ui(RawInput::default(), |ui| {
        let _ = GlifoCanvasWidget::new(&mut state, &theme).show(ui);
    });
    out_wf_first.textures_delta.clear();
    let pix_wf_first = rasterize_canvas_subregion(&state, &theme, 900, 700);
    let arrows_new_first = find_arrow_polygons(&out_wf_first.shapes);
    let expected_tri_new_first =
        compute_start_node_arrow_triangle(&state.contours[0], true, &state.viewport)
            .expect("Contour 0 must have start arrow at new first node");
    assert!(
        (arrows_new_first[0][0]
            - Pos2::new(
                expected_tri_new_first[0].x.0,
                expected_tri_new_first[0].y.0
            ))
        .length()
            < 1e-3,
        "Make Node First must move the emitted start-node arrow to the new first node"
    );
    assert!(
        (arrows_new_first[0][0] - arrows_rev[0][0]).length() > 50.0,
        "Start-node arrow must relocate across the glyph to the new start node"
    );
    let centroid_new_first = Pos2::new(
        (expected_tri_new_first[0].x.0 + expected_tri_new_first[1].x.0 + expected_tri_new_first[2].x.0) / 3.0,
        (expected_tri_new_first[0].y.0 + expected_tri_new_first[1].y.0 + expected_tri_new_first[2].y.0) / 3.0,
    );
    let p_arrow_new_first = pix_wf_first
        .pixel(centroid_new_first.x.round() as u32, centroid_new_first.y.round() as u32)
        .expect("new first arrow centroid inside pixmap");
    assert!(
        p_arrow_new_first.blue() > 180 && p_arrow_new_first.red() < 50,
        "Relocated start-node arrow centroid must be rendered with blue arrow_color in headless.rs, got {p_arrow_new_first:?}"
    );

    // =========================================================================
    // 3. Open Contour Perpendicular Endpoint Ticks after Shift+Delete
    // =========================================================================
    state.selected_nodes = vec![state.contours[0][0].id];
    let mut shift_del_input = RawInput::default();
    shift_del_input.events.push(Event::Key {
        key: Key::Delete,
        physical_key: None,
        pressed: true,
        repeat: false,
        modifiers: EguiModifiers {
            shift: true,
            ..Default::default()
        },
    });
    let mut out_open = ctx.run_ui(shift_del_input, |ui| {
        let _ = GlifoCanvasWidget::new(&mut state, &theme).show(ui);
    });
    out_open.textures_delta.clear();
    assert!(!state.is_contour_closed(0));
    let pix_open = rasterize_canvas_subregion(&state, &theme, 900, 700);

    let expected_ticks = compute_open_contour_endpoint_ticks(&state.contours[0], &state.viewport);
    assert_eq!(expected_ticks.len(), 2, "Open contour must have 2 endpoint ticks");

    let tick_color = theme.open_endpoint_color32();
    for (pa, pb) in &expected_ticks {
        let exp_a = Pos2::new(pa.x.0, pa.y.0);
        let exp_b = Pos2::new(pb.x.0, pb.y.0);
        let found_tick = out_open.shapes.iter().any(|cs| match &cs.shape {
            Shape::LineSegment { points, stroke } => {
                stroke.color == tick_color
                    && (points[0] - exp_a).length() < 1e-3
                    && (points[1] - exp_b).length() < 1e-3
            }
            _ => false,
        });
        assert!(
            found_tick,
            "Open contour perpendicular endpoint tick ({exp_a:?} -> {exp_b:?}) must be emitted into egui shapes"
        );
        // Verify headless.rs also renders the perpendicular tick near its outer tip
        let tip_sample = exp_a + (exp_b - exp_a) * 0.85;
        let p_tick = pix_open
            .pixel(tip_sample.x.round() as u32, tip_sample.y.round() as u32)
            .expect("open tick tip inside pixmap");
        assert!(
            (p_tick.red(), p_tick.green(), p_tick.blue()) != (bg_c.r(), bg_c.g(), bg_c.b()),
            "Open contour perpendicular endpoint tick at {tip_sample:?} must be rendered in headless.rs"
        );
    }

    // =========================================================================
    // 4. Single-Point Open Contour Cross 'X' (2 Diagonal Line Segments)
    // =========================================================================
    let mut single_state = GlifoCanvasState::new(theme.clone());
    single_state.viewport.pan_offset = grafia::core::ScreenPoint::new(0.0, 600.0);
    single_state.contours = vec![vec![grafia::core::Node::new(
        grafia::core::NodeId(1),
        250.0,
        300.0,
        NodeType::Corner,
    )]];
    single_state.active_contour_closed = false;
    single_state.set_contour_closed(0, false);
    single_state.sync_paths_from_contours();

    let mut out_single = ctx.run_ui(RawInput::default(), |ui| {
        let _ = GlifoCanvasWidget::new(&mut single_state, &theme).show(ui);
    });
    out_single.textures_delta.clear();
    let pix_single = rasterize_canvas_subregion(&single_state, &theme, 900, 700);

    let center_scr = single_state.viewport.to_screen(FontPoint::new(250.0, 300.0));
    let half = theme.edit_canvas.node_size_dp * 0.5;
    let node_rect = egui::Rect::from_min_max(
        Pos2::new(center_scr.x.0 - half, center_scr.y.0 - half),
        Pos2::new(center_scr.x.0 + half, center_scr.y.0 + half),
    );
    let corner_col = theme.edit_canvas.corner_node_color32();
    let diag1_found = out_single.shapes.iter().any(|cs| match &cs.shape {
        Shape::LineSegment { points, stroke } => {
            stroke.color == corner_col
                && (points[0] - node_rect.left_top()).length() < 1e-3
                && (points[1] - node_rect.right_bottom()).length() < 1e-3
        }
        _ => false,
    });
    let diag2_found = out_single.shapes.iter().any(|cs| match &cs.shape {
        Shape::LineSegment { points, stroke } => {
            stroke.color == corner_col
                && (points[0] - node_rect.left_bottom()).length() < 1e-3
                && (points[1] - node_rect.right_top()).length() < 1e-3
        }
        _ => false,
    });
    assert!(
        diag1_found && diag2_found,
        "Single-point open contour must emit both diagonal line segments of the 'X' cross"
    );
    let p_single_center = pix_single
        .pixel(center_scr.x.0.round() as u32, center_scr.y.0.round() as u32)
        .expect("single-point cross center inside pixmap");
    assert!(
        (p_single_center.red(), p_single_center.green(), p_single_center.blue())
            != (bg_c.r(), bg_c.g(), bg_c.b()),
        "Single-point open contour 'X' intersection center must be rendered in headless.rs"
    );

    // =========================================================================
    // 5. PenTool Hover Ghost-Node Circle when hovered_segment_hit.is_some()
    // =========================================================================
    let mut pen_state = GlifoCanvasState::init_glyph_a();
    pen_state.active_tool = CanvasTool::Pen;
    let ghost_font = FontPoint::new(350.0, 700.0);
    let seg0 = grafia::core::extract_segments(&pen_state.contours[0], true)[1];
    pen_state.hovered_segment_hit = Some(grafia::core::SegmentHit {
        contour_idx: 0,
        segment: seg0,
        t: 0.5,
        point: ghost_font,
        distance_font: 0.0,
    });

    let mut out_ghost = ctx.run_ui(RawInput::default(), |ui| {
        let _ = GlifoCanvasWidget::new(&mut pen_state, &theme).show(ui);
    });
    out_ghost.textures_delta.clear();
    let pix_ghost = rasterize_canvas_subregion(&pen_state, &theme, 900, 700);

    let ghost_scr = pen_state.viewport.to_screen(ghost_font);
    let ghost_pos = Pos2::new(ghost_scr.x.0, ghost_scr.y.0);
    let glow_col = theme.edit_canvas.selected_glow_color32();
    let smooth_col = theme.edit_canvas.smooth_node_color32();
    let ghost_outer_stroke_found = out_ghost.shapes.iter().any(|cs| match &cs.shape {
        Shape::Circle(circle) => {
            (circle.center - ghost_pos).length() < 1e-3
                && (circle.radius - half).abs() < 1e-3
                && circle.stroke.color == glow_col
        }
        _ => false,
    });
    let ghost_inner_fill_found = out_ghost.shapes.iter().any(|cs| match &cs.shape {
        Shape::Circle(circle) => {
            (circle.center - ghost_pos).length() < 1e-3
                && (circle.radius - half * 0.5).abs() < 1e-3
                && circle.fill == smooth_col
        }
        _ => false,
    });
    assert!(
        ghost_outer_stroke_found && ghost_inner_fill_found,
        "PenTool with hovered_segment_hit.is_some() must emit both outer glow ring and inner smooth filled circle in egui"
    );

    let p_ghost_center = pix_ghost
        .pixel(ghost_scr.x.0.round() as u32, ghost_scr.y.0.round() as u32)
        .expect("ghost center pixel inside bounds");
    assert_eq!(
        (p_ghost_center.red(), p_ghost_center.green(), p_ghost_center.blue()),
        (smooth_col.r(), smooth_col.g(), smooth_col.b()),
        "PenTool ghost-node center in headless.rs must match egui inner filled circle (smooth_color)"
    );
}

#[test]
fn test_v010_canvas_demo_full_ui_and_epaint_tessellation_no_panic() {
    let ctx = Context::default();
    let theme = GlifoTheme::sonoma_default();
    let mut state = GlifoCanvasState::init_glyph_a();

    for arch in grafia::tokens::GlyphArchetype::all() {
        state.load_glyph_archetype(arch);
        for mode in [
            CanvasViewMode::EditWireframe,
            CanvasViewMode::PreviewFilled,
            CanvasViewMode::ShapeThumbnail,
        ] {
            state.view_mode = mode;
            let raw_input = RawInput {
                screen_rect: Some(egui::Rect::from_min_size(
                    Pos2::ZERO,
                    Vec2::new(1223.0, 811.0),
                )),
                ..Default::default()
            };
            let mut full_output = ctx.run_ui(raw_input, |ui| {
                egui::Panel::top("top_toolbar").show(ui, |ui| {
                    ui.horizontal(|ui| {
                        ui.heading("glifo");
                        ui.add(egui::Separator::default().vertical());
                    });
                });
                egui::Panel::bottom("status_bar").show(ui, |ui| {
                    ui.horizontal(|ui| {
                        ui.label("Status");
                        ui.add(egui::Separator::default().vertical());
                    });
                });
                egui::Panel::left("shapes_sidebar").show(ui, |ui| {
                    ui.add(egui::Separator::default().horizontal());
                    for contour in &state.glyph.contours {
                        ui.push_id(contour.id.0, |ui| {
                            ui.horizontal(|ui| {
                                ui.add(grafia::locator::ShapeLocatorWidget::new(
                                    &state.glyph,
                                    contour.id,
                                ));
                                let _ = ui.selectable_label(false, "Path");
                            });
                        });
                    }
                });
                egui::CentralPanel::default().show(ui, |ui| {
                    let canvas_size = ui.available_size();
                    let widget =
                        GlifoCanvasWidget::new(&mut state, &theme).with_size(canvas_size);
                    widget.show(ui);
                });
            });
            full_output.textures_delta.clear();
            let primitives = ctx.tessellate(full_output.shapes, full_output.pixels_per_point);
            assert!(
                !primitives.is_empty(),
                "Tessellated primitives must not be empty for {:?} in {:?}",
                arch,
                mode
            );
        }
    }
}

#[test]
fn test_open_contour_after_shift_delete_not_filled_in_preview_mode() {
    let mut state = GlifoCanvasState::init_glyph_o();
    state.fit_viewport_to_glyph(640.0, 480.0);
    let theme = GlifoTheme::sonoma_default();

    // Select the right-most Smooth OnCurve node (510, 250) on outer contour 0 of /o
    let right_node_id = state.contours[0]
        .iter()
        .find(|n| (n.point.x.0 - 510.0).abs() < 1.0 && (n.point.y.0 - 250.0).abs() < 1.0)
        .expect("right extremum node of /o")
        .id;
    state.selected_nodes = vec![right_node_id];

    // Cut contour 0 open via Shift+Delete
    let canvas_rect = egui::Rect::from_min_size(Pos2::ZERO, Vec2::new(640.0, 480.0));
    let shift_del = Event::Key {
        key: Key::Delete,
        physical_key: None,
        pressed: true,
        repeat: false,
        modifiers: egui::Modifiers {
            shift: true,
            ..Default::default()
        },
    };
    assert!(grafia::adapters::keyboard::dispatch_keyboard_event(
        &mut state,
        &shift_del,
        true,
        false,
        false,
        canvas_rect,
    ));
    assert!(
        !state.is_contour_closed(0),
        "Outer contour 0 must be open after Shift+Delete"
    );
    assert!(
        state.is_contour_closed(1),
        "Inner contour 1 must remain closed"
    );

    // Switch to PreviewFilled mode
    state.view_mode = CanvasViewMode::PreviewFilled;

    // 1. Check headless.rs rasterization:
    // Point (95.0, 250.0) in font space lies inside the left stem (between outer x=50 and inner x=140).
    // Because outer contour 0 is now OPEN (closed == false), it must NOT be filled!
    // Point (280.0, 250.0) in font space lies at the center of closed inner contour 1, so it MUST be filled!
    let pixmap = grafia::adapters::headless::rasterize_canvas_subregion(&state, &theme, 640, 480);
    let left_stem_scr = state.viewport.to_screen(FontPoint::new(95.0, 250.0));
    let inner_center_scr = state.viewport.to_screen(FontPoint::new(280.0, 250.0));

    let bg_col = theme.canvas_background_color32();
    let fill_col = theme.preview_canvas.fill_color32();

    let p_left_stem = pixmap
        .pixel(
            left_stem_scr.x.0.round() as u32,
            left_stem_scr.y.0.round() as u32,
        )
        .expect("left stem pixel in bounds");
    assert_eq!(
        (p_left_stem.red(), p_left_stem.green(), p_left_stem.blue()),
        (bg_col.r(), bg_col.g(), bg_col.b()),
        "Open outer contour 0 must NOT be filled in PreviewFilled mode (left stem must stay background color)"
    );

    let p_inner_center = pixmap
        .pixel(
            inner_center_scr.x.0.round() as u32,
            inner_center_scr.y.0.round() as u32,
        )
        .expect("inner center pixel in bounds");
    assert_eq!(
        (
            p_inner_center.red(),
            p_inner_center.green(),
            p_inner_center.blue()
        ),
        (fill_col.r(), fill_col.g(), fill_col.b()),
        "Closed inner contour 1 must be filled solid in PreviewFilled when outer contour 0 is open"
    );

    // 2. Check egui CanvasPainter mesh bounding box:
    // No filled mesh vertex may have x < inner contour left edge (x_font = 140.0)
    let inner_left_scr_x = state.viewport.to_screen(FontPoint::new(140.0, 250.0)).x.0;
    let ctx = Context::default();
    let mut out = ctx.run_ui(RawInput::default(), |ui| {
        let _ = GlifoCanvasWidget::new(&mut state, &theme)
            .with_size(Vec2::new(640.0, 480.0))
            .show(ui);
    });
    out.textures_delta.clear();
    let mut mesh_count = 0usize;
    for clipped in &out.shapes {
        if let egui::Shape::Mesh(mesh) = &clipped.shape {
            mesh_count += 1;
            for v in &mesh.vertices {
                assert!(
                    v.pos.x >= inner_left_scr_x - 2.0,
                    "PreviewFilled mesh vertex x={} extends into open outer contour (< {})",
                    v.pos.x,
                    inner_left_scr_x
                );
            }
        }
    }
    assert_eq!(mesh_count, 1, "Exactly 1 filled mesh for closed inner contour 1");
}

#[test]
fn test_open_contour_in_shape_thumbnail_mode_renders_unclosed_stroked_path() {
    let mut state = GlifoCanvasState::init_glyph_o();
    state.fit_viewport_to_glyph(640.0, 480.0);
    let theme = GlifoTheme::sonoma_default();

    let right_node_id = state.contours[0]
        .iter()
        .find(|n| (n.point.x.0 - 510.0).abs() < 1.0 && (n.point.y.0 - 250.0).abs() < 1.0)
        .expect("right extremum node of /o")
        .id;
    state.selected_nodes = vec![right_node_id];

    let canvas_rect = egui::Rect::from_min_size(Pos2::ZERO, Vec2::new(640.0, 480.0));
    let shift_del = Event::Key {
        key: Key::Delete,
        physical_key: None,
        pressed: true,
        repeat: false,
        modifiers: egui::Modifiers {
            shift: true,
            ..Default::default()
        },
    };
    assert!(grafia::adapters::keyboard::dispatch_keyboard_event(
        &mut state,
        &shift_del,
        true,
        false,
        false,
        canvas_rect,
    ));
    assert!(!state.is_contour_closed(0));
    assert!(state.is_contour_closed(1));

    state.view_mode = CanvasViewMode::ShapeThumbnail;

    // 1. Verify egui CanvasPainter emits closed: false and fill: TRANSPARENT for open contour 0
    let ctx = Context::default();
    let mut out = ctx.run_ui(RawInput::default(), |ui| {
        let _ = GlifoCanvasWidget::new(&mut state, &theme)
            .with_size(Vec2::new(640.0, 480.0))
            .show(ui);
    });
    out.textures_delta.clear();

    let path_shapes: Vec<&egui::epaint::PathShape> = out
        .shapes
        .iter()
        .filter_map(|cs| match &cs.shape {
            egui::Shape::Path(ps) => Some(ps),
            _ => None,
        })
        .collect();
    assert_eq!(path_shapes.len(), 2, "Must emit 2 thumbnail PathShapes");
    assert!(
        !path_shapes[0].closed,
        "Open contour 0 thumbnail PathShape must have closed == false"
    );
    assert_eq!(
        path_shapes[0].fill,
        egui::Color32::TRANSPARENT,
        "Open contour 0 thumbnail PathShape must have TRANSPARENT fill"
    );
    assert!(
        path_shapes[1].closed,
        "Closed contour 1 thumbnail PathShape must have closed == true"
    );
    assert_eq!(
        path_shapes[1].fill,
        theme.shape_canvas.text_active_color32(),
        "Closed contour 1 thumbnail PathShape must have active fill color"
    );

    // 2. Verify headless::rasterize_canvas_subregion strokes open contour 0 instead of filling it
    let pixmap = grafia::adapters::headless::rasterize_canvas_subregion(&state, &theme, 640, 480);
    let pad = theme.shape_canvas.header_height_dp * 2.0;
    let row_y0 = pad + theme.shape_canvas.header_height_dp;
    let card_w = theme.shape_canvas.thumbnail_width_dp * 4.0;
    let card_h = theme.shape_canvas.thumbnail_height_dp * 4.0;
    let mut scaled_cfg = theme.shape_canvas.clone();
    scaled_cfg.thumbnail_width_dp = card_w;
    scaled_cfg.thumbnail_height_dp = card_h;
    scaled_cfg.thumbnail_padding_dp = theme.shape_canvas.thumbnail_padding_dp * 4.0;
    let proj0 = grafia::render::ShapeThumbnailProjection::compute(
        &state.paths[0],
        &scaled_cfg,
        state.contours[0].len(),
        true,
    );
    // Point (220.0, 250.0) is inside the cavity of the open C-arc (between left curve x=50 and chord x=280)
    let cavity_x = (pad as f64 + proj0.offset_x + 220.0 * proj0.scale).round() as u32;
    let cavity_y = (row_y0 as f64 + proj0.offset_y + 250.0 * proj0.scale).round() as u32;
    let card_bg = theme.shape_canvas.accent_selection_color32();
    let px = pixmap
        .pixel(cavity_x, cavity_y)
        .expect("thumbnail cavity pixel in bounds");
    assert_eq!(
        (px.red(), px.green(), px.blue()),
        (card_bg.r(), card_bg.g(), card_bg.b()),
        "Headless ShapeThumbnail for open C-arc contour 0 must leave the chord cavity background-colored"
    );
}

#[test]
fn test_deleting_all_nodes_of_contour_0_preserves_contour_1_in_wireframe_and_preview() {
    let theme = GlifoTheme::sonoma_default();
    let canvas_rect = egui::Rect::from_min_size(Pos2::ZERO, Vec2::new(640.0, 480.0));
    let ctx = Context::default();

    for shift_held in [false, true] {
        let mut state = GlifoCanvasState::init_glyph_o();
        state.fit_viewport_to_glyph(640.0, 480.0);

        // Select all 12 nodes of outer contour 0
        state.selected_nodes = state.contours[0].iter().map(|n| n.id).collect();
        assert_eq!(state.selected_nodes.len(), 12);

        let del_event = Event::Key {
            key: Key::Delete,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: egui::Modifiers {
                shift: shift_held,
                ..Default::default()
            },
        };
        assert!(grafia::adapters::keyboard::dispatch_keyboard_event(
            &mut state,
            &del_event,
            shift_held,
            false,
            false,
            canvas_rect,
        ));

        assert_eq!(
            state.contours.len(),
            1,
            "Empty contour 0 must be removed from state.contours (shift={shift_held})"
        );
        assert_eq!(
            state.paths.len(),
            1,
            "state.paths.len() must equal state.contours.len() (shift={shift_held})"
        );
        assert_eq!(
            state.glyph.contours.len(),
            1,
            "state.glyph.contours.len() must equal 1 (shift={shift_held})"
        );
        assert!(
            state.is_contour_closed(0),
            "Surviving inner contour must remain closed (shift={shift_held})"
        );

        // Check EditWireframe mode renders the surviving contour's closed curve
        state.view_mode = CanvasViewMode::EditWireframe;
        let mut out_wf = ctx.run_ui(RawInput::default(), |ui| {
            let _ = GlifoCanvasWidget::new(&mut state, &theme)
                .with_size(Vec2::new(640.0, 480.0))
                .show(ui);
        });
        out_wf.textures_delta.clear();
        let has_closed_wireframe = out_wf.shapes.iter().any(|cs| match &cs.shape {
            egui::Shape::Path(ps) => ps.closed && ps.points.len() > 8,
            _ => false,
        });
        assert!(
            has_closed_wireframe,
            "Surviving inner contour must render its closed wireframe curve in EditWireframe (shift={shift_held})"
        );

        // Check PreviewFilled mode renders the surviving contour filled at (280.0, 250.0)
        state.view_mode = CanvasViewMode::PreviewFilled;
        let pixmap =
            grafia::adapters::headless::rasterize_canvas_subregion(&state, &theme, 640, 480);
        let center_scr = state.viewport.to_screen(FontPoint::new(280.0, 250.0));
        let fill_col = theme.preview_canvas.fill_color32();
        let px = pixmap
            .pixel(
                center_scr.x.0.round() as u32,
                center_scr.y.0.round() as u32,
            )
            .expect("center pixel in bounds");
        assert_eq!(
            (px.red(), px.green(), px.blue()),
            (fill_col.r(), fill_col.g(), fill_col.b()),
            "Surviving inner contour must be filled in PreviewFilled (shift={shift_held})"
        );
    }
}

#[test]
fn test_compound_multi_contour_delete_preserves_open_contour_role_and_single_point_open_state() {
    let canvas_rect = egui::Rect::from_min_size(Pos2::ZERO, Vec2::new(640.0, 480.0));

    // 1. Compound Shift+Delete: cut contour 0 open (1 node selected) AND delete all 12 nodes of contour 1 simultaneously
    let mut state = GlifoCanvasState::init_glyph_o();
    state.glyph.contours[0].role = grafia::core::ShapeBooleanRole::Mask;
    let c0_right_id = state.contours[0]
        .iter()
        .find(|n| (n.point.x.0 - 510.0).abs() < 1.0 && (n.point.y.0 - 250.0).abs() < 1.0)
        .expect("right extremum node of /o")
        .id;
    let mut sel = vec![c0_right_id];
    sel.extend(state.contours[1].iter().map(|n| n.id));
    state.selected_nodes = sel;

    let shift_del = Event::Key {
        key: Key::Delete,
        physical_key: None,
        pressed: true,
        repeat: false,
        modifiers: egui::Modifiers {
            shift: true,
            ..Default::default()
        },
    };
    assert!(grafia::adapters::keyboard::dispatch_keyboard_event(
        &mut state,
        &shift_del,
        true,
        false,
        false,
        canvas_rect,
    ));
    assert_eq!(state.contours.len(), 1);
    assert!(!state.is_contour_closed(0), "Contour 0 must be cut open");
    assert_eq!(
        state.active_open_contour,
        Some(0),
        "active_open_contour must remain Some(0) when contour 1 is completely deleted in the same Shift+Delete"
    );
    assert_eq!(
        state.glyph.contours[0].role,
        grafia::core::ShapeBooleanRole::Mask,
        "Surviving contour must preserve its ShapeBooleanRole across prune_empty_contours and sync_paths_from_contours"
    );

    // 2. Second Shift+Delete on an already-open contour: deleting OffCurve handle on top-left segment
    // must NOT rotate across the open gap or connect bottom (280, -10) to top (280, 510)!
    let first_on_before = state.contours[0].first().unwrap().id;
    let last_on_before = state.contours[0].last().unwrap().id;
    let top_left_h1_id = state.contours[0][1].id;
    assert_eq!(state.contours[0][1].node_type, NodeType::OffCurve);
    state.selected_nodes = vec![top_left_h1_id];
    assert!(grafia::adapters::keyboard::dispatch_keyboard_event(
        &mut state,
        &shift_del,
        true,
        false,
        false,
        canvas_rect,
    ));
    assert_eq!(
        state.contours[0].first().unwrap().id,
        first_on_before,
        "Deleting an OffCurve handle on an already-open contour must NOT rotate the open endpoints"
    );
    assert_eq!(
        state.contours[0].last().unwrap().id,
        last_on_before,
        "Deleting an OffCurve handle on an already-open contour must keep the same ending OnCurve node"
    );

    // 3. Undo + Redo roundtrip on compound multi-contour deletion
    assert!(state.undo());
    assert!(state.undo());
    assert_eq!(state.contours.len(), 2);
    assert!(state.is_contour_closed(0));
    assert!(state.is_contour_closed(1));
    assert!(state.redo());
    assert_eq!(state.contours.len(), 1);
    assert!(!state.is_contour_closed(0));

    // 4. Normal Delete reducing a closed contour to 1 node must mark it as open (!is_contour_closed)
    let mut state_single = GlifoCanvasState::init_glyph_a();
    // Delete 2 of the 3 nodes of inner contour 1 of /A using normal Delete
    state_single.selected_nodes = vec![state_single.contours[1][0].id, state_single.contours[1][1].id];
    let normal_del = Event::Key {
        key: Key::Delete,
        physical_key: None,
        pressed: true,
        repeat: false,
        modifiers: egui::Modifiers::NONE,
    };
    assert!(grafia::adapters::keyboard::dispatch_keyboard_event(
        &mut state_single,
        &normal_del,
        false,
        false,
        false,
        canvas_rect,
    ));
    assert_eq!(state_single.contours[1].len(), 1);
    assert!(
        !state_single.is_contour_closed(1),
        "A contour reduced to 1 surviving node must be marked open (!closed) so it renders as a single-point 'X'"
    );
}

/// Multi-view consistency verifier across all 4 rendering projections (`STD-ARCH`):
/// 1. `CanvasViewMode::EditWireframe` (`GlifoCanvasWidget` + `rasterize_canvas_subregion`)
/// 2. `CanvasViewMode::PreviewFilled` (`GlifoCanvasWidget` + `rasterize_canvas_subregion`)
/// 3. `CanvasViewMode::ShapeThumbnail` (`GlifoCanvasWidget` + `rasterize_canvas_subregion`)
/// 4. `ShapeLocatorRenderer::render_locator` for each contour in `state.glyph.contours`
pub fn assert_all_views_consistent(state: &GlifoCanvasState) {
    state.assert_invariants();

    let theme = state.theme.clone();
    let ctx = Context::default();
    let mut working = GlifoCanvasState::new(theme.clone());
    working.viewport = state.viewport;
    working.paths = state.paths.clone();
    working.contours = state.contours.clone();
    working.selected_nodes = state.selected_nodes.clone();
    working.glyph = state.glyph.clone();
    working.active_tool = state.active_tool;
    working.select_state = state.select_state.clone();
    working.pen_state = state.pen_state.clone();
    working.active_contour_closed = state.active_contour_closed;
    working.active_open_contour = state.active_open_contour;
    working.pending_pen_bcp_out = state.pending_pen_bcp_out;

    // 1. EditWireframe view
    working.view_mode = CanvasViewMode::EditWireframe;
    let mut out_wf = ctx.run_ui(RawInput::default(), |ui| {
        let _ = GlifoCanvasWidget::new(&mut working, &theme)
            .with_size(Vec2::new(320.0, 240.0))
            .show(ui);
    });
    out_wf.textures_delta.clear();
    let pix_wf = grafia::adapters::headless::rasterize_canvas_subregion(&working, &theme, 320, 240);
    assert_eq!((pix_wf.width(), pix_wf.height()), (320, 240));

    // 2. PreviewFilled view
    working.view_mode = CanvasViewMode::PreviewFilled;
    let mut out_pf = ctx.run_ui(RawInput::default(), |ui| {
        let _ = GlifoCanvasWidget::new(&mut working, &theme)
            .with_size(Vec2::new(320.0, 240.0))
            .show(ui);
    });
    out_pf.textures_delta.clear();
    let pix_pf = grafia::adapters::headless::rasterize_canvas_subregion(&working, &theme, 320, 240);
    assert_eq!((pix_pf.width(), pix_pf.height()), (320, 240));

    let mesh_count = out_pf
        .shapes
        .iter()
        .filter(|cs| matches!(cs.shape, egui::Shape::Mesh(_)))
        .count();
    let all_open = (0..state.contours.len()).all(|idx| !state.is_contour_closed(idx));
    if all_open {
        assert_eq!(
            mesh_count, 0,
            "PreviewFilled must emit 0 filled meshes when all contours are open"
        );
    }

    // 3. ShapeThumbnail view
    working.view_mode = CanvasViewMode::ShapeThumbnail;
    let mut out_st = ctx.run_ui(RawInput::default(), |ui| {
        let _ = GlifoCanvasWidget::new(&mut working, &theme)
            .with_size(Vec2::new(320.0, 240.0))
            .show(ui);
    });
    out_st.textures_delta.clear();
    let pix_st = grafia::adapters::headless::rasterize_canvas_subregion(&working, &theme, 320, 240);
    assert_eq!((pix_st.width(), pix_st.height()), (320, 240));

    let path_shapes: Vec<&egui::epaint::PathShape> = out_st
        .shapes
        .iter()
        .filter_map(|cs| match &cs.shape {
            egui::Shape::Path(ps) => Some(ps),
            _ => None,
        })
        .collect();
    let multi_node_contours: Vec<usize> = (0..state.contours.len())
        .filter(|&idx| state.contours[idx].len() >= 2)
        .collect();
    assert_eq!(
        path_shapes.len(),
        multi_node_contours.len(),
        "ShapeThumbnail must emit one PathShape per contour with >= 2 nodes"
    );
    for (shape_pos, &c_idx) in multi_node_contours.iter().enumerate() {
        let ps = path_shapes[shape_pos];
        if !state.is_contour_closed(c_idx) {
            assert!(
                !ps.closed && ps.fill == egui::Color32::TRANSPARENT,
                "Open contour {c_idx} in ShapeThumbnail must emit unclosed PathShape with TRANSPARENT fill"
            );
        }
    }

    // 4. ShapeLocatorRenderer for each contour
    let locator_style = grafia::locator::ShapeLocatorStyle::default();
    let locator_renderer = grafia::locator::ShapeLocatorRenderer::new(&state.glyph, locator_style);
    for (idx, gc) in state.glyph.contours.iter().enumerate() {
        let (_ctx_mesh, focus_mesh) =
            locator_renderer.render_locator(gc.id, egui::vec2(24.0, 16.0));
        if !state.is_contour_closed(idx) && state.contours[idx].len() >= 2 && gc.visible {
            assert!(
                !focus_mesh.vertices.is_empty() && !focus_mesh.indices.is_empty(),
                "Open contour {idx} with >= 2 nodes must produce a non-empty stroked focus_mesh in ShapeLocatorRenderer"
            );
        }
    }
}

#[test]
fn test_multi_view_consistency_helper_across_closed_mixed_and_all_open_states() {
    let canvas_rect = egui::Rect::from_min_size(Pos2::ZERO, Vec2::new(640.0, 480.0));
    let shift_del = Event::Key {
        key: Key::Delete,
        physical_key: None,
        pressed: true,
        repeat: false,
        modifiers: egui::Modifiers {
            shift: true,
            ..Default::default()
        },
    };

    // 1. Closed /A and /o
    let state_a = GlifoCanvasState::init_glyph_a();
    assert_all_views_consistent(&state_a);
    let mut state_o = GlifoCanvasState::init_glyph_o();
    assert_all_views_consistent(&state_o);

    // 2. Mixed open outer contour 0 + closed inner contour 1 on /o
    state_o.selected_nodes = vec![state_o.contours[0][0].id];
    assert!(grafia::adapters::keyboard::dispatch_keyboard_event(
        &mut state_o,
        &shift_del,
        true,
        false,
        false,
        canvas_rect,
    ));
    assert_all_views_consistent(&state_o);

    // 3. All-open contours on /o (cut inner contour 1 open as well)
    state_o.selected_nodes = vec![state_o.contours[1][0].id];
    assert!(grafia::adapters::keyboard::dispatch_keyboard_event(
        &mut state_o,
        &shift_del,
        true,
        false,
        false,
        canvas_rect,
    ));
    assert!(!state_o.is_contour_closed(0) && !state_o.is_contour_closed(1));
    assert_all_views_consistent(&state_o);
}

#[derive(Debug, Clone)]
enum StatefulCanvasAction {
    SelectNode(usize, usize, bool),
    SelectSegment(usize, usize, bool),
    Marquee(f32, f32, f32, f32, bool, bool),
    Delete,
    ShiftDelete,
    PenClick(f32, f32, f32, f32),
    Reverse,
    MakeFirst,
    ToggleSmooth,
    Nudge(u8, bool, bool),
    Undo,
    Redo,
}

fn apply_stateful_action(state: &mut GlifoCanvasState, action: &StatefulCanvasAction) {
    use kurbo::ParamCurve;
    let canvas_rect = egui::Rect::from_min_size(Pos2::ZERO, Vec2::new(640.0, 480.0));
    let ctx = Context::default();
    let theme = state.theme.clone();

    match *action {
        StatefulCanvasAction::SelectNode(c_seed, n_seed, shift) => {
            state.active_tool = CanvasTool::Select;
            state.pen_state = grafia::interaction::PenToolState::Idle;
            state.pending_pen_bcp_out = None;
            if !state.contours.is_empty() {
                let c_idx = c_seed % state.contours.len();
                if !state.contours[c_idx].is_empty() {
                    let n_idx = n_seed % state.contours[c_idx].len();
                    let scr = state.viewport.to_screen(state.contours[c_idx][n_idx].point);
                    let pos = Pos2::new(scr.x.0, scr.y.0);
                    let modifiers = EguiModifiers {
                        shift,
                        ..Default::default()
                    };
                    let mut input = RawInput::default();
                    input.events.push(Event::PointerButton {
                        pos,
                        button: PointerButton::Primary,
                        pressed: true,
                        modifiers,
                    });
                    input.events.push(Event::PointerButton {
                        pos,
                        button: PointerButton::Primary,
                        pressed: false,
                        modifiers,
                    });
                    let _ = run_canvas_frame(&ctx, state, &theme, input);
                }
            }
        }
        StatefulCanvasAction::SelectSegment(c_seed, s_seed, shift) => {
            state.active_tool = CanvasTool::Select;
            state.pen_state = grafia::interaction::PenToolState::Idle;
            state.pending_pen_bcp_out = None;
            if !state.contours.is_empty() {
                let c_idx = c_seed % state.contours.len();
                let segs = grafia::core::extract_segments(
                    &state.contours[c_idx],
                    state.is_contour_closed(c_idx),
                );
                if !segs.is_empty() {
                    let seg = segs[s_seed % segs.len()];
                    let mid = match seg {
                        grafia::core::ContourSegment::Line { line, .. } => line.eval(0.5),
                        grafia::core::ContourSegment::Cubic { cubic, .. } => cubic.eval(0.5),
                    };
                    let scr = state.viewport.to_screen(FontPoint::new(mid.x, mid.y));
                    let pos = Pos2::new(scr.x.0, scr.y.0);
                    let modifiers = EguiModifiers {
                        shift,
                        ..Default::default()
                    };
                    let mut input = RawInput::default();
                    input.events.push(Event::PointerButton {
                        pos,
                        button: PointerButton::Primary,
                        pressed: true,
                        modifiers,
                    });
                    input.events.push(Event::PointerButton {
                        pos,
                        button: PointerButton::Primary,
                        pressed: false,
                        modifiers,
                    });
                    let _ = run_canvas_frame(&ctx, state, &theme, input);
                }
            }
        }
        StatefulCanvasAction::Marquee(x0, y0, x1, y1, shift, alt) => {
            state.active_tool = CanvasTool::Select;
            state.pen_state = grafia::interaction::PenToolState::Idle;
            state.pending_pen_bcp_out = None;
            let p0 = Pos2::new(x0, y0);
            let p1 = Pos2::new(x1, y1);
            let modifiers = EguiModifiers {
                shift,
                alt,
                ..Default::default()
            };
            let mut input = RawInput::default();
            input.events.push(Event::PointerButton {
                pos: p0,
                button: PointerButton::Primary,
                pressed: true,
                modifiers,
            });
            input.events.push(Event::PointerMoved(p1));
            input.events.push(Event::PointerButton {
                pos: p1,
                button: PointerButton::Primary,
                pressed: false,
                modifiers,
            });
            let _ = run_canvas_frame(&ctx, state, &theme, input);
        }
        StatefulCanvasAction::Delete => {
            let ev = Event::Key {
                key: Key::Delete,
                physical_key: None,
                pressed: true,
                repeat: false,
                modifiers: EguiModifiers::NONE,
            };
            let _ = grafia::adapters::keyboard::dispatch_keyboard_event(
                state, &ev, false, false, false, canvas_rect,
            );
        }
        StatefulCanvasAction::ShiftDelete => {
            let ev = Event::Key {
                key: Key::Delete,
                physical_key: None,
                pressed: true,
                repeat: false,
                modifiers: EguiModifiers {
                    shift: true,
                    ..Default::default()
                },
            };
            let _ = grafia::adapters::keyboard::dispatch_keyboard_event(
                state, &ev, true, false, false, canvas_rect,
            );
        }
        StatefulCanvasAction::PenClick(x, y, dx, dy) => {
            state.active_tool = CanvasTool::Pen;
            state.select_state = grafia::interaction::SelectToolState::Idle;
            let p0 = Pos2::new(x, y);
            let p1 = Pos2::new((x + dx).clamp(10.0, 620.0), (y + dy).clamp(10.0, 460.0));
            let mut input = RawInput::default();
            input.events.push(Event::PointerButton {
                pos: p0,
                button: PointerButton::Primary,
                pressed: true,
                modifiers: EguiModifiers::NONE,
            });
            if (p1 - p0).length() > 1.0 {
                input.events.push(Event::PointerMoved(p1));
            }
            input.events.push(Event::PointerButton {
                pos: p1,
                button: PointerButton::Primary,
                pressed: false,
                modifiers: EguiModifiers::NONE,
            });
            let _ = run_canvas_frame(&ctx, state, &theme, input);
        }
        StatefulCanvasAction::Reverse => {
            let _ = state.reverse_selected_or_all_contours();
        }
        StatefulCanvasAction::MakeFirst => {
            let _ = state.make_selected_node_first();
        }
        StatefulCanvasAction::ToggleSmooth => {
            let ev = Event::Key {
                key: Key::Enter,
                physical_key: None,
                pressed: true,
                repeat: false,
                modifiers: EguiModifiers::NONE,
            };
            let _ = grafia::adapters::keyboard::dispatch_keyboard_event(
                state, &ev, false, false, false, canvas_rect,
            );
        }
        StatefulCanvasAction::Nudge(dir, shift, cmd) => {
            let key = match dir % 4 {
                0 => Key::ArrowLeft,
                1 => Key::ArrowRight,
                2 => Key::ArrowUp,
                _ => Key::ArrowDown,
            };
            let ev = Event::Key {
                key,
                physical_key: None,
                pressed: true,
                repeat: false,
                modifiers: EguiModifiers {
                    shift,
                    command: cmd,
                    ..Default::default()
                },
            };
            let _ = grafia::adapters::keyboard::dispatch_keyboard_event(
                state, &ev, shift, cmd, false, canvas_rect,
            );
        }
        StatefulCanvasAction::Undo => {
            let _ = state.undo();
        }
        StatefulCanvasAction::Redo => {
            let _ = state.redo();
        }
    }
}

fn arb_stateful_action() -> impl proptest::strategy::Strategy<Value = StatefulCanvasAction> {
    use proptest::prelude::*;
    prop_oneof![
        (0usize..8, 0usize..24, any::<bool>())
            .prop_map(|(c, n, sh)| StatefulCanvasAction::SelectNode(c, n, sh)),
        (0usize..8, 0usize..16, any::<bool>())
            .prop_map(|(c, s, sh)| StatefulCanvasAction::SelectSegment(c, s, sh)),
        (
            20.0f32..600.0,
            20.0f32..440.0,
            20.0f32..600.0,
            20.0f32..440.0,
            any::<bool>(),
            any::<bool>()
        )
            .prop_map(|(x0, y0, x1, y1, sh, alt)| {
                StatefulCanvasAction::Marquee(x0, y0, x1, y1, sh, alt)
            }),
        Just(StatefulCanvasAction::Delete),
        Just(StatefulCanvasAction::ShiftDelete),
        (40.0f32..580.0, 40.0f32..420.0, -40.0f32..40.0, -40.0f32..40.0)
            .prop_map(|(x, y, dx, dy)| StatefulCanvasAction::PenClick(x, y, dx, dy)),
        Just(StatefulCanvasAction::Reverse),
        Just(StatefulCanvasAction::MakeFirst),
        Just(StatefulCanvasAction::ToggleSmooth),
        (0u8..4, any::<bool>(), any::<bool>())
            .prop_map(|(d, sh, cmd)| StatefulCanvasAction::Nudge(d, sh, cmd)),
        Just(StatefulCanvasAction::Undo),
        Just(StatefulCanvasAction::Redo),
    ]
}

proptest::proptest! {
    #![proptest_config(proptest::test_runner::Config::with_cases(32))]
    #[test]
    fn proptest_stateful_canvas_interactions_preserve_invariants_and_view_consistency(
        actions_o in proptest::collection::vec(arb_stateful_action(), 1..=25),
        actions_a in proptest::collection::vec(arb_stateful_action(), 1..=25),
    ) {
        let mut state_o = GlifoCanvasState::init_glyph_o();
        state_o.fit_viewport_to_glyph(640.0, 480.0);
        state_o.assert_invariants();
        for action in &actions_o {
            apply_stateful_action(&mut state_o, action);
            state_o.assert_invariants();
        }
        assert_all_views_consistent(&state_o);

        let mut state_a = GlifoCanvasState::init_glyph_a();
        state_a.fit_viewport_to_glyph(640.0, 480.0);
        state_a.assert_invariants();
        for action in &actions_a {
            apply_stateful_action(&mut state_a, action);
            state_a.assert_invariants();
        }
        assert_all_views_consistent(&state_a);
    }
}

#[test]
fn test_grafia_host_bridge_load_edit_export_roundtrip() {
    use grafia::{
        AlignmentZone, AnnotationId, AnnotationSubTool, CanvasAnnotation, ContourId, Glyph,
        GlyphContour, GlyphMetrics, GrafiaCanvasResponse, GrafiaCanvasState, GrafiaCanvasWidget,
        GrafiaPreviewWidget, GrafiaTheme, Node, NodeId, ShapeBooleanRole,
    };

    let theme = GrafiaTheme::sonoma_default();
    let mut state = GrafiaCanvasState::new(theme.clone());
    state.selected_nodes.push(NodeId(999));

    let mut custom_contour = GlyphContour::new(
        ContourId(0),
        vec![
            Node::new(NodeId(10), 100.0, 50.0, NodeType::Corner),
            Node::new(NodeId(11), 500.0, 50.0, NodeType::Corner),
            Node::new(NodeId(12), 500.0, 620.0, NodeType::Corner),
            Node::new(NodeId(13), 100.0, 620.0, NodeType::Corner),
        ],
    );
    custom_contour.role = ShapeBooleanRole::Mask;
    custom_contour.visible = false;
    custom_contour.closed = true;

    let mut open_stroke = GlyphContour::new(
        ContourId(1),
        vec![
            Node::new(NodeId(20), 150.0, 300.0, NodeType::Corner),
            Node::new(NodeId(21), 450.0, 300.0, NodeType::Corner),
        ],
    );
    open_stroke.closed = false;

    let annotation = CanvasAnnotation::new_for_sub_tool(
        AnnotationId(7),
        FontPoint::new(300.0, 300.0),
        AnnotationSubTool::Circle,
    );
    let custom_glyph = Glyph {
        name: "CustomDelta".to_string(),
        upm: 2048,
        width: 640.0,
        height: 1400.0,
        contours: vec![custom_contour, open_stroke],
        annotations: vec![annotation.clone()],
    };
    let custom_metrics = GlyphMetrics {
        ascender: 1600.0,
        cap_height: 1400.0,
        x_height: 1024.0,
        baseline: 0.0,
        descender: -448.0,
        alignment_zones: vec![AlignmentZone {
            position: 1400.0,
            size: 24.0,
        }],
    };

    state.load_glyph(custom_glyph.clone(), custom_metrics.clone());
    assert_eq!(state.viewport.upm, 2048);
    assert_eq!(state.metrics, custom_metrics);
    assert!(state.selected_nodes.is_empty());
    assert!(!state.history.can_undo());
    assert_eq!(state.active_open_contour, Some(1));

    let ctx = Context::default();
    state.selected_nodes = vec![NodeId(10)];
    let mut raw = RawInput::default();
    raw.events.push(Event::Key {
        key: Key::ArrowRight,
        physical_key: None,
        pressed: true,
        repeat: false,
        modifiers: egui::Modifiers {
            shift: true,
            ..Default::default()
        },
    });
    let mut resp: Option<GrafiaCanvasResponse> = None;
    let mut out = ctx.run_ui(raw, |ui| {
        resp = Some(GrafiaCanvasWidget::new(&mut state, &theme).show(ui));
        ui.add(GrafiaPreviewWidget::new(&state));
    });
    out.textures_delta.clear();
    assert!(resp.expect("GrafiaCanvasWidget response").changed);

    let exported = state.export_glyph();
    assert_eq!(exported.name, "CustomDelta");
    assert_eq!(exported.upm, 2048);
    assert_eq!(exported.width, 640.0);
    assert_eq!(exported.contours.len(), 2);
    assert_eq!(exported.contours[0].nodes[0].point.x.0, 110.0);
    assert_eq!(exported.contours[0].role, ShapeBooleanRole::Mask);
    assert!(!exported.contours[0].visible);
    assert!(exported.contours[0].closed);
    assert!(!exported.contours[1].closed);
    assert_eq!(exported.annotations, vec![annotation]);

    assert!(state.undo());
    assert_eq!(state.export_glyph().contours[0].nodes[0].point.x.0, 100.0);
    assert!(state.redo());
    assert_eq!(state.export_glyph().contours[0].nodes[0].point.x.0, 110.0);

    // Edge case: load_glyph with upm=0, height=0.0, duplicate NodeId(0), and 1-OnCurve contour with OffCurve nodes marked closed
    let mut single_node_contour = GlyphContour::new(
        ContourId(99),
        vec![
            Node::new(NodeId(0), 10.0, 20.0, NodeType::Corner),
            Node::new(NodeId(0), 15.0, 25.0, NodeType::OffCurve),
        ],
    );
    single_node_contour.closed = true;
    let mut dup_id_contour = GlyphContour::new(
        ContourId(99),
        vec![
            Node::new(NodeId(0), -5.0, 0.0, NodeType::OffCurve),
            Node::new(NodeId(0), 0.0, 0.0, NodeType::Corner),
            Node::new(NodeId(0), 100.0, 0.0, NodeType::Corner),
            Node::new(NodeId(0), 110.0, 0.0, NodeType::OffCurve),
        ],
    );
    dup_id_contour.closed = false;
    let edge_glyph = Glyph {
        name: "EdgeGlyph".to_string(),
        upm: 0,
        width: 500.0,
        height: 0.0,
        contours: vec![single_node_contour, dup_id_contour],
        annotations: Vec::new(),
    };
    state.load_glyph(edge_glyph, custom_metrics);
    assert_eq!(state.viewport.upm, 1);
    assert_eq!(state.glyph.height, 1400.0);
    assert!(!state.is_contour_closed(0));
    assert_eq!(state.contours[0].len(), 1);
    let exported_edge = state.export_glyph();
    assert_eq!(exported_edge.contours[1].nodes.len(), 2);
    assert_ne!(
        exported_edge.contours[1].nodes[0].id,
        exported_edge.contours[1].nodes[1].id
    );
}







