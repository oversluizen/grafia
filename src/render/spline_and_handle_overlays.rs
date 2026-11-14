//! `SPEC-009` (`grafia v0.8.0` Part A) `egui::Painter` overlays: `HobbyStar` (`★`) and `EulerSpiro` (`◎`)
//! markers, BBox rotation `"∠ ..."` pill, `CanvasSmartHandle`s, `VirtualExtraNode`s, `CanvasBackgroundImage`,
//! `show_node_coordinates`, `show_background_nodes`, and `show_component_piece_frames`.

pub use crate::render::splines_pixmap_pass::{
    render_background_image_tiny_skia, render_spline_and_handle_overlays_tiny_skia,
    render_spline_node_marker_tiny_skia,
};

use crate::adapters::state::{CanvasViewMode, GrafiaCanvasState};
use crate::core::smart_handles::{smart_handle_track_endpoints_upm, smart_handle_upm_position};
use crate::core::{FontPoint, GlyphContour, NodeType};
use crate::render::canvas_render_cache::{cached_extra_nodes, get_or_upload_background_texture};
use crate::render::ts_primitives::paint_pill_egui;
use crate::tokens::{
    white_tint_with_alpha_color32, GrafiaTheme, BBOX_ROTATE_PILL_OFFSET_X_DP,
    BBOX_ROTATE_PILL_OFFSET_Y_DP, BG_IMAGE_SELECTION_WIDTH_DP, COMPONENT_PIECE_FRAME_WIDTH_DP,
    DEG_TO_RAD_F64, EULER_SPIRO_INNER_RADIUS_DP, EULER_SPIRO_OUTER_RADIUS_DP,
    EULER_SPIRO_RING_WIDTH_DP, EXTRA_NODE_DIAMOND_RADIUS_DP, EXTRA_NODE_STROKE_WIDTH_DP,
    HOBBY_STAR_HALF_STEP_RAD, HOBBY_STAR_INNER_RADIUS_DP, HOBBY_STAR_OUTER_RADIUS_DP,
    HOBBY_STAR_TOP_ANGLE_RAD, HOBBY_STAR_VERTICES, NODE_COORD_LABEL_FONT_SIZE_DP,
    NODE_COORD_LABEL_OFFSET_X_DP, NODE_COORD_LABEL_OFFSET_Y_DP, SMART_HANDLE_DIAMOND_RADIUS_DP,
    SMART_HANDLE_PILL_OFFSET_Y_DP, SMART_HANDLE_TICK_RADIUS_DP, SMART_HANDLE_TRACK_WIDTH_DP,
};
use egui::epaint::Vertex;
use egui::{Align2, FontId, Mesh, Painter, Pos2, Rect, Shape, Stroke};

pub(crate) fn component_bounds_upm(contours: &[GlyphContour]) -> Option<(f64, f64, f64, f64)> {
    let mut min_x = f64::INFINITY;
    let mut max_x = f64::NEG_INFINITY;
    let mut min_y = f64::INFINITY;
    let mut max_y = f64::NEG_INFINITY;
    let mut any = false;
    for gc in contours {
        if !gc.visible {
            continue;
        }
        for n in &gc.nodes {
            min_x = min_x.min(n.point.x.0);
            max_x = max_x.max(n.point.x.0);
            min_y = min_y.min(n.point.y.0);
            max_y = max_y.max(n.point.y.0);
            any = true;
        }
    }
    any.then_some((min_x, max_x, min_y, max_y))
}

pub(crate) fn bg_image_quad_upm(
    img: &crate::tokens::CanvasBackgroundImage,
) -> Option<([u32; 4], [FontPoint; 4])> {
    if img.width_px == 0 || img.height_px == 0 || img.alpha <= 0.0 {
        return None;
    }
    let expected = (img.width_px as usize)
        .saturating_mul(img.height_px as usize)
        .saturating_mul(4);
    if img.rgba_pixels.len() < expected {
        return None;
    }
    let [raw_x, raw_y, raw_w, raw_h] =
        img.crop_rect_px
            .unwrap_or([0, 0, img.width_px, img.height_px]);
    let cx = raw_x.min(img.width_px.saturating_sub(1));
    let cy = raw_y.min(img.height_px.saturating_sub(1));
    let cw = raw_w.min(img.width_px.saturating_sub(cx));
    let ch = raw_h.min(img.height_px.saturating_sub(cy));
    if cw == 0 || ch == 0 {
        return None;
    }
    let w_upm = f64::from(cw) * img.scale_x;
    let h_upm = f64::from(ch) * img.scale_y;
    let rad = img.rotation_deg * DEG_TO_RAD_F64;
    let (cr, sr) = (rad.cos(), rad.sin());
    let rot = |lx: f64, ly: f64| -> FontPoint {
        FontPoint::new(
            img.position.x.0 + lx * cr - ly * sr,
            img.position.y.0 + lx * sr + ly * cr,
        )
    };
    let corners = [
        rot(0.0, 0.0),
        rot(w_upm, 0.0),
        rot(w_upm, h_upm),
        rot(0.0, h_upm),
    ];
    Some(([cx, cy, cw, ch], corners))
}

