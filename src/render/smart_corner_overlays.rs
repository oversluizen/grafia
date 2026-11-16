//! `SPEC-012` & `SPEC-013` Pass-4 overlay rendering for Non-Destructive Smart Corners & Ink Traps,
//! Curvature Heatmap Envelopes, Live Optical Symmetry Ghost Contours, and Curved Trajectories
//! across both `egui::Painter` and `tiny_skia::Pixmap`.

pub(crate) use crate::render::stem_gauge_and_audit_overlays::{
    paint_outline_audit_egui, render_outline_audit_tiny_skia,
};

use crate::adapters::state::GrafiaCanvasState;
use crate::core::math_consts::{
    COMB_DEFAULT_SAMPLES, COMB_SCALE_UPM, COMB_STROKE_WIDTH_DP, CURVE_FLATTEN_TOLERANCE,
    TUNNI_LINE_WIDTH_DP,
};
use crate::core::{
    contour_to_bezpath_with_splines, CurvatureCombStyle, FontPoint, ScreenPoint,
    SymmetryOverlayMode, ViewportTransform,
};
use crate::render::stem_gauge_and_audit_overlays::{
    fill_heatmap_quad_ts, paint_centroid_and_stem_gauge_egui, paint_tangent_snap_egui,
    render_centroid_and_stem_gauge_tiny_skia, render_tangent_snap_tiny_skia,
};
use crate::render::ts_primitives::ts_stroke_line;
use crate::tokens::spline_and_handle_tokens::curvature_heatmap_color32;
use crate::tokens::GrafiaTheme;
use egui::{Painter, Pos2, Shape, Stroke};
use kurbo::PathEl;
use tiny_skia::Pixmap;

pub(crate) fn flatten_nodes_to_screen_points(
    nodes: &[crate::core::Node],
    closed: bool,
    vp: &ViewportTransform,
) -> Vec<ScreenPoint> {
    let bez = contour_to_bezpath_with_splines(nodes, closed, 0.0, &[]);
    let mut pts = Vec::new();
    kurbo::flatten(bez.iter(), CURVE_FLATTEN_TOLERANCE, |el| match el {
        PathEl::MoveTo(p) | PathEl::LineTo(p) => {
            pts.push(vp.to_screen(FontPoint::new(p.x, p.y)));
        }
        _ => {}
    });
    pts
}

/// Renders `SPEC-012` Smart Corner preview outlines, Symmetry Ghost contours, and Curvature Heatmap Envelopes into `egui::Painter`.
pub fn paint_smart_corner_and_symmetry_overlays_egui(
    painter: &Painter,
    state: &GrafiaCanvasState,
    theme: &GrafiaTheme,
    vp: &ViewportTransform,
) {
    if !state.smart_corners.is_empty() {
        let preview_stroke = Stroke::new(
            TUNNI_LINE_WIDTH_DP,
            theme.edit_canvas.selected_glow_color32(),
        );
        for preview in state.compute_smart_corner_preview_contours() {
            let scr_pts = flatten_nodes_to_screen_points(&preview.nodes, preview.closed, vp);
            if scr_pts.len() >= 2 {
                let pts: Vec<Pos2> = scr_pts.iter().map(|s| Pos2::new(s.x.0, s.y.0)).collect();
                if preview.closed {
                    painter.add(Shape::closed_line(pts, preview_stroke));
                } else {
                    painter.add(Shape::line(pts, preview_stroke));
                }
            }
        }
    }

    if state.curve_harmony.symmetry_overlay_mode != SymmetryOverlayMode::Off {
        let ghost_stroke = Stroke::new(COMB_STROKE_WIDTH_DP, theme.edit_canvas.bcp_line_color32());
        for (c_idx, ghost_nodes) in state.compute_symmetry_ghost_contours().iter().enumerate() {
            let closed = state.is_contour_closed(c_idx);
            let scr_pts = flatten_nodes_to_screen_points(ghost_nodes, closed, vp);
            if scr_pts.len() >= 2 {
                let pts: Vec<Pos2> = scr_pts.iter().map(|s| Pos2::new(s.x.0, s.y.0)).collect();
                if closed {
                    painter.add(Shape::closed_line(pts, ghost_stroke));
                } else {
                    painter.add(Shape::line(pts, ghost_stroke));
                }
            }
        }
    }

    if state.curve_harmony.show_curvature_comb
        && state.curve_harmony.curvature_comb_style == CurvatureCombStyle::HeatmapEnvelope
    {
        paint_curvature_heatmap_envelope_egui(painter, state, theme, vp);
    }
    paint_centroid_and_stem_gauge_egui(painter, state, theme, vp);
    if let Some(snap) = state.curve_harmony.active_tangent_snap {
        paint_tangent_snap_egui(painter, state, theme, vp, snap);
    }
    crate::render::preview_bands::paint_curved_trajectories_egui(painter, state, theme, vp);
}

