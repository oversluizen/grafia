//! Pass 4/5 Pen close-halo, `pending_pen_first_bcp_in`, and authentic Reference Vector Canvas vector cursor overlay (`SPEC-002 §3.2.4.4`, `SPEC-003 §2.1.3, §2.2, §2.3 & §2.4`).

use crate::adapters::pen_interaction::find_open_contour_close_or_resume_target;
use crate::adapters::state::{CanvasTool, CanvasViewMode, GlifoCanvasState};
use crate::adapters::v020_tools::V020DragState;
use crate::core::{BBoxAction, NodeType, ScreenPoint};
use crate::interaction::{PenToolState, SelectToolState};
use crate::tokens::{
    color32_to_tiny_skia, GHOST_NODE_STROKE_WIDTH_DP, GlifoCursorKind, GlifoTheme,
    HAND_CURSOR_DIMENSIONS_DP, LASSO_CURSOR_KNOT_INNER_RECT_DP, LASSO_CURSOR_KNOT_OUTER_RECT_DP,
    LASSO_CURSOR_LOOP_CENTER_AND_RADII_DP, LASSO_CURSOR_TAIL_INNER_POLYGON_DP,
    LASSO_CURSOR_TAIL_OUTER_POLYGON_DP, PEN_CLOSE_HALO_RADIUS_FACTOR,
    PEN_ELEMENT_BADGE_INNER_RECT_DP, PEN_ELEMENT_BADGE_OUTER_RECT_DP, PEN_NIB_BREATHER_HOLE_DP,
    PEN_NIB_COLLAR_CUFF_RECT_DP, PEN_NIB_INNER_POLYGON_DP, PEN_NIB_OUTER_POLYGON_DP,
    PEN_NIB_SLIT_Y_RANGE_AND_WIDTH_DP, PEN_PLUS_INNER_HBAR_RECT_DP, PEN_PLUS_INNER_VBAR_RECT_DP,
    PEN_PLUS_OUTER_HBAR_RECT_DP, PEN_PLUS_OUTER_VBAR_RECT_DP, ROTATE_CURSOR_GAP_RECT_DP,
    ROTATE_CURSOR_HEAD_INNER_POLYGON_DP, ROTATE_CURSOR_HEAD_OUTER_POLYGON_DP,
    ROTATE_CURSOR_RING_RADII_DP, SCALE_CURSOR_LEFT_HEAD_INNER_DP, SCALE_CURSOR_LEFT_HEAD_OUTER_DP,
    SCALE_CURSOR_RIGHT_HEAD_INNER_DP, SCALE_CURSOR_RIGHT_HEAD_OUTER_DP,
    SCALE_CURSOR_SHAFT_INNER_RECT_DP, SCALE_CURSOR_SHAFT_OUTER_RECT_DP,
    SELECT_ARROW_INNER_POLYGON_DP, SELECT_ARROW_OUTER_POLYGON_DP,
    SELECT_ELEMENT_BADGE_INNER_RECT_DP, SELECT_ELEMENT_BADGE_OUTER_RECT_DP,
    SLANT_CURSOR_BOT_BAR_INNER_DP, SLANT_CURSOR_OUTER_POLYGON_DP, SLANT_CURSOR_TOP_BAR_INNER_DP,
    ZOOM_CURSOR_HANDLE_INNER_POLYGON_DP, ZOOM_CURSOR_HANDLE_OUTER_POLYGON_DP,
    ZOOM_CURSOR_HBAR_RECT_DP, ZOOM_CURSOR_LENS_RADII_DP, ZOOM_CURSOR_VBAR_RECT_DP,
};
use egui::{Painter, Pos2, Rect, Shape, Stroke};
use tiny_skia::{FillRule, Paint, PathBuilder, Pixmap, Transform};

