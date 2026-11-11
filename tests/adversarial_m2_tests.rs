//! Adversarial Challenge Test Suite for Milestone 2: GlifoCanvasWidget Interaction Engine.
//!
//! Empirically tests:
//! 1. SelectTool state transitions & 2D marquee intersection accuracy.
//! 2. Empty marquee cancellation and multi-node selection drag.
//! 3. Rapid tool switching (V <-> P, Escape) during active gestures.
//! 4. PenTool anchor placement, mouse-release state cleanup, and contour closing.
//! 5. 45° angle snapping constraints and Option-click smooth/corner toggling.

use egui::{Context, Event, Key, Modifiers as EguiModifiers, PointerButton, Pos2, RawInput, Vec2};
use grafia::adapters::{
    CanvasTool, GlifoCanvasResponse, GlifoCanvasState, GlifoCanvasWidget,
};
use grafia::core::{snap_vector_45_degrees, FontPoint, NodeType};
use grafia::interaction::{PenToolState, SelectToolState};
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
fn test_adversarial_select_marquee_2d_bounds_isolation() {
    let ctx = Context::default();
    let theme = GlifoTheme::sonoma_default();
    let mut state = GlifoCanvasState::init_glyph_a();

    // Outer contour nodes in glyph A:
    // Apex nodes: (300, 700) and (400, 700)
    // Crossbar nodes: (230, 180) and (470, 180)
    // Base nodes: (30, 0), (670, 0), (540, 0), (160, 0)

    // We drag a marquee box covering ONLY the apex:
    // Font X: [200.0, 500.0], Font Y: [650.0, 750.0]
    let top_left = state.viewport.to_screen(FontPoint::new(200.0, 750.0));
    let bottom_right = state.viewport.to_screen(FontPoint::new(500.0, 650.0));

    let p_start = Pos2::new(top_left.x.0, top_left.y.0);
    let p_end = Pos2::new(bottom_right.x.0, bottom_right.y.0);

    // Mouse down
    let mut input = RawInput::default();
    input.events.push(Event::PointerButton {
        pos: p_start,
        button: PointerButton::Primary,
        pressed: true,
        modifiers: EguiModifiers::NONE,
    });
    let _ = run_canvas_frame(&ctx, &mut state, &theme, input);

    // Mouse move
    let mut input = RawInput::default();
    input.events.push(Event::PointerMoved(p_end));
    let _ = run_canvas_frame(&ctx, &mut state, &theme, input);

    // Mouse release to finalize marquee
    let mut input = RawInput::default();
    input.events.push(Event::PointerButton {
        pos: p_end,
        button: PointerButton::Primary,
        pressed: false,
        modifiers: EguiModifiers::NONE,
    });
    let _ = run_canvas_frame(&ctx, &mut state, &theme, input);

    let apex_node_1 = state.contours[0][1].id; // (300, 700)
    let apex_node_2 = state.contours[0][2].id; // (400, 700)

    assert!(
        state.selected_nodes.contains(&apex_node_1),
        "Apex node 1 (300, 700) must be selected"
    );
    assert!(
        state.selected_nodes.contains(&apex_node_2),
        "Apex node 2 (400, 700) must be selected"
    );

    // Nodes at y = 180 (crossbar) have X in [230, 470], which overlaps in X,
    // but their Y coordinate (180) is far outside [650, 750].
    let crossbar_node_1 = state.contours[0][5].id; // (470, 180)
    let crossbar_node_2 = state.contours[0][6].id; // (230, 180)

    assert!(
        !state.selected_nodes.contains(&crossbar_node_1),
        "BUG FOUND: Node at (470, 180) was erroneously selected by marquee covering Y in [650, 750]! Marquee must check 2D bounds, not 1D X-projection!"
    );
    assert!(
        !state.selected_nodes.contains(&crossbar_node_2),
        "BUG FOUND: Node at (230, 180) was erroneously selected by marquee covering Y in [650, 750]!"
    );
}

