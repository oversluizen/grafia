//! Normative COTS Verification Suite for `SPEC-003` (`glifo v0.2.0`):
//! Transform Tools (`R`, `S`), 8-Knob Bounding Box (`Cmd+Alt+Shift+B`), `LassoTool` (`Shift+V`),
//! `ZoomTool` (`Z`, `Cmd+Space`), Canvas Clipboard (`Cmd+C`, `Cmd+X`, `Cmd+V`), and the
//! 7 Topological/Geometric Path Operations + `TransformPanel` (`TC-V020-01` .. `TC-V020-07`).
//!
//! Grounded 100% in Live Reference Vector deterministic Golden Master Vectors.

use egui::{Event, Key, Modifiers as EguiModifiers, PointerButton, Pos2, RawInput, Rect, Vec2};
use grafia::adapters::headless::rasterize_canvas_subregion;
use grafia::adapters::input_dispatcher::InputDispatcher;
use grafia::{
    BBoxAction, CanvasTool, DuplicateWarningKind, FontPoint, GlifoCanvasState, GlifoTheme,
    GlyphContour, MetricsOrigin, Node, NodeId, NodeType, ScreenPoint, TransformGridAnchor,
};

fn default_canvas_rect() -> Rect {
    Rect::from_min_size(Pos2::ZERO, Vec2::new(800.0, 600.0))
}

#[allow(clippy::too_many_arguments)]
fn run_dispatch(
    state: &mut GlifoCanvasState,
    events: Vec<Event>,
    shift: bool,
    alt: bool,
    cmd: bool,
    ctrl: bool,
) -> bool {
    let ctx = egui::Context::default();
    let m = EguiModifiers {
        alt,
        ctrl: ctrl || cmd,
        shift,
        mac_cmd: cmd,
        command: cmd,
    };
    let raw = RawInput {
        screen_rect: Some(default_canvas_rect()),
        time: Some(1.0),
        events,
        ..Default::default()
    };
    ctx.input_mut(|i| i.modifiers = m);
    let theme = state.theme.clone();
    let mut changed = false;
    let mut full_output = ctx.run_ui(raw, |ui| {
        let events_clone = ui.input(|i| i.events.clone());
        let mut disp = InputDispatcher::new(state, &theme, default_canvas_rect());
        changed = disp.dispatch_events(&events_clone, ui);
    });
    full_output.textures_delta.clear();
    changed
}

fn mods(shift: bool, alt: bool, cmd: bool, ctrl: bool) -> EguiModifiers {
    EguiModifiers {
        alt,
        ctrl: ctrl || cmd,
        shift,
        mac_cmd: cmd,
        command: cmd,
    }
}

fn press_key_v020(
    state: &mut GlifoCanvasState,
    key: Key,
    shift: bool,
    alt: bool,
    cmd: bool,
    ctrl: bool,
) {
    let m = mods(shift, alt, cmd, ctrl);
    let _ = run_dispatch(
        state,
        vec![Event::Key {
            key,
            physical_key: Some(key),
            pressed: true,
            repeat: false,
            modifiers: m,
        }],
        shift,
        alt,
        cmd,
        ctrl,
    );
}

fn pointer_press(
    state: &mut GlifoCanvasState,
    scr: ScreenPoint,
    shift: bool,
    alt: bool,
    cmd: bool,
    ctrl: bool,
) {
    let pos = Pos2::new(scr.x.0, scr.y.0);
    let m = mods(shift, alt, cmd, ctrl);
    let _ = run_dispatch(
        state,
        vec![
            Event::PointerMoved(pos),
            Event::PointerButton {
                pos,
                button: PointerButton::Primary,
                pressed: true,
                modifiers: m,
            },
        ],
        shift,
        alt,
        cmd,
        ctrl,
    );
}

fn pointer_move(
    state: &mut GlifoCanvasState,
    scr: ScreenPoint,
    shift: bool,
    alt: bool,
    cmd: bool,
    ctrl: bool,
) {
    let pos = Pos2::new(scr.x.0, scr.y.0);
    let _ = run_dispatch(
        state,
        vec![Event::PointerMoved(pos)],
        shift,
        alt,
        cmd,
        ctrl,
    );
}

fn pointer_release(
    state: &mut GlifoCanvasState,
    scr: ScreenPoint,
    shift: bool,
    alt: bool,
    cmd: bool,
    ctrl: bool,
) {
    let pos = Pos2::new(scr.x.0, scr.y.0);
    let m = mods(shift, alt, cmd, ctrl);
    let _ = run_dispatch(
        state,
        vec![Event::PointerButton {
            pos,
            button: PointerButton::Primary,
            pressed: false,
            modifiers: m,
        }],
        shift,
        alt,
        cmd,
        ctrl,
    );
}

fn contour_signed_area(contour: &[Node]) -> f64 {
    let pts: Vec<FontPoint> = contour.iter().map(|n| n.point).collect();
    grafia::compute_signed_area(&pts)
}

fn set_single_contour(state: &mut GlifoCanvasState, nodes: Vec<Node>, closed: bool) {
    state.contours = vec![nodes.clone()];
    let mut gc = GlyphContour::new(grafia::ContourId(0), nodes);
    gc.closed = closed;
    state.glyph.contours = vec![gc];
    state.active_contour_closed = closed;
    state.active_open_contour = if closed { None } else { Some(0) };
    state.selected_nodes.clear();
    state.sync_paths_from_contours();
    state.assert_invariants();
}

fn assert_pt_near(actual: FontPoint, expected_x: f64, expected_y: f64, tol: f64, label: &str) {
    assert!(
        (actual.x.0 - expected_x).abs() <= tol && (actual.y.0 - expected_y).abs() <= tol,
        "{label}: expected ({expected_x:.6}, {expected_y:.6}), got ({:.6}, {:.6}) (tol={tol})",
        actual.x.0,
        actual.y.0
    );
}

// ============================================================================
// TC-V020-01: 8-Knob Bounding Box Calculation, Active Knobs, 3-Zone Ray Hit-Test & Fallthrough
// Verified Scenarios: M-01, M-02, M-03, M-11, M-30
// ============================================================================
#[test]
fn test_spec003_bbox_calculation_active_knobs_hit_zones_and_priority_fallthrough() {
    let mut state = GlifoCanvasState::new(GlifoTheme::sonoma_default());
    state.viewport.zoom = 1.0;
    state.viewport.pan_offset = ScreenPoint::new(200.0, 400.0);

    // Build a closed contour with 2 OnCurve nodes and 2 unselected OffCurve handles extending above y=200
    let nodes = vec![
        Node::new(NodeId(1), 0.0, 0.0, NodeType::Corner),
        Node::new(NodeId(2), 0.0, 300.0, NodeType::OffCurve),
        Node::new(NodeId(3), 100.0, 300.0, NodeType::OffCurve),
        Node::new(NodeId(4), 100.0, 200.0, NodeType::Corner),
    ];
    set_single_contour(&mut state, nodes, true);

    // M-01: With < 2 selected nodes or show_bounding_box == false, bounding box is not active
    state.selected_nodes = vec![NodeId(1)];
    state.show_bounding_box = true;
    assert!(state.selection_bounds().is_none() || state.active_bbox_knobs().is_empty());

    // Toggle Cmd+Alt+Shift+B to toggle show_bounding_box
    state.show_bounding_box = false;
    press_key_v020(&mut state, Key::B, true, true, true, false);
    assert!(state.show_bounding_box);

    // Select the 2 OnCurve nodes (NodeId(1) at (0,0) and NodeId(4) at (100,200));
    // unselected OffCurve handles (NodeId(2),(3) at y=300) MUST NOT be included in _selectionBounds!
    state.selected_nodes = vec![NodeId(1), NodeId(4)];
    let bounds = state
        .selection_bounds()
        .expect("_selectionBounds must be Some for 2 selected nodes");
    assert!((bounds.min_x - 0.0).abs() < 1e-6);
    assert!((bounds.max_x - 100.0).abs() < 1e-6);
    assert!((bounds.min_y - 0.0).abs() < 1e-6);
    assert!((bounds.max_y - 200.0).abs() < 1e-6);
    assert_eq!(state.active_bbox_knobs().len(), 8);
    assert_eq!(
        state.bounding_box_pill_labels(),
        Some(("100".to_string(), "200".to_string()))
    );

    // M-02: Vertical collinear selection (w <= 1.0 UPM, h * zoom > 4.0 dp) -> only knobs [1, 7] (S, N)
    state.find_node_mut(NodeId(4)).unwrap().point = FontPoint::new(0.5, 200.0);
    state.sync_paths_from_contours();
    assert_eq!(state.active_bbox_knobs(), vec![1, 7]);

    // M-03: Horizontal collinear selection (h <= 1.0 UPM, w * zoom > 4.0 dp) -> only knobs [3, 5] (W, E)
    state.find_node_mut(NodeId(4)).unwrap().point = FontPoint::new(150.0, 0.5);
    state.sync_paths_from_contours();
    assert_eq!(state.active_bbox_knobs(), vec![3, 5]);

    // Restore 100 x 200 box
    state.find_node_mut(NodeId(4)).unwrap().point = FontPoint::new(100.0, 200.0);
    state.sync_paths_from_contours();
    let bounds = state.selection_bounds().unwrap();

    // Verify 3-Zone Ray-Projection Hit-Test on knob 8 (Top-Right at font (100, 200)):
    // Box corner in screen space: (300, 200); Knob center is 1.5 * handle_size outside along (+1, -1) in screen space.
    let box_scr_8 = bounds.box_screen_point(8, &state.viewport);
    let knob_scr_8 = bounds.knob_screen_center(8, &state.viewport);
    let ray_dx = knob_scr_8.x.0 - box_scr_8.x.0;
    let ray_dy = knob_scr_8.y.0 - box_scr_8.y.0;

    // Zone 1 (t = 0.3 <= 0.7): MUST return None (falls through to NodeId(4) sitting at corner!)
    let pt_zone1 = ScreenPoint::new(box_scr_8.x.0 + 0.3 * ray_dx, box_scr_8.y.0 + 0.3 * ray_dy);
    assert_eq!(state.hit_test_bbox_knob(pt_zone1, false, false), None);

    // Zone 2 (t = 1.0 in 0.7..=1.7): Scale(8)
    let pt_zone2 = ScreenPoint::new(box_scr_8.x.0 + 1.0 * ray_dx, box_scr_8.y.0 + 1.0 * ray_dy);
    assert_eq!(
        state.hit_test_bbox_knob(pt_zone2, false, false),
        Some(BBoxAction::Scale(8))
    );

    // Zone 3 (t = 1.85 > 1.7): Rotate(8) on corner knob
    let pt_zone3 = ScreenPoint::new(box_scr_8.x.0 + 1.85 * ray_dx, box_scr_8.y.0 + 1.85 * ray_dy);
    assert_eq!(
        state.hit_test_bbox_knob(pt_zone3, false, false),
        Some(BBoxAction::Rotate(8))
    );

    // Zone 3 on knob 1 (Bottom-Center) -> Slant(1); Zone 3 on knob 5 with Ctrl+Alt -> Slant(5)
    let box_scr_1 = bounds.box_screen_point(1, &state.viewport);
    let knob_scr_1 = bounds.knob_screen_center(1, &state.viewport);
    let pt_slant_1 = ScreenPoint::new(
        box_scr_1.x.0 + 1.85 * (knob_scr_1.x.0 - box_scr_1.x.0),
        box_scr_1.y.0 + 1.85 * (knob_scr_1.y.0 - box_scr_1.y.0),
    );
    assert_eq!(
        state.hit_test_bbox_knob(pt_slant_1, false, false),
        Some(BBoxAction::Slant(1))
    );

    let box_scr_5 = bounds.box_screen_point(5, &state.viewport);
    let knob_scr_5 = bounds.knob_screen_center(5, &state.viewport);
    let pt_slant_5 = ScreenPoint::new(
        box_scr_5.x.0 + 1.85 * (knob_scr_5.x.0 - box_scr_5.x.0),
        box_scr_5.y.0 + 1.85 * (knob_scr_5.y.0 - box_scr_5.y.0),
    );
    assert_eq!(
        state.hit_test_bbox_knob(pt_slant_5, true, true),
        Some(BBoxAction::Slant(5))
    );

    // M-30: Clicking at t = 0.0 (directly on NodeId(4) at (100,200)) drags NodeId(4), NOT BBox knob 8!
    pointer_press(&mut state, box_scr_8, false, false, false, false);
    pointer_move(
        &mut state,
        ScreenPoint::new(box_scr_8.x.0 + 20.0, box_scr_8.y.0),
        false,
        false,
        false,
        false,
    );
    pointer_release(
        &mut state,
        ScreenPoint::new(box_scr_8.x.0 + 20.0, box_scr_8.y.0),
        false,
        false,
        false,
        false,
    );
    // Both selected nodes translated by +20 in X (NodeId(1) -> (20, 0), NodeId(4) -> (120, 200))
    assert_pt_near(state.find_node(NodeId(1)).unwrap().point, 20.0, 0.0, 1e-4, "M-30 node 1");
    assert_pt_near(state.find_node(NodeId(4)).unwrap().point, 120.0, 200.0, 1e-4, "M-30 node 4");
    assert!(state.undo());

    // M-11: Slanted & Space-Panning Marquee Selection:
    // (a) Alt-drag marquee when italic_angle_deg != 0.0 snaps selection_angle_deg = italic_angle_deg
    //     and selects OnCurve only.
    state.selected_nodes.clear();
    state.italic_angle_deg = 12.0;
    state.selection_angle_deg = 0.0;
    state.show_bounding_box_slanted = false;
    let m_start = state.viewport.to_screen(FontPoint::new(-20.0, 220.0));
    let m_mid = state.viewport.to_screen(FontPoint::new(50.0, -20.0));
    let m_panned = state.viewport.to_screen(FontPoint::new(120.0, -20.0));
    pointer_press(&mut state, m_start, false, true, false, false);
    pointer_move(&mut state, m_mid, false, true, false, false);
    pointer_release(&mut state, m_mid, false, true, false, false);
    assert!((state.selection_angle_deg - 12.0).abs() < 1e-6, "M-11(a) Alt marquee must snap selection_angle_deg to italic_angle_deg");
    assert!(state.show_bounding_box_slanted, "M-11(a) Alt marquee with italic_angle_deg != 0 sets show_bounding_box_slanted");

    // (b) Ctrl-drag marquee dynamically adjusts selection_angle_deg in [-60°, +60°]
    state.italic_angle_deg = 0.0;
    state.selected_nodes.clear();
    let c_start = state.viewport.to_screen(FontPoint::new(-60.0, -40.0));
    let c_end = state.viewport.to_screen(FontPoint::new(-10.0, 60.0));
    pointer_press(&mut state, c_start, false, false, false, true);
    pointer_move(&mut state, c_end, false, false, false, true);
    pointer_release(&mut state, c_end, false, false, false, true);
    assert!(state.selection_angle_deg > 10.0 && state.selection_angle_deg <= 60.0, "M-11(b) Ctrl marquee dynamic slant angle");
    assert!(state.show_bounding_box_slanted);

    // (c) Holding Space during active MarqueeSelecting pans marquee_start without switching to PreviewFilled!
    state.selection_angle_deg = 0.0;
    state.show_bounding_box_slanted = false;
    state.selected_nodes.clear();
    let sp_start = state.viewport.to_screen(FontPoint::new(-80.0, 220.0));
    let sp_mid = state.viewport.to_screen(FontPoint::new(30.0, -20.0));
    pointer_press(&mut state, sp_start, false, false, false, false);
    pointer_move(&mut state, sp_mid, false, false, false, false);
    assert_eq!(state.selected_nodes, vec![NodeId(1)]);
    // Press Space while MarqueeSelecting and move +90 dx in screen space so marquee_start shifts from -80 to +10,
    // and marquee now covers x in [10, 120] -> selecting NodeId(4) at (100,200) instead of NodeId(1) at (0,0)!
    press_key_v020(&mut state, Key::Space, false, false, false, false);
    assert_eq!(state.view_mode, grafia::CanvasViewMode::EditWireframe, "M-11(c) Space during marquee must NOT enter PreviewFilled");
    pointer_move(&mut state, m_panned, false, false, false, false);
    pointer_release(&mut state, m_panned, false, false, false, false);
    assert!(state.selected_nodes.contains(&NodeId(4)), "M-11(c) Space-panned marquee must update live selection to cover NodeId(4)");
    assert!(!state.selected_nodes.contains(&NodeId(1)), "M-11(c) Space-panned marquee must move start past NodeId(1)");

    // Visual render smoke check with bounding box and pills active
    let pix = rasterize_canvas_subregion(&state, &state.theme, 600, 450);
    assert_eq!(pix.width(), 600);
}

