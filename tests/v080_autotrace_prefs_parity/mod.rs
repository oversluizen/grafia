//! Normative COTS Verification Suite for `SPEC-009` (`grafia v0.8.0` Auto-Trace, Preferences & Rendering Parity):
//! - `TC-V080-007`: Bitmap & Shadowed-Camera Auto-Trace (`GlobalLuminance` + `AdaptiveLocal`, Speckle Rejection, Winding & Counter Recovery)
//! - `TC-V080-008`: Bitmap Auto-Trace Perspective Homography Deskew, Sharp Corner Detection (`>= 12`) & Undo
//! - `TC-V080-009`: `GuideThickness::ScreenDp` vs `GuideThickness::FontUnits(f64)` Zoom Scaling & Band Edge Snapping
//! - `TC-V080-010`: `GrafiaPreferences` Custom Nudge/Kerning/SnapMask/Touch & `GrafiaKeymap` Rebind
//! - `TC-V080-011`: `egui` (`GrafiaCanvasWidget` + `ctx.tessellate`) vs `tiny-skia` Rendering Parity (`SSIM >= 0.985` Light & Dark)
//! - `TC-V080-012`: `proptest!` Hobby Tension & BBox Affine Stability
//! - `TC-V080-013`: `proptest!` Auto-Trace & Preferences Ranges
//! - `TC-V080-014`: Stateful `arbtest` Multi-Feature Undo/Redo Fuzzing

use super::{
    canvas_rect, compute_ssim, dispatch_events, pointer_move, pointer_press, pointer_release,
    set_contours, SSIM_DEFAULT_WINDOW_SIZE,
};
use approx::assert_relative_eq;
use arbtest::arbtest;
use egui::{
    Event, Key, Modifiers as EguiModifiers, PointerButton, Pos2, RawInput, TouchDeviceId, TouchId,
    TouchPhase, Vec2,
};
use grafia::adapters::headless::{pixmap_to_rgba_image, rasterize_canvas_subregion};
use grafia::{
    contour_signed_area, trace_background_image, Affine2D, AppearanceMode, AutoTraceConfig,
    AutoTraceThresholdMode, CanvasBackgroundImage, CanvasComponent, CanvasSmartHandle, CanvasTool,
    ComponentId, ContourId, FontPoint, Glyph, GlyphContour, GlyphMetrics, GlyphSlot,
    GrafiaCanvasState, GrafiaCanvasWidget, GrafiaKeymap, GrafiaPreferences, GrafiaTheme, Guide,
    GuideId, GuideThickness, HobbyDirConstraint, KeyChord, Node, NodeId, NodeType, ScreenPoint,
    SelectionInfoAction, SnapEngine, SnapTargetMask,
};
use image::{DynamicImage, ImageBuffer, Rgba};
use proptest::prelude::*;
use std::collections::HashMap;

fn send_key(
    state: &mut GrafiaCanvasState,
    key: Key,
    shift: bool,
    alt: bool,
    cmd: bool,
    ctrl: bool,
) {
    let rect = canvas_rect(800.0, 600.0);
    let m = EguiModifiers {
        alt,
        ctrl,
        shift,
        mac_cmd: cmd,
        command: cmd || ctrl,
    };
    let ev = Event::Key {
        key,
        physical_key: Some(key),
        pressed: true,
        repeat: false,
        modifiers: m,
    };
    dispatch_events(state, rect, vec![ev], shift, alt, cmd, ctrl);
}

fn build_synthetic_o_image(width: u32, height: u32, shadowed: bool) -> CanvasBackgroundImage {
    let cx = (width as f64) * 0.5;
    let cy = (height as f64) * 0.5;
    let r_outer = 88.0_f64;
    let r_inner = 48.0_f64;
    let mut rgba = Vec::with_capacity((width * height * 4) as usize);
    for y in 0..height {
        for x in 0..width {
            let fx = f64::from(x) + 0.5;
            let fy = f64::from(y) + 0.5;
            let dist = (fx - cx).hypot(fy - cy);
            let in_ring = (r_inner..=r_outer).contains(&dist);
            let in_speckle = (28..30).contains(&x) && (28..30).contains(&y);
            let is_ink = in_ring || in_speckle;
            let paper_luma = if shadowed {
                let diag = (fx / f64::from(width) + fy / f64::from(height)) * 0.5;
                (255.0 * (0.40 + 0.55 * diag)).round().clamp(1.0, 254.0) as u8
            } else {
                245
            };
            let lum = if is_ink {
                if shadowed {
                    ((f64::from(paper_luma) * 0.14).round() as u8).max(8)
                } else {
                    18
                }
            } else {
                paper_luma
            };
            rgba.extend_from_slice(&[lum, lum, lum, 255]);
        }
    }
    let mut bg = CanvasBackgroundImage::new(rgba, width, height);
    bg.scale_x = 2.0;
    bg.scale_y = 2.0;
    bg.position = FontPoint::new(20.0, 10.0);
    bg
}

fn solve_homography_8x8_test(src: [(f64, f64); 4], dst: [(f64, f64); 4]) -> [f64; 9] {
    let mut m = [[0.0_f64; 9]; 8];
    for i in 0..4 {
        let (x, y) = src[i];
        let (u, v) = dst[i];
        let r0 = i * 2;
        let r1 = r0 + 1;
        m[r0] = [x, y, 1.0, 0.0, 0.0, 0.0, -u * x, -u * y, u];
        m[r1] = [0.0, 0.0, 0.0, x, y, 1.0, -v * x, -v * y, v];
    }
    for col in 0..8 {
        let mut pivot = col;
        for row in (col + 1)..8 {
            if m[row][col].abs() > m[pivot][col].abs() {
                pivot = row;
            }
        }
        m.swap(col, pivot);
        let div = m[col][col];
        for cell in &mut m[col][col..9] {
            *cell /= div;
        }
        let pivot_row = m[col];
        for (row, row_vals) in m.iter_mut().enumerate() {
            if row == col {
                continue;
            }
            let factor = row_vals[col];
            for (dst, &pv) in row_vals[col..9].iter_mut().zip(&pivot_row[col..9]) {
                *dst -= factor * pv;
            }
        }
    }
    [
        m[0][8], m[1][8], m[2][8], m[3][8], m[4][8], m[5][8], m[6][8], m[7][8], 1.0,
    ]
}