#[test]
fn test_adversarial_pen_tool_anchor_click_and_hover_bug() {
    let ctx = Context::default();
    let theme = GlifoTheme::sonoma_default();
    let mut state = GlifoCanvasState::new(theme.clone());
    state.active_tool = CanvasTool::Pen;

    let click_pos = Pos2::new(300.0, 300.0);

    // 1. Click to place an anchor (mouse down)
    let mut input = RawInput::default();
    input.events.push(Event::PointerButton {
        pos: click_pos,
        button: PointerButton::Primary,
        pressed: true,
        modifiers: EguiModifiers::NONE,
    });
    let _ = run_canvas_frame(&ctx, &mut state, &theme, input);

    assert_eq!(state.contours.len(), 1);
    assert_eq!(state.contours[0].len(), 1);
    let node_id = state.contours[0][0].id;
    assert_eq!(state.contours[0][0].node_type, NodeType::Corner);

    // 2. Mouse release at the same position (pure click, NO drag)
    let mut input = RawInput::default();
    input.events.push(Event::PointerButton {
        pos: click_pos,
        button: PointerButton::Primary,
        pressed: false,
        modifiers: EguiModifiers::NONE,
    });
    let _ = run_canvas_frame(&ctx, &mut state, &theme, input);

    // After mouse up, PenToolState must be Idle!
    assert_eq!(
        state.pen_state,
        PenToolState::Idle,
        "BUG FOUND: Mouse release after placing anchor must return pen_state to Idle!"
    );

    // 3. Move mouse to aim for next point (hover without mouse button pressed)
    let hover_pos = click_pos + Vec2::new(50.0, 50.0);
    let mut input = RawInput::default();
    input.events.push(Event::PointerMoved(hover_pos));
    let _ = run_canvas_frame(&ctx, &mut state, &theme, input);

    let placed_node = state.find_node(node_id).unwrap();
    assert_eq!(
        placed_node.node_type,
        NodeType::Corner,
        "BUG FOUND: Mouse hover after click-release must NOT turn corner node into Smooth node or extrude handles!"
    );
    assert_eq!(
        state.pen_state,
        PenToolState::Idle,
        "BUG FOUND: Mouse hover without button press must NOT enter DraggingHandles!"
    );
}

#[test]
fn test_adversarial_empty_marquee_cancellation() {
    let ctx = Context::default();
    let theme = GlifoTheme::sonoma_default();
    let mut state = GlifoCanvasState::init_glyph_a();

    // Click on empty space outside the glyph (e.g. at screen 10, 10)
    let p_empty = Pos2::new(10.0, 10.0);
    let p_drag = Pos2::new(30.0, 30.0);

    let mut input = RawInput::default();
    input.events.push(Event::PointerButton {
        pos: p_empty,
        button: PointerButton::Primary,
        pressed: true,
        modifiers: EguiModifiers::NONE,
    });
    let _ = run_canvas_frame(&ctx, &mut state, &theme, input);

    assert!(state.selected_nodes.is_empty());
    assert!(matches!(state.select_state, SelectToolState::MarqueeSelecting { .. }));

    // Drag over empty space
    let mut input = RawInput::default();
    input.events.push(Event::PointerMoved(p_drag));
    let _ = run_canvas_frame(&ctx, &mut state, &theme, input);

    // Release mouse
    let mut input = RawInput::default();
    input.events.push(Event::PointerButton {
        pos: p_drag,
        button: PointerButton::Primary,
        pressed: false,
        modifiers: EguiModifiers::NONE,
    });
    let _ = run_canvas_frame(&ctx, &mut state, &theme, input);

    assert_eq!(state.select_state, SelectToolState::Idle);
    assert!(state.selected_nodes.is_empty(), "Empty marquee must cancel cleanly with 0 nodes selected");
}

#[test]
fn test_adversarial_multi_node_drag_with_shift_snap() {
    let ctx = Context::default();
    let theme = GlifoTheme::sonoma_default();
    let mut state = GlifoCanvasState::init_glyph_a();

    let node1_id = state.contours[0][1].id;
    let node2_id = state.contours[0][2].id;
    let init_pt1 = state.find_node(node1_id).unwrap().point;
    let init_pt2 = state.find_node(node2_id).unwrap().point;

    state.selected_nodes = vec![node1_id, node2_id];

    let start_screen = state.viewport.to_screen(init_pt1);
    let start_pos = Pos2::new(start_screen.x.0, start_screen.y.0);

    // Mouse down on node 1 without shift (to initiate drag of the current multi-selection)
    let mut input = RawInput::default();
    input.events.push(Event::PointerButton {
        pos: start_pos,
        button: PointerButton::Primary,
        pressed: true,
        modifiers: EguiModifiers::NONE,
    });
    let _ = run_canvas_frame(&ctx, &mut state, &theme, input);

    assert_eq!(
        state.selected_nodes.len(),
        2,
        "Mouse down on an already-selected node without Shift must preserve multi-selection"
    );

    // Drag with Shift held towards (100, 30) px
    ctx.input_mut(|i| i.modifiers.shift = true);
    let target_pos = start_pos + Vec2::new(100.0, 30.0);
    let mut input = RawInput::default();
    input.events.push(Event::PointerMoved(target_pos));
    let _ = run_canvas_frame(&ctx, &mut state, &theme, input);


    let new_pt1 = state.find_node(node1_id).unwrap().point;
    let new_pt2 = state.find_node(node2_id).unwrap().point;

    let delta1 = (new_pt1.x.0 - init_pt1.x.0, new_pt1.y.0 - init_pt1.y.0);
    let delta2 = (new_pt2.x.0 - init_pt2.x.0, new_pt2.y.0 - init_pt2.y.0);

    assert_ne!(delta1, (0.0, 0.0), "Selected nodes must move");
    assert_eq!(delta1, delta2, "All selected nodes in multi-selection must move by identical delta");
}


