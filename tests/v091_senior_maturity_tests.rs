//! Normative Verification Suite for `SPEC-014` (`grafia v0.9.1` — Senior Architectural Maturity).
//! Covers `TC-MATURITY-001`..`TC-MATURITY-010`.

mod common;

#[path = "../examples/vector_canvas_demo.rs"]
pub mod demo_app;

use grafia::render::canvas_render_cache::get_or_upload_color_sublayers_texture;
use grafia::{
    ActiveMirrorAxis, AnchorId, CanvasAnchor, CanvasColorSublayer, CanvasCurveHarmonyScene,
    CanvasDocumentScene, CanvasGuideSet, CanvasHintingAndHoiScene, CanvasLayerAttachments,
    CanvasTool, ColorLayerPaint, ContourId, CurvedTrajectoryLink, CustomReferenceFrame, FontPoint,
    Glyph, GlyphContour, GlyphMetrics, GradientExtend, GradientStop, GrafiaCanvasState,
    GrafiaTheme, Guide, GuideId, Modifiers, Node, NodeId, NodeType, PostScriptHint,
    PostScriptHintAxis, RectUpm, ReferenceFramePalette, RulerTickRangeUpm, SelectToolState,
    SmartCornerSpec, TrueTypeInstructionKind, TrueTypeRoundingGrid, TrueTypeVisualInstruction,
    WritingDirection, MAX_INTERACTIVE_DRAG_PIXMAP_DIM,
};

fn sample_closed_triangle_contour(id: usize, offset_x: f64, offset_y: f64) -> GlyphContour {
    let mut gc = GlyphContour::new(
        ContourId(id),
        vec![
            Node::new(
                NodeId((id as u64) * 10 + 1),
                100.0 + offset_x,
                100.0 + offset_y,
                NodeType::Corner,
            ),
            Node::new(
                NodeId((id as u64) * 10 + 2),
                400.0 + offset_x,
                100.0 + offset_y,
                NodeType::Corner,
            ),
            Node::new(
                NodeId((id as u64) * 10 + 3),
                250.0 + offset_x,
                500.0 + offset_y,
                NodeType::Corner,
            ),
        ],
    );
    gc.closed = true;
    gc
}

#[test]
fn tc_maturity_001_transient_substructs_and_snapshot_cohesion_undo_redo_roundtrip() {
    let mut state = GrafiaCanvasState::new(GrafiaTheme::sonoma_default());

    // Verify cohesive transient sub-structs exist and start clean
    assert!(state.pen_transients.drag_last_pt.is_none());
    assert!(state.drag_transients.drag_start_font_positions.is_empty());
    assert!(!state.text_transients.suppress_text_stream_sync);

    let c0 = sample_closed_triangle_contour(0, 0.0, 0.0);
    state.glyph.contours = vec![c0.clone()];
    state.contours = vec![c0.nodes];
    state.sync_paths_from_contours();

    let before = state.snapshot();
    assert!(before.layers.anchors.is_empty());
    assert!(before.hinting.postscript_hints.is_empty());

    // Mutate across multiple domains and verify bit-exact Undo -> Redo roundtrip
    state.anchors.push(CanvasAnchor::new(
        AnchorId(1),
        "top",
        FontPoint::new(250.0, 700.0),
    ));
    state.postscript_hints.push(PostScriptHint {
        id: 1,
        axis: PostScriptHintAxis::Horizontal,
        origin_upm: 100.0,
        width_upm: 60.0,
        is_ghost: false,
        top_ghost: false,
        start_node: None,
        end_node: None,
        selected: false,
    });
    state.color_sublayers.push(CanvasColorSublayer {
        id: 1,
        name: "Base".to_string(),
        contours: state.glyph.contours.clone(),
        paint: ColorLayerPaint::Solid {
            rgba: [220, 40, 40, 255],
        },
        opacity: 1.0,
        visible: true,
        locked: false,
    });
    state.record_mutation("Multi-Domain Edit", before.clone());
    let after = state.snapshot();

    assert!(state.undo());
    assert_eq!(state.snapshot(), before);
    assert!(state.redo());
    assert_eq!(state.snapshot(), after);
    assert!(state.validate_invariants().is_ok());
}

