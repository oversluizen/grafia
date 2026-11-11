//! `SPEC-005` (`grafia v0.4.0`) visual overlays for `egui::Painter` and `tiny_skia::Pixmap`:
//! - `KnifeTool` (`E`) cut line & intersection circles (`drawBackgroundInRect:`)
//! - `PrimitivesTool` (`F`) rectangle/circle preview & `"W × H"` dimension pill (`drawForegroundInRect:`)
//! - `MeasurementTool` (`L` & `Cmd+Ctrl+Opt`) Blue/Green/Red coordinate & segment delta labels + ray distance/angle pills
//! - `AnnotationTool` (`A`) all 5 `CanvasAnnotation` types (`Text`, `Arrow`, `Circle`, `Plus`, `Minus`) + handles
//! - `PencilTool` (`B`) freehand trail polyline & sample points
//! - `PixelTool` (`X`) hovered grid cell preview

pub use crate::render::v040_overlays_ts::render_v040_overlays_tiny_skia;

use crate::adapters::state::{CanvasTool, GrafiaCanvasState};
use crate::core::{
    compute_measurement_typographic_angle_deg, format_measurement_value, pixel_cell_bounds_upm,
    snap_to_pixel_cell, AnnotationKind, CanvasAnnotation, FontPoint, PrimitiveSubTool,
};
use crate::render::ts_primitives::paint_pill_egui;
use crate::tokens::{
    ANNOTATION_ARROW_HEAD_ANGLE_RAD, ANNOTATION_ARROW_HEAD_LEN_DP,
    ANNOTATION_DEFAULT_TEXT_HEIGHT_UPM, ANNOTATION_FONT_SIZE_DP, ANNOTATION_HANDLE_RADIUS_DP,
    ANNOTATION_STROKE_WIDTH_DP, ANNOTATION_SYMBOL_HALF_ARM_DP, KNIFE_INTERSECTION_RADIUS_DP,
    KNIFE_LINE_STROKE_WIDTH_DP, MEASUREMENT_ANGLE_BADGE_OFFSET_DP, MEASUREMENT_DOT_RADIUS_DP,
    MEASUREMENT_LABEL_FONT_SIZE_DP, MEASUREMENT_LABEL_OFFSET_DP, MEASUREMENT_LINE_STROKE_WIDTH_DP,
    PENCIL_SAMPLE_DOT_RADIUS_DP, PENCIL_TRAIL_STROKE_WIDTH_DP, PIXEL_PREVIEW_STROKE_WIDTH_DP,
    PRIMITIVE_BADGE_OFFSET_DP, PRIMITIVE_PREVIEW_STROKE_WIDTH_DP,
};
use egui::{Align2, Color32, FontId, Painter, Pos2, Rect, Stroke, Vec2};

