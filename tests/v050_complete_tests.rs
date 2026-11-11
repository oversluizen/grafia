#![allow(unexpected_cfgs, clippy::too_many_arguments)]
//! Normative COTS Verification Suite for `SPEC-006` (`grafia v0.5.0`):
//! Multi-Glyph Horizontal Text Strip (`CanvasTextStream`), Interactive Spacing (`LSB` / `RSB` /
//! Contour Shift), Kerning Delta Emission, Baseline Highlight Wedges, and Text-Mode Measurement Line
//! (`TC-V050-001` .. `TC-V050-018`).
//!
//! Standards: ISO/IEC/IEEE 42010:2011, ISO/IEC/IEEE 29119-3:2021, IEEE 1012-2016, SemVer 2.0.0.
//! Baseline: OpenType 1.9 Horizontal Metrics (`hmtx` / `GPOS` Pair Adjustment) & CAGD Multi-Slot Layout.

use approx::assert_relative_eq;
use arbtest::arbtest;
use egui::{Event, Key, Modifiers as EguiModifiers, Pos2, RawInput, Rect, Vec2};
use grafia::adapters::headless::{
    pixmap_to_rgba_image, rasterize_canvas_subregion, rasterize_full_window,
};
use grafia::adapters::input_dispatcher::InputDispatcher;
use grafia::{
    AppearanceMode, CanvasHostRequest, CanvasTool, ContourId, FontPoint, Glyph, GlyphContour,
    GlyphSlot, GrafiaCanvasState, GrafiaTheme, KerningDelta, KerningIndicatorQuad, Node, NodeId,
    NodeType, RgbaColorToken, ScreenPoint, SlotScanlineMeasurement, SpacingDelta,
    TextStreamEditKind, TextStreamEditRequest, KERNING_INCREMENT_HIGH_UPM,
    KERNING_INCREMENT_LOW_UPM, KERNING_NEGATIVE_FILL_LIGHT, KERNING_POSITIVE_FILL_LIGHT,
    SPACING_INCREMENT_HIGH_UPM, SPACING_INCREMENT_LOW_UPM, TEXT_MEASUREMENT_LINE_DEFAULT_Y_UPM,
    TEXT_MODE_NUMBERS_THRESHOLD_DEFAULT,
};
use proptest::prelude::*;
use rstest::rstest;

fn default_canvas_rect() -> Rect {
    Rect::from_min_size(Pos2::ZERO, Vec2::new(960.0, 600.0))
}

fn make_stem_glyph(name: &str, min_x: f64, max_x: f64, advance_width: f64) -> Glyph {
    let nodes = vec![
        Node::new(NodeId(1), min_x, 0.0, NodeType::Corner),
        Node::new(NodeId(2), max_x, 0.0, NodeType::Corner),
        Node::new(NodeId(3), max_x, 700.0, NodeType::Corner),
        Node::new(NodeId(4), min_x, 700.0, NodeType::Corner),
    ];
    let mut g = Glyph::new(name, 1000);
    g.width = advance_width;
    g.height = 700.0;
    g.contours.push(GlyphContour::new(ContourId(0), nodes));
    g
}

