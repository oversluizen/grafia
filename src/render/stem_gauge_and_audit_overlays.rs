//! `SPEC-011` & `SPEC-012` Pass-4 overlay helpers for:
//! - $G^2$ Star (`★`) and $G^3$ Diamond (`❖`) Continuity Node Badges,
//! - Proactive Vector Outline Audit Red Arrows & Angled Handle Warnings,
//! - Diagonal Leg & Virtual Apex (`/A`, `/V`) Tangent Extension Snap Rays,
//! - Green's Theorem Visual Centroid Crosshair $\oplus$,
//! - Live Orthogonal Stem Thickness Gauge under the Cursor.

use crate::adapters::state::GrafiaCanvasState;
use crate::core::math_consts::{
    ANGLED_HANDLE_PILL_OFFSET_DP, APEX_SNAP_BADGE_RADIUS_DP, AUDIT_ARROW_LABEL_OFFSET_DP,
    AUDIT_ARROW_SHAFT_DP, AUDIT_ARROW_WING_DP, AUDIT_ARROW_WING_SPREAD_DP, CENTROID_CROSS_ARM_DP,
    CENTROID_CROSS_RADIUS_DP, COMB_STROKE_WIDTH_DP, GENIUS_BADGE_RADIUS_DP,
    GENIUS_STAR_INNER_RADIUS_DP, GENIUS_STAR_VERTEX_COUNT, TUNNI_DASH_LEN_DP, TUNNI_GAP_LEN_DP,
    TUNNI_KNOB_RADIUS_DP, TUNNI_LINE_WIDTH_DP, TUNNI_PILL_OFFSET_DP,
};
use crate::core::{
    extract_segments, resolve_outline_audit_target, resolve_tangent_extension_ray_endpoints,
    ScreenPoint, StemGaugeMatch, ViewportTransform,
};
use crate::render::ts_primitives::{
    fill_pill_ts, paint_dashed_line_egui, paint_pill_egui, ts_fill_circle, ts_stroke_circle,
    ts_stroke_dashed_line, ts_stroke_line,
};
use crate::tokens::{color32_to_tiny_skia, GrafiaTheme};
use egui::{Painter, Pos2, Stroke};
use tiny_skia::{FillRule, Paint, PathBuilder, Pixmap, Transform};