fn build_tilted_h_background_image() -> (CanvasBackgroundImage, [(f64, f64); 4]) {
    let w = 256_u32;
    let h = 256_u32;
    let quad = [(34.0, 24.0), (226.0, 18.0), (244.0, 238.0), (14.0, 232.0)];
    let rect = [(0.0, 0.0), (256.0, 0.0), (256.0, 256.0), (0.0, 256.0)];
    // Map camera pixel (u, v) -> upright sheet coordinate (sx, sy)
    let h_cam_to_sheet = solve_homography_8x8_test(quad, rect);
    let mut rgba = Vec::with_capacity((w * h * 4) as usize);
    for py in 0..h {
        for px in 0..w {
            let u = f64::from(px) + 0.5;
            let v = f64::from(py) + 0.5;
            let denom = h_cam_to_sheet[6] * u + h_cam_to_sheet[7] * v + h_cam_to_sheet[8];
            let sx = (h_cam_to_sheet[0] * u + h_cam_to_sheet[1] * v + h_cam_to_sheet[2]) / denom;
            let sy = (h_cam_to_sheet[3] * u + h_cam_to_sheet[4] * v + h_cam_to_sheet[5]) / denom;
            let in_sheet = (0.0..=256.0).contains(&sx) && (0.0..=256.0).contains(&sy);
            let in_left_stem = (56.0..=96.0).contains(&sx) && (36.0..=220.0).contains(&sy);
            let in_right_stem = (160.0..=200.0).contains(&sx) && (36.0..=220.0).contains(&sy);
            let in_crossbar = (96.0..=160.0).contains(&sx) && (110.0..=146.0).contains(&sy);
            let lum = if in_sheet && (in_left_stem || in_right_stem || in_crossbar) {
                16_u8
            } else {
                244_u8
            };
            rgba.extend_from_slice(&[lum, lum, lum, 255]);
        }
    }
    let mut bg = CanvasBackgroundImage::new(rgba, w, h);
    bg.scale_x = 2.0;
    bg.scale_y = 2.0;
    (bg, quad)
}

fn assert_has_hv_extrema(nodes: &[Node]) {
    let mut min_x = f64::INFINITY;
    let mut max_x = f64::NEG_INFINITY;
    let mut min_y = f64::INFINITY;
    let mut max_y = f64::NEG_INFINITY;
    for n in nodes {
        assert!(n.point.x.0.is_finite() && n.point.y.0.is_finite());
        min_x = min_x.min(n.point.x.0);
        max_x = max_x.max(n.point.x.0);
        min_y = min_y.min(n.point.y.0);
        max_y = max_y.max(n.point.y.0);
    }
    let on_curve: Vec<&Node> = nodes
        .iter()
        .filter(|n| n.node_type != NodeType::OffCurve)
        .collect();
    assert!(
        on_curve.iter().any(|n| (n.point.x.0 - min_x).abs() <= 2.0),
        "missing left horizontal extremum OnCurve node"
    );
    assert!(
        on_curve.iter().any(|n| (n.point.x.0 - max_x).abs() <= 2.0),
        "missing right horizontal extremum OnCurve node"
    );
    assert!(
        on_curve.iter().any(|n| (n.point.y.0 - min_y).abs() <= 2.0),
        "missing bottom vertical extremum OnCurve node"
    );
    assert!(
        on_curve.iter().any(|n| (n.point.y.0 - max_y).abs() <= 2.0),
        "missing top vertical extremum OnCurve node"
    );
}

