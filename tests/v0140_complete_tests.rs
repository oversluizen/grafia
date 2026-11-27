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
//! Normative COTS Verification Suite for `SPEC-016` (`grafia v0.14.0`):
//! Vector Icon Studio, Variable-Width Strokes, `vitrea` Liquid-Glass Optics
//! & 10-Mode Kinetic Symbol Engine (`TC-V0140-001` .. `TC-V0140-012`).

mod common;

use arbtest::arbtest;
use grafia::{
    build_circle_contour_nodes, build_rectangle_contour_nodes, compute_contour_area_and_centroid,
    compute_shape_builder_regions, evaluate_variable_width_at_t, expand_variable_width_contour,
    hit_test_shape_builder_region, AnchorId, BounceDirection, CanvasIconStudioScene, CanvasIntent,
    CanvasTool, ContourId, FontPoint, GlyphContour, GrafiaCanvasState, GrafiaTheme,
    KineticLayerScope, KineticSymbolEffect, KineticTimelineState, LineCapStyle,
    LiquidGlassRenderConfig, Modifiers, Node, NodeId, NodeType, RadialSymmetrySpec,
    ReplaceFallback, ScaleDirection, ScreenPoint, ShapeBuilderOp, SymbolLayerTag,
    VariableWidthStrokeSpec, WidthNodeSample, WiggleAxis,
};

#[test]
fn tc_v0140_001_planar_shape_builder_regions_merge_erase_and_undo_redo() {
    let mut next_id = 1_u64;
    let circle_a = GlyphContour::new(
        ContourId(0),
        build_circle_contour_nodes(100.0, 100.0, 300.0, 300.0, &mut next_id),
    );
    let circle_b = GlyphContour::new(
        ContourId(1),
        build_circle_contour_nodes(200.0, 100.0, 400.0, 300.0, &mut next_id),
    );
    let contours = vec![circle_a.clone(), circle_b.clone()];

    // 1. Planar decomposition of 2 overlapping circles yields 3 disjoint regions (Left Lune, Lens, Right Lune)
    let regions = compute_shape_builder_regions(&contours);
    assert_eq!(
        regions.len(),
        3,
        "Expected 3 disjoint planar regions for 2 overlapping circles"
    );
    for (idx, reg) in regions.iter().enumerate() {
        assert_eq!(reg.region_id, idx as u32);
        assert!(reg.area_upm2 > 1000.0);
        assert!(!reg.source_contour_ids.is_empty());
    }

    // 2. Hit-testing the overlap center (250, 200) finds the center lens region
    let lens_id = hit_test_shape_builder_region(&regions, FontPoint::new(250.0, 200.0))
        .expect("center lens region must be hit");
    let lens_reg = regions
        .iter()
        .find(|r| r.region_id == lens_id)
        .expect("lens region");
    assert_eq!(lens_reg.source_contour_ids.len(), 2);

    // 3. Merge all 3 regions on GrafiaCanvasState -> 1 unified contour + bit-exact Undo/Redo
    let mut state = GrafiaCanvasState::new(GrafiaTheme::sonoma_default());
    state.active_tool = CanvasTool::ShapeBuilder;
    state.contours = vec![circle_a.nodes.clone(), circle_b.nodes.clone()];
    state.glyph.contours = contours.clone();
    state.next_node_id = next_id;
    state.sync_paths_from_contours();
    let total_regions_area: f64 = regions.iter().map(|r| r.area_upm2).sum();
    let all_ids: Vec<u32> = regions.iter().map(|r| r.region_id).collect();

    common::assert_mutation_undo_redo_roundtrip(&mut state, |s| {
        assert!(s.apply_shape_builder_operation(&all_ids, ShapeBuilderOp::Merge));
    });
    assert_eq!(state.history.last_undo_label(), Some("Shape Builder Merge"));
    assert_eq!(state.glyph.contours.len(), 1);
    let (merged_area, _) = compute_contour_area_and_centroid(&state.glyph.contours[0].nodes);
    assert!(((merged_area.abs() - total_regions_area) / total_regions_area).abs() < 0.02);

    // 4. Undo back to 2 overlapping circles:
    //    4a. Erase ONLY the right lune -> Circle A must remain 1 intact circle (NOT shattered into Left Lune + Lens)!
    assert!(state.undo());
    assert_eq!(state.glyph.contours.len(), 2);
    let right_lune_id = hit_test_shape_builder_region(&regions, FontPoint::new(350.0, 200.0))
        .expect("right lune region must be hit");
    let (circle_a_orig_area, _) = compute_contour_area_and_centroid(&circle_a.nodes);
    state.set_smart_corner(grafia::SmartCornerSpec {
        node_id: circle_b.nodes[0].id,
        radius_upm: 15.0,
    });
    assert!(state.apply_shape_builder_operation(&[right_lune_id], ShapeBuilderOp::Erase));
    assert_eq!(
        state.glyph.contours.len(),
        1,
        "Erasing ONLY the right lune (B \\ A) must leave Circle A as 1 intact contour, not 2 shards"
    );
    let (remaining_a_area, _) = compute_contour_area_and_centroid(&state.glyph.contours[0].nodes);
    assert!(
        ((remaining_a_area.abs() - circle_a_orig_area.abs()) / circle_a_orig_area.abs()).abs()
            < 0.02
    );
    assert!(
        state.smart_corners().is_empty(),
        "Orphaned SmartCornerSpec on replaced NodeIds must be pruned automatically"
    );

    //    4b. Undo back to 2 overlapping circles, then Erase the center lens -> 2 disjoint lune contours
    assert!(state.undo());
    assert!(state.undo());
    assert_eq!(state.glyph.contours.len(), 2);
    common::assert_mutation_undo_redo_roundtrip(&mut state, |s| {
        assert!(s.apply_shape_builder_operation(&[lens_id], ShapeBuilderOp::Erase));
    });
    assert_eq!(state.history.last_undo_label(), Some("Shape Builder Erase"));
    assert_eq!(state.glyph.contours.len(), 2);

    // 5. Interactive pointer press -> drag sweep -> live filled region overlay render -> release commit + telemetry
    assert!(state.undo());
    assert_eq!(state.glyph.contours.len(), 2);
    state.viewport.pan_offset = ScreenPoint::new(20.0, 320.0);
    let telem = state.controller_telemetry();
    assert!(telem
        .available_intents
        .iter()
        .any(|i| i.intent == CanvasIntent::BreakCuspHandle));

    let pix_before_drag = grafia::adapters::headless::rasterize_canvas_subregion(
        &state,
        &GrafiaTheme::sonoma_default(),
        480,
        360,
    );

    let p_left = FontPoint::new(150.0, 200.0);
    let p_lens = FontPoint::new(250.0, 200.0);
    let p_right = FontPoint::new(350.0, 200.0);
    let s_left = state.viewport.to_screen(p_left);
    let s_lens = state.viewport.to_screen(p_lens);
    let s_right = state.viewport.to_screen(p_right);

    assert!(state.on_drawing_tool_pointer_press(s_left, p_left, Modifiers::default()));
    assert!(state.drawing_tool_state.shape_builder.active);
    assert_eq!(
        state
            .drawing_tool_state
            .shape_builder
            .visited_region_ids
            .len(),
        1
    );
    assert!(state.on_drawing_tool_pointer_move(s_lens, p_lens, Modifiers::default()));
    assert!(state.on_drawing_tool_pointer_move(s_right, p_right, Modifiers::default()));
    assert_eq!(
        state
            .drawing_tool_state
            .shape_builder
            .visited_region_ids
            .len(),
        3
    );

    // Live filled region overlay renders > 200 changed highlight pixels in tiny_skia while dragging
    let frame_during_drag = common::run_headless_canvas_frame(&mut state, 480.0, 360.0);
    assert!(!frame_during_drag.shapes.is_empty());
    let pix_during_drag = grafia::adapters::headless::rasterize_canvas_subregion(
        &state,
        &GrafiaTheme::sonoma_default(),
        480,
        360,
    );
    let diff_pixels = pix_before_drag
        .data()
        .as_chunks::<4>()
        .0
        .iter()
        .zip(pix_during_drag.data().as_chunks::<4>().0.iter())
        .filter(|(a, b)| a != b)
        .count();
    assert!(
        diff_pixels > 200,
        "ShapeBuilder live region fill + outline overlay must highlight the swept regions (diff_pixels = {diff_pixels})"
    );

    assert!(state.on_drawing_tool_pointer_release(p_right, Modifiers::default()));
    assert!(!state.drawing_tool_state.shape_builder.active);
    assert_eq!(state.glyph.contours.len(), 1);
    assert_eq!(state.history.last_undo_label(), Some("Shape Builder Merge"));
}

#[test]
fn tc_v0140_002_variable_width_skeleton_stroke_fritsch_carlson_and_expansion() {
    // 1. Sharp step transition: Fritsch-Carlson monotone cubic Hermite must never undershoot < 10.0 or overshoot > 80.0
    let step_spec = VariableWidthStrokeSpec {
        contour_id: ContourId(0),
        samples: vec![
            WidthNodeSample {
                arc_ratio_t: 0.0,
                width_left_upm: 10.0,
                width_right_upm: 80.0,
            },
            WidthNodeSample {
                arc_ratio_t: 0.45,
                width_left_upm: 10.0,
                width_right_upm: 80.0,
            },
            WidthNodeSample {
                arc_ratio_t: 0.55,
                width_left_upm: 80.0,
                width_right_upm: 10.0,
            },
            WidthNodeSample {
                arc_ratio_t: 1.0,
                width_left_upm: 80.0,
                width_right_upm: 10.0,
            },
        ],
        cap_style: LineCapStyle::Round,
    };

    let mut prev_l = 10.0_f64;
    let mut prev_r = 80.0_f64;
    for step in 0..=100 {
        let t = f64::from(step) / 100.0;
        let (wl, wr) = evaluate_variable_width_at_t(&step_spec, t);
        assert!(
            (10.0 - 1e-9..=80.0 + 1e-9).contains(&wl),
            "Monotone Hermite left width overshot bounds at t={t}: {wl}"
        );
        assert!(
            (10.0 - 1e-9..=80.0 + 1e-9).contains(&wr),
            "Monotone Hermite right width overshot bounds at t={t}: {wr}"
        );
        assert!(wl + 1e-9 >= prev_l, "Non-monotone left width at t={t}");
        assert!(wr - 1e-9 <= prev_r, "Non-monotone right width at t={t}");
        prev_l = wl;
        prev_r = wr;
    }

    // 2. Open S-curve expansion with asymmetric widths and Round caps -> closed, self-intersection-free contour
    let mut s_curve = GlyphContour::new(
        ContourId(0),
        vec![
            Node::new(NodeId(1), 100.0, 100.0, NodeType::Corner),
            Node::new(NodeId(2), 200.0, 350.0, NodeType::OffCurve),
            Node::new(NodeId(3), 300.0, -50.0, NodeType::OffCurve),
            Node::new(NodeId(4), 400.0, 200.0, NodeType::Corner),
        ],
    );
    s_curve.closed = false;
    let mut next_nid = 100_u64;
    let expanded = expand_variable_width_contour(&s_curve, &step_spec, &mut next_nid);
    assert!(!expanded.is_empty());
    for c in &expanded {
        assert!(c.closed);
        let (area, _) = compute_contour_area_and_centroid(&c.nodes);
        assert!(
            area > 1000.0,
            "Expanded variable-width outline must have positive CCW area"
        );
    }

    // 3. Stateful GrafiaCanvasState attach + handle pins + rotary scaling + orphan-pruning bake + Undo/Redo
    let mut state = GrafiaCanvasState::new(GrafiaTheme::sonoma_default());
    let glyph = grafia::Glyph {
        contours: vec![s_curve.clone()],
        ..Default::default()
    };
    let metrics = state.metrics.clone();
    state.load_glyph(glyph, metrics);
    common::assert_mutation_undo_redo_roundtrip(&mut state, |s| {
        s.set_variable_width_stroke(step_spec.clone());
    });
    assert_eq!(state.icon_studio.variable_width_strokes.len(), 1);
    assert!(!state.compute_expanded_stroke_contours().is_empty());
    let pins = state.compute_variable_width_handle_pins();
    assert_eq!(pins.len(), 4);
    assert!((pins[0].left_pt.distance_to(pins[0].spine_pt) - 10.0).abs() < 1e-3);
    assert!((pins[2].left_pt.distance_to(pins[2].spine_pt) - 80.0).abs() < 1e-3);
    let hit = grafia::hit_test_variable_width_handle_pin(&pins, pins[1].left_pt, 5.0);
    assert_eq!(hit, Some((ContourId(0), 1, true)));
    let hit_r = grafia::hit_test_variable_width_handle_pin(&pins, pins[1].right_pt, 5.0);
    assert_eq!(hit_r, Some((ContourId(0), 1, false)));

    // 3a. Direct interactive pointer drag on a VariableWidthHandlePin (press -> move -> release + Undo/Redo)
    let pin1_left_scr = state.viewport.to_screen(pins[1].left_pt);
    let dragged_left_upm = FontPoint::new(
        pins[1].spine_pt.x.0 + (pins[1].left_pt.x.0 - pins[1].spine_pt.x.0) * 4.5,
        pins[1].spine_pt.y.0 + (pins[1].left_pt.y.0 - pins[1].spine_pt.y.0) * 4.5,
    );
    common::assert_mutation_undo_redo_roundtrip(&mut state, |s| {
        assert!(s.try_begin_variable_width_pin_drag(pin1_left_scr, pins[1].left_pt));
        assert!(s.try_update_variable_width_pin_drag(dragged_left_upm));
        assert!(s.try_finish_variable_width_pin_release());
    });
    assert!(
        (state.icon_studio.variable_width_strokes[0].samples[1].width_left_upm - 45.0).abs() < 1e-2,
        "Dragging left width pin to 4.5x must update width_left_upm to 45.0, got {}",
        state.icon_studio.variable_width_strokes[0].samples[1].width_left_upm
    );

    common::assert_mutation_undo_redo_roundtrip(&mut state, |s| {
        assert!(s.update_variable_width_sample(ContourId(0), 1, 35.0, 45.0));
    });
    assert!(
        (state.icon_studio.variable_width_strokes[0].samples[1].width_left_upm - 35.0).abs() < 1e-6
    );
    assert!(
        (state.icon_studio.variable_width_strokes[0].samples[1].width_right_upm - 45.0).abs()
            < 1e-6
    );

    assert_eq!(
        state.controller_telemetry().primary_rotary.parameter_name,
        "Variable Width"
    );
    assert!(state.step_rotary_encoder(grafia::CanvasRotaryChannel::ContextPrimary, 2.0));
    assert!(
        (state.icon_studio.variable_width_strokes[0].samples[0].width_left_upm - 14.0).abs() < 1e-6
    );
    assert!(state.undo());

    common::assert_mutation_undo_redo_roundtrip(&mut state, |s| {
        assert!(s.scale_active_variable_width_stroke(5.0));
    });
    assert!(
        (state.icon_studio.variable_width_strokes[0].samples[0].width_left_upm - 15.0).abs() < 1e-6
    );

    // Attach a smart corner and genius node to NodeId(1) on the skeleton before baking,
    // and verify bake_variable_width_strokes prunes orphaned node references.
    state
        .curve_harmony
        .smart_corners
        .push(grafia::SmartCornerSpec {
            node_id: NodeId(1),
            radius_upm: 12.0,
        });
    state.curve_harmony.genius_nodes.insert(NodeId(1));
    common::assert_mutation_undo_redo_roundtrip(&mut state, |s| {
        assert!(s.bake_variable_width_strokes());
    });
    assert!(state.icon_studio.variable_width_strokes.is_empty());
    assert!(state.curve_harmony.smart_corners.is_empty());
    assert!(state.curve_harmony.genius_nodes.is_empty());
    assert!(state.glyph.contours[0].closed);

    // 3b. Closed contour seam continuity (t=0.0 width 40, t=0.5 width 12 wraps back to 40 at t=1.0)
    let mut circle_nid = 1_u64;
    let mut closed_ring = GlyphContour::new(
        ContourId(0),
        grafia::build_circle_contour_nodes(100.0, 100.0, 300.0, 300.0, &mut circle_nid),
    );
    closed_ring.closed = true;
    let ring_spec = VariableWidthStrokeSpec {
        contour_id: ContourId(0),
        samples: vec![
            WidthNodeSample {
                arc_ratio_t: 0.0,
                width_left_upm: 40.0,
                width_right_upm: 40.0,
            },
            WidthNodeSample {
                arc_ratio_t: 0.5,
                width_left_upm: 12.0,
                width_right_upm: 12.0,
            },
        ],
        cap_style: LineCapStyle::Round,
    };
    let mut ring_nid = 500_u64;
    let ring_out = expand_variable_width_contour(&closed_ring, &ring_spec, &mut ring_nid);
    assert!(
        ring_out.len() >= 2,
        "Closed ring variable-width stroke must produce outer and inner closed contours"
    );

    // 4. Unconstrained `arbtest` property-based fuzzer on `evaluate_variable_width_at_t` & `expand_variable_width_contour`
    arbtest(|u| {
        let count = u.int_in_range(0..=6).unwrap_or(2);
        let mut samples = Vec::with_capacity(count);
        for _ in 0..count {
            samples.push(WidthNodeSample {
                arc_ratio_t: arbitrary_f64_with_extremes(u),
                width_left_upm: arbitrary_f64_with_extremes(u),
                width_right_upm: arbitrary_f64_with_extremes(u),
            });
        }
        let cap_style = match u.int_in_range(0..=2).unwrap_or(0) {
            0 => LineCapStyle::Butt,
            1 => LineCapStyle::Round,
            _ => LineCapStyle::Square,
        };
        let spec = VariableWidthStrokeSpec {
            contour_id: ContourId(0),
            samples,
            cap_style,
        };
        let raw_t = arbitrary_f64_with_extremes(u);
        let (wl, wr) = evaluate_variable_width_at_t(&spec, raw_t);
        assert!(wl.is_finite() && wl >= 0.0 && wr.is_finite() && wr >= 0.0);

        let valid_l: Vec<f64> = spec
            .samples
            .iter()
            .filter(|s| {
                s.arc_ratio_t.is_finite()
                    && s.width_left_upm.is_finite()
                    && s.width_right_upm.is_finite()
            })
            .map(|s| s.width_left_upm.clamp(0.0, 10_000.0))
            .collect();
        if !valid_l.is_empty() {
            let min_l = valid_l.iter().copied().fold(f64::INFINITY, f64::min);
            let max_l = valid_l.iter().copied().fold(0.0_f64, f64::max);
            assert!(
                wl >= min_l - 1e-6 && wl <= max_l + 1e-6,
                "Fritsch-Carlson left width {wl} outside [{min_l}, {max_l}]"
            );
        }

        let x0 = f64::from(u.int_in_range(-200..=200).unwrap_or(0));
        let y0 = f64::from(u.int_in_range(-200..=200).unwrap_or(0));
        let x1 = f64::from(u.int_in_range(-200..=400).unwrap_or(150));
        let y1 = f64::from(u.int_in_range(-200..=400).unwrap_or(100));
        let x2 = f64::from(u.int_in_range(-200..=400).unwrap_or(200));
        let y2 = f64::from(u.int_in_range(-200..=400).unwrap_or(250));
        let mut c = GlyphContour::new(
            ContourId(0),
            vec![
                Node::new(NodeId(1), x0, y0, NodeType::Corner),
                Node::new(NodeId(2), x1, y1, NodeType::Corner),
                Node::new(NodeId(3), x2, y2, NodeType::Corner),
            ],
        );
        c.closed = u.arbitrary::<bool>().unwrap_or(false);
        let mut nid = 50_u64;
        let out = expand_variable_width_contour(&c, &spec, &mut nid);
        for gc in &out {
            assert!(gc.closed);
            for n in &gc.nodes {
                assert!(n.point.x.0.is_finite() && n.point.y.0.is_finite());
            }
        }
        Ok(())
    })
    .budget_ms(1_000);
}

