//! Multi-pass canvas rendering into `egui::Painter` (STD-ARCH).

use crate::adapters::cursor_overlay::paint_pen_extras_and_cursor_egui;
use crate::adapters::state::{CanvasTool, CanvasViewMode, GrafiaCanvasState};
use crate::core::{extract_segments, ContourSegment, FontPoint, NodeType};
use crate::tokens::{
    GrafiaTheme, CURVE_FLATTEN_TOLERANCE, DEFAULT_ASCENDER_UPM, DEFAULT_BASELINE_UPM,
    DEFAULT_CAP_HEIGHT_UPM, DEFAULT_DESCENDER_UPM, DEFAULT_X_HEIGHT_UPM,
    METRICS_LABEL_FONT_SIZE_DP, METRICS_LABEL_OFFSET_X_DP, METRICS_LABEL_OFFSET_Y_DP,
};
use egui::{Align2, FontId, Painter, Pos2, Rect, Shape, Stroke};
use kurbo::PathEl;

pub struct CanvasPainter<'a> {
    pub painter: &'a Painter,
    pub canvas_rect: Rect,
    pub state: &'a GrafiaCanvasState,
    pub theme: &'a GrafiaTheme,
}

impl<'a> CanvasPainter<'a> {
    #[must_use]
    pub fn new(
        painter: &'a Painter,
        canvas_rect: Rect,
        state: &'a GrafiaCanvasState,
        theme: &'a GrafiaTheme,
    ) -> Self {
        Self {
            painter,
            canvas_rect,
            state,
            theme,
        }
    }

    pub fn paint(&self) {
        self.painter.rect_filled(
            self.canvas_rect,
            0.0,
            self.theme.canvas_background_color32(),
        );
        match self.state.view_mode {
            CanvasViewMode::EditWireframe => {
                self.pass_1_metrics();
                if !(self.state.active_tool == CanvasTool::Text
                    && !self.state.text_stream.slots.is_empty())
                {
                    self.pass_3_wireframe_and_tangents();
                    self.pass_4_nodes();
                }
                self.pass_5_marquee();
                crate::render::paint_v020_overlays_egui(self.painter, self.state, self.theme);
                crate::render::paint_v030_overlays_egui(self.painter, self.state);
                crate::render::paint_v040_overlays_egui(self.painter, self.state);
                crate::render::paint_v050_overlays_egui(self.painter, self.state);
                crate::render::paint_v060_overlays_egui(
                    self.painter,
                    self.state,
                    self.theme,
                    self.canvas_rect,
                );
            }
            CanvasViewMode::PreviewFilled => {
                self.pass_1_metrics();
                self.pass_2_silhouette_fill();
                crate::render::paint_v050_overlays_egui(self.painter, self.state);
                crate::render::paint_v060_overlays_egui(
                    self.painter,
                    self.state,
                    self.theme,
                    self.canvas_rect,
                );
            }
            CanvasViewMode::ShapeThumbnail => self.paint_shape_thumbnails(),
        }
        paint_pen_extras_and_cursor_egui(self.painter, self.canvas_rect, self.state, self.theme);
    }

