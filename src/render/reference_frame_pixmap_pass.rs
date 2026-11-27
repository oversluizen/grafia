//! Pass 1 `tiny-skia` pixmap rendering for `CustomReferenceFrame` (`SPEC-010 §2.3`).

use crate::adapters::state::GrafiaCanvasState;
use crate::core::{reference_frame_center_upm, FontPoint, ScreenPoint};
use crate::render::reference_frame_pass::ReferenceFramePalette;
use crate::render::shared_geom::{
    project_keyline_circles_screen, project_rect_upm_to_screen,
    project_subdivision_segments_screen, ref_frame_crosshair_half_dp, ref_frame_dash_gap_dp,
    resolve_band_thickness_px, resolve_reference_frame_span_screen, snap_pixel_center,
};
use crate::render::ts_primitives::{
    ts_fill_rect, ts_stroke_circle, ts_stroke_dashed_line, ts_stroke_line, ts_stroke_rect,
};
use crate::tokens::trace_and_band_tokens::guide_band_fill_color32;
use crate::tokens::GrafiaTheme;
use egui::{Pos2, Rect, Vec2};
use tiny_skia::Pixmap;

pub fn paint_custom_reference_frame_pixmap_pass(
    pixmap: &mut Pixmap,
    state: &GrafiaCanvasState,
    theme: &GrafiaTheme,
) {
    let Some(frame) = state.custom_reference_frame.as_ref() else {
        return;
    };
    let vp = state.active_slot_viewport();
    let stroke_w = theme.edit_canvas.guide_stroke_width_dp;
    let palette = ReferenceFramePalette::from_theme(theme);
    let outer_col = palette.outer;
    let inner_col = palette.inner;
    let (dash_dp, gap_dp) = ref_frame_dash_gap_dp();
    let canvas_rect = Rect::from_min_size(
        Pos2::ZERO,
        Vec2::new(pixmap.width() as f32, pixmap.height() as f32),
    );

    let (x0_scr, y0_scr, x1_scr, y1_scr) =
        resolve_reference_frame_span_screen(frame, &vp, canvas_rect);
    for line in &frame.parallel_lines {
        if !line.y_or_x_upm.is_finite() {
            continue;
        }
        let (band_px, line_w) = resolve_band_thickness_px(line.thickness, vp.zoom, stroke_w);
        if line.horizontal {
            let y_scr = snap_pixel_center(vp.to_screen(FontPoint::new(0.0, line.y_or_x_upm)).y.0);
            if let Some(b_px) = band_px {
                let half = b_px * 0.5;
                ts_fill_rect(
                    pixmap,
                    x0_scr,
                    y_scr - half,
                    x1_scr,
                    y_scr + half,
                    guide_band_fill_color32(inner_col),
                );
            }
            let a = ScreenPoint::new(x0_scr, y_scr);
            let b = ScreenPoint::new(x1_scr, y_scr);
            if line.dashed {
                ts_stroke_dashed_line(pixmap, a, b, outer_col, line_w, dash_dp, gap_dp);
            } else {
                ts_stroke_line(pixmap, a, b, inner_col, line_w);
            }
        } else {
            let x_scr = snap_pixel_center(vp.to_screen(FontPoint::new(line.y_or_x_upm, 0.0)).x.0);
            if let Some(b_px) = band_px {
                let half = b_px * 0.5;
                ts_fill_rect(
                    pixmap,
                    x_scr - half,
                    y0_scr,
                    x_scr + half,
                    y1_scr,
                    guide_band_fill_color32(inner_col),
                );
            }
            let a = ScreenPoint::new(x_scr, y0_scr);
            let b = ScreenPoint::new(x_scr, y1_scr);
            if line.dashed {
                ts_stroke_dashed_line(pixmap, a, b, outer_col, line_w, dash_dp, gap_dp);
            } else {
                ts_stroke_line(pixmap, a, b, inner_col, line_w);
            }
        }
    }

    paint_frame_boxes_and_circles_ts(pixmap, frame, state.glyph.width, &vp, stroke_w, palette);
    paint_icon_optical_balance_ts(pixmap, frame, state, &vp, stroke_w, palette);
}

