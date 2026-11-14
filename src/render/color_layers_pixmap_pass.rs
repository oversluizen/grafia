//! Pass 2 & Pass 4 `tiny-skia` pixmap rendering and colored thumbnail projection for
//! `CanvasColorSublayer` (`SPEC-010 §2.1`).

use crate::adapters::state::GrafiaCanvasState;
use crate::core::math_consts::{
    BYTE_MAX_F64, MIN_VIEWPORT_ZOOM, THUMBNAIL_MIN_SPAN_UPM, THUMBNAIL_PADDING_FRAC,
};
use crate::core::{
    contour_to_bezpath_with_splines, evaluate_color_layer_paint_at_upm, porter_duff_src_over,
    CanvasColorSublayer, ColorLayerPaint, FontPoint, GlyphContour, ScreenPoint, ViewportTransform,
};
use crate::render::shared_geom::{gradient_overlay_radii_dp, project_gradient_handles_screen};
use crate::render::ts_primitives::{ts_fill_circle, ts_stroke_circle, ts_stroke_line};
use crate::tokens::trace_and_band_tokens::{gradient_handle_ring_color32, rgba_u8_to_color32};
use crate::tokens::{AppearanceMode, GrafiaTheme};
use kurbo::PathEl;
use tiny_skia::{FillRule, Paint, PathBuilder, Pixmap, Transform};

fn build_contours_ts_path(
    contours: &[GlyphContour],
    vp: &ViewportTransform,
) -> Option<tiny_skia::Path> {
    let mut pb = PathBuilder::new();
    for gc in contours {
        if !gc.visible || gc.nodes.len() < 2 {
            continue;
        }
        let bez = contour_to_bezpath_with_splines(&gc.nodes, gc.closed, 0.0, &[]);
        for el in bez.elements() {
            match *el {
                PathEl::MoveTo(p) => {
                    let s = vp.to_screen(FontPoint::new(p.x, p.y));
                    pb.move_to(s.x.0, s.y.0);
                }
                PathEl::LineTo(p) => {
                    let s = vp.to_screen(FontPoint::new(p.x, p.y));
                    pb.line_to(s.x.0, s.y.0);
                }
                PathEl::QuadTo(p1, p2) => {
                    let s1 = vp.to_screen(FontPoint::new(p1.x, p1.y));
                    let s2 = vp.to_screen(FontPoint::new(p2.x, p2.y));
                    pb.quad_to(s1.x.0, s1.y.0, s2.x.0, s2.y.0);
                }
                PathEl::CurveTo(p1, p2, p3) => {
                    let s1 = vp.to_screen(FontPoint::new(p1.x, p1.y));
                    let s2 = vp.to_screen(FontPoint::new(p2.x, p2.y));
                    let s3 = vp.to_screen(FontPoint::new(p3.x, p3.y));
                    pb.cubic_to(s1.x.0, s1.y.0, s2.x.0, s2.y.0, s3.x.0, s3.y.0);
                }
                PathEl::ClosePath => pb.close(),
            }
        }
    }
    pb.finish()
}

fn rasterize_contours_coverage_mask(
    contours: &[GlyphContour],
    vp: &ViewportTransform,
    width: u32,
    height: u32,
) -> Option<Vec<u8>> {
    let mut mask_pix = Pixmap::new(width.max(1), height.max(1))?;
    let path = build_contours_ts_path(contours, vp)?;
    let mut paint = Paint::default();
    paint.set_color_rgba8(255, 255, 255, 255);
    paint.anti_alias = true;
    mask_pix.fill_path(
        &path,
        &paint,
        FillRule::Winding,
        Transform::identity(),
        None,
    );
    Some(
        mask_pix
            .data()
            .as_chunks::<4>()
            .0
            .iter()
            .map(|px| px[3])
            .collect(),
    )
}

/// Rasterizes `sublayers` into an unmultiplied RGBA buffer of size `width * height * 4`.
#[must_use]
pub fn rasterize_color_sublayers_rgba(
    sublayers: &[CanvasColorSublayer],
    vp: &ViewportTransform,
    width: u32,
    height: u32,
) -> Vec<u8> {
    let w = width.max(1);
    let h = height.max(1);
    let mut out = vec![0_u8; (w * h * 4) as usize];
    let mut clip_mask: Option<Vec<u8>> = None;

    for sub in sublayers {
        if !sub.visible || sub.contours.is_empty() {
            continue;
        }
        let Some(cov) = rasterize_contours_coverage_mask(&sub.contours, vp, w, h) else {
            continue;
        };
        if matches!(sub.paint, ColorLayerPaint::ClippingMask) {
            if let Some(ref mut existing) = clip_mask {
                for (c, &m) in existing.iter_mut().zip(cov.iter()) {
                    *c = ((u16::from(*c) * u16::from(m)) / 255) as u8;
                }
            } else {
                clip_mask = Some(cov);
            }
            continue;
        }
        composite_sublayer_into_rgba(&mut out, &cov, clip_mask.as_deref(), sub, vp, w, h);
    }
    out
}

