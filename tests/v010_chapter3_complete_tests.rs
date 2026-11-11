//! Comprehensive COTS Integration & Behavioral Verification Suite for Vector Canvas Handbook Chapter 3
//! and Reference Vector Specification Verification (`SPEC-002` Rev. 3).

use egui::{Event, Key, Modifiers as EguiModifiers, PointerButton, Pos2, RawInput, Rect};
use grafia::adapters::headless::{pixmap_to_rgba_image, rasterize_canvas_subregion};
use grafia::adapters::input_dispatcher::InputDispatcher;
use grafia::adapters::state::{CanvasTool, GlifoCanvasState};
use grafia::core::{
    extract_segments, find_nearest_cubic_extremum, ContourSegment, FontPoint, Node, NodeId,
    NodeType, ScreenPoint,
};
use grafia::interaction::{PenToolState, SelectToolState};
use grafia::render::{compute_open_contour_endpoint_ticks, compute_start_node_arrow_triangle};
use grafia::tokens::GlifoTheme;
use kurbo::{ParamCurve, ParamCurveDeriv};
use rstest::rstest;

fn canvas_rect() -> Rect {
    Rect::from_min_max(Pos2::new(0.0, 0.0), Pos2::new(1280.0, 800.0))
}

#[allow(clippy::too_many_arguments)]
fn run_dispatch(
    state: &mut GlifoCanvasState,
    events: Vec<Event>,
    shift: bool,
    alt: bool,
    cmd: bool,
    ctrl: bool,
    space_down: bool,
    time: f64,
) -> bool {
    let ctx = egui::Context::default();
    let mods = EguiModifiers {
        alt,
        ctrl,
        shift,
        mac_cmd: cmd,
        command: cmd,
    };
    let mut full_events = events;
    if space_down {
        full_events.insert(
            0,
            Event::Key {
                key: Key::Space,
                physical_key: None,
                pressed: true,
                repeat: false,
                modifiers: mods,
            },
        );
    }
    let raw = RawInput {
        screen_rect: Some(canvas_rect()),
        time: Some(time),
        events: full_events,
        ..Default::default()
    };
    ctx.input_mut(|i| i.modifiers = mods);
    let theme = state.theme.clone();
    let mut changed = false;
    let mut full_output = ctx.run_ui(raw, |ui| {
        let events_clone = ui.input(|i| i.events.clone());
        let mut disp = InputDispatcher::new(state, &theme, canvas_rect());
        changed = disp.dispatch_events(&events_clone, ui);
    });
    full_output.textures_delta.clear();
    changed
}

fn press_key(state: &mut GlifoCanvasState, key: Key, shift: bool, alt: bool, cmd: bool) -> bool {
    let mods = EguiModifiers {
        alt,
        ctrl: cmd,
        shift,
        mac_cmd: cmd,
        command: cmd,
    };
    run_dispatch(
        state,
        vec![Event::Key {
            key,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: mods,
        }],
        shift,
        alt,
        cmd,
        cmd,
        false,
        1.0,
    )
}

fn assert_all_views_consistent(state: &GlifoCanvasState) {
    state.assert_invariants();
    assert_eq!(state.contours.len(), state.paths.len());
    assert_eq!(state.contours.len(), state.glyph.contours.len());
    let pix_edit = rasterize_canvas_subregion(state, &state.theme, 320, 240);
    assert_eq!(pix_edit.width(), 320);
}

// ============================================================================
// A. Full Undo / Redo (`Cmd/Ctrl+Z`, `Cmd/Ctrl+Shift+Z`, `Cmd/Ctrl+Y`)
// ============================================================================

#[test]
fn test_spec002_a_full_undo_redo_across_all_mutations() {
    let mut state = GlifoCanvasState::init_glyph_o();
    let initial_contours = state.contours.clone();
    let initial_closed = state.closed_flags();

    // 1. Nudge node 0 via keyboard
    let n0_id = state.contours[0][0].id;
    state.selected_nodes = vec![n0_id];
    let p0_before = state.contours[0][0].point;
    assert!(press_key(&mut state, Key::ArrowRight, true, false, false));
    assert!((state.contours[0][0].point.x.0 - (p0_before.x.0 + 10.0)).abs() < 1e-6);

    // Undo via Cmd+Z restores exact position
    assert!(press_key(&mut state, Key::Z, false, false, true));
    assert_eq!(state.contours[0][0].point, p0_before);

    // Redo via Cmd+Shift+Z re-applies nudge
    assert!(press_key(&mut state, Key::Z, true, false, true));
    assert!((state.contours[0][0].point.x.0 - (p0_before.x.0 + 10.0)).abs() < 1e-6);

    // Undo via Cmd+Z then Redo via Cmd+Y
    assert!(press_key(&mut state, Key::Z, false, false, true));
    assert_eq!(state.contours[0][0].point, p0_before);
    assert!(press_key(&mut state, Key::Y, false, false, true));
    assert!((state.contours[0][0].point.x.0 - (p0_before.x.0 + 10.0)).abs() < 1e-6);
    assert!(press_key(&mut state, Key::Z, false, false, true));

    // 2. Shift+Delete (cut contour open) -> Undo restores closed contour
    state.selected_nodes = vec![n0_id];
    assert!(press_key(&mut state, Key::Delete, true, false, false));
    assert!(!state.is_contour_closed(0));
    assert!(press_key(&mut state, Key::Z, false, false, true));
    assert_eq!(state.closed_flags(), initial_closed);
    assert_eq!(state.contours, initial_contours);

    // 3. Cmd+Shift+R (Reverse contour) -> Undo restores winding order
    assert!(press_key(&mut state, Key::R, true, false, true));
    assert_ne!(state.contours, initial_contours);
    assert!(press_key(&mut state, Key::Z, false, false, true));
    assert_eq!(state.contours, initial_contours);
}

// ============================================================================
// B. Pen Tool (`P` / `CanvasTool`) — Persistent Cubics, Alt Cusp, Shift Extremum, Resume Open
// ============================================================================

#[test]
fn test_spec002_b1_pen_click_and_drag_creates_persistent_cubic_beziers() {
    let mut state = GlifoCanvasState::new(GlifoTheme::sonoma_default());
    state.viewport.zoom = 1.0;
    state.viewport.pan_offset = ScreenPoint::new(100.0, 600.0);
    state.active_tool = CanvasTool::Pen;

    // 1. Click at (100, 200) in font space -> places first Corner anchor
    let scr1 = state.viewport.to_screen(FontPoint::new(100.0, 200.0));
    run_dispatch(
        &mut state,
        vec![
            Event::PointerButton {
                pos: Pos2::new(scr1.x.0, scr1.y.0),
                button: PointerButton::Primary,
                pressed: true,
                modifiers: EguiModifiers::NONE,
            },
            Event::PointerButton {
                pos: Pos2::new(scr1.x.0, scr1.y.0),
                button: PointerButton::Primary,
                pressed: false,
                modifiers: EguiModifiers::NONE,
            },
        ],
        false,
        false,
        false,
        false,
        false,
        1.0,
    );
    assert_eq!(state.contours.len(), 1);
    assert_eq!(state.contours[0].len(), 1);
    assert_eq!(state.contours[0][0].node_type, NodeType::Corner);

    // 2. Click at (300, 400) and drag to (360, 400) -> extrudes bcp_in (240, 400) and bcp_out (360, 400)
    let scr2 = state.viewport.to_screen(FontPoint::new(300.0, 400.0));
    let scr2_drag = state.viewport.to_screen(FontPoint::new(360.0, 400.0));
    run_dispatch(
        &mut state,
        vec![
            Event::PointerButton {
                pos: Pos2::new(scr2.x.0, scr2.y.0),
                button: PointerButton::Primary,
                pressed: true,
                modifiers: EguiModifiers::NONE,
            },
            Event::PointerMoved(Pos2::new(scr2_drag.x.0, scr2_drag.y.0)),
            Event::PointerButton {
                pos: Pos2::new(scr2_drag.x.0, scr2_drag.y.0),
                button: PointerButton::Primary,
                pressed: false,
                modifiers: EguiModifiers::NONE,
            },
        ],
        false,
        false,
        false,
        false,
        false,
        2.0,
    );

    assert_eq!(state.contours[0].len(), 4);
    assert_eq!(state.contours[0][1].node_type, NodeType::OffCurve);
    assert_eq!(state.contours[0][2].node_type, NodeType::OffCurve);
    assert!((state.contours[0][2].point.x.0 - 240.0).abs() < 1e-3);
    assert_eq!(state.contours[0][3].node_type, NodeType::Smooth);
    assert_eq!(state.pending_pen_bcp_out, Some((0, FontPoint::new(360.0, 400.0))));

    // 3. Click at (500, 200) -> consumes pending_pen_bcp_out (360, 400) so the second segment is also Cubic!
    let scr3 = state.viewport.to_screen(FontPoint::new(500.0, 200.0));
    run_dispatch(
        &mut state,
        vec![
            Event::PointerButton {
                pos: Pos2::new(scr3.x.0, scr3.y.0),
                button: PointerButton::Primary,
                pressed: true,
                modifiers: EguiModifiers::NONE,
            },
            Event::PointerButton {
                pos: Pos2::new(scr3.x.0, scr3.y.0),
                button: PointerButton::Primary,
                pressed: false,
                modifiers: EguiModifiers::NONE,
            },
        ],
        false,
        false,
        false,
        false,
        false,
        3.0,
    );

    let segs = extract_segments(&state.contours[0], false);
    assert_eq!(segs.len(), 2, "Must have 2 persistent cubic segments");
    assert!(matches!(segs[0], ContourSegment::Cubic { .. }));
    assert!(matches!(segs[1], ContourSegment::Cubic { .. }));
}

#[test]
fn test_spec002_b2_pen_alt_during_drag_creates_cusp_corner_and_freezes_bcp_in() {
    let mut state = GlifoCanvasState::new(GlifoTheme::sonoma_default());
    state.viewport.zoom = 1.0;
    state.viewport.pan_offset = ScreenPoint::new(100.0, 600.0);
    state.active_tool = CanvasTool::Pen;

    let scr1 = state.viewport.to_screen(FontPoint::new(100.0, 100.0));
    run_dispatch(
        &mut state,
        vec![
            Event::PointerButton {
                pos: Pos2::new(scr1.x.0, scr1.y.0),
                button: PointerButton::Primary,
                pressed: true,
                modifiers: EguiModifiers::NONE,
            },
            Event::PointerButton {
                pos: Pos2::new(scr1.x.0, scr1.y.0),
                button: PointerButton::Primary,
                pressed: false,
                modifiers: EguiModifiers::NONE,
            },
        ],
        false,
        false,
        false,
        false,
        false,
        1.0,
    );

    let scr2 = state.viewport.to_screen(FontPoint::new(300.0, 300.0));
    let scr2_sym = state.viewport.to_screen(FontPoint::new(350.0, 300.0));
    run_dispatch(
        &mut state,
        vec![
            Event::PointerButton {
                pos: Pos2::new(scr2.x.0, scr2.y.0),
                button: PointerButton::Primary,
                pressed: true,
                modifiers: EguiModifiers::NONE,
            },
            Event::PointerMoved(Pos2::new(scr2_sym.x.0, scr2_sym.y.0)),
        ],
        false,
        false,
        false,
        false,
        false,
        2.0,
    );

    let scr2_alt = state.viewport.to_screen(FontPoint::new(300.0, 420.0));
    run_dispatch(
        &mut state,
        vec![
            Event::PointerMoved(Pos2::new(scr2_alt.x.0, scr2_alt.y.0)),
            Event::PointerButton {
                pos: Pos2::new(scr2_alt.x.0, scr2_alt.y.0),
                button: PointerButton::Primary,
                pressed: false,
                modifiers: EguiModifiers { alt: true, ..Default::default() },
            },
        ],
        false,
        true,
        false,
        false,
        false,
        2.5,
    );

    let last_anchor = state.contours[0].last().expect("last anchor");
    assert_eq!(
        last_anchor.node_type,
        NodeType::Corner,
        "Alt during Pen drag must convert anchor to Corner"
    );
    let bcp_in = &state.contours[0][state.contours[0].len() - 2];
    assert!(
        (bcp_in.point.x.0 - 250.0).abs() < 1e-3 && (bcp_in.point.y.0 - 300.0).abs() < 1e-3,
        "bcp_in must remain frozen at pre-Alt position (250, 300), got {:?}",
        bcp_in.point
    );
    let (_, bcp_out) = state.pending_pen_bcp_out.expect("pending bcp_out");
    assert!(
        (bcp_out.x.0 - 300.0).abs() < 1e-3 && (bcp_out.y.0 - 420.0).abs() < 1e-3,
        "bcp_out must follow cursor to (300, 420), got {:?}",
        bcp_out
    );
}

#[test]
fn test_spec002_b3_shift_pen_hover_and_click_snaps_to_cubic_extremum() {
    let mut state = GlifoCanvasState::init_glyph_o();
    // Delete the top extremum node of /o so the top arch spans a single cubic with an interior extremum at t = 0.5
    let top_id = state.contours[0][0].id;
    state.selected_nodes = vec![top_id];
    assert!(press_key(&mut state, Key::Delete, false, false, false));

    state.active_tool = CanvasTool::Pen;

    let segs = extract_segments(&state.contours[0], true);
    // Find the cubic segment that has an interior extremum near t = 0.5
    let arch_seg = segs
        .into_iter()
        .find(|s| {
            if let ContourSegment::Cubic { cubic, .. } = s {
                let (et, _) = find_nearest_cubic_extremum(*cubic, 0.42);
                (et - 0.5).abs() < 0.05
            } else {
                false
            }
        })
        .expect("Deleted top node must leave an arch cubic with interior extremum at t=0.5");

    let ContourSegment::Cubic { cubic, .. } = arch_seg else {
        panic!("Expected cubic segment");
    };
    let (ext_t, ext_pt) = find_nearest_cubic_extremum(cubic, 0.42);

    let deriv = cubic.deriv().eval(ext_t);
    assert!(
        deriv.x.abs() < 1e-4 || deriv.y.abs() < 1e-4,
        "Extremum derivative must have dx/dt=0 or dy/dt=0, got ({}, {})",
        deriv.x,
        deriv.y
    );

    let hover_pt_raw = cubic.eval(0.42);
    let hover_scr = state.viewport.to_screen(FontPoint::new(hover_pt_raw.x, hover_pt_raw.y));
    run_dispatch(
        &mut state,
        vec![Event::PointerMoved(Pos2::new(hover_scr.x.0, hover_scr.y.0))],
        true,
        false,
        false,
        false,
        false,
        1.0,
    );
    let hit = state.hovered_segment_hit.expect("hovered segment hit");
    assert!((hit.t - ext_t).abs() < 1e-4);
    assert!((hit.point.x.0 - ext_pt.x.0).abs() < 1e-2);
    assert!((hit.point.y.0 - ext_pt.y.0).abs() < 1e-2);
}

#[test]
fn test_spec002_b4_pen_resumes_and_closes_open_contour_after_shift_delete() {
    let mut state = GlifoCanvasState::init_glyph_a();
    let node0_id = state.contours[0][0].id;
    state.selected_nodes = vec![node0_id];
    assert!(press_key(&mut state, Key::Delete, true, false, false));
    assert!(!state.is_contour_closed(0));

    state.active_tool = CanvasTool::Pen;
    let last_pt = state.contours[0].last().unwrap().point;
    let first_pt = state.contours[0].first().unwrap().point;
    let last_scr = state.viewport.to_screen(last_pt);
    let first_scr = state.viewport.to_screen(first_pt);

    run_dispatch(
        &mut state,
        vec![
            Event::PointerButton {
                pos: Pos2::new(last_scr.x.0, last_scr.y.0),
                button: PointerButton::Primary,
                pressed: true,
                modifiers: EguiModifiers::NONE,
            },
            Event::PointerButton {
                pos: Pos2::new(last_scr.x.0, last_scr.y.0),
                button: PointerButton::Primary,
                pressed: false,
                modifiers: EguiModifiers::NONE,
            },
        ],
        false,
        false,
        false,
        false,
        false,
        1.0,
    );
    assert_eq!(state.active_open_contour, Some(0));

    run_dispatch(
        &mut state,
        vec![
            Event::PointerButton {
                pos: Pos2::new(first_scr.x.0, first_scr.y.0),
                button: PointerButton::Primary,
                pressed: true,
                modifiers: EguiModifiers::NONE,
            },
            Event::PointerButton {
                pos: Pos2::new(first_scr.x.0, first_scr.y.0),
                button: PointerButton::Primary,
                pressed: false,
                modifiers: EguiModifiers::NONE,
            },
        ],
        false,
        false,
        false,
        false,
        false,
        2.0,
    );
    assert!(state.is_contour_closed(0), "Clicking first node of resumed open contour 0 must close it");
    assert_eq!(state.pen_state, PenToolState::Idle);
}

// ============================================================================
// C. Select Tool (`V` / `SelectTool`) — Reference Segment-Interaction Matrix & Modifiers
// ============================================================================

#[test]
fn test_spec002_c1_reference_segment_interaction_matrix_line_and_cubic() {
    let mut state_a = GlifoCanvasState::init_glyph_a();
    let p0_before = state_a.contours[0][0].point;
    let p1_before = state_a.contours[0][1].point;
    let p0_id = state_a.contours[0][0].id;
    let p1_id = state_a.contours[0][1].id;
    let mid_line = FontPoint::new(
        (p0_before.x.0 + p1_before.x.0) * 0.5,
        (p0_before.y.0 + p1_before.y.0) * 0.5,
    );
    let mid_scr = state_a.viewport.to_screen(mid_line);

    run_dispatch(
        &mut state_a,
        vec![
            Event::PointerButton {
                pos: Pos2::new(mid_scr.x.0, mid_scr.y.0),
                button: PointerButton::Primary,
                pressed: true,
                modifiers: EguiModifiers::NONE,
            },
            Event::PointerButton {
                pos: Pos2::new(mid_scr.x.0, mid_scr.y.0),
                button: PointerButton::Primary,
                pressed: false,
                modifiers: EguiModifiers::NONE,
            },
        ],
        false,
        false,
        false,
        false,
        false,
        1.0,
    );
    assert_eq!(
        state_a.selected_nodes,
        vec![p0_id, p1_id],
        "Single click on unselected Line segment must select its 2 OnCurve endpoints"
    );

    let drag_target_scr = state_a
        .viewport
        .to_screen(FontPoint::new(mid_line.x.0 + 40.0, mid_line.y.0));
    run_dispatch(
        &mut state_a,
        vec![
            Event::PointerButton {
                pos: Pos2::new(mid_scr.x.0, mid_scr.y.0),
                button: PointerButton::Primary,
                pressed: true,
                modifiers: EguiModifiers::NONE,
            },
            Event::PointerMoved(Pos2::new(drag_target_scr.x.0, drag_target_scr.y.0)),
            Event::PointerButton {
                pos: Pos2::new(drag_target_scr.x.0, drag_target_scr.y.0),
                button: PointerButton::Primary,
                pressed: false,
                modifiers: EguiModifiers::NONE,
            },
        ],
        false,
        false,
        false,
        false,
        false,
        3.0,
    );
    assert!((state_a.contours[0][0].point.x.0 - (p0_before.x.0 + 40.0)).abs() < 1e-2);
    assert!((state_a.contours[0][1].point.x.0 - (p1_before.x.0 + 40.0)).abs() < 1e-2);

    let mut state_o = GlifoCanvasState::init_glyph_o();
    let segs_o = extract_segments(&state_o.contours[0], true);
    let ContourSegment::Cubic {
        start_idx,
        h1_idx,
        h2_idx,
        end_idx,
        cubic,
    } = segs_o[0]
    else {
        panic!("Expected cubic segment on /o");
    };
    let p0 = state_o.contours[0][start_idx].point;
    let h1 = state_o.contours[0][h1_idx].point;
    let h2 = state_o.contours[0][h2_idx].point;
    let p3 = state_o.contours[0][end_idx].point;

    let mid_curve = cubic.eval(0.5);
    let mid_curve_scr = state_o
        .viewport
        .to_screen(FontPoint::new(mid_curve.x, mid_curve.y));
    let drag_curve_scr = state_o
        .viewport
        .to_screen(FontPoint::new(mid_curve.x - 30.0, mid_curve.y + 30.0));

    run_dispatch(
        &mut state_o,
        vec![
            Event::PointerButton {
                pos: Pos2::new(mid_curve_scr.x.0, mid_curve_scr.y.0),
                button: PointerButton::Primary,
                pressed: true,
                modifiers: EguiModifiers::NONE,
            },
            Event::PointerMoved(Pos2::new(drag_curve_scr.x.0, drag_curve_scr.y.0)),
            Event::PointerButton {
                pos: Pos2::new(drag_curve_scr.x.0, drag_curve_scr.y.0),
                button: PointerButton::Primary,
                pressed: false,
                modifiers: EguiModifiers::NONE,
            },
        ],
        false,
        false,
        false,
        false,
        false,
        1.0,
    );
    assert_eq!(state_o.contours[0][start_idx].point, p0);
    assert_eq!(state_o.contours[0][end_idx].point, p3);
    assert_ne!(state_o.contours[0][h1_idx].point, h1);
    assert_ne!(state_o.contours[0][h2_idx].point, h2);

    state_o.undo();
    run_dispatch(
        &mut state_o,
        vec![
            Event::PointerButton {
                pos: Pos2::new(mid_curve_scr.x.0, mid_curve_scr.y.0),
                button: PointerButton::Primary,
                pressed: true,
                modifiers: EguiModifiers::NONE,
            },
            Event::PointerButton {
                pos: Pos2::new(mid_curve_scr.x.0, mid_curve_scr.y.0),
                button: PointerButton::Primary,
                pressed: false,
                modifiers: EguiModifiers::NONE,
            },
        ],
        false,
        false,
        false,
        false,
        false,
        3.0,
    );
    assert_eq!(
        state_o.selected_nodes.len(),
        4,
        "Single click on unselected Cubic segment must select all 4 nodes (start_on, h1, h2, end_on)"
    );
}

#[test]
fn test_spec002_c2_delete_on_single_off_curve_bcp_converts_cubic_to_straight_line() {
    let mut state = GlifoCanvasState::init_glyph_o();
    let count_before = state.contours[0].len();
    assert_eq!(state.contours[0][1].node_type, NodeType::OffCurve);
    assert_eq!(state.contours[0][2].node_type, NodeType::OffCurve);
    let h1_id = state.contours[0][1].id;
    state.selected_nodes = vec![h1_id];

    assert!(press_key(&mut state, Key::Delete, false, false, false));
    assert_eq!(
        state.contours[0].len(),
        count_before - 2,
        "Deleting one OffCurve handle must remove BOTH OffCurve handles of that cubic segment"
    );
    let segs = extract_segments(&state.contours[0], true);
    assert!(
        matches!(segs[0], ContourSegment::Line { .. }),
        "First segment must now be a straight Line segment"
    );
}

#[test]
fn test_spec002_c3_and_c4_shift_marquee_xor_and_alt_marquee_on_curve_only() {
    let mut state = GlifoCanvasState::init_glyph_o();
    let all_on_curve_ids: Vec<NodeId> = state
        .contours
        .iter()
        .flat_map(|c| c.iter().filter(|n| n.node_type != NodeType::OffCurve).map(|n| n.id))
        .collect();

    run_dispatch(
        &mut state,
        vec![
            Event::PointerButton {
                pos: Pos2::new(10.0, 10.0),
                button: PointerButton::Primary,
                pressed: true,
                modifiers: EguiModifiers { alt: true, ..Default::default() },
            },
            Event::PointerMoved(Pos2::new(1200.0, 750.0)),
            Event::PointerButton {
                pos: Pos2::new(1200.0, 750.0),
                button: PointerButton::Primary,
                pressed: false,
                modifiers: EguiModifiers { alt: true, ..Default::default() },
            },
        ],
        false,
        true,
        false,
        false,
        false,
        1.0,
    );
    assert_eq!(
        state.selected_nodes, all_on_curve_ids,
        "Alt + Marquee must select only OnCurve nodes and ignore all OffCurve BCP handles"
    );

    run_dispatch(
        &mut state,
        vec![
            Event::PointerButton {
                pos: Pos2::new(10.0, 10.0),
                button: PointerButton::Primary,
                pressed: true,
                modifiers: EguiModifiers { shift: true, ..Default::default() },
            },
            Event::PointerMoved(Pos2::new(1200.0, 750.0)),
            Event::PointerButton {
                pos: Pos2::new(1200.0, 750.0),
                button: PointerButton::Primary,
                pressed: false,
                modifiers: EguiModifiers { shift: true, ..Default::default() },
            },
        ],
        true,
        false,
        false,
        false,
        false,
        2.0,
    );
    for id in &all_on_curve_ids {
        assert!(
            !state.selected_nodes.contains(id),
            "Shift + Marquee XOR must deselect previously selected OnCurve node {:?}",
            id
        );
    }
    assert!(
        !state.selected_nodes.is_empty(),
        "Shift + Marquee XOR must select the previously unselected OffCurve nodes"
    );
}