fn rasterize_egui_widget_canvas(
    state: &mut GrafiaCanvasState,
    width: u32,
    height: u32,
) -> DynamicImage {
    let rect = canvas_rect(width as f32, height as f32);
    let ctx = egui::Context::default();
    if state.theme.is_dark() {
        ctx.set_visuals(egui::Visuals::dark());
    } else {
        ctx.set_visuals(egui::Visuals::light());
    }
    let mut events = Vec::new();
    if let Some(cur) = state.cursor_screen_pos {
        events.push(Event::PointerMoved(Pos2::new(cur.x.0, cur.y.0)));
    }
    let raw = RawInput {
        screen_rect: Some(rect),
        time: Some(1.0),
        events,
        ..Default::default()
    };
    let mut textures: HashMap<egui::TextureId, (usize, usize, Vec<[u8; 4]>)> = HashMap::new();
    let mut full_output = ctx.run_ui(raw, |ui| {
        egui::CentralPanel::default()
            .frame(egui::Frame::NONE)
            .show(ui, |panel_ui| {
                panel_ui.add(
                    GrafiaCanvasWidget::from_state(state)
                        .with_size(Vec2::new(width as f32, height as f32)),
                );
            });
    });
    for (tid, deltas) in &full_output.textures_delta.set {
        for delta in deltas {
            let [dw, dh] = delta.image.size();
            let patch: Vec<[u8; 4]> = match &delta.image {
                egui::ImageData::Color(img) => img.pixels.iter().map(|c| c.to_array()).collect(),
            };
            if let Some([ox, oy]) = delta.pos {
                if let Some((tw, th, buf)) = textures.get_mut(tid) {
                    for ry in 0..dh {
                        for rx in 0..dw {
                            let tx = ox + rx;
                            let ty = oy + ry;
                            if tx < *tw && ty < *th {
                                buf[ty * *tw + tx] = patch[ry * dw + rx];
                            }
                        }
                    }
                }
            } else {
                textures.insert(*tid, (dw, dh, patch));
            }
        }
    }
    full_output.textures_delta.clear();

    let bg = state
        .theme
        .canvas_background_color32()
        .to_srgba_unmultiplied();
    let ss = 4_u32;
    let ss_w = width * ss;
    let ss_h = height * ss;
    let inv_ss = 1.0 / (ss as f32);
    let mut fb: Vec<[f32; 4]> = vec![
        [
            f32::from(bg[0]) / 255.0,
            f32::from(bg[1]) / 255.0,
            f32::from(bg[2]) / 255.0,
            1.0,
        ];
        (ss_w * ss_h) as usize
    ];
    let clipped = ctx.tessellate(full_output.shapes, 1.0);
    for prim in clipped {
        let egui::epaint::Primitive::Mesh(mesh) = prim.primitive else {
            continue;
        };
        let clip = prim.clip_rect;
        let tex = textures.get(&mesh.texture_id);
        for tri in mesh.indices.as_chunks::<3>().0 {
            let v0 = mesh.vertices[tri[0] as usize];
            let v1 = mesh.vertices[tri[1] as usize];
            let v2 = mesh.vertices[tri[2] as usize];
            let min_sx = ((v0.pos.x.min(v1.pos.x).min(v2.pos.x).max(clip.left()) * (ss as f32))
                .floor()
                .max(0.0)) as i32;
            let max_sx = ((v0.pos.x.max(v1.pos.x).max(v2.pos.x).min(clip.right()) * (ss as f32))
                .ceil()
                .min(ss_w as f32 - 1.0)) as i32;
            let min_sy = ((v0.pos.y.min(v1.pos.y).min(v2.pos.y).max(clip.top()) * (ss as f32))
                .floor()
                .max(0.0)) as i32;
            let max_sy = ((v0.pos.y.max(v1.pos.y).max(v2.pos.y).min(clip.bottom()) * (ss as f32))
                .ceil()
                .min(ss_h as f32 - 1.0)) as i32;
            let area = (v1.pos.x - v0.pos.x) * (v2.pos.y - v0.pos.y)
                - (v1.pos.y - v0.pos.y) * (v2.pos.x - v0.pos.x);
            if area.abs() < 1e-6 {
                continue;
            }
            let inv_area = 1.0 / area;
            let c0 = v0.color.to_array();
            let c1 = v1.color.to_array();
            let c2 = v2.color.to_array();
            for sy in min_sy..=max_sy {
                let py_f = (sy as f32 + 0.5) * inv_ss;
                for sx in min_sx..=max_sx {
                    let px_f = (sx as f32 + 0.5) * inv_ss;
                    let p = Pos2::new(px_f, py_f);
                    if !clip.contains(p) {
                        continue;
                    }
                    let w0 = ((v1.pos.x - p.x) * (v2.pos.y - p.y)
                        - (v1.pos.y - p.y) * (v2.pos.x - p.x))
                        * inv_area;
                    let w1 = ((v2.pos.x - p.x) * (v0.pos.y - p.y)
                        - (v2.pos.y - p.y) * (v0.pos.x - p.x))
                        * inv_area;
                    let w2 = 1.0 - w0 - w1;
                    if w0 < 0.0 || w1 < 0.0 || w2 < 0.0 {
                        continue;
                    }
                    let mut sr =
                        (w0 * f32::from(c0[0]) + w1 * f32::from(c1[0]) + w2 * f32::from(c2[0]))
                            / 255.0;
                    let mut sg =
                        (w0 * f32::from(c0[1]) + w1 * f32::from(c1[1]) + w2 * f32::from(c2[1]))
                            / 255.0;
                    let mut sb =
                        (w0 * f32::from(c0[2]) + w1 * f32::from(c1[2]) + w2 * f32::from(c2[2]))
                            / 255.0;
                    let mut sa =
                        (w0 * f32::from(c0[3]) + w1 * f32::from(c1[3]) + w2 * f32::from(c2[3]))
                            / 255.0;
                    if let Some((tw, th, tpx)) = tex {
                        let u = (w0 * v0.uv.x + w1 * v1.uv.x + w2 * v2.uv.x).clamp(0.0, 1.0);
                        let v = (w0 * v0.uv.y + w1 * v1.uv.y + w2 * v2.uv.y).clamp(0.0, 1.0);
                        let fx = (u * (*tw as f32) - 0.5).clamp(0.0, (*tw as f32 - 1.0).max(0.0));
                        let fy = (v * (*th as f32) - 0.5).clamp(0.0, (*th as f32 - 1.0).max(0.0));
                        let x0 = (fx.floor() as usize).min(tw.saturating_sub(1));
                        let y0 = (fy.floor() as usize).min(th.saturating_sub(1));
                        let x1 = (x0 + 1).min(tw.saturating_sub(1));
                        let y1 = (y0 + 1).min(th.saturating_sub(1));
                        let tx_f = fx - (x0 as f32);
                        let ty_f = fy - (y0 as f32);
                        let p00 = tpx[y0 * *tw + x0];
                        let p10 = tpx[y0 * *tw + x1];
                        let p01 = tpx[y1 * *tw + x0];
                        let p11 = tpx[y1 * *tw + x1];
                        let bilerp = |ch: usize| -> f32 {
                            let top = f32::from(p00[ch]) * (1.0 - tx_f) + f32::from(p10[ch]) * tx_f;
                            let bot = f32::from(p01[ch]) * (1.0 - tx_f) + f32::from(p11[ch]) * tx_f;
                            (top * (1.0 - ty_f) + bot * ty_f) / 255.0
                        };
                        sr *= bilerp(0);
                        sg *= bilerp(1);
                        sb *= bilerp(2);
                        sa *= bilerp(3);
                    }
                    let dst = &mut fb[(sy as u32 * ss_w + sx as u32) as usize];
                    let inv_a = (1.0 - sa).clamp(0.0, 1.0);
                    dst[0] = (sr + dst[0] * inv_a).clamp(0.0, 1.0);
                    dst[1] = (sg + dst[1] * inv_a).clamp(0.0, 1.0);
                    dst[2] = (sb + dst[2] * inv_a).clamp(0.0, 1.0);
                    dst[3] = 1.0;
                }
            }
        }
    }
    let mut img = ImageBuffer::new(width, height);
    let inv_samples = 1.0 / ((ss * ss) as f32);
    for y in 0..height {
        for x in 0..width {
            let mut acc_r = 0.0_f32;
            let mut acc_g = 0.0_f32;
            let mut acc_b = 0.0_f32;
            for oy in 0..ss {
                let row_off = (y * ss + oy) * ss_w;
                for ox in 0..ss {
                    let sp = fb[(row_off + x * ss + ox) as usize];
                    acc_r += sp[0];
                    acc_g += sp[1];
                    acc_b += sp[2];
                }
            }
            img.put_pixel(
                x,
                y,
                Rgba([
                    (acc_r * inv_samples * 255.0).round() as u8,
                    (acc_g * inv_samples * 255.0).round() as u8,
                    (acc_b * inv_samples * 255.0).round() as u8,
                    255,
                ]),
            );
        }
    }
    DynamicImage::ImageRgba8(img)
}

// ============================================================================
// TC-V080-007: Bitmap & Shadowed-Camera Auto-Trace Winding & Counter Recovery
// ============================================================================
#[test]
fn tc_v080_007_auto_trace_global_and_adaptive_local_shadow_and_speckle() {
    // 1. Uniform flatbed scan with 2x2 px (4 px^2 < 9.0) speckle -> GlobalLuminance(128)
    let clean_bg = build_synthetic_o_image(256, 256, false);
    let global_cfg = AutoTraceConfig {
        threshold_mode: AutoTraceThresholdMode::GlobalLuminance(128),
        min_speckle_area_px2: 9.0,
        snap_extrema: true,
        ..Default::default()
    };
    let clean_contours = trace_background_image(&clean_bg, &global_cfg);
    assert_eq!(
        clean_contours.len(),
        2,
        "GlobalLuminance must reject 2px speckle and emit outer + counter contours"
    );
    let mut clean_areas: Vec<f64> = clean_contours
        .iter()
        .map(|c| contour_signed_area(&c.nodes))
        .collect();
    clean_areas.sort_by(|a, b| a.total_cmp(b));
    assert!(
        clean_areas[0] < -10_000.0 && clean_areas[1] > 40_000.0,
        "Expected 1 CW counter (< 0) and 1 CCW outer (> 0), got {clean_areas:?}"
    );
    for c in &clean_contours {
        assert!(c.closed);
        assert_has_hv_extrema(&c.nodes);
    }

    // 2. Strong 40% -> 95% diagonal shadow gradient + 2x2 px speckle -> AdaptiveLocal
    let shadowed_bg = build_synthetic_o_image(256, 256, true);
    let adaptive_cfg = AutoTraceConfig {
        threshold_mode: AutoTraceThresholdMode::AdaptiveLocal {
            window_radius_px: 24,
            bias_k: 0.18,
        },
        min_speckle_area_px2: 9.0,
        snap_extrema: true,
        ..Default::default()
    };
    let shadow_contours = trace_background_image(&shadowed_bg, &adaptive_cfg);
    assert_eq!(
        shadow_contours.len(),
        2,
        "AdaptiveLocal must reject diagonal paper shadow + 2px speckle and recover 'O' + counter"
    );
    let mut shadow_areas: Vec<f64> = shadow_contours
        .iter()
        .map(|c| contour_signed_area(&c.nodes))
        .collect();
    shadow_areas.sort_by(|a, b| a.total_cmp(b));
    assert!(
        shadow_areas[0] < -10_000.0 && shadow_areas[1] > 40_000.0,
        "Expected 1 CW counter (< 0) and 1 CCW outer (> 0) under shadow, got {shadow_areas:?}"
    );
    for c in &shadow_contours {
        assert!(c.closed);
        assert_has_hv_extrema(&c.nodes);
    }

    // Adversarial edge case (c): uniform all-white and all-black bitmaps must return empty Vec without panic
    let all_white_bg = CanvasBackgroundImage::new([255_u8, 255, 255, 255].repeat(32 * 32), 32, 32);
    let all_black_bg = CanvasBackgroundImage::new([0_u8, 0, 0, 255].repeat(32 * 32), 32, 32);
    for bg in [&all_white_bg, &all_black_bg] {
        for invert_polarity in [false, true] {
            let cfg_global = AutoTraceConfig {
                threshold_mode: AutoTraceThresholdMode::GlobalLuminance(128),
                invert_polarity,
                ..Default::default()
            };
            assert!(
                trace_background_image(bg, &cfg_global).is_empty(),
                "Uniform bitmap under GlobalLuminance(invert={invert_polarity}) must return empty Vec"
            );
            let cfg_adaptive = AutoTraceConfig {
                threshold_mode: AutoTraceThresholdMode::AdaptiveLocal {
                    window_radius_px: 8,
                    bias_k: 0.18,
                },
                invert_polarity,
                ..Default::default()
            };
            assert!(
                trace_background_image(bg, &cfg_adaptive).is_empty(),
                "Uniform bitmap under AdaptiveLocal(invert={invert_polarity}) must return empty Vec"
            );
        }
    }
}