// ============================================================================
// TC-V020-02: Bounding-Box Scale, Rotate, Slant, Alt-Center Pivot & 3-Pass OffCurve Alignment
// Verified Scenarios: M-04, M-05, M-06, M-07, M-08, M-09, M-10
// ============================================================================
#[test]
fn test_spec003_bbox_scale_rotate_slant_pivots_and_partial_bcp_alignment() {
    let mut state = GlifoCanvasState::new(GlifoTheme::sonoma_default());
    state.viewport.zoom = 1.0;
    state.viewport.pan_offset = ScreenPoint::new(200.0, 400.0);
    state.show_bounding_box = true;

    // Closed contour: P0=(0,0), P1=(100,0), P2=(100,100) Smooth with unselected handle H=(100,140), P3=(0,200)
    let nodes = vec![
        Node::new(NodeId(1), 0.0, 0.0, NodeType::Corner),
        Node::new(NodeId(2), 100.0, 0.0, NodeType::Corner),
        Node::new(NodeId(3), 100.0, 100.0, NodeType::Smooth),
        Node::new(NodeId(4), 100.0, 140.0, NodeType::OffCurve),
        Node::new(NodeId(5), 50.0, 200.0, NodeType::OffCurve),
        Node::new(NodeId(6), 0.0, 200.0, NodeType::Corner),
    ];
    set_single_contour(&mut state, nodes, true);
    state.selected_nodes = vec![NodeId(1), NodeId(2), NodeId(3)]; // box is [0,100] x [0,100]

    // M-04: Drag knob 8 (Top-Right of [0,100]x[0,100]) by (+50, +50) in font space -> scales 1.5x around knob 0 (0,0)
    let bounds = state.selection_bounds().unwrap();
    let k8_scr = bounds.knob_screen_center(8, &state.viewport);
    let target_scr = ScreenPoint::new(k8_scr.x.0 + 50.0, k8_scr.y.0 - 50.0);
    pointer_press(&mut state, k8_scr, false, false, false, false);
    pointer_move(&mut state, target_scr, false, false, false, false);
    pointer_release(&mut state, target_scr, false, false, false, false);

    assert_pt_near(state.find_node(NodeId(1)).unwrap().point, 0.0, 0.0, 1e-4, "M-04 pivot");
    assert_pt_near(state.find_node(NodeId(2)).unwrap().point, 150.0, 0.0, 1e-4, "M-04 P1");
    assert_pt_near(state.find_node(NodeId(3)).unwrap().point, 150.0, 150.0, 1e-4, "M-04 P2");
    // Pass 3: unselected OffCurve handle NodeId(4) attached to selected NodeId(3) shifted rigidly by (+50, +50) -> (150, 190)!
    assert_pt_near(
        state.find_node(NodeId(4)).unwrap().point,
        150.0,
        190.0,
        1e-4,
        "M-04 unselected attached OffCurve handle",
    );

    // Undo restores original coordinates
    assert!(state.undo());

    // M-06: Alt-drag on knob 5 (Middle-Right of [0,100]x[0,100]) by +25 dx -> scales around center (50,50) with 2x delta (sx = 1.5), NEVER duplicating contours!
    let bounds = state.selection_bounds().unwrap();
    let k5_scr = bounds.knob_screen_center(5, &state.viewport);
    let k5_end = ScreenPoint::new(k5_scr.x.0 + 25.0, k5_scr.y.0);
    pointer_press(&mut state, k5_scr, false, true, false, false);
    pointer_move(&mut state, k5_end, false, true, false, false);
    pointer_release(&mut state, k5_end, false, true, false, false);
    assert_eq!(state.contours.len(), 1, "M-06: BBox Alt-drag must NEVER duplicate contours");
    assert_pt_near(state.find_node(NodeId(1)).unwrap().point, -25.0, 0.0, 1e-4, "M-06 left");
    assert_pt_near(state.find_node(NodeId(2)).unwrap().point, 125.0, 0.0, 1e-4, "M-06 right");
    assert!(state.undo());

    // M-05: Shift-drag on horizontal knob 5 by +50 dx -> uniform 2D scale (sx = 1.5, sy = 1.5) around opposite knob 3 (0, 50)
    let bounds = state.selection_bounds().unwrap();
    let k5_scr = bounds.knob_screen_center(5, &state.viewport);
    let k5_shift_end = ScreenPoint::new(k5_scr.x.0 + 50.0, k5_scr.y.0);
    pointer_press(&mut state, k5_scr, true, false, false, false);
    pointer_move(&mut state, k5_shift_end, true, false, false, false);
    pointer_release(&mut state, k5_shift_end, true, false, false, false);
    assert_pt_near(state.find_node(NodeId(3)).unwrap().point, 150.0, 125.0, 1e-4, "M-05 Shift uniform");
    assert!(state.undo());

    // M-08: Negative scale (dragging knob 5 by -200 dx past opposite knob 3 -> sx = -1.0) mirrors coordinates without reversing node order
    let bounds = state.selection_bounds().unwrap();
    let k5_scr = bounds.knob_screen_center(5, &state.viewport);
    let k5_neg_end = ScreenPoint::new(k5_scr.x.0 - 200.0, k5_scr.y.0);
    pointer_press(&mut state, k5_scr, false, false, false, false);
    pointer_move(&mut state, k5_neg_end, false, false, false, false);
    pointer_release(&mut state, k5_neg_end, false, false, false, false);
    assert_eq!(state.contours[0][0].id, NodeId(1), "M-08: winding array order must not flip");
    assert_pt_near(state.find_node(NodeId(2)).unwrap().point, -100.0, 0.0, 1e-4, "M-08 mirrored P1");
    assert!(state.undo());

    // M-09: Drag in Zone 3 (t = 1.85) outside corner knob 8 with Shift -> snaps rotation to 45° around opposite knob 0 (0,0)
    let bounds = state.selection_bounds().unwrap();
    let box_scr_8 = bounds.box_screen_point(8, &state.viewport);
    let knob_scr_8 = bounds.knob_screen_center(8, &state.viewport);
    let rot_start = ScreenPoint::new(
        box_scr_8.x.0 + 1.85 * (knob_scr_8.x.0 - box_scr_8.x.0),
        box_scr_8.y.0 + 1.85 * (knob_scr_8.y.0 - box_scr_8.y.0),
    );
    // Rotate toward +Y axis (~45 deg CCW in font space from (100,100) around (0,0))
    let rot_end = state.viewport.to_screen(FontPoint::new(0.0, 150.0));
    pointer_press(&mut state, rot_start, true, false, false, false);
    pointer_move(&mut state, rot_end, true, false, false, false);
    pointer_release(&mut state, rot_end, true, false, false, false);
    // P1=(100,0) rotated +45 deg around (0,0) -> (70.710678, 70.710678)
    let cos45 = 100.0 * std::f64::consts::FRAC_1_SQRT_2;
    assert_pt_near(state.find_node(NodeId(2)).unwrap().point, cos45, cos45, 1e-3, "M-09 45deg BBox rotate");
    // SPEC-003 §2.1.5.3 Pass 3: NodeId(3) is Smooth at (100,100) -> rotated +45 deg around (0,0) to (0, 141.421356).
    // Its incoming neighbor NodeId(2) was also transformed to (70.710678, 70.710678), so vector b->a is (-0.707107, +0.707107).
    // Therefore unselected outgoing OffCurve handle NodeId(4) (initial length 40) MUST be rotated collinear to
    // (0 - 40*0.707107, 141.421356 + 40*0.707107) = (-28.284271, 169.705627), NOT merely translated to (0, 181.421356)!
    let p3_rot = state.find_node(NodeId(3)).unwrap().point;
    let h4_rot = state.find_node(NodeId(4)).unwrap().point;
    assert_pt_near(
        h4_rot,
        p3_rot.x.0 - 40.0 * std::f64::consts::FRAC_1_SQRT_2,
        p3_rot.y.0 + 40.0 * std::f64::consts::FRAC_1_SQRT_2,
        1e-2,
        "M-04/M-09 Pass 3 Smooth collinearity rotation of unselected OffCurve handle",
    );
    assert!(state.undo());

    // M-07: Shift + Alt drag on corner knob 8 by (+25, +10) -> proportional scale around center (50, 50)
    // with 2x delta (sx = (100 + 50)/100 = 1.5, sy = (100 + 20)/100 = 1.2 -> max = 1.5), NEVER duplicating!
    let bounds = state.selection_bounds().unwrap();
    let k8_scr = bounds.knob_screen_center(8, &state.viewport);
    let k8_sa_end = ScreenPoint::new(k8_scr.x.0 + 25.0, k8_scr.y.0 - 10.0);
    pointer_press(&mut state, k8_scr, true, true, false, false);
    pointer_move(&mut state, k8_sa_end, true, true, false, false);
    pointer_release(&mut state, k8_sa_end, true, true, false, false);
    assert_eq!(state.contours.len(), 1, "M-07 must never duplicate");
    assert_pt_near(state.find_node(NodeId(1)).unwrap().point, -25.0, -25.0, 1e-4, "M-07 Shift+Alt center pivot P0");
    assert_pt_near(state.find_node(NodeId(3)).unwrap().point, 125.0, 125.0, 1e-4, "M-07 Shift+Alt center pivot P2");
    assert!(state.undo());

    // M-10: Drag in Zone 3 outside knob 1 (Bottom-Center at (50, 0)) horizontally by -20 dx in font space
    // -> horizontal shear around opposite knob 7 (50, 100): at y=0 (dy=-100, h=100), x shifts by -20!
    let bounds = state.selection_bounds().unwrap();
    let box_scr_1 = bounds.box_screen_point(1, &state.viewport);
    let knob_scr_1 = bounds.knob_screen_center(1, &state.viewport);
    let slant_start = ScreenPoint::new(
        box_scr_1.x.0 + 1.85 * (knob_scr_1.x.0 - box_scr_1.x.0),
        box_scr_1.y.0 + 1.85 * (knob_scr_1.y.0 - box_scr_1.y.0),
    );
    let slant_end = ScreenPoint::new(slant_start.x.0 - 20.0, slant_start.y.0);
    pointer_press(&mut state, slant_start, false, false, false, false);
    pointer_move(&mut state, slant_end, false, false, false, false);
    pointer_release(&mut state, slant_end, false, false, false, false);
    assert_pt_near(state.find_node(NodeId(1)).unwrap().point, -17.563117, 0.0, 1e-2, "M-10 slant bottom node P0");
    assert_pt_near(state.find_node(NodeId(3)).unwrap().point, 100.0, 100.0, 1e-3, "M-10 slant top node P2 unchanged");
    assert!(state.undo());
}