#[test]
fn test_spec002_c5_ctrl_alt_drag_mirrors_opposite_smooth_handle_symmetrically() {
    let mut state = GlifoCanvasState::new(GlifoTheme::sonoma_default());
    state.contours = vec![vec![
        Node::new(NodeId(1), 0.0, 0.0, NodeType::Corner),
        Node::new(NodeId(2), 0.0, 55.0, NodeType::OffCurve),
        Node::new(NodeId(3), 45.0, 100.0, NodeType::OffCurve),
        Node::new(NodeId(4), 100.0, 100.0, NodeType::Smooth),
        Node::new(NodeId(5), 155.0, 100.0, NodeType::OffCurve),
        Node::new(NodeId(6), 200.0, 55.0, NodeType::OffCurve),
        Node::new(NodeId(7), 200.0, 0.0, NodeType::Corner),
    ]];
    state.active_contour_closed = true;
    state.sync_paths_from_contours();

    state.selected_nodes = vec![NodeId(3)];
    state.record_drag_start();
    state.apply_node_displacement_with_modifiers_and_ctrl(20.0, 30.0, false, true, true);
    assert_eq!(
        state.contours[0][2].point,
        FontPoint::new(65.0, 130.0),
        "Dragged handle (45,100) + (20,30) must move to (65,130)"
    );
    assert_eq!(
        state.contours[0][4].point,
        FontPoint::new(135.0, 70.0),
        "Ctrl+Alt handle drag at Smooth anchor (100,100) must mirror opposite handle (155,100) to (135,70)"
    );
}

#[test]
fn test_spec002_c6_and_c7_triple_click_and_two_stage_cmd_a_and_cmd_d() {
    let mut state = GlifoCanvasState::init_glyph_o();
    let total_on_curve = state
        .contours
        .iter()
        .flat_map(|c| c.iter())
        .filter(|n| n.node_type != NodeType::OffCurve)
        .count();
    let total_all = state.contours.iter().map(|c| c.len()).sum::<usize>();

    assert!(press_key(&mut state, Key::A, false, false, true));
    assert_eq!(state.selected_nodes.len(), total_on_curve);

    assert!(press_key(&mut state, Key::A, false, false, true));
    assert_eq!(state.selected_nodes.len(), total_all);

    // Escape must clear selection WITHOUT opening any closed contour!
    assert!(press_key(&mut state, Key::Escape, false, false, false));
    assert!(state.selected_nodes.is_empty());
    assert!(state.is_contour_closed(0), "Escape must not cut contour 0 open");
    assert!(state.is_contour_closed(1), "Escape must not cut last contour 1 open");

    assert!(press_key(&mut state, Key::A, false, false, true));
    assert_eq!(state.selected_nodes.len(), total_on_curve);
    assert!(press_key(&mut state, Key::D, false, false, true));
    assert!(state.selected_nodes.is_empty());

    let scr0 = state.viewport.to_screen(state.contours[0][0].point);
    for t in [1.0, 1.1, 1.2] {
        run_dispatch(
            &mut state,
            vec![
                Event::PointerButton {
                    pos: Pos2::new(scr0.x.0, scr0.y.0),
                    button: PointerButton::Primary,
                    pressed: true,
                    modifiers: EguiModifiers::NONE,
                },
                Event::PointerButton {
                    pos: Pos2::new(scr0.x.0, scr0.y.0),
                    button: PointerButton::Primary,
                    pressed: false,
                    modifiers: EguiModifiers::NONE,
                },
            ],
            false,
            false,
            false,
            false,
            false,
            t,
        );
    }
    assert_eq!(
        state.selected_nodes.len(),
        total_all,
        "Triple-click must select all nodes across all contours in the glyph"
    );
}

// ============================================================================
// D. Global Viewport Navigation (Spacebar Pan, Zoom at Cursor, Cmd + =/-/0)
// ============================================================================

#[rstest]
#[case(ScreenPoint::new(250.0, 180.0), 1.5)]
#[case(ScreenPoint::new(640.0, 400.0), 0.75)]
#[case(ScreenPoint::new(980.0, 620.0), 2.2)]
fn test_spec002_d_viewport_zoom_at_cursor_invariance_and_space_pan(
    #[case] cursor_scr: ScreenPoint,
    #[case] factor: f32,
) {
    let mut state = GlifoCanvasState::init_glyph_a();
    let font_before = state.viewport.to_font(cursor_scr);
    state.viewport.zoom_at_cursor(cursor_scr, factor);
    let font_after = state.viewport.to_font(cursor_scr);
    assert!((font_before.x.0 - font_after.x.0).abs() < 1e-2);
    assert!((font_before.y.0 - font_after.y.0).abs() < 1e-2);

    let pan_before = state.viewport.pan_offset;
    run_dispatch(
        &mut state,
        vec![
            Event::PointerButton {
                pos: Pos2::new(300.0, 300.0),
                button: PointerButton::Primary,
                pressed: true,
                modifiers: EguiModifiers::NONE,
            },
            Event::PointerMoved(Pos2::new(375.0, 260.0)),
            Event::PointerButton {
                pos: Pos2::new(375.0, 260.0),
                button: PointerButton::Primary,
                pressed: false,
                modifiers: EguiModifiers::NONE,
            },
        ],
        false,
        false,
        false,
        false,
        true,
        1.0,
    );
    assert!((state.viewport.pan_offset.x.0 - (pan_before.x.0 + 75.0)).abs() < 1e-3);
    assert!((state.viewport.pan_offset.y.0 - (pan_before.y.0 - 40.0)).abs() < 1e-3);
    assert_eq!(state.select_state, SelectToolState::Idle);

    let z0 = state.viewport.zoom;
    assert!(press_key(&mut state, Key::Plus, false, false, true));
    assert!(state.viewport.zoom > z0);
    assert!(press_key(&mut state, Key::Minus, false, false, true));
    assert!((state.viewport.zoom - z0).abs() < 1e-3);
    assert!(press_key(&mut state, Key::Num0, false, false, true));
}

// ============================================================================
// E. Visual Vector Canvas Edit View Appearance (Metrics Box, Start Node Arrow, Open Endpoints)
// ============================================================================

#[test]
fn test_spec002_e_v010_edit_view_visual_elements_and_rasterization() {
    let mut state = GlifoCanvasState::init_glyph_a();
    state.fit_viewport_to_glyph(800.0, 600.0);

    let tri = compute_start_node_arrow_triangle(&state.contours[0], true, &state.viewport);
    assert!(tri.is_some(), "Start-node direction arrow triangle must be computed for contour 0");

    state.set_contour_closed(0, false);
    let ticks = compute_open_contour_endpoint_ticks(&state.contours[0], &state.viewport);
    assert_eq!(
        ticks.len(),
        2,
        "Open contour with >= 2 nodes must have 2 perpendicular endpoint cross-ticks"
    );

    state.contours.push(vec![Node::new(
        NodeId(999),
        300.0,
        350.0,
        NodeType::Corner,
    )]);
    state.sync_paths_from_contours();
    let pixmap = rasterize_canvas_subregion(&state, &state.theme, 800, 600);
    let img = pixmap_to_rgba_image(pixmap);
    assert_eq!(img.width(), 800);
    assert_eq!(img.height(), 600);
}

fn count_diff_pixels(a: &image::RgbaImage, b: &image::RgbaImage) -> usize {
    assert_eq!(a.dimensions(), b.dimensions());
    a.pixels()
        .zip(b.pixels())
        .filter(|(p1, p2)| p1 != p2)
        .count()
}

// ============================================================================
// F. Exhaustive v0.1.0 Behavioral & Visual Pixel-Delta Verification Tests
// ============================================================================

#[test]
fn test_spec002_f1_alt_drag_on_smooth_on_curve_node_decouples_handles_and_preserves_smooth_type() {
    let mut state = GlifoCanvasState::init_glyph_o();
    state.fit_viewport_to_glyph(640.0, 480.0);
    let before_img = pixmap_to_rgba_image(rasterize_canvas_subregion(&state, &state.theme, 640, 480));

    let anchor_before = state.contours[0][0].point;
    let h_out_before = state.contours[0][1].point;
    let h_in_before = state.contours[0][11].point;
    assert_eq!(state.contours[0][0].node_type, NodeType::Smooth);

    let scr0 = state.viewport.to_screen(anchor_before);
    let scr1 = state
        .viewport
        .to_screen(FontPoint::new(anchor_before.x.0, anchor_before.y.0 + 75.0));
    let alt_mods = EguiModifiers { alt: true, ..Default::default() };

    // Step 1: Mouse-down with Alt on Smooth node 0 MUST NOT toggle Smooth -> Corner or push phantom history!
    run_dispatch(
        &mut state,
        vec![Event::PointerButton {
            pos: Pos2::new(scr0.x.0, scr0.y.0),
            button: PointerButton::Primary,
            pressed: true,
            modifiers: alt_mods,
        }],
        false,
        true,
        false,
        false,
        false,
        1.0,
    );
    assert_eq!(
        state.contours[0][0].node_type,
        NodeType::Smooth,
        "Alt + PrimaryDown on a Smooth OnCurve node must NOT toggle Smooth to Corner on mouse-down"
    );
    assert!(
        !state.history.can_undo(),
        "Alt + PrimaryDown must not push a phantom Toggle Smooth/Corner entry before dragging"
    );

    // Step 2: Drag and release with Alt held
    run_dispatch(
        &mut state,
        vec![
            Event::PointerMoved(Pos2::new(scr1.x.0, scr1.y.0)),
            Event::PointerButton {
                pos: Pos2::new(scr1.x.0, scr1.y.0),
                button: PointerButton::Primary,
                pressed: false,
                modifiers: alt_mods,
            },
        ],
        false,
        true,
        false,
        false,
        false,
        1.1,
    );

    assert_eq!(
        state.contours[0][0].node_type,
        NodeType::Smooth,
        "Alt + Drag on a Smooth OnCurve node must preserve NodeType::Smooth (must NOT convert to Corner)"
    );
    assert!((state.contours[0][0].point.y.0 - (anchor_before.y.0 + 75.0)).abs() < 1e-2);
    assert_eq!(
        state.contours[0][1].point, h_out_before,
        "Outgoing OffCurve handle must remain fixed in world space during Alt + Drag on OnCurve anchor"
    );
    assert_eq!(
        state.contours[0][11].point, h_in_before,
        "Incoming OffCurve handle must remain fixed in world space during Alt + Drag on OnCurve anchor"
    );

    let after_img = pixmap_to_rgba_image(rasterize_canvas_subregion(&state, &state.theme, 640, 480));
    assert!(count_diff_pixels(&before_img, &after_img) > 200);

    // Single Undo restores pre-drag state and leaves history empty (1 drag = 1 undo transaction)
    assert!(state.undo());
    assert_eq!(state.contours[0][0].point, anchor_before);
    assert_eq!(state.contours[0][0].node_type, NodeType::Smooth);
    assert!(!state.history.can_undo(), "Single Undo after Alt+Drag must leave undo stack empty");
}

#[test]
fn test_spec002_f2_option_drag_on_selected_contour_duplicates_contour() {
    let mut state = GlifoCanvasState::init_glyph_o();
    state.fit_viewport_to_glyph(640.0, 480.0);
    let before_img = pixmap_to_rgba_image(rasterize_canvas_subregion(&state, &state.theme, 640, 480));

    // Select all nodes of outer contour 0
    state.selected_nodes = state.contours[0].iter().map(|n| n.id).collect();
    let orig_c0 = state.contours[0].clone();
    let scr0 = state.viewport.to_screen(state.contours[0][0].point);
    let scr1 = state.viewport.to_screen(FontPoint::new(
        state.contours[0][0].point.x.0 + 140.0,
        state.contours[0][0].point.y.0 + 60.0,
    ));
    let alt_mods = EguiModifiers { alt: true, ..Default::default() };

    run_dispatch(
        &mut state,
        vec![
            Event::PointerButton {
                pos: Pos2::new(scr0.x.0, scr0.y.0),
                button: PointerButton::Primary,
                pressed: true,
                modifiers: alt_mods,
            },
            Event::PointerMoved(Pos2::new(scr1.x.0, scr1.y.0)),
            Event::PointerButton {
                pos: Pos2::new(scr1.x.0, scr1.y.0),
                button: PointerButton::Primary,
                pressed: false,
                modifiers: alt_mods,
            },
        ],
        false,
        true,
        false,
        false,
        false,
        1.0,
    );

    assert_eq!(
        state.contours.len(),
        3,
        "Option/Alt + Drag on a selected contour must duplicate the contour"
    );
    assert_eq!(
        state.contours[0], orig_c0,
        "Original contour 0 must remain untouched at its original coordinates"
    );
    assert!(
        (state.contours[2][0].point.x.0 - (orig_c0[0].point.x.0 + 140.0)).abs() < 1e-2,
        "Duplicated contour must be translated by the drag delta"
    );
    let after_img = pixmap_to_rgba_image(rasterize_canvas_subregion(&state, &state.theme, 640, 480));
    assert!(count_diff_pixels(&before_img, &after_img) > 500);
}

#[test]
fn test_spec002_f3_shift_drag_45_degree_axis_constrain_on_node_handle_and_pen() {
    let mut state = GlifoCanvasState::init_glyph_o();
    let p0 = state.contours[0][0].point;
    let scr0 = state.viewport.to_screen(p0);
    let scr_drag = state.viewport.to_screen(FontPoint::new(p0.x.0 + 80.0, p0.y.0 + 12.0));
    let shift_mods = EguiModifiers { shift: true, ..Default::default() };

    // 1. Shift + Drag on OnCurve node
    run_dispatch(
        &mut state,
        vec![
            Event::PointerButton {
                pos: Pos2::new(scr0.x.0, scr0.y.0),
                button: PointerButton::Primary,
                pressed: true,
                modifiers: shift_mods,
            },
            Event::PointerMoved(Pos2::new(scr_drag.x.0, scr_drag.y.0)),
            Event::PointerButton {
                pos: Pos2::new(scr_drag.x.0, scr_drag.y.0),
                button: PointerButton::Primary,
                pressed: false,
                modifiers: shift_mods,
            },
        ],
        true,
        false,
        false,
        false,
        false,
        1.0,
    );
    assert!(
        (state.contours[0][0].point.y.0 - p0.y.0).abs() < 1e-3,
        "Shift + Drag on OnCurve node with near-horizontal vector must lock strictly to 0° horizontal axis"
    );

    // 2. Shift + Drag on OffCurve BCP handle (locks displacement to 45° diagonal when dx ≈ dy)
    let h1_before = state.contours[0][1].point;
    let scr_h1 = state.viewport.to_screen(h1_before);
    let scr_h1_drag = state.viewport.to_screen(FontPoint::new(h1_before.x.0 + 60.0, h1_before.y.0 + 52.0));
    run_dispatch(
        &mut state,
        vec![
            Event::PointerButton {
                pos: Pos2::new(scr_h1.x.0, scr_h1.y.0),
                button: PointerButton::Primary,
                pressed: true,
                modifiers: shift_mods,
            },
            Event::PointerMoved(Pos2::new(scr_h1_drag.x.0, scr_h1_drag.y.0)),
            Event::PointerButton {
                pos: Pos2::new(scr_h1_drag.x.0, scr_h1_drag.y.0),
                button: PointerButton::Primary,
                pressed: false,
                modifiers: shift_mods,
            },
        ],
        true,
        false,
        false,
        false,
        false,
        2.0,
    );
    let h1_dx = (state.contours[0][1].point.x.0 - h1_before.x.0).abs();
    let h1_dy = (state.contours[0][1].point.y.0 - h1_before.y.0).abs();
    assert!(
        (h1_dx - h1_dy).abs() < 1e-2,
        "Shift + Drag on OffCurve handle with diagonal vector must snap strictly to 45° (dx={h1_dx}, dy={h1_dy})"
    );

    // 3. Shift + Drag in PenTool handle extrusion (DraggingHandles)
    let mut pen_state = GlifoCanvasState::new(GlifoTheme::sonoma_default());
    pen_state.viewport.zoom = 1.0;
    pen_state.viewport.pan_offset = ScreenPoint::new(100.0, 600.0);
    pen_state.active_tool = CanvasTool::Pen;
    let pen_p0 = pen_state.viewport.to_screen(FontPoint::new(200.0, 200.0));
    let pen_p1 = pen_state.viewport.to_screen(FontPoint::new(400.0, 300.0));
    let pen_p1_drag = pen_state.viewport.to_screen(FontPoint::new(490.0, 310.0));
    run_dispatch(
        &mut pen_state,
        vec![
            Event::PointerButton {
                pos: Pos2::new(pen_p0.x.0, pen_p0.y.0),
                button: PointerButton::Primary,
                pressed: true,
                modifiers: EguiModifiers::NONE,
            },
            Event::PointerButton {
                pos: Pos2::new(pen_p0.x.0, pen_p0.y.0),
                button: PointerButton::Primary,
                pressed: false,
                modifiers: EguiModifiers::NONE,
            },
            Event::PointerButton {
                pos: Pos2::new(pen_p1.x.0, pen_p1.y.0),
                button: PointerButton::Primary,
                pressed: true,
                modifiers: shift_mods,
            },
            Event::PointerMoved(Pos2::new(pen_p1_drag.x.0, pen_p1_drag.y.0)),
            Event::PointerButton {
                pos: Pos2::new(pen_p1_drag.x.0, pen_p1_drag.y.0),
                button: PointerButton::Primary,
                pressed: false,
                modifiers: shift_mods,
            },
        ],
        true,
        false,
        false,
        false,
        false,
        3.0,
    );
    let (_, bcp_out) = pen_state.pending_pen_bcp_out.expect("pending_pen_bcp_out");
    assert!(
        (bcp_out.y.0 - 300.0).abs() < 1e-3,
        "Shift + Drag in PenTool must constrain extruded bcp_out strictly to 0° horizontal axis, got {:?}",
        bcp_out
    );
}

#[test]
fn test_spec002_f4_pen_tool_cmd_temporary_select_tool_override_and_resume_open_path() {
    let mut state = GlifoCanvasState::new(GlifoTheme::sonoma_default());
    state.viewport.zoom = 1.0;
    state.viewport.pan_offset = ScreenPoint::new(100.0, 600.0);
    state.active_tool = CanvasTool::Pen;

    // 1. Place 2 open points in PenTool
    for (t, pt) in [(1.0, FontPoint::new(100.0, 100.0)), (2.0, FontPoint::new(250.0, 250.0))] {
        let scr = state.viewport.to_screen(pt);
        run_dispatch(
            &mut state,
            vec![
                Event::PointerButton {
                    pos: Pos2::new(scr.x.0, scr.y.0),
                    button: PointerButton::Primary,
                    pressed: true,
                    modifiers: EguiModifiers::NONE,
                },
                Event::PointerButton {
                    pos: Pos2::new(scr.x.0, scr.y.0),
                    button: PointerButton::Primary,
                    pressed: false,
                    modifiers: EguiModifiers::NONE,
                },
            ],
            false,
            false,
            false,
            false,
            false,
            t,
        );
    }
    assert_eq!(state.contours[0].len(), 2);

    // 2. Hold Cmd while still in PenTool -> temporarily drag node 0 via SelectTool!
    let scr_n0 = state.viewport.to_screen(FontPoint::new(100.0, 100.0));
    let scr_n0_moved = state.viewport.to_screen(FontPoint::new(100.0, 180.0));
    let cmd_mods = EguiModifiers { command: true, mac_cmd: true, ..Default::default() };
    run_dispatch(
        &mut state,
        vec![
            Event::PointerButton {
                pos: Pos2::new(scr_n0.x.0, scr_n0.y.0),
                button: PointerButton::Primary,
                pressed: true,
                modifiers: cmd_mods,
            },
            Event::PointerMoved(Pos2::new(scr_n0_moved.x.0, scr_n0_moved.y.0)),
            Event::PointerButton {
                pos: Pos2::new(scr_n0_moved.x.0, scr_n0_moved.y.0),
                button: PointerButton::Primary,
                pressed: false,
                modifiers: cmd_mods,
            },
        ],
        false,
        false,
        true,
        false,
        false,
        3.0,
    );
    assert!((state.contours[0][0].point.y.0 - 180.0).abs() < 1e-2);
    assert_eq!(state.active_tool, CanvasTool::Pen);

    // 3. Release Cmd and click at (400, 150) -> continues appending to the SAME open contour 0!
    let scr3 = state.viewport.to_screen(FontPoint::new(400.0, 150.0));
    run_dispatch(
        &mut state,
        vec![
            Event::PointerButton {
                pos: Pos2::new(scr3.x.0, scr3.y.0),
                button: PointerButton::Primary,
                pressed: true,
                modifiers: EguiModifiers::NONE,
            },
            Event::PointerButton {
                pos: Pos2::new(scr3.x.0, scr3.y.0),
                button: PointerButton::Primary,
                pressed: false,
                modifiers: EguiModifiers::NONE,
            },
        ],
        false,
        false,
        false,
        false,
        false,
        4.0,
    );
    assert_eq!(state.contours.len(), 1);
    assert_eq!(state.contours[0].len(), 3);
}

#[test]
fn test_spec002_f5_drag_curve_all_four_endpoint_combinations_and_alt_override() {
    use grafia::core::apply_cubic_segment_belly_drag_with_alt;

    for (s0, s3, alt) in [
        (NodeType::Smooth, NodeType::Smooth, false),
        (NodeType::Corner, NodeType::Corner, false),
        (NodeType::Smooth, NodeType::Corner, false),
        (NodeType::Corner, NodeType::Smooth, false),
        (NodeType::Smooth, NodeType::Smooth, true),
    ] {
        let mut contour = vec![
            Node::new(NodeId(1), 0.0, 0.0, s0),
            Node::new(NodeId(2), 0.0, 100.0, NodeType::OffCurve),
            Node::new(NodeId(3), 200.0, 100.0, NodeType::OffCurve),
            Node::new(NodeId(4), 200.0, 0.0, s3),
        ];
        let seg = extract_segments(&contour, false)[0];
        let h1_init = contour[1].point;
        let h2_init = contour[2].point;
        apply_cubic_segment_belly_drag_with_alt(
            &mut contour,
            false,
            seg,
            0.5,
            h1_init,
            h2_init,
            40.0,
            60.0,
            alt,
        );
        if s0 == NodeType::Smooth && !alt {
            assert!(
                (contour[1].point.x.0 - 0.0).abs() < 1e-4,
                "Smooth start endpoint without Alt must constrain H1 along vertical tangent x=0"
            );
        } else {
            assert!(
                contour[1].point.x.0 > 10.0,
                "Corner start endpoint or Alt override must allow H1 to move horizontally"
            );
        }
        if s3 == NodeType::Smooth && !alt {
            assert!(
                (contour[2].point.x.0 - 200.0).abs() < 1e-4,
                "Smooth end endpoint without Alt must constrain H2 along vertical tangent x=200"
            );
        } else {
            assert!(
                contour[2].point.x.0 > 210.0,
                "Corner end endpoint or Alt override must allow H2 to move horizontally"
            );
        }
    }
}

#[test]
fn test_spec002_f6_pen_tool_close_open_contour_by_clicking_and_dragging_node_0_and_mid_drag_undo() {
    let mut state = GlifoCanvasState::new(GlifoTheme::sonoma_default());
    state.viewport.zoom = 1.0;
    state.viewport.pan_offset = ScreenPoint::new(100.0, 600.0);
    state.active_tool = CanvasTool::Pen;

    let p0 = FontPoint::new(150.0, 150.0);
    let p1 = FontPoint::new(350.0, 350.0);
    for (t, pt) in [(1.0, p0), (2.0, p1)] {
        let scr = state.viewport.to_screen(pt);
        run_dispatch(
            &mut state,
            vec![
                Event::PointerButton {
                    pos: Pos2::new(scr.x.0, scr.y.0),
                    button: PointerButton::Primary,
                    pressed: true,
                    modifiers: EguiModifiers::NONE,
                },
                Event::PointerButton {
                    pos: Pos2::new(scr.x.0, scr.y.0),
                    button: PointerButton::Primary,
                    pressed: false,
                    modifiers: EguiModifiers::NONE,
                },
            ],
            false,
            false,
            false,
            false,
            false,
            t,
        );
    }

    // Click and drag on node 0 to close smoothly with both bcp_in and bcp_out
    let scr0 = state.viewport.to_screen(p0);
    let scr0_drag = state.viewport.to_screen(FontPoint::new(p0.x.0 - 60.0, p0.y.0 + 40.0));
    run_dispatch(
        &mut state,
        vec![
            Event::PointerButton {
                pos: Pos2::new(scr0.x.0, scr0.y.0),
                button: PointerButton::Primary,
                pressed: true,
                modifiers: EguiModifiers::NONE,
            },
            Event::PointerMoved(Pos2::new(scr0_drag.x.0, scr0_drag.y.0)),
            Event::PointerButton {
                pos: Pos2::new(scr0_drag.x.0, scr0_drag.y.0),
                button: PointerButton::Primary,
                pressed: false,
                modifiers: EguiModifiers::NONE,
            },
        ],
        false,
        false,
        false,
        false,
        false,
        3.0,
    );
    assert!(state.is_contour_closed(0));
    assert_eq!(state.contours[0][0].node_type, NodeType::Smooth);

    // Mid-drag Undo test in SelectTool
    state.active_tool = CanvasTool::Select;
    let snap_before_drag = state.contours.clone();
    run_dispatch(
        &mut state,
        vec![
            Event::PointerButton {
                pos: Pos2::new(scr0.x.0, scr0.y.0),
                button: PointerButton::Primary,
                pressed: true,
                modifiers: EguiModifiers::NONE,
            },
            Event::PointerMoved(Pos2::new(scr0.x.0 + 90.0, scr0.y.0 + 90.0)),
        ],
        false,
        false,
        false,
        false,
        false,
        4.0,
    );
    assert!(matches!(state.select_state, SelectToolState::DraggingNodes { .. }));
    assert!(press_key(&mut state, Key::Z, false, false, true));
    assert_eq!(state.select_state, SelectToolState::Idle);
    assert_eq!(state.contours, snap_before_drag);
}

