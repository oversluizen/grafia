//! Multi-Layer Vector Color, Gradients (`Linear`, `Radial`, `Sweep`), Bitmaps & `ClippingMask`
//! (`SPEC-010 §2.1`, `ISO/IEC 14496-22:2019` `COLR v1` & Porter-Duff `SrcOver`).

use crate::core::canvas_entities::CanvasBackgroundImage;
use crate::core::coordinates::{FontPoint, ScreenPoint, ViewportTransform};
use crate::core::glyph_model::GlyphContour;
use crate::core::math_consts::{
    ALPHA_MAX_F32, ALPHA_MIN_F32, BYTE_MAX_F64, FULL_CIRCLE_DEG, GRADIENT_HANDLE_HIT_RADIUS_DP,
    GRADIENT_MIN_SPAN_EPS, GRADIENT_STOP_MAX, GRADIENT_STOP_MIN, SWEEP_DEFAULT_HANDLE_LEN_UPM,
};
use serde::{Deserialize, Serialize};

/// Gradient spread / extend mode outside `[0.0, 1.0]` (`ISO/IEC 14496-22` `Extend`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum GradientExtend {
    #[default]
    Pad,
    Repeat,
    Reflect,
}

/// Alias matching `SPEC-010 §2.1` (`GradientSpread`).
pub type GradientSpread = GradientExtend;

/// Color stop along a gradient line (`offset` in `[0.0, 1.0]`, unmultiplied `rgba`).
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct GradientStop {
    pub offset: f64,
    pub rgba: [u8; 4],
}

impl GradientStop {
    #[must_use]
    pub fn new(offset: f64, rgba: [u8; 4]) -> Self {
        debug_assert!(offset.is_finite(), "GradientStop offset must be finite");
        Self {
            offset: offset.clamp(GRADIENT_STOP_MIN, GRADIENT_STOP_MAX),
            rgba,
        }
    }
}

/// Paint shader specification for a `CanvasColorSublayer` (`SPEC-010 §2.1`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum ColorLayerPaint {
    Solid {
        rgba: [u8; 4],
    },
    LinearGradient {
        p0: FontPoint,
        p1: FontPoint,
        stops: Vec<GradientStop>,
        spread: GradientSpread,
    },
    RadialGradient {
        c0: FontPoint,
        r0: f64,
        c1: FontPoint,
        r1: f64,
        stops: Vec<GradientStop>,
        spread: GradientSpread,
    },
    SweepGradient {
        center: FontPoint,
        start_deg: f64,
        end_deg: f64,
        stops: Vec<GradientStop>,
    },
    Bitmap {
        image: CanvasBackgroundImage,
        ppem_strike: Option<u16>,
    },
    ClippingMask,
}

/// Single color sublayer in `GrafiaCanvasState::color_sublayers` (`SPEC-010 §2.1`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CanvasColorSublayer {
    pub id: u64,
    pub name: String,
    pub contours: Vec<GlyphContour>,
    pub paint: ColorLayerPaint,
    pub opacity: f32,
    pub visible: bool,
    pub locked: bool,
}

/// Host notification payload when a color sublayer is mutated on canvas (`SPEC-010 §2.1`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum ColorSublayerMutation {
    UpdatePaint(ColorLayerPaint),
    SetVisible(bool),
    SetLocked(bool),
    SetOpacity(f32),
}

/// Interactive handle kind on a selected gradient sublayer (`SPEC-010 §2.1`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GradientHandleKind {
    StartEndpoint,
    EndEndpoint,
    RadialInnerRadius,
    StopTick(usize),
}

/// Applies `Pad`, `Repeat`, or `Reflect` spread mapping to raw parameter `t`.
#[must_use]
pub fn apply_gradient_spread(t: f64, spread: GradientSpread) -> f64 {
    debug_assert!(t.is_finite(), "gradient parameter t must be finite");
    if !t.is_finite() {
        return GRADIENT_STOP_MIN;
    }
    match spread {
        GradientExtend::Pad => t.clamp(GRADIENT_STOP_MIN, GRADIENT_STOP_MAX),
        GradientExtend::Repeat => t.rem_euclid(GRADIENT_STOP_MAX),
        GradientExtend::Reflect => {
            let p = t.rem_euclid(2.0);
            p.min(2.0 - p)
        }
    }
}

