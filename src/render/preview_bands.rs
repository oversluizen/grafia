//! `SPEC-013` Pass-1 and Pass-2 overlays for Subtle Metric Overshoot Ribbons,
//! Scanline Optical Inter-Shape Rhythm Fill, and Measurement Rainbow Heatmap
//! across both `egui::Painter` and `tiny_skia::Pixmap`.

use crate::adapters::state::GrafiaCanvasState;
use crate::core::math_consts::{
    COMB_STROKE_WIDTH_DP, CURVED_TRAJ_ARC_SAMPLES, CURVED_TRAJ_PREVIEW_T,
    OPTICAL_DEFAULT_MAX_CAVITY_UPM, OPTICAL_DEFAULT_SAMPLE_COUNT, TUNNI_KNOB_RADIUS_DP,
};
use crate::core::{
    compute_measurement_rainbow_quads, compute_pair_optical_whitespace,
    evaluate_curved_master_point, FontPoint, OpticalPairConfig, ScreenPoint, ViewportTransform,
};
use crate::render::guide_and_zone_overlays::alignment_zone_screen_quad;
use crate::render::smart_corner_overlays::flatten_nodes_to_screen_points;
use crate::render::ts_primitives::{ts_fill_circle, ts_stroke_line};
use crate::tokens::{
    color32_to_tiny_skia, measurement_rainbow_color32, optical_rhythm_fill_color32,
    overshoot_ribbon_fill_color32, GrafiaTheme,
};
use egui::epaint::Vertex;
use egui::{Color32, Mesh, Painter, Pos2, Shape, Stroke};
use tiny_skia::{FillRule, Paint, PathBuilder, Pixmap, Transform};

fn paint_quad_mesh_egui(painter: &Painter, q: [ScreenPoint; 4], fill: Color32) {
    let mut mesh = Mesh::default();
    for p in q {
        mesh.vertices.push(Vertex {
            pos: Pos2::new(p.x.0, p.y.0),
            uv: Pos2::ZERO,
            color: fill,
        });
    }
    mesh.indices.extend_from_slice(&[0, 1, 2, 0, 2, 3]);
    painter.add(Shape::mesh(mesh));
}