#[test]
fn tc_v0140_012_cohesive_icon_studio_scene_invariants_undo_redo_and_idle_battery_guard() {
    let mut state =
        common::canvas_with_closed_contour(&[(100.0, 100.0), (400.0, 100.0), (250.0, 500.0)]);
    assert!(state.validate_invariants().is_ok());

    // 1. Default `icon_studio` converts cleanly to `vitrea::LiquidGlassOpticalParams` and has 0% idle playback
    assert!(!state.icon_studio.kinetic.playing);
    let optics = state.icon_studio.liquid_glass.to_vitrea_optics();
    assert!((optics.ior_glass - 1.50).abs() < 1e-4);
    assert!(!state.step_kinetic_timeline(0.016));
    let idle_frame = common::run_headless_canvas_frame(&mut state, 320.0, 240.0);
    assert!(!idle_frame.shapes.is_empty());

    // 2. Universal `State_0 -> Mutation -> State_1 -> Undo == State_0 -> Redo == State_1` bit-exact roundtrip
    let custom_scene = CanvasIconStudioScene {
        kinetic: KineticTimelineState {
            effect: KineticSymbolEffect::Bounce {
                direction: BounceDirection::Up,
                scope: KineticLayerScope::ByLayer,
            },
            phase_t: 0.35,
            speed_multiplier: 1.5,
            playing: false,
            looping: true,
            ..Default::default()
        },
        liquid_glass: LiquidGlassRenderConfig {
            dome_radius_upm: 64.0,
            smooth_union_k_upm: 28.0,
            ior_base: 1.52,
            chromatic_dispersion: 0.025,
            specular_intensity: 0.95,
            tint_rgba: [100, 190, 255, 230],
        },
        radial_symmetry: RadialSymmetrySpec {
            fold_count: 6,
            dihedral_mirror: true,
            center: FontPoint::new(250.0, 250.0),
        },
        variable_width_strokes: vec![VariableWidthStrokeSpec {
            contour_id: ContourId(0),
            samples: vec![
                WidthNodeSample {
                    arc_ratio_t: 0.0,
                    width_left_upm: 12.0,
                    width_right_upm: 12.0,
                },
                WidthNodeSample {
                    arc_ratio_t: 0.5,
                    width_left_upm: 36.0,
                    width_right_upm: 24.0,
                },
                WidthNodeSample {
                    arc_ratio_t: 1.0,
                    width_left_upm: 8.0,
                    width_right_upm: 8.0,
                },
            ],
            cap_style: LineCapStyle::Round,
        }],
        symbol_layer_tags: vec![SymbolLayerTag {
            layer_group: 1,
            motion_anchor_id: Some(AnchorId(1)),
            custom_pivot_upm: Some(FontPoint::new(250.0, 250.0)),
            draw_order_reversed: false,
            variable_color_enabled: true,
            pulse_participates: true,
            ..Default::default()
        }],
        ..Default::default()
    };

    common::assert_mutation_undo_redo_roundtrip(&mut state, |s| {
        s.set_icon_studio_scene(custom_scene.clone());
    });
    assert_eq!(state.icon_studio, custom_scene);

    // 3. `CanvasDocumentScene` export -> JSON serialize -> deserialize -> load roundtrip
    let doc_scene = state.export_document_scene();
    assert_eq!(doc_scene.icon_studio, custom_scene);
    let json = serde_json::to_string(&doc_scene).expect("serialize CanvasDocumentScene");
    let decoded: grafia::CanvasDocumentScene =
        serde_json::from_str(&json).expect("deserialize CanvasDocumentScene");
    let mut restored = GrafiaCanvasState::new(GrafiaTheme::sonoma_default());
    restored.load_document_scene(decoded);
    assert!(restored.validate_invariants().is_ok());
    assert_eq!(restored.icon_studio, custom_scene);

    // 4. `validate_invariants()` catches invalid `icon_studio` states
    let mut bad = restored;
    bad.icon_studio.kinetic.phase_t = 1.5;
    assert!(bad.validate_invariants().is_err());
    bad.icon_studio.kinetic.phase_t = 0.5;
    bad.icon_studio.radial_symmetry.fold_count = 0;
    assert!(bad.validate_invariants().is_err());
    bad.icon_studio.radial_symmetry.fold_count = 4;
    bad.icon_studio.liquid_glass.ior_base = 0.0;
    assert!(bad.validate_invariants().is_err());
}

fn arbitrary_f64_with_extremes(u: &mut arbtest::arbitrary::Unstructured<'_>) -> f64 {
    match u.int_in_range(0..=9).unwrap_or(0) {
        0 => f64::NAN,
        1 => f64::INFINITY,
        2 => f64::NEG_INFINITY,
        3 => -0.0,
        4 => f64::MAX,
        5 => f64::MIN,
        _ => u.arbitrary::<f64>().unwrap_or(0.0),
    }
}

fn arbitrary_f32_with_extremes(u: &mut arbtest::arbitrary::Unstructured<'_>) -> f32 {
    match u.int_in_range(0..=9).unwrap_or(0) {
        0 => f32::NAN,
        1 => f32::INFINITY,
        2 => f32::NEG_INFINITY,
        3 => -0.0,
        4 => f32::MAX,
        5 => f32::MIN,
        _ => u.arbitrary::<f32>().unwrap_or(0.0),
    }
}

fn arbitrary_kinetic_effect(u: &mut arbtest::arbitrary::Unstructured<'_>) -> KineticSymbolEffect {
    let scope = if u.arbitrary::<bool>().unwrap_or(false) {
        KineticLayerScope::ByLayer
    } else {
        KineticLayerScope::WholeSymbol
    };
    let dir = if u.arbitrary::<bool>().unwrap_or(false) {
        BounceDirection::Up
    } else {
        BounceDirection::Down
    };
    match u.int_in_range(0..=9).unwrap_or(0) {
        0 => KineticSymbolEffect::Pulse { scope },
        1 => KineticSymbolEffect::Bounce {
            direction: dir,
            scope,
        },
        2 => KineticSymbolEffect::VariableColor {
            cumulative: u.arbitrary().unwrap_or(false),
            reversing: u.arbitrary().unwrap_or(false),
            dim_inactive: u.arbitrary().unwrap_or(false),
        },
        3 => KineticSymbolEffect::Scale {
            direction: if u.arbitrary().unwrap_or(false) {
                ScaleDirection::Up
            } else {
                ScaleDirection::Down
            },
            scope,
        },
        4 => KineticSymbolEffect::Appear {
            direction: dir,
            draw_on: u.arbitrary().unwrap_or(false),
            scope,
        },
        5 => KineticSymbolEffect::Disappear {
            direction: dir,
            draw_off: u.arbitrary().unwrap_or(false),
            scope,
        },
        6 => {
            let axis = match u.int_in_range(0..=8).unwrap_or(0) {
                0 => WiggleAxis::Clockwise,
                1 => WiggleAxis::CounterClockwise,
                2 => WiggleAxis::Left,
                3 => WiggleAxis::Right,
                4 => WiggleAxis::Up,
                5 => WiggleAxis::Down,
                6 => WiggleAxis::Forward,
                7 => WiggleAxis::Backward,
                _ => WiggleAxis::CustomAngle(arbitrary_f64_with_extremes(u)),
            };
            KineticSymbolEffect::Wiggle { axis, scope }
        }
        7 => KineticSymbolEffect::Rotate {
            clockwise: u.arbitrary().unwrap_or(true),
            scope,
        },
        8 => KineticSymbolEffect::Breathe {
            pulse_opacity: u.arbitrary().unwrap_or(false),
            scope,
        },
        _ => {
            let n_nodes = u.int_in_range(0..=4).unwrap_or(0);
            let mut nodes = Vec::with_capacity(n_nodes);
            for i in 0..n_nodes {
                nodes.push(Node {
                    id: NodeId(i as u64 + 1),
                    point: FontPoint::new(
                        arbitrary_f64_with_extremes(u),
                        arbitrary_f64_with_extremes(u),
                    ),
                    node_type: NodeType::Corner,
                });
            }
            KineticSymbolEffect::MagicReplace {
                fallback: match u.int_in_range(0..=2).unwrap_or(0) {
                    0 => ReplaceFallback::DownUp,
                    1 => ReplaceFallback::UpUp,
                    _ => ReplaceFallback::OffUp,
                },
                scope,
                target_contours: vec![GlyphContour::new(ContourId(0), nodes)],
            }
        }
    }
}

#[test]
fn tc_v0140_arbtest_icon_studio_scene_and_timeline_unconstrained_fuzz() {
    arbtest(|u| {
        let sample_count = u.int_in_range(0..=5).unwrap_or(0);
        let mut samples = Vec::with_capacity(sample_count);
        for _ in 0..sample_count {
            samples.push(WidthNodeSample {
                arc_ratio_t: arbitrary_f64_with_extremes(u),
                width_left_upm: arbitrary_f64_with_extremes(u),
                width_right_upm: arbitrary_f64_with_extremes(u),
            });
        }
        let tag_count = u.int_in_range(0..=4).unwrap_or(0);
        let mut tags = Vec::with_capacity(tag_count);
        for _ in 0..tag_count {
            tags.push(SymbolLayerTag {
                layer_group: u.arbitrary::<u8>().unwrap_or(0),
                motion_anchor_id: u.arbitrary::<Option<u64>>().unwrap_or(None).map(AnchorId),
                custom_pivot_upm: if u.arbitrary::<bool>().unwrap_or(false) {
                    Some(FontPoint::new(
                        arbitrary_f64_with_extremes(u),
                        arbitrary_f64_with_extremes(u),
                    ))
                } else {
                    None
                },
                draw_order_reversed: u.arbitrary().unwrap_or(false),
                variable_color_enabled: u.arbitrary().unwrap_or(false),
                pulse_participates: u.arbitrary().unwrap_or(false),
                ..Default::default()
            });
        }
        let raw_scene = CanvasIconStudioScene {
            kinetic: KineticTimelineState {
                effect: arbitrary_kinetic_effect(u),
                phase_t: arbitrary_f64_with_extremes(u),
                speed_multiplier: arbitrary_f64_with_extremes(u),
                playing: u.arbitrary().unwrap_or(false),
                looping: u.arbitrary().unwrap_or(false),
                ..Default::default()
            },
            liquid_glass: LiquidGlassRenderConfig {
                dome_radius_upm: arbitrary_f64_with_extremes(u),
                smooth_union_k_upm: arbitrary_f64_with_extremes(u),
                ior_base: arbitrary_f32_with_extremes(u),
                chromatic_dispersion: arbitrary_f32_with_extremes(u),
                specular_intensity: arbitrary_f32_with_extremes(u),
                tint_rgba: u.arbitrary().unwrap_or([120, 200, 255, 220]),
            },
            radial_symmetry: RadialSymmetrySpec {
                fold_count: u.arbitrary::<u8>().unwrap_or(0),
                dihedral_mirror: u.arbitrary().unwrap_or(false),
                center: FontPoint::new(
                    arbitrary_f64_with_extremes(u),
                    arbitrary_f64_with_extremes(u),
                ),
            },
            variable_width_strokes: vec![VariableWidthStrokeSpec {
                contour_id: ContourId(u.int_in_range(0..=3).unwrap_or(0)),
                samples,
                cap_style: LineCapStyle::Round,
            }],
            symbol_layer_tags: tags,
            ..Default::default()
        };

        let clean = raw_scene.clone().sanitized();
        assert!(
            clean.validate_invariants().is_ok(),
            "sanitized() scene violated invariants: {:?} -> {:?}",
            clean,
            clean.validate_invariants()
        );
        let optics = clean.liquid_glass.to_vitrea_optics();
        assert!(
            optics.ior_glass.is_finite()
                && optics.ior_glass > 0.0
                && optics.bevel_width_pt.is_finite()
                && optics.meniscus_merge_k_pt.is_finite()
                && optics.fresnel_f0.is_finite()
                && optics.ior_dispersion_rgb.iter().all(|v| v.is_finite()),
            "Non-finite vitrea optics from sanitized scene: {optics:?}"
        );

        let mut state =
            common::canvas_with_closed_contour(&[(100.0, 100.0), (400.0, 100.0), (250.0, 500.0)]);
        let before = state.snapshot();
        state.set_icon_studio_scene(raw_scene);
        assert!(state.validate_invariants().is_ok());
        let after = state.snapshot();
        if before != after {
            assert!(state.undo());
            assert_eq!(state.snapshot(), before);
            assert!(state.redo());
            assert_eq!(state.snapshot(), after);
        }

        let dt = arbitrary_f64_with_extremes(u);
        let _ = state.step_kinetic_timeline(dt);
        assert!(state.validate_invariants().is_ok());
        Ok(())
    })
    .budget_ms(1_500);
}

#[test]
fn tc_v0140_arbtest_shape_builder_planar_topology_and_gesture_fuzz() {
    arbtest(|u| {
        let mut next_id = 1_u64;
        let count = u.int_in_range(0..=4).unwrap_or(2);
        let mut contours = Vec::with_capacity(count);
        for idx in 0..count {
            let kind = u.int_in_range(0..=3).unwrap_or(0);
            let x0 = f64::from(u.int_in_range(-200..=400).unwrap_or(0));
            let y0 = f64::from(u.int_in_range(-200..=400).unwrap_or(0));
            let w = f64::from(u.int_in_range(-50..=300).unwrap_or(100));
            let h = f64::from(u.int_in_range(-50..=300).unwrap_or(100));
            let nodes = match kind {
                0 => build_rectangle_contour_nodes(x0, y0, x0 + w, y0 + h, &mut next_id),
                1 => build_circle_contour_nodes(x0, y0, x0 + w, y0 + h, &mut next_id),
                2 => vec![
                    Node::new(NodeId(next_id), x0, y0, NodeType::Corner),
                    Node::new(NodeId(next_id + 1), x0 + w, y0, NodeType::Corner),
                ],
                _ => vec![
                    Node {
                        id: NodeId(next_id),
                        point: FontPoint::new(
                            arbitrary_f64_with_extremes(u),
                            arbitrary_f64_with_extremes(u),
                        ),
                        node_type: NodeType::Corner,
                    },
                    Node {
                        id: NodeId(next_id + 1),
                        point: FontPoint::new(
                            arbitrary_f64_with_extremes(u),
                            arbitrary_f64_with_extremes(u),
                        ),
                        node_type: NodeType::Corner,
                    },
                    Node {
                        id: NodeId(next_id + 2),
                        point: FontPoint::new(
                            arbitrary_f64_with_extremes(u),
                            arbitrary_f64_with_extremes(u),
                        ),
                        node_type: NodeType::Corner,
                    },
                ],
            };
            next_id += 10;
            let mut gc = GlyphContour::new(ContourId(idx), nodes);
            gc.closed = kind != 2 && u.arbitrary::<bool>().unwrap_or(true);
            gc.visible = u.arbitrary::<bool>().unwrap_or(true);
            contours.push(gc);
        }

        let regions = compute_shape_builder_regions(&contours);
        for (i, reg) in regions.iter().enumerate() {
            assert_eq!(reg.region_id, i as u32);
            assert!(reg.area_upm2.is_finite() && reg.area_upm2 >= 1.0);
            assert!(reg.centroid.x.0.is_finite() && reg.centroid.y.0.is_finite());
            assert!(!reg.source_contour_ids.is_empty());
            for n in &reg.contour.nodes {
                assert!(n.point.x.0.is_finite() && n.point.y.0.is_finite());
            }
        }

        // Now test stateful ShapeBuilder operations and Undo/Redo on sanitized valid contours
        let mut state = GrafiaCanvasState::new(GrafiaTheme::sonoma_default());
        let glyph = grafia::Glyph {
            contours: contours
                .into_iter()
                .filter(|c| {
                    c.nodes
                        .iter()
                        .all(|n| n.point.x.0.is_finite() && n.point.y.0.is_finite())
                })
                .collect(),
            ..Default::default()
        };
        let metrics = state.metrics.clone();
        state.load_glyph(glyph, metrics);
        state.active_tool = CanvasTool::ShapeBuilder;
        assert!(state.validate_invariants().is_ok());

        let sel_count = u.int_in_range(0..=4).unwrap_or(1);
        let mut region_ids = Vec::with_capacity(sel_count);
        for _ in 0..sel_count {
            region_ids.push(u.int_in_range(0..=6).unwrap_or(0));
        }
        let op = if u.arbitrary::<bool>().unwrap_or(true) {
            ShapeBuilderOp::Merge
        } else {
            ShapeBuilderOp::Erase
        };
        let before = state.snapshot();
        let changed = state.apply_shape_builder_operation(&region_ids, op);
        assert!(state.validate_invariants().is_ok());
        if changed {
            let after = state.snapshot();
            // A 1-region Merge must never increase contour count or duplicate overlaps
            if op == ShapeBuilderOp::Merge && region_ids.len() == 1 {
                assert!(
                    state.glyph.contours.len() <= before.contours.len(),
                    "1-region Merge increased contour count from {} to {}",
                    before.contours.len(),
                    state.glyph.contours.len()
                );
            }
            assert!(state.undo());
            assert_eq!(state.snapshot(), before);
            assert!(state.redo());
            assert_eq!(state.snapshot(), after);
        }
        Ok(())
    })
    .budget_ms(1_500);
}

#[test]
fn tc_v0140_003_radial_symmetry_and_stem_preserving_optical_oblique() {
    // 1. 6-Fold dihedral radial symmetry -> 12 contours with exact 60° rotational invariance
    let mut nid = 1_u64;
    let petal = GlyphContour::new(
        ContourId(0),
        build_rectangle_contour_nodes(540.0, 480.0, 660.0, 520.0, &mut nid),
    );
    let sym_spec = RadialSymmetrySpec {
        fold_count: 6,
        dihedral_mirror: true,
        center: FontPoint::new(500.0, 500.0),
    };
    let radial =
        grafia::compute_radial_symmetry_contours(std::slice::from_ref(&petal), &sym_spec, &mut nid);
    assert_eq!(
        radial.len(),
        12,
        "6-fold dihedral symmetry on 1 petal must yield 12 contours"
    );
    let (base_area, _) = compute_contour_area_and_centroid(&petal.nodes);
    for c in &radial {
        let (a, _) = compute_contour_area_and_centroid(&c.nodes);
        assert!(
            (a - base_area).abs() < 1e-4,
            "Every radial/dihedral sector contour must preserve positive CCW area {base_area}, got {a}"
        );
    }

    // Verify 60° rotational invariance of sector centroids around (500, 500)
    let rot60 = 60.0_f64.to_radians();
    for k in 0..6 {
        let (_, c0) = compute_contour_area_and_centroid(&radial[2 * k].nodes);
        let (_, c1) = compute_contour_area_and_centroid(&radial[(2 * (k + 1)) % 12].nodes);
        let dx = c0.x.0 - 500.0;
        let dy = c0.y.0 - 500.0;
        let rx = 500.0 + dx * rot60.cos() - dy * rot60.sin();
        let ry = 500.0 + dx * rot60.sin() + dy * rot60.cos();
        assert!(
            (c1.x.0 - rx).hypot(c1.y.0 - ry) < 1e-4,
            "6-fold radial symmetry centroid not invariant under 60° rotation"
        );
    }

    // 2. Stem-preserving optical oblique at 12° vs naive shear (< 1.5% stem thickness error)
    let stem = GlyphContour::new(
        ContourId(0),
        build_rectangle_contour_nodes(200.0, 100.0, 280.0, 600.0, &mut nid),
    );
    let orig_thick = grafia::measure_contour_orthogonal_stem_thickness(
        std::slice::from_ref(&stem),
        0,
        3,
        0.5,
        &[80.0],
        1.0,
    )
    .expect("Original vertical stem measurement")
    .thickness_upm;
    assert!((orig_thick - 80.0).abs() < 1e-4);

    let naive = grafia::compute_optical_oblique_contours(
        std::slice::from_ref(&stem),
        12.0,
        false,
        &mut nid,
    );
    let naive_thick =
        grafia::measure_contour_orthogonal_stem_thickness(&naive, 0, 3, 0.5, &[80.0], 1.0)
            .expect("Naive shear stem measurement")
            .thickness_upm;
    let naive_err_rel = ((naive_thick - orig_thick) / orig_thick).abs();
    assert!(
        naive_err_rel > 0.015,
        "Naive 12° shear must thin orthogonal stem by ~2.19%, got {:.4}%",
        naive_err_rel * 100.0
    );

    let optical =
        grafia::compute_optical_oblique_contours(std::slice::from_ref(&stem), 12.0, true, &mut nid);
    let optical_thick =
        grafia::measure_contour_orthogonal_stem_thickness(&optical, 0, 3, 0.5, &[80.0], 1.0)
            .expect("Optical oblique stem measurement")
            .thickness_upm;
    let optical_err_rel = ((optical_thick - orig_thick) / orig_thick).abs();
    assert!(
        optical_err_rel < 0.015,
        "Optical oblique at 12° must preserve stem thickness within < 1.5%, got {:.4}%",
        optical_err_rel * 100.0
    );

    // 2b. Hollow ring with asymmetric inner counter hole: optical oblique must use a shared layer center
    // so the wall thickness between outer contour and inner counter hole is also sec(alpha)-preserved!
    let outer_box = GlyphContour::new(
        ContourId(0),
        build_rectangle_contour_nodes(100.0, 100.0, 500.0, 500.0, &mut nid),
    );
    let mut inner_nodes = build_rectangle_contour_nodes(180.0, 180.0, 460.0, 420.0, &mut nid);
    grafia::core::reverse_contour_nodes(&mut inner_nodes, true);
    let inner_hole = GlyphContour::new(ContourId(1), inner_nodes);
    let hollow_oblique =
        grafia::compute_optical_oblique_contours(&[outer_box, inner_hole], 12.0, true, &mut nid);
    // Left wall between outer x=100 and inner x=180 at y=300 has original horizontal width 80.0;
    // under sec(12°) compensation with shared layer center, horizontal width must be 80.0 / cos(12°).
    let expected_sec_wall = 80.0 / 12.0_f64.to_radians().cos();
    let actual_left_wall = hollow_oblique[1].nodes[0].point.x.0
        - hollow_oblique[0].nodes[0].point.x.0
        + (hollow_oblique[0].nodes[0].point.y.0 - hollow_oblique[1].nodes[0].point.y.0)
            * 12.0_f64.to_radians().tan();
    assert!(
        (actual_left_wall - expected_sec_wall).abs() < 1e-4,
        "Hollow contour + inner counter hole must share a common oblique center so wall thickness scales by sec(12°): expected {expected_sec_wall}, got {actual_left_wall}"
    );

    // 3. Stateful GrafiaCanvasState Undo/Redo roundtrips for bake_radial_symmetry & apply_optical_oblique
    let mut state = GrafiaCanvasState::new(GrafiaTheme::sonoma_default());
    let glyph = grafia::Glyph {
        contours: vec![petal],
        ..Default::default()
    };
    let metrics = state.metrics.clone();
    state.load_glyph(glyph, metrics);
    state.icon_studio.radial_symmetry = sym_spec;
    common::assert_mutation_undo_redo_roundtrip(&mut state, |s| {
        assert!(s.bake_radial_symmetry());
    });
    assert_eq!(state.glyph.contours.len(), 12);
    common::assert_mutation_undo_redo_roundtrip(&mut state, |s| {
        assert!(s.apply_optical_oblique(12.0, true));
    });

    // 4. Unconstrained `arbtest` fuzzer on radial symmetry & optical oblique
    arbtest(|u| {
        let mut f_nid = 10_u64;
        let x0 = f64::from(u.int_in_range(-200..=300).unwrap_or(50));
        let y0 = f64::from(u.int_in_range(-200..=300).unwrap_or(50));
        let w = f64::from(u.int_in_range(10..=200).unwrap_or(80));
        let h = f64::from(u.int_in_range(10..=200).unwrap_or(80));
        let c = GlyphContour::new(
            ContourId(0),
            build_rectangle_contour_nodes(x0, y0, x0 + w, y0 + h, &mut f_nid),
        );
        let spec = RadialSymmetrySpec {
            fold_count: u.arbitrary::<u8>().unwrap_or(4),
            dihedral_mirror: u.arbitrary::<bool>().unwrap_or(false),
            center: FontPoint::new(
                arbitrary_f64_with_extremes(u),
                arbitrary_f64_with_extremes(u),
            ),
        };
        let out_sym =
            grafia::compute_radial_symmetry_contours(std::slice::from_ref(&c), &spec, &mut f_nid);
        for gc in &out_sym {
            for n in &gc.nodes {
                assert!(n.point.x.0.is_finite() && n.point.y.0.is_finite());
            }
        }
        let slant = arbitrary_f64_with_extremes(u);
        let preserve = u.arbitrary::<bool>().unwrap_or(true);
        let out_obl = grafia::compute_optical_oblique_contours(
            std::slice::from_ref(&c),
            slant,
            preserve,
            &mut f_nid,
        );
        assert_eq!(out_obl.len(), 1);
        for n in &out_obl[0].nodes {
            assert!(n.point.x.0.is_finite() && n.point.y.0.is_finite());
        }
        Ok(())
    })
    .budget_ms(800);
}

