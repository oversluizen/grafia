//! Normative COTS Verification Suite for `SPEC-004` (`glifo v0.3.0`):
//! Local & Global Guides (`CanvasGuide`), Magnetic `SnapEngine` (`SnapEngine`), & Alignment Zone Badges
//! (`TC-V030-001` .. `TC-V030-007`).
//!
//! Grounded 100% in Live Reference Vector reference specification & reference Runtime Truth.

use egui::{Event, Key, Modifiers as EguiModifiers, PointerButton, Pos2, RawInput, Rect, Vec2};
use grafia::adapters::headless::rasterize_canvas_subregion;
use grafia::adapters::input_dispatcher::InputDispatcher;
use grafia::{
    AlignmentBadgeMode, AlignmentZone, FontPoint, GlifoCanvasState, GlifoTheme, Node, NodeId,
    NodeType, ScreenPoint, SelectedGuideRef,
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
        ctrl,
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
        ctrl,
        shift,
        mac_cmd: cmd,
        command: cmd,
    }
}

fn press_key_v030(
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

fn assert_pt_near(actual: FontPoint, expected_x: f64, expected_y: f64, tol: f64, label: &str) {
    assert!(
        (actual.x.0 - expected_x).abs() <= tol && (actual.y.0 - expected_y).abs() <= tol,
        "{label}: expected ({expected_x:.6}, {expected_y:.6}), got ({:.6}, {:.6}) (tol={tol})",
        actual.x.0,
        actual.y.0
    );
}

// ============================================================================
// TC-V030-001: Local & Global Guide Creation, Smart 2-Node Rule & Visibility Toggle
// ============================================================================
#[test]
fn test_tc_v030_001_guide_creation_smart_two_node_rule_and_visibility_toggle() {
    let mut state = GlifoCanvasState::new(GlifoTheme::sonoma_default());
    state.viewport.zoom = 1.0;
    state.viewport.pan_offset = ScreenPoint::new(200.0, 400.0);

    // Verify exact empirical Reference Vector color tokens
    let c = &state.theme.edit_canvas;
    assert_eq!((c.local_guide_color.r, c.local_guide_color.g, c.local_guide_color.b, c.local_guide_color.a), (62, 76, 207, 179));
    assert_eq!((c.global_guide_color.r, c.global_guide_color.g, c.global_guide_color.b, c.global_guide_color.a), (207, 79, 62, 179));
    assert_eq!((c.alignment_zone_fill.r, c.alignment_zone_fill.g, c.alignment_zone_fill.b, c.alignment_zone_fill.a), (206, 167, 124, 51));
    assert_eq!((c.snap_indicator_color.r, c.snap_indicator_color.g, c.snap_indicator_color.b, c.snap_indicator_color.a), (153, 51, 51, 255));

    // Case 0: 0 selected nodes -> Add Local Guide creates horizontal (0.0 deg) guide at fallback/cursor position
    state.selected_nodes.clear();
    state.show_guides = false;
    let id0 = state.add_guide_from_selection(false, FontPoint::new(120.0, 250.0));
    assert!(state.show_guides, "Adding a guide must automatically enable show_guides");
    assert_eq!(state.guides.len(), 1);
    assert_eq!(state.selected_guide, Some(SelectedGuideRef { id: id0, is_global: false }));
    assert_pt_near(state.guides[0].position, 120.0, 250.0, 1e-6, "Case 0 local guide pos");
    assert!((state.guides[0].angle_deg - 0.0).abs() < 1e-6);
    assert!(!state.guides[0].is_global);

    // Infinite line endpoints at 200,000 UPM (`beginPointForLayerWidth:` / `endPointForLayerWidth:`)
    let (begin, end) = state.guides[0].infinite_endpoints();
    assert_pt_near(begin, 120.0 - 200_000.0, 250.0, 1e-3, "beginPoint 0 deg");
    assert_pt_near(end, 120.0 + 200_000.0, 250.0, 1e-3, "endPoint 0 deg");

    // Case 1: 1 selected node -> Add Global Guide creates horizontal (0.0 deg) global guide at node position
    state.load_preset_a();
    let node_a = state.contours[0][0].clone();
    state.selected_nodes = vec![node_a.id];
    let id1 = state.add_guide_from_selection(true, FontPoint::new(0.0, 0.0));
    assert_eq!(state.global_guides.len(), 1);
    assert_eq!(state.selected_guide, Some(SelectedGuideRef { id: id1, is_global: true }));
    assert_pt_near(state.global_guides[0].position, node_a.point.x.0, node_a.point.y.0, 1e-6, "Case 1 global guide pos");
    assert!((state.global_guides[0].angle_deg - 0.0).abs() < 1e-6);
    assert!(state.global_guides[0].is_global);

    // Case 2: Smart 2-Node Rule (`PathRenderer::doAddGuide:` @ spec_ref..spec_ref)
    // Select 2 nodes at (100, 100) and (300, 300) -> midpoint (200, 200), angle = 45.0 deg!
    state.contours = vec![vec![
        Node::new(NodeId(10), 100.0, 100.0, NodeType::Corner),
        Node::new(NodeId(11), 300.0, 300.0, NodeType::Corner),
        Node::new(NodeId(12), 100.0, 300.0, NodeType::Corner),
    ]];
    state.sync_paths_from_contours();
    state.selected_nodes = vec![NodeId(10), NodeId(11)];
    let id2 = state.add_guide_from_selection(false, FontPoint::new(0.0, 0.0));
    let g2 = state.find_guide(id2).expect("Smart 2-node guide must exist");
    assert_pt_near(g2.position, 200.0, 200.0, 1e-6, "Smart 2-Node midpoint");
    assert!(
        (g2.angle_deg - 45.0).abs() < 1e-6 || (g2.angle_deg - (-135.0)).abs() < 1e-6,
        "Smart 2-Node angle must pass through both nodes (45 deg), got {}",
        g2.angle_deg
    );
    assert!(state.selected_nodes.is_empty(), "Adding a guide clears node selection and selects guide handle");

    // Cmd/Ctrl+Shift+L toggles `show_guides`
    assert!(state.show_guides);
    press_key_v030(&mut state, Key::L, true, false, true, true);
    assert!(!state.show_guides);
    press_key_v030(&mut state, Key::L, true, false, true, true);
    assert!(state.show_guides);
}

// ============================================================================
// TC-V030-002: Guide Knob Selection, Drag Move, Alt-Duplicate, Double-Click 90° Flip,
// Line-Drag Rotation, Lock Angle & Lock Guides
// ============================================================================
#[test]
fn test_tc_v030_002_guide_knob_move_duplicate_flip90_rotate_and_locks() {
    let mut state = GlifoCanvasState::new(GlifoTheme::sonoma_default());
    state.viewport.zoom = 1.0;
    state.viewport.pan_offset = ScreenPoint::new(200.0, 400.0);
    state.contours.clear();
    state.sync_paths_from_contours();

    let gid = state.add_guide_from_selection(false, FontPoint::new(100.0, 100.0));
    assert_eq!(state.guides.len(), 1);

    // 1. Drag guide knob from (100, 100) to (150, 180) (with Ctrl held to bypass magnetic snap for exact raw move test)
    let knob_scr = state.viewport.to_screen(FontPoint::new(100.0, 100.0));
    let dest_scr = state.viewport.to_screen(FontPoint::new(150.0, 180.0));
    pointer_press(&mut state, knob_scr, false, false, false, true);
    pointer_move(&mut state, dest_scr, false, false, false, true);
    pointer_release(&mut state, dest_scr, false, false, false, true);
    assert_pt_near(state.find_guide(gid).unwrap().position, 150.0, 180.0, 1e-4, "Dragged guide knob");

    // 2. Arrow-key nudge on selected guide (`1`, `10` with Shift, `100` with Cmd/Ctrl)
    press_key_v030(&mut state, Key::ArrowUp, false, false, false, false);
    assert_pt_near(state.find_guide(gid).unwrap().position, 150.0, 181.0, 1e-4, "Nudge +1Y");
    press_key_v030(&mut state, Key::ArrowRight, true, false, false, false);
    assert_pt_near(state.find_guide(gid).unwrap().position, 160.0, 181.0, 1e-4, "Nudge +10X");

    // 3. Double-Click on Guide Knob flips angle by +90.0 deg perpendicular (0.0 -> 90.0 -> 180.0 -> -90.0)
    state.double_click_guide_knob(gid);
    assert!((state.find_guide(gid).unwrap().angle_deg - 90.0).abs() < 1e-6);
    state.double_click_guide_knob(gid);
    assert!((state.find_guide(gid).unwrap().angle_deg - 180.0).abs() < 1e-6);
    state.double_click_guide_knob(gid);
    assert!((state.find_guide(gid).unwrap().angle_deg - (-90.0)).abs() < 1e-6);
    state.double_click_guide_knob(gid);
    assert!((state.find_guide(gid).unwrap().angle_deg - 0.0).abs() < 1e-6);

    // 4. Dragging on the selected guide line away from the knob rotates `angle_deg` around `position`
    // Guide is horizontal (0 deg) through (160, 181). Grab the line at (260, 181) and drag to (260, 281) -> 45.0 deg!
    let line_grab_scr = state.viewport.to_screen(FontPoint::new(260.0, 181.0));
    let line_rot_scr = state.viewport.to_screen(FontPoint::new(260.0, 281.0));
    pointer_press(&mut state, line_grab_scr, false, false, false, false);
    pointer_move(&mut state, line_rot_scr, true, false, false, false); // Shift snaps to 45 deg increments
    pointer_release(&mut state, line_rot_scr, true, false, false, false);
    assert!(
        (state.find_guide(gid).unwrap().angle_deg - 45.0).abs() < 1e-4,
        "Dragging guide line to +100,+100 from knob must rotate guide to 45.0 deg, got {}",
        state.find_guide(gid).unwrap().angle_deg
    );

    // 4b. When `italic_angle_deg != 0.0` (e.g. 12.0 deg), dragging guide line with `Shift` snaps to `90.0 - italic_angle_deg` (78.0 deg)
    state.italic_angle_deg = 12.0;
    let dx_italic = 100.0 * (79.5_f64.to_radians()).cos();
    let dy_italic = 100.0 * (79.5_f64.to_radians()).sin();
    let line_grab_45 = state.viewport.to_screen(FontPoint::new(260.0, 281.0));
    let line_rot_italic = state.viewport.to_screen(FontPoint::new(160.0 + dx_italic, 181.0 + dy_italic));
    pointer_press(&mut state, line_grab_45, false, false, false, false);
    pointer_move(&mut state, line_rot_italic, true, false, false, false);
    pointer_release(&mut state, line_rot_italic, true, false, false, false);
    assert!(
        (state.find_guide(gid).unwrap().angle_deg - 78.0).abs() < 1e-4,
        "Shift-dragging guide line near italic angle (90 - 12 = 78 deg) must snap to 78.0 deg, got {}",
        state.find_guide(gid).unwrap().angle_deg
    );
    state.italic_angle_deg = 0.0;
    state.find_guide_mut(gid).unwrap().angle_deg = 45.0;

    // 5. `lock_angle == true` prevents both double-click 90° flip and line-drag rotation
    state.find_guide_mut(gid).unwrap().lock_angle = true;
    state.double_click_guide_knob(gid);
    assert!((state.find_guide(gid).unwrap().angle_deg - 45.0).abs() < 1e-6, "lock_angle must block 90 deg flip");
    state.find_guide_mut(gid).unwrap().lock_angle = false;

    // 6. Option/Alt-drag on guide knob duplicates the guide and drags the duplicate
    let knob_now_scr = state.viewport.to_screen(FontPoint::new(160.0, 181.0));
    let dup_dest_scr = state.viewport.to_screen(FontPoint::new(200.0, 220.0));
    pointer_press(&mut state, knob_now_scr, false, true, false, true);
    pointer_move(&mut state, dup_dest_scr, false, true, false, true);
    pointer_release(&mut state, dup_dest_scr, false, true, false, true);
    assert_eq!(state.guides.len(), 2, "Alt-dragging guide knob must duplicate the guide");

    // 7. `lock_guides == true` prevents moving or deleting guides
    state.lock_guides = true;
    let count_before = state.guides.len();
    press_key_v030(&mut state, Key::Backspace, false, false, false, false);
    assert_eq!(state.guides.len(), count_before, "lock_guides must prevent deleting guides");
}

// ============================================================================
// TC-V030-003: Measurement Guide Mode (`show_measurement == true` / `calculateIntersectionsForLayer:`)
// ============================================================================
#[test]
fn test_tc_v030_003_measurement_guide_ray_contour_intersections_and_pass4_persistence() {
    let mut state = GlifoCanvasState::new(GlifoTheme::sonoma_default());
    state.load_preset_o(); // `/o` has outer contour (50..550) and inner counter (140..460) at y = 250.0
    state.viewport.zoom = 0.8;
    state.viewport.pan_offset = ScreenPoint::new(140.0, 460.0);

    let gid = state.add_guide_from_selection(false, FontPoint::new(300.0, 250.0));
    state.find_guide_mut(gid).unwrap().angle_deg = 0.0;
    state.find_guide_mut(gid).unwrap().show_measurement = true;

    let measurement = state
        .guide_measurement_ray(gid)
        .expect("Guide with show_measurement=true must compute ray intersections");

    // `/o` at y = 250 has 4 contour intersections: x = 50, 140, 420, 510 -> 3 interior segments (90, 280, 90)
    assert_eq!(
        measurement.intersections.len(),
        4,
        "Horizontal measurement guide through /o at y=250 must find 4 contour intersections"
    );
    assert!((measurement.intersections[0].x.0 - 50.0).abs() < 1.0);
    assert!((measurement.intersections[1].x.0 - 140.0).abs() < 1.0);
    assert!((measurement.intersections[2].x.0 - 420.0).abs() < 1.0);
    assert!((measurement.intersections[3].x.0 - 510.0).abs() < 1.0);
    assert_eq!(measurement.segments.len(), 3);
    assert!((measurement.segments[0].distance - 90.0).abs() < 1.5, "Left stem width ~ 90 UPM");
    assert!((measurement.segments[1].distance - 280.0).abs() < 1.5, "Counter width ~ 280 UPM");
    assert!((measurement.segments[2].distance - 90.0).abs() < 1.5, "Right stem width ~ 90 UPM");
}

// ============================================================================
// TC-V030-004: Magnetic `SnapEngine` (`SnapEngine` Parity) & Pass 4 `#993333FF` Indicators
// ============================================================================
#[test]
fn test_tc_v030_004_magnetic_snap_engine_nodes_metrics_guides_and_intersection() {
    let mut state = GlifoCanvasState::new(GlifoTheme::sonoma_default());
    state.viewport.zoom = 1.0; // snap_threshold = 5.0 dp = 5.0 UPM
    state.viewport.pan_offset = ScreenPoint::new(200.0, 400.0);

    // 2 OnCurve nodes: NodeId(1) fixed at (100, 200), NodeId(2) dragged near x=100 and y=500 (x-height)
    state.contours = vec![vec![
        Node::new(NodeId(1), 100.0, 200.0, NodeType::Corner),
        Node::new(NodeId(2), 250.0, 350.0, NodeType::Corner),
        Node::new(NodeId(3), 300.0, 100.0, NodeType::Corner),
    ]];
    state.sync_paths_from_contours();

    // Also add a vertical Local Guide at x = 400.0
    let gid = state.add_guide_from_selection(false, FontPoint::new(400.0, 250.0));
    state.find_guide_mut(gid).unwrap().angle_deg = 90.0;
    state.selected_guide = None;

    // Select NodeId(2) at (250, 350) and drag it to (102.0, 498.5):
    // - Within 2.0 UPM of NodeId(1).x = 100.0 (Vertical node alignment snapper)
    // - Within 1.5 UPM of Metrics x_height = 500.0 (Horizontal metric line snapper)
    let start_scr = state.viewport.to_screen(FontPoint::new(250.0, 350.0));
    let near_snap_scr = state.viewport.to_screen(FontPoint::new(102.0, 498.5));
    pointer_press(&mut state, start_scr, false, false, false, false);
    pointer_move(&mut state, near_snap_scr, false, false, false, false);

    // Must snap to exact intersection (100.0, 500.0) and populate active_snappers (at most 2)!
    let snapped_pt = state.find_node(NodeId(2)).unwrap().point;
    assert_pt_near(snapped_pt, 100.0, 500.0, 1e-4, "2-axis magnetic snap (Node X=100 + x-Height Y=500)");
    assert!(!state.active_snappers.is_empty(), "Active magnetic snappers must be populated during drag");
    assert!(state.active_snappers.len() <= 2, "SnapEngine caps active snappers at 2");

    // Drag near the vertical Local Guide at x = 400.0 (e.g., (397.5, 310.0))
    let near_guide_scr = state.viewport.to_screen(FontPoint::new(397.5, 310.0));
    pointer_move(&mut state, near_guide_scr, false, false, false, false);
    let snapped_guide_pt = state.find_node(NodeId(2)).unwrap().point;
    assert!((snapped_guide_pt.x.0 - 400.0).abs() < 1e-4, "Must snap X to vertical guide at x=400.0");

    // Releasing mouse clears active_snappers
    pointer_release(&mut state, near_guide_scr, false, false, false, false);
    assert!(state.active_snappers.is_empty(), "Mouse release must clear active_snappers");

    // 4b. Italic-angle non-self-intersection: when `italic_angle_deg = 12.0`, dragging NodeId(2) to
    // `(100.0, 208.0)` (`8.0 UPM` vertically above NodeId(1) `(100.0, 200.0)`, outside its 5.0 UPM point radius)
    // must snap to the vertical line x = 100.0 at `(100.0, 208.0)`, NOT self-intersect NodeId(1)'s own
    // 90° and 78° lines and yank the node back to `(100.0, 200.0)`!
    state.italic_angle_deg = 12.0;
    let cur_n2 = state.find_node(NodeId(2)).unwrap().point;
    let cur_n2_scr = state.viewport.to_screen(cur_n2);
    let above_n1_scr = state.viewport.to_screen(FontPoint::new(100.0, 208.0));
    pointer_press(&mut state, cur_n2_scr, false, false, false, false);
    pointer_move(&mut state, above_n1_scr, false, false, false, false);
    let pt_above = state.find_node(NodeId(2)).unwrap().point;
    assert_pt_near(
        pt_above,
        100.0,
        208.0,
        1e-4,
        "Italic-angle line from same node must not self-intersect 90° line to yank (100, 208) back to (100, 200)",
    );
    pointer_release(&mut state, above_n1_scr, false, false, false, false);
    state.italic_angle_deg = 0.0;

    // 4c. Direct point-target snap (`is_point_target == true`) to a diagonal guide knob `(200.0, 200.0)` at `35.0°`
    // when approached diagonally at `(202.0, 202.0)` (`distance = 2.828 <= 5.0 UPM`).
    state.guides.clear();
    state.selected_nodes.clear();
    let diag_gid = state.add_guide_from_selection(false, FontPoint::new(200.0, 200.0));
    state.set_guide_angle(diag_gid, 35.0);
    state.selected_guide = None;
    let n2_start_scr = state.viewport.to_screen(state.find_node(NodeId(2)).unwrap().point);
    let near_knob_scr = state.viewport.to_screen(FontPoint::new(202.0, 202.0));
    pointer_press(&mut state, n2_start_scr, false, false, false, false);
    pointer_move(&mut state, near_knob_scr, false, false, false, false);
    let snapped_knob_pt = state.find_node(NodeId(2)).unwrap().point;
    assert_pt_near(
        snapped_knob_pt,
        200.0,
        200.0,
        1e-4,
        "Dragging within 5.0 dp of a 35° guide knob (200, 200) must snap directly to the knob point",
    );
    assert_eq!(state.active_snappers.len(), 1);
    assert!(
        state.active_snappers[0].is_point_target,
        "Snapping directly to a guide knob or node point must set is_point_target = true"
    );
    pointer_release(&mut state, near_knob_scr, false, false, false, false);
}

// ============================================================================
// TC-V030-005: `Control` Key Magnetic Snap Bypass
// ============================================================================
#[test]
fn test_tc_v030_005_control_key_bypasses_magnetic_snapper() {
    let mut state = GlifoCanvasState::new(GlifoTheme::sonoma_default());
    state.viewport.zoom = 1.0;
    state.viewport.pan_offset = ScreenPoint::new(200.0, 400.0);

    state.contours = vec![vec![
        Node::new(NodeId(1), 100.0, 200.0, NodeType::Corner),
        Node::new(NodeId(2), 250.0, 350.0, NodeType::Corner),
    ]];
    state.sync_paths_from_contours();

    let start_scr = state.viewport.to_screen(FontPoint::new(250.0, 350.0));
    let raw_target = FontPoint::new(102.0, 498.5);
    let raw_scr = state.viewport.to_screen(raw_target);

    // Drag with `ctrl = true`: magnetic snapping MUST be bypassed!
    pointer_press(&mut state, start_scr, false, false, false, true);
    pointer_move(&mut state, raw_scr, false, false, false, true);

    let pt = state.find_node(NodeId(2)).unwrap().point;
    assert_pt_near(pt, 102.0, 498.5, 1e-4, "Ctrl held must bypass magnetic snapping");
    assert!(state.active_snappers.is_empty(), "Ctrl held must keep active_snappers empty");
    pointer_release(&mut state, raw_scr, false, false, false, true);

    // Also verify `PenTool` (`P`) magnetic snapping and `Ctrl` snap bypass (`SPEC-004 §1.2 & TC-V030-005`)
    let mut pen_state = GlifoCanvasState::new(GlifoTheme::sonoma_default());
    pen_state.viewport.zoom = 1.0;
    pen_state.viewport.pan_offset = ScreenPoint::new(200.0, 400.0);
    pen_state.active_tool = grafia::CanvasTool::Pen;
    let _ = pen_state.add_guide_from_selection(false, FontPoint::new(50.0, 250.0));
    let vert_gid = pen_state.add_guide_from_selection(false, FontPoint::new(100.0, 100.0));
    pen_state.set_guide_angle(vert_gid, 90.0);
    pen_state.selected_guide = None;

    // 1. PenTool click at (101.8, 248.8) without Ctrl -> snaps to (100.0, 250.0) (Vertical Guide X = 100, Horizontal Guide Y = 250)!
    let pen_snap_scr = pen_state.viewport.to_screen(FontPoint::new(101.8, 248.8));
    pointer_press(&mut pen_state, pen_snap_scr, false, false, false, false);
    assert!(
        !pen_state.active_snappers.is_empty(),
        "PenTool point placement within snap tolerance must populate active_snappers"
    );
    pointer_release(&mut pen_state, pen_snap_scr, false, false, false, false);
    let pen_contour = pen_state.contours.last().expect("PenTool must have created open contour");
    assert_pt_near(
        pen_contour.last().unwrap().point,
        100.0,
        250.0,
        1e-4,
        "PenTool point placement must magnetically snap to (100.0, 250.0)",
    );
    assert!(
        pen_state.active_snappers.is_empty(),
        "PenTool pointer release must clear active_snappers"
    );

    // 2. PenTool click at (351.7, 248.6) with `ctrl = true` -> bypasses magnetic snap & grid rounding!
    let pen_raw_scr = pen_state.viewport.to_screen(FontPoint::new(351.7, 248.6));
    pointer_press(&mut pen_state, pen_raw_scr, false, false, false, true);
    assert!(
        pen_state.active_snappers.is_empty(),
        "PenTool with Ctrl held must keep active_snappers empty"
    );
    pointer_release(&mut pen_state, pen_raw_scr, false, false, false, true);
    let pen_contour = pen_state.contours.last().unwrap();
    assert_pt_near(
        pen_contour.last().unwrap().point,
        351.7,
        248.6,
        1e-4,
        "PenTool with Ctrl held must bypass magnetic snapping and keep exact cursor point",
    );
    // 3. PenTool magnetic snapping to metrics & nodes even when `guides` and `global_guides` are EMPTY,
    // and subdivision grid rounding (`grid_step_upm = 50.0, grid_subdivisions = 5` -> `10.0 UPM` step).
    let mut pen_no_guides = GlifoCanvasState::new(GlifoTheme::sonoma_default());
    pen_no_guides.viewport.zoom = 1.0;
    pen_no_guides.viewport.pan_offset = ScreenPoint::new(200.0, 550.0);
    pen_no_guides.active_tool = grafia::CanvasTool::Pen;
    pen_no_guides.guides.clear();
    pen_no_guides.global_guides.clear();
    pen_no_guides.grid_step_upm = 50.0;
    pen_no_guides.grid_subdivisions = 5; // effective_grid_step = 10.0 UPM

    // 3a. Click at (112.0, 211.0) (away from metrics/nodes) -> rounds to subdivision grid (110.0, 210.0), NOT main grid (100.0, 200.0)!
    let pen_subgrid_scr = pen_no_guides.viewport.to_screen(FontPoint::new(112.0, 211.0));
    pointer_press(&mut pen_no_guides, pen_subgrid_scr, false, false, false, false);
    pointer_release(&mut pen_no_guides, pen_subgrid_scr, false, false, false, false);
    assert_pt_near(
        pen_no_guides.contours.last().unwrap().last().unwrap().point,
        110.0,
        210.0,
        1e-4,
        "PenTool unsnapped placement must round to effective_grid_step (50 / 5 = 10.0 UPM)",
    );

    // 3b. Click at (111.5, 498.5) with empty guides -> magnetically snaps to Node X = 110.0 and x-Height Y = 500.0!
    let pen_metric_scr = pen_no_guides.viewport.to_screen(FontPoint::new(111.5, 498.5));
    pointer_press(&mut pen_no_guides, pen_metric_scr, false, false, false, false);
    assert!(
        !pen_no_guides.active_snappers.is_empty(),
        "PenTool must magnetically snap to metrics and nodes even when guides are empty"
    );
    pointer_release(&mut pen_no_guides, pen_metric_scr, false, false, false, false);
    assert_pt_near(
        pen_no_guides.contours.last().unwrap().last().unwrap().point,
        110.0,
        500.0,
        1e-4,
        "PenTool with empty guides must snap to (110.0, 500.0)",
    );
}

// ============================================================================
// TC-V030-006: Alignment Zone Bands & Pass 4 Alignment Indicators (Modes 1, 2, 3, 4)
// ============================================================================
#[test]
fn test_tc_v030_006_alignment_zone_bands_and_pass4_node_badges() {
    let mut state = GlifoCanvasState::new(GlifoTheme::sonoma_default());
    state.viewport.zoom = 1.0;
    state.viewport.pan_offset = ScreenPoint::new(200.0, 400.0);

    // Default metrics must include standard alignment zones (Cap Height 700/+16, x-Height 500/+16, Baseline 0/-16, Descender -200/-16)
    assert!(
        state.metrics.alignment_zones.contains(&AlignmentZone { position: 500.0, size: 16.0 }),
        "Default metrics must include x-Height alignment zone (500, +16)"
    );
    assert!(
        state.metrics.alignment_zones.contains(&AlignmentZone { position: 0.0, size: -16.0 }),
        "Default metrics must include Baseline alignment zone (0, -16)"
    );

    // Add a Local Guide at y = 300.0 (0 deg) and a Global Guide at x = 250.0 (90 deg)
    let local_gid = state.add_guide_from_selection(false, FontPoint::new(100.0, 300.0));
    state.find_guide_mut(local_gid).unwrap().angle_deg = 0.0;
    let global_gid = state.add_guide_from_selection(true, FontPoint::new(250.0, 100.0));
    state.find_guide_mut(global_gid).unwrap().angle_deg = 90.0;

    // Check `classify_node_alignment` (`FontMasterMetrics::isOnMetrics:metrics:italicAngle:inLayer:` + guide check):
    // 1. Node at (150, 0.0) -> On Baseline metric line -> Mode 1 (MetricDiamond)
    assert_eq!(
        state.classify_node_alignment(FontPoint::new(150.0, 0.0)),
        Some(AlignmentBadgeMode::OnMetricLine)
    );
    // 2. Node at (150, 500.0) -> On x-Height metric line -> Mode 1 (MetricDiamond)
    assert_eq!(
        state.classify_node_alignment(FontPoint::new(150.0, 500.0)),
        Some(AlignmentBadgeMode::OnMetricLine)
    );
    // 3. Node at (300, 510.0) -> Inside x-Height overshoot zone [500, 516] -> Mode 3 (ZoneCircle)
    assert_eq!(
        state.classify_node_alignment(FontPoint::new(300.0, 510.0)),
        Some(AlignmentBadgeMode::InAlignmentZone)
    );
    // 4. Node at (300, -10.0) -> Inside Baseline overshoot zone [-16, 0] -> Mode 3 (ZoneCircle)
    assert_eq!(
        state.classify_node_alignment(FontPoint::new(300.0, -10.0)),
        Some(AlignmentBadgeMode::InAlignmentZone)
    );
    // 5. Node at (0.0, 220.0) -> On LSB vertical line (x = 0) -> Mode 4 (VerticalMetricRect)
    assert_eq!(
        state.classify_node_alignment(FontPoint::new(0.0, 220.0)),
        Some(AlignmentBadgeMode::OnVerticalMetric)
    );
    // 6. Node at (180.0, 300.0) -> On Local Guide -> Mode 2 (OnLocalGuideDiamond)
    assert_eq!(
        state.classify_node_alignment(FontPoint::new(180.0, 300.0)),
        Some(AlignmentBadgeMode::OnLocalGuide)
    );
    // 7. Node at (250.0, 180.0) -> On Global Guide -> Mode 2 (OnGlobalGuideDiamond)
    assert_eq!(
        state.classify_node_alignment(FontPoint::new(250.0, 180.0)),
        Some(AlignmentBadgeMode::OnGlobalGuide)
    );
    // 8. Node at (180.0, 220.0) -> Interior free point -> None
    assert_eq!(
        state.classify_node_alignment(FontPoint::new(180.0, 220.0)),
        None
    );
    // 9. When `show_metrics == false`, Mode 1, 3, 4 metric/zone badges are suppressed (`None`),
    // while Mode 2 guide badges remain active as long as `show_guides == true`!
    state.show_metrics = false;
    assert_eq!(
        state.classify_node_alignment(FontPoint::new(150.0, 500.0)),
        None,
        "show_metrics = false must suppress Mode 1 metric badges"
    );
    assert_eq!(
        state.classify_node_alignment(FontPoint::new(300.0, 510.0)),
        None,
        "show_metrics = false must suppress Mode 3 alignment zone badges"
    );
    assert_eq!(
        state.classify_node_alignment(FontPoint::new(0.0, 220.0)),
        None,
        "show_metrics = false must suppress Mode 4 vertical metric badges"
    );
    assert_eq!(
        state.classify_node_alignment(FontPoint::new(180.0, 300.0)),
        Some(AlignmentBadgeMode::OnLocalGuide),
        "show_metrics = false must preserve Mode 2 guide badges when show_guides = true"
    );
    state.show_metrics = true;
}

// ============================================================================
// TC-V030-007: `egui` <-> `headless` Visual Parity & Full Undo/Redo Bit-Exactness
// ============================================================================
#[test]
fn test_tc_v030_007_headless_rasterization_and_undo_redo_bit_exactness() {
    let mut state = GlifoCanvasState::new(GlifoTheme::sonoma_default());
    state.load_preset_o();
    state.viewport.zoom = 0.75;
    state.viewport.pan_offset = ScreenPoint::new(160.0, 480.0);

    // Add a Local Guide with measurement mode and a Global Guide
    let gid_local = state.add_guide_from_selection(false, FontPoint::new(300.0, 250.0));
    state.set_guide_show_measurement(gid_local, true);
    let gid_global = state.add_guide_from_selection(true, FontPoint::new(300.0, 250.0));
    state.set_guide_angle(gid_global, 90.0);

    // Verify Undo/Redo restores guide state bit-for-bit
    assert_eq!(state.guides.len(), 1);
    assert_eq!(state.global_guides.len(), 1);
    assert!(state.undo()); // Undo set_guide_angle(90.0)
    assert!((state.find_guide(gid_global).unwrap().angle_deg - 0.0).abs() < 1e-6);
    assert!(state.redo());
    assert!((state.find_guide(gid_global).unwrap().angle_deg - 90.0).abs() < 1e-6);

    // Rasterize via headless software renderer and verify non-empty visual primitives for guides, zones, and badges
    let pixmap = rasterize_canvas_subregion(&state, &state.theme, 800, 600);
    let img = pixmap.data();
    assert_eq!(img.len(), 800 * 600 * 4);

    // Verify Local Guide Blue (#3E4CCFB3 blended on #FAFAFA -> ~(98, 108, 221)) and Global Guide Red (#CF4F3EB3 blended on #FAFAFA -> ~(220, 111, 98))
    let mut saw_local_guide_blue = false;
    let mut saw_global_guide_red = false;
    for px in img.as_chunks::<4>().0 {
        if px[2] > 180 && px[0] < 130 && px[1] < 140 {
            saw_local_guide_blue = true;
        }
        if px[0] > 180 && px[1] < 135 && px[2] < 120 && px[0] > px[1] + 70 {
            saw_global_guide_red = true;
        }
    }
    assert!(saw_local_guide_blue, "Headless rasterizer must render Local Guide blue pixels");
    assert!(saw_global_guide_red, "Headless rasterizer must render Global Guide red pixels");

    // Verify `lock_guides = true` renders padlock shackle above padlock body in headless tiny_skia
    state.lock_guides = true;
    let pix_locked = rasterize_canvas_subregion(&state, &state.theme, 800, 600);
    assert_ne!(
        img,
        pix_locked.data(),
        "Enabling lock_guides must render padlock icons (body + shackle) in headless tiny_skia"
    );
    state.lock_guides = false;

    // Save Frame 44 (Local, Global & Measurement Guides + Alignment Zone Bands on /o)
    let out_dir = std::path::PathBuf::from(
        r"C:\Users\jhove\.gemini\antigravity-cli\brain\fccb9142-9217-4702-a2ca-95303b541ee0",
    );
    if out_dir.exists() {
        let img44 = grafia::adapters::headless::pixmap_to_rgba_image(pixmap);
        let _ = img44.save(out_dir.join("frame_44_v030_local_global_and_measurement_guides.png"));

        // Build Frame 45 (Active Magnetic Snappers `#993333FF` + Modes 1..4 Alignment Badges on /n)
        let mut state45 = GlifoCanvasState::new(GlifoTheme::sonoma_default());
        state45.load_preset_n();
        state45.viewport.zoom = 0.78;
        state45.viewport.pan_offset = ScreenPoint::new(150.0, 490.0);
        let gid_diag = state45.add_guide_from_selection(false, FontPoint::new(280.0, 250.0));
        state45.set_guide_angle(gid_diag, 35.0);
        state45.active_snappers = vec![
            grafia::ActiveSnapper {
                from: FontPoint::new(70.0, 0.0),
                to: FontPoint::new(70.0, 500.0),
                angle_deg: Some(90.0),
                is_point_target: false,
            },
            grafia::ActiveSnapper {
                from: FontPoint::new(0.0, 500.0),
                to: FontPoint::new(490.0, 500.0),
                angle_deg: Some(0.0),
                is_point_target: false,
            },
        ];
        let pixmap45 = rasterize_canvas_subregion(&state45, &state45.theme, 800, 600);
        let img45 = grafia::adapters::headless::pixmap_to_rgba_image(pixmap45);
        let _ = img45.save(out_dir.join("frame_45_v030_magnetic_snapper_and_alignment_zone_badges.png"));
    }
}

// ============================================================================
// TC-V030-008: Canvas Grid (`PathRenderer::drawGrid` @ `spec_ref` / `arm64 spec_ref`
// & `GridSettings` @ `spec_ref`): Main Grid, Subdivisions, Sloped Grid & Zoom Thresholds
// ============================================================================
#[test]
fn test_tc_v030_008_canvas_grid_main_subdivisions_slope_and_zoom_thresholds() {
    let mut state = GlifoCanvasState::new(GlifoTheme::sonoma_default());
    state.contours.clear();
    state.sync_paths_from_contours();
    state.show_metrics = false;
    state.show_guides = false;
    state.viewport.pan_offset = ScreenPoint::new(200.0, 300.0);

    // 1. Verify exact empirical Reference Vector Grid Color Tokens (`ThemeColor` & `ThemeColor`)
    let c = &state.theme.edit_canvas;
    assert_eq!(
        (c.grid_main_color.r, c.grid_main_color.g, c.grid_main_color.b, c.grid_main_color.a),
        (128, 115, 102, 102),
        "_GridMainColorDefault (0.5, 0.45, 0.4, 0.4) -> #80736666"
    );
    assert_eq!(
        (c.grid_sub_color.r, c.grid_sub_color.g, c.grid_sub_color.b, c.grid_sub_color.a),
        (64, 89, 89, 64),
        "_GridSubColorDefault (0.25, 0.35, 0.35, 0.25) -> #40595940"
    );

    // 2. Default `1.0 UPM` grid at `zoom = 1.0`: `main_step_px = 1.0 <= 8.0 dp` -> grid is suppressed!
    state.show_grid = true;
    state.grid_step_upm = 1.0;
    state.grid_subdivisions = 1;
    state.viewport.zoom = 1.0;
    let pix_suppressed = rasterize_canvas_subregion(&state, &state.theme, 400, 300);
    let non_bg_suppressed = pix_suppressed
        .data()
        .as_chunks::<4>()
        .0
        .iter()
        .filter(|px| px[0] != 255 || px[1] != 255 || px[2] != 255)
        .count();
    assert_eq!(
        non_bg_suppressed, 0,
        "1 UPM grid at zoom=1.0 (1.0 dp <= 8.0 dp threshold) must not draw grid lines"
    );

    // 3. Zoom in to `zoom = 12.0` (`main_step_px = 12.0 > 8.0 dp`) -> main grid lines are drawn!
    state.viewport.zoom = 12.0;
    let pix_zoomed = rasterize_canvas_subregion(&state, &state.theme, 400, 300);
    let non_bg_zoomed = pix_zoomed
        .data()
        .as_chunks::<4>()
        .0
        .iter()
        .filter(|px| px[0] != 255 || px[1] != 255 || px[2] != 255)
        .count();
    assert!(
        non_bg_zoomed > 1000,
        "1 UPM grid at zoom=12.0 (12.0 dp > 8.0 dp threshold) must render main grid lines"
    );

    // 4. Modular grid: `grid_step_upm = 50.0`, `grid_subdivisions = 5` at `zoom = 1.0`
    // -> `effective_grid_step() == 10.0 UPM` (`grid metrics` `gridMain / gridSubDivision`)
    // -> `main_step_px = 50.0 > 8.0 dp` AND `sub_step_px = 10.0 > 4.0 dp`: both main and subdivision grids render!
    state.viewport.zoom = 1.0;
    state.grid_step_upm = 50.0;
    state.grid_subdivisions = 5;
    assert!(
        (state.effective_grid_step() - 10.0).abs() < 1e-6,
        "effective_grid_step must equal grid_step_upm / grid_subdivisions (50 / 5 = 10.0)"
    );

    let pix_modular = rasterize_canvas_subregion(&state, &state.theme, 400, 300);
    // Check a pixel on a main grid line (x = 200 + 50 = 250, y = 125) vs subdivision grid line (x = 200 + 10 = 210, y = 125)
    let idx_main = (125 * 400 + 250) * 4;
    let idx_sub = (125 * 400 + 210) * 4;
    let px_main = &pix_modular.data()[idx_main..idx_main + 4];
    let px_sub = &pix_modular.data()[idx_sub..idx_sub + 4];
    assert!(
        px_main[0] < 255 && px_main[2] < px_main[0],
        "Main grid line at x=250 must have warm brown-grey tint (#80736666), got {px_main:?}"
    );
    assert!(
        px_sub[0] < 255 && px_sub[1] >= px_sub[0],
        "Subdivision grid line at x=210 must have cool teal-grey tint (#40595940), got {px_sub:?}"
    );

    // 5. Sloped grid (`grid_slope_rise = 200`, `grid_slope_run = 50` -> dx/dy = +0.25)
    // and backslanted grid (`grid_slope_rise = 200`, `grid_slope_run = -50` -> dx/dy = -0.25)
    state.grid_slope_rise = 200;
    state.grid_slope_run = 50;
    let pix_sloped = rasterize_canvas_subregion(&state, &state.theme, 400, 300);
    assert_ne!(
        pix_modular.data(),
        pix_sloped.data(),
        "Sloped grid (slope_rise=200, slope_run=50) must shear vertical grid lines"
    );
    state.grid_slope_run = -50;
    let pix_backslanted = rasterize_canvas_subregion(&state, &state.theme, 400, 300);
    assert_ne!(
        pix_modular.data(),
        pix_backslanted.data(),
        "Backslanted grid (slope_rise=200, slope_run=-50) must shear vertical grid lines (`testl %edi, %edi` != 0 parity)"
    );
    assert_ne!(
        pix_sloped.data(),
        pix_backslanted.data(),
        "Backslanted grid (-50) must shear in the opposite direction of forward-slanted grid (+50)"
    );

    // 6. Toggling `show_grid = false` hides the grid completely
    state.show_grid = false;
    let pix_hidden = rasterize_canvas_subregion(&state, &state.theme, 400, 300);
    let non_bg_hidden = pix_hidden
        .data()
        .as_chunks::<4>()
        .0
        .iter()
        .filter(|px| px[0] != 255 || px[1] != 255 || px[2] != 255)
        .count();
    assert_eq!(non_bg_hidden, 0, "show_grid = false must suppress all grid rendering");

    // Save Frame 46 (Canvas Grid Main + Subdivisions + Guides + Glyph /o)
    let out_dir = std::path::PathBuf::from(
        r"C:\Users\jhove\.gemini\antigravity-cli\brain\fccb9142-9217-4702-a2ca-95303b541ee0",
    );
    if out_dir.exists() {
        let mut state46 = GlifoCanvasState::new(GlifoTheme::sonoma_default());
        state46.load_preset_o();
        state46.viewport.zoom = 0.78;
        state46.viewport.pan_offset = ScreenPoint::new(150.0, 490.0);
        state46.show_grid = true;
        state46.grid_step_upm = 100.0;
        state46.grid_subdivisions = 5; // 100 UPM main grid, 20 UPM subdivision grid
        let gid = state46.add_guide_from_selection(false, FontPoint::new(300.0, 250.0));
        state46.set_guide_show_measurement(gid, true);
        let pixmap46 = rasterize_canvas_subregion(&state46, &state46.theme, 800, 600);
        let img46 = grafia::adapters::headless::pixmap_to_rgba_image(pixmap46);
        let _ = img46.save(out_dir.join("frame_46_v030_canvas_grid_main_and_subdivisions.png"));
    }
}

/// `TC-V030-009`: 3-Way HostApp Appearance Switch (`AppearanceMode::{Light, Dark, System}`)
/// and Empirical Reference Vector (``) Dark Mode Rendering Parity.
#[test]
fn test_tc_v030_009_appearance_mode_light_dark_system_and_dark_canvas_parity() {
    use grafia::AppearanceMode;

    // 1. Verify empirical Reference Vector Dark Mode tokens vs Light Mode tokens
    let light = GlifoTheme::sonoma_default();
    let dark = GlifoTheme::sonoma_dark();
    assert!(!light.is_dark(), "sonoma_default must report is_dark() == false");
    assert!(dark.is_dark(), "sonoma_dark must report is_dark() == true");

    // Canvas background: Light #FFFFFF vs Dark #1E1E1E (`NSColor::darkContentBackgroundColor` @ spec_ref)
    assert_eq!(
        light.canvas_background_color32(),
        egui::Color32::from_rgb(0xFF, 0xFF, 0xFF)
    );
    assert_eq!(
        dark.canvas_background_color32(),
        egui::Color32::from_rgb(0x1E, 0x1E, 0x1E)
    );
    assert_eq!(
        dark.sidebar_background_color32(),
        egui::Color32::from_rgb(0x26, 0x26, 0x26)
    );
    assert_eq!(
        dark.infobox_background_color32(),
        egui::Color32::from_rgb(0x2C, 0x2C, 0x2E)
    );
    assert_eq!(
        dark.splitter_color32(),
        egui::Color32::from_rgb(0x38, 0x38, 0x3A)
    );

    // Foreground stroke & fill: Light #1D1D1F vs Dark #FFFFFF (`ThemeColor` @ spec_ref)
    assert_eq!(
        dark.contour_stroke_color32(),
        egui::Color32::from_rgb(0xFF, 0xFF, 0xFF)
    );
    assert_eq!(
        dark.preview_fill_color32(),
        egui::Color32::from_rgb(0xFF, 0xFF, 0xFF)
    );

    // Corner node (`ThemeColor` @ spec_ref: #5693E0) & Smooth node (`ThemeColor` @ spec_ref: #52D189)
    assert_eq!(
        dark.corner_node_color32(),
        egui::Color32::from_rgb(0x56, 0x93, 0xE0)
    );
    assert_eq!(
        dark.smooth_node_color32(),
        egui::Color32::from_rgb(0x52, 0xD1, 0x89)
    );
    // Off-curve BCP handle (`_OffCurvePointColorDefaultDark` @ spec_ref: #AC8E68)
    assert_eq!(
        dark.bcp_handle_color32(),
        egui::Color32::from_rgb(0xAC, 0x8E, 0x68)
    );

    // 2. Verify 3-way `AppearanceMode::{Light, Dark, System}` resolution
    assert!(!GlifoTheme::for_appearance(AppearanceMode::Light, false).is_dark());
    assert!(
        !GlifoTheme::for_appearance(AppearanceMode::Light, true).is_dark(),
        "Explicit AppearanceMode::Light must stay Light even when OS system_is_dark == true"
    );
    assert!(
        GlifoTheme::for_appearance(AppearanceMode::Dark, false).is_dark(),
        "Explicit AppearanceMode::Dark must stay Dark even when OS system_is_dark == false"
    );
    assert!(GlifoTheme::for_appearance(AppearanceMode::Dark, true).is_dark());
    assert!(
        !GlifoTheme::for_appearance(AppearanceMode::System, false).is_dark(),
        "AppearanceMode::System must resolve to Light when OS system_is_dark == false"
    );
    assert!(
        GlifoTheme::for_appearance(AppearanceMode::System, true).is_dark(),
        "AppearanceMode::System must resolve to Dark when OS system_is_dark == true"
    );

    // 3. Verify `GlifoCanvasState` state transitions for `Light`, `Dark`, and `System`
    let mut state = GlifoCanvasState::new(GlifoTheme::sonoma_default());
    assert_eq!(state.appearance_mode, AppearanceMode::Light);
    assert!(!state.theme.is_dark());

    // Switch to explicit Dark mode
    state.set_appearance_mode(AppearanceMode::Dark);
    assert_eq!(state.appearance_mode, AppearanceMode::Dark);
    assert!(state.theme.is_dark());

    // Switch to System mode with OS in Light mode -> resolves to Light
    state.set_appearance_with_system(AppearanceMode::System, false);
    assert_eq!(state.appearance_mode, AppearanceMode::System);
    assert!(!state.system_dark_mode);
    assert!(!state.theme.is_dark());

    // OS switches to Dark mode while in AppearanceMode::System -> automatically flips theme to Dark
    state.set_system_dark_mode(true);
    assert!(state.system_dark_mode);
    assert!(state.theme.is_dark());

    // While OS is Dark, user explicitly picks Light in HostApp Preferences -> forces Light
    state.set_appearance_mode(AppearanceMode::Light);
    assert_eq!(state.appearance_mode, AppearanceMode::Light);
    assert!(!state.theme.is_dark());

    // Switching glyph archetypes (including back to GlyphArchetype::A) must preserve AppearanceMode and theme
    state.set_appearance_with_system(AppearanceMode::Dark, false);
    state.load_glyph_archetype(grafia::tokens::GlyphArchetype::O);
    assert_eq!(state.appearance_mode, AppearanceMode::Dark);
    assert!(state.theme.is_dark());
    state.load_glyph_archetype(grafia::tokens::GlyphArchetype::A);
    assert_eq!(
        state.appearance_mode,
        AppearanceMode::Dark,
        "load_glyph_archetype(GlyphArchetype::A) must preserve AppearanceMode::Dark"
    );
    assert!(
        state.theme.is_dark(),
        "load_glyph_archetype(GlyphArchetype::A) must preserve Dark GlifoTheme"
    );

    // 4. Verify `egui::Context` automatic synchronization & `GlifoCanvasWidget::from_state` / `new`
    state.set_appearance_mode(AppearanceMode::System);
    let ctx = egui::Context::default();
    ctx.set_visuals(egui::Visuals::light());
    let mut out1 = ctx.run_ui(egui::RawInput::default(), |ui| {
        egui::CentralPanel::default().show(ui, |ui| {
            grafia::GlifoCanvasWidget::from_state(&mut state).show(ui);
        });
    });
    out1.textures_delta.clear();
    assert!(
        !state.theme.is_dark(),
        "When AppearanceMode::System is active and egui Visuals::light() is used, state.theme must sync to Light"
    );

    ctx.set_visuals(egui::Visuals::dark());
    let mut out2 = ctx.run_ui(egui::RawInput::default(), |ui| {
        egui::CentralPanel::default().show(ui, |ui| {
            grafia::GlifoCanvasWidget::from_state(&mut state).show(ui);
        });
    });
    out2.textures_delta.clear();
    assert!(
        state.theme.is_dark(),
        "When AppearanceMode::System is active and egui Visuals::dark() is used, state.theme must sync to Dark"
    );

    // Verify that when explicit `AppearanceMode::Dark` is set on `state`, `GlifoCanvasWidget::new(&mut state, &light)`
    // still honors `state.theme` (Dark) AND emits the #1E1E1E canvas background rect into egui::Painter!
    ctx.set_visuals(egui::Visuals::light());
    state.set_appearance_mode(AppearanceMode::Dark);
    let mut out_explicit_dark = ctx.run_ui(egui::RawInput::default(), |ui| {
        egui::CentralPanel::default().show(ui, |ui| {
            grafia::GlifoCanvasWidget::new(&mut state, &light).show(ui);
        });
    });
    out_explicit_dark.textures_delta.clear();
    let has_dark_bg_rect = out_explicit_dark.shapes.iter().any(|cs| match &cs.shape {
        egui::Shape::Rect(r) => r.fill == egui::Color32::from_rgb(0x1E, 0x1E, 0x1E),
        _ => false,
    });
    assert!(
        has_dark_bg_rect,
        "egui CanvasPainter in AppearanceMode::Dark must emit #1E1E1E canvas background rect even when caller passes &light to GlifoCanvasWidget::new"
    );

    // Verify ShapeLocatorStyle::for_dark_mode(true) uses white-alpha context silhouette for dark sidebar contrast
    let loc_dark = grafia::ShapeLocatorStyle::for_dark_mode(true);
    let loc_light = grafia::ShapeLocatorStyle::for_dark_mode(false);
    assert_ne!(
        loc_dark.context_color, loc_light.context_color,
        "ShapeLocatorStyle::for_dark_mode(true) must use white-alpha context color instead of black-alpha"
    );
    assert_eq!(loc_dark.context_color.to_srgba_unmultiplied()[0], 255);

    // 5. Verify Headless `tiny_skia` and `egui` Dark Mode rendering across all v0.1..v0.3 layers
    let mut dark_state = GlifoCanvasState::new(GlifoTheme::sonoma_dark());
    dark_state.load_preset_o();
    dark_state.viewport.zoom = 0.78;
    dark_state.viewport.pan_offset = ScreenPoint::new(420.0, 460.0);
    dark_state.show_grid = true;
    dark_state.grid_step_upm = 100.0;
    dark_state.grid_subdivisions = 5;
    dark_state.show_metrics = true;
    dark_state.show_guides = true;
    dark_state.show_bounding_box = true;
    let gid_local = dark_state.add_guide_from_selection(false, FontPoint::new(300.0, 250.0));
    dark_state.set_guide_show_measurement(gid_local, true);
    let gid_global = dark_state.add_guide_from_selection(true, FontPoint::new(300.0, 400.0));
    dark_state.set_guide_angle(gid_global, 15.0);
    dark_state.selected_nodes.push(NodeId(1));
    dark_state.selected_nodes.push(NodeId(4));

    let pix_dark = rasterize_canvas_subregion(&dark_state, &dark_state.theme, 1280, 800);
    // Top-left corner (x=3, y=3, away from 100 UPM grid lines) must be dark #1E1E1E (30, 30, 30)
    let idx = (3 * 1280 + 3) * 4;
    let corner_px = &pix_dark.data()[idx..idx + 4];
    assert_eq!(
        corner_px,
        &[30, 30, 30, 255],
        "Dark mode canvas background pixel must be #1E1E1E (30, 30, 30, 255)"
    );

    let full_pix = grafia::adapters::headless::rasterize_full_window(
        &dark_state,
        &dark_state.theme,
    );
    // Verify that EVERY pixel in the left Sidebar (x in 10..240, y in 10..800) is pure #262626 (38, 38, 38)
    // with ZERO grid lines or infinite guide lines bleeding across the sidebar!
    for y in (10..800).step_by(5) {
        let sp = full_pix.pixel(100, y).expect("sidebar pixel");
        assert_eq!(
            (sp.red(), sp.green(), sp.blue()),
            (0x26, 0x26, 0x26),
            "Sidebar pixel at (100, {y}) must be #262626 with zero grid/guide bleed"
        );
        let ip = full_pix.pixel(1100, y).expect("infobox pixel");
        assert_eq!(
            (ip.red(), ip.green(), ip.blue()),
            (0x2C, 0x2C, 0x2E),
            "InfoBox pixel at (1100, {y}) must be #2C2C2E with zero grid/guide bleed"
        );
    }

    // Save Frame 47 (Full Window Dark Mode Canvas with Sidebar, InfoBox, Preview Strip & v0.1..v0.3 overlays)
    let out_dir = std::path::PathBuf::from(
        r"C:\Users\jhove\.gemini\antigravity-cli\brain\fccb9142-9217-4702-a2ca-95303b541ee0",
    );
    if out_dir.exists() {
        let img47 = grafia::adapters::headless::pixmap_to_rgba_image(full_pix);
        let _ = img47.save(out_dir.join("frame_47_v030_dark_mode_full_canvas.png"));
    }
}
