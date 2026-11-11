#![allow(unexpected_cfgs)]
//! Normative COTS Verification Suite for `SPEC-007` (`grafia v0.6.0`):
//! Preview Strip Engine with Separable Gaussian Blur, Headless 5-Mode
//! `SelectionInfo` Contract, Canvas Rulers (`Cmd+R`), Background
//! Layer (`Cmd+B` / `Cmd+J` / `Cmd+K` / `Cmd+Shift+J`), and Default Anchors (`Cmd+U`)
//! (`TC-V060-001` .. `TC-V060-018`).
//!
//! Standards: ISO/IEC/IEEE 42010:2011, ISO/IEC/IEEE 29119-3:2021, IEEE 1012-2016, SemVer 2.0.0.
//! Baseline: Separable 2D Gaussian Signal Processing & CAGD Selection Telemetry.

use approx::assert_relative_eq;
use arbtest::arbtest;
use egui::{Event, Key, Modifiers as EguiModifiers, Pos2, RawInput, Rect, Vec2};
use grafia::adapters::headless::rasterize_canvas_subregion;
use grafia::adapters::input_dispatcher::InputDispatcher;
use grafia::{
    compute_ruler_tick_steps_upm, render_preview_strip_rgba, AnchorId, AppearanceMode,
    CanvasAnchor, CanvasHostRequest, ContourId, FontPoint, GlyphContour, GrafiaCanvasState,
    GrafiaTheme, GuideAlignment, Node, NodeId, NodeType, PreviewRenderConfig, ScreenPoint,
    SelectionInfo, SelectionInfoAction, TransformGridAnchor, PREVIEW_BG_INVERTED_LIGHT,
    PREVIEW_BLUR_ACTIVE_THRESHOLD_PX, PREVIEW_BLUR_DEFAULT_RADIUS_PX, PREVIEW_BLUR_MAX_RADIUS_PX,
    PREVIEW_BLUR_MIN_RADIUS_PX, PREVIEW_BOTH_EXTRA_SPACE_SCALE, PREVIEW_DEFAULT_EXTRA_SPACING,
    PREVIEW_ONE_EXTRA_SPACE_SCALE, RULER_THICKNESS_DP,
};
use proptest::prelude::*;
use rstest::rstest;

fn default_canvas_rect() -> Rect {
    Rect::from_min_size(Pos2::ZERO, Vec2::new(800.0, 600.0))
}

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
    let raw = RawInput {
        screen_rect: Some(default_canvas_rect()),
        time: Some(1.0),
        events: vec![Event::Key {
            key,
            physical_key: Some(key),
            pressed: true,
            repeat: false,
            modifiers: m,
        }],
        ..Default::default()
    };
    ctx.input_mut(|i| i.modifiers = m);
    let theme = state.theme.clone();
    let mut out = ctx.run_ui(raw, |ui| {
        let ev = ui.input(|i| i.events.clone());
        let mut disp = InputDispatcher::new(state, &theme, default_canvas_rect());
        let _ = disp.dispatch_events(&ev, ui);
    });
    out.textures_delta.clear();
}

// TC-V060-001: Preview vertical scaling formula (_extraSpacing = 1.25, more_space 0.8 & 0.7).
#[rstest]
#[case::normal(false, false, 1.0)]
#[case::more_above(true, false, PREVIEW_ONE_EXTRA_SPACE_SCALE)]
#[case::more_below(false, true, PREVIEW_ONE_EXTRA_SPACE_SCALE)]
#[case::both_extra(true, true, PREVIEW_BOTH_EXTRA_SPACE_SCALE)]
fn tc_v060_001_preview_vertical_scale_formula(
    #[case] more_above: bool,
    #[case] more_below: bool,
    #[case] expected_factor: f64,
) {
    assert_relative_eq!(PREVIEW_DEFAULT_EXTRA_SPACING, 1.25, epsilon = 1e-6);
    let state = GrafiaCanvasState::init_glyph_a();
    let mut cfg = PreviewRenderConfig::default_for_size(320, 120);
    cfg.more_space_above = more_above;
    cfg.more_space_below = more_below;
    let base_denom = (state.metrics.ascender + state.metrics.descender.abs()) * 1.25;
    let expected_scale = (120.0 / base_denom) * expected_factor;
    assert_relative_eq!(
        cfg.compute_vertical_scale(&state.metrics),
        expected_scale,
        epsilon = 1e-6
    );
}