pub fn paint_background_image_egui(
    painter: &Painter,
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
    let vp = state.active_slot_viewport();
    let scr: [Pos2; 4] = corners_upm.map(|pt| {
        let s = vp.to_screen(pt);
        Pos2::new(s.x.0, s.y.0)
    });

    let tex = get_or_upload_background_texture(
        painter.ctx(),
        state.content_generation(),
        img,
        [cx, cy, cw, ch],
    );
    let tint = white_tint_with_alpha_color32(img.alpha);

    let mut mesh = Mesh::with_texture(tex.id());
    let uvs = [
        Pos2::new(0.0, 1.0),
        Pos2::new(1.0, 1.0),
        Pos2::new(1.0, 0.0),
        Pos2::new(0.0, 0.0),
    ];
    for i in 0..4 {
        mesh.vertices.push(Vertex {
            pos: scr[i],
            uv: uvs[i],
            color: tint,
        });
    }
    mesh.indices.extend_from_slice(&[0, 1, 2, 0, 2, 3]);
    painter.add(Shape::mesh(mesh));

    if img.selected {
        let stroke = Stroke::new(
            BG_IMAGE_SELECTION_WIDTH_DP,
            theme.edit_canvas.selected_glow_color32(),
        );
        painter.add(Shape::closed_line(scr.to_vec(), stroke));
    }
}

pub fn paint_spline_node_marker_egui(
    painter: &Painter,
    center: Pos2,
    node_type: NodeType,
    theme: &GrafiaTheme,
) {
    match node_type {
        NodeType::HobbyStar { .. } => {
            let col = theme.hobby_star_node_color32();
            let mut verts = [Pos2::ZERO; HOBBY_STAR_VERTICES];
            for (i, slot) in verts.iter_mut().enumerate() {
                let r = if i % 2 == 0 {
                    HOBBY_STAR_OUTER_RADIUS_DP
                } else {
                    HOBBY_STAR_INNER_RADIUS_DP
                };
                let ang = HOBBY_STAR_TOP_ANGLE_RAD + (i as f32) * HOBBY_STAR_HALF_STEP_RAD;
                *slot = Pos2::new(center.x + r * ang.cos(), center.y + r * ang.sin());
            }
            for i in 0..HOBBY_STAR_VERTICES {
                let p0 = verts[i];
                let p1 = verts[(i + 1) % HOBBY_STAR_VERTICES];
                painter.add(Shape::convex_polygon(
                    vec![center, p0, p1],
                    col,
                    Stroke::NONE,
                ));
            }
        }
        NodeType::EulerSpiro { .. } => {
            let col = theme.euler_spiro_node_color32();
            painter.circle_stroke(
                center,
                EULER_SPIRO_OUTER_RADIUS_DP,
                Stroke::new(EULER_SPIRO_RING_WIDTH_DP, col),
            );
            painter.circle_filled(center, EULER_SPIRO_INNER_RADIUS_DP, col);
        }
        _ => {}
    }
}

pub fn paint_spline_and_handle_overlays_egui(
    painter: &Painter,
    state: &GrafiaCanvasState,
    theme: &GrafiaTheme,
) {
    if state.view_mode != CanvasViewMode::EditWireframe {
        return;
    }
    let vp = state.active_slot_viewport();

    if state.show_background && state.show_background_nodes {
        paint_background_nodes_egui(painter, state, theme, &vp);
    }
    if state.show_component_piece_frames {
        paint_component_frames_egui(painter, state, theme, &vp);
    }
    paint_smart_handles_egui(painter, state, theme, &vp);
    if state.show_extra_nodes {
        paint_extra_nodes_egui(painter, state, theme, &vp);
    }
    if state.show_node_coordinates {
        paint_node_coordinates_egui(painter, state, theme, &vp);
    }
    paint_rotation_pill_egui(painter, state, &vp);
}

fn paint_background_nodes_egui(
    painter: &Painter,
    state: &GrafiaCanvasState,
    theme: &GrafiaTheme,
    vp: &crate::core::ViewportTransform,
) {
    let half = theme.edit_canvas.node_size_dp * 0.5;
    let bg_col = theme.background_node_fill_color32();
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
            let c = Pos2::new(scr.x.0, scr.y.0);
            if n.node_type == NodeType::OffCurve {
                painter.circle_filled(c, half * 0.5, bg_col);
            } else {
                painter.rect_filled(
                    Rect::from_min_max(
                        Pos2::new(c.x - half, c.y - half),
                        Pos2::new(c.x + half, c.y + half),
                    ),
                    0.0,
                    bg_col,
                );
            }
        }
    }
}