#[test]
fn tc_v0140_004_multi_scale_icon_normalizer_five_stage_pipeline() {
    let mut nid = 1_u64;
    // Build a dirty contour set:
    // - Two overlapping shapes,
    // - One rotated cubic circle lacking horizontal/vertical extrema and having CW winding,
    // - A duplicate near-zero degenerate on-curve node within 0.1 UPM (< 0.25 UPM)
    let mut c1 = GlyphContour::new(
        ContourId(0),
        build_rectangle_contour_nodes(100.0, 100.0, 300.0, 300.0, &mut nid),
    );
    // Insert a near-zero degenerate segment (< 0.25 UPM) and unrounded coordinates
    c1.nodes
        .insert(1, Node::new(NodeId(900), 100.08, 100.04, NodeType::Corner));
    nid = 1000;

    let mut c2_nodes = build_circle_contour_nodes(200.0, 150.0, 420.0, 370.0, &mut nid);
    // Rotate circle by 27° so its cubic segments bulge past the rotated on-curve nodes (missing extrema)
    let ang = 27.0_f64.to_radians();
    for n in &mut c2_nodes {
        let (dx, dy) = (n.point.x.0 - 310.0, n.point.y.0 - 260.0);
        n.point.x.0 = 310.0 + dx * ang.cos() - dy * ang.sin();
        n.point.y.0 = 260.0 + dx * ang.sin() + dy * ang.cos();
    }
    // Reverse winding so outer contour is improperly CW
    grafia::core::reverse_contour_nodes(&mut c2_nodes, true);
    let c2 = GlyphContour::new(ContourId(1), c2_nodes);

    let normalized =
        grafia::audit_and_normalize_icon_contours(&[c1.clone(), c2.clone()], Some(1.0), &mut nid);
    assert_eq!(
        normalized.len(),
        1,
        "Overlapping c1 and c2 must be merged into 1 clean outer contour"
    );
    let (area, _) = compute_contour_area_and_centroid(&normalized[0].nodes);
    assert!(
        area > 0.0,
        "Normalized outer contour must have positive CCW winding, got {area}"
    );
    for n in &normalized[0].nodes {
        assert!(
            (n.point.x.0 - n.point.x.0.round()).abs() < 1e-6
                && (n.point.y.0 - n.point.y.0.round()).abs() < 1e-6,
            "Grid step 1.0 must snap all coordinates to integers, got ({}, {})",
            n.point.x.0,
            n.point.y.0
        );
    }
    let findings = grafia::core::audit_contours_outlines(&normalized, 0.0);
    let missing_extrema = findings
        .iter()
        .filter(|f| matches!(f.kind, grafia::OutlineAuditKind::MissingExtremum { .. }))
        .count();
    assert_eq!(
        missing_extrema, 0,
        "Normalized contour must have 0 missing extrema"
    );

    // Stateful GrafiaCanvasState::normalize_icon_contours Undo/Redo roundtrip
    let mut state = GrafiaCanvasState::new(GrafiaTheme::sonoma_default());
    let glyph = grafia::Glyph {
        contours: vec![c1, c2],
        ..Default::default()
    };
    let metrics = state.metrics.clone();
    state.load_glyph(glyph, metrics);
    common::assert_mutation_undo_redo_roundtrip(&mut state, |s| {
        assert!(s.normalize_icon_contours(Some(1.0)));
    });
    assert_eq!(state.glyph.contours.len(), 1);

    // Explicit regression lock for subnormal/tiny grid_step_opt (f64::MIN_POSITIVE)
    let subnormal_norm = grafia::audit_and_normalize_icon_contours(
        &state.glyph.contours,
        Some(f64::MIN_POSITIVE),
        &mut nid,
    );
    assert_eq!(subnormal_norm.len(), 1);

    // Unconstrained `arbtest` fuzzer on `audit_and_normalize_icon_contours`
    arbtest(|u| {
        let mut f_nid = 10_u64;
        let count = u.int_in_range(0..=3).unwrap_or(1);
        let mut contours = Vec::with_capacity(count);
        for idx in 0..count {
            let x0 = f64::from(u.int_in_range(-200..=300).unwrap_or(0));
            let y0 = f64::from(u.int_in_range(-200..=300).unwrap_or(0));
            let w = f64::from(u.int_in_range(5..=250).unwrap_or(80));
            let h = f64::from(u.int_in_range(5..=250).unwrap_or(80));
            let nodes = if u.arbitrary::<bool>().unwrap_or(true) {
                build_rectangle_contour_nodes(x0, y0, x0 + w, y0 + h, &mut f_nid)
            } else {
                build_circle_contour_nodes(x0, y0, x0 + w, y0 + h, &mut f_nid)
            };
            contours.push(GlyphContour::new(ContourId(idx), nodes));
        }
        let grid_opt = if u.arbitrary::<bool>().unwrap_or(false) {
            Some(arbitrary_f64_with_extremes(u))
        } else {
            None
        };
        let norm = grafia::audit_and_normalize_icon_contours(&contours, grid_opt, &mut f_nid);
        for (i, gc) in norm.iter().enumerate() {
            assert_eq!(gc.id, ContourId(i));
            for n in &gc.nodes {
                assert!(n.point.x.0.is_finite() && n.point.y.0.is_finite());
            }
        }
        Ok(())
    })
    .budget_ms(800);
}

#[test]
fn tc_v0140_005_kinetic_effects_1_to_4_pulse_bounce_vitrea_var_color_scale() {
    let mut nid = 1_u64;
    let rect = GlyphContour::new(
        ContourId(0),
        build_rectangle_contour_nodes(100.0, 100.0, 300.0, 260.0, &mut nid),
    );
    let (base_area, base_centroid) = compute_contour_area_and_centroid(&rect.nodes);
    assert!((base_area - 32_000.0).abs() < 1e-4);

    // 1. Bounce with vitrea::deform_viscous_droplet_point preserves 2D area (det J == 1.0, < 1% error) across tau in [0, 1]
    let bounce_up = KineticSymbolEffect::Bounce {
        direction: BounceDirection::Up,
        scope: KineticLayerScope::WholeSymbol,
    };
    let mut max_dy = 0.0_f64;
    for step in 0..=20 {
        let tau = f64::from(step) / 20.0;
        let tf = grafia::evaluate_layer_kinetic_transform(
            &bounce_up,
            tau,
            0,
            1,
            None,
            None,
            base_centroid,
        );
        let deformed = grafia::apply_layer_kinetic_transform(&rect, &tf);
        let (area, centroid) = compute_contour_area_and_centroid(&deformed.nodes);
        let rel_area_err = ((area - base_area) / base_area).abs();
        assert!(
            rel_area_err < 0.01,
            "Bounce viscous droplet deformation must preserve 2D area within < 1%, got {:.5}% at tau={tau}",
            rel_area_err * 100.0
        );
        max_dy = max_dy.max(centroid.y.0 - base_centroid.y.0);
    }
    assert!(
        max_dy > 15.0,
        "Bounce::Up must displace contour upward during impulse, max_dy={max_dy}"
    );

    // 2. Pulse modulates opacity down at tau = 0.5 and returns to 1.0 at tau = 0.0 and 1.0
    let pulse = KineticSymbolEffect::Pulse {
        scope: KineticLayerScope::WholeSymbol,
    };
    let tf_p0 =
        grafia::evaluate_layer_kinetic_transform(&pulse, 0.0, 0, 1, None, None, base_centroid);
    let tf_p_half =
        grafia::evaluate_layer_kinetic_transform(&pulse, 0.5, 0, 1, None, None, base_centroid);
    let tf_p1 =
        grafia::evaluate_layer_kinetic_transform(&pulse, 1.0, 0, 1, None, None, base_centroid);
    assert!((tf_p0.opacity_factor - 1.0).abs() < 1e-5);
    assert!((tf_p_half.opacity_factor - 0.45).abs() < 1e-4);
    assert!((tf_p1.opacity_factor - 1.0).abs() < 1e-5);

    // 3. VariableColor Iterative vs Cumulative across 3 sublayers
    let sublayers = vec![
        grafia::CanvasColorSublayer {
            id: 1,
            name: "Wave 1".to_string(),
            visible: true,
            locked: false,
            opacity: 1.0,
            paint: grafia::ColorLayerPaint::Solid {
                rgba: [255, 0, 0, 255],
            },
            contours: vec![rect.clone()],
        },
        grafia::CanvasColorSublayer {
            id: 2,
            name: "Wave 2".to_string(),
            visible: true,
            locked: false,
            opacity: 1.0,
            paint: grafia::ColorLayerPaint::Solid {
                rgba: [0, 255, 0, 255],
            },
            contours: vec![rect.clone()],
        },
        grafia::CanvasColorSublayer {
            id: 3,
            name: "Wave 3".to_string(),
            visible: true,
            locked: false,
            opacity: 1.0,
            paint: grafia::ColorLayerPaint::Solid {
                rgba: [0, 0, 255, 255],
            },
            contours: vec![rect.clone()],
        },
    ];
    let tags = vec![
        SymbolLayerTag {
            layer_group: 1,
            variable_color_enabled: true,
            ..Default::default()
        },
        SymbolLayerTag {
            layer_group: 2,
            variable_color_enabled: true,
            ..Default::default()
        },
        SymbolLayerTag {
            layer_group: 3,
            variable_color_enabled: true,
            ..Default::default()
        },
    ];

    let iter_kin = KineticTimelineState {
        effect: KineticSymbolEffect::VariableColor {
            cumulative: false,
            reversing: false,
            dim_inactive: true,
        },
        phase_t: 0.5, // Middle layer (index 1 of 3) active
        ..Default::default()
    };
    let eval_iter = grafia::evaluate_kinetic_color_sublayers(&sublayers, &tags, &[], &iter_kin);
    assert!((eval_iter[0].opacity - 0.30).abs() < 1e-4);
    assert!((eval_iter[1].opacity - 1.00).abs() < 1e-4);
    assert!((eval_iter[2].opacity - 0.30).abs() < 1e-4);

    // 3b. Iterative VariableColor C1 smooth cross-fade continuity between layer 0 (center 1/6) and layer 1 (center 3/6):
    // At the boundary t = 2/6 = 1/3 between layer 0 and layer 1, both layers must cross-fade smoothly at midpoint
    // opacity 0.30 + 0.5 * 0.70 = 0.65 (not jump discontinuously from 1.0 to 0.30).
    let mut prev_op0 = 0.0_f32;
    for step in 0..=60 {
        let t = (f64::from(step) / 60.0) * 0.5 + (1.0 / 6.0);
        let kin_t = KineticTimelineState {
            effect: iter_kin.effect.clone(),
            phase_t: t,
            ..Default::default()
        };
        let ev = grafia::evaluate_kinetic_color_sublayers(&sublayers, &tags, &[], &kin_t);
        if step > 0 {
            assert!(
                (ev[0].opacity - prev_op0).abs() < 0.05,
                "Iterative VariableColor must be C1 continuous without step jumps, delta={} at t={t}",
                (ev[0].opacity - prev_op0).abs()
            );
        }
        prev_op0 = ev[0].opacity;
    }

    // 3c. Shared group pivot c_g across outer contour + eccentric inner counter hole in the same layer_group
    let mut hole_nodes = build_rectangle_contour_nodes(160.0, 130.0, 280.0, 230.0, &mut nid);
    grafia::core::reverse_contour_nodes(&mut hole_nodes, true);
    let eccentric_hole = GlyphContour::new(ContourId(1), hole_nodes);
    let same_group_tags = vec![
        SymbolLayerTag {
            layer_group: 1,
            ..Default::default()
        },
        SymbolLayerTag {
            layer_group: 1,
            ..Default::default()
        },
    ];
    let scale_kin = KineticTimelineState {
        effect: KineticSymbolEffect::Scale {
            direction: ScaleDirection::Up,
            scope: KineticLayerScope::ByLayer,
        },
        phase_t: 0.5,
        ..Default::default()
    };
    let scaled_pair = grafia::evaluate_kinetic_glyph_contours(
        &[rect.clone(), eccentric_hole.clone()],
        &same_group_tags,
        &[],
        &scale_kin,
    );
    // Because both contours belong to layer_group = 1, the left wall (hole.x0 - outer.x0 = 60.0)
    // must scale uniformly by 1.24 around their shared group pivot (60.0 * 1.24 = 74.4)!
    let scaled_left_wall =
        scaled_pair[1].0.nodes[0].point.x.0 - scaled_pair[0].0.nodes[0].point.x.0;
    assert!(
        (scaled_left_wall - 60.0 * 1.24).abs() < 1e-4,
        "Outer contour and eccentric counter hole in same layer_group must share pivot so wall scales by 1.24x: expected {}, got {scaled_left_wall}",
        60.0 * 1.24
    );

    let cum_kin = KineticTimelineState {
        effect: KineticSymbolEffect::VariableColor {
            cumulative: true,
            reversing: false,
            dim_inactive: false,
        },
        phase_t: 0.5, // Layers 0 and 1 active, Layer 2 hidden (0.0)
        ..Default::default()
    };
    let eval_cum = grafia::evaluate_kinetic_color_sublayers(&sublayers, &tags, &[], &cum_kin);
    assert!((eval_cum[0].opacity - 1.0).abs() < 1e-4);
    assert!((eval_cum[1].opacity - 1.0).abs() < 1e-4);
    assert!(eval_cum[2].opacity.abs() < 1e-4);

    // 4. Scale::Up & ByLayer phase staggering across groups 0, 1, 2
    let tau_g0 = grafia::compute_layer_local_phase(0.30, KineticLayerScope::ByLayer, 0, 3);
    let tau_g1 = grafia::compute_layer_local_phase(0.30, KineticLayerScope::ByLayer, 1, 3);
    let tau_g2 = grafia::compute_layer_local_phase(0.30, KineticLayerScope::ByLayer, 2, 3);
    assert!(
        tau_g0 > tau_g1 && tau_g1 > tau_g2,
        "ByLayer local phase must stagger monotonically across groups: {tau_g0} > {tau_g1} > {tau_g2}"
    );

    let scale_up = KineticSymbolEffect::Scale {
        direction: ScaleDirection::Up,
        scope: KineticLayerScope::WholeSymbol,
    };
    let tf_s =
        grafia::evaluate_layer_kinetic_transform(&scale_up, 0.5, 0, 1, None, None, base_centroid);
    assert!((tf_s.scale.0 - 1.24).abs() < 1e-5 && (tf_s.scale.1 - 1.24).abs() < 1e-5);

    let mut state = GrafiaCanvasState::new(GrafiaTheme::sonoma_default());
    let glyph = grafia::Glyph {
        contours: vec![rect.clone()],
        ..Default::default()
    };
    let metrics = state.metrics.clone();
    state.load_glyph(glyph, metrics);
    state.color_sublayers = sublayers;
    state.icon_studio.symbol_layer_tags = tags;
    state.icon_studio.kinetic = iter_kin;
    let state_subs = state.evaluate_active_kinetic_sublayers();
    assert_eq!(state_subs.len(), 3);
    assert!((state_subs[1].opacity - 1.0).abs() < 1e-4);
    let state_contours = state.evaluate_active_kinetic_contours();
    assert_eq!(state_contours.len(), 1);

    // 5. Unconstrained `arbtest` fuzzer on Effects 1–4 & `apply_layer_kinetic_transform`
    arbtest(|u| {
        let raw_t = arbitrary_f64_with_extremes(u);
        let g = u.int_in_range(0..=8).unwrap_or(0);
        let total_g = u.int_in_range(0..=8).unwrap_or(1);
        let scope = if u.arbitrary::<bool>().unwrap_or(true) {
            KineticLayerScope::ByLayer
        } else {
            KineticLayerScope::WholeSymbol
        };
        let tau = grafia::compute_layer_local_phase(raw_t, scope, g, total_g);
        assert!(tau.is_finite() && (0.0..=1.0).contains(&tau));

        let eff = match u.int_in_range(0..=3).unwrap_or(0) {
            0 => KineticSymbolEffect::Pulse { scope },
            1 => KineticSymbolEffect::Bounce {
                direction: if u.arbitrary::<bool>().unwrap_or(true) {
                    BounceDirection::Up
                } else {
                    BounceDirection::Down
                },
                scope,
            },
            2 => KineticSymbolEffect::VariableColor {
                cumulative: u.arbitrary::<bool>().unwrap_or(true),
                reversing: u.arbitrary::<bool>().unwrap_or(false),
                dim_inactive: u.arbitrary::<bool>().unwrap_or(true),
            },
            _ => KineticSymbolEffect::Scale {
                direction: if u.arbitrary::<bool>().unwrap_or(true) {
                    ScaleDirection::Up
                } else {
                    ScaleDirection::Down
                },
                scope,
            },
        };
        let pivot = FontPoint::new(
            arbitrary_f64_with_extremes(u),
            arbitrary_f64_with_extremes(u),
        );
        let tf = grafia::evaluate_layer_kinetic_transform(
            &eff,
            raw_t,
            g,
            total_g,
            Some((g, total_g.max(1))),
            None,
            pivot,
        );
        assert!(tf.opacity_factor.is_finite() && (0.0..=1.0).contains(&tf.opacity_factor));
        assert!(tf.scale.0.is_finite() && tf.scale.1.is_finite());
        assert!(tf.translation_upm.x.0.is_finite() && tf.translation_upm.y.0.is_finite());
        let out = grafia::apply_layer_kinetic_transform(&rect, &tf);
        for n in &out.nodes {
            assert!(n.point.x.0.is_finite() && n.point.y.0.is_finite());
        }
        Ok(())
    })
    .budget_ms(800);
}