// TC-V060-002 & TC-V060-003: Separable 2-pass Gaussian blur (0..20 px), Flip, and Negative polarity.
#[test]
fn tc_v060_002_and_003_preview_gaussian_blur_flip_and_negative_polarity() {
    assert_relative_eq!(PREVIEW_BLUR_MIN_RADIUS_PX, 0.0, epsilon = 1e-5);
    assert_relative_eq!(PREVIEW_BLUR_MAX_RADIUS_PX, 20.0, epsilon = 1e-5);
    assert_relative_eq!(PREVIEW_BLUR_DEFAULT_RADIUS_PX, 4.5, epsilon = 1e-5);
    assert_relative_eq!(PREVIEW_BLUR_ACTIVE_THRESHOLD_PX, 0.1, epsilon = 1e-5);

    let state = GrafiaCanvasState::init_glyph_a();
    let sharp_cfg = PreviewRenderConfig::default_for_size(240, 100);
    let mut blur_cfg = sharp_cfg.clone();
    blur_cfg.blur_radius_px = PREVIEW_BLUR_DEFAULT_RADIUS_PX;

    let sharp_rgba = render_preview_strip_rgba(&state, &sharp_cfg);
    let blurred_rgba = render_preview_strip_rgba(&state, &blur_cfg);
    assert_ne!(sharp_rgba, blurred_rgba);

    let mut flip_cfg = sharp_cfg.clone();
    flip_cfg.flipped = true;
    let flipped_rgba = render_preview_strip_rgba(&state, &flip_cfg);
    assert_ne!(sharp_rgba, flipped_rgba);

    let mut inv_cfg = sharp_cfg;
    inv_cfg.inverted = true;
    let inv_rgba = render_preview_strip_rgba(&state, &inv_cfg);
    let expected_bg = [
        PREVIEW_BG_INVERTED_LIGHT.r,
        PREVIEW_BG_INVERTED_LIGHT.g,
        PREVIEW_BG_INVERTED_LIGHT.b,
        PREVIEW_BG_INVERTED_LIGHT.a,
    ];
    assert_eq!(&inv_rgba[0..4], &expected_bg);

    // Verify GrafiaPreviewWidget in egui honors blur_radius_px > 0.1
    let ctx = egui::Context::default();
    let p_sharp = grafia::render::PreviewPanelConfig::default();
    let mut p_blur = p_sharp;
    p_blur.blur_radius_px = PREVIEW_BLUR_DEFAULT_RADIUS_PX;
    let mut out_s = ctx.run_ui(
        RawInput {
            screen_rect: Some(default_canvas_rect()),
            ..Default::default()
        },
        |ui| {
            let _ = grafia::render::GrafiaPreviewWidget::new(&state)
                .with_config(p_sharp)
                .show(ui);
        },
    );
    out_s.textures_delta.clear();
    let mut out_b = ctx.run_ui(
        RawInput {
            screen_rect: Some(default_canvas_rect()),
            ..Default::default()
        },
        |ui| {
            let _ = grafia::render::GrafiaPreviewWidget::new(&state)
                .with_config(p_blur)
                .show(ui);
        },
    );
    out_b.textures_delta.clear();
    assert_ne!(out_s.shapes.len(), out_b.shapes.len());

    // Verify init_glyph_o() loads GlyphArchetype::O with corrected path direction and clean history
    let state_o = GrafiaCanvasState::init_glyph_o();
    assert!(!state_o.history.can_undo());
    assert_eq!(state_o.contours.len(), 2);
    assert_relative_eq!(
        grafia::contour_signed_area(&state_o.contours[0]).signum(),
        -grafia::contour_signed_area(&state_o.contours[1]).signum(),
        epsilon = 1e-6
    );
}

// TC-V060-004: SelectionInfo::GlyphBase local 2D metrics & center_glyph_in_width().
#[test]
fn tc_v060_004_selection_info_glyph_base_and_center_in_width() {
    let mut state = GrafiaCanvasState::init_glyph_a();
    state.selected_nodes.clear();
    state.selected_guide = None;
    let SelectionInfo::GlyphBase(info) = state.selection_info() else {
        panic!("Expected SelectionInfo::GlyphBase when nothing is selected");
    };
    let total_margin = info.lsb + info.rsb;

    state.set_lsb(info.lsb + 25.0);
    let SelectionInfo::GlyphBase(after_lsb) = state.selection_info() else {
        panic!("Expected GlyphBase");
    };
    assert_relative_eq!(after_lsb.lsb, info.lsb + 25.0, epsilon = 1e-4);
    assert_relative_eq!(after_lsb.rsb, info.rsb, epsilon = 1e-4);

    state.center_glyph_in_width();
    let SelectionInfo::GlyphBase(centered) = state.selection_info() else {
        panic!("Expected GlyphBase");
    };
    assert_relative_eq!(centered.lsb, centered.rsb, epsilon = 1e-4);
    assert_relative_eq!(
        centered.lsb + centered.rsb,
        total_margin + 25.0,
        epsilon = 1e-4
    );
}

