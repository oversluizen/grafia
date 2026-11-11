//! `SPEC-004` (`glifo v0.3.0`) visual overlays for `egui::Painter` and `tiny_skia::Pixmap`:
//! - Pass 1 Alignment Zone translucent horizontal bands (`#CEA77C33`)
//! - Pass 4 Local (`#3E4CCFB3`) & Global (`#CF4F3EB3`) Guide lines, Knobs, Directional Ticks, & Padlocks
//! - Pass 4 Measurement Guide (`show_measurement == true`) intersection dots, ticks, and distance pills
//! - Pass 4 Active Magnetic Snappers (`#993333FF` `1.2 dp` lines + `4.0 dp` endpoint circles)
//! - Pass 4 Alignment Zone & Guide Node Badges (Modes `1`..`4` from `_drawAlignmentIndicator:`)

use crate::adapters::state::GlifoCanvasState;
use crate::core::{FontPoint, NodeType, ScreenPoint};
use crate::snapping::{AlignmentBadgeMode, Guide, SelectedGuideRef};
use crate::tokens::{
    alignment_badge_global_guide_fill_color32, alignment_badge_local_guide_fill_color32,
    alignment_badge_metric_fill_color32, alignment_badge_stroke_color32,
    alignment_zone_fill_color32, bbox_pill_bg_color32, bbox_pill_text_color32, color32_to_tiny_skia,
    global_guide_color32, local_guide_color32, pill_digit_bitmap_3x5, snap_indicator_color32,
    BADGE_MODE1_SELECTED_SIZE_DP, BADGE_MODE1_UNSELECTED_SIZE_DP, BADGE_MODE234_SELECTED_SIZE_DP,
    BADGE_MODE234_UNSELECTED_SIZE_DP, BADGE_STROKE_WIDTH_DP, BADGE_VERTICAL_RECT_EXTRA_H_DP,
    BADGE_VERTICAL_RECT_HALF_W_DP, BADGE_ZOOM_SCALE_MAX, BADGE_ZOOM_SCALE_MIN,
    BBOX_HANDLE_ZOOM_EXP, BBOX_PILL_CHAR_WIDTH_DP, BBOX_PILL_FONT_SIZE_DP, BBOX_PILL_MIN_HEIGHT_DP,
    BBOX_PILL_PAD_X_DP, DEFAULT_X_HEIGHT_UPM, DEG_TO_RAD_F64, GUIDE_KNOB_RADIUS_DP,
    GUIDE_LINE_STROKE_WIDTH_DP, GUIDE_LOCK_BODY_HALF_W_DP, GUIDE_LOCK_BODY_H_DP,
    GUIDE_LOCK_SHACKLE_RADIUS_DP, GUIDE_LOCK_STROKE_WIDTH_DP, GUIDE_MEASUREMENT_DOT_RADIUS_DP,
    GUIDE_MEASUREMENT_PILL_OFFSET_DP, GUIDE_MEASUREMENT_TICK_HALF_DP, GUIDE_SELECTED_KNOB_RADIUS_DP,
    GUIDE_SELECTED_LINE_STROKE_WIDTH_DP, GUIDE_SELECTED_TICK_LEN_DP, PILL_DIGIT_ADVANCE_PX,
    PILL_DIGIT_FONT_HEIGHT_PX, PILL_DIGIT_FONT_WIDTH_PX, SNAP_INDICATOR_CIRCLE_RADIUS_DP,
    SNAP_INDICATOR_STROKE_WIDTH_DP,
};
use egui::epaint::{PathShape, PathStroke};
use egui::{Align2, Color32, FontId, Painter, Pos2, Rect, Shape, Stroke};
use tiny_skia::{FillRule, Paint, PathBuilder, Pixmap, Stroke as TsStroke, Transform};

