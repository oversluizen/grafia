//! Canonical Black-Box COTS Test Builders for `grafia` (`SPEC-014 §2.7`).
//! Eliminates copy-pasted contour setup and raw internal field poking across integration tests.

use grafia::{
    ContourId, Glyph, GlyphContour, GrafiaCanvasState, GrafiaCanvasWidget, GrafiaTheme, Node,
    NodeId, NodeType,
};

/// Creates a [`GrafiaCanvasState`] populated via the canonical public `load_glyph` API
/// with a single closed corner contour from `points_upm`.
#[must_use]
pub fn canvas_with_closed_contour(points_upm: &[(f64, f64)]) -> GrafiaCanvasState {
    let mut state = GrafiaCanvasState::new(GrafiaTheme::sonoma_default());
    let nodes: Vec<Node> = points_upm
        .iter()
        .enumerate()
        .map(|(i, &(x, y))| Node::new(NodeId((i as u64) + 1), x, y, NodeType::Corner))
        .collect();
    let mut contour = GlyphContour::new(ContourId(0), nodes);
    contour.closed = true;
    let glyph = Glyph {
        contours: vec![contour],
        ..Glyph::default()
    };
    let metrics = state.metrics.clone();
    state.load_glyph(glyph, metrics);
    state
}

/// Runs a single headless `egui::Context` frame rendering [`GrafiaCanvasWidget`] at `(width_dp, height_dp)`.
pub fn run_headless_canvas_frame(
    state: &mut GrafiaCanvasState,
    width_dp: f32,
    height_dp: f32,
) -> egui::FullOutput {
    let ctx = egui::Context::default();
    let rect = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::Vec2::new(width_dp, height_dp));
    let input = egui::RawInput {
        screen_rect: Some(rect),
        ..Default::default()
    };
    let theme = state.theme.clone();
    let mut out = ctx.run_ui(input, |ui| {
        GrafiaCanvasWidget::new(state, &theme)
            .with_size(egui::Vec2::new(width_dp, height_dp))
            .show(ui);
    });
    out.textures_delta.clear();
    out
}

/// Executes `mutate(state)` and verifies bit-exact `State_0 -> Actie -> State_1 -> Undo == State_0 -> Redo == State_1`
/// along with `validate_invariants().is_ok()` at every step.
pub fn assert_mutation_undo_redo_roundtrip(
    state: &mut GrafiaCanvasState,
    mutate: impl FnOnce(&mut GrafiaCanvasState),
) {
    assert!(state.validate_invariants().is_ok());
    let before = state.snapshot();
    mutate(state);
    assert!(state.validate_invariants().is_ok());
    let after = state.snapshot();
    assert_ne!(
        before, after,
        "Mutation must produce a distinct state snapshot"
    );
    assert!(state.undo());
    assert!(state.validate_invariants().is_ok());
    assert_eq!(state.snapshot(), before);
    assert!(state.redo());
    assert!(state.validate_invariants().is_ok());
    assert_eq!(state.snapshot(), after);
}