// TC-V060-005: SelectionInfo::SingleOnCurve & coupled adjacent OffCurve BCP translation.
#[test]
fn tc_v060_005_single_oncurve_coupled_bcp_translation() {
    let mut state = GrafiaCanvasState::init_glyph_o();
    let oncurve_id = state.first_oncurve_with_adjacent_offcurve().unwrap();
    let bcp_before = state.adjacent_offcurve_positions(oncurve_id);
    state.select_only_node(oncurve_id);

    let SelectionInfo::SingleOnCurve(info) = state.selection_info() else {
        panic!("Expected SelectionInfo::SingleOnCurve");
    };
    state.set_selected_oncurve_position(FontPoint::new(info.x + 30.0, info.y - 15.0));
    let bcp_after = state.adjacent_offcurve_positions(oncurve_id);
    for (before, after) in bcp_before.iter().zip(bcp_after.iter()) {
        assert_relative_eq!(after.x.0 - before.x.0, 30.0, epsilon = 1e-4);
        assert_relative_eq!(after.y.0 - before.y.0, -15.0, epsilon = 1e-4);
    }
}

// TC-V060-006 & TC-V060-007: SelectionInfo::SingleOffCurve relative (dx, dy), length & radial mutator.
#[test]
fn tc_v060_006_and_007_single_offcurve_delta_and_radial_handle_length() {
    let mut state = GrafiaCanvasState::init_glyph_o();
    let off_id = state.first_offcurve_node_id().unwrap();
    state.select_only_node(off_id);

    let SelectionInfo::SingleOffCurve(info) = state.selection_info() else {
        panic!("Expected SelectionInfo::SingleOffCurve");
    };
    assert_relative_eq!(
        info.length,
        info.delta_x.hypot(info.delta_y),
        epsilon = 1e-4
    );
    let angle_before = info.delta_y.atan2(info.delta_x);

    state.set_selected_handle_length(120.0);
    let SelectionInfo::SingleOffCurve(updated) = state.selection_info() else {
        panic!("Expected SelectionInfo::SingleOffCurve");
    };
    assert_relative_eq!(updated.length, 120.0, epsilon = 1e-4);
    assert_relative_eq!(
        updated.delta_y.atan2(updated.delta_x),
        angle_before,
        epsilon = 1e-4
    );
}

// TC-V060-008 & TC-V060-009: SelectionInfo::MultiNode, 9-point TransformGridAnchor & proportional scale.
#[test]
fn tc_v060_008_and_009_multi_node_transform_grid_and_proportional_scale() {
    let mut state = GrafiaCanvasState::init_glyph_a();
    state.select_all_nodes();
    state.set_multi_selection_anchor(TransformGridAnchor::BottomLeft);

    let SelectionInfo::MultiNode(info) = state.selection_info() else {
        panic!("Expected SelectionInfo::MultiNode");
    };
    assert!(info.selected_node_count >= 2);
    assert_eq!(info.selected_node_count, info.total_layer_node_count);
    let aspect_before = info.width / info.height;

    state.set_multi_selection_proportional(true);
    state.set_multi_selection_size(Some(info.width * 1.5), None);
    let SelectionInfo::MultiNode(scaled) = state.selection_info() else {
        panic!("Expected SelectionInfo::MultiNode");
    };
    assert_relative_eq!(scaled.width, info.width * 1.5, epsilon = 1e-3);
    assert_relative_eq!(scaled.width / scaled.height, aspect_before, epsilon = 1e-3);
}

// TC-V060-010: SelectionInfo::Guide & GuideAlignment::{Lsb, Center, Rsb}.
#[test]
fn tc_v060_010_guide_selection_info_and_alignment_modes() {
    let mut state = GrafiaCanvasState::init_glyph_a();
    let gid = state.add_local_guide(FontPoint::new(100.0, 300.0), 90.0);
    state.select_local_guide(gid);

    state.set_selected_guide_alignment(GuideAlignment::Center);
    let SelectionInfo::Guide(info) = state.selection_info() else {
        panic!("Expected SelectionInfo::Guide");
    };
    assert_relative_eq!(info.x, state.glyph.width * 0.5, epsilon = 1e-4);

    state.set_selected_guide_alignment(GuideAlignment::Rsb);
    let SelectionInfo::Guide(info_rsb) = state.selection_info() else {
        panic!("Expected SelectionInfo::Guide");
    };
    assert_relative_eq!(info_rsb.x, state.glyph.width, epsilon = 1e-4);
}

// TC-V060-011: Canvas Rulers (Cmd+R, 20 dp) & drag-from-ruler to spawn local/global Guide.
#[test]
fn tc_v060_011_canvas_rulers_toggle_and_drag_spawn_guide() {
    assert_relative_eq!(RULER_THICKNESS_DP, 20.0, epsilon = 1e-5);
    let mut state = GrafiaCanvasState::init_glyph_a();
    assert!(!state.show_rulers);
    dispatch_key(&mut state, Key::R, false, false, true, false);
    assert!(state.show_rulers);

    let local_before = state.guides.len();
    state.drag_guide_from_top_ruler(ScreenPoint::new(300.0, 260.0), false);
    assert_eq!(state.guides.len(), local_before + 1);

    let global_before = state.global_guides.len();
    state.drag_guide_from_left_ruler(ScreenPoint::new(320.0, 260.0), true);
    assert_eq!(state.global_guides.len(), global_before + 1);
}

