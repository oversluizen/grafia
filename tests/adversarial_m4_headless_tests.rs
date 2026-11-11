//! Adversarial Empirical Verification Suite for Headless Offscreen Rasterization (Milestone 4).
//!
//! Normative References:
//! - GL-1005: Code for anything a machine could tell you got wrong.
//! - STD-ARCH: Industry Software Engineering & Testing Standards (Rules 0, 3, 4, 8, 12).
//! - Milestone 4 Mandate: Headless Offscreen Rasterizer Adversarial Verifier.

use grafia::adapters::headless::{
    kurbo_to_tiny_skia_path, pixmap_to_rgba_image, rasterize_canvas_subregion,
    rasterize_full_window,
};
use grafia::adapters::state::{CanvasViewMode, GlifoCanvasState};
use grafia::core::{FontPoint, Node, NodeId, NodeType, ScreenPoint};
use grafia::tokens::{
    GlifoTheme, GOLDEN_MASTER_CANVAS_WIDTH_DP,
    GOLDEN_MASTER_SIDEBAR_WIDTH_DP, GOLDEN_MASTER_SPLITTER_WIDTH_DP,
    GOLDEN_MASTER_WINDOW_HEIGHT_DP, GOLDEN_MASTER_WINDOW_WIDTH_DP, MIN_VIEWPORT_ZOOM,
};
use kurbo::{BezPath, Point};
use std::panic::catch_unwind;

/// Helper to count pixels in a Pixmap matching a target RGBA color within a per-channel tolerance.
fn count_matching_pixels(
    pixmap: &tiny_skia::Pixmap,
    target: [u8; 4],
    tolerance: u8,
) -> usize {
    let mut count = 0;
    for pixel in pixmap.pixels() {
        let dr = (pixel.red() as i16 - target[0] as i16).unsigned_abs() as u8;
        let dg = (pixel.green() as i16 - target[1] as i16).unsigned_abs() as u8;
        let db = (pixel.blue() as i16 - target[2] as i16).unsigned_abs() as u8;
        let da = (pixel.alpha() as i16 - target[3] as i16).unsigned_abs() as u8;
        if dr <= tolerance && dg <= tolerance && db <= tolerance && da <= tolerance {
            count += 1;
        }
    }
    count
}

// ============================================================================
// 1. View Mode Parity: EditWireframe
// ============================================================================

#[test]
fn test_view_mode_edit_wireframe_all_node_types_and_contours_pixel_coverage() {
    let theme = GlifoTheme::sonoma_default();
    let mut state = GlifoCanvasState::new(theme.clone());
    state.view_mode = CanvasViewMode::EditWireframe;

    // Viewport: 1.0 zoom, pan at (200.0, 200.0)
    state.viewport.zoom = 1.0;
    state.viewport.pan_offset = ScreenPoint::new(200.0, 200.0);

    // Build a contour with Corner, Smooth, and OffCurve (BCP handle) nodes
    let contour = vec![
        Node::new(NodeId(1), -100.0, -100.0, NodeType::Corner),
        Node::new(NodeId(2), 0.0, 100.0, NodeType::Smooth),
        Node::new(NodeId(3), 50.0, 150.0, NodeType::OffCurve),
        Node::new(NodeId(4), 100.0, -100.0, NodeType::Corner),
    ];

    state.contours.push(contour);
    state.sync_paths_from_contours();

    let width = 600;
    let height = 600;
    let pixmap = rasterize_canvas_subregion(&state, &theme, width, height);

    // Corner color: #2B78E4 -> [43, 120, 228, 255]
    let corner_c = theme.edit_canvas.corner_node_color32();
    let corner_target = [corner_c.r(), corner_c.g(), corner_c.b(), corner_c.a()];
    let corner_pixels = count_matching_pixels(&pixmap, corner_target, 5);

    // Smooth color: #3CAD66 -> [60, 173, 102, 255]
    let smooth_c = theme.edit_canvas.smooth_node_color32();
    let smooth_target = [smooth_c.r(), smooth_c.g(), smooth_c.b(), smooth_c.a()];
    let smooth_pixels = count_matching_pixels(&pixmap, smooth_target, 10);

    // BCP handle color: #8C6845 -> [140, 104, 69, 255]
    let handle_c = theme.edit_canvas.bcp_handle_color32();
    let handle_target = [handle_c.r(), handle_c.g(), handle_c.b(), handle_c.a()];
    let handle_pixels = count_matching_pixels(&pixmap, handle_target, 10);

    // Contour stroke: #000000 -> [0, 0, 0, 255]
    let stroke_c = theme.preview_canvas.fill_color32();
    let stroke_target = [stroke_c.r(), stroke_c.g(), stroke_c.b(), stroke_c.a()];
    let stroke_pixels = count_matching_pixels(&pixmap, stroke_target, 20);

    // Background: #FFFFFF -> [255, 255, 255, 255]
    let bg_c = theme.canvas_background_color32();
    let bg_target = [bg_c.r(), bg_c.g(), bg_c.b(), bg_c.a()];
    let bg_pixels = count_matching_pixels(&pixmap, bg_target, 5);

    assert!(corner_pixels > 0, "Corner nodes must have non-zero pixel coverage (got {corner_pixels})");
    assert!(smooth_pixels > 0, "Smooth nodes must have non-zero pixel coverage (got {smooth_pixels})");
    assert!(handle_pixels > 0, "BCP handles must have non-zero pixel coverage (got {handle_pixels})");
    assert!(stroke_pixels > 0, "Contour stroke must have non-zero pixel coverage (got {stroke_pixels})");
    assert!(bg_pixels > 0, "Canvas background must have non-zero pixel coverage (got {bg_pixels})");
}

