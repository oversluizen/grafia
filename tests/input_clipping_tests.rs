//! Test suite for Input Clipping and Pointer Isolation (TC-GLIFO-V01-02).
//!
//! Asserts that clicks outside the canvas rectangle (e.g. on toolbar or window chrome)
//! do not add points, mutate the active glyph, or leak events into the canvas tool state machine.

use egui::{vec2, Context, Event, PointerButton, Pos2, RawInput, Rect};
use grafia::adapters::{CanvasTool, GlifoCanvasState, GlifoCanvasWidget};
use grafia::tokens::GlifoTheme;

fn total_node_count(state: &GlifoCanvasState) -> usize {
    state.contours.iter().map(|c| c.len()).sum()
}

#[test]
fn test_pointer_down_outside_canvas_does_not_add_node_in_pen_tool() {
    let ctx = Context::default();
    let theme = GlifoTheme::sonoma_default();
    let mut state = GlifoCanvasState::init_glyph_a();
    state.active_tool = CanvasTool::Pen;

    let initial_node_count = total_node_count(&state);

    // Run frame with a top toolbar area (0..40 Y) and canvas below (40..600 Y)
    let mut raw_input = RawInput {
        screen_rect: Some(Rect::from_min_size(Pos2::ZERO, vec2(800.0, 600.0))),
        ..Default::default()
    };
    // Click at (25.0, 20.0), which is in the toolbar area outside the canvas
    raw_input.events.push(Event::PointerButton {
        pos: Pos2::new(25.0, 20.0),
        button: PointerButton::Primary,
        pressed: true,
        modifiers: egui::Modifiers::NONE,
    });
    raw_input.events.push(Event::PointerButton {
        pos: Pos2::new(25.0, 20.0),
        button: PointerButton::Primary,
        pressed: false,
        modifiers: egui::Modifiers::NONE,
    });

    let mut full_output = ctx.run_ui(raw_input, |ui| {
        // Toolbar consumes the top 40 dp
        let (toolbar_rect, _) = ui.allocate_exact_size(vec2(800.0, 40.0), egui::Sense::click());
        ui.painter().rect_filled(toolbar_rect, 0.0, egui::Color32::DARK_GRAY);

        // Canvas in the remaining area
        GlifoCanvasWidget::new(&mut state, &theme).show(ui);
    });
    full_output.textures_delta.clear();

    let node_count_after = total_node_count(&state);
    assert_eq!(
        node_count_after, initial_node_count,
        "Clicking outside the canvas rectangle (in the toolbar) must NEVER add a node to the glyph!"
    );
}

#[test]
fn test_click_outside_canvas_does_not_clear_selection() {
    let ctx = Context::default();
    let theme = GlifoTheme::sonoma_default();
    let mut state = GlifoCanvasState::init_glyph_a();
    state.active_tool = CanvasTool::Select;

    // Select the first node
    if let Some(first_contour) = state.contours.first() {
        if let Some(first_node) = first_contour.first() {
            state.selected_nodes = vec![first_node.id];
        }
    }
    assert!(!state.selected_nodes.is_empty());

    // Click outside canvas on toolbar
    let mut raw_input = RawInput {
        screen_rect: Some(Rect::from_min_size(Pos2::ZERO, vec2(800.0, 600.0))),
        ..Default::default()
    };
    raw_input.events.push(Event::PointerButton {
        pos: Pos2::new(10.0, 10.0),
        button: PointerButton::Primary,
        pressed: true,
        modifiers: egui::Modifiers::NONE,
    });

    let mut full_output = ctx.run_ui(raw_input, |ui| {
        let _ = ui.allocate_exact_size(vec2(800.0, 40.0), egui::Sense::click());
        GlifoCanvasWidget::new(&mut state, &theme).show(ui);
    });
    full_output.textures_delta.clear();

    assert!(
        !state.selected_nodes.is_empty(),
        "Clicking outside the canvas must NOT clear the canvas selection!"
    );
}
