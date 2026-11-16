//! `SPEC-005` `tiny-skia` overlay renderers for `KnifeTool`, `PrimitivesTool`,
//! `MeasurementTool`, `AnnotationTool`, `PencilTool`, and `PixelTool`.

use crate::adapters::state::{CanvasTool, GrafiaCanvasState};
use crate::core::{
    compute_measurement_typographic_angle_deg, format_measurement_value, pixel_cell_bounds_upm,
    snap_to_pixel_cell, AnnotationKind, CanvasAnnotation, FontPoint, PrimitiveSubTool,
};
use crate::render::ts_primitives::{
    draw_bitmap_string_ts, fill_circle_ts, fill_pill_ts, stroke_rect_ts, stroke_ts,
};
use crate::tokens::{
    color32_to_tiny_skia, ANNOTATION_ARROW_HEAD_ANGLE_RAD, ANNOTATION_ARROW_HEAD_LEN_DP,
    ANNOTATION_DEFAULT_TEXT_HEIGHT_UPM, ANNOTATION_HANDLE_RADIUS_DP, ANNOTATION_STROKE_WIDTH_DP,
    ANNOTATION_SYMBOL_HALF_ARM_DP, KNIFE_INTERSECTION_RADIUS_DP, KNIFE_LINE_STROKE_WIDTH_DP,
    MEASUREMENT_ANGLE_BADGE_OFFSET_DP, MEASUREMENT_DOT_RADIUS_DP, MEASUREMENT_LABEL_OFFSET_DP,
    MEASUREMENT_LINE_STROKE_WIDTH_DP, PENCIL_SAMPLE_DOT_RADIUS_DP, PENCIL_TRAIL_STROKE_WIDTH_DP,
    PIXEL_PREVIEW_STROKE_WIDTH_DP, PRIMITIVE_BADGE_OFFSET_DP, PRIMITIVE_PREVIEW_STROKE_WIDTH_DP,
};
use tiny_skia::{PathBuilder, Pixmap};

/// Renders all `SPEC-005` tool overlays and annotations onto `tiny_skia::Pixmap`.
pub fn render_drawing_tool_overlays_tiny_skia(pixmap: &mut Pixmap, state: &GrafiaCanvasState) {
    if state.drawing_tool_state.knife.active {
        let p0 = state
            .viewport
            .to_screen(state.drawing_tool_state.knife.start_upm);
        let p1 = state
            .viewport
            .to_screen(state.drawing_tool_state.knife.current_upm);
        let mut pb = PathBuilder::new();
        pb.move_to(p0.x.0, p0.y.0);
        pb.line_to(p1.x.0, p1.y.0);
        stroke_ts(
            pixmap,
            pb,
            color32_to_tiny_skia(state.theme.knife_line_color32()),
            KNIFE_LINE_STROKE_WIDTH_DP,
        );
        let fill = color32_to_tiny_skia(state.theme.knife_intersection_fill_color32());
        for hit in &state.drawing_tool_state.knife.intersections {
            let s = state.viewport.to_screen(*hit);
            fill_circle_ts(pixmap, s.x.0, s.y.0, KNIFE_INTERSECTION_RADIUS_DP, fill);
        }
    }
    if state.drawing_tool_state.primitive_drag.active {
        render_primitive_drag_preview_ts(pixmap, state);
    }
    if state.active_tool == CanvasTool::Measurement {
        render_measurement_tool_ts(pixmap, state);
    }
    if state.drawing_tool_state.show_annotations || state.active_tool == CanvasTool::Annotation {
        for ann in &state.glyph.annotations {
            render_annotation_ts(pixmap, state, ann);
        }
    }
    if state.drawing_tool_state.pencil.active
        && state.drawing_tool_state.pencil.points_upm.len() >= 2
    {
        let col = color32_to_tiny_skia(state.theme.pencil_trail_stroke_color32());
        let mut pb = PathBuilder::new();
        let first = state
            .viewport
            .to_screen(state.drawing_tool_state.pencil.points_upm[0]);
        pb.move_to(first.x.0, first.y.0);
        for pt in &state.drawing_tool_state.pencil.points_upm[1..] {
            let s = state.viewport.to_screen(*pt);
            pb.line_to(s.x.0, s.y.0);
        }
        stroke_ts(pixmap, pb, col, PENCIL_TRAIL_STROKE_WIDTH_DP);
        for pt in &state.drawing_tool_state.pencil.points_upm {
            let s = state.viewport.to_screen(*pt);
            fill_circle_ts(pixmap, s.x.0, s.y.0, PENCIL_SAMPLE_DOT_RADIUS_DP, col);
        }
    }
    render_pixel_and_brush_preview_ts(pixmap, state);
}