// ============================================================================
// 2. View Mode Parity: PreviewFilled & EvenOdd Counter Hole Subtraction
// ============================================================================

#[test]
fn test_view_mode_preview_filled_even_odd_compound_path_subtraction() {
    let theme = GlifoTheme::sonoma_default();
    let mut state = GlifoCanvasState::init_glyph_a();
    state.view_mode = CanvasViewMode::PreviewFilled;

    // In typography standards, compound paths represent contours + counter holes.
    let mut compound = BezPath::new();
    for p in &state.paths {
        for el in p.elements() {
            compound.push(*el);
        }
    }
    state.paths = vec![compound];

    let width = 800;
    let height = 600;
    let pixmap = rasterize_canvas_subregion(&state, &theme, width, height);

    // 1. Center of the counter hole (triangle: (187, 240), (413, 240), (300, 480))
    let counter_center_font = FontPoint::new(300.0, 320.0);
    let counter_scr = state.viewport.to_screen(counter_center_font);
    let hx = counter_scr.x.0.round() as u32;
    let hy = counter_scr.y.0.round() as u32;

    let hole_pixel = pixmap.pixel(hx, hy).expect("Counter hole pixel must be inside bounds");
    assert_eq!(
        hole_pixel.red(),
        255,
        "EvenOdd subtraction: Counter hole center must remain background (#FFFFFF)"
    );

    // 2. Point inside the solid body of glyph /A (e.g. left stem)
    let body_font = FontPoint::new(150.0, 150.0);
    let body_scr = state.viewport.to_screen(body_font);
    let bx = body_scr.x.0.round() as u32;
    let by = body_scr.y.0.round() as u32;

    let body_pixel = pixmap.pixel(bx, by).expect("Body pixel must be inside bounds");
    assert_eq!(
        body_pixel.red(),
        0,
        "Glyph body must be filled solid black (#000000)"
    );

    // 3. Point far outside the glyph (pure background)
    let outside_pixel = pixmap.pixel(10, 10).expect("Outside pixel must be inside bounds");
    assert_eq!(
        outside_pixel.red(),
        255,
        "Outside canvas region must remain background (#FFFFFF)"
    );
}

