//! Multi-pass canvas rendering into `egui::Painter` (STD-ARCH).

use crate::adapters::cursor_overlay::paint_pen_extras_and_cursor_egui;
use crate::adapters::state::{CanvasTool, CanvasViewMode, GlifoCanvasState};
use crate::core::{extract_segments, ContourSegment, FontPoint, NodeType};
use crate::render::{
    compute_open_contour_endpoint_ticks, compute_start_node_arrow_triangle, ProjectedNode,
    ShapeThumbnailProjection,
};
use crate::tokens::{
    CURVE_FLATTEN_TOLERANCE, DEFAULT_ASCENDER_UPM, DEFAULT_BASELINE_UPM, DEFAULT_CAP_HEIGHT_UPM,
    DEFAULT_DESCENDER_UPM, DEFAULT_X_HEIGHT_UPM, GHOST_NODE_STROKE_WIDTH_DP, GlifoTheme,
    METRICS_LABEL_FONT_SIZE_DP, METRICS_LABEL_OFFSET_X_DP, METRICS_LABEL_OFFSET_Y_DP,
    START_NODE_ARROW_OUTLINE_WIDTH_DP,
};
use egui::epaint::{PathShape, PathStroke};
use egui::{Align2, FontId, Painter, Pos2, Rect, Shape, Stroke, StrokeKind};
use kurbo::PathEl;

pub struct CanvasPainter<'a> {
    pub painter: &'a Painter,
    pub canvas_rect: Rect,
    pub state: &'a GlifoCanvasState,
    pub theme: &'a GlifoTheme,
}

impl<'a> CanvasPainter<'a> {
    #[must_use]
    pub fn new(
        painter: &'a Painter,
        canvas_rect: Rect,
        state: &'a GlifoCanvasState,
        theme: &'a GlifoTheme,
    ) -> Self {
        Self { painter, canvas_rect, state, theme }
    }

    pub fn paint(&self) {
        self.painter.rect_filled(self.canvas_rect, 0.0, self.theme.canvas_background_color32());
        match self.state.view_mode {
            CanvasViewMode::EditWireframe => {
                self.pass_1_metrics();
                self.pass_3_wireframe_and_tangents();
                self.pass_4_nodes();
                self.pass_5_marquee();
                crate::render::paint_v020_overlays_egui(self.painter, self.state, self.theme);
                crate::render::paint_v030_overlays_egui(self.painter, self.state);
                crate::render::paint_v040_overlays_egui(self.painter, self.state);
            }
            CanvasViewMode::PreviewFilled => {
                self.pass_1_metrics();
                self.pass_2_silhouette_fill();
            }
            CanvasViewMode::ShapeThumbnail => self.paint_shape_thumbnails(),
        }
        paint_pen_extras_and_cursor_egui(self.painter, self.canvas_rect, self.state, self.theme);
    }

