//! `SPEC-008 §2.4 & §2.5`: 8-Step Fit Curve (`Ctrl+Opt+1..8`, `57%..92%`),
//! Handle Normalization, and Live Contour Stroke data types (`ContourStrokeStyle`).

use crate::core::bezier_ops::NodeId;
use crate::core::coordinates::FontPoint;
use crate::core::curve_manipulation::{extract_segments, ContourSegment};
use crate::core::glyph_model::GlyphContour;
pub use crate::core::stroke_expand::{
    expand_single_contour_stroke, expand_stroked_contours_preview,
};
use crate::tokens::{
    DEFAULT_CONTOUR_STROKE_WIDTH_UPM, FIT_CURVE_CONFIG_MAX_MIN_BOUND, FIT_CURVE_CONFIG_MIN_BOUND,
    FIT_CURVE_DEFAULT_MAX_RATIO, FIT_CURVE_DEFAULT_MIN_RATIO, FIT_CURVE_MAX_ALLOWED_RATIO,
    FIT_CURVE_MAX_COLUMN_INDEX, FIT_CURVE_MAX_INTERSECTION_CHORD_MULTIPLE,
    FIT_CURVE_MIN_ALLOWED_RATIO, FIT_CURVE_MIN_CHORD_LEN_EPS, FIT_CURVE_MIN_SPAN_RATIO,
    FIT_CURVE_PARALLEL_DET_EPS, FIT_CURVE_STEP_DELTA_FRACTION, FIT_CURVE_STEP_DIVISOR,
};
use serde::{Deserialize, Serialize};

/// 8-step Fit Curve configuration (`57%..92%` by default, `SPEC-008 §2.4.1`).
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct FitCurveConfig {
    pub min_ratio: f64,
    pub max_ratio: f64,
}

impl Default for FitCurveConfig {
    fn default() -> Self {
        Self {
            min_ratio: FIT_CURVE_DEFAULT_MIN_RATIO,
            max_ratio: FIT_CURVE_DEFAULT_MAX_RATIO,
        }
    }
}

impl FitCurveConfig {
    /// Returns a validated copy enforcing $0.0 \le \text{min\_ratio} \le 1.0$, $\text{max\_ratio} \le 1.5$,
    /// and $\text{max\_ratio} \ge \text{min\_ratio} + 0.125$ (`SPEC-008 §2.4.1`).
    #[must_use]
    pub fn sanitized(&self) -> Self {
        let min_r = self
            .min_ratio
            .clamp(FIT_CURVE_CONFIG_MIN_BOUND, FIT_CURVE_CONFIG_MAX_MIN_BOUND);
        let max_r = self.max_ratio.clamp(
            min_r + FIT_CURVE_MIN_SPAN_RATIO,
            FIT_CURVE_MAX_ALLOWED_RATIO,
        );
        Self {
            min_ratio: min_r,
            max_ratio: max_r,
        }
    }

    /// Returns the target handle ratio for column `col` (`0..=7`) on the 8-step Fit Curve ladder:
    /// $r(\text{col}) = \text{min\_ratio} + (\text{max\_ratio} - \text{min\_ratio}) \cdot \frac{\min(\text{col}, 7)}{7}$.
    #[must_use]
    pub fn ratio_for_column(&self, col: usize) -> f64 {
        let cfg = self.sanitized();
        let c = col.min(FIT_CURVE_MAX_COLUMN_INDEX) as f64;
        cfg.min_ratio + (cfg.max_ratio - cfg.min_ratio) * (c / FIT_CURVE_STEP_DIVISOR)
    }

    /// Returns the relative step delta $\pm 0.125 \cdot \max(\text{max\_ratio} - \text{min\_ratio}, 0.125)$.
    #[must_use]
    pub fn step_delta(&self, longer: bool) -> f64 {
        let cfg = self.sanitized();
        let span = (cfg.max_ratio - cfg.min_ratio).max(FIT_CURVE_MIN_SPAN_RATIO);
        let mag = FIT_CURVE_STEP_DELTA_FRACTION * span;
        if longer {
            mag
        } else {
            -mag
        }
    }
}

/// Stroke alignment relative to the spine contour (`SPEC-008 §2.5.1`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum StrokePosition {
    Center,
    InsideLeft,
    OutsideRight,
}

/// End-cap style for open stroked contours (`SPEC-008 §2.5.1`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum LineCapStyle {
    Butt,
    Round,
    Square,
}

/// Corner join style for stroked contours (`SPEC-008 §2.5.1`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum LineJoinStyle {
    Miter,
    Round,
    Bevel,
}

/// Non-destructive live stroke parameters attached to a `ContourId` (`SPEC-008 §2.5.1`).
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct ContourStrokeStyle {
    pub width: f64,
    pub height: f64,
    pub position: StrokePosition,
    pub cap_start: LineCapStyle,
    pub cap_end: LineCapStyle,
    pub join: LineJoinStyle,
}

impl Default for ContourStrokeStyle {
    fn default() -> Self {
        Self {
            width: DEFAULT_CONTOUR_STROKE_WIDTH_UPM,
            height: DEFAULT_CONTOUR_STROKE_WIDTH_UPM,
            position: StrokePosition::Center,
            cap_start: LineCapStyle::Butt,
            cap_end: LineCapStyle::Butt,
            join: LineJoinStyle::Miter,
        }
    }
}

