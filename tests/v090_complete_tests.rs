#![allow(
    clippy::too_many_lines,
    clippy::cognitive_complexity,
    clippy::expect_used,
    clippy::unwrap_used,
    clippy::panic,
    clippy::too_many_arguments,
    clippy::iter_over_hash_type
)]
#![allow(unexpected_cfgs)]
//! Normative COTS Verification Suite for `SPEC-010` (`grafia v0.9.0` Slice 3: `TC-V090-001` .. `TC-V090-007`):
//! - `TC-V090-001`: Multi-Sublayer `Solid`, `LinearGradient`, `ClippingMask` & Porter-Duff `SrcOver` Compositing
//! - `TC-V090-002`: `RadialGradient`, `SweepGradient` & `Bitmap` Sublayer Handles + `GradientStop` Drag/Mutation
//! - `TC-V090-003`: `render_colored_thumbnail` Multi-Layer Projection (`Light` & `Dark` + Monochrome Fallback)
//! - `TC-V090-004`: Vertical `WritingDirection::TopToBottom` Caret & `GlyphSlot` Advancement
//! - `TC-V090-005`: Vertical `TSB` / `BSB` & Vertical Kerning Shortcuts (`MetricsAxis::Vertical` + `"Change Vertical Metrics"` Undo)
//! - `TC-V090-006`: `CustomReferenceFrame` Outer/Inner Box, $3 \times 3$ Subdivisions & `SnapEngine` Integration
//! - `TC-V090-007`: `CustomReferenceFrame` Parallel `ReferenceBandLine`s (`GuideThickness::FontUnits(32.5)`) & Keyline Circles (`SSIM >= 0.985`)

#[path = "visual_pixel_diff_tests.rs"]
pub mod visual_pixel_diff_tests;

use approx::assert_relative_eq;
use egui::{Event, Key, Modifiers as EguiModifiers, PointerButton, Pos2, RawInput, Rect, Vec2};
use grafia::adapters::headless::{pixmap_to_rgba_image, rasterize_canvas_subregion};
use grafia::adapters::input_dispatcher::InputDispatcher;
use grafia::{
    render_colored_thumbnail, AppearanceMode, CanvasBackgroundImage, CanvasColorSublayer,
    CanvasHostRequest, CanvasTool, ColorLayerPaint, ColorSublayerMutation, ContourId,
    CustomReferenceFrame, FontPoint, Glyph, GlyphContour, GlyphSlot, GradientExtend,
    GradientSpread, GradientStop, GrafiaCanvasState, GrafiaCanvasWidget, GrafiaTheme,
    GuideThickness, KerningDelta, MetricsAxis, Node, NodeId, NodeType, RectUpm, ReferenceBandLine,
    ScreenPoint, SelectionInfo, SelectionInfoAction, SnapTargetKind, SnapTargetMask, SpacingDelta,
    WritingDirection,
};
use image::{DynamicImage, ImageBuffer, Rgba};
use std::collections::HashMap;
use visual_pixel_diff_tests::{compute_ssim, SSIM_DEFAULT_WINDOW_SIZE};

pub(crate) fn canvas_rect(w: f32, h: f32) -> Rect {
    Rect::from_min_size(Pos2::ZERO, Vec2::new(w, h))
}

pub(crate) fn rect_contour(
    id: usize,
    base_node: u64,
    x0: f64,
    y0: f64,
    x1: f64,
    y1: f64,
) -> GlyphContour {
    GlyphContour::new(
        ContourId(id),
        vec![
            Node::new(NodeId(base_node), x0, y0, NodeType::Corner),
            Node::new(NodeId(base_node + 1), x1, y0, NodeType::Corner),
            Node::new(NodeId(base_node + 2), x1, y1, NodeType::Corner),
            Node::new(NodeId(base_node + 3), x0, y1, NodeType::Corner),
        ],
    )
}

pub(crate) fn dispatch_events(
    state: &mut GrafiaCanvasState,
    rect: Rect,
    events: Vec<Event>,
    shift: bool,
    alt: bool,
    cmd: bool,
    ctrl: bool,
) -> bool {
    let ctx = egui::Context::default();
    ctx.tessellation_options_mut(|o| o.feathering = false);
    let m = EguiModifiers {
        alt,
        ctrl,
        shift,
        mac_cmd: cmd,
        command: cmd || ctrl,
    };
    let raw = RawInput {
        screen_rect: Some(rect),
        time: Some(1.0),
        events,
        ..Default::default()
    };
    ctx.input_mut(|i| i.modifiers = m);
    let theme = state.theme.clone();
    let mut changed = false;
    let mut out = ctx.run_ui(raw, |ui| {
        let ev = ui.input(|i| i.events.clone());
        let mut disp = InputDispatcher::new(state, &theme, rect);
        changed = disp.dispatch_events(&ev, ui);
    });
    out.textures_delta.clear();
    changed
}

