//! Adversarial empirical test suite for SSIM and PMR differential analysis (Milestone 4).
//!
//! Normative references:
//! - STD-ARCH: Industry Software Engineering & Testing Standards
//! - Wang et al. (2004): Image Quality Assessment: From Error Visibility to Structural Similarity
//! - M4 Mandate: SSIM & PMR Adversarial Verifier
//!
//! Empirical Challenge Dimensions:
//! 1. Identity invariance: identical images produce SSIM == 1.0 (within 1e-9) and PMR == 0.0%.
//! 2. Inverted/orthogonal sensitivity: pure black vs pure white produces SSIM ≈ 0.0 and PMR == 100.0%.
//! 3. Numerical stability: uniform constant images (all 0, all 255, all 128) do not panic or produce NaN/Inf.
//! 4. Mismatch tolerance boundary: channel diff exactly 12 (no mismatch) vs 13 (mismatch).
//! 5. Dimension mismatch: non-matching widths or heights cleanly panic without silent truncation.
//! 6. Block/window sizing edge cases: window_size = 0, 1, larger than image, non-divisible dimensions.

use image::{DynamicImage, ImageBuffer, Rgba};
use std::panic::catch_unwind;

#[path = "visual_pixel_diff_tests.rs"]
#[allow(dead_code, unused_imports)]
mod visual_pixel_diff_tests;

use visual_pixel_diff_tests::{
    compute_pmr_and_diff_heatmap, compute_ssim, HEATMAP_MISMATCH_COLOR, PMR_COLOR_TOLERANCE,
    SSIM_DEFAULT_WINDOW_SIZE,
};

fn create_solid_image(width: u32, height: u32, color: Rgba<u8>) -> DynamicImage {
    let mut buf = ImageBuffer::new(width, height);
    for pixel in buf.pixels_mut() {
        *pixel = color;
    }
    DynamicImage::ImageRgba8(buf)
}

fn create_gradient_image(width: u32, height: u32) -> DynamicImage {
    let mut buf = ImageBuffer::new(width, height);
    for (x, y, pixel) in buf.enumerate_pixels_mut() {
        let r = ((x * 255) / width.max(1)) as u8;
        let g = ((y * 255) / height.max(1)) as u8;
        let b = (((x + y) * 128) / (width + height).max(1)) as u8;
        *pixel = Rgba([r, g, b, 255]);
    }
    DynamicImage::ImageRgba8(buf)
}

// ============================================================================
// 1. Identity Invariance & Self-Comparison Precision
// ============================================================================

#[test]
fn test_adversarial_ssim_and_pmr_identity_invariance_complex_gradient() {
    let img = create_gradient_image(128, 128);

    let ssim = compute_ssim(&img, &img, SSIM_DEFAULT_WINDOW_SIZE);
    assert!(
        (ssim - 1.0).abs() < 1e-9,
        "Identity SSIM must equal 1.0 within 1e-9, got {ssim:.12}"
    );

    let (pmr, heatmap) = compute_pmr_and_diff_heatmap(&img, &img, PMR_COLOR_TOLERANCE);
    assert_eq!(pmr, 0.0, "Identity PMR must be exactly 0.0%");

    // Ensure no mismatch pixels in heatmap
    for pixel in heatmap.pixels() {
        assert_ne!(
            *pixel, HEATMAP_MISMATCH_COLOR,
            "Identity heatmap must not contain mismatch color"
        );
    }
}

#[test]
fn test_adversarial_ssim_and_pmr_identity_invariance_constant_levels() {
    for level in [0, 1, 64, 128, 192, 254, 255] {
        let img = create_solid_image(64, 64, Rgba([level, level, level, 255]));

        let ssim = compute_ssim(&img, &img, SSIM_DEFAULT_WINDOW_SIZE);
        assert!(
            (ssim - 1.0).abs() < 1e-9,
            "Identity SSIM for constant level {level} must equal 1.0 within 1e-9, got {ssim:.12}"
        );

        let (pmr, _) = compute_pmr_and_diff_heatmap(&img, &img, PMR_COLOR_TOLERANCE);
        assert_eq!(
            pmr, 0.0,
            "Identity PMR for constant level {level} must be 0.0%"
        );
    }
}

// ============================================================================
// 2. Inverted / Orthogonal Image Sensitivity
// ============================================================================

