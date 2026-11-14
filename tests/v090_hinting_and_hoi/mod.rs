//! Normative COTS Verification Suite for `SPEC-010` (`grafia v0.9.0` Slice 4: `TC-V090-008` .. `TC-V090-013`):
//! - `TC-V090-008`: PostScript Stem & Ghost Hint Bands, Polarity Badge, Telemetry Pill (`"20"` / `"21"`), Click Selection (`SelectionInfo::PostScriptHint`), `SelectionInfoAction::MutatePostScriptHint`, and `Delete`/`Backspace` Removal (`CanvasHostRequest::MutatePostScriptHint` + `"Delete PostScript Hint"` Undo)
//! - `TC-V090-009`: TrueType Visual Instructor (`CanvasTool::TrueTypeInstructor`) at `16 PPEM` (`62.5 UPM/px` at `1000 UPM`), `X` Key Projection Axis Toggle `(1.0, 0.0) <-> (0.0, 1.0)`, `Snap`/`Stem`/`Shift`/`Interpolate`/`Delta` Badges & Arrows, Click Selection (`SelectionInfo::TrueTypeInstruction`), `SelectionInfoAction::MutateTrueTypeInstruction`, and `CanvasHostRequest::MutateTrueTypeInstruction`
//! - `TC-V090-010`: Higher-Order Interpolation (`HOI`) 4-Layer Control Polygon (`show_hoi_diamonds = true`, `CanvasTool::Hoi`) & `HoiSiblingContours` (`prev`/`current`/`next` diamond markers, `SelectionInfo::HoiTiming`, `SelectionInfoAction::MutateHoiNode`, `CanvasHostRequest::MutateHoiLayerNode`)
//! - `TC-V090-011`: Property-Based `proptest` Color Gradient Stop (`2..8`), `ppem` (`8..144`), and Vertical Metrics (`advance_height` in `[200.0, 2000.0]`) Stability
//! - `TC-V090-012`: Stateful `arbtest` Color, Vertical Metrics & Hinting Undo/Redo Fuzzing + `state.validate_invariants().is_ok()`
//! - `TC-V090-013`: Pass-1 Pixel Preview Grid (`show_pixel_preview_ppem = Some(24)`) & Backdrop Ghost Contours (`backdrop_ghost_contours`) while `CanvasTool::Pen` is active without blocking `Pen` click/drag interaction (`SSIM >= 0.985`)

use crate::visual_pixel_diff_tests::{compute_ssim, SSIM_DEFAULT_WINDOW_SIZE};
use crate::{
    canvas_rect, pointer_press, pointer_release, rasterize_egui_widget_canvas, rect_contour,
    send_key,
};
use approx::assert_relative_eq;
use egui::Key;
use grafia::adapters::headless::{pixmap_to_rgba_image, rasterize_canvas_subregion};
use grafia::{
    CanvasColorSublayer, CanvasHostRequest, CanvasTool, ColorLayerPaint, CompanionMasterLayer,
    FontPoint, Glyph, GlyphContour, GlyphSlot, GradientSpread, GradientStop, GrafiaCanvasState,
    HoiCubicLayerSet, HoiSiblingContours, Node, NodeId, PostScriptHint, PostScriptHintAxis,
    PostScriptHintMutation, ScreenPoint, SelectionInfo, SelectionInfoAction,
    TrueTypeInstructionKind, TrueTypeInstructionMutation, TrueTypeRoundingGrid,
    TrueTypeVisualInstruction, WritingDirection,
};
use image::DynamicImage;
use proptest::prelude::*;