// ============================================================================
// TC-V080-008: Bitmap Auto-Trace Perspective Homography, Corners & Undo
// ============================================================================
#[test]
fn tc_v080_008_auto_trace_perspective_homography_corners_and_undo() {
    let mut state = GrafiaCanvasState::new(GrafiaTheme::sonoma_default());
    let (tilted_h, quad) = build_tilted_h_background_image();
    state.set_background_image(Some(tilted_h));
    assert!(state.contours.is_empty());

    let cfg = AutoTraceConfig {
        threshold_mode: AutoTraceThresholdMode::GlobalLuminance(128),
        perspective_quad_px: Some(quad),
        min_speckle_area_px2: 12.0,
        rdp_tolerance_px: 1.15,
        corner_angle_deg: 125.0,
        fit_error_upm: 2.5,
        snap_extrema: true,
        ..Default::default()
    };
    assert!(
        state.trace_active_background_image(&cfg),
        "trace_active_background_image must succeed on perspective-tilted 'H'"
    );
    assert_eq!(state.contours.len(), 1, "Rectified 'H' must be 1 contour");
    assert!(
        contour_signed_area(&state.contours[0]) > 20_000.0,
        "Outer 'H' contour must wind CCW (signed_area > 0)"
    );

    let corners: Vec<&Node> = state.contours[0]
        .iter()
        .filter(|n| n.node_type == NodeType::Corner)
        .collect();
    assert!(
        corners.len() >= 12,
        "Expected >= 12 sharp NodeType::Corner stem junctions on rectified 'H', got {}",
        corners.len()
    );

    // Verify 3x3 homography rectified vertical left stem (x ~= 56 * 2 = 112 UPM at both top & bottom)
    let top_left_x = corners
        .iter()
        .filter(|n| n.point.y.0 > 350.0)
        .map(|n| n.point.x.0)
        .fold(f64::INFINITY, f64::min);
    let bot_left_x = corners
        .iter()
        .filter(|n| n.point.y.0 < 160.0)
        .map(|n| n.point.x.0)
        .fold(f64::INFINITY, f64::min);
    assert!(
        (top_left_x - bot_left_x).abs() <= 8.0,
        "Homography deskew must align top-left ({top_left_x:.2}) and bottom-left ({bot_left_x:.2}) stem corners vertically"
    );

    assert_eq!(
        state.history.last_undo_label(),
        Some("Trace Background Image")
    );
    assert!(state.undo());
    assert!(
        state.contours.is_empty(),
        "Undo must restore empty foreground contours"
    );
    state.assert_invariants();
}

// ============================================================================
// TC-V080-009: GuideThickness::ScreenDp vs GuideThickness::FontUnits(f64) & Snap
// ============================================================================
#[test]
fn tc_v080_009_guide_thickness_screen_dp_vs_font_units_and_snap() {
    let mut state = GrafiaCanvasState::new(GrafiaTheme::sonoma_default());
    state.show_metrics = false;
    state.show_grid = false;
    state.show_guides = true;
    state.theme.edit_canvas.guide_stroke_width_dp = 2.0;

    let mut guide_a = Guide::new(GuideId(1), FontPoint::new(20.0, 260.0), 0.0, false);
    guide_a.thickness = GuideThickness::ScreenDp;
    let mut guide_b = Guide::new(GuideId(2), FontPoint::new(20.0, 100.0), 0.0, false);
    guide_b.thickness = GuideThickness::FontUnits(32.5);
    state.guides = vec![guide_a, guide_b];

    let measure_band_px = |st: &GrafiaCanvasState, y_upm: f64| -> u32 {
        let pix = rasterize_canvas_subregion(st, &st.theme, 400, 720);
        let img = pixmap_to_rgba_image(pix);
        let bg = st.theme.canvas_background_color32().to_srgba_unmultiplied();
        let center_y = st
            .viewport
            .to_screen(FontPoint::new(200.0, y_upm))
            .y
            .0
            .round() as i32;
        let sample_x = 280_u32;
        let mut count = 0_u32;
        for py in (center_y - 60).max(0)..=(center_y + 60).min(719) {
            let p = img.get_pixel(sample_x, py as u32).0;
            if p[0] != bg[0] || p[1] != bg[1] || p[2] != bg[2] {
                count += 1;
            }
        }
        count
    };

    // Zoom 0.5x: Guide A stays ~2 px, Guide B = 32.5 * 0.5 = 16.25 px
    state.viewport.zoom = 0.5;
    state.viewport.pan_offset = ScreenPoint::new(20.0, 320.0);
    let a_half_px = measure_band_px(&state, 260.0);
    let b_half_px = measure_band_px(&state, 100.0);
    assert!(
        (2..=3).contains(&a_half_px),
        "Guide A (ScreenDp, 2.0 dp) at 0.5x must render ~2 px thick, got {a_half_px}"
    );
    assert!(
        (15..=18).contains(&b_half_px),
        "Guide B (FontUnits(32.5)) at 0.5x must render ~16.25 px thick, got {b_half_px}"
    );

    // Zoom 2.0x: Guide A stays ~2 px, Guide B = 32.5 * 2.0 = 65.0 px
    state.viewport.zoom = 2.0;
    state.viewport.pan_offset = ScreenPoint::new(20.0, 640.0);
    let a_two_px = measure_band_px(&state, 260.0);
    let b_two_px = measure_band_px(&state, 100.0);
    assert!(
        (2..=3).contains(&a_two_px),
        "Guide A (ScreenDp, 2.0 dp) at 2.0x must remain ~2 px thick, got {a_two_px}"
    );
    assert!(
        (63..=67).contains(&b_two_px),
        "Guide B (FontUnits(32.5)) at 2.0x must scale to ~65.0 px thick, got {b_two_px}"
    );

    // SnapEngine snaps to Guide B centerline (y = 100.0) AND +-16.25 UPM band edges (116.25 & 83.75)
    state.viewport.zoom = 1.0;
    let guide_ctx = grafia::snapping::SnapGuideContext {
        show_guides: true,
        local_guides: &state.guides,
        global_guides: &[],
    };
    let snap_center = SnapEngine::snap_point(
        FontPoint::new(200.0, 101.5),
        (&[], &[]),
        &GlyphMetrics::default(),
        (600.0, 0.0),
        guide_ctx,
        1.0,
        false,
    );
    assert_relative_eq!(snap_center.snapped_point.y.0, 100.0, epsilon = 1e-6);

    let snap_upper = SnapEngine::snap_point(
        FontPoint::new(200.0, 117.5),
        (&[], &[]),
        &GlyphMetrics::default(),
        (600.0, 0.0),
        guide_ctx,
        1.0,
        false,
    );
    assert_relative_eq!(snap_upper.snapped_point.y.0, 116.25, epsilon = 1e-6);

    let snap_lower = SnapEngine::snap_point(
        FontPoint::new(200.0, 82.5),
        (&[], &[]),
        &GlyphMetrics::default(),
        (600.0, 0.0),
        guide_ctx,
        1.0,
        false,
    );
    assert_relative_eq!(snap_lower.snapped_point.y.0, 83.75, epsilon = 1e-6);
}

