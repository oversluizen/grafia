#![doc = include_str!("../../docs/guide/rendering_and_caching.md")]

pub mod bbox_and_direction_overlays;
pub mod canvas_grid;
pub mod canvas_render_cache;
pub mod color_layers_pass;
pub mod color_layers_pixmap_pass;
pub mod component_and_stroke_overlays;
pub mod drawing_tool_overlays;
pub mod edit_canvas;
pub mod guide_and_zone_overlays;
pub mod guide_band_and_loupe_overlays;
pub mod guides_pixmap_pass;
pub mod harmony_overlays;
pub mod hinting_and_hoi_pass;
pub mod hinting_and_hoi_pixmap_pass;
pub mod loupe_pixmap_pass;
pub mod preview_bands;
pub mod preview_canvas;
pub mod preview_panel_pixmap;
pub mod preview_strip_cache;
pub mod reference_frame_pass;
pub mod reference_frame_pixmap_pass;
pub mod ruler_and_background_overlays;
pub mod rulers_pixmap_pass;
pub mod selection_info_hud;
pub mod selection_info_hud_format;
pub mod shape_canvas;
pub mod shared_geom;
pub mod smart_corner_overlays;
pub mod spline_and_handle_overlays;
pub mod splines_pixmap_pass;
pub mod stem_gauge_and_audit_overlays;
pub mod tessellation;
pub mod text_strip_overlays;
pub mod text_strip_pixmap_pass;
pub mod tools_pixmap_pass;
pub(crate) mod ts_primitives;

pub use bbox_and_direction_overlays::{
    paint_bbox_and_direction_overlays_egui, paint_duplicate_warnings_egui,
    render_bbox_and_direction_overlays_tiny_skia, render_duplicate_warnings_tiny_skia,
};
pub use canvas_grid::{paint_canvas_grid_egui, render_canvas_grid_tiny_skia};
pub use canvas_render_cache::{
    cached_contour_bezpath, cached_expanded_stroke_contours, cached_extra_nodes,
    get_or_upload_background_texture, get_or_upload_color_sublayers_texture,
    with_preview_strip_cache,
};
pub use color_layers_pass::{paint_color_layer_handles_pass, paint_color_layers_pass};
pub use color_layers_pixmap_pass::{
    paint_color_layer_handles_pixmap_pass, paint_color_layers_pixmap_pass,
    rasterize_color_sublayers_rgba, render_colored_thumbnail,
};
pub use component_and_stroke_overlays::{
    paint_component_and_stroke_overlays_egui, render_component_and_stroke_overlays_tiny_skia,
};
pub use drawing_tool_overlays::{
    paint_drawing_tool_overlays_egui, render_drawing_tool_overlays_tiny_skia,
};
pub use edit_canvas::{
    compute_open_contour_endpoint_ticks, compute_start_node_arrow_triangle, ProjectedNode,
};
pub use guide_and_zone_overlays::{
    paint_alignment_badges_egui, paint_alignment_zones_egui, paint_guide_and_snap_overlays_egui,
    render_alignment_badges_tiny_skia, render_alignment_zones_tiny_skia,
    render_guide_and_snap_overlays_tiny_skia,
};
pub use guide_band_and_loupe_overlays::{
    paint_guide_band_egui, paint_offcurve_handle_egui, paint_top_cursor_and_loupe_overlays_egui,
    render_edit_active_fill_tiny_skia, render_guide_band_tiny_skia,
    render_offcurve_handle_tiny_skia, render_top_cursor_and_loupe_overlays_tiny_skia,
};
pub use harmony_overlays::{paint_harmony_overlays_egui, render_harmony_overlays_tiny_skia};
pub use hinting_and_hoi_pass::{paint_hinting_and_hoi_pass, paint_pixel_preview_and_ghost_pass};
pub use hinting_and_hoi_pixmap_pass::{
    paint_hinting_and_hoi_pixmap_pass, paint_pixel_preview_and_ghost_pixmap_pass,
};
pub use preview_bands::{
    paint_curved_trajectories_egui, paint_measurement_rainbow_egui, paint_optical_rhythm_fill_egui,
    paint_overshoot_ribbons_egui, render_curved_trajectories_tiny_skia,
    render_measurement_rainbow_tiny_skia, render_optical_rhythm_fill_tiny_skia,
    render_overshoot_ribbons_tiny_skia,
};
pub use preview_canvas::{
    rasterize_preview_panel, render_preview_panel_into_rect, GrafiaPreviewWidget,
    PreviewFilledProjection, PreviewPanelConfig,
};
pub use preview_strip_cache::{
    apply_separable_gaussian_blur_rgba, build_gaussian_kernel_1d, render_preview_strip_rgba,
    PreviewRenderConfig, PreviewStripCache,
};
pub use reference_frame_pass::{paint_custom_reference_frame_pass, ReferenceFramePalette};
pub use reference_frame_pixmap_pass::paint_custom_reference_frame_pixmap_pass;
pub use ruler_and_background_overlays::{
    compute_ruler_tick_steps_upm, paint_ruler_and_background_overlays_egui,
    render_ruler_and_background_overlays_tiny_skia, RulerTickRangeUpm,
};
pub use selection_info_hud::{
    compute_hud_proportional_icon_rect, compute_hud_rect, compute_hud_transform_grid_dot_positions,
    format_selection_info_hud_lines, paint_selection_info_hud_egui,
    render_selection_info_hud_tiny_skia, should_render_selection_info_hud,
    TRANSFORM_GRID_ANCHORS_3X3,
};
pub use shape_canvas::{render_shape_thumbnails_tiny_skia, ShapeThumbnailProjection};
pub use spline_and_handle_overlays::{
    paint_background_image_egui, paint_spline_and_handle_overlays_egui,
    paint_spline_node_marker_egui, render_background_image_tiny_skia,
    render_spline_and_handle_overlays_tiny_skia, render_spline_node_marker_tiny_skia,
};
pub use tessellation::{
    is_polygon_convex, tessellate_bez_path_nonzero, tessellate_closed_contours_nonzero,
    tessellate_contours_even_odd, tessellate_contours_nonzero, tessellate_glyph_contours_nonzero,
};
pub use text_strip_overlays::{
    paint_text_strip_overlays_egui, render_text_strip_overlays_tiny_skia,
};
