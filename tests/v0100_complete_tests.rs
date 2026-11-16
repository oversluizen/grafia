#![allow(
    clippy::too_many_lines,
    clippy::cognitive_complexity,
    clippy::expect_used,
    clippy::unwrap_used,
    clippy::panic,
    clippy::too_many_arguments,
    clippy::iter_over_hash_type
)]
#![allow(unexpected_cfgs)]
//! Normative COTS Verification Suite for `SPEC-011` (`grafia v0.10.0`):
//! $G^2$ Smoothness Continuity, Romer/Henestrosa Harmonize (`Ctrl+Shift+H`),
//! Interactive Tangent Balance (Tunni) Lines (`[0.20, 0.95]`), Proportional Handle Servo,
//! Interpolated Neighbour Nudge (`C`), Master Trajectory Kink & Crossing Detector,
//! Proactive Vector Outline Audit, Off-Axis Handle Warning / Ratio HUD, and Diagonal Apex Snapping (`TC-V0100-001` .. `TC-V0100-019`).

use approx::assert_relative_eq;
use arbtest::arbtest;
use egui::{Event, Key, Modifiers as EguiModifiers, Pos2, RawInput, Rect, Vec2};
use grafia::adapters::headless::rasterize_canvas_subregion;
use grafia::adapters::input_dispatcher::InputDispatcher;
use grafia::{
    cubic_signed_curvature, AppearanceMode, CompanionMasterLayer, ContinuityGrade, ContourId,
    FontPoint, GlyphContour, GrafiaCanvasState, GrafiaTheme, MasterCompatibilityMode, Node, NodeId,
    NodeType, OutlineAuditKind, TangentExtensionSnapKind, TUNNI_HIT_RADIUS_DP, TUNNI_MAX_RATIO,
    TUNNI_MIN_RATIO,
};
use proptest::prelude::*;
use rstest::rstest;

fn default_canvas_rect() -> Rect {
    Rect::from_min_size(Pos2::ZERO, Vec2::new(960.0, 600.0))
}

fn make_two_joined_cubics_contour(d_in: f64, d_out: f64) -> GlyphContour {
    let mut gc = GlyphContour::new(
        ContourId(0),
        vec![
            Node::new(NodeId(1), -100.0, d_in, NodeType::Corner),
            Node::new(NodeId(2), -60.0, d_in, NodeType::OffCurve),
            Node::new(NodeId(3), -40.0, 0.0, NodeType::OffCurve),
            Node::new(NodeId(4), 0.0, 0.0, NodeType::Smooth),
            Node::new(NodeId(5), 40.0, 0.0, NodeType::OffCurve),
            Node::new(NodeId(6), 60.0, d_out, NodeType::OffCurve),
            Node::new(NodeId(7), 100.0, d_out, NodeType::Smooth),
        ],
    );
    gc.closed = false;
    gc
}

fn make_tunni_arc_contour(h1_y: f64, h2_x: f64) -> GlyphContour {
    let mut gc = GlyphContour::new(
        ContourId(0),
        vec![
            Node::new(NodeId(10), 0.0, 0.0, NodeType::Corner),
            Node::new(NodeId(11), 0.0, h1_y, NodeType::OffCurve),
            Node::new(NodeId(12), h2_x, 100.0, NodeType::OffCurve),
            Node::new(NodeId(13), 100.0, 100.0, NodeType::Smooth),
        ],
    );
    gc.closed = false;
    gc
}

fn set_layer_contours(state: &mut GrafiaCanvasState, contours: Vec<GlyphContour>) {
    state.contours = contours.iter().map(|c| c.nodes.clone()).collect();
    state.active_contour_closed = contours.last().is_none_or(|c| c.closed);
    state.glyph.contours = contours;
    state.sync_paths_from_contours();
}

#[allow(clippy::too_many_arguments)]
fn dispatch_key(
    state: &mut GrafiaCanvasState,
    key: Key,
    shift: bool,
    alt: bool,
    cmd: bool,
    ctrl: bool,
) {
    let ctx = egui::Context::default();
    let m = EguiModifiers {
        alt,
        ctrl,
        shift,
        mac_cmd: cmd,
        command: cmd,
    };
    let events = vec![Event::Key {
        key,
        physical_key: Some(key),
        pressed: true,
        repeat: false,
        modifiers: m,
    }];
    let raw = RawInput {
        screen_rect: Some(default_canvas_rect()),
        time: Some(1.0),
        events,
        ..Default::default()
    };
    ctx.input_mut(|i| i.modifiers = m);
    let theme = state.theme.clone();
    let mut out = ctx.run_ui(raw, |ui| {
        let ev = ui.input(|i| i.events.clone());
        let _ = InputDispatcher::new(state, &theme, default_canvas_rect()).dispatch_events(&ev, ui);
    });
    out.textures_delta.clear();
}

// TC-V0100-001: Parametric cubic signed curvature kappa(t) evaluation.
#[test]
fn tc_v0100_001_cubic_signed_curvature_quarter_circle_and_collinear() {
    let k = 0.552_284_75_f64;
    let (p0, p1) = (FontPoint::new(100.0, 0.0), FontPoint::new(100.0, 100.0 * k));
    let (p2, p3) = (FontPoint::new(100.0 * k, 100.0), FontPoint::new(0.0, 100.0));
    let km = cubic_signed_curvature(p0, p1, p2, p3, 0.5);
    assert_relative_eq!(
        cubic_signed_curvature(p0, p1, p2, p3, 0.0),
        0.01,
        epsilon = 2e-3
    );
    assert_relative_eq!(km, 0.01, epsilon = 2e-3);
    assert_relative_eq!(
        cubic_signed_curvature(p0, p1, p2, p3, 1.0),
        0.01,
        epsilon = 2e-3
    );
    assert_relative_eq!(
        cubic_signed_curvature(p3, p2, p1, p0, 0.5),
        -km,
        epsilon = 1e-6
    );
    let k_line = cubic_signed_curvature(
        FontPoint::new(0.0, 0.0),
        FontPoint::new(30.0, 0.0),
        FontPoint::new(70.0, 0.0),
        FontPoint::new(100.0, 0.0),
        0.5,
    );
    assert_relative_eq!(k_line, 0.0, epsilon = 1e-7);
}

// TC-V0100-002, TC-V0100-003 & TC-V0100-004: Ctrl+Shift+H Romer/Henestrosa G2 Harmonize, handle-only harmonize & live G2 node.
#[test]
fn tc_v0100_002_to_004_g2_harmonize_anchor_handles_and_live_genius_node() {
    let mut state = GrafiaCanvasState::init_glyph_a();
    set_layer_contours(
        &mut state,
        vec![make_two_joined_cubics_contour(100.0, 25.0)],
    );
    state.selected_nodes = vec![NodeId(4)];
    dispatch_key(&mut state, Key::H, true, false, false, true);
    let n = &state.glyph.contours[0].nodes;
    let a = (n[3].point.x.0 - n[2].point.x.0).hypot(n[3].point.y.0 - n[2].point.y.0);
    let b = (n[4].point.x.0 - n[3].point.x.0).hypot(n[4].point.y.0 - n[3].point.y.0);
    assert_relative_eq!(a / b, 2.0, epsilon = 1e-3);
    assert_relative_eq!(
        cubic_signed_curvature(n[0].point, n[1].point, n[2].point, n[3].point, 1.0),
        cubic_signed_curvature(n[3].point, n[4].point, n[5].point, n[6].point, 0.0),
        epsilon = 1e-4
    );

    set_layer_contours(
        &mut state,
        vec![make_two_joined_cubics_contour(100.0, 25.0)],
    );
    state.harmonize_handles_keep_node(NodeId(4));
    let n2 = &state.glyph.contours[0].nodes;
    assert_relative_eq!(n2[3].point.x.0, 0.0, epsilon = 1e-4);
    assert_relative_eq!(
        cubic_signed_curvature(n2[0].point, n2[1].point, n2[2].point, n2[3].point, 1.0),
        cubic_signed_curvature(n2[3].point, n2[4].point, n2[5].point, n2[6].point, 0.0),
        epsilon = 1e-4
    );

    state.set_node_genius(NodeId(4), true);
    state.translate_node_upm(NodeId(3), FontPoint::new(-15.0, 0.0));
    let n3 = &state.glyph.contours[0].nodes;
    assert_relative_eq!(
        cubic_signed_curvature(n3[0].point, n3[1].point, n3[2].point, n3[3].point, 1.0),
        cubic_signed_curvature(n3[3].point, n3[4].point, n3[5].point, n3[6].point, 0.0),
        epsilon = 1e-4
    );

    // Outer off-curve handle move (Q1 = NodeId(2)) via interactive pointer drag must also re-harmonize genius_nodes / g3_nodes!
    let vp_q1 = state.active_slot_viewport();
    let q1_start_s = vp_q1.to_screen(state.find_node(NodeId(2)).unwrap().point);
    let q1_end_s = vp_q1.to_screen(FontPoint::new(-60.0, 140.0));
    let ctx_q1 = egui::Context::default();
    let raw_q1 = RawInput {
        screen_rect: Some(default_canvas_rect()),
        time: Some(1.5),
        events: vec![
            Event::PointerButton {
                pos: Pos2::new(q1_start_s.x.0, q1_start_s.y.0),
                button: egui::PointerButton::Primary,
                pressed: true,
                modifiers: EguiModifiers::NONE,
            },
            Event::PointerMoved(Pos2::new(q1_end_s.x.0, q1_end_s.y.0)),
            Event::PointerButton {
                pos: Pos2::new(q1_end_s.x.0, q1_end_s.y.0),
                button: egui::PointerButton::Primary,
                pressed: false,
                modifiers: EguiModifiers::NONE,
            },
        ],
        ..Default::default()
    };
    let theme_q1 = state.theme.clone();
    let mut out_q1 = ctx_q1.run_ui(raw_q1, |ui| {
        let ev = ui.input(|i| i.events.clone());
        let _ = InputDispatcher::new(&mut state, &theme_q1, default_canvas_rect())
            .dispatch_events(&ev, ui);
    });
    out_q1.textures_delta.clear();
    let n4 = &state.glyph.contours[0].nodes;
    assert_relative_eq!(
        cubic_signed_curvature(n4[0].point, n4[1].point, n4[2].point, n4[3].point, 1.0),
        cubic_signed_curvature(n4[3].point, n4[4].point, n4[5].point, n4[6].point, 0.0),
        epsilon = 1e-4
    );
}