#[test]
fn test_spec002_f7_reverse_contour_visual_pixel_delta_in_all_three_modes_and_make_node_first() {
    use grafia::adapters::state::CanvasViewMode;
    use grafia::locator::{contour_winding_label, ShapeLocatorRenderer};

    // (a) Closed contour in EditWireframe: Reverse Contour flips the blue direction arrow 180°
    let mut state_w = GlifoCanvasState::init_glyph_a();
    state_w.fit_viewport_to_glyph(640.0, 480.0);
    let img_w_before = pixmap_to_rgba_image(rasterize_canvas_subregion(&state_w, &state_w.theme, 640, 480));
    assert_eq!(contour_winding_label(&state_w.contours[0]), "CW");
    assert_eq!(contour_winding_label(&state_w.contours[1]), "CCW");

    assert!(state_w.reverse_selected_or_all_contours());
    assert_eq!(contour_winding_label(&state_w.contours[0]), "CCW");
    assert_eq!(contour_winding_label(&state_w.contours[1]), "CW");
    let img_w_after = pixmap_to_rgba_image(rasterize_canvas_subregion(&state_w, &state_w.theme, 640, 480));
    assert!(
        count_diff_pixels(&img_w_before, &img_w_after) >= 30,
        "Reversing closed contour in EditWireframe must visibly flip the start-node direction arrow"
    );

    // (b) Closed inner counter of /A in PreviewFilled (Non-Zero Winding): reversing ONLY inner counter fills the hole solid black!
    let mut state_p = GlifoCanvasState::init_glyph_a();
    state_p.fit_viewport_to_glyph(640.0, 480.0);
    state_p.view_mode = CanvasViewMode::PreviewFilled;
    let img_p_before = pixmap_to_rgba_image(rasterize_canvas_subregion(&state_p, &state_p.theme, 640, 480));
    let hole_scr = state_p.viewport.to_screen(FontPoint::new(300.0, 320.0));
    let (hx, hy) = (hole_scr.x.0.round() as u32, hole_scr.y.0.round() as u32);
    assert_eq!(
        img_p_before.get_pixel(hx, hy)[0],
        255,
        "Before reversing inner counter, /A counter hole must be white (winding +1 - 1 = 0)"
    );

    // Select only inner counter 1 and reverse it
    state_p.selected_nodes = vec![state_p.contours[1][0].id];
    assert!(state_p.reverse_selected_or_all_contours());
    let img_p_reversed = pixmap_to_rgba_image(rasterize_canvas_subregion(&state_p, &state_p.theme, 640, 480));
    assert_eq!(
        img_p_reversed.get_pixel(hx, hy)[0],
        0,
        "After reversing inner counter, /A counter hole MUST fill solid black (Non-Zero Winding +1 + 1 = +2)!"
    );
    assert!(count_diff_pixels(&img_p_before, &img_p_reversed) > 1000);

    // Reverse inner counter again -> restores white counter hole!
    assert!(state_p.reverse_selected_or_all_contours());
    let img_p_restored = pixmap_to_rgba_image(rasterize_canvas_subregion(&state_p, &state_p.theme, 640, 480));
    assert_eq!(img_p_restored.get_pixel(hx, hy)[0], 255);

    // (c) Open contour: Reverse Contour jumps the start-node direction arrow to the opposite open endpoint
    let mut state_open = GlifoCanvasState::init_glyph_a();
    state_open.fit_viewport_to_glyph(640.0, 480.0);
    state_open.selected_nodes = vec![state_open.contours[0][0].id];
    assert!(press_key(&mut state_open, Key::Delete, true, false, false));
    assert!(!state_open.is_contour_closed(0));
    let open_first_before = state_open.contours[0].first().unwrap().point;
    let open_last_before = state_open.contours[0].last().unwrap().point;
    let img_open_before = pixmap_to_rgba_image(rasterize_canvas_subregion(&state_open, &state_open.theme, 640, 480));

    state_open.selected_nodes = vec![state_open.contours[0][0].id];
    assert!(state_open.reverse_selected_or_all_contours());
    assert_eq!(state_open.contours[0].first().unwrap().point, open_last_before);
    assert_eq!(state_open.contours[0].last().unwrap().point, open_first_before);
    let img_open_after = pixmap_to_rgba_image(rasterize_canvas_subregion(&state_open, &state_open.theme, 640, 480));
    assert!(
        count_diff_pixels(&img_open_before, &img_open_after) >= 20,
        "Reversing an open contour must jump the direction arrow to the opposite open endpoint"
    );

    // (d) Make Node First (`Cmd+Shift+F` / `make_selected_node_first`) rotates start node on closed contour
    let mut state_mf = GlifoCanvasState::init_glyph_a();
    state_mf.fit_viewport_to_glyph(640.0, 480.0);
    let img_mf_before = pixmap_to_rgba_image(rasterize_canvas_subregion(&state_mf, &state_mf.theme, 640, 480));
    let target_new_first = state_mf.contours[0][2].id;
    state_mf.selected_nodes = vec![target_new_first];
    assert!(press_key(&mut state_mf, Key::F, true, false, true));
    assert_eq!(state_mf.contours[0][0].id, target_new_first);
    let img_mf_after = pixmap_to_rgba_image(rasterize_canvas_subregion(&state_mf, &state_mf.theme, 640, 480));
    assert!(
        count_diff_pixels(&img_mf_before, &img_mf_after) >= 20,
        "Make Node First must move the start-node direction arrow to the newly selected start node"
    );

    // (e) ShapeLocatorRenderer renders true cubic curves on /o (> 20 vertices in tessellated mesh for a 4-anchor oval)
    let state_o = GlifoCanvasState::init_glyph_o();
    let loc_renderer = ShapeLocatorRenderer::new(&state_o.glyph, grafia::locator::ShapeLocatorStyle::default());
    let (ctx_mesh, focus_mesh) = loc_renderer.render_locator(grafia::core::ContourId(0), egui::vec2(96.0, 64.0));
    assert!(ctx_mesh.vertices.len() > 20, "Context locator mesh on cubic /o must have > 20 curve vertices");
    assert!(focus_mesh.vertices.len() > 20, "Focus locator mesh on cubic /o must have > 20 curve vertices");
}

#[test]
fn test_spec002_f8_shift_double_click_multi_contour_reversed_tab_alt_arrow_nudge_and_degenerate_2node_cubic() {
    // 1. Double-Click on contour 0 + Shift + Double-Click on contour 1 selects both contours (24 nodes)
    let mut state = GlifoCanvasState::init_glyph_o();
    let h0_scr = state.viewport.to_screen(state.contours[0][1].point);
    let h1_scr = state.viewport.to_screen(state.contours[1][1].point);
    for t in [1.0, 1.1] {
        run_dispatch(
            &mut state,
            vec![
                Event::PointerButton {
                    pos: Pos2::new(h0_scr.x.0, h0_scr.y.0),
                    button: PointerButton::Primary,
                    pressed: true,
                    modifiers: EguiModifiers::NONE,
                },
                Event::PointerButton {
                    pos: Pos2::new(h0_scr.x.0, h0_scr.y.0),
                    button: PointerButton::Primary,
                    pressed: false,
                    modifiers: EguiModifiers::NONE,
                },
            ],
            false,
            false,
            false,
            false,
            false,
            t,
        );
    }
    assert_eq!(state.selected_nodes.len(), 12);

    let shift_mods = EguiModifiers { shift: true, ..Default::default() };
    for t in [2.0, 2.1] {
        run_dispatch(
            &mut state,
            vec![
                Event::PointerButton {
                    pos: Pos2::new(h1_scr.x.0, h1_scr.y.0),
                    button: PointerButton::Primary,
                    pressed: true,
                    modifiers: shift_mods,
                },
                Event::PointerButton {
                    pos: Pos2::new(h1_scr.x.0, h1_scr.y.0),
                    button: PointerButton::Primary,
                    pressed: false,
                    modifiers: shift_mods,
                },
            ],
            true,
            false,
            false,
            false,
            false,
            t,
        );
    }
    assert_eq!(state.selected_nodes.len(), 24, "Shift + Double-Click on inner ring must add second contour");

    // 2. Tab cycling respects winding direction before and after Reverse Contour
    let mut state_tab = GlifoCanvasState::init_glyph_a();
    let id0 = state_tab.contours[0][0].id;
    let id1_orig = state_tab.contours[0][1].id;
    let id_last_orig = state_tab.contours[0].last().unwrap().id;
    state_tab.selected_nodes = vec![id0];
    assert!(press_key(&mut state_tab, Key::Tab, false, false, false));
    assert_eq!(state_tab.selected_nodes, vec![id1_orig]);

    state_tab.selected_nodes = vec![id0];
    assert!(state_tab.reverse_selected_or_all_contours());
    assert_eq!(state_tab.contours[0][0].id, id0);
    assert!(press_key(&mut state_tab, Key::Tab, false, false, false));
    assert_eq!(
        state_tab.selected_nodes,
        vec![id_last_orig],
        "After Reverse Contour, Tab from node 0 must advance in the reversed winding order"
    );

    // 3. Alt + Arrow nudges OnCurve anchor without moving attached OffCurve handles, and Arrow on OffCurve preserves G1 Smooth
    let mut state_nudge = GlifoCanvasState::init_glyph_o();
    let n0_id = state_nudge.contours[0][0].id;
    let h_out_before = state_nudge.contours[0][1].point;
    let h_in_before = state_nudge.contours[0][11].point;
    state_nudge.selected_nodes = vec![n0_id];
    assert!(press_key(&mut state_nudge, Key::ArrowUp, true, true, false)); // Shift+Alt+Up (+10 upm decoupled)
    assert_eq!(
        state_nudge.contours[0][1].point, h_out_before,
        "Alt+Arrow on OnCurve must keep outgoing OffCurve handle fixed"
    );
    assert_eq!(
        state_nudge.contours[0][11].point, h_in_before,
        "Alt+Arrow on OnCurve must keep incoming OffCurve handle fixed"
    );

    // Nudging an OffCurve handle without Alt rotates the opposite Smooth handle collinearly
    let mut state_h_nudge = GlifoCanvasState::init_glyph_o();
    let h1_id = state_h_nudge.contours[0][1].id;
    let h11_y_before = state_h_nudge.contours[0][11].point.y.0;
    state_h_nudge.selected_nodes = vec![h1_id];
    assert!(press_key(&mut state_h_nudge, Key::ArrowUp, true, false, false));
    assert!(
        state_h_nudge.contours[0][11].point.y.0 < h11_y_before - 1.0,
        "Nudging outgoing OffCurve handle upward must rotate incoming Smooth handle downward"
    );

    // 4. Degenerate 2-node closed cubic contour under Reverse Contour & Make Node First
    let mut state_2n = GlifoCanvasState::new(GlifoTheme::sonoma_default());
    state_2n.contours = vec![vec![
        Node::new(NodeId(1), 100.0, 200.0, NodeType::Smooth),
        Node::new(NodeId(2), 100.0, 350.0, NodeType::OffCurve),
        Node::new(NodeId(3), 300.0, 350.0, NodeType::OffCurve),
        Node::new(NodeId(4), 300.0, 200.0, NodeType::Smooth),
        Node::new(NodeId(5), 300.0, 50.0, NodeType::OffCurve),
        Node::new(NodeId(6), 100.0, 50.0, NodeType::OffCurve),
    ]];
    state_2n.active_contour_closed = true;
    state_2n.sync_paths_from_contours();
    assert!(state_2n.reverse_selected_or_all_contours());
    assert_eq!(state_2n.contours[0][0].id, NodeId(1));
    assert_eq!(state_2n.contours[0][1].id, NodeId(6));
    state_2n.selected_nodes = vec![NodeId(4)];
    assert!(state_2n.make_selected_node_first());
    assert_eq!(state_2n.contours[0][0].id, NodeId(4));
}

#[test]
fn test_spec002_f9_curve_heal_top_extremum_o_pen_close_untwisted_and_multi_contour_closed_authority() {
    // 1. Deleting top Smooth extremum (280, 510) of /o must heal the 180° arch so B(0.5) passes through y = 510.0
    let mut state_o = GlifoCanvasState::init_glyph_o();
    let top_id = state_o.contours[0][0].id;
    assert_eq!(state_o.contours[0][0].point, FontPoint::new(280.0, 510.0));
    state_o.selected_nodes = vec![top_id];
    assert!(press_key(&mut state_o, Key::Delete, false, false, false));

    let segs_o = extract_segments(&state_o.contours[0], true);
    let ContourSegment::Cubic { cubic: top_arch, .. } = *segs_o.last().expect("closing top arch segment") else {
        panic!("Expected healed top arch to be a Cubic segment");
    };
    let mid_arch = top_arch.eval(0.5);
    assert!(
        (495.0..=525.0).contains(&mid_arch.y),
        "Healed top arch midpoint of /o must be in [495.0, 525.0], got {}",
        mid_arch.y
    );
    assert!(
        (mid_arch.x - 280.0).abs() < 0.5 && (mid_arch.y - 510.0).abs() < 0.5,
        "Healed top arch midpoint B(0.5) of /o must match deleted extremum (280.0, 510.0), got ({}, {})",
        mid_arch.x,
        mid_arch.y
    );

    // 1b. Deleting 2 adjacent Smooth nodes on a 3-segment parallel-tangent top arch of /o must ALSO preserve y = 510.0
    let mut state_o3 = GlifoCanvasState::init_glyph_o();
    let top_orig_id = state_o3.contours[0][0].id;
    let seg0_o3 = extract_segments(&state_o3.contours[0], true)[0];
    let split_top_id = grafia::core::split_contour_segment(&mut state_o3.contours[0], seg0_o3, 0.5, &mut state_o3.next_node_id);
    let seg_last_o3 = *extract_segments(&state_o3.contours[0], true).last().unwrap();
    let split_left_id = grafia::core::split_contour_segment(&mut state_o3.contours[0], seg_last_o3, 0.5, &mut state_o3.next_node_id);
    state_o3.sync_paths_from_contours();
    state_o3.selected_nodes = vec![split_left_id, top_orig_id, split_top_id];
    assert!(press_key(&mut state_o3, Key::Delete, false, false, false));
    let segs_o3 = extract_segments(&state_o3.contours[0], true);
    let ContourSegment::Cubic { cubic: top_arch_3, .. } = *segs_o3.last().expect("closing multi-seg healed top arch") else {
        panic!("Expected multi-seg healed top arch to be a Cubic segment");
    };
    let mid_arch_3 = top_arch_3.eval(0.5);
    assert!(
        (495.0..=525.0).contains(&mid_arch_3.y) && (mid_arch_3.y - 510.0).abs() < 0.5,
        "Healed multi-segment top arch midpoint of /o must equal 510.0 (in [495.0, 525.0]), got {}",
        mid_arch_3.y
    );

    // 2. PenTool click-and-drag close on start node 0 must orient bcp_out at cursor, bcp_in opposite,
    //    and align previous Corner outgoing handle h1 collinearly toward bcp_in (zero S-kink)
    let mut pen_s = GlifoCanvasState::new(GlifoTheme::sonoma_default());
    pen_s.viewport.zoom = 1.0;
    pen_s.viewport.pan_offset = ScreenPoint::new(50.0, 650.0);
    pen_s.active_tool = CanvasTool::Pen;

    let p0_f = FontPoint::new(220.0, 170.0);
    let p1_f = FontPoint::new(360.0, 520.0);
    let p1_drag_f = FontPoint::new(470.0, 520.0);
    let p2_f = FontPoint::new(520.0, 220.0);
    let close_drag_f = FontPoint::new(130.0, 250.0);

    let s_p0 = pen_s.viewport.to_screen(p0_f);
    let s_p1 = pen_s.viewport.to_screen(p1_f);
    let s_p1_drag = pen_s.viewport.to_screen(p1_drag_f);
    let s_p2 = pen_s.viewport.to_screen(p2_f);
    let s_close_drag = pen_s.viewport.to_screen(close_drag_f);

    run_dispatch(
        &mut pen_s,
        vec![
            Event::PointerButton { pos: Pos2::new(s_p0.x.0, s_p0.y.0), button: PointerButton::Primary, pressed: true, modifiers: EguiModifiers::NONE },
            Event::PointerButton { pos: Pos2::new(s_p0.x.0, s_p0.y.0), button: PointerButton::Primary, pressed: false, modifiers: EguiModifiers::NONE },
            Event::PointerButton { pos: Pos2::new(s_p1.x.0, s_p1.y.0), button: PointerButton::Primary, pressed: true, modifiers: EguiModifiers::NONE },
            Event::PointerMoved(Pos2::new(s_p1_drag.x.0, s_p1_drag.y.0)),
            Event::PointerButton { pos: Pos2::new(s_p1_drag.x.0, s_p1_drag.y.0), button: PointerButton::Primary, pressed: false, modifiers: EguiModifiers::NONE },
            Event::PointerButton { pos: Pos2::new(s_p2.x.0, s_p2.y.0), button: PointerButton::Primary, pressed: true, modifiers: EguiModifiers::NONE },
            Event::PointerButton { pos: Pos2::new(s_p2.x.0, s_p2.y.0), button: PointerButton::Primary, pressed: false, modifiers: EguiModifiers::NONE },
            Event::PointerButton { pos: Pos2::new(s_p0.x.0, s_p0.y.0), button: PointerButton::Primary, pressed: true, modifiers: EguiModifiers::NONE },
            Event::PointerMoved(Pos2::new(s_close_drag.x.0, s_close_drag.y.0)),
            Event::PointerButton { pos: Pos2::new(s_close_drag.x.0, s_close_drag.y.0), button: PointerButton::Primary, pressed: false, modifiers: EguiModifiers::NONE },
        ],
        false,
        false,
        false,
        false,
        false,
        1.0,
    );

    assert!(pen_s.is_contour_closed(0));
    assert_eq!(pen_s.contours[0][0].node_type, NodeType::Smooth);
    let bcp_out_actual = pen_s.contours[0][1].point;
    let c_len = pen_s.contours[0].len();
    let bcp_in_actual = pen_s.contours[0][c_len - 1].point;
    let close_h1_actual = pen_s.contours[0][c_len - 2].point;
    let expected_bcp_in = FontPoint::new(2.0 * p0_f.x.0 - close_drag_f.x.0, 2.0 * p0_f.y.0 - close_drag_f.y.0);
    let expected_close_h1 = FontPoint::new(
        p2_f.x.0 + (expected_bcp_in.x.0 - p2_f.x.0) / 3.0,
        p2_f.y.0 + (expected_bcp_in.y.0 - p2_f.y.0) / 3.0,
    );
    assert!((bcp_out_actual.x.0 - close_drag_f.x.0).abs() < 1e-3 && (bcp_out_actual.y.0 - close_drag_f.y.0).abs() < 1e-3);
    assert!((bcp_in_actual.x.0 - expected_bcp_in.x.0).abs() < 1e-3 && (bcp_in_actual.y.0 - expected_bcp_in.y.0).abs() < 1e-3);
    assert!(
        (close_h1_actual.x.0 - expected_close_h1.x.0).abs() < 1e-2
            && (close_h1_actual.y.0 - expected_close_h1.y.0).abs() < 1e-2,
        "Closing segment h1 from Corner node must point toward bcp_in {:?}, got {:?}",
        expected_close_h1,
        close_h1_actual
    );

    // 2b. PenTool multi-step drag close on start node 0 when segment 0 was a straight Line (Corner -> Corner)
    //     must update segment 0's auto-synthesized h2 (contours[0][2]) on every PointerMoved event
    let mut pen_line0 = GlifoCanvasState::new(GlifoTheme::sonoma_default());
    pen_line0.viewport.zoom = 1.0;
    pen_line0.viewport.pan_offset = ScreenPoint::new(50.0, 650.0);
    pen_line0.active_tool = CanvasTool::Pen;
    let s_close_step1 = pen_line0.viewport.to_screen(FontPoint::new(210.0, 180.0));
    run_dispatch(
        &mut pen_line0,
        vec![
            Event::PointerButton { pos: Pos2::new(s_p0.x.0, s_p0.y.0), button: PointerButton::Primary, pressed: true, modifiers: EguiModifiers::NONE },
            Event::PointerButton { pos: Pos2::new(s_p0.x.0, s_p0.y.0), button: PointerButton::Primary, pressed: false, modifiers: EguiModifiers::NONE },
            Event::PointerButton { pos: Pos2::new(s_p1.x.0, s_p1.y.0), button: PointerButton::Primary, pressed: true, modifiers: EguiModifiers::NONE },
            Event::PointerButton { pos: Pos2::new(s_p1.x.0, s_p1.y.0), button: PointerButton::Primary, pressed: false, modifiers: EguiModifiers::NONE },
            Event::PointerButton { pos: Pos2::new(s_p2.x.0, s_p2.y.0), button: PointerButton::Primary, pressed: true, modifiers: EguiModifiers::NONE },
            Event::PointerButton { pos: Pos2::new(s_p2.x.0, s_p2.y.0), button: PointerButton::Primary, pressed: false, modifiers: EguiModifiers::NONE },
            Event::PointerButton { pos: Pos2::new(s_p0.x.0, s_p0.y.0), button: PointerButton::Primary, pressed: true, modifiers: EguiModifiers::NONE },
            Event::PointerMoved(Pos2::new(s_close_step1.x.0, s_close_step1.y.0)),
            Event::PointerMoved(Pos2::new(s_close_drag.x.0, s_close_drag.y.0)),
            Event::PointerButton { pos: Pos2::new(s_close_drag.x.0, s_close_drag.y.0), button: PointerButton::Primary, pressed: false, modifiers: EguiModifiers::NONE },
        ],
        false,
        false,
        false,
        false,
        false,
        1.0,
    );
    let seg0_h2_actual = pen_line0.contours[0][2].point;
    let expected_seg0_h2 = FontPoint::new(
        p1_f.x.0 + (close_drag_f.x.0 - p1_f.x.0) / 3.0,
        p1_f.y.0 + (close_drag_f.y.0 - p1_f.y.0) / 3.0,
    );
    assert!(
        (seg0_h2_actual.x.0 - expected_seg0_h2.x.0).abs() < 1e-2
            && (seg0_h2_actual.y.0 - expected_seg0_h2.y.0).abs() < 1e-2,
        "Segment 0 auto-synthesized h2 must track final bcp_out across multiple PointerMoved events: expected {:?}, got {:?}",
        expected_seg0_h2,
        seg0_h2_actual
    );

    // 3. Multi-contour closed state authority: cutting contour 0 open on 2-contour /o must never
    //    corrupt contour 1 or lose open/closed state when active_contour_closed is mutated
    let mut multi = GlifoCanvasState::init_glyph_o();
    let c0_n0 = multi.contours[0][0].id;
    multi.selected_nodes = vec![c0_n0];
    assert!(press_key(&mut multi, Key::Delete, true, false, false));
    assert!(!multi.is_contour_closed(0), "Contour 0 must be open after Shift+Delete");
    assert!(multi.is_contour_closed(1), "Contour 1 must remain closed");
    multi.sync_paths_from_contours();
    assert!(!multi.is_contour_closed(0));
    assert!(multi.is_contour_closed(1));
    assert!(!multi.glyph.contours[0].closed);
    assert!(multi.glyph.contours[1].closed);
}

#[test]
fn test_spec002_f10_calibrated_hit_testing_nearest_node_priority_over_segment() {
    use grafia::tokens::{
        NODE_HIT_TEST_RADIUS_DP, PEN_NODE_HIT_RADIUS_DP, SEGMENT_HIT_TOLERANCE_DP,
        SELECT_NODE_HIT_RADIUS_DP,
    };

    let node_r = std::hint::black_box(NODE_HIT_TEST_RADIUS_DP);
    let select_r = std::hint::black_box(SELECT_NODE_HIT_RADIUS_DP);
    let pen_r = std::hint::black_box(PEN_NODE_HIT_RADIUS_DP);
    let seg_tol = std::hint::black_box(SEGMENT_HIT_TOLERANCE_DP);
    assert!(
        (node_r - 7.5).abs() < 1e-6
            && (select_r - 7.5).abs() < 1e-6
            && (pen_r - 7.0).abs() < 1e-6
            && (seg_tol - 6.0).abs() < 1e-6,
        "Hit-test radii must match Reference (select=7.5, pen=7.0, seg=6.0), got ({select_r}, {pen_r}, {seg_tol})"
    );

    // 1. Clicking 5.5 dp away from node 0 right along segment 0 must hit node 0, NEVER segment 0!
    let mut state = GlifoCanvasState::init_glyph_a();
    let n0_id = state.contours[0][0].id;
    let p0_scr = state.viewport.to_screen(state.contours[0][0].point);
    let p1_scr = state.viewport.to_screen(state.contours[0][1].point);
    let dx = p1_scr.x.0 - p0_scr.x.0;
    let dy = p1_scr.y.0 - p0_scr.y.0;
    let len = dx.hypot(dy);
    let click_near_n0 = ScreenPoint::new(p0_scr.x.0 + (dx / len) * 5.5, p0_scr.y.0 + (dy / len) * 5.5);

    assert_eq!(
        state.hit_test_node(click_near_n0),
        Some(n0_id),
        "Point at 5.5 dp from node 0 must hit node 0 within NODE_HIT_TEST_RADIUS_DP (7.5 dp)"
    );

    run_dispatch(
        &mut state,
        vec![
            Event::PointerButton {
                pos: Pos2::new(click_near_n0.x.0, click_near_n0.y.0),
                button: PointerButton::Primary,
                pressed: true,
                modifiers: EguiModifiers::NONE,
            },
            Event::PointerButton {
                pos: Pos2::new(click_near_n0.x.0, click_near_n0.y.0),
                button: PointerButton::Primary,
                pressed: false,
                modifiers: EguiModifiers::NONE,
            },
        ],
        false,
        false,
        false,
        false,
        false,
        1.0,
    );
    assert_eq!(
        state.selected_nodes,
        vec![n0_id],
        "Clicking 5.5 dp from node 0 along segment 0 must select only node 0, not both endpoints of segment 0"
    );

    // 2. When two nodes are both within NODE_HIT_TEST_RADIUS_DP, hit_test_node returns the NEAREST node
    let mut close_state = GlifoCanvasState::new(GlifoTheme::sonoma_default());
    close_state.viewport.zoom = 1.0;
    close_state.viewport.pan_offset = ScreenPoint::new(100.0, 500.0);
    close_state.contours = vec![vec![
        Node::new(NodeId(1), 100.0, 200.0, NodeType::Corner),
        Node::new(NodeId(2), 105.0, 200.0, NodeType::Corner),
    ]];
    close_state.set_contour_closed(0, false);
    close_state.active_open_contour = Some(0);
    close_state.sync_paths_from_contours();

    let near_second_scr = close_state.viewport.to_screen(FontPoint::new(104.2, 200.0));
    assert_eq!(
        close_state.hit_test_node(near_second_scr),
        Some(NodeId(2)),
        "hit_test_node must return the nearest node (NodeId(2)), not the first node in iteration order"
    );
}

