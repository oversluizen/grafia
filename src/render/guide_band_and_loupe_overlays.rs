//! `SPEC-009` (`grafia v0.8.0` Part B) `egui::Painter` visual primitives:
//! - `GuideThickness::FontUnits(f64)` zoom-proportional translucent guide bands + clipped rays
//! - `OffCurveHandleShape` (`Circle`, `Square`, `Cross`) rendering
//! - `full_crosshair_cursor` full-canvas hairlines
//! - `show_touch_precision_loupe` circular `2.5x` crosshair magnifier above touch/pointer contact

pub use crate::render::loupe_pixmap_pass::{
    render_edit_active_fill_tiny_skia, render_guide_band_tiny_skia,
    render_offcurve_handle_tiny_skia, render_top_cursor_and_loupe_overlays_tiny_skia,
};

use crate::adapters::state::GrafiaCanvasState;
use crate::core::{FontPoint, ScreenPoint};
use crate::snapping::{Guide, GuideThickness};
use crate::tokens::{
    full_crosshair_color32, guide_band_fill_color32, touch_loupe_fill_color32,
    touch_loupe_ring_color32, GrafiaTheme, OffCurveHandleShape, CURVE_FLATTEN_TOLERANCE,
    FULL_CROSSHAIR_STROKE_WIDTH_DP, GUIDE_CLIP_MARGIN_DP, GUIDE_CLIP_RAY_EPS_DP,
    GUIDE_SELECTED_LINE_STROKE_WIDTH_DP, OFFCURVE_CROSS_ARM_SCALE, OFFCURVE_CROSS_STROKE_WIDTH_DP,
    STROKE_WIDTH_MAX_DP, STROKE_WIDTH_MIN_DP, TOUCH_LOUPE_CENTER_LIFT_DP,
    TOUCH_LOUPE_CROSSHAIR_HALF_DP, TOUCH_LOUPE_CROSSHAIR_WIDTH_DP, TOUCH_LOUPE_MAGNIFICATION,
    TOUCH_LOUPE_RADIUS_DP, TOUCH_LOUPE_RING_WIDTH_DP,
};
use egui::epaint::{PathShape, PathStroke};
use egui::{Color32, Painter, Pos2, Rect, Shape, Stroke};
use kurbo::PathEl;

pub(crate) fn effective_guide_line_width_dp(state: &GrafiaCanvasState, is_selected: bool) -> f32 {
    let base = state
        .theme
        .edit_canvas
        .guide_stroke_width_dp
        .clamp(STROKE_WIDTH_MIN_DP, STROKE_WIDTH_MAX_DP);
    if is_selected {
        base.max(GUIDE_SELECTED_LINE_STROKE_WIDTH_DP)
    } else {
        base
    }
}

pub(crate) fn clip_infinite_guide_screen_segment(
    guide: &Guide,
    state: &GrafiaCanvasState,
    min_x: f32,
    min_y: f32,
    max_x: f32,
    max_y: f32,
) -> Option<(ScreenPoint, ScreenPoint)> {
    let left = min_x - GUIDE_CLIP_MARGIN_DP;
    let right = max_x + GUIDE_CLIP_MARGIN_DP;
    let top = min_y - GUIDE_CLIP_MARGIN_DP;
    let bottom = max_y + GUIDE_CLIP_MARGIN_DP;
    let p = state.viewport.to_screen(guide.position);
    let (ux, uy) = guide.direction_unit();
    let dx = ux as f32;
    let dy = -(uy as f32);
    let len = dx.hypot(dy);
    if len <= GUIDE_CLIP_RAY_EPS_DP {
        return None;
    }
    let ux_s = dx / len;
    let uy_s = dy / len;

    let mut t_min = f32::NEG_INFINITY;
    let mut t_max = f32::INFINITY;
    for (dir, origin, lo, hi) in [(ux_s, p.x.0, left, right), (uy_s, p.y.0, top, bottom)] {
        if dir.abs() <= GUIDE_CLIP_RAY_EPS_DP {
            if origin < lo || origin > hi {
                return None;
            }
        } else {
            let t1 = (lo - origin) / dir;
            let t2 = (hi - origin) / dir;
            t_min = t_min.max(t1.min(t2));
            t_max = t_max.min(t1.max(t2));
        }
    }
    if t_min > t_max || !t_min.is_finite() || !t_max.is_finite() {
        return None;
    }
    Some((
        ScreenPoint::new(p.x.0 + t_min * ux_s, p.y.0 + t_min * uy_s),
        ScreenPoint::new(p.x.0 + t_max * ux_s, p.y.0 + t_max * uy_s),
    ))
}

