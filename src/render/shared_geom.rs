//! Shared 2D screen-space geometry projections for `egui` and `tiny-skia` passes (`SPEC-010`).

use crate::core::math_consts::{
    GRADIENT_ENDPOINT_RADIUS_DP, GRADIENT_LINE_STROKE_WIDTH_DP, GRADIENT_STOP_SELECTED_RADIUS_DP,
    GRADIENT_STOP_TICK_RADIUS_DP, REF_FRAME_CROSSHAIR_HALF_DP, REF_FRAME_DASH_LEN_DP,
    REF_FRAME_GAP_LEN_DP,
};
use crate::core::{
    compute_subdivision_grid_lines_upm, gradient_control_segment_upm, gradient_stop_position_upm,
    gradient_stops_slice, reference_frame_center_upm, ColorLayerPaint, CustomReferenceFrame,
    FontPoint, RectUpm, ScreenPoint, ViewportTransform,
};
use crate::snapping::GuideThickness;
use egui::{Pos2, Rect};

/// Snaps a screen-space coordinate to the nearest pixel center (`.5`) for crisp `1.0 dp` strokes.
#[must_use]
pub fn snap_pixel_center(v: f32) -> f32 {
    (v - 0.5).round() + 0.5
}

/// Projected screen-space rectangle `[min_x, min_y, max_x, max_y]`.
#[must_use]
pub fn project_rect_upm_to_screen(rect_upm: RectUpm, vp: &ViewportTransform) -> Rect {
    let p0 = vp.to_screen(FontPoint::new(rect_upm.min_x, rect_upm.max_y));
    let p1 = vp.to_screen(FontPoint::new(rect_upm.max_x, rect_upm.min_y));
    Rect::from_two_pos(
        Pos2::new(
            snap_pixel_center(p0.x.0.min(p1.x.0)),
            snap_pixel_center(p0.y.0.min(p1.y.0)),
        ),
        Pos2::new(
            snap_pixel_center(p0.x.0.max(p1.x.0)),
            snap_pixel_center(p0.y.0.max(p1.y.0)),
        ),
    )
}

/// Projects interior $N_x \times N_y$ subdivision segments inside `box_upm` to screen space.
#[must_use]
pub fn project_subdivision_segments_screen(
    box_upm: RectUpm,
    nx: u8,
    ny: u8,
    vp: &ViewportTransform,
) -> Vec<(ScreenPoint, ScreenPoint)> {
    let (xs, ys) = compute_subdivision_grid_lines_upm(box_upm, nx, ny);
    let mut segs = Vec::with_capacity(xs.len() + ys.len());
    for x in xs {
        let a = vp.to_screen(FontPoint::new(x, box_upm.min_y));
        let b = vp.to_screen(FontPoint::new(x, box_upm.max_y));
        let sx = snap_pixel_center(a.x.0);
        segs.push((
            ScreenPoint::new(sx, snap_pixel_center(a.y.0)),
            ScreenPoint::new(sx, snap_pixel_center(b.y.0)),
        ));
    }
    for y in ys {
        let a = vp.to_screen(FontPoint::new(box_upm.min_x, y));
        let b = vp.to_screen(FontPoint::new(box_upm.max_x, y));
        let sy = snap_pixel_center(a.y.0);
        segs.push((
            ScreenPoint::new(snap_pixel_center(a.x.0), sy),
            ScreenPoint::new(snap_pixel_center(b.x.0), sy),
        ));
    }
    segs
}

/// Projects keyline circles `(center_scr, radius_px)` for `CustomReferenceFrame`.
#[must_use]
pub fn project_keyline_circles_screen(
    frame: &CustomReferenceFrame,
    default_width_upm: f64,
    vp: &ViewportTransform,
) -> Vec<(ScreenPoint, f32)> {
    let center_upm = reference_frame_center_upm(frame, default_width_upm);
    let center_scr = vp.to_screen(center_upm);
    let mut out = Vec::new();
    for &r_upm in &frame.keyline_circles_radii_upm {
        if r_upm.is_finite() && r_upm > 0.0 {
            let r_px = (r_upm as f32) * vp.zoom;
            if r_px > 0.5 {
                out.push((center_scr, r_px));
            }
        }
    }
    out
}