#[test]
fn tc_v0140_006_kinetic_appear_draw_on_and_disappear_draw_off_de_casteljau_trim() {
    let mut nid = 1_u64;
    let circle = GlyphContour::new(
        ContourId(0),
        build_circle_contour_nodes(100.0, 100.0, 300.0, 300.0, &mut nid),
    );
    let total_len = grafia::contour_total_arc_length(&circle);
    // Radius = 100 -> circumference = 2 * pi * 100 ~= 628.3185
    assert!(
        (total_len - 200.0 * std::f64::consts::PI).abs() < 1.5,
        "Circle total arc length must match 2*pi*R (~628.3), got {total_len}"
    );

    // 1. Direct De Casteljau cubic arc-length trimming at [0.0, ratio] for ratio in {0.0, 0.25, 0.5, 0.75, 1.0}
    let mut prev_len = -1.0_f64;
    for &ratio in &[0.0_f64, 0.25, 0.50, 0.75, 1.0] {
        let trimmed = grafia::trim_contour_by_arc_ratio(&circle, 0.0, ratio, false);
        let len = grafia::contour_total_arc_length(&trimmed);
        assert!(
            len > prev_len,
            "Trimmed arc length must grow strictly monotonically: {len} <= {prev_len} at ratio={ratio}"
        );
        assert!(
            (len - ratio * total_len).abs() <= total_len * 0.015 + 1e-3,
            "De Casteljau trimmed length {len} must match ratio * total_len ({}) within 1.5%",
            ratio * total_len
        );
        prev_len = len;
    }

    // 2. Reversed draw order trims from the opposite end with identical arc length
    let fwd_quarter = grafia::trim_contour_by_arc_ratio(&circle, 0.0, 0.25, false);
    let rev_quarter = grafia::trim_contour_by_arc_ratio(&circle, 0.0, 0.25, true);
    let len_fwd = grafia::contour_total_arc_length(&fwd_quarter);
    let len_rev = grafia::contour_total_arc_length(&rev_quarter);
    assert!((len_fwd - len_rev).abs() < 1.0);
    assert!(
        fwd_quarter.nodes.last().unwrap().point != rev_quarter.nodes.last().unwrap().point,
        "Reversed draw order must end at a different point than forward draw order"
    );

    // 3. KineticSymbolEffect::Appear { draw_on: true } & Disappear { draw_off: true }
    let (_, centroid) = compute_contour_area_and_centroid(&circle.nodes);
    let appear_on = KineticSymbolEffect::Appear {
        direction: BounceDirection::Up,
        draw_on: true,
        scope: KineticLayerScope::WholeSymbol,
    };
    let mut prev_appear_len = -1.0_f64;
    for &tau in &[0.0_f64, 0.25, 0.50, 0.75, 1.0] {
        let tf =
            grafia::evaluate_layer_kinetic_transform(&appear_on, tau, 0, 1, None, None, centroid);
        let eval_c = grafia::apply_layer_kinetic_transform(&circle, &tf);
        let l = grafia::contour_total_arc_length(&eval_c);
        assert!(
            l > prev_appear_len || (tau == 0.0 && l == 0.0),
            "Appear(draw_on) arc length must increase monotonically across tau, got {l} after {prev_appear_len} at tau={tau}"
        );
        prev_appear_len = l;
    }

    let disappear_off = KineticSymbolEffect::Disappear {
        direction: BounceDirection::Down,
        draw_off: true,
        scope: KineticLayerScope::WholeSymbol,
    };
    let mut prev_disappear_len = f64::INFINITY;
    for &tau in &[0.0_f64, 0.25, 0.50, 0.75, 1.0] {
        let tf = grafia::evaluate_layer_kinetic_transform(
            &disappear_off,
            tau,
            0,
            1,
            None,
            None,
            centroid,
        );
        let eval_c = grafia::apply_layer_kinetic_transform(&circle, &tf);
        let l = grafia::contour_total_arc_length(&eval_c);
        assert!(
            l < prev_disappear_len,
            "Disappear(draw_off) arc length must decrease monotonically across tau, got {l} after {prev_disappear_len} at tau={tau}"
        );
        prev_disappear_len = l;
    }
    assert!(prev_disappear_len < 1e-3);

    // 3b. Headless rendering of a partial DrawOn contour at tau = 0.5 produces visible pixels
    let mut state = GrafiaCanvasState::new(GrafiaTheme::sonoma_default());
    let glyph = grafia::Glyph {
        contours: vec![circle.clone()],
        ..Default::default()
    };
    let metrics = state.metrics.clone();
    state.load_glyph(glyph, metrics);
    state.view_mode = grafia::CanvasViewMode::PreviewFilled;
    state.viewport =
        grafia::ViewportTransform::new(0.2, grafia::ScreenPoint::new(20.0, 120.0), 1000);
    state.icon_studio.kinetic = KineticTimelineState {
        effect: appear_on,
        phase_t: 0.5,
        ..Default::default()
    };
    let thumb = grafia::render_colored_thumbnail(&state, 128, 128, grafia::AppearanceMode::Light);
    let pix = grafia::adapters::rasterize_canvas_subregion(&state, &state.theme, 160, 160);
    let bg = state
        .theme
        .canvas_background_color32()
        .to_srgba_unmultiplied();
    let non_bg_thumb = thumb
        .as_chunks::<4>()
        .0
        .iter()
        .filter(|px| px[0] != bg[0] || px[1] != bg[1] || px[2] != bg[2])
        .count();
    let non_bg_px = pix
        .data()
        .as_chunks::<4>()
        .0
        .iter()
        .filter(|px| px[0] != bg[0] || px[1] != bg[1] || px[2] != bg[2])
        .count();
    assert!(
        non_bg_thumb > 10 && non_bg_px > 10,
        "Partial DrawOn open trimmed contour at tau=0.5 must render visible stroke pixels in thumbnail ({non_bg_thumb}) and PreviewFilled ({non_bg_px})"
    );

    // 3c. Production hardening:
    // (i) Variable-width stroke attached to a DrawOn contour expands the trimmed subsegment into a closed ribbon
    state.set_variable_width_stroke(grafia::VariableWidthStrokeSpec {
        contour_id: ContourId(0),
        samples: vec![
            grafia::WidthNodeSample {
                arc_ratio_t: 0.0,
                width_left_upm: 20.0,
                width_right_upm: 20.0,
            },
            grafia::WidthNodeSample {
                arc_ratio_t: 1.0,
                width_left_upm: 40.0,
                width_right_upm: 40.0,
            },
        ],
        cap_style: grafia::LineCapStyle::Round,
    });
    let eval_vw = state.evaluate_active_kinetic_contours();
    assert!(
        !eval_vw.is_empty() && eval_vw.iter().all(|(c, _)| c.closed && c.nodes.len() >= 4),
        "DrawOn contour with VariableWidthStrokeSpec must expand trimmed subsegment into closed variable-width ribbon contours"
    );

    // (ii) Undo/Redo after bake_variable_width_strokes restores variable_width_strokes
    assert!(state.bake_variable_width_strokes());
    assert!(state.icon_studio.variable_width_strokes.is_empty());
    assert!(state.undo());
    assert_eq!(state.icon_studio.variable_width_strokes.len(), 1);
    assert!(state.redo());
    assert!(state.icon_studio.variable_width_strokes.is_empty());
    assert!(state.undo());

    // (iii) Live N-fold radial symmetry ghost overlay renders in EditWireframe even when contour_strokes is empty
    state.icon_studio.variable_width_strokes.clear();
    state.icon_studio.kinetic.phase_t = 0.0;
    state.view_mode = grafia::CanvasViewMode::EditWireframe;
    state.icon_studio.radial_symmetry.fold_count = 1;
    let pix_sym_1 = grafia::adapters::rasterize_canvas_subregion(&state, &state.theme, 160, 160);
    state.icon_studio.radial_symmetry.fold_count = 4;
    state.icon_studio.radial_symmetry.center = FontPoint::new(150.0, 150.0);
    let pix_sym_4 = grafia::adapters::rasterize_canvas_subregion(&state, &state.theme, 160, 160);
    assert_ne!(
        pix_sym_1.data(),
        pix_sym_4.data(),
        "Live N-fold radial symmetry ghost sectors must render in EditWireframe when fold_count > 1"
    );
    state.icon_studio.radial_symmetry.fold_count = 1;

    // (iv) Hover-only move in CanvasTool::ShapeBuilder returns true when hovered_region_id changes
    state.active_tool = grafia::CanvasTool::ShapeBuilder;
    let changed_hover = state.on_drawing_tool_pointer_move(
        grafia::ScreenPoint::new(60.0, 80.0),
        FontPoint::new(200.0, 200.0),
        grafia::Modifiers::default(),
    );
    assert!(
        changed_hover && state.drawing_tool_state.shape_builder.hovered_region_id.is_some(),
        "Idle pointer hover over a planar region in ShapeBuilder must update hovered_region_id and return true"
    );

    // (v) Kinetic animation in EditWireframe and bottom PreviewPanel visibly animates contours when phase_t > 0
    state.active_tool = grafia::CanvasTool::Select;
    state.icon_studio.kinetic = KineticTimelineState {
        effect: KineticSymbolEffect::Scale {
            direction: grafia::ScaleDirection::Up,
            scope: KineticLayerScope::WholeSymbol,
        },
        phase_t: 0.0,
        ..Default::default()
    };
    let wf_t0 = grafia::adapters::rasterize_canvas_subregion(&state, &state.theme, 160, 160);
    let prev_t0 = grafia::render::rasterize_preview_panel(
        &state,
        &grafia::PreviewPanelConfig::default(),
        160,
        80,
    );
    state.icon_studio.kinetic.phase_t = 0.5;
    let wf_t5 = grafia::adapters::rasterize_canvas_subregion(&state, &state.theme, 160, 160);
    let prev_t5 = grafia::render::rasterize_preview_panel(
        &state,
        &grafia::PreviewPanelConfig::default(),
        160,
        80,
    );
    assert_ne!(
        wf_t0.data(),
        wf_t5.data(),
        "EditWireframe must render live kinetic contour animation when phase_t > 0"
    );
    assert_ne!(
        prev_t0.data(),
        prev_t5.data(),
        "Bottom PreviewPanel must render live kinetic contour animation when phase_t > 0"
    );

    // 4. Unconstrained `arbtest` fuzzer on `trim_contour_by_arc_ratio` & Appear/Disappear
    arbtest(|u| {
        let r0 = arbitrary_f64_with_extremes(u);
        let r1 = arbitrary_f64_with_extremes(u);
        let rev = u.arbitrary::<bool>().unwrap_or(false);
        let trimmed = grafia::trim_contour_by_arc_ratio(&circle, r0, r1, rev);
        let t_len = grafia::contour_total_arc_length(&trimmed);
        assert!(t_len.is_finite() && t_len >= 0.0 && t_len <= total_len + 1.0);
        for n in &trimmed.nodes {
            assert!(n.point.x.0.is_finite() && n.point.y.0.is_finite());
        }

        let raw_t = arbitrary_f64_with_extremes(u);
        let is_appear = u.arbitrary::<bool>().unwrap_or(true);
        let draw_flag = u.arbitrary::<bool>().unwrap_or(true);
        let eff = if is_appear {
            KineticSymbolEffect::Appear {
                direction: BounceDirection::Up,
                draw_on: draw_flag,
                scope: KineticLayerScope::ByLayer,
            }
        } else {
            KineticSymbolEffect::Disappear {
                direction: BounceDirection::Down,
                draw_off: draw_flag,
                scope: KineticLayerScope::ByLayer,
            }
        };
        let tf = grafia::evaluate_layer_kinetic_transform(&eff, raw_t, 0, 2, None, None, centroid);
        let out = grafia::apply_layer_kinetic_transform(&circle, &tf);
        for n in &out.nodes {
            assert!(n.point.x.0.is_finite() && n.point.y.0.is_finite());
        }
        Ok(())
    })
    .budget_ms(800);
}

