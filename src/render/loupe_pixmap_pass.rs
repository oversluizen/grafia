//! `SPEC-009` `tiny-skia` offscreen overlays for guide bands, off-curve handles,
//! active editing fill, full crosshair cursor, and touch precision loupe.

use crate::adapters::headless::append_kurbo_to_builder;
use crate::adapters::state::GrafiaCanvasState;
use crate::core::{FontPoint, ScreenPoint};
use crate::render::guide_band_and_loupe_overlays::{compute_loupe_center, guide_band_screen_quad};
use crate::render::ts_primitives::{fill_circle_ts, stroke_line_ts, stroke_ts};
use crate::render::PreviewFilledProjection;
use crate::snapping::Guide;
use crate::tokens::{
    color32_to_tiny_skia, edit_active_fill_color32, full_crosshair_color32,
    guide_band_fill_color32, touch_loupe_fill_color32, touch_loupe_ring_color32, GrafiaTheme,
    OffCurveHandleShape, CURVE_FLATTEN_TOLERANCE, FULL_CROSSHAIR_STROKE_WIDTH_DP,
    OFFCURVE_CROSS_ARM_SCALE, OFFCURVE_CROSS_STROKE_WIDTH_DP, TOUCH_LOUPE_CROSSHAIR_HALF_DP,
    TOUCH_LOUPE_CROSSHAIR_WIDTH_DP, TOUCH_LOUPE_MAGNIFICATION, TOUCH_LOUPE_RADIUS_DP,
    TOUCH_LOUPE_RING_WIDTH_DP,
};
use kurbo::PathEl;
use tiny_skia::{FillRule, Paint, PathBuilder, Pixmap, Rect as TsRect, Transform};

pub fn render_guide_band_tiny_skia(
    pixmap: &mut Pixmap,
    guide: &Guide,
    state: &GrafiaCanvasState,
    s0: ScreenPoint,
    s1: ScreenPoint,
    guide_col: egui::Color32,
) {
    let Some(quad) = guide_band_screen_quad(guide, state, s0, s1) else {
        return;
    };
    let fill = color32_to_tiny_skia(guide_band_fill_color32(guide_col));
    let mut pb = PathBuilder::new();
    pb.move_to(quad[0].x.0, quad[0].y.0);
    for p in &quad[1..] {
        pb.line_to(p.x.0, p.y.0);
    }
    pb.close();
    if let Some(path) = pb.finish() {
        let mut paint = Paint::default();
        paint.set_color(fill);
        paint.anti_alias = true;
        pixmap.fill_path(
            &path,
            &paint,
            FillRule::Winding,
            Transform::identity(),
            None,
        );
    }
}

pub fn render_offcurve_handle_tiny_skia(
    pixmap: &mut Pixmap,
    cx: f32,
    cy: f32,
    radius_dp: f32,
    shape: OffCurveHandleShape,
    color: tiny_skia::Color,
) {
    match shape {
        OffCurveHandleShape::Circle => fill_circle_ts(pixmap, cx, cy, radius_dp, color),
        OffCurveHandleShape::Square => {
            if let Some(rect) = TsRect::from_xywh(
                cx - radius_dp,
                cy - radius_dp,
                radius_dp * 2.0,
                radius_dp * 2.0,
            ) {
                let mut paint = Paint::default();
                paint.set_color(color);
                pixmap.fill_rect(rect, &paint, Transform::identity(), None);
            }
        }
        OffCurveHandleShape::Cross => {
            let arm = radius_dp * OFFCURVE_CROSS_ARM_SCALE * 2.0;
            let mut pb = PathBuilder::new();
            pb.move_to(cx - arm, cy - arm);
            pb.line_to(cx + arm, cy + arm);
            pb.move_to(cx - arm, cy + arm);
            pb.line_to(cx + arm, cy - arm);
            stroke_ts(pixmap, pb, color, OFFCURVE_CROSS_STROKE_WIDTH_DP);
        }
    }
}

pub fn render_edit_active_fill_tiny_skia(
    pixmap: &mut Pixmap,
    state: &GrafiaCanvasState,
    theme: &GrafiaTheme,
) {
    let vp = state.active_slot_viewport();
    let mut paint = Paint::default();
    paint.set_color(color32_to_tiny_skia(edit_active_fill_color32(
        theme.preview_canvas.fill_color32(),
    )));
    paint.anti_alias = true;
    let mut pb = PathBuilder::new();
    for (c_idx, path) in state.paths.iter().enumerate() {
        if !state.is_contour_closed(c_idx) {
            continue;
        }
        let proj = PreviewFilledProjection::compute(path, &vp);
        append_kurbo_to_builder(&mut pb, &proj.screen_path);
    }
    if let Some(compound_path) = pb.finish() {
        pixmap.fill_path(
            &compound_path,
            &paint,
            FillRule::Winding,
            Transform::identity(),
            None,
        );
    }
}

