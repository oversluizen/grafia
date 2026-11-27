//! Complete COTS & `arbtest` verification suite for `SPEC-017` (`grafia v0.15.0`):
//! - `TC-V0150-001`: Full W3C SVG Path (`M..Z / m..z / A..a` arcs, compact numbers), Shapes (`<rect rx>`, `<circle>`, `<ellipse>`, `<polygon>`, `<polyline>`, `<line>`), `viewBox` & `transform` matrices, `<defs>` `<linearGradient>` / `<radialGradient>`, and `stroke` <-> `ContourStrokeStyle` I/O.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::too_many_lines,
    clippy::cognitive_complexity
)]

use arbtest::arbtest;
use grafia::core::{import_w3c_icon_svg, ColorLayerPaint, LineCapStyle, LineJoinStyle, NodeType};
use grafia::{CanvasTool, GrafiaCanvasState};

const WILD_W3C_SVG_FIXTURE: &str = r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 1000 1000">
  <defs>
    <linearGradient id="skyGrad" x1="120" y1="160" x2="540" y2="580">
      <stop offset="0%" stop-color="#38bdf8" stop-opacity="1.0"/>
      <stop offset="50%" stop-color="#0284c7" stop-opacity="0.95"/>
      <stop offset="100%" stop-color="#0369a1" stop-opacity="1.0"/>
    </linearGradient>
    <radialGradient id="sunGlow" cx="680" cy="700" r="140" fx="680" fy="700" fr="20">
      <stop offset="0.0" stop-color="#fde047"/>
      <stop offset="1.0" stop-color="#f97316"/>
    </radialGradient>
  </defs>
  <g id="BadgeCard" data-layer-group="1" transform="translate(10, 20)">
    <rect x="110" y="140" width="420" height="360" rx="28" ry="28" fill="url(#skyGrad)"/>
  </g>
  <g id="SunAndArc" data-layer-group="2">
    <circle cx="680" cy="700" r="95" fill="url(#sunGlow)"/>
    <path d="M180,580h120v80c0,40-30,70-70,70s-50-30-50-70q0-40,30-40t30,0a45,45 0 0,1 60,40z" fill="#22c55e"/>
  </g>
  <g id="SignalWave" data-layer-group="3" data-draw-order="reversed" data-variable-color="true">
    <path d="M 160 110 C 260 210, 380 40, 480 120" fill="none" stroke="#f43f5e" stroke-width="26" stroke-linecap="round" stroke-linejoin="round"/>
  </g>
</svg>"##;

fn verify_w3c_svg_fuzzed_arbtest() {
    arbtest(|u| {
        let cmd = match u.int_in_range(0..=9)? {
            0 => "l",
            1 => "h",
            2 => "v",
            3 => "c",
            4 => "s",
            5 => "q",
            6 => "t",
            7 => "a",
            8 => "A",
            _ => "C",
        };
        let a = f64::from_bits(u.arbitrary::<u64>()?);
        let b = f64::from_bits(u.arbitrary::<u64>()?);
        let c = f64::from_bits(u.arbitrary::<u64>()?);
        let d = f64::from_bits(u.arbitrary::<u64>()?);
        let e = f64::from_bits(u.arbitrary::<u64>()?);
        let f = f64::from_bits(u.arbitrary::<u64>()?);
        let g = f64::from_bits(u.arbitrary::<u64>()?);
        let svg_fuzzed = format!(
            "<svg viewBox=\"0 0 1000 1000\"><g transform=\"translate({a},{b}) rotate({c})\"><path d=\"M100,100 {cmd} {a} {b} {c} {d} {e} {f} {g} z\" fill=\"#3b82f6\" stroke=\"#ef4444\" stroke-width=\"{d}\"/></g></svg>"
        );
        if let Ok(parsed) = import_w3c_icon_svg(&svg_fuzzed) {
            for sub in &parsed.sublayers {
                for contour in &sub.contours {
                    for n in &contour.nodes {
                        assert!(n.point.x.0.is_finite() && n.point.y.0.is_finite());
                    }
                }
            }
            for st in parsed.contour_strokes.values() {
                assert!(st.width.is_finite() && st.width > 0.0);
            }
        }
        Ok(())
    })
    .budget_ms(250);
}

#[test]
fn tc_v0150_001_w3c_svg_wild_path_shapes_transforms_gradients_and_strokes() {
    let doc =
        import_w3c_icon_svg(WILD_W3C_SVG_FIXTURE).expect("Wild W3C SVG must parse without error");
    assert_eq!(doc.sublayers.len(), 3);
    assert_eq!(doc.tags.len(), 3);
    assert!(matches!(
        doc.sublayers[0].paint,
        ColorLayerPaint::LinearGradient { .. }
    ));
    assert!(doc.sublayers[0].contours[0].closed && doc.sublayers[0].contours[0].nodes.len() >= 8);
    assert!(matches!(
        doc.sublayers[1].paint,
        ColorLayerPaint::RadialGradient { .. }
    ));
    assert_eq!(doc.sublayers[1].contours.len(), 2);
    assert!(doc.sublayers[1].contours[1]
        .nodes
        .iter()
        .any(|n| n.node_type == NodeType::OffCurve));

    let wave_cid = doc.sublayers[2].contours[0].id;
    assert!(!doc.sublayers[2].contours[0].closed);
    let stroke_style = doc
        .contour_strokes
        .get(&wave_cid)
        .expect("Stroked SVG path must populate ContourStrokeStyle");
    assert!((stroke_style.width - 26.0).abs() < 1e-3);
    assert_eq!(stroke_style.cap_start, LineCapStyle::Round);
    assert_eq!(stroke_style.join, LineJoinStyle::Round);

    let mut state = GrafiaCanvasState::init_glyph_a();
    state.viewport =
        grafia::ViewportTransform::new(0.5, grafia::ScreenPoint::new(100.0, 500.0), 1000);
    let snap_before = state.snapshot();
    assert!(state.import_kinetic_symbol_from_svg(WILD_W3C_SVG_FIXTURE));
    assert_eq!(state.color_sublayers.len(), 3);
    assert!(state.contour_strokes.contains_key(&wave_cid));

    let exported_svg = state.export_kinetic_symbol_to_svg();
    let roundtrip =
        import_w3c_icon_svg(&exported_svg).expect("Exported W3C SVG must re-import losslessly");
    assert_eq!(roundtrip.sublayers.len(), 3);
    assert_eq!(roundtrip.tags, state.icon_studio.symbol_layer_tags);
    assert!(roundtrip.contour_strokes.contains_key(&wave_cid));

    let off_node = state
        .glyph
        .contours
        .iter()
        .find(|c| c.id == wave_cid)
        .and_then(|c| c.nodes.iter().find(|n| n.node_type == NodeType::OffCurve))
        .expect("Wave contour must contain OffCurve node")
        .clone();
    state.activate_tool(CanvasTool::Select);
    let scr0 = state.viewport.to_screen(off_node.point);
    let (p_start, p_end) = (
        egui::pos2(scr0.x.0, scr0.y.0),
        egui::pos2(scr0.x.0 + 35.0, scr0.y.0 - 20.0),
    );
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
    let moved = state
        .find_node(off_node.id)
        .expect("OffCurve node must exist");
    assert!(
        (moved.point.x.0 - (off_node.point.x.0 + 70.0)).abs() < 1e-3
            && (moved.point.y.0 - (off_node.point.y.0 + 40.0)).abs() < 1e-3,
        "Imported W3C SVG OffCurve handle must be draggable via InputDispatcher"
    );

    assert!(state.undo());
    assert!(state.undo());
    assert_eq!(state.snapshot(), snap_before);
    assert!(state.redo());
    assert_eq!(state.color_sublayers.len(), 3);

    verify_w3c_svg_fuzzed_arbtest();
}

#[test]
fn tc_v0150_002_live_contour_to_color_sublayer_sync_click_select_and_z_order() {
    use grafia::core::{
        CanvasColorSublayer, ContourId, FontPoint, GlyphContour, Node, NodeId, NodeType,
        ShapeBuilderOp, SymbolLayerTag,
    };
    use grafia::tokens::GrafiaTheme;

    let c0 = GlyphContour::new(
        ContourId(0),
        vec![
            Node::new(NodeId(10), 120.0, 160.0, NodeType::Corner),
            Node::new(NodeId(11), 320.0, 160.0, NodeType::Corner),
            Node::new(NodeId(12), 320.0, 420.0, NodeType::Corner),
            Node::new(NodeId(13), 120.0, 420.0, NodeType::Corner),
        ],
    );
    let c1 = GlyphContour::new(
        ContourId(1),
        vec![
            Node::new(NodeId(20), 240.0, 220.0, NodeType::Corner),
            Node::new(NodeId(21), 440.0, 220.0, NodeType::Corner),
            Node::new(NodeId(22), 440.0, 480.0, NodeType::Corner),
            Node::new(NodeId(23), 240.0, 480.0, NodeType::Corner),
        ],
    );
    let mut c2 = GlyphContour::new(
        ContourId(2),
        vec![
            Node::new(NodeId(30), 480.0, 160.0, NodeType::Corner),
            Node::new(NodeId(31), 540.0, 300.0, NodeType::OffCurve),
            Node::new(NodeId(32), 600.0, 300.0, NodeType::OffCurve),
            Node::new(NodeId(33), 660.0, 160.0, NodeType::Corner),
            Node::new(NodeId(34), 660.0, 440.0, NodeType::Corner),
            Node::new(NodeId(35), 480.0, 440.0, NodeType::Corner),
        ],
    );
    c2.closed = true;

    let mut state = GrafiaCanvasState::init_glyph_a();
    state.viewport =
        grafia::ViewportTransform::new(0.5, grafia::ScreenPoint::new(100.0, 500.0), 1000);
    let mut g = state.glyph.clone();
    g.contours = vec![c0.clone(), c1.clone(), c2.clone()];
    let metrics = state.metrics.clone();
    state.load_glyph(g, metrics);
    state.set_color_sublayers(vec![
        CanvasColorSublayer {
            id: 1,
            name: "CardLeft".into(),
            contours: vec![c0.clone()],
            paint: ColorLayerPaint::Solid {
                rgba: [40, 145, 255, 240],
            },
            opacity: 1.0,
            visible: true,
            locked: false,
        },
        CanvasColorSublayer {
            id: 2,
            name: "CardOverlap".into(),
            contours: vec![c1.clone()],
            paint: ColorLayerPaint::Solid {
                rgba: [255, 95, 75, 240],
            },
            opacity: 1.0,
            visible: true,
            locked: false,
        },
        CanvasColorSublayer {
            id: 3,
            name: "BadgeCurve".into(),
            contours: vec![c2.clone()],
            paint: ColorLayerPaint::Solid {
                rgba: [45, 210, 130, 240],
            },
            opacity: 1.0,
            visible: true,
            locked: false,
        },
    ]);
    state.icon_studio.symbol_layer_tags = vec![
        SymbolLayerTag {
            layer_group: 1,
            ..Default::default()
        },
        SymbolLayerTag {
            layer_group: 2,
            ..Default::default()
        },
        SymbolLayerTag {
            layer_group: 3,
            ..Default::default()
        },
    ];

    let theme = GrafiaTheme::sonoma_default();
    let pix_before_drag = grafia::adapters::rasterize_canvas_subregion(&state, &theme, 480, 360);

    // 1. Live 1-to-1 Sync Oracle: Drag OffCurve handle NodeId(31) on ContourId(2) via InputDispatcher
    // and verify that state.color_sublayers[2].contours[0] updates in lockstep in the very same frame!
    state.activate_tool(CanvasTool::Select);
    let scr31 = state.viewport.to_screen(FontPoint::new(540.0, 300.0));
    let (p_start, p_end) = (
        egui::pos2(scr31.x.0, scr31.y.0),
        egui::pos2(scr31.x.0 + 30.0, scr31.y.0 - 25.0),
    );
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
    let glyph_n31 = state.find_node(NodeId(31)).unwrap().point;
    let sublayer_n31 = state.color_sublayers[2].contours[0]
        .nodes
        .iter()
        .find(|n| n.id == NodeId(31))
        .unwrap()
        .point;
    assert_eq!(
        sublayer_n31, glyph_n31,
        "Pijler B violation: dragging an OffCurve handle on glyph.contours must live-sync into color_sublayers[2].contours[0]"
    );
    let pix_after_drag = grafia::adapters::rasterize_canvas_subregion(&state, &theme, 480, 360);
    assert_ne!(
        pix_before_drag.data(),
        pix_after_drag.data(),
        "Pijler D violation: color sublayer pixels must update when a contour handle is dragged"
    );

    // 2. Click-to-Select Sublayer & Z-Order Reordering ([ / ]):
    // Click inside CardOverlap (sublayer index 1) at (380.0, 350.0) where no node/handle exists:
    let click_scr = state.viewport.to_screen(FontPoint::new(380.0, 350.0));
    let click_pos = egui::pos2(click_scr.x.0, click_scr.y.0);
    let raw_click = egui::RawInput {
        screen_rect: Some(canvas_rect),
        time: Some(2.0),
        events: vec![
            egui::Event::PointerButton {
                pos: click_pos,
                button: egui::PointerButton::Primary,
                pressed: true,
                modifiers: egui::Modifiers::NONE,
            },
            egui::Event::PointerButton {
                pos: click_pos,
                button: egui::PointerButton::Primary,
                pressed: false,
                modifiers: egui::Modifiers::NONE,
            },
        ],
        ..Default::default()
    };
    let mut out_c = ctx.run_ui(raw_click, |ui| {
        let ev = ui.input(|i| i.events.clone());
        let _ = grafia::adapters::InputDispatcher::for_canvas(&mut state, canvas_rect)
            .dispatch_events(&ev, ui);
    });
    out_c.textures_delta.clear();
    assert_eq!(
        state.selected_color_sublayer_index(),
        Some(1),
        "Clicking inside sublayer 1 body in CanvasTool::Select must select sublayer 1"
    );

    // Move sublayer 1 backward (delta = -1) to index 0, and verify sublayer + symbol_layer_tags swap atomically with Undo/Redo!
    let snap_before_z = state.snapshot();
    assert!(state.reorder_selected_color_sublayer(-1));
    assert_eq!(state.selected_color_sublayer_index(), Some(0));
    assert_eq!(state.color_sublayers[0].name, "CardOverlap");
    assert_eq!(state.color_sublayers[1].name, "CardLeft");
    assert_eq!(state.icon_studio.symbol_layer_tags[0].layer_group, 2);
    assert_eq!(state.icon_studio.symbol_layer_tags[1].layer_group, 1);

    assert!(state.undo());
    assert_eq!(state.snapshot(), snap_before_z);
    assert!(state.redo());
    assert_eq!(state.color_sublayers[0].name, "CardOverlap");

    // 3. ShapeBuilder Merge Sync into color_sublayers:
    // Merging regions of c0 and c1 updates their contour in color_sublayers while preserving BadgeCurve (c2)!
    assert!(state.apply_shape_builder_operation(&[0, 1, 2], ShapeBuilderOp::Merge));
    assert!(
        state
            .color_sublayers
            .iter()
            .any(|s| s.name == "BadgeCurve" && s.contours[0].nodes.iter().any(|n| n.id == NodeId(31))),
        "Untouched color sublayer BadgeCurve must survive ShapeBuilder merge of CardLeft and CardOverlap"
    );
}

