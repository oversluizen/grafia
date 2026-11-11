//! Integration tests for grafia v0.1.0-alpha baseline.

use approx::assert_relative_eq;
use grafia::core::{
    align_collinear_opposite, compute_signed_area, snap_vector_45_degrees, FontPoint, NodeType,
    ScreenPoint, ViewportTransform,
};
use grafia::interaction::{CommandHistory, GlyphSnapshot, PenToolState};
use grafia::render::{PreviewFilledProjection, ProjectedNode, ShapeThumbnailProjection};
use grafia::tokens::GrafiaTheme;
use kurbo::{BezPath, Shape};
use rstest::rstest;
use std::sync::Arc;

#[test]
fn test_01_tokens_loaded_from_json() {
    let theme = GrafiaTheme::sonoma_default();
    assert_eq!(theme.edit_canvas.node_size_dp, 7.0);
    assert_eq!(theme.shape_canvas.row_height_dp, 20.0);
    assert_eq!(theme.shape_canvas.thumbnail_width_dp, 24.0);
    assert_eq!(theme.shape_canvas.thumbnail_height_dp, 16.0);
    assert_eq!(theme.snapping.angle_constraint_step_deg, 45.0);
}

#[test]
fn test_02_coordinate_roundtrip() {
    let vp = ViewportTransform::new(0.5, ScreenPoint::new(400.0, 500.0), 1000);
    let original = FontPoint::new(250.0, 450.0);
    let screen = vp.to_screen(original);
    let back = vp.to_font(screen);

    assert_relative_eq!(original.x.0, back.x.0, epsilon = 1e-4);
    assert_relative_eq!(original.y.0, back.y.0, epsilon = 1e-4);
}

#[test]
fn test_03_edit_canvas_projected_node_hit_test() {
    let theme = GrafiaTheme::sonoma_default();
    let vp = ViewportTransform::new(1.0, ScreenPoint::new(0.0, 1000.0), 1000);
    let font_pt = FontPoint::new(100.0, 200.0);

    let projected = ProjectedNode::new(font_pt, NodeType::Corner, false, &vp, &theme.edit_canvas);
    assert_eq!(projected.node_type, NodeType::Corner);

    // Hit test directly at center and 2dp away
    assert!(projected.contains_screen_pt(projected.screen_pos));
    let near_pt = ScreenPoint::new(projected.screen_pos.x.0 + 2.0, projected.screen_pos.y.0);
    assert!(projected.contains_screen_pt(near_pt));

    // Outside bounds
    let far_pt = ScreenPoint::new(projected.screen_pos.x.0 + 10.0, projected.screen_pos.y.0);
    assert!(!projected.contains_screen_pt(far_pt));
}

#[test]
fn test_04_preview_canvas_projection() {
    let vp = ViewportTransform::new(0.5, ScreenPoint::new(0.0, 500.0), 1000);
    let mut font_path = BezPath::new();
    font_path.move_to((0.0, 0.0));
    font_path.line_to((100.0, 200.0));

    let preview = PreviewFilledProjection::compute(&font_path, &vp);
    assert_relative_eq!(preview.baseline_y_screen, 500.0);
    assert!(!preview.screen_path.is_empty());
}

#[test]
fn test_05_shape_canvas_thumbnail_fit_24x16() {
    let theme = GrafiaTheme::sonoma_default();
    let mut path = BezPath::new();
    path.move_to((100.0, 100.0));
    path.line_to((500.0, 100.0));
    path.line_to((500.0, 700.0));
    path.close_path();

    let thumbnail = ShapeThumbnailProjection::compute(&path, &theme.shape_canvas, 3, true);
    let bbox = thumbnail.fitted_path.bounding_box();

    assert!(bbox.width() <= 20.01);
    assert!(bbox.height() <= 12.01);
    assert!(thumbnail.is_clockwise);
    assert_eq!(thumbnail.node_count, 3);
}

#[rstest]
#[case(100.0, 10.0, 100.0, 0.0)]
#[case(10.0, 100.0, 0.0, 100.0)]
#[case(95.0, 102.0, 99.0, 99.0)]
fn test_06_angle_snapping_45(
    #[case] raw_dx: f64,
    #[case] raw_dy: f64,
    #[case] exp_dx: f64,
    #[case] exp_dy: f64,
) {
    let (dx, dy) = snap_vector_45_degrees(raw_dx, raw_dy);
    assert_relative_eq!(dx, exp_dx, epsilon = 2.0);
    assert_relative_eq!(dy, exp_dy, epsilon = 2.0);
}