    fn pass_1_metrics(&self) {
        let baseline_y = self.state.viewport.pan_offset.y.0;
        let base_stroke = Stroke::new(
            self.theme.preview_canvas.baseline_stroke_width_dp,
            self.theme.preview_canvas.baseline_color32(),
        );
        self.painter.line_segment(
            [Pos2::new(self.canvas_rect.left(), baseline_y), Pos2::new(self.canvas_rect.right(), baseline_y)],
            base_stroke,
        );
        let lsb_x = self.state.viewport.pan_offset.x.0;
        self.painter.line_segment(
            [Pos2::new(lsb_x, self.canvas_rect.top()), Pos2::new(lsb_x, self.canvas_rect.bottom())],
            base_stroke,
        );
        if self.state.view_mode != CanvasViewMode::EditWireframe {
            return;
        }
        crate::render::paint_v030_grid_egui(self.painter, self.canvas_rect, self.state);
        if !self.state.show_metrics {
            return;
        }
        crate::render::paint_v030_alignment_zones_egui(self.painter, self.canvas_rect, self.state);
        let adv_w = if self.state.glyph.width > 0.0 { self.state.glyph.width } else { DEFAULT_X_HEIGHT_UPM };
        let guide_stroke = Stroke::new(
            self.theme.preview_canvas.baseline_stroke_width_dp,
            self.theme.metrics_guide_color32(),
        );
        let label_color = self.theme.metrics_label_color32();
        let font_id = FontId::proportional(METRICS_LABEL_FONT_SIZE_DP);

        let top_l = self.state.viewport.to_screen(FontPoint::new(0.0, DEFAULT_ASCENDER_UPM));
        let bot_l = self.state.viewport.to_screen(FontPoint::new(0.0, DEFAULT_DESCENDER_UPM));
        let top_r = self.state.viewport.to_screen(FontPoint::new(adv_w, DEFAULT_ASCENDER_UPM));
        let bot_r = self.state.viewport.to_screen(FontPoint::new(adv_w, DEFAULT_DESCENDER_UPM));
        self.painter.line_segment([Pos2::new(top_l.x.0, top_l.y.0), Pos2::new(bot_l.x.0, bot_l.y.0)], guide_stroke);
        self.painter.line_segment([Pos2::new(top_r.x.0, top_r.y.0), Pos2::new(bot_r.x.0, bot_r.y.0)], guide_stroke);

        let h_metrics = [
            ("Asc 800", DEFAULT_ASCENDER_UPM),
            ("Cap 700", DEFAULT_CAP_HEIGHT_UPM),
            ("xH 500", DEFAULT_X_HEIGHT_UPM),
            ("Base 0", DEFAULT_BASELINE_UPM),
            ("Desc -200", DEFAULT_DESCENDER_UPM),
        ];
        for (label, y_upm) in h_metrics {
            let l_scr = self.state.viewport.to_screen(FontPoint::new(0.0, y_upm));
            let r_scr = self.state.viewport.to_screen(FontPoint::new(adv_w, y_upm));
            self.painter.line_segment([Pos2::new(l_scr.x.0, l_scr.y.0), Pos2::new(r_scr.x.0, r_scr.y.0)], guide_stroke);
            self.painter.text(
                Pos2::new(l_scr.x.0 - METRICS_LABEL_OFFSET_X_DP, l_scr.y.0 - METRICS_LABEL_OFFSET_Y_DP),
                Align2::RIGHT_BOTTOM,
                label,
                font_id.clone(),
                label_color,
            );
        }
    }

    fn pass_2_silhouette_fill(&self) {
        let fill_color = self.theme.preview_canvas.fill_color32();
        let closed_flags = self.state.closed_flags();
        let mesh = crate::render::tessellate_closed_contours_nonzero(
            &self.state.contours,
            &closed_flags,
            &self.state.viewport,
            fill_color,
        );
        if !mesh.is_empty() {
            self.painter.add(Shape::mesh(mesh));
        }
    }