pub fn render_top_cursor_and_loupe_overlays_tiny_skia(
    pixmap: &mut Pixmap,
    state: &GrafiaCanvasState,
    theme: &GrafiaTheme,
) {
    let Some(scr_pt) = state.cursor_screen_pos else {
        return;
    };
    let w = pixmap.width() as f32;
    let h = pixmap.height() as f32;
    if scr_pt.x.0 < 0.0 || scr_pt.x.0 > w || scr_pt.y.0 < 0.0 || scr_pt.y.0 > h {
        return;
    }

    if theme.edit_canvas.full_crosshair_cursor {
        let ch_col =
            color32_to_tiny_skia(full_crosshair_color32(theme.preview_canvas.fill_color32()));
        stroke_line_ts(
            pixmap,
            0.0,
            scr_pt.y.0,
            w,
            scr_pt.y.0,
            ch_col,
            FULL_CROSSHAIR_STROKE_WIDTH_DP,
        );
        stroke_line_ts(
            pixmap,
            scr_pt.x.0,
            0.0,
            scr_pt.x.0,
            h,
            ch_col,
            FULL_CROSSHAIR_STROKE_WIDTH_DP,
        );
    }

    if state.preferences.show_touch_precision_loupe {
        render_touch_precision_loupe_ts(pixmap, state, theme, scr_pt, w, h);
    }
}

fn render_touch_precision_loupe_ts(
    pixmap: &mut Pixmap,
    state: &GrafiaCanvasState,
    theme: &GrafiaTheme,
    scr_pt: ScreenPoint,
    w: f32,
    h: f32,
) {
    let (cx, cy) = compute_loupe_center(scr_pt, 0.0, 0.0, w, h);
    let is_dark = theme.is_dark();
    let fill_col = color32_to_tiny_skia(touch_loupe_fill_color32(is_dark));
    let ring_col = color32_to_tiny_skia(touch_loupe_ring_color32(is_dark));
    fill_circle_ts(pixmap, cx, cy, TOUCH_LOUPE_RADIUS_DP, fill_col);

    let vp = state.active_slot_viewport();
    let stroke_col = color32_to_tiny_skia(theme.preview_canvas.fill_color32());
    let stroke_w = theme.edit_canvas.contour_stroke_width_dp;
    let r_sq = TOUCH_LOUPE_RADIUS_DP * TOUCH_LOUPE_RADIUS_DP;
    let mut pb = PathBuilder::new();
    for bez_path in &state.paths {
        let mut prev: Option<(f32, f32)> = None;
        kurbo::flatten(bez_path.iter(), CURVE_FLATTEN_TOLERANCE, |el| match el {
            PathEl::MoveTo(p) => {
                let s = vp.to_screen(FontPoint::new(p.x, p.y));
                prev = Some((
                    cx + (s.x.0 - scr_pt.x.0) * TOUCH_LOUPE_MAGNIFICATION,
                    cy + (s.y.0 - scr_pt.y.0) * TOUCH_LOUPE_MAGNIFICATION,
                ));
            }
            PathEl::LineTo(p) => {
                let s = vp.to_screen(FontPoint::new(p.x, p.y));
                let nx = cx + (s.x.0 - scr_pt.x.0) * TOUCH_LOUPE_MAGNIFICATION;
                let ny = cy + (s.y.0 - scr_pt.y.0) * TOUCH_LOUPE_MAGNIFICATION;
                if let Some((px, py)) = prev {
                    let da = (px - cx) * (px - cx) + (py - cy) * (py - cy);
                    let db = (nx - cx) * (nx - cx) + (ny - cy) * (ny - cy);
                    if da <= r_sq && db <= r_sq {
                        pb.move_to(px, py);
                        pb.line_to(nx, ny);
                    }
                }
                prev = Some((nx, ny));
            }
            _ => {}
        });
    }
    stroke_ts(pixmap, pb, stroke_col, stroke_w);

    stroke_line_ts(
        pixmap,
        cx - TOUCH_LOUPE_CROSSHAIR_HALF_DP,
        cy,
        cx + TOUCH_LOUPE_CROSSHAIR_HALF_DP,
        cy,
        ring_col,
        TOUCH_LOUPE_CROSSHAIR_WIDTH_DP,
    );
    stroke_line_ts(
        pixmap,
        cx,
        cy - TOUCH_LOUPE_CROSSHAIR_HALF_DP,
        cx,
        cy + TOUCH_LOUPE_CROSSHAIR_HALF_DP,
        ring_col,
        TOUCH_LOUPE_CROSSHAIR_WIDTH_DP,
    );
    let mut ring_pb = PathBuilder::new();
    ring_pb.push_circle(cx, cy, TOUCH_LOUPE_RADIUS_DP);
    stroke_ts(pixmap, ring_pb, ring_col, TOUCH_LOUPE_RING_WIDTH_DP);
}