#[test]
fn test_07_collinear_opposite_lock() {
    let anchor = FontPoint::new(100.0, 100.0);
    let bcp_in = FontPoint::new(80.0, 100.0);
    let bcp_out = FontPoint::new(130.0, 100.0);

    let aligned_in = align_collinear_opposite(anchor, bcp_in, bcp_out);
    assert_relative_eq!(aligned_in.x.0, 80.0, epsilon = 1e-4);
    assert_relative_eq!(aligned_in.y.0, 100.0, epsilon = 1e-4);
}

#[test]
fn test_08_shoelace_winding_area() {
    // Triangle (0,0) -> (100,200) -> (200,0) in font space (Y-up)
    let pts = vec![
        FontPoint::new(0.0, 0.0),
        FontPoint::new(100.0, 200.0),
        FontPoint::new(200.0, 0.0),
    ];
    let area = compute_signed_area(&pts);
    assert!(
        area < 0.0,
        "Clockwise triangle must have negative signed area in Y-up space"
    );
}

#[test]
fn test_09_pen_tool_symmetric_handles() {
    let anchor = FontPoint::new(200.0, 200.0);
    let state = PenToolState::compute_symmetric_handles(anchor, (30.0, 40.0), false);

    if let PenToolState::DraggingHandles {
        anchor: a,
        bcp_out,
        bcp_in,
        is_closing,
    } = state
    {
        assert_relative_eq!(a.x.0, 200.0);
        assert_relative_eq!(bcp_out.x.0, 230.0);
        assert_relative_eq!(bcp_out.y.0, 240.0);
        assert_relative_eq!(bcp_in.x.0, 170.0);
        assert_relative_eq!(bcp_in.y.0, 160.0);
        assert!(!is_closing);
    } else {
        panic!("Expected DraggingHandles state");
    }
}

#[test]
fn test_10_history_undo_redo_cow() {
    let mut history = CommandHistory::new();
    let initial_paths = Arc::new([BezPath::new()]);
    let mut current = GlyphSnapshot {
        paths: initial_paths.clone(),
        ..GlyphSnapshot::default()
    };

    let mut modified_path = BezPath::new();
    modified_path.move_to((10.0, 20.0));
    let modified_snapshot = GlyphSnapshot {
        paths: Arc::new([modified_path]),
        ..GlyphSnapshot::default()
    };

    history.record("Move", current.clone(), modified_snapshot.clone());
    current = modified_snapshot;

    assert!(history.can_undo());
    assert!(history.undo(&mut current));
    assert_eq!(current.paths.len(), 1);
    assert!(current.paths[0].is_empty()); // reverted to initial

    assert!(history.can_redo());
    assert!(history.redo(&mut current));
    assert_eq!(current.paths[0].elements().len(), 1); // reapplied
}

fn assert_mutation_undo_redo_roundtrip<F>(
    state: &mut grafia::adapters::GrafiaCanvasState,
    label: &str,
    mutate: F,
) where
    F: FnOnce(&mut grafia::adapters::GrafiaCanvasState),
{
    state
        .validate_invariants()
        .unwrap_or_else(|e| panic!("[{label}] Pre-mutation invariant failed: {e}"));
    state.assert_invariants();
    let snap_0 = state.snapshot();

    mutate(state);

    state
        .validate_invariants()
        .unwrap_or_else(|e| panic!("[{label}] Post-mutation invariant failed: {e}"));
    state.assert_invariants();
    let snap_1 = state.snapshot();
    assert_ne!(
        snap_0, snap_1,
        "[{label}] Mutation must produce a distinct state (State_1 != State_0)"
    );

    assert!(state.undo(), "[{label}] Undo must succeed");
    state
        .validate_invariants()
        .unwrap_or_else(|e| panic!("[{label}] Post-undo invariant failed: {e}"));
    state.assert_invariants();
    assert_eq!(
        state.snapshot(),
        snap_0,
        "[{label}] Undo must restore State_0 bit-exactly"
    );

    assert!(state.redo(), "[{label}] Redo must succeed");
    state
        .validate_invariants()
        .unwrap_or_else(|e| panic!("[{label}] Post-redo invariant failed: {e}"));
    state.assert_invariants();
    assert_eq!(
        state.snapshot(),
        snap_1,
        "[{label}] Redo must restore State_1 bit-exactly"
    );
}