#[test]
fn test_adversarial_rapid_tool_switching_resets_in_flight_states() {
    let ctx = Context::default();
    let theme = GlifoTheme::sonoma_default();
    let mut state = GlifoCanvasState::init_glyph_a();

    // Start a marquee drag in SelectTool
    state.select_state = SelectToolState::MarqueeSelecting {
        start_screen: grafia::core::ScreenPoint::new(10.0, 10.0),
        current_screen: grafia::core::ScreenPoint::new(50.0, 50.0),
    };

    // Press 'P' mid-gesture
    let mut input = RawInput::default();
    input.events.push(Event::Key {
        key: Key::P,
        physical_key: None,
        pressed: true,
        repeat: false,
        modifiers: EguiModifiers::NONE,
    });
    let _ = run_canvas_frame(&ctx, &mut state, &theme, input);

    assert_eq!(state.active_tool, CanvasTool::Pen);
    assert_eq!(state.select_state, SelectToolState::Idle, "Switching to Pen must reset select_state to Idle");

    // Put Pen in DraggingHandles
    state.pen_state = PenToolState::compute_symmetric_handles(
        FontPoint::new(100.0, 100.0),
        (50.0, 50.0),
        false,
    );

    // Press 'V' mid-gesture
    let mut input = RawInput::default();
    input.events.push(Event::Key {
        key: Key::V,
        physical_key: None,
        pressed: true,
        repeat: false,
        modifiers: EguiModifiers::NONE,
    });
    let _ = run_canvas_frame(&ctx, &mut state, &theme, input);

    assert_eq!(state.active_tool, CanvasTool::Select);
    assert_eq!(state.pen_state, PenToolState::Idle, "Switching to Select must reset pen_state to Idle");

    // Press Escape
    state.selected_nodes = vec![grafia::core::NodeId(1)];
    let mut input = RawInput::default();
    input.events.push(Event::Key {
        key: Key::Escape,
        physical_key: None,
        pressed: true,
        repeat: false,
        modifiers: EguiModifiers::NONE,
    });
    let _ = run_canvas_frame(&ctx, &mut state, &theme, input);

    assert!(state.selected_nodes.is_empty(), "Escape must clear selection");
    assert_eq!(state.select_state, SelectToolState::Idle);
    assert_eq!(state.pen_state, PenToolState::Idle);
}

#[test]
fn test_adversarial_pen_tool_contour_closing() {
    let ctx = Context::default();
    let theme = GlifoTheme::sonoma_default();
    let mut state = GlifoCanvasState::new(theme.clone());
    state.active_tool = CanvasTool::Pen;

    // Place 3 points forming a triangle
    let pt0 = Pos2::new(100.0, 100.0);
    let pt1 = Pos2::new(200.0, 100.0);
    let pt2 = Pos2::new(200.0, 200.0);

    for pt in [pt0, pt1, pt2] {
        let mut input = RawInput::default();
        input.events.push(Event::PointerButton {
            pos: pt,
            button: PointerButton::Primary,
            pressed: true,
            modifiers: EguiModifiers::NONE,
        });
        input.events.push(Event::PointerButton {
            pos: pt,
            button: PointerButton::Primary,
            pressed: false,
            modifiers: EguiModifiers::NONE,
        });
        let _ = run_canvas_frame(&ctx, &mut state, &theme, input);
    }

    assert_eq!(state.contours[0].len(), 3, "Contour should have 3 nodes before closing");
    let _start_node_id = state.contours[0][0].id;


    // Now click on the first node pt0 to CLOSE the contour
    let mut input = RawInput::default();
    input.events.push(Event::PointerButton {
        pos: pt0,
        button: PointerButton::Primary,
        pressed: true,
        modifiers: EguiModifiers::NONE,
    });
    input.events.push(Event::PointerButton {
        pos: pt0,
        button: PointerButton::Primary,
        pressed: false,
        modifiers: EguiModifiers::NONE,
    });
    let _ = run_canvas_frame(&ctx, &mut state, &theme, input);

    // Closing the contour should NOT append a 4th redundant duplicate node!
    // Instead it should close contour 0, leaving 3 distinct vertices.
    assert_eq!(
        state.contours[0].len(),
        3,
        "BUG FOUND: Clicking the start node in PenTool appended a duplicate node instead of closing the contour!"
    );
}

