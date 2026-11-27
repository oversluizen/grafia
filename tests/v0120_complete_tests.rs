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
//! Normative COTS Verification Suite for `SPEC-013` (`grafia v0.12.0`):
//! Interactive Structural `Stem` Tool & Measurement Rainbow,
//! Non-Linear Curved Master Trajectories (`CurvedTrajectoryLink`),
//! Local `Trowel` & `Cleaner` Brushes,
//! `QuickPreviewOnDrag` & Subtle Metric Overshoot Bands,
//! and Scanline Optical Inter-Shape Rhythm Integrals & Clean 1-Axis Live Mirror
//! (`TC-V0120-001` .. `TC-V0120-012`).

use approx::assert_relative_eq;
use arbtest::arbtest;
use egui::{Context, RawInput, Shape};
use grafia::adapters::headless::{pixmap_to_rgba_image, rasterize_canvas_subregion};
use grafia::{
    classify_node_continuity_in_contours, compute_pair_optical_whitespace,
    evaluate_curved_master_point, ActiveMirrorAxis, CalligraphicNibSpec, CalligraphicSample,
    CanvasHostRequest, CanvasTool, CompanionMasterLayer, ContinuityGrade, ContourId,
    CurvedTrajectoryLink, FontPoint, Glyph, GlyphContour, GlyphSlot, GrafiaCanvasState,
    GrafiaCanvasWidget, GrafiaTheme, Modifiers, Node, NodeId, NodeType, OpticalPairConfig,
    SelectToolState, StemAdjustmentMode, StemOrientation,
};
use image::DynamicImage;
use proptest::prelude::*;

#[path = "visual_pixel_diff_tests.rs"]
#[allow(unused_imports)]
pub mod visual_pixel_diff_tests;
use visual_pixel_diff_tests::{compute_ssim, SSIM_DEFAULT_WINDOW_SIZE};

fn set_layer_contours(state: &mut GrafiaCanvasState, contours: Vec<GlyphContour>) {
    state.contours = contours.iter().map(|c| c.nodes.clone()).collect();
    state.active_contour_closed = contours.last().is_none_or(|c| c.closed);
    state.glyph.contours = contours;
    state.sync_paths_from_contours();
}

fn make_rect_contour(
    id: usize,
    base_node: u64,
    x0: f64,
    y0: f64,
    x1: f64,
    y1: f64,
    cw: bool,
) -> GlyphContour {
    let nodes = if cw {
        vec![
            Node::new(NodeId(base_node), x0, y0, NodeType::Corner),
            Node::new(NodeId(base_node + 1), x0, y1, NodeType::Corner),
            Node::new(NodeId(base_node + 2), x1, y1, NodeType::Corner),
            Node::new(NodeId(base_node + 3), x1, y0, NodeType::Corner),
        ]
    } else {
        vec![
            Node::new(NodeId(base_node), x0, y0, NodeType::Corner),
            Node::new(NodeId(base_node + 1), x1, y0, NodeType::Corner),
            Node::new(NodeId(base_node + 2), x1, y1, NodeType::Corner),
            Node::new(NodeId(base_node + 3), x0, y1, NodeType::Corner),
        ]
    };
    GlyphContour::new(ContourId(id), nodes)
}

fn make_h_contours() -> Vec<GlyphContour> {
    // Single-contour 'H' with left vertical stem [0..80, 0..700], right vertical stem [320..400, 0..700],
    // and horizontal crossbar [80..320, 315..385] (thickness 70 UPM).
    vec![GlyphContour::new(
        ContourId(0),
        vec![
            Node::new(NodeId(1), 0.0, 0.0, NodeType::Corner),
            Node::new(NodeId(2), 80.0, 0.0, NodeType::Corner),
            Node::new(NodeId(3), 80.0, 315.0, NodeType::Corner),
            Node::new(NodeId(4), 320.0, 315.0, NodeType::Corner),
            Node::new(NodeId(5), 320.0, 0.0, NodeType::Corner),
            Node::new(NodeId(6), 400.0, 0.0, NodeType::Corner),
            Node::new(NodeId(7), 400.0, 700.0, NodeType::Corner),
            Node::new(NodeId(8), 320.0, 700.0, NodeType::Corner),
            Node::new(NodeId(9), 320.0, 385.0, NodeType::Corner),
            Node::new(NodeId(10), 80.0, 385.0, NodeType::Corner),
            Node::new(NodeId(11), 80.0, 700.0, NodeType::Corner),
            Node::new(NodeId(12), 0.0, 700.0, NodeType::Corner),
        ],
    )]
}

fn make_v_diagonal_contours() -> Vec<GlyphContour> {
    // Two diagonal legs with exact perpendicular thickness 80.0 UPM at +-20 degrees from vertical.
    // Leg direction unit u = (sin 20, cos 20), inward normal n = (cos 20, -sin 20).
    let theta = 20.0_f64.to_radians();
    let (ux, uy) = (theta.sin(), theta.cos());
    let (nx, ny) = (theta.cos(), -theta.sin());
    let len = 600.0;
    let w = 80.0;
    let p0 = (100.0, 50.0);
    let p1 = (p0.0 + w * nx, p0.1 + w * ny);
    let p2 = (p1.0 + len * ux, p1.1 + len * uy);
    let p3 = (p0.0 + len * ux, p0.1 + len * uy);

    let (ux2, uy2) = (-theta.sin(), theta.cos());
    let (nx2, ny2) = (theta.cos(), theta.sin());
    let q0 = (500.0, 50.0);
    let q1 = (q0.0 + w * nx2, q0.1 + w * ny2);
    let q2 = (q1.0 + len * ux2, q1.1 + len * uy2);
    let q3 = (q0.0 + len * ux2, q0.1 + len * uy2);

    vec![
        GlyphContour::new(
            ContourId(0),
            vec![
                Node::new(NodeId(20), p0.0, p0.1, NodeType::Corner),
                Node::new(NodeId(21), p1.0, p1.1, NodeType::Corner),
                Node::new(NodeId(22), p2.0, p2.1, NodeType::Corner),
                Node::new(NodeId(23), p3.0, p3.1, NodeType::Corner),
            ],
        ),
        GlyphContour::new(
            ContourId(1),
            vec![
                Node::new(NodeId(30), q0.0, q0.1, NodeType::Corner),
                Node::new(NodeId(31), q1.0, q1.1, NodeType::Corner),
                Node::new(NodeId(32), q2.0, q2.1, NodeType::Corner),
                Node::new(NodeId(33), q3.0, q3.1, NodeType::Corner),
            ],
        ),
    ]
}

fn make_arch_stem_contour() -> GlyphContour {
    // Vertical stem [100..180, 0..400] topped by a smooth cubic arch from (180, 400) to (100, 400).
    GlyphContour::new(
        ContourId(0),
        vec![
            Node::new(NodeId(1), 100.0, 0.0, NodeType::Corner),
            Node::new(NodeId(2), 180.0, 0.0, NodeType::Corner),
            Node::new(NodeId(3), 180.0, 400.0, NodeType::Smooth),
            Node::new(NodeId(4), 180.0, 460.0, NodeType::OffCurve),
            Node::new(NodeId(5), 100.0, 460.0, NodeType::OffCurve),
            Node::new(NodeId(6), 100.0, 400.0, NodeType::Smooth),
        ],
    )
}

fn make_oval_contour(
    id: usize,
    base_node: u64,
    x_min: f64,
    y_min: f64,
    x_max: f64,
    y_max: f64,
    cw: bool,
) -> GlyphContour {
    let mut nodes = grafia::build_superellipse_contour_nodes(
        (FontPoint::new(x_min, y_min), FontPoint::new(x_max, y_max)),
        2.0,
    );
    for (i, n) in nodes.iter_mut().enumerate() {
        n.id = NodeId(base_node + i as u64);
    }
    if cw {
        grafia::core::reverse_contour_nodes(&mut nodes, true);
    }
    GlyphContour::new(ContourId(id), nodes)
}

fn rasterize_egui_canvas_for_ssim(
    state: &mut GrafiaCanvasState,
    width: u32,
    height: u32,
) -> DynamicImage {
    let rect = egui::Rect::from_min_size(
        egui::Pos2::ZERO,
        egui::Vec2::new(width as f32, height as f32),
    );
    let ctx = egui::Context::default();
    ctx.tessellation_options_mut(|o| o.feathering = false);
    if state.theme.is_dark() {
        ctx.set_visuals(egui::Visuals::dark());
    } else {
        ctx.set_visuals(egui::Visuals::light());
    }
    let raw = RawInput {
        screen_rect: Some(rect),
        time: Some(1.0),
        ..Default::default()
    };
    let mut full_output = ctx.run_ui(raw, |ui| {
        egui::CentralPanel::default()
            .frame(egui::Frame::NONE)
            .show(ui, |panel_ui| {
                panel_ui.add(
                    GrafiaCanvasWidget::from_state(state)
                        .with_size(egui::Vec2::new(width as f32, height as f32)),
                );
            });
    });
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
                    if !clip.contains(egui::Pos2::new(px, py)) {
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
                    let sr =
                        (w0 * f32::from(c0[0]) + w1 * f32::from(c1[0]) + w2 * f32::from(c2[0]))
                            / 255.0;
                    let sg =
                        (w0 * f32::from(c0[1]) + w1 * f32::from(c1[1]) + w2 * f32::from(c2[1]))
                            / 255.0;
                    let sb =
                        (w0 * f32::from(c0[2]) + w1 * f32::from(c1[2]) + w2 * f32::from(c2[2]))
                            / 255.0;
                    let sa =
                        (w0 * f32::from(c0[3]) + w1 * f32::from(c1[3]) + w2 * f32::from(c2[3]))
                            / 255.0;
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
    let mut img = image::ImageBuffer::new(width, height);
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
                image::Rgba([
                    (acc[0] * scale * 255.0).round() as u8,
                    (acc[1] * scale * 255.0).round() as u8,
                    (acc[2] * scale * 255.0).round() as u8,
                    255,
                ]),
            );
        }
    }
    DynamicImage::ImageRgba8(img)
}