pub(crate) fn guide_band_screen_quad(
    guide: &Guide,
    state: &GrafiaCanvasState,
    s0: ScreenPoint,
    s1: ScreenPoint,
) -> Option<[ScreenPoint; 4]> {
    let GuideThickness::FontUnits(width_upm) = guide.thickness else {
        return None;
    };
    if !width_upm.is_finite() || width_upm <= 0.0 {
        return None;
    }
    let half_dp = (width_upm as f32) * 0.5 * state.viewport.zoom;
    if half_dp <= 0.0 {
        return None;
    }
    let vx = s1.x.0 - s0.x.0;
    let vy = s1.y.0 - s0.y.0;
    let len = vx.hypot(vy);
    if len <= GUIDE_CLIP_RAY_EPS_DP {
        return None;
    }
    let nx = (-vy / len) * half_dp;
    let ny = (vx / len) * half_dp;
    Some([
        ScreenPoint::new(s0.x.0 + nx, s0.y.0 + ny),
        ScreenPoint::new(s1.x.0 + nx, s1.y.0 + ny),
        ScreenPoint::new(s1.x.0 - nx, s1.y.0 - ny),
        ScreenPoint::new(s0.x.0 - nx, s0.y.0 - ny),
    ])
}

pub fn paint_guide_band_egui(
    painter: &Painter,
    guide: &Guide,
    state: &GrafiaCanvasState,
    s0: ScreenPoint,
    s1: ScreenPoint,
    guide_col: Color32,
) {
    let Some(quad) = guide_band_screen_quad(guide, state, s0, s1) else {
        return;
    };
    let fill = guide_band_fill_color32(guide_col);
    let pts: Vec<Pos2> = quad.iter().map(|p| Pos2::new(p.x.0, p.y.0)).collect();
    painter.add(Shape::Path(PathShape {
        points: pts,
        closed: true,
        fill,
        stroke: PathStroke::NONE,
    }));
}

pub fn paint_offcurve_handle_egui(
    painter: &Painter,
    center: Pos2,
    radius_dp: f32,
    shape: OffCurveHandleShape,
    color: Color32,
) {
    match shape {
        OffCurveHandleShape::Circle => {
            painter.circle_filled(center, radius_dp, color);
        }
        OffCurveHandleShape::Square => {
            let rect = Rect::from_min_max(
                Pos2::new(center.x - radius_dp, center.y - radius_dp),
                Pos2::new(center.x + radius_dp, center.y + radius_dp),
            );
            painter.rect_filled(rect, 0.0, color);
        }
        OffCurveHandleShape::Cross => {
            let arm = radius_dp * OFFCURVE_CROSS_ARM_SCALE * 2.0;
            let stroke = Stroke::new(OFFCURVE_CROSS_STROKE_WIDTH_DP, color);
            painter.line_segment(
                [
                    Pos2::new(center.x - arm, center.y - arm),
                    Pos2::new(center.x + arm, center.y + arm),
                ],
                stroke,
            );
            painter.line_segment(
                [
                    Pos2::new(center.x - arm, center.y + arm),
                    Pos2::new(center.x + arm, center.y - arm),
                ],
                stroke,
            );
        }
    }
}

pub(crate) fn compute_loupe_center(
    scr_pt: ScreenPoint,
    min_x: f32,
    min_y: f32,
    max_x: f32,
    max_y: f32,
) -> (f32, f32) {
    let pad = TOUCH_LOUPE_RADIUS_DP + TOUCH_LOUPE_RING_WIDTH_DP;
    let cx = scr_pt
        .x
        .0
        .clamp(min_x + pad, (max_x - pad).max(min_x + pad));
    let lifted_y = scr_pt.y.0 - TOUCH_LOUPE_CENTER_LIFT_DP;
    let cy = if lifted_y >= min_y + pad {
        lifted_y
    } else {
        (scr_pt.y.0 + TOUCH_LOUPE_CENTER_LIFT_DP).min((max_y - pad).max(min_y + pad))
    };
    (cx, cy)
}

