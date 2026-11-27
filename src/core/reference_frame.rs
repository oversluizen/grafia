//! Domain-Agnostic `CustomReferenceFrame` (`SPEC-010 §2.3`):
//! Nested 2D Bounding Boxes (`RectUpm`), $N_x \times N_y$ Subdivision Lattices,
//! Keyline Circles, and Parallel `ReferenceBandLine`s.

use crate::core::coordinates::FontPoint;
use crate::core::glyph_model::GlyphContour;
use crate::core::math_consts::{
    ICON_KEYLINE_DEFAULT_SIZE_UPM, ICON_KEYLINE_GRID_SUBDIV, ICON_KEYLINE_INNER_CIRCLE_RADIUS_FRAC,
    ICON_KEYLINE_MAX_SIZE_UPM, ICON_KEYLINE_MIN_SIZE_UPM, ICON_KEYLINE_OUTER_CIRCLE_RADIUS_FRAC,
    ICON_KEYLINE_RECT_LONG_INSET_FRAC, ICON_KEYLINE_RECT_SHORT_INSET_FRAC,
    ICON_KEYLINE_SQUARE_INSET_FRAC,
};
use crate::core::stem_gauge_and_balance::compute_contours_counter_balance;
use crate::snapping::GuideThickness;
use serde::{Deserialize, Serialize};

/// Semantic category of a magnetic snap target (`SPEC-010 §2.3`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SnapTargetKind {
    Guide,
    MetricLine,
    Grid,
    Node,
    Background,
    ItalicAngle,
    ReferenceFrame,
}

/// Axis-aligned rectangle in font-design units (`UPM`, `SPEC-010 §2.3`).
#[derive(Debug, Clone, Copy, PartialEq, Default, Serialize, Deserialize)]
pub struct RectUpm {
    pub min_x: f64,
    pub min_y: f64,
    pub max_x: f64,
    pub max_y: f64,
}

impl RectUpm {
    #[must_use]
    pub fn new(min_x: f64, min_y: f64, max_x: f64, max_y: f64) -> Self {
        debug_assert!(
            min_x.is_finite() && min_y.is_finite() && max_x.is_finite() && max_y.is_finite()
        );
        Self {
            min_x: min_x.min(max_x),
            min_y: min_y.min(max_y),
            max_x: max_x.max(min_x),
            max_y: max_y.max(min_y),
        }
    }

    #[must_use]
    pub fn width(&self) -> f64 {
        (self.max_x - self.min_x).max(0.0)
    }

    #[must_use]
    pub fn height(&self) -> f64 {
        (self.max_y - self.min_y).max(0.0)
    }

    #[must_use]
    pub fn center(&self) -> FontPoint {
        FontPoint::new(
            0.5 * (self.min_x + self.max_x),
            0.5 * (self.min_y + self.max_y),
        )
    }
}

/// Parallel reference line or zoom-scaling band (`SPEC-010 §2.3`).
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct ReferenceBandLine {
    pub y_or_x_upm: f64,
    pub horizontal: bool,
    pub thickness: GuideThickness,
    pub dashed: bool,
    pub snap_enabled: bool,
}

/// Domain-agnostic 2D reference frame (`SPEC-010 §2.3`, `SPEC-017 §2.8`).
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct CustomReferenceFrame {
    pub outer_box: Option<RectUpm>,
    pub inner_box: Option<RectUpm>,
    pub inner_grid_subdivisions: Option<(u8, u8)>,
    pub center_crosshair: bool,
    pub keyline_circles_radii_upm: Vec<f64>,
    pub parallel_lines: Vec<ReferenceBandLine>,
    #[serde(default)]
    pub show_diagonals: bool,
    #[serde(default)]
    pub keyline_rects: Vec<RectUpm>,
}

