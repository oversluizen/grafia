#![allow(unexpected_cfgs)]
#![cfg(feature = "v070_red")]
//! Normative COTS Verification Suite for `SPEC-008` (`grafia v0.7.0`):
//! Background Layer (`Cmd+B`, `Cmd+J`, `Cmd+K`), 2D `CanvasAnchor` & Mark Cloud (`ghost_paths`),
//! 2D Pre-Resolved `CanvasComponent` & Decompose (`Cmd+Shift+D`), Fit Curve (`Ctrl+Opt+1..8`),
//! Live Contour Stroke & Expand Outline, and `CompanionMasterLayer` / Master Compatibility (`TC-V070-001` .. `TC-V070-017`).

use approx::assert_relative_eq;
use arbtest::arbtest;
use egui::{Event, Key, Modifiers as EguiModifiers, Pos2, RawInput, Rect, Vec2};
use grafia::adapters::headless::rasterize_canvas_subregion;
use grafia::adapters::input_dispatcher::InputDispatcher;
use grafia::{
    contour_signed_area, Affine2D, AnchorId, AppearanceMode, CanvasAnchor, CanvasComponent, CanvasHostRequest,
    CompanionMasterLayer, ComponentId, ContourId, ContourStrokeStyle, FitCurveConfig, FontPoint, GhostMarkPath,
    GlifoCanvasState, GlifoTheme, GlyphContour, Guide, GuideId, LineCapStyle, LineJoinStyle, Node, NodeId,
    NodeType, StrokePosition, ANCHOR_DIAMOND_RADIUS_DP, ANCHOR_DIAMOND_SELECTED_RADIUS_DP,
    BACKGROUND_STROKE_WIDTH_DP, FIT_CURVE_DEFAULT_MAX_RATIO, FIT_CURVE_DEFAULT_MIN_RATIO,
};
use proptest::prelude::*;
use rstest::rstest;

fn default_canvas_rect() -> Rect { Rect::from_min_size(Pos2::ZERO, Vec2::new(960.0, 600.0)) }

fn make_rect_contour(id: usize, base_node: u64, x0: f64, y0: f64, x1: f64, y1: f64) -> GlyphContour {
    GlyphContour::new(ContourId(id), vec![
        Node::new(NodeId(base_node), x0, y0, NodeType::Corner), Node::new(NodeId(base_node + 1), x1, y0, NodeType::Corner),
        Node::new(NodeId(base_node + 2), x1, y1, NodeType::Corner), Node::new(NodeId(base_node + 3), x0, y1, NodeType::Corner),
    ])
}

fn make_quarter_arc_contour(id: usize, h1_y: f64, h2_x: f64) -> GlyphContour {
    let mut gc = GlyphContour::new(ContourId(id), vec![
        Node::new(NodeId(100), 0.0, 0.0, NodeType::Corner), Node::new(NodeId(101), 0.0, h1_y, NodeType::OffCurve),
        Node::new(NodeId(102), h2_x, 100.0, NodeType::OffCurve), Node::new(NodeId(103), 100.0, 100.0, NodeType::Smooth),
    ]);
    gc.closed = false;
    gc
}

fn set_layer_contours(state: &mut GlifoCanvasState, contours: Vec<GlyphContour>) {
    state.contours = contours.iter().map(|c| c.nodes.clone()).collect();
    state.active_contour_closed = contours.last().map_or(true, |c| c.closed);
    state.glyph.contours = contours;
    state.sync_paths_from_contours();
}

#[allow(clippy::too_many_arguments)]
fn dispatch_key(state: &mut GlifoCanvasState, key: Key, shift: bool, alt: bool, cmd: bool, ctrl: bool) {
    let ctx = egui::Context::default();
    let m = EguiModifiers { alt, ctrl, shift, mac_cmd: cmd, command: cmd };
    let events = vec![Event::Key { key, physical_key: Some(key), pressed: true, repeat: false, modifiers: m }];
    let raw = RawInput { screen_rect: Some(default_canvas_rect()), time: Some(1.0), events, ..Default::default() };
    ctx.input_mut(|i| i.modifiers = m);
    let theme = state.theme.clone();
    let mut out = ctx.run_ui(raw, |ui| {
        let ev = ui.input(|i| i.events.clone());
        let _ = InputDispatcher::new(state, &theme, default_canvas_rect()).dispatch_events(&ev, ui);
    });
    out.textures_delta.clear();
}