// ============================================================================
// TC-V020-03: RotateTool (R) & ScaleTool (S) Shared Origin, Relocation, Cmd-Select & Shift Zones
// Verified Scenarios: M-12, M-13, M-14, M-15, M-16, M-17, M-31
// ============================================================================
#[test]
fn test_spec003_rotate_and_scale_tools_origin_relocation_cmd_update_and_shift_zones() {
    let mut state = GlifoCanvasState::new(GlifoTheme::sonoma_default());
    state.viewport.zoom = 1.0;
    state.viewport.pan_offset = ScreenPoint::new(200.0, 400.0);

    let nodes = vec![
        Node::new(NodeId(1), 0.0, 0.0, NodeType::Corner),
        Node::new(NodeId(2), 100.0, 0.0, NodeType::Corner),
        Node::new(NodeId(3), 100.0, 100.0, NodeType::Corner),
        Node::new(NodeId(4), 0.0, 100.0, NodeType::Corner),
    ];
    set_single_contour(&mut state, nodes, true);
    state.selected_nodes = vec![NodeId(1), NodeId(2), NodeId(3), NodeId(4)];

    // Press 'R' to activate RotateTool -> transform_origin initializes to center (50, 50)
    press_key_v020(&mut state, Key::R, false, false, false, false);
    assert_eq!(state.active_tool, CanvasTool::Rotate);
    assert_pt_near(state.transform_origin, 50.0, 50.0, 1e-6, "Initial R origin");

    // M-12: Click without drag (<= 2.0 dp) at font (12.4, 18.6) with round_to_grid=true snaps to (12.0, 19.0),
    // whereas holding Ctrl bypasses grid snap and keeps (12.4, 18.6)! Then click (0, 0).
    state.round_to_grid = true;
    state.grid_step_upm = 1.0;
    let can_undo_before = state.history.can_undo();
    let frac_scr = state.viewport.to_screen(FontPoint::new(12.4, 18.6));
    pointer_press(&mut state, frac_scr, false, false, false, false);
    pointer_release(&mut state, frac_scr, false, false, false, false);
    assert_pt_near(state.transform_origin, 12.0, 19.0, 1e-6, "M-12 click rounds to grid when !Ctrl");
    pointer_press(&mut state, frac_scr, false, false, false, true);
    pointer_release(&mut state, frac_scr, false, false, false, true);
    assert_pt_near(state.transform_origin, 12.4, 18.6, 1e-4, "M-12 Ctrl click bypasses grid snap");
    state.round_to_grid = false;

    let origin_target_scr = state.viewport.to_screen(FontPoint::new(0.0, 0.0));
    pointer_press(&mut state, origin_target_scr, false, false, false, false);
    pointer_release(&mut state, origin_target_scr, false, false, false, false);
    assert_pt_near(state.transform_origin, 0.0, 0.0, 1e-6, "M-12 click relocates origin");
    assert_eq!(state.history.can_undo(), can_undo_before);

    // Switching directly from 'R' to 'S' preserves manually relocated transform_origin (0, 0)!
    press_key_v020(&mut state, Key::S, false, false, false, false);
    assert_eq!(state.active_tool, CanvasTool::Scale);
    assert_pt_near(state.transform_origin, 0.0, 0.0, 1e-6, "R->S preserves custom origin");

    // M-13: Dragging within 3.0 dp of transform_origin moves transform_origin live to (10, 20)
    let start_near_origin = ScreenPoint::new(origin_target_scr.x.0 + 1.5, origin_target_scr.y.0);
    let new_origin_scr = state.viewport.to_screen(FontPoint::new(10.0, 20.0));
    pointer_press(&mut state, start_near_origin, false, false, false, false);
    pointer_move(&mut state, new_origin_scr, false, false, false, false);
    pointer_release(&mut state, new_origin_scr, false, false, false, false);
    assert_pt_near(state.transform_origin, 10.0, 20.0, 1e-6, "M-13 drag origin");

    // Relocate origin back to (0, 0)
    pointer_press(&mut state, origin_target_scr, false, false, false, false);
    pointer_release(&mut state, origin_target_scr, false, false, false, false);

    // M-15: ScaleTool Shift Zone 1 (|sin(theta)| < 0.2, near horizontal drag from (100, 100) to (200, 105)):
    // Horizontal-only scale (sx = 2.0, sy = 1.0)!
    let s_start = state.viewport.to_screen(FontPoint::new(100.0, 100.0));
    let s_horiz = state.viewport.to_screen(FontPoint::new(200.0, 105.0));
    pointer_press(&mut state, s_start, true, false, false, false);
    pointer_move(&mut state, s_horiz, true, false, false, false);
    pointer_release(&mut state, s_horiz, true, false, false, false);
    assert_pt_near(state.find_node(NodeId(3)).unwrap().point, 200.0, 100.0, 1e-4, "M-15 Shift horiz scale");
    assert!(state.undo());

    // M-16: ScaleTool Shift Zone 2 (|sin(theta)| > 0.86, near vertical drag from (100, 100) to (105, 200)):
    // Vertical-only scale (sx = 1.0, sy = 2.0)!
    let s_vert = state.viewport.to_screen(FontPoint::new(105.0, 200.0));
    pointer_press(&mut state, s_start, true, false, false, false);
    pointer_move(&mut state, s_vert, true, false, false, false);
    pointer_release(&mut state, s_vert, true, false, false, false);
    assert_pt_near(state.find_node(NodeId(3)).unwrap().point, 100.0, 200.0, 1e-4, "M-16 Shift vert scale");
    assert!(state.undo());

    // M-17: ScaleTool Shift Zone 3 (0.2 <= |sin(theta)| <= 0.86, diagonal drag from (100, 100) to (180, 140)):
    // Uniform proportional scale max(1.8, 1.4) = 1.8 -> (180, 180)!
    let s_diag = state.viewport.to_screen(FontPoint::new(180.0, 140.0));
    pointer_press(&mut state, s_start, true, false, false, false);
    pointer_move(&mut state, s_diag, true, false, false, false);
    pointer_release(&mut state, s_diag, true, false, false, false);
    assert_pt_near(state.find_node(NodeId(3)).unwrap().point, 180.0, 180.0, 1e-4, "M-17 Shift uniform max");
    assert!(state.undo());

    // M-14: Switch to RotateTool ('R'), drag with Shift from (100, 0) to (70, 90) -> snaps to +45° around (0, 0)
    press_key_v020(&mut state, Key::R, false, false, false, false);
    let r_start = state.viewport.to_screen(FontPoint::new(100.0, 0.0));
    let r_end = state.viewport.to_screen(FontPoint::new(70.0, 90.0));
    pointer_press(&mut state, r_start, true, true, false, false);
    pointer_move(&mut state, r_end, true, true, false, false);
    pointer_release(&mut state, r_end, true, true, false, false);
    assert_eq!(state.contours.len(), 1, "M-14: RotateTool Alt must never duplicate");
    let cos45 = 100.0 * std::f64::consts::FRAC_1_SQRT_2;
    assert_pt_near(state.find_node(NodeId(2)).unwrap().point, cos45, cos45, 1e-3, "M-14 Rotate 45deg");
    assert!(state.undo());

    // M-31: Temporary Cmd-Select while in RotateTool updates selection to [NodeId(2), NodeId(3)]
    // and automatically updates transform_origin to the new selection's center (100, 50)!
    let m_tl = state.viewport.to_screen(FontPoint::new(80.0, 120.0));
    let m_br = state.viewport.to_screen(FontPoint::new(120.0, -20.0));
    pointer_press(&mut state, m_tl, false, false, true, false);
    pointer_move(&mut state, m_br, false, false, true, false);
    pointer_release(&mut state, m_br, false, false, true, false);
    assert_eq!(state.selected_nodes, vec![NodeId(2), NodeId(3)]);
    assert_pt_near(state.transform_origin, 100.0, 50.0, 1e-4, "M-31 Cmd-Select updates transform_origin");
}

// ============================================================================
// TC-V020-04: LassoTool (Shift+V) Segment-Drag Suppression, Sampling & Modifier Matrix
// Verified Scenarios: M-18
// ============================================================================
#[test]
fn test_spec003_lasso_tool_freeform_polygon_segment_suppression_and_modifiers() {
    let mut state = GlifoCanvasState::new(GlifoTheme::sonoma_default());
    state.viewport.zoom = 1.0;
    state.viewport.pan_offset = ScreenPoint::new(200.0, 400.0);

    let nodes = vec![
        Node::new(NodeId(1), 0.0, 0.0, NodeType::Corner),
        Node::new(NodeId(2), 30.0, 60.0, NodeType::OffCurve),
        Node::new(NodeId(3), 70.0, 60.0, NodeType::OffCurve),
        Node::new(NodeId(4), 100.0, 0.0, NodeType::Corner),
        Node::new(NodeId(5), 100.0, 100.0, NodeType::Corner),
        Node::new(NodeId(6), 0.0, 100.0, NodeType::Corner),
    ];
    set_single_contour(&mut state, nodes, true);

    // Activate LassoTool via Shift+V
    press_key_v020(&mut state, Key::V, true, false, false, false);
    assert_eq!(state.active_tool, CanvasTool::Lasso);
    assert!(!state.active_tool.allows_path_segment_interaction());

    // M-18: Start drag directly on top of the cubic segment belly (50, 45) -> MUST start Lasso, NOT DraggingSegment!
    let p_start = state.viewport.to_screen(FontPoint::new(50.0, 45.0));
    let p_a = state.viewport.to_screen(FontPoint::new(-20.0, -20.0));
    let p_b = state.viewport.to_screen(FontPoint::new(60.0, -20.0));
    let p_c = state.viewport.to_screen(FontPoint::new(60.0, 80.0));
    let p_d = state.viewport.to_screen(FontPoint::new(-20.0, 80.0));

    // 1. No modifiers: replaces selection with both OnCurve (NodeId(1)) and OffCurve (NodeId(2)) inside loop
    pointer_press(&mut state, p_start, false, false, false, false);
    pointer_move(&mut state, p_c, false, false, false, false);
    pointer_move(&mut state, p_d, false, false, false, false);
    pointer_move(&mut state, p_a, false, false, false, false);
    pointer_move(&mut state, p_b, false, false, false, false);
    pointer_release(&mut state, p_b, false, false, false, false);
    assert_pt_near(
        state.find_node(NodeId(2)).unwrap().point,
        30.0,
        60.0,
        1e-6,
        "Curve handle must not be dragged by Lasso",
    );
    assert_eq!(state.selected_nodes, vec![NodeId(1), NodeId(2)]);

    // 2. Alt modifier: selects OnCurve ONLY (NodeId(1)), ignoring OffCurve (NodeId(2))
    pointer_press(&mut state, p_a, false, true, false, false);
    pointer_move(&mut state, p_b, false, true, false, false);
    pointer_move(&mut state, p_c, false, true, false, false);
    pointer_move(&mut state, p_d, false, true, false, false);
    pointer_release(&mut state, p_d, false, true, false, false);
    assert_eq!(state.selected_nodes, vec![NodeId(1)]);

    // 3. Shift + Alt: XOR-toggles OnCurve nodes inside loop (toggles NodeId(1) off while adding NodeId(6) at (0, 100))
    let p_top = state.viewport.to_screen(FontPoint::new(-20.0, 120.0));
    let p_top_r = state.viewport.to_screen(FontPoint::new(40.0, 120.0));
    pointer_press(&mut state, p_a, true, true, false, false);
    pointer_move(&mut state, p_b, true, true, false, false);
    pointer_move(&mut state, p_top_r, true, true, false, false);
    pointer_move(&mut state, p_top, true, true, false, false);
    pointer_release(&mut state, p_top, true, true, false, false);
    assert_eq!(state.selected_nodes, vec![NodeId(6)]);
}

// ============================================================================
// TC-V020-05: ZoomTool (Z & Cmd+Space) 23-Step Discrete Click Zoom & 10.0 dp Marquee Threshold
// Verified Scenarios: M-19, M-20
// ============================================================================
#[test]
fn test_spec003_zoom_tool_discrete_steps_and_marquee_10dp_threshold() {
    let mut state = GlifoCanvasState::init_glyph_a();
    state.viewport.zoom = 1.0;
    state.viewport.pan_offset = ScreenPoint::new(200.0, 400.0);

    // Activate ZoomTool via 'Z'
    press_key_v020(&mut state, Key::Z, false, false, false, false);
    assert_eq!(state.active_tool, CanvasTool::Zoom);

    // M-19: Click (< 10.0 dp movement) at screen (300, 250) steps from 1.0 to 1.5 in ZOOM_STEPS,
    // keeping font point under cursor invariant!
    let click_scr = ScreenPoint::new(300.0, 250.0);
    let font_before = state.viewport.to_font(click_scr);
    pointer_press(&mut state, click_scr, false, false, false, false);
    pointer_release(&mut state, click_scr, false, false, false, false);
    assert!((state.viewport.zoom - 1.5).abs() < 1e-5);
    let font_after = state.viewport.to_font(click_scr);
    assert_pt_near(font_after, font_before.x.0, font_before.y.0, 1e-3, "M-19 anchor preserved");

    // Alt + Click steps down from 1.5 to 1.0 in ZOOM_STEPS
    pointer_press(&mut state, click_scr, false, true, false, false);
    pointer_release(&mut state, click_scr, false, true, false, false);
    assert!((state.viewport.zoom - 1.0).abs() < 1e-5);

    // M-20: Marquee drag with w + h >= 10.0 dp (e.g. 200 x 150 dp on 800 x 600 canvas -> 4x zoom in)
    let m_start = ScreenPoint::new(200.0, 150.0);
    let m_end = ScreenPoint::new(400.0, 300.0);
    let center_font = state.viewport.to_font(ScreenPoint::new(300.0, 225.0));
    pointer_press(&mut state, m_start, false, false, false, false);
    pointer_move(&mut state, m_end, false, false, false, false);
    pointer_release(&mut state, m_end, false, false, false, false);
    assert!((state.viewport.zoom - 4.0).abs() < 1e-4);
    let viewport_center_font = state.viewport.to_font(ScreenPoint::new(400.0, 300.0));
    assert_pt_near(viewport_center_font, center_font.x.0, center_font.y.0, 1e-2, "M-20 marquee centered");

    // Alt + Marquee zooms out by min(w/vw, h/vh) = 0.25 -> zoom returns from 4.0 to 1.0
    pointer_press(&mut state, m_start, false, true, false, false);
    pointer_move(&mut state, m_end, false, true, false, false);
    pointer_release(&mut state, m_end, false, true, false, false);
    assert!((state.viewport.zoom - 1.0).abs() < 1e-4);
}

// ============================================================================
// TC-V020-06: Clipboard Copy, Cut, Paste, Closed Seam Wrap, (+10,+10) Collision & Splice
// Verified Scenarios: M-21, M-22, M-23, M-24
// ============================================================================
#[test]
fn test_spec003_clipboard_copy_cut_paste_closed_seam_collision_offset_and_splice() {
    let mut state = GlifoCanvasState::new(GlifoTheme::sonoma_default());
    let nodes = vec![
        Node::new(NodeId(1), 0.0, 0.0, NodeType::Corner),
        Node::new(NodeId(2), 100.0, 0.0, NodeType::Corner),
        Node::new(NodeId(3), 100.0, 100.0, NodeType::Corner),
        Node::new(NodeId(4), 50.0, 140.0, NodeType::OffCurve),
        Node::new(NodeId(5), 20.0, 140.0, NodeType::OffCurve),
        Node::new(NodeId(6), 0.0, 100.0, NodeType::Smooth),
    ];
    set_single_contour(&mut state, nodes, true);

    // M-23: Empty selection or OffCurve-only selection is a complete no-op on copy_selection
    assert!(!state.copy_selection());
    state.selected_nodes = vec![NodeId(4)];
    assert!(!state.copy_selection());
    assert!(state.clipboard_contours.is_empty());

    // M-21: Select all OnCurve nodes -> copy_selection clones full closed contour;
    // paste_clipboard detects identical contour at (0,0) and applies (+10, +10) collision offset!
    state.selected_nodes = vec![NodeId(1), NodeId(2), NodeId(3), NodeId(6)];
    assert!(state.copy_selection());
    assert_eq!(state.clipboard_contours.len(), 1);
    assert!(state.clipboard_contours[0].closed);

    assert!(state.paste_clipboard());
    assert_eq!(state.contours.len(), 2);
    assert!(state.is_contour_closed(1));
    assert_pt_near(state.contours[1][0].point, 10.0, 10.0, 1e-6, "M-21 (+10,+10) collision offset");
    assert!(state.contours[1].iter().all(|n| state.selected_nodes.contains(&n.id)));
    assert!(state.contours[0].iter().all(|n| !state.selected_nodes.contains(&n.id)));
    assert!(state.undo());

    // M-22: Partial chain selected across P0 seam (NodeId(6) at index 5 and NodeId(1) at index 0) ->
    // extracted as 1 single open contour [NodeId(6)(0,100)->NodeId(1)(0,0)] with trailing OffCurve handles stripped!
    state.selected_nodes = vec![NodeId(6), NodeId(1)];
    assert!(state.copy_selection());
    assert_eq!(state.clipboard_contours.len(), 1);
    assert!(!state.clipboard_contours[0].closed);
    assert_eq!(state.clipboard_contours[0].nodes.len(), 2);
    assert_pt_near(state.clipboard_contours[0].nodes[0].point, 0.0, 100.0, 1e-6, "M-22 seam start");
    assert_pt_near(state.clipboard_contours[0].nodes[1].point, 0.0, 0.0, 1e-6, "M-22 seam end");

    // Mode B (pasteInsertPath:): When 1 open contour is on clipboard and >= 2 contiguous OnCurve nodes
    // are selected on target contour, paste_clipboard splices the open contour into the target contour!
    let splice_clip = vec![
        Node::new(NodeId(90), 100.0, 0.0, NodeType::Corner),
        Node::new(NodeId(91), 140.0, 50.0, NodeType::Corner),
        Node::new(NodeId(92), 100.0, 100.0, NodeType::Corner),
    ];
    state.clipboard_contours = vec![grafia::ClipboardContour {
        nodes: splice_clip,
        closed: false,
    }];
    state.selected_nodes = vec![NodeId(2), NodeId(3)]; // (100,0) -> (100,100)
    assert!(state.paste_clipboard());
    assert!(
        state.contours[0]
            .iter()
            .any(|n| (n.point.x.0 - 140.0).abs() < 1e-6 && (n.point.y.0 - 50.0).abs() < 1e-6),
        "M-22 pasteInsertPath must splice (140, 50) into contour 0"
    );
    assert!(state.undo());

    // Mode B across closed-contour P0 seam: selecting contiguous [NodeId(6), NodeId(1)] across P0
    // splices the open clipboard contour across the P0 seam while preserving unselected NodeId(2) and NodeId(3)!
    let seam_clip = vec![
        Node::new(NodeId(93), 0.0, 100.0, NodeType::Corner),
        Node::new(NodeId(94), -40.0, 50.0, NodeType::Corner),
        Node::new(NodeId(95), 0.0, 0.0, NodeType::Corner),
    ];
    state.clipboard_contours = vec![grafia::ClipboardContour {
        nodes: seam_clip,
        closed: false,
    }];
    state.selected_nodes = vec![NodeId(6), NodeId(1)];
    assert!(state.paste_clipboard());
    assert_eq!(state.contours.len(), 1, "Mode B across P0 seam must splice into contour 0");
    assert!(
        state.contours[0].iter().any(|n| (n.point.x.0 - (-40.0)).abs() < 1e-6),
        "Mode B across P0 seam must insert (-40, 50)"
    );
    assert!(
        state.contours[0].iter().any(|n| (n.point.x.0 - 100.0).abs() < 1e-6 && (n.point.y.0 - 0.0).abs() < 1e-6),
        "Mode B across P0 seam must NOT wipe out unselected NodeId(2) at (100, 0)!"
    );
    assert!(state.undo());

    // Non-contiguous OnCurve selection [NodeId(1), NodeId(3)] MUST NOT trigger Mode B splice,
    // and instead falls back to Mode A (appending a 2nd contour)!
    state.selected_nodes = vec![NodeId(1), NodeId(3)];
    assert!(state.paste_clipboard());
    assert_eq!(state.contours.len(), 2, "Non-contiguous selection must fall back to Mode A paste");
    assert!(state.undo());

    // M-24: Cut (Cmd+X) copies selection and breaks closed contour open via cut_open_delete
    state.selected_nodes = vec![NodeId(2)];
    assert!(state.cut_selection());
    assert!(!state.is_contour_closed(0), "M-24: Cut must break closed contour open");
}