fn alignment_zone_screen_quad(
    state: &GlifoCanvasState,
    pos_upm: f64,
    size_upm: f64,
) -> [ScreenPoint; 4] {
    let adv_w = if state.glyph.width > 0.0 { state.glyph.width } else { DEFAULT_X_HEIGHT_UPM };
    let y0 = pos_upm.min(pos_upm + size_upm);
    let y1 = pos_upm.max(pos_upm + size_upm);
    let half_xh = state.metrics.x_height * 0.5;
    let tan_italic = (state.italic_angle_deg * DEG_TO_RAD_F64).tan();
    let dx0 = (y0 - half_xh) * tan_italic;
    let dx1 = (y1 - half_xh) * tan_italic;
    [
        state.viewport.to_screen(FontPoint::new(dx1, y1)),
        state.viewport.to_screen(FontPoint::new(adv_w + dx1, y1)),
        state.viewport.to_screen(FontPoint::new(adv_w + dx0, y0)),
        state.viewport.to_screen(FontPoint::new(dx0, y0)),
    ]
}

pub fn paint_v030_alignment_zones_egui(painter: &Painter, _canvas_rect: Rect, state: &GlifoCanvasState) {
    if !state.show_metrics {
        return;
    }
    let fill = state.theme.edit_canvas.alignment_zone_fill.to_color32();
    let _ = alignment_zone_fill_color32();
    for zone in &state.metrics.alignment_zones {
        if zone.size.abs() <= 0.0 { continue; }
        let q = alignment_zone_screen_quad(state, zone.position, zone.size);
        let pts: Vec<Pos2> = q.iter().map(|p| Pos2::new(p.x.0, p.y.0)).collect();
        painter.add(Shape::Path(PathShape {
            points: pts,
            closed: true,
            fill,
            stroke: PathStroke::NONE,
        }));
    }
}

pub fn render_v030_alignment_zones_tiny_skia(pixmap: &mut Pixmap, state: &GlifoCanvasState) {
    if !state.show_metrics {
        return;
    }
    let col = state.theme.edit_canvas.alignment_zone_fill.to_tiny_skia_color();
    let mut paint = Paint::default();
    paint.set_color(col);
    paint.anti_alias = true;
    for zone in &state.metrics.alignment_zones {
        if zone.size.abs() <= 0.0 { continue; }
        let q = alignment_zone_screen_quad(state, zone.position, zone.size);
        let mut pb = PathBuilder::new();
        pb.move_to(q[0].x.0, q[0].y.0);
        for p in &q[1..] {
            pb.line_to(p.x.0, p.y.0);
        }
        pb.close();
        if let Some(path) = pb.finish() {
            pixmap.fill_path(&path, &paint, FillRule::Winding, Transform::identity(), None);
        }
    }
}

fn guide_screen_segment(guide: &Guide, state: &GlifoCanvasState) -> (ScreenPoint, ScreenPoint) {
    let (b_font, e_font) = guide.infinite_endpoints();
    (state.viewport.to_screen(b_font), state.viewport.to_screen(e_font))
}

fn badge_zoom_scale(zoom: f32) -> f32 {
    zoom.powf(BBOX_HANDLE_ZOOM_EXP).clamp(BADGE_ZOOM_SCALE_MIN, BADGE_ZOOM_SCALE_MAX)
}

fn badge_fill_color32(mode: AlignmentBadgeMode) -> Color32 {
    match mode {
        AlignmentBadgeMode::OnMetricLine
        | AlignmentBadgeMode::InAlignmentZone
        | AlignmentBadgeMode::OnVerticalMetric => alignment_badge_metric_fill_color32(),
        AlignmentBadgeMode::OnLocalGuide => alignment_badge_local_guide_fill_color32(),
        AlignmentBadgeMode::OnGlobalGuide => alignment_badge_global_guide_fill_color32(),
    }
}

fn vertical_metric_quad(cx: f32, cy: f32, half_h: f32, italic_deg: f64) -> [ScreenPoint; 4] {
    let hw = BADGE_VERTICAL_RECT_HALF_W_DP;
    let hh = half_h + BADGE_VERTICAL_RECT_EXTRA_H_DP;
    let dx_top = hh * ((italic_deg * DEG_TO_RAD_F64).tan() as f32);
    [
        ScreenPoint::new(cx - hw + dx_top, cy - hh),
        ScreenPoint::new(cx + hw + dx_top, cy - hh),
        ScreenPoint::new(cx + hw - dx_top, cy + hh),
        ScreenPoint::new(cx - hw - dx_top, cy + hh),
    ]
}

