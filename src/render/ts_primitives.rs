//! Shared `egui` pill badge and `tiny-skia` primitive drawing helpers for canvas overlays.

use crate::core::ScreenPoint;
use crate::tokens::{
    bbox_pill_bg_color32, bbox_pill_text_color32, color32_to_tiny_skia, pill_digit_bitmap_3x5,
    BBOX_PILL_CHAR_WIDTH_DP, BBOX_PILL_FONT_SIZE_DP, BBOX_PILL_MIN_HEIGHT_DP, BBOX_PILL_PAD_X_DP,
    PILL_DIGIT_ADVANCE_PX, PILL_DIGIT_FONT_HEIGHT_PX, PILL_DIGIT_FONT_WIDTH_PX,
};
use egui::{Align2, FontId, Painter, Pos2, Rect, Vec2};
use tiny_skia::{
    FillRule, Paint, PathBuilder, Pixmap, Rect as TsRect, Stroke as TsStroke, StrokeDash, Transform,
};

#[must_use]
pub(crate) fn pill_width_dp(text: &str) -> f32 {
    ((text.len() as f32) * BBOX_PILL_CHAR_WIDTH_DP + BBOX_PILL_PAD_X_DP * 2.0)
        .max(BBOX_PILL_MIN_HEIGHT_DP * 2.0)
}

pub(crate) fn paint_pill_egui(painter: &Painter, center: Pos2, text: &str) {
    let rect = Rect::from_center_size(
        center,
        Vec2::new(pill_width_dp(text), BBOX_PILL_MIN_HEIGHT_DP),
    );
    painter.rect_filled(rect, BBOX_PILL_MIN_HEIGHT_DP * 0.5, bbox_pill_bg_color32());
    painter.text(
        center,
        Align2::CENTER_CENTER,
        text,
        FontId::proportional(BBOX_PILL_FONT_SIZE_DP),
        bbox_pill_text_color32(),
    );
}

pub(crate) fn fill_circle_ts(
    pixmap: &mut Pixmap,
    cx: f32,
    cy: f32,
    r: f32,
    color: tiny_skia::Color,
) {
    let mut pb = PathBuilder::new();
    pb.push_circle(cx, cy, r);
    if let Some(path) = pb.finish() {
        let mut p = Paint::default();
        p.set_color(color);
        p.anti_alias = true;
        pixmap.fill_path(&path, &p, FillRule::Winding, Transform::identity(), None);
    }
}

pub(crate) fn stroke_ts(pixmap: &mut Pixmap, pb: PathBuilder, color: tiny_skia::Color, width: f32) {
    if let Some(path) = pb.finish() {
        let mut paint = Paint::default();
        paint.set_color(color);
        paint.anti_alias = true;
        let stroke = TsStroke {
            width,
            ..Default::default()
        };
        pixmap.stroke_path(&path, &paint, &stroke, Transform::identity(), None);
    }
}

pub(crate) fn stroke_dashed_ts(
    pixmap: &mut Pixmap,
    pb: PathBuilder,
    color: tiny_skia::Color,
    width: f32,
) {
    if let Some(path) = pb.finish() {
        let mut paint = Paint::default();
        paint.set_color(color);
        paint.anti_alias = true;
        let stroke = TsStroke {
            width,
            dash: StrokeDash::new(vec![width, width], 0.0),
            ..Default::default()
        };
        pixmap.stroke_path(&path, &paint, &stroke, Transform::identity(), None);
    }
}

pub(crate) fn stroke_rect_ts(
    pixmap: &mut Pixmap,
    x0: f32,
    y0: f32,
    x1: f32,
    y1: f32,
    col: tiny_skia::Color,
    width: f32,
) {
    if let Some(r) = TsRect::from_ltrb(x0.min(x1), y0.min(y1), x0.max(x1), y0.max(y1)) {
        let mut pb = PathBuilder::new();
        pb.push_rect(r);
        stroke_ts(pixmap, pb, col, width);
    }
}