/// Linearly interpolates `stops` at normalized parameter `t` in `[0.0, 1.0]`.
#[must_use]
pub fn sample_gradient_stops(stops: &[GradientStop], t: f64) -> [u8; 4] {
    debug_assert!(t.is_finite(), "sample_gradient_stops requires finite t");
    if stops.is_empty() {
        return [0, 0, 0, 0];
    }
    let clamped = t.clamp(GRADIENT_STOP_MIN, GRADIENT_STOP_MAX);
    if stops.len() == 1 || clamped <= stops[0].offset {
        return stops[0].rgba;
    }
    let last = stops[stops.len() - 1];
    if clamped >= last.offset {
        return last.rgba;
    }
    for pair in stops.windows(2) {
        let (s0, s1) = (pair[0], pair[1]);
        if clamped >= s0.offset && clamped <= s1.offset {
            let span = (s1.offset - s0.offset).max(GRADIENT_MIN_SPAN_EPS);
            let u = ((clamped - s0.offset) / span).clamp(GRADIENT_STOP_MIN, GRADIENT_STOP_MAX);
            return std::array::from_fn(|i| lerp_u8(s0.rgba[i], s1.rgba[i], u));
        }
    }
    last.rgba
}

fn lerp_u8(a: u8, b: u8, t: f64) -> u8 {
    (f64::from(a) + (f64::from(b) - f64::from(a)) * t)
        .round()
        .clamp(0.0, BYTE_MAX_F64) as u8
}

/// Evaluates `ColorLayerPaint` at font-space coordinate `pt`, modulated by `opacity`.
#[must_use]
pub fn evaluate_color_layer_paint_at_upm(
    paint: &ColorLayerPaint,
    pt: FontPoint,
    opacity: f32,
) -> [u8; 4] {
    debug_assert!(pt.x.0.is_finite() && pt.y.0.is_finite());
    let base = match paint {
        ColorLayerPaint::Solid { rgba } => *rgba,
        ColorLayerPaint::LinearGradient {
            p0,
            p1,
            stops,
            spread,
        } => {
            let (dx, dy) = (p1.x.0 - p0.x.0, p1.y.0 - p0.y.0);
            let len_sq = dx * dx + dy * dy;
            let raw_t = if len_sq > GRADIENT_MIN_SPAN_EPS {
                ((pt.x.0 - p0.x.0) * dx + (pt.y.0 - p0.y.0) * dy) / len_sq
            } else {
                GRADIENT_STOP_MIN
            };
            sample_gradient_stops(stops, apply_gradient_spread(raw_t, *spread))
        }
        ColorLayerPaint::RadialGradient {
            c0,
            r0,
            c1,
            r1,
            stops,
            spread,
        } => {
            let dist = (pt.x.0 - 0.5 * (c0.x.0 + c1.x.0)).hypot(pt.y.0 - 0.5 * (c0.y.0 + c1.y.0));
            let inner = r0.min(*r1).max(0.0);
            let span = r1.max(*r0).max(inner) - inner;
            let raw_t = if span > GRADIENT_MIN_SPAN_EPS {
                (dist - inner) / span
            } else {
                f64::from(u8::from(dist > inner))
            };
            sample_gradient_stops(stops, apply_gradient_spread(raw_t, *spread))
        }
        ColorLayerPaint::SweepGradient {
            center,
            start_deg,
            end_deg,
            stops,
        } => {
            let deg = (pt.y.0 - center.y.0)
                .atan2(pt.x.0 - center.x.0)
                .to_degrees()
                .rem_euclid(FULL_CIRCLE_DEG);
            let span = (end_deg - start_deg).abs().max(GRADIENT_MIN_SPAN_EPS);
            let t = ((deg - start_deg).rem_euclid(FULL_CIRCLE_DEG) / span)
                .clamp(GRADIENT_STOP_MIN, GRADIENT_STOP_MAX);
            sample_gradient_stops(stops, t)
        }
        ColorLayerPaint::Bitmap { image, .. } => sample_bitmap_paint(image, pt),
        ColorLayerPaint::ClippingMask => [0, 0, 0, 0],
    };
    let op = f64::from(opacity.clamp(ALPHA_MIN_F32, ALPHA_MAX_F32));
    let a = (f64::from(base[3]) * op).round().clamp(0.0, BYTE_MAX_F64) as u8;
    [base[0], base[1], base[2], a]
}

