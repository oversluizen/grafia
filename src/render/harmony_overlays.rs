//! `SPEC-011` Pass-4 overlay rendering for Tangent Balance (Tunni) Lines,
//! $G^2 / G^3$ Continuity Node Badges, and Live 120 fps `CurvatureComb` (`Krommingskam`)
//! across both `egui::Painter` and `tiny_skia::Pixmap`.

use crate::adapters::state::GrafiaCanvasState;
use crate::core::math_consts::{
    COMB_STROKE_WIDTH_DP, GENIUS_BADGE_RADIUS_DP, INFLECTION_CROSS_HALF_DP, PERCENT_SCALE_FACTOR,
    TUNNI_DASH_LEN_DP, TUNNI_GAP_LEN_DP, TUNNI_KNOB_RADIUS_DP, TUNNI_LINE_WIDTH_DP,
    TUNNI_PILL_OFFSET_DP,
};
use crate::core::ScreenPoint;
use crate::render::stem_gauge_and_audit_overlays::{diamond_badge_points, star_badge_points};
use crate::render::ts_primitives::{
    fill_pill_ts, paint_dashed_line_egui, paint_pill_egui, ts_fill_circle, ts_stroke_circle,
    ts_stroke_dashed_line, ts_stroke_line,
};
use crate::tokens::GrafiaTheme;
use egui::{Painter, Pos2, Stroke};
use tiny_skia::Pixmap;

/// Renders `SPEC-011` Pass-4 overlays (Tunni balance lines, $G^2 / G^3$ badges, and Curvature Comb) into `egui::Painter`.
pub fn paint_harmony_overlays_egui(
    painter: &Painter,
    state: &GrafiaCanvasState,
    theme: &GrafiaTheme,
) {
    let vp = state.active_slot_viewport();
    if state.show_tunni_lines {
        paint_tunni_overlays_egui(painter, state, theme, &vp);
    }

    if !state.curve_harmony.genius_nodes.is_empty() || !state.curve_harmony.g3_nodes.is_empty() {
        let g2_stroke = Stroke::new(
            TUNNI_LINE_WIDTH_DP,
            theme.edit_canvas.selected_glow_color32(),
        );
        let g3_stroke = Stroke::new(TUNNI_LINE_WIDTH_DP, theme.edit_canvas.smooth_node_color32());
        for contour in &state.contours {
            for node in contour {
                if state.curve_harmony.g3_nodes.contains(&node.id) {
                    let s = vp.to_screen(node.point);
                    let pts = diamond_badge_points(s).to_vec();
                    painter.add(egui::Shape::closed_line(pts, g3_stroke));
                } else if state.curve_harmony.genius_nodes.contains(&node.id) {
                    let s = vp.to_screen(node.point);
                    painter.add(egui::Shape::closed_line(star_badge_points(s), g2_stroke));
                }
            }
        }
    }

    if state.curve_harmony.show_curvature_comb {
        paint_curvature_comb_egui(painter, state, theme, &vp);
    }

    if state.show_master_compatibility {
        paint_full_master_trajectories_egui(painter, state, theme, &vp);
    }
    crate::render::smart_corner_overlays::paint_smart_corner_and_symmetry_overlays_egui(
        painter, state, theme, &vp,
    );
}

fn paint_tunni_overlays_egui(
    painter: &Painter,
    state: &GrafiaCanvasState,
    theme: &GrafiaTheme,
    vp: &crate::core::ViewportTransform,
) {
    let line_color = theme.edit_canvas.selected_glow_color32();
    let knob_color = theme.edit_canvas.smooth_node_color32();
    let stroke = Stroke::new(TUNNI_LINE_WIDTH_DP, line_color);
    let dash_stroke = Stroke::new(COMB_STROKE_WIDTH_DP, line_color);
    for ov in state.compute_tunni_overlays() {
        let (p1_s, p2_s, m_s, apex_s) = (
            vp.to_screen(ov.p1),
            vp.to_screen(ov.p2),
            vp.to_screen(ov.tunni_point),
            vp.to_screen(ov.apex),
        );
        for end in [p1_s, p2_s] {
            paint_dashed_line_egui(
                painter,
                end,
                apex_s,
                dash_stroke,
                TUNNI_DASH_LEN_DP,
                TUNNI_GAP_LEN_DP,
            );
        }
        painter.line_segment(
            [Pos2::new(p1_s.x.0, p1_s.y.0), Pos2::new(p2_s.x.0, p2_s.y.0)],
            stroke,
        );
        painter.circle_filled(
            Pos2::new(m_s.x.0, m_s.y.0),
            TUNNI_KNOB_RADIUS_DP,
            knob_color,
        );
        painter.circle_stroke(
            Pos2::new(m_s.x.0, m_s.y.0),
            TUNNI_KNOB_RADIUS_DP + COMB_STROKE_WIDTH_DP,
            stroke,
        );
        for (pt, r) in [(p1_s, ov.ratio_p1), (p2_s, ov.ratio_p2)] {
            let lbl = format!("{:.0}%", r * PERCENT_SCALE_FACTOR);
            paint_pill_egui(
                painter,
                Pos2::new(
                    pt.x.0 + TUNNI_PILL_OFFSET_DP,
                    pt.y.0 - TUNNI_PILL_OFFSET_DP * 0.5,
                ),
                &lbl,
            );
        }
    }
}

