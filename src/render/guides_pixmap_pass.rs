//! `SPEC-004` `tiny-skia` overlay renderers for alignment zones, guides,
//! measurement rays, magnetic snappers, and alignment badges.

use crate::adapters::state::GrafiaCanvasState;
use crate::core::{NodeType, ScreenPoint};
use crate::render::guide_and_zone_overlays::{
    alignment_zone_screen_quad, badge_fill_color32_for_theme, badge_zoom_scale,
    guide_screen_segment, vertical_metric_quad,
};
use crate::render::ts_primitives::{
    fill_and_stroke_poly_ts, fill_circle_ts, fill_pill_ts, stroke_ts,
};
use crate::snapping::{AlignmentBadgeMode, SelectedGuideRef};
use crate::tokens::{
    alignment_badge_stroke_color32, color32_to_tiny_skia, BADGE_MODE1_SELECTED_SIZE_DP,
    BADGE_MODE1_UNSELECTED_SIZE_DP, BADGE_MODE234_SELECTED_SIZE_DP,
    BADGE_MODE234_UNSELECTED_SIZE_DP, BADGE_STROKE_WIDTH_DP, GUIDE_KNOB_RADIUS_DP,
    GUIDE_LINE_STROKE_WIDTH_DP, GUIDE_LOCK_BODY_HALF_W_DP, GUIDE_LOCK_BODY_H_DP,
    GUIDE_LOCK_SHACKLE_RADIUS_DP, GUIDE_LOCK_STROKE_WIDTH_DP, GUIDE_MEASUREMENT_DOT_RADIUS_DP,
    GUIDE_MEASUREMENT_PILL_OFFSET_DP, GUIDE_MEASUREMENT_TICK_HALF_DP,
    GUIDE_SELECTED_KNOB_RADIUS_DP, GUIDE_SELECTED_LINE_STROKE_WIDTH_DP, GUIDE_SELECTED_TICK_LEN_DP,
    SNAP_INDICATOR_CIRCLE_RADIUS_DP, SNAP_INDICATOR_STROKE_WIDTH_DP,
};
use tiny_skia::{FillRule, Paint, PathBuilder, Pixmap, Transform};

pub fn render_alignment_zones_tiny_skia(pixmap: &mut Pixmap, state: &GrafiaCanvasState) {
    if !state.show_metrics {
        return;
    }
    let col = state
        .theme
        .edit_canvas
        .alignment_zone_fill
        .to_tiny_skia_color();
    let mut paint = Paint::default();
    paint.set_color(col);
    paint.anti_alias = true;
    for zone in &state.metrics.alignment_zones {
        if zone.size.abs() <= 0.0 {
            continue;
        }
        let q = alignment_zone_screen_quad(state, zone.position, zone.size);
        let mut pb = PathBuilder::new();
        pb.move_to(q[0].x.0, q[0].y.0);
        for p in &q[1..] {
            pb.line_to(p.x.0, p.y.0);
        }
        pb.close();
        if let Some(path) = pb.finish() {
            pixmap.fill_path(
                &path,
                &paint,
                FillRule::Winding,
                Transform::identity(),
                None,
            );
        }
    }
}

pub fn render_guide_and_snap_overlays_tiny_skia(pixmap: &mut Pixmap, state: &GrafiaCanvasState) {
    if state.show_guides {
        let (pw, ph) = (pixmap.width() as f32, pixmap.height() as f32);
        for guide in state.guides.iter().chain(state.global_guides.iter()) {
            render_single_guide_ts(pixmap, state, guide, pw, ph);
        }
        render_guide_measurement_rays_ts(pixmap, state);
    }

    let snap_col = color32_to_tiny_skia(state.theme.edit_canvas.snap_indicator_color.to_color32());
    for snapper in &state.active_snappers {
        let (a, b) = (
            state.viewport.to_screen(snapper.from),
            state.viewport.to_screen(snapper.to),
        );
        if !snapper.is_point_target {
            let mut pb = PathBuilder::new();
            pb.move_to(a.x.0, a.y.0);
            pb.line_to(b.x.0, b.y.0);
            stroke_ts(pixmap, pb, snap_col, SNAP_INDICATOR_STROKE_WIDTH_DP);
            fill_circle_ts(
                pixmap,
                a.x.0,
                a.y.0,
                SNAP_INDICATOR_CIRCLE_RADIUS_DP,
                snap_col,
            );
        }
        fill_circle_ts(
            pixmap,
            b.x.0,
            b.y.0,
            SNAP_INDICATOR_CIRCLE_RADIUS_DP,
            snap_col,
        );
    }
}