fn bbox_action_to_cursor(action: BBoxAction) -> GlifoCursorKind {
    match action {
        BBoxAction::Scale(_) => GlifoCursorKind::Scale,
        BBoxAction::Rotate(_) => GlifoCursorKind::Rotate,
        BBoxAction::Slant(_) => GlifoCursorKind::BBoxSlant,
    }
}

impl GlifoCanvasState {
    /// Determines which authentic Vector Canvas vector cursor (`GlifoCursorKind`) should be active at
    /// `pointer_pos`, strictly gated to `canvas_rect` (or an active canvas drag).
    #[must_use]
    pub fn desired_cursor_kind(&self, canvas_rect: Rect, pointer_pos: Option<ScreenPoint>) -> Option<GlifoCursorKind> {
        let pt = pointer_pos?;
        let active_drag = !matches!(self.select_state, SelectToolState::Idle)
            || !matches!(self.pen_state, PenToolState::Idle)
            || !matches!(self.v020_drag, V020DragState::Idle);
        if !canvas_rect.contains(Pos2::new(pt.x.0, pt.y.0)) && !active_drag { return None; }
        if matches!(self.select_state, SelectToolState::PanningViewport { .. }) { return Some(GlifoCursorKind::HandClosed); }
        if self.view_mode == CanvasViewMode::PreviewFilled { return Some(GlifoCursorKind::HandOpen); }
        if let V020DragState::BBoxDragging { action, .. } = self.v020_drag { return Some(bbox_action_to_cursor(action)); }
        if !matches!(self.select_state, SelectToolState::Idle) { return Some(GlifoCursorKind::SelectArrow); }

        let (is_ctrl, is_alt) = (self.last_modifiers.control, self.last_modifiers.option);
        match self.active_tool {
            CanvasTool::Pen => {
                if self.hovered_pen_close_node.is_some()
                    || self.hovered_pen_close_target.is_some()
                    || find_open_contour_close_or_resume_target(self, pt).is_some()
                    || self.hit_test_pen_node(pt).is_some_and(|id| self.find_node(id).is_some_and(|n| n.node_type != NodeType::OffCurve))
                {
                    Some(GlifoCursorKind::PenElement)
                } else if self.hovered_segment_hit.is_some() {
                    Some(GlifoCursorKind::PenPlus)
                } else {
                    Some(GlifoCursorKind::PenStandard)
                }
            }
            CanvasTool::Select | CanvasTool::Lasso => {
                if let Some(act) = self.hit_test_bbox_knob(pt, is_ctrl, is_alt) {
                    return Some(bbox_action_to_cursor(act));
                }
                if self.hit_test_node(pt).is_some() {
                    Some(GlifoCursorKind::SelectElement)
                } else if self.active_tool == CanvasTool::Lasso {
                    Some(GlifoCursorKind::Lasso)
                } else {
                    Some(GlifoCursorKind::SelectArrow)
                }
            }
            CanvasTool::Rotate => Some(GlifoCursorKind::Rotate),
            CanvasTool::Scale => Some(GlifoCursorKind::Scale),
            CanvasTool::Zoom => Some(if is_alt { GlifoCursorKind::ZoomOut } else { GlifoCursorKind::ZoomIn }),
            CanvasTool::Pencil => Some(GlifoCursorKind::PenStandard),
            CanvasTool::Knife
            | CanvasTool::Primitives
            | CanvasTool::Measurement
            | CanvasTool::Annotation
            | CanvasTool::Pixel => Some(GlifoCursorKind::SelectArrow),
        }
    }
}

trait CursorSink {
    fn poly(&mut self, pts: &[(f32, f32)], is_black: bool);
    fn rect(&mut self, r: [f32; 4], is_black: bool);
    fn circle(&mut self, dx: f32, dy: f32, r: f32, is_black: bool);
    fn vline(&mut self, y0: f32, y1: f32, w: f32, is_black: bool);
}