    fn pass_3_wireframe_and_tangents(&self) {
        let contour_stroke = Stroke::new(
            self.theme.edit_canvas.contour_stroke_width_dp,
            self.theme.preview_canvas.fill_color32(),
        );
        let bcp_line_stroke = Stroke::new(
            self.theme.edit_canvas.bcp_line_width_dp,
            self.theme.edit_canvas.bcp_line_color32(),
        );

        for (c_idx, contour) in self.state.contours.iter().enumerate() {
            if contour.len() < 2 {
                continue;
            }
            let is_closed = self.state.is_contour_closed(c_idx);
            let mut points: Vec<Pos2> = Vec::new();
            if let Some(bez_path) = self.state.paths.get(c_idx) {
                kurbo::flatten(bez_path.iter(), CURVE_FLATTEN_TOLERANCE, |el| match el {
                    PathEl::MoveTo(p) | PathEl::LineTo(p) => {
                        let scr = self.state.viewport.to_screen(FontPoint::new(p.x, p.y));
                        points.push(Pos2::new(scr.x.0, scr.y.0));
                    }
                    _ => {}
                });
            }
            if points.len() >= 2 {
                if is_closed {
                    self.painter.add(Shape::closed_line(points, contour_stroke));
                } else {
                    self.painter.add(Shape::line(points, contour_stroke));
                }
            }

            for seg in extract_segments(contour, is_closed) {
                if let ContourSegment::Cubic { start_idx, h1_idx, h2_idx, end_idx, .. } = seg {
                    let p0_s = self.state.viewport.to_screen(contour[start_idx].point);
                    let h1_s = self.state.viewport.to_screen(contour[h1_idx].point);
                    let p3_s = self.state.viewport.to_screen(contour[end_idx].point);
                    let h2_s = self.state.viewport.to_screen(contour[h2_idx].point);
                    self.painter.line_segment([Pos2::new(p0_s.x.0, p0_s.y.0), Pos2::new(h1_s.x.0, h1_s.y.0)], bcp_line_stroke);
                    self.painter.line_segment([Pos2::new(p3_s.x.0, p3_s.y.0), Pos2::new(h2_s.x.0, h2_s.y.0)], bcp_line_stroke);
                }
            }
        }

        if let Some((c_idx, bcp_out)) = self.state.pending_pen_bcp_out {
            if let Some(last_on) = self
                .state
                .contours
                .get(c_idx)
                .and_then(|c| c.iter().rev().find(|n| n.node_type != NodeType::OffCurve))
            {
                let a_scr = self.state.viewport.to_screen(last_on.point);
                let out_scr = self.state.viewport.to_screen(bcp_out);
                let a_pos = Pos2::new(a_scr.x.0, a_scr.y.0);
                let out_pos = Pos2::new(out_scr.x.0, out_scr.y.0);
                self.painter.line_segment([a_pos, out_pos], bcp_line_stroke);
                self.painter.circle_filled(
                    out_pos,
                    self.theme.edit_canvas.bcp_handle_size_dp * 0.5,
                    self.theme.edit_canvas.bcp_handle_color32(),
                );
            }
        }

        if let crate::interaction::PenToolState::DraggingHandles { anchor, bcp_out, bcp_in, .. } = self.state.pen_state {
            let a_scr = self.state.viewport.to_screen(anchor);
            let a_pos = Pos2::new(a_scr.x.0, a_scr.y.0);
            let r = self.theme.edit_canvas.bcp_handle_size_dp * 0.5;
            let hc = self.theme.edit_canvas.bcp_handle_color32();
            for h_pt in [bcp_out, bcp_in] {
                if h_pt != anchor {
                    let h_scr = self.state.viewport.to_screen(h_pt);
                    let h_pos = Pos2::new(h_scr.x.0, h_scr.y.0);
                    self.painter.line_segment([a_pos, h_pos], bcp_line_stroke);
                    self.painter.circle_filled(h_pos, r, hc);
                }
            }
        }
    }