fn sample_bitmap_paint(image: &CanvasBackgroundImage, pt: FontPoint) -> [u8; 4] {
    if image.width_px == 0 || image.height_px == 0 || image.rgba_pixels.len() < 4 {
        return [0, 0, 0, 0];
    }
    let sx = image.scale_x.abs().max(GRADIENT_MIN_SPAN_EPS);
    let sy = image.scale_y.abs().max(GRADIENT_MIN_SPAN_EPS);
    let u = (pt.x.0 - image.position.x.0) / (f64::from(image.width_px) * sx);
    let v = (pt.y.0 - image.position.y.0) / (f64::from(image.height_px) * sy);
    if !(GRADIENT_STOP_MIN..=GRADIENT_STOP_MAX).contains(&u)
        || !(GRADIENT_STOP_MIN..=GRADIENT_STOP_MAX).contains(&v)
    {
        return [0, 0, 0, 0];
    }
    let px = ((u * f64::from(image.width_px)).floor() as u32).min(image.width_px - 1);
    let py = (((1.0 - v) * f64::from(image.height_px)).floor() as u32).min(image.height_px - 1);
    let idx = ((py * image.width_px + px) * 4) as usize;
    let Some(&[r, g, b, raw_a]) = image.rgba_pixels.get(idx..idx + 4) else {
        return [0, 0, 0, 0];
    };
    [
        r,
        g,
        b,
        lerp_u8(0, raw_a, f64::from(image.alpha.clamp(0.0, 1.0))),
    ]
}

/// Unmultiplied RGBA Porter-Duff `SrcOver` compositing: `C_out = C_src + (1 - a_src) * C_dst`.
#[must_use]
pub fn porter_duff_src_over(src: [u8; 4], dst: [u8; 4]) -> [u8; 4] {
    let sa = f64::from(src[3]) / BYTE_MAX_F64;
    let da = f64::from(dst[3]) / BYTE_MAX_F64;
    let out_a = sa + da * (1.0 - sa);
    if out_a <= GRADIENT_MIN_SPAN_EPS {
        return [0, 0, 0, 0];
    }
    let blend = |sc: u8, dc: u8| -> u8 {
        ((f64::from(sc) * sa + f64::from(dc) * da * (1.0 - sa)) / out_a)
            .round()
            .clamp(0.0, BYTE_MAX_F64) as u8
    };
    let a = (out_a * BYTE_MAX_F64).round().clamp(0.0, BYTE_MAX_F64) as u8;
    [
        blend(src[0], dst[0]),
        blend(src[1], dst[1]),
        blend(src[2], dst[2]),
        a,
    ]
}

/// Returns the primary `(start_upm, end_upm)` control ray for gradient handles and stop ticks.
#[must_use]
pub fn gradient_control_segment_upm(paint: &ColorLayerPaint) -> Option<(FontPoint, FontPoint)> {
    match paint {
        ColorLayerPaint::LinearGradient { p0, p1, .. } => Some((*p0, *p1)),
        ColorLayerPaint::RadialGradient { c1, r1, .. } => {
            Some((*c1, FontPoint::new(c1.x.0 + *r1, c1.y.0)))
        }
        ColorLayerPaint::SweepGradient {
            center, end_deg, ..
        } => {
            let rad = end_deg.to_radians();
            Some((
                *center,
                FontPoint::new(
                    center.x.0 + SWEEP_DEFAULT_HANDLE_LEN_UPM * rad.cos(),
                    center.y.0 + SWEEP_DEFAULT_HANDLE_LEN_UPM * rad.sin(),
                ),
            ))
        }
        ColorLayerPaint::Solid { .. }
        | ColorLayerPaint::Bitmap { .. }
        | ColorLayerPaint::ClippingMask => None,
    }
}

/// Returns the stop slice for a gradient paint variant, if any.
#[must_use]
pub fn gradient_stops_slice(paint: &ColorLayerPaint) -> Option<&[GradientStop]> {
    match paint {
        ColorLayerPaint::LinearGradient { stops, .. }
        | ColorLayerPaint::RadialGradient { stops, .. }
        | ColorLayerPaint::SweepGradient { stops, .. } => Some(stops.as_slice()),
        ColorLayerPaint::Solid { .. }
        | ColorLayerPaint::Bitmap { .. }
        | ColorLayerPaint::ClippingMask => None,
    }
}

/// Computes the 2D font-space handle position of `stop_index` along the gradient's control ray.
#[must_use]
pub fn gradient_stop_position_upm(paint: &ColorLayerPaint, stop_index: usize) -> Option<FontPoint> {
    let (p0, p1) = gradient_control_segment_upm(paint)?;
    let stops = gradient_stops_slice(paint)?;
    let stop = stops.get(stop_index)?;
    let t = stop.offset.clamp(GRADIENT_STOP_MIN, GRADIENT_STOP_MAX);
    Some(FontPoint::new(
        p0.x.0 + (p1.x.0 - p0.x.0) * t,
        p0.y.0 + (p1.y.0 - p0.y.0) * t,
    ))
}