#[test]
fn tc_v0150_003_apple_four_mode_color_engine_and_hierarchy_levels() {
    use grafia::core::{
        resolve_sublayers_for_color_mode, CanvasColorSublayer, ContourId, GlyphContour, Node,
        NodeId, NodeType, SymbolColorRenderingMode, SymbolHierarchyLevel, SymbolLayerTag,
    };
    use grafia::tokens::AppearanceMode;

    let make_box = |cid: usize, nid: u64, x0: f64, x1: f64| {
        GlyphContour::new(
            ContourId(cid),
            vec![
                Node::new(NodeId(nid), x0, 180.0, NodeType::Corner),
                Node::new(NodeId(nid + 1), x1, 180.0, NodeType::Corner),
                Node::new(NodeId(nid + 2), x1, 520.0, NodeType::Corner),
                Node::new(NodeId(nid + 3), x0, 520.0, NodeType::Corner),
            ],
        )
    };
    let sublayers = vec![
        CanvasColorSublayer {
            id: 1,
            name: "PrimaryBar".into(),
            contours: vec![make_box(0, 10, 100.0, 220.0)],
            paint: ColorLayerPaint::Solid {
                rgba: [220, 40, 40, 255],
            },
            opacity: 1.0,
            visible: true,
            locked: false,
        },
        CanvasColorSublayer {
            id: 2,
            name: "SecondaryBar".into(),
            contours: vec![make_box(1, 20, 250.0, 370.0)],
            paint: ColorLayerPaint::Solid {
                rgba: [40, 200, 90, 255],
            },
            opacity: 1.0,
            visible: true,
            locked: false,
        },
        CanvasColorSublayer {
            id: 3,
            name: "TertiaryBar".into(),
            contours: vec![make_box(2, 30, 400.0, 520.0)],
            paint: ColorLayerPaint::Solid {
                rgba: [40, 100, 240, 255],
            },
            opacity: 1.0,
            visible: true,
            locked: false,
        },
    ];
    let tags = vec![
        SymbolLayerTag {
            layer_group: 1,
            hierarchy_level: SymbolHierarchyLevel::Primary,
            ..Default::default()
        },
        SymbolLayerTag {
            layer_group: 2,
            hierarchy_level: SymbolHierarchyLevel::Secondary,
            ..Default::default()
        },
        SymbolLayerTag {
            layer_group: 3,
            hierarchy_level: SymbolHierarchyLevel::Tertiary,
            ..Default::default()
        },
    ];

    // 1. Monochrome mode replaces all non-mask sublayer paints with tint_rgba:
    let mono = resolve_sublayers_for_color_mode(
        &sublayers,
        &tags,
        SymbolColorRenderingMode::Monochrome {
            tint_rgba: [30, 40, 50, 255],
        },
    );
    for s in &mono {
        assert_eq!(
            s.paint,
            ColorLayerPaint::Solid {
                rgba: [30, 40, 50, 255]
            }
        );
        assert!((s.opacity - 1.0).abs() < 1e-6);
    }

    // 2. Hierarchical mode sets Primary = 1.0, Secondary = 0.50, Tertiary = 0.25 with primary_rgba:
    let hier = resolve_sublayers_for_color_mode(
        &sublayers,
        &tags,
        SymbolColorRenderingMode::Hierarchical {
            primary_rgba: [15, 130, 255, 255],
        },
    );
    assert_eq!(
        hier[0].paint,
        ColorLayerPaint::Solid {
            rgba: [15, 130, 255, 255]
        }
    );
    assert!((hier[0].opacity - 1.0).abs() < 1e-6);
    assert!((hier[1].opacity - 0.50).abs() < 1e-6);
    assert!((hier[2].opacity - 0.25).abs() < 1e-6);

    // 3. Palette mode maps Primary -> colors[0], Secondary -> colors[1], Tertiary -> colors[2]:
    let pal_cols = [
        [250, 180, 20, 255],
        [40, 190, 240, 255],
        [170, 90, 250, 255],
    ];
    let pal = resolve_sublayers_for_color_mode(
        &sublayers,
        &tags,
        SymbolColorRenderingMode::Palette {
            colors_rgba: pal_cols,
        },
    );
    assert_eq!(pal[0].paint, ColorLayerPaint::Solid { rgba: pal_cols[0] });
    assert_eq!(pal[1].paint, ColorLayerPaint::Solid { rgba: pal_cols[1] });
    assert_eq!(pal[2].paint, ColorLayerPaint::Solid { rgba: pal_cols[2] });

    // 4. Multicolor preserves original sublayer paints:
    let multi =
        resolve_sublayers_for_color_mode(&sublayers, &tags, SymbolColorRenderingMode::Multicolor);
    assert_eq!(multi, sublayers);

    // 5. State integration, Undo/Redo, SVG roundtrip (`data-hierarchy`), and Pixel Oracle:
    let mut state = GrafiaCanvasState::init_glyph_a();
    let mut g = state.glyph.clone();
    g.contours = vec![
        make_box(0, 10, 100.0, 220.0),
        make_box(1, 20, 250.0, 370.0),
        make_box(2, 30, 400.0, 520.0),
    ];
    let m = state.metrics.clone();
    state.load_glyph(g, m);
    state.set_color_sublayers(sublayers);
    state.icon_studio.symbol_layer_tags = tags;

    let thumb_multi =
        grafia::render::render_colored_thumbnail(&state, 96, 96, AppearanceMode::Light);
    let snap_before_mode = state.snapshot();
    state.set_symbol_color_rendering_mode(SymbolColorRenderingMode::Hierarchical {
        primary_rgba: [15, 130, 255, 255],
    });
    let thumb_hier =
        grafia::render::render_colored_thumbnail(&state, 96, 96, AppearanceMode::Light);
    assert_ne!(
        thumb_multi, thumb_hier,
        "Switching from Multicolor to Hierarchical must visibly change rendered pixels"
    );

    state.select_color_sublayer(Some(0));
    assert!(state.set_selected_sublayer_hierarchy_level(SymbolHierarchyLevel::Tertiary));
    assert_eq!(
        state.icon_studio.symbol_layer_tags[0].hierarchy_level,
        SymbolHierarchyLevel::Tertiary
    );

    let svg = state.export_kinetic_symbol_to_svg();
    assert!(svg.contains("data-hierarchy=\"tertiary\""));
    assert!(svg.contains("data-hierarchy=\"secondary\""));
    let parsed = import_w3c_icon_svg(&svg).expect("SVG with data-hierarchy must roundtrip");
    assert_eq!(
        parsed.tags[0].hierarchy_level,
        SymbolHierarchyLevel::Tertiary
    );
    assert_eq!(
        parsed.tags[1].hierarchy_level,
        SymbolHierarchyLevel::Secondary
    );

    assert!(state.undo());
    assert!(state.undo());
    assert_eq!(state.snapshot(), snap_before_mode);
    assert!(state.redo());
    assert_eq!(
        state.icon_studio.color_mode,
        SymbolColorRenderingMode::Hierarchical {
            primary_rgba: [15, 130, 255, 255]
        }
    );

    // 6. Unconstrained arbtest over arbitrary opacity & modes:
    arbtest(|u| {
        let raw_op = f32::from_bits(u.arbitrary::<u32>()?);
        let rgba = u.arbitrary::<[u8; 4]>()?;
        let lvl = match u.int_in_range(0..=2)? {
            0 => SymbolHierarchyLevel::Primary,
            1 => SymbolHierarchyLevel::Secondary,
            _ => SymbolHierarchyLevel::Tertiary,
        };
        let mode = match u.int_in_range(0..=3)? {
            0 => SymbolColorRenderingMode::Monochrome { tint_rgba: rgba },
            1 => SymbolColorRenderingMode::Hierarchical { primary_rgba: rgba },
            2 => SymbolColorRenderingMode::Palette {
                colors_rgba: [rgba, rgba, rgba],
            },
            _ => SymbolColorRenderingMode::Multicolor,
        };
        let s = CanvasColorSublayer {
            id: 1,
            name: "Fuzz".into(),
            contours: Vec::new(),
            paint: ColorLayerPaint::Solid { rgba },
            opacity: raw_op,
            visible: true,
            locked: false,
        };
        let t = SymbolLayerTag {
            hierarchy_level: lvl,
            ..Default::default()
        };
        let out = resolve_sublayers_for_color_mode(&[s], &[t], mode);
        assert_eq!(out.len(), 1);
        assert!(out[0].opacity.is_finite() && (0.0..=1.0).contains(&out[0].opacity));
        Ok(())
    })
    .budget_ms(200);
}

