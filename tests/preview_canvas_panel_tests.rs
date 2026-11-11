//! Test Suite for Upgraded ShapeLocator Dimensions (48x36 dp) and Live Glyph Preview Panel.
//!
//! Normative References:
//! - STD-ARCH: De Verankeringswet (Red Test First).
//! - Vector Canvas Sidebar Outline View shape thumbnail size (48x36 dp).
//! - Vector Canvas Bottom Preview Panel (`PreviewPanel`, `GOLDEN_MASTER_CANVAS_PREVIEW_HEIGHT_DP = 163.0`)
//!   with Normal, Invert (white-on-black), and Flip (upside-down vertical reflection) modes.

use egui::{vec2, Pos2, Rect, Vec2};
use grafia::adapters::headless::{
    pixmap_to_rgba_image, rasterize_full_window, rasterize_preview_panel,
};
use grafia::adapters::{CanvasViewMode, GrafiaCanvasState, GrafiaCanvasWidget};
use grafia::locator::{ShapeLocatorRenderer, ShapeLocatorStyle};
use grafia::render::{GrafiaPreviewWidget, PreviewPanelConfig};
use grafia::tokens::{
    GrafiaTheme, GOLDEN_MASTER_CANVAS_EDIT_HEIGHT_DP, GOLDEN_MASTER_CANVAS_PREVIEW_HEIGHT_DP,
    GOLDEN_MASTER_CANVAS_WIDTH_DP, GOLDEN_MASTER_SIDEBAR_WIDTH_DP, LOCATOR_DEFAULT_HEIGHT_DP,
    LOCATOR_DEFAULT_WIDTH_DP, PREVIEW_PANEL_DEFAULT_HEIGHT_DP,
};
use image::{Rgba, RgbaImage};
use std::path::PathBuf;

#[test]
fn test_shape_locator_default_dimensions_match_v010_sidebar() {
    assert_eq!(
        LOCATOR_DEFAULT_WIDTH_DP, 48.0,
        "Sidebar shape thumbnail width must be 48.0 dp to match Vector Canvas"
    );
    assert_eq!(
        LOCATOR_DEFAULT_HEIGHT_DP, 36.0,
        "Sidebar shape thumbnail height must be 36.0 dp to match Vector Canvas"
    );

    let default_style = ShapeLocatorStyle::default();
    assert_eq!(default_style.size, vec2(48.0, 36.0));
    assert_eq!(default_style.padding, 2.0);

    let state = GrafiaCanvasState::init_glyph_a();
    let renderer = ShapeLocatorRenderer::new(&state.glyph, default_style.clone());
    let (ctx_m, foc_m) = renderer.render_locator(state.glyph.contours[0].id, default_style.size);
    assert!(!ctx_m.vertices.is_empty());
    assert!(!foc_m.vertices.is_empty());

    for v in &ctx_m.vertices {
        assert!(
            v.pos.x >= -1.0 && v.pos.x <= 49.0 && v.pos.y >= -1.0 && v.pos.y <= 37.0,
            "Vertex {:?} must lie within 48x36 bounds",
            v.pos
        );
    }
}

#[test]
fn test_preview_panel_default_height_matches_golden_master_163dp() {
    assert_eq!(
        PREVIEW_PANEL_DEFAULT_HEIGHT_DP, GOLDEN_MASTER_CANVAS_PREVIEW_HEIGHT_DP,
        "Preview panel default height must match GOLDEN_MASTER_CANVAS_PREVIEW_HEIGHT_DP (163.0 dp)"
    );
    let cfg = PreviewPanelConfig::default();
    assert_eq!(cfg.height_dp, 163.0);
    assert!(!cfg.inverted);
    assert!(!cfg.flipped);
    assert!(cfg.show_baseline);
}

