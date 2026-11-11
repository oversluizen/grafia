//! Test suite for ShapeLocator (Micro-Viewport Vector Minimap) (TC-LOCATOR-01).
//!
//! Asserts:
//! 1. `ShapeLocator` renders a 48x36 dp contextual micro-viewport by default (and supports arbitrary sizes).
//! 2. It renders the entire glyph in subtle context grey and highlights the target contour.
//! 3. Bounding box coordinates remain strictly within the requested size frame.

use egui::{vec2, Pos2, Rect};
use grafia::adapters::GlifoCanvasState;
use grafia::locator::{ShapeLocatorRenderer, ShapeLocatorStyle};

#[test]
fn test_shape_locator_renders_two_layer_micro_viewport() {
    let state = GlifoCanvasState::init_glyph_a();
    let contour_ids = state.glyph.contour_ids();
    assert_eq!(contour_ids.len(), 2, "Glyph /A must have 2 contours");

    let style = ShapeLocatorStyle::default();
    let size = style.size;
    assert_eq!(size, vec2(48.0, 36.0));
    let renderer = ShapeLocatorRenderer::new(&state.glyph, style);

    // Test locator for inner contour (contour 1)
    let inner_contour_id = contour_ids[1];
    let (context_mesh, focus_mesh) = renderer.render_locator(inner_contour_id, size);

    // Context mesh must contain all glyph contours
    assert!(
        !context_mesh.vertices.is_empty(),
        "Context layer must render the full glyph"
    );
    assert_eq!(
        context_mesh.indices.len() % 3, 0,
        "Context mesh indices must form valid triangles"
    );

    // Focus mesh must render the specific highlighted contour
    assert!(
        !focus_mesh.vertices.is_empty(),
        "Focus layer must render the target contour"
    );

    // Assert that all vertices fit within the 48x36 dp bounds (with 1dp tolerance for stroke anti-aliasing)
    let bounds = Rect::from_min_size(Pos2::ZERO, size);
    for v in &context_mesh.vertices {
        assert!(
            v.pos.x >= -1.0 && v.pos.x <= size.x + 1.0 && v.pos.y >= -1.0 && v.pos.y <= size.y + 1.0,
            "Context vertex {:?} must be within locator bounds {:?}",
            v.pos,
            bounds
        );
    }
    for v in &focus_mesh.vertices {
        assert!(
            v.pos.x >= -1.0 && v.pos.x <= size.x + 1.0 && v.pos.y >= -1.0 && v.pos.y <= size.y + 1.0,
            "Focus vertex {:?} must be within locator bounds {:?}",
            v.pos,
            bounds
        );
    }
}

#[test]
fn test_shape_locator_mask_role_renders_mask_color() {
    let mut state = GlifoCanvasState::init_glyph_a();
    let size = vec2(24.0, 16.0);
    let style = ShapeLocatorStyle::default();

    // Mark contour 1 as Mask role
    state.glyph.contours[1].role = grafia::core::ShapeBooleanRole::Mask;
    let renderer = ShapeLocatorRenderer::new(&state.glyph, style.clone());
    let (_, focus_mesh) = renderer.render_locator(state.glyph.contours[1].id, size);

    assert!(!focus_mesh.vertices.is_empty());
    for v in &focus_mesh.vertices {
        assert_eq!(
            v.color, style.mask_color,
            "Focus mesh for Mask contour must use mask_color"
        );
    }
}

#[test]
fn test_shape_locator_respects_visibility_and_degenerate_contours() {
    let mut state = GlifoCanvasState::init_glyph_a();
    let size = vec2(24.0, 16.0);
    let style = ShapeLocatorStyle::default();

    // Hide contour 0
    state.glyph.contours[0].visible = false;
    let renderer = ShapeLocatorRenderer::new(&state.glyph, style.clone());
    let (_, focus_mesh_hidden) = renderer.render_locator(state.glyph.contours[0].id, size);
    assert!(
        focus_mesh_hidden.vertices.is_empty(),
        "Hidden contour must not produce focus vertices"
    );

    // Empty / degenerate glyph
    let empty_glyph = grafia::core::Glyph::new("empty", 1000);
    let empty_renderer = ShapeLocatorRenderer::new(&empty_glyph, style);
    let (ctx_m, foc_m) = empty_renderer.render_locator(grafia::core::ContourId(0), size);
    assert!(ctx_m.is_empty(), "Empty glyph context mesh must be empty");
    assert!(foc_m.is_empty(), "Empty glyph focus mesh must be empty");
}