#[test]
fn test_11_universal_action_undo_redo_bit_exact_roundtrip_and_invariants() {
    use grafia::adapters::GrafiaCanvasState;
    use grafia::core::{
        cut_open_delete_nodes_in_contour, heal_delete_nodes, split_contour_segment, ContourSegment,
        NodeId, SegmentHit, ShapeBooleanRole,
    };

    let mut state = GrafiaCanvasState::init_glyph_a();
    // Set non-default contour metadata on counter contour to prove metadata roundtrip preservation
    state.glyph.contours[1].role = ShapeBooleanRole::Mask;
    state.glyph.contours[1].visible = false;
    state
        .validate_invariants()
        .expect("Initial /A state invariants must hold");

    // 1. Reverse Contour roundtrip (including non-default contour role & visibility metadata)
    assert_mutation_undo_redo_roundtrip(&mut state, "Reverse Contour", |s| {
        assert!(s.reverse_selected_or_all_contours());
    });
    assert_eq!(state.glyph.contours[1].role, ShapeBooleanRole::Mask);
    assert!(!state.glyph.contours[1].visible);

    // 2. Make Selected Node First roundtrip (direct method call without history tampering)
    state.selected_nodes = vec![state.contours[0][2].id];
    assert_mutation_undo_redo_roundtrip(&mut state, "Make Node First", |s| {
        assert!(s.make_selected_node_first());
    });

    // 3. Toggle Smooth/Corner roundtrip on /O
    let mut state_o = GrafiaCanvasState::init_glyph_o();
    let target_node_id = state_o.contours[0][0].id;
    assert_mutation_undo_redo_roundtrip(&mut state_o, "Toggle Smooth/Corner", |s| {
        s.toggle_node_smooth_corner(target_node_id);
    });

    // 4. Node Drag Displacement roundtrip
    state_o.selected_nodes = vec![state_o.contours[0][0].id];
    assert_mutation_undo_redo_roundtrip(&mut state_o, "Drag Node Displacement", |s| {
        let before = s.snapshot();
        s.record_drag_start();
        s.apply_node_displacement(25.0, -15.0, false);
        s.record_mutation("Drag Edit", before);
    });

    // 5. Split Segment roundtrip
    assert_mutation_undo_redo_roundtrip(&mut state_o, "Split Cubic Segment", |s| {
        let before = s.snapshot();
        let closed = s.is_contour_closed(0);
        let seg = grafia::core::extract_segments(&s.contours[0], closed)[0];
        let new_id = split_contour_segment(&mut s.contours[0], seg, 0.5, &mut s.next_node_id);
        s.selected_nodes = vec![new_id];
        s.sync_paths_from_contours();
        s.record_mutation("Add Node on Segment", before);
    });

    // 6. Heal Delete Node roundtrip
    state_o.selected_nodes = vec![state_o.contours[0][0].id];
    assert_mutation_undo_redo_roundtrip(&mut state_o, "Heal Delete Node", |s| {
        let before = s.snapshot();
        let del_id = s.selected_nodes[0];
        heal_delete_nodes(&mut s.contours[0], true, &[del_id], &mut s.next_node_id);
        s.selected_nodes.clear();
        s.sync_paths_from_contours();
        s.record_mutation("Delete Nodes", before);
    });

    // 7. Cut Contour Open roundtrip
    state_o.selected_nodes = vec![state_o.contours[0][0].id];
    assert_mutation_undo_redo_roundtrip(&mut state_o, "Cut Contour Open", |s| {
        let before = s.snapshot();
        let cut_id = s.selected_nodes[0];
        cut_open_delete_nodes_in_contour(&mut s.contours[0], true, &[cut_id]);
        s.set_contour_closed(0, false);
        s.active_open_contour = Some(0);
        s.selected_nodes.clear();
        s.sync_paths_from_contours();
        s.record_mutation("Cut Contour Open", before);
    });

    // 8. Negative invariant checks: verify validate_invariants() catches desyncs, duplicates, and dangling IDs
    let mut broken = GrafiaCanvasState::init_glyph_a();
    broken.selected_nodes.push(NodeId(999_999));
    assert!(
        broken.validate_invariants().is_err(),
        "validate_invariants must reject dangling NodeId in selected_nodes"
    );

    let mut broken_dup_sel = GrafiaCanvasState::init_glyph_a();
    let valid_id = broken_dup_sel.contours[0][0].id;
    broken_dup_sel.selected_nodes = vec![valid_id, valid_id];
    assert!(
        broken_dup_sel.validate_invariants().is_err(),
        "validate_invariants must reject duplicate NodeId in selected_nodes"
    );

    let mut broken_dup_node = GrafiaCanvasState::init_glyph_a();
    broken_dup_node.contours[1][0].id = broken_dup_node.contours[0][0].id;
    broken_dup_node.glyph.contours[1].nodes[0].id = broken_dup_node.contours[0][0].id;
    assert!(
        broken_dup_node.validate_invariants().is_err(),
        "validate_invariants must reject duplicate NodeId across contours"
    );

    let mut broken_paths = GrafiaCanvasState::init_glyph_a();
    broken_paths.paths.pop();
    assert!(
        broken_paths.validate_invariants().is_err(),
        "validate_invariants must reject contours.len() != paths.len()"
    );

    let mut broken_open = GrafiaCanvasState::init_glyph_a();
    broken_open.active_open_contour = Some(99);
    assert!(
        broken_open.validate_invariants().is_err(),
        "validate_invariants must reject out-of-bounds active_open_contour"
    );

    let mut broken_hover = GrafiaCanvasState::init_glyph_a();
    broken_hover.hovered_segment_hit = Some(SegmentHit {
        contour_idx: 99,
        segment: ContourSegment::Line {
            start_idx: 0,
            end_idx: 1,
            line: kurbo::Line::new((0.0, 0.0), (10.0, 10.0)),
        },
        t: 0.5,
        point: FontPoint::new(5.0, 5.0),
        distance_font: 0.0,
    });
    assert!(
        broken_hover.validate_invariants().is_err(),
        "validate_invariants must reject out-of-bounds hovered_segment_hit"
    );

    // 8b. Single-Point / Zero-OnCurve Contour Invariant & Open-Contour Handle Integrity
    let mut broken_single_closed = GrafiaCanvasState::init_glyph_a();
    broken_single_closed.contours[0].truncate(1);
    broken_single_closed.glyph.contours[0].nodes.truncate(1);
    broken_single_closed.glyph.contours[0].closed = true;
    assert!(
        broken_single_closed.validate_invariants().is_err(),
        "validate_invariants must reject closed contour with <= 1 OnCurve node"
    );

    let mut broken_open_closed_flag = GrafiaCanvasState::init_glyph_a();
    broken_open_closed_flag.active_open_contour = Some(0);
    assert!(
        broken_open_closed_flag.validate_invariants().is_err(),
        "validate_invariants must reject active_open_contour pointing to a closed contour"
    );

    let mut broken_open_handles = GrafiaCanvasState::init_glyph_o();
    broken_open_handles.set_contour_closed(0, false);
    broken_open_handles.contours[0].remove(1); // Remove one OffCurve leaving a run of 1 OffCurve
    broken_open_handles.sync_paths_from_contours();
    assert!(
        broken_open_handles.validate_invariants().is_err(),
        "validate_invariants must reject open contour with interior OffCurve run length != 0 and != 2"
    );
}