// ============================================================================
// TC-V090-008: PostScript Horizontal/Vertical Stem & Ghost Hint Bands
// ============================================================================
#[test]
fn tc_v090_008_postscript_stem_and_ghost_hint_bands_selection_mutation_and_delete() {
    let rect = canvas_rect(640.0, 480.0);
    let mut state = GrafiaCanvasState::init_glyph_a();
    state.viewport.zoom = 0.45;
    state.viewport.pan_offset = ScreenPoint::new(120.0, 400.0);
    state.show_metrics = false;
    state.show_grid = false;
    state.show_guides = false;
    state.show_selection_info_hud = false;

    let pix_without = rasterize_canvas_subregion(&state, &state.theme, 640, 480);

    let h_stem = PostScriptHint {
        id: 1,
        axis: PostScriptHintAxis::Horizontal,
        origin_upm: 0.0,
        width_upm: 72.0,
        is_ghost: false,
        top_ghost: false,
        start_node: Some(NodeId(1)),
        end_node: Some(NodeId(2)),
        selected: false,
    };
    let v_stem = PostScriptHint {
        id: 2,
        axis: PostScriptHintAxis::Vertical,
        origin_upm: 120.0,
        width_upm: 84.0,
        is_ghost: false,
        top_ghost: false,
        start_node: Some(NodeId(2)),
        end_node: Some(NodeId(3)),
        selected: false,
    };
    let top_ghost = PostScriptHint {
        id: 3,
        axis: PostScriptHintAxis::Horizontal,
        origin_upm: 700.0,
        width_upm: 21.0,
        is_ghost: true,
        top_ghost: true,
        start_node: Some(NodeId(3)),
        end_node: None,
        selected: false,
    };
    let bottom_ghost = PostScriptHint {
        id: 4,
        axis: PostScriptHintAxis::Horizontal,
        origin_upm: 0.0,
        width_upm: 20.0,
        is_ghost: true,
        top_ghost: false,
        start_node: Some(NodeId(1)),
        end_node: None,
        selected: false,
    };

    // 1. Telemetry Pill ("20" / "21") and Polarity Badge ("↑" / "↓") invariants
    assert_relative_eq!(top_ghost.effective_width_upm(), 21.0, epsilon = 1e-6);
    assert_relative_eq!(bottom_ghost.effective_width_upm(), 20.0, epsilon = 1e-6);
    assert_eq!(top_ghost.telemetry_pill_label(), "21");
    assert_eq!(bottom_ghost.telemetry_pill_label(), "20");
    assert_eq!(top_ghost.polarity_badge(), "↑");
    assert_eq!(bottom_ghost.polarity_badge(), "↓");
    assert_eq!(h_stem.telemetry_pill_label(), "0, 72");

    state.set_postscript_hints(vec![h_stem, v_stem, top_ghost]);
    assert_eq!(state.postscript_hints.len(), 3);

    // 2. Render horizontal/vertical stems and ghost band in Pass 4 (egui vs tiny-skia SSIM >= 0.985)
    let ts_pix = rasterize_canvas_subregion(&state, &state.theme, 640, 480);
    assert_ne!(
        ts_pix.data(),
        pix_without.data(),
        "PostScript stem & ghost hint bands must render in Pass 4"
    );
    let ts_img = DynamicImage::ImageRgba8(pixmap_to_rgba_image(ts_pix));
    let egui_img = rasterize_egui_widget_canvas(&mut state, 640, 480);
    let ssim = compute_ssim(&egui_img, &ts_img, SSIM_DEFAULT_WINDOW_SIZE);
    assert!(
        ssim >= 0.985,
        "TC-V090-008 egui vs tiny-skia PostScript hint bands SSIM must be >= 0.985, got {ssim:.6}"
    );

    // 3. Click-select horizontal stem hint on canvas (inside y in [0, 72])
    let hint1_scr = state.viewport.to_screen(FontPoint::new(300.0, 36.0));
    pointer_press(&mut state, rect, hint1_scr, false, false, false, false);
    pointer_release(&mut state, rect, hint1_scr, false, false, false, false);
    assert_eq!(state.selected_postscript_hint, Some(1));
    match state.selection_info() {
        SelectionInfo::PostScriptHint {
            hint_id,
            horizontal,
            origin_upm,
            width_upm,
            is_ghost,
            top_ghost,
        } => {
            assert_eq!(hint_id, 1);
            assert!(horizontal);
            assert_relative_eq!(origin_upm, 0.0, epsilon = 1e-6);
            assert_relative_eq!(width_upm, 72.0, epsilon = 1e-6);
            assert!(!is_ghost);
            assert!(!top_ghost);
        }
        other => panic!("Expected SelectionInfo::PostScriptHint, got {other:?}"),
    }

    // 4. SelectionInfoAction::MutatePostScriptHint emits MutatePostScriptHint(Update) + Undo/Redo
    let _ = state.drain_host_requests();
    assert!(
        state.apply_selection_info_action(SelectionInfoAction::MutatePostScriptHint {
            origin_upm: 12.0,
            width_upm: 78.0,
            top_ghost: false,
        })
    );
    assert_relative_eq!(state.postscript_hints[0].origin_upm, 12.0, epsilon = 1e-6);
    assert_relative_eq!(state.postscript_hints[0].width_upm, 78.0, epsilon = 1e-6);
    let reqs = state.drain_host_requests();
    assert!(reqs.contains(&CanvasHostRequest::MutatePostScriptHint(
        PostScriptHintMutation::Update {
            hint_id: 1,
            origin_upm: 12.0,
            width_upm: 78.0,
            top_ghost: false,
        }
    )));
    assert!(state.undo());
    assert_relative_eq!(state.postscript_hints[0].origin_upm, 0.0, epsilon = 1e-6);
    assert!(state.redo());
    assert_relative_eq!(state.postscript_hints[0].origin_upm, 12.0, epsilon = 1e-6);

    // 5. Select ghost hint (id = 3) and press Delete -> removes hint, emits Delete request, records "Delete PostScript Hint" Undo
    state.select_postscript_hint(Some(3));
    let _ = state.drain_host_requests();
    send_key(&mut state, Key::Delete, false, false, false, false);
    assert_eq!(state.postscript_hints.len(), 2);
    assert_eq!(
        state.history.last_undo_label(),
        Some("Delete PostScript Hint")
    );
    let del_reqs = state.drain_host_requests();
    assert!(del_reqs.contains(&CanvasHostRequest::MutatePostScriptHint(
        PostScriptHintMutation::Delete { hint_id: 3 }
    )));
    assert!(state.undo());
    assert_eq!(state.postscript_hints.len(), 3);
    assert!(state.validate_invariants().is_ok());
}

