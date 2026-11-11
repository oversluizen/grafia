//! `SPEC-005` (`glifo v0.4.0`) visual overlays for `egui::Painter` and `tiny_skia::Pixmap`:
//! - `KnifeTool` (`E`) cut line & intersection circles (`drawBackgroundInRect:`)
//! - `PrimitivesTool` (`F`) rectangle/circle preview & `"W × H"` dimension pill (`drawForegroundInRect:`)
//! - `MeasurementTool` (`L` & `Cmd+Ctrl+Opt`) Blue/Green/Red coordinate & segment delta labels + ray distance/angle pills
//! - `AnnotationTool` (`A`) all 5 `CanvasAnnotation` types (`Text`, `Arrow`, `Circle`, `Plus`, `Minus`) + handles
//! - `PencilTool` (`B`) freehand trail polyline & sample points
//! - `PixelTool` (`X`) hovered grid cell preview

use crate::adapters::state::{CanvasTool, GlifoCanvasState};
use crate::core::{compute_measurement_typographic_angle_deg, format_measurement_value, pixel_cell_bounds_upm, snap_to_pixel_cell, AnnotationKind, CanvasAnnotation, FontPoint, PrimitiveSubTool};
use crate::tokens::{
    bbox_pill_bg_color32, bbox_pill_text_color32, color32_to_tiny_skia, pill_digit_bitmap_3x5,
    ANNOTATION_ARROW_HEAD_ANGLE_RAD, ANNOTATION_ARROW_HEAD_LEN_DP, ANNOTATION_DEFAULT_TEXT_HEIGHT_UPM,
    ANNOTATION_FONT_SIZE_DP, ANNOTATION_HANDLE_RADIUS_DP, ANNOTATION_STROKE_WIDTH_DP,
    ANNOTATION_SYMBOL_HALF_ARM_DP, BBOX_PILL_CHAR_WIDTH_DP, BBOX_PILL_FONT_SIZE_DP,
    BBOX_PILL_MIN_HEIGHT_DP, BBOX_PILL_PAD_X_DP, KNIFE_INTERSECTION_RADIUS_DP,
    KNIFE_LINE_STROKE_WIDTH_DP, MEASUREMENT_ANGLE_BADGE_OFFSET_DP, MEASUREMENT_DOT_RADIUS_DP,
    MEASUREMENT_LABEL_FONT_SIZE_DP, MEASUREMENT_LABEL_OFFSET_DP, MEASUREMENT_LINE_STROKE_WIDTH_DP,
    PENCIL_SAMPLE_DOT_RADIUS_DP, PENCIL_TRAIL_STROKE_WIDTH_DP, PILL_DIGIT_ADVANCE_PX,
    PILL_DIGIT_FONT_HEIGHT_PX, PILL_DIGIT_FONT_WIDTH_PX, PIXEL_PREVIEW_STROKE_WIDTH_DP,
    PRIMITIVE_BADGE_OFFSET_DP, PRIMITIVE_PREVIEW_STROKE_WIDTH_DP,
};
use egui::{Align2, Color32, FontId, Painter, Pos2, Rect, Stroke, Vec2};
use tiny_skia::{FillRule, Paint, PathBuilder, Pixmap, Stroke as TsStroke, Transform};

fn pill_width_dp(text: &str) -> f32 {
    ((text.len() as f32) * BBOX_PILL_CHAR_WIDTH_DP + BBOX_PILL_PAD_X_DP * 2.0).max(BBOX_PILL_MIN_HEIGHT_DP * 2.0)
}

fn paint_pill_egui(painter: &Painter, center: Pos2, text: &str) {
    let rect = Rect::from_center_size(center, Vec2::new(pill_width_dp(text), BBOX_PILL_MIN_HEIGHT_DP));
    painter.rect_filled(rect, BBOX_PILL_MIN_HEIGHT_DP * 0.5, bbox_pill_bg_color32());
    painter.text(center, Align2::CENTER_CENTER, text, FontId::proportional(BBOX_PILL_FONT_SIZE_DP), bbox_pill_text_color32());
}