// ============================================================================
// TC-V080-010: GrafiaPreferences Nudge/Kerning/SnapMask/Touch & GrafiaKeymap
// ============================================================================
#[test]
fn tc_v080_010_preferences_nudge_kerning_snap_mask_and_keymap_rebind() {
    let mut state = GrafiaCanvasState::new(GrafiaTheme::sonoma_default());
    state.viewport.zoom = 1.0;
    state.viewport.pan_offset = ScreenPoint::new(100.0, 600.0);

    let prefs = GrafiaPreferences {
        nudge_step_small_upm: 0.25,
        nudge_step_medium_upm: 5.0,
        kerning_step_low_upm: 2.0,
        round_to_grid: false,
        snap_radius_dp: 6.0,
        snap_targets: SnapTargetMask {
            guides: false,
            metrics: true,
            grid: false,
            nodes: false,
            background: false,
            italic_angle: false,
            reference_frame: false,
        },
        touch_finger_offset_dp: 44.0,
        show_touch_precision_loupe: true,
        ..GrafiaPreferences::default()
    };
    state.apply_preferences(&prefs);

    let mut keymap = GrafiaKeymap::default();
    keymap.bind_tool(CanvasTool::Knife, KeyChord::plain(Key::K));
    state.apply_keymap(&keymap);

    // 1. Fractional 0.25 UPM & medium 5.0 UPM keyboard nudges via InputDispatcher
    set_contours(
        &mut state,
        vec![GlyphContour::new(
            ContourId(1),
            vec![
                Node::new(NodeId(1), 100.0, 200.0, NodeType::Corner),
                Node::new(NodeId(2), 200.0, 200.0, NodeType::Corner),
                Node::new(NodeId(3), 200.0, 300.0, NodeType::Corner),
            ],
        )],
    );
    state.selected_nodes = vec![NodeId(1)];
    send_key(&mut state, Key::ArrowRight, false, false, false, false);
    assert_relative_eq!(
        state.find_node(NodeId(1)).map_or(0.0, |n| n.point.x.0),
        100.25,
        epsilon = 1e-6
    );
    send_key(&mut state, Key::ArrowUp, true, false, false, false);
    assert_relative_eq!(
        state.find_node(NodeId(1)).map_or(0.0, |n| n.point.y.0),
        205.0,
        epsilon = 1e-6
    );

    // 2. SnapTargetMask: guides = false bypassed, metrics = true stays active
    state.guides = vec![Guide::new(
        GuideId(10),
        FontPoint::new(150.0, 320.0),
        0.0,
        false,
    )];
    state.show_guides = true;
    state.show_metrics = true;
    let bypassed_guide = state.evaluate_magnetic_snap(FontPoint::new(150.0, 321.5), &[], false);
    assert_relative_eq!(bypassed_guide.snapped_point.y.0, 321.5, epsilon = 1e-6);
    let snapped_metric = state.evaluate_magnetic_snap(FontPoint::new(150.0, 502.0), &[], false);
    assert_relative_eq!(snapped_metric.snapped_point.y.0, 500.0, epsilon = 1e-6);

    // 3. Kerning step = 2.0 UPM in Text mode
    state.set_text_stream_slots(
        vec![
            GlyphSlot::new(
                Glyph {
                    name: "A".into(),
                    width: 600.0,
                    ..Glyph::default()
                },
                0,
            ),
            GlyphSlot::new(
                Glyph {
                    name: "V".into(),
                    width: 600.0,
                    ..Glyph::default()
                },
                0,
            ),
        ],
        1,
    );
    state.activate_tool(CanvasTool::Text);
    send_key(&mut state, Key::ArrowRight, false, true, false, true);
    assert_eq!(state.text_stream.slots[1].kern_with_prev, 2);

    // 4. Touch finger offset (44.0 dp upward) selects NodeId(2) when finger lands 44 dp below it
    state.set_text_stream_slots(Vec::new(), 0);
    set_contours(
        &mut state,
        vec![GlyphContour::new(
            ContourId(1),
            vec![
                Node::new(NodeId(1), 100.0, 200.0, NodeType::Corner),
                Node::new(NodeId(2), 200.0, 200.0, NodeType::Corner),
                Node::new(NodeId(3), 200.0, 300.0, NodeType::Corner),
            ],
        )],
    );
    state.activate_tool(CanvasTool::Select);
    state.selected_nodes.clear();
    let n2_scr = state.viewport.to_screen(FontPoint::new(200.0, 200.0));
    let finger_pos = Pos2::new(n2_scr.x.0, n2_scr.y.0 + 44.0);
    let rect = canvas_rect(800.0, 600.0);
    dispatch_events(
        &mut state,
        rect,
        vec![
            Event::Touch {
                device_id: TouchDeviceId(1),
                id: TouchId(1),
                phase: TouchPhase::Start,
                pos: finger_pos,
                force: None,
            },
            Event::PointerButton {
                pos: finger_pos,
                button: PointerButton::Primary,
                pressed: true,
                modifiers: EguiModifiers::NONE,
            },
            Event::PointerButton {
                pos: finger_pos,
                button: PointerButton::Primary,
                pressed: false,
                modifiers: EguiModifiers::NONE,
            },
        ],
        false,
        false,
        false,
        false,
    );
    assert_eq!(
        state.selected_nodes,
        vec![NodeId(2)],
        "Touch contact 44 dp below NodeId(2) must hit NodeId(2) via touch_finger_offset_dp"
    );

    // 5. Keymap rebind: 'E' no longer activates Knife, 'K' activates Knife
    send_key(&mut state, Key::E, false, false, false, false);
    assert_eq!(state.active_tool, CanvasTool::Select);
    send_key(&mut state, Key::K, false, false, false, false);
    assert_eq!(state.active_tool, CanvasTool::Knife);
}

