//! `tiny-skia` rendering of `pending_pen_first_bcp_in`, `P0` close halo, and vector cursor overlay.

use crate::adapters::cursor_overlay::{emit_cursor_primitives, CursorSink};
use crate::adapters::state::{CanvasTool, CanvasViewMode, GrafiaCanvasState};
use crate::core::{NodeType, ScreenPoint};
use crate::tokens::{
    color32_to_tiny_skia, GrafiaTheme, GHOST_NODE_STROKE_WIDTH_DP, PEN_CLOSE_HALO_RADIUS_FACTOR,
};
use egui::{Pos2, Rect};
use tiny_skia::{FillRule, Paint, PathBuilder, Pixmap, Transform};

struct TinySkiaCursorSink<'a> {
    pixmap: &'a mut Pixmap,
    origin: ScreenPoint,
    white: tiny_skia::Color,
    black: tiny_skia::Color,
}

impl CursorSink for TinySkiaCursorSink<'_> {
    fn poly(&mut self, pts: &[(f32, f32)], is_black: bool) {
        let Some(&(fx, fy)) = pts.first() else {
            return;
        };
        let mut pb = PathBuilder::new();
        pb.move_to(self.origin.x.0 + fx, self.origin.y.0 + fy);
        for &(dx, dy) in &pts[1..] {
            pb.line_to(self.origin.x.0 + dx, self.origin.y.0 + dy);
        }
        pb.close();
        if let Some(path) = pb.finish() {
            let mut paint = Paint::default();
            paint.set_color(if is_black { self.black } else { self.white });
            paint.anti_alias = true;
            self.pixmap.fill_path(
                &path,
                &paint,
                FillRule::Winding,
                Transform::identity(),
                None,
            );
        }
    }
    fn rect(&mut self, r: [f32; 4], is_black: bool) {
        self.poly(
            &[(r[0], r[1]), (r[2], r[1]), (r[2], r[3]), (r[0], r[3])],
            is_black,
        );
    }
    fn circle(&mut self, dx: f32, dy: f32, r: f32, is_black: bool) {
        fill_circle_ts(
            self.pixmap,
            self.origin.x.0 + dx,
            self.origin.y.0 + dy,
            r,
            if is_black { self.black } else { self.white },
        );
    }
    fn vline(&mut self, y0: f32, y1: f32, w: f32, is_black: bool) {
        let mut pb = PathBuilder::new();
        pb.move_to(self.origin.x.0, self.origin.y.0 + y0);
        pb.line_to(self.origin.x.0, self.origin.y.0 + y1);
        if let Some(path) = pb.finish() {
            let mut paint = Paint::default();
            paint.set_color(if is_black { self.black } else { self.white });
            paint.anti_alias = true;
            let stroke = tiny_skia::Stroke {
                width: w,
                ..Default::default()
            };
            self.pixmap
                .stroke_path(&path, &paint, &stroke, Transform::identity(), None);
        }
    }
}

fn fill_circle_ts(pixmap: &mut Pixmap, cx: f32, cy: f32, r: f32, color: tiny_skia::Color) {
    let mut pb = PathBuilder::new();
    pb.push_circle(cx, cy, r);
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

/// Renders `pending_pen_first_bcp_in`, the `P0` closing ring/halo (`hovered_pen_close_target`),
/// and the Pass 5 custom Vector Canvas vector cursor into a `tiny_skia::Pixmap`.
pub fn render_pen_extras_and_cursor_tiny_skia(
    pixmap: &mut Pixmap,
    state: &GrafiaCanvasState,
    theme: &GrafiaTheme,
) {
    if state.view_mode == CanvasViewMode::EditWireframe {
        if let Some((c_idx, first_bcp_in)) = state.pending_pen_first_bcp_in {
            if !state.is_contour_closed(c_idx) {
                if let Some(first_on) = state
                    .contours
                    .get(c_idx)
                    .and_then(|c| c.iter().find(|n| n.node_type != NodeType::OffCurve))
                {
                    let (a_scr, in_scr) = (
                        state.viewport.to_screen(first_on.point),
                        state.viewport.to_screen(first_bcp_in),
                    );
                    let mut pb = PathBuilder::new();
                    pb.move_to(a_scr.x.0, a_scr.y.0);
                    pb.line_to(in_scr.x.0, in_scr.y.0);
                    if let Some(path) = pb.finish() {
                        let mut paint = Paint::default();
                        paint.set_color(color32_to_tiny_skia(theme.edit_canvas.bcp_line_color32()));
                        paint.anti_alias = true;
                        let stroke = tiny_skia::Stroke {
                            width: theme.edit_canvas.bcp_line_width_dp,
                            ..Default::default()
                        };
                        pixmap.stroke_path(&path, &paint, &stroke, Transform::identity(), None);
                    }
                    fill_circle_ts(
                        pixmap,
                        in_scr.x.0,
                        in_scr.y.0,
                        theme.edit_canvas.bcp_handle_size_dp * 0.5,
                        color32_to_tiny_skia(theme.edit_canvas.bcp_handle_color32()),
                    );
                    if first_on.node_type == NodeType::Smooth {
                        fill_circle_ts(
                            pixmap,
                            a_scr.x.0,
                            a_scr.y.0,
                            theme.edit_canvas.node_size_dp * 0.5,
                            color32_to_tiny_skia(theme.edit_canvas.smooth_node_color32()),
                        );
                    }
                }
            }
        }
        if state.active_tool == CanvasTool::Pen {
            if let Some(close_id) = state
                .hovered_pen_close_node
                .or_else(|| state.hovered_pen_close_target.map(|(_, id)| id))
            {
                if let Some(node) = state.find_node(close_id) {
                    let scr = state.viewport.to_screen(node.point);
                    let radius =
                        theme.edit_canvas.node_size_dp * 0.5 * PEN_CLOSE_HALO_RADIUS_FACTOR;
                    let mut pb = PathBuilder::new();
                    pb.push_circle(scr.x.0, scr.y.0, radius);
                    if let Some(path) = pb.finish() {
                        let mut paint = Paint::default();
                        paint.set_color(color32_to_tiny_skia(
                            theme.edit_canvas.selected_glow_color32(),
                        ));
                        paint.anti_alias = true;
                        let stroke = tiny_skia::Stroke {
                            width: GHOST_NODE_STROKE_WIDTH_DP,
                            ..Default::default()
                        };
                        pixmap.stroke_path(&path, &paint, &stroke, Transform::identity(), None);
                    }
                }
            }
        }
    }
    let Some(scr_pt) = state.cursor_screen_pos else {
        return;
    };
    let canvas_rect = Rect::from_min_size(
        Pos2::ZERO,
        egui::vec2(pixmap.width() as f32, pixmap.height() as f32),
    );
    let Some(kind) = state.desired_cursor_kind(canvas_rect, Some(scr_pt)) else {
        return;
    };
    let mut sink = TinySkiaCursorSink {
        pixmap,
        origin: scr_pt,
        white: color32_to_tiny_skia(theme.canvas_background_color32()),
        black: color32_to_tiny_skia(theme.preview_canvas.fill_color32()),
    };
    emit_cursor_primitives(kind, &mut sink);
}