// ============================================================================
// TC-V090-009: TrueTypeInstructor PPEM Grid, X Axis Toggle & Instruction Badges
// ============================================================================
#[test]
fn tc_v090_009_truetype_instructor_ppem_grid_axis_toggle_and_instruction_badges() {
    let rect = canvas_rect(640.0, 480.0);
    let mut state = GrafiaCanvasState::init_glyph_a();
    state.viewport.zoom = 0.45;
    state.viewport.pan_offset = ScreenPoint::new(120.0, 400.0);
    state.show_metrics = false;
    state.show_grid = false;
    state.show_guides = false;
    state.show_selection_info_hud = false;

    // 1. Activate TrueTypeInstructor at 16 PPEM (1000 UPM / 16 PPEM = 62.5 UPM/px)
    state.activate_tool(CanvasTool::TrueTypeInstructor);
    state.set_truetype_instructor_ppem(16);
    assert_eq!(state.active_tool, CanvasTool::TrueTypeInstructor);
    assert_eq!(state.truetype_instructor_ppem, 16);
    assert_relative_eq!(state.truetype_upm_per_px(), 62.5, epsilon = 1e-6);

    // 2. X key toggles projection axis (1.0, 0.0) <-> (0.0, 1.0)
    assert_eq!(state.truetype_projection_axis(), (1.0, 0.0));
    send_key(&mut state, Key::X, false, false, false, false);
    assert_eq!(state.truetype_projection_axis(), (0.0, 1.0));
    send_key(&mut state, Key::X, false, false, false, false);
    assert_eq!(state.truetype_projection_axis(), (1.0, 0.0));

    // 3. Add Snap (Touch), Stem, Shift, Interpolate, and Delta instructions via keyboard shortcuts & API
    let node_ids: Vec<NodeId> = state.contours[0].iter().map(|n| n.id).collect();
    assert!(node_ids.len() >= 3);
    let _ = state.drain_host_requests();

    // Select node 0 and press 'A' -> Snap (Touch) instruction
    state.selected_nodes = vec![node_ids[0]];
    send_key(&mut state, Key::A, false, false, false, false);
    assert_eq!(state.truetype_instructions.len(), 1);
    assert_eq!(
        state.truetype_instructions[0].kind,
        TrueTypeInstructionKind::Snap
    );

    // Select nodes 0 and 1 and press 'S' -> Stem instruction, then 'E' -> Delta instruction
    state.selected_nodes = vec![node_ids[0], node_ids[1]];
    send_key(&mut state, Key::S, false, false, false, false);
    send_key(&mut state, Key::E, false, false, false, false);

    // Also add Shift and Interpolate instructions so all 5 badge/arrow types are exercised
    state.add_truetype_instruction(TrueTypeVisualInstruction {
        id: 100,
        kind: TrueTypeInstructionKind::Shift,
        source_node: Some(node_ids[0]),
        target_node: node_ids[2],
        ppem: 16,
        round_mode: TrueTypeRoundingGrid::ToGrid,
        stem_index: None,
        zone_index: None,
        delta_pixels: 0.0,
        x_axis: true,
        selected: false,
    });
    state.add_truetype_instruction(TrueTypeVisualInstruction {
        id: 101,
        kind: TrueTypeInstructionKind::Interpolate,
        source_node: Some(node_ids[1]),
        target_node: node_ids[2],
        ppem: 16,
        round_mode: TrueTypeRoundingGrid::Off,
        stem_index: None,
        zone_index: None,
        delta_pixels: 0.0,
        x_axis: false,
        selected: false,
    });
    assert_eq!(state.truetype_instructions.len(), 5);

    let add_reqs = state.drain_host_requests();
    assert!(add_reqs.iter().any(|r| matches!(
        r,
        CanvasHostRequest::MutateTrueTypeInstruction(TrueTypeInstructionMutation::Add(_))
    )));

    // 4. Render TrueTypeInstructor PPEM grid, grid-fitted outline & instruction badges/arrows (egui vs tiny-skia SSIM >= 0.985)
    let ts_pix = rasterize_canvas_subregion(&state, &state.theme, 640, 480);
    let ts_img = DynamicImage::ImageRgba8(pixmap_to_rgba_image(ts_pix));
    let egui_img = rasterize_egui_widget_canvas(&mut state, 640, 480);
    let ssim = compute_ssim(&egui_img, &ts_img, SSIM_DEFAULT_WINDOW_SIZE);
    assert!(
        ssim >= 0.985,
        "TC-V090-009 egui vs tiny-skia TrueTypeInstructor SSIM must be >= 0.985, got {ssim:.6}"
    );

    // 5. Click-select an instruction badge on canvas and mutate via SelectionInfoAction::MutateTrueTypeInstruction
    let target_pt = state.contours[0][0].point;
    let badge_scr = state.viewport.to_screen(target_pt);
    pointer_press(&mut state, rect, badge_scr, false, false, false, false);
    pointer_release(&mut state, rect, badge_scr, false, false, false, false);
    assert!(state.selected_truetype_instruction.is_some());

    let sel_id = state.selected_truetype_instruction.unwrap();
    match state.selection_info() {
        SelectionInfo::TrueTypeInstruction {
            instruction_id,
            ppem,
            ..
        } => {
            assert_eq!(instruction_id, sel_id);
            assert_eq!(ppem, 16);
        }
        other => panic!("Expected SelectionInfo::TrueTypeInstruction, got {other:?}"),
    }

    let _ = state.drain_host_requests();
    assert!(
        state.apply_selection_info_action(SelectionInfoAction::MutateTrueTypeInstruction {
            kind: TrueTypeInstructionKind::Stem,
            ppem: 24,
            round_mode: TrueTypeRoundingGrid::ToHalfGrid,
            stem_index: Some(2),
            zone_index: Some(1),
        })
    );
    let mut_reqs = state.drain_host_requests();
    assert!(mut_reqs.iter().any(|r| matches!(
        r,
        CanvasHostRequest::MutateTrueTypeInstruction(TrueTypeInstructionMutation::Update(_))
    )));
    assert!(state.undo());
    assert!(state.redo());
    assert!(state.validate_invariants().is_ok());
}