// ============================================================================
// TC-V080-011: egui (GrafiaCanvasWidget) vs tiny-skia Rendering Parity (Light & Dark)
// ============================================================================
#[test]
fn tc_v080_011_egui_vs_tiny_skia_rendering_parity_v080() {
    for mode in [AppearanceMode::Light, AppearanceMode::Dark] {
        let is_dark = mode == AppearanceMode::Dark;
        let mut state = GrafiaCanvasState::new(GrafiaTheme::for_appearance(mode, is_dark));
        state.set_appearance_with_system(mode, is_dark);
        state.viewport.zoom = 1.0;
        state.viewport.pan_offset = ScreenPoint::new(60.0, 440.0);
        state.show_metrics = false;
        state.show_grid = false;
        state.show_selection_info_hud = false;
        state.show_guides = true;
        state.show_extra_nodes = true;
        state.show_bounding_box = false;

        // 1. HobbyStar + EulerSpiro contour & 2 crossing contours for ExtraNodes
        let spline_contour = GlyphContour::new(
            ContourId(1),
            vec![
                Node::new(
                    NodeId(1),
                    40.0,
                    260.0,
                    NodeType::HobbyStar {
                        tension_in: 1.1,
                        tension_out: 1.1,
                        dir_constraint: HobbyDirConstraint::Free,
                    },
                ),
                Node::new(
                    NodeId(2),
                    110.0,
                    330.0,
                    NodeType::EulerSpiro {
                        dir_constraint: HobbyDirConstraint::Horizontal,
                    },
                ),
                Node::new(
                    NodeId(3),
                    180.0,
                    260.0,
                    NodeType::HobbyStar {
                        tension_in: 1.1,
                        tension_out: 1.1,
                        dir_constraint: HobbyDirConstraint::Free,
                    },
                ),
                Node::new(
                    NodeId(4),
                    110.0,
                    190.0,
                    NodeType::EulerSpiro {
                        dir_constraint: HobbyDirConstraint::Horizontal,
                    },
                ),
            ],
        );
        let cross_h = GlyphContour::new(
            ContourId(2),
            vec![
                Node::new(NodeId(11), 230.0, 240.0, NodeType::Corner),
                Node::new(NodeId(12), 370.0, 240.0, NodeType::Corner),
                Node::new(NodeId(13), 370.0, 280.0, NodeType::Corner),
                Node::new(NodeId(14), 230.0, 280.0, NodeType::Corner),
            ],
        );
        let cross_v = GlyphContour::new(
            ContourId(3),
            vec![
                Node::new(NodeId(21), 280.0, 190.0, NodeType::Corner),
                Node::new(NodeId(22), 320.0, 190.0, NodeType::Corner),
                Node::new(NodeId(23), 320.0, 330.0, NodeType::Corner),
                Node::new(NodeId(24), 280.0, 330.0, NodeType::Corner),
            ],
        );
        set_contours(&mut state, vec![spline_contour, cross_h, cross_v]);
        state.selected_nodes = vec![NodeId(11), NodeId(12), NodeId(13), NodeId(14)];
        state.active_bbox_rotation_deg = Some(15.0);
        state.cursor_screen_pos = Some(ScreenPoint::new(430.0, 160.0));
        state.last_pointer_screen_pos = Some(ScreenPoint::new(430.0, 160.0));

        // 2. Selected CanvasComponent + CanvasSmartHandle
        state.set_components(vec![CanvasComponent {
            id: ComponentId(5),
            component_name: "stem".into(),
            contours: vec![GlyphContour::new(
                ContourId(30),
                vec![
                    Node::new(NodeId(51), 410.0, 190.0, NodeType::Corner),
                    Node::new(NodeId(52), 470.0, 190.0, NodeType::Corner),
                    Node::new(NodeId(53), 470.0, 310.0, NodeType::Corner),
                    Node::new(NodeId(54), 410.0, 310.0, NodeType::Corner),
                ],
            )],
            transform: Affine2D::identity(),
            locked: false,
            automatic_alignment: false,
            selected: true,
        }]);
        state.set_smart_handles(vec![CanvasSmartHandle {
            component_id: ComponentId(5),
            axis_id: "wdth".into(),
            axis_name: "Width".into(),
            anchor_upm: FontPoint::new(410.0, 340.0),
            direction_unit: (1.0, 0.0),
            scale_upm_per_unit: 1.0,
            min_val: 0.0,
            default_val: 40.0,
            max_val: 90.0,
            current_val: 55.0,
        }]);

        // 3. GuideThickness::FontUnits(32.5) band at y = 80.0
        let mut band_guide = Guide::new(GuideId(7), FontPoint::new(40.0, 80.0), 0.0, false);
        band_guide.thickness = GuideThickness::FontUnits(32.5);
        state.guides = vec![band_guide];

        let egui_img = rasterize_egui_widget_canvas(&mut state, 620, 460);
        let ts_pix = rasterize_canvas_subregion(&state, &state.theme, 620, 460);
        let ts_img = DynamicImage::ImageRgba8(pixmap_to_rgba_image(ts_pix));

        // Verify GuideThickness::FontUnits(32.5) band is actually painted 12 px away from centerline y = 360
        let bg = state
            .theme
            .canvas_background_color32()
            .to_srgba_unmultiplied();
        let band_px_ts = ts_img.to_rgba8().get_pixel(300, 360 - 12).0;
        assert!(
            band_px_ts[0] != bg[0] || band_px_ts[1] != bg[1] || band_px_ts[2] != bg[2],
            "GuideThickness::FontUnits(32.5) band must be rendered in mode {mode:?}"
        );

        let ssim = compute_ssim(&egui_img, &ts_img, SSIM_DEFAULT_WINDOW_SIZE);
        assert!(
            ssim >= 0.985,
            "egui GrafiaCanvasWidget vs tiny-skia SSIM in {mode:?} must be >= 0.985, got {ssim:.6}"
        );

        // Adversarial edge case (d): 10 consecutive idle frames in CanvasRenderCache
        // Verify get_or_upload_background_texture and cached_extra_nodes do NOT recompute when content_generation is unchanged
        let mut bg_for_cache =
            CanvasBackgroundImage::new([120_u8, 160, 200, 255].repeat(16 * 16), 16, 16);
        bg_for_cache.alpha = 0.5;
        state.set_background_image(Some(bg_for_cache));
        let gen0 = state.content_generation();
        let expected_extra = grafia::render::cached_extra_nodes(&state);
        assert_eq!(expected_extra.len(), 4);

        let idle_ctx = egui::Context::default();
        if state.theme.is_dark() {
            idle_ctx.set_visuals(egui::Visuals::dark());
        } else {
            idle_ctx.set_visuals(egui::Visuals::light());
        }
        let idle_rect = canvas_rect(620.0, 460.0);
        for frame_idx in 0..10_usize {
            let raw = RawInput {
                screen_rect: Some(idle_rect),
                time: Some(1.0 + (frame_idx as f64) * 0.016),
                ..Default::default()
            };
            let mut out = idle_ctx.run_ui(raw, |ui| {
                egui::CentralPanel::default()
                    .frame(egui::Frame::NONE)
                    .show(ui, |panel_ui| {
                        panel_ui.add(
                            GrafiaCanvasWidget::from_state(&mut state)
                                .with_size(Vec2::new(620.0, 460.0)),
                        );
                    });
            });
            let uploaded_textures = !out.textures_delta.set.is_empty();
            out.textures_delta.clear();
            if frame_idx == 0 {
                assert!(
                    uploaded_textures,
                    "Frame 0 must upload background texture once"
                );
            } else {
                assert!(
                    !uploaded_textures,
                    "Idle frame {frame_idx} must NOT re-upload background texture when content_generation is unchanged"
                );
            }
            assert_eq!(state.content_generation(), gen0);
            assert_eq!(grafia::render::cached_extra_nodes(&state), expected_extra);
        }
    }
}

