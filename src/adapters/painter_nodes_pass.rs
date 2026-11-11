//! Pass 4 (`nodes`), Pass 5 (`marquee`), and `ShapeThumbnail` view passes for `CanvasPainter`.

use crate::adapters::painter_pass::CanvasPainter;
use crate::adapters::state::CanvasTool;
use crate::core::NodeType;
use crate::render::{
    compute_open_contour_endpoint_ticks, compute_start_node_arrow_triangle, ProjectedNode,
    ShapeThumbnailProjection,
};
use crate::tokens::{
    CURVE_FLATTEN_TOLERANCE, GHOST_NODE_STROKE_WIDTH_DP, START_NODE_ARROW_OUTLINE_WIDTH_DP,
};
use egui::epaint::{PathShape, PathStroke};
use egui::{Pos2, Rect, Shape, Stroke, StrokeKind};

impl<'a> CanvasPainter<'a> {
    pub(crate) fn pass_4_nodes(&self) {
        let vp = self.state.active_slot_viewport();
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
                && self
                    .state
                    .pending_pen_bcp_out
                    .is_none_or(|(c, _)| c != c_idx)
                && self
                    .state
                    .pending_pen_first_bcp_in
                    .is_none_or(|(c, _)| c != c_idx);
            for node in contour {
                let is_sel = self.state.selected_nodes.contains(&node.id);
                let proj = ProjectedNode::new(
                    node.point,
                    node.node_type,
                    is_sel,
                    &vp,
                    &self.theme.edit_canvas,
                );
                let center = Pos2::new(proj.screen_pos.x.0, proj.screen_pos.y.0);
                let half = self.theme.edit_canvas.node_size_dp * 0.5;
                let node_rect = Rect::from_min_max(
                    Pos2::new(center.x - half, center.y - half),
                    Pos2::new(center.x + half, center.y + half),
                );

                if is_single_open_point {
                    let single_stroke = Stroke::new(sw, corner_color);
                    self.painter.line_segment(
                        [node_rect.left_top(), node_rect.right_bottom()],
                        single_stroke,
                    );
                    self.painter.line_segment(
                        [node_rect.left_bottom(), node_rect.right_top()],
                        single_stroke,
                    );
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
                            self.painter.circle_filled(
                                center,
                                r,
                                self.theme.edit_canvas.bcp_handle_color32(),
                            );
                        }
                    }
                }
                if is_sel {
                    let glow_rect = Rect::from_min_max(
                        Pos2::new(node_rect.left() - sw, node_rect.top() - sw),
                        Pos2::new(node_rect.right() + sw, node_rect.bottom() + sw),
                    );
                    self.painter
                        .rect_stroke(glow_rect, 0.0, glow_stroke, StrokeKind::Middle);
                }
            }

            if let Some([tip, left, right]) =
                compute_start_node_arrow_triangle(contour, is_closed, &vp)
            {
                self.painter.add(Shape::convex_polygon(
                    vec![
                        Pos2::new(tip.x.0, tip.y.0),
                        Pos2::new(left.x.0, left.y.0),
                        Pos2::new(right.x.0, right.y.0),
                    ],
                    arrow_color,
                    Stroke::new(
                        START_NODE_ARROW_OUTLINE_WIDTH_DP,
                        self.theme.canvas_background_color32(),
                    ),
                ));
            }

            if !is_closed {
                for (pa, pb) in compute_open_contour_endpoint_ticks(contour, &vp) {
                    self.painter.line_segment(
                        [Pos2::new(pa.x.0, pa.y.0), Pos2::new(pb.x.0, pb.y.0)],
                        open_tick_stroke,
                    );
                }
            }
        }

        if self.state.active_tool == CanvasTool::Pen {
            if let Some(hit) = self.state.hovered_segment_hit {
                let ghost_scr = vp.to_screen(hit.point);
                let ghost_pos = Pos2::new(ghost_scr.x.0, ghost_scr.y.0);
                let half = self.theme.edit_canvas.node_size_dp * 0.5;
                self.painter.circle_stroke(
                    ghost_pos,
                    half,
                    Stroke::new(GHOST_NODE_STROKE_WIDTH_DP, glow_color),
                );
                self.painter
                    .circle_filled(ghost_pos, half * 0.5, smooth_color);
            }
        }
    }

    pub(crate) fn pass_5_marquee(&self) {
        if let crate::interaction::SelectToolState::MarqueeSelecting {
            start_screen,
            current_screen,
        } = self.state.select_state
        {
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
            let egui_rect = Rect::from_min_max(
                Pos2::new(rect.x0 as f32, rect.y0 as f32),
                Pos2::new(rect.x1 as f32, rect.y1 as f32),
            );
            self.painter.rect_filled(
                egui_rect,
                0.0,
                self.theme.edit_canvas.rubberband_fill_color32(),
            );
            self.painter.rect_stroke(
                egui_rect,
                0.0,
                Stroke::new(
                    self.theme.edit_canvas.rubberband_stroke_width_dp,
                    self.theme.edit_canvas.rubberband_stroke_color32(),
                ),
                StrokeKind::Middle,
            );
        }
    }

    pub(crate) fn paint_shape_thumbnails(&self) {
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
            scaled_cfg.thumbnail_padding_dp =
                self.theme.shape_canvas.thumbnail_padding_dp * 2.0 * 2.0;
            let proj = ShapeThumbnailProjection::compute(path, &scaled_cfg, node_count, true);
            let card_rect = Rect::from_min_size(
                Pos2::new(self.canvas_rect.left() + pad, row_y),
                egui::Vec2::new(card_w, card_h),
            );
            self.painter.rect_filled(
                card_rect,
                self.theme.shape_canvas.selection_corner_radius_dp,
                self.theme.shape_canvas.accent_selection_color32(),
            );
            let mut points = Vec::new();
            kurbo::flatten(
                proj.fitted_path.iter(),
                CURVE_FLATTEN_TOLERANCE,
                |el| match el {
                    kurbo::PathEl::MoveTo(p) | kurbo::PathEl::LineTo(p) => {
                        points.push(Pos2::new(
                            card_rect.min.x + p.x as f32,
                            card_rect.min.y + p.y as f32,
                        ));
                    }
                    _ => {}
                },
            );
            if points.len() >= 2 {
                let active_color = self.theme.shape_canvas.text_active_color32();
                if is_closed {
                    let mesh = crate::render::tessellate_bez_path_nonzero(
                        &proj.fitted_path,
                        card_rect.min,
                        active_color,
                    );
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
                    stroke: PathStroke::new(
                        self.theme.edit_canvas.contour_stroke_width_dp,
                        active_color,
                    ),
                }));
            }
            row_y += card_h + self.theme.shape_canvas.header_height_dp;
        }
    }
}
