//! Transient sub-state containers, live-drag pixmap scaling, and host `CanvasDocumentScene` load/export operations (`SPEC-014`).

use crate::adapters::bbox_selection_tools::BBoxDragState;
use crate::adapters::color_layer_interaction::GradientDragState;
use crate::adapters::guide_management_ops::GuideDragState;
use crate::adapters::state::{CanvasTool, CanvasViewMode, GrafiaCanvasState};
use crate::core::{
    FontPoint, HoiCubicLayerSet, HoiSiblingContours, Node, NodeId, NodeType, ScreenPoint,
    ViewportTransform,
};
use crate::interaction::{CommandHistory, GlyphSnapshot, Modifiers, PenToolState, SelectToolState};

/// Internal pen-tool gesture and handle-freezing transient state (`SPEC-014 §2.2`).
#[derive(Debug, Clone, Default)]
pub struct PenInteractionTransients {
    pub drag_last_pt: Option<FontPoint>,
    pub handle_delta: (f64, f64),
    pub alt_frozen_bcp_in: Option<FontPoint>,
    pub seg_has_explicit_h1: bool,
    pub resuming_corner_line_end: bool,
    pub join_b_had_out_handles: bool,
    pub join_b_was_smooth: bool,
}

/// Persistent session, selection, keymap, touch, and host-request state (`SPEC-014 §2.2`).
#[derive(Debug, Clone)]
pub struct SessionAndHostState {
    pub next_guide_id: u64,
    pub next_anchor_id: u64,
    pub selected_companion_nodes: Vec<(u32, NodeId)>,
    pub selected_color_sublayer: Option<usize>,
    pub selected_gradient_stop: Option<(usize, usize)>,
    pub truetype_x_axis_projection: bool,
    pub hoi_layer_sets: Vec<HoiCubicLayerSet>,
    pub hoi_sibling_contours: Option<HoiSiblingContours>,
    pub touch_state: crate::tokens::TouchInteractionState,
    pub keymap: crate::tokens::GrafiaKeymap,
    pub fit_curve_config: crate::core::FitCurveConfig,
    pub preview_config: crate::render::PreviewRenderConfig,
    pub host_requests: Vec<crate::core::CanvasHostRequest>,
}

impl Default for SessionAndHostState {
    fn default() -> Self {
        Self {
            next_guide_id: 1,
            next_anchor_id: 1,
            selected_companion_nodes: Vec::new(),
            selected_color_sublayer: None,
            selected_gradient_stop: None,
            truetype_x_axis_projection: true,
            hoi_layer_sets: Vec::new(),
            hoi_sibling_contours: None,
            touch_state: crate::tokens::TouchInteractionState::default(),
            keymap: crate::tokens::GrafiaKeymap::default(),
            fit_curve_config: crate::core::FitCurveConfig::default(),
            preview_config: crate::render::PreviewRenderConfig::default(),
            host_requests: Vec::new(),
        }
    }
}

/// Internal drag-session, marquee, tool-override, and auxiliary transient state (`SPEC-014 §2.2`).
#[derive(Debug, Clone, Default)]
pub struct DragSessionTransients {
    pub prev_view_mode: Option<CanvasViewMode>,
    pub drag_start_font_positions: Vec<(NodeId, FontPoint)>,
    pub drag_start_node_types: Vec<(NodeId, FontPoint, NodeType)>,
    pub drag_start_contours: Vec<Vec<Node>>,
    pub last_click_time_pos: Option<(f64, ScreenPoint)>,
    pub last_double_click_time_pos: Option<(f64, ScreenPoint)>,
    pub marquee_pre_selection: Vec<NodeId>,
    pub marquee_modifiers: Modifiers,
    pub alt_marquee_pending_start: Option<ScreenPoint>,
    pub active_drag_before_snapshot: Option<GlyphSnapshot>,
    pub pending_segment_click: Option<(usize, crate::core::ContourSegment, Modifiers)>,
    pub alt_duplicate_pending: bool,
    pub temp_tool_before_override: Option<CanvasTool>,
    pub last_modifiers: Modifiers,
    pub temp_zoom_via_key_space: bool,
    pub ruler_drag_is_horizontal: Option<bool>,
    pub pre_all_layers_select_snapshot: Option<GlyphSnapshot>,
    pub saved_viewport: Option<ViewportTransform>,
    pub active_smart_handle_drag: Option<usize>,
    pub active_smart_corner_pin_drag: Option<NodeId>,
    pub active_gradient_drag: Option<GradientDragState>,
    pub active_anchor_drag: bool,
    pub guide_drag: GuideDragState,
    pub session_config: SessionAndHostState,
}

