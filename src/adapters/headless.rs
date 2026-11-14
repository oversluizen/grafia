//! Pure-Rust offscreen rasterizer adapter for visual pixel-diff testing (STD-ARCH).

use crate::adapters::cursor_overlay::render_pen_extras_and_cursor_tiny_skia;
use crate::adapters::headless_passes::{render_marquee, render_nodes, render_preview_silhouette};
use crate::adapters::state::{CanvasTool, CanvasViewMode, GrafiaCanvasState};
use crate::core::{FontPoint, NodeType};
pub use crate::render::rasterize_preview_panel;
use crate::render::{
    render_preview_panel_into_rect, render_shape_thumbnails_tiny_skia, PreviewPanelConfig,
};
use crate::tokens::{
    color32_to_tiny_skia, GrafiaTheme, DEFAULT_ASCENDER_UPM, DEFAULT_BASELINE_UPM,
    DEFAULT_CAP_HEIGHT_UPM, DEFAULT_DESCENDER_UPM, DEFAULT_X_HEIGHT_UPM,
    GOLDEN_MASTER_CANVAS_EDIT_HEIGHT_DP, GOLDEN_MASTER_CANVAS_PREVIEW_HEIGHT_DP,
    GOLDEN_MASTER_CANVAS_WIDTH_DP, GOLDEN_MASTER_INFOBOX_WIDTH_DP, GOLDEN_MASTER_SIDEBAR_WIDTH_DP,
    GOLDEN_MASTER_SPLITTER_WIDTH_DP, GOLDEN_MASTER_WINDOW_HEIGHT_DP, GOLDEN_MASTER_WINDOW_WIDTH_DP,
    STROKE_WIDTH_MAX_DP, STROKE_WIDTH_MIN_DP,
};
use kurbo::BezPath;
use tiny_skia::{Paint, Path, PathBuilder, Pixmap, Rect, Stroke, Transform};

