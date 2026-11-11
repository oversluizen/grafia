//! `SPEC-007` (`grafia v0.6.0`) `tiny-skia` overlay renderers for background reference contours,
//! 2D glyph anchors, and 20 dp top/left canvas rulers.

use crate::adapters::headless::kurbo_to_tiny_skia_path;
use crate::adapters::state::{CanvasViewMode, GrafiaCanvasState};
use crate::core::{FontPoint, ScreenPoint};
use crate::render::v060_overlays::{
    build_background_contour_bezpath, compute_ruler_tick_steps_upm, draw_bitmap_digits_tiny_skia,
};
use crate::tokens::{
    color32_to_tiny_skia, GrafiaTheme, ANCHOR_DIAMOND_RADIUS_DP, ANCHOR_DIAMOND_SELECTED_RADIUS_DP,
    ANCHOR_LABEL_OFFSET_DP, BACKGROUND_STROKE_WIDTH_DP, PILL_DIGIT_FONT_HEIGHT_PX,
    RULER_LABEL_OFFSET_DP, RULER_MAJOR_TICK_LEN_DP, RULER_MINOR_TICK_LEN_DP, RULER_THICKNESS_DP,
};
use tiny_skia::{
    FillRule, Paint, PathBuilder, Pixmap, Rect as TsRect, Stroke as TsStroke, Transform,
};

/// Renders background reference layer contours, glyph anchors, and 20 dp rulers in `tiny-skia`.
pub fn render_v060_overlays_tiny_skia(
    pixmap: &mut Pixmap,
    state: &GrafiaCanvasState,
    theme: &GrafiaTheme,
    rect_x: f32,
    rect_y: f32,
    rect_w: f32,
    rect_h: f32,
) {
    if state.view_mode != CanvasViewMode::PreviewFilled {
        let active_vp = state.active_slot_viewport();
        if state.show_background && !state.background_contours.is_empty() {
            let mut paint = Paint::default();
            paint.set_color(color32_to_tiny_skia(
                theme.background_layer_stroke_color32(),
            ));
            paint.anti_alias = true;
            let stroke = TsStroke {
                width: BACKGROUND_STROKE_WIDTH_DP,
                ..Default::default()
            };
            for gc in &state.background_contours {
                if !gc.visible || gc.nodes.len() < 2 {
                    continue;
                }
                let bez = build_background_contour_bezpath(&gc.nodes, gc.closed, &active_vp);
                if let Some(ts_path) = kurbo_to_tiny_skia_path(&bez) {
                    pixmap.stroke_path(&ts_path, &paint, &stroke, Transform::identity(), None);
                }
            }
        }

        for anchor in &state.anchors {
            let scr = active_vp.to_screen(anchor.position);
            let r = if anchor.selected {
                ANCHOR_DIAMOND_SELECTED_RADIUS_DP
            } else {
                ANCHOR_DIAMOND_RADIUS_DP
            };
            let col = theme.anchor_fill_color32(anchor.selected);
            let mut pb = PathBuilder::new();
            pb.move_to(scr.x.0, scr.y.0 - r);
            pb.line_to(scr.x.0 + r, scr.y.0);
            pb.line_to(scr.x.0, scr.y.0 + r);
            pb.line_to(scr.x.0 - r, scr.y.0);
            pb.close();
            if let Some(path) = pb.finish() {
                let mut paint = Paint::default();
                paint.set_color(color32_to_tiny_skia(col));
                paint.anti_alias = true;
                pixmap.fill_path(
                    &path,
                    &paint,
                    FillRule::Winding,
                    Transform::identity(),
                    None,
                );
                draw_bitmap_digits_tiny_skia(
                    pixmap,
                    scr.x.0 + ANCHOR_LABEL_OFFSET_DP,
                    scr.y.0 - ANCHOR_LABEL_OFFSET_DP,
                    &anchor.name,
                    col,
                );
            }
        }
    }

    if state.show_rulers {
        render_v060_rulers_tiny_skia(pixmap, state, theme, rect_x, rect_y, rect_w, rect_h);
    }
    crate::render::v070_overlays::render_v070_overlays_tiny_skia(pixmap, state, theme);
    crate::render::v060_hud::render_v060_hud_tiny_skia(
        pixmap, state, theme, rect_x, rect_y, rect_w, rect_h,
    );
}

