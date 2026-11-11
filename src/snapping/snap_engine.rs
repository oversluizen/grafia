//! Magnetic `SnapEngine` (`SnapEngine` parity) & Alignment Zone / Metric Classification
//! (`SPEC-004 §2.3–§2.4`, `canvas engine` `spec_ref..spec_ref`, `spec_ref..spec_ref`).

use crate::core::{unslant_point, FontPoint, Node, NodeId, NodeType};
use crate::snapping::guides::Guide;
use crate::tokens::{
    DEFAULT_ALIGNMENT_ZONE_SIZE_UPM, DEFAULT_ASCENDER_UPM, DEFAULT_BASELINE_UPM,
    DEFAULT_CAP_HEIGHT_UPM, DEFAULT_DESCENDER_UPM, DEFAULT_SNAP_THRESHOLD_DP, DEFAULT_X_HEIGHT_UPM,
    DEG_TO_RAD_F64, GUIDE_BADGE_TOL_MAX_UPM, GUIDE_BADGE_TOL_MIN_UPM, MAX_ACTIVE_SNAPPERS,
    METRIC_BADGE_TOLERANCE_UPM, RIGHT_ANGLE_DEG_F64, SNAP_LINE_INTERSECT_DET_EPS,
};
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
    /// Mode `1` (`spec_ref`): Diamond on horizontal metric line or alignment zone base.
    OnMetricLine,
    /// Mode `2` (`spec_ref`): Blue diamond on Local Guide (`#3E4CCF66`).
    OnLocalGuide,
    /// Mode `2` (`spec_ref`): Red diamond on Global Guide (`#CF4F3E66`).
    OnGlobalGuide,
    /// Mode `3` (`spec_ref`): Circle inside alignment zone overshoot band (`#CEA77C80`).
    InAlignmentZone,
    /// Mode `4` (`spec_ref`): Sheared rectangle on vertical LSB/RSB line (`#CEA77C80`).
    OnVerticalMetric,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ActiveSnapper {
    pub from: FontPoint,
    pub to: FontPoint,
    pub angle_deg: Option<f64>,
    pub is_point_target: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub struct SnapResult {
    pub snapped_point: FontPoint,
    pub delta: (f64, f64),
    pub active_snappers: Vec<ActiveSnapper>,
}

#[derive(Debug, Clone, Copy)]
struct CandidateLineSnapper {
    anchor: FontPoint,
    angle_deg: f64,
    proj: FontPoint,
    dist: f64,
}

fn project_onto_angled_line(pt: FontPoint, anchor: FontPoint, angle_deg: f64) -> (FontPoint, f64) {
    let rad = angle_deg * DEG_TO_RAD_F64;
    let (ux, uy) = (rad.cos(), rad.sin());
    let dx = pt.x.0 - anchor.x.0;
    let dy = pt.y.0 - anchor.y.0;
    let t = dx * ux + dy * uy;
    let proj = FontPoint::new(anchor.x.0 + t * ux, anchor.y.0 + t * uy);
    (proj, pt.distance_to(proj))
}

fn intersect_two_infinite_lines(
    a1: FontPoint,
    angle1_deg: f64,
    a2: FontPoint,
    angle2_deg: f64,
) -> Option<FontPoint> {
    let r1 = angle1_deg * DEG_TO_RAD_F64;
    let r2 = angle2_deg * DEG_TO_RAD_F64;
    let (u1x, u1y) = (r1.cos(), r1.sin());
    let (u2x, u2y) = (r2.cos(), r2.sin());
    let det = u1x * (-u2y) - u1y * (-u2x);
    if det.abs() <= SNAP_LINE_INTERSECT_DET_EPS {
        return None;
    }
    let dx = a2.x.0 - a1.x.0;
    let dy = a2.y.0 - a1.y.0;
    let t1 = (dx * (-u2y) - dy * (-u2x)) / det;
    Some(FontPoint::new(a1.x.0 + t1 * u1x, a1.y.0 + t1 * u1y))
}

pub struct SnapEngine;

impl SnapEngine {
    /// Evaluates magnetic snapping (`PathRenderer::snapAtPoint:layer:withModifier:tolerance:` @ `spec_ref`).
    ///
    /// When `ctrl_bypass == true`, magnetic snapping is bypassed and returns the raw `query` point.
    #[allow(clippy::too_many_arguments)]
    #[must_use]
    pub fn snap_point(
        query: FontPoint,
        contours: &[Vec<Node>],
        excluded_nodes: &[NodeId],
        metrics: &GlyphMetrics,
        advance_width: f64,
        italic_angle_deg: f64,
        show_guides: bool,
        local_guides: &[Guide],
        global_guides: &[Guide],
        zoom: f32,
        ctrl_bypass: bool,
    ) -> SnapResult {
        if ctrl_bypass {
            return SnapResult {
                snapped_point: query,
                delta: (0.0, 0.0),
                active_snappers: Vec::new(),
            };
        }

        let tolerance_upm =
            f64::from(DEFAULT_SNAP_THRESHOLD_DP) / f64::from(zoom.max(f32::EPSILON));
        let mut best_point: Option<(FontPoint, f64)> = None;
        let mut line_candidates: Vec<CandidateLineSnapper> = Vec::new();

        let mut push_line = |anchor: FontPoint, angle_deg: f64| {
            let (proj, dist) = project_onto_angled_line(query, anchor, angle_deg);
            if dist <= tolerance_upm {
                line_candidates.push(CandidateLineSnapper {
                    anchor,
                    angle_deg,
                    proj,
                    dist,
                });
            }
        };

        let mut push_point = |target: FontPoint| {
            let d = query.distance_to(target);
            if d <= tolerance_upm && best_point.is_none_or(|(_, best_d)| d < best_d) {
                best_point = Some((target, d));
            }
        };

        // 1. Unselected contour nodes (horizontal 0°, vertical 90°, italic angle, and direct point snap)
        let slant_line_angle = RIGHT_ANGLE_DEG_F64 - italic_angle_deg;
        for contour in contours {
            let n = contour.len();
            for (idx, node) in contour.iter().enumerate() {
                if excluded_nodes.contains(&node.id)
                    || (excluded_nodes.len() == 1
                        && n >= 3
                        && node.node_type == NodeType::OffCurve
                        && ((contour[(idx + n - 1) % n].node_type == NodeType::Smooth
                            && contour[(idx + n - 2) % n].id == excluded_nodes[0])
                            || (contour[(idx + 1) % n].node_type == NodeType::Smooth
                                && contour[(idx + 2) % n].id == excluded_nodes[0])))
                {
                    continue;
                }
                push_point(node.point);
                push_line(node.point, 0.0);
                push_line(node.point, RIGHT_ANGLE_DEG_F64);
                if italic_angle_deg.abs() > 0.0 {
                    push_line(node.point, slant_line_angle);
                }
            }
        }

        // 2. Metrics lines & alignment zones
        for y_m in metrics.horizontal_metric_lines() {
            push_line(FontPoint::new(query.x.0, y_m), 0.0);
        }
        for zone in &metrics.alignment_zones {
            push_line(FontPoint::new(query.x.0, zone.position), 0.0);
        }
        let half_xh = metrics.x_height * 0.5;
        push_line(FontPoint::new(0.0, half_xh), slant_line_angle);
        if advance_width > 0.0 {
            push_line(FontPoint::new(advance_width, half_xh), slant_line_angle);
        }

        // 3. Visible Local & Global Guides
        if show_guides {
            for guide in local_guides.iter().chain(global_guides.iter()) {
                push_point(guide.position);
                push_line(guide.position, guide.angle_deg);
            }
        }

        // Sort line candidates by perpendicular distance ascending
        line_candidates.sort_by(|a, b| {
            a.dist
                .partial_cmp(&b.dist)
                .unwrap_or(std::cmp::Ordering::Equal)
        });

        // Check if we have two non-parallel line snappers from distinct anchors whose intersection is within tolerance
        if let Some(&first) = line_candidates.first() {
            if let Some(&second) = line_candidates.iter().skip(1).find(|cand| {
                first.anchor.distance_to(cand.anchor) > SNAP_LINE_INTERSECT_DET_EPS
                    && intersect_two_infinite_lines(
                        first.anchor,
                        first.angle_deg,
                        cand.anchor,
                        cand.angle_deg,
                    )
                    .is_some()
            }) {
                if let Some(inter) = intersect_two_infinite_lines(
                    first.anchor,
                    first.angle_deg,
                    second.anchor,
                    second.angle_deg,
                ) {
                    if query.distance_to(inter) <= tolerance_upm * 2.0
                        && best_point.is_none_or(|(_, pt_dist)| {
                            query.distance_to(inter) + SNAP_LINE_INTERSECT_DET_EPS < pt_dist
                        })
                    {
                        let mut active = vec![
                            ActiveSnapper {
                                from: first.anchor,
                                to: inter,
                                angle_deg: Some(first.angle_deg),
                                is_point_target: false,
                            },
                            ActiveSnapper {
                                from: second.anchor,
                                to: inter,
                                angle_deg: Some(second.angle_deg),
                                is_point_target: false,
                            },
                        ];
                        active.truncate(MAX_ACTIVE_SNAPPERS);
                        return SnapResult {
                            snapped_point: inter,
                            delta: (inter.x.0 - query.x.0, inter.y.0 - query.y.0),
                            active_snappers: active,
                        };
                    }
                }
            }
        }

        if let Some((pt_target, _)) = best_point {
            return SnapResult {
                snapped_point: pt_target,
                delta: (pt_target.x.0 - query.x.0, pt_target.y.0 - query.y.0),
                active_snappers: vec![ActiveSnapper {
                    from: pt_target,
                    to: pt_target,
                    angle_deg: None,
                    is_point_target: true,
                }],
            };
        }

        if let Some(&best_line) = line_candidates.first() {
            return SnapResult {
                snapped_point: best_line.proj,
                delta: (
                    best_line.proj.x.0 - query.x.0,
                    best_line.proj.y.0 - query.y.0,
                ),
                active_snappers: vec![ActiveSnapper {
                    from: best_line.anchor,
                    to: best_line.proj,
                    angle_deg: Some(best_line.angle_deg),
                    is_point_target: false,
                }],
            };
        }

        SnapResult {
            snapped_point: query,
            delta: (0.0, 0.0),
            active_snappers: Vec::new(),
        }
    }
}

/// Classifies an `OnCurve` node point for Pass 4 Alignment Indicators
/// (`FontMasterMetrics::isOnMetrics:metrics:italicAngle:inLayer:` @ `spec_ref` + `drawAlignmentZoneIndicators:` @ `spec_ref`).
#[allow(clippy::too_many_arguments)]
#[must_use]
pub fn classify_node_alignment(
    pt: FontPoint,
    metrics: &GlyphMetrics,
    advance_width: f64,
    italic_angle_deg: f64,
    grid_step_upm: f64,
    show_guides: bool,
    local_guides: &[Guide],
    global_guides: &[Guide],
) -> Option<AlignmentBadgeMode> {
    // 1. Mode 1: On horizontal metric line or alignment zone base (`|y - pos| < 0.6`)
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

    // 2. Mode 3: Inside alignment zone overshoot band (`pos <= y <= pos + size` or `pos + size <= y <= pos`)
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

    // 3. Mode 4: On vertical LSB (x = 0) or RSB (x = advance_width) metric line (sheared by italic_angle_deg)
    let unslanted = unslant_point(pt, italic_angle_deg, metrics.x_height * 0.5);
    if unslanted.x.0.abs() < METRIC_BADGE_TOLERANCE_UPM
        || (advance_width > 0.0
            && (unslanted.x.0 - advance_width).abs() < METRIC_BADGE_TOLERANCE_UPM)
    {
        return Some(AlignmentBadgeMode::OnVerticalMetric);
    }

    // 4. Mode 2: On visible Local or Global Guide (`clamp(0.5 * grid_step, 0.4, 2.0)` @ `spec_ref..spec_ref`)
    // Note: in `drawAlignmentZoneIndicators:` (`spec_ref..spec_ref`), guide alignment is evaluated independently
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