impl CustomReferenceFrame {
    /// Constructs the canonical 4-shape Apple/Material icon keyline grid (`Circle`, `Square`,
    /// `Vertical Rectangle`, `Horizontal Rectangle`, `Diagonals`, and `4x4` lattice, `SPEC-017 §2.8`).
    #[must_use]
    pub fn icon_keyline_template(size_upm: f64) -> Self {
        let s = if size_upm.is_finite() {
            size_upm.clamp(ICON_KEYLINE_MIN_SIZE_UPM, ICON_KEYLINE_MAX_SIZE_UPM)
        } else {
            ICON_KEYLINE_DEFAULT_SIZE_UPM
        };
        let sq_in = s * ICON_KEYLINE_SQUARE_INSET_FRAC;
        let long_in = s * ICON_KEYLINE_RECT_LONG_INSET_FRAC;
        let short_in = s * ICON_KEYLINE_RECT_SHORT_INSET_FRAC;
        Self {
            outer_box: Some(RectUpm::new(0.0, 0.0, s, s)),
            inner_box: Some(RectUpm::new(sq_in, sq_in, s - sq_in, s - sq_in)),
            inner_grid_subdivisions: Some((ICON_KEYLINE_GRID_SUBDIV, ICON_KEYLINE_GRID_SUBDIV)),
            center_crosshair: true,
            keyline_circles_radii_upm: vec![
                s * ICON_KEYLINE_INNER_CIRCLE_RADIUS_FRAC,
                s * ICON_KEYLINE_OUTER_CIRCLE_RADIUS_FRAC,
            ],
            parallel_lines: Vec::new(),
            show_diagonals: true,
            keyline_rects: vec![
                RectUpm::new(short_in, long_in, s - short_in, s - long_in),
                RectUpm::new(long_in, short_in, s - long_in, s - short_in),
            ],
        }
    }
}

/// Geometric bounding-box center vs. Green's-theorem area-weighted optical centroid (`SPEC-017 §2.8`).
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct IconOpticalBalanceReport {
    pub bbox_center_upm: FontPoint,
    pub optical_centroid_upm: FontPoint,
    pub delta_upm: FontPoint,
    pub total_area_upm2: f64,
}

/// Evaluates the geometric bounding-box center and area-weighted optical centroid across `contours` (`SPEC-017 §2.8`).
#[must_use]
pub fn compute_icon_optical_balance(contours: &[GlyphContour]) -> Option<IconOpticalBalanceReport> {
    let mut min_x = f64::INFINITY;
    let mut max_x = f64::NEG_INFINITY;
    let mut min_y = f64::INFINITY;
    let mut max_y = f64::NEG_INFINITY;
    for n in contours
        .iter()
        .filter(|c| c.visible)
        .flat_map(|c| c.nodes.iter())
    {
        if n.point.x.0.is_finite() && n.point.y.0.is_finite() {
            min_x = min_x.min(n.point.x.0);
            max_x = max_x.max(n.point.x.0);
            min_y = min_y.min(n.point.y.0);
            max_y = max_y.max(n.point.y.0);
        }
    }
    if !min_x.is_finite() || !min_y.is_finite() {
        return None;
    }
    let bbox_center_upm = FontPoint::new(0.5 * (min_x + max_x), 0.5 * (min_y + max_y));
    let balance = compute_contours_counter_balance(contours);
    let optical_centroid_upm = if balance.contours.is_empty()
        || !balance.overall_centroid.x.0.is_finite()
        || !balance.overall_centroid.y.0.is_finite()
    {
        bbox_center_upm
    } else {
        balance.overall_centroid
    };
    let delta_upm = FontPoint::new(
        optical_centroid_upm.x.0 - bbox_center_upm.x.0,
        optical_centroid_upm.y.0 - bbox_center_upm.y.0,
    );
    Some(IconOpticalBalanceReport {
        bbox_center_upm,
        optical_centroid_upm,
        delta_upm,
        total_area_upm2: balance.net_black_area_upm2,
    })
}

