//! `CanvasViewMode::LiquidGlass` Physical Renderer via `vitrea v0.1.0` (`SPEC-016 §5.1`):
//! 2D Signed Distance Field (`vitrea::sdf_closed_polyline_sample`), $C^1$ metaball meniscus fusion
//! (`vitrea::sdf_smooth_union_with_grad`), 3D dome normals (`vitrea::dome_normal_3d_from_sdf`),
//! Cauchy 3-wavelength RGB refraction (`vitrea::refract_chromatic_rgb`), and Schlick-Fresnel /
//! Blinn-Phong specular reflection (`vitrea::evaluate_schlick_fresnel_and_blinn_phong`) with
//! Active Bounding-Box Culling (Pijler G).

use crate::adapters::state::GrafiaCanvasState;
use crate::core::math_consts::{
    BYTE_MAX_F32, CURVE_FLATTEN_TOLERANCE, ICON_STUDIO_MAX_COORD_ABS_UPM,
    LIQUID_GLASS_BACKDROP_ALT_RGB, LIQUID_GLASS_BACKDROP_BASE_RGB, LIQUID_GLASS_BACKDROP_TILE_UPM,
    LIQUID_GLASS_BBOX_MARGIN_FACTOR, LIQUID_GLASS_DRAG_STEP_PX, LIQUID_GLASS_KEY_LIGHT_INV_SQRT3,
    LIQUID_GLASS_MIN_STEP_PX, LIQUID_GLASS_PROXIMITY_RADIUS_UPM, LIQUID_GLASS_SPECULAR_GAIN,
    LIQUID_GLASS_TINT_MIX_GAIN,
};
use crate::core::{
    contour_to_bezpath_with_splines, FontPoint, GlyphContour, LiquidGlassRenderConfig, ScreenPoint,
    ViewportTransform,
};
use crate::render::canvas_render_cache::get_or_upload_liquid_glass_texture;
use egui::{Color32, Painter, Pos2, Rect};
use kurbo::PathEl;
use tiny_skia::Pixmap;

/// Optical surface sample evaluated at a single 2D point via `vitrea v0.1.0` (`SPEC-016 §5.1`).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LiquidGlassSurfaceEval {
    pub coverage: f32,
    pub normal_3d: vitrea::GlamVec3,
    pub refracted_rgb_pts: [vitrea::GlamVec2; 3],
    pub fresnel_rim_factor: f32,
    pub specular_highlight: f32,
}

fn sanitize_f32_coord(v: f64) -> f32 {
    if v.is_finite() {
        v.clamp(
            -ICON_STUDIO_MAX_COORD_ABS_UPM,
            ICON_STUDIO_MAX_COORD_ABS_UPM,
        ) as f32
    } else {
        0.0
    }
}

fn flatten_closed_contour_polylines(contours: &[GlyphContour]) -> Vec<Vec<vitrea::GlamVec2>> {
    let mut polys = Vec::new();
    for gc in contours {
        if !gc.visible || !gc.closed || gc.nodes.len() < 3 {
            continue;
        }
        let bez = contour_to_bezpath_with_splines(&gc.nodes, true, 0.0, &[]);
        let mut pts = Vec::new();
        kurbo::flatten(bez.iter(), CURVE_FLATTEN_TOLERANCE, |el| match el {
            PathEl::MoveTo(p) | PathEl::LineTo(p) => {
                pts.push(vitrea::GlamVec2::new(
                    sanitize_f32_coord(p.x),
                    sanitize_f32_coord(p.y),
                ));
            }
            _ => {}
        });
        if pts.len() >= 3 {
            polys.push(pts);
        }
    }
    polys
}

fn eval_fused_sdf_at_pt(
    pt: vitrea::GlamVec2,
    polys: &[Vec<vitrea::GlamVec2>],
    smooth_k: f32,
) -> Option<vitrea::SdfSample2d> {
    let mut acc: Option<vitrea::SdfSample2d> = None;
    for poly in polys {
        let s = vitrea::sdf_closed_polyline_sample(pt, poly);
        acc = Some(match acc {
            None => s,
            Some(prev) => vitrea::sdf_smooth_union_with_grad(prev, s, smooth_k),
        });
    }
    acc
}

fn eval_surface_from_sdf(
    pt: vitrea::GlamVec2,
    sdf: vitrea::SdfSample2d,
    optics: &vitrea::LiquidGlassOpticalParams,
    cursor_upm: Option<vitrea::GlamVec2>,
) -> LiquidGlassSurfaceEval {
    let coverage = ((1.0 - sdf.distance) * 0.5).clamp(0.0, 1.0);
    let normal_3d = vitrea::dome_normal_3d_from_sdf(sdf, optics.bevel_width_pt);
    let refracted_rgb_pts = vitrea::refract_chromatic_rgb(
        pt,
        normal_3d,
        optics.dome_thickness_pt,
        optics.ior_dispersion_rgb,
    );
    let fresnel_rim_factor = (1.0 - normal_3d.z.clamp(0.0, 1.0)).clamp(0.0, 1.0);
    let specular_highlight = vitrea::evaluate_schlick_fresnel_and_blinn_phong(
        pt,
        normal_3d,
        cursor_upm,
        LIQUID_GLASS_PROXIMITY_RADIUS_UPM,
        optics.ior_glass,
    );
    LiquidGlassSurfaceEval {
        coverage,
        normal_3d,
        refracted_rgb_pts,
        fresnel_rim_factor,
        specular_highlight,
    }
}

