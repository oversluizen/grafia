#![allow(
    clippy::too_many_lines,
    clippy::cognitive_complexity,
    clippy::expect_used,
    clippy::unwrap_used,
    clippy::panic,
    clippy::too_many_arguments,
    clippy::iter_over_hash_type
)]
//! Normative COTS Verification Suite for `SPEC-009` (`grafia v0.8.0`):
//! - `TC-V080-001`: Hobby (`NodeType::HobbyStar`) & Euler/Spiro (`NodeType::EulerSpiro`) Closed Cycle Symmetry
//! - `TC-V080-002`: Hobby Directional Constraints & `convert_selected_hobby_nodes_to_cubic` (`SSIM >= 0.999` + Undo)
//! - `TC-V080-003`: 8+4 BBox Outer-Corner Rotation (`[6.0 dp, 18.0 dp]`), `Shift` `15°` Snap & Live `"∠ ..."` Pill
//! - `TC-V080-004`: 8+4 BBox Mid-Edge `Ctrl+Alt` Shear & Smooth-Node Collinearity
//! - `TC-V080-005`: `CanvasSmartHandle` Angled Ray Projection, Clamping, `4.0 dp` Default Snap & Host Event
//! - `TC-V080-006`: Self-Intersection Virtual `ExtraNode` Translation Stability & Click Host Event
//! - `TC-V080-015`: `SelectionInfo::HobbyNode`, `SelectionInfo::BackgroundImage` & `interpolate_with_background(0.5)`
//! - `TC-V080-016`: Curve Conversions, `apply_path_op_across_all_layers`, Viewport Bookmark & Node Visibility Parity

#[path = "visual_pixel_diff_tests.rs"]
#[allow(unused_imports)]
pub mod visual_pixel_diff_tests;

use approx::assert_relative_eq;
use egui::{Event, Modifiers as EguiModifiers, PointerButton, Pos2, RawInput, Rect, Vec2};
use grafia::adapters::headless::{pixmap_to_rgba_image, rasterize_canvas_subregion};
use grafia::adapters::input_dispatcher::InputDispatcher;
use grafia::adapters::CanvasPainter;
use grafia::{
    compute_layer_extra_nodes, contour_signed_area, contour_to_bezpath_with_splines,
    evaluate_euler_spiro_chain_to_cubics, evaluate_hobby_chain_to_cubics, Affine2D, BBoxAction,
    BackgroundImageMutation, CanvasBackgroundImage, CanvasComponent, CanvasHostRequest,
    CanvasSmartHandle, CompanionMasterLayer, ComponentId, ContourId, FontPoint, GlyphContour,
    GrafiaCanvasState, GrafiaTheme, HobbyDirConstraint, LayerPathOp, Node, NodeId, NodeType,
    ScreenPoint, SelectionInfo, SelectionInfoAction, TransformGridAnchor,
};
use image::{DynamicImage, ImageBuffer, Rgba};
use kurbo::{CubicBez, ParamCurve, ParamCurveDeriv, ParamCurveNearest, PathEl, Point};
use std::collections::HashMap;
use visual_pixel_diff_tests::{compute_ssim, SSIM_DEFAULT_WINDOW_SIZE};

fn canvas_rect(w: f32, h: f32) -> Rect {
    Rect::from_min_size(Pos2::ZERO, Vec2::new(w, h))
}

fn mods(shift: bool, alt: bool, cmd: bool, ctrl: bool) -> EguiModifiers {
    EguiModifiers {
        alt,
        ctrl: ctrl || cmd,
        shift,
        mac_cmd: cmd,
        command: cmd,
    }
}

fn dispatch_events(
    state: &mut GrafiaCanvasState,
    rect: Rect,
    events: Vec<Event>,
    shift: bool,
    alt: bool,
    cmd: bool,
    ctrl: bool,
) {
    let ctx = egui::Context::default();
    let m = mods(shift, alt, cmd, ctrl);
    let raw = RawInput {
        screen_rect: Some(rect),
        time: Some(1.0),
        events,
        ..Default::default()
    };
    ctx.input_mut(|i| i.modifiers = m);
    let theme = state.theme.clone();
    let mut out = ctx.run_ui(raw, |ui| {
        let ev = ui.input(|i| i.events.clone());
        let mut disp = InputDispatcher::new(state, &theme, rect);
        let _ = disp.dispatch_events(&ev, ui);
    });
    out.textures_delta.clear();
}

fn pointer_press(
    state: &mut GrafiaCanvasState,
    rect: Rect,
    scr: ScreenPoint,
    shift: bool,
    alt: bool,
    cmd: bool,
    ctrl: bool,
) {
    let pos = Pos2::new(scr.x.0, scr.y.0);
    let m = mods(shift, alt, cmd, ctrl);
    let evs = vec![
        Event::PointerMoved(pos),
        Event::PointerButton {
            pos,
            button: PointerButton::Primary,
            pressed: true,
            modifiers: m,
        },
    ];
    dispatch_events(state, rect, evs, shift, alt, cmd, ctrl);
}

fn pointer_move(
    state: &mut GrafiaCanvasState,
    rect: Rect,
    scr: ScreenPoint,
    shift: bool,
    alt: bool,
    cmd: bool,
    ctrl: bool,
) {
    let pos = Pos2::new(scr.x.0, scr.y.0);
    dispatch_events(
        state,
        rect,
        vec![Event::PointerMoved(pos)],
        shift,
        alt,
        cmd,
        ctrl,
    );
}

fn pointer_release(
    state: &mut GrafiaCanvasState,
    rect: Rect,
    scr: ScreenPoint,
    shift: bool,
    alt: bool,
    cmd: bool,
    ctrl: bool,
) {
    let pos = Pos2::new(scr.x.0, scr.y.0);
    let m = mods(shift, alt, cmd, ctrl);
    let ev = Event::PointerButton {
        pos,
        button: PointerButton::Primary,
        pressed: false,
        modifiers: m,
    };
    dispatch_events(state, rect, vec![ev], shift, alt, cmd, ctrl);
}

fn set_contours(state: &mut GrafiaCanvasState, contours: Vec<GlyphContour>) {
    state.contours = contours.iter().map(|c| c.nodes.clone()).collect();
    state.active_contour_closed = contours.last().is_none_or(|c| c.closed);
    state.active_open_contour = contours.iter().rposition(|c| !c.closed);
    state.glyph.contours = contours;
    state.selected_nodes.clear();
    state.sync_paths_from_contours();
    state.assert_invariants();
}

fn cubic_curvature_at(c: CubicBez, t: f64) -> f64 {
    let d1 = c.deriv().eval(t);
    let d2 = c.deriv().deriv().eval(t);
    let cross = (d1.x * d2.y - d1.y * d2.x).abs();
    let speed = d1.x.hypot(d1.y);
    cross / (speed * speed * speed).max(1e-12)
}