#[test]
fn test_grafia_preview_widget_normal_inverted_and_flipped_modes() {
    let state = GrafiaCanvasState::init_glyph_a();
    let cfg_normal = PreviewPanelConfig::default();
    let cfg_flipped = PreviewPanelConfig {
        flipped: true,
        ..Default::default()
    };

    let pix_normal = rasterize_preview_panel(&state, &cfg_normal, 400, 163);
    let pix_flipped = rasterize_preview_panel(&state, &cfg_flipped, 400, 163);

    // Count black glyph pixels in upper half vs lower half of the 163px preview canvas.
    // For glyph /A: in normal mode, the wide base + crossbar are in the lower half, so lower_black > upper_black.
    // In flipped (upside-down) mode, the wide base + crossbar are in the upper half, so upper_black > lower_black!
    let count_half_black = |pix: &tiny_skia::Pixmap, top_half: bool| -> usize {
        let y_range = if top_half { 0..81 } else { 82..163 };
        let mut count = 0;
        for y in y_range {
            for x in 0..400 {
                if let Some(p) = pix.pixel(x, y) {
                    if p.red() < 50 && p.green() < 50 && p.blue() < 50 {
                        count += 1;
                    }
                }
            }
        }
        count
    };

    let norm_top = count_half_black(&pix_normal, true);
    let norm_bot = count_half_black(&pix_normal, false);
    assert!(
        norm_bot > norm_top,
        "Normal /A preview must have more black pixels in bottom half ({norm_bot}) than top half ({norm_top})"
    );

    let flip_top = count_half_black(&pix_flipped, true);
    let flip_bot = count_half_black(&pix_flipped, false);
    assert!(
        flip_top > flip_bot,
        "Flipped (upside-down) /A preview must have more black pixels in top half ({flip_top}) than bottom half ({flip_bot})"
    );

    // Also verify egui widget renders flipped mode cleanly
    let ctx = egui::Context::default();
    let screen_rect = Rect::from_min_size(Pos2::ZERO, Vec2::new(735.0, 200.0));
    let raw_input = egui::RawInput {
        screen_rect: Some(screen_rect),
        ..Default::default()
    };
    let mut full_output = ctx.run_ui(raw_input, |ui| {
        ui.add(GrafiaPreviewWidget::new(&state).with_config(cfg_flipped));
    });
    full_output.textures_delta.clear();
    let primitives = ctx.tessellate(full_output.shapes, full_output.pixels_per_point);
    assert!(!primitives.is_empty());
}

#[test]
fn test_rasterize_full_window_renders_bottom_preview_silhouette() {
    let theme = GrafiaTheme::sonoma_default();
    let state = GrafiaCanvasState::init_glyph_a();
    let pixmap = rasterize_full_window(&state, &theme);

    // Check bottom preview area: x in [251..986], y in [618..811]
    let x_start = (GOLDEN_MASTER_SIDEBAR_WIDTH_DP + 1.0) as u32;
    let x_end = x_start + (GOLDEN_MASTER_CANVAS_WIDTH_DP as u32);
    let y_start = (GOLDEN_MASTER_CANVAS_EDIT_HEIGHT_DP + 2.0) as u32;
    let y_end = pixmap.height();

    let mut black_preview_pixels = 0usize;
    for y in y_start..y_end {
        for x in x_start..x_end {
            if let Some(p) = pixmap.pixel(x, y) {
                if p.red() == 0 && p.green() == 0 && p.blue() == 0 {
                    black_preview_pixels += 1;
                }
            }
        }
    }
    assert!(
        black_preview_pixels >= 500,
        "rasterize_full_window must render the filled glyph silhouette in the bottom preview region (got {black_preview_pixels} black pixels)"
    );
}