// TC-V070-001 & TC-V070-002: Cmd+J (Selection to Background, replace vs. append & empty selection).
#[test]
fn tc_v070_001_and_002_selection_to_background_partial_append_and_all() {
    assert_relative_eq!(BACKGROUND_STROKE_WIDTH_DP, 1.0, epsilon = 1e-4);
    let mut state = GlifoCanvasState::init_glyph_a();
    set_layer_contours(&mut state, vec![make_rect_contour(1, 10, 0.0, 0.0, 100.0, 200.0), make_rect_contour(2, 20, 200.0, 0.0, 300.0, 200.0)]);
    state.selected_nodes = vec![NodeId(10)];
    dispatch_key(&mut state, Key::J, false, false, true, false);
    assert_eq!(state.background_contours.len(), 1);
    assert_ne!(state.background_contours[0].id, state.glyph.contours[0].id);

    state.selected_nodes = vec![NodeId(20)];
    dispatch_key(&mut state, Key::J, false, true, true, false);
    assert_eq!(state.background_contours.len(), 2);

    state.background_contours.clear();
    state.selected_nodes.clear();
    let snap_before_all = state.snapshot();
    dispatch_key(&mut state, Key::J, false, false, true, false);
    assert_eq!(state.background_contours.len(), 2);
    let snap_after_all = state.snapshot();
    assert!(state.undo() && state.snapshot() == snap_before_all);
    assert!(state.redo() && state.snapshot() == snap_after_all);
    state.validate_invariants().unwrap();
}

// TC-V070-003 & TC-V070-004: Cmd+K (Swap with Background) and Cmd+B / Cmd+Opt+B (Background Editing & Visibility).
#[test]
fn tc_v070_003_and_004_swap_and_toggle_background_editing() {
    let mut state = GlifoCanvasState::init_glyph_a();
    set_layer_contours(&mut state, vec![make_rect_contour(1, 10, 0.0, 0.0, 100.0, 100.0)]);
    state.background_contours = vec![make_rect_contour(2, 20, 50.0, 50.0, 150.0, 150.0), make_rect_contour(3, 30, 200.0, 50.0, 300.0, 150.0)];
    state.selected_nodes = vec![NodeId(10)];
    let snap_0 = state.snapshot();

    dispatch_key(&mut state, Key::K, false, false, true, false);
    assert_eq!(state.glyph.contours.len(), 2);
    assert_eq!(state.background_contours.len(), 1);
    assert!(state.selected_nodes.is_empty());
    let snap_1 = state.snapshot();
    assert!(state.undo() && state.snapshot() == snap_0);
    assert!(state.redo() && state.snapshot() == snap_1);

    dispatch_key(&mut state, Key::B, false, true, true, false);
    assert!(!state.show_background && !state.editing_background);
    dispatch_key(&mut state, Key::B, false, true, true, false);
    assert!(state.show_background);

    dispatch_key(&mut state, Key::B, false, false, true, false);
    assert!(state.editing_background && state.glyph.contours.len() == 1);
    state.translate_node_upm(NodeId(10), FontPoint::new(25.0, 0.0));
    dispatch_key(&mut state, Key::B, false, false, true, false);
    assert!(!state.editing_background);
    assert_relative_eq!(state.background_contours[0].nodes[0].point.x.0, 25.0, epsilon = 1e-4);
    state.validate_invariants().unwrap();
}