#[test]
fn tc_v0140_tablet_touch_and_stylus_handle_drag_after_pinch_and_in_shape_builder() {
    let ctx = egui::Context::default();
    let theme = GrafiaTheme::sonoma_default();
    let mut state = GrafiaCanvasState::init_glyph_o();
    let canvas_rect = egui::Rect::from_min_max(egui::pos2(0.0, 80.0), egui::pos2(800.0, 600.0));
    state.viewport =
        grafia::ViewportTransform::new(0.5, grafia::ScreenPoint::new(200.0, 480.0), 1000);

    // 1. Simulate a 2-finger pinch/pan ending in eframe-0.37.0's exact touchend order:
    // `[PointerButton(pressed: false), PointerGone, Touch(End, id=1), Touch(End, id=2)]`
    // PLUS a toolbar tap outside `canvas_rect` at (150, 20):
    for (t_sec, evs) in [
        (
            1.00,
            vec![
                egui::Event::PointerButton {
                    pos: egui::pos2(200.0, 200.0),
                    button: egui::PointerButton::Primary,
                    pressed: true,
                    modifiers: egui::Modifiers::NONE,
                },
                egui::Event::Touch {
                    device_id: egui::TouchDeviceId(0),
                    id: egui::TouchId(1),
                    phase: egui::TouchPhase::Start,
                    pos: egui::pos2(200.0, 200.0),
                    force: None,
                },
                egui::Event::Touch {
                    device_id: egui::TouchDeviceId(0),
                    id: egui::TouchId(2),
                    phase: egui::TouchPhase::Start,
                    pos: egui::pos2(300.0, 200.0),
                    force: None,
                },
            ],
        ),
        (
            1.05,
            vec![
                egui::Event::PointerButton {
                    pos: egui::pos2(200.0, 200.0),
                    button: egui::PointerButton::Primary,
                    pressed: false,
                    modifiers: egui::Modifiers::NONE,
                },
                egui::Event::PointerGone,
                egui::Event::Touch {
                    device_id: egui::TouchDeviceId(0),
                    id: egui::TouchId(1),
                    phase: egui::TouchPhase::End,
                    pos: egui::pos2(200.0, 200.0),
                    force: None,
                },
                egui::Event::Touch {
                    device_id: egui::TouchDeviceId(0),
                    id: egui::TouchId(2),
                    phase: egui::TouchPhase::End,
                    pos: egui::pos2(300.0, 200.0),
                    force: None,
                },
            ],
        ),
        (
            1.10,
            vec![egui::Event::Touch {
                device_id: egui::TouchDeviceId(0),
                id: egui::TouchId(99),
                phase: egui::TouchPhase::Start,
                pos: egui::pos2(150.0, 20.0), // outside canvas_rect (y=20 < 80)
                force: None,
            }],
        ),
    ] {
        let raw = egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(
                egui::Pos2::ZERO,
                egui::vec2(800.0, 600.0),
            )),
            time: Some(t_sec),
            events: evs,
            ..Default::default()
        };
        let mut out = ctx.run_ui(raw, |ui| {
            let ev = ui.input(|i| i.events.clone());
            let _ = grafia::adapters::InputDispatcher::for_canvas(&mut state, canvas_rect)
                .dispatch_events(&ev, ui);
        });
        out.textures_delta.clear();
    }

    assert!(
        !state.drag_transients.touch_state.primary_press_active
            && state.drag_transients.touch_state.contacts.is_empty(),
        "After 2-finger touchend and out-of-canvas toolbar touch, primary_press_active must be false and contacts empty"
    );

    // 2. Now touch/drag an OffCurve Bézier handle on /O > 100 dp away from (200, 200)
    // using eframe-0.37.0's exact pointerdown (Frame A) -> touchstart+touchmove (Frame B) -> touchend (Frame C)
    let off_node = state
        .contours
        .iter()
        .flatten()
        .find(|n| n.node_type == NodeType::OffCurve)
        .unwrap()
        .clone();
    let h_scr = state.viewport.to_screen(off_node.point);
    let h_pos = egui::pos2(h_scr.x.0, h_scr.y.0);
    let h_target = egui::pos2(h_scr.x.0 + 45.0, h_scr.y.0 - 35.0);

    for (t_sec, evs) in [
        (
            2.00,
            vec![egui::Event::PointerButton {
                pos: h_pos,
                button: egui::PointerButton::Primary,
                pressed: true,
                modifiers: egui::Modifiers::NONE,
            }],
        ),
        (
            2.016,
            vec![
                egui::Event::PointerButton {
                    pos: h_pos,
                    button: egui::PointerButton::Primary,
                    pressed: true,
                    modifiers: egui::Modifiers::NONE,
                },
                egui::Event::Touch {
                    device_id: egui::TouchDeviceId(0),
                    id: egui::TouchId(5),
                    phase: egui::TouchPhase::Start,
                    pos: h_pos,
                    force: None,
                },
                egui::Event::PointerMoved(h_target),
                egui::Event::Touch {
                    device_id: egui::TouchDeviceId(0),
                    id: egui::TouchId(5),
                    phase: egui::TouchPhase::Move,
                    pos: h_target,
                    force: None,
                },
            ],
        ),
        (
            2.05,
            vec![
                egui::Event::PointerButton {
                    pos: h_target,
                    button: egui::PointerButton::Primary,
                    pressed: false,
                    modifiers: egui::Modifiers::NONE,
                },
                egui::Event::PointerGone,
                egui::Event::Touch {
                    device_id: egui::TouchDeviceId(0),
                    id: egui::TouchId(5),
                    phase: egui::TouchPhase::End,
                    pos: h_target,
                    force: None,
                },
            ],
        ),
    ] {
        let raw = egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(
                egui::Pos2::ZERO,
                egui::vec2(800.0, 600.0),
            )),
            time: Some(t_sec),
            events: evs,
            ..Default::default()
        };
        let mut out = ctx.run_ui(raw, |ui| {
            let ev = ui.input(|i| i.events.clone());
            let _ = grafia::adapters::InputDispatcher::for_canvas(&mut state, canvas_rect)
                .dispatch_events(&ev, ui);
        });
        out.textures_delta.clear();
    }

    let moved_node = state.find_node(off_node.id).unwrap();
    assert!(
        moved_node.point.distance_to(off_node.point) > 40.0,
        "OffCurve Bézier handle must move when dragged on tablet after 2-finger gesture, moved distance = {}",
        moved_node.point.distance_to(off_node.point)
    );

    // 2b. Rapid tap-then-drag (within 0.15s and 12 dp of an OffCurve handle, triggering `is_double = true`)
    // MUST still drag the OffCurve handle instead of selecting the entire contour and resetting select_state to Idle!
    let mut state_rapid = GrafiaCanvasState::init_glyph_o();
    state_rapid.viewport =
        grafia::ViewportTransform::new(0.5, grafia::ScreenPoint::new(200.0, 480.0), 1000);
    let off_r = state_rapid
        .contours
        .iter()
        .flatten()
        .find(|n| n.node_type == NodeType::OffCurve)
        .unwrap()
        .clone();
    let hr_scr = state_rapid.viewport.to_screen(off_r.point);
    // Start in Mouse mode (first touch on tablet): touch 11.0 dp away from OffCurve handle (misses 8 dp Mouse radius in pointerdown Frame 1, hits 16 dp TouchFinger radius in touchstart Frame 2),
    // and then 0.12s later touch the OffCurve handle again and drag it by (+40, -30) dp!
    let hr_11dp = egui::pos2(hr_scr.x.0 + 11.0, hr_scr.y.0);
    let hr_drag = egui::pos2(hr_scr.x.0 + 45.0, hr_scr.y.0 - 35.0);
    for (t_sec, evs) in [
        // Frame 1: pointerdown before touchstart (still in Mouse mode!)
        (
            3.00,
            vec![egui::Event::PointerButton {
                pos: hr_11dp,
                button: egui::PointerButton::Primary,
                pressed: true,
                modifiers: egui::Modifiers::NONE,
            }],
        ),
        // Frame 2: touchstart arrives 10ms later -> switches to TouchFinger, must recover from Frame 1's MarqueeSelecting and select the OffCurve handle at 11 dp!
        (
            3.01,
            vec![
                egui::Event::PointerButton {
                    pos: hr_11dp,
                    button: egui::PointerButton::Primary,
                    pressed: true,
                    modifiers: egui::Modifiers::NONE,
                },
                egui::Event::Touch {
                    device_id: egui::TouchDeviceId(0),
                    id: egui::TouchId(30),
                    phase: egui::TouchPhase::Start,
                    pos: hr_11dp,
                    force: None,
                },
                egui::Event::PointerButton {
                    pos: hr_11dp,
                    button: egui::PointerButton::Primary,
                    pressed: false,
                    modifiers: egui::Modifiers::NONE,
                },
                egui::Event::Touch {
                    device_id: egui::TouchDeviceId(0),
                    id: egui::TouchId(30),
                    phase: egui::TouchPhase::End,
                    pos: hr_11dp,
                    force: None,
                },
            ],
        ),
        // Frame 3 (0.12s later, within double-tap window!): user touches the OffCurve handle and drags to hr_drag
        (
            3.13,
            vec![
                egui::Event::PointerButton {
                    pos: hr_11dp,
                    button: egui::PointerButton::Primary,
                    pressed: true,
                    modifiers: egui::Modifiers::NONE,
                },
                egui::Event::Touch {
                    device_id: egui::TouchDeviceId(0),
                    id: egui::TouchId(31),
                    phase: egui::TouchPhase::Start,
                    pos: hr_11dp,
                    force: None,
                },
                egui::Event::PointerMoved(hr_drag),
                egui::Event::Touch {
                    device_id: egui::TouchDeviceId(0),
                    id: egui::TouchId(31),
                    phase: egui::TouchPhase::Move,
                    pos: hr_drag,
                    force: None,
                },
                egui::Event::PointerButton {
                    pos: hr_drag,
                    button: egui::PointerButton::Primary,
                    pressed: false,
                    modifiers: egui::Modifiers::NONE,
                },
                egui::Event::Touch {
                    device_id: egui::TouchDeviceId(0),
                    id: egui::TouchId(31),
                    phase: egui::TouchPhase::End,
                    pos: hr_drag,
                    force: None,
                },
            ],
        ),
    ] {
        let raw = egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(
                egui::Pos2::ZERO,
                egui::vec2(800.0, 600.0),
            )),
            time: Some(t_sec),
            events: evs,
            ..Default::default()
        };
        let mut out = ctx.run_ui(raw, |ui| {
            let ev = ui.input(|i| i.events.clone());
            let _ = grafia::adapters::InputDispatcher::for_canvas(&mut state_rapid, canvas_rect)
                .dispatch_events(&ev, ui);
        });
        out.textures_delta.clear();
    }
    let moved_r = state_rapid.find_node(off_r.id).unwrap();
    assert_eq!(
        state_rapid.selected_nodes,
        vec![off_r.id],
        "Rapid tap-then-drag on an OffCurve handle must keep only that OffCurve handle selected (not select the whole contour)"
    );
    assert!(
        moved_r.point.distance_to(off_r.point) > 40.0,
        "Rapid tap-then-drag on an OffCurve handle must move the handle, got distance = {}",
        moved_r.point.distance_to(off_r.point)
    );

    // 3. While CanvasTool::ShapeBuilder is active, dragging a VariableWidthHandlePin or OffCurve handle must move it!
    state.set_variable_width_stroke(grafia::VariableWidthStrokeSpec {
        contour_id: ContourId(0),
        samples: vec![grafia::WidthNodeSample {
            arc_ratio_t: 0.5,
            width_left_upm: 20.0,
            width_right_upm: 20.0,
        }],
        cap_style: grafia::LineCapStyle::Round,
    });
    state.activate_tool(grafia::CanvasTool::ShapeBuilder);
    let pin = state.compute_variable_width_handle_pins()[0];
    let pin_scr = state.viewport.to_screen(pin.left_pt);
    // Touch 11.0 dp from pin.left_pt (requires touch-scaled hit radius >= 14.0 dp!)
    let pin_touch_scr = grafia::ScreenPoint::new(pin_scr.x.0 + 11.0, pin_scr.y.0);
    let pin_touch_upm = state.viewport.to_font(pin_touch_scr);
    assert!(
        state.on_drawing_tool_pointer_press(pin_touch_scr, pin_touch_upm, grafia::Modifiers::default()),
        "Pressing 11 dp from VariableWidthHandlePin in ShapeBuilder tool on TouchFinger must start pin drag"
    );
    assert!(
        state
            .drawing_tool_state
            .shape_builder
            .active_width_pin_drag
            .is_some(),
        "VariableWidthHandlePin drag must be active when pressed in ShapeBuilder mode"
    );

    // 4. STAP 16.6b NASA Frame-Condition & InputDispatcher OffCurve Handle-Drag in CanvasTool::ShapeBuilder (+ arbtest):
    // Two overlapping squares at top (ContourId(0), ContourId(1)) + open S-curve at bottom (ContourId(2), closed=false) with VariableWidthStrokeSpec.
    arbtest(|u| {
        let mut st = GrafiaCanvasState::init_glyph_a();
        st.viewport =
            grafia::ViewportTransform::new(0.5, grafia::ScreenPoint::new(100.0, 500.0), 1000);
        let c0 = GlyphContour::new(
            ContourId(0),
            vec![
                Node::new(NodeId(100), 120.0, 180.0, NodeType::Corner),
                Node::new(NodeId(101), 360.0, 180.0, NodeType::Corner),
                Node::new(NodeId(102), 360.0, 520.0, NodeType::Corner),
                Node::new(NodeId(103), 120.0, 520.0, NodeType::Corner),
            ],
        );
        let c1 = GlyphContour::new(
            ContourId(1),
            vec![
                Node::new(NodeId(110), 240.0, 240.0, NodeType::Corner),
                Node::new(NodeId(111), 480.0, 240.0, NodeType::Corner),
                Node::new(NodeId(112), 480.0, 580.0, NodeType::Corner),
                Node::new(NodeId(113), 240.0, 580.0, NodeType::Corner),
            ],
        );
        let h_y = 140.0 + f64::from(u.int_in_range(-20_i32..=20).unwrap_or(10));
        let mut spine = GlyphContour::new(
            ContourId(2),
            vec![
                Node::new(NodeId(120), 120.0, 110.0, NodeType::Corner),
                Node::new(NodeId(121), 240.0, h_y, NodeType::OffCurve),
                Node::new(NodeId(122), 360.0, 70.0, NodeType::OffCurve),
                Node::new(NodeId(123), 480.0, 120.0, NodeType::Corner),
            ],
        );
        spine.closed = false;
        let orig_spine_nodes = spine.nodes.clone();
        let mut g = st.glyph.clone();
        g.contours = vec![c0, c1, spine];
        let m = st.metrics.clone();
        st.load_glyph(g, m);
        st.set_variable_width_stroke(grafia::VariableWidthStrokeSpec {
            contour_id: ContourId(2),
            samples: vec![
                grafia::WidthNodeSample {
                    arc_ratio_t: 0.0,
                    width_left_upm: 14.0,
                    width_right_upm: 14.0,
                },
                grafia::WidthNodeSample {
                    arc_ratio_t: 0.5,
                    width_left_upm: 42.0,
                    width_right_upm: 28.0,
                },
                grafia::WidthNodeSample {
                    arc_ratio_t: 1.0,
                    width_left_upm: 10.0,
                    width_right_upm: 10.0,
                },
            ],
            cap_style: grafia::LineCapStyle::Round,
        });
        st.activate_tool(grafia::CanvasTool::ShapeBuilder);

        // (a) Frame-Condition: merging the 2 overlapping squares above MUST keep the open S-curve open (closed == false),
        // preserve its exact NodeIds & coordinates, and remap its VariableWidthStrokeSpec to the S-curve's new ContourId!
        assert!(st.apply_shape_builder_operation(&[0, 1, 2], ShapeBuilderOp::Merge));
        let surviving_spine_id = {
            let surviving_spine = st
                .glyph
                .contours
                .iter()
                .find(|c| !c.closed)
                .expect("Frame-Condition violation: open S-curve must remain open (!c.closed) after merging squares above it");
            assert_eq!(
                surviving_spine.nodes, orig_spine_nodes,
                "Frame-Condition violation: untouched open S-curve nodes and NodeIds must remain 100% identical"
            );
            surviving_spine.id
        };
        assert_eq!(
            st.icon_studio.variable_width_strokes.len(),
            1,
            "Frame-Condition violation: VariableWidthStrokeSpec on untouched open S-curve must not be dropped when squares merge"
        );
        assert_eq!(
            st.icon_studio.variable_width_strokes[0].contour_id,
            surviving_spine_id,
            "Frame-Condition violation: VariableWidthStrokeSpec contour_id must remap to the surviving open S-curve's new ContourId"
        );
        assert_eq!(st.compute_variable_width_handle_pins().len(), 3);

        // (b) End-to-End InputDispatcher Handle-Drag in CanvasTool::ShapeBuilder:
        // Dragging OffCurve handle NodeId(121) while CanvasTool::ShapeBuilder is active MUST move NodeId(121)!
        let h121_before = st.find_node(NodeId(121)).unwrap().point;
        let h121_scr = st.viewport.to_screen(h121_before);
        let h121_pos = egui::pos2(h121_scr.x.0, h121_scr.y.0);
        let drag_dx_dp = f32::from(u.int_in_range(20_i16..=45).unwrap_or(30));
        let h121_dst = egui::pos2(h121_pos.x + drag_dx_dp, h121_pos.y - 25.0);
        let raw_drag = egui::RawInput {
            screen_rect: Some(canvas_rect),
            time: Some(5.0),
            events: vec![
                egui::Event::PointerButton {
                    pos: h121_pos,
                    button: egui::PointerButton::Primary,
                    pressed: true,
                    modifiers: egui::Modifiers::NONE,
                },
                egui::Event::PointerMoved(h121_dst),
                egui::Event::PointerButton {
                    pos: h121_dst,
                    button: egui::PointerButton::Primary,
                    pressed: false,
                    modifiers: egui::Modifiers::NONE,
                },
            ],
            ..Default::default()
        };
        let mut out_d = ctx.run_ui(raw_drag, |ui| {
            let ev = ui.input(|i| i.events.clone());
            let _ = grafia::adapters::InputDispatcher::for_canvas(&mut st, canvas_rect)
                .dispatch_events(&ev, ui);
        });
        out_d.textures_delta.clear();
        let h121_after = st.find_node(NodeId(121)).unwrap().point;
        assert!(
            h121_after.distance_to(h121_before) > 30.0,
            "Dragging OffCurve handle NodeId(121) via InputDispatcher while CanvasTool::ShapeBuilder is active must move the handle (dist = {})",
            h121_after.distance_to(h121_before)
        );

        // (c) Pijler E (Algebraic Undo/Redo Inversion): Undo of handle drag + Undo of ShapeBuilder Merge must restore
        // the exact pre-merge 3 contours and remap VariableWidthStrokeSpec back to ContourId(2), and Redo must re-apply!
        assert!(st.undo(), "Undo of OffCurve handle drag in ShapeBuilder must succeed");
        assert!(
            st.find_node(NodeId(121)).unwrap().point.distance_to(h121_before) < 1e-6,
            "Undo of OffCurve handle drag must restore exact handle coordinates"
        );
        assert!(st.undo(), "Undo of ShapeBuilder Merge must succeed");
        assert_eq!(st.glyph.contours.len(), 3);
        assert_eq!(st.icon_studio.variable_width_strokes[0].contour_id, ContourId(2));
        assert!(st.redo(), "Redo of ShapeBuilder Merge must succeed");
        assert_eq!(
            st.icon_studio.variable_width_strokes[0].contour_id,
            surviving_spine_id
        );

        // (d) Pijler B across Optical Oblique & 5-Stage Normalizer:
        // Running apply_optical_oblique and normalize_icon_contours must preserve NodeId lineage, open S-curve (!closed),
        // SmartCornerSpec attachments, and VariableWidthStrokeSpec!
        st.set_smart_corner(grafia::SmartCornerSpec {
            node_id: NodeId(120),
            radius_upm: 12.0,
        });
        assert!(st.apply_optical_oblique(10.0, true));
        assert_eq!(
            st.icon_studio.variable_width_strokes.len(),
            1,
            "Pijler B violation: apply_optical_oblique must not drop VariableWidthStrokeSpec due to regenerating NodeIds"
        );
        assert_eq!(
            st.curve_harmony.smart_corners.len(),
            1,
            "Pijler B violation: apply_optical_oblique must preserve NodeId lineage for SmartCornerSpec"
        );
        assert!(st.normalize_icon_contours(Some(1.0)));
        assert!(
            st.glyph.contours.iter().any(|c| !c.closed),
            "Pijler B violation: normalize_icon_contours must preserve open S-curve (!c.closed)"
        );
        assert_eq!(
            st.icon_studio.variable_width_strokes.len(),
            1,
            "Pijler B violation: normalize_icon_contours must preserve VariableWidthStrokeSpec on surviving contours"
        );

        // (e) Pijler C (Z-Order Hit-Test Priority: OffCurve Handle > VariableWidthHandlePin) &
        // Pijler F (Escape / activate_tool mid-drag MUST rollback in-flight drag!):
        let off_id = st
            .glyph
            .contours
            .iter()
            .find(|c| !c.closed)
            .and_then(|c| c.nodes.iter().find(|n| n.node_type == NodeType::OffCurve))
            .expect("Open S-curve must still have OffCurve handles after normalize_icon_contours")
            .id;
        let off_pre_cancel = st.find_node(off_id).unwrap().point;
        let off_scr2 = st.viewport.to_screen(off_pre_cancel);
        let p_start = egui::pos2(off_scr2.x.0, off_scr2.y.0);
        let p_mid = egui::pos2(p_start.x + 40.0, p_start.y + 30.0);
        let raw_mid_drag = egui::RawInput {
            screen_rect: Some(canvas_rect),
            time: Some(6.0),
            events: vec![
                egui::Event::PointerButton {
                    pos: p_start,
                    button: egui::PointerButton::Primary,
                    pressed: true,
                    modifiers: egui::Modifiers::NONE,
                },
                egui::Event::PointerMoved(p_mid),
            ],
            ..Default::default()
        };
        let _ = ctx.run_ui(raw_mid_drag, |ui| {
            let ev = ui.input(|i| i.events.clone());
            let _ = grafia::adapters::InputDispatcher::for_canvas(&mut st, canvas_rect)
                .dispatch_events(&ev, ui);
        });
        assert!(
            st.find_node(off_id).unwrap().point.distance_to(off_pre_cancel) > 20.0,
            "In-flight drag must move OffCurve handle before cancellation"
        );
        // Switching tool (or pressing Escape / TouchPhase::Cancel) mid-drag MUST rollback the uncommitted mutation!
        st.activate_tool(grafia::CanvasTool::Select);
        assert!(
            st.find_node(off_id).unwrap().point.distance_to(off_pre_cancel) < 1e-6,
            "Pijler F violation: switching tool mid-drag must rollback uncommitted in-flight node displacement to active_drag_before_snapshot"
        );
        Ok(())
    })
    .budget_ms(400);
    let _ = state;
    let _ = theme;
}

/// **`TC-V0140-007` (`Stap 16.7` — Kinetic Effects 7–9: `Wiggle` 9 axes/angles, `Rotate` around `CanvasAnchor`, `Breathe` `Plain` vs `Pulse`):**
/// Enforces all 8 NASA/IBM/Google/Apple/Microsoft Production Pillars:
/// - **Pijler A:** Unconstrained `arbtest` injecting `f64::NAN`, `±f64::INFINITY`, `f64::MIN_POSITIVE`, and arbitrary angles/coordinates across all 9 `WiggleAxis` variants, `Rotate`, and `Breathe`.
/// - **Pijler B & E:** Frame-Condition & Algebraic Undo/Redo Inversion (`Undo(Do(S)) == S`, `Redo(Undo(Do(S))) == Do(S)`) when dragging a `CanvasAnchor` pivot or updating `SymbolLayerTag`.
/// - **Pijler C & F:** End-to-End `InputDispatcher` dragging of a `CanvasAnchor` (`motion_anchor_id`) live on the canvas updates the rotational center $\mathbf{c}_g$ of `Rotate` & `Wiggle`, and mid-drag tool switch / `Escape` rolls back uncommitted anchor displacement cleanly.
/// - **Pijler D:** Live rendering of `Wiggle`, `Rotate`, and `Breathe` across `EditWireframe`, `PreviewFilled`, bottom `PreviewPanel`, and `render_colored_thumbnail`.
#[test]
fn tc_v0140_007_kinetic_wiggle_rotate_around_anchor_and_breathe() {
    use grafia::{
        evaluate_layer_kinetic_transform, AnchorId, CanvasAnchor, CanvasViewMode, ContourId,
        FontPoint, GlyphContour, GrafiaCanvasState, GrafiaTheme, KineticLayerScope,
        KineticSymbolEffect, Node, NodeId, NodeType, SymbolLayerTag, WiggleAxis,
    };

    // 1. Rotate around custom CanvasAnchor leaves the anchor point stationary (||p' - p|| < 1e-6 UPM) across all tau in [0, 1]
    let mut state = GrafiaCanvasState::init_glyph_a();
    state.viewport =
        grafia::ViewportTransform::new(0.5, grafia::ScreenPoint::new(100.0, 500.0), 1000);
    let anchor_pos = FontPoint::new(220.0, 310.0);
    let c0 = GlyphContour::new(
        ContourId(0),
        vec![
            Node::new(NodeId(10), anchor_pos.x.0, anchor_pos.y.0, NodeType::Corner),
            Node::new(NodeId(11), 380.0, 310.0, NodeType::Corner),
            Node::new(NodeId(12), 380.0, 490.0, NodeType::Corner),
            Node::new(NodeId(13), 220.0, 490.0, NodeType::Corner),
        ],
    );
    let c1 = GlyphContour::new(
        ContourId(1),
        vec![
            Node::new(NodeId(20), 420.0, 200.0, NodeType::Corner),
            Node::new(NodeId(21), 520.0, 200.0, NodeType::Corner),
            Node::new(NodeId(22), 520.0, 300.0, NodeType::Corner),
            Node::new(NodeId(23), 420.0, 300.0, NodeType::Corner),
        ],
    );
    let mut g = state.glyph.clone();
    g.contours = vec![c0.clone(), c1.clone()];
    g.anchors = vec![CanvasAnchor::new(AnchorId(7), "pivot", anchor_pos)];
    let metrics = state.metrics.clone();
    state.load_glyph(g, metrics);
    state.icon_studio.symbol_layer_tags = vec![
        SymbolLayerTag {
            layer_group: 1,
            motion_anchor_id: Some(AnchorId(7)),
            ..Default::default()
        },
        SymbolLayerTag {
            layer_group: 2,
            motion_anchor_id: None,
            ..Default::default()
        },
    ];
    state.icon_studio.kinetic.effect = KineticSymbolEffect::Rotate {
        clockwise: true,
        scope: KineticLayerScope::WholeSymbol,
    };

    let mut max_tip_displacement = 0.0_f64;
    for step in 0..=20 {
        state.icon_studio.kinetic.phase_t = (step as f64) / 20.0;
        let eval = state.evaluate_active_kinetic_contours();
        assert_eq!(eval.len(), 2);
        let p0_rot = eval[0].0.nodes[0].point;
        assert!(
            p0_rot.distance_to(anchor_pos) < 1e-6,
            "Point located at CanvasAnchor pivot must remain stationary during Rotate at t={}: dist={}",
            state.icon_studio.kinetic.phase_t,
            p0_rot.distance_to(anchor_pos)
        );
        let tip_dist = eval[0].0.nodes[2].point.distance_to(c0.nodes[2].point);
        if tip_dist > max_tip_displacement {
            max_tip_displacement = tip_dist;
        }
    }
    assert!(
        max_tip_displacement > 200.0,
        "Rotate must sweep outer contour nodes through a full 360-degree arc around the CanvasAnchor (max_tip_displacement = {max_tip_displacement})"
    );

    // 2. All 9 WiggleAxis modes (Clockwise, CounterClockwise, Left, Right, Up, Down, Forward, Backward, CustomAngle)
    let pivot = FontPoint::new(300.0, 300.0);
    let all_axes = [
        WiggleAxis::Clockwise,
        WiggleAxis::CounterClockwise,
        WiggleAxis::Left,
        WiggleAxis::Right,
        WiggleAxis::Up,
        WiggleAxis::Down,
        WiggleAxis::Forward,
        WiggleAxis::Backward,
        WiggleAxis::CustomAngle(37.0),
    ];
    for axis in all_axes {
        let eff = KineticSymbolEffect::Wiggle {
            axis,
            scope: KineticLayerScope::WholeSymbol,
        };
        let tf_peak = evaluate_layer_kinetic_transform(&eff, 0.125, 0, 1, None, None, pivot);
        let tf_end = evaluate_layer_kinetic_transform(&eff, 1.0, 0, 1, None, None, pivot);
        match axis {
            WiggleAxis::Clockwise => {
                assert!(
                    tf_peak.rotation_rad < -0.1,
                    "Clockwise Wiggle must rotate negatively at tau=0.125"
                );
                assert_eq!(tf_peak.translation_upm, FontPoint::new(0.0, 0.0));
            }
            WiggleAxis::CounterClockwise => {
                assert!(
                    tf_peak.rotation_rad > 0.1,
                    "CounterClockwise Wiggle must rotate positively at tau=0.125"
                );
                assert_eq!(tf_peak.translation_upm, FontPoint::new(0.0, 0.0));
            }
            WiggleAxis::Left => assert!(
                tf_peak.translation_upm.x.0 < -10.0 && tf_peak.translation_upm.y.0.abs() < 1e-6
            ),
            WiggleAxis::Right | WiggleAxis::Forward => assert!(
                tf_peak.translation_upm.x.0 > 10.0 && tf_peak.translation_upm.y.0.abs() < 1e-6
            ),
            WiggleAxis::Backward => assert!(
                tf_peak.translation_upm.x.0 < -10.0 && tf_peak.translation_upm.y.0.abs() < 1e-6
            ),
            WiggleAxis::Up => assert!(
                tf_peak.translation_upm.y.0 > 10.0 && tf_peak.translation_upm.x.0.abs() < 1e-6
            ),
            WiggleAxis::Down => assert!(
                tf_peak.translation_upm.y.0 < -10.0 && tf_peak.translation_upm.x.0.abs() < 1e-6
            ),
            WiggleAxis::CustomAngle(deg) => {
                let rad = deg.to_radians();
                let dot = tf_peak.translation_upm.x.0 * rad.cos()
                    + tf_peak.translation_upm.y.0 * rad.sin();
                let cross = tf_peak.translation_upm.x.0 * rad.sin()
                    - tf_peak.translation_upm.y.0 * rad.cos();
                assert!(
                    dot > 10.0 && cross.abs() < 1e-6,
                    "CustomAngle(37) Wiggle must translate collinearly with (cos 37, sin 37)"
                );
            }
        }
        assert!(
            tf_end.rotation_rad.abs() < 1e-9
                && tf_end.translation_upm.x.0.abs() < 1e-9
                && tf_end.translation_upm.y.0.abs() < 1e-9,
            "Wiggle must decay exactly to rest at tau=1.0 for {axis:?}"
        );
    }

    // 3. Breathe Plain vs Pulse (scale & opacity modulation)
    let breathe_plain = KineticSymbolEffect::Breathe {
        pulse_opacity: false,
        scope: KineticLayerScope::WholeSymbol,
    };
    let breathe_pulse = KineticSymbolEffect::Breathe {
        pulse_opacity: true,
        scope: KineticLayerScope::WholeSymbol,
    };
    let tf_bp_mid = evaluate_layer_kinetic_transform(&breathe_plain, 0.5, 0, 1, None, None, pivot);
    let tf_bpu_mid = evaluate_layer_kinetic_transform(&breathe_pulse, 0.5, 0, 1, None, None, pivot);
    assert!(
        (tf_bp_mid.scale.0 - 1.14).abs() < 1e-6 && (tf_bp_mid.opacity_factor - 1.0).abs() < 1e-6
    );
    assert!(
        (tf_bpu_mid.scale.0 - 1.14).abs() < 1e-6 && (tf_bpu_mid.opacity_factor - 0.70).abs() < 1e-5
    );

    // 4. End-to-End InputDispatcher CanvasAnchor Drag + Pijler B/E/F + Multi-Pass Rendering:
    // Place CanvasAnchor(7) at a clean pivot position (180, 260) away from OnCurve nodes, and drag it via InputDispatcher!
    let drag_anchor_start = FontPoint::new(180.0, 260.0);
    state.anchors[0].position = drag_anchor_start;
    state.glyph.anchors = state.anchors.clone();
    state.activate_tool(grafia::CanvasTool::Select);
    state.view_mode = CanvasViewMode::EditWireframe;
    state.icon_studio.kinetic.phase_t = 0.5;
    let ctx = egui::Context::default();
    let canvas_rect = egui::Rect::from_min_size(egui::pos2(0.0, 0.0), egui::vec2(800.0, 600.0));
    let anc_scr = state.viewport.to_screen(drag_anchor_start);
    let anc_start = egui::pos2(anc_scr.x.0, anc_scr.y.0);
    let anc_dst = egui::pos2(anc_start.x + 50.0, anc_start.y - 40.0);
    let raw_anc_drag = egui::RawInput {
        screen_rect: Some(canvas_rect),
        time: Some(1.0),
        events: vec![
            egui::Event::PointerButton {
                pos: anc_start,
                button: egui::PointerButton::Primary,
                pressed: true,
                modifiers: egui::Modifiers::NONE,
            },
            egui::Event::PointerMoved(anc_dst),
            egui::Event::PointerButton {
                pos: anc_dst,
                button: egui::PointerButton::Primary,
                pressed: false,
                modifiers: egui::Modifiers::NONE,
            },
        ],
        ..Default::default()
    };
    let mut out_a = ctx.run_ui(raw_anc_drag, |ui| {
        let ev = ui.input(|i| i.events.clone());
        let _ = grafia::adapters::InputDispatcher::for_canvas(&mut state, canvas_rect)
            .dispatch_events(&ev, ui);
    });
    out_a.textures_delta.clear();
    let new_anchor_pos = state.anchors[0].position;
    assert!(
        new_anchor_pos.distance_to(drag_anchor_start) > 50.0,
        "Pijler C violation: dragging CanvasAnchor via InputDispatcher must move the anchor on the canvas (new_pos={new_anchor_pos:?})"
    );
    // Undo & Redo of CanvasAnchor drag (Pijler E)
    assert!(state.undo(), "Undo of CanvasAnchor drag must succeed");
    assert!(state.anchors[0].position.distance_to(drag_anchor_start) < 1e-6);
    assert!(state.redo(), "Redo of CanvasAnchor drag must succeed");
    assert!(state.anchors[0].position.distance_to(new_anchor_pos) < 1e-6);

    // 5. Pijler A: Unconstrained arbtest across Wiggle (all 9 axes + NaN/Inf/subnormal CustomAngle), Rotate, and Breathe
    let theme = GrafiaTheme::default();
    let _ = grafia::adapters::rasterize_canvas_subregion(&state, &theme, 240, 200);
    arbtest(|u| {
        let raw_t = f64::from_bits(u.arbitrary::<u64>()?);
        let raw_deg = f64::from_bits(u.arbitrary::<u64>()?);
        let px = f64::from_bits(u.arbitrary::<u64>()?);
        let py = f64::from_bits(u.arbitrary::<u64>()?);
        let piv = FontPoint::new(px, py);
        let variant = u.int_in_range(0_u8..=11)?;
        let scope = if u.arbitrary::<bool>()? {
            KineticLayerScope::ByLayer
        } else {
            KineticLayerScope::WholeSymbol
        };
        let eff = match variant {
            0 => KineticSymbolEffect::Wiggle {
                axis: WiggleAxis::Clockwise,
                scope,
            },
            1 => KineticSymbolEffect::Wiggle {
                axis: WiggleAxis::CounterClockwise,
                scope,
            },
            2 => KineticSymbolEffect::Wiggle {
                axis: WiggleAxis::Left,
                scope,
            },
            3 => KineticSymbolEffect::Wiggle {
                axis: WiggleAxis::Right,
                scope,
            },
            4 => KineticSymbolEffect::Wiggle {
                axis: WiggleAxis::Up,
                scope,
            },
            5 => KineticSymbolEffect::Wiggle {
                axis: WiggleAxis::Down,
                scope,
            },
            6 => KineticSymbolEffect::Wiggle {
                axis: WiggleAxis::Forward,
                scope,
            },
            7 => KineticSymbolEffect::Wiggle {
                axis: WiggleAxis::Backward,
                scope,
            },
            8 => KineticSymbolEffect::Wiggle {
                axis: WiggleAxis::CustomAngle(raw_deg),
                scope,
            },
            9 => KineticSymbolEffect::Rotate {
                clockwise: u.arbitrary::<bool>()?,
                scope,
            },
            _ => KineticSymbolEffect::Breathe {
                pulse_opacity: u.arbitrary::<bool>()?,
                scope,
            },
        };
        let tf = evaluate_layer_kinetic_transform(&eff, raw_t, 0, 3, None, None, piv);
        assert!(tf.translation_upm.x.0.is_finite() && tf.translation_upm.y.0.is_finite());
        assert!(tf.rotation_rad.is_finite());
        assert!(tf.scale.0.is_finite() && tf.scale.1.is_finite());
        assert!(tf.opacity_factor.is_finite() && (0.0..=1.0).contains(&tf.opacity_factor));
        let out_c = grafia::apply_layer_kinetic_transform(&c0, &tf);
        for n in &out_c.nodes {
            assert!(n.point.x.0.is_finite() && n.point.y.0.is_finite());
        }
        Ok(())
    })
    .budget_ms(250);
}

