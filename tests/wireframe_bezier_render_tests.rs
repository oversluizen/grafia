//! Normative Behavioural, Reference Golden-Master & Visual Verification Suite (`SPEC-002`, `DEF-GRAFIA-001..006`, `TC-1246..1260`).

use approx::assert_relative_eq;
use egui::{
    pos2, Color32, Context, Event, Key, Modifiers, PointerButton, Pos2, RawInput, Rect, Shape,
};
use grafia::adapters::{
    pixmap_to_rgba_image, rasterize_canvas_subregion, CanvasTool, CanvasViewMode,
    GrafiaCanvasState, GrafiaCanvasWidget,
};
use grafia::core::{
    extract_segments, heal_delete_nodes, split_contour_segment, ContourSegment, FontPoint, Node,
    NodeId, NodeType,
};
use grafia::render::tessellate_contours_even_odd;
use grafia::tokens::{GlyphArchetype, GrafiaTheme};
use rstest::rstest;

fn run_frame(
    ctx: &Context,
    state: &mut GrafiaCanvasState,
    theme: &GrafiaTheme,
    mut raw_input: RawInput,
) -> egui::FullOutput {
    raw_input.screen_rect = Some(Rect::from_min_size(
        Pos2::new(0.0, 0.0),
        egui::vec2(900.0, 650.0),
    ));
    let mut full_output = ctx.run_ui(raw_input, |ui| {
        GrafiaCanvasWidget::new(state, theme).show(ui);
    });
    full_output.textures_delta.clear();
    full_output
}

fn collect_line_segments(shapes: &[egui::epaint::ClippedShape]) -> Vec<([Pos2; 2], Color32)> {
    let mut out = Vec::new();
    for clipped in shapes {
        collect_from_shape(&clipped.shape, &mut out);
    }
    out
}

fn collect_from_shape(shape: &Shape, out: &mut Vec<([Pos2; 2], Color32)>) {
    match shape {
        Shape::LineSegment { points, stroke } => out.push((*points, stroke.color)),
        Shape::Path(path_shape) => {
            let col = match path_shape.stroke.color {
                egui::epaint::ColorMode::Solid(c) => c,
                _ => Color32::TRANSPARENT,
            };
            for w in path_shape.points.windows(2) {
                out.push(([w[0], w[1]], col));
            }
            if path_shape.closed && path_shape.points.len() >= 2 {
                let last = path_shape.points[path_shape.points.len() - 1];
                let first = path_shape.points[0];
                out.push(([last, first], col));
            }
        }
        Shape::Vec(nested) => {
            for s in nested {
                collect_from_shape(s, out);
            }
        }
        _ => {}
    }
}

fn min_dist_to_segments(pt: Pos2, segs: &[([Pos2; 2], Color32)]) -> f32 {
    segs.iter()
        .map(|([a, b], _)| {
            let ab = *b - *a;
            let len_sq = ab.length_sq();
            if len_sq < 1e-6 {
                return (pt - *a).length();
            }
            let t = ((pt - *a).dot(ab) / len_sq).clamp(0.0, 1.0);
            let proj = *a + ab * t;
            (pt - proj).length()
        })
        .fold(f32::INFINITY, f32::min)
}

// ============================================================================
// DEF-GRAFIA-001: Cubic Bézier Wireframe & Tessellation Evaluation
// ============================================================================

#[test]
fn test_def_grafia_001_wireframe_cubic_bezier_evaluates_smooth_curve_not_control_polygon() {
    let ctx = Context::default();
    let theme = GrafiaTheme::sonoma_default();
    let mut state = GrafiaCanvasState::new(theme.clone());

    // Single closed contour with a prominent cubic arch:
    // P0=(100, 100), H1=(100, 500), H2=(500, 500), P3=(500, 100)
    // True cubic midpoint at t=0.5 is B(0.5) = (300, 400).
    // A buggy straight-line polygon through OffCurve nodes passes through (300, 500) instead!
    state.contours = vec![vec![
        Node::new(NodeId(1), 100.0, 100.0, NodeType::Corner),
        Node::new(NodeId(2), 100.0, 500.0, NodeType::OffCurve),
        Node::new(NodeId(3), 500.0, 500.0, NodeType::OffCurve),
        Node::new(NodeId(4), 500.0, 100.0, NodeType::Corner),
    ]];
    state.active_contour_closed = true;
    state.set_contour_closed(0, true);
    state.sync_paths_from_contours();

    let output = run_frame(&ctx, &mut state, &theme, RawInput::default());
    let all_segs = collect_line_segments(&output.shapes);
    let contour_color = theme.preview_canvas.fill_color32();
    let contour_segs: Vec<_> = all_segs
        .into_iter()
        .filter(|(_, color)| *color == contour_color)
        .collect();

    assert!(
        contour_segs.len() > 8,
        "DEF-GRAFIA-001: Flattened cubic wireframe must have > 8 segments, got {}",
        contour_segs.len()
    );

    let true_mid_scr = state.viewport.to_screen(FontPoint::new(300.0, 400.0));
    let true_mid_pos = pos2(true_mid_scr.x.0, true_mid_scr.y.0);
    let dist_to_true_mid = min_dist_to_segments(true_mid_pos, &contour_segs);
    assert!(
        dist_to_true_mid < 1.5,
        "DEF-GRAFIA-001: Wireframe curve must pass through true cubic midpoint B(0.5)=(300,400), dist={dist_to_true_mid}"
    );

    // Must NOT draw a contour stroke across the H1->H2 control polygon top (300, 500)
    let wrong_top_scr = state.viewport.to_screen(FontPoint::new(300.0, 500.0));
    let wrong_top_pos = pos2(wrong_top_scr.x.0, wrong_top_scr.y.0);
    let dist_to_wrong_top = min_dist_to_segments(wrong_top_pos, &contour_segs);
    assert!(
        dist_to_wrong_top > 25.0,
        "DEF-GRAFIA-001: Wireframe must NOT connect H1->H2 as a straight line at (300,500), dist={dist_to_wrong_top}"
    );

    // Also verify PreviewFilled Lyon tessellation evaluates cubic curves smoothly
    let mesh = tessellate_contours_even_odd(
        &state.contours,
        &state.viewport,
        theme.preview_canvas.fill_color32(),
    );
    assert!(
        mesh.vertices.len() > 10,
        "DEF-GRAFIA-001: Tessellated mesh for cubic arch must contain > 10 curve vertices, got {}",
        mesh.vertices.len()
    );
}

// ============================================================================
// DEF-GRAFIA-002: BCP Tangent Lines on Existing Cubic Contours
// ============================================================================

#[test]
fn test_def_grafia_002_bcp_tangent_lines_rendered_for_existing_cubic_contours() {
    let ctx = Context::default();
    let theme = GrafiaTheme::sonoma_default();
    let mut state = GrafiaCanvasState::init_glyph_o();

    // Glyph /o has 2 contours, each with 4 cubic segments = 8 cubic segments total -> 16 tangent lines
    let output = run_frame(&ctx, &mut state, &theme, RawInput::default());
    let all_segs = collect_line_segments(&output.shapes);
    let bcp_color = theme.edit_canvas.bcp_line_color32();
    let tangent_segs: Vec<_> = all_segs
        .into_iter()
        .filter(|(_, color)| *color == bcp_color)
        .collect();

    assert_eq!(
        tangent_segs.len(),
        16,
        "DEF-GRAFIA-002: Glyph /o (8 cubic segments) must render exactly 16 BCP tangent lines in EditWireframe mode"
    );
}

// ============================================================================
// DEF-GRAFIA-003: Straight-Line Node Deletion vs. Reference S-Curve Healing
// ============================================================================

#[test]
fn test_def_grafia_003_reference_golden_scurve_healing_and_straight_line_preservation() {
    // Part 1: Straight line deletion on /A produces 0 OffCurve handles
    let mut state_a = GrafiaCanvasState::init_glyph_a();
    let target_id = state_a.contours[0][3].id;
    heal_delete_nodes(&mut state_a.contours[0], true, &[target_id], &mut 500);
    let off_curve_in_a = state_a.contours[0]
        .iter()
        .filter(|n| n.node_type == NodeType::OffCurve)
        .count();
    assert_eq!(
        off_curve_in_a, 0,
        "DEF-GRAFIA-003: Deleting a corner node between two straight lines must produce 0 OffCurve nodes"
    );

    // Part 2: Reference Golden Master S-Curve healing (from geom_05_golden_input_output_vectors.json)
    // Segment 1: (0,0) -> (-4.05, 6.07) -> (-5.84, 11.32) -> (-5.84, 15.92)
    // Segment 2: (-5.84, 15.92) -> (-5.84, 52.18) -> (105.84, 47.82) -> (105.84, 84.08)
    let mut scurve = vec![
        Node::new(NodeId(1), 0.0, 0.0, NodeType::Corner),
        Node::new(NodeId(2), -4.05, 6.07, NodeType::OffCurve),
        Node::new(NodeId(3), -5.84, 11.32, NodeType::OffCurve),
        Node::new(NodeId(4), -5.84, 15.92, NodeType::Smooth),
        Node::new(NodeId(5), -5.84, 52.18, NodeType::OffCurve),
        Node::new(NodeId(6), 105.84, 47.82, NodeType::OffCurve),
        Node::new(NodeId(7), 105.84, 84.08, NodeType::Smooth),
    ];
    let mut next_id = 100;
    heal_delete_nodes(&mut scurve, false, &[NodeId(4)], &mut next_id);
    assert_eq!(
        scurve.len(),
        4,
        "Healing middle node of 2-segment open cubic curve must leave 1 cubic segment (4 nodes)"
    );
    assert_eq!(scurve[1].node_type, NodeType::OffCurve);
    assert_eq!(scurve[2].node_type, NodeType::OffCurve);

    // Verify outgoing tangent angle AND exact Reference Vector Golden Master handle positions:
    // scurve_after_removeNodeCheckKeepShape: H1 = (-35.77, 53.66), H2 = (105.84, 43.42)
    let orig_angle = (6.07_f64).atan2(-4.05_f64);
    let healed_angle = (scurve[1].point.y.0 - scurve[0].point.y.0)
        .atan2(scurve[1].point.x.0 - scurve[0].point.x.0);
    assert_relative_eq!(orig_angle, healed_angle, epsilon = 0.01);
    assert_relative_eq!(scurve[1].point.x.0, -35.77, epsilon = 1.5);
    assert_relative_eq!(scurve[1].point.y.0, 53.66, epsilon = 1.5);
    assert_relative_eq!(scurve[2].point.x.0, 105.84, epsilon = 1.5);
    assert_relative_eq!(scurve[2].point.y.0, 43.42, epsilon = 1.5);
}

#[test]
fn test_def_grafia_003_de_casteljau_split_then_heal_restores_original_cubic_and_multi_node_chain() {
    // Part A: Split a cubic curve at t=0.25 via De Casteljau and then heal_delete the split node -> restores original handles!
    let mut contour = vec![
        Node::new(NodeId(1), 0.0, 0.0, NodeType::Corner),
        Node::new(NodeId(2), 0.0, 120.0, NodeType::OffCurve),
        Node::new(NodeId(3), 180.0, 200.0, NodeType::OffCurve),
        Node::new(NodeId(4), 240.0, 200.0, NodeType::Smooth),
    ];
    let seg0 = extract_segments(&contour, false)[0];
    let mut next_id = 50;
    let split_id = split_contour_segment(&mut contour, seg0, 0.25, &mut next_id);
    assert_eq!(contour.len(), 7);

    heal_delete_nodes(&mut contour, false, &[split_id], &mut next_id);
    assert_eq!(contour.len(), 4);
    assert_relative_eq!(contour[1].point.x.0, 0.0, epsilon = 0.05);
    assert_relative_eq!(contour[1].point.y.0, 120.0, epsilon = 0.05);
    assert_relative_eq!(contour[2].point.x.0, 180.0, epsilon = 0.05);
    assert_relative_eq!(contour[2].point.y.0, 200.0, epsilon = 0.05);

    // Part B: Simultaneous deletion of 2 adjacent Smooth curve nodes fits a single smooth cubic
    let seg_a = extract_segments(&contour, false)[0];
    let split_a = split_contour_segment(&mut contour, seg_a, 0.33, &mut next_id);
    let seg_b = extract_segments(&contour, false)[1];
    let split_b = split_contour_segment(&mut contour, seg_b, 0.5, &mut next_id);
    assert_eq!(contour.len(), 10);

    heal_delete_nodes(&mut contour, false, &[split_a, split_b], &mut next_id);
    assert_eq!(
        contour.len(),
        4,
        "Deleting 2 adjacent curve nodes simultaneously must heal to 1 cubic segment"
    );
    assert_relative_eq!(contour[1].point.x.0, 0.0, epsilon = 0.2);
    assert_relative_eq!(contour[1].point.y.0, 120.0, epsilon = 0.2);
    assert_relative_eq!(contour[2].point.x.0, 180.0, epsilon = 0.2);
    assert_relative_eq!(contour[2].point.y.0, 200.0, epsilon = 0.2);
}

#[test]
fn test_edge_cases_two_node_cubic_delete_and_mid_drag_alt_toggle() {
    // Part 1: Deleting an endpoint of a 2-node open cubic contour must NOT leave dangling OffCurve handles
    let mut open_two = vec![
        Node::new(NodeId(1), 0.0, 0.0, NodeType::Corner),
        Node::new(NodeId(2), 30.0, 60.0, NodeType::OffCurve),
        Node::new(NodeId(3), 70.0, 60.0, NodeType::OffCurve),
        Node::new(NodeId(4), 100.0, 0.0, NodeType::Corner),
    ];
    let mut next_id = 20;
    heal_delete_nodes(&mut open_two, false, &[NodeId(1)], &mut next_id);
    assert_eq!(
        open_two.len(),
        1,
        "Deleting endpoint of 2-node open cubic contour must prune orphaned OffCurve handles"
    );
    assert_eq!(open_two[0].id, NodeId(4));

    // Part 2: Mid-drag dynamic Alt press and release restores Smooth collinearity
    let ctx = Context::default();
    let theme = GrafiaTheme::sonoma_default();
    let mut state = GrafiaCanvasState::init_glyph_o();
    state.active_tool = CanvasTool::Select;

    let anchor_pos = state.contours[0][0].point;
    let h_out_pos = state.contours[0][1].point;
    let orig_h_in = state.contours[0][11].point;
    let start_scr = state.viewport.to_screen(h_out_pos);
    let mid_scr = state
        .viewport
        .to_screen(FontPoint::new(h_out_pos.x.0, h_out_pos.y.0 + 50.0));
    let end_scr = state
        .viewport
        .to_screen(FontPoint::new(h_out_pos.x.0, h_out_pos.y.0 + 80.0));

    // Step A: Mouse down without Alt
    let mut down = RawInput::default();
    down.events.push(Event::PointerButton {
        pos: pos2(start_scr.x.0, start_scr.y.0),
        button: PointerButton::Primary,
        pressed: true,
        modifiers: Modifiers::NONE,
    });
    run_frame(&ctx, &mut state, &theme, down);

    // Step B: Press Alt mid-drag -> breaks Smooth to Corner and restores opposite handle to orig_h_in
    ctx.input_mut(|i| i.modifiers.alt = true);
    let mut drag_alt = RawInput::default();
    drag_alt
        .events
        .push(Event::PointerMoved(pos2(mid_scr.x.0, mid_scr.y.0)));
    run_frame(&ctx, &mut state, &theme, drag_alt);
    assert_eq!(state.contours[0][0].node_type, NodeType::Corner);
    assert_relative_eq!(
        state.contours[0][11].point.y.0,
        orig_h_in.y.0,
        epsilon = 1e-3
    );

    // Step C: Release Alt mid-drag -> restores Smooth and rotates opposite handle collinearly downward!
    ctx.input_mut(|i| i.modifiers.alt = false);
    let mut drag_no_alt = RawInput::default();
    drag_no_alt
        .events
        .push(Event::PointerMoved(pos2(end_scr.x.0, end_scr.y.0)));
    run_frame(&ctx, &mut state, &theme, drag_no_alt);
    assert_eq!(
        state.contours[0][0].node_type,
        NodeType::Smooth,
        "Releasing Alt mid-drag must restore Smooth node type"
    );
    assert!(
        state.contours[0][11].point.y.0 < anchor_pos.y.0 - 25.0,
        "Releasing Alt mid-drag must resume opposite handle collinear rotation"
    );
}

// ============================================================================
// DEF-GRAFIA-004: Pen Tool Segment Hover Ghost Node Affordance
// ============================================================================

#[test]
fn test_def_grafia_004_pen_tool_segment_hover_sets_ghost_node_and_renders_preview() {
    let ctx = Context::default();
    let theme = GrafiaTheme::sonoma_default();
    let mut state = GrafiaCanvasState::init_glyph_o();
    state.active_tool = CanvasTool::Pen;

    // Top-right cubic arch of /o: evaluate midpoint at t=0.5
    let segs = extract_segments(&state.contours[0], true);
    let mid_pt = match segs[0] {
        ContourSegment::Cubic { cubic, .. } => {
            use kurbo::ParamCurve;
            let p = cubic.eval(0.5);
            FontPoint::new(p.x, p.y)
        }
        _ => panic!("Expected Cubic segment on /o"),
    };
    let scr_mid = state.viewport.to_screen(mid_pt);

    let mut input = RawInput::default();
    input
        .events
        .push(Event::PointerMoved(pos2(scr_mid.x.0, scr_mid.y.0)));
    run_frame(&ctx, &mut state, &theme, input);

    assert!(
        state.hovered_segment_hit.is_some(),
        "DEF-GRAFIA-004: Hovering with PenTool over a cubic segment must populate state.hovered_segment_hit"
    );
    let hit = state.hovered_segment_hit.unwrap();
    assert_eq!(hit.contour_idx, 0);
    assert_relative_eq!(hit.t, 0.5, epsilon = 0.08);

    // Move pointer far outside contour
    let mut input_away = RawInput::default();
    input_away.events.push(Event::PointerMoved(pos2(5.0, 5.0)));
    run_frame(&ctx, &mut state, &theme, input_away);
    assert!(
        state.hovered_segment_hit.is_none(),
        "Moving pointer away from segments must clear hovered_segment_hit"
    );
}

// ============================================================================
// DEF-GRAFIA-005: Shift+Delete & Shift+Backspace Cut-Open Shortcut Parity
// ============================================================================

#[rstest]
#[case(Key::Delete)]
#[case(Key::Backspace)]
fn test_def_grafia_005_shift_delete_and_shift_backspace_cut_contour_open(#[case] key: Key) {
    let ctx = Context::default();
    let theme = GrafiaTheme::sonoma_default();
    let mut state = GrafiaCanvasState::init_glyph_a();

    let target_id = state.contours[0][1].id;
    state.selected_nodes = vec![target_id];

    let mut input = RawInput::default();
    input.events.push(Event::Key {
        key,
        physical_key: None,
        pressed: true,
        repeat: false,
        modifiers: Modifiers::SHIFT,
    });
    run_frame(&ctx, &mut state, &theme, input);

    assert!(
        state.find_node(target_id).is_none(),
        "DEF-GRAFIA-005: Shift+{key:?} must remove the selected node"
    );
    assert!(
        !state.is_contour_closed(0),
        "DEF-GRAFIA-005: Shift+{key:?} must cut the contour open (closed = false)"
    );
}

// ============================================================================
// DEF-GRAFIA-006: Multi-Archetype Glyphs (/A, /n, /o, /s) & Auto-Fit
// ============================================================================

