//! Core mathematical, coordinate, and bezier types for grafia.

pub mod align_and_connect;
pub mod anchors_components;
pub mod annotations;
pub mod bezier_ops;
pub mod boolean_geom;
pub mod boolean_ops;
pub mod clipboard_ops;
pub mod coordinates;
pub mod curve_manipulation;
pub mod fit_and_stroke;
pub mod glyph_model;
pub mod handle_constraints;
pub mod inflection_ops;
pub mod knife_ops;
pub mod lasso_and_zoom;
pub mod master_compatibility;
pub mod measurement_ops;
pub mod nudge_ops;
pub mod path_ops;
pub mod primitives_ops;
pub mod segment_ops;
pub mod segment_slide;
pub mod selection_info;
pub mod selection_info_ops;
pub mod stroke_expand;
pub mod text_stream_ops;
pub mod tidy_align_connect;
pub mod transform_ops;

pub use annotations::{
    hit_test_annotation_body, hit_test_annotation_handle, AnnotationId, AnnotationKind,
    AnnotationSubTool, CanvasAnnotation,
};

pub use crate::render::compute_ruler_tick_steps_upm;
pub use anchors_components::{
    compute_mark_cloud_contours, hit_test_component_upm, Affine2D, AnchorSelectionInfo,
    CanvasComponent, CanvasHostRequest, ComponentId, ComponentSelectionInfo, GhostMarkPath,
    TextStreamEditKind, TextStreamEditRequest,
};
pub use bezier_ops::{
    align_collinear_opposite, compute_signed_area, snap_vector_45_degrees, Node, NodeId, NodeType,
};
pub use boolean_ops::{
    contour_signed_area, intersect_contours, remove_overlap_in_contours, subtract_contours,
};
pub use clipboard_ops::{
    extract_clipboard_contours, paste_clipboard_into_contours, CanvasClipboard, ClipboardContour,
};
pub use coordinates::{FontPoint, FontUnit, ScreenDp, ScreenPoint, ViewportTransform};
pub use curve_manipulation::{
    cut_open_delete_nodes, cut_open_delete_nodes_in_contour, extract_segments, heal_delete_nodes,
    hit_test_segments, split_contour_segment, ContourSegment, SegmentHit,
};
pub use fit_and_stroke::{
    apply_fit_curve_delta_to_contours, apply_fit_curve_ratio_to_contours,
    expand_single_contour_stroke, expand_stroked_contours_preview,
    normalize_cubic_handles_in_contours, solve_cubic_tangent_triangle, ContourStrokeStyle,
    FitCurveConfig, LineCapStyle, LineJoinStyle, StrokePosition,
};
pub use glyph_model::{
    ContourId, Glyph, GlyphContour, GlyphHierarchy, Shape, ShapeBooleanRole, ShapeId,
};
pub use handle_constraints::{
    convert_line_segment_to_cubic, convert_line_segment_to_cubic_with_grid, cycle_selection_tab,
    enforce_smooth_collinearity_on_handle_drag, expanded_nodes_with_attached_handles,
    fit_cubic_to_segment_chain, toggle_selected_on_curve_smooth,
};
pub use knife_ops::{
    compute_knife_intersections, constrain_knife_endpoint, contour_intersections_with_segment,
    intersect_finite_segments, knife_add_nodes_in_layer, knife_cut_paths_in_layer,
};
pub use lasso_and_zoom::{
    compute_marquee_zoom, next_zoom_step, point_in_polygon, prev_zoom_step,
    select_nodes_in_lasso_polygon, select_nodes_in_slanted_marquee,
    slanted_marquee_parallelogram_screen,
};
pub use master_compatibility::{
    compute_master_compatibility_report, select_nodes_in_rect_across_layers, CompanionMasterLayer,
    MasterCompatibilityReport,
};
pub use measurement_ops::{
    compute_layer_measurement_labels, compute_measurement_ray_intersections,
    compute_measurement_typographic_angle_deg, format_measurement_value, MeasurementLayerLabels,
};
pub use nudge_ops::{
    apply_ctrl_alt_interpolate_nudge, apply_ctrl_alt_mirrored_handle_drag,
    project_point_onto_infinite_line, slide_smooth_line_node_unlimited,
    slide_smooth_node_on_cubic_path_shift_middle,
};
pub use path_ops::{
    add_extremes_to_contours, add_inflection_points_to_contours, correct_path_direction_layer,
    make_node_first_v020,
};
pub use primitives_ops::{
    bresenham_pixel_cells, build_circle_contour_nodes, build_rectangle_contour_nodes,
    compute_primitive_rect_upm, find_pixel_cell_contour_index, fit_freehand_contour_nodes,
    pixel_cell_bounds_upm, simplify_polyline_rdp, snap_to_pixel_cell, PrimitiveSubTool,
    ToolDialogRequest,
};
pub use segment_ops::{
    apply_cubic_segment_belly_drag, apply_cubic_segment_belly_drag_with_alt,
    apply_proportional_tension_handle_drag, find_nearest_cubic_extremum,
    make_selected_on_curve_node_first, remove_off_curve_segment_handles, reverse_contour_nodes,
    slide_on_curve_node_along_contour,
};
pub use selection_info::{
    align_oncurve_nodes_to_background, collect_adjacent_offcurve_positions,
    compute_radially_scaled_handle_pos, enforce_smooth_collinearity_around_oncurve,
    find_first_offcurve_node_id, find_first_oncurve_with_adjacent_offcurve,
    project_point_to_background_contours, resolve_tethered_oncurve_for_offcurve,
    translate_oncurve_with_adjacent_handles, update_offcurve_with_smooth_constraint, AnchorId,
    CanvasAnchor, GlyphBaseInfo, GuideAlignment, GuideInfo, MultiNodeInfo, SelectionInfo,
    SelectionInfoAction, SingleOffCurveInfo, SingleOnCurveInfo,
};
pub use text_stream_ops::{
    compute_glyph_lsb_and_rsb, intersect_contours_at_horizontal_y, CanvasTextStream, GlyphSlot,
    KerningDelta, KerningIndicatorQuad, SlotScanlineMeasurement, SpacingDelta,
};
pub use tidy_align_connect::{
    align_selected_nodes, close_open_contours_in_layer,
    connect_selected_open_endpoints_in_contours, find_duplicate_warning_nodes, tidy_up_contours,
    DuplicateWarningKind,
};
pub use transform_ops::{
    apply_mode_nodes_transform, compute_bbox_rotate_or_slant_affine, compute_bbox_scale_affine,
    compute_layer_bounds, compute_rotate_tool_affine, compute_scale_tool_affine,
    compute_selection_bounds, hit_test_bbox_knobs, slant_point, unslant_point, BBoxAction,
    MetricsOrigin, SelectionBounds, TransformGridAnchor, TransformOriginSpec,
};