#[test]
fn tc_maturity_002_named_domain_parameter_contexts_deterministic() {
    let tick_range = RulerTickRangeUpm {
        start_step: -2,
        end_step: 10,
        minor_step_upm: 50,
        major_step_upm: 200,
    };
    assert_eq!(tick_range.start_step, -2);
    assert_eq!(tick_range.end_step, 10);
    assert_eq!(tick_range.minor_step_upm, 50);
    assert_eq!(tick_range.major_step_upm, 200);

    let theme = GrafiaTheme::sonoma_dark();
    let palette = ReferenceFramePalette::from_theme(&theme);
    assert_eq!(palette.outer, theme.reference_frame_outer.to_color32());
    assert_eq!(palette.inner, theme.reference_frame_inner.to_color32());
}

#[test]
fn tc_maturity_003_live_drag_caps_color_layer_pixmap_at_max_interactive_dim_on_4k() {
    assert_eq!(MAX_INTERACTIVE_DRAG_PIXMAP_DIM, 512);

    let mut state = GrafiaCanvasState::new(GrafiaTheme::sonoma_default());
    let c0 = sample_closed_triangle_contour(0, 0.0, 0.0);
    state.glyph.contours = vec![c0.clone()];
    state.contours = vec![c0.nodes];
    state.sync_paths_from_contours();
    state.color_sublayers.push(CanvasColorSublayer {
        id: 1,
        name: "Gradient Fill".to_string(),
        contours: state.glyph.contours.clone(),
        paint: ColorLayerPaint::LinearGradient {
            p0: FontPoint::new(100.0, 100.0),
            p1: FontPoint::new(400.0, 500.0),
            stops: vec![
                GradientStop {
                    offset: 0.0,
                    rgba: [255, 60, 0, 255],
                },
                GradientStop {
                    offset: 1.0,
                    rgba: [0, 120, 255, 255],
                },
            ],
            spread: GradientExtend::Pad,
        },
        opacity: 1.0,
        visible: true,
        locked: false,
    });

    // Simulate active node drag on a 4K (3840 x 2160) canvas
    state.select_state = SelectToolState::DraggingNodes {
        selected: vec![NodeId(1)],
        start_pt: FontPoint::new(100.0, 100.0),
        current_pt: FontPoint::new(120.0, 115.0),
        modifiers: Modifiers::default(),
    };
    assert!(state.is_interactive_drag_active());

    let (eff_w, eff_h, scale) = state.effective_color_layer_pixmap_dimensions(3840, 2160);
    assert_eq!(eff_w, 512);
    assert_eq!(eff_h, 288);
    assert!((scale - (512.0 / 3840.0)).abs() < 1e-5);

    let ctx = egui::Context::default();
    let tex = get_or_upload_color_sublayers_texture(&ctx, &state, 3840, 2160);
    assert_eq!(tex.size(), [512, 288]);
}