fn pill_width_dp(text: &str) -> f32 {
    ((text.len() as f32) * BBOX_PILL_CHAR_WIDTH_DP + BBOX_PILL_PAD_X_DP * 2.0)
        .max(BBOX_PILL_MIN_HEIGHT_DP * 2.0)
}

fn paint_pill_egui(painter: &Painter, center: Pos2, text: &str) {
    let rect = Rect::from_center_size(center, egui::Vec2::new(pill_width_dp(text), BBOX_PILL_MIN_HEIGHT_DP));
    painter.rect_filled(rect, BBOX_PILL_MIN_HEIGHT_DP * 0.5, bbox_pill_bg_color32());
    painter.text(center, Align2::CENTER_CENTER, text, FontId::proportional(BBOX_PILL_FONT_SIZE_DP), bbox_pill_text_color32());
}

pub fn paint_v030_overlays_egui(painter: &Painter, state: &GlifoCanvasState) {
    if state.show_guides {
        for guide in state.guides.iter().chain(state.global_guides.iter()) {
            let col = if guide.is_global {
                state.theme.edit_canvas.global_guide_color.to_color32()
            } else {
                state.theme.edit_canvas.local_guide_color.to_color32()
            };
            let _ = (local_guide_color32(), global_guide_color32());
            let is_sel = state.selected_guide == Some(SelectedGuideRef { id: guide.id, is_global: guide.is_global });
            let sw = if is_sel { GUIDE_SELECTED_LINE_STROKE_WIDTH_DP } else { GUIDE_LINE_STROKE_WIDTH_DP };
            let (s0, s1) = guide_screen_segment(guide, state);
            painter.line_segment([Pos2::new(s0.x.0, s0.y.0), Pos2::new(s1.x.0, s1.y.0)], Stroke::new(sw, col));

            let k_scr = state.viewport.to_screen(guide.position);
            let k_pos = Pos2::new(k_scr.x.0, k_scr.y.0);
            if state.lock_guides || guide.locked {
                let body = Rect::from_min_max(
                    Pos2::new(k_pos.x - GUIDE_LOCK_BODY_HALF_W_DP, k_pos.y - GUIDE_LOCK_BODY_H_DP * 0.5),
                    Pos2::new(k_pos.x + GUIDE_LOCK_BODY_HALF_W_DP, k_pos.y + GUIDE_LOCK_BODY_H_DP * 0.5),
                );
                painter.rect_filled(body, 1.0, col);
                painter.circle_stroke(Pos2::new(k_pos.x, body.top()), GUIDE_LOCK_SHACKLE_RADIUS_DP, Stroke::new(GUIDE_LOCK_STROKE_WIDTH_DP, col));
            } else {
                let kr = if is_sel { GUIDE_SELECTED_KNOB_RADIUS_DP } else { GUIDE_KNOB_RADIUS_DP };
                painter.circle_filled(k_pos, kr, col);
                if is_sel {
                    let (ux, uy) = guide.direction_unit();
                    let (nx, ny) = ((uy as f32) * GUIDE_SELECTED_TICK_LEN_DP, (ux as f32) * GUIDE_SELECTED_TICK_LEN_DP);
                    painter.line_segment([Pos2::new(k_pos.x - nx, k_pos.y - ny), Pos2::new(k_pos.x + nx, k_pos.y + ny)], Stroke::new(GUIDE_SELECTED_LINE_STROKE_WIDTH_DP, col));
                }
            }
        }

        for ray in state.all_measurement_rays() {
            let col = if ray.is_global {
                state.theme.edit_canvas.global_guide_color.to_color32()
            } else {
                state.theme.edit_canvas.local_guide_color.to_color32()
            };
            let (nx, ny) = ((ray.direction.1 as f32) * GUIDE_MEASUREMENT_TICK_HALF_DP, (ray.direction.0 as f32) * GUIDE_MEASUREMENT_TICK_HALF_DP);
            for pt in &ray.intersections {
                let scr = state.viewport.to_screen(*pt);
                let p = Pos2::new(scr.x.0, scr.y.0);
                painter.circle_filled(p, GUIDE_MEASUREMENT_DOT_RADIUS_DP, col);
                painter.line_segment([Pos2::new(p.x - nx, p.y - ny), Pos2::new(p.x + nx, p.y + ny)], Stroke::new(GUIDE_LINE_STROKE_WIDTH_DP, col));
            }
            for seg in &ray.segments {
                let mid = state.viewport.to_screen(seg.midpoint);
                paint_pill_egui(painter, Pos2::new(mid.x.0, mid.y.0 - GUIDE_MEASUREMENT_PILL_OFFSET_DP), &format!("{}", seg.distance.round() as i64));
            }
        }
    }

    let snap_col = state.theme.edit_canvas.snap_indicator_color.to_color32();
    let _ = snap_indicator_color32();
    let snap_stroke = Stroke::new(SNAP_INDICATOR_STROKE_WIDTH_DP, snap_col);
    for snapper in &state.active_snappers {
        let (a, b) = (state.viewport.to_screen(snapper.from), state.viewport.to_screen(snapper.to));
        let (pa, pb) = (Pos2::new(a.x.0, a.y.0), Pos2::new(b.x.0, b.y.0));
        if !snapper.is_point_target {
            painter.line_segment([pa, pb], snap_stroke);
            painter.circle_filled(pa, SNAP_INDICATOR_CIRCLE_RADIUS_DP, snap_col);
        }
        painter.circle_filled(pb, SNAP_INDICATOR_CIRCLE_RADIUS_DP, snap_col);
    }
}

