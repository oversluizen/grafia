//! Pure-Rust offscreen rasterizer adapter for visual pixel-diff testing (STD-ARCH).

use crate::adapters::cursor_overlay::render_pen_extras_and_cursor_tiny_skia;
use crate::adapters::state::{CanvasTool, CanvasViewMode, GlifoCanvasState};
use crate::core::{extract_segments, ContourSegment, FontPoint, NodeType};
pub use crate::render::rasterize_preview_panel;
use crate::render::{
    compute_open_contour_endpoint_ticks, compute_start_node_arrow_triangle,
    render_preview_panel_into_rect, render_shape_thumbnails_tiny_skia, PreviewFilledProjection,
    PreviewPanelConfig,
};
use crate::tokens::{
    color32_to_tiny_skia, DEFAULT_ASCENDER_UPM, DEFAULT_BASELINE_UPM, DEFAULT_CAP_HEIGHT_UPM,
    DEFAULT_DESCENDER_UPM, DEFAULT_X_HEIGHT_UPM, GHOST_NODE_STROKE_WIDTH_DP, GlifoTheme,
    GOLDEN_MASTER_CANVAS_EDIT_HEIGHT_DP, GOLDEN_MASTER_CANVAS_PREVIEW_HEIGHT_DP,
    GOLDEN_MASTER_CANVAS_WIDTH_DP, GOLDEN_MASTER_INFOBOX_WIDTH_DP, GOLDEN_MASTER_SIDEBAR_WIDTH_DP,
    GOLDEN_MASTER_SPLITTER_WIDTH_DP, GOLDEN_MASTER_WINDOW_HEIGHT_DP,
    GOLDEN_MASTER_WINDOW_WIDTH_DP, START_NODE_ARROW_OUTLINE_WIDTH_DP,
};
use kurbo::BezPath;
use tiny_skia::{FillRule, Paint, Path, PathBuilder, Pixmap, Rect, Stroke, Transform};

fn append_kurbo_to_builder(pb: &mut PathBuilder, path: &BezPath) {
    for el in path.elements() {
        match el {
            kurbo::PathEl::MoveTo(p) => pb.move_to(p.x as f32, p.y as f32),
            kurbo::PathEl::LineTo(p) => pb.line_to(p.x as f32, p.y as f32),
            kurbo::PathEl::QuadTo(p1, p2) => pb.quad_to(p1.x as f32, p1.y as f32, p2.x as f32, p2.y as f32),
            kurbo::PathEl::CurveTo(p1, p2, p3) => {
                pb.cubic_to(p1.x as f32, p1.y as f32, p2.x as f32, p2.y as f32, p3.x as f32, p3.y as f32);
            }
            kurbo::PathEl::ClosePath => pb.close(),
        }
    }
}

#[must_use]
pub fn kurbo_to_tiny_skia_path(path: &BezPath) -> Option<Path> {
    let mut pb = PathBuilder::new();
    append_kurbo_to_builder(&mut pb, path);
    pb.finish()
}

#[must_use]
pub fn pixmap_to_rgba_image(pixmap: Pixmap) -> image::RgbaImage {
    let (w, h) = (pixmap.width(), pixmap.height());
    image::RgbaImage::from_raw(w, h, pixmap.take())
        .expect("tiny_skia Pixmap byte buffer must match image dimensions")
}