fn stroke_rect_ts(pixmap: &mut Pixmap, x0: f32, y0: f32, x1: f32, y1: f32, col: tiny_skia::Color, width: f32) {
    if let Some(r) = tiny_skia::Rect::from_ltrb(x0.min(x1), y0.min(y1), x0.max(x1), y0.max(y1)) {
        let mut pb = PathBuilder::new();
        pb.push_rect(r);
        stroke_ts(pixmap, pb, col, width);
    }
}

/// Renders all `SPEC-005` (`glifo v0.4.0`) tool overlays and annotations onto `egui::Painter`.
pub fn paint_v040_overlays_egui(painter: &Painter, state: &GlifoCanvasState) {
    if state.v040.knife.active {
        let (p0, p1) = (state.viewport.to_screen(state.v040.knife.start_upm), state.viewport.to_screen(state.v040.knife.current_upm));
        painter.line_segment([Pos2::new(p0.x.0, p0.y.0), Pos2::new(p1.x.0, p1.y.0)], Stroke::new(KNIFE_LINE_STROKE_WIDTH_DP, state.theme.knife_line_color32()));
        let fill = state.theme.knife_intersection_fill_color32();
        for hit in &state.v040.knife.intersections {
            let s = state.viewport.to_screen(*hit);
            painter.circle_filled(Pos2::new(s.x.0, s.y.0), KNIFE_INTERSECTION_RADIUS_DP, fill);
        }
    }
    if state.v040.primitive_drag.active {
        let (min_x, min_y, max_x, max_y) = state.v040.primitive_drag.preview_rect_upm();
        let (s_min, s_max) = (state.viewport.to_screen(FontPoint::new(min_x, max_y)), state.viewport.to_screen(FontPoint::new(max_x, min_y)));
        let rect = Rect::from_two_pos(Pos2::new(s_min.x.0, s_min.y.0), Pos2::new(s_max.x.0, s_max.y.0));
        let stroke = Stroke::new(PRIMITIVE_PREVIEW_STROKE_WIDTH_DP, state.theme.primitive_preview_stroke_color32());
        match state.v040.primitive_sub_tool {
            PrimitiveSubTool::Rectangle => { painter.rect_stroke(rect, 0.0, stroke, egui::StrokeKind::Middle); }
            PrimitiveSubTool::Circle => { painter.circle_stroke(rect.center(), rect.width().max(rect.height()) * 0.5, stroke); }
        }
        let dim_txt = format!("{}x{}", (max_x - min_x).round() as i64, (max_y - min_y).round() as i64);
        paint_pill_egui(painter, Pos2::new(rect.max.x + PRIMITIVE_BADGE_OFFSET_DP, rect.max.y + PRIMITIVE_BADGE_OFFSET_DP), &dim_txt);
    }
    if state.active_tool == CanvasTool::Measurement { paint_measurement_tool_egui(painter, state); }
    if state.v040.show_annotations || state.active_tool == CanvasTool::Annotation {
        for ann in &state.glyph.annotations { paint_annotation_egui(painter, state, ann); }
    }
    if state.v040.pencil.active && state.v040.pencil.points_upm.len() >= 2 {
        let col = state.theme.pencil_trail_stroke_color32();
        let pts: Vec<Pos2> = state.v040.pencil.points_upm.iter().map(|p| { let s = state.viewport.to_screen(*p); Pos2::new(s.x.0, s.y.0) }).collect();
        for w in pts.windows(2) { painter.line_segment([w[0], w[1]], Stroke::new(PENCIL_TRAIL_STROKE_WIDTH_DP, col)); }
        for p in pts { painter.circle_filled(p, PENCIL_SAMPLE_DOT_RADIUS_DP, col); }
    }
    if state.active_tool == CanvasTool::Pixel {
        if let Some(scr) = state.last_pointer_screen_pos {
            let step = state.effective_pixel_step_upm();
            let (min_x, min_y, max_x, max_y) = pixel_cell_bounds_upm(snap_to_pixel_cell(state.viewport.to_font(scr), step), step);
            let (p0, p1) = (state.viewport.to_screen(FontPoint::new(min_x, max_y)), state.viewport.to_screen(FontPoint::new(max_x, min_y)));
            let rect = Rect::from_two_pos(Pos2::new(p0.x.0, p0.y.0), Pos2::new(p1.x.0, p1.y.0));
            painter.rect_stroke(rect, 0.0, Stroke::new(PIXEL_PREVIEW_STROKE_WIDTH_DP, state.theme.pixel_preview_stroke_color32()), egui::StrokeKind::Middle);
        }
    }
}