fn emit_cursor_primitives(kind: GlifoCursorKind, sink: &mut impl CursorSink) {
    match kind {
        GlifoCursorKind::SelectArrow | GlifoCursorKind::SelectElement => {
            sink.poly(SELECT_ARROW_OUTER_POLYGON_DP, false);
            sink.poly(SELECT_ARROW_INNER_POLYGON_DP, true);
            if kind == GlifoCursorKind::SelectElement {
                sink.rect(SELECT_ELEMENT_BADGE_OUTER_RECT_DP, false);
                sink.rect(SELECT_ELEMENT_BADGE_INNER_RECT_DP, true);
            }
        }
        GlifoCursorKind::PenStandard | GlifoCursorKind::PenElement | GlifoCursorKind::PenPlus => {
            sink.poly(PEN_NIB_OUTER_POLYGON_DP, false);
            sink.poly(PEN_NIB_INNER_POLYGON_DP, true);
            sink.rect(PEN_NIB_COLLAR_CUFF_RECT_DP, true);
            let slit = PEN_NIB_SLIT_Y_RANGE_AND_WIDTH_DP;
            sink.vline(slit[0], slit[1], slit[2], false);
            let hole = PEN_NIB_BREATHER_HOLE_DP;
            sink.circle(hole[0], hole[1], hole[2], false);
            if kind == GlifoCursorKind::PenElement {
                sink.rect(PEN_ELEMENT_BADGE_OUTER_RECT_DP, false);
                sink.rect(PEN_ELEMENT_BADGE_INNER_RECT_DP, true);
            } else if kind == GlifoCursorKind::PenPlus {
                sink.rect(PEN_PLUS_OUTER_VBAR_RECT_DP, false);
                sink.rect(PEN_PLUS_OUTER_HBAR_RECT_DP, false);
                sink.rect(PEN_PLUS_INNER_VBAR_RECT_DP, true);
                sink.rect(PEN_PLUS_INNER_HBAR_RECT_DP, true);
            }
        }
        GlifoCursorKind::HandOpen | GlifoCursorKind::HandClosed => {
            let [palm_r, outline_w, finger_dy] = HAND_CURSOR_DIMENSIONS_DP;
            sink.circle(0.0, 0.0, palm_r + outline_w, false);
            sink.circle(0.0, 0.0, palm_r, true);
            if kind == GlifoCursorKind::HandOpen {
                sink.circle(0.0, -finger_dy, palm_r * 0.5, true);
            }
        }
        GlifoCursorKind::ZoomIn | GlifoCursorKind::ZoomOut => {
            let [r_out, r_blk, r_in] = ZOOM_CURSOR_LENS_RADII_DP;
            sink.poly(ZOOM_CURSOR_HANDLE_OUTER_POLYGON_DP, false);
            sink.poly(ZOOM_CURSOR_HANDLE_INNER_POLYGON_DP, true);
            sink.circle(0.0, 0.0, r_out, false);
            sink.circle(0.0, 0.0, r_blk, true);
            sink.circle(0.0, 0.0, r_in, false);
            sink.rect(ZOOM_CURSOR_HBAR_RECT_DP, true);
            if kind == GlifoCursorKind::ZoomIn {
                sink.rect(ZOOM_CURSOR_VBAR_RECT_DP, true);
            }
        }
        GlifoCursorKind::Rotate => {
            let [r_out, r_blk, r_in] = ROTATE_CURSOR_RING_RADII_DP;
            sink.circle(0.0, 0.0, r_out, false);
            sink.circle(0.0, 0.0, r_blk, true);
            sink.circle(0.0, 0.0, r_in, false);
            sink.rect(ROTATE_CURSOR_GAP_RECT_DP, false);
            sink.poly(ROTATE_CURSOR_HEAD_OUTER_POLYGON_DP, false);
            sink.poly(ROTATE_CURSOR_HEAD_INNER_POLYGON_DP, true);
        }
        GlifoCursorKind::Scale => {
            sink.rect(SCALE_CURSOR_SHAFT_OUTER_RECT_DP, false);
            sink.poly(SCALE_CURSOR_LEFT_HEAD_OUTER_DP, false);
            sink.poly(SCALE_CURSOR_RIGHT_HEAD_OUTER_DP, false);
            sink.rect(SCALE_CURSOR_SHAFT_INNER_RECT_DP, true);
            sink.poly(SCALE_CURSOR_LEFT_HEAD_INNER_DP, true);
            sink.poly(SCALE_CURSOR_RIGHT_HEAD_INNER_DP, true);
        }
        GlifoCursorKind::Lasso => {
            let [cx, cy, r_out, r_blk, r_in] = LASSO_CURSOR_LOOP_CENTER_AND_RADII_DP;
            sink.poly(LASSO_CURSOR_TAIL_OUTER_POLYGON_DP, false);
            sink.circle(cx, cy, r_out, false);
            sink.circle(cx, cy, r_blk, true);
            sink.circle(cx, cy, r_in, false);
            sink.poly(LASSO_CURSOR_TAIL_INNER_POLYGON_DP, true);
            sink.rect(LASSO_CURSOR_KNOT_OUTER_RECT_DP, false);
            sink.rect(LASSO_CURSOR_KNOT_INNER_RECT_DP, true);
        }
        GlifoCursorKind::BBoxSlant => {
            sink.poly(SLANT_CURSOR_OUTER_POLYGON_DP, false);
            sink.poly(SLANT_CURSOR_TOP_BAR_INNER_DP, true);
            sink.poly(SLANT_CURSOR_BOT_BAR_INNER_DP, true);
        }
    }
}

