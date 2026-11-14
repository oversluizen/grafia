//! `SPEC-009` `tiny-skia` offscreen overlays for background images, Hobby/Euler node markers,
//! smart handles, extra nodes, node coordinates, and rotation pills.

use crate::adapters::state::{CanvasViewMode, GrafiaCanvasState};
use crate::core::smart_handles::{smart_handle_track_endpoints_upm, smart_handle_upm_position};
use crate::core::{FontPoint, NodeType};
use crate::render::canvas_render_cache::cached_extra_nodes;
use crate::render::spline_and_handle_overlays::{bg_image_quad_upm, component_bounds_upm};
use crate::render::ts_primitives::{
    draw_bitmap_string_ts, fill_circle_ts, fill_pill_ts, stroke_line_ts, stroke_rect_ts, stroke_ts,
};
use crate::tokens::{
    color32_to_tiny_skia, GrafiaTheme, BBOX_ROTATE_PILL_OFFSET_X_DP, BBOX_ROTATE_PILL_OFFSET_Y_DP,
    BG_IMAGE_SELECTION_WIDTH_DP, COMPONENT_PIECE_FRAME_WIDTH_DP, EULER_SPIRO_INNER_RADIUS_DP,
    EULER_SPIRO_OUTER_RADIUS_DP, EULER_SPIRO_RING_WIDTH_DP, EXTRA_NODE_DIAMOND_RADIUS_DP,
    EXTRA_NODE_STROKE_WIDTH_DP, HOBBY_STAR_HALF_STEP_RAD, HOBBY_STAR_INNER_RADIUS_DP,
    HOBBY_STAR_OUTER_RADIUS_DP, HOBBY_STAR_TOP_ANGLE_RAD, HOBBY_STAR_VERTICES,
    NODE_COORD_LABEL_OFFSET_X_DP, NODE_COORD_LABEL_OFFSET_Y_DP, PILL_GLYPH_H_DP,
    RGBA_CHANNEL_MAX_F32, SMART_HANDLE_DIAMOND_RADIUS_DP, SMART_HANDLE_PILL_OFFSET_Y_DP,
    SMART_HANDLE_TICK_RADIUS_DP, SMART_HANDLE_TRACK_WIDTH_DP,
};
use tiny_skia::{
    FillRule, Paint, PathBuilder, Pixmap, PixmapPaint, PremultipliedColorU8, Rect as TsRect,
    Transform,
};

pub fn render_background_image_tiny_skia(
    pixmap: &mut Pixmap,
    state: &GrafiaCanvasState,
    theme: &GrafiaTheme,
) {
    if state.view_mode == CanvasViewMode::ShapeThumbnail {
        return;
    }
    let Some(img) = state.background_image.as_ref() else {
        return;
    };
    let Some(([cx, cy, cw, ch], corners_upm)) = bg_image_quad_upm(img) else {
        return;
    };
    let Some(mut sub_pix) = Pixmap::new(cw, ch) else {
        return;
    };
    let alpha_scale = img.alpha.clamp(0.0, 1.0);
    let dst = sub_pix.pixels_mut();
    for row in 0..ch {
        for col in 0..cw {
            let src_row = cy + (ch - 1 - row);
            let src_col = cx + col;
            let base = ((src_row * img.width_px + src_col) as usize) * 4;
            let r = img.rgba_pixels[base];
            let g = img.rgba_pixels[base + 1];
            let b = img.rgba_pixels[base + 2];
            let a_f = (f32::from(img.rgba_pixels[base + 3]) / RGBA_CHANNEL_MAX_F32) * alpha_scale;
            let a = (a_f * RGBA_CHANNEL_MAX_F32).round() as u8;
            let pm_r = ((f32::from(r) * a_f).round() as u8).min(a);
            let pm_g = ((f32::from(g) * a_f).round() as u8).min(a);
            let pm_b = ((f32::from(b) * a_f).round() as u8).min(a);
            if let Some(pm) = PremultipliedColorU8::from_rgba(pm_r, pm_g, pm_b, a) {
                dst[(row * cw + col) as usize] = pm;
            }
        }
    }

    let vp = state.active_slot_viewport();
    let s0 = vp.to_screen(corners_upm[0]);
    let s1 = vp.to_screen(corners_upm[1]);
    let s2 = vp.to_screen(corners_upm[2]);
    let s3 = vp.to_screen(corners_upm[3]);
    let ux = (s1.x.0 - s0.x.0) / (cw as f32);
    let uy = (s1.y.0 - s0.y.0) / (cw as f32);
    let vx = (s3.x.0 - s0.x.0) / (ch as f32);
    let vy = (s3.y.0 - s0.y.0) / (ch as f32);
    let xform = Transform::from_row(ux, uy, vx, vy, s0.x.0, s0.y.0);
    pixmap.draw_pixmap(0, 0, sub_pix.as_ref(), &PixmapPaint::default(), xform, None);

    if img.selected {
        let mut pb = PathBuilder::new();
        pb.move_to(s0.x.0, s0.y.0);
        pb.line_to(s1.x.0, s1.y.0);
        pb.line_to(s2.x.0, s2.y.0);
        pb.line_to(s3.x.0, s3.y.0);
        pb.close();
        stroke_ts(
            pixmap,
            pb,
            color32_to_tiny_skia(theme.edit_canvas.selected_glow_color32()),
            BG_IMAGE_SELECTION_WIDTH_DP,
        );
    }
}

