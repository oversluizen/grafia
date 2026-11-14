//! `SPEC-007` (`grafia v0.6.0`) canvas overlays for `egui` and `tiny-skia`:
//! Background Reference Layer contours, 2D Glyph Anchors (`top`, `bottom`, `ogonek`),
//! and 20 dp Top/Left Canvas Rulers (`Cmd+R`) with UPM ticks and live cursor hairlines.

pub use crate::render::rulers_pixmap_pass::render_ruler_and_background_overlays_tiny_skia;

use crate::adapters::state::{CanvasViewMode, GrafiaCanvasState};
use crate::core::{FontPoint, ScreenPoint};
use crate::render::ts_primitives::draw_bitmap_string_ts;
use crate::tokens::{
    color32_to_tiny_skia, GrafiaTheme, ANCHOR_DIAMOND_RADIUS_DP, ANCHOR_DIAMOND_SELECTED_RADIUS_DP,
    ANCHOR_LABEL_FONT_SIZE_DP, ANCHOR_LABEL_OFFSET_DP, BACKGROUND_STROKE_WIDTH_DP,
    CURVE_FLATTEN_TOLERANCE, RULER_CANDIDATE_MAJOR_STEPS_UPM, RULER_LABEL_FONT_SIZE_DP,
    RULER_LABEL_OFFSET_DP, RULER_MAJOR_TICK_LEN_DP, RULER_MINOR_SUBDIVISIONS,
    RULER_MINOR_TICK_LEN_DP, RULER_MIN_MAJOR_SPACING_DP, RULER_MIN_ZOOM_CLAMP,
};
use egui::{Align2, FontId, Painter, Pos2, Rect, Shape, Stroke};
use kurbo::BezPath;
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
        .unwrap_or(RULER_CANDIDATE_MAJOR_STEPS_UPM[RULER_CANDIDATE_MAJOR_STEPS_UPM.len() - 1]);
    (major / RULER_MINOR_SUBDIVISIONS, major)
}

pub(crate) fn build_background_contour_bezpath(
    nodes: &[crate::core::Node],
    closed: bool,
    vp: &crate::core::ViewportTransform,
) -> BezPath {
    if nodes.len() < 2 {
        return BezPath::new();
    }
    let upm_path = crate::core::contour_to_bezpath_with_splines(nodes, closed, 0.0, &[]);
    let affine =
        kurbo::Affine::translate((f64::from(vp.pan_offset.x.0), f64::from(vp.pan_offset.y.0)))
            * kurbo::Affine::scale_non_uniform(f64::from(vp.zoom), -f64::from(vp.zoom));
    affine * upm_path
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
pub fn paint_ruler_and_background_overlays_egui(
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
        paint_canvas_rulers_egui(painter, state, theme, canvas_rect);
    }
    crate::render::component_and_stroke_overlays::paint_component_and_stroke_overlays_egui(
        painter, state, theme,
    );
    crate::render::selection_info_hud::paint_selection_info_hud_egui(
        painter,
        state,
        theme,
        canvas_rect,
    );
}

/// UPM tick range and minor/major step intervals for horizontal and vertical ruler rendering (`SPEC-014 §3.2`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RulerTickRangeUpm {
    pub start_step: i64,
    pub end_step: i64,
    pub minor_step_upm: i64,
    pub major_step_upm: i64,
}

fn paint_canvas_rulers_egui(
    painter: &Painter,
    state: &GrafiaCanvasState,
    theme: &GrafiaTheme,
    canvas_rect: Rect,
) {
    let thickness = theme.ruler_tokens.thickness_dp.max(1.0);
    let top_rect = Rect::from_min_max(
        canvas_rect.min,
        Pos2::new(canvas_rect.max.x, canvas_rect.min.y + thickness),
    );
    let left_rect = Rect::from_min_max(
        canvas_rect.min,
        Pos2::new(canvas_rect.min.x + thickness, canvas_rect.max.y),
    );
    let bg_col = theme.ruler_bg_fill_color32();
    let border_stroke = Stroke::new(1.0, theme.ruler_border_stroke_color32());
    let tick_col = theme.ruler_tick_color32();

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
    let x_range = RulerTickRangeUpm {
        start_step: ((f_min.x.0 / minor_upm).floor() as i64) * step_i,
        end_step: ((f_max.x.0 / minor_upm).ceil() as i64) * step_i,
        minor_step_upm: step_i,
        major_step_upm: major_i,
    };
    paint_ruler_x_ticks_egui(painter, state, canvas_rect, top_rect, x_range, tick_col);

    let y_range = RulerTickRangeUpm {
        start_step: ((f_min.y.0 / minor_upm).floor() as i64) * step_i,
        end_step: ((f_max.y.0 / minor_upm).ceil() as i64) * step_i,
        minor_step_upm: step_i,
        major_step_upm: major_i,
    };
    paint_ruler_y_ticks_egui(painter, state, canvas_rect, left_rect, y_range, tick_col);

    if let Some(cur) = state.last_pointer_screen_pos.or(state.cursor_screen_pos) {
        let h_stroke = Stroke::new(1.0, theme.ruler_hairline_color32());
        if cur.x.0 >= canvas_rect.min.x + thickness && cur.x.0 <= canvas_rect.max.x {
            let pts = [
                Pos2::new(cur.x.0, top_rect.min.y),
                Pos2::new(cur.x.0, top_rect.max.y),
            ];
            painter.line_segment(pts, h_stroke);
        }
        if cur.y.0 >= canvas_rect.min.y + thickness && cur.y.0 <= canvas_rect.max.y {
            let pts = [
                Pos2::new(left_rect.min.x, cur.y.0),
                Pos2::new(left_rect.max.x, cur.y.0),
            ];
            painter.line_segment(pts, h_stroke);
        }
    }
}

fn paint_ruler_x_ticks_egui(
    painter: &Painter,
    state: &GrafiaCanvasState,
    canvas_rect: Rect,
    top_rect: Rect,
    range: RulerTickRangeUpm,
    tick_col: egui::Color32,
) {
    let RulerTickRangeUpm {
        start_step: start_x,
        end_step: end_x,
        minor_step_upm: step_i,
        major_step_upm: major_i,
    } = range;
    let thickness = top_rect.height().max(1.0);
    let tick_stroke = Stroke::new(1.0, tick_col);
    let mut ux = start_x;
    while ux <= end_x {
        let sx = state.viewport.to_screen(FontPoint::new(ux as f64, 0.0)).x.0;
        if sx >= canvas_rect.min.x + thickness && sx <= canvas_rect.max.x {
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
}

fn paint_ruler_y_ticks_egui(
    painter: &Painter,
    state: &GrafiaCanvasState,
    canvas_rect: Rect,
    left_rect: Rect,
    range: RulerTickRangeUpm,
    tick_col: egui::Color32,
) {
    let RulerTickRangeUpm {
        start_step: start_y,
        end_step: end_y,
        minor_step_upm: step_i,
        major_step_upm: major_i,
    } = range;
    let thickness = left_rect.width().max(1.0);
    let tick_stroke = Stroke::new(1.0, tick_col);
    let mut uy = start_y;
    while uy <= end_y {
        let sy = state.viewport.to_screen(FontPoint::new(0.0, uy as f64)).y.0;
        if sy >= canvas_rect.min.y + thickness && sy <= canvas_rect.max.y {
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
}