pub(crate) fn fill_heatmap_quad_ts(pixmap: &mut Pixmap, q: [ScreenPoint; 4], fill: egui::Color32) {
    let mut pb = PathBuilder::new();
    pb.move_to(q[0].x.0, q[0].y.0);
    pb.line_to(q[1].x.0, q[1].y.0);
    pb.line_to(q[2].x.0, q[2].y.0);
    pb.line_to(q[3].x.0, q[3].y.0);
    pb.close();
    if let Some(path) = pb.finish() {
        let mut paint = Paint::default();
        paint.set_color(color32_to_tiny_skia(fill));
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

pub(crate) fn star_badge_points(center: ScreenPoint) -> Vec<Pos2> {
    (0..GENIUS_STAR_VERTEX_COUNT)
        .map(|idx| {
            let angle = (idx as f32) * (std::f32::consts::TAU / (GENIUS_STAR_VERTEX_COUNT as f32))
                - std::f32::consts::FRAC_PI_2;
            let r = if idx % 2 == 0 {
                GENIUS_BADGE_RADIUS_DP
            } else {
                GENIUS_STAR_INNER_RADIUS_DP
            };
            Pos2::new(center.x.0 + r * angle.cos(), center.y.0 + r * angle.sin())
        })
        .collect()
}

pub(crate) fn diamond_badge_points(center: ScreenPoint) -> [Pos2; 4] {
    let r = GENIUS_BADGE_RADIUS_DP;
    [
        Pos2::new(center.x.0, center.y.0 - r),
        Pos2::new(center.x.0 + r, center.y.0),
        Pos2::new(center.x.0, center.y.0 + r),
        Pos2::new(center.x.0 - r, center.y.0),
    ]
}

fn resolve_hovered_stem_measurement(
    state: &GrafiaCanvasState,
) -> Option<crate::core::StemThicknessMeasurement> {
    if !state.curve_harmony.show_stem_gauge {
        return None;
    }
    let hit = state.hovered_segment_hit.as_ref()?;
    let contour = state.contours.get(hit.contour_idx)?;
    let segs = extract_segments(contour, state.is_contour_closed(hit.contour_idx));
    let seg_idx = segs.iter().position(|s| *s == hit.segment).unwrap_or(0);
    state.measure_orthogonal_stem_thickness(hit.contour_idx, seg_idx, hit.t)
}

pub(crate) fn paint_centroid_and_stem_gauge_egui(
    painter: &Painter,
    state: &GrafiaCanvasState,
    theme: &GrafiaTheme,
    vp: &ViewportTransform,
) {
    if state.curve_harmony.show_visual_centroid {
        let c = state.compute_layer_counter_balance().overall_centroid;
        let s = vp.to_screen(c);
        let stroke = Stroke::new(
            TUNNI_LINE_WIDTH_DP,
            theme.edit_canvas.selected_glow_color32(),
        );
        painter.circle_stroke(Pos2::new(s.x.0, s.y.0), CENTROID_CROSS_RADIUS_DP, stroke);
        painter.line_segment(
            [
                Pos2::new(s.x.0 - CENTROID_CROSS_ARM_DP, s.y.0),
                Pos2::new(s.x.0 + CENTROID_CROSS_ARM_DP, s.y.0),
            ],
            stroke,
        );
        painter.line_segment(
            [
                Pos2::new(s.x.0, s.y.0 - CENTROID_CROSS_ARM_DP),
                Pos2::new(s.x.0, s.y.0 + CENTROID_CROSS_ARM_DP),
            ],
            stroke,
        );
    }
    if let Some(meas) = resolve_hovered_stem_measurement(state) {
        let col = match meas.gauge_match {
            StemGaugeMatch::Matched => theme.edit_canvas.smooth_node_color32(),
            StemGaugeMatch::Thinner | StemGaugeMatch::Thicker => {
                theme.master_incompat_highlight_color32()
            }
        };
        let stroke = Stroke::new(TUNNI_LINE_WIDTH_DP, col);
        let p0 = vp.to_screen(meas.p_entry);
        let p1 = vp.to_screen(meas.p_exit);
        painter.line_segment(
            [Pos2::new(p0.x.0, p0.y.0), Pos2::new(p1.x.0, p1.y.0)],
            stroke,
        );
        painter.circle_filled(Pos2::new(p0.x.0, p0.y.0), TUNNI_KNOB_RADIUS_DP, col);
        painter.circle_filled(Pos2::new(p1.x.0, p1.y.0), TUNNI_KNOB_RADIUS_DP, col);
        let mid = Pos2::new(
            0.5 * (p0.x.0 + p1.x.0) + TUNNI_PILL_OFFSET_DP * 0.5,
            0.5 * (p0.y.0 + p1.y.0) - TUNNI_PILL_OFFSET_DP * 0.5,
        );
        let lbl = format!("{:.1}", meas.thickness_upm);
        paint_pill_egui(painter, mid, &lbl);
    }
}

pub(crate) fn render_centroid_and_stem_gauge_tiny_skia(
    pixmap: &mut Pixmap,
    state: &GrafiaCanvasState,
    theme: &GrafiaTheme,
    vp: &ViewportTransform,
) {
    if state.curve_harmony.show_visual_centroid {
        let c = state.compute_layer_counter_balance().overall_centroid;
        let s = vp.to_screen(c);
        let col = theme.edit_canvas.selected_glow_color32();
        ts_stroke_circle(
            pixmap,
            s,
            CENTROID_CROSS_RADIUS_DP,
            col,
            TUNNI_LINE_WIDTH_DP,
        );
        ts_stroke_line(
            pixmap,
            ScreenPoint::new(s.x.0 - CENTROID_CROSS_ARM_DP, s.y.0),
            ScreenPoint::new(s.x.0 + CENTROID_CROSS_ARM_DP, s.y.0),
            col,
            TUNNI_LINE_WIDTH_DP,
        );
        ts_stroke_line(
            pixmap,
            ScreenPoint::new(s.x.0, s.y.0 - CENTROID_CROSS_ARM_DP),
            ScreenPoint::new(s.x.0, s.y.0 + CENTROID_CROSS_ARM_DP),
            col,
            TUNNI_LINE_WIDTH_DP,
        );
    }
    if let Some(meas) = resolve_hovered_stem_measurement(state) {
        let col = match meas.gauge_match {
            StemGaugeMatch::Matched => theme.edit_canvas.smooth_node_color32(),
            StemGaugeMatch::Thinner | StemGaugeMatch::Thicker => {
                theme.master_incompat_highlight_color32()
            }
        };
        let p0 = vp.to_screen(meas.p_entry);
        let p1 = vp.to_screen(meas.p_exit);
        ts_stroke_line(pixmap, p0, p1, col, TUNNI_LINE_WIDTH_DP);
        ts_fill_circle(pixmap, p0, TUNNI_KNOB_RADIUS_DP, col);
        ts_fill_circle(pixmap, p1, TUNNI_KNOB_RADIUS_DP, col);
        let lbl = format!("{:.1}", meas.thickness_upm);
        fill_pill_ts(
            pixmap,
            0.5 * (p0.x.0 + p1.x.0) + TUNNI_PILL_OFFSET_DP * 0.5,
            0.5 * (p0.y.0 + p1.y.0) - TUNNI_PILL_OFFSET_DP * 0.5,
            &lbl,
        );
    }
}

pub(crate) fn paint_outline_audit_egui(
    painter: &Painter,
    state: &GrafiaCanvasState,
    theme: &GrafiaTheme,
    vp: &ViewportTransform,
) {
    if !state.curve_harmony.show_outline_audit {
        return;
    }
    let warn_col = theme.master_incompat_highlight_color32();
    let warn_stroke = Stroke::new(TUNNI_LINE_WIDTH_DP, warn_col);
    for finding in state.audit_layer_outlines() {
        if let Some((pt, label)) = resolve_outline_audit_target(&state.glyph.contours, finding.kind)
        {
            let s = vp.to_screen(pt);
            let tail = Pos2::new(s.x.0 + AUDIT_ARROW_SHAFT_DP, s.y.0 - AUDIT_ARROW_SHAFT_DP);
            let tip = Pos2::new(s.x.0, s.y.0);
            let w1 = Pos2::new(
                s.x.0 + AUDIT_ARROW_WING_DP,
                s.y.0 - AUDIT_ARROW_WING_SPREAD_DP * 0.5,
            );
            let w2 = Pos2::new(
                s.x.0 + AUDIT_ARROW_WING_SPREAD_DP * 0.5,
                s.y.0 - AUDIT_ARROW_WING_DP,
            );
            painter.line_segment([tail, tip], warn_stroke);
            painter.line_segment([tip, w1], warn_stroke);
            painter.line_segment([tip, w2], warn_stroke);
            painter.circle_stroke(tip, TUNNI_KNOB_RADIUS_DP, warn_stroke);
            paint_pill_egui(
                painter,
                Pos2::new(
                    tail.x + AUDIT_ARROW_LABEL_OFFSET_DP,
                    tail.y - AUDIT_ARROW_LABEL_OFFSET_DP * 0.5,
                ),
                label,
            );
        }
    }
    for w in state.compute_angled_handle_warnings() {
        if let (Some(sn), Some(hn)) = (
            state.find_node(w.smooth_node_id),
            state.find_node(w.handle_node_id),
        ) {
            let ss = vp.to_screen(sn.point);
            let sh = vp.to_screen(hn.point);
            let h_pos = Pos2::new(sh.x.0, sh.y.0);
            painter.line_segment([Pos2::new(ss.x.0, ss.y.0), h_pos], warn_stroke);
            painter.circle_filled(h_pos, TUNNI_KNOB_RADIUS_DP, warn_col);
            painter.circle_stroke(
                h_pos,
                TUNNI_KNOB_RADIUS_DP + COMB_STROKE_WIDTH_DP,
                warn_stroke,
            );
            let deg_lbl = format!("{:.1}°", w.deviation_deg);
            paint_pill_egui(
                painter,
                Pos2::new(
                    sh.x.0 + ANGLED_HANDLE_PILL_OFFSET_DP,
                    sh.y.0 - ANGLED_HANDLE_PILL_OFFSET_DP * 0.5,
                ),
                &deg_lbl,
            );
        }
    }
}

pub(crate) fn paint_tangent_snap_egui(
    painter: &Painter,
    state: &GrafiaCanvasState,
    theme: &GrafiaTheme,
    vp: &ViewportTransform,
    snap: crate::core::TangentExtensionSnap,
) {
    let col = theme.edit_canvas.selected_glow_color32();
    let stroke = Stroke::new(TUNNI_LINE_WIDTH_DP, col);
    let ref_contours = state.build_drag_reference_contours();
    for (a, b) in resolve_tangent_extension_ray_endpoints(&ref_contours, snap) {
        paint_dashed_line_egui(
            painter,
            vp.to_screen(a),
            vp.to_screen(b),
            stroke,
            TUNNI_DASH_LEN_DP,
            TUNNI_GAP_LEN_DP,
        );
    }
    let s = vp.to_screen(snap.snap_point);
    let r = APEX_SNAP_BADGE_RADIUS_DP;
    painter.circle_stroke(Pos2::new(s.x.0, s.y.0), r, stroke);
    painter.line_segment(
        [Pos2::new(s.x.0 - r, s.y.0), Pos2::new(s.x.0 + r, s.y.0)],
        stroke,
    );
    painter.line_segment(
        [Pos2::new(s.x.0, s.y.0 - r), Pos2::new(s.x.0, s.y.0 + r)],
        stroke,
    );
}

pub(crate) fn render_outline_audit_tiny_skia(
    pixmap: &mut Pixmap,
    state: &GrafiaCanvasState,
    theme: &GrafiaTheme,
    vp: &ViewportTransform,
) {
    if !state.curve_harmony.show_outline_audit {
        return;
    }
    let warn_col = theme.master_incompat_highlight_color32();
    for finding in state.audit_layer_outlines() {
        if let Some((pt, label)) = resolve_outline_audit_target(&state.glyph.contours, finding.kind)
        {
            let s = vp.to_screen(pt);
            let tail = ScreenPoint::new(s.x.0 + AUDIT_ARROW_SHAFT_DP, s.y.0 - AUDIT_ARROW_SHAFT_DP);
            let w1 = ScreenPoint::new(
                s.x.0 + AUDIT_ARROW_WING_DP,
                s.y.0 - AUDIT_ARROW_WING_SPREAD_DP * 0.5,
            );
            let w2 = ScreenPoint::new(
                s.x.0 + AUDIT_ARROW_WING_SPREAD_DP * 0.5,
                s.y.0 - AUDIT_ARROW_WING_DP,
            );
            ts_stroke_line(pixmap, tail, s, warn_col, TUNNI_LINE_WIDTH_DP);
            ts_stroke_line(pixmap, s, w1, warn_col, TUNNI_LINE_WIDTH_DP);
            ts_stroke_line(pixmap, s, w2, warn_col, TUNNI_LINE_WIDTH_DP);
            ts_stroke_circle(
                pixmap,
                s,
                TUNNI_KNOB_RADIUS_DP,
                warn_col,
                TUNNI_LINE_WIDTH_DP,
            );
            fill_pill_ts(
                pixmap,
                tail.x.0 + AUDIT_ARROW_LABEL_OFFSET_DP,
                tail.y.0 - AUDIT_ARROW_LABEL_OFFSET_DP * 0.5,
                label,
            );
        }
    }
    for w in state.compute_angled_handle_warnings() {
        if let (Some(sn), Some(hn)) = (
            state.find_node(w.smooth_node_id),
            state.find_node(w.handle_node_id),
        ) {
            let ss = vp.to_screen(sn.point);
            let sh = vp.to_screen(hn.point);
            ts_stroke_line(pixmap, ss, sh, warn_col, TUNNI_LINE_WIDTH_DP);
            ts_fill_circle(pixmap, sh, TUNNI_KNOB_RADIUS_DP, warn_col);
            ts_stroke_circle(
                pixmap,
                sh,
                TUNNI_KNOB_RADIUS_DP + COMB_STROKE_WIDTH_DP,
                warn_col,
                TUNNI_LINE_WIDTH_DP,
            );
            let deg_lbl = format!("{:.1}°", w.deviation_deg);
            fill_pill_ts(
                pixmap,
                sh.x.0 + ANGLED_HANDLE_PILL_OFFSET_DP,
                sh.y.0 - ANGLED_HANDLE_PILL_OFFSET_DP * 0.5,
                &deg_lbl,
            );
        }
    }
}

pub(crate) fn render_tangent_snap_tiny_skia(
    pixmap: &mut Pixmap,
    state: &GrafiaCanvasState,
    theme: &GrafiaTheme,
    vp: &ViewportTransform,
    snap: crate::core::TangentExtensionSnap,
) {
    let col = theme.edit_canvas.selected_glow_color32();
    let ref_contours = state.build_drag_reference_contours();
    for (a, b) in resolve_tangent_extension_ray_endpoints(&ref_contours, snap) {
        ts_stroke_dashed_line(
            pixmap,
            vp.to_screen(a),
            vp.to_screen(b),
            col,
            TUNNI_LINE_WIDTH_DP,
            TUNNI_DASH_LEN_DP,
            TUNNI_GAP_LEN_DP,
        );
    }
    let s = vp.to_screen(snap.snap_point);
    let r = APEX_SNAP_BADGE_RADIUS_DP;
    ts_stroke_circle(pixmap, s, r, col, TUNNI_LINE_WIDTH_DP);
    ts_stroke_line(
        pixmap,
        ScreenPoint::new(s.x.0 - r, s.y.0),
        ScreenPoint::new(s.x.0 + r, s.y.0),
        col,
        TUNNI_LINE_WIDTH_DP,
    );
    ts_stroke_line(
        pixmap,
        ScreenPoint::new(s.x.0, s.y.0 - r),
        ScreenPoint::new(s.x.0, s.y.0 + r),
        col,
        TUNNI_LINE_WIDTH_DP,
    );
}