pub(crate) fn append_kurbo_to_builder(pb: &mut PathBuilder, path: &BezPath) {
    for el in path.elements() {
        match el {
            kurbo::PathEl::MoveTo(p) => pb.move_to(p.x as f32, p.y as f32),
            kurbo::PathEl::LineTo(p) => pb.line_to(p.x as f32, p.y as f32),
            kurbo::PathEl::QuadTo(p1, p2) => {
                pb.quad_to(p1.x as f32, p1.y as f32, p2.x as f32, p2.y as f32)
            }
            kurbo::PathEl::CurveTo(p1, p2, p3) => {
                pb.cubic_to(
                    p1.x as f32,
                    p1.y as f32,
                    p2.x as f32,
                    p2.y as f32,
                    p3.x as f32,
                    p3.y as f32,
                );
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
    image::RgbaImage::from_raw(w, h, pixmap.take()).unwrap_or_else(|| image::RgbaImage::new(w, h))
}

pub(crate) fn alloc_pixmap_or_1x1(width: u32, height: u32) -> Pixmap {
    assert!(width > 0 && height > 0, "Pixmap dimensions must be > 0");
    let Some(p) = Pixmap::new(width, height).or_else(|| Pixmap::new(1, 1)) else {
        unreachable!()
    };
    p
}

fn render_metrics_box(pixmap: &mut Pixmap, state: &GrafiaCanvasState, theme: &GrafiaTheme) {
    crate::render::render_canvas_grid_tiny_skia(pixmap, state);
    crate::render::paint_custom_reference_frame_pixmap_pass(pixmap, state, theme);
    crate::render::paint_pixel_preview_and_ghost_pixmap_pass(pixmap, state, theme);
    if !state.show_metrics || (state.contours.is_empty() && state.glyph.width <= 0.0) {
        return;
    }
    crate::render::render_alignment_zones_tiny_skia(pixmap, state);
    let vp = state.active_slot_viewport();
    let adv_w = if state.glyph.width > 0.0 {
        state.glyph.width
    } else {
        DEFAULT_X_HEIGHT_UPM
    };
    let mut paint = Paint::default();
    paint.set_color(color32_to_tiny_skia(theme.metrics_guide_color32()));
    paint.anti_alias = true;
    let stroke = Stroke {
        width: theme
            .edit_canvas
            .metric_line_width_dp
            .clamp(STROKE_WIDTH_MIN_DP, STROKE_WIDTH_MAX_DP),
        ..Default::default()
    };

    let mut pb = PathBuilder::new();
    let top_l = vp.to_screen(FontPoint::new(0.0, DEFAULT_ASCENDER_UPM));
    let bot_l = vp.to_screen(FontPoint::new(0.0, DEFAULT_DESCENDER_UPM));
    let top_r = vp.to_screen(FontPoint::new(adv_w, DEFAULT_ASCENDER_UPM));
    let bot_r = vp.to_screen(FontPoint::new(adv_w, DEFAULT_DESCENDER_UPM));
    pb.move_to(top_l.x.0, top_l.y.0);
    pb.line_to(bot_l.x.0, bot_l.y.0);
    pb.move_to(top_r.x.0, top_r.y.0);
    pb.line_to(bot_r.x.0, bot_r.y.0);

    for y_upm in [
        DEFAULT_ASCENDER_UPM,
        DEFAULT_CAP_HEIGHT_UPM,
        DEFAULT_X_HEIGHT_UPM,
        DEFAULT_BASELINE_UPM,
        DEFAULT_DESCENDER_UPM,
    ] {
        let l_scr = vp.to_screen(FontPoint::new(0.0, y_upm));
        let r_scr = vp.to_screen(FontPoint::new(adv_w, y_upm));
        pb.move_to(l_scr.x.0, l_scr.y.0);
        pb.line_to(r_scr.x.0, r_scr.y.0);
    }
    if let Some(path) = pb.finish() {
        pixmap.stroke_path(&path, &paint, &stroke, Transform::identity(), None);
    }
}

fn append_contour_bcp_tangent_lines_ts(
    t_pb: &mut PathBuilder,
    state: &GrafiaCanvasState,
    vp: &crate::core::ViewportTransform,
) {
    if !state.show_nodes_offcurve {
        return;
    }
    for (c_idx, contour) in state.contours.iter().enumerate() {
        let is_closed = state.is_contour_closed(c_idx);
        let (anchors, spans) = crate::core::hobby_spline::extract_anchor_spans(contour, is_closed);
        for (s_pos, e_pos, off) in spans {
            let (s_idx, e_idx) = (anchors[s_pos], anchors[e_pos]);
            if off.len() >= 2 {
                let (p0, h1) = (
                    vp.to_screen(contour[s_idx].point),
                    vp.to_screen(contour[off[0]].point),
                );
                let (p3, h2) = (
                    vp.to_screen(contour[e_idx].point),
                    vp.to_screen(contour[off[1]].point),
                );
                t_pb.move_to(p0.x.0, p0.y.0);
                t_pb.line_to(h1.x.0, h1.y.0);
                t_pb.move_to(p3.x.0, p3.y.0);
                t_pb.line_to(h2.x.0, h2.y.0);
            } else if off.len() == 1 {
                let p0 = vp.to_screen(contour[s_idx].point);
                let q1 = vp.to_screen(contour[off[0]].point);
                let p2 = vp.to_screen(contour[e_idx].point);
                t_pb.move_to(p0.x.0, p0.y.0);
                t_pb.line_to(q1.x.0, q1.y.0);
                t_pb.move_to(p2.x.0, p2.y.0);
                t_pb.line_to(q1.x.0, q1.y.0);
            }
        }
    }
}

fn render_wireframe_contours(pixmap: &mut Pixmap, state: &GrafiaCanvasState, theme: &GrafiaTheme) {
    let vp = state.active_slot_viewport();
    let affine = kurbo::Affine::translate((vp.pan_offset.x.0 as f64, vp.pan_offset.y.0 as f64))
        * kurbo::Affine::scale_non_uniform(vp.zoom as f64, -(vp.zoom as f64));

    let mut paint = Paint::default();
    paint.set_color(color32_to_tiny_skia(theme.preview_canvas.fill_color32()));
    paint.anti_alias = true;
    let stroke = Stroke {
        width: theme.edit_canvas.contour_stroke_width_dp,
        ..Default::default()
    };

    for path in &state.paths {
        let screen_path = affine * path;
        if let Some(ts_path) = kurbo_to_tiny_skia_path(&screen_path) {
            pixmap.stroke_path(&ts_path, &paint, &stroke, Transform::identity(), None);
        }
    }

    let mut bcp_paint = Paint::default();
    bcp_paint.set_color(color32_to_tiny_skia(theme.edit_canvas.bcp_line_color32()));
    bcp_paint.anti_alias = true;
    let bcp_stroke = Stroke {
        width: theme.edit_canvas.bcp_line_width_dp,
        ..Default::default()
    };
    let mut t_pb = PathBuilder::new();
    append_contour_bcp_tangent_lines_ts(&mut t_pb, state, &vp);
    if let Some((c_idx, bcp_out)) = state.pending_pen_bcp_out {
        if let Some(last_on) = state
            .contours
            .get(c_idx)
            .and_then(|c| c.iter().rev().find(|n| n.node_type != NodeType::OffCurve))
        {
            let a_scr = vp.to_screen(last_on.point);
            let out_scr = vp.to_screen(bcp_out);
            t_pb.move_to(a_scr.x.0, a_scr.y.0);
            t_pb.line_to(out_scr.x.0, out_scr.y.0);
        }
    }
    if let crate::interaction::PenToolState::DraggingHandles {
        anchor,
        bcp_out,
        bcp_in,
        ..
    } = state.pen_state
    {
        let a_scr = vp.to_screen(anchor);
        for h_pt in [bcp_out, bcp_in] {
            if h_pt != anchor {
                let h_scr = vp.to_screen(h_pt);
                t_pb.move_to(a_scr.x.0, a_scr.y.0);
                t_pb.line_to(h_scr.x.0, h_scr.y.0);
            }
        }
    }
    if let Some(t_path) = t_pb.finish() {
        pixmap.stroke_path(
            &t_path,
            &bcp_paint,
            &bcp_stroke,
            Transform::identity(),
            None,
        );
    }
}

fn render_canvas_passes(
    pixmap: &mut Pixmap,
    state: &GrafiaCanvasState,
    theme: &GrafiaTheme,
    include_thumbnails: bool,
) {
    match state.view_mode {
        CanvasViewMode::EditWireframe => {
            render_metrics_box(pixmap, state, theme);
            crate::render::render_background_image_tiny_skia(pixmap, state, theme);
            crate::render::paint_color_layers_pixmap_pass(pixmap, state);
            if !(state.active_tool == CanvasTool::Text && !state.text_stream.slots.is_empty()) {
                if state.preferences.fill_active_glyph_while_editing {
                    crate::render::render_edit_active_fill_tiny_skia(pixmap, state, theme);
                }
                render_wireframe_contours(pixmap, state, theme);
                render_nodes(pixmap, state, theme);
            }
            render_marquee(pixmap, state, theme);
            crate::render::render_bbox_and_direction_overlays_tiny_skia(pixmap, state, theme);
            crate::render::render_guide_and_snap_overlays_tiny_skia(pixmap, state);
            crate::render::render_drawing_tool_overlays_tiny_skia(pixmap, state);
            crate::render::render_text_strip_overlays_tiny_skia(pixmap, state);
            crate::render::render_ruler_and_background_overlays_tiny_skia(
                pixmap,
                state,
                theme,
                0.0,
                0.0,
                pixmap.width() as f32,
                pixmap.height() as f32,
            );
            crate::render::render_spline_and_handle_overlays_tiny_skia(pixmap, state, theme);
            crate::render::paint_color_layer_handles_pixmap_pass(pixmap, state, theme);
            crate::render::paint_hinting_and_hoi_pixmap_pass(pixmap, state, theme);
        }
        CanvasViewMode::PreviewFilled => {
            crate::render::render_background_image_tiny_skia(pixmap, state, theme);
            crate::render::paint_color_layers_pixmap_pass(pixmap, state);
            render_preview_silhouette(pixmap, state, theme);
            crate::render::render_text_strip_overlays_tiny_skia(pixmap, state);
            crate::render::render_ruler_and_background_overlays_tiny_skia(
                pixmap,
                state,
                theme,
                0.0,
                0.0,
                pixmap.width() as f32,
                pixmap.height() as f32,
            );
        }
        CanvasViewMode::ShapeThumbnail => {
            if include_thumbnails {
                render_shape_thumbnails_tiny_skia(pixmap, state, theme);
            }
        }
    }
}

#[must_use]
pub fn rasterize_canvas_subregion(
    state: &GrafiaCanvasState,
    theme: &GrafiaTheme,
    width: u32,
    height: u32,
) -> Pixmap {
    let mut pixmap = alloc_pixmap_or_1x1(width, height);
    pixmap.fill(color32_to_tiny_skia(theme.canvas_background_color32()));
    render_canvas_passes(&mut pixmap, state, theme, true);
    crate::render::render_top_cursor_and_loupe_overlays_tiny_skia(&mut pixmap, state, theme);
    render_pen_extras_and_cursor_tiny_skia(&mut pixmap, state, theme);
    pixmap
}

fn fill_window_chrome_panels_ts(
    pixmap: &mut Pixmap,
    theme: &GrafiaTheme,
    canvas_x: f32,
    infobox_x: f32,
    status_y: f32,
) {
    for (x, y, w, h, col) in [
        (
            0.0,
            0.0,
            GOLDEN_MASTER_SIDEBAR_WIDTH_DP,
            GOLDEN_MASTER_WINDOW_HEIGHT_DP,
            theme.sidebar_background_color32(),
        ),
        (
            infobox_x,
            0.0,
            GOLDEN_MASTER_INFOBOX_WIDTH_DP,
            GOLDEN_MASTER_WINDOW_HEIGHT_DP,
            theme.infobox_background_color32(),
        ),
        (
            canvas_x,
            status_y,
            GOLDEN_MASTER_CANVAS_WIDTH_DP,
            GOLDEN_MASTER_WINDOW_HEIGHT_DP - status_y,
            theme.infobox_background_color32(),
        ),
    ] {
        if let Some(r) = Rect::from_xywh(x, y, w, h) {
            let mut paint = Paint::default();
            paint.set_color(color32_to_tiny_skia(col));
            pixmap.fill_rect(r, &paint, Transform::identity(), None);
        }
    }
}

#[must_use]
pub fn rasterize_full_window(state: &GrafiaCanvasState, theme: &GrafiaTheme) -> Pixmap {
    let (width, height) = (
        GOLDEN_MASTER_WINDOW_WIDTH_DP as u32,
        GOLDEN_MASTER_WINDOW_HEIGHT_DP as u32,
    );
    let mut pixmap = alloc_pixmap_or_1x1(width, height);
    let canvas_x = GOLDEN_MASTER_SIDEBAR_WIDTH_DP + GOLDEN_MASTER_SPLITTER_WIDTH_DP;
    let infobox_x = canvas_x + GOLDEN_MASTER_CANVAS_WIDTH_DP + GOLDEN_MASTER_SPLITTER_WIDTH_DP;
    pixmap.fill(color32_to_tiny_skia(theme.canvas_background_color32()));
    render_canvas_passes(&mut pixmap, state, theme, false);
    let preview_y = GOLDEN_MASTER_CANVAS_EDIT_HEIGHT_DP + GOLDEN_MASTER_SPLITTER_WIDTH_DP;
    let status_y = preview_y + GOLDEN_MASTER_CANVAS_PREVIEW_HEIGHT_DP;
    fill_window_chrome_panels_ts(&mut pixmap, theme, canvas_x, infobox_x, status_y);
    let preview_cfg = PreviewPanelConfig {
        inverted: (state.theme.is_dark() != theme.is_dark())
            ^ state.drag_transients.preview_config.inverted,
        flipped: state.drag_transients.preview_config.flipped,
        blur_radius_px: state.drag_transients.preview_config.blur_radius_px,
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
        (
            GOLDEN_MASTER_SIDEBAR_WIDTH_DP,
            0.0,
            GOLDEN_MASTER_SPLITTER_WIDTH_DP,
            GOLDEN_MASTER_WINDOW_HEIGHT_DP,
        ),
        (
            canvas_x + GOLDEN_MASTER_CANVAS_WIDTH_DP,
            0.0,
            GOLDEN_MASTER_SPLITTER_WIDTH_DP,
            GOLDEN_MASTER_WINDOW_HEIGHT_DP,
        ),
        (
            canvas_x,
            GOLDEN_MASTER_CANVAS_EDIT_HEIGHT_DP,
            GOLDEN_MASTER_CANVAS_WIDTH_DP,
            GOLDEN_MASTER_SPLITTER_WIDTH_DP,
        ),
    ] {
        if let Some(r) = Rect::from_xywh(x, y, w, h) {
            pixmap.fill_rect(r, &splitter_paint, Transform::identity(), None);
        }
    }
    pixmap
}