/// Renders all `SPEC-005` (`grafia v0.4.0`) tool overlays and annotations onto `egui::Painter`.
pub fn paint_v040_overlays_egui(painter: &Painter, state: &GrafiaCanvasState) {
    if state.v040.knife.active {
        let (p0, p1) = (
            state.viewport.to_screen(state.v040.knife.start_upm),
            state.viewport.to_screen(state.v040.knife.current_upm),
        );
        painter.line_segment(
            [Pos2::new(p0.x.0, p0.y.0), Pos2::new(p1.x.0, p1.y.0)],
            Stroke::new(KNIFE_LINE_STROKE_WIDTH_DP, state.theme.knife_line_color32()),
        );
        let fill = state.theme.knife_intersection_fill_color32();
        for hit in &state.v040.knife.intersections {
            let s = state.viewport.to_screen(*hit);
            painter.circle_filled(Pos2::new(s.x.0, s.y.0), KNIFE_INTERSECTION_RADIUS_DP, fill);
        }
    }
    if state.v040.primitive_drag.active {
        let (min_x, min_y, max_x, max_y) = state.v040.primitive_drag.preview_rect_upm();
        let (s_min, s_max) = (
            state.viewport.to_screen(FontPoint::new(min_x, max_y)),
            state.viewport.to_screen(FontPoint::new(max_x, min_y)),
        );
        let rect = Rect::from_two_pos(
            Pos2::new(s_min.x.0, s_min.y.0),
            Pos2::new(s_max.x.0, s_max.y.0),
        );
        let stroke = Stroke::new(
            PRIMITIVE_PREVIEW_STROKE_WIDTH_DP,
            state.theme.primitive_preview_stroke_color32(),
        );
        match state.v040.primitive_sub_tool {
            PrimitiveSubTool::Rectangle => {
                painter.rect_stroke(rect, 0.0, stroke, egui::StrokeKind::Middle);
            }
            PrimitiveSubTool::Circle => {
                painter.circle_stroke(rect.center(), rect.width().max(rect.height()) * 0.5, stroke);
            }
        }
        let dim_txt = format!(
            "{}x{}",
            (max_x - min_x).round() as i64,
            (max_y - min_y).round() as i64
        );
        paint_pill_egui(
            painter,
            Pos2::new(
                rect.max.x + PRIMITIVE_BADGE_OFFSET_DP,
                rect.max.y + PRIMITIVE_BADGE_OFFSET_DP,
            ),
            &dim_txt,
        );
    }
    if state.active_tool == CanvasTool::Measurement {
        paint_measurement_tool_egui(painter, state);
    }
    if state.v040.show_annotations || state.active_tool == CanvasTool::Annotation {
        for ann in &state.glyph.annotations {
            paint_annotation_egui(painter, state, ann);
        }
    }
    if state.v040.pencil.active && state.v040.pencil.points_upm.len() >= 2 {
        let col = state.theme.pencil_trail_stroke_color32();
        let pts: Vec<Pos2> = state
            .v040
            .pencil
            .points_upm
            .iter()
            .map(|p| {
                let s = state.viewport.to_screen(*p);
                Pos2::new(s.x.0, s.y.0)
            })
            .collect();
        for w in pts.windows(2) {
            painter.line_segment([w[0], w[1]], Stroke::new(PENCIL_TRAIL_STROKE_WIDTH_DP, col));
        }
        for p in pts {
            painter.circle_filled(p, PENCIL_SAMPLE_DOT_RADIUS_DP, col);
        }
    }
    if state.active_tool == CanvasTool::Pixel {
        if let Some(scr) = state.last_pointer_screen_pos {
            let step = state.effective_pixel_step_upm();
            let (min_x, min_y, max_x, max_y) =
                pixel_cell_bounds_upm(snap_to_pixel_cell(state.viewport.to_font(scr), step), step);
            let (p0, p1) = (
                state.viewport.to_screen(FontPoint::new(min_x, max_y)),
                state.viewport.to_screen(FontPoint::new(max_x, min_y)),
            );
            let rect = Rect::from_two_pos(Pos2::new(p0.x.0, p0.y.0), Pos2::new(p1.x.0, p1.y.0));
            painter.rect_stroke(
                rect,
                0.0,
                Stroke::new(
                    PIXEL_PREVIEW_STROKE_WIDTH_DP,
                    state.theme.pixel_preview_stroke_color32(),
                ),
                egui::StrokeKind::Middle,
            );
        }
    }
}

fn paint_measurement_tool_egui(painter: &Painter, state: &GrafiaCanvasState) {
    let labels = state.compute_measurement_layer_labels();
    let font = FontId::proportional(MEASUREMENT_LABEL_FONT_SIZE_DP);
    let blue = state.theme.measurement_oncurve_label_color32();
    let green = state.theme.measurement_offcurve_label_color32();
    let red = state.theme.measurement_segment_delta_color32();
    for (pt, txt) in &labels.oncurve_labels {
        let s = state.viewport.to_screen(*pt);
        painter.text(
            Pos2::new(
                s.x.0 + MEASUREMENT_LABEL_OFFSET_DP,
                s.y.0 + MEASUREMENT_LABEL_OFFSET_DP,
            ),
            Align2::LEFT_TOP,
            txt,
            font.clone(),
            blue,
        );
    }
    for (pt, txt) in &labels.offcurve_labels {
        let s = state.viewport.to_screen(*pt);
        painter.text(
            Pos2::new(
                s.x.0 + MEASUREMENT_LABEL_OFFSET_DP,
                s.y.0 - MEASUREMENT_LABEL_OFFSET_DP,
            ),
            Align2::LEFT_BOTTOM,
            txt,
            font.clone(),
            green,
        );
    }
    for (pt, txt) in &labels.segment_deltas {
        let s = state.viewport.to_screen(*pt);
        painter.text(
            Pos2::new(s.x.0, s.y.0),
            Align2::CENTER_CENTER,
            txt,
            font.clone(),
            red,
        );
    }
    let m_col = state.theme.measurement_line_color32();
    for (pt, txt) in &labels.guide_angles {
        let s = state.viewport.to_screen(*pt);
        painter.text(
            Pos2::new(
                s.x.0 + MEASUREMENT_LABEL_OFFSET_DP,
                s.y.0 - MEASUREMENT_LABEL_OFFSET_DP,
            ),
            Align2::LEFT_BOTTOM,
            txt,
            font.clone(),
            m_col,
        );
    }
    if state.v040.measurement.active {
        let s0 = state.viewport.to_screen(state.v040.measurement.start_upm);
        let s1 = state.viewport.to_screen(state.v040.measurement.current_upm);
        painter.line_segment(
            [Pos2::new(s0.x.0, s0.y.0), Pos2::new(s1.x.0, s1.y.0)],
            Stroke::new(MEASUREMENT_LINE_STROKE_WIDTH_DP, m_col),
        );
        let hits = &state.v040.measurement.intersections;
        for h in hits {
            let hs = state.viewport.to_screen(*h);
            painter.circle_filled(Pos2::new(hs.x.0, hs.y.0), MEASUREMENT_DOT_RADIUS_DP, m_col);
        }
        for pair in hits.windows(2) {
            let d = (pair[1].x.0 - pair[0].x.0).hypot(pair[1].y.0 - pair[0].y.0);
            if d > 0.5 {
                let mid = FontPoint::new(
                    (pair[0].x.0 + pair[1].x.0) * 0.5,
                    (pair[0].y.0 + pair[1].y.0) * 0.5,
                );
                let ms = state.viewport.to_screen(mid);
                paint_pill_egui(
                    painter,
                    Pos2::new(ms.x.0, ms.y.0),
                    &format_measurement_value(d),
                );
            }
        }
        let deg = compute_measurement_typographic_angle_deg(
            state.v040.measurement.start_upm,
            state.v040.measurement.current_upm,
        );
        paint_pill_egui(
            painter,
            Pos2::new(
                s1.x.0 + MEASUREMENT_ANGLE_BADGE_OFFSET_DP,
                s1.y.0 - MEASUREMENT_ANGLE_BADGE_OFFSET_DP,
            ),
            &format!("{}°", format_measurement_value(deg)),
        );
    }
}

