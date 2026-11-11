//! `SPEC-003` (`v0.2.0`) visual overlays for `egui::Painter` and `tiny_skia::Pixmap`:
//! - 8-Knob Selection Bounding Box + Width & Height Capsule Pill Badges
//! - `RotateTool` / `ScaleTool` Origin Reticle + Active Drag Ray Line
//! - `LassoTool` Freeform Closed-Loop Polygon + `ZoomTool` Marquee
//! - Coincident Node Warning Badges (`Red` `OnCurve` & `Yellow` `OffCurve`)

use crate::adapters::state::{CanvasTool, GrafiaCanvasState};
use crate::adapters::v020_tools::V020DragState;
use crate::core::DuplicateWarningKind;
use crate::render::ts_primitives::{
    fill_circle_ts, fill_pill_ts, paint_pill_egui, pill_width_dp, stroke_dashed_ts, stroke_ts,
};
use crate::tokens::{
    bbox_dashed_stroke_color32, bbox_knob_fill_color32, bbox_knob_stroke_color32,
    color32_to_tiny_skia, duplicate_curve_node_color32, duplicate_line_node_color32,
    lasso_fill_color32, lasso_stroke_color32, origin_bg_fill_color32, origin_bg_stroke_color32,
    origin_tick_color32, zoom_marquee_fill_color32, zoom_marquee_stroke_color32, GrafiaTheme,
    BBOX_BASE_HANDLE_SIZE_DP, BBOX_DASH_STROKE_WIDTH_DP, BBOX_HANDLE_ZOOM_EXP,
    BBOX_KNOB_STROKE_WIDTH_DP, BBOX_PILL_MIN_HEIGHT_DP, BBOX_PILL_OFFSET_DP, LASSO_STROKE_WIDTH_DP,
    ORIGIN_RETICLE_BG_RADIUS_DP, ORIGIN_RETICLE_CENTER_HALF_DP, ORIGIN_RETICLE_STROKE_WIDTH_DP,
    ORIGIN_RETICLE_TICK_INNER_DP, ORIGIN_RETICLE_TICK_OUTER_DP, WARNING_OFFCURVE_RADIUS_EXTRA_DP,
    WARNING_ONCURVE_RADIUS_EXTRA_DP,
};
use egui::epaint::{PathShape, PathStroke};
use egui::{Painter, Pos2, Rect, Shape, Stroke, StrokeKind};
use tiny_skia::{FillRule, Paint, PathBuilder, Pixmap, Stroke as TsStroke, Transform};

pub fn paint_duplicate_warnings_egui(
    painter: &Painter,
    state: &GrafiaCanvasState,
    theme: &GrafiaTheme,
) {
    let base_node_r = theme.edit_canvas.node_size_dp * 0.5;
    let base_off_r = theme.edit_canvas.bcp_handle_size_dp * 0.5;
    for (_id, pt, kind) in state.duplicate_warning_nodes() {
        let scr = state.viewport.to_screen(pt);
        let (col, r) = match kind {
            DuplicateWarningKind::CoincidentOnCurve => (
                duplicate_line_node_color32(),
                base_node_r + WARNING_ONCURVE_RADIUS_EXTRA_DP,
            ),
            DuplicateWarningKind::CoincidentOffCurve => (
                duplicate_curve_node_color32(),
                base_off_r + WARNING_OFFCURVE_RADIUS_EXTRA_DP,
            ),
        };
        let pos = Pos2::new(scr.x.0, scr.y.0);
        painter.circle_filled(pos, r, col);
        painter.circle_stroke(pos, r, Stroke::new(LASSO_STROKE_WIDTH_DP, col));
    }
}

pub fn render_duplicate_warnings_tiny_skia(
    pixmap: &mut Pixmap,
    state: &GrafiaCanvasState,
    theme: &GrafiaTheme,
) {
    let base_node_r = theme.edit_canvas.node_size_dp * 0.5;
    let base_off_r = theme.edit_canvas.bcp_handle_size_dp * 0.5;
    for (_id, pt, kind) in state.duplicate_warning_nodes() {
        let scr = state.viewport.to_screen(pt);
        let (col, r) = match kind {
            DuplicateWarningKind::CoincidentOnCurve => (
                duplicate_line_node_color32(),
                base_node_r + WARNING_ONCURVE_RADIUS_EXTRA_DP,
            ),
            DuplicateWarningKind::CoincidentOffCurve => (
                duplicate_curve_node_color32(),
                base_off_r + WARNING_OFFCURVE_RADIUS_EXTRA_DP,
            ),
        };
        let ts_col = color32_to_tiny_skia(col);
        fill_circle_ts(pixmap, scr.x.0, scr.y.0, r, ts_col);
        let mut pb = PathBuilder::new();
        pb.push_circle(scr.x.0, scr.y.0, r);
        stroke_ts(pixmap, pb, ts_col, LASSO_STROKE_WIDTH_DP);
    }
}

