#![doc = include_str!("../../docs/guide/core_math_and_topology.md")]

pub mod align_and_connect;
pub mod anchors_components;
pub mod annotations;
pub mod auto_trace;
pub mod auto_trace_bezier;
pub mod auto_trace_fit;
pub mod auto_trace_homography;
pub mod bbox_rotate_shear_ops;
pub mod bezier_ops;
pub mod boolean_geom;
pub mod boolean_ops;
pub mod canvas_entities;
pub mod clipboard_ops;
pub mod color_layers;
pub mod contour_order_ops;
pub mod controller_ports;
pub mod coordinates;
pub mod curvature_heatmap;
pub mod curve_conversions;
pub mod curve_harmony;
pub mod curve_manipulation;
pub mod curved_trajectory;
pub mod diagonal_and_ratio_hud;
pub mod document_scene;
pub mod euler_spiro;
pub mod extra_nodes;
pub mod fit_and_stroke;
pub mod glyph_model;
pub mod handle_constraints;
pub mod hinting_and_hoi;
pub mod hobby_spline;
pub mod host_requests;
pub mod inflection_ops;
pub mod knife_ops;
pub mod lasso_and_zoom;
pub mod local_brushes;
pub mod master_compatibility;
pub mod master_matching;
pub mod math_consts;
pub mod measurement_ops;
pub mod nudge_ops;
pub mod optical_rhythm;
pub mod outline_audit;
pub mod path_ops;
pub mod primitives_ops;
pub mod reference_frame;
pub mod ring_slice_ops;
pub mod segment_degree_ops;
pub mod segment_ops;
pub mod segment_slide;
pub mod selection_info;
pub mod selection_info_ops;
pub mod servo_and_nudge;
pub mod smart_corners_brush;
pub mod smart_handles;
pub mod spline_support;
pub mod stem_gauge_and_balance;
pub mod stem_tool;
pub mod stroke_expand;
pub mod text_stream_ops;
pub mod tidy_align_connect;
pub mod transform_ops;
pub mod tunni_and_comb;
pub mod vertical_stream;