fn sample_backdrop_rgb_at_upm(pt: vitrea::GlamVec2) -> [f32; 3] {
    let tile = LIQUID_GLASS_BACKDROP_TILE_UPM;
    let gx = (pt.x / tile).floor() as i32;
    let gy = (pt.y / tile).floor() as i32;
    if ((gx ^ gy) & 1) == 0 {
        LIQUID_GLASS_BACKDROP_BASE_RGB
    } else {
        LIQUID_GLASS_BACKDROP_ALT_RGB
    }
}

/// Evaluates the exact `vitrea` `(SdfSample2d, LiquidGlassSurfaceEval)` at `pt_upm` in font space (`SPEC-016 §5.1`).
#[must_use]
pub fn evaluate_liquid_glass_point_upm(
    contours: &[GlyphContour],
    config: &LiquidGlassRenderConfig,
    pt_upm: FontPoint,
) -> Option<(vitrea::SdfSample2d, LiquidGlassSurfaceEval)> {
    let polys = flatten_closed_contour_polylines(contours);
    if polys.is_empty() {
        return None;
    }
    let clean_cfg = config.sanitized();
    let optics = clean_cfg.to_vitrea_optics();
    let pt = vitrea::GlamVec2::new(
        sanitize_f32_coord(pt_upm.x.0),
        sanitize_f32_coord(pt_upm.y.0),
    );
    let sdf = eval_fused_sdf_at_pt(pt, &polys, optics.meniscus_merge_k_pt)?;
    let surf = eval_surface_from_sdf(pt, sdf, &optics, None);
    Some((sdf, surf))
}

fn compute_active_screen_bbox(
    polys: &[Vec<vitrea::GlamVec2>],
    margin_upm: f32,
    vp: &ViewportTransform,
    w: u32,
    h: u32,
) -> Option<(u32, u32, u32, u32)> {
    let (mut min_x, mut min_y) = (f32::INFINITY, f32::INFINITY);
    let (mut max_x, mut max_y) = (f32::NEG_INFINITY, f32::NEG_INFINITY);
    for p in polys.iter().flatten() {
        min_x = min_x.min(p.x);
        min_y = min_y.min(p.y);
        max_x = max_x.max(p.x);
        max_y = max_y.max(p.y);
    }
    if !min_x.is_finite() || !max_x.is_finite() {
        return None;
    }
    let s_min = vp.to_screen(FontPoint::new(
        f64::from(min_x - margin_upm),
        f64::from(max_y + margin_upm),
    ));
    let s_max = vp.to_screen(FontPoint::new(
        f64::from(max_x + margin_upm),
        f64::from(min_y - margin_upm),
    ));
    let x0 = (s_min.x.0.min(s_max.x.0).floor().max(0.0) as u32).min(w);
    let y0 = (s_min.y.0.min(s_max.y.0).floor().max(0.0) as u32).min(h);
    let x1 = (s_min.x.0.max(s_max.x.0).ceil().max(0.0) as u32).min(w);
    let y1 = (s_min.y.0.max(s_max.y.0).ceil().max(0.0) as u32).min(h);
    (x1 > x0 && y1 > y0).then_some((x0, y0, x1, y1))
}

fn compose_glass_pixel_rgba(
    surf: &LiquidGlassSurfaceEval,
    tint_f: [f32; 4],
    specular_intensity: f32,
) -> [u8; 4] {
    let [pr, pg, pb] = surf.refracted_rgb_pts;
    let refr = [
        sample_backdrop_rgb_at_upm(pr)[0],
        sample_backdrop_rgb_at_upm(pg)[1],
        sample_backdrop_rgb_at_upm(pb)[2],
    ];
    let l = LIQUID_GLASS_KEY_LIGHT_INV_SQRT3;
    let key_dot =
        (surf.normal_3d.x * (-l) + surf.normal_3d.y * l + surf.normal_3d.z * l).clamp(0.0, 1.0);
    let mix_t = (tint_f[3] * LIQUID_GLASS_TINT_MIX_GAIN).clamp(0.0, 1.0);
    let rim_spec = ((surf.fresnel_rim_factor * 0.5 + surf.specular_highlight)
        * specular_intensity
        * LIQUID_GLASS_SPECULAR_GAIN
        * (0.5 + 0.5 * key_dot))
        .clamp(0.0, 1.0);
    let mut out = [0_u8; 4];
    for c in 0..3 {
        let tinted = refr[c] * (1.0 - mix_t) + tint_f[c] * mix_t;
        let lit = (tinted + rim_spec * (1.0 - surf.fresnel_rim_factor * 0.5)).clamp(0.0, 1.0);
        out[c] = (lit * BYTE_MAX_F32).round() as u8;
    }
    out[3] = (surf.coverage.clamp(0.0, 1.0) * BYTE_MAX_F32).round() as u8;
    out
}