// TC-V070-005, TC-V070-006 & TC-V070-007: CanvasAnchor hit-test, Tab cycling, snapped drag, Delete & Mark Cloud.
#[test]
fn tc_v070_005_to_007_anchors_tab_cycling_snap_delete_and_mark_cloud() {
    assert_relative_eq!(ANCHOR_DIAMOND_RADIUS_DP, 4.5, epsilon = 1e-4);
    assert_relative_eq!(ANCHOR_DIAMOND_SELECTED_RADIUS_DP, 6.0, epsilon = 1e-4);
    let mut state = GlifoCanvasState::init_glyph_a();
    state.set_anchors(vec![
        CanvasAnchor { id: AnchorId(1), name: "top".into(), position: FontPoint::new(297.0, 720.0), selected: false },
        CanvasAnchor { id: AnchorId(2), name: "bottom".into(), position: FontPoint::new(300.0, 0.0), selected: false },
        CanvasAnchor { id: AnchorId(3), name: "ogonek".into(), position: FontPoint::new(420.0, 0.0), selected: false },
    ]);
    state.set_ghost_mark_paths(vec![GhostMarkPath { anchor_name: "top".into(), contours: vec![make_rect_contour(90, 900, -20.0, 0.0, 20.0, 60.0)] }]);
    state.show_mark_cloud = true;

    assert!(state.select_anchor_at_upm(FontPoint::new(298.0, 720.0), false));
    assert!(state.anchors[0].selected);
    let cloud = state.compute_active_mark_cloud_contours();
    assert_eq!(cloud.len(), 1);
    assert_relative_eq!(cloud[0].nodes[0].point.x.0, 277.0, epsilon = 1e-4);
    assert_relative_eq!(cloud[0].nodes[0].point.y.0, 720.0, epsilon = 1e-4);

    dispatch_key(&mut state, Key::Tab, false, false, false, false);
    assert!(state.anchors[1].selected && !state.anchors[0].selected);
    dispatch_key(&mut state, Key::Tab, false, false, false, false);
    assert!(state.anchors[2].selected && state.compute_active_mark_cloud_contours().is_empty());
    dispatch_key(&mut state, Key::Tab, true, false, false, false);
    assert!(state.anchors[1].selected);
    dispatch_key(&mut state, Key::Tab, true, false, false, false);
    assert!(state.anchors[0].selected);

    state.global_guides.push(Guide::new(GuideId(1), FontPoint::new(300.0, 500.0), 90.0, true));
    state.drag_selected_anchors_to_upm(FontPoint::new(299.0, 720.0), false);
    assert_relative_eq!(state.anchors[0].position.x.0, 300.0, epsilon = 1e-4);

    dispatch_key(&mut state, Key::Delete, false, false, false, false);
    assert_eq!(state.anchors.len(), 2);
    assert!(state.undo() && state.anchors.len() == 3);
}

// TC-V070-008, TC-V070-009 & TC-V070-010: CanvasComponent selection, locks, double-click & Cmd+Shift+D Decompose.
#[test]
fn tc_v070_008_to_010_components_locks_double_click_and_decompose() {
    let mut state = GlifoCanvasState::init_glyph_a();
    set_layer_contours(&mut state, Vec::new());
    let base_rect = vec![make_rect_contour(1, 1, 0.0, 0.0, 100.0, 200.0)];
    state.set_components(vec![
        CanvasComponent { id: ComponentId(1), component_name: "acutecomb".into(), contours: base_rect.clone(), transform: Affine2D::translation(50.0, 500.0), locked: false, automatic_alignment: false, selected: false },
        CanvasComponent { id: ComponentId(2), component_name: "gravecomb".into(), contours: base_rect.clone(), transform: Affine2D { a: -1.0, b: 0.0, c: 0.0, d: 1.0, tx: 300.0, ty: 500.0 }, locked: false, automatic_alignment: true, selected: false },
        CanvasComponent { id: ComponentId(3), component_name: "lockedpart".into(), contours: base_rect, transform: Affine2D::translation(400.0, 0.0), locked: true, automatic_alignment: false, selected: false },
    ]);

    assert!(!state.select_component_at_upm(FontPoint::new(450.0, 100.0), false));
    assert!(state.select_component_at_upm(FontPoint::new(100.0, 600.0), false));
    state.translate_selected_components(FontPoint::new(10.0, 20.0));
    assert_relative_eq!(state.components[0].transform.tx, 60.0, epsilon = 1e-4);

    assert!(state.select_component_at_upm(FontPoint::new(250.0, 600.0), true));
    state.translate_selected_components(FontPoint::new(10.0, 0.0));
    assert_relative_eq!(state.components[1].transform.tx, 300.0, epsilon = 1e-4);

    state.double_click_component_at_upm(FontPoint::new(110.0, 620.0));
    assert_eq!(state.drain_host_requests(), vec![CanvasHostRequest::OpenComponentSource { id: ComponentId(1), component_name: "acutecomb".into() }]);

    let area_before = contour_signed_area(&state.components[0].contours[0].nodes).signum();
    dispatch_key(&mut state, Key::D, true, false, true, false);
    assert_eq!(state.components.len(), 1);
    assert_eq!(state.glyph.contours.len(), 2);
    assert_relative_eq!(area_before, contour_signed_area(&state.glyph.contours[1].nodes).signum(), epsilon = 1e-4);
    assert!(state.undo() && state.components.len() == 3);
}

