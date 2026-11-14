//! Interactive Multi-Layer Color Gradients, Vertical Stream, `CustomReferenceFrame`, Hinting, and `HOI` Presets.

use super::MockFontHost;
use grafia::adapters::{CanvasTool, GrafiaCanvasState};
use grafia::core::{
    CanvasColorSublayer, ColorLayerPaint, CompanionMasterLayer, CustomReferenceFrame, FontPoint,
    GradientSpread, GradientStop, Node, NodeId, PostScriptHint, PostScriptHintAxis, RectUpm,
    TrueTypeInstructionKind, TrueTypeRoundingGrid, TrueTypeVisualInstruction, WritingDirection,
};

const COLR_SHADOW_DX_UPM: f64 = 18.0;
const COLR_SHADOW_DY_UPM: f64 = -14.0;
const COLR_GRAD_P0_X_UPM: f64 = 80.0;
const COLR_GRAD_P0_Y_UPM: f64 = 50.0;
const COLR_GRAD_P1_X_UPM: f64 = 520.0;
const COLR_GRAD_P1_Y_UPM: f64 = 680.0;
const COLR_RADIAL_CX_UPM: f64 = 300.0;
const COLR_RADIAL_CY_UPM: f64 = 360.0;
const COLR_RADIAL_R0_UPM: f64 = 30.0;
const COLR_RADIAL_R1_UPM: f64 = 320.0;
const COLR_SHADOW_OPACITY: f32 = 0.9;
const COLR_PRIMARY_OPACITY: f32 = 0.85;
const REF_OUTER_MAX_X_UPM: f64 = 600.0;
const REF_OUTER_MAX_Y_UPM: f64 = 750.0;
const REF_INNER_MIN_X_UPM: f64 = 60.0;
const REF_INNER_MIN_Y_UPM: f64 = 50.0;
const REF_INNER_MAX_X_UPM: f64 = 540.0;
const REF_INNER_MAX_Y_UPM: f64 = 700.0;
const REF_KEYLINE_R1_UPM: f64 = 180.0;
const REF_KEYLINE_R2_UPM: f64 = 260.0;
const REF_GRID_SUBDIV: u8 = 3;
const PS_STEM_Y_UPM: f64 = 0.0;
const PS_STEM_WIDTH_UPM: f64 = 68.0;
const PS_GHOST_TOP_Y_UPM: f64 = 700.0;
const DEMO_TT_PPEM: u16 = 16;
const HOI_SHIFT_M0_DX_UPM: f64 = -28.0;
const HOI_SHIFT_M0_DY_UPM: f64 = -16.0;
const HOI_SHIFT_M1_DX_UPM: f64 = 22.0;
const HOI_SHIFT_M1_DY_UPM: f64 = 18.0;

impl MockFontHost {
    pub fn apply_color_layers_colrv1_preset(&mut self, state: &mut GrafiaCanvasState) {
        if !state.color_sublayers.is_empty() {
            state.set_color_sublayers(Vec::new());
            state.select_color_sublayer(None);
            self.push_log("Preset: Cleared Color Sublayers");
            return;
        }
        let mut shadow_contours = state.glyph.contours.clone();
        for c in &mut shadow_contours {
            for n in &mut c.nodes {
                n.point.x.0 += COLR_SHADOW_DX_UPM;
                n.point.y.0 += COLR_SHADOW_DY_UPM;
            }
        }
        let layers = vec![
            CanvasColorSublayer {
                id: 1,
                name: "ShadowRadial".into(),
                contours: shadow_contours,
                paint: ColorLayerPaint::RadialGradient {
                    c0: FontPoint::new(COLR_RADIAL_CX_UPM, COLR_RADIAL_CY_UPM),
                    r0: COLR_RADIAL_R0_UPM,
                    c1: FontPoint::new(COLR_RADIAL_CX_UPM, COLR_RADIAL_CY_UPM),
                    r1: COLR_RADIAL_R1_UPM,
                    stops: vec![
                        GradientStop::new(0.0, [255, 165, 45, 220]),
                        GradientStop::new(1.0, [195, 40, 135, 190]),
                    ],
                    spread: GradientSpread::Pad,
                },
                opacity: COLR_SHADOW_OPACITY,
                visible: true,
                locked: false,
            },
            CanvasColorSublayer {
                id: 2,
                name: "PrimaryLinear".into(),
                contours: state.glyph.contours.clone(),
                paint: ColorLayerPaint::LinearGradient {
                    p0: FontPoint::new(COLR_GRAD_P0_X_UPM, COLR_GRAD_P0_Y_UPM),
                    p1: FontPoint::new(COLR_GRAD_P1_X_UPM, COLR_GRAD_P1_Y_UPM),
                    stops: vec![
                        GradientStop::new(0.0, [40, 135, 255, 235]),
                        GradientStop::new(0.5, [95, 225, 185, 235]),
                        GradientStop::new(1.0, [255, 90, 120, 235]),
                    ],
                    spread: GradientSpread::Pad,
                },
                opacity: COLR_PRIMARY_OPACITY,
                visible: true,
                locked: false,
            },
        ];
        state.set_color_sublayers(layers);
        state.select_color_sublayer(Some(1));
        state.select_gradient_stop(Some((1, 1)));
        self.push_log("Preset: Gradients active (Drag gradient endpoints/stops!)");
    }