/// Hit-tests interactive gradient handles and stop ticks in screen space.
#[must_use]
pub fn hit_test_gradient_handle(
    paint: &ColorLayerPaint,
    cursor_scr: ScreenPoint,
    vp: &ViewportTransform,
) -> Option<GradientHandleKind> {
    let (p0, p1) = gradient_control_segment_upm(paint)?;
    let stops = gradient_stops_slice(paint)?;
    for idx in 1..stops.len().saturating_sub(1) {
        if gradient_stop_position_upm(paint, idx).is_some_and(|pt| {
            vp.to_screen(pt).distance_to(cursor_scr) <= GRADIENT_HANDLE_HIT_RADIUS_DP
        }) {
            return Some(GradientHandleKind::StopTick(idx));
        }
    }
    if vp.to_screen(p1).distance_to(cursor_scr) <= GRADIENT_HANDLE_HIT_RADIUS_DP {
        return Some(GradientHandleKind::EndEndpoint);
    }
    if vp.to_screen(p0).distance_to(cursor_scr) <= GRADIENT_HANDLE_HIT_RADIUS_DP {
        return Some(GradientHandleKind::StartEndpoint);
    }
    if let ColorLayerPaint::RadialGradient { c0, r0, .. } = paint {
        if vp
            .to_screen(FontPoint::new(c0.x.0 + *r0, c0.y.0))
            .distance_to(cursor_scr)
            <= GRADIENT_HANDLE_HIT_RADIUS_DP
        {
            return Some(GradientHandleKind::RadialInnerRadius);
        }
    }
    (0..stops.len()).find_map(|idx| {
        gradient_stop_position_upm(paint, idx)
            .filter(|pt| vp.to_screen(*pt).distance_to(cursor_scr) <= GRADIENT_HANDLE_HIT_RADIUS_DP)
            .map(|_| GradientHandleKind::StopTick(idx))
    })
}

/// Mutates `paint` in response to dragging `handle` to `target_upm`.
pub fn apply_gradient_handle_drag(
    paint: &mut ColorLayerPaint,
    handle: GradientHandleKind,
    target_upm: FontPoint,
) -> bool {
    debug_assert!(target_upm.x.0.is_finite() && target_upm.y.0.is_finite());
    if let GradientHandleKind::StopTick(idx) = handle {
        let Some((p0, p1)) = gradient_control_segment_upm(paint) else {
            return false;
        };
        let (dx, dy) = (p1.x.0 - p0.x.0, p1.y.0 - p0.y.0);
        let len_sq = (dx * dx + dy * dy).max(GRADIENT_MIN_SPAN_EPS);
        let t = (((target_upm.x.0 - p0.x.0) * dx + (target_upm.y.0 - p0.y.0) * dy) / len_sq)
            .clamp(GRADIENT_STOP_MIN, GRADIENT_STOP_MAX);
        return match paint {
            ColorLayerPaint::LinearGradient { stops, .. }
            | ColorLayerPaint::RadialGradient { stops, .. }
            | ColorLayerPaint::SweepGradient { stops, .. } => {
                stops.get_mut(idx).map(|stop| stop.offset = t).is_some()
            }
            _ => false,
        };
    }
    apply_endpoint_drag(paint, handle, target_upm)
}

fn apply_endpoint_drag(
    paint: &mut ColorLayerPaint,
    handle: GradientHandleKind,
    target_upm: FontPoint,
) -> bool {
    match (paint, handle) {
        (ColorLayerPaint::LinearGradient { p0, .. }, GradientHandleKind::StartEndpoint) => {
            *p0 = target_upm;
            true
        }
        (ColorLayerPaint::LinearGradient { p1, .. }, GradientHandleKind::EndEndpoint) => {
            *p1 = target_upm;
            true
        }
        (ColorLayerPaint::RadialGradient { c0, c1, .. }, GradientHandleKind::StartEndpoint) => {
            *c0 = target_upm;
            *c1 = target_upm;
            true
        }
        (ColorLayerPaint::RadialGradient { c1, r1, .. }, GradientHandleKind::EndEndpoint) => {
            *r1 = (target_upm.x.0 - c1.x.0)
                .hypot(target_upm.y.0 - c1.y.0)
                .max(1.0);
            true
        }
        (ColorLayerPaint::RadialGradient { c0, r0, .. }, GradientHandleKind::RadialInnerRadius) => {
            *r0 = (target_upm.x.0 - c0.x.0)
                .hypot(target_upm.y.0 - c0.y.0)
                .max(0.0);
            true
        }
        (ColorLayerPaint::SweepGradient { center, .. }, GradientHandleKind::StartEndpoint) => {
            *center = target_upm;
            true
        }
        (
            ColorLayerPaint::SweepGradient {
                center, end_deg, ..
            },
            GradientHandleKind::EndEndpoint,
        ) => {
            *end_deg = (target_upm.y.0 - center.y.0)
                .atan2(target_upm.x.0 - center.x.0)
                .to_degrees()
                .rem_euclid(FULL_CIRCLE_DEG);
            true
        }
        _ => false,
    }
}
