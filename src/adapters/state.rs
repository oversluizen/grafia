//! Canvas state, tool modes, and coordinate management for `GrafiaCanvasWidget`.

use crate::adapters::v020_tools::V020DragState;
use crate::adapters::v030_guides::GuideDragState;
use crate::adapters::v040_tools::V040ToolState;
use crate::core::{
    ClipboardContour, FontPoint, Node, NodeId, NodeType, ScreenPoint, SegmentHit,
    TransformGridAnchor, TransformOriginSpec, ViewportTransform,
};
use crate::interaction::{CommandHistory, GlyphSnapshot, Modifiers, PenToolState, SelectToolState};
use crate::snapping::{ActiveSnapper, GlyphMetrics, Guide, SelectedGuideRef};
use crate::tokens::{GlyphArchetype, GrafiaTheme};
use kurbo::BezPath;
use std::sync::atomic::{AtomicU64, Ordering};

static NEXT_CONTENT_GENERATION: AtomicU64 = AtomicU64::new(1);

fn next_content_generation() -> u64 {
    NEXT_CONTENT_GENERATION.fetch_add(1, Ordering::Relaxed)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum CanvasTool {
    #[default]
    Select,
    Pen,
    Rotate,
    Scale,
    Lasso,
    Zoom,
    Knife,
    Primitives,
    Measurement,
    Annotation,
    Pencil,
    Pixel,
    Text,
}

#[allow(non_upper_case_globals)]
impl CanvasTool {
    pub const SelectTool: CanvasTool = CanvasTool::Select;
    pub const PenTool: CanvasTool = CanvasTool::Pen;
    pub const RotateTool: CanvasTool = CanvasTool::Rotate;
    pub const ScaleTool: CanvasTool = CanvasTool::Scale;
    pub const LassoTool: CanvasTool = CanvasTool::Lasso;
    pub const ZoomTool: CanvasTool = CanvasTool::Zoom;
    pub const KnifeTool: CanvasTool = CanvasTool::Knife;
    pub const PrimitivesTool: CanvasTool = CanvasTool::Primitives;
    pub const MeasurementTool: CanvasTool = CanvasTool::Measurement;
    pub const AnnotationTool: CanvasTool = CanvasTool::Annotation;
    pub const PencilTool: CanvasTool = CanvasTool::Pencil;
    pub const PixelTool: CanvasTool = CanvasTool::Pixel;
    pub const TextTool: CanvasTool = CanvasTool::Text;
    #[must_use]
    pub const fn allows_path_segment_interaction(self) -> bool {
        matches!(self, Self::Select | Self::Pen)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum CanvasViewMode {
    #[default]
    EditWireframe,
    PreviewFilled,
    ShapeThumbnail,
}

#[derive(Debug, Clone)]
pub struct GrafiaCanvasResponse {
    pub response: egui::Response,
    pub active_tool: CanvasTool,
    pub view_mode: CanvasViewMode,
    pub changed: bool,
    pub selected_nodes: Vec<NodeId>,
}

impl std::ops::Deref for GrafiaCanvasResponse {
    type Target = egui::Response;
    fn deref(&self) -> &Self::Target {
        &self.response
    }
}

#[derive(Debug)]
pub struct GrafiaCanvasState {
    pub viewport: ViewportTransform,
    pub paths: Vec<BezPath>,
    pub contours: Vec<Vec<Node>>,
    pub selected_nodes: Vec<NodeId>,
    pub glyph: crate::core::Glyph,
    pub active_tool: CanvasTool,
    pub select_state: SelectToolState,
    pub pen_state: PenToolState,
    pub history: CommandHistory,
    pub view_mode: CanvasViewMode,
    pub prev_view_mode: Option<CanvasViewMode>,
    pub theme: GrafiaTheme,
    pub next_node_id: u64,
    pub drag_start_font_positions: Vec<(NodeId, FontPoint)>,
    pub drag_start_node_types: Vec<(NodeId, FontPoint, NodeType)>,
    pub drag_start_contours: Vec<Vec<Node>>,
    pub active_contour_closed: bool,
    pub last_click_time_pos: Option<(f64, ScreenPoint)>,
    pub last_double_click_time_pos: Option<(f64, ScreenPoint)>,
    pub pen_drag_last_pt: Option<FontPoint>,
    pub pen_handle_delta: (f64, f64),
    pub hovered_segment_hit: Option<SegmentHit>,
    pub active_archetype: GlyphArchetype,
    pub marquee_pre_selection: Vec<NodeId>,
    pub marquee_modifiers: Modifiers,
    pub alt_marquee_pending_start: Option<ScreenPoint>,
    pub active_drag_before_snapshot: Option<GlyphSnapshot>,
    pub active_open_contour: Option<usize>,
    pub pending_pen_bcp_out: Option<(usize, FontPoint)>,
    pub pending_pen_first_bcp_in: Option<(usize, FontPoint)>,
    pub hovered_pen_close_target: Option<(usize, NodeId)>,
    pub hovered_pen_close_node: Option<NodeId>,
    pub cursor_screen_pos: Option<ScreenPoint>,
    pub pen_alt_frozen_bcp_in: Option<FontPoint>,
    pub last_pointer_screen_pos: Option<ScreenPoint>,
    pub pending_segment_click: Option<(usize, crate::core::ContourSegment, Modifiers)>,
    pub alt_duplicate_pending: bool,
    pub pen_seg_has_explicit_h1: bool,
    pub pen_resuming_corner_line_end: bool,
    pub pen_join_b_had_out_handles: bool,
    pub pen_join_b_was_smooth: bool,
    pub grid_step_upm: f64,
    pub show_bounding_box: bool,
    pub show_bounding_box_slanted: bool,
    pub selection_angle_deg: f64,
    pub italic_angle_deg: f64,
    pub round_to_grid: bool,
    pub transform_grid_anchor: TransformGridAnchor,
    pub transform_origin_mode: TransformOriginSpec,
    pub transform_origin: FontPoint,
    pub transform_origin_custom: bool,
    pub clipboard_contours: Vec<ClipboardContour>,
    pub v020_drag: V020DragState,
    pub temp_tool_before_override: Option<CanvasTool>,
    pub last_modifiers: Modifiers,
    pub temp_zoom_via_key_space: bool,
    pub guides: Vec<Guide>,
    pub global_guides: Vec<Guide>,
    pub selected_guide: Option<SelectedGuideRef>,
    pub show_guides: bool,
    pub lock_guides: bool,
    pub next_guide_id: u64,
    pub metrics: GlyphMetrics,
    pub active_snappers: Vec<ActiveSnapper>,
    pub guide_drag: GuideDragState,
    pub show_metrics: bool,
    pub show_grid: bool,
    pub grid_subdivisions: u32,
    pub grid_slope_rise: i32,
    pub grid_slope_run: i32,
    pub appearance_mode: crate::tokens::AppearanceMode,
    pub system_dark_mode: bool,
    pub v040: V040ToolState,
    pub text_stream: crate::core::CanvasTextStream,
    pub show_text_measurement_line: bool,
    pub temp_show_text_measurement_line: bool,
    pub text_measurement_y_upm: f64,
    pub text_mode_numbers_threshold: f64,
    pub last_select_tool: CanvasTool,
    pub text_drag_anchor_caret: Option<(usize, ScreenPoint)>,
    pub text_measurement_dragging: bool,
    pub suppress_text_stream_sync: bool,
    pub text_tool_activated_in_frame: bool,
    pub multi_selection_proportional: bool,
    pub show_rulers: bool,
    pub ruler_drag_is_horizontal: Option<bool>,
    pub background_contours: Vec<crate::core::GlyphContour>,
    pub show_background: bool,
    pub editing_background_layer: bool,
    pub editing_background: bool,
    pub anchors: Vec<crate::core::CanvasAnchor>,
    pub next_anchor_id: u64,
    pub center_in_width_count: u64,
    pub preview_config: crate::render::PreviewRenderConfig,
    pub show_selection_info_hud: bool,
    pub ghost_paths: Vec<crate::core::GhostMarkPath>,
    pub show_mark_cloud: bool,
    pub components: Vec<crate::core::CanvasComponent>,
    pub host_requests: Vec<crate::core::CanvasHostRequest>,
    pub fit_curve_config: crate::core::FitCurveConfig,
    pub contour_strokes:
        std::collections::BTreeMap<crate::core::ContourId, crate::core::ContourStrokeStyle>,
    pub companion_master_layers: Vec<crate::core::CompanionMasterLayer>,
    pub show_master_compatibility: bool,
    pub select_all_layers_mode: bool,
    pub selected_companion_nodes: Vec<(u32, NodeId)>,
    pub pre_all_layers_select_snapshot: Option<GlyphSnapshot>,
    pub content_generation: u64,
}

impl GrafiaCanvasState {
    #[must_use]
    pub fn new(theme: GrafiaTheme) -> Self {
        let is_dark = theme.is_dark();
        let appearance_mode = if is_dark {
            crate::tokens::AppearanceMode::Dark
        } else {
            crate::tokens::AppearanceMode::Light
        };
        Self {
            viewport: ViewportTransform::new(1.0, ScreenPoint::new(0.0, 0.0), 1000),
            paths: Vec::new(),
            contours: Vec::new(),
            selected_nodes: Vec::new(),
            glyph: crate::core::Glyph::default(),
            active_tool: CanvasTool::Select,
            select_state: SelectToolState::Idle,
            pen_state: PenToolState::Idle,
            history: CommandHistory::new(),
            view_mode: CanvasViewMode::EditWireframe,
            prev_view_mode: None,
            theme,
            next_node_id: 1,
            drag_start_font_positions: Vec::new(),
            drag_start_node_types: Vec::new(),
            drag_start_contours: Vec::new(),
            active_contour_closed: false,
            last_click_time_pos: None,
            last_double_click_time_pos: None,
            pen_drag_last_pt: None,
            pen_handle_delta: (0.0, 0.0),
            hovered_segment_hit: None,
            active_archetype: GlyphArchetype::A,
            marquee_pre_selection: Vec::new(),
            marquee_modifiers: Modifiers::default(),
            alt_marquee_pending_start: None,
            active_drag_before_snapshot: None,
            active_open_contour: None,
            pending_pen_bcp_out: None,
            pending_pen_first_bcp_in: None,
            hovered_pen_close_target: None,
            hovered_pen_close_node: None,
            cursor_screen_pos: None,
            pen_alt_frozen_bcp_in: None,
            last_pointer_screen_pos: None,
            pending_segment_click: None,
            alt_duplicate_pending: false,
            pen_seg_has_explicit_h1: false,
            pen_resuming_corner_line_end: false,
            pen_join_b_had_out_handles: false,
            pen_join_b_was_smooth: false,
            grid_step_upm: crate::tokens::DEFAULT_GRID_STEP_UPM,
            show_bounding_box: false,
            show_bounding_box_slanted: false,
            selection_angle_deg: 0.0,
            italic_angle_deg: 0.0,
            round_to_grid: false,
            transform_grid_anchor: TransformGridAnchor::Center,
            transform_origin_mode: TransformOriginSpec::BoundingBoxCenter,
            transform_origin: FontPoint::new(0.0, 0.0),
            transform_origin_custom: false,
            clipboard_contours: Vec::new(),
            v020_drag: V020DragState::Idle,
            temp_tool_before_override: None,
            last_modifiers: Modifiers::default(),
            temp_zoom_via_key_space: false,
            guides: Vec::new(),
            global_guides: Vec::new(),
            selected_guide: None,
            show_guides: true,
            lock_guides: false,
            next_guide_id: 1,
            metrics: GlyphMetrics::default(),
            active_snappers: Vec::new(),
            guide_drag: GuideDragState::Idle,
            show_metrics: true,
            show_grid: true,
            grid_subdivisions: 1,
            grid_slope_rise: 0,
            grid_slope_run: 0,
            appearance_mode,
            system_dark_mode: is_dark,
            v040: V040ToolState::default(),
            text_stream: crate::core::CanvasTextStream::new(),
            show_text_measurement_line: false,
            temp_show_text_measurement_line: false,
            text_measurement_y_upm: crate::tokens::TEXT_MEASUREMENT_LINE_DEFAULT_Y_UPM,
            text_mode_numbers_threshold: crate::tokens::TEXT_MODE_NUMBERS_THRESHOLD_DEFAULT,
            last_select_tool: CanvasTool::Select,
            text_drag_anchor_caret: None,
            text_measurement_dragging: false,
            suppress_text_stream_sync: false,
            text_tool_activated_in_frame: false,
            multi_selection_proportional: false,
            show_rulers: false,
            ruler_drag_is_horizontal: None,
            background_contours: Vec::new(),
            show_background: true,
            editing_background_layer: false,
            editing_background: false,
            anchors: Vec::new(),
            next_anchor_id: 1,
            center_in_width_count: 0,
            preview_config: crate::render::PreviewRenderConfig::default(),
            show_selection_info_hud: true,
            ghost_paths: Vec::new(),
            show_mark_cloud: true,
            components: Vec::new(),
            host_requests: Vec::new(),
            fit_curve_config: crate::core::FitCurveConfig::default(),
            contour_strokes: std::collections::BTreeMap::new(),
            companion_master_layers: Vec::new(),
            show_master_compatibility: false,
            select_all_layers_mode: false,
            selected_companion_nodes: Vec::new(),
            pre_all_layers_select_snapshot: None,
            content_generation: next_content_generation(),
        }
    }

    /// Returns the monotonic content revision counter incremented on any geometry, layer, text-stream,
    /// anchor, component, stroke, or light/dark theme change.
    #[must_use]
    pub const fn content_generation(&self) -> u64 {
        self.content_generation
    }

    /// Advances `content_generation` to a new monotonically increasing value.
    pub fn bump_content_generation(&mut self) {
        self.content_generation = next_content_generation();
    }

    pub fn set_appearance_mode(&mut self, mode: crate::tokens::AppearanceMode) {
        self.set_appearance_with_system(mode, self.system_dark_mode);
    }

    pub fn set_system_dark_mode(&mut self, system_is_dark: bool) {
        self.set_appearance_with_system(self.appearance_mode, system_is_dark);
    }

    pub fn set_appearance_with_system(
        &mut self,
        mode: crate::tokens::AppearanceMode,
        system_is_dark: bool,
    ) {
        if self.appearance_mode != mode || self.system_dark_mode != system_is_dark {
            self.appearance_mode = mode;
            self.system_dark_mode = system_is_dark;
            self.theme = GrafiaTheme::for_appearance(self.appearance_mode, self.system_dark_mode);
            self.bump_content_generation();
        }
    }
}
