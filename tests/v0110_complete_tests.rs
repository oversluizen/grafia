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
//! Normative COTS Verification Suite for `SPEC-012` (`grafia v0.11.0`):
//! Non-Destructive Smart Corners (`radius_upm > 0.0`), Ink Traps (`radius_upm < 0.0`),
//! Pressure & Tilt Calligraphic Brush, Live Curvature Comb & Heatmap Envelope / Inflection Markers,
//! Orthogonal Stem Thickness Gauge, Green's Theorem Counter Balance & Centroid,
//! and Superellipse / Optical Symmetry Ghost Overlay (`TC-V0110-001` .. `TC-V0110-016`).

use approx::assert_relative_eq;
use arbtest::arbtest;
use grafia::adapters::headless::rasterize_canvas_subregion;
use grafia::{
    build_superellipse_contour_nodes, compute_contour_area_and_centroid, contour_signed_area,
    find_cubic_inflection_points, fit_calligraphic_brush_stroke, AppearanceMode,
    CalligraphicNibSpec, CalligraphicSample, ContourId, CurvatureCombStyle, FontPoint,
    GlyphContour, GrafiaCanvasState, GrafiaTheme, Node, NodeId, NodeType, SmartCornerSpec,
    StemGaugeMatch, SymmetryOverlayMode,
};
use proptest::prelude::*;
use rstest::rstest;

