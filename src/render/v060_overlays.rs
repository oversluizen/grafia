//! `SPEC-007` (`grafia v0.6.0`) canvas overlays for `egui` and `tiny-skia`:
//! Background Reference Layer contours, 2D Glyph Anchors (`top`, `bottom`, `ogonek`),
//! and 20 dp Top/Left Canvas Rulers (`Cmd+R`) with UPM ticks and live cursor hairlines.

pub use crate::render::v060_overlays_ts::render_v060_overlays_tiny_skia;

use crate::adapters::state::{CanvasViewMode, GrafiaCanvasState};
use crate::core::{extract_segments, ContourSegment, FontPoint, ScreenPoint};
use crate::render::ts_primitives::draw_bitmap_string_ts;
use crate::tokens::{
    color32_to_tiny_skia, GrafiaTheme, ANCHOR_DIAMOND_RADIUS_DP, ANCHOR_DIAMOND_SELECTED_RADIUS_DP,
    ANCHOR_LABEL_FONT_SIZE_DP, ANCHOR_LABEL_OFFSET_DP, BACKGROUND_STROKE_WIDTH_DP,
    CURVE_FLATTEN_TOLERANCE, RULER_CANDIDATE_MAJOR_STEPS_UPM, RULER_LABEL_FONT_SIZE_DP,
    RULER_LABEL_OFFSET_DP, RULER_MAJOR_TICK_LEN_DP, RULER_MINOR_SUBDIVISIONS,
    RULER_MINOR_TICK_LEN_DP, RULER_MIN_MAJOR_SPACING_DP, RULER_MIN_ZOOM_CLAMP, RULER_THICKNESS_DP,
};
use egui::{Align2, FontId, Painter, Pos2, Rect, Shape, Stroke};
use kurbo::{BezPath, Point as KurboPoint};
use tiny_skia::Pixmap;

/// Computes zoom-adaptive `(minor_step_upm, major_step_upm)` on a `1-2-5` scale (`SPEC-007 §2.4`).
#[must_use]
pub fn compute_ruler_tick_steps_upm(zoom: f32) -> (f64, f64) {
    let z = f64::from(if zoom.is_finite() {
        zoom.max(RULER_MIN_ZOOM_CLAMP)
    } else {
        RULER_MIN_ZOOM_CLAMP
    });
    let major = RULER_CANDIDATE_MAJOR_STEPS_UPM
        .iter()
        .copied()
        .find(|&m| m * z >= RULER_MIN_MAJOR_SPACING_DP)
        .unwrap_or_else(|| {
            *RULER_CANDIDATE_MAJOR_STEPS_UPM
                .last()
                .expect("ruler candidates")
        });
    (major / RULER_MINOR_SUBDIVISIONS, major)
}

pub(crate) fn build_background_contour_bezpath(
    nodes: &[crate::core::Node],
    closed: bool,
    vp: &crate::core::ViewportTransform,
) -> BezPath {
    let mut path = BezPath::new();
    if nodes.len() < 2 {
        return path;
    }
    let s0 = vp.to_screen(nodes[0].point);
    path.move_to(KurboPoint::new(f64::from(s0.x.0), f64::from(s0.y.0)));
    for seg in extract_segments(nodes, closed) {
        match seg {
            ContourSegment::Line { end_idx, .. } => {
                let p = vp.to_screen(nodes[end_idx].point);
                path.line_to(KurboPoint::new(f64::from(p.x.0), f64::from(p.y.0)));
            }
            ContourSegment::Cubic {
                h1_idx,
                h2_idx,
                end_idx,
                ..
            } => {
                let (h1, h2, p3) = (
                    vp.to_screen(nodes[h1_idx].point),
                    vp.to_screen(nodes[h2_idx].point),
                    vp.to_screen(nodes[end_idx].point),
                );
                path.curve_to(
                    KurboPoint::new(f64::from(h1.x.0), f64::from(h1.y.0)),
                    KurboPoint::new(f64::from(h2.x.0), f64::from(h2.y.0)),
                    KurboPoint::new(f64::from(p3.x.0), f64::from(p3.y.0)),
                );
            }
        }
    }
    if closed {
        path.close_path();
    }
    path
}

