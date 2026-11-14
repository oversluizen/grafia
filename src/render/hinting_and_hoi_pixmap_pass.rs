//! Pass 1 and Pass 4 `tiny-skia` pixmap rendering for Pixel Preview, Backdrop Ghost Contours,
//! PostScript Stem/Ghost Hints, TrueType Visual Instructor, and HOI Control Polygons (`SPEC-010 §2.4..§2.5`).

use crate::adapters::headless::kurbo_to_tiny_skia_path;
use crate::adapters::state::{CanvasTool, CanvasViewMode, GrafiaCanvasState};
use crate::core::hinting_and_hoi::{
    compute_hoi_overlay_geometry, compute_pixel_preview_cells_upm, find_contour_node_upm,
    grid_fit_contour_nodes, HoiDiamondRole, PostScriptHint, PostScriptHintAxis,
};
use crate::core::math_consts::{
    HOI_DASH_LEN_DP, HOI_DIAMOND_CURR_RADIUS_DP, HOI_DIAMOND_NEXT_RADIUS_DP,
    HOI_DIAMOND_PREV_RADIUS_DP, HOI_GAP_LEN_DP, PS_HINT_PILL_HEIGHT_DP, PS_HINT_PILL_MARGIN_DP,
    PS_HINT_PILL_WIDTH_DP, TT_ARROW_HEAD_HALF_W_DP, TT_ARROW_HEAD_LEN_DP, TT_BADGE_RADIUS_DP,
    TT_BADGE_SELECTED_RADIUS_DP,
};
use crate::core::{FontPoint, ScreenPoint};
use crate::render::ts_primitives::{
    fill_and_stroke_poly_ts, stroke_ts, ts_fill_circle, ts_fill_rect, ts_stroke_circle,
    ts_stroke_dashed_line, ts_stroke_line, ts_stroke_rect,
};
use crate::tokens::trace_and_band_tokens::{
    hoi_next_diamond_color32, hoi_prev_diamond_color32, pixel_preview_cell_fill_color32,
    pixel_preview_grid_line_color32, tt_grid_fit_outline_color32, tt_instruction_color32,
};
use crate::tokens::{color32_to_tiny_skia, GrafiaTheme, CURVE_FLATTEN_TOLERANCE};
use kurbo::BezPath;
use tiny_skia::{FillRule, Paint, PathBuilder, Pixmap, Transform};

pub fn paint_pixel_preview_and_ghost_pixmap_pass(
    pixmap: &mut Pixmap,
    state: &GrafiaCanvasState,
    theme: &GrafiaTheme,
) {
    if state.view_mode != CanvasViewMode::EditWireframe {
        return;
    }
    let vp = state.active_slot_viewport();
    if !state.backdrop_ghost_contours.is_empty() {
        let ghost_col = theme.backdrop_ghost_fill.to_color32();
        let affine =
            kurbo::Affine::translate((f64::from(vp.pan_offset.x.0), f64::from(vp.pan_offset.y.0)))
                * kurbo::Affine::scale_non_uniform(f64::from(vp.zoom), -f64::from(vp.zoom));
        let mut compound = BezPath::new();
        for c in &state.backdrop_ghost_contours {
            if !c.closed || c.nodes.len() < 2 {
                continue;
            }
            let upm_path =
                crate::core::contour_to_bezpath_with_splines(&c.nodes, c.closed, 0.0, &[]);
            let scr_path = affine * upm_path;
            for el in scr_path.elements() {
                compound.push(*el);
            }
        }
        if let Some(ts_fill) = kurbo_to_tiny_skia_path(&compound) {
            let mut paint = Paint::default();
            paint.set_color(color32_to_tiny_skia(ghost_col));
            paint.anti_alias = true;
            pixmap.fill_path(
                &ts_fill,
                &paint,
                FillRule::Winding,
                Transform::identity(),
                None,
            );
        }
    }

    let active_ppem = state.show_pixel_preview_ppem.or_else(|| {
        (state.active_tool == CanvasTool::TrueTypeInstructor)
            .then_some(state.truetype_instructor_ppem)
    });
    let Some(ppem) = active_ppem else {
        return;
    };
    let is_dark = theme.is_dark();
    let cell_fill = pixel_preview_cell_fill_color32(is_dark);
    let cell_line = pixel_preview_grid_line_color32(is_dark);
    let sw = theme.edit_canvas.guide_stroke_width_dp;
    for (x0_upm, y0_upm, x1_upm, y1_upm) in
        compute_pixel_preview_cells_upm(&state.glyph.contours, state.glyph.upm, ppem)
    {
        let tl = vp.to_screen(FontPoint::new(x0_upm, y1_upm));
        let br = vp.to_screen(FontPoint::new(x1_upm, y0_upm));
        ts_fill_rect(pixmap, tl.x.0, tl.y.0, br.x.0, br.y.0, cell_fill);
        ts_stroke_rect(pixmap, tl.x.0, tl.y.0, br.x.0, br.y.0, cell_line, sw);
    }
}