#[test]
fn test_view_mode_preview_filled_glyph_a_counter_hole_empirical() {
    let theme = GlifoTheme::sonoma_default();
    let mut state = GlifoCanvasState::init_glyph_a();
    state.view_mode = CanvasViewMode::PreviewFilled;

    // init_glyph_a has separate outer and inner paths in state.paths
    assert_eq!(state.paths.len(), 2, "Glyph /A has disjoint outer and inner paths");

    let width = 800;
    let height = 600;
    let pixmap = rasterize_canvas_subregion(&state, &theme, width, height);

    // 1. Counter hole center: font coordinate (300.0, 320.0) -> screen (389, 306)
    let counter_center_font = FontPoint::new(300.0, 320.0);
    let counter_scr = state.viewport.to_screen(counter_center_font);
    let hx = counter_scr.x.0.round() as u32;
    let hy = counter_scr.y.0.round() as u32;

    let hole_pixel = pixmap.pixel(hx, hy).expect("Counter hole pixel inside bounds");
    assert_eq!(
        hole_pixel.red(),
        255,
        "Counter hole center MUST be white (background) due to EvenOdd compound path subtraction! Got R={}",
        hole_pixel.red()
    );
    assert_eq!(hole_pixel.green(), 255);
    assert_eq!(hole_pixel.blue(), 255);

    // Also assert direct pixel at (389, 306)
    let direct_hole = pixmap.pixel(389, 306).expect("Direct pixel (389, 306)");
    assert_eq!(direct_hole.red(), 255, "Pixel at (389, 306) must be white canvas background");
    assert_eq!(direct_hole.green(), 255);
    assert_eq!(direct_hole.blue(), 255);

    // 2. Glyph body: font coordinate (150.0, 150.0)
    let body_font = FontPoint::new(150.0, 150.0);
    let body_scr = state.viewport.to_screen(body_font);
    let bx = body_scr.x.0.round() as u32;
    let by = body_scr.y.0.round() as u32;

    let body_pixel = pixmap.pixel(bx, by).expect("Body pixel inside bounds");
    assert_eq!(
        body_pixel.red(),
        0,
        "Glyph body must be solid black (#000000)! Got R={}",
        body_pixel.red()
    );
    assert_eq!(body_pixel.green(), 0);
    assert_eq!(body_pixel.blue(), 0);

    // 3. Canvas background far outside glyph
    let outside_pixel = pixmap.pixel(10, 10).expect("Outside pixel inside bounds");
    assert_eq!(outside_pixel.red(), 255);
    assert_eq!(outside_pixel.green(), 255);
    assert_eq!(outside_pixel.blue(), 255);
}

// ============================================================================
// 3. Degenerate Geometry Handling
// ============================================================================

#[test]
fn test_degenerate_empty_contours_list_produces_clean_background() {
    let theme = GlifoTheme::sonoma_default();
    let mut state = GlifoCanvasState::new(theme.clone());
    state.contours.clear();
    state.paths.clear();

    for mode in [CanvasViewMode::EditWireframe, CanvasViewMode::PreviewFilled, CanvasViewMode::ShapeThumbnail] {
        state.view_mode = mode;
        let pixmap = rasterize_canvas_subregion(&state, &theme, 200, 200);
        let bg_c = theme.canvas_background_color32();
        let bg_target = [bg_c.r(), bg_c.g(), bg_c.b(), bg_c.a()];
        let matching = count_matching_pixels(&pixmap, bg_target, 0);
        assert_eq!(matching, 200 * 200, "Empty contours must produce 100% clean background in {mode:?}");
    }
}

#[test]
fn test_degenerate_single_point_contour_handles_gracefully() {
    let theme = GlifoTheme::sonoma_default();
    let mut state = GlifoCanvasState::new(theme.clone());

    // Single-point contour (MoveTo only)
    let contour = vec![Node::new(NodeId(1), 100.0, 100.0, NodeType::Corner)];
    state.contours.push(contour);
    state.sync_paths_from_contours();

    for mode in [CanvasViewMode::EditWireframe, CanvasViewMode::PreviewFilled] {
        state.view_mode = mode;
        let pixmap = rasterize_canvas_subregion(&state, &theme, 300, 300);
        assert_eq!(pixmap.width(), 300);
        assert_eq!(pixmap.height(), 300);
    }
}