// ============================================================================
// TC-V020-07: The 7 Path Operations, Duplicate Warnings, TransformPanel & Section 6 Golden Masters
// Verified Scenarios: M-25, M-26, M-27, M-28, M-29, M-32, M-33 + Golden Masters 6.1A..6.5
// ============================================================================
#[test]
fn test_spec003_path_ops_reverse_make_first_correct_direction_extremes_tidy_align_connect_and_panel() {
    let mut state = GlifoCanvasState::new(GlifoTheme::sonoma_default());

    // ------------------------------------------------------------------------
    // 1. M-25: make_node_first on Closed vs. Open contour (P_last vs interior)
    // ------------------------------------------------------------------------
    let open_nodes = vec![
        Node::new(NodeId(1), 0.0, 0.0, NodeType::Corner),
        Node::new(NodeId(2), 50.0, 50.0, NodeType::Corner),
        Node::new(NodeId(3), 100.0, 0.0, NodeType::Corner),
    ];
    set_single_contour(&mut state, open_nodes, false);
    // Selecting interior node (NodeId(2)) on open contour -> no-op!
    state.selected_nodes = vec![NodeId(2)];
    assert!(!state.make_node_first());
    // Selecting P_last (NodeId(3)) on open contour -> reverses contour so NodeId(3) becomes P0!
    state.selected_nodes = vec![NodeId(3)];
    assert!(state.make_node_first());
    assert_eq!(state.contours[0][0].id, NodeId(3));

    // ------------------------------------------------------------------------
    // 2. Golden Master 6.1A: Tilted / Bowed Cubic (3 Extrema: Left X, Top Y, Right X)
    // ------------------------------------------------------------------------
    let gm_61a = vec![
        Node::new(NodeId(1), 20.0, 0.0, NodeType::Corner),
        Node::new(NodeId(2), -40.0, 60.0, NodeType::OffCurve),
        Node::new(NodeId(3), 140.0, 120.0, NodeType::OffCurve),
        Node::new(NodeId(4), 80.0, 0.0, NodeType::Corner),
    ];
    set_single_contour(&mut state, gm_61a, true);
    state.round_to_grid = false;
    state.grid_step_upm = 0.0;
    state.selected_nodes = vec![NodeId(1)]; // scope to segment so post-tidy doesn't alter unrelated
    assert!(state.add_extremes(false));
    assert_eq!(state.contours[0].len(), 13, "Golden Master 6.1A must produce 13 nodes");
    let expected_61a = [
        (20.000000, 0.000000, NodeType::Corner),
        (11.213203, 8.786797, NodeType::OffCurve),
        (7.573593, 17.573594, NodeType::OffCurve),
        (7.573593, 25.795050, NodeType::Smooth),
        (7.573594, 49.985812, NodeType::OffCurve),
        (39.084186, 69.282031, NodeType::OffCurve),
        (63.700905, 69.282032, NodeType::Smooth),
        (79.479876, 69.282033, NodeType::OffCurve),
        (92.426409, 61.353934, NodeType::OffCurve),
        (92.426407, 41.704946, NodeType::Smooth),
        (92.426406, 31.286792, NodeType::OffCurve),
        (88.786795, 17.573590, NodeType::OffCurve),
        (80.000000, 0.000000, NodeType::Corner),
    ];
    for (idx, (ex, ey, ent)) in expected_61a.iter().enumerate() {
        assert_pt_near(state.contours[0][idx].point, *ex, *ey, 1e-3, &format!("GM 6.1A[{idx}]"));
        assert_eq!(state.contours[0][idx].node_type, *ent, "GM 6.1A[{idx}] node_type");
    }

    // ------------------------------------------------------------------------
    // 3. Golden Master 6.1B: Near-Endpoint Extremum (d = 7.951 in [0.8, 30.0)):
    //    (a) Corner + force=false -> Suppressed (count=7)
    //    (b) Corner + force=true  -> Inserted + keeps Corner (count=10)
    //    (c) Smooth + force=false -> Inserted + heals old Smooth (count=7)
    // ------------------------------------------------------------------------
    let make_61b = |mid_type: NodeType| -> Vec<Node> {
        vec![
            Node::new(NodeId(1), 0.0, 0.0, NodeType::Corner),
            Node::new(NodeId(2), 0.0, 60.0, NodeType::OffCurve),
            Node::new(NodeId(3), 50.0, 103.0, NodeType::OffCurve),
            Node::new(NodeId(4), 100.0, 98.0, mid_type),
            Node::new(NodeId(5), 150.0, 93.0, NodeType::OffCurve),
            Node::new(NodeId(6), 200.0, 50.0, NodeType::OffCurve),
            Node::new(NodeId(7), 200.0, 0.0, NodeType::Corner),
        ]
    };
    // 6.1B(1): Corner + force=false -> Suppressed (count = 7)
    set_single_contour(&mut state, make_61b(NodeType::Corner), true);
    state.selected_nodes = vec![NodeId(1)];
    let _ = state.add_extremes(false);
    assert_eq!(state.contours[0].len(), 7, "GM 6.1B(1) Corner force=false must stay 7 nodes");
    assert_pt_near(state.contours[0][3].point, 100.0, 98.0, 1e-4, "GM 6.1B(1) unchanged");

    // 6.1B(2): Corner + force=true -> Inserts (92.058928, 98.395119) Smooth and keeps (100, 98) Corner (count = 10)
    set_single_contour(&mut state, make_61b(NodeType::Corner), true);
    state.selected_nodes = vec![NodeId(1)];
    assert!(state.add_extremes(true));
    assert_eq!(state.contours[0].len(), 10, "GM 6.1B(2) Corner force=true must have 10 nodes");
    assert_pt_near(state.contours[0][3].point, 92.058928, 98.395119, 1e-3, "GM 6.1B(2) extremum");
    assert_pt_near(state.contours[0][6].point, 100.0, 98.0, 1e-3, "GM 6.1B(2) preserved corner");

    // 6.1B(3): Smooth + force=false -> Inserts (92.058928, 98.395119) Smooth and heals (100, 98) (count = 7)
    set_single_contour(&mut state, make_61b(NodeType::Smooth), true);
    state.selected_nodes = vec![NodeId(1)];
    assert!(state.add_extremes(false));
    assert_eq!(state.contours[0].len(), 7, "GM 6.1B(3) Smooth force=false must heal to 7 nodes");
    assert_pt_near(state.contours[0][3].point, 92.058928, 98.395119, 1e-3, "GM 6.1B(3) absorbed smooth");
    assert_pt_near(state.contours[0][4].point, 144.612188, 98.395119, 0.5, "GM 6.1B(3) healed handle 1");
    assert_pt_near(state.contours[0][5].point, 200.000000, 52.625000, 0.5, "GM 6.1B(3) healed handle 2");

    // 6.1B(4): Closed-contour seam wrap-around: when the final cubic segment (ending at P0 Smooth)
    // has an extremum within 30 UPM of P0, force=false must absorb P0 across the closed seam!
    let seam_smooth_61b = vec![
        Node::new(NodeId(1), 100.0, 98.0, NodeType::Smooth),
        Node::new(NodeId(2), 150.0, 93.0, NodeType::OffCurve),
        Node::new(NodeId(3), 200.0, 50.0, NodeType::OffCurve),
        Node::new(NodeId(4), 200.0, 0.0, NodeType::Corner),
        Node::new(NodeId(5), 0.0, 0.0, NodeType::Corner),
        Node::new(NodeId(6), 0.0, 60.0, NodeType::OffCurve),
        Node::new(NodeId(7), 50.0, 103.0, NodeType::OffCurve),
    ];
    set_single_contour(&mut state, seam_smooth_61b, true);
    state.selected_nodes.clear();
    assert!(state.add_extremes(false));
    assert_eq!(state.contours[0].len(), 7, "Closed-contour P0 Smooth within 30 UPM must be absorbed into 7 nodes");

    // ------------------------------------------------------------------------
    // 4. Golden Master 6.2: add_inflection_points (Symmetric & Asymmetric S-Curves)
    // ------------------------------------------------------------------------
    let gm_62_sym = vec![
        Node::new(NodeId(1), 0.0, 0.0, NodeType::Corner),
        Node::new(NodeId(2), 0.0, 100.0, NodeType::OffCurve),
        Node::new(NodeId(3), 100.0, 0.0, NodeType::OffCurve),
        Node::new(NodeId(4), 100.0, 100.0, NodeType::Corner),
    ];
    set_single_contour(&mut state, gm_62_sym, false);
    assert!(state.add_inflection_points());
    assert_eq!(state.contours[0].len(), 7);
    assert_pt_near(state.contours[0][3].point, 50.0, 50.0, 1e-4, "GM 6.2(1) t=0.5 inflection");

    let gm_62_asym = vec![
        Node::new(NodeId(1), 0.0, 0.0, NodeType::Corner),
        Node::new(NodeId(2), 20.0, 140.0, NodeType::OffCurve),
        Node::new(NodeId(3), 80.0, -20.0, NodeType::OffCurve),
        Node::new(NodeId(4), 100.0, 100.0, NodeType::Corner),
    ];
    set_single_contour(&mut state, gm_62_asym, false);
    assert!(state.add_inflection_points());
    assert_eq!(state.contours[0].len(), 7);
    assert_pt_near(state.contours[0][1].point, 10.363877, 72.547140, 1e-3, "GM 6.2(2)[1]");
    assert_pt_near(state.contours[0][2].point, 31.468749, 64.536818, 1e-3, "GM 6.2(2)[2]");
    assert_pt_near(state.contours[0][3].point, 52.182781, 56.674839, 1e-3, "GM 6.2(2)[3]");
    assert_pt_near(state.contours[0][4].point, 71.442268, 49.364930, 1e-3, "GM 6.2(2)[4]");
    assert_pt_near(state.contours[0][5].point, 90.363877, 42.183263, 1e-3, "GM 6.2(2)[5]");

    // ------------------------------------------------------------------------
    // 5. Golden Master 6.3: tidy_up_paths & Duplicate-Node Warning Circles
    // ------------------------------------------------------------------------
    state.grid_step_upm = 1.0;
    let c0_63 = vec![
        Node::new(NodeId(1), 0.0, 0.0, NodeType::Corner),
        Node::new(NodeId(2), 0.0, 0.0, NodeType::Corner), // duplicate OnCurve -> Red warning circle!
        Node::new(NodeId(3), 33.0, 0.0, NodeType::OffCurve),
        Node::new(NodeId(4), 67.0, 0.0, NodeType::OffCurve),
        Node::new(NodeId(5), 100.0, 0.0, NodeType::Corner),
        Node::new(NodeId(6), 100.0, 100.0, NodeType::Corner),
        Node::new(NodeId(7), 100.0, 150.0, NodeType::OffCurve),
        Node::new(NodeId(8), 50.0, 200.0, NodeType::OffCurve),
        Node::new(NodeId(9), 0.0, 200.0, NodeType::Smooth),
    ];
    let c1_63 = vec![Node::new(NodeId(10), 300.0, 300.0, NodeType::Corner)];
    state.contours = vec![c0_63.clone(), c1_63.clone()];
    let mut gc0 = GlyphContour::new(grafia::ContourId(0), c0_63);
    gc0.closed = true;
    let mut gc1 = GlyphContour::new(grafia::ContourId(1), c1_63);
    gc1.closed = false;
    state.glyph.contours = vec![gc0, gc1];
    state.selected_nodes.clear();
    state.sync_paths_from_contours();

    // Verify duplicate warning circle detection before tidy_up_paths
    let warnings = state.duplicate_warning_nodes();
    assert!(
        warnings
            .iter()
            .any(|(_, _, kind)| *kind == DuplicateWarningKind::CoincidentOnCurve),
        "Must detect red CoincidentOnCurve warning on duplicate (0,0)"
    );

    assert!(state.tidy_up_paths());
    assert_eq!(state.contours.len(), 1, "GM 6.3 degenerate contour 1 must be removed");
    assert_eq!(state.contours[0].len(), 6, "GM 6.3 contour 0 must have 6 nodes");
    let expected_63 = [
        (0.0, 0.0, NodeType::Corner),
        (100.0, 0.0, NodeType::Corner),
        (100.0, 100.0, NodeType::Smooth),
        (100.0, 150.0, NodeType::OffCurve),
        (50.0, 200.0, NodeType::OffCurve),
        (0.0, 200.0, NodeType::Corner),
    ];
    for (idx, (ex, ey, ent)) in expected_63.iter().enumerate() {
        assert_pt_near(state.contours[0][idx].point, *ex, *ey, 1e-4, &format!("GM 6.3[{idx}]"));
        assert_eq!(state.contours[0][idx].node_type, *ent, "GM 6.3[{idx}] node_type");
    }

    // SPEC-003 §2.6.5.5: tidy_up_paths (checkConnections) when promoting Corner to Smooth with
    // non-zero perpendicular deviation (d_perp = 1.0 < 0.25 * sqrt(50) = 1.767) MUST snap the
    // adjacent OffCurve handle collinear with the incoming line tangent (and round to grid)!
    let c_collinear_snap = vec![
        Node::new(NodeId(1), 0.0, 0.0, NodeType::Corner),
        Node::new(NodeId(2), 100.0, 0.0, NodeType::Corner),
        Node::new(NodeId(3), 101.0, 100.0, NodeType::Corner), // slightly off-vertical -> promotes to Smooth
        Node::new(NodeId(4), 101.0, 150.0, NodeType::OffCurve), // d_perp < 1.767 -> must snap collinear with (100,0)->(101,100)!
        Node::new(NodeId(5), 50.0, 200.0, NodeType::OffCurve),
        Node::new(NodeId(6), 0.0, 200.0, NodeType::Corner),
    ];
    state.round_to_grid = false;
    state.grid_step_upm = 0.0;
    set_single_contour(&mut state, c_collinear_snap, true);
    assert!(state.tidy_up_paths());
    assert_eq!(state.contours[0][2].node_type, NodeType::Smooth);
    assert!(
        (state.contours[0][3].point.x.0 - 101.5).abs() < 0.1,
        "SPEC-003 §2.6.5.5: tidy_up_paths must snap OffCurve handle collinear (expected x ~ 101.5, got {})",
        state.contours[0][3].point.x.0
    );

    // ------------------------------------------------------------------------
    // 6. Golden Master 6.4: correct_path_direction (3-Level Nested + Open Unchanged)
    // ------------------------------------------------------------------------
    let p0_outer_cw = vec![
        Node::new(NodeId(1), 0.0, 0.0, NodeType::Corner),
        Node::new(NodeId(2), 0.0, 500.0, NodeType::Corner),
        Node::new(NodeId(3), 500.0, 500.0, NodeType::Corner),
        Node::new(NodeId(4), 500.0, 0.0, NodeType::Corner),
    ];
    let p1_counter_ccw = vec![
        Node::new(NodeId(5), 100.0, 100.0, NodeType::Corner),
        Node::new(NodeId(6), 400.0, 100.0, NodeType::Corner),
        Node::new(NodeId(7), 400.0, 400.0, NodeType::Corner),
        Node::new(NodeId(8), 100.0, 400.0, NodeType::Corner),
    ];
    let p2_island_cw = vec![
        Node::new(NodeId(9), 200.0, 200.0, NodeType::Corner),
        Node::new(NodeId(10), 200.0, 300.0, NodeType::Corner),
        Node::new(NodeId(11), 300.0, 300.0, NodeType::Corner),
        Node::new(NodeId(12), 300.0, 200.0, NodeType::Corner),
    ];
    let p3_open = vec![
        Node::new(NodeId(13), 0.0, 600.0, NodeType::Corner),
        Node::new(NodeId(14), 500.0, 600.0, NodeType::Corner),
    ];
    state.contours = vec![
        p0_outer_cw.clone(),
        p1_counter_ccw.clone(),
        p2_island_cw.clone(),
        p3_open.clone(),
    ];
    state.glyph.contours = vec![
        GlyphContour::new(grafia::ContourId(0), p0_outer_cw),
        GlyphContour::new(grafia::ContourId(1), p1_counter_ccw),
        GlyphContour::new(grafia::ContourId(2), p2_island_cw),
        {
            let mut gc = GlyphContour::new(grafia::ContourId(3), p3_open);
            gc.closed = false;
            gc
        },
    ];
    state.sync_paths_from_contours();
    assert!(state.correct_path_direction());
    // Outer (depth 0) -> CCW starting at (0,0): [(0,0), (500,0), (500,500), (0,500)]
    assert!(contour_signed_area(&state.contours[0]) > 0.0, "GM 6.4 outer must be CCW");
    assert_pt_near(state.contours[0][0].point, 0.0, 0.0, 1e-4, "GM 6.4 outer P0 findMinNode");
    // Counter (depth 1) -> CW starting at (100,100): [(100,100), (100,400), (400,400), (400,100)]
    assert!(contour_signed_area(&state.contours[1]) < 0.0, "GM 6.4 counter must be CW");
    assert_pt_near(state.contours[1][0].point, 100.0, 100.0, 1e-4, "GM 6.4 counter P0 findMinNode");
    // Island (depth 2) -> CCW starting at (200,200)
    assert!(contour_signed_area(&state.contours[2]) > 0.0, "GM 6.4 island must be CCW");
    assert_pt_near(state.contours[2][0].point, 200.0, 200.0, 1e-4, "GM 6.4 island P0 findMinNode");
    // Open path 3 -> completely untouched
    assert!(!state.is_contour_closed(3));
    assert_pt_near(state.contours[3][0].point, 0.0, 600.0, 1e-4, "GM 6.4 open untouched");

    // SPEC-003 §2.6.3 boundary-touching edge case: inner closed contour whose first OnCurve node P0=(0, 250)
    // touches the left vertical edge x=0 of outer [0..500]x[0..500] while its interior lies strictly inside:
    // MUST still be detected as nesting depth 1 (CW, signed_area < 0)!
    let outer_box = vec![
        Node::new(NodeId(1), 0.0, 0.0, NodeType::Corner),
        Node::new(NodeId(2), 500.0, 0.0, NodeType::Corner),
        Node::new(NodeId(3), 500.0, 500.0, NodeType::Corner),
        Node::new(NodeId(4), 0.0, 500.0, NodeType::Corner),
    ];
    let inner_touching = vec![
        Node::new(NodeId(5), 0.0, 250.0, NodeType::Corner), // touches outer left edge x=0!
        Node::new(NodeId(6), 300.0, 100.0, NodeType::Corner),
        Node::new(NodeId(7), 300.0, 400.0, NodeType::Corner),
    ];
    state.contours = vec![outer_box.clone(), inner_touching.clone()];
    state.glyph.contours = vec![
        GlyphContour::new(grafia::ContourId(0), outer_box),
        GlyphContour::new(grafia::ContourId(1), inner_touching),
    ];
    state.sync_paths_from_contours();
    let _ = state.correct_path_direction();
    assert!(
        contour_signed_area(&state.contours[1]) < 0.0,
        "Inner contour whose P0 touches outer boundary must still get depth=1 (CW)"
    );

    // ------------------------------------------------------------------------
    // 7. Golden Master 6.5: align_selection (Branch A 2-node handle snap & Branch B 0..8)
    // ------------------------------------------------------------------------
    let align_nodes = vec![
        Node::new(NodeId(1), 100.0, 200.0, NodeType::Smooth),
        Node::new(NodeId(2), 110.0, 260.0, NodeType::OffCurve),
        Node::new(NodeId(3), 150.0, 260.0, NodeType::OffCurve),
        Node::new(NodeId(4), 150.0, 215.0, NodeType::Corner),
        Node::new(NodeId(5), 200.0, 230.0, NodeType::Corner),
    ];
    set_single_contour(&mut state, align_nodes, false);
    // GM 6.5(1): 1 OnCurve (100,200) + 1 adjacent OffCurve (110,260) -> OffCurve snaps X to 100.0
    state.selected_nodes = vec![NodeId(1), NodeId(2)];
    assert!(state.align_selection());
    assert_pt_near(state.find_node(NodeId(1)).unwrap().point, 100.0, 200.0, 1e-4, "GM 6.5(1) anchor fixed");
    assert_pt_near(state.find_node(NodeId(2)).unwrap().point, 100.0, 260.0, 1e-4, "GM 6.5(1) handle X snapped");

    // GM 6.5(2): 3 OnCurve nodes [(100,200), (150,215), (200,230)] across Bottom (0), Center (4), Top (8)
    for (anchor, expected_y) in [
        (TransformGridAnchor::BottomLeft, 200.0),
        (TransformGridAnchor::Center, 215.0),
        (TransformGridAnchor::TopRight, 230.0),
    ] {
        let three_nodes = vec![
            Node::new(NodeId(1), 100.0, 200.0, NodeType::Corner),
            Node::new(NodeId(2), 150.0, 215.0, NodeType::Corner),
            Node::new(NodeId(3), 200.0, 230.0, NodeType::Corner),
        ];
        set_single_contour(&mut state, three_nodes, false);
        state.set_transform_grid_anchor(anchor);
        state.selected_nodes = vec![NodeId(1), NodeId(2), NodeId(3)];
        assert!(state.align_selection());
        for id in [NodeId(1), NodeId(2), NodeId(3)] {
            assert!((state.find_node(id).unwrap().point.y.0 - expected_y).abs() < 1e-4);
        }
    }

    // SPEC-003 §2.6.6.2 Branch B (_alignOtherNodes:): when a Smooth anchor and its incoming neighbor
    // are aligned horizontally (e.g. y -> 200.0), its unselected outgoing OffCurve handle MUST be
    // rotated collinear (y -> 200.0), NOT merely translated vertically!
    let branch_b_smooth = vec![
        Node::new(NodeId(1), 50.0, 190.0, NodeType::Corner),
        Node::new(NodeId(2), 100.0, 200.0, NodeType::Smooth),
        Node::new(NodeId(3), 150.0, 210.0, NodeType::OffCurve), // unselected outgoing handle of Smooth NodeId(2)
        Node::new(NodeId(4), 180.0, 210.0, NodeType::OffCurve),
        Node::new(NodeId(5), 200.0, 210.0, NodeType::Corner),
    ];
    set_single_contour(&mut state, branch_b_smooth, false);
    state.set_transform_grid_anchor(TransformGridAnchor::Center); // aligns y of selected [1, 2, 5] to y = 200.0
    state.selected_nodes = vec![NodeId(1), NodeId(2), NodeId(5)];
    assert!(state.align_selection());
    // Since NodeId(1) at (50, 200) and Smooth NodeId(2) at (100, 200) are now horizontal,
    // unselected OffCurve NodeId(3) must be rotated collinear with (50,200)->(100,200) so its y == 200.0!
    assert!(
        (state.find_node(NodeId(3)).unwrap().point.y.0 - 200.0).abs() < 1e-3,
        "SPEC-003 §2.6.6.2 Branch B must rotate unselected opposite OffCurve handle collinear (got y={})",
        state.find_node(NodeId(3)).unwrap().point.y.0
    );

    // ------------------------------------------------------------------------
    // 8. M-33: connect_selected_open_endpoints (All 4 Cases A / B / C / D)
    // ------------------------------------------------------------------------
    // Case A & B: Same open contour (< 1.0 UPM and >= 1.0 UPM) -> sets closed = true
    let open_same = vec![
        Node::new(NodeId(1), 0.0, 0.0, NodeType::Corner),
        Node::new(NodeId(2), 50.0, 80.0, NodeType::Corner),
        Node::new(NodeId(3), 100.0, 0.0, NodeType::Corner),
    ];
    set_single_contour(&mut state, open_same, false);
    state.selected_nodes = vec![NodeId(1), NodeId(3)];
    assert!(state.connect_selected_open_endpoints());
    assert!(state.is_contour_closed(0));

    // Case C: Two different open contours with endpoints < 1.0 UPM -> merges duplicate endpoint into 1 open contour
    let ca = vec![
        Node::new(NodeId(1), 0.0, 0.0, NodeType::Corner),
        Node::new(NodeId(2), 100.0, 0.0, NodeType::Corner),
    ];
    let cb_near = vec![
        Node::new(NodeId(3), 100.4, 0.2, NodeType::Corner),
        Node::new(NodeId(4), 200.0, 0.0, NodeType::Corner),
    ];
    state.contours = vec![ca.clone(), cb_near.clone()];
    state.glyph.contours = vec![
        {
            let mut g = GlyphContour::new(grafia::ContourId(0), ca.clone());
            g.closed = false;
            g
        },
        {
            let mut g = GlyphContour::new(grafia::ContourId(1), cb_near);
            g.closed = false;
            g
        },
    ];
    state.sync_paths_from_contours();
    state.selected_nodes = vec![NodeId(2), NodeId(3)];
    assert!(state.connect_selected_open_endpoints());
    assert_eq!(state.contours.len(), 1);
    assert!(!state.is_contour_closed(0));
    assert_eq!(state.contours[0].len(), 3, "Case C (< 1.0 UPM) must weld into 3 nodes");

    // Case D: Two different open contours with endpoints >= 1.0 UPM -> bridges with straight line into 1 open contour (4 nodes)
    let cb_far = vec![
        Node::new(NodeId(3), 120.0, 0.0, NodeType::Corner),
        Node::new(NodeId(4), 200.0, 0.0, NodeType::Corner),
    ];
    state.contours = vec![ca.clone(), cb_far.clone()];
    state.glyph.contours = vec![
        {
            let mut g = GlyphContour::new(grafia::ContourId(0), ca);
            g.closed = false;
            g
        },
        {
            let mut g = GlyphContour::new(grafia::ContourId(1), cb_far);
            g.closed = false;
            g
        },
    ];
    state.sync_paths_from_contours();
    state.selected_nodes = vec![NodeId(2), NodeId(3)];
    assert!(state.connect_selected_open_endpoints());
    assert_eq!(state.contours.len(), 1);
    assert!(!state.is_contour_closed(0));
    assert_eq!(state.contours[0].len(), 4, "Case D (>= 1.0 UPM) must bridge keeping 4 nodes");

    // ------------------------------------------------------------------------
    // 9. M-32: TransformPanel Programmatic Operations & MetricsOrigin (0..4)
    // ------------------------------------------------------------------------
    state.glyph.width = 600.0;
    state.glyph.height = 700.0;
    state.italic_angle_deg = 0.0;
    for (mode, expected_y) in [
        (MetricsOrigin::Baseline, 0.0),
        (MetricsOrigin::Ascender, 800.0),
        (MetricsOrigin::CapHeight, 700.0),
        (MetricsOrigin::HalfXHeight, 250.0),
        (MetricsOrigin::XHeight, 500.0),
    ] {
        state.set_metrics_origin(mode);
        assert_pt_near(state.resolve_transform_origin(), 300.0, expected_y, 1e-4, "MetricsOrigin");
    }

    // Whole-layer transform when selected_nodes is empty vs selection transform when non-empty
    let box_nodes = vec![
        Node::new(NodeId(1), 0.0, 0.0, NodeType::Corner),
        Node::new(NodeId(2), 100.0, 0.0, NodeType::Corner),
        Node::new(NodeId(3), 100.0, 100.0, NodeType::Corner),
        Node::new(NodeId(4), 0.0, 100.0, NodeType::Corner),
    ];
    set_single_contour(&mut state, box_nodes, true);
    state.set_transform_grid_anchor(TransformGridAnchor::Center); // pivot = (50, 50)
    state.selected_nodes.clear();

    assert!(state.flip_horizontal());
    assert_eq!(state.contours[0][0].id, NodeId(1), "flip_horizontal must not reverse array order");
    assert_pt_near(state.contours[0][0].point, 100.0, 0.0, 1e-4, "flip_horizontal P0");
    assert!(state.undo());

    assert!(state.flip_vertical());
    assert_pt_near(state.contours[0][0].point, 0.0, 100.0, 1e-4, "flip_vertical P0");
    assert!(state.undo());

    assert!(state.scale_by_factor(2.0, 1.5));
    assert_pt_near(state.contours[0][2].point, 150.0, 125.0, 1e-4, "scale_by_factor P2");
    assert!(state.undo());

    assert!(state.rotate_by_degrees(90.0));
    assert_pt_near(state.contours[0][1].point, 100.0, 100.0, 1e-4, "rotate_by_degrees 90 P1");
    assert!(state.undo());

    assert!(state.slant_by_degrees(45.0, true));
    assert_pt_near(state.contours[0][2].point, 150.0, 100.0, 1e-4, "slant_by_degrees 45 horiz P2");
    assert!(state.undo());
}