// ============================================================================
// TC-V090-010: HOI Control-Polygon Diamond Overlay & HoiSiblingContours
// ============================================================================
#[test]
fn tc_v090_010_hoi_control_polygon_diamond_overlay_and_sibling_contours() {
    let mut state = GrafiaCanvasState::init_glyph_a();
    state.viewport.zoom = 0.45;
    state.viewport.pan_offset = ScreenPoint::new(120.0, 400.0);
    state.show_metrics = false;
    state.show_grid = false;
    state.show_guides = false;
    state.show_selection_info_hud = false;

    let base_contour = state.glyph.contours[0].clone();
    let make_shifted_contour = |dx: f64, dy: f64| -> GlyphContour {
        let nodes = base_contour
            .nodes
            .iter()
            .map(|n| Node::new(n.id, n.point.x.0 + dx, n.point.y.0 + dy, n.node_type))
            .collect();
        GlyphContour::new(base_contour.id, nodes)
    };

    let c0 = make_shifted_contour(-60.0, -30.0);
    let c1 = make_shifted_contour(0.0, 45.0);
    let c2 = make_shifted_contour(70.0, 20.0);
    let c3 = make_shifted_contour(120.0, -25.0);

    // Load 4 compatible CompanionMasterLayers + HoiSiblingContours
    state.companion_master_layers = vec![
        CompanionMasterLayer {
            id: 1,
            name: "P0".into(),
            color_rgba: [0, 122, 255, 180],
            contours: vec![c0.nodes.clone()],
            closed_flags: vec![true],
            anchors: Vec::new(),
            editable: true,
        },
        CompanionMasterLayer {
            id: 2,
            name: "P1".into(),
            color_rgba: [52, 199, 89, 180],
            contours: vec![c1.nodes.clone()],
            closed_flags: vec![true],
            anchors: Vec::new(),
            editable: true,
        },
        CompanionMasterLayer {
            id: 3,
            name: "P2".into(),
            color_rgba: [255, 149, 0, 180],
            contours: vec![c2.nodes.clone()],
            closed_flags: vec![true],
            anchors: Vec::new(),
            editable: true,
        },
        CompanionMasterLayer {
            id: 4,
            name: "P3".into(),
            color_rgba: [175, 82, 222, 180],
            contours: vec![c3.nodes.clone()],
            closed_flags: vec![true],
            anchors: Vec::new(),
            editable: true,
        },
    ];
    state.set_hoi_sibling_contours(Some(HoiSiblingContours {
        layer_names: vec!["P0".into(), "P1".into(), "P2".into(), "P3".into()],
        contours_per_layer: vec![vec![c0], vec![c1], vec![c2], vec![c3]],
    }));
    state.set_hoi_layer_sets(vec![HoiCubicLayerSet {
        c1: FontPoint::new(0.3, 0.2),
        c2: FontPoint::new(0.7, 0.9),
    }]);

    let pix_without = rasterize_canvas_subregion(&state, &state.theme, 640, 480);

    state.set_show_hoi_diamonds(true);
    state.activate_tool(CanvasTool::Hoi);
    assert!(state.show_hoi_diamonds);
    assert_eq!(state.active_tool, CanvasTool::Hoi);

    // 1. Render 3-segment control-net polyline & prev/current/next diamond markers in Pass 4
    let ts_pix = rasterize_canvas_subregion(&state, &state.theme, 640, 480);
    assert_ne!(
        ts_pix.data(),
        pix_without.data(),
        "HOI control-polygon polylines and diamond markers must render when show_hoi_diamonds == true"
    );
    let ts_img = DynamicImage::ImageRgba8(pixmap_to_rgba_image(ts_pix));
    let egui_img = rasterize_egui_widget_canvas(&mut state, 640, 480);
    let ssim = compute_ssim(&egui_img, &ts_img, SSIM_DEFAULT_WINDOW_SIZE);
    assert!(
        ssim >= 0.985,
        "TC-V090-010 egui vs tiny-skia HOI diamond overlay SSIM must be >= 0.985, got {ssim:.6}"
    );

    // 2. SelectionInfo::HoiTiming in CanvasTool::Hoi
    match state.selection_info() {
        SelectionInfo::HoiTiming {
            c1_x,
            c1_y,
            c2_x,
            c2_y,
        } => {
            assert_relative_eq!(c1_x, 0.3, epsilon = 1e-6);
            assert_relative_eq!(c1_y, 0.2, epsilon = 1e-6);
            assert_relative_eq!(c2_x, 0.7, epsilon = 1e-6);
            assert_relative_eq!(c2_y, 0.9, epsilon = 1e-6);
        }
        other => panic!("Expected SelectionInfo::HoiTiming, got {other:?}"),
    }

    // 3. SelectionInfoAction::MutateHoiNode updates layer node + timing & emits CanvasHostRequest::MutateHoiLayerNode
    let first_nid = base_contour.nodes[0].id;
    let _ = state.drain_host_requests();
    assert!(
        state.apply_selection_info_action(SelectionInfoAction::MutateHoiNode {
            layer_index: 1,
            contour_index: 0,
            node_id: first_nid,
            new_pos: FontPoint::new(180.0, 260.0),
            c1: FontPoint::new(0.25, 0.35),
            c2: FontPoint::new(0.75, 0.85),
        })
    );
    let reqs = state.drain_host_requests();
    assert!(reqs.contains(&CanvasHostRequest::MutateHoiLayerNode {
        layer_index: 1,
        contour_index: 0,
        node_id: first_nid,
        new_pos: FontPoint::new(180.0, 260.0),
    }));
    assert!(state.undo());
    assert!(state.redo());
    assert!(state.validate_invariants().is_ok());
}

