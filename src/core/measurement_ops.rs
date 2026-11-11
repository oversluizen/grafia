//! `CanvasTool` (`L` & `Cmd+Ctrl+Opt` `spec_ref`) coordinate labels, italic-compensated
//! segment deltas, and ray/sidebearing intersections (`SPEC-005 §2.3`).
//!
//! Extracted 1-to-1 from `CanvasTool` (`spec_ref..spec_ref`) in Reference Vector (``).

use crate::core::knife_ops::{compute_knife_intersections, intersect_finite_segments};
use crate::core::{extract_segments, ContourSegment, FontPoint, Node, NodeType};
use crate::snapping::Guide;
use crate::tokens::{
    GUIDE_PERPENDICULAR_DEG, KNIFE_PARAM_DEDUP_EPS, MEASUREMENT_DEC_ROUND_FACTOR,
    MEASUREMENT_POINT_DEDUP_UPM, MEASUREMENT_SIDEBEARING_EXTENT_UPM,
};
use kurbo::ParamCurve;

/// Precomputed coordinate and segment-delta labels for `drawLocationsForLayer:origin:scale:shouldDrawDark:` (`spec_ref`).
#[derive(Debug, Clone, Default, PartialEq)]
pub struct MeasurementLayerLabels {
    /// Blue `"x, y"` coordinate labels for `OnCurve` nodes (SPEC-005).
    pub oncurve_labels: Vec<(FontPoint, String)>,
    /// Green `"x, y"` coordinate labels for `OffCurve` BCP handles (SPEC-005).
    pub offcurve_labels: Vec<(FontPoint, String)>,
    /// Red `"Δx, Δy"` italic-compensated segment midpoint delta labels (`spec_ref..spec_ref`).
    pub segment_deltas: Vec<(FontPoint, String)>,
    /// Guide angle labels `"θ°"` drawn when `show_guides` is enabled (`spec_ref`).
    pub guide_angles: Vec<(FontPoint, String)>,
}

/// Formats a floating-point typographic measurement value to at most 1 decimal place (`drawMonospacedDigitBadgeAtPoint:`).
#[must_use]
pub fn format_measurement_value(val: f64) -> String {
    let rounded = (val * MEASUREMENT_DEC_ROUND_FACTOR).round() / MEASUREMENT_DEC_ROUND_FACTOR;
    if (rounded - rounded.round()).abs() <= KNIFE_PARAM_DEDUP_EPS {
        format!("{}", rounded.round() as i64)
    } else {
        format!("{rounded:.1}")
    }
}

/// Computes all coordinate and segment delta labels for the active layer
/// (`CanvasTool::drawLocationsForLayer:origin:scale:shouldDrawDark:` at `spec_ref`).
#[must_use]
pub fn compute_layer_measurement_labels(
    contours: &[Vec<Node>],
    closed_flags: &[bool],
    italic_angle_deg: f64,
    guides: &[Guide],
    global_guides: &[Guide],
    show_guides: bool,
) -> MeasurementLayerLabels {
    let mut out = MeasurementLayerLabels::default();
    let tan_italic = italic_angle_deg.to_radians().tan();

    for (c_idx, contour) in contours.iter().enumerate() {
        for node in contour {
            let label = format!(
                "{}, {}",
                node.point.x.0.round() as i64,
                node.point.y.0.round() as i64
            );
            if node.node_type == NodeType::OffCurve {
                out.offcurve_labels.push((node.point, label));
            } else {
                out.oncurve_labels.push((node.point, label));
            }
        }

        let closed = closed_flags.get(c_idx).copied().unwrap_or(true);
        for seg in extract_segments(contour, closed) {
            let (p0, p1, mid) = match seg {
                ContourSegment::Line { line, .. } => {
                    let m = line.eval(0.5);
                    (
                        FontPoint::new(line.p0.x, line.p0.y),
                        FontPoint::new(line.p1.x, line.p1.y),
                        FontPoint::new(m.x, m.y),
                    )
                }
                ContourSegment::Cubic { cubic, .. } => {
                    let m = cubic.eval(0.5);
                    (
                        FontPoint::new(cubic.p0.x, cubic.p0.y),
                        FontPoint::new(cubic.p3.x, cubic.p3.y),
                        FontPoint::new(m.x, m.y),
                    )
                }
            };
            // `spec_ref..spec_ref`: `dx_italic = (p1.x - p0.x) - (p1.y - p0.y) * tan(italicAngle)`
            let raw_dx = p1.x.0 - p0.x.0;
            let raw_dy = p1.y.0 - p0.y.0;
            let dx_italic = (raw_dx - raw_dy * tan_italic).round() as i64;
            let dy_round = raw_dy.round() as i64;
            out.segment_deltas
                .push((mid, format!("{dx_italic}, {dy_round}")));
        }
    }

    if show_guides {
        for g in guides.iter().chain(global_guides.iter()) {
            out.guide_angles.push((
                g.position,
                format!("{}°", format_measurement_value(g.angle_deg)),
            ));
        }
    }

    out
}