// TC-V0120-001: Structural Stem Detection (Orthogonal 'H' & Diagonal 'V').
#[test]
fn tc_v0120_001_detect_structural_stems_orthogonal_and_diagonal() {
    let mut state = GrafiaCanvasState::init_glyph_a();
    state.active_tool = CanvasTool::Stem;
    set_layer_contours(&mut state, make_h_contours());

    let h_stems = state.detect_structural_stems();
    let v_count = h_stems
        .iter()
        .filter(|s| s.orientation == StemOrientation::Vertical && (s.width_upm - 80.0).abs() <= 0.1)
        .count();
    let h_count = h_stems
        .iter()
        .filter(|s| {
            s.orientation == StemOrientation::Horizontal && (s.width_upm - 70.0).abs() <= 0.1
        })
        .count();
    assert!(
        v_count >= 2,
        "Expected at least 2 vertical stems of width 80 UPM on 'H', got {h_stems:?}"
    );
    assert!(
        h_count >= 1,
        "Expected at least 1 horizontal crossbar of width 70 UPM on 'H', got {h_stems:?}"
    );

    set_layer_contours(&mut state, make_v_diagonal_contours());
    let v_stems = state.detect_structural_stems();
    let diag_count = v_stems
        .iter()
        .filter(|s| s.orientation == StemOrientation::Diagonal && (s.width_upm - 80.0).abs() <= 0.1)
        .count();
    assert_eq!(
        diag_count, 2,
        "Expected 2 diagonal stems of perpendicular width 80 UPM on 'V', got {v_stems:?}"
    );
}

// TC-V0120-002: Structural Stem Thickness Edit, Handle Servo & PropagateStemDelta.
#[test]
fn tc_v0120_002_adjust_structural_stem_servo_and_propagate_delta() {
    let mut state = GrafiaCanvasState::init_glyph_a();
    set_layer_contours(&mut state, vec![make_arch_stem_contour()]);

    let stems = state.detect_structural_stems();
    assert!(!stems.is_empty(), "Expected vertical stem on arch contour");
    let stem_id = stems[0].id;
    assert_relative_eq!(stems[0].width_upm, 80.0, epsilon = 0.1);

    let snap_0 = state.snapshot();
    assert!(state.adjust_structural_stem(
        stem_id,
        20.0,
        StemAdjustmentMode::SymmetricCenter,
        0.0,
        true
    ));

    let stems_after = state.detect_structural_stems();
    assert_relative_eq!(stems_after[0].width_upm, 100.0, epsilon = 0.1);

    let closed_flags = state.closed_flags();
    let g_right = classify_node_continuity_in_contours(&state.contours, &closed_flags, NodeId(3));
    let g_left = classify_node_continuity_in_contours(&state.contours, &closed_flags, NodeId(6));
    assert!(
        g_right >= ContinuityGrade::G1Tangent && g_left >= ContinuityGrade::G1Tangent,
        "Smooth arch joins must remain G1-collinear without kinks after stem widening: right={g_right:?}, left={g_left:?}"
    );

    let reqs = state.drain_host_requests();
    assert!(
        reqs.iter().any(|r| matches!(
            r,
            CanvasHostRequest::PropagateStemDelta {
                orientation: StemOrientation::Vertical,
                old_width_upm,
                new_width_upm,
                ..
            } if (*old_width_upm - 80.0).abs() < 0.2 && (*new_width_upm - 100.0).abs() < 0.2
        )),
        "Expected PropagateStemDelta host request, got {reqs:?}"
    );
    assert_eq!(state.history.last_undo_label(), Some("Adjust Stem"));

    let snap_1 = state.snapshot();
    assert!(state.undo() && state.snapshot() == snap_0);
    assert!(state.redo() && state.snapshot() == snap_1);

    // Verify LockLeft and LockRight modes
    assert!(state.undo());
    assert!(state.adjust_structural_stem(stem_id, 15.0, StemAdjustmentMode::LockLeft, 0.0, false));
    assert_relative_eq!(
        state.glyph.contours[0].nodes[0].point.x.0,
        100.0,
        epsilon = 1e-3
    );
    assert_relative_eq!(
        state.glyph.contours[0].nodes[1].point.x.0,
        195.0,
        epsilon = 1e-3
    );
    assert!(state.undo());
    assert!(state.adjust_structural_stem(stem_id, 15.0, StemAdjustmentMode::LockRight, 0.0, false));
    assert_relative_eq!(
        state.glyph.contours[0].nodes[0].point.x.0,
        85.0,
        epsilon = 1e-3
    );
    assert_relative_eq!(
        state.glyph.contours[0].nodes[1].point.x.0,
        180.0,
        epsilon = 1e-3
    );
}

// TC-V0120-003: Measurement Rainbow Heatmap Rendering Parity (egui vs tiny-skia).
#[test]
fn tc_v0120_003_measurement_rainbow_heatmap_rendering_parity() {
    let theme = GrafiaTheme::sonoma_default();
    let mut state = GrafiaCanvasState::init_glyph_a();
    state.show_metrics = false;
    state.show_grid = false;
    state.show_guides = false;
    state.show_nodes_oncurve = false;
    state.show_nodes_offcurve = false;
    state.show_selection_info_hud = false;
    set_layer_contours(
        &mut state,
        vec![
            make_oval_contour(0, 10, 50.0, 50.0, 450.0, 550.0, false),
            make_oval_contour(1, 30, 140.0, 100.0, 360.0, 500.0, true),
        ],
    );
    state.fit_viewport_to_glyph(320.0, 240.0);

    let px_off = rasterize_canvas_subregion(&state, &theme, 320, 240);
    state.set_show_measurement_rainbow(true);
    assert!(state.curve_harmony.show_measurement_rainbow);
    let px_on = rasterize_canvas_subregion(&state, &theme, 320, 240);
    assert_ne!(
        px_off.data(),
        px_on.data(),
        "Enabling show_measurement_rainbow must shade interior contour thickness"
    );

    let egui_img = rasterize_egui_canvas_for_ssim(&mut state, 320, 240);
    let img_on = DynamicImage::ImageRgba8(pixmap_to_rgba_image(px_on));
    let parity_ssim = compute_ssim(&egui_img, &img_on, SSIM_DEFAULT_WINDOW_SIZE);
    assert!(
        parity_ssim >= 0.985,
        "Measurement rainbow egui vs tiny-skia SSIM must be >= 0.985, got {parity_ssim:.4}"
    );

    state.toggle_show_measurement_rainbow();
    assert!(!state.curve_harmony.show_measurement_rainbow);
}

// TC-V0120-004: Quadratic & Cubic CurvedTrajectoryLink Evaluation.
#[test]
fn tc_v0120_004_curved_trajectory_link_quadratic_and_cubic_evaluation() {
    let p_a = FontPoint::new(0.0, 0.0);
    let p_b = FontPoint::new(100.0, 0.0);
    let link_q = CurvedTrajectoryLink {
        node_id: NodeId(10),
        companion_layer_index: 0,
        control1: FontPoint::new(50.0, 120.0),
        control2: None,
    };

    let p0 = evaluate_curved_master_point(p_a, p_b, Some(&link_q), 0.0);
    let p1 = evaluate_curved_master_point(p_a, p_b, Some(&link_q), 1.0);
    let pm = evaluate_curved_master_point(p_a, p_b, Some(&link_q), 0.5);
    let p_lin = evaluate_curved_master_point(p_a, p_b, None, 0.5);

    assert_relative_eq!(p0.x.0, 0.0, epsilon = 1e-6);
    assert_relative_eq!(p0.y.0, 0.0, epsilon = 1e-6);
    assert_relative_eq!(p1.x.0, 100.0, epsilon = 1e-6);
    assert_relative_eq!(p1.y.0, 0.0, epsilon = 1e-6);
    assert_relative_eq!(pm.x.0, 50.0, epsilon = 1e-6);
    assert_relative_eq!(pm.y.0, 60.0, epsilon = 1e-6);
    assert_relative_eq!(p_lin.x.0, 50.0, epsilon = 1e-6);
    assert_relative_eq!(p_lin.y.0, 0.0, epsilon = 1e-6);

    let link_c = CurvedTrajectoryLink {
        control2: Some(FontPoint::new(50.0, 120.0)),
        ..link_q.clone()
    };
    let pm_cubic = evaluate_curved_master_point(p_a, p_b, Some(&link_c), 0.5);
    assert_relative_eq!(pm_cubic.x.0, 50.0, epsilon = 1e-6);
    assert_relative_eq!(pm_cubic.y.0, 90.0, epsilon = 1e-6);

    let mut state = GrafiaCanvasState::init_glyph_a();
    state.active_tool = CanvasTool::CurvedTrajectory;
    set_layer_contours(
        &mut state,
        vec![GlyphContour::new(
            ContourId(0),
            vec![
                Node::new(NodeId(10), 0.0, 0.0, NodeType::Corner),
                Node::new(NodeId(11), 0.0, 200.0, NodeType::Corner),
                Node::new(NodeId(12), 200.0, 200.0, NodeType::Corner),
            ],
        )],
    );
    state.set_companion_master_layers(vec![CompanionMasterLayer {
        id: 1,
        name: "Bold".into(),
        color_rgba: [220, 60, 80, 180],
        contours: vec![vec![
            Node::new(NodeId(10), 100.0, 0.0, NodeType::Corner),
            Node::new(NodeId(11), 100.0, 200.0, NodeType::Corner),
            Node::new(NodeId(12), 300.0, 200.0, NodeType::Corner),
        ]],
        closed_flags: vec![true],
        anchors: vec![],
        editable: false,
    }]);

    let snap_0 = state.snapshot();
    state.set_curved_trajectory_link(link_q);
    assert_eq!(
        state.history.last_undo_label(),
        Some("Edit Master Trajectory")
    );
    let interp = state
        .evaluate_curved_master_contour(0, 0, 0.5)
        .expect("interpolated contour");
    assert_relative_eq!(interp[0].point.x.0, 50.0, epsilon = 1e-4);
    assert_relative_eq!(interp[0].point.y.0, 60.0, epsilon = 1e-4);
    assert_relative_eq!(interp[1].point.x.0, 50.0, epsilon = 1e-4);
    assert_relative_eq!(interp[1].point.y.0, 200.0, epsilon = 1e-4);

    let snap_1 = state.snapshot();
    assert!(state.undo() && state.snapshot() == snap_0);
    assert!(state.redo() && state.snapshot() == snap_1);

    state.remove_curved_trajectory_link(NodeId(10), 0);
    let interp_lin = state.evaluate_curved_master_contour(0, 0, 0.5).unwrap();
    assert_relative_eq!(interp_lin[0].point.y.0, 0.0, epsilon = 1e-4);

    // Interactive pointer press + drag of control1 in CanvasTool::CurvedTrajectory + t = 0.5 ghost contour rendering
    state.selected_nodes = vec![NodeId(10)];
    let vp_ct = state.active_slot_viewport();
    let ct_start = vp_ct.to_screen(FontPoint::new(50.0, 40.0));
    let ct_end = vp_ct.to_screen(FontPoint::new(50.0, 140.0));
    let ctx_ct = Context::default();
    let theme_ct = GrafiaTheme::sonoma_default();
    let raw_ct = RawInput {
        events: vec![
            egui::Event::PointerButton {
                pos: egui::Pos2::new(ct_start.x.0, ct_start.y.0),
                button: egui::PointerButton::Primary,
                pressed: true,
                modifiers: egui::Modifiers::NONE,
            },
            egui::Event::PointerMoved(egui::Pos2::new(ct_end.x.0, ct_end.y.0)),
            egui::Event::PointerButton {
                pos: egui::Pos2::new(ct_end.x.0, ct_end.y.0),
                button: egui::PointerButton::Primary,
                pressed: false,
                modifiers: egui::Modifiers::NONE,
            },
        ],
        ..Default::default()
    };
    let mut out_ct = ctx_ct.run_ui(raw_ct, |ui| {
        let _ = GrafiaCanvasWidget::new(&mut state, &theme_ct).show(ui);
    });
    out_ct.textures_delta.clear();
    let dragged_interp = state.evaluate_curved_master_contour(0, 0, 0.5).unwrap();
    assert_relative_eq!(dragged_interp[0].point.y.0, 70.0, epsilon = 2.0);
}