#[test]
fn test_degenerate_coincident_and_zero_length_segments() {
    let theme = GlifoTheme::sonoma_default();
    let mut state = GlifoCanvasState::new(theme.clone());

    // Coincident points: MoveTo(50, 50) -> LineTo(50, 50) -> LineTo(50, 50)
    let mut zero_len_path = BezPath::new();
    zero_len_path.move_to(Point::new(50.0, 50.0));
    zero_len_path.line_to(Point::new(50.0, 50.0));
    zero_len_path.close_path();

    state.paths.push(zero_len_path);
    state.contours.push(vec![
        Node::new(NodeId(1), 50.0, 50.0, NodeType::Corner),
        Node::new(NodeId(2), 50.0, 50.0, NodeType::Corner),
    ]);

    for mode in [CanvasViewMode::EditWireframe, CanvasViewMode::PreviewFilled] {
        state.view_mode = mode;
        let pixmap = rasterize_canvas_subregion(&state, &theme, 200, 200);
        assert_eq!(pixmap.width(), 200);
        assert_eq!(pixmap.height(), 200);
    }
}

#[test]
fn test_degenerate_pixmap_dimensions_1x1_and_0x0() {
    let state = GlifoCanvasState::init_glyph_a();
    let theme = GlifoTheme::sonoma_default();

    // 1x1 pixmap: must succeed and produce valid pixel
    let pixmap_1x1 = rasterize_canvas_subregion(&state, &theme, 1, 1);
    assert_eq!(pixmap_1x1.width(), 1);
    assert_eq!(pixmap_1x1.height(), 1);

    // 0x0 pixmap: tiny-skia requires width > 0 and height > 0.
    // Verify that catch_unwind detects the expected panic without crashing the test runner.
    let panic_result = catch_unwind(|| {
        rasterize_canvas_subregion(&state, &theme, 0, 0)
    });
    assert!(panic_result.is_err(), "0x0 pixmap allocation is caught via catch_unwind");
}

// ============================================================================
// 4. Viewport Scaling & Translation Stability
// ============================================================================

#[test]
fn test_viewport_extreme_zoom_and_large_translations() {
    let theme = GlifoTheme::sonoma_default();
    let mut state = GlifoCanvasState::init_glyph_a();

    // 1. Minimum zoom (0.01)
    state.viewport.zoom = MIN_VIEWPORT_ZOOM;
    state.viewport.pan_offset = ScreenPoint::new(100.0, 100.0);
    let pixmap_min_zoom = rasterize_canvas_subregion(&state, &theme, 400, 400);
    assert_eq!(pixmap_min_zoom.width(), 400);

    // 2. Extreme magnification (50.0)
    state.viewport.zoom = 50.0;
    state.viewport.pan_offset = ScreenPoint::new(200.0, 200.0);
    let pixmap_max_zoom = rasterize_canvas_subregion(&state, &theme, 400, 400);
    assert_eq!(pixmap_max_zoom.width(), 400);

    // 3. Negative large translation (offscreen top-left)
    state.viewport.zoom = 1.0;
    state.viewport.pan_offset = ScreenPoint::new(-100_000.0, -100_000.0);
    let pixmap_neg_pan = rasterize_canvas_subregion(&state, &theme, 300, 300);
    assert_eq!(pixmap_neg_pan.width(), 300);

    // 4. Positive large translation (offscreen bottom-right)
    state.viewport.pan_offset = ScreenPoint::new(100_000.0, 100_000.0);
    let pixmap_pos_pan = rasterize_canvas_subregion(&state, &theme, 300, 300);
    assert_eq!(pixmap_pos_pan.width(), 300);
}

// ============================================================================
// 5. Utility & Subsystem Parity
// ============================================================================

#[test]
fn test_kurbo_to_tiny_skia_path_element_mapping() {
    // Empty path
    let empty = BezPath::new();
    assert!(kurbo_to_tiny_skia_path(&empty).is_none());

    // Single MoveTo
    let mut move_only = BezPath::new();
    move_only.move_to(Point::new(10.0, 10.0));
    assert!(kurbo_to_tiny_skia_path(&move_only).is_none());

    // Full path with LineTo, QuadTo, CurveTo, ClosePath
    let mut full = BezPath::new();
    full.move_to(Point::new(0.0, 0.0));
    full.line_to(Point::new(10.0, 0.0));
    full.quad_to(Point::new(15.0, 5.0), Point::new(20.0, 10.0));
    full.curve_to(Point::new(25.0, 15.0), Point::new(30.0, 20.0), Point::new(35.0, 25.0));
    full.close_path();

    let ts_path = kurbo_to_tiny_skia_path(&full);
    assert!(ts_path.is_some(), "Complete kurbo BezPath must map to tiny_skia Path");
}

