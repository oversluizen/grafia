//! Pass 1 and Pass 4 `egui::Painter` rendering for Pixel Preview, Backdrop Ghost Contours,
//! PostScript Stem/Ghost Hints, TrueType Visual Instructor, and HOI Control Polygons (`SPEC-010 §2.4..§2.5`).

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
use crate::render::tessellate_glyph_contours_nonzero;
use crate::render::ts_primitives::paint_dashed_line_egui;
use crate::tokens::trace_and_band_tokens::{
    hoi_next_diamond_color32, hoi_prev_diamond_color32, pixel_preview_cell_fill_color32,
    pixel_preview_grid_line_color32, tt_grid_fit_outline_color32, tt_instruction_color32,
};
use crate::tokens::{GrafiaTheme, CURVE_FLATTEN_TOLERANCE};
use egui::{Painter, Pos2, Rect, Shape, Stroke, StrokeKind};

pub fn paint_pixel_preview_and_ghost_pass(
    painter: &Painter,
    state: &GrafiaCanvasState,
    theme: &GrafiaTheme,
) {
    if state.view_mode != CanvasViewMode::EditWireframe {
        return;
    }
    let vp = state.active_slot_viewport();
    if !state.backdrop_ghost_contours.is_empty() {
        let ghost_col = theme.backdrop_ghost_fill.to_color32();
        let mesh =
            tessellate_glyph_contours_nonzero(&state.backdrop_ghost_contours, &vp, ghost_col);
        if !mesh.is_empty() {
            painter.add(Shape::mesh(mesh));
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
    let cell_stroke = Stroke::new(
        theme.edit_canvas.guide_stroke_width_dp,
        pixel_preview_grid_line_color32(is_dark),
    );
    for (x0_upm, y0_upm, x1_upm, y1_upm) in
        compute_pixel_preview_cells_upm(&state.glyph.contours, state.glyph.upm, ppem)
    {
        let tl = vp.to_screen(FontPoint::new(x0_upm, y1_upm));
        let br = vp.to_screen(FontPoint::new(x1_upm, y0_upm));
        let r = Rect::from_min_max(
            Pos2::new(tl.x.0.min(br.x.0), tl.y.0.min(br.y.0)),
            Pos2::new(tl.x.0.max(br.x.0), tl.y.0.max(br.y.0)),
        );
        painter.rect_filled(r, 0.0, cell_fill);
        painter.rect_stroke(r, 0.0, cell_stroke, StrokeKind::Middle);
    }
}

pub fn paint_hinting_and_hoi_pass(
    painter: &Painter,
    state: &GrafiaCanvasState,
    theme: &GrafiaTheme,
    canvas_rect: Rect,
) {
    if state.view_mode != CanvasViewMode::EditWireframe {
        return;
    }
    let vp = state.active_slot_viewport();
    for hint in &state.postscript_hints {
        paint_single_postscript_hint_egui(painter, hint, theme, canvas_rect, &vp);
    }
    if state.active_tool == CanvasTool::TrueTypeInstructor {
        paint_truetype_grid_fitted_outlines_egui(painter, state, theme, &vp);
    }
    if !state.truetype_instructions.is_empty() {
        paint_truetype_instruction_badges_egui(painter, state, theme, &vp);
    }
    if state.show_hoi_diamonds || state.active_tool == CanvasTool::Hoi {
        paint_hoi_overlays_egui(painter, state, theme, &vp);
    }
}

fn paint_single_postscript_hint_egui(
    painter: &Painter,
    hint: &PostScriptHint,
    theme: &GrafiaTheme,
    canvas_rect: Rect,
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
    let stroke = Stroke::new(sw, border_col);

    let (band_rect, l0, l1, pill_rect) = if hint.axis == PostScriptHintAxis::Horizontal {
        let y_a = vp.to_screen(FontPoint::new(0.0, hi_upm)).y.0;
        let y_b = vp.to_screen(FontPoint::new(0.0, lo_upm)).y.0;
        let (y0, y1) = (y_a.min(y_b), y_a.max(y_b));
        let (x0, x1) = (canvas_rect.left(), canvas_rect.right());
        let px0 = x0 + PS_HINT_PILL_MARGIN_DP;
        let pcy = (y0 + y1) * 0.5;
        (
            Rect::from_min_max(Pos2::new(x0, y0), Pos2::new(x1, y1)),
            [Pos2::new(x0, y0), Pos2::new(x1, y0)],
            [Pos2::new(x0, y1), Pos2::new(x1, y1)],
            Rect::from_min_max(
                Pos2::new(px0, pcy - PS_HINT_PILL_HEIGHT_DP * 0.5),
                Pos2::new(
                    px0 + PS_HINT_PILL_WIDTH_DP,
                    pcy + PS_HINT_PILL_HEIGHT_DP * 0.5,
                ),
            ),
        )
    } else {
        let x_a = vp.to_screen(FontPoint::new(lo_upm, 0.0)).x.0;
        let x_b = vp.to_screen(FontPoint::new(hi_upm, 0.0)).x.0;
        let (x0, x1) = (x_a.min(x_b), x_a.max(x_b));
        let (y0, y1) = (canvas_rect.top(), canvas_rect.bottom());
        let pcx = (x0 + x1) * 0.5;
        let py0 = y0 + PS_HINT_PILL_MARGIN_DP;
        (
            Rect::from_min_max(Pos2::new(x0, y0), Pos2::new(x1, y1)),
            [Pos2::new(x0, y0), Pos2::new(x0, y1)],
            [Pos2::new(x1, y0), Pos2::new(x1, y1)],
            Rect::from_min_max(
                Pos2::new(pcx - PS_HINT_PILL_WIDTH_DP * 0.5, py0),
                Pos2::new(
                    pcx + PS_HINT_PILL_WIDTH_DP * 0.5,
                    py0 + PS_HINT_PILL_HEIGHT_DP,
                ),
            ),
        )
    };
    painter.rect_filled(band_rect, 0.0, fill_col);
    painter.line_segment(l0, stroke);
    painter.line_segment(l1, stroke);
    painter.rect_filled(pill_rect, 0.0, fill_col);
    painter.rect_stroke(pill_rect, 0.0, stroke, StrokeKind::Middle);
}

fn paint_truetype_grid_fitted_outlines_egui(
    painter: &Painter,
    state: &GrafiaCanvasState,
    theme: &GrafiaTheme,
    vp: &crate::core::ViewportTransform,
) {
    let fit_stroke = Stroke::new(
        theme.edit_canvas.guide_stroke_width_dp,
        tt_grid_fit_outline_color32(theme.is_dark()),
    );
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
                let s = vp.to_screen(FontPoint::new(p.x, p.y));
                pts.push(Pos2::new(s.x.0, s.y.0));
            }
            _ => {}
        });
        if pts.len() >= 2 {
            painter.add(if gc.closed {
                Shape::closed_line(pts, fit_stroke)
            } else {
                Shape::line(pts, fit_stroke)
            });
        }
    }
}