fn paint_measurement_tool_egui(painter: &Painter, state: &GlifoCanvasState) {
    let labels = state.compute_measurement_layer_labels();
    let font = FontId::proportional(MEASUREMENT_LABEL_FONT_SIZE_DP);
    let blue = state.theme.measurement_oncurve_label_color32();
    let green = state.theme.measurement_offcurve_label_color32();
    let red = state.theme.measurement_segment_delta_color32();
    for (pt, txt) in &labels.oncurve_labels {
        let s = state.viewport.to_screen(*pt);
        painter.text(Pos2::new(s.x.0 + MEASUREMENT_LABEL_OFFSET_DP, s.y.0 + MEASUREMENT_LABEL_OFFSET_DP), Align2::LEFT_TOP, txt, font.clone(), blue);
    }
    for (pt, txt) in &labels.offcurve_labels {
        let s = state.viewport.to_screen(*pt);
        painter.text(Pos2::new(s.x.0 + MEASUREMENT_LABEL_OFFSET_DP, s.y.0 - MEASUREMENT_LABEL_OFFSET_DP), Align2::LEFT_BOTTOM, txt, font.clone(), green);
    }
    for (pt, txt) in &labels.segment_deltas {
        let s = state.viewport.to_screen(*pt);
        painter.text(Pos2::new(s.x.0, s.y.0), Align2::CENTER_CENTER, txt, font.clone(), red);
    }
    let m_col = state.theme.measurement_line_color32();
    for (pt, txt) in &labels.guide_angles {
        let s = state.viewport.to_screen(*pt);
        painter.text(Pos2::new(s.x.0 + MEASUREMENT_LABEL_OFFSET_DP, s.y.0 - MEASUREMENT_LABEL_OFFSET_DP), Align2::LEFT_BOTTOM, txt, font.clone(), m_col);
    }
    if state.v040.measurement.active {
        let s0 = state.viewport.to_screen(state.v040.measurement.start_upm);
        let s1 = state.viewport.to_screen(state.v040.measurement.current_upm);
        painter.line_segment([Pos2::new(s0.x.0, s0.y.0), Pos2::new(s1.x.0, s1.y.0)], Stroke::new(MEASUREMENT_LINE_STROKE_WIDTH_DP, m_col));
        let hits = &state.v040.measurement.intersections;
        for h in hits {
            let hs = state.viewport.to_screen(*h);
            painter.circle_filled(Pos2::new(hs.x.0, hs.y.0), MEASUREMENT_DOT_RADIUS_DP, m_col);
        }
        for pair in hits.windows(2) {
            let d = (pair[1].x.0 - pair[0].x.0).hypot(pair[1].y.0 - pair[0].y.0);
            if d > 0.5 {
                let mid = FontPoint::new((pair[0].x.0 + pair[1].x.0) * 0.5, (pair[0].y.0 + pair[1].y.0) * 0.5);
                let ms = state.viewport.to_screen(mid);
                paint_pill_egui(painter, Pos2::new(ms.x.0, ms.y.0), &format_measurement_value(d));
            }
        }
        let deg = compute_measurement_typographic_angle_deg(state.v040.measurement.start_upm, state.v040.measurement.current_upm);
        paint_pill_egui(painter, Pos2::new(s1.x.0 + MEASUREMENT_ANGLE_BADGE_OFFSET_DP, s1.y.0 - MEASUREMENT_ANGLE_BADGE_OFFSET_DP), &format!("{}°", format_measurement_value(deg)));
    }
}