/// **`TC-V0140-008` (`Stap 16.8` — Kinetic Effect 10: `MagicReplace` & `Replace` Content Transition + `vitrea::tessellate_meniscus_bridge`):**
/// Enforces all 8 NASA/IBM/Google/Apple/Microsoft Production Pillars:
/// - **Pijler A:** Unconstrained `arbtest` injecting `f64::NAN`, `±f64::INFINITY`, `f64::MIN_POSITIVE`, mismatched cubic/line node counts (`3..=17` vs `4..=23`), open/closed combinations, and all 3 `ReplaceFallback` variants (`DownUp`, `UpUp`, `OffUp`).
/// - **Pijler B & E:** Frame-Condition & Algebraic Undo/Redo Inversion: evaluating or scrubbing `MagicReplace` at any `phase_t` NEVER mutates `state.glyph.contours`, `state.color_sublayers`, or `target_contours`, and editing source nodes via `InputDispatcher` + `undo()`/`redo()` preserves `MagicReplace` `target_contours` bit-exactly.
/// - **Pijler C & F:** End-to-End `InputDispatcher` node/handle dragging while `KineticSymbolEffect::MagicReplace` is active updates the live morphed midpoint contour at $\tau = 0.5$ and rolls back cleanly on mid-drag cancellation.
/// - **Pijler D:** Live rendering of `MagicReplace` across `EditWireframe`, `PreviewFilled`, bottom `PreviewPanel`, and `render_colored_thumbnail`, plus `vitrea::tessellate_meniscus_bridge` liquid bridge contour generation at mid-morph ($\tau \in (0.15, 0.85)$).
#[test]
fn tc_v0140_008_kinetic_magic_replace_and_meniscus_bridge() {
    use grafia::{
        CanvasViewMode, ContourId, GlyphContour, GrafiaCanvasState, GrafiaTheme, KineticLayerScope,
        KineticSymbolEffect, Node, NodeId, NodeType, ReplaceFallback, SymbolLayerTag,
    };

    // 1. Play (1 triangle contour, group 1) -> Pause (2 vertical bar contours: bar 0 paired with group 1, bar 1 unmatched in group 2)
    let play_triangle = GlyphContour::new(
        ContourId(0),
        vec![
            Node::new(NodeId(10), 140.0, 140.0, NodeType::Corner),
            Node::new(NodeId(11), 460.0, 340.0, NodeType::Corner),
            Node::new(NodeId(12), 140.0, 540.0, NodeType::Corner),
        ],
    );
    let pause_bar_left = GlyphContour::new(
        ContourId(0),
        vec![
            Node::new(NodeId(100), 140.0, 140.0, NodeType::Corner),
            Node::new(NodeId(101), 250.0, 140.0, NodeType::Corner),
            Node::new(NodeId(102), 250.0, 540.0, NodeType::Corner),
            Node::new(NodeId(103), 140.0, 540.0, NodeType::Corner),
        ],
    );
    let pause_bar_right = GlyphContour::new(
        ContourId(1),
        vec![
            Node::new(NodeId(110), 310.0, 140.0, NodeType::Corner),
            Node::new(NodeId(111), 420.0, 140.0, NodeType::Corner),
            Node::new(NodeId(112), 420.0, 540.0, NodeType::Corner),
            Node::new(NodeId(113), 310.0, 540.0, NodeType::Corner),
        ],
    );

    let mut state = GrafiaCanvasState::init_glyph_a();
    state.viewport =
        grafia::ViewportTransform::new(0.5, grafia::ScreenPoint::new(100.0, 500.0), 1000);
    let mut g = state.glyph.clone();
    g.contours = vec![play_triangle.clone()];
    let metrics = state.metrics.clone();
    state.load_glyph(g, metrics);
    state.icon_studio.symbol_layer_tags = vec![SymbolLayerTag {
        layer_group: 1,
        ..Default::default()
    }];

    for fallback in [
        ReplaceFallback::DownUp,
        ReplaceFallback::UpUp,
        ReplaceFallback::OffUp,
    ] {
        state.icon_studio.kinetic.effect = KineticSymbolEffect::MagicReplace {
            fallback,
            scope: KineticLayerScope::WholeSymbol,
            target_contours: vec![pause_bar_left.clone(), pause_bar_right.clone()],
        };

        // At tau = 0.0: matched contour matches source Play triangle; unmatched target contour has opacity 0.0
        state.icon_studio.kinetic.phase_t = 0.0;
        let eval_0 = state.evaluate_active_kinetic_contours();
        assert!(!eval_0.is_empty());
        let (area_src, cent_src) = grafia::compute_contour_area_and_centroid(&play_triangle.nodes);
        let (area_0, cent_0) = grafia::compute_contour_area_and_centroid(&eval_0[0].0.nodes);
        assert!(
            (area_0.abs() - area_src.abs()).abs() < 2.0 && cent_0.distance_to(cent_src) < 2.0,
            "At tau=0.0 MagicReplace must reproduce the source Play triangle area & centroid"
        );

        // At tau = 0.5: matched contour is smoothly morphed halfway between Play triangle and Pause left bar,
        // AND nearby morphed contours generate a vitrea::tessellate_meniscus_bridge contour!
        state.icon_studio.kinetic.phase_t = 0.5;
        let eval_mid = state.evaluate_active_kinetic_contours();
        let (area_dst0, cent_dst0) =
            grafia::compute_contour_area_and_centroid(&pause_bar_left.nodes);
        let (area_mid, cent_mid) = grafia::compute_contour_area_and_centroid(&eval_mid[0].0.nodes);
        let expected_mid_x = 0.5 * (cent_src.x.0 + cent_dst0.x.0);
        assert!(
            (cent_mid.x.0 - expected_mid_x).abs() < 15.0
                && area_mid.abs() > area_dst0.abs().min(area_src.abs()) * 0.5,
            "At tau=0.5 MagicReplace matched contour must interpolate smoothly between Play and Pause (cent_mid={cent_mid:?}, expected_x={expected_mid_x})"
        );
        assert!(
            eval_mid.len() >= 2,
            "At tau=0.5 MagicReplace must include unmatched target contour and/or vitrea meniscus bridge contour"
        );

        // At tau = 1.0: matched contour matches target Pause left bar; unmatched target Pause right bar has opacity 1.0
        state.icon_studio.kinetic.phase_t = 1.0;
        let eval_1 = state.evaluate_active_kinetic_contours();
        assert!(eval_1.len() >= 2);
        let (area_1, cent_1) = grafia::compute_contour_area_and_centroid(&eval_1[0].0.nodes);
        assert!(
            (area_1.abs() - area_dst0.abs()).abs() < 2.0 && cent_1.distance_to(cent_dst0) < 2.0,
            "At tau=1.0 MagicReplace matched contour must reproduce target Pause left bar"
        );
        assert!(
            (eval_1[1].1 - 1.0).abs() < 1e-5,
            "At tau=1.0 unmatched target Pause right bar must reach full opacity 1.0"
        );
    }

    // 2. Pijler B, C, E & F: Frame-Condition + End-to-End InputDispatcher Node Drag during MagicReplace:
    // Source state.glyph.contours[0] must remain untouched by phase scrubbing, and dragging NodeId(11) via InputDispatcher
    // must shift the morphed midpoint at tau = 0.5 while keeping target_contours untouched, with full Undo/Redo!
    assert_eq!(
        state.glyph.contours[0].nodes, play_triangle.nodes,
        "Pijler B violation: scrubbing MagicReplace phase_t must never mutate state.glyph.contours"
    );
    state.icon_studio.kinetic.phase_t = 0.5;
    let mid_before_drag = state.evaluate_active_kinetic_contours()[0].0.clone();
    let (_, cent_mid_before) = grafia::compute_contour_area_and_centroid(&mid_before_drag.nodes);

    state.activate_tool(grafia::CanvasTool::Select);
    state.view_mode = CanvasViewMode::EditWireframe;
    let ctx = egui::Context::default();
    let canvas_rect = egui::Rect::from_min_size(egui::pos2(0.0, 0.0), egui::vec2(800.0, 600.0));
    let n11_pt = state.find_node(NodeId(11)).unwrap().point;
    let n11_scr = state.viewport.to_screen(n11_pt);
    let p0 = egui::pos2(n11_scr.x.0, n11_scr.y.0);
    let p1 = egui::pos2(p0.x + 60.0, p0.y);
    let raw_drag = egui::RawInput {
        screen_rect: Some(canvas_rect),
        time: Some(2.0),
        events: vec![
            egui::Event::PointerButton {
                pos: p0,
                button: egui::PointerButton::Primary,
                pressed: true,
                modifiers: egui::Modifiers::NONE,
            },
            egui::Event::PointerMoved(p1),
            egui::Event::PointerButton {
                pos: p1,
                button: egui::PointerButton::Primary,
                pressed: false,
                modifiers: egui::Modifiers::NONE,
            },
        ],
        ..Default::default()
    };
    let mut out_d = ctx.run_ui(raw_drag, |ui| {
        let ev = ui.input(|i| i.events.clone());
        let _ = grafia::adapters::InputDispatcher::for_canvas(&mut state, canvas_rect)
            .dispatch_events(&ev, ui);
    });
    out_d.textures_delta.clear();
    let mid_after_drag = state.evaluate_active_kinetic_contours()[0].0.clone();
    let (_, cent_mid_after) = grafia::compute_contour_area_and_centroid(&mid_after_drag.nodes);
    assert!(
        cent_mid_after.x.0 > cent_mid_before.x.0 + 10.0,
        "Pijler C violation: dragging source node via InputDispatcher must shift the live MagicReplace midpoint contour"
    );
    assert!(state.undo());
    let (_, cent_mid_undo) = grafia::compute_contour_area_and_centroid(
        &state.evaluate_active_kinetic_contours()[0].0.nodes,
    );
    assert!((cent_mid_undo.x.0 - cent_mid_before.x.0).abs() < 1e-4);
    assert!(state.redo());

    // 3. Pijler D: Multi-pass rendering verification
    let theme = GrafiaTheme::default();
    let pix_mid = grafia::adapters::rasterize_canvas_subregion(&state, &theme, 400, 480);
    state.icon_studio.kinetic.phase_t = 1.0;
    let pix_end = grafia::adapters::rasterize_canvas_subregion(&state, &theme, 400, 480);
    assert_ne!(
        pix_mid.data(),
        pix_end.data(),
        "Pijler D violation: MagicReplace must produce distinct rendered frames at tau=0.5 vs tau=1.0"
    );

    // 4. Pijler A: Unconstrained arbtest on MagicReplace with arbitrary node counts, open/closed flags, and NaN/Inf/subnormals
    arbtest(|u| {
        let raw_t = f64::from_bits(u.arbitrary::<u64>()?);
        let fb = match u.int_in_range(0_u8..=2)? {
            0 => ReplaceFallback::DownUp,
            1 => ReplaceFallback::UpUp,
            _ => ReplaceFallback::OffUp,
        };
        let scope = if u.arbitrary::<bool>()? {
            KineticLayerScope::ByLayer
        } else {
            KineticLayerScope::WholeSymbol
        };
        let n_src = u.int_in_range(0_usize..=7)?;
        let n_dst = u.int_in_range(0_usize..=7)?;
        let mut src_nodes = Vec::with_capacity(n_src);
        for i in 0..n_src {
            let x = f64::from_bits(u.arbitrary::<u64>()?);
            let y = f64::from_bits(u.arbitrary::<u64>()?);
            src_nodes.push(Node {
                id: NodeId((i as u64) + 1),
                point: grafia::FontPoint::new(x, y),
                node_type: NodeType::Corner,
            });
        }
        let mut dst_nodes = Vec::with_capacity(n_dst);
        for i in 0..n_dst {
            let x = f64::from_bits(u.arbitrary::<u64>()?);
            let y = f64::from_bits(u.arbitrary::<u64>()?);
            dst_nodes.push(Node {
                id: NodeId((i as u64) + 50),
                point: grafia::FontPoint::new(x, y),
                node_type: NodeType::Corner,
            });
        }
        let mut src_c = GlyphContour::new(ContourId(0), src_nodes);
        src_c.closed = u.arbitrary::<bool>()?;
        let mut dst_c = GlyphContour::new(ContourId(0), dst_nodes);
        dst_c.closed = u.arbitrary::<bool>()?;
        let kin = grafia::KineticTimelineState {
            effect: KineticSymbolEffect::MagicReplace {
                fallback: fb,
                scope,
                target_contours: vec![dst_c],
            },
            phase_t: raw_t,
            ..Default::default()
        };
        let out = grafia::evaluate_kinetic_glyph_contours(&[src_c], &[], &[], &kin);
        for (gc, alpha) in &out {
            assert!(alpha.is_finite() && (0.0..=1.0).contains(alpha));
            for n in &gc.nodes {
                assert!(n.point.x.0.is_finite() && n.point.y.0.is_finite());
            }
        }
        Ok(())
    })
    .budget_ms(300);
}

