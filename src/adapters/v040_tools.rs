//! `SPEC-005` (`grafia v0.4.0`) tool state machines & `GrafiaCanvasState` helper methods
//! for `Knife`, `Primitives`, `Measurement`, `Annotation`, `Pencil`, and `Pixel` tools.

use crate::adapters::state::{CanvasTool, GrafiaCanvasState};
use crate::core::{
    build_circle_contour_nodes, build_rectangle_contour_nodes, compute_layer_measurement_labels,
    compute_measurement_ray_intersections, compute_primitive_rect_upm, remove_overlap_in_contours,
    AnnotationId, AnnotationKind, AnnotationSubTool, CanvasAnnotation, ContourId, FontPoint,
    GlyphContour, MeasurementLayerLabels, NodeId, PrimitiveSubTool, ToolDialogRequest,
};
use crate::snapping::{Guide, GuideId, SelectedGuideRef};
use crate::tokens::{KNIFE_MIN_DRAG_UPM, PIXEL_DEFAULT_STEP_UPM, PRIMITIVE_MIN_SIZE_UPM};
use egui::Key;

/// Active drag state for `KnifeTool` (`E` / `Shift+E` — `CanvasTool`).
#[derive(Debug, Clone, Default, PartialEq)]
pub struct KnifeDragState {
    pub active: bool,
    pub start_upm: FontPoint,
    pub current_upm: FontPoint,
    pub intersections: Vec<FontPoint>,
    pub alt_add_nodes: bool,
}

/// Active drag state for `PrimitivesTool` (`F` / `Shift+F` — `CanvasTool`).
#[derive(Debug, Clone, Default, PartialEq)]
pub struct PrimitiveDragState {
    pub active: bool,
    pub start_upm: FontPoint,
    pub current_upm: FontPoint,
    pub last_raw_upm: FontPoint,
    pub shift: bool,
    pub alt: bool,
    pub round_to_grid: bool,
    pub grid_step_upm: f64,
}

impl PrimitiveDragState {
    /// Returns `(min_x, min_y, max_x, max_y)` in UPM for the in-flight primitive drag.
    #[must_use]
    pub fn preview_rect_upm(&self) -> (f64, f64, f64, f64) {
        compute_primitive_rect_upm(
            self.start_upm,
            self.current_upm,
            self.shift,
            self.alt,
            self.grid_step_upm,
            self.round_to_grid,
        )
    }
}

/// Active drag state for `MeasurementTool` (`L` & `Cmd+Ctrl+Opt` `spec_ref` — `CanvasTool`).
#[derive(Debug, Clone, Default, PartialEq)]
pub struct MeasurementDragState {
    pub active: bool,
    pub start_upm: FontPoint,
    pub current_upm: FontPoint,
    pub intersections: Vec<FontPoint>,
}

/// Active drag mode for `AnnotationTool` (`A` — `AnnotationTool`).
#[derive(Debug, Clone, Default, PartialEq)]
pub enum AnnotationDragMode {
    #[default]
    Idle,
    MoveBody {
        start_upm: FontPoint,
        initial_positions: Vec<(AnnotationId, FontPoint)>,
    },
    DragHandle {
        id: AnnotationId,
        start_upm: FontPoint,
        initial_kind: AnnotationKind,
    },
}

/// Active freehand stroke state for `PencilTool` (`B` — `PenTool`).
#[derive(Debug, Clone, Default, PartialEq)]
pub struct PencilDragState {
    pub active: bool,
    pub points_upm: Vec<FontPoint>,
}

/// Active cell painting/erasing state for `PixelTool` (`X` — `PixelTool`).
#[derive(Debug, Clone, Default, PartialEq)]
pub struct PixelDragState {
    pub active: bool,
    pub erase_mode: bool,
    pub last_cell: Option<(i64, i64)>,
}

/// Bundled state for all `grafia v0.4.0` tools (`SPEC-005`).
#[derive(Debug, Clone, PartialEq)]
pub struct V040ToolState {
    pub knife: KnifeDragState,
    pub primitive_sub_tool: PrimitiveSubTool,
    pub primitive_drag: PrimitiveDragState,
    pub last_dialog_request: Option<ToolDialogRequest>,
    pub measurement: MeasurementDragState,
    pub annotation_sub_tool: AnnotationSubTool,
    pub selected_annotations: Vec<AnnotationId>,
    pub clipboard_annotations: Vec<CanvasAnnotation>,
    pub annotation_drag: AnnotationDragMode,
    pub next_annotation_id: u64,
    pub show_annotations: bool,
    pub pencil: PencilDragState,
    pub pixel: PixelDragState,
    pub last_pixel_cell: Option<(i64, i64)>,
    pub pixel_size_upm: f64,
}