fn paint_annotation_egui(painter: &Painter, state: &GlifoCanvasState, ann: &CanvasAnnotation) {
    let is_sel = state.v040.selected_annotations.contains(&ann.id);
    let col: Color32 = state.theme.annotation_stroke_color32(is_sel);
    let stroke = Stroke::new(ANNOTATION_STROKE_WIDTH_DP, col);
    let pos_scr = state.viewport.to_screen(ann.position);
    let c = Pos2::new(pos_scr.x.0, pos_scr.y.0);
    match &ann.kind {
        AnnotationKind::Text { text, width_upm } => {
            let display = if text.is_empty() { "empty note" } else { text.as_str() };
            let end_scr = state.viewport.to_screen(FontPoint::new(ann.position.x.0 + *width_upm, ann.position.y.0 - ANNOTATION_DEFAULT_TEXT_HEIGHT_UPM));
            let rect = Rect::from_two_pos(c, Pos2::new(end_scr.x.0, end_scr.y.0));
            if is_sel || state.active_tool == CanvasTool::Annotation {
                painter.rect_stroke(rect, 2.0, stroke, egui::StrokeKind::Middle);
            }
            painter.text(c, Align2::LEFT_TOP, display, FontId::proportional(ANNOTATION_FONT_SIZE_DP), col);
        }
        AnnotationKind::Arrow { angle_deg, length_upm } => {
            let rad = angle_deg.to_radians();
            let tip_scr = state.viewport.to_screen(FontPoint::new(ann.position.x.0 + *length_upm * rad.cos(), ann.position.y.0 + *length_upm * rad.sin()));
            let t = Pos2::new(tip_scr.x.0, tip_scr.y.0);
            painter.line_segment([c, t], stroke);
            let dir = (t - c).normalized();
            let back = Pos2::new(t.x - dir.x * ANNOTATION_ARROW_HEAD_LEN_DP, t.y - dir.y * ANNOTATION_ARROW_HEAD_LEN_DP);
            let perp = Vec2::new(-dir.y, dir.x) * (ANNOTATION_ARROW_HEAD_LEN_DP * (ANNOTATION_ARROW_HEAD_ANGLE_RAD as f32));
            painter.line_segment([t, back + perp], stroke);
            painter.line_segment([t, back - perp], stroke);
        }
        AnnotationKind::Circle { radius_upm } => {
            painter.circle_stroke(c, ((*radius_upm as f32) * state.viewport.zoom).max(2.0), stroke);
        }
        AnnotationKind::Plus => {
            let arm = ANNOTATION_SYMBOL_HALF_ARM_DP;
            painter.line_segment([Pos2::new(c.x - arm, c.y), Pos2::new(c.x + arm, c.y)], stroke);
            painter.line_segment([Pos2::new(c.x, c.y - arm), Pos2::new(c.x, c.y + arm)], stroke);
        }
        AnnotationKind::Minus => {
            let arm = ANNOTATION_SYMBOL_HALF_ARM_DP;
            painter.line_segment([Pos2::new(c.x - arm, c.y), Pos2::new(c.x + arm, c.y)], stroke);
        }
    }
    if is_sel || state.active_tool == CanvasTool::Annotation {
        if let Some(h_upm) = ann.handle_position_upm() {
            let hs = state.viewport.to_screen(h_upm);
            painter.circle_filled(Pos2::new(hs.x.0, hs.y.0), ANNOTATION_HANDLE_RADIUS_DP, col);
        }
    }
}

