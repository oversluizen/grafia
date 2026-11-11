#![allow(unexpected_cfgs)]
#![cfg(feature = "v080_red")]
//! Normative COTS Verification Suite for `SPEC-009` (`grafia v0.8.0`):
//! $G^2$ Smoothness Continuity, Romer/Henestrosa Harmonize (`Ctrl+Shift+H`),
//! Interactive Tangent Balance (Tunni) Lines (`[0.20, 0.95]`), Proportional Handle Servo,
//! Interpolated Neighbour Nudge (`C`), Master Trajectory Kink & Crossing Detector,
//! Proactive Vector Outline Audit, Off-Axis Handle Warning / Ratio HUD, and Diagonal Apex Snapping (`TC-V080-001` .. `TC-V080-016`).

use approx::assert_relative_eq;
use arbtest::arbtest;
use egui::{Event, Key, Modifiers as EguiModifiers, Pos2, RawInput, Rect, Vec2};
use grafia::adapters::headless::rasterize_canvas_subregion;
use grafia::adapters::input_dispatcher::InputDispatcher;
use grafia::{
    cubic_signed_curvature, AppearanceMode, CompanionMasterLayer, ContourId, FontPoint,
    GlyphContour, GrafiaCanvasState, GrafiaTheme, MasterCompatibilityMode, Node, NodeId, NodeType,
    OutlineAuditKind, TangentExtensionSnapKind, TUNNI_HIT_RADIUS_DP, TUNNI_MAX_RATIO,
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
    state.active_contour_closed = contours.last().map_or(true, |c| c.closed);
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

// TC-V080-001: Parametric cubic signed curvature kappa(t) evaluation.
#[test]
fn tc_v080_001_cubic_signed_curvature_quarter_circle_and_collinear() {
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

// TC-V080-002, TC-V080-003 & TC-V080-004: Ctrl+Shift+H Romer/Henestrosa G2 Harmonize, handle-only harmonize & live G2 node.
#[test]
fn tc_v080_002_to_004_g2_harmonize_anchor_handles_and_live_genius_node() {
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
}

// TC-V080-005, TC-V080-006 & TC-V080-007: Tangent Balance (Tunni) overlay, hit-test, ratio clamping [0.20, 0.95] & balance.
#[rstest]
#[case::clamp_low(0.10, 0.20)]
#[case::mid_55(0.55, 0.55)]
#[case::mid_75(0.75, 0.75)]
#[case::clamp_high(0.99, 0.95)]
fn tc_v080_005_to_007_tunni_lines_hit_test_clamp_and_balance(
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
    }
}

// TC-V080-008, TC-V080-009, TC-V080-010 & TC-V080-011: Servo scaling, Neighbour Nudge (C), Undo/Redo & rendering.
#[test]
fn tc_v080_008_to_011_servo_scaling_power_nudge_undo_and_rendering() {
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

    set_layer_contours(&mut state, vec![make_two_joined_cubics_contour(80.0, 40.0)]);
    state.set_node_genius(NodeId(4), true);
    state.show_tunni_lines = true;
    state.set_appearance_mode(AppearanceMode::Light);
    let light_px = rasterize_canvas_subregion(&state, &GrafiaTheme::sonoma_default(), 320, 180);
    state.set_appearance_mode(AppearanceMode::Dark);
    let dark_px = rasterize_canvas_subregion(&state, &GrafiaTheme::sonoma_dark(), 320, 180);
    assert_ne!(light_px.data(), dark_px.data());
}

// TC-V080-013: MasterCompatibilityMode::FullNodeTrajectories, trajectory crossing & smooth-node kink detector.
#[test]
fn tc_v080_013_master_full_trajectories_crossings_and_smooth_kink_detector() {
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

// TC-V080-014: Proactive Vector Outline Audit (MissingExtremum, QuasiOrthogonalStem, CrossedHandles, CollinearCubic).
#[test]
fn tc_v080_014_proactive_vector_outline_audit_four_defects() {
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
        vec![missing_ext, quasi_stem, crossed, collinear],
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
}

// TC-V080-015 & TC-V080-016: Off-axis handle warning, handle-ratio HUD, and diagonal tangent extension / virtual apex snap.
#[test]
fn tc_v080_015_and_016_angled_handle_hud_and_diagonal_virtual_apex_snap() {
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

    // TC-V080-016: Truncated /A diagonal legs converging toward virtual apex (200, 700).
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
}

// TC-V080-017: Automatic cross-master contour order, winding & cyclic Node 0 matching (`auto_match_master_compatibility`).
#[test]
fn tc_v080_017_auto_match_master_compatibility_order_winding_and_cyclic_shift() {
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
}

proptest! {
    #[test]
    fn prop_v080_harmonize_equalizes_curvature(d_in in 20.0f64..160.0f64, d_out in 20.0f64..160.0f64) {
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

// TC-V080-012: Stateful pull-based `arbtest` fuzzing across G2, Harmonize, Balance, Servo & Neighbour Nudge.
#[test]
fn tc_v080_012_arbtest_stateful_genius_harmonize_tunni_servo_power_nudge() {
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