// ============================================================================
// TC-V020-08: Pass-5 Vector Cursors, Modifier-Aware BBox Hover, Slanted Marquee Parallelogram
// Rendering, Cmd/Ctrl+Space Zoom Override Priority, and ScaleTool Per-Axis > 5.0 UPM Guard
// Verified Scenarios: M-34, M-35, M-36, M-37
// ============================================================================
#[test]
fn test_spec003_vector_cursors_slanted_marquee_render_cmd_space_zoom_and_scale_axis_guard() {
    use grafia::adapters::headless::pixmap_to_rgba_image;
    use grafia::tokens::GlifoCursorKind;

    let mut state = GlifoCanvasState::new(GlifoTheme::sonoma_default());
    state.viewport.zoom = 1.0;
    state.viewport.pan_offset = ScreenPoint::new(200.0, 400.0);
    state.show_bounding_box = true;

    let nodes = vec![
        Node::new(NodeId(1), 0.0, 0.0, NodeType::Corner),
        Node::new(NodeId(2), 100.0, 0.0, NodeType::Corner),
        Node::new(NodeId(3), 100.0, 200.0, NodeType::Corner),
        Node::new(NodeId(4), 0.0, 200.0, NodeType::Corner),
    ];
    set_single_contour(&mut state, nodes, true);
    state.selected_nodes = vec![NodeId(1), NodeId(2), NodeId(3), NodeId(4)];
    let rect = default_canvas_rect();

    // 1. M-34: Modifier-aware desired_cursor_kind on BBox Zone 3 (k = 5 Middle-Right):
    //    Without Ctrl+Alt -> Rotate; With Ctrl+Alt -> BBoxSlant!
    let bounds = state.selection_bounds().unwrap();
    let box_5 = bounds.box_screen_point(5, &state.viewport);
    let knob_5 = bounds.knob_screen_center(5, &state.viewport);
    let zone3_k5 = ScreenPoint::new(
        box_5.x.0 + 1.85 * (knob_5.x.0 - box_5.x.0),
        box_5.y.0 + 1.85 * (knob_5.y.0 - box_5.y.0),
    );
    pointer_move(&mut state, zone3_k5, false, false, false, false);
    assert_eq!(
        state.desired_cursor_kind(rect, Some(zone3_k5)),
        Some(GlifoCursorKind::Rotate),
        "M-34: Zone 3 on knob 5 without Ctrl+Alt must be Rotate cursor"
    );
    pointer_move(&mut state, zone3_k5, false, true, false, true);
    assert_eq!(
        state.desired_cursor_kind(rect, Some(zone3_k5)),
        Some(GlifoCursorKind::BBoxSlant),
        "M-34: Zone 3 on knob 5 with Ctrl+Alt must be BBoxSlant cursor"
    );

    // 2. M-34: ZoomTool cursor switches from ZoomIn (+) to ZoomOut (-) when Alt is held!
    state.activate_tool(CanvasTool::Zoom);
    let hover_pt = ScreenPoint::new(100.0, 100.0);
    pointer_move(&mut state, hover_pt, false, false, false, false);
    assert_eq!(
        state.desired_cursor_kind(rect, Some(hover_pt)),
        Some(GlifoCursorKind::ZoomIn),
        "M-34: ZoomTool without Alt must report ZoomIn cursor"
    );
    pointer_move(&mut state, hover_pt, false, true, false, false);
    assert_eq!(
        state.desired_cursor_kind(rect, Some(hover_pt)),
        Some(GlifoCursorKind::ZoomOut),
        "M-34: ZoomTool with Alt held must report ZoomOut cursor"
    );

    // 3. M-34: Pass-5 rendered vector cursors for SelectArrow, ZoomIn, ZoomOut, Rotate, Scale,
    //    Lasso, and BBoxSlant must each produce distinct pixel rasters (never falling back to SelectArrow)!
    let render_cursor_patch = |tool: CanvasTool, alt: bool, ctrl: bool, pt: ScreenPoint| -> Vec<u8> {
        let s = state.snapshot();
        let mut st = GlifoCanvasState::new(GlifoTheme::sonoma_default());
        st.restore_snapshot(&s);
        st.viewport = state.viewport;
        st.show_bounding_box = true;
        st.activate_tool(tool);
        pointer_move(&mut st, pt, false, alt, false, ctrl);
        st.cursor_screen_pos = Some(pt);
        pixmap_to_rgba_image(rasterize_canvas_subregion(&st, &st.theme, 240, 200)).into_raw()
    };
    let cursor_pt = ScreenPoint::new(80.0, 80.0);
    let img_select = render_cursor_patch(CanvasTool::Select, false, false, cursor_pt);
    let img_zoom_in = render_cursor_patch(CanvasTool::Zoom, false, false, cursor_pt);
    let img_zoom_out = render_cursor_patch(CanvasTool::Zoom, true, false, cursor_pt);
    let img_rotate = render_cursor_patch(CanvasTool::Rotate, false, false, cursor_pt);
    let img_scale = render_cursor_patch(CanvasTool::Scale, false, false, cursor_pt);
    let img_lasso = render_cursor_patch(CanvasTool::Lasso, false, false, cursor_pt);
    assert_ne!(img_zoom_in, img_select, "M-34: ZoomIn vector cursor must not fall back to SelectArrow");
    assert_ne!(img_zoom_out, img_select, "M-34: ZoomOut vector cursor must not fall back to SelectArrow");
    assert_ne!(img_zoom_in, img_zoom_out, "M-34: ZoomIn (+) and ZoomOut (-) cursors must differ visually");
    assert_ne!(img_rotate, img_select, "M-34: Rotate vector cursor must not fall back to SelectArrow");
    assert_ne!(img_scale, img_select, "M-34: Scale vector cursor must not fall back to SelectArrow");
    assert_ne!(img_lasso, img_select, "M-34: Lasso vector cursor must not fall back to SelectArrow");
    assert_ne!(img_rotate, img_scale, "M-34: Rotate and Scale vector cursors must differ visually");

    // 4. M-35: Slanted Marquee Parallelogram Rendering in headless & egui painter:
    //    Dragging a marquee with italic slant / Ctrl (selection_angle_deg != 0) must render a slanted parallelogram
    //    whose pixels differ from an axis-aligned marquee over the same start/current screen points!
    let mut s_rect_m = GlifoCanvasState::new(GlifoTheme::sonoma_default());
    let m_a = ScreenPoint::new(60.0, 160.0);
    let m_b = ScreenPoint::new(160.0, 40.0);
    pointer_press(&mut s_rect_m, m_a, false, false, false, false);
    pointer_move(&mut s_rect_m, m_b, false, false, false, false);
    let img_rect_marquee = pixmap_to_rgba_image(rasterize_canvas_subregion(&s_rect_m, &s_rect_m.theme, 260, 220)).into_raw();

    let mut s_slant_m = GlifoCanvasState::new(GlifoTheme::sonoma_default());
    pointer_press(&mut s_slant_m, m_a, false, false, false, true);
    pointer_move(&mut s_slant_m, m_b, false, false, false, true);
    assert!(s_slant_m.show_bounding_box_slanted && s_slant_m.selection_angle_deg.abs() > 5.0);
    let img_slant_marquee = pixmap_to_rgba_image(rasterize_canvas_subregion(&s_slant_m, &s_slant_m.theme, 260, 220)).into_raw();
    assert_ne!(
        img_slant_marquee, img_rect_marquee,
        "M-35: Slanted marquee must render a slanted 4-vertex parallelogram, not an axis-aligned rect"
    );

    // 5. M-36: Temporary Cmd/Ctrl + Space and Cmd/Ctrl + Alt + Space ZoomTool Override:
    //    While in SelectTool, pressing Cmd+Space and clicking (with Cmd still held!) MUST zoom in
    //    (1.0 -> 1.5), and clicking with Cmd+Alt held MUST zoom out (1.5 -> 1.0), then releasing Space
    //    restores SelectTool!
    state.activate_tool(CanvasTool::Select);
    state.viewport.zoom = 1.0;
    press_key_v020(&mut state, Key::Space, false, false, true, false);
    assert_eq!(state.active_tool, CanvasTool::Zoom, "M-36: Cmd+Space must activate temporary ZoomTool");
    let z_pt = ScreenPoint::new(250.0, 250.0);
    pointer_press(&mut state, z_pt, false, false, true, false);
    pointer_release(&mut state, z_pt, false, false, true, false);
    assert!(
        (state.viewport.zoom - 1.5).abs() < 1e-5,
        "M-36: Clicking while holding Cmd+Space must zoom in to 1.5 (not be hijacked by Cmd-Select!), got {}",
        state.viewport.zoom
    );
    pointer_press(&mut state, z_pt, false, true, true, false);
    pointer_release(&mut state, z_pt, false, true, true, false);
    assert!(
        (state.viewport.zoom - 1.0).abs() < 1e-5,
        "M-36: Clicking while holding Cmd+Alt+Space must zoom out to 1.0, got {}",
        state.viewport.zoom
    );
    let _ = run_dispatch(
        &mut state,
        vec![Event::Key {
            key: Key::Space,
            physical_key: Some(Key::Space),
            pressed: false,
            repeat: false,
            modifiers: EguiModifiers::NONE,
        }],
        false,
        false,
        false,
        false,
    );
    assert_eq!(state.active_tool, CanvasTool::Select, "M-36: Releasing Space must restore SelectTool");

    // 6. M-37: ScaleTool ('S') Per-Axis Minimum Pivot Distance Guard (|Ax - Ox| > 5.0 && |Ay - Oy| > 5.0):
    //    When transform_origin is (50, 50) and drag starts at (53, 120) (hypot = 70.06 > 5.0, but |Ax - Ox| = 3.0 <= 5.0),
    //    dragging in ScaleTool MUST NOT set active = true!
    state.activate_tool(CanvasTool::Scale);
    state.transform_origin = FontPoint::new(50.0, 50.0);
    state.transform_origin_custom = true;
    let near_x_start = state.viewport.to_screen(FontPoint::new(53.0, 120.0));
    let near_x_end = state.viewport.to_screen(FontPoint::new(150.0, 200.0));
    pointer_press(&mut state, near_x_start, false, false, false, false);
    pointer_move(&mut state, near_x_end, false, false, false, false);
    assert!(
        matches!(
            state.v020_drag,
            grafia::V020DragState::TransformPendingOrDragging { active: false, .. }
        ),
        "M-37: ScaleTool drag starting within <= 5.0 UPM on X axis must keep active == false"
    );
    pointer_release(&mut state, near_x_end, false, false, false, false);
}