/// Renders all `SPEC-005` (`glifo v0.4.0`) tool overlays and annotations onto `tiny_skia::Pixmap`.
pub fn render_v040_overlays_tiny_skia(pixmap: &mut Pixmap, state: &GlifoCanvasState) {
    if state.v040.knife.active {
        let p0 = state.viewport.to_screen(state.v040.knife.start_upm);
        let p1 = state.viewport.to_screen(state.v040.knife.current_upm);
        let mut pb = PathBuilder::new();
        pb.move_to(p0.x.0, p0.y.0);
        pb.line_to(p1.x.0, p1.y.0);
        stroke_ts(pixmap, pb, color32_to_tiny_skia(state.theme.knife_line_color32()), KNIFE_LINE_STROKE_WIDTH_DP);
        let fill = color32_to_tiny_skia(state.theme.knife_intersection_fill_color32());
        for hit in &state.v040.knife.intersections {
            let s = state.viewport.to_screen(*hit);
            fill_circle_ts(pixmap, s.x.0, s.y.0, KNIFE_INTERSECTION_RADIUS_DP, fill);
        }
    }
    if state.v040.primitive_drag.active {
        let (min_x, min_y, max_x, max_y) = state.v040.primitive_drag.preview_rect_upm();
        let (s_min, s_max) = (state.viewport.to_screen(FontPoint::new(min_x, max_y)), state.viewport.to_screen(FontPoint::new(max_x, min_y)));
        let col = color32_to_tiny_skia(state.theme.primitive_preview_stroke_color32());
        match state.v040.primitive_sub_tool {
            PrimitiveSubTool::Rectangle => stroke_rect_ts(pixmap, s_min.x.0, s_min.y.0, s_max.x.0, s_max.y.0, col, PRIMITIVE_PREVIEW_STROKE_WIDTH_DP),
            PrimitiveSubTool::Circle => {
                let mut pb = PathBuilder::new();
                let (cx, cy) = ((s_min.x.0 + s_max.x.0) * 0.5, (s_min.y.0 + s_max.y.0) * 0.5);
                let r = ((s_max.x.0 - s_min.x.0).abs().max((s_max.y.0 - s_min.y.0).abs()) * 0.5).max(1.0);
                pb.push_circle(cx, cy, r);
                stroke_ts(pixmap, pb, col, PRIMITIVE_PREVIEW_STROKE_WIDTH_DP);
            }
        }
        let dim_txt = format!("{}x{}", (max_x - min_x).round() as i64, (max_y - min_y).round() as i64);
        fill_pill_ts(pixmap, s_max.x.0 + PRIMITIVE_BADGE_OFFSET_DP, s_max.y.0 + PRIMITIVE_BADGE_OFFSET_DP, &dim_txt);
    }
    if state.active_tool == CanvasTool::Measurement { render_measurement_tool_ts(pixmap, state); }
    if state.v040.show_annotations || state.active_tool == CanvasTool::Annotation {
        for ann in &state.glyph.annotations { render_annotation_ts(pixmap, state, ann); }
    }
    if state.v040.pencil.active && state.v040.pencil.points_upm.len() >= 2 {
        let col = color32_to_tiny_skia(state.theme.pencil_trail_stroke_color32());
        let mut pb = PathBuilder::new();
        let first = state.viewport.to_screen(state.v040.pencil.points_upm[0]);
        pb.move_to(first.x.0, first.y.0);
        for pt in &state.v040.pencil.points_upm[1..] { let s = state.viewport.to_screen(*pt); pb.line_to(s.x.0, s.y.0); }
        stroke_ts(pixmap, pb, col, PENCIL_TRAIL_STROKE_WIDTH_DP);
        for pt in &state.v040.pencil.points_upm { let s = state.viewport.to_screen(*pt); fill_circle_ts(pixmap, s.x.0, s.y.0, PENCIL_SAMPLE_DOT_RADIUS_DP, col); }
    }
    if state.active_tool == CanvasTool::Pixel {
        if let Some(scr) = state.last_pointer_screen_pos {
            let step = state.effective_pixel_step_upm();
            let (min_x, min_y, max_x, max_y) = pixel_cell_bounds_upm(snap_to_pixel_cell(state.viewport.to_font(scr), step), step);
            let (p0, p1) = (state.viewport.to_screen(FontPoint::new(min_x, max_y)), state.viewport.to_screen(FontPoint::new(max_x, min_y)));
            stroke_rect_ts(pixmap, p0.x.0, p0.y.0, p1.x.0, p1.y.0, color32_to_tiny_skia(state.theme.pixel_preview_stroke_color32()), PIXEL_PREVIEW_STROKE_WIDTH_DP);
        }
    }
}

