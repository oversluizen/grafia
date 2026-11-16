//! `SPEC-009 §2.3` On-canvas 1D ray-projected parametric `CanvasSmartHandle` geometry,
//! `[min_val, max_val]` clamping, `4.0 dp` default-value snapping, and hit-testing.

use crate::core::canvas_entities::CanvasSmartHandle;
use crate::core::math_consts::{
    HOBBY_MIN_CHORD_EPS, SMART_HANDLE_DEFAULT_SNAP_DP, SMART_HANDLE_HIT_RADIUS_DP,
    SMART_HANDLE_MIN_SCALE_EPS,
};
use crate::core::{CanvasComponent, FontPoint, ScreenPoint, ViewportTransform};

fn unit_dir(dir: (f64, f64)) -> (f64, f64) {
    let len = dir.0.hypot(dir.1);
    if len <= HOBBY_MIN_CHORD_EPS {
        (1.0, 0.0)
    } else {
        (dir.0 / len, dir.1 / len)
    }
}

fn value_to_point(handle: &CanvasSmartHandle, val: f64) -> FontPoint {
    let u = unit_dir(handle.direction_unit);
    let dist_upm = (val - handle.min_val) * handle.scale_upm_per_unit;
    FontPoint::new(
        handle.anchor_upm.x.0 + dist_upm * u.0,
        handle.anchor_upm.y.0 + dist_upm * u.1,
    )
}

#[must_use]
pub fn smart_handle_upm_position(handle: &CanvasSmartHandle) -> FontPoint {
    debug_assert!(handle.min_val.is_finite() && handle.max_val.is_finite());
    let lo = handle.min_val.min(handle.max_val);
    let hi = handle.min_val.max(handle.max_val);
    value_to_point(handle, handle.current_val.clamp(lo, hi))
}

#[must_use]
pub fn smart_handle_track_endpoints_upm(
    handle: &CanvasSmartHandle,
) -> (FontPoint, FontPoint, FontPoint) {
    let lo = handle.min_val.min(handle.max_val);
    let hi = handle.min_val.max(handle.max_val);
    (
        value_to_point(handle, lo),
        value_to_point(handle, handle.default_val.clamp(lo, hi)),
        value_to_point(handle, hi),
    )
}

#[must_use]
pub fn project_pointer_onto_smart_handle(
    handle: &CanvasSmartHandle,
    font_pt: FontPoint,
    viewport: &ViewportTransform,
) -> f64 {
    let lo = handle.min_val.min(handle.max_val);
    let hi = handle.min_val.max(handle.max_val);
    if handle.scale_upm_per_unit.abs() <= SMART_HANDLE_MIN_SCALE_EPS {
        return handle.current_val.clamp(lo, hi);
    }
    let u = unit_dir(handle.direction_unit);
    let dx = font_pt.x.0 - handle.anchor_upm.x.0;
    let dy = font_pt.y.0 - handle.anchor_upm.y.0;
    let s_upm = dx * u.0 + dy * u.1;
    let raw_val = handle.min_val + s_upm / handle.scale_upm_per_unit;
    let clamped = raw_val.clamp(lo, hi);
    let def_clamped = handle.default_val.clamp(lo, hi);
    let dist_to_def_dp =
        (clamped - def_clamped).abs() * handle.scale_upm_per_unit.abs() * f64::from(viewport.zoom);
    if dist_to_def_dp <= f64::from(SMART_HANDLE_DEFAULT_SNAP_DP) {
        def_clamped
    } else {
        clamped
    }
}

#[must_use]
pub fn hit_test_smart_handles(
    handles: &[CanvasSmartHandle],
    components: &[CanvasComponent],
    viewport: &ViewportTransform,
    cursor_scr: ScreenPoint,
) -> Option<usize> {
    hit_test_smart_handles_with_radius(
        handles,
        components,
        viewport,
        cursor_scr,
        SMART_HANDLE_HIT_RADIUS_DP,
    )
}

#[must_use]
pub fn hit_test_smart_handles_with_radius(
    handles: &[CanvasSmartHandle],
    components: &[CanvasComponent],
    viewport: &ViewportTransform,
    cursor_scr: ScreenPoint,
    hit_radius_dp: f32,
) -> Option<usize> {
    let mut best: Option<(f32, usize)> = None;
    for (idx, h) in handles.iter().enumerate() {
        let is_active_comp = components
            .iter()
            .any(|c| c.id == h.component_id && c.selected && !c.locked);
        if !is_active_comp {
            continue;
        }
        let scr = viewport.to_screen(smart_handle_upm_position(h));
        let d = scr.distance_to(cursor_scr);
        if d <= hit_radius_dp && best.is_none_or(|(bd, _)| d < bd) {
            best = Some((d, idx));
        }
    }
    best.map(|(_, idx)| idx)
}
