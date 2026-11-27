//! `SPEC-011`..`SPEC-015` & `SPEC-018 §2` CAGD, Stamdikte, Kalligrafie & Hardware Controller Presets.

use super::{DemoPreset, MockFontHost};
use egui::{ProgressBar, RichText, Separator, Ui};
use grafia::adapters::{CanvasTool, GrafiaCanvasState};
use grafia::core::{
    ActiveMirrorAxis, CalligraphicNibSpec, CalligraphicSample, CanvasIntent, CanvasRotaryChannel,
    CompanionMasterLayer, CurvatureCombStyle, CurvedTrajectoryLink, FontPoint, InputDeviceKind,
    NodeId, SmartCornerSpec, StylusAccessoryEvent,
};
use grafia::tokens::{GlyphArchetype, FIT_CURVE_MAX_COLUMN_INDEX};

const DEMO_SMART_CORNER_FILLET_UPM: f64 = 28.0;
const DEMO_SMART_CORNER_TRAP_UPM: f64 = -18.0;
const DEMO_NIB_MIN_WIDTH_UPM: f64 = 12.0;
const DEMO_NIB_MAX_WIDTH_UPM: f64 = 44.0;
const DEMO_NIB_ANGLE_DEG: f64 = 30.0;
const DEMO_NIB_TILT_INFLUENCE: f64 = 0.5;
const DEMO_REF_STEM_V_UPM: f64 = 84.0;
const DEMO_REF_STEM_H_UPM: f64 = 68.0;
const DEMO_REF_STEM_TOL_UPM: f64 = 6.0;
const DEMO_OVERSHOOT_UPM: f64 = 14.0;
const DEMO_TRAJ_CTRL_X_UPM: f64 = 340.0;
const DEMO_TRAJ_CTRL_Y_UPM: f64 = 760.0;
const DEMO_SQUEEZE_X_DP: f32 = 320.0;
const DEMO_SQUEEZE_Y_DP: f32 = 240.0;

impl MockFontHost {
    pub fn apply_cagd_domain_case(&mut self, preset: DemoPreset, state: &mut GrafiaCanvasState) {
        self.ensure_letter_glyph(state, GlyphArchetype::O);
        state.select_all_nodes();
        match preset {
            DemoPreset::G2HarmonizeAndGeniusNodes => {
                state.harmonize_selected_nodes();
                if let Some(&id0) = state.selected_nodes.first() {
                    state.set_node_genius(id0, true);
                }
                state.show_tunni_lines = true;
                self.push_log("Case 07: G² Krommingscontinuïteit & Genius-knoop (★) vergrendeld");
            }
            DemoPreset::G3ClothoidEulerSpiral => {
                state.harmonize_selected_nodes_g3();
                if let Some(&id3) = state.selected_nodes.get(3) {
                    state.set_node_g3_lock(id3, true);
                }
                state.curve_harmony.show_curvature_comb = true;
                self.push_log("Case 08: G³ Clothoïde / Euler-Spiraal (❖) nul-schok overgang");
            }
            DemoPreset::TunniLinesAndHandleRatios => {
                state.show_tunni_lines = true;
                state.apply_fit_curve_column(self.fit_curve_col);
                self.fit_curve_col = (self.fit_curve_col + 1) % (FIT_CURVE_MAX_COLUMN_INDEX + 1);
                self.push_log("Case 09: Blauwe Tunni-hefboomlijnen & 55% Handle-Ratio pill-badges");
            }
            DemoPreset::CurvatureCombSpikesAndHeatmap => {
                state.curve_harmony.show_curvature_comb = true;
                let next =
                    if state.curve_harmony.curvature_comb_style == CurvatureCombStyle::SpinesOnly {
                        CurvatureCombStyle::HeatmapEnvelope
                    } else {
                        CurvatureCombStyle::SpinesOnly
                    };
                state.set_curvature_comb_style(next);
                self.push_log(format!("Case 10: Krommingskam stijl = {next:?}"));
            }
            DemoPreset::RedArrowsOutlineAudit => {
                state.curve_harmony.show_outline_audit = true;
                if let Some(n) = state.contours.first_mut().and_then(|c| c.get_mut(1)) {
                    n.point.y.0 += 48.0;
                }
                state.sync_paths_from_contours();
                self.push_log("Case 11: Rode-Pijlen Contour-Audit detecteert extrema-overshoot");
            }
            _ => {
                let _ = state.convert_selected_cubic_nodes_to_hobby();
                state.power_nudge_enabled = true;
                state.curve_harmony.servo_scaling_enabled = true;
                self.push_log("Case 12: Hobby-Splines + Power Nudge & Proportionele Servo-Handles");
            }
        }
        state.bump_content_generation();
    }