// TC-V070-011 & TC-V070-012: Fit Curve 8-step ladder (57%..92%), guards, shorter/longer & handle normalization.
#[rstest]
#[case::step_1_57_pct(0, Key::Num1, 0.57)]
#[case::step_2_62_pct(1, Key::Num2, 0.62)]
#[case::step_3_67_pct(2, Key::Num3, 0.67)]
#[case::step_4_72_pct(3, Key::Num4, 0.72)]
#[case::step_5_77_pct(4, Key::Num5, 0.77)]
#[case::step_6_82_pct(5, Key::Num6, 0.82)]
#[case::step_7_87_pct(6, Key::Num7, 0.87)]
#[case::step_8_92_pct(7, Key::Num8, 0.92)]
fn tc_v070_011_and_012_fit_curve_ladder_and_handle_normalization(#[case] col: usize, #[case] key: Key, #[case] expected_ratio: f64) {
    assert_relative_eq!(FIT_CURVE_DEFAULT_MIN_RATIO, 0.57, epsilon = 1e-4);
    assert_relative_eq!(FIT_CURVE_DEFAULT_MAX_RATIO, 0.92, epsilon = 1e-4);
    assert_relative_eq!(FitCurveConfig::default().ratio_for_column(col), expected_ratio, epsilon = 1e-4);

    let mut state = GlifoCanvasState::init_glyph_a();
    set_layer_contours(&mut state, vec![make_quarter_arc_contour(1, 50.0, 50.0)]);
    state.selected_nodes = vec![NodeId(100)];

    dispatch_key(&mut state, key, false, true, false, true);
    assert_relative_eq!(state.glyph.contours[0].nodes[1].point.y.0, 100.0 * expected_ratio, epsilon = 1e-3);
    assert_relative_eq!(state.glyph.contours[0].nodes[2].point.x.0, 100.0 * (1.0 - expected_ratio), epsilon = 1e-3);

    if col == 0 {
        state.fit_curve_step_delta(true);
        assert_relative_eq!(state.glyph.contours[0].nodes[1].point.y.0, 61.375, epsilon = 1e-2);
        state.fit_curve_step_delta(false);
        assert_relative_eq!(state.glyph.contours[0].nodes[1].point.y.0, 57.0, epsilon = 1e-2);

        set_layer_contours(&mut state, vec![make_quarter_arc_contour(1, 60.0, 20.0)]);
        state.selected_nodes = vec![NodeId(100)];
        state.normalize_selected_cubic_handles();
        assert_relative_eq!(state.glyph.contours[0].nodes[1].point.y.0, 70.0, epsilon = 1e-3);
        assert_relative_eq!(state.glyph.contours[0].nodes[2].point.x.0, 30.0, epsilon = 1e-3);

        let mut par = GlyphContour::new(ContourId(1), vec![
            Node::new(NodeId(100), 0.0, 0.0, NodeType::Corner), Node::new(NodeId(101), 0.0, 50.0, NodeType::OffCurve),
            Node::new(NodeId(102), 100.0, 150.0, NodeType::OffCurve), Node::new(NodeId(103), 100.0, 100.0, NodeType::Smooth),
        ]);
        par.closed = false;
        set_layer_contours(&mut state, vec![par]);
        state.selected_nodes = vec![NodeId(100)];
        state.apply_fit_curve_column(3);
        assert_relative_eq!(state.glyph.contours[0].nodes[1].point.y.0, 50.0, epsilon = 1e-4);
    }
}

