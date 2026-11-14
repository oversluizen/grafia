//! Normative Verification Suite for `SPEC-014` (`grafia v0.9.1` — Senior Architectural Maturity).
//! Covers `TC-MATURITY-001`..`TC-MATURITY-005`.

use grafia::render::canvas_render_cache::get_or_upload_color_sublayers_texture;
use grafia::{
    AnchorId, CanvasAnchor, CanvasColorSublayer, CanvasDocumentScene, CanvasGuideSet,
    CanvasHintingAndHoiScene, CanvasLayerAttachments, ColorLayerPaint, ContourId,
    CustomReferenceFrame, FontPoint, Glyph, GlyphContour, GlyphMetrics, GradientExtend,
    GradientStop, GrafiaCanvasState, GrafiaTheme, Guide, GuideId, Modifiers, Node, NodeId,
    NodeType, PostScriptHint, PostScriptHintAxis, RectUpm, ReferenceFramePalette,
    RulerTickRangeUpm, SelectToolState, TrueTypeInstructionKind, TrueTypeRoundingGrid,
    TrueTypeVisualInstruction, WritingDirection, MAX_INTERACTIVE_DRAG_PIXMAP_DIM,
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
        }),
        writing_direction: WritingDirection::TopToBottom,
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
