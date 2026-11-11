//! `SPEC-007` (`grafia v0.6.0`) Bottom Preview Strip Engine with Separable 2-Pass 1D Gaussian Blur,
//! Vertical Scale Formula (`extra_spacing`, `more_space_above`, `more_space_below`),
//! Vertical Flip, and Negative Polarity (`inverted`).

use crate::adapters::headless::kurbo_to_tiny_skia_path;
use crate::adapters::state::GrafiaCanvasState;
use crate::core::{extract_segments, ContourSegment, ScreenPoint, ViewportTransform};
use crate::render::PreviewFilledProjection;
use crate::snapping::GlyphMetrics;
use crate::tokens::{
    DEFAULT_PREVIEW_STRIP_HEIGHT_PX, DEFAULT_PREVIEW_STRIP_WIDTH_PX, DEFAULT_X_HEIGHT_UPM,
    GAUSSIAN_MIN_WEIGHT_SUM, GAUSSIAN_SIGMA_RADIUS_MULTIPLIER, PREVIEW_BG_INVERTED_DARK,
    PREVIEW_BG_NORMAL_DARK, PREVIEW_BLUR_ACTIVE_THRESHOLD_PX, PREVIEW_BLUR_MAX_RADIUS_PX,
    PREVIEW_BLUR_MIN_RADIUS_PX, PREVIEW_BOTH_EXTRA_SPACE_SCALE, PREVIEW_DEFAULT_EXTRA_SPACING,
    PREVIEW_FG_INVERTED_DARK, PREVIEW_FG_NORMAL_DARK, PREVIEW_ONE_EXTRA_SPACE_SCALE,
    RGBA_CHANNEL_COUNT, RGBA_MAX_U8_F32,
};
use kurbo::BezPath;
use serde::{Deserialize, Serialize};
use tiny_skia::{Color as TsColor, FillRule, Paint, PathBuilder, Pixmap, Transform};

/// Configuration for the `SPEC-007` (`v0.6.0`) bottom preview strip renderer.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct PreviewRenderConfig {
    pub width_px: u32,
    pub height_px: u32,
    pub blur_radius_px: f32,
    pub flipped: bool,
    pub inverted: bool,
    pub extra_spacing: f64,
    pub more_space_above: bool,
    pub more_space_below: bool,
}

impl Default for PreviewRenderConfig {
    fn default() -> Self {
        Self::default_for_size(
            DEFAULT_PREVIEW_STRIP_WIDTH_PX,
            DEFAULT_PREVIEW_STRIP_HEIGHT_PX,
        )
    }
}

impl PreviewRenderConfig {
    #[must_use]
    pub fn default_for_size(width_px: u32, height_px: u32) -> Self {
        Self {
            width_px: width_px.max(1),
            height_px: height_px.max(1),
            blur_radius_px: PREVIEW_BLUR_MIN_RADIUS_PX,
            flipped: false,
            inverted: false,
            extra_spacing: PREVIEW_DEFAULT_EXTRA_SPACING,
            more_space_above: false,
            more_space_below: false,
        }
    }

    /// Computes vertical UPM-to-pixel scale factor (`SPEC-007 §2.1.1`).
    #[must_use]
    pub fn compute_vertical_scale(&self, metrics: &GlyphMetrics) -> f64 {
        let denom = ((metrics.ascender + metrics.descender.abs()) * self.extra_spacing).max(1.0);
        let base_scale = f64::from(self.height_px.max(1)) / denom;
        let space_factor = match (self.more_space_above, self.more_space_below) {
            (true, true) => PREVIEW_BOTH_EXTRA_SPACE_SCALE,
            (true, false) | (false, true) => PREVIEW_ONE_EXTRA_SPACE_SCALE,
            (false, false) => 1.0,
        };
        base_scale * space_factor
    }
}

