//! Conformance test suite for Curve Manipulation and Node Editing (TC-1237, TC-1238, TC-1241, TC-1244, TC-1245).
//!
//! Normative: SPEC-002 Deel I §4 & Deel II (Oracle Matrix).
//!
//! Asserts:
//! 1. `TC-1238`: Curve Splitting with Pen Tool (De Casteljau 100% shape preservation).
//! 2. `TC-1237`: Reposition Node while Drawing with Pen Tool via Spacebar hold.
//! 3. `TC-1244`: Node Deletion with Curve Healing via cubic reconnection (closed = true).
//! 4. `TC-1245`: Node Deletion without Healing via Alt+Backspace (cut open, closed = false).
//! 5. `TC-1241`: Whole Contour Selection on Double-Click near path segment in SelectTool.

use egui::{pos2, Context, Event, Key, Modifiers, PointerButton, RawInput};
use grafia::adapters::{CanvasTool, CanvasViewMode, GlifoCanvasState, GlifoCanvasWidget};
use grafia::core::{
    extract_segments, split_contour_segment, ContourSegment, FontPoint,
    Node, NodeId, NodeType, ScreenPoint,
};
use grafia::tokens::GlifoTheme;
use kurbo::{CubicBez, ParamCurve, PathEl, Point};

fn frame(ctx: &Context, state: &mut GlifoCanvasState, theme: &GlifoTheme, raw_input: RawInput) {
    let mut full_output = ctx.run_ui(raw_input, |ui| {
        GlifoCanvasWidget::new(state, theme).show(ui);
    });
    full_output.textures_delta.clear();
}

// ============================================================================
// TC-1238: Curve Splitting with Pen Tool
// ============================================================================

#[test]
fn test_tc_1238_curve_splitting_cubic_bezier_100_percent_shape_preservation() {
    // Construct a cubic bezier curve: P0=(100, 100), H1=(150, 300), H2=(350, 300), P3=(400, 100)
    let p0 = Point::new(100.0, 100.0);
    let p1 = Point::new(150.0, 300.0);
    let p2 = Point::new(350.0, 300.0);
    let p3 = Point::new(400.0, 100.0);
    let original_cubic = CubicBez::new(p0, p1, p2, p3);

    let mut contour = vec![
        Node::new(NodeId(1), p0.x, p0.y, NodeType::Corner),
        Node::new(NodeId(2), p1.x, p1.y, NodeType::OffCurve),
        Node::new(NodeId(3), p2.x, p2.y, NodeType::OffCurve),
        Node::new(NodeId(4), p3.x, p3.y, NodeType::Corner),
    ];

    let segments = extract_segments(&contour, false);
    assert_eq!(segments.len(), 1);
    let seg = segments[0];

    // Split at parameter t = 0.42
    let split_t = 0.42;
    let mut next_id = 100;
    let split_node_id = split_contour_segment(&mut contour, seg, split_t, &mut next_id);

    // After split: [Start, H1, in_h, Split, out_h, H2, End] -> 7 nodes
    assert_eq!(contour.len(), 7, "Cubic split must produce exactly 7 nodes (2 on-curve, 4 off-curve, 1 split on-curve)");
    let split_node = contour.iter().find(|n| n.id == split_node_id).expect("Split node must exist");
    assert_eq!(split_node.node_type, NodeType::Smooth, "Split node must be Smooth");

    // Extract the 2 resulting segments
    let new_segments = extract_segments(&contour, false);
    assert_eq!(new_segments.len(), 2, "Contour must now have 2 cubic segments");

    let (c1, c2) = match (new_segments[0], new_segments[1]) {
        (ContourSegment::Cubic { cubic: c1, .. }, ContourSegment::Cubic { cubic: c2, .. }) => (c1, c2),
        _ => panic!("Both segments must be cubic"),
    };

    // Assert 100% curve preservation: For any u in [0, 1], c1 and c2 match original_cubic exactly
    let sample_count = 50;
    for i in 0..=sample_count {
        let u = i as f64 / sample_count as f64;

        // Sample on c1 corresponds to original_cubic at (u * split_t)
        let pt_c1 = c1.eval(u);
        let orig_t1 = u * split_t;
        let pt_orig1 = original_cubic.eval(orig_t1);
        let err1 = (pt_c1.x - pt_orig1.x).hypot(pt_c1.y - pt_orig1.y);
        assert!(err1 < 1e-10, "Curve segment 1 deviation {err1} exceeds threshold at u={u}");

        // Sample on c2 corresponds to original_cubic at (split_t + u * (1.0 - split_t))
        let pt_c2 = c2.eval(u);
        let orig_t2 = split_t + u * (1.0 - split_t);
        let pt_orig2 = original_cubic.eval(orig_t2);
        let err2 = (pt_c2.x - pt_orig2.x).hypot(pt_c2.y - pt_orig2.y);
        assert!(err2 < 1e-10, "Curve segment 2 deviation {err2} exceeds threshold at u={u}");
    }
}

