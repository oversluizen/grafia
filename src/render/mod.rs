//! Rendering projections for the three `grafia` viewports: Edit, Preview, and Shape.

pub mod edit_canvas;
pub mod preview_canvas;
pub mod shape_canvas;
pub mod tessellation;
pub mod v020_overlays;
pub mod v030_grid;
pub mod v030_overlays;
pub mod v040_overlays;

pub use edit_canvas::{
    compute_open_contour_endpoint_ticks, compute_start_node_arrow_triangle, ProjectedNode,
};
pub use preview_canvas::{
    rasterize_preview_panel, render_preview_panel_into_rect, GlifoPreviewWidget,
    GrafiaPreviewWidget, PreviewFilledProjection, PreviewPanelConfig,
};
pub use shape_canvas::{render_shape_thumbnails_tiny_skia, ShapeThumbnailProjection};
pub use tessellation::{
    is_polygon_convex, tessellate_bez_path_nonzero, tessellate_closed_contours_nonzero,
    tessellate_contours_even_odd, tessellate_contours_nonzero,
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
