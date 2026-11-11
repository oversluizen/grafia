//! `SPEC-006` (`grafia v0.5.0`) `tiny-skia` overlay renderers for passive neighbor glyphs,
//! kerning wedges, text selection/caret, and text measurement scanlines.

use crate::adapters::state::{CanvasTool, GrafiaCanvasState};
use crate::core::{
    compute_glyph_lsb_and_rsb, extract_segments, format_measurement_value, ContourSegment,
    FontPoint, Node,
};
use crate::render::ts_primitives::{draw_bitmap_label_ts, fill_circle_ts, stroke_line_ts};
use crate::render::v050_overlays::{caret_x_upm, shifted_slot_contours};
use crate::tokens::{
    color32_to_tiny_skia, TEXT_CARET_STROKE_WIDTH_DP, TEXT_MEASUREMENT_KNOB_RADIUS_DP,
    TEXT_MEASUREMENT_LINE_STROKE_WIDTH_DP, TEXT_METRICS_NUMBER_OFFSET_DP,
};
use tiny_skia::{FillRule, Paint, PathBuilder, Pixmap, Transform};

/// Renders all `SPEC-006` (`grafia v0.5.0`) multi-glyph stream, kerning, and scanline overlays onto `tiny_skia::Pixmap`.
pub fn render_v050_overlays_tiny_skia(pixmap: &mut Pixmap, state: &GrafiaCanvasState) {
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
                    if let Some(r) = tiny_skia::Rect::from_ltrb(
                        p0.x.0.min(p1.x.0),
                        p0.y.0.min(p1.y.0),
                        p0.x.0.max(p1.x.0),
                        p0.y.0.max(p1.y.0),
                    ) {
                        let mut paint = Paint::default();
                        paint.set_color(color32_to_tiny_skia(
                            state.theme.text_selection_fill_color32(),
                        ));
                        pixmap.fill_rect(r, &paint, Transform::identity(), None);
                    }
                }
            }
        }

        for quad in state.compute_kerning_indicator_quads() {
            let fill = color32_to_tiny_skia(if quad.kerning_value_upm < 0 {
                state.theme.kerning_negative_fill_color32()
            } else {
                state.theme.kerning_positive_fill_color32()
            });
            let mut pb = PathBuilder::new();
            let s0 = state.viewport.to_screen(quad.corners_upm[0]);
            pb.move_to(s0.x.0, s0.y.0);
            for pt in &quad.corners_upm[1..] {
                let s = state.viewport.to_screen(*pt);
                pb.line_to(s.x.0, s.y.0);
            }
            pb.close();
            if let Some(path) = pb.finish() {
                let mut paint = Paint::default();
                paint.set_color(fill);
                paint.anti_alias = true;
                pixmap.fill_path(
                    &path,
                    &paint,
                    FillRule::Winding,
                    Transform::identity(),
                    None,
                );
            }
        }

        let passive_col = color32_to_tiny_skia(state.theme.passive_glyph_fill_color32());
        for (slot_idx, slot) in state.text_stream.slots.iter().enumerate() {
            if slot_idx == state.text_stream.active_slot_index
                && state.active_tool != CanvasTool::Text
            {
                continue;
            }
            fill_slot_contours_ts(pixmap, state, slot, passive_col);
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
            stroke_line_ts(
                pixmap,
                p_bot.x.0,
                p_bot.y.0,
                p_top.x.0,
                p_top.y.0,
                color32_to_tiny_skia(state.theme.text_caret_color32()),
                TEXT_CARET_STROKE_WIDTH_DP,
            );
        }

        if state.should_draw_text_mode_numbers() {
            let num_col = color32_to_tiny_skia(state.theme.text_metrics_number_color32());
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
                draw_bitmap_label_ts(
                    pixmap,
                    l_pt.x.0,
                    l_pt.y.0 + TEXT_METRICS_NUMBER_OFFSET_DP,
                    &format!("{}", lsb.round() as i64),
                    num_col,
                );
                draw_bitmap_label_ts(
                    pixmap,
                    r_pt.x.0,
                    r_pt.y.0 + TEXT_METRICS_NUMBER_OFFSET_DP,
                    &format!("{}", rsb.round() as i64),
                    num_col,
                );
                if slot_idx > 0 && slot.kern_with_prev != 0 {
                    let k_pt = state
                        .viewport
                        .to_screen(FontPoint::new(slot.origin_x_upm, state.metrics.descender));
                    draw_bitmap_label_ts(
                        pixmap,
                        k_pt.x.0,
                        k_pt.y.0 + TEXT_METRICS_NUMBER_OFFSET_DP * 2.0,
                        &format!("{:+}", slot.kern_with_prev),
                        num_col,
                    );
                }
            }
        }
    }

    if state.show_text_measurement_line || state.temp_show_text_measurement_line {
        render_scanline_ts(pixmap, state);
    }
}