fn render_measurement_tool_ts(pixmap: &mut Pixmap, state: &GlifoCanvasState) {
    let labels = state.compute_measurement_layer_labels();
    let blue = color32_to_tiny_skia(state.theme.measurement_oncurve_label_color32());
    let green = color32_to_tiny_skia(state.theme.measurement_offcurve_label_color32());
    let red = color32_to_tiny_skia(state.theme.measurement_segment_delta_color32());
    let m_col = color32_to_tiny_skia(state.theme.measurement_line_color32());
    for (pt, txt) in &labels.oncurve_labels {
        let s = state.viewport.to_screen(*pt);
        draw_bitmap_string_ts(pixmap, s.x.0 + MEASUREMENT_LABEL_OFFSET_DP, s.y.0 + MEASUREMENT_LABEL_OFFSET_DP, txt, blue);
    }
    for (pt, txt) in &labels.offcurve_labels {
        let s = state.viewport.to_screen(*pt);
        draw_bitmap_string_ts(pixmap, s.x.0 + MEASUREMENT_LABEL_OFFSET_DP, s.y.0 - MEASUREMENT_LABEL_OFFSET_DP, txt, green);
    }
    for (pt, txt) in &labels.segment_deltas {
        let s = state.viewport.to_screen(*pt);
        draw_bitmap_string_ts(pixmap, s.x.0, s.y.0, txt, red);
    }
    for (pt, txt) in &labels.guide_angles {
        let s = state.viewport.to_screen(*pt);
        draw_bitmap_string_ts(pixmap, s.x.0 + MEASUREMENT_LABEL_OFFSET_DP, s.y.0 - MEASUREMENT_LABEL_OFFSET_DP, txt, m_col);
    }
    if state.v040.measurement.active {
        let (s0, s1) = (state.viewport.to_screen(state.v040.measurement.start_upm), state.viewport.to_screen(state.v040.measurement.current_upm));
        let mut pb = PathBuilder::new();
        pb.move_to(s0.x.0, s0.y.0); pb.line_to(s1.x.0, s1.y.0);
        stroke_ts(pixmap, pb, m_col, MEASUREMENT_LINE_STROKE_WIDTH_DP);
        for h in &state.v040.measurement.intersections {
            let hs = state.viewport.to_screen(*h);
            fill_circle_ts(pixmap, hs.x.0, hs.y.0, MEASUREMENT_DOT_RADIUS_DP, m_col);
        }
        for pair in state.v040.measurement.intersections.windows(2) {
            let d = (pair[1].x.0 - pair[0].x.0).hypot(pair[1].y.0 - pair[0].y.0);
            if d > 0.5 {
                let mid = FontPoint::new((pair[0].x.0 + pair[1].x.0) * 0.5, (pair[0].y.0 + pair[1].y.0) * 0.5);
                let ms = state.viewport.to_screen(mid);
                fill_pill_ts(pixmap, ms.x.0, ms.y.0, &format_measurement_value(d));
            }
        }
        let deg = compute_measurement_typographic_angle_deg(state.v040.measurement.start_upm, state.v040.measurement.current_upm);
        fill_pill_ts(pixmap, s1.x.0 + MEASUREMENT_ANGLE_BADGE_OFFSET_DP, s1.y.0 - MEASUREMENT_ANGLE_BADGE_OFFSET_DP, &format!("{}°", format_measurement_value(deg)));
    }
}