#[test]
fn test_tc_1238_pen_tool_click_on_segment_splits_via_canvas_interaction() {
    let ctx = Context::default();
    let theme = GlifoTheme::sonoma_default();
    let mut state = GlifoCanvasState::init_glyph_a();
    state.active_tool = CanvasTool::Pen;

    // Outer contour of glyph A is 8 corner nodes forming a polygon.
    let initial_node_count = state.contours[0].len();
    assert_eq!(initial_node_count, 8);

    // Segment 0 connects (50.0, 0.0) to (190.0, 0.0) along bottom baseline y = 0.0
    // Midpoint in font units is (120.0, 0.0)
    let mid_font = FontPoint::new(120.0, 0.0);
    let scr_pt = state.viewport.to_screen(mid_font);
    let click_pos = pos2(scr_pt.x.0, scr_pt.y.0);

    // Click Pen tool on the segment
    let mut input = RawInput::default();
    input.events.push(Event::PointerButton {
        pos: click_pos,
        button: PointerButton::Primary,
        pressed: true,
        modifiers: Modifiers::NONE,
    });
    frame(&ctx, &mut state, &theme, input);

    // Contour 0 must now have 9 nodes (1 inserted on-curve node)
    assert_eq!(
        state.contours[0].len(),
        initial_node_count + 1,
        "Clicking Pen tool on segment must split segment and insert a new on-curve node"
    );

    // The newly inserted node must be selected
    assert_eq!(state.selected_nodes.len(), 1);
    let new_node_id = state.selected_nodes[0];
    let inserted = state.find_node(new_node_id).expect("Inserted node must exist");
    assert!((inserted.point.x.0 - 120.0).abs() < 1.0);
    assert!((inserted.point.y.0 - 0.0).abs() < 1.0);
}

#[test]
fn test_tc_1238_pen_tool_click_outside_tolerance_places_new_node() {
    let ctx = Context::default();
    let theme = GlifoTheme::sonoma_default();
    let mut state = GlifoCanvasState::init_glyph_a();
    state.active_tool = CanvasTool::Pen;

    let initial_contours = state.contours.len();

    // Click far away from any contour (e.g. at (800, 800) font space)
    let far_font = FontPoint::new(800.0, 800.0);
    let scr_pt = state.viewport.to_screen(far_font);

    let mut input = RawInput::default();
    input.events.push(Event::PointerButton {
        pos: pos2(scr_pt.x.0, scr_pt.y.0),
        button: PointerButton::Primary,
        pressed: true,
        modifiers: Modifiers::NONE,
    });
    frame(&ctx, &mut state, &theme, input);

    // Because state.active_contour_closed is true, clicking empty canvas starts a new contour
    assert_eq!(state.contours.len(), initial_contours + 1);
    let new_contour = state.contours.last().unwrap();
    assert_eq!(new_contour.len(), 1);
    assert_eq!(new_contour[0].node_type, NodeType::Corner);
}

// ============================================================================
// TC-1237: Reposition Node while Drawing with Pen Tool (Spacebar Drag)
// ============================================================================