fn paint_component_frames_egui(
    painter: &Painter,
    state: &GrafiaCanvasState,
    theme: &GrafiaTheme,
    vp: &crate::core::ViewportTransform,
) {
    let frame_stroke = Stroke::new(
        COMPONENT_PIECE_FRAME_WIDTH_DP,
        theme.component_piece_frame_color32(),
    );
    for comp in &state.components {
        let transformed = comp.transformed_contours();
        if let Some((min_x, max_x, min_y, max_y)) = component_bounds_upm(&transformed) {
            let s0 = vp.to_screen(FontPoint::new(min_x, max_y));
            let s1 = vp.to_screen(FontPoint::new(max_x, min_y));
            let r = Rect::from_two_pos(Pos2::new(s0.x.0, s0.y.0), Pos2::new(s1.x.0, s1.y.0));
            painter.add(Shape::closed_line(
                vec![
                    r.left_top(),
                    r.right_top(),
                    r.right_bottom(),
                    r.left_bottom(),
                ],
                frame_stroke,
            ));
        }
    }
}

fn paint_smart_handles_egui(
    painter: &Painter,
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
        let track_col = theme.smart_handle_track_color32();
        let fill_col = theme.smart_handle_fill_color32();
        painter.line_segment(
            [
                Pos2::new(s_min.x.0, s_min.y.0),
                Pos2::new(s_max.x.0, s_max.y.0),
            ],
            Stroke::new(SMART_HANDLE_TRACK_WIDTH_DP, track_col),
        );
        painter.circle_filled(
            Pos2::new(s_def.x.0, s_def.y.0),
            SMART_HANDLE_TICK_RADIUS_DP,
            track_col,
        );
        let r = SMART_HANDLE_DIAMOND_RADIUS_DP;
        let diamond = vec![
            Pos2::new(s_cur.x.0, s_cur.y.0 - r),
            Pos2::new(s_cur.x.0 + r, s_cur.y.0),
            Pos2::new(s_cur.x.0, s_cur.y.0 + r),
            Pos2::new(s_cur.x.0 - r, s_cur.y.0),
        ];
        painter.add(Shape::convex_polygon(diamond, fill_col, Stroke::NONE));
        paint_pill_egui(
            painter,
            Pos2::new(s_cur.x.0, s_cur.y.0 + SMART_HANDLE_PILL_OFFSET_Y_DP),
            &format!("{:.1}", handle.current_val),
        );
    }
}

fn paint_extra_nodes_egui(
    painter: &Painter,
    state: &GrafiaCanvasState,
    theme: &GrafiaTheme,
    vp: &crate::core::ViewportTransform,
) {
    let stroke = Stroke::new(
        EXTRA_NODE_STROKE_WIDTH_DP,
        theme.extra_node_stroke_color32(),
    );
    let sel_fill = theme.extra_node_selected_fill_color32();
    let r = EXTRA_NODE_DIAMOND_RADIUS_DP;
    for en in cached_extra_nodes(state) {
        let scr = vp.to_screen(en.position);
        let diamond = vec![
            Pos2::new(scr.x.0, scr.y.0 - r),
            Pos2::new(scr.x.0 + r, scr.y.0),
            Pos2::new(scr.x.0, scr.y.0 + r),
            Pos2::new(scr.x.0 - r, scr.y.0),
        ];
        if state.selected_extra_node == Some(en.key) {
            painter.add(Shape::convex_polygon(
                diamond.clone(),
                sel_fill,
                Stroke::NONE,
            ));
        }
        painter.add(Shape::closed_line(diamond, stroke));
    }
}

fn paint_node_coordinates_egui(
    painter: &Painter,
    state: &GrafiaCanvasState,
    theme: &GrafiaTheme,
    vp: &crate::core::ViewportTransform,
) {
    let label_col = theme.node_coordinate_label_color32();
    let font = FontId::monospace(NODE_COORD_LABEL_FONT_SIZE_DP);
    for contour in &state.contours {
        for n in contour {
            if n.node_type == NodeType::OffCurve && !state.show_nodes_offcurve {
                continue;
            }
            if n.node_type != NodeType::OffCurve && !state.show_nodes_oncurve {
                continue;
            }
            let scr = vp.to_screen(n.point);
            painter.text(
                Pos2::new(
                    scr.x.0 + NODE_COORD_LABEL_OFFSET_X_DP,
                    scr.y.0 - NODE_COORD_LABEL_OFFSET_Y_DP,
                ),
                Align2::LEFT_BOTTOM,
                format!("{:.0},{:.0}", n.point.x.0, n.point.y.0),
                font.clone(),
                label_col,
            );
        }
    }
}

fn paint_rotation_pill_egui(
    painter: &Painter,
    state: &GrafiaCanvasState,
    vp: &crate::core::ViewportTransform,
) {
    if let Some(rot_label) = state.active_bbox_rotation_pill_label() {
        let base_pos = state
            .last_pointer_screen_pos
            .map(|p| Pos2::new(p.x.0, p.y.0))
            .or_else(|| {
                state.selection_bounds().map(|b| {
                    let k8 = b.knob_screen_center(8, vp);
                    Pos2::new(k8.x.0, k8.y.0)
                })
            });
        if let Some(anchor) = base_pos {
            paint_pill_egui(
                painter,
                Pos2::new(
                    anchor.x + BBOX_ROTATE_PILL_OFFSET_X_DP,
                    anchor.y + BBOX_ROTATE_PILL_OFFSET_Y_DP,
                ),
                &rot_label,
            );
        }
    }
}