fn paint_full_master_trajectories_egui(
    painter: &Painter,
    state: &GrafiaCanvasState,
    theme: &GrafiaTheme,
    vp: &crate::core::ViewportTransform,
) {
    let report = state.compute_master_kinks_and_crossings();
    let traj_stroke = Stroke::new(COMB_STROKE_WIDTH_DP, theme.master_compat_ray_color32());
    let warn_stroke = Stroke::new(
        TUNNI_LINE_WIDTH_DP,
        theme.master_incompat_highlight_color32(),
    );
    if state.curve_harmony.master_compatibility_mode
        == crate::core::MasterCompatibilityMode::FullNodeTrajectories
    {
        for (pa, pb) in &report.node_trajectories {
            let sa = vp.to_screen(*pa);
            let sb = vp.to_screen(*pb);
            painter.line_segment(
                [Pos2::new(sa.x.0, sa.y.0), Pos2::new(sb.x.0, sb.y.0)],
                traj_stroke,
            );
        }
    }
    for cross in &report.crossings {
        let s = vp.to_screen(cross.intersection);
        for (dx, dy) in [
            (INFLECTION_CROSS_HALF_DP, INFLECTION_CROSS_HALF_DP),
            (INFLECTION_CROSS_HALF_DP, -INFLECTION_CROSS_HALF_DP),
        ] {
            painter.line_segment(
                [
                    Pos2::new(s.x.0 - dx, s.y.0 - dy),
                    Pos2::new(s.x.0 + dx, s.y.0 + dy),
                ],
                warn_stroke,
            );
        }
    }
    for kink in &report.kink_warnings {
        if let Some(node) = state.find_node(kink.node_id) {
            let s = vp.to_screen(node.point);
            painter.circle_stroke(Pos2::new(s.x.0, s.y.0), GENIUS_BADGE_RADIUS_DP, warn_stroke);
        }
    }
}

fn paint_curvature_comb_egui(
    painter: &Painter,
    state: &GrafiaCanvasState,
    theme: &GrafiaTheme,
    vp: &crate::core::ViewportTransform,
) {
    let tooth_stroke = Stroke::new(COMB_STROKE_WIDTH_DP, theme.edit_canvas.bcp_line_color32());
    let env_stroke = Stroke::new(
        COMB_STROKE_WIDTH_DP,
        theme.edit_canvas.smooth_node_color32(),
    );
    let inf_stroke = Stroke::new(TUNNI_LINE_WIDTH_DP, theme.edit_canvas.corner_node_color32());
    for comb in state.compute_curvature_comb_overlay() {
        for tooth in &comb.teeth {
            let b = vp.to_screen(tooth.base);
            let t = vp.to_screen(tooth.tip);
            painter.line_segment(
                [Pos2::new(b.x.0, b.y.0), Pos2::new(t.x.0, t.y.0)],
                tooth_stroke,
            );
        }
        for pair in comb.teeth.windows(2) {
            let t0 = vp.to_screen(pair[0].tip);
            let t1 = vp.to_screen(pair[1].tip);
            painter.line_segment(
                [Pos2::new(t0.x.0, t0.y.0), Pos2::new(t1.x.0, t1.y.0)],
                env_stroke,
            );
        }
        for inf in &comb.inflections {
            let s = vp.to_screen(*inf);
            for (dx, dy) in [
                (INFLECTION_CROSS_HALF_DP, INFLECTION_CROSS_HALF_DP),
                (INFLECTION_CROSS_HALF_DP, -INFLECTION_CROSS_HALF_DP),
            ] {
                painter.line_segment(
                    [
                        Pos2::new(s.x.0 - dx, s.y.0 - dy),
                        Pos2::new(s.x.0 + dx, s.y.0 + dy),
                    ],
                    inf_stroke,
                );
            }
        }
        for dip in &comb.dkappa_dip_points {
            let s = vp.to_screen(*dip);
            painter.circle_stroke(Pos2::new(s.x.0, s.y.0), TUNNI_KNOB_RADIUS_DP, inf_stroke);
        }
    }
}