#[test]
fn test_pixmap_to_rgba_image_roundtrip() {
    let state = GlifoCanvasState::init_glyph_a();
    let theme = GlifoTheme::sonoma_default();
    let pixmap = rasterize_canvas_subregion(&state, &theme, 100, 80);

    let img = pixmap_to_rgba_image(pixmap);
    assert_eq!(img.width(), 100);
    assert_eq!(img.height(), 80);
}

#[test]
fn test_rasterize_full_window_layout_and_dimensions() {
    let state = GlifoCanvasState::init_glyph_a();
    let theme = GlifoTheme::sonoma_default();
    let pixmap = rasterize_full_window(&state, &theme);

    let expected_w = GOLDEN_MASTER_WINDOW_WIDTH_DP as u32;
    let expected_h = GOLDEN_MASTER_WINDOW_HEIGHT_DP as u32;

    assert_eq!(pixmap.width(), expected_w);
    assert_eq!(pixmap.height(), expected_h);

    // Sidebar region check (e.g. x = 50, y = 50)
    let sidebar_c = theme.sidebar_background_color32();
    let sp = pixmap.pixel(50, 50).expect("Sidebar pixel inside bounds");
    assert_eq!(sp.red(), sidebar_c.r());
    assert_eq!(sp.green(), sidebar_c.g());
    assert_eq!(sp.blue(), sidebar_c.b());

    // Splitter line check (at GOLDEN_MASTER_SIDEBAR_WIDTH_DP = 250)
    let splitter_c = theme.splitter_color32();
    let split_p = pixmap.pixel(GOLDEN_MASTER_SIDEBAR_WIDTH_DP as u32, 50).expect("Splitter pixel");
    assert_eq!(split_p.red(), splitter_c.r());
    assert_eq!(split_p.green(), splitter_c.g());
    assert_eq!(split_p.blue(), splitter_c.b());

    // Canvas region check (e.g. x = 300, y = 50)
    let canvas_c = theme.canvas_background_color32();
    let cp = pixmap.pixel(300, 50).expect("Canvas pixel inside bounds");
    assert_eq!(cp.red(), canvas_c.r());
    assert_eq!(cp.green(), canvas_c.g());
    assert_eq!(cp.blue(), canvas_c.b());

    // Infobox region check
    let infobox_x = (GOLDEN_MASTER_SIDEBAR_WIDTH_DP
        + GOLDEN_MASTER_SPLITTER_WIDTH_DP
        + GOLDEN_MASTER_CANVAS_WIDTH_DP
        + GOLDEN_MASTER_SPLITTER_WIDTH_DP
        + 20.0) as u32;
    let infobox_c = theme.infobox_background_color32();
    let ip = pixmap.pixel(infobox_x, 50).expect("Infobox pixel inside bounds");
    assert_eq!(ip.red(), infobox_c.r());
    assert_eq!(ip.green(), infobox_c.g());
    assert_eq!(ip.blue(), infobox_c.b());
}

// ============================================================================
// 6. Multi-Contour & Nested Compound Path Stress Tests (EvenOdd Parity)
// ============================================================================

