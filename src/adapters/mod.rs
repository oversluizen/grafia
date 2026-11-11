//! GUI framework integration for egui and eframe.
//!
//! Exposes `GrafiaCanvasWidget` and `GrafiaCanvasState` connecting typography math,
//! interaction tool state machines, and multi-pass painting on `egui::Painter`.

pub mod contour_join;
pub mod cursor_overlay;
pub mod cursor_overlay_ts;
pub mod egui_adapter;
pub mod headless;
pub mod headless_passes;
pub mod input_dispatcher;
pub mod input_pointer_drag;
pub mod invariants;
pub mod keyboard;
pub mod keyboard_edit;
pub mod painter_nodes_pass;
pub mod painter_pass;
pub mod pen_interaction;
pub mod select_drag_ops;
pub mod select_interaction;
pub mod snapshot_ops;
pub mod state;
pub mod state_sync;
pub mod v020_drag_ops;
pub mod v020_interaction;
pub mod v020_tools;
pub mod v030_guide_drag;
pub mod v030_guides;
pub mod v040_annotation_pixel;
pub mod v040_interaction;
pub mod v040_tools;
pub mod v050_interaction;
pub mod v050_keys;
pub mod v060_layers;
pub mod v060_multi_ops;
pub mod v060_ops;
pub mod v060_vc;
pub mod v070_layers_vc;
pub mod v070_ops;

pub use egui_adapter::GrafiaCanvasWidget;
pub use headless::{pixmap_to_rgba_image, rasterize_canvas_subregion};
pub use input_dispatcher::InputDispatcher;
pub use painter_pass::CanvasPainter;
pub use state::{CanvasTool, CanvasViewMode, GrafiaCanvasResponse, GrafiaCanvasState};
pub use v020_tools::V020DragState;
pub use v030_guides::GuideDragState;
pub use v040_tools::{
    AnnotationDragMode, KnifeDragState, MeasurementDragState, PencilDragState, PixelDragState,
    PrimitiveDragState, V040ToolState,
};