fn render_v060_rulers_tiny_skia(
    pixmap: &mut Pixmap,
    state: &GrafiaCanvasState,
    theme: &GrafiaTheme,
    rect_x: f32,
    rect_y: f32,
    rect_w: f32,
    rect_h: f32,
) {
    let mut bg_paint = Paint::default();
    bg_paint.set_color(color32_to_tiny_skia(theme.ruler_bg_fill_color32()));
    if let Some(top_r) = TsRect::from_xywh(rect_x, rect_y, rect_w, RULER_THICKNESS_DP) {
        pixmap.fill_rect(top_r, &bg_paint, Transform::identity(), None);
    }
    if let Some(left_r) = TsRect::from_xywh(rect_x, rect_y, RULER_THICKNESS_DP, rect_h) {
        pixmap.fill_rect(left_r, &bg_paint, Transform::identity(), None);
    }

    let mut border_paint = Paint::default();
    border_paint.set_color(color32_to_tiny_skia(theme.ruler_border_stroke_color32()));
    let stroke_1 = TsStroke {
        width: 1.0,
        ..Default::default()
    };
    let mut pb = PathBuilder::new();
    pb.move_to(rect_x, rect_y + RULER_THICKNESS_DP);
    pb.line_to(rect_x + rect_w, rect_y + RULER_THICKNESS_DP);
    pb.move_to(rect_x + RULER_THICKNESS_DP, rect_y);
    pb.line_to(rect_x + RULER_THICKNESS_DP, rect_y + rect_h);
    if let Some(border_path) = pb.finish() {
        pixmap.stroke_path(
            &border_path,
            &border_paint,
            &stroke_1,
            Transform::identity(),
            None,
        );
    }

    let tick_col32 = theme.ruler_tick_color32();
    let mut tick_paint = Paint::default();
    tick_paint.set_color(color32_to_tiny_skia(tick_col32));
    let mut tick_pb = PathBuilder::new();

    let f_min = state
        .viewport
        .to_font(ScreenPoint::new(rect_x, rect_y + rect_h));
    let f_max = state
        .viewport
        .to_font(ScreenPoint::new(rect_x + rect_w, rect_y));
    let (minor_upm, major_upm) = compute_ruler_tick_steps_upm(state.viewport.zoom);
    let step_i = (minor_upm.round() as i64).max(1);
    let major_i = (major_upm.round() as i64).max(1);

    let start_x = ((f_min.x.0 / minor_upm).floor() as i64) * step_i;
    let end_x = ((f_max.x.0 / minor_upm).ceil() as i64) * step_i;
    let mut ux = start_x;
    while ux <= end_x {
        let sx = rect_x + state.viewport.to_screen(FontPoint::new(ux as f64, 0.0)).x.0;
        if sx >= rect_x + RULER_THICKNESS_DP && sx <= rect_x + rect_w {
            let is_major = ux % major_i == 0;
            let len = if is_major {
                RULER_MAJOR_TICK_LEN_DP
            } else {
                RULER_MINOR_TICK_LEN_DP
            };
            tick_pb.move_to(sx, rect_y + RULER_THICKNESS_DP - len);
            tick_pb.line_to(sx, rect_y + RULER_THICKNESS_DP);
            if is_major {
                draw_bitmap_digits_tiny_skia(
                    pixmap,
                    sx + RULER_LABEL_OFFSET_DP,
                    rect_y + RULER_LABEL_OFFSET_DP,
                    &format!("{ux}"),
                    tick_col32,
                );
            }
        }
        ux += step_i;
    }

    let start_y = ((f_min.y.0 / minor_upm).floor() as i64) * step_i;
    let end_y = ((f_max.y.0 / minor_upm).ceil() as i64) * step_i;
    let mut uy = start_y;
    while uy <= end_y {
        let sy = rect_y + state.viewport.to_screen(FontPoint::new(0.0, uy as f64)).y.0;
        if sy >= rect_y + RULER_THICKNESS_DP && sy <= rect_y + rect_h {
            let is_major = uy % major_i == 0;
            let len = if is_major {
                RULER_MAJOR_TICK_LEN_DP
            } else {
                RULER_MINOR_TICK_LEN_DP
            };
            tick_pb.move_to(rect_x + RULER_THICKNESS_DP - len, sy);
            tick_pb.line_to(rect_x + RULER_THICKNESS_DP, sy);
            if is_major && sy + (PILL_DIGIT_FONT_HEIGHT_PX as f32) < rect_y + rect_h {
                draw_bitmap_digits_tiny_skia(
                    pixmap,
                    rect_x + RULER_LABEL_OFFSET_DP,
                    sy + RULER_LABEL_OFFSET_DP,
                    &format!("{uy}"),
                    tick_col32,
                );
            }
        }
        uy += step_i;
    }
    if let Some(tick_path) = tick_pb.finish() {
        pixmap.stroke_path(
            &tick_path,
            &tick_paint,
            &stroke_1,
            Transform::identity(),
            None,
        );
    }
    if let Some(cur) = state.last_pointer_screen_pos.or(state.cursor_screen_pos) {
        let mut h_paint = Paint::default();
        h_paint.set_color(color32_to_tiny_skia(theme.ruler_hairline_color32()));
        let mut h_pb = PathBuilder::new();
        let (cx, cy) = (rect_x + cur.x.0, rect_y + cur.y.0);
        if cx >= rect_x + RULER_THICKNESS_DP && cx <= rect_x + rect_w {
            h_pb.move_to(cx, rect_y);
            h_pb.line_to(cx, rect_y + RULER_THICKNESS_DP);
        }
        if cy >= rect_y + RULER_THICKNESS_DP && cy <= rect_y + rect_h {
            h_pb.move_to(rect_x, cy);
            h_pb.line_to(rect_x + RULER_THICKNESS_DP, cy);
        }
        if let Some(h_path) = h_pb.finish() {
            pixmap.stroke_path(&h_path, &h_paint, &stroke_1, Transform::identity(), None);
        }
    }
}