#[test]
fn tc_maturity_004_pointer_release_restores_native_4k_pixmap_and_caches_idle_frames() {
    let mut state = GrafiaCanvasState::new(GrafiaTheme::sonoma_default());
    let c0 = sample_closed_triangle_contour(0, 0.0, 0.0);
    state.glyph.contours = vec![c0.clone()];
    state.contours = vec![c0.nodes];
    state.sync_paths_from_contours();
    state.color_sublayers.push(CanvasColorSublayer {
        id: 1,
        name: "Solid Fill".to_string(),
        contours: state.glyph.contours.clone(),
        paint: ColorLayerPaint::Solid {
            rgba: [40, 180, 90, 255],
        },
        opacity: 1.0,
        visible: true,
        locked: false,
    });

    let ctx = egui::Context::default();

    // 1. Active drag at 1920 by 1080 -> capped at 512 by 288
    state.select_state = SelectToolState::DraggingNodes {
        selected: vec![NodeId(1)],
        start_pt: FontPoint::new(100.0, 100.0),
        current_pt: FontPoint::new(110.0, 110.0),
        modifiers: Modifiers::default(),
    };
    let drag_tex = get_or_upload_color_sublayers_texture(&ctx, &state, 1920, 1080);
    assert_eq!(drag_tex.size(), [512, 288]);

    // 2. Release pointer -> immediately restores native 1920 by 1080 resolution
    state.select_state = SelectToolState::Idle;
    assert!(!state.is_interactive_drag_active());
    let idle_tex = get_or_upload_color_sublayers_texture(&ctx, &state, 1920, 1080);
    assert_eq!(idle_tex.size(), [1920, 1080]);

    // 3. Subsequent idle frames perform 0 additional GPU texture allocations
    let allocated_after_release = ctx.tex_manager().read().allocated().len();
    for _ in 0..5 {
        let cached = get_or_upload_color_sublayers_texture(&ctx, &state, 1920, 1080);
        assert_eq!(cached.id(), idle_tex.id());
    }
    assert_eq!(
        ctx.tex_manager().read().allocated().len(),
        allocated_after_release
    );
}

fn sample_hinting_and_hoi_scene() -> CanvasHintingAndHoiScene {
    CanvasHintingAndHoiScene {
        postscript_hints: vec![PostScriptHint {
            id: 7,
            axis: PostScriptHintAxis::Vertical,
            origin_upm: 120.0,
            width_upm: 80.0,
            is_ghost: false,
            top_ghost: false,
            start_node: None,
            end_node: None,
            selected: false,
        }],
        truetype_instructions: vec![TrueTypeVisualInstruction {
            id: 3,
            kind: TrueTypeInstructionKind::Anchor,
            source_node: None,
            target_node: NodeId(1),
            ppem: 24,
            round_mode: TrueTypeRoundingGrid::ToGrid,
            stem_index: None,
            zone_index: None,
            delta_pixels: 0.0,
            x_axis: true,
            selected: false,
        }],
        truetype_instructor_ppem: 24,
        hoi_layer_sets: Vec::new(),
        hoi_sibling_contours: None,
        show_pixel_preview_ppem: Some(24),
    }
}

fn sample_layer_attachments() -> CanvasLayerAttachments {
    CanvasLayerAttachments {
        background_contours: vec![sample_closed_triangle_contour(1, 0.0, 0.0)],
        anchors: vec![CanvasAnchor::new(
            AnchorId(10),
            "top",
            FontPoint::new(320.0, 700.0),
        )],
        components: Vec::new(),
        contour_strokes: std::collections::BTreeMap::new(),
        companion_master_layers: Vec::new(),
        smart_handles: Vec::new(),
        background_image: None,
        backdrop_ghost_contours: vec![sample_closed_triangle_contour(2, -50.0, 0.0)],
    }
}

