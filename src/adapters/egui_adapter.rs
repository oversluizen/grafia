//! `GrafiaCanvasWidget` implementation and gesture routing for egui.
//!
//! Enforces zero magic numbers and zero hardcoded colors.

use crate::adapters::input_dispatcher::InputDispatcher;
use crate::adapters::painter_pass::CanvasPainter;
use crate::adapters::state::{CanvasTool, CanvasViewMode, GrafiaCanvasResponse, GrafiaCanvasState};
use crate::interaction::SelectToolState;
use crate::tokens::{GrafiaTheme, DEFAULT_CANVAS_HEIGHT_DP, DEFAULT_CANVAS_WIDTH_DP};
use egui::{CursorIcon, Sense, Vec2};

pub struct GrafiaCanvasWidget<'a> {
    pub state: &'a mut GrafiaCanvasState,
    pub theme: Option<&'a GrafiaTheme>,
    pub desired_size: Option<Vec2>,
}

pub type GlifoCanvasWidget<'a> = GrafiaCanvasWidget<'a>;

impl<'a> GrafiaCanvasWidget<'a> {
    #[must_use]
    pub fn new(state: &'a mut GrafiaCanvasState, theme: &'a GrafiaTheme) -> Self {
        Self {
            state,
            theme: Some(theme),
            desired_size: None,
        }
    }

    /// Creates a `GrafiaCanvasWidget` driven directly by `state.theme` and `state.appearance_mode`
    /// (`Light`, `Dark`, `System`) without requiring an external `GrafiaTheme` borrow.
    #[must_use]
    pub fn from_state(state: &'a mut GrafiaCanvasState) -> Self {
        Self {
            state,
            theme: None,
            desired_size: None,
        }
    }

    #[must_use]
    pub fn with_size(mut self, size: Vec2) -> Self {
        self.desired_size = Some(size);
        self
    }

    pub fn show(self, ui: &mut egui::Ui) -> GrafiaCanvasResponse {
        self.state.set_system_dark_mode(ui.visuals().dark_mode);
        let size = self.desired_size.unwrap_or_else(|| {
            Vec2::new(DEFAULT_CANVAS_WIDTH_DP, DEFAULT_CANVAS_HEIGHT_DP)
        });
        let (rect, response) = ui.allocate_exact_size(size, Sense::click_and_drag());

        // Resolve authoritative theme (honoring `state.theme` when `AppearanceMode` differs from caller's `theme`)
        let resolved_theme = match self.theme {
            Some(ext) if ext.is_dark() == self.state.theme.is_dark() => ext.clone(),
            _ => self.state.theme.clone(),
        };

        // Process pointer and keyboard interactions through gated input dispatcher
        let events = ui.input(|i| i.events.clone());
        let mut dispatcher = InputDispatcher::new(self.state, &resolved_theme, rect);
        let changed = dispatcher.dispatch_events(&events, ui);

        if response.clicked() || response.drag_started() {
            response.request_focus();
        }
        let latest_pointer = ui
            .input(|i| i.pointer.latest_pos())
            .map(|p| crate::core::ScreenPoint::new(p.x, p.y));
        if let Some(pos) = latest_pointer.filter(|p| rect.contains(egui::pos2(p.x.0, p.y.0))) {
            self.state.cursor_screen_pos = Some(pos);
            let space_held = ui.input(|i| i.key_down(egui::Key::Space));
            let icon = if matches!(self.state.select_state, SelectToolState::PanningViewport { .. }) {
                CursorIcon::Grabbing
            } else if self.state.view_mode == CanvasViewMode::PreviewFilled || space_held {
                CursorIcon::Grab
            } else {
                match self.state.active_tool {
                    CanvasTool::Pen => {
                        if self.state.hovered_pen_close_node.is_some()
                            || self.state.hovered_pen_close_target.is_some()
                        {
                            CursorIcon::PointingHand
                        } else if self.state.hit_test_pen_node(pos).is_some() {
                            CursorIcon::Move
                        } else {
                            CursorIcon::Crosshair
                        }
                    }
                    CanvasTool::Select | CanvasTool::Lasso => {
                        if matches!(
                            self.state.select_state,
                            SelectToolState::DraggingNodes { .. }
                                | SelectToolState::DraggingSegment { .. }
                        ) || self.state.guide_drag != crate::adapters::v030_guides::GuideDragState::Idle
                        {
                            CursorIcon::Grabbing
                        } else if self.state.hit_test_node(pos).is_some()
                            || self.state.hit_test_guides(pos).is_some()
                        {
                            CursorIcon::PointingHand
                        } else if self.state.active_tool == CanvasTool::Lasso {
                            CursorIcon::Crosshair
                        } else {
                            CursorIcon::Default
                        }
                    }
                    CanvasTool::Rotate | CanvasTool::Scale => CursorIcon::Crosshair,
                    CanvasTool::Zoom => {
                        if ui.input(|i| i.modifiers.alt) {
                            CursorIcon::ZoomOut
                        } else {
                            CursorIcon::ZoomIn
                        }
                    }
                    CanvasTool::Knife
                    | CanvasTool::Primitives
                    | CanvasTool::Measurement
                    | CanvasTool::Pencil
                    | CanvasTool::Pixel => CursorIcon::Crosshair,
                    CanvasTool::Annotation => CursorIcon::Default,
                }
            };
            ui.ctx().set_cursor_icon(icon);
        } else {
            self.state.cursor_screen_pos = None;
        }

        // Render multi-pass canvas
        let active_theme = match self.theme {
            Some(ext) if ext.is_dark() == self.state.theme.is_dark() => ext,
            _ => &self.state.theme,
        };
        let painter = ui.painter_at(rect);
        let canvas_painter = CanvasPainter::new(&painter, rect, self.state, active_theme);
        canvas_painter.paint();

        GrafiaCanvasResponse {
            response,
            active_tool: self.state.active_tool,
            view_mode: self.state.view_mode,
            changed,
            selected_nodes: self.state.selected_nodes.clone(),
        }
    }
}