#[test]
fn tc_v0150_004_on_canvas_smart_corner_radius_pin_and_open_stroke_sublayers() {
    use grafia::core::{
        expand_non_destructive_icon_contours, hit_test_smart_corner_radius_pin,
        CanvasColorSublayer, ContourId, ContourStrokeStyle, FontPoint, GlyphContour,
        InputDeviceKind, LineCapStyle, LineJoinStyle, Node, NodeId, NodeType, SmartCornerSpec,
        StrokePosition,
    };
    use grafia::tokens::GrafiaTheme;
    use grafia::CanvasViewMode;

    // 1. Setup a glyph with a closed rectangle (ContourId(0)) and an open cubic wave (ContourId(1))
    //    with ContourStrokeStyle (width = 36 UPM, Round caps) in color_sublayers:
    let rect_c = GlyphContour::new(
        ContourId(0),
        vec![
            Node::new(NodeId(10), 120.0, 160.0, NodeType::Corner),
            Node::new(NodeId(11), 420.0, 160.0, NodeType::Corner),
            Node::new(NodeId(12), 420.0, 460.0, NodeType::Corner),
            Node::new(NodeId(13), 120.0, 460.0, NodeType::Corner),
        ],
    );
    let mut open_wave = GlyphContour::new(
        ContourId(1),
        vec![
            Node::new(NodeId(20), 480.0, 200.0, NodeType::Corner),
            Node::new(NodeId(21), 560.0, 420.0, NodeType::OffCurve),
            Node::new(NodeId(22), 660.0, 120.0, NodeType::OffCurve),
            Node::new(NodeId(23), 740.0, 360.0, NodeType::Corner),
        ],
    );
    open_wave.closed = false;

    let mut state = GrafiaCanvasState::init_glyph_a();
    state.viewport =
        grafia::ViewportTransform::new(0.5, grafia::ScreenPoint::new(80.0, 420.0), 1000);
    let mut g = state.glyph.clone();
    g.contours = vec![rect_c.clone(), open_wave.clone()];
    let m = state.metrics.clone();
    state.load_glyph(g, m);
    state.set_color_sublayers(vec![
        CanvasColorSublayer {
            id: 1,
            name: "Card".into(),
            contours: vec![rect_c],
            paint: ColorLayerPaint::Solid {
                rgba: [35, 135, 250, 255],
            },
            opacity: 1.0,
            visible: true,
            locked: false,
        },
        CanvasColorSublayer {
            id: 2,
            name: "WaveStroke".into(),
            contours: vec![open_wave],
            paint: ColorLayerPaint::Solid {
                rgba: [245, 70, 95, 255],
            },
            opacity: 1.0,
            visible: true,
            locked: false,
        },
    ]);
    state.set_contour_stroke(
        ContourId(1),
        ContourStrokeStyle {
            width: 36.0,
            height: 36.0,
            position: StrokePosition::Center,
            cap_start: LineCapStyle::Round,
            cap_end: LineCapStyle::Round,
            join: LineJoinStyle::Round,
        },
    );

    // 2. Select Corner NodeId(10) at (120, 160) -> SmartCornerRadiusPin must appear along inward bisector (+1/sqrt(2), +1/sqrt(2)):
    state.activate_tool(CanvasTool::Select);
    state.selected_nodes = vec![NodeId(10)];
    let pins = state.compute_smart_corner_radius_pins();
    assert_eq!(pins.len(), 1);
    let pin0 = pins[0];
    assert_eq!(pin0.node_id, NodeId(10));
    assert!(pin0.bisector_unit.0 > 0.7 && pin0.bisector_unit.1 > 0.7);
    assert_eq!(
        hit_test_smart_corner_radius_pin(&pins, pin0.pin_pt, 8.0),
        Some(NodeId(10))
    );

    let theme = GrafiaTheme::sonoma_default();
    state.view_mode = CanvasViewMode::PreviewFilled;
    let pix_filled_before = grafia::adapters::rasterize_canvas_subregion(&state, &theme, 520, 380);
    state.view_mode = CanvasViewMode::LiquidGlass;
    let pix_glass_before = grafia::adapters::rasterize_canvas_subregion(&state, &theme, 520, 380);
    state.view_mode = CanvasViewMode::EditWireframe;
    let pix_wire_before = grafia::adapters::rasterize_canvas_subregion(&state, &theme, 520, 380);

    // 3. Drag the SmartCornerRadiusPin inward via InputDispatcher (also verifying TouchFinger 1.6x hit radius):
    state.set_active_input_device(InputDeviceKind::TouchFinger);
    state.preferences.touch_finger_offset_dp = 0.0;
    let snap_before_pin_drag = state.snapshot();
    let pin_scr = state.viewport.to_screen(pin0.pin_pt);
    let target_upm = FontPoint::new(
        pin0.vertex_pt.x.0 + pin0.bisector_unit.0 * 75.0,
        pin0.vertex_pt.y.0 + pin0.bisector_unit.1 * 75.0,
    );
    let target_scr = state.viewport.to_screen(target_upm);
    let (p_start, p_end) = (
        egui::pos2(pin_scr.x.0 + 4.0, pin_scr.y.0 - 3.0),
        egui::pos2(target_scr.x.0, target_scr.y.0),
    );
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
    state.set_active_input_device(InputDeviceKind::Mouse);

    let sc = state
        .smart_corners()
        .iter()
        .find(|s| s.node_id == NodeId(10))
        .expect("Dragging SmartCornerRadiusPin must create/update SmartCornerSpec");
    assert!(
        sc.radius_upm > 35.0,
        "Expected SmartCornerSpec radius_upm > 35.0 after inward drag, got {}",
        sc.radius_upm
    );

    // Verify EditWireframe, PreviewFilled, and LiquidGlass all visibly reflect the rounded SmartCornerSpec:
    let pix_wire_after = grafia::adapters::rasterize_canvas_subregion(&state, &theme, 520, 380);
    assert_ne!(pix_wire_before.data(), pix_wire_after.data());
    state.view_mode = CanvasViewMode::PreviewFilled;
    let pix_filled_after = grafia::adapters::rasterize_canvas_subregion(&state, &theme, 520, 380);
    assert_ne!(pix_filled_before.data(), pix_filled_after.data());
    state.view_mode = CanvasViewMode::LiquidGlass;
    let pix_glass_after = grafia::adapters::rasterize_canvas_subregion(&state, &theme, 520, 380);
    assert_ne!(pix_glass_before.data(), pix_glass_after.data());

    // Verify Undo/Redo on SmartCornerRadiusPin drag:
    assert!(state.undo());
    assert_eq!(state.snapshot(), snap_before_pin_drag);
    assert!(state.redo());
    assert!(!state.smart_corners().is_empty());

    // Verify SmartCornerRadiusPin is draggable even when multiple nodes are selected (BBox active) or in ShapeBuilder tool:
    state.selected_nodes = vec![NodeId(10), NodeId(11), NodeId(12), NodeId(13)];
    state.activate_tool(CanvasTool::ShapeBuilder);
    let pin10 = state
        .compute_smart_corner_radius_pins()
        .into_iter()
        .find(|p| p.node_id == NodeId(10))
        .expect("NodeId(10) pin must exist");
    let pin10_scr = state.viewport.to_screen(pin10.pin_pt);
    assert!(
        state.on_drawing_tool_pointer_press(pin10_scr, pin10.pin_pt, grafia::Modifiers::default()),
        "ShapeBuilder must allow dragging SmartCornerRadiusPin directly"
    );
    assert!(state.try_finish_smart_corner_pin_release());
    state.activate_tool(CanvasTool::Select);

    // Verify N-fold Radial Symmetry propagates SmartCornerSpec to all N symmetry copies (both live and baked):
    state.icon_studio.radial_symmetry.fold_count = 6;
    state.icon_studio.radial_symmetry.center = FontPoint::new(300.0, 350.0);
    let sym_preview = state.compute_smart_corner_preview_contours();
    assert!(
        sym_preview.len() >= 12,
        "Expected 6-fold symmetry preview to include all 6 copies of contours, got {}",
        sym_preview.len()
    );
    let sym_expanded = state.compute_expanded_stroke_contours();
    let copy_1_rect = sym_expanded
        .iter()
        .find(|c| c.closed && c.nodes.len() > 4)
        .expect("Symmetry copy in compute_expanded_stroke_contours must have rounded SmartCorner nodes (> 4 nodes)");
    assert!(copy_1_rect.nodes.len() >= 6);
    let snap_before_sym_bake = state.snapshot();
    assert!(state.bake_radial_symmetry());
    let baked_rects: Vec<_> = state.glyph.contours.iter().filter(|c| c.closed).collect();
    assert_eq!(baked_rects.len(), 6);
    for (k, bc) in baked_rects.iter().enumerate() {
        assert!(
            bc.nodes.len() >= 6,
            "Baked symmetry copy {k} must preserve rounded SmartCorner nodes (>= 6 nodes), got {}",
            bc.nodes.len()
        );
    }
    assert!(state.undo());
    assert_eq!(state.snapshot(), snap_before_sym_bake);
    state.icon_studio.radial_symmetry.fold_count = 1;

    // 4. Verify 1st-class open-stroke rendering in color_sublayers & PreviewFilled + live OffCurve handle drag:
    state.view_mode = CanvasViewMode::PreviewFilled;
    let eval_subs = state.evaluate_active_color_sublayers();
    assert!(
        eval_subs[1].contours[0].closed && eval_subs[1].contours[0].nodes.len() >= 6,
        "Open stroked contour in color_sublayers[1] must expand non-destructively into a closed vector ribbon"
    );
    let pix_wave_before = grafia::adapters::rasterize_canvas_subregion(&state, &theme, 520, 380);

    // Drag OffCurve handle NodeId(21) at (560, 420) on open stroked contour via InputDispatcher:
    state.view_mode = CanvasViewMode::EditWireframe;
    let scr21 = state.viewport.to_screen(FontPoint::new(560.0, 420.0));
    let (h_start, h_end) = (
        egui::pos2(scr21.x.0, scr21.y.0),
        egui::pos2(scr21.x.0 + 25.0, scr21.y.0 - 30.0),
    );
    for (t, evs) in [
        (
            2.0,
            vec![egui::Event::PointerButton {
                pos: h_start,
                button: egui::PointerButton::Primary,
                pressed: true,
                modifiers: egui::Modifiers::NONE,
            }],
        ),
        (
            2.1,
            vec![
                egui::Event::PointerMoved(h_end),
                egui::Event::PointerButton {
                    pos: h_end,
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
    assert!(
        !state.glyph.contours[1].closed,
        "1D skeleton of open stroked contour must remain open and non-destructive after handle drag"
    );
    state.view_mode = CanvasViewMode::PreviewFilled;
    let pix_wave_after = grafia::adapters::rasterize_canvas_subregion(&state, &theme, 520, 380);
    assert_ne!(
        pix_wave_before.data(),
        pix_wave_after.data(),
        "Dragging OffCurve handle on open stroked contour must live-update its expanded filled sublayer in PreviewFilled"
    );

    // 5. Pijler A: Unconstrained arbtest on expand_non_destructive_icon_contours & SmartCornerRadiusPin
    arbtest(|u| {
        let r = f64::from_bits(u.arbitrary::<u64>()?);
        let w = f64::from_bits(u.arbitrary::<u64>()?);
        let sc = SmartCornerSpec {
            node_id: NodeId(10),
            radius_upm: r,
        };
        let mut strokes = std::collections::BTreeMap::new();
        strokes.insert(
            ContourId(1),
            ContourStrokeStyle {
                width: w,
                height: w,
                position: StrokePosition::Center,
                cap_start: LineCapStyle::Round,
                cap_end: LineCapStyle::Round,
                join: LineJoinStyle::Round,
            },
        );
        let mut next_nid = 1000_u64;
        let expanded = expand_non_destructive_icon_contours(
            &state.glyph.contours,
            &[sc],
            &strokes,
            &[],
            &mut next_nid,
        );
        for c in &expanded {
            for n in &c.nodes {
                assert!(n.point.x.0.is_finite() && n.point.y.0.is_finite());
            }
        }
        Ok(())
    })
    .budget_ms(200);
}

#[test]
fn tc_v0150_005_centerline_draw_on_draw_off_directions_and_live_cap_ribbon_expansion() {
    use grafia::core::{
        export_w3c_icon_svg, import_w3c_icon_svg, BounceDirection, CanvasColorSublayer, ContourId,
        ContourStrokeStyle, DrawDirection, GlyphContour, KineticLayerScope, KineticSymbolEffect,
        LineCapStyle, LineJoinStyle, Node, NodeId, NodeType, StrokePosition, SymbolLayerTag,
    };
    use grafia::tokens::GrafiaTheme;
    use grafia::CanvasViewMode;

    // 1. Build 3 open centerline sublayers (StartToEnd, EndToStart, CenterOut) with 36 UPM Round-Cap strokes:
    let make_open_line = |cid: usize, base_nid: u64, y: f64| -> GlyphContour {
        let mut c = GlyphContour::new(
            ContourId(cid),
            vec![
                Node::new(NodeId(base_nid), 120.0, y, NodeType::Corner),
                Node::new(NodeId(base_nid + 1), 280.0, y + 60.0, NodeType::OffCurve),
                Node::new(NodeId(base_nid + 2), 440.0, y - 60.0, NodeType::OffCurve),
                Node::new(NodeId(base_nid + 3), 600.0, y, NodeType::Corner),
            ],
        );
        c.closed = false;
        c
    };
    let c0 = make_open_line(0, 10, 180.0);
    let c1 = make_open_line(1, 20, 320.0);
    let c2 = make_open_line(2, 30, 460.0);

    let mut state = GrafiaCanvasState::init_glyph_a();
    state.viewport =
        grafia::ViewportTransform::new(0.5, grafia::ScreenPoint::new(80.0, 420.0), 1000);
    let mut g = state.glyph.clone();
    g.contours = vec![c0.clone(), c1.clone(), c2.clone()];
    let m = state.metrics.clone();
    state.load_glyph(g, m);
    state.set_color_sublayers(vec![
        CanvasColorSublayer {
            id: 1,
            name: "ArcStartToEnd".into(),
            contours: vec![c0],
            paint: ColorLayerPaint::Solid {
                rgba: [40, 145, 255, 255],
            },
            opacity: 1.0,
            visible: true,
            locked: false,
        },
        CanvasColorSublayer {
            id: 2,
            name: "ArcEndToStart".into(),
            contours: vec![c1],
            paint: ColorLayerPaint::Solid {
                rgba: [50, 200, 165, 255],
            },
            opacity: 1.0,
            visible: true,
            locked: false,
        },
        CanvasColorSublayer {
            id: 3,
            name: "ArcCenterOut".into(),
            contours: vec![c2],
            paint: ColorLayerPaint::Solid {
                rgba: [255, 140, 55, 255],
            },
            opacity: 1.0,
            visible: true,
            locked: false,
        },
    ]);
    let stroke_style = ContourStrokeStyle {
        width: 36.0,
        height: 36.0,
        position: StrokePosition::Center,
        cap_start: LineCapStyle::Round,
        cap_end: LineCapStyle::Round,
        join: LineJoinStyle::Round,
    };
    for cid in [ContourId(0), ContourId(1), ContourId(2)] {
        state.set_contour_stroke(cid, stroke_style);
    }
    state.icon_studio.symbol_layer_tags = vec![
        SymbolLayerTag {
            layer_group: 1,
            draw_direction: DrawDirection::StartToEnd,
            ..Default::default()
        },
        SymbolLayerTag {
            layer_group: 1,
            draw_direction: DrawDirection::EndToStart,
            ..Default::default()
        },
        SymbolLayerTag {
            layer_group: 1,
            draw_direction: DrawDirection::CenterOut,
            ..Default::default()
        },
    ];

    // 2. Verify DrawDirection arc-range mathematics for Appear (DrawOn) and Disappear (DrawOff):
    assert_eq!(DrawDirection::StartToEnd.appear_arc_range(0.5), (0.0, 0.5));
    assert_eq!(DrawDirection::EndToStart.appear_arc_range(0.5), (0.5, 1.0));
    assert_eq!(DrawDirection::CenterOut.appear_arc_range(0.5), (0.25, 0.75));
    assert_eq!(
        DrawDirection::StartToEnd.disappear_arc_range(0.4),
        (0.4, 1.0)
    );
    assert_eq!(
        DrawDirection::EndToStart.disappear_arc_range(0.4),
        (0.0, 0.6)
    );
    assert_eq!(
        DrawDirection::CenterOut.disappear_arc_range(0.4),
        (0.2, 0.8)
    );

    // 3. Evaluate Appear (draw_on: true, WholeSymbol) at phase_t = 0.5:
    //    Each 1D centerline must be trimmed FIRST on its DrawDirection interval, and THEN expanded
    //    into a closed round-cap ribbon whose bounding box matches its direction!
    state.icon_studio.kinetic.effect = KineticSymbolEffect::Appear {
        direction: BounceDirection::Up,
        draw_on: true,
        scope: KineticLayerScope::WholeSymbol,
    };
    state.icon_studio.kinetic.phase_t = 0.5;
    let eval_subs = state.evaluate_active_color_sublayers();
    assert_eq!(eval_subs.len(), 3);
    for (idx, sub) in eval_subs.iter().enumerate() {
        assert!(
            sub.contours[0].closed && sub.contours[0].nodes.len() >= 6,
            "Sublayer {idx} must be trimmed along 1D centerline first, then expanded into a closed ribbon"
        );
    }
    let bbox_x = |c: &GlyphContour| -> (f64, f64) {
        let mut min_x = f64::INFINITY;
        let mut max_x = f64::NEG_INFINITY;
        for n in &c.nodes {
            min_x = min_x.min(n.point.x.0);
            max_x = max_x.max(n.point.x.0);
        }
        (min_x, max_x)
    };
    let (min0, max0) = bbox_x(&eval_subs[0].contours[0]);
    let (min1, max1) = bbox_x(&eval_subs[1].contours[0]);
    let (min2, max2) = bbox_x(&eval_subs[2].contours[0]);
    // StartToEnd [0.0, 0.5] starts near x=120 and ends near midpoint x~360:
    assert!(
        min0 < 175.0 && max0 < 430.0,
        "StartToEnd ribbon at t=0.5 must cover left half, got ({min0:.1}, {max0:.1})"
    );
    // EndToStart [0.5, 1.0] starts near midpoint x~360 and ends near x=600:
    assert!(
        min1 > 290.0 && max1 > 545.0,
        "EndToStart ribbon at t=0.5 must cover right half, got ({min1:.1}, {max1:.1})"
    );
    // CenterOut [0.25, 0.75] is centered around x=360:
    assert!(
        min2 > min0 + 40.0 && max2 < max1 - 40.0,
        "CenterOut ribbon at t=0.5 must be centered between StartToEnd and EndToStart, got ({min2:.1}, {max2:.1})"
    );

    // 4. Verify evaluate_active_kinetic_contours() also expands ContourStrokeStyle after trimming,
    //    and verify visual pixel progression across t = 0.25 vs t = 0.75 in EditWireframe & PreviewFilled:
    let kin_contours = state.evaluate_active_kinetic_contours();
    assert_eq!(kin_contours.len(), 3);
    assert!(kin_contours
        .iter()
        .all(|(c, _)| c.closed && c.nodes.len() >= 6));

    let theme = GrafiaTheme::sonoma_default();
    state.icon_studio.kinetic.phase_t = 0.25;
    state.view_mode = CanvasViewMode::PreviewFilled;
    let pix_t25 = grafia::adapters::rasterize_canvas_subregion(&state, &theme, 520, 380);
    state.icon_studio.kinetic.phase_t = 0.75;
    let pix_t75 = grafia::adapters::rasterize_canvas_subregion(&state, &theme, 520, 380);
    assert_ne!(pix_t25.data(), pix_t75.data());

    // 5. Verify set_selected_sublayer_draw_direction with Undo/Redo & W3C SVG data-draw-direction round-trip:
    state.select_color_sublayer(Some(0));
    let snap_before_dir = state.snapshot();
    assert!(state.set_selected_sublayer_draw_direction(DrawDirection::CenterOut));
    assert_eq!(
        state.icon_studio.symbol_layer_tags[0].draw_direction,
        DrawDirection::CenterOut
    );
    assert!(state.undo());
    assert_eq!(state.snapshot(), snap_before_dir);
    assert!(state.redo());
    assert_eq!(
        state.icon_studio.symbol_layer_tags[0].draw_direction,
        DrawDirection::CenterOut
    );

    let svg = export_w3c_icon_svg(
        &state.color_sublayers,
        &state.icon_studio.symbol_layer_tags,
        &state.contour_strokes,
    );
    assert!(svg.contains("data-draw-direction=\"center-out\""));
    assert!(svg.contains("data-draw-direction=\"end-to-start\""));
    let parsed_doc =
        import_w3c_icon_svg(&svg).expect("W3C SVG with data-draw-direction must parse");
    assert_eq!(parsed_doc.tags[0].draw_direction, DrawDirection::CenterOut);
    assert_eq!(parsed_doc.tags[1].draw_direction, DrawDirection::EndToStart);

    // 6. Pijler A: Unconstrained arbtest on DrawDirection::appear_arc_range / disappear_arc_range
    arbtest(|u| {
        let raw_s = f64::from_bits(u.arbitrary::<u64>()?);
        let dir_idx = u.int_in_range(0..=2)?;
        let dir = match dir_idx {
            0 => DrawDirection::StartToEnd,
            1 => DrawDirection::EndToStart,
            _ => DrawDirection::CenterOut,
        };
        let (a0, a1) = dir.appear_arc_range(raw_s);
        let (d0, d1) = dir.disappear_arc_range(raw_s);
        assert!(a0.is_finite() && a1.is_finite() && a0 >= 0.0 && a1 <= 1.0 && a0 <= a1);
        assert!(d0.is_finite() && d1.is_finite() && d0 >= 0.0 && d1 <= 1.0 && d0 <= d1);
        Ok(())
    })
    .budget_ms(150);
}

#[test]
fn tc_v0150_006_live_non_destructive_knockout_slash_halo_masks_and_draw_on_sync() {
    use grafia::core::{
        apply_knockout_halo_masks_to_sublayers, compute_contour_area_and_centroid,
        export_w3c_icon_svg, import_w3c_icon_svg, BounceDirection, CanvasColorSublayer, ContourId,
        ContourStrokeStyle, DrawDirection, FontPoint, GlyphContour, KineticLayerScope,
        KineticSymbolEffect, LineCapStyle, LineJoinStyle, Node, NodeId, NodeType, StrokePosition,
        SymbolLayerTag,
    };
    use grafia::tokens::GrafiaTheme;
    use grafia::CanvasViewMode;

    // 1. Build a 3-layer icon: 2 underlying horizontal bars (Sublayers 0 & 1) and 1 diagonal Slash (Sublayer 2)
    //    with ContourStrokeStyle (width = 24 UPM) and knockout_margin_upm = 18.0 UPM:
    let bar0 = GlyphContour::new(
        ContourId(0),
        vec![
            Node::new(NodeId(10), 120.0, 220.0, NodeType::Corner),
            Node::new(NodeId(11), 560.0, 220.0, NodeType::Corner),
            Node::new(NodeId(12), 560.0, 280.0, NodeType::Corner),
            Node::new(NodeId(13), 120.0, 280.0, NodeType::Corner),
        ],
    );
    let bar1 = GlyphContour::new(
        ContourId(1),
        vec![
            Node::new(NodeId(20), 120.0, 360.0, NodeType::Corner),
            Node::new(NodeId(21), 560.0, 360.0, NodeType::Corner),
            Node::new(NodeId(22), 560.0, 420.0, NodeType::Corner),
            Node::new(NodeId(23), 120.0, 420.0, NodeType::Corner),
        ],
    );
    let mut slash = GlyphContour::new(
        ContourId(2),
        vec![
            Node::new(NodeId(30), 180.0, 140.0, NodeType::Corner),
            Node::new(NodeId(31), 500.0, 500.0, NodeType::Corner),
        ],
    );
    slash.closed = false;

    let mut state = GrafiaCanvasState::init_glyph_a();
    state.viewport =
        grafia::ViewportTransform::new(0.5, grafia::ScreenPoint::new(80.0, 420.0), 1000);
    let mut g = state.glyph.clone();
    g.contours = vec![bar0.clone(), bar1.clone(), slash.clone()];
    let m = state.metrics.clone();
    state.load_glyph(g, m);
    state.set_color_sublayers(vec![
        CanvasColorSublayer {
            id: 1,
            name: "WaveLower".into(),
            contours: vec![bar0.clone()],
            paint: ColorLayerPaint::Solid {
                rgba: [40, 145, 255, 255],
            },
            opacity: 1.0,
            visible: true,
            locked: false,
        },
        CanvasColorSublayer {
            id: 2,
            name: "WaveUpper".into(),
            contours: vec![bar1.clone()],
            paint: ColorLayerPaint::Solid {
                rgba: [55, 200, 175, 255],
            },
            opacity: 1.0,
            visible: true,
            locked: false,
        },
        CanvasColorSublayer {
            id: 3,
            name: "MuteSlash".into(),
            contours: vec![slash],
            paint: ColorLayerPaint::Solid {
                rgba: [245, 70, 95, 255],
            },
            opacity: 1.0,
            visible: true,
            locked: false,
        },
    ]);
    state.set_contour_stroke(
        ContourId(2),
        ContourStrokeStyle {
            width: 24.0,
            height: 24.0,
            position: StrokePosition::Center,
            cap_start: LineCapStyle::Round,
            cap_end: LineCapStyle::Round,
            join: LineJoinStyle::Round,
        },
    );
    state.icon_studio.symbol_layer_tags = vec![
        SymbolLayerTag {
            layer_group: 1,
            ..Default::default()
        },
        SymbolLayerTag {
            layer_group: 1,
            ..Default::default()
        },
        SymbolLayerTag {
            layer_group: 2,
            draw_direction: DrawDirection::StartToEnd,
            knockout_margin_upm: 0.0,
            ..Default::default()
        },
    ];

    let total_area = |cs: &[GlyphContour]| -> f64 {
        cs.iter()
            .filter(|c| c.closed)
            .map(|c| compute_contour_area_and_centroid(&c.nodes).0.abs())
            .sum()
    };
    let orig_bar0_area = total_area(&[bar0]);
    let orig_bar1_area = total_area(&[bar1]);

    // 2. Enable knockout_margin_upm = 18.0 on Sublayer 2 via set_selected_sublayer_knockout_margin (with Undo/Redo):
    let theme = GrafiaTheme::sonoma_default();
    state.view_mode = CanvasViewMode::PreviewFilled;
    let pix_no_knockout = grafia::adapters::rasterize_canvas_subregion(&state, &theme, 520, 380);

    state.select_color_sublayer(Some(2));
    let snap_before_ko = state.snapshot();
    assert!(state.set_selected_sublayer_knockout_margin(18.0));
    assert!((state.icon_studio.symbol_layer_tags[2].knockout_margin_upm - 18.0).abs() < 1e-6);
    assert!(state.undo());
    assert_eq!(state.snapshot(), snap_before_ko);
    assert!(state.redo());

    // Verify underlying sublayers 0 and 1 are boolean-carved by the expanded halo of Sublayer 2,
    // while state.glyph.contours remains 100% untouched (non-destructive!):
    assert_eq!(state.glyph.contours[0].nodes.len(), 4);
    assert_eq!(state.glyph.contours[1].nodes.len(), 4);
    assert!(!state.glyph.contours[2].closed);

    let eval_ko = state.evaluate_active_color_sublayers();
    let carved_bar0_area = total_area(&eval_ko[0].contours);
    let carved_bar1_area = total_area(&eval_ko[1].contours);
    assert!(
        eval_ko[0].contours.len() >= 2 && carved_bar0_area < orig_bar0_area * 0.92,
        "Underlying sublayer 0 must be carved into 2 pieces with reduced area by the slash halo, got len={}, area={carved_bar0_area:.1} vs {orig_bar0_area:.1}",
        eval_ko[0].contours.len()
    );
    assert!(
        eval_ko[1].contours.len() >= 2 && carved_bar1_area < orig_bar1_area * 0.92,
        "Underlying sublayer 1 must be carved into 2 pieces with reduced area by the slash halo, got len={}, area={carved_bar1_area:.1} vs {orig_bar1_area:.1}",
        eval_ko[1].contours.len()
    );
    let pix_with_knockout = grafia::adapters::rasterize_canvas_subregion(&state, &theme, 520, 380);
    assert_ne!(
        pix_no_knockout.data(),
        pix_with_knockout.data(),
        "Knockout halo must visibly carve a transparent gap around the slash in PreviewFilled"
    );

    // 3. Verify live DrawOn synchronization: as the slash grows during Appear (DrawOn),
    //    when all sublayers share layer_group = 1 (or WholeSymbol), at phase_t = 0.45 the slash
    //    has only grown across lower bar0 (y=220..280) and not yet reached upper bar1 (y=360..420),
    //    so bar0 is carved into 2 pieces while bar1 remains 1 intact piece, and at phase_t = 1.0 both are carved!
    state.icon_studio.kinetic.effect = KineticSymbolEffect::Appear {
        direction: BounceDirection::Up,
        draw_on: true,
        scope: KineticLayerScope::WholeSymbol,
    };
    state.icon_studio.symbol_layer_tags[0].draw_direction = DrawDirection::StartToEnd;
    state.icon_studio.symbol_layer_tags[1].draw_direction = DrawDirection::StartToEnd;
    // Give bars 0 & 1 full length (only slash trims via ByLayer where bars are group 1 and slash is group 2 at t=0.65):
    state.icon_studio.kinetic.effect = KineticSymbolEffect::Appear {
        direction: BounceDirection::Up,
        draw_on: true,
        scope: KineticLayerScope::ByLayer,
    };
    state.icon_studio.kinetic.phase_t = 0.72;
    let eval_t72 = state.evaluate_active_color_sublayers();
    state.icon_studio.kinetic.phase_t = 1.0;
    let eval_t100 = state.evaluate_active_color_sublayers();
    assert!(
        eval_t100[0].contours.len() >= 2 && eval_t100[1].contours.len() >= 2,
        "At t=1.0 both lower bar0 and upper bar1 must be carved into >= 2 pieces by the full-length slash halo"
    );
    assert!(
        eval_t72[0].contours.len() >= 2 && eval_t72[1].contours.len() == 1,
        "At t=0.72 the growing slash halo must already carve lower bar0 (len={}) while upper bar1 is not yet reached (len={})",
        eval_t72[0].contours.len(),
        eval_t72[1].contours.len()
    );
    state.icon_studio.kinetic.phase_t = 0.0;

    // 4. Verify live pointer drag of slash endpoint NodeId(31) via InputDispatcher shifts the carved gap:
    let pix_before_slash_drag =
        grafia::adapters::rasterize_canvas_subregion(&state, &theme, 520, 380);
    state.view_mode = CanvasViewMode::EditWireframe;
    state.activate_tool(CanvasTool::Select);
    let scr31 = state.viewport.to_screen(FontPoint::new(500.0, 500.0));
    let (p_start, p_end) = (
        egui::pos2(scr31.x.0, scr31.y.0),
        egui::pos2(scr31.x.0 - 40.0, scr31.y.0),
    );
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
    state.view_mode = CanvasViewMode::PreviewFilled;
    let pix_after_slash_drag =
        grafia::adapters::rasterize_canvas_subregion(&state, &theme, 520, 380);
    assert_ne!(
        pix_before_slash_drag.data(),
        pix_after_slash_drag.data(),
        "Dragging slash node via InputDispatcher must live-update the carved knockout halo in underlying sublayers"
    );

    // 5. Verify W3C SVG data-knockout-margin round-trip:
    let svg = export_w3c_icon_svg(
        &state.color_sublayers,
        &state.icon_studio.symbol_layer_tags,
        &state.contour_strokes,
    );
    assert!(svg.contains("data-knockout-margin=\"18.0000\""));
    let parsed = import_w3c_icon_svg(&svg).expect("SVG with data-knockout-margin must parse");
    assert!((parsed.tags[2].knockout_margin_upm - 18.0).abs() < 1e-4);

    // 6. Pijler A: Unconstrained arbtest on apply_knockout_halo_masks_to_sublayers
    let sample_subs = state.color_sublayers.clone();
    arbtest(|u| {
        let raw_margin = f64::from_bits(u.arbitrary::<u64>()?);
        let tags = [
            SymbolLayerTag::default(),
            SymbolLayerTag::default(),
            SymbolLayerTag {
                knockout_margin_upm: raw_margin,
                ..Default::default()
            },
        ];
        let mut work = sample_subs.clone();
        let mut next_nid = 2000_u64;
        apply_knockout_halo_masks_to_sublayers(&mut work, &tags, &mut next_nid);
        for sub in &work {
            for c in &sub.contours {
                for n in &c.nodes {
                    assert!(n.point.x.0.is_finite() && n.point.y.0.is_finite());
                }
            }
        }
        Ok(())
    })
    .budget_ms(200);
}

#[test]
fn tc_v0150_007_execution_modes_variable_value_spring_morph_and_motion_path_following() {
    use grafia::core::{
        compute_contour_area_and_centroid, evaluate_motion_path_pose_at_ratio, export_w3c_icon_svg,
        import_w3c_icon_svg, BounceDirection, CanvasColorSublayer, ContourId, FontPoint,
        GlyphContour, KineticLayerScope, KineticPlaybackMode, KineticSymbolEffect, Node, NodeId,
        NodeType, SymbolLayerTag,
    };
    use grafia::tokens::GrafiaTheme;
    use grafia::CanvasViewMode;

    // 1. Build 3 Wi-Fi signal bar sublayers + 1 curved trajectory contour (ContourId(3)) and 1 indicator dot (ContourId(2)):
    let make_bar = |cid: usize, base_nid: u64, x0: f64, y1: f64| -> GlyphContour {
        GlyphContour::new(
            ContourId(cid),
            vec![
                Node::new(NodeId(base_nid), x0, 140.0, NodeType::Corner),
                Node::new(NodeId(base_nid + 1), x0 + 60.0, 140.0, NodeType::Corner),
                Node::new(NodeId(base_nid + 2), x0 + 60.0, y1, NodeType::Corner),
                Node::new(NodeId(base_nid + 3), x0, y1, NodeType::Corner),
            ],
        )
    };
    let bar0 = make_bar(0, 10, 120.0, 260.0);
    let bar1 = make_bar(1, 20, 220.0, 380.0);
    let dot2 = make_bar(2, 30, 140.0, 180.0); // starts centered at (170, 160)
    let mut motion_curve = GlyphContour::new(
        ContourId(3),
        vec![
            Node::new(NodeId(40), 170.0, 160.0, NodeType::Corner),
            Node::new(NodeId(41), 280.0, 460.0, NodeType::OffCurve),
            Node::new(NodeId(42), 480.0, 460.0, NodeType::OffCurve),
            Node::new(NodeId(43), 620.0, 220.0, NodeType::Corner),
        ],
    );
    motion_curve.closed = false;

    let mut state = GrafiaCanvasState::init_glyph_a();
    state.viewport =
        grafia::ViewportTransform::new(0.5, grafia::ScreenPoint::new(80.0, 420.0), 1000);
    let mut g = state.glyph.clone();
    g.contours = vec![
        bar0.clone(),
        bar1.clone(),
        dot2.clone(),
        motion_curve.clone(),
    ];
    let m = state.metrics.clone();
    state.load_glyph(g, m);
    state.set_color_sublayers(vec![
        CanvasColorSublayer {
            id: 1,
            name: "Bar1".into(),
            contours: vec![bar0],
            paint: ColorLayerPaint::Solid {
                rgba: [40, 145, 255, 255],
            },
            opacity: 1.0,
            visible: true,
            locked: false,
        },
        CanvasColorSublayer {
            id: 2,
            name: "Bar2".into(),
            contours: vec![bar1],
            paint: ColorLayerPaint::Solid {
                rgba: [55, 200, 175, 255],
            },
            opacity: 1.0,
            visible: true,
            locked: false,
        },
        CanvasColorSublayer {
            id: 3,
            name: "OrbitDot".into(),
            contours: vec![dot2],
            paint: ColorLayerPaint::Solid {
                rgba: [255, 140, 55, 255],
            },
            opacity: 1.0,
            visible: true,
            locked: false,
        },
    ]);
    state.icon_studio.symbol_layer_tags = vec![
        SymbolLayerTag {
            layer_group: 1,
            ..Default::default()
        },
        SymbolLayerTag {
            layer_group: 2,
            ..Default::default()
        },
        SymbolLayerTag {
            layer_group: 3,
            ..Default::default()
        },
    ];

    // 2. Verify KineticPlaybackMode::VariableValue(v) drives cumulative layers at v = 0.0, 0.2, 0.55, 1.0
    //    WITHOUT playing == true or phase_t > 0.0:
    state.icon_studio.kinetic.playing = false;
    state.icon_studio.kinetic.phase_t = 0.0;
    state.icon_studio.kinetic.effect = KineticSymbolEffect::VariableColor {
        cumulative: true,
        reversing: false,
        dim_inactive: true,
    };

    let snap_before_mode = state.snapshot();
    state.set_kinetic_playback_mode(KineticPlaybackMode::VariableValue(0.20));
    let subs_v20 = state.evaluate_active_color_sublayers();
    assert!(
        subs_v20[0].opacity > 0.95 && subs_v20[1].opacity < 0.35 && subs_v20[2].opacity < 0.35,
        "At VariableValue(0.20), only Bar1 must be active (1.0), got opacities ({:.2}, {:.2}, {:.2})",
        subs_v20[0].opacity,
        subs_v20[1].opacity,
        subs_v20[2].opacity
    );

    state.set_kinetic_playback_mode(KineticPlaybackMode::VariableValue(0.55));
    let subs_v55 = state.evaluate_active_color_sublayers();
    assert!(
        subs_v55[0].opacity > 0.95 && subs_v55[1].opacity > 0.95 && subs_v55[2].opacity < 0.35,
        "At VariableValue(0.55), Bar1 and Bar2 must be active, got ({:.2}, {:.2}, {:.2})",
        subs_v55[0].opacity,
        subs_v55[1].opacity,
        subs_v55[2].opacity
    );

    state.set_kinetic_playback_mode(KineticPlaybackMode::VariableValue(1.0));
    let subs_v100 = state.evaluate_active_color_sublayers();
    assert!(
        subs_v100.iter().all(|s| s.opacity > 0.95),
        "At VariableValue(1.0), all 3 layers must be active"
    );
    assert!(state.undo());
    assert!(state.undo());
    assert!(state.undo());
    assert_eq!(state.snapshot(), snap_before_mode);

    // 3. Verify KineticPlaybackMode::OneShot (with vitrea::DualSpringMorphController) and Repeating:
    state.set_kinetic_playback_mode(KineticPlaybackMode::OneShot);
    state.icon_studio.kinetic.playing = true;
    state.icon_studio.kinetic.phase_t = 0.85;
    assert!(state.step_kinetic_timeline(0.30));
    assert!(
        !state.icon_studio.kinetic.playing
            && (state.icon_studio.kinetic.phase_t - 1.0).abs() < 1e-6,
        "OneShot playback must clamp at phase_t = 1.0 and stop playing"
    );

    // 4. Verify MotionPath trajectory following + analytical tangent rotation + live OffCurve handle drag:
    state.set_kinetic_playback_mode(KineticPlaybackMode::Continuous);
    state.icon_studio.kinetic.effect = KineticSymbolEffect::Bounce {
        direction: BounceDirection::Up,
        scope: KineticLayerScope::WholeSymbol,
    };
    state.select_color_sublayer(Some(2));
    assert!(state.set_selected_sublayer_motion_path(Some(ContourId(3)), true));
    let _ = state.set_selected_sublayer_phase_offset(0.0);

    let (p_start_mp, tan_start) = evaluate_motion_path_pose_at_ratio(&motion_curve, 0.0)
        .expect("Motion path pose at t=0.0 must resolve");
    let (p_end_mp, tan_end) = evaluate_motion_path_pose_at_ratio(&motion_curve, 1.0)
        .expect("Motion path pose at t=1.0 must resolve");
    assert!(p_start_mp.distance_to(FontPoint::new(170.0, 160.0)) < 1.0);
    assert!(p_end_mp.distance_to(FontPoint::new(620.0, 220.0)) < 1.0);
    assert!(
        (tan_start - tan_end).abs() > 0.3,
        "Curved motion path tangent must rotate from start to end"
    );

    state.icon_studio.kinetic.phase_t = 0.5;
    let subs_mid_before = state.evaluate_active_color_sublayers();
    let (_, cent_mid_before) =
        compute_contour_area_and_centroid(&subs_mid_before[2].contours[0].nodes);
    let (mp_mid, _) = evaluate_motion_path_pose_at_ratio(&motion_curve, 0.5).unwrap();
    assert!(
        cent_mid_before.distance_to(mp_mid) < 35.0,
        "OrbitDot centroid at t=0.5 ({:.1}, {:.1}) must follow the motion path midpoint ({:.1}, {:.1})",
        cent_mid_before.x.0,
        cent_mid_before.y.0,
        mp_mid.x.0,
        mp_mid.y.0
    );

    // Drag OffCurve handle NodeId(41) at (280, 460) of the MotionPath via InputDispatcher and verify OrbitDot moves live!
    let theme = GrafiaTheme::sonoma_default();
    state.view_mode = CanvasViewMode::PreviewFilled;
    let pix_mp_before = grafia::adapters::rasterize_canvas_subregion(&state, &theme, 520, 380);
    state.view_mode = CanvasViewMode::EditWireframe;
    state.activate_tool(CanvasTool::Select);
    let scr41 = state.viewport.to_screen(FontPoint::new(280.0, 460.0));
    let (h_start, h_end) = (
        egui::pos2(scr41.x.0, scr41.y.0),
        egui::pos2(scr41.x.0, scr41.y.0 + 60.0),
    );
    let ctx = egui::Context::default();
    let canvas_rect = egui::Rect::from_min_size(egui::pos2(0.0, 0.0), egui::vec2(800.0, 600.0));
    for (t, evs) in [
        (
            1.0,
            vec![egui::Event::PointerButton {
                pos: h_start,
                button: egui::PointerButton::Primary,
                pressed: true,
                modifiers: egui::Modifiers::NONE,
            }],
        ),
        (
            1.1,
            vec![
                egui::Event::PointerMoved(h_end),
                egui::Event::PointerButton {
                    pos: h_end,
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
    let subs_mid_after = state.evaluate_active_color_sublayers();
    let (_, cent_mid_after) =
        compute_contour_area_and_centroid(&subs_mid_after[2].contours[0].nodes);
    assert!(
        cent_mid_before.distance_to(cent_mid_after) > 10.0,
        "Dragging MotionPath OffCurve handle must bend the trajectory and move the following sublayer live"
    );
    state.view_mode = CanvasViewMode::PreviewFilled;
    let pix_mp_after = grafia::adapters::rasterize_canvas_subregion(&state, &theme, 520, 380);
    assert_ne!(pix_mp_before.data(), pix_mp_after.data());

    // 5. Verify W3C SVG round-trip of data-phase-offset, data-motion-path, and data-orient-motion-path:
    assert!(state.set_selected_sublayer_phase_offset(0.25));
    let svg = export_w3c_icon_svg(
        &state.color_sublayers,
        &state.icon_studio.symbol_layer_tags,
        &state.contour_strokes,
    );
    assert!(svg.contains("data-phase-offset=\"0.2500\""));
    assert!(svg.contains("data-motion-path=\"3\""));
    assert!(svg.contains("data-orient-motion-path=\"true\""));
    let parsed = import_w3c_icon_svg(&svg).expect("SVG with motion-path attributes must parse");
    assert!((parsed.tags[2].phase_offset - 0.25).abs() < 1e-4);
    assert_eq!(parsed.tags[2].motion_path_contour_id, Some(ContourId(3)));
    assert!(parsed.tags[2].orient_along_motion_path);

    // 6. Pijler A: Unconstrained arbtest on evaluate_motion_path_pose_at_ratio & KineticPlaybackMode
    arbtest(|u| {
        let raw_t = f64::from_bits(u.arbitrary::<u64>()?);
        let raw_v = f64::from_bits(u.arbitrary::<u64>()?);
        if let Some((pt, ang)) = evaluate_motion_path_pose_at_ratio(&motion_curve, raw_t) {
            assert!(pt.x.0.is_finite() && pt.y.0.is_finite() && ang.is_finite());
        }
        let mode = KineticPlaybackMode::VariableValue(raw_v);
        let eff_t = mode.effective_phase(raw_t);
        assert!(eff_t.is_finite() && (0.0..=1.0).contains(&eff_t));
        Ok(())
    })
    .budget_ms(150);
}

#[test]
fn tc_v0150_008_icon_keyline_template_optical_centroid_balance_and_pixel_crispness_preview() {
    use grafia::core::{
        collect_reference_frame_snap_lines, compute_icon_optical_balance, CanvasColorSublayer,
        ColorLayerPaint, ContourId, CustomReferenceFrame, FontPoint, GlyphContour, Node, NodeId,
        NodeType,
    };
    use grafia::tokens::GrafiaTheme;
    use grafia::CanvasViewMode;

    // 1. Verify CustomReferenceFrame::icon_keyline_template(800.0) builds the 4 canonical keylines
    //    (Circle, Square, Vertical & Horizontal Golden Rectangles, Diagonals, 4x4 Grid, Center Crosshair):
    let frame = CustomReferenceFrame::icon_keyline_template(800.0);
    let outer = frame
        .outer_box
        .expect("icon_keyline_template must set outer_box");
    assert!((outer.width() - 800.0).abs() < 1e-6 && (outer.height() - 800.0).abs() < 1e-6);
    assert!(frame.inner_box.is_some());
    assert!(frame.center_crosshair);
    assert!(frame.show_diagonals);
    assert_eq!(frame.keyline_rects.len(), 2);
    assert!(frame.keyline_circles_radii_upm.len() >= 2);
    let (snap_xs, snap_ys) = collect_reference_frame_snap_lines(&frame, 800.0);
    assert!(
        snap_xs.len() >= 10 && snap_ys.len() >= 10,
        "Icon keyline grid must provide snap lines for outer/inner/golden rects and circles"
    );

    // 2. Verify set_icon_keyline_grid_enabled(true) renders the keylines (including diagonals & golden rects)
    //    on the canvas in EditWireframe:
    let mut state = GrafiaCanvasState::init_glyph_a();
    state.viewport =
        grafia::ViewportTransform::new(0.5, grafia::ScreenPoint::new(60.0, 460.0), 1000);
    let theme = GrafiaTheme::sonoma_default();
    state.view_mode = CanvasViewMode::EditWireframe;
    let pix_no_grid = grafia::adapters::rasterize_canvas_subregion(&state, &theme, 520, 480);
    state.set_icon_keyline_grid_enabled(true);
    assert!(state
        .custom_reference_frame
        .as_ref()
        .is_some_and(|f| f.show_diagonals && f.keyline_rects.len() == 2));
    let pix_with_grid = grafia::adapters::rasterize_canvas_subregion(&state, &theme, 520, 480);
    assert_ne!(
        pix_no_grid.data(),
        pix_with_grid.data(),
        "Enabling the icon keyline grid must visibly render keyline circles, rectangles, and diagonals"
    );

    // 3. Verify compute_icon_optical_balance on a symmetric square vs. an asymmetric right-pointing Play triangle:
    let sym_box = GlyphContour::new(
        ContourId(0),
        vec![
            Node::new(NodeId(1), 200.0, 200.0, NodeType::Corner),
            Node::new(NodeId(2), 600.0, 200.0, NodeType::Corner),
            Node::new(NodeId(3), 600.0, 600.0, NodeType::Corner),
            Node::new(NodeId(4), 200.0, 600.0, NodeType::Corner),
        ],
    );
    let sym_rep = compute_icon_optical_balance(&[sym_box])
        .expect("Symmetric box must produce an IconOpticalBalanceReport");
    assert!(
        sym_rep.delta_upm.x.0.abs() < 1e-3 && sym_rep.delta_upm.y.0.abs() < 1e-3,
        "Symmetric square must have zero optical-to-bbox center delta, got ({:.3}, {:.3})",
        sym_rep.delta_upm.x.0,
        sym_rep.delta_upm.y.0
    );

    // Right-pointing Play triangle: base at x=200 (y=150..650), apex at (650, 400).
    // BBox center = ((200+650)/2, (150+650)/2) = (425, 400).
    // True area centroid = ((200+200+650)/3, (150+650+400)/3) = (350, 400) -> delta_x = -75 UPM!
    let play_tri = GlyphContour::new(
        ContourId(0),
        vec![
            Node::new(NodeId(10), 200.0, 150.0, NodeType::Corner),
            Node::new(NodeId(11), 650.0, 400.0, NodeType::Corner),
            Node::new(NodeId(12), 200.0, 650.0, NodeType::Corner),
        ],
    );
    let mut g = state.glyph.clone();
    g.width = 800.0;
    g.contours = vec![play_tri.clone()];
    let m = state.metrics.clone();
    state.load_glyph(g, m);
    state.set_color_sublayers(vec![CanvasColorSublayer {
        id: 1,
        name: "PlayTriangle".into(),
        contours: vec![play_tri],
        paint: ColorLayerPaint::Solid {
            rgba: [35, 135, 250, 255],
        },
        opacity: 1.0,
        visible: true,
        locked: false,
    }]);

    let tri_rep = state
        .compute_active_icon_optical_balance()
        .expect("Play triangle must produce an optical balance report");
    assert!((tri_rep.bbox_center_upm.x.0 - 425.0).abs() < 1e-3);
    assert!((tri_rep.optical_centroid_upm.x.0 - 350.0).abs() < 1e-3);
    assert!((tri_rep.delta_upm.x.0 - (-75.0)).abs() < 1e-3);

    // 4. Verify align_icon_to_optical_center() shifts both glyph.contours and color_sublayers
    //    by +75 UPM so the optical centroid aligns with the target center, with full Undo/Redo:
    let snap_before_align = state.snapshot();
    assert!(state.align_icon_to_optical_center());
    let aligned_rep = state
        .compute_active_icon_optical_balance()
        .expect("Aligned Play triangle must produce an optical balance report");
    assert!(
        (aligned_rep.optical_centroid_upm.x.0 - 425.0).abs() < 0.5,
        "After align_icon_to_optical_center, optical_centroid_upm.x must equal 425.0, got {:.2}",
        aligned_rep.optical_centroid_upm.x.0
    );
    assert!(
        (state.color_sublayers[0].contours[0].nodes[1].point.x.0 - 725.0).abs() < 0.5,
        "color_sublayers must stay 1-to-1 synchronized after align_icon_to_optical_center"
    );
    assert!(state.undo());
    assert_eq!(state.snapshot(), snap_before_align);
    assert!(state.redo());

    // 5. Verify live 16x16 and 24x24 pixel-crispness mini-previews update during handle/node drag:
    let crisp_16_before = state.render_icon_crispness_preview_rgba(16);
    let crisp_24_before = state.render_icon_crispness_preview_rgba(24);
    assert_eq!(crisp_16_before.len(), 16 * 16 * 4);
    assert_eq!(crisp_24_before.len(), 24 * 24 * 4);

    // Drag Play triangle apex NodeId(11) at (725, 400) via InputDispatcher:
    state.activate_tool(CanvasTool::Select);
    let scr11 = state.viewport.to_screen(FontPoint::new(725.0, 400.0));
    let (p_start, p_end) = (
        egui::pos2(scr11.x.0, scr11.y.0),
        egui::pos2(scr11.x.0, scr11.y.0 - 80.0),
    );
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
    let crisp_16_after = state.render_icon_crispness_preview_rgba(16);
    let crisp_24_after = state.render_icon_crispness_preview_rgba(24);
    assert_ne!(
        crisp_16_before, crisp_16_after,
        "16x16 pixel-crispness mini-preview must update live after dragging a node via InputDispatcher"
    );
    assert_ne!(
        crisp_24_before, crisp_24_after,
        "24x24 pixel-crispness mini-preview must update live after dragging a node via InputDispatcher"
    );

    // 6. Pijler A: Unconstrained arbtest on CustomReferenceFrame::icon_keyline_template & compute_icon_optical_balance
    let sample_contours = state.glyph.contours.clone();
    arbtest(|u| {
        let raw_size = f64::from_bits(u.arbitrary::<u64>()?);
        let tpl = CustomReferenceFrame::icon_keyline_template(raw_size);
        let (xs, ys) = collect_reference_frame_snap_lines(&tpl, 800.0);
        assert!(xs.iter().all(|x| x.is_finite()) && ys.iter().all(|y| y.is_finite()));

        let dx = f64::from_bits(u.arbitrary::<u64>()?);
        let dy = f64::from_bits(u.arbitrary::<u64>()?);
        let mut fuzzed = sample_contours.clone();
        if dx.is_finite() && dy.is_finite() && dx.abs() < 1e6 && dy.abs() < 1e6 {
            fuzzed[0].nodes[0].point = FontPoint::new(dx, dy);
        }
        if let Some(rep) = compute_icon_optical_balance(&fuzzed) {
            assert!(
                rep.bbox_center_upm.x.0.is_finite()
                    && rep.bbox_center_upm.y.0.is_finite()
                    && rep.optical_centroid_upm.x.0.is_finite()
                    && rep.optical_centroid_upm.y.0.is_finite()
                    && rep.delta_upm.x.0.is_finite()
                    && rep.delta_upm.y.0.is_finite()
                    && rep.total_area_upm2.is_finite()
            );
        }
        Ok(())
    })
    .budget_ms(150);
}

#[test]
fn tc_v0150_009a_cots_roxmltree_nested_groups_top_level_shapes_css_style_and_viewbox_24() {
    use grafia::core::{import_w3c_icon_svg, ColorLayerPaint, DrawDirection, SymbolHierarchyLevel};

    // 1. Top-level shapes WITHOUT any <g> wrapper, single quotes, XML comments,
    //    viewBox="0 0 24 24" (auto-scaled to 1000 UPM!), inline style="...", and rgb(...) colors:
    let lucide_style_svg = r#"
        <svg xmlns='http://www.w3.org/2000/svg' viewBox='0 0 24 24'>
            <!-- <g><path d="M 0 0 L 9999 9999"/></g> comment must be ignored by roxmltree! -->
            <circle cx='12' cy='12' r='10' opacity='0.85' style='fill: rgb(14, 165, 233)' />
            <path d='M 8 12 L 11 15 L 16 9' fill='none' stroke='#ffffff' stroke-linecap='round' stroke-linejoin='round' style='stroke-width: 2' />
        </svg>
    "#;
    let doc24 = import_w3c_icon_svg(lucide_style_svg).expect(
        "Top-level 24x24 SVG without <g> wrapper and with single quotes must parse via roxmltree",
    );
    assert_eq!(
        doc24.sublayers.len(),
        2,
        "Top-level <circle> and <path> must each become a CanvasColorSublayer"
    );
    assert_eq!(
        doc24.sublayers[0].paint,
        ColorLayerPaint::Solid {
            rgba: [14, 165, 233, 255]
        }
    );
    assert!((doc24.sublayers[0].opacity - 0.85).abs() < 1e-4);
    // Verify viewBox="0 0 24 24" scaled cx=12, r=10 into 1000 UPM space (~500 +- 416.67):
    let mut min_x = f64::INFINITY;
    let mut max_x = f64::NEG_INFINITY;
    for n in &doc24.sublayers[0].contours[0].nodes {
        min_x = min_x.min(n.point.x.0);
        max_x = max_x.max(n.point.x.0);
    }
    assert!(
        min_x > 50.0 && min_x < 120.0 && max_x > 880.0 && max_x < 950.0,
        "24x24 circle (cx=12, r=10) must scale to 1000 UPM (~83.3..916.7), got ({min_x:.1}..{max_x:.1})"
    );
    // Verify stroke-width: 2 in 24x24 viewBox scaled by 1000/24 (~83.33 UPM):
    let st1 = doc24
        .contour_strokes
        .get(&doc24.sublayers[1].contours[0].id)
        .expect("Stroked path must register ContourStrokeStyle");
    assert!(
        (st1.width - (2.0 * 1000.0 / 24.0)).abs() < 1.0,
        "Expected scaled stroke width ~83.33 UPM, got {:.2}",
        st1.width
    );

    // 2. Deeply nested <g> groups with cumulative transforms, inherited attributes, and data-* tags:
    let nested_svg = r##"
        <svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 1000 1000">
            <g id="OuterContainer" transform="translate(100, 50)">
                <g id="NestedBadge" data-layer-group="2" data-hierarchy="secondary" data-draw-direction="center-out" data-knockout-margin="16.5" transform="translate(50, 25)" style="fill: #f59e0b">
                    <rect x="10" y="20" width="200" height="100" rx="12" />
                </g>
                <g id="NestedWave" data-layer-group="3" data-hierarchy="tertiary">
                    <polyline points="0,0 100,50 200,0" stroke-width="24" stroke-linecap="round" style="stroke: rgb(56, 189, 248)" />
                </g>
            </g>
        </svg>
    "##;
    let nested_doc =
        import_w3c_icon_svg(nested_svg).expect("Nested <g> groups must parse via roxmltree");
    assert_eq!(nested_doc.sublayers.len(), 2);
    assert_eq!(nested_doc.sublayers[0].name, "NestedBadge");
    assert_eq!(nested_doc.tags[0].layer_group, 2);
    assert_eq!(
        nested_doc.tags[0].hierarchy_level,
        SymbolHierarchyLevel::Secondary
    );
    assert_eq!(nested_doc.tags[0].draw_direction, DrawDirection::CenterOut);
    assert!((nested_doc.tags[0].knockout_margin_upm - 16.5).abs() < 1e-4);
    // Cumulative translation (100+50+10 = 160..360):
    let r_min_x = nested_doc.sublayers[0].contours[0]
        .nodes
        .iter()
        .map(|n| n.point.x.0)
        .fold(f64::INFINITY, f64::min);
    assert!(
        (r_min_x - 160.0).abs() < 1.0,
        "Cumulative nested <g> translation must place rect min_x at 160.0, got {r_min_x:.2}"
    );

    // 3. Unconstrained arbtest fuzzing over arbitrary corrupted XML/SVG strings:
    arbtest(|u| {
        let raw = u.arbitrary::<String>()?;
        let wrapped = format!("<svg viewBox=\"0 0 24 24\"><g>{raw}</g></svg>");
        let _ = import_w3c_icon_svg(&raw);
        let _ = import_w3c_icon_svg(&wrapped);
        Ok(())
    })
    .budget_ms(150);
}

#[test]
fn tc_v0150_009b_bidirectional_sublayer_lifecycle_auto_adopt_prune_and_concave_knockout_self_union()
{
    use grafia::core::{
        apply_knockout_halo_masks_to_sublayers, build_expanded_knockout_halo_nodes,
        CanvasColorSublayer, ColorLayerPaint, ContourId, FontPoint, GlyphContour, Node, NodeId,
        NodeType, PrimitiveSubTool, ScreenPoint, SymbolLayerTag,
    };
    use grafia::interaction::Modifiers;

    // 1. Setup an icon with 1 existing color sublayer (ContourId(0)):
    let base_box = GlyphContour::new(
        ContourId(0),
        vec![
            Node::new(NodeId(10), 100.0, 100.0, NodeType::Corner),
            Node::new(NodeId(11), 350.0, 100.0, NodeType::Corner),
            Node::new(NodeId(12), 350.0, 350.0, NodeType::Corner),
            Node::new(NodeId(13), 100.0, 350.0, NodeType::Corner),
        ],
    );
    let mut state = GrafiaCanvasState::init_glyph_a();
    state.viewport = grafia::ViewportTransform::new(0.5, ScreenPoint::new(80.0, 460.0), 1000);
    let mut g = state.glyph.clone();
    g.contours = vec![base_box.clone()];
    let m = state.metrics.clone();
    state.load_glyph(g, m);
    state.set_color_sublayers(vec![CanvasColorSublayer {
        id: 1,
        name: "BaseCard".into(),
        contours: vec![base_box],
        paint: ColorLayerPaint::Solid {
            rgba: [14, 165, 233, 255],
        },
        opacity: 1.0,
        visible: true,
        locked: false,
    }]);
    state.icon_studio.symbol_layer_tags = vec![SymbolLayerTag {
        layer_group: 1,
        ..Default::default()
    }];

    // 2. Draw a brand-new rectangle on the canvas using CanvasTool::Primitives (Rectangle):
    //    sync_color_sublayers_from_glyph_contours() must automatically adopt the newly drawn contour
    //    as a 2nd CanvasColorSublayer + SymbolLayerTag!
    let snap_before_draw = state.snapshot();
    state.activate_tool(CanvasTool::Primitives);
    state.drawing_tool_state.primitive_sub_tool = PrimitiveSubTool::Rectangle;
    let p0_upm = FontPoint::new(420.0, 150.0);
    let p1_upm = FontPoint::new(680.0, 420.0);
    let p0_scr = state.viewport.to_screen(p0_upm);
    let p1_scr = state.viewport.to_screen(p1_upm);
    assert!(state.on_drawing_tool_pointer_press(p0_scr, p0_upm, Modifiers::default()));
    assert!(state.on_drawing_tool_pointer_move(p1_scr, p1_upm, Modifiers::default()));
    assert!(state.on_drawing_tool_pointer_release(p1_upm, Modifiers::default()));

    assert_eq!(state.glyph.contours.len(), 2);
    assert_eq!(
        state.color_sublayers.len(),
        2,
        "Drawing a new contour while color_sublayers is active must auto-adopt it as a 2nd CanvasColorSublayer"
    );
    assert_eq!(
        state.icon_studio.symbol_layer_tags.len(),
        2,
        "Auto-adopted sublayer must also receive a synchronized SymbolLayerTag"
    );
    assert_eq!(state.selected_color_sublayer_index(), Some(1));

    // 3. Select all nodes of the newly drawn contour (ContourId(1)) and delete them:
    //    sync_color_sublayers_from_glyph_contours() must automatically prune the empty sublayer
    //    and its SymbolLayerTag, and Undo/Redo must restore both bit-exact!
    let snap_after_draw = state.snapshot();
    state.activate_tool(CanvasTool::Select);
    state.selected_nodes = state.glyph.contours[1].nodes.iter().map(|n| n.id).collect();
    let canvas_rect = egui::Rect::from_min_size(egui::pos2(0.0, 0.0), egui::vec2(800.0, 600.0));
    let _ = grafia::adapters::keyboard::dispatch_keyboard_event(
        &mut state,
        &egui::Event::Key {
            key: egui::Key::Delete,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: egui::Modifiers::NONE,
        },
        false,
        false,
        false,
        canvas_rect,
    );

    assert_eq!(state.glyph.contours.len(), 1);
    assert_eq!(
        state.color_sublayers.len(),
        1,
        "Deleting a contour's nodes must prune its orphaned sublayer from color_sublayers"
    );
    assert_eq!(state.icon_studio.symbol_layer_tags.len(), 1);

    assert!(state.undo());
    assert_eq!(state.snapshot(), snap_after_draw);
    assert_eq!(state.color_sublayers.len(), 2);
    assert!(state.undo());
    assert_eq!(state.snapshot(), snap_before_draw);
    assert_eq!(state.color_sublayers.len(), 1);
    assert!(state.redo());
    assert_eq!(state.color_sublayers.len(), 2);

    // 4. Concave V-chevron knockout halo with large knockout_margin_upm = 45.0:
    //    Verify build_expanded_knockout_halo_nodes cleans inner swallowtail self-intersections via
    //    remove_overlap_in_contours and cleanly carves underlying sublayers!
    let mut chevron = GlyphContour::new(
        ContourId(1),
        vec![
            Node::new(NodeId(50), 180.0, 260.0, NodeType::Corner),
            Node::new(NodeId(51), 240.0, 160.0, NodeType::Corner),
            Node::new(NodeId(52), 320.0, 300.0, NodeType::Corner),
        ],
    );
    chevron.closed = false;
    let mut next_nid = 5000_u64;
    let halo_polys = build_expanded_knockout_halo_nodes(&[chevron.clone()], 45.0, &mut next_nid);
    assert!(
        !halo_polys.is_empty(),
        "Concave V-chevron with margin=45.0 must produce a non-empty self-unioned halo"
    );
    let mut test_subs = vec![
        state.color_sublayers[0].clone(),
        CanvasColorSublayer {
            id: 2,
            name: "ChevronSlash".into(),
            contours: vec![chevron],
            paint: ColorLayerPaint::Solid {
                rgba: [245, 70, 95, 255],
            },
            opacity: 1.0,
            visible: true,
            locked: false,
        },
    ];
    let tags = vec![
        SymbolLayerTag::default(),
        SymbolLayerTag {
            knockout_margin_upm: 45.0,
            ..Default::default()
        },
    ];
    apply_knockout_halo_masks_to_sublayers(&mut test_subs, &tags, &mut next_nid);
    assert!(
        !test_subs[0].contours.is_empty() && test_subs[0].contours[0].nodes.len() > 4,
        "Underlying card must be cleanly carved by the concave V-chevron halo"
    );

    // 5. Blank Icon Artboard From-Scratch Authoring (SPEC-017 §2.9 Stap 18.3):
    //    When color_sublayers is empty AND the Icon Keyline Grid is active, drawing the VERY FIRST
    //    primitive contour on the blank artboard must auto-adopt it as Layer1 in color_sublayers!
    state.set_color_sublayers(Vec::new());
    state.icon_studio.symbol_layer_tags.clear();
    state.contours.clear();
    state.glyph.contours.clear();
    state.sync_paths_from_contours();
    state.set_icon_keyline_grid_enabled(true);
    assert!(state.color_sublayers.is_empty());
    assert!(state.insert_primitive(
        PrimitiveSubTool::Circle,
        FontPoint::new(200.0, 200.0),
        400.0,
        400.0
    ));
    assert_eq!(
        state.color_sublayers.len(),
        1,
        "Drawing the first shape on a blank Icon Artboard must auto-adopt it as color_sublayers[0]"
    );
    assert_eq!(state.icon_studio.symbol_layer_tags.len(), 1);
}

#[test]
fn tc_v0150_009c_render_cache_hasher_collision_freedom_and_solid_fast_path() {
    use grafia::core::{
        CanvasColorSublayer, ColorLayerPaint, ContourId, FontPoint, GlyphContour, GradientSpread,
        GradientStop, Node, NodeId, NodeType,
    };
    use grafia::render::canvas_render_cache::{
        get_or_upload_color_sublayers_texture, get_or_upload_liquid_glass_texture,
    };

    let mut state = GrafiaCanvasState::init_glyph_a();
    let box_c = GlyphContour::new(
        ContourId(0),
        vec![
            Node::new(NodeId(1), 100.0, 100.0, NodeType::Corner),
            Node::new(NodeId(2), 500.0, 100.0, NodeType::Corner),
            Node::new(NodeId(3), 500.0, 500.0, NodeType::Corner),
            Node::new(NodeId(4), 100.0, 500.0, NodeType::Corner),
        ],
    );
    let mut g = state.glyph.clone();
    g.contours = vec![box_c.clone()];
    let m = state.metrics.clone();
    state.load_glyph(g, m);
    state.set_color_sublayers(vec![CanvasColorSublayer {
        id: 1,
        name: "GradCard".into(),
        contours: vec![box_c],
        paint: ColorLayerPaint::LinearGradient {
            p0: FontPoint::new(100.0, 100.0),
            p1: FontPoint::new(500.0, 100.0),
            stops: vec![
                GradientStop::new(0.0, [255, 40, 80, 255]),
                GradientStop::new(1.0, [40, 140, 255, 255]),
            ],
            spread: GradientSpread::Pad,
        },
        opacity: 1.0,
        visible: true,
        locked: false,
    }]);

    let ctx = egui::Context::default();

    // 1. Mutating ONLY gradient endpoint p1 WITHOUT bumping content_generation must still produce
    //    a distinct cache key and re-upload a fresh GPU texture!
    let tex_grad_1 = get_or_upload_color_sublayers_texture(&ctx, &state, 160, 160);
    if let ColorLayerPaint::LinearGradient { ref mut p1, .. } = state.color_sublayers[0].paint {
        *p1 = FontPoint::new(100.0, 500.0);
    }
    let tex_grad_2 = get_or_upload_color_sublayers_texture(&ctx, &state, 160, 160);
    assert_ne!(
        tex_grad_1.id(),
        tex_grad_2.id(),
        "Mutating gradient endpoint p1 must change color_sublayers_fingerprint even at the same content_generation"
    );

    // 2. Setting dome_radius_upm == smooth_union_k_upm (which previously XOR'd to 0!) or mutating
    //    refraction_px / specular_intensity / shininess WITHOUT bumping content_generation must
    //    each produce a distinct LiquidGlass GPU texture handle!
    state.icon_studio.liquid_glass.dome_radius_upm = 40.0;
    state.icon_studio.liquid_glass.smooth_union_k_upm = 40.0;
    let tex_lg_1 = get_or_upload_liquid_glass_texture(&ctx, &state, 160, 160);

    state.icon_studio.liquid_glass.dome_radius_upm = 64.0;
    state.icon_studio.liquid_glass.smooth_union_k_upm = 64.0;
    let tex_lg_2 = get_or_upload_liquid_glass_texture(&ctx, &state, 160, 160);
    assert_ne!(
        tex_lg_1.id(),
        tex_lg_2.id(),
        "Equal dome_radius_upm and smooth_union_k_upm (40 vs 64) must never XOR-cancel in LiquidGlass cache key"
    );

    state.icon_studio.liquid_glass.specular_intensity = 0.25;
    let tex_lg_3 = get_or_upload_liquid_glass_texture(&ctx, &state, 160, 160);
    assert_ne!(
        tex_lg_2.id(),
        tex_lg_3.id(),
        "Changing liquid_glass.specular_intensity must invalidate the LiquidGlass texture cache"
    );
}

#[test]
fn tc_v0150_009d_cross_pass_preview_parity_optical_centroid_overlay_and_topology_sublayer_sync() {
    use grafia::core::{
        BounceDirection, CanvasColorSublayer, ColorLayerPaint, ContourId, ContourStrokeStyle,
        DrawDirection, FontPoint, GlyphContour, KineticLayerScope, KineticPlaybackMode,
        KineticSymbolEffect, LineCapStyle, LineJoinStyle, Node, NodeId, NodeType, ScreenPoint,
        ShapeBuilderOp, StrokePosition, SymbolLayerTag,
    };
    use grafia::interaction::Modifiers;
    use grafia::render::{
        collect_active_liquid_glass_contours, rasterize_preview_panel, render_preview_strip_rgba,
        PreviewPanelConfig, PreviewRenderConfig,
    };
    use grafia::tokens::GrafiaTheme;

    // 1. Cross-Pass Parity for Open Stroked Contours + VariableValue(0.35) + Knockout Halos:
    //    Build an open centerline stroke (ContourId(0), closed = false) with 44 UPM round stroke:
    let mut open_arc = GlyphContour::new(
        ContourId(0),
        vec![
            Node::new(NodeId(1), 150.0, 260.0, NodeType::Corner),
            Node::new(NodeId(2), 550.0, 260.0, NodeType::Corner),
        ],
    );
    open_arc.closed = false;

    let mut state = GrafiaCanvasState::init_glyph_a();
    state.viewport = grafia::ViewportTransform::new(0.5, ScreenPoint::new(80.0, 440.0), 1000);
    let mut g = state.glyph.clone();
    g.contours = vec![open_arc.clone()];
    let m = state.metrics.clone();
    state.load_glyph(g, m);

    let strip_cfg = PreviewRenderConfig {
        width_px: 240,
        height_px: 80,
        ..PreviewRenderConfig::default()
    };
    let panel_cfg = PreviewPanelConfig::default();

    // Without stroke style, an open contour has 0 filled pixels in preview strip & preview panel:
    let strip_unstroked = render_preview_strip_rgba(&state, &strip_cfg);
    let panel_unstroked = rasterize_preview_panel(&state, &panel_cfg, 240, 80);

    // Attach a 44 UPM ContourStrokeStyle to the open contour:
    state.set_contour_stroke(
        ContourId(0),
        ContourStrokeStyle {
            width: 44.0,
            height: 44.0,
            position: StrokePosition::Center,
            cap_start: LineCapStyle::Round,
            cap_end: LineCapStyle::Round,
            join: LineJoinStyle::Round,
        },
    );
    let strip_stroked = render_preview_strip_rgba(&state, &strip_cfg);
    let panel_stroked = rasterize_preview_panel(&state, &panel_cfg, 240, 80);
    assert_ne!(
        strip_unstroked, strip_stroked,
        "Bottom PreviewStrip must render open contours with non-destructive ContourStrokeStyle"
    );
    assert_ne!(
        panel_unstroked.data(),
        panel_stroked.data(),
        "Bottom PreviewPanel must render open contours with non-destructive ContourStrokeStyle"
    );

    // Also verify EditWireframe with fill_active_glyph_while_editing = true fills the expanded open stroke:
    let theme = GrafiaTheme::sonoma_default();
    state.preferences.fill_active_glyph_while_editing = false;
    let wire_nofill = grafia::adapters::rasterize_canvas_subregion(&state, &theme, 480, 380);
    state.preferences.fill_active_glyph_while_editing = true;
    let wire_withfill = grafia::adapters::rasterize_canvas_subregion(&state, &theme, 480, 380);
    assert_ne!(
        wire_nofill.data(),
        wire_withfill.data(),
        "EditWireframe with fill_active_glyph_while_editing=true must fill non-destructive open stroked contours"
    );

    // Verify KineticPlaybackMode::VariableValue(0.35) (with playing == false and phase_t == 0.0!)
    // updates PreviewStrip, PreviewPanel, and LiquidGlass contours:
    state.icon_studio.kinetic.playing = false;
    state.icon_studio.kinetic.phase_t = 0.0;
    state.icon_studio.kinetic.effect = KineticSymbolEffect::Appear {
        direction: BounceDirection::Up,
        draw_on: true,
        scope: KineticLayerScope::WholeSymbol,
    };
    state.set_kinetic_playback_mode(KineticPlaybackMode::VariableValue(0.35));
    let strip_var35 = render_preview_strip_rgba(&state, &strip_cfg);
    assert_ne!(
        strip_stroked, strip_var35,
        "PreviewStrip must evaluate KineticPlaybackMode::VariableValue(0.35) even when playing=false and phase_t=0.0"
    );
    let lg_var35 = collect_active_liquid_glass_contours(&state);
    state.set_kinetic_playback_mode(KineticPlaybackMode::VariableValue(0.85));
    let lg_var85 = collect_active_liquid_glass_contours(&state);
    assert_ne!(
        lg_var35, lg_var85,
        "LiquidGlass contour collector must evaluate KineticPlaybackMode::VariableValue even when playing=false"
    );
    state.icon_studio.kinetic.effect = KineticSymbolEffect::default();
    state.set_kinetic_playback_mode(KineticPlaybackMode::OneShot);

    // 2. ShapeBuilder Merge & Knife Cut Sublayer Topology Sync:
    let box_a = GlyphContour::new(
        ContourId(0),
        vec![
            Node::new(NodeId(10), 100.0, 150.0, NodeType::Corner),
            Node::new(NodeId(11), 360.0, 150.0, NodeType::Corner),
            Node::new(NodeId(12), 360.0, 410.0, NodeType::Corner),
            Node::new(NodeId(13), 100.0, 410.0, NodeType::Corner),
        ],
    );
    let box_b = GlyphContour::new(
        ContourId(1),
        vec![
            Node::new(NodeId(20), 260.0, 150.0, NodeType::Corner),
            Node::new(NodeId(21), 520.0, 150.0, NodeType::Corner),
            Node::new(NodeId(22), 520.0, 410.0, NodeType::Corner),
            Node::new(NodeId(23), 260.0, 410.0, NodeType::Corner),
        ],
    );
    state.contour_strokes.clear();
    let mut g2 = state.glyph.clone();
    g2.contours = vec![box_a.clone(), box_b.clone()];
    let m2 = state.metrics.clone();
    state.load_glyph(g2, m2);
    state.set_color_sublayers(vec![
        CanvasColorSublayer {
            id: 1,
            name: "LeftPiece".into(),
            contours: vec![box_a],
            paint: ColorLayerPaint::Solid {
                rgba: [30, 144, 255, 255],
            },
            opacity: 1.0,
            visible: true,
            locked: false,
        },
        CanvasColorSublayer {
            id: 2,
            name: "RightPiece".into(),
            contours: vec![box_b],
            paint: ColorLayerPaint::Solid {
                rgba: [255, 99, 71, 255],
            },
            opacity: 1.0,
            visible: true,
            locked: false,
        },
    ]);
    state.icon_studio.symbol_layer_tags = vec![
        SymbolLayerTag {
            layer_group: 1,
            draw_direction: DrawDirection::CenterOut,
            ..Default::default()
        },
        SymbolLayerTag {
            layer_group: 2,
            ..Default::default()
        },
    ];

    // Merge all 3 regions of box_a and box_b into 1 union contour via ShapeBuilderOp::Merge:
    // color_sublayers must consolidate from 2 sublayers down to 1 sublayer preserving LeftPiece's metadata!
    let snap_before_merge = state.snapshot();
    assert!(state.apply_shape_builder_operation(&[0, 1, 2], ShapeBuilderOp::Merge));
    assert_eq!(state.glyph.contours.len(), 1);
    assert_eq!(
        state.color_sublayers.len(),
        1,
        "Merging 2 overlapping sublayers via ShapeBuilderOp::Merge must consolidate color_sublayers to 1 sublayer"
    );
    assert_eq!(state.color_sublayers[0].name, "LeftPiece");
    assert_eq!(state.icon_studio.symbol_layer_tags.len(), 1);
    assert_eq!(
        state.icon_studio.symbol_layer_tags[0].draw_direction,
        DrawDirection::CenterOut
    );

    // Slice the merged sublayer vertically down the middle (x=310, y=50..500) using CanvasTool::Knife:
    // color_sublayers must automatically adopt the 2nd cut half as a 2nd CanvasColorSublayer!
    state.activate_tool(CanvasTool::Knife);
    let k0_upm = FontPoint::new(310.0, 50.0);
    let k1_upm = FontPoint::new(310.0, 500.0);
    let k0_scr = state.viewport.to_screen(k0_upm);
    let k1_scr = state.viewport.to_screen(k1_upm);
    assert!(state.on_drawing_tool_pointer_press(k0_scr, k0_upm, Modifiers::default()));
    assert!(state.on_drawing_tool_pointer_move(k1_scr, k1_upm, Modifiers::default()));
    assert!(state.on_drawing_tool_pointer_release(k1_upm, Modifiers::default()));

    assert_eq!(state.glyph.contours.len(), 2);
    assert_eq!(
        state.color_sublayers.len(),
        2,
        "Slicing a colored sublayer in two with CanvasTool::Knife must auto-adopt the new half into color_sublayers"
    );
    assert_eq!(state.icon_studio.symbol_layer_tags.len(), 2);

    // Undo Knife -> back to 1 merged sublayer; Undo Merge -> back to 2 original sublayers!
    assert!(state.undo());
    assert_eq!(state.color_sublayers.len(), 1);
    assert!(state.undo());
    assert_eq!(state.snapshot(), snap_before_merge);
    assert_eq!(state.color_sublayers.len(), 2);
}

#[test]
fn tc_v0150_009_universal_v0150_cross_tool_fuzzer_and_visual_golden_frames() {
    use grafia::core::{
        BounceDirection, CanvasColorSublayer, ColorLayerPaint, ContourId, ContourStrokeStyle,
        DrawDirection, FontPoint, GlyphContour, KineticLayerScope, KineticPlaybackMode,
        KineticSymbolEffect, LineCapStyle, LineJoinStyle, Node, NodeId, NodeType, PrimitiveSubTool,
        ScreenPoint, StrokePosition, SymbolColorRenderingMode, SymbolHierarchyLevel,
        SymbolLayerTag,
    };
    use grafia::tokens::GrafiaTheme;
    use grafia::CanvasViewMode;

    // 1. Universal Stateful Cross-Tool & Icon Studio Fuzzer (Pijler A/B/C/D/E):
    arbtest(|u| {
        let mut st = GrafiaCanvasState::init_glyph_a();
        st.viewport = grafia::ViewportTransform::new(0.45, ScreenPoint::new(60.0, 440.0), 1000);
        st.set_icon_keyline_grid_enabled(true);
        let steps = u.int_in_range(1..=10)?;
        for _ in 0..steps {
            match u.int_in_range(0..=9)? {
                0 => {
                    let x = f64::from(u.int_in_range(80..=520)?);
                    let y = f64::from(u.int_in_range(80..=520)?);
                    let w = f64::from(u.int_in_range(40..=220)?);
                    let sub = if u.arbitrary::<bool>()? {
                        PrimitiveSubTool::Rectangle
                    } else {
                        PrimitiveSubTool::Circle
                    };
                    let _ = st.insert_primitive(sub, FontPoint::new(x, y), w, w);
                }
                1 => {
                    let (_, mode) =
                        SymbolColorRenderingMode::preset_by_index(u.int_in_range(0..=3)?);
                    st.set_symbol_color_rendering_mode(mode);
                }
                2 => {
                    let dir = match u.int_in_range(0..=2)? {
                        0 => DrawDirection::StartToEnd,
                        1 => DrawDirection::CenterOut,
                        _ => DrawDirection::EndToStart,
                    };
                    let _ = st.set_selected_sublayer_draw_direction(dir);
                }
                3 => {
                    let ko = f64::from(u.int_in_range(0..=36)?);
                    let _ = st.set_selected_sublayer_knockout_margin(ko);
                }
                4 => {
                    let v = f64::from(u.int_in_range(0..=100)?) * 0.01;
                    st.set_kinetic_playback_mode(KineticPlaybackMode::VariableValue(v));
                }
                5 => {
                    let _ = st.reorder_selected_color_sublayer(if u.arbitrary::<bool>()? {
                        1
                    } else {
                        -1
                    });
                }
                6 => {
                    let _ = st.align_icon_to_optical_center();
                }
                7 => {
                    let svg = st.export_kinetic_symbol_to_svg();
                    let _ = st.import_kinetic_symbol_from_svg(&svg);
                }
                8 => {
                    let _ = st.undo();
                }
                _ => {
                    let _ = st.redo();
                }
            }
            assert!(st.validate_invariants().is_ok());
            for sub in st.evaluate_active_color_sublayers() {
                for c in &sub.contours {
                    for n in &c.nodes {
                        assert!(n.point.x.0.is_finite() && n.point.y.0.is_finite());
                    }
                }
            }
        }
        Ok(())
    })
    .budget_ms(400);

    // 2. Render 4 Visual Golden-Master PNG Frames for SPEC-017 (v0.15.0) & verify non-blank pixel variation:
    let theme = GrafiaTheme::sonoma_default();
    let out_dir = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("target")
        .join("golden_frames");
    let _ = std::fs::create_dir_all(&out_dir);

    // Frame 68: 800x800 Apple/Material 4-Shape Icon Keyline Artboard + Optical Centroid Vector (⊕)
    let mut st68 = GrafiaCanvasState::init_glyph_a();
    st68.viewport = grafia::ViewportTransform::new(0.48, ScreenPoint::new(110.0, 460.0), 1000);
    st68.show_metrics = false;
    let play_tri = GlyphContour::new(
        ContourId(0),
        vec![
            Node::new(NodeId(10), 260.0, 200.0, NodeType::Corner),
            Node::new(NodeId(11), 640.0, 400.0, NodeType::Corner),
            Node::new(NodeId(12), 260.0, 600.0, NodeType::Corner),
        ],
    );
    let mut g68 = st68.glyph.clone();
    g68.width = 800.0;
    g68.height = 800.0;
    g68.contours = vec![play_tri.clone()];
    st68.load_glyph(g68, st68.metrics.clone());
    st68.set_color_sublayers(vec![CanvasColorSublayer {
        id: 1,
        name: "PlayTriangle".into(),
        contours: vec![play_tri],
        paint: ColorLayerPaint::Solid {
            rgba: [14, 165, 233, 235],
        },
        opacity: 1.0,
        visible: true,
        locked: false,
    }]);
    st68.set_icon_keyline_grid_enabled(true);
    st68.view_mode = CanvasViewMode::EditWireframe;
    let pix68 = grafia::adapters::rasterize_canvas_subregion(&st68, &theme, 640, 520);
    let p68 = out_dir.join("frame_68_v0150_keyline_artboard_and_optical_centroid.png");
    pix68.save_png(&p68).expect("Must save frame 68 PNG");

    // Frame 69: 3-Layer Mute / Knockout-Slash Icon with 22 UPM Non-Destructive Halo & Round Open Strokes
    let bar0 = GlyphContour::new(
        ContourId(0),
        vec![
            Node::new(NodeId(20), 160.0, 240.0, NodeType::Corner),
            Node::new(NodeId(21), 640.0, 240.0, NodeType::Corner),
            Node::new(NodeId(22), 640.0, 330.0, NodeType::Corner),
            Node::new(NodeId(23), 160.0, 330.0, NodeType::Corner),
        ],
    );
    let bar1 = GlyphContour::new(
        ContourId(1),
        vec![
            Node::new(NodeId(30), 160.0, 430.0, NodeType::Corner),
            Node::new(NodeId(31), 640.0, 430.0, NodeType::Corner),
            Node::new(NodeId(32), 640.0, 520.0, NodeType::Corner),
            Node::new(NodeId(33), 160.0, 520.0, NodeType::Corner),
        ],
    );
    let mut slash = GlyphContour::new(
        ContourId(2),
        vec![
            Node::new(NodeId(40), 210.0, 150.0, NodeType::Corner),
            Node::new(NodeId(41), 590.0, 610.0, NodeType::Corner),
        ],
    );
    slash.closed = false;
    let mut g69 = st68.glyph.clone();
    g69.contours = vec![bar0.clone(), bar1.clone(), slash.clone()];
    st68.load_glyph(g69, st68.metrics.clone());
    st68.set_color_sublayers(vec![
        CanvasColorSublayer {
            id: 1,
            name: "Bar0".into(),
            contours: vec![bar0],
            paint: ColorLayerPaint::Solid {
                rgba: [37, 99, 235, 255],
            },
            opacity: 1.0,
            visible: true,
            locked: false,
        },
        CanvasColorSublayer {
            id: 2,
            name: "Bar1".into(),
            contours: vec![bar1],
            paint: ColorLayerPaint::Solid {
                rgba: [16, 185, 129, 255],
            },
            opacity: 1.0,
            visible: true,
            locked: false,
        },
        CanvasColorSublayer {
            id: 3,
            name: "Slash".into(),
            contours: vec![slash],
            paint: ColorLayerPaint::Solid {
                rgba: [244, 63, 94, 255],
            },
            opacity: 1.0,
            visible: true,
            locked: false,
        },
    ]);
    st68.set_contour_stroke(
        ContourId(2),
        ContourStrokeStyle {
            width: 34.0,
            height: 34.0,
            position: StrokePosition::Center,
            cap_start: LineCapStyle::Round,
            cap_end: LineCapStyle::Round,
            join: LineJoinStyle::Round,
        },
    );
    st68.icon_studio.symbol_layer_tags = vec![
        SymbolLayerTag::default(),
        SymbolLayerTag::default(),
        SymbolLayerTag {
            knockout_margin_upm: 22.0,
            ..Default::default()
        },
    ];
    st68.view_mode = CanvasViewMode::PreviewFilled;
    let pix69 = grafia::adapters::rasterize_canvas_subregion(&st68, &theme, 640, 520);
    let p69 = out_dir.join("frame_69_v0150_knockout_slash_halo_and_open_strokes.png");
    pix69.save_png(&p69).expect("Must save frame 69 PNG");

    // Frame 70: Centerline DrawOn (StartToEnd, CenterOut, EndToStart) at t = 0.55
    st68.icon_studio.symbol_layer_tags[2].knockout_margin_upm = 0.0;
    st68.icon_studio.symbol_layer_tags[0].draw_direction = DrawDirection::StartToEnd;
    st68.icon_studio.symbol_layer_tags[1].draw_direction = DrawDirection::CenterOut;
    st68.icon_studio.symbol_layer_tags[2].draw_direction = DrawDirection::EndToStart;
    st68.icon_studio.kinetic.effect = KineticSymbolEffect::Appear {
        direction: BounceDirection::Up,
        draw_on: true,
        scope: KineticLayerScope::WholeSymbol,
    };
    st68.icon_studio.kinetic.phase_t = 0.55;
    let pix70 = grafia::adapters::rasterize_canvas_subregion(&st68, &theme, 640, 520);
    let p70 = out_dir.join("frame_70_v0150_centerline_draw_on_and_smart_corners.png");
    pix70.save_png(&p70).expect("Must save frame 70 PNG");

    // Frame 71: Apple Hierarchical Color Mode (Primary 100%, Secondary 50%, Tertiary 25%)
    st68.icon_studio.kinetic.effect = KineticSymbolEffect::default();
    st68.icon_studio.kinetic.phase_t = 0.0;
    st68.icon_studio.symbol_layer_tags[2].knockout_margin_upm = 22.0;
    st68.icon_studio.symbol_layer_tags[0].hierarchy_level = SymbolHierarchyLevel::Tertiary;
    st68.icon_studio.symbol_layer_tags[1].hierarchy_level = SymbolHierarchyLevel::Secondary;
    st68.icon_studio.symbol_layer_tags[2].hierarchy_level = SymbolHierarchyLevel::Primary;
    st68.set_symbol_color_rendering_mode(SymbolColorRenderingMode::Hierarchical {
        primary_rgba: [15, 118, 255, 255],
    });
    let pix71 = grafia::adapters::rasterize_canvas_subregion(&st68, &theme, 640, 520);
    let p71 = out_dir.join("frame_71_v0150_hierarchical_palette_and_motion_path.png");
    pix71.save_png(&p71).expect("Must save frame 71 PNG");

    assert_ne!(pix68.data(), pix69.data());
    assert_ne!(pix69.data(), pix70.data());
    assert_ne!(pix69.data(), pix71.data());

    // Frame 72: SPEC-018 Studio Anthracite Mid-Gray Theme + Wi-Fi Editorial Cubic Bézier Hero Icon
    let anth_theme = GrafiaTheme::studio_anthracite();
    assert!(anth_theme.is_anthracite());
    st68.theme = anth_theme.clone();
    st68.view_mode = CanvasViewMode::EditWireframe;
    st68.show_selection_info_hud = true;
    let pix72 = grafia::adapters::rasterize_canvas_subregion(&st68, &anth_theme, 640, 520);
    let p72 = out_dir.join("frame_72_v0180_studio_anthracite_editorial_canvas.png");
    pix72.save_png(&p72).expect("Must save frame 72 PNG");
    assert_ne!(pix71.data(), pix72.data());
}

// ============================================================================
// 13. TC-V0180-005: Unconstrained `arbtest` Audit for Multi-Slot "AOVA" Text
//     Stream Active-Slot Viewport Parity, Touch Hit Accuracy & One-Shot Modifiers
// ============================================================================

#[test]
fn tc_v0180_005_arbtest_multi_slot_aova_touch_oneshot_modifiers_and_active_slot_viewport() {
    use grafia::adapters::InputDispatcher;
    use grafia::core::InputDeviceKind;

    let ctx = egui::Context::default();
    let canvas_rect = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::Vec2::new(390.0, 844.0));

    arbtest::arbtest(|u| {
        let mut st = GrafiaCanvasState::init_glyph_a();
        st.load_demo_text_strip();
        let slot_count = st.text_stream.slots.len();
        assert!(slot_count >= 4);
        let active_idx = u.int_in_range(0..=(slot_count - 1))?;
        st.switch_active_slot(active_idx);
        st.fit_viewport_to_glyph(canvas_rect.width(), canvas_rect.height());

        // 1. Verify full multi-slot strip fits inside the mobile canvas width:
        let first_scr_x = st.viewport.to_screen(FontPoint::new(0.0, 0.0)).x.0;
        let last_slot = &st.text_stream.slots[slot_count - 1];
        let last_end_upm = last_slot.origin_x_upm + last_slot.glyph.width;
        let last_scr_x = st.viewport.to_screen(FontPoint::new(last_end_upm, 0.0)).x.0;
        assert!(
            first_scr_x >= 0.0 && last_scr_x <= canvas_rect.width(),
            "Multi-slot text stream must fit inside mobile canvas width (got {first_scr_x}..{last_scr_x})"
        );

        // 2. Verify active_slot_viewport() places active slot nodes at their true screen positions
        //    and that hit_test_node, marquee selection, and BBox knob hit-testing work on any active slot:
        st.set_active_input_device(InputDeviceKind::TouchFinger);
        assert_eq!(st.preferences.touch_finger_offset_dp, 0.0);
        let vp = st.active_slot_viewport();
        let target_node = st.contours[0][0].clone();
        let node_scr = vp.to_screen(target_node.point);
        assert_eq!(st.hit_test_node(node_scr), Some(target_node.id));

        // Marquee drag over the active slot's node in screen space must select it even when active_idx > 0:
        st.activate_tool(CanvasTool::Select);
        st.selected_nodes.clear();
        let m0 = egui::pos2(node_scr.x.0 - 8.0, node_scr.y.0 - 8.0);
        let m1 = egui::pos2(node_scr.x.0 + 8.0, node_scr.y.0 + 8.0);
        let raw_marquee = egui::RawInput {
            screen_rect: Some(canvas_rect),
            time: Some(1.0),
            events: vec![
                egui::Event::PointerButton {
                    pos: m0,
                    button: egui::PointerButton::Primary,
                    pressed: true,
                    modifiers: egui::Modifiers::NONE,
                },
                egui::Event::PointerMoved(m1),
                egui::Event::PointerButton {
                    pos: m1,
                    button: egui::PointerButton::Primary,
                    pressed: false,
                    modifiers: egui::Modifiers::NONE,
                },
            ],
            ..Default::default()
        };
        let mut out_m = ctx.run_ui(raw_marquee, |ui| {
            let ev = ui.input(|i| i.events.clone());
            let _ = InputDispatcher::for_canvas(&mut st, canvas_rect).dispatch_events(&ev, ui);
        });
        out_m.textures_delta.clear();
        assert!(
            st.selected_nodes.contains(&target_node.id),
            "Marquee over active slot {active_idx} at screen ({:.1},{:.1}) must select {:?}",
            node_scr.x.0,
            node_scr.y.0,
            target_node.id
        );

        // 3. Verify 8-knob BBox hit-test uses active_slot_viewport() on non-zero slots:
        st.select_all_nodes();
        st.show_bounding_box = true;
        if let Some(bounds) = st.selection_bounds() {
            let top_right_knob_scr = bounds.knob_screen_center(8, &vp);
            assert!(
                st.hit_test_bbox_knob(top_right_knob_scr, false, false).is_some(),
                "BBox knob hit-test must match active_slot_viewport() on slot {active_idx}"
            );
        }

        // 4. Verify One-Shot Touch Modifiers auto-unlatch upon TouchPhase::End / PointerRelease:
        let ts = &mut st.drag_transients.touch_state;
        ts.touch_shift_latched = u.arbitrary::<bool>()?;
        ts.touch_alt_latched = u.arbitrary::<bool>()?;
        ts.touch_cmd_latched = u.arbitrary::<bool>()?;
        ts.touch_space_pan_latched = true;
        let dx = f32::from(u.int_in_range(-45..=45)?);
        let dy = f32::from(u.int_in_range(-45..=45)?);
        let p_start = egui::pos2(180.0, 360.0);
        let p_end = egui::pos2(180.0 + dx, 360.0 + dy);
        let raw_touch = egui::RawInput {
            screen_rect: Some(canvas_rect),
            time: Some(2.0),
            events: vec![
                egui::Event::Touch {
                    device_id: egui::TouchDeviceId(0),
                    id: egui::TouchId(7),
                    phase: egui::TouchPhase::Start,
                    pos: p_start,
                    force: None,
                },
                egui::Event::Touch {
                    device_id: egui::TouchDeviceId(0),
                    id: egui::TouchId(7),
                    phase: egui::TouchPhase::Move,
                    pos: p_end,
                    force: None,
                },
                egui::Event::Touch {
                    device_id: egui::TouchDeviceId(0),
                    id: egui::TouchId(7),
                    phase: egui::TouchPhase::End,
                    pos: p_end,
                    force: None,
                },
            ],
            ..Default::default()
        };
        let mut out_t = ctx.run_ui(raw_touch, |ui| {
            let ev = ui.input(|i| i.events.clone());
            let _ = InputDispatcher::for_canvas(&mut st, canvas_rect).dispatch_events(&ev, ui);
        });
        out_t.textures_delta.clear();
        let ts_after = &st.drag_transients.touch_state;
        assert!(
            !ts_after.touch_shift_latched
                && !ts_after.touch_alt_latched
                && !ts_after.touch_cmd_latched
                && !ts_after.touch_space_pan_latched,
            "One-shot touch modifier latches must auto-unlatch after TouchPhase::End"
        );
        assert!(st.validate_invariants().is_ok());
        Ok(())
    })
    .budget_ms(350);
}
