//! `SPEC-004` (`glifo v0.3.0`) Snapping, Local & Global Guides (`CanvasGuide`), Magnetic `SnapEngine`
//! (`SnapEngine`), and Alignment Zone Badges (`drawAlignmentZoneIndicators:`).

pub mod guides;
pub mod snap_engine;

pub use guides::{
    calculate_guide_intersections, normalize_guide_angle, smart_two_node_guide_geometry,
    Guide, GuideHitKind, GuideId, GuideMeasurementRay, GuideMeasurementSegment, SelectedGuideRef,
};
pub use snap_engine::{
    classify_node_alignment, ActiveSnapper, AlignmentBadgeMode, AlignmentZone, GlyphMetrics,
    SnapEngine, SnapResult,
};