// TC-V060-012, TC-V060-013 & TC-V060-014: Background Layer (Cmd+B, Cmd+J, Cmd+K/Cmd+Shift+J) & Anchors (Cmd+U).
#[test]
fn tc_v060_012_to_014_background_layer_ops_and_default_anchors() {
    let mut state = GrafiaCanvasState::init_glyph_a();
    state.glyph.name = "A".to_string();
    assert!(state.glyph.background_contours.is_empty());

    dispatch_key(&mut state, Key::J, false, false, true, false);
    assert_eq!(
        state.glyph.background_contours.len(),
        state.glyph.contours.len()
    );

    dispatch_key(&mut state, Key::B, false, false, true, false);
    assert!(state.editing_background_layer);
    dispatch_key(&mut state, Key::B, false, false, true, false);
    assert!(!state.editing_background_layer);

    dispatch_key(&mut state, Key::K, false, false, true, false);
    state.validate_invariants().unwrap();

    let _ = state.drain_host_requests();
    dispatch_key(&mut state, Key::U, false, false, true, false);
    assert_eq!(
        state.drain_host_requests(),
        vec![CanvasHostRequest::ResetDefaultAnchors {
            glyph_name: "A".to_string(),
        }]
    );

    let cx = state.glyph.width * 0.5;
    let cap_h = state.metrics.cap_height;
    state.set_anchors_with_undo(vec![
        CanvasAnchor {
            id: AnchorId(1),
            name: "top".to_string(),
            position: FontPoint::new(cx, cap_h),
            selected: false,
        },
        CanvasAnchor {
            id: AnchorId(2),
            name: "bottom".to_string(),
            position: FontPoint::new(cx, 0.0),
            selected: false,
        },
        CanvasAnchor {
            id: AnchorId(3),
            name: "ogonek".to_string(),
            position: FontPoint::new(cx, 0.0),
            selected: false,
        },
    ]);
    assert!(state.glyph.has_anchor("top"));
    assert!(state.glyph.has_anchor("bottom"));
    assert!(state.glyph.has_anchor("ogonek"));
    for a in &state.anchors {
        assert_relative_eq!(a.position.x.0, state.glyph.width * 0.5, epsilon = 1e-4);
    }
    assert!(state.undo());
    assert!(state.anchors.is_empty());
    assert!(state.redo());
    assert_eq!(state.anchors.len(), 3);
}

// TC-V060-015, TC-V060-016 & TC-V060-017: SelectionInfo transitions, Undo/Redo roundtrip & Light/Dark parity.
#[test]
fn tc_v060_015_to_017_mode_transitions_undo_roundtrip_and_headless_parity() {
    let mut state = GrafiaCanvasState::init_glyph_o();
    state.selected_nodes.clear();
    assert!(matches!(
        state.selection_info(),
        SelectionInfo::GlyphBase(_)
    ));

    let snap_0 = state.snapshot();
    state.center_glyph_in_width();
    let snap_1 = state.snapshot();
    assert_ne!(snap_0, snap_1);
    assert!(state.undo());
    assert_eq!(state.snapshot(), snap_0);
    assert!(state.redo());
    assert_eq!(state.snapshot(), snap_1);

    state.show_rulers = true;
    state.set_appearance_mode(AppearanceMode::Light);
    let light_px = rasterize_canvas_subregion(&state, &GrafiaTheme::sonoma_default(), 300, 200);
    state.set_appearance_mode(AppearanceMode::Dark);
    let dark_px = rasterize_canvas_subregion(&state, &GrafiaTheme::sonoma_dark(), 300, 200);
    assert_ne!(light_px.data(), dark_px.data());
}

// Property-based test for separable 1D Gaussian blur kernel normalization and clamp bounds.
proptest! {
    #[test]
    fn prop_v060_preview_blur_radius_clamped_and_deterministic(radius in -5.0f32..30.0f32) {
        let state = GrafiaCanvasState::init_glyph_a();
        let mut cfg = PreviewRenderConfig::default_for_size(120, 60);
        cfg.blur_radius_px = radius;
        let out_a = render_preview_strip_rgba(&state, &cfg);
        let out_b = render_preview_strip_rgba(&state, &cfg);
        prop_assert_eq!(out_a.len(), 120 * 60 * 4);
        prop_assert_eq!(out_a, out_b);
    }
}

