//! Host-Port Event Polling and Interactive Stekker Presets for `MockFontHost`.

use super::MockFontHost;
use grafia::adapters::{CanvasTool, GrafiaCanvasState};
use grafia::core::{
    Affine2D, CanvasComponent, ComponentId, ContourStrokeStyle, LineCapStyle, LineJoinStyle,
    StrokePosition,
};
use grafia::tokens::{AppearanceMode, GlyphArchetype, GrafiaTheme, FIT_CURVE_MAX_COLUMN_INDEX};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DemoPreset {
    ToggleGlyphAO,
    TextStreamAova,
    RulersAndBg,
    AnchorsAndMarkCloud,
    ComponentAndDecompose,
    FitCurveStep,
    LiveStrokeAndExpand,
    MasterCompat,
    HobbyAndSmartHandles,
    AutoTraceBgImage,
    ColorLayersColrV1,
    VerticalTextAndRefFrame,
    HintingPixelPreviewAndHoi,
    CurveHarmonyAndAudit,
    SmartCornersBrushAndGauge,
    StemRainbowTrajectoryAndMirror,
    ControllerRotaryAndHaptics,
    ToggleTheme,
}

impl DemoPreset {
    pub const ALL: [(Self, &'static str); 18] = [
        (Self::ToggleGlyphAO, "[Glyph: A / O]"),
        (Self::TextStreamAova, "[v0.5 Text Stream \"AOVA\" (T)]"),
        (Self::RulersAndBg, "[v0.6 Rulers (Cmd+R) + Bg Layer]"),
        (
            Self::AnchorsAndMarkCloud,
            "[v0.7 Anchors + Mark Cloud (Cmd+U)]",
        ),
        (
            Self::ComponentAndDecompose,
            "[v0.7 Component + Decompose (Cmd+Shift+D)]",
        ),
        (Self::FitCurveStep, "[v0.7 Fit Curve (57%..92%)]"),
        (Self::LiveStrokeAndExpand, "[v0.7 Live Stroke + Expand]"),
        (Self::MasterCompat, "[v0.7 Master Compat (Ctrl+Opt+Cmd+N)]"),
        (
            Self::HobbyAndSmartHandles,
            "[v0.8 Hobby Splines + SmartHandle]",
        ),
        (Self::AutoTraceBgImage, "[v0.8 Bg Image + Auto-Trace]"),
        (Self::ColorLayersColrV1, "[v0.9 COLRv1 Gradients]"),
        (
            Self::VerticalTextAndRefFrame,
            "[v0.9 Vertical Text + Ref Frame]",
        ),
        (
            Self::HintingPixelPreviewAndHoi,
            "[v0.9 Hinting + 16 PPEM + HOI (I/H)]",
        ),
        (Self::CurveHarmonyAndAudit, "[v0.10 G2/G3 + Tunni + Audit]"),
        (
            Self::SmartCornersBrushAndGauge,
            "[v0.11 Smart Corners + Brush + Stem Gauge]",
        ),
        (
            Self::StemRainbowTrajectoryAndMirror,
            "[v0.12 Stem + Rainbow + Trajectory + Mirror]",
        ),
        (
            Self::ControllerRotaryAndHaptics,
            "[SPEC-015 Loupedeck / Wacom / PieMenu Bridge]",
        ),
        (Self::ToggleTheme, "[Light / Dark Mode]"),
    ];
}

impl MockFontHost {
    pub fn apply_preset(
        &mut self,
        preset: DemoPreset,
        state: &mut GrafiaCanvasState,
        theme: &mut GrafiaTheme,
    ) {
        match preset {
            DemoPreset::ToggleGlyphAO => {
                self.apply_toggle_glyph_preset(state);
            }
            DemoPreset::TextStreamAova => {
                self.text_buffer = "AOVA".to_owned();
                let slots = self.build_slots_for_text(&self.text_buffer);
                state.activate_tool(CanvasTool::Text);
                state.set_text_stream_slots_with_caret(slots, 1, 2);
                self.push_log("Preset: Loaded Text Stream \"AOVA\" with live kerning pairs");
            }
            DemoPreset::RulersAndBg => {
                self.apply_rulers_and_bg_preset(state);
            }
            DemoPreset::AnchorsAndMarkCloud => {
                state.reset_default_anchors();
                self.poll_host_ports(state);
            }
            DemoPreset::ComponentAndDecompose
            | DemoPreset::FitCurveStep
            | DemoPreset::LiveStrokeAndExpand => {
                self.apply_component_or_stroke_preset(preset, state);
            }
            DemoPreset::MasterCompat => {
                self.apply_master_compat_preset(state);
            }
            DemoPreset::HobbyAndSmartHandles => {
                self.apply_hobby_and_smart_handles_preset(state);
            }
            DemoPreset::AutoTraceBgImage => {
                self.apply_auto_trace_bg_image_preset(state);
            }
            DemoPreset::ColorLayersColrV1 => {
                self.apply_color_layers_colrv1_preset(state);
            }
            DemoPreset::VerticalTextAndRefFrame => {
                self.apply_vertical_text_and_ref_frame_preset(state);
            }
            DemoPreset::HintingPixelPreviewAndHoi => {
                self.apply_hinting_pixel_preview_and_hoi_preset(state);
            }
            DemoPreset::CurveHarmonyAndAudit => {
                self.apply_curve_harmony_and_audit_preset(state);
            }
            DemoPreset::SmartCornersBrushAndGauge => {
                self.apply_smart_corners_brush_and_gauge_preset(state);
            }
            DemoPreset::StemRainbowTrajectoryAndMirror => {
                self.apply_stem_rainbow_trajectory_and_mirror_preset(state);
            }
            DemoPreset::ControllerRotaryAndHaptics => {
                self.apply_controller_rotary_and_haptics_preset(state);
            }
            DemoPreset::ToggleTheme => {
                self.apply_toggle_theme_preset(state, theme);
            }
        }
    }