fn render_metrics_box(pixmap: &mut Pixmap, state: &GlifoCanvasState, theme: &GlifoTheme) {
    crate::render::render_v030_grid_tiny_skia(pixmap, state);
    if !state.show_metrics || (state.contours.is_empty() && state.glyph.width <= 0.0) {
        return;
    }
    crate::render::render_v030_alignment_zones_tiny_skia(pixmap, state);
    let adv_w = if state.glyph.width > 0.0 { state.glyph.width } else { DEFAULT_X_HEIGHT_UPM };
    let mut paint = Paint::default();
    paint.set_color(color32_to_tiny_skia(theme.metrics_guide_color32()));
    paint.anti_alias = true;
    let stroke = Stroke { width: theme.preview_canvas.baseline_stroke_width_dp, ..Default::default() };

    let mut pb = PathBuilder::new();
    let top_l = state.viewport.to_screen(FontPoint::new(0.0, DEFAULT_ASCENDER_UPM));
    let bot_l = state.viewport.to_screen(FontPoint::new(0.0, DEFAULT_DESCENDER_UPM));
    let top_r = state.viewport.to_screen(FontPoint::new(adv_w, DEFAULT_ASCENDER_UPM));
    let bot_r = state.viewport.to_screen(FontPoint::new(adv_w, DEFAULT_DESCENDER_UPM));
    pb.move_to(top_l.x.0, top_l.y.0);
    pb.line_to(bot_l.x.0, bot_l.y.0);
    pb.move_to(top_r.x.0, top_r.y.0);
    pb.line_to(bot_r.x.0, bot_r.y.0);

    for y_upm in [DEFAULT_ASCENDER_UPM, DEFAULT_CAP_HEIGHT_UPM, DEFAULT_X_HEIGHT_UPM, DEFAULT_BASELINE_UPM, DEFAULT_DESCENDER_UPM] {
        let l_scr = state.viewport.to_screen(FontPoint::new(0.0, y_upm));
        let r_scr = state.viewport.to_screen(FontPoint::new(adv_w, y_upm));
        pb.move_to(l_scr.x.0, l_scr.y.0);
        pb.line_to(r_scr.x.0, r_scr.y.0);
    }
    if let Some(path) = pb.finish() {
        pixmap.stroke_path(&path, &paint, &stroke, Transform::identity(), None);
    }
}

fn render_wireframe_contours(pixmap: &mut Pixmap, state: &GlifoCanvasState, theme: &GlifoTheme) {
    let affine = kurbo::Affine::translate((
        state.viewport.pan_offset.x.0 as f64,
        state.viewport.pan_offset.y.0 as f64,
    )) * kurbo::Affine::scale_non_uniform(state.viewport.zoom as f64, -(state.viewport.zoom as f64));

    let mut paint = Paint::default();
    paint.set_color(color32_to_tiny_skia(theme.preview_canvas.fill_color32()));
    paint.anti_alias = true;
    let stroke = Stroke { width: theme.edit_canvas.contour_stroke_width_dp, ..Default::default() };

    for path in &state.paths {
        let screen_path = affine * path;
        if let Some(ts_path) = kurbo_to_tiny_skia_path(&screen_path) {
            pixmap.stroke_path(&ts_path, &paint, &stroke, Transform::identity(), None);
        }
    }

    let mut bcp_paint = Paint::default();
    bcp_paint.set_color(color32_to_tiny_skia(theme.edit_canvas.bcp_line_color32()));
    bcp_paint.anti_alias = true;
    let bcp_stroke = Stroke { width: theme.edit_canvas.bcp_line_width_dp, ..Default::default() };
    let mut t_pb = PathBuilder::new();
    for (c_idx, contour) in state.contours.iter().enumerate() {
        let is_closed = state.is_contour_closed(c_idx);
        for seg in extract_segments(contour, is_closed) {
            if let ContourSegment::Cubic { start_idx, h1_idx, h2_idx, end_idx, .. } = seg {
                let p0 = state.viewport.to_screen(contour[start_idx].point);
                let h1 = state.viewport.to_screen(contour[h1_idx].point);
                let p3 = state.viewport.to_screen(contour[end_idx].point);
                let h2 = state.viewport.to_screen(contour[h2_idx].point);
                t_pb.move_to(p0.x.0, p0.y.0);
                t_pb.line_to(h1.x.0, h1.y.0);
                t_pb.move_to(p3.x.0, p3.y.0);
                t_pb.line_to(h2.x.0, h2.y.0);
            }
        }
    }
    if let Some((c_idx, bcp_out)) = state.pending_pen_bcp_out {
        if let Some(last_on) = state.contours.get(c_idx).and_then(|c| c.iter().rev().find(|n| n.node_type != NodeType::OffCurve)) {
            let a_scr = state.viewport.to_screen(last_on.point);
            let out_scr = state.viewport.to_screen(bcp_out);
            t_pb.move_to(a_scr.x.0, a_scr.y.0);
            t_pb.line_to(out_scr.x.0, out_scr.y.0);
        }
    }
    if let crate::interaction::PenToolState::DraggingHandles { anchor, bcp_out, bcp_in, .. } = state.pen_state {
        let a_scr = state.viewport.to_screen(anchor);
        for h_pt in [bcp_out, bcp_in] {
            if h_pt != anchor {
                let h_scr = state.viewport.to_screen(h_pt);
                t_pb.move_to(a_scr.x.0, a_scr.y.0);
                t_pb.line_to(h_scr.x.0, h_scr.y.0);
            }
        }
    }
    if let Some(t_path) = t_pb.finish() {
        pixmap.stroke_path(&t_path, &bcp_paint, &bcp_stroke, Transform::identity(), None);
    }
}