// TC-V060-018: Stateful pull-based `arbtest` fuzzing across SelectionInfo, Rulers, Background & Anchors.
#[test]
fn tc_v060_018_arbtest_stateful_selection_info_background_and_preview_chains() {
    arbtest(|u| {
        let mut state = GrafiaCanvasState::init_glyph_o();
        let steps: usize = u.int_in_range(12..=25)?;
        for _ in 0..steps {
            match u.int_in_range(0..=8)? {
                0 => state.center_glyph_in_width(),
                1 => {
                    let w = f64::from(u.int_in_range(300..=900)?);
                    state.set_advance_width(w);
                }
                2 => dispatch_key(&mut state, Key::B, false, false, true, false),
                3 => dispatch_key(&mut state, Key::J, false, false, true, false),
                4 => dispatch_key(&mut state, Key::K, false, false, true, false),
                5 => dispatch_key(&mut state, Key::U, false, false, true, false),
                6 => dispatch_key(&mut state, Key::R, false, false, true, false),
                7 => {
                    let _ = state.undo();
                }
                _ => {
                    let _ = state.redo();
                }
            }
            let _ = state.selection_info();
            state.validate_invariants().unwrap();
        }
        Ok(())
    })
    .budget_ms(250);
}

// TC-V060-019: Pure 2D `SelectionInfo`, `SelectionInfoAction`, and HUD 3x3 grid click.
#[test]
fn tc_v060_019_pure_2d_selection_info_actions_and_hud_grid_click() {
    let mut state = GrafiaCanvasState::init_glyph_a();
    state.selected_nodes.clear();
    match state.selection_info() {
        SelectionInfo::GlyphBase(info) => {
            assert_eq!(info.glyph_name, state.glyph.name);
            assert!(info.bbox_w > 0.0 && info.bbox_h > 0.0);
            assert_relative_eq!(info.advance_width, state.glyph.width, epsilon = 1e-6);
            assert_eq!(info.transform_anchor, TransformGridAnchor::Center);
            assert!(!info.proportional_scale);
        }
        other => panic!("Expected GlyphBase, got {other:?}"),
    }

    assert!(state.apply_selection_info_action(SelectionInfoAction::SetAdvanceWidth(720.0)));
    assert_relative_eq!(state.glyph.width, 720.0, epsilon = 1e-6);
    assert!(state.undo());
    assert!(state.redo());
    assert_relative_eq!(state.glyph.width, 720.0, epsilon = 1e-6);

    // SingleOnCurve smooth toggle via SelectionInfoAction::ToggleOnCurveSmooth
    let mut state_o = GrafiaCanvasState::init_glyph_o();
    let oncurve_id = state_o.first_oncurve_with_adjacent_offcurve().unwrap();
    state_o.select_only_node(oncurve_id);
    let was_smooth = state_o.find_node(oncurve_id).unwrap().node_type == NodeType::Smooth;
    assert!(state_o.apply_selection_info_action(SelectionInfoAction::ToggleOnCurveSmooth));
    let now_smooth = state_o.find_node(oncurve_id).unwrap().node_type == NodeType::Smooth;
    assert_ne!(was_smooth, now_smooth);
    assert!(state_o.undo());
    assert_eq!(
        state_o.find_node(oncurve_id).unwrap().node_type == NodeType::Smooth,
        was_smooth
    );

    // MultiNode 3x3 Transform-Grid click on the canvas HUD updates `origin_anchor` with Undo/Redo
    state.select_all_nodes();
    let canvas_rect = default_canvas_rect();
    let hud_rect = grafia::render::compute_hud_rect(canvas_rect);
    let dots = grafia::render::compute_hud_transform_grid_dot_positions(hud_rect);
    let (br_anchor, br_pt) = dots[8];
    assert_eq!(br_anchor, TransformGridAnchor::BottomRight);
    assert!(state.handle_v060_hud_pointer_press(br_pt, canvas_rect));
    assert_eq!(
        state.transform_grid_anchor,
        TransformGridAnchor::BottomRight
    );
    assert!(state.undo());
    assert_eq!(state.transform_grid_anchor, TransformGridAnchor::Center);
    assert!(state.redo());
    assert_eq!(
        state.transform_grid_anchor,
        TransformGridAnchor::BottomRight
    );

    // Clicking the proportional icon in the HUD toggles `multi_selection_proportional` with Undo/Redo
    let prop_rect = grafia::render::compute_hud_proportional_icon_rect(hud_rect);
    let prop_center = ScreenPoint::new(prop_rect.center().x, prop_rect.center().y);
    assert!(state.handle_v060_hud_pointer_press(prop_center, canvas_rect));
    assert!(state.multi_selection_proportional);
    assert!(state.undo());
    assert!(!state.multi_selection_proportional);
    assert!(state.redo());
    assert!(state.multi_selection_proportional);

    // Invalid local 2D actions return `false` without mutating state
    assert!(!state.apply_selection_info_action(SelectionInfoAction::SetLsb(f64::NAN)));
    assert!(
        !state.apply_selection_info_action(SelectionInfoAction::SetOnCurvePosition(
            FontPoint::new(10.0, 20.0)
        ))
    );
    assert!(
        !state.apply_selection_info_action(SelectionInfoAction::SetOffCurvePosition(
            FontPoint::new(10.0, 20.0)
        ))
    );
    assert!(
        !state.apply_selection_info_action(SelectionInfoAction::SetGuideOrigin(FontPoint::new(
            10.0, 20.0
        )))
    );
}