fn point_in_triangle_2d(p: Pos2, a: Pos2, b: Pos2, c: Pos2) -> bool {
    let sign = |p1: Pos2, p2: Pos2, p3: Pos2| -> f32 {
        (p1.x - p3.x) * (p2.y - p3.y) - (p2.x - p3.x) * (p1.y - p3.y)
    };
    let d1 = sign(p, a, b);
    let d2 = sign(p, b, c);
    let d3 = sign(p, c, a);
    let has_neg = (d1 < 0.0) || (d2 < 0.0) || (d3 < 0.0);
    let has_pos = (d1 > 0.0) || (d2 > 0.0) || (d3 > 0.0);
    !(has_neg && has_pos)
}

fn mesh_covers_point(mesh: &egui::Mesh, pt: Pos2) -> bool {
    for &[i0, i1, i2] in mesh.indices.as_chunks::<3>().0 {
        let a = mesh.vertices[i0 as usize].pos;
        let b = mesh.vertices[i1 as usize].pos;
        let c = mesh.vertices[i2 as usize].pos;
        if point_in_triangle_2d(pt, a, b, c) {
            return true;
        }
    }
    false
}

#[test]
fn test_shape_locator_open_contour_renders_stroked_focus_mesh_not_filled_chord() {
    let size = vec2(24.0, 16.0);
    let style = ShapeLocatorStyle::default();

    // 1. A 2-node straight line open contour (closed: false) must produce non-empty focus_mesh triangles
    let mut line_glyph = grafia::core::Glyph::new("line", 1000);
    let mut line_contour = grafia::core::GlyphContour::new(
        grafia::core::ContourId(0),
        vec![
            grafia::core::Node::new(
                grafia::core::NodeId(1),
                100.0,
                100.0,
                grafia::core::NodeType::Corner,
            ),
            grafia::core::Node::new(
                grafia::core::NodeId(2),
                400.0,
                400.0,
                grafia::core::NodeType::Corner,
            ),
        ],
    );
    line_contour.closed = false;
    line_glyph.contours.push(line_contour);

    let line_renderer = ShapeLocatorRenderer::new(&line_glyph, style.clone());
    let (line_ctx_mesh, line_focus_mesh) =
        line_renderer.render_locator(grafia::core::ContourId(0), size);
    assert!(
        line_ctx_mesh.is_empty(),
        "Open contour must not be filled in context_mesh"
    );
    assert!(
        !line_focus_mesh.vertices.is_empty() && !line_focus_mesh.indices.is_empty(),
        "2-node open straight line contour must produce non-empty stroked focus_mesh triangles"
    );

    // 2. Open C-arc contour (left half of /o after Shift+Delete on right extremum (510, 250))
    let mut state = GlifoCanvasState::init_glyph_o();
    let right_node_id = state.contours[0]
        .iter()
        .find(|n| (n.point.x.0 - 510.0).abs() < 1.0 && (n.point.y.0 - 250.0).abs() < 1.0)
        .expect("right extremum node of /o")
        .id;
    grafia::core::cut_open_delete_nodes(&mut state.contours[0], &[right_node_id]);
    state.set_contour_closed(0, false);
    state.sync_paths_from_contours();
    assert!(!state.glyph.contours[0].closed);

    let arc_renderer = ShapeLocatorRenderer::new(&state.glyph, style);
    let (_, arc_focus_mesh) = arc_renderer.render_locator(grafia::core::ContourId(0), size);
    assert!(
        !arc_focus_mesh.indices.is_empty(),
        "Open C-arc contour must produce non-empty focus_mesh"
    );

    // Compute the locator-space coordinates of the C-arc cavity interior (200.0, 250.0) in font space.
    // The open C-arc runs from (280, -10) around left extremum (50, 250) to (280, 510),
    // while contour 1 spans x in [140, 420], so overall glyph bounds are x in [50, 420], y in [-10, 510].
    let min_x = 50.0_f64;
    let max_x = 420.0_f64;
    let min_y = -10.0_f64;
    let max_y = 510.0_f64;
    let pad = 1.0_f64;
    let avail_w = (size.x as f64 - pad * 2.0).max(1.0);
    let avail_h = (size.y as f64 - pad * 2.0).max(1.0);
    let scale = (avail_w / (max_x - min_x)).min(avail_h / (max_y - min_y));
    let offset_x = pad + (avail_w - (max_x - min_x) * scale) * 0.5;
    let offset_y = pad + (avail_h - (max_y - min_y) * scale) * 0.5;
    let cavity_pt = Pos2::new(
        (offset_x + (200.0 - min_x) * scale) as f32,
        (offset_y + (max_y - 250.0) * scale) as f32,
    );

    assert!(
        !mesh_covers_point(&arc_focus_mesh, cavity_pt),
        "Open C-arc focus_mesh must be stroked, NOT filled across the interior chord cavity at {cavity_pt:?}"
    );
}