fn render_pixel_and_brush_preview_ts(pixmap: &mut Pixmap, state: &GrafiaCanvasState) {
    if state.active_tool == CanvasTool::Pixel {
        if let Some(scr) = state.last_pointer_screen_pos {
            let step = state.effective_pixel_step_upm();
            let (min_x, min_y, max_x, max_y) =
                pixel_cell_bounds_upm(snap_to_pixel_cell(state.viewport.to_font(scr), step), step);
            let (p0, p1) = (
                state.viewport.to_screen(FontPoint::new(min_x, max_y)),
                state.viewport.to_screen(FontPoint::new(max_x, min_y)),
            );
            stroke_rect_ts(
                pixmap,
                p0.x.0,
                p0.y.0,
                p1.x.0,
                p1.y.0,
                color32_to_tiny_skia(state.theme.pixel_preview_stroke_color32()),
                PIXEL_PREVIEW_STROKE_WIDTH_DP,
            );
        }
    }
    if matches!(state.active_tool, CanvasTool::Trowel | CanvasTool::Cleaner) {
        if let Some(scr) = state.cursor_screen_pos.or(state.last_pointer_screen_pos) {
            let mut pb = PathBuilder::new();
            pb.push_circle(scr.x.0, scr.y.0, state.effective_brush_radius_dp());
            stroke_ts(
                pixmap,
                pb,
                color32_to_tiny_skia(state.theme.primitive_preview_stroke_color32()),
                PRIMITIVE_PREVIEW_STROKE_WIDTH_DP,
            );
        }
    }
}

fn render_primitive_drag_preview_ts(pixmap: &mut Pixmap, state: &GrafiaCanvasState) {
    let (min_x, min_y, max_x, max_y) = state.drawing_tool_state.primitive_drag.preview_rect_upm();
    let (s_min, s_max) = (
        state.viewport.to_screen(FontPoint::new(min_x, max_y)),
        state.viewport.to_screen(FontPoint::new(max_x, min_y)),
    );
    let col = color32_to_tiny_skia(state.theme.primitive_preview_stroke_color32());
    match state.drawing_tool_state.primitive_sub_tool {
        PrimitiveSubTool::Rectangle => stroke_rect_ts(
            pixmap,
            s_min.x.0,
            s_min.y.0,
            s_max.x.0,
            s_max.y.0,
            col,
            PRIMITIVE_PREVIEW_STROKE_WIDTH_DP,
        ),
        PrimitiveSubTool::Circle => {
            let mut pb = PathBuilder::new();
            let (cx, cy) = ((s_min.x.0 + s_max.x.0) * 0.5, (s_min.y.0 + s_max.y.0) * 0.5);
            let r = ((s_max.x.0 - s_min.x.0)
                .abs()
                .max((s_max.y.0 - s_min.y.0).abs())
                * 0.5)
                .max(1.0);
            pb.push_circle(cx, cy, r);
            stroke_ts(pixmap, pb, col, PRIMITIVE_PREVIEW_STROKE_WIDTH_DP);
        }
    }
    let dim_txt = format!(
        "{}x{}",
        (max_x - min_x).round() as i64,
        (max_y - min_y).round() as i64
    );
    fill_pill_ts(
        pixmap,
        s_max.x.0 + PRIMITIVE_BADGE_OFFSET_DP,
        s_max.y.0 + PRIMITIVE_BADGE_OFFSET_DP,
        &dim_txt,
    );
}

