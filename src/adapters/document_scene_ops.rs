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

/// Internal drag-session, marquee, tool-override, and auxiliary transient state (`SPEC-014 §2.2`).
#[derive(Debug, Clone)]
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
    pub active_gradient_drag: Option<GradientDragState>,
    pub guide_drag: GuideDragState,
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

impl Default for DragSessionTransients {
    fn default() -> Self {
        Self {
            prev_view_mode: None,
            drag_start_font_positions: Vec::new(),
            drag_start_node_types: Vec::new(),
            drag_start_contours: Vec::new(),
            last_click_time_pos: None,
            last_double_click_time_pos: None,
            marquee_pre_selection: Vec::new(),
            marquee_modifiers: Modifiers::default(),
            alt_marquee_pending_start: None,
            active_drag_before_snapshot: None,
            pending_segment_click: None,
            alt_duplicate_pending: false,
            temp_tool_before_override: None,
            last_modifiers: Modifiers::default(),
            temp_zoom_via_key_space: false,
            ruler_drag_is_horizontal: None,
            pre_all_layers_select_snapshot: None,
            saved_viewport: None,
            active_smart_handle_drag: None,
            active_gradient_drag: None,
            guide_drag: GuideDragState::Idle,
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
        ) || matches!(self.pen_state, PenToolState::DraggingHandles { .. })
            || self.bbox_drag != BBoxDragState::Idle
            || self.drag_transients.guide_drag != GuideDragState::Idle
            || self.drag_transients.active_gradient_drag.is_some()
            || self.drag_transients.active_smart_handle_drag.is_some()
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
        self.italic_angle_deg = italic_deg;
        self.guides = scene.guides.local_guides;
        self.global_guides = scene.guides.global_guides;
        self.show_guides = scene.guides.show_guides;
        self.lock_guides = scene.guides.lock_guides;
        self.selected_guide = None;
        for g in self.guides.iter().chain(self.global_guides.iter()) {
            self.drag_transients.next_guide_id = self.drag_transients.next_guide_id.max(g.id.0 + 1);
        }
        self.background_contours = scene.layers.background_contours;
        self.anchors = scene.layers.anchors;
        for a in &self.anchors {
            self.drag_transients.next_anchor_id =
                self.drag_transients.next_anchor_id.max(a.id.0 + 1);
        }
        self.components = scene.layers.components;
        self.contour_strokes = scene.layers.contour_strokes;
        self.companion_master_layers = scene.layers.companion_master_layers;
        self.smart_handles = scene.layers.smart_handles;
        self.background_image = scene.layers.background_image;
        self.backdrop_ghost_contours = scene.layers.backdrop_ghost_contours;
        self.color_sublayers = scene.color_layers;
        self.drag_transients.selected_color_sublayer = None;
        self.drag_transients.selected_gradient_stop = None;
        self.postscript_hints = scene.hinting.postscript_hints;
        self.selected_postscript_hint = None;
        self.truetype_instructions = scene.hinting.truetype_instructions;
        self.selected_truetype_instruction = None;
        self.truetype_instructor_ppem = scene.hinting.truetype_instructor_ppem.max(1);
        self.drag_transients.hoi_layer_sets = scene.hinting.hoi_layer_sets;
        self.drag_transients.hoi_sibling_contours = scene.hinting.hoi_sibling_contours;
        self.show_pixel_preview_ppem = scene.hinting.show_pixel_preview_ppem;
        self.custom_reference_frame = scene.reference_frame;
        self.text_stream.writing_direction = scene.writing_direction;
        self.history = CommandHistory::new();
        self.bump_content_generation();
        self.assert_invariants();
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
        }
    }
}