impl std::ops::Deref for DragSessionTransients {
    type Target = SessionAndHostState;
    fn deref(&self) -> &Self::Target {
        &self.session_config
    }
}

impl std::ops::DerefMut for DragSessionTransients {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.session_config
    }
}

/// Internal text-tool caret drag and measurement-line transient state (`SPEC-014 §2.2`).
#[derive(Debug, Clone)]
pub struct TextInteractionTransients {
    pub temp_show_text_measurement_line: bool,
    pub text_mode_numbers_threshold: f64,
    pub last_select_tool: CanvasTool,
    pub text_drag_anchor_caret: Option<(usize, ScreenPoint)>,
    pub suppress_text_stream_sync: bool,
    pub text_tool_activated_in_frame: bool,
}

impl Default for TextInteractionTransients {
    fn default() -> Self {
        Self {
            temp_show_text_measurement_line: false,
            text_mode_numbers_threshold: crate::tokens::TEXT_MODE_NUMBERS_THRESHOLD_DEFAULT,
            last_select_tool: CanvasTool::Select,
            text_drag_anchor_caret: None,
            suppress_text_stream_sync: false,
            text_tool_activated_in_frame: false,
        }
    }
}

impl GrafiaCanvasState {
    /// Returns `true` when any continuous interactive drag gesture is active on the canvas (`SPEC-014 §2.3`).
    #[must_use]
    pub fn is_interactive_drag_active(&self) -> bool {
        matches!(
            self.select_state,
            SelectToolState::DraggingNodes { .. }
                | SelectToolState::DraggingSegment { .. }
                | SelectToolState::DraggingTunniLine { .. }
                | SelectToolState::PanningViewport { .. }
        ) || matches!(self.pen_state, PenToolState::DraggingHandles { .. })
            || self.bbox_drag != BBoxDragState::Idle
            || self.drag_transients.guide_drag != GuideDragState::Idle
            || self.drag_transients.active_gradient_drag.is_some()
            || self.drag_transients.active_smart_handle_drag.is_some()
            || self.drag_transients.active_smart_corner_pin_drag.is_some()
            || self.drag_transients.active_anchor_drag
            || self.drawing_tool_state.knife.active
            || self.drawing_tool_state.primitive_drag.active
            || self.drawing_tool_state.pencil.active
            || self.drawing_tool_state.pixel.active
            || self.drawing_tool_state.brush_stroke_active
            || self.drawing_tool_state.shape_builder.active
            || self.text_measurement_dragging
    }

    /// Computes effective `(width_px, height_px, scale)` for COLRv1 color sublayer pixmap rasterization (`SPEC-014 §2.3`).
    #[must_use]
    pub fn effective_color_layer_pixmap_dimensions(
        &self,
        viewport_w: u32,
        viewport_h: u32,
    ) -> (u32, u32, f32) {
        let w = viewport_w.max(1);
        let h = viewport_h.max(1);
        let max_dim = crate::core::MAX_INTERACTIVE_DRAG_PIXMAP_DIM;
        if !self.is_interactive_drag_active() || w.max(h) <= max_dim {
            return (w, h, 1.0);
        }
        let scale = (max_dim as f32) / (w.max(h) as f32);
        let eff_w = ((w as f32) * scale).round().max(1.0) as u32;
        let eff_h = ((h as f32) * scale).round().max(1.0) as u32;
        (eff_w, eff_h, scale)
    }