// TC-V0100-005, TC-V0100-006 & TC-V0100-007: Tangent Balance (Tunni) overlay, hit-test, ratio clamping [0.20, 0.95] & balance.
#[rstest]
#[case::clamp_low(0.10, 0.20)]
#[case::mid_55(0.55, 0.55)]
#[case::mid_75(0.75, 0.75)]
#[case::clamp_high(0.99, 0.95)]
fn tc_v0100_005_to_007_tunni_lines_hit_test_clamp_and_balance(
    #[case] requested_ratio: f64,
    #[case] expected_ratio: f64,
) {
    assert_relative_eq!(TUNNI_MIN_RATIO, 0.20, epsilon = 1e-4);
    assert_relative_eq!(TUNNI_MAX_RATIO, 0.95, epsilon = 1e-4);
    assert_relative_eq!(TUNNI_HIT_RADIUS_DP, 6.0, epsilon = 1e-4);

    let mut state = GrafiaCanvasState::init_glyph_a();
    set_layer_contours(&mut state, vec![make_tunni_arc_contour(60.0, 40.0)]);
    state.toggle_show_tunni_lines();
    let overlays = state.compute_tunni_overlays();
    assert_eq!(overlays.len(), 1);
    assert_relative_eq!(overlays[0].tunni_point.x.0, 20.0, epsilon = 1e-3);
    assert_relative_eq!(overlays[0].tunni_point.y.0, 80.0, epsilon = 1e-3);
    assert_eq!(
        state.hit_test_tunni_line_at_upm(FontPoint::new(20.0, 80.0)),
        Some((ContourId(0), 0))
    );
    assert_eq!(
        state.hit_test_tunni_line_at_upm(FontPoint::new(200.0, 200.0)),
        None
    );

    state.scale_segment_tunni_ratio(ContourId(0), 0, requested_ratio);
    assert_relative_eq!(
        state.glyph.contours[0].nodes[1].point.y.0,
        100.0 * expected_ratio,
        epsilon = 1e-3
    );
    assert_relative_eq!(
        state.glyph.contours[0].nodes[2].point.x.0,
        100.0 * (1.0 - expected_ratio),
        epsilon = 1e-3
    );

    if (requested_ratio - 0.55).abs() < 1e-4 {
        set_layer_contours(&mut state, vec![make_tunni_arc_contour(40.0, 20.0)]);
        state.balance_segment_tunni(ContourId(0), 0);
        assert_relative_eq!(
            state.glyph.contours[0].nodes[1].point.y.0,
            60.0,
            epsilon = 1e-3
        );
        assert_relative_eq!(
            state.glyph.contours[0].nodes[2].point.x.0,
            40.0,
            epsilon = 1e-3
        );

        // Interactive pointer drag on Tunni midpoint (20, 80) -> (10, 90) via InputDispatcher + Undo/Redo.
        let snap_before_drag = state.snapshot();
        let vp = state.active_slot_viewport();
        let start_scr = vp.to_screen(FontPoint::new(20.0, 80.0));
        let end_scr = vp.to_screen(FontPoint::new(10.0, 90.0));
        let ctx = egui::Context::default();
        let raw = RawInput {
            screen_rect: Some(default_canvas_rect()),
            time: Some(2.0),
            events: vec![
                Event::PointerButton {
                    pos: Pos2::new(start_scr.x.0, start_scr.y.0),
                    button: egui::PointerButton::Primary,
                    pressed: true,
                    modifiers: EguiModifiers::NONE,
                },
                Event::PointerMoved(Pos2::new(end_scr.x.0, end_scr.y.0)),
                Event::PointerButton {
                    pos: Pos2::new(end_scr.x.0, end_scr.y.0),
                    button: egui::PointerButton::Primary,
                    pressed: false,
                    modifiers: EguiModifiers::NONE,
                },
            ],
            ..Default::default()
        };
        let theme = state.theme.clone();
        let mut out = ctx.run_ui(raw, |ui| {
            let ev = ui.input(|i| i.events.clone());
            let _ = InputDispatcher::new(&mut state, &theme, default_canvas_rect())
                .dispatch_events(&ev, ui);
        });
        out.textures_delta.clear();
        assert_relative_eq!(
            state.glyph.contours[0].nodes[1].point.y.0,
            80.0,
            epsilon = 1.0
        );
        assert!(state.undo() && state.snapshot() == snap_before_drag);
    }
}