// ============================================================================
// TC-V090-011: Property-Based proptest Color Stops, PPEM Scales & Vertical Metrics
// ============================================================================
proptest! {
    #![proptest_config(ProptestConfig::with_cases(32))]
    #[test]
    fn tc_v090_011_proptest_color_stops_ppem_scales_and_vertical_metrics(
        stop_count in 2usize..8usize,
        ppem in 8u16..144u16,
        advance_height in 200.0f64..2000.0f64,
        zoom in 0.1f32..8.0f32,
    ) {
        let mut state = GrafiaCanvasState::init_glyph_a();
        state.viewport.zoom = zoom;

        let mut stops = Vec::with_capacity(stop_count);
        for idx in 0..stop_count {
            let t = (idx as f64) / ((stop_count - 1) as f64);
            stops.push(GradientStop {
                offset: t,
                rgba: [(idx as u8) * 30, 120, 200, 255],
            });
        }
        state.set_color_sublayers(vec![CanvasColorSublayer {
            id: 1,
            name: "FuzzGrad".into(),
            contours: vec![rect_contour(1, 100, 20.0, 20.0, 280.0, 280.0)],
            paint: ColorLayerPaint::LinearGradient {
                p0: FontPoint::new(20.0, 20.0),
                p1: FontPoint::new(280.0, 280.0),
                stops,
                spread: GradientSpread::Pad,
            },
            opacity: 1.0,
            visible: true,
            locked: false,
        }]);

        state.set_truetype_instructor_ppem(ppem);
        let upm_per_px = state.truetype_upm_per_px();
        prop_assert!(upm_per_px.is_finite() && upm_per_px > 0.0);

        let mut g = Glyph::new("V", 1000);
        g.width = 1000.0;
        g.height = advance_height;
        g.contours.push(rect_contour(0, 1, 100.0, 100.0, 900.0, 800.0));
        let mut slot0 = GlyphSlot::new(g.clone(), 0);
        slot0.advance_height = Some(advance_height);
        slot0.vert_origin_y = Some(880.0);
        let mut slot1 = GlyphSlot::new(g, -25);
        slot1.advance_height = Some(advance_height);
        slot1.vert_origin_y = Some(880.0);

        state.set_writing_direction(WritingDirection::TopToBottom);
        state.set_text_stream_slots(vec![slot0, slot1], 0);
        prop_assert!(state.text_stream.slots[1].origin_y_upm.is_finite());
        prop_assert_eq!(state.validate_invariants(), Ok(()));
    }
}

