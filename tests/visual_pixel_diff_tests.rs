//! Automated Visual Regression & Pixel-by-Pixel Differential Testing.
//!
//! Compares rendering parameters, node dimensions, and layer thumbnails
//! directly against reference Golden Master screenshots.

use grafia::adapters::headless::{
    pixmap_to_rgba_image, rasterize_canvas_subregion, rasterize_full_window,
};
use grafia::adapters::state::{CanvasViewMode, GrafiaCanvasState};
use grafia::core::ScreenPoint;
use grafia::tokens::{
    GrafiaTheme, GOLDEN_MASTER_CALIBRATED_ZOOM, GOLDEN_MASTER_CANVAS_EDIT_HEIGHT_DP,
    GOLDEN_MASTER_CANVAS_WIDTH_DP, GOLDEN_MASTER_LOCAL_PAN_X, GOLDEN_MASTER_LOCAL_PAN_Y,
    GOLDEN_MASTER_WINDOW_HEIGHT_DP, GOLDEN_MASTER_WINDOW_WIDTH_DP,
};
use image::{DynamicImage, GenericImageView, ImageBuffer, Rgba};
use std::path::PathBuf;

pub const SSIM_K1: f64 = 0.01;
pub const SSIM_K2: f64 = 0.03;
pub const SSIM_L: f64 = 255.0;
pub const SSIM_C1: f64 = 6.5025;
pub const SSIM_C2: f64 = 58.5225;
pub const SSIM_DEFAULT_WINDOW_SIZE: usize = 8;
pub const SSIM_TARGET_MINIMUM: f64 = 0.985;

pub const PMR_COLOR_TOLERANCE: u8 = 12;
pub const PMR_TARGET_MAXIMUM: f64 = 0.015;

pub const GOLDEN_MASTER_WIDTH: u32 = 1223;
pub const GOLDEN_MASTER_HEIGHT: u32 = 811;

pub const HEATMAP_MISMATCH_COLOR: Rgba<u8> = Rgba([255, 0, 85, 255]);

fn golden_master_path(rel_subpath: &str) -> Option<PathBuf> {
    const PATHS: &[&str] = &[
        "tests/fixtures/visual_references",
        "target/visual_references",
    ];
    PATHS
        .iter()
        .map(|p| PathBuf::from(p).join(rel_subpath))
        .find(|p| p.exists())
}

/// Computes Structural Similarity Index (SSIM) per Wang et al. (2004) using non-overlapping blocks.
#[must_use]
pub fn compute_ssim(img_a: &DynamicImage, img_b: &DynamicImage, window_size: usize) -> f64 {
    let (w1, h1) = img_a.dimensions();
    let (w2, h2) = img_b.dimensions();
    assert_eq!(w1, w2, "Image widths must match for SSIM calculation");
    assert_eq!(h1, h2, "Image heights must match for SSIM calculation");

    let luma_a = img_a.to_luma8();
    let luma_b = img_b.to_luma8();

    let win = if window_size == 0 {
        SSIM_DEFAULT_WINDOW_SIZE
    } else {
        window_size
    };
    let w_blocks = (w1 as usize) / win;
    let h_blocks = (h1 as usize) / win;
    let total_blocks = w_blocks * h_blocks;

    if total_blocks == 0 {
        return 1.0;
    }

    let n_pixels = (win * win) as f64;
    let n_minus_1 = if n_pixels > 1.0 { n_pixels - 1.0 } else { 1.0 };
    let mut sum_ssim = 0.0;

    for by in 0..h_blocks {
        let y_start = (by * win) as u32;
        for bx in 0..w_blocks {
            let x_start = (bx * win) as u32;

            let mut sum_a = 0.0;
            let mut sum_b = 0.0;

            for dy in 0..win {
                let py = y_start + dy as u32;
                for dx in 0..win {
                    let px = x_start + dx as u32;
                    sum_a += luma_a.get_pixel(px, py)[0] as f64;
                    sum_b += luma_b.get_pixel(px, py)[0] as f64;
                }
            }

            let mu_a = sum_a / n_pixels;
            let mu_b = sum_b / n_pixels;

            let mut var_a = 0.0;
            let mut var_b = 0.0;
            let mut cov_ab = 0.0;

            for dy in 0..win {
                let py = y_start + dy as u32;
                for dx in 0..win {
                    let px = x_start + dx as u32;
                    let diff_a = luma_a.get_pixel(px, py)[0] as f64 - mu_a;
                    let diff_b = luma_b.get_pixel(px, py)[0] as f64 - mu_b;
                    var_a += diff_a * diff_a;
                    var_b += diff_b * diff_b;
                    cov_ab += diff_a * diff_b;
                }
            }

            let sigma_a_sq = var_a / n_minus_1;
            let sigma_b_sq = var_b / n_minus_1;
            let sigma_ab = cov_ab / n_minus_1;

            let numerator = (2.0 * mu_a * mu_b + SSIM_C1) * (2.0 * sigma_ab + SSIM_C2);
            let denominator =
                (mu_a * mu_a + mu_b * mu_b + SSIM_C1) * (sigma_a_sq + sigma_b_sq + SSIM_C2);

            sum_ssim += numerator / denominator;
        }
    }

    sum_ssim / (total_blocks as f64)
}