pub fn paint_hinting_and_hoi_pixmap_pass(
    pixmap: &mut Pixmap,
    state: &GrafiaCanvasState,
    theme: &GrafiaTheme,
) {
    if state.view_mode != CanvasViewMode::EditWireframe {
        return;
    }
    let vp = state.active_slot_viewport();
    for hint in &state.postscript_hints {
        paint_single_postscript_hint_ts(pixmap, hint, theme, &vp);
    }
    if state.active_tool == CanvasTool::TrueTypeInstructor {
        paint_truetype_grid_fitted_outlines_ts(pixmap, state, theme, &vp);
    }
    if !state.truetype_instructions.is_empty() {
        paint_truetype_instruction_badges_ts(pixmap, state, theme, &vp);
    }
    if state.show_hoi_diamonds || state.active_tool == CanvasTool::Hoi {
        paint_hoi_overlays_ts(pixmap, state, theme, &vp);
    }
}

fn paint_single_postscript_hint_ts(
    pixmap: &mut Pixmap,
    hint: &PostScriptHint,
    theme: &GrafiaTheme,
    vp: &crate::core::ViewportTransform,
) {
    let (lo_upm, hi_upm) = hint.span_upm();
    let fill_col = if hint.is_ghost {
        theme.ps_hint_ghost_fill.to_color32()
    } else {
        theme.ps_hint_stem_fill.to_color32()
    };
    let border_col = if hint.selected {
        theme.edit_canvas.selected_glow_color32()
    } else {
        fill_col
    };
    let sw = if hint.selected {
        theme.edit_canvas.contour_stroke_width_dp
    } else {
        theme.edit_canvas.guide_stroke_width_dp
    };
    let (w_scr, h_scr) = (pixmap.width() as f32, pixmap.height() as f32);

    let (bx, by, l0, l1, px, py) = if hint.axis == PostScriptHintAxis::Horizontal {
        let y_a = vp.to_screen(FontPoint::new(0.0, hi_upm)).y.0;
        let y_b = vp.to_screen(FontPoint::new(0.0, lo_upm)).y.0;
        let (y0, y1) = (y_a.min(y_b), y_a.max(y_b));
        let px0 = PS_HINT_PILL_MARGIN_DP;
        let pcy = (y0 + y1) * 0.5;
        (
            (0.0, w_scr),
            (y0, y1),
            (ScreenPoint::new(0.0, y0), ScreenPoint::new(w_scr, y0)),
            (ScreenPoint::new(0.0, y1), ScreenPoint::new(w_scr, y1)),
            (px0, px0 + PS_HINT_PILL_WIDTH_DP),
            (
                pcy - PS_HINT_PILL_HEIGHT_DP * 0.5,
                pcy + PS_HINT_PILL_HEIGHT_DP * 0.5,
            ),
        )
    } else {
        let x_a = vp.to_screen(FontPoint::new(lo_upm, 0.0)).x.0;
        let x_b = vp.to_screen(FontPoint::new(hi_upm, 0.0)).x.0;
        let (x0, x1) = (x_a.min(x_b), x_a.max(x_b));
        let pcx = (x0 + x1) * 0.5;
        let py0 = PS_HINT_PILL_MARGIN_DP;
        (
            (x0, x1),
            (0.0, h_scr),
            (ScreenPoint::new(x0, 0.0), ScreenPoint::new(x0, h_scr)),
            (ScreenPoint::new(x1, 0.0), ScreenPoint::new(x1, h_scr)),
            (
                pcx - PS_HINT_PILL_WIDTH_DP * 0.5,
                pcx + PS_HINT_PILL_WIDTH_DP * 0.5,
            ),
            (py0, py0 + PS_HINT_PILL_HEIGHT_DP),
        )
    };
    ts_fill_rect(pixmap, bx.0, by.0, bx.1, by.1, fill_col);
    ts_stroke_line(pixmap, l0.0, l0.1, border_col, sw);
    ts_stroke_line(pixmap, l1.0, l1.1, border_col, sw);
    ts_fill_rect(pixmap, px.0, py.0, px.1, py.1, fill_col);
    ts_stroke_rect(pixmap, px.0, py.0, px.1, py.1, border_col, sw);
}

fn paint_truetype_grid_fitted_outlines_ts(
    pixmap: &mut Pixmap,
    state: &GrafiaCanvasState,
    theme: &GrafiaTheme,
    vp: &crate::core::ViewportTransform,
) {
    let fit_col = color32_to_tiny_skia(tt_grid_fit_outline_color32(theme.is_dark()));
    let fit_w = theme.edit_canvas.guide_stroke_width_dp;
    for gc in &state.glyph.contours {
        if gc.nodes.len() < 2 {
            continue;
        }
        let fitted = grid_fit_contour_nodes(
            &gc.nodes,
            state.glyph.upm,
            state.truetype_instructor_ppem,
            state.drag_transients.truetype_x_axis_projection,
            &state.truetype_instructions,
        );
        let bez = crate::core::contour_to_bezpath_with_splines(&fitted, gc.closed, 0.0, &[]);
        let mut pts = Vec::new();
        kurbo::flatten(bez.iter(), CURVE_FLATTEN_TOLERANCE, |el| match el {
            kurbo::PathEl::MoveTo(p) | kurbo::PathEl::LineTo(p) => {
                pts.push(vp.to_screen(FontPoint::new(p.x, p.y)));
            }
            _ => {}
        });
        if pts.len() >= 2 {
            let mut pb = PathBuilder::new();
            pb.move_to(pts[0].x.0, pts[0].y.0);
            for pt in &pts[1..] {
                pb.line_to(pt.x.0, pt.y.0);
            }
            if gc.closed {
                pb.close();
            }
            stroke_ts(pixmap, pb, fit_col, fit_w);
        }
    }
}

