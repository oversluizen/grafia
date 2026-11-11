//! `SPEC-004` (`grafia v0.3.0`) visual overlays for `egui::Painter` and `tiny_skia::Pixmap`:
//! - Pass 1 Alignment Zone translucent horizontal bands (`#CEA77C33`)
//! - Pass 4 Local (`#3E4CCFB3`) & Global (`#CF4F3EB3`) Guide lines, Knobs, Directional Ticks, & Padlocks
//! - Pass 4 Measurement Guide (`show_measurement == true`) intersection dots, ticks, and distance pills
//! - Pass 4 Active Magnetic Snappers (`#993333FF` `1.2 dp` lines + `4.0 dp` endpoint circles)
//! - Pass 4 Alignment Zone & Guide Node Badges (Modes `1`..`4` from `_drawAlignmentIndicator:`)

pub use crate::render::v030_overlays_ts::{
    render_v030_alignment_badges_tiny_skia, render_v030_alignment_zones_tiny_skia,
    render_v030_overlays_tiny_skia,
};

use crate::adapters::state::GrafiaCanvasState;
use crate::core::{FontPoint, NodeType, ScreenPoint};
use crate::render::ts_primitives::paint_pill_egui;
use crate::snapping::{AlignmentBadgeMode, Guide, SelectedGuideRef};
use crate::tokens::{
    alignment_badge_global_guide_fill_color32, alignment_badge_local_guide_fill_color32,
    alignment_badge_metric_fill_color32, alignment_badge_stroke_color32,
    alignment_zone_fill_color32, global_guide_color32, local_guide_color32, snap_indicator_color32,
    BADGE_MODE1_SELECTED_SIZE_DP, BADGE_MODE1_UNSELECTED_SIZE_DP, BADGE_MODE234_SELECTED_SIZE_DP,
    BADGE_MODE234_UNSELECTED_SIZE_DP, BADGE_STROKE_WIDTH_DP, BADGE_VERTICAL_RECT_EXTRA_H_DP,
    BADGE_VERTICAL_RECT_HALF_W_DP, BADGE_ZOOM_SCALE_MAX, BADGE_ZOOM_SCALE_MIN,
    BBOX_HANDLE_ZOOM_EXP, DEFAULT_X_HEIGHT_UPM, DEG_TO_RAD_F64, GUIDE_KNOB_RADIUS_DP,
    GUIDE_LINE_STROKE_WIDTH_DP, GUIDE_LOCK_BODY_HALF_W_DP, GUIDE_LOCK_BODY_H_DP,
    GUIDE_LOCK_SHACKLE_RADIUS_DP, GUIDE_LOCK_STROKE_WIDTH_DP, GUIDE_MEASUREMENT_DOT_RADIUS_DP,
    GUIDE_MEASUREMENT_PILL_OFFSET_DP, GUIDE_MEASUREMENT_TICK_HALF_DP,
    GUIDE_SELECTED_KNOB_RADIUS_DP, GUIDE_SELECTED_LINE_STROKE_WIDTH_DP, GUIDE_SELECTED_TICK_LEN_DP,
    SNAP_INDICATOR_CIRCLE_RADIUS_DP, SNAP_INDICATOR_STROKE_WIDTH_DP,
};
use egui::epaint::{PathShape, PathStroke};
use egui::{Color32, Painter, Pos2, Rect, Shape, Stroke};

pub(crate) fn alignment_zone_screen_quad(
    state: &GrafiaCanvasState,
    pos_upm: f64,
    size_upm: f64,
) -> [ScreenPoint; 4] {
    let adv_w = if state.glyph.width > 0.0 {
        state.glyph.width
    } else {
        DEFAULT_X_HEIGHT_UPM
    };
    let y0 = pos_upm.min(pos_upm + size_upm);
    let y1 = pos_upm.max(pos_upm + size_upm);
    let half_xh = state.metrics.x_height * 0.5;
    let tan_italic = (state.italic_angle_deg * DEG_TO_RAD_F64).tan();
    let dx0 = (y0 - half_xh) * tan_italic;
    let dx1 = (y1 - half_xh) * tan_italic;
    let vp = state.active_slot_viewport();
    [
        vp.to_screen(FontPoint::new(dx1, y1)),
        vp.to_screen(FontPoint::new(adv_w + dx1, y1)),
        vp.to_screen(FontPoint::new(adv_w + dx0, y0)),
        vp.to_screen(FontPoint::new(dx0, y0)),
    ]
}