// TC-V0120-005: CanvasTool::Trowel Strictly Local Refitting.
#[test]
fn tc_v0120_005_trowel_brush_strictly_local_refitting() {
    let mut state = GrafiaCanvasState::init_glyph_a();
    state.active_tool = CanvasTool::Trowel;
    let mut nodes = Vec::with_capacity(40);
    for i in 0..40 {
        let angle = (i as f64) * std::f64::consts::TAU / 40.0;
        let ripple = if i % 2 == 0 { 2.5 } else { -2.5 };
        let r = 100.0 + ripple;
        nodes.push(Node::new(
            NodeId((i + 1) as u64),
            200.0 + r * angle.cos(),
            200.0 + r * angle.sin(),
            NodeType::Corner,
        ));
    }
    set_layer_contours(&mut state, vec![GlyphContour::new(ContourId(0), nodes)]);

    let center = FontPoint::new(200.0, 300.0);
    let radius = 65.0;
    let outside_before: Vec<(NodeId, u64, u64)> = state.glyph.contours[0]
        .nodes
        .iter()
        .filter(|n| n.point.distance_to(center) > radius + 1e-6)
        .map(|n| (n.id, n.point.x.0.to_bits(), n.point.y.0.to_bits()))
        .collect();
    assert!(outside_before.len() >= 20);

    let count_before = state.glyph.contours[0].nodes.len();
    let snap_0 = state.snapshot();
    assert!(state.apply_trowel_brush_at_upm(center, radius, 6.0));
    assert_eq!(state.history.last_undo_label(), Some("Trowel Smooth"));

    let count_after = state.glyph.contours[0].nodes.len();
    assert!(
        count_after < count_before,
        "Trowel brush must reduce noisy node count inside brush disc ({count_before} -> {count_after})"
    );

    for (nid, xb, yb) in &outside_before {
        let n = state
            .find_node(*nid)
            .expect("Node outside brush disc must remain present");
        assert_eq!(
            n.point.x.0.to_bits(),
            *xb,
            "Node {nid:?} X coordinate outside Trowel disc must be 100% bit-identical"
        );
        assert_eq!(
            n.point.y.0.to_bits(),
            *yb,
            "Node {nid:?} Y coordinate outside Trowel disc must be 100% bit-identical"
        );
    }

    let snap_1 = state.snapshot();
    assert!(state.undo() && state.snapshot() == snap_0);
    assert!(state.redo() && state.snapshot() == snap_1);
    state.validate_invariants().unwrap();
}

// TC-V0120-006: CanvasTool::Cleaner Micro-Loop & Duplicate Node Healing.
#[test]
fn tc_v0120_006_cleaner_brush_micro_loop_and_duplicate_node_healing() {
    let mut state = GrafiaCanvasState::init_glyph_a();
    state.active_tool = CanvasTool::Cleaner;
    // Closed square with a self-intersecting 3x3 UPM micro-loop near (100, 200) and a near-duplicate node.
    let defective = GlyphContour::new(
        ContourId(0),
        vec![
            Node::new(NodeId(1), 0.0, 0.0, NodeType::Corner),
            Node::new(NodeId(2), 200.0, 0.0, NodeType::Corner),
            Node::new(NodeId(3), 200.0, 200.0, NodeType::Corner),
            Node::new(NodeId(4), 102.0, 200.0, NodeType::Corner),
            Node::new(NodeId(5), 99.0, 203.0, NodeType::Corner),
            Node::new(NodeId(6), 102.0, 203.0, NodeType::Corner),
            Node::new(NodeId(7), 99.0, 200.0, NodeType::Corner),
            Node::new(NodeId(8), 99.0005, 200.0002, NodeType::Corner),
            Node::new(NodeId(9), 0.0, 200.0, NodeType::Corner),
        ],
    );
    set_layer_contours(&mut state, vec![defective]);
    let snap_0 = state.snapshot();

    assert!(state.apply_cleaner_brush_at_upm(FontPoint::new(100.0, 201.0), 30.0, 25.0));
    assert_eq!(state.history.last_undo_label(), Some("Clean Contour"));
    assert!(
        state.glyph.contours[0].nodes.len() <= 5,
        "Micro-loop and duplicate nodes must be pruned, got {:?}",
        state.glyph.contours[0].nodes
    );
    state.validate_invariants().unwrap();

    let snap_1 = state.snapshot();
    assert!(state.undo() && state.snapshot() == snap_0);
    assert!(state.redo() && state.snapshot() == snap_1);
}

// TC-V0120-007: QuickPreviewOnDrag Wireframe Suppression & Subtle Metric Overshoot Ribbons.
#[test]
fn tc_v0120_007_quick_preview_on_drag_and_overshoot_ribbons() {
    let ctx = Context::default();
    let theme = GrafiaTheme::sonoma_default();
    let mut state = GrafiaCanvasState::init_glyph_a();
    state.set_quick_preview_on_drag(true);
    assert!(state.curve_harmony.quick_preview_on_drag);

    // 1. Idle frame: wireframe node squares / circles are emitted
    let mut out_idle = ctx.run_ui(RawInput::default(), |ui| {
        let _ = GrafiaCanvasWidget::new(&mut state, &theme).show(ui);
    });
    out_idle.textures_delta.clear();
    let corner_col = theme.edit_canvas.corner_node_color32();
    let idle_node_shapes = out_idle
        .shapes
        .iter()
        .filter(|cs| matches!(&cs.shape, Shape::Rect(r) if r.fill == corner_col))
        .count();
    assert!(
        idle_node_shapes > 0,
        "Idle frame must render corner node rects"
    );

    // 2. Active drag frame with quick_preview_on_drag == true: wireframe node markers suppressed, filled mesh present
    state.select_state = SelectToolState::DraggingNodes {
        selected: vec![state.contours[0][0].id],
        start_pt: FontPoint::new(30.0, 0.0),
        current_pt: FontPoint::new(40.0, 0.0),
        modifiers: Modifiers::default(),
    };
    assert!(state.is_interactive_drag_active());
    let mut out_drag = ctx.run_ui(RawInput::default(), |ui| {
        let _ = GrafiaCanvasWidget::new(&mut state, &theme).show(ui);
    });
    out_drag.textures_delta.clear();
    let drag_node_shapes = out_drag
        .shapes
        .iter()
        .filter(|cs| matches!(&cs.shape, Shape::Rect(r) if r.fill == corner_col))
        .count();
    let drag_meshes = out_drag
        .shapes
        .iter()
        .filter(|cs| matches!(&cs.shape, Shape::Mesh(_)))
        .count();
    assert_eq!(
        drag_node_shapes, 0,
        "QuickPreviewOnDrag must suppress Pass-4 wireframe node rects while dragging"
    );
    assert!(
        drag_meshes > 0,
        "QuickPreviewOnDrag must render filled Non-Zero contour mesh during drag"
    );

    // 3. Pointer release: wireframe node markers immediately restored
    state.select_state = SelectToolState::Idle;
    let mut out_rel = ctx.run_ui(RawInput::default(), |ui| {
        let _ = GrafiaCanvasWidget::new(&mut state, &theme).show(ui);
    });
    out_rel.textures_delta.clear();
    let rel_node_shapes = out_rel
        .shapes
        .iter()
        .filter(|cs| matches!(&cs.shape, Shape::Rect(r) if r.fill == corner_col))
        .count();
    assert_eq!(rel_node_shapes, idle_node_shapes);

    // 4. Subtle Metric Overshoot Ribbons even when alignment_zones is empty
    state.fit_viewport_to_glyph(320.0, 240.0);
    state.metrics.alignment_zones.clear();
    let px_no_ov = rasterize_canvas_subregion(&state, &theme, 320, 240);
    state.set_round_overshoot_upm(Some(16.0));
    assert_eq!(state.curve_harmony.round_overshoot_upm, Some(16.0));
    let px_with_ov = rasterize_canvas_subregion(&state, &theme, 320, 240);
    assert_ne!(
        px_no_ov.data(),
        px_with_ov.data(),
        "round_overshoot_upm must render subtle overshoot bands in Pass 1"
    );
}

// TC-V0120-008: Scanline Optical Whitespace Quadrature (HH vs OO Compensation).
#[test]
fn tc_v0120_008_scanline_optical_whitespace_quadrature_hh_vs_oo() {
    let h_contour = make_rect_contour(0, 1, 50.0, 0.0, 350.0, 700.0, false);
    let o_contour = make_oval_contour(0, 20, 50.0, 0.0, 350.0, 700.0, false);
    let cfg = OpticalPairConfig {
        y_min_upm: 0.0,
        y_max_upm: 700.0,
        sample_count: 64,
        max_cavity_depth_upm: 200.0,
        italic_angle_deg: 0.0,
    };

    let m_hh = compute_pair_optical_whitespace(
        std::slice::from_ref(&h_contour),
        400.0,
        std::slice::from_ref(&h_contour),
        400.0,
        &cfg,
    );
    let m_oo = compute_pair_optical_whitespace(
        std::slice::from_ref(&o_contour),
        400.0,
        std::slice::from_ref(&o_contour),
        400.0,
        &cfg,
    );

    assert_relative_eq!(m_hh.min_clearance_upm, 100.0, epsilon = 0.5);
    assert_relative_eq!(m_hh.optical_area_upm2, 70_000.0, epsilon = 500.0);
    assert!(
        m_oo.optical_area_upm2 > m_hh.optical_area_upm2 + 10_000.0,
        "OO optical area ({:.1}) must be strictly larger than HH ({:.1}) at equal mechanical sidebearings",
        m_oo.optical_area_upm2,
        m_hh.optical_area_upm2
    );

    // Solving for equal optical_area_upm2 on OO yields tighter right_offset_x_upm (< 400.0)
    let mut lo = 300.0_f64;
    let mut hi = 400.0_f64;
    for _ in 0..24 {
        let mid = 0.5 * (lo + hi);
        let area = compute_pair_optical_whitespace(
            std::slice::from_ref(&o_contour),
            400.0,
            std::slice::from_ref(&o_contour),
            mid,
            &cfg,
        )
        .optical_area_upm2;
        if area > m_hh.optical_area_upm2 {
            hi = mid;
        } else {
            lo = mid;
        }
    }
    let equalized_oo_offset = 0.5 * (lo + hi);
    assert!(
        equalized_oo_offset < 385.0,
        "Equalizing optical area on OO must tighten pair advance offset (< 385 UPM), got {equalized_oo_offset:.2}"
    );
}