pub fn render_spline_node_marker_tiny_skia(
    pixmap: &mut Pixmap,
    cx: f32,
    cy: f32,
    node_type: NodeType,
    theme: &GrafiaTheme,
) {
    match node_type {
        NodeType::HobbyStar { .. } => {
            let col = color32_to_tiny_skia(theme.hobby_star_node_color32());
            let mut pb = PathBuilder::new();
            for i in 0..HOBBY_STAR_VERTICES {
                let r = if i % 2 == 0 {
                    HOBBY_STAR_OUTER_RADIUS_DP
                } else {
                    HOBBY_STAR_INNER_RADIUS_DP
                };
                let ang = HOBBY_STAR_TOP_ANGLE_RAD + (i as f32) * HOBBY_STAR_HALF_STEP_RAD;
                let px = cx + r * ang.cos();
                let py = cy + r * ang.sin();
                if i == 0 {
                    pb.move_to(px, py);
                } else {
                    pb.line_to(px, py);
                }
            }
            pb.close();
            if let Some(path) = pb.finish() {
                let mut paint = Paint::default();
                paint.set_color(col);
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
        NodeType::EulerSpiro { .. } => {
            let col = color32_to_tiny_skia(theme.euler_spiro_node_color32());
            let mut ring_pb = PathBuilder::new();
            ring_pb.push_circle(cx, cy, EULER_SPIRO_OUTER_RADIUS_DP);
            stroke_ts(pixmap, ring_pb, col, EULER_SPIRO_RING_WIDTH_DP);
            fill_circle_ts(pixmap, cx, cy, EULER_SPIRO_INNER_RADIUS_DP, col);
        }
        _ => {}
    }
}

pub fn render_spline_and_handle_overlays_tiny_skia(
    pixmap: &mut Pixmap,
    state: &GrafiaCanvasState,
    theme: &GrafiaTheme,
) {
    if state.view_mode != CanvasViewMode::EditWireframe {
        return;
    }
    let vp = state.active_slot_viewport();

    if state.show_background && state.show_background_nodes {
        render_background_nodes_ts(pixmap, state, theme, &vp);
    }
    if state.show_component_piece_frames {
        render_component_frames_ts(pixmap, state, theme, &vp);
    }
    render_smart_handles_ts(pixmap, state, theme, &vp);
    if state.show_extra_nodes {
        render_extra_nodes_ts(pixmap, state, theme, &vp);
    }
    if state.show_node_coordinates {
        render_node_coordinates_ts(pixmap, state, theme, &vp);
    }
    render_rotation_pill_ts(pixmap, state, &vp);
}

fn render_background_nodes_ts(
    pixmap: &mut Pixmap,
    state: &GrafiaCanvasState,
    theme: &GrafiaTheme,
    vp: &crate::core::ViewportTransform,
) {
    let half = theme.edit_canvas.node_size_dp * 0.5;
    let size = theme.edit_canvas.node_size_dp;
    let bg_col = color32_to_tiny_skia(theme.background_node_fill_color32());
    let mut paint = Paint::default();
    paint.set_color(bg_col);
    for gc in &state.background_contours {
        if !gc.visible {
            continue;
        }
        for n in &gc.nodes {
            if n.node_type == NodeType::OffCurve && !state.show_nodes_offcurve {
                continue;
            }
            if n.node_type != NodeType::OffCurve && !state.show_nodes_oncurve {
                continue;
            }
            let scr = vp.to_screen(n.point);
            if n.node_type == NodeType::OffCurve {
                fill_circle_ts(pixmap, scr.x.0, scr.y.0, half * 0.5, bg_col);
            } else if let Some(r) = TsRect::from_xywh(scr.x.0 - half, scr.y.0 - half, size, size) {
                pixmap.fill_rect(r, &paint, Transform::identity(), None);
            }
        }
    }
}

fn render_component_frames_ts(
    pixmap: &mut Pixmap,
    state: &GrafiaCanvasState,
    theme: &GrafiaTheme,
    vp: &crate::core::ViewportTransform,
) {
    let frame_col = color32_to_tiny_skia(theme.component_piece_frame_color32());
    for comp in &state.components {
        let transformed = comp.transformed_contours();
        if let Some((min_x, max_x, min_y, max_y)) = component_bounds_upm(&transformed) {
            let s0 = vp.to_screen(FontPoint::new(min_x, max_y));
            let s1 = vp.to_screen(FontPoint::new(max_x, min_y));
            stroke_rect_ts(
                pixmap,
                s0.x.0,
                s0.y.0,
                s1.x.0,
                s1.y.0,
                frame_col,
                COMPONENT_PIECE_FRAME_WIDTH_DP,
            );
        }
    }
}

fn render_smart_handles_ts(
    pixmap: &mut Pixmap,
    state: &GrafiaCanvasState,
    theme: &GrafiaTheme,
    vp: &crate::core::ViewportTransform,
) {
    for handle in &state.smart_handles {
        let is_sel_comp = state
            .components
            .iter()
            .any(|c| c.id == handle.component_id && c.selected);
        if !is_sel_comp {
            continue;
        }
        let (p_min, p_def, p_max) = smart_handle_track_endpoints_upm(handle);
        let s_min = vp.to_screen(p_min);
        let s_def = vp.to_screen(p_def);
        let s_max = vp.to_screen(p_max);
        let s_cur = vp.to_screen(smart_handle_upm_position(handle));
        let track_col = color32_to_tiny_skia(theme.smart_handle_track_color32());
        let fill_col = color32_to_tiny_skia(theme.smart_handle_fill_color32());

        stroke_line_ts(
            pixmap,
            s_min.x.0,
            s_min.y.0,
            s_max.x.0,
            s_max.y.0,
            track_col,
            SMART_HANDLE_TRACK_WIDTH_DP,
        );
        fill_circle_ts(
            pixmap,
            s_def.x.0,
            s_def.y.0,
            SMART_HANDLE_TICK_RADIUS_DP,
            track_col,
        );

        let r = SMART_HANDLE_DIAMOND_RADIUS_DP;
        let mut d_pb = PathBuilder::new();
        d_pb.move_to(s_cur.x.0, s_cur.y.0 - r);
        d_pb.line_to(s_cur.x.0 + r, s_cur.y.0);
        d_pb.line_to(s_cur.x.0, s_cur.y.0 + r);
        d_pb.line_to(s_cur.x.0 - r, s_cur.y.0);
        d_pb.close();
        if let Some(path) = d_pb.finish() {
            let mut fp = Paint::default();
            fp.set_color(fill_col);
            fp.anti_alias = true;
            pixmap.fill_path(&path, &fp, FillRule::Winding, Transform::identity(), None);
        }
        fill_pill_ts(
            pixmap,
            s_cur.x.0,
            s_cur.y.0 + SMART_HANDLE_PILL_OFFSET_Y_DP,
            &format!("{:.1}", handle.current_val),
        );
    }
}

fn render_extra_nodes_ts(
    pixmap: &mut Pixmap,
    state: &GrafiaCanvasState,
    theme: &GrafiaTheme,
    vp: &crate::core::ViewportTransform,
) {
    let stroke_col = color32_to_tiny_skia(theme.extra_node_stroke_color32());
    let sel_fill_col = color32_to_tiny_skia(theme.extra_node_selected_fill_color32());
    let r = EXTRA_NODE_DIAMOND_RADIUS_DP;
    for en in cached_extra_nodes(state) {
        let scr = vp.to_screen(en.position);
        let mut pb = PathBuilder::new();
        pb.move_to(scr.x.0, scr.y.0 - r);
        pb.line_to(scr.x.0 + r, scr.y.0);
        pb.line_to(scr.x.0, scr.y.0 + r);
        pb.line_to(scr.x.0 - r, scr.y.0);
        pb.close();
        if state.selected_extra_node == Some(en.key) {
            let mut fpb = PathBuilder::new();
            fpb.move_to(scr.x.0, scr.y.0 - r);
            fpb.line_to(scr.x.0 + r, scr.y.0);
            fpb.line_to(scr.x.0, scr.y.0 + r);
            fpb.line_to(scr.x.0 - r, scr.y.0);
            fpb.close();
            if let Some(fpath) = fpb.finish() {
                let mut fp = Paint::default();
                fp.set_color(sel_fill_col);
                fp.anti_alias = true;
                pixmap.fill_path(&fpath, &fp, FillRule::Winding, Transform::identity(), None);
            }
        }
        stroke_ts(pixmap, pb, stroke_col, EXTRA_NODE_STROKE_WIDTH_DP);
    }
}

fn render_node_coordinates_ts(
    pixmap: &mut Pixmap,
    state: &GrafiaCanvasState,
    theme: &GrafiaTheme,
    vp: &crate::core::ViewportTransform,
) {
    let label_col = color32_to_tiny_skia(theme.node_coordinate_label_color32());
    for contour in &state.contours {
        for n in contour {
            if n.node_type == NodeType::OffCurve && !state.show_nodes_offcurve {
                continue;
            }
            if n.node_type != NodeType::OffCurve && !state.show_nodes_oncurve {
                continue;
            }
            let scr = vp.to_screen(n.point);
            draw_bitmap_string_ts(
                pixmap,
                scr.x.0 + NODE_COORD_LABEL_OFFSET_X_DP,
                scr.y.0 - NODE_COORD_LABEL_OFFSET_Y_DP - PILL_GLYPH_H_DP,
                &format!("{:.0},{:.0}", n.point.x.0, n.point.y.0),
                label_col,
            );
        }
    }
}

fn render_rotation_pill_ts(
    pixmap: &mut Pixmap,
    state: &GrafiaCanvasState,
    vp: &crate::core::ViewportTransform,
) {
    if let Some(rot_label) = state.active_bbox_rotation_pill_label() {
        let base_pos = state
            .last_pointer_screen_pos
            .map(|p| (p.x.0, p.y.0))
            .or_else(|| {
                state.selection_bounds().map(|b| {
                    let k8 = b.knob_screen_center(8, vp);
                    (k8.x.0, k8.y.0)
                })
            });
        if let Some((ax, ay)) = base_pos {
            fill_pill_ts(
                pixmap,
                ax + BBOX_ROTATE_PILL_OFFSET_X_DP,
                ay + BBOX_ROTATE_PILL_OFFSET_Y_DP,
                &rot_label,
            );
        }
    }
}