#[test]
fn test_spec002_f11_pen_tool_new_curve_close_priority_drag_threshold_and_first_bcp_in() {
    let mut state = GlifoCanvasState::new(GlifoTheme::sonoma_default());
    state.viewport.zoom = 1.0;
    state.viewport.pan_offset = ScreenPoint::new(50.0, 650.0);
    state.active_tool = CanvasTool::Pen;

    let p0 = FontPoint::new(200.0, 200.0);
    let p0_drag = FontPoint::new(204.0, 200.0); // 4 dp outgoing handle right next to P0!
    let p1 = FontPoint::new(400.0, 450.0);
    let p1_drag = FontPoint::new(480.0, 450.0);
    let p2 = FontPoint::new(480.0, 200.0);

    let s_p0 = state.viewport.to_screen(p0);
    let s_p0_drag = state.viewport.to_screen(p0_drag);
    let s_p1 = state.viewport.to_screen(p1);
    let s_p1_drag = state.viewport.to_screen(p1_drag);
    let s_p2 = state.viewport.to_screen(p2);

    // 1. Place P0 with click+drag (>= DRAG_THRESHOLD_DP = 3.0 dp): must store both pending_pen_bcp_out AND pending_pen_first_bcp_in!
    run_dispatch(
        &mut state,
        vec![
            Event::PointerButton { pos: Pos2::new(s_p0.x.0, s_p0.y.0), button: PointerButton::Primary, pressed: true, modifiers: EguiModifiers::NONE },
            Event::PointerMoved(Pos2::new(s_p0_drag.x.0, s_p0_drag.y.0)),
            Event::PointerButton { pos: Pos2::new(s_p0_drag.x.0, s_p0_drag.y.0), button: PointerButton::Primary, pressed: false, modifiers: EguiModifiers::NONE },
        ],
        false, false, false, false, false, 1.0,
    );
    assert_eq!(state.contours[0][0].node_type, NodeType::Smooth);
    assert_eq!(
        state.pending_pen_first_bcp_in,
        Some((0, FontPoint::new(196.0, 200.0))),
        "Click+dragging first node P0 must preserve symmetric incoming handle in pending_pen_first_bcp_in"
    );

    // 2. Place P1 (click+drag) and P2 (click with 1.5 dp micro-jitter < DRAG_THRESHOLD_DP: must stay Corner!)
    run_dispatch(
        &mut state,
        vec![
            Event::PointerButton { pos: Pos2::new(s_p1.x.0, s_p1.y.0), button: PointerButton::Primary, pressed: true, modifiers: EguiModifiers::NONE },
            Event::PointerMoved(Pos2::new(s_p1_drag.x.0, s_p1_drag.y.0)),
            Event::PointerButton { pos: Pos2::new(s_p1_drag.x.0, s_p1_drag.y.0), button: PointerButton::Primary, pressed: false, modifiers: EguiModifiers::NONE },
            Event::PointerButton { pos: Pos2::new(s_p2.x.0, s_p2.y.0), button: PointerButton::Primary, pressed: true, modifiers: EguiModifiers::NONE },
            Event::PointerMoved(Pos2::new(s_p2.x.0 + 1.5, s_p2.y.0)),
            Event::PointerButton { pos: Pos2::new(s_p2.x.0 + 1.5, s_p2.y.0), button: PointerButton::Primary, pressed: false, modifiers: EguiModifiers::NONE },
        ],
        false, false, false, false, false, 2.0,
    );
    assert_eq!(
        state.contours[0].last().unwrap().node_type,
        NodeType::Corner,
        "Pointer move of 1.5 dp (< DRAG_THRESHOLD_DP = 3.0 dp) must keep P2 as Corner"
    );

    // 3. Clicking an interior node P1 or OffCurve handle in PenTool must select it and NEVER append a duplicate node
    let len_before_interior_click = state.contours[0].len();
    run_dispatch(
        &mut state,
        vec![
            Event::PointerButton { pos: Pos2::new(s_p1.x.0, s_p1.y.0), button: PointerButton::Primary, pressed: true, modifiers: EguiModifiers::NONE },
            Event::PointerButton { pos: Pos2::new(s_p1.x.0, s_p1.y.0), button: PointerButton::Primary, pressed: false, modifiers: EguiModifiers::NONE },
        ],
        false, false, false, false, false, 3.0,
    );
    assert_eq!(
        state.contours[0].len(),
        len_before_interior_click,
        "Clicking interior node P1 in PenTool must NOT fall through and append a duplicate node"
    );

    // 4. Hovering 5.0 dp from P0 (right over segment 0 / outgoing handle at 204,200) must set hovered_pen_close_target
    //    and keep hovered_segment_hit == None, and render a visible close halo!
    let img_no_hover = pixmap_to_rgba_image(rasterize_canvas_subregion(&state, &state.theme, 640, 480));
    let hover_close_scr = ScreenPoint::new(s_p0.x.0 + 5.0, s_p0.y.0);
    run_dispatch(
        &mut state,
        vec![Event::PointerMoved(Pos2::new(hover_close_scr.x.0, hover_close_scr.y.0))],
        false, false, false, false, false, 4.0,
    );
    assert_eq!(
        state.hovered_pen_close_target,
        Some((0, state.contours[0][0].id)),
        "Hovering within NODE_HIT_TEST_RADIUS_DP of P0 must set hovered_pen_close_target"
    );
    assert_eq!(
        state.hovered_segment_hit, None,
        "Hovering near P0 to close must NEVER trigger segment-split ghost node on segment 0"
    );
    let img_close_hover = pixmap_to_rgba_image(rasterize_canvas_subregion(&state, &state.theme, 640, 480));
    assert!(
        count_diff_pixels(&img_no_hover, &img_close_hover) >= 8,
        "Hovering P0 to close must render a visible closing halo around P0"
    );

    // 5. Clicking at hover_close_scr (with 1.0 dp micro-jitter) must close the contour on P0 (NOT split segment 0!),
    //    preserve P0's outgoing handle (204, 200), and insert pending_pen_first_bcp_in (196, 200) as closing h2!
    //    First verify that Undo + Redo before closing preserves BOTH pending_pen_first_bcp_in AND pending_pen_bcp_out!
    let mut undo_pen = GlifoCanvasState::new(GlifoTheme::sonoma_default());
    undo_pen.viewport.zoom = 1.0;
    undo_pen.viewport.pan_offset = ScreenPoint::new(50.0, 650.0);
    undo_pen.active_tool = CanvasTool::Pen;
    run_dispatch(
        &mut undo_pen,
        vec![
            Event::PointerButton { pos: Pos2::new(s_p0.x.0, s_p0.y.0), button: PointerButton::Primary, pressed: true, modifiers: EguiModifiers::NONE },
            Event::PointerMoved(Pos2::new(s_p0_drag.x.0, s_p0_drag.y.0)),
            Event::PointerButton { pos: Pos2::new(s_p0_drag.x.0, s_p0_drag.y.0), button: PointerButton::Primary, pressed: false, modifiers: EguiModifiers::NONE },
            Event::PointerButton { pos: Pos2::new(s_p1.x.0, s_p1.y.0), button: PointerButton::Primary, pressed: true, modifiers: EguiModifiers::NONE },
            Event::PointerMoved(Pos2::new(s_p1_drag.x.0, s_p1_drag.y.0)),
            Event::PointerButton { pos: Pos2::new(s_p1_drag.x.0, s_p1_drag.y.0), button: PointerButton::Primary, pressed: false, modifiers: EguiModifiers::NONE },
        ],
        false, false, false, false, false, 1.0,
    );
    // Resuming on P1 with Alt+Drag must redirect pending_pen_bcp_out WITHOUT collapsing P1's existing incoming H2 or P0's outgoing H1!
    let h1_before_resume = undo_pen.contours[0][1].point;
    let h2_before_resume = undo_pen.contours[0][2].point;
    let s_p1_cusp = undo_pen.viewport.to_screen(FontPoint::new(400.0, 360.0));
    let alt_mods = EguiModifiers { alt: true, ..Default::default() };
    run_dispatch(
        &mut undo_pen,
        vec![
            Event::PointerButton { pos: Pos2::new(s_p1.x.0, s_p1.y.0), button: PointerButton::Primary, pressed: true, modifiers: alt_mods },
            Event::PointerMoved(Pos2::new(s_p1_cusp.x.0, s_p1_cusp.y.0)),
            Event::PointerButton { pos: Pos2::new(s_p1_cusp.x.0, s_p1_cusp.y.0), button: PointerButton::Primary, pressed: false, modifiers: alt_mods },
        ],
        false, true, false, false, false, 2.0,
    );
    assert_eq!(undo_pen.contours[0][1].point, h1_before_resume, "Resuming on P_last must preserve previous node's explicit outgoing H1");
    assert_eq!(undo_pen.contours[0][2].point, h2_before_resume, "Alt+Drag resume on P_last must freeze existing incoming H2");
    assert_eq!(undo_pen.pending_pen_bcp_out, Some((0, FontPoint::new(400.0, 360.0))));
    assert!(undo_pen.undo(), "Undo resume on P1");
    assert_eq!(undo_pen.pending_pen_bcp_out, Some((0, p1_drag)), "Undo must restore previous pending_pen_bcp_out");
    assert!(undo_pen.undo(), "Undo placing P1");
    assert_eq!(undo_pen.pending_pen_bcp_out, Some((0, p0_drag)), "Undoing P1 must restore P0's pending_pen_bcp_out");
    assert_eq!(undo_pen.pending_pen_first_bcp_in, Some((0, FontPoint::new(196.0, 200.0))));

    run_dispatch(
        &mut state,
        vec![
            Event::PointerButton { pos: Pos2::new(hover_close_scr.x.0, hover_close_scr.y.0), button: PointerButton::Primary, pressed: true, modifiers: EguiModifiers::NONE },
            Event::PointerMoved(Pos2::new(hover_close_scr.x.0 + 1.0, hover_close_scr.y.0)),
            Event::PointerButton { pos: Pos2::new(hover_close_scr.x.0 + 1.0, hover_close_scr.y.0), button: PointerButton::Primary, pressed: false, modifiers: EguiModifiers::NONE },
        ],
        false, false, false, false, false, 5.0,
    );
    assert!(state.is_contour_closed(0), "Clicking near P0 must close contour 0 instead of splitting segment 0");
    assert_eq!(state.pending_pen_first_bcp_in, None, "pending_pen_first_bcp_in must be consumed upon closing");
    assert_eq!(
        state.contours[0][1].point,
        FontPoint::new(204.0, 200.0),
        "Closing with a click on P0 must preserve P0's existing outgoing handle"
    );
    assert_eq!(
        state.contours[0][0].node_type,
        NodeType::Corner,
        "Scenario C1: when prevNode was placed without drag, closing on P0 without drag must convert P0 to Corner"
    );
    assert_eq!(
        state.contours[0].last().unwrap().point,
        p2,
        "Scenario C1: when prevNode was placed without drag, closing segment must be a straight Line ending at P2 without extra OffCurve handles"
    );
    assert_all_views_consistent(&state);
}

#[test]
fn test_spec002_f12_authentic_v010_reference_vector_cursors_and_canvas_gating() {
    use grafia::tokens::GlifoCursorKind;

    let mut state = GlifoCanvasState::init_glyph_o();
    let canvas_rect = Rect::from_min_size(Pos2::new(0.0, 0.0), egui::vec2(640.0, 480.0));

    // 1. Outside canvas_rect when idle -> None
    assert_eq!(
        state.desired_cursor_kind(canvas_rect, Some(ScreenPoint::new(700.0, 200.0))),
        None,
        "Pointer outside canvas_rect when idle must not set a canvas cursor"
    );

    // 2. SelectTool over empty canvas -> SelectArrow; over OnCurve/OffCurve node -> SelectElement
    let empty_scr = ScreenPoint::new(20.0, 20.0);
    let node0_scr = state.viewport.to_screen(state.contours[0][0].point);
    assert_eq!(
        state.desired_cursor_kind(canvas_rect, Some(empty_scr)),
        Some(GlifoCursorKind::SelectArrow)
    );
    assert_eq!(
        state.desired_cursor_kind(canvas_rect, Some(node0_scr)),
        Some(GlifoCursorKind::SelectElement)
    );

    // 3. PenTool over empty canvas -> PenStandard; over segment -> PenPlus; over OnCurve/P0 -> PenElement
    state.active_tool = CanvasTool::Pen;
    assert_eq!(
        state.desired_cursor_kind(canvas_rect, Some(empty_scr)),
        Some(GlifoCursorKind::PenStandard)
    );
    assert_eq!(
        state.desired_cursor_kind(canvas_rect, Some(node0_scr)),
        Some(GlifoCursorKind::PenElement)
    );

    let seg0 = extract_segments(&state.contours[0], true)[0];
    let ContourSegment::Cubic { cubic, .. } = seg0 else { panic!("expected cubic"); };
    let mid_pt = cubic.eval(0.5);
    let mid_scr = state.viewport.to_screen(FontPoint::new(mid_pt.x, mid_pt.y));
    run_dispatch(
        &mut state,
        vec![Event::PointerMoved(Pos2::new(mid_scr.x.0, mid_scr.y.0))],
        false, false, false, false, false, 1.0,
    );
    assert_eq!(
        state.desired_cursor_kind(canvas_rect, Some(mid_scr)),
        Some(GlifoCursorKind::PenPlus)
    );

    // 4. Visual Pass 5 rendering in headless.rs when cursor_screen_pos is set
    state.hovered_segment_hit = None;
    state.cursor_screen_pos = None;
    let base_img = pixmap_to_rgba_image(rasterize_canvas_subregion(&state, &state.theme, 640, 480));

    state.cursor_screen_pos = Some(ScreenPoint::new(120.0, 120.0));
    let pen_std_img = pixmap_to_rgba_image(rasterize_canvas_subregion(&state, &state.theme, 640, 480));
    assert!(
        count_diff_pixels(&base_img, &pen_std_img) >= 25,
        "Pass 5 must visibly render the Vector Canvas PenStandard fountain-pen nib vector cursor"
    );

    state.cursor_screen_pos = Some(mid_scr);
    state.hovered_segment_hit = Some(grafia::core::SegmentHit {
        contour_idx: 0,
        segment: seg0,
        t: 0.5,
        point: FontPoint::new(mid_pt.x, mid_pt.y),
        distance_font: 0.0,
    });
    let pen_plus_img = pixmap_to_rgba_image(rasterize_canvas_subregion(&state, &state.theme, 640, 480));
    assert!(
        count_diff_pixels(&base_img, &pen_plus_img) >= 35,
        "Pass 5 must visibly render the Vector Canvas PenPlus cursor and ghost node"
    );

    // 5. Render an open contour with pending_pen_first_bcp_in, hovered_pen_close_target halo, and PenElement cursor
    let mut pen_open = GlifoCanvasState::new(GlifoTheme::sonoma_default());
    pen_open.viewport.zoom = 1.0;
    pen_open.viewport.pan_offset = ScreenPoint::new(40.0, 420.0);
    pen_open.active_tool = CanvasTool::Pen;
    pen_open.contours = vec![vec![
        Node::new(NodeId(1), 120.0, 140.0, NodeType::Smooth),
        Node::new(NodeId(2), 120.0, 260.0, NodeType::OffCurve),
        Node::new(NodeId(3), 240.0, 320.0, NodeType::OffCurve),
        Node::new(NodeId(4), 340.0, 320.0, NodeType::Smooth),
        Node::new(NodeId(5), 440.0, 320.0, NodeType::OffCurve),
        Node::new(NodeId(6), 480.0, 220.0, NodeType::OffCurve),
        Node::new(NodeId(7), 440.0, 120.0, NodeType::Corner),
    ]];
    pen_open.set_contour_closed(0, false);
    pen_open.active_open_contour = Some(0);
    pen_open.pending_pen_first_bcp_in = Some((0, FontPoint::new(120.0, 40.0)));
    pen_open.pending_pen_bcp_out = Some((0, FontPoint::new(340.0, 60.0)));
    pen_open.sync_paths_from_contours();
    let p0_scr = pen_open.viewport.to_screen(FontPoint::new(120.0, 140.0));
    pen_open.hovered_pen_close_target = Some((0, NodeId(1)));
    pen_open.cursor_screen_pos = Some(p0_scr);

    let out_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("target/visual_verification");
    let _ = std::fs::create_dir_all(&out_dir);
    let close_img = pixmap_to_rgba_image(rasterize_canvas_subregion(&pen_open, &pen_open.theme, 640, 480));
    close_img
        .save(out_dir.join("v010_cursors_and_pen_close.png"))
        .expect("Failed to save visual verification PNG");
}

#[test]
fn test_spec002_f13_ctrl_alt_drag_slides_on_curve_node_along_line_and_cubic_preserving_curvature() {
    use kurbo::ParamCurveNearest;

    // 1. Slide an OnCurve node along a straight Line segment on /A with Cmd + Ctrl + Drag
    let mut state_a = GlifoCanvasState::init_glyph_a();
    let p0_orig = state_a.contours[0][0].point;
    let p1_orig = state_a.contours[0][1].point;
    let line0 = kurbo::Line::new(
        kurbo::Point::new(p0_orig.x.0, p0_orig.y.0),
        kurbo::Point::new(p1_orig.x.0, p1_orig.y.0),
    );
    let s0 = state_a.viewport.to_screen(p0_orig);
    let target_line_pt = FontPoint::new(
        p0_orig.x.0 + (p1_orig.x.0 - p0_orig.x.0) * 0.25 + 35.0,
        p0_orig.y.0 + (p1_orig.y.0 - p0_orig.y.0) * 0.25 - 20.0,
    );
    let target_line_scr = state_a.viewport.to_screen(target_line_pt);
    let cmd_ctrl_mods = EguiModifiers { command: true, mac_cmd: true, ctrl: true, ..Default::default() };

    run_dispatch(
        &mut state_a,
        vec![
            Event::PointerButton {
                pos: Pos2::new(s0.x.0, s0.y.0),
                button: PointerButton::Primary,
                pressed: true,
                modifiers: cmd_ctrl_mods,
            },
            Event::PointerMoved(Pos2::new(target_line_scr.x.0, target_line_scr.y.0)),
            Event::PointerButton {
                pos: Pos2::new(target_line_scr.x.0, target_line_scr.y.0),
                button: PointerButton::Primary,
                pressed: false,
                modifiers: cmd_ctrl_mods,
            },
        ],
        false, false, true, true, false, 1.0,
    );

    let p0_slid = state_a.contours[0][0].point;
    assert!(p0_slid.distance_to(p0_orig) > 20.0, "Node 0 must slide along segment 0");
    let nearest_on_line = line0.nearest(kurbo::Point::new(p0_slid.x.0, p0_slid.y.0), 1e-6);
    assert!(
        nearest_on_line.distance_sq.sqrt() < 1e-3,
        "Cmd+Ctrl+Drag on Line segment must constrain node strictly onto the original line, dist={}",
        nearest_on_line.distance_sq.sqrt()
    );

    // 2. Slide an OnCurve node along a Cubic curve on /o with Cmd + Ctrl + Drag preserving curvature
    let mut state_o = GlifoCanvasState::init_glyph_o();
    let orig_segs = extract_segments(&state_o.contours[0], true);
    let ContourSegment::Cubic { cubic: orig_c0, .. } = orig_segs[0] else { panic!("expected cubic"); };
    let top_id = state_o.contours[0][0].id;
    let top_scr = state_o.viewport.to_screen(state_o.contours[0][0].point);
    let slide_target_pt = orig_c0.eval(0.2);
    let slide_target_scr = state_o.viewport.to_screen(FontPoint::new(slide_target_pt.x, slide_target_pt.y));

    run_dispatch(
        &mut state_o,
        vec![
            Event::PointerButton {
                pos: Pos2::new(top_scr.x.0, top_scr.y.0),
                button: PointerButton::Primary,
                pressed: true,
                modifiers: cmd_ctrl_mods,
            },
            Event::PointerMoved(Pos2::new(slide_target_scr.x.0, slide_target_scr.y.0)),
            Event::PointerButton {
                pos: Pos2::new(slide_target_scr.x.0, slide_target_scr.y.0),
                button: PointerButton::Primary,
                pressed: false,
                modifiers: cmd_ctrl_mods,
            },
        ],
        false, false, true, true, false, 2.0,
    );

    let slid_top = state_o.find_node(top_id).unwrap().point;
    assert!(
        slid_top.distance_to(FontPoint::new(280.0, 510.0)) > 10.0,
        "Slid OnCurve node must move along the cubic curve"
    );
    let new_segs = extract_segments(&state_o.contours[0], true);
    let ContourSegment::Cubic { cubic: new_c0, .. } = new_segs[0] else { panic!("expected cubic"); };
    for u in [0.25, 0.5, 0.75] {
        let pt_new = new_c0.eval(u);
        let solve = orig_c0.nearest(pt_new, 1e-6);
        assert!(
            solve.distance_sq.sqrt() < 0.05,
            "Shortened cubic segment after slide must preserve original De Casteljau curvature (err={} at u={u})",
            solve.distance_sq.sqrt()
        );
    }

    let snap_after = state_o.snapshot();
    assert!(state_o.undo());
    assert!(state_o.redo());
    assert_eq!(state_o.snapshot(), snap_after);
    assert_all_views_consistent(&state_o);

    // 3. Asymmetric split (t = 0.25) and heal on a parallel-tangent 180° arch must preserve peak y = 510.0 and symmetry
    let mut state_asym = GlifoCanvasState::init_glyph_o();
    let top_n = state_asym.contours[0][0].id;
    state_asym.selected_nodes = vec![top_n];
    assert!(press_key(&mut state_asym, Key::Delete, false, false, false));
    let arch_seg = *extract_segments(&state_asym.contours[0], true).last().unwrap();
    let asym_id = grafia::core::split_contour_segment(&mut state_asym.contours[0], arch_seg, 0.25, &mut state_asym.next_node_id);
    state_asym.sync_paths_from_contours();
    state_asym.selected_nodes = vec![asym_id];
    assert!(press_key(&mut state_asym, Key::Delete, false, false, false));
    let healed_segs = extract_segments(&state_asym.contours[0], true);
    let ContourSegment::Cubic { cubic: healed_arch, .. } = *healed_segs.last().unwrap() else {
        panic!("Expected healed arch to be Cubic");
    };
    let healed_mid = healed_arch.eval(0.5);
    assert!(
        (healed_mid.x - 280.0).abs() < 1.0 && (healed_mid.y - 510.0).abs() < 1.0,
        "Healing an asymmetric t=0.25 split on parallel-tangent arch must preserve peak (280, 510), got ({}, {})",
        healed_mid.x,
        healed_mid.y
    );

    // 4. Slide on a 2-node closed cubic oval and across a mixed Line + Cubic transition
    let mut oval_2n = vec![
        Node::new(NodeId(1), 100.0, 200.0, NodeType::Smooth),
        Node::new(NodeId(2), 100.0, 350.0, NodeType::OffCurve),
        Node::new(NodeId(3), 300.0, 350.0, NodeType::OffCurve),
        Node::new(NodeId(4), 300.0, 200.0, NodeType::Smooth),
        Node::new(NodeId(5), 300.0, 50.0, NodeType::OffCurve),
        Node::new(NodeId(6), 100.0, 50.0, NodeType::OffCurve),
    ];
    assert!(grafia::core::slide_on_curve_node_along_contour(
        &mut oval_2n,
        true,
        NodeId(1),
        FontPoint::new(135.0, 290.0)
    ));
    assert!(oval_2n[0].point.y.0 > 240.0, "2-node closed cubic oval node must slide smoothly upward");
}