fn paint_truetype_instruction_badges_egui(
    painter: &Painter,
    state: &GrafiaCanvasState,
    theme: &GrafiaTheme,
    vp: &crate::core::ViewportTransform,
) {
    let base_col = tt_instruction_color32(theme.is_dark());
    let sel_col = theme.edit_canvas.selected_glow_color32();
    let ring_col = theme.canvas_background_color32();
    let ring_stroke = Stroke::new(theme.edit_canvas.guide_stroke_width_dp, ring_col);
    for ins in &state.truetype_instructions {
        let Some(t_upm) = find_contour_node_upm(&state.contours, ins.target_node) else {
            continue;
        };
        let t_scr = vp.to_screen(t_upm);
        let t_pos = Pos2::new(t_scr.x.0, t_scr.y.0);
        let col = if ins.selected { sel_col } else { base_col };
        let line_stroke = Stroke::new(theme.edit_canvas.contour_stroke_width_dp, col);

        if let Some(s_id) = ins.source_node {
            if let Some(s_upm) = find_contour_node_upm(&state.contours, s_id) {
                let s_scr = vp.to_screen(s_upm);
                let s_pos = Pos2::new(s_scr.x.0, s_scr.y.0);
                painter.line_segment([s_pos, t_pos], line_stroke);
                let (dx, dy) = (t_pos.x - s_pos.x, t_pos.y - s_pos.y);
                let len = dx.hypot(dy);
                if len > TT_ARROW_HEAD_LEN_DP {
                    let (ux, uy) = (dx / len, dy / len);
                    let (nx, ny) = (-uy, ux);
                    let (bx, by) = (
                        t_pos.x - ux * TT_ARROW_HEAD_LEN_DP,
                        t_pos.y - uy * TT_ARROW_HEAD_LEN_DP,
                    );
                    for sign in [-1.0_f32, 1.0_f32] {
                        let wing = Pos2::new(
                            bx + nx * TT_ARROW_HEAD_HALF_W_DP * sign,
                            by + ny * TT_ARROW_HEAD_HALF_W_DP * sign,
                        );
                        painter.line_segment([t_pos, wing], line_stroke);
                    }
                }
            }
        }
        let r = if ins.selected {
            TT_BADGE_SELECTED_RADIUS_DP
        } else {
            TT_BADGE_RADIUS_DP
        };
        painter.circle_filled(t_pos, r, col);
        painter.circle_stroke(t_pos, r, ring_stroke);
    }
}

fn paint_hoi_overlays_egui(
    painter: &Painter,
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
    let net_stroke = Stroke::new(theme.edit_canvas.guide_stroke_width_dp, active_col);

    for poly in &geom.polylines {
        for pair in poly.windows(2) {
            let sa = vp.to_screen(pair[0]);
            let sb = vp.to_screen(pair[1]);
            paint_dashed_line_egui(painter, sa, sb, net_stroke, HOI_DASH_LEN_DP, HOI_GAP_LEN_DP);
        }
    }

    let outline_stroke = Stroke::new(
        theme.edit_canvas.guide_stroke_width_dp,
        theme.canvas_background_color32(),
    );
    for (pt_upm, role) in &geom.diamonds {
        let scr: ScreenPoint = vp.to_screen(*pt_upm);
        let (r, col) = match role {
            HoiDiamondRole::PrevLayer => (HOI_DIAMOND_PREV_RADIUS_DP, prev_col),
            HoiDiamondRole::ActiveLayer => (HOI_DIAMOND_CURR_RADIUS_DP, active_col),
            HoiDiamondRole::NextLayer => (HOI_DIAMOND_NEXT_RADIUS_DP, next_col),
        };
        let quad = vec![
            Pos2::new(scr.x.0, scr.y.0 - r),
            Pos2::new(scr.x.0 + r, scr.y.0),
            Pos2::new(scr.x.0, scr.y.0 + r),
            Pos2::new(scr.x.0 - r, scr.y.0),
        ];
        painter.add(Shape::convex_polygon(quad.clone(), col, Stroke::NONE));
        painter.add(Shape::closed_line(quad, outline_stroke));
    }
}