// TC-V0120-009: Scanline Cavity Clamping & Minimum Collision Clearance ('L' + 'T').
#[test]
fn tc_v0120_009_scanline_cavity_clamping_and_min_collision_clearance() {
    // 'L': vertical stem [50..130, 0..700] + base [50..380, 0..80]
    let l_contour = GlyphContour::new(
        ContourId(0),
        vec![
            Node::new(NodeId(1), 50.0, 0.0, NodeType::Corner),
            Node::new(NodeId(2), 380.0, 0.0, NodeType::Corner),
            Node::new(NodeId(3), 380.0, 80.0, NodeType::Corner),
            Node::new(NodeId(4), 130.0, 80.0, NodeType::Corner),
            Node::new(NodeId(5), 130.0, 700.0, NodeType::Corner),
            Node::new(NodeId(6), 50.0, 700.0, NodeType::Corner),
        ],
    );
    // 'T': top bar [20..380, 620..700] + stem [160..240, 0..700]
    let t_contour = GlyphContour::new(
        ContourId(1),
        vec![
            Node::new(NodeId(10), 160.0, 0.0, NodeType::Corner),
            Node::new(NodeId(11), 240.0, 0.0, NodeType::Corner),
            Node::new(NodeId(12), 240.0, 620.0, NodeType::Corner),
            Node::new(NodeId(13), 380.0, 620.0, NodeType::Corner),
            Node::new(NodeId(14), 380.0, 700.0, NodeType::Corner),
            Node::new(NodeId(15), 20.0, 700.0, NodeType::Corner),
            Node::new(NodeId(16), 20.0, 620.0, NodeType::Corner),
            Node::new(NodeId(17), 160.0, 620.0, NodeType::Corner),
        ],
    );

    let cfg_clamped = OpticalPairConfig {
        y_min_upm: 0.0,
        y_max_upm: 700.0,
        sample_count: 64,
        max_cavity_depth_upm: 60.0,
        italic_angle_deg: 0.0,
    };
    let cfg_unclamped = OpticalPairConfig {
        max_cavity_depth_upm: 600.0,
        ..cfg_clamped
    };

    // Place 'T' at right_offset_x_upm = 340.0 (negative kerning -60 UPM on 400 UPM advance)
    let m_clamped = compute_pair_optical_whitespace(
        std::slice::from_ref(&l_contour),
        400.0,
        std::slice::from_ref(&t_contour),
        340.0,
        &cfg_clamped,
    );
    let m_unclamped = compute_pair_optical_whitespace(
        std::slice::from_ref(&l_contour),
        400.0,
        std::slice::from_ref(&t_contour),
        340.0,
        &cfg_unclamped,
    );

    assert!(
        m_clamped.optical_area_upm2 < m_unclamped.optical_area_upm2,
        "Clamped cavity depth must bound open pocket area ({:.1} < {:.1})",
        m_clamped.optical_area_upm2,
        m_unclamped.optical_area_upm2
    );
    // At y in [0..80], L right exit is 380, T stem left entry is 340 + 160 = 500 -> gap = 120.
    // At y in [620..700], L stem right exit is 130, T top bar left entry is 340 + 20 = 360 -> gap = 230.
    assert_relative_eq!(m_clamped.min_clearance_upm, 120.0, epsilon = 1.0);
}

// TC-V0120-010: Clean 1-Axis Live Mirror Drawing & Rhythm Overlay Parity.
#[test]
fn tc_v0120_010_live_axis_mirror_drawing_and_optical_rhythm_overlay_parity() {
    let theme = GrafiaTheme::sonoma_default();
    let mut state = GrafiaCanvasState::init_glyph_a();
    set_layer_contours(&mut state, Vec::new());

    state.set_active_mirror_axis(ActiveMirrorAxis::Vertical { x_upm: 300.0 });
    assert_eq!(
        state.curve_harmony.active_mirror_axis,
        ActiveMirrorAxis::Vertical { x_upm: 300.0 }
    );

    let snap_0 = state.snapshot();
    let nib = CalligraphicNibSpec {
        min_width_upm: 20.0,
        max_width_upm: 40.0,
        base_angle_deg: 30.0,
        tilt_influence: 0.0,
    };
    let samples = [
        CalligraphicSample {
            pos: FontPoint::new(100.0, 100.0),
            pressure: 0.5,
            tilt_x: 0.0,
            tilt_y: 0.0,
        },
        CalligraphicSample {
            pos: FontPoint::new(180.0, 400.0),
            pressure: 0.8,
            tilt_x: 0.0,
            tilt_y: 0.0,
        },
    ];
    assert!(state
        .apply_calligraphic_brush_stroke(nib, &samples)
        .is_some());
    assert_eq!(
        state.glyph.contours.len(),
        2,
        "Drawing 1 stroke under ActiveMirrorAxis::Vertical must emit 2 symmetric contours"
    );

    // Check mirror symmetry across x = 300.0
    let c0_cx: f64 = state.glyph.contours[0]
        .nodes
        .iter()
        .map(|n| n.point.x.0)
        .sum::<f64>()
        / (state.glyph.contours[0].nodes.len() as f64);
    let c1_cx: f64 = state.glyph.contours[1]
        .nodes
        .iter()
        .map(|n| n.point.x.0)
        .sum::<f64>()
        / (state.glyph.contours[1].nodes.len() as f64);
    assert_relative_eq!(0.5 * (c0_cx + c1_cx), 300.0, epsilon = 1e-2);

    // Single Undo step removes both contours
    let snap_1 = state.snapshot();
    assert!(state.undo() && state.snapshot() == snap_0);
    assert!(state.glyph.contours.is_empty());
    assert!(state.redo() && state.snapshot() == snap_1);

    // Also test add_contour_with_active_mirror under Horizontal axis
    state.set_active_mirror_axis(ActiveMirrorAxis::Horizontal { y_upm: 350.0 });
    let rect_c = make_rect_contour(0, 500, 100.0, 50.0, 200.0, 150.0, false);
    state.add_contour_with_active_mirror(rect_c);
    assert_eq!(state.glyph.contours.len(), 4);

    // Verify Primitives and Pencil also reflect across ActiveMirrorAxis in one Undo step
    let snap_prim = state.snapshot();
    assert!(state.insert_primitive(
        grafia::PrimitiveSubTool::Rectangle,
        FontPoint::new(50.0, 50.0),
        40.0,
        40.0
    ));
    assert_eq!(
        state.glyph.contours.len(),
        6,
        "Primitives under ActiveMirrorAxis must emit 2 symmetric contours in one Undo step"
    );
    assert!(state.undo() && state.snapshot() == snap_prim);

    // Verify interactive canvas pointer events for Trowel and Cleaner tools
    let ctx_tools = Context::default();
    let mut tool_state = GrafiaCanvasState::init_glyph_a();
    let mut noisy_nodes = Vec::with_capacity(40);
    for i in 0..40 {
        let angle = (i as f64) * std::f64::consts::TAU / 40.0;
        let r = 100.0 + if i % 2 == 0 { 2.5 } else { -2.5 };
        noisy_nodes.push(Node::new(
            NodeId((i + 1) as u64),
            200.0 + r * angle.cos(),
            200.0 + r * angle.sin(),
            NodeType::Corner,
        ));
    }
    set_layer_contours(
        &mut tool_state,
        vec![GlyphContour::new(ContourId(0), noisy_nodes)],
    );
    tool_state.active_tool = CanvasTool::Trowel;
    let scr_top = tool_state.viewport.to_screen(FontPoint::new(200.0, 300.0));
    let pos_top = egui::Pos2::new(scr_top.x.0, scr_top.y.0);
    let raw_press = RawInput {
        events: vec![
            egui::Event::PointerButton {
                pos: pos_top,
                button: egui::PointerButton::Primary,
                pressed: true,
                modifiers: egui::Modifiers::NONE,
            },
            egui::Event::PointerButton {
                pos: pos_top,
                button: egui::PointerButton::Primary,
                pressed: false,
                modifiers: egui::Modifiers::NONE,
            },
        ],
        ..Default::default()
    };
    let mut out_tools = ctx_tools.run_ui(raw_press, |ui| {
        let _ = GrafiaCanvasWidget::new(&mut tool_state, &theme).show(ui);
    });
    out_tools.textures_delta.clear();
    assert!(
        tool_state.glyph.contours[0].nodes.len() < 40,
        "Interactive pointer click with CanvasTool::Trowel must smooth nodes under cursor"
    );
    assert_eq!(tool_state.history.last_undo_label(), Some("Trowel Smooth"));

    // Verify continuous interactive pointer dragging (PointerMoved while primary held) with Trowel + visible brush circle
    let count_after_click = tool_state.glyph.contours[0].nodes.len();
    let scr_right = tool_state.viewport.to_screen(FontPoint::new(300.0, 200.0));
    let scr_bot = tool_state.viewport.to_screen(FontPoint::new(200.0, 100.0));
    let raw_drag_trowel = RawInput {
        events: vec![
            egui::Event::PointerButton {
                pos: pos_top,
                button: egui::PointerButton::Primary,
                pressed: true,
                modifiers: egui::Modifiers::NONE,
            },
            egui::Event::PointerMoved(egui::Pos2::new(scr_right.x.0, scr_right.y.0)),
            egui::Event::PointerMoved(egui::Pos2::new(scr_bot.x.0, scr_bot.y.0)),
            egui::Event::PointerButton {
                pos: egui::Pos2::new(scr_bot.x.0, scr_bot.y.0),
                button: egui::PointerButton::Primary,
                pressed: false,
                modifiers: egui::Modifiers::NONE,
            },
        ],
        ..Default::default()
    };
    let mut out_drag_trowel = ctx_tools.run_ui(raw_drag_trowel, |ui| {
        let _ = GrafiaCanvasWidget::new(&mut tool_state, &theme).show(ui);
    });
    out_drag_trowel.textures_delta.clear();
    assert!(
        tool_state.glyph.contours[0].nodes.len() < count_after_click,
        "Continuous PointerMoved drag in CanvasTool::Trowel must smooth additional segments along the sweep"
    );
    let has_brush_circle = out_drag_trowel
        .shapes
        .iter()
        .any(|cs| matches!(&cs.shape, Shape::Circle(c) if (c.radius - 40.0).abs() < 1.0));
    assert!(
        has_brush_circle,
        "CanvasTool::Trowel must render a visible brush circle of radius BRUSH_DEFAULT_RADIUS_DP (40 dp) around cursor_screen_pos"
    );

    // Verify Trowel handles closed contours with multiple separate windows inside disc
    let mut multi_win = GrafiaCanvasState::init_glyph_a();
    let mut mw_nodes = Vec::new();
    for i in 0..24 {
        let angle = (i as f64) * std::f64::consts::TAU / 24.0;
        let rx = 25.0;
        let ry = 120.0;
        mw_nodes.push(Node::new(
            NodeId((i + 1) as u64),
            200.0 + rx * angle.cos(),
            200.0 + ry * angle.sin(),
            NodeType::Corner,
        ));
    }
    set_layer_contours(
        &mut multi_win,
        vec![GlyphContour::new(ContourId(0), mw_nodes)],
    );
    let center_mw = FontPoint::new(200.0, 200.0);
    let rad_mw = 60.0;
    let out_mw: Vec<(NodeId, u64, u64)> = multi_win.glyph.contours[0]
        .nodes
        .iter()
        .filter(|n| n.point.distance_to(center_mw) > rad_mw + 1e-6)
        .map(|n| (n.id, n.point.x.0.to_bits(), n.point.y.0.to_bits()))
        .collect();
    assert!(multi_win.apply_trowel_brush_at_upm(center_mw, rad_mw, 5.0));
    for (nid, xb, yb) in &out_mw {
        let n = multi_win
            .find_node(*nid)
            .expect("Exterior node on multi-window contour must remain present");
        assert_eq!(n.point.x.0.to_bits(), *xb);
        assert_eq!(n.point.y.0.to_bits(), *yb);
    }

    // Verify Cleaner removes wrap-around micro-loop and duplicate node without touching exterior nodes
    let mut wrap_clean = GrafiaCanvasState::init_glyph_a();
    let wrap_contour = GlyphContour::new(
        ContourId(0),
        vec![
            Node::new(NodeId(1), 99.0, 200.0, NodeType::Corner),
            Node::new(NodeId(2), 99.0005, 200.0002, NodeType::Corner),
            Node::new(NodeId(3), 0.0, 200.0, NodeType::Corner),
            Node::new(NodeId(4), 0.0, 0.0, NodeType::Corner),
            Node::new(NodeId(5), 200.0, 0.0, NodeType::Corner),
            Node::new(NodeId(6), 200.0, 200.0, NodeType::Corner),
            Node::new(NodeId(7), 102.0, 200.0, NodeType::Corner),
            Node::new(NodeId(8), 99.0, 203.0, NodeType::Corner),
            Node::new(NodeId(9), 102.0, 203.0, NodeType::Corner),
        ],
    );
    set_layer_contours(&mut wrap_clean, vec![wrap_contour]);
    assert!(wrap_clean.apply_cleaner_brush_at_upm(FontPoint::new(100.0, 201.0), 30.0, 25.0));
    assert!(
        wrap_clean.glyph.contours[0].nodes.len() <= 5,
        "Wrap-around micro-loop at contour boundary must be pruned, got {:?}",
        wrap_clean.glyph.contours[0].nodes
    );

    // Optical Rhythm Fill overlay parity in TextTool mode
    state.active_tool = CanvasTool::Text;
    state.show_metrics = false;
    state.show_grid = false;
    state.show_guides = false;
    state.show_selection_info_hud = false;
    state.text_transients.text_mode_numbers_threshold = 10_000.0;
    state.viewport =
        grafia::ViewportTransform::new(0.25, grafia::ScreenPoint::new(30.0, 200.0), 1000);
    let g_h = Glyph {
        name: "H".into(),
        width: 400.0,
        contours: vec![make_rect_contour(0, 1, 50.0, 0.0, 350.0, 700.0, false)],
        ..Default::default()
    };
    state.set_text_stream_slots(
        vec![GlyphSlot::new(g_h.clone(), 0), GlyphSlot::new(g_h, 0)],
        0,
    );
    let px_rhythm_off = rasterize_canvas_subregion(&state, &theme, 320, 240);
    state.set_show_optical_rhythm_fill(true);
    let px_rhythm_on = rasterize_canvas_subregion(&state, &theme, 320, 240);
    assert_ne!(px_rhythm_off.data(), px_rhythm_on.data());

    let egui_rhythm = rasterize_egui_canvas_for_ssim(&mut state, 320, 240);
    let ts_rhythm = DynamicImage::ImageRgba8(pixmap_to_rgba_image(px_rhythm_on));
    let rhythm_ssim = compute_ssim(&egui_rhythm, &ts_rhythm, SSIM_DEFAULT_WINDOW_SIZE);
    assert!(
        rhythm_ssim >= 0.985,
        "Optical rhythm fill egui vs tiny-skia SSIM must be >= 0.985, got {rhythm_ssim:.4}"
    );
}