fn render_measurement_tool_ts(pixmap: &mut Pixmap, state: &GrafiaCanvasState) {
    let labels = state.compute_measurement_layer_labels();
    let blue = color32_to_tiny_skia(state.theme.measurement_oncurve_label_color32());
    let green = color32_to_tiny_skia(state.theme.measurement_offcurve_label_color32());
    let red = color32_to_tiny_skia(state.theme.measurement_segment_delta_color32());
    let m_col = color32_to_tiny_skia(state.theme.measurement_line_color32());
    for (pt, txt) in &labels.oncurve_labels {
        let s = state.viewport.to_screen(*pt);
        draw_bitmap_string_ts(
            pixmap,
            s.x.0 + MEASUREMENT_LABEL_OFFSET_DP,
            s.y.0 + MEASUREMENT_LABEL_OFFSET_DP,
            txt,
            blue,
        );
    }
    for (pt, txt) in &labels.offcurve_labels {
        let s = state.viewport.to_screen(*pt);
        draw_bitmap_string_ts(
            pixmap,
            s.x.0 + MEASUREMENT_LABEL_OFFSET_DP,
            s.y.0 - MEASUREMENT_LABEL_OFFSET_DP,
            txt,
            green,
        );
    }
    for (pt, txt) in &labels.segment_deltas {
        let s = state.viewport.to_screen(*pt);
        draw_bitmap_string_ts(pixmap, s.x.0, s.y.0, txt, red);
    }
    for (pt, txt) in &labels.guide_angles {
        let s = state.viewport.to_screen(*pt);
        draw_bitmap_string_ts(
            pixmap,
            s.x.0 + MEASUREMENT_LABEL_OFFSET_DP,
            s.y.0 - MEASUREMENT_LABEL_OFFSET_DP,
            txt,
            m_col,
        );
    }
    if state.drawing_tool_state.measurement.active {
        render_active_measurement_line_ts(pixmap, state, m_col);
    }
}

fn render_active_measurement_line_ts(
    pixmap: &mut Pixmap,
    state: &GrafiaCanvasState,
    m_col: tiny_skia::Color,
) {
    let (s0, s1) = (
        state
            .viewport
            .to_screen(state.drawing_tool_state.measurement.start_upm),
        state
            .viewport
            .to_screen(state.drawing_tool_state.measurement.current_upm),
    );
    let mut pb = PathBuilder::new();
    pb.move_to(s0.x.0, s0.y.0);
    pb.line_to(s1.x.0, s1.y.0);
    stroke_ts(pixmap, pb, m_col, MEASUREMENT_LINE_STROKE_WIDTH_DP);
    for h in &state.drawing_tool_state.measurement.intersections {
        let hs = state.viewport.to_screen(*h);
        fill_circle_ts(pixmap, hs.x.0, hs.y.0, MEASUREMENT_DOT_RADIUS_DP, m_col);
    }
    for pair in state
        .drawing_tool_state
        .measurement
        .intersections
        .windows(2)
    {
        let d = (pair[1].x.0 - pair[0].x.0).hypot(pair[1].y.0 - pair[0].y.0);
        if d > 0.5 {
            let mid = FontPoint::new(
                (pair[0].x.0 + pair[1].x.0) * 0.5,
                (pair[0].y.0 + pair[1].y.0) * 0.5,
            );
            let ms = state.viewport.to_screen(mid);
            fill_pill_ts(pixmap, ms.x.0, ms.y.0, &format_measurement_value(d));
        }
    }
    let deg = compute_measurement_typographic_angle_deg(
        state.drawing_tool_state.measurement.start_upm,
        state.drawing_tool_state.measurement.current_upm,
    );
    fill_pill_ts(
        pixmap,
        s1.x.0 + MEASUREMENT_ANGLE_BADGE_OFFSET_DP,
        s1.y.0 - MEASUREMENT_ANGLE_BADGE_OFFSET_DP,
        &format!("{}°", format_measurement_value(deg)),
    );
}