    pub fn apply_vertical_text_and_ref_frame_preset(&mut self, state: &mut GrafiaCanvasState) {
        let next_dir = if state.text_stream.writing_direction == WritingDirection::TopToBottom {
            WritingDirection::LeftToRight
        } else {
            WritingDirection::TopToBottom
        };
        self.text_buffer = "AOVA".to_owned();
        let slots = self.build_slots_for_text(&self.text_buffer);
        state.activate_tool(CanvasTool::Text);
        state.set_text_stream_slots_with_caret(slots, 1, 2);
        state.set_writing_direction(next_dir);
        let frame = (next_dir == WritingDirection::TopToBottom).then(|| CustomReferenceFrame {
            outer_box: Some(RectUpm::new(
                0.0,
                0.0,
                REF_OUTER_MAX_X_UPM,
                REF_OUTER_MAX_Y_UPM,
            )),
            inner_box: Some(RectUpm::new(
                REF_INNER_MIN_X_UPM,
                REF_INNER_MIN_Y_UPM,
                REF_INNER_MAX_X_UPM,
                REF_INNER_MAX_Y_UPM,
            )),
            inner_grid_subdivisions: Some((REF_GRID_SUBDIV, REF_GRID_SUBDIV)),
            center_crosshair: true,
            keyline_circles_radii_upm: vec![REF_KEYLINE_R1_UPM, REF_KEYLINE_R2_UPM],
            parallel_lines: Vec::new(),
        });
        state.set_custom_reference_frame(frame);
        self.push_log(format!(
            "Preset: WritingDirection={next_dir:?} + CustomReferenceFrame={}",
            state.custom_reference_frame.is_some()
        ));
    }

    pub fn apply_hinting_pixel_preview_and_hoi_preset(&mut self, state: &mut GrafiaCanvasState) {
        state.set_postscript_hints(vec![
            PostScriptHint {
                id: 1,
                axis: PostScriptHintAxis::Horizontal,
                origin_upm: PS_STEM_Y_UPM,
                width_upm: PS_STEM_WIDTH_UPM,
                is_ghost: false,
                top_ghost: false,
                start_node: None,
                end_node: None,
                selected: true,
            },
            PostScriptHint {
                id: 2,
                axis: PostScriptHintAxis::Horizontal,
                origin_upm: PS_GHOST_TOP_Y_UPM,
                width_upm: 0.0,
                is_ghost: true,
                top_ghost: true,
                start_node: None,
                end_node: None,
                selected: false,
            },
        ]);
        Self::populate_demo_truetype_instructions_and_hoi(state);
        let next_tool = if state.active_tool == CanvasTool::TrueTypeInstructor {
            CanvasTool::Hoi
        } else {
            CanvasTool::TrueTypeInstructor
        };
        state.activate_tool(next_tool);
        self.push_log(format!(
            "Preset: PS/TT Hints + 16 PPEM Grid + HOI Diamonds (Tool={next_tool:?})"
        ));
    }

    fn populate_demo_truetype_instructions_and_hoi(state: &mut GrafiaCanvasState) {
        let all_ids: Vec<NodeId> = state
            .contours
            .iter()
            .flat_map(|c| c.iter().map(|n| n.id))
            .collect();
        if let Some(&n0) = all_ids.first() {
            let n1 = all_ids.get(1).copied().unwrap_or(n0);
            state.set_truetype_instructions(vec![
                TrueTypeVisualInstruction {
                    id: 1,
                    kind: TrueTypeInstructionKind::Anchor,
                    source_node: None,
                    target_node: n0,
                    ppem: DEMO_TT_PPEM,
                    round_mode: TrueTypeRoundingGrid::ToGrid,
                    stem_index: None,
                    zone_index: Some(0),
                    delta_pixels: 0.0,
                    x_axis: false,
                    selected: false,
                },
                TrueTypeVisualInstruction {
                    id: 2,
                    kind: TrueTypeInstructionKind::Stem,
                    source_node: Some(n0),
                    target_node: n1,
                    ppem: DEMO_TT_PPEM,
                    round_mode: TrueTypeRoundingGrid::ToGrid,
                    stem_index: Some(0),
                    zone_index: None,
                    delta_pixels: 0.0,
                    x_axis: false,
                    selected: true,
                },
            ]);
        }
        state.set_truetype_instructor_ppem(DEMO_TT_PPEM);
        state.set_pixel_preview_ppem(Some(DEMO_TT_PPEM));
        let shift = |dx: f64, dy: f64| -> Vec<Vec<Node>> {
            let mut copy = state.contours.clone();
            for c in &mut copy {
                for n in c {
                    n.point.x.0 += dx;
                    n.point.y.0 += dy;
                }
            }
            copy
        };
        let flags: Vec<bool> = state.glyph.contours.iter().map(|c| c.closed).collect();
        state.set_companion_master_layers(vec![
            CompanionMasterLayer {
                id: 1,
                name: "M0".into(),
                color_rgba: [65, 145, 235, 190],
                contours: shift(HOI_SHIFT_M0_DX_UPM, HOI_SHIFT_M0_DY_UPM),
                closed_flags: flags.clone(),
                anchors: Vec::new(),
                editable: true,
            },
            CompanionMasterLayer {
                id: 2,
                name: "M1".into(),
                color_rgba: [85, 205, 145, 190],
                contours: shift(HOI_SHIFT_M1_DX_UPM, HOI_SHIFT_M1_DY_UPM),
                closed_flags: flags,
                anchors: Vec::new(),
                editable: true,
            },
        ]);
        state.set_show_hoi_diamonds(true);
    }
}