// TC-V070-013, TC-V070-014 & TC-V070-015: Live Contour Stroke offset, Expand Outline & Light/Dark rendering.
#[test]
fn tc_v070_013_to_015_live_stroke_offset_expand_outline_and_rendering() {
    let mut state = GlifoCanvasState::init_glyph_a();
    let mut open_line = GlyphContour::new(ContourId(0), vec![
        Node::new(NodeId(1), 100.0, 200.0, NodeType::Corner), Node::new(NodeId(2), 400.0, 200.0, NodeType::Corner),
    ]);
    open_line.closed = false;
    set_layer_contours(&mut state, vec![open_line, make_rect_contour(1, 10, 100.0, 300.0, 300.0, 500.0)]);
    state.set_contour_stroke(ContourId(0), ContourStrokeStyle { width: 40.0, height: 40.0, position: StrokePosition::Center, cap_start: LineCapStyle::Square, cap_end: LineCapStyle::Butt, join: LineJoinStyle::Miter });
    state.set_contour_stroke(ContourId(1), ContourStrokeStyle { width: 20.0, height: 20.0, position: StrokePosition::Center, cap_start: LineCapStyle::Butt, cap_end: LineCapStyle::Butt, join: LineJoinStyle::Round });

    let preview = state.compute_expanded_stroke_contours();
    assert_eq!(preview.len(), 3);
    assert!(preview.iter().all(|c| c.closed));
    let min_x = preview[0].nodes.iter().map(|n| n.point.x.0).fold(f64::INFINITY, f64::min);
    let max_x = preview[0].nodes.iter().map(|n| n.point.x.0).fold(f64::NEG_INFINITY, f64::max);
    assert_relative_eq!(min_x, 80.0, epsilon = 1e-2);
    assert_relative_eq!(max_x, 400.0, epsilon = 1e-2);
    assert_relative_eq!(contour_signed_area(&preview[1].nodes).signum(), -contour_signed_area(&preview[2].nodes).signum(), epsilon = 1e-4);

    state.selected_nodes.clear();
    let snap_0 = state.snapshot();
    state.expand_selected_strokes();
    assert_eq!(state.glyph.contours.len(), 3);
    assert!(state.contour_strokes.is_empty());
    let snap_1 = state.snapshot();
    assert!(state.undo() && state.snapshot() == snap_0 && state.contour_strokes.len() == 2);
    assert!(state.redo() && state.snapshot() == snap_1 && state.undo());

    state.background_contours = vec![make_rect_contour(9, 90, 50.0, 50.0, 250.0, 250.0)];
    state.set_anchors(vec![CanvasAnchor { id: AnchorId(1), name: "top".into(), position: FontPoint::new(200.0, 600.0), selected: true }]);
    state.set_ghost_mark_paths(vec![GhostMarkPath { anchor_name: "top".into(), contours: vec![make_rect_contour(8, 80, -15.0, 0.0, 15.0, 40.0)] }]);
    state.show_mark_cloud = true;
    state.set_components(vec![CanvasComponent { id: ComponentId(1), component_name: "part".into(), contours: vec![make_rect_contour(7, 70, 0.0, 0.0, 60.0, 120.0)], transform: Affine2D::translation(320.0, 100.0), locked: false, automatic_alignment: false, selected: false }]);
    state.set_appearance_mode(AppearanceMode::Light);
    let light_px = rasterize_canvas_subregion(&state, &GlifoTheme::sonoma_default(), 320, 180);
    state.set_appearance_mode(AppearanceMode::Dark);
    let dark_px = rasterize_canvas_subregion(&state, &GlifoTheme::sonoma_dark(), 320, 180);
    assert_ne!(light_px.data(), dark_px.data());
}

proptest! {
    #[test]
    fn prop_v070_fit_curve_ratio_bounds(min_r in 0.20f64..0.75f64, span in 0.15f64..0.45f64, col in 0usize..8usize) {
        let cfg = FitCurveConfig { min_ratio: min_r, max_ratio: min_r + span };
        let r = cfg.ratio_for_column(col);
        prop_assert!(r >= min_r - 1e-6 && r <= min_r + span + 1e-6);
    }
}