// ============================================================================
// TC-V090-012: Stateful arbtest Color, Vertical Metrics & Hinting Undo/Redo
// ============================================================================
#[test]
fn tc_v090_012_arbtest_stateful_color_vertical_metrics_and_hinting_undo_redo() {
    arbtest::arbtest(|u| {
        let mut state = GrafiaCanvasState::init_glyph_a();
        state.set_writing_direction(WritingDirection::TopToBottom);
        let mut g = Glyph::new("V", 1000);
        g.width = 1000.0;
        g.height = 1000.0;
        g.contours
            .push(rect_contour(0, 1, 100.0, 100.0, 900.0, 800.0));
        let mut slot = GlyphSlot::new(g, 0);
        slot.advance_height = Some(1000.0);
        slot.vert_origin_y = Some(880.0);
        state.set_text_stream_slots(vec![slot], 0);

        state.set_color_sublayers(vec![CanvasColorSublayer {
            id: 1,
            name: "G0".into(),
            contours: vec![rect_contour(1, 10, 40.0, 40.0, 240.0, 240.0)],
            paint: ColorLayerPaint::LinearGradient {
                p0: FontPoint::new(40.0, 40.0),
                p1: FontPoint::new(240.0, 240.0),
                stops: vec![
                    GradientStop {
                        offset: 0.0,
                        rgba: [255, 0, 0, 255],
                    },
                    GradientStop {
                        offset: 0.5,
                        rgba: [0, 255, 0, 255],
                    },
                    GradientStop {
                        offset: 1.0,
                        rgba: [0, 0, 255, 255],
                    },
                ],
                spread: GradientSpread::Pad,
            },
            opacity: 1.0,
            visible: true,
            locked: false,
        }]);
        state.select_color_sublayer(Some(0));
        state.select_gradient_stop(Some((0, 1)));

        state.set_postscript_hints(vec![PostScriptHint {
            id: 1,
            axis: PostScriptHintAxis::Horizontal,
            origin_upm: 100.0,
            width_upm: 60.0,
            is_ghost: false,
            top_ghost: false,
            start_node: None,
            end_node: None,
            selected: false,
        }]);

        let steps: usize = u.int_in_range(8..=20)?;
        for _ in 0..steps {
            let op: u8 = u.int_in_range(0..=5)?;
            match op {
                0 => {
                    let pct = f64::from(u.int_in_range(5..=95u8)?);
                    state.select_gradient_stop(Some((0, 1)));
                    let _ = state.apply_selection_info_action(
                        SelectionInfoAction::MutateGradientStop {
                            offset_pct: pct,
                            rgba: [100, 180, 220, 255],
                        },
                    );
                }
                1 => {
                    let vis = state.color_sublayers[0].visible;
                    let _ = state.set_color_sublayer_visibility(0, !vis);
                }
                2 => {
                    state.activate_tool(CanvasTool::Text);
                    send_key(&mut state, Key::CloseBracket, false, false, false, false);
                }
                3 => {
                    if !state.postscript_hints.is_empty() {
                        let hid = state.postscript_hints[0].id;
                        state.select_postscript_hint(Some(hid));
                        let orig = f64::from(u.int_in_range(10..=400u16)?);
                        let w = f64::from(u.int_in_range(20..=120u16)?);
                        let _ = state.apply_selection_info_action(
                            SelectionInfoAction::MutatePostScriptHint {
                                origin_upm: orig,
                                width_upm: w,
                                top_ghost: false,
                            },
                        );
                    }
                }
                4 => {
                    if state.undo() {
                        assert!(state.validate_invariants().is_ok());
                        assert!(state.redo());
                    }
                }
                _ => {
                    let _ = state.undo();
                }
            }
            assert!(state.validate_invariants().is_ok());
        }
        Ok(())
    })
    .budget_ms(250);
}

// ============================================================================
// TC-V090-013: show_pixel_preview_ppem & backdrop_ghost_contours Pass-1 Overlays
// ============================================================================
#[test]
fn tc_v090_013_pixel_preview_ppem_and_backdrop_ghost_contours_pass1_overlays() {
    let rect = canvas_rect(640.0, 480.0);
    let mut state = GrafiaCanvasState::init_glyph_a();
    state.viewport.zoom = 0.45;
    state.viewport.pan_offset = ScreenPoint::new(140.0, 390.0);
    state.show_metrics = false;
    state.show_grid = false;
    state.show_guides = false;
    state.show_selection_info_hud = false;

    state.activate_tool(CanvasTool::Pen);
    let pix_without = rasterize_canvas_subregion(&state, &state.theme, 640, 480);

    // Enable 24 PPEM Pass-1 pixel preview and load 2 backdrop_ghost_contours
    state.set_pixel_preview_ppem(Some(24));
    state.set_backdrop_ghost_contours(vec![
        rect_contour(10, 500, -140.0, 0.0, -30.0, 680.0),
        rect_contour(11, 510, 640.0, 0.0, 760.0, 680.0),
    ]);
    assert_eq!(state.show_pixel_preview_ppem, Some(24));
    assert_eq!(state.backdrop_ghost_contours.len(), 2);

    // 1. Pass 1 renders 24 PPEM raster grid + ghost silhouettes with egui vs tiny-skia SSIM >= 0.985
    let ts_pix = rasterize_canvas_subregion(&state, &state.theme, 640, 480);
    assert_ne!(
        ts_pix.data(),
        pix_without.data(),
        "show_pixel_preview_ppem and backdrop_ghost_contours must render in Pass 1"
    );
    let ts_img = DynamicImage::ImageRgba8(pixmap_to_rgba_image(ts_pix));
    let egui_img = rasterize_egui_widget_canvas(&mut state, 640, 480);
    let ssim = compute_ssim(&egui_img, &ts_img, SSIM_DEFAULT_WINDOW_SIZE);
    assert!(
        ssim >= 0.985,
        "TC-V090-013 egui vs tiny-skia Pass-1 pixel preview & ghost contours SSIM must be >= 0.985, got {ssim:.6}"
    );

    // 2. Active CanvasTool::Pen click interaction is NOT blocked by Pass-1 overlays
    let initial_contour_count = state.contours.len();
    let p1 = state.viewport.to_screen(FontPoint::new(120.0, 520.0));
    let p2 = state.viewport.to_screen(FontPoint::new(260.0, 520.0));
    pointer_press(&mut state, rect, p1, false, false, false, false);
    pointer_release(&mut state, rect, p1, false, false, false, false);
    pointer_press(&mut state, rect, p2, false, false, false, false);
    pointer_release(&mut state, rect, p2, false, false, false, false);
    assert_eq!(state.contours.len(), initial_contour_count + 1);
    assert_eq!(state.contours.last().unwrap().len(), 2);
    assert!(state.validate_invariants().is_ok());
}