// ============================================================================
// TC-V020-09: Visual Verification Renders (frames 37..40 @ 1223x811 + 2x2 Strip)
// ============================================================================
#[test]
fn test_spec003_v020_visual_verification_renders_and_strip() {
    use grafia::adapters::headless::pixmap_to_rgba_image;
    use grafia::V020DragState;
    use image::{Rgba, RgbaImage};
    use std::fs;
    use std::path::PathBuf;

    const W: u32 = 1223;
    const H: u32 = 811;
    const QW: u32 = 610;
    const QH: u32 = 404;

    let target_dirs = [
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("target").join("visual_verification"),
        PathBuf::from(r"C:\Users\jhove\.gemini\antigravity-cli\brain\fccb9142-9217-4702-a2ca-95303b541ee0"),
    ];
    for dir in &target_dirs {
        fs::create_dir_all(dir).expect("Must create visual verification output directory");
    }

    fn glyph_5x7(ch: char) -> [u8; 7] {
        match ch.to_ascii_uppercase() {
            'A' => [0b01110, 0b10001, 0b10001, 0b11111, 0b10001, 0b10001, 0b10001],
            'B' => [0b11110, 0b10001, 0b10001, 0b11110, 0b10001, 0b10001, 0b11110],
            'C' => [0b01110, 0b10001, 0b10000, 0b10000, 0b10000, 0b10001, 0b01110],
            'D' => [0b11100, 0b10010, 0b10001, 0b10001, 0b10001, 0b10010, 0b11100],
            'E' => [0b11111, 0b10000, 0b10000, 0b11110, 0b10000, 0b10000, 0b11111],
            'F' => [0b11111, 0b10000, 0b10000, 0b11110, 0b10000, 0b10000, 0b10000],
            'G' => [0b01110, 0b10001, 0b10000, 0b10111, 0b10001, 0b10001, 0b01110],
            'H' => [0b10001, 0b10001, 0b10001, 0b11111, 0b10001, 0b10001, 0b10001],
            'I' => [0b01110, 0b00100, 0b00100, 0b00100, 0b00100, 0b00100, 0b01110],
            'J' => [0b00111, 0b00010, 0b00010, 0b00010, 0b10010, 0b10010, 0b01100],
            'K' => [0b10001, 0b10010, 0b10100, 0b11000, 0b10100, 0b10010, 0b10001],
            'L' => [0b10000, 0b10000, 0b10000, 0b10000, 0b10000, 0b10000, 0b11111],
            'M' => [0b10001, 0b11011, 0b10101, 0b10101, 0b10001, 0b10001, 0b10001],
            'N' => [0b10001, 0b11001, 0b10101, 0b10011, 0b10001, 0b10001, 0b10001],
            'O' => [0b01110, 0b10001, 0b10001, 0b10001, 0b10001, 0b10001, 0b01110],
            'P' => [0b11110, 0b10001, 0b10001, 0b11110, 0b10000, 0b10000, 0b10000],
            'Q' => [0b01110, 0b10001, 0b10001, 0b10001, 0b10101, 0b10010, 0b01101],
            'R' => [0b11110, 0b10001, 0b10001, 0b11110, 0b10100, 0b10010, 0b10001],
            'S' => [0b01111, 0b10000, 0b10000, 0b01110, 0b00001, 0b00001, 0b11110],
            'T' => [0b11111, 0b00100, 0b00100, 0b00100, 0b00100, 0b00100, 0b00100],
            'U' => [0b10001, 0b10001, 0b10001, 0b10001, 0b10001, 0b10001, 0b01110],
            'V' => [0b10001, 0b10001, 0b10001, 0b10001, 0b10001, 0b01010, 0b00100],
            'W' => [0b10001, 0b10001, 0b10001, 0b10101, 0b10101, 0b11011, 0b10001],
            'X' => [0b10001, 0b10001, 0b01010, 0b00100, 0b01010, 0b10001, 0b10001],
            'Y' => [0b10001, 0b10001, 0b01010, 0b00100, 0b00100, 0b00100, 0b00100],
            'Z' => [0b11111, 0b00001, 0b00010, 0b00100, 0b01000, 0b10000, 0b11111],
            '0' => [0b01110, 0b10001, 0b10011, 0b10101, 0b11001, 0b10001, 0b01110],
            '1' => [0b00100, 0b01100, 0b00100, 0b00100, 0b00100, 0b00100, 0b01110],
            '2' => [0b01110, 0b10001, 0b00001, 0b00110, 0b01000, 0b10000, 0b11111],
            '3' => [0b11110, 0b00001, 0b00001, 0b01110, 0b00001, 0b00001, 0b11110],
            '4' => [0b10010, 0b10010, 0b10010, 0b11111, 0b00010, 0b00010, 0b00010],
            '5' => [0b11111, 0b10000, 0b10000, 0b11110, 0b00001, 0b00001, 0b11110],
            '6' => [0b01110, 0b10000, 0b10000, 0b11110, 0b10001, 0b10001, 0b01110],
            '7' => [0b11111, 0b00001, 0b00010, 0b00100, 0b01000, 0b01000, 0b01000],
            '8' => [0b01110, 0b10001, 0b10001, 0b01110, 0b10001, 0b10001, 0b01110],
            '9' => [0b01110, 0b10001, 0b10001, 0b01111, 0b00001, 0b00001, 0b01110],
            '[' => [0b01110, 0b01000, 0b01000, 0b01000, 0b01000, 0b01000, 0b01110],
            ']' => [0b01110, 0b00010, 0b00010, 0b00010, 0b00010, 0b00010, 0b01110],
            '(' => [0b00100, 0b01000, 0b01000, 0b01000, 0b01000, 0b01000, 0b00100],
            ')' => [0b00100, 0b00010, 0b00010, 0b00010, 0b00010, 0b00010, 0b00100],
            '/' => [0b00001, 0b00010, 0b00100, 0b00100, 0b01000, 0b10000, 0b10000],
            '+' => [0b00000, 0b00100, 0b00100, 0b11111, 0b00100, 0b00100, 0b00000],
            '-' => [0b00000, 0b00000, 0b00000, 0b11111, 0b00000, 0b00000, 0b00000],
            '=' => [0b00000, 0b00000, 0b11111, 0b00000, 0b11111, 0b00000, 0b00000],
            ':' => [0b00000, 0b00100, 0b00100, 0b00000, 0b00100, 0b00100, 0b00000],
            '.' => [0b00000, 0b00000, 0b00000, 0b00000, 0b00000, 0b00110, 0b00110],
            '&' => [0b01100, 0b10010, 0b10100, 0b01000, 0b10101, 0b10010, 0b01101],
            _ => [0; 7],
        }
    }

    fn draw_text(img: &mut RgbaImage, text: &str, x0: u32, y0: u32, scale: u32, col: Rgba<u8>) {
        for (c_idx, ch) in text.chars().enumerate() {
            let bits = glyph_5x7(ch);
            let base_x = x0 + (c_idx as u32) * (6 * scale);
            for (row, &mask) in bits.iter().enumerate() {
                for col_i in 0..5_u32 {
                    if (mask >> (4 - col_i)) & 1 == 1 {
                        for sy in 0..scale {
                            for sx in 0..scale {
                                let px = base_x + col_i * scale + sx;
                                let py = y0 + (row as u32) * scale + sy;
                                if px < img.width() && py < img.height() {
                                    img.put_pixel(px, py, col);
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    fn raster_quad(
        state: &GlifoCanvasState,
        title: &str,
        accent: [u8; 4],
        cursor: Option<(f32, f32)>,
        drag_from: Option<(f32, f32)>,
    ) -> RgbaImage {
        let mut img = pixmap_to_rgba_image(rasterize_canvas_subregion(state, &state.theme, QW, QH));

        if let (Some((x0, y0)), Some((x1, y1))) = (drag_from, cursor) {
            for step in 0..=80 {
                if (step / 4) % 2 == 1 {
                    continue;
                }
                let t = (step as f32) / 80.0;
                let x = (x0 + (x1 - x0) * t).round() as i32;
                let y = (y0 + (y1 - y0) * t).round() as i32;
                for dy in -1..=1 {
                    for dx in -1..=1 {
                        let (px, py) = (x + dx, y + dy);
                        if px >= 0 && py >= 0 && (px as u32) < QW && (py as u32) < QH {
                            img.put_pixel(px as u32, py as u32, Rgba([255, 140, 0, 240]));
                        }
                    }
                }
            }
        }

        if let Some((cx, cy)) = cursor {
            for a in 0..60 {
                let rad = (a as f32) * std::f32::consts::TAU / 60.0;
                for r in [11.0_f32, 12.0_f32] {
                    let rx = (cx + r * rad.cos()).round() as i32;
                    let ry = (cy + r * rad.sin()).round() as i32;
                    if rx >= 0 && ry >= 0 && (rx as u32) < QW && (ry as u32) < QH {
                        img.put_pixel(rx as u32, ry as u32, Rgba(accent));
                    }
                }
            }
        }

        let banner_w = (28 + (title.len() as u32) * 12).min(QW - 8);
        for py in 8..34 {
            for px in 8..banner_w {
                img.put_pixel(px, py, if px < 18 { Rgba(accent) } else { Rgba([24, 26, 32, 240]) });
            }
        }
        draw_text(&mut img, title, 24, 14, 2, Rgba([255, 255, 255, 255]));
        img
    }

    fn compose_quad_frame(
        q_tl: &RgbaImage,
        q_tr: &RgbaImage,
        q_bl: &RgbaImage,
        q_br: &RgbaImage,
    ) -> RgbaImage {
        let mut frame = RgbaImage::from_pixel(W, H, Rgba([180, 185, 195, 255]));
        for (src, ox, oy) in [(q_tl, 0, 0), (q_tr, QW + 3, 0), (q_bl, 0, QH + 3), (q_br, QW + 3, QH + 3)] {
            for y in 0..QH {
                for x in 0..QW {
                    frame.put_pixel(ox + x, oy + y, *src.get_pixel(x, y));
                }
            }
        }
        frame
    }

    let count_delta = |a: &RgbaImage, b: &RgbaImage| -> usize {
        a.pixels().zip(b.pixels()).filter(|(p1, p2)| p1 != p2).count()
    };

    // ========================================================================
    // Frame 37: SelectTool + show_bounding_box = true, 8-Knob BBox + Pills + Scale/Rotate
    // ========================================================================
    let mut s37_a = GlifoCanvasState::init_glyph_o();
    s37_a.fit_viewport_to_glyph(QW as f32, QH as f32);
    s37_a.show_bounding_box = true;
    s37_a.selected_nodes = s37_a.contours[0].iter().map(|n| n.id).collect();
    let k8_37a = s37_a.selection_bounds().unwrap().knob_screen_center(8, &s37_a.viewport);
    let q37_a = raster_quad(
        &s37_a,
        "37A: /O 8-KNOB BBOX + W/H PILLS",
        [0, 122, 255, 255],
        Some((k8_37a.x.0, k8_37a.y.0)),
        None,
    );

    let mut s37_b = GlifoCanvasState::init_glyph_a();
    s37_b.fit_viewport_to_glyph(QW as f32, QH as f32);
    s37_b.show_bounding_box = true;
    s37_b.selected_nodes = s37_b.contours[0].iter().map(|n| n.id).collect();
    let b37_b0 = s37_b.selection_bounds().unwrap();
    let k8_37b_start = b37_b0.knob_screen_center(8, &s37_b.viewport);
    let k8_37b_end = ScreenPoint::new(k8_37b_start.x.0 + 38.0, k8_37b_start.y.0 - 24.0);
    pointer_press(&mut s37_b, k8_37b_start, false, false, false, false);
    pointer_move(&mut s37_b, k8_37b_end, false, false, false, false);
    pointer_release(&mut s37_b, k8_37b_end, false, false, false, false);
    let q37_b = raster_quad(
        &s37_b,
        "37B: /A BBOX KNOB-8 SCALE + PILLS",
        [40, 167, 69, 255],
        Some((k8_37b_end.x.0, k8_37b_end.y.0)),
        Some((k8_37b_start.x.0, k8_37b_start.y.0)),
    );

    let mut s37_c = GlifoCanvasState::init_glyph_o();
    s37_c.fit_viewport_to_glyph(QW as f32, QH as f32);
    s37_c.show_bounding_box = true;
    s37_c.selected_nodes = s37_c.contours[0].iter().map(|n| n.id).collect();
    let b37_c0 = s37_c.selection_bounds().unwrap();
    let k5_37c_start = b37_c0.knob_screen_center(5, &s37_c.viewport);
    let k5_37c_end = ScreenPoint::new(k5_37c_start.x.0 + 32.0, k5_37c_start.y.0);
    pointer_press(&mut s37_c, k5_37c_start, false, true, false, false);
    pointer_move(&mut s37_c, k5_37c_end, false, true, false, false);
    pointer_release(&mut s37_c, k5_37c_end, false, true, false, false);
    let q37_c = raster_quad(
        &s37_c,
        "37C: /O ALT-CENTER BBOX SCALE",
        [175, 82, 222, 255],
        Some((k5_37c_end.x.0, k5_37c_end.y.0)),
        Some((k5_37c_start.x.0, k5_37c_start.y.0)),
    );

    let mut s37_d = GlifoCanvasState::init_glyph_a();
    s37_d.fit_viewport_to_glyph(QW as f32, QH as f32);
    s37_d.show_bounding_box = true;
    s37_d.selected_nodes = s37_d.contours[0].iter().map(|n| n.id).collect();
    let b37_d0 = s37_d.selection_bounds().unwrap();
    let box_8 = b37_d0.box_screen_point(8, &s37_d.viewport);
    let knob_8 = b37_d0.knob_screen_center(8, &s37_d.viewport);
    let rot37_start = ScreenPoint::new(
        box_8.x.0 + 1.85 * (knob_8.x.0 - box_8.x.0),
        box_8.y.0 + 1.85 * (knob_8.y.0 - box_8.y.0),
    );
    let rot37_end = ScreenPoint::new(rot37_start.x.0 - 45.0, rot37_start.y.0 - 30.0);
    pointer_press(&mut s37_d, rot37_start, false, true, false, false);
    pointer_move(&mut s37_d, rot37_end, false, true, false, false);
    pointer_release(&mut s37_d, rot37_end, false, true, false, false);
    let q37_d = raster_quad(
        &s37_d,
        "37D: /A BBOX ZONE-3 ROTATE",
        [255, 149, 0, 255],
        Some((rot37_end.x.0, rot37_end.y.0)),
        Some((rot37_start.x.0, rot37_start.y.0)),
    );

    assert!(count_delta(&q37_a, &q37_c) > 500, "BBox Alt-center scale must visibly transform /o");
    assert!(count_delta(&q37_b, &q37_d) > 500, "BBox Zone-3 rotate must visibly transform /A");
    let frame_37 = compose_quad_frame(&q37_a, &q37_b, &q37_c, &q37_d);
    assert_eq!(frame_37.dimensions(), (W, H));

    // ========================================================================
    // Frame 38: RotateTool [R] & ScaleTool [S] Reticle + Active Drag Ray Line
    // ========================================================================
    let mut s38_a = GlifoCanvasState::init_glyph_o();
    s38_a.fit_viewport_to_glyph(QW as f32, QH as f32);
    s38_a.selected_nodes = s38_a.contours[0].iter().map(|n| n.id).collect();
    s38_a.activate_tool(CanvasTool::Rotate);
    s38_a.transform_origin = FontPoint::new(280.0, 255.0);
    s38_a.transform_origin_custom = true;
    let r38_start_f = FontPoint::new(530.0, 255.0);
    let r38_end_f = FontPoint::new(470.0, 440.0);
    let r38_start_s = s38_a.viewport.to_screen(r38_start_f);
    let r38_end_s = s38_a.viewport.to_screen(r38_end_f);
    pointer_press(&mut s38_a, r38_start_s, false, false, false, false);
    pointer_move(&mut s38_a, r38_end_s, false, false, false, false);
    assert!(matches!(s38_a.v020_drag, V020DragState::TransformPendingOrDragging { active: true, is_rotate: true, .. }));
    let q38_a = raster_quad(
        &s38_a,
        "38A: [R] ROTATE RETICLE + RAY",
        [0, 122, 255, 255],
        Some((r38_end_s.x.0, r38_end_s.y.0)),
        Some((r38_start_s.x.0, r38_start_s.y.0)),
    );

    let mut s38_b = GlifoCanvasState::init_glyph_a();
    s38_b.fit_viewport_to_glyph(QW as f32, QH as f32);
    s38_b.selected_nodes = s38_b.contours.iter().flat_map(|c| c.iter().map(|n| n.id)).collect();
    s38_b.activate_tool(CanvasTool::Rotate);
    let r38b_start_f = FontPoint::new(650.0, 350.0);
    let r38b_end_f = FontPoint::new(560.0, 580.0);
    let r38b_start_s = s38_b.viewport.to_screen(r38b_start_f);
    let r38b_end_s = s38_b.viewport.to_screen(r38b_end_f);
    pointer_press(&mut s38_b, r38b_start_s, true, false, false, false);
    pointer_move(&mut s38_b, r38b_end_s, true, false, false, false);
    let q38_b = raster_quad(
        &s38_b,
        "38B: [R] SHIFT 45 DEG SNAP + RAY",
        [40, 167, 69, 255],
        Some((r38b_end_s.x.0, r38b_end_s.y.0)),
        Some((r38b_start_s.x.0, r38b_start_s.y.0)),
    );

    let mut s38_c = GlifoCanvasState::init_glyph_o();
    s38_c.fit_viewport_to_glyph(QW as f32, QH as f32);
    s38_c.selected_nodes = s38_c.contours[0].iter().map(|n| n.id).collect();
    s38_c.activate_tool(CanvasTool::Scale);
    s38_c.transform_origin = FontPoint::new(100.0, 100.0);
    s38_c.transform_origin_custom = true;
    let s38c_start_s = s38_c.viewport.to_screen(FontPoint::new(450.0, 450.0));
    let s38c_end_s = s38_c.viewport.to_screen(FontPoint::new(540.0, 520.0));
    pointer_press(&mut s38_c, s38c_start_s, false, false, false, false);
    pointer_move(&mut s38_c, s38c_end_s, false, false, false, false);
    assert!(matches!(s38_c.v020_drag, V020DragState::TransformPendingOrDragging { active: true, is_rotate: false, .. }));
    let q38_c = raster_quad(
        &s38_c,
        "38C: [S] SCALE RETICLE + RAY",
        [175, 82, 222, 255],
        Some((s38c_end_s.x.0, s38c_end_s.y.0)),
        Some((s38c_start_s.x.0, s38c_start_s.y.0)),
    );

    let mut s38_d = GlifoCanvasState::init_glyph_n();
    s38_d.fit_viewport_to_glyph(QW as f32, QH as f32);
    s38_d.set_metrics_origin(MetricsOrigin::HalfXHeight);
    s38_d.activate_tool(CanvasTool::Scale);
    s38_d.flip_horizontal();
    s38_d.slant_by_degrees(12.0, true);
    let orig38_d = s38_d.viewport.to_screen(s38_d.transform_origin);
    let q38_d = raster_quad(
        &s38_d,
        "38D: FLIP H + SLANT 12 DEG (/N)",
        [255, 149, 0, 255],
        Some((orig38_d.x.0, orig38_d.y.0)),
        None,
    );

    assert!(count_delta(&q38_a, &q38_c) > 500);
    let frame_38 = compose_quad_frame(&q38_a, &q38_b, &q38_c, &q38_d);
    assert_eq!(frame_38.dimensions(), (W, H));

    // ========================================================================
    // Frame 39: LassoTool [Shift+V] Freeform Polygon & ZoomTool [Z] Marquee
    // ========================================================================
    let mut s39_a = GlifoCanvasState::init_glyph_o();
    s39_a.fit_viewport_to_glyph(QW as f32, QH as f32);
    s39_a.activate_tool(CanvasTool::Lasso);
    let lasso_pts_a = [
        FontPoint::new(40.0, 220.0),
        FontPoint::new(310.0, 560.0),
        FontPoint::new(560.0, 480.0),
        FontPoint::new(520.0, 200.0),
        FontPoint::new(260.0, 180.0),
    ];
    let scr_a0 = s39_a.viewport.to_screen(lasso_pts_a[0]);
    pointer_press(&mut s39_a, scr_a0, false, false, false, false);
    for pt in &lasso_pts_a[1..] {
        let scr = s39_a.viewport.to_screen(*pt);
        pointer_move(&mut s39_a, scr, false, false, false, false);
    }
    assert!(matches!(s39_a.v020_drag, V020DragState::LassoDragging { .. }));
    assert!(!s39_a.selected_nodes.is_empty());
    let scr_a_last = s39_a.viewport.to_screen(*lasso_pts_a.last().unwrap());
    let q39_a = raster_quad(
        &s39_a,
        "39A: [SHIFT+V] LASSO POLYGON (/O)",
        [0, 122, 255, 255],
        Some((scr_a_last.x.0, scr_a_last.y.0)),
        Some((scr_a0.x.0, scr_a0.y.0)),
    );

    let mut s39_b = GlifoCanvasState::init_glyph_s();
    s39_b.fit_viewport_to_glyph(QW as f32, QH as f32);
    s39_b.activate_tool(CanvasTool::Lasso);
    let lasso_pts_b = [
        FontPoint::new(20.0, 120.0),
        FontPoint::new(80.0, 540.0),
        FontPoint::new(480.0, 540.0),
        FontPoint::new(460.0, 220.0),
    ];
    let scr_b0 = s39_b.viewport.to_screen(lasso_pts_b[0]);
    pointer_press(&mut s39_b, scr_b0, false, true, false, false);
    for pt in &lasso_pts_b[1..] {
        let scr = s39_b.viewport.to_screen(*pt);
        pointer_move(&mut s39_b, scr, false, true, false, false);
    }
    assert!(s39_b.selected_nodes.iter().all(|&id| s39_b.find_node(id).unwrap().node_type != NodeType::OffCurve));
    let scr_b_last = s39_b.viewport.to_screen(*lasso_pts_b.last().unwrap());
    let q39_b = raster_quad(
        &s39_b,
        "39B: [ALT+LASSO] ONCURVE ONLY (/S)",
        [40, 167, 69, 255],
        Some((scr_b_last.x.0, scr_b_last.y.0)),
        Some((scr_b0.x.0, scr_b0.y.0)),
    );

    let mut s39_c = GlifoCanvasState::init_glyph_a();
    s39_c.fit_viewport_to_glyph(QW as f32, QH as f32);
    s39_c.activate_tool(CanvasTool::Zoom);
    let z_start = ScreenPoint::new(190.0, 95.0);
    let z_end = ScreenPoint::new(420.0, 265.0);
    pointer_press(&mut s39_c, z_start, false, false, false, false);
    pointer_move(&mut s39_c, z_end, false, false, false, false);
    assert!(matches!(s39_c.v020_drag, V020DragState::ZoomDragging { .. }));
    let q39_c = raster_quad(
        &s39_c,
        "39C: [Z] ZOOM MARQUEE DRAG (/A)",
        [0, 122, 255, 255],
        Some((z_end.x.0, z_end.y.0)),
        Some((z_start.x.0, z_start.y.0)),
    );

    let mut s39_d = GlifoCanvasState::init_glyph_a();
    s39_d.fit_viewport_to_glyph(QW as f32, QH as f32);
    s39_d.activate_tool(CanvasTool::Zoom);
    let z_start_f = s39_d.viewport.to_font(z_start);
    let z_end_f = s39_d.viewport.to_font(z_end);
    let _ = grafia::adapters::v020_interaction::handle_v020_pointer_press(
        &mut s39_d,
        z_start,
        z_start_f,
        grafia::Modifiers::default(),
        false,
        false,
    );
    let _ = grafia::adapters::v020_interaction::handle_v020_pointer_move(
        &mut s39_d,
        z_end,
        z_end_f,
        false,
        false,
        false,
        false,
    );
    let _ = grafia::adapters::v020_interaction::handle_v020_pointer_release(
        &mut s39_d,
        false,
        false,
        false,
        Rect::from_min_size(Pos2::ZERO, Vec2::new(QW as f32, QH as f32)),
    );
    let q39_d = raster_quad(
        &s39_d,
        "39D: [Z] MARQUEE ZOOMED IN (/A)",
        [255, 149, 0, 255],
        Some((QW as f32 * 0.5, QH as f32 * 0.5)),
        None,
    );

    assert!(count_delta(&q39_c, &q39_d) > 1000);
    let frame_39 = compose_quad_frame(&q39_a, &q39_b, &q39_c, &q39_d);
    assert_eq!(frame_39.dimensions(), (W, H));

    // ========================================================================
    // Frame 40: add_extremes + add_inflection_points (/s), connect_open, & Duplicate Warnings -> tidy_up_paths
    // ========================================================================
    let mut s40_a = GlifoCanvasState::init_glyph_s();
    // Append a tilted bowed cubic loop beside /s to showcase simultaneous extrema + inflection insertion
    let extra_s_curve = vec![
        Node::new(NodeId(401), 460.0, 60.0, NodeType::Corner),
        Node::new(NodeId(402), 360.0, 240.0, NodeType::OffCurve),
        Node::new(NodeId(403), 680.0, 320.0, NodeType::OffCurve),
        Node::new(NodeId(404), 560.0, 500.0, NodeType::Corner),
    ];
    s40_a.contours.push(extra_s_curve.clone());
    let mut gc_extra = GlyphContour::new(grafia::ContourId(1), extra_s_curve);
    gc_extra.closed = false;
    s40_a.glyph.contours.push(gc_extra);
    s40_a.glyph.width = 680.0;
    s40_a.sync_paths_from_contours();
    s40_a.fit_viewport_to_glyph(QW as f32, QH as f32);
    let _ = s40_a.add_extremes(true);
    let _ = s40_a.add_inflection_points();
    s40_a.selected_nodes = s40_a
        .contours
        .iter()
        .flat_map(|c| c.iter().filter(|n| n.node_type == NodeType::Smooth).map(|n| n.id))
        .collect();
    let q40_a = raster_quad(
        &s40_a,
        "40A: /S EXTREMES + INFLECTIONS",
        [40, 167, 69, 255],
        None,
        None,
    );

    let mut s40_b = GlifoCanvasState::new(GlifoTheme::sonoma_default());
    let open_1 = vec![
        Node::new(NodeId(1), 60.0, 120.0, NodeType::Corner),
        Node::new(NodeId(2), 60.0, 460.0, NodeType::Corner),
        Node::new(NodeId(3), 260.0, 460.0, NodeType::Corner),
    ];
    let open_2 = vec![
        Node::new(NodeId(4), 340.0, 460.0, NodeType::Corner),
        Node::new(NodeId(5), 540.0, 460.0, NodeType::Corner),
        Node::new(NodeId(6), 540.0, 120.0, NodeType::Corner),
    ];
    s40_b.contours = vec![open_1.clone(), open_2.clone()];
    let mut gc1 = GlyphContour::new(grafia::ContourId(0), open_1);
    gc1.closed = false;
    let mut gc2 = GlyphContour::new(grafia::ContourId(1), open_2);
    gc2.closed = false;
    s40_b.glyph.contours = vec![gc1, gc2];
    s40_b.glyph.width = 600.0;
    s40_b.glyph.height = 600.0;
    s40_b.sync_paths_from_contours();
    s40_b.fit_viewport_to_glyph(QW as f32, QH as f32);
    s40_b.selected_nodes = vec![NodeId(3), NodeId(4)];
    assert!(s40_b.connect_selected_open_endpoints());
    s40_b.selected_nodes = vec![NodeId(1), NodeId(6)];
    assert!(s40_b.connect_selected_open_endpoints());
    assert!(s40_b.is_contour_closed(0));
    let q40_b = raster_quad(
        &s40_b,
        "40B: CONNECT OPEN ENDPOINTS",
        [0, 122, 255, 255],
        None,
        None,
    );

    let mut s40_c = GlifoCanvasState::new(GlifoTheme::sonoma_default());
    let warn_nodes = vec![
        Node::new(NodeId(1), 80.0, 80.0, NodeType::Corner),
        Node::new(NodeId(2), 80.0, 80.0, NodeType::Corner), // Red CoincidentOnCurve warning!
        Node::new(NodeId(3), 290.0, 80.0, NodeType::OffCurve),
        Node::new(NodeId(4), 290.0, 80.0, NodeType::OffCurve), // Yellow CoincidentOffCurve warning + collinear on line!
        Node::new(NodeId(5), 500.0, 80.0, NodeType::Corner),
        Node::new(NodeId(6), 500.0, 320.0, NodeType::Corner), // Will promote to Smooth on tidy!
        Node::new(NodeId(7), 500.0, 480.0, NodeType::OffCurve),
        Node::new(NodeId(8), 320.0, 560.0, NodeType::OffCurve),
        Node::new(NodeId(9), 80.0, 560.0, NodeType::Smooth),
    ];
    set_single_contour(&mut s40_c, warn_nodes, true);
    s40_c.glyph.width = 600.0;
    s40_c.glyph.height = 650.0;
    s40_c.fit_viewport_to_glyph(QW as f32, QH as f32);
    let warnings_before = s40_c.duplicate_warning_nodes();
    assert!(warnings_before.iter().any(|(_, _, k)| *k == DuplicateWarningKind::CoincidentOnCurve));
    assert!(warnings_before.iter().any(|(_, _, k)| *k == DuplicateWarningKind::CoincidentOffCurve));
    let q40_c = raster_quad(
        &s40_c,
        "40C: DUPLICATE WARNINGS (RED/YLW)",
        [230, 38, 38, 255],
        None,
        None,
    );

    let mut s40_d = GlifoCanvasState::new(GlifoTheme::sonoma_default());
    s40_d.contours = s40_c.contours.clone();
    s40_d.glyph = s40_c.glyph.clone();
    s40_d.viewport = s40_c.viewport;
    s40_d.sync_paths_from_contours();
    assert!(s40_d.tidy_up_paths());
    assert!(s40_d.duplicate_warning_nodes().is_empty(), "tidy_up_paths must clear duplicate warnings");
    let q40_d = raster_quad(
        &s40_d,
        "40D: AFTER TIDY UP PATHS (CLEAN)",
        [40, 167, 69, 255],
        None,
        None,
    );

    assert!(count_delta(&q40_c, &q40_d) > 200, "tidy_up_paths must visibly remove warning circles");
    let frame_40 = compose_quad_frame(&q40_a, &q40_b, &q40_c, &q40_d);
    assert_eq!(frame_40.dimensions(), (W, H));

    // ========================================================================
    // Save all 4 1223x811 frames + 2x2 verification strip (2446x1622)
    // ========================================================================
    let named_frames = [
        ("frame_37_v020_bounding_box_and_pills.png", &frame_37),
        ("frame_38_v020_rotate_and_scale_reticle.png", &frame_38),
        ("frame_39_v020_lasso_and_zoom_marquee.png", &frame_39),
        ("frame_40_v020_path_ops_and_duplicate_warnings.png", &frame_40),
    ];

    let mut strip = RgbaImage::from_pixel(W * 2, H * 2, Rgba([24, 24, 28, 255]));
    for (idx, (fname, img)) in named_frames.iter().enumerate() {
        let col = (idx as u32) % 2;
        let row = (idx as u32) / 2;
        for py in 0..H {
            for px in 0..W {
                strip.put_pixel(col * W + px, row * H + py, *img.get_pixel(px, py));
            }
        }
        for dir in &target_dirs {
            let p = dir.join(fname);
            img.save(&p).expect("Must save v0.2.0 verification PNG frame");
            assert!(p.exists());
        }
    }

    for dir in &target_dirs {
        let strip_path = dir.join("glifo_v020_verification_strip.png");
        strip.save(&strip_path).expect("Must save glifo_v020_verification_strip.png");
        assert!(strip_path.exists());
    }

    // Frame 41: Vector Cursors (ZoomIn, Rotate, Lasso, BBoxSlant) + Slanted Marquee Parallelogram
    let mut s41_a = GlifoCanvasState::init_glyph_a();
    s41_a.fit_viewport_to_glyph(QW as f32, QH as f32);
    s41_a.show_bounding_box = true;
    s41_a.selected_nodes = s41_a.contours[0].iter().map(|n| n.id).collect();
    let b41_a = s41_a.selection_bounds().unwrap();
    let box_5_41 = b41_a.box_screen_point(5, &s41_a.viewport);
    let knob_5_41 = b41_a.knob_screen_center(5, &s41_a.viewport);
    let slant_hover_pt = ScreenPoint::new(
        box_5_41.x.0 + 1.85 * (knob_5_41.x.0 - box_5_41.x.0),
        box_5_41.y.0 + 1.85 * (knob_5_41.y.0 - box_5_41.y.0),
    );
    pointer_move(&mut s41_a, slant_hover_pt, false, true, false, true);
    s41_a.cursor_screen_pos = Some(slant_hover_pt);
    let q41_a = raster_quad(
        &s41_a,
        "41A: BBOX SLANT VECTOR CURSOR (CTRL+ALT)",
        [0, 122, 255, 255],
        Some((slant_hover_pt.x.0, slant_hover_pt.y.0)),
        None,
    );

    let mut s41_b = GlifoCanvasState::init_glyph_o();
    s41_b.fit_viewport_to_glyph(QW as f32, QH as f32);
    let m41_start = ScreenPoint::new(150.0, 320.0);
    let m41_end = ScreenPoint::new(360.0, 90.0);
    pointer_press(&mut s41_b, m41_start, false, false, false, true);
    pointer_move(&mut s41_b, m41_end, false, false, false, true);
    s41_b.cursor_screen_pos = Some(m41_end);
    let q41_b = raster_quad(
        &s41_b,
        "41B: SLANTED MARQUEE PARALLELOGRAM (CTRL)",
        [40, 167, 69, 255],
        Some((m41_end.x.0, m41_end.y.0)),
        Some((m41_start.x.0, m41_start.y.0)),
    );

    let mut s41_c = GlifoCanvasState::init_glyph_s();
    s41_c.fit_viewport_to_glyph(QW as f32, QH as f32);
    s41_c.activate_tool(CanvasTool::Lasso);
    let lasso_cur = ScreenPoint::new(300.0, 200.0);
    pointer_move(&mut s41_c, lasso_cur, false, false, false, false);
    s41_c.cursor_screen_pos = Some(lasso_cur);
    let q41_c = raster_quad(
        &s41_c,
        "41C: LASSO VECTOR CURSOR [SHIFT+V]",
        [175, 82, 222, 255],
        Some((lasso_cur.x.0, lasso_cur.y.0)),
        None,
    );

    let mut s41_d = GlifoCanvasState::init_glyph_n();
    s41_d.fit_viewport_to_glyph(QW as f32, QH as f32);
    s41_d.activate_tool(CanvasTool::Zoom);
    let zoom_cur = ScreenPoint::new(300.0, 200.0);
    pointer_move(&mut s41_d, zoom_cur, false, false, false, false);
    s41_d.cursor_screen_pos = Some(zoom_cur);
    let q41_d = raster_quad(
        &s41_d,
        "41D: ZOOM-IN (+) VECTOR CURSOR [Z / CMD+SPACE]",
        [255, 149, 0, 255],
        Some((zoom_cur.x.0, zoom_cur.y.0)),
        None,
    );

    let frame_41 = compose_quad_frame(&q41_a, &q41_b, &q41_c, &q41_d);
    for dir in &target_dirs {
        let p = dir.join("frame_41_v020_vector_cursors_and_slanted_marquee.png");
        frame_41.save(&p).expect("Must save frame_41_v020_vector_cursors_and_slanted_marquee.png");
        assert!(p.exists());
    }
}
