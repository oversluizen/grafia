//! Host-Port Event Polling and Interactive Stekker Presets for `MockFontHost`.

use super::MockFontHost;
use grafia::adapters::{CanvasTool, GrafiaCanvasState};
use grafia::core::{
    Affine2D, CanvasComponent, CompanionMasterLayer, ComponentId, ContourStrokeStyle, LineCapStyle,
    LineJoinStyle, StrokePosition,
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
    ToggleTheme,
}

impl DemoPreset {
    pub const ALL: [(Self, &'static str); 9] = [
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
                let next = if state.active_archetype == GlyphArchetype::A {
                    GlyphArchetype::O
                } else {
                    GlyphArchetype::A
                };
                state.load_glyph_archetype(next);
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
            DemoPreset::TextStreamAova => {
                self.text_buffer = "AOVA".to_owned();
                let slots = self.build_slots_for_text(&self.text_buffer);
                state.activate_tool(CanvasTool::Text);
                state.set_text_stream_slots_with_caret(slots, 1, 2);
                self.push_log("Preset: Loaded Text Stream \"AOVA\" with live kerning pairs");
            }
            DemoPreset::RulersAndBg => {
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
            DemoPreset::AnchorsAndMarkCloud => {
                state.reset_default_anchors();
                self.poll_host_ports(state);
            }
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
            DemoPreset::MasterCompat => {
                self.apply_master_compat_preset(state);
            }
            DemoPreset::ToggleTheme => {
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
        }
    }

    fn apply_master_compat_preset(&mut self, state: &mut GrafiaCanvasState) {
        let shift_contours = |dx: f64, dy: f64| -> Vec<Vec<grafia::core::Node>> {
            let mut copy = state.contours.clone();
            for c in &mut copy {
                for n in c {
                    n.point.x.0 += dx;
                    n.point.y.0 += dy;
                }
            }
            copy
        };
        let closed_flags: Vec<bool> = state.glyph.contours.iter().map(|c| c.closed).collect();
        let make_master = |id, name: &str, color_rgba, contours, editable| CompanionMasterLayer {
            id,
            name: name.into(),
            color_rgba,
            contours,
            closed_flags: if editable {
                closed_flags.clone()
            } else {
                Vec::new()
            },
            anchors: if editable {
                state.anchors.clone()
            } else {
                Vec::new()
            },
            editable,
        };
        state.set_companion_master_layers(vec![
            make_master(
                1,
                "Light",
                [55, 145, 225, 200],
                shift_contours(-18.0, -10.0),
                true,
            ),
            make_master(
                2,
                "Bold",
                [65, 195, 125, 200],
                shift_contours(24.0, 14.0),
                true,
            ),
            make_master(3, "BrokenMaster", [225, 55, 55, 215], Vec::new(), false),
        ]);
        state.show_master_compatibility = !state.show_master_compatibility;
        self.push_log(format!(
            "Preset: Master Compatibility = {}",
            state.show_master_compatibility
        ));
    }
}