/// Builds a normalized 1D Gaussian kernel `(radius, weights)` for $\sigma \in [0, 20]$, $R = \lceil 3\sigma \rceil$.
#[must_use]
pub fn build_gaussian_kernel_1d(blur_radius_px: f32) -> (usize, Vec<f32>) {
    let sigma = if blur_radius_px.is_finite() {
        blur_radius_px.clamp(PREVIEW_BLUR_MIN_RADIUS_PX, PREVIEW_BLUR_MAX_RADIUS_PX)
    } else {
        PREVIEW_BLUR_MIN_RADIUS_PX
    };
    if sigma <= PREVIEW_BLUR_ACTIVE_THRESHOLD_PX {
        return (0, vec![1.0]);
    }
    let radius = (sigma * GAUSSIAN_SIGMA_RADIUS_MULTIPLIER).ceil() as usize;
    let two_sigma_sq = 2.0 * sigma * sigma;
    let side = radius * 2 + 1;
    let mut weights = Vec::with_capacity(side);
    let mut sum = 0.0f32;
    let r_isize = radius as isize;
    for k in -r_isize..=r_isize {
        let kf = k as f32;
        let w = (-(kf * kf) / two_sigma_sq).exp();
        weights.push(w);
        sum += w;
    }
    let inv_sum = 1.0 / sum.max(GAUSSIAN_MIN_WEIGHT_SUM);
    for w in &mut weights {
        *w *= inv_sum;
    }
    (radius, weights)
}

/// Applies a separable 2-pass (horizontal + vertical) 1D Gaussian convolution with clamp-to-edge boundary handling.
pub fn apply_separable_gaussian_blur_rgba(
    rgba: &mut [u8],
    width: usize,
    height: usize,
    blur_radius_px: f32,
) {
    if width == 0 || height == 0 || rgba.len() < width * height * RGBA_CHANNEL_COUNT {
        return;
    }
    let (radius, weights) = build_gaussian_kernel_1d(blur_radius_px);
    if radius == 0 {
        return;
    }
    let r_isize = radius as isize;
    let max_x = (width - 1) as isize;
    let max_y = (height - 1) as isize;
    let mut horiz = vec![0.0f32; width * height * RGBA_CHANNEL_COUNT];

    for y in 0..height {
        let row_offset = y * width;
        for x in 0..width {
            let mut acc = [0.0f32; RGBA_CHANNEL_COUNT];
            for (wi, k) in (-r_isize..=r_isize).enumerate() {
                let sx = (x as isize + k).clamp(0, max_x) as usize;
                let src_idx = (row_offset + sx) * RGBA_CHANNEL_COUNT;
                let w = weights[wi];
                acc[0] += f32::from(rgba[src_idx]) * w;
                acc[1] += f32::from(rgba[src_idx + 1]) * w;
                acc[2] += f32::from(rgba[src_idx + 2]) * w;
                acc[3] += f32::from(rgba[src_idx + 3]) * w;
            }
            let dst_idx = (row_offset + x) * RGBA_CHANNEL_COUNT;
            horiz[dst_idx..dst_idx + RGBA_CHANNEL_COUNT].copy_from_slice(&acc);
        }
    }

    for y in 0..height {
        for x in 0..width {
            let mut acc = [0.0f32; RGBA_CHANNEL_COUNT];
            for (wi, k) in (-r_isize..=r_isize).enumerate() {
                let sy = (y as isize + k).clamp(0, max_y) as usize;
                let src_idx = (sy * width + x) * RGBA_CHANNEL_COUNT;
                let w = weights[wi];
                acc[0] += horiz[src_idx] * w;
                acc[1] += horiz[src_idx + 1] * w;
                acc[2] += horiz[src_idx + 2] * w;
                acc[3] += horiz[src_idx + 3] * w;
            }
            let dst_idx = (y * width + x) * RGBA_CHANNEL_COUNT;
            rgba[dst_idx] = acc[0].round().clamp(0.0, RGBA_MAX_U8_F32) as u8;
            rgba[dst_idx + 1] = acc[1].round().clamp(0.0, RGBA_MAX_U8_F32) as u8;
            rgba[dst_idx + 2] = acc[2].round().clamp(0.0, RGBA_MAX_U8_F32) as u8;
            rgba[dst_idx + 3] = acc[3].round().clamp(0.0, RGBA_MAX_U8_F32) as u8;
        }
    }
}

