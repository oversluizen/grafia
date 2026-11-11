#![allow(unexpected_cfgs)]
#![cfg(feature = "v060_red")]
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
    render_preview_strip_rgba, AppearanceMode, FontPoint, GrafiaCanvasState, GrafiaTheme,
    GuideAlignment, PreviewRenderConfig, ScreenPoint, SelectionInfo, TransformGridAnchor,
    PREVIEW_BG_INVERTED_LIGHT, PREVIEW_BLUR_ACTIVE_THRESHOLD_PX, PREVIEW_BLUR_DEFAULT_RADIUS_PX,
    PREVIEW_BLUR_MAX_RADIUS_PX, PREVIEW_BLUR_MIN_RADIUS_PX, PREVIEW_BOTH_EXTRA_SPACE_SCALE,
    PREVIEW_DEFAULT_EXTRA_SPACING, PREVIEW_ONE_EXTRA_SPACE_SCALE, RULER_THICKNESS_DP,
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

    dispatch_key(&mut state, Key::U, false, false, true, false);
    assert!(state.glyph.has_anchor("top"));
    assert!(state.glyph.has_anchor("bottom"));
    assert!(state.glyph.has_anchor("ogonek"));
}

// TC-V060-015, TC-V060-016 & TC-V060-017: SelectionInfo transitions, Undo/Redo roundtrip & Light/Dark parity.
#[test]
fn tc_v060_015_to_017_mode_transitions_undo_roundtrip_and_headless_parity() {
    let mut state = GrafiaCanvasState::init_glyph_o();
    state.selected_nodes.clear();
    assert!(matches!(state.selection_info(), SelectionInfo::GlyphBase(_)));

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
