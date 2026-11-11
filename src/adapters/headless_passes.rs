//! Node, marquee, and preview silhouette passes for `tiny_skia::Pixmap` offscreen rendering.

use crate::adapters::headless::append_kurbo_to_builder;
use crate::adapters::state::{CanvasTool, GrafiaCanvasState};
use crate::core::NodeType;
use crate::render::{
    compute_open_contour_endpoint_ticks, compute_start_node_arrow_triangle, PreviewFilledProjection,
};
use crate::tokens::{
    color32_to_tiny_skia, GrafiaTheme, GHOST_NODE_STROKE_WIDTH_DP,
    START_NODE_ARROW_OUTLINE_WIDTH_DP,
};
use tiny_skia::{FillRule, Paint, PathBuilder, Pixmap, Rect, Stroke, Transform};

fn fill_circle(pixmap: &mut Pixmap, cx: f32, cy: f32, r: f32, color: tiny_skia::Color) {
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

fn stroke_pb(pixmap: &mut Pixmap, pb: PathBuilder, color: tiny_skia::Color, width: f32) {
    if let Some(path) = pb.finish() {
        let mut paint = Paint::default();
        paint.set_color(color);
        paint.anti_alias = true;
        let stroke = Stroke {
            width,
            ..Default::default()
        };
        pixmap.stroke_path(&path, &paint, &stroke, Transform::identity(), None);
    }
}

pub(crate) fn render_nodes(pixmap: &mut Pixmap, state: &GrafiaCanvasState, theme: &GrafiaTheme) {
    let vp = state.active_slot_viewport();
    crate::render::render_duplicate_warnings_tiny_skia(pixmap, state, theme);
    crate::render::render_v030_alignment_badges_tiny_skia(pixmap, state);
    let half_node = theme.edit_canvas.node_size_dp * 0.5;
    let node_size = theme.edit_canvas.node_size_dp;
    let handle_radius = theme.edit_canvas.bcp_handle_size_dp * 0.5;
    let corner_color = color32_to_tiny_skia(theme.edit_canvas.corner_node_color32());
    let smooth_color = color32_to_tiny_skia(theme.edit_canvas.smooth_node_color32());
    let handle_color = color32_to_tiny_skia(theme.edit_canvas.bcp_handle_color32());
    let arrow_color = color32_to_tiny_skia(theme.start_node_arrow_color32());
    let open_color = color32_to_tiny_skia(theme.open_endpoint_color32());
    let glow_color = color32_to_tiny_skia(theme.edit_canvas.selected_glow_color32());
    let bg_color = color32_to_tiny_skia(theme.canvas_background_color32());
    let sw = theme.edit_canvas.contour_stroke_width_dp;

    for (c_idx, contour) in state.contours.iter().enumerate() {
        let is_closed = state.is_contour_closed(c_idx);
        let is_single_open_point = !is_closed
            && contour.len() == 1
            && contour[0].node_type == NodeType::Corner
            && state.pending_pen_bcp_out.is_none_or(|(c, _)| c != c_idx)
            && state
                .pending_pen_first_bcp_in
                .is_none_or(|(c, _)| c != c_idx);
        for node in contour {
            let scr = vp.to_screen(node.point);
            let (cx, cy) = (scr.x.0, scr.y.0);
            if is_single_open_point {
                let mut pb = PathBuilder::new();
                pb.move_to(cx - half_node, cy - half_node);
                pb.line_to(cx + half_node, cy + half_node);
                pb.move_to(cx - half_node, cy + half_node);
                pb.line_to(cx + half_node, cy - half_node);
                stroke_pb(pixmap, pb, corner_color, sw);
            } else {
                match node.node_type {
                    NodeType::Corner => {
                        if let Some(rect) =
                            Rect::from_xywh(cx - half_node, cy - half_node, node_size, node_size)
                        {
                            let mut paint = Paint::default();
                            paint.set_color(corner_color);
                            pixmap.fill_rect(rect, &paint, Transform::identity(), None);
                        }
                    }
                    NodeType::Smooth => fill_circle(pixmap, cx, cy, half_node, smooth_color),
                    NodeType::OffCurve => fill_circle(pixmap, cx, cy, handle_radius, handle_color),
                }
            }

            if state.selected_nodes.contains(&node.id) {
                if let Some(g_rect) = Rect::from_xywh(
                    cx - half_node - sw,
                    cy - half_node - sw,
                    node_size + sw * 2.0,
                    node_size + sw * 2.0,
                ) {
                    let mut g_pb = PathBuilder::new();
                    g_pb.push_rect(g_rect);
                    stroke_pb(pixmap, g_pb, glow_color, sw * 2.0);
                }
            }
        }

        if let Some([tip, left, right]) = compute_start_node_arrow_triangle(contour, is_closed, &vp)
        {
            let mut pb = PathBuilder::new();
            pb.move_to(tip.x.0, tip.y.0);
            pb.line_to(left.x.0, left.y.0);
            pb.line_to(right.x.0, right.y.0);
            pb.close();
            if let Some(path) = pb.finish() {
                let mut halo_paint = Paint::default();
                halo_paint.set_color(bg_color);
                halo_paint.anti_alias = true;
                let halo_stroke = Stroke {
                    width: START_NODE_ARROW_OUTLINE_WIDTH_DP * 2.0,
                    ..Default::default()
                };
                pixmap.stroke_path(
                    &path,
                    &halo_paint,
                    &halo_stroke,
                    Transform::identity(),
                    None,
                );
                let mut paint = Paint::default();
                paint.set_color(arrow_color);
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

        if !is_closed {
            let mut pb = PathBuilder::new();
            for (pa, pb_pt) in compute_open_contour_endpoint_ticks(contour, &vp) {
                pb.move_to(pa.x.0, pa.y.0);
                pb.line_to(pb_pt.x.0, pb_pt.y.0);
            }
            stroke_pb(pixmap, pb, open_color, sw);
        }
    }

    if let Some((_c_idx, bcp_out)) = state.pending_pen_bcp_out {
        let out_scr = vp.to_screen(bcp_out);
        fill_circle(
            pixmap,
            out_scr.x.0,
            out_scr.y.0,
            handle_radius,
            handle_color,
        );
    }

    if let crate::interaction::PenToolState::DraggingHandles {
        anchor,
        bcp_out,
        bcp_in,
        ..
    } = state.pen_state
    {
        for h_pt in [bcp_out, bcp_in] {
            if h_pt != anchor {
                let h_scr = vp.to_screen(h_pt);
                fill_circle(pixmap, h_scr.x.0, h_scr.y.0, handle_radius, handle_color);
            }
        }
    }

    if state.active_tool == CanvasTool::Pen {
        if let Some(hit) = state.hovered_segment_hit {
            let g_scr = vp.to_screen(hit.point);
            let mut pb = PathBuilder::new();
            pb.push_circle(g_scr.x.0, g_scr.y.0, half_node);
            stroke_pb(pixmap, pb, glow_color, GHOST_NODE_STROKE_WIDTH_DP);
            fill_circle(pixmap, g_scr.x.0, g_scr.y.0, half_node * 0.5, smooth_color);
        }
    }
}

pub(crate) fn render_marquee(pixmap: &mut Pixmap, state: &GrafiaCanvasState, theme: &GrafiaTheme) {
    let fill_col = color32_to_tiny_skia(theme.edit_canvas.rubberband_fill_color32());
    let stroke_col = color32_to_tiny_skia(theme.edit_canvas.rubberband_stroke_color32());
    let sw = theme.edit_canvas.rubberband_stroke_width_dp;
    if let crate::interaction::SelectToolState::MarqueeSelecting {
        start_screen,
        current_screen,
    } = state.select_state
    {
        if state.show_bounding_box_slanted && state.selection_angle_deg.abs() > 0.0 {
            let verts = crate::core::slanted_marquee_parallelogram_screen(
                start_screen,
                current_screen,
                state.selection_angle_deg,
            );
            if (verts[1].x.0 - verts[0].x.0).abs() > 1.0
                && (verts[2].y.0 - verts[0].y.0).abs() > 1.0
            {
                let mut pb = PathBuilder::new();
                pb.move_to(verts[0].x.0, verts[0].y.0);
                for v in &verts[1..] {
                    pb.line_to(v.x.0, v.y.0);
                }
                pb.close();
                if let Some(path) = pb.finish() {
                    let mut fill_paint = Paint::default();
                    fill_paint.set_color(fill_col);
                    fill_paint.anti_alias = true;
                    pixmap.fill_path(
                        &path,
                        &fill_paint,
                        FillRule::Winding,
                        Transform::identity(),
                        None,
                    );
                    let mut pb2 = PathBuilder::new();
                    pb2.move_to(verts[0].x.0, verts[0].y.0);
                    for v in &verts[1..] {
                        pb2.line_to(v.x.0, v.y.0);
                    }
                    pb2.close();
                    stroke_pb(pixmap, pb2, stroke_col, sw);
                }
                return;
            }
        }
    }
    let Some(rect) = state.select_state.marquee_rect() else {
        return;
    };
    let (w, h) = ((rect.x1 - rect.x0) as f32, (rect.y1 - rect.y0) as f32);
    if let Some(ts_rect) = Rect::from_xywh(rect.x0 as f32, rect.y0 as f32, w.max(1.0), h.max(1.0)) {
        let mut fill_paint = Paint::default();
        fill_paint.set_color(fill_col);
        pixmap.fill_rect(ts_rect, &fill_paint, Transform::identity(), None);
        let mut pb = PathBuilder::new();
        pb.push_rect(ts_rect);
        stroke_pb(pixmap, pb, stroke_col, sw);
    }
}

pub(crate) fn render_preview_silhouette(
    pixmap: &mut Pixmap,
    state: &GrafiaCanvasState,
    theme: &GrafiaTheme,
) {
    let vp = state.active_slot_viewport();
    let mut paint = Paint::default();
    paint.set_color(color32_to_tiny_skia(theme.preview_canvas.fill_color32()));
    paint.anti_alias = true;
    let mut pb = PathBuilder::new();
    for (c_idx, path) in state.paths.iter().enumerate() {
        if !state.is_contour_closed(c_idx) {
            continue;
        }
        let proj = PreviewFilledProjection::compute(path, &vp);
        append_kurbo_to_builder(&mut pb, &proj.screen_path);
    }
    if let Some(compound_path) = pb.finish() {
        pixmap.fill_path(
            &compound_path,
            &paint,
            FillRule::Winding,
            Transform::identity(),
            None,
        );
    }
}