fn paint_curvature_heatmap_envelope_egui(
    painter: &Painter,
    state: &GrafiaCanvasState,
    theme: &GrafiaTheme,
    vp: &ViewportTransform,
) {
    let env_stroke = Stroke::new(TUNNI_LINE_WIDTH_DP, theme.edit_canvas.smooth_node_color32());
    for env in state.compute_curvature_heatmap_envelopes(COMB_DEFAULT_SAMPLES, COMB_SCALE_UPM) {
        for pair in env.samples.windows(2) {
            let c0 = curvature_heatmap_color32(
                pair[0].normalized_intensity,
                pair[0].polarity_sign,
                theme.is_dark(),
            );
            let c1 = curvature_heatmap_color32(
                pair[1].normalized_intensity,
                pair[1].polarity_sign,
                theme.is_dark(),
            );
            let b0 = vp.to_screen(pair[0].base_point);
            let b1 = vp.to_screen(pair[1].base_point);
            let t1 = vp.to_screen(pair[1].tip_point);
            let t0 = vp.to_screen(pair[0].tip_point);
            let mut mesh = egui::Mesh::default();
            for (pt, col) in [(b0, c0), (b1, c1), (t1, c1), (t0, c0)] {
                mesh.vertices.push(egui::epaint::Vertex {
                    pos: Pos2::new(pt.x.0, pt.y.0),
                    uv: Pos2::ZERO,
                    color: col,
                });
            }
            mesh.indices.extend_from_slice(&[0, 1, 2, 0, 2, 3]);
            painter.add(Shape::mesh(mesh));
        }
        if env.closed_polygon.len() >= 3 {
            let pts: Vec<Pos2> = env
                .closed_polygon
                .iter()
                .map(|p| {
                    let s = vp.to_screen(*p);
                    Pos2::new(s.x.0, s.y.0)
                })
                .collect();
            painter.add(Shape::closed_line(pts, env_stroke));
        }
    }
}

/// Renders `SPEC-012` Smart Corner preview outlines, Symmetry Ghost contours, and Curvature Heatmap Envelopes into `tiny_skia::Pixmap`.
pub fn render_smart_corner_and_symmetry_overlays_tiny_skia(
    pixmap: &mut Pixmap,
    state: &GrafiaCanvasState,
    theme: &GrafiaTheme,
    vp: &ViewportTransform,
) {
    if !state.smart_corners.is_empty() {
        let col = theme.edit_canvas.selected_glow_color32();
        for preview in state.compute_smart_corner_preview_contours() {
            let scr_pts = flatten_nodes_to_screen_points(&preview.nodes, preview.closed, vp);
            for pair in scr_pts.windows(2) {
                ts_stroke_line(pixmap, pair[0], pair[1], col, TUNNI_LINE_WIDTH_DP);
            }
            if preview.closed && scr_pts.len() >= 3 {
                if let (Some(&first), Some(&last)) = (scr_pts.first(), scr_pts.last()) {
                    ts_stroke_line(pixmap, last, first, col, TUNNI_LINE_WIDTH_DP);
                }
            }
        }
    }

    if state.curve_harmony.symmetry_overlay_mode != SymmetryOverlayMode::Off {
        let col = theme.edit_canvas.bcp_line_color32();
        for (c_idx, ghost_nodes) in state.compute_symmetry_ghost_contours().iter().enumerate() {
            let closed = state.is_contour_closed(c_idx);
            let scr_pts = flatten_nodes_to_screen_points(ghost_nodes, closed, vp);
            for pair in scr_pts.windows(2) {
                ts_stroke_line(pixmap, pair[0], pair[1], col, COMB_STROKE_WIDTH_DP);
            }
            if closed && scr_pts.len() >= 3 {
                if let (Some(&first), Some(&last)) = (scr_pts.first(), scr_pts.last()) {
                    ts_stroke_line(pixmap, last, first, col, COMB_STROKE_WIDTH_DP);
                }
            }
        }
    }

    if state.curve_harmony.show_curvature_comb
        && state.curve_harmony.curvature_comb_style == CurvatureCombStyle::HeatmapEnvelope
    {
        let col = theme.edit_canvas.smooth_node_color32();
        for env in state.compute_curvature_heatmap_envelopes(COMB_DEFAULT_SAMPLES, COMB_SCALE_UPM) {
            for pair in env.samples.windows(2) {
                let avg_i = 0.5 * (pair[0].normalized_intensity + pair[1].normalized_intensity);
                let fill = curvature_heatmap_color32(avg_i, pair[0].polarity_sign, theme.is_dark());
                let quad = [
                    vp.to_screen(pair[0].base_point),
                    vp.to_screen(pair[1].base_point),
                    vp.to_screen(pair[1].tip_point),
                    vp.to_screen(pair[0].tip_point),
                ];
                fill_heatmap_quad_ts(pixmap, quad, fill);
            }
            if env.closed_polygon.len() >= 3 {
                let scr_pts: Vec<_> = env
                    .closed_polygon
                    .iter()
                    .map(|p| vp.to_screen(*p))
                    .collect();
                for pair in scr_pts.windows(2) {
                    ts_stroke_line(pixmap, pair[0], pair[1], col, TUNNI_LINE_WIDTH_DP);
                }
                if let (Some(&first), Some(&last)) = (scr_pts.first(), scr_pts.last()) {
                    ts_stroke_line(pixmap, last, first, col, TUNNI_LINE_WIDTH_DP);
                }
            }
        }
    }
    render_centroid_and_stem_gauge_tiny_skia(pixmap, state, theme, vp);
    if let Some(snap) = state.curve_harmony.active_tangent_snap {
        render_tangent_snap_tiny_skia(pixmap, state, theme, vp, snap);
    }
    crate::render::preview_bands::render_curved_trajectories_tiny_skia(pixmap, state, theme, vp);
}