#[test]
fn test_tc_1237_spacebar_hold_shifts_node_position_during_pen_drag() {
    let ctx = Context::default();
    let theme = GlifoTheme::sonoma_default();
    let mut state = GlifoCanvasState::new(theme.clone());
    state.active_tool = CanvasTool::Pen;

    // Viewport with 1:1 zoom, pan (0, 0)
    state.viewport.zoom = 1.0;
    state.viewport.pan_offset = ScreenPoint::new(0.0, 0.0);

    // 1. Mouse down at (100, 100) -> places node
    let click_pos = pos2(100.0, 100.0);
    let mut input = RawInput::default();
    input.events.push(Event::PointerButton {
        pos: click_pos,
        button: PointerButton::Primary,
        pressed: true,
        modifiers: Modifiers::NONE,
    });
    frame(&ctx, &mut state, &theme, input);

    assert_eq!(state.contours.len(), 1);
    assert_eq!(state.contours[0].len(), 1);
    let node_id = state.contours[0][0].id;
    let initial_pt = state.contours[0][0].point;

    // 2. Drag without Spacebar to (130, 100) -> extrudes handles, anchor stays at (100, 100)
    let mut input = RawInput::default();
    input.events.push(Event::PointerMoved(pos2(130.0, 100.0)));
    frame(&ctx, &mut state, &theme, input);

    let node_pt_after_handle_drag = state.find_node(node_id).unwrap().point;
    assert_eq!(node_pt_after_handle_drag.x.0, initial_pt.x.0, "Anchor must not move during normal handle drag");

    // 3. Now press and hold Spacebar while continuing drag from (130, 100) to (180, 140) (+50, +40)
    let mut input = RawInput::default();
    input.events.push(Event::Key {
        key: Key::Space,
        physical_key: None,
        pressed: true,
        repeat: false,
        modifiers: Modifiers::NONE,
    });
    input.events.push(Event::PointerMoved(pos2(180.0, 140.0)));
    frame(&ctx, &mut state, &theme, input);

    // TC-1237 assertion: The on-curve node itself MUST shift by the drag delta!
    let node_pt_after_space_drag = state.find_node(node_id).unwrap().point;
    assert!(
        (node_pt_after_space_drag.x.0 - (initial_pt.x.0 + 50.0)).abs() < 1.0,
        "Holding Spacebar during drag must shift on-curve node X by drag delta"
    );
    assert!(
        (node_pt_after_space_drag.y.0 - (initial_pt.y.0 - 40.0)).abs() < 1.0,
        "Holding Spacebar during drag must shift on-curve node Y by drag delta"
    );

    // In Pen drag, Spacebar must NOT change view_mode to PreviewFilled
    assert_eq!(state.view_mode, CanvasViewMode::EditWireframe);

    // 4. Release Spacebar and continue drag to (220, 140) (+40 in X)
    // TC-1237: Releasing Spacebar resumes handle extrusion relative to the relocated anchor
    let mut input = RawInput::default();
    input.events.push(Event::Key {
        key: Key::Space,
        physical_key: None,
        pressed: false,
        repeat: false,
        modifiers: Modifiers::NONE,
    });
    input.events.push(Event::PointerMoved(pos2(220.0, 140.0)));
    frame(&ctx, &mut state, &theme, input);

    // Anchor node must stay at the relocated position
    let final_anchor_pt = state.find_node(node_id).unwrap().point;
    assert_eq!(final_anchor_pt.x.0, node_pt_after_space_drag.x.0);
    assert_eq!(final_anchor_pt.y.0, node_pt_after_space_drag.y.0);

    // Handles must resume extruding from the relocated anchor
    match state.pen_state {
        grafia::interaction::PenToolState::DraggingHandles { anchor, bcp_out, bcp_in, .. } => {
            assert_eq!(anchor.x.0, final_anchor_pt.x.0);
            assert_eq!(anchor.y.0, final_anchor_pt.y.0);
            // Mouse is at (220, 140), anchor is at (150, 60 font space, delta = 70)
            let dx_out = bcp_out.x.0 - anchor.x.0;
            assert!(dx_out > 65.0, "Handle must resume extrusion to match mouse cursor");
            let dx_in = bcp_in.x.0 - anchor.x.0;
            assert!((dx_in + dx_out).abs() < 1.0, "Opposite handle must remain symmetric");
        }
        _ => panic!("PenTool must be in DraggingHandles after releasing Spacebar and moving pointer"),
    }
}

// ============================================================================
// TC-1244 & TC-1245: Node Deletion with Curve Healing vs. Open Cut
// ============================================================================

#[test]
fn test_tc_1244_backspace_on_straight_line_contour_keeps_straight_line_without_brown_dots() {
    let ctx = Context::default();
    let theme = GlifoTheme::sonoma_default();
    let mut state = GlifoCanvasState::init_glyph_a();

    // Contour 0 of glyph A is closed and consists purely of 8 straight-line Corner nodes
    assert!(state.is_contour_closed(0));
    let initial_node_count = state.contours[0].len(); // 8 corner nodes

    // Select node at index 2 (between two straight line segments)
    let target_node_id = state.contours[0][2].id;
    state.selected_nodes = vec![target_node_id];

    // Press Backspace without modifiers
    let mut input = RawInput::default();
    input.events.push(Event::Key {
        key: Key::Backspace,
        physical_key: None,
        pressed: true,
        repeat: false,
        modifiers: Modifiers::NONE,
    });
    frame(&ctx, &mut state, &theme, input);

    // 1. Selected node was removed
    assert!(state.find_node(target_node_id).is_none(), "Target node must be deleted");

    // 2. DEF-GLIFO-003: Deleting a corner node between two straight lines MUST keep a straight line (8 - 1 = 7 nodes, 0 OffCurve handles)
    assert_eq!(
        state.contours[0].len(),
        initial_node_count - 1,
        "Deleting a corner node between straight segments must NOT insert spurious OffCurve handles"
    );
    assert!(state.is_contour_closed(0), "Contour must remain closed");
    assert!(state.selected_nodes.is_empty(), "Selection must be cleared after deletion");

    let off_curve_count = state.contours[0].iter().filter(|n| n.node_type == NodeType::OffCurve).count();
    assert_eq!(off_curve_count, 0, "DEF-GLIFO-003: Straight-line node deletion must produce 0 brown OffCurve dots");
}

