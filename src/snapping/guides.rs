//! `CanvasGuide` Data Model, Geometry, Smart 2-Node Rule, Hit-Testing, and Measurement Intersections
//! (`SPEC-004 §2.2`, `core geometry` `spec_ref..spec_ref`).

use crate::core::{
    extract_segments, ContourSegment, FontPoint, Node, ScreenPoint, ViewportTransform,
};
use crate::tokens::{
    CURVE_FLATTEN_TOLERANCE, CURVE_ROOT_DEDUP_EPS, DEG_TO_RAD_F64, FULL_CIRCLE_DEG_F64,
    GUIDE_ANGLE_CHANGE_EPS, GUIDE_HALF_CIRCLE_DEG, GUIDE_INFINITE_HALF_EXTENT_UPM,
    GUIDE_KNOB_HIT_RADIUS_DP, GUIDE_LINE_HIT_TOLERANCE_DP, RAD_TO_DEG_F64, RIGHT_ANGLE_DEG_F64,
    SNAP_LINE_INTERSECT_DET_EPS,
};
use kurbo::{Line, PathEl};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub struct GuideId(pub u64);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct SelectedGuideRef {
    pub id: GuideId,
    pub is_global: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GuideHitKind {
    Knob(SelectedGuideRef),
    Line(SelectedGuideRef),
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Guide {
    pub id: GuideId,
    pub position: FontPoint,
    pub angle_deg: f64,
    pub is_global: bool,
    pub locked: bool,
    pub lock_angle: bool,
    pub show_measurement: bool,
    pub name: Option<String>,
    #[serde(default)]
    pub alignment: crate::core::GuideAlignment,
}

#[derive(Debug, Clone, PartialEq)]
pub struct GuideMeasurementSegment {
    pub start: FontPoint,
    pub end: FontPoint,
    pub midpoint: FontPoint,
    pub distance: f64,
}

#[derive(Debug, Clone, PartialEq)]
pub struct GuideMeasurementRay {
    pub guide_id: GuideId,
    pub is_global: bool,
    pub direction: (f64, f64),
    pub intersections: Vec<FontPoint>,
    pub segments: Vec<GuideMeasurementSegment>,
}

/// Normalizes any angle into `(-180.0, 180.0]` (`CanvasGuide::setAngle:` @ `spec_ref..spec_ref`).
#[must_use]
pub fn normalize_guide_angle(mut angle_deg: f64) -> f64 {
    if !angle_deg.is_finite() {
        return 0.0;
    }
    while angle_deg > GUIDE_HALF_CIRCLE_DEG {
        angle_deg -= FULL_CIRCLE_DEG_F64;
    }
    while angle_deg <= -GUIDE_HALF_CIRCLE_DEG {
        angle_deg += FULL_CIRCLE_DEG_F64;
    }
    angle_deg
}

/// Computes the midpoint and normalized angle through two selected points (`PathRenderer::doAddGuide:` @ `spec_ref`).
#[must_use]
pub fn smart_two_node_guide_geometry(a: FontPoint, b: FontPoint) -> (FontPoint, f64) {
    let mid = FontPoint::new((a.x.0 + b.x.0) * 0.5, (a.y.0 + b.y.0) * 0.5);
    let dx = b.x.0 - a.x.0;
    let dy = b.y.0 - a.y.0;
    let angle = if dx.hypot(dy) <= GUIDE_ANGLE_CHANGE_EPS {
        0.0
    } else {
        normalize_guide_angle(dy.atan2(dx) * RAD_TO_DEG_F64)
    };
    (mid, angle)
}

impl Guide {
    #[must_use]
    pub fn new(id: GuideId, position: FontPoint, angle_deg: f64, is_global: bool) -> Self {
        Self {
            id,
            position,
            angle_deg: normalize_guide_angle(angle_deg),
            is_global,
            locked: false,
            lock_angle: false,
            show_measurement: false,
            name: None,
            alignment: crate::core::GuideAlignment::Lsb,
        }
    }

    /// Returns unit direction vector `(cos(theta), sin(theta))` for `self.angle_deg`.
    #[must_use]
    pub fn direction_unit(&self) -> (f64, f64) {
        let rad = self.angle_deg * DEG_TO_RAD_F64;
        (rad.cos(), rad.sin())
    }

    /// Returns infinite guide endpoints (`beginPointForLayerWidth:` @ `spec_ref`, `endPointForLayerWidth:` @ `spec_ref`).
    #[must_use]
    pub fn infinite_endpoints(&self) -> (FontPoint, FontPoint) {
        let (ux, uy) = self.direction_unit();
        let begin = FontPoint::new(
            self.position.x.0 - GUIDE_INFINITE_HALF_EXTENT_UPM * ux,
            self.position.y.0 - GUIDE_INFINITE_HALF_EXTENT_UPM * uy,
        );
        let end = FontPoint::new(
            self.position.x.0 + GUIDE_INFINITE_HALF_EXTENT_UPM * ux,
            self.position.y.0 + GUIDE_INFINITE_HALF_EXTENT_UPM * uy,
        );
        (begin, end)
    }

    /// Updates `self.angle_deg` if `!self.locked && !self.lock_angle` (`CanvasGuide::setAngle:` @ `spec_ref`).
    pub fn set_angle(&mut self, new_angle_deg: f64) -> bool {
        if self.locked || self.lock_angle {
            return false;
        }
        let norm = normalize_guide_angle(new_angle_deg);
        if (norm - self.angle_deg).abs() <= GUIDE_ANGLE_CHANGE_EPS {
            return false;
        }
        self.angle_deg = norm;
        true
    }

    /// Rotates the guide by `+90.0°` perpendicular (`doubleClickGuideKnob`).
    pub fn flip_perpendicular(&mut self) -> bool {
        self.set_angle(self.angle_deg + RIGHT_ANGLE_DEG_F64)
    }

    /// Projects a font-space point onto this infinite guide line (`nearest_point_on_line`).
    #[must_use]
    pub fn nearest_point_on_line(&self, pt: FontPoint) -> FontPoint {
        let (ux, uy) = self.direction_unit();
        let dx = pt.x.0 - self.position.x.0;
        let dy = pt.y.0 - self.position.y.0;
        let t = dx * ux + dy * uy;
        FontPoint::new(self.position.x.0 + t * ux, self.position.y.0 + t * uy)
    }

    /// Returns perpendicular distance in UPM from `pt` to this guide line (`CanvasGuide::hitTest:inLayer:tolerance:` @ `spec_ref`).
    #[must_use]
    pub fn perpendicular_distance(&self, pt: FontPoint) -> f64 {
        let proj = self.nearest_point_on_line(pt);
        pt.distance_to(proj)
    }

    /// Hit-tests the guide knob and line in screen space.
    #[must_use]
    pub fn hit_test_screen(
        &self,
        cursor_scr: ScreenPoint,
        viewport: &ViewportTransform,
    ) -> Option<GuideHitKind> {
        let guide_ref = SelectedGuideRef {
            id: self.id,
            is_global: self.is_global,
        };
        let knob_scr = viewport.to_screen(self.position);
        if cursor_scr.distance_to(knob_scr) <= GUIDE_KNOB_HIT_RADIUS_DP {
            return Some(GuideHitKind::Knob(guide_ref));
        }
        let cursor_font = viewport.to_font(cursor_scr);
        let tol_upm =
            f64::from(GUIDE_LINE_HIT_TOLERANCE_DP) / f64::from(viewport.zoom.max(f32::EPSILON));
        if self.perpendicular_distance(cursor_font) <= tol_upm {
            return Some(GuideHitKind::Line(guide_ref));
        }
        None
    }
}

fn intersect_infinite_ray_with_segment(
    origin: FontPoint,
    ux: f64,
    uy: f64,
    a: FontPoint,
    b: FontPoint,
) -> Option<(f64, FontPoint)> {
    let sx = b.x.0 - a.x.0;
    let sy = b.y.0 - a.y.0;
    let det = ux * (-sy) - uy * (-sx);
    if det.abs() <= SNAP_LINE_INTERSECT_DET_EPS {
        return None;
    }
    let rx = a.x.0 - origin.x.0;
    let ry = a.y.0 - origin.y.0;
    let t_ray = (rx * (-sy) - ry * (-sx)) / det;
    let u_seg = (ux * ry - uy * rx) / det;
    if (-CURVE_ROOT_DEDUP_EPS..=1.0 + CURVE_ROOT_DEDUP_EPS).contains(&u_seg) {
        let u_clamped = u_seg.clamp(0.0, 1.0);
        let pt = FontPoint::new(a.x.0 + u_clamped * sx, a.y.0 + u_clamped * sy);
        Some((t_ray, pt))
    } else {
        None
    }
}

/// Computes sorted contour intersections and interior/consecutive distances along `guide`
/// (`CanvasGuide::calculateIntersectionsForLayer:` @ `spec_ref`).
#[must_use]
pub fn calculate_guide_intersections(
    guide: &Guide,
    contours: &[Vec<Node>],
    closed_flags: &[bool],
) -> GuideMeasurementRay {
    let (ux, uy) = guide.direction_unit();
    let mut raw_hits: Vec<(f64, FontPoint)> = Vec::new();

    for (c_idx, contour) in contours.iter().enumerate() {
        let is_closed = closed_flags.get(c_idx).copied().unwrap_or(true);
        if contour.len() < 2 {
            continue;
        }
        for seg in extract_segments(contour, is_closed) {
            match seg {
                ContourSegment::Line {
                    start_idx, end_idx, ..
                } => {
                    let a = contour[start_idx].point;
                    let b = contour[end_idx].point;
                    if let Some(hit) =
                        intersect_infinite_ray_with_segment(guide.position, ux, uy, a, b)
                    {
                        raw_hits.push(hit);
                    }
                }
                ContourSegment::Cubic { cubic, .. } => {
                    let mut prev = FontPoint::new(cubic.p0.x, cubic.p0.y);
                    kurbo::flatten(
                        [
                            PathEl::MoveTo(cubic.p0),
                            PathEl::CurveTo(cubic.p1, cubic.p2, cubic.p3),
                        ],
                        CURVE_FLATTEN_TOLERANCE * 0.5,
                        |el| {
                            if let PathEl::LineTo(p) = el {
                                let curr = FontPoint::new(p.x, p.y);
                                if let Some(hit) = intersect_infinite_ray_with_segment(
                                    guide.position,
                                    ux,
                                    uy,
                                    prev,
                                    curr,
                                ) {
                                    raw_hits.push(hit);
                                }
                                prev = curr;
                            }
                        },
                    );
                    let _ = Line::new(cubic.p0, cubic.p3);
                }
            }
        }
    }

    raw_hits.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap_or(std::cmp::Ordering::Equal));

    let mut intersections: Vec<FontPoint> = Vec::new();
    for (_t, pt) in raw_hits {
        if intersections
            .last()
            .is_none_or(|prev| prev.distance_to(pt) > 0.5)
        {
            intersections.push(pt);
        }
    }

    let mut segments = Vec::new();
    for pair in intersections.windows(2) {
        let start = pair[0];
        let end = pair[1];
        let distance = start.distance_to(end);
        let midpoint = FontPoint::new((start.x.0 + end.x.0) * 0.5, (start.y.0 + end.y.0) * 0.5);
        segments.push(GuideMeasurementSegment {
            start,
            end,
            midpoint,
            distance,
        });
    }

    GuideMeasurementRay {
        guide_id: guide.id,
        is_global: guide.is_global,
        direction: (ux, uy),
        intersections,
        segments,
    }
}
