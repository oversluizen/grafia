//! `SPEC-016`..`SPEC-018` Interactive Demo Showcase Presets & Hero Templates for Domains V & VI:
//! Vector, Kleur, Liquid Glass & Kinetische SF-Symbolen (`Cases 25..36`).

use super::icon_hero_templates::{
    build_compass_rosette_petal, build_microphone_knockout_hero_icon,
    build_orbit_motion_path_hero_icon, build_play_optical_hero_icon, build_wifi_hero_icon,
    make_ellipse_contour, solid_sublayer, EDITORIAL_SLATE_BLUE,
};
use super::{DemoPreset, MockFontHost};
use grafia::adapters::{CanvasTool, CanvasViewMode, GrafiaCanvasState};
use grafia::core::{
    ContourId, FontPoint, GlyphContour, KineticPlaybackMode, KineticSymbolEffect, LineCapStyle,
    Node, NodeId, NodeType, ShapeBuilderOp, SmartCornerSpec, VariableWidthStrokeSpec,
    WidthNodeSample,
};

pub(crate) const DEMO_OBLIQUE_DEG: f64 = 12.0;
pub(crate) const DEMO_SYMMETRY_CENTER: FontPoint = FontPoint::new(400.0, 400.0);
pub(crate) const DEMO_KINETIC_DEFAULT_PHASE: f64 = 0.45;
pub(crate) const DEMO_DT_MIN_SEC: f32 = 0.001;
pub(crate) const DEMO_DT_MAX_SEC: f32 = 0.05;

pub(crate) fn demo_kinetic_effect_by_idx(idx: usize) -> KineticSymbolEffect {
    let targets = if idx >= 9 {
        vec![
            MockFontHost::make_rect_contour(0, 800, 140.0, 180.0, 250.0, 560.0),
            MockFontHost::make_rect_contour(1, 810, 290.0, 180.0, 400.0, 560.0),
        ]
    } else {
        Vec::new()
    };
    KineticSymbolEffect::preset_by_index(idx, targets)
}

impl MockFontHost {
    pub fn apply_icon_and_kinetic_domain_case(
        &mut self,
        preset: DemoPreset,
        state: &mut GrafiaCanvasState,
    ) {
        use DemoPreset as P;
        match preset {
            P::IconKeylineArtboardAndOpticalPlay => self.apply_play_optical_balance_preset(state),
            P::IconPlanarShapeBuilderMerge => self.apply_shape_builder_and_var_width_preset(state),
            P::IconAsymmetricVariableWidthPins => self.apply_var_width_only_case(state),
            P::IconRadialSymmetryAndOblique => self.apply_symmetry_oblique_and_clean_preset(state),
            P::IconColrV1MultiStopGradients => self.apply_color_layers_colrv1_preset(state),
            P::IconLiquidGlassAndBakeColrV1 => self.apply_liquid_glass_case(state),
            P::KineticWifiBounceAndColorModes => self.apply_kinetic_symbol_timeline_preset(state),
            P::KineticCenterlineDrawOnOff => {
                self.apply_kinetic_symbol_timeline_preset(state);
                state.icon_studio.kinetic.effect = demo_kinetic_effect_by_idx(4);
                state.icon_studio.kinetic.phase_t = 0.62;
                state.bump_content_generation();
                self.push_log("Case 32: SF Symbols 7 Centerline DrawOn (Midden↔Uit)");
            }
            P::KineticMicrophoneKnockoutHalo => self.apply_mute_slash_knockout_preset(state),
            P::KineticOrbitMotionPathAndSensor => self.apply_orbit_motion_path_preset(state),
            P::KineticMagicReplaceAndSvgRoundtrip => {
                self.apply_kinetic_symbol_timeline_preset(state);
                state.icon_studio.kinetic.effect = demo_kinetic_effect_by_idx(9);
                state.icon_studio.kinetic.phase_t = 0.50;
                let svg = state.export_kinetic_symbol_to_svg();
                let _ = state.import_kinetic_symbol_from_svg(&svg);
                self.push_log("Case 35: MagicReplace Morph + 2-Weg SVG Roundtrip");
            }
            _ => self.apply_controller_rotary_and_haptics_preset(state),
        }
    }

    pub fn apply_icon_studio_preset(&mut self, preset: DemoPreset, state: &mut GrafiaCanvasState) {
        self.apply_icon_and_kinetic_domain_case(preset, state);
    }