#[test]
fn test_tc_1244_backspace_heals_curve_and_keeps_contour_closed() {
    let ctx = Context::default();
    let theme = GlifoTheme::sonoma_default();
    let mut state = GlifoCanvasState::new(theme.clone());

    // Closed cubic contour with 3 Smooth on-curve nodes and 6 OffCurve handles (9 nodes total)
    state.contours = vec![vec![
        Node::new(NodeId(1), 100.0, 0.0, NodeType::Smooth),
        Node::new(NodeId(2), 160.0, 0.0, NodeType::OffCurve),
        Node::new(NodeId(3), 200.0, 40.0, NodeType::OffCurve),
        Node::new(NodeId(4), 200.0, 100.0, NodeType::Smooth),
        Node::new(NodeId(5), 200.0, 160.0, NodeType::OffCurve),
        Node::new(NodeId(6), 160.0, 200.0, NodeType::OffCurve),
        Node::new(NodeId(7), 100.0, 200.0, NodeType::Smooth),
        Node::new(NodeId(8), 40.0, 140.0, NodeType::OffCurve),
        Node::new(NodeId(9), 40.0, 60.0, NodeType::OffCurve),
    ]];
    state.active_contour_closed = true;
    state.set_contour_closed(0, true);
    state.sync_paths_from_contours();

    // Select Smooth node at index 3 (NodeId(4) at (200, 100)) between two cubic segments
    let target_node_id = NodeId(4);
    state.selected_nodes = vec![target_node_id];

    let mut input = RawInput::default();
    input.events.push(Event::Key {
        key: Key::Backspace,
        physical_key: None,
        pressed: true,
        repeat: false,
        modifiers: Modifiers::NONE,
    });
    frame(&ctx, &mut state, &theme, input);

    // 1. Target Smooth node and inner handles removed, replaced by 1 healed cubic segment (2 OffCurve handles)
    // Remaining: 2 Smooth on-curve nodes + 4 OffCurve handles = 6 nodes
    assert!(state.find_node(target_node_id).is_none(), "Target curve node must be deleted");
    assert_eq!(
        state.contours[0].len(),
        6,
        "Curve healing must merge two cubic segments into one cubic segment (6 nodes total)"
    );
    assert!(state.is_contour_closed(0), "Contour must remain closed after curve healing");
    let off_curve_count = state.contours[0].iter().filter(|n| n.node_type == NodeType::OffCurve).count();
    assert_eq!(off_curve_count, 4, "Two closed cubic segments must have 4 OffCurve handles");
}

#[test]
fn test_tc_1245_alt_backspace_cuts_contour_open_without_healing() {
    let ctx = Context::default();
    let theme = GlifoTheme::sonoma_default();
    let mut state = GlifoCanvasState::init_glyph_a();

    assert!(state.is_contour_closed(0));
    let initial_node_count = state.contours[0].len(); // 8

    // Select node at index 1
    let target_node_id = state.contours[0][1].id;
    state.selected_nodes = vec![target_node_id];

    // Press Alt + Backspace
    let mut input = RawInput::default();
    input.events.push(Event::Key {
        key: Key::Backspace,
        physical_key: None,
        pressed: true,
        repeat: false,
        modifiers: Modifiers::ALT,
    });
    frame(&ctx, &mut state, &theme, input);

    // 1. Target node removed
    assert!(state.find_node(target_node_id).is_none());

    // 2. Node count is simply reduced by 1 (NO healing handles added)
    assert_eq!(
        state.contours[0].len(),
        initial_node_count - 1,
        "Alt+Backspace must remove node without adding healing handles"
    );

    // 3. Contour MUST now be cut open (closed = false)!
    assert!(
        !state.is_contour_closed(0),
        "Alt+Backspace must cut the contour open (closed = false)"
    );

    // 4. BezPath must not end in ClosePath
    let path_elements = state.paths[0].elements();
    let has_close = path_elements.iter().any(|el| matches!(el, kurbo::PathEl::ClosePath));
    assert!(
        !has_close,
        "Bezier path for cut-open contour must NOT contain ClosePath element"
    );
}

// ============================================================================
// TC-1241: Whole Contour Selection on Double-Click
// ============================================================================

