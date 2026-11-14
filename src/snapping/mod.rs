#![doc = include_str!("../../docs/guide/snapping_and_reference_frames.md")]

pub mod guides;
pub mod snap_band_edges;
pub mod snap_engine;

pub use guides::{
    calculate_guide_intersections, normalize_guide_angle, smart_two_node_guide_geometry, Guide,
    GuideHitKind, GuideId, GuideMeasurementRay, GuideMeasurementSegment, GuideThickness,
    SelectedGuideRef,
};
pub use snap_band_edges::SnapQueryConfig;
pub use snap_engine::{
    classify_node_alignment, ActiveSnapper, AlignmentBadgeMode, AlignmentZone, GlyphMetrics,
    SnapEngine, SnapGuideContext, SnapResult,
};