#[test]
fn test_adversarial_inverted_black_vs_white_sensitivity() {
    let black = create_solid_image(64, 64, Rgba([0, 0, 0, 255]));
    let white = create_solid_image(64, 64, Rgba([255, 255, 255, 255]));

    let ssim = compute_ssim(&black, &white, SSIM_DEFAULT_WINDOW_SIZE);
    // Analytical expectation: C1*C2 / ((255^2 + C1)*C2) = 6.5025 / 65031.5025 ≈ 0.00009999
    assert!(
        (0.0..0.001).contains(&ssim),
        "SSIM between pure black and pure white must be ≈ 0.0 (< 0.001), got {ssim:.6}"
    );

    let (pmr, heatmap) = compute_pmr_and_diff_heatmap(&black, &white, PMR_COLOR_TOLERANCE);
    assert_eq!(
        pmr, 1.0,
        "PMR between pure black and pure white must be exactly 100.0%"
    );

    for pixel in heatmap.pixels() {
        assert_eq!(
            *pixel, HEATMAP_MISMATCH_COLOR,
            "Every pixel in 100% mismatch heatmap must be HEATMAP_MISMATCH_COLOR"
        );
    }
}

#[test]
fn test_adversarial_checkerboard_inversion_sensitivity() {
    let w = 64;
    let h = 64;
    let mut buf_a = ImageBuffer::new(w, h);
    let mut buf_b = ImageBuffer::new(w, h);

    for y in 0..h {
        for x in 0..w {
            let is_even = ((x / 8) + (y / 8)) % 2 == 0;
            if is_even {
                buf_a.put_pixel(x, y, Rgba([0, 0, 0, 255]));
                buf_b.put_pixel(x, y, Rgba([255, 255, 255, 255]));
            } else {
                buf_a.put_pixel(x, y, Rgba([255, 255, 255, 255]));
                buf_b.put_pixel(x, y, Rgba([0, 0, 0, 255]));
            }
        }
    }
    let img_a = DynamicImage::ImageRgba8(buf_a);
    let img_b = DynamicImage::ImageRgba8(buf_b);

    let (pmr, _) = compute_pmr_and_diff_heatmap(&img_a, &img_b, PMR_COLOR_TOLERANCE);
    assert_eq!(pmr, 1.0, "Inverted checkerboard PMR must be 100.0%");

    let ssim = compute_ssim(&img_a, &img_b, SSIM_DEFAULT_WINDOW_SIZE);
    assert!(
        ssim < 0.1,
        "Inverted checkerboard SSIM must be < 0.1, got {ssim:.4}"
    );
}

// ============================================================================
// 3. Numerical Stability (Zero Variance & Block Boundaries)
// ============================================================================

#[test]
fn test_adversarial_numerical_stability_zero_variance() {
    let img_0 = create_solid_image(32, 32, Rgba([0, 0, 0, 255]));
    let img_1 = create_solid_image(32, 32, Rgba([1, 1, 1, 255]));
    let img_128 = create_solid_image(32, 32, Rgba([128, 128, 128, 255]));
    let img_255 = create_solid_image(32, 32, Rgba([255, 255, 255, 255]));

    for (name, img) in [
        ("all-0", &img_0),
        ("all-1", &img_1),
        ("all-128", &img_128),
        ("all-255", &img_255),
    ] {
        let ssim = compute_ssim(img, img, 8);
        assert!(!ssim.is_nan(), "{name} SSIM must not be NaN");
        assert!(!ssim.is_infinite(), "{name} SSIM must not be infinite");
        assert!(ssim.is_finite(), "{name} SSIM must be finite");
        assert!((ssim - 1.0).abs() < 1e-9, "{name} self SSIM must equal 1.0");
    }

    // Between distinct uniform images (both having zero variance)
    let ssim_0_1 = compute_ssim(&img_0, &img_1, 8);
    assert!(ssim_0_1.is_finite(), "SSIM(0, 1) must be finite");
    assert!(!ssim_0_1.is_nan(), "SSIM(0, 1) must not be NaN");
    assert!(ssim_0_1 > 0.8 && ssim_0_1 < 1.0);

    let ssim_128_255 = compute_ssim(&img_128, &img_255, 8);
    assert!(ssim_128_255.is_finite(), "SSIM(128, 255) must be finite");
    assert!(!ssim_128_255.is_nan(), "SSIM(128, 255) must not be NaN");
}