#[test]
fn test_tc_1241_double_click_near_path_segment_selects_entire_contour() {
    let ctx = Context::default();
    let theme = GlifoTheme::sonoma_default();
    let mut state = GlifoCanvasState::init_glyph_a();
    state.active_tool = CanvasTool::Select;
    state.selected_nodes.clear();

    // Glyph A has 2 contours: contour 0 (outer, 8 nodes), contour 1 (inner, 3 nodes)
    let outer_node_ids: Vec<NodeId> = state.contours[0].iter().map(|n| n.id).collect();
    let inner_node_ids: Vec<NodeId> = state.contours[1].iter().map(|n| n.id).collect();

    // Pick a point near a segment of contour 0 (e.g. at (120, 0) font units, bottom bar of A)
    let mid_font = FontPoint::new(120.0, 0.0);
    let scr_pt = state.viewport.to_screen(mid_font);
    // Offset slightly by 2.0 dp so it is NOT directly on a node
    let click_pos = pos2(scr_pt.x.0, scr_pt.y.0 + 2.0);

    // 1st click
    let mut input = RawInput::default();
    input.events.push(Event::PointerButton {
        pos: click_pos,
        button: PointerButton::Primary,
        pressed: true,
        modifiers: Modifiers::NONE,
    });
    frame(&ctx, &mut state, &theme, input);

    // Release
    let mut input = RawInput::default();
    input.events.push(Event::PointerButton {
        pos: click_pos,
        button: PointerButton::Primary,
        pressed: false,
        modifiers: Modifiers::NONE,
    });
    frame(&ctx, &mut state, &theme, input);

    // 2nd click (double-click) at same location within time window
    let mut input = RawInput::default();
    input.events.push(Event::PointerButton {
        pos: click_pos,
        button: PointerButton::Primary,
        pressed: true,
        modifiers: Modifiers::NONE,
    });
    frame(&ctx, &mut state, &theme, input);

    // TC-1241 assertion: All nodes of contour 0 MUST be selected!
    assert_eq!(
        state.selected_nodes.len(),
        outer_node_ids.len(),
        "Double click near segment must select all nodes in that contour"
    );
    for id in &outer_node_ids {
        assert!(state.selected_nodes.contains(id), "Node {id:?} must be in selected_nodes");
    }

    // Inner contour 1 nodes MUST NOT be selected
    for id in &inner_node_ids {
        assert!(!state.selected_nodes.contains(id), "Inner contour node {id:?} must NOT be selected");
    }
}

#[test]
fn test_tc_1238_pen_tool_click_on_closing_segment_splits_without_corruption() {
    let ctx = Context::default();
    let theme = GlifoTheme::sonoma_default();
    let mut state = GlifoCanvasState::init_glyph_a();
    state.active_tool = CanvasTool::Pen;

    // Segment 7 connects node 7 at (160, 0) to node 0 at (30, 0) (wrap-around segment)
    let initial_len = state.contours[0].len();
    let mid_closing = FontPoint::new(95.0, 0.0);
    let scr_pt = state.viewport.to_screen(mid_closing);

    let mut input = RawInput::default();
    input.events.push(Event::PointerButton {
        pos: pos2(scr_pt.x.0, scr_pt.y.0),
        button: PointerButton::Primary,
        pressed: true,
        modifiers: Modifiers::NONE,
    });
    frame(&ctx, &mut state, &theme, input);

    assert_eq!(state.contours[0].len(), initial_len + 1);
    assert!(state.is_contour_closed(0));

    // Ensure BezPath ends with ClosePath and has no QuadTo corruption
    let els = state.paths[0].elements();
    assert!(matches!(els.last(), Some(PathEl::ClosePath)));
    let has_quad = els.iter().any(|el| matches!(el, PathEl::QuadTo(..)));
    assert!(!has_quad, "Splitting closing segment must not generate corrupt quad_to");
}

#[test]
fn test_tc_1238_extreme_zoom_tolerance() {
    for zoom in [0.01_f32, 100.0_f32] {
        let mut state = GlifoCanvasState::init_glyph_a();
        state.viewport.zoom = zoom;
        let tol_font = (grafia::tokens::SEGMENT_HIT_TOLERANCE_DP as f64) / (zoom as f64);
        let closed_flags: Vec<bool> = (0..state.contours.len())
            .map(|i| state.is_contour_closed(i))
            .collect();

        // Target on segment 0 at (120, 0)
        let hit = grafia::core::hit_test_segments(&state.contours, &closed_flags, FontPoint::new(120.0, 0.0), tol_font);
        assert!(hit.is_some(), "Hit test must resolve within tolerance at zoom {zoom}");
        let h = hit.unwrap();
        assert!(h.t >= 0.0 && h.t <= 1.0, "Parameter t must be within [0, 1] at zoom {zoom}");
    }
}