struct EguiCursorSink<'a> {
    painter: &'a Painter,
    origin: Pos2,
    white: egui::Color32,
    black: egui::Color32,
}

impl CursorSink for EguiCursorSink<'_> {
    fn poly(&mut self, pts: &[(f32, f32)], is_black: bool) {
        let c = if is_black { self.black } else { self.white };
        let poly: Vec<Pos2> = pts.iter().map(|&(dx, dy)| Pos2::new(self.origin.x + dx, self.origin.y + dy)).collect();
        self.painter.add(Shape::convex_polygon(poly, c, Stroke::NONE));
    }
    fn rect(&mut self, r: [f32; 4], is_black: bool) {
        let c = if is_black { self.black } else { self.white };
        let rect = Rect::from_min_max(Pos2::new(self.origin.x + r[0], self.origin.y + r[1]), Pos2::new(self.origin.x + r[2], self.origin.y + r[3]));
        self.painter.rect_filled(rect, 0.0, c);
    }
    fn circle(&mut self, dx: f32, dy: f32, r: f32, is_black: bool) {
        let c = if is_black { self.black } else { self.white };
        self.painter.circle_filled(Pos2::new(self.origin.x + dx, self.origin.y + dy), r, c);
    }
    fn vline(&mut self, y0: f32, y1: f32, w: f32, is_black: bool) {
        let c = if is_black { self.black } else { self.white };
        self.painter.line_segment([Pos2::new(self.origin.x, self.origin.y + y0), Pos2::new(self.origin.x, self.origin.y + y1)], Stroke::new(w, c));
    }
}