#[test]
fn tc_maturity_005_canvas_document_scene_host_load_and_export_roundtrip() {
    let glyph = Glyph {
        contours: vec![sample_closed_triangle_contour(0, 20.0, 30.0)],
        ..Glyph::default()
    };

    let scene = CanvasDocumentScene {
        glyph,
        metrics: GlyphMetrics {
            advance_width: 640.0,
            ascender: 800.0,
            cap_height: 700.0,
            x_height: 500.0,
            baseline: 0.0,
            descender: -200.0,
            italic_angle_deg: 0.0,
            alignment_zones: Vec::new(),
        },
        guides: CanvasGuideSet {
            local_guides: vec![Guide::new(
                GuideId(1),
                FontPoint::new(0.0, 500.0),
                0.0,
                false,
            )],
            global_guides: vec![Guide::new(
                GuideId(2),
                FontPoint::new(320.0, 0.0),
                90.0,
                true,
            )],
            show_guides: true,
            lock_guides: false,
        },
        layers: sample_layer_attachments(),
        color_layers: vec![CanvasColorSublayer {
            id: 1,
            name: "Primary".to_string(),
            contours: vec![sample_closed_triangle_contour(0, 20.0, 30.0)],
            paint: ColorLayerPaint::Solid {
                rgba: [15, 130, 240, 255],
            },
            opacity: 0.9,
            visible: true,
            locked: false,
        }],
        hinting: sample_hinting_and_hoi_scene(),
        reference_frame: Some(CustomReferenceFrame {
            outer_box: Some(RectUpm::new(0.0, -200.0, 1000.0, 800.0)),
            inner_box: Some(RectUpm::new(100.0, -100.0, 900.0, 700.0)),
            inner_grid_subdivisions: Some((3, 3)),
            center_crosshair: true,
            keyline_circles_radii_upm: vec![250.0, 400.0],
            parallel_lines: Vec::new(),
            ..Default::default()
        }),
        writing_direction: WritingDirection::TopToBottom,
        curve_harmony: CanvasCurveHarmonyScene::default(),
        ..CanvasDocumentScene::default()
    };

    let mut state = GrafiaCanvasState::new(GrafiaTheme::sonoma_default());
    state.load_document_scene(scene.clone());

    assert!(state.validate_invariants().is_ok());
    assert_eq!(state.contours.len(), 1);
    assert_eq!(state.paths.len(), 1);
    assert_eq!(
        state.text_stream.writing_direction,
        WritingDirection::TopToBottom
    );

    let exported = state.export_document_scene();
    assert_eq!(exported, scene);
}

#[test]
fn tc_maturity_006_ruler_and_text_stream_theme_tokens_runtime_configurable() {
    let mut theme = GrafiaTheme::sonoma_default();
    assert_eq!(theme.ruler_tokens.thickness_dp, grafia::RULER_THICKNESS_DP);
    assert_eq!(
        theme.text_stream_tokens.caret_color,
        grafia::TEXT_CARET_COLOR_LIGHT
    );

    // Customize ruler & text-stream colors/thickness at runtime on GrafiaTheme
    theme.ruler_tokens.thickness_dp = 24.0;
    theme.ruler_tokens.hairline_color = grafia::RgbaColorToken::new(255, 0, 128, 255);
    theme.text_stream_tokens.kerning_negative_fill = grafia::RgbaColorToken::new(200, 10, 20, 180);
    theme.text_stream_tokens.kerning_positive_fill = grafia::RgbaColorToken::new(10, 200, 80, 180);

    let state = GrafiaCanvasState::new(theme.clone());
    assert_eq!(state.theme.ruler_tokens.thickness_dp, 24.0);
    assert_eq!(
        state.theme.ruler_tokens.hairline_color,
        grafia::RgbaColorToken::new(255, 0, 128, 255)
    );
    assert_eq!(
        state.theme.text_stream_tokens.kerning_negative_fill,
        grafia::RgbaColorToken::new(200, 10, 20, 180)
    );
}

#[test]
fn tc_maturity_007_preferences_hit_radii_kerning_steps_and_ghost_hint_widths_configurable() {
    let mut state = GrafiaCanvasState::new(GrafiaTheme::sonoma_default());
    let c0 = sample_closed_triangle_contour(0, 0.0, 0.0);
    state.glyph.contours = vec![c0.clone()];
    state.contours = vec![c0.nodes];
    state.sync_paths_from_contours();

    // 1. Custom `hit_test_radius_dp` and `segment_hit_tolerance_dp` on `state.preferences`
    state.preferences.hit_test_radius_dp = 14.0;
    state.preferences.segment_hit_tolerance_dp = 9.0;
    assert!((state.effective_pen_node_hit_radius_dp() - 14.0).abs() < 1e-5);
    assert!((state.effective_segment_hit_tolerance_dp() - 9.0).abs() < 1e-5);

    // 2. Custom keyboard kerning/spacing increments on `state.preferences`
    state.preferences.kerning_increment_low_upm = 2.5;
    state.preferences.kerning_increment_high_upm = 25.0;
    assert!((state.effective_kerning_step_upm(false) - 2.5).abs() < 1e-6);
    assert!((state.effective_kerning_step_upm(true) - 25.0).abs() < 1e-6);

    // 3. PostScript ghost hint respects custom positive `width_upm` and falls back to 20/21 only when `width_upm <= 0.0`
    let default_ghost = PostScriptHint {
        id: 1,
        axis: PostScriptHintAxis::Horizontal,
        origin_upm: 700.0,
        width_upm: 0.0,
        is_ghost: true,
        top_ghost: true,
        start_node: None,
        end_node: None,
        selected: false,
    };
    assert!((default_ghost.effective_width_upm() - 20.0).abs() < 1e-6);

    let custom_ghost = PostScriptHint {
        width_upm: 15.0,
        ..default_ghost
    };
    assert!((custom_ghost.effective_width_upm() - 15.0).abs() < 1e-6);
}