fn render_annotation_ts(pixmap: &mut Pixmap, state: &GlifoCanvasState, ann: &CanvasAnnotation) {
    let is_sel = state.v040.selected_annotations.contains(&ann.id);
    let col = color32_to_tiny_skia(state.theme.annotation_stroke_color32(is_sel));
    let pos_scr = state.viewport.to_screen(ann.position);
    match &ann.kind {
        AnnotationKind::Text { text, width_upm } => {
            let end_scr = state.viewport.to_screen(FontPoint::new(ann.position.x.0 + *width_upm, ann.position.y.0 - ANNOTATION_DEFAULT_TEXT_HEIGHT_UPM));
            stroke_rect_ts(pixmap, pos_scr.x.0, pos_scr.y.0, end_scr.x.0, end_scr.y.0, col, ANNOTATION_STROKE_WIDTH_DP);
            let display = if text.is_empty() { "NOTE" } else { text.as_str() };
            draw_bitmap_string_ts(pixmap, pos_scr.x.0 + MEASUREMENT_LABEL_OFFSET_DP, pos_scr.y.0 + MEASUREMENT_LABEL_OFFSET_DP, display, col);
        }
        AnnotationKind::Arrow { angle_deg, length_upm } => {
            let rad = angle_deg.to_radians();
            let tip = state.viewport.to_screen(FontPoint::new(ann.position.x.0 + *length_upm * rad.cos(), ann.position.y.0 + *length_upm * rad.sin()));
            let mut pb = PathBuilder::new();
            pb.move_to(pos_scr.x.0, pos_scr.y.0);
            pb.line_to(tip.x.0, tip.y.0);
            let (vx, vy) = (tip.x.0 - pos_scr.x.0, tip.y.0 - pos_scr.y.0);
            let len = vx.hypot(vy).max(1.0);
            let (dx, dy) = (vx / len, vy / len);
            let (bx, by) = (tip.x.0 - dx * ANNOTATION_ARROW_HEAD_LEN_DP, tip.y.0 - dy * ANNOTATION_ARROW_HEAD_LEN_DP);
            let wing = ANNOTATION_ARROW_HEAD_LEN_DP * (ANNOTATION_ARROW_HEAD_ANGLE_RAD as f32);
            let (px, py) = (-dy * wing, dx * wing);
            pb.move_to(tip.x.0, tip.y.0);
            pb.line_to(bx + px, by + py);
            pb.move_to(tip.x.0, tip.y.0);
            pb.line_to(bx - px, by - py);
            stroke_ts(pixmap, pb, col, ANNOTATION_STROKE_WIDTH_DP);
        }
        AnnotationKind::Circle { radius_upm } => {
            let mut pb = PathBuilder::new();
            pb.push_circle(pos_scr.x.0, pos_scr.y.0, ((*radius_upm as f32) * state.viewport.zoom).max(2.0));
            stroke_ts(pixmap, pb, col, ANNOTATION_STROKE_WIDTH_DP);
        }
        AnnotationKind::Plus => {
            let arm = ANNOTATION_SYMBOL_HALF_ARM_DP;
            let mut pb = PathBuilder::new();
            pb.move_to(pos_scr.x.0 - arm, pos_scr.y.0);
            pb.line_to(pos_scr.x.0 + arm, pos_scr.y.0);
            pb.move_to(pos_scr.x.0, pos_scr.y.0 - arm);
            pb.line_to(pos_scr.x.0, pos_scr.y.0 + arm);
            stroke_ts(pixmap, pb, col, ANNOTATION_STROKE_WIDTH_DP);
        }
        AnnotationKind::Minus => {
            let arm = ANNOTATION_SYMBOL_HALF_ARM_DP;
            let mut pb = PathBuilder::new();
            pb.move_to(pos_scr.x.0 - arm, pos_scr.y.0);
            pb.line_to(pos_scr.x.0 + arm, pos_scr.y.0);
            stroke_ts(pixmap, pb, col, ANNOTATION_STROKE_WIDTH_DP);
        }
    }
    if let Some(h_upm) = ann.handle_position_upm() {
        let hs = state.viewport.to_screen(h_upm);
        fill_circle_ts(pixmap, hs.x.0, hs.y.0, ANNOTATION_HANDLE_RADIUS_DP, col);
    }
}

