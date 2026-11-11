//! GUI framework integration for egui and eframe.
//!
//! Exposes `GrafiaCanvasWidget` and `GrafiaCanvasState` connecting typography math,
//! interaction tool state machines, and multi-pass painting on `egui::Painter`.

pub mod contour_join;
pub mod cursor_overlay;
pub mod egui_adapter;
pub mod headless;
pub mod input_dispatcher;
pub mod invariants;
pub mod keyboard;
pub mod painter_pass;
pub mod pen_interaction;
pub mod select_interaction;
pub mod snapshot_ops;
pub mod state;
pub mod v020_interaction;
pub mod v020_tools;
pub mod v030_guides;
pub mod v040_interaction;
pub mod v040_tools;

pub use egui_adapter::{GlifoCanvasWidget, GrafiaCanvasWidget};
pub use headless::{pixmap_to_rgba_image, rasterize_canvas_subregion};
pub use input_dispatcher::InputDispatcher;
pub use painter_pass::CanvasPainter;
pub use state::{
    CanvasTool, CanvasViewMode, GlifoCanvasResponse, GlifoCanvasState, GrafiaCanvasResponse,
    GrafiaCanvasState,
};
pub use v020_tools::V020DragState;
pub use v030_guides::GuideDragState;
pub use v040_tools::{
    AnnotationDragMode, KnifeDragState, MeasurementDragState, PencilDragState, PixelDragState,
    PrimitiveDragState, V040ToolState,
};