    pub fn apply_stems_domain_case(&mut self, preset: DemoPreset, state: &mut GrafiaCanvasState) {
        match preset {
            DemoPreset::SmartCornersFilletsAndInkTraps => {
                self.ensure_letter_glyph(state, GlyphArchetype::A);
                for (id, r) in [
                    (NodeId(1), DEMO_SMART_CORNER_FILLET_UPM),
                    (NodeId(2), DEMO_SMART_CORNER_TRAP_UPM),
                ] {
                    state.set_smart_corner(SmartCornerSpec {
                        node_id: id,
                        radius_upm: r,
                    });
                }
                state.selected_nodes = vec![NodeId(1), NodeId(2)];
                self.push_log("Case 13: Niet-destructieve Smart Corner (+28) & Inktval (-18 UPM)");
            }
            DemoPreset::Calligraphic6DofNibBrush => self.apply_calligraphic_brush_case(state),
            DemoPreset::StructuralStemGaugeAndTool => {
                self.ensure_letter_glyph(state, GlyphArchetype::A);
                state.set_reference_stems(
                    vec![DEMO_REF_STEM_V_UPM, DEMO_REF_STEM_H_UPM],
                    DEMO_REF_STEM_TOL_UPM,
                );
                state.set_show_stem_gauge(true);
                state.activate_tool(CanvasTool::Stem);
                self.push_log(
                    "Case 15: Structurele Stamdikte-Meter (84/68 UPM) & Stem-Tool actief",
                );
            }
            DemoPreset::IntersectionRulerAndRainbow => self.apply_intersection_ruler_case(state),
            DemoPreset::TrowelSmoothingAndCleanerBrush => {
                self.ensure_letter_glyph(state, GlyphArchetype::O);
                let next = if state.active_tool == CanvasTool::Trowel {
                    CanvasTool::Cleaner
                } else {
                    CanvasTool::Trowel
                };
                state.activate_tool(next);
                self.push_log(format!("Case 17: Borstel-Gereedschap = {next:?}"));
            }
            _ => {
                self.ensure_letter_glyph(state, GlyphArchetype::O);
                state.set_show_optical_rhythm_fill(true);
                state.set_round_overshoot_upm(Some(DEMO_OVERSHOOT_UPM));
                state.set_show_visual_centroid(true);
                state.set_active_mirror_axis(ActiveMirrorAxis::Vertical {
                    x_upm: state.glyph.width * 0.5,
                });
                self.push_log("Case 18: Optisch Ritme + Overshoot-Band (14 UPM) + Zwaartepunt (⊕)");
            }
        }
    }

    fn apply_calligraphic_brush_case(&mut self, state: &mut GrafiaCanvasState) {
        self.ensure_letter_glyph(state, GlyphArchetype::A);
        let nib = CalligraphicNibSpec {
            min_width_upm: DEMO_NIB_MIN_WIDTH_UPM,
            max_width_upm: DEMO_NIB_MAX_WIDTH_UPM,
            base_angle_deg: DEMO_NIB_ANGLE_DEG,
            tilt_influence: DEMO_NIB_TILT_INFLUENCE,
        };
        let mk_s = |x, y, p| CalligraphicSample {
            pos: FontPoint::new(x, y),
            pressure: p,
            tilt_x: 15.0,
            tilt_y: 10.0,
        };
        let _ = state.apply_calligraphic_brush_stroke(
            nib,
            &[
                mk_s(110.0, 540.0, 0.25),
                mk_s(280.0, 665.0, 0.88),
                mk_s(470.0, 545.0, 0.25),
            ],
        );
        self.push_log("Case 14: 6-DoF Druk & Stylus-Tilt Kalligrafie-Penseelstreek");
    }

    fn apply_intersection_ruler_case(&mut self, state: &mut GrafiaCanvasState) {
        self.ensure_letter_glyph(state, GlyphArchetype::O);
        state.set_show_measurement_rainbow(true);
        state.activate_tool(CanvasTool::Measurement);
        let mods = grafia::Modifiers::default();
        let _ = state.on_drawing_tool_pointer_press(
            grafia::ScreenPoint::new(180.0, 360.0),
            FontPoint::new(40.0, 350.0),
            mods,
        );
        let _ = state.on_drawing_tool_pointer_move(
            grafia::ScreenPoint::new(620.0, 360.0),
            FontPoint::new(660.0, 350.0),
            mods,
        );
        self.push_log("Case 16: Snijpunten-Meetlat (L) + Continue Dikte-Regenboog op /O");
    }