// ============================================================================
// Adversarial Audit (a): Degenerate Gradients (Zero-Length, Zero-Radius, Empty Stops)
// ============================================================================
#[test]
fn tc_v090_014_adversarial_degenerate_gradients_zero_length_zero_radius_empty_stops() {
    let mut state = GrafiaCanvasState::init_glyph_a();
    state.viewport.zoom = 0.5;
    state.viewport.pan_offset = ScreenPoint::new(100.0, 400.0);
    state.show_metrics = false;
    state.show_grid = false;
    state.show_guides = false;
    state.show_selection_info_hud = false;

    let zero_len_linear = CanvasColorSublayer {
        id: 1,
        name: "ZeroLenLinear".into(),
        contours: vec![rect_contour(1, 100, 40.0, 40.0, 220.0, 220.0)],
        paint: ColorLayerPaint::LinearGradient {
            p0: FontPoint::new(100.0, 100.0),
            p1: FontPoint::new(100.0, 100.0),
            stops: vec![
                GradientStop {
                    offset: 0.0,
                    rgba: [220, 40, 60, 255],
                },
                GradientStop {
                    offset: 1.0,
                    rgba: [40, 120, 240, 255],
                },
            ],
            spread: GradientSpread::Repeat,
        },
        opacity: 1.0,
        visible: true,
        locked: false,
    };

    let zero_rad_radial = CanvasColorSublayer {
        id: 2,
        name: "ZeroRadRadial".into(),
        contours: vec![rect_contour(2, 110, 240.0, 40.0, 420.0, 220.0)],
        paint: ColorLayerPaint::RadialGradient {
            c0: FontPoint::new(320.0, 120.0),
            r0: 0.0,
            c1: FontPoint::new(320.0, 120.0),
            r1: 0.0,
            stops: vec![
                GradientStop {
                    offset: 0.0,
                    rgba: [60, 200, 100, 255],
                },
                GradientStop {
                    offset: 1.0,
                    rgba: [240, 180, 40, 255],
                },
            ],
            spread: GradientSpread::Reflect,
        },
        opacity: 1.0,
        visible: true,
        locked: false,
    };

    let empty_stops_sublayer = CanvasColorSublayer {
        id: 3,
        name: "EmptyStops".into(),
        contours: vec![rect_contour(3, 120, 100.0, 240.0, 340.0, 400.0)],
        paint: ColorLayerPaint::LinearGradient {
            p0: FontPoint::new(100.0, 240.0),
            p1: FontPoint::new(340.0, 400.0),
            stops: Vec::new(),
            spread: GradientSpread::Pad,
        },
        opacity: 1.0,
        visible: true,
        locked: false,
    };

    state.set_color_sublayers(vec![zero_len_linear, zero_rad_radial, empty_stops_sublayer]);
    for sub_idx in 0..3 {
        state.select_color_sublayer(Some(sub_idx));
        let ts_pix = rasterize_canvas_subregion(&state, &state.theme, 480, 360);
        let ts_img = DynamicImage::ImageRgba8(pixmap_to_rgba_image(ts_pix));
        let egui_img = rasterize_egui_widget_canvas(&mut state, 480, 360);
        let ssim = compute_ssim(&egui_img, &ts_img, SSIM_DEFAULT_WINDOW_SIZE);
        assert!(
            ssim >= 0.985,
            "Degenerate gradient sublayer {sub_idx} must render cleanly without NaN/Inf (SSIM={ssim:.6})"
        );
    }
    assert!(state.validate_invariants().is_ok());
}