#[test]
fn test_spec002_f14_reference_build4108_golden_master_vectors_and_pen_close_scenarios() {
    // 1. Hit-test priority: already-selected node wins within SELECT_NODE_HIT_RADIUS_DP = 7.5 dp
    let mut ht = GlifoCanvasState::new(GlifoTheme::sonoma_default());
    ht.viewport.zoom = 1.0;
    ht.viewport.pan_offset = ScreenPoint::new(100.0, 500.0);
    ht.contours = vec![vec![
        Node::new(NodeId(1), 100.0, 200.0, NodeType::Corner),
        Node::new(NodeId(2), 104.0, 200.0, NodeType::Corner),
    ]];
    ht.selected_nodes = vec![NodeId(1)];
    ht.sync_paths_from_contours();
    let probe_scr = ht.viewport.to_screen(FontPoint::new(103.5, 200.0));
    assert_eq!(
        ht.hit_test_node(probe_scr),
        Some(NodeId(1)),
        "Selected node within 7.5 dp must take priority over unselected node (elementAtPoint:atLayer:ignoreLocked:)"
    );
    let probe_72 = ht.viewport.to_screen(FontPoint::new(92.8, 200.0)); // 7.2 dp from NodeId(1)
    assert_eq!(
        ht.hit_test_node(probe_72),
        Some(NodeId(1)),
        "SelectTool hit_test_node must hit at 7.2 dp (< 7.5 dp)"
    );
    assert_eq!(
        ht.hit_test_pen_node(probe_72),
        None,
        "PenTool hit_test_pen_node must NOT hit at 7.2 dp (>= PEN_NODE_HIT_RADIUS_DP = 7.0 dp)"
    );

    // 2A. Reference Golden Master 1A-2: Ctrl + Alt + Drag on OnCurve node (setInterpolate:withPoint: / Nudge)
    let make_gm_arch = || {
        let mut s = GlifoCanvasState::new(GlifoTheme::sonoma_default());
        s.viewport.zoom = 1.0;
        s.viewport.pan_offset = ScreenPoint::new(100.0, 500.0);
        s.contours = vec![vec![
            Node::new(NodeId(1), 0.0, 0.0, NodeType::Corner),
            Node::new(NodeId(2), 0.0, 55.0, NodeType::OffCurve),
            Node::new(NodeId(3), 45.0, 100.0, NodeType::OffCurve),
            Node::new(NodeId(4), 100.0, 100.0, NodeType::Smooth),
            Node::new(NodeId(5), 155.0, 100.0, NodeType::OffCurve),
            Node::new(NodeId(6), 200.0, 55.0, NodeType::OffCurve),
            Node::new(NodeId(7), 200.0, 0.0, NodeType::Corner),
        ]];
        s.active_contour_closed = true;
        s.sync_paths_from_contours();
        s
    };

    let mut gm_nudge = make_gm_arch();
    let gm_nudge_before = gm_nudge.snapshot();
    let p3_scr = gm_nudge.viewport.to_screen(FontPoint::new(100.0, 100.0));
    let p3_drag_scr = gm_nudge.viewport.to_screen(FontPoint::new(120.0, 130.0));
    let ctrl_alt_mods = EguiModifiers { alt: true, ctrl: true, ..Default::default() };
    run_dispatch(
        &mut gm_nudge,
        vec![
            Event::PointerButton { pos: Pos2::new(p3_scr.x.0, p3_scr.y.0), button: PointerButton::Primary, pressed: true, modifiers: ctrl_alt_mods },
            Event::PointerMoved(Pos2::new(p3_drag_scr.x.0, p3_drag_scr.y.0)),
            Event::PointerButton { pos: Pos2::new(p3_drag_scr.x.0, p3_drag_scr.y.0), button: PointerButton::Primary, pressed: false, modifiers: ctrl_alt_mods },
        ],
        false, true, true, false, false, 1.0,
    );
    assert!((gm_nudge.contours[0][1].point.x.0 - 0.0).abs() < 1e-4 && (gm_nudge.contours[0][1].point.y.0 - 71.5).abs() < 1e-4);
    assert!((gm_nudge.contours[0][2].point.x.0 - 54.0).abs() < 1e-4 && (gm_nudge.contours[0][2].point.y.0 - 130.0).abs() < 1e-4);
    assert!((gm_nudge.contours[0][3].point.x.0 - 120.0).abs() < 1e-4 && (gm_nudge.contours[0][3].point.y.0 - 130.0).abs() < 1e-4);
    assert!((gm_nudge.contours[0][4].point.x.0 - 164.0).abs() < 1e-4 && (gm_nudge.contours[0][4].point.y.0 - 130.0).abs() < 1e-4);
    assert!((gm_nudge.contours[0][5].point.x.0 - 200.0).abs() < 1e-4 && (gm_nudge.contours[0][5].point.y.0 - 71.5).abs() < 1e-4);
    gm_nudge.assert_invariants();
    assert_all_views_consistent(&gm_nudge);
    let gm_nudge_after = gm_nudge.snapshot();
    assert!(gm_nudge.undo());
    assert_eq!(gm_nudge.contours, gm_nudge_before.contours);
    assert!(gm_nudge.redo());
    assert_eq!(gm_nudge.contours, gm_nudge_after.contours);

    // 2B. Reference Golden Master 1A-2 via Ctrl + Alt + Arrow keyboard Nudge (2x Shift+Right = +20, 3x Shift+Up = +30)
    let mut gm_key_nudge = make_gm_arch();
    gm_key_nudge.selected_nodes = vec![NodeId(4)];
    assert!(press_key(&mut gm_key_nudge, Key::ArrowRight, true, true, true));
    assert!(press_key(&mut gm_key_nudge, Key::ArrowRight, true, true, true));
    assert!(press_key(&mut gm_key_nudge, Key::ArrowUp, true, true, true));
    assert!(press_key(&mut gm_key_nudge, Key::ArrowUp, true, true, true));
    assert!(press_key(&mut gm_key_nudge, Key::ArrowUp, true, true, true));
    assert!((gm_key_nudge.contours[0][1].point.x.0 - 0.0).abs() < 1e-4 && (gm_key_nudge.contours[0][1].point.y.0 - 71.5).abs() < 1e-4);
    assert!((gm_key_nudge.contours[0][2].point.x.0 - 54.0).abs() < 1e-4 && (gm_key_nudge.contours[0][2].point.y.0 - 130.0).abs() < 1e-4);
    assert!((gm_key_nudge.contours[0][3].point.x.0 - 120.0).abs() < 1e-4 && (gm_key_nudge.contours[0][3].point.y.0 - 130.0).abs() < 1e-4);
    assert!((gm_key_nudge.contours[0][4].point.x.0 - 164.0).abs() < 1e-4 && (gm_key_nudge.contours[0][4].point.y.0 - 130.0).abs() < 1e-4);
    assert!((gm_key_nudge.contours[0][5].point.x.0 - 200.0).abs() < 1e-4 && (gm_key_nudge.contours[0][5].point.y.0 - 71.5).abs() < 1e-4);
    gm_key_nudge.assert_invariants();
    assert_all_views_consistent(&gm_key_nudge);

    // 2C. Reference Golden Master setMirrored: Ctrl + Alt + Drag on OffCurve handle (45,100) by (20,30) -> (65,130), opposite (155,100) -> (135.0, 70.0)
    let mut gm_mirror = make_gm_arch();
    let gm_mirror_before = gm_mirror.snapshot();
    let h_in_scr = gm_mirror.viewport.to_screen(FontPoint::new(45.0, 100.0));
    let h_in_drag_scr = gm_mirror.viewport.to_screen(FontPoint::new(65.0, 130.0));
    run_dispatch(
        &mut gm_mirror,
        vec![
            Event::PointerButton { pos: Pos2::new(h_in_scr.x.0, h_in_scr.y.0), button: PointerButton::Primary, pressed: true, modifiers: ctrl_alt_mods },
            Event::PointerMoved(Pos2::new(h_in_drag_scr.x.0, h_in_drag_scr.y.0)),
            Event::PointerButton { pos: Pos2::new(h_in_drag_scr.x.0, h_in_drag_scr.y.0), button: PointerButton::Primary, pressed: false, modifiers: ctrl_alt_mods },
        ],
        false, true, true, false, false, 1.0,
    );
    assert!((gm_mirror.contours[0][2].point.x.0 - 65.0).abs() < 1e-4 && (gm_mirror.contours[0][2].point.y.0 - 130.0).abs() < 1e-4);
    assert!(
        (gm_mirror.contours[0][4].point.x.0 - 135.0).abs() < 1e-4 && (gm_mirror.contours[0][4].point.y.0 - 70.0).abs() < 1e-4,
        "setMirrored: opposite handle must mirror to (135.0, 70.0), got {:?}",
        gm_mirror.contours[0][4].point
    );
    gm_mirror.assert_invariants();
    assert_all_views_consistent(&gm_mirror);
    let gm_mirror_after = gm_mirror.snapshot();
    assert!(gm_mirror.undo());
    assert_eq!(gm_mirror.contours, gm_mirror_before.contours);
    assert!(gm_mirror.redo());
    assert_eq!(gm_mirror.contours, gm_mirror_after.contours);

    // 3A & 3B. Reference Golden Master Test 2A & Test 2B: Cmd + Ctrl + Drag on Smooth OnCurve node (moveNodeOnPath: / shiftMiddle:) via run_dispatch
    let cmd_ctrl_mods = EguiModifiers { command: true, mac_cmd: true, ctrl: true, ..Default::default() };
    let mut gm_slide_2a = make_gm_arch();
    let gm_slide_2a_before = gm_slide_2a.snapshot();
    let s_2a_target = gm_slide_2a.viewport.to_screen(FontPoint::new(120.0, 100.0));
    run_dispatch(
        &mut gm_slide_2a,
        vec![
            Event::PointerButton { pos: Pos2::new(p3_scr.x.0, p3_scr.y.0), button: PointerButton::Primary, pressed: true, modifiers: cmd_ctrl_mods },
            Event::PointerMoved(Pos2::new(s_2a_target.x.0, s_2a_target.y.0)),
            Event::PointerButton { pos: Pos2::new(s_2a_target.x.0, s_2a_target.y.0), button: PointerButton::Primary, pressed: false, modifiers: cmd_ctrl_mods },
        ],
        false, false, true, true, false, 1.0,
    );
    let p3_2a = gm_slide_2a.contours[0][3].point;
    let exact_2a_x = 4_294_360.0 / 35_937.0;
    let exact_2a_y = 3_524_660.0 / 35_937.0;
    assert!(
        (p3_2a.x.0 - exact_2a_x).abs() < 1e-9 && (p3_2a.y.0 - exact_2a_y).abs() < 1e-9,
        "Golden Master Test 2A exact De Casteljau at s=4/33 expected ({exact_2a_x}, {exact_2a_y}), got {:?}",
        p3_2a
    );
    gm_slide_2a.assert_invariants();
    assert_all_views_consistent(&gm_slide_2a);
    let gm_slide_2a_after = gm_slide_2a.snapshot();
    assert!(gm_slide_2a.undo());
    assert_eq!(gm_slide_2a.contours, gm_slide_2a_before.contours);
    assert!(gm_slide_2a.redo());
    assert_eq!(gm_slide_2a.contours, gm_slide_2a_after.contours);

    let mut gm_slide_2b = make_gm_arch();
    let gm_slide_2b_before = gm_slide_2b.snapshot();
    let s_2b_target = gm_slide_2b.viewport.to_screen(FontPoint::new(80.0, 90.0));
    run_dispatch(
        &mut gm_slide_2b,
        vec![
            Event::PointerButton { pos: Pos2::new(p3_scr.x.0, p3_scr.y.0), button: PointerButton::Primary, pressed: true, modifiers: cmd_ctrl_mods },
            Event::PointerMoved(Pos2::new(s_2b_target.x.0, s_2b_target.y.0)),
            Event::PointerButton { pos: Pos2::new(s_2b_target.x.0, s_2b_target.y.0), button: PointerButton::Primary, pressed: false, modifiers: cmd_ctrl_mods },
        ],
        false, false, true, true, false, 1.0,
    );
    let p3_2b = gm_slide_2b.contours[0][3].point;
    let exact_2b_x = 2_893_040.0 / 35_937.0;
    let exact_2b_y = 3_524_660.0 / 35_937.0;
    assert!(
        (p3_2b.x.0 - exact_2b_x).abs() < 1e-9 && (p3_2b.y.0 - exact_2b_y).abs() < 1e-9,
        "Golden Master Test 2B exact De Casteljau at u=29/33 expected ({exact_2b_x}, {exact_2b_y}), got {:?}",
        p3_2b
    );
    gm_slide_2b.assert_invariants();
    assert_all_views_consistent(&gm_slide_2b);
    let gm_slide_2b_after = gm_slide_2b.snapshot();
    assert!(gm_slide_2b.undo());
    assert_eq!(gm_slide_2b.contours, gm_slide_2b_before.contours);
    assert!(gm_slide_2b.redo());
    assert_eq!(gm_slide_2b.contours, gm_slide_2b_after.contours);

    // Verify bare Ctrl (without Cmd and without Alt) in SelectTool performs standard translation, NOT path sliding
    let mut gm_bare_ctrl = make_gm_arch();
    let bare_ctrl_mods = EguiModifiers { command: false, mac_cmd: false, ctrl: true, ..Default::default() };
    run_dispatch(
        &mut gm_bare_ctrl,
        vec![
            Event::PointerButton { pos: Pos2::new(p3_scr.x.0, p3_scr.y.0), button: PointerButton::Primary, pressed: true, modifiers: bare_ctrl_mods },
            Event::PointerMoved(Pos2::new(s_2a_target.x.0, s_2a_target.y.0)),
            Event::PointerButton { pos: Pos2::new(s_2a_target.x.0, s_2a_target.y.0), button: PointerButton::Primary, pressed: false, modifiers: bare_ctrl_mods },
        ],
        false, false, false, true, false, 1.0,
    );
    assert!(
        (gm_bare_ctrl.contours[0][3].point.x.0 - 120.0).abs() < 1e-4
            && (gm_bare_ctrl.contours[0][3].point.y.0 - 100.0).abs() < 1e-4,
        "Bare Ctrl without Cmd and without Alt must translate node normally to (120, 100)"
    );

    // 3C. Reference Golden Master Test 2C: Alt + Drag on a Smooth OnCurve point between straight line segments (nearest_point_on_line)
    let mut gm_2c = GlifoCanvasState::new(GlifoTheme::sonoma_default());
    gm_2c.viewport.zoom = 1.0;
    gm_2c.viewport.pan_offset = ScreenPoint::new(100.0, 500.0);
    gm_2c.contours = vec![vec![
        Node::new(NodeId(1), 0.0, 0.0, NodeType::Corner),
        Node::new(NodeId(2), 100.0, 100.0, NodeType::Smooth),
        Node::new(NodeId(3), 200.0, 200.0, NodeType::Corner),
    ]];
    gm_2c.set_contour_closed(0, false);
    gm_2c.sync_paths_from_contours();
    let gm_2c_before = gm_2c.snapshot();
    let s_100 = gm_2c.viewport.to_screen(FontPoint::new(100.0, 100.0));
    let s_120_90 = gm_2c.viewport.to_screen(FontPoint::new(120.0, 90.0));
    let alt_mods = EguiModifiers { alt: true, ..Default::default() };
    run_dispatch(
        &mut gm_2c,
        vec![
            Event::PointerButton { pos: Pos2::new(s_100.x.0, s_100.y.0), button: PointerButton::Primary, pressed: true, modifiers: alt_mods },
            Event::PointerMoved(Pos2::new(s_120_90.x.0, s_120_90.y.0)),
            Event::PointerButton { pos: Pos2::new(s_120_90.x.0, s_120_90.y.0), button: PointerButton::Primary, pressed: false, modifiers: alt_mods },
        ],
        false, true, false, false, false, 1.0,
    );
    assert!(
        (gm_2c.contours[0][1].point.x.0 - 105.0).abs() < 1e-4
            && (gm_2c.contours[0][1].point.y.0 - 105.0).abs() < 1e-4,
        "Golden Master Test 2C expected (105.0, 105.0), got {:?}",
        gm_2c.contours[0][1].point
    );
    gm_2c.assert_invariants();
    assert_all_views_consistent(&gm_2c);
    let gm_2c_after = gm_2c.snapshot();
    assert!(gm_2c.undo(), "Alt + Drag on Smooth line node must be undoable");
    assert_eq!(
        gm_2c.contours, gm_2c_before.contours,
        "Single Undo after Alt + Drag (Test 2C) must restore exact pre-drag contour without duplicate Toggle history entry"
    );
    assert!(
        !gm_2c.history.can_undo(),
        "Alt + Drag must record exactly ONE atomic undo entry, not two"
    );
    assert!(gm_2c.redo());
    assert_eq!(gm_2c.contours, gm_2c_after.contours);

    // 4. PenTool Scenarios C1, C2, C3 (start point node[0] placed WITH drag, preserving contours[0][1]!)
    // Scenario C1: node[0] placed with drag -> node[1] placed without drag -> close on node[0] without drag
    //              preserves contours[0][1] AND converts node[0] from Smooth to Corner (half-line/half-curve checkConnection)!
    let mut pen_c1 = GlifoCanvasState::new(GlifoTheme::sonoma_default());
    pen_c1.viewport.zoom = 1.0;
    pen_c1.viewport.pan_offset = ScreenPoint::new(100.0, 500.0);
    pen_c1.active_tool = CanvasTool::Pen;
    let c_p0 = pen_c1.viewport.to_screen(FontPoint::new(100.0, 100.0));
    let c_p0_out = pen_c1.viewport.to_screen(FontPoint::new(140.0, 150.0));
    let c_p1 = pen_c1.viewport.to_screen(FontPoint::new(300.0, 200.0));
    let c_p2 = pen_c1.viewport.to_screen(FontPoint::new(250.0, 50.0));
    run_dispatch(
        &mut pen_c1,
        vec![
            Event::PointerButton { pos: Pos2::new(c_p0.x.0, c_p0.y.0), button: PointerButton::Primary, pressed: true, modifiers: EguiModifiers::NONE },
            Event::PointerMoved(Pos2::new(c_p0_out.x.0, c_p0_out.y.0)),
            Event::PointerButton { pos: Pos2::new(c_p0_out.x.0, c_p0_out.y.0), button: PointerButton::Primary, pressed: false, modifiers: EguiModifiers::NONE },
            Event::PointerButton { pos: Pos2::new(c_p1.x.0, c_p1.y.0), button: PointerButton::Primary, pressed: true, modifiers: EguiModifiers::NONE },
            Event::PointerButton { pos: Pos2::new(c_p1.x.0, c_p1.y.0), button: PointerButton::Primary, pressed: false, modifiers: EguiModifiers::NONE },
            Event::PointerButton { pos: Pos2::new(c_p2.x.0, c_p2.y.0), button: PointerButton::Primary, pressed: true, modifiers: EguiModifiers::NONE },
            Event::PointerButton { pos: Pos2::new(c_p2.x.0, c_p2.y.0), button: PointerButton::Primary, pressed: false, modifiers: EguiModifiers::NONE },
            Event::PointerMoved(Pos2::new(c_p0.x.0, c_p0.y.0)),
        ],
        false, false, false, false, false, 1.0,
    );
    assert_eq!(pen_c1.hovered_pen_close_node, Some(pen_c1.contours[0][0].id));
    let pen_c1_open_snap = pen_c1.snapshot();
    run_dispatch(
        &mut pen_c1,
        vec![
            Event::PointerButton { pos: Pos2::new(c_p0.x.0, c_p0.y.0), button: PointerButton::Primary, pressed: true, modifiers: EguiModifiers::NONE },
            Event::PointerButton { pos: Pos2::new(c_p0.x.0, c_p0.y.0), button: PointerButton::Primary, pressed: false, modifiers: EguiModifiers::NONE },
        ],
        false, false, false, false, false, 2.0,
    );
    assert!(pen_c1.is_contour_closed(0));
    assert_eq!(pen_c1.contours[0][1].point, FontPoint::new(140.0, 150.0), "Scenario C1 must preserve contours[0][1]");
    assert_eq!(pen_c1.contours[0][0].node_type, NodeType::Corner, "Scenario C1 half-line/half-curve close without drag must set node[0] to Corner");
    let c1_segs = extract_segments(&pen_c1.contours[0], true);
    assert!(
        matches!(c1_segs.last(), Some(ContourSegment::Line { .. })),
        "Scenario C1 closing segment after a Line prevNode must be a straight ContourSegment::Line"
    );
    pen_c1.assert_invariants();
    assert_all_views_consistent(&pen_c1);
    let pen_c1_closed_snap = pen_c1.snapshot();
    assert!(pen_c1.undo());
    assert_eq!(pen_c1.contours, pen_c1_open_snap.contours);
    assert!(!pen_c1.is_contour_closed(0));
    assert!(pen_c1.redo());
    assert_eq!(pen_c1.contours, pen_c1_closed_snap.contours);
    assert!(pen_c1.is_contour_closed(0));

    // Scenario C2: node[0] placed with drag -> close on node[0] WITH drag mirrors h2 = 2*node[0] - dragPt while preserving contours[0][1]!
    let mut pen_c2 = GlifoCanvasState::new(GlifoTheme::sonoma_default());
    pen_c2.viewport.zoom = 1.0;
    pen_c2.viewport.pan_offset = ScreenPoint::new(100.0, 500.0);
    pen_c2.active_tool = CanvasTool::Pen;
    let c_close_drag = pen_c2.viewport.to_screen(FontPoint::new(80.0, 130.0));
    run_dispatch(
        &mut pen_c2,
        vec![
            Event::PointerButton { pos: Pos2::new(c_p0.x.0, c_p0.y.0), button: PointerButton::Primary, pressed: true, modifiers: EguiModifiers::NONE },
            Event::PointerMoved(Pos2::new(c_p0_out.x.0, c_p0_out.y.0)),
            Event::PointerButton { pos: Pos2::new(c_p0_out.x.0, c_p0_out.y.0), button: PointerButton::Primary, pressed: false, modifiers: EguiModifiers::NONE },
            Event::PointerButton { pos: Pos2::new(c_p1.x.0, c_p1.y.0), button: PointerButton::Primary, pressed: true, modifiers: EguiModifiers::NONE },
            Event::PointerButton { pos: Pos2::new(c_p1.x.0, c_p1.y.0), button: PointerButton::Primary, pressed: false, modifiers: EguiModifiers::NONE },
            Event::PointerButton { pos: Pos2::new(c_p2.x.0, c_p2.y.0), button: PointerButton::Primary, pressed: true, modifiers: EguiModifiers::NONE },
            Event::PointerButton { pos: Pos2::new(c_p2.x.0, c_p2.y.0), button: PointerButton::Primary, pressed: false, modifiers: EguiModifiers::NONE },
            Event::PointerButton { pos: Pos2::new(c_p0.x.0, c_p0.y.0), button: PointerButton::Primary, pressed: true, modifiers: EguiModifiers::NONE },
            Event::PointerMoved(Pos2::new(c_close_drag.x.0, c_close_drag.y.0)),
            Event::PointerButton { pos: Pos2::new(c_close_drag.x.0, c_close_drag.y.0), button: PointerButton::Primary, pressed: false, modifiers: EguiModifiers::NONE },
        ],
        false, false, false, false, false, 1.0,
    );
    assert!(pen_c2.is_contour_closed(0));
    assert_eq!(pen_c2.contours[0][0].node_type, NodeType::Smooth);
    assert_eq!(
        pen_c2.contours[0][1].point,
        FontPoint::new(140.0, 150.0),
        "Scenario C2 closing drag on node[0] must 100% preserve initially dragged outgoing handle contours[0][1]!"
    );
    assert_eq!(
        pen_c2.contours[0].last().unwrap().point,
        FontPoint::new(120.0, 70.0),
        "Scenario C2 closing drag must mirror dragPt (80,130) across node[0] (100,100) into h2 = (120,70)"
    );
    pen_c2.assert_invariants();
    assert_all_views_consistent(&pen_c2);
    let pen_c2_closed_snap = pen_c2.snapshot();
    assert!(pen_c2.undo());
    assert!(!pen_c2.is_contour_closed(0));
    assert!(pen_c2.redo());
    assert_eq!(pen_c2.contours, pen_c2_closed_snap.contours);

    // Scenario C3: node[0] placed with drag AND prevNode placed with drag (pending_pen_bcp_out = (200, 30)) ->
    //              close on node[0] with drag preserves both contours[0][1] and h1 = prev_out, and mirrors h2 = 2*node[0] - dragPt!
    let mut pen_c3 = GlifoCanvasState::new(GlifoTheme::sonoma_default());
    pen_c3.viewport.zoom = 1.0;
    pen_c3.viewport.pan_offset = ScreenPoint::new(100.0, 500.0);
    pen_c3.active_tool = CanvasTool::Pen;
    let c_p2_out = pen_c3.viewport.to_screen(FontPoint::new(200.0, 30.0));
    run_dispatch(
        &mut pen_c3,
        vec![
            Event::PointerButton { pos: Pos2::new(c_p0.x.0, c_p0.y.0), button: PointerButton::Primary, pressed: true, modifiers: EguiModifiers::NONE },
            Event::PointerMoved(Pos2::new(c_p0_out.x.0, c_p0_out.y.0)),
            Event::PointerButton { pos: Pos2::new(c_p0_out.x.0, c_p0_out.y.0), button: PointerButton::Primary, pressed: false, modifiers: EguiModifiers::NONE },
            Event::PointerButton { pos: Pos2::new(c_p1.x.0, c_p1.y.0), button: PointerButton::Primary, pressed: true, modifiers: EguiModifiers::NONE },
            Event::PointerButton { pos: Pos2::new(c_p1.x.0, c_p1.y.0), button: PointerButton::Primary, pressed: false, modifiers: EguiModifiers::NONE },
            Event::PointerButton { pos: Pos2::new(c_p2.x.0, c_p2.y.0), button: PointerButton::Primary, pressed: true, modifiers: EguiModifiers::NONE },
            Event::PointerMoved(Pos2::new(c_p2_out.x.0, c_p2_out.y.0)),
            Event::PointerButton { pos: Pos2::new(c_p2_out.x.0, c_p2_out.y.0), button: PointerButton::Primary, pressed: false, modifiers: EguiModifiers::NONE },
            Event::PointerButton { pos: Pos2::new(c_p0.x.0, c_p0.y.0), button: PointerButton::Primary, pressed: true, modifiers: EguiModifiers::NONE },
            Event::PointerMoved(Pos2::new(c_close_drag.x.0, c_close_drag.y.0)),
            Event::PointerButton { pos: Pos2::new(c_close_drag.x.0, c_close_drag.y.0), button: PointerButton::Primary, pressed: false, modifiers: EguiModifiers::NONE },
        ],
        false, false, false, false, false, 1.0,
    );
    assert!(pen_c3.is_contour_closed(0));
    assert_eq!(pen_c3.contours[0][1].point, FontPoint::new(140.0, 150.0), "Scenario C3 must preserve contours[0][1]");
    let c3_len = pen_c3.contours[0].len();
    assert_eq!(pen_c3.contours[0][c3_len - 2].point, FontPoint::new(200.0, 30.0), "Scenario C3 must preserve prevNode's explicit h1");
    assert_eq!(pen_c3.contours[0][c3_len - 1].point, FontPoint::new(120.0, 70.0), "Scenario C3 must mirror dragPt into h2 = (120,70)");
    pen_c3.assert_invariants();
    assert_all_views_consistent(&pen_c3);
    let pen_c3_closed_snap = pen_c3.snapshot();
    assert!(pen_c3.undo());
    assert!(!pen_c3.is_contour_closed(0));
    assert!(pen_c3.redo());
    assert_eq!(pen_c3.contours, pen_c3_closed_snap.contours);

    // 5. Invariant validation on hovered_pen_close_node & Delete cleanup
    let mut inv_state = GlifoCanvasState::init_glyph_a();
    inv_state.hovered_pen_close_node = Some(NodeId(99999));
    assert!(
        inv_state.validate_invariants().is_err(),
        "validate_invariants must reject dangling hovered_pen_close_node"
    );
    inv_state.hovered_pen_close_node = Some(inv_state.contours[0][0].id);
    inv_state.selected_nodes = vec![inv_state.contours[0][0].id];
    assert!(press_key(&mut inv_state, Key::Delete, false, false, false));
    assert_eq!(inv_state.hovered_pen_close_node, None, "Delete must clear hovered_pen_close_node");
}