    fn pass_4_nodes(&self) {
        crate::render::paint_duplicate_warnings_egui(self.painter, self.state, self.theme);
        crate::render::paint_v030_alignment_badges_egui(self.painter, self.state);
        let corner_color = self.theme.edit_canvas.corner_node_color32();
        let smooth_color = self.theme.edit_canvas.smooth_node_color32();
        let glow_color = self.theme.edit_canvas.selected_glow_color32();
        let arrow_color = self.theme.start_node_arrow_color32();
        let sw = self.theme.edit_canvas.contour_stroke_width_dp;
        let open_tick_stroke = Stroke::new(sw, self.theme.open_endpoint_color32());
        let glow_stroke = Stroke::new(sw * 2.0, glow_color);

        for (c_idx, contour) in self.state.contours.iter().enumerate() {
            let is_closed = self.state.is_contour_closed(c_idx);
            let is_single_open_point = !is_closed
                && contour.len() == 1
                && contour[0].node_type == NodeType::Corner
                && self.state.pending_pen_bcp_out.is_none_or(|(c, _)| c != c_idx)
                && self.state.pending_pen_first_bcp_in.is_none_or(|(c, _)| c != c_idx);
            for node in contour {
                let is_sel = self.state.selected_nodes.contains(&node.id);
                let proj = ProjectedNode::new(node.point, node.node_type, is_sel, &self.state.viewport, &self.theme.edit_canvas);
                let center = Pos2::new(proj.screen_pos.x.0, proj.screen_pos.y.0);
                let half = self.theme.edit_canvas.node_size_dp * 0.5;
                let node_rect = Rect::from_min_max(Pos2::new(center.x - half, center.y - half), Pos2::new(center.x + half, center.y + half));

                if is_single_open_point {
                    let single_stroke = Stroke::new(sw, corner_color);
                    self.painter.line_segment([node_rect.left_top(), node_rect.right_bottom()], single_stroke);
                    self.painter.line_segment([node_rect.left_bottom(), node_rect.right_top()], single_stroke);
                } else {
                    match node.node_type {
                        NodeType::Corner => {
                            self.painter.rect_filled(node_rect, 0.0, corner_color);
                        }
                        NodeType::Smooth => {
                            self.painter.circle_filled(center, half, smooth_color);
                        }
                        NodeType::OffCurve => {
                            let r = self.theme.edit_canvas.bcp_handle_size_dp * 0.5;
                            self.painter.circle_filled(center, r, self.theme.edit_canvas.bcp_handle_color32());
                        }
                    }
                }
                if is_sel {
                    let glow_rect = Rect::from_min_max(
                        Pos2::new(node_rect.left() - sw, node_rect.top() - sw),
                        Pos2::new(node_rect.right() + sw, node_rect.bottom() + sw),
                    );
                    self.painter.rect_stroke(glow_rect, 0.0, glow_stroke, StrokeKind::Middle);
                }
            }

            if let Some([tip, left, right]) = compute_start_node_arrow_triangle(contour, is_closed, &self.state.viewport) {
                self.painter.add(Shape::convex_polygon(
                    vec![Pos2::new(tip.x.0, tip.y.0), Pos2::new(left.x.0, left.y.0), Pos2::new(right.x.0, right.y.0)],
                    arrow_color,
                    Stroke::new(START_NODE_ARROW_OUTLINE_WIDTH_DP, self.theme.canvas_background_color32()),
                ));
            }

            if !is_closed {
                for (pa, pb) in compute_open_contour_endpoint_ticks(contour, &self.state.viewport) {
                    self.painter.line_segment([Pos2::new(pa.x.0, pa.y.0), Pos2::new(pb.x.0, pb.y.0)], open_tick_stroke);
                }
            }
        }

        if self.state.active_tool == CanvasTool::Pen {
            if let Some(hit) = self.state.hovered_segment_hit {
                let ghost_scr = self.state.viewport.to_screen(hit.point);
                let ghost_pos = Pos2::new(ghost_scr.x.0, ghost_scr.y.0);
                let half = self.theme.edit_canvas.node_size_dp * 0.5;
                self.painter.circle_stroke(ghost_pos, half, Stroke::new(GHOST_NODE_STROKE_WIDTH_DP, glow_color));
                self.painter.circle_filled(ghost_pos, half * 0.5, smooth_color);
            }
        }
    }