fn paint_annotation_egui(painter: &Painter, state: &GrafiaCanvasState, ann: &CanvasAnnotation) {
    let is_sel = state.v040.selected_annotations.contains(&ann.id);
    let col: Color32 = state.theme.annotation_stroke_color32(is_sel);
    let stroke = Stroke::new(ANNOTATION_STROKE_WIDTH_DP, col);
    let pos_scr = state.viewport.to_screen(ann.position);
    let c = Pos2::new(pos_scr.x.0, pos_scr.y.0);
    match &ann.kind {
        AnnotationKind::Text { text, width_upm } => {
            let display = if text.is_empty() {
                "empty note"
            } else {
                text.as_str()
            };
            let end_scr = state.viewport.to_screen(FontPoint::new(
                ann.position.x.0 + *width_upm,
                ann.position.y.0 - ANNOTATION_DEFAULT_TEXT_HEIGHT_UPM,
            ));
            let rect = Rect::from_two_pos(c, Pos2::new(end_scr.x.0, end_scr.y.0));
            if is_sel || state.active_tool == CanvasTool::Annotation {
                painter.rect_stroke(rect, 2.0, stroke, egui::StrokeKind::Middle);
            }
            painter.text(
                c,
                Align2::LEFT_TOP,
                display,
                FontId::proportional(ANNOTATION_FONT_SIZE_DP),
                col,
            );
        }
        AnnotationKind::Arrow {
            angle_deg,
            length_upm,
        } => {
            let rad = angle_deg.to_radians();
            let tip_scr = state.viewport.to_screen(FontPoint::new(
                ann.position.x.0 + *length_upm * rad.cos(),
                ann.position.y.0 + *length_upm * rad.sin(),
            ));
            let t = Pos2::new(tip_scr.x.0, tip_scr.y.0);
            painter.line_segment([c, t], stroke);
            let dir = (t - c).normalized();
            let back = Pos2::new(
                t.x - dir.x * ANNOTATION_ARROW_HEAD_LEN_DP,
                t.y - dir.y * ANNOTATION_ARROW_HEAD_LEN_DP,
            );
            let perp = Vec2::new(-dir.y, dir.x)
                * (ANNOTATION_ARROW_HEAD_LEN_DP * (ANNOTATION_ARROW_HEAD_ANGLE_RAD as f32));
            painter.line_segment([t, back + perp], stroke);
            painter.line_segment([t, back - perp], stroke);
        }
        AnnotationKind::Circle { radius_upm } => {
            painter.circle_stroke(
                c,
                ((*radius_upm as f32) * state.viewport.zoom).max(2.0),
                stroke,
            );
        }
        AnnotationKind::Plus => {
            let arm = ANNOTATION_SYMBOL_HALF_ARM_DP;
            painter.line_segment(
                [Pos2::new(c.x - arm, c.y), Pos2::new(c.x + arm, c.y)],
                stroke,
            );
            painter.line_segment(
                [Pos2::new(c.x, c.y - arm), Pos2::new(c.x, c.y + arm)],
                stroke,
            );
        }
        AnnotationKind::Minus => {
            let arm = ANNOTATION_SYMBOL_HALF_ARM_DP;
            painter.line_segment(
                [Pos2::new(c.x - arm, c.y), Pos2::new(c.x + arm, c.y)],
                stroke,
            );
        }
    }
    if is_sel || state.active_tool == CanvasTool::Annotation {
        if let Some(h_upm) = ann.handle_position_upm() {
            let hs = state.viewport.to_screen(h_upm);
            painter.circle_filled(Pos2::new(hs.x.0, hs.y.0), ANNOTATION_HANDLE_RADIUS_DP, col);
        }
    }
}