#[test]
fn test_tc_1244_backspace_heals_closing_node_0_with_cubic_reconnection() {
    let ctx = Context::default();
    let theme = GlifoTheme::sonoma_default();
    let mut state = GlifoCanvasState::new(theme.clone());

    // Closed cubic contour where node 0 is a Smooth node at the wrap-around junction
    state.contours = vec![vec![
        Node::new(NodeId(1), 100.0, 0.0, NodeType::Smooth),
        Node::new(NodeId(2), 160.0, 0.0, NodeType::OffCurve),
        Node::new(NodeId(3), 200.0, 40.0, NodeType::OffCurve),
        Node::new(NodeId(4), 200.0, 100.0, NodeType::Smooth),
        Node::new(NodeId(5), 200.0, 160.0, NodeType::OffCurve),
        Node::new(NodeId(6), 160.0, 200.0, NodeType::OffCurve),
        Node::new(NodeId(7), 100.0, 200.0, NodeType::Smooth),
        Node::new(NodeId(8), 40.0, 140.0, NodeType::OffCurve),
        Node::new(NodeId(9), 40.0, 60.0, NodeType::OffCurve),
    ]];
    state.active_contour_closed = true;
    state.set_contour_closed(0, true);
    state.sync_paths_from_contours();

    // Select node 0 (first node in contour, junction of wrap-around)
    let node0_id = state.contours[0][0].id;
    state.selected_nodes = vec![node0_id];

    let mut input = RawInput::default();
    input.events.push(Event::Key {
        key: Key::Backspace,
        physical_key: None,
        pressed: true,
        repeat: false,
        modifiers: Modifiers::NONE,
    });
    frame(&ctx, &mut state, &theme, input);

    // Node 0 removed, replaced by 2 off-curve healing handles at wrap-around
    assert!(state.find_node(node0_id).is_none());
    assert!(state.is_contour_closed(0));

    // In state.paths[0], closing segment must be CurveTo + ClosePath, NOT QuadTo
    let els = state.paths[0].elements();
    let has_quad = els.iter().any(|el| matches!(el, PathEl::QuadTo(..)));
    assert!(!has_quad, "Closing curve healing must emit CurveTo, never QuadTo");

    let has_curve = els.iter().any(|el| matches!(el, PathEl::CurveTo(..)));
    assert!(has_curve, "Path must contain CurveTo reconnection element");
}

#[test]
fn test_tc_1244_backspace_on_open_contour_endpoint_does_not_loop() {
    let ctx = Context::default();
    let theme = GlifoTheme::sonoma_default();
    let mut state = GlifoCanvasState::new(theme.clone());

    // Create an open contour of 3 nodes: (0, 0) -> (100, 0) -> (200, 0)
    let n1 = Node::new(NodeId(1), 0.0, 0.0, NodeType::Corner);
    let n2 = Node::new(NodeId(2), 100.0, 0.0, NodeType::Corner);
    let n3 = Node::new(NodeId(3), 200.0, 0.0, NodeType::Corner);
    state.contours = vec![vec![n1, n2, n3]];
    state.active_contour_closed = false;
    state.set_contour_closed(0, false);
    state.sync_paths_from_contours();

    // Delete node 1 (endpoint of open contour)
    state.selected_nodes = vec![NodeId(1)];
    let mut input = RawInput::default();
    input.events.push(Event::Key {
        key: Key::Backspace,
        physical_key: None,
        pressed: true,
        repeat: false,
        modifiers: Modifiers::NONE,
    });
    frame(&ctx, &mut state, &theme, input);

    // Must have 2 nodes left, NOT 4 (no healing handles wrapping to other end!)
    assert_eq!(state.contours[0].len(), 2, "Deleting endpoint of open contour must not add wrap handles");
    assert!(!state.is_contour_closed(0));
    let has_close = state.paths[0].elements().iter().any(|el| matches!(el, PathEl::ClosePath));
    assert!(!has_close, "Open contour must not be closed after endpoint deletion");
}

#[test]
fn test_tc_1244_backspace_on_off_curve_node_removes_handle_only() {
    let ctx = Context::default();
    let theme = GlifoTheme::sonoma_default();
    let mut state = GlifoCanvasState::new(theme.clone());

    // Cubic segment: P0, H1, H2, P3
    let p0 = Node::new(NodeId(1), 0.0, 0.0, NodeType::Corner);
    let h1 = Node::new(NodeId(2), 30.0, 50.0, NodeType::OffCurve);
    let h2 = Node::new(NodeId(3), 70.0, 50.0, NodeType::OffCurve);
    let p3 = Node::new(NodeId(4), 100.0, 0.0, NodeType::Corner);
    state.contours = vec![vec![p0, h1, h2, p3]];
    state.active_contour_closed = true;
    state.set_contour_closed(0, true);
    state.sync_paths_from_contours();

    // Delete off-curve handle H1
    state.selected_nodes = vec![NodeId(2)];
    let mut input = RawInput::default();
    input.events.push(Event::Key {
        key: Key::Backspace,
        physical_key: None,
        pressed: true,
        repeat: false,
        modifiers: Modifiers::NONE,
    });
    frame(&ctx, &mut state, &theme, input);

    // Both H1 and H2 of the cubic segment must be removed, reverting the segment to a straight line (2 OnCurve nodes)
    assert_eq!(state.contours[0].len(), 2);
    assert!(state.find_node(NodeId(2)).is_none());
    assert!(state.find_node(NodeId(3)).is_none());
}

