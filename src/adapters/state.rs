//! Canvas state, tool modes, and coordinate management for `GrafiaCanvasWidget`.

use crate::adapters::bbox_selection_tools::BBoxDragState;
pub use crate::adapters::document_scene_ops::{
    DragSessionTransients, PenInteractionTransients, TextInteractionTransients,
};
use crate::adapters::drawing_tool_state::DrawingToolState;
use crate::core::{
    CanvasColorSublayer, ClipboardContour, CustomReferenceFrame, FontPoint, GlyphContour, Node,
    NodeId, PostScriptHint, ScreenPoint, SegmentHit, TransformGridAnchor, TransformOriginSpec,
    TrueTypeVisualInstruction, ViewportTransform,
};
use crate::interaction::{CommandHistory, PenToolState, SelectToolState};
use crate::snapping::{ActiveSnapper, GlyphMetrics, Guide, SelectedGuideRef};
use crate::tokens::{GlyphArchetype, GrafiaTheme};
use kurbo::BezPath;
use std::sync::atomic::{AtomicU64, Ordering};

static NEXT_CONTENT_GENERATION: AtomicU64 = AtomicU64::new(1);

pub(crate) fn next_content_generation() -> u64 {
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
    TrueTypeInstructor,
    Hoi,
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
    pub const TrueTypeInstructorTool: CanvasTool = CanvasTool::TrueTypeInstructor;
    pub const HoiTool: CanvasTool = CanvasTool::Hoi;
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
    pub theme: GrafiaTheme,
    pub next_node_id: u64,
    pub active_contour_closed: bool,
    pub hovered_segment_hit: Option<SegmentHit>,
    pub active_archetype: GlyphArchetype,
    pub active_open_contour: Option<usize>,
    pub pending_pen_bcp_out: Option<(usize, FontPoint)>,
    pub pending_pen_first_bcp_in: Option<(usize, FontPoint)>,
    pub hovered_pen_close_target: Option<(usize, NodeId)>,
    pub cursor_screen_pos: Option<ScreenPoint>,
    pub last_pointer_screen_pos: Option<ScreenPoint>,
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
    pub bbox_drag: BBoxDragState,
    pub guides: Vec<Guide>,
    pub global_guides: Vec<Guide>,
    pub selected_guide: Option<SelectedGuideRef>,
    pub show_guides: bool,
    pub lock_guides: bool,
    pub metrics: GlyphMetrics,
    pub active_snappers: Vec<ActiveSnapper>,
    pub show_metrics: bool,
    pub show_grid: bool,
    pub grid_subdivisions: u32,
    pub grid_slope_rise: i32,
    pub grid_slope_run: i32,
    pub appearance_mode: crate::tokens::AppearanceMode,
    pub system_dark_mode: bool,
    pub drawing_tool_state: DrawingToolState,
    pub text_stream: crate::core::CanvasTextStream,
    pub show_text_measurement_line: bool,
    pub text_measurement_y_upm: f64,
    pub text_measurement_dragging: bool,
    pub multi_selection_proportional: bool,
    pub show_rulers: bool,
    pub background_contours: Vec<crate::core::GlyphContour>,
    pub show_background: bool,
    pub editing_background_layer: bool,
    pub anchors: Vec<crate::core::CanvasAnchor>,
    pub show_selection_info_hud: bool,
    pub ghost_paths: Vec<crate::core::GhostMarkPath>,
    pub show_mark_cloud: bool,
    pub components: Vec<crate::core::CanvasComponent>,
    pub host_requests: Vec<crate::core::CanvasHostRequest>,
    pub contour_strokes:
        std::collections::BTreeMap<crate::core::ContourId, crate::core::ContourStrokeStyle>,
    pub companion_master_layers: Vec<crate::core::CompanionMasterLayer>,
    pub show_master_compatibility: bool,
    pub select_all_layers_mode: bool,
    pub background_image: Option<crate::tokens::CanvasBackgroundImage>,
    pub smart_handles: Vec<crate::tokens::CanvasSmartHandle>,
    pub show_extra_nodes: bool,
    pub selected_extra_node: Option<crate::tokens::ExtraNodeKey>,
    pub show_nodes_oncurve: bool,
    pub show_nodes_offcurve: bool,
    pub show_node_coordinates: bool,
    pub show_background_nodes: bool,
    pub show_component_piece_frames: bool,
    pub active_bbox_rotation_deg: Option<f64>,
    pub preferences: crate::tokens::GrafiaPreferences,
    pub color_sublayers: Vec<CanvasColorSublayer>,
    pub custom_reference_frame: Option<CustomReferenceFrame>,
    pub postscript_hints: Vec<PostScriptHint>,
    pub selected_postscript_hint: Option<u64>,
    pub truetype_instructions: Vec<TrueTypeVisualInstruction>,
    pub selected_truetype_instruction: Option<u64>,
    pub truetype_instructor_ppem: u16,
    pub show_hoi_diamonds: bool,
    pub show_pixel_preview_ppem: Option<u16>,
    pub backdrop_ghost_contours: Vec<GlyphContour>,
    pub content_generation: u64,
    pub pen_transients: PenInteractionTransients,
    pub drag_transients: DragSessionTransients,
    pub text_transients: TextInteractionTransients,
}