pub fn paint_v020_overlays_egui(painter: &Painter, state: &GrafiaCanvasState, theme: &GrafiaTheme) {
    if state.show_bounding_box {
        if let Some(b) = state.selection_bounds() {
            if b.is_displayable(state.viewport.zoom) {
                let c0 = b.box_screen_point(0, &state.viewport);
                let c2 = b.box_screen_point(2, &state.viewport);
                let c8 = b.box_screen_point(8, &state.viewport);
                let c6 = b.box_screen_point(6, &state.viewport);
                let stroke = Stroke::new(BBOX_DASH_STROKE_WIDTH_DP, bbox_dashed_stroke_color32());
                let corners = [
                    Pos2::new(c0.x.0, c0.y.0),
                    Pos2::new(c2.x.0, c2.y.0),
                    Pos2::new(c8.x.0, c8.y.0),
                    Pos2::new(c6.x.0, c6.y.0),
                    Pos2::new(c0.x.0, c0.y.0),
                ];
                painter.add(Shape::dashed_line(
                    &corners,
                    stroke,
                    BBOX_DASH_STROKE_WIDTH_DP,
                    BBOX_DASH_STROKE_WIDTH_DP,
                ));

                let k_r =
                    BBOX_BASE_HANDLE_SIZE_DP * state.viewport.zoom.powf(BBOX_HANDLE_ZOOM_EXP) * 0.5;
                let k_stroke = Stroke::new(BBOX_KNOB_STROKE_WIDTH_DP, bbox_knob_stroke_color32());
                for k in b.active_knobs(state.viewport.zoom) {
                    let kc = b.knob_screen_center(k, &state.viewport);
                    let pos = Pos2::new(kc.x.0, kc.y.0);
                    painter.circle_filled(pos, k_r, bbox_knob_fill_color32());
                    painter.circle_stroke(pos, k_r, k_stroke);
                }

                if let Some((w_txt, h_txt)) = state.bounding_box_pill_labels() {
                    let s_mid = b.box_screen_point(1, &state.viewport);
                    let e_mid = b.box_screen_point(5, &state.viewport);
                    let w_cy =
                        s_mid.y.0 + BBOX_PILL_OFFSET_DP * 2.0 + BBOX_PILL_MIN_HEIGHT_DP * 0.5;
                    let h_cx = e_mid.x.0 + BBOX_PILL_OFFSET_DP * 2.0 + pill_width_dp(&h_txt) * 0.5;
                    paint_pill_egui(painter, Pos2::new(s_mid.x.0, w_cy), &w_txt);
                    paint_pill_egui(painter, Pos2::new(h_cx, e_mid.y.0), &h_txt);
                }
            }
        }
    }

    if matches!(state.active_tool, CanvasTool::Rotate | CanvasTool::Scale) {
        let orig = state.viewport.to_screen(state.transform_origin);
        let center = Pos2::new(orig.x.0, orig.y.0);
        let bg_stroke = Stroke::new(ORIGIN_RETICLE_STROKE_WIDTH_DP, origin_bg_stroke_color32());
        let tick_stroke = Stroke::new(ORIGIN_RETICLE_STROKE_WIDTH_DP, origin_tick_color32());
        painter.circle_filled(
            center,
            ORIGIN_RETICLE_BG_RADIUS_DP,
            origin_bg_fill_color32(),
        );
        painter.circle_stroke(center, ORIGIN_RETICLE_BG_RADIUS_DP, bg_stroke);
        for (dx, dy) in [(-1.0, 0.0), (1.0, 0.0), (0.0, -1.0), (0.0, 1.0)] {
            painter.line_segment(
                [
                    Pos2::new(
                        center.x + dx * ORIGIN_RETICLE_TICK_INNER_DP,
                        center.y + dy * ORIGIN_RETICLE_TICK_INNER_DP,
                    ),
                    Pos2::new(
                        center.x + dx * ORIGIN_RETICLE_TICK_OUTER_DP,
                        center.y + dy * ORIGIN_RETICLE_TICK_OUTER_DP,
                    ),
                ],
                tick_stroke,
            );
        }
        let cr = Rect::from_center_size(
            center,
            egui::Vec2::splat(ORIGIN_RETICLE_CENTER_HALF_DP * 2.0),
        );
        painter.rect_filled(cr, 0.0, origin_tick_color32());

        if let V020DragState::TransformPendingOrDragging {
            active: true,
            current_scr,
            ..
        } = state.v020_drag
        {
            painter.line_segment(
                [center, Pos2::new(current_scr.x.0, current_scr.y.0)],
                bg_stroke,
            );
        }
    }

    match &state.v020_drag {
        V020DragState::LassoDragging { points_scr, .. } if points_scr.len() >= 2 => {
            let pts: Vec<Pos2> = points_scr.iter().map(|p| Pos2::new(p.x.0, p.y.0)).collect();
            painter.add(Shape::Path(PathShape {
                points: pts,
                closed: true,
                fill: lasso_fill_color32(),
                stroke: PathStroke::new(LASSO_STROKE_WIDTH_DP, lasso_stroke_color32()),
            }));
        }
        V020DragState::ZoomDragging {
            start_scr,
            current_scr,
        } => {
            let r = Rect::from_two_pos(
                Pos2::new(start_scr.x.0, start_scr.y.0),
                Pos2::new(current_scr.x.0, current_scr.y.0),
            );
            painter.rect_filled(r, 0.0, zoom_marquee_fill_color32());
            painter.rect_stroke(
                r,
                0.0,
                Stroke::new(
                    theme.edit_canvas.rubberband_stroke_width_dp,
                    zoom_marquee_stroke_color32(),
                ),
                StrokeKind::Middle,
            );
        }
        _ => {}
    }
}