// TC-V0100-008, TC-V0100-009, TC-V0100-010 & TC-V0100-011: Servo scaling, Neighbour Nudge (C), Undo/Redo & rendering.
#[test]
fn tc_v0100_008_to_011_servo_scaling_power_nudge_undo_and_rendering() {
    let mut state = GrafiaCanvasState::init_glyph_a();
    set_layer_contours(&mut state, vec![make_tunni_arc_contour(60.0, 40.0)]);
    let snap_0 = state.snapshot();
    state.servo_translate_on_curve_node(NodeId(10), FontPoint::new(-50.0, 0.0));
    assert_relative_eq!(
        state.glyph.contours[0].nodes[0].point.x.0,
        -50.0,
        epsilon = 1e-3
    );
    assert_relative_eq!(
        state.glyph.contours[0].nodes[2].point.x.0,
        10.0,
        epsilon = 1e-3
    );
    let snap_1 = state.snapshot();
    assert!(
        state.undo() && state.snapshot() == snap_0 && state.redo() && state.snapshot() == snap_1
    );

    dispatch_key(&mut state, Key::C, false, false, false, false);
    assert!(state.power_nudge_enabled);
    let mut stem = GlyphContour::new(
        ContourId(0),
        vec![
            Node::new(NodeId(20), 0.0, 0.0, NodeType::Corner),
            Node::new(NodeId(21), 100.0, 0.0, NodeType::Corner),
            Node::new(NodeId(22), 200.0, 0.0, NodeType::Corner),
            Node::new(NodeId(23), 400.0, 0.0, NodeType::Corner),
        ],
    );
    stem.closed = false;
    set_layer_contours(&mut state, vec![stem]);
    state.selected_nodes = vec![NodeId(22)];
    let snap_pn_0 = state.snapshot();
    dispatch_key(&mut state, Key::ArrowRight, true, false, false, false);
    assert_relative_eq!(
        state.glyph.contours[0].nodes[0].point.x.0,
        0.0,
        epsilon = 1e-3
    );
    assert_relative_eq!(
        state.glyph.contours[0].nodes[1].point.x.0,
        105.0,
        epsilon = 1e-3
    );
    assert_relative_eq!(
        state.glyph.contours[0].nodes[2].point.x.0,
        210.0,
        epsilon = 1e-3
    );
    assert!(state.undo() && state.snapshot() == snap_pn_0);

    state.power_nudge_translate_selected(FontPoint::new(40.0, 0.0));
    assert_relative_eq!(
        state.glyph.contours[0].nodes[0].point.x.0,
        0.0,
        epsilon = 1e-3
    );
    assert_relative_eq!(
        state.glyph.contours[0].nodes[1].point.x.0,
        120.0,
        epsilon = 1e-3
    );
    assert_relative_eq!(
        state.glyph.contours[0].nodes[2].point.x.0,
        240.0,
        epsilon = 1e-3
    );
    assert!(state.undo() && state.snapshot() == snap_pn_0);

    // Interactive mouse node dragging (SelectToolState::DraggingNodes) must also respect power_nudge_enabled!
    let vp_pn = state.active_slot_viewport();
    let pn_start = vp_pn.to_screen(FontPoint::new(200.0, 0.0));
    let pn_end = vp_pn.to_screen(FontPoint::new(240.0, 0.0));
    let ctx_pn = egui::Context::default();
    let raw_pn = RawInput {
        screen_rect: Some(default_canvas_rect()),
        time: Some(2.5),
        events: vec![
            Event::PointerButton {
                pos: Pos2::new(pn_start.x.0, pn_start.y.0),
                button: egui::PointerButton::Primary,
                pressed: true,
                modifiers: EguiModifiers::NONE,
            },
            Event::PointerMoved(Pos2::new(pn_end.x.0, pn_end.y.0)),
            Event::PointerButton {
                pos: Pos2::new(pn_end.x.0, pn_end.y.0),
                button: egui::PointerButton::Primary,
                pressed: false,
                modifiers: EguiModifiers::NONE,
            },
        ],
        ..Default::default()
    };
    let theme_pn = state.theme.clone();
    let mut out_pn = ctx_pn.run_ui(raw_pn, |ui| {
        let ev = ui.input(|i| i.events.clone());
        let _ = InputDispatcher::new(&mut state, &theme_pn, default_canvas_rect())
            .dispatch_events(&ev, ui);
    });
    out_pn.textures_delta.clear();
    assert_relative_eq!(
        state.glyph.contours[0].nodes[1].point.x.0,
        120.0,
        epsilon = 1.5
    );
    assert_relative_eq!(
        state.glyph.contours[0].nodes[2].point.x.0,
        240.0,
        epsilon = 1.5
    );
    state.power_nudge_enabled = false;

    // Interactive mouse node dragging must also respect curve_harmony.servo_scaling_enabled!
    set_layer_contours(&mut state, vec![make_tunni_arc_contour(60.0, 40.0)]);
    state.curve_harmony.servo_scaling_enabled = true;
    state.preferences.snap_targets.nodes = false;
    state.show_metrics = false;
    state.show_grid = false;
    let vp_sv = state.active_slot_viewport();
    let sv_start = vp_sv.to_screen(FontPoint::new(0.0, 0.0));
    let sv_end = vp_sv.to_screen(FontPoint::new(-50.0, 0.0));
    let raw_sv = RawInput {
        screen_rect: Some(default_canvas_rect()),
        time: Some(2.8),
        events: vec![
            Event::PointerButton {
                pos: Pos2::new(sv_start.x.0, sv_start.y.0),
                button: egui::PointerButton::Primary,
                pressed: true,
                modifiers: EguiModifiers::NONE,
            },
            Event::PointerMoved(Pos2::new(sv_end.x.0, sv_end.y.0)),
            Event::PointerButton {
                pos: Pos2::new(sv_end.x.0, sv_end.y.0),
                button: egui::PointerButton::Primary,
                pressed: false,
                modifiers: EguiModifiers::NONE,
            },
        ],
        ..Default::default()
    };
    let mut out_sv = ctx_pn.run_ui(raw_sv, |ui| {
        let ev = ui.input(|i| i.events.clone());
        let _ = InputDispatcher::new(&mut state, &theme_pn, default_canvas_rect())
            .dispatch_events(&ev, ui);
    });
    out_sv.textures_delta.clear();
    assert_relative_eq!(
        state.glyph.contours[0].nodes[2].point.x.0,
        10.0,
        epsilon = 1.5
    );
    state.curve_harmony.servo_scaling_enabled = false;
    state.preferences.snap_targets.nodes = true;
    state.show_metrics = true;
    state.show_grid = true;

    set_layer_contours(&mut state, vec![make_two_joined_cubics_contour(80.0, 40.0)]);
    state.set_node_genius(NodeId(4), true);
    state.set_node_g3_lock(NodeId(7), true);
    state.show_tunni_lines = true;
    let mut out_badges = ctx_pn.run_ui(RawInput::default(), |ui| {
        let _ = grafia::GrafiaCanvasWidget::new(&mut state, &theme_pn).show(ui);
    });
    out_badges.textures_delta.clear();
    let has_star_10_poly = out_badges
        .shapes
        .iter()
        .any(|cs| matches!(&cs.shape, egui::Shape::Path(p) if p.closed && p.points.len() == 10));
    let has_diamond_4_poly = out_badges
        .shapes
        .iter()
        .any(|cs| matches!(&cs.shape, egui::Shape::Path(p) if p.closed && p.points.len() == 4));
    assert!(
        has_star_10_poly && has_diamond_4_poly,
        "G2Curvature (genius_nodes) must draw 10-vertex star (★) and G3Clothoid (g3_nodes) must draw 4-vertex diamond (❖)"
    );
    state.set_appearance_mode(AppearanceMode::Light);
    let light_px = rasterize_canvas_subregion(&state, &GrafiaTheme::sonoma_default(), 320, 180);
    state.set_appearance_mode(AppearanceMode::Dark);
    let dark_px = rasterize_canvas_subregion(&state, &GrafiaTheme::sonoma_dark(), 320, 180);
    assert_ne!(light_px.data(), dark_px.data());
}

// TC-V0100-013: MasterCompatibilityMode::FullNodeTrajectories, trajectory crossing & smooth-node kink detector.
#[test]
fn tc_v0100_013_master_full_trajectories_crossings_and_smooth_kink_detector() {
    let mut state = GrafiaCanvasState::init_glyph_a();
    state.set_master_compatibility_mode(MasterCompatibilityMode::FullNodeTrajectories);
    set_layer_contours(
        &mut state,
        vec![make_two_joined_cubics_contour(100.0, 50.0)],
    );
    // Companion layer with rotated tangent angle (>1 deg) and unequal handle ratio (|r_A - r_B| > 0.05) at Smooth NodeId(4),
    // plus crossed node trajectories between nodes 0 and 1.
    let comp_nodes = vec![
        Node::new(NodeId(1), -40.0, 120.0, NodeType::Corner),
        Node::new(NodeId(2), -110.0, 120.0, NodeType::OffCurve),
        Node::new(NodeId(3), -20.0, -5.0, NodeType::OffCurve),
        Node::new(NodeId(4), 0.0, 0.0, NodeType::Smooth),
        Node::new(NodeId(5), 80.0, 20.0, NodeType::OffCurve),
        Node::new(NodeId(6), 90.0, 60.0, NodeType::OffCurve),
        Node::new(NodeId(7), 120.0, 60.0, NodeType::Smooth),
    ];
    state.set_companion_master_layers(vec![CompanionMasterLayer {
        id: 1,
        name: "Bold".into(),
        color_rgba: [220, 60, 80, 180],
        contours: vec![comp_nodes],
        closed_flags: vec![false],
        anchors: vec![],
        editable: false,
    }]);
    let report = state.compute_master_kinks_and_crossings();
    assert_eq!(report.node_trajectories.len(), 7);
    assert!(!report.crossings.is_empty());
    assert!(report
        .kink_warnings
        .iter()
        .any(|k| k.node_id == NodeId(4) && k.companion_layer_id == 1 && k.severity > 0.0));
}

