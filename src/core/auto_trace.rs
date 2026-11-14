//! `SPEC-009 §2.5`: Bitmap & Shadowed-Camera Auto-Trace on `CanvasBackgroundImage`.
//! Implements integral-image $O(1)$ Sauvola local adaptive thresholding, sub-pixel bilinear
//! Marching Squares, speckle rejection, and end-to-end `trace_background_image`.

use crate::core::auto_trace_fit::{
    build_traced_contours_upm, poly_signed_area, simplify_closed_loop_px,
};
use crate::core::auto_trace_homography::{extract_cropped_luma, rectify_perspective_luma};
use crate::core::canvas_entities::CanvasBackgroundImage;
use crate::core::host_requests::{AutoTraceConfig, AutoTraceThresholdMode};
use crate::core::math_consts::{
    ADAPTIVE_BIAS_MAX_F64, ADAPTIVE_BIAS_MIN_F64, ADAPTIVE_DARK_ANCHOR_RATIO,
    AUTO_TRACE_MAX_LUMINANCE, AUTO_TRACE_MIN_LUMINANCE, AUTO_TRACE_SAUVOLA_DYNAMIC_RANGE,
    BYTE_MAX_F64, MARCHING_SADDLE_QUARTER, MARCHING_SQUARES_ISO_EPS, MARCHING_SQUARES_QUANT_SCALE,
    TRACE_MIN_LOOP_VERTICES, TRACE_MIN_RDP_TOL_PX,
};
use crate::core::GlyphContour;
use std::collections::BTreeMap;

fn compute_adaptive_sauvola_field(
    luma: &[f64],
    w: usize,
    h: usize,
    window_radius_px: u16,
    bias_k: f32,
    invert_polarity: bool,
) -> Vec<f64> {
    let r = usize::from(window_radius_px).max(1);
    let k = f64::from(bias_k).clamp(ADAPTIVE_BIAS_MIN_F64, ADAPTIVE_BIAS_MAX_F64);
    let sat_w = w + 1;
    let mut sat_sum = vec![0.0_f64; sat_w * (h + 1)];
    let mut sat_sq = vec![0.0_f64; sat_w * (h + 1)];
    let mut min_lum = BYTE_MAX_F64;
    let mut max_lum = 0.0_f64;
    for y in 0..h {
        let mut row_sum = 0.0;
        let mut row_sq = 0.0;
        for x in 0..w {
            let val = luma[y * w + x];
            min_lum = min_lum.min(val);
            max_lum = max_lum.max(val);
            row_sum += val;
            row_sq += val * val;
            let idx = (y + 1) * sat_w + (x + 1);
            sat_sum[idx] = sat_sum[y * sat_w + (x + 1)] + row_sum;
            sat_sq[idx] = sat_sq[y * sat_w + (x + 1)] + row_sq;
        }
    }
    let global_mean = sat_sum[h * sat_w + w] / ((w * h) as f64);
    if (max_lum - min_lum).abs() <= MARCHING_SQUARES_ISO_EPS {
        return vec![-1.0_f64; w * h];
    }
    let dark_floor = min_lum + ADAPTIVE_DARK_ANCHOR_RATIO * (global_mean - min_lum);
    let bright_ceil = max_lum - ADAPTIVE_DARK_ANCHOR_RATIO * (max_lum - global_mean);

    let mut phi = Vec::with_capacity(w * h);
    for y in 0..h {
        let y0 = y.saturating_sub(r);
        let y1 = (y + r + 1).min(h);
        for x in 0..w {
            let x0 = x.saturating_sub(r);
            let x1 = (x + r + 1).min(w);
            let count = ((x1 - x0) * (y1 - y0)) as f64;
            let sum =
                sat_sum[y1 * sat_w + x1] - sat_sum[y0 * sat_w + x1] - sat_sum[y1 * sat_w + x0]
                    + sat_sum[y0 * sat_w + x0];
            let sq = sat_sq[y1 * sat_w + x1] - sat_sq[y0 * sat_w + x1] - sat_sq[y1 * sat_w + x0]
                + sat_sq[y0 * sat_w + x0];
            let mean = sum / count;
            let sigma = (sq / count - mean * mean).max(0.0).sqrt();
            let lum = luma[y * w + x];
            if invert_polarity {
                let sauvola = mean * (1.0 + k * (1.0 - sigma / AUTO_TRACE_SAUVOLA_DYNAMIC_RANGE));
                phi.push(lum - sauvola.min(bright_ceil));
            } else {
                let sauvola = mean * (1.0 - k * (1.0 - sigma / AUTO_TRACE_SAUVOLA_DYNAMIC_RANGE));
                phi.push(sauvola.max(dark_floor) - lum);
            }
        }
    }
    phi
}