// TC-V070-016: Stateful pull-based `arbtest` multi-tool action chains.
#[test]
fn tc_v070_016_arbtest_stateful_background_anchors_components_fit_and_stroke() {
    arbtest(|u| {
        let mut state = GlifoCanvasState::init_glyph_a();
        set_layer_contours(&mut state, vec![make_quarter_arc_contour(0, 55.0, 45.0)]);
        state.set_anchors(vec![CanvasAnchor { id: AnchorId(1), name: "top".into(), position: FontPoint::new(250.0, 700.0), selected: true }]);
        state.set_components(vec![CanvasComponent { id: ComponentId(1), component_name: "stem".into(), contours: vec![make_rect_contour(10, 50, 0.0, 0.0, 80.0, 500.0)], transform: Affine2D::translation(100.0, 0.0), locked: false, automatic_alignment: false, selected: true }]);
        for _ in 0..u.int_in_range(10..=22)? {
            match u.int_in_range(0..=8)? {
                0 => dispatch_key(&mut state, Key::J, false, u.arbitrary()?, true, false),
                1 => dispatch_key(&mut state, Key::K, false, false, true, false),
                2 => dispatch_key(&mut state, Key::B, false, false, true, false),
                3 => state.drag_selected_anchors_to_upm(FontPoint::new(250.0 + f64::from(u.int_in_range(-40..=40)?), 700.0), true),
                4 => dispatch_key(&mut state, Key::Delete, false, false, false, false),
                5 => dispatch_key(&mut state, Key::D, true, false, true, false),
                6 => { state.selected_nodes = vec![NodeId(100)]; state.apply_fit_curve_column(u.int_in_range(0..=7)?); }
                7 => if let Some(c) = state.glyph.contours.first() {
                    state.set_contour_stroke(c.id, ContourStrokeStyle { width: 16.0, height: 16.0, position: StrokePosition::Center, cap_start: LineCapStyle::Butt, cap_end: LineCapStyle::Butt, join: LineJoinStyle::Miter });
                    state.expand_selected_strokes();
                },
                _ => if state.undo() { state.validate_invariants().unwrap(); let _ = state.redo(); },
            }
            state.validate_invariants().unwrap();
        }
        Ok(())
    }).budget_ms(250);
}

// TC-V070-017: CompanionMasterLayer, Master Compatibility (Ctrl+Opt+Cmd+N) & SelectAllLayers multi-layer node editing.
#[test]
fn tc_v070_017_companion_master_layer_compatibility_and_select_all_layers() {
    let mut state = GlifoCanvasState::init_glyph_a();
    set_layer_contours(&mut state, vec![make_rect_contour(0, 10, 100.0, 0.0, 200.0, 700.0)]);
    state.set_anchors(vec![CanvasAnchor { id: AnchorId(1), name: "top".into(), position: FontPoint::new(150.0, 720.0), selected: false }]);

    let bold_nodes = make_rect_contour(0, 50, 80.0, 0.0, 240.0, 700.0).nodes;
    let cond_nodes = make_rect_contour(0, 80, 110.0, 0.0, 180.0, 700.0).nodes;
    let top_b = CanvasAnchor { id: AnchorId(10), name: "top".into(), position: FontPoint::new(160.0, 730.0), selected: false };
    let top_c = CanvasAnchor { id: AnchorId(20), name: "top".into(), position: FontPoint::new(145.0, 720.0), selected: false };
    state.set_companion_master_layers(vec![
        CompanionMasterLayer { id: 1, name: "Bold".into(), color_rgba: [220, 60, 80, 180], contours: vec![bold_nodes], closed_flags: vec![true], anchors: vec![top_b], editable: true },
        CompanionMasterLayer { id: 2, name: "Condensed".into(), color_rgba: [60, 140, 220, 180], contours: vec![cond_nodes], closed_flags: vec![true], anchors: vec![top_c], editable: false },
    ]);

    dispatch_key(&mut state, Key::N, false, true, true, true);
    assert!(state.show_master_compatibility);
    let rep_ok = state.compute_master_compatibility();
    assert!(rep_ok.is_compatible && rep_ok.incompatible_contour_indices.is_empty());
    assert_eq!(rep_ok.start_node_vectors.len(), 2);
    assert_eq!(rep_ok.anchor_vectors.len(), 2);

    state.companion_master_layers[1].contours[0][1].node_type = NodeType::OffCurve;
    let rep_bad = state.compute_master_compatibility();
    assert!(!rep_bad.is_compatible && rep_bad.incompatible_contour_indices == vec![0]);
    state.companion_master_layers[1].contours[0][1].node_type = NodeType::Corner;

    state.select_all_layers_mode = true;
    let snap_before = state.snapshot();
    state.select_nodes_in_rect_all_layers(FontPoint::new(70.0, -10.0), FontPoint::new(120.0, 20.0));
    state.translate_selected_across_all_layers(FontPoint::new(15.0, 5.0));
    assert_relative_eq!(state.glyph.contours[0].nodes[0].point.x.0, 115.0, epsilon = 1e-4);
    assert_relative_eq!(state.companion_master_layers[0].contours[0][0].point.x.0, 95.0, epsilon = 1e-4);
    assert_relative_eq!(state.companion_master_layers[1].contours[0][0].point.x.0, 110.0, epsilon = 1e-4);
    assert!(state.undo() && state.snapshot() == snap_before);
}
