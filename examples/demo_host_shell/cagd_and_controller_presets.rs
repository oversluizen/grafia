//! `SPEC-011`..`SPEC-013` & `SPEC-015` Interactive Demo Presets and
//! Loupedeck CT / Wacom Intuos 4 / `egui-macview` PieMenu Controller Simulator Panel.

use super::MockFontHost;
use egui::{ProgressBar, RichText, Separator, Ui};
use grafia::adapters::GrafiaCanvasState;
use grafia::core::{
    ActiveMirrorAxis, CalligraphicNibSpec, CalligraphicSample, CanvasIntent, CanvasRotaryChannel,
    CompanionMasterLayer, CurvatureCombStyle, CurvedTrajectoryLink, FontPoint, InputDeviceKind,
    NodeId, SmartCornerSpec, StylusAccessoryEvent,
};
use grafia::tokens::GlyphArchetype;

const DEMO_SMART_CORNER_FILLET_UPM: f64 = 28.0;
const DEMO_SMART_CORNER_TRAP_UPM: f64 = -18.0;
const DEMO_NIB_MIN_WIDTH_UPM: f64 = 12.0;
const DEMO_NIB_MAX_WIDTH_UPM: f64 = 44.0;
const DEMO_NIB_ANGLE_DEG: f64 = 30.0;
const DEMO_NIB_TILT_INFLUENCE: f64 = 0.5;
const DEMO_BRUSH_P0_X: f64 = 120.0;
const DEMO_BRUSH_P0_Y: f64 = 520.0;
const DEMO_BRUSH_P1_X: f64 = 260.0;
const DEMO_BRUSH_P1_Y: f64 = 640.0;
const DEMO_BRUSH_P2_X: f64 = 420.0;
const DEMO_BRUSH_P2_Y: f64 = 540.0;
const DEMO_PRESSURE_LOW: f64 = 0.25;
const DEMO_PRESSURE_HIGH: f64 = 0.85;
const DEMO_TILT_X: f64 = 15.0;
const DEMO_TILT_Y: f64 = 10.0;
const DEMO_REF_STEM_V_UPM: f64 = 84.0;
const DEMO_REF_STEM_H_UPM: f64 = 68.0;
const DEMO_REF_STEM_TOL_UPM: f64 = 6.0;
const DEMO_OVERSHOOT_UPM: f64 = 14.0;
const DEMO_TRAJ_CTRL_X_UPM: f64 = 340.0;
const DEMO_TRAJ_CTRL_Y_UPM: f64 = 760.0;
const DEMO_COMP_SHIFT_X_UPM: f64 = 36.0;
const DEMO_COMP_SHIFT_Y_UPM: f64 = 24.0;
const DEMO_SQUEEZE_X_DP: f32 = 320.0;
const DEMO_SQUEEZE_Y_DP: f32 = 240.0;