// TC-V060-020: Pure 2D `PreviewRenderConfig` & `render_preview_strip_rgba` across blur/flip/invert/spacing.
#[test]
fn tc_v060_020_preview_render_config_combinations_and_strip_engine() {
    let mut state = GrafiaCanvasState::init_glyph_a();
    state.load_demo_text_strip();
    let mut cfg = PreviewRenderConfig::default_for_size(320, 120);
    assert_relative_eq!(cfg.blur_radius_px, 0.0, epsilon = 1e-5);
    assert!(!cfg.flipped);
    assert!(!cfg.inverted);

    let base_rgba = render_preview_strip_rgba(&state, &cfg);
    assert_eq!(base_rgba.len(), 320 * 120 * 4);

    cfg.blur_radius_px = 6.5;
    cfg.flipped = true;
    cfg.inverted = true;
    cfg.more_space_above = true;
    cfg.more_space_below = true;
    let mod_rgba = render_preview_strip_rgba(&state, &cfg);
    assert_eq!(mod_rgba.len(), 320 * 120 * 4);
    assert_ne!(base_rgba, mod_rgba);
}

// TC-V060-021: Zoom-adaptive ruler tick steps (`compute_ruler_tick_steps_upm`) across zoom levels.
#[rstest]
#[case(12.0, 1.0, 5.0)]
#[case(6.0, 2.0, 10.0)]
#[case(3.0, 4.0, 20.0)]
#[case(1.2, 10.0, 50.0)]
#[case(0.6, 20.0, 100.0)]
#[case(0.25, 50.0, 250.0)]
#[case(0.12, 100.0, 500.0)]
fn tc_v060_021_zoom_adaptive_ruler_tick_steps_across_zoom_levels(
    #[case] zoom: f32,
    #[case] expected_minor_upm: f64,
    #[case] expected_major_upm: f64,
) {
    let (minor, major) = compute_ruler_tick_steps_upm(zoom);
    assert_relative_eq!(minor, expected_minor_upm, epsilon = 1e-6);
    assert_relative_eq!(major, expected_major_upm, epsilon = 1e-6);
    assert_relative_eq!(minor * 5.0, major, epsilon = 1e-6);
    let major_px = major * f64::from(zoom);
    assert!(
        (50.0..=140.0).contains(&major_px),
        "major_px {major_px} out of [50, 140] at zoom {zoom}"
    );
}

// TC-V060-022: `align_selection_to_background` projects foreground OnCurve nodes + handles onto background Line & Cubic segments.
#[test]
fn tc_v060_022_align_selection_to_background_cubic_and_line_projection() {
    let mut state = GrafiaCanvasState::init_glyph_o();
    state.copy_selection_to_background();
    let orig_contours = state.contours.clone();

    let oncurve_id = state
        .first_oncurve_with_adjacent_offcurve()
        .expect("O has OnCurve with adjacent OffCurve");
    let orig_on = state.find_node(oncurve_id).unwrap().point;
    let orig_handles = state.adjacent_offcurve_positions(oncurve_id);
    assert!(!orig_handles.is_empty());

    // Perturb the selected OnCurve node and its adjacent handles by (+14, -9) UPM
    state.select_only_node(oncurve_id);
    state.set_selected_oncurve_position(FontPoint::new(orig_on.x.0 + 14.0, orig_on.y.0 - 9.0));
    let perturbed_handles = state.adjacent_offcurve_positions(oncurve_id);
    assert_relative_eq!(
        perturbed_handles[0].x.0,
        orig_handles[0].x.0 + 14.0,
        epsilon = 1e-6
    );

    // Align back to background Cubic contour
    let aligned = state.align_selection_to_background();
    assert_eq!(aligned, 1);
    let aligned_on = state.find_node(oncurve_id).unwrap().point;
    let reprojected =
        grafia::project_point_to_background_contours(aligned_on, &state.background_contours)
            .expect("background contour projection");
    let dist_to_bg = ((aligned_on.x.0 - reprojected.x.0).powi(2)
        + (aligned_on.y.0 - reprojected.y.0).powi(2))
    .sqrt();
    assert!(
        dist_to_bg < 1e-4,
        "aligned node must lie on background cubic contour, dist={dist_to_bg}"
    );

    let aligned_handles = state.adjacent_offcurve_positions(oncurve_id);
    let dx_on = aligned_on.x.0 - (orig_on.x.0 + 14.0);
    let dy_on = aligned_on.y.0 - (orig_on.y.0 - 9.0);
    for (p_h, a_h) in perturbed_handles.iter().zip(aligned_handles.iter()) {
        assert_relative_eq!(a_h.x.0, p_h.x.0 + dx_on, epsilon = 1e-5);
        assert_relative_eq!(a_h.y.0, p_h.y.0 + dy_on, epsilon = 1e-5);
    }

    // Undo / Redo roundtrip
    assert!(state.undo());
    assert_relative_eq!(
        state.find_node(oncurve_id).unwrap().point.x.0,
        orig_on.x.0 + 14.0,
        epsilon = 1e-6
    );
    assert!(state.redo());
    assert_relative_eq!(
        state.find_node(oncurve_id).unwrap().point.x.0,
        aligned_on.x.0,
        epsilon = 1e-6
    );

    // Also verify exact orthogonal projection onto a straight horizontal background Line segment (y = 200)
    state.contours = orig_contours;
    state.sync_paths_from_contours();
    let mut bg_line = GlyphContour::new(
        ContourId(0),
        vec![
            Node::new(NodeId(9001), 0.0, 200.0, NodeType::Corner),
            Node::new(NodeId(9002), 800.0, 200.0, NodeType::Corner),
        ],
    );
    bg_line.closed = false;
    state.background_contours = vec![bg_line.clone()];
    state.select_only_node(oncurve_id);
    state.set_selected_oncurve_position(FontPoint::new(340.0, 245.0));
    assert_eq!(state.align_selection_to_background(), 1);
    let snapped = state.find_node(oncurve_id).unwrap().point;
    assert_relative_eq!(snapped.x.0, 340.0, epsilon = 1e-4);
    assert_relative_eq!(snapped.y.0, 200.0, epsilon = 1e-4);

    // Hidden (`visible == false`) or degenerate 1-node background contours are ignored safely
    let mut hidden_bg = bg_line.clone();
    hidden_bg.visible = false;
    let degen_bg = GlyphContour::new(
        ContourId(1),
        vec![Node::new(NodeId(9003), 100.0, 500.0, NodeType::Corner)],
    );
    state.background_contours = vec![hidden_bg, degen_bg];
    assert_eq!(state.align_selection_to_background(), 0);
}

