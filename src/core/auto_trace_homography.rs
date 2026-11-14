//! Crop luminance extraction and $3 \times 3$ planar homography perspective rectification
//! for `SPEC-009 §2.5` Bitmap & Camera Auto-Trace (`< 399` eSLOC).

use crate::core::canvas_entities::CanvasBackgroundImage;
use crate::core::math_consts::{
    BYTE_MAX_F64, HOMOGRAPHY_DENOM_EPS, HOMOGRAPHY_SYSTEM_DIM, LUMA_BLUE_COEFF, LUMA_GREEN_COEFF,
    LUMA_RED_COEFF,
};
use crate::core::spline_support::solve_linear_system;

pub(crate) fn extract_cropped_luma(
    img: &CanvasBackgroundImage,
    invert_polarity: bool,
) -> Option<(u32, u32, u32, u32, Vec<f64>)> {
    if img.width_px < 2 || img.height_px < 2 {
        return None;
    }
    let expected = (img.width_px as usize)
        .saturating_mul(img.height_px as usize)
        .saturating_mul(4);
    if img.rgba_pixels.len() < expected {
        return None;
    }
    let [rx, ry, rw, rh] = img
        .crop_rect_px
        .unwrap_or([0, 0, img.width_px, img.height_px]);
    let cx = rx.min(img.width_px.saturating_sub(1));
    let cy = ry.min(img.height_px.saturating_sub(1));
    let cw = rw.min(img.width_px.saturating_sub(cx));
    let ch = rh.min(img.height_px.saturating_sub(cy));
    if cw < 2 || ch < 2 {
        return None;
    }
    let bg_lum = if invert_polarity { 0.0 } else { BYTE_MAX_F64 };
    let mut luma = Vec::with_capacity((cw as usize) * (ch as usize));
    for row in cy..(cy + ch) {
        for col in cx..(cx + cw) {
            let base = ((row * img.width_px + col) as usize) * 4;
            let r = f64::from(img.rgba_pixels[base]);
            let g = f64::from(img.rgba_pixels[base + 1]);
            let b = f64::from(img.rgba_pixels[base + 2]);
            let a = f64::from(img.rgba_pixels[base + 3]) / BYTE_MAX_F64;
            let px_lum = LUMA_RED_COEFF * r + LUMA_GREEN_COEFF * g + LUMA_BLUE_COEFF * b;
            luma.push(px_lum * a + bg_lum * (1.0 - a));
        }
    }
    Some((cx, cy, cw, ch, luma))
}

fn compute_planar_homography(src: [(f64, f64); 4], dst: [(f64, f64); 4]) -> Option<[f64; 9]> {
    let mut a = vec![vec![0.0_f64; HOMOGRAPHY_SYSTEM_DIM]; HOMOGRAPHY_SYSTEM_DIM];
    let mut b = vec![0.0_f64; HOMOGRAPHY_SYSTEM_DIM];
    for i in 0..4 {
        let (x, y) = src[i];
        let (u, v) = dst[i];
        if !x.is_finite() || !y.is_finite() || !u.is_finite() || !v.is_finite() {
            return None;
        }
        a[i * 2] = vec![x, y, 1.0, 0.0, 0.0, 0.0, -u * x, -u * y];
        b[i * 2] = u;
        a[i * 2 + 1] = vec![0.0, 0.0, 0.0, x, y, 1.0, -v * x, -v * y];
        b[i * 2 + 1] = v;
    }
    let sol = solve_linear_system(&mut a, &mut b)?;
    Some([
        sol[0], sol[1], sol[2], sol[3], sol[4], sol[5], sol[6], sol[7], 1.0,
    ])
}

fn sample_bilinear(luma: &[f64], cw: u32, ch: u32, fx: f64, fy: f64, fallback: f64) -> f64 {
    let w_f = f64::from(cw);
    let h_f = f64::from(ch);
    if fx < 0.0 || fy < 0.0 || fx > w_f || fy > h_f {
        return fallback;
    }
    let sx = (fx - 0.5).clamp(0.0, (cw - 1) as f64);
    let sy = (fy - 0.5).clamp(0.0, (ch - 1) as f64);
    let x0 = sx.floor() as usize;
    let y0 = sy.floor() as usize;
    let x1 = (x0 + 1).min((cw - 1) as usize);
    let y1 = (y0 + 1).min((ch - 1) as usize);
    let tx = sx - (x0 as f64);
    let ty = sy - (y0 as f64);
    let stride = cw as usize;
    let v00 = luma[y0 * stride + x0];
    let v10 = luma[y0 * stride + x1];
    let v01 = luma[y1 * stride + x0];
    let v11 = luma[y1 * stride + x1];
    let top = v00 * (1.0 - tx) + v10 * tx;
    let bot = v01 * (1.0 - tx) + v11 * tx;
    top * (1.0 - ty) + bot * ty
}

pub(crate) fn rectify_perspective_luma(
    luma: &[f64],
    cx: u32,
    cy: u32,
    cw: u32,
    ch: u32,
    quad: [(f64, f64); 4],
    invert_polarity: bool,
) -> Option<Vec<f64>> {
    let dst_rect = [
        (0.0, 0.0),
        (f64::from(cw), 0.0),
        (f64::from(cw), f64::from(ch)),
        (0.0, f64::from(ch)),
    ];
    let local_quad = quad.map(|(qx, qy)| (qx - f64::from(cx), qy - f64::from(cy)));
    let h = compute_planar_homography(dst_rect, local_quad)?;
    let fallback = if invert_polarity { 0.0 } else { BYTE_MAX_F64 };
    let mut out = Vec::with_capacity((cw as usize) * (ch as usize));
    for ry in 0..ch {
        for rx in 0..cw {
            let u = f64::from(rx) + 0.5;
            let v = f64::from(ry) + 0.5;
            let denom = h[6] * u + h[7] * v + h[8];
            if denom.abs() <= HOMOGRAPHY_DENOM_EPS {
                out.push(fallback);
                continue;
            }
            let sx = (h[0] * u + h[1] * v + h[2]) / denom;
            let sy = (h[3] * u + h[4] * v + h[5]) / denom;
            out.push(sample_bilinear(luma, cw, ch, sx, sy, fallback));
        }
    }
    Some(out)
}