    fn reset_icon_demo_canvas(
        &self,
        st: &mut GrafiaCanvasState,
        name: &str,
        cs: Vec<GlyphContour>,
    ) {
        st.select_color_sublayer(None);
        st.color_sublayers.clear();
        st.contour_strokes.clear();
        st.curve_harmony.smart_corners.clear();
        st.icon_studio.variable_width_strokes.clear();
        st.icon_studio.symbol_layer_tags.clear();
        (
            st.icon_studio.kinetic.playing,
            st.icon_studio.kinetic.phase_t,
        ) = (false, 0.0);
        st.icon_studio.kinetic.playback_mode = KineticPlaybackMode::OneShot;
        st.icon_studio.radial_symmetry.fold_count = 1;
        (st.view_mode, st.show_metrics, st.show_selection_info_hud) =
            (CanvasViewMode::default(), false, true);
        st.activate_tool(CanvasTool::Select);
        let mut g = st.glyph.clone();
        (g.name, g.width, g.height, g.contours) = (name.into(), 800.0, 800.0, cs);
        st.load_glyph(g, st.metrics.clone());
        st.set_icon_keyline_grid_enabled(true);
        self.fit_canvas_viewport(st);
    }

    pub fn apply_blank_icon_artboard_preset(&mut self, st: &mut GrafiaCanvasState) {
        self.reset_icon_demo_canvas(st, "icon.blank", Vec::new());
        st.preferences.fill_active_glyph_while_editing = true;
        st.activate_tool(CanvasTool::Primitives);
        st.bump_content_generation();
        self.push_log("Icon Studio: Nieuw 800×800 Keyline Artboard");
    }

    fn load_hero_icon_spec(
        &self,
        st: &mut GrafiaCanvasState,
        name: &str,
        spec: super::icon_hero_templates::HeroIconSpec,
        sel_sub: usize,
        sel_nodes: Vec<NodeId>,
    ) {
        let (contours, sublayers, tags, strokes) = spec;
        self.reset_icon_demo_canvas(st, name, contours);
        st.set_color_sublayers(sublayers);
        for (cid, style) in strokes {
            st.set_contour_stroke(cid, style);
        }
        st.icon_studio.symbol_layer_tags = tags;
        st.select_color_sublayer(Some(sel_sub));
        st.selected_nodes = sel_nodes;
        st.bump_content_generation();
    }

    pub fn apply_mute_slash_knockout_preset(&mut self, st: &mut GrafiaCanvasState) {
        self.load_hero_icon_spec(
            st,
            "icon.muteslash",
            build_microphone_knockout_hero_icon(),
            3,
            vec![NodeId(440), NodeId(441)],
        );
        self.push_log("Case 33: Microfoon Knockout-Slash (Halo = 22 UPM)");
    }

    pub fn apply_play_optical_balance_preset(&mut self, st: &mut GrafiaCanvasState) {
        let (contours, sublayers, tags, corners) = build_play_optical_hero_icon();
        self.reset_icon_demo_canvas(st, "icon.play.optical", contours);
        st.set_color_sublayers(sublayers);
        st.icon_studio.symbol_layer_tags = tags;
        for sc in corners {
            st.set_smart_corner(sc);
        }
        st.select_color_sublayer(Some(1));
        st.selected_nodes = vec![NodeId(520), NodeId(521), NodeId(522)];
        st.preferences.fill_active_glyph_while_editing = true;
        st.bump_content_generation();
        self.push_log("Case 25: Play-Cirkel op 800×800 Keylines (⊕)");
    }

    pub fn apply_orbit_motion_path_preset(&mut self, st: &mut GrafiaCanvasState) {
        self.load_hero_icon_spec(
            st,
            "icon.orbit.motion",
            build_orbit_motion_path_hero_icon(),
            2,
            vec![NodeId(640), NodeId(641), NodeId(642)],
        );
        st.icon_studio.kinetic.effect = demo_kinetic_effect_by_idx(0);
        st.icon_studio.kinetic.phase_t = 0.22;
        st.set_kinetic_playback_mode(KineticPlaybackMode::VariableValue(0.65));
        st.bump_content_generation();
        self.push_log("Case 34: Satelliet-Baan (MotionPath) + Sensor %");
    }

    fn apply_var_width_only_case(&mut self, st: &mut GrafiaCanvasState) {
        let mk_n = |id, x, y, t| Node::new(NodeId(id), x, y, t);
        let nodes = vec![
            mk_n(150, 140.0, 260.0, NodeType::Corner),
            mk_n(151, 310.0, 620.0, NodeType::OffCurve),
            mk_n(152, 490.0, 140.0, NodeType::OffCurve),
            mk_n(153, 660.0, 520.0, NodeType::Corner),
        ];
        let mut spine = GlyphContour::new(ContourId(0), nodes);
        spine.closed = false;
        self.reset_icon_demo_canvas(st, "icon.varwidth", vec![spine]);
        st.set_variable_width_stroke(Self::default_demo_var_width_spec(ContourId(0)));
        st.selected_nodes = vec![NodeId(150), NodeId(153)];
        st.bump_content_generation();
        self.push_log("Case 27: Variabele Lijndikte-Pinnen (14 → 42/28 → 10 UPM)");
    }