/// Renders `SPEC-011` Pass-4 overlays (Tunni balance lines, $G^2 / G^3$ badges, and Curvature Comb) into `tiny_skia::Pixmap`.
pub fn render_harmony_overlays_tiny_skia(
    pixmap: &mut Pixmap,
    state: &GrafiaCanvasState,
    theme: &GrafiaTheme,
) {
    let vp = state.active_slot_viewport();
    if state.show_tunni_lines {
        render_tunni_overlays_tiny_skia(pixmap, state, theme, &vp);
    }

    if !state.curve_harmony.genius_nodes.is_empty() || !state.curve_harmony.g3_nodes.is_empty() {
        let g2_col = theme.edit_canvas.selected_glow_color32();
        let g3_col = theme.edit_canvas.smooth_node_color32();
        for contour in &state.contours {
            for node in contour {
                if state.curve_harmony.g3_nodes.contains(&node.id) {
                    let pts = diamond_badge_points(vp.to_screen(node.point));
                    for idx in 0..pts.len() {
                        let p0 = pts[idx];
                        let p1 = pts[(idx + 1) % pts.len()];
                        ts_stroke_line(
                            pixmap,
                            ScreenPoint::new(p0.x, p0.y),
                            ScreenPoint::new(p1.x, p1.y),
                            g3_col,
                            TUNNI_LINE_WIDTH_DP,
                        );
                    }
                } else if state.curve_harmony.genius_nodes.contains(&node.id) {
                    let pts = star_badge_points(vp.to_screen(node.point));
                    for idx in 0..pts.len() {
                        let p0 = pts[idx];
                        let p1 = pts[(idx + 1) % pts.len()];
                        ts_stroke_line(
                            pixmap,
                            ScreenPoint::new(p0.x, p0.y),
                            ScreenPoint::new(p1.x, p1.y),
                            g2_col,
                            TUNNI_LINE_WIDTH_DP,
                        );
                    }
                }
            }
        }
    }

    if state.curve_harmony.show_curvature_comb {
        render_curvature_comb_tiny_skia(pixmap, state, theme, &vp);
    }

    if state.show_master_compatibility {
        render_full_master_trajectories_tiny_skia(pixmap, state, theme, &vp);
    }
    crate::render::smart_corner_overlays::render_smart_corner_and_symmetry_overlays_tiny_skia(
        pixmap, state, theme, &vp,
    );
}

fn render_tunni_overlays_tiny_skia(
    pixmap: &mut Pixmap,
    state: &GrafiaCanvasState,
    theme: &GrafiaTheme,
    vp: &crate::core::ViewportTransform,
) {
    let line_color = theme.edit_canvas.selected_glow_color32();
    let knob_color = theme.edit_canvas.smooth_node_color32();
    for ov in state.compute_tunni_overlays() {
        let p1_s = vp.to_screen(ov.p1);
        let p2_s = vp.to_screen(ov.p2);
        let m_s = vp.to_screen(ov.tunni_point);
        let apex_s = vp.to_screen(ov.apex);
        for end in [p1_s, p2_s] {
            ts_stroke_dashed_line(
                pixmap,
                end,
                apex_s,
                line_color,
                COMB_STROKE_WIDTH_DP,
                TUNNI_DASH_LEN_DP,
                TUNNI_GAP_LEN_DP,
            );
        }
        ts_stroke_line(pixmap, p1_s, p2_s, line_color, TUNNI_LINE_WIDTH_DP);
        ts_fill_circle(pixmap, m_s, TUNNI_KNOB_RADIUS_DP, knob_color);
        ts_stroke_circle(
            pixmap,
            m_s,
            TUNNI_KNOB_RADIUS_DP + COMB_STROKE_WIDTH_DP,
            line_color,
            TUNNI_LINE_WIDTH_DP,
        );
        for (pt, r) in [(p1_s, ov.ratio_p1), (p2_s, ov.ratio_p2)] {
            let lbl = format!("{:.0}%", r * PERCENT_SCALE_FACTOR);
            fill_pill_ts(
                pixmap,
                pt.x.0 + TUNNI_PILL_OFFSET_DP,
                pt.y.0 - TUNNI_PILL_OFFSET_DP * 0.5,
                &lbl,
            );
        }
    }
}