#[test]
fn test_spec002_f15_dual_oracle_reference_build4108_json_differential_golden_master() {
    let raw_json = std::fs::read_to_string("tokens/v010_bezier_golden_master.json")
        .expect("Failed to read tokens/v010_bezier_golden_master.json");
    let gm: serde_json::Value =
        serde_json::from_str(&raw_json).expect("Failed to parse Golden Master JSON");

    assert_eq!(gm["oracle_metadata"]["build"].as_u64(), Some(4108));

    let parse_node_type = |s: &str| match s {
        "corner" => NodeType::Corner,
        "smooth" => NodeType::Smooth,
        "off_curve" => NodeType::OffCurve,
        other => panic!("Unknown node_type in Golden Master JSON: {other}"),
    };
    let parse_nodes = |arr: &serde_json::Value| -> Vec<Node> {
        arr.as_array()
            .expect("Expected node array")
            .iter()
            .enumerate()
            .map(|(i, v)| {
                Node::new(
                    NodeId((1000 + i) as u64),
                    v["x"].as_f64().unwrap(),
                    v["y"].as_f64().unwrap(),
                    parse_node_type(v["node_type"].as_str().unwrap()),
                )
            })
            .collect()
    };
    let assert_nodes_match_json = |actual: &[Node], expected_arr: &serde_json::Value, label: &str| {
        let exp = parse_nodes(expected_arr);
        assert_eq!(actual.len(), exp.len(), "{label}: node count mismatch");
        for (idx, (a, e)) in actual.iter().zip(exp.iter()).enumerate() {
            assert!(
                (a.point.x.0 - e.point.x.0).abs() < 1e-4
                    && (a.point.y.0 - e.point.y.0).abs() < 1e-4,
                "{label}[{idx}]: expected ({}, {}), got ({}, {})",
                e.point.x.0,
                e.point.y.0,
                a.point.x.0,
                a.point.y.0
            );
            assert_eq!(a.node_type, e.node_type, "{label}[{idx}]: node_type mismatch");
        }
    };

    // 1. De Casteljau split at t=0.5
    let dc = &gm["de_casteljau_split_t05"];
    let mut dc_nodes = parse_nodes(&dc["input_nodes"]);
    let mut next_id = 100;
    let dc_seg = extract_segments(&dc_nodes, false)[0];
    grafia::core::split_contour_segment(
        &mut dc_nodes,
        dc_seg,
        dc["split_t"].as_f64().unwrap(),
        &mut next_id,
    );
    assert_nodes_match_json(&dc_nodes, &dc["expected_nodes"], "de_casteljau_split_t05");

    // 2. Extremum snap with Shift + PenTool
    let ex = &gm["extremum_snap_shift_pen"];
    let mut ex_state = GlifoCanvasState::new(GlifoTheme::sonoma_default());
    ex_state.viewport.zoom = 1.0;
    ex_state.viewport.pan_offset = ScreenPoint::new(100.0, 500.0);
    ex_state.active_tool = CanvasTool::Pen;
    ex_state.contours = vec![parse_nodes(&ex["input_nodes"])];
    ex_state.set_contour_closed(0, false);
    ex_state.sync_paths_from_contours();
    let probe_scr = ex_state.viewport.to_screen(FontPoint::new(
        ex["probe_point"]["x"].as_f64().unwrap(),
        ex["probe_point"]["y"].as_f64().unwrap(),
    ));
    let shift_mods = EguiModifiers { shift: true, ..Default::default() };
    run_dispatch(
        &mut ex_state,
        vec![
            Event::PointerMoved(Pos2::new(probe_scr.x.0, probe_scr.y.0)),
            Event::PointerButton {
                pos: Pos2::new(probe_scr.x.0, probe_scr.y.0),
                button: PointerButton::Primary,
                pressed: true,
                modifiers: shift_mods,
            },
            Event::PointerButton {
                pos: Pos2::new(probe_scr.x.0, probe_scr.y.0),
                button: PointerButton::Primary,
                pressed: false,
                modifiers: shift_mods,
            },
        ],
        true, false, false, false, false, 1.0,
    );
    let exp_ex_pt = FontPoint::new(
        ex["expected_extremum_point"]["x"].as_f64().unwrap(),
        ex["expected_extremum_point"]["y"].as_f64().unwrap(),
    );
    assert!((ex_state.contours[0][3].point.x.0 - exp_ex_pt.x.0).abs() < 1e-4);
    assert!((ex_state.contours[0][3].point.y.0 - exp_ex_pt.y.0).abs() < 1e-4);
    assert!((ex_state.contours[0][2].point.x.0 - ex["expected_left_bcp"]["x"].as_f64().unwrap()).abs() < 1e-4);
    assert!((ex_state.contours[0][4].point.x.0 - ex["expected_right_bcp"]["x"].as_f64().unwrap()).abs() < 1e-4);

    // 3. Heal delete S-curve
    let hd = &gm["heal_delete_scurve"];
    let mut hd_nodes = parse_nodes(&hd["input_nodes"]);
    let del_id = hd_nodes[hd["delete_node_index"].as_u64().unwrap() as usize].id;
    grafia::core::heal_delete_nodes(&mut hd_nodes, false, &[del_id], &mut next_id);
    assert_eq!(hd_nodes.len(), hd["expected_node_count"].as_u64().unwrap() as usize);
    let hd_cubic = kurbo::CubicBez::new(
        kurbo::Point::new(hd_nodes[0].point.x.0, hd_nodes[0].point.y.0),
        kurbo::Point::new(hd_nodes[1].point.x.0, hd_nodes[1].point.y.0),
        kurbo::Point::new(hd_nodes[2].point.x.0, hd_nodes[2].point.y.0),
        kurbo::Point::new(hd_nodes[3].point.x.0, hd_nodes[3].point.y.0),
    );
    let hd_mid = hd_cubic.eval(0.5);
    assert!((hd_mid.x - hd["expected_midpoint_t05"]["x"].as_f64().unwrap()).abs() < 1.0);
    assert!((hd_mid.y - hd["expected_midpoint_t05"]["y"].as_f64().unwrap()).abs() < 1.0);

    // 4. Cut open delete nodes
    let co = &gm["cut_open_delete_nodes"];
    let mut co_nodes = parse_nodes(&co["input_nodes"]);
    let co_del_id = co_nodes[co["delete_node_index"].as_u64().unwrap() as usize].id;
    grafia::core::cut_open_delete_nodes(&mut co_nodes, &[co_del_id]);
    assert_nodes_match_json(&co_nodes, &co["expected_nodes"], "cut_open_delete_nodes");

    // 5. Remove OffCurve segment handles
    let ro = &gm["remove_off_curve_segment_handles"];
    let mut ro_nodes = parse_nodes(&ro["input_nodes"]);
    let ro_del_id = ro_nodes[ro["delete_node_index"].as_u64().unwrap() as usize].id;
    assert!(grafia::core::remove_off_curve_segment_handles(&mut ro_nodes, true, ro_del_id));
    assert_nodes_match_json(&ro_nodes, &ro["expected_nodes"], "remove_off_curve_segment_handles");

    // 6. dragCurve 4 cases
    let dc4 = &gm["drag_curve_4_cases"];
    let t_drag = dc4["t"].as_f64().unwrap();
    let dx_drag = dc4["delta"]["dx"].as_f64().unwrap();
    let dy_drag = dc4["delta"]["dy"].as_f64().unwrap();
    let init_h1 = FontPoint::new(
        dc4["initial_h1"]["x"].as_f64().unwrap(),
        dc4["initial_h1"]["y"].as_f64().unwrap(),
    );
    let init_h2 = FontPoint::new(
        dc4["initial_h2"]["x"].as_f64().unwrap(),
        dc4["initial_h2"]["y"].as_f64().unwrap(),
    );
    for (case_key, p0_type, p3_type) in [
        ("case_a_corner_corner", NodeType::Corner, NodeType::Corner),
        ("case_b_smooth_smooth_vertical_tangents", NodeType::Smooth, NodeType::Smooth),
        ("case_c_smooth_corner_vertical_p0", NodeType::Smooth, NodeType::Corner),
        ("case_d_corner_smooth_vertical_p3", NodeType::Corner, NodeType::Smooth),
    ] {
        let mut nodes = vec![
            Node::new(NodeId(1), 0.0, -50.0, NodeType::Corner),
            Node::new(NodeId(2), 0.0, 0.0, p0_type),
            Node::new(NodeId(3), init_h1.x.0, init_h1.y.0, NodeType::OffCurve),
            Node::new(NodeId(4), init_h2.x.0, init_h2.y.0, NodeType::OffCurve),
            Node::new(NodeId(5), 100.0, 0.0, p3_type),
            Node::new(NodeId(6), 100.0, -50.0, NodeType::Corner),
        ];
        let seg = extract_segments(&nodes, true)[1];
        grafia::core::apply_cubic_segment_belly_drag_with_alt(
            &mut nodes,
            true,
            seg,
            t_drag,
            init_h1,
            init_h2,
            dx_drag,
            dy_drag,
            false,
        );
        let exp_h1 = &dc4[case_key]["expected_h1"];
        let exp_h2 = &dc4[case_key]["expected_h2"];
        assert!(
            (nodes[2].point.x.0 - exp_h1["x"].as_f64().unwrap()).abs() < 1e-4
                && (nodes[2].point.y.0 - exp_h1["y"].as_f64().unwrap()).abs() < 1e-4,
            "{case_key} H1 mismatch: got {:?}",
            nodes[2].point
        );
        assert!(
            (nodes[3].point.x.0 - exp_h2["x"].as_f64().unwrap()).abs() < 1e-4
                && (nodes[3].point.y.0 - exp_h2["y"].as_f64().unwrap()).abs() < 1e-4,
            "{case_key} H2 mismatch: got {:?}",
            nodes[3].point
        );
    }

    // 7. Ctrl+Alt Nudge setInterpolate:withPoint: (1A-1 and 1A-2)
    for case_key in ["case_1a1_single_cubic", "case_1a2_two_segment_arch"] {
        let c = &gm["ctrl_alt_nudge_interpolate"][case_key];
        let mut s = GlifoCanvasState::new(GlifoTheme::sonoma_default());
        s.contours = vec![parse_nodes(&c["input_nodes"])];
        s.active_contour_closed = true;
        s.sync_paths_from_contours();
        let sel_idx = c["selected_index"].as_u64().unwrap() as usize;
        s.selected_nodes = vec![s.contours[0][sel_idx].id];
        s.record_drag_start();
        s.apply_node_displacement_with_modifiers_and_ctrl(
            c["delta"]["dx"].as_f64().unwrap(),
            c["delta"]["dy"].as_f64().unwrap(),
            false,
            true,
            true,
        );
        assert_nodes_match_json(&s.contours[0], &c["expected_nodes"], case_key);
    }

    // 8. Ctrl+Alt setMirrored on OffCurve handle
    let sm = &gm["ctrl_alt_set_mirrored"];
    let mut sm_state = GlifoCanvasState::new(GlifoTheme::sonoma_default());
    sm_state.contours = vec![parse_nodes(
        &gm["ctrl_alt_nudge_interpolate"]["case_1a2_two_segment_arch"]["input_nodes"],
    )];
    sm_state.active_contour_closed = true;
    sm_state.sync_paths_from_contours();
    let sm_idx = sm["dragged_index"].as_u64().unwrap() as usize;
    sm_state.selected_nodes = vec![sm_state.contours[0][sm_idx].id];
    sm_state.record_drag_start();
    sm_state.apply_node_displacement_with_modifiers_and_ctrl(
        sm["delta"]["dx"].as_f64().unwrap(),
        sm["delta"]["dy"].as_f64().unwrap(),
        false,
        true,
        true,
    );
    assert!(
        (sm_state.contours[0][2].point.x.0 - sm["expected_dragged_handle"]["x"].as_f64().unwrap()).abs() < 1e-4
            && (sm_state.contours[0][2].point.y.0 - sm["expected_dragged_handle"]["y"].as_f64().unwrap()).abs() < 1e-4
    );
    assert!(
        (sm_state.contours[0][4].point.x.0 - sm["expected_opposite_handle"]["x"].as_f64().unwrap()).abs() < 1e-4
            && (sm_state.contours[0][4].point.y.0 - sm["expected_opposite_handle"]["y"].as_f64().unwrap()).abs() < 1e-4
    );

    // 9. Cmd+Ctrl Slide Node On Path (2A & 2B)
    let sl = &gm["cmd_ctrl_slide_node_on_path"];
    let sl_init = parse_nodes(
        &gm["ctrl_alt_nudge_interpolate"]["case_1a2_two_segment_arch"]["input_nodes"],
    );
    let mut sl_2a = sl_init.clone();
    let mid_id = sl_2a[3].id;
    let dx_2a = sl["case_2a_target"]["x"].as_f64().unwrap() - sl_init[3].point.x.0;
    let dy_2a = sl["case_2a_target"]["y"].as_f64().unwrap() - sl_init[3].point.y.0;
    assert!(grafia::core::slide_smooth_node_on_cubic_path_shift_middle(
        &mut sl_2a,
        &sl_init,
        true,
        mid_id,
        dx_2a,
        dy_2a,
    ));
    assert!((sl_2a[3].point.x.0 - sl["case_2a_expected_node"]["x"].as_f64().unwrap()).abs() < 1e-9);
    assert!((sl_2a[3].point.y.0 - sl["case_2a_expected_node"]["y"].as_f64().unwrap()).abs() < 1e-9);

    let mut sl_2b = sl_init.clone();
    let dx_2b = sl["case_2b_target"]["x"].as_f64().unwrap() - sl_init[3].point.x.0;
    let dy_2b = sl["case_2b_target"]["y"].as_f64().unwrap() - sl_init[3].point.y.0;
    assert!(grafia::core::slide_smooth_node_on_cubic_path_shift_middle(
        &mut sl_2b,
        &sl_init,
        true,
        mid_id,
        dx_2b,
        dy_2b,
    ));
    assert!((sl_2b[3].point.x.0 - sl["case_2b_expected_node"]["x"].as_f64().unwrap()).abs() < 1e-9);
    assert!((sl_2b[3].point.y.0 - sl["case_2b_expected_node"]["y"].as_f64().unwrap()).abs() < 1e-9);

    // 10. Alt Slide Smooth Line Node (2C)
    let sl2c = &gm["alt_slide_smooth_line_node_2c"];
    let sl2c_init = parse_nodes(&sl2c["input_nodes"]);
    let mut sl2c_nodes = sl2c_init.clone();
    let sl2c_id = sl2c_nodes[1].id;
    let dx_2c = sl2c["drag_target"]["x"].as_f64().unwrap() - sl2c_init[1].point.x.0;
    let dy_2c = sl2c["drag_target"]["y"].as_f64().unwrap() - sl2c_init[1].point.y.0;
    assert!(grafia::core::slide_smooth_line_node_unlimited(
        &mut sl2c_nodes,
        &sl2c_init,
        false,
        sl2c_id,
        dx_2c,
        dy_2c,
    ));
    assert!((sl2c_nodes[1].point.x.0 - sl2c["expected_point"]["x"].as_f64().unwrap()).abs() < 1e-6);
    assert!((sl2c_nodes[1].point.y.0 - sl2c["expected_point"]["y"].as_f64().unwrap()).abs() < 1e-6);
    assert_eq!(sl2c_nodes[1].node_type, parse_node_type(sl2c["expected_node_type"].as_str().unwrap()));

    // 11. PenTool Close Scenarios A, B, C1, C2, C3 from Golden Master JSON
    let pcs = &gm["pen_close_scenarios"];
    let gm_pt = |key: &str| FontPoint::new(pcs[key]["x"].as_f64().unwrap(), pcs[key]["y"].as_f64().unwrap());
    let p0_pt = gm_pt("p0");
    let p0_out_pt = gm_pt("p0_out");
    let p1_pt = gm_pt("p1");
    let p2_pt = gm_pt("p2");
    let p2_out_pt = gm_pt("p2_out");
    let close_drag_pt = gm_pt("close_drag");

    let new_pen_gm_state = || {
        let mut s = GlifoCanvasState::new(GlifoTheme::sonoma_default());
        s.viewport.zoom = 1.0;
        s.viewport.pan_offset = ScreenPoint::new(100.0, 500.0);
        s.active_tool = CanvasTool::Pen;
        s
    };

    // Scenario A (pure line triangle click-close)
    let mut gm_pen_a = new_pen_gm_state();
    let s_p0 = gm_pen_a.viewport.to_screen(p0_pt);
    let s_p0_out = gm_pen_a.viewport.to_screen(p0_out_pt);
    let s_p1 = gm_pen_a.viewport.to_screen(p1_pt);
    let s_p2 = gm_pen_a.viewport.to_screen(p2_pt);
    let s_p2_out = gm_pen_a.viewport.to_screen(p2_out_pt);
    let s_close_drag = gm_pen_a.viewport.to_screen(close_drag_pt);
    run_dispatch(
        &mut gm_pen_a,
        vec![
            Event::PointerButton { pos: Pos2::new(s_p0.x.0, s_p0.y.0), button: PointerButton::Primary, pressed: true, modifiers: EguiModifiers::NONE },
            Event::PointerButton { pos: Pos2::new(s_p0.x.0, s_p0.y.0), button: PointerButton::Primary, pressed: false, modifiers: EguiModifiers::NONE },
            Event::PointerButton { pos: Pos2::new(s_p1.x.0, s_p1.y.0), button: PointerButton::Primary, pressed: true, modifiers: EguiModifiers::NONE },
            Event::PointerButton { pos: Pos2::new(s_p1.x.0, s_p1.y.0), button: PointerButton::Primary, pressed: false, modifiers: EguiModifiers::NONE },
            Event::PointerButton { pos: Pos2::new(s_p2.x.0, s_p2.y.0), button: PointerButton::Primary, pressed: true, modifiers: EguiModifiers::NONE },
            Event::PointerButton { pos: Pos2::new(s_p2.x.0, s_p2.y.0), button: PointerButton::Primary, pressed: false, modifiers: EguiModifiers::NONE },
            Event::PointerButton { pos: Pos2::new(s_p0.x.0, s_p0.y.0), button: PointerButton::Primary, pressed: true, modifiers: EguiModifiers::NONE },
            Event::PointerButton { pos: Pos2::new(s_p0.x.0, s_p0.y.0), button: PointerButton::Primary, pressed: false, modifiers: EguiModifiers::NONE },
        ],
        false, false, false, false, false, 1.0,
    );
    assert!(gm_pen_a.is_contour_closed(0));
    assert_eq!(gm_pen_a.contours[0].len(), pcs["scenario_a_expected_count"].as_u64().unwrap() as usize);
    assert_eq!(gm_pen_a.contours[0][0].node_type, parse_node_type(pcs["scenario_a_expected_node0_type"].as_str().unwrap()));

    // Scenario B (start node[0] placed without drag -> close on node[0] WITH drag)
    let mut gm_pen_b = new_pen_gm_state();
    run_dispatch(
        &mut gm_pen_b,
        vec![
            Event::PointerButton { pos: Pos2::new(s_p0.x.0, s_p0.y.0), button: PointerButton::Primary, pressed: true, modifiers: EguiModifiers::NONE },
            Event::PointerButton { pos: Pos2::new(s_p0.x.0, s_p0.y.0), button: PointerButton::Primary, pressed: false, modifiers: EguiModifiers::NONE },
            Event::PointerButton { pos: Pos2::new(s_p1.x.0, s_p1.y.0), button: PointerButton::Primary, pressed: true, modifiers: EguiModifiers::NONE },
            Event::PointerButton { pos: Pos2::new(s_p1.x.0, s_p1.y.0), button: PointerButton::Primary, pressed: false, modifiers: EguiModifiers::NONE },
            Event::PointerButton { pos: Pos2::new(s_p2.x.0, s_p2.y.0), button: PointerButton::Primary, pressed: true, modifiers: EguiModifiers::NONE },
            Event::PointerButton { pos: Pos2::new(s_p2.x.0, s_p2.y.0), button: PointerButton::Primary, pressed: false, modifiers: EguiModifiers::NONE },
            Event::PointerButton { pos: Pos2::new(s_p0.x.0, s_p0.y.0), button: PointerButton::Primary, pressed: true, modifiers: EguiModifiers::NONE },
            Event::PointerMoved(Pos2::new(s_close_drag.x.0, s_close_drag.y.0)),
            Event::PointerButton { pos: Pos2::new(s_close_drag.x.0, s_close_drag.y.0), button: PointerButton::Primary, pressed: false, modifiers: EguiModifiers::NONE },
        ],
        false, false, false, false, false, 1.0,
    );
    assert!(gm_pen_b.is_contour_closed(0));
    assert_eq!(gm_pen_b.contours[0][0].node_type, parse_node_type(pcs["scenario_b_expected_node0_type"].as_str().unwrap()));
    assert_eq!(gm_pen_b.contours[0][1].point, gm_pt("scenario_b_expected_out_handle"));
    assert_eq!(gm_pen_b.contours[0].last().unwrap().point, gm_pt("scenario_b_expected_in_handle"));

    // Scenario C1, C2, C3 (start node[0] placed WITH drag)
    let mut gm_pen_c1 = new_pen_gm_state();
    run_dispatch(
        &mut gm_pen_c1,
        vec![
            Event::PointerButton { pos: Pos2::new(s_p0.x.0, s_p0.y.0), button: PointerButton::Primary, pressed: true, modifiers: EguiModifiers::NONE },
            Event::PointerMoved(Pos2::new(s_p0_out.x.0, s_p0_out.y.0)),
            Event::PointerButton { pos: Pos2::new(s_p0_out.x.0, s_p0_out.y.0), button: PointerButton::Primary, pressed: false, modifiers: EguiModifiers::NONE },
            Event::PointerButton { pos: Pos2::new(s_p1.x.0, s_p1.y.0), button: PointerButton::Primary, pressed: true, modifiers: EguiModifiers::NONE },
            Event::PointerButton { pos: Pos2::new(s_p1.x.0, s_p1.y.0), button: PointerButton::Primary, pressed: false, modifiers: EguiModifiers::NONE },
            Event::PointerButton { pos: Pos2::new(s_p2.x.0, s_p2.y.0), button: PointerButton::Primary, pressed: true, modifiers: EguiModifiers::NONE },
            Event::PointerButton { pos: Pos2::new(s_p2.x.0, s_p2.y.0), button: PointerButton::Primary, pressed: false, modifiers: EguiModifiers::NONE },
            Event::PointerButton { pos: Pos2::new(s_p0.x.0, s_p0.y.0), button: PointerButton::Primary, pressed: true, modifiers: EguiModifiers::NONE },
            Event::PointerButton { pos: Pos2::new(s_p0.x.0, s_p0.y.0), button: PointerButton::Primary, pressed: false, modifiers: EguiModifiers::NONE },
        ],
        false, false, false, false, false, 1.0,
    );
    assert_eq!(gm_pen_c1.contours[0][0].node_type, parse_node_type(pcs["scenario_c1_expected_node0_type"].as_str().unwrap()));
    assert_eq!(gm_pen_c1.contours[0][1].point, gm_pt("scenario_c1_expected_preserved_out_handle"));

    let mut gm_pen_c2 = new_pen_gm_state();
    run_dispatch(
        &mut gm_pen_c2,
        vec![
            Event::PointerButton { pos: Pos2::new(s_p0.x.0, s_p0.y.0), button: PointerButton::Primary, pressed: true, modifiers: EguiModifiers::NONE },
            Event::PointerMoved(Pos2::new(s_p0_out.x.0, s_p0_out.y.0)),
            Event::PointerButton { pos: Pos2::new(s_p0_out.x.0, s_p0_out.y.0), button: PointerButton::Primary, pressed: false, modifiers: EguiModifiers::NONE },
            Event::PointerButton { pos: Pos2::new(s_p1.x.0, s_p1.y.0), button: PointerButton::Primary, pressed: true, modifiers: EguiModifiers::NONE },
            Event::PointerButton { pos: Pos2::new(s_p1.x.0, s_p1.y.0), button: PointerButton::Primary, pressed: false, modifiers: EguiModifiers::NONE },
            Event::PointerButton { pos: Pos2::new(s_p2.x.0, s_p2.y.0), button: PointerButton::Primary, pressed: true, modifiers: EguiModifiers::NONE },
            Event::PointerButton { pos: Pos2::new(s_p2.x.0, s_p2.y.0), button: PointerButton::Primary, pressed: false, modifiers: EguiModifiers::NONE },
            Event::PointerButton { pos: Pos2::new(s_p0.x.0, s_p0.y.0), button: PointerButton::Primary, pressed: true, modifiers: EguiModifiers::NONE },
            Event::PointerMoved(Pos2::new(s_close_drag.x.0, s_close_drag.y.0)),
            Event::PointerButton { pos: Pos2::new(s_close_drag.x.0, s_close_drag.y.0), button: PointerButton::Primary, pressed: false, modifiers: EguiModifiers::NONE },
        ],
        false, false, false, false, false, 1.0,
    );
    assert_eq!(gm_pen_c2.contours[0][0].node_type, parse_node_type(pcs["scenario_c2_expected_node0_type"].as_str().unwrap()));
    assert_eq!(gm_pen_c2.contours[0][1].point, gm_pt("scenario_c2_expected_preserved_out_handle"));
    assert_eq!(gm_pen_c2.contours[0].last().unwrap().point, gm_pt("scenario_c2_expected_in_handle"));

    let mut gm_pen_c3 = new_pen_gm_state();
    run_dispatch(
        &mut gm_pen_c3,
        vec![
            Event::PointerButton { pos: Pos2::new(s_p0.x.0, s_p0.y.0), button: PointerButton::Primary, pressed: true, modifiers: EguiModifiers::NONE },
            Event::PointerMoved(Pos2::new(s_p0_out.x.0, s_p0_out.y.0)),
            Event::PointerButton { pos: Pos2::new(s_p0_out.x.0, s_p0_out.y.0), button: PointerButton::Primary, pressed: false, modifiers: EguiModifiers::NONE },
            Event::PointerButton { pos: Pos2::new(s_p1.x.0, s_p1.y.0), button: PointerButton::Primary, pressed: true, modifiers: EguiModifiers::NONE },
            Event::PointerButton { pos: Pos2::new(s_p1.x.0, s_p1.y.0), button: PointerButton::Primary, pressed: false, modifiers: EguiModifiers::NONE },
            Event::PointerButton { pos: Pos2::new(s_p2.x.0, s_p2.y.0), button: PointerButton::Primary, pressed: true, modifiers: EguiModifiers::NONE },
            Event::PointerMoved(Pos2::new(s_p2_out.x.0, s_p2_out.y.0)),
            Event::PointerButton { pos: Pos2::new(s_p2_out.x.0, s_p2_out.y.0), button: PointerButton::Primary, pressed: false, modifiers: EguiModifiers::NONE },
            Event::PointerButton { pos: Pos2::new(s_p0.x.0, s_p0.y.0), button: PointerButton::Primary, pressed: true, modifiers: EguiModifiers::NONE },
            Event::PointerMoved(Pos2::new(s_close_drag.x.0, s_close_drag.y.0)),
            Event::PointerButton { pos: Pos2::new(s_close_drag.x.0, s_close_drag.y.0), button: PointerButton::Primary, pressed: false, modifiers: EguiModifiers::NONE },
        ],
        false, false, false, false, false, 1.0,
    );
    let c3_len = gm_pen_c3.contours[0].len();
    assert_eq!(gm_pen_c3.contours[0][1].point, gm_pt("scenario_c3_expected_preserved_out_handle"));
    assert_eq!(gm_pen_c3.contours[0][c3_len - 2].point, gm_pt("scenario_c3_expected_prev_out_handle"));
    assert_eq!(gm_pen_c3.contours[0][c3_len - 1].point, gm_pt("scenario_c3_expected_in_handle"));

    // 12. Reverse Contour & Make Node First
    let rmf = &gm["reverse_and_make_first"];
    let mut rev_nodes = parse_nodes(&rmf["input_nodes"]);
    grafia::core::reverse_contour_nodes(&mut rev_nodes, true);
    assert_nodes_match_json(&rev_nodes, &rmf["reversed_expected_nodes"], "reverse_contour_nodes");

    let mut mf_nodes = parse_nodes(&rmf["input_nodes"]);
    let mf_id = mf_nodes[rmf["make_first_index"].as_u64().unwrap() as usize].id;
    assert!(grafia::core::make_selected_on_curve_node_first(&mut mf_nodes, true, &[mf_id]));
    assert_nodes_match_json(&mf_nodes, &rmf["make_first_expected_nodes"], "make_selected_on_curve_node_first");
}