impl MockFontHost {
    pub fn apply_master_compat_preset(&mut self, state: &mut GrafiaCanvasState) {
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

    pub fn apply_curve_harmony_and_audit_preset(&mut self, state: &mut GrafiaCanvasState) {
        if state.active_archetype != GlyphArchetype::O {
            state.load_glyph_archetype(GlyphArchetype::O);
            state.glyph.name = "O".into();
        }
        state.select_all_nodes();
        state.harmonize_selected_nodes();
        if let Some(&first_id) = state.selected_nodes.first() {
            state.set_node_genius(first_id, true);
        }
        if let Some(&second_id) = state.selected_nodes.get(3) {
            state.set_node_g3_lock(second_id, true);
        }
        state.show_tunni_lines = true;
        state.curve_harmony.show_curvature_comb = true;
        state.curve_harmony.show_outline_audit = true;
        state.curve_harmony.servo_scaling_enabled = true;
        state.bump_content_generation();
        self.push_log(
            "Preset v0.10: G2/G3 Harmonized + Genius(★)/G3(❖) Badges + Tunni + Comb + Audit active",
        );
    }

    pub fn apply_smart_corners_brush_and_gauge_preset(&mut self, state: &mut GrafiaCanvasState) {
        if state.active_archetype != GlyphArchetype::A {
            state.load_glyph_archetype(GlyphArchetype::A);
            state.glyph.name = "A".into();
        }
        state.set_smart_corner(SmartCornerSpec {
            node_id: NodeId(1),
            radius_upm: DEMO_SMART_CORNER_FILLET_UPM,
        });
        state.set_smart_corner(SmartCornerSpec {
            node_id: NodeId(2),
            radius_upm: DEMO_SMART_CORNER_TRAP_UPM,
        });
        let nib = CalligraphicNibSpec {
            min_width_upm: DEMO_NIB_MIN_WIDTH_UPM,
            max_width_upm: DEMO_NIB_MAX_WIDTH_UPM,
            base_angle_deg: DEMO_NIB_ANGLE_DEG,
            tilt_influence: DEMO_NIB_TILT_INFLUENCE,
        };
        let samples = [
            CalligraphicSample {
                pos: FontPoint::new(DEMO_BRUSH_P0_X, DEMO_BRUSH_P0_Y),
                pressure: DEMO_PRESSURE_LOW,
                tilt_x: DEMO_TILT_X,
                tilt_y: DEMO_TILT_Y,
            },
            CalligraphicSample {
                pos: FontPoint::new(DEMO_BRUSH_P1_X, DEMO_BRUSH_P1_Y),
                pressure: DEMO_PRESSURE_HIGH,
                tilt_x: DEMO_TILT_X,
                tilt_y: DEMO_TILT_Y,
            },
            CalligraphicSample {
                pos: FontPoint::new(DEMO_BRUSH_P2_X, DEMO_BRUSH_P2_Y),
                pressure: DEMO_PRESSURE_LOW,
                tilt_x: DEMO_TILT_X,
                tilt_y: DEMO_TILT_Y,
            },
        ];
        let _ = state.apply_calligraphic_brush_stroke(nib, &samples);
        state.set_reference_stems(
            vec![DEMO_REF_STEM_V_UPM, DEMO_REF_STEM_H_UPM],
            DEMO_REF_STEM_TOL_UPM,
        );
        state.set_show_stem_gauge(true);
        state.set_show_visual_centroid(true);
        state.curve_harmony.show_curvature_comb = true;
        state.set_curvature_comb_style(CurvatureCombStyle::HeatmapEnvelope);
        self.push_log(
            "Preset v0.11: Smart Corner + Ink Trap + Calligraphic Stroke + Heatmap + Stem Gauge + Centroid ⊕",
        );
    }

    pub fn apply_stem_rainbow_trajectory_and_mirror_preset(
        &mut self,
        state: &mut GrafiaCanvasState,
    ) {
        if state.active_archetype != GlyphArchetype::O {
            state.load_glyph_archetype(GlyphArchetype::O);
            state.glyph.name = "O".into();
        }
        state.set_show_measurement_rainbow(true);
        state.set_round_overshoot_upm(Some(DEMO_OVERSHOOT_UPM));
        state.set_active_mirror_axis(ActiveMirrorAxis::Vertical {
            x_upm: state.glyph.width * 0.5,
        });
        let mut shifted = state.contours.clone();
        for c in &mut shifted {
            for n in c {
                n.point.x.0 += DEMO_COMP_SHIFT_X_UPM;
                n.point.y.0 += DEMO_COMP_SHIFT_Y_UPM;
            }
        }
        let closed_flags: Vec<bool> = state.glyph.contours.iter().map(|c| c.closed).collect();
        state.set_companion_master_layers(vec![CompanionMasterLayer {
            id: 1,
            name: "BoldMaster".into(),
            color_rgba: [65, 195, 125, 200],
            contours: shifted,
            closed_flags,
            anchors: Vec::new(),
            editable: true,
        }]);
        state.show_master_compatibility = true;
        if let Some(first_node) = state.contours.first().and_then(|c| c.first()) {
            state.set_curved_trajectory_link(CurvedTrajectoryLink {
                node_id: first_node.id,
                companion_layer_index: 0,
                control1: FontPoint::new(DEMO_TRAJ_CTRL_X_UPM, DEMO_TRAJ_CTRL_Y_UPM),
                control2: None,
            });
        }
        self.push_log(
            "Preset v0.12: Measurement Rainbow + Overshoot Ribbons + Curved Trajectory + 1-Axis Mirror",
        );
    }

    pub fn apply_controller_rotary_and_haptics_preset(&mut self, state: &mut GrafiaCanvasState) {
        let latched = !state.is_intent_latched(CanvasIntent::ConstrainAngle);
        state.set_intent_latched(CanvasIntent::ConstrainAngle, latched);
        state.step_rotary_encoder(CanvasRotaryChannel::ContextPrimary, 1.0);
        self.poll_host_ports(state);
        let tel = state.controller_telemetry();
        self.push_log(format!(
            "Preset SPEC-015: Dial={} ({}) | Intents={}",
            tel.primary_rotary.parameter_name,
            tel.primary_rotary.formatted_value,
            tel.available_intents.len()
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
            let channels = [
                ("Primary", CanvasRotaryChannel::ContextPrimary),
                ("Zoom", CanvasRotaryChannel::ViewportZoom),
                ("Angle", CanvasRotaryChannel::CanvasRotationOrSlant),
                ("Undo/Redo", CanvasRotaryChannel::HistoryScrub),
                ("Tension/R", CanvasRotaryChannel::CurveTensionOrRadius),
                ("Kern/Spc", CanvasRotaryChannel::KerningOrSpacing),
            ];
            for (name, ch) in channels {
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
