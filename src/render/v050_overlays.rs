//! `SPEC-006` (`grafia v0.5.0`) visual overlays for `egui::Painter` and `tiny_skia::Pixmap`:
//! - Passive neighbor glyph silhouettes (`theme.passive_glyph_fill`)
//! - Kerning baseline highlight wedges (`theme.kerning_negative_fill` / `theme.kerning_positive_fill`)
//! - Multi-slot selection highlight & text caret in `CanvasTool::Text`
//! - Text-mode measurement scanline, intersection knobs, and zoom-gated numeric badges

pub use crate::render::v050_overlays_ts::render_v050_overlays_tiny_skia;

use crate::adapters::state::{CanvasTool, GrafiaCanvasState};
use crate::core::{compute_glyph_lsb_and_rsb, format_measurement_value, FontPoint, Node};
use crate::render::tessellate_closed_contours_nonzero;
use crate::tokens::{
    TEXT_CARET_STROKE_WIDTH_DP, TEXT_MEASUREMENT_KNOB_RADIUS_DP,
    TEXT_MEASUREMENT_LINE_STROKE_WIDTH_DP, TEXT_METRICS_NUMBER_FONT_SIZE_DP,
    TEXT_METRICS_NUMBER_OFFSET_DP,
};
use egui::{Align2, FontId, Painter, Pos2, Rect, Shape, Stroke};

pub(crate) fn shifted_slot_contours(slot: &crate::core::GlyphSlot) -> (Vec<Vec<Node>>, Vec<bool>) {
    let dx = slot.origin_x_upm;
    let mut contours = Vec::with_capacity(slot.glyph.contours.len());
    let mut closed = Vec::with_capacity(slot.glyph.contours.len());
    for gc in &slot.glyph.contours {
        if !gc.visible || gc.nodes.is_empty() {
            continue;
        }
        let mut nodes = gc.nodes.clone();
        for n in &mut nodes {
            n.point.x.0 += dx;
        }
        contours.push(nodes);
        closed.push(gc.closed);
    }
    (contours, closed)
}

pub(crate) fn caret_x_upm(state: &GrafiaCanvasState) -> f64 {
    if state.text_stream.caret_index < state.text_stream.slots.len() {
        state.text_stream.slots[state.text_stream.caret_index].origin_x_upm
    } else {
        state
            .text_stream
            .slots
            .last()
            .map_or(0.0, |last| last.origin_x_upm + last.glyph.width)
    }
}

/// Renders all `SPEC-006` (`grafia v0.5.0`) multi-glyph stream, kerning, and scanline overlays onto `egui::Painter`.
pub fn paint_v050_overlays_egui(painter: &Painter, state: &GrafiaCanvasState) {
    if !state.text_stream.slots.is_empty() {
        if state.active_tool == CanvasTool::Text {
            if let Some((s, e)) = state.text_stream.selection_range {
                if s < e && e <= state.text_stream.slots.len() {
                    let x0 = state.text_stream.slots[s].origin_x_upm;
                    let last = &state.text_stream.slots[e - 1];
                    let x1 = last.origin_x_upm + last.glyph.width;
                    let p0 = state
                        .viewport
                        .to_screen(FontPoint::new(x0, state.metrics.ascender));
                    let p1 = state
                        .viewport
                        .to_screen(FontPoint::new(x1, state.metrics.descender));
                    let rect =
                        Rect::from_two_pos(Pos2::new(p0.x.0, p0.y.0), Pos2::new(p1.x.0, p1.y.0));
                    painter.rect_filled(rect, 0.0, state.theme.text_selection_fill_color32());
                }
            }
        }

        for quad in state.compute_kerning_indicator_quads() {
            let fill = if quad.kerning_value_upm < 0 {
                state.theme.kerning_negative_fill_color32()
            } else {
                state.theme.kerning_positive_fill_color32()
            };
            let pts: Vec<Pos2> = quad
                .corners_upm
                .iter()
                .map(|pt| {
                    let s = state.viewport.to_screen(*pt);
                    Pos2::new(s.x.0, s.y.0)
                })
                .collect();
            painter.add(Shape::convex_polygon(pts, fill, Stroke::NONE));
        }

        let passive_fill = state.theme.passive_glyph_fill_color32();
        for (slot_idx, slot) in state.text_stream.slots.iter().enumerate() {
            if slot_idx == state.text_stream.active_slot_index
                && state.active_tool != CanvasTool::Text
            {
                continue;
            }
            let (contours, closed) = shifted_slot_contours(slot);
            let mesh = tessellate_closed_contours_nonzero(
                &contours,
                &closed,
                &state.viewport,
                passive_fill,
            );
            if !mesh.is_empty() {
                painter.add(Shape::mesh(mesh));
            }
        }

        if state.active_tool == CanvasTool::Text {
            let cx = caret_x_upm(state);
            let half_xh = state.metrics.x_height * 0.5;
            let tan_a = state.italic_angle_deg.to_radians().tan();
            let p_bot = state.viewport.to_screen(FontPoint::new(
                cx + (state.metrics.descender - half_xh) * tan_a,
                state.metrics.descender,
            ));
            let p_top = state.viewport.to_screen(FontPoint::new(
                cx + (state.metrics.ascender - half_xh) * tan_a,
                state.metrics.ascender,
            ));
            painter.line_segment(
                [
                    Pos2::new(p_bot.x.0, p_bot.y.0),
                    Pos2::new(p_top.x.0, p_top.y.0),
                ],
                Stroke::new(TEXT_CARET_STROKE_WIDTH_DP, state.theme.text_caret_color32()),
            );
        }

        if state.should_draw_text_mode_numbers() {
            let font = FontId::monospace(TEXT_METRICS_NUMBER_FONT_SIZE_DP);
            let num_col = state.theme.text_metrics_number_color32();
            for (slot_idx, slot) in state.text_stream.slots.iter().enumerate() {
                let raw_contours: Vec<Vec<Node>> = slot
                    .glyph
                    .contours
                    .iter()
                    .map(|gc| gc.nodes.clone())
                    .collect();
                let (lsb, rsb) = compute_glyph_lsb_and_rsb(&raw_contours, slot.glyph.width);
                let l_pt = state.viewport.to_screen(FontPoint::new(
                    slot.origin_x_upm + lsb * 0.5,
                    state.metrics.descender,
                ));
                let r_pt = state.viewport.to_screen(FontPoint::new(
                    slot.origin_x_upm + slot.glyph.width - rsb * 0.5,
                    state.metrics.descender,
                ));
                painter.text(
                    Pos2::new(l_pt.x.0, l_pt.y.0 + TEXT_METRICS_NUMBER_OFFSET_DP),
                    Align2::CENTER_TOP,
                    format!("{}", lsb.round() as i64),
                    font.clone(),
                    num_col,
                );
                painter.text(
                    Pos2::new(r_pt.x.0, r_pt.y.0 + TEXT_METRICS_NUMBER_OFFSET_DP),
                    Align2::CENTER_TOP,
                    format!("{}", rsb.round() as i64),
                    font.clone(),
                    num_col,
                );
                if slot_idx > 0 && slot.kern_with_prev != 0 {
                    let k_pt = state
                        .viewport
                        .to_screen(FontPoint::new(slot.origin_x_upm, state.metrics.descender));
                    painter.text(
                        Pos2::new(k_pt.x.0, k_pt.y.0 + TEXT_METRICS_NUMBER_OFFSET_DP * 2.0),
                        Align2::CENTER_TOP,
                        format!("{:+}", slot.kern_with_prev),
                        font.clone(),
                        num_col,
                    );
                }
            }
        }
    }

    if state.show_text_measurement_line || state.temp_show_text_measurement_line {
        paint_scanline_egui(painter, state);
    }
}