    fn apply_toggle_glyph_preset(&mut self, state: &mut GrafiaCanvasState) {
        let next = if state.active_archetype == GlyphArchetype::A {
            GlyphArchetype::O
        } else {
            GlyphArchetype::A
        };
        state.load_glyph_archetype(next);
        super::toolbar_specs::fit_demo_canvas_viewport(state);
        state.glyph.name = if next == GlyphArchetype::A {
            "A".into()
        } else {
            "O".into()
        };
        self.push_log(format!(
            "Preset: Switched active glyph to /{}",
            state.glyph.name
        ));
    }

    fn apply_rulers_and_bg_preset(&mut self, state: &mut GrafiaCanvasState) {
        state.toggle_rulers();
        state.select_all_nodes();
        state.copy_selection_to_background();
        state.show_background = true;
        for gc in &mut state.background_contours {
            for n in &mut gc.nodes {
                n.point.x.0 += 18.0;
                n.point.y.0 += 12.0;
            }
        }
        state.sync_paths_from_contours();
        self.push_log(format!(
            "Preset: Rulers={} + Background Layer ({} contours)",
            state.show_rulers,
            state.background_contours.len()
        ));
    }

    fn apply_toggle_theme_preset(
        &mut self,
        state: &mut GrafiaCanvasState,
        theme: &mut GrafiaTheme,
    ) {
        let next = if state.appearance_mode == AppearanceMode::Light {
            AppearanceMode::Dark
        } else {
            AppearanceMode::Light
        };
        state.set_appearance_mode(next);
        *theme = if next == AppearanceMode::Dark {
            GrafiaTheme::sonoma_dark()
        } else {
            GrafiaTheme::sonoma_default()
        };
        self.push_log(format!("Preset: Switched appearance mode to {next:?}"));
    }

    fn apply_component_or_stroke_preset(
        &mut self,
        preset: DemoPreset,
        state: &mut GrafiaCanvasState,
    ) {
        match preset {
            DemoPreset::ComponentAndDecompose => {
                if state.components.is_empty() {
                    let comp_contours = self
                        .glyph_library
                        .get("acutecomb")
                        .map_or_else(Vec::new, |g| g.contours.clone());
                    state.set_components(vec![CanvasComponent {
                        id: ComponentId(1),
                        component_name: "acutecomb".into(),
                        contours: comp_contours,
                        transform: Affine2D::translation(350.0, 560.0),
                        locked: false,
                        automatic_alignment: false,
                        selected: true,
                    }]);
                    self.push_log(
                        "Preset: Injected selected component 'acutecomb' (Click again to Decompose)",
                    );
                } else {
                    for c in &mut state.components {
                        if !c.locked {
                            c.selected = true;
                        }
                    }
                    let _ = state.decompose_selected_components();
                    self.push_log(
                        "Preset: Decomposed 'acutecomb' into editable foreground contours",
                    );
                }
            }
            DemoPreset::FitCurveStep => {
                if state.active_archetype != GlyphArchetype::O {
                    state.load_glyph_archetype(GlyphArchetype::O);
                    super::toolbar_specs::fit_demo_canvas_viewport(state);
                    state.glyph.name = "O".into();
                }
                state.select_all_nodes();
                state.apply_fit_curve_column(self.fit_curve_col);
                self.push_log(format!(
                    "Preset: Fit Curve Column {} applied to cubic segments",
                    self.fit_curve_col + 1
                ));
                self.fit_curve_col = (self.fit_curve_col + 1) % (FIT_CURVE_MAX_COLUMN_INDEX + 1);
            }
            DemoPreset::LiveStrokeAndExpand => {
                if state.contour_strokes.is_empty() {
                    if let Some(c) = state.glyph.contours.first() {
                        state.set_contour_stroke(
                            c.id,
                            ContourStrokeStyle {
                                width: 28.0,
                                height: 28.0,
                                position: StrokePosition::Center,
                                cap_start: LineCapStyle::Round,
                                cap_end: LineCapStyle::Round,
                                join: LineJoinStyle::Round,
                            },
                        );
                    }
                    self.push_log(
                        "Preset: Live Stroke (28 UPM Round) enabled (Click again to Expand)",
                    );
                } else {
                    state.selected_nodes.clear();
                    state.expand_selected_strokes();
                    self.push_log("Preset: Expanded Live Stroke into closed foreground contours");
                }
            }
            _ => {}
        }
    }
}