fn paint_icon_optical_balance_ts(
    pixmap: &mut Pixmap,
    frame: &crate::core::CustomReferenceFrame,
    state: &GrafiaCanvasState,
    vp: &crate::core::ViewportTransform,
    stroke_w: f32,
    palette: ReferenceFramePalette,
) {
    if !frame.show_diagonals {
        return;
    }
    if let Some(rep) = state.compute_active_icon_optical_balance() {
        let b_scr = vp.to_screen(rep.bbox_center_upm);
        let o_scr = vp.to_screen(rep.optical_centroid_upm);
        let half = ref_frame_crosshair_half_dp();
        ts_stroke_line(pixmap, b_scr, o_scr, palette.keyline, stroke_w);
        ts_stroke_circle(pixmap, o_scr, half, palette.keyline, stroke_w);
        ts_stroke_line(
            pixmap,
            ScreenPoint::new(o_scr.x.0 - half, o_scr.y.0),
            ScreenPoint::new(o_scr.x.0 + half, o_scr.y.0),
            palette.keyline,
            stroke_w,
        );
        ts_stroke_line(
            pixmap,
            ScreenPoint::new(o_scr.x.0, o_scr.y.0 - half),
            ScreenPoint::new(o_scr.x.0, o_scr.y.0 + half),
            palette.keyline,
            stroke_w,
        );
    }
}

fn paint_frame_boxes_and_circles_ts(
    pixmap: &mut Pixmap,
    frame: &crate::core::CustomReferenceFrame,
    default_width_upm: f64,
    vp: &crate::core::ViewportTransform,
    stroke_w: f32,
    palette: ReferenceFramePalette,
) {
    let ReferenceFramePalette {
        outer: outer_col,
        inner: inner_col,
        subdiv: subdiv_col,
        keyline: keyline_col,
    } = palette;
    let (dash_dp, gap_dp) = ref_frame_dash_gap_dp();
    if let Some(outer) = frame.outer_box {
        let r = project_rect_upm_to_screen(outer, vp);
        ts_stroke_rect(
            pixmap,
            r.left(),
            r.top(),
            r.right(),
            r.bottom(),
            outer_col,
            stroke_w,
        );
        if frame.show_diagonals {
            for (a, b) in [
                (
                    ScreenPoint::new(r.left(), r.top()),
                    ScreenPoint::new(r.right(), r.bottom()),
                ),
                (
                    ScreenPoint::new(r.right(), r.top()),
                    ScreenPoint::new(r.left(), r.bottom()),
                ),
            ] {
                ts_stroke_dashed_line(pixmap, a, b, subdiv_col, stroke_w, dash_dp, gap_dp);
            }
        }
    }
    if let Some(inner) = frame.inner_box {
        let r = project_rect_upm_to_screen(inner, vp);
        ts_stroke_rect(
            pixmap,
            r.left(),
            r.top(),
            r.right(),
            r.bottom(),
            inner_col,
            stroke_w,
        );
    }
    for kr in &frame.keyline_rects {
        let r = project_rect_upm_to_screen(*kr, vp);
        ts_stroke_rect(
            pixmap,
            r.left(),
            r.top(),
            r.right(),
            r.bottom(),
            keyline_col,
            stroke_w,
        );
    }
    if let Some((nx, ny)) = frame.inner_grid_subdivisions {
        if let Some(target_box) = frame.inner_box.or(frame.outer_box) {
            for (a, b) in project_subdivision_segments_screen(target_box, nx, ny, vp) {
                ts_stroke_dashed_line(pixmap, a, b, subdiv_col, stroke_w, dash_dp, gap_dp);
            }
        }
    }
    paint_frame_circles_and_crosshair_ts(pixmap, frame, default_width_upm, vp, stroke_w, palette);
}

fn paint_frame_circles_and_crosshair_ts(
    pixmap: &mut Pixmap,
    frame: &crate::core::CustomReferenceFrame,
    default_width_upm: f64,
    vp: &crate::core::ViewportTransform,
    stroke_w: f32,
    palette: ReferenceFramePalette,
) {
    for (center_scr, radius_px) in project_keyline_circles_screen(frame, default_width_upm, vp) {
        ts_stroke_circle(pixmap, center_scr, radius_px, palette.keyline, stroke_w);
    }
    if frame.center_crosshair {
        let c = vp.to_screen(reference_frame_center_upm(frame, default_width_upm));
        let half = ref_frame_crosshair_half_dp();
        ts_stroke_line(
            pixmap,
            ScreenPoint::new(c.x.0 - half, c.y.0),
            ScreenPoint::new(c.x.0 + half, c.y.0),
            palette.inner,
            stroke_w,
        );
        ts_stroke_line(
            pixmap,
            ScreenPoint::new(c.x.0, c.y.0 - half),
            ScreenPoint::new(c.x.0, c.y.0 + half),
            palette.inner,
            stroke_w,
        );
    }
}