#[test]
fn test_adversarial_option_click_toggle_and_empty_space() {
    let ctx = Context::default();
    let theme = GlifoTheme::sonoma_default();
    let mut state = GlifoCanvasState::init_glyph_a();

    let node_id = state.contours[0][0].id;
    assert_eq!(state.find_node(node_id).unwrap().node_type, NodeType::Corner);

    let screen_pt = state.viewport.to_screen(state.find_node(node_id).unwrap().point);
    let click_pos = Pos2::new(screen_pt.x.0, screen_pt.y.0);

    // Option-click on node
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
    assert_eq!(state.find_node(node_id).unwrap().node_type, NodeType::Smooth);

    // Option-click again: back to Corner
    let mut input = RawInput::default();
    input.events.push(Event::PointerButton {
        pos: click_pos,
        button: PointerButton::Primary,
        pressed: true,
        modifiers: alt_modifiers,
    });
    let _ = run_canvas_frame(&ctx, &mut state, &theme, input);
    assert_eq!(state.find_node(node_id).unwrap().node_type, NodeType::Corner);

    // Option-click on empty space: must be a no-op (no panic, no tool state change)
    let empty_pos = Pos2::new(10.0, 10.0);
    let mut input = RawInput::default();
    input.events.push(Event::PointerButton {
        pos: empty_pos,
        button: PointerButton::Primary,
        pressed: true,
        modifiers: alt_modifiers,
    });
    let _ = run_canvas_frame(&ctx, &mut state, &theme, input);
    assert_eq!(state.select_state, SelectToolState::Idle);
    assert!(state.selected_nodes.is_empty());
}

#[test]
fn test_adversarial_snap_vector_45_degrees_all_quadrants() {
    // Test all 8 cardinal and intercardinal angles: 0, 45, 90, 135, 180, 225, 270, 315
    let test_cases = [
        // (input_dx, input_dy, expected_angle_deg)
        (100.0, 5.0, 0.0),       // close to 0° -> 0°
        (100.0, 80.0, 45.0),     // close to 45° -> 45°
        (10.0, 100.0, 90.0),     // close to 90° -> 90°
        (-80.0, 100.0, 135.0),   // close to 135° -> 135°
        (-100.0, 5.0, 180.0),    // close to 180° -> 180°
        (-100.0, -90.0, 225.0),  // close to 225° -> 225°
        (5.0, -100.0, 270.0),    // close to 270° -> 270°
        (90.0, -100.0, 315.0),   // close to 315° -> 315°
    ];

    for (dx, dy, expected_deg) in test_cases {
        let (snapped_dx, snapped_dy) = snap_vector_45_degrees(dx, dy);
        let actual_deg = snapped_dy.atan2(snapped_dx).to_degrees().rem_euclid(360.0);
        let diff = (actual_deg - expected_deg).abs();
        let circular_diff = diff.min(360.0 - diff);
        assert!(
            circular_diff < 1.0,
            "Input ({dx}, {dy}) expected angle {expected_deg}°, got {actual_deg}° (diff {circular_diff}°)"
        );
    }

    // Boundary conditions: (0, 0)
    let (zero_x, zero_y) = snap_vector_45_degrees(0.0, 0.0);
    assert_eq!((zero_x, zero_y), (0.0, 0.0));

    // Sub-epsilon vectors
    let (sub_x, sub_y) = snap_vector_45_degrees(1e-7, 1e-7);
    assert_eq!((sub_x, sub_y), (0.0, 0.0));
}