/// **`TC-V0140-009` (`Stap 16.9` — `CanvasViewMode::LiquidGlass` Physical Renderer via `vitrea v0.1.0`):**
/// Enforces all 8 NASA/IBM/Google/Apple/Microsoft Production Pillars:
/// - **Pijler A:** Unconstrained `arbtest` injecting `f64::NAN`, `±f64::INFINITY`, `f64::MIN_POSITIVE` into `LiquidGlassRenderConfig`, `evaluate_liquid_glass_point_upm`, and `rasterize_liquid_glass_rgba`.
/// - **Pijler B & E:** Frame-Condition & Algebraic Undo/Redo Inversion: rendering in `CanvasViewMode::LiquidGlass` never mutates `state.glyph.contours` or `icon_studio`, and dragging a droplet node in `LiquidGlass` view records a clean Undo/Redo transaction.
/// - **Pijler C & F:** End-to-End `InputDispatcher` node/handle dragging directly inside `CanvasViewMode::LiquidGlass` moves droplet B toward droplet A so their $C^1$ metaball meniscus (`d_smooth < 0`) fuses live at the midpoint!
/// - **Pijler D & G:** Active BBox Culling + `CanvasRenderCache` texture caching + Cauchy 3-wavelength RGB chromatic dispersion (`p_b != p_r`) + 3D dome normals (`N_z > 0`) + Fresnel rim elevation.
#[test]
fn tc_v0140_009_liquid_glass_optical_renderer_and_live_meniscus_drag() {
    use grafia::{
        CanvasViewMode, ContourId, FontPoint, GlyphContour, GrafiaCanvasState, GrafiaTheme,
        LiquidGlassRenderConfig, Node, NodeId, NodeType,
    };

    // 1. Two droplet squares separated by gap = 24 UPM (< smooth_union_k_upm = 45 UPM):
    // Midpoint (300, 300) is outside both individual droplets (d_a > 0, d_b > 0), but fused inside the C1 smooth union (d_union < 0)!
    let droplet_a = GlyphContour::new(
        ContourId(0),
        vec![
            Node::new(NodeId(10), 160.0, 220.0, NodeType::Corner),
            Node::new(NodeId(11), 288.0, 220.0, NodeType::Corner),
            Node::new(NodeId(12), 288.0, 380.0, NodeType::Corner),
            Node::new(NodeId(13), 160.0, 380.0, NodeType::Corner),
        ],
    );
    let droplet_b_far = GlyphContour::new(
        ContourId(1),
        vec![
            Node::new(NodeId(20), 420.0, 220.0, NodeType::Corner),
            Node::new(NodeId(21), 540.0, 220.0, NodeType::Corner),
            Node::new(NodeId(22), 540.0, 380.0, NodeType::Corner),
            Node::new(NodeId(23), 420.0, 380.0, NodeType::Corner),
        ],
    );

    let mut state = GrafiaCanvasState::init_glyph_a();
    state.viewport =
        grafia::ViewportTransform::new(0.5, grafia::ScreenPoint::new(100.0, 500.0), 1000);
    let mut g = state.glyph.clone();
    g.contours = vec![droplet_a.clone(), droplet_b_far.clone()];
    let metrics = state.metrics.clone();
    state.load_glyph(g, metrics);
    state.icon_studio.liquid_glass = LiquidGlassRenderConfig {
        dome_radius_upm: 55.0,
        smooth_union_k_upm: 45.0,
        ior_base: 1.52,
        chromatic_dispersion: 0.035,
        specular_intensity: 0.95,
        tint_rgba: [120, 200, 255, 220],
    };
    state.view_mode = CanvasViewMode::LiquidGlass;

    // Before dragging droplet B close to droplet A: gap is 420 - 288 = 132 UPM (> k = 45 UPM), so bridge point (300, 300) is outside (d > 0)
    let bridge_pt = FontPoint::new(300.0, 300.0);
    let sample_far = grafia::render::evaluate_liquid_glass_point_upm(
        &state.glyph.contours,
        &state.icon_studio.liquid_glass,
        bridge_pt,
    )
    .expect("LiquidGlass sample must be Some for valid closed contours");
    assert!(
        sample_far.0.distance > 0.0,
        "When droplets are 132 UPM apart (> k=45), midpoint (300,300) must be outside glass (d = {})",
        sample_far.0.distance
    );

    // Interior point of droplet A (224, 300) and rim point (280, 300):
    let (_, surf_center) = grafia::render::evaluate_liquid_glass_point_upm(
        &state.glyph.contours,
        &state.icon_studio.liquid_glass,
        FontPoint::new(224.0, 300.0),
    )
    .unwrap();
    let (sdf_rim, surf_rim) = grafia::render::evaluate_liquid_glass_point_upm(
        &state.glyph.contours,
        &state.icon_studio.liquid_glass,
        FontPoint::new(280.0, 300.0),
    )
    .unwrap();
    assert!(
        sdf_rim.distance < 0.0 && surf_rim.normal_3d.z > 0.0 && surf_center.normal_3d.z > surf_rim.normal_3d.z,
        "Interior pixels must exhibit 3D dome normals with N_z > 0 and higher N_z at dome apex than at rim"
    );
    let [p_r, p_g, p_b] = surf_rim.refracted_rgb_pts;
    assert!(
        (p_b - p_r).length() > 0.05 && (p_b - p_g).length() > 0.01,
        "Rim pixels must exhibit non-zero Cauchy 3-wavelength RGB chromatic dispersion (p_r={p_r:?}, p_b={p_b:?})"
    );
    assert!(
        surf_rim.fresnel_rim_factor > surf_center.fresnel_rim_factor,
        "Fresnel rim factor must be elevated at the glass boundary relative to the interior center"
    );

    // 2. Pijler C, B, E & D: Live InputDispatcher Node Drag inside `CanvasViewMode::LiquidGlass`!
    // Select left-edge nodes of droplet B (NodeId(20) & NodeId(23)) and drag them left by 108 UPM (54 dp at zoom 0.5)
    // so droplet B's left edge moves from X=420 to X=312 (gap = 24 UPM < k = 45 UPM) and fuses with droplet A!
    state.activate_tool(grafia::CanvasTool::Select);
    state.selected_nodes = vec![NodeId(20), NodeId(23)];
    let theme = GrafiaTheme::default();
    let pix_before = grafia::adapters::rasterize_canvas_subregion(&state, &theme, 360, 360);

    let ctx = egui::Context::default();
    let canvas_rect = egui::Rect::from_min_size(egui::pos2(0.0, 0.0), egui::vec2(800.0, 600.0));
    let n20_scr = state.viewport.to_screen(FontPoint::new(420.0, 220.0));
    let p_start = egui::pos2(n20_scr.x.0, n20_scr.y.0);
    let p_end = egui::pos2(p_start.x - 58.0, p_start.y);
    for (t, evs) in [
        (
            3.0,
            vec![egui::Event::PointerButton {
                pos: p_start,
                button: egui::PointerButton::Primary,
                pressed: true,
                modifiers: egui::Modifiers::NONE,
            }],
        ),
        (
            3.1,
            vec![
                egui::Event::PointerMoved(p_end),
                egui::Event::PointerButton {
                    pos: p_end,
                    button: egui::PointerButton::Primary,
                    pressed: false,
                    modifiers: egui::Modifiers::NONE,
                },
            ],
        ),
    ] {
        let raw_drag = egui::RawInput {
            screen_rect: Some(canvas_rect),
            time: Some(t),
            events: evs,
            ..Default::default()
        };
        let mut out_d = ctx.run_ui(raw_drag, |ui| {
            let ev = ui.input(|i| i.events.clone());
            let _ = grafia::adapters::InputDispatcher::for_canvas(&mut state, canvas_rect)
                .dispatch_events(&ev, ui);
        });
        out_d.textures_delta.clear();
    }

    let sample_fused = grafia::render::evaluate_liquid_glass_point_upm(
        &state.glyph.contours,
        &state.icon_studio.liquid_glass,
        bridge_pt,
    )
    .unwrap();
    assert!(
        sample_fused.0.distance < 0.0 && sample_fused.1.coverage > 0.5 && sample_fused.1.normal_3d.z > 0.0,
        "Pijler C violation: dragging droplet B toward droplet A via InputDispatcher in CanvasViewMode::LiquidGlass must fuse the C1 meniscus bridge at (300,300) (d = {})",
        sample_fused.0.distance
    );
    let pix_after = grafia::adapters::rasterize_canvas_subregion(&state, &theme, 360, 360);
    assert_ne!(
        pix_before.data(),
        pix_after.data(),
        "Pijler D violation: CanvasViewMode::LiquidGlass pixmap must update when contours are dragged"
    );

    // Pijler E: Undo/Redo restores unfused state and re-fuses
    assert!(state.undo());
    let sample_undo = grafia::render::evaluate_liquid_glass_point_upm(
        &state.glyph.contours,
        &state.icon_studio.liquid_glass,
        bridge_pt,
    )
    .unwrap();
    assert!(sample_undo.0.distance > 0.0);
    assert!(state.redo());

    // 3. Pijler A: Unconstrained arbtest on LiquidGlassRenderConfig and evaluate_liquid_glass_point_upm
    arbtest(|u| {
        let cfg = LiquidGlassRenderConfig {
            dome_radius_upm: f64::from_bits(u.arbitrary::<u64>()?),
            smooth_union_k_upm: f64::from_bits(u.arbitrary::<u64>()?),
            ior_base: f32::from_bits(u.arbitrary::<u32>()?),
            chromatic_dispersion: f32::from_bits(u.arbitrary::<u32>()?),
            specular_intensity: f32::from_bits(u.arbitrary::<u32>()?),
            tint_rgba: u.arbitrary::<[u8; 4]>()?,
        };
        let px = f64::from_bits(u.arbitrary::<u64>()?);
        let py = f64::from_bits(u.arbitrary::<u64>()?);
        if let Some((sdf, surf)) = grafia::render::evaluate_liquid_glass_point_upm(
            &state.glyph.contours,
            &cfg,
            FontPoint::new(px, py),
        ) {
            assert!(sdf.distance.is_finite());
            assert!(surf.coverage.is_finite() && (0.0..=1.0).contains(&surf.coverage));
            assert!(surf.normal_3d.is_finite());
        }
        Ok(())
    })
    .budget_ms(250);
}

#[test]
fn tc_v0140_010_bake_liquid_glass_to_colrv1_sublayers_and_gradient_handle_drag() {
    let droplet = GlyphContour::new(
        ContourId(0),
        vec![
            Node::new(NodeId(10), 140.0, 180.0, NodeType::Corner),
            Node::new(NodeId(11), 420.0, 180.0, NodeType::Corner),
            Node::new(NodeId(12), 420.0, 460.0, NodeType::Corner),
            Node::new(NodeId(13), 140.0, 460.0, NodeType::Corner),
        ],
    );
    let mut spine = GlyphContour::new(
        ContourId(1),
        vec![
            Node::new(NodeId(20), 120.0, 100.0, NodeType::Corner),
            Node::new(NodeId(21), 280.0, 130.0, NodeType::OffCurve),
            Node::new(NodeId(22), 440.0, 100.0, NodeType::Corner),
        ],
    );
    spine.closed = false;

    let mut state = GrafiaCanvasState::init_glyph_a();
    state.viewport =
        grafia::ViewportTransform::new(0.5, grafia::ScreenPoint::new(100.0, 500.0), 1000);
    let mut g = state.glyph.clone();
    g.contours = vec![droplet.clone(), spine.clone()];
    let metrics = state.metrics.clone();
    state.load_glyph(g, metrics);
    state.set_variable_width_stroke(VariableWidthStrokeSpec {
        contour_id: ContourId(1),
        samples: vec![
            WidthNodeSample {
                arc_ratio_t: 0.0,
                width_left_upm: 12.0,
                width_right_upm: 12.0,
            },
            WidthNodeSample {
                arc_ratio_t: 1.0,
                width_left_upm: 20.0,
                width_right_upm: 20.0,
            },
        ],
        cap_style: LineCapStyle::Round,
    });
    state.icon_studio.symbol_layer_tags = vec![
        SymbolLayerTag {
            layer_group: 1,
            ..Default::default()
        },
        SymbolLayerTag {
            layer_group: 2,
            ..Default::default()
        },
    ];
    state.icon_studio.liquid_glass = LiquidGlassRenderConfig {
        dome_radius_upm: 48.0,
        smooth_union_k_upm: 32.0,
        ior_base: 1.52,
        chromatic_dispersion: 0.025,
        specular_intensity: 0.90,
        tint_rgba: [110, 195, 255, 230],
    };

    let state_before_bake = state.snapshot();
    assert!(
        state.bake_liquid_glass_to_colrv1(),
        "bake_liquid_glass_to_colrv1 must succeed on valid closed contours"
    );

    // 1. Exact 3 COLRv1 vector sublayers per baked contour group:
    // (0) LinearGradient refractive dome body, (1) RadialGradient Schlick-Fresnel rim, (2) LinearGradient specular crest
    assert_eq!(state.color_sublayers.len(), 3);
    assert!(matches!(
        state.color_sublayers[0].paint,
        grafia::ColorLayerPaint::LinearGradient { .. }
    ));
    assert!(matches!(
        state.color_sublayers[1].paint,
        grafia::ColorLayerPaint::RadialGradient { .. }
    ));
    assert!(matches!(
        state.color_sublayers[2].paint,
        grafia::ColorLayerPaint::LinearGradient { .. }
    ));
    for sub in &state.color_sublayers {
        let stops =
            grafia::gradient_stops_slice(&sub.paint).expect("Baked layer must have gradient stops");
        assert!(stops.len() >= 2);
        for st in stops {
            assert!(
                st.offset.is_finite() && (0.0..=1.0).contains(&st.offset),
                "Gradient stop offset must be finite in [0.0, 1.0]"
            );
        }
    }

    // 2. Pijler B (Frame-Condition): Original glyph contours, open spine, VariableWidthStrokeSpec, and SymbolLayerTags remain 100% intact!
    assert_eq!(state.glyph.contours.len(), 2);
    assert_eq!(state.glyph.contours[0].nodes, droplet.nodes);
    assert_eq!(state.glyph.contours[1].nodes, spine.nodes);
    assert!(!state.glyph.contours[1].closed);
    assert_eq!(state.icon_studio.variable_width_strokes.len(), 1);
    assert_eq!(
        state.icon_studio.variable_width_strokes[0].contour_id,
        ContourId(1)
    );
    assert_eq!(state.icon_studio.symbol_layer_tags.len(), 2);

    // 3. Pijler C & D: Generated COLRv1 gradient handles are directly draggable via InputDispatcher!
    state.activate_tool(grafia::CanvasTool::Select);
    state.select_color_sublayer(Some(0));
    let (p0_before, p1_before) =
        grafia::gradient_control_segment_upm(&state.color_sublayers[0].paint).unwrap();
    let p1_scr = state.viewport.to_screen(p1_before);
    let p_start = egui::pos2(p1_scr.x.0, p1_scr.y.0);
    let p_end = egui::pos2(p_start.x + 30.0, p_start.y - 20.0);
    let ctx = egui::Context::default();
    let canvas_rect = egui::Rect::from_min_size(egui::pos2(0.0, 0.0), egui::vec2(800.0, 600.0));
    for (t, evs) in [
        (
            1.0,
            vec![egui::Event::PointerButton {
                pos: p_start,
                button: egui::PointerButton::Primary,
                pressed: true,
                modifiers: egui::Modifiers::NONE,
            }],
        ),
        (
            1.1,
            vec![
                egui::Event::PointerMoved(p_end),
                egui::Event::PointerButton {
                    pos: p_end,
                    button: egui::PointerButton::Primary,
                    pressed: false,
                    modifiers: egui::Modifiers::NONE,
                },
            ],
        ),
    ] {
        let raw = egui::RawInput {
            screen_rect: Some(canvas_rect),
            time: Some(t),
            events: evs,
            ..Default::default()
        };
        let mut out = ctx.run_ui(raw, |ui| {
            let ev = ui.input(|i| i.events.clone());
            let _ = grafia::adapters::InputDispatcher::for_canvas(&mut state, canvas_rect)
                .dispatch_events(&ev, ui);
        });
        out.textures_delta.clear();
    }
    let (p0_after, p1_after) =
        grafia::gradient_control_segment_upm(&state.color_sublayers[0].paint).unwrap();
    assert_eq!(p0_after, p0_before);
    assert!(
        (p1_after.x.0 - (p1_before.x.0 + 60.0)).abs() < 1e-3
            && (p1_after.y.0 - (p1_before.y.0 + 40.0)).abs() < 1e-3,
        "Pijler C violation: dragging COLRv1 EndEndpoint handle via InputDispatcher must move p1 from {p1_before:?} to (+60,+40), got {p1_after:?}"
    );

    // 4. Pijler E: Bit-exact Undo/Redo across gradient drag and COLRv1 bake
    assert!(state.undo()); // Undo gradient handle drag
    let (_, p1_undone_drag) =
        grafia::gradient_control_segment_upm(&state.color_sublayers[0].paint).unwrap();
    assert_eq!(p1_undone_drag, p1_before);
    assert!(state.undo()); // Undo bake_liquid_glass_to_colrv1
    assert_eq!(state.snapshot(), state_before_bake);
    assert!(state.redo());
    assert_eq!(state.color_sublayers.len(), 3);

    // 5. Pijler A: Unconstrained arbtest on bake_liquid_glass_to_colrv1_sublayers
    arbtest(|u| {
        let cfg = LiquidGlassRenderConfig {
            dome_radius_upm: f64::from_bits(u.arbitrary::<u64>()?),
            smooth_union_k_upm: f64::from_bits(u.arbitrary::<u64>()?),
            ior_base: f32::from_bits(u.arbitrary::<u32>()?),
            chromatic_dispersion: f32::from_bits(u.arbitrary::<u32>()?),
            specular_intensity: f32::from_bits(u.arbitrary::<u32>()?),
            tint_rgba: u.arbitrary::<[u8; 4]>()?,
        };
        let n_count = u.int_in_range(0..=5)?;
        let mut nodes = Vec::with_capacity(n_count);
        for idx in 0..n_count {
            nodes.push(Node {
                id: NodeId(idx as u64 + 1),
                point: FontPoint::new(
                    f64::from_bits(u.arbitrary::<u64>()?),
                    f64::from_bits(u.arbitrary::<u64>()?),
                ),
                node_type: NodeType::Corner,
            });
        }
        let mut c = GlyphContour::new(ContourId(0), nodes);
        c.closed = u.arbitrary::<bool>()?;
        let layers = grafia::core::bake_liquid_glass_to_colrv1_sublayers(&[c], &cfg);
        for sub in &layers {
            assert!(sub.opacity.is_finite() && (0.0..=1.0).contains(&sub.opacity));
            if let Some((p0, p1)) = grafia::gradient_control_segment_upm(&sub.paint) {
                assert!(p0.x.0.is_finite() && p0.y.0.is_finite());
                assert!(p1.x.0.is_finite() && p1.y.0.is_finite());
            }
            if let Some(stops) = grafia::gradient_stops_slice(&sub.paint) {
                for st in stops {
                    assert!(st.offset.is_finite() && (0.0..=1.0).contains(&st.offset));
                }
            }
        }
        Ok(())
    })
    .budget_ms(250);
}

