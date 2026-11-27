//! `SPEC-018 §2` Guided Showcase: 6 Domeinen × 6 Cases = 36-Case Keynote Feature Matrix,
//! `ShowcaseCoachCard` & Stap-voor-Stap Keynote Navigator.

use super::guided_showcase_cards::{COACH_CARDS, TOOL_DUTCH_GUIDES};
use super::{DemoPreset, MockFontHost};
use egui::{RichText, ScrollArea, Ui};
use grafia::adapters::{CanvasTool, GrafiaCanvasState};
use grafia::tokens::GrafiaTheme;

/// De 6 architectuur-domeinen waarin alle 36 showcase-cases (`6×6`) van `grafia` zijn gegroepeerd.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ShowcaseChapter {
    BasisTekenen,
    CurvenEnAudit,
    KalligrafieEnStems,
    TekstEnMasters,
    IconStudioEnAnimatie,
    HostStekkersEnHardware,
}

const CH_1_PRESETS: [DemoPreset; 6] = [
    DemoPreset::EditorialContoursAndInfoBox,
    DemoPreset::RulersGuidesAndBgLayer,
    DemoPreset::TextStreamAovaKerning,
    DemoPreset::VerticalCjkStreamAndFrame,
    DemoPreset::AnchorsAndDiacriticCloud,
    DemoPreset::SmartComponentsAndDecompose,
];

const CH_2_PRESETS: [DemoPreset; 6] = [
    DemoPreset::G2HarmonizeAndGeniusNodes,
    DemoPreset::G3ClothoidEulerSpiral,
    DemoPreset::TunniLinesAndHandleRatios,
    DemoPreset::CurvatureCombSpikesAndHeatmap,
    DemoPreset::RedArrowsOutlineAudit,
    DemoPreset::HobbySplinesAndPowerNudge,
];

const CH_3_PRESETS: [DemoPreset; 6] = [
    DemoPreset::SmartCornersFilletsAndInkTraps,
    DemoPreset::Calligraphic6DofNibBrush,
    DemoPreset::StructuralStemGaugeAndTool,
    DemoPreset::IntersectionRulerAndRainbow,
    DemoPreset::TrowelSmoothingAndCleanerBrush,
    DemoPreset::OpticalRhythmOvershootAndCentroid,
];

const CH_4_PRESETS: [DemoPreset; 6] = [
    DemoPreset::MultiMasterCompatAndAutoMatch,
    DemoPreset::CurvedMasterTrajectories,
    DemoPreset::HigherOrderInterpolationHoi,
    DemoPreset::TrueTypeVisualInstructor16Ppem,
    DemoPreset::PostScriptStemAndGhostHints,
    DemoPreset::BitmapScanAndAutoTrace,
];

const CH_5_PRESETS: [DemoPreset; 6] = [
    DemoPreset::IconKeylineArtboardAndOpticalPlay,
    DemoPreset::IconPlanarShapeBuilderMerge,
    DemoPreset::IconAsymmetricVariableWidthPins,
    DemoPreset::IconRadialSymmetryAndOblique,
    DemoPreset::IconColrV1MultiStopGradients,
    DemoPreset::IconLiquidGlassAndBakeColrV1,
];

const CH_6_PRESETS: [DemoPreset; 6] = [
    DemoPreset::KineticWifiBounceAndColorModes,
    DemoPreset::KineticCenterlineDrawOnOff,
    DemoPreset::KineticMicrophoneKnockoutHalo,
    DemoPreset::KineticOrbitMotionPathAndSensor,
    DemoPreset::KineticMagicReplaceAndSvgRoundtrip,
    DemoPreset::BimanualThumbPieAndHardwareDials,
];

impl ShowcaseChapter {
    pub const ALL: [Self; 6] = [
        Self::BasisTekenen,
        Self::CurvenEnAudit,
        Self::KalligrafieEnStems,
        Self::TekstEnMasters,
        Self::IconStudioEnAnimatie,
        Self::HostStekkersEnHardware,
    ];