fn fill_quad_tiny_skia(pixmap: &mut Pixmap, q: [ScreenPoint; 4], fill: Color32) {
    let mut pb = PathBuilder::new();
    pb.move_to(q[0].x.0, q[0].y.0);
    pb.line_to(q[1].x.0, q[1].y.0);
    pb.line_to(q[2].x.0, q[2].y.0);
    pb.line_to(q[3].x.0, q[3].y.0);
    pb.close();
    if let Some(path) = pb.finish() {
        let mut paint = Paint::default();
        paint.set_color(color32_to_tiny_skia(fill));
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

/// Renders Pass-1 subtle horizontal metric overshoot ribbons (`round_overshoot_upm`) into `egui::Painter` (`SPEC-013 §2.4.2`).
pub fn paint_overshoot_ribbons_egui(painter: &Painter, state: &GrafiaCanvasState) {
    let Some(ov) = state
        .curve_harmony
        .round_overshoot_upm
        .filter(|v| v.is_finite() && *v > 0.0)
    else {
        return;
    };
    let fill = overshoot_ribbon_fill_color32(state.theme.is_dark());
    for (pos_upm, size_upm) in [
        (state.metrics.cap_height, ov),
        (state.metrics.x_height, ov),
        (0.0, -ov),
    ] {
        let q = alignment_zone_screen_quad(state, pos_upm, size_upm);
        paint_quad_mesh_egui(painter, q, fill);
    }
}

/// Renders Pass-1 subtle horizontal metric overshoot ribbons (`round_overshoot_upm`) into `tiny_skia::Pixmap` (`SPEC-013 §2.4.2`).
pub fn render_overshoot_ribbons_tiny_skia(pixmap: &mut Pixmap, state: &GrafiaCanvasState) {
    let Some(ov) = state
        .curve_harmony
        .round_overshoot_upm
        .filter(|v| v.is_finite() && *v > 0.0)
    else {
        return;
    };
    let fill = overshoot_ribbon_fill_color32(state.theme.is_dark());
    for (pos_upm, size_upm) in [
        (state.metrics.cap_height, ov),
        (state.metrics.x_height, ov),
        (0.0, -ov),
    ] {
        let q = alignment_zone_screen_quad(state, pos_upm, size_upm);
        fill_quad_tiny_skia(pixmap, q, fill);
    }
}

fn collect_optical_rhythm_screen_quads(state: &GrafiaCanvasState) -> Vec<[ScreenPoint; 4]> {
    if !state.curve_harmony.show_optical_rhythm_fill || state.text_stream.slots.len() < 2 {
        return Vec::new();
    }
    let cfg = OpticalPairConfig {
        y_min_upm: state.metrics.baseline,
        y_max_upm: state.metrics.cap_height,
        sample_count: OPTICAL_DEFAULT_SAMPLE_COUNT,
        max_cavity_depth_upm: OPTICAL_DEFAULT_MAX_CAVITY_UPM,
        italic_angle_deg: state.italic_angle_deg,
    };
    let mut quads = Vec::new();
    for pair in state.text_stream.slots.windows(2) {
        let s0 = &pair[0];
        let s1 = &pair[1];
        let rel_offset = s1.origin_x_upm - s0.origin_x_upm;
        let metrics = compute_pair_optical_whitespace(
            &s0.glyph.contours,
            s0.glyph.width,
            &s1.glyph.contours,
            rel_offset,
            &cfg,
        );
        for span_pair in metrics.scanline_spans.windows(2) {
            let (y0, xl0, xr0) = span_pair[0];
            let (y1, xl1, xr1) = span_pair[1];
            if xr0 <= xl0 || xr1 <= xl1 {
                continue;
            }
            let ox = s0.origin_x_upm;
            let oy = s0.origin_y_upm;
            quads.push([
                state.viewport.to_screen(FontPoint::new(ox + xl0, oy + y0)),
                state.viewport.to_screen(FontPoint::new(ox + xr0, oy + y0)),
                state.viewport.to_screen(FontPoint::new(ox + xr1, oy + y1)),
                state.viewport.to_screen(FontPoint::new(ox + xl1, oy + y1)),
            ]);
        }
    }
    quads
}

/// Renders Pass-1 optical rhythm water-volume polygons between adjacent `GlyphSlot`s into `egui::Painter` (`SPEC-013 §2.5.1`).
pub fn paint_optical_rhythm_fill_egui(painter: &Painter, state: &GrafiaCanvasState) {
    let quads = collect_optical_rhythm_screen_quads(state);
    if quads.is_empty() {
        return;
    }
    let fill = optical_rhythm_fill_color32(state.theme.is_dark());
    for q in quads {
        paint_quad_mesh_egui(painter, q, fill);
    }
}

/// Renders Pass-1 optical rhythm water-volume polygons between adjacent `GlyphSlot`s into `tiny_skia::Pixmap` (`SPEC-013 §2.5.1`).
pub fn render_optical_rhythm_fill_tiny_skia(pixmap: &mut Pixmap, state: &GrafiaCanvasState) {
    let quads = collect_optical_rhythm_screen_quads(state);
    if quads.is_empty() {
        return;
    }
    let fill = optical_rhythm_fill_color32(state.theme.is_dark());
    let mut pb = PathBuilder::new();
    for q in quads {
        pb.move_to(q[0].x.0, q[0].y.0);
        pb.line_to(q[1].x.0, q[1].y.0);
        pb.line_to(q[2].x.0, q[2].y.0);
        pb.line_to(q[3].x.0, q[3].y.0);
        pb.close();
    }
    if let Some(path) = pb.finish() {
        let mut paint = Paint::default();
        paint.set_color(color32_to_tiny_skia(fill));
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

/// Renders Pass-2 interior Measurement Rainbow heatmap quads into `egui::Painter` (`SPEC-013 §2.1.3`).
pub fn paint_measurement_rainbow_egui(painter: &Painter, state: &GrafiaCanvasState) {
    if !state.curve_harmony.show_measurement_rainbow {
        return;
    }
    let vp = state.active_slot_viewport();
    let is_dark = state.theme.is_dark();
    for (quad, ratio) in compute_measurement_rainbow_quads(&state.glyph.contours) {
        let scr_q = [
            vp.to_screen(quad[0]),
            vp.to_screen(quad[1]),
            vp.to_screen(quad[2]),
            vp.to_screen(quad[3]),
        ];
        paint_quad_mesh_egui(painter, scr_q, measurement_rainbow_color32(ratio, is_dark));
    }
}

/// Renders Pass-2 interior Measurement Rainbow heatmap quads into `tiny_skia::Pixmap` (`SPEC-013 §2.1.3`).
pub fn render_measurement_rainbow_tiny_skia(pixmap: &mut Pixmap, state: &GrafiaCanvasState) {
    if !state.curve_harmony.show_measurement_rainbow {
        return;
    }
    let vp = state.active_slot_viewport();
    let is_dark = state.theme.is_dark();
    for (quad, ratio) in compute_measurement_rainbow_quads(&state.glyph.contours) {
        let scr_q = [
            vp.to_screen(quad[0]),
            vp.to_screen(quad[1]),
            vp.to_screen(quad[2]),
            vp.to_screen(quad[3]),
        ];
        fill_quad_tiny_skia(pixmap, scr_q, measurement_rainbow_color32(ratio, is_dark));
    }
}

/// Renders Pass-4 `CurvedTrajectoryLink` arcs and control handles into `egui::Painter` (`SPEC-013 §2.2.2`).
pub fn paint_curved_trajectories_egui(
    painter: &Painter,
    state: &GrafiaCanvasState,
    theme: &GrafiaTheme,
    vp: &ViewportTransform,
) {
    if state.curve_harmony.curved_trajectories.is_empty() {
        return;
    }
    let stroke = Stroke::new(COMB_STROKE_WIDTH_DP, theme.master_compat_ray_color32());
    let knob_col = theme.edit_canvas.smooth_node_color32();
    for link in &state.curve_harmony.curved_trajectories {
        let Some((pa, pb)) = resolve_trajectory_endpoints(state, link) else {
            continue;
        };
        let n_steps = CURVED_TRAJ_ARC_SAMPLES.max(1);
        for k in 0..n_steps {
            let t0 = (k as f64) / (n_steps as f64);
            let t1 = ((k + 1) as f64) / (n_steps as f64);
            let p0 = vp.to_screen(evaluate_curved_master_point(pa, pb, Some(link), t0));
            let p1 = vp.to_screen(evaluate_curved_master_point(pa, pb, Some(link), t1));
            painter.line_segment(
                [Pos2::new(p0.x.0, p0.y.0), Pos2::new(p1.x.0, p1.y.0)],
                stroke,
            );
        }
        let pm = vp.to_screen(evaluate_curved_master_point(
            pa,
            pb,
            Some(link),
            CURVED_TRAJ_PREVIEW_T,
        ));
        painter.circle_filled(Pos2::new(pm.x.0, pm.y.0), TUNNI_KNOB_RADIUS_DP, knob_col);
        let (sa, sb, sc1) = (
            vp.to_screen(pa),
            vp.to_screen(pb),
            vp.to_screen(link.control1),
        );
        painter.line_segment(
            [Pos2::new(sa.x.0, sa.y.0), Pos2::new(sc1.x.0, sc1.y.0)],
            stroke,
        );
        painter.circle_filled(Pos2::new(sc1.x.0, sc1.y.0), TUNNI_KNOB_RADIUS_DP, knob_col);
        if let Some(c2) = link.control2 {
            let sc2 = vp.to_screen(c2);
            painter.line_segment(
                [Pos2::new(sc2.x.0, sc2.y.0), Pos2::new(sb.x.0, sb.y.0)],
                stroke,
            );
            painter.circle_filled(Pos2::new(sc2.x.0, sc2.y.0), TUNNI_KNOB_RADIUS_DP, knob_col);
        } else {
            painter.line_segment(
                [Pos2::new(sc1.x.0, sc1.y.0), Pos2::new(sb.x.0, sb.y.0)],
                stroke,
            );
        }
    }
    paint_curved_trajectory_ghost_contours_egui(painter, state, vp, stroke);
}

fn resolve_trajectory_endpoints(
    state: &GrafiaCanvasState,
    link: &crate::core::CurvedTrajectoryLink,
) -> Option<(FontPoint, FontPoint)> {
    let na = state.find_node(link.node_id)?;
    let comp = state
        .companion_master_layers
        .get(link.companion_layer_index)?;
    let nb = comp
        .contours
        .iter()
        .flatten()
        .find(|n| n.id == link.node_id)?;
    Some((na.point, nb.point))
}

fn paint_curved_trajectory_ghost_contours_egui(
    painter: &Painter,
    state: &GrafiaCanvasState,
    vp: &ViewportTransform,
    stroke: Stroke,
) {
    if !(state.show_master_compatibility
        || state.active_tool == crate::adapters::state::CanvasTool::CurvedTrajectory)
    {
        return;
    }
    for comp_idx in 0..state.companion_master_layers.len() {
        for c_idx in 0..state.contours.len() {
            if let Some(ghost_nodes) =
                state.evaluate_curved_master_contour(c_idx, comp_idx, CURVED_TRAJ_PREVIEW_T)
            {
                let closed = state.is_contour_closed(c_idx);
                let scr_pts = flatten_nodes_to_screen_points(&ghost_nodes, closed, vp);
                if scr_pts.len() >= 2 {
                    let pts: Vec<Pos2> = scr_pts.iter().map(|s| Pos2::new(s.x.0, s.y.0)).collect();
                    if closed {
                        painter.add(Shape::closed_line(pts, stroke));
                    } else {
                        painter.add(Shape::line(pts, stroke));
                    }
                }
            }
        }
    }
}

/// Renders Pass-4 `CurvedTrajectoryLink` arcs and control handles into `tiny_skia::Pixmap` (`SPEC-013 §2.2.2`).
pub fn render_curved_trajectories_tiny_skia(
    pixmap: &mut Pixmap,
    state: &GrafiaCanvasState,
    theme: &GrafiaTheme,
    vp: &ViewportTransform,
) {
    if state.curve_harmony.curved_trajectories.is_empty() {
        return;
    }
    let col = theme.master_compat_ray_color32();
    let knob_col = theme.edit_canvas.smooth_node_color32();
    for link in &state.curve_harmony.curved_trajectories {
        let Some((pa, pb)) = resolve_trajectory_endpoints(state, link) else {
            continue;
        };
        let n_steps = CURVED_TRAJ_ARC_SAMPLES.max(1);
        for k in 0..n_steps {
            let t0 = (k as f64) / (n_steps as f64);
            let t1 = ((k + 1) as f64) / (n_steps as f64);
            let p0 = vp.to_screen(evaluate_curved_master_point(pa, pb, Some(link), t0));
            let p1 = vp.to_screen(evaluate_curved_master_point(pa, pb, Some(link), t1));
            ts_stroke_line(pixmap, p0, p1, col, COMB_STROKE_WIDTH_DP);
        }
        let pm = vp.to_screen(evaluate_curved_master_point(
            pa,
            pb,
            Some(link),
            CURVED_TRAJ_PREVIEW_T,
        ));
        ts_fill_circle(pixmap, pm, TUNNI_KNOB_RADIUS_DP, knob_col);
        let (sa, sb, sc1) = (
            vp.to_screen(pa),
            vp.to_screen(pb),
            vp.to_screen(link.control1),
        );
        ts_stroke_line(pixmap, sa, sc1, col, COMB_STROKE_WIDTH_DP);
        ts_fill_circle(pixmap, sc1, TUNNI_KNOB_RADIUS_DP, knob_col);
        if let Some(c2) = link.control2 {
            let sc2 = vp.to_screen(c2);
            ts_stroke_line(pixmap, sc2, sb, col, COMB_STROKE_WIDTH_DP);
            ts_fill_circle(pixmap, sc2, TUNNI_KNOB_RADIUS_DP, knob_col);
        } else {
            ts_stroke_line(pixmap, sc1, sb, col, COMB_STROKE_WIDTH_DP);
        }
    }
    render_curved_trajectory_ghost_contours_ts(pixmap, state, vp, col);
}

fn render_curved_trajectory_ghost_contours_ts(
    pixmap: &mut Pixmap,
    state: &GrafiaCanvasState,
    vp: &ViewportTransform,
    col: egui::Color32,
) {
    if !(state.show_master_compatibility
        || state.active_tool == crate::adapters::state::CanvasTool::CurvedTrajectory)
    {
        return;
    }
    for comp_idx in 0..state.companion_master_layers.len() {
        for c_idx in 0..state.contours.len() {
            if let Some(ghost_nodes) =
                state.evaluate_curved_master_contour(c_idx, comp_idx, CURVED_TRAJ_PREVIEW_T)
            {
                let closed = state.is_contour_closed(c_idx);
                let pts = flatten_nodes_to_screen_points(&ghost_nodes, closed, vp);
                for pair in pts.windows(2) {
                    ts_stroke_line(pixmap, pair[0], pair[1], col, COMB_STROKE_WIDTH_DP);
                }
                if closed && pts.len() >= 3 {
                    if let (Some(&first), Some(&last)) = (pts.first(), pts.last()) {
                        ts_stroke_line(pixmap, last, first, col, COMB_STROKE_WIDTH_DP);
                    }
                }
            }
        }
    }
}
