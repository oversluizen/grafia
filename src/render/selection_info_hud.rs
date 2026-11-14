//! `SPEC-007` (`grafia v0.6.0`) 5-mode contextual Grey Info Box HUD renderer
//! for both `egui` and `tiny-skia`, including the interactive 9-dot 3x3 Transform-Grid
//! and proportional scale indicator in `MultiNode` mode.

use crate::adapters::state::{CanvasViewMode, GrafiaCanvasState};
use crate::core::{ScreenPoint, SelectionInfo, TransformGridAnchor};
use crate::render::ruler_and_background_overlays::draw_bitmap_digits_tiny_skia;
pub use crate::render::selection_info_hud_format::format_selection_info_hud_lines;
use crate::tokens::{
    color32_to_tiny_skia, GrafiaTheme, HUD_BOTTOM_MARGIN_DP, HUD_BOX_HEIGHT_DP, HUD_BOX_WIDTH_DP,
    HUD_CORNER_RADIUS_DP, HUD_FONT_SIZE_PRIMARY_DP, HUD_FONT_SIZE_SECONDARY_DP,
    HUD_GRID_DOT_ACTIVE_RADIUS_DP, HUD_GRID_DOT_RADIUS_DP, HUD_GRID_ORIGIN_OFFSET_X_DP,
    HUD_GRID_ORIGIN_OFFSET_Y_DP, HUD_GRID_STEP_DP, HUD_LINE1_OFFSET_Y_DP, HUD_LINE2_OFFSET_Y_DP,
    HUD_MULTI_TEXT_OFFSET_X_DP, HUD_PROP_ICON_OFFSET_X_DP, HUD_PROP_ICON_OFFSET_Y_DP,
    HUD_PROP_ICON_SIZE_DP, HUD_TEXT_PAD_X_DP,
};
use egui::{Align2, FontId, Painter, Pos2, Rect, Stroke};
use tiny_skia::{
    FillRule, Paint, PathBuilder, Pixmap, Rect as TsRect, Stroke as TsStroke, Transform,
};

/// Ordered 3x3 Transform-Grid anchors `(row, col)` in `0..=2`.
pub const TRANSFORM_GRID_ANCHORS_3X3: [(TransformGridAnchor, usize, usize); 9] = [
    (TransformGridAnchor::TopLeft, 0, 0),
    (TransformGridAnchor::TopCenter, 0, 1),
    (TransformGridAnchor::TopRight, 0, 2),
    (TransformGridAnchor::MiddleLeft, 1, 0),
    (TransformGridAnchor::Center, 1, 1),
    (TransformGridAnchor::MiddleRight, 1, 2),
    (TransformGridAnchor::BottomLeft, 2, 0),
    (TransformGridAnchor::BottomCenter, 2, 1),
    (TransformGridAnchor::BottomRight, 2, 2),
];

/// Returns `true` when the contextual Selection Info HUD should be painted on the canvas.
#[must_use]
pub fn should_render_selection_info_hud(state: &GrafiaCanvasState) -> bool {
    state.show_selection_info_hud
        && state.view_mode == CanvasViewMode::EditWireframe
        && (!state.contours.is_empty() || state.glyph.width > 0.0 || state.selected_guide.is_some())
}

/// Computes the screen-space bounding `egui::Rect` of the bottom Grey Info Box HUD.
#[must_use]
pub fn compute_hud_rect(canvas_rect: Rect) -> Rect {
    let w = HUD_BOX_WIDTH_DP.min((canvas_rect.width() - HUD_TEXT_PAD_X_DP * 2.0).max(1.0));
    let h = HUD_BOX_HEIGHT_DP;
    let min_x = canvas_rect.center().x - w * 0.5;
    let max_y = (canvas_rect.max.y - HUD_BOTTOM_MARGIN_DP).max(canvas_rect.min.y + h);
    let min_y = max_y - h;
    Rect::from_min_max(Pos2::new(min_x, min_y), Pos2::new(min_x + w, max_y))
}

/// Computes the 9 screen positions of the 3x3 Transform-Grid dots inside `hud_rect`.
#[must_use]
pub fn compute_hud_transform_grid_dot_positions(
    hud_rect: Rect,
) -> [(TransformGridAnchor, ScreenPoint); 9] {
    let base_x = hud_rect.min.x + HUD_GRID_ORIGIN_OFFSET_X_DP;
    let base_y = hud_rect.min.y + HUD_GRID_ORIGIN_OFFSET_Y_DP;
    TRANSFORM_GRID_ANCHORS_3X3.map(|(anchor, row, col)| {
        let x = base_x + (col as f32) * HUD_GRID_STEP_DP;
        let y = base_y + (row as f32) * HUD_GRID_STEP_DP;
        (anchor, ScreenPoint::new(x, y))
    })
}

/// Computes the screen-space `egui::Rect` of the proportional scale toggle icon inside `hud_rect`.
#[must_use]
pub fn compute_hud_proportional_icon_rect(hud_rect: Rect) -> Rect {
    let min = Pos2::new(
        hud_rect.min.x + HUD_PROP_ICON_OFFSET_X_DP,
        hud_rect.min.y + HUD_PROP_ICON_OFFSET_Y_DP,
    );
    Rect::from_min_size(
        min,
        egui::vec2(HUD_PROP_ICON_SIZE_DP, HUD_PROP_ICON_SIZE_DP),
    )
}