#[test]
fn tc_maturity_008_wasm32_browser_canvas_entrypoint_and_fs_guards() {
    let options = demo_app::build_native_options();
    assert_eq!(options.renderer, eframe::Renderer::Glow);
    assert!(!options.system_font_fallback);
    assert!(!options.persist_window);

    let mut state =
        common::canvas_with_closed_contour(&[(100.0, 100.0), (400.0, 100.0), (250.0, 500.0)]);
    let out = common::run_headless_canvas_frame(&mut state, 640.0, 480.0);
    assert!(!out.shapes.is_empty());
}

#[test]
fn tc_maturity_009_canvas_document_scene_v010_to_v012_curve_harmony_roundtrip() {
    let mut state =
        common::canvas_with_closed_contour(&[(100.0, 100.0), (400.0, 100.0), (250.0, 500.0)]);

    state.set_node_genius(NodeId(1), true);
    state.set_node_g3_lock(NodeId(2), true);
    common::assert_mutation_undo_redo_roundtrip(&mut state, |s| {
        s.set_smart_corner(SmartCornerSpec {
            node_id: NodeId(3),
            radius_upm: 24.0,
        });
    });
    state.set_reference_stems(vec![84.0, 68.0], 6.0);
    state.set_curved_trajectory_link(CurvedTrajectoryLink {
        node_id: NodeId(1),
        companion_layer_index: 0,
        control1: FontPoint::new(150.0, 260.0),
        control2: Some(FontPoint::new(220.0, 310.0)),
    });
    state.set_active_mirror_axis(ActiveMirrorAxis::Vertical { x_upm: 300.0 });
    state.set_round_overshoot_upm(Some(14.0));

    let scene = state.export_document_scene();
    assert!(scene.curve_harmony.genius_nodes.contains(&NodeId(1)));
    assert!(scene.curve_harmony.g3_nodes.contains(&NodeId(2)));
    assert_eq!(
        scene.curve_harmony.smart_corners,
        vec![SmartCornerSpec {
            node_id: NodeId(3),
            radius_upm: 24.0
        }]
    );
    assert_eq!(scene.curve_harmony.reference_stems_upm, vec![84.0, 68.0]);
    assert_eq!(scene.curve_harmony.curved_trajectories.len(), 1);
    assert_eq!(
        scene.curve_harmony.active_mirror_axis,
        ActiveMirrorAxis::Vertical { x_upm: 300.0 }
    );
    assert_eq!(scene.curve_harmony.round_overshoot_upm, Some(14.0));

    let mut restored = GrafiaCanvasState::new(GrafiaTheme::sonoma_default());
    restored.load_document_scene(scene.clone());
    assert!(restored.validate_invariants().is_ok());
    assert_eq!(
        restored.smart_corners(),
        &restored.curve_harmony.smart_corners
    );
    assert_eq!(restored.export_document_scene(), scene);
}