// TC-V0100-014: Proactive Vector Outline Audit (MissingExtremum, QuasiOrthogonalStem, CrossedHandles, CollinearCubic).
#[test]
fn tc_v0100_014_proactive_vector_outline_audit_four_defects() {
    let mut state = GrafiaCanvasState::init_glyph_a();
    let missing_ext = GlyphContour::new(
        ContourId(0),
        vec![
            Node::new(NodeId(1), 0.0, 0.0, NodeType::Corner),
            Node::new(NodeId(2), 0.0, 160.0, NodeType::OffCurve),
            Node::new(NodeId(3), 100.0, 160.0, NodeType::OffCurve),
            Node::new(NodeId(4), 100.0, 20.0, NodeType::Corner),
        ],
    );
    let quasi_stem = GlyphContour::new(
        ContourId(1),
        vec![
            Node::new(NodeId(10), 200.0, 0.0, NodeType::Corner),
            Node::new(NodeId(11), 203.0, 300.0, NodeType::Corner),
            Node::new(NodeId(12), 280.0, 300.0, NodeType::Corner),
        ],
    );
    let crossed = GlyphContour::new(
        ContourId(2),
        vec![
            Node::new(NodeId(20), 0.0, 0.0, NodeType::Corner),
            Node::new(NodeId(21), 100.0, 100.0, NodeType::OffCurve),
            Node::new(NodeId(22), 0.0, 100.0, NodeType::OffCurve),
            Node::new(NodeId(23), 100.0, 0.0, NodeType::Corner),
        ],
    );
    let collinear = GlyphContour::new(
        ContourId(3),
        vec![
            Node::new(NodeId(30), 0.0, 0.0, NodeType::Corner),
            Node::new(NodeId(31), 30.0, 0.01, NodeType::OffCurve),
            Node::new(NodeId(32), 70.0, -0.01, NodeType::OffCurve),
            Node::new(NodeId(33), 100.0, 0.0, NodeType::Corner),
        ],
    );
    set_layer_contours(
        &mut state,
        vec![
            missing_ext,
            quasi_stem.clone(),
            crossed.clone(),
            collinear.clone(),
        ],
    );
    let findings = state.audit_layer_outlines();
    assert!(findings.iter().any(|f| matches!(
        f.kind,
        OutlineAuditKind::MissingExtremum { contour_idx: 0, .. }
    )));
    assert!(findings.iter().any(|f| matches!(
        f.kind,
        OutlineAuditKind::QuasiOrthogonalStem { contour_idx: 1, .. }
    )));
    assert!(findings.iter().any(|f| matches!(
        f.kind,
        OutlineAuditKind::CrossedHandles { contour_idx: 2, .. }
    )));
    assert!(findings.iter().any(|f| matches!(
        f.kind,
        OutlineAuditKind::CollinearCubic { contour_idx: 3, .. }
    )));

    // Slanted extremum check: monotonic in upright (x'(t) > 0, y'(t) > 0), but has interior x_slant'(t) = 0 at italic_angle_deg = 12.0.
    let mut slanted_arch = GlyphContour::new(
        ContourId(4),
        vec![
            Node::new(NodeId(40), 0.0, 0.0, NodeType::Corner),
            Node::new(NodeId(41), 5.0, 100.0, NodeType::OffCurve),
            Node::new(NodeId(42), 5.0, 200.0, NodeType::OffCurve),
            Node::new(NodeId(43), 60.0, 300.0, NodeType::Corner),
        ],
    );
    slanted_arch.closed = false;
    set_layer_contours(&mut state, vec![slanted_arch]);
    state.italic_angle_deg = 0.0;
    assert!(!state
        .audit_layer_outlines()
        .iter()
        .any(|f| matches!(f.kind, OutlineAuditKind::MissingExtremum { .. })));
    state.italic_angle_deg = 12.0;
    assert!(state.audit_layer_outlines().iter().any(|f| matches!(
        f.kind,
        OutlineAuditKind::MissingExtremum { contour_idx: 0, .. }
    )));
    state.italic_angle_deg = 0.0;

    // Visual Red Arrow verification for each non-MissingExtremum defect + AngledHandleWarning alone:
    for single_defect in [quasi_stem, crossed, collinear] {
        set_layer_contours(&mut state, vec![single_defect]);
        state.curve_harmony.show_outline_audit = false;
        let off_px = rasterize_canvas_subregion(&state, &GrafiaTheme::sonoma_default(), 480, 360);
        state.curve_harmony.show_outline_audit = true;
        let on_px = rasterize_canvas_subregion(&state, &GrafiaTheme::sonoma_default(), 480, 360);
        assert!(
            off_px.data() != on_px.data(),
            "show_outline_audit must render vector Red Arrow for every OutlineAuditKind variant"
        );
    }

    let mut angled_only = GlyphContour::new(
        ContourId(0),
        vec![
            Node::new(NodeId(50), 0.0, 0.0, NodeType::Corner),
            Node::new(NodeId(51), 0.0, 55.0, NodeType::OffCurve),
            Node::new(NodeId(52), 45.0, 102.5, NodeType::OffCurve),
            Node::new(NodeId(53), 100.0, 100.0, NodeType::Smooth),
            Node::new(NodeId(54), 155.0, 97.5, NodeType::OffCurve),
            Node::new(NodeId(55), 200.0, 55.0, NodeType::OffCurve),
            Node::new(NodeId(56), 200.0, 0.0, NodeType::Corner),
        ],
    );
    angled_only.closed = false;
    set_layer_contours(&mut state, vec![angled_only]);
    assert!(!state.compute_angled_handle_warnings().is_empty());
    state.curve_harmony.show_outline_audit = false;
    let ang_off = rasterize_canvas_subregion(&state, &GrafiaTheme::sonoma_default(), 480, 360);
    state.curve_harmony.show_outline_audit = true;
    let ang_on = rasterize_canvas_subregion(&state, &GrafiaTheme::sonoma_default(), 480, 360);
    assert!(
        ang_off.data() != ang_on.data(),
        "show_outline_audit must highlight AngledHandleWarning ray and degree pill"
    );
    let h_scr = state
        .active_slot_viewport()
        .to_screen(FontPoint::new(45.0, 102.5));
    let (hx, hy) = (
        h_scr.x.0.round().clamp(0.0, 479.0) as usize,
        h_scr.y.0.round().clamp(0.0, 359.0) as usize,
    );
    let h_idx = (hy * 480 + hx) * 4;
    assert_ne!(
        &ang_off.data()[h_idx..h_idx + 4],
        &ang_on.data()[h_idx..h_idx + 4],
        "AngledHandleWarning handle knob center pixel must be highlighted in warning color, not overwritten by default handle pass"
    );
}

// TC-V0100-015 & TC-V0100-016: Off-axis handle warning, handle-ratio HUD, and diagonal tangent extension / virtual apex snap.
#[test]
fn tc_v0100_015_and_016_angled_handle_hud_and_diagonal_virtual_apex_snap() {
    let mut state = GrafiaCanvasState::init_glyph_a();
    // Quarter arc with P0=(0,0) Smooth, P1=(0,60), P2=(25,103) (tilted ~3 deg off horizontal at Smooth P3=(100,100)).
    let mut arc = make_tunni_arc_contour(60.0, 25.0);
    arc.nodes[0].node_type = NodeType::Smooth;
    arc.nodes[2].point = FontPoint::new(25.0, 104.0);
    set_layer_contours(&mut state, vec![arc]);

    let warnings = state.compute_angled_handle_warnings();
    assert!(warnings.iter().any(|w| w.smooth_node_id == NodeId(13)
        && w.handle_node_id == NodeId(12)
        && w.deviation_deg > 1.0));

    set_layer_contours(&mut state, vec![make_tunni_arc_contour(60.0, 25.0)]);
    let hud = state.compute_segment_handle_ratios();
    assert_eq!(hud.len(), 1);
    assert_relative_eq!(hud[0].ratio_out, 60.0, epsilon = 1e-2);
    assert_relative_eq!(hud[0].ratio_in, 75.0, epsilon = 1e-2);

    // TC-V0100-016: Truncated /A diagonal legs converging toward virtual apex (200, 700).
    let mut leg_l = GlyphContour::new(
        ContourId(0),
        vec![
            Node::new(NodeId(1), 0.0, 0.0, NodeType::Corner),
            Node::new(NodeId(2), 100.0, 350.0, NodeType::Corner),
        ],
    );
    let mut leg_r = GlyphContour::new(
        ContourId(1),
        vec![
            Node::new(NodeId(3), 400.0, 0.0, NodeType::Corner),
            Node::new(NodeId(4), 300.0, 350.0, NodeType::Corner),
        ],
    );
    leg_l.closed = false;
    leg_r.closed = false;
    set_layer_contours(&mut state, vec![leg_l, leg_r]);

    let apex_snap = state
        .compute_tangent_extension_snaps(FontPoint::new(198.0, 697.0), 10.0)
        .expect("virtual apex");
    assert!(matches!(
        apex_snap.kind,
        TangentExtensionSnapKind::VirtualApex { .. }
    ));
    assert_relative_eq!(apex_snap.snap_point.x.0, 200.0, epsilon = 1e-2);
    assert_relative_eq!(apex_snap.snap_point.y.0, 700.0, epsilon = 1e-2);

    let ray_snap = state
        .compute_tangent_extension_snaps(FontPoint::new(152.0, 525.0), 10.0)
        .expect("ray extension");
    assert!(matches!(
        ray_snap.kind,
        TangentExtensionSnapKind::RayExtension { contour_idx: 0, .. }
    ));
    assert_relative_eq!(ray_snap.snap_point.x.0, 150.0, epsilon = 1.0);

    // Live single-node drag of NodeId(2) (100, 350) near virtual apex (198, 697) must snap to (200, 700)!
    let vp = state.active_slot_viewport();
    let start_s = vp.to_screen(FontPoint::new(100.0, 350.0));
    let target_s = vp.to_screen(FontPoint::new(198.0, 697.0));
    let ctx = egui::Context::default();
    let raw = RawInput {
        screen_rect: Some(default_canvas_rect()),
        time: Some(3.0),
        events: vec![
            Event::PointerButton {
                pos: Pos2::new(start_s.x.0, start_s.y.0),
                button: egui::PointerButton::Primary,
                pressed: true,
                modifiers: EguiModifiers::NONE,
            },
            Event::PointerMoved(Pos2::new(target_s.x.0, target_s.y.0)),
        ],
        ..Default::default()
    };
    let theme = state.theme.clone();
    let mut out = ctx.run_ui(raw, |ui| {
        let ev = ui.input(|i| i.events.clone());
        let _ = InputDispatcher::new(&mut state, &theme, default_canvas_rect())
            .dispatch_events(&ev, ui);
    });
    out.textures_delta.clear();
    let moved_pt = state.find_node(NodeId(2)).expect("NodeId(2)").point;
    assert_relative_eq!(moved_pt.x.0, 200.0, epsilon = 1e-2);
    assert_relative_eq!(moved_pt.y.0, 700.0, epsilon = 1e-2);
}