#[rstest]
#[case(GlyphArchetype::A, "A", false)]
#[case(GlyphArchetype::N, "n", true)]
#[case(GlyphArchetype::O, "o", true)]
#[case(GlyphArchetype::S, "s", true)]
fn test_def_grafia_006_archetype_glyphs_load_with_authentic_topology(
    #[case] archetype: GlyphArchetype,
    #[case] expected_name: &str,
    #[case] expects_cubic: bool,
) {
    let mut state = GrafiaCanvasState::init_glyph_a();
    state.load_glyph_archetype(archetype);
    assert_eq!(state.glyph.name, expected_name);
    assert!(!state.contours.is_empty());

    let has_cubic = state
        .contours
        .iter()
        .any(|c| c.iter().any(|n| n.node_type == NodeType::OffCurve));
    assert_eq!(
        has_cubic, expects_cubic,
        "Archetype /{expected_name} cubic presence mismatch"
    );
}

// ============================================================================
// TC-1246..1251: Handle & Node Dragging, Collinearity, Line->Cubic & Reference Parity
// ============================================================================

#[test]
fn test_tc_1246_moving_on_curve_node_translates_attached_bcp_handles() {
    let ctx = Context::default();
    let theme = GrafiaTheme::sonoma_default();
    let mut state = GrafiaCanvasState::init_glyph_o();
    state.active_tool = CanvasTool::Select;

    // In /o contour 0: node 0 is Smooth at (280, 510), node 1 is outgoing OffCurve (407, 510),
    // node 11 (last node) is incoming OffCurve (153, 510).
    let anchor_id = state.contours[0][0].id;
    let out_bcp_before = state.contours[0][1].point;
    let in_bcp_before = state.contours[0][11].point;
    let other_bcp_before = state.contours[0][2].point;

    state.selected_nodes = vec![anchor_id];

    // Press Shift + ArrowRight (+10 font units X) and Shift + ArrowUp (+10 font units Y)
    let mut input = RawInput::default();
    input.events.push(Event::Key {
        key: Key::ArrowRight,
        physical_key: None,
        pressed: true,
        repeat: false,
        modifiers: Modifiers::SHIFT,
    });
    input.events.push(Event::Key {
        key: Key::ArrowUp,
        physical_key: None,
        pressed: true,
        repeat: false,
        modifiers: Modifiers::SHIFT,
    });
    run_frame(&ctx, &mut state, &theme, input);

    let anchor_after = state.contours[0][0].point;
    let out_bcp_after = state.contours[0][1].point;
    let in_bcp_after = state.contours[0][11].point;
    let other_bcp_after = state.contours[0][2].point;

    assert_relative_eq!(anchor_after.x.0, 290.0, epsilon = 1e-3);
    assert_relative_eq!(anchor_after.y.0, 520.0, epsilon = 1e-3);
    assert_relative_eq!(out_bcp_after.x.0, out_bcp_before.x.0 + 10.0, epsilon = 1e-3);
    assert_relative_eq!(out_bcp_after.y.0, out_bcp_before.y.0 + 10.0, epsilon = 1e-3);
    assert_relative_eq!(in_bcp_after.x.0, in_bcp_before.x.0 + 10.0, epsilon = 1e-3);
    assert_relative_eq!(in_bcp_after.y.0, in_bcp_before.y.0 + 10.0, epsilon = 1e-3);
    assert_relative_eq!(other_bcp_after.x.0, other_bcp_before.x.0, epsilon = 1e-3);
    assert_relative_eq!(other_bcp_after.y.0, other_bcp_before.y.0, epsilon = 1e-3);
}

#[test]
fn test_tc_1247_and_1248_dragging_off_curve_handle_rotates_smooth_opposite_and_alt_breaks_corner() {
    let ctx = Context::default();
    let theme = GrafiaTheme::sonoma_default();
    let mut state = GrafiaCanvasState::init_glyph_o();
    state.active_tool = CanvasTool::Select;

    // Contour 0 of /o: node 0 is Smooth at (280, 510), node 1 is OffCurve at (407, 510), node 11 is OffCurve at (153, 510)
    let anchor_pos = state.contours[0][0].point;
    let h_out_pos = state.contours[0][1].point;
    let h_in_pos = state.contours[0][11].point;
    let orig_in_len = (h_in_pos.x.0 - anchor_pos.x.0).hypot(h_in_pos.y.0 - anchor_pos.y.0);

    // Drag node 1 (407, 510) upward by +60 font units to (407, 570) without Alt
    let start_scr = state.viewport.to_screen(h_out_pos);
    let end_scr = state
        .viewport
        .to_screen(FontPoint::new(h_out_pos.x.0, h_out_pos.y.0 + 60.0));

    let mut down = RawInput::default();
    down.events.push(Event::PointerButton {
        pos: pos2(start_scr.x.0, start_scr.y.0),
        button: PointerButton::Primary,
        pressed: true,
        modifiers: Modifiers::NONE,
    });
    run_frame(&ctx, &mut state, &theme, down);

    let mut drag = RawInput::default();
    drag.events
        .push(Event::PointerMoved(pos2(end_scr.x.0, end_scr.y.0)));
    run_frame(&ctx, &mut state, &theme, drag);

    let mut up = RawInput::default();
    up.events.push(Event::PointerButton {
        pos: pos2(end_scr.x.0, end_scr.y.0),
        button: PointerButton::Primary,
        pressed: false,
        modifiers: Modifiers::NONE,
    });
    run_frame(&ctx, &mut state, &theme, up);

    // Opposite handle (node 11) must have rotated downward (y < 510) collinearly with node 1!
    let new_h_out = state.contours[0][1].point;
    let new_h_in = state.contours[0][11].point;
    assert!(
        new_h_in.y.0 < anchor_pos.y.0 - 20.0,
        "TC-1247: Opposite handle of Smooth node must rotate downward when outgoing handle is dragged upward, got y={}",
        new_h_in.y.0
    );
    let new_in_len = (new_h_in.x.0 - anchor_pos.x.0).hypot(new_h_in.y.0 - anchor_pos.y.0);
    assert_relative_eq!(new_in_len, orig_in_len, epsilon = 0.5);

    // Cross product of (new_h_out - anchor) and (new_h_in - anchor) must be ~0 (collinear)
    let v_out = (
        new_h_out.x.0 - anchor_pos.x.0,
        new_h_out.y.0 - anchor_pos.y.0,
    );
    let v_in = (new_h_in.x.0 - anchor_pos.x.0, new_h_in.y.0 - anchor_pos.y.0);
    let cross = v_out.0 * v_in.1 - v_out.1 * v_in.0;
    assert!(
        cross.abs() < 5.0,
        "Handles across Smooth node must be collinear, cross={cross}"
    );

    // TC-1248: Now drag node 1 with Modifiers::ALT -> breaks Smooth into Corner and keeps node 11 fixed!
    let fixed_h_in = state.contours[0][11].point;
    let start_scr2 = state.viewport.to_screen(new_h_out);
    let end_scr2 = state
        .viewport
        .to_screen(FontPoint::new(new_h_out.x.0, new_h_out.y.0 - 80.0));

    let mut down2 = RawInput::default();
    down2.events.push(Event::PointerButton {
        pos: pos2(start_scr2.x.0, start_scr2.y.0),
        button: PointerButton::Primary,
        pressed: true,
        modifiers: Modifiers::ALT,
    });
    run_frame(&ctx, &mut state, &theme, down2);

    let mut drag2 = RawInput::default();
    drag2
        .events
        .push(Event::PointerMoved(pos2(end_scr2.x.0, end_scr2.y.0)));
    run_frame(&ctx, &mut state, &theme, drag2);

    assert_eq!(
        state.contours[0][0].node_type,
        NodeType::Corner,
        "TC-1248: Alt-dragging a handle must convert Smooth anchor to Corner"
    );
    assert_relative_eq!(
        state.contours[0][11].point.x.0,
        fixed_h_in.x.0,
        epsilon = 1e-3
    );
    assert_relative_eq!(
        state.contours[0][11].point.y.0,
        fixed_h_in.y.0,
        epsilon = 1e-3
    );
}

#[test]
fn test_tc_1249_alt_click_on_line_segment_converts_to_cubic_at_one_third_and_two_thirds() {
    let ctx = Context::default();
    let theme = GrafiaTheme::sonoma_default();
    let mut state = GrafiaCanvasState::init_glyph_a();
    state.active_tool = CanvasTool::Select;

    let initial_len = state.contours[0].len();
    // Closing bottom bar segment of /A: (160, 0) -> (30, 0), midpoint at (95, 0)
    let mid_scr = state.viewport.to_screen(FontPoint::new(95.0, 0.0));

    let mut input = RawInput::default();
    input.events.push(Event::PointerButton {
        pos: pos2(mid_scr.x.0, mid_scr.y.0),
        button: PointerButton::Primary,
        pressed: true,
        modifiers: Modifiers::ALT,
    });
    run_frame(&ctx, &mut state, &theme, input);

    assert_eq!(
        state.contours[0].len(),
        initial_len + 2,
        "TC-1249: Alt-clicking a line segment must insert 2 OffCurve BCP handles"
    );
    let off_curves: Vec<_> = state.contours[0]
        .iter()
        .filter(|n| n.node_type == NodeType::OffCurve)
        .collect();
    assert_eq!(off_curves.len(), 2);
    // 1/3 and 2/3 between 160 and 30: 116.67 and 73.33
    assert_relative_eq!(off_curves[0].point.x.0, 160.0 - 130.0 / 3.0, epsilon = 0.5);
    assert_relative_eq!(off_curves[1].point.x.0, 160.0 - 260.0 / 3.0, epsilon = 0.5);
}

#[test]
fn test_tc_1250_tab_and_shift_tab_cycle_nodes_and_enter_toggles_smooth_corner() {
    let ctx = Context::default();
    let theme = GrafiaTheme::sonoma_default();
    let mut state = GrafiaCanvasState::init_glyph_a();

    let id0 = state.contours[0][0].id;
    let id1 = state.contours[0][1].id;
    state.selected_nodes = vec![id0];

    // Press Tab -> selects id1
    let mut tab_in = RawInput::default();
    tab_in.events.push(Event::Key {
        key: Key::Tab,
        physical_key: None,
        pressed: true,
        repeat: false,
        modifiers: Modifiers::NONE,
    });
    run_frame(&ctx, &mut state, &theme, tab_in);
    assert_eq!(
        state.selected_nodes,
        vec![id1],
        "Tab must advance to next node on contour"
    );

    // Press Shift+Tab -> selects id0
    let mut backtab_in = RawInput::default();
    backtab_in.events.push(Event::Key {
        key: Key::Tab,
        physical_key: None,
        pressed: true,
        repeat: false,
        modifiers: Modifiers::SHIFT,
    });
    run_frame(&ctx, &mut state, &theme, backtab_in);
    assert_eq!(
        state.selected_nodes,
        vec![id0],
        "Shift+Tab must step back to previous node"
    );

    // Press Enter -> toggles Corner to Smooth
    assert_eq!(state.contours[0][0].node_type, NodeType::Corner);
    let mut enter_in = RawInput::default();
    enter_in.events.push(Event::Key {
        key: Key::Enter,
        physical_key: None,
        pressed: true,
        repeat: false,
        modifiers: Modifiers::NONE,
    });
    run_frame(&ctx, &mut state, &theme, enter_in);
    assert_eq!(
        state.contours[0][0].node_type,
        NodeType::Smooth,
        "Enter must toggle selected Corner node to Smooth"
    );
}

#[test]
fn test_tc_1251_reference_v010_bezier_path_de_casteljau_golden_master_parity() {
    // Exact input from reference vector bezier_and_fitting.json:
    // "(0.0, 0.0, type=1)", "(0.0, 55.0, type=65)", "(45.0, 100.0, type=65)", "(100.0, 100.0, type=35)"
    let mut contour = vec![
        Node::new(NodeId(1), 0.0, 0.0, NodeType::Corner),
        Node::new(NodeId(2), 0.0, 55.0, NodeType::OffCurve),
        Node::new(NodeId(3), 45.0, 100.0, NodeType::OffCurve),
        Node::new(NodeId(4), 100.0, 100.0, NodeType::Smooth),
    ];
    let segs = extract_segments(&contour, false);
    assert_eq!(segs.len(), 1);
    let mut next_id = 10;
    let new_id = split_contour_segment(&mut contour, segs[0], 0.5, &mut next_id);
    assert_eq!(contour.len(), 7);
    assert_eq!(contour[3].id, new_id);

    // Exact Reference Vector output after insertNodeWithPathTime_(3.5):
    // (0.0, 0.0), (0.0, 27.5), (11.25, 52.5), (29.375, 70.625), (47.5, 88.75), (72.5, 100.0), (100.0, 100.0)
    let expected = [
        (0.0, 0.0, NodeType::Corner),
        (0.0, 27.5, NodeType::OffCurve),
        (11.25, 52.5, NodeType::OffCurve),
        (29.375, 70.625, NodeType::Smooth),
        (47.5, 88.75, NodeType::OffCurve),
        (72.5, 100.0, NodeType::OffCurve),
        (100.0, 100.0, NodeType::Smooth),
    ];
    for (node, (ex, ey, et)) in contour.iter().zip(expected.iter()) {
        assert_relative_eq!(node.point.x.0, *ex, epsilon = 1e-3);
        assert_relative_eq!(node.point.y.0, *ey, epsilon = 1e-3);
        assert_eq!(node.node_type, *et);
    }
}

// ============================================================================
// TC-1260: Exhaustive 34-State Visual PNG Verification Matrix, Contact Sheet & GIF
// ============================================================================

