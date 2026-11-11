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
    AnnotationDragMode, CanvasTool, CanvasViewMode, GrafiaCanvasResponse, GrafiaCanvasState,
    GrafiaCanvasWidget, GuideDragState, KnifeDragState, MeasurementDragState, PencilDragState,
    PixelDragState, PrimitiveDragState, V020DragState, V040ToolState,
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
    project_point_to_background_contours, remove_overlap_in_contours,
    select_nodes_in_lasso_polygon, simplify_polyline_rdp, snap_to_pixel_cell,
    snap_vector_45_degrees, split_contour_segment, subtract_contours, tidy_up_contours, Affine2D,
    AnchorId, AnchorSelectionInfo, AnnotationId, AnnotationKind, AnnotationSubTool, BBoxAction,
    CanvasAnchor, CanvasAnnotation, CanvasClipboard, CanvasComponent, CanvasHostRequest,
    CanvasTextStream, ClipboardContour, CompanionMasterLayer, ComponentId, ComponentSelectionInfo,
    ContourId, ContourSegment, ContourStrokeStyle, DuplicateWarningKind, FitCurveConfig, FontPoint,
    FontUnit, GhostMarkPath, Glyph, GlyphBaseInfo, GlyphContour, GlyphHierarchy, GlyphSlot,
    GuideAlignment, GuideInfo, KerningDelta, KerningIndicatorQuad, LineCapStyle, LineJoinStyle,
    MasterCompatibilityReport, MeasurementLayerLabels, MetricsOrigin, MultiNodeInfo, Node, NodeId,
    NodeType, PrimitiveSubTool, ScreenDp, ScreenPoint, SegmentHit, SelectionBounds, SelectionInfo,
    SelectionInfoAction, Shape, ShapeBooleanRole, ShapeId, SingleOffCurveInfo, SingleOnCurveInfo,
    SlotScanlineMeasurement, SpacingDelta, StrokePosition, TextStreamEditKind,
    TextStreamEditRequest, ToolDialogRequest, TransformGridAnchor, TransformOriginSpec,
    ViewportTransform,
};
pub use interaction::{
    CanvasGesture, CommandHistory, GlyphSnapshot, Modifiers, PenToolState, SelectToolState,
};
pub use locator::{ShapeLocatorRenderer, ShapeLocatorStyle, ShapeLocatorWidget};
pub use render::{
    compute_ruler_tick_steps_upm, render_preview_strip_rgba, tessellate_contours_even_odd,
    tessellate_contours_nonzero, tessellate_glyph_contours_nonzero, GrafiaPreviewWidget,
    PreviewFilledProjection, PreviewPanelConfig, PreviewRenderConfig, PreviewStripCache,
    ProjectedNode, ShapeThumbnailProjection,
};
pub use snapping::{
    calculate_guide_intersections, classify_node_alignment, normalize_guide_angle,
    smart_two_node_guide_geometry, ActiveSnapper, AlignmentBadgeMode, AlignmentZone, GlyphMetrics,
    Guide, GuideHitKind, GuideId, GuideMeasurementRay, GuideMeasurementSegment, SelectedGuideRef,
    SnapEngine, SnapResult,
};
pub use tokens::{
    AppearanceMode, GrafiaTheme, RgbaColorToken, ANCHOR_DIAMOND_RADIUS_DP,
    ANCHOR_DIAMOND_SELECTED_RADIUS_DP, ANCHOR_HIT_RADIUS_DP, BACKGROUND_LAYER_STROKE_DARK,
    BACKGROUND_LAYER_STROKE_LIGHT, BACKGROUND_STROKE_WIDTH_DP, FIT_CURVE_DEFAULT_MAX_RATIO,
    FIT_CURVE_DEFAULT_MIN_RATIO, FIT_CURVE_MAX_INTERSECTION_CHORD_MULTIPLE,
    KERNING_INCREMENT_HIGH_UPM, KERNING_INCREMENT_LOW_UPM, KERNING_NEGATIVE_FILL_DARK,
    KERNING_NEGATIVE_FILL_LIGHT, KERNING_POSITIVE_FILL_DARK, KERNING_POSITIVE_FILL_LIGHT,
    PASSIVE_GLYPH_FILL_DARK, PASSIVE_GLYPH_FILL_LIGHT, PREVIEW_BG_INVERTED_DARK,
    PREVIEW_BG_INVERTED_LIGHT, PREVIEW_BG_NORMAL_DARK, PREVIEW_BG_NORMAL_LIGHT,
    PREVIEW_BLUR_ACTIVE_THRESHOLD_PX, PREVIEW_BLUR_DEFAULT_RADIUS_PX, PREVIEW_BLUR_MAX_RADIUS_PX,
    PREVIEW_BLUR_MIN_RADIUS_PX, PREVIEW_BOTH_EXTRA_SPACE_SCALE, PREVIEW_DEFAULT_EXTRA_SPACING,
    PREVIEW_FG_INVERTED_DARK, PREVIEW_FG_INVERTED_LIGHT, PREVIEW_FG_NORMAL_DARK,
    PREVIEW_FG_NORMAL_LIGHT, PREVIEW_ONE_EXTRA_SPACE_SCALE, RULER_BG_FILL_DARK,
    RULER_BG_FILL_LIGHT, RULER_BORDER_STROKE_DARK, RULER_BORDER_STROKE_LIGHT,
    RULER_HAIRLINE_COLOR_DARK, RULER_HAIRLINE_COLOR_LIGHT, RULER_THICKNESS_DP,
    RULER_TICK_COLOR_DARK, RULER_TICK_COLOR_LIGHT, SPACING_INCREMENT_HIGH_UPM,
    SPACING_INCREMENT_LOW_UPM, TEXT_CARET_COLOR_DARK, TEXT_CARET_COLOR_LIGHT,
    TEXT_MEASUREMENT_KNOB_FILL_DARK, TEXT_MEASUREMENT_KNOB_FILL_LIGHT,
    TEXT_MEASUREMENT_LINE_COLOR_DARK, TEXT_MEASUREMENT_LINE_COLOR_LIGHT,
    TEXT_MEASUREMENT_LINE_DEFAULT_Y_UPM, TEXT_METRICS_NUMBER_COLOR_DARK,
    TEXT_METRICS_NUMBER_COLOR_LIGHT, TEXT_MODE_NUMBERS_THRESHOLD_DEFAULT,
    TEXT_SELECTION_DRAG_THRESHOLD_DP, TEXT_SELECTION_FILL_DARK, TEXT_SELECTION_FILL_LIGHT,
};
