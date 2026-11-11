//! `SPEC-004 §2.5` (`glifo v0.3.0`) Canvas Grid (`PathRenderer::drawGrid` @ `spec_ref` / `arm64 spec_ref`
//! & `FontMasterMetrics::roundingSettings` @ `spec_ref` / `arm64 spec_ref`):
//! - Subdivision grid (`grid_sub_color = #40595940`, `0.4 dp` orthogonal / `0.6 dp` sloped) when
//!   `show_grid && grid_subdivisions >= 2 && sub_step_px > 4.0 dp`.
//! - Main grid (`grid_main_color = #80736666`, `0.5 dp` orthogonal / `0.7 dp` sloped) when
//!   `show_grid && main_step_px > 8.0 dp`.

use crate::adapters::state::GlifoCanvasState;
use crate::core::{FontPoint, ScreenPoint};
use crate::tokens::{
    GRID_MAIN_LINE_WIDTH_DP, GRID_MAIN_MIN_STEP_DP, GRID_MAIN_SLOPED_LINE_WIDTH_DP,
    GRID_MAX_LINES_PER_AXIS, GRID_MAX_SLOPE_COMPONENT, GRID_MIN_SUBDIVISIONS,
    GRID_SUB_LINE_WIDTH_DP, GRID_SUB_MIN_STEP_DP, GRID_SUB_SLOPED_LINE_WIDTH_DP,
};
use egui::{Painter, Pos2, Rect, Stroke};
use tiny_skia::{Paint, PathBuilder, Pixmap, Stroke as TsStroke, Transform};

fn slope_shear_dx_per_dy(state: &GlifoCanvasState) -> Option<f64> {
    if state.grid_slope_rise > 0
        && state.grid_slope_rise < GRID_MAX_SLOPE_COMPONENT
        && state.grid_slope_run != 0
        && state.grid_slope_run.abs() < GRID_MAX_SLOPE_COMPONENT
    {
        Some(f64::from(state.grid_slope_run) / f64::from(state.grid_slope_rise))
    } else {
        None
    }
}

fn collect_grid_lines_for_step(
    state: &GlifoCanvasState,
    step_upm: f64,
    bounds: Rect,
    horiz_out: &mut Vec<(ScreenPoint, ScreenPoint)>,
    vert_out: &mut Vec<(ScreenPoint, ScreenPoint)>,
) {
    if step_upm <= 0.0 || !step_upm.is_finite() {
        return;
    }
    let (min_scr_x, min_scr_y) = (bounds.left(), bounds.top());
    let (max_scr_x, max_scr_y) = (bounds.right(), bounds.bottom());
    let tl_font = state.viewport.to_font(ScreenPoint::new(min_scr_x, min_scr_y));
    let br_font = state.viewport.to_font(ScreenPoint::new(max_scr_x, max_scr_y));
    let y_min = tl_font.y.0.min(br_font.y.0);
    let y_max = tl_font.y.0.max(br_font.y.0);
    let shear = slope_shear_dx_per_dy(state).unwrap_or(0.0);
    let max_abs_y = y_min.abs().max(y_max.abs());
    let shear_pad = shear.abs() * max_abs_y;
    let x_min = tl_font.x.0.min(br_font.x.0) - shear_pad;
    let x_max = tl_font.x.0.max(br_font.x.0) + shear_pad;

    let ky_start = (y_min / step_upm).floor() as i64;
    let ky_end = (y_max / step_upm).ceil() as i64;
    if (ky_end - ky_start) <= GRID_MAX_LINES_PER_AXIS {
        for ky in ky_start..=ky_end {
            let y_upm = (ky as f64) * step_upm;
            let sy = state.viewport.to_screen(FontPoint::new(0.0, y_upm)).y.0;
            if sy >= min_scr_y && sy <= max_scr_y {
                horiz_out.push((
                    ScreenPoint::new(min_scr_x, sy),
                    ScreenPoint::new(max_scr_x, sy),
                ));
            }
        }
    }

    let kx_start = (x_min / step_upm).floor() as i64;
    let kx_end = (x_max / step_upm).ceil() as i64;
    if (kx_end - kx_start) <= GRID_MAX_LINES_PER_AXIS {
        for kx in kx_start..=kx_end {
            let x0 = (kx as f64) * step_upm;
            let p_top = state
                .viewport
                .to_screen(FontPoint::new(x0 + y_max * shear, y_max));
            let p_bot = state
                .viewport
                .to_screen(FontPoint::new(x0 + y_min * shear, y_min));
            vert_out.push((p_top, p_bot));
        }
    }
}