pub(crate) fn send_key(
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
    let _ = dispatch_events(state, rect, vec![ev], shift, alt, cmd, ctrl);
}

pub(crate) fn pointer_press(
    state: &mut GrafiaCanvasState,
    rect: Rect,
    scr: ScreenPoint,
    shift: bool,
    alt: bool,
    cmd: bool,
    ctrl: bool,
) {
    let pos = Pos2::new(scr.x.0, scr.y.0);
    let m = EguiModifiers {
        alt,
        ctrl,
        shift,
        mac_cmd: cmd,
        command: cmd || ctrl,
    };
    let _ = dispatch_events(
        state,
        rect,
        vec![
            Event::PointerMoved(pos),
            Event::PointerButton {
                pos,
                button: PointerButton::Primary,
                pressed: true,
                modifiers: m,
            },
        ],
        shift,
        alt,
        cmd,
        ctrl,
    );
}

pub(crate) fn pointer_move(
    state: &mut GrafiaCanvasState,
    rect: Rect,
    scr: ScreenPoint,
    shift: bool,
    alt: bool,
    cmd: bool,
    ctrl: bool,
) {
    let pos = Pos2::new(scr.x.0, scr.y.0);
    let _ = dispatch_events(
        state,
        rect,
        vec![Event::PointerMoved(pos)],
        shift,
        alt,
        cmd,
        ctrl,
    );
}

pub(crate) fn pointer_release(
    state: &mut GrafiaCanvasState,
    rect: Rect,
    scr: ScreenPoint,
    shift: bool,
    alt: bool,
    cmd: bool,
    ctrl: bool,
) {
    let pos = Pos2::new(scr.x.0, scr.y.0);
    let m = EguiModifiers {
        alt,
        ctrl,
        shift,
        mac_cmd: cmd,
        command: cmd || ctrl,
    };
    let _ = dispatch_events(
        state,
        rect,
        vec![Event::PointerButton {
            pos,
            button: PointerButton::Primary,
            pressed: false,
            modifiers: m,
        }],
        shift,
        alt,
        cmd,
        ctrl,
    );
}