    /// Atomically loads a complete [`crate::core::CanvasDocumentScene`] into this state (`SPEC-014 §2.4`).
    pub fn load_document_scene(&mut self, scene: crate::core::CanvasDocumentScene) {
        let orig_glyph = scene.glyph.clone();
        let italic_deg = scene.metrics.italic_angle_deg;
        self.load_glyph(scene.glyph, scene.metrics);
        self.glyph.upm = orig_glyph.upm;
        self.glyph.width = orig_glyph.width;
        self.glyph.height = orig_glyph.height;
        self.glyph.background_contours = orig_glyph.background_contours;
        self.glyph.anchors = orig_glyph.anchors;
        self.glyph.postscript_hints = orig_glyph.postscript_hints;
        self.glyph.truetype_instructions = orig_glyph.truetype_instructions;
        self.glyph.hoi_layer_sets = orig_glyph.hoi_layer_sets;
        self.glyph.hoi_sibling_contours = orig_glyph.hoi_sibling_contours;
        self.glyph.backdrop_ghost_contours = orig_glyph.backdrop_ghost_contours;
        if orig_glyph.upm == 0 {
            self.viewport.upm = 1000;
        }
        self.italic_angle_deg = if italic_deg.is_finite() {
            italic_deg
        } else {
            0.0
        };
        self.apply_loaded_scene_guides_and_layers(scene.guides, scene.layers);
        self.color_sublayers = scene
            .color_layers
            .into_iter()
            .map(sanitize_loaded_color_sublayer)
            .collect();
        self.drag_transients.selected_color_sublayer = None;
        self.drag_transients.selected_gradient_stop = None;
        self.apply_loaded_scene_hinting_and_harmony(
            scene.hinting,
            scene.reference_frame,
            scene.writing_direction,
            scene.curve_harmony,
        );
        self.icon_studio = scene.icon_studio.sanitized();
        self.history = CommandHistory::new();
        self.bump_content_generation();
        self.assert_invariants();
    }

    fn apply_loaded_scene_guides_and_layers(
        &mut self,
        guides: crate::core::CanvasGuideSet,
        layers: crate::core::CanvasLayerAttachments,
    ) {
        let mut seen_guides = std::collections::HashSet::new();
        let valid_guide = |g: &crate::snapping::Guide| {
            g.position.x.0.is_finite() && g.position.y.0.is_finite() && g.angle_deg.is_finite()
        };
        self.guides = guides
            .local_guides
            .into_iter()
            .filter(|g| valid_guide(g) && seen_guides.insert(g.id))
            .collect();
        self.global_guides = guides
            .global_guides
            .into_iter()
            .filter(|g| valid_guide(g) && seen_guides.insert(g.id))
            .collect();
        self.show_guides = guides.show_guides;
        self.lock_guides = guides.lock_guides;
        self.selected_guide = None;
        for g in self.guides.iter().chain(self.global_guides.iter()) {
            self.drag_transients.next_guide_id = self.drag_transients.next_guide_id.max(g.id.0 + 1);
        }
        self.background_contours = layers.background_contours;
        self.anchors = layers.anchors;
        for a in &self.anchors {
            self.drag_transients.next_anchor_id =
                self.drag_transients.next_anchor_id.max(a.id.0 + 1);
        }
        self.components = layers.components;
        self.contour_strokes = layers.contour_strokes;
        self.companion_master_layers = layers.companion_master_layers;
        self.smart_handles = layers.smart_handles;
        self.background_image = layers.background_image;
        self.backdrop_ghost_contours = layers.backdrop_ghost_contours;
    }

    fn apply_loaded_scene_hinting_and_harmony(
        &mut self,
        hinting: crate::core::CanvasHintingAndHoiScene,
        reference_frame: Option<crate::core::CustomReferenceFrame>,
        writing_direction: crate::core::WritingDirection,
        curve_harmony: crate::core::CanvasCurveHarmonyScene,
    ) {
        self.postscript_hints = hinting
            .postscript_hints
            .into_iter()
            .filter(|h| h.origin_upm.is_finite() && h.width_upm.is_finite())
            .collect();
        self.selected_postscript_hint = None;
        self.truetype_instructions = hinting
            .truetype_instructions
            .into_iter()
            .filter(|i| i.ppem > 0 && i.delta_pixels.is_finite())
            .collect();
        self.selected_truetype_instruction = None;
        self.truetype_instructor_ppem = hinting.truetype_instructor_ppem.max(1);
        self.drag_transients.hoi_layer_sets = hinting
            .hoi_layer_sets
            .into_iter()
            .filter(|s| {
                s.c1.x.0.is_finite()
                    && s.c1.y.0.is_finite()
                    && s.c2.x.0.is_finite()
                    && s.c2.y.0.is_finite()
            })
            .collect();
        self.drag_transients.hoi_sibling_contours = hinting.hoi_sibling_contours;
        self.show_pixel_preview_ppem = hinting.show_pixel_preview_ppem.filter(|&p| p > 0);
        self.custom_reference_frame = reference_frame.map(|mut rf| {
            rf.keyline_circles_radii_upm.retain(|r| r.is_finite());
            rf
        });
        self.text_stream.writing_direction = writing_direction;
        self.curve_harmony.genius_nodes = curve_harmony.genius_nodes;
        self.curve_harmony.g3_nodes = curve_harmony.g3_nodes;
        let mut seen_sc = std::collections::HashSet::new();
        self.curve_harmony.smart_corners = curve_harmony
            .smart_corners
            .into_iter()
            .filter(|sc| sc.radius_upm.is_finite() && seen_sc.insert(sc.node_id))
            .collect();
        self.curve_harmony.reference_stems_upm = curve_harmony.reference_stems_upm;
        self.curve_harmony.curved_trajectories = curve_harmony.curved_trajectories;
        self.curve_harmony.active_mirror_axis = curve_harmony.active_mirror_axis;
        self.curve_harmony.round_overshoot_upm = curve_harmony.round_overshoot_upm;
    }