/// Computes all sorted intersections along the measurement drag segment `(start, end)`,
/// including contour intersections AND Left/Right Sidebearing intersections (`calculateIntersectionsForDrag` `spec_ref..spec_ref`).
#[must_use]
pub fn compute_measurement_ray_intersections(
    contours: &[Vec<Node>],
    closed_flags: &[bool],
    advance_width: f64,
    italic_angle_deg: f64,
    slant_origin_y: f64,
    start: FontPoint,
    end: FontPoint,
) -> Vec<FontPoint> {
    let dx = end.x.0 - start.x.0;
    let dy = end.y.0 - start.y.0;
    let len_sq = dx * dx + dy * dy;
    if len_sq <= KNIFE_PARAM_DEDUP_EPS {
        return vec![start];
    }

    let mut points = Vec::new();
    points.push(start);
    points.push(end);
    points.extend(compute_knife_intersections(contours, closed_flags, start, end));

    // Left sidebearing (x = 0) and Right sidebearing (x = advance_width), sheared by `italic_angle_deg` around `slant_origin_y` (`spec_ref..spec_ref`)
    let tan_italic = italic_angle_deg.to_radians().tan();
    let y_low = -MEASUREMENT_SIDEBEARING_EXTENT_UPM;
    let y_high = MEASUREMENT_SIDEBEARING_EXTENT_UPM;
    for base_x in [0.0, advance_width] {
        let sb_p0 = FontPoint::new(base_x + (y_low - slant_origin_y) * tan_italic, y_low);
        let sb_p1 = FontPoint::new(base_x + (y_high - slant_origin_y) * tan_italic, y_high);
        if let Some((_u, _v, hit_pt)) = intersect_finite_segments(start, end, sb_p0, sb_p1) {
            points.push(hit_pt);
        }
    }

    // Sort along the ray direction `start -> end` (`_GSSortPoint`)
    points.sort_by(|a, b| {
        let ta = ((a.x.0 - start.x.0) * dx + (a.y.0 - start.y.0) * dy) / len_sq;
        let tb = ((b.x.0 - start.x.0) * dx + (b.y.0 - start.y.0) * dy) / len_sq;
        ta.partial_cmp(&tb).unwrap_or(std::cmp::Ordering::Equal)
    });

    // Deduplicate near-coincident points (`spec_ref..spec_ref`)
    let mut deduped: Vec<FontPoint> = Vec::with_capacity(points.len());
    for pt in points {
        if deduped.last().is_none_or(|prev| {
            (pt.x.0 - prev.x.0).hypot(pt.y.0 - prev.y.0) > MEASUREMENT_POINT_DEDUP_UPM
        }) {
            deduped.push(pt);
        }
    }
    deduped
}

/// Computes the Vector Canvas typographic angle in degrees (`spec_ref..spec_ref` in `drawMeasurementLine:scale:`),
/// where vertical upward is `0.0°` and counter-clockwise is positive, normalized via `asin(sin(rad))`.
#[must_use]
pub fn compute_measurement_typographic_angle_deg(start: FontPoint, end: FontPoint) -> f64 {
    let dx = end.x.0 - start.x.0;
    let dy = end.y.0 - start.y.0;
    if dx.hypot(dy) <= KNIFE_PARAM_DEDUP_EPS {
        return 0.0;
    }
    let math_deg = dy.atan2(dx).to_degrees();
    let shifted_rad = (math_deg - GUIDE_PERPENDICULAR_DEG).to_radians();
    shifted_rad.sin().asin().to_degrees()
}