macro_rules! construct_default_canvas_state {
    ($theme:expr, $appearance_mode:expr, $is_dark:expr) => {
        GrafiaCanvasState {
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
            theme: $theme,
            next_node_id: 1,
            active_contour_closed: false,
            hovered_segment_hit: None,
            active_archetype: GlyphArchetype::A,
            active_open_contour: None,
            pending_pen_bcp_out: None,
            pending_pen_first_bcp_in: None,
            hovered_pen_close_target: None,
            cursor_screen_pos: None,
            last_pointer_screen_pos: None,
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
            bbox_drag: BBoxDragState::Idle,
            guides: Vec::new(),
            global_guides: Vec::new(),
            selected_guide: None,
            show_guides: true,
            lock_guides: false,
            metrics: GlyphMetrics::default(),
            active_snappers: Vec::new(),
            show_metrics: true,
            show_grid: true,
            grid_subdivisions: 1,
            grid_slope_rise: 0,
            grid_slope_run: 0,
            appearance_mode: $appearance_mode,
            system_dark_mode: $is_dark,
            drawing_tool_state: DrawingToolState::default(),
            text_stream: crate::core::CanvasTextStream::new(),
            show_text_measurement_line: false,
            text_measurement_y_upm: crate::tokens::TEXT_MEASUREMENT_LINE_DEFAULT_Y_UPM,
            text_measurement_dragging: false,
            multi_selection_proportional: false,
            show_rulers: false,
            background_contours: Vec::new(),
            show_background: true,
            editing_background_layer: false,
            anchors: Vec::new(),
            show_selection_info_hud: true,
            ghost_paths: Vec::new(),
            show_mark_cloud: true,
            components: Vec::new(),
            host_requests: Vec::new(),
            contour_strokes: std::collections::BTreeMap::new(),
            companion_master_layers: Vec::new(),
            show_master_compatibility: false,
            select_all_layers_mode: false,
            background_image: None,
            smart_handles: Vec::new(),
            show_extra_nodes: false,
            selected_extra_node: None,
            show_nodes_oncurve: true,
            show_nodes_offcurve: true,
            show_node_coordinates: false,
            show_background_nodes: false,
            show_component_piece_frames: false,
            active_bbox_rotation_deg: None,
            preferences: crate::tokens::GrafiaPreferences {
                round_to_grid: false,
                snap_radius_dp: crate::tokens::DEFAULT_SNAP_THRESHOLD_DP,
                hit_test_radius_dp: crate::tokens::SELECT_NODE_HIT_RADIUS_DP,
                scroll_wheel_zooms: false,
                passive_neighbor_opacity: 1.0,
                ..crate::tokens::GrafiaPreferences::default()
            },
            color_sublayers: Vec::new(),
            custom_reference_frame: None,
            postscript_hints: Vec::new(),
            selected_postscript_hint: None,
            truetype_instructions: Vec::new(),
            selected_truetype_instruction: None,
            truetype_instructor_ppem: crate::core::math_consts::TT_DEFAULT_PPEM,
            show_hoi_diamonds: false,
            show_pixel_preview_ppem: None,
            backdrop_ghost_contours: Vec::new(),
            content_generation: next_content_generation(),
            pen_transients: PenInteractionTransients::default(),
            drag_transients: DragSessionTransients::default(),
            text_transients: TextInteractionTransients::default(),
        }
    };
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
        construct_default_canvas_state!(theme, appearance_mode, is_dark)
    }
}