// TC-V0100-017: Automatic cross-master contour order, winding & cyclic Node 0 matching (`auto_match_master_compatibility`).
#[test]
fn tc_v0100_017_auto_match_master_compatibility_order_winding_and_cyclic_shift() {
    let mut state = GrafiaCanvasState::init_glyph_a();
    state.set_master_compatibility_mode(MasterCompatibilityMode::FullNodeTrajectories);

    let ref_c0 = vec![
        Node::new(NodeId(100), 0.0, 0.0, NodeType::Corner),
        Node::new(NodeId(101), 0.0, 400.0, NodeType::Corner),
        Node::new(NodeId(102), 200.0, 400.0, NodeType::Corner),
        Node::new(NodeId(103), 200.0, 0.0, NodeType::Corner),
    ];
    let ref_c1 = vec![
        Node::new(NodeId(200), 500.0, 100.0, NodeType::Corner),
        Node::new(NodeId(201), 500.0, 300.0, NodeType::Corner),
        Node::new(NodeId(202), 700.0, 300.0, NodeType::Corner),
        Node::new(NodeId(203), 700.0, 100.0, NodeType::Corner),
    ];
    state.set_companion_master_layers(vec![CompanionMasterLayer {
        id: 1,
        name: "Bold".into(),
        color_rgba: [220, 60, 80, 180],
        contours: vec![ref_c0, ref_c1],
        closed_flags: vec![true, true],
        anchors: vec![],
        editable: false,
    }]);

    // Foreground has contour 1 first (with Node 0 shifted by 2) and contour 0 second (with reversed winding).
    let fg_first_shifted = GlyphContour::new(
        ContourId(0),
        vec![
            Node::new(NodeId(1), 690.0, 310.0, NodeType::Corner),
            Node::new(NodeId(2), 690.0, 90.0, NodeType::Corner),
            Node::new(NodeId(3), 510.0, 90.0, NodeType::Corner),
            Node::new(NodeId(4), 510.0, 310.0, NodeType::Corner),
        ],
    );
    let fg_second_reversed = GlyphContour::new(
        ContourId(1),
        vec![
            Node::new(NodeId(10), 0.0, 0.0, NodeType::Corner),
            Node::new(NodeId(11), 210.0, 0.0, NodeType::Corner),
            Node::new(NodeId(12), 210.0, 410.0, NodeType::Corner),
            Node::new(NodeId(13), 0.0, 410.0, NodeType::Corner),
        ],
    );
    set_layer_contours(&mut state, vec![fg_first_shifted, fg_second_reversed]);
    let snap_before = state.snapshot();

    assert!(state.auto_match_master_compatibility(1));
    assert!(state
        .compute_master_kinks_and_crossings()
        .crossings
        .is_empty());
    assert_relative_eq!(
        state.glyph.contours[0].nodes[0].point.x.0,
        0.0,
        epsilon = 1e-3
    );
    assert_relative_eq!(
        state.glyph.contours[0].nodes[1].point.y.0,
        410.0,
        epsilon = 1e-3
    );
    assert_relative_eq!(
        state.glyph.contours[1].nodes[0].point.x.0,
        510.0,
        epsilon = 1e-3
    );
    assert_relative_eq!(
        state.glyph.contours[1].nodes[0].point.y.0,
        90.0,
        epsilon = 1e-3
    );

    let snap_after = state.snapshot();
    assert!(state.undo() && state.snapshot() == snap_before);
    assert!(state.redo() && state.snapshot() == snap_after);

    // Concentric rings sharing identical center (200, 200) + open contour with reversed endpoints.
    let ref_outer_ring = vec![
        Node::new(NodeId(300), 0.0, 0.0, NodeType::Corner),
        Node::new(NodeId(301), 0.0, 400.0, NodeType::Corner),
        Node::new(NodeId(302), 400.0, 400.0, NodeType::Corner),
        Node::new(NodeId(303), 400.0, 0.0, NodeType::Corner),
    ];
    let ref_inner_ring = vec![
        Node::new(NodeId(310), 100.0, 100.0, NodeType::Corner),
        Node::new(NodeId(311), 300.0, 100.0, NodeType::Corner),
        Node::new(NodeId(312), 300.0, 300.0, NodeType::Corner),
        Node::new(NodeId(313), 100.0, 300.0, NodeType::Corner),
    ];
    let ref_open_stroke = vec![
        Node::new(NodeId(320), 0.0, 500.0, NodeType::Corner),
        Node::new(NodeId(321), 200.0, 500.0, NodeType::Corner),
        Node::new(NodeId(322), 400.0, 500.0, NodeType::Corner),
    ];
    state.set_companion_master_layers(vec![CompanionMasterLayer {
        id: 2,
        name: "ConcentricAndOpen".into(),
        color_rgba: [80, 160, 240, 180],
        contours: vec![ref_outer_ring, ref_inner_ring, ref_open_stroke],
        closed_flags: vec![true, true, false],
        anchors: vec![],
        editable: false,
    }]);

    let mut fg_open_reversed = GlyphContour::new(
        ContourId(0),
        vec![
            Node::new(NodeId(30), 400.0, 510.0, NodeType::Corner),
            Node::new(NodeId(31), 200.0, 510.0, NodeType::Corner),
            Node::new(NodeId(32), 0.0, 510.0, NodeType::Corner),
        ],
    );
    fg_open_reversed.closed = false;
    let fg_inner_first = GlyphContour::new(
        ContourId(1),
        vec![
            Node::new(NodeId(20), 105.0, 105.0, NodeType::Corner),
            Node::new(NodeId(21), 295.0, 105.0, NodeType::Corner),
            Node::new(NodeId(22), 295.0, 295.0, NodeType::Corner),
            Node::new(NodeId(23), 105.0, 295.0, NodeType::Corner),
        ],
    );
    let fg_outer_second = GlyphContour::new(
        ContourId(2),
        vec![
            Node::new(NodeId(10), 0.0, 0.0, NodeType::Corner),
            Node::new(NodeId(11), 0.0, 400.0, NodeType::Corner),
            Node::new(NodeId(12), 400.0, 400.0, NodeType::Corner),
            Node::new(NodeId(13), 400.0, 0.0, NodeType::Corner),
        ],
    );
    set_layer_contours(
        &mut state,
        vec![fg_open_reversed, fg_inner_first, fg_outer_second],
    );
    assert!(state.auto_match_master_compatibility(2));
    assert_relative_eq!(
        state.glyph.contours[0].nodes[2].point.x.0,
        400.0,
        epsilon = 1e-3
    );
    assert_relative_eq!(
        state.glyph.contours[1].nodes[0].point.x.0,
        105.0,
        epsilon = 1e-3
    );
    assert!(!state.glyph.contours[2].closed);
    assert_relative_eq!(
        state.glyph.contours[2].nodes[0].point.x.0,
        0.0,
        epsilon = 1e-3
    );

    // Identical bounding-box 6-node contours where only OnCurve/OffCurve topology disambiguates pairing.
    let ref_hex = vec![
        Node::new(NodeId(400), 0.0, 0.0, NodeType::Corner),
        Node::new(NodeId(401), 0.0, 200.0, NodeType::Corner),
        Node::new(NodeId(402), 0.0, 400.0, NodeType::Corner),
        Node::new(NodeId(403), 400.0, 400.0, NodeType::Corner),
        Node::new(NodeId(404), 400.0, 200.0, NodeType::Corner),
        Node::new(NodeId(405), 400.0, 0.0, NodeType::Corner),
    ];
    let ref_cubic_loop = vec![
        Node::new(NodeId(410), 0.0, 0.0, NodeType::Corner),
        Node::new(NodeId(411), 0.0, 400.0, NodeType::Corner),
        Node::new(NodeId(412), 150.0, 400.0, NodeType::OffCurve),
        Node::new(NodeId(413), 250.0, 400.0, NodeType::OffCurve),
        Node::new(NodeId(414), 400.0, 400.0, NodeType::Corner),
        Node::new(NodeId(415), 400.0, 0.0, NodeType::Corner),
    ];
    state.set_companion_master_layers(vec![CompanionMasterLayer {
        id: 3,
        name: "SameBBoxTopology".into(),
        color_rgba: [120, 200, 120, 180],
        contours: vec![ref_hex.clone(), ref_cubic_loop.clone()],
        closed_flags: vec![true, true],
        anchors: vec![],
        editable: false,
    }]);
    set_layer_contours(
        &mut state,
        vec![
            GlyphContour::new(ContourId(0), ref_cubic_loop),
            GlyphContour::new(ContourId(1), ref_hex),
        ],
    );
    assert!(state.auto_match_master_compatibility(3));
    assert_eq!(state.glyph.contours[0].nodes[2].node_type, NodeType::Corner);
    assert_eq!(
        state.glyph.contours[1].nodes[2].node_type,
        NodeType::OffCurve
    );
}

