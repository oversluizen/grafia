//! Preview canvas filled silhouette projection and live glyph preview widget.

use crate::adapters::headless::kurbo_to_tiny_skia_path;
use crate::adapters::state::GrafiaCanvasState;
use crate::core::coordinates::{ScreenPoint, ViewportTransform};
use crate::tokens::{
    color32_to_tiny_skia, preview_panel_baseline_color, preview_panel_bg_inverted,
    preview_panel_bg_normal, preview_panel_border_color, preview_panel_fill_inverted,
    preview_panel_fill_normal, DEFAULT_ASCENDER_UPM, DEFAULT_DESCENDER_UPM, DEFAULT_X_HEIGHT_UPM,
    PREVIEW_PANEL_AUTOFIT_MARGIN, PREVIEW_PANEL_BASELINE_STROKE_WIDTH_DP,
    PREVIEW_PANEL_DEFAULT_HEIGHT_DP, PREVIEW_PANEL_MIN_HEIGHT_DP, PREVIEW_PANEL_PADDING_DP,
};
use egui::{vec2, Pos2, Response, Sense, Shape, Stroke, Ui, Widget};
use kurbo::BezPath;
use tiny_skia::{FillRule, Paint, PathBuilder, Pixmap, Rect as TsRect, Stroke as TsStroke, Transform};

#[derive(Debug, Clone, PartialEq)]
pub struct PreviewFilledProjection {
    pub screen_path: BezPath,
    pub baseline_y_screen: f32,
}

impl PreviewFilledProjection {
    #[must_use]
    pub fn compute(font_path: &BezPath, vp: &ViewportTransform) -> Self {
        let affine = kurbo::Affine::translate((vp.pan_offset.x.0 as f64, vp.pan_offset.y.0 as f64))
            * kurbo::Affine::scale_non_uniform(vp.zoom as f64, -(vp.zoom as f64));
        let screen_path = affine * font_path;

        Self {
            screen_path,
            baseline_y_screen: vp.pan_offset.y.0,
        }
    }
}

/// Configuration for the live glyph preview panel.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PreviewPanelConfig {
    pub inverted: bool,
    pub flipped: bool,
    pub show_baseline: bool,
    pub padding_dp: f32,
    pub height_dp: f32,
}

impl Default for PreviewPanelConfig {
    fn default() -> Self {
        Self {
            inverted: false,
            flipped: false,
            show_baseline: true,
            padding_dp: PREVIEW_PANEL_PADDING_DP,
            height_dp: PREVIEW_PANEL_DEFAULT_HEIGHT_DP,
        }
    }
}

/// An immediate-mode egui widget rendering a live, Non-Zero Winding filled glyph preview
/// for the bottom preview panel.
pub struct GrafiaPreviewWidget<'a> {
    state: &'a GrafiaCanvasState,
    config: PreviewPanelConfig,
}

pub type GlifoPreviewWidget<'a> = GrafiaPreviewWidget<'a>;

impl<'a> GrafiaPreviewWidget<'a> {
    #[must_use]
    pub fn new(state: &'a GrafiaCanvasState) -> Self {
        Self {
            state,
            config: PreviewPanelConfig::default(),
        }
    }

    #[must_use]
    pub fn with_config(mut self, config: PreviewPanelConfig) -> Self {
        self.config = config;
        self
    }

    pub fn show(self, ui: &mut Ui) -> Response {
        let avail_w = ui.available_width().max(PREVIEW_PANEL_MIN_HEIGHT_DP);
        let size = vec2(avail_w, self.config.height_dp);
        let (rect, response) = ui.allocate_exact_size(size, Sense::hover());

        if ui.is_rect_visible(rect) {
            let use_dark = self.state.theme.is_dark() ^ self.config.inverted;
            let bg_color = if use_dark {
                preview_panel_bg_inverted()
            } else {
                preview_panel_bg_normal()
            };
            ui.painter().rect_filled(rect, 0.0, bg_color);

            let border_stroke = Stroke::new(PREVIEW_PANEL_BASELINE_STROKE_WIDTH_DP, preview_panel_border_color());
            ui.painter().line_segment([rect.left_top(), rect.right_top()], border_stroke);

            let asc = DEFAULT_ASCENDER_UPM;
            let desc = DEFAULT_DESCENDER_UPM;
            let v_span = (asc - desc).max(1.0);
            let pad = self.config.padding_dp as f64;
            let avail_h = (rect.height() as f64 - pad * 2.0).max(1.0);
            let scale = (avail_h / v_span) * PREVIEW_PANEL_AUTOFIT_MARGIN;

            let center_y = rect.center().y;
            let normal_baseline_y = center_y + ((asc + desc) * 0.5 * scale) as f32;
            let draw_baseline_y = if self.config.flipped {
                2.0 * center_y - normal_baseline_y
            } else {
                normal_baseline_y
            };

            let adv_w = if self.state.glyph.width > 0.0 {
                self.state.glyph.width
            } else {
                DEFAULT_X_HEIGHT_UPM
            };
            let origin_x_scr = rect.center().x - (adv_w * 0.5 * scale) as f32;

            if self.config.show_baseline {
                let baseline_stroke = Stroke::new(
                    PREVIEW_PANEL_BASELINE_STROKE_WIDTH_DP,
                    preview_panel_baseline_color(),
                );
                ui.painter().line_segment(
                    [
                        Pos2::new(rect.left(), draw_baseline_y),
                        Pos2::new(rect.right(), draw_baseline_y),
                    ],
                    baseline_stroke,
                );
            }

            let temp_vp = ViewportTransform::new(
                scale as f32,
                ScreenPoint::new(origin_x_scr, normal_baseline_y),
                self.state.glyph.upm,
            );

            let fill_color = if use_dark {
                preview_panel_fill_inverted()
            } else {
                preview_panel_fill_normal()
            };

            let closed_flags = self.state.closed_flags();
            let mut mesh = crate::render::tessellate_closed_contours_nonzero(
                &self.state.contours,
                &closed_flags,
                &temp_vp,
                fill_color,
            );
            if self.config.flipped {
                for v in &mut mesh.vertices {
                    v.pos.y = 2.0 * center_y - v.pos.y;
                }
            }
            if !mesh.is_empty() {
                ui.painter().add(Shape::mesh(mesh));
            }
        }

        response
    }
}