fn fill_circle_ts(pixmap: &mut Pixmap, cx: f32, cy: f32, r: f32, color: tiny_skia::Color) {
    let mut pb = PathBuilder::new();
    pb.push_circle(cx, cy, r);
    if let Some(path) = pb.finish() {
        let mut p = Paint::default();
        p.set_color(color);
        p.anti_alias = true;
        pixmap.fill_path(&path, &p, FillRule::Winding, Transform::identity(), None);
    }
}

fn stroke_ts(pixmap: &mut Pixmap, pb: PathBuilder, color: tiny_skia::Color, width: f32) {
    if let Some(path) = pb.finish() {
        let mut paint = Paint::default();
        paint.set_color(color);
        paint.anti_alias = true;
        pixmap.stroke_path(&path, &paint, &TsStroke { width, ..Default::default() }, Transform::identity(), None);
    }
}

fn fill_pill_ts(pixmap: &mut Pixmap, cx: f32, cy: f32, text: &str) {
    let w = pill_width_dp(text);
    let r = BBOX_PILL_MIN_HEIGHT_DP * 0.5;
    let bg_col = color32_to_tiny_skia(bbox_pill_bg_color32());
    let inner_w = (w - BBOX_PILL_MIN_HEIGHT_DP).max(1.0);
    fill_circle_ts(pixmap, cx - inner_w * 0.5, cy, r, bg_col);
    fill_circle_ts(pixmap, cx + inner_w * 0.5, cy, r, bg_col);
    if let Some(rect) = tiny_skia::Rect::from_xywh(cx - inner_w * 0.5, cy - r, inner_w, BBOX_PILL_MIN_HEIGHT_DP) {
        let mut p = Paint::default();
        p.set_color(bg_col);
        pixmap.fill_rect(rect, &p, Transform::identity(), None);
    }
    let start_x = cx - ((text.len() as f32) * PILL_DIGIT_ADVANCE_PX) * 0.5;
    let start_y = cy - (PILL_DIGIT_FONT_HEIGHT_PX as f32) * 0.5;
    draw_bitmap_string_ts(pixmap, start_x, start_y, text, color32_to_tiny_skia(bbox_pill_text_color32()));
}

fn draw_bitmap_string_ts(pixmap: &mut Pixmap, start_x: f32, start_y: f32, text: &str, col: tiny_skia::Color) {
    let mut tp = Paint::default();
    tp.set_color(col);
    for (idx, ch) in text.chars().enumerate() {
        let rows = pill_digit_bitmap_3x5(ch);
        let gx = start_x + (idx as f32) * PILL_DIGIT_ADVANCE_PX;
        for (ry, &mask) in rows.iter().enumerate() {
            for rx in 0..PILL_DIGIT_FONT_WIDTH_PX {
                if (mask >> (PILL_DIGIT_FONT_WIDTH_PX - 1 - rx)) & 1 == 1 {
                    if let Some(px_rect) = tiny_skia::Rect::from_xywh((gx + rx as f32).round(), (start_y + ry as f32).round(), 1.0, 1.0) {
                        pixmap.fill_rect(px_rect, &tp, Transform::identity(), None);
                    }
                }
            }
        }
    }
}