proptest! {
    #[test]
    fn prop_v0100_harmonize_equalizes_curvature(d_in in 20.0f64..160.0f64, d_out in 20.0f64..160.0f64) {
        let mut state = GrafiaCanvasState::init_glyph_a();
        set_layer_contours(&mut state, vec![make_two_joined_cubics_contour(d_in, d_out)]);
        state.selected_nodes = vec![NodeId(4)];
        state.harmonize_selected_nodes();
        let n = &state.glyph.contours[0].nodes;
        let k_in = cubic_signed_curvature(n[0].point, n[1].point, n[2].point, n[3].point, 1.0);
        let k_out = cubic_signed_curvature(n[3].point, n[4].point, n[5].point, n[6].point, 0.0);
        prop_assert!((k_in - k_out).abs() < 1e-4);
    }
}

// TC-V0100-012: Stateful pull-based `arbtest` fuzzing across G2, Harmonize, Balance, Servo & Neighbour Nudge.
#[test]
fn tc_v0100_012_arbtest_stateful_genius_harmonize_tunni_servo_power_nudge() {
    arbtest(|u| {
        let mut state = GrafiaCanvasState::init_glyph_a();
        set_layer_contours(&mut state, vec![make_two_joined_cubics_contour(90.0, 45.0)]);
        state.show_tunni_lines = true;
        for _ in 0..u.int_in_range(10..=20)? {
            match u.int_in_range(0..=6)? {
                0 => {
                    state.selected_nodes = vec![NodeId(4)];
                    state.harmonize_selected_nodes();
                }
                1 => state.harmonize_handles_keep_node(NodeId(4)),
                2 => state.set_node_genius(NodeId(4), u.arbitrary()?),
                3 => state.scale_segment_tunni_ratio(
                    ContourId(0),
                    0,
                    f64::from(u.int_in_range(10..=105)?) / 100.0,
                ),
                4 => state.balance_segment_tunni(ContourId(0), 0),
                5 => state.servo_translate_on_curve_node(
                    NodeId(1),
                    FontPoint::new(f64::from(u.int_in_range(-20..=20)?), 0.0),
                ),
                _ => {
                    if state.undo() {
                        state.validate_invariants().unwrap();
                        let _ = state.redo();
                    }
                }
            }
            state.validate_invariants().unwrap();
        }
        Ok(())
    })
    .budget_ms(250);
}

// TC-V0100-018: Live G0 / G1 / G2 / G3 continuity classifier & G3Clothoid solver/lock.
#[test]
fn tc_v0100_018_g0_to_g3_continuity_classifier_and_g3_clothoid_solver() {
    let mut state = GrafiaCanvasState::init_glyph_a();
    // 1. Sharp G0 join (non-collinear handles at NodeId(4)).
    let mut g0_c = make_two_joined_cubics_contour(100.0, 25.0);
    g0_c.nodes[4].point = FontPoint::new(30.0, 30.0);
    set_layer_contours(&mut state, vec![g0_c]);
    assert_eq!(
        state.classify_node_continuity(NodeId(4)),
        ContinuityGrade::G0Corner
    );

    // 2. Collinear G1 join with unequal curvature (d_in=100, d_out=25, a=b=40).
    set_layer_contours(
        &mut state,
        vec![make_two_joined_cubics_contour(100.0, 25.0)],
    );
    assert_eq!(
        state.classify_node_continuity(NodeId(4)),
        ContinuityGrade::G1Tangent
    );

    // 3. G2 curvature-continuous join after harmonize_selected_nodes.
    state.selected_nodes = vec![NodeId(4)];
    state.harmonize_selected_nodes();
    let grade_g2 = state.classify_node_continuity(NodeId(4));
    assert!(
        matches!(
            grade_g2,
            ContinuityGrade::G2Curvature | ContinuityGrade::G3Clothoid
        ),
        "expected at least G2Curvature, got {grade_g2:?}"
    );

    // 4. G3 clothoid harmonization & live G3 lock under handle drag.
    set_layer_contours(&mut state, vec![make_two_joined_cubics_contour(80.0, 80.0)]);
    state.selected_nodes = vec![NodeId(4)];
    state.harmonize_selected_nodes_g3();
    assert_eq!(
        state.classify_node_continuity(NodeId(4)),
        ContinuityGrade::G3Clothoid
    );

    state.set_node_g3_lock(NodeId(4), true);
    state.translate_node_upm(NodeId(3), FontPoint::new(-10.0, 0.0));
    assert_eq!(
        state.classify_node_continuity(NodeId(4)),
        ContinuityGrade::G3Clothoid
    );
}

// TC-V0100-019: Live 120 fps CurvatureComb overlay, inflection detection & dkappa/ds dip points.
#[test]
fn tc_v0100_019_live_curvature_comb_overlay_and_inflections() {
    let mut state = GrafiaCanvasState::init_glyph_a();
    // S-curve with an interior inflection at t = 0.5: P0=(0,0), P1=(50,120), P2=(150,-120), P3=(200,0).
    let mut s_curve = GlyphContour::new(
        ContourId(0),
        vec![
            Node::new(NodeId(1), 0.0, 0.0, NodeType::Corner),
            Node::new(NodeId(2), 50.0, 120.0, NodeType::OffCurve),
            Node::new(NodeId(3), 150.0, -120.0, NodeType::OffCurve),
            Node::new(NodeId(4), 200.0, 0.0, NodeType::Corner),
        ],
    );
    s_curve.closed = false;
    set_layer_contours(&mut state, vec![s_curve]);

    state.toggle_show_curvature_comb();
    let combs = state.compute_curvature_comb_overlay();
    assert_eq!(combs.len(), 1);
    assert!(combs[0].teeth.len() >= 32);
    assert!(
        !combs[0].inflections.is_empty(),
        "S-curve must report at least one interior inflection point"
    );
    assert_relative_eq!(combs[0].inflections[0].x.0, 100.0, epsilon = 5.0);
    assert_relative_eq!(combs[0].inflections[0].y.0, 0.0, epsilon = 5.0);

    // When a node on the segment is selected/active, emits 64 high-density teeth.
    state.selected_nodes = vec![NodeId(1)];
    let active_combs = state.compute_curvature_comb_overlay();
    assert_eq!(active_combs[0].teeth.len(), 64);

    // When segment 1 (not segment 0) of a 2-segment contour is hovered, segment 1 gets 64 teeth and segment 0 gets 32.
    set_layer_contours(&mut state, vec![make_two_joined_cubics_contour(80.0, 80.0)]);
    state.selected_nodes.clear();
    let segs = grafia::extract_segments(&state.contours[0], false);
    state.hovered_segment_hit = Some(grafia::SegmentHit {
        contour_idx: 0,
        segment: segs[1],
        t: 0.5,
        point: FontPoint::new(50.0, 40.0),
        distance_font: 0.0,
    });
    let two_combs = state.compute_curvature_comb_overlay();
    assert_eq!(two_combs.len(), 2);
    assert_eq!(two_combs[0].teeth.len(), 32);
    assert_eq!(two_combs[1].teeth.len(), 64);
}