#[test]
fn test_spec002_f16_mid_drag_modifier_transitions_zero_length_handles_and_2node_closed_cubic() {
    // 1. Rapid mid-drag modifier combinations (None -> Alt -> Ctrl+Alt -> Cmd+Ctrl -> None -> PointerRelease)
    let mut state = GlifoCanvasState::new(GlifoTheme::sonoma_default());
    state.viewport.zoom = 1.0;
    state.viewport.pan_offset = ScreenPoint::new(100.0, 500.0);
    state.contours = vec![vec![
        Node::new(NodeId(1), 0.0, 0.0, NodeType::Corner),
        Node::new(NodeId(2), 0.0, 55.0, NodeType::OffCurve),
        Node::new(NodeId(3), 45.0, 100.0, NodeType::OffCurve),
        Node::new(NodeId(4), 100.0, 100.0, NodeType::Smooth),
        Node::new(NodeId(5), 155.0, 100.0, NodeType::OffCurve),
        Node::new(NodeId(6), 200.0, 55.0, NodeType::OffCurve),
        Node::new(NodeId(7), 200.0, 0.0, NodeType::Corner),
    ]];
    state.active_contour_closed = true;
    state.sync_paths_from_contours();
    let before_snap = state.snapshot();

    let p3_scr = state.viewport.to_screen(FontPoint::new(100.0, 100.0));
    let target_scr = state.viewport.to_screen(FontPoint::new(120.0, 130.0));
    let slide_scr = state.viewport.to_screen(FontPoint::new(120.0, 100.0));

    // Step A: PrimaryDown without modifiers
    run_dispatch(
        &mut state,
        vec![Event::PointerButton {
            pos: Pos2::new(p3_scr.x.0, p3_scr.y.0),
            button: PointerButton::Primary,
            pressed: true,
            modifiers: EguiModifiers::NONE,
        }],
        false, false, false, false, false, 1.0,
    );

    // Step B: Hold Alt mid-drag -> anchor moves to (120, 130), attached BCPs stay at (45, 100) and (155, 100)
    run_dispatch(
        &mut state,
        vec![Event::PointerMoved(Pos2::new(target_scr.x.0, target_scr.y.0))],
        false, true, false, false, false, 1.1,
    );
    assert!((state.contours[0][3].point.x.0 - 120.0).abs() < 1e-4 && (state.contours[0][3].point.y.0 - 130.0).abs() < 1e-4);
    assert!((state.contours[0][2].point.x.0 - 45.0).abs() < 1e-4 && (state.contours[0][2].point.y.0 - 100.0).abs() < 1e-4);

    // Step C: Switch to Ctrl + Alt mid-drag -> setInterpolate:withPoint: Nudge
    run_dispatch(
        &mut state,
        vec![Event::PointerMoved(Pos2::new(target_scr.x.0, target_scr.y.0))],
        false, true, false, true, false, 1.2,
    );
    assert!((state.contours[0][1].point.y.0 - 71.5).abs() < 1e-4);
    assert!((state.contours[0][2].point.x.0 - 54.0).abs() < 1e-4 && (state.contours[0][2].point.y.0 - 130.0).abs() < 1e-4);

    // Step D: Switch to Cmd + Ctrl mid-drag -> moveNodeOnPath: / shiftMiddle: at s = 4/33
    run_dispatch(
        &mut state,
        vec![Event::PointerMoved(Pos2::new(slide_scr.x.0, slide_scr.y.0))],
        false, false, true, true, false, 1.3,
    );
    let exact_2a_x = 4_294_360.0 / 35_937.0;
    let exact_2a_y = 3_524_660.0 / 35_937.0;
    assert!((state.contours[0][3].point.x.0 - exact_2a_x).abs() < 1e-9);
    assert!((state.contours[0][3].point.y.0 - exact_2a_y).abs() < 1e-9);

    // Step E: Release all modifiers mid-drag -> rigid translation of anchor + attached BCPs by (+20, +30)
    run_dispatch(
        &mut state,
        vec![
            Event::PointerMoved(Pos2::new(target_scr.x.0, target_scr.y.0)),
            Event::PointerButton {
                pos: Pos2::new(target_scr.x.0, target_scr.y.0),
                button: PointerButton::Primary,
                pressed: false,
                modifiers: EguiModifiers::NONE,
            },
        ],
        false, false, false, false, false, 1.4,
    );
    assert!((state.contours[0][3].point.x.0 - 120.0).abs() < 1e-4 && (state.contours[0][3].point.y.0 - 130.0).abs() < 1e-4);
    assert!((state.contours[0][2].point.x.0 - 65.0).abs() < 1e-4 && (state.contours[0][2].point.y.0 - 130.0).abs() < 1e-4);
    assert!((state.contours[0][1].point.x.0 - 0.0).abs() < 1e-4 && (state.contours[0][1].point.y.0 - 55.0).abs() < 1e-4);
    state.assert_invariants();
    assert_all_views_consistent(&state);
    let after_snap = state.snapshot();
    assert!(state.undo());
    assert_eq!(state.contours, before_snap.contours);
    assert!(state.redo());
    assert_eq!(state.contours, after_snap.contours);

    // 2. Degenerate zero-length handles (distance == 0.0): align_collinear_opposite must NOT teleport bcp_in to 180° (-X)
    let anchor = FontPoint::new(100.0, 100.0);
    let bcp_in_vertical = FontPoint::new(100.0, 40.0);
    let bcp_out_zero = FontPoint::new(100.0, 100.0);
    let aligned = grafia::core::align_collinear_opposite(anchor, bcp_in_vertical, bcp_out_zero);
    assert_eq!(
        aligned, bcp_in_vertical,
        "Degenerate zero-length bcp_out (distance == 0.0) must leave bcp_in unchanged instead of snapping to -X"
    );

    // 3. 2-node closed cubic contour (n == 6) including self-intersecting figure-8: shiftMiddle: must handle n == 6 directly
    let oval_init = vec![
        Node::new(NodeId(1), 0.0, 0.0, NodeType::Smooth),
        Node::new(NodeId(2), 0.0, 100.0, NodeType::OffCurve),
        Node::new(NodeId(3), 200.0, -100.0, NodeType::OffCurve),
        Node::new(NodeId(4), 200.0, 0.0, NodeType::Smooth),
        Node::new(NodeId(5), 200.0, 100.0, NodeType::OffCurve),
        Node::new(NodeId(6), 0.0, -100.0, NodeType::OffCurve),
    ];
    let mut fig8 = oval_init.clone();
    assert!(
        grafia::core::slide_smooth_node_on_cubic_path_shift_middle(
            &mut fig8,
            &oval_init,
            true,
            NodeId(4),
            0.0,
            30.0,
        ),
        "slide_smooth_node_on_cubic_path_shift_middle must support 2-node closed cubic contours (n == 6)"
    );
    assert!(fig8[3].point.y.0 > 15.0, "Smooth node on 2-node closed figure-8 cubic must slide along tangent");
}

#[test]
fn test_spec002_f17_join_open_contours_in_pen_tool_and_select_tool() {
    // 1A. PenTool: Join active open contour A to start point P0 of open contour B
    let mut state_pen = GlifoCanvasState::new(GlifoTheme::sonoma_default());
    state_pen.viewport.zoom = 1.0;
    state_pen.viewport.pan_offset = ScreenPoint::new(100.0, 500.0);
    state_pen.active_tool = CanvasTool::Pen;
    state_pen.contours = vec![
        vec![
            Node::new(NodeId(1), 0.0, 0.0, NodeType::Corner),
            Node::new(NodeId(2), 100.0, 0.0, NodeType::Corner),
        ],
        vec![
            Node::new(NodeId(3), 200.0, 0.0, NodeType::Corner),
            Node::new(NodeId(4), 300.0, 0.0, NodeType::Corner),
        ],
    ];
    state_pen.set_contour_closed(0, false);
    state_pen.set_contour_closed(1, false);
    state_pen.active_open_contour = Some(0);
    state_pen.sync_paths_from_contours();
    let before_pen_join = state_pen.snapshot();

    let b_p0_scr = state_pen.viewport.to_screen(FontPoint::new(200.0, 0.0));
    run_dispatch(
        &mut state_pen,
        vec![
            Event::PointerButton {
                pos: Pos2::new(b_p0_scr.x.0, b_p0_scr.y.0),
                button: PointerButton::Primary,
                pressed: true,
                modifiers: EguiModifiers::NONE,
            },
            Event::PointerButton {
                pos: Pos2::new(b_p0_scr.x.0, b_p0_scr.y.0),
                button: PointerButton::Primary,
                pressed: false,
                modifiers: EguiModifiers::NONE,
            },
        ],
        false, false, false, false, false, 1.0,
    );
    assert_eq!(state_pen.contours.len(), 1, "PenTool clicking P0 of open contour B must merge A and B into 1 open contour");
    assert!(!state_pen.is_contour_closed(0), "Merged contour must remain open");
    assert_eq!(state_pen.contours[0].len(), 4);
    assert_eq!(state_pen.contours[0][0].point, FontPoint::new(0.0, 0.0));
    assert_eq!(state_pen.contours[0][1].point, FontPoint::new(100.0, 0.0));
    assert_eq!(state_pen.contours[0][2].point, FontPoint::new(200.0, 0.0));
    assert_eq!(state_pen.contours[0][3].point, FontPoint::new(300.0, 0.0));
    state_pen.assert_invariants();
    assert_all_views_consistent(&state_pen);
    let after_pen_join = state_pen.snapshot();
    assert!(state_pen.undo());
    assert_eq!(state_pen.contours, before_pen_join.contours);
    assert!(state_pen.redo());
    assert_eq!(state_pen.contours, after_pen_join.contours);

    // 1B. PenTool: Join active open contour A (with pending_pen_bcp_out) to P_last of open contour B (reversing B and dragging cubic handles!)
    let mut state_pen_rev = GlifoCanvasState::new(GlifoTheme::sonoma_default());
    state_pen_rev.viewport.zoom = 1.0;
    state_pen_rev.viewport.pan_offset = ScreenPoint::new(100.0, 500.0);
    state_pen_rev.active_tool = CanvasTool::Pen;
    state_pen_rev.contours = vec![
        vec![
            Node::new(NodeId(1), 0.0, 0.0, NodeType::Corner),
            Node::new(NodeId(2), 100.0, 0.0, NodeType::Smooth),
        ],
        vec![
            Node::new(NodeId(3), 300.0, 0.0, NodeType::Corner),
            Node::new(NodeId(4), 200.0, 0.0, NodeType::Corner),
        ],
    ];
    state_pen_rev.set_contour_closed(0, false);
    state_pen_rev.set_contour_closed(1, false);
    state_pen_rev.active_open_contour = Some(0);
    state_pen_rev.pending_pen_bcp_out = Some((0, FontPoint::new(140.0, 60.0)));
    state_pen_rev.sync_paths_from_contours();

    let b_last_scr = state_pen_rev.viewport.to_screen(FontPoint::new(200.0, 0.0));
    let b_drag_scr = state_pen_rev.viewport.to_screen(FontPoint::new(240.0, 60.0));
    run_dispatch(
        &mut state_pen_rev,
        vec![
            Event::PointerButton {
                pos: Pos2::new(b_last_scr.x.0, b_last_scr.y.0),
                button: PointerButton::Primary,
                pressed: true,
                modifiers: EguiModifiers::NONE,
            },
            Event::PointerMoved(Pos2::new(b_drag_scr.x.0, b_drag_scr.y.0)),
            Event::PointerButton {
                pos: Pos2::new(b_drag_scr.x.0, b_drag_scr.y.0),
                button: PointerButton::Primary,
                pressed: false,
                modifiers: EguiModifiers::NONE,
            },
        ],
        false, false, false, false, false, 1.0,
    );
    assert_eq!(state_pen_rev.contours.len(), 1, "Joining onto P_last of B must merge A and reversed B into 1 open contour");
    assert!(!state_pen_rev.is_contour_closed(0));
    assert_eq!(state_pen_rev.contours[0].first().unwrap().point, FontPoint::new(0.0, 0.0));
    assert_eq!(state_pen_rev.contours[0].last().unwrap().point, FontPoint::new(300.0, 0.0));
    let rev_segs = extract_segments(&state_pen_rev.contours[0], false);
    assert_eq!(rev_segs.len(), 3);
    assert!(matches!(rev_segs[1], ContourSegment::Cubic { .. }), "Connecting segment with pending_pen_bcp_out + drag must be Cubic");
    state_pen_rev.assert_invariants();
    assert_all_views_consistent(&state_pen_rev);

    // 2A. SelectTool: Dragging P_last onto P0 of the SAME open contour (>= 2 remaining OnCurve nodes) merges endpoints and closes contour
    let mut state_sel_close = GlifoCanvasState::new(GlifoTheme::sonoma_default());
    state_sel_close.viewport.zoom = 1.0;
    state_sel_close.viewport.pan_offset = ScreenPoint::new(100.0, 500.0);
    state_sel_close.active_tool = CanvasTool::Select;
    state_sel_close.contours = vec![vec![
        Node::new(NodeId(1), 0.0, 0.0, NodeType::Corner),
        Node::new(NodeId(2), 100.0, 150.0, NodeType::Corner),
        Node::new(NodeId(3), 200.0, 50.0, NodeType::Corner),
    ]];
    state_sel_close.set_contour_closed(0, false);
    state_sel_close.sync_paths_from_contours();
    let before_sel_close = state_sel_close.snapshot();

    let p_last_scr = state_sel_close.viewport.to_screen(FontPoint::new(200.0, 50.0));
    let p0_scr = state_sel_close.viewport.to_screen(FontPoint::new(0.0, 0.0));
    run_dispatch(
        &mut state_sel_close,
        vec![
            Event::PointerButton {
                pos: Pos2::new(p_last_scr.x.0, p_last_scr.y.0),
                button: PointerButton::Primary,
                pressed: true,
                modifiers: EguiModifiers::NONE,
            },
            Event::PointerMoved(Pos2::new(p0_scr.x.0, p0_scr.y.0)),
            Event::PointerButton {
                pos: Pos2::new(p0_scr.x.0, p0_scr.y.0),
                button: PointerButton::Primary,
                pressed: false,
                modifiers: EguiModifiers::NONE,
            },
        ],
        false, false, false, false, false, 1.0,
    );
    assert!(state_sel_close.is_contour_closed(0), "Dragging P_last onto P0 of same open contour in SelectTool must close contour");
    assert_eq!(state_sel_close.contours[0].len(), 2, "Merged endpoints must leave 2 OnCurve nodes");
    assert_eq!(state_sel_close.contours[0][0].point, FontPoint::new(0.0, 0.0));
    state_sel_close.assert_invariants();
    assert_all_views_consistent(&state_sel_close);
    let after_sel_close = state_sel_close.snapshot();
    assert!(state_sel_close.undo());
    assert_eq!(state_sel_close.contours, before_sel_close.contours);
    assert!(!state_sel_close.is_contour_closed(0));
    assert!(state_sel_close.redo());
    assert_eq!(state_sel_close.contours, after_sel_close.contours);
    assert!(state_sel_close.is_contour_closed(0));

    // 2B. SelectTool: Dragging endpoint of open contour A onto endpoint of open contour B merges endpoints and joins A and B into 1 open contour
    let mut state_sel_join = GlifoCanvasState::new(GlifoTheme::sonoma_default());
    state_sel_join.viewport.zoom = 1.0;
    state_sel_join.viewport.pan_offset = ScreenPoint::new(100.0, 500.0);
    state_sel_join.active_tool = CanvasTool::Select;
    state_sel_join.contours = vec![
        vec![
            Node::new(NodeId(1), 0.0, 0.0, NodeType::Corner),
            Node::new(NodeId(2), 100.0, 50.0, NodeType::Corner),
        ],
        vec![
            Node::new(NodeId(3), 300.0, 0.0, NodeType::Corner),
            Node::new(NodeId(4), 200.0, 100.0, NodeType::Corner),
        ],
    ];
    state_sel_join.set_contour_closed(0, false);
    state_sel_join.set_contour_closed(1, false);
    state_sel_join.sync_paths_from_contours();
    let before_sel_join = state_sel_join.snapshot();

    let a_end_scr = state_sel_join.viewport.to_screen(FontPoint::new(100.0, 50.0));
    let b_end_scr = state_sel_join.viewport.to_screen(FontPoint::new(200.0, 100.0));
    run_dispatch(
        &mut state_sel_join,
        vec![
            Event::PointerButton {
                pos: Pos2::new(a_end_scr.x.0, a_end_scr.y.0),
                button: PointerButton::Primary,
                pressed: true,
                modifiers: EguiModifiers::NONE,
            },
            Event::PointerMoved(Pos2::new(b_end_scr.x.0, b_end_scr.y.0)),
            Event::PointerButton {
                pos: Pos2::new(b_end_scr.x.0, b_end_scr.y.0),
                button: PointerButton::Primary,
                pressed: false,
                modifiers: EguiModifiers::NONE,
            },
        ],
        false, false, false, false, false, 1.0,
    );
    assert_eq!(state_sel_join.contours.len(), 1, "Dropping A_last onto B_last in SelectTool must join A and B into 1 open contour");
    assert!(!state_sel_join.is_contour_closed(0), "Joined contour from two open contours must remain open");
    assert_eq!(state_sel_join.contours[0].len(), 3, "Two 2-node open contours merged at one shared endpoint must have 3 nodes");
    assert_eq!(state_sel_join.contours[0][0].point, FontPoint::new(0.0, 0.0));
    assert_eq!(state_sel_join.contours[0][1].point, FontPoint::new(200.0, 100.0));
    assert_eq!(state_sel_join.contours[0][2].point, FontPoint::new(300.0, 0.0));
    state_sel_join.assert_invariants();
    assert_all_views_consistent(&state_sel_join);
    let after_sel_join = state_sel_join.snapshot();
    assert!(state_sel_join.undo());
    assert_eq!(state_sel_join.contours, before_sel_join.contours);
    assert!(state_sel_join.redo());
    assert_eq!(state_sel_join.contours, after_sel_join.contours);

    // 2C. Boundary check: 2-node open contour dragging P_last onto P0 (< 2 remaining OnCurve nodes) must NOT collapse into a 1-node closed contour
    let mut state_2node = GlifoCanvasState::new(GlifoTheme::sonoma_default());
    state_2node.viewport.zoom = 1.0;
    state_2node.viewport.pan_offset = ScreenPoint::new(100.0, 500.0);
    state_2node.active_tool = CanvasTool::Select;
    state_2node.contours = vec![vec![
        Node::new(NodeId(1), 0.0, 0.0, NodeType::Corner),
        Node::new(NodeId(2), 100.0, 0.0, NodeType::Corner),
    ]];
    state_2node.set_contour_closed(0, false);
    state_2node.sync_paths_from_contours();
    let n2_scr = state_2node.viewport.to_screen(FontPoint::new(100.0, 0.0));
    let n1_scr = state_2node.viewport.to_screen(FontPoint::new(0.0, 0.0));
    run_dispatch(
        &mut state_2node,
        vec![
            Event::PointerButton {
                pos: Pos2::new(n2_scr.x.0, n2_scr.y.0),
                button: PointerButton::Primary,
                pressed: true,
                modifiers: EguiModifiers::NONE,
            },
            Event::PointerMoved(Pos2::new(n1_scr.x.0, n1_scr.y.0)),
            Event::PointerButton {
                pos: Pos2::new(n1_scr.x.0, n1_scr.y.0),
                button: PointerButton::Primary,
                pressed: false,
                modifiers: EguiModifiers::NONE,
            },
        ],
        false, false, false, false, false, 1.0,
    );
    assert!(!state_2node.is_contour_closed(0), "2-node open contour without handles must not collapse into 1-node closed contour");
    assert_eq!(state_2node.contours[0].len(), 2);
    state_2node.assert_invariants();

    // 3. Chaining 3 open contours A -> B -> C in sequence in PenTool with multi-step Undo/Redo
    let mut state_chain = GlifoCanvasState::new(GlifoTheme::sonoma_default());
    state_chain.viewport.zoom = 1.0;
    state_chain.viewport.pan_offset = ScreenPoint::new(100.0, 500.0);
    state_chain.active_tool = CanvasTool::Pen;
    state_chain.contours = vec![
        vec![
            Node::new(NodeId(1), 0.0, 0.0, NodeType::Corner),
            Node::new(NodeId(2), 80.0, 0.0, NodeType::Corner),
        ],
        vec![
            Node::new(NodeId(3), 160.0, 0.0, NodeType::Smooth),
            Node::new(NodeId(4), 240.0, 0.0, NodeType::Corner),
        ],
        vec![
            Node::new(NodeId(5), 320.0, 0.0, NodeType::Corner),
            Node::new(NodeId(6), 400.0, 0.0, NodeType::Corner),
        ],
    ];
    for idx in 0..3 {
        state_chain.set_contour_closed(idx, false);
    }
    state_chain.active_open_contour = Some(0);
    state_chain.pending_pen_bcp_out = Some((0, FontPoint::new(110.0, 40.0)));
    state_chain.sync_paths_from_contours();
    let chain_s0 = state_chain.snapshot();

    let b0_scr = state_chain.viewport.to_screen(FontPoint::new(160.0, 0.0));
    run_dispatch(
        &mut state_chain,
        vec![
            Event::PointerButton {
                pos: Pos2::new(b0_scr.x.0, b0_scr.y.0),
                button: PointerButton::Primary,
                pressed: true,
                modifiers: EguiModifiers::NONE,
            },
            Event::PointerButton {
                pos: Pos2::new(b0_scr.x.0, b0_scr.y.0),
                button: PointerButton::Primary,
                pressed: false,
                modifiers: EguiModifiers::NONE,
            },
        ],
        false, false, false, false, false, 1.0,
    );
    assert_eq!(state_chain.contours.len(), 2, "First join A -> B must leave 2 open contours");
    assert_eq!(
        state_chain.contours[0][4].node_type,
        NodeType::Corner,
        "Joining cubic A -> B onto straight-line B_0..B_1 must set B_0 to Corner"
    );
    let chain_s1 = state_chain.snapshot();

    let c0_scr = state_chain.viewport.to_screen(FontPoint::new(320.0, 0.0));
    run_dispatch(
        &mut state_chain,
        vec![
            Event::PointerButton {
                pos: Pos2::new(c0_scr.x.0, c0_scr.y.0),
                button: PointerButton::Primary,
                pressed: true,
                modifiers: EguiModifiers::NONE,
            },
            Event::PointerButton {
                pos: Pos2::new(c0_scr.x.0, c0_scr.y.0),
                button: PointerButton::Primary,
                pressed: false,
                modifiers: EguiModifiers::NONE,
            },
        ],
        false, false, false, false, false, 2.0,
    );
    assert_eq!(state_chain.contours.len(), 1, "Second join (AB) -> C must leave 1 open contour");
    let chain_s2 = state_chain.snapshot();
    assert!(state_chain.undo());
    assert_eq!(state_chain.contours, chain_s1.contours);
    assert!(state_chain.undo());
    assert_eq!(state_chain.contours, chain_s0.contours);
    assert!(state_chain.redo());
    assert_eq!(state_chain.contours, chain_s1.contours);
    assert!(state_chain.redo());
    assert_eq!(state_chain.contours, chain_s2.contours);
}