    pub fn apply_master_compat_preset(&mut self, state: &mut GrafiaCanvasState) {
        self.ensure_letter_glyph(state, GlyphArchetype::O);
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
        state.set_companion_master_layers(vec![
            CompanionMasterLayer {
                id: 1,
                name: "Light".into(),
                color_rgba: [52, 116, 178, 200],
                contours: shift_contours(-18.0, -10.0),
                closed_flags: closed_flags.clone(),
                anchors: state.anchors.clone(),
                editable: true,
            },
            CompanionMasterLayer {
                id: 2,
                name: "Bold".into(),
                color_rgba: [64, 152, 138, 200],
                contours: shift_contours(24.0, 14.0),
                closed_flags,
                anchors: state.anchors.clone(),
                editable: true,
            },
        ]);
        state.show_master_compatibility = true;
        let _ = state.auto_match_master_compatibility(1);
        self.push_log("Case 19: Multi-Master Compatibiliteitsstralen (Light ↔ Bold) & Auto-Match");
    }

    pub fn apply_curved_master_trajectories_case(&mut self, state: &mut GrafiaCanvasState) {
        self.apply_master_compat_preset(state);
        state.activate_tool(CanvasTool::CurvedTrajectory);
        if let Some(first_node) = state.contours.first().and_then(|c| c.first()) {
            state.set_curved_trajectory_link(CurvedTrajectoryLink {
                node_id: first_node.id,
                companion_layer_index: 0,
                control1: FontPoint::new(DEMO_TRAJ_CTRL_X_UPM, DEMO_TRAJ_CTRL_Y_UPM),
                control2: None,
            });
        }
        self.push_log("Case 20: Gebogen Master-Traject voor knik-vrije roterende interpolatie");
    }

    pub fn apply_controller_rotary_and_haptics_preset(&mut self, state: &mut GrafiaCanvasState) {
        self.set_mobile_thumb_hud_enabled(true, state);
        let latched = !state.is_intent_latched(CanvasIntent::ConstrainAngle);
        state.set_intent_latched(CanvasIntent::ConstrainAngle, latched);
        state.step_rotary_encoder(CanvasRotaryChannel::ContextPrimary, 1.0);
        self.poll_host_ports(state);
        let tel = state.controller_telemetry();
        self.push_log(format!(
            "Case 36: Liquid Glass Pie Menu + Vliegwiel | Dial={} ({})",
            tel.primary_rotary.parameter_name, tel.primary_rotary.formatted_value
        ));
    }

    pub fn render_controller_simulator_section(ui: &mut Ui, state: &mut GrafiaCanvasState) {
        ui.add(Separator::default().horizontal());
        ui.strong("SPEC-015 Controller / Rotary / Haptic Bridge");
        let tel = state.controller_telemetry();
        ui.horizontal(|ui| {
            ui.label(
                RichText::new(format!(
                    "OLED Dial: {} = {}",
                    tel.primary_rotary.parameter_name, tel.primary_rotary.formatted_value
                ))
                .strong(),
            );
            ui.add(
                ProgressBar::new(tel.primary_rotary.normalized_ratio as f32).desired_width(72.0),
            );
        });
        ui.horizontal_wrapped(|ui| {
            for item in &tel.available_intents {
                let lbl = format!("{} [{}]", item.short_label, item.shortcut_hint);
                if ui.selectable_label(item.active, lbl).clicked() {
                    state.set_intent_latched(item.intent, !item.active);
                }
            }
        });
        ui.horizontal_wrapped(|ui| {
            for (name, ch) in [
                ("Primary", CanvasRotaryChannel::ContextPrimary),
                ("Zoom", CanvasRotaryChannel::ViewportZoom),
                ("Angle", CanvasRotaryChannel::CanvasRotationOrSlant),
                ("Undo/Redo", CanvasRotaryChannel::HistoryScrub),
                ("Tension/R", CanvasRotaryChannel::CurveTensionOrRadius),
                ("Kern/Spc", CanvasRotaryChannel::KerningOrSpacing),
            ] {
                if ui.button(format!("{name} -")).clicked() {
                    state.step_rotary_encoder(ch, -1.0);
                }
                if ui.button(format!("{name} +")).clicked() {
                    state.step_rotary_encoder(ch, 1.0);
                }
            }
        });
        ui.horizontal_wrapped(|ui| {
            let cur_dev = state.active_input_device();
            for (lbl, dev) in [
                ("Mouse", InputDeviceKind::Mouse),
                ("Stylus", InputDeviceKind::Stylus),
                ("Touch (1.6x)", InputDeviceKind::TouchFinger),
            ] {
                if ui.selectable_label(cur_dev == dev, lbl).clicked() {
                    state.set_active_input_device(dev);
                }
            }
            if ui.button("Pencil DoubleTap").clicked() {
                state.dispatch_stylus_accessory_event(StylusAccessoryEvent::DoubleTap);
            }
            if ui.button("Pencil Squeeze").clicked() {
                state.dispatch_stylus_accessory_event(StylusAccessoryEvent::Squeeze {
                    pos: grafia::ScreenPoint::new(DEMO_SQUEEZE_X_DP, DEMO_SQUEEZE_Y_DP),
                });
            }
        });
    }
}