    fn pass_5_marquee(&self) {
        if let crate::interaction::SelectToolState::MarqueeSelecting { start_screen, current_screen } = self.state.select_state {
            if self.state.show_bounding_box_slanted && self.state.selection_angle_deg.abs() > 0.0 {
                let verts = crate::core::slanted_marquee_parallelogram_screen(
                    start_screen,
                    current_screen,
                    self.state.selection_angle_deg,
                );
                let points: Vec<Pos2> = verts.iter().map(|p| Pos2::new(p.x.0, p.y.0)).collect();
                self.painter.add(Shape::Path(PathShape {
                    points,
                    closed: true,
                    fill: self.theme.edit_canvas.rubberband_fill_color32(),
                    stroke: PathStroke::new(
                        self.theme.edit_canvas.rubberband_stroke_width_dp,
                        self.theme.edit_canvas.rubberband_stroke_color32(),
                    ),
                }));
                return;
            }
        }
        if let Some(rect) = self.state.select_state.marquee_rect() {
            let egui_rect = Rect::from_min_max(Pos2::new(rect.x0 as f32, rect.y0 as f32), Pos2::new(rect.x1 as f32, rect.y1 as f32));
            self.painter.rect_filled(egui_rect, 0.0, self.theme.edit_canvas.rubberband_fill_color32());
            self.painter.rect_stroke(
                egui_rect,
                0.0,
                Stroke::new(self.theme.edit_canvas.rubberband_stroke_width_dp, self.theme.edit_canvas.rubberband_stroke_color32()),
                StrokeKind::Middle,
            );
        }
    }

    fn paint_shape_thumbnails(&self) {
        let pad = self.theme.shape_canvas.header_height_dp * 2.0;
        let mut row_y = self.canvas_rect.top() + pad + self.theme.shape_canvas.header_height_dp;
        let card_w = self.theme.shape_canvas.thumbnail_width_dp * 2.0 * 2.0;
        let card_h = self.theme.shape_canvas.thumbnail_height_dp * 2.0 * 2.0;

        for (idx, path) in self.state.paths.iter().enumerate() {
            let is_closed = self.state.is_contour_closed(idx);
            let node_count = self.state.contours.get(idx).map_or(0, |c| c.len());
            let mut scaled_cfg = self.theme.shape_canvas.clone();
            scaled_cfg.thumbnail_width_dp = card_w;
            scaled_cfg.thumbnail_height_dp = card_h;
            scaled_cfg.thumbnail_padding_dp = self.theme.shape_canvas.thumbnail_padding_dp * 2.0 * 2.0;
            let proj = ShapeThumbnailProjection::compute(path, &scaled_cfg, node_count, true);
            let card_rect = Rect::from_min_size(Pos2::new(self.canvas_rect.left() + pad, row_y), egui::Vec2::new(card_w, card_h));
            self.painter.rect_filled(
                card_rect,
                self.theme.shape_canvas.selection_corner_radius_dp,
                self.theme.shape_canvas.accent_selection_color32(),
            );
            let mut points = Vec::new();
            kurbo::flatten(proj.fitted_path.iter(), CURVE_FLATTEN_TOLERANCE, |el| match el {
                kurbo::PathEl::MoveTo(p) | kurbo::PathEl::LineTo(p) => {
                    points.push(Pos2::new(card_rect.min.x + p.x as f32, card_rect.min.y + p.y as f32));
                }
                _ => {}
            });
            if points.len() >= 2 {
                let active_color = self.theme.shape_canvas.text_active_color32();
                if is_closed {
                    let mesh = crate::render::tessellate_bez_path_nonzero(&proj.fitted_path, card_rect.min, active_color);
                    if !mesh.is_empty() {
                        self.painter.add(Shape::mesh(mesh));
                    }
                }
                let path_fill = if is_closed && crate::render::is_polygon_convex(&points) {
                    active_color
                } else {
                    egui::Color32::TRANSPARENT
                };
                self.painter.add(Shape::Path(PathShape {
                    points,
                    closed: is_closed,
                    fill: path_fill,
                    stroke: PathStroke::new(self.theme.edit_canvas.contour_stroke_width_dp, active_color),
                }));
            }
            row_y += card_h + self.theme.shape_canvas.header_height_dp;
        }
    }
}