/// Resolves the span `[min_coord, max_coord]` in screen space for parallel reference lines.
#[must_use]
pub fn resolve_reference_frame_span_screen(
    frame: &CustomReferenceFrame,
    vp: &ViewportTransform,
    canvas_rect: Rect,
) -> (f32, f32, f32, f32) {
    if let Some(outer) = frame.outer_box.or(frame.inner_box) {
        let r = project_rect_upm_to_screen(outer, vp);
        (r.left(), r.top(), r.right(), r.bottom())
    } else {
        (
            canvas_rect.left(),
            canvas_rect.top(),
            canvas_rect.right(),
            canvas_rect.bottom(),
        )
    }
}

/// Returns `(band_thickness_px, stroke_width_dp)` for a `GuideThickness` at `zoom`.
#[must_use]
pub fn resolve_band_thickness_px(
    thickness: GuideThickness,
    zoom: f32,
    base_dp: f32,
) -> (Option<f32>, f32) {
    match thickness {
        GuideThickness::ScreenDp => (None, base_dp),
        GuideThickness::FontUnits(w) => {
            if w.is_finite() && w > 0.0 {
                Some(((w as f32) * zoom).max(1.0))
                    .map(|px| (Some(px), base_dp))
                    .unwrap_or((None, base_dp))
            } else {
                (None, base_dp)
            }
        }
    }
}

/// Projected gradient handle overlay item in screen space.
#[derive(Debug, Clone)]
pub struct ProjectedGradientHandles {
    pub start_scr: ScreenPoint,
    pub end_scr: ScreenPoint,
    pub radial_r0_scr: Option<(ScreenPoint, f32)>,
    pub radial_r1_scr: Option<(ScreenPoint, f32)>,
    pub stop_ticks: Vec<(ScreenPoint, [u8; 4], bool)>,
}

/// Projects interactive gradient handles and stop ticks for a selected sublayer.
#[must_use]
pub fn project_gradient_handles_screen(
    paint: &ColorLayerPaint,
    selected_stop_idx: Option<usize>,
    vp: &ViewportTransform,
) -> Option<ProjectedGradientHandles> {
    let (p0, p1) = gradient_control_segment_upm(paint)?;
    let stops = gradient_stops_slice(paint)?;
    let start_scr = vp.to_screen(p0);
    let end_scr = vp.to_screen(p1);
    let (radial_r0_scr, radial_r1_scr) = match paint {
        ColorLayerPaint::RadialGradient { c0, r0, c1, r1, .. } => {
            let c0_scr = vp.to_screen(*c0);
            let c1_scr = vp.to_screen(*c1);
            let r0_px = ((*r0 as f32) * vp.zoom).max(0.0);
            let r1_px = ((*r1 as f32) * vp.zoom).max(1.0);
            (Some((c0_scr, r0_px)), Some((c1_scr, r1_px)))
        }
        _ => (None, None),
    };
    let mut stop_ticks = Vec::with_capacity(stops.len());
    for (idx, stop) in stops.iter().enumerate() {
        if let Some(pt) = gradient_stop_position_upm(paint, idx) {
            stop_ticks.push((vp.to_screen(pt), stop.rgba, selected_stop_idx == Some(idx)));
        }
    }
    Some(ProjectedGradientHandles {
        start_scr,
        end_scr,
        radial_r0_scr,
        radial_r1_scr,
        stop_ticks,
    })
}

#[must_use]
pub const fn ref_frame_dash_gap_dp() -> (f32, f32) {
    (REF_FRAME_DASH_LEN_DP, REF_FRAME_GAP_LEN_DP)
}

#[must_use]
pub const fn ref_frame_crosshair_half_dp() -> f32 {
    REF_FRAME_CROSSHAIR_HALF_DP
}

#[must_use]
pub const fn gradient_overlay_radii_dp() -> (f32, f32, f32, f32) {
    (
        GRADIENT_ENDPOINT_RADIUS_DP,
        GRADIENT_STOP_TICK_RADIUS_DP,
        GRADIENT_STOP_SELECTED_RADIUS_DP,
        GRADIENT_LINE_STROKE_WIDTH_DP,
    )
}