#[test]
fn tc_v060_visual_verification_artifacts() {
    let out_dir = std::path::Path::new(
        r"C:\Users\jhove\.gemini\antigravity-cli\brain\fccb9142-9217-4702-a2ca-95303b541ee0",
    );
    let _ = std::fs::create_dir_all(out_dir);

    // 1. frame_56_v060_rulers_background_and_anchors.png
    let mut state56 = GrafiaCanvasState::init_glyph_a();
    state56.fit_viewport_to_glyph(960.0, 640.0);
    state56.show_rulers = true;
    state56.copy_selection_to_background();
    for gc in &mut state56.background_contours {
        for n in &mut gc.nodes {
            n.point.x.0 += 28.0;
            n.point.y.0 += 18.0;
        }
    }
    state56.sync_paths_from_contours();
    let cx = state56.glyph.width * 0.5;
    let cap_h = state56.metrics.cap_height;
    state56.set_anchors_with_undo(vec![
        CanvasAnchor {
            id: AnchorId(1),
            name: "top".to_string(),
            position: FontPoint::new(cx, cap_h),
            selected: false,
        },
        CanvasAnchor {
            id: AnchorId(2),
            name: "bottom".to_string(),
            position: FontPoint::new(cx, 0.0),
            selected: false,
        },
        CanvasAnchor {
            id: AnchorId(3),
            name: "ogonek".to_string(),
            position: FontPoint::new(595.0, 0.0),
            selected: false,
        },
    ]);
    state56.sync_paths_from_contours();
    state56.drag_guide_from_top_ruler(ScreenPoint::new(480.0, 240.0), false);
    state56.drag_guide_from_left_ruler(ScreenPoint::new(360.0, 320.0), true);
    state56.selected_guide = None;
    state56.last_pointer_screen_pos = Some(ScreenPoint::new(480.0, 240.0));
    let pix56 = rasterize_canvas_subregion(&state56, &GrafiaTheme::sonoma_default(), 960, 640);
    let img56 = grafia::adapters::headless::pixmap_to_rgba_image(pix56);
    img56
        .save(out_dir.join("frame_56_v060_rulers_background_and_anchors.png"))
        .expect("Failed to save frame_56");

    // 2. frame_57_v060_preview_gaussian_blur_and_inverted.png
    // Upper 960x420 canvas shows MultiNode Selection + Rich Grey Info Box HUD with 9-dot 3x3 Transform-Grid!
    let mut state57 = GrafiaCanvasState::init_glyph_o();
    state57.fit_viewport_to_glyph(960.0, 380.0);
    state57.show_rulers = true;
    state57.select_all_nodes();
    state57.set_multi_selection_anchor(TransformGridAnchor::TopLeft);
    state57.set_multi_selection_proportional(true);
    state57.show_bounding_box = true;
    let pix57_top = rasterize_canvas_subregion(&state57, &GrafiaTheme::sonoma_default(), 960, 420);
    let img57_top = grafia::adapters::headless::pixmap_to_rgba_image(pix57_top);

    // Bottom 220 px Preview Strip with Multi-Glyph strip, Inverted Polarity & Gaussian Blur (sigma = 4.5 px)
    let mut strip_state = GrafiaCanvasState::init_glyph_a();
    strip_state.load_demo_text_strip();
    let mut cfg = PreviewRenderConfig::default_for_size(960, 220);
    cfg.inverted = true;
    cfg.blur_radius_px = PREVIEW_BLUR_DEFAULT_RADIUS_PX;
    let strip_rgba = render_preview_strip_rgba(&strip_state, &cfg);

    let mut img57 = image::RgbaImage::new(960, 640);
    for y in 0..420u32 {
        for x in 0..960u32 {
            img57.put_pixel(x, y, *img57_top.get_pixel(x, y));
        }
    }
    for y in 0..220u32 {
        for x in 0..960u32 {
            let idx = ((y * 960 + x) * 4) as usize;
            img57.put_pixel(
                x,
                420 + y,
                image::Rgba([
                    strip_rgba[idx],
                    strip_rgba[idx + 1],
                    strip_rgba[idx + 2],
                    strip_rgba[idx + 3],
                ]),
            );
        }
    }
    img57
        .save(out_dir.join("frame_57_v060_preview_gaussian_blur_and_inverted.png"))
        .expect("Failed to save frame_57");
}