fn compute_signed_ink_field(
    luma: &[f64],
    cw: u32,
    ch: u32,
    mode: AutoTraceThresholdMode,
    invert_polarity: bool,
) -> Vec<f64> {
    let w = cw as usize;
    let h = ch as usize;
    debug_assert_eq!(luma.len(), w * h);
    let mut min_lum = BYTE_MAX_F64;
    let mut max_lum = 0.0_f64;
    for &val in luma {
        min_lum = min_lum.min(val);
        max_lum = max_lum.max(val);
    }
    if (max_lum - min_lum).abs() <= MARCHING_SQUARES_ISO_EPS {
        return vec![-1.0_f64; w * h];
    }
    match mode {
        AutoTraceThresholdMode::GlobalLuminance(raw_t) => {
            let thresh = f64::from(raw_t.clamp(AUTO_TRACE_MIN_LUMINANCE, AUTO_TRACE_MAX_LUMINANCE));
            luma.iter()
                .map(|&lum| {
                    if invert_polarity {
                        lum - thresh
                    } else {
                        thresh - lum
                    }
                })
                .collect()
        }
        AutoTraceThresholdMode::AdaptiveLocal {
            window_radius_px,
            bias_k,
        } => compute_adaptive_sauvola_field(luma, w, h, window_radius_px, bias_k, invert_polarity),
    }
}

fn interp_iso(a: (f64, f64), b: (f64, f64), va: f64, vb: f64) -> (f64, f64) {
    let denom = va - vb;
    let t = if denom.abs() <= MARCHING_SQUARES_ISO_EPS {
        0.5
    } else {
        (va / denom).clamp(0.0, 1.0)
    };
    (a.0 + t * (b.0 - a.0), a.1 + t * (b.1 - a.1))
}

fn quant_key(pt: (f64, f64)) -> (i64, i64) {
    (
        (pt.0 * MARCHING_SQUARES_QUANT_SCALE).round() as i64,
        (pt.1 * MARCHING_SQUARES_QUANT_SCALE).round() as i64,
    )
}

type MarchingSegment = ((f64, f64), (f64, f64));

fn chain_marching_segments(segments: &[MarchingSegment]) -> Vec<Vec<(f64, f64)>> {
    let mut outgoing: BTreeMap<(i64, i64), Vec<usize>> = BTreeMap::new();
    for (idx, &(p0, _)) in segments.iter().enumerate() {
        outgoing.entry(quant_key(p0)).or_default().push(idx);
    }
    let mut visited = vec![false; segments.len()];
    let mut loops = Vec::new();
    for start_idx in 0..segments.len() {
        if visited[start_idx] {
            continue;
        }
        let mut poly = Vec::new();
        let mut cur_idx = start_idx;
        loop {
            visited[cur_idx] = true;
            let (p0, p1) = segments[cur_idx];
            poly.push(p0);
            let next_opt = outgoing
                .get(&quant_key(p1))
                .and_then(|list| list.iter().copied().find(|&cand| !visited[cand]));
            if let Some(next_idx) = next_opt {
                cur_idx = next_idx;
            } else {
                break;
            }
        }
        if poly.len() >= TRACE_MIN_LOOP_VERTICES {
            loops.push(poly);
        }
    }
    loops
}

