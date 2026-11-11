//! `grafia` — Interactive 2D Cubic Bezier Contour, Glyph & Vector Canvas Engine.
//!
//! A high-performance, modular, multiplatform Rust canvas runtime for font editors
//! and vector design applications, wrapping `egui`, `kurbo`, `lyon_tessellation`, and `tiny-skia`.

pub mod adapters;
pub mod core;
pub mod interaction;
pub mod locator;
pub mod render;
pub mod snapping;
pub mod tokens;

pub use adapters::{
    AnnotationDragMode, CanvasTool, CanvasViewMode, GlifoCanvasResponse, GlifoCanvasState,
    GlifoCanvasWidget, GrafiaCanvasResponse, GrafiaCanvasState, GrafiaCanvasWidget, GuideDragState,
    KnifeDragState, MeasurementDragState, PencilDragState, PixelDragState, PrimitiveDragState,
    V020DragState, V040ToolState,
};
pub use core::{
    add_extremes_to_contours, add_inflection_points_to_contours, align_collinear_opposite,
    align_selected_nodes, bresenham_pixel_cells, build_circle_contour_nodes,
    build_rectangle_contour_nodes, close_open_contours_in_layer, compute_knife_intersections,
    compute_layer_measurement_labels, compute_measurement_ray_intersections,
    compute_measurement_typographic_angle_deg, compute_primitive_rect_upm,
    compute_selection_bounds, compute_signed_area, connect_selected_open_endpoints_in_contours,
    constrain_knife_endpoint, contour_signed_area, correct_path_direction_layer,
    cut_open_delete_nodes, extract_clipboard_contours, extract_segments,
    find_duplicate_warning_nodes, find_pixel_cell_contour_index, fit_freehand_contour_nodes,
    format_measurement_value, heal_delete_nodes, hit_test_annotation_body,
    hit_test_annotation_handle, hit_test_bbox_knobs, hit_test_segments, intersect_contours,
    knife_add_nodes_in_layer, knife_cut_paths_in_layer, make_node_first_v020, next_zoom_step,
    paste_clipboard_into_contours, pixel_cell_bounds_upm, point_in_polygon, prev_zoom_step,
    remove_overlap_in_contours, select_nodes_in_lasso_polygon, simplify_polyline_rdp,
    snap_to_pixel_cell, snap_vector_45_degrees, split_contour_segment, subtract_contours,
    tidy_up_contours, AnnotationId, AnnotationKind,
    AnnotationSubTool, BBoxAction, CanvasAnnotation, CanvasClipboard, ClipboardContour, ContourId,
    ContourSegment, DuplicateWarningKind, FontPoint, FontUnit, Glyph, GlyphContour, GlyphHierarchy,
    MeasurementLayerLabels, MetricsOrigin, Node, NodeId, NodeType, PrimitiveSubTool, ScreenDp,
    ScreenPoint, SegmentHit, SelectionBounds, Shape, ShapeBooleanRole, ShapeId, ToolDialogRequest,
    TransformGridAnchor, TransformOriginSpec, ViewportTransform,
};
pub use interaction::{
    CanvasGesture, CommandHistory, GlyphSnapshot, Modifiers, PenToolState, SelectToolState,
};
pub use locator::{ShapeLocatorRenderer, ShapeLocatorStyle, ShapeLocatorWidget};
pub use render::{
    tessellate_contours_even_odd, tessellate_contours_nonzero, GlifoPreviewWidget,
    GrafiaPreviewWidget, PreviewFilledProjection, PreviewPanelConfig, ProjectedNode,
    ShapeThumbnailProjection,
};
pub use snapping::{
    calculate_guide_intersections, classify_node_alignment, normalize_guide_angle,
    smart_two_node_guide_geometry, ActiveSnapper, AlignmentBadgeMode, AlignmentZone, GlyphMetrics,
    Guide, GuideHitKind, GuideId, GuideMeasurementRay, GuideMeasurementSegment, SelectedGuideRef,
    SnapEngine, SnapResult,
};
pub use tokens::{AppearanceMode, GlifoTheme, GrafiaTheme, RgbaColorToken};