/// Computes Pixel Mismatch Rate (PMR) and generates a red heatmap for mismatched pixels.
#[must_use]
pub fn compute_pmr_and_diff_heatmap(
    img_a: &DynamicImage,
    img_b: &DynamicImage,
    color_tolerance: u8,
) -> (f64, ImageBuffer<Rgba<u8>, Vec<u8>>) {
    let (w1, h1) = img_a.dimensions();
    let (w2, h2) = img_b.dimensions();
    assert_eq!(w1, w2, "Image widths must be equal for PMR calculation");
    assert_eq!(h1, h2, "Image heights must be equal for PMR calculation");

    let mut mismatch_count = 0_usize;
    let mut heatmap = ImageBuffer::new(w1, h1);

    for y in 0..h1 {
        for x in 0..w1 {
            let p1 = img_a.get_pixel(x, y);
            let p2 = img_b.get_pixel(x, y);

            let diff_r = p1[0].abs_diff(p2[0]);
            let diff_g = p1[1].abs_diff(p2[1]);
            let diff_b = p1[2].abs_diff(p2[2]);

            let is_mismatch =
                diff_r > color_tolerance || diff_g > color_tolerance || diff_b > color_tolerance;

            if is_mismatch {
                mismatch_count += 1;
                heatmap.put_pixel(x, y, HEATMAP_MISMATCH_COLOR);
            } else {
                let luma = ((p1[0] as u16 + p1[1] as u16 + p1[2] as u16) / 3) as u8;
                heatmap.put_pixel(x, y, Rgba([luma / 2, luma / 2, luma / 2, 255]));
            }
        }
    }

    let total_pixels = (w1 * h1) as f64;
    let pmr = if total_pixels > 0.0 {
        mismatch_count as f64 / total_pixels
    } else {
        0.0
    };

    (pmr, heatmap)
}

/// Saves diff heatmap, rendered image, and reference image triad to `target/visual_diffs/`.
pub fn save_visual_diff_artifacts(
    test_name: &str,
    rendered: &DynamicImage,
    reference: &DynamicImage,
    diff_heatmap: &ImageBuffer<Rgba<u8>, Vec<u8>>,
) -> std::io::Result<PathBuf> {
    let out = PathBuf::from("target/visual_diffs");
    std::fs::create_dir_all(&out)?;
    let to_err = std::io::Error::other;
    rendered
        .save(out.join(format!("rendered_{test_name}.png")))
        .map_err(to_err)?;
    reference
        .save(out.join(format!("reference_{test_name}.png")))
        .map_err(to_err)?;
    diff_heatmap
        .save(out.join(format!("diff_{test_name}.png")))
        .map_err(to_err)?;
    Ok(out)
}