pub fn paint_v030_alignment_badges_egui(painter: &Painter, state: &GlifoCanvasState) {
    let scale = badge_zoom_scale(state.viewport.zoom);
    let badge_stroke = Stroke::new(BADGE_STROKE_WIDTH_DP, alignment_badge_stroke_color32());
    for node in state.contours.iter().flatten() {
        if node.node_type == NodeType::OffCurve { continue; }
        let Some(mode) = state.classify_node_alignment(node.point) else { continue; };
        let is_sel = state.selected_nodes.contains(&node.id);
        let scr = state.viewport.to_screen(node.point);
        let center = Pos2::new(scr.x.0, scr.y.0);
        let fill = badge_fill_color32(mode);
        let base_sz = match (mode, is_sel) {
            (AlignmentBadgeMode::OnMetricLine, false) => BADGE_MODE1_UNSELECTED_SIZE_DP,
            (AlignmentBadgeMode::OnMetricLine, true) => BADGE_MODE1_SELECTED_SIZE_DP,
            (_, false) => BADGE_MODE234_UNSELECTED_SIZE_DP,
            (_, true) => BADGE_MODE234_SELECTED_SIZE_DP,
        };
        let half = base_sz * scale * 0.5;
        match mode {
            AlignmentBadgeMode::OnMetricLine | AlignmentBadgeMode::OnLocalGuide | AlignmentBadgeMode::OnGlobalGuide => {
                let pts = vec![
                    Pos2::new(center.x, center.y - half), Pos2::new(center.x + half, center.y),
                    Pos2::new(center.x, center.y + half), Pos2::new(center.x - half, center.y),
                ];
                painter.add(Shape::Path(PathShape { points: pts, closed: true, fill, stroke: PathStroke::new(BADGE_STROKE_WIDTH_DP, alignment_badge_stroke_color32()) }));
            }
            AlignmentBadgeMode::InAlignmentZone => {
                painter.circle_filled(center, half, fill);
                painter.circle_stroke(center, half, badge_stroke);
            }
            AlignmentBadgeMode::OnVerticalMetric => {
                let q = vertical_metric_quad(center.x, center.y, half, state.italic_angle_deg);
                let pts: Vec<Pos2> = q.iter().map(|p| Pos2::new(p.x.0, p.y.0)).collect();
                painter.add(Shape::Path(PathShape { points: pts, closed: true, fill, stroke: PathStroke::new(BADGE_STROKE_WIDTH_DP, alignment_badge_stroke_color32()) }));
            }
        }
    }
}

