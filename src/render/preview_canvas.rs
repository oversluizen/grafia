//! Preview canvas filled silhouette projection and live glyph preview widget.

pub use crate::render::preview_canvas_ts::{
    rasterize_preview_panel, render_preview_panel_into_rect,
};

use crate::adapters::state::GrafiaCanvasState;
use crate::core::coordinates::{ScreenPoint, ViewportTransform};
use crate::tokens::{
    preview_panel_baseline_color, preview_panel_bg_inverted, preview_panel_bg_normal,
    preview_panel_border_color, preview_panel_fill_inverted, preview_panel_fill_normal,
    DEFAULT_ASCENDER_UPM, DEFAULT_DESCENDER_UPM, DEFAULT_X_HEIGHT_UPM,
    PREVIEW_PANEL_AUTOFIT_MARGIN, PREVIEW_PANEL_BASELINE_STROKE_WIDTH_DP,
    PREVIEW_PANEL_DEFAULT_HEIGHT_DP, PREVIEW_PANEL_MIN_HEIGHT_DP, PREVIEW_PANEL_PADDING_DP,
};
use egui::{vec2, Pos2, Response, Sense, Shape, Stroke, Ui, Widget};
use kurbo::BezPath;

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
    pub blur_radius_px: f32,
}

impl Default for PreviewPanelConfig {
    fn default() -> Self {
        Self {
            inverted: false,
            flipped: false,
            show_baseline: true,
            padding_dp: PREVIEW_PANEL_PADDING_DP,
            height_dp: PREVIEW_PANEL_DEFAULT_HEIGHT_DP,
            blur_radius_px: 0.0,
        }
    }
}

/// An immediate-mode egui widget rendering a live, Non-Zero Winding filled glyph preview
/// for the bottom preview panel.
pub struct GrafiaPreviewWidget<'a> {
    state: &'a GrafiaCanvasState,
    config: PreviewPanelConfig,
}

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

            let border_stroke = Stroke::new(
                PREVIEW_PANEL_BASELINE_STROKE_WIDTH_DP,
                preview_panel_border_color(),
            );
            ui.painter()
                .line_segment([rect.left_top(), rect.right_top()], border_stroke);

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

            let adv_w = if let Some(last) = self.state.text_stream.slots.last() {
                (last.origin_x_upm + last.glyph.width).max(DEFAULT_X_HEIGHT_UPM)
            } else if self.state.glyph.width > 0.0 {
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

            if self.config.blur_radius_px > crate::tokens::PREVIEW_BLUR_ACTIVE_THRESHOLD_PX {
                let rw = (rect.width().round().max(1.0)) as u32;
                let rh = (rect.height().round().max(1.0)) as u32;
                let pix = rasterize_preview_panel(self.state, &self.config, rw, rh);
                let data = pix.data();
                let w_us = rw as usize;
                let h_us = rh as usize;
                for y in 0..h_us {
                    let mut x = 0usize;
                    while x < w_us {
                        let idx = (y * w_us + x) * crate::tokens::RGBA_CHANNEL_COUNT;
                        let col = crate::tokens::preview_rgba_slice_to_color32(
                            &data[idx..idx + crate::tokens::RGBA_CHANNEL_COUNT],
                        );
                        let mut end_x = x + 1;
                        while end_x < w_us {
                            let nidx = (y * w_us + end_x) * crate::tokens::RGBA_CHANNEL_COUNT;
                            if data[nidx..nidx + crate::tokens::RGBA_CHANNEL_COUNT]
                                != data[idx..idx + crate::tokens::RGBA_CHANNEL_COUNT]
                            {
                                break;
                            }
                            end_x += 1;
                        }
                        if col != bg_color {
                            let r = egui::Rect::from_min_max(
                                Pos2::new(rect.left() + x as f32, rect.top() + y as f32),
                                Pos2::new(rect.left() + end_x as f32, rect.top() + (y + 1) as f32),
                            );
                            ui.painter().rect_filled(r, 0.0, col);
                        }
                        x = end_x;
                    }
                }
                ui.painter()
                    .line_segment([rect.left_top(), rect.right_top()], border_stroke);
            } else if self.state.text_stream.slots.is_empty() {
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
            } else {
                for slot in &self.state.text_stream.slots {
                    let slot_vp = temp_vp.with_origin_x_upm(slot.origin_x_upm);
                    let mut mesh = crate::render::tessellate_glyph_contours_nonzero(
                        &slot.glyph.contours,
                        &slot_vp,
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