#[test]
fn tc_maturity_010_guided_showcase_chapters_and_dutch_coach_cards_complete() {
    use demo_app::demo_host_shell::{coach_card, tool_dutch_guide, DemoPreset, ShowcaseChapter};

    assert_eq!(ShowcaseChapter::ALL.len(), 6);
    assert_eq!(DemoPreset::ALL.len(), 36);
    assert_eq!(demo_app::TOOL_ICON_SPECS.len(), 20);
    for ch in ShowcaseChapter::ALL {
        assert!(!ch.tab_label().is_empty());
        assert!(!ch.summary().is_empty());
    }
    for (preset, _) in DemoPreset::ALL {
        let card = coach_card(preset);
        assert!(!card.short_button.is_empty());
        assert!(!card.title.is_empty());
        assert!(!card.what_is_it.is_empty());
        assert!(!card.why_host_plug.is_empty());
        assert!(!card.try_it_yourself.is_empty());
    }
    for (tool, _, _) in demo_app::TOOL_ICON_SPECS {
        let (nl_name, nl_how) = tool_dutch_guide(tool);
        assert!(!nl_name.is_empty() && !nl_how.is_empty());
    }

    let mut app = demo_app::GrafiaVectorCanvasDemo::new();
    assert_eq!(app.host.tour_index, 0);
    app.host.step_tour_next(&mut app.state, &mut app.theme);
    assert_eq!(app.host.tour_index, 1);
    app.host.step_tour_prev(&mut app.state, &mut app.theme);
    assert_eq!(app.host.tour_index, 0);
    let _ = CanvasTool::Select;
}

fn verify_mark_cloud_and_stroke_id_monotonicity(state: &GrafiaCanvasState) {
    use grafia::core::{
        compute_mark_cloud_contours, expand_stroked_contours_preview, AnchorId, CanvasAnchor,
        ContourStrokeStyle, GhostMarkPath, LineCapStyle, LineJoinStyle, StrokePosition,
    };
    use std::collections::HashSet;

    let anchors = vec![CanvasAnchor {
        id: AnchorId(1),
        name: "top".to_string(),
        position: FontPoint::new(250.0, 600.0),
        selected: true,
    }];
    let mark_a = GhostMarkPath {
        anchor_name: "_top".to_string(),
        contours: state.glyph.contours.clone(),
    };
    let mark_b = GhostMarkPath {
        anchor_name: "_top".to_string(),
        contours: state.glyph.contours.clone(),
    };
    let cloud = compute_mark_cloud_contours(&anchors, &[mark_a, mark_b], true);
    assert_eq!(cloud.len(), 2);
    let mut cloud_node_ids = HashSet::new();
    for c in &cloud {
        for n in &c.nodes {
            assert!(
                cloud_node_ids.insert(n.id),
                "Duplicate NodeId {:?} across mark cloud contours",
                n.id
            );
        }
    }

    let mut high_contour = state.glyph.contours[0].clone();
    for (i, n) in high_contour.nodes.iter_mut().enumerate() {
        n.id = NodeId(25_000 + i as u64);
    }
    let mut strokes = std::collections::BTreeMap::new();
    strokes.insert(
        high_contour.id,
        ContourStrokeStyle {
            width: 20.0,
            height: 20.0,
            position: StrokePosition::Center,
            cap_start: LineCapStyle::Round,
            cap_end: LineCapStyle::Round,
            join: LineJoinStyle::Miter,
        },
    );
    let preview = expand_stroked_contours_preview(&[high_contour], &strokes);
    assert!(!preview.is_empty());
    for c in &preview {
        for n in &c.nodes {
            assert!(n.id.0 > 25_002);
        }
    }
}