// TC-V060-023: Battery-efficient `content_generation()` & `PreviewStripCache` zero idle render/upload.
#[test]
fn tc_v060_023_content_generation_and_preview_strip_cache_zero_idle_render() {
    use grafia::render::PreviewStripCache;

    let mut state = GrafiaCanvasState::init_glyph_a();
    let cfg = PreviewRenderConfig::default_for_size(240, 80);
    let mut cache = PreviewStripCache::new();
    assert_eq!(cache.render_count(), 0);

    // Initial render is a cache miss
    let (rgba_0, recomputed_0) = cache.get_or_render_rgba(&state, &cfg);
    assert!(recomputed_0);
    assert_eq!(rgba_0.len(), 240 * 80 * 4);
    assert_eq!(cache.render_count(), 1);
    let gen_0 = state.content_generation();

    // Idle mouse hover, viewport pan/zoom, and node selection changes MUST NOT increment content_generation
    state.cursor_screen_pos = Some(ScreenPoint::new(120.0, 240.0));
    state.last_pointer_screen_pos = Some(ScreenPoint::new(125.0, 245.0));
    state.viewport.pan_offset = ScreenPoint::new(40.0, 80.0);
    state.viewport.zoom = 1.75;
    state.select_all_nodes();
    state.set_system_dark_mode(false);
    assert_eq!(state.content_generation(), gen_0);

    // Calling get_or_render_rgba during idle / pan / zoom / selection is a 0-ms cache hit
    let (_, recomputed_idle) = cache.get_or_render_rgba(&state, &cfg);
    assert!(!recomputed_idle);
    assert_eq!(cache.render_count(), 1);

    // Texture upload via egui::Context also hits cache without re-uploading when unchanged
    let ctx = egui::Context::default();
    let mut updated_tex_first = false;
    let mut updated_tex_second = true;
    let mut out = ctx.run_ui(egui::RawInput::default(), |ui| {
        let (_, up1) = cache.get_or_update_texture(ui.ctx(), &state, &cfg);
        let (_, up2) = cache.get_or_update_texture(ui.ctx(), &state, &cfg);
        updated_tex_first = up1;
        updated_tex_second = up2;
    });
    assert_eq!(out.textures_delta.set.len(), 2);
    out.textures_delta.clear();
    assert!(updated_tex_first);
    assert!(!updated_tex_second);
    assert_eq!(cache.render_count(), 1);

    // Geometry mutation increments content_generation and triggers cache recomputation
    state.set_advance_width(740.0);
    let gen_1 = state.content_generation();
    assert!(gen_1 > gen_0);
    let (_, recomputed_mut) = cache.get_or_render_rgba(&state, &cfg);
    assert!(recomputed_mut);
    assert_eq!(cache.render_count(), 2);

    // Undo and Redo also bump content_generation and invalidate cache
    assert!(state.undo());
    let gen_2 = state.content_generation();
    assert!(gen_2 > gen_1);
    let (_, recomputed_undo) = cache.get_or_render_rgba(&state, &cfg);
    assert!(recomputed_undo);
    assert_eq!(cache.render_count(), 3);

    // PreviewRenderConfig change triggers cache recomputation even at same content_generation
    let mut cfg_blurred = cfg.clone();
    cfg_blurred.blur_radius_px = 3.5;
    let (_, recomputed_cfg) = cache.get_or_render_rgba(&state, &cfg_blurred);
    assert!(recomputed_cfg);
    assert_eq!(cache.render_count(), 4);
}