fn make_havo_strip_state() -> GrafiaCanvasState {
    let mut state = GrafiaCanvasState::init_glyph_a();
    let slots = vec![
        GlyphSlot::new(make_stem_glyph("H", 60.0, 540.0, 600.0), 0),
        GlyphSlot::new(make_stem_glyph("A", 40.0, 560.0, 600.0), -20),
        GlyphSlot::new(make_stem_glyph("V", 40.0, 560.0, 600.0), -80),
        GlyphSlot::new(make_stem_glyph("O", 50.0, 600.0, 650.0), 30),
    ];
    state.set_text_stream_slots(slots, 1);
    state
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

// TC-V050-001: Multi-glyph horizontal layout & cumulative origin formula.
#[test]
fn tc_v050_001_multi_glyph_cumulative_origins_with_kerning() {
    let state = make_havo_strip_state();
    let slots = &state.text_stream.slots;
    assert_eq!(slots.len(), 4);
    assert_relative_eq!(slots[0].origin_x_upm, 0.0, epsilon = 1e-6);
    assert_relative_eq!(slots[1].origin_x_upm, 580.0, epsilon = 1e-6);
    assert_relative_eq!(slots[2].origin_x_upm, 1100.0, epsilon = 1e-6);
    assert_relative_eq!(slots[3].origin_x_upm, 1730.0, epsilon = 1e-6);
    state.validate_invariants().unwrap();
}

// TC-V050-002 & TC-V050-003: Active editable slot vs. passive context slots & double-click switch.
#[test]
fn tc_v050_002_and_003_active_vs_passive_slots_and_double_click_switch() {
    let mut state = make_havo_strip_state();
    state.active_tool = CanvasTool::Text;
    assert_eq!(state.text_stream.active_slot_index, 1);
    assert_eq!(state.glyph.name, "A");

    let slot2_center_upm = FontPoint::new(1400.0, 350.0);
    state.handle_double_click_upm(slot2_center_upm);
    assert_eq!(state.text_stream.active_slot_index, 2);
    assert_eq!(state.text_stream.caret_index, 2);
    assert_eq!(state.glyph.name, "V");
    assert_eq!(state.active_tool, CanvasTool::Select);
    assert_relative_eq!(state.active_slot_origin_x_upm(), 1100.0, epsilon = 1e-6);
    let slot2_node_scr = state.viewport.to_screen(FontPoint::new(1140.0, 0.0));
    assert!(state.hit_test_node(slot2_node_scr).is_some());
    state.validate_invariants().unwrap();
}

// TC-V050-004 & TC-V050-005: TextTool ('T'), caret placement, drag selection, Shift+Click & Escape.
#[test]
fn tc_v050_004_and_005_text_tool_caret_selection_and_escape() {
    let mut state = make_havo_strip_state();
    state.active_tool = CanvasTool::Select;
    dispatch_key(&mut state, Key::T, false, false, false, false);
    assert_eq!(state.active_tool, CanvasTool::Text);

    state.place_text_caret_at_upm(FontPoint::new(100.0, 200.0), false);
    assert_eq!(state.text_stream.caret_index, 0);
    assert_eq!(state.text_stream.active_slot_index, 0);

    state.place_text_caret_at_upm(FontPoint::new(1650.0, 200.0), true);
    assert_eq!(state.text_stream.selection_range, Some((0, 3)));

    dispatch_key(&mut state, Key::Escape, false, false, false, false);
    assert_eq!(state.active_tool, CanvasTool::Select);
    assert_eq!(state.text_stream.selection_range, None);
}

// TC-V050-006: Home / End & Left / Right arrow caret stepping across slots.
#[test]
fn tc_v050_006_home_end_and_arrow_slot_stepping() {
    let mut state = make_havo_strip_state();
    state.active_tool = CanvasTool::Text;
    state.set_caret_index(1);
    dispatch_key(&mut state, Key::ArrowRight, false, false, false, false);
    assert_eq!(state.text_stream.caret_index, 2);
    assert_eq!(state.text_stream.active_slot_index, 2);

    dispatch_key(&mut state, Key::Home, false, false, false, false);
    assert_eq!(state.text_stream.active_slot_index, 1);
    dispatch_key(&mut state, Key::End, false, false, false, false);
    assert_eq!(state.text_stream.active_slot_index, 2);
}

// TC-V050-007, TC-V050-008 & TC-V050-009: Parameterized LSB, RSB, and Contour-Shift keyboard matrix.
#[rstest]
#[case::ctrl_right_lsb_low(false, false, true, Key::ArrowRight, 1, 0, 1)]
#[case::ctrl_shift_left_lsb_high(true, false, true, Key::ArrowLeft, -10, 0, -10)]
#[case::cmd_right_rsb_low(false, true, false, Key::ArrowRight, 0, 1, 1)]
#[case::cmd_shift_left_rsb_high(true, true, false, Key::ArrowLeft, 0, -10, -10)]
#[case::cmd_ctrl_right_shift_contours(false, true, true, Key::ArrowRight, 1, -1, 0)]
#[case::cmd_ctrl_shift_left_shift_contours(true, true, true, Key::ArrowLeft, -10, 10, 0)]
fn tc_v050_007_to_009_spacing_keyboard_matrix_and_deltas(
    #[case] shift: bool,
    #[case] cmd: bool,
    #[case] ctrl: bool,
    #[case] key: Key,
    #[case] expected_lsb_delta: i32,
    #[case] expected_rsb_delta: i32,
    #[case] expected_width_delta: i32,
) {
    assert_eq!(SPACING_INCREMENT_LOW_UPM, 1);
    assert_eq!(SPACING_INCREMENT_HIGH_UPM, 10);
    let mut state = make_havo_strip_state();
    state.active_tool = CanvasTool::Text;
    let lsb_before = state.active_glyph_lsb();
    let rsb_before = state.active_glyph_rsb();
    let width_before = state.glyph.width;

    dispatch_key(&mut state, key, shift, false, cmd, ctrl);

    assert_relative_eq!(
        state.active_glyph_lsb(),
        lsb_before + f64::from(expected_lsb_delta),
        epsilon = 1e-6
    );
    assert_relative_eq!(
        state.active_glyph_rsb(),
        rsb_before + f64::from(expected_rsb_delta),
        epsilon = 1e-6
    );
    assert_relative_eq!(
        state.glyph.width,
        width_before + f64::from(expected_width_delta),
        epsilon = 1e-6
    );
    let deltas: Vec<SpacingDelta> = state.drain_spacing_deltas();
    assert_eq!(deltas.len(), 1);
    assert_eq!(deltas[0].lsb_delta_upm, expected_lsb_delta);
    assert_eq!(deltas[0].rsb_delta_upm, expected_rsb_delta);
    assert_eq!(deltas[0].width_delta_upm, expected_width_delta);
}

// TC-V050-010, TC-V050-011 & TC-V050-012: Left & Right Kerning shortcuts + KerningDelta emission.
#[allow(clippy::too_many_arguments)]
#[rstest]
#[case::left_kern_low(false, false, true, Key::ArrowLeft, 0, 1, -1, -21)]
#[case::left_kern_high(true, false, true, Key::ArrowRight, 0, 1, 10, -10)]
#[case::right_kern_low(false, true, false, Key::ArrowRight, 1, 2, 1, -79)]
#[case::right_kern_high(true, true, false, Key::ArrowLeft, 1, 2, -10, -90)]
fn tc_v050_010_to_012_kerning_shortcuts_and_host_delta_events(
    #[case] shift: bool,
    #[case] cmd: bool,
    #[case] ctrl: bool,
    #[case] key: Key,
    #[case] expected_left_slot: usize,
    #[case] expected_right_slot: usize,
    #[case] expected_delta: i32,
    #[case] expected_new_kern: i32,
) {
    assert_eq!(KERNING_INCREMENT_LOW_UPM, 1);
    assert_eq!(KERNING_INCREMENT_HIGH_UPM, 10);
    let mut state = make_havo_strip_state();
    state.active_tool = CanvasTool::Text;
    dispatch_key(&mut state, key, shift, true, cmd, ctrl);

    let deltas: Vec<KerningDelta> = state.drain_kerning_deltas();
    assert_eq!(deltas.len(), 1);
    assert_eq!(deltas[0].left_slot_index, expected_left_slot);
    assert_eq!(deltas[0].right_slot_index, expected_right_slot);
    assert_eq!(deltas[0].delta_upm, expected_delta);
    assert_eq!(deltas[0].new_kern_upm, expected_new_kern);
}

// TC-V050-013: Kerning baseline highlight wedges (Negative Cyan vs. Positive Yellow + italic shear).
#[test]
fn tc_v050_013_kerning_highlight_wedges_and_colors() {
    let mut state = make_havo_strip_state();
    state.italic_angle_deg = 12.0;
    let quads: Vec<KerningIndicatorQuad> = state.compute_kerning_indicator_quads();
    assert_eq!(quads.len(), 3);
    assert!(quads[0].kerning_value_upm < 0);
    assert!(quads[1].kerning_value_upm < 0);
    assert!(quads[2].kerning_value_upm > 0);
    assert_eq!(
        KERNING_NEGATIVE_FILL_LIGHT,
        RgbaColorToken::new(28, 158, 209, 51)
    );
    assert_eq!(
        KERNING_POSITIVE_FILL_LIGHT,
        RgbaColorToken::new(242, 219, 51, 51)
    );
    assert!(
        (quads[0].corners_upm[3].x.0 - quads[0].corners_upm[0].x.0).abs() > 1.0,
        "Italic angle must shear kerning wedge from descender to ascender"
    );
}

// TC-V050-014 & TC-V050-015: Text-Mode Measurement Line (default 200 UPM) & threshold (100).
#[test]
fn tc_v050_014_and_015_text_measurement_line_and_numbers_threshold() {
    assert_relative_eq!(TEXT_MEASUREMENT_LINE_DEFAULT_Y_UPM, 200.0, epsilon = 1e-6);
    assert_relative_eq!(TEXT_MODE_NUMBERS_THRESHOLD_DEFAULT, 100.0, epsilon = 1e-6);
    let mut state = make_havo_strip_state();
    state.show_text_measurement_line = true;
    let scans: Vec<SlotScanlineMeasurement> = state.compute_text_scanline_measurements();
    assert_eq!(scans.len(), 4);
    assert_relative_eq!(scans[1].measured_lsb_upm.unwrap(), 40.0, epsilon = 1e-4);
    assert_relative_eq!(scans[1].measured_rsb_upm.unwrap(), 40.0, epsilon = 1e-4);

    state.viewport.zoom = 0.05;
    assert!(!state.should_draw_text_mode_numbers());
    state.viewport.zoom = 0.25;
    assert!(state.should_draw_text_mode_numbers());
}

// TC-V050-016 & TC-V050-017: Repeated slot sync ("NON") & Headless Light/Dark rendering parity.
#[test]
fn tc_v050_016_and_017_repeated_slot_sync_and_headless_rendering() {
    let mut state = GrafiaCanvasState::init_glyph_a();
    let slots = vec![
        GlyphSlot::new(make_stem_glyph("N", 50.0, 550.0, 600.0), 0),
        GlyphSlot::new(make_stem_glyph("O", 50.0, 600.0, 650.0), -25),
        GlyphSlot::new(make_stem_glyph("N", 50.0, 550.0, 600.0), -25),
    ];
    state.set_text_stream_slots(slots, 0);
    state.active_tool = CanvasTool::Text;
    dispatch_key(&mut state, Key::ArrowRight, true, false, true, false);
    assert_relative_eq!(
        state.text_stream.slots[0].glyph.width,
        610.0,
        epsilon = 1e-6
    );
    assert_relative_eq!(
        state.text_stream.slots[2].glyph.width,
        610.0,
        epsilon = 1e-6
    );

    state.show_text_measurement_line = true;
    state.set_appearance_mode(AppearanceMode::Light);
    let light_px = rasterize_canvas_subregion(&state, &GrafiaTheme::sonoma_default(), 320, 180);
    state.set_appearance_mode(AppearanceMode::Dark);
    let dark_px = rasterize_canvas_subregion(&state, &GrafiaTheme::sonoma_dark(), 320, 180);
    assert_ne!(light_px.data(), dark_px.data());

    let out_dir = std::path::Path::new(
        r"C:\Users\jhove\.gemini\antigravity-cli\brain\fccb9142-9217-4702-a2ca-95303b541ee0",
    );
    if out_dir.exists() {
        let mut s54 = GrafiaCanvasState::init_glyph_a();
        s54.load_demo_text_strip();
        s54.active_tool = CanvasTool::Text;
        s54.text_stream.selection_range = Some((1, 3));
        s54.text_stream.caret_index = 2;
        s54.viewport.zoom = 0.21;
        s54.viewport.pan_offset = ScreenPoint::new(300.0, 315.0);
        let img54 =
            pixmap_to_rgba_image(rasterize_full_window(&s54, &GrafiaTheme::sonoma_default()));
        let _ = img54.save(out_dir.join("frame_54_v050_text_stream_and_kerning.png"));

        let mut s55 = GrafiaCanvasState::init_glyph_a();
        s55.load_demo_text_strip();
        s55.switch_active_slot(2);
        s55.active_tool = CanvasTool::Select;
        s55.show_text_measurement_line = true;
        s55.set_text_measurement_y(240.0);
        s55.italic_angle_deg = 8.0;
        s55.viewport.zoom = 0.21;
        s55.viewport.pan_offset = ScreenPoint::new(300.0, 315.0);
        s55.set_appearance_mode(AppearanceMode::Dark);
        let img55 = pixmap_to_rgba_image(rasterize_full_window(&s55, &GrafiaTheme::sonoma_dark()));
        let _ = img55.save(out_dir.join("frame_55_v050_text_measurement_scanline.png"));
    }
}

// Property-based test for cumulative slot origin invariant under arbitrary widths and kerning.
proptest! {
    #[test]
    fn prop_v050_cumulative_slot_origins(
        widths in prop::collection::vec(200.0f64..900.0f64, 2..8),
        kerns in prop::collection::vec(-150i32..150i32, 2..8),
    ) {
        let n = widths.len().min(kerns.len());
        let mut state = GrafiaCanvasState::init_glyph_a();
        let slots: Vec<GlyphSlot> = (0..n)
            .map(|i| GlyphSlot::new(make_stem_glyph("G", 20.0, widths[i] - 20.0, widths[i]), if i == 0 { 0 } else { kerns[i] }))
            .collect();
        state.set_text_stream_slots(slots, 0);
        let mut acc = 0.0f64;
        for i in 0..n {
            if i > 0 {
                acc += widths[i - 1] + f64::from(kerns[i]);
            }
            prop_assert!((state.text_stream.slots[i].origin_x_upm - acc).abs() < 1e-6);
        }
    }
}

// TC-V050-018: Stateful pull-based `arbtest` multi-glyph spacing, kerning & editing fuzzing.
#[test]
fn tc_v050_018_arbtest_stateful_multi_glyph_text_stream_and_kerning_chains() {
    arbtest(|u| {
        let mut state = make_havo_strip_state();
        let steps: usize = u.int_in_range(12..=25)?;
        for _ in 0..steps {
            match u.int_in_range(0..=8)? {
                0 => {
                    let shift: bool = u.arbitrary()?;
                    dispatch_key(&mut state, Key::ArrowRight, shift, false, false, true);
                }
                1 => {
                    let shift: bool = u.arbitrary()?;
                    dispatch_key(&mut state, Key::ArrowLeft, shift, false, true, false);
                }
                2 => {
                    let shift: bool = u.arbitrary()?;
                    dispatch_key(&mut state, Key::ArrowRight, shift, false, true, true);
                }
                3 => {
                    let shift: bool = u.arbitrary()?;
                    dispatch_key(&mut state, Key::ArrowLeft, shift, true, false, true);
                }
                4 => {
                    let shift: bool = u.arbitrary()?;
                    dispatch_key(&mut state, Key::ArrowRight, shift, true, true, false);
                }
                5 => {
                    let idx = u.int_in_range(0..=state.text_stream.slots.len() - 1)?;
                    state.switch_active_slot(idx);
                }
                6 => {
                    let y = f64::from(u.int_in_range(-100..=700)?);
                    state.set_text_measurement_y(y);
                }
                7 => {
                    let _ = state.undo();
                }
                _ => {
                    let _ = state.redo();
                }
            }
            state.validate_invariants().unwrap();
        }
        Ok(())
    })
    .budget_ms(250);
}

// TC-V050-019: Live character typing (`Event::Text`) & deletion (`Backspace`/`Delete`) at the caret via `CanvasHostRequest::EditTextStream`.
#[test]
fn tc_v050_019_live_text_typing_and_backspace_delete_events() {
    let mut state = make_havo_strip_state();
    state.activate_tool(CanvasTool::Select);
    state.set_caret_index(2);
    let _ = state.drain_host_requests();

    let ctx = egui::Context::default();
    let theme = state.theme.clone();
    // Pressing Key::T + Event::Text("t") in the same frame must activate Text tool without typing "t".
    let mut out_t = ctx.run_ui(
        RawInput {
            screen_rect: Some(default_canvas_rect()),
            events: vec![
                Event::Key {
                    key: Key::T,
                    physical_key: Some(Key::T),
                    pressed: true,
                    repeat: false,
                    modifiers: EguiModifiers::NONE,
                },
                Event::Text("t".to_string()),
            ],
            ..Default::default()
        },
        |ui| {
            let ev = ui.input(|i| i.events.clone());
            let mut disp = InputDispatcher::new(&mut state, &theme, default_canvas_rect());
            let _ = disp.dispatch_events(&ev, ui);
        },
    );
    out_t.textures_delta.clear();
    assert_eq!(state.active_tool, CanvasTool::Text);
    assert_eq!(state.text_stream.slots.len(), 4);
    assert!(state.drain_host_requests().is_empty());

    let mut out = ctx.run_ui(
        RawInput {
            screen_rect: Some(default_canvas_rect()),
            events: vec![Event::Text("X".to_string())],
            ..Default::default()
        },
        |ui| {
            let ev = ui.input(|i| i.events.clone());
            let mut disp = InputDispatcher::new(&mut state, &theme, default_canvas_rect());
            let _ = disp.dispatch_events(&ev, ui);
        },
    );
    out.textures_delta.clear();

    // `grafia` must NOT synthesize a dummy glyph internally; it emits `CanvasHostRequest::EditTextStream`!
    assert_eq!(state.text_stream.slots.len(), 4);
    assert_eq!(
        state.drain_host_requests(),
        vec![CanvasHostRequest::EditTextStream(TextStreamEditRequest {
            caret_index: 2,
            selection_range: None,
            edit: TextStreamEditKind::InsertText("X".to_string()),
        })]
    );

    // Host resolves "X" and updates the stream slots with caret and Undo support:
    let mut slots_after_x = state.text_stream.slots.clone();
    slots_after_x.insert(
        2,
        GlyphSlot::new(make_stem_glyph("X", 45.0, 555.0, 600.0), 0),
    );
    state.set_text_stream_slots_with_caret(slots_after_x, 2, 3);
    assert_eq!(state.text_stream.slots.len(), 5);
    assert_eq!(state.text_stream.slots[2].glyph.name, "X");
    assert_eq!(state.text_stream.caret_index, 3);

    // Replace selection range (1..3) by typing "O", then Backspace once and Delete once.
    state.text_stream.selection_range = Some((1, 3));
    state.insert_text_at_caret("O");
    assert_eq!(
        state.drain_host_requests(),
        vec![CanvasHostRequest::EditTextStream(TextStreamEditRequest {
            caret_index: 3,
            selection_range: Some((1, 3)),
            edit: TextStreamEditKind::InsertText("O".to_string()),
        })]
    );
    let mut slots_after_o = state.text_stream.slots.clone();
    slots_after_o.splice(
        1..3,
        [GlyphSlot::new(make_stem_glyph("O", 50.0, 600.0, 650.0), 0)],
    );
    state.set_text_stream_slots_with_caret(slots_after_o, 1, 2);
    assert_eq!(state.text_stream.slots.len(), 4);
    assert_eq!(state.text_stream.slots[1].glyph.name, "O");

    dispatch_key(&mut state, Key::Backspace, false, false, false, false);
    assert_eq!(
        state.drain_host_requests(),
        vec![CanvasHostRequest::EditTextStream(TextStreamEditRequest {
            caret_index: 2,
            selection_range: None,
            edit: TextStreamEditKind::Backspace,
        })]
    );
    let mut slots_after_bs = state.text_stream.slots.clone();
    slots_after_bs.remove(1);
    state.set_text_stream_slots_with_caret(slots_after_bs, 1, 1);
    assert_eq!(state.text_stream.slots.len(), 3);

    dispatch_key(&mut state, Key::Delete, false, false, false, false);
    assert_eq!(
        state.drain_host_requests(),
        vec![CanvasHostRequest::EditTextStream(TextStreamEditRequest {
            caret_index: 1,
            selection_range: None,
            edit: TextStreamEditKind::DeleteForward,
        })]
    );
    let mut slots_after_del = state.text_stream.slots.clone();
    slots_after_del.remove(1);
    state.set_text_stream_slots_with_caret(slots_after_del, 1, 1);
    assert_eq!(state.text_stream.slots.len(), 2);

    assert!(state.undo());
    assert_eq!(state.text_stream.slots.len(), 3);
    assert!(state.redo());
    assert_eq!(state.text_stream.slots.len(), 2);
    state.assert_invariants();
}

// TC-V050-020: Interactive vertical mouse dragging of the Text-Mode Measurement Scanline.
#[test]
fn tc_v050_020_interactive_measurement_scanline_vertical_drag() {
    let mut state = make_havo_strip_state();
    state.show_text_measurement_line = true;
    state.set_text_measurement_y(200.0);
    state.activate_tool(CanvasTool::Select);

    let start_scr = state.viewport.to_screen(FontPoint::new(150.0, 200.0));
    let target_scr = state.viewport.to_screen(FontPoint::new(150.0, 340.0));
    let ctx = egui::Context::default();
    let theme = state.theme.clone();
    let mut out = ctx.run_ui(
        RawInput {
            screen_rect: Some(default_canvas_rect()),
            events: vec![
                Event::PointerButton {
                    pos: Pos2::new(start_scr.x.0, start_scr.y.0),
                    button: egui::PointerButton::Primary,
                    pressed: true,
                    modifiers: EguiModifiers::NONE,
                },
                Event::PointerMoved(Pos2::new(target_scr.x.0, target_scr.y.0)),
                Event::PointerButton {
                    pos: Pos2::new(target_scr.x.0, target_scr.y.0),
                    button: egui::PointerButton::Primary,
                    pressed: false,
                    modifiers: EguiModifiers::NONE,
                },
            ],
            ..Default::default()
        },
        |ui| {
            let ev = ui.input(|i| i.events.clone());
            let mut disp = InputDispatcher::new(&mut state, &theme, default_canvas_rect());
            let _ = disp.dispatch_events(&ev, ui);
        },
    );
    out.textures_delta.clear();

    assert!(!state.text_measurement_dragging);
    assert_relative_eq!(state.text_measurement_y_upm, 340.0, epsilon = 1e-3);
    state.assert_invariants();
}

// TC-V050-021: Windows modifier disambiguation (`ctrl && command && !mac_cmd` -> LSB; `Alt + Arrow` -> RSB).
#[test]
fn tc_v050_021_windows_ctrl_and_alt_modifier_ergonomics() {
    let mut state = make_havo_strip_state();
    state.activate_tool(CanvasTool::Text);
    let _ = state.drain_spacing_deltas();
    let lsb_0 = state.active_glyph_lsb();
    let rsb_0 = state.active_glyph_rsb();
    let width_0 = state.glyph.width;

    // Simulate Windows physical Ctrl +Shift + Right (`ctrl: true, command: true, mac_cmd: false`).
    let win_ctrl_shift = EguiModifiers {
        alt: false,
        ctrl: true,
        shift: true,
        mac_cmd: false,
        command: true,
    };
    let ctx = egui::Context::default();
    ctx.input_mut(|i| i.modifiers = win_ctrl_shift);
    let theme = state.theme.clone();
    let mut out = ctx.run_ui(
        RawInput {
            screen_rect: Some(default_canvas_rect()),
            events: vec![Event::Key {
                key: Key::ArrowRight,
                physical_key: Some(Key::ArrowRight),
                pressed: true,
                repeat: false,
                modifiers: win_ctrl_shift,
            }],
            ..Default::default()
        },
        |ui| {
            let ev = ui.input(|i| i.events.clone());
            let mut disp = InputDispatcher::new(&mut state, &theme, default_canvas_rect());
            let _ = disp.dispatch_events(&ev, ui);
        },
    );
    out.textures_delta.clear();

    // Must adjust LSB (+10 UPM) and advance_width (+10 UPM), NOT contour-shift with constant width!
    assert_relative_eq!(state.active_glyph_lsb(), lsb_0 + 10.0, epsilon = 1e-6);
    assert_relative_eq!(state.active_glyph_rsb(), rsb_0, epsilon = 1e-6);
    assert_relative_eq!(state.glyph.width, width_0 + 10.0, epsilon = 1e-6);

    // Now test Windows ergonomic fallback `Alt + Shift + ArrowRight` (`!ctrl && !cmd && alt`) -> RSB (+10 UPM).
    dispatch_key(&mut state, Key::ArrowRight, true, true, false, false);
    assert_relative_eq!(state.active_glyph_lsb(), lsb_0 + 10.0, epsilon = 1e-6);
    assert_relative_eq!(state.active_glyph_rsb(), rsb_0 + 10.0, epsilon = 1e-6);
    assert_relative_eq!(state.glyph.width, width_0 + 20.0, epsilon = 1e-6);
    state.assert_invariants();
}