/// Computes the ray-ray intersection $T$ of outgoing tangent rays from $P_0 \to P_1$ and $P_3 \to P_2$,
/// returning `Some((T, t1, t2))` when both rays intersect forward ($u > 0, w > 0$) and satisfy
/// the chord multiple bound $\|T - P_0\| < 2 \|P_3 - P_0\|$ (`SPEC-008 §2.4.2`).
#[must_use]
pub fn solve_cubic_tangent_triangle(
    p0: FontPoint,
    p1: FontPoint,
    p2: FontPoint,
    p3: FontPoint,
) -> Option<(FontPoint, f64, f64)> {
    let chord_len = p0.distance_to(p3);
    if chord_len <= FIT_CURVE_MIN_CHORD_LEN_EPS {
        return None;
    }
    let v1x = p1.x.0 - p0.x.0;
    let v1y = p1.y.0 - p0.y.0;
    let v2x = p2.x.0 - p3.x.0;
    let v2y = p2.y.0 - p3.y.0;
    let det = v1y * v2x - v1x * v2y;
    if det.abs() <= FIT_CURVE_PARALLEL_DET_EPS {
        return None;
    }
    let dx = p3.x.0 - p0.x.0;
    let dy = p3.y.0 - p0.y.0;
    let u = (dy * v2x - dx * v2y) / det;
    let w = (v1x * dy - v1y * dx) / det;
    if u <= 0.0 || w <= 0.0 {
        return None;
    }
    let t_pt = FontPoint::new(p0.x.0 + u * v1x, p0.y.0 + u * v1y);
    let max_leg = FIT_CURVE_MAX_INTERSECTION_CHORD_MULTIPLE * chord_len;
    if p0.distance_to(t_pt) >= max_leg || p3.distance_to(t_pt) >= max_leg {
        return None;
    }
    Some((t_pt, 1.0 / u, 1.0 / w))
}

/// Applies a uniform Fit Curve ratio $r$ to all selected cubic segments in `contours` (`SPEC-008 §2.4.2`).
pub fn apply_fit_curve_ratio_to_contours(
    contours: &mut [GlyphContour],
    selected_nodes: &[NodeId],
    ratio: f64,
) -> bool {
    let r = ratio.clamp(FIT_CURVE_MIN_ALLOWED_RATIO, FIT_CURVE_MAX_ALLOWED_RATIO);
    mutate_selected_cubic_handles(contours, selected_nodes, |_, _| (r, r))
}

/// Steps all selected cubic segment handle ratios by `delta`, clamped to `[min_ratio, max_ratio]` (`SPEC-008 §2.4.3`).
pub fn apply_fit_curve_delta_to_contours(
    contours: &mut [GlyphContour],
    selected_nodes: &[NodeId],
    delta: f64,
    min_ratio: f64,
    max_ratio: f64,
) -> bool {
    let lo = min_ratio.clamp(FIT_CURVE_MIN_ALLOWED_RATIO, FIT_CURVE_MAX_ALLOWED_RATIO);
    let hi = max_ratio.clamp(lo, FIT_CURVE_MAX_ALLOWED_RATIO);
    mutate_selected_cubic_handles(contours, selected_nodes, |t1, t2| {
        ((t1 + delta).clamp(lo, hi), (t2 + delta).clamp(lo, hi))
    })
}

/// Normalizes each selected cubic segment's handles to their arithmetic mean ratio $\bar{t} = 0.5(t_1 + t_2)$ (`SPEC-008 §2.4.3`).
pub fn normalize_cubic_handles_in_contours(
    contours: &mut [GlyphContour],
    selected_nodes: &[NodeId],
) -> bool {
    mutate_selected_cubic_handles(contours, selected_nodes, |t1, t2| {
        let avg = (0.5 * (t1 + t2)).clamp(FIT_CURVE_MIN_ALLOWED_RATIO, FIT_CURVE_MAX_ALLOWED_RATIO);
        (avg, avg)
    })
}

fn mutate_selected_cubic_handles<F>(
    contours: &mut [GlyphContour],
    selected_nodes: &[NodeId],
    mut compute_ratios: F,
) -> bool
where
    F: FnMut(f64, f64) -> (f64, f64),
{
    if selected_nodes.is_empty() {
        return false;
    }
    let mut changed = false;
    for contour in contours.iter_mut() {
        let segments = extract_segments(&contour.nodes, contour.closed);
        for seg in segments {
            if let ContourSegment::Cubic {
                start_idx,
                h1_idx,
                h2_idx,
                end_idx,
                ..
            } = seg
            {
                let ids = [
                    contour.nodes[start_idx].id,
                    contour.nodes[h1_idx].id,
                    contour.nodes[h2_idx].id,
                    contour.nodes[end_idx].id,
                ];
                if !ids.iter().any(|id| selected_nodes.contains(id)) {
                    continue;
                }
                let p0 = contour.nodes[start_idx].point;
                let p1 = contour.nodes[h1_idx].point;
                let p2 = contour.nodes[h2_idx].point;
                let p3 = contour.nodes[end_idx].point;
                if let Some((t_pt, t1, t2)) = solve_cubic_tangent_triangle(p0, p1, p2, p3) {
                    let (r1, r2) = compute_ratios(t1, t2);
                    contour.nodes[h1_idx].point = FontPoint::new(
                        p0.x.0 + r1 * (t_pt.x.0 - p0.x.0),
                        p0.y.0 + r1 * (t_pt.y.0 - p0.y.0),
                    );
                    contour.nodes[h2_idx].point = FontPoint::new(
                        p3.x.0 + r2 * (t_pt.x.0 - p3.x.0),
                        p3.y.0 + r2 * (t_pt.y.0 - p3.y.0),
                    );
                    changed = true;
                }
            }
        }
    }
    changed
}