proptest! {
    // TC-V0120-011: Property-Based proptest Stem Deltas, Curved Trajectories & Optical Scans.
    #[test]
    fn tc_v0120_011_proptest_stem_deltas_curved_trajectories_and_optical_scans(
        delta_w in -30.0f64..80.0f64,
        t in 0.0f64..1.0f64,
        gap_offset in 360.0f64..520.0f64
    ) {
        let mut state = GrafiaCanvasState::init_glyph_a();
        set_layer_contours(&mut state, vec![make_arch_stem_contour()]);
        let stems = state.detect_structural_stems();
        prop_assert!(!stems.is_empty());
        let _ = state.adjust_structural_stem(
            stems[0].id,
            delta_w,
            StemAdjustmentMode::SymmetricCenter,
            0.0,
            false,
        );
        prop_assert!(state.validate_invariants().is_ok());

        let link = CurvedTrajectoryLink {
            node_id: NodeId(1),
            companion_layer_index: 0,
            control1: FontPoint::new(40.0, 90.0),
            control2: Some(FontPoint::new(80.0, -30.0)),
        };
        let pt = evaluate_curved_master_point(
            FontPoint::new(0.0, 0.0),
            FontPoint::new(120.0, 40.0),
            Some(&link),
            t,
        );
        prop_assert!(pt.x.0.is_finite() && pt.y.0.is_finite());

        let h_contour = make_rect_contour(0, 1, 50.0, 0.0, 350.0, 700.0, false);
        let cfg = OpticalPairConfig {
            y_min_upm: 0.0,
            y_max_upm: 700.0,
            sample_count: 32,
            max_cavity_depth_upm: 200.0,
            italic_angle_deg: 0.0,
        };
        let m1 = compute_pair_optical_whitespace(
            std::slice::from_ref(&h_contour),
            400.0,
            std::slice::from_ref(&h_contour),
            gap_offset,
            &cfg,
        );
        let m2 = compute_pair_optical_whitespace(
            std::slice::from_ref(&h_contour),
            400.0,
            std::slice::from_ref(&h_contour),
            gap_offset + 20.0,
            &cfg,
        );
        prop_assert!(m1.optical_area_upm2.is_finite() && m2.optical_area_upm2 >= m1.optical_area_upm2);
    }
}

// TC-V0120-012: Stateful arbtest Multi-Tool Fuzzing (Stem + Trajectory + Trowel + Cleaner + Mirror + Undo).
#[test]
fn tc_v0120_012_arbtest_stateful_multi_tool_fuzzing_and_undo_roundtrip() {
    arbtest(|u| {
        let mut state = GrafiaCanvasState::init_glyph_a();
        set_layer_contours(&mut state, vec![make_arch_stem_contour()]);
        for _ in 0..u.int_in_range(8..=16)? {
            let before = state.snapshot();
            match u.int_in_range(0..=6)? {
                0 => {
                    let stems = state.detect_structural_stems();
                    if let Some(first) = stems.first() {
                        let dw = f64::from(u.int_in_range(-10..=25)?);
                        if state.adjust_structural_stem(
                            first.id,
                            dw,
                            StemAdjustmentMode::SymmetricCenter,
                            0.0,
                            false,
                        ) {
                            let after = state.snapshot();
                            assert!(state.undo() && state.snapshot() == before);
                            assert!(state.redo() && state.snapshot() == after);
                        }
                    }
                }
                1 => {
                    state.set_curved_trajectory_link(CurvedTrajectoryLink {
                        node_id: NodeId(1),
                        companion_layer_index: 0,
                        control1: FontPoint::new(
                            f64::from(u.int_in_range(0..=200)?),
                            f64::from(u.int_in_range(0..=200)?),
                        ),
                        control2: None,
                    });
                    let after = state.snapshot();
                    if after != before {
                        assert!(state.undo() && state.snapshot() == before);
                        assert!(state.redo() && state.snapshot() == after);
                    }
                }
                2 => {
                    let _ =
                        state.apply_trowel_brush_at_upm(FontPoint::new(140.0, 400.0), 50.0, 4.0);
                }
                3 => {
                    let _ =
                        state.apply_cleaner_brush_at_upm(FontPoint::new(100.0, 0.0), 30.0, 25.0);
                }
                4 => {
                    state.set_active_mirror_axis(ActiveMirrorAxis::Vertical { x_upm: 300.0 });
                    let before_mirror = state.snapshot();
                    let c = make_rect_contour(0, 900, 20.0, 20.0, 60.0, 80.0, false);
                    state.add_contour_with_active_mirror(c);
                    let after = state.snapshot();
                    assert!(state.undo() && state.snapshot() == before_mirror);
                    assert!(state.redo() && state.snapshot() == after);
                }
                5 => {
                    state.set_show_measurement_rainbow(u.arbitrary()?);
                    state.set_quick_preview_on_drag(u.arbitrary()?);
                    state.set_show_optical_rhythm_fill(u.arbitrary()?);
                }
                _ => {
                    if state.undo() {
                        state.validate_invariants().unwrap();
                        let _ = state.redo();
                    }
                }
            }
            state.validate_invariants().unwrap();
        }
        Ok(())
    })
    .budget_ms(250);
}