fn fill_slot_contours_ts(
    pixmap: &mut Pixmap,
    state: &GrafiaCanvasState,
    slot: &crate::core::GlyphSlot,
    color: tiny_skia::Color,
) {
    let (contours, closed) = shifted_slot_contours(slot);
    let mut pb = PathBuilder::new();
    for (contour, is_closed) in contours.iter().zip(closed.iter()) {
        if !*is_closed || contour.len() < 2 {
            continue;
        }
        let s0 = state.viewport.to_screen(contour[0].point);
        pb.move_to(s0.x.0, s0.y.0);
        for seg in extract_segments(contour, true) {
            match seg {
                ContourSegment::Line { end_idx, .. } => {
                    let p = state.viewport.to_screen(contour[end_idx].point);
                    pb.line_to(p.x.0, p.y.0);
                }
                ContourSegment::Cubic {
                    h1_idx,
                    h2_idx,
                    end_idx,
                    ..
                } => {
                    let h1 = state.viewport.to_screen(contour[h1_idx].point);
                    let h2 = state.viewport.to_screen(contour[h2_idx].point);
                    let p3 = state.viewport.to_screen(contour[end_idx].point);
                    pb.cubic_to(h1.x.0, h1.y.0, h2.x.0, h2.y.0, p3.x.0, p3.y.0);
                }
            }
        }
        pb.close();
    }
    if let Some(path) = pb.finish() {
        let mut paint = Paint::default();
        paint.set_color(color);
        paint.anti_alias = true;
        pixmap.fill_path(
            &path,
            &paint,
            FillRule::Winding,
            Transform::identity(),
            None,
        );
    }
}

fn render_scanline_ts(pixmap: &mut Pixmap, state: &GrafiaCanvasState) {
    let scans = state.compute_text_scanline_measurements();
    let y_upm = state.text_measurement_y_upm;
    let x_start = scans.first().map_or(0.0, |s| s.lsb_x_upm);
    let x_end = scans.last().map_or(state.glyph.width, |s| s.rsb_x_upm);
    let p0 = state.viewport.to_screen(FontPoint::new(x_start, y_upm));
    let p1 = state.viewport.to_screen(FontPoint::new(x_end, y_upm));
    stroke_line_ts(
        pixmap,
        p0.x.0,
        p0.y.0,
        p1.x.0,
        p1.y.0,
        color32_to_tiny_skia(state.theme.text_measurement_line_color32()),
        TEXT_MEASUREMENT_LINE_STROKE_WIDTH_DP,
    );
    let knob_col = color32_to_tiny_skia(state.theme.text_measurement_knob_fill_color32());
    let num_col = color32_to_tiny_skia(state.theme.text_metrics_number_color32());
    let draw_nums = state.should_draw_text_mode_numbers();

    for scan in &scans {
        for x in [scan.lsb_x_upm, scan.rsb_x_upm]
            .into_iter()
            .chain(scan.contour_hit_xs_upm.iter().copied())
        {
            let s = state.viewport.to_screen(FontPoint::new(x, scan.y_upm));
            fill_circle_ts(
                pixmap,
                s.x.0,
                s.y.0,
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
                draw_bitmap_label_ts(
                    pixmap,
                    s.x.0,
                    s.y.0 - TEXT_METRICS_NUMBER_OFFSET_DP,
                    &format_measurement_value(m_lsb),
                    num_col,
                );
            }
            if let (Some(m_rsb), Some(&last_x)) =
                (scan.measured_rsb_upm, scan.contour_hit_xs_upm.last())
            {
                let s = state
                    .viewport
                    .to_screen(FontPoint::new((last_x + scan.rsb_x_upm) * 0.5, scan.y_upm));
                draw_bitmap_label_ts(
                    pixmap,
                    s.x.0,
                    s.y.0 - TEXT_METRICS_NUMBER_OFFSET_DP,
                    &format_measurement_value(m_rsb),
                    num_col,
                );
            }
        }
    }
}