pub fn paint_v030_grid_egui(painter: &Painter, canvas_rect: Rect, state: &GlifoCanvasState) {
    if !state.show_grid || state.grid_step_upm <= 0.0 {
        return;
    }
    let zoom = f64::from(state.viewport.zoom);
    let main_step_upm = state.grid_step_upm;
    let subdiv = state.grid_subdivisions.max(1);
    let sub_step_upm = main_step_upm / f64::from(subdiv);
    let draw_sub = subdiv >= GRID_MIN_SUBDIVISIONS && (sub_step_upm * zoom) > GRID_SUB_MIN_STEP_DP;
    let draw_main = (main_step_upm * zoom) > GRID_MAIN_MIN_STEP_DP;
    let is_sloped = slope_shear_dx_per_dy(state).is_some();

    if draw_sub {
        let mut h_lines = Vec::new();
        let mut v_lines = Vec::new();
        collect_grid_lines_for_step(state, sub_step_upm, canvas_rect, &mut h_lines, &mut v_lines);
        let col = state.theme.edit_canvas.grid_sub_color.to_color32();
        let h_stroke = Stroke::new(GRID_SUB_LINE_WIDTH_DP, col);
        let v_stroke = Stroke::new(
            if is_sloped { GRID_SUB_SLOPED_LINE_WIDTH_DP } else { GRID_SUB_LINE_WIDTH_DP },
            col,
        );
        for (a, b) in h_lines {
            painter.line_segment([Pos2::new(a.x.0, a.y.0), Pos2::new(b.x.0, b.y.0)], h_stroke);
        }
        for (a, b) in v_lines {
            painter.line_segment([Pos2::new(a.x.0, a.y.0), Pos2::new(b.x.0, b.y.0)], v_stroke);
        }
    }

    if draw_main {
        let mut h_lines = Vec::new();
        let mut v_lines = Vec::new();
        collect_grid_lines_for_step(state, main_step_upm, canvas_rect, &mut h_lines, &mut v_lines);
        let col = state.theme.edit_canvas.grid_main_color.to_color32();
        let h_stroke = Stroke::new(GRID_MAIN_LINE_WIDTH_DP, col);
        let v_stroke = Stroke::new(
            if is_sloped { GRID_MAIN_SLOPED_LINE_WIDTH_DP } else { GRID_MAIN_LINE_WIDTH_DP },
            col,
        );
        for (a, b) in h_lines {
            painter.line_segment([Pos2::new(a.x.0, a.y.0), Pos2::new(b.x.0, b.y.0)], h_stroke);
        }
        for (a, b) in v_lines {
            painter.line_segment([Pos2::new(a.x.0, a.y.0), Pos2::new(b.x.0, b.y.0)], v_stroke);
        }
    }
}

pub fn render_v030_grid_tiny_skia(pixmap: &mut Pixmap, state: &GlifoCanvasState) {
    if !state.show_grid || state.grid_step_upm <= 0.0 {
        return;
    }
    let bounds = Rect::from_min_max(
        Pos2::ZERO,
        Pos2::new(pixmap.width() as f32, pixmap.height() as f32),
    );
    let zoom = f64::from(state.viewport.zoom);
    let main_step_upm = state.grid_step_upm;
    let subdiv = state.grid_subdivisions.max(1);
    let sub_step_upm = main_step_upm / f64::from(subdiv);
    let draw_sub = subdiv >= GRID_MIN_SUBDIVISIONS && (sub_step_upm * zoom) > GRID_SUB_MIN_STEP_DP;
    let draw_main = (main_step_upm * zoom) > GRID_MAIN_MIN_STEP_DP;
    let is_sloped = slope_shear_dx_per_dy(state).is_some();

    if draw_sub {
        let mut h_lines = Vec::new();
        let mut v_lines = Vec::new();
        collect_grid_lines_for_step(state, sub_step_upm, bounds, &mut h_lines, &mut v_lines);
        let col = state.theme.edit_canvas.grid_sub_color.to_tiny_skia_color();
        stroke_segments_ts(pixmap, &h_lines, col, GRID_SUB_LINE_WIDTH_DP);
        stroke_segments_ts(
            pixmap,
            &v_lines,
            col,
            if is_sloped { GRID_SUB_SLOPED_LINE_WIDTH_DP } else { GRID_SUB_LINE_WIDTH_DP },
        );
    }

    if draw_main {
        let mut h_lines = Vec::new();
        let mut v_lines = Vec::new();
        collect_grid_lines_for_step(state, main_step_upm, bounds, &mut h_lines, &mut v_lines);
        let col = state.theme.edit_canvas.grid_main_color.to_tiny_skia_color();
        stroke_segments_ts(pixmap, &h_lines, col, GRID_MAIN_LINE_WIDTH_DP);
        stroke_segments_ts(
            pixmap,
            &v_lines,
            col,
            if is_sloped { GRID_MAIN_SLOPED_LINE_WIDTH_DP } else { GRID_MAIN_LINE_WIDTH_DP },
        );
    }
}

fn stroke_segments_ts(
    pixmap: &mut Pixmap,
    lines: &[(ScreenPoint, ScreenPoint)],
    color: tiny_skia::Color,
    width: f32,
) {
    if lines.is_empty() {
        return;
    }
    let mut pb = PathBuilder::new();
    for (a, b) in lines {
        pb.move_to(a.x.0, a.y.0);
        pb.line_to(b.x.0, b.y.0);
    }
    if let Some(path) = pb.finish() {
        let mut paint = Paint::default();
        paint.set_color(color);
        paint.anti_alias = true;
        pixmap.stroke_path(
            &path,
            &paint,
            &TsStroke { width, ..Default::default() },
            Transform::identity(),
            None,
        );
    }
}