#[test]
fn test_12_cold_start_smoke_and_zero_idle_repaint_leak() {
    use egui::{Context, RawInput};
    use grafia::adapters::{CanvasViewMode, GrafiaCanvasState, GrafiaCanvasWidget};
    use std::time::{Duration, Instant};

    let cold_start_begin = Instant::now();
    let theme = GrafiaTheme::sonoma_default();
    let mut state = GrafiaCanvasState::init_glyph_a();
    state
        .validate_invariants()
        .expect("Cold-start state invariants must hold");

    let ctx = Context::default();
    let mut first_output = ctx.run_ui(RawInput::default(), |ui| {
        let _ = GrafiaCanvasWidget::new(&mut state, &theme).show(ui);
    });
    first_output.textures_delta.clear();
    let primitives = ctx.tessellate(first_output.shapes, first_output.pixels_per_point);
    let cold_start_elapsed = cold_start_begin.elapsed();

    assert!(
        !primitives.is_empty(),
        "Cold-start Frame 1 must produce non-empty tessellated primitives"
    );
    assert!(
        cold_start_elapsed < Duration::from_millis(500),
        "STD-ARCH §3.3 Cold-Start Smoke Test failed: init + Frame 1 took {:?} (limit < 500ms)",
        cold_start_elapsed
    );

    // Verify 0% idle repaint leak after 2 warmup frames across all view modes (STD-ARCH §3.4)
    for mode in [
        CanvasViewMode::EditWireframe,
        CanvasViewMode::PreviewFilled,
        CanvasViewMode::ShapeThumbnail,
    ] {
        state.view_mode = mode;
        for frame_idx in 0..3 {
            let input = RawInput {
                time: Some(1.0 + f64::from(frame_idx) * 0.016),
                ..Default::default()
            };
            let mut out = ctx.run_ui(input, |ui| {
                let _ = GrafiaCanvasWidget::new(&mut state, &theme).show(ui);
            });
            out.textures_delta.clear();
            if frame_idx == 2 {
                let repaint_delay = out.viewport_output[&egui::ViewportId::ROOT].repaint_delay;
                assert!(
                    repaint_delay > Duration::ZERO,
                    "STD-ARCH §3.4 Idle Repaint Leak in {mode:?}: expected repaint_delay > 0, got {repaint_delay:?}"
                );
            }
        }
    }
}