fn composite_sublayer_into_rgba(
    out: &mut [u8],
    cov: &[u8],
    clip_mask: Option<&[u8]>,
    sub: &CanvasColorSublayer,
    vp: &ViewportTransform,
    w: u32,
    h: u32,
) {
    for py in 0..h {
        let sy = (py as f32) + 0.5;
        for px in 0..w {
            let p_idx = (py * w + px) as usize;
            let raw_cov = cov[p_idx];
            if raw_cov == 0 {
                continue;
            }
            let clip_a = clip_mask.map_or(255_u8, |cm| cm[p_idx]);
            if clip_a == 0 {
                continue;
            }
            let eff_cov = (f64::from(raw_cov) * f64::from(clip_a)) / (BYTE_MAX_F64 * BYTE_MAX_F64);
            let sx = (px as f32) + 0.5;
            let pt_upm = vp.to_font(ScreenPoint::new(sx, sy));
            let mut src = evaluate_color_layer_paint_at_upm(&sub.paint, pt_upm, sub.opacity);
            src[3] = (f64::from(src[3]) * eff_cov)
                .round()
                .clamp(0.0, BYTE_MAX_F64) as u8;
            if src[3] == 0 {
                continue;
            }
            let base = p_idx * 4;
            let dst = [out[base], out[base + 1], out[base + 2], out[base + 3]];
            let blended = porter_duff_src_over(src, dst);
            out[base..base + 4].copy_from_slice(&blended);
        }
    }
}

/// Blends visible `state.color_sublayers` onto `pixmap` during Pass 2 (`SPEC-010 §2.1`).
pub fn paint_color_layers_pixmap_pass(pixmap: &mut Pixmap, state: &GrafiaCanvasState) {
    if state.color_sublayers.is_empty() {
        return;
    }
    let vp = state.active_slot_viewport();
    let w = pixmap.width();
    let h = pixmap.height();
    let overlay_rgba = rasterize_color_sublayers_rgba(&state.color_sublayers, &vp, w, h);
    blend_unmultiplied_rgba_onto_pixmap(pixmap, &overlay_rgba);
}

fn blend_unmultiplied_rgba_onto_pixmap(pixmap: &mut Pixmap, overlay_rgba: &[u8]) {
    let data = pixmap.data_mut();
    for (dst_pm, src) in data
        .as_chunks_mut::<4>()
        .0
        .iter_mut()
        .zip(overlay_rgba.as_chunks::<4>().0.iter())
    {
        if src[3] == 0 {
            continue;
        }
        let sa = f64::from(src[3]) / BYTE_MAX_F64;
        let inv_sa = 1.0 - sa;
        dst_pm[0] = (f64::from(src[0]) * sa + f64::from(dst_pm[0]) * inv_sa)
            .round()
            .clamp(0.0, BYTE_MAX_F64) as u8;
        dst_pm[1] = (f64::from(src[1]) * sa + f64::from(dst_pm[1]) * inv_sa)
            .round()
            .clamp(0.0, BYTE_MAX_F64) as u8;
        dst_pm[2] = (f64::from(src[2]) * sa + f64::from(dst_pm[2]) * inv_sa)
            .round()
            .clamp(0.0, BYTE_MAX_F64) as u8;
        dst_pm[3] = (f64::from(src[3]) + f64::from(dst_pm[3]) * inv_sa)
            .round()
            .clamp(0.0, BYTE_MAX_F64) as u8;
    }
}

/// Paints interactive gradient handles in `tiny-skia` Pass 4 when a color sublayer is selected.
pub fn paint_color_layer_handles_pixmap_pass(
    pixmap: &mut Pixmap,
    state: &GrafiaCanvasState,
    theme: &GrafiaTheme,
) {
    let Some(sub_idx) = state.drag_transients.selected_color_sublayer else {
        return;
    };
    let Some(sub) = state.color_sublayers.get(sub_idx) else {
        return;
    };
    if !sub.visible || sub.locked {
        return;
    }
    let sel_stop = state
        .drag_transients
        .selected_gradient_stop
        .and_then(|(s, idx)| (s == sub_idx).then_some(idx));
    let vp = state.active_slot_viewport();
    let Some(proj) = project_gradient_handles_screen(&sub.paint, sel_stop, &vp) else {
        return;
    };
    let ring_col = gradient_handle_ring_color32(theme.is_dark());
    let (end_r, stop_r, sel_stop_r, line_w) = gradient_overlay_radii_dp();

    if let Some((c0_scr, r0_px)) = proj.radial_r0_scr {
        if r0_px > 0.5 {
            ts_stroke_circle(pixmap, c0_scr, r0_px, ring_col, 1.0);
        }
    }
    if let Some((c1_scr, r1_px)) = proj.radial_r1_scr {
        ts_stroke_circle(pixmap, c1_scr, r1_px, ring_col, 1.0);
    }
    ts_stroke_line(pixmap, proj.start_scr, proj.end_scr, ring_col, line_w);
    ts_stroke_circle(pixmap, proj.start_scr, end_r, ring_col, line_w);
    ts_stroke_circle(pixmap, proj.end_scr, end_r, ring_col, line_w);
    for (pt_scr, rgba, is_sel) in proj.stop_ticks {
        let r = if is_sel { sel_stop_r } else { stop_r };
        ts_fill_circle(pixmap, pt_scr, r, rgba_u8_to_color32(rgba));
        ts_stroke_circle(pixmap, pt_scr, r, ring_col, 1.0);
    }
}

