//! Normative COTS Verification Suite for `SPEC-005` (`glifo v0.4.0`):
//! Specialized Edit Tools (`KnifeTool`, `PrimitivesTool`, `MeasurementTool`, `AnnotationTool`,
//! `PencilTool`, `PixelTool`), Coordinate & Segment Delta Overlays, and Canvas Annotations
//! (`TC-V040-001` .. `TC-V040-009`).
//!
//! Grounded 100% in Live Reference Vector reference specification & reference Runtime Truth.

use egui::{Event, Key, Modifiers as EguiModifiers, PointerButton, Pos2, RawInput, Rect, Vec2};
use grafia::adapters::headless::rasterize_canvas_subregion;
use grafia::adapters::input_dispatcher::InputDispatcher;
use grafia::{
    AnnotationId, AnnotationKind, AnnotationSubTool, AppearanceMode, CanvasAnnotation, CanvasTool,
    FontPoint, GlifoCanvasState, GlifoTheme, Node, NodeId, NodeType, PrimitiveSubTool, ScreenPoint,
    ToolDialogRequest,
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

fn press_key(
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

fn init_single_box_state() -> GlifoCanvasState {
    let mut state = GlifoCanvasState::new(GlifoTheme::sonoma_default());
    state.viewport.zoom = 0.5;
    state.viewport.pan_offset = ScreenPoint::new(100.0, 500.0);
    let rect_nodes = vec![
        Node::new(NodeId(1), 100.0, 100.0, NodeType::Corner),
        Node::new(NodeId(2), 500.0, 100.0, NodeType::Corner),
        Node::new(NodeId(3), 500.0, 500.0, NodeType::Corner),
        Node::new(NodeId(4), 100.0, 500.0, NodeType::Corner),
    ];
    state.next_node_id = 5;
    state.contours = vec![rect_nodes.clone()];
    state.glyph.contours = vec![grafia::GlyphContour::new(grafia::ContourId(0), rect_nodes)];
    state.active_contour_closed = true;
    state.sync_paths_from_contours();
    state
}

// ============================================================================
// TC-V040-001: KnifeTool (`E`) Pairwise Closed-Contour Slicing & Shift Constrain
// ============================================================================
#[test]
fn tc_v040_001_knife_tool_cuts_closed_contour_into_two_halves_and_constrains_with_shift() {
    let mut state = init_single_box_state();
    assert_eq!(state.contours.len(), 1);

    // Press 'E' to activate KnifeTool (`-[CanvasTool trigger] = @"e"`)
    press_key(&mut state, Key::E, false, false, false, false);
    assert_eq!(state.active_tool, CanvasTool::Knife);

    // Drag across the box from (50, 300) to (550, 315) with Shift held -> constrained horizontally at y=300
    let start_scr = state.viewport.to_screen(FontPoint::new(50.0, 300.0));
    let drag_scr = state.viewport.to_screen(FontPoint::new(550.0, 315.0));

    pointer_press(&mut state, start_scr, true, false, false, false);
    pointer_move(&mut state, drag_scr, true, false, false, false);

    // While dragging, KnifeState must be active, constrained to y=300, and have 2 contour intersections at x=100 and x=500
    assert!(state.v040.knife.active);
    assert!((state.v040.knife.current_upm.y.0 - 300.0).abs() < 1e-3);
    assert_eq!(state.v040.knife.intersections.len(), 2);
    assert!((state.v040.knife.intersections[0].x.0 - 100.0).abs() < 1.0);
    assert!((state.v040.knife.intersections[1].x.0 - 500.0).abs() < 1.0);

    // Release pointer to commit cut (`CanvasTool::cutPathsInLayer:forPoint:endPoint:`)
    pointer_release(&mut state, drag_scr, true, false, false, false);
    assert!(!state.v040.knife.active);

    // Closed rectangle is sliced into 2 closed contours (bottom half y in [100, 300], top half y in [300, 500])
    assert_eq!(state.contours.len(), 2);
    assert!(state.is_contour_closed(0));
    assert!(state.is_contour_closed(1));
    assert_eq!(state.contours[0].len(), 4);
    assert_eq!(state.contours[1].len(), 4);

    // Undo / Redo bit-exact roundtrip
    assert!(state.undo());
    assert_eq!(state.contours.len(), 1);
    assert_eq!(state.contours[0].len(), 4);
    assert!(state.redo());
    assert_eq!(state.contours.len(), 2);
}

// ============================================================================
// TC-V040-002: KnifeTool `Option`/`Alt` (`addNodesInLayer:`) & 1-Hit `dividePathAtNode:`
// ============================================================================
#[test]
fn tc_v040_002_knife_tool_alt_adds_nodes_without_slicing_and_single_hit_opens_contour() {
    let mut state = init_single_box_state();
    press_key(&mut state, Key::E, false, false, false, false);

    // Part A: Option/Alt (`addNodesInLayer:forPoint:endPoint:`) inserts nodes at both hits without splitting
    let start_scr = state.viewport.to_screen(FontPoint::new(50.0, 250.0));
    let end_scr = state.viewport.to_screen(FontPoint::new(550.0, 250.0));
    pointer_press(&mut state, start_scr, false, true, false, false);
    pointer_move(&mut state, end_scr, false, true, false, false);
    pointer_release(&mut state, end_scr, false, true, false, false);

    assert_eq!(state.contours.len(), 1, "Alt+Knife must not split contour");
    assert_eq!(state.contours[0].len(), 6, "2 OnCurve nodes inserted at x=100,y=250 and x=500,y=250");
    assert_eq!(state.selected_nodes.len(), 2);

    // Undo back to 4-node rectangle
    assert!(state.undo());
    assert_eq!(state.contours[0].len(), 4);

    // Part B: Single intersection (`count == 3` -> `dividePathAtNode:`) opens the closed contour at the hit point
    let in_scr = state.viewport.to_screen(FontPoint::new(300.0, 300.0));
    let out_scr = state.viewport.to_screen(FontPoint::new(550.0, 300.0));
    pointer_press(&mut state, in_scr, false, false, false, false);
    pointer_move(&mut state, out_scr, false, false, false, false);
    assert_eq!(state.v040.knife.intersections.len(), 1);
    pointer_release(&mut state, out_scr, false, false, false, false);

    assert_eq!(state.contours.len(), 1);
    assert!(!state.is_contour_closed(0), "Single knife hit on closed contour must divide & open it");
    let first = state.contours[0].first().unwrap().point;
    let last = state.contours[0].last().unwrap().point;
    assert!((first.x.0 - 500.0).abs() < 1.0 && (first.y.0 - 300.0).abs() < 1.0);
    assert!((last.x.0 - 500.0).abs() < 1.0 && (last.y.0 - 300.0).abs() < 1.0);
}

// ============================================================================
// TC-V040-003: PrimitivesTool (`F` / `Shift+F`) Rect & Circle, Modifiers & Dialog
// ============================================================================
#[test]
fn tc_v040_003_primitives_tool_rect_and_circle_modifiers_and_single_click_dialog() {
    let mut state = GlifoCanvasState::new(GlifoTheme::sonoma_default());
    state.viewport.zoom = 0.5;
    state.viewport.pan_offset = ScreenPoint::new(100.0, 500.0);
    state.active_snappers.clear();

    // Press 'F' -> activates PrimitivesTool (Rectangle sub-tool by default)
    press_key(&mut state, Key::F, false, false, false, false);
    assert_eq!(state.active_tool, CanvasTool::Primitives);
    assert_eq!(state.v040.primitive_sub_tool, PrimitiveSubTool::Rectangle);

    // 1. Drag with Shift (square constraint: width == height == max(|dx|, |dy|))
    let p0 = state.viewport.to_screen(FontPoint::new(100.0, 100.0));
    let p1 = state.viewport.to_screen(FontPoint::new(300.0, 220.0));
    pointer_press(&mut state, p0, true, false, false, true);
    pointer_move(&mut state, p1, true, false, false, true);
    assert!(state.v040.primitive_drag.active);
    let rect = state.v040.primitive_drag.preview_rect_upm();
    assert!(((rect.2 - rect.0) - 200.0).abs() < 1.0);
    assert!(((rect.3 - rect.1) - 200.0).abs() < 1.0);
    pointer_release(&mut state, p1, true, false, false, true);

    assert_eq!(state.contours.len(), 1);
    assert_eq!(state.contours[0].len(), 4);
    assert!(state.contours[0].iter().all(|n| n.node_type == NodeType::Corner));

    // 2. Press Shift+F -> cycles to Circle sub-tool (`CanvasTool`)
    press_key(&mut state, Key::F, true, false, false, false);
    assert_eq!(state.v040.primitive_sub_tool, PrimitiveSubTool::Circle);

    // Drag with Alt (center origin: expands symmetrically around start (400, 400) by +-100 -> [300..500, 300..500])
    let c0 = state.viewport.to_screen(FontPoint::new(400.0, 400.0));
    let c1 = state.viewport.to_screen(FontPoint::new(500.0, 500.0));
    pointer_press(&mut state, c0, false, true, false, true);
    pointer_move(&mut state, c1, false, true, false, true);
    let crect = state.v040.primitive_drag.preview_rect_upm();
    assert!((crect.0 - 300.0).abs() < 1.0 && (crect.2 - 500.0).abs() < 1.0);
    assert!((crect.1 - 300.0).abs() < 1.0 && (crect.3 - 500.0).abs() < 1.0);
    pointer_release(&mut state, c1, false, true, false, true);

    assert_eq!(state.contours.len(), 2);
    // Cubic Bézier circle has 4 Smooth extreme nodes + 8 OffCurve kappa handles = 12 nodes
    assert_eq!(state.contours[1].len(), 12);
    let smooth_count = state.contours[1].iter().filter(|n| n.node_type == NodeType::Smooth).count();
    let offcurve_count = state.contours[1].iter().filter(|n| n.node_type == NodeType::OffCurve).count();
    assert_eq!(smooth_count, 4);
    assert_eq!(offcurve_count, 8);

    // 3. Single click without drag emits `ToolDialogRequest::Primitive` (`makePrimitiveWithDialog:`)
    let click_pt = state.viewport.to_screen(FontPoint::new(250.0, 250.0));
    pointer_press(&mut state, click_pt, false, false, false, true);
    pointer_release(&mut state, click_pt, false, false, false, true);
    assert!(matches!(
        state.v040.last_dialog_request,
        Some(ToolDialogRequest::Primitive {
            sub_tool: PrimitiveSubTool::Circle,
            default_width: 100.0,
            default_height: 100.0,
            ..
        })
    ));

    // Programmatic dialog confirmation `insert_primitive` creates the primitive with Undo support
    assert!(state.insert_primitive(PrimitiveSubTool::Rectangle, FontPoint::new(250.0, 250.0), 120.0, 80.0));
    assert_eq!(state.contours.len(), 3);
    assert!(state.undo());
    assert_eq!(state.contours.len(), 2);
}

// ============================================================================
// TC-V040-004: MeasurementTool (`L` & `Cmd+Ctrl+Opt`) Coordinate & Delta Labels
// ============================================================================
#[test]
fn tc_v040_004_measurement_tool_coordinate_labels_and_temporary_modifier_override() {
    let mut state = GlifoCanvasState::init_glyph_o();
    state.italic_angle_deg = 12.0;
    assert_eq!(state.active_tool, CanvasTool::Select);

    // 1. Temporary activation via Cmd+Ctrl+Opt (`tempTrigger = spec_ref`)
    pointer_move(
        &mut state,
        ScreenPoint::new(300.0, 300.0),
        false,
        true,
        true,
        true,
    );
    assert_eq!(
        state.active_tool,
        CanvasTool::Measurement,
        "Cmd+Ctrl+Opt (spec_ref) must temporarily activate MeasurementTool"
    );

    // Release modifiers -> restores Select tool
    pointer_move(
        &mut state,
        ScreenPoint::new(300.0, 300.0),
        false,
        false,
        false,
        false,
    );
    assert_eq!(state.active_tool, CanvasTool::Select);

    // 2. Permanent activation via 'L' (`-[CanvasTool trigger] = @"l"`)
    press_key(&mut state, Key::L, false, false, false, false);
    assert_eq!(state.active_tool, CanvasTool::Measurement);

    // Compute coordinate & delta labels (`drawLocationsForLayer:origin:scale:shouldDrawDark:`)
    let labels = state.compute_measurement_layer_labels();
    assert!(
        !labels.oncurve_labels.is_empty(),
        "OnCurve nodes must produce Blue coordinate labels"
    );
    assert!(
        !labels.offcurve_labels.is_empty(),
        "OffCurve BCP handles must produce Green coordinate labels"
    );
    assert!(
        !labels.segment_deltas.is_empty(),
        "Segments must produce Red italic-compensated (dx, dy) midpoint labels"
    );
}

// ============================================================================
// TC-V040-005: MeasurementTool Ray Intersections, Sidebearings, Angle & 'G' Guide
// ============================================================================
#[test]
fn tc_v040_005_measurement_ray_intersections_sidebearings_angle_and_promote_to_guide() {
    let mut state = init_single_box_state();
    state.glyph.width = 600.0;
    press_key(&mut state, Key::L, false, false, false, false);

    // Drag horizontally from (-50, 300) across LSB (x=0), left edge (x=100), right edge (x=500), RSB (x=600) to (650, 300)
    let p_start = state.viewport.to_screen(FontPoint::new(-50.0, 300.0));
    let p_end = state.viewport.to_screen(FontPoint::new(650.0, 300.0));

    pointer_press(&mut state, p_start, false, false, false, false);
    pointer_move(&mut state, p_end, false, false, false, false);

    assert!(state.v040.measurement.active);
    let hits = &state.v040.measurement.intersections;
    // Ray hits: start (-50), LSB (0), contour left (100), contour right (500), RSB (600), end (650)
    let xs: Vec<f64> = hits.iter().map(|p| p.x.0).collect();
    assert!(xs.iter().any(|&x| (x - 0.0).abs() < 1.0), "Must intersect LSB x=0: {xs:?}");
    assert!(xs.iter().any(|&x| (x - 100.0).abs() < 1.0), "Must intersect contour x=100: {xs:?}");
    assert!(xs.iter().any(|&x| (x - 500.0).abs() < 1.0), "Must intersect contour x=500: {xs:?}");
    assert!(xs.iter().any(|&x| (x - 600.0).abs() < 1.0), "Must intersect RSB x=600: {xs:?}");

    // Press 'G' while dragging measurement line (`CanvasTool::keyDown:` at spec_ref)
    assert!(state.guides.is_empty());
    press_key(&mut state, Key::G, false, false, false, false);
    assert_eq!(state.guides.len(), 1, "Pressing 'G' during measurement drag must create a local Guide");
    assert!(state.guides[0].show_measurement, "Promoted guide must have show_measurement = true");
    assert!(state.show_guides);

    pointer_release(&mut state, p_end, false, false, false, false);
    assert!(!state.v040.measurement.active);
}

// ============================================================================
// TC-V040-006: AnnotationTool (`A`) All 5 Types, Handle Resize/Rotate, Delete & Undo
// ============================================================================
#[test]
fn tc_v040_006_annotation_tool_five_types_handle_manipulation_delete_and_undo() {
    let mut state = GlifoCanvasState::new(GlifoTheme::sonoma_default());
    state.viewport.zoom = 0.5;
    state.viewport.pan_offset = ScreenPoint::new(100.0, 500.0);

    // Activate AnnotationTool ('A')
    press_key(&mut state, Key::A, false, false, false, false);
    assert_eq!(state.active_tool, CanvasTool::Annotation);
    assert_eq!(state.v040.annotation_sub_tool, AnnotationSubTool::Text);

    // 1. Click to place Text annotation (type 1)
    let p_text = state.viewport.to_screen(FontPoint::new(100.0, 400.0));
    pointer_press(&mut state, p_text, false, false, false, false);
    pointer_release(&mut state, p_text, false, false, false, false);
    assert_eq!(state.glyph.annotations.len(), 1);
    assert!(matches!(state.glyph.annotations[0].kind, AnnotationKind::Text { .. }));

    // 2. Cycle sub-tool via Shift+A -> Arrow (2), Circle (3), Plus (4), Minus (5)
    press_key(&mut state, Key::A, true, false, false, false);
    assert_eq!(state.v040.annotation_sub_tool, AnnotationSubTool::Arrow);
    let p_arrow = state.viewport.to_screen(FontPoint::new(250.0, 400.0));
    pointer_press(&mut state, p_arrow, false, false, false, false);
    pointer_release(&mut state, p_arrow, false, false, false, false);

    press_key(&mut state, Key::A, true, false, false, false);
    assert_eq!(state.v040.annotation_sub_tool, AnnotationSubTool::Circle);
    let p_circle = state.viewport.to_screen(FontPoint::new(400.0, 300.0));
    pointer_press(&mut state, p_circle, false, false, false, false);
    pointer_release(&mut state, p_circle, false, false, false, false);
    // Circle default width/radius is 100.0 UPM (`spec_ref` in AnnotationTool)
    match state.glyph.annotations[2].kind {
        AnnotationKind::Circle { radius_upm } => assert!((radius_upm - 100.0).abs() < 1e-3),
        ref other => panic!("Expected Circle annotation, got {other:?}"),
    }

    // Drag Circle control handle at (400, 300 - 100) = (400, 200) downward to (400, 150) -> radius increases to 150.0
    let circle_handle_scr = state.viewport.to_screen(FontPoint::new(400.0, 200.0));
    let circle_handle_drag = state.viewport.to_screen(FontPoint::new(400.0, 150.0));
    pointer_press(&mut state, circle_handle_scr, false, false, false, false);
    pointer_move(&mut state, circle_handle_drag, false, false, false, false);
    pointer_release(&mut state, circle_handle_drag, false, false, false, false);
    match state.glyph.annotations[2].kind {
        AnnotationKind::Circle { radius_upm } => assert!((radius_upm - 150.0).abs() < 1.0),
        ref other => panic!("Expected Circle annotation, got {other:?}"),
    }

    // Place Plus (4) and Minus (5)
    press_key(&mut state, Key::A, true, false, false, false);
    assert_eq!(state.v040.annotation_sub_tool, AnnotationSubTool::Plus);
    let p_plus = state.viewport.to_screen(FontPoint::new(550.0, 400.0));
    pointer_press(&mut state, p_plus, false, false, false, false);
    pointer_release(&mut state, p_plus, false, false, false, false);

    press_key(&mut state, Key::A, true, false, false, false);
    assert_eq!(state.v040.annotation_sub_tool, AnnotationSubTool::Minus);
    let p_minus = state.viewport.to_screen(FontPoint::new(650.0, 400.0));
    pointer_press(&mut state, p_minus, false, false, false, false);
    pointer_release(&mut state, p_minus, false, false, false, false);

    assert_eq!(state.glyph.annotations.len(), 5);

    // Cmd+A in AnnotationTool selects all 5 annotations (`AnnotationTool::selectAll:`)
    press_key(&mut state, Key::A, false, false, true, false);
    assert_eq!(state.v040.selected_annotations.len(), 5);

    // Backspace deletes all selected annotations (`AnnotationTool::deleteBackward:`) with Undo/Redo
    press_key(&mut state, Key::Backspace, false, false, false, false);
    assert!(state.glyph.annotations.is_empty());
    assert!(state.undo());
    assert_eq!(state.glyph.annotations.len(), 5);
    assert!(state.redo());
    assert!(state.glyph.annotations.is_empty());
}

// ============================================================================
// TC-V040-007: PencilTool (`B`) Freehand Sampling & Cubic Bézier Curve Fitting
// ============================================================================
#[test]
fn tc_v040_007_pencil_tool_freehand_sampling_and_cubic_curve_fitting() {
    let mut state = GlifoCanvasState::new(GlifoTheme::sonoma_default());
    state.viewport.zoom = 0.5;
    state.viewport.pan_offset = ScreenPoint::new(100.0, 500.0);

    // Press 'B' -> activates PencilTool (`-[PenTool trigger] = @"b"`)
    press_key(&mut state, Key::B, false, false, false, false);
    assert_eq!(state.active_tool, CanvasTool::Pencil);

    // 1. Open freehand stroke
    let pts = [
        FontPoint::new(100.0, 200.0),
        FontPoint::new(160.0, 280.0),
        FontPoint::new(240.0, 320.0),
        FontPoint::new(340.0, 260.0),
        FontPoint::new(420.0, 180.0),
    ];
    let scr0 = state.viewport.to_screen(pts[0]);
    pointer_press(&mut state, scr0, false, false, false, false);
    for pt in &pts[1..] {
        let scr = state.viewport.to_screen(*pt);
        pointer_move(&mut state, scr, false, false, false, false);
    }
    assert!(state.v040.pencil.active);
    assert!(state.v040.pencil.points_upm.len() >= 4);
    let scr_last = state.viewport.to_screen(*pts.last().unwrap());
    pointer_release(
        &mut state,
        scr_last,
        false,
        false,
        false,
        false,
    );

    assert_eq!(state.contours.len(), 1);
    assert!(!state.is_contour_closed(0), "Open stroke far from start must produce an open contour");
    assert!(state.contours[0].iter().any(|n| n.node_type == NodeType::OffCurve));

    // 2. Closed freehand loop (ends near start point)
    let loop_pts = [
        FontPoint::new(200.0, 400.0),
        FontPoint::new(300.0, 500.0),
        FontPoint::new(400.0, 400.0),
        FontPoint::new(300.0, 300.0),
        FontPoint::new(202.0, 401.0),
    ];
    let lscr0 = state.viewport.to_screen(loop_pts[0]);
    pointer_press(&mut state, lscr0, false, false, false, false);
    for pt in &loop_pts[1..] {
        let lscr = state.viewport.to_screen(*pt);
        pointer_move(&mut state, lscr, false, false, false, false);
    }
    let lscr_last = state.viewport.to_screen(*loop_pts.last().unwrap());
    pointer_release(
        &mut state,
        lscr_last,
        false,
        false,
        false,
        false,
    );

    assert_eq!(state.contours.len(), 2);
    assert!(state.is_contour_closed(1), "Loop ending near start must auto-close");

    // Undo / Redo roundtrip
    assert!(state.undo());
    assert_eq!(state.contours.len(), 1);
    assert!(state.redo());
    assert_eq!(state.contours.len(), 2);
}

// ============================================================================
// TC-V040-008: PixelTool (`X`) Grid Cell Placement, Erase Mode & Shift Line
// ============================================================================
#[test]
fn tc_v040_008_pixel_tool_grid_cell_paint_erase_and_shift_bresenham_line() {
    let mut state = GlifoCanvasState::new(GlifoTheme::sonoma_default());
    state.viewport.zoom = 0.5;
    state.viewport.pan_offset = ScreenPoint::new(100.0, 500.0);
    state.grid_step_upm = 50.0;

    // Press 'X' -> activates PixelTool (`-[PixelTool trigger] = @"x"`)
    press_key(&mut state, Key::X, false, false, false, false);
    assert_eq!(state.active_tool, CanvasTool::Pixel);

    // 1. Click cell (100, 100) -> adds 50x50 pixel contour at [100..150, 100..150]
    let cell_a = state.viewport.to_screen(FontPoint::new(110.0, 110.0));
    pointer_press(&mut state, cell_a, false, false, false, false);
    pointer_release(&mut state, cell_a, false, false, false, false);
    assert_eq!(state.contours.len(), 1);
    assert_eq!(state.contours[0].len(), 4);

    // 2. Shift-click at cell (250, 100) -> Bresenham line adds cells (150, 100), (200, 100), (250, 100) = 4 total
    let cell_b = state.viewport.to_screen(FontPoint::new(260.0, 110.0));
    pointer_press(&mut state, cell_b, true, false, false, false);
    pointer_release(&mut state, cell_b, true, false, false, false);
    assert_eq!(state.contours.len(), 4);

    // 3. Clicking an occupied cell (100, 100) enters erase mode (`remove = YES`) and removes it
    pointer_press(&mut state, cell_a, false, false, false, false);
    pointer_release(&mut state, cell_a, false, false, false, false);
    assert_eq!(state.contours.len(), 3);

    // Undo / Redo roundtrip
    assert!(state.undo());
    assert_eq!(state.contours.len(), 4);
}

// ============================================================================
// TC-V040-009: Headless & egui Parity + Light/Dark Theme Rendering for v0.4.0
// ============================================================================
#[test]
fn tc_v040_009_headless_and_light_dark_rendering_parity_for_v040_tools() {
    let mut state = init_single_box_state();
    state.glyph.annotations.push(CanvasAnnotation {
        id: AnnotationId(1),
        position: FontPoint::new(300.0, 300.0),
        kind: AnnotationKind::Circle { radius_upm: 100.0 },
    });
    state.glyph.annotations.push(CanvasAnnotation {
        id: AnnotationId(2),
        position: FontPoint::new(200.0, 400.0),
        kind: AnnotationKind::Arrow {
            angle_deg: 35.0,
            length_upm: 90.0,
        },
    });
    state.v040.show_annotations = true;
    state.active_tool = CanvasTool::Measurement;
    state.v040.measurement.active = true;
    state.v040.measurement.start_upm = FontPoint::new(50.0, 300.0);
    state.v040.measurement.current_upm = FontPoint::new(550.0, 300.0);
    state.recompute_measurement_intersections();

    // Render in Light Mode
    state.set_appearance_mode(AppearanceMode::Light);
    let light_theme = state.theme.clone();
    let pix_light = rasterize_canvas_subregion(&state, &light_theme, 800, 600);

    // Render in Dark Mode
    state.set_appearance_mode(AppearanceMode::Dark);
    let dark_theme = state.theme.clone();
    let pix_dark = rasterize_canvas_subregion(&state, &dark_theme, 800, 600);

    assert_ne!(
        pix_light.data(),
        pix_dark.data(),
        "Light and Dark mode renders for v0.4.0 overlays must be visually distinct"
    );
}

// ============================================================================
// TC-V040-010: PrimitivesTool Space-Bar Panning & Grid Rounding
// ============================================================================
#[test]
fn tc_v040_010_primitives_space_pan_and_grid_rounding() {
    let mut state = GlifoCanvasState::new(GlifoTheme::sonoma_default());
    state.viewport.zoom = 0.5;
    state.viewport.pan_offset = ScreenPoint::new(100.0, 500.0);
    state.round_to_grid = true;
    state.grid_step_upm = 25.0;
    press_key(&mut state, Key::F, false, false, false, false);

    let p0 = state.viewport.to_screen(FontPoint::new(100.0, 100.0));
    let p1 = state.viewport.to_screen(FontPoint::new(212.0, 212.0));
    pointer_press(&mut state, p0, false, false, false, true);
    pointer_move(&mut state, p1, false, false, false, true);

    // Hold Space (`prev_view_mode = Some(...)`) and move by (+50, +50) -> start_upm translates from (100,100) to (150,150)
    press_key(&mut state, Key::Space, false, false, false, false);
    let p2 = state.viewport.to_screen(FontPoint::new(262.0, 262.0));
    pointer_move(&mut state, p2, false, false, false, true);
    assert!((state.v040.primitive_drag.start_upm.x.0 - 150.0).abs() < 1.0);
    assert!((state.v040.primitive_drag.start_upm.y.0 - 150.0).abs() < 1.0);

    // Grid rounding to 25.0 UPM snaps 262.0 -> 250.0
    let rect = state.v040.primitive_drag.preview_rect_upm();
    assert!((rect.0 - 150.0).abs() < 1e-3 && (rect.2 - 250.0).abs() < 1e-3);
    pointer_release(&mut state, p2, false, false, false, true);
    assert_eq!(state.contours.len(), 1);
}

// ============================================================================
// TC-V040-011: MeasurementTool Node Snapping & Typographic Angle (0° = Up)
// ============================================================================
#[test]
fn tc_v040_011_measurement_node_snapping_and_ccw_vertical_angle() {
    let mut state = init_single_box_state();
    press_key(&mut state, Key::L, false, false, false, false);

    // Click near node (100, 100) at (104, 104) -> snaps start_upm to exact node (100, 100)
    let near_n1 = state.viewport.to_screen(FontPoint::new(104.0, 104.0));
    let near_n4 = state.viewport.to_screen(FontPoint::new(104.0, 496.0));
    pointer_press(&mut state, near_n1, false, false, false, false);
    assert!((state.v040.measurement.start_upm.x.0 - 100.0).abs() < 1e-3);
    assert!((state.v040.measurement.start_upm.y.0 - 100.0).abs() < 1e-3);

    // Drag near node (100, 500) -> snaps current_upm to (100, 500)
    pointer_move(&mut state, near_n4, false, false, false, false);
    assert!((state.v040.measurement.current_upm.x.0 - 100.0).abs() < 1e-3);
    assert!((state.v040.measurement.current_upm.y.0 - 500.0).abs() < 1e-3);

    // Vertical upward (100, 100) -> (100, 500) has typographic angle 0.0° (`spec_ref..spec_ref`)
    let angle_up = grafia::compute_measurement_typographic_angle_deg(
        state.v040.measurement.start_upm,
        state.v040.measurement.current_upm,
    );
    assert!(angle_up.abs() < 1e-3, "Vertical upward angle must be 0°, got {angle_up}");
}

// ============================================================================
// TC-V040-012: Annotation Arrow Rotate, Text Width Resize & Shift Body Drag
// ============================================================================
#[test]
fn tc_v040_012_annotation_arrow_rotate_text_resize_and_shift_body_drag() {
    let mut state = GlifoCanvasState::new(GlifoTheme::sonoma_default());
    state.viewport.zoom = 0.5;
    state.viewport.pan_offset = ScreenPoint::new(100.0, 500.0);
    state.active_tool = CanvasTool::Annotation;

    // 1. Place Text annotation at (100, 300), drag its handle from (200, 300) to (280, 300) -> width = 180.0
    let p_text = state.viewport.to_screen(FontPoint::new(100.0, 300.0));
    pointer_press(&mut state, p_text, false, false, false, false);
    pointer_release(&mut state, p_text, false, false, false, false);
    let h_text = state.viewport.to_screen(FontPoint::new(200.0, 300.0));
    let h_text_drag = state.viewport.to_screen(FontPoint::new(280.0, 300.0));
    pointer_press(&mut state, h_text, false, false, false, false);
    pointer_move(&mut state, h_text_drag, false, false, false, false);
    pointer_release(&mut state, h_text_drag, false, false, false, false);
    match state.glyph.annotations[0].kind {
        AnnotationKind::Text { width_upm, .. } => assert!((width_upm - 180.0).abs() < 1.0),
        ref other => panic!("Expected Text, got {other:?}"),
    }

    // 2. Drag Text body with Shift held from (100, 300) toward (250, 315) -> constrained horizontally to y=300
    let drag_body = state.viewport.to_screen(FontPoint::new(250.0, 315.0));
    pointer_press(&mut state, p_text, false, false, false, false);
    pointer_move(&mut state, drag_body, true, false, false, false);
    pointer_release(&mut state, drag_body, true, false, false, false);
    assert!((state.glyph.annotations[0].position.x.0 - 250.0).abs() < 1.0);
    assert!((state.glyph.annotations[0].position.y.0 - 300.0).abs() < 1e-3);

    // 3. Place Arrow annotation at (400, 200), drag its handle to (400, 300) -> angle = 90.0°
    state.v040.annotation_sub_tool = AnnotationSubTool::Arrow;
    let p_arr = state.viewport.to_screen(FontPoint::new(400.0, 200.0));
    pointer_press(&mut state, p_arr, false, false, false, false);
    pointer_release(&mut state, p_arr, false, false, false, false);
    let h_arr = state.viewport.to_screen(
        state.glyph.annotations[1].handle_position_upm().unwrap(),
    );
    let h_arr_up = state.viewport.to_screen(FontPoint::new(400.0, 300.0));
    pointer_press(&mut state, h_arr, false, false, false, false);
    pointer_move(&mut state, h_arr_up, false, false, false, false);
    pointer_release(&mut state, h_arr_up, false, false, false, false);
    match state.glyph.annotations[1].kind {
        AnnotationKind::Arrow { angle_deg, .. } => assert!((angle_deg - 90.0).abs() < 1.0),
        ref other => panic!("Expected Arrow, got {other:?}"),
    }
}

// ============================================================================
// TC-V040-013: KnifeTool Slicing Cubic Bézier Contours ('O' Archetype)
// ============================================================================
#[test]
fn tc_v040_013_knife_tool_cuts_cubic_bezier_contour_glyph_o() {
    let mut state = GlifoCanvasState::init_glyph_o();
    let initial_contours = state.contours.len();
    press_key(&mut state, Key::E, false, false, false, false);

    // Drag horizontally across the middle of 'O'
    let s0 = state.viewport.to_screen(FontPoint::new(-50.0, 350.0));
    let s1 = state.viewport.to_screen(FontPoint::new(800.0, 350.0));
    pointer_press(&mut state, s0, true, false, false, false);
    pointer_move(&mut state, s1, true, false, false, false);
    assert_eq!(state.v040.knife.intersections.len(), 4);
    pointer_release(&mut state, s1, true, false, false, false);

    // Outer + inner counter ring sliced across both stems produces 2 disjoint arch contours (top & bottom)
    assert_eq!(state.contours.len(), initial_contours);
    assert!(
        state.contours.iter().any(|c| c.iter().all(|n| n.point.y.0 <= 350.5))
            && state.contours.iter().any(|c| c.iter().all(|n| n.point.y.0 >= 349.5)),
        "Slicing /o at y=350 must produce a bottom arch (y <= 350) and a top arch (y >= 350)"
    );
    state.assert_invariants();
}

// ============================================================================
// TC-V040-014: egui GlifoCanvasWidget Full-Pass Paint Across All v0.4.0 Tools
// ============================================================================
#[test]
fn tc_v040_014_egui_widget_renders_all_v040_overlays_without_panic() {
    let mut state = GlifoCanvasState::init_glyph_o();
    state.glyph.annotations.push(CanvasAnnotation::new_for_sub_tool(
        AnnotationId(1),
        FontPoint::new(150.0, 450.0),
        AnnotationSubTool::Text,
    ));
    state.glyph.annotations.push(CanvasAnnotation::new_for_sub_tool(
        AnnotationId(2),
        FontPoint::new(350.0, 350.0),
        AnnotationSubTool::Circle,
    ));
    state.glyph.annotations.push(CanvasAnnotation::new_for_sub_tool(
        AnnotationId(3),
        FontPoint::new(450.0, 250.0),
        AnnotationSubTool::Arrow,
    ));
    state.glyph.annotations.push(CanvasAnnotation::new_for_sub_tool(
        AnnotationId(4),
        FontPoint::new(550.0, 450.0),
        AnnotationSubTool::Plus,
    ));
    state.glyph.annotations.push(CanvasAnnotation::new_for_sub_tool(
        AnnotationId(5),
        FontPoint::new(620.0, 450.0),
        AnnotationSubTool::Minus,
    ));

    let ctx = egui::Context::default();
    for mode in [AppearanceMode::Light, AppearanceMode::Dark] {
        state.set_appearance_mode(mode);
        for tool in [
            CanvasTool::Knife,
            CanvasTool::Primitives,
            CanvasTool::Measurement,
            CanvasTool::Annotation,
            CanvasTool::Pencil,
            CanvasTool::Pixel,
        ] {
            state.active_tool = tool;
            let raw = RawInput {
                screen_rect: Some(default_canvas_rect()),
                ..Default::default()
            };
            let theme = state.theme.clone();
            let mut out = ctx.run_ui(raw, |ui| {
                let _ = grafia::GlifoCanvasWidget::new(&mut state, &theme).show(ui);
            });
            out.textures_delta.clear();
        }
    }
}

// ============================================================================
// TC-V040-015: Visual Proof PNG Artifacts (`frame_48` .. `frame_53`)
// ============================================================================

#[test]
fn tc_v040_015_generate_visual_proof_pngs() {
    let out_dirs = [
        std::path::PathBuf::from(r"D:\Programmeren\glifo\target\visual_verification"),
        std::path::PathBuf::from(
            r"C:\Users\jhove\.gemini\antigravity-cli\brain\fccb9142-9217-4702-a2ca-95303b541ee0",
        ),
    ];
    for dir in &out_dirs {
        let _ = std::fs::create_dir_all(dir);
    }

    let save_frame = |name: &str, state: &GlifoCanvasState| {
        let theme = state.theme.clone();
        let pm = rasterize_canvas_subregion(state, &theme, 960, 640);
        for dir in &out_dirs {
            if dir.exists() {
                let path = dir.join(name);
                pm.save_png(&path).expect("failed to save visual proof PNG");
            }
        }
        pm
    };

    // Frame 48: KnifeTool active drag with orange slice line + intersection circles
    let mut s48 = GlifoCanvasState::init_glyph_a();
    press_key(&mut s48, Key::E, false, false, false, false);
    let p0 = s48.viewport.to_screen(FontPoint::new(0.0, 260.0));
    let p1 = s48.viewport.to_screen(FontPoint::new(700.0, 260.0));
    pointer_press(&mut s48, p0, false, false, false, false);
    pointer_move(&mut s48, p1, false, false, false, false);
    let pm48 = save_frame("frame_48_v040_knife_tool.png", &s48);
    assert!(!s48.v040.knife.intersections.is_empty());

    // Frame 49: PrimitivesTool (inserted Rectangle + active Circle preview with dimension badge)
    let mut s49 = GlifoCanvasState::init_glyph_a();
    press_key(&mut s49, Key::F, false, false, false, false);
    s49.insert_primitive(PrimitiveSubTool::Rectangle, FontPoint::new(40.0, 480.0), 140.0, 140.0);
    press_key(&mut s49, Key::F, true, false, false, false);
    let c0 = s49.viewport.to_screen(FontPoint::new(500.0, 480.0));
    let c1 = s49.viewport.to_screen(FontPoint::new(660.0, 640.0));
    pointer_press(&mut s49, c0, false, false, false, false);
    pointer_move(&mut s49, c1, true, false, false, false);
    let pm49 = save_frame("frame_49_v040_primitives_rect_circle.png", &s49);
    assert_ne!(pm48.data(), pm49.data());

    // Frame 50: MeasurementTool (Blue/Green/Red labels + active ruler ray with intersections)
    let mut s50 = GlifoCanvasState::init_glyph_o();
    s50.selected_nodes = s50.contours[0].iter().map(|n| n.id).collect();
    press_key(&mut s50, Key::L, false, false, false, false);
    let m0 = s50.viewport.to_screen(FontPoint::new(-50.0, 350.0));
    let m1 = s50.viewport.to_screen(FontPoint::new(750.0, 350.0));
    pointer_press(&mut s50, m0, false, false, false, false);
    pointer_move(&mut s50, m1, false, false, false, false);
    let pm50 = save_frame("frame_50_v040_measurement_tool_coords.png", &s50);
    assert_ne!(pm49.data(), pm50.data());

    // Frame 51: AnnotationTool (all 5 CanvasAnnotation types: Text, Circle, Arrow, Plus, Minus)
    let mut s51 = GlifoCanvasState::init_glyph_a();
    press_key(&mut s51, Key::A, false, false, false, false);
    s51.glyph.annotations.push(CanvasAnnotation::new_for_sub_tool(AnnotationId(1), FontPoint::new(80.0, 620.0), AnnotationSubTool::Text));
    s51.glyph.annotations.push(CanvasAnnotation::new_for_sub_tool(AnnotationId(2), FontPoint::new(350.0, 350.0), AnnotationSubTool::Circle));
    s51.glyph.annotations.push(CanvasAnnotation::new_for_sub_tool(AnnotationId(3), FontPoint::new(500.0, 220.0), AnnotationSubTool::Arrow));
    s51.glyph.annotations.push(CanvasAnnotation::new_for_sub_tool(AnnotationId(4), FontPoint::new(140.0, 200.0), AnnotationSubTool::Plus));
    s51.glyph.annotations.push(CanvasAnnotation::new_for_sub_tool(AnnotationId(5), FontPoint::new(580.0, 560.0), AnnotationSubTool::Minus));
    s51.v040.selected_annotations = vec![AnnotationId(2), AnnotationId(3)];
    let pm51 = save_frame("frame_51_v040_annotations_five_types.png", &s51);
    assert_ne!(pm50.data(), pm51.data());

    // Frame 52: PencilTool & PixelTool (freehand trail + stamped pixel cells)
    let mut s52 = GlifoCanvasState::init_glyph_a();
    s52.grid_step_upm = 50.0;
    press_key(&mut s52, Key::X, false, false, false, false);
    for (gx, gy) in [(1, 8), (2, 8), (3, 8), (2, 9)] {
        let p = s52.viewport.to_screen(FontPoint::new(f64::from(gx) * 50.0 + 10.0, f64::from(gy) * 50.0 + 10.0));
        pointer_press(&mut s52, p, false, false, false, false);
        pointer_release(&mut s52, p, false, false, false, false);
    }
    press_key(&mut s52, Key::B, false, false, false, false);
    for i in 0..=12 {
        let t = f64::from(i) / 12.0;
        let pt = s52.viewport.to_screen(FontPoint::new(420.0 + 200.0 * t, 480.0 + 90.0 * (t * std::f64::consts::PI).sin()));
        if i == 0 {
            pointer_press(&mut s52, pt, false, false, false, false);
        } else {
            pointer_move(&mut s52, pt, false, false, false, false);
        }
    }
    let pm52 = save_frame("frame_52_v040_pencil_and_pixel_tools.png", &s52);
    assert_ne!(pm51.data(), pm52.data());

    // Frame 53: Dark Mode (`AppearanceMode::Dark`) with Annotations & Measurement Ruler
    let mut s53 = GlifoCanvasState::init_glyph_a();
    s53.glyph.annotations = s51.glyph.annotations.clone();
    s53.v040.selected_annotations = s51.v040.selected_annotations.clone();
    s53.set_appearance_mode(AppearanceMode::Dark);
    s53.active_tool = CanvasTool::Measurement;
    s53.v040.measurement.active = true;
    s53.v040.measurement.start_upm = FontPoint::new(0.0, 260.0);
    s53.v040.measurement.current_upm = FontPoint::new(700.0, 260.0);
    s53.recompute_measurement_intersections();
    let pm53 = save_frame("frame_53_v040_dark_mode_tools.png", &s53);
    assert_ne!(pm51.data(), pm53.data());
}

// ============================================================================
// TC-V040-016: KnifeTool Slicing Through OnCurve Seam Nodes & Multi-Stem Contours
// ============================================================================
#[test]
fn tc_v040_016_knife_tool_slices_through_oncurve_seam_nodes_and_multi_stem_contours() {
    // 1. Create a Circle via PrimitivesTool::Circle at [100..500, 100..500].
    // Its 4 OnCurve extreme nodes sit at (300, 100), (500, 300), (300, 500), (100, 300).
    let mut state = GlifoCanvasState::new(GlifoTheme::sonoma_default());
    state.viewport.zoom = 0.5;
    state.viewport.pan_offset = ScreenPoint::new(100.0, 500.0);
    assert!(state.insert_primitive(
        PrimitiveSubTool::Circle,
        FontPoint::new(100.0, 100.0),
        400.0,
        400.0,
    ));
    assert_eq!(state.contours.len(), 1);

    // Slice vertically right through node[0] (300, 100) and node[6] (300, 500)
    press_key(&mut state, Key::E, false, false, false, false);
    let s_bot = state.viewport.to_screen(FontPoint::new(300.0, 20.0));
    let s_top = state.viewport.to_screen(FontPoint::new(300.0, 580.0));
    pointer_press(&mut state, s_bot, true, false, false, false);
    pointer_move(&mut state, s_top, true, false, false, false);
    assert_eq!(
        state.v040.knife.intersections.len(),
        2,
        "Knife passing directly through OnCurve extreme nodes (300,100) and (300,500) must detect both hits"
    );
    pointer_release(&mut state, s_top, true, false, false, false);
    assert_eq!(
        state.contours.len(),
        2,
        "Circle sliced through OnCurve seam nodes must split into 2 closed halves"
    );
    state.assert_invariants();

    // 2. Multi-stem horizontal slice across Glyph '/A' outer contour (crosses both left and right legs = 4 hits)
    let mut state_a = GlifoCanvasState::init_glyph_a();
    // Remove inner counter so we test a single non-convex contour crossed 4 times at y = 100
    state_a.contours.truncate(1);
    state_a.glyph.contours.truncate(1);
    state_a.sync_paths_from_contours();
    press_key(&mut state_a, Key::E, false, false, false, false);
    let a_left = state_a.viewport.to_screen(FontPoint::new(-50.0, 100.0));
    let a_right = state_a.viewport.to_screen(FontPoint::new(750.0, 100.0));
    pointer_press(&mut state_a, a_left, true, false, false, false);
    pointer_move(&mut state_a, a_right, true, false, false, false);
    assert_eq!(
        state_a.v040.knife.intersections.len(),
        4,
        "Horizontal slice at y=100 across '/A' outer contour must hit 4 leg edges"
    );
    pointer_release(&mut state_a, a_right, true, false, false, false);
    assert_eq!(
        state_a.contours.len(),
        3,
        "4-hit pairwise slice across both legs of '/A' must split the contour into 3 closed pieces"
    );
    state_a.assert_invariants();
}

// ============================================================================
// TC-V040-017: Annotation Clipboard (Cmd+C/X/V), Empty Text Body Hit-Test & F-Key Cycling
// ============================================================================
#[test]
fn tc_v040_017_annotation_clipboard_empty_text_body_hit_and_f_key_subtool_cycling() {
    let mut state = GlifoCanvasState::new(GlifoTheme::sonoma_default());
    state.viewport.zoom = 0.5;
    state.viewport.pan_offset = ScreenPoint::new(100.0, 500.0);

    // 1. Pressing 'F' activates Primitives (Rectangle); pressing 'F' again while in Primitives cycles to Circle
    press_key(&mut state, Key::F, false, false, false, false);
    assert_eq!(state.active_tool, CanvasTool::Primitives);
    assert_eq!(state.v040.primitive_sub_tool, PrimitiveSubTool::Rectangle);
    press_key(&mut state, Key::F, false, false, false, false);
    assert_eq!(
        state.v040.primitive_sub_tool,
        PrimitiveSubTool::Circle,
        "Pressing 'F' while already in PrimitivesTool must cycle Rectangle <-> Circle (SPEC-005 §2.2.1)"
    );

    // 2. Place an empty Text annotation at (100, 300); clicking inside its body at (150, 285) must select/move it, NOT spawn a 2nd annotation
    press_key(&mut state, Key::A, false, false, false, false);
    assert_eq!(state.v040.annotation_sub_tool, AnnotationSubTool::Text);
    let p_origin = state.viewport.to_screen(FontPoint::new(100.0, 300.0));
    pointer_press(&mut state, p_origin, false, false, false, false);
    pointer_release(&mut state, p_origin, false, false, false, false);
    assert_eq!(state.glyph.annotations.len(), 1);

    let p_inside_body = state.viewport.to_screen(FontPoint::new(150.0, 285.0));
    let p_drag_to = state.viewport.to_screen(FontPoint::new(200.0, 285.0));
    pointer_press(&mut state, p_inside_body, false, false, false, false);
    pointer_move(&mut state, p_drag_to, false, false, false, false);
    pointer_release(&mut state, p_drag_to, false, false, false, false);
    assert_eq!(
        state.glyph.annotations.len(),
        1,
        "Clicking inside 'empty note' Text annotation box must hit-test existing annotation, not create a duplicate"
    );
    assert!((state.glyph.annotations[0].position.x.0 - 150.0).abs() < 1.0);

    // 3. Annotation Clipboard: Cmd+C -> Cmd+V duplicates with (+10, +10) offset; Cmd+X cuts with Undo
    press_key(&mut state, Key::C, false, false, true, false);
    press_key(&mut state, Key::V, false, false, true, false);
    assert_eq!(
        state.glyph.annotations.len(),
        2,
        "Cmd+V in AnnotationTool must paste copied annotation"
    );
    assert!((state.glyph.annotations[1].position.x.0 - 160.0).abs() < 1.0);
    assert!((state.glyph.annotations[1].position.y.0 - 310.0).abs() < 1.0);

    press_key(&mut state, Key::X, false, false, true, false);
    assert_eq!(
        state.glyph.annotations.len(),
        1,
        "Cmd+X in AnnotationTool must cut the selected annotation"
    );
    assert!(state.undo());
    assert_eq!(state.glyph.annotations.len(), 2);
}

// ============================================================================
// TC-V040-018: Temp Modifier Overrides (Cmd+Ctrl+Opt across PointerRelease & Space, Cmd in Knife/Pencil)
// ============================================================================
#[test]
fn tc_v040_018_temp_modifier_overrides_measurement_and_cmd_select_in_knife_pencil() {
    let mut state = init_single_box_state();
    press_key(&mut state, Key::E, false, false, false, false);
    assert_eq!(state.active_tool, CanvasTool::Knife);

    // 1. Holding Cmd (spec_ref) in KnifeTool temporarily switches to SelectTool on click and restores Knife on release
    let n1_scr = state.viewport.to_screen(FontPoint::new(100.0, 100.0));
    pointer_press(&mut state, n1_scr, false, false, true, false);
    assert_eq!(
        state.active_tool,
        CanvasTool::Select,
        "Cmd-click in KnifeTool must temporarily activate SelectTool (tempTrigger = spec_ref)"
    );
    assert_eq!(state.selected_nodes.len(), 1);
    pointer_release(&mut state, n1_scr, false, false, true, false);
    assert_eq!(
        state.active_tool,
        CanvasTool::Knife,
        "Releasing pointer after Cmd-click must restore KnifeTool"
    );

    // 2. Holding Cmd+Ctrl+Opt temporarily activates MeasurementTool; releasing pointer while STILL holding Cmd+Ctrl+Opt keeps MeasurementTool
    let m0 = state.viewport.to_screen(FontPoint::new(50.0, 300.0));
    let m1 = state.viewport.to_screen(FontPoint::new(550.0, 300.0));
    pointer_move(&mut state, m0, false, true, true, true);
    assert_eq!(state.active_tool, CanvasTool::Measurement);
    pointer_press(&mut state, m0, false, true, true, true);
    pointer_move(&mut state, m1, false, true, true, true);
    pointer_release(&mut state, m1, false, true, true, true);
    assert_eq!(
        state.active_tool,
        CanvasTool::Measurement,
        "MeasurementTool must remain active after pointer release while Cmd+Ctrl+Opt is still held"
    );
    // Releasing Cmd+Ctrl+Opt restores KnifeTool
    pointer_move(&mut state, m1, false, false, false, false);
    assert_eq!(state.active_tool, CanvasTool::Knife);
}

// ============================================================================
// TC-V040-019: Regression Test for Jaco's Crash Report (`NodeId(17)` stale selection
// after Knife cut open twice -> Connect Open Endpoints -> Select Contour -> Add Inflections,
// plus `hit_test_node_with_radius` open endpoint priority & multi-contour `Connect Open Endpoints`)
// ============================================================================
#[test]
fn tc_v040_019_jaco_crash_regression_knife_open_connect_add_inflections_and_selection_invariants() {
    // 1. Exact reproduction on `/n`:
    // Cut `/n` open twice with single-intersection knife cuts -> produces 2 open contours.
    let mut state = GlifoCanvasState::new(GlifoTheme::sonoma_default());
    state.load_glyph_archetype(grafia::tokens::GlyphArchetype::N);
    let mut flags = state.closed_flags();
    assert!(grafia::core::knife_cut_paths_in_layer(
        &mut state.contours,
        &mut flags,
        FontPoint::new(0.0, 250.0),
        FontPoint::new(110.0, 250.0),
        &mut state.next_node_id,
    ));
    assert_eq!(state.contours.len(), 1);
    assert!(!flags[0]);

    assert!(grafia::core::knife_cut_paths_in_layer(
        &mut state.contours,
        &mut flags,
        FontPoint::new(300.0, 460.0),
        FontPoint::new(300.0, 560.0),
        &mut state.next_node_id,
    ));
    assert_eq!(state.contours.len(), 2);
    assert!(!flags[0] && !flags[1]);
    state.glyph.contours = state
        .contours
        .iter()
        .enumerate()
        .map(|(i, c)| {
            let mut gc = grafia::core::GlyphContour::new(grafia::core::ContourId(i), c.clone());
            gc.closed = flags[i];
            gc
        })
        .collect();
    state.active_contour_closed = false;
    state.sync_paths_from_contours();
    state.assert_invariants();

    // Verify NodeId(17) is an OffCurve node in one of the open contours
    assert!(
        state
            .contours
            .iter()
            .flatten()
            .any(|n| n.id == NodeId(17) && n.node_type == NodeType::OffCurve),
        "NodeId(17) must exist as an OffCurve node on /n"
    );

    // 1b. First test Jaco's exact sequence: close contour 0 & contour 1 via Connect Open Endpoints,
    // select all nodes of the contour containing NodeId(17), and call `add_inflection_points()`.
    let snapshot_two_open = state.snapshot();
    state.selected_nodes = vec![
        state.contours[0].first().unwrap().id,
        state.contours[0].last().unwrap().id,
    ];
    assert!(state.connect_selected_open_endpoints());
    state.selected_nodes = vec![
        state.contours[1].first().unwrap().id,
        state.contours[1].last().unwrap().id,
    ];
    assert!(state.connect_selected_open_endpoints());
    assert!(state.is_contour_closed(0) && state.is_contour_closed(1));

    // Select all nodes of the contour containing NodeId(17), deform the cubic slightly so the
    // segment containing NodeId(17) has an inflection root, and invoke `add_inflection_points()`.
    let c_with_17 = state
        .contours
        .iter()
        .position(|c| c.iter().any(|n| n.id == NodeId(17)))
        .expect("contour with NodeId(17)");
    if let Some(n17) = state.find_node_mut(NodeId(17)) {
        n17.point = FontPoint::new(190.0, 220.0);
    }
    state.sync_paths_from_contours();
    state.selected_nodes = state.contours[c_with_17].iter().map(|n| n.id).collect();
    assert!(state.selected_nodes.contains(&NodeId(17)));

    assert!(
        state.add_inflection_points(),
        "add_inflection_points must subdivide the inflected cubic and never panic on stale selected NodeId(17)"
    );
    state.assert_invariants();

    // 1c. Restore the 2 open contours and test:
    // - Marquee-selecting all 4 open endpoints of both open contours -> `connect_selected_open_endpoints()` closes both!
    state.restore_snapshot(&snapshot_two_open);
    state.selected_nodes = vec![
        state.contours[0].first().unwrap().id,
        state.contours[0].last().unwrap().id,
        state.contours[1].first().unwrap().id,
        state.contours[1].last().unwrap().id,
    ];
    assert!(
        state.connect_selected_open_endpoints(),
        "Connecting when all 4 open endpoints of 2 open halves are selected must close each open contour"
    );
    assert!(state.is_contour_closed(0) && state.is_contour_closed(1));
    state.assert_invariants();

    // - Closing contour 0 first, then hit-testing at the shared cut point WITHOUT clearing selection:
    //   the remaining open contour's endpoint at the same coordinate must win even when contour 0's closed node is still selected!
    state.restore_snapshot(&snapshot_two_open);
    state.selected_nodes = vec![
        state.contours[0].first().unwrap().id,
        state.contours[0].last().unwrap().id,
    ];
    assert!(state.connect_selected_open_endpoints());
    assert!(state.is_contour_closed(0) && !state.is_contour_closed(1));
    let open_ep = state.contours[1].first().unwrap().clone();
    let open_ep_scr = state.viewport.to_screen(open_ep.point);
    let hit_id_while_c0_selected = state
        .hit_test_node(open_ep_scr)
        .expect("must hit node at shared cut coordinate while c0 is still selected");
    assert_eq!(
        hit_id_while_c0_selected, open_ep.id,
        "hit_test_node_with_radius must prioritize open-contour endpoints over closed-contour nodes even when the closed contour's node is still selected"
    );

    // 1d. Single-intersection Knife cut on a cubic segment (e.g. `/n` arch at x=300) followed by
    // `connect_selected_open_endpoints()`: closing must NOT overwrite the trailing `OffCurve` handle into `Corner`!
    let mut state_cubic_cut = GlifoCanvasState::init_glyph_n();
    let mut flags_cc = state_cubic_cut.closed_flags();
    assert!(grafia::core::knife_cut_paths_in_layer(
        &mut state_cubic_cut.contours,
        &mut flags_cc,
        FontPoint::new(300.0, 460.0),
        FontPoint::new(300.0, 560.0),
        &mut state_cubic_cut.next_node_id,
    ));
    state_cubic_cut.set_contour_closed(0, false);
    state_cubic_cut.sync_paths_from_contours();
    state_cubic_cut.assert_invariants();
    let off_count_before = state_cubic_cut.contours[0]
        .iter()
        .filter(|n| n.node_type == NodeType::OffCurve)
        .count();
    state_cubic_cut.selected_nodes = vec![
        state_cubic_cut.contours[0].first().unwrap().id,
        state_cubic_cut.contours[0].last().unwrap().id,
    ];
    assert!(state_cubic_cut.connect_selected_open_endpoints());
    let off_count_after = state_cubic_cut.contours[0]
        .iter()
        .filter(|n| n.node_type == NodeType::OffCurve)
        .count();
    assert_eq!(
        off_count_after, off_count_before,
        "connect_selected_open_endpoints must not corrupt the trailing OffCurve handle into a Corner node after popping the merged OnCurve endpoint"
    );
    state_cubic_cut.assert_invariants();

    // 1e. 2-OnCurve open cubic loop with coincident endpoints: `connect_selected_open_endpoints()`
    // must not pop the 2nd OnCurve node and leave a 1-OnCurve closed contour.
    let mut state_two_on = GlifoCanvasState::new(GlifoTheme::sonoma_default());
    state_two_on.contours = vec![vec![
        grafia::core::Node::new(NodeId(1), 0.0, 0.0, NodeType::Corner),
        grafia::core::Node::new(NodeId(2), 100.0, 0.0, NodeType::OffCurve),
        grafia::core::Node::new(NodeId(3), 0.0, 100.0, NodeType::OffCurve),
        grafia::core::Node::new(NodeId(4), 0.5, 0.0, NodeType::Corner),
    ]];
    state_two_on.active_contour_closed = false;
    state_two_on.sync_paths_from_contours();
    state_two_on.selected_nodes = vec![NodeId(1), NodeId(4)];
    assert!(state_two_on.connect_selected_open_endpoints());
    state_two_on.assert_invariants();
    // Calling `tidy_up_paths()` on that closed 2-OnCurve tear-drop contour must also preserve invariants!
    let _ = state_two_on.tidy_up_paths();
    state_two_on.assert_invariants();

    // 2. Also test `add_inflection_points()` on `/s` with all nodes (including OffCurve handles) selected
    let mut state_s = GlifoCanvasState::init_glyph_s();
    // Deform one spine handle on `/s` so a single cubic segment contains an inflection root
    state_s.contours[0][11].point = FontPoint::new(200.0, 260.0);
    state_s.sync_paths_from_contours();
    state_s.selected_nodes = state_s.contours[0].iter().map(|n| n.id).collect();
    assert!(state_s.add_inflection_points());
    state_s.assert_invariants();

    // 3. Test `add_extremes(false)`, `add_extremes(true)`, `tidy_up_paths()`, `close_open_paths()`,
    // and `Pixel` erase while `selected_nodes` and `active_open_contour` contain stale/affected nodes
    let mut state_ext = GlifoCanvasState::init_glyph_s();
    state_ext.selected_nodes = state_ext.contours[0].iter().map(|n| n.id).collect();
    let _ = state_ext.add_extremes(false);
    state_ext.assert_invariants();
    state_ext.selected_nodes = state_ext.contours[0].iter().map(|n| n.id).collect();
    let _ = state_ext.add_extremes(true);
    state_ext.assert_invariants();
    state_ext.selected_nodes = state_ext.contours[0].iter().map(|n| n.id).collect();
    let _ = state_ext.tidy_up_paths();
    state_ext.assert_invariants();

    // Open a contour, set active_open_contour = Some(0) and pending_pen_bcp_out, and call `close_open_paths()`
    let mut state_open = GlifoCanvasState::init_glyph_n();
    state_open.set_contour_closed(0, false);
    state_open.active_open_contour = Some(0);
    state_open.pending_pen_bcp_out = Some((0, FontPoint::new(120.0, 200.0)));
    state_open.pending_pen_first_bcp_in = Some((0, FontPoint::new(80.0, 100.0)));
    state_open.selected_nodes = state_open.contours[0].iter().map(|n| n.id).collect();
    assert!(state_open.close_open_paths());
    assert!(state_open.pending_pen_bcp_out.is_none());
    assert!(state_open.pending_pen_first_bcp_in.is_none());
    state_open.assert_invariants();

    // Stamp a pixel cell, select its nodes, then erase it with PixelTool while selected
    let mut state_px = GlifoCanvasState::new(GlifoTheme::sonoma_default());
    state_px.viewport.zoom = 0.5;
    state_px.viewport.pan_offset = ScreenPoint::new(100.0, 500.0);
    state_px.grid_step_upm = 50.0;
    press_key(&mut state_px, Key::X, false, false, false, false);
    let p_cell = state_px.viewport.to_screen(FontPoint::new(25.0, 25.0));
    pointer_press(&mut state_px, p_cell, false, false, false, false);
    pointer_release(&mut state_px, p_cell, false, false, false, false);
    assert_eq!(state_px.contours.len(), 1);
    state_px.selected_nodes = state_px.contours[0].iter().map(|n| n.id).collect();
    pointer_press(&mut state_px, p_cell, false, false, false, false);
    pointer_release(&mut state_px, p_cell, false, false, false, false);
    assert_eq!(state_px.contours.len(), 0);
    assert!(state_px.selected_nodes.is_empty());
    state_px.assert_invariants();
}

// ============================================================================
// TC-V040-020: State-Dependent Pull-Based Action-Chain Test (`arbtest`)
// ============================================================================
#[test]
fn tc_v040_020_arbtest_stateful_multi_tool_action_chains() {
    use grafia::core::{extract_segments, ContourSegment};
    use grafia::tokens::GlyphArchetype;

    use std::cell::Cell;
    let runs = Cell::new(0usize);
    arbtest::arbtest(|u| {
        runs.set(runs.get() + 1);
        let archetype = *u.choose(&[
            GlyphArchetype::A,
            GlyphArchetype::O,
            GlyphArchetype::N,
            GlyphArchetype::S,
        ])?;
        let mut state = GlifoCanvasState::new(GlifoTheme::sonoma_default());
        state.viewport.zoom = 0.5;
        state.viewport.pan_offset = ScreenPoint::new(150.0, 500.0);
        state.load_glyph_archetype(archetype);
        state.assert_invariants();

        let steps = u.int_in_range(12..=25)?;
        for _ in 0..steps {
            // (a) Pull-based Live Selection from current state
            let sel_mode = u.int_in_range(0..=4)?;
            match sel_mode {
                0 => {
                    // Select all nodes of a live contour (including OffCurve handles, like clicking a Shapes sidebar row)
                    if !state.contours.is_empty() {
                        let contour = u.choose(&state.contours)?;
                        state.selected_nodes = contour.iter().map(|n| n.id).collect();
                    }
                }
                1 => {
                    // Select all currently open endpoints across all open contours
                    let mut open_eps = Vec::new();
                    for (c_idx, contour) in state.contours.iter().enumerate() {
                        if !state.is_contour_closed(c_idx) && !contour.is_empty() {
                            open_eps.push(contour.first().unwrap().id);
                            if contour.len() > 1 {
                                open_eps.push(contour.last().unwrap().id);
                            }
                        }
                    }
                    open_eps.dedup();
                    state.selected_nodes = open_eps;
                }
                2 => {
                    // Select 2 open endpoints from the same or different open contours
                    let mut open_eps = Vec::new();
                    for (c_idx, contour) in state.contours.iter().enumerate() {
                        if !state.is_contour_closed(c_idx) && contour.len() >= 2 {
                            open_eps.push(contour.first().unwrap().id);
                            open_eps.push(contour.last().unwrap().id);
                        }
                    }
                    if open_eps.len() >= 2 {
                        let idx_a = u.int_in_range(0..=(open_eps.len() - 1))?;
                        let id_a = open_eps.remove(idx_a);
                        let id_b = *u.choose(&open_eps)?;
                        state.selected_nodes = if id_a == id_b { vec![id_a] } else { vec![id_a, id_b] };
                    }
                }
                3 => {
                    // Select a random subset of live NodeIds
                    let all_ids: Vec<NodeId> =
                        state.contours.iter().flat_map(|c| c.iter().map(|n| n.id)).collect();
                    let mut chosen = Vec::new();
                    for id in all_ids {
                        if u.arbitrary::<bool>()? {
                            chosen.push(id);
                        }
                    }
                    state.selected_nodes = chosen;
                }
                _ => {
                    // Keep existing selection or clear
                    if u.arbitrary::<bool>()? {
                        state.selected_nodes.clear();
                    }
                }
            }
            state.assert_invariants();

            // (b) Pull-based Live Tool / Path Mutation
            let action = u.int_in_range(0..=21)?;
            match action {
                0 => {
                    // Knife cut across a live segment's midpoint or bounding box (1-hit, 2-hit, or Alt add-nodes)
                    let mut seg_points = Vec::new();
                    for (c_idx, contour) in state.contours.iter().enumerate() {
                        let closed = state.is_contour_closed(c_idx);
                        for seg in extract_segments(contour, closed) {
                            match seg {
                                ContourSegment::Line { line, .. } => {
                                    let mx = 0.5 * (line.p0.x + line.p1.x);
                                    let my = 0.5 * (line.p0.y + line.p1.y);
                                    let dx = line.p1.x - line.p0.x;
                                    let dy = line.p1.y - line.p0.y;
                                    seg_points.push((FontPoint::new(mx, my), FontPoint::new(-dy, dx)));
                                }
                                ContourSegment::Cubic { cubic, .. } => {
                                    use kurbo::{ParamCurve, ParamCurveDeriv};
                                    let m = cubic.eval(0.5);
                                    let d = cubic.deriv().eval(0.5);
                                    seg_points.push((FontPoint::new(m.x, m.y), FontPoint::new(-d.y, d.x)));
                                }
                            }
                        }
                    }
                    if !seg_points.is_empty() {
                        let &(mid, norm) = u.choose(&seg_points)?;
                        let len = norm.x.0.hypot(norm.y.0).max(1.0);
                        let nx = norm.x.0 / len;
                        let ny = norm.y.0 / len;
                    let knife_kind = u.int_in_range(0..=2)?;
                        let span = if knife_kind == 0 { 40.0 } else { 600.0 };
                        let p0 = FontPoint::new(mid.x.0 - nx * span, mid.y.0 - ny * span);
                        let p1 = FontPoint::new(mid.x.0 + nx * span, mid.y.0 + ny * span);
                        let before = state.snapshot();
                        if knife_kind == 2 {
                            let flags = state.closed_flags();
                            let added = grafia::core::knife_add_nodes_in_layer(
                                &mut state.contours,
                                &flags,
                                p0,
                                p1,
                                &mut state.next_node_id,
                            );
                            if !added.is_empty() {
                                state.selected_nodes = added;
                                state.sync_paths_from_contours();
                                state.record_mutation("Add Nodes with Knife", before);
                            }
                        } else {
                            let mut flags = state.closed_flags();
                            if grafia::core::knife_cut_paths_in_layer(
                                &mut state.contours,
                                &mut flags,
                                p0,
                                p1,
                                &mut state.next_node_id,
                            ) {
                                state.selected_nodes.clear();
                                state.glyph.contours = state
                                    .contours
                                    .iter()
                                    .enumerate()
                                    .map(|(i, c)| {
                                        let mut gc = grafia::core::GlyphContour::new(
                                            grafia::core::ContourId(i),
                                            c.clone(),
                                        );
                                        gc.closed = flags[i];
                                        gc
                                    })
                                    .collect();
                                state.active_contour_closed =
                                    flags.last().copied().unwrap_or(false);
                                state.sync_paths_from_contours();
                                state.record_mutation("Cut Paths", before);
                            }
                        }
                    }
                }
                1 => {
                    let _ = state.add_inflection_points();
                }
                2 => {
                    let force = u.arbitrary::<bool>()?;
                    let _ = state.add_extremes(force);
                }
                3 => {
                    let _ = state.tidy_up_paths();
                }
                4 => {
                    let _ = state.align_selection();
                }
                5 => {
                    let _ = state.connect_selected_open_endpoints();
                }
                6 => {
                    let _ = state.close_open_paths();
                }
                7 => {
                    let _ = state.reverse_selected_or_all_contours();
                }
                8 => {
                    let _ = state.make_selected_node_first();
                }
                9 => {
                    let _ = state.correct_path_direction();
                }
                10 => {
                    // Delete / Backspace (healing delete vs Shift+Delete cut-open delete)
                    let cut_open = u.arbitrary::<bool>()?;
                    let rect = default_canvas_rect();
                    let ev = Event::Key {
                        key: Key::Backspace,
                        physical_key: Some(Key::Backspace),
                        pressed: true,
                        repeat: false,
                        modifiers: mods(cut_open, false, false, false),
                    };
                    let _ = grafia::adapters::keyboard::dispatch_keyboard_event(
                        &mut state, &ev, cut_open, false, false, rect,
                    );
                }
                11 => {
                    let _ = state.copy_selection();
                    let _ = state.paste_clipboard();
                }
                12 => {
                    let _ = state.cut_selection();
                }
                13 => {
                    let _ = state.paste_clipboard();
                }
                14 => {
                    // Pixel tool click/erase on a live cell
                    state.grid_step_upm = 50.0;
                    state.activate_tool(CanvasTool::Pixel);
                    let target_pt = if let Some(contour) = state.contours.first() {
                        contour.first().map_or(FontPoint::new(75.0, 75.0), |n| n.point)
                    } else {
                        FontPoint::new(75.0, 75.0)
                    };
                    let scr = state.viewport.to_screen(target_pt);
                    let m = grafia::interaction::Modifiers::default();
                    let _ = state.on_v040_pointer_press(scr, target_pt, m);
                    let _ = state.on_v040_pointer_release(target_pt, m);
                }
                15 => {
                    // Primitives insert (Rectangle or Circle/Oval)
                    let sub = if u.arbitrary::<bool>()? {
                        PrimitiveSubTool::Rectangle
                    } else {
                        PrimitiveSubTool::Circle
                    };
                    let ox = f64::from(u.int_in_range(20..=300)?);
                    let oy = f64::from(u.int_in_range(20..=300)?);
                    let w = f64::from(u.int_in_range(40..=200)?);
                    let h = f64::from(u.int_in_range(40..=200)?);
                    let _ = state.insert_primitive(sub, FontPoint::new(ox, oy), w, h);
                }
                16 => {
                    let _ = state.flip_horizontal();
                }
                17 => {
                    let _ = state.flip_vertical();
                }
                18 => {
                    let deg = f64::from(u.int_in_range(-90..=90)?);
                    let _ = state.rotate_by_degrees(deg);
                }
                19 => {
                    let sx = if u.arbitrary::<bool>()? { 1.25 } else { 0.8 };
                    let sy = if u.arbitrary::<bool>()? { 1.25 } else { 0.8 };
                    let _ = state.scale_by_factor(sx, sy);
                }
                20 => {
                    let _ = state.remove_overlap();
                }
                _ => {
                    if u.arbitrary::<bool>()? {
                        let _ = state.undo();
                    } else {
                        let _ = state.redo();
                    }
                }
            }

            // (c) Mandatory post-step invariant assertion
            if let Err(err) = state.validate_invariants() {
                panic!("Step action={action} violated invariants: {err}");
            }
        }
        Ok(())
    })
    .budget_ms(2000);
    assert!(
        runs.get() >= 200,
        "arbtest must execute at least 200 stateful action-chain seeds (ran {})",
        runs.get()
    );
}

// ============================================================================
// TC-V040-021: Boolean `Remove Overlap` (`Cmd+Shift+O` / `BooleanPathEngine`)
// on Rectilinear Stems (`+`/`T`), Adjacent `PixelTool` Cells, Overlapping
// Cubic `Circle` Contours, Enclosed Counters, and `Subtract` / `Intersect`
// ============================================================================
#[test]
fn tc_v040_021_boolean_remove_overlap_rectilinear_pixels_cubic_circles_and_counters() {
    // 1. Overlapping rectilinear stems forming a '+' cross:
    // Horizontal bar [100..500, 250..350] + Vertical bar [250..350, 100..500]
    let mut state = GlifoCanvasState::new(GlifoTheme::sonoma_default());
    state.viewport.zoom = 0.5;
    state.viewport.pan_offset = ScreenPoint::new(100.0, 500.0);
    assert!(state.insert_primitive(
        PrimitiveSubTool::Rectangle,
        FontPoint::new(100.0, 250.0),
        400.0,
        100.0,
    ));
    assert!(state.insert_primitive(
        PrimitiveSubTool::Rectangle,
        FontPoint::new(250.0, 100.0),
        100.0,
        400.0,
    ));
    // Also add an open contour to prove open contours are left untouched (`[path closed]` at `spec_ref`)
    let open_nodes = vec![
        Node::new(NodeId(state.next_node_id), 20.0, 20.0, NodeType::Corner),
        Node::new(NodeId(state.next_node_id + 1), 80.0, 20.0, NodeType::Corner),
    ];
    state.next_node_id += 2;
    state.contours.push(open_nodes.clone());
    let mut open_gc = grafia::GlyphContour::new(grafia::ContourId(2), open_nodes);
    open_gc.closed = false;
    state.glyph.contours.push(open_gc);
    state.sync_paths_from_contours();
    state.selected_nodes.clear();

    // Trigger via Cmd+Shift+O keyboard shortcut
    press_key(&mut state, Key::O, true, false, true, false);
    state.assert_invariants();
    // 2 overlapping closed rectangles merge into 1 closed 12-corner '+' contour + 1 untouched open contour = 2 contours
    assert_eq!(state.contours.len(), 2, "2 overlapping bars + 1 open path must yield 1 merged closed contour + 1 open contour");
    let closed_idx = (0..state.contours.len()).find(|&i| state.is_contour_closed(i)).unwrap();
    let open_idx = (0..state.contours.len()).find(|&i| !state.is_contour_closed(i)).unwrap();
    assert_eq!(
        state.contours[closed_idx].len(),
        12,
        "Merged '+' cross must have exactly 12 Corner nodes"
    );
    assert_eq!(state.contours[open_idx].len(), 2, "Open contour must remain untouched");
    assert!(
        grafia::contour_signed_area(&state.contours[closed_idx]) > 0.0,
        "Outer merged contour must have CCW positive signed area"
    );

    // Undo / Redo roundtrip
    assert!(state.undo());
    assert_eq!(state.contours.len(), 3);
    assert!(state.redo());
    assert_eq!(state.contours.len(), 2);

    // 2. Adjacent `PixelTool` cells sharing edges:
    // Stamp 3 horizontal adjacent 50x50 cells at (100,100), (150,100), (200,100) -> Remove Overlap merges into 1 clean 4-node [100..250, 100..150] rectangle!
    let mut state_px = GlifoCanvasState::new(GlifoTheme::sonoma_default());
    state_px.viewport.zoom = 0.5;
    state_px.viewport.pan_offset = ScreenPoint::new(100.0, 500.0);
    state_px.grid_step_upm = 50.0;
    press_key(&mut state_px, Key::X, false, false, false, false);
    for x in [110.0, 160.0, 210.0] {
        let scr = state_px.viewport.to_screen(FontPoint::new(x, 110.0));
        pointer_press(&mut state_px, scr, false, false, false, false);
        pointer_release(&mut state_px, scr, false, false, false, false);
    }
    assert_eq!(state_px.contours.len(), 3);
    state_px.selected_nodes.clear();
    assert!(state_px.remove_overlap());
    state_px.assert_invariants();
    assert_eq!(
        state_px.contours.len(),
        1,
        "3 adjacent pixel cells must merge into 1 closed contour"
    );
    assert_eq!(
        state_px.contours[0].len(),
        4,
        "Collinear outer edges of merged pixel row must be simplified to 4 Corner nodes (`removeCornersFromPaths:`)"
    );

    // 3. Overlapping curved `Circle` cubic Bézier contours + enclosed CW counter preservation:
    let mut state_circles = GlifoCanvasState::new(GlifoTheme::sonoma_default());
    state_circles.viewport.zoom = 0.5;
    state_circles.viewport.pan_offset = ScreenPoint::new(100.0, 500.0);
    // Outer Circle 1: [100..400, 100..400]
    assert!(state_circles.insert_primitive(
        PrimitiveSubTool::Circle,
        FontPoint::new(100.0, 100.0),
        300.0,
        300.0,
    ));
    // Outer Circle 2 overlapping Circle 1: [250..550, 100..400]
    assert!(state_circles.insert_primitive(
        PrimitiveSubTool::Circle,
        FontPoint::new(250.0, 100.0),
        300.0,
        300.0,
    ));
    // Enclosed CW counter hole inside Circle 1: [160..230, 210..290]
    assert!(state_circles.insert_primitive(
        PrimitiveSubTool::Rectangle,
        FontPoint::new(160.0, 210.0),
        70.0,
        80.0,
    ));
    // Orient the enclosed hole CW (via `correct_path_direction` or `reverse_contour_nodes`)
    grafia::core::segment_ops::reverse_contour_nodes(&mut state_circles.contours[2], true);
    state_circles.sync_paths_from_contours();
    state_circles.selected_nodes.clear();

    assert!(
        state_circles.remove_overlap(),
        "Overlapping cubic circles must be merged by remove_overlap"
    );
    state_circles.assert_invariants();
    assert_eq!(
        state_circles.contours.len(),
        2,
        "2 overlapping outer circles + 1 enclosed CW hole must yield 1 merged outer contour + 1 preserved CW counter"
    );
    // Outer contour must still contain Cubic OffCurve handles and span x in [100..550]
    let outer_c = &state_circles.contours[0];
    assert!(outer_c.iter().any(|n| n.node_type == NodeType::OffCurve));
    let min_x = outer_c.iter().map(|n| n.point.x.0).fold(f64::INFINITY, f64::min);
    let max_x = outer_c.iter().map(|n| n.point.x.0).fold(f64::NEG_INFINITY, f64::max);
    assert!((min_x - 100.0).abs() < 2.0 && (max_x - 550.0).abs() < 2.0);
    // Inner counter must have negative (CW) signed area
    assert!(grafia::contour_signed_area(&state_circles.contours[1]) < 0.0);

    // 4. Direct boolean `subtract_contours` and `intersect_contours` verification:
    let mut next_id = 1000;
    let rect_a = grafia::build_rectangle_contour_nodes(100.0, 100.0, 400.0, 400.0, &mut next_id);
    let rect_b = grafia::build_rectangle_contour_nodes(250.0, 200.0, 500.0, 300.0, &mut next_id);
    let diff = grafia::subtract_contours(std::slice::from_ref(&rect_a), std::slice::from_ref(&rect_b), &mut next_id);
    assert_eq!(diff.len(), 1);
    assert_eq!(diff[0].len(), 8, "Subtracting side notch from rectangle yields an 8-corner C-shape");
    let inter = grafia::intersect_contours(std::slice::from_ref(&rect_a), std::slice::from_ref(&rect_b), &mut next_id);
    assert_eq!(inter.len(), 1);
    assert_eq!(inter[0].len(), 4, "Intersection of [100..400,100..400] and [250..500,200..300] is [250..400,200..300]");

    // 5. Single-contour self-intersection removal (figure-8 bow-tie contour -> 2 CCW triangles):
    let mut state_fig8 = GlifoCanvasState::new(GlifoTheme::sonoma_default());
    let bow_tie = vec![
        Node::new(NodeId(1), 100.0, 100.0, NodeType::Corner),
        Node::new(NodeId(2), 400.0, 400.0, NodeType::Corner),
        Node::new(NodeId(3), 400.0, 100.0, NodeType::Corner),
        Node::new(NodeId(4), 100.0, 400.0, NodeType::Corner),
    ];
    state_fig8.next_node_id = 10;
    state_fig8.contours = vec![bow_tie.clone()];
    state_fig8.glyph.contours = vec![grafia::GlyphContour::new(grafia::ContourId(0), bow_tie)];
    state_fig8.sync_paths_from_contours();
    assert!(
        state_fig8.remove_overlap(),
        "Single self-intersecting figure-8 contour must be decomposed by remove_overlap"
    );
    state_fig8.assert_invariants();
    assert_eq!(
        state_fig8.contours.len(),
        2,
        "Figure-8 self-intersecting contour must split at (250,250) into 2 closed triangle contours"
    );
    assert!(state_fig8.contours.iter().all(|c| c.len() == 3 && grafia::contour_signed_area(c) > 0.0));

    // 6. Completely contained redundant positive CCW contour + coincident identical Circle contours:
    let mut state_contained = GlifoCanvasState::new(GlifoTheme::sonoma_default());
    assert!(state_contained.insert_primitive(
        PrimitiveSubTool::Rectangle,
        FontPoint::new(100.0, 100.0),
        400.0,
        400.0,
    ));
    assert!(state_contained.insert_primitive(
        PrimitiveSubTool::Rectangle,
        FontPoint::new(200.0, 200.0),
        100.0,
        100.0,
    ));
    state_contained.selected_nodes.clear();
    assert!(
        state_contained.remove_overlap(),
        "Redundant positive CCW contour strictly inside outer CCW contour must be removed"
    );
    state_contained.assert_invariants();
    assert_eq!(state_contained.contours.len(), 1);
    assert_eq!(state_contained.contours[0].len(), 4);

    let mut state_coincident = GlifoCanvasState::new(GlifoTheme::sonoma_default());
    for _ in 0..2 {
        assert!(state_coincident.insert_primitive(
            PrimitiveSubTool::Circle,
            FontPoint::new(100.0, 100.0),
            300.0,
            300.0,
        ));
    }
    state_coincident.selected_nodes.clear();
    assert!(
        state_coincident.remove_overlap(),
        "Two identical coincident Circle contours must deduplicate into 1 Circle contour"
    );
    state_coincident.assert_invariants();
    assert_eq!(state_coincident.contours.len(), 1);
}

// ============================================================================
// TC-V040-022: Multi-Hit `KnifeTool` (`E`) Slicing (`>= 4` Intersections Across
// Multi-Stem `/n` Contour and `/o` Outer CCW + Inner CW Counter Ring)
// ============================================================================
#[test]
fn tc_v040_022_knife_tool_multi_hit_four_intersections_on_glyph_n_and_glyph_o_counter_ring() {
    // Part 1: Single closed contour `/n` sliced horizontally at y = 200 across both stems (4 intersections)
    let mut state_n = GlifoCanvasState::init_glyph_n();
    assert_eq!(state_n.contours.len(), 1);
    press_key(&mut state_n, Key::E, false, false, false, false);

    let n_left = state_n.viewport.to_screen(FontPoint::new(-50.0, 200.0));
    let n_right = state_n.viewport.to_screen(FontPoint::new(650.0, 200.0));
    pointer_press(&mut state_n, n_left, true, false, false, false);
    pointer_move(&mut state_n, n_right, true, false, false, false);
    assert_eq!(
        state_n.v040.knife.intersections.len(),
        4,
        "Horizontal knife cut at y=200 across /n must detect 4 stem intersections"
    );
    pointer_release(&mut state_n, n_right, true, false, false, false);
    state_n.assert_invariants();

    assert_eq!(
        state_n.contours.len(),
        3,
        "Slicing /n across both stems at y=200 must split 1 contour into 3 closed contours (top arch + 2 bottom stems)"
    );
    // Verify 2 contours lie in y <= 200 (bottom stems) and 1 contour lies in y >= 200 (top arch)
    let bottom_stems = state_n
        .contours
        .iter()
        .filter(|c| c.iter().all(|n| n.point.y.0 <= 200.5))
        .count();
    let top_arch = state_n
        .contours
        .iter()
        .filter(|c| c.iter().all(|n| n.point.y.0 >= 199.5))
        .count();
    assert_eq!(bottom_stems, 2, "Must have 2 bottom stem contours below y=200");
    assert_eq!(top_arch, 1, "Must have 1 top arch contour above y=200");

    // Part 2: Outer + inner counter ring `/o` sliced horizontally at y = 250 (2 hits on outer + 2 hits on inner = 4 hits)
    let mut state_o = GlifoCanvasState::init_glyph_o();
    assert_eq!(state_o.contours.len(), 2);
    // Normalize `/o` path direction first (`outer` CCW, `inner` CW)
    let _ = state_o.correct_path_direction();
    assert!(grafia::contour_signed_area(&state_o.contours[0]) > 0.0);
    assert!(grafia::contour_signed_area(&state_o.contours[1]) < 0.0);

    press_key(&mut state_o, Key::E, false, false, false, false);
    let o_left = state_o.viewport.to_screen(FontPoint::new(-50.0, 250.0));
    let o_right = state_o.viewport.to_screen(FontPoint::new(650.0, 250.0));
    pointer_press(&mut state_o, o_left, true, false, false, false);
    pointer_move(&mut state_o, o_right, true, false, false, false);
    assert_eq!(
        state_o.v040.knife.intersections.len(),
        4,
        "Horizontal knife cut at y=250 across /o must detect 4 intersections (outer left, inner left, inner right, outer right)"
    );
    pointer_release(&mut state_o, o_right, true, false, false, false);
    state_o.assert_invariants();

    assert_eq!(
        state_o.contours.len(),
        2,
        "Slicing /o across both walls must bridge outer+inner rings along (hit[0]..hit[1]) and (hit[2]..hit[3]) into 2 closed arch contours"
    );
    let o_bottom = state_o
        .contours
        .iter()
        .filter(|c| c.iter().all(|n| n.point.y.0 <= 250.5))
        .count();
    let o_top = state_o
        .contours
        .iter()
        .filter(|c| c.iter().all(|n| n.point.y.0 >= 249.5))
        .count();
    assert_eq!(o_bottom, 1, "Must produce 1 bottom arch contour (y <= 250)");
    assert_eq!(o_top, 1, "Must produce 1 top arch contour (y >= 250)");

    // Undo / Redo roundtrip on `/o`
    assert!(state_o.undo());
    assert_eq!(state_o.contours.len(), 2);
    assert!(state_o.redo());
    assert_eq!(state_o.contours.len(), 2);
    state_o.assert_invariants();

    // Part 3: Partial knife cut across ONE wall of `/o` (from outside at (-50, 350) into the hollow counter at (350, 350))
    // 1 hit on outer ring + 1 hit on inner counter ring = 2 hits paired across distinct rings -> bridges into 1 closed C-contour!
    assert!(state_o.undo());
    let o_mid = state_o.viewport.to_screen(FontPoint::new(350.0, 350.0));
    let o_left_350 = state_o.viewport.to_screen(FontPoint::new(-50.0, 350.0));
    pointer_press(&mut state_o, o_left_350, true, false, false, false);
    pointer_move(&mut state_o, o_mid, true, false, false, false);
    assert_eq!(state_o.v040.knife.intersections.len(), 2);
    pointer_release(&mut state_o, o_mid, true, false, false, false);
    state_o.assert_invariants();
    assert_eq!(
        state_o.contours.len(),
        1,
        "Slicing across one wall of /o (1 outer hit + 1 inner hit) must bridge outer and inner rings into 1 closed C-shaped contour"
    );
    assert!(state_o.is_contour_closed(0));
}