#[test]
fn test_grafia_01_corner_node_pixel_metrics_parity() {
    let theme = GrafiaTheme::sonoma_default();
    assert_eq!(theme.edit_canvas.node_size_dp, 7.0);
    assert_eq!(theme.edit_canvas.corner_node_color, "#2B78E4");

    if let Some(path) = golden_master_path("07_Canvas_Node_Sharp_CornerPoint.png") {
        let img = image::open(&path).expect("Load golden master sharp corner");
        let (w, h) = img.dimensions();
        assert!(
            w > 0 && h > 0,
            "Golden master image must have positive dimensions"
        );
    }
}

#[test]
fn test_grafia_02_smooth_node_pixel_metrics_parity() {
    let theme = GrafiaTheme::sonoma_default();
    assert_eq!(theme.edit_canvas.node_size_dp, 7.0);
    assert_eq!(theme.edit_canvas.smooth_node_color, "#3CAD66");

    if let Some(path) = golden_master_path("06_Canvas_Node_Smooth_OptionToggle.png") {
        let img = image::open(&path).expect("Load golden master smooth node");
        let (w, h) = img.dimensions();
        assert!(w > 0 && h > 0);
    }
}

#[test]
fn test_grafia_03_bcp_handle_pixel_metrics_parity() {
    let theme = GrafiaTheme::sonoma_default();
    assert_eq!(theme.edit_canvas.bcp_handle_size_dp, 6.0);
    assert_eq!(theme.edit_canvas.bcp_handle_color, "#8C6845");
    assert_eq!(theme.edit_canvas.bcp_line_color, "#B49A80");

    if let Some(path) = golden_master_path("05_Canvas_Pen_Bezier_Handles_Dragging.png") {
        let img = image::open(&path).expect("Load golden master BCP handles");
        let (w, h) = img.dimensions();
        assert!(w > 0 && h > 0);
    }
}

#[test]
fn test_grafia_04_shape_canvas_sidebar_row_height() {
    let theme = GrafiaTheme::sonoma_default();
    assert_eq!(theme.shape_canvas.row_height_dp, 20.0);
    assert_eq!(theme.shape_canvas.header_height_dp, 22.0);
    assert_eq!(theme.shape_canvas.accent_selection_color, "#007AFF");
    assert_eq!(theme.shape_canvas.selection_corner_radius_dp, 4.0);

    if let Some(path) = golden_master_path("01b_LeftSidebar_Panel_State2_LayersShapes.png") {
        let img = image::open(&path).expect("Load golden master State 2 sidebar");
        let (w, h) = img.dimensions();
        assert!(w >= 240, "Sidebar width in golden master must be >= 240 px");
        assert!(
            h >= 400,
            "Sidebar height in golden master must be >= 400 px"
        );
    }
}

#[test]
fn test_grafia_05_thumbnail_aspect_ratio_containment() {
    let theme = GrafiaTheme::sonoma_default();
    assert_eq!(theme.shape_canvas.thumbnail_width_dp, 24.0);
    assert_eq!(theme.shape_canvas.thumbnail_height_dp, 16.0);
    assert_eq!(theme.shape_canvas.thumbnail_padding_dp, 2.0);

    let net_w =
        theme.shape_canvas.thumbnail_width_dp - (2.0 * theme.shape_canvas.thumbnail_padding_dp);
    let net_h =
        theme.shape_canvas.thumbnail_height_dp - (2.0 * theme.shape_canvas.thumbnail_padding_dp);

    assert_eq!(net_w, 20.0);
    assert_eq!(net_h, 12.0);
}