// TC-V0100-020: Tunni dashed apex rays + handle-ratio HUD pills & Tablet 1-finger touch drag + sticky modifiers.
#[test]
fn tc_v0100_020_tunni_apex_rays_ratio_pills_and_tablet_touch_ergonomics() {
    let mut state = GrafiaCanvasState::init_glyph_a();
    state.viewport.zoom = 1.5;
    state.viewport.pan_offset = grafia::ScreenPoint::new(200.0, 350.0);
    set_layer_contours(&mut state, vec![make_tunni_arc_contour(60.0, 25.0)]);
    state.show_tunni_lines = false;
    let px_no_tunni = rasterize_canvas_subregion(&state, &GrafiaTheme::sonoma_default(), 640, 480);
    state.show_tunni_lines = true;
    let px_tunni = rasterize_canvas_subregion(&state, &GrafiaTheme::sonoma_default(), 640, 480);
    // Check that near the apex (0, 100) (40 UPM = 60 dp above P1=(0,60)), dashed projection rays paint pixels!
    let vp = state.active_slot_viewport();
    let apex_scr = vp.to_screen(FontPoint::new(0.0, 95.0));
    let (ax, ay) = (apex_scr.x.0.round() as i32, apex_scr.y.0.round() as i32);
    let mut apex_diff = false;
    for dy in -5..=5 {
        for dx in -5..=5 {
            let x = (ax + dx).clamp(0, 639) as usize;
            let y = (ay + dy).clamp(0, 479) as usize;
            let idx = (y * 640 + x) * 4;
            if px_no_tunni.data()[idx..idx + 4] != px_tunni.data()[idx..idx + 4] {
                apex_diff = true;
            }
        }
    }
    assert!(
        apex_diff,
        "show_tunni_lines must render dashed apex projection rays up to the tangent intersection apex"
    );

    // 1-finger Touch event sequence without PointerButton must select and drag NodeId(10) at (0, 0) -> (40, 0)
    let start_scr = vp.to_screen(FontPoint::new(0.0, 0.0));
    let end_scr = vp.to_screen(FontPoint::new(40.0, 0.0));
    let ctx = egui::Context::default();
    let raw = RawInput {
        screen_rect: Some(default_canvas_rect()),
        time: Some(4.0),
        events: vec![
            Event::Touch {
                device_id: egui::TouchDeviceId(0),
                id: egui::TouchId(1),
                phase: egui::TouchPhase::Start,
                pos: Pos2::new(start_scr.x.0, start_scr.y.0),
                force: None,
            },
            Event::Touch {
                device_id: egui::TouchDeviceId(0),
                id: egui::TouchId(1),
                phase: egui::TouchPhase::Move,
                pos: Pos2::new(end_scr.x.0, end_scr.y.0),
                force: None,
            },
            Event::Touch {
                device_id: egui::TouchDeviceId(0),
                id: egui::TouchId(1),
                phase: egui::TouchPhase::End,
                pos: Pos2::new(end_scr.x.0, end_scr.y.0),
                force: None,
            },
        ],
        ..Default::default()
    };
    let theme = state.theme.clone();
    let mut out = ctx.run_ui(raw, |ui| {
        let ev = ui.input(|i| i.events.clone());
        let _ = InputDispatcher::new(&mut state, &theme, default_canvas_rect())
            .dispatch_events(&ev, ui);
    });
    out.textures_delta.clear();
    let moved_n10 = state.find_node(NodeId(10)).expect("NodeId(10)").point;
    assert_relative_eq!(moved_n10.x.0, 40.0, epsilon = 1.5);

    // Sticky touch_space_pan_latched pans the viewport on a 1-finger touch drag
    state.drag_transients.touch_state.touch_space_pan_latched = true;
    let pan_before = state.viewport.pan_offset;
    let raw_pan = RawInput {
        screen_rect: Some(default_canvas_rect()),
        time: Some(5.0),
        events: vec![
            Event::Touch {
                device_id: egui::TouchDeviceId(0),
                id: egui::TouchId(2),
                phase: egui::TouchPhase::Start,
                pos: Pos2::new(300.0, 300.0),
                force: None,
            },
            Event::Touch {
                device_id: egui::TouchDeviceId(0),
                id: egui::TouchId(2),
                phase: egui::TouchPhase::Move,
                pos: Pos2::new(360.0, 340.0),
                force: None,
            },
            Event::Touch {
                device_id: egui::TouchDeviceId(0),
                id: egui::TouchId(2),
                phase: egui::TouchPhase::End,
                pos: Pos2::new(360.0, 340.0),
                force: None,
            },
        ],
        ..Default::default()
    };
    let mut out_pan = ctx.run_ui(raw_pan, |ui| {
        let ev = ui.input(|i| i.events.clone());
        let _ = InputDispatcher::new(&mut state, &theme, default_canvas_rect())
            .dispatch_events(&ev, ui);
    });
    out_pan.textures_delta.clear();
    assert_relative_eq!(
        state.viewport.pan_offset.x.0,
        pan_before.x.0 + 60.0,
        epsilon = 1e-2
    );
    assert_relative_eq!(
        state.viewport.pan_offset.y.0,
        pan_before.y.0 + 40.0,
        epsilon = 1e-2
    );
    state.drag_transients.touch_state.touch_space_pan_latched = false;

    // 2-finger touch gesture starting with Finger 1 on NodeId(10) at (40, 0) must cancel tentative 1-finger node drag when Finger 2 touches down
    let n10_scr = state
        .active_slot_viewport()
        .to_screen(FontPoint::new(40.0, 0.0));
    let raw_two_finger = RawInput {
        screen_rect: Some(default_canvas_rect()),
        time: Some(6.0),
        events: vec![
            Event::Touch {
                device_id: egui::TouchDeviceId(0),
                id: egui::TouchId(10),
                phase: egui::TouchPhase::Start,
                pos: Pos2::new(n10_scr.x.0, n10_scr.y.0),
                force: None,
            },
            Event::Touch {
                device_id: egui::TouchDeviceId(0),
                id: egui::TouchId(10),
                phase: egui::TouchPhase::Move,
                pos: Pos2::new(n10_scr.x.0 + 15.0, n10_scr.y.0),
                force: None,
            },
            Event::Touch {
                device_id: egui::TouchDeviceId(0),
                id: egui::TouchId(11),
                phase: egui::TouchPhase::Start,
                pos: Pos2::new(n10_scr.x.0 + 100.0, n10_scr.y.0),
                force: None,
            },
            Event::Touch {
                device_id: egui::TouchDeviceId(0),
                id: egui::TouchId(10),
                phase: egui::TouchPhase::Move,
                pos: Pos2::new(n10_scr.x.0 + 35.0, n10_scr.y.0),
                force: None,
            },
            Event::Touch {
                device_id: egui::TouchDeviceId(0),
                id: egui::TouchId(11),
                phase: egui::TouchPhase::Move,
                pos: Pos2::new(n10_scr.x.0 + 120.0, n10_scr.y.0),
                force: None,
            },
            Event::Touch {
                device_id: egui::TouchDeviceId(0),
                id: egui::TouchId(10),
                phase: egui::TouchPhase::End,
                pos: Pos2::new(n10_scr.x.0 + 35.0, n10_scr.y.0),
                force: None,
            },
            Event::Touch {
                device_id: egui::TouchDeviceId(0),
                id: egui::TouchId(11),
                phase: egui::TouchPhase::End,
                pos: Pos2::new(n10_scr.x.0 + 120.0, n10_scr.y.0),
                force: None,
            },
        ],
        ..Default::default()
    };
    let mut out_2f = ctx.run_ui(raw_two_finger, |ui| {
        let ev = ui.input(|i| i.events.clone());
        let _ = InputDispatcher::new(&mut state, &theme, default_canvas_rect())
            .dispatch_events(&ev, ui);
    });
    out_2f.textures_delta.clear();
    assert_eq!(state.select_state, grafia::SelectToolState::Idle);
    let restored_n10 = state.find_node(NodeId(10)).expect("NodeId(10)").point;
    assert_relative_eq!(restored_n10.x.0, 40.0, epsilon = 1.5);

    render_v0100_visual_proof_frames();
}