#[test]
fn test_shape_thumbnail_view_mode_egui_uses_96x64_scaled_cards() {
    let theme = GrafiaTheme::sonoma_default();
    let mut state = GrafiaCanvasState::init_glyph_o();
    state.view_mode = CanvasViewMode::ShapeThumbnail;

    let ctx = egui::Context::default();
    let screen_rect = Rect::from_min_size(Pos2::ZERO, Vec2::new(800.0, 600.0));
    let raw_input = egui::RawInput {
        screen_rect: Some(screen_rect),
        ..Default::default()
    };

    let mut full_output = ctx.run_ui(raw_input, |ui| {
        GrafiaCanvasWidget::new(&mut state, &theme)
            .with_size(Vec2::new(800.0, 600.0))
            .show(ui);
    });
    full_output.textures_delta.clear();
    let primitives = ctx.tessellate(full_output.shapes, full_output.pixels_per_point);

    // Find max x/y span of tessellated thumbnail cards: with 96x64 dp cards (4x 24x16),
    // the second card reaches y > 180 dp (whereas unscaled 24x16 reached only y ~ 62 dp).
    let mut max_y = 0.0_f32;
    for prim in &primitives {
        if let egui::epaint::Primitive::Mesh(mesh) = &prim.primitive {
            for v in &mesh.vertices {
                if v.color == theme.shape_canvas.accent_selection_color32() && v.pos.y > max_y {
                    max_y = v.pos.y;
                }
            }
        }
    }
    assert!(
        max_y >= 180.0,
        "CanvasViewMode::ShapeThumbnail in egui must render 96x64 dp cards matching headless (max_y was {max_y})"
    );
}

#[test]
fn test_generate_visual_proof_preview_panel_and_full_window() {
    let out_dir = PathBuf::from("target/visual_verification");
    std::fs::create_dir_all(&out_dir).expect("create target/visual_verification");

    let state = GrafiaCanvasState::init_glyph_a();
    let theme = GrafiaTheme::sonoma_default();

    let cfg_norm = PreviewPanelConfig::default();
    let cfg_inv = PreviewPanelConfig {
        inverted: true,
        ..Default::default()
    };
    let cfg_flip = PreviewPanelConfig {
        flipped: true,
        ..Default::default()
    };

    let w = 240u32;
    let h = 163u32;
    let img_norm = pixmap_to_rgba_image(rasterize_preview_panel(&state, &cfg_norm, w, h));
    let img_inv = pixmap_to_rgba_image(rasterize_preview_panel(&state, &cfg_inv, w, h));
    let img_flip = pixmap_to_rgba_image(rasterize_preview_panel(&state, &cfg_flip, w, h));

    let mut strip = RgbaImage::from_pixel(w * 3 + 8, h, Rgba([210, 210, 215, 255]));
    for (idx, sub) in [&img_norm, &img_inv, &img_flip].into_iter().enumerate() {
        let x_off = (idx as u32) * (w + 4);
        for y in 0..h {
            for x in 0..w {
                strip.put_pixel(x_off + x, y, *sub.get_pixel(x, y));
            }
        }
    }
    strip
        .save(out_dir.join("frame_42_preview_panel_normal_inverted_flipped.png"))
        .expect("save frame_42");

    let full_win = pixmap_to_rgba_image(rasterize_full_window(&state, &theme));
    full_win
        .save(out_dir.join("frame_43_full_window_with_bottom_preview.png"))
        .expect("save frame_43");
}

fn point_in_triangle_2d(p: Pos2, a: Pos2, b: Pos2, c: Pos2) -> bool {
    let cross = |u: Pos2, v: Pos2, w: Pos2| (v.x - u.x) * (w.y - u.y) - (v.y - u.y) * (w.x - u.x);
    let d1 = cross(p, a, b);
    let d2 = cross(p, b, c);
    let d3 = cross(p, c, a);
    !((d1 < 0.0 || d2 < 0.0 || d3 < 0.0) && (d1 > 0.0 || d2 > 0.0 || d3 > 0.0))
}