fn make_square_contour(id: usize, base_node: u64, size: f64) -> GlyphContour {
    GlyphContour::new(
        ContourId(id),
        vec![
            Node::new(NodeId(base_node), 0.0, 0.0, NodeType::Corner),
            Node::new(NodeId(base_node + 1), size, 0.0, NodeType::Corner),
            Node::new(NodeId(base_node + 2), size, size, NodeType::Corner),
            Node::new(NodeId(base_node + 3), 0.0, size, NodeType::Corner),
        ],
    )
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

fn make_quarter_circle_contour(id: usize) -> GlyphContour {
    let k = 0.552_284_75_f64;
    let mut gc = GlyphContour::new(
        ContourId(id),
        vec![
            Node::new(NodeId(100), 100.0, 0.0, NodeType::Corner),
            Node::new(NodeId(101), 100.0, 100.0 * k, NodeType::OffCurve),
            Node::new(NodeId(102), 100.0 * k, 100.0, NodeType::OffCurve),
            Node::new(NodeId(103), 0.0, 100.0, NodeType::Smooth),
        ],
    );
    gc.closed = false;
    gc
}

fn set_layer_contours(state: &mut GrafiaCanvasState, contours: Vec<GlyphContour>) {
    state.contours = contours.iter().map(|c| c.nodes.clone()).collect();
    state.active_contour_closed = contours.last().is_none_or(|c| c.closed);
    state.glyph.contours = contours;
    state.sync_paths_from_contours();
}

// TC-V0110-001 & TC-V0110-002: Non-destructive Smart Corner fillet (radius > 0) & max-radius clamp (0.48 * min_edge).
#[rstest]
#[case::fillet_20(20.0, 20.0)]
#[case::fillet_50(50.0, 50.0)]
#[case::fillet_clamped(500.0, 96.0)]
fn tc_v0110_001_and_002_smart_corner_fillet_and_edge_clamp(
    #[case] radius_upm: f64,
    #[case] expected_trim: f64,
) {
    let mut state = GrafiaCanvasState::init_glyph_a();
    set_layer_contours(&mut state, vec![make_square_contour(0, 10, 200.0)]);
    state.set_smart_corner(SmartCornerSpec {
        node_id: NodeId(10),
        radius_upm,
    });
    assert_eq!(state.glyph.contours[0].nodes.len(), 4);

    let preview = state.compute_smart_corner_preview_contours();
    assert_eq!(preview.len(), 1);
    assert_eq!(preview[0].nodes.len(), 7);
    assert!(preview[0]
        .nodes
        .iter()
        .any(|n| (n.point.x.0 - expected_trim).abs() < 1e-2 && n.point.y.0.abs() < 1e-2));
    assert!(preview[0]
        .nodes
        .iter()
        .any(|n| n.point.x.0.abs() < 1e-2 && (n.point.y.0 - expected_trim).abs() < 1e-2));
}

// TC-V0110-003 & TC-V0110-004: Non-destructive Ink Trap notch (radius < 0) and destructive bake_smart_corners() with Undo.
#[test]
fn tc_v0110_003_and_004_ink_trap_notch_and_bake_smart_corners_with_undo() {
    let mut state = GrafiaCanvasState::init_glyph_a();
    set_layer_contours(&mut state, vec![make_square_contour(0, 10, 200.0)]);
    let sharp_area = contour_signed_area(&state.glyph.contours[0].nodes).abs();

    state.set_smart_corner(SmartCornerSpec {
        node_id: NodeId(10),
        radius_upm: -25.0,
    });
    let trap_preview = state.compute_smart_corner_preview_contours();
    assert_eq!(trap_preview.len(), 1);
    assert!(trap_preview[0].nodes.len() >= 6);
    assert!(contour_signed_area(&trap_preview[0].nodes).abs() < sharp_area);

    state.set_smart_corner(SmartCornerSpec {
        node_id: NodeId(11),
        radius_upm: 30.0,
    });
    let snap_before_bake = state.snapshot();
    state.bake_smart_corners();
    assert!(state.smart_corners.is_empty() && state.glyph.contours[0].nodes.len() >= 9);
    let snap_after_bake = state.snapshot();

    assert!(state.undo() && state.snapshot() == snap_before_bake);
    assert_eq!(state.glyph.contours[0].nodes.len(), 4);
    assert_eq!(state.smart_corners.len(), 2);
    assert!(state.redo() && state.snapshot() == snap_after_bake);
    state.validate_invariants().unwrap();

    // Corner flanked by a zero-length retracted OffCurve handle should still fillet cleanly.
    let retracted_cubic_square = GlyphContour::new(
        ContourId(0),
        vec![
            Node::new(NodeId(50), 0.0, 0.0, NodeType::Corner),
            Node::new(NodeId(51), 0.0, 0.0, NodeType::OffCurve),
            Node::new(NodeId(52), 140.0, 0.0, NodeType::OffCurve),
            Node::new(NodeId(53), 200.0, 0.0, NodeType::Corner),
            Node::new(NodeId(54), 200.0, 200.0, NodeType::Corner),
            Node::new(NodeId(55), 0.0, 200.0, NodeType::Corner),
        ],
    );
    set_layer_contours(&mut state, vec![retracted_cubic_square]);
    state.set_smart_corner(SmartCornerSpec {
        node_id: NodeId(50),
        radius_upm: 20.0,
    });
    let retracted_prev = state.compute_smart_corner_preview_contours();
    assert!(retracted_prev[0].nodes.len() > 6);
    state.bake_smart_corners();
    state.validate_invariants().unwrap();
}

// TC-V0110-005, TC-V0110-006 & TC-V0110-007: Calligraphic Brush pressure width, tilt modulation & canvas Undo.
#[test]
fn tc_v0110_005_to_007_calligraphic_brush_pressure_tilt_and_undo() {
    let nib = CalligraphicNibSpec {
        min_width_upm: 20.0,
        max_width_upm: 80.0,
        base_angle_deg: 90.0,
        tilt_influence: 0.5,
    };
    let samples = vec![
        CalligraphicSample {
            pos: FontPoint::new(0.0, 200.0),
            pressure: 0.0,
            tilt_x: 0.0,
            tilt_y: 0.0,
        },
        CalligraphicSample {
            pos: FontPoint::new(150.0, 200.0),
            pressure: 0.5,
            tilt_x: 0.0,
            tilt_y: 0.0,
        },
        CalligraphicSample {
            pos: FontPoint::new(300.0, 200.0),
            pressure: 1.0,
            tilt_x: 0.0,
            tilt_y: 0.0,
        },
    ];
    let c0 = fit_calligraphic_brush_stroke(nib, &samples).expect("valid stroke");
    assert!(c0.closed);
    let start_ys: Vec<f64> = c0
        .nodes
        .iter()
        .filter(|n| n.point.x.0.abs() < 1.0)
        .map(|n| n.point.y.0)
        .collect();
    let end_ys: Vec<f64> = c0
        .nodes
        .iter()
        .filter(|n| (n.point.x.0 - 300.0).abs() < 1.0)
        .map(|n| n.point.y.0)
        .collect();
    let span_start = start_ys.iter().copied().fold(f64::NEG_INFINITY, f64::max)
        - start_ys.iter().copied().fold(f64::INFINITY, f64::min);
    let span_end = end_ys.iter().copied().fold(f64::NEG_INFINITY, f64::max)
        - end_ys.iter().copied().fold(f64::INFINITY, f64::min);
    assert_relative_eq!(span_start, 20.0, epsilon = 1.0);
    assert_relative_eq!(span_end, 80.0, epsilon = 1.0);

    let tilted = vec![
        CalligraphicSample {
            pos: FontPoint::new(0.0, 200.0),
            pressure: -0.5,
            tilt_x: 45.0,
            tilt_y: 45.0,
        },
        CalligraphicSample {
            pos: FontPoint::new(300.0, 200.0),
            pressure: 1.8,
            tilt_x: 45.0,
            tilt_y: 45.0,
        },
    ];
    let c_tilt = fit_calligraphic_brush_stroke(nib, &tilted).expect("tilted stroke");
    assert_ne!(c0.nodes[0].point, c_tilt.nodes[0].point);
    assert!(fit_calligraphic_brush_stroke(nib, &samples[..1]).is_none());

    let mut state = GrafiaCanvasState::init_glyph_a();
    let base_count = state.glyph.contours.len();
    let snap_0 = state.snapshot();
    assert!(state
        .apply_calligraphic_brush_stroke(nib, &samples)
        .is_some());
    assert_eq!(state.glyph.contours.len(), base_count + 1);
    let snap_1 = state.snapshot();
    assert!(
        state.undo() && state.snapshot() == snap_0 && state.redo() && state.snapshot() == snap_1
    );

    // Curved calligraphic stroke must fit non-collinear Catmull-Rom OffCurve handles along the curve tangent!
    let curved_samples = vec![
        CalligraphicSample {
            pos: FontPoint::new(0.0, 0.0),
            pressure: 0.5,
            tilt_x: 0.0,
            tilt_y: 0.0,
        },
        CalligraphicSample {
            pos: FontPoint::new(100.0, 120.0),
            pressure: 0.5,
            tilt_x: 0.0,
            tilt_y: 0.0,
        },
        CalligraphicSample {
            pos: FontPoint::new(200.0, 0.0),
            pressure: 0.5,
            tilt_x: 0.0,
            tilt_y: 0.0,
        },
    ];
    let c_arc = fit_calligraphic_brush_stroke(nib, &curved_samples).expect("curved stroke");
    // At the middle Smooth node on the left rail (index 3), its outgoing OffCurve handle (index 4) must be horizontal (dy ≈ 0), not collinear with the chord to rail[2]!
    assert_eq!(c_arc.nodes[3].node_type, NodeType::Smooth);
    assert_eq!(c_arc.nodes[4].node_type, NodeType::OffCurve);
    assert!(
        (c_arc.nodes[4].point.y.0 - c_arc.nodes[3].point.y.0).abs() < 5.0,
        "Catmull-Rom tangent handle at arch peak must be horizontal rather than polygonal chord, got {:?}",
        c_arc.nodes[4].point
    );
}

// TC-V0110-008, TC-V0110-009, TC-V0110-010 & TC-V0110-013: Curvature Comb spines, Heatmap Envelope, inflection markers & rendering.
#[test]
fn tc_v0110_008_to_010_and_013_curvature_comb_heatmap_inflections_and_rendering() {
    let mut state = GrafiaCanvasState::init_glyph_a();
    set_layer_contours(&mut state, vec![make_quarter_circle_contour(0)]);
    state.toggle_show_curvature_comb();
    let combs = state.compute_curvature_comb_spines(16, 5000.0);
    assert_eq!(combs[0].len(), 17);
    let mid = &combs[0][8];
    assert_relative_eq!(
        (mid.tip_point.x.0 - mid.base_point.x.0).hypot(mid.tip_point.y.0 - mid.base_point.y.0),
        50.0,
        epsilon = 8.0
    );

    let mut s_curve = GlyphContour::new(
        ContourId(0),
        vec![
            Node::new(NodeId(1), 0.0, 0.0, NodeType::Corner),
            Node::new(NodeId(2), 50.0, 100.0, NodeType::OffCurve),
            Node::new(NodeId(3), 50.0, -100.0, NodeType::OffCurve),
            Node::new(NodeId(4), 100.0, 0.0, NodeType::Smooth),
        ],
    );
    s_curve.closed = false;
    set_layer_contours(&mut state, vec![s_curve]);
    let s_combs = state.compute_curvature_comb_spines(10, 2000.0);
    assert!(s_combs[0][0].signed_curvature * s_combs[0][10].signed_curvature < 0.0);
    assert_relative_eq!(s_combs[0][5].signed_curvature, 0.0, epsilon = 1e-5);

    // TC-V0110-013: Exact cubic inflection points & CurvatureCombStyle::HeatmapEnvelope.
    let infl = find_cubic_inflection_points(
        0,
        0,
        FontPoint::new(0.0, 0.0),
        FontPoint::new(50.0, 100.0),
        FontPoint::new(50.0, -100.0),
        FontPoint::new(100.0, 0.0),
    );
    assert_eq!(infl.len(), 1);
    assert_relative_eq!(infl[0].t, 0.5, epsilon = 1e-3);
    assert_relative_eq!(infl[0].point.x.0, 50.0, epsilon = 1e-2);
    assert_eq!(state.compute_layer_inflection_markers().len(), 1);
    state.set_curvature_comb_style(CurvatureCombStyle::HeatmapEnvelope);
    let env = state.compute_curvature_heatmap_envelopes(12, 3000.0);
    assert_eq!(env.len(), 1);
    assert!(env[0].closed_polygon.len() >= 26 && env[0].inflections.len() == 1);
    assert!(env[0]
        .samples
        .iter()
        .any(|s| (s.normalized_intensity - 1.0).abs() < 1e-4));
    assert!(
        env[0].samples.first().unwrap().polarity_sign
            * env[0].samples.last().unwrap().polarity_sign
            < 0
    );

    set_layer_contours(
        &mut state,
        vec![
            make_quarter_circle_contour(0),
            make_square_contour(1, 30, 180.0),
        ],
    );
    state.set_smart_corner(SmartCornerSpec {
        node_id: NodeId(30),
        radius_upm: 25.0,
    });
    state.set_smart_corner(SmartCornerSpec {
        node_id: NodeId(31),
        radius_upm: -20.0,
    });
    state.set_appearance_mode(AppearanceMode::Light);
    let light_px = rasterize_canvas_subregion(&state, &GrafiaTheme::sonoma_default(), 320, 180);
    state.set_appearance_mode(AppearanceMode::Dark);
    let dark_px = rasterize_canvas_subregion(&state, &GrafiaTheme::sonoma_dark(), 320, 180);
    assert_ne!(light_px.data(), dark_px.data());

    // HeatmapEnvelope must render filled intensity-modulated quad meshes in egui::Painter and tiny-skia
    state.set_appearance_mode(AppearanceMode::Light);
    let ctx_hm = egui::Context::default();
    let theme_hm = GrafiaTheme::sonoma_default();
    let mut out_hm = ctx_hm.run_ui(egui::RawInput::default(), |ui| {
        let _ = grafia::GrafiaCanvasWidget::new(&mut state, &theme_hm).show(ui);
    });
    out_hm.textures_delta.clear();
    let hm_mesh_count = out_hm
        .shapes
        .iter()
        .filter(|cs| matches!(&cs.shape, egui::Shape::Mesh(_)))
        .count();
    assert!(
        hm_mesh_count > 4,
        "CurvatureCombStyle::HeatmapEnvelope must render intensity-modulated filled quad meshes, got {hm_mesh_count}"
    );
}

// TC-V0110-014, TC-V0110-015 & TC-V0110-016: Orthogonal Stem Gauge, Green's Theorem Counter Balance & Superellipse / Symmetry Overlay.
#[test]
fn tc_v0110_014_to_016_stem_gauge_counter_balance_superellipse_and_symmetry() {
    let mut state = GrafiaCanvasState::init_glyph_a();
    state.set_reference_stems(vec![80.0], 2.0);
    set_layer_contours(
        &mut state,
        vec![
            make_rect_contour(0, 10, 0.0, 0.0, 81.0, 500.0, false),
            make_rect_contour(1, 20, 200.0, 0.0, 268.0, 500.0, false),
            make_rect_contour(2, 30, 400.0, 0.0, 494.0, 500.0, false),
        ],
    );
    let m_ok = state
        .measure_orthogonal_stem_thickness(0, 3, 0.5)
        .expect("stem 0");
    assert_relative_eq!(m_ok.p_entry.x.0, 0.0, epsilon = 1e-2);
    assert_relative_eq!(m_ok.p_exit.x.0, 81.0, epsilon = 1e-2);
    assert_relative_eq!(m_ok.thickness_upm, 81.0, epsilon = 1e-2);
    assert_eq!(m_ok.nearest_reference_upm, Some(80.0));
    assert_eq!(m_ok.gauge_match, StemGaugeMatch::Matched);
    assert_eq!(
        state
            .measure_orthogonal_stem_thickness(1, 3, 0.5)
            .unwrap()
            .gauge_match,
        StemGaugeMatch::Thinner
    );
    assert_eq!(
        state
            .measure_orthogonal_stem_thickness(2, 3, 0.5)
            .unwrap()
            .gauge_match,
        StemGaugeMatch::Thicker
    );

    // TC-V0110-015: Green's Theorem outer 200 by 300 (60,000) minus inner counter 100 by 150 (15,000) centered at (100, 150).
    set_layer_contours(
        &mut state,
        vec![
            make_rect_contour(0, 10, 0.0, 0.0, 200.0, 300.0, false),
            make_rect_contour(1, 20, 50.0, 75.0, 150.0, 225.0, true),
        ],
    );
    let (a0, c0) = compute_contour_area_and_centroid(&state.glyph.contours[0].nodes);
    assert_relative_eq!(a0.abs(), 60_000.0, epsilon = 1.0);
    assert_relative_eq!(c0.x.0, 100.0, epsilon = 1e-2);
    assert_relative_eq!(c0.y.0, 150.0, epsilon = 1e-2);
    let bal = state.compute_layer_counter_balance();
    assert_relative_eq!(bal.net_black_area_upm2, 45_000.0, epsilon = 1.0);
    assert_relative_eq!(bal.total_counter_area_upm2, 15_000.0, epsilon = 1.0);
    assert_relative_eq!(bal.black_to_white_ratio, 3.0, epsilon = 1e-3);
    assert_relative_eq!(bal.overall_centroid.x.0, 100.0, epsilon = 1e-2);
    let m_from_counter = state
        .measure_orthogonal_stem_thickness(1, 0, 0.5)
        .expect("stem from inner counter left wall");
    assert_relative_eq!(m_from_counter.thickness_upm, 50.0, epsilon = 1e-2);
    assert_relative_eq!(m_from_counter.p_exit.x.0, 0.0, epsilon = 1e-2);

    // Visual Centroid crosshair ⊕ and Live Orthogonal Stem Thickness Gauge rendering under hovered_segment_hit
    let theme_sg = GrafiaTheme::sonoma_default();
    state.fit_viewport_to_glyph(320.0, 240.0);
    let px_base = rasterize_canvas_subregion(&state, &theme_sg, 320, 240);
    state.set_show_visual_centroid(true);
    let px_centroid = rasterize_canvas_subregion(&state, &theme_sg, 320, 240);
    assert_ne!(
        px_base.data(),
        px_centroid.data(),
        "set_show_visual_centroid(true) must render the CounterBalanceReport::overall_centroid crosshair ⊕"
    );

    let segs_0 = grafia::extract_segments(&state.contours[0], true);
    state.hovered_segment_hit = Some(grafia::SegmentHit {
        contour_idx: 0,
        segment: segs_0[3],
        t: 0.5,
        point: FontPoint::new(0.0, 150.0),
        distance_font: 0.0,
    });
    state.set_show_stem_gauge(true);
    let px_gauge = rasterize_canvas_subregion(&state, &theme_sg, 320, 240);
    assert_ne!(
        px_centroid.data(),
        px_gauge.data(),
        "set_show_stem_gauge(true) must render the Live Orthogonal Stem Thickness Gauge under hovered_segment_hit"
    );
    state.set_show_stem_gauge(false);
    state.set_show_visual_centroid(false);

    // TC-V0110-016: Superellipse / Squircle generator & Optical Symmetry Ghost Overlay.
    let ell = build_superellipse_contour_nodes(
        (FontPoint::new(0.0, 0.0), FontPoint::new(200.0, 200.0)),
        2.0,
    );
    let sq = build_superellipse_contour_nodes(
        (FontPoint::new(0.0, 0.0), FontPoint::new(200.0, 200.0)),
        4.0,
    );
    assert_eq!(sq.len(), 12);
    assert!(
        compute_contour_area_and_centroid(&sq).0.abs()
            > compute_contour_area_and_centroid(&ell).0.abs()
    );
    state.set_symmetry_overlay_mode(SymmetryOverlayMode::Off);
    assert!(state.compute_symmetry_ghost_contours().is_empty());
    state.set_symmetry_overlay_mode(SymmetryOverlayMode::HorizontalAxis);
    assert_relative_eq!(
        state.compute_symmetry_ghost_contours()[0][0].point.y.0,
        300.0,
        epsilon = 1e-2
    );
    state.set_symmetry_overlay_mode(SymmetryOverlayMode::VerticalAxis);
    assert_relative_eq!(
        state.compute_symmetry_ghost_contours()[0][0].point.x.0,
        200.0,
        epsilon = 1e-2
    );
    state.set_symmetry_overlay_mode(SymmetryOverlayMode::PointCenter);
    let ghost_pt = state.compute_symmetry_ghost_contours();
    assert_eq!(ghost_pt.len(), 2);
    assert_relative_eq!(ghost_pt[0][0].point.x.0, 200.0, epsilon = 1e-2);
    assert_relative_eq!(ghost_pt[0][0].point.y.0, 300.0, epsilon = 1e-2);
}

proptest! {
    #[test]
    fn tc_v0110_011_prop_smart_corners_and_brush_finite(radius in -80.0f64..120.0f64, p0 in 0.0f64..1.0f64, p1 in 0.0f64..1.0f64) {
        let mut state = GrafiaCanvasState::init_glyph_a();
        set_layer_contours(&mut state, vec![make_square_contour(0, 10, 200.0)]);
        state.set_smart_corner(SmartCornerSpec { node_id: NodeId(10), radius_upm: radius });
        let prev = state.compute_smart_corner_preview_contours();
        prop_assert!(prev[0].nodes.iter().all(|n| n.point.x.0.is_finite() && n.point.y.0.is_finite()));
        let nib = CalligraphicNibSpec { min_width_upm: 10.0, max_width_upm: 60.0, base_angle_deg: 30.0, tilt_influence: 0.3 };
        let stroke = fit_calligraphic_brush_stroke(nib, &[
            CalligraphicSample { pos: FontPoint::new(0.0, 0.0), pressure: p0, tilt_x: 10.0, tilt_y: -10.0 },
            CalligraphicSample { pos: FontPoint::new(120.0, 80.0), pressure: p1, tilt_x: -10.0, tilt_y: 20.0 },
        ]).unwrap();
        prop_assert!(stroke.closed && stroke.nodes.iter().all(|n| n.point.x.0.is_finite() && n.point.y.0.is_finite()));
    }
}

// TC-V0110-012: Stateful pull-based `arbtest` fuzzing across Smart Corners, Ink Traps, Brush & Curvature Comb.
#[test]
fn tc_v0110_012_arbtest_stateful_smart_corners_brush_and_curvature_comb() {
    arbtest(|u| {
        let mut state = GrafiaCanvasState::init_glyph_a();
        set_layer_contours(
            &mut state,
            vec![
                make_square_contour(0, 10, 200.0),
                make_quarter_circle_contour(1),
            ],
        );
        for _ in 0..u.int_in_range(10..=20)? {
            match u.int_in_range(0..=5)? {
                0 => {
                    state.set_smart_corner(SmartCornerSpec {
                        node_id: NodeId(10),
                        radius_upm: f64::from(u.int_in_range(-40..=60)?),
                    });
                    let _ = state.compute_smart_corner_preview_contours();
                }
                1 => state.bake_smart_corners(),
                2 => {
                    let nib = CalligraphicNibSpec {
                        min_width_upm: 12.0,
                        max_width_upm: 48.0,
                        base_angle_deg: 45.0,
                        tilt_influence: 0.2,
                    };
                    let _ = state.apply_calligraphic_brush_stroke(
                        nib,
                        &[
                            CalligraphicSample {
                                pos: FontPoint::new(20.0, 40.0),
                                pressure: 0.3,
                                tilt_x: 0.0,
                                tilt_y: 0.0,
                            },
                            CalligraphicSample {
                                pos: FontPoint::new(180.0, 120.0),
                                pressure: 0.8,
                                tilt_x: 15.0,
                                tilt_y: 5.0,
                            },
                        ],
                    );
                }
                3 => state.toggle_show_curvature_comb(),
                4 => {
                    let _ = state.compute_curvature_comb_spines(8, 2000.0);
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