fn fill_circle(pixmap: &mut Pixmap, cx: f32, cy: f32, r: f32, color: tiny_skia::Color) {
    let mut pb = PathBuilder::new();
    pb.push_circle(cx, cy, r);
    if let Some(path) = pb.finish() {
        let mut paint = Paint::default();
        paint.set_color(color);
        paint.anti_alias = true;
        pixmap.fill_path(&path, &paint, FillRule::Winding, Transform::identity(), None);
    }
}

fn stroke_pb(pixmap: &mut Pixmap, pb: PathBuilder, color: tiny_skia::Color, width: f32) {
    if let Some(path) = pb.finish() {
        let mut paint = Paint::default();
        paint.set_color(color);
        paint.anti_alias = true;
        let stroke = Stroke { width, ..Default::default() };
        pixmap.stroke_path(&path, &paint, &stroke, Transform::identity(), None);
    }
}

fn render_nodes(pixmap: &mut Pixmap, state: &GlifoCanvasState, theme: &GlifoTheme) {
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
            && state.pending_pen_first_bcp_in.is_none_or(|(c, _)| c != c_idx);
        for node in contour {
            let scr = state.viewport.to_screen(node.point);
            let (cx, cy) = (scr.x.0, scr.y.0);
            if is_single_open_point {
                let mut pb = PathBuilder::new();
                pb.move_to(cx - half_node, cy - half_node); pb.line_to(cx + half_node, cy + half_node);
                pb.move_to(cx - half_node, cy + half_node); pb.line_to(cx + half_node, cy - half_node);
                stroke_pb(pixmap, pb, corner_color, sw);
            } else {
                match node.node_type {
                    NodeType::Corner => {
                        if let Some(rect) = Rect::from_xywh(cx - half_node, cy - half_node, node_size, node_size) {
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
                if let Some(g_rect) = Rect::from_xywh(cx - half_node - sw, cy - half_node - sw, node_size + sw * 2.0, node_size + sw * 2.0) {
                    let mut g_pb = PathBuilder::new();
                    g_pb.push_rect(g_rect);
                    stroke_pb(pixmap, g_pb, glow_color, sw * 2.0);
                }
            }
        }

        if let Some([tip, left, right]) = compute_start_node_arrow_triangle(contour, is_closed, &state.viewport) {
            let mut pb = PathBuilder::new();
            pb.move_to(tip.x.0, tip.y.0);
            pb.line_to(left.x.0, left.y.0);
            pb.line_to(right.x.0, right.y.0);
            pb.close();
            if let Some(path) = pb.finish() {
                let mut halo_paint = Paint::default();
                halo_paint.set_color(bg_color);
                halo_paint.anti_alias = true;
                let halo_stroke = Stroke { width: START_NODE_ARROW_OUTLINE_WIDTH_DP * 2.0, ..Default::default() };
                pixmap.stroke_path(&path, &halo_paint, &halo_stroke, Transform::identity(), None);
                let mut paint = Paint::default();
                paint.set_color(arrow_color);
                paint.anti_alias = true;
                pixmap.fill_path(&path, &paint, FillRule::Winding, Transform::identity(), None);
            }
        }

        if !is_closed {
            let mut pb = PathBuilder::new();
            for (pa, pb_pt) in compute_open_contour_endpoint_ticks(contour, &state.viewport) {
                pb.move_to(pa.x.0, pa.y.0); pb.line_to(pb_pt.x.0, pb_pt.y.0);
            }
            stroke_pb(pixmap, pb, open_color, sw);
        }
    }

    if let Some((_c_idx, bcp_out)) = state.pending_pen_bcp_out {
        let out_scr = state.viewport.to_screen(bcp_out);
        fill_circle(pixmap, out_scr.x.0, out_scr.y.0, handle_radius, handle_color);
    }

    if let crate::interaction::PenToolState::DraggingHandles { anchor, bcp_out, bcp_in, .. } = state.pen_state {
        for h_pt in [bcp_out, bcp_in] {
            if h_pt != anchor {
                let h_scr = state.viewport.to_screen(h_pt);
                fill_circle(pixmap, h_scr.x.0, h_scr.y.0, handle_radius, handle_color);
            }
        }
    }

    if state.active_tool == CanvasTool::Pen {
        if let Some(hit) = state.hovered_segment_hit {
            let g_scr = state.viewport.to_screen(hit.point);
            let mut pb = PathBuilder::new();
            pb.push_circle(g_scr.x.0, g_scr.y.0, half_node);
            stroke_pb(pixmap, pb, glow_color, GHOST_NODE_STROKE_WIDTH_DP);
            fill_circle(pixmap, g_scr.x.0, g_scr.y.0, half_node * 0.5, smooth_color);
        }
    }
}

fn render_marquee(pixmap: &mut Pixmap, state: &GlifoCanvasState, theme: &GlifoTheme) {
    let fill_col = color32_to_tiny_skia(theme.edit_canvas.rubberband_fill_color32());
    let stroke_col = color32_to_tiny_skia(theme.edit_canvas.rubberband_stroke_color32());
    let sw = theme.edit_canvas.rubberband_stroke_width_dp;
    if let crate::interaction::SelectToolState::MarqueeSelecting { start_screen, current_screen } = state.select_state {
        if state.show_bounding_box_slanted && state.selection_angle_deg.abs() > 0.0 {
            let verts = crate::core::slanted_marquee_parallelogram_screen(start_screen, current_screen, state.selection_angle_deg);
            if (verts[1].x.0 - verts[0].x.0).abs() > 1.0 && (verts[2].y.0 - verts[0].y.0).abs() > 1.0 {
                let mut pb = PathBuilder::new();
                pb.move_to(verts[0].x.0, verts[0].y.0);
                for v in &verts[1..] { pb.line_to(v.x.0, v.y.0); }
                pb.close();
                if let Some(path) = pb.finish() {
                    let mut fill_paint = Paint::default();
                    fill_paint.set_color(fill_col);
                    fill_paint.anti_alias = true;
                    pixmap.fill_path(&path, &fill_paint, FillRule::Winding, Transform::identity(), None);
                    let mut pb2 = PathBuilder::new();
                    pb2.move_to(verts[0].x.0, verts[0].y.0);
                    for v in &verts[1..] { pb2.line_to(v.x.0, v.y.0); }
                    pb2.close();
                    stroke_pb(pixmap, pb2, stroke_col, sw);
                }
                return;
            }
        }
    }
    let Some(rect) = state.select_state.marquee_rect() else { return; };
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

fn render_preview_silhouette(pixmap: &mut Pixmap, state: &GlifoCanvasState, theme: &GlifoTheme) {
    let mut paint = Paint::default();
    paint.set_color(color32_to_tiny_skia(theme.preview_canvas.fill_color32()));
    paint.anti_alias = true;
    let mut pb = PathBuilder::new();
    for (c_idx, path) in state.paths.iter().enumerate() {
        if !state.is_contour_closed(c_idx) {
            continue;
        }
        let proj = PreviewFilledProjection::compute(path, &state.viewport);
        append_kurbo_to_builder(&mut pb, &proj.screen_path);
    }
    if let Some(compound_path) = pb.finish() {
        pixmap.fill_path(&compound_path, &paint, FillRule::Winding, Transform::identity(), None);
    }
}

#[must_use]
pub fn rasterize_canvas_subregion(
    state: &GlifoCanvasState,
    theme: &GlifoTheme,
    width: u32,
    height: u32,
) -> Pixmap {
    let mut pixmap = Pixmap::new(width, height).expect("Failed to allocate tiny_skia Pixmap for canvas subregion");
    pixmap.fill(color32_to_tiny_skia(theme.canvas_background_color32()));
    match state.view_mode {
        CanvasViewMode::EditWireframe => {
            render_metrics_box(&mut pixmap, state, theme);
            render_wireframe_contours(&mut pixmap, state, theme);
            render_nodes(&mut pixmap, state, theme);
            render_marquee(&mut pixmap, state, theme);
            crate::render::render_v020_overlays_tiny_skia(&mut pixmap, state, theme);
            crate::render::render_v030_overlays_tiny_skia(&mut pixmap, state);
            crate::render::render_v040_overlays_tiny_skia(&mut pixmap, state);
        }
        CanvasViewMode::PreviewFilled => render_preview_silhouette(&mut pixmap, state, theme),
        CanvasViewMode::ShapeThumbnail => render_shape_thumbnails_tiny_skia(&mut pixmap, state, theme),
    }
    render_pen_extras_and_cursor_tiny_skia(&mut pixmap, state, theme);
    pixmap
}

#[must_use]
pub fn rasterize_full_window(state: &GlifoCanvasState, theme: &GlifoTheme) -> Pixmap {
    let (width, height) = (GOLDEN_MASTER_WINDOW_WIDTH_DP as u32, GOLDEN_MASTER_WINDOW_HEIGHT_DP as u32);
    let mut pixmap = Pixmap::new(width, height).expect("Failed to allocate tiny_skia Pixmap for full window");
    let canvas_x = GOLDEN_MASTER_SIDEBAR_WIDTH_DP + GOLDEN_MASTER_SPLITTER_WIDTH_DP;
    let infobox_x = canvas_x + GOLDEN_MASTER_CANVAS_WIDTH_DP + GOLDEN_MASTER_SPLITTER_WIDTH_DP;
    pixmap.fill(color32_to_tiny_skia(theme.canvas_background_color32()));
    match state.view_mode {
        CanvasViewMode::EditWireframe => {
            render_metrics_box(&mut pixmap, state, theme);
            render_wireframe_contours(&mut pixmap, state, theme);
            render_nodes(&mut pixmap, state, theme);
            render_marquee(&mut pixmap, state, theme);
            crate::render::render_v020_overlays_tiny_skia(&mut pixmap, state, theme);
            crate::render::render_v030_overlays_tiny_skia(&mut pixmap, state);
            crate::render::render_v040_overlays_tiny_skia(&mut pixmap, state);
        }
        CanvasViewMode::PreviewFilled => render_preview_silhouette(&mut pixmap, state, theme),
        CanvasViewMode::ShapeThumbnail => {}
    }
    let preview_y = GOLDEN_MASTER_CANVAS_EDIT_HEIGHT_DP + GOLDEN_MASTER_SPLITTER_WIDTH_DP;
    let status_y = preview_y + GOLDEN_MASTER_CANVAS_PREVIEW_HEIGHT_DP;
    for (x, y, w, h, col) in [
        (0.0, 0.0, GOLDEN_MASTER_SIDEBAR_WIDTH_DP, GOLDEN_MASTER_WINDOW_HEIGHT_DP, theme.sidebar_background_color32()),
        (infobox_x, 0.0, GOLDEN_MASTER_INFOBOX_WIDTH_DP, GOLDEN_MASTER_WINDOW_HEIGHT_DP, theme.infobox_background_color32()),
        (canvas_x, status_y, GOLDEN_MASTER_CANVAS_WIDTH_DP, GOLDEN_MASTER_WINDOW_HEIGHT_DP - status_y, theme.infobox_background_color32()),
    ] {
        if let Some(r) = Rect::from_xywh(x, y, w, h) {
            let mut paint = Paint::default();
            paint.set_color(color32_to_tiny_skia(col));
            pixmap.fill_rect(r, &paint, Transform::identity(), None);
        }
    }
    let preview_cfg = PreviewPanelConfig {
        inverted: state.theme.is_dark() != theme.is_dark(),
        ..PreviewPanelConfig::default()
    };
    render_preview_panel_into_rect(
        &mut pixmap,
        state,
        &preview_cfg,
        canvas_x,
        preview_y,
        GOLDEN_MASTER_CANVAS_WIDTH_DP,
        GOLDEN_MASTER_CANVAS_PREVIEW_HEIGHT_DP,
    );
    let mut splitter_paint = Paint::default();
    splitter_paint.set_color(color32_to_tiny_skia(theme.splitter_color32()));
    for (x, y, w, h) in [
        (GOLDEN_MASTER_SIDEBAR_WIDTH_DP, 0.0, GOLDEN_MASTER_SPLITTER_WIDTH_DP, GOLDEN_MASTER_WINDOW_HEIGHT_DP),
        (canvas_x + GOLDEN_MASTER_CANVAS_WIDTH_DP, 0.0, GOLDEN_MASTER_SPLITTER_WIDTH_DP, GOLDEN_MASTER_WINDOW_HEIGHT_DP),
        (canvas_x, GOLDEN_MASTER_CANVAS_EDIT_HEIGHT_DP, GOLDEN_MASTER_CANVAS_WIDTH_DP, GOLDEN_MASTER_SPLITTER_WIDTH_DP),
    ] {
        if let Some(r) = Rect::from_xywh(x, y, w, h) {
            pixmap.fill_rect(r, &splitter_paint, Transform::identity(), None);
        }
    }
    pixmap
}