/// Paints the 5-mode contextual Grey Info Box HUD in `egui`.
pub fn paint_selection_info_hud_egui(
    painter: &Painter,
    state: &GrafiaCanvasState,
    theme: &GrafiaTheme,
    canvas_rect: Rect,
) {
    if !should_render_selection_info_hud(state) {
        return;
    }
    let info = state.selection_info();
    let hud_rect = compute_hud_rect(canvas_rect);
    painter.rect(
        hud_rect,
        HUD_CORNER_RADIUS_DP,
        theme.hud_bg_fill_color32(),
        Stroke::new(1.0, theme.hud_border_stroke_color32()),
        egui::StrokeKind::Inside,
    );

    let text_x = if let SelectionInfo::MultiNode(m) = &info {
        paint_hud_multi_node_grid_egui(painter, hud_rect, m, theme);
        hud_rect.min.x + HUD_MULTI_TEXT_OFFSET_X_DP
    } else {
        hud_rect.min.x + HUD_TEXT_PAD_X_DP
    };

    let (line1, line2) = format_selection_info_hud_lines(&info);
    painter.text(
        Pos2::new(text_x, hud_rect.min.y + HUD_LINE1_OFFSET_Y_DP),
        Align2::LEFT_TOP,
        line1,
        FontId::monospace(HUD_FONT_SIZE_PRIMARY_DP),
        theme.hud_text_primary_color32(),
    );
    painter.text(
        Pos2::new(text_x, hud_rect.min.y + HUD_LINE2_OFFSET_Y_DP),
        Align2::LEFT_TOP,
        line2,
        FontId::monospace(HUD_FONT_SIZE_SECONDARY_DP),
        theme.hud_text_secondary_color32(),
    );
}

fn paint_hud_multi_node_grid_egui(
    painter: &Painter,
    hud_rect: Rect,
    m: &crate::core::MultiNodeInfo,
    theme: &GrafiaTheme,
) {
    let dots = compute_hud_transform_grid_dot_positions(hud_rect);
    let line_stroke = Stroke::new(1.0, theme.hud_border_stroke_color32());
    for i in [0usize, 3, 6] {
        painter.line_segment(
            [
                Pos2::new(dots[i].1.x.0, dots[i].1.y.0),
                Pos2::new(dots[i + 2].1.x.0, dots[i + 2].1.y.0),
            ],
            line_stroke,
        );
    }
    for i in [0usize, 1, 2] {
        painter.line_segment(
            [
                Pos2::new(dots[i].1.x.0, dots[i].1.y.0),
                Pos2::new(dots[i + 6].1.x.0, dots[i + 6].1.y.0),
            ],
            line_stroke,
        );
    }
    for (anchor, pt) in dots {
        let is_active = anchor == m.origin_anchor;
        let r = if is_active {
            HUD_GRID_DOT_ACTIVE_RADIUS_DP
        } else {
            HUD_GRID_DOT_RADIUS_DP
        };
        let col = if is_active {
            theme.hud_accent_fill_color32()
        } else {
            theme.hud_dot_inactive_color32()
        };
        painter.circle_filled(Pos2::new(pt.x.0, pt.y.0), r, col);
    }

    let prop_rect = compute_hud_proportional_icon_rect(hud_rect);
    let prop_col = if m.proportional_scale {
        theme.hud_accent_fill_color32()
    } else {
        theme.hud_dot_inactive_color32()
    };
    painter.rect_stroke(
        prop_rect,
        2.0,
        Stroke::new(1.0, prop_col),
        egui::StrokeKind::Inside,
    );
    if m.proportional_scale {
        painter.line_segment(
            [prop_rect.left_bottom(), prop_rect.right_top()],
            Stroke::new(1.0, prop_col),
        );
    }
}

fn build_rounded_rect_path(rect: Rect, radius: f32) -> Option<tiny_skia::Path> {
    let r = radius
        .min(rect.width() * 0.5)
        .min(rect.height() * 0.5)
        .max(0.0);
    let x0 = rect.min.x;
    let y0 = rect.min.y;
    let x1 = rect.max.x;
    let y1 = rect.max.y;
    let mut pb = PathBuilder::new();
    pb.move_to(x0 + r, y0);
    pb.line_to(x1 - r, y0);
    pb.quad_to(x1, y0, x1, y0 + r);
    pb.line_to(x1, y1 - r);
    pb.quad_to(x1, y1, x1 - r, y1);
    pb.line_to(x0 + r, y1);
    pb.quad_to(x0, y1, x0, y1 - r);
    pb.line_to(x0, y0 + r);
    pb.quad_to(x0, y0, x0 + r, y0);
    pb.close();
    pb.finish()
}