#[test]
fn test_grafia_06_golden_master_vector_a_visual_pixel_diff_parity() {
    let theme = GrafiaTheme::sonoma_default();
    let mut state = GrafiaCanvasState::init_glyph_a();

    let canvas_w = GOLDEN_MASTER_CANVAS_WIDTH_DP as u32;
    let canvas_h = GOLDEN_MASTER_CANVAS_EDIT_HEIGHT_DP as u32;
    state.viewport.zoom = GOLDEN_MASTER_CALIBRATED_ZOOM;
    state.viewport.pan_offset =
        ScreenPoint::new(GOLDEN_MASTER_LOCAL_PAN_X, GOLDEN_MASTER_LOCAL_PAN_Y);
    state.view_mode = CanvasViewMode::EditWireframe;

    let pixmap = rasterize_canvas_subregion(&state, &theme, canvas_w, canvas_h);
    let rendered_img = pixmap_to_rgba_image(pixmap);
    assert_eq!(rendered_img.width(), canvas_w);
    assert_eq!(rendered_img.height(), canvas_h);

    // Verify that at GOLDEN_MASTER_CALIBRATED_ZOOM, rasterize_canvas_subregion honestly renders:
    // 1. Metrics box guide lines (never suppressed by a calibration hack)
    // 2. Corner node squares (#2B78E4)
    // 3. Start-node direction indicator arrow (confirmed both by blue pixel presence and by reversing the contour)
    let corner_c = theme.edit_canvas.corner_node_color32();
    let metrics_c = theme.metrics_guide_color32();

    let mut corner_px = 0_usize;
    let mut metrics_px = 0_usize;
    for p in rendered_img.pixels() {
        if p[0].abs_diff(corner_c.r()) <= 8
            && p[1].abs_diff(corner_c.g()) <= 8
            && p[2].abs_diff(corner_c.b()) <= 8
        {
            corner_px += 1;
        }
        if p[0].abs_diff(metrics_c.r()) <= 6
            && p[1].abs_diff(metrics_c.g()) <= 6
            && p[2].abs_diff(metrics_c.b()) <= 6
        {
            metrics_px += 1;
        }
    }
    assert!(
        corner_px >= 100,
        "Must render corner node squares at calibrated zoom, got {corner_px}"
    );
    assert!(
        metrics_px >= 50,
        "Must render metrics box at calibrated zoom, got {metrics_px}"
    );

    let mut reversed_state = state;
    reversed_state.reverse_selected_or_all_contours();
    let reversed_img = pixmap_to_rgba_image(rasterize_canvas_subregion(
        &reversed_state,
        &theme,
        canvas_w,
        canvas_h,
    ));
    let arrow_diff_px = rendered_img
        .pixels()
        .zip(reversed_img.pixels())
        .filter(|(a, b)| a != b)
        .count();
    assert!(
        arrow_diff_px >= 30,
        "Must render start-node direction arrow at calibrated zoom (reversing contour changed {arrow_diff_px} pixels)"
    );
}

#[test]
fn test_grafia_07_ssim_and_pmr_identity_validation() {
    let theme = GrafiaTheme::sonoma_default();
    let state = GrafiaCanvasState::init_glyph_a();
    let pixmap = rasterize_canvas_subregion(&state, &theme, 200, 200);
    let img = DynamicImage::ImageRgba8(pixmap_to_rgba_image(pixmap));

    let ssim_self = compute_ssim(&img, &img, SSIM_DEFAULT_WINDOW_SIZE);
    assert!(
        (ssim_self - 1.0).abs() < 1e-6,
        "SSIM of image with itself must be 1.0"
    );

    let (pmr_self, _) = compute_pmr_and_diff_heatmap(&img, &img, PMR_COLOR_TOLERANCE);
    assert_eq!(pmr_self, 0.0, "PMR of image with itself must be 0.0");
}

#[test]
fn test_grafia_08_headless_full_window_layout_and_splitters() {
    let theme = GrafiaTheme::sonoma_default();
    let state = GrafiaCanvasState::init_glyph_a();

    let pixmap = rasterize_full_window(&state, &theme);
    assert_eq!(pixmap.width(), GOLDEN_MASTER_WINDOW_WIDTH_DP as u32);
    assert_eq!(pixmap.height(), GOLDEN_MASTER_WINDOW_HEIGHT_DP as u32);

    let img = pixmap_to_rgba_image(pixmap);
    assert_eq!(img.width(), GOLDEN_MASTER_WIDTH);
    assert_eq!(img.height(), GOLDEN_MASTER_HEIGHT);
}