fn verify_dirty_scene_sanitization(state: &mut GrafiaCanvasState, c0: grafia::core::GlyphContour) {
    use grafia::core::{CanvasColorSublayer, ColorLayerPaint, GradientStop};
    use grafia::snapping::{Guide, GuideId};

    let mut dirty_scene = state.export_document_scene();
    dirty_scene.guides.local_guides = vec![
        Guide::new(GuideId(7), FontPoint::new(100.0, 200.0), 0.0, false),
        Guide::new(GuideId(7), FontPoint::new(f64::NAN, 200.0), 0.0, false),
    ];
    dirty_scene.color_layers = vec![CanvasColorSublayer {
        id: 1,
        name: "External".to_string(),
        contours: vec![c0],
        paint: ColorLayerPaint::LinearGradient {
            p0: FontPoint::new(0.0, 0.0),
            p1: FontPoint::new(100.0, 100.0),
            stops: vec![GradientStop {
                offset: 1.75,
                rgba: [255, 0, 0, 255],
            }],
            spread: grafia::core::GradientSpread::Pad,
        },
        opacity: 2.5,
        visible: true,
        locked: false,
    }];
    dirty_scene.hinting = sample_hinting_and_hoi_scene();
    dirty_scene.hinting.truetype_instructor_ppem = 0;
    dirty_scene.hinting.show_pixel_preview_ppem = Some(0);
    dirty_scene.hinting.postscript_hints[0].origin_upm = f64::NAN;
    dirty_scene.hinting.truetype_instructions[0].ppem = 0;
    dirty_scene.hinting.truetype_instructions[0].delta_pixels = f64::INFINITY;
    dirty_scene.curve_harmony.smart_corners = vec![
        SmartCornerSpec {
            node_id: NodeId(1),
            radius_upm: 20.0,
        },
        SmartCornerSpec {
            node_id: NodeId(1),
            radius_upm: f64::NAN,
        },
    ];
    state.load_document_scene(dirty_scene);
    assert!(state.validate_invariants().is_ok());
}

#[test]
fn tc_maturity_011_pre_flight_production_robustness_and_battery_pan_hardening() {
    use grafia::adapters::headless::rasterize_canvas_subregion;
    use grafia::core::{
        ContourStrokeStyle, LineCapStyle, LineJoinStyle, ScreenPoint, StrokePosition,
    };

    let theme = GrafiaTheme::sonoma_default();
    let mut state =
        common::canvas_with_closed_contour(&[(100.0, 100.0), (400.0, 100.0), (250.0, 500.0)]);

    let pix = rasterize_canvas_subregion(&state, &theme, 0, 0);
    assert_eq!((pix.width(), pix.height()), (1, 1));

    verify_mark_cloud_and_stroke_id_monotonicity(&state);

    let c0 = state.glyph.contours[0].clone();
    let mut c1 = c0.clone();
    c1.id = ContourId(1);
    for (i, n) in c1.nodes.iter_mut().enumerate() {
        n.id = NodeId(100 + i as u64);
        n.point.x.0 += 350.0;
    }
    state.contours = vec![c0.nodes.clone(), c1.nodes.clone()];
    state.glyph.contours = vec![c0.clone(), c1.clone()];
    state.sync_paths_from_contours();
    let style = ContourStrokeStyle {
        width: 16.0,
        height: 16.0,
        position: StrokePosition::Center,
        cap_start: LineCapStyle::Butt,
        cap_end: LineCapStyle::Butt,
        join: LineJoinStyle::Bevel,
    };
    state.set_contour_stroke(ContourId(0), style);
    state.set_contour_stroke(ContourId(1), style);
    state.selected_nodes = vec![c0.nodes[0].id];
    assert!(state.expand_selected_strokes());
    assert_eq!(state.glyph.contours.len(), 3);
    assert!(state.contour_strokes.contains_key(&ContourId(2)));

    state.select_state = SelectToolState::PanningViewport {
        last_screen: ScreenPoint::new(100.0, 100.0),
    };
    assert!(state.is_interactive_drag_active());
    let (eff_w, eff_h, scale) = state.effective_color_layer_pixmap_dimensions(3840, 2160);
    assert_eq!(eff_w, MAX_INTERACTIVE_DRAG_PIXMAP_DIM);
    assert!(eff_h < 2160 && scale < 1.0);
    state.select_state = SelectToolState::Idle;

    verify_dirty_scene_sanitization(&mut state, c0);
}