    /// Exports the current document scene into a serializable [`crate::core::CanvasDocumentScene`] (`SPEC-014 §2.4`).
    #[must_use]
    pub fn export_document_scene(&self) -> crate::core::CanvasDocumentScene {
        let mut metrics = self.metrics.clone();
        if metrics.advance_width <= 0.0 {
            metrics.advance_width = self.glyph.width;
        }
        metrics.italic_angle_deg = self.italic_angle_deg;
        let mut exported_glyph = self.glyph.clone();
        exported_glyph.contours = self.export_glyph().contours;
        crate::core::CanvasDocumentScene {
            glyph: exported_glyph,
            metrics,
            guides: crate::core::CanvasGuideSet {
                local_guides: self.guides.clone(),
                global_guides: self.global_guides.clone(),
                show_guides: self.show_guides,
                lock_guides: self.lock_guides,
            },
            layers: crate::core::CanvasLayerAttachments {
                background_contours: self.background_contours.clone(),
                anchors: self.anchors.clone(),
                components: self.components.clone(),
                contour_strokes: self.contour_strokes.clone(),
                companion_master_layers: self.companion_master_layers.clone(),
                smart_handles: self.smart_handles.clone(),
                background_image: self.background_image.clone(),
                backdrop_ghost_contours: self.backdrop_ghost_contours.clone(),
            },
            color_layers: self.color_sublayers.clone(),
            hinting: crate::core::CanvasHintingAndHoiScene {
                postscript_hints: self.postscript_hints.clone(),
                truetype_instructions: self.truetype_instructions.clone(),
                truetype_instructor_ppem: self.truetype_instructor_ppem,
                hoi_layer_sets: self.drag_transients.hoi_layer_sets.clone(),
                hoi_sibling_contours: self.drag_transients.hoi_sibling_contours.clone(),
                show_pixel_preview_ppem: self.show_pixel_preview_ppem,
            },
            reference_frame: self.custom_reference_frame.clone(),
            writing_direction: self.text_stream.writing_direction,
            curve_harmony: crate::core::CanvasCurveHarmonyScene {
                genius_nodes: self.curve_harmony.genius_nodes.clone(),
                g3_nodes: self.curve_harmony.g3_nodes.clone(),
                smart_corners: self.curve_harmony.smart_corners.clone(),
                reference_stems_upm: self.curve_harmony.reference_stems_upm.clone(),
                curved_trajectories: self.curve_harmony.curved_trajectories.clone(),
                active_mirror_axis: self.curve_harmony.active_mirror_axis,
                round_overshoot_upm: self.curve_harmony.round_overshoot_upm,
            },
            icon_studio: self.icon_studio.clone(),
        }
    }
}

fn sanitize_loaded_color_sublayer(
    mut sub: crate::core::CanvasColorSublayer,
) -> crate::core::CanvasColorSublayer {
    sub.opacity = if sub.opacity.is_finite() {
        sub.opacity.clamp(0.0, 1.0)
    } else {
        1.0
    };
    let sanitize_stops = |stops: &mut Vec<crate::core::GradientStop>| {
        for st in stops {
            st.offset = if st.offset.is_finite() {
                st.offset.clamp(0.0, 1.0)
            } else {
                0.0
            };
        }
    };
    match &mut sub.paint {
        crate::core::ColorLayerPaint::LinearGradient { stops, .. }
        | crate::core::ColorLayerPaint::RadialGradient { stops, .. }
        | crate::core::ColorLayerPaint::SweepGradient { stops, .. } => sanitize_stops(stops),
        crate::core::ColorLayerPaint::Solid { .. }
        | crate::core::ColorLayerPaint::Bitmap { .. }
        | crate::core::ColorLayerPaint::ClippingMask => {}
    }
    sub
}
