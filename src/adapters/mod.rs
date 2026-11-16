#![doc = include_str!("../../docs/guide/adapters_and_host_port.md")]

pub mod anchor_and_stroke_ops;
pub mod annotation_and_pixel_ops;
pub mod background_and_ruler_ops;
pub mod bbox_drag_ops;
pub mod bbox_selection_tools;
pub mod color_layer_interaction;
pub mod component_and_layer_controller;
pub mod contour_edit_interaction;
pub mod contour_join;
pub mod controller_ports_ops;
pub mod controller_rotary_ops;
pub mod cursor_overlay;
pub mod cursor_pixmap_pass;
pub mod curve_harmony_ops;
pub mod document_scene_ops;
pub mod drawing_tool_interaction;
pub mod drawing_tool_state;
pub mod egui_adapter;
pub mod guide_drag_ops;
pub mod guide_management_ops;
pub mod headless;
pub mod headless_passes;
pub mod hinting_and_hoi_interaction;
pub mod input_dispatcher;
pub mod input_pointer_drag;
pub mod invariants;
pub mod keyboard;
pub mod keyboard_edit;
pub mod keymap_and_touch_ops;
pub mod multi_selection_ops;
pub mod painter_nodes_pass;
pub mod painter_pass;
pub mod pen_interaction;
pub mod preferences_ops;
pub mod select_drag_ops;
pub mod select_interaction;
pub mod selection_info_controller;
pub mod selection_mutation_ops;
pub mod smart_corners_and_stem_ops;
pub mod smart_handle_interaction;
pub mod snapshot_ops;
pub mod spline_and_trace_ops;
pub mod state;
pub mod state_sync;
pub mod stem_brush_and_rhythm_ops;
pub mod text_stream_interaction;
pub mod text_stream_keys;
pub mod vertical_stream_ops;

pub use bbox_selection_tools::BBoxDragState;
pub use color_layer_interaction::GradientDragState;
pub use drawing_tool_state::{
    AnnotationDragMode, DrawingToolState, KnifeDragState, MeasurementDragState, PencilDragState,
    PixelDragState, PrimitiveDragState,
};
pub use egui_adapter::GrafiaCanvasWidget;
pub use guide_management_ops::GuideDragState;
pub use headless::{pixmap_to_rgba_image, rasterize_canvas_subregion};
pub use input_dispatcher::InputDispatcher;
pub use painter_pass::CanvasPainter;
pub use state::{
    CanvasTool, CanvasViewMode, DragSessionTransients, GrafiaCanvasResponse, GrafiaCanvasState,
    PenInteractionTransients, TextInteractionTransients,
};