#[test]
fn test_tc_1244_multi_node_simultaneous_deletion() {
    let ctx = Context::default();
    let theme = GlifoTheme::sonoma_default();
    let mut state = GlifoCanvasState::init_glyph_a();

    // Select node 2 in contour 0 and node 1 in contour 1
    let id1 = state.contours[0][2].id;
    let id2 = state.contours[1][1].id;
    state.selected_nodes = vec![id1, id2];

    let mut input = RawInput::default();
    input.events.push(Event::Key {
        key: Key::Backspace,
        physical_key: None,
        pressed: true,
        repeat: false,
        modifiers: Modifiers::NONE,
    });
    frame(&ctx, &mut state, &theme, input);

    assert!(state.find_node(id1).is_none());
    assert!(state.find_node(id2).is_none());
    assert!(state.selected_nodes.is_empty());
}

#[test]
fn test_tc_1245_alt_backspace_cuts_open_contour_with_curves_prunes_handles() {
    let ctx = Context::default();
    let theme = GlifoTheme::sonoma_default();
    let mut state = GlifoCanvasState::new(theme.clone());

    // Closed contour with cubic curves: On0, H0_1, H0_2, On1, H1_1, H1_2, On2
    let on0 = Node::new(NodeId(1), 0.0, 0.0, NodeType::Corner);
    let h0_1 = Node::new(NodeId(2), 20.0, 50.0, NodeType::OffCurve);
    let h0_2 = Node::new(NodeId(3), 80.0, 50.0, NodeType::OffCurve);
    let on1 = Node::new(NodeId(4), 100.0, 0.0, NodeType::Corner);
    let h1_1 = Node::new(NodeId(5), 120.0, 50.0, NodeType::OffCurve);
    let h1_2 = Node::new(NodeId(6), 180.0, 50.0, NodeType::OffCurve);
    let on2 = Node::new(NodeId(7), 200.0, 0.0, NodeType::Corner);

    state.contours = vec![vec![on0, h0_1, h0_2, on1, h1_1, h1_2, on2]];
    state.active_contour_closed = true;
    state.set_contour_closed(0, true);
    state.sync_paths_from_contours();

    // Select on1 and Alt+Backspace (cut open)
    state.selected_nodes = vec![NodeId(4)];
    let mut input = RawInput::default();
    input.events.push(Event::Key {
        key: Key::Backspace,
        physical_key: None,
        pressed: true,
        repeat: false,
        modifiers: Modifiers::ALT,
    });
    frame(&ctx, &mut state, &theme, input);

    assert!(!state.is_contour_closed(0));
    // The open contour must NEVER start or end on an OffCurve handle
    let first_node = state.contours[0].first().unwrap();
    let last_node = state.contours[0].last().unwrap();
    assert_ne!(first_node.node_type, NodeType::OffCurve, "Open contour must not start on handle");
    assert_ne!(last_node.node_type, NodeType::OffCurve, "Open contour must not end on handle");
}

#[test]
fn test_tc_1241_shift_double_click_multi_contour_toggle() {
    let ctx = Context::default();
    let theme = GlifoTheme::sonoma_default();
    let mut state = GlifoCanvasState::init_glyph_a();
    state.active_tool = CanvasTool::Select;

    // Start with contour 0 selected
    state.selected_nodes = state.contours[0].iter().map(|n| n.id).collect();
    let initial_count = state.selected_nodes.len();

    // Shift + Double-click near inner contour 1 segment
    // Inner counter of A has segment between (270, 240) and (370, 240) -> mid (320, 240)
    let mid_font = FontPoint::new(320.0, 240.0);
    let scr_pt = state.viewport.to_screen(mid_font);
    let click_pos = pos2(scr_pt.x.0, scr_pt.y.0 + 1.0);

    // 1st click with Shift
    let mut input = RawInput::default();
    input.events.push(Event::PointerButton {
        pos: click_pos,
        button: PointerButton::Primary,
        pressed: true,
        modifiers: Modifiers::SHIFT,
    });
    frame(&ctx, &mut state, &theme, input);

    // Release
    let mut input = RawInput::default();
    input.events.push(Event::PointerButton {
        pos: click_pos,
        button: PointerButton::Primary,
        pressed: false,
        modifiers: Modifiers::SHIFT,
    });
    frame(&ctx, &mut state, &theme, input);

    // 2nd click with Shift (double click)
    let mut input = RawInput::default();
    input.events.push(Event::PointerButton {
        pos: click_pos,
        button: PointerButton::Primary,
        pressed: true,
        modifiers: Modifiers::SHIFT,
    });
    frame(&ctx, &mut state, &theme, input);

    // Both contours must now be selected
    let expected_total = initial_count + state.contours[1].len();
    assert_eq!(state.selected_nodes.len(), expected_total, "Shift+double click must add contour to selection");
}

