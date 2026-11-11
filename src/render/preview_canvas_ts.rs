//! Offscreen `tiny-skia` rasterizer for the bottom glyph preview panel.

use crate::adapters::headless::kurbo_to_tiny_skia_path;
use crate::adapters::state::GrafiaCanvasState;
use crate::core::coordinates::{ScreenPoint, ViewportTransform};
use crate::render::preview_canvas::{PreviewFilledProjection, PreviewPanelConfig};
use crate::tokens::{
    color32_to_tiny_skia, preview_panel_baseline_color, preview_panel_bg_inverted,
    preview_panel_bg_normal, preview_panel_fill_inverted, preview_panel_fill_normal,
    DEFAULT_ASCENDER_UPM, DEFAULT_DESCENDER_UPM, DEFAULT_X_HEIGHT_UPM,
    PREVIEW_PANEL_AUTOFIT_MARGIN, PREVIEW_PANEL_BASELINE_STROKE_WIDTH_DP,
};
use kurbo::BezPath;
use tiny_skia::{
    FillRule, Paint, PathBuilder, Pixmap, Rect as TsRect, Stroke as TsStroke, Transform,
};

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

    let adv_w = if let Some(last) = state.text_stream.slots.last() {
        (last.origin_x_upm + last.glyph.width).max(DEFAULT_X_HEIGHT_UPM)
    } else if state.glyph.width > 0.0 {
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
            pixmap.stroke_path(
                &line_path,
                &base_paint,
                &stroke,
                Transform::identity(),
                None,
            );
        }
    }

    let temp_vp = ViewportTransform::new(
        scale as f32,
        ScreenPoint::new(origin_x_scr, normal_baseline_y),
        state.glyph.upm,
    );

    let fill_color = if use_dark {
        preview_panel_fill_inverted()
    } else {
        preview_panel_fill_normal()
    };
    if state.text_stream.slots.is_empty() {
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
            let mut fill_paint = Paint::default();
            fill_paint.set_color(color32_to_tiny_skia(fill_color));
            fill_paint.anti_alias = true;
            pixmap.fill_path(
                &ts_path,
                &fill_paint,
                FillRule::Winding,
                Transform::identity(),
                None,
            );
        }
    } else {
        let ts_col = color32_to_tiny_skia(fill_color);
        for slot in &state.text_stream.slots {
            let slot_vp = temp_vp.with_origin_x_upm(slot.origin_x_upm);
            let mut pb = PathBuilder::new();
            for gc in &slot.glyph.contours {
                if !gc.visible || !gc.closed || gc.nodes.len() < 2 {
                    continue;
                }
                let s0 = slot_vp.to_screen(gc.nodes[0].point);
                pb.move_to(
                    s0.x.0,
                    if config.flipped {
                        2.0 * center_y - s0.y.0
                    } else {
                        s0.y.0
                    },
                );
                for seg in crate::core::extract_segments(&gc.nodes, true) {
                    match seg {
                        crate::core::ContourSegment::Line { end_idx, .. } => {
                            let p = slot_vp.to_screen(gc.nodes[end_idx].point);
                            pb.line_to(
                                p.x.0,
                                if config.flipped {
                                    2.0 * center_y - p.y.0
                                } else {
                                    p.y.0
                                },
                            );
                        }
                        crate::core::ContourSegment::Cubic {
                            h1_idx,
                            h2_idx,
                            end_idx,
                            ..
                        } => {
                            let h1 = slot_vp.to_screen(gc.nodes[h1_idx].point);
                            let h2 = slot_vp.to_screen(gc.nodes[h2_idx].point);
                            let p3 = slot_vp.to_screen(gc.nodes[end_idx].point);
                            let y1 = if config.flipped {
                                2.0 * center_y - h1.y.0
                            } else {
                                h1.y.0
                            };
                            let y2 = if config.flipped {
                                2.0 * center_y - h2.y.0
                            } else {
                                h2.y.0
                            };
                            let y3 = if config.flipped {
                                2.0 * center_y - p3.y.0
                            } else {
                                p3.y.0
                            };
                            pb.cubic_to(h1.x.0, y1, h2.x.0, y2, p3.x.0, y3);
                        }
                    }
                }
                pb.close();
            }
            if let Some(path) = pb.finish() {
                let mut fill_paint = Paint::default();
                fill_paint.set_color(ts_col);
                fill_paint.anti_alias = true;
                pixmap.fill_path(
                    &path,
                    &fill_paint,
                    FillRule::Winding,
                    Transform::identity(),
                    None,
                );
            }
        }
    }
    if config.blur_radius_px > crate::tokens::PREVIEW_BLUR_ACTIVE_THRESHOLD_PX {
        let rx = (rect_x.max(0.0) as usize).min(pixmap.width() as usize);
        let ry = (rect_y.max(0.0) as usize).min(pixmap.height() as usize);
        let rw = (rect_w.max(0.0) as usize).min((pixmap.width() as usize).saturating_sub(rx));
        let rh = (rect_h.max(0.0) as usize).min((pixmap.height() as usize).saturating_sub(ry));
        if rw > 0 && rh > 0 {
            let pw = pixmap.width() as usize;
            let data = pixmap.data_mut();
            let mut sub = vec![0u8; rw * rh * crate::tokens::RGBA_CHANNEL_COUNT];
            for y in 0..rh {
                let src = ((ry + y) * pw + rx) * crate::tokens::RGBA_CHANNEL_COUNT;
                let dst = y * rw * crate::tokens::RGBA_CHANNEL_COUNT;
                let len = rw * crate::tokens::RGBA_CHANNEL_COUNT;
                sub[dst..dst + len].copy_from_slice(&data[src..src + len]);
            }
            crate::render::apply_separable_gaussian_blur_rgba(
                &mut sub,
                rw,
                rh,
                config.blur_radius_px,
            );
            for y in 0..rh {
                let dst = ((ry + y) * pw + rx) * crate::tokens::RGBA_CHANNEL_COUNT;
                let src = y * rw * crate::tokens::RGBA_CHANNEL_COUNT;
                let len = rw * crate::tokens::RGBA_CHANNEL_COUNT;
                data[dst..dst + len].copy_from_slice(&sub[src..src + len]);
            }
        }
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
    let mut pixmap =
        Pixmap::new(width, height).expect("Failed to allocate Pixmap for preview panel");
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