#[test]
fn test_shape_thumbnail_concave_glyph_n_uses_lyon_nonzero_fill_without_convex_fan_corruption() {
    let theme = GrafiaTheme::sonoma_default();
    let mut state = GrafiaCanvasState::init_glyph_n();
    state.view_mode = CanvasViewMode::ShapeThumbnail;

    let ctx = egui::Context::default();
    let screen_rect = Rect::from_min_size(Pos2::ZERO, Vec2::new(800.0, 600.0));
    let mut full_output = ctx.run_ui(
        egui::RawInput {
            screen_rect: Some(screen_rect),
            ..Default::default()
        },
        |ui| {
            GrafiaCanvasWidget::new(&mut state, &theme)
                .with_size(Vec2::new(800.0, 600.0))
                .show(ui);
        },
    );
    full_output.textures_delta.clear();

    let has_lyon_mesh = full_output
        .shapes
        .iter()
        .any(|cs| matches!(&cs.shape, egui::Shape::Mesh(m) if !m.is_empty()));
    assert!(
        has_lyon_mesh,
        "ShapeThumbnail must emit a non-empty Lyon Mesh for closed concave contour /n"
    );

    let primitives = ctx.tessellate(full_output.shapes, full_output.pixels_per_point);
    let pad = theme.shape_canvas.header_height_dp * 2.0;
    let row_y = pad + theme.shape_canvas.header_height_dp;
    let mut scaled_cfg = theme.shape_canvas.clone();
    scaled_cfg.thumbnail_width_dp *= 4.0;
    scaled_cfg.thumbnail_height_dp *= 4.0;
    scaled_cfg.thumbnail_padding_dp *= 4.0;
    let proj = grafia::render::ShapeThumbnailProjection::compute(
        &state.paths[0],
        &scaled_cfg,
        state.contours[0].len(),
        true,
    );

    // Concave hollow under the arch of /n at font coordinate (280, 150) vs solid left stem at (115, 250)
    let hollow_pt = Pos2::new(
        pad + (proj.offset_x + 280.0 * proj.scale) as f32,
        row_y + (scaled_cfg.thumbnail_padding_dp as f64 + (510.0 - 150.0) * proj.scale) as f32,
    );
    let stem_pt = Pos2::new(
        pad + (proj.offset_x + 115.0 * proj.scale) as f32,
        row_y + (scaled_cfg.thumbnail_padding_dp as f64 + (510.0 - 250.0) * proj.scale) as f32,
    );
    let active_color = theme.shape_canvas.text_active_color32();

    let card_bg = theme.shape_canvas.accent_selection_color32();
    let mut hollow_top_color = egui::Color32::TRANSPARENT;
    let mut stem_top_color = egui::Color32::TRANSPARENT;
    for prim in &primitives {
        if let egui::epaint::Primitive::Mesh(mesh) = &prim.primitive {
            for &[i0, i1, i2] in mesh.indices.as_chunks::<3>().0 {
                let (v0, v1, v2) = (
                    &mesh.vertices[i0 as usize],
                    &mesh.vertices[i1 as usize],
                    &mesh.vertices[i2 as usize],
                );
                if point_in_triangle_2d(hollow_pt, v0.pos, v1.pos, v2.pos) {
                    hollow_top_color = v0.color;
                }
                if point_in_triangle_2d(stem_pt, v0.pos, v1.pos, v2.pos) {
                    stem_top_color = v0.color;
                }
            }
        }
    }
    assert_eq!(
        stem_top_color, active_color,
        "Left stem of /n at {stem_pt:?} must render in active silhouette color in ShapeThumbnail"
    );
    assert_eq!(
        hollow_top_color, card_bg,
        "Concave arch hollow of /n at {hollow_pt:?} must remain card background blue (not filled by convex fan)"
    );

    // Also verify double-inflection concave glyph /s suppresses convex-fan PathShape fill in ShapeThumbnail
    let mut state_s = GrafiaCanvasState::init_glyph_s();
    state_s.view_mode = CanvasViewMode::ShapeThumbnail;
    let mut out_s = ctx.run_ui(
        egui::RawInput {
            screen_rect: Some(screen_rect),
            ..Default::default()
        },
        |ui| {
            GrafiaCanvasWidget::new(&mut state_s, &theme)
                .with_size(Vec2::new(800.0, 600.0))
                .show(ui);
        },
    );
    out_s.textures_delta.clear();
    assert!(
        out_s
            .shapes
            .iter()
            .any(|cs| matches!(&cs.shape, egui::Shape::Mesh(m) if !m.is_empty())),
        "ShapeThumbnail must emit non-empty Lyon Mesh for concave /s"
    );
    for cs in &out_s.shapes {
        if let egui::Shape::Path(ps) = &cs.shape {
            assert_eq!(
                ps.fill,
                egui::Color32::TRANSPARENT,
                "Concave /s PathShape in ShapeThumbnail must have TRANSPARENT fill to prevent convex-fan corruption"
            );
        }
    }
}