/// Paints `pending_pen_first_bcp_in`, the `P0` closing ring/halo (`hovered_pen_close_target`),
/// and the Pass 5 custom Vector Canvas vector cursor into `egui::Painter`.
pub fn paint_pen_extras_and_cursor_egui(
    painter: &Painter,
    canvas_rect: Rect,
    state: &GlifoCanvasState,
    theme: &GlifoTheme,
) {
    if state.view_mode == CanvasViewMode::EditWireframe {
        if let Some((c_idx, first_bcp_in)) = state.pending_pen_first_bcp_in {
            if !state.is_contour_closed(c_idx) {
                if let Some(first_on) = state.contours.get(c_idx).and_then(|c| c.iter().find(|n| n.node_type != NodeType::OffCurve)) {
                    let (a_scr, in_scr) = (state.viewport.to_screen(first_on.point), state.viewport.to_screen(first_bcp_in));
                    let (a_pos, in_pos) = (Pos2::new(a_scr.x.0, a_scr.y.0), Pos2::new(in_scr.x.0, in_scr.y.0));
                    painter.line_segment([a_pos, in_pos], Stroke::new(theme.edit_canvas.bcp_line_width_dp, theme.edit_canvas.bcp_line_color32()));
                    painter.circle_filled(in_pos, theme.edit_canvas.bcp_handle_size_dp * 0.5, theme.edit_canvas.bcp_handle_color32());
                    if first_on.node_type == NodeType::Smooth {
                        painter.circle_filled(a_pos, theme.edit_canvas.node_size_dp * 0.5, theme.edit_canvas.smooth_node_color32());
                    }
                }
            }
        }
        if state.active_tool == CanvasTool::Pen {
            if let Some(close_id) = state.hovered_pen_close_node.or_else(|| state.hovered_pen_close_target.map(|(_, id)| id)) {
                if let Some(node) = state.find_node(close_id) {
                    let scr = state.viewport.to_screen(node.point);
                    let radius = theme.edit_canvas.node_size_dp * 0.5 * PEN_CLOSE_HALO_RADIUS_FACTOR;
                    painter.circle_stroke(Pos2::new(scr.x.0, scr.y.0), radius, Stroke::new(GHOST_NODE_STROKE_WIDTH_DP, theme.edit_canvas.selected_glow_color32()));
                }
            }
        }
    }
    let Some(scr_pt) = state.cursor_screen_pos else { return; };
    let Some(kind) = state.desired_cursor_kind(canvas_rect, Some(scr_pt)) else { return; };
    let mut sink = EguiCursorSink {
        painter,
        origin: Pos2::new(scr_pt.x.0, scr_pt.y.0),
        white: theme.canvas_background_color32(),
        black: theme.preview_canvas.fill_color32(),
    };
    emit_cursor_primitives(kind, &mut sink);
}

struct TinySkiaCursorSink<'a> {
    pixmap: &'a mut Pixmap,
    origin: ScreenPoint,
    white: tiny_skia::Color,
    black: tiny_skia::Color,
}

impl CursorSink for TinySkiaCursorSink<'_> {
    fn poly(&mut self, pts: &[(f32, f32)], is_black: bool) {
        let Some(&(fx, fy)) = pts.first() else { return; };
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
            self.pixmap.fill_path(&path, &paint, FillRule::Winding, Transform::identity(), None);
        }
    }
    fn rect(&mut self, r: [f32; 4], is_black: bool) {
        self.poly(&[(r[0], r[1]), (r[2], r[1]), (r[2], r[3]), (r[0], r[3])], is_black);
    }
    fn circle(&mut self, dx: f32, dy: f32, r: f32, is_black: bool) {
        fill_circle_ts(self.pixmap, self.origin.x.0 + dx, self.origin.y.0 + dy, r, if is_black { self.black } else { self.white });
    }
    fn vline(&mut self, y0: f32, y1: f32, w: f32, is_black: bool) {
        let mut pb = PathBuilder::new();
        pb.move_to(self.origin.x.0, self.origin.y.0 + y0);
        pb.line_to(self.origin.x.0, self.origin.y.0 + y1);
        if let Some(path) = pb.finish() {
            let mut paint = Paint::default();
            paint.set_color(if is_black { self.black } else { self.white });
            paint.anti_alias = true;
            let stroke = tiny_skia::Stroke { width: w, ..Default::default() };
            self.pixmap.stroke_path(&path, &paint, &stroke, Transform::identity(), None);
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
        pixmap.fill_path(&path, &paint, FillRule::Winding, Transform::identity(), None);
    }
}

