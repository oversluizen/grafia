//! Pass 2 & Pass 4 `egui::Painter` rendering for `CanvasColorSublayer` (`SPEC-010 §2.1`).

use crate::adapters::state::GrafiaCanvasState;
use crate::render::canvas_render_cache::get_or_upload_color_sublayers_texture;
use crate::render::shared_geom::{gradient_overlay_radii_dp, project_gradient_handles_screen};
use crate::tokens::trace_and_band_tokens::{gradient_handle_ring_color32, rgba_u8_to_color32};
use crate::tokens::GrafiaTheme;
use egui::{Color32, Painter, Pos2, Rect, Stroke};

/// Paints visible `state.color_sublayers` in `egui` Pass 2 via the generation-keyed texture cache.
pub fn paint_color_layers_pass(painter: &Painter, state: &GrafiaCanvasState, rect: Rect) {
    if state.color_sublayers.is_empty() || !state.color_sublayers.iter().any(|s| s.visible) {
        return;
    }
    let w = (rect.width().round() as u32).max(1);
    let h = (rect.height().round() as u32).max(1);
    let tex = get_or_upload_color_sublayers_texture(painter.ctx(), state, w, h);
    let uv = Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0));
    painter.image(tex.id(), rect, uv, Color32::WHITE);
}

/// Paints interactive gradient handles in `egui` Pass 4 when a color sublayer is selected.
pub fn paint_color_layer_handles_pass(
    painter: &Painter,
    state: &GrafiaCanvasState,
    theme: &GrafiaTheme,
) {
    let Some(sub_idx) = state.drag_transients.selected_color_sublayer else {
        return;
    };
    let Some(sub) = state.color_sublayers.get(sub_idx) else {
        return;
    };
    if !sub.visible || sub.locked {
        return;
    }
    let sel_stop = state
        .drag_transients
        .selected_gradient_stop
        .and_then(|(s, idx)| (s == sub_idx).then_some(idx));
    let vp = state.active_slot_viewport();
    let Some(proj) = project_gradient_handles_screen(&sub.paint, sel_stop, &vp) else {
        return;
    };
    let ring_col = gradient_handle_ring_color32(theme.is_dark());
    let (end_r, stop_r, sel_stop_r, line_w) = gradient_overlay_radii_dp();

    if let Some((c0_scr, r0_px)) = proj.radial_r0_scr {
        if r0_px > 0.5 {
            painter.circle_stroke(
                Pos2::new(c0_scr.x.0, c0_scr.y.0),
                r0_px,
                Stroke::new(1.0, ring_col),
            );
        }
    }
    if let Some((c1_scr, r1_px)) = proj.radial_r1_scr {
        painter.circle_stroke(
            Pos2::new(c1_scr.x.0, c1_scr.y.0),
            r1_px,
            Stroke::new(1.0, ring_col),
        );
    }
    painter.line_segment(
        [
            Pos2::new(proj.start_scr.x.0, proj.start_scr.y.0),
            Pos2::new(proj.end_scr.x.0, proj.end_scr.y.0),
        ],
        Stroke::new(line_w, ring_col),
    );
    painter.circle_stroke(
        Pos2::new(proj.start_scr.x.0, proj.start_scr.y.0),
        end_r,
        Stroke::new(line_w, ring_col),
    );
    painter.circle_stroke(
        Pos2::new(proj.end_scr.x.0, proj.end_scr.y.0),
        end_r,
        Stroke::new(line_w, ring_col),
    );
    for (pt_scr, rgba, is_sel) in proj.stop_ticks {
        let r = if is_sel { sel_stop_r } else { stop_r };
        let center = Pos2::new(pt_scr.x.0, pt_scr.y.0);
        painter.circle_filled(center, r, rgba_u8_to_color32(rgba));
        painter.circle_stroke(center, r, Stroke::new(1.0, ring_col));
    }
}
