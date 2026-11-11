//! Shape canvas thumbnail projection and winding inspection.

use crate::adapters::headless::kurbo_to_tiny_skia_path;
use crate::adapters::state::GlifoCanvasState;
use crate::tokens::{color32_to_tiny_skia, GlifoTheme, ShapeCanvasTokens};
use kurbo::{BezPath, Rect, Shape};
use tiny_skia::{FillRule, Paint, Pixmap, Rect as TsRect, Stroke as TsStroke, Transform};

#[derive(Debug, Clone, PartialEq)]
pub struct ShapeThumbnailProjection {
    pub scale: f64,
    pub offset_x: f64,
    pub offset_y: f64,
    pub target_rect: Rect,
    pub fitted_path: BezPath,
    pub is_clockwise: bool,
    pub node_count: usize,
}

impl ShapeThumbnailProjection {
    /// Computes a thumbnail projection fitting any `kurbo::BezPath` into `tokens` bounds.
    #[must_use]
    pub fn compute(path: &BezPath, tokens: &ShapeCanvasTokens, node_count: usize, is_clockwise: bool) -> Self {
        let bbox = path.bounding_box();
        let avail_w = (tokens.thumbnail_width_dp - (2.0 * tokens.thumbnail_padding_dp)) as f64;
        let avail_h = (tokens.thumbnail_height_dp - (2.0 * tokens.thumbnail_padding_dp)) as f64;

        let bw = bbox.width().max(1.0);
        let bh = bbox.height().max(1.0);

        let scale = (avail_w / bw).min(avail_h / bh);
        let offset_x = tokens.thumbnail_padding_dp as f64 - (bbox.min_x() * scale);
        let offset_y = tokens.thumbnail_padding_dp as f64 - (bbox.min_y() * scale);
        let flip_y = tokens.thumbnail_padding_dp as f64 + (bbox.max_y() * scale);

        let affine = kurbo::Affine::translate((offset_x, flip_y))
            * kurbo::Affine::scale_non_uniform(scale, -scale);
        let fitted_path = affine * path;

        Self {
            scale,
            offset_x,
            offset_y,
            target_rect: Rect::new(0.0, 0.0, tokens.thumbnail_width_dp as f64, tokens.thumbnail_height_dp as f64),
            fitted_path,
            is_clockwise,
            node_count,
        }
    }
}

/// Renders `CanvasViewMode::ShapeThumbnail` cards into a `tiny_skia::Pixmap`.
pub fn render_shape_thumbnails_tiny_skia(
    pixmap: &mut Pixmap,
    state: &GlifoCanvasState,
    theme: &GlifoTheme,
) {
    if state.paths.is_empty() {
        return;
    }
    let pad = theme.shape_canvas.header_height_dp * 2.0;
    let mut row_y = pad + theme.shape_canvas.header_height_dp;
    let card_w = theme.shape_canvas.thumbnail_width_dp * 2.0 * 2.0;
    let card_h = theme.shape_canvas.thumbnail_height_dp * 2.0 * 2.0;

    for (idx, path) in state.paths.iter().enumerate() {
        let is_closed = state.is_contour_closed(idx);
        let node_count = state.contours.get(idx).map_or(0, |c| c.len());
        let mut scaled_cfg = theme.shape_canvas.clone();
        scaled_cfg.thumbnail_width_dp = card_w;
        scaled_cfg.thumbnail_height_dp = card_h;
        scaled_cfg.thumbnail_padding_dp = theme.shape_canvas.thumbnail_padding_dp * 2.0 * 2.0;
        let proj = ShapeThumbnailProjection::compute(path, &scaled_cfg, node_count, true);
        if let Some(card_rect) = TsRect::from_xywh(pad, row_y, card_w, card_h) {
            let mut card_paint = Paint::default();
            card_paint.set_color(color32_to_tiny_skia(
                theme.shape_canvas.accent_selection_color32(),
            ));
            pixmap.fill_rect(card_rect, &card_paint, Transform::identity(), None);
        }
        let translated = kurbo::Affine::translate((pad as f64, row_y as f64)) * proj.fitted_path;
        if let Some(ts_path) = kurbo_to_tiny_skia_path(&translated) {
            let mut fg_paint = Paint::default();
            fg_paint.set_color(color32_to_tiny_skia(theme.shape_canvas.text_active_color32()));
            fg_paint.anti_alias = true;
            if is_closed {
                pixmap.fill_path(&ts_path, &fg_paint, FillRule::Winding, Transform::identity(), None);
            } else {
                let stroke = TsStroke {
                    width: theme.edit_canvas.contour_stroke_width_dp,
                    ..Default::default()
                };
                pixmap.stroke_path(&ts_path, &fg_paint, &stroke, Transform::identity(), None);
            }
        }
        row_y += card_h + theme.shape_canvas.header_height_dp;
    }
}
