//! Magnetic `SnapEngine` (`SnapEngine` parity) & Alignment Zone / Metric Classification
//! (`SPEC-004 §2.3–§2.4`, `SPEC-009 §2.6`).

use crate::core::host_requests::SnapTargetMask;
use crate::core::math_consts::{
    DEFAULT_ALIGNMENT_ZONE_SIZE_UPM, DEFAULT_ASCENDER_UPM, DEFAULT_BASELINE_UPM,
    DEFAULT_CAP_HEIGHT_UPM, DEFAULT_DESCENDER_UPM, DEFAULT_SNAP_THRESHOLD_DP, DEFAULT_X_HEIGHT_UPM,
    GUIDE_BADGE_TOL_MAX_UPM, GUIDE_BADGE_TOL_MIN_UPM, METRIC_BADGE_TOLERANCE_UPM,
};
use crate::core::{unslant_point, FontPoint, Node, NodeId, SnapTargetKind};
use crate::snapping::guides::Guide;
use crate::snapping::snap_band_edges::SnapQueryConfig;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct AlignmentZone {
    pub position: f64,
    pub size: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct GlyphMetrics {
    pub ascender: f64,
    pub cap_height: f64,
    pub x_height: f64,
    pub baseline: f64,
    pub descender: f64,
    #[serde(default)]
    pub advance_width: f64,
    #[serde(default)]
    pub italic_angle_deg: f64,
    pub alignment_zones: Vec<AlignmentZone>,
}

impl Default for GlyphMetrics {
    fn default() -> Self {
        Self {
            ascender: DEFAULT_ASCENDER_UPM,
            cap_height: DEFAULT_CAP_HEIGHT_UPM,
            x_height: DEFAULT_X_HEIGHT_UPM,
            baseline: DEFAULT_BASELINE_UPM,
            descender: DEFAULT_DESCENDER_UPM,
            advance_width: 0.0,
            italic_angle_deg: 0.0,
            alignment_zones: vec![
                AlignmentZone {
                    position: DEFAULT_CAP_HEIGHT_UPM,
                    size: DEFAULT_ALIGNMENT_ZONE_SIZE_UPM,
                },
                AlignmentZone {
                    position: DEFAULT_X_HEIGHT_UPM,
                    size: DEFAULT_ALIGNMENT_ZONE_SIZE_UPM,
                },
                AlignmentZone {
                    position: DEFAULT_BASELINE_UPM,
                    size: -DEFAULT_ALIGNMENT_ZONE_SIZE_UPM,
                },
                AlignmentZone {
                    position: DEFAULT_DESCENDER_UPM,
                    size: -DEFAULT_ALIGNMENT_ZONE_SIZE_UPM,
                },
            ],
        }
    }
}

impl GlyphMetrics {
    #[must_use]
    pub fn horizontal_metric_lines(&self) -> [f64; 5] {
        [
            self.ascender,
            self.cap_height,
            self.x_height,
            self.baseline,
            self.descender,
        ]
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum AlignmentBadgeMode {
    OnMetricLine,
    OnLocalGuide,
    OnGlobalGuide,
    InAlignmentZone,
    OnVerticalMetric,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ActiveSnapper {
    pub from: FontPoint,
    pub to: FontPoint,
    pub angle_deg: Option<f64>,
    pub is_point_target: bool,
    pub target_kind: Option<SnapTargetKind>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct SnapResult {
    pub snapped_point: FontPoint,
    pub delta: (f64, f64),
    pub active_snappers: Vec<ActiveSnapper>,
}

/// Guide visibility and local/global guide slices passed to [`SnapEngine::snap_point`].
#[derive(Debug, Clone, Copy)]
pub struct SnapGuideContext<'a> {
    pub show_guides: bool,
    pub local_guides: &'a [Guide],
    pub global_guides: &'a [Guide],
}

pub struct SnapEngine;

impl SnapEngine {
    /// Evaluates magnetic snapping (`PathRenderer::snapAtPoint:layer:withModifier:tolerance:`).
    ///
    /// When `ctrl_bypass == true`, magnetic snapping is bypassed and returns the raw `query` point.
    #[must_use]
    pub fn snap_point(
        query: FontPoint,
        contours_and_excluded: (&[Vec<Node>], &[NodeId]),
        metrics: &GlyphMetrics,
        advance_and_italic: (f64, f64),
        guides: SnapGuideContext<'_>,
        zoom: f32,
        ctrl_bypass: bool,
    ) -> SnapResult {
        let (contours, excluded_nodes) = contours_and_excluded;
        let (advance_width, italic_angle_deg) = advance_and_italic;
        Self::snap_point_with_config(
            query,
            &SnapQueryConfig {
                contours,
                excluded_nodes,
                background_contours: &[],
                show_background: false,
                metrics,
                show_metrics: true,
                advance_width,
                italic_angle_deg,
                show_guides: guides.show_guides,
                local_guides: guides.local_guides,
                global_guides: guides.global_guides,
                show_grid: false,
                grid_step_upm: 0.0,
                zoom,
                snap_radius_dp: DEFAULT_SNAP_THRESHOLD_DP,
                targets: SnapTargetMask::all(),
                custom_reference_frame: None,
                ctrl_bypass,
            },
        )
    }
}

/// Classifies an `OnCurve` node point for Pass 4 Alignment Indicators.
#[must_use]
pub fn classify_node_alignment(
    pt: FontPoint,
    metrics: &GlyphMetrics,
    advance_and_italic: (f64, f64),
    grid_step_upm: f64,
    show_guides: bool,
    local_guides: &[Guide],
    global_guides: &[Guide],
) -> Option<AlignmentBadgeMode> {
    let (advance_width, italic_angle_deg) = advance_and_italic;
    for y_m in metrics.horizontal_metric_lines() {
        if (pt.y.0 - y_m).abs() < METRIC_BADGE_TOLERANCE_UPM {
            return Some(AlignmentBadgeMode::OnMetricLine);
        }
    }
    for zone in &metrics.alignment_zones {
        if (pt.y.0 - zone.position).abs() < METRIC_BADGE_TOLERANCE_UPM {
            return Some(AlignmentBadgeMode::OnMetricLine);
        }
    }

    for zone in &metrics.alignment_zones {
        let in_zone = if zone.size > 0.0 {
            pt.y.0 >= zone.position && pt.y.0 <= zone.position + zone.size
        } else if zone.size < 0.0 {
            pt.y.0 >= zone.position + zone.size && pt.y.0 <= zone.position
        } else {
            false
        };
        if in_zone {
            return Some(AlignmentBadgeMode::InAlignmentZone);
        }
    }

    let unslanted = unslant_point(pt, italic_angle_deg, metrics.x_height * 0.5);
    if unslanted.x.0.abs() < METRIC_BADGE_TOLERANCE_UPM
        || (advance_width > 0.0
            && (unslanted.x.0 - advance_width).abs() < METRIC_BADGE_TOLERANCE_UPM)
    {
        return Some(AlignmentBadgeMode::OnVerticalMetric);
    }

    if show_guides {
        let guide_tol =
            (grid_step_upm * 0.5).clamp(GUIDE_BADGE_TOL_MIN_UPM, GUIDE_BADGE_TOL_MAX_UPM);
        for g in local_guides {
            if g.perpendicular_distance(pt) < guide_tol {
                return Some(AlignmentBadgeMode::OnLocalGuide);
            }
        }
        for g in global_guides {
            if g.perpendicular_distance(pt) < guide_tol {
                return Some(AlignmentBadgeMode::OnGlobalGuide);
            }
        }
    }

    None
}
