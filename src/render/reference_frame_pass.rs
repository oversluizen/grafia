//! Pass 1 `egui::Painter` rendering for `CustomReferenceFrame` (`SPEC-010 §2.3`).

use crate::adapters::state::GrafiaCanvasState;
use crate::core::reference_frame_center_upm;
use crate::core::{FontPoint, ScreenPoint};
use crate::render::shared_geom::{
    project_keyline_circles_screen, project_rect_upm_to_screen,
    project_subdivision_segments_screen, ref_frame_crosshair_half_dp, ref_frame_dash_gap_dp,
    resolve_band_thickness_px, resolve_reference_frame_span_screen, snap_pixel_center,
};
use crate::render::ts_primitives::paint_dashed_line_egui;
use crate::tokens::trace_and_band_tokens::{
    guide_band_fill_color32, ref_frame_inner_color32, ref_frame_keyline_color32,
    ref_frame_outer_color32, ref_frame_subdiv_color32,
};
use crate::tokens::GrafiaTheme;
use egui::{Color32, Painter, Pos2, Rect, Stroke, StrokeKind};

/// Resolved `egui::Color32` palette for Custom Reference Frame rendering (`SPEC-014 §3.2`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ReferenceFramePalette {
    pub outer: Color32,
    pub inner: Color32,
    pub subdiv: Color32,
    pub keyline: Color32,
}

impl ReferenceFramePalette {
    /// Resolves the reference frame palette from `theme`.
    #[must_use]
    pub fn from_theme(theme: &GrafiaTheme) -> Self {
        if theme.reference_frame_outer.a == 0 && theme.reference_frame_inner.a == 0 {
            let is_dark = theme.is_dark();
            Self {
                outer: ref_frame_outer_color32(is_dark),
                inner: ref_frame_inner_color32(is_dark),
                subdiv: ref_frame_subdiv_color32(is_dark),
                keyline: ref_frame_keyline_color32(is_dark),
            }
        } else {
            Self {
                outer: theme.reference_frame_outer.to_color32(),
                inner: theme.reference_frame_inner.to_color32(),
                subdiv: theme.reference_frame_subdiv.to_color32(),
                keyline: theme.reference_frame_keyline.to_color32(),
            }
        }
    }
}

#[derive(Debug, Clone, Copy)]
struct ReferenceFrameSpanScr {
    x0: f32,
    y0: f32,
    x1: f32,
    y1: f32,
}

pub fn paint_custom_reference_frame_pass(
    painter: &Painter,
    state: &GrafiaCanvasState,
    theme: &GrafiaTheme,
    canvas_rect: Rect,
) {
    let Some(frame) = state.custom_reference_frame.as_ref() else {
        return;
    };
    let vp = state.active_slot_viewport();
    let stroke_w = theme.edit_canvas.guide_stroke_width_dp;
    let palette = ReferenceFramePalette::from_theme(theme);
    let (dash_dp, gap_dp) = ref_frame_dash_gap_dp();

    let (x0_scr, y0_scr, x1_scr, y1_scr) =
        resolve_reference_frame_span_screen(frame, &vp, canvas_rect);
    let span = ReferenceFrameSpanScr {
        x0: x0_scr,
        y0: y0_scr,
        x1: x1_scr,
        y1: y1_scr,
    };
    for line in &frame.parallel_lines {
        if line.y_or_x_upm.is_finite() {
            paint_parallel_line_egui(
                painter,
                line,
                &vp,
                span,
                stroke_w,
                (palette.outer, palette.inner),
                (dash_dp, gap_dp),
            );
        }
    }

    paint_frame_boxes_and_circles_egui(painter, frame, state.glyph.width, &vp, stroke_w, palette);
    if frame.show_diagonals {
        if let Some(rep) = state.compute_active_icon_optical_balance() {
            let b_scr = vp.to_screen(rep.bbox_center_upm);
            let o_scr = vp.to_screen(rep.optical_centroid_upm);
            let half = ref_frame_crosshair_half_dp();
            let st = Stroke::new(stroke_w, palette.keyline);
            painter.line_segment(
                [
                    Pos2::new(b_scr.x.0, b_scr.y.0),
                    Pos2::new(o_scr.x.0, o_scr.y.0),
                ],
                st,
            );
            painter.circle_stroke(Pos2::new(o_scr.x.0, o_scr.y.0), half, st);
            painter.line_segment(
                [
                    Pos2::new(o_scr.x.0 - half, o_scr.y.0),
                    Pos2::new(o_scr.x.0 + half, o_scr.y.0),
                ],
                st,
            );
            painter.line_segment(
                [
                    Pos2::new(o_scr.x.0, o_scr.y.0 - half),
                    Pos2::new(o_scr.x.0, o_scr.y.0 + half),
                ],
                st,
            );
        }
    }
}