// TC-V0130-001 .. TC-V0130-008: SPEC-015 5 Universal System-Agnostic Controller Ports ("Stekkers").
#[test]
fn tc_v0130_001_to_008_system_agnostic_controller_intent_rotary_telemetry_haptic_and_stylus_ports()
{
    use grafia::{
        CanvasHapticEvent, CanvasIntent, CanvasIntentMask, CanvasRotaryChannel, InputDeviceKind,
        SmartCornerSpec, StylusAccessoryEvent,
    };

    let mut state = GrafiaCanvasState::init_glyph_a();
    // 1. Semantic Intent Port (CanvasIntent & CanvasIntentMask)
    state.set_intent_latched(CanvasIntent::BreakCuspHandle, true);
    state.set_external_intents(
        CanvasIntentMask::from_intent(CanvasIntent::LockAspectRatio)
            | CanvasIntentMask::from_intent(CanvasIntent::OriginFromCenter),
    );
    let eff = state.effective_intents(Modifiers::default(), false);
    assert!(eff.contains(CanvasIntent::BreakCuspHandle));
    assert!(eff.contains(CanvasIntent::LockAspectRatio));
    assert!(eff.contains(CanvasIntent::OriginFromCenter));

    // 2. Controller Telemetry Port
    state.activate_tool(CanvasTool::Primitives);
    let tel = state.controller_telemetry();
    assert_eq!(tel.active_tool, CanvasTool::Primitives);
    assert!(tel
        .available_intents
        .iter()
        .any(|i| i.intent == CanvasIntent::LockAspectRatio && i.short_label == "1:1" && i.active));
    assert!(tel
        .available_intents
        .iter()
        .any(|i| i.intent == CanvasIntent::OriginFromCenter
            && i.short_label == "⊙ Center"
            && i.active));

    state.clear_external_intents();
    assert!(state
        .effective_intents(Modifiers::default(), false)
        .is_empty());

    // 3. Continuous Rotary Encoder Port (ContextPrimary auto-routing + dedicated channels)
    let first_nid = state.contours[0][0].id;
    state.selected_nodes = vec![first_nid];
    state.set_smart_corner(SmartCornerSpec {
        node_id: first_nid,
        radius_upm: 20.0,
    });
    assert!(state.step_rotary_encoder(CanvasRotaryChannel::ContextPrimary, 3.0));
    assert!(state.smart_corners()[0].radius_upm > 20.0);
    assert_eq!(
        state.controller_telemetry().primary_rotary.parameter_name,
        "Corner Radius"
    );

    let zoom_before = state.viewport.zoom;
    assert!(state.step_rotary_encoder(CanvasRotaryChannel::ViewportZoom, 2.0));
    assert!(state.viewport.zoom > zoom_before);

    // HistoryScrub (-1.0 Undo, +1.0 Redo)
    let r_after_step = state.smart_corners()[0].radius_upm;
    assert!(state.step_rotary_encoder(CanvasRotaryChannel::HistoryScrub, -1.0));
    assert_relative_eq!(state.smart_corners()[0].radius_upm, 20.0, epsilon = 1e-3);
    assert!(state.step_rotary_encoder(CanvasRotaryChannel::HistoryScrub, 1.0));
    assert_relative_eq!(
        state.smart_corners()[0].radius_upm,
        r_after_step,
        epsilon = 1e-3
    );

    // 4. Haptic Feedback Event Port
    let haptics = state.drain_haptic_events();
    assert!(haptics.iter().any(|e| matches!(
        e,
        CanvasHapticEvent::IntentLatchToggled {
            intent: CanvasIntent::BreakCuspHandle,
            latched: true
        }
    )));
    assert!(haptics.iter().any(|e| matches!(
        e,
        CanvasHapticEvent::RotaryDetentTick {
            channel: CanvasRotaryChannel::ContextPrimary
        }
    )));
    assert!(state.drain_haptic_events().is_empty());

    // 5. Mobile & Stylus Precision Port (InputDeviceKind::TouchFinger, 1.6x hit radius, Loupe & Stylus DoubleTap)
    let mut prefs = state.preferences.clone();
    prefs.touch_finger_offset_dp = 36.0;
    prefs.show_touch_precision_loupe = true;
    state.apply_preferences(&prefs);

    state.set_active_input_device(InputDeviceKind::Mouse);
    let raw_scr = grafia::ScreenPoint::new(200.0, 300.0);
    assert_eq!(state.resolve_effective_pointer_screen_pos(raw_scr), raw_scr);

    let n0_scr = state
        .active_slot_viewport()
        .to_screen(state.contours[0][0].point);
    let offset_probe = grafia::ScreenPoint::new(n0_scr.x.0 + 10.5, n0_scr.y.0);
    assert!(state.hit_test_node(offset_probe).is_none());

    state.set_active_input_device(InputDeviceKind::TouchFinger);
    assert_relative_eq!(
        state.resolve_effective_pointer_screen_pos(raw_scr).y.0,
        264.0,
        epsilon = 1e-3
    );
    assert_eq!(state.hit_test_node(offset_probe), Some(first_nid));

    let rect = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::Vec2::new(800.0, 600.0));
    state.cursor_screen_pos = Some(grafia::ScreenPoint::new(300.0, 250.0));
    let loupe_mid = state.compute_precision_loupe_overlay(rect).expect("loupe");
    assert!(!loupe_mid.flipped_from_top_edge);
    state.cursor_screen_pos = Some(grafia::ScreenPoint::new(300.0, 35.0));
    let loupe_top = state
        .compute_precision_loupe_overlay(rect)
        .expect("top loupe");
    assert!(loupe_top.flipped_from_top_edge);
    assert!(loupe_top.lens_center_screen_pos.y.0 - loupe_top.lens_radius_dp >= rect.top());

    // Stylus DoubleTap swaps between Pen and Select
    state.activate_tool(CanvasTool::Pen);
    assert!(state.dispatch_stylus_accessory_event(StylusAccessoryEvent::DoubleTap));
    assert_eq!(state.active_tool, CanvasTool::Select);
    assert!(state.dispatch_stylus_accessory_event(StylusAccessoryEvent::DoubleTap));
    assert_eq!(state.active_tool, CanvasTool::Pen);
    assert!(state.dispatch_stylus_accessory_event(StylusAccessoryEvent::BarrelHold(true)));
    assert!(state
        .effective_intents(Modifiers::default(), false)
        .contains(CanvasIntent::BreakCuspHandle));
    assert!(state.dispatch_stylus_accessory_event(StylusAccessoryEvent::BarrelHold(false)));

    // Graceful multi-touch promotion rolls back uncommitted drag
    state.drag_transients.active_drag_before_snapshot = Some(state.snapshot());
    state.contours[0][0].point.x.0 += 123.0;
    assert!(state.promote_to_multitouch_gesture());
    assert_relative_eq!(state.contours[0][0].point.x.0, 30.0, epsilon = 1e-3);

    // 6. Selected Corner node without existing SmartCornerSpec auto-creates SmartCornerSpec on ContextPrimary
    let second_nid = state.contours[0][1].id;
    state.selected_nodes = vec![second_nid];
    assert!(state
        .smart_corners()
        .iter()
        .all(|s| s.node_id != second_nid));
    assert_eq!(
        state.controller_telemetry().primary_rotary.parameter_name,
        "Corner Radius"
    );
    assert!(state.step_rotary_encoder(CanvasRotaryChannel::ContextPrimary, 4.0));
    assert_relative_eq!(
        state
            .smart_corners()
            .iter()
            .find(|s| s.node_id == second_nid)
            .expect("auto-created SmartCornerSpec for selected Corner node")
            .radius_upm,
        8.0,
        epsilon = 1e-3
    );

    // 7. KerningOrSpacing in CanvasTool::Text adjusts active slot pair kerning and emits KerningDelta
    state.load_demo_text_strip();
    state.activate_tool(CanvasTool::Text);
    state.switch_active_slot(1);
    let kern_before = state.text_stream.slots[1].kern_with_prev;
    assert!(state.step_rotary_encoder(CanvasRotaryChannel::KerningOrSpacing, -3.0));
    assert!(
        state.text_stream.slots[1].kern_with_prev < kern_before,
        "KerningOrSpacing in Text mode must adjust active slot pair kerning"
    );
    assert!(!state.drain_kerning_deltas().is_empty());

    // 8. Closing an open contour by dragging endpoint in Select emits CanvasHapticEvent::ContourClosed
    let mut open_state = GrafiaCanvasState::init_glyph_a();
    let mut open_tri = GlyphContour::new(
        ContourId(0),
        vec![
            Node::new(NodeId(1), 0.0, 0.0, NodeType::Corner),
            Node::new(NodeId(2), 100.0, 200.0, NodeType::Corner),
            Node::new(NodeId(3), 4.0, 0.0, NodeType::Corner),
        ],
    );
    open_tri.closed = false;
    set_layer_contours(&mut open_state, vec![open_tri]);
    open_state.selected_nodes = vec![NodeId(3)];
    assert!(
        grafia::adapters::contour_join::try_join_or_close_open_contours_on_select_release(
            &mut open_state
        )
    );
    assert!(
        open_state
            .drain_haptic_events()
            .contains(&CanvasHapticEvent::ContourClosed),
        "Closing an open contour in Select must emit CanvasHapticEvent::ContourClosed"
    );

    // 9. Multi-touch promotion during active Trowel brush stroke cancels brush_stroke_active cleanly
    let mut brush_mt = GrafiaCanvasState::init_glyph_a();
    brush_mt.activate_tool(CanvasTool::Trowel);
    let before_mt = brush_mt.snapshot();
    let p_scr = brush_mt.viewport.to_screen(brush_mt.contours[0][0].point);
    let p_font = brush_mt.contours[0][0].point;
    assert!(brush_mt.on_drawing_tool_pointer_press(p_scr, p_font, Modifiers::default()));
    assert!(brush_mt.drawing_tool_state.brush_stroke_active);
    assert!(brush_mt.promote_to_multitouch_gesture());
    assert!(
        !brush_mt.drawing_tool_state.brush_stroke_active,
        "promote_to_multitouch_gesture must reset brush_stroke_active"
    );
    assert_eq!(brush_mt.snapshot(), before_mt);

    // 10. Interactive CurvedTrajectory tool also hit-tests and drags cubic control2 handle
    let mut ct_cubic = GrafiaCanvasState::init_glyph_a();
    ct_cubic.activate_tool(CanvasTool::CurvedTrajectory);
    ct_cubic.set_curved_trajectory_link(CurvedTrajectoryLink {
        node_id: NodeId(1),
        companion_layer_index: 0,
        control1: FontPoint::new(40.0, 80.0),
        control2: Some(FontPoint::new(140.0, 120.0)),
    });
    let c2_scr = ct_cubic
        .active_slot_viewport()
        .to_screen(FontPoint::new(140.0, 120.0));
    assert!(ct_cubic.on_drawing_tool_pointer_press(
        c2_scr,
        FontPoint::new(140.0, 120.0),
        Modifiers::default()
    ));
    assert!(ct_cubic.on_drawing_tool_pointer_move(
        c2_scr,
        FontPoint::new(165.0, 155.0),
        Modifiers::default()
    ));
    assert!(ct_cubic
        .on_drawing_tool_pointer_release(FontPoint::new(165.0, 155.0), Modifiers::default()));
    assert_eq!(
        ct_cubic.curve_harmony.curved_trajectories[0].control2,
        Some(FontPoint::new(165.0, 155.0))
    );
}