/// Renders the 5-mode contextual Grey Info Box HUD in `tiny-skia`.
pub fn render_selection_info_hud_tiny_skia(
    pixmap: &mut Pixmap,
    state: &GrafiaCanvasState,
    theme: &GrafiaTheme,
    rect_x: f32,
    rect_y: f32,
    rect_w: f32,
    rect_h: f32,
) {
    if !should_render_selection_info_hud(state) {
        return;
    }
    let canvas_rect = Rect::from_min_size(Pos2::new(rect_x, rect_y), egui::vec2(rect_w, rect_h));
    let hud_rect = compute_hud_rect(canvas_rect);
    let stroke_1 = TsStroke {
        width: 1.0,
        ..Default::default()
    };

    if let Some(bg_path) = build_rounded_rect_path(hud_rect, HUD_CORNER_RADIUS_DP) {
        let mut bg_paint = Paint::default();
        bg_paint.set_color(color32_to_tiny_skia(theme.hud_bg_fill_color32()));
        bg_paint.anti_alias = true;
        pixmap.fill_path(
            &bg_path,
            &bg_paint,
            FillRule::Winding,
            Transform::identity(),
            None,
        );

        let mut border_paint = Paint::default();
        border_paint.set_color(color32_to_tiny_skia(theme.hud_border_stroke_color32()));
        border_paint.anti_alias = true;
        pixmap.stroke_path(
            &bg_path,
            &border_paint,
            &stroke_1,
            Transform::identity(),
            None,
        );
    }

    let info = state.selection_info();
    let text_x = if let SelectionInfo::MultiNode(m) = &info {
        render_hud_multi_node_grid_ts(pixmap, hud_rect, m, theme, &stroke_1);
        hud_rect.min.x + HUD_MULTI_TEXT_OFFSET_X_DP
    } else {
        hud_rect.min.x + HUD_TEXT_PAD_X_DP
    };

    let (line1, line2) = format_selection_info_hud_lines(&info);
    draw_bitmap_digits_tiny_skia(
        pixmap,
        text_x,
        hud_rect.min.y + HUD_LINE1_OFFSET_Y_DP,
        &line1,
        theme.hud_text_primary_color32(),
    );
    draw_bitmap_digits_tiny_skia(
        pixmap,
        text_x,
        hud_rect.min.y + HUD_LINE2_OFFSET_Y_DP,
        &line2,
        theme.hud_text_secondary_color32(),
    );
}

fn render_hud_multi_node_grid_ts(
    pixmap: &mut Pixmap,
    hud_rect: Rect,
    m: &crate::core::MultiNodeInfo,
    theme: &GrafiaTheme,
    stroke_1: &TsStroke,
) {
    let dots = compute_hud_transform_grid_dot_positions(hud_rect);
    let mut grid_pb = PathBuilder::new();
    for i in [0usize, 3, 6] {
        grid_pb.move_to(dots[i].1.x.0, dots[i].1.y.0);
        grid_pb.line_to(dots[i + 2].1.x.0, dots[i + 2].1.y.0);
    }
    for i in [0usize, 1, 2] {
        grid_pb.move_to(dots[i].1.x.0, dots[i].1.y.0);
        grid_pb.line_to(dots[i + 6].1.x.0, dots[i + 6].1.y.0);
    }
    if let Some(grid_path) = grid_pb.finish() {
        let mut g_paint = Paint::default();
        g_paint.set_color(color32_to_tiny_skia(theme.hud_border_stroke_color32()));
        pixmap.stroke_path(&grid_path, &g_paint, stroke_1, Transform::identity(), None);
    }

    for (anchor, pt) in dots {
        let is_active = anchor == m.origin_anchor;
        let r = if is_active {
            HUD_GRID_DOT_ACTIVE_RADIUS_DP
        } else {
            HUD_GRID_DOT_RADIUS_DP
        };
        let col = if is_active {
            theme.hud_accent_fill_color32()
        } else {
            theme.hud_dot_inactive_color32()
        };
        if let Some(circle) = PathBuilder::from_circle(pt.x.0, pt.y.0, r) {
            let mut d_paint = Paint::default();
            d_paint.set_color(color32_to_tiny_skia(col));
            d_paint.anti_alias = true;
            pixmap.fill_path(
                &circle,
                &d_paint,
                FillRule::Winding,
                Transform::identity(),
                None,
            );
        }
    }

    let prop_rect = compute_hud_proportional_icon_rect(hud_rect);
    let prop_col = if m.proportional_scale {
        theme.hud_accent_fill_color32()
    } else {
        theme.hud_dot_inactive_color32()
    };
    if let Some(ts_prop) = TsRect::from_xywh(
        prop_rect.min.x,
        prop_rect.min.y,
        prop_rect.width(),
        prop_rect.height(),
    ) {
        let mut p_paint = Paint::default();
        p_paint.set_color(color32_to_tiny_skia(prop_col));
        let mut p_pb = PathBuilder::new();
        p_pb.push_rect(ts_prop);
        if m.proportional_scale {
            p_pb.move_to(prop_rect.min.x, prop_rect.max.y);
            p_pb.line_to(prop_rect.max.x, prop_rect.min.y);
        }
        if let Some(p_path) = p_pb.finish() {
            pixmap.stroke_path(&p_path, &p_paint, stroke_1, Transform::identity(), None);
        }
    }
}