impl Widget for GrafiaPreviewWidget<'_> {
    fn ui(self, ui: &mut Ui) -> Response {
        self.show(ui)
    }
}

/// Renders the bottom preview panel into a sub-rectangle of a `tiny_skia::Pixmap`.
pub fn render_preview_panel_into_rect(
    pixmap: &mut Pixmap,
    state: &GrafiaCanvasState,
    config: &PreviewPanelConfig,
    rect_x: f32,
    rect_y: f32,
    rect_w: f32,
    rect_h: f32,
) {
    let use_dark = state.theme.is_dark() ^ config.inverted;
    let bg_color = if use_dark {
        preview_panel_bg_inverted()
    } else {
        preview_panel_bg_normal()
    };
    if let Some(ts_rect) = TsRect::from_xywh(rect_x, rect_y, rect_w, rect_h) {
        let mut bg_paint = Paint::default();
        bg_paint.set_color(color32_to_tiny_skia(bg_color));
        pixmap.fill_rect(ts_rect, &bg_paint, Transform::identity(), None);
    }

    let asc = DEFAULT_ASCENDER_UPM;
    let desc = DEFAULT_DESCENDER_UPM;
    let v_span = (asc - desc).max(1.0);
    let pad = config.padding_dp as f64;
    let avail_h = (rect_h as f64 - pad * 2.0).max(1.0);
    let scale = (avail_h / v_span) * PREVIEW_PANEL_AUTOFIT_MARGIN;

    let center_y = rect_y + rect_h * 0.5;
    let normal_baseline_y = center_y + ((asc + desc) * 0.5 * scale) as f32;
    let draw_baseline_y = if config.flipped {
        2.0 * center_y - normal_baseline_y
    } else {
        normal_baseline_y
    };

    let adv_w = if state.glyph.width > 0.0 {
        state.glyph.width
    } else {
        DEFAULT_X_HEIGHT_UPM
    };
    let origin_x_scr = rect_x + rect_w * 0.5 - (adv_w * 0.5 * scale) as f32;

    if config.show_baseline {
        let mut pb = PathBuilder::new();
        pb.move_to(rect_x, draw_baseline_y);
        pb.line_to(rect_x + rect_w, draw_baseline_y);
        if let Some(line_path) = pb.finish() {
            let mut base_paint = Paint::default();
            base_paint.set_color(color32_to_tiny_skia(preview_panel_baseline_color()));
            let stroke = TsStroke {
                width: PREVIEW_PANEL_BASELINE_STROKE_WIDTH_DP,
                ..Default::default()
            };
            pixmap.stroke_path(&line_path, &base_paint, &stroke, Transform::identity(), None);
        }
    }

    let temp_vp = ViewportTransform::new(
        scale as f32,
        ScreenPoint::new(origin_x_scr, normal_baseline_y),
        state.glyph.upm,
    );

    let mut compound = BezPath::new();
    let flip_affine = kurbo::Affine::translate((0.0, 2.0 * center_y as f64))
        * kurbo::Affine::scale_non_uniform(1.0, -1.0);
    for (c_idx, path) in state.paths.iter().enumerate() {
        if !state.is_contour_closed(c_idx) {
            continue;
        }
        let proj = PreviewFilledProjection::compute(path, &temp_vp);
        let final_path = if config.flipped {
            flip_affine * proj.screen_path
        } else {
            proj.screen_path
        };
        for el in final_path.elements() {
            compound.push(*el);
        }
    }

    if let Some(ts_path) = kurbo_to_tiny_skia_path(&compound) {
        let fill_color = if use_dark {
            preview_panel_fill_inverted()
        } else {
            preview_panel_fill_normal()
        };
        let mut fill_paint = Paint::default();
        fill_paint.set_color(color32_to_tiny_skia(fill_color));
        fill_paint.anti_alias = true;
        pixmap.fill_path(&ts_path, &fill_paint, FillRule::Winding, Transform::identity(), None);
    }
}

/// Offscreen `tiny-skia` rasterizer for the bottom preview panel.
#[must_use]
pub fn rasterize_preview_panel(
    state: &GrafiaCanvasState,
    config: &PreviewPanelConfig,
    width: u32,
    height: u32,
) -> Pixmap {
    let mut pixmap = Pixmap::new(width, height).expect("Failed to allocate Pixmap for preview panel");
    render_preview_panel_into_rect(
        &mut pixmap,
        state,
        config,
        0.0,
        0.0,
        width as f32,
        height as f32,
    );
    pixmap
}