#[test]
fn test_adversarial_numerical_stability_window_sizes_and_subblocks() {
    let img = create_gradient_image(37, 43); // Non-multiples of 8

    // window_size = 0 (defaults to 8)
    let ssim_def = compute_ssim(&img, &img, 0);
    assert!((ssim_def - 1.0).abs() < 1e-9);

    // window_size = 1 (1x1 window, n_pixels = 1, test n-1 denominator protection)
    let ssim_win1 = compute_ssim(&img, &img, 1);
    assert!((ssim_win1 - 1.0).abs() < 1e-9);

    // window_size = 64 on 37x43 image (w_blocks = 0, h_blocks = 0, total_blocks = 0)
    let ssim_win64 = compute_ssim(&img, &img, 64);
    assert_eq!(
        ssim_win64, 1.0,
        "SSIM with window larger than image must return 1.0"
    );

    // Degenerate 0x0 image
    let empty_img = DynamicImage::ImageRgba8(ImageBuffer::new(0, 0));
    let ssim_empty = compute_ssim(&empty_img, &empty_img, 8);
    assert_eq!(ssim_empty, 1.0);

    let (pmr_empty, _) = compute_pmr_and_diff_heatmap(&empty_img, &empty_img, 12);
    assert_eq!(pmr_empty, 0.0);
}

// ============================================================================
// 4. Mismatch Tolerance Boundary (Diff 12 vs 13)
// ============================================================================

#[test]
fn test_adversarial_pmr_tolerance_boundary_exact_12_vs_13() {
    let w = 10;
    let h = 10;
    let base = create_solid_image(w, h, Rgba([100, 100, 100, 255]));

    // 1. Delta exactly 12 on Red channel: 112 vs 100 -> NOT mismatch
    let delta_r_12 = create_solid_image(w, h, Rgba([112, 100, 100, 255]));
    let (pmr_r_12, _) = compute_pmr_and_diff_heatmap(&base, &delta_r_12, 12);
    assert_eq!(
        pmr_r_12, 0.0,
        "Delta 12 on R channel must NOT be a mismatch"
    );

    // 2. Delta exactly 12 on Green channel: 100 vs 112 -> NOT mismatch
    let delta_g_12 = create_solid_image(w, h, Rgba([100, 112, 100, 255]));
    let (pmr_g_12, _) = compute_pmr_and_diff_heatmap(&base, &delta_g_12, 12);
    assert_eq!(
        pmr_g_12, 0.0,
        "Delta 12 on G channel must NOT be a mismatch"
    );

    // 3. Delta exactly 12 on Blue channel: 100 vs 112 -> NOT mismatch
    let delta_b_12 = create_solid_image(w, h, Rgba([100, 100, 112, 255]));
    let (pmr_b_12, _) = compute_pmr_and_diff_heatmap(&base, &delta_b_12, 12);
    assert_eq!(
        pmr_b_12, 0.0,
        "Delta 12 on B channel must NOT be a mismatch"
    );

    // 4. Delta exactly 12 on ALL channels (112, 112, 112 vs 100, 100, 100) -> NOT mismatch
    let delta_all_12 = create_solid_image(w, h, Rgba([112, 112, 112, 255]));
    let (pmr_all_12, _) = compute_pmr_and_diff_heatmap(&base, &delta_all_12, 12);
    assert_eq!(
        pmr_all_12, 0.0,
        "Delta 12 on all channels must NOT be a mismatch"
    );

    // 5. Negative delta exactly 12 (88, 88, 88 vs 100, 100, 100) -> NOT mismatch
    let delta_neg_12 = create_solid_image(w, h, Rgba([88, 88, 88, 255]));
    let (pmr_neg_12, _) = compute_pmr_and_diff_heatmap(&base, &delta_neg_12, 12);
    assert_eq!(pmr_neg_12, 0.0, "Negative delta 12 must NOT be a mismatch");

    // 6. Delta exactly 13 on Red channel: 113 vs 100 -> MISMATCH!
    let delta_r_13 = create_solid_image(w, h, Rgba([113, 100, 100, 255]));
    let (pmr_r_13, heatmap_r_13) = compute_pmr_and_diff_heatmap(&base, &delta_r_13, 12);
    assert_eq!(pmr_r_13, 1.0, "Delta 13 on R channel MUST be a mismatch");
    assert_eq!(heatmap_r_13.get_pixel(0, 0), &HEATMAP_MISMATCH_COLOR);

    // 7. Delta exactly 13 on Green channel: 100 vs 113 -> MISMATCH!
    let delta_g_13 = create_solid_image(w, h, Rgba([100, 113, 100, 255]));
    let (pmr_g_13, _) = compute_pmr_and_diff_heatmap(&base, &delta_g_13, 12);
    assert_eq!(pmr_g_13, 1.0, "Delta 13 on G channel MUST be a mismatch");

    // 8. Delta exactly 13 on Blue channel: 100 vs 113 -> MISMATCH!
    let delta_b_13 = create_solid_image(w, h, Rgba([100, 100, 113, 255]));
    let (pmr_b_13, _) = compute_pmr_and_diff_heatmap(&base, &delta_b_13, 12);
    assert_eq!(pmr_b_13, 1.0, "Delta 13 on B channel MUST be a mismatch");

    // 9. Negative delta exactly 13 (87 vs 100) -> MISMATCH!
    let delta_neg_13 = create_solid_image(w, h, Rgba([87, 100, 100, 255]));
    let (pmr_neg_13, _) = compute_pmr_and_diff_heatmap(&base, &delta_neg_13, 12);
    assert_eq!(pmr_neg_13, 1.0, "Negative delta 13 MUST be a mismatch");

    // 10. Single pixel perturbation in 10x10 (100 pixels) grid:
    // Exactly 1 pixel at (5, 5) has diff 13, all other 99 pixels have diff 0
    let mut single_mut_buf = ImageBuffer::new(w, h);
    for pixel in single_mut_buf.pixels_mut() {
        *pixel = Rgba([100, 100, 100, 255]);
    }
    single_mut_buf.put_pixel(5, 5, Rgba([113, 100, 100, 255]));
    let single_mut_img = DynamicImage::ImageRgba8(single_mut_buf);

    let (pmr_single, heatmap_single) = compute_pmr_and_diff_heatmap(&base, &single_mut_img, 12);
    assert_eq!(
        pmr_single, 0.01,
        "1 mismatch in 100 pixels must yield PMR exactly 0.01 (1.0%)"
    );
    assert_eq!(
        heatmap_single.get_pixel(5, 5),
        &HEATMAP_MISMATCH_COLOR,
        "Pixel (5, 5) must be highlighted in heatmap"
    );
    assert_ne!(
        heatmap_single.get_pixel(0, 0),
        &HEATMAP_MISMATCH_COLOR,
        "Pixel (0, 0) must not be highlighted in heatmap"
    );
}