impl egui::Widget for GrafiaCanvasWidget<'_> {
    fn ui(self, ui: &mut egui::Ui) -> egui::Response {
        self.show(ui).response
    }
}

impl GrafiaCanvasState {
    fn apply_panel_affine(&mut self, affine: kurbo::Affine, label: &'static str) -> bool {
        if self.contours.is_empty() {
            return false;
        }
        let before = self.snapshot();
        let shadow = self.contours.clone();
        let flags = self.closed_flags();
        let target_ids: Vec<crate::core::NodeId> = if !self.selected_nodes.is_empty() {
            self.selected_nodes.clone()
        } else {
            self.contours.iter().flat_map(|c| c.iter().map(|n| n.id)).collect()
        };
        let grid = self.effective_grid_step();
        crate::core::apply_mode_nodes_transform(
            &mut self.contours,
            &shadow,
            &flags,
            &target_ids,
            affine,
            false,
            grid,
        );
        self.sync_paths_from_contours();
        self.record_mutation(label, before);
        self.assert_invariants();
        true
    }

    pub fn flip_horizontal(&mut self) -> bool {
        let p = self.resolve_transform_origin();
        let aff = kurbo::Affine::translate((p.x.0, p.y.0))
            * kurbo::Affine::scale_non_uniform(-1.0, 1.0)
            * kurbo::Affine::translate((-p.x.0, -p.y.0));
        self.apply_panel_affine(aff, "Flip Horizontal")
    }

    pub fn flip_vertical(&mut self) -> bool {
        let p = self.resolve_transform_origin();
        let aff = kurbo::Affine::translate((p.x.0, p.y.0))
            * kurbo::Affine::scale_non_uniform(1.0, -1.0)
            * kurbo::Affine::translate((-p.x.0, -p.y.0));
        self.apply_panel_affine(aff, "Flip Vertical")
    }

    pub fn scale_by_factor(&mut self, sx: f64, sy: f64) -> bool {
        if sx.abs() <= crate::tokens::SCALE_MIN_FACTOR || sy.abs() <= crate::tokens::SCALE_MIN_FACTOR {
            return false;
        }
        let p = self.resolve_transform_origin();
        let aff = kurbo::Affine::translate((p.x.0, p.y.0))
            * kurbo::Affine::scale_non_uniform(sx, sy)
            * kurbo::Affine::translate((-p.x.0, -p.y.0));
        self.apply_panel_affine(aff, "Scale Selection")
    }

    pub fn rotate_by_degrees(&mut self, angle_deg: f64) -> bool {
        let clamped = angle_deg.clamp(-crate::tokens::FULL_CIRCLE_DEG_F64, crate::tokens::FULL_CIRCLE_DEG_F64);
        let p = self.resolve_transform_origin();
        let aff = kurbo::Affine::rotate_about(
            clamped * crate::tokens::DEG_TO_RAD_F64,
            kurbo::Point::new(p.x.0, p.y.0),
        );
        self.apply_panel_affine(aff, "Rotate Selection")
    }

    pub fn slant_by_degrees(&mut self, angle_deg: f64, horizontal: bool) -> bool {
        if angle_deg.abs() > crate::tokens::SLANT_MAX_ANGLE_DEG {
            return false;
        }
        let p = self.resolve_transform_origin();
        let sh = (angle_deg * crate::tokens::DEG_TO_RAD_F64).tan();
        let shear = if horizontal {
            kurbo::Affine::new([1.0, 0.0, sh, 1.0, 0.0, 0.0])
        } else {
            kurbo::Affine::new([1.0, sh, 0.0, 1.0, 0.0, 0.0])
        };
        let aff = kurbo::Affine::translate((p.x.0, p.y.0))
            * shear
            * kurbo::Affine::translate((-p.x.0, -p.y.0));
        self.apply_panel_affine(aff, "Slant Selection")
    }

    #[must_use]
    pub fn duplicate_warning_nodes(
        &self,
    ) -> Vec<(crate::core::NodeId, crate::core::FontPoint, crate::core::DuplicateWarningKind)> {
        let dragging = !matches!(self.select_state, SelectToolState::Idle)
            || !matches!(self.v020_drag, crate::adapters::v020_tools::V020DragState::Idle);
        crate::core::find_duplicate_warning_nodes(&self.contours, &self.closed_flags(), dragging)
    }
}