// ============================================================================
// Adversarial Audit (b): Multi-Window / Dual-State CanvasRenderCache Stability
// ============================================================================
#[test]
fn tc_v090_015_adversarial_multi_window_dual_state_canvas_render_cache_ping_pong_stability() {
    let rect = canvas_rect(320.0, 240.0);
    let ctx = egui::Context::default();

    let mut state_a = GrafiaCanvasState::init_glyph_a();
    state_a.background_image = Some(grafia::CanvasBackgroundImage::new(
        [200_u8, 60, 80, 255].repeat(8 * 8),
        8,
        8,
    ));
    state_a.set_color_sublayers(vec![CanvasColorSublayer {
        id: 1,
        name: "SubA".into(),
        contours: vec![rect_contour(1, 10, 40.0, 40.0, 240.0, 240.0)],
        paint: ColorLayerPaint::Solid {
            rgba: [220, 50, 60, 255],
        },
        opacity: 1.0,
        visible: true,
        locked: false,
    }]);

    let mut state_b = GrafiaCanvasState::init_glyph_a();
    state_b.background_image = Some(grafia::CanvasBackgroundImage::new(
        [40_u8, 160, 220, 255].repeat(8 * 8),
        8,
        8,
    ));
    state_b.set_color_sublayers(vec![CanvasColorSublayer {
        id: 2,
        name: "SubB".into(),
        contours: vec![rect_contour(2, 20, 80.0, 80.0, 280.0, 280.0)],
        paint: ColorLayerPaint::Solid {
            rgba: [40, 140, 240, 255],
        },
        opacity: 0.9,
        visible: true,
        locked: false,
    }]);

    // Render alternating frames: state_a (0), state_b (1), state_a (2), state_b (3), state_a (4), state_b (5)
    for frame_idx in 0..6 {
        let raw = egui::RawInput {
            screen_rect: Some(rect),
            time: Some(f64::from(frame_idx)),
            ..Default::default()
        };
        let mut out = ctx.run_ui(raw, |ui| {
            let active_state = if frame_idx % 2 == 0 {
                &mut state_a
            } else {
                &mut state_b
            };
            ui.add(
                grafia::GrafiaCanvasWidget::from_state(active_state)
                    .with_size(egui::Vec2::new(320.0, 240.0)),
            );
        });
        if frame_idx >= 2 {
            assert!(
                out.textures_delta.set.is_empty(),
                "Alternating dual-state frame {frame_idx} must perform ZERO texture re-uploads (got {} uploads)",
                out.textures_delta.set.len()
            );
        }
        out.textures_delta.clear();
    }
}

// ============================================================================
// Adversarial Audit (c): Extreme TrueTypeInstructor (ppem = 0) & PostScriptHint (width = 0.0)
// ============================================================================
#[test]
fn tc_v090_016_adversarial_extreme_truetype_zero_ppem_and_zero_width_postscript_hint() {
    let rect = canvas_rect(480.0, 360.0);
    let mut state = GrafiaCanvasState::init_glyph_a();
    state.viewport.zoom = 0.45;
    state.viewport.pan_offset = ScreenPoint::new(120.0, 300.0);
    state.show_metrics = false;
    state.show_grid = false;
    state.show_guides = false;
    state.show_selection_info_hud = false;

    // 1. Direct truetype_upm_per_px(1000, 0) and set_truetype_instructor_ppem(0) must clamp safely without division-by-zero
    let step_zero = grafia::truetype_upm_per_px(1000, 0);
    assert!(step_zero.is_finite() && step_zero > 0.0);
    assert_relative_eq!(step_zero, 1000.0, epsilon = 1e-6);

    let fitted_zero = grafia::grid_fit_contour_nodes(&state.contours[0], 1000, 0, true, &[]);
    assert_eq!(fitted_zero.len(), state.contours[0].len());
    assert!(fitted_zero
        .iter()
        .all(|n| n.point.x.0.is_finite() && n.point.y.0.is_finite()));

    let full_box = vec![rect_contour(1, 1, 50.0, 50.0, 950.0, 950.0)];
    let cells_zero = grafia::compute_pixel_preview_cells_upm(&full_box, 1000, 0);
    assert_eq!(cells_zero.len(), 1);

    state.activate_tool(CanvasTool::TrueTypeInstructor);
    state.set_truetype_instructor_ppem(0);
    assert!(state.truetype_instructor_ppem >= 1);
    assert!(state.truetype_upm_per_px().is_finite() && state.truetype_upm_per_px() > 0.0);

    // 2. PostScriptHint with width_upm = 0.0 must render and hit-test without panic
    state.activate_tool(CanvasTool::Select);
    state.set_postscript_hints(vec![PostScriptHint {
        id: 99,
        axis: PostScriptHintAxis::Horizontal,
        origin_upm: 200.0,
        width_upm: 0.0,
        is_ghost: false,
        top_ghost: false,
        start_node: None,
        end_node: None,
        selected: false,
    }]);

    let ts_pix = rasterize_canvas_subregion(&state, &state.theme, 480, 360);
    let ts_img = DynamicImage::ImageRgba8(pixmap_to_rgba_image(ts_pix));
    let egui_img = rasterize_egui_widget_canvas(&mut state, 480, 360);
    let ssim = compute_ssim(&egui_img, &ts_img, SSIM_DEFAULT_WINDOW_SIZE);
    assert!(
        ssim >= 0.985,
        "Zero-width PostScriptHint must render with SSIM >= 0.985, got {ssim:.6}"
    );

    let click_scr = state.viewport.to_screen(FontPoint::new(250.0, 200.0));
    pointer_press(&mut state, rect, click_scr, false, false, false, false);
    pointer_release(&mut state, rect, click_scr, false, false, false, false);
    assert_eq!(state.selected_postscript_hint, Some(99));
    assert!(state.validate_invariants().is_ok());
}