fn render_full_master_trajectories_tiny_skia(
    pixmap: &mut Pixmap,
    state: &GrafiaCanvasState,
    theme: &GrafiaTheme,
    vp: &crate::core::ViewportTransform,
) {
    let report = state.compute_master_kinks_and_crossings();
    let traj_col = theme.master_compat_ray_color32();
    let warn_col = theme.master_incompat_highlight_color32();
    if state.curve_harmony.master_compatibility_mode
        == crate::core::MasterCompatibilityMode::FullNodeTrajectories
    {
        for (pa, pb) in &report.node_trajectories {
            let sa = vp.to_screen(*pa);
            let sb = vp.to_screen(*pb);
            ts_stroke_line(pixmap, sa, sb, traj_col, COMB_STROKE_WIDTH_DP);
        }
    }
    for cross in &report.crossings {
        let s = vp.to_screen(cross.intersection);
        for (dx, dy) in [
            (INFLECTION_CROSS_HALF_DP, INFLECTION_CROSS_HALF_DP),
            (INFLECTION_CROSS_HALF_DP, -INFLECTION_CROSS_HALF_DP),
        ] {
            ts_stroke_line(
                pixmap,
                ScreenPoint::new(s.x.0 - dx, s.y.0 - dy),
                ScreenPoint::new(s.x.0 + dx, s.y.0 + dy),
                warn_col,
                TUNNI_LINE_WIDTH_DP,
            );
        }
    }
    for kink in &report.kink_warnings {
        if let Some(node) = state.find_node(kink.node_id) {
            let s = vp.to_screen(node.point);
            ts_stroke_circle(
                pixmap,
                s,
                GENIUS_BADGE_RADIUS_DP,
                warn_col,
                TUNNI_LINE_WIDTH_DP,
            );
        }
    }
}

fn render_curvature_comb_tiny_skia(
    pixmap: &mut Pixmap,
    state: &GrafiaCanvasState,
    theme: &GrafiaTheme,
    vp: &crate::core::ViewportTransform,
) {
    let tooth_col = theme.edit_canvas.bcp_line_color32();
    let env_col = theme.edit_canvas.smooth_node_color32();
    let inf_col = theme.edit_canvas.corner_node_color32();
    for comb in state.compute_curvature_comb_overlay() {
        for tooth in &comb.teeth {
            let b = vp.to_screen(tooth.base);
            let t = vp.to_screen(tooth.tip);
            ts_stroke_line(pixmap, b, t, tooth_col, COMB_STROKE_WIDTH_DP);
        }
        for pair in comb.teeth.windows(2) {
            let t0 = vp.to_screen(pair[0].tip);
            let t1 = vp.to_screen(pair[1].tip);
            ts_stroke_line(pixmap, t0, t1, env_col, COMB_STROKE_WIDTH_DP);
        }
        for inf in &comb.inflections {
            let s = vp.to_screen(*inf);
            for (dx, dy) in [
                (INFLECTION_CROSS_HALF_DP, INFLECTION_CROSS_HALF_DP),
                (INFLECTION_CROSS_HALF_DP, -INFLECTION_CROSS_HALF_DP),
            ] {
                ts_stroke_line(
                    pixmap,
                    ScreenPoint::new(s.x.0 - dx, s.y.0 - dy),
                    ScreenPoint::new(s.x.0 + dx, s.y.0 + dy),
                    inf_col,
                    TUNNI_LINE_WIDTH_DP,
                );
            }
        }
        for dip in &comb.dkappa_dip_points {
            let s = vp.to_screen(*dip);
            ts_stroke_circle(
                pixmap,
                s,
                TUNNI_KNOB_RADIUS_DP,
                inf_col,
                TUNNI_LINE_WIDTH_DP,
            );
        }
    }
}
