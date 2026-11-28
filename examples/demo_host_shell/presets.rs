//! `SPEC-018 §2` Host-Port Event Polling, 3-Tier `StudioThemePreset`, and the
//! 36-Case (`6×6`) Investor & Systems-Engineering Keynote Showcase for `MockFontHost`.

use super::MockFontHost;
use egui_macview::{MacAppearance, MacTheme};
use grafia::adapters::{CanvasTool, GrafiaCanvasState};
use grafia::core::{
    Affine2D, CanvasComponent, CanvasSmartHandle, ComponentId, ContourStrokeStyle, FontPoint,
    LineCapStyle, LineJoinStyle, StrokePosition,
};
use grafia::tokens::{AppearanceMode, GlyphArchetype, GrafiaTheme};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum StudioThemePreset {
    #[default]
    PaperLight,
    StudioAnthracite,
    SonomaDark,
}

impl StudioThemePreset {
    pub const ALL: [Self; 3] = [Self::PaperLight, Self::StudioAnthracite, Self::SonomaDark];
    pub const LABELS: [&'static str; 3] = [
        "Thema: Papier Licht",
        "Thema: Studio Grijs",
        "Thema: Sonoma Donker",
    ];

    #[must_use]
    pub const fn mac_appearance(self) -> MacAppearance {
        match self {
            Self::PaperLight => MacAppearance::Aqua,
            Self::StudioAnthracite => MacAppearance::Anthracite,
            Self::SonomaDark => MacAppearance::DarkAqua,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DemoPreset {
    EditorialContoursAndInfoBox,
    RulersGuidesAndBgLayer,
    TextStreamAovaKerning,
    VerticalCjkStreamAndFrame,
    AnchorsAndDiacriticCloud,
    SmartComponentsAndDecompose,
    G2HarmonizeAndGeniusNodes,
    G3ClothoidEulerSpiral,
    TunniLinesAndHandleRatios,
    CurvatureCombSpikesAndHeatmap,
    RedArrowsOutlineAudit,
    HobbySplinesAndPowerNudge,
    SmartCornersFilletsAndInkTraps,
    Calligraphic6DofNibBrush,
    StructuralStemGaugeAndTool,
    IntersectionRulerAndRainbow,
    TrowelSmoothingAndCleanerBrush,
    OpticalRhythmOvershootAndCentroid,
    MultiMasterCompatAndAutoMatch,
    CurvedMasterTrajectories,
    HigherOrderInterpolationHoi,
    TrueTypeVisualInstructor16Ppem,
    PostScriptStemAndGhostHints,
    BitmapScanAndAutoTrace,
    IconKeylineArtboardAndOpticalPlay,
    IconPlanarShapeBuilderMerge,
    IconAsymmetricVariableWidthPins,
    IconRadialSymmetryAndOblique,
    IconColrV1MultiStopGradients,
    IconLiquidGlassAndBakeColrV1,
    KineticWifiBounceAndColorModes,
    KineticCenterlineDrawOnOff,
    KineticMicrophoneKnockoutHalo,
    KineticOrbitMotionPathAndSensor,
    KineticMagicReplaceAndSvgRoundtrip,
    BimanualThumbPieAndHardwareDials,
}

use DemoPreset as P;

impl DemoPreset {
    #[rustfmt::skip]
    pub const ALL: [(Self, &'static str); 36] = [
        (P::EditorialContoursAndInfoBox, "01. /A & /O Contour & InfoBox"),
        (P::RulersGuidesAndBgLayer, "02. Linialen, Guides & Achtergrond"),
        (P::TextStreamAovaKerning, "03. Tekststrook \"AOVA\" & Kerning"),
        (P::VerticalCjkStreamAndFrame, "04. Verticale CJK-Stroom & 3×3 Kader"),
        (P::AnchorsAndDiacriticCloud, "05. Ankers & Accent-Wolk (Cmd+U)"),
        (P::SmartComponentsAndDecompose, "06. Smart Components & wght-Schuif"),
        (P::G2HarmonizeAndGeniusNodes, "07. G² Harmonisatie & Genius (★)"),
        (P::G3ClothoidEulerSpiral, "08. G³ Clothoïde / Euler-Spiraal (❖)"),
        (P::TunniLinesAndHandleRatios, "09. Tunni-Lijnen & 55% Ratio-Badges"),
        (P::CurvatureCombSpikesAndHeatmap, "10. Krommingskam (Spikes & Heatmap)"),
        (P::RedArrowsOutlineAudit, "11. Rode-Pijlen Contour-Audit"),
        (P::HobbySplinesAndPowerNudge, "12. Hobby-Splines & Servo Power-Nudge"),
        (P::SmartCornersFilletsAndInkTraps, "13. Smart Corners & Inktvallen"),
        (P::Calligraphic6DofNibBrush, "14. 6-DoF Druk/Tilt Kalligrafie-Penseel"),
        (P::StructuralStemGaugeAndTool, "15. Stamdikte-Meter & Stem-Tool"),
        (P::IntersectionRulerAndRainbow, "16. Snijpunten-Meetlat & Regenboog"),
        (P::TrowelSmoothingAndCleanerBrush, "17. Strijk-Troffel & Gum-Borstel"),
        (P::OpticalRhythmOvershootAndCentroid, "18. Optisch Ritme & Zwaartepunt (⊕)"),
        (P::MultiMasterCompatAndAutoMatch, "19. Multi-Master Stralen & Auto-Match"),
        (P::CurvedMasterTrajectories, "20. Gebogen Master-Trajecten"),
        (P::HigherOrderInterpolationHoi, "21. Hogere-Orde Interpolatie (HOI)"),
        (P::TrueTypeVisualInstructor16Ppem, "22. TrueType Instructor & 16px Grid"),
        (P::PostScriptStemAndGhostHints, "23. PostScript Stam- & Ghost-Hints"),
        (P::BitmapScanAndAutoTrace, "24. Scan-Onderlegger & Auto-Trace"),
        (P::IconKeylineArtboardAndOpticalPlay, "25. 800×800 Keylines & Play-Optica (⊕)"),
        (P::IconPlanarShapeBuilderMerge, "26. Planaire Vormenbouwer (Merge)"),
        (P::IconAsymmetricVariableWidthPins, "27. Variabele Lijndikte-Pinnen"),
        (P::IconRadialSymmetryAndOblique, "28. 6× Kompas-Rozet & +12° Oblique"),
        (P::IconColrV1MultiStopGradients, "29. COLRv1 Multi-Stop Kleurverlopen"),
        (P::IconLiquidGlassAndBakeColrV1, "30. Vitrea Liquid Glass → Bake COLRv1"),
        (P::KineticWifiBounceAndColorModes, "31. Wi-Fi Signaal & 4 Apple Kleurmodi"),
        (P::KineticCenterlineDrawOnOff, "32. Hartlijn DrawOn (Midden↔Uit)"),
        (P::KineticMicrophoneKnockoutHalo, "33. Microfoon Knockout-Slash (22 UPM)"),
        (P::KineticOrbitMotionPathAndSensor, "34. Satelliet-Baan (MotionPath & Sensor)"),
        (P::KineticMagicReplaceAndSvgRoundtrip, "35. MagicReplace Morph & 2-Weg SVG"),
        (P::BimanualThumbPieAndHardwareDials, "36. Liquid Glass Pie Menu & Haptics"),
    ];
}

impl MockFontHost {
    #[must_use]
    pub fn active_mac_theme(&self) -> MacTheme {
        MacTheme::new(self.studio_theme.mac_appearance())
    }

    pub fn apply_studio_theme(
        &mut self,
        preset: StudioThemePreset,
        state: &mut GrafiaCanvasState,
        theme: &mut GrafiaTheme,
    ) {
        self.studio_theme = preset;
        let (mode, next_theme) = match preset {
            StudioThemePreset::PaperLight => (AppearanceMode::Light, GrafiaTheme::sonoma_default()),
            StudioThemePreset::StudioAnthracite => {
                (AppearanceMode::Dark, GrafiaTheme::studio_anthracite())
            }
            StudioThemePreset::SonomaDark => (AppearanceMode::Dark, GrafiaTheme::sonoma_dark()),
        };
        state.set_appearance_with_system(mode, mode == AppearanceMode::Dark);
        *theme = next_theme.clone();
        state.theme = next_theme;
        state.bump_content_generation();
    }

    pub fn apply_preset(
        &mut self,
        preset: DemoPreset,
        state: &mut GrafiaCanvasState,
        theme: &mut GrafiaTheme,
    ) {
        if let Some(idx) = DemoPreset::ALL.iter().position(|(p, _)| *p == preset) {
            self.tour_index = idx;
            self.active_workspace = super::sonoma_top_bar::StudioSubWorkspace::from_tour_index(idx);
        }
        self.active_chapter = super::coach_card(preset).chapter;
        self.studio_mode = if self.tour_index >= 24
            && preset != P::IconColrV1MultiStopGradients
            && preset != P::BimanualThumbPieAndHardwareDials
        {
            super::StudioWorkspaceMode::IconStudio
        } else {
            super::StudioWorkspaceMode::LetterStudio
        };
        state.show_selection_info_hud = true;
        self.dispatch_preset_domain(preset, state, theme);
    }

    fn dispatch_preset_domain(
        &mut self,
        preset: DemoPreset,
        state: &mut GrafiaCanvasState,
        _theme: &mut GrafiaTheme,
    ) {
        match preset {
            P::EditorialContoursAndInfoBox => self.apply_toggle_glyph_preset(state),
            P::RulersGuidesAndBgLayer => self.apply_rulers_and_bg_preset(state),
            P::TextStreamAovaKerning => {
                self.ensure_letter_glyph(state, GlyphArchetype::A);
                self.text_buffer = "AOVA".to_owned();
                let slots = self.build_slots_for_text(&self.text_buffer);
                state.activate_tool(CanvasTool::Text);
                state.set_text_stream_slots_with_caret(slots, 1, 2);
                state.show_text_measurement_line = true;
                self.fit_canvas_viewport(state);
                self.push_log("Case 03: Tekststrook \"AOVA\" met live kerning en meetlijn actief");
            }
            P::VerticalCjkStreamAndFrame => self.apply_vertical_text_and_ref_frame_preset(state),
            P::AnchorsAndDiacriticCloud => {
                self.ensure_letter_glyph(state, GlyphArchetype::A);
                state.reset_default_anchors();
                self.poll_host_ports(state);
            }
            P::SmartComponentsAndDecompose => self.apply_smart_components_case(state),
            P::G2HarmonizeAndGeniusNodes
            | P::G3ClothoidEulerSpiral
            | P::TunniLinesAndHandleRatios
            | P::CurvatureCombSpikesAndHeatmap
            | P::RedArrowsOutlineAudit
            | P::HobbySplinesAndPowerNudge => self.apply_cagd_domain_case(preset, state),
            P::SmartCornersFilletsAndInkTraps
            | P::Calligraphic6DofNibBrush
            | P::StructuralStemGaugeAndTool
            | P::IntersectionRulerAndRainbow
            | P::TrowelSmoothingAndCleanerBrush
            | P::OpticalRhythmOvershootAndCentroid => self.apply_stems_domain_case(preset, state),
            P::MultiMasterCompatAndAutoMatch
            | P::CurvedMasterTrajectories
            | P::HigherOrderInterpolationHoi
            | P::TrueTypeVisualInstructor16Ppem
            | P::PostScriptStemAndGhostHints
            | P::BitmapScanAndAutoTrace => self.apply_masters_and_hinting_case(preset, state),
            _ => self.apply_icon_and_kinetic_domain_case(preset, state),
        }
    }

    pub(crate) fn ensure_letter_glyph(&self, st: &mut GrafiaCanvasState, arch: GlyphArchetype) {
        st.select_color_sublayer(None);
        st.color_sublayers.clear();
        st.contour_strokes.clear();
        st.icon_studio.kinetic.playing = false;
        st.icon_studio.kinetic.phase_t = 0.0;
        st.icon_studio.radial_symmetry.fold_count = 1;
        st.set_icon_keyline_grid_enabled(false);
        st.show_metrics = true;
        st.load_glyph_archetype(arch);
        st.show_selection_info_hud = true;
        st.glyph.name = if arch == GlyphArchetype::A { "A" } else { "O" }.into();
        self.fit_canvas_viewport(st);
    }

    fn apply_toggle_glyph_preset(&mut self, state: &mut GrafiaCanvasState) {
        let next = if state.active_archetype == GlyphArchetype::A {
            GlyphArchetype::O
        } else {
            GlyphArchetype::A
        };
        self.ensure_letter_glyph(state, next);
        if let Some(first_id) = state.contours.first().and_then(|c| c.first()).map(|n| n.id) {
            state.selected_nodes = vec![first_id];
        }
        self.push_log(format!(
            "Case 01: Editorial contour /{} met zwevende InfoBox",
            state.glyph.name
        ));
    }

    fn apply_rulers_and_bg_preset(&mut self, state: &mut GrafiaCanvasState) {
        self.ensure_letter_glyph(state, GlyphArchetype::A);
        state.show_rulers = true;
        state.select_all_nodes();
        state.copy_selection_to_background();
        state.show_background = true;
        for gc in &mut state.background_contours {
            for n in &mut gc.nodes {
                (n.point.x.0, n.point.y.0) = (n.point.x.0 + 18.0, n.point.y.0 + 12.0);
            }
        }
        state.add_guide_from_selection(false, FontPoint::new(300.0, 250.0));
        if let Some(c) = state.glyph.contours.first() {
            state.set_contour_stroke(
                c.id,
                ContourStrokeStyle {
                    width: 18.0,
                    height: 18.0,
                    position: StrokePosition::Center,
                    cap_start: LineCapStyle::Round,
                    cap_end: LineCapStyle::Round,
                    join: LineJoinStyle::Round,
                },
            );
        }
        state.sync_paths_from_contours();
        self.push_log("Case 02: 20dp Linialen + Referentie-Achtergrondlaag + Hulplijn actief");
    }

    fn apply_smart_components_case(&mut self, state: &mut GrafiaCanvasState) {
        self.ensure_letter_glyph(state, GlyphArchetype::A);
        if state.components.is_empty() {
            let cs = self
                .glyph_library
                .get("acutecomb")
                .map_or_else(Vec::new, |g| g.contours.clone());
            state.set_components(vec![CanvasComponent {
                id: ComponentId(1),
                component_name: "acutecomb".into(),
                contours: cs,
                transform: Affine2D::translation(330.0, 560.0),
                locked: false,
                automatic_alignment: true,
                selected: true,
            }]);
            state.set_smart_handles(vec![CanvasSmartHandle {
                component_id: ComponentId(1),
                axis_id: "wght".into(),
                axis_name: "StemWeight".into(),
                anchor_upm: FontPoint::new(140.0, 360.0),
                direction_unit: (1.0, 0.0),
                scale_upm_per_unit: 2.0,
                min_val: 40.0,
                default_val: 90.0,
                max_val: 160.0,
                current_val: 95.0,
            }]);
            self.push_log("Case 06: Smart Component 'acutecomb' + on-canvas wght-schuif actief");
        } else {
            let _ = state.decompose_selected_components();
            self.push_log("Case 06: Component ontleed naar bewerkbare voorgrond-contouren");
        }
    }
}