pub(crate) fn draw_bitmap_digits_tiny_skia(
    pixmap: &mut Pixmap,
    x: f32,
    y: f32,
    text: &str,
    col: egui::Color32,
) {
    draw_bitmap_string_ts(pixmap, x, y, text, color32_to_tiny_skia(col));
}

/// Paints background reference layer contours, glyph anchors, and 20 dp rulers in `egui`.
pub fn paint_v060_overlays_egui(
    painter: &Painter,
    state: &GrafiaCanvasState,
    theme: &GrafiaTheme,
    canvas_rect: Rect,
) {
    if state.view_mode != CanvasViewMode::PreviewFilled {
        let active_vp = state.active_slot_viewport();
        if state.show_background && !state.background_contours.is_empty() {
            let stroke = Stroke::new(
                BACKGROUND_STROKE_WIDTH_DP,
                theme.background_layer_stroke_color32(),
            );
            for gc in &state.background_contours {
                if !gc.visible || gc.nodes.len() < 2 {
                    continue;
                }
                let bez = build_background_contour_bezpath(&gc.nodes, gc.closed, &active_vp);
                let mut pts = Vec::new();
                kurbo::flatten(bez.iter(), CURVE_FLATTEN_TOLERANCE, |el| match el {
                    kurbo::PathEl::MoveTo(pt) | kurbo::PathEl::LineTo(pt) => {
                        pts.push(Pos2::new(pt.x as f32, pt.y as f32))
                    }
                    _ => {}
                });
                if pts.len() >= 2 {
                    painter.add(if gc.closed {
                        Shape::closed_line(pts, stroke)
                    } else {
                        Shape::line(pts, stroke)
                    });
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
            let fill = theme.anchor_fill_color32(anchor.selected);
            let diamond = vec![
                Pos2::new(scr.x.0, scr.y.0 - r),
                Pos2::new(scr.x.0 + r, scr.y.0),
                Pos2::new(scr.x.0, scr.y.0 + r),
                Pos2::new(scr.x.0 - r, scr.y.0),
            ];
            painter.add(Shape::convex_polygon(diamond, fill, Stroke::NONE));
            painter.text(
                Pos2::new(
                    scr.x.0 + ANCHOR_LABEL_OFFSET_DP,
                    scr.y.0 - ANCHOR_LABEL_OFFSET_DP,
                ),
                Align2::LEFT_BOTTOM,
                &anchor.name,
                FontId::proportional(ANCHOR_LABEL_FONT_SIZE_DP),
                fill,
            );
        }
    }

    if state.show_rulers {
        paint_v060_rulers_egui(painter, state, theme, canvas_rect);
    }
    crate::render::v070_overlays::paint_v070_overlays_egui(painter, state, theme);
    crate::render::v060_hud::paint_v060_hud_egui(painter, state, theme, canvas_rect);
}

fn paint_v060_rulers_egui(
    painter: &Painter,
    state: &GrafiaCanvasState,
    theme: &GrafiaTheme,
    canvas_rect: Rect,
) {
    let top_rect = Rect::from_min_max(
        canvas_rect.min,
        Pos2::new(canvas_rect.max.x, canvas_rect.min.y + RULER_THICKNESS_DP),
    );
    let left_rect = Rect::from_min_max(
        canvas_rect.min,
        Pos2::new(canvas_rect.min.x + RULER_THICKNESS_DP, canvas_rect.max.y),
    );
    let bg_col = theme.ruler_bg_fill_color32();
    let border_stroke = Stroke::new(1.0, theme.ruler_border_stroke_color32());
    let tick_col = theme.ruler_tick_color32();
    let tick_stroke = Stroke::new(1.0, tick_col);

    painter.rect_filled(top_rect, 0.0, bg_col);
    painter.rect_filled(left_rect, 0.0, bg_col);
    painter.line_segment(
        [top_rect.left_bottom(), top_rect.right_bottom()],
        border_stroke,
    );
    painter.line_segment(
        [left_rect.right_top(), left_rect.right_bottom()],
        border_stroke,
    );

    let f_min = state
        .viewport
        .to_font(ScreenPoint::new(canvas_rect.min.x, canvas_rect.max.y));
    let f_max = state
        .viewport
        .to_font(ScreenPoint::new(canvas_rect.max.x, canvas_rect.min.y));

    let (minor_upm, major_upm) = compute_ruler_tick_steps_upm(state.viewport.zoom);
    let step_i = (minor_upm.round() as i64).max(1);
    let major_i = (major_upm.round() as i64).max(1);
    let start_x = ((f_min.x.0 / minor_upm).floor() as i64) * step_i;
    let end_x = ((f_max.x.0 / minor_upm).ceil() as i64) * step_i;

    let mut ux = start_x;
    while ux <= end_x {
        let sx = state.viewport.to_screen(FontPoint::new(ux as f64, 0.0)).x.0;
        if sx >= canvas_rect.min.x + RULER_THICKNESS_DP && sx <= canvas_rect.max.x {
            let is_major = ux % major_i == 0;
            let len = if is_major {
                RULER_MAJOR_TICK_LEN_DP
            } else {
                RULER_MINOR_TICK_LEN_DP
            };
            painter.line_segment(
                [
                    Pos2::new(sx, top_rect.max.y - len),
                    Pos2::new(sx, top_rect.max.y),
                ],
                tick_stroke,
            );
            if is_major {
                painter.text(
                    Pos2::new(
                        sx + RULER_LABEL_OFFSET_DP,
                        top_rect.min.y + RULER_LABEL_OFFSET_DP,
                    ),
                    Align2::LEFT_TOP,
                    format!("{ux}"),
                    FontId::monospace(RULER_LABEL_FONT_SIZE_DP),
                    tick_col,
                );
            }
        }
        ux += step_i;
    }

    let start_y = ((f_min.y.0 / minor_upm).floor() as i64) * step_i;
    let end_y = ((f_max.y.0 / minor_upm).ceil() as i64) * step_i;
    let mut uy = start_y;
    while uy <= end_y {
        let sy = state.viewport.to_screen(FontPoint::new(0.0, uy as f64)).y.0;
        if sy >= canvas_rect.min.y + RULER_THICKNESS_DP && sy <= canvas_rect.max.y {
            let is_major = uy % major_i == 0;
            let len = if is_major {
                RULER_MAJOR_TICK_LEN_DP
            } else {
                RULER_MINOR_TICK_LEN_DP
            };
            painter.line_segment(
                [
                    Pos2::new(left_rect.max.x - len, sy),
                    Pos2::new(left_rect.max.x, sy),
                ],
                tick_stroke,
            );
            if is_major {
                painter.text(
                    Pos2::new(
                        left_rect.min.x + RULER_LABEL_OFFSET_DP,
                        sy + RULER_LABEL_OFFSET_DP,
                    ),
                    Align2::LEFT_TOP,
                    format!("{uy}"),
                    FontId::monospace(RULER_LABEL_FONT_SIZE_DP),
                    tick_col,
                );
            }
        }
        uy += step_i;
    }

    if let Some(cur) = state.last_pointer_screen_pos.or(state.cursor_screen_pos) {
        let h_stroke = Stroke::new(1.0, theme.ruler_hairline_color32());
        if cur.x.0 >= canvas_rect.min.x + RULER_THICKNESS_DP && cur.x.0 <= canvas_rect.max.x {
            painter.line_segment(
                [
                    Pos2::new(cur.x.0, top_rect.min.y),
                    Pos2::new(cur.x.0, top_rect.max.y),
                ],
                h_stroke,
            );
        }
        if cur.y.0 >= canvas_rect.min.y + RULER_THICKNESS_DP && cur.y.0 <= canvas_rect.max.y {
            painter.line_segment(
                [
                    Pos2::new(left_rect.min.x, cur.y.0),
                    Pos2::new(left_rect.max.x, cur.y.0),
                ],
                h_stroke,
            );
        }
    }
}