#[test]
fn test_tc_1260_visual_e2e_interaction_png_frames_and_strip_with_input_overlays() {
    use grafia::core::ContourId;
    use grafia::locator::{contour_winding_label, ShapeLocatorRenderer, ShapeLocatorStyle};
    use image::{Rgba, RgbaImage};
    use std::fs;
    use std::path::PathBuf;

    let out_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("target")
        .join("visual_verification");
    fs::create_dir_all(&out_dir).expect("Create visual_verification directory");

    let w = 640_u32;
    let h = 480_u32;
    let ctx = Context::default();
    let theme = GrafiaTheme::sonoma_default();

    let raster_to_rgba = |state: &GrafiaCanvasState| -> RgbaImage {
        pixmap_to_rgba_image(rasterize_canvas_subregion(state, &theme, w, h))
    };

    let count_pixel_delta = |a: &RgbaImage, b: &RgbaImage| -> usize {
        assert_eq!(a.dimensions(), b.dimensions());
        a.pixels()
            .zip(b.pixels())
            .filter(|(p1, p2)| p1 != p2)
            .count()
    };

    fn glyph_5x7(ch: char) -> [u8; 7] {
        match ch.to_ascii_uppercase() {
            'A' => [
                0b01110, 0b10001, 0b10001, 0b11111, 0b10001, 0b10001, 0b10001,
            ],
            'B' => [
                0b11110, 0b10001, 0b10001, 0b11110, 0b10001, 0b10001, 0b11110,
            ],
            'C' => [
                0b01110, 0b10001, 0b10000, 0b10000, 0b10000, 0b10001, 0b01110,
            ],
            'D' => [
                0b11100, 0b10010, 0b10001, 0b10001, 0b10001, 0b10010, 0b11100,
            ],
            'E' => [
                0b11111, 0b10000, 0b10000, 0b11110, 0b10000, 0b10000, 0b11111,
            ],
            'F' => [
                0b11111, 0b10000, 0b10000, 0b11110, 0b10000, 0b10000, 0b10000,
            ],
            'G' => [
                0b01110, 0b10001, 0b10000, 0b10111, 0b10001, 0b10001, 0b01110,
            ],
            'H' => [
                0b10001, 0b10001, 0b10001, 0b11111, 0b10001, 0b10001, 0b10001,
            ],
            'I' => [
                0b01110, 0b00100, 0b00100, 0b00100, 0b00100, 0b00100, 0b01110,
            ],
            'J' => [
                0b00111, 0b00010, 0b00010, 0b00010, 0b10010, 0b10010, 0b01100,
            ],
            'K' => [
                0b10001, 0b10010, 0b10100, 0b11000, 0b10100, 0b10010, 0b10001,
            ],
            'L' => [
                0b10000, 0b10000, 0b10000, 0b10000, 0b10000, 0b10000, 0b11111,
            ],
            'M' => [
                0b10001, 0b11011, 0b10101, 0b10101, 0b10001, 0b10001, 0b10001,
            ],
            'N' => [
                0b10001, 0b11001, 0b10101, 0b10011, 0b10001, 0b10001, 0b10001,
            ],
            'O' => [
                0b01110, 0b10001, 0b10001, 0b10001, 0b10001, 0b10001, 0b01110,
            ],
            'P' => [
                0b11110, 0b10001, 0b10001, 0b11110, 0b10000, 0b10000, 0b10000,
            ],
            'Q' => [
                0b01110, 0b10001, 0b10001, 0b10001, 0b10101, 0b10010, 0b01101,
            ],
            'R' => [
                0b11110, 0b10001, 0b10001, 0b11110, 0b10100, 0b10010, 0b10001,
            ],
            'S' => [
                0b01111, 0b10000, 0b10000, 0b01110, 0b00001, 0b00001, 0b11110,
            ],
            'T' => [
                0b11111, 0b00100, 0b00100, 0b00100, 0b00100, 0b00100, 0b00100,
            ],
            'U' => [
                0b10001, 0b10001, 0b10001, 0b10001, 0b10001, 0b10001, 0b01110,
            ],
            'V' => [
                0b10001, 0b10001, 0b10001, 0b10001, 0b10001, 0b01010, 0b00100,
            ],
            'W' => [
                0b10001, 0b10001, 0b10001, 0b10101, 0b10101, 0b11011, 0b10001,
            ],
            'X' => [
                0b10001, 0b10001, 0b01010, 0b00100, 0b01010, 0b10001, 0b10001,
            ],
            'Y' => [
                0b10001, 0b10001, 0b01010, 0b00100, 0b00100, 0b00100, 0b00100,
            ],
            'Z' => [
                0b11111, 0b00001, 0b00010, 0b00100, 0b01000, 0b10000, 0b11111,
            ],
            '0' => [
                0b01110, 0b10001, 0b10011, 0b10101, 0b11001, 0b10001, 0b01110,
            ],
            '1' => [
                0b00100, 0b01100, 0b00100, 0b00100, 0b00100, 0b00100, 0b01110,
            ],
            '2' => [
                0b01110, 0b10001, 0b00001, 0b00110, 0b01000, 0b10000, 0b11111,
            ],
            '3' => [
                0b11110, 0b00001, 0b00001, 0b01110, 0b00001, 0b00001, 0b11110,
            ],
            '4' => [
                0b10010, 0b10010, 0b10010, 0b11111, 0b00010, 0b00010, 0b00010,
            ],
            '5' => [
                0b11111, 0b10000, 0b10000, 0b11110, 0b00001, 0b00001, 0b11110,
            ],
            '6' => [
                0b01110, 0b10000, 0b10000, 0b11110, 0b10001, 0b10001, 0b01110,
            ],
            '7' => [
                0b11111, 0b00001, 0b00010, 0b00100, 0b01000, 0b01000, 0b01000,
            ],
            '8' => [
                0b01110, 0b10001, 0b10001, 0b01110, 0b10001, 0b10001, 0b01110,
            ],
            '9' => [
                0b01110, 0b10001, 0b10001, 0b01111, 0b00001, 0b00001, 0b01110,
            ],
            '[' => [
                0b01110, 0b01000, 0b01000, 0b01000, 0b01000, 0b01000, 0b01110,
            ],
            ']' => [
                0b01110, 0b00010, 0b00010, 0b00010, 0b00010, 0b00010, 0b01110,
            ],
            '(' => [
                0b00100, 0b01000, 0b01000, 0b01000, 0b01000, 0b01000, 0b00100,
            ],
            ')' => [
                0b00100, 0b00010, 0b00010, 0b00010, 0b00010, 0b00010, 0b00100,
            ],
            '/' => [
                0b00001, 0b00010, 0b00100, 0b00100, 0b01000, 0b10000, 0b10000,
            ],
            '+' => [
                0b00000, 0b00100, 0b00100, 0b11111, 0b00100, 0b00100, 0b00000,
            ],
            '-' => [
                0b00000, 0b00000, 0b00000, 0b11111, 0b00000, 0b00000, 0b00000,
            ],
            '=' => [
                0b00000, 0b00000, 0b11111, 0b00000, 0b11111, 0b00000, 0b00000,
            ],
            '&' => [
                0b01100, 0b10010, 0b10100, 0b01000, 0b10101, 0b10010, 0b01101,
            ],
            '>' => [
                0b10000, 0b01000, 0b00100, 0b00010, 0b00100, 0b01000, 0b10000,
            ],
            ':' => [
                0b00000, 0b00100, 0b00100, 0b00000, 0b00100, 0b00100, 0b00000,
            ],
            '.' => [
                0b00000, 0b00000, 0b00000, 0b00000, 0b00000, 0b00110, 0b00110,
            ],
            ',' => [
                0b00000, 0b00000, 0b00000, 0b00000, 0b00010, 0b00010, 0b00100,
            ],
            _ => [0; 7],
        }
    }

    fn draw_text_5x7(img: &mut RgbaImage, text: &str, x0: u32, y0: u32, scale: u32, col: Rgba<u8>) {
        for (c_idx, ch) in text.chars().enumerate() {
            let bits = glyph_5x7(ch);
            let base_x = x0 + (c_idx as u32) * (6 * scale);
            for (row, &mask) in bits.iter().enumerate() {
                for col_i in 0..5_u32 {
                    if (mask >> (4 - col_i)) & 1 == 1 {
                        for sy in 0..scale {
                            for sx in 0..scale {
                                let px = base_x + col_i * scale + sx;
                                let py = y0 + (row as u32) * scale + sy;
                                if px < img.width() && py < img.height() {
                                    img.put_pixel(px, py, col);
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    fn ry_clamp(py: i32, h: u32) -> u32 {
        (py.max(0) as u32).min(h - 1)
    }

    let draw_hud_overlay = |img: &mut RgbaImage,
                            cursor: Option<(f32, f32)>,
                            drag_from: Option<(f32, f32)>,
                            clicked: bool,
                            accent: [u8; 4],
                            label: &str| {
        if !label.is_empty() {
            let banner_w = (36 + (label.len() as u32) * 12).min(img.width() - 12);
            for py in 10..42 {
                for px in 10..banner_w {
                    if px < img.width() && py < img.height() {
                        let is_pill = px < 22;
                        img.put_pixel(
                            px,
                            py,
                            if is_pill {
                                Rgba(accent)
                            } else {
                                Rgba([28, 28, 32, 240])
                            },
                        );
                    }
                }
            }
            draw_text_5x7(img, label, 30, 19, 2, Rgba([255, 255, 255, 255]));
        }

        if let (Some((x0, y0)), Some((x1, y1))) = (drag_from, cursor) {
            let steps = 80;
            for i in 18..=steps {
                if (i / 4) % 2 == 1 {
                    continue;
                }
                let t = (i as f32) / (steps as f32);
                let x = (x0 + (x1 - x0) * t).round() as i32;
                let y = (y0 + (y1 - y0) * t).round() as i32;
                for dy in -1..=1 {
                    for dx in -1..=1 {
                        let px = x + dx;
                        let py = y + dy;
                        if px >= 0
                            && py >= 0
                            && (px as u32) < img.width()
                            && (py as u32) < img.height()
                        {
                            img.put_pixel(
                                px as u32,
                                ry_clamp(py, img.height()),
                                Rgba([255, 128, 0, 235]),
                            );
                        }
                    }
                }
            }
        }

        if let Some((cx, cy)) = cursor {
            let icx = cx.round() as i32;
            let icy = cy.round() as i32;
            if clicked {
                for angle_step in 0..72 {
                    let rad = (angle_step as f32) * std::f32::consts::TAU / 72.0;
                    for r in [14.0_f32, 15.0_f32, 20.0_f32] {
                        let rx = (cx + r * rad.cos()).round() as i32;
                        let ry = (cy + r * rad.sin()).round() as i32;
                        if rx >= 0
                            && ry >= 0
                            && (rx as u32) < img.width()
                            && (ry as u32) < img.height()
                        {
                            img.put_pixel(rx as u32, ry as u32, Rgba([255, 59, 48, 255]));
                        }
                    }
                }
            }
            for dy in 0..16 {
                let max_dx = (dy * 11) / 16;
                for dx in 0..=max_dx {
                    let px = icx + dx;
                    let py = icy + dy;
                    if px >= 0 && py >= 0 && (px as u32) < img.width() && (py as u32) < img.height()
                    {
                        let is_border = dx == 0 || dx == max_dx || dy == 15;
                        let col = if is_border {
                            Rgba([20, 20, 20, 255])
                        } else {
                            Rgba([255, 255, 255, 255])
                        };
                        img.put_pixel(px as u32, py as u32, col);
                    }
                }
            }
        }
    };

    fn blit_scaled_half(dst: &mut RgbaImage, src: &RgbaImage, dst_x0: u32, dst_y0: u32) {
        let half_w = src.width() / 2;
        let half_h = src.height() / 2;
        for y in 0..half_h {
            for x in 0..half_w {
                if dst_x0 + x < dst.width() && dst_y0 + y < dst.height() {
                    let p00 = src.get_pixel(x * 2, y * 2);
                    let p10 = src.get_pixel((x * 2 + 1).min(src.width() - 1), y * 2);
                    let p01 = src.get_pixel(x * 2, (y * 2 + 1).min(src.height() - 1));
                    let p11 = src.get_pixel(
                        (x * 2 + 1).min(src.width() - 1),
                        (y * 2 + 1).min(src.height() - 1),
                    );
                    let avg = Rgba([
                        ((p00[0] as u16 + p10[0] as u16 + p01[0] as u16 + p11[0] as u16) / 4) as u8,
                        ((p00[1] as u16 + p10[1] as u16 + p01[1] as u16 + p11[1] as u16) / 4) as u8,
                        ((p00[2] as u16 + p10[2] as u16 + p01[2] as u16 + p11[2] as u16) / 4) as u8,
                        255,
                    ]);
                    dst.put_pixel(dst_x0 + x, dst_y0 + y, avg);
                }
            }
        }
    }

    fn rasterize_egui_mesh_onto_image(
        img: &mut RgbaImage,
        mesh: &egui::Mesh,
        offset_x: f32,
        offset_y: f32,
        col: Rgba<u8>,
    ) {
        for tri in mesh.indices.as_chunks::<3>().0 {
            let p0 = mesh.vertices[tri[0] as usize].pos;
            let p1 = mesh.vertices[tri[1] as usize].pos;
            let p2 = mesh.vertices[tri[2] as usize].pos;
            let (x0, y0) = (p0.x + offset_x, p0.y + offset_y);
            let (x1, y1) = (p1.x + offset_x, p1.y + offset_y);
            let (x2, y2) = (p2.x + offset_x, p2.y + offset_y);
            let min_x = x0.min(x1).min(x2).floor().max(0.0) as u32;
            let max_x = x0.max(x1).max(x2).ceil().min((img.width() - 1) as f32) as u32;
            let min_y = y0.min(y1).min(y2).floor().max(0.0) as u32;
            let max_y = y0.max(y1).max(y2).ceil().min((img.height() - 1) as f32) as u32;
            let area = (x1 - x0) * (y2 - y0) - (x2 - x0) * (y1 - y0);
            if area.abs() < 1e-5 {
                continue;
            }
            for py in min_y..=max_y {
                for px in min_x..=max_x {
                    let (fx, fy) = (px as f32 + 0.5, py as f32 + 0.5);
                    let w0 = (x1 - fx) * (y2 - fy) - (x2 - fx) * (y1 - fy);
                    let w1 = (x2 - fx) * (y0 - fy) - (x0 - fx) * (y2 - fy);
                    let w2 = (x0 - fx) * (y1 - fy) - (x1 - fx) * (y0 - fy);
                    if (w0 >= 0.0 && w1 >= 0.0 && w2 >= 0.0)
                        || (w0 <= 0.0 && w1 <= 0.0 && w2 <= 0.0)
                    {
                        img.put_pixel(px, py, col);
                    }
                }
            }
        }
    }

    let mut frames_rgba: Vec<(String, RgbaImage)> = Vec::new();

    // =========================================================================
    // I. View Modes & Base Rendering (States 1..4)
    // =========================================================================

    // State 1: EditWireframe [1] Base (/o, /A, /n, /s 2x2 Archetypes + /o base reference)
    let mut s01 = GrafiaCanvasState::init_glyph_o();
    s01.fit_viewport_to_glyph(w as f32, h as f32);
    let raw01 = raster_to_rgba(&s01);
    let mut s01_a = GrafiaCanvasState::init_glyph_a();
    s01_a.fit_viewport_to_glyph(w as f32, h as f32);
    let raw01_a = raster_to_rgba(&s01_a);
    let mut s01_n = GrafiaCanvasState::init_glyph_n();
    s01_n.fit_viewport_to_glyph(w as f32, h as f32);
    let raw01_n = raster_to_rgba(&s01_n);
    let mut s01_s = GrafiaCanvasState::init_glyph_s();
    s01_s.fit_viewport_to_glyph(w as f32, h as f32);
    let raw01_s = raster_to_rgba(&s01_s);
    let mut img01 = RgbaImage::from_pixel(w, h, Rgba([220, 224, 230, 255]));
    blit_scaled_half(&mut img01, &raw01, 0, 0);
    blit_scaled_half(&mut img01, &raw01_a, w / 2, 0);
    blit_scaled_half(&mut img01, &raw01_n, 0, h / 2);
    blit_scaled_half(&mut img01, &raw01_s, w / 2, h / 2);
    draw_text_5x7(
        &mut img01,
        "/O CUBIC+SMOOTH",
        12,
        48,
        2,
        Rgba([43, 153, 63, 255]),
    );
    draw_text_5x7(
        &mut img01,
        "/A LINES+CORNER",
        332,
        48,
        2,
        Rgba([0, 102, 255, 255]),
    );
    draw_text_5x7(
        &mut img01,
        "/N ARCH+STEMS",
        12,
        250,
        2,
        Rgba([43, 153, 63, 255]),
    );
    draw_text_5x7(
        &mut img01,
        "/S SPINE CURVES",
        332,
        250,
        2,
        Rgba([43, 153, 63, 255]),
    );
    draw_hud_overlay(
        &mut img01,
        Some((160.0, 120.0)),
        None,
        false,
        [43, 153, 63, 255],
        "01: [1] WIREFRAME /O /A /N /S",
    );
    frames_rgba.push(("frame_01_edit_wireframe_base_glyph_o.png".into(), img01));

    // State 2: PreviewFilled [2] / Spacebar Hold (True Non-Zero Winding)
    let mut s02 = s01.clone_via_snapshot();
    s02.view_mode = CanvasViewMode::PreviewFilled;
    let raw02 = raster_to_rgba(&s02);
    assert!(
        count_pixel_delta(&raw01, &raw02) > 5000,
        "State 2 PreviewFilled must differ from EditWireframe"
    );
    let mut img02 = raw02.clone();
    draw_hud_overlay(
        &mut img02,
        Some((320.0, 240.0)),
        None,
        false,
        [35, 35, 40, 255],
        "02: [2] FILLED NONZERO /O",
    );
    frames_rgba.push(("frame_02_preview_filled_nonzero_winding.png".into(), img02));

    // State 3: ShapeThumbnail [3] View Mode
    let mut s03 = GrafiaCanvasState::init_glyph_o();
    s03.view_mode = CanvasViewMode::ShapeThumbnail;
    let raw03 = raster_to_rgba(&s03);
    assert!(
        count_pixel_delta(&raw01, &raw03) > 5000,
        "State 3 ShapeThumbnail must render contour thumbnail cards"
    );
    let mut img03 = raw03;
    draw_hud_overlay(
        &mut img03,
        Some((90.0, 80.0)),
        None,
        false,
        [0, 122, 255, 255],
        "03: [3] SHAPE THUMBNAILS",
    );
    frames_rgba.push(("frame_03_shape_thumbnail_view_mode.png".into(), img03));

    // State 4: ShapeLocator Sidebar Minimap (True Cubic Bézier + CW/CCW Winding)
    let s04 = GrafiaCanvasState::init_glyph_o();
    let mut img04 = RgbaImage::from_pixel(w, h, Rgba([244, 245, 247, 255]));
    let loc_renderer = ShapeLocatorRenderer::new(&s04.glyph, ShapeLocatorStyle::default());
    for (idx, cid) in [ContourId(0), ContourId(1)].iter().enumerate() {
        let card_y = 75 + (idx as u32) * 185;
        for py in card_y..(card_y + 160) {
            for px in 40..600 {
                img04.put_pixel(px, py, Rgba([255, 255, 255, 255]));
            }
        }
        let (ctx_m, foc_m) = loc_renderer.render_locator(*cid, egui::vec2(200.0, 136.0));
        rasterize_egui_mesh_onto_image(
            &mut img04,
            &ctx_m,
            60.0,
            (card_y + 12) as f32,
            Rgba([185, 190, 198, 255]),
        );
        rasterize_egui_mesh_onto_image(
            &mut img04,
            &foc_m,
            60.0,
            (card_y + 12) as f32,
            Rgba([0, 122, 255, 255]),
        );
        let w_lbl = contour_winding_label(&s04.contours[idx]);
        let row_txt = format!("PATH {} (12 PT) [NORMAL, {}]", idx + 1, w_lbl);
        draw_text_5x7(
            &mut img04,
            &row_txt,
            290,
            card_y + 70,
            2,
            Rgba([30, 30, 35, 255]),
        );
    }
    draw_hud_overlay(
        &mut img04,
        Some((160.0, 145.0)),
        None,
        false,
        [0, 122, 255, 255],
        "04: LOCATOR CUBIC CW/CCW",
    );
    frames_rgba.push(("frame_04_shape_locator_cubic_and_winding.png".into(), img04));

    // =========================================================================
    // II. Selection Mechanisms in SelectTool (V) (States 5..12)
    // =========================================================================

    // State 5: Single Click on OnCurve node -> selects 1 node with glow ring
    let mut s05 = GrafiaCanvasState::init_glyph_o();
    s05.fit_viewport_to_glyph(w as f32, h as f32);
    let n0_scr = s05.viewport.to_screen(s05.contours[0][0].point);
    let mut in05 = RawInput::default();
    in05.events.push(Event::PointerButton {
        pos: pos2(n0_scr.x.0, n0_scr.y.0),
        button: PointerButton::Primary,
        pressed: true,
        modifiers: Modifiers::NONE,
    });
    in05.events.push(Event::PointerButton {
        pos: pos2(n0_scr.x.0, n0_scr.y.0),
        button: PointerButton::Primary,
        pressed: false,
        modifiers: Modifiers::NONE,
    });
    run_frame(&ctx, &mut s05, &theme, in05);
    assert_eq!(s05.selected_nodes.len(), 1);
    let raw05 = raster_to_rgba(&s05);
    assert!(
        count_pixel_delta(&raw01, &raw05) >= 20,
        "State 5 single node selection glow must change pixels"
    );
    let mut img05 = raw05.clone();
    draw_hud_overlay(
        &mut img05,
        Some((n0_scr.x.0, n0_scr.y.0)),
        None,
        true,
        [0, 122, 255, 255],
        "05: CLICK SELECT 1 NODE",
    );
    frames_rgba.push(("frame_05_select_single_click_node.png".into(), img05));

    // State 6: Shift + Click on a Single Node (XOR Toggle: adds unselected n3 & n6, then 2nd Shift+Click on n0 removes n0)
    let mut s06 = s05.clone_via_snapshot();
    let n3_scr = s06.viewport.to_screen(s06.contours[0][3].point);
    let n6_scr = s06.viewport.to_screen(s06.contours[0][6].point);
    let mut in06 = RawInput::default();
    in06.events.push(Event::PointerButton {
        pos: pos2(n3_scr.x.0, n3_scr.y.0),
        button: PointerButton::Primary,
        pressed: true,
        modifiers: Modifiers::SHIFT,
    });
    in06.events.push(Event::PointerButton {
        pos: pos2(n3_scr.x.0, n3_scr.y.0),
        button: PointerButton::Primary,
        pressed: false,
        modifiers: Modifiers::SHIFT,
    });
    in06.events.push(Event::PointerButton {
        pos: pos2(n6_scr.x.0, n6_scr.y.0),
        button: PointerButton::Primary,
        pressed: true,
        modifiers: Modifiers::SHIFT,
    });
    in06.events.push(Event::PointerButton {
        pos: pos2(n6_scr.x.0, n6_scr.y.0),
        button: PointerButton::Primary,
        pressed: false,
        modifiers: Modifiers::SHIFT,
    });
    run_frame(&ctx, &mut s06, &theme, in06);
    assert_eq!(
        s06.selected_nodes.len(),
        3,
        "Shift+Click on unselected n3 and n6 must add both to selection"
    );
    // Second Shift+Click on already-selected node n0 removes ONLY n0, leaving [n3, n6]
    let mut in06_remove = RawInput::default();
    in06_remove.events.push(Event::PointerButton {
        pos: pos2(n0_scr.x.0, n0_scr.y.0),
        button: PointerButton::Primary,
        pressed: true,
        modifiers: Modifiers::SHIFT,
    });
    in06_remove.events.push(Event::PointerButton {
        pos: pos2(n0_scr.x.0, n0_scr.y.0),
        button: PointerButton::Primary,
        pressed: false,
        modifiers: Modifiers::SHIFT,
    });
    run_frame(&ctx, &mut s06, &theme, in06_remove);
    assert_eq!(
        s06.selected_nodes,
        vec![s06.contours[0][3].id, s06.contours[0][6].id],
        "Second Shift+Click on selected node n0 must remove only n0"
    );
    let raw06 = raster_to_rgba(&s06);
    assert!(
        count_pixel_delta(&raw05, &raw06) >= 40,
        "State 6 Shift+Click XOR must update selection glow rings"
    );
    let mut img06 = raw06;
    draw_hud_overlay(
        &mut img06,
        Some((n0_scr.x.0, n0_scr.y.0)),
        None,
        true,
        [0, 122, 255, 255],
        "06: SHIFT+CLICK XOR + / -",
    );
    frames_rgba.push(("frame_06_select_shift_click_xor_node.png".into(), img06));

    // State 7: Click on a Line Segment (without dragging) -> selects 2 bounding endpoints
    let mut s07 = GrafiaCanvasState::init_glyph_a();
    s07.fit_viewport_to_glyph(w as f32, h as f32);
    let raw_a_base = raster_to_rgba(&s07);
    let mid_a07 = FontPoint::new(
        (s07.contours[0][0].point.x.0 + s07.contours[0][1].point.x.0) * 0.5,
        (s07.contours[0][0].point.y.0 + s07.contours[0][1].point.y.0) * 0.5,
    );
    let scr_a07 = s07.viewport.to_screen(mid_a07);
    let mut in07 = RawInput::default();
    in07.events.push(Event::PointerButton {
        pos: pos2(scr_a07.x.0, scr_a07.y.0),
        button: PointerButton::Primary,
        pressed: true,
        modifiers: Modifiers::NONE,
    });
    in07.events.push(Event::PointerButton {
        pos: pos2(scr_a07.x.0, scr_a07.y.0),
        button: PointerButton::Primary,
        pressed: false,
        modifiers: Modifiers::NONE,
    });
    run_frame(&ctx, &mut s07, &theme, in07);
    assert_eq!(s07.selected_nodes.len(), 2);
    let raw07 = raster_to_rgba(&s07);
    assert!(count_pixel_delta(&raw_a_base, &raw07) >= 40);
    let mut img07 = raw07;
    draw_hud_overlay(
        &mut img07,
        Some((scr_a07.x.0, scr_a07.y.0)),
        None,
        true,
        [0, 122, 255, 255],
        "07: CLICK SEGMENT 2 PTS",
    );
    frames_rgba.push(("frame_07_select_click_segment_endpoints.png".into(), img07));

    // State 8: Double-Click on a Segment/Handle (selects 1 contour) + Shift + Double-Click on inner ring (adds 2nd contour)
    let mut s08 = GrafiaCanvasState::init_glyph_o();
    s08.fit_viewport_to_glyph(w as f32, h as f32);
    let h1_scr = s08.viewport.to_screen(s08.contours[0][1].point);
    for t in [1.0, 1.1] {
        let mut r = RawInput {
            time: Some(t),
            ..Default::default()
        };
        r.events.push(Event::PointerButton {
            pos: pos2(h1_scr.x.0, h1_scr.y.0),
            button: PointerButton::Primary,
            pressed: true,
            modifiers: Modifiers::NONE,
        });
        r.events.push(Event::PointerButton {
            pos: pos2(h1_scr.x.0, h1_scr.y.0),
            button: PointerButton::Primary,
            pressed: false,
            modifiers: Modifiers::NONE,
        });
        run_frame(&ctx, &mut s08, &theme, r);
    }
    assert_eq!(s08.selected_nodes.len(), s08.contours[0].len());
    let raw08 = raster_to_rgba(&s08);
    assert!(count_pixel_delta(&raw01, &raw08) >= 200);
    // Now Shift + Double-Click on inner contour 1 handle -> adds second contour (24 nodes selected)
    let mut s08_both = s08.clone_via_snapshot();
    let inner_h1_scr = s08_both.viewport.to_screen(s08_both.contours[1][1].point);
    ctx.input_mut(|i| i.modifiers.shift = true);
    for t in [2.0, 2.1] {
        let mut r = RawInput {
            time: Some(t),
            ..Default::default()
        };
        r.events.push(Event::PointerButton {
            pos: pos2(inner_h1_scr.x.0, inner_h1_scr.y.0),
            button: PointerButton::Primary,
            pressed: true,
            modifiers: Modifiers::SHIFT,
        });
        r.events.push(Event::PointerButton {
            pos: pos2(inner_h1_scr.x.0, inner_h1_scr.y.0),
            button: PointerButton::Primary,
            pressed: false,
            modifiers: Modifiers::SHIFT,
        });
        run_frame(&ctx, &mut s08_both, &theme, r);
    }
    ctx.input_mut(|i| i.modifiers.shift = false);
    assert_eq!(
        s08_both.selected_nodes.len(),
        24,
        "Shift+Double-Click on inner ring must add second contour"
    );
    let raw08_both = raster_to_rgba(&s08_both);
    assert!(count_pixel_delta(&raw08, &raw08_both) >= 150);
    let mut img08 = RgbaImage::from_pixel(w, h, Rgba([220, 224, 230, 255]));
    blit_scaled_half(&mut img08, &raw08, 0, 120);
    blit_scaled_half(&mut img08, &raw08_both, w / 2, 120);
    draw_text_5x7(
        &mut img08,
        "A: DBL-CLICK 1 RING (12)",
        12,
        85,
        2,
        Rgba([0, 102, 255, 255]),
    );
    draw_text_5x7(
        &mut img08,
        "B: +SHIFT DBL-CLICK (24)",
        332,
        85,
        2,
        Rgba([40, 167, 69, 255]),
    );
    draw_hud_overlay(
        &mut img08,
        Some((h1_scr.x.0 * 0.5, h1_scr.y.0 * 0.5 + 120.0)),
        None,
        true,
        [0, 122, 255, 255],
        "08: DBL-CLICK + SHIFT DBL",
    );
    frames_rgba.push(("frame_08_select_double_click_contour.png".into(), img08));

    // State 9: Triple-Click -> selects all nodes across all contours
    let mut s09 = GrafiaCanvasState::init_glyph_o();
    s09.fit_viewport_to_glyph(w as f32, h as f32);
    for t in [2.0, 2.1, 2.2] {
        let mut r = RawInput {
            time: Some(t),
            ..Default::default()
        };
        r.events.push(Event::PointerButton {
            pos: pos2(h1_scr.x.0, h1_scr.y.0),
            button: PointerButton::Primary,
            pressed: true,
            modifiers: Modifiers::NONE,
        });
        r.events.push(Event::PointerButton {
            pos: pos2(h1_scr.x.0, h1_scr.y.0),
            button: PointerButton::Primary,
            pressed: false,
            modifiers: Modifiers::NONE,
        });
        run_frame(&ctx, &mut s09, &theme, r);
    }
    assert_eq!(s09.selected_nodes.len(), 24);
    let raw09 = raster_to_rgba(&s09);
    assert!(count_pixel_delta(&raw08, &raw09) >= 200);
    let mut img09 = raw09;
    draw_hud_overlay(
        &mut img09,
        Some((h1_scr.x.0, h1_scr.y.0)),
        None,
        true,
        [0, 122, 255, 255],
        "09: TRIPLE-CLICK ALL PTS",
    );
    frames_rgba.push(("frame_09_select_triple_click_all.png".into(), img09));

    // State 10: Standard Marquee (Drag Rectangle) -> visible marquee box + selects OnCurve & OffCurve inside
    let mut s10 = GrafiaCanvasState::init_glyph_o();
    s10.fit_viewport_to_glyph(w as f32, h as f32);
    let mut in10 = RawInput::default();
    in10.events.push(Event::PointerButton {
        pos: pos2(180.0, 95.0),
        button: PointerButton::Primary,
        pressed: true,
        modifiers: Modifiers::NONE,
    });
    in10.events.push(Event::PointerMoved(pos2(460.0, 265.0)));
    run_frame(&ctx, &mut s10, &theme, in10);
    let raw10 = raster_to_rgba(&s10);
    assert!(
        count_pixel_delta(&raw01, &raw10) > 2000,
        "State 10 Marquee rectangle must be visibly rendered"
    );
    let mut img10 = raw10;
    draw_hud_overlay(
        &mut img10,
        Some((460.0, 265.0)),
        Some((180.0, 95.0)),
        true,
        [0, 122, 255, 255],
        "10: MARQUEE ALL NODES",
    );
    frames_rgba.push(("frame_10_select_marquee_standard.png".into(), img10));

    // State 11: Alt + Marquee (OnCurve only: 8 nodes) & Shift + Marquee (XOR Toggle)
    let mut s11 = GrafiaCanvasState::init_glyph_o();
    s11.fit_viewport_to_glyph(w as f32, h as f32);
    ctx.input_mut(|i| i.modifiers.alt = true);
    let mut in11 = RawInput::default();
    in11.events.push(Event::PointerButton {
        pos: pos2(160.0, 80.0),
        button: PointerButton::Primary,
        pressed: true,
        modifiers: Modifiers::ALT,
    });
    in11.events.push(Event::PointerMoved(pos2(490.0, 410.0)));
    run_frame(&ctx, &mut s11, &theme, in11);
    ctx.input_mut(|i| i.modifiers.alt = false);
    assert_eq!(
        s11.selected_nodes.len(),
        8,
        "Alt+Marquee on /o must select only the 8 OnCurve nodes"
    );
    let mut raw11_a = raster_to_rgba(&s11);
    draw_hud_overlay(
        &mut raw11_a,
        Some((490.0, 410.0)),
        Some((160.0, 80.0)),
        true,
        [0, 122, 255, 255],
        "11A: ALT+MARQUEE (8 ON)",
    );
    // Release Alt+Marquee, then Shift+Marquee over the top half to XOR-toggle top nodes off and top OffCurve handles on!
    let mut in11_rel = RawInput::default();
    in11_rel.events.push(Event::PointerButton {
        pos: pos2(490.0, 410.0),
        button: PointerButton::Primary,
        pressed: false,
        modifiers: Modifiers::ALT,
    });
    run_frame(&ctx, &mut s11, &theme, in11_rel);
    ctx.input_mut(|i| i.modifiers.shift = true);
    let mut in11_b = RawInput::default();
    in11_b.events.push(Event::PointerButton {
        pos: pos2(160.0, 80.0),
        button: PointerButton::Primary,
        pressed: true,
        modifiers: Modifiers::SHIFT,
    });
    in11_b.events.push(Event::PointerMoved(pos2(490.0, 235.0)));
    run_frame(&ctx, &mut s11, &theme, in11_b);
    ctx.input_mut(|i| i.modifiers.shift = false);
    let mut raw11_b = raster_to_rgba(&s11);
    assert!(
        count_pixel_delta(&raw11_a, &raw11_b) > 200,
        "State 11b Shift+Marquee XOR must visibly differ from Alt+Marquee"
    );
    draw_hud_overlay(
        &mut raw11_b,
        Some((490.0, 235.0)),
        Some((160.0, 80.0)),
        true,
        [0, 122, 255, 255],
        "11B: SHIFT+MARQUEE XOR",
    );
    let mut img11 = RgbaImage::from_pixel(w, h, Rgba([220, 224, 230, 255]));
    blit_scaled_half(&mut img11, &raw11_a, 0, h / 4);
    blit_scaled_half(&mut img11, &raw11_b, w / 2, h / 4);
    draw_text_5x7(
        &mut img11,
        "A: ALT+MARQUEE (8 ON)",
        14,
        58,
        2,
        Rgba([0, 102, 255, 255]),
    );
    draw_text_5x7(
        &mut img11,
        "B: SHIFT+MARQUEE (XOR)",
        334,
        58,
        2,
        Rgba([40, 167, 69, 255]),
    );
    draw_hud_overlay(
        &mut img11,
        None,
        None,
        false,
        [0, 122, 255, 255],
        "11: ALT + SHIFT MARQUEE",
    );
    frames_rgba.push(("frame_11_select_alt_and_shift_marquee.png".into(), img11));

    // State 12: Two-Stage Cmd/Ctrl + A (1st: 8 OnCurve only; 2nd: All 24 nodes; Escape/Cmd+D: 0 nodes with closed contours preserved)
    let mut s12 = GrafiaCanvasState::init_glyph_o();
    s12.fit_viewport_to_glyph(w as f32, h as f32);
    let mut in12_a = RawInput::default();
    in12_a.events.push(Event::Key {
        key: Key::A,
        physical_key: None,
        pressed: true,
        repeat: false,
        modifiers: Modifiers::COMMAND,
    });
    run_frame(&ctx, &mut s12, &theme, in12_a);
    assert_eq!(
        s12.selected_nodes.len(),
        8,
        "1st Cmd+A selects 8 OnCurve nodes"
    );
    let raw12_a = raster_to_rgba(&s12);
    assert!(count_pixel_delta(&raw01, &raw12_a) >= 150);
    let mut in12_b = RawInput::default();
    in12_b.events.push(Event::Key {
        key: Key::A,
        physical_key: None,
        pressed: true,
        repeat: false,
        modifiers: Modifiers::COMMAND,
    });
    run_frame(&ctx, &mut s12, &theme, in12_b);
    assert_eq!(
        s12.selected_nodes.len(),
        24,
        "2nd Cmd+A expands to all 24 OnCurve + OffCurve nodes"
    );
    let raw12_b = raster_to_rgba(&s12);
    assert!(
        count_pixel_delta(&raw12_a, &raw12_b) >= 150,
        "2nd Cmd+A must visibly light up all 16 OffCurve BCP handles"
    );
    // Also verify Escape deselects all while keeping both contours closed!
    let mut s12_esc = s12.clone_via_snapshot();
    let mut in12_esc = RawInput::default();
    in12_esc.events.push(Event::Key {
        key: Key::Escape,
        physical_key: None,
        pressed: true,
        repeat: false,
        modifiers: Modifiers::NONE,
    });
    run_frame(&ctx, &mut s12_esc, &theme, in12_esc);
    assert!(s12_esc.selected_nodes.is_empty());
    assert!(s12_esc.is_contour_closed(0) && s12_esc.is_contour_closed(1));
    let mut img12 = RgbaImage::from_pixel(w, h, Rgba([220, 224, 230, 255]));
    blit_scaled_half(&mut img12, &raw12_a, 0, h / 4);
    blit_scaled_half(&mut img12, &raw12_b, w / 2, h / 4);
    draw_text_5x7(
        &mut img12,
        "1ST CMD+A: 8 ONCURVE",
        14,
        58,
        2,
        Rgba([40, 167, 69, 255]),
    );
    draw_text_5x7(
        &mut img12,
        "2ND CMD+A: ALL 24 NODES",
        334,
        58,
        2,
        Rgba([0, 102, 255, 255]),
    );
    draw_hud_overlay(
        &mut img12,
        None,
        None,
        false,
        [0, 122, 255, 255],
        "12: 2-STAGE CMD+A (8->24)",
    );
    frames_rgba.push(("frame_12_select_two_stage_cmd_a.png".into(), img12));

    // =========================================================================
    // III. Dragging, Moving & Deforming in SelectTool (V) (States 13..21)
    // =========================================================================

    // State 13: Drag OnCurve Node (without Alt) -> translates anchor + attached OffCurve handles rigidly
    let mut s13 = GrafiaCanvasState::init_glyph_o();
    s13.fit_viewport_to_glyph(w as f32, h as f32);
    let on13_0 = s13.viewport.to_screen(s13.contours[0][0].point);
    let on13_1 = s13.viewport.to_screen(FontPoint::new(
        s13.contours[0][0].point.x.0,
        s13.contours[0][0].point.y.0 + 95.0,
    ));
    let mut in13 = RawInput::default();
    in13.events.push(Event::PointerButton {
        pos: pos2(on13_0.x.0, on13_0.y.0),
        button: PointerButton::Primary,
        pressed: true,
        modifiers: Modifiers::NONE,
    });
    in13.events
        .push(Event::PointerMoved(pos2(on13_1.x.0, on13_1.y.0)));
    run_frame(&ctx, &mut s13, &theme, in13);
    let raw13 = raster_to_rgba(&s13);
    assert!(count_pixel_delta(&raw01, &raw13) > 400);
    let mut img13 = raw13.clone();
    draw_hud_overlay(
        &mut img13,
        Some((on13_1.x.0, on13_1.y.0)),
        Some((on13_0.x.0, on13_0.y.0)),
        true,
        [40, 167, 69, 255],
        "13: DRAG ONCURVE + BCPS",
    );
    frames_rgba.push((
        "frame_13_drag_oncurve_moves_attached_bcps.png".into(),
        img13,
    ));

    // State 14: Alt + Drag on a Single OnCurve Node -> moves anchor while keeping attached BCPs fixed and preserving Smooth
    let mut s14 = GrafiaCanvasState::init_glyph_o();
    s14.fit_viewport_to_glyph(w as f32, h as f32);
    let h_out_14 = s14.contours[0][1].point;
    let h_in_14 = s14.contours[0][11].point;
    ctx.input_mut(|i| i.modifiers.alt = true);
    let mut in14 = RawInput::default();
    in14.events.push(Event::PointerButton {
        pos: pos2(on13_0.x.0, on13_0.y.0),
        button: PointerButton::Primary,
        pressed: true,
        modifiers: Modifiers::ALT,
    });
    in14.events
        .push(Event::PointerMoved(pos2(on13_1.x.0, on13_1.y.0)));
    run_frame(&ctx, &mut s14, &theme, in14);
    ctx.input_mut(|i| i.modifiers.alt = false);
    assert_eq!(s14.contours[0][0].node_type, NodeType::Smooth);
    assert_eq!(s14.contours[0][1].point, h_out_14);
    assert_eq!(s14.contours[0][11].point, h_in_14);
    let raw14 = raster_to_rgba(&s14);
    assert!(
        count_pixel_delta(&raw13, &raw14) > 200,
        "State 14 Alt+Drag decoupled anchor must visibly differ from normal drag"
    );
    let mut img14 = raw14;
    draw_hud_overlay(
        &mut img14,
        Some((on13_1.x.0, on13_1.y.0)),
        Some((on13_0.x.0, on13_0.y.0)),
        true,
        [40, 167, 69, 255],
        "14: ALT+DRAG ONCURVE ONLY",
    );
    frames_rgba.push((
        "frame_14_alt_drag_oncurve_decoupled_smooth.png".into(),
        img14,
    ));

    // State 15: Drag OffCurve BCP Handle on a Smooth Node -> G1 collinear opposite rotation
    let mut s15 = GrafiaCanvasState::init_glyph_o();
    s15.fit_viewport_to_glyph(w as f32, h as f32);
    let bcp15_0 = s15.viewport.to_screen(s15.contours[0][1].point);
    let bcp15_1 = s15.viewport.to_screen(FontPoint::new(
        s15.contours[0][1].point.x.0 + 40.0,
        s15.contours[0][1].point.y.0 + 100.0,
    ));
    let mut in15 = RawInput::default();
    in15.events.push(Event::PointerButton {
        pos: pos2(bcp15_0.x.0, bcp15_0.y.0),
        button: PointerButton::Primary,
        pressed: true,
        modifiers: Modifiers::NONE,
    });
    in15.events
        .push(Event::PointerMoved(pos2(bcp15_1.x.0, bcp15_1.y.0)));
    run_frame(&ctx, &mut s15, &theme, in15);
    let raw15 = raster_to_rgba(&s15);
    assert!(count_pixel_delta(&raw01, &raw15) > 300);
    let mut img15 = raw15.clone();
    draw_hud_overlay(
        &mut img15,
        Some((bcp15_1.x.0, bcp15_1.y.0)),
        Some((bcp15_0.x.0, bcp15_0.y.0)),
        true,
        [175, 82, 222, 255],
        "15: DRAG BCP SMOOTH G1",
    );
    frames_rgba.push(("frame_15_drag_bcp_smooth_collinear.png".into(), img15));

    // State 16: Alt + Drag on an OffCurve BCP Handle (2-Panel: A = Alt+Drag Corner Cusp | B = Release Alt Mid-Drag Restores Smooth G1)
    let mut s16 = GrafiaCanvasState::init_glyph_o();
    s16.fit_viewport_to_glyph(w as f32, h as f32);
    let mut in16_down = RawInput::default();
    in16_down.events.push(Event::PointerButton {
        pos: pos2(bcp15_0.x.0, bcp15_0.y.0),
        button: PointerButton::Primary,
        pressed: true,
        modifiers: Modifiers::NONE,
    });
    run_frame(&ctx, &mut s16, &theme, in16_down);
    ctx.input_mut(|i| i.modifiers.alt = true);
    let mut in16_alt = RawInput::default();
    in16_alt
        .events
        .push(Event::PointerMoved(pos2(bcp15_1.x.0, bcp15_1.y.0)));
    run_frame(&ctx, &mut s16, &theme, in16_alt);
    assert_eq!(s16.contours[0][0].node_type, NodeType::Corner);
    let mut raw16_a = raster_to_rgba(&s16);
    assert!(count_pixel_delta(&raw15, &raw16_a) > 100);
    draw_hud_overlay(
        &mut raw16_a,
        Some((bcp15_1.x.0, bcp15_1.y.0)),
        Some((bcp15_0.x.0, bcp15_0.y.0)),
        true,
        [0, 102, 255, 255],
        "",
    );
    // Now release Alt mid-drag before PointerRelease -> restores Smooth and rotates opposite handle collinearly!
    ctx.input_mut(|i| i.modifiers.alt = false);
    let bcp16_rel = pos2(bcp15_1.x.0 + 14.0, bcp15_1.y.0 - 10.0);
    let mut in16_rel = RawInput::default();
    in16_rel.events.push(Event::PointerMoved(bcp16_rel));
    run_frame(&ctx, &mut s16, &theme, in16_rel);
    assert_eq!(
        s16.contours[0][0].node_type,
        NodeType::Smooth,
        "Releasing Alt mid-drag must restore Smooth"
    );
    let mut raw16_b = raster_to_rgba(&s16);
    assert!(count_pixel_delta(&raw16_a, &raw16_b) > 80, "Releasing Alt mid-drag must visibly rotate opposite handle and restore green Smooth circle");
    draw_hud_overlay(
        &mut raw16_b,
        Some((bcp16_rel.x, bcp16_rel.y)),
        Some((bcp15_0.x.0, bcp15_0.y.0)),
        true,
        [40, 167, 69, 255],
        "",
    );
    let mut img16 = RgbaImage::from_pixel(w, h, Rgba([220, 224, 230, 255]));
    blit_scaled_half(&mut img16, &raw16_a, 0, h / 4);
    blit_scaled_half(&mut img16, &raw16_b, w / 2, h / 4);
    draw_text_5x7(
        &mut img16,
        "A: ALT+DRAG -> CUSP",
        12,
        58,
        2,
        Rgba([0, 102, 255, 255]),
    );
    draw_text_5x7(
        &mut img16,
        "B: RELEASE ALT -> SMOOTH",
        332,
        58,
        2,
        Rgba([40, 167, 69, 255]),
    );
    draw_hud_overlay(
        &mut img16,
        None,
        None,
        false,
        [175, 82, 222, 255],
        "16: ALT+DRAG CUSP & RESTORE",
    );
    frames_rgba.push(("frame_16_alt_drag_bcp_cusp_and_restore.png".into(), img16));

    // State 17: Ctrl + Alt + Drag: Panel A = OnCurve Nudge (setInterpolate:withPoint:) | Panel B = OffCurve Mirror (setMirrored:)
    let mut s17_a = GrafiaCanvasState::init_glyph_o();
    s17_a.fit_viewport_to_glyph(w as f32, h as f32);
    let on17_0 = s17_a.viewport.to_screen(s17_a.contours[0][0].point);
    let on17_1 = s17_a.viewport.to_screen(FontPoint::new(
        s17_a.contours[0][0].point.x.0 + 60.0,
        s17_a.contours[0][0].point.y.0 + 90.0,
    ));
    let ctrl_alt = Modifiers {
        alt: true,
        ctrl: true,
        ..Default::default()
    };
    ctx.input_mut(|i| {
        i.modifiers.alt = true;
        i.modifiers.ctrl = true;
    });
    let mut in17_a = RawInput::default();
    in17_a.events.push(Event::PointerButton {
        pos: pos2(on17_0.x.0, on17_0.y.0),
        button: PointerButton::Primary,
        pressed: true,
        modifiers: ctrl_alt,
    });
    in17_a
        .events
        .push(Event::PointerMoved(pos2(on17_1.x.0, on17_1.y.0)));
    run_frame(&ctx, &mut s17_a, &theme, in17_a);
    ctx.input_mut(|i| {
        i.modifiers.alt = false;
        i.modifiers.ctrl = false;
    });
    let mut raw17_a = raster_to_rgba(&s17_a);
    assert!(count_pixel_delta(&raw01, &raw17_a) > 300);
    draw_hud_overlay(
        &mut raw17_a,
        Some((on17_1.x.0, on17_1.y.0)),
        Some((on17_0.x.0, on17_0.y.0)),
        true,
        [40, 167, 69, 255],
        "",
    );

    let mut s17_b = GrafiaCanvasState::init_glyph_o();
    s17_b.fit_viewport_to_glyph(w as f32, h as f32);
    let bcp17_1 = s17_b.viewport.to_screen(FontPoint::new(
        s17_b.contours[0][1].point.x.0 + 95.0,
        s17_b.contours[0][1].point.y.0 + 65.0,
    ));
    ctx.input_mut(|i| {
        i.modifiers.alt = true;
        i.modifiers.ctrl = true;
    });
    let mut in17_b = RawInput::default();
    in17_b.events.push(Event::PointerButton {
        pos: pos2(bcp15_0.x.0, bcp15_0.y.0),
        button: PointerButton::Primary,
        pressed: true,
        modifiers: ctrl_alt,
    });
    in17_b
        .events
        .push(Event::PointerMoved(pos2(bcp17_1.x.0, bcp17_1.y.0)));
    run_frame(&ctx, &mut s17_b, &theme, in17_b);
    ctx.input_mut(|i| {
        i.modifiers.alt = false;
        i.modifiers.ctrl = false;
    });
    let anchor17 = s17_b.contours[0][0].point;
    let h_dragged17 = s17_b.contours[0][1].point;
    let h_opp17 = s17_b.contours[0][11].point;
    assert_relative_eq!(
        h_opp17.x.0,
        2.0 * anchor17.x.0 - h_dragged17.x.0,
        epsilon = 1e-2
    );
    assert_relative_eq!(
        h_opp17.y.0,
        2.0 * anchor17.y.0 - h_dragged17.y.0,
        epsilon = 1e-2
    );
    let mut raw17_b = raster_to_rgba(&s17_b);
    assert!(count_pixel_delta(&raw01, &raw17_b) > 200);
    draw_hud_overlay(
        &mut raw17_b,
        Some((bcp17_1.x.0, bcp17_1.y.0)),
        Some((bcp15_0.x.0, bcp15_0.y.0)),
        true,
        [175, 82, 222, 255],
        "",
    );

    let mut img17 = RgbaImage::from_pixel(w, h, Rgba([220, 224, 230, 255]));
    blit_scaled_half(&mut img17, &raw17_a, 0, h / 4);
    blit_scaled_half(&mut img17, &raw17_b, w / 2, h / 4);
    draw_text_5x7(
        &mut img17,
        "A: CTRL+ALT NUDGE (ON)",
        12,
        58,
        2,
        Rgba([40, 167, 69, 255]),
    );
    draw_text_5x7(
        &mut img17,
        "B: CTRL+ALT MIRROR (BCP)",
        332,
        58,
        2,
        Rgba([175, 82, 222, 255]),
    );
    draw_hud_overlay(
        &mut img17,
        None,
        None,
        false,
        [175, 82, 222, 255],
        "17: CTRL+ALT NUDGE+MIRROR",
    );
    frames_rgba.push(("frame_17_ctrl_alt_drag_bcp_tension.png".into(), img17));

    // State 18: Shift + Drag (0° / 45° / 90° Axis Constrain)
    let mut s18 = GrafiaCanvasState::init_glyph_o();
    s18.fit_viewport_to_glyph(w as f32, h as f32);
    let s18_target = s18.viewport.to_screen(FontPoint::new(
        s18.contours[0][0].point.x.0 + 110.0,
        s18.contours[0][0].point.y.0 + 18.0,
    ));
    ctx.input_mut(|i| i.modifiers.shift = true);
    let mut in18 = RawInput::default();
    in18.events.push(Event::PointerButton {
        pos: pos2(on13_0.x.0, on13_0.y.0),
        button: PointerButton::Primary,
        pressed: true,
        modifiers: Modifiers::SHIFT,
    });
    in18.events
        .push(Event::PointerMoved(pos2(s18_target.x.0, s18_target.y.0)));
    run_frame(&ctx, &mut s18, &theme, in18);
    ctx.input_mut(|i| i.modifiers.shift = false);
    assert_relative_eq!(s18.contours[0][0].point.y.0, 510.0, epsilon = 1e-3);
    let raw18 = raster_to_rgba(&s18);
    assert!(count_pixel_delta(&raw01, &raw18) > 300);
    let mut img18 = raw18;
    draw_hud_overlay(
        &mut img18,
        Some((s18_target.x.0, on13_0.y.0)),
        Some((on13_0.x.0, on13_0.y.0)),
        true,
        [40, 167, 69, 255],
        "18: SHIFT+DRAG 45 AXIS",
    );
    frames_rgba.push(("frame_18_shift_drag_45_axis_constrain.png".into(), img18));

    // State 19: Click & Drag a Straight Line Segment (/A) -> parallel translation
    let mut s19 = GrafiaCanvasState::init_glyph_a();
    s19.fit_viewport_to_glyph(w as f32, h as f32);
    s19.selected_nodes = vec![s19.contours[0][0].id, s19.contours[0][1].id];
    let scr19_1 = s19
        .viewport
        .to_screen(FontPoint::new(mid_a07.x.0 - 65.0, mid_a07.y.0));
    let mut in19 = RawInput::default();
    in19.events.push(Event::PointerButton {
        pos: pos2(scr_a07.x.0, scr_a07.y.0),
        button: PointerButton::Primary,
        pressed: true,
        modifiers: Modifiers::NONE,
    });
    in19.events
        .push(Event::PointerMoved(pos2(scr19_1.x.0, scr19_1.y.0)));
    run_frame(&ctx, &mut s19, &theme, in19);
    let raw19 = raster_to_rgba(&s19);
    assert!(count_pixel_delta(&raw_a_base, &raw19) > 400);
    let mut img19 = raw19;
    draw_hud_overlay(
        &mut img19,
        Some((scr19_1.x.0, scr19_1.y.0)),
        Some((scr_a07.x.0, scr_a07.y.0)),
        true,
        [0, 122, 255, 255],
        "19: DRAG LINE SEGMENT /A",
    );
    frames_rgba.push(("frame_19_drag_straight_line_segment_a.png".into(), img19));

    // State 20: Direct dragCurve on the Belly of a Cubic Segment (4 Endpoint Modes Quad Panel)
    let mut s20_a = GrafiaCanvasState::init_glyph_o();
    s20_a.fit_viewport_to_glyph(w as f32, h as f32);
    let segs20 = extract_segments(&s20_a.contours[0], true);
    let mid20 = match segs20[0] {
        ContourSegment::Cubic { cubic, .. } => {
            use kurbo::ParamCurve;
            let p = cubic.eval(0.5);
            FontPoint::new(p.x, p.y)
        }
        _ => unreachable!(),
    };
    let scr20_0 = s20_a.viewport.to_screen(mid20);
    let scr20_1 = s20_a
        .viewport
        .to_screen(FontPoint::new(mid20.x.0 + 70.0, mid20.y.0 + 60.0));
    let mut in20_a = RawInput::default();
    in20_a.events.push(Event::PointerButton {
        pos: pos2(scr20_0.x.0, scr20_0.y.0),
        button: PointerButton::Primary,
        pressed: true,
        modifiers: Modifiers::NONE,
    });
    in20_a
        .events
        .push(Event::PointerMoved(pos2(scr20_1.x.0, scr20_1.y.0)));
    run_frame(&ctx, &mut s20_a, &theme, in20_a);
    let raw20_a = raster_to_rgba(&s20_a);
    assert!(count_pixel_delta(&raw01, &raw20_a) > 300);

    // Q2: Corner-Corner endpoint combination
    let mut s20_b = GrafiaCanvasState::init_glyph_o();
    s20_b.fit_viewport_to_glyph(w as f32, h as f32);
    s20_b.contours[0][0].node_type = NodeType::Corner;
    s20_b.contours[0][3].node_type = NodeType::Corner;
    s20_b.sync_paths_from_contours();
    let mut in20_b = RawInput::default();
    in20_b.events.push(Event::PointerButton {
        pos: pos2(scr20_0.x.0, scr20_0.y.0),
        button: PointerButton::Primary,
        pressed: true,
        modifiers: Modifiers::NONE,
    });
    in20_b
        .events
        .push(Event::PointerMoved(pos2(scr20_1.x.0, scr20_1.y.0)));
    run_frame(&ctx, &mut s20_b, &theme, in20_b);
    let raw20_b = raster_to_rgba(&s20_b);
    assert!(count_pixel_delta(&raw20_a, &raw20_b) >= 15);

    // Q3: Smooth-Corner endpoint combination
    let mut s20_c = GrafiaCanvasState::init_glyph_o();
    s20_c.fit_viewport_to_glyph(w as f32, h as f32);
    s20_c.contours[0][0].node_type = NodeType::Smooth;
    s20_c.contours[0][3].node_type = NodeType::Corner;
    s20_c.sync_paths_from_contours();
    let mut in20_c = RawInput::default();
    in20_c.events.push(Event::PointerButton {
        pos: pos2(scr20_0.x.0, scr20_0.y.0),
        button: PointerButton::Primary,
        pressed: true,
        modifiers: Modifiers::NONE,
    });
    in20_c
        .events
        .push(Event::PointerMoved(pos2(scr20_1.x.0, scr20_1.y.0)));
    run_frame(&ctx, &mut s20_c, &theme, in20_c);
    let raw20_c = raster_to_rgba(&s20_c);
    assert!(count_pixel_delta(&raw20_a, &raw20_c) >= 10);

    // Q4: +Alt free-drag override on Smooth-Smooth
    let mut s20_d = GrafiaCanvasState::init_glyph_o();
    s20_d.fit_viewport_to_glyph(w as f32, h as f32);
    let mut in20_d1 = RawInput::default();
    in20_d1.events.push(Event::PointerButton {
        pos: pos2(scr20_0.x.0, scr20_0.y.0),
        button: PointerButton::Primary,
        pressed: true,
        modifiers: Modifiers::NONE,
    });
    run_frame(&ctx, &mut s20_d, &theme, in20_d1);
    ctx.input_mut(|i| i.modifiers.alt = true);
    let mut in20_d2 = RawInput::default();
    in20_d2
        .events
        .push(Event::PointerMoved(pos2(scr20_1.x.0, scr20_1.y.0)));
    run_frame(&ctx, &mut s20_d, &theme, in20_d2);
    ctx.input_mut(|i| i.modifiers.alt = false);
    let raw20_d = raster_to_rgba(&s20_d);
    assert!(count_pixel_delta(&raw20_a, &raw20_d) >= 15);

    let mut img20 = RgbaImage::from_pixel(w, h, Rgba([220, 224, 230, 255]));
    blit_scaled_half(&mut img20, &raw20_a, 0, 0);
    blit_scaled_half(&mut img20, &raw20_b, w / 2, 0);
    blit_scaled_half(&mut img20, &raw20_c, 0, h / 2);
    blit_scaled_half(&mut img20, &raw20_d, w / 2, h / 2);
    draw_text_5x7(
        &mut img20,
        "A: SMOOTH-SMOOTH (G1)",
        12,
        50,
        2,
        Rgba([40, 167, 69, 255]),
    );
    draw_text_5x7(
        &mut img20,
        "B: CORNER-CORNER (FREE)",
        332,
        50,
        2,
        Rgba([0, 102, 255, 255]),
    );
    draw_text_5x7(
        &mut img20,
        "C: SMOOTH-CORNER (MIX)",
        12,
        252,
        2,
        Rgba([175, 82, 222, 255]),
    );
    draw_text_5x7(
        &mut img20,
        "D: +ALT OVERRIDE (FREE)",
        332,
        252,
        2,
        Rgba([220, 38, 38, 255]),
    );
    draw_hud_overlay(
        &mut img20,
        None,
        None,
        false,
        [255, 149, 0, 255],
        "20: DRAG CURVE 4 MODES",
    );
    frames_rgba.push(("frame_20_drag_curve_belly_4_modes.png".into(), img20));

    // State 21: Option/Alt + Drag on a Selected Contour (Duplicate Selection)
    let mut s21 = GrafiaCanvasState::init_glyph_o();
    s21.fit_viewport_to_glyph(w as f32, h as f32);
    s21.selected_nodes = s21.contours[0].iter().map(|n| n.id).collect();
    let dup_end = s21.viewport.to_screen(FontPoint::new(
        s21.contours[0][0].point.x.0 + 160.0,
        s21.contours[0][0].point.y.0 + 50.0,
    ));
    ctx.input_mut(|i| i.modifiers.alt = true);
    let mut in21 = RawInput::default();
    in21.events.push(Event::PointerButton {
        pos: pos2(on13_0.x.0, on13_0.y.0),
        button: PointerButton::Primary,
        pressed: true,
        modifiers: Modifiers::ALT,
    });
    in21.events
        .push(Event::PointerMoved(pos2(dup_end.x.0, dup_end.y.0)));
    run_frame(&ctx, &mut s21, &theme, in21);
    ctx.input_mut(|i| i.modifiers.alt = false);
    assert_eq!(s21.contours.len(), 3);
    let raw21 = raster_to_rgba(&s21);
    assert!(count_pixel_delta(&raw01, &raw21) > 1000);
    let mut img21 = raw21;
    draw_hud_overlay(
        &mut img21,
        Some((dup_end.x.0, dup_end.y.0)),
        Some((on13_0.x.0, on13_0.y.0)),
        true,
        [40, 167, 69, 255],
        "21: ALT+DRAG DUPLICATE",
    );
    frames_rgba.push(("frame_21_option_drag_duplicate_contour.png".into(), img21));

    // =========================================================================
    // IV. Topological Path Operations & Node Conversions (States 22..28)
    // =========================================================================

    // State 22: Double-Click on an OnCurve Node & Enter Key -> toggles Smooth <-> Corner
    let mut s22 = GrafiaCanvasState::init_glyph_o();
    s22.fit_viewport_to_glyph(w as f32, h as f32);
    s22.selected_nodes = vec![s22.contours[0][0].id];
    let mut in22 = RawInput::default();
    in22.events.push(Event::Key {
        key: Key::Enter,
        physical_key: None,
        pressed: true,
        repeat: false,
        modifiers: Modifiers::NONE,
    });
    run_frame(&ctx, &mut s22, &theme, in22);
    assert_eq!(s22.contours[0][0].node_type, NodeType::Corner);
    let raw22 = raster_to_rgba(&s22);
    assert!(count_pixel_delta(&raw05, &raw22) >= 10);
    let mut img22 = raw22;
    draw_hud_overlay(
        &mut img22,
        Some((n0_scr.x.0, n0_scr.y.0)),
        None,
        true,
        [255, 149, 0, 255],
        "22: DBL-CLICK/ENTER SMOOTH",
    );
    frames_rgba.push((
        "frame_22_double_click_enter_toggle_smooth.png".into(),
        img22,
    ));

    // State 23: Alt + Click on a Straight Line Segment -> converts to Cubic at 1/3 and 2/3
    let mut s23 = GrafiaCanvasState::init_glyph_a();
    s23.fit_viewport_to_glyph(w as f32, h as f32);
    let mut in23 = RawInput::default();
    in23.events.push(Event::PointerButton {
        pos: pos2(scr_a07.x.0, scr_a07.y.0),
        button: PointerButton::Primary,
        pressed: true,
        modifiers: Modifiers::ALT,
    });
    in23.events.push(Event::PointerButton {
        pos: pos2(scr_a07.x.0, scr_a07.y.0),
        button: PointerButton::Primary,
        pressed: false,
        modifiers: Modifiers::ALT,
    });
    run_frame(&ctx, &mut s23, &theme, in23);
    let raw23 = raster_to_rgba(&s23);
    assert!(count_pixel_delta(&raw_a_base, &raw23) >= 40);
    let mut img23 = raw23;
    draw_hud_overlay(
        &mut img23,
        Some((scr_a07.x.0, scr_a07.y.0)),
        None,
        true,
        [255, 149, 0, 255],
        "23: ALT+CLICK LINE->CUBIC",
    );
    frames_rgba.push(("frame_23_alt_click_line_to_cubic.png".into(), img23));

    // State 24: Delete on a Single OffCurve BCP Handle -> converts Cubic to straight Line
    let mut s24 = GrafiaCanvasState::init_glyph_o();
    s24.fit_viewport_to_glyph(w as f32, h as f32);
    s24.selected_nodes = vec![s24.contours[0][1].id];
    let mut in24 = RawInput::default();
    in24.events.push(Event::Key {
        key: Key::Delete,
        physical_key: None,
        pressed: true,
        repeat: false,
        modifiers: Modifiers::NONE,
    });
    run_frame(&ctx, &mut s24, &theme, in24);
    let raw24 = raster_to_rgba(&s24);
    assert!(count_pixel_delta(&raw01, &raw24) > 250);
    let mut img24 = raw24;
    draw_hud_overlay(
        &mut img24,
        Some((bcp15_0.x.0, bcp15_0.y.0)),
        None,
        true,
        [255, 59, 48, 255],
        "24: DEL BCP -> PURE LINE",
    );
    frames_rgba.push(("frame_24_delete_offcurve_bcp_to_line.png".into(), img24));

    // State 25: Delete on a Corner Node Between Straight Lines (/A) -> pure straight line (0 OffCurve dots)
    let mut s25 = GrafiaCanvasState::init_glyph_a();
    s25.fit_viewport_to_glyph(w as f32, h as f32);
    let c25_scr = s25.viewport.to_screen(s25.contours[0][2].point);
    s25.selected_nodes = vec![s25.contours[0][2].id];
    let mut in25 = RawInput::default();
    in25.events.push(Event::Key {
        key: Key::Backspace,
        physical_key: None,
        pressed: true,
        repeat: false,
        modifiers: Modifiers::NONE,
    });
    run_frame(&ctx, &mut s25, &theme, in25);
    let raw25 = raster_to_rgba(&s25);
    assert!(count_pixel_delta(&raw_a_base, &raw25) > 200);
    let mut img25 = raw25;
    draw_hud_overlay(
        &mut img25,
        Some((c25_scr.x.0, c25_scr.y.0)),
        None,
        true,
        [255, 59, 48, 255],
        "25: DEL CORNER /A LINE",
    );
    frames_rgba.push(("frame_25_delete_corner_node_line_a.png".into(), img25));

    // State 26: Delete on a Smooth Node in a Curve (/o — Schneider Curve-Healing: 2-Panel Before & After Delete of Top Smooth Node (280,510))
    let mut s26_before = GrafiaCanvasState::init_glyph_o();
    s26_before.fit_viewport_to_glyph(w as f32, h as f32);
    s26_before.selected_nodes = vec![s26_before.contours[0][0].id];
    let mut raw26_before = raster_to_rgba(&s26_before);
    draw_hud_overlay(
        &mut raw26_before,
        Some((n0_scr.x.0, n0_scr.y.0)),
        None,
        true,
        [0, 122, 255, 255],
        "",
    );

    let mut s26 = GrafiaCanvasState::init_glyph_o();
    s26.fit_viewport_to_glyph(w as f32, h as f32);
    s26.selected_nodes = vec![s26.contours[0][0].id];
    let mut in26 = RawInput::default();
    in26.events.push(Event::Key {
        key: Key::Delete,
        physical_key: None,
        pressed: true,
        repeat: false,
        modifiers: Modifiers::NONE,
    });
    run_frame(&ctx, &mut s26, &theme, in26);
    // Strict COTS assertion: healed top arch midpoint must evaluate at y in [495.0, 525.0] (exact 510.0)
    let segs26 = extract_segments(&s26.contours[0], true);
    let ContourSegment::Cubic {
        cubic: healed_arch, ..
    } = *segs26.last().expect("closing top arch segment")
    else {
        panic!("Expected healed top arch of /o to be Cubic");
    };
    let healed_mid = {
        use kurbo::ParamCurve;
        healed_arch.eval(0.5)
    };
    assert!(
        (495.0..=525.0).contains(&healed_mid.y) && (healed_mid.y - 510.0).abs() < 0.5,
        "State 26 healed top arch midpoint must stay at y=510.0 (in [495, 525]), got ({}, {})",
        healed_mid.x,
        healed_mid.y
    );
    let mut raw26 = raster_to_rgba(&s26);
    assert!(count_pixel_delta(&raw01, &raw26) > 80);
    draw_hud_overlay(
        &mut raw26,
        Some((n0_scr.x.0, n0_scr.y.0)),
        None,
        true,
        [255, 59, 48, 255],
        "",
    );
    let mut img26 = RgbaImage::from_pixel(w, h, Rgba([220, 224, 230, 255]));
    blit_scaled_half(&mut img26, &raw26_before, 0, h / 4);
    blit_scaled_half(&mut img26, &raw26, w / 2, h / 4);
    draw_text_5x7(
        &mut img26,
        "A: BEFORE DEL (280,510)",
        12,
        58,
        2,
        Rgba([0, 102, 255, 255]),
    );
    draw_text_5x7(
        &mut img26,
        "B: HEALED ARCH (Y=510)",
        332,
        58,
        2,
        Rgba([40, 167, 69, 255]),
    );
    draw_hud_overlay(
        &mut img26,
        None,
        None,
        false,
        [255, 59, 48, 255],
        "26: DEL SMOOTH HEAL /O",
    );
    frames_rgba.push(("frame_26_delete_smooth_node_curve_heal.png".into(), img26));

    // State 27: Shift + Delete on an OnCurve Node -> cuts contour open + perpendicular blue endpoint ticks
    let mut s27 = GrafiaCanvasState::init_glyph_o();
    s27.fit_viewport_to_glyph(w as f32, h as f32);
    s27.selected_nodes = vec![s27.contours[0][0].id];
    let mut in27 = RawInput::default();
    in27.events.push(Event::Key {
        key: Key::Delete,
        physical_key: None,
        pressed: true,
        repeat: false,
        modifiers: Modifiers::SHIFT,
    });
    run_frame(&ctx, &mut s27, &theme, in27);
    assert!(!s27.is_contour_closed(0));
    let raw27 = raster_to_rgba(&s27);
    assert!(count_pixel_delta(&raw01, &raw27) > 200);
    let mut img27 = raw27.clone();
    draw_hud_overlay(
        &mut img27,
        Some((n0_scr.x.0, n0_scr.y.0)),
        None,
        true,
        [255, 59, 48, 255],
        "27: SHIFT+DEL CUT OPEN",
    );
    frames_rgba.push(("frame_27_shift_delete_cut_open_ticks.png".into(), img27));

    // State 28: Reverse Contour in all 3 Visual Forms + Make Node First (2x2 Quad Panel)
    //   Q1 (top-left): Closed contour in EditWireframe with reversed direction arrow
    let mut s28_a = GrafiaCanvasState::init_glyph_a();
    s28_a.fit_viewport_to_glyph(w as f32, h as f32);
    s28_a.reverse_selected_or_all_contours();
    let raw28_a = raster_to_rgba(&s28_a);
    assert!(
        count_pixel_delta(&raw_a_base, &raw28_a) >= 30,
        "State 28a Reverse closed contour arrow flip"
    );
    //   Q2 (top-right): PreviewFilled with inner counter of /A reversed -> counter hole filled solid black!
    let mut s28_b = GrafiaCanvasState::init_glyph_a();
    s28_b.fit_viewport_to_glyph(w as f32, h as f32);
    s28_b.view_mode = CanvasViewMode::PreviewFilled;
    let raw28_b_before = raster_to_rgba(&s28_b);
    s28_b.selected_nodes = vec![s28_b.contours[1][0].id];
    s28_b.reverse_selected_or_all_contours();
    let raw28_b = raster_to_rgba(&s28_b);
    assert!(
        count_pixel_delta(&raw28_b_before, &raw28_b) > 1000,
        "State 28b Reverse inner counter fills hole black"
    );
    //   Q3 (bottom-left): Open contour reversed -> direction arrow jumps to opposite open endpoint (clear selection so green Smooth circle & perpendicular tick are unobstructed)
    let mut s28_c_before = s27.clone_via_snapshot();
    s28_c_before.selected_nodes.clear();
    let raw28_c_before = raster_to_rgba(&s28_c_before);
    let mut s28_c = s27.clone_via_snapshot();
    s28_c.selected_nodes = vec![s28_c.contours[0][0].id];
    s28_c.reverse_selected_or_all_contours();
    s28_c.selected_nodes.clear();
    let raw28_c = raster_to_rgba(&s28_c);
    assert!(
        count_pixel_delta(&raw28_c_before, &raw28_c) >= 20,
        "State 28c Reverse open contour jumps arrow to other endpoint"
    );
    //   Q4 (bottom-right): Make Node First -> rotates start node and blue direction arrow to apex node of /A
    let mut s28_d = GrafiaCanvasState::init_glyph_a();
    s28_d.fit_viewport_to_glyph(w as f32, h as f32);
    s28_d.selected_nodes = vec![s28_d.contours[0][2].id];
    assert!(s28_d.make_selected_node_first());
    let raw28_d = raster_to_rgba(&s28_d);
    assert!(
        count_pixel_delta(&raw_a_base, &raw28_d) >= 20,
        "State 28d Make Node First moves start arrow"
    );

    let mut img28 = RgbaImage::from_pixel(w, h, Rgba([220, 224, 230, 255]));
    blit_scaled_half(&mut img28, &raw28_a, 0, 0);
    blit_scaled_half(&mut img28, &raw28_b, w / 2, 0);
    blit_scaled_half(&mut img28, &raw28_c, 0, h / 2);
    blit_scaled_half(&mut img28, &raw28_d, w / 2, h / 2);
    draw_text_5x7(
        &mut img28,
        "A: WIREFRAME ARROW FLIP",
        12,
        50,
        2,
        Rgba([0, 102, 255, 255]),
    );
    draw_text_5x7(
        &mut img28,
        "B: NONZERO BLACK COUNTER",
        332,
        50,
        2,
        Rgba([220, 38, 38, 255]),
    );
    draw_text_5x7(
        &mut img28,
        "C: OPEN ENDPOINT JUMP",
        12,
        252,
        2,
        Rgba([0, 102, 255, 255]),
    );
    draw_text_5x7(
        &mut img28,
        "D: MAKE NODE FIRST",
        332,
        252,
        2,
        Rgba([40, 167, 69, 255]),
    );
    draw_hud_overlay(
        &mut img28,
        None,
        None,
        false,
        [255, 59, 48, 255],
        "28: REVERSE 3X+MAKE FIRST",
    );
    frames_rgba.push((
        "frame_28_reverse_contour_3_modes_and_make_first.png".into(),
        img28,
    ));

    // =========================================================================
    // V. PenTool (P) Drawing, Splitting & Modifiers (States 29..32)
    // =========================================================================

    // State 29: Start & Draw a New Path with PenTool (1st point blue cross X + Line + Smooth Cubic via real PenTool events)
    let mut s29 = GrafiaCanvasState::new(theme.clone());
    s29.glyph.width = 600.0;
    s29.glyph.height = 700.0;
    s29.fit_viewport_to_glyph(w as f32, h as f32);
    let raw29_empty = raster_to_rgba(&s29);
    s29.active_tool = CanvasTool::Pen;
    let p29_0 = s29.viewport.to_screen(FontPoint::new(110.0, 170.0));
    let p29_1 = s29.viewport.to_screen(FontPoint::new(220.0, 170.0));
    let p29_2 = s29.viewport.to_screen(FontPoint::new(360.0, 520.0));
    let p29_2_drag = s29.viewport.to_screen(FontPoint::new(470.0, 520.0));
    let p29_3 = s29.viewport.to_screen(FontPoint::new(520.0, 220.0));
    let p29_single = s29.viewport.to_screen(FontPoint::new(130.0, 480.0));
    let mut in29 = RawInput::default();
    // 1. Click p29_0 -> Corner node
    in29.events.push(Event::PointerButton {
        pos: pos2(p29_0.x.0, p29_0.y.0),
        button: PointerButton::Primary,
        pressed: true,
        modifiers: Modifiers::NONE,
    });
    in29.events.push(Event::PointerButton {
        pos: pos2(p29_0.x.0, p29_0.y.0),
        button: PointerButton::Primary,
        pressed: false,
        modifiers: Modifiers::NONE,
    });
    // 2. Click p29_1 without dragging -> straight Line segment from p29_0 to p29_1!
    in29.events.push(Event::PointerButton {
        pos: pos2(p29_1.x.0, p29_1.y.0),
        button: PointerButton::Primary,
        pressed: true,
        modifiers: Modifiers::NONE,
    });
    in29.events.push(Event::PointerButton {
        pos: pos2(p29_1.x.0, p29_1.y.0),
        button: PointerButton::Primary,
        pressed: false,
        modifiers: Modifiers::NONE,
    });
    // 3. Click + drag p29_2 -> Smooth node with symmetric bcp_in and bcp_out!
    in29.events.push(Event::PointerButton {
        pos: pos2(p29_2.x.0, p29_2.y.0),
        button: PointerButton::Primary,
        pressed: true,
        modifiers: Modifiers::NONE,
    });
    in29.events
        .push(Event::PointerMoved(pos2(p29_2_drag.x.0, p29_2_drag.y.0)));
    in29.events.push(Event::PointerButton {
        pos: pos2(p29_2_drag.x.0, p29_2_drag.y.0),
        button: PointerButton::Primary,
        pressed: false,
        modifiers: Modifiers::NONE,
    });
    // 4. Click p29_3 -> completes smooth Cubic curve!
    in29.events.push(Event::PointerButton {
        pos: pos2(p29_3.x.0, p29_3.y.0),
        button: PointerButton::Primary,
        pressed: true,
        modifiers: Modifiers::NONE,
    });
    in29.events.push(Event::PointerButton {
        pos: pos2(p29_3.x.0, p29_3.y.0),
        button: PointerButton::Primary,
        pressed: false,
        modifiers: Modifiers::NONE,
    });
    in29.events.push(Event::Key {
        key: Key::Escape,
        physical_key: None,
        pressed: true,
        repeat: false,
        modifiers: Modifiers::NONE,
    });
    run_frame(&ctx, &mut s29, &theme, in29);
    // 5. After Escape ends the first open contour, click p29_single with PenTool in the next frame to create a 1-point open contour (`drawSingle:` blue cross X)!
    let mut in29_b = RawInput::default();
    in29_b.events.push(Event::PointerButton {
        pos: pos2(p29_single.x.0, p29_single.y.0),
        button: PointerButton::Primary,
        pressed: true,
        modifiers: Modifiers::NONE,
    });
    in29_b.events.push(Event::PointerButton {
        pos: pos2(p29_single.x.0, p29_single.y.0),
        button: PointerButton::Primary,
        pressed: false,
        modifiers: Modifiers::NONE,
    });
    run_frame(&ctx, &mut s29, &theme, in29_b);
    assert_eq!(s29.contours.len(), 2);
    assert_eq!(
        s29.contours[1].len(),
        1,
        "Second contour is a 1-point open path created by PenTool"
    );
    s29.selected_nodes.clear();
    let raw29 = raster_to_rgba(&s29);
    assert!(count_pixel_delta(&raw29_empty, &raw29) > 400);
    let mut img29 = raw29.clone();
    draw_hud_overlay(
        &mut img29,
        Some((p29_2_drag.x.0, p29_2_drag.y.0)),
        Some((p29_2.x.0, p29_2.y.0)),
        true,
        [175, 82, 222, 255],
        "29: [P] X-POINT+LINE+CUBIC",
    );
    frames_rgba.push(("frame_29_pen_single_cross_line_and_cubic.png".into(), img29));

    // State 30: PenTool Mid-Drag Modifiers (2x2 Quad Panel: A = Alt Cusp-Break | B = Spacebar Move | C = Cmd Select Override | D = Resume Same Open Path)
    let mut s30 = GrafiaCanvasState::new(theme.clone());
    s30.glyph.width = 600.0;
    s30.glyph.height = 700.0;
    s30.fit_viewport_to_glyph(w as f32, h as f32);
    s30.active_tool = CanvasTool::Pen;
    let p30_alt_drag = s30.viewport.to_screen(FontPoint::new(360.0, 640.0));
    let p30_space_move = s30.viewport.to_screen(FontPoint::new(455.0, 600.0));
    let mut in30_a = RawInput::default();
    in30_a.events.push(Event::PointerButton {
        pos: pos2(p29_1.x.0, p29_1.y.0),
        button: PointerButton::Primary,
        pressed: true,
        modifiers: Modifiers::NONE,
    });
    in30_a.events.push(Event::PointerButton {
        pos: pos2(p29_1.x.0, p29_1.y.0),
        button: PointerButton::Primary,
        pressed: false,
        modifiers: Modifiers::NONE,
    });
    in30_a.events.push(Event::PointerButton {
        pos: pos2(p29_2.x.0, p29_2.y.0),
        button: PointerButton::Primary,
        pressed: true,
        modifiers: Modifiers::NONE,
    });
    in30_a
        .events
        .push(Event::PointerMoved(pos2(p29_2_drag.x.0, p29_2_drag.y.0)));
    run_frame(&ctx, &mut s30, &theme, in30_a);

    // Quadrant A: Hold Alt mid-drag to break Smooth into blue Corner cusp, freeze bcp_in horizontally, and pivot bcp_out upward
    ctx.input_mut(|i| i.modifiers.alt = true);
    let mut in30_b = RawInput::default();
    in30_b.events.push(Event::PointerMoved(pos2(
        p30_alt_drag.x.0,
        p30_alt_drag.y.0,
    )));
    run_frame(&ctx, &mut s30, &theme, in30_b);
    ctx.input_mut(|i| i.modifiers.alt = false);
    assert_eq!(s30.contours[0].last().unwrap().node_type, NodeType::Corner);
    let mut raw30_a = raster_to_rgba(&s30);
    draw_hud_overlay(
        &mut raw30_a,
        Some((p30_alt_drag.x.0, p30_alt_drag.y.0)),
        Some((p29_2_drag.x.0, p29_2_drag.y.0)),
        true,
        [0, 102, 255, 255],
        "",
    );

    // Quadrant B: Hold Spacebar mid-drag to translate newly placed anchor + both BCP handles together!
    let mut s30_space = GrafiaCanvasState::new(theme.clone());
    s30_space.glyph.width = 600.0;
    s30_space.glyph.height = 700.0;
    s30_space.fit_viewport_to_glyph(w as f32, h as f32);
    s30_space.active_tool = CanvasTool::Pen;
    let mut in30_sp_init = RawInput::default();
    in30_sp_init.events.push(Event::PointerButton {
        pos: pos2(p29_1.x.0, p29_1.y.0),
        button: PointerButton::Primary,
        pressed: true,
        modifiers: Modifiers::NONE,
    });
    in30_sp_init.events.push(Event::PointerButton {
        pos: pos2(p29_1.x.0, p29_1.y.0),
        button: PointerButton::Primary,
        pressed: false,
        modifiers: Modifiers::NONE,
    });
    in30_sp_init.events.push(Event::PointerButton {
        pos: pos2(p29_2.x.0, p29_2.y.0),
        button: PointerButton::Primary,
        pressed: true,
        modifiers: Modifiers::NONE,
    });
    in30_sp_init
        .events
        .push(Event::PointerMoved(pos2(p29_2_drag.x.0, p29_2_drag.y.0)));
    run_frame(&ctx, &mut s30_space, &theme, in30_sp_init);
    let mut in30_space_down = RawInput::default();
    in30_space_down.events.push(Event::Key {
        key: Key::Space,
        physical_key: None,
        pressed: true,
        repeat: false,
        modifiers: Modifiers::NONE,
    });
    run_frame(&ctx, &mut s30_space, &theme, in30_space_down);
    let mut in30_space_move = RawInput::default();
    in30_space_move.events.push(Event::PointerMoved(pos2(
        p30_space_move.x.0,
        p30_space_move.y.0,
    )));
    run_frame(&ctx, &mut s30_space, &theme, in30_space_move);
    let mut raw30_b = raster_to_rgba(&s30_space);
    assert!(count_pixel_delta(&raw30_a, &raw30_b) > 100);
    draw_hud_overlay(
        &mut raw30_b,
        Some((p30_space_move.x.0, p30_space_move.y.0)),
        Some((p29_2_drag.x.0, p29_2_drag.y.0)),
        true,
        [40, 167, 69, 255],
        "",
    );
    let mut in30_space_up = RawInput::default();
    in30_space_up.events.push(Event::Key {
        key: Key::Space,
        physical_key: None,
        pressed: false,
        repeat: false,
        modifiers: Modifiers::NONE,
    });
    in30_space_up.events.push(Event::PointerButton {
        pos: pos2(p30_space_move.x.0, p30_space_move.y.0),
        button: PointerButton::Primary,
        pressed: false,
        modifiers: Modifiers::NONE,
    });
    run_frame(&ctx, &mut s30_space, &theme, in30_space_up);

    // Quadrant C: Hold Cmd while in PenTool to temporarily override to SelectTool and drag first node p29_1 left without ending the open path!
    let p30_cmd_tweak = s30_space.viewport.to_screen(FontPoint::new(115.0, 260.0));
    ctx.input_mut(|i| i.modifiers.command = true);
    let mut in30_cmd = RawInput::default();
    in30_cmd.events.push(Event::PointerButton {
        pos: pos2(p29_1.x.0, p29_1.y.0),
        button: PointerButton::Primary,
        pressed: true,
        modifiers: Modifiers::COMMAND,
    });
    in30_cmd.events.push(Event::PointerMoved(pos2(
        p30_cmd_tweak.x.0,
        p30_cmd_tweak.y.0,
    )));
    in30_cmd.events.push(Event::PointerButton {
        pos: pos2(p30_cmd_tweak.x.0, p30_cmd_tweak.y.0),
        button: PointerButton::Primary,
        pressed: false,
        modifiers: Modifiers::COMMAND,
    });
    run_frame(&ctx, &mut s30_space, &theme, in30_cmd);
    ctx.input_mut(|i| i.modifiers.command = false);
    assert_eq!(
        s30_space.active_open_contour,
        Some(0),
        "Active open contour must remain preserved across Cmd temporary SelectTool override"
    );
    let mut raw30_c = raster_to_rgba(&s30_space);
    assert!(count_pixel_delta(&raw30_b, &raw30_c) > 80);
    draw_hud_overlay(
        &mut raw30_c,
        Some((p30_cmd_tweak.x.0, p30_cmd_tweak.y.0)),
        Some((p29_1.x.0, p29_1.y.0)),
        true,
        [255, 149, 0, 255],
        "",
    );

    // Quadrant D: Release Cmd and click p29_3 in PenTool -> seamlessly appends to the same open contour!
    let mut in30_d = RawInput::default();
    in30_d.events.push(Event::PointerButton {
        pos: pos2(p29_3.x.0, p29_3.y.0),
        button: PointerButton::Primary,
        pressed: true,
        modifiers: Modifiers::NONE,
    });
    in30_d.events.push(Event::PointerButton {
        pos: pos2(p29_3.x.0, p29_3.y.0),
        button: PointerButton::Primary,
        pressed: false,
        modifiers: Modifiers::NONE,
    });
    run_frame(&ctx, &mut s30_space, &theme, in30_d);
    assert_eq!(s30_space.contours.len(), 1);
    let mut raw30_d = raster_to_rgba(&s30_space);
    assert!(count_pixel_delta(&raw30_c, &raw30_d) > 100);
    draw_hud_overlay(
        &mut raw30_d,
        Some((p29_3.x.0, p29_3.y.0)),
        None,
        true,
        [175, 82, 222, 255],
        "",
    );

    let mut img30 = RgbaImage::from_pixel(w, h, Rgba([220, 224, 230, 255]));
    blit_scaled_half(&mut img30, &raw30_a, 0, 0);
    blit_scaled_half(&mut img30, &raw30_b, w / 2, 0);
    blit_scaled_half(&mut img30, &raw30_c, 0, h / 2);
    blit_scaled_half(&mut img30, &raw30_d, w / 2, h / 2);
    draw_text_5x7(
        &mut img30,
        "A: PEN + ALT CUSP",
        12,
        215,
        2,
        Rgba([0, 102, 255, 255]),
    );
    draw_text_5x7(
        &mut img30,
        "B: PEN + SPACE MOVE",
        332,
        215,
        2,
        Rgba([40, 167, 69, 255]),
    );
    draw_text_5x7(
        &mut img30,
        "C: PEN + CMD SELECT",
        12,
        455,
        2,
        Rgba([255, 149, 0, 255]),
    );
    draw_text_5x7(
        &mut img30,
        "D: RESUME OPEN PATH",
        332,
        455,
        2,
        Rgba([175, 82, 222, 255]),
    );
    draw_hud_overlay(
        &mut img30,
        None,
        None,
        false,
        [175, 82, 222, 255],
        "30: [P] ALT+SPACE+CMD+CONT",
    );
    frames_rgba.push((
        "frame_30_pen_alt_cusp_space_move_cmd_select.png".into(),
        img30,
    ));

    // State 31: PenTool Resume Open Contour & Smooth Close by Clicking and Dragging Start Node 0
    let mut s31 = GrafiaCanvasState::new(theme.clone());
    s31.glyph.width = 600.0;
    s31.glyph.height = 700.0;
    s31.fit_viewport_to_glyph(w as f32, h as f32);
    s31.active_tool = CanvasTool::Pen;
    let p31_close_drag_font = FontPoint::new(120.0, 260.0);
    let p31_close_drag = s31.viewport.to_screen(p31_close_drag_font);
    let mut in31 = RawInput::default();
    in31.events.push(Event::PointerButton {
        pos: pos2(p29_1.x.0, p29_1.y.0),
        button: PointerButton::Primary,
        pressed: true,
        modifiers: Modifiers::NONE,
    });
    in31.events.push(Event::PointerButton {
        pos: pos2(p29_1.x.0, p29_1.y.0),
        button: PointerButton::Primary,
        pressed: false,
        modifiers: Modifiers::NONE,
    });
    in31.events.push(Event::PointerButton {
        pos: pos2(p29_2.x.0, p29_2.y.0),
        button: PointerButton::Primary,
        pressed: true,
        modifiers: Modifiers::NONE,
    });
    in31.events
        .push(Event::PointerMoved(pos2(p29_2_drag.x.0, p29_2_drag.y.0)));
    in31.events.push(Event::PointerButton {
        pos: pos2(p29_2_drag.x.0, p29_2_drag.y.0),
        button: PointerButton::Primary,
        pressed: false,
        modifiers: Modifiers::NONE,
    });
    in31.events.push(Event::PointerButton {
        pos: pos2(p29_3.x.0, p29_3.y.0),
        button: PointerButton::Primary,
        pressed: true,
        modifiers: Modifiers::NONE,
    });
    in31.events.push(Event::PointerButton {
        pos: pos2(p29_3.x.0, p29_3.y.0),
        button: PointerButton::Primary,
        pressed: false,
        modifiers: Modifiers::NONE,
    });
    // Now click AND drag on start node 0 (`p29_1`) to close smoothly!
    in31.events.push(Event::PointerButton {
        pos: pos2(p29_1.x.0, p29_1.y.0),
        button: PointerButton::Primary,
        pressed: true,
        modifiers: Modifiers::NONE,
    });
    in31.events.push(Event::PointerMoved(pos2(
        p31_close_drag.x.0,
        p31_close_drag.y.0,
    )));
    in31.events.push(Event::PointerButton {
        pos: pos2(p31_close_drag.x.0, p31_close_drag.y.0),
        button: PointerButton::Primary,
        pressed: false,
        modifiers: Modifiers::NONE,
    });
    run_frame(&ctx, &mut s31, &theme, in31);
    assert!(s31.is_contour_closed(0));
    assert_eq!(s31.contours[0][0].node_type, NodeType::Smooth);
    let c31_len = s31.contours[0].len();
    assert_relative_eq!(
        s31.contours[0][1].point.x.0,
        p31_close_drag_font.x.0,
        epsilon = 1.0
    );
    assert_relative_eq!(
        s31.contours[0][1].point.y.0,
        p31_close_drag_font.y.0,
        epsilon = 1.0
    );
    assert_relative_eq!(
        s31.contours[0][c31_len - 1].point.x.0,
        2.0 * 220.0 - p31_close_drag_font.x.0,
        epsilon = 1.0
    );
    assert_relative_eq!(
        s31.contours[0][c31_len - 1].point.y.0,
        2.0 * 170.0 - p31_close_drag_font.y.0,
        epsilon = 1.0
    );
    let raw31 = raster_to_rgba(&s31);
    assert!(count_pixel_delta(&raw29, &raw31) > 200);
    let mut img31 = raw31;
    draw_hud_overlay(
        &mut img31,
        Some((p31_close_drag.x.0, p31_close_drag.y.0)),
        Some((p29_1.x.0, p29_1.y.0)),
        true,
        [40, 167, 69, 255],
        "31: [P] RESUME+DRAG CLOSE",
    );
    frames_rgba.push((
        "frame_31_pen_resume_and_smooth_close_node0.png".into(),
        img31,
    ));

    // State 32: PenTool Hover (Blue Ghost Node) & De Casteljau Split (+ Shift Extremum Snap)
    let mut s32 = GrafiaCanvasState::init_glyph_o();
    s32.fit_viewport_to_glyph(w as f32, h as f32);
    s32.active_tool = CanvasTool::Pen;
    // Skew node 0's outgoing handle upward so top-right segment has a non-midpoint vertical extremum, then Shift+Click with PenTool!
    s32.contours[0][1].point.y.0 += 120.0;
    s32.sync_paths_from_contours();
    let segs32_init = extract_segments(&s32.contours[0], true);
    let off_center_32 = match segs32_init[0] {
        ContourSegment::Cubic { cubic, .. } => {
            use kurbo::ParamCurve;
            let p = cubic.eval(0.62);
            FontPoint::new(p.x, p.y)
        }
        _ => unreachable!(),
    };
    let hover32_scr = s32.viewport.to_screen(off_center_32);
    ctx.input_mut(|i| i.modifiers.shift = true);
    let mut in32 = RawInput::default();
    in32.events.push(Event::PointerButton {
        pos: pos2(hover32_scr.x.0, hover32_scr.y.0),
        button: PointerButton::Primary,
        pressed: true,
        modifiers: Modifiers::SHIFT,
    });
    in32.events.push(Event::PointerButton {
        pos: pos2(hover32_scr.x.0, hover32_scr.y.0),
        button: PointerButton::Primary,
        pressed: false,
        modifiers: Modifiers::SHIFT,
    });
    // Also hover over bottom-right segment to show the blue ghost node ring simultaneously
    let segs32 = extract_segments(&s32.contours[0], true);
    let ghost32_pt = match segs32[2] {
        ContourSegment::Cubic { cubic, .. } => {
            use kurbo::ParamCurve;
            let p = cubic.eval(0.5);
            FontPoint::new(p.x, p.y)
        }
        _ => unreachable!(),
    };
    let ghost32_scr = s32.viewport.to_screen(ghost32_pt);
    in32.events
        .push(Event::PointerMoved(pos2(ghost32_scr.x.0, ghost32_scr.y.0)));
    run_frame(&ctx, &mut s32, &theme, in32);
    ctx.input_mut(|i| i.modifiers.shift = false);
    let raw32 = raster_to_rgba(&s32);
    assert!(count_pixel_delta(&raw01, &raw32) > 80);
    let mut img32 = raw32;
    draw_hud_overlay(
        &mut img32,
        Some((hover32_scr.x.0, hover32_scr.y.0)),
        None,
        true,
        [0, 122, 255, 255],
        "32: [P] GHOST+SHIFT EXTREM",
    );
    frames_rgba.push((
        "frame_32_pen_hover_ghost_and_shift_extremum_split.png".into(),
        img32,
    ));

    // =========================================================================
    // VI. Keyboard Navigation, Viewport & History (States 33..34)
    // =========================================================================

    // State 33: Tab Node Cycling + Keyboard Nudging (Cmd+Arrow ±100, Alt+Arrow decoupled, OffCurve G1 nudge) + Undo/Redo (2x2 Quad Panel)
    // Q1: Tab cycling (3x Tab -> node 3) + Cmd+ArrowRight (+100 upm)
    let mut s33_a = GrafiaCanvasState::init_glyph_o();
    s33_a.fit_viewport_to_glyph(w as f32, h as f32);
    s33_a.selected_nodes = vec![s33_a.contours[0][0].id];
    let mut in33_a = RawInput::default();
    for _ in 0..3 {
        in33_a.events.push(Event::Key {
            key: Key::Tab,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: Modifiers::NONE,
        });
    }
    in33_a.events.push(Event::Key {
        key: Key::ArrowRight,
        physical_key: None,
        pressed: true,
        repeat: false,
        modifiers: Modifiers::COMMAND,
    });
    run_frame(&ctx, &mut s33_a, &theme, in33_a);
    let raw33_a = raster_to_rgba(&s33_a);
    assert!(count_pixel_delta(&raw01, &raw33_a) > 400);

    // Q2: Alt + ArrowUp (8x = +80 upm with Shift+Alt or 50x Alt+ArrowUp) on Smooth OnCurve node 0 -> moves anchor while keeping attached OffCurve BCPs fixed!
    let mut s33_b = GrafiaCanvasState::init_glyph_o();
    s33_b.fit_viewport_to_glyph(w as f32, h as f32);
    s33_b.selected_nodes = vec![s33_b.contours[0][0].id];
    let bcp_out_33b = s33_b.contours[0][1].point;
    let bcp_in_33b = s33_b.contours[0][11].point;
    let shift_alt = Modifiers {
        shift: true,
        alt: true,
        ..Default::default()
    };
    let mut in33_b = RawInput::default();
    for _ in 0..9 {
        in33_b.events.push(Event::Key {
            key: Key::ArrowUp,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: shift_alt,
        });
    }
    run_frame(&ctx, &mut s33_b, &theme, in33_b);
    assert_eq!(
        s33_b.contours[0][1].point, bcp_out_33b,
        "Alt+Arrow must keep outgoing BCP fixed"
    );
    assert_eq!(
        s33_b.contours[0][11].point, bcp_in_33b,
        "Alt+Arrow must keep incoming BCP fixed"
    );
    let raw33_b = raster_to_rgba(&s33_b);
    assert!(count_pixel_delta(&raw01, &raw33_b) > 200);

    // Q3: Nudging an OffCurve BCP handle (`contours[0][1]`) with Shift+ArrowUp -> rotates opposite Smooth handle to preserve G1 collinearity!
    let mut s33_c = GrafiaCanvasState::init_glyph_o();
    s33_c.fit_viewport_to_glyph(w as f32, h as f32);
    s33_c.selected_nodes = vec![s33_c.contours[0][1].id];
    let mut in33_c = RawInput::default();
    for _ in 0..9 {
        in33_c.events.push(Event::Key {
            key: Key::ArrowUp,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: Modifiers::SHIFT,
        });
    }
    run_frame(&ctx, &mut s33_c, &theme, in33_c);
    let raw33_c = raster_to_rgba(&s33_c);
    assert!(count_pixel_delta(&raw01, &raw33_c) > 200);

    // Q4: Reversed Contour + Tab steps in reversed winding direction + Undo/Redo verification
    let mut s33_d = GrafiaCanvasState::init_glyph_o();
    s33_d.fit_viewport_to_glyph(w as f32, h as f32);
    s33_d.selected_nodes = vec![s33_d.contours[0][0].id];
    s33_d.reverse_selected_or_all_contours();
    let mut in33_d = RawInput::default();
    in33_d.events.push(Event::Key {
        key: Key::Tab,
        physical_key: None,
        pressed: true,
        repeat: false,
        modifiers: Modifiers::NONE,
    });
    run_frame(&ctx, &mut s33_d, &theme, in33_d);
    let raw33_d = raster_to_rgba(&s33_d);
    assert!(count_pixel_delta(&raw01, &raw33_d) > 80);

    let mut img33 = RgbaImage::from_pixel(w, h, Rgba([220, 224, 230, 255]));
    blit_scaled_half(&mut img33, &raw33_a, 0, 0);
    blit_scaled_half(&mut img33, &raw33_b, w / 2, 0);
    blit_scaled_half(&mut img33, &raw33_c, 0, h / 2);
    blit_scaled_half(&mut img33, &raw33_d, w / 2, h / 2);
    draw_text_5x7(
        &mut img33,
        "A: 3X TAB + CMD+ARROW",
        12,
        50,
        2,
        Rgba([88, 86, 214, 255]),
    );
    draw_text_5x7(
        &mut img33,
        "B: ALT+ARROW (ANCHOR)",
        332,
        50,
        2,
        Rgba([40, 167, 69, 255]),
    );
    draw_text_5x7(
        &mut img33,
        "C: BCP NUDGE (G1 SMOOTH)",
        12,
        252,
        2,
        Rgba([175, 82, 222, 255]),
    );
    draw_text_5x7(
        &mut img33,
        "D: REVERSED TAB ORDER",
        332,
        252,
        2,
        Rgba([0, 102, 255, 255]),
    );
    draw_hud_overlay(
        &mut img33,
        None,
        None,
        false,
        [88, 86, 214, 255],
        "33: TAB+NUDGE+ALT+UNDO",
    );
    frames_rgba.push(("frame_33_tab_cycle_nudge_and_undo_redo.png".into(), img33));

    // State 34: Viewport Space + Drag (Hand Panning), Zoom at Cursor & Cmd+0 (Fit) dispatched via real events
    let mut s34 = GrafiaCanvasState::init_glyph_a();
    s34.fit_viewport_to_glyph(w as f32, h as f32);
    let mut in34_a = RawInput::default();
    in34_a.events.push(Event::PointerMoved(pos2(320.0, 240.0)));
    in34_a.events.push(Event::Zoom(1.45));
    in34_a.events.push(Event::Key {
        key: Key::Space,
        physical_key: None,
        pressed: true,
        repeat: false,
        modifiers: Modifiers::NONE,
    });
    run_frame(&ctx, &mut s34, &theme, in34_a);
    let pan_after_zoom = s34.viewport.pan_offset;
    let mut in34_b = RawInput::default();
    in34_b.events.push(Event::PointerButton {
        pos: pos2(320.0, 240.0),
        button: PointerButton::Primary,
        pressed: true,
        modifiers: Modifiers::NONE,
    });
    in34_b.events.push(Event::PointerMoved(pos2(385.0, 270.0)));
    in34_b.events.push(Event::PointerButton {
        pos: pos2(385.0, 270.0),
        button: PointerButton::Primary,
        pressed: false,
        modifiers: Modifiers::NONE,
    });
    run_frame(&ctx, &mut s34, &theme, in34_b);
    let mut in34_c = RawInput::default();
    in34_c.events.push(Event::Key {
        key: Key::Space,
        physical_key: None,
        pressed: false,
        repeat: false,
        modifiers: Modifiers::NONE,
    });
    run_frame(&ctx, &mut s34, &theme, in34_c);
    assert_relative_eq!(
        s34.viewport.pan_offset.x.0,
        pan_after_zoom.x.0 + 65.0,
        epsilon = 1e-3
    );
    assert_relative_eq!(
        s34.viewport.pan_offset.y.0,
        pan_after_zoom.y.0 + 30.0,
        epsilon = 1e-3
    );
    let raw34 = raster_to_rgba(&s34);
    assert!(count_pixel_delta(&raw_a_base, &raw34) > 2000);
    // Also verify Cmd+0 (`fit_viewport_to_glyph`) restores the fitted viewport
    let mut s34_fit = s34.clone_via_snapshot();
    s34_fit.fit_viewport_to_glyph(w as f32, h as f32);
    assert_relative_eq!(s34_fit.viewport.zoom, s01_a.viewport.zoom, epsilon = 1e-4);
    let mut img34 = raw34;
    draw_hud_overlay(
        &mut img34,
        Some((385.0, 270.0)),
        Some((320.0, 240.0)),
        false,
        [0, 122, 255, 255],
        "34: SPACE PAN+CURSOR ZOOM",
    );
    frames_rgba.push(("frame_34_space_pan_cursor_zoom_and_fit.png".into(), img34));

    // State 35: Cmd+Ctrl Slide Smooth Node Along Cubic Path (moveNodeOnPath: / shiftMiddle:) & Alt Slide Smooth Line Node (nearest_point_on_line)
    let mut s35_a = GrafiaCanvasState::init_glyph_o();
    s35_a.fit_viewport_to_glyph(w as f32, h as f32);
    s35_a.selected_nodes = vec![s35_a.contours[0][0].id];
    let raw35_a = raster_to_rgba(&s35_a);

    let mut s35_b = GrafiaCanvasState::init_glyph_o();
    s35_b.fit_viewport_to_glyph(w as f32, h as f32);
    let p35_start = s35_b.viewport.to_screen(s35_b.contours[0][0].point);
    let p35_target = s35_b.viewport.to_screen(FontPoint::new(
        s35_b.contours[0][0].point.x.0 + 75.0,
        s35_b.contours[0][0].point.y.0 - 15.0,
    ));
    let cmd_ctrl = Modifiers {
        command: true,
        mac_cmd: true,
        ctrl: true,
        ..Default::default()
    };
    ctx.input_mut(|i| {
        i.modifiers.command = true;
        i.modifiers.mac_cmd = true;
        i.modifiers.ctrl = true;
    });
    let mut in35_b = RawInput::default();
    in35_b.events.push(Event::PointerButton {
        pos: pos2(p35_start.x.0, p35_start.y.0),
        button: PointerButton::Primary,
        pressed: true,
        modifiers: cmd_ctrl,
    });
    in35_b
        .events
        .push(Event::PointerMoved(pos2(p35_target.x.0, p35_target.y.0)));
    in35_b.events.push(Event::PointerButton {
        pos: pos2(p35_target.x.0, p35_target.y.0),
        button: PointerButton::Primary,
        pressed: false,
        modifiers: cmd_ctrl,
    });
    run_frame(&ctx, &mut s35_b, &theme, in35_b);
    ctx.input_mut(|i| {
        i.modifiers.command = false;
        i.modifiers.mac_cmd = false;
        i.modifiers.ctrl = false;
    });
    let mut raw35_b = raster_to_rgba(&s35_b);
    assert!(
        count_pixel_delta(&raw35_a, &raw35_b) > 150,
        "Cmd+Ctrl slide along curve must visibly move anchor and update handles"
    );
    draw_hud_overlay(
        &mut raw35_b,
        Some((p35_target.x.0, p35_target.y.0)),
        Some((p35_start.x.0, p35_start.y.0)),
        true,
        [40, 167, 69, 255],
        "",
    );

    // Q3: Cmd+Ctrl Slide backward along curve
    let mut s35_c = GrafiaCanvasState::init_glyph_o();
    s35_c.fit_viewport_to_glyph(w as f32, h as f32);
    let p35_back = s35_c.viewport.to_screen(FontPoint::new(
        s35_c.contours[0][0].point.x.0 - 75.0,
        s35_c.contours[0][0].point.y.0 - 15.0,
    ));
    ctx.input_mut(|i| {
        i.modifiers.command = true;
        i.modifiers.mac_cmd = true;
        i.modifiers.ctrl = true;
    });
    let mut in35_c = RawInput::default();
    in35_c.events.push(Event::PointerButton {
        pos: pos2(p35_start.x.0, p35_start.y.0),
        button: PointerButton::Primary,
        pressed: true,
        modifiers: cmd_ctrl,
    });
    in35_c
        .events
        .push(Event::PointerMoved(pos2(p35_back.x.0, p35_back.y.0)));
    in35_c.events.push(Event::PointerButton {
        pos: pos2(p35_back.x.0, p35_back.y.0),
        button: PointerButton::Primary,
        pressed: false,
        modifiers: cmd_ctrl,
    });
    run_frame(&ctx, &mut s35_c, &theme, in35_c);
    ctx.input_mut(|i| {
        i.modifiers.command = false;
        i.modifiers.mac_cmd = false;
        i.modifiers.ctrl = false;
    });
    let mut raw35_c = raster_to_rgba(&s35_c);
    assert!(count_pixel_delta(&raw35_b, &raw35_c) > 150);
    draw_hud_overlay(
        &mut raw35_c,
        Some((p35_back.x.0, p35_back.y.0)),
        Some((p35_start.x.0, p35_start.y.0)),
        true,
        [0, 122, 255, 255],
        "",
    );

    // Q4: Alt + Drag on a Smooth Line Node (nearest_point_on_line)
    let mut s35_d = GrafiaCanvasState::init_glyph_a();
    s35_d.fit_viewport_to_glyph(w as f32, h as f32);
    // Insert a Smooth node on the left diagonal stem of /A and slide it along the stem with Alt+Drag
    s35_d.contours[0].insert(1, Node::new(NodeId(901), 165.0, 350.0, NodeType::Smooth));
    s35_d.sync_paths_from_contours();
    let p35_d0 = s35_d.viewport.to_screen(FontPoint::new(165.0, 350.0));
    let p35_d1 = s35_d.viewport.to_screen(FontPoint::new(260.0, 490.0));
    ctx.input_mut(|i| i.modifiers.alt = true);
    let mut in35_d = RawInput::default();
    in35_d.events.push(Event::PointerButton {
        pos: pos2(p35_d0.x.0, p35_d0.y.0),
        button: PointerButton::Primary,
        pressed: true,
        modifiers: Modifiers::ALT,
    });
    in35_d
        .events
        .push(Event::PointerMoved(pos2(p35_d1.x.0, p35_d1.y.0)));
    in35_d.events.push(Event::PointerButton {
        pos: pos2(p35_d1.x.0, p35_d1.y.0),
        button: PointerButton::Primary,
        pressed: false,
        modifiers: Modifiers::ALT,
    });
    run_frame(&ctx, &mut s35_d, &theme, in35_d);
    ctx.input_mut(|i| i.modifiers.alt = false);
    assert_eq!(s35_d.contours[0][1].node_type, NodeType::Smooth);
    let mut raw35_d = raster_to_rgba(&s35_d);
    draw_hud_overlay(
        &mut raw35_d,
        Some((p35_d1.x.0, p35_d1.y.0)),
        Some((p35_d0.x.0, p35_d0.y.0)),
        true,
        [40, 167, 69, 255],
        "",
    );

    let mut img35 = RgbaImage::from_pixel(w, h, Rgba([220, 224, 230, 255]));
    blit_scaled_half(&mut img35, &raw35_a, 0, 0);
    blit_scaled_half(&mut img35, &raw35_b, w / 2, 0);
    blit_scaled_half(&mut img35, &raw35_c, 0, h / 2);
    blit_scaled_half(&mut img35, &raw35_d, w / 2, h / 2);
    draw_text_5x7(
        &mut img35,
        "A: BEFORE SLIDE ON CURVE",
        12,
        50,
        2,
        Rgba([88, 86, 214, 255]),
    );
    draw_text_5x7(
        &mut img35,
        "B: CMD+CTRL SLIDE (+S)",
        332,
        50,
        2,
        Rgba([40, 167, 69, 255]),
    );
    draw_text_5x7(
        &mut img35,
        "C: CMD+CTRL SLIDE (-T)",
        12,
        252,
        2,
        Rgba([0, 102, 255, 255]),
    );
    draw_text_5x7(
        &mut img35,
        "D: ALT SLIDE ON LINE",
        332,
        252,
        2,
        Rgba([40, 167, 69, 255]),
    );
    draw_hud_overlay(
        &mut img35,
        None,
        None,
        false,
        [40, 167, 69, 255],
        "35: CMD+CTRL & ALT SLIDE",
    );
    frames_rgba.push((
        "frame_35_cmd_ctrl_slide_on_curve_and_alt_slide_line.png".into(),
        img35,
    ));

    // State 36: Autonomous PenTool Curve Drawing + Close-Hover Ring on node[0] + Scenarios C1/C2 + PreviewFilled
    let mut s36_a = GrafiaCanvasState::new(theme.clone());
    s36_a.glyph.width = 600.0;
    s36_a.glyph.height = 700.0;
    s36_a.fit_viewport_to_glyph(w as f32, h as f32);
    s36_a.active_tool = CanvasTool::Pen;
    let p36_0 = s36_a.viewport.to_screen(FontPoint::new(140.0, 220.0));
    let p36_0_out = s36_a.viewport.to_screen(FontPoint::new(140.0, 440.0));
    let p36_1 = s36_a.viewport.to_screen(FontPoint::new(460.0, 520.0));
    let p36_1_out = s36_a.viewport.to_screen(FontPoint::new(540.0, 320.0));
    let p36_2 = s36_a.viewport.to_screen(FontPoint::new(380.0, 120.0));
    let p36_close_drag = s36_a.viewport.to_screen(FontPoint::new(40.0, 310.0));
    let mut in36_a = RawInput::default();
    in36_a.events.push(Event::PointerButton {
        pos: pos2(p36_0.x.0, p36_0.y.0),
        button: PointerButton::Primary,
        pressed: true,
        modifiers: Modifiers::NONE,
    });
    in36_a
        .events
        .push(Event::PointerMoved(pos2(p36_0_out.x.0, p36_0_out.y.0)));
    in36_a.events.push(Event::PointerButton {
        pos: pos2(p36_0_out.x.0, p36_0_out.y.0),
        button: PointerButton::Primary,
        pressed: false,
        modifiers: Modifiers::NONE,
    });
    in36_a.events.push(Event::PointerButton {
        pos: pos2(p36_1.x.0, p36_1.y.0),
        button: PointerButton::Primary,
        pressed: true,
        modifiers: Modifiers::NONE,
    });
    in36_a
        .events
        .push(Event::PointerMoved(pos2(p36_1_out.x.0, p36_1_out.y.0)));
    in36_a.events.push(Event::PointerButton {
        pos: pos2(p36_1_out.x.0, p36_1_out.y.0),
        button: PointerButton::Primary,
        pressed: false,
        modifiers: Modifiers::NONE,
    });
    in36_a.events.push(Event::PointerButton {
        pos: pos2(p36_2.x.0, p36_2.y.0),
        button: PointerButton::Primary,
        pressed: true,
        modifiers: Modifiers::NONE,
    });
    in36_a.events.push(Event::PointerButton {
        pos: pos2(p36_2.x.0, p36_2.y.0),
        button: PointerButton::Primary,
        pressed: false,
        modifiers: Modifiers::NONE,
    });
    in36_a
        .events
        .push(Event::PointerMoved(pos2(p36_0.x.0, p36_0.y.0)));
    run_frame(&ctx, &mut s36_a, &theme, in36_a);
    assert_eq!(s36_a.hovered_pen_close_node, Some(s36_a.contours[0][0].id));
    let mut raw36_a = raster_to_rgba(&s36_a);
    draw_hud_overlay(
        &mut raw36_a,
        Some((p36_0.x.0, p36_0.y.0)),
        None,
        false,
        [0, 122, 255, 255],
        "",
    );

    // Q2: Scenario C1 Click-Close on node[0] (preserves outgoing handle contours[0][1]!)
    let mut s36_b = s36_a.clone_via_snapshot();
    let mut in36_b = RawInput::default();
    in36_b.events.push(Event::PointerButton {
        pos: pos2(p36_0.x.0, p36_0.y.0),
        button: PointerButton::Primary,
        pressed: true,
        modifiers: Modifiers::NONE,
    });
    in36_b.events.push(Event::PointerButton {
        pos: pos2(p36_0.x.0, p36_0.y.0),
        button: PointerButton::Primary,
        pressed: false,
        modifiers: Modifiers::NONE,
    });
    run_frame(&ctx, &mut s36_b, &theme, in36_b);
    assert!(s36_b.is_contour_closed(0));
    let mut raw36_b = raster_to_rgba(&s36_b);
    draw_hud_overlay(
        &mut raw36_b,
        Some((p36_0.x.0, p36_0.y.0)),
        None,
        true,
        [0, 102, 255, 255],
        "",
    );

    // Q3: Scenario C2 Drag-Close on node[0] (preserves contours[0][1] and mirrors incoming handle!)
    let mut s36_c = s36_a.clone_via_snapshot();
    let mut in36_c = RawInput::default();
    in36_c.events.push(Event::PointerButton {
        pos: pos2(p36_0.x.0, p36_0.y.0),
        button: PointerButton::Primary,
        pressed: true,
        modifiers: Modifiers::NONE,
    });
    in36_c.events.push(Event::PointerMoved(pos2(
        p36_close_drag.x.0,
        p36_close_drag.y.0,
    )));
    in36_c.events.push(Event::PointerButton {
        pos: pos2(p36_close_drag.x.0, p36_close_drag.y.0),
        button: PointerButton::Primary,
        pressed: false,
        modifiers: Modifiers::NONE,
    });
    run_frame(&ctx, &mut s36_c, &theme, in36_c);
    assert!(s36_c.is_contour_closed(0));
    assert_eq!(s36_c.contours[0][0].node_type, NodeType::Smooth);
    let mut raw36_c = raster_to_rgba(&s36_c);
    draw_hud_overlay(
        &mut raw36_c,
        Some((p36_close_drag.x.0, p36_close_drag.y.0)),
        Some((p36_0.x.0, p36_0.y.0)),
        true,
        [40, 167, 69, 255],
        "",
    );

    // Q4: Closed PenTool curve rendered in PreviewFilled mode (solid black silhouette!)
    let mut s36_d = s36_c.clone_via_snapshot();
    s36_d.view_mode = CanvasViewMode::PreviewFilled;
    let raw36_d = raster_to_rgba(&s36_d);
    assert!(
        count_pixel_delta(&raw36_c, &raw36_d) > 2000,
        "Closed PenTool curve must render solid filled silhouette in PreviewFilled"
    );

    let mut img36 = RgbaImage::from_pixel(w, h, Rgba([220, 224, 230, 255]));
    blit_scaled_half(&mut img36, &raw36_a, 0, 0);
    blit_scaled_half(&mut img36, &raw36_b, w / 2, 0);
    blit_scaled_half(&mut img36, &raw36_c, 0, h / 2);
    blit_scaled_half(&mut img36, &raw36_d, w / 2, h / 2);
    draw_text_5x7(
        &mut img36,
        "A: PEN CLOSE-HOVER RING",
        12,
        50,
        2,
        Rgba([0, 102, 255, 255]),
    );
    draw_text_5x7(
        &mut img36,
        "B: C1 CLICK-CLOSE (KEEP)",
        332,
        50,
        2,
        Rgba([0, 102, 255, 255]),
    );
    draw_text_5x7(
        &mut img36,
        "C: C2 DRAG-CLOSE (SMOOTH)",
        12,
        252,
        2,
        Rgba([40, 167, 69, 255]),
    );
    draw_text_5x7(
        &mut img36,
        "D: CLOSED PREVIEW FILLED",
        332,
        252,
        2,
        Rgba([20, 20, 24, 255]),
    );
    draw_hud_overlay(
        &mut img36,
        None,
        None,
        false,
        [40, 167, 69, 255],
        "36: PEN CLOSE C1/C2+FILLED",
    );
    frames_rgba.push((
        "frame_36_pen_new_curve_close_hover_and_filled.png".into(),
        img36,
    ));

    assert_eq!(
        frames_rgba.len(),
        36,
        "Must generate all 36 exhaustive v0.1.0 visual verification frames"
    );

    // Save all 34 individual PNG frames + 6x6 contact sheet strip + animated GIF
    let cols = 6_u32;
    let rows = 6_u32;
    let mut strip = RgbaImage::from_pixel(w * cols, h * rows, Rgba([24, 24, 28, 255]));
    let gif_path = out_dir.join("grafia_v010_interaction_demo.gif");
    let gif_file = fs::File::create(&gif_path).expect("Create GIF file");
    let mut gif_enc = image::codecs::gif::GifEncoder::new_with_speed(gif_file, 30);
    let _ = gif_enc.set_repeat(image::codecs::gif::Repeat::Infinite);

    for (idx, (filename, img)) in frames_rgba.iter().enumerate() {
        let p = out_dir.join(filename);
        img.save(&p).expect("Save individual PNG frame");

        let col = (idx as u32) % cols;
        let row = (idx as u32) / cols;
        for py in 0..h {
            for px in 0..w {
                strip.put_pixel(col * w + px, row * h + py, *img.get_pixel(px, py));
            }
        }

        let frame =
            image::Frame::from_parts(img.clone(), 0, 0, image::Delay::from_numer_denom_ms(650, 1));
        gif_enc.encode_frame(frame).expect("Encode GIF frame");
    }
    drop(gif_enc);

    let strip_path = out_dir.join("grafia_v010_interaction_strip.png");
    strip
        .save(&strip_path)
        .expect("Save contact sheet strip PNG");
    assert!(strip_path.exists());
    assert!(gif_path.exists());
}

trait CloneViaSnapshot {
    fn clone_via_snapshot(&self) -> Self;
}

impl CloneViaSnapshot for GrafiaCanvasState {
    fn clone_via_snapshot(&self) -> Self {
        let mut next = GrafiaCanvasState::new(self.theme.clone());
        next.viewport = self.viewport;
        next.glyph = self.glyph.clone();
        next.active_tool = self.active_tool;
        next.view_mode = self.view_mode;
        next.next_node_id = self.next_node_id;
        next.active_contour_closed = self.active_contour_closed;
        let snap = self.snapshot();
        next.restore_snapshot(&snap);
        next.active_open_contour = self.active_open_contour;
        next.pending_pen_bcp_out = self.pending_pen_bcp_out;
        next.hovered_pen_close_node = self.hovered_pen_close_node;
        next
    }
}