fn paint_parallel_line_egui(
    painter: &Painter,
    line: &crate::core::ReferenceBandLine,
    vp: &crate::core::ViewportTransform,
    span: ReferenceFrameSpanScr,
    stroke_w: f32,
    cols: (egui::Color32, egui::Color32),
    dash_gap: (f32, f32),
) {
    let ReferenceFrameSpanScr {
        x0: x0_scr,
        y0: y0_scr,
        x1: x1_scr,
        y1: y1_scr,
    } = span;
    let (outer_col, inner_col) = cols;
    let (dash_dp, gap_dp) = dash_gap;
    let (band_px, line_w) = resolve_band_thickness_px(line.thickness, vp.zoom, stroke_w);
    let (a, b, band_rect) = if line.horizontal {
        let y_scr = snap_pixel_center(vp.to_screen(FontPoint::new(0.0, line.y_or_x_upm)).y.0);
        let br = band_px.map(|b_px| {
            Rect::from_min_max(
                Pos2::new(x0_scr, y_scr - b_px * 0.5),
                Pos2::new(x1_scr, y_scr + b_px * 0.5),
            )
        });
        (
            ScreenPoint::new(x0_scr, y_scr),
            ScreenPoint::new(x1_scr, y_scr),
            br,
        )
    } else {
        let x_scr = snap_pixel_center(vp.to_screen(FontPoint::new(line.y_or_x_upm, 0.0)).x.0);
        let br = band_px.map(|b_px| {
            Rect::from_min_max(
                Pos2::new(x_scr - b_px * 0.5, y0_scr),
                Pos2::new(x_scr + b_px * 0.5, y1_scr),
            )
        });
        (
            ScreenPoint::new(x_scr, y0_scr),
            ScreenPoint::new(x_scr, y1_scr),
            br,
        )
    };
    if let Some(rect) = band_rect {
        painter.rect_filled(rect, 0.0, guide_band_fill_color32(inner_col));
    }
    if line.dashed {
        paint_dashed_line_egui(
            painter,
            a,
            b,
            Stroke::new(line_w, outer_col),
            dash_dp,
            gap_dp,
        );
    } else {
        painter.line_segment(
            [Pos2::new(a.x.0, a.y.0), Pos2::new(b.x.0, b.y.0)],
            Stroke::new(line_w, inner_col),
        );
    }
}

fn paint_frame_boxes_and_circles_egui(
    painter: &Painter,
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
        painter.rect_stroke(r, 0.0, Stroke::new(stroke_w, outer_col), StrokeKind::Middle);
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
                paint_dashed_line_egui(
                    painter,
                    a,
                    b,
                    Stroke::new(stroke_w, subdiv_col),
                    dash_dp,
                    gap_dp,
                );
            }
        }
    }
    if let Some(inner) = frame.inner_box {
        let r = project_rect_upm_to_screen(inner, vp);
        painter.rect_stroke(r, 0.0, Stroke::new(stroke_w, inner_col), StrokeKind::Middle);
    }
    for kr in &frame.keyline_rects {
        let r = project_rect_upm_to_screen(*kr, vp);
        painter.rect_stroke(
            r,
            0.0,
            Stroke::new(stroke_w, keyline_col),
            StrokeKind::Middle,
        );
    }
    if let Some((nx, ny)) = frame.inner_grid_subdivisions {
        if let Some(target_box) = frame.inner_box.or(frame.outer_box) {
            for (a, b) in project_subdivision_segments_screen(target_box, nx, ny, vp) {
                paint_dashed_line_egui(
                    painter,
                    a,
                    b,
                    Stroke::new(stroke_w, subdiv_col),
                    dash_dp,
                    gap_dp,
                );
            }
        }
    }
    paint_frame_circles_and_crosshair_egui(
        painter,
        frame,
        default_width_upm,
        vp,
        stroke_w,
        palette,
    );
}

fn paint_frame_circles_and_crosshair_egui(
    painter: &Painter,
    frame: &crate::core::CustomReferenceFrame,
    default_width_upm: f64,
    vp: &crate::core::ViewportTransform,
    stroke_w: f32,
    palette: ReferenceFramePalette,
) {
    for (center_scr, radius_px) in project_keyline_circles_screen(frame, default_width_upm, vp) {
        painter.circle_stroke(
            Pos2::new(center_scr.x.0, center_scr.y.0),
            radius_px,
            Stroke::new(stroke_w, palette.keyline),
        );
    }
    if frame.center_crosshair {
        let c = vp.to_screen(reference_frame_center_upm(frame, default_width_upm));
        let half = ref_frame_crosshair_half_dp();
        let st = Stroke::new(stroke_w, palette.inner);
        painter.line_segment(
            [
                Pos2::new(c.x.0 - half, c.y.0),
                Pos2::new(c.x.0 + half, c.y.0),
            ],
            st,
        );
        painter.line_segment(
            [
                Pos2::new(c.x.0, c.y.0 - half),
                Pos2::new(c.x.0, c.y.0 + half),
            ],
            st,
        );
    }
}