/// Rasterizes the bottom preview strip into a deterministic `[u8]` RGBA buffer (`SPEC-007 §2.1`).
#[must_use]
pub fn render_preview_strip_rgba(state: &GrafiaCanvasState, cfg: &PreviewRenderConfig) -> Vec<u8> {
    let w = cfg.width_px.max(1);
    let h = cfg.height_px.max(1);
    let mut pixmap = Pixmap::new(w, h).expect("Failed to allocate preview strip Pixmap");

    let is_dark = state.theme.is_dark();
    let (bg_tok, fg_tok) = match (is_dark, cfg.inverted) {
        (false, false) => (state.theme.preview_bg_normal, state.theme.preview_fg_normal),
        (false, true) => (
            state.theme.preview_bg_inverted,
            state.theme.preview_fg_inverted,
        ),
        (true, false) => (PREVIEW_BG_NORMAL_DARK, PREVIEW_FG_NORMAL_DARK),
        (true, true) => (PREVIEW_BG_INVERTED_DARK, PREVIEW_FG_INVERTED_DARK),
    };

    pixmap.fill(TsColor::from_rgba8(bg_tok.r, bg_tok.g, bg_tok.b, bg_tok.a));

    let scale = cfg.compute_vertical_scale(&state.metrics);
    let center_y = h as f32 * 0.5;
    let baseline_y =
        center_y + ((state.metrics.ascender + state.metrics.descender) * 0.5 * scale) as f32;

    let adv_w = if let Some(last) = state.text_stream.slots.last() {
        (last.origin_x_upm + last.glyph.width).max(DEFAULT_X_HEIGHT_UPM)
    } else if state.glyph.width > 0.0 {
        state.glyph.width
    } else {
        DEFAULT_X_HEIGHT_UPM
    };
    let origin_x_scr = w as f32 * 0.5 - (adv_w * 0.5 * scale) as f32;
    let vp = ViewportTransform::new(
        scale as f32,
        ScreenPoint::new(origin_x_scr, baseline_y),
        state.glyph.upm.max(1),
    );

    let mut fg_paint = Paint::default();
    fg_paint.set_color(TsColor::from_rgba8(fg_tok.r, fg_tok.g, fg_tok.b, fg_tok.a));
    fg_paint.anti_alias = true;

    if state.text_stream.slots.is_empty() {
        let mut compound = BezPath::new();
        for (c_idx, path) in state.paths.iter().enumerate() {
            if !state.is_contour_closed(c_idx) {
                continue;
            }
            let proj = PreviewFilledProjection::compute(path, &vp);
            for el in proj.screen_path.elements() {
                compound.push(*el);
            }
        }
        if let Some(ts_path) = kurbo_to_tiny_skia_path(&compound) {
            pixmap.fill_path(
                &ts_path,
                &fg_paint,
                FillRule::Winding,
                Transform::identity(),
                None,
            );
        }
    } else {
        for slot in &state.text_stream.slots {
            let slot_vp = vp.with_origin_x_upm(slot.origin_x_upm);
            let mut pb = PathBuilder::new();
            for gc in &slot.glyph.contours {
                if !gc.visible || !gc.closed || gc.nodes.len() < 2 {
                    continue;
                }
                let s0 = slot_vp.to_screen(gc.nodes[0].point);
                pb.move_to(s0.x.0, s0.y.0);
                for seg in extract_segments(&gc.nodes, true) {
                    match seg {
                        ContourSegment::Line { end_idx, .. } => {
                            let p = slot_vp.to_screen(gc.nodes[end_idx].point);
                            pb.line_to(p.x.0, p.y.0);
                        }
                        ContourSegment::Cubic {
                            h1_idx,
                            h2_idx,
                            end_idx,
                            ..
                        } => {
                            let h1 = slot_vp.to_screen(gc.nodes[h1_idx].point);
                            let h2 = slot_vp.to_screen(gc.nodes[h2_idx].point);
                            let p3 = slot_vp.to_screen(gc.nodes[end_idx].point);
                            pb.cubic_to(h1.x.0, h1.y.0, h2.x.0, h2.y.0, p3.x.0, p3.y.0);
                        }
                    }
                }
                pb.close();
            }
            if let Some(path) = pb.finish() {
                pixmap.fill_path(
                    &path,
                    &fg_paint,
                    FillRule::Winding,
                    Transform::identity(),
                    None,
                );
            }
        }
    }

    let mut rgba: Vec<u8> = pixmap.take();
    let w_usize = w as usize;
    let h_usize = h as usize;
    if cfg.flipped && h_usize > 1 {
        let row_bytes = w_usize * RGBA_CHANNEL_COUNT;
        for y in 0..(h_usize / 2) {
            let top_start = y * row_bytes;
            let bot_start = (h_usize - 1 - y) * row_bytes;
            for b in 0..row_bytes {
                rgba.swap(top_start + b, bot_start + b);
            }
        }
    }

    apply_separable_gaussian_blur_rgba(&mut rgba, w_usize, h_usize, cfg.blur_radius_px);
    rgba
}

