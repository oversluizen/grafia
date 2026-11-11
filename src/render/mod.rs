//! Rendering projections for the three `grafia` viewports: Edit, Preview, and Shape.

pub mod edit_canvas;
pub mod preview_canvas;
pub mod preview_canvas_ts;
pub mod shape_canvas;
pub mod tessellation;
pub(crate) mod ts_primitives;
pub mod v020_overlays;
pub mod v030_grid;
pub mod v030_overlays;
pub mod v030_overlays_ts;
pub mod v040_overlays;
pub mod v040_overlays_ts;
pub mod v050_overlays;
pub mod v050_overlays_ts;
pub mod v060_hud;
pub mod v060_overlays;
pub mod v060_overlays_ts;
pub mod v060_preview;
pub mod v070_overlays;

pub use edit_canvas::{
    compute_open_contour_endpoint_ticks, compute_start_node_arrow_triangle, ProjectedNode,
};
pub use preview_canvas::{
    rasterize_preview_panel, render_preview_panel_into_rect, GrafiaPreviewWidget,
    PreviewFilledProjection, PreviewPanelConfig,
};
pub use shape_canvas::{render_shape_thumbnails_tiny_skia, ShapeThumbnailProjection};
pub use tessellation::{
    is_polygon_convex, tessellate_bez_path_nonzero, tessellate_closed_contours_nonzero,
    tessellate_contours_even_odd, tessellate_contours_nonzero, tessellate_glyph_contours_nonzero,
};
pub use v020_overlays::{
    paint_duplicate_warnings_egui, paint_v020_overlays_egui, render_duplicate_warnings_tiny_skia,
    render_v020_overlays_tiny_skia,
};
pub use v030_grid::{paint_v030_grid_egui, render_v030_grid_tiny_skia};
pub use v030_overlays::{
    paint_v030_alignment_badges_egui, paint_v030_alignment_zones_egui, paint_v030_overlays_egui,
    render_v030_alignment_badges_tiny_skia, render_v030_alignment_zones_tiny_skia,
    render_v030_overlays_tiny_skia,
};
pub use v040_overlays::{paint_v040_overlays_egui, render_v040_overlays_tiny_skia};
pub use v050_overlays::{paint_v050_overlays_egui, render_v050_overlays_tiny_skia};
pub use v060_hud::{
    compute_hud_proportional_icon_rect, compute_hud_rect, compute_hud_transform_grid_dot_positions,
    format_selection_info_hud_lines, paint_v060_hud_egui, render_v060_hud_tiny_skia,
    should_render_selection_info_hud, TRANSFORM_GRID_ANCHORS_3X3,
};
pub use v060_overlays::{
    compute_ruler_tick_steps_upm, paint_v060_overlays_egui, render_v060_overlays_tiny_skia,
};
pub use v060_preview::{
    apply_separable_gaussian_blur_rgba, build_gaussian_kernel_1d, render_preview_strip_rgba,
    PreviewRenderConfig, PreviewStripCache,
};
pub use v070_overlays::{paint_v070_overlays_egui, render_v070_overlays_tiny_skia};
