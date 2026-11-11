//! `SPEC-008` (`grafia v0.7.0`) canvas overlays for `egui` and `tiny-skia`:
//! `GhostMarkPath` mark preview cloud, 2D `CanvasComponent` silhouettes,
//! Live `ContourStrokeStyle` expanded outlines, `CompanionMasterLayer` contours,
//! and Master Compatibility (`Ctrl+Opt+Cmd+N`) start-node & anchor vectors.

use crate::adapters::headless::kurbo_to_tiny_skia_path;
use crate::adapters::state::{CanvasViewMode, GrafiaCanvasState};
use crate::core::{extract_segments, ContourSegment, Node, ViewportTransform};
use crate::render::tessellate_glyph_contours_nonzero;
use crate::tokens::{
    color32_to_tiny_skia, preview_rgba_slice_to_color32, GrafiaTheme, BACKGROUND_STROKE_WIDTH_DP,
    COMPANION_LAYER_STROKE_WIDTH_DP, CURVE_FLATTEN_TOLERANCE, MASTER_COMPAT_DOT_RADIUS_DP,
    MASTER_COMPAT_RAY_STROKE_WIDTH_DP, MASTER_INCOMPAT_STROKE_WIDTH_DP,
};
use egui::{Painter, Pos2, Shape, Stroke};
use kurbo::{BezPath, Point as KurboPoint};
use tiny_skia::{FillRule, Paint, PathBuilder, Pixmap, Stroke as TsStroke, Transform};

fn build_screen_bezpath(nodes: &[Node], closed: bool, vp: &ViewportTransform) -> BezPath {
    let mut path = BezPath::new();
    if nodes.len() < 2 {
        return path;
    }
    let s0 = vp.to_screen(nodes[0].point);
    path.move_to(KurboPoint::new(f64::from(s0.x.0), f64::from(s0.y.0)));
    for seg in extract_segments(nodes, closed) {
        match seg {
            ContourSegment::Line { end_idx, .. } => {
                let p = vp.to_screen(nodes[end_idx].point);
                path.line_to(KurboPoint::new(f64::from(p.x.0), f64::from(p.y.0)));
            }
            ContourSegment::Cubic {
                h1_idx,
                h2_idx,
                end_idx,
                ..
            } => {
                let h1 = vp.to_screen(nodes[h1_idx].point);
                let h2 = vp.to_screen(nodes[h2_idx].point);
                let p3 = vp.to_screen(nodes[end_idx].point);
                path.curve_to(
                    KurboPoint::new(f64::from(h1.x.0), f64::from(h1.y.0)),
                    KurboPoint::new(f64::from(h2.x.0), f64::from(h2.y.0)),
                    KurboPoint::new(f64::from(p3.x.0), f64::from(p3.y.0)),
                );
            }
        }
    }
    if closed {
        path.close_path();
    }
    path
}

fn paint_contour_group_egui(
    painter: &Painter,
    contours: &[crate::core::GlyphContour],
    vp: &ViewportTransform,
    fill_col: egui::Color32,
    stroke_col: egui::Color32,
    stroke_w: f32,
) {
    if contours.is_empty() {
        return;
    }
    let mesh = tessellate_glyph_contours_nonzero(contours, vp, fill_col);
    if !mesh.is_empty() {
        painter.add(Shape::mesh(mesh));
    }
    let stroke = Stroke::new(stroke_w, stroke_col);
    for c in contours {
        if c.nodes.len() < 2 {
            continue;
        }
        let bez = build_screen_bezpath(&c.nodes, c.closed, vp);
        let mut pts = Vec::new();
        kurbo::flatten(bez.iter(), CURVE_FLATTEN_TOLERANCE, |el| match el {
            kurbo::PathEl::MoveTo(pt) | kurbo::PathEl::LineTo(pt) => {
                pts.push(Pos2::new(pt.x as f32, pt.y as f32));
            }
            _ => {}
        });
        if pts.len() >= 2 {
            painter.add(if c.closed {
                Shape::closed_line(pts, stroke)
            } else {
                Shape::line(pts, stroke)
            });
        }
    }
}