fn render_single_guide_ts(
    pixmap: &mut Pixmap,
    state: &GrafiaCanvasState,
    guide: &crate::snapping::Guide,
    pw: f32,
    ph: f32,
) {
    let col32 = if guide.is_global {
        state.theme.edit_canvas.global_guide_color.to_color32()
    } else {
        state.theme.edit_canvas.local_guide_color.to_color32()
    };
    let ts_col = color32_to_tiny_skia(col32);
    let is_sel = state.selected_guide
        == Some(SelectedGuideRef {
            id: guide.id,
            is_global: guide.is_global,
        });
    let sw =
        crate::render::guide_band_and_loupe_overlays::effective_guide_line_width_dp(state, is_sel);
    let _ = GUIDE_LINE_STROKE_WIDTH_DP;
    let seg = crate::render::guide_band_and_loupe_overlays::clip_infinite_guide_screen_segment(
        guide, state, 0.0, 0.0, pw, ph,
    )
    .unwrap_or_else(|| guide_screen_segment(guide, state));
    crate::render::render_guide_band_tiny_skia(pixmap, guide, state, seg.0, seg.1, col32);
    let mut pb = PathBuilder::new();
    pb.move_to(seg.0.x.0, seg.0.y.0);
    pb.line_to(seg.1.x.0, seg.1.y.0);
    stroke_ts(pixmap, pb, ts_col, sw);

    let k_scr = state.viewport.to_screen(guide.position);
    if state.lock_guides || guide.locked {
        let top_y = k_scr.y.0 - GUIDE_LOCK_BODY_H_DP * 0.5;
        if let Some(r) = tiny_skia::Rect::from_xywh(
            k_scr.x.0 - GUIDE_LOCK_BODY_HALF_W_DP,
            top_y,
            GUIDE_LOCK_BODY_HALF_W_DP * 2.0,
            GUIDE_LOCK_BODY_H_DP,
        ) {
            let mut p = Paint::default();
            p.set_color(ts_col);
            pixmap.fill_rect(r, &p, Transform::identity(), None);
        }
        let mut spb = PathBuilder::new();
        spb.push_circle(k_scr.x.0, top_y, GUIDE_LOCK_SHACKLE_RADIUS_DP);
        stroke_ts(pixmap, spb, ts_col, GUIDE_LOCK_STROKE_WIDTH_DP);
    } else {
        let kr = if is_sel {
            GUIDE_SELECTED_KNOB_RADIUS_DP
        } else {
            GUIDE_KNOB_RADIUS_DP
        };
        fill_circle_ts(pixmap, k_scr.x.0, k_scr.y.0, kr, ts_col);
        if is_sel {
            let (ux, uy) = guide.direction_unit();
            let (nx, ny) = (
                (uy as f32) * GUIDE_SELECTED_TICK_LEN_DP,
                (ux as f32) * GUIDE_SELECTED_TICK_LEN_DP,
            );
            let mut tpb = PathBuilder::new();
            tpb.move_to(k_scr.x.0 - nx, k_scr.y.0 - ny);
            tpb.line_to(k_scr.x.0 + nx, k_scr.y.0 + ny);
            stroke_ts(pixmap, tpb, ts_col, GUIDE_SELECTED_LINE_STROKE_WIDTH_DP);
        }
    }
}

fn render_guide_measurement_rays_ts(pixmap: &mut Pixmap, state: &GrafiaCanvasState) {
    for ray in state.all_measurement_rays() {
        let m_col = if ray.is_global {
            state
                .theme
                .edit_canvas
                .global_guide_color
                .to_tiny_skia_color()
        } else {
            state
                .theme
                .edit_canvas
                .local_guide_color
                .to_tiny_skia_color()
        };
        let (nx, ny) = (
            (ray.direction.1 as f32) * GUIDE_MEASUREMENT_TICK_HALF_DP,
            (ray.direction.0 as f32) * GUIDE_MEASUREMENT_TICK_HALF_DP,
        );
        for pt in &ray.intersections {
            let scr = state.viewport.to_screen(*pt);
            fill_circle_ts(
                pixmap,
                scr.x.0,
                scr.y.0,
                GUIDE_MEASUREMENT_DOT_RADIUS_DP,
                m_col,
            );
            let mut tpb = PathBuilder::new();
            tpb.move_to(scr.x.0 - nx, scr.y.0 - ny);
            tpb.line_to(scr.x.0 + nx, scr.y.0 + ny);
            stroke_ts(pixmap, tpb, m_col, GUIDE_LINE_STROKE_WIDTH_DP);
        }
        for seg in &ray.segments {
            let mid = state.viewport.to_screen(seg.midpoint);
            fill_pill_ts(
                pixmap,
                mid.x.0,
                mid.y.0 - GUIDE_MEASUREMENT_PILL_OFFSET_DP,
                &format!("{}", seg.distance.round() as i64),
            );
        }
    }
}

pub fn render_alignment_badges_tiny_skia(pixmap: &mut Pixmap, state: &GrafiaCanvasState) {
    let vp = state.active_slot_viewport();
    let scale = badge_zoom_scale(vp.zoom);
    let _ = alignment_badge_stroke_color32();
    let stroke_col = color32_to_tiny_skia(state.theme.alignment_badge_stroke_color32());
    for node in state.contours.iter().flatten() {
        if node.node_type == NodeType::OffCurve {
            continue;
        }
        let Some(mode) = state.classify_node_alignment(node.point) else {
            continue;
        };
        let is_sel = state.selected_nodes.contains(&node.id);
        let scr = vp.to_screen(node.point);
        let fill_col = color32_to_tiny_skia(badge_fill_color32_for_theme(mode, &state.theme));
        let base_sz = match (mode, is_sel) {
            (AlignmentBadgeMode::OnMetricLine, false) => BADGE_MODE1_UNSELECTED_SIZE_DP,
            (AlignmentBadgeMode::OnMetricLine, true) => BADGE_MODE1_SELECTED_SIZE_DP,
            (_, false) => BADGE_MODE234_UNSELECTED_SIZE_DP,
            (_, true) => BADGE_MODE234_SELECTED_SIZE_DP,
        };
        let half = base_sz * scale * 0.5;
        match mode {
            AlignmentBadgeMode::OnMetricLine
            | AlignmentBadgeMode::OnLocalGuide
            | AlignmentBadgeMode::OnGlobalGuide => {
                let pts = [
                    ScreenPoint::new(scr.x.0, scr.y.0 - half),
                    ScreenPoint::new(scr.x.0 + half, scr.y.0),
                    ScreenPoint::new(scr.x.0, scr.y.0 + half),
                    ScreenPoint::new(scr.x.0 - half, scr.y.0),
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
