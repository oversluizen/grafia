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
use crate::tokens::{
    load_archetype_preset, AUTOFIT_MARGIN_RATIO, DEFAULT_CANVAS_HEIGHT_DP,
    DEFAULT_CANVAS_WIDTH_DP, GrafiaTheme, GlyphAGoldenMaster, GlyphArchetype,
};
use kurbo::{BezPath, Point};

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

    #[must_use]
    pub const fn allows_path_segment_interaction(self) -> bool {
        matches!(self, Self::Select | Self::Pen)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum CanvasViewMode { #[default] EditWireframe, PreviewFilled, ShapeThumbnail }

#[derive(Debug, Clone)]
pub struct GrafiaCanvasResponse {
    pub response: egui::Response,
    pub active_tool: CanvasTool,
    pub view_mode: CanvasViewMode,
    pub changed: bool,
    pub selected_nodes: Vec<NodeId>,
}

pub type GlifoCanvasResponse = GrafiaCanvasResponse;

impl std::ops::Deref for GrafiaCanvasResponse {
    type Target = egui::Response;
    fn deref(&self) -> &Self::Target { &self.response }
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
}

pub type GlifoCanvasState = GrafiaCanvasState;

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
            paths: Vec::new(), contours: Vec::new(), selected_nodes: Vec::new(),
            glyph: crate::core::Glyph::default(), active_tool: CanvasTool::Select,
            select_state: SelectToolState::Idle, pen_state: PenToolState::Idle,
            history: CommandHistory::new(), view_mode: CanvasViewMode::EditWireframe,
            prev_view_mode: None, theme, next_node_id: 1,
            drag_start_font_positions: Vec::new(), drag_start_node_types: Vec::new(),
            drag_start_contours: Vec::new(),
            active_contour_closed: false, last_click_time_pos: None,
            last_double_click_time_pos: None, pen_drag_last_pt: None,
            pen_handle_delta: (0.0, 0.0), hovered_segment_hit: None,
            active_archetype: GlyphArchetype::A, marquee_pre_selection: Vec::new(),
            marquee_modifiers: Modifiers::default(), alt_marquee_pending_start: None,
            active_drag_before_snapshot: None, active_open_contour: None,
            pending_pen_bcp_out: None, pending_pen_first_bcp_in: None,
            hovered_pen_close_target: None, hovered_pen_close_node: None,
            cursor_screen_pos: None, pen_alt_frozen_bcp_in: None,
            last_pointer_screen_pos: None, pending_segment_click: None,
            alt_duplicate_pending: false, pen_seg_has_explicit_h1: false,
            pen_resuming_corner_line_end: false, pen_join_b_had_out_handles: false,
            pen_join_b_was_smooth: false, grid_step_upm: crate::tokens::DEFAULT_GRID_STEP_UPM,
            show_bounding_box: false, show_bounding_box_slanted: false,
            selection_angle_deg: 0.0, italic_angle_deg: 0.0, round_to_grid: false,
            transform_grid_anchor: TransformGridAnchor::Center,
            transform_origin_mode: TransformOriginSpec::BoundingBoxCenter,
            transform_origin: FontPoint::new(0.0, 0.0), transform_origin_custom: false,
            clipboard_contours: Vec::new(), v020_drag: V020DragState::Idle,
            temp_tool_before_override: None, last_modifiers: Modifiers::default(),
            temp_zoom_via_key_space: false,
            guides: Vec::new(), global_guides: Vec::new(), selected_guide: None,
            show_guides: true, lock_guides: false, next_guide_id: 1,
            metrics: GlyphMetrics::default(), active_snappers: Vec::new(),
            guide_drag: GuideDragState::Idle,
            show_metrics: true, show_grid: true, grid_subdivisions: 1,
            grid_slope_rise: 0, grid_slope_run: 0,
            appearance_mode, system_dark_mode: is_dark, v040: V040ToolState::default(),
        }
    }

    pub fn set_appearance_mode(&mut self, mode: crate::tokens::AppearanceMode) {
        self.set_appearance_with_system(mode, self.system_dark_mode);
    }
    pub fn set_system_dark_mode(&mut self, system_is_dark: bool) {
        self.set_appearance_with_system(self.appearance_mode, system_is_dark);
    }
    pub fn set_appearance_with_system(&mut self, mode: crate::tokens::AppearanceMode, system_is_dark: bool) {
        self.appearance_mode = mode;
        self.system_dark_mode = system_is_dark;
        self.theme = GrafiaTheme::for_appearance(self.appearance_mode, self.system_dark_mode);
    }

    #[must_use]
    pub fn init_glyph_a() -> Self {
        let master = GlyphAGoldenMaster::load();
        let mut state = Self::new(GrafiaTheme::sonoma_default());
        state.viewport.upm = master.upm;
        let zoom = ((DEFAULT_CANVAS_HEIGHT_DP * 0.5) / master.cap_height as f32) * 0.5;
        let pan_x = (DEFAULT_CANVAS_WIDTH_DP - (master.width as f32 * zoom)) * 0.5;
        let pan_y = DEFAULT_CANVAS_HEIGHT_DP * 0.5 + (master.cap_height as f32 * zoom * 0.5);
        state.viewport.zoom = zoom;
        state.viewport.pan_offset = ScreenPoint::new(pan_x, pan_y);

        let outer: Vec<Node> = master.outer_contour.iter().map(|pt| {
            let n = Node::new(NodeId(state.next_node_id), pt.x, pt.y, NodeType::Corner);
            state.next_node_id += 1; n
        }).collect();
        let inner: Vec<Node> = master.inner_counter.iter().map(|pt| {
            let n = Node::new(NodeId(state.next_node_id), pt.x, pt.y, NodeType::Corner);
            state.next_node_id += 1; n
        }).collect();

        state.contours = vec![outer.clone(), inner.clone()];
        state.glyph = crate::core::Glyph {
            name: master.glyphname, upm: master.upm, width: master.width, height: master.cap_height,
            contours: vec![
                crate::core::GlyphContour::new(crate::core::ContourId(0), outer),
                crate::core::GlyphContour::new(crate::core::ContourId(1), inner),
            ],
            annotations: Vec::new(),
        };
        state.active_contour_closed = true;
        state.sync_paths_from_contours();
        state.assert_invariants();
        state
    }

    #[must_use]
    pub fn init_glyph_o() -> Self { let mut s = Self::init_glyph_a(); s.load_glyph_archetype(GlyphArchetype::O); s }
    #[must_use]
    pub fn init_glyph_n() -> Self { let mut s = Self::init_glyph_a(); s.load_glyph_archetype(GlyphArchetype::N); s }
    #[must_use]
    pub fn init_glyph_s() -> Self { let mut s = Self::init_glyph_a(); s.load_glyph_archetype(GlyphArchetype::S); s }

    pub fn load_glyph_archetype(&mut self, archetype: GlyphArchetype) {
        self.active_archetype = archetype;
        self.selected_nodes.clear();
        self.hovered_segment_hit = None;
        self.select_state = SelectToolState::Idle;
        self.pen_state = PenToolState::Idle;
        self.active_open_contour = None;
        self.pending_pen_bcp_out = None;
        self.pending_pen_first_bcp_in = None;
        self.hovered_pen_close_target = None;
        self.hovered_pen_close_node = None;
        if archetype == GlyphArchetype::A {
            let (theme, mode, sys_dark) = (self.theme.clone(), self.appearance_mode, self.system_dark_mode);
            *self = Self::init_glyph_a();
            (self.theme, self.appearance_mode, self.system_dark_mode) = (theme, mode, sys_dark);
            return;
        }
        let preset = load_archetype_preset(archetype);
        self.viewport.upm = preset.upm as u16;
        self.contours.clear();
        let mut g_contours = Vec::new();
        for (c_idx, cp) in preset.contours.iter().enumerate() {
            let nodes: Vec<Node> = cp.nodes.iter().map(|np| {
                let ntype = match np.node_type.as_str() {
                    "smooth" => NodeType::Smooth,
                    "off_curve" => NodeType::OffCurve,
                    _ => NodeType::Corner,
                };
                let n = Node::new(NodeId(self.next_node_id), np.x, np.y, ntype);
                self.next_node_id += 1; n
            }).collect();
            self.contours.push(nodes.clone());
            let mut gc = crate::core::GlyphContour::new(crate::core::ContourId(c_idx), nodes);
            gc.closed = cp.closed;
            g_contours.push(gc);
        }
        self.glyph = crate::core::Glyph {
            name: preset.glyph_name, upm: preset.upm as u16,
            width: preset.advance_width, height: preset.cap_height, contours: g_contours,
            annotations: Vec::new(),
        };
        self.active_contour_closed = true;
        self.sync_paths_from_contours();
        self.assert_invariants();
    }

    pub fn fit_viewport_to_glyph(&mut self, canvas_w: f32, canvas_h: f32) {
        let glyph_h = (self.glyph.height as f32).max(1.0);
        let glyph_w = (self.glyph.width as f32).max(1.0);
        let zoom = ((canvas_h * AUTOFIT_MARGIN_RATIO) / glyph_h).max(crate::tokens::MIN_VIEWPORT_ZOOM);
        let pan_x = (canvas_w - glyph_w * zoom) * 0.5;
        let pan_y = canvas_h * 0.5 + (glyph_h * zoom * 0.5);
        self.viewport.zoom = zoom;
        self.viewport.pan_offset = ScreenPoint::new(pan_x, pan_y);
    }

    #[must_use]
    pub fn is_contour_closed(&self, c_idx: usize) -> bool {
        if let Some(gc) = self.glyph.contours.get(c_idx) {
            gc.closed
        } else if c_idx + 1 == self.contours.len() {
            self.active_contour_closed
        } else {
            true
        }
    }

    #[must_use]
    pub fn closed_flags(&self) -> Vec<bool> { (0..self.contours.len()).map(|i| self.is_contour_closed(i)).collect() }

    pub fn set_contour_closed(&mut self, c_idx: usize, closed: bool) {
        while self.glyph.contours.len() <= c_idx && self.glyph.contours.len() < self.contours.len() {
            let idx = self.glyph.contours.len();
            let mut gc = crate::core::GlyphContour::new(crate::core::ContourId(idx), self.contours[idx].clone());
            gc.closed = if idx + 1 == self.contours.len() { self.active_contour_closed } else { true };
            self.glyph.contours.push(gc);
        }
        if let Some(c) = self.glyph.contours.get_mut(c_idx) { c.closed = closed; }
        if c_idx + 1 == self.contours.len() || self.active_open_contour == Some(c_idx) {
            self.active_contour_closed = closed;
        }
    }

    pub fn sync_paths_from_contours(&mut self) {
        self.paths.clear();
        let pt_of = |n: &Node| Point::new(n.point.x.0, n.point.y.0);
        for (c_idx, contour) in self.contours.iter().enumerate() {
            if contour.is_empty() {
                self.paths.push(BezPath::new());
                continue;
            }
            let is_closed = self.is_contour_closed(c_idx);
            let mut path = BezPath::new();
            path.move_to(pt_of(&contour[0]));
            let mut i = 1;
            while i < contour.len() {
                if contour[i].node_type == NodeType::OffCurve {
                    if i + 2 < contour.len() && contour[i + 1].node_type == NodeType::OffCurve {
                        path.curve_to(pt_of(&contour[i]), pt_of(&contour[i + 1]), pt_of(&contour[i + 2])); i += 3;
                    } else if is_closed && i + 1 == contour.len() - 1 && contour[i + 1].node_type == NodeType::OffCurve {
                        path.curve_to(pt_of(&contour[i]), pt_of(&contour[i + 1]), pt_of(&contour[0])); i += 2;
                    } else if i + 1 < contour.len() {
                        path.quad_to(pt_of(&contour[i]), pt_of(&contour[i + 1])); i += 2;
                    } else if is_closed && i == contour.len() - 1 {
                        path.quad_to(pt_of(&contour[i]), pt_of(&contour[0])); i += 1;
                    } else { path.line_to(pt_of(&contour[i])); i += 1; }
                } else { path.line_to(pt_of(&contour[i])); i += 1; }
            }
            if is_closed { path.close_path(); }
            self.paths.push(path);
        }
        let old_closed: Vec<bool> = (0..self.contours.len()).map(|i| self.is_contour_closed(i)).collect();
        let old_meta: Vec<(crate::core::ShapeBooleanRole, bool, bool)> = (0..self.contours.len())
            .map(|i| self.glyph.contours.get(i).map_or((crate::core::ShapeBooleanRole::Normal, true, false), |gc| (gc.role, gc.visible, gc.locked)))
            .collect();
        self.glyph.contours = self.contours.iter().enumerate().map(|(idx, nodes)| {
            let mut gc = crate::core::GlyphContour::new(crate::core::ContourId(idx), nodes.clone());
            gc.closed = old_closed[idx];
            gc.role = old_meta[idx].0;
            gc.visible = old_meta[idx].1;
            gc.locked = old_meta[idx].2;
            gc
        }).collect();
        let mut valid_ids = std::collections::HashSet::new();
        for contour in &self.contours {
            for n in contour {
                valid_ids.insert(n.id);
                if n.id.0 >= self.next_node_id {
                    self.next_node_id = n.id.0 + 1;
                }
            }
        }
        let mut seen_sel = std::collections::HashSet::with_capacity(self.selected_nodes.len());
        self.selected_nodes.retain(|id| valid_ids.contains(id) && seen_sel.insert(*id));
        let (c_len, pen_idle) = (self.contours.len(), self.pen_state == PenToolState::Idle);
        if self.active_open_contour.is_some_and(|i| i >= c_len || (pen_idle && self.is_contour_closed(i))) {
            self.active_open_contour = None;
        }
        if self.pending_pen_bcp_out.is_some_and(|(i, _)| i >= c_len || (pen_idle && self.is_contour_closed(i))) {
            self.pending_pen_bcp_out = None;
        }
        if self.pending_pen_first_bcp_in.is_some_and(|(i, _)| i >= c_len || (pen_idle && self.is_contour_closed(i))) {
            self.pending_pen_first_bcp_in = None;
        }
        if self.hovered_pen_close_target.is_some_and(|(i, id)| i >= c_len || !valid_ids.contains(&id)) {
            self.hovered_pen_close_target = None;
        }
        if self.hovered_pen_close_node.is_some_and(|id| !valid_ids.contains(&id)) {
            self.hovered_pen_close_node = None;
        }
        if self.hovered_segment_hit.as_ref().is_some_and(|h| h.contour_idx >= c_len) {
            self.hovered_segment_hit = None;
        }
    }

    #[must_use]
    pub fn find_node(&self, id: NodeId) -> Option<&Node> {
        self.contours.iter().flat_map(|c| c.iter()).find(|n| n.id == id)
    }

    pub fn find_node_mut(&mut self, id: NodeId) -> Option<&mut Node> {
        self.contours.iter_mut().flat_map(|c| c.iter_mut()).find(|n| n.id == id)
    }
}