impl Default for V040ToolState {
    fn default() -> Self {
        Self {
            knife: KnifeDragState::default(),
            primitive_sub_tool: PrimitiveSubTool::Rectangle,
            primitive_drag: PrimitiveDragState::default(),
            last_dialog_request: None,
            measurement: MeasurementDragState::default(),
            annotation_sub_tool: AnnotationSubTool::Text,
            selected_annotations: Vec::new(),
            clipboard_annotations: Vec::new(),
            annotation_drag: AnnotationDragMode::Idle,
            next_annotation_id: 1,
            show_annotations: true,
            pencil: PencilDragState::default(),
            pixel: PixelDragState::default(),
            last_pixel_cell: None,
            pixel_size_upm: PIXEL_DEFAULT_STEP_UPM,
        }
    }
}

impl V040ToolState {
    pub fn reset_in_flight_drags(&mut self) {
        self.knife = KnifeDragState::default();
        self.primitive_drag = PrimitiveDragState::default();
        self.measurement = MeasurementDragState::default();
        self.annotation_drag = AnnotationDragMode::Idle;
        self.pencil = PencilDragState::default();
        self.pixel = PixelDragState::default();
    }
}

impl GrafiaCanvasState {
    /// Programmatically inserts a rectangle or circle primitive contour with Undo recording
    /// (`CanvasTool::makePrimitiveWithDialog:` confirmation at `spec_ref`).
    pub fn insert_primitive(
        &mut self,
        sub_tool: PrimitiveSubTool,
        origin_upm: FontPoint,
        width_upm: f64,
        height_upm: f64,
    ) -> bool {
        if width_upm < PRIMITIVE_MIN_SIZE_UPM || height_upm < PRIMITIVE_MIN_SIZE_UPM {
            return false;
        }
        let before = self.snapshot();
        let min_x = origin_upm.x.0;
        let min_y = origin_upm.y.0;
        let max_x = min_x + width_upm;
        let max_y = min_y + height_upm;
        let nodes = match sub_tool {
            PrimitiveSubTool::Rectangle => {
                build_rectangle_contour_nodes(min_x, min_y, max_x, max_y, &mut self.next_node_id)
            }
            PrimitiveSubTool::Circle => {
                build_circle_contour_nodes(min_x, min_y, max_x, max_y, &mut self.next_node_id)
            }
        };
        self.selected_nodes = nodes.iter().map(|n| n.id).collect();
        let c_idx = self.contours.len();
        self.contours.push(nodes.clone());
        self.glyph
            .contours
            .push(GlyphContour::new(ContourId(c_idx), nodes));
        self.active_contour_closed = true;
        self.sync_paths_from_contours();
        self.record_mutation("Add Primitive", before);
        self.assert_invariants();
        true
    }

    /// Computes Blue (`OnCurve`), Green (`OffCurve`), Red (`Segment Delta`), and Guide Angle labels
    /// (`CanvasTool::drawLocationsForLayer:origin:scale:shouldDrawDark:` at `spec_ref`).
    #[must_use]
    pub fn compute_measurement_layer_labels(&self) -> MeasurementLayerLabels {
        let closed_flags = self.closed_flags();
        compute_layer_measurement_labels(
            &self.contours,
            &closed_flags,
            self.italic_angle_deg,
            &self.guides,
            &self.global_guides,
            self.show_guides,
        )
    }

    /// Recomputes sorted intersections along the active measurement drag ray (`calculateIntersectionsForDrag` `spec_ref`).
    pub fn recompute_measurement_intersections(&mut self) {
        if !self.v040.measurement.active {
            self.v040.measurement.intersections.clear();
            return;
        }
        let closed_flags = self.closed_flags();
        let slant_origin_y = self.metrics.x_height * 0.5;
        self.v040.measurement.intersections = compute_measurement_ray_intersections(
            &self.contours,
            &closed_flags,
            self.glyph.width,
            self.italic_angle_deg,
            slant_origin_y,
            self.v040.measurement.start_upm,
            self.v040.measurement.current_upm,
        );
    }

    /// Promotes the active measurement drag line to a local `Guide` with `show_measurement = true`
    /// when `'G'` is pressed during a measurement drag (`CanvasTool::keyDown:` at `spec_ref`).
    pub fn promote_measurement_to_guide(&mut self) -> bool {
        if !self.v040.measurement.active {
            return false;
        }
        let start = self.v040.measurement.start_upm;
        let end = self.v040.measurement.current_upm;
        let dx = end.x.0 - start.x.0;
        let dy = end.y.0 - start.y.0;
        if dx.hypot(dy) <= KNIFE_MIN_DRAG_UPM {
            return false;
        }
        let before = self.snapshot();
        let angle_deg = dy.atan2(dx).to_degrees();
        let id = GuideId(self.next_guide_id);
        self.next_guide_id += 1;
        let mut guide = Guide::new(id, start, angle_deg, false);
        guide.show_measurement = true;
        self.guides.push(guide);
        self.show_guides = true;
        self.selected_guide = Some(SelectedGuideRef {
            id,
            is_global: false,
        });
        self.record_mutation("Promote Measurement to Guide", before);
        true
    }