/// Paints `SPEC-008` (`v0.7.0`) overlays into `egui::Painter`.
pub fn paint_v070_overlays_egui(painter: &Painter, state: &GrafiaCanvasState, theme: &GrafiaTheme) {
    if state.view_mode == CanvasViewMode::ShapeThumbnail {
        return;
    }
    let vp = state.active_slot_viewport();

    for comp in &state.components {
        let col = theme.component_fill_color32(comp.selected, comp.locked);
        let transformed = comp.transformed_contours();
        paint_contour_group_egui(
            painter,
            &transformed,
            &vp,
            col,
            col,
            BACKGROUND_STROKE_WIDTH_DP,
        );
    }

    let cloud = state.compute_active_mark_cloud_contours();
    if !cloud.is_empty() {
        let col = theme.mark_cloud_fill_color32();
        paint_contour_group_egui(painter, &cloud, &vp, col, col, BACKGROUND_STROKE_WIDTH_DP);
    }

    if !state.contour_strokes.is_empty() {
        let expanded = state.compute_expanded_stroke_contours();
        let col = theme.component_fill_color32(true, false);
        paint_contour_group_egui(
            painter,
            &expanded,
            &vp,
            col,
            col,
            COMPANION_LAYER_STROKE_WIDTH_DP,
        );
    }

    for layer in &state.companion_master_layers {
        let col = preview_rgba_slice_to_color32(&layer.color_rgba);
        let stroke = Stroke::new(COMPANION_LAYER_STROKE_WIDTH_DP, col);
        for (ci, nodes) in layer.contours.iter().enumerate() {
            let closed = layer.closed_flags.get(ci).copied().unwrap_or(true);
            let bez = build_screen_bezpath(nodes, closed, &vp);
            let mut pts = Vec::new();
            kurbo::flatten(bez.iter(), CURVE_FLATTEN_TOLERANCE, |el| match el {
                kurbo::PathEl::MoveTo(pt) | kurbo::PathEl::LineTo(pt) => {
                    pts.push(Pos2::new(pt.x as f32, pt.y as f32));
                }
                _ => {}
            });
            if pts.len() >= 2 {
                painter.add(if closed {
                    Shape::closed_line(pts, stroke)
                } else {
                    Shape::line(pts, stroke)
                });
            }
        }
    }

    if state.show_master_compatibility {
        let rep = state.compute_master_compatibility();
        let incompat_stroke = Stroke::new(
            MASTER_INCOMPAT_STROKE_WIDTH_DP,
            theme.master_incompat_highlight_color32(),
        );
        for &ci in &rep.incompatible_contour_indices {
            if let Some(gc) = state.glyph.contours.get(ci) {
                let bez = build_screen_bezpath(&gc.nodes, gc.closed, &vp);
                let mut pts = Vec::new();
                kurbo::flatten(bez.iter(), CURVE_FLATTEN_TOLERANCE, |el| match el {
                    kurbo::PathEl::MoveTo(pt) | kurbo::PathEl::LineTo(pt) => {
                        pts.push(Pos2::new(pt.x as f32, pt.y as f32));
                    }
                    _ => {}
                });
                if pts.len() >= 2 {
                    painter.add(if gc.closed {
                        Shape::closed_line(pts, incompat_stroke)
                    } else {
                        Shape::line(pts, incompat_stroke)
                    });
                }
            }
        }
        let ray_col = theme.master_compat_ray_color32();
        let ray_stroke = Stroke::new(MASTER_COMPAT_RAY_STROKE_WIDTH_DP, ray_col);
        for (a, b) in rep
            .start_node_vectors
            .iter()
            .chain(rep.anchor_vectors.iter())
        {
            let sa = vp.to_screen(*a);
            let sb = vp.to_screen(*b);
            let pa = Pos2::new(sa.x.0, sa.y.0);
            let pb = Pos2::new(sb.x.0, sb.y.0);
            painter.line_segment([pa, pb], ray_stroke);
            painter.circle_filled(pa, MASTER_COMPAT_DOT_RADIUS_DP, ray_col);
            painter.circle_filled(pb, MASTER_COMPAT_DOT_RADIUS_DP, ray_col);
        }
    }
}

fn render_contour_group_tiny_skia(
    pixmap: &mut Pixmap,
    contours: &[crate::core::GlyphContour],
    vp: &ViewportTransform,
    col: egui::Color32,
    stroke_w: f32,
    fill_rule: FillRule,
) {
    if contours.is_empty() {
        return;
    }
    let mut paint = Paint::default();
    paint.set_color(color32_to_tiny_skia(col));
    paint.anti_alias = true;
    let stroke = TsStroke {
        width: stroke_w,
        ..Default::default()
    };

    let mut compound = BezPath::new();
    for c in contours {
        if c.nodes.len() < 2 {
            continue;
        }
        let bez = build_screen_bezpath(&c.nodes, c.closed, vp);
        if let Some(ts_path) = kurbo_to_tiny_skia_path(&bez) {
            pixmap.stroke_path(&ts_path, &paint, &stroke, Transform::identity(), None);
        }
        if c.closed {
            for el in bez.elements() {
                compound.push(*el);
            }
        }
    }
    if let Some(ts_fill) = kurbo_to_tiny_skia_path(&compound) {
        pixmap.fill_path(&ts_fill, &paint, fill_rule, Transform::identity(), None);
    }
}