    fn apply_liquid_glass_case(&mut self, st: &mut GrafiaCanvasState) {
        if st.view_mode == CanvasViewMode::LiquidGlass {
            let _ = st.bake_liquid_glass_to_colrv1();
            st.view_mode = CanvasViewMode::default();
            self.push_log("Case 30: Vitrea Liquid Glass gebakken naar vector COLRv1");
        } else {
            self.apply_kinetic_symbol_timeline_preset(st);
            st.view_mode = CanvasViewMode::LiquidGlass;
            self.push_log("Case 30: Vitrea Liquid Glass → klik voor Bake COLRv1");
        }
    }

    pub(crate) fn apply_shape_builder_and_var_width_preset(&mut self, st: &mut GrafiaCanvasState) {
        if st.active_tool == CanvasTool::ShapeBuilder && st.glyph.contours.len() >= 2 {
            let _ = st.apply_shape_builder_operation(&[0, 1], ShapeBuilderOp::Merge);
            self.push_log("Case 26: Planaire regio's atomair samengesmolten");
            return;
        }
        let c0 = make_ellipse_contour(0, 100, 315.0, 425.0, 145.0, 145.0);
        let c1 = make_ellipse_contour(1, 120, 465.0, 425.0, 145.0, 145.0);
        self.reset_icon_demo_canvas(st, "icon.shapebuilder", vec![c0, c1]);
        st.activate_tool(CanvasTool::ShapeBuilder);
        let _ = st.on_drawing_tool_pointer_move(
            grafia::ScreenPoint::new(300.0, 300.0),
            FontPoint::new(390.0, 425.0),
            grafia::Modifiers::default(),
        );
        self.push_log("Case 26: Planaire Vormenbouwer (ShapeBuilder) actief");
    }

    pub(crate) fn apply_symmetry_oblique_and_clean_preset(&mut self, st: &mut GrafiaCanvasState) {
        if st.icon_studio.radial_symmetry.fold_count <= 1 {
            let p = build_compass_rosette_petal();
            self.reset_icon_demo_canvas(st, "icon.rosette", vec![p.clone()]);
            st.set_color_sublayers(vec![solid_sublayer(
                1,
                "RosettePetal",
                p,
                EDITORIAL_SLATE_BLUE,
            )]);
            st.set_smart_corner(SmartCornerSpec {
                node_id: NodeId(203),
                radius_upm: 16.0,
            });
            st.selected_nodes = vec![NodeId(203)];
            (
                st.icon_studio.radial_symmetry.fold_count,
                st.icon_studio.radial_symmetry.center,
            ) = (6, DEMO_SYMMETRY_CENTER);
            st.bump_content_generation();
            self.push_log("Case 28: 6× Kompas-Rozet & SmartCorner pin");
        } else {
            let _ = (
                st.bake_radial_symmetry(),
                st.apply_optical_oblique(DEMO_OBLIQUE_DEG, true),
                st.normalize_icon_contours(Some(1.0)),
            );
            self.push_log("Case 28: 6× Symmetrie gebakken + +12° Oblique");
        }
    }

    pub(crate) fn apply_kinetic_symbol_timeline_preset(&mut self, st: &mut GrafiaCanvasState) {
        self.load_hero_icon_spec(
            st,
            "wifi.dot",
            build_wifi_hero_icon(),
            0,
            vec![NodeId(300), NodeId(303), NodeId(320), NodeId(323)],
        );
        st.icon_studio.kinetic.effect = demo_kinetic_effect_by_idx(1);
        st.icon_studio.kinetic.phase_t = DEMO_KINETIC_DEFAULT_PHASE;
        st.bump_content_generation();
        self.push_log("Case 31: Wi-Fi Signaal (gedempt editorial kleurenpalet)");
    }

    pub(crate) fn default_demo_var_width_spec(contour_id: ContourId) -> VariableWidthStrokeSpec {
        let mk = |t, l, r| WidthNodeSample {
            arc_ratio_t: t,
            width_left_upm: l,
            width_right_upm: r,
        };
        let samples = vec![
            mk(0.0, 14.0, 14.0),
            mk(0.5, 42.0, 28.0),
            mk(1.0, 10.0, 10.0),
        ];
        VariableWidthStrokeSpec {
            contour_id,
            samples,
            cap_style: LineCapStyle::Round,
        }
    }
}