// ============================================================================
// TC-V080-012: Property-Based proptest Hobby Tension & BBox Affine Stability
// ============================================================================
proptest! {
    #![proptest_config(ProptestConfig {
        cases: 48,
        failure_persistence: None,
        ..ProptestConfig::default()
    })]
    #[test]
    fn tc_v080_012_proptest_hobby_tension_and_bbox_affine_stability(
        tau in 0.75_f64..=4.0_f64,
        theta in -180.0_f64..=180.0_f64,
        gamma in -1.0_f64..=1.0_f64,
    ) {
        let mut state = GrafiaCanvasState::new(GrafiaTheme::sonoma_default());
        let prefs = GrafiaPreferences {
            nudge_step_small_upm: 0.5,
            round_to_grid: false,
            ..GrafiaPreferences::default()
        };
        state.apply_preferences(&prefs);
        prop_assert!((state.preferences.nudge_step_small_upm - 0.5).abs() < 1e-9);

        let contour = GlyphContour::new(
            ContourId(1),
            vec![
                Node::new(
                    NodeId(1),
                    200.0,
                    120.0,
                    NodeType::HobbyStar {
                        tension_in: tau,
                        tension_out: tau,
                        dir_constraint: HobbyDirConstraint::Free,
                    },
                ),
                Node::new(
                    NodeId(2),
                    340.0,
                    260.0,
                    NodeType::HobbyStar {
                        tension_in: tau,
                        tension_out: tau,
                        dir_constraint: HobbyDirConstraint::Vertical,
                    },
                ),
                Node::new(
                    NodeId(3),
                    200.0,
                    400.0,
                    NodeType::HobbyStar {
                        tension_in: tau,
                        tension_out: tau,
                        dir_constraint: HobbyDirConstraint::Horizontal,
                    },
                ),
                Node::new(
                    NodeId(4),
                    60.0,
                    260.0,
                    NodeType::EulerSpiro {
                        dir_constraint: HobbyDirConstraint::Free,
                    },
                ),
            ],
        );
        set_contours(&mut state, vec![contour]);
        state.selected_nodes = vec![NodeId(1), NodeId(2), NodeId(3), NodeId(4)];
        let _ = state.rotate_by_degrees(theta);
        let shear_deg = gamma.atan().to_degrees();
        let _ = state.slant_by_degrees(shear_deg, true);

        prop_assert!(state.validate_invariants().is_ok());
        for node in state.contours.iter().flatten() {
            prop_assert!(node.point.x.0.is_finite());
            prop_assert!(node.point.y.0.is_finite());
        }
        for path in &state.paths {
            for el in path.elements() {
                match el {
                    kurbo::PathEl::MoveTo(p) | kurbo::PathEl::LineTo(p) => {
                        prop_assert!(p.x.is_finite() && p.y.is_finite());
                    }
                    kurbo::PathEl::QuadTo(p1, p2) => {
                        prop_assert!(p1.x.is_finite() && p1.y.is_finite());
                        prop_assert!(p2.x.is_finite() && p2.y.is_finite());
                    }
                    kurbo::PathEl::CurveTo(p1, p2, p3) => {
                        prop_assert!(p1.x.is_finite() && p1.y.is_finite());
                        prop_assert!(p2.x.is_finite() && p2.y.is_finite());
                        prop_assert!(p3.x.is_finite() && p3.y.is_finite());
                    }
                    kurbo::PathEl::ClosePath => {}
                }
            }
        }
    }
}

// ============================================================================
// TC-V080-013: Property-Based proptest Auto-Trace & Preferences Ranges
// ============================================================================
proptest! {
    #![proptest_config(ProptestConfig::with_cases(32))]
    #[test]
    fn tc_v080_013_proptest_auto_trace_and_preferences_ranges(
        luminance_threshold in 16_u8..=240_u8,
        nudge_step_small_upm in 0.05_f64..=50.0_f64,
        guide_stroke_width_dp in 0.5_f32..=4.0_f32,
    ) {
        let w = 64_u32;
        let h = 64_u32;
        let mut rgba = Vec::with_capacity((w * h * 4) as usize);
        for y in 0..h {
            for x in 0..w {
                let d = (f64::from(x) - 32.0).hypot(f64::from(y) - 32.0);
                let lum = if d <= 18.0 { 4_u8 } else { 252_u8 };
                rgba.extend_from_slice(&[lum, lum, lum, 255]);
            }
        }
        let mut bg = CanvasBackgroundImage::new(rgba, w, h);
        bg.scale_x = 4.0;
        bg.scale_y = 4.0;

        let cfg = AutoTraceConfig {
            threshold_mode: AutoTraceThresholdMode::GlobalLuminance(luminance_threshold),
            min_speckle_area_px2: 6.0,
            ..Default::default()
        };
        let traced = trace_background_image(&bg, &cfg);
        prop_assert!(!traced.is_empty());
        for gc in &traced {
            prop_assert!(gc.closed);
            for n in &gc.nodes {
                prop_assert!(n.point.x.0.is_finite() && n.point.y.0.is_finite());
            }
        }

        let mut state = GrafiaCanvasState::new(GrafiaTheme::sonoma_default());
        state.theme.edit_canvas.guide_stroke_width_dp = guide_stroke_width_dp;
        let prefs = GrafiaPreferences {
            nudge_step_small_upm,
            round_to_grid: false,
            ..GrafiaPreferences::default()
        };
        state.apply_preferences(&prefs);
        set_contours(&mut state, traced);
        if let Some(first_id) = state.contours.first().and_then(|c| c.first()).map(|n| n.id) {
            let x_before = state.find_node(first_id).map_or(0.0, |n| n.point.x.0);
            state.selected_nodes = vec![first_id];
            send_key(&mut state, Key::ArrowRight, false, false, false, false);
            let x_after = state.find_node(first_id).map_or(0.0, |n| n.point.x.0);
            prop_assert!((x_after - (x_before + nudge_step_small_upm)).abs() <= 1e-4);
        }
        prop_assert!(state.validate_invariants().is_ok());
    }
}