    fn pass_1_metrics(&self) {
        let vp = self.state.active_slot_viewport();
        let baseline_y = vp.pan_offset.y.0;
        let base_stroke = Stroke::new(
            self.theme.preview_canvas.baseline_stroke_width_dp,
            self.theme.preview_canvas.baseline_color32(),
        );
        self.painter.line_segment(
            [
                Pos2::new(self.canvas_rect.left(), baseline_y),
                Pos2::new(self.canvas_rect.right(), baseline_y),
            ],
            base_stroke,
        );
        let lsb_x = vp.pan_offset.x.0;
        self.painter.line_segment(
            [
                Pos2::new(lsb_x, self.canvas_rect.top()),
                Pos2::new(lsb_x, self.canvas_rect.bottom()),
            ],
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
        let adv_w = if self.state.glyph.width > 0.0 {
            self.state.glyph.width
        } else {
            DEFAULT_X_HEIGHT_UPM
        };
        let guide_stroke = Stroke::new(
            self.theme.preview_canvas.baseline_stroke_width_dp,
            self.theme.metrics_guide_color32(),
        );
        let label_color = self.theme.metrics_label_color32();
        let font_id = FontId::proportional(METRICS_LABEL_FONT_SIZE_DP);

        let top_l = vp.to_screen(FontPoint::new(0.0, DEFAULT_ASCENDER_UPM));
        let bot_l = vp.to_screen(FontPoint::new(0.0, DEFAULT_DESCENDER_UPM));
        let top_r = vp.to_screen(FontPoint::new(adv_w, DEFAULT_ASCENDER_UPM));
        let bot_r = vp.to_screen(FontPoint::new(adv_w, DEFAULT_DESCENDER_UPM));
        self.painter.line_segment(
            [
                Pos2::new(top_l.x.0, top_l.y.0),
                Pos2::new(bot_l.x.0, bot_l.y.0),
            ],
            guide_stroke,
        );
        self.painter.line_segment(
            [
                Pos2::new(top_r.x.0, top_r.y.0),
                Pos2::new(bot_r.x.0, bot_r.y.0),
            ],
            guide_stroke,
        );

        let h_metrics = [
            ("Asc 800", DEFAULT_ASCENDER_UPM),
            ("Cap 700", DEFAULT_CAP_HEIGHT_UPM),
            ("xH 500", DEFAULT_X_HEIGHT_UPM),
            ("Base 0", DEFAULT_BASELINE_UPM),
            ("Desc -200", DEFAULT_DESCENDER_UPM),
        ];
        for (label, y_upm) in h_metrics {
            let l_scr = vp.to_screen(FontPoint::new(0.0, y_upm));
            let r_scr = vp.to_screen(FontPoint::new(adv_w, y_upm));
            self.painter.line_segment(
                [
                    Pos2::new(l_scr.x.0, l_scr.y.0),
                    Pos2::new(r_scr.x.0, r_scr.y.0),
                ],
                guide_stroke,
            );
            self.painter.text(
                Pos2::new(
                    l_scr.x.0 - METRICS_LABEL_OFFSET_X_DP,
                    l_scr.y.0 - METRICS_LABEL_OFFSET_Y_DP,
                ),
                Align2::RIGHT_BOTTOM,
                label,
                font_id.clone(),
                label_color,
            );
        }
    }

    fn pass_2_silhouette_fill(&self) {
        let vp = self.state.active_slot_viewport();
        let fill_color = self.theme.preview_canvas.fill_color32();
        let closed_flags = self.state.closed_flags();
        let mesh = crate::render::tessellate_closed_contours_nonzero(
            &self.state.contours,
            &closed_flags,
            &vp,
            fill_color,
        );
        if !mesh.is_empty() {
            self.painter.add(Shape::mesh(mesh));
        }
    }

    fn pass_3_wireframe_and_tangents(&self) {
        let vp = self.state.active_slot_viewport();
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
                        let scr = vp.to_screen(FontPoint::new(p.x, p.y));
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
                if let ContourSegment::Cubic {
                    start_idx,
                    h1_idx,
                    h2_idx,
                    end_idx,
                    ..
                } = seg
                {
                    let p0_s = vp.to_screen(contour[start_idx].point);
                    let h1_s = vp.to_screen(contour[h1_idx].point);
                    let p3_s = vp.to_screen(contour[end_idx].point);
                    let h2_s = vp.to_screen(contour[h2_idx].point);
                    self.painter.line_segment(
                        [Pos2::new(p0_s.x.0, p0_s.y.0), Pos2::new(h1_s.x.0, h1_s.y.0)],
                        bcp_line_stroke,
                    );
                    self.painter.line_segment(
                        [Pos2::new(p3_s.x.0, p3_s.y.0), Pos2::new(h2_s.x.0, h2_s.y.0)],
                        bcp_line_stroke,
                    );
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
                let a_scr = vp.to_screen(last_on.point);
                let out_scr = vp.to_screen(bcp_out);
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

        if let crate::interaction::PenToolState::DraggingHandles {
            anchor,
            bcp_out,
            bcp_in,
            ..
        } = self.state.pen_state
        {
            let a_scr = vp.to_screen(anchor);
            let a_pos = Pos2::new(a_scr.x.0, a_scr.y.0);
            let r = self.theme.edit_canvas.bcp_handle_size_dp * 0.5;
            let hc = self.theme.edit_canvas.bcp_handle_color32();
            for h_pt in [bcp_out, bcp_in] {
                if h_pt != anchor {
                    let h_scr = vp.to_screen(h_pt);
                    let h_pos = Pos2::new(h_scr.x.0, h_scr.y.0);
                    self.painter.line_segment([a_pos, h_pos], bcp_line_stroke);
                    self.painter.circle_filled(h_pos, r, hc);
                }
            }
        }
    }
}
