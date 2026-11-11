//! Test suite for Keyboard Shortcuts and Modifiers (TC-GLIFO-V01-03..05, TC-1227, TC-1234, TC-1236).
//!
//! Asserts:
//! 1. Spacebar temporary hold switches view_mode to `PreviewFilled`; release returns to `EditWireframe`.
//! 2. Arrow keys nudge selected nodes by 1.0 UPM (normal) and 10.0 UPM (with Shift).
//! 3. Escape key clears selection.

use egui::{Context, Event, Key, Modifiers, RawInput};
use grafia::adapters::{CanvasViewMode, GlifoCanvasState, GlifoCanvasWidget};
use grafia::tokens::GlifoTheme;

fn frame(ctx: &Context, state: &mut GlifoCanvasState, theme: &GlifoTheme, raw_input: RawInput) {
    let mut full_output = ctx.run_ui(raw_input, |ui| {
        GlifoCanvasWidget::new(state, theme).show(ui);
    });
    full_output.textures_delta.clear();
}

#[test]
fn test_spacebar_hold_and_release_preview_toggle() {
    let ctx = Context::default();
    let theme = GlifoTheme::sonoma_default();
    let mut state = GlifoCanvasState::init_glyph_a();
    state.view_mode = CanvasViewMode::EditWireframe;

    // 1. Initial state is EditWireframe
    assert_eq!(state.view_mode, CanvasViewMode::EditWireframe);

    // 2. Press Spacebar down
    let mut input = RawInput::default();
    input.events.push(Event::Key {
        key: Key::Space,
        physical_key: None,
        pressed: true,
        repeat: false,
        modifiers: Modifiers::NONE,
    });
    frame(&ctx, &mut state, &theme, input);
    assert_eq!(
        state.view_mode,
        CanvasViewMode::PreviewFilled,
        "Holding Spacebar must temporarily switch view_mode to PreviewFilled"
    );

    // 3. Release Spacebar
    let mut input = RawInput::default();
    input.events.push(Event::Key {
        key: Key::Space,
        physical_key: None,
        pressed: false,
        repeat: false,
        modifiers: Modifiers::NONE,
    });
    frame(&ctx, &mut state, &theme, input);
    assert_eq!(
        state.view_mode,
        CanvasViewMode::EditWireframe,
        "Releasing Spacebar must restore EditWireframe mode"
    );
}

#[test]
fn test_arrow_keys_nudge_selection_by_1_and_10_upm() {
    let ctx = Context::default();
    let theme = GlifoTheme::sonoma_default();
    let mut state = GlifoCanvasState::init_glyph_a();

    // Select the first node
    let first_node = state.contours[0][0].clone();
    state.selected_nodes = vec![first_node.id];
    let original_pt = first_node.point;

    // Press ArrowRight without modifiers -> +1.0 X
    let mut input = RawInput::default();
    input.events.push(Event::Key {
        key: Key::ArrowRight,
        physical_key: None,
        pressed: true,
        repeat: false,
        modifiers: Modifiers::NONE,
    });
    frame(&ctx, &mut state, &theme, input);

    let pt_after_right = state.contours[0][0].point;
    assert_eq!(
        pt_after_right.x.0,
        original_pt.x.0 + 1.0,
        "ArrowRight must nudge node by exactly +1.0 UPM"
    );
    assert_eq!(pt_after_right.y.0, original_pt.y.0);

    // Press Shift + ArrowUp -> +10.0 Y
    let mut input = RawInput::default();
    input.events.push(Event::Key {
        key: Key::ArrowUp,
        physical_key: None,
        pressed: true,
        repeat: false,
        modifiers: Modifiers::SHIFT,
    });
    frame(&ctx, &mut state, &theme, input);

    let pt_after_up = state.contours[0][0].point;
    assert_eq!(
        pt_after_up.y.0,
        original_pt.y.0 + 10.0,
        "Shift+ArrowUp must nudge node by exactly +10.0 UPM"
    );

    // Press Command + ArrowLeft -> -100.0 X
    let mut input = RawInput::default();
    input.events.push(Event::Key {
        key: Key::ArrowLeft,
        physical_key: None,
        pressed: true,
        repeat: false,
        modifiers: Modifiers::COMMAND,
    });
    frame(&ctx, &mut state, &theme, input);

    let pt_after_left = state.contours[0][0].point;
    assert_eq!(
        pt_after_left.x.0,
        pt_after_right.x.0 - 100.0,
        "Command+ArrowLeft must nudge node by exactly -100.0 UPM"
    );

    // Press ArrowDown -> -1.0 Y
    let mut input = RawInput::default();
    input.events.push(Event::Key {
        key: Key::ArrowDown,
        physical_key: None,
        pressed: true,
        repeat: false,
        modifiers: Modifiers::NONE,
    });
    frame(&ctx, &mut state, &theme, input);

    let pt_after_down = state.contours[0][0].point;
    assert_eq!(
        pt_after_down.y.0,
        pt_after_up.y.0 - 1.0,
        "ArrowDown must nudge node by exactly -1.0 UPM"
    );
}

#[test]
fn test_arrow_keys_with_empty_selection_does_not_mutate() {
    let ctx = Context::default();
    let theme = GlifoTheme::sonoma_default();
    let mut state = GlifoCanvasState::init_glyph_a();
    state.selected_nodes.clear();

    let original_pt = state.contours[0][0].point;

    let mut input = RawInput::default();
    input.events.push(Event::Key {
        key: Key::ArrowRight,
        physical_key: None,
        pressed: true,
        repeat: false,
        modifiers: Modifiers::NONE,
    });
    let mut full_output = ctx.run_ui(input, |ui| {
        let resp = GlifoCanvasWidget::new(&mut state, &theme).show(ui);
        assert!(!resp.changed, "Arrow key with empty selection must not report changed");
    });
    full_output.textures_delta.clear();

    assert_eq!(
        state.contours[0][0].point, original_pt,
        "Empty selection must not be nudged"
    );
}

#[test]
fn test_escape_key_clears_selection() {
    let ctx = Context::default();
    let theme = GlifoTheme::sonoma_default();
    let mut state = GlifoCanvasState::init_glyph_a();

    state.selected_nodes = vec![state.contours[0][0].id];
    assert!(!state.selected_nodes.is_empty());

    let mut input = RawInput::default();
    input.events.push(Event::Key {
        key: Key::Escape,
        physical_key: None,
        pressed: true,
        repeat: false,
        modifiers: Modifiers::NONE,
    });
    frame(&ctx, &mut state, &theme, input);

    assert!(
        state.selected_nodes.is_empty(),
        "Pressing Escape must clear all active selections"
    );
}