// ============================================================================
// TC-V080-014: Stateful arbtest Multi-Feature Undo/Redo Fuzzing
// ============================================================================
#[test]
fn tc_v080_014_arbtest_multi_feature_undo_redo_fuzzing() {
    arbtest(|u| {
        let mut state = GrafiaCanvasState::new(GrafiaTheme::sonoma_default());
        state.viewport.zoom = 1.0;
        state.viewport.pan_offset = ScreenPoint::new(100.0, 500.0);
        state.show_bounding_box = true;

        // Seed synthetic background image for auto-trace steps
        let mut rgba = Vec::with_capacity(48 * 48 * 4);
        for y in 0..48_u32 {
            for x in 0..48_u32 {
                let lum = if (12..36).contains(&x) && (12..36).contains(&y) {
                    12_u8
                } else {
                    248_u8
                };
                rgba.extend_from_slice(&[lum, lum, lum, 255]);
            }
        }
        let mut bg = CanvasBackgroundImage::new(rgba, 48, 48);
        bg.scale_x = 3.0;
        bg.scale_y = 3.0;
        state.background_image = Some(bg);

        set_contours(
            &mut state,
            vec![GlyphContour::new(
                ContourId(1),
                vec![
                    Node::new(
                        NodeId(1),
                        120.0,
                        120.0,
                        NodeType::HobbyStar {
                            tension_in: 1.0,
                            tension_out: 1.0,
                            dir_constraint: HobbyDirConstraint::Free,
                        },
                    ),
                    Node::new(NodeId(2), 260.0, 120.0, NodeType::Corner),
                    Node::new(NodeId(3), 260.0, 260.0, NodeType::Corner),
                    Node::new(NodeId(4), 120.0, 260.0, NodeType::Corner),
                ],
            )],
        );
        state.components = vec![CanvasComponent {
            id: ComponentId(1),
            component_name: "part".into(),
            contours: Vec::new(),
            transform: Affine2D::identity(),
            locked: false,
            automatic_alignment: false,
            selected: true,
        }];
        state.smart_handles = vec![CanvasSmartHandle {
            component_id: ComponentId(1),
            axis_id: "wght".into(),
            axis_name: "Weight".into(),
            anchor_upm: FontPoint::new(320.0, 200.0),
            direction_unit: (1.0, 0.0),
            scale_upm_per_unit: 1.0,
            min_val: 0.0,
            default_val: 50.0,
            max_val: 100.0,
            current_val: 50.0,
        }];
        state.selected_nodes = vec![NodeId(1)];

        // Verify deterministic auto-trace + Undo round-trip at start of trajectory
        let base_snap = state.snapshot();
        assert!(
            state.trace_active_background_image(&AutoTraceConfig::default()),
            "trace_active_background_image must succeed in multi-feature Undo/Redo test"
        );
        let traced_snap = state.snapshot();
        assert!(state.undo());
        assert_eq!(state.snapshot(), base_snap);
        assert!(state.redo());
        assert_eq!(state.snapshot(), traced_snap);
        assert!(state.undo());
        assert_eq!(state.snapshot(), base_snap);

        let mut timeline = vec![state.snapshot()];
        let mut cursor = 0_usize;
        let rect = canvas_rect(800.0, 600.0);
        let steps: usize = u.int_in_range(8..=20)?;

        for _ in 0..steps {
            let op: u8 = u.int_in_range(0..=6)?;
            match op {
                0 => {
                    // Mutate HobbyStar tension on currently selected HobbyStar node
                    let t_milli: u16 = u.int_in_range(800..=3200)?;
                    let tau = f64::from(t_milli) / 1000.0;
                    if state.apply_selection_info_action(SelectionInfoAction::SetHobbyNode {
                        tension_in: tau,
                        tension_out: tau,
                        dir_constraint: HobbyDirConstraint::Free,
                    }) {
                        timeline.truncate(cursor + 1);
                        timeline.push(state.snapshot());
                        cursor += 1;
                    }
                }
                1 => {
                    // BBox rotation / slant
                    let deg_i: i16 = u.int_in_range(-45..=45)?;
                    if deg_i != 0 && state.rotate_by_degrees(f64::from(deg_i)) {
                        timeline.truncate(cursor + 1);
                        timeline.push(state.snapshot());
                        cursor += 1;
                    }
                }
                2 => {
                    // Smart-handle pointer drag
                    if let Some(h) = state.smart_handles.first().cloned() {
                        let start_scr = state.viewport.to_screen(FontPoint::new(
                            h.anchor_upm.x.0 + h.current_val,
                            h.anchor_upm.y.0,
                        ));
                        let target_val = f64::from(u.int_in_range(5..=95)?);
                        if (target_val - h.current_val).abs() > 6.0 {
                            let end_scr = state.viewport.to_screen(FontPoint::new(
                                h.anchor_upm.x.0 + target_val,
                                h.anchor_upm.y.0,
                            ));
                            let before = state.snapshot();
                            pointer_press(&mut state, rect, start_scr, false, false, false, false);
                            pointer_move(&mut state, rect, end_scr, false, false, false, false);
                            pointer_release(&mut state, rect, end_scr, false, false, false, false);
                            if state.snapshot() != before {
                                timeline.truncate(cursor + 1);
                                timeline.push(state.snapshot());
                                cursor += 1;
                            }
                        }
                    }
                }
                3 => {
                    // Auto-trace background image (capped so contour count stays small)
                    if state.contours.len() < 4
                        && state.trace_active_background_image(&AutoTraceConfig::default())
                    {
                        timeline.truncate(cursor + 1);
                        timeline.push(state.snapshot());
                        cursor += 1;
                    }
                }
                4 => {
                    // Update preferences + nudge currently selected node(s)
                    let step_q: u8 = u.int_in_range(1..=8)?;
                    let prefs = GrafiaPreferences {
                        nudge_step_small_upm: f64::from(step_q) * 0.5,
                        round_to_grid: false,
                        ..GrafiaPreferences::default()
                    };
                    state.apply_preferences(&prefs);
                    let before = state.snapshot();
                    send_key(&mut state, Key::ArrowRight, false, false, false, false);
                    if state.snapshot() != before {
                        timeline.truncate(cursor + 1);
                        timeline.push(state.snapshot());
                        cursor += 1;
                    }
                }
                5 => {
                    if cursor > 0 {
                        assert!(state.undo());
                        cursor -= 1;
                        assert_eq!(state.snapshot(), timeline[cursor]);
                    }
                }
                _ => {
                    if cursor + 1 < timeline.len() {
                        assert!(state.redo());
                        cursor += 1;
                        assert_eq!(state.snapshot(), timeline[cursor]);
                    }
                }
            }
            assert!(state.validate_invariants().is_ok());
        }
        Ok(())
    });
}