pub fn paint_v030_alignment_zones_egui(
    painter: &Painter,
    _canvas_rect: Rect,
    state: &GrafiaCanvasState,
) {
    if !state.show_metrics {
        return;
    }
    let fill = state.theme.edit_canvas.alignment_zone_fill.to_color32();
    let _ = alignment_zone_fill_color32();
    for zone in &state.metrics.alignment_zones {
        if zone.size.abs() <= 0.0 {
            continue;
        }
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

pub(crate) fn guide_screen_segment(
    guide: &Guide,
    state: &GrafiaCanvasState,
) -> (ScreenPoint, ScreenPoint) {
    let (b_font, e_font) = guide.infinite_endpoints();
    (
        state.viewport.to_screen(b_font),
        state.viewport.to_screen(e_font),
    )
}

pub(crate) fn badge_zoom_scale(zoom: f32) -> f32 {
    zoom.powf(BBOX_HANDLE_ZOOM_EXP)
        .clamp(BADGE_ZOOM_SCALE_MIN, BADGE_ZOOM_SCALE_MAX)
}

pub(crate) fn badge_fill_color32(mode: AlignmentBadgeMode) -> Color32 {
    match mode {
        AlignmentBadgeMode::OnMetricLine
        | AlignmentBadgeMode::InAlignmentZone
        | AlignmentBadgeMode::OnVerticalMetric => alignment_badge_metric_fill_color32(),
        AlignmentBadgeMode::OnLocalGuide => alignment_badge_local_guide_fill_color32(),
        AlignmentBadgeMode::OnGlobalGuide => alignment_badge_global_guide_fill_color32(),
    }
}

pub(crate) fn vertical_metric_quad(
    cx: f32,
    cy: f32,
    half_h: f32,
    italic_deg: f64,
) -> [ScreenPoint; 4] {
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

pub fn paint_v030_overlays_egui(painter: &Painter, state: &GrafiaCanvasState) {
    if state.show_guides {
        for guide in state.guides.iter().chain(state.global_guides.iter()) {
            let col = if guide.is_global {
                state.theme.edit_canvas.global_guide_color.to_color32()
            } else {
                state.theme.edit_canvas.local_guide_color.to_color32()
            };
            let _ = (local_guide_color32(), global_guide_color32());
            let is_sel = state.selected_guide
                == Some(SelectedGuideRef {
                    id: guide.id,
                    is_global: guide.is_global,
                });
            let sw = if is_sel {
                GUIDE_SELECTED_LINE_STROKE_WIDTH_DP
            } else {
                GUIDE_LINE_STROKE_WIDTH_DP
            };
            let (s0, s1) = guide_screen_segment(guide, state);
            painter.line_segment(
                [Pos2::new(s0.x.0, s0.y.0), Pos2::new(s1.x.0, s1.y.0)],
                Stroke::new(sw, col),
            );

            let k_scr = state.viewport.to_screen(guide.position);
            let k_pos = Pos2::new(k_scr.x.0, k_scr.y.0);
            if state.lock_guides || guide.locked {
                let body = Rect::from_min_max(
                    Pos2::new(
                        k_pos.x - GUIDE_LOCK_BODY_HALF_W_DP,
                        k_pos.y - GUIDE_LOCK_BODY_H_DP * 0.5,
                    ),
                    Pos2::new(
                        k_pos.x + GUIDE_LOCK_BODY_HALF_W_DP,
                        k_pos.y + GUIDE_LOCK_BODY_H_DP * 0.5,
                    ),
                );
                painter.rect_filled(body, 1.0, col);
                painter.circle_stroke(
                    Pos2::new(k_pos.x, body.top()),
                    GUIDE_LOCK_SHACKLE_RADIUS_DP,
                    Stroke::new(GUIDE_LOCK_STROKE_WIDTH_DP, col),
                );
            } else {
                let kr = if is_sel {
                    GUIDE_SELECTED_KNOB_RADIUS_DP
                } else {
                    GUIDE_KNOB_RADIUS_DP
                };
                painter.circle_filled(k_pos, kr, col);
                if is_sel {
                    let (ux, uy) = guide.direction_unit();
                    let (nx, ny) = (
                        (uy as f32) * GUIDE_SELECTED_TICK_LEN_DP,
                        (ux as f32) * GUIDE_SELECTED_TICK_LEN_DP,
                    );
                    painter.line_segment(
                        [
                            Pos2::new(k_pos.x - nx, k_pos.y - ny),
                            Pos2::new(k_pos.x + nx, k_pos.y + ny),
                        ],
                        Stroke::new(GUIDE_SELECTED_LINE_STROKE_WIDTH_DP, col),
                    );
                }
            }
        }

        for ray in state.all_measurement_rays() {
            let col = if ray.is_global {
                state.theme.edit_canvas.global_guide_color.to_color32()
            } else {
                state.theme.edit_canvas.local_guide_color.to_color32()
            };
            let (nx, ny) = (
                (ray.direction.1 as f32) * GUIDE_MEASUREMENT_TICK_HALF_DP,
                (ray.direction.0 as f32) * GUIDE_MEASUREMENT_TICK_HALF_DP,
            );
            for pt in &ray.intersections {
                let scr = state.viewport.to_screen(*pt);
                let p = Pos2::new(scr.x.0, scr.y.0);
                painter.circle_filled(p, GUIDE_MEASUREMENT_DOT_RADIUS_DP, col);
                painter.line_segment(
                    [Pos2::new(p.x - nx, p.y - ny), Pos2::new(p.x + nx, p.y + ny)],
                    Stroke::new(GUIDE_LINE_STROKE_WIDTH_DP, col),
                );
            }
            for seg in &ray.segments {
                let mid = state.viewport.to_screen(seg.midpoint);
                paint_pill_egui(
                    painter,
                    Pos2::new(mid.x.0, mid.y.0 - GUIDE_MEASUREMENT_PILL_OFFSET_DP),
                    &format!("{}", seg.distance.round() as i64),
                );
            }
        }
    }

    let snap_col = state.theme.edit_canvas.snap_indicator_color.to_color32();
    let _ = snap_indicator_color32();
    let snap_stroke = Stroke::new(SNAP_INDICATOR_STROKE_WIDTH_DP, snap_col);
    for snapper in &state.active_snappers {
        let (a, b) = (
            state.viewport.to_screen(snapper.from),
            state.viewport.to_screen(snapper.to),
        );
        let (pa, pb) = (Pos2::new(a.x.0, a.y.0), Pos2::new(b.x.0, b.y.0));
        if !snapper.is_point_target {
            painter.line_segment([pa, pb], snap_stroke);
            painter.circle_filled(pa, SNAP_INDICATOR_CIRCLE_RADIUS_DP, snap_col);
        }
        painter.circle_filled(pb, SNAP_INDICATOR_CIRCLE_RADIUS_DP, snap_col);
    }
}

pub fn paint_v030_alignment_badges_egui(painter: &Painter, state: &GrafiaCanvasState) {
    let vp = state.active_slot_viewport();
    let scale = badge_zoom_scale(vp.zoom);
    let badge_stroke = Stroke::new(BADGE_STROKE_WIDTH_DP, alignment_badge_stroke_color32());
    for node in state.contours.iter().flatten() {
        if node.node_type == NodeType::OffCurve {
            continue;
        }
        let Some(mode) = state.classify_node_alignment(node.point) else {
            continue;
        };
        let is_sel = state.selected_nodes.contains(&node.id);
        let scr = vp.to_screen(node.point);
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
            AlignmentBadgeMode::OnMetricLine
            | AlignmentBadgeMode::OnLocalGuide
            | AlignmentBadgeMode::OnGlobalGuide => {
                let pts = vec![
                    Pos2::new(center.x, center.y - half),
                    Pos2::new(center.x + half, center.y),
                    Pos2::new(center.x, center.y + half),
                    Pos2::new(center.x - half, center.y),
                ];
                painter.add(Shape::Path(PathShape {
                    points: pts,
                    closed: true,
                    fill,
                    stroke: PathStroke::new(
                        BADGE_STROKE_WIDTH_DP,
                        alignment_badge_stroke_color32(),
                    ),
                }));
            }
            AlignmentBadgeMode::InAlignmentZone => {
                painter.circle_filled(center, half, fill);
                painter.circle_stroke(center, half, badge_stroke);
            }
            AlignmentBadgeMode::OnVerticalMetric => {
                let q = vertical_metric_quad(center.x, center.y, half, state.italic_angle_deg);
                let pts: Vec<Pos2> = q.iter().map(|p| Pos2::new(p.x.0, p.y.0)).collect();
                painter.add(Shape::Path(PathShape {
                    points: pts,
                    closed: true,
                    fill,
                    stroke: PathStroke::new(
                        BADGE_STROKE_WIDTH_DP,
                        alignment_badge_stroke_color32(),
                    ),
                }));
            }
        }
    }
}