fn paint_truetype_instruction_badges_ts(
    pixmap: &mut Pixmap,
    state: &GrafiaCanvasState,
    theme: &GrafiaTheme,
    vp: &crate::core::ViewportTransform,
) {
    let base_col = tt_instruction_color32(theme.is_dark());
    let sel_col = theme.edit_canvas.selected_glow_color32();
    let ring_col = theme.canvas_background_color32();
    let sw = theme.edit_canvas.contour_stroke_width_dp;
    let ring_w = theme.edit_canvas.guide_stroke_width_dp;

    for ins in &state.truetype_instructions {
        let Some(t_upm) = find_contour_node_upm(&state.contours, ins.target_node) else {
            continue;
        };
        let t_scr = vp.to_screen(t_upm);
        let col = if ins.selected { sel_col } else { base_col };

        if let Some(s_id) = ins.source_node {
            if let Some(s_upm) = find_contour_node_upm(&state.contours, s_id) {
                let s_scr = vp.to_screen(s_upm);
                ts_stroke_line(pixmap, s_scr, t_scr, col, sw);
                let (dx, dy) = (t_scr.x.0 - s_scr.x.0, t_scr.y.0 - s_scr.y.0);
                let len = dx.hypot(dy);
                if len > TT_ARROW_HEAD_LEN_DP {
                    let (ux, uy) = (dx / len, dy / len);
                    let (nx, ny) = (-uy, ux);
                    let (bx, by) = (
                        t_scr.x.0 - ux * TT_ARROW_HEAD_LEN_DP,
                        t_scr.y.0 - uy * TT_ARROW_HEAD_LEN_DP,
                    );
                    for sign in [-1.0_f32, 1.0_f32] {
                        let wing = ScreenPoint::new(
                            bx + nx * TT_ARROW_HEAD_HALF_W_DP * sign,
                            by + ny * TT_ARROW_HEAD_HALF_W_DP * sign,
                        );
                        ts_stroke_line(pixmap, t_scr, wing, col, sw);
                    }
                }
            }
        }
        let r = if ins.selected {
            TT_BADGE_SELECTED_RADIUS_DP
        } else {
            TT_BADGE_RADIUS_DP
        };
        ts_fill_circle(pixmap, t_scr, r, col);
        ts_stroke_circle(pixmap, t_scr, r, ring_col, ring_w);
    }
}

fn paint_hoi_overlays_ts(
    pixmap: &mut Pixmap,
    state: &GrafiaCanvasState,
    theme: &GrafiaTheme,
    vp: &crate::core::ViewportTransform,
) {
    let geom = compute_hoi_overlay_geometry(
        &state.companion_master_layers,
        state.drag_transients.hoi_sibling_contours.as_ref(),
    );
    let is_dark = theme.is_dark();
    let active_col = theme.hoi_diamond_color.to_color32();
    let prev_col = hoi_prev_diamond_color32(is_dark);
    let next_col = hoi_next_diamond_color32(is_dark);
    let sw = theme.edit_canvas.guide_stroke_width_dp;

    for poly in &geom.polylines {
        for pair in poly.windows(2) {
            let sa = vp.to_screen(pair[0]);
            let sb = vp.to_screen(pair[1]);
            ts_stroke_dashed_line(
                pixmap,
                sa,
                sb,
                active_col,
                sw,
                HOI_DASH_LEN_DP,
                HOI_GAP_LEN_DP,
            );
        }
    }

    let outline_col = color32_to_tiny_skia(theme.canvas_background_color32());
    for (pt_upm, role) in &geom.diamonds {
        let scr = vp.to_screen(*pt_upm);
        let (r, col) = match role {
            HoiDiamondRole::PrevLayer => (HOI_DIAMOND_PREV_RADIUS_DP, prev_col),
            HoiDiamondRole::ActiveLayer => (HOI_DIAMOND_CURR_RADIUS_DP, active_col),
            HoiDiamondRole::NextLayer => (HOI_DIAMOND_NEXT_RADIUS_DP, next_col),
        };
        let quad = [
            ScreenPoint::new(scr.x.0, scr.y.0 - r),
            ScreenPoint::new(scr.x.0 + r, scr.y.0),
            ScreenPoint::new(scr.x.0, scr.y.0 + r),
            ScreenPoint::new(scr.x.0 - r, scr.y.0),
        ];
        fill_and_stroke_poly_ts(pixmap, &quad, color32_to_tiny_skia(col), outline_col, sw);
    }
}