/// Renders an unmultiplied RGBA thumbnail (`width * height * 4`) of `state` in `appearance` (`SPEC-010 §2.1`).
/// Falls back to monochrome Non-Zero Winding when `state.color_sublayers` is empty.
#[must_use]
pub fn render_colored_thumbnail(
    state: &GrafiaCanvasState,
    width: u32,
    height: u32,
    appearance: AppearanceMode,
) -> Vec<u8> {
    let w = width.max(1);
    let h = height.max(1);
    let is_dark = appearance == AppearanceMode::Dark;
    let theme = GrafiaTheme::for_appearance(appearance, is_dark);
    let bg = theme.canvas_background_color32().to_srgba_unmultiplied();
    let mut out = Vec::with_capacity((w * h * 4) as usize);
    for _ in 0..(w * h) {
        out.extend_from_slice(&[bg[0], bg[1], bg[2], 255]);
    }
    let vp = compute_thumbnail_viewport(state, w, h);
    if !state.color_sublayers.is_empty() {
        let col_rgba = rasterize_color_sublayers_rgba(&state.color_sublayers, &vp, w, h);
        for (dst, src) in out
            .as_chunks_mut::<4>()
            .0
            .iter_mut()
            .zip(col_rgba.as_chunks::<4>().0.iter())
        {
            if src[3] > 0 {
                *dst = porter_duff_src_over(*src, *dst);
            }
        }
        return out;
    }
    if let Some(cov) = rasterize_contours_coverage_mask(&state.glyph.contours, &vp, w, h) {
        let fg = theme.preview_canvas.fill_color32().to_srgba_unmultiplied();
        for (dst, &m) in out.as_chunks_mut::<4>().0.iter_mut().zip(cov.iter()) {
            if m > 0 {
                *dst =
                    porter_duff_src_over([fg[0], fg[1], fg[2], m], [dst[0], dst[1], dst[2], 255]);
            }
        }
    }
    out
}

fn compute_thumbnail_viewport(
    state: &GrafiaCanvasState,
    width: u32,
    height: u32,
) -> ViewportTransform {
    let mut min_x = f64::INFINITY;
    let mut max_x = f64::NEG_INFINITY;
    let mut min_y = f64::INFINITY;
    let mut max_y = f64::NEG_INFINITY;
    for gc in state
        .glyph
        .contours
        .iter()
        .chain(state.color_sublayers.iter().flat_map(|s| s.contours.iter()))
    {
        for n in &gc.nodes {
            min_x = min_x.min(n.point.x.0);
            max_x = max_x.max(n.point.x.0);
            min_y = min_y.min(n.point.y.0);
            max_y = max_y.max(n.point.y.0);
        }
    }
    if !min_x.is_finite() {
        min_x = 0.0;
        max_x = state.glyph.width.max(THUMBNAIL_MIN_SPAN_UPM);
        min_y = state.metrics.descender;
        max_y = state.metrics.ascender;
    }
    let span_x = (max_x - min_x).max(THUMBNAIL_MIN_SPAN_UPM);
    let span_y = (max_y - min_y).max(THUMBNAIL_MIN_SPAN_UPM);
    let pad = 1.0 - 2.0 * THUMBNAIL_PADDING_FRAC;
    let zoom_x = (f64::from(width) * pad) / span_x;
    let zoom_y = (f64::from(height) * pad) / span_y;
    let zoom = (zoom_x.min(zoom_y) as f32).max(MIN_VIEWPORT_ZOOM);
    let cx = 0.5 * (min_x + max_x);
    let cy = 0.5 * (min_y + max_y);
    ViewportTransform {
        upm: state.viewport.upm.max(1),
        zoom,
        pan_offset: ScreenPoint::new(
            0.5 * (width as f32) - (cx as f32) * zoom,
            0.5 * (height as f32) + (cy as f32) * zoom,
        ),
    }
}