pub fn paint_top_cursor_and_loupe_overlays_egui(
    painter: &Painter,
    canvas_rect: Rect,
    state: &GrafiaCanvasState,
    theme: &GrafiaTheme,
) {
    let Some(scr_pt) = state.cursor_screen_pos else {
        return;
    };
    let pos = Pos2::new(scr_pt.x.0, scr_pt.y.0);
    if !canvas_rect.contains(pos) {
        return;
    }

    if theme.edit_canvas.full_crosshair_cursor {
        let stroke = Stroke::new(
            FULL_CROSSHAIR_STROKE_WIDTH_DP,
            full_crosshair_color32(theme.preview_canvas.fill_color32()),
        );
        painter.line_segment(
            [
                Pos2::new(canvas_rect.left(), pos.y),
                Pos2::new(canvas_rect.right(), pos.y),
            ],
            stroke,
        );
        painter.line_segment(
            [
                Pos2::new(pos.x, canvas_rect.top()),
                Pos2::new(pos.x, canvas_rect.bottom()),
            ],
            stroke,
        );
    }

    if state.preferences.show_touch_precision_loupe {
        paint_touch_precision_loupe_egui(painter, canvas_rect, state, theme, scr_pt);
    }
}

fn paint_touch_precision_loupe_egui(
    painter: &Painter,
    canvas_rect: Rect,
    state: &GrafiaCanvasState,
    theme: &GrafiaTheme,
    scr_pt: ScreenPoint,
) {
    let (cx, cy) = compute_loupe_center(
        scr_pt,
        canvas_rect.left(),
        canvas_rect.top(),
        canvas_rect.right(),
        canvas_rect.bottom(),
    );
    let center = Pos2::new(cx, cy);
    let is_dark = theme.is_dark();
    let fill_col = touch_loupe_fill_color32(is_dark);
    let ring_col = touch_loupe_ring_color32(is_dark);
    painter.circle_filled(center, TOUCH_LOUPE_RADIUS_DP, fill_col);

    let vp = state.active_slot_viewport();
    let contour_stroke = Stroke::new(
        theme.edit_canvas.contour_stroke_width_dp,
        theme.preview_canvas.fill_color32(),
    );
    let r_sq = TOUCH_LOUPE_RADIUS_DP * TOUCH_LOUPE_RADIUS_DP;
    for bez_path in &state.paths {
        let mut prev: Option<Pos2> = None;
        kurbo::flatten(bez_path.iter(), CURVE_FLATTEN_TOLERANCE, |el| match el {
            PathEl::MoveTo(p) => {
                let s = vp.to_screen(FontPoint::new(p.x, p.y));
                prev = Some(Pos2::new(
                    cx + (s.x.0 - scr_pt.x.0) * TOUCH_LOUPE_MAGNIFICATION,
                    cy + (s.y.0 - scr_pt.y.0) * TOUCH_LOUPE_MAGNIFICATION,
                ));
            }
            PathEl::LineTo(p) => {
                let s = vp.to_screen(FontPoint::new(p.x, p.y));
                let next = Pos2::new(
                    cx + (s.x.0 - scr_pt.x.0) * TOUCH_LOUPE_MAGNIFICATION,
                    cy + (s.y.0 - scr_pt.y.0) * TOUCH_LOUPE_MAGNIFICATION,
                );
                if let Some(pa) = prev {
                    let da = (pa.x - cx) * (pa.x - cx) + (pa.y - cy) * (pa.y - cy);
                    let db = (next.x - cx) * (next.x - cx) + (next.y - cy) * (next.y - cy);
                    if da <= r_sq && db <= r_sq {
                        painter.line_segment([pa, next], contour_stroke);
                    }
                }
                prev = Some(next);
            }
            _ => {}
        });
    }

    let ch_stroke = Stroke::new(TOUCH_LOUPE_CROSSHAIR_WIDTH_DP, ring_col);
    painter.line_segment(
        [
            Pos2::new(cx - TOUCH_LOUPE_CROSSHAIR_HALF_DP, cy),
            Pos2::new(cx + TOUCH_LOUPE_CROSSHAIR_HALF_DP, cy),
        ],
        ch_stroke,
    );
    painter.line_segment(
        [
            Pos2::new(cx, cy - TOUCH_LOUPE_CROSSHAIR_HALF_DP),
            Pos2::new(cx, cy + TOUCH_LOUPE_CROSSHAIR_HALF_DP),
        ],
        ch_stroke,
    );
    painter.circle_stroke(
        center,
        TOUCH_LOUPE_RADIUS_DP,
        Stroke::new(TOUCH_LOUPE_RING_WIDTH_DP, ring_col),
    );
}