    pub const LETTER_STUDIO_TABS: [(Self, &'static str); 4] = [
        (Self::BasisTekenen, "I. Typografie"),
        (Self::CurvenEnAudit, "II. CAGD Bochten"),
        (Self::KalligrafieEnStems, "III. Stems & Optica"),
        (Self::TekstEnMasters, "IV. Masters & Hinting"),
    ];

    #[must_use]
    pub const fn tab_label(self) -> &'static str {
        match self {
            Self::BasisTekenen => "I. Typografie & Kern (01–06)",
            Self::CurvenEnAudit => "II. CAGD & Bochten (07–12)",
            Self::KalligrafieEnStems => "III. Stems & Optica (13–18)",
            Self::TekstEnMasters => "IV. Masters & Hinting (19–24)",
            Self::IconStudioEnAnimatie => "V. Vector & Glas Studio (25–30)",
            Self::HostStekkersEnHardware => "VI. Kinetisch & Hardware (31–36)",
        }
    }

    #[must_use]
    pub const fn short_pill(self) -> &'static str {
        match self {
            Self::BasisTekenen => "I. Typo",
            Self::CurvenEnAudit => "II. CAGD",
            Self::KalligrafieEnStems => "III. Stems",
            Self::TekstEnMasters => "IV. Masters",
            Self::IconStudioEnAnimatie => "V. Icon",
            Self::HostStekkersEnHardware => "VI. Kinetic",
        }
    }

    #[must_use]
    pub const fn presets(self) -> &'static [DemoPreset; 6] {
        match self {
            Self::BasisTekenen => &CH_1_PRESETS,
            Self::CurvenEnAudit => &CH_2_PRESETS,
            Self::KalligrafieEnStems => &CH_3_PRESETS,
            Self::TekstEnMasters => &CH_4_PRESETS,
            Self::IconStudioEnAnimatie => &CH_5_PRESETS,
            Self::HostStekkersEnHardware => &CH_6_PRESETS,
        }
    }

    #[must_use]
    pub const fn summary(self) -> &'static str {
        match self {
            Self::BasisTekenen => {
                "Editoriale contouren, InfoBox, 20dp linialen, tekststrook 'AOVA', CJK-kader, ankers en Smart Components."
            }
            Self::CurvenEnAudit => {
                "G² harmonisatie, Genius (★), G³ Clothoïde (❖), Tunni-lijnen, krommingskam, fout-audit en Hobby-splines."
            }
            Self::KalligrafieEnStems => {
                "Smart Corners, inktvallen, 6-DoF kalligrafie-penseel, stamdikte-meter, meetlat, troffel en zwaartepunt (⊕)."
            }
            Self::TekstEnMasters => {
                "Multi-Master compatibiliteit, gebogen master-trajecten, HOI-diamanten, 16px TrueType/PostScript hinting en auto-trace."
            }
            Self::IconStudioEnAnimatie => {
                "800×800 Keylines, Play-optica (⊕), Shape Builder, variabele lijndikte, 6× symmetrie, COLRv1 en Vitrea Liquid Glass."
            }
            Self::HostStekkersEnHardware => {
                "Wi-Fi 4-mode kleur, SF Symbols 7 hartlijn DrawOn, Knockout-Slash halo, MotionPath satelliet, SVG roundtrip en Pie Menu."
            }
        }
    }
}

/// Duidelijke Nederlandse uitleg-kaart voor elke van de 36 showcase-cases.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ShowcaseCoachCard {
    pub chapter: ShowcaseChapter,
    pub short_button: &'static str,
    pub title: &'static str,
    pub what_is_it: &'static str,
    pub why_host_plug: &'static str,
    pub try_it_yourself: &'static str,
}

#[must_use]
pub fn coach_card(preset: DemoPreset) -> ShowcaseCoachCard {
    COACH_CARDS
        .iter()
        .find(|(p, _)| *p == preset)
        .map_or(COACH_CARDS[0].1, |(_, c)| *c)
}

#[must_use]
pub fn tool_dutch_guide(tool: CanvasTool) -> (&'static str, &'static str) {
    TOOL_DUTCH_GUIDES
        .iter()
        .find(|(t, _, _)| *t == tool)
        .map_or(
            (TOOL_DUTCH_GUIDES[0].1, TOOL_DUTCH_GUIDES[0].2),
            |(_, n, h)| (*n, *h),
        )
}

impl MockFontHost {
    pub fn step_tour_prev(&mut self, state: &mut GrafiaCanvasState, theme: &mut GrafiaTheme) {
        let len = DemoPreset::ALL.len();
        self.tour_index = if self.tour_index == 0 {
            len - 1
        } else {
            self.tour_index - 1
        };
        let (preset, _) = DemoPreset::ALL[self.tour_index];
        self.apply_preset(preset, state, theme);
    }

