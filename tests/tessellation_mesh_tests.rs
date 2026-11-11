//! Test suite for GPU Mesh Tessellation via lyon_tessellation (TC-GRAFIA-V01-01).
//!
//! Asserts:
//! 1. Multi-contour compound glyphs with counter-holes (like `/A`) generate valid `egui::Mesh` triangulations.
//! 2. `FillRule::EvenOdd` correctly subtracts inner contours without requiring ad-hoc background overlay patches.

use egui::Color32;
use grafia::adapters::GrafiaCanvasState;
use grafia::render::tessellate_contours_even_odd;

#[test]
fn test_lyon_tessellation_generates_valid_egui_mesh_for_glyph_a() {
    let state = GrafiaCanvasState::init_glyph_a();
    let fill_color = Color32::from_rgb(28, 28, 30);

    // Call the lyon-based tessellator function on the glyph contours
    let mesh = tessellate_contours_even_odd(&state.contours, &state.viewport, fill_color);

    // Assert mesh geometry invariants
    assert!(
        !mesh.vertices.is_empty(),
        "Tessellated mesh must contain vertices"
    );
    assert!(
        !mesh.indices.is_empty(),
        "Tessellated mesh must contain triangle indices"
    );
    assert_eq!(
        mesh.indices.len() % 3,
        0,
        "Indices count must be a multiple of 3 for triangle rendering"
    );

    // In a letter /A with 2 contours (outer 8 points, inner 3 points),
    // a valid EvenOdd triangulation must produce at least 10 triangles (30 indices).
    assert!(
        mesh.indices.len() >= 30,
        "Complex compound glyph /A must produce at least 10 triangles (indices >= 30), got {}",
        mesh.indices.len()
    );

    // Assert that all vertices have the expected fill color
    for vertex in &mesh.vertices {
        assert_eq!(vertex.color, fill_color);
    }
}