/// Renders `pending_pen_first_bcp_in`, the `P0` closing ring/halo (`hovered_pen_close_target`),
/// and the Pass 5 custom Vector Canvas vector cursor into a `tiny_skia::Pixmap`.
pub fn render_pen_extras_and_cursor_tiny_skia(
    pixmap: &mut Pixmap,
    state: &GlifoCanvasState,
    theme: &GlifoTheme,
) {
    if state.view_mode == CanvasViewMode::EditWireframe {
        if let Some((c_idx, first_bcp_in)) = state.pending_pen_first_bcp_in {
            if !state.is_contour_closed(c_idx) {
                if let Some(first_on) = state.contours.get(c_idx).and_then(|c| c.iter().find(|n| n.node_type != NodeType::OffCurve)) {
                    let (a_scr, in_scr) = (state.viewport.to_screen(first_on.point), state.viewport.to_screen(first_bcp_in));
                    let mut pb = PathBuilder::new();
                    pb.move_to(a_scr.x.0, a_scr.y.0);
                    pb.line_to(in_scr.x.0, in_scr.y.0);
                    if let Some(path) = pb.finish() {
                        let mut paint = Paint::default();
                        paint.set_color(color32_to_tiny_skia(theme.edit_canvas.bcp_line_color32()));
                        paint.anti_alias = true;
                        let stroke = tiny_skia::Stroke { width: theme.edit_canvas.bcp_line_width_dp, ..Default::default() };
                        pixmap.stroke_path(&path, &paint, &stroke, Transform::identity(), None);
                    }
                    fill_circle_ts(pixmap, in_scr.x.0, in_scr.y.0, theme.edit_canvas.bcp_handle_size_dp * 0.5, color32_to_tiny_skia(theme.edit_canvas.bcp_handle_color32()));
                    if first_on.node_type == NodeType::Smooth {
                        fill_circle_ts(pixmap, a_scr.x.0, a_scr.y.0, theme.edit_canvas.node_size_dp * 0.5, color32_to_tiny_skia(theme.edit_canvas.smooth_node_color32()));
                    }
                }
            }
        }
        if state.active_tool == CanvasTool::Pen {
            if let Some(close_id) = state.hovered_pen_close_node.or_else(|| state.hovered_pen_close_target.map(|(_, id)| id)) {
                if let Some(node) = state.find_node(close_id) {
                    let scr = state.viewport.to_screen(node.point);
                    let radius = theme.edit_canvas.node_size_dp * 0.5 * PEN_CLOSE_HALO_RADIUS_FACTOR;
                    let mut pb = PathBuilder::new();
                    pb.push_circle(scr.x.0, scr.y.0, radius);
                    if let Some(path) = pb.finish() {
                        let mut paint = Paint::default();
                        paint.set_color(color32_to_tiny_skia(theme.edit_canvas.selected_glow_color32()));
                        paint.anti_alias = true;
                        let stroke = tiny_skia::Stroke { width: GHOST_NODE_STROKE_WIDTH_DP, ..Default::default() };
                        pixmap.stroke_path(&path, &paint, &stroke, Transform::identity(), None);
                    }
                }
            }
        }
    }
    let Some(scr_pt) = state.cursor_screen_pos else { return; };
    let canvas_rect = Rect::from_min_size(Pos2::ZERO, egui::vec2(pixmap.width() as f32, pixmap.height() as f32));
    let Some(kind) = state.desired_cursor_kind(canvas_rect, Some(scr_pt)) else { return; };
    let mut sink = TinySkiaCursorSink {
        pixmap,
        origin: scr_pt,
        white: color32_to_tiny_skia(theme.canvas_background_color32()),
        black: color32_to_tiny_skia(theme.preview_canvas.fill_color32()),
    };
    emit_cursor_primitives(kind, &mut sink);
}