pub(crate) fn rasterize_egui_widget_canvas(
    state: &mut GrafiaCanvasState,
    width: u32,
    height: u32,
) -> DynamicImage {
    let rect = canvas_rect(width as f32, height as f32);
    let ctx = egui::Context::default();
    ctx.tessellation_options_mut(|o| o.feathering = false);
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
                let py = (sy as f32 + 0.500_17) * inv_ss;
                for sx in min_sx..=max_sx {
                    let px = (sx as f32 + 0.500_13) * inv_ss;
                    if !clip.contains(Pos2::new(px, py)) {
                        continue;
                    }
                    let w0 = ((v1.pos.x - px) * (v2.pos.y - py)
                        - (v1.pos.y - py) * (v2.pos.x - px))
                        * inv_area;
                    let w1 = ((v2.pos.x - px) * (v0.pos.y - py)
                        - (v2.pos.y - py) * (v0.pos.x - px))
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
                    if let Some((tw, th, tbuf)) = tex {
                        if *tw > 0 && *th > 0 {
                            let u = (w0 * v0.uv.x + w1 * v1.uv.x + w2 * v2.uv.x).clamp(0.0, 1.0);
                            let v = (w0 * v0.uv.y + w1 * v1.uv.y + w2 * v2.uv.y).clamp(0.0, 1.0);
                            let tx = ((u * (*tw as f32)).floor() as usize).min(*tw - 1);
                            let ty = ((v * (*th as f32)).floor() as usize).min(*th - 1);
                            let tp = tbuf[ty * *tw + tx];
                            sr *= f32::from(tp[0]) / 255.0;
                            sg *= f32::from(tp[1]) / 255.0;
                            sb *= f32::from(tp[2]) / 255.0;
                            sa *= f32::from(tp[3]) / 255.0;
                        }
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
    let scale = 1.0 / ((ss * ss) as f32);
    for y in 0..height {
        for x in 0..width {
            let mut acc = [0.0_f32; 3];
            for dy in 0..ss {
                for dx in 0..ss {
                    let s = fb[((y * ss + dy) * ss_w + (x * ss + dx)) as usize];
                    acc[0] += s[0];
                    acc[1] += s[1];
                    acc[2] += s[2];
                }
            }
            img.put_pixel(
                x,
                y,
                Rgba([
                    (acc[0] * scale * 255.0).round().clamp(0.0, 255.0) as u8,
                    (acc[1] * scale * 255.0).round().clamp(0.0, 255.0) as u8,
                    (acc[2] * scale * 255.0).round().clamp(0.0, 255.0) as u8,
                    255,
                ]),
            );
        }
    }
    DynamicImage::ImageRgba8(img)
}

// ============================================================================
// TC-V090-001: Multi-Sublayer Solid, LinearGradient, ClippingMask & SrcOver Compositing
// ============================================================================
#[test]
fn tc_v090_001_multi_sublayer_solid_linear_gradient_clipping_mask_and_src_over() {
    let mut state = GrafiaCanvasState::new(GrafiaTheme::sonoma_default());
    state.viewport.zoom = 1.0;
    state.viewport.pan_offset = ScreenPoint::new(80.0, 420.0);
    state.show_metrics = false;
    state.show_grid = false;
    state.show_guides = false;
    state.show_selection_info_hud = false;

    // Sublayer 0: ClippingMask restricting painting to x in [100, 320], y in [80, 300]
    let clip_sublayer = CanvasColorSublayer {
        id: 1,
        name: "Mask".into(),
        contours: vec![rect_contour(1, 10, 100.0, 80.0, 320.0, 300.0)],
        paint: ColorLayerPaint::ClippingMask,
        opacity: 1.0,
        visible: true,
        locked: false,
    };

    // Sublayer 1: LinearGradient spanning x in [40, 380], y in [40, 340] (wider than mask!)
    let grad_sublayer = CanvasColorSublayer {
        id: 2,
        name: "BodyGradient".into(),
        contours: vec![rect_contour(2, 20, 40.0, 40.0, 380.0, 340.0)],
        paint: ColorLayerPaint::LinearGradient {
            p0: FontPoint::new(100.0, 80.0),
            p1: FontPoint::new(320.0, 300.0),
            stops: vec![
                GradientStop {
                    offset: 0.0,
                    rgba: [230, 50, 60, 255],
                },
                GradientStop {
                    offset: 1.0,
                    rgba: [40, 120, 240, 255],
                },
            ],
            spread: GradientSpread::Pad,
        },
        opacity: 1.0,
        visible: true,
        locked: false,
    };

    // Sublayer 2: Translucent Solid highlight inside mask x in [150, 270], y in [130, 250]
    let solid_sublayer = CanvasColorSublayer {
        id: 3,
        name: "Highlight".into(),
        contours: vec![rect_contour(3, 30, 150.0, 130.0, 270.0, 250.0)],
        paint: ColorLayerPaint::Solid {
            rgba: [250, 220, 40, 180],
        },
        opacity: 0.85,
        visible: true,
        locked: false,
    };

    state.set_color_sublayers(vec![clip_sublayer, grad_sublayer, solid_sublayer]);
    assert_eq!(state.color_sublayers.len(), 3);
    assert_eq!(GradientExtend::Pad, GradientSpread::Pad);

    let ts_pix = rasterize_canvas_subregion(&state, &state.theme, 520, 460);
    let ts_img = DynamicImage::ImageRgba8(pixmap_to_rgba_image(ts_pix));
    let egui_img = rasterize_egui_widget_canvas(&mut state, 520, 460);

    // 1. Pixels inside grad_sublayer (x=60, y=200) but OUTSIDE ClippingMask [100..320] must stay background!
    let bg = state
        .theme
        .canvas_background_color32()
        .to_srgba_unmultiplied();
    let out_scr = state.viewport.to_screen(FontPoint::new(60.0, 200.0));
    let out_px = ts_img
        .to_rgba8()
        .get_pixel(out_scr.x.0.round() as u32, out_scr.y.0.round() as u32)
        .0;
    assert_eq!(
        [out_px[0], out_px[1], out_px[2]],
        [bg[0], bg[1], bg[2]],
        "Pixel outside ClippingMask must remain untouched background color"
    );

    // 2. Pixels inside ClippingMask & LinearGradient (x=120, y=100) and inside Solid highlight (x=210, y=190) must be painted
    let in_grad_scr = state.viewport.to_screen(FontPoint::new(120.0, 100.0));
    let in_grad_px = ts_img
        .to_rgba8()
        .get_pixel(
            in_grad_scr.x.0.round() as u32,
            in_grad_scr.y.0.round() as u32,
        )
        .0;
    assert_ne!(
        [in_grad_px[0], in_grad_px[1], in_grad_px[2]],
        [bg[0], bg[1], bg[2]],
        "Pixel inside ClippingMask must render LinearGradient"
    );
    let in_hi_scr = state.viewport.to_screen(FontPoint::new(210.0, 190.0));
    let in_hi_px = ts_img
        .to_rgba8()
        .get_pixel(in_hi_scr.x.0.round() as u32, in_hi_scr.y.0.round() as u32)
        .0;
    assert_ne!(
        in_hi_px, in_grad_px,
        "Porter-Duff SrcOver highlight sublayer must composite over gradient sublayer"
    );

    // 3. Real egui GrafiaCanvasWidget vs tiny-skia SSIM >= 0.985
    let ssim = compute_ssim(&egui_img, &ts_img, SSIM_DEFAULT_WINDOW_SIZE);
    assert!(
        ssim >= 0.985,
        "TC-V090-001 egui vs tiny-skia color layer compositing SSIM must be >= 0.985, got {ssim:.6}"
    );
    assert!(state.validate_invariants().is_ok());
}

// ============================================================================
// TC-V090-002: RadialGradient, SweepGradient & Bitmap Sublayer Handles + GradientStop Drag/Mutation
// ============================================================================
#[test]
fn tc_v090_002_radial_sweep_bitmap_sublayers_and_gradient_stop_mutation_undo() {
    let rect = canvas_rect(640.0, 480.0);
    let mut state = GrafiaCanvasState::new(GrafiaTheme::sonoma_default());
    state.viewport.zoom = 1.0;
    state.viewport.pan_offset = ScreenPoint::new(100.0, 400.0);
    state.show_metrics = false;
    state.show_grid = false;
    state.show_selection_info_hud = false;

    let radial_sublayer = CanvasColorSublayer {
        id: 10,
        name: "Radial".into(),
        contours: vec![rect_contour(1, 10, 40.0, 60.0, 260.0, 280.0)],
        paint: ColorLayerPaint::RadialGradient {
            c0: FontPoint::new(100.0, 180.0),
            r0: 10.0,
            c1: FontPoint::new(100.0, 180.0),
            r1: 120.0,
            stops: vec![
                GradientStop {
                    offset: 0.0,
                    rgba: [255, 80, 40, 255],
                },
                GradientStop {
                    offset: 0.35,
                    rgba: [80, 220, 120, 255],
                },
                GradientStop {
                    offset: 1.0,
                    rgba: [40, 90, 250, 255],
                },
            ],
            spread: GradientSpread::Reflect,
        },
        opacity: 1.0,
        visible: true,
        locked: false,
    };

    let sweep_sublayer = CanvasColorSublayer {
        id: 20,
        name: "Sweep".into(),
        contours: vec![rect_contour(2, 20, 280.0, 60.0, 440.0, 220.0)],
        paint: ColorLayerPaint::SweepGradient {
            center: FontPoint::new(360.0, 140.0),
            start_deg: 0.0,
            end_deg: 270.0,
            stops: vec![
                GradientStop {
                    offset: 0.0,
                    rgba: [240, 60, 180, 255],
                },
                GradientStop {
                    offset: 1.0,
                    rgba: [60, 220, 240, 255],
                },
            ],
        },
        opacity: 0.9,
        visible: true,
        locked: false,
    };

    let mut bmp = CanvasBackgroundImage::new([40_u8, 180, 90, 255].repeat(8 * 8), 8, 8);
    bmp.position = FontPoint::new(280.0, 240.0);
    bmp.scale_x = 10.0;
    bmp.scale_y = 10.0;
    let bitmap_sublayer = CanvasColorSublayer {
        id: 30,
        name: "Strike".into(),
        contours: vec![rect_contour(3, 30, 280.0, 240.0, 360.0, 320.0)],
        paint: ColorLayerPaint::Bitmap {
            image: bmp,
            ppem_strike: Some(32),
        },
        opacity: 1.0,
        visible: true,
        locked: false,
    };

    state.set_color_sublayers(vec![radial_sublayer, sweep_sublayer, bitmap_sublayer]);
    state.select_color_sublayer(Some(0));
    state.select_gradient_stop(Some((0, 1)));

    // 1. SelectionInfo::GradientStop telemetry for stop index 1 (offset 0.35 -> 35.0%)
    match state.selection_info() {
        SelectionInfo::GradientStop {
            sublayer_index,
            stop_index,
            offset_pct,
            rgba,
        } => {
            assert_eq!(sublayer_index, 0);
            assert_eq!(stop_index, 1);
            assert_relative_eq!(offset_pct, 35.0, epsilon = 1e-4);
            assert_eq!(rgba, [80, 220, 120, 255]);
        }
        other => panic!("Expected SelectionInfo::GradientStop, got {other:?}"),
    }

    // 2. Drag intermediate stop tick along radial ray (100 + 0.35 * 120 = 142, 180) -> (100 + 0.65 * 120 = 178, 180)
    let _ = state.drain_host_requests();
    let stop_start_scr = state.viewport.to_screen(FontPoint::new(142.0, 180.0));
    let stop_end_scr = state.viewport.to_screen(FontPoint::new(178.0, 180.0));
    pointer_press(&mut state, rect, stop_start_scr, false, false, false, false);
    pointer_move(&mut state, rect, stop_end_scr, false, false, false, false);
    pointer_release(&mut state, rect, stop_end_scr, false, false, false, false);

    match &state.color_sublayers[0].paint {
        ColorLayerPaint::RadialGradient { stops, .. } => {
            assert_relative_eq!(stops[1].offset, 0.65, epsilon = 1e-3);
        }
        other => panic!("Expected RadialGradient, got {other:?}"),
    }
    assert_eq!(
        state.history.last_undo_label(),
        Some("Change Color Gradient")
    );
    let reqs = state.drain_host_requests();
    assert!(reqs.contains(&CanvasHostRequest::MutateColorSublayer {
        sublayer_index: 0,
        mutation: ColorSublayerMutation::UpdatePaint(state.color_sublayers[0].paint.clone()),
    }));

    // Undo restores stop offset 0.35
    assert!(state.undo());
    match &state.color_sublayers[0].paint {
        ColorLayerPaint::RadialGradient { stops, .. } => {
            assert_relative_eq!(stops[1].offset, 0.35, epsilon = 1e-4);
        }
        other => panic!("Expected RadialGradient, got {other:?}"),
    }

    // 3. SelectionInfoAction::MutateGradientStop clamps offset_pct to [0, 100] and records Undo
    assert!(
        state.apply_selection_info_action(SelectionInfoAction::MutateGradientStop {
            offset_pct: 65.0,
            rgba: [90, 240, 130, 255],
        })
    );
    match state.selection_info() {
        SelectionInfo::GradientStop {
            offset_pct, rgba, ..
        } => {
            assert_relative_eq!(offset_pct, 65.0, epsilon = 1e-4);
            assert_eq!(rgba, [90, 240, 130, 255]);
        }
        other => panic!("Expected updated SelectionInfo::GradientStop, got {other:?}"),
    }
    assert_eq!(
        state.history.last_undo_label(),
        Some("Change Color Gradient")
    );
    assert!(state.undo());
    match state.selection_info() {
        SelectionInfo::GradientStop {
            offset_pct, rgba, ..
        } => {
            assert_relative_eq!(offset_pct, 35.0, epsilon = 1e-4);
            assert_eq!(rgba, [80, 220, 120, 255]);
        }
        other => panic!("Expected restored SelectionInfo::GradientStop, got {other:?}"),
    }
    assert!(state.validate_invariants().is_ok());
}

// ============================================================================
// TC-V090-003: render_colored_thumbnail Multi-Layer Projection (Light & Dark + Fallback)
// ============================================================================
#[test]
fn tc_v090_003_render_colored_thumbnail_light_dark_and_monochrome_fallback() {
    let mut state = GrafiaCanvasState::init_glyph_a();

    // 1. Monochrome fallback when color_sublayers is empty
    let mono_light = render_colored_thumbnail(&state, 64, 64, AppearanceMode::Light);
    let mono_dark = render_colored_thumbnail(&state, 64, 64, AppearanceMode::Dark);
    assert_eq!(mono_light.len(), 64 * 64 * 4);
    assert_eq!(mono_dark.len(), 64 * 64 * 4);
    assert_ne!(mono_light, mono_dark);

    // 2. 4-sublayer color glyph stack
    state.set_color_sublayers(vec![
        CanvasColorSublayer {
            id: 1,
            name: "Mask".into(),
            contours: vec![rect_contour(1, 10, 50.0, 0.0, 550.0, 700.0)],
            paint: ColorLayerPaint::ClippingMask,
            opacity: 1.0,
            visible: true,
            locked: false,
        },
        CanvasColorSublayer {
            id: 2,
            name: "Base".into(),
            contours: vec![rect_contour(2, 20, 50.0, 0.0, 550.0, 700.0)],
            paint: ColorLayerPaint::Solid {
                rgba: [230, 45, 55, 255],
            },
            opacity: 1.0,
            visible: true,
            locked: false,
        },
        CanvasColorSublayer {
            id: 3,
            name: "Linear".into(),
            contours: vec![rect_contour(3, 30, 120.0, 100.0, 480.0, 600.0)],
            paint: ColorLayerPaint::LinearGradient {
                p0: FontPoint::new(120.0, 100.0),
                p1: FontPoint::new(480.0, 600.0),
                stops: vec![
                    GradientStop {
                        offset: 0.0,
                        rgba: [30, 200, 110, 255],
                    },
                    GradientStop {
                        offset: 1.0,
                        rgba: [40, 110, 250, 255],
                    },
                ],
                spread: GradientSpread::Pad,
            },
            opacity: 0.9,
            visible: true,
            locked: false,
        },
        CanvasColorSublayer {
            id: 4,
            name: "Sweep".into(),
            contours: vec![rect_contour(4, 40, 200.0, 220.0, 400.0, 480.0)],
            paint: ColorLayerPaint::SweepGradient {
                center: FontPoint::new(300.0, 350.0),
                start_deg: 0.0,
                end_deg: 360.0,
                stops: vec![
                    GradientStop {
                        offset: 0.0,
                        rgba: [255, 220, 40, 255],
                    },
                    GradientStop {
                        offset: 1.0,
                        rgba: [255, 90, 200, 255],
                    },
                ],
            },
            opacity: 0.85,
            visible: true,
            locked: false,
        },
    ]);

    let col_light = render_colored_thumbnail(&state, 64, 64, AppearanceMode::Light);
    let col_dark = render_colored_thumbnail(&state, 64, 64, AppearanceMode::Dark);
    assert_eq!(col_light.len(), 64 * 64 * 4);
    assert_eq!(col_dark.len(), 64 * 64 * 4);
    assert_ne!(col_light, mono_light);
    assert_ne!(col_light, col_dark);
    assert!(col_light
        .as_chunks::<4>()
        .0
        .iter()
        .any(|px| px[0] > 180 && px[1] < 100));
}

// ============================================================================
// TC-V090-004: Vertical WritingDirection::TopToBottom Caret & GlyphSlot Advancement
// ============================================================================
#[test]
fn tc_v090_004_vertical_top_to_bottom_caret_and_slot_advancement() {
    let mut state = GrafiaCanvasState::init_glyph_a();
    state.viewport.zoom = 0.25;
    state.viewport.pan_offset = ScreenPoint::new(400.0, 120.0);

    let make_v_slot = |name: &str, kern: i32| -> GlyphSlot {
        let mut g = Glyph::new(name, 1000);
        g.width = 1000.0;
        g.height = 1000.0;
        g.contours
            .push(rect_contour(0, 1, 120.0, 80.0, 880.0, 780.0));
        let mut slot = GlyphSlot::new(g, kern);
        slot.advance_height = Some(1000.0);
        slot.vert_origin_y = Some(880.0);
        slot
    };

    state.set_writing_direction(WritingDirection::TopToBottom);
    state.set_text_stream_slots(
        vec![
            make_v_slot("uni4E00", 0),
            make_v_slot("uni4E8C", 0),
            make_v_slot("uni4E09", -50),
        ],
        0,
    );
    state.activate_tool(CanvasTool::Text);

    // 1. Vertical center axis x_c = W_i / 2 = 500, origin_x_upm = 0.0, cumulative downward origin_y_upm:
    assert_eq!(
        state.text_stream.writing_direction,
        WritingDirection::TopToBottom
    );
    assert_relative_eq!(
        state.text_stream.slots[0].vertical_center_x_upm(),
        500.0,
        epsilon = 1e-6
    );
    assert_relative_eq!(state.text_stream.slots[0].origin_y_upm, 0.0, epsilon = 1e-6);
    assert_relative_eq!(
        state.text_stream.slots[1].origin_y_upm,
        -1000.0,
        epsilon = 1e-6
    );
    assert_relative_eq!(
        state.text_stream.slots[2].origin_y_upm,
        -1950.0,
        epsilon = 1e-6
    );

    // 2. ArrowDown / ArrowUp steps caret vertically across slots
    state.set_caret_index(0);
    send_key(&mut state, Key::ArrowDown, false, false, false, false);
    assert_eq!(state.text_stream.caret_index, 1);
    assert_eq!(state.text_stream.active_slot_index, 1);

    send_key(&mut state, Key::ArrowDown, false, false, false, false);
    assert_eq!(state.text_stream.caret_index, 2);
    assert_eq!(state.text_stream.active_slot_index, 2);

    send_key(&mut state, Key::ArrowUp, false, false, false, false);
    assert_eq!(state.text_stream.caret_index, 1);
    assert_eq!(state.text_stream.active_slot_index, 1);

    // 3. Double-clicking slot 2 in vertical layout switches active slot & enters Select tool
    let slot2_center_upm = FontPoint::new(500.0, -1550.0);
    assert!(state.handle_double_click_upm(slot2_center_upm));
    assert_eq!(state.text_stream.active_slot_index, 2);
    assert_eq!(state.glyph.name, "uni4E09");
    assert_eq!(state.active_tool, CanvasTool::Select);
    assert_relative_eq!(state.active_slot_origin_y_upm(), -1950.0, epsilon = 1e-6);
    assert!(state.validate_invariants().is_ok());
}

// ============================================================================
// TC-V090-005: Vertical TSB / BSB & Vertical Kerning Shortcuts
// ============================================================================
#[test]
fn tc_v090_005_vertical_tsb_bsb_and_kerning_shortcuts_with_undo() {
    let mut state = GrafiaCanvasState::init_glyph_a();
    let make_v_slot = |name: &str, kern: i32| -> GlyphSlot {
        let mut g = Glyph::new(name, 1000);
        g.width = 1000.0;
        g.height = 1000.0;
        g.contours
            .push(rect_contour(0, 1, 100.0, 100.0, 900.0, 780.0));
        let mut slot = GlyphSlot::new(g, kern);
        slot.advance_height = Some(1000.0);
        slot.vert_origin_y = Some(880.0);
        slot
    };

    state.set_writing_direction(WritingDirection::TopToBottom);
    state.set_text_stream_slots(
        vec![
            make_v_slot("V0", 0),
            make_v_slot("V1", -20),
            make_v_slot("V2", 0),
        ],
        1,
    );
    state.activate_tool(CanvasTool::Text);
    let _ = state.drain_spacing_deltas();
    let _ = state.drain_kerning_deltas();

    // Initial TSB = vert_origin_y (880) - max_y (780) = 100; BSB = min_y (100) - (880 - 1000) = 220
    assert_relative_eq!(state.active_glyph_tsb(), 100.0, epsilon = 1e-6);
    assert_relative_eq!(state.active_glyph_bsb(), 220.0, epsilon = 1e-6);

    // 1. '[' / ']' and '{' / '}' or Ctrl/Cmd + ArrowUp/Down adjust TSB / BSB on MetricsAxis::Vertical
    send_key(&mut state, Key::OpenBracket, false, false, false, false);
    assert_relative_eq!(state.active_glyph_tsb(), 99.0, epsilon = 1e-6);
    send_key(&mut state, Key::CloseBracket, false, false, false, false);
    assert_relative_eq!(state.active_glyph_tsb(), 100.0, epsilon = 1e-6);

    // Shift + CloseBracket ('}') adjusts BSB by +10 UPM
    send_key(&mut state, Key::CloseBracket, true, false, false, false);
    assert_relative_eq!(state.active_glyph_bsb(), 230.0, epsilon = 1e-6);
    assert_eq!(
        state.history.last_undo_label(),
        Some("Change Vertical Metrics")
    );

    let sp_deltas: Vec<SpacingDelta> = state.drain_spacing_deltas();
    assert!(!sp_deltas.is_empty());
    let last_sp = sp_deltas.last().expect("spacing delta");
    assert_eq!(last_sp.axis, MetricsAxis::Vertical);
    assert_eq!(last_sp.bsb_delta_upm, 10);
    assert_eq!(last_sp.advance_height_delta_upm, 10);

    // Undo restores BSB to 220.0
    assert!(state.undo());
    assert_relative_eq!(state.active_glyph_bsb(), 220.0, epsilon = 1e-6);

    // 2. Vertical kerning shortcut (Alt + Ctrl + ArrowDown / ArrowUp) emits KerningDelta with MetricsAxis::Vertical
    send_key(&mut state, Key::ArrowDown, true, true, false, true);
    let kern_deltas: Vec<KerningDelta> = state.drain_kerning_deltas();
    assert_eq!(kern_deltas.len(), 1);
    assert_eq!(kern_deltas[0].axis, MetricsAxis::Vertical);
    assert_eq!(kern_deltas[0].left_slot_index, 0);
    assert_eq!(kern_deltas[0].right_slot_index, 1);
    assert_eq!(kern_deltas[0].delta_upm, 10);
    assert_eq!(kern_deltas[0].new_kern_upm, -10);
    assert_eq!(
        state.history.last_undo_label(),
        Some("Change Vertical Metrics")
    );
    assert!(state.undo());
    assert_eq!(state.text_stream.slots[1].kern_with_prev, -20);

    // 3. Verify LeftToRight emits MetricsAxis::Horizontal
    state.set_writing_direction(WritingDirection::LeftToRight);
    send_key(&mut state, Key::ArrowRight, false, false, false, true);
    let h_sp = state.drain_spacing_deltas();
    assert_eq!(h_sp.last().map(|d| d.axis), Some(MetricsAxis::Horizontal));
    assert!(state.validate_invariants().is_ok());
}

// ============================================================================
// TC-V090-006: CustomReferenceFrame Outer/Inner Box, 3x3 Subdivisions & SnapEngine Integration
// ============================================================================
#[test]
fn tc_v090_006_custom_reference_frame_boxes_subdivisions_and_snapping() {
    let mut state = GrafiaCanvasState::new(GrafiaTheme::sonoma_default());
    state.viewport.zoom = 1.0;
    state.viewport.pan_offset = ScreenPoint::new(80.0, 500.0);
    state.show_metrics = false;
    state.show_grid = false;
    state.show_guides = false;

    let frame = CustomReferenceFrame {
        outer_box: Some(RectUpm::new(0.0, -120.0, 1000.0, 880.0)),
        inner_box: Some(RectUpm::new(60.0, -60.0, 940.0, 820.0)),
        inner_grid_subdivisions: Some((3, 3)),
        center_crosshair: true,
        keyline_circles_radii_upm: Vec::new(),
        parallel_lines: Vec::new(),
        ..Default::default()
    };

    let pix_without = rasterize_canvas_subregion(&state, &state.theme, 600, 480);
    state.set_custom_reference_frame(Some(frame));
    let pix_with = rasterize_canvas_subregion(&state, &state.theme, 600, 480);
    assert_ne!(
        pix_without.data(),
        pix_with.data(),
        "CustomReferenceFrame outer/inner boxes & 3x3 lattice must render in Pass 1"
    );

    // Inner box spans x in [60, 940] (width 880 -> 1/3 step = 293.333333, x_1 = 353.333333),
    // y in [-60, 820] (height 880 -> 1/3 step = 293.333333, y_1 = 233.333333).
    // Query near inner_box left edge (60.0) and subdivision y_1 (233.333333):
    let sub_x1 = 60.0 + 880.0 / 3.0;
    let sub_y1 = -60.0 + 880.0 / 3.0;
    let snap_res =
        state.evaluate_magnetic_snap(FontPoint::new(sub_x1 + 2.0, sub_y1 - 1.5), &[], false);
    assert_relative_eq!(snap_res.snapped_point.x.0, sub_x1, epsilon = 1e-4);
    assert_relative_eq!(snap_res.snapped_point.y.0, sub_y1, epsilon = 1e-4);
    assert!(snap_res
        .active_snappers
        .iter()
        .any(|s| s.target_kind == Some(SnapTargetKind::ReferenceFrame)));

    // Disabling SnapTargetMask::reference_frame bypasses reference frame snapping
    let mut prefs = state.preferences.clone();
    prefs.snap_targets = SnapTargetMask {
        reference_frame: false,
        ..SnapTargetMask::all()
    };
    state.apply_preferences(&prefs);
    let unsnapped =
        state.evaluate_magnetic_snap(FontPoint::new(sub_x1 + 2.0, sub_y1 - 1.5), &[], false);
    assert_relative_eq!(unsnapped.snapped_point.x.0, sub_x1 + 2.0, epsilon = 1e-4);
    assert_relative_eq!(unsnapped.snapped_point.y.0, sub_y1 - 1.5, epsilon = 1e-4);
    assert!(state.validate_invariants().is_ok());
}

// ============================================================================
// TC-V090-007: CustomReferenceFrame Parallel FontUnits Bands & Keyline Circles (SSIM >= 0.985)
// ============================================================================
#[test]
fn tc_v090_007_custom_reference_frame_parallel_bands_and_keyline_circles() {
    let mut state = GrafiaCanvasState::new(GrafiaTheme::sonoma_default());
    state.viewport.zoom = 0.35;
    state.viewport.pan_offset = ScreenPoint::new(80.0, 480.0);
    state.show_metrics = false;
    state.show_grid = false;
    state.show_guides = false;
    state.show_selection_info_hud = false;

    let mut parallel_lines = Vec::new();
    for y in [0.0, 250.0, 500.0, 750.0, 1000.0] {
        parallel_lines.push(ReferenceBandLine {
            y_or_x_upm: y,
            horizontal: true,
            thickness: GuideThickness::FontUnits(32.5),
            dashed: false,
            snap_enabled: true,
        });
    }
    for y in [-250.0, 1250.0] {
        parallel_lines.push(ReferenceBandLine {
            y_or_x_upm: y,
            horizontal: true,
            thickness: GuideThickness::ScreenDp,
            dashed: true,
            snap_enabled: true,
        });
    }

    let frame = CustomReferenceFrame {
        outer_box: Some(RectUpm::new(0.0, 0.0, 1000.0, 1000.0)),
        inner_box: None,
        inner_grid_subdivisions: None,
        center_crosshair: false,
        keyline_circles_radii_upm: vec![150.0, 280.0],
        parallel_lines,
        ..Default::default()
    };
    state.set_custom_reference_frame(Some(frame));

    // 1. SnapEngine snaps to band centerline (y = 500.0) and +-16.25 UPM edges (516.25 & 483.75),
    // as well as keyline circle cardinal extrema (center (500, 500) + radius 280 -> x = 780.0)
    state.viewport.zoom = 1.0;
    let snap_center = state.evaluate_magnetic_snap(FontPoint::new(300.0, 501.5), &[], false);
    assert_relative_eq!(snap_center.snapped_point.y.0, 500.0, epsilon = 1e-5);

    let snap_upper_edge = state.evaluate_magnetic_snap(FontPoint::new(300.0, 517.5), &[], false);
    assert_relative_eq!(snap_upper_edge.snapped_point.y.0, 516.25, epsilon = 1e-5);

    let snap_lower_edge = state.evaluate_magnetic_snap(FontPoint::new(300.0, 482.5), &[], false);
    assert_relative_eq!(snap_lower_edge.snapped_point.y.0, 483.75, epsilon = 1e-5);

    let snap_circle = state.evaluate_magnetic_snap(FontPoint::new(781.5, 500.0), &[], false);
    assert_relative_eq!(snap_circle.snapped_point.x.0, 780.0, epsilon = 1e-5);

    // 2. Real egui GrafiaCanvasWidget vs tiny-skia SSIM >= 0.985
    state.viewport.zoom = 0.35;
    state.viewport.pan_offset = ScreenPoint::new(80.0, 480.0);
    let ts_pix = rasterize_canvas_subregion(&state, &state.theme, 560, 520);
    let ts_img = DynamicImage::ImageRgba8(pixmap_to_rgba_image(ts_pix));
    let egui_img = rasterize_egui_widget_canvas(&mut state, 560, 520);

    let ssim = compute_ssim(&egui_img, &ts_img, SSIM_DEFAULT_WINDOW_SIZE);
    assert!(
        ssim >= 0.985,
        "TC-V090-007 egui vs tiny-skia CustomReferenceFrame SSIM must be >= 0.985, got {ssim:.6}"
    );
    assert!(state.validate_invariants().is_ok());
}

mod v090_hinting_and_hoi;