    pub fn step_tour_next(&mut self, state: &mut GrafiaCanvasState, theme: &mut GrafiaTheme) {
        let len = DemoPreset::ALL.len();
        self.tour_index = (self.tour_index + 1) % len;
        let (preset, _) = DemoPreset::ALL[self.tour_index];
        self.apply_preset(preset, state, theme);
    }

    #[must_use]
    pub const fn active_letter_tab(&self) -> ShowcaseChapter {
        self.active_chapter
    }

    /// Renders the 6-Domain × 6-Case = 36-Case Keynote Navigator inside the Left Studio Sidebar (`SPEC-018 §1.2`).
    pub fn render_keynote_36_case_navigator(
        &mut self,
        ui: &mut Ui,
        state: &mut GrafiaCanvasState,
        theme: &mut GrafiaTheme,
    ) {
        let (active_preset, _) = DemoPreset::ALL[self.tour_index.min(DemoPreset::ALL.len() - 1)];
        let card = coach_card(active_preset);
        ui.horizontal(|ui| {
            if ui
                .button("◀")
                .on_hover_text("Vorige case in de 36-staps Keynote-tour")
                .clicked()
            {
                self.step_tour_prev(state, theme);
            }
            ui.strong(format!("Keynote {}/36", self.tour_index + 1));
            if ui
                .button("▶")
                .on_hover_text("Volgende case in de 36-staps Keynote-tour")
                .clicked()
            {
                self.step_tour_next(state, theme);
            }
            if ui
                .button("↻")
                .on_hover_text("Herlaad actieve case")
                .clicked()
            {
                self.apply_preset(active_preset, state, theme);
            }
        });
        ui.horizontal_wrapped(|ui| {
            for ch in ShowcaseChapter::ALL {
                let is_on = self.active_chapter == ch;
                if ui
                    .selectable_label(is_on, ch.short_pill())
                    .on_hover_text(ch.summary())
                    .clicked()
                {
                    self.active_chapter = ch;
                }
            }
        });
        ui.label(RichText::new(self.active_chapter.tab_label()).strong());
        for &preset in self.active_chapter.presets() {
            let c = coach_card(preset);
            let tip = format!(
                "{}\n\n• Wat zie je: {}\n• Architectuur: {}\n• Probeer zelf: {}",
                c.title, c.what_is_it, c.why_host_plug, c.try_it_yourself
            );
            if ui
                .selectable_label(active_preset == preset, c.short_button)
                .on_hover_text(tip)
                .clicked()
            {
                self.apply_preset(preset, state, theme);
            }
        }
        ui.separator();
        ui.label(RichText::new(card.title).strong());
        ui.small(card.what_is_it);
        ui.small(format!("→ {}", card.try_it_yourself));
    }

    pub fn render_chapter_and_preset_buttons(
        &mut self,
        ui: &mut Ui,
        state: &mut GrafiaCanvasState,
        theme: &mut GrafiaTheme,
    ) {
        self.render_keynote_36_case_navigator(ui, state, theme);
    }

    pub fn render_coach_banner(
        &mut self,
        ui: &mut Ui,
        state: &mut GrafiaCanvasState,
        theme: &mut GrafiaTheme,
    ) {
        let (active_preset, _) = DemoPreset::ALL[self.tour_index.min(DemoPreset::ALL.len() - 1)];
        let card = coach_card(active_preset);
        let (tool_name, _) = tool_dutch_guide(state.active_tool);
        ScrollArea::horizontal()
            .id_salt("coach_banner_top_row_scroll")
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    if ui.button("◀").clicked() {
                        self.step_tour_prev(state, theme);
                    }
                    ui.strong(format!(
                        "Case {:02}/36 — {}",
                        self.tour_index + 1,
                        card.title
                    ));
                    if ui.button("▶").clicked() {
                        self.step_tour_next(state, theme);
                    }
                    ui.separator();
                    ui.label(card.what_is_it);
                    ui.separator();
                    ui.label(RichText::new(format!("Tool: {tool_name}")).strong());
                    if ui.button("✕").clicked() {
                        self.show_coach_banner = false;
                    }
                });
            });
    }
}