fn paint_scanline_egui(painter: &Painter, state: &GrafiaCanvasState) {
    let scans = state.compute_text_scanline_measurements();
    let y_upm = state.text_measurement_y_upm;
    let x_start = scans.first().map_or(0.0, |s| s.lsb_x_upm);
    let x_end = scans.last().map_or(state.glyph.width, |s| s.rsb_x_upm);
    let p0 = state.viewport.to_screen(FontPoint::new(x_start, y_upm));
    let p1 = state.viewport.to_screen(FontPoint::new(x_end, y_upm));
    painter.line_segment(
        [Pos2::new(p0.x.0, p0.y.0), Pos2::new(p1.x.0, p1.y.0)],
        Stroke::new(
            TEXT_MEASUREMENT_LINE_STROKE_WIDTH_DP,
            state.theme.text_measurement_line_color32(),
        ),
    );
    let knob_col = state.theme.text_measurement_knob_fill_color32();
    let num_col = state.theme.text_metrics_number_color32();
    let font = FontId::monospace(TEXT_METRICS_NUMBER_FONT_SIZE_DP);
    let draw_nums = state.should_draw_text_mode_numbers();

    for scan in &scans {
        for x in [scan.lsb_x_upm, scan.rsb_x_upm]
            .into_iter()
            .chain(scan.contour_hit_xs_upm.iter().copied())
        {
            let s = state.viewport.to_screen(FontPoint::new(x, scan.y_upm));
            painter.circle_filled(
                Pos2::new(s.x.0, s.y.0),
                TEXT_MEASUREMENT_KNOB_RADIUS_DP,
                knob_col,
            );
        }
        if draw_nums {
            if let (Some(m_lsb), Some(&first_x)) =
                (scan.measured_lsb_upm, scan.contour_hit_xs_upm.first())
            {
                let s = state
                    .viewport
                    .to_screen(FontPoint::new((scan.lsb_x_upm + first_x) * 0.5, scan.y_upm));
                painter.text(
                    Pos2::new(s.x.0, s.y.0 - TEXT_METRICS_NUMBER_OFFSET_DP),
                    Align2::CENTER_BOTTOM,
                    format_measurement_value(m_lsb),
                    font.clone(),
                    num_col,
                );
            }
            if let (Some(m_rsb), Some(&last_x)) =
                (scan.measured_rsb_upm, scan.contour_hit_xs_upm.last())
            {
                let s = state
                    .viewport
                    .to_screen(FontPoint::new((last_x + scan.rsb_x_upm) * 0.5, scan.y_upm));
                painter.text(
                    Pos2::new(s.x.0, s.y.0 - TEXT_METRICS_NUMBER_OFFSET_DP),
                    Align2::CENTER_BOTTOM,
                    format_measurement_value(m_rsb),
                    font.clone(),
                    num_col,
                );
            }
        }
    }
}