#[test]
fn test_spec002_f18_multi_node_ctrl_alt_drag_and_arrow_nudge_interpolation_without_duplicate() {
    let make_contour = || {
        let mut s = GlifoCanvasState::new(GlifoTheme::sonoma_default());
        s.viewport.zoom = 1.0;
        s.viewport.pan_offset = ScreenPoint::new(100.0, 500.0);
        s.contours = vec![vec![
            Node::new(NodeId(1), 0.0, 0.0, NodeType::Corner),
            Node::new(NodeId(2), 0.0, 55.0, NodeType::OffCurve),
            Node::new(NodeId(3), 45.0, 100.0, NodeType::OffCurve),
            Node::new(NodeId(4), 100.0, 100.0, NodeType::Smooth),
            Node::new(NodeId(5), 155.0, 100.0, NodeType::OffCurve),
            Node::new(NodeId(6), 200.0, 55.0, NodeType::OffCurve),
            Node::new(NodeId(7), 200.0, 0.0, NodeType::Corner),
        ]];
        s.active_contour_closed = true;
        s.sync_paths_from_contours();
        s
    };

    // Select TWO OnCurve nodes: NodeId(4) at (100, 100) and NodeId(7) at (200, 0).
    // Segment [NodeId(1) (0,0), NodeId(2) (0,55), NodeId(3) (45,100), NodeId(4) (100,100)] has ONE endpoint selected (NodeId(4)) and ONE unselected (NodeId(1)).
    // Segment [NodeId(4), NodeId(5), NodeId(6), NodeId(7)] has BOTH endpoints selected, so both handles move rigidly by (20, 30).
    let mut s_drag = make_contour();
    s_drag.selected_nodes = vec![NodeId(4), NodeId(7)];
    let p4_scr = s_drag.viewport.to_screen(FontPoint::new(100.0, 100.0));
    let p4_drag_scr = s_drag.viewport.to_screen(FontPoint::new(120.0, 130.0));
    let ctrl_alt_mods = EguiModifiers { alt: true, ctrl: true, ..Default::default() };

    run_dispatch(
        &mut s_drag,
        vec![
            Event::PointerButton {
                pos: Pos2::new(p4_scr.x.0, p4_scr.y.0),
                button: PointerButton::Primary,
                pressed: true,
                modifiers: ctrl_alt_mods,
            },
            Event::PointerMoved(Pos2::new(p4_drag_scr.x.0, p4_drag_scr.y.0)),
            Event::PointerButton {
                pos: Pos2::new(p4_drag_scr.x.0, p4_drag_scr.y.0),
                button: PointerButton::Primary,
                pressed: false,
                modifiers: ctrl_alt_mods,
            },
        ],
        false, true, false, true, false, 1.0,
    );

    assert_eq!(
        s_drag.contours.len(),
        1,
        "Ctrl + Alt + Drag on multiple OnCurve nodes must NOT trigger optionDragDuplicate"
    );
    // Boundary cubic segment [NodeId(1)..NodeId(4)] must be proportionally nudge-interpolated:
    assert!((s_drag.contours[0][0].point.x.0 - 0.0).abs() < 1e-4 && (s_drag.contours[0][0].point.y.0 - 0.0).abs() < 1e-4);
    assert!((s_drag.contours[0][1].point.x.0 - 0.0).abs() < 1e-4 && (s_drag.contours[0][1].point.y.0 - 71.5).abs() < 1e-4);
    assert!((s_drag.contours[0][2].point.x.0 - 54.0).abs() < 1e-4 && (s_drag.contours[0][2].point.y.0 - 130.0).abs() < 1e-4);
    assert!((s_drag.contours[0][3].point.x.0 - 120.0).abs() < 1e-4 && (s_drag.contours[0][3].point.y.0 - 130.0).abs() < 1e-4);
    // Interior cubic segment [NodeId(4)..NodeId(7)] has both endpoints selected, so handles (5) and (6) translate by (+20, +30):
    assert!((s_drag.contours[0][4].point.x.0 - 175.0).abs() < 1e-4 && (s_drag.contours[0][4].point.y.0 - 130.0).abs() < 1e-4);
    assert!((s_drag.contours[0][5].point.x.0 - 220.0).abs() < 1e-4 && (s_drag.contours[0][5].point.y.0 - 85.0).abs() < 1e-4);
    assert!((s_drag.contours[0][6].point.x.0 - 220.0).abs() < 1e-4 && (s_drag.contours[0][6].point.y.0 - 30.0).abs() < 1e-4);
    s_drag.assert_invariants();
    assert_all_views_consistent(&s_drag);

    // Same multi-node selection nudged via Ctrl + Alt + Arrow keys must produce identical coordinates
    let mut s_key = make_contour();
    s_key.selected_nodes = vec![NodeId(4), NodeId(7)];
    assert!(press_key(&mut s_key, Key::ArrowRight, true, true, true));
    assert!(press_key(&mut s_key, Key::ArrowRight, true, true, true));
    assert!(press_key(&mut s_key, Key::ArrowUp, true, true, true));
    assert!(press_key(&mut s_key, Key::ArrowUp, true, true, true));
    assert!(press_key(&mut s_key, Key::ArrowUp, true, true, true));
    assert_eq!(s_key.contours, s_drag.contours);
}

#[test]
fn test_spec002_f19_pen_resume_on_corner_line_endpoint_extrudes_only_bcp_out() {
    let mut state = GlifoCanvasState::new(GlifoTheme::sonoma_default());
    state.viewport.zoom = 1.0;
    state.viewport.pan_offset = ScreenPoint::new(100.0, 500.0);
    state.active_tool = CanvasTool::Pen;
    state.contours = vec![vec![
        Node::new(NodeId(1), 100.0, 100.0, NodeType::Corner),
        Node::new(NodeId(2), 300.0, 100.0, NodeType::Corner),
    ]];
    state.set_contour_closed(0, false);
    state.active_open_contour = None;
    state.sync_paths_from_contours();

    let p_last_scr = state.viewport.to_screen(FontPoint::new(300.0, 100.0));
    let drag_out_scr = state.viewport.to_screen(FontPoint::new(360.0, 150.0));
    let p_next_scr = state.viewport.to_screen(FontPoint::new(500.0, 100.0));

    run_dispatch(
        &mut state,
        vec![
            Event::PointerButton {
                pos: Pos2::new(p_last_scr.x.0, p_last_scr.y.0),
                button: PointerButton::Primary,
                pressed: true,
                modifiers: EguiModifiers::NONE,
            },
            Event::PointerMoved(Pos2::new(drag_out_scr.x.0, drag_out_scr.y.0)),
        ],
        false, false, false, false, false, 1.0,
    );

    // Mid-drag visual check: P_last at (300, 100) must remain rendered as blue Corner/open-endpoint (not overwritten by a phantom bcp_in handle circle at anchor)
    let mid_drag_img = pixmap_to_rgba_image(rasterize_canvas_subregion(&state, &state.theme, 640, 480));
    let px = mid_drag_img.get_pixel(p_last_scr.x.0 as u32, p_last_scr.y.0 as u32);
    let handle_c = state.theme.edit_canvas.bcp_handle_color32();
    assert_ne!(
        (px[0], px[1], px[2]),
        (handle_c.r(), handle_c.g(), handle_c.b()),
        "During DraggingHandles on resumed Corner P_last, anchor pixel must not be overwritten by phantom bcp_in handle circle"
    );
    assert!(px[2] > 200, "Anchor pixel at P_last must keep its blue Corner/open-endpoint color, got {px:?}");

    run_dispatch(
        &mut state,
        vec![Event::PointerButton {
            pos: Pos2::new(drag_out_scr.x.0, drag_out_scr.y.0),
            button: PointerButton::Primary,
            pressed: false,
            modifiers: EguiModifiers::NONE,
        }],
        false, false, false, false, false, 1.1,
    );

    assert_eq!(
        state.contours[0].len(),
        2,
        "Resuming and dragging on a Corner line endpoint P_last must NOT insert OffCurve handles into the preceding Line segment"
    );
    assert_eq!(
        state.contours[0][1].node_type,
        NodeType::Corner,
        "P_last after straight Line segment must remain NodeType::Corner when pulling out bcp_out"
    );
    assert_eq!(
        state.pending_pen_bcp_out,
        Some((0, FontPoint::new(360.0, 150.0))),
        "Dragging on resumed P_last must store outgoing handle in pending_pen_bcp_out"
    );

    // Next click at (500, 100) must create a Cubic segment using pending_pen_bcp_out while keeping segment 0 a Line!
    run_dispatch(
        &mut state,
        vec![
            Event::PointerButton {
                pos: Pos2::new(p_next_scr.x.0, p_next_scr.y.0),
                button: PointerButton::Primary,
                pressed: true,
                modifiers: EguiModifiers::NONE,
            },
            Event::PointerButton {
                pos: Pos2::new(p_next_scr.x.0, p_next_scr.y.0),
                button: PointerButton::Primary,
                pressed: false,
                modifiers: EguiModifiers::NONE,
            },
        ],
        false, false, false, false, false, 2.0,
    );
    let segs = extract_segments(&state.contours[0], false);
    assert_eq!(segs.len(), 2);
    assert!(matches!(segs[0], ContourSegment::Line { .. }), "First segment must remain a straight Line");
    assert!(matches!(segs[1], ContourSegment::Cubic { .. }), "Second segment must be Cubic from pending_pen_bcp_out");
    state.assert_invariants();
    assert_all_views_consistent(&state);
}

#[test]
fn test_spec002_f20_pen_integer_grid_rounding_preserves_sub_unit_split_and_extremum_precision() {
    let mut state = GlifoCanvasState::new(GlifoTheme::sonoma_default());
    state.viewport.zoom = 1.0;
    state.viewport.pan_offset = ScreenPoint::new(100.0, 500.0);
    state.active_tool = CanvasTool::Pen;
    assert_eq!(state.grid_step_upm, 1.0);

    let frac_pt = FontPoint::new(123.37, 245.82);
    assert_eq!(frac_pt.round_to_grid(), FontPoint::new(123.0, 246.0));
    let frac_scr = state.viewport.to_screen(frac_pt);

    run_dispatch(
        &mut state,
        vec![
            Event::PointerButton {
                pos: Pos2::new(frac_scr.x.0, frac_scr.y.0),
                button: PointerButton::Primary,
                pressed: true,
                modifiers: EguiModifiers::NONE,
            },
            Event::PointerButton {
                pos: Pos2::new(frac_scr.x.0, frac_scr.y.0),
                button: PointerButton::Primary,
                pressed: false,
                modifiers: EguiModifiers::NONE,
            },
        ],
        false, false, false, false, false, 1.0,
    );

    assert_eq!(
        state.contours[0][0].point,
        FontPoint::new(123.0, 246.0),
        "TC-GLIFO-015: PenTool click on empty space must snap new anchor to 1.0 UPM integer grid"
    );
    state.assert_invariants();
}

#[test]
fn test_spec002_f21_live_clang_runtime_parity_alt_line_convert_nudge_zero_span_and_drag_curve_265() {
    // 1. Option/Alt-Click on Line segment P0=(0,0) -> P1=(100,50) with round_to_grid (1.0 vs 0.0)
    let make_line_state = |grid_step: f64, round_grid: bool| {
        let mut s = GlifoCanvasState::new(GlifoTheme::sonoma_default());
        s.viewport.zoom = 1.0;
        s.viewport.pan_offset = ScreenPoint::new(100.0, 500.0);
        s.active_tool = CanvasTool::Select;
        s.grid_step_upm = grid_step;
        s.round_to_grid = round_grid;
        s.contours = vec![vec![
            Node::new(NodeId(1), 0.0, 0.0, NodeType::Corner),
            Node::new(NodeId(2), 100.0, 50.0, NodeType::Corner),
        ]];
        s.set_contour_closed(0, false);
        s.sync_paths_from_contours();
        s
    };

    let mut s_grid = make_line_state(1.0, true);
    let mid_scr = s_grid.viewport.to_screen(FontPoint::new(50.0, 25.0));
    let alt_mods = EguiModifiers { alt: true, ..Default::default() };
    run_dispatch(
        &mut s_grid,
        vec![
            Event::PointerButton {
                pos: Pos2::new(mid_scr.x.0, mid_scr.y.0),
                button: PointerButton::Primary,
                pressed: true,
                modifiers: alt_mods,
            },
            Event::PointerButton {
                pos: Pos2::new(mid_scr.x.0, mid_scr.y.0),
                button: PointerButton::Primary,
                pressed: false,
                modifiers: alt_mods,
            },
        ],
        false, true, false, false, false, 1.0,
    );
    assert_eq!(s_grid.contours[0].len(), 4);
    assert_eq!(
        s_grid.contours[0][1].point,
        FontPoint::new(33.0, 17.0),
        "Alt-Click on Line (0,0)->(100,50) with round_to_grid=true (grid_step_upm=1.0) must round H1 to (33.0, 17.0)"
    );
    assert_eq!(
        s_grid.contours[0][2].point,
        FontPoint::new(67.0, 33.0),
        "Alt-Click on Line (0,0)->(100,50) with round_to_grid=true (grid_step_upm=1.0) must round H2 to (67.0, 33.0)"
    );

    let mut s_unrounded = make_line_state(1.0, false);
    run_dispatch(
        &mut s_unrounded,
        vec![
            Event::PointerButton {
                pos: Pos2::new(mid_scr.x.0, mid_scr.y.0),
                button: PointerButton::Primary,
                pressed: true,
                modifiers: alt_mods,
            },
            Event::PointerButton {
                pos: Pos2::new(mid_scr.x.0, mid_scr.y.0),
                button: PointerButton::Primary,
                pressed: false,
                modifiers: alt_mods,
            },
        ],
        false, true, false, false, false, 1.0,
    );
    assert_eq!(s_unrounded.contours[0].len(), 4);
    assert!(
        (s_unrounded.contours[0][1].point.x.0 - 100.0 / 3.0).abs() < 1e-6
            && (s_unrounded.contours[0][1].point.y.0 - 50.0 / 3.0).abs() < 1e-6,
        "Alt-Click on Line (0,0)->(100,50) with grid_step_upm=0.0 must preserve exact 1/3 H1, got {:?}",
        s_unrounded.contours[0][1].point
    );
    assert!(
        (s_unrounded.contours[0][2].point.x.0 - 200.0 / 3.0).abs() < 1e-6
            && (s_unrounded.contours[0][2].point.y.0 - 100.0 / 3.0).abs() < 1e-6,
        "Alt-Click on Line (0,0)->(100,50) with grid_step_upm=0.0 must preserve exact 2/3 H2, got {:?}",
        s_unrounded.contours[0][2].point
    );

    // 2. Ctrl+Alt Nudge Overshoot Anchor Rule & Zero-Span Fallback (CanvasEngine spec_ref / spec_ref)
    let mut s_overshoot = GlifoCanvasState::new(GlifoTheme::sonoma_default());
    s_overshoot.contours = vec![vec![
        Node::new(NodeId(1), 0.0, 0.0, NodeType::Corner),
        Node::new(NodeId(2), 30.0, 60.0, NodeType::OffCurve),
        Node::new(NodeId(3), 70.0, 60.0, NodeType::OffCurve),
        Node::new(NodeId(4), 100.0, 0.0, NodeType::Corner),
        Node::new(NodeId(5), 70.0, -12.5, NodeType::OffCurve),
        Node::new(NodeId(6), 30.0, -30.0, NodeType::OffCurve),
    ]];
    s_overshoot.active_contour_closed = true;
    s_overshoot.sync_paths_from_contours();
    s_overshoot.selected_nodes = vec![NodeId(4)];
    s_overshoot.record_drag_start();
    s_overshoot.apply_node_displacement_with_modifiers_and_ctrl(10.0, 15.0, false, true, true);
    assert!(
        (s_overshoot.contours[0][1].point.x.0 - 33.0).abs() < 1e-4
            && (s_overshoot.contours[0][1].point.y.0 - 60.0).abs() < 1e-4
    );
    assert!(
        (s_overshoot.contours[0][2].point.x.0 - 77.0).abs() < 1e-4
            && (s_overshoot.contours[0][2].point.y.0 - 60.0).abs() < 1e-4
    );
    assert!(
        (s_overshoot.contours[0][4].point.x.0 - 77.0).abs() < 1e-4
            && (s_overshoot.contours[0][4].point.y.0 - (-3.75)).abs() < 1e-4,
        "Overshoot segment H3 must use ref_y = -40.0 -> (77.0, -3.75), got {:?}",
        s_overshoot.contours[0][4].point
    );
    assert!(
        (s_overshoot.contours[0][5].point.x.0 - 33.0).abs() < 1e-4
            && (s_overshoot.contours[0][5].point.y.0 - (-30.0)).abs() < 1e-4,
        "Overshoot segment H4 must use ref_y = -40.0 -> (33.0, -30.0), got {:?}",
        s_overshoot.contours[0][5].point
    );

    // Zero-span fallback: P0=(0,0), H1=(30,0), H2=(70,60), P3=(100,0) nudged at P3 by (0, +20)
    // Because span_y == 0.0 and |H2.y - P3.y| = 60.0 > 0.1, H2.y must get weight 0.0 and stay at 60.0!
    let mut s_zero_span = GlifoCanvasState::new(GlifoTheme::sonoma_default());
    s_zero_span.contours = vec![vec![
        Node::new(NodeId(1), 0.0, 0.0, NodeType::Corner),
        Node::new(NodeId(2), 30.0, 0.0, NodeType::OffCurve),
        Node::new(NodeId(3), 70.0, 60.0, NodeType::OffCurve),
        Node::new(NodeId(4), 100.0, 0.0, NodeType::Corner),
    ]];
    s_zero_span.set_contour_closed(0, false);
    s_zero_span.sync_paths_from_contours();
    s_zero_span.selected_nodes = vec![NodeId(4)];
    s_zero_span.record_drag_start();
    s_zero_span.apply_node_displacement_with_modifiers_and_ctrl(0.0, 20.0, false, true, true);
    assert!(
        (s_zero_span.contours[0][2].point.x.0 - 70.0).abs() < 1e-4
            && (s_zero_span.contours[0][2].point.y.0 - 60.0).abs() < 1e-4,
        "Zero-span fallback: when span_y == 0 and |H2.y - P3.y| > 0.1, H2 must get weight 0.0 and stay at (70, 60), got {:?}",
        s_zero_span.contours[0][2].point
    );

    // When H2.y == P3.y == 0.0 (collinear with P3 on Y), H2.y moves with P3 (+20.0)
    let mut s_collinear_span = GlifoCanvasState::new(GlifoTheme::sonoma_default());
    s_collinear_span.contours = vec![vec![
        Node::new(NodeId(1), 0.0, 0.0, NodeType::Corner),
        Node::new(NodeId(2), 30.0, 0.0, NodeType::OffCurve),
        Node::new(NodeId(3), 70.0, 0.0, NodeType::OffCurve),
        Node::new(NodeId(4), 100.0, 0.0, NodeType::Corner),
    ]];
    s_collinear_span.set_contour_closed(0, false);
    s_collinear_span.sync_paths_from_contours();
    s_collinear_span.selected_nodes = vec![NodeId(4)];
    s_collinear_span.record_drag_start();
    s_collinear_span.apply_node_displacement_with_modifiers_and_ctrl(0.0, 20.0, false, true, true);
    assert!(
        (s_collinear_span.contours[0][2].point.y.0 - 20.0).abs() < 1e-4,
        "Zero-span fallback: when H2.y == P3.y == 0.0, H2.y must move with P3 to 20.0, got {:?}",
        s_collinear_span.contours[0][2].point
    );

    // 3. Cubic Segment Drag Weights (CanvasEngine spec_ref..spec_ref: 2.65*(1-t) and 2.65*t, plus 6000.0 UPM max handle length clamp)
    for (t, exp_h1, exp_h2) in [
        (0.25, FontPoint::new(49.875, 99.75), FontPoint::new(76.625, 73.25)),
        (0.50, FontPoint::new(43.25, 86.50), FontPoint::new(83.25, 86.50)),
        (0.75, FontPoint::new(36.625, 73.25), FontPoint::new(89.875, 99.75)),
    ] {
        let mut nodes = vec![
            Node::new(NodeId(1), 0.0, 0.0, NodeType::Corner),
            Node::new(NodeId(2), 30.0, 60.0, NodeType::OffCurve),
            Node::new(NodeId(3), 70.0, 60.0, NodeType::OffCurve),
            Node::new(NodeId(4), 100.0, 0.0, NodeType::Corner),
        ];
        let seg = extract_segments(&nodes, false)[0];
        grafia::core::apply_cubic_segment_belly_drag_with_alt(
            &mut nodes,
            false,
            seg,
            t,
            FontPoint::new(30.0, 60.0),
            FontPoint::new(70.0, 60.0),
            10.0,
            20.0,
            false,
        );
        assert!(
            (nodes[1].point.x.0 - exp_h1.x.0).abs() < 1e-6
                && (nodes[1].point.y.0 - exp_h1.y.0).abs() < 1e-6,
            "t={t} H1 expected {exp_h1:?}, got {:?}",
            nodes[1].point
        );
        assert!(
            (nodes[2].point.x.0 - exp_h2.x.0).abs() < 1e-6
                && (nodes[2].point.y.0 - exp_h2.y.0).abs() < 1e-6,
            "t={t} H2 expected {exp_h2:?}, got {:?}",
            nodes[2].point
        );
    }

    // Angle-locked branch max handle length clamp at 6000.0 UPM
    let mut locked_nodes = vec![
        Node::new(NodeId(1), 0.0, 0.0, NodeType::Smooth),
        Node::new(NodeId(2), 0.0, 100.0, NodeType::OffCurve),
        Node::new(NodeId(3), 100.0, 100.0, NodeType::OffCurve),
        Node::new(NodeId(4), 100.0, 0.0, NodeType::Smooth),
    ];
    let locked_seg = extract_segments(&locked_nodes, false)[0];
    grafia::core::apply_cubic_segment_belly_drag_with_alt(
        &mut locked_nodes,
        false,
        locked_seg,
        0.5,
        FontPoint::new(0.0, 100.0),
        FontPoint::new(100.0, 100.0),
        0.0,
        20_000.0,
        false,
    );
    assert!(
        (locked_nodes[1].point.distance_to(FontPoint::new(0.0, 0.0)) - 6000.0).abs() < 1e-4,
        "Angle-locked handle length must be clamped to 6000.0 UPM, got {:?}",
        locked_nodes[1].point
    );
    assert!(
        (locked_nodes[2].point.distance_to(FontPoint::new(100.0, 0.0)) - 6000.0).abs() < 1e-4,
        "Angle-locked handle length must be clamped to 6000.0 UPM, got {:?}",
        locked_nodes[2].point
    );
}