#[test]
fn test_view_mode_preview_filled_glyph_b_two_counter_holes() {
    let theme = GlifoTheme::sonoma_default();
    let mut state = GlifoCanvasState::new(theme.clone());
    state.view_mode = CanvasViewMode::PreviewFilled;
    state.viewport.zoom = 1.0;
    state.viewport.pan_offset = ScreenPoint::new(0.0, 800.0);

    // Outer hull of /B: (100, 100) to (400, 700)
    let mut outer = BezPath::new();
    outer.move_to(Point::new(100.0, 100.0));
    outer.line_to(Point::new(400.0, 100.0));
    outer.line_to(Point::new(400.0, 700.0));
    outer.line_to(Point::new(100.0, 700.0));
    outer.close_path();

    // Upper counter hole (CW opposite winding): (180, 420) -> (180, 620) -> (320, 620) -> (320, 420)
    let mut upper_hole = BezPath::new();
    upper_hole.move_to(Point::new(180.0, 420.0));
    upper_hole.line_to(Point::new(180.0, 620.0));
    upper_hole.line_to(Point::new(320.0, 620.0));
    upper_hole.line_to(Point::new(320.0, 420.0));
    upper_hole.close_path();

    // Lower counter hole (CW opposite winding): (180, 180) -> (180, 380) -> (320, 380) -> (320, 180)
    let mut lower_hole = BezPath::new();
    lower_hole.move_to(Point::new(180.0, 180.0));
    lower_hole.line_to(Point::new(180.0, 380.0));
    lower_hole.line_to(Point::new(320.0, 380.0));
    lower_hole.line_to(Point::new(320.0, 180.0));
    lower_hole.close_path();

    state.paths = vec![outer, upper_hole, lower_hole];

    let pixmap = rasterize_canvas_subregion(&state, &theme, 600, 800);

    // 1. Upper hole center: font (250, 520) -> screen (250, 280) must be white (255)
    let p_upper = pixmap.pixel(250, 280).expect("Upper hole pixel");
    assert_eq!(p_upper.red(), 255, "Upper counter hole must be white (subtracted by EvenOdd)");
    assert_eq!(p_upper.green(), 255);
    assert_eq!(p_upper.blue(), 255);

    // 2. Lower hole center: font (250, 280) -> screen (250, 520) must be white (255)
    let p_lower = pixmap.pixel(250, 520).expect("Lower hole pixel");
    assert_eq!(p_lower.red(), 255, "Lower counter hole must be white (subtracted by EvenOdd)");
    assert_eq!(p_lower.green(), 255);
    assert_eq!(p_lower.blue(), 255);

    // 3. Middle crossbar between holes: font (250, 400) -> screen (250, 400) must be black (0)
    let p_bar = pixmap.pixel(250, 400).expect("Crossbar pixel");
    assert_eq!(p_bar.red(), 0, "Crossbar between holes must be black body");

    // 4. Left stem: font (140, 400) -> screen (140, 400) must be black (0)
    let p_stem = pixmap.pixel(140, 400).expect("Stem pixel");
    assert_eq!(p_stem.red(), 0, "Stem must be black body");

    // 5. Outside: font (50, 50) -> screen (50, 750) must be white (255)
    let p_out = pixmap.pixel(50, 750).expect("Outside pixel");
    assert_eq!(p_out.red(), 255, "Outside glyph must be white background");
}

#[test]
fn test_view_mode_preview_filled_accented_glyph_aacute_disjoint_contours() {
    let theme = GlifoTheme::sonoma_default();
    let mut state = GlifoCanvasState::new(theme.clone());
    state.view_mode = CanvasViewMode::PreviewFilled;
    state.viewport.zoom = 1.0;
    state.viewport.pan_offset = ScreenPoint::new(0.0, 800.0);

    // Outer hull of /A: (100, 100) to (400, 600)
    let mut base_outer = BezPath::new();
    base_outer.move_to(Point::new(100.0, 100.0));
    base_outer.line_to(Point::new(400.0, 100.0));
    base_outer.line_to(Point::new(400.0, 600.0));
    base_outer.line_to(Point::new(100.0, 600.0));
    base_outer.close_path();

    // Inner counter hole of /A (CW opposite winding): (200, 250) -> (200, 450) -> (300, 450) -> (300, 250)
    let mut base_hole = BezPath::new();
    base_hole.move_to(Point::new(200.0, 250.0));
    base_hole.line_to(Point::new(200.0, 450.0));
    base_hole.line_to(Point::new(300.0, 450.0));
    base_hole.line_to(Point::new(300.0, 250.0));
    base_hole.close_path();

    // Disjoint acute accent contour above /A: (220, 680) to (340, 760)
    let mut accent = BezPath::new();
    accent.move_to(Point::new(220.0, 680.0));
    accent.line_to(Point::new(340.0, 680.0));
    accent.line_to(Point::new(340.0, 760.0));
    accent.line_to(Point::new(220.0, 760.0));
    accent.close_path();

    state.paths = vec![base_outer, base_hole, accent];

    let pixmap = rasterize_canvas_subregion(&state, &theme, 600, 800);

    // 1. Accent body: font (280, 720) -> screen (280, 80) must be black (0)
    let p_acc = pixmap.pixel(280, 80).expect("Accent pixel");
    assert_eq!(p_acc.red(), 0, "Accent mark must be black body");

    // 2. Gap between accent and /A: font (280, 640) -> screen (280, 160) must be white (255)
    let p_gap = pixmap.pixel(280, 160).expect("Gap pixel");
    assert_eq!(p_gap.red(), 255, "Gap between /A and accent must be white");

    // 3. /A body: font (150, 350) -> screen (150, 450) must be black (0)
    let p_body = pixmap.pixel(150, 450).expect("Body pixel");
    assert_eq!(p_body.red(), 0, "/A body must be black");

    // 4. /A counter hole: font (250, 350) -> screen (250, 450) must be white (255)
    let p_hole = pixmap.pixel(250, 450).expect("Counter hole pixel");
    assert_eq!(p_hole.red(), 255, "/A counter hole must be white (subtracted by EvenOdd)");

    // 5. Outside: font (50, 50) -> screen (50, 750) must be white (255)
    let p_out = pixmap.pixel(50, 750).expect("Outside pixel");
    assert_eq!(p_out.red(), 255, "Outside must be white background");
}