fn render_v0100_visual_proof_frames() {
    use grafia::adapters::headless::pixmap_to_rgba_image;
    let out_dir = std::path::Path::new(
        r"C:\Users\jhove\.gemini\antigravity-cli\brain\fccb9142-9217-4702-a2ca-95303b541ee0",
    );
    let _ = std::fs::create_dir_all(out_dir);
    let (w, h) = (1223_u32, 811_u32);

    // Frame 60: Multi-cubic contour with Tunni balance lines, dashed apex rays, Tunni midpoint rings, and 60%/75% HUD pills
    let mut st60 = GrafiaCanvasState::init_glyph_a();
    st60.viewport.zoom = 1.35;
    st60.viewport.pan_offset = grafia::ScreenPoint::new(340.0, 410.0);
    let mut c60 = GlyphContour::new(
        ContourId(0),
        vec![
            Node::new(NodeId(1), 0.0, 0.0, NodeType::Smooth),
            Node::new(NodeId(2), 0.0, 120.0, NodeType::OffCurve),
            Node::new(NodeId(3), 50.0, 200.0, NodeType::OffCurve),
            Node::new(NodeId(4), 200.0, 200.0, NodeType::Smooth),
            Node::new(NodeId(5), 320.0, 200.0, NodeType::OffCurve),
            Node::new(NodeId(6), 400.0, 120.0, NodeType::OffCurve),
            Node::new(NodeId(7), 400.0, 0.0, NodeType::Smooth),
            Node::new(NodeId(8), 400.0, -120.0, NodeType::OffCurve),
            Node::new(NodeId(9), 300.0, -200.0, NodeType::OffCurve),
            Node::new(NodeId(10), 200.0, -200.0, NodeType::Smooth),
            Node::new(NodeId(11), 80.0, -200.0, NodeType::OffCurve),
            Node::new(NodeId(12), 0.0, -120.0, NodeType::OffCurve),
        ],
    );
    c60.closed = true;
    set_layer_contours(&mut st60, vec![c60]);
    st60.show_tunni_lines = true;
    let px60 = rasterize_canvas_subregion(&st60, &GrafiaTheme::sonoma_default(), w, h);
    pixmap_to_rgba_image(px60)
        .save(out_dir.join("frame_60_v0100_tunni_lines_and_handle_ratio_hud.png"))
        .unwrap();

    // Frame 61: S-curve with 64-tooth Curvature Comb, outer envelope, inflection cross, |dkappa/ds| dip ring, and G2/G3 badges in Dark Mode
    let mut st61 = GrafiaCanvasState::init_glyph_a();
    st61.set_appearance_mode(AppearanceMode::Dark);
    st61.viewport.zoom = 1.5;
    st61.viewport.pan_offset = grafia::ScreenPoint::new(340.0, 440.0);
    let mut c61 = GlyphContour::new(
        ContourId(0),
        vec![
            Node::new(NodeId(1), 0.0, -80.0, NodeType::Corner),
            Node::new(NodeId(2), 40.0, 160.0, NodeType::OffCurve),
            Node::new(NodeId(3), 180.0, -160.0, NodeType::OffCurve),
            Node::new(NodeId(4), 240.0, 40.0, NodeType::Smooth),
            Node::new(NodeId(5), 300.0, 240.0, NodeType::OffCurve),
            Node::new(NodeId(6), 420.0, 120.0, NodeType::OffCurve),
            Node::new(NodeId(7), 480.0, 0.0, NodeType::Smooth),
        ],
    );
    c61.closed = false;
    set_layer_contours(&mut st61, vec![c61]);
    st61.set_show_curvature_comb(true);
    st61.set_node_genius(NodeId(4), true);
    st61.set_node_g3_lock(NodeId(7), true);
    st61.selected_nodes = vec![NodeId(1), NodeId(4)];
    let px61 = rasterize_canvas_subregion(&st61, &GrafiaTheme::sonoma_dark(), w, h);
    pixmap_to_rgba_image(px61)
        .save(out_dir.join("frame_61_v0100_curvature_comb_and_g2_g3_badges.png"))
        .unwrap();

    // Frame 62: Outline audit Red Arrows ("EXT", "STEM", "CROSS", "FLAT") + angled handle warning ("3.0°" pill)
    let mut st62 = GrafiaCanvasState::init_glyph_a();
    st62.viewport.zoom = 1.35;
    st62.viewport.pan_offset = grafia::ScreenPoint::new(240.0, 580.0);
    let mut c62_ext = GlyphContour::new(
        ContourId(0),
        vec![
            Node::new(NodeId(1), 0.0, 120.0, NodeType::Corner),
            Node::new(NodeId(2), 0.0, 340.0, NodeType::OffCurve),
            Node::new(NodeId(3), 160.0, 340.0, NodeType::OffCurve),
            Node::new(NodeId(4), 160.0, 140.0, NodeType::Smooth),
            Node::new(NodeId(5), 164.2, 60.0, NodeType::OffCurve),
            Node::new(NodeId(6), 120.0, 0.0, NodeType::OffCurve),
            Node::new(NodeId(7), 40.0, 0.0, NodeType::Corner),
        ],
    );
    c62_ext.closed = false;
    let c62_stem = GlyphContour::new(
        ContourId(1),
        vec![
            Node::new(NodeId(10), 260.0, 0.0, NodeType::Corner),
            Node::new(NodeId(11), 264.0, 280.0, NodeType::Corner),
            Node::new(NodeId(12), 340.0, 280.0, NodeType::Corner),
        ],
    );
    let c62_cross = GlyphContour::new(
        ContourId(2),
        vec![
            Node::new(NodeId(20), 400.0, 60.0, NodeType::Corner),
            Node::new(NodeId(21), 540.0, 240.0, NodeType::OffCurve),
            Node::new(NodeId(22), 400.0, 240.0, NodeType::OffCurve),
            Node::new(NodeId(23), 540.0, 60.0, NodeType::Corner),
        ],
    );
    let c62_flat = GlyphContour::new(
        ContourId(3),
        vec![
            Node::new(NodeId(30), 120.0, -80.0, NodeType::Corner),
            Node::new(NodeId(31), 220.0, -79.99, NodeType::OffCurve),
            Node::new(NodeId(32), 360.0, -80.01, NodeType::OffCurve),
            Node::new(NodeId(33), 480.0, -80.0, NodeType::Corner),
        ],
    );
    set_layer_contours(&mut st62, vec![c62_ext, c62_stem, c62_cross, c62_flat]);
    st62.curve_harmony.show_outline_audit = true;
    let px62 = rasterize_canvas_subregion(&st62, &GrafiaTheme::sonoma_default(), w, h);
    pixmap_to_rgba_image(px62)
        .save(out_dir.join("frame_62_v0100_red_arrows_outline_audit_and_angled_handles.png"))
        .unwrap();

    // Frame 63: MasterCompatibilityMode::FullNodeTrajectories with trajectory lines, crossing × markers, kink rings, and diagonal virtual apex ray on /A
    let mut st63 = GrafiaCanvasState::init_glyph_a();
    st63.viewport.zoom = 0.68;
    st63.viewport.pan_offset = grafia::ScreenPoint::new(420.0, 660.0);
    st63.show_master_compatibility = true;
    st63.set_master_compatibility_mode(MasterCompatibilityMode::FullNodeTrajectories);
    let mut leg_l = GlyphContour::new(
        ContourId(0),
        vec![
            Node::new(NodeId(1), 40.0, 0.0, NodeType::Corner),
            Node::new(NodeId(2), 160.0, 420.0, NodeType::Corner),
            Node::new(NodeId(3), 240.0, 420.0, NodeType::Corner),
            Node::new(NodeId(4), 130.0, 0.0, NodeType::Corner),
        ],
    );
    let mut leg_r = GlyphContour::new(
        ContourId(1),
        vec![
            Node::new(NodeId(10), 520.0, 0.0, NodeType::Corner),
            Node::new(NodeId(11), 400.0, 420.0, NodeType::Corner),
            Node::new(NodeId(12), 450.0, 260.0, NodeType::OffCurve),
            Node::new(NodeId(13), 490.0, 160.0, NodeType::Smooth),
            Node::new(NodeId(14), 530.0, 60.0, NodeType::OffCurve),
        ],
    );
    leg_l.closed = true;
    leg_r.closed = true;
    set_layer_contours(&mut st63, vec![leg_l, leg_r]);
    let comp_l = vec![
        Node::new(NodeId(1), 160.0, 450.0, NodeType::Corner),
        Node::new(NodeId(2), 20.0, -20.0, NodeType::Corner),
        Node::new(NodeId(3), 280.0, 450.0, NodeType::Corner),
        Node::new(NodeId(4), 160.0, -20.0, NodeType::Corner),
    ];
    let comp_r = vec![
        Node::new(NodeId(10), 560.0, -20.0, NodeType::Corner),
        Node::new(NodeId(11), 430.0, 450.0, NodeType::Corner),
        Node::new(NodeId(12), 470.0, 210.0, NodeType::OffCurve),
        Node::new(NodeId(13), 520.0, 160.0, NodeType::Smooth),
        Node::new(NodeId(14), 590.0, 80.0, NodeType::OffCurve),
    ];
    st63.set_companion_master_layers(vec![CompanionMasterLayer {
        id: 1,
        name: "Bold".into(),
        color_rgba: [220, 60, 80, 180],
        contours: vec![comp_l, comp_r],
        closed_flags: vec![true, true],
        anchors: vec![],
        editable: false,
    }]);
    st63.curve_harmony.active_tangent_snap = st63
        .compute_tangent_extension_snaps(FontPoint::new(280.0, 840.0), 60.0)
        .or(Some(grafia::TangentExtensionSnap {
            snap_point: FontPoint::new(280.0, 840.0),
            distance_upm: 0.0,
            kind: TangentExtensionSnapKind::VirtualApex {
                contour_idx_a: 0,
                seg_idx_a: 0,
                contour_idx_b: 1,
                seg_idx_b: 0,
            },
        }));
    let px63 = rasterize_canvas_subregion(&st63, &GrafiaTheme::sonoma_default(), w, h);
    pixmap_to_rgba_image(px63)
        .save(out_dir.join("frame_63_v0100_master_trajectories_kinks_and_virtual_apex.png"))
        .unwrap();
}