// ============================================================================
// 5. Image Dimension Mismatch Enforcement
// ============================================================================

#[test]
fn test_adversarial_dimension_mismatch_panics_cleanly() {
    let img_100x100 = create_solid_image(100, 100, Rgba([50, 50, 50, 255]));
    let img_101x100 = create_solid_image(101, 100, Rgba([50, 50, 50, 255]));
    let img_100x101 = create_solid_image(100, 101, Rgba([50, 50, 50, 255]));

    // 1. SSIM width mismatch panics cleanly
    let ssim_w_panic = catch_unwind(|| {
        let _ = compute_ssim(&img_100x100, &img_101x100, 8);
    });
    assert!(
        ssim_w_panic.is_err(),
        "compute_ssim must panic on width mismatch"
    );

    // 2. SSIM height mismatch panics cleanly
    let ssim_h_panic = catch_unwind(|| {
        let _ = compute_ssim(&img_100x100, &img_100x101, 8);
    });
    assert!(
        ssim_h_panic.is_err(),
        "compute_ssim must panic on height mismatch"
    );

    // 3. PMR width mismatch panics cleanly
    let pmr_w_panic = catch_unwind(|| {
        let _ = compute_pmr_and_diff_heatmap(&img_100x100, &img_101x100, 12);
    });
    assert!(
        pmr_w_panic.is_err(),
        "compute_pmr_and_diff_heatmap must panic on width mismatch"
    );

    // 4. PMR height mismatch panics cleanly
    let pmr_h_panic = catch_unwind(|| {
        let _ = compute_pmr_and_diff_heatmap(&img_100x100, &img_100x101, 12);
    });
    assert!(
        pmr_h_panic.is_err(),
        "compute_pmr_and_diff_heatmap must panic on height mismatch"
    );
}