    /// Executes Boolean `Remove Overlap` (`Cmd+Shift+O` / `GlyphLayer::removeOverlap` -> `BooleanPathEngine`).
    pub fn remove_overlap(&mut self) -> bool {
        let before = self.snapshot();
        let mut flags = self.closed_flags();
        if !remove_overlap_in_contours(
            &mut self.contours,
            &mut flags,
            &self.selected_nodes,
            &mut self.next_node_id,
        ) {
            return false;
        }
        let valid_ids: std::collections::HashSet<NodeId> = self
            .contours
            .iter()
            .flat_map(|c| c.iter().map(|n| n.id))
            .collect();
        self.selected_nodes.retain(|id| valid_ids.contains(id));
        self.sync_contours_and_closed_flags(&flags);
        self.record_mutation("Remove Overlap", before);
        self.assert_invariants();
        true
    }
}

/// Synchronizes temporary `Cmd + Ctrl + Opt` (`tempTrigger = spec_ref`) activation of `MeasurementTool`.
pub fn sync_temp_measurement_modifier_override(
    state: &mut GrafiaCanvasState,
    is_cmd: bool,
    is_ctrl: bool,
    is_alt: bool,
    is_space: bool,
) {
    if state.active_tool == CanvasTool::Text {
        state.temp_show_text_measurement_line = is_cmd && is_ctrl && is_alt && !is_space;
        return;
    }
    state.temp_show_text_measurement_line = false;
    if is_cmd && is_ctrl && is_alt && !is_space && !state.temp_zoom_via_key_space {
        if state.active_tool != CanvasTool::Measurement {
            if state.temp_tool_before_override.is_none() {
                state.temp_tool_before_override = Some(state.active_tool);
            }
            state.active_tool = CanvasTool::Measurement;
        }
    } else if !state.temp_zoom_via_key_space
        && !(is_space && (is_cmd || is_ctrl))
        && state.active_tool == CanvasTool::Measurement
        && state.temp_tool_before_override.is_some()
        && !state.v040.measurement.active
    {
        if let Some(prev) = state.temp_tool_before_override.take() {
            state.active_tool = prev;
        }
    }
}

/// Dispatches `SPEC-005` (`v0.4.0`) keyboard shortcuts (`E`, `F`/`Shift+F`, `L`, `A`/`Shift+A`/`Cmd+A`, `B`, `X`, `G`, `Backspace`/`Delete`, `Cmd+Shift+O`).
pub fn dispatch_v040_keyboard_shortcut(
    state: &mut GrafiaCanvasState,
    key: Key,
    shift_active: bool,
    alt_active: bool,
    cmd_active: bool,
) -> bool {
    if state.active_tool == CanvasTool::Annotation && cmd_active && !shift_active && !alt_active {
        match key {
            Key::A => {
                state.select_all_annotations();
                return true;
            }
            Key::C => return state.copy_selected_annotations(),
            Key::X => return state.cut_selected_annotations(),
            Key::V => return state.paste_clipboard_annotations(),
            _ => {}
        }
    }
    if !cmd_active && !alt_active {
        match key {
            Key::E => {
                state.activate_tool(CanvasTool::Knife);
                return true;
            }
            Key::F => {
                if state.active_tool == CanvasTool::Primitives || shift_active {
                    state.v040.primitive_sub_tool = state.v040.primitive_sub_tool.cycle_next();
                }
                state.activate_tool(CanvasTool::Primitives);
                return true;
            }
            Key::L => {
                state.activate_tool(CanvasTool::Measurement);
                return true;
            }
            Key::G if state.active_tool == CanvasTool::Measurement => {
                return state.promote_measurement_to_guide();
            }
            Key::A => {
                if shift_active {
                    state.v040.annotation_sub_tool = state.v040.annotation_sub_tool.cycle_next();
                }
                state.activate_tool(CanvasTool::Annotation);
                return true;
            }
            Key::B => {
                state.activate_tool(CanvasTool::Pencil);
                return true;
            }
            Key::X => {
                state.activate_tool(CanvasTool::Pixel);
                return true;
            }
            _ => {}
        }
    }
    match key {
        Key::Backspace | Key::Delete
            if state.active_tool == CanvasTool::Annotation
                && !state.v040.selected_annotations.is_empty() =>
        {
            state.delete_selected_annotations()
        }
        Key::O if cmd_active && shift_active && !alt_active => state.remove_overlap(),
        _ => false,
    }
}