fn rasterize_egui_canvas(state: &GrafiaCanvasState, width: u32, height: u32) -> DynamicImage {
    let rect = canvas_rect(width as f32, height as f32);
    let ctx = egui::Context::default();
    let raw = RawInput {
        screen_rect: Some(rect),
        time: Some(1.0),
        ..Default::default()
    };
    let mut full_output = ctx.run_ui(raw, |ui| {
        let painter = ui.painter_at(rect);
        CanvasPainter::new(&painter, rect, state, &state.theme).paint();
    });
    let mut textures: HashMap<egui::TextureId, (usize, usize, Vec<[u8; 4]>)> = HashMap::new();
    for (tid, deltas) in &full_output.textures_delta.set {
        for delta in deltas {
            if delta.pos.is_none() {
                let [tw, th] = delta.image.size();
                let pixels: Vec<[u8; 4]> = match &delta.image {
                    egui::ImageData::Color(img) => {
                        img.pixels.iter().map(|c| c.to_array()).collect()
                    }
                };
                textures.insert(*tid, (tw, th, pixels));
            }
        }
    }
    full_output.textures_delta.clear();
    let bg = state
        .theme
        .canvas_background_color32()
        .to_srgba_unmultiplied();
    let mut fb: Vec<[f32; 4]> = vec![
        [
            f32::from(bg[0]) / 255.0,
            f32::from(bg[1]) / 255.0,
            f32::from(bg[2]) / 255.0,
            1.0,
        ];
        (width * height) as usize
    ];
    let clipped = ctx.tessellate(full_output.shapes, 1.0);
    for prim in clipped {
        let egui::epaint::Primitive::Mesh(mesh) = prim.primitive else {
            continue;
        };
        let tex = textures.get(&mesh.texture_id);
        for tri in mesh.indices.as_chunks::<3>().0 {
            let v0 = mesh.vertices[tri[0] as usize];
            let v1 = mesh.vertices[tri[1] as usize];
            let v2 = mesh.vertices[tri[2] as usize];
            let min_x = v0.pos.x.min(v1.pos.x).min(v2.pos.x).floor().max(0.0) as i32;
            let max_x = v0
                .pos
                .x
                .max(v1.pos.x)
                .max(v2.pos.x)
                .ceil()
                .min(width as f32 - 1.0) as i32;
            let min_y = v0.pos.y.min(v1.pos.y).min(v2.pos.y).floor().max(0.0) as i32;
            let max_y = v0
                .pos
                .y
                .max(v1.pos.y)
                .max(v2.pos.y)
                .ceil()
                .min(height as f32 - 1.0) as i32;
            let area = (v1.pos.x - v0.pos.x) * (v2.pos.y - v0.pos.y)
                - (v1.pos.y - v0.pos.y) * (v2.pos.x - v0.pos.x);
            if area.abs() < 1e-6 {
                continue;
            }
            let inv_area = 1.0 / area;
            for py in min_y..=max_y {
                for px in min_x..=max_x {
                    let p = Pos2::new(px as f32 + 0.5, py as f32 + 0.5);
                    let w0 = ((v1.pos.x - p.x) * (v2.pos.y - p.y)
                        - (v1.pos.y - p.y) * (v2.pos.x - p.x))
                        * inv_area;
                    let w1 = ((v2.pos.x - p.x) * (v0.pos.y - p.y)
                        - (v2.pos.y - p.y) * (v0.pos.x - p.x))
                        * inv_area;
                    let w2 = 1.0 - w0 - w1;
                    if w0 < -1e-4 || w1 < -1e-4 || w2 < -1e-4 {
                        continue;
                    }
                    let c0 = v0.color.to_array();
                    let c1 = v1.color.to_array();
                    let c2 = v2.color.to_array();
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
                        let tx =
                            ((u * (*tw as f32 - 1.0)).round() as usize).min(tw.saturating_sub(1));
                        let ty =
                            ((v * (*th as f32 - 1.0)).round() as usize).min(th.saturating_sub(1));
                        let tc = tpx[ty * *tw + tx];
                        sr *= f32::from(tc[0]) / 255.0;
                        sg *= f32::from(tc[1]) / 255.0;
                        sb *= f32::from(tc[2]) / 255.0;
                        sa *= f32::from(tc[3]) / 255.0;
                    }
                    let dst = &mut fb[(py as u32 * width + px as u32) as usize];
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
    for (idx, px) in fb.iter().enumerate() {
        let x = (idx as u32) % width;
        let y = (idx as u32) / width;
        img.put_pixel(
            x,
            y,
            Rgba([
                (px[0] * 255.0).round() as u8,
                (px[1] * 255.0).round() as u8,
                (px[2] * 255.0).round() as u8,
                255,
            ]),
        );
    }
    DynamicImage::ImageRgba8(img)
}

// ============================================================================
// TC-V080-001: Hobby & Euler/Spiro Closed Cycle Symmetry
// ============================================================================
#[test]
fn tc_v080_001_hobby_and_euler_spiro_closed_cycle_symmetry() {
    let hobby_type = NodeType::HobbyStar {
        tension_in: 1.0,
        tension_out: 1.0,
        dir_constraint: HobbyDirConstraint::Free,
    };
    let diamond_hobby = vec![
        Node::new(NodeId(1), 100.0, 0.0, hobby_type),
        Node::new(NodeId(2), 200.0, 100.0, hobby_type),
        Node::new(NodeId(3), 100.0, 200.0, hobby_type),
        Node::new(NodeId(4), 0.0, 100.0, hobby_type),
    ];

    let solved_hobby = evaluate_hobby_chain_to_cubics(&diamond_hobby, true, 0.0, &[]);
    assert_eq!(
        solved_hobby.len(),
        4,
        "4-node closed HobbyStar diamond must solve to 4 cubic segments"
    );

    let ref_mock_k = solved_hobby[0].mock_curvature_out;
    let target_circle_k = 0.01; // R = 100 UPM -> curvature = 1 / 100 = 0.01
    for (q, seg) in solved_hobby.iter().enumerate() {
        assert_relative_eq!(seg.mock_curvature_out, ref_mock_k, epsilon = 1e-9);
        assert_relative_eq!(
            seg.mock_curvature_in,
            seg.mock_curvature_out,
            epsilon = 1e-9
        );
        let k0 = cubic_curvature_at(seg.cubic, 0.0);
        let k_mid = cubic_curvature_at(seg.cubic, 0.5);
        let k1 = cubic_curvature_at(seg.cubic, 1.0);
        assert!(
            (k0 - target_circle_k).abs() < 1e-3
                && (k_mid - target_circle_k).abs() < 1e-3
                && (k1 - target_circle_k).abs() < 1e-3,
            "quadrant {q}: Hobby cubic curvature ({k0:.6}, {k_mid:.6}, {k1:.6}) must match 0.01 within 1e-3"
        );
    }

    let euler_type = NodeType::EulerSpiro {
        dir_constraint: HobbyDirConstraint::Free,
    };
    let diamond_euler = vec![
        Node::new(NodeId(11), 100.0, 0.0, euler_type),
        Node::new(NodeId(12), 200.0, 100.0, euler_type),
        Node::new(NodeId(13), 100.0, 200.0, euler_type),
        Node::new(NodeId(14), 0.0, 100.0, euler_type),
    ];
    let solved_euler = evaluate_euler_spiro_chain_to_cubics(&diamond_euler, true, 0.0, &[]);
    assert_eq!(
        solved_euler.len(),
        4,
        "4-node closed EulerSpiro diamond must solve to 4 cubic segments"
    );
    let euler_k0 = cubic_curvature_at(solved_euler[0].cubic, 0.5);
    for (q, seg) in solved_euler.iter().enumerate() {
        let km = cubic_curvature_at(seg.cubic, 0.5);
        assert_relative_eq!(km, euler_k0, epsilon = 1e-6);
        assert!(
            (km - target_circle_k).abs() < 1e-3,
            "quadrant {q}: EulerSpiro midpoint curvature {km:.6} must match circular curvature within 1e-3"
        );
        let mid = seg.cubic.eval(0.5);
        let radius = (mid.x - 100.0).hypot(mid.y - 100.0);
        assert_relative_eq!(radius, 100.0, epsilon = 0.5);
        let next = &solved_euler[(q + 1) % 4];
        let t_in = seg.cubic.deriv().eval(1.0);
        let t_out = next.cubic.deriv().eval(0.0);
        let a_in = t_in.y.atan2(t_in.x);
        let a_out = t_out.y.atan2(t_out.x);
        assert_relative_eq!(a_in, a_out, epsilon = 1e-6);
    }

    // Adversarial edge case (b): 2-node open and 2-node closed HobbyStar & EulerSpiro chains
    let two_hobby = vec![
        Node::new(NodeId(21), 50.0, 100.0, hobby_type),
        Node::new(NodeId(22), 250.0, 180.0, hobby_type),
    ];
    let hobby_open_2 = evaluate_hobby_chain_to_cubics(&two_hobby, false, 0.0, &[]);
    let hobby_closed_2 = evaluate_hobby_chain_to_cubics(&two_hobby, true, 0.0, &[]);
    assert_eq!(hobby_open_2.len(), 1);
    assert_eq!(hobby_closed_2.len(), 2);
    for seg in hobby_open_2.iter().chain(hobby_closed_2.iter()) {
        assert!(
            seg.cubic.p0.x.is_finite()
                && seg.cubic.p0.y.is_finite()
                && seg.cubic.p1.x.is_finite()
                && seg.cubic.p1.y.is_finite()
                && seg.cubic.p2.x.is_finite()
                && seg.cubic.p2.y.is_finite()
                && seg.cubic.p3.x.is_finite()
                && seg.cubic.p3.y.is_finite()
        );
    }
    let bez_hobby_open_2 = contour_to_bezpath_with_splines(&two_hobby, false, 0.0, &[]);
    let bez_hobby_closed_2 = contour_to_bezpath_with_splines(&two_hobby, true, 0.0, &[]);
    assert!(!bez_hobby_open_2.elements().is_empty());
    assert!(!bez_hobby_closed_2.elements().is_empty());

    let two_euler = vec![
        Node::new(NodeId(31), 50.0, 100.0, euler_type),
        Node::new(NodeId(32), 250.0, 180.0, euler_type),
    ];
    let euler_open_2 = evaluate_euler_spiro_chain_to_cubics(&two_euler, false, 0.0, &[]);
    let euler_closed_2 = evaluate_euler_spiro_chain_to_cubics(&two_euler, true, 0.0, &[]);
    assert_eq!(euler_open_2.len(), 1);
    assert_eq!(euler_closed_2.len(), 2);
    for seg in euler_open_2.iter().chain(euler_closed_2.iter()) {
        assert!(
            seg.cubic.p0.x.is_finite()
                && seg.cubic.p0.y.is_finite()
                && seg.cubic.p1.x.is_finite()
                && seg.cubic.p1.y.is_finite()
                && seg.cubic.p2.x.is_finite()
                && seg.cubic.p2.y.is_finite()
                && seg.cubic.p3.x.is_finite()
                && seg.cubic.p3.y.is_finite()
        );
    }
    let bez_euler_open_2 = contour_to_bezpath_with_splines(&two_euler, false, 0.0, &[]);
    let bez_euler_closed_2 = contour_to_bezpath_with_splines(&two_euler, true, 0.0, &[]);
    assert!(!bez_euler_open_2.elements().is_empty());
    assert!(!bez_euler_closed_2.elements().is_empty());
}

// ============================================================================
// TC-V080-002: Hobby Directional Constraints & convert_selected_hobby_nodes_to_cubic
// ============================================================================
#[test]
fn tc_v080_002_hobby_dir_constraints_and_convert_to_cubic() {
    let mut state = GrafiaCanvasState::new(GrafiaTheme::sonoma_default());
    state.viewport.zoom = 0.7;
    state.viewport.pan_offset = ScreenPoint::new(140.0, 440.0);
    state.italic_angle_deg = 12.0;
    state.show_metrics = false;
    state.show_grid = false;
    state.show_selection_info_hud = false;

    let nodes = vec![
        Node::new(
            NodeId(1),
            200.0,
            0.0,
            NodeType::HobbyStar {
                tension_in: 1.0,
                tension_out: 1.0,
                dir_constraint: HobbyDirConstraint::Horizontal,
            },
        ),
        Node::new(
            NodeId(2),
            380.0,
            220.0,
            NodeType::HobbyStar {
                tension_in: 1.1,
                tension_out: 1.1,
                dir_constraint: HobbyDirConstraint::Vertical,
            },
        ),
        Node::new(
            NodeId(3),
            310.0,
            498.0,
            NodeType::HobbyStar {
                tension_in: 1.0,
                tension_out: 1.0,
                dir_constraint: HobbyDirConstraint::FollowMetricLine,
            },
        ),
        Node::new(
            NodeId(4),
            120.0,
            390.0,
            NodeType::HobbyStar {
                tension_in: 1.2,
                tension_out: 1.0,
                dir_constraint: HobbyDirConstraint::SlantedItalic,
            },
        ),
        Node::new(
            NodeId(5),
            50.0,
            150.0,
            NodeType::HobbyStar {
                tension_in: 1.0,
                tension_out: 1.0,
                dir_constraint: HobbyDirConstraint::Free,
            },
        ),
    ];
    let metrics_y = [0.0, 500.0, 700.0, 800.0, -200.0];
    let solved = evaluate_hobby_chain_to_cubics(&nodes, true, state.italic_angle_deg, &metrics_y);
    assert_eq!(solved.len(), 5);

    // Node 0: Horizontal -> tangent aligned with 0° or 180° within 1e-6 rad
    let d_horiz = solved[0].cubic.deriv().eval(0.0);
    let ang_horiz = d_horiz.y.atan2(d_horiz.x).sin().abs();
    assert!(
        ang_horiz < 1e-6,
        "Horizontal tangent sin(angle) must be < 1e-6, got {ang_horiz}"
    );

    // Node 1: Vertical -> tangent aligned with +/-90° within 1e-6 rad
    let d_vert = solved[1].cubic.deriv().eval(0.0);
    let cos_vert = d_vert.y.atan2(d_vert.x).cos().abs();
    assert!(
        cos_vert < 1e-6,
        "Vertical tangent cos(angle) must be < 1e-6, got {cos_vert}"
    );

    // Node 2: FollowMetricLine -> horizontal tangent (parallel to metric line y = 500) within 1e-6 rad
    let d_metric = solved[2].cubic.deriv().eval(0.0);
    assert!(d_metric.y.atan2(d_metric.x).sin().abs() < 1e-6);

    // Node 3: SlantedItalic -> aligned to italic axis (sin(12°), cos(12°)) or opposite within 1e-6 rad
    let d_italic = solved[3].cubic.deriv().eval(0.0);
    let italic_rad = 12.0_f64.to_radians();
    let expected_dir = (italic_rad.sin(), italic_rad.cos());
    let len_it = d_italic.x.hypot(d_italic.y);
    let cross_it =
        ((d_italic.x / len_it) * expected_dir.1 - (d_italic.y / len_it) * expected_dir.0).abs();
    assert!(
        cross_it < 1e-6,
        "SlantedItalic tangent cross error must be < 1e-6, got {cross_it}"
    );

    set_contours(
        &mut state,
        vec![GlyphContour::new(ContourId(0), nodes.clone())],
    );
    state.show_nodes_oncurve = false;
    state.show_nodes_offcurve = false;
    let before_img = DynamicImage::ImageRgba8(pixmap_to_rgba_image(rasterize_canvas_subregion(
        &state,
        &state.theme,
        480,
        420,
    )));

    state.selected_nodes = vec![NodeId(1), NodeId(2), NodeId(3), NodeId(4), NodeId(5)];
    assert!(state.convert_selected_hobby_nodes_to_cubic());
    state.assert_invariants();
    assert_eq!(
        state.history.last_undo_label(),
        Some("Convert Hobby/Euler Splines")
    );
    assert!(
        state.contours[0]
            .iter()
            .all(|n| !matches!(n.node_type, NodeType::HobbyStar { .. })),
        "All selected HobbyStar nodes must be converted to standard cubic nodes"
    );
    assert!(
        state.contours[0]
            .iter()
            .any(|n| n.node_type == NodeType::OffCurve),
        "Converted Hobby contour must contain materialized OffCurve handles"
    );

    state.selected_nodes.clear();
    let after_img = DynamicImage::ImageRgba8(pixmap_to_rgba_image(rasterize_canvas_subregion(
        &state,
        &state.theme,
        480,
        420,
    )));
    let ssim = compute_ssim(&before_img, &after_img, SSIM_DEFAULT_WINDOW_SIZE);
    assert!(
        ssim >= 0.999,
        "Hobby -> Cubic conversion must preserve rendered contour pixels (SSIM >= 0.999, got {ssim:.6})"
    );

    assert!(state.undo());
    assert_eq!(state.contours[0], nodes);

    // Also verify EulerSpiro -> Cubic conversion + SSIM >= 0.999 + Undo
    let euler_nodes: Vec<Node> = (0..4)
        .map(|i| {
            let (x, y) = match i {
                0 => (200.0, 40.0),
                1 => (360.0, 200.0),
                2 => (200.0, 360.0),
                _ => (40.0, 200.0),
            };
            Node::new(
                NodeId(20 + i),
                x,
                y,
                NodeType::EulerSpiro {
                    dir_constraint: HobbyDirConstraint::Free,
                },
            )
        })
        .collect();
    set_contours(
        &mut state,
        vec![GlyphContour::new(ContourId(0), euler_nodes.clone())],
    );
    let euler_before = DynamicImage::ImageRgba8(pixmap_to_rgba_image(rasterize_canvas_subregion(
        &state,
        &state.theme,
        480,
        420,
    )));
    state.selected_nodes = vec![NodeId(20), NodeId(21), NodeId(22), NodeId(23)];
    assert!(state.convert_selected_euler_spiro_nodes_to_cubic());
    assert_eq!(
        state.history.last_undo_label(),
        Some("Convert Hobby/Euler Splines")
    );
    state.selected_nodes.clear();
    let euler_after = DynamicImage::ImageRgba8(pixmap_to_rgba_image(rasterize_canvas_subregion(
        &state,
        &state.theme,
        480,
        420,
    )));
    let euler_ssim = compute_ssim(&euler_before, &euler_after, SSIM_DEFAULT_WINDOW_SIZE);
    assert!(
        euler_ssim >= 0.999,
        "EulerSpiro -> Cubic conversion must achieve SSIM >= 0.999, got {euler_ssim:.6}"
    );
    assert!(state.undo());
    assert_eq!(state.contours[0], euler_nodes);
}

// ============================================================================
// TC-V080-003: 8+4 BBox Outer-Corner Rotation, Shift 15° Snap & Live "∠ ..." Pill
// ============================================================================
#[test]
fn tc_v080_003_bbox_outer_corner_rotation_and_shift_snap() {
    let rect = canvas_rect(800.0, 600.0);
    let mut state = GrafiaCanvasState::new(GrafiaTheme::sonoma_default());
    state.viewport.zoom = 1.0;
    state.viewport.pan_offset = ScreenPoint::new(300.0, 350.0);
    state.show_bounding_box = true;

    let orig_nodes = vec![
        Node::new(NodeId(1), 0.0, 0.0, NodeType::Corner),
        Node::new(NodeId(2), 200.0, 0.0, NodeType::Corner),
        Node::new(NodeId(3), 200.0, 100.0, NodeType::Smooth),
        Node::new(NodeId(4), 140.0, 100.0, NodeType::OffCurve),
        Node::new(NodeId(5), 60.0, 100.0, NodeType::OffCurve),
        Node::new(NodeId(6), 0.0, 100.0, NodeType::Smooth),
    ];
    set_contours(
        &mut state,
        vec![GlyphContour::new(ContourId(0), orig_nodes.clone())],
    );
    // Select OnCurve nodes only; unselected OffCurve handles (4, 5) must rotate rigidly too!
    state.selected_nodes = vec![NodeId(1), NodeId(2), NodeId(3), NodeId(6)];
    state.set_transform_grid_anchor(TransformGridAnchor::Center); // Pivot = (100, 50)

    let bounds = state.selection_bounds().expect("selection bounds");
    for corner in [0u8, 2, 6, 8] {
        let box_s = bounds.box_screen_point(corner, &state.viewport);
        let knob_s = bounds.knob_screen_center(corner, &state.viewport);
        let dx = knob_s.x.0 - box_s.x.0;
        let dy = knob_s.y.0 - box_s.y.0;
        let len = dx.hypot(dy);
        let (ux, uy) = (dx / len, dy / len);
        // 12.0 dp radially outside corner knob center -> inside [6.0 dp, 18.0 dp]
        let ring_pt = ScreenPoint::new(knob_s.x.0 + ux * 12.0, knob_s.y.0 + uy * 12.0);
        assert_eq!(
            state.hit_test_bbox_knob(ring_pt, false, false),
            Some(BBoxAction::Rotate(corner)),
            "12.0 dp radially outside corner {corner} must hit Rotate({corner})"
        );
        // 24.0 dp radially outside -> outside [6.0 dp, 18.0 dp]
        let far_pt = ScreenPoint::new(knob_s.x.0 + ux * 24.0, knob_s.y.0 + uy * 24.0);
        assert_eq!(state.hit_test_bbox_knob(far_pt, false, false), None);
    }

    // 1. Free rotation without Shift by +24.5° around Center pivot (100, 50)
    let box_8 = bounds.box_screen_point(8, &state.viewport);
    let knob_8 = bounds.knob_screen_center(8, &state.viewport);
    let dir_x = knob_8.x.0 - box_8.x.0;
    let dir_y = knob_8.y.0 - box_8.y.0;
    let dir_len = dir_x.hypot(dir_y);
    let start_scr = ScreenPoint::new(
        knob_8.x.0 + (dir_x / dir_len) * 12.0,
        knob_8.y.0 + (dir_y / dir_len) * 12.0,
    );
    let start_font = state.viewport.to_font(start_scr);
    let pivot = FontPoint::new(100.0, 50.0);
    let a0 = (start_font.y.0 - pivot.y.0).atan2(start_font.x.0 - pivot.x.0);
    let r0 = start_font.distance_to(pivot);
    let free_deg = 24.5_f64;
    let a_free = a0 + free_deg.to_radians();
    let free_end_scr = state.viewport.to_screen(FontPoint::new(
        pivot.x.0 + r0 * a_free.cos(),
        pivot.y.0 + r0 * a_free.sin(),
    ));

    let pre_drag_pix = rasterize_canvas_subregion(&state, &state.theme, 640, 480);
    pointer_press(&mut state, rect, start_scr, false, false, false, false);
    pointer_move(&mut state, rect, free_end_scr, false, false, false, false);

    let pill = state
        .active_bbox_rotation_pill_label()
        .expect("live ∠ telemetry pill must be active during BBox rotation drag");
    assert!(
        pill.starts_with("∠") && pill.contains("24.5"),
        "expected live angle pill containing '∠' and '24.5', got {pill}"
    );
    let mid_drag_pix = rasterize_canvas_subregion(&state, &state.theme, 640, 480);
    assert_ne!(pre_drag_pix.data(), mid_drag_pix.data());

    pointer_release(&mut state, rect, free_end_scr, false, false, false, false);
    assert!(state.active_bbox_rotation_pill_label().is_none());

    let rot_aff = kurbo::Affine::rotate_about(free_deg.to_radians(), Point::new(100.0, 50.0));
    for orig in &orig_nodes {
        let expected = rot_aff * Point::new(orig.point.x.0, orig.point.y.0);
        let actual = state.find_node(orig.id).expect("node").point;
        assert_relative_eq!(actual.x.0, expected.x, epsilon = 1e-3);
        assert_relative_eq!(actual.y.0, expected.y, epsilon = 1e-3);
    }

    let rotated_snap = state.snapshot();
    assert!(state.undo());
    assert_eq!(state.contours[0], orig_nodes);
    assert!(state.redo());
    assert_eq!(state.snapshot(), rotated_snap);
    assert!(state.undo());

    // 2. Shift-snapped rotation (+28° -> quantizes to +30.0°, 15° multiple!) around BottomLeft pivot (0, 0)
    state.set_transform_grid_anchor(TransformGridAnchor::BottomLeft);
    let bounds2 = state.selection_bounds().expect("bounds2");
    let k8_2 = bounds2.knob_screen_center(8, &state.viewport);
    let b8_2 = bounds2.box_screen_point(8, &state.viewport);
    let dx2 = k8_2.x.0 - b8_2.x.0;
    let dy2 = k8_2.y.0 - b8_2.y.0;
    let l2 = dx2.hypot(dy2);
    let start_scr_2 = ScreenPoint::new(k8_2.x.0 + (dx2 / l2) * 12.0, k8_2.y.0 + (dy2 / l2) * 12.0);
    let sf2 = state.viewport.to_font(start_scr_2);
    let a0_bl = sf2.y.0.atan2(sf2.x.0);
    let r_bl = sf2.x.0.hypot(sf2.y.0);
    let raw_shift_deg = 28.0_f64;
    let end_scr_2 = state.viewport.to_screen(FontPoint::new(
        r_bl * (a0_bl + raw_shift_deg.to_radians()).cos(),
        r_bl * (a0_bl + raw_shift_deg.to_radians()).sin(),
    ));

    pointer_press(&mut state, rect, start_scr_2, true, false, false, false);
    pointer_move(&mut state, rect, end_scr_2, true, false, false, false);
    let shift_pill = state
        .active_bbox_rotation_pill_label()
        .expect("Shift rotation pill");
    assert!(
        shift_pill.contains("30"),
        "28° with Shift must quantize to 30° (15° step), got pill {shift_pill}"
    );
    pointer_release(&mut state, rect, end_scr_2, true, false, false, false);

    let rot30 = kurbo::Affine::rotate_about(30.0_f64.to_radians(), Point::new(0.0, 0.0));
    for orig in &orig_nodes {
        let expected = rot30 * Point::new(orig.point.x.0, orig.point.y.0);
        let actual = state.find_node(orig.id).expect("node").point;
        assert_relative_eq!(actual.x.0, expected.x, epsilon = 1e-3);
        assert_relative_eq!(actual.y.0, expected.y, epsilon = 1e-3);
    }
    assert!(state.undo());
    assert_eq!(state.contours[0], orig_nodes);
}

// ============================================================================
// TC-V080-004: 8+4 BBox Mid-Edge Shear (Ctrl+Alt)
// ============================================================================
#[test]
fn tc_v080_004_bbox_mid_edge_shear_ctrl_alt() {
    let rect = canvas_rect(800.0, 600.0);
    let mut state = GrafiaCanvasState::new(GrafiaTheme::sonoma_default());
    state.viewport.zoom = 1.0;
    state.viewport.pan_offset = ScreenPoint::new(250.0, 420.0);
    state.show_bounding_box = true;

    // Closed contour spanning x in [0, 100], y in [0, 200] with Smooth right-side node (100, 100)
    // and collinear vertical handles (100, 60) & (100, 140)
    let orig_nodes = vec![
        Node::new(NodeId(1), 0.0, 0.0, NodeType::Corner),
        Node::new(NodeId(2), 100.0, 0.0, NodeType::Corner),
        Node::new(NodeId(3), 100.0, 60.0, NodeType::OffCurve),
        Node::new(NodeId(4), 100.0, 100.0, NodeType::Smooth),
        Node::new(NodeId(5), 100.0, 140.0, NodeType::OffCurve),
        Node::new(NodeId(6), 100.0, 200.0, NodeType::Corner),
        Node::new(NodeId(7), 0.0, 200.0, NodeType::Corner),
    ];
    set_contours(
        &mut state,
        vec![GlyphContour::new(ContourId(0), orig_nodes.clone())],
    );
    state.selected_nodes = vec![NodeId(1), NodeId(2), NodeId(4), NodeId(6), NodeId(7)];

    let bounds = state.selection_bounds().expect("bounds");
    let top_mid_scr = bounds.knob_screen_center(7, &state.viewport);
    assert_eq!(
        state.hit_test_bbox_knob(top_mid_scr, true, true),
        Some(BBoxAction::Slant(7))
    );

    let sheared_scr = ScreenPoint::new(top_mid_scr.x.0 + 40.0, top_mid_scr.y.0);
    pointer_press(&mut state, rect, top_mid_scr, false, true, false, true);
    pointer_move(&mut state, rect, sheared_scr, false, true, false, true);
    pointer_release(&mut state, rect, sheared_scr, false, true, false, true);

    // Bottom edge (y = 0) fixed
    let p1 = state.find_node(NodeId(1)).expect("p1").point;
    let p2 = state.find_node(NodeId(2)).expect("p2").point;
    assert_relative_eq!(p1.x.0, 0.0, epsilon = 1e-4);
    assert_relative_eq!(p1.y.0, 0.0, epsilon = 1e-4);
    assert_relative_eq!(p2.x.0, 100.0, epsilon = 1e-4);
    assert_relative_eq!(p2.y.0, 0.0, epsilon = 1e-4);

    // Top edge (y = 200) shifted horizontally by +40 UPM
    let p6 = state.find_node(NodeId(6)).expect("p6").point;
    let p7 = state.find_node(NodeId(7)).expect("p7").point;
    assert_relative_eq!(p6.x.0, 140.0, epsilon = 1e-4);
    assert_relative_eq!(p6.y.0, 200.0, epsilon = 1e-4);
    assert_relative_eq!(p7.x.0, 40.0, epsilon = 1e-4);
    assert_relative_eq!(p7.y.0, 200.0, epsilon = 1e-4);

    // Smooth-node collinearity at NodeId(4) (between OffCurve NodeId(3) and OffCurve NodeId(5))
    let h_in = state.find_node(NodeId(3)).expect("h_in").point;
    let p_sm = state.find_node(NodeId(4)).expect("p_sm").point;
    let h_out = state.find_node(NodeId(5)).expect("h_out").point;
    let v_in = (p_sm.x.0 - h_in.x.0, p_sm.y.0 - h_in.y.0);
    let v_out = (h_out.x.0 - p_sm.x.0, h_out.y.0 - p_sm.y.0);
    let cross = (v_in.0 * v_out.1 - v_in.1 * v_out.0).abs();
    assert!(
        cross < 1e-4 && (v_in.0 * v_out.0 + v_in.1 * v_out.1) > 0.0,
        "Smooth node handles must remain strictly collinear after shear, cross={cross}"
    );

    assert_eq!(state.history.last_undo_label(), Some("Transform Selection"));
    assert!(state.undo());
    assert_eq!(state.contours[0], orig_nodes);

    // Adversarial edge case (a): 0-height & 0-width degenerate selections during BBox shear/rotation
    // 1) 0-height selection (NodeId(1) at (0, 0) and NodeId(2) at (100, 0))
    state.selected_nodes = vec![NodeId(1), NodeId(2)];
    let zero_h_bounds = state.selection_bounds().expect("zero_h_bounds");
    assert_relative_eq!(zero_h_bounds.height(), 0.0, epsilon = 1e-9);
    let zero_h_shear = grafia::core::bbox_rotate_shear_ops::compute_bbox_shear_affine(
        &zero_h_bounds,
        7,
        FontPoint::new(50.0, 0.0),
        FontPoint::new(120.0, 0.0),
    );
    for coeff in zero_h_shear.as_coeffs() {
        assert!(coeff.is_finite());
    }
    let (_, zero_h_rot) = grafia::core::bbox_rotate_shear_ops::compute_bbox_rotation_deg_and_affine(
        FontPoint::new(0.0, 0.0),
        FontPoint::new(0.0, 0.0),
        FontPoint::new(0.0, 0.0),
        true,
    );
    for coeff in zero_h_rot.as_coeffs() {
        assert!(coeff.is_finite());
    }
    let top_zero_h = zero_h_bounds.knob_screen_center(7, &state.viewport);
    pointer_press(&mut state, rect, top_zero_h, false, true, false, true);
    pointer_move(
        &mut state,
        rect,
        ScreenPoint::new(top_zero_h.x.0 + 50.0, top_zero_h.y.0),
        false,
        true,
        false,
        true,
    );
    pointer_release(
        &mut state,
        rect,
        ScreenPoint::new(top_zero_h.x.0 + 50.0, top_zero_h.y.0),
        false,
        true,
        false,
        true,
    );
    for n in state.contours.iter().flatten() {
        assert!(n.point.x.0.is_finite() && n.point.y.0.is_finite());
    }
    assert!(state.validate_invariants().is_ok());

    // 2) 0-width selection (NodeId(1) at (0, 0) and NodeId(7) at (0, 200))
    state.show_bounding_box_slanted = false;
    set_contours(
        &mut state,
        vec![GlyphContour::new(ContourId(0), orig_nodes)],
    );
    state.selected_nodes = vec![NodeId(1), NodeId(7)];
    let zero_w_bounds = state.selection_bounds().expect("zero_w_bounds");
    assert_relative_eq!(zero_w_bounds.width(), 0.0, epsilon = 1e-9);
    let zero_w_shear = grafia::core::bbox_rotate_shear_ops::compute_bbox_shear_affine(
        &zero_w_bounds,
        5,
        FontPoint::new(0.0, 100.0),
        FontPoint::new(0.0, 180.0),
    );
    for coeff in zero_w_shear.as_coeffs() {
        assert!(coeff.is_finite());
    }
    let right_zero_w = zero_w_bounds.knob_screen_center(5, &state.viewport);
    pointer_press(&mut state, rect, right_zero_w, false, true, false, true);
    pointer_move(
        &mut state,
        rect,
        ScreenPoint::new(right_zero_w.x.0, right_zero_w.y.0 + 50.0),
        false,
        true,
        false,
        true,
    );
    pointer_release(
        &mut state,
        rect,
        ScreenPoint::new(right_zero_w.x.0, right_zero_w.y.0 + 50.0),
        false,
        true,
        false,
        true,
    );
    for n in state.contours.iter().flatten() {
        assert!(n.point.x.0.is_finite() && n.point.y.0.is_finite());
    }
    assert!(state.validate_invariants().is_ok());
}

// ============================================================================
// TC-V080-005: CanvasSmartHandle Ray Projection, Clamping & 4.0 dp Default Snap
// ============================================================================
#[test]
fn tc_v080_005_smart_handle_ray_projection_clamping_and_snap() {
    let rect = canvas_rect(800.0, 600.0);
    let mut state = GrafiaCanvasState::new(GrafiaTheme::sonoma_default());
    state.viewport.zoom = 1.0;
    state.viewport.pan_offset = ScreenPoint::new(200.0, 400.0);

    let comp_contour = GlyphContour::new(
        ContourId(0),
        vec![
            Node::new(NodeId(1), 0.0, 0.0, NodeType::Corner),
            Node::new(NodeId(2), 80.0, 0.0, NodeType::Corner),
            Node::new(NodeId(3), 80.0, 200.0, NodeType::Corner),
            Node::new(NodeId(4), 0.0, 200.0, NodeType::Corner),
        ],
    );
    state.set_components(vec![CanvasComponent {
        id: ComponentId(1),
        component_name: "stem".into(),
        contours: vec![comp_contour],
        transform: Affine2D::identity(),
        locked: false,
        automatic_alignment: false,
        selected: false,
    }]);

    let angle_rad = 30.0_f64.to_radians();
    let dir = (angle_rad.cos(), angle_rad.sin());
    state.set_smart_handles(vec![CanvasSmartHandle {
        component_id: ComponentId(1),
        axis_id: "width".into(),
        axis_name: "width".into(),
        anchor_upm: FontPoint::new(100.0, 100.0),
        direction_unit: dir,
        scale_upm_per_unit: 2.0,
        min_val: 10.0,
        default_val: 50.0,
        max_val: 100.0,
        current_val: 30.0,
    }]);

    let pix_unsel = rasterize_canvas_subregion(&state, &state.theme, 640, 480);
    state.components[0].selected = true;
    let pix_sel = rasterize_canvas_subregion(&state, &state.theme, 640, 480);
    assert_ne!(
        pix_unsel.data(),
        pix_sel.data(),
        "Selected component must render smart handle diamond, track, and pill"
    );

    // Initial handle UPM position: anchor + 2.0 * (30 - 10) * dir = (100, 100) + 40 * dir
    let h_start_upm = FontPoint::new(100.0 + 40.0 * dir.0, 100.0 + 40.0 * dir.1);
    let h_start_scr = state.viewport.to_screen(h_start_upm);
    let _ = state.drain_host_requests();

    pointer_press(&mut state, rect, h_start_scr, false, false, false, false);

    // 1. Drag way past min_val (s = -60 UPM along ray) -> clamps strictly to min_val = 10.0
    let past_min_scr = state
        .viewport
        .to_screen(FontPoint::new(100.0 - 60.0 * dir.0, 100.0 - 60.0 * dir.1));
    pointer_move(&mut state, rect, past_min_scr, false, false, false, false);
    assert_relative_eq!(state.smart_handles[0].current_val, 10.0, epsilon = 1e-6);
    let reqs_min = state.drain_host_requests();
    assert_eq!(
        reqs_min.last(),
        Some(&CanvasHostRequest::UpdateSmartComponentAxis {
            component_id: ComponentId(1),
            axis_id: "width".into(),
            new_value: 10.0,
        })
    );

    // 2. Drag within 4.0 dp (2.5 dp at zoom 1.0 -> 2.5 UPM along ray) of default_val = 50.0
    // default_val = 50.0 sits at anchor + 2.0 * (50 - 10) * dir = anchor + 80.0 * dir
    let near_def_scr = state.viewport.to_screen(FontPoint::new(
        100.0 + 82.5 * dir.0 + 10.0 * (-dir.1),
        100.0 + 82.5 * dir.1 + 10.0 * dir.0,
    ));
    pointer_move(&mut state, rect, near_def_scr, false, false, false, false);
    assert_relative_eq!(state.smart_handles[0].current_val, 50.0, epsilon = 1e-6);
    let reqs_def = state.drain_host_requests();
    assert_eq!(
        reqs_def.last(),
        Some(&CanvasHostRequest::UpdateSmartComponentAxis {
            component_id: ComponentId(1),
            axis_id: "width".into(),
            new_value: 50.0,
        })
    );

    // 3. Drag way past max_val (s = +260 UPM along ray > 2.0 * (100 - 10) = 180) -> clamps to 100.0
    let past_max_scr = state
        .viewport
        .to_screen(FontPoint::new(100.0 + 260.0 * dir.0, 100.0 + 260.0 * dir.1));
    pointer_move(&mut state, rect, past_max_scr, false, false, false, false);
    pointer_release(&mut state, rect, past_max_scr, false, false, false, false);
    assert_relative_eq!(state.smart_handles[0].current_val, 100.0, epsilon = 1e-6);
    let reqs_max = state.drain_host_requests();
    assert!(
        reqs_max.contains(&CanvasHostRequest::UpdateSmartComponentAxis {
            component_id: ComponentId(1),
            axis_id: "width".into(),
            new_value: 100.0,
        })
    );
}

// ============================================================================
// TC-V080-006: Virtual ExtraNode Intersection Stability & Click Host Event
// ============================================================================
#[test]
fn tc_v080_006_virtual_extra_nodes_intersection_stability() {
    let rect_canvas = canvas_rect(800.0, 600.0);
    let horiz = GlyphContour::new(
        ContourId(0),
        vec![
            Node::new(NodeId(1), 0.0, 100.0, NodeType::Corner),
            Node::new(NodeId(2), 300.0, 100.0, NodeType::Corner),
            Node::new(NodeId(3), 300.0, 160.0, NodeType::Corner),
            Node::new(NodeId(4), 0.0, 160.0, NodeType::Corner),
        ],
    );
    let vert = GlyphContour::new(
        ContourId(1),
        vec![
            Node::new(NodeId(5), 120.0, 0.0, NodeType::Corner),
            Node::new(NodeId(6), 180.0, 0.0, NodeType::Corner),
            Node::new(NodeId(7), 180.0, 260.0, NodeType::Corner),
            Node::new(NodeId(8), 120.0, 260.0, NodeType::Corner),
        ],
    );
    let contours_a = vec![horiz, vert];
    let extra_a = compute_layer_extra_nodes(&contours_a);
    assert_eq!(
        extra_a.len(),
        4,
        "Two overlapping perpendicular stroke rectangles must have exactly 4 interior crossings"
    );

    // Translate both contours by (+10, +5) UPM
    let mut contours_b = contours_a.clone();
    for c in &mut contours_b {
        for n in &mut c.nodes {
            n.point.x.0 += 10.0;
            n.point.y.0 += 5.0;
        }
    }
    let extra_b = compute_layer_extra_nodes(&contours_b);
    assert_eq!(extra_b.len(), 4);
    for (ea, eb) in extra_a.iter().zip(extra_b.iter()) {
        assert_eq!(
            ea.key, eb.key,
            "ExtraNodeKey must remain strictly identical under rigid translation"
        );
        assert_relative_eq!(eb.position.x.0, ea.position.x.0 + 10.0, epsilon = 1e-4);
        assert_relative_eq!(eb.position.y.0, ea.position.y.0 + 5.0, epsilon = 1e-4);
    }

    let mut state = GrafiaCanvasState::new(GrafiaTheme::sonoma_default());
    state.viewport.zoom = 1.0;
    state.viewport.pan_offset = ScreenPoint::new(150.0, 420.0);
    set_contours(&mut state, contours_a);

    state.show_extra_nodes = false;
    let pix_off = rasterize_canvas_subregion(&state, &state.theme, 640, 480);
    state.show_extra_nodes = true;
    let pix_on = rasterize_canvas_subregion(&state, &state.theme, 640, 480);
    assert_ne!(
        pix_off.data(),
        pix_on.data(),
        "show_extra_nodes == true must render hollow diamond markers"
    );

    let first_extra = extra_a[0];
    let click_scr = state.viewport.to_screen(first_extra.position);
    let _ = state.drain_host_requests();
    pointer_press(
        &mut state,
        rect_canvas,
        click_scr,
        false,
        false,
        false,
        false,
    );
    pointer_release(
        &mut state,
        rect_canvas,
        click_scr,
        false,
        false,
        false,
        false,
    );

    assert_eq!(state.selected_extra_node, Some(first_extra.key));
    assert_eq!(
        state.drain_host_requests(),
        vec![CanvasHostRequest::SelectOrMutateExtraNode {
            key: first_extra.key,
        }]
    );
}

// ============================================================================
// TC-V080-015: HobbyNode & BackgroundImage SelectionInfo + interpolate_with_background
// ============================================================================
#[test]
fn tc_v080_015_hobby_and_background_image_selection_info_and_interpolate() {
    let mut state = GrafiaCanvasState::new(GrafiaTheme::sonoma_default());
    let fg_nodes = vec![
        Node::new(
            NodeId(1),
            100.0,
            0.0,
            NodeType::HobbyStar {
                tension_in: 1.0,
                tension_out: 1.0,
                dir_constraint: HobbyDirConstraint::Free,
            },
        ),
        Node::new(NodeId(2), 300.0, 0.0, NodeType::Corner),
        Node::new(NodeId(3), 300.0, 400.0, NodeType::Corner),
        Node::new(NodeId(4), 100.0, 400.0, NodeType::Corner),
    ];
    set_contours(
        &mut state,
        vec![GlyphContour::new(ContourId(0), fg_nodes.clone())],
    );

    // 1. HobbyNode SelectionInfo query & mutation
    state.selected_nodes = vec![NodeId(1)];
    match state.selection_info() {
        SelectionInfo::HobbyNode(h) => {
            assert_eq!(h.node_id, NodeId(1));
            assert_relative_eq!(h.tension_in, 1.0, epsilon = 1e-6);
            assert_relative_eq!(h.tension_out, 1.0, epsilon = 1e-6);
            assert_eq!(h.dir_constraint, HobbyDirConstraint::Free);
        }
        other => panic!("expected SelectionInfo::HobbyNode, got {other:?}"),
    }

    assert!(
        state.apply_selection_info_action(SelectionInfoAction::SetHobbyNode {
            tension_in: 1.75,
            tension_out: 2.4,
            dir_constraint: HobbyDirConstraint::Horizontal,
        })
    );
    match state.selection_info() {
        SelectionInfo::HobbyNode(h) => {
            assert_relative_eq!(h.tension_in, 1.75, epsilon = 1e-6);
            assert_relative_eq!(h.tension_out, 2.4, epsilon = 1e-6);
            assert_eq!(h.dir_constraint, HobbyDirConstraint::Horizontal);
        }
        other => panic!("expected updated HobbyNode, got {other:?}"),
    }
    assert!(state.undo());

    // 2. BackgroundImage SelectionInfo query, mutation & reset
    state.selected_nodes.clear();
    let mut bg_img = CanvasBackgroundImage::new([200u8, 100, 50, 255].repeat(16), 4, 4);
    bg_img.selected = true;
    state.set_background_image(Some(bg_img));

    match state.selection_info() {
        SelectionInfo::BackgroundImage(info) => {
            assert_relative_eq!(info.x, 0.0, epsilon = 1e-6);
            assert_relative_eq!(info.scale_x_pct, 100.0, epsilon = 1e-6);
            assert_relative_eq!(info.scale_y_pct, 100.0, epsilon = 1e-6);
            assert!(!info.locked);
        }
        other => panic!("expected SelectionInfo::BackgroundImage, got {other:?}"),
    }

    assert!(
        state.apply_selection_info_action(SelectionInfoAction::MutateBackgroundImage(
            BackgroundImageMutation {
                position: Some(FontPoint::new(45.0, -20.0)),
                scale_x: Some(1.5),
                scale_y: Some(0.8),
                rotation_deg: Some(12.5),
                alpha: Some(0.65),
                locked: Some(false),
                crop_rect_px: Some(Some([1, 1, 2, 2])),
            }
        ))
    );
    match state.selection_info() {
        SelectionInfo::BackgroundImage(info) => {
            assert_relative_eq!(info.x, 45.0, epsilon = 1e-6);
            assert_relative_eq!(info.y, -20.0, epsilon = 1e-6);
            assert_relative_eq!(info.scale_x_pct, 150.0, epsilon = 1e-6);
            assert_relative_eq!(info.scale_y_pct, 80.0, epsilon = 1e-6);
            assert_relative_eq!(info.rotation_deg, 12.5, epsilon = 1e-6);
            assert_relative_eq!(info.alpha, 0.65, epsilon = 1e-5);
            assert_eq!(info.crop_rect_px, Some([1, 1, 2, 2]));
        }
        other => panic!("expected mutated BackgroundImage, got {other:?}"),
    }

    assert!(state.apply_selection_info_action(SelectionInfoAction::ResetBackgroundImageTransform));
    match state.selection_info() {
        SelectionInfo::BackgroundImage(info) => {
            assert_relative_eq!(info.x, 0.0, epsilon = 1e-6);
            assert_relative_eq!(info.y, 0.0, epsilon = 1e-6);
            assert_relative_eq!(info.scale_x_pct, 100.0, epsilon = 1e-6);
            assert_relative_eq!(info.scale_y_pct, 100.0, epsilon = 1e-6);
            assert_relative_eq!(info.rotation_deg, 0.0, epsilon = 1e-6);
            assert_eq!(info.crop_rect_px, None);
        }
        other => panic!("expected reset BackgroundImage, got {other:?}"),
    }

    // 3. interpolate_with_background(0.5) + Undo ("Interpolate with Background")
    let bg_nodes = vec![
        Node::new(
            NodeId(101),
            140.0,
            20.0,
            NodeType::HobbyStar {
                tension_in: 1.0,
                tension_out: 1.0,
                dir_constraint: HobbyDirConstraint::Free,
            },
        ),
        Node::new(NodeId(102), 360.0, 20.0, NodeType::Corner),
        Node::new(NodeId(103), 360.0, 440.0, NodeType::Corner),
        Node::new(NodeId(104), 140.0, 440.0, NodeType::Corner),
    ];
    state.background_contours = vec![GlyphContour::new(ContourId(100), bg_nodes.clone())];
    assert!(state.interpolate_with_background(0.5));
    assert_eq!(
        state.history.last_undo_label(),
        Some("Interpolate with Background")
    );
    for (i, n) in state.contours[0].iter().enumerate() {
        let ex = 0.5 * (fg_nodes[i].point.x.0 + bg_nodes[i].point.x.0);
        let ey = 0.5 * (fg_nodes[i].point.y.0 + bg_nodes[i].point.y.0);
        assert_relative_eq!(n.point.x.0, ex, epsilon = 1e-6);
        assert_relative_eq!(n.point.y.0, ey, epsilon = 1e-6);
    }
    assert!(state.undo());
    for (i, n) in state.contours[0].iter().enumerate() {
        assert_relative_eq!(n.point.x.0, fg_nodes[i].point.x.0, epsilon = 1e-6);
        assert_relative_eq!(n.point.y.0, fg_nodes[i].point.y.0, epsilon = 1e-6);
    }
}

// ============================================================================
// TC-V080-016: Curve Conversions, All-Layers Path Ops, Bookmarks & Node Visibility Parity
// ============================================================================
#[test]
fn tc_v080_016_curve_conversions_all_layers_path_ops_bookmarks_and_node_toggles() {
    let mut state = GrafiaCanvasState::new(GrafiaTheme::sonoma_default());
    state.viewport.zoom = 0.8;
    state.viewport.pan_offset = ScreenPoint::new(140.0, 430.0);

    // 1. Cubic <-> Hobby conversion roundtrip
    let cubic_arch = GlyphContour::new(
        ContourId(0),
        vec![
            Node::new(NodeId(1), 0.0, 0.0, NodeType::Corner),
            Node::new(NodeId(2), 0.0, 120.0, NodeType::OffCurve),
            Node::new(NodeId(3), 80.0, 200.0, NodeType::OffCurve),
            Node::new(NodeId(4), 200.0, 200.0, NodeType::Smooth),
            Node::new(NodeId(5), 320.0, 200.0, NodeType::OffCurve),
            Node::new(NodeId(6), 400.0, 120.0, NodeType::OffCurve),
            Node::new(NodeId(7), 400.0, 0.0, NodeType::Corner),
        ],
    );
    set_contours(&mut state, vec![cubic_arch.clone()]);
    state.selected_nodes = vec![NodeId(1), NodeId(4), NodeId(7)];
    assert!(state.convert_selected_cubic_nodes_to_hobby());
    state.assert_invariants();
    assert_eq!(state.contours[0].len(), 3);
    assert!(state.contours[0]
        .iter()
        .all(|n| matches!(n.node_type, NodeType::HobbyStar { .. })));
    assert!(state.undo());
    assert_eq!(state.contours[0], cubic_arch.nodes);

    // 2. Cubic -> Quadratic (<= 0.5 UPM geometric error) & exact Quadratic -> Cubic degree elevation
    let mut open_cubic = cubic_arch.clone();
    open_cubic.closed = false;
    set_contours(&mut state, vec![open_cubic]);
    let orig_bez = contour_to_bezpath_with_splines(&state.contours[0], false, 0.0, &[]);
    assert!(state.convert_selected_contours_to_quadratic());
    state.assert_invariants();
    assert!(state.paths[0]
        .elements()
        .iter()
        .any(|el| matches!(el, PathEl::QuadTo(..))));
    assert!(!state.paths[0]
        .elements()
        .iter()
        .any(|el| matches!(el, PathEl::CurveTo(..))));

    let quad_bez = state.paths[0].clone();
    // Sample 40 points along original cubic segments and check nearest distance to quad_bez <= 0.5 UPM
    let mut max_err = 0.0_f64;
    let mut last_pt = Point::ZERO;
    for el in orig_bez.elements() {
        match *el {
            PathEl::MoveTo(p) | PathEl::LineTo(p) => last_pt = p,
            PathEl::CurveTo(p1, p2, p3) => {
                let c = CubicBez::new(last_pt, p1, p2, p3);
                for step in 0..=20 {
                    let pt = c.eval(f64::from(step) / 20.0);
                    let mut best = f64::INFINITY;
                    let mut q_prev = Point::ZERO;
                    for qel in quad_bez.elements() {
                        match *qel {
                            PathEl::MoveTo(qp) => q_prev = qp,
                            PathEl::LineTo(qp) => {
                                let d = kurbo::Line::new(q_prev, qp)
                                    .nearest(pt, 1e-6)
                                    .distance_sq
                                    .sqrt();
                                best = best.min(d);
                                q_prev = qp;
                            }
                            PathEl::QuadTo(qc, qp) => {
                                let d = kurbo::QuadBez::new(q_prev, qc, qp)
                                    .nearest(pt, 1e-6)
                                    .distance_sq
                                    .sqrt();
                                best = best.min(d);
                                q_prev = qp;
                            }
                            _ => {}
                        }
                    }
                    max_err = max_err.max(best);
                }
                last_pt = p3;
            }
            _ => {}
        }
    }
    assert!(
        max_err <= 0.5,
        "Cubic -> Quadratic approximation error must be <= 0.5 UPM, got {max_err:.6}"
    );

    // Exact degree elevation Quadratic -> Cubic
    let first_quad = quad_bez
        .elements()
        .iter()
        .find_map(|el| match *el {
            PathEl::QuadTo(q1, p2) => Some((q1, p2)),
            _ => None,
        })
        .expect("at least one QuadTo");
    let p0_q = match quad_bez.elements()[0] {
        PathEl::MoveTo(p) => p,
        _ => Point::ZERO,
    };
    assert!(state.convert_selected_contours_to_cubic());
    state.assert_invariants();
    match state.paths[0].elements()[1] {
        PathEl::CurveTo(h1, h2, p3) => {
            let exp_h1 = Point::new(
                (p0_q.x + 2.0 * first_quad.0.x) / 3.0,
                (p0_q.y + 2.0 * first_quad.0.y) / 3.0,
            );
            let exp_h2 = Point::new(
                (2.0 * first_quad.0.x + first_quad.1.x) / 3.0,
                (2.0 * first_quad.0.y + first_quad.1.y) / 3.0,
            );
            assert_relative_eq!(h1.x, exp_h1.x, epsilon = 1e-6);
            assert_relative_eq!(h1.y, exp_h1.y, epsilon = 1e-6);
            assert_relative_eq!(h2.x, exp_h2.x, epsilon = 1e-6);
            assert_relative_eq!(h2.y, exp_h2.y, epsilon = 1e-6);
            assert_relative_eq!(p3.x, first_quad.1.x, epsilon = 1e-6);
        }
        other => panic!("expected elevated CurveTo, got {other:?}"),
    }

    // 3. add_offcurve_to_selected_segments & remove_offcurve_from_selected_segments
    let box_contour = GlyphContour::new(
        ContourId(0),
        vec![
            Node::new(NodeId(1), 0.0, 0.0, NodeType::Corner),
            Node::new(NodeId(2), 300.0, 0.0, NodeType::Corner),
            Node::new(NodeId(3), 300.0, 300.0, NodeType::Corner),
            Node::new(NodeId(4), 0.0, 300.0, NodeType::Corner),
        ],
    );
    set_contours(&mut state, vec![box_contour.clone()]);
    state.selected_nodes = vec![NodeId(1), NodeId(2)];
    assert!(state.add_offcurve_to_selected_segments());
    state.assert_invariants();
    assert_eq!(state.contours[0].len(), 6);
    assert_relative_eq!(state.contours[0][1].point.x.0, 100.0, epsilon = 1e-6);
    assert_relative_eq!(state.contours[0][2].point.x.0, 200.0, epsilon = 1e-6);

    state.selected_nodes = vec![NodeId(1), NodeId(2)];
    assert!(state.remove_offcurve_from_selected_segments());
    state.assert_invariants();
    assert_eq!(state.contours[0].len(), 4);

    // 4. apply_path_op_across_all_layers(LayerPathOp::ReverseContours)
    let active_area_before = contour_signed_area(&state.contours[0]);
    let edit_nodes = vec![
        Node::new(NodeId(51), 0.0, 0.0, NodeType::Corner),
        Node::new(NodeId(52), 240.0, 0.0, NodeType::Corner),
        Node::new(NodeId(53), 240.0, 240.0, NodeType::Corner),
        Node::new(NodeId(54), 0.0, 240.0, NodeType::Corner),
    ];
    let locked_nodes = vec![
        Node::new(NodeId(61), 0.0, 0.0, NodeType::Corner),
        Node::new(NodeId(62), 180.0, 0.0, NodeType::Corner),
        Node::new(NodeId(63), 180.0, 180.0, NodeType::Corner),
        Node::new(NodeId(64), 0.0, 180.0, NodeType::Corner),
    ];
    let edit_area_before = contour_signed_area(&edit_nodes);
    state.set_companion_master_layers(vec![
        CompanionMasterLayer {
            id: 1,
            name: "Bold".into(),
            color_rgba: [220, 60, 80, 180],
            contours: vec![edit_nodes],
            closed_flags: vec![true],
            anchors: Vec::new(),
            editable: true,
        },
        CompanionMasterLayer {
            id: 2,
            name: "Locked".into(),
            color_rgba: [60, 140, 220, 180],
            contours: vec![locked_nodes.clone()],
            closed_flags: vec![true],
            anchors: Vec::new(),
            editable: false,
        },
    ]);
    state.selected_nodes.clear();
    let snap_pre_op = state.snapshot();
    assert!(state.apply_path_op_across_all_layers(LayerPathOp::ReverseContours));
    assert_relative_eq!(
        contour_signed_area(&state.contours[0]),
        -active_area_before,
        epsilon = 1e-6
    );
    assert_relative_eq!(
        contour_signed_area(&state.companion_master_layers[0].contours[0]),
        -edit_area_before,
        epsilon = 1e-6
    );
    assert_eq!(
        state.companion_master_layers[1].contours[0], locked_nodes,
        "editable == false companion layer must remain untouched"
    );
    assert!(state.undo() && state.snapshot() == snap_pre_op);

    // 5. Viewport Bookmark save & swap toggle
    state.viewport.zoom = 1.25;
    state.viewport.pan_offset = ScreenPoint::new(180.0, 390.0);
    state.save_viewport_bookmark();
    state.viewport.zoom = 2.5;
    state.viewport.pan_offset = ScreenPoint::new(90.0, 210.0);
    assert!(state.toggle_saved_viewport_bookmark());
    assert_relative_eq!(state.viewport.zoom, 1.25, epsilon = 1e-6);
    assert_relative_eq!(state.viewport.pan_offset.x.0, 180.0, epsilon = 1e-4);
    assert!(state.toggle_saved_viewport_bookmark());
    assert_relative_eq!(state.viewport.zoom, 2.5, epsilon = 1e-6);

    // 6. Granular node/overlay visibility toggles & egui vs tiny-skia SSIM >= 0.985 parity
    state.companion_master_layers.clear();
    state.viewport.zoom = 1.0;
    state.viewport.pan_offset = ScreenPoint::new(150.0, 360.0);
    state.show_metrics = false;
    state.show_grid = false;
    state.show_selection_info_hud = false;

    set_contours(&mut state, vec![cubic_arch]);
    state.background_contours = vec![GlyphContour::new(
        ContourId(10),
        vec![
            Node::new(NodeId(201), 40.0, 40.0, NodeType::Corner),
            Node::new(NodeId(202), 140.0, 40.0, NodeType::Corner),
            Node::new(NodeId(203), 140.0, 140.0, NodeType::Corner),
        ],
    )];
    state.set_components(vec![CanvasComponent {
        id: ComponentId(9),
        component_name: "dot".into(),
        contours: vec![GlyphContour::new(
            ContourId(20),
            vec![
                Node::new(NodeId(301), 220.0, 40.0, NodeType::Corner),
                Node::new(NodeId(302), 280.0, 40.0, NodeType::Corner),
                Node::new(NodeId(303), 280.0, 100.0, NodeType::Corner),
                Node::new(NodeId(304), 220.0, 100.0, NodeType::Corner),
            ],
        )],
        transform: Affine2D::identity(),
        locked: false,
        automatic_alignment: false,
        selected: false,
    }]);

    let base_ts = rasterize_canvas_subregion(&state, &state.theme, 560, 400);
    state.show_nodes_oncurve = false;
    let no_on_ts = rasterize_canvas_subregion(&state, &state.theme, 560, 400);
    assert_ne!(base_ts.data(), no_on_ts.data());
    state.show_nodes_oncurve = true;

    state.show_nodes_offcurve = false;
    let no_off_ts = rasterize_canvas_subregion(&state, &state.theme, 560, 400);
    assert_ne!(base_ts.data(), no_off_ts.data());
    state.show_nodes_offcurve = true;

    state.show_node_coordinates = true;
    let coords_ts = rasterize_canvas_subregion(&state, &state.theme, 560, 400);
    assert_ne!(base_ts.data(), coords_ts.data());
    state.show_node_coordinates = false;

    state.show_background_nodes = true;
    state.show_component_piece_frames = true;
    let rich_ts = rasterize_canvas_subregion(&state, &state.theme, 560, 400);
    assert_ne!(base_ts.data(), rich_ts.data());

    let egui_img = rasterize_egui_canvas(&state, 560, 400);
    let ts_img = DynamicImage::ImageRgba8(pixmap_to_rgba_image(rich_ts));
    let parity_ssim = compute_ssim(&egui_img, &ts_img, SSIM_DEFAULT_WINDOW_SIZE);
    assert!(
        parity_ssim >= 0.985,
        "egui vs tiny-skia rendering parity must achieve SSIM >= 0.985, got {parity_ssim:.6}"
    );
}

mod v080_autotrace_prefs_parity;