/// Resolves the geometric center of `frame` (falling back to `(0.5 * default_width, 0.5 * default_width)`).
#[must_use]
pub fn reference_frame_center_upm(frame: &CustomReferenceFrame, default_width: f64) -> FontPoint {
    debug_assert!(default_width.is_finite());
    if let Some(inner) = frame.inner_box {
        return inner.center();
    }
    if let Some(outer) = frame.outer_box {
        return outer.center();
    }
    FontPoint::new(0.5 * default_width, 0.5 * default_width)
}

/// Computes interior subdivision lines `(xs_upm, ys_upm)` for `box_rect` and `(nx, ny)`.
#[must_use]
pub fn compute_subdivision_grid_lines_upm(
    box_rect: RectUpm,
    nx: u8,
    ny: u8,
) -> (Vec<f64>, Vec<f64>) {
    let mut xs = Vec::new();
    let mut ys = Vec::new();
    if nx > 1 {
        let step_x = box_rect.width() / f64::from(nx);
        for i in 1..nx {
            xs.push(box_rect.min_x + f64::from(i) * step_x);
        }
    }
    if ny > 1 {
        let step_y = box_rect.height() / f64::from(ny);
        for j in 1..ny {
            ys.push(box_rect.min_y + f64::from(j) * step_y);
        }
    }
    (xs, ys)
}

/// Collects all vertical X-axis and horizontal Y-axis snap coordinates (`(xs_upm, ys_upm)`) from `frame`.
#[must_use]
pub fn collect_reference_frame_snap_lines(
    frame: &CustomReferenceFrame,
    default_width: f64,
) -> (Vec<f64>, Vec<f64>) {
    debug_assert!(default_width.is_finite());
    let mut xs = Vec::new();
    let mut ys = Vec::new();
    if let Some(outer) = frame.outer_box {
        xs.push(outer.min_x);
        xs.push(outer.max_x);
        ys.push(outer.min_y);
        ys.push(outer.max_y);
    }
    if let Some(inner) = frame.inner_box {
        xs.push(inner.min_x);
        xs.push(inner.max_x);
        ys.push(inner.min_y);
        ys.push(inner.max_y);
    }
    for kr in &frame.keyline_rects {
        if kr.min_x.is_finite()
            && kr.max_x.is_finite()
            && kr.min_y.is_finite()
            && kr.max_y.is_finite()
        {
            xs.push(kr.min_x);
            xs.push(kr.max_x);
            ys.push(kr.min_y);
            ys.push(kr.max_y);
        }
    }
    if let Some((nx, ny)) = frame.inner_grid_subdivisions {
        if let Some(target_box) = frame.inner_box.or(frame.outer_box) {
            let (sub_xs, sub_ys) = compute_subdivision_grid_lines_upm(target_box, nx, ny);
            xs.extend(sub_xs);
            ys.extend(sub_ys);
        }
    }
    let center = reference_frame_center_upm(frame, default_width);
    if frame.center_crosshair {
        xs.push(center.x.0);
        ys.push(center.y.0);
    }
    for &r in &frame.keyline_circles_radii_upm {
        if r.is_finite() && r > 0.0 {
            xs.push(center.x.0 - r);
            xs.push(center.x.0 + r);
            ys.push(center.y.0 - r);
            ys.push(center.y.0 + r);
        }
    }
    for line in &frame.parallel_lines {
        if !line.snap_enabled || !line.y_or_x_upm.is_finite() {
            continue;
        }
        let target = if line.horizontal { &mut ys } else { &mut xs };
        target.push(line.y_or_x_upm);
        if let GuideThickness::FontUnits(w) = line.thickness {
            if w.is_finite() && w > 0.0 {
                let half = 0.5 * w;
                target.push(line.y_or_x_upm - half);
                target.push(line.y_or_x_upm + half);
            }
        }
    }
    (xs, ys)
}