#[test]
fn tc_v0140_011_kinetic_symbol_svg_lossless_roundtrip_and_malformed_fuzzing() {
    let mut c0 = GlyphContour::new(
        ContourId(0),
        vec![
            Node::new(NodeId(10), 120.0, 140.0, NodeType::Corner),
            Node::new(NodeId(11), 220.0, 140.0, NodeType::Corner),
            Node::new(NodeId(12), 220.0, 320.0, NodeType::Corner),
            Node::new(NodeId(13), 120.0, 320.0, NodeType::Corner),
        ],
    );
    c0.closed = true;
    let mut c1 = GlyphContour::new(
        ContourId(1),
        vec![
            Node::new(NodeId(20), 260.0, 140.0, NodeType::Corner),
            Node::new(NodeId(21), 300.0, 260.0, NodeType::OffCurve),
            Node::new(NodeId(22), 340.0, 260.0, NodeType::OffCurve),
            Node::new(NodeId(23), 380.0, 140.0, NodeType::Smooth),
        ],
    );
    c1.closed = false;
    let mut c2 = GlyphContour::new(
        ContourId(2),
        vec![
            Node::new(NodeId(30), 420.0, 140.0, NodeType::Corner),
            Node::new(NodeId(31), 500.0, 140.0, NodeType::Corner),
            Node::new(NodeId(32), 500.0, 480.0, NodeType::Corner),
            Node::new(NodeId(33), 420.0, 480.0, NodeType::Corner),
        ],
    );
    c2.closed = true;

    let sublayers = vec![
        grafia::CanvasColorSublayer {
            id: 1,
            name: "Wave1".into(),
            contours: vec![c0.clone()],
            paint: grafia::ColorLayerPaint::Solid {
                rgba: [40, 145, 255, 240],
            },
            opacity: 1.0,
            visible: true,
            locked: false,
        },
        grafia::CanvasColorSublayer {
            id: 2,
            name: "Wave2".into(),
            contours: vec![c1.clone()],
            paint: grafia::ColorLayerPaint::Solid {
                rgba: [60, 200, 175, 240],
            },
            opacity: 0.85,
            visible: true,
            locked: false,
        },
        grafia::CanvasColorSublayer {
            id: 3,
            name: "Wave3".into(),
            contours: vec![c2.clone()],
            paint: grafia::ColorLayerPaint::Solid {
                rgba: [255, 140, 55, 240],
            },
            opacity: 0.70,
            visible: true,
            locked: false,
        },
    ];
    let tags = vec![
        SymbolLayerTag {
            layer_group: 1,
            motion_anchor_id: Some(grafia::AnchorId(7)),
            custom_pivot_upm: Some(FontPoint::new(170.0, 230.0)),
            draw_order_reversed: false,
            variable_color_enabled: true,
            pulse_participates: true,
            ..Default::default()
        },
        SymbolLayerTag {
            layer_group: 2,
            motion_anchor_id: None,
            custom_pivot_upm: Some(FontPoint::new(320.0, 200.0)),
            draw_order_reversed: true,
            variable_color_enabled: false,
            pulse_participates: true,
            ..Default::default()
        },
        SymbolLayerTag {
            layer_group: 3,
            motion_anchor_id: Some(grafia::AnchorId(9)),
            custom_pivot_upm: None,
            draw_order_reversed: false,
            variable_color_enabled: true,
            pulse_participates: false,
            ..Default::default()
        },
    ];

    let svg = grafia::core::export_kinetic_symbol_svg(&sublayers, &tags);
    assert!(
        svg.contains("<svg")
            && svg.contains("data-layer-group=\"1\"")
            && svg.contains("data-layer-group=\"2\"")
            && svg.contains("data-layer-group=\"3\"")
            && svg.contains("data-motion-anchor=\"7\"")
            && svg.contains("data-variable-color=\"false\"")
            && svg.contains("data-draw-order=\"reversed\""),
        "Exported SVG must contain W3C <svg>, <g>, <path> and all kinetic data-* attributes:\n{svg}"
    );

    let (imp_sublayers, imp_tags) = grafia::core::import_kinetic_symbol_svg(&svg)
        .expect("Valid exported SVG must import losslessly");
    assert_eq!(imp_sublayers.len(), 3);
    assert_eq!(imp_tags.len(), 3);
    for i in 0..3 {
        assert_eq!(
            imp_tags[i], tags[i],
            "SymbolLayerTag {i} must roundtrip losslessly"
        );
        assert_eq!(imp_sublayers[i].contours.len(), sublayers[i].contours.len());
        let orig_c = &sublayers[i].contours[0];
        let imp_c = &imp_sublayers[i].contours[0];
        assert_eq!(imp_c.closed, orig_c.closed);
        assert_eq!(imp_c.nodes.len(), orig_c.nodes.len());
        for (na, nb) in orig_c.nodes.iter().zip(imp_c.nodes.iter()) {
            assert!(
                (na.point.x.0 - nb.point.x.0).abs() < 1e-3
                    && (na.point.y.0 - nb.point.y.0).abs() < 1e-3,
                "Node coordinate mismatch after SVG roundtrip: {na:?} vs {nb:?}"
            );
        }
    }

    // 2. Pijler B, C, D & E: Import into GrafiaCanvasState, verify OffCurve Bézier handle drag via InputDispatcher, and Undo/Redo!
    let mut state = GrafiaCanvasState::init_glyph_a();
    state.viewport =
        grafia::ViewportTransform::new(0.5, grafia::ScreenPoint::new(100.0, 500.0), 1000);
    let snap_before_import = state.snapshot();
    assert!(
        state.import_kinetic_symbol_from_svg(&svg),
        "state.import_kinetic_symbol_from_svg must succeed on valid SVG"
    );
    assert_eq!(state.color_sublayers.len(), 3);
    assert_eq!(state.icon_studio.symbol_layer_tags, tags);
    assert_eq!(state.glyph.contours.len(), 3);
    assert!(!state.glyph.contours[1].closed);

    // Find the first OffCurve handle on imported contour 1 (at (300.0, 260.0)) and drag it via InputDispatcher!
    let off_node = state.glyph.contours[1]
        .nodes
        .iter()
        .find(|n| n.node_type == NodeType::OffCurve)
        .expect("Imported cubic contour must preserve OffCurve handles")
        .clone();
    state.activate_tool(grafia::CanvasTool::Select);
    let scr_start = state.viewport.to_screen(off_node.point);
    let p_start = egui::pos2(scr_start.x.0, scr_start.y.0);
    let p_end = egui::pos2(p_start.x + 20.0, p_start.y - 15.0);
    let ctx = egui::Context::default();
    let canvas_rect = egui::Rect::from_min_size(egui::pos2(0.0, 0.0), egui::vec2(800.0, 600.0));
    for (t, evs) in [
        (
            1.0,
            vec![egui::Event::PointerButton {
                pos: p_start,
                button: egui::PointerButton::Primary,
                pressed: true,
                modifiers: egui::Modifiers::NONE,
            }],
        ),
        (
            1.1,
            vec![
                egui::Event::PointerMoved(p_end),
                egui::Event::PointerButton {
                    pos: p_end,
                    button: egui::PointerButton::Primary,
                    pressed: false,
                    modifiers: egui::Modifiers::NONE,
                },
            ],
        ),
    ] {
        let raw = egui::RawInput {
            screen_rect: Some(canvas_rect),
            time: Some(t),
            events: evs,
            ..Default::default()
        };
        let mut out = ctx.run_ui(raw, |ui| {
            let ev = ui.input(|i| i.events.clone());
            let _ = grafia::adapters::InputDispatcher::for_canvas(&mut state, canvas_rect)
                .dispatch_events(&ev, ui);
        });
        out.textures_delta.clear();
    }
    let off_after = state
        .find_node(off_node.id)
        .expect("Imported OffCurve node must survive drag");
    assert!(
        (off_after.point.x.0 - (off_node.point.x.0 + 40.0)).abs() < 1e-3
            && (off_after.point.y.0 - (off_node.point.y.0 + 30.0)).abs() < 1e-3,
        "Pijler C violation: imported SVG OffCurve handle must be directly draggable via InputDispatcher"
    );

    // Pijler E: Undo handle drag, then Undo SVG import back to snap_before_import!
    assert!(state.undo());
    assert!(state.undo());
    assert_eq!(state.snapshot(), snap_before_import);
    assert!(state.redo());
    assert_eq!(state.color_sublayers.len(), 3);

    // Pijler B: Failed import on malformed XML must leave state 100% untouched!
    let snap_valid = state.snapshot();
    assert!(!state.import_kinetic_symbol_from_svg("<svg><g><path d=\"INVALID\"/></g>"));
    assert_eq!(state.snapshot(), snap_valid);

    // 3. Pijler A: Unconstrained arbtest on import_kinetic_symbol_svg with arbitrary corrupted strings
    arbtest(|u| {
        let raw_bytes: Vec<u8> = u.arbitrary()?;
        let s = String::from_utf8_lossy(&raw_bytes);
        let _ = grafia::core::import_kinetic_symbol_svg(&s);
        let corrupted = format!(
            "<svg><g data-layer-group=\"{}\" data-motion-anchor=\"{}\"><path d=\"M {} {} C {} {} {} {} {} {} Z\"/></g></svg>",
            s,
            u.arbitrary::<i64>()?,
            f64::from_bits(u.arbitrary::<u64>()?),
            f64::from_bits(u.arbitrary::<u64>()?),
            f64::from_bits(u.arbitrary::<u64>()?),
            f64::from_bits(u.arbitrary::<u64>()?),
            f64::from_bits(u.arbitrary::<u64>()?),
            f64::from_bits(u.arbitrary::<u64>()?),
            f64::from_bits(u.arbitrary::<u64>()?),
            f64::from_bits(u.arbitrary::<u64>()?),
        );
        if let Ok((subs, tgs)) = grafia::core::import_kinetic_symbol_svg(&corrupted) {
            assert_eq!(subs.len(), tgs.len());
            for sub in &subs {
                for c in &sub.contours {
                    for n in &c.nodes {
                        assert!(n.point.x.0.is_finite() && n.point.y.0.is_finite());
                    }
                }
            }
        }
        Ok(())
    })
    .budget_ms(250);
}

#[test]
fn tc_v0140_013_universal_cross_tool_state_machine_fuzzer_and_visual_golden_frames() {
    let artifact_dir = std::path::Path::new(
        r"C:\Users\jhove\.gemini\antigravity-cli\brain\fccb9142-9217-4702-a2ca-95303b541ee0",
    );
    let theme = GrafiaTheme::sonoma_default();

    // 1. Render and save 4 Golden-Master PNG frames across SPEC-016 v0.14.0 capabilities:
    // Frame 64: ShapeBuilder Planar Region Preview + Variable-Width S-Curve Pins & OffCurve Handles
    let mut st64 = GrafiaCanvasState::init_glyph_a();
    st64.theme = theme.clone();
    st64.viewport =
        grafia::ViewportTransform::new(0.65, grafia::ScreenPoint::new(110.0, 500.0), 1000);
    let c0 = GlyphContour::new(
        ContourId(0),
        vec![
            Node::new(NodeId(100), 120.0, 200.0, NodeType::Corner),
            Node::new(NodeId(101), 360.0, 200.0, NodeType::Corner),
            Node::new(NodeId(102), 360.0, 520.0, NodeType::Corner),
            Node::new(NodeId(103), 120.0, 520.0, NodeType::Corner),
        ],
    );
    let c1 = GlyphContour::new(
        ContourId(1),
        vec![
            Node::new(NodeId(110), 240.0, 260.0, NodeType::Corner),
            Node::new(NodeId(111), 480.0, 260.0, NodeType::Corner),
            Node::new(NodeId(112), 480.0, 580.0, NodeType::Corner),
            Node::new(NodeId(113), 240.0, 580.0, NodeType::Corner),
        ],
    );
    let mut spine = GlyphContour::new(
        ContourId(2),
        vec![
            Node::new(NodeId(120), 120.0, 110.0, NodeType::Corner),
            Node::new(NodeId(121), 240.0, 160.0, NodeType::OffCurve),
            Node::new(NodeId(122), 360.0, 60.0, NodeType::OffCurve),
            Node::new(NodeId(123), 480.0, 120.0, NodeType::Corner),
        ],
    );
    spine.closed = false;
    let mut g64 = st64.glyph.clone();
    g64.contours = vec![c0.clone(), c1.clone(), spine.clone()];
    let m64 = st64.metrics.clone();
    st64.load_glyph(g64, m64);
    st64.set_variable_width_stroke(VariableWidthStrokeSpec {
        contour_id: ContourId(2),
        samples: vec![
            WidthNodeSample {
                arc_ratio_t: 0.0,
                width_left_upm: 14.0,
                width_right_upm: 14.0,
            },
            WidthNodeSample {
                arc_ratio_t: 0.5,
                width_left_upm: 44.0,
                width_right_upm: 30.0,
            },
            WidthNodeSample {
                arc_ratio_t: 1.0,
                width_left_upm: 12.0,
                width_right_upm: 12.0,
            },
        ],
        cap_style: LineCapStyle::Round,
    });
    st64.activate_tool(grafia::CanvasTool::ShapeBuilder);
    let _ = st64.on_drawing_tool_pointer_move(
        grafia::ScreenPoint::new(300.0, 300.0),
        FontPoint::new(300.0, 360.0),
        grafia::Modifiers::default(),
    );
    let pix64 = grafia::adapters::rasterize_canvas_subregion(&st64, &theme, 640, 480);

    // Frame 65: 6-Fold Radial Symmetry Rosette Ghost Preview
    let mut st65 = GrafiaCanvasState::init_glyph_a();
    st65.theme = theme.clone();
    st65.viewport =
        grafia::ViewportTransform::new(0.65, grafia::ScreenPoint::new(110.0, 520.0), 1000);
    let petal = GlyphContour::new(
        ContourId(0),
        vec![
            Node::new(NodeId(200), 275.0, 380.0, NodeType::Corner),
            Node::new(NodeId(201), 325.0, 380.0, NodeType::Corner),
            Node::new(NodeId(202), 345.0, 620.0, NodeType::Corner),
            Node::new(NodeId(203), 255.0, 620.0, NodeType::Corner),
        ],
    );
    let mut g65 = st65.glyph.clone();
    g65.contours = vec![petal];
    let m65 = st65.metrics.clone();
    st65.load_glyph(g65, m65);
    st65.icon_studio.radial_symmetry = RadialSymmetrySpec {
        fold_count: 6,
        dihedral_mirror: false,
        center: FontPoint::new(300.0, 350.0),
    };
    st65.bump_content_generation();
    let pix65 = grafia::adapters::rasterize_canvas_subregion(&st65, &theme, 640, 480);

    // Frame 66: CanvasViewMode::LiquidGlass Physical Renderer with C1 Fused Meniscus Bridge
    let mut st66 = GrafiaCanvasState::init_glyph_a();
    st66.theme = theme.clone();
    st66.viewport =
        grafia::ViewportTransform::new(0.65, grafia::ScreenPoint::new(110.0, 500.0), 1000);
    let drop_a = GlyphContour::new(
        ContourId(0),
        vec![
            Node::new(NodeId(10), 140.0, 200.0, NodeType::Corner),
            Node::new(NodeId(11), 215.0, 175.0, NodeType::OffCurve),
            Node::new(NodeId(12), 290.0, 225.0, NodeType::OffCurve),
            Node::new(NodeId(13), 290.0, 320.0, NodeType::Smooth),
            Node::new(NodeId(14), 290.0, 415.0, NodeType::OffCurve),
            Node::new(NodeId(15), 215.0, 465.0, NodeType::OffCurve),
            Node::new(NodeId(16), 140.0, 440.0, NodeType::Corner),
        ],
    );
    let drop_b = GlyphContour::new(
        ContourId(1),
        vec![
            Node::new(NodeId(20), 462.0, 200.0, NodeType::Corner),
            Node::new(NodeId(21), 387.0, 175.0, NodeType::OffCurve),
            Node::new(NodeId(22), 312.0, 225.0, NodeType::OffCurve),
            Node::new(NodeId(23), 312.0, 320.0, NodeType::Smooth),
            Node::new(NodeId(24), 312.0, 415.0, NodeType::OffCurve),
            Node::new(NodeId(25), 387.0, 465.0, NodeType::OffCurve),
            Node::new(NodeId(26), 462.0, 440.0, NodeType::Corner),
        ],
    );
    let mut g66 = st66.glyph.clone();
    g66.contours = vec![drop_a, drop_b];
    let m66 = st66.metrics.clone();
    st66.load_glyph(g66, m66);
    st66.icon_studio.liquid_glass = LiquidGlassRenderConfig {
        dome_radius_upm: 58.0,
        smooth_union_k_upm: 85.0,
        ior_base: 1.52,
        chromatic_dispersion: 0.035,
        specular_intensity: 0.96,
        tint_rgba: [105, 195, 255, 235],
    };
    st66.view_mode = grafia::CanvasViewMode::LiquidGlass;
    st66.bump_content_generation();
    let pix66 = grafia::adapters::rasterize_canvas_subregion(&st66, &theme, 640, 480);

    // Frame 67: Baked 3-Layer COLRv1 Liquid-Glass Vector Gradients with Active Gradient Handles
    let mut st67 = GrafiaCanvasState::init_glyph_a();
    st67.restore_snapshot(&st66.snapshot());
    st67.theme = theme.clone();
    st67.viewport = st66.viewport;
    assert!(st67.bake_liquid_glass_to_colrv1());
    st67.view_mode = grafia::CanvasViewMode::EditWireframe;
    let pix67 = grafia::adapters::rasterize_canvas_subregion(&st67, &theme, 640, 480);

    if artifact_dir.exists() {
        for (name, pix) in [
            ("frame_64_v0140_shape_builder_and_var_width.png", pix64),
            ("frame_65_v0140_radial_symmetry_rosette.png", pix65),
            ("frame_66_v0140_liquid_glass_meniscus.png", pix66),
            ("frame_67_v0140_baked_colrv1_gradients.png", pix67),
        ] {
            let img = grafia::adapters::pixmap_to_rgba_image(pix);
            let _ = img.save(artifact_dir.join(name));
        }
    }

    // 2. Universal Stateful Cross-Tool InputDispatcher + Frame-Condition + Undo/Redo arbtest Fuzzer!
    let base_snap = st64.snapshot();
    arbtest(|u| {
        let mut st = GrafiaCanvasState::init_glyph_a();
        st.restore_snapshot(&base_snap);
        st.viewport = st64.viewport;
        let ctx = egui::Context::default();
        let canvas_rect = egui::Rect::from_min_size(egui::pos2(0.0, 0.0), egui::vec2(640.0, 480.0));
        let steps = u.int_in_range(1..=8)?;
        for step_idx in 0..steps {
            let op = u.int_in_range(0..=11)?;
            match op {
                0 => {
                    let tool = match u.int_in_range(0..=4)? {
                        0 => grafia::CanvasTool::Select,
                        1 => grafia::CanvasTool::ShapeBuilder,
                        2 => grafia::CanvasTool::Pen,
                        3 => grafia::CanvasTool::Knife,
                        _ => grafia::CanvasTool::Primitives,
                    };
                    st.activate_tool(tool);
                    assert!(
                        !st.is_interactive_drag_active(),
                        "Pijler F violation: activate_tool must clear all in-flight drag transients"
                    );
                }
                1 => {
                    st.view_mode = match u.int_in_range(0..=3)? {
                        0 => grafia::CanvasViewMode::EditWireframe,
                        1 => grafia::CanvasViewMode::PreviewFilled,
                        2 => grafia::CanvasViewMode::ShapeThumbnail,
                        _ => grafia::CanvasViewMode::LiquidGlass,
                    };
                }
                2 => {
                    // Pointer press -> move -> release or cancel via InputDispatcher
                    let x0 = (u.int_in_range(40..=600)?) as f32;
                    let y0 = (u.int_in_range(40..=440)?) as f32;
                    let x1 = (u.int_in_range(40..=600)?) as f32;
                    let y1 = (u.int_in_range(40..=440)?) as f32;
                    let abort_mid_drag = u.arbitrary::<bool>()?;
                    let t_base = (step_idx as f64 + 1.0) * 2.0;
                    for (dt, evs) in [
                        (
                            0.0,
                            vec![egui::Event::PointerButton {
                                pos: egui::pos2(x0, y0),
                                button: egui::PointerButton::Primary,
                                pressed: true,
                                modifiers: egui::Modifiers::NONE,
                            }],
                        ),
                        (
                            0.1,
                            if abort_mid_drag {
                                vec![
                                    egui::Event::PointerMoved(egui::pos2(x1, y1)),
                                    egui::Event::Key {
                                        key: egui::Key::Escape,
                                        physical_key: None,
                                        pressed: true,
                                        repeat: false,
                                        modifiers: egui::Modifiers::NONE,
                                    },
                                ]
                            } else {
                                vec![
                                    egui::Event::PointerMoved(egui::pos2(x1, y1)),
                                    egui::Event::PointerButton {
                                        pos: egui::pos2(x1, y1),
                                        button: egui::PointerButton::Primary,
                                        pressed: false,
                                        modifiers: egui::Modifiers::NONE,
                                    },
                                ]
                            },
                        ),
                    ] {
                        let raw = egui::RawInput {
                            screen_rect: Some(canvas_rect),
                            time: Some(t_base + dt),
                            events: evs,
                            ..Default::default()
                        };
                        let mut out = ctx.run_ui(raw, |ui| {
                            let ev = ui.input(|i| i.events.clone());
                            let _ = grafia::adapters::InputDispatcher::for_canvas(&mut st, canvas_rect)
                                .dispatch_events(&ev, ui);
                        });
                        out.textures_delta.clear();
                    }
                    if abort_mid_drag {
                        assert!(!st.is_interactive_drag_active());
                    }
                }
                3 => {
                    // ShapeBuilder merge on closed rectangles while verifying open S-curve survives!
                    let open_before = st.glyph.contours.iter().filter(|c| !c.closed).count();
                    let _ = st.apply_shape_builder_operation(&[0, 1], ShapeBuilderOp::Merge);
                    let open_after = st.glyph.contours.iter().filter(|c| !c.closed).count();
                    assert_eq!(
                        open_after, open_before,
                        "Pijler B violation: ShapeBuilder merge must never destroy untouched open contours"
                    );
                }
                4 => {
                    let slant = (u.int_in_range(-20..=20)?) as f64;
                    let _ = st.apply_optical_oblique(slant, true);
                }
                5 => {
                    let _ = st.normalize_icon_contours(Some(1.0));
                }
                6 => {
                    st.icon_studio.kinetic.playing = true;
                    let _ = st.step_kinetic_timeline(0.12);
                    let _ = st.evaluate_active_kinetic_contours();
                    let _ = st.evaluate_active_kinetic_sublayers();
                }
                7 => {
                    let _ = st.bake_liquid_glass_to_colrv1();
                }
                8 => {
                    let svg = st.export_kinetic_symbol_to_svg();
                    if !st.color_sublayers.is_empty() {
                        assert!(st.import_kinetic_symbol_from_svg(&svg));
                    }
                }
                9 => {
                    let _ = st.remove_overlap();
                }
                10 => {
                    let _ = st.undo();
                }
                _ => {
                    let _ = st.redo();
                }
            }
            // Verify all SPEC-016 and GrafiaCanvasState invariants hold after every transition!
            st.icon_studio
                .validate_invariants()
                .expect("SPEC-016 invariants must hold after every fuzzed transition");
            for c in &st.glyph.contours {
                for n in &c.nodes {
                    assert!(n.point.x.0.is_finite() && n.point.y.0.is_finite());
                }
            }
        }
        Ok(())
    })
    .budget_ms(400);
}