/// Dirty-state cache for the `SPEC-007` (`v0.6.0`) bottom preview strip RGBA buffer and `egui::TextureHandle`.
///
/// Executes `render_preview_strip_rgba` and GPU texture upload (`TextureHandle::set`) **only** when
/// `(state.content_generation(), state.theme.is_dark(), cfg)` changes, yielding 0.0 ms CPU and
/// 0 GPU uploads during idle pointer hover, viewport pan/zoom, or selection changes.
#[derive(Default)]
pub struct PreviewStripCache {
    last_generation: Option<u64>,
    last_is_dark: Option<bool>,
    last_config: Option<PreviewRenderConfig>,
    cached_rgba: Vec<u8>,
    texture: Option<egui::TextureHandle>,
    render_count: u64,
}

impl PreviewStripCache {
    /// Creates an empty `PreviewStripCache`.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Returns the number of times `render_preview_strip_rgba` has been executed by this cache.
    #[must_use]
    pub const fn render_count(&self) -> u64 {
        self.render_count
    }

    /// Explicitly invalidates the cached buffer so the next query recomputes the preview strip.
    pub fn invalidate(&mut self) {
        self.last_generation = None;
    }

    /// Returns `(&[u8], recomputed)`, rasterizing the preview strip only when content or config changed.
    pub fn get_or_render_rgba(
        &mut self,
        state: &GrafiaCanvasState,
        cfg: &PreviewRenderConfig,
    ) -> (&[u8], bool) {
        let gen = state.content_generation();
        let is_dark = state.theme.is_dark();
        let is_hit = self.last_generation == Some(gen)
            && self.last_is_dark == Some(is_dark)
            && self.last_config.as_ref() == Some(cfg);
        if !is_hit {
            self.cached_rgba = render_preview_strip_rgba(state, cfg);
            self.last_generation = Some(gen);
            self.last_is_dark = Some(is_dark);
            self.last_config = Some(cfg.clone());
            self.render_count += 1;
        }
        (&self.cached_rgba, !is_hit)
    }

    /// Returns `(&egui::TextureHandle, updated)`, uploading to the GPU texture only on a cache miss.
    pub fn get_or_update_texture(
        &mut self,
        ctx: &egui::Context,
        state: &GrafiaCanvasState,
        cfg: &PreviewRenderConfig,
    ) -> (&egui::TextureHandle, bool) {
        let w = cfg.width_px.max(1) as usize;
        let h = cfg.height_px.max(1) as usize;
        let (_, recomputed) = self.get_or_render_rgba(state, cfg);
        let need_upload = recomputed || self.texture.is_none();
        if need_upload {
            let img = egui::ColorImage::from_rgba_unmultiplied([w, h], &self.cached_rgba);
            if let Some(tex) = &mut self.texture {
                tex.set(img, egui::TextureOptions::LINEAR);
            } else {
                self.texture = Some(ctx.load_texture(
                    "grafia_preview_strip_tex",
                    img,
                    egui::TextureOptions::LINEAR,
                ));
            }
        }
        (
            self.texture
                .as_ref()
                .expect("preview strip texture initialized"),
            need_upload,
        )
    }
}