/// Rasterizes the active `LiquidGlassRenderConfig` optical scene into an unmultiplied RGBA buffer
/// of size `width * height * 4` using Active BBox Culling (`SPEC-016 §5.1`, Pijler G).
#[must_use]
pub fn rasterize_liquid_glass_rgba(
    contours: &[GlyphContour],
    config: &LiquidGlassRenderConfig,
    vp: &ViewportTransform,
    cursor_upm: Option<FontPoint>,
    interactive_drag: bool,
    width: u32,
    height: u32,
) -> Vec<u8> {
    let (w, h) = (width.max(1), height.max(1));
    let mut out = vec![0_u8; (w * h * 4) as usize];
    let polys = flatten_closed_contour_polylines(contours);
    if polys.is_empty() {
        return out;
    }
    let clean_cfg = config.sanitized();
    let optics = clean_cfg.to_vitrea_optics();
    let margin = optics.meniscus_merge_k_pt * LIQUID_GLASS_BBOX_MARGIN_FACTOR;
    let Some((x0, y0, x1, y1)) = compute_active_screen_bbox(&polys, margin, vp, w, h) else {
        return out;
    };
    let cursor_v = cursor_upm
        .map(|c| vitrea::GlamVec2::new(sanitize_f32_coord(c.x.0), sanitize_f32_coord(c.y.0)));
    let tint_f = clean_cfg.tint_rgba.map(|b| f32::from(b) / BYTE_MAX_F32);
    let step = if interactive_drag {
        LIQUID_GLASS_DRAG_STEP_PX
    } else {
        LIQUID_GLASS_MIN_STEP_PX
    };

    for py in (y0..y1).step_by(step as usize) {
        let sy = (py as f32) + 0.5;
        for px in (x0..x1).step_by(step as usize) {
            let sx = (px as f32) + 0.5;
            let pt_f = vp.to_font(ScreenPoint::new(sx, sy));
            let pt = vitrea::GlamVec2::new(pt_f.x.0 as f32, pt_f.y.0 as f32);
            let Some(sdf) = eval_fused_sdf_at_pt(pt, &polys, optics.meniscus_merge_k_pt) else {
                continue;
            };
            if sdf.distance > 1.0 {
                continue;
            }
            let surf = eval_surface_from_sdf(pt, sdf, &optics, cursor_v);
            if surf.coverage <= 0.0 {
                continue;
            }
            let px_rgba = compose_glass_pixel_rgba(&surf, tint_f, clean_cfg.specular_intensity);
            for dy in 0..step.min(y1 - py) {
                for dx in 0..step.min(x1 - px) {
                    let idx = (((py + dy) * w + (px + dx)) as usize) * 4;
                    out[idx..idx + 4].copy_from_slice(&px_rgba);
                }
            }
        }
    }
    out
}

/// Evaluates the active contours for Liquid Glass rendering (including kinetic phase evaluation,
/// `SmartCornerSpec` fillets, knockout halo masks, and stroke expansion when active).
#[must_use]
pub fn collect_active_liquid_glass_contours(state: &GrafiaCanvasState) -> Vec<GlyphContour> {
    state
        .evaluate_active_preview_silhouette_contours()
        .into_iter()
        .map(|(c, _)| c)
        .collect()
}

/// Paints `CanvasViewMode::LiquidGlass` into `egui::Painter` via the generation-keyed texture cache (`SPEC-016 §5.1`).
pub fn paint_liquid_glass_pass_egui(painter: &Painter, state: &GrafiaCanvasState, rect: Rect) {
    let w = (rect.width().round() as u32).max(1);
    let h = (rect.height().round() as u32).max(1);
    let tex = get_or_upload_liquid_glass_texture(painter.ctx(), state, w, h);
    let uv = Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0));
    painter.image(tex.id(), rect, uv, Color32::WHITE);
}

/// Renders `CanvasViewMode::LiquidGlass` into a `tiny_skia::Pixmap` (`SPEC-016 §5.1`).
pub fn render_liquid_glass_pass_tiny_skia(pixmap: &mut Pixmap, state: &GrafiaCanvasState) {
    let contours = collect_active_liquid_glass_contours(state);
    let vp = state.active_slot_viewport();
    let cursor_upm = state.cursor_screen_pos.map(|s| vp.to_font(s));
    let rgba = rasterize_liquid_glass_rgba(
        &contours,
        &state.icon_studio.liquid_glass,
        &vp,
        cursor_upm,
        state.is_interactive_drag_active(),
        pixmap.width(),
        pixmap.height(),
    );
    crate::render::color_layers_pixmap_pass::blend_unmultiplied_rgba_onto_pixmap(pixmap, &rgba);
}