pub fn render_v020_overlays_tiny_skia(
    pixmap: &mut Pixmap,
    state: &GrafiaCanvasState,
    theme: &GrafiaTheme,
) {
    if state.show_bounding_box {
        if let Some(b) = state.selection_bounds() {
            if b.is_displayable(state.viewport.zoom) {
                let c0 = b.box_screen_point(0, &state.viewport);
                let c2 = b.box_screen_point(2, &state.viewport);
                let c8 = b.box_screen_point(8, &state.viewport);
                let c6 = b.box_screen_point(6, &state.viewport);
                let mut pb = PathBuilder::new();
                pb.move_to(c0.x.0, c0.y.0);
                pb.line_to(c2.x.0, c2.y.0);
                pb.line_to(c8.x.0, c8.y.0);
                pb.line_to(c6.x.0, c6.y.0);
                pb.close();
                stroke_dashed_ts(
                    pixmap,
                    pb,
                    color32_to_tiny_skia(bbox_dashed_stroke_color32()),
                    BBOX_DASH_STROKE_WIDTH_DP,
                );

                let k_r =
                    BBOX_BASE_HANDLE_SIZE_DP * state.viewport.zoom.powf(BBOX_HANDLE_ZOOM_EXP) * 0.5;
                for k in b.active_knobs(state.viewport.zoom) {
                    let kc = b.knob_screen_center(k, &state.viewport);
                    fill_circle_ts(
                        pixmap,
                        kc.x.0,
                        kc.y.0,
                        k_r,
                        color32_to_tiny_skia(bbox_knob_fill_color32()),
                    );
                    let mut kpb = PathBuilder::new();
                    kpb.push_circle(kc.x.0, kc.y.0, k_r);
                    stroke_ts(
                        pixmap,
                        kpb,
                        color32_to_tiny_skia(bbox_knob_stroke_color32()),
                        BBOX_KNOB_STROKE_WIDTH_DP,
                    );
                }

                if let Some((w_txt, h_txt)) = state.bounding_box_pill_labels() {
                    let s_mid = b.box_screen_point(1, &state.viewport);
                    let e_mid = b.box_screen_point(5, &state.viewport);
                    let w_cy =
                        s_mid.y.0 + BBOX_PILL_OFFSET_DP * 2.0 + BBOX_PILL_MIN_HEIGHT_DP * 0.5;
                    let h_cx = e_mid.x.0 + BBOX_PILL_OFFSET_DP * 2.0 + pill_width_dp(&h_txt) * 0.5;
                    fill_pill_ts(pixmap, s_mid.x.0, w_cy, &w_txt);
                    fill_pill_ts(pixmap, h_cx, e_mid.y.0, &h_txt);
                }
            }
        }
    }

    if matches!(state.active_tool, CanvasTool::Rotate | CanvasTool::Scale) {
        let orig = state.viewport.to_screen(state.transform_origin);
        fill_circle_ts(
            pixmap,
            orig.x.0,
            orig.y.0,
            ORIGIN_RETICLE_BG_RADIUS_DP,
            color32_to_tiny_skia(origin_bg_fill_color32()),
        );
        let mut pb = PathBuilder::new();
        pb.push_circle(orig.x.0, orig.y.0, ORIGIN_RETICLE_BG_RADIUS_DP);
        if let V020DragState::TransformPendingOrDragging {
            active: true,
            current_scr,
            ..
        } = state.v020_drag
        {
            pb.move_to(orig.x.0, orig.y.0);
            pb.line_to(current_scr.x.0, current_scr.y.0);
        }
        stroke_ts(
            pixmap,
            pb,
            color32_to_tiny_skia(origin_bg_stroke_color32()),
            ORIGIN_RETICLE_STROKE_WIDTH_DP,
        );
        let mut tpb = PathBuilder::new();
        for (dx, dy) in [(-1.0, 0.0), (1.0, 0.0), (0.0, -1.0), (0.0, 1.0)] {
            tpb.move_to(
                orig.x.0 + dx * ORIGIN_RETICLE_TICK_INNER_DP,
                orig.y.0 + dy * ORIGIN_RETICLE_TICK_INNER_DP,
            );
            tpb.line_to(
                orig.x.0 + dx * ORIGIN_RETICLE_TICK_OUTER_DP,
                orig.y.0 + dy * ORIGIN_RETICLE_TICK_OUTER_DP,
            );
        }
        stroke_ts(
            pixmap,
            tpb,
            color32_to_tiny_skia(origin_tick_color32()),
            ORIGIN_RETICLE_STROKE_WIDTH_DP,
        );
        if let Some(cr) = tiny_skia::Rect::from_xywh(
            orig.x.0 - ORIGIN_RETICLE_CENTER_HALF_DP,
            orig.y.0 - ORIGIN_RETICLE_CENTER_HALF_DP,
            ORIGIN_RETICLE_CENTER_HALF_DP * 2.0,
            ORIGIN_RETICLE_CENTER_HALF_DP * 2.0,
        ) {
            let mut cp = Paint::default();
            cp.set_color(color32_to_tiny_skia(origin_tick_color32()));
            pixmap.fill_rect(cr, &cp, Transform::identity(), None);
        }
    }

    match &state.v020_drag {
        V020DragState::LassoDragging { points_scr, .. } if points_scr.len() >= 2 => {
            let mut pb = PathBuilder::new();
            pb.move_to(points_scr[0].x.0, points_scr[0].y.0);
            for p in &points_scr[1..] {
                pb.line_to(p.x.0, p.y.0);
            }
            pb.close();
            if let Some(path) = pb.finish() {
                let mut fp = Paint::default();
                fp.set_color(color32_to_tiny_skia(lasso_fill_color32()));
                fp.anti_alias = true;
                pixmap.fill_path(&path, &fp, FillRule::Winding, Transform::identity(), None);
                let mut sp = Paint::default();
                sp.set_color(color32_to_tiny_skia(lasso_stroke_color32()));
                sp.anti_alias = true;
                let s = TsStroke {
                    width: LASSO_STROKE_WIDTH_DP,
                    ..Default::default()
                };
                pixmap.stroke_path(&path, &sp, &s, Transform::identity(), None);
            }
        }
        V020DragState::ZoomDragging {
            start_scr,
            current_scr,
        } => {
            let (x0, y0) = (
                start_scr.x.0.min(current_scr.x.0),
                start_scr.y.0.min(current_scr.y.0),
            );
            let (w, h) = (
                (start_scr.x.0 - current_scr.x.0).abs().max(1.0),
                (start_scr.y.0 - current_scr.y.0).abs().max(1.0),
            );
            if let Some(r) = tiny_skia::Rect::from_xywh(x0, y0, w, h) {
                let mut fp = Paint::default();
                fp.set_color(color32_to_tiny_skia(zoom_marquee_fill_color32()));
                pixmap.fill_rect(r, &fp, Transform::identity(), None);
                let mut pb = PathBuilder::new();
                pb.push_rect(r);
                stroke_ts(
                    pixmap,
                    pb,
                    color32_to_tiny_skia(zoom_marquee_stroke_color32()),
                    theme.edit_canvas.rubberband_stroke_width_dp,
                );
            }
        }
        _ => {}
    }
}