fn extract_marching_squares_loops(phi: &[f64], cw: u32, ch: u32) -> Vec<Vec<(f64, f64)>> {
    let w = cw as usize;
    let h = ch as usize;
    debug_assert_eq!(phi.len(), w * h);
    let pw = w + 2;
    let ph = h + 2;
    let mut padded = vec![-1.0_f64; pw * ph];
    for y in 0..h {
        for x in 0..w {
            let mut v = phi[y * w + x];
            if v.abs() < MARCHING_SQUARES_ISO_EPS {
                v = -MARCHING_SQUARES_ISO_EPS;
            }
            padded[(y + 1) * pw + (x + 1)] = v;
        }
    }

    let mut segments: Vec<((f64, f64), (f64, f64))> = Vec::new();
    for gy in 0..(ph - 1) {
        let y0 = (gy as f64) - 0.5;
        let y1 = y0 + 1.0;
        for gx in 0..(pw - 1) {
            let x0 = (gx as f64) - 0.5;
            let x1 = x0 + 1.0;
            let v00 = padded[gy * pw + gx];
            let v10 = padded[gy * pw + gx + 1];
            let v11 = padded[(gy + 1) * pw + gx + 1];
            let v01 = padded[(gy + 1) * pw + gx];
            let mask = (u8::from(v00 > 0.0) << 3)
                | (u8::from(v10 > 0.0) << 2)
                | (u8::from(v11 > 0.0) << 1)
                | u8::from(v01 > 0.0);
            if mask == 0 || mask == 15 {
                continue;
            }
            let top = interp_iso((x0, y0), (x1, y0), v00, v10);
            let right = interp_iso((x1, y0), (x1, y1), v10, v11);
            let bottom = interp_iso((x0, y1), (x1, y1), v01, v11);
            let left = interp_iso((x0, y0), (x0, y1), v00, v01);
            let center_in = (v00 + v10 + v11 + v01) * MARCHING_SADDLE_QUARTER > 0.0;
            match mask {
                1 => segments.push((left, bottom)),
                2 => segments.push((bottom, right)),
                3 => segments.push((left, right)),
                4 => segments.push((right, top)),
                5 if center_in => {
                    segments.push((left, top));
                    segments.push((right, bottom));
                }
                5 => {
                    segments.push((left, bottom));
                    segments.push((right, top));
                }
                6 => segments.push((bottom, top)),
                7 => segments.push((left, top)),
                8 => segments.push((top, left)),
                9 => segments.push((top, bottom)),
                10 if center_in => {
                    segments.push((top, right));
                    segments.push((bottom, left));
                }
                10 => {
                    segments.push((top, left));
                    segments.push((bottom, right));
                }
                11 => segments.push((top, right)),
                12 => segments.push((right, left)),
                13 => segments.push((right, bottom)),
                14 => segments.push((bottom, left)),
                _ => {}
            }
        }
    }
    chain_marching_segments(&segments)
}

/// Traces `CanvasBackgroundImage` into closed foreground + counter `GlyphContour`s (`SPEC-009 §2.5.1`).
#[must_use]
pub fn trace_background_image(
    image: &CanvasBackgroundImage,
    config: &AutoTraceConfig,
) -> Vec<GlyphContour> {
    if !image.scale_x.is_finite()
        || !image.scale_y.is_finite()
        || image.scale_x.abs() == 0.0
        || image.scale_y.abs() == 0.0
        || !image.rotation_deg.is_finite()
    {
        return Vec::new();
    }
    let Some((cx, cy, cw, ch, mut luma)) = extract_cropped_luma(image, config.invert_polarity)
    else {
        return Vec::new();
    };
    if let Some(quad) = config.perspective_quad_px {
        if let Some(rectified) =
            rectify_perspective_luma(&luma, cx, cy, cw, ch, quad, config.invert_polarity)
        {
            luma = rectified;
        }
    }

    let phi =
        compute_signed_ink_field(&luma, cw, ch, config.threshold_mode, config.invert_polarity);
    let raw_loops = extract_marching_squares_loops(&phi, cw, ch);
    let min_area = if config.min_speckle_area_px2.is_finite() {
        config.min_speckle_area_px2.max(0.0)
    } else {
        0.0
    };
    let rdp_tol = if config.rdp_tolerance_px.is_finite() {
        config.rdp_tolerance_px.max(TRACE_MIN_RDP_TOL_PX)
    } else {
        TRACE_MIN_RDP_TOL_PX
    };

    let filtered_loops: Vec<Vec<(f64, f64)>> = raw_loops
        .into_iter()
        .filter(|lp| poly_signed_area(lp).abs() >= min_area)
        .map(|lp| simplify_closed_loop_px(&lp, rdp_tol))
        .filter(|lp| lp.len() >= TRACE_MIN_LOOP_VERTICES)
        .collect();

    build_traced_contours_upm(
        filtered_loops,
        ch,
        image.position,
        image.scale_x,
        image.scale_y,
        image.rotation_deg,
        config,
    )
}