// TC-V0130-009 .. TC-V0130-012: Touchscreen Auto-Detection, OffCurve Handle Priority, Universal Touch Hit-Radii, Tap Slop & Double-Tap Line-to-Cubic.
#[test]
fn tc_v0130_009_to_012_touchscreen_auto_detection_handle_priority_segment_selection_and_double_tap_line_to_cubic(
) {
    use grafia::{AnchorId, CanvasAnchor, GrafiaPreferences, InputDeviceKind};

    let ctx = Context::default();
    let theme = GrafiaTheme::sonoma_default();
    let mut state = GrafiaCanvasState::init_glyph_a();
    assert_eq!(state.active_input_device(), InputDeviceKind::Mouse);
    assert_relative_eq!(
        GrafiaPreferences::default().touch_finger_offset_dp,
        0.0,
        epsilon = 1e-6
    );

    // 1. TC-V0130-009: Dispatching egui::Event::Touch automatically switches active_input_device to TouchFinger.
    let n0_scr = state.viewport.to_screen(state.contours[0][0].point);
    let touch_pos = egui::Pos2::new(n0_scr.x.0 - 14.5, n0_scr.y.0);
    let raw_touch = RawInput {
        events: vec![
            egui::Event::Touch {
                device_id: egui::TouchDeviceId(1),
                id: egui::TouchId(1),
                phase: egui::TouchPhase::Start,
                pos: touch_pos,
                force: Some(0.5),
            },
            egui::Event::Touch {
                device_id: egui::TouchDeviceId(1),
                id: egui::TouchId(1),
                phase: egui::TouchPhase::End,
                pos: touch_pos,
                force: Some(0.0),
            },
        ],
        ..Default::default()
    };
    let mut out1 = ctx.run_ui(raw_touch, |ui| {
        let _ = GrafiaCanvasWidget::new(&mut state, &theme).show(ui);
    });
    out1.textures_delta.clear();
    assert_eq!(
        state.active_input_device(),
        InputDeviceKind::TouchFinger,
        "Receiving egui::Event::Touch must automatically switch active_input_device to TouchFinger"
    );
    assert!(
        state.effective_hit_radius_dp() >= 16.0,
        "TouchFinger node hit radius must be >= 16.0 dp, got {}",
        state.effective_hit_radius_dp()
    );
    assert_eq!(
        state.selected_nodes,
        vec![state.contours[0][0].id],
        "Touch press 14.5 dp from node 0 must select node 0 under >= 16.0 dp touch hit radius"
    );

    // 2. TC-V0130-010: OffCurve handle priority over nearby OnCurve node AND closer OffCurve over farther selected OffCurve + Tunni line.
    let mut state_h = GrafiaCanvasState::init_glyph_o();
    state_h.set_active_input_device(InputDeviceKind::TouchFinger);
    state_h.show_tunni_lines = true;
    // Place OnCurve(1) at (100, 100), OffCurve(2) at (100, 110) [10 dp apart at zoom=1.0], OffCurve(3) at (114, 120), OnCurve(4) at (200, 120)
    // Tangents at (100,100)->(100,110) [vertical] and (200,120)->(114,120) [horizontal] intersect at (100, 120), creating a real Tunni line!
    state_h.viewport =
        grafia::ViewportTransform::new(1.0, grafia::ScreenPoint::new(50.0, 500.0), 1000);
    let custom_cubic = GlyphContour::new(
        ContourId(0),
        vec![
            Node::new(NodeId(1), 100.0, 100.0, NodeType::Smooth),
            Node::new(NodeId(2), 100.0, 110.0, NodeType::OffCurve),
            Node::new(NodeId(3), 114.0, 120.0, NodeType::OffCurve),
            Node::new(NodeId(4), 200.0, 120.0, NodeType::Smooth),
        ],
    );
    set_layer_contours(&mut state_h, vec![custom_cubic]);
    assert!(
        !state_h.compute_tunni_overlays().is_empty(),
        "Non-collinear cubic arch must produce a live Tunni line overlay"
    );
    // Even if NodeId(3) is already selected, touching at (102.8, 112.0) (~3.4 dp from OffCurve(2) RIGHT ON the Tunni line from (100,110) to (114,120))
    // must select and drag OffCurve(2), NOT the Tunni line or OnCurve(1)!
    state_h.selected_nodes = vec![NodeId(3), NodeId(1)];
    let probe_scr = state_h.viewport.to_screen(FontPoint::new(102.8, 112.0));
    assert_eq!(
        state_h.hit_test_node(probe_scr),
        Some(NodeId(2)),
        "Closest OffCurve handle NodeId(2) must beat both nearby OnCurve NodeId(1) and farther selected OffCurve NodeId(3)"
    );
    let drag_to_scr = state_h.viewport.to_screen(FontPoint::new(102.8, 150.0));
    let raw_h_drag = RawInput {
        events: vec![
            egui::Event::Touch {
                device_id: egui::TouchDeviceId(1),
                id: egui::TouchId(2),
                phase: egui::TouchPhase::Start,
                pos: egui::Pos2::new(probe_scr.x.0, probe_scr.y.0),
                force: Some(0.5),
            },
            egui::Event::Touch {
                device_id: egui::TouchDeviceId(1),
                id: egui::TouchId(2),
                phase: egui::TouchPhase::Move,
                pos: egui::Pos2::new(drag_to_scr.x.0, drag_to_scr.y.0),
                force: Some(0.5),
            },
            egui::Event::Touch {
                device_id: egui::TouchDeviceId(1),
                id: egui::TouchId(2),
                phase: egui::TouchPhase::End,
                pos: egui::Pos2::new(drag_to_scr.x.0, drag_to_scr.y.0),
                force: Some(0.0),
            },
        ],
        ..Default::default()
    };
    state_h.selected_nodes = vec![NodeId(3)];
    let mut out_h = ctx.run_ui(raw_h_drag, |ui| {
        let _ = GrafiaCanvasWidget::new(&mut state_h, &theme).show(ui);
    });
    out_h.textures_delta.clear();
    assert_eq!(state_h.selected_nodes, vec![NodeId(2)]);
    assert!(
        state_h.find_node(NodeId(2)).unwrap().point.y.0 > 135.0,
        "Touch-dragging on Tunni line 3.4 dp from OffCurve(2) must move OffCurve handle NodeId(2), not Tunni line or OnCurve(1)"
    );
    assert_relative_eq!(
        state_h.find_node(NodeId(3)).unwrap().point.x.0,
        114.0,
        epsilon = 1e-4
    );
    assert_relative_eq!(
        state_h.find_node(NodeId(1)).unwrap().point.y.0,
        100.0,
        epsilon = 1e-4
    );
    // Double-tapping OnCurve NodeId(1) at (100, 100) when OffCurve(2) is 10 dp away must toggle NodeId(1) Smooth -> Corner!
    assert!(state_h.undo());
    let n1_scr = state_h.viewport.to_screen(FontPoint::new(100.0, 100.0));
    let n1_pos = egui::Pos2::new(n1_scr.x.0, n1_scr.y.0 + 9.0);
    for (t_sec, tid) in [(3.0, 20), (3.15, 21)] {
        let mut out_dt = ctx.run_ui(
            RawInput {
                time: Some(t_sec),
                events: vec![
                    egui::Event::Touch {
                        device_id: egui::TouchDeviceId(1),
                        id: egui::TouchId(tid),
                        phase: egui::TouchPhase::Start,
                        pos: n1_pos,
                        force: Some(0.5),
                    },
                    egui::Event::Touch {
                        device_id: egui::TouchDeviceId(1),
                        id: egui::TouchId(tid),
                        phase: egui::TouchPhase::End,
                        pos: n1_pos,
                        force: Some(0.0),
                    },
                ],
                ..Default::default()
            },
            |ui| {
                let _ = GrafiaCanvasWidget::new(&mut state_h, &theme).show(ui);
            },
        );
        out_dt.textures_delta.clear();
    }
    assert_eq!(
        state_h.find_node(NodeId(1)).unwrap().node_type,
        NodeType::Corner,
        "Double-tapping 9.0 dp from OnCurve NodeId(1) in TouchFinger mode must toggle Smooth -> Corner without being shadowed by nearby OffCurve"
    );

    // 3. TC-V0130-011: Universal Touch Hit-Radii (>= 14.0 dp) for Segments (with 4 dp tap roll), Anchors, and Guides.
    let mut state_seg = GrafiaCanvasState::init_glyph_a();
    state_seg.set_active_input_device(InputDeviceKind::TouchFinger);
    state_seg.selected_nodes.clear();
    // Bottom bar of /A is from (160, 0) to (30, 0); midpoint is (95, 0). Tap 11.0 dp below it with 4.0 dp finger roll!
    let seg_mid_scr = state_seg.viewport.to_screen(FontPoint::new(95.0, 0.0));
    let tap_start = egui::Pos2::new(seg_mid_scr.x.0, seg_mid_scr.y.0 + 11.0);
    let tap_roll = egui::Pos2::new(seg_mid_scr.x.0 + 3.0, seg_mid_scr.y.0 + 13.5);
    let raw_seg_tap = RawInput {
        events: vec![
            egui::Event::Touch {
                device_id: egui::TouchDeviceId(1),
                id: egui::TouchId(3),
                phase: egui::TouchPhase::Start,
                pos: tap_start,
                force: Some(0.5),
            },
            egui::Event::Touch {
                device_id: egui::TouchDeviceId(1),
                id: egui::TouchId(3),
                phase: egui::TouchPhase::Move,
                pos: tap_roll,
                force: Some(0.5),
            },
            egui::Event::Touch {
                device_id: egui::TouchDeviceId(1),
                id: egui::TouchId(3),
                phase: egui::TouchPhase::End,
                pos: tap_roll,
                force: Some(0.0),
            },
        ],
        ..Default::default()
    };
    let mut out_seg = ctx.run_ui(raw_seg_tap, |ui| {
        let _ = GrafiaCanvasWidget::new(&mut state_seg, &theme).show(ui);
    });
    out_seg.textures_delta.clear();
    assert_eq!(
        state_seg.selected_nodes.len(),
        2,
        "Touch tap 11.0 dp from segment with 4 dp finger roll must select the segment's 2 endpoint nodes"
    );

    // Touch hit-testing on CanvasAnchor and Guide at 11.0 dp offset
    state_seg.set_anchors(vec![CanvasAnchor {
        id: AnchorId(1),
        name: "top".into(),
        position: FontPoint::new(300.0, 700.0),
        selected: false,
    }]);
    let anchor_scr = state_seg.viewport.to_screen(FontPoint::new(300.0, 700.0));
    let anchor_probe_upm = state_seg.viewport.to_font(grafia::ScreenPoint::new(
        anchor_scr.x.0 + 11.0,
        anchor_scr.y.0,
    ));
    assert!(
        state_seg.select_anchor_at_upm(anchor_probe_upm, false),
        "TouchFinger must hit-test CanvasAnchor at 11.0 dp"
    );

    let gid = state_seg.add_local_guide(FontPoint::new(300.0, 350.0), 0.0);
    let guide_scr = state_seg.viewport.to_screen(FontPoint::new(300.0, 350.0));
    let guide_hit = state_seg.hit_test_guides(grafia::ScreenPoint::new(
        guide_scr.x.0,
        guide_scr.y.0 + 11.0,
    ));
    assert!(
        guide_hit.is_some(),
        "TouchFinger must hit-test Guide at 11.0 dp"
    );
    let _ = gid;

    // Touch hit-testing on Bounding Box Knobs (`hit_test_bbox_knob`) at 19.0 dp lateral offset from knob center (where Mouse 15.0 dp misses)
    state_seg.select_all_nodes();
    state_seg.show_bounding_box = true;
    let bounds = state_seg.selection_bounds().unwrap();
    let knob5_scr = bounds.knob_screen_center(5, &state_seg.viewport);
    let bbox_probe = grafia::ScreenPoint::new(knob5_scr.x.0, knob5_scr.y.0 + 19.0);
    state_seg.set_active_input_device(InputDeviceKind::Mouse);
    assert!(
        state_seg
            .hit_test_bbox_knob(bbox_probe, false, false)
            .is_none(),
        "Mouse mode must miss BBox knob at 19.0 dp lateral offset"
    );
    state_seg.set_active_input_device(InputDeviceKind::TouchFinger);
    assert!(
        state_seg
            .hit_test_bbox_knob(bbox_probe, false, false)
            .is_some(),
        "TouchFinger mode must hit-test BBox knob at 19.0 dp lateral offset"
    );

    // 4. TC-V0130-012: Double-tapping (with 12.0 dp finger slop between taps) on a straight Line segment converts it to a Cubic with 2 OffCurve handles selected!
    let mut state_dbl = GrafiaCanvasState::init_glyph_a();
    state_dbl.fit_viewport_to_glyph(760.0, 520.0);
    state_dbl.set_active_input_device(InputDeviceKind::TouchFinger);
    let snap_before_convert = state_dbl.snapshot();
    let count_before_convert = state_dbl.contours[0].len();
    let line_mid_scr = state_dbl.viewport.to_screen(FontPoint::new(95.0, 0.0));
    let tap1 = egui::Pos2::new(line_mid_scr.x.0, line_mid_scr.y.0);
    let tap2 = egui::Pos2::new(line_mid_scr.x.0 + 12.0, line_mid_scr.y.0 + 3.0);

    let raw_tap1_ptr = RawInput {
        time: Some(1.000),
        events: vec![egui::Event::PointerButton {
            pos: tap1,
            button: egui::PointerButton::Primary,
            pressed: true,
            modifiers: egui::Modifiers::NONE,
        }],
        ..Default::default()
    };
    let mut out_t1a = ctx.run_ui(raw_tap1_ptr, |ui| {
        let _ = GrafiaCanvasWidget::new(&mut state_dbl, &theme).show(ui);
    });
    out_t1a.textures_delta.clear();

    // 75ms later in the next frame (when WebGL frame rendering takes > 50ms), browser fires touchstart(Start) + synthesized PointerButton(pressed: true) for the SAME unreleased physical finger press!
    let raw_tap1 = RawInput {
        time: Some(1.075),
        events: vec![
            egui::Event::Touch {
                device_id: egui::TouchDeviceId(1),
                id: egui::TouchId(10),
                phase: egui::TouchPhase::Start,
                pos: tap1,
                force: Some(0.5),
            },
            egui::Event::PointerButton {
                pos: tap1,
                button: egui::PointerButton::Primary,
                pressed: true,
                modifiers: egui::Modifiers::NONE,
            },
            egui::Event::Touch {
                device_id: egui::TouchDeviceId(1),
                id: egui::TouchId(10),
                phase: egui::TouchPhase::End,
                pos: tap1,
                force: Some(0.0),
            },
            egui::Event::PointerButton {
                pos: tap1,
                button: egui::PointerButton::Primary,
                pressed: false,
                modifiers: egui::Modifiers::NONE,
            },
        ],
        ..Default::default()
    };
    let mut out_t1 = ctx.run_ui(raw_tap1, |ui| {
        let _ = GrafiaCanvasWidget::new(&mut state_dbl, &theme).show(ui);
    });
    out_t1.textures_delta.clear();
    assert_eq!(
        state_dbl.contours[0].len(),
        count_before_convert,
        "pointerdown at t=1.000 followed 7ms later by touchstart+PointerButton at t=1.007 is ONE physical tap and must NOT trigger a double-tap"
    );
    let snap_after_tap1 = state_dbl.snapshot();

    let raw_tap2 = RawInput {
        time: Some(1.18),
        events: vec![
            egui::Event::PointerButton {
                pos: tap2,
                button: egui::PointerButton::Primary,
                pressed: true,
                modifiers: egui::Modifiers::NONE,
            },
            egui::Event::Touch {
                device_id: egui::TouchDeviceId(1),
                id: egui::TouchId(11),
                phase: egui::TouchPhase::Start,
                pos: tap2,
                force: Some(0.5),
            },
            egui::Event::PointerButton {
                pos: tap2,
                button: egui::PointerButton::Primary,
                pressed: true,
                modifiers: egui::Modifiers::NONE,
            },
            egui::Event::Touch {
                device_id: egui::TouchDeviceId(1),
                id: egui::TouchId(11),
                phase: egui::TouchPhase::End,
                pos: tap2,
                force: Some(0.0),
            },
            egui::Event::PointerButton {
                pos: tap2,
                button: egui::PointerButton::Primary,
                pressed: false,
                modifiers: egui::Modifiers::NONE,
            },
        ],
        ..Default::default()
    };
    let mut out_t2 = ctx.run_ui(raw_tap2, |ui| {
        let _ = GrafiaCanvasWidget::new(&mut state_dbl, &theme).show(ui);
    });
    out_t2.textures_delta.clear();

    assert_eq!(
        state_dbl.contours[0].len(),
        count_before_convert + 2,
        "Double-tapping a straight Line segment in TouchFinger mode must convert it to a Cubic with 2 OffCurve handles"
    );
    assert_eq!(
        state_dbl.history.last_undo_label(),
        Some("Convert Line to Cubic")
    );
    assert_eq!(
        state_dbl.selected_nodes.len(),
        2,
        "Both newly created OffCurve handles must be selected immediately after double-tap conversion"
    );
    for &hid in &state_dbl.selected_nodes {
        assert_eq!(
            state_dbl.find_node(hid).unwrap().node_type,
            NodeType::OffCurve
        );
    }
    let snap_after_convert = state_dbl.snapshot();
    // Touch-dragging one of the 2 newly selected OffCurve handles must select and move ONLY that handle!
    let h1_id = state_dbl.selected_nodes[0];
    let h2_id = state_dbl.selected_nodes[1];
    let h1_before = state_dbl.find_node(h1_id).unwrap().point;
    let h2_before = state_dbl.find_node(h2_id).unwrap().point;
    let h1_scr = state_dbl.viewport.to_screen(h1_before);
    let h1_drag_scr = egui::Pos2::new(h1_scr.x.0, h1_scr.y.0 - 30.0);
    let raw_drag_h1 = RawInput {
        time: Some(2.0),
        events: vec![
            egui::Event::Touch {
                device_id: egui::TouchDeviceId(1),
                id: egui::TouchId(12),
                phase: egui::TouchPhase::Start,
                pos: egui::Pos2::new(h1_scr.x.0, h1_scr.y.0),
                force: Some(0.5),
            },
            egui::Event::Touch {
                device_id: egui::TouchDeviceId(1),
                id: egui::TouchId(12),
                phase: egui::TouchPhase::Move,
                pos: h1_drag_scr,
                force: Some(0.5),
            },
            egui::Event::Touch {
                device_id: egui::TouchDeviceId(1),
                id: egui::TouchId(12),
                phase: egui::TouchPhase::End,
                pos: h1_drag_scr,
                force: Some(0.0),
            },
        ],
        ..Default::default()
    };
    let mut out_dh1 = ctx.run_ui(raw_drag_h1, |ui| {
        let _ = GrafiaCanvasWidget::new(&mut state_dbl, &theme).show(ui);
    });
    out_dh1.textures_delta.clear();
    assert_eq!(state_dbl.selected_nodes, vec![h1_id]);
    assert!(state_dbl.find_node(h1_id).unwrap().point.y.0 > h1_before.y.0 + 20.0);
    assert_eq!(state_dbl.find_node(h2_id).unwrap().point, h2_before);
    assert!(state_dbl.undo());
    assert!(
        state_dbl.undo()
            && state_dbl.contours == snap_before_convert.contours
            && state_dbl.snapshot() == snap_after_tap1
    );
    assert!(state_dbl.redo() && state_dbl.snapshot() == snap_after_convert);
}