pub fn render_v030_overlays_tiny_skia(pixmap: &mut Pixmap, state: &GlifoCanvasState) {
    if state.show_guides {
        for guide in state.guides.iter().chain(state.global_guides.iter()) {
            let ts_col = if guide.is_global {
                state.theme.edit_canvas.global_guide_color.to_tiny_skia_color()
            } else {
                state.theme.edit_canvas.local_guide_color.to_tiny_skia_color()
            };
            let is_sel = state.selected_guide == Some(SelectedGuideRef { id: guide.id, is_global: guide.is_global });
            let sw = if is_sel { GUIDE_SELECTED_LINE_STROKE_WIDTH_DP } else { GUIDE_LINE_STROKE_WIDTH_DP };
            let (s0, s1) = guide_screen_segment(guide, state);
            let mut pb = PathBuilder::new();
            pb.move_to(s0.x.0, s0.y.0);
            pb.line_to(s1.x.0, s1.y.0);
            stroke_ts(pixmap, pb, ts_col, sw);

            let k_scr = state.viewport.to_screen(guide.position);
            if state.lock_guides || guide.locked {
                let top_y = k_scr.y.0 - GUIDE_LOCK_BODY_H_DP * 0.5;
                if let Some(r) = tiny_skia::Rect::from_xywh(
                    k_scr.x.0 - GUIDE_LOCK_BODY_HALF_W_DP, top_y,
                    GUIDE_LOCK_BODY_HALF_W_DP * 2.0, GUIDE_LOCK_BODY_H_DP,
                ) {
                    let mut p = Paint::default();
                    p.set_color(ts_col);
                    pixmap.fill_rect(r, &p, Transform::identity(), None);
                }
                let mut spb = PathBuilder::new();
                spb.push_circle(k_scr.x.0, top_y, GUIDE_LOCK_SHACKLE_RADIUS_DP);
                stroke_ts(pixmap, spb, ts_col, GUIDE_LOCK_STROKE_WIDTH_DP);
            } else {
                let kr = if is_sel { GUIDE_SELECTED_KNOB_RADIUS_DP } else { GUIDE_KNOB_RADIUS_DP };
                fill_circle_ts(pixmap, k_scr.x.0, k_scr.y.0, kr, ts_col);
                if is_sel {
                    let (ux, uy) = guide.direction_unit();
                    let (nx, ny) = ((uy as f32) * GUIDE_SELECTED_TICK_LEN_DP, (ux as f32) * GUIDE_SELECTED_TICK_LEN_DP);
                    let mut tpb = PathBuilder::new();
                    tpb.move_to(k_scr.x.0 - nx, k_scr.y.0 - ny);
                    tpb.line_to(k_scr.x.0 + nx, k_scr.y.0 + ny);
                    stroke_ts(pixmap, tpb, ts_col, GUIDE_SELECTED_LINE_STROKE_WIDTH_DP);
                }
            }
        }

        for ray in state.all_measurement_rays() {
            let m_col = if ray.is_global {
                state.theme.edit_canvas.global_guide_color.to_tiny_skia_color()
            } else {
                state.theme.edit_canvas.local_guide_color.to_tiny_skia_color()
            };
            let (nx, ny) = ((ray.direction.1 as f32) * GUIDE_MEASUREMENT_TICK_HALF_DP, (ray.direction.0 as f32) * GUIDE_MEASUREMENT_TICK_HALF_DP);
            for pt in &ray.intersections {
                let scr = state.viewport.to_screen(*pt);
                fill_circle_ts(pixmap, scr.x.0, scr.y.0, GUIDE_MEASUREMENT_DOT_RADIUS_DP, m_col);
                let mut tpb = PathBuilder::new();
                tpb.move_to(scr.x.0 - nx, scr.y.0 - ny);
                tpb.line_to(scr.x.0 + nx, scr.y.0 + ny);
                stroke_ts(pixmap, tpb, m_col, GUIDE_LINE_STROKE_WIDTH_DP);
            }
            for seg in &ray.segments {
                let mid = state.viewport.to_screen(seg.midpoint);
                fill_pill_ts(pixmap, mid.x.0, mid.y.0 - GUIDE_MEASUREMENT_PILL_OFFSET_DP, &format!("{}", seg.distance.round() as i64));
            }
        }
    }

    let snap_col = color32_to_tiny_skia(state.theme.edit_canvas.snap_indicator_color.to_color32());
    for snapper in &state.active_snappers {
        let (a, b) = (state.viewport.to_screen(snapper.from), state.viewport.to_screen(snapper.to));
        if !snapper.is_point_target {
            let mut pb = PathBuilder::new();
            pb.move_to(a.x.0, a.y.0);
            pb.line_to(b.x.0, b.y.0);
            stroke_ts(pixmap, pb, snap_col, SNAP_INDICATOR_STROKE_WIDTH_DP);
            fill_circle_ts(pixmap, a.x.0, a.y.0, SNAP_INDICATOR_CIRCLE_RADIUS_DP, snap_col);
        }
        fill_circle_ts(pixmap, b.x.0, b.y.0, SNAP_INDICATOR_CIRCLE_RADIUS_DP, snap_col);
    }
}