fn render_annotation_ts(pixmap: &mut Pixmap, state: &GrafiaCanvasState, ann: &CanvasAnnotation) {
    let is_sel = state
        .drawing_tool_state
        .selected_annotations
        .contains(&ann.id);
    let col = color32_to_tiny_skia(state.theme.annotation_stroke_color32(is_sel));
    let pos_scr = state.viewport.to_screen(ann.position);
    match &ann.kind {
        AnnotationKind::Text { text, width_upm } => {
            let end_scr = state.viewport.to_screen(FontPoint::new(
                ann.position.x.0 + *width_upm,
                ann.position.y.0 - ANNOTATION_DEFAULT_TEXT_HEIGHT_UPM,
            ));
            stroke_rect_ts(
                pixmap,
                pos_scr.x.0,
                pos_scr.y.0,
                end_scr.x.0,
                end_scr.y.0,
                col,
                ANNOTATION_STROKE_WIDTH_DP,
            );
            let display = if text.is_empty() {
                "NOTE"
            } else {
                text.as_str()
            };
            draw_bitmap_string_ts(
                pixmap,
                pos_scr.x.0 + MEASUREMENT_LABEL_OFFSET_DP,
                pos_scr.y.0 + MEASUREMENT_LABEL_OFFSET_DP,
                display,
                col,
            );
        }
        AnnotationKind::Arrow {
            angle_deg,
            length_upm,
        } => {
            let rad = angle_deg.to_radians();
            let tip = state.viewport.to_screen(FontPoint::new(
                ann.position.x.0 + *length_upm * rad.cos(),
                ann.position.y.0 + *length_upm * rad.sin(),
            ));
            render_arrow_annotation_ts(pixmap, pos_scr, tip, col);
        }
        AnnotationKind::Circle { radius_upm } => {
            let mut pb = PathBuilder::new();
            pb.push_circle(
                pos_scr.x.0,
                pos_scr.y.0,
                ((*radius_upm as f32) * state.viewport.zoom).max(2.0),
            );
            stroke_ts(pixmap, pb, col, ANNOTATION_STROKE_WIDTH_DP);
        }
        AnnotationKind::Plus | AnnotationKind::Minus => {
            let arm = ANNOTATION_SYMBOL_HALF_ARM_DP;
            let mut pb = PathBuilder::new();
            pb.move_to(pos_scr.x.0 - arm, pos_scr.y.0);
            pb.line_to(pos_scr.x.0 + arm, pos_scr.y.0);
            if matches!(ann.kind, AnnotationKind::Plus) {
                pb.move_to(pos_scr.x.0, pos_scr.y.0 - arm);
                pb.line_to(pos_scr.x.0, pos_scr.y.0 + arm);
            }
            stroke_ts(pixmap, pb, col, ANNOTATION_STROKE_WIDTH_DP);
        }
    }
    if let Some(h_upm) = ann.handle_position_upm() {
        let hs = state.viewport.to_screen(h_upm);
        fill_circle_ts(pixmap, hs.x.0, hs.y.0, ANNOTATION_HANDLE_RADIUS_DP, col);
    }
}

fn render_arrow_annotation_ts(
    pixmap: &mut Pixmap,
    pos_scr: crate::core::ScreenPoint,
    tip: crate::core::ScreenPoint,
    col: tiny_skia::Color,
) {
    let mut pb = PathBuilder::new();
    pb.move_to(pos_scr.x.0, pos_scr.y.0);
    pb.line_to(tip.x.0, tip.y.0);
    let (vx, vy) = (tip.x.0 - pos_scr.x.0, tip.y.0 - pos_scr.y.0);
    let len = vx.hypot(vy).max(1.0);
    let (dx, dy) = (vx / len, vy / len);
    let (bx, by) = (
        tip.x.0 - dx * ANNOTATION_ARROW_HEAD_LEN_DP,
        tip.y.0 - dy * ANNOTATION_ARROW_HEAD_LEN_DP,
    );
    let wing = ANNOTATION_ARROW_HEAD_LEN_DP * (ANNOTATION_ARROW_HEAD_ANGLE_RAD as f32);
    let (px, py) = (-dy * wing, dx * wing);
    pb.move_to(tip.x.0, tip.y.0);
    pb.line_to(bx + px, by + py);
    pb.move_to(tip.x.0, tip.y.0);
    pb.line_to(bx - px, by - py);
    stroke_ts(pixmap, pb, col, ANNOTATION_STROKE_WIDTH_DP);
}