pub(crate) fn stroke_line_ts(
    pixmap: &mut Pixmap,
    x0: f32,
    y0: f32,
    x1: f32,
    y1: f32,
    col: tiny_skia::Color,
    width: f32,
) {
    let mut pb = PathBuilder::new();
    pb.move_to(x0, y0);
    pb.line_to(x1, y1);
    stroke_ts(pixmap, pb, col, width);
}

pub(crate) fn fill_and_stroke_poly_ts(
    pixmap: &mut Pixmap,
    pts: &[ScreenPoint],
    fill_col: tiny_skia::Color,
    stroke_col: tiny_skia::Color,
    sw: f32,
) {
    let mut pb = PathBuilder::new();
    pb.move_to(pts[0].x.0, pts[0].y.0);
    for p in &pts[1..] {
        pb.line_to(p.x.0, p.y.0);
    }
    pb.close();
    if let Some(path) = pb.finish() {
        let mut fp = Paint::default();
        fp.set_color(fill_col);
        fp.anti_alias = true;
        pixmap.fill_path(&path, &fp, FillRule::Winding, Transform::identity(), None);
        let mut sp = Paint::default();
        sp.set_color(stroke_col);
        sp.anti_alias = true;
        pixmap.stroke_path(
            &path,
            &sp,
            &TsStroke {
                width: sw,
                ..Default::default()
            },
            Transform::identity(),
            None,
        );
    }
}

pub(crate) fn draw_bitmap_string_ts(
    pixmap: &mut Pixmap,
    start_x: f32,
    start_y: f32,
    text: &str,
    col: tiny_skia::Color,
) {
    let mut tp = Paint::default();
    tp.set_color(col);
    for (idx, ch) in text.chars().enumerate() {
        let rows = pill_digit_bitmap_3x5(ch);
        let gx = start_x + (idx as f32) * PILL_DIGIT_ADVANCE_PX;
        for (ry, &mask) in rows.iter().enumerate() {
            for rx in 0..PILL_DIGIT_FONT_WIDTH_PX {
                if (mask >> (PILL_DIGIT_FONT_WIDTH_PX - 1 - rx)) & 1 == 1 {
                    if let Some(px_rect) = TsRect::from_xywh(
                        (gx + rx as f32).round(),
                        (start_y + ry as f32).round(),
                        1.0,
                        1.0,
                    ) {
                        pixmap.fill_rect(px_rect, &tp, Transform::identity(), None);
                    }
                }
            }
        }
    }
}

pub(crate) fn draw_bitmap_label_ts(
    pixmap: &mut Pixmap,
    cx: f32,
    cy: f32,
    text: &str,
    col: tiny_skia::Color,
) {
    let start_x = cx - ((text.len() as f32) * PILL_DIGIT_ADVANCE_PX) * 0.5;
    let start_y = cy - (PILL_DIGIT_FONT_HEIGHT_PX as f32) * 0.5;
    draw_bitmap_string_ts(pixmap, start_x, start_y, text, col);
}

pub(crate) fn fill_pill_ts(pixmap: &mut Pixmap, cx: f32, cy: f32, text: &str) {
    let w = pill_width_dp(text);
    let r = BBOX_PILL_MIN_HEIGHT_DP * 0.5;
    let bg_col = color32_to_tiny_skia(bbox_pill_bg_color32());
    let inner_w = (w - BBOX_PILL_MIN_HEIGHT_DP).max(1.0);
    fill_circle_ts(pixmap, cx - inner_w * 0.5, cy, r, bg_col);
    fill_circle_ts(pixmap, cx + inner_w * 0.5, cy, r, bg_col);
    if let Some(rect) =
        TsRect::from_xywh(cx - inner_w * 0.5, cy - r, inner_w, BBOX_PILL_MIN_HEIGHT_DP)
    {
        let mut p = Paint::default();
        p.set_color(bg_col);
        pixmap.fill_rect(rect, &p, Transform::identity(), None);
    }
    draw_bitmap_label_ts(
        pixmap,
        cx,
        cy,
        text,
        color32_to_tiny_skia(bbox_pill_text_color32()),
    );
}