#[test]
fn test_heal_delete_marquee_selected_on_curve_and_attached_handles_matches_single_on_curve_heal() {
    let state = GlifoCanvasState::init_glyph_o();
    let base_contour = state.contours[0].clone();

    // Node 3 is Smooth OnCurve (510, 250); Node 2 is incoming OffCurve (510, 394); Node 4 is outgoing OffCurve (510, 106)
    assert_eq!(base_contour[2].node_type, NodeType::OffCurve);
    assert_eq!(base_contour[3].node_type, NodeType::Smooth);
    assert_eq!(base_contour[4].node_type, NodeType::OffCurve);

    let id_off_in = base_contour[2].id;
    let id_on_curve = base_contour[3].id;
    let id_off_out = base_contour[4].id;

    let mut contour_single = base_contour.clone();
    let mut next_id_single = 1000;
    grafia::core::heal_delete_nodes(&mut contour_single, true, &[id_on_curve], &mut next_id_single);

    let mut contour_marquee = base_contour;
    let mut next_id_marquee = 1000;
    grafia::core::heal_delete_nodes(
        &mut contour_marquee,
        true,
        &[id_off_in, id_on_curve, id_off_out],
        &mut next_id_marquee,
    );

    assert_eq!(
        contour_marquee.len(),
        contour_single.len(),
        "Marquee-deleting [off_in, on_curve, off_out] must heal to the exact same node count as deleting [on_curve]"
    );
    for (a, b) in contour_single.iter().zip(contour_marquee.iter()) {
        assert_eq!(
            a.node_type, b.node_type,
            "Node types must match (neighboring Smooth nodes must not be degraded to Corner)"
        );
        assert!(
            (a.point.x.0 - b.point.x.0).abs() < 1e-6 && (a.point.y.0 - b.point.y.0).abs() < 1e-6,
            "Healed coordinates must match: expected {:?}, got {:?}",
            a.point,
            b.point
        );
    }
}

#[test]
fn test_cut_open_delete_nodes_strips_orphaned_and_mismatched_interior_off_curve_handles() {
    let state = GlifoCanvasState::init_glyph_o();
    let base_contour = state.contours[0].clone();

    // Case (a): Delete right OnCurve node (index 3) + 1 OffCurve handle from bottom-left segment (index 7)
    let mut contour_a = base_contour.clone();
    grafia::core::cut_open_delete_nodes(&mut contour_a, &[base_contour[3].id, base_contour[7].id]);
    // Verify no interior run of OffCurve nodes has length != 2
    let mut idx = 0;
    while idx < contour_a.len() {
        assert_ne!(
            contour_a[idx].node_type,
            NodeType::OffCurve,
            "Expected OnCurve at boundary index {idx}"
        );
        let mut j = idx + 1;
        while j < contour_a.len() && contour_a[j].node_type == NodeType::OffCurve {
            j += 1;
        }
        let run_len = j - (idx + 1);
        assert!(
            run_len == 0 || run_len == 2,
            "Case (a): Interior OffCurve run between OnCurve nodes must be 0 or 2, got {run_len}"
        );
        idx = j;
    }

    // Case (b1): Delete two non-adjacent OnCurve nodes (index 3 and index 9) separated by cubic segments,
    // which would otherwise leave a run of 4 interior OffCurve handles (indices 7, 8, 10, 11) between OnCurve 6 and OnCurve 0!
    let mut contour_b1 = base_contour.clone();
    grafia::core::cut_open_delete_nodes(&mut contour_b1, &[base_contour[3].id, base_contour[9].id]);
    assert_eq!(
        contour_b1.iter().filter(|n| n.node_type == NodeType::OffCurve).count(),
        0,
        "Case (b1): Deleting two non-adjacent OnCurve nodes must strip the 4 orphaned interior OffCurve handles"
    );

    // Case (b2): Delete two non-adjacent OnCurve nodes (index 3 and index 9) plus one OffCurve handle on each side of index 9 (index 7 and index 11),
    // which would otherwise leave 2 mismatched OffCurve handles (index 8 and index 10) between OnCurve 6 and OnCurve 0!
    let mut contour_b = base_contour.clone();
    grafia::core::cut_open_delete_nodes(
        &mut contour_b,
        &[
            base_contour[3].id,
            base_contour[7].id,
            base_contour[9].id,
            base_contour[11].id,
        ],
    );
    let off_count_b = contour_b
        .iter()
        .filter(|n| n.node_type == NodeType::OffCurve)
        .count();
    assert_eq!(
        off_count_b, 0,
        "Case (b): Mismatched OffCurve handles from two different segments across a deleted OnCurve node must be stripped"
    );

    // Case (c): Delete 3 of the 4 OnCurve nodes (indices 0, 3, 6), leaving only 1 OnCurve node (index 9)
    let mut contour_c = base_contour.clone();
    grafia::core::cut_open_delete_nodes(
        &mut contour_c,
        &[base_contour[0].id, base_contour[3].id, base_contour[6].id],
    );
    assert_eq!(contour_c.len(), 1, "Single surviving OnCurve node must have 0 OffCurve nodes");
    assert_ne!(contour_c[0].node_type, NodeType::OffCurve);
}