#[test]
fn test_view_mode_preview_filled_concentric_nested_even_odd_rings() {
    let theme = GlifoTheme::sonoma_default();
    let mut state = GlifoCanvasState::new(theme.clone());
    state.view_mode = CanvasViewMode::PreviewFilled;
    state.viewport.zoom = 1.0;
    state.viewport.pan_offset = ScreenPoint::new(0.0, 800.0);

    // Outer: (100, 100) to (500, 500)
    let mut ring1 = BezPath::new();
    ring1.move_to(Point::new(100.0, 100.0));
    ring1.line_to(Point::new(500.0, 100.0));
    ring1.line_to(Point::new(500.0, 500.0));
    ring1.line_to(Point::new(100.0, 500.0));
    ring1.close_path();

    // Middle counter ring (CW opposite winding): (200, 200) -> (200, 400) -> (400, 400) -> (400, 200)
    let mut ring2 = BezPath::new();
    ring2.move_to(Point::new(200.0, 200.0));
    ring2.line_to(Point::new(200.0, 400.0));
    ring2.line_to(Point::new(400.0, 400.0));
    ring2.line_to(Point::new(400.0, 200.0));
    ring2.close_path();

    // Inner: (260, 260) to (340, 340)
    let mut ring3 = BezPath::new();
    ring3.move_to(Point::new(260.0, 260.0));
    ring3.line_to(Point::new(340.0, 260.0));
    ring3.line_to(Point::new(340.0, 340.0));
    ring3.line_to(Point::new(260.0, 340.0));
    ring3.close_path();

    state.paths = vec![ring1, ring2, ring3];

    let pixmap = rasterize_canvas_subregion(&state, &theme, 600, 800);

    // 1. Outer band (1 crossing): font (150, 300) -> screen (150, 500) -> black (0)
    let p_outer = pixmap.pixel(150, 500).expect("Outer band pixel");
    assert_eq!(p_outer.red(), 0, "Outer band (1 crossing) must be black");

    // 2. Middle band (2 crossings): font (230, 300) -> screen (230, 500) -> white (255)
    let p_mid = pixmap.pixel(230, 500).expect("Middle band pixel");
    assert_eq!(p_mid.red(), 255, "Middle band (2 crossings) must be white (subtracted)");

    // 3. Inner island (3 crossings): font (300, 300) -> screen (300, 500) -> black (0)
    let p_inner = pixmap.pixel(300, 500).expect("Inner center pixel");
    assert_eq!(p_inner.red(), 0, "Inner center (3 crossings) must be black (filled again by EvenOdd)");

    // 4. Outside (0 crossings): font (50, 50) -> screen (50, 750) -> white (255)
    let p_out = pixmap.pixel(50, 750).expect("Outside pixel");
    assert_eq!(p_out.red(), 255, "Outside (0 crossings) must be white background");
}