pub use crate::render::compute_ruler_tick_steps_upm;
pub use align_and_connect::{
    align_selected_nodes, close_open_contours_in_layer, connect_selected_open_endpoints_in_contours,
};
pub use anchors_components::{
    compute_mark_cloud_contours, hit_test_component_upm, Affine2D, AnchorSelectionInfo,
    CanvasComponent, ComponentSelectionInfo, GhostMarkPath,
};
pub use annotations::{
    hit_test_annotation_body, hit_test_annotation_handle, AnnotationId, AnnotationKind,
    AnnotationSubTool, CanvasAnnotation,
};
pub use auto_trace::trace_background_image;
pub use bbox_rotate_shear_ops::{
    apply_rigid_or_affine_coupled_transform, compute_bbox_rotation_deg_and_affine,
    compute_bbox_shear_affine, hit_test_bbox_outer_corner_rotation,
};
pub use bezier_ops::{
    align_collinear_opposite, compute_signed_area, snap_vector_45_degrees, Node, NodeId, NodeType,
};
pub use boolean_ops::{
    contour_signed_area, intersect_contours, remove_overlap_in_contours, subtract_contours,
};
pub use canvas_entities::{
    BackgroundImageInfo, BackgroundImageMutation, CanvasBackgroundImage, CanvasSmartHandle,
    ExtraNodeKey, HobbyDirConstraint, HobbyNodeInfo, LayerPathOp, VirtualExtraNode,
};
pub use clipboard_ops::{
    extract_clipboard_contours, paste_clipboard_into_contours, CanvasClipboard, ClipboardContour,
};
pub use color_layers::{
    apply_gradient_handle_drag, apply_gradient_spread, evaluate_color_layer_paint_at_upm,
    gradient_control_segment_upm, gradient_stop_position_upm, gradient_stops_slice,
    hit_test_gradient_handle, porter_duff_src_over, sample_gradient_stops, CanvasColorSublayer,
    ColorLayerPaint, ColorSublayerMutation, GradientExtend, GradientHandleKind, GradientSpread,
    GradientStop,
};
pub use contour_order_ops::{
    correct_path_direction_layer, flatten_contour_points, make_node_first_or_reverse_open,
};
pub use controller_ports::{
    CanvasHapticEvent, CanvasIntent, CanvasIntentMask, CanvasRotaryChannel, ControllerPortState,
    ControllerTelemetryDescriptor, InputDeviceKind, IntentTelemetryItem, PrecisionLoupeGeometry,
    RotaryTelemetryInfo, StylusAccessoryEvent,
};
pub use coordinates::{FontPoint, FontUnit, ScreenDp, ScreenPoint, ViewportTransform};
pub use curvature_heatmap::{
    compute_contours_curvature_comb_spines, compute_contours_curvature_heatmap_envelopes,
    compute_contours_inflection_markers, find_cubic_inflection_points, CurvatureCombSpine,
    CurvatureCombStyle, CurvatureHeatmapEnvelope, CurvatureHeatmapSample, InflectionMarker,
};
pub use curve_conversions::{
    convert_contour_to_cubic, convert_contour_to_quadratic, convert_cubic_to_hobby_in_contour,
    convert_hobby_or_euler_in_contour,
};
pub use curve_harmony::{
    cubic_dkappa_ds, cubic_signed_curvature, harmonize_smooth_join_g3_in_contour,
    harmonize_smooth_join_in_contour, CurveHarmonyState,
};
pub use curve_manipulation::{
    cut_open_delete_nodes, cut_open_delete_nodes_in_contour, extract_segments, heal_delete_nodes,
    hit_test_segments, split_contour_segment, ContourSegment, SegmentHit,
};
pub use curved_trajectory::{
    compute_measurement_rainbow_quads, evaluate_curved_master_contour_nodes,
    evaluate_curved_master_point, CurvedTrajectoryLink,
};
pub use diagonal_and_ratio_hud::{
    compute_drag_tangent_extension_snap, compute_segment_handle_ratios_in_contours,
    compute_tangent_extension_snaps_in_contours, resolve_tangent_extension_ray_endpoints,
    SegmentHandleRatioHud, TangentExtensionSnap, TangentExtensionSnapKind,
};
pub use document_scene::{
    CanvasDocumentScene, CanvasGuideSet, CanvasHintingAndHoiScene, CanvasLayerAttachments,
};
pub use extra_nodes::{compute_layer_extra_nodes, hit_test_extra_nodes};
pub use fit_and_stroke::{
    apply_fit_curve_delta_to_contours, apply_fit_curve_ratio_to_contours,
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
pub use hinting_and_hoi::{
    compute_hoi_overlay_geometry, compute_pixel_preview_cells_upm, find_contour_node_upm,
    grid_fit_contour_nodes, grid_fit_coord_upm, truetype_upm_per_px, HoiCubicLayerSet,
    HoiDiamondRole, HoiOverlayGeometry, HoiSiblingContours, PostScriptHint, PostScriptHintAxis,
    PostScriptHintMutation, TrueTypeInstructionKind, TrueTypeInstructionMutation,
    TrueTypeRoundingGrid, TrueTypeVisualInstruction,
};
pub use hobby_spline::{
    evaluate_euler_spiro_chain_to_cubics, evaluate_hobby_chain_to_cubics, hobby_velocity_rho,
    SolvedSplineSegment,
};
pub use host_requests::{
    AutoTraceConfig, AutoTraceThresholdMode, CanvasHostRequest, ComponentId, SnapTargetMask,
    TextStreamEditKind, TextStreamEditRequest, TouchInteractionState,
};
pub use inflection_ops::add_inflection_points_to_contours;
pub use knife_ops::{
    compute_knife_intersections, constrain_knife_endpoint, contour_intersections_with_segment,
    intersect_finite_segments, knife_add_nodes_in_layer, knife_cut_paths_in_layer,
};
pub use lasso_and_zoom::{
    apply_mode_nodes_transform, compute_marquee_zoom, compute_rotate_tool_affine,
    compute_scale_tool_affine, next_zoom_step, point_in_polygon, prev_zoom_step,
    select_nodes_in_lasso_polygon, select_nodes_in_slanted_marquee,
    slanted_marquee_parallelogram_screen, MetricsOrigin, TransformOriginSpec,
};
pub use local_brushes::{apply_cleaner_brush_in_contours, apply_trowel_brush_in_contours};
pub use master_compatibility::{
    compute_master_compatibility_report, select_nodes_in_rect_across_layers, CompanionMasterLayer,
    MasterCompatibilityReport,
};
pub use master_matching::{
    auto_match_contours_to_companion, compute_master_kinks_and_crossings_for_layers, KinkWarning,
    MasterCompatibilityMode, MasterKinkAndCrossingReport, TrajectoryCrossing,
};
pub use math_consts::{
    MAX_INTERACTIVE_DRAG_PIXMAP_DIM, TUNNI_HIT_RADIUS_DP, TUNNI_MAX_RATIO, TUNNI_MIN_RATIO,
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
pub use optical_rhythm::{
    compute_pair_optical_whitespace, reflect_contour_across_mirror_axis, ActiveMirrorAxis,
    OpticalPairConfig, OpticalPairMetrics,
};
pub use outline_audit::{
    audit_contours_outlines, compute_angled_handle_warnings_in_contours,
    resolve_outline_audit_target, AngledHandleWarning, OutlineAuditFinding, OutlineAuditKind,
};
pub use path_ops::add_extremes_to_contours;
pub use primitives_ops::{
    bresenham_pixel_cells, build_circle_contour_nodes, build_rectangle_contour_nodes,
    compute_primitive_rect_upm, find_pixel_cell_contour_index, fit_freehand_contour_nodes,
    pixel_cell_bounds_upm, simplify_polyline_rdp, snap_to_pixel_cell, PrimitiveSubTool,
    ToolDialogRequest,
};
pub use reference_frame::{
    collect_reference_frame_snap_lines, compute_subdivision_grid_lines_upm,
    reference_frame_center_upm, CustomReferenceFrame, RectUpm, ReferenceBandLine, SnapTargetKind,
};
pub use segment_degree_ops::{
    add_offcurve_in_contour, interpolate_contours_linear, remove_offcurve_in_contour,
};
pub use segment_ops::{
    apply_cubic_segment_belly_drag, apply_cubic_segment_belly_drag_with_alt,
    apply_proportional_tension_handle_drag, find_nearest_cubic_extremum,
    remove_off_curve_segment_handles,
};
pub use segment_slide::{
    make_selected_on_curve_node_first, reverse_contour_nodes, slide_on_curve_node_along_contour,
};
pub use selection_info::{
    AnchorId, CanvasAnchor, GlyphBaseInfo, GuideAlignment, GuideInfo, MultiNodeInfo, SelectionInfo,
    SelectionInfoAction, SingleOffCurveInfo, SingleOnCurveInfo,
};
pub use selection_info_ops::{
    align_oncurve_nodes_to_background, collect_adjacent_offcurve_positions,
    compute_radially_scaled_handle_pos, enforce_smooth_collinearity_around_oncurve,
    find_first_offcurve_node_id, find_first_oncurve_with_adjacent_offcurve,
    project_point_to_background_contours, resolve_tethered_oncurve_for_offcurve,
    translate_oncurve_with_adjacent_handles, update_offcurve_with_smooth_constraint,
};
pub use servo_and_nudge::{
    classify_node_continuity_in_contours, enforce_genius_or_g3_on_handle_move,
    power_nudge_contours, servo_translate_node_in_contours, ContinuityGrade,
};
pub use smart_corners_brush::{
    evaluate_smart_corner_contours, fit_calligraphic_brush_stroke, CalligraphicNibSpec,
    CalligraphicSample, SmartCornerSpec,
};
pub use smart_handles::{
    hit_test_smart_handles, project_pointer_onto_smart_handle, smart_handle_track_endpoints_upm,
    smart_handle_upm_position,
};
pub use spline_support::contour_to_bezpath_with_splines;
pub use stem_gauge_and_balance::{
    build_superellipse_contour_nodes, compute_contour_area_and_centroid,
    compute_contours_counter_balance, compute_contours_symmetry_ghost,
    measure_contour_orthogonal_stem_thickness, ContourAreaCentroid, CounterBalanceReport,
    StemGaugeMatch, StemThicknessMeasurement, SymmetryOverlayMode,
};
pub use stem_tool::{
    adjust_structural_stem_in_contours, detect_structural_stems_in_contours, DetectedStem,
    StemAdjustmentMode, StemOrientation,
};
pub use stroke_expand::{expand_single_contour_stroke, expand_stroked_contours_preview};
pub use text_stream_ops::{
    compute_glyph_lsb_and_rsb, intersect_contours_at_horizontal_y, CanvasTextStream, GlyphSlot,
    KerningDelta, KerningIndicatorQuad, SlotScanlineMeasurement, SpacingDelta,
};
pub use tidy_align_connect::{
    find_duplicate_warning_nodes, tidy_up_contours, DuplicateWarningKind,
};
pub use transform_ops::{
    compute_bbox_rotate_or_slant_affine, compute_bbox_scale_affine, compute_layer_bounds,
    compute_selection_bounds, hit_test_bbox_knobs, hit_test_bbox_knobs_with_extent, slant_point,
    unslant_point, BBoxAction, SelectionBounds, TransformGridAnchor,
};
pub use tunni_and_comb::{
    balance_tunni_in_contour, compute_curvature_combs_for_contours,
    compute_tunni_overlays_for_contours, drag_tunni_in_contour, scale_tunni_in_contour,
    CurvatureCombPoly, CurvatureCombTooth, TunniSegmentOverlay,
};
pub use vertical_stream::{
    compute_glyph_tsb_and_bsb, compute_vertical_kerning_indicator_quads,
    recompute_vertical_slot_origins, resolve_vertical_caret_index_at_upm,
    resolve_vertical_slot_at_upm, MetricsAxis, WritingDirection,
};