/// Renders `SPEC-008` (`v0.7.0`) overlays into a `tiny_skia::Pixmap`.
pub fn render_v070_overlays_tiny_skia(
    pixmap: &mut Pixmap,
    state: &GrafiaCanvasState,
    theme: &GrafiaTheme,
) {
    if state.view_mode == CanvasViewMode::ShapeThumbnail {
        return;
    }
    let vp = state.active_slot_viewport();

    for comp in &state.components {
        let col = theme.component_fill_color32(comp.selected, comp.locked);
        let transformed = comp.transformed_contours();
        render_contour_group_tiny_skia(
            pixmap,
            &transformed,
            &vp,
            col,
            BACKGROUND_STROKE_WIDTH_DP,
            FillRule::Winding,
        );
    }

    let cloud = state.compute_active_mark_cloud_contours();
    if !cloud.is_empty() {
        let col = theme.mark_cloud_fill_color32();
        render_contour_group_tiny_skia(
            pixmap,
            &cloud,
            &vp,
            col,
            BACKGROUND_STROKE_WIDTH_DP,
            FillRule::Winding,
        );
    }

    if !state.contour_strokes.is_empty() {
        let expanded = state.compute_expanded_stroke_contours();
        let col = theme.component_fill_color32(true, false);
        render_contour_group_tiny_skia(
            pixmap,
            &expanded,
            &vp,
            col,
            COMPANION_LAYER_STROKE_WIDTH_DP,
            FillRule::EvenOdd,
        );
    }

    for layer in &state.companion_master_layers {
        let col = preview_rgba_slice_to_color32(&layer.color_rgba);
        let mut paint = Paint::default();
        paint.set_color(color32_to_tiny_skia(col));
        paint.anti_alias = true;
        let stroke = TsStroke {
            width: COMPANION_LAYER_STROKE_WIDTH_DP,
            ..Default::default()
        };
        for (ci, nodes) in layer.contours.iter().enumerate() {
            let closed = layer.closed_flags.get(ci).copied().unwrap_or(true);
            let bez = build_screen_bezpath(nodes, closed, &vp);
            if let Some(ts_path) = kurbo_to_tiny_skia_path(&bez) {
                pixmap.stroke_path(&ts_path, &paint, &stroke, Transform::identity(), None);
            }
        }
    }

    if state.show_master_compatibility {
        let rep = state.compute_master_compatibility();
        let mut inc_paint = Paint::default();
        inc_paint.set_color(color32_to_tiny_skia(
            theme.master_incompat_highlight_color32(),
        ));
        inc_paint.anti_alias = true;
        let inc_stroke = TsStroke {
            width: MASTER_INCOMPAT_STROKE_WIDTH_DP,
            ..Default::default()
        };
        for &ci in &rep.incompatible_contour_indices {
            if let Some(gc) = state.glyph.contours.get(ci) {
                let bez = build_screen_bezpath(&gc.nodes, gc.closed, &vp);
                if let Some(ts_path) = kurbo_to_tiny_skia_path(&bez) {
                    pixmap.stroke_path(
                        &ts_path,
                        &inc_paint,
                        &inc_stroke,
                        Transform::identity(),
                        None,
                    );
                }
            }
        }

        let mut ray_paint = Paint::default();
        ray_paint.set_color(color32_to_tiny_skia(theme.master_compat_ray_color32()));
        ray_paint.anti_alias = true;
        let ray_stroke = TsStroke {
            width: MASTER_COMPAT_RAY_STROKE_WIDTH_DP,
            ..Default::default()
        };
        let mut pb = PathBuilder::new();
        for (a, b) in rep
            .start_node_vectors
            .iter()
            .chain(rep.anchor_vectors.iter())
        {
            let sa = vp.to_screen(*a);
            let sb = vp.to_screen(*b);
            pb.move_to(sa.x.0, sa.y.0);
            pb.line_to(sb.x.0, sb.y.0);
        }
        if let Some(ray_path) = pb.finish() {
            pixmap.stroke_path(
                &ray_path,
                &ray_paint,
                &ray_stroke,
                Transform::identity(),
                None,
            );
        }
        let mut dot_pb = PathBuilder::new();
        for (a, b) in rep
            .start_node_vectors
            .iter()
            .chain(rep.anchor_vectors.iter())
        {
            let sa = vp.to_screen(*a);
            let sb = vp.to_screen(*b);
            dot_pb.push_circle(sa.x.0, sa.y.0, MASTER_COMPAT_DOT_RADIUS_DP);
            dot_pb.push_circle(sb.x.0, sb.y.0, MASTER_COMPAT_DOT_RADIUS_DP);
        }
        if let Some(dot_path) = dot_pb.finish() {
            pixmap.fill_path(
                &dot_path,
                &ray_paint,
                FillRule::Winding,
                Transform::identity(),
                None,
            );
        }
    }
}