pub fn render_v030_alignment_badges_tiny_skia(pixmap: &mut Pixmap, state: &GlifoCanvasState) {
    let scale = badge_zoom_scale(state.viewport.zoom);
    let stroke_col = color32_to_tiny_skia(alignment_badge_stroke_color32());
    for node in state.contours.iter().flatten() {
        if node.node_type == NodeType::OffCurve { continue; }
        let Some(mode) = state.classify_node_alignment(node.point) else { continue; };
        let is_sel = state.selected_nodes.contains(&node.id);
        let scr = state.viewport.to_screen(node.point);
        let fill_col = color32_to_tiny_skia(badge_fill_color32(mode));
        let base_sz = match (mode, is_sel) {
            (AlignmentBadgeMode::OnMetricLine, false) => BADGE_MODE1_UNSELECTED_SIZE_DP,
            (AlignmentBadgeMode::OnMetricLine, true) => BADGE_MODE1_SELECTED_SIZE_DP,
            (_, false) => BADGE_MODE234_UNSELECTED_SIZE_DP,
            (_, true) => BADGE_MODE234_SELECTED_SIZE_DP,
        };
        let half = base_sz * scale * 0.5;
        match mode {
            AlignmentBadgeMode::OnMetricLine | AlignmentBadgeMode::OnLocalGuide | AlignmentBadgeMode::OnGlobalGuide => {
                let pts = [
                    ScreenPoint::new(scr.x.0, scr.y.0 - half), ScreenPoint::new(scr.x.0 + half, scr.y.0),
                    ScreenPoint::new(scr.x.0, scr.y.0 + half), ScreenPoint::new(scr.x.0 - half, scr.y.0),
                ];
                fill_and_stroke_poly_ts(pixmap, &pts, fill_col, stroke_col, BADGE_STROKE_WIDTH_DP);
            }
            AlignmentBadgeMode::InAlignmentZone => {
                fill_circle_ts(pixmap, scr.x.0, scr.y.0, half, fill_col);
                let mut pb = PathBuilder::new();
                pb.push_circle(scr.x.0, scr.y.0, half);
                stroke_ts(pixmap, pb, stroke_col, BADGE_STROKE_WIDTH_DP);
            }
            AlignmentBadgeMode::OnVerticalMetric => {
                let q = vertical_metric_quad(scr.x.0, scr.y.0, half, state.italic_angle_deg);
                fill_and_stroke_poly_ts(pixmap, &q, fill_col, stroke_col, BADGE_STROKE_WIDTH_DP);
            }
        }
    }
}

fn fill_and_stroke_poly_ts(pixmap: &mut Pixmap, pts: &[ScreenPoint], fill_col: tiny_skia::Color, stroke_col: tiny_skia::Color, sw: f32) {
    let mut pb = PathBuilder::new();
    pb.move_to(pts[0].x.0, pts[0].y.0);
    for p in &pts[1..] { pb.line_to(p.x.0, p.y.0); }
    pb.close();
    if let Some(path) = pb.finish() {
        let mut fp = Paint::default();
        fp.set_color(fill_col);
        fp.anti_alias = true;
        pixmap.fill_path(&path, &fp, FillRule::Winding, Transform::identity(), None);
        let mut sp = Paint::default();
        sp.set_color(stroke_col);
        sp.anti_alias = true;
        pixmap.stroke_path(&path, &sp, &TsStroke { width: sw, ..Default::default() }, Transform::identity(), None);
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
    let mut tp = Paint::default();
    tp.set_color(color32_to_tiny_skia(bbox_pill_text_color32()));
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
