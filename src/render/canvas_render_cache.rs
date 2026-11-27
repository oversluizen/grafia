//! Generation-keyed render cache for GPU background textures, expanded stroke outlines,
//! cubic intersection extra nodes, contour `BezPath` conversion, and preview strip rasterization.

use crate::adapters::state::GrafiaCanvasState;
use crate::core::{
    ColorLayerPaint, ContourId, ContourStrokeStyle, GlyphContour, GradientSpread, GradientStop,
    Node, ShapeBuilderRegion, ViewportTransform,
};
use crate::render::PreviewStripCache;
use crate::tokens::{CanvasBackgroundImage, VirtualExtraNode};
use egui::{ColorImage, TextureHandle, TextureOptions};
use kurbo::BezPath;
use std::cell::RefCell;
use std::collections::hash_map::DefaultHasher;
use std::collections::BTreeMap;
use std::hash::{Hash, Hasher};

const MAX_CACHED_BEZPATHS: usize = 128;
const MAX_CACHED_TEXTURES: usize = 16;

type BgTextureKey = (usize, u64, u32, u32, [u32; 4], usize, u8, u8);
type ColorSublayersTextureKey = (usize, u64, u32, u32, u32, u32, u32, usize);

struct CanvasRenderCache {
    bg_textures: BTreeMap<BgTextureKey, TextureHandle>,
    color_layer_textures: BTreeMap<ColorSublayersTextureKey, TextureHandle>,
    extra_nodes_key: Option<(u64, u64)>,
    extra_nodes: Vec<VirtualExtraNode>,
    expanded_strokes_key: Option<(u64, u64, u64)>,
    expanded_strokes: Vec<GlyphContour>,
    shape_builder_key: Option<(u64, u64)>,
    shape_builder_regions: Vec<ShapeBuilderRegion>,
    bezpath_entries: BTreeMap<u64, BezPath>,
    preview_strip_cache: PreviewStripCache,
}

thread_local! {
    static RENDER_CACHE: RefCell<CanvasRenderCache> = RefCell::new(CanvasRenderCache {
        bg_textures: BTreeMap::new(),
        color_layer_textures: BTreeMap::new(),
        extra_nodes_key: None,
        extra_nodes: Vec::new(),
        expanded_strokes_key: None,
        expanded_strokes: Vec::new(),
        shape_builder_key: None,
        shape_builder_regions: Vec::new(),
        bezpath_entries: BTreeMap::new(),
        preview_strip_cache: PreviewStripCache::new(),
    });
}

fn hash_nodes_slice(h: &mut DefaultHasher, nodes: &[Node], closed: bool) {
    (closed, nodes.len()).hash(h);
    for n in nodes {
        (
            n.id.0,
            n.point.x.0.to_bits(),
            n.point.y.0.to_bits(),
            format!("{:?}", n.node_type),
        )
            .hash(h);
    }
}

fn nodes_slice_fingerprint(nodes: &[Node], closed: bool) -> u64 {
    let mut h = DefaultHasher::new();
    hash_nodes_slice(&mut h, nodes, closed);
    h.finish()
}

fn contours_fingerprint(contours: &[GlyphContour]) -> u64 {
    let mut h = DefaultHasher::new();
    contours.len().hash(&mut h);
    for gc in contours {
        (gc.id.0, gc.visible).hash(&mut h);
        hash_nodes_slice(&mut h, &gc.nodes, gc.closed);
    }
    h.finish()
}

fn strokes_fingerprint(
    strokes: &BTreeMap<ContourId, ContourStrokeStyle>,
    var_strokes: &[crate::core::VariableWidthStrokeSpec],
    sym: &crate::core::RadialSymmetrySpec,
    smart_corners: &[crate::core::SmartCornerSpec],
) -> u64 {
    let mut h = DefaultHasher::new();
    (strokes.len(), var_strokes.len(), smart_corners.len()).hash(&mut h);
    (
        sym.fold_count,
        sym.dihedral_mirror,
        sym.center.x.0.to_bits(),
        sym.center.y.0.to_bits(),
    )
        .hash(&mut h);
    for sc in smart_corners {
        (sc.node_id.0, sc.radius_upm.to_bits()).hash(&mut h);
    }
    for (cid, st) in strokes {
        (cid.0, st.width.to_bits(), st.height.to_bits()).hash(&mut h);
    }
    for vs in var_strokes {
        (vs.contour_id.0, vs.samples.len()).hash(&mut h);
        for s in &vs.samples {
            (
                s.arc_ratio_t.to_bits(),
                s.width_left_upm.to_bits(),
                s.width_right_upm.to_bits(),
            )
                .hash(&mut h);
        }
    }
    h.finish()
}

/// Returns a cached `egui::TextureHandle` for `img` cropped to `crop_rect`, uploading to the GPU
/// only when `content_generation` or image/crop parameters change.
#[must_use = "cached GPU texture handle must be used for painting"]
pub fn get_or_upload_background_texture(
    ctx: &egui::Context,
    content_generation: u64,
    img: &CanvasBackgroundImage,
    crop_rect: [u32; 4],
) -> TextureHandle {
    let ctx_id = std::sync::Arc::as_ptr(&ctx.tex_manager()) as usize;
    let first_byte = img.rgba_pixels.first().copied().unwrap_or(0);
    let last_byte = img.rgba_pixels.last().copied().unwrap_or(0);
    let key: BgTextureKey = (
        ctx_id,
        content_generation,
        img.width_px,
        img.height_px,
        crop_rect,
        img.rgba_pixels.len(),
        first_byte,
        last_byte,
    );
    RENDER_CACHE.with(|cell| {
        let mut cache = cell.borrow_mut();
        if let Some(handle) = cache.bg_textures.get(&key) {
            return handle.clone();
        }
        if cache.bg_textures.len() >= MAX_CACHED_TEXTURES {
            let _ = cache.bg_textures.pop_first();
        }
        let [cx, cy, cw, ch] = crop_rect;
        let mut cropped_rgba = Vec::with_capacity((cw as usize) * (ch as usize) * 4);
        for row in cy..(cy + ch) {
            for col in cx..(cx + cw) {
                let base = ((row * img.width_px + col) as usize) * 4;
                cropped_rgba.extend_from_slice(&img.rgba_pixels[base..base + 4]);
            }
        }
        let color_img =
            ColorImage::from_rgba_unmultiplied([cw as usize, ch as usize], &cropped_rgba);
        let tex = ctx.load_texture("grafia_bg_image", color_img, TextureOptions::LINEAR);
        cache.bg_textures.insert(key, tex.clone());
        tex
    })
}

/// Returns cached `VirtualExtraNode`s for `state`, recomputing only when `content_generation` or
/// contour geometry changes.
#[must_use]
pub fn cached_extra_nodes(state: &GrafiaCanvasState) -> Vec<VirtualExtraNode> {
    let key = (
        state.content_generation(),
        contours_fingerprint(&state.glyph.contours),
    );
    RENDER_CACHE.with(|cell| {
        let mut cache = cell.borrow_mut();
        if cache.extra_nodes_key != Some(key) {
            cache.extra_nodes = state.compute_active_extra_nodes();
            cache.extra_nodes_key = Some(key);
        }
        cache.extra_nodes.clone()
    })
}

/// Returns cached expanded stroke contours for `state`, recomputing only when `content_generation`,
/// contours, or `contour_strokes` change.
#[must_use]
pub fn cached_expanded_stroke_contours(state: &GrafiaCanvasState) -> Vec<GlyphContour> {
    let key = (
        state.content_generation(),
        contours_fingerprint(&state.glyph.contours),
        strokes_fingerprint(
            &state.contour_strokes,
            &state.icon_studio.variable_width_strokes,
            &state.icon_studio.radial_symmetry,
            &state.curve_harmony.smart_corners,
        ),
    );
    RENDER_CACHE.with(|cell| {
        let mut cache = cell.borrow_mut();
        if cache.expanded_strokes_key != Some(key) {
            cache.expanded_strokes = state.compute_expanded_stroke_contours();
            cache.expanded_strokes_key = Some(key);
        }
        cache.expanded_strokes.clone()
    })
}

/// Returns cached `ShapeBuilderRegion`s for `state`, recomputing only when `content_generation` or
/// contour geometry changes (`SPEC-016 §3.1`).
#[must_use]
pub fn cached_shape_builder_regions(state: &GrafiaCanvasState) -> Vec<ShapeBuilderRegion> {
    let key = (
        state.content_generation(),
        contours_fingerprint(&state.glyph.contours),
    );
    RENDER_CACHE.with(|cell| {
        let mut cache = cell.borrow_mut();
        if cache.shape_builder_key != Some(key) {
            cache.shape_builder_regions =
                crate::core::compute_shape_builder_regions(&state.glyph.contours);
            cache.shape_builder_key = Some(key);
        }
        cache.shape_builder_regions.clone()
    })
}

/// Returns a cached `kurbo::BezPath` for `(nodes, closed)` so tessellation avoids re-running the
/// spline solver on unchanged contours.
#[must_use]
pub fn cached_contour_bezpath(nodes: &[Node], closed: bool) -> BezPath {
    let key = nodes_slice_fingerprint(nodes, closed);
    RENDER_CACHE.with(|cell| {
        let mut cache = cell.borrow_mut();
        if let Some(existing) = cache.bezpath_entries.get(&key) {
            return existing.clone();
        }
        if cache.bezpath_entries.len() >= MAX_CACHED_BEZPATHS {
            cache.bezpath_entries.clear();
        }
        let path = crate::core::contour_to_bezpath_with_splines(nodes, closed, 0.0, &[]);
        cache.bezpath_entries.insert(key, path.clone());
        path
    })
}

/// Accesses the shared thread-local `PreviewStripCache` instance for bottom preview strip caching.
pub fn with_preview_strip_cache<R>(f: impl FnOnce(&mut PreviewStripCache) -> R) -> R {
    RENDER_CACHE.with(|cell| {
        let mut cache = cell.borrow_mut();
        f(&mut cache.preview_strip_cache)
    })
}

fn hash_stops(h: &mut DefaultHasher, stops: &[GradientStop], spread: Option<GradientSpread>) {
    if let Some(sp) = spread {
        (sp as u8).hash(h);
    }
    for st in stops {
        st.offset.to_bits().hash(h);
        st.rgba.hash(h);
    }
}

fn color_sublayers_fingerprint(sublayers: &[crate::core::CanvasColorSublayer]) -> u64 {
    let mut h = DefaultHasher::new();
    sublayers.len().hash(&mut h);
    for sub in sublayers {
        (
            sub.id,
            sub.visible,
            sub.opacity.to_bits(),
            contours_fingerprint(&sub.contours),
        )
            .hash(&mut h);
        match &sub.paint {
            ColorLayerPaint::Solid { rgba } => (0_u8, *rgba).hash(&mut h),
            ColorLayerPaint::LinearGradient {
                p0,
                p1,
                stops,
                spread,
            } => {
                (1_u8, p0.x.0.to_bits(), p0.y.0.to_bits()).hash(&mut h);
                (p1.x.0.to_bits(), p1.y.0.to_bits()).hash(&mut h);
                hash_stops(&mut h, stops, Some(*spread));
            }
            ColorLayerPaint::RadialGradient {
                c0,
                r0,
                c1,
                r1,
                stops,
                spread,
            } => {
                (2_u8, c0.x.0.to_bits(), c0.y.0.to_bits(), r0.to_bits()).hash(&mut h);
                (c1.x.0.to_bits(), c1.y.0.to_bits(), r1.to_bits()).hash(&mut h);
                hash_stops(&mut h, stops, Some(*spread));
            }
            ColorLayerPaint::SweepGradient {
                center,
                start_deg,
                end_deg,
                stops,
            } => {
                (3_u8, center.x.0.to_bits(), center.y.0.to_bits()).hash(&mut h);
                (start_deg.to_bits(), end_deg.to_bits()).hash(&mut h);
                hash_stops(&mut h, stops, None);
            }
            ColorLayerPaint::Bitmap { image, ppem_strike } => {
                (4_u8, *ppem_strike, image.width_px, image.height_px).hash(&mut h);
            }
            ColorLayerPaint::ClippingMask => 5_u8.hash(&mut h),
        }
    }
    h.finish()
}

fn resolve_scaled_color_layer_vp(
    state: &GrafiaCanvasState,
    width: u32,
    height: u32,
) -> (u32, u32, ViewportTransform, ViewportTransform) {
    let vp = state.active_slot_viewport();
    let (eff_w, eff_h, scale) = state.effective_color_layer_pixmap_dimensions(width, height);
    let mut scaled_vp = vp;
    if scale < 1.0 {
        scaled_vp.zoom *= scale;
        scaled_vp.pan_offset.x.0 *= scale;
        scaled_vp.pan_offset.y.0 *= scale;
    }
    (eff_w, eff_h, vp, scaled_vp)
}

fn get_or_upload_cached_rgba_texture(
    ctx: &egui::Context,
    key: ColorSublayersTextureKey,
    eff_w: u32,
    eff_h: u32,
    name: &'static str,
    build_rgba: impl FnOnce() -> Vec<u8>,
) -> TextureHandle {
    RENDER_CACHE.with(|cell| {
        let mut cache = cell.borrow_mut();
        if let Some(handle) = cache.color_layer_textures.get(&key) {
            return handle.clone();
        }
        if cache.color_layer_textures.len() >= MAX_CACHED_TEXTURES {
            let _ = cache.color_layer_textures.pop_first();
        }
        let rgba = build_rgba();
        let color_img = ColorImage::from_rgba_unmultiplied([eff_w as usize, eff_h as usize], &rgba);
        let tex = ctx.load_texture(name, color_img, TextureOptions::LINEAR);
        cache.color_layer_textures.insert(key, tex.clone());
        tex
    })
}

fn make_color_tex_key(
    ctx: &egui::Context,
    gen: u64,
    eff_w: u32,
    eff_h: u32,
    vp: &ViewportTransform,
    fp: usize,
) -> ColorSublayersTextureKey {
    let ctx_id = std::sync::Arc::as_ptr(&ctx.tex_manager()) as usize;
    let (z, px, py) = (
        vp.zoom.to_bits(),
        vp.pan_offset.x.0.to_bits(),
        vp.pan_offset.y.0.to_bits(),
    );
    (ctx_id, gen, eff_w, eff_h, z, px, py, fp)
}

/// Returns a cached `egui::TextureHandle` for `state.color_sublayers` rasterized at `(width, height)`.
#[must_use = "cached GPU texture handle must be used for painting"]
pub fn get_or_upload_color_sublayers_texture(
    ctx: &egui::Context,
    state: &GrafiaCanvasState,
    width: u32,
    height: u32,
) -> TextureHandle {
    let (eff_w, eff_h, _, scaled_vp) = resolve_scaled_color_layer_vp(state, width, height);
    let eval_sublayers = state.evaluate_active_color_sublayers();
    let fp = color_sublayers_fingerprint(&eval_sublayers) as usize;
    let gen = state.content_generation();
    let key = make_color_tex_key(ctx, gen, eff_w, eff_h, &scaled_vp, fp);
    get_or_upload_cached_rgba_texture(ctx, key, eff_w, eff_h, "grafia_color_sublayers", || {
        crate::render::color_layers_pixmap_pass::rasterize_color_sublayers_rgba(
            &eval_sublayers,
            &scaled_vp,
            eff_w,
            eff_h,
        )
    })
}

/// Returns a cached `egui::TextureHandle` for `CanvasViewMode::LiquidGlass` (`SPEC-016 §5.1`).
#[must_use = "cached GPU texture handle must be used for painting"]
pub fn get_or_upload_liquid_glass_texture(
    ctx: &egui::Context,
    state: &GrafiaCanvasState,
    width: u32,
    height: u32,
) -> TextureHandle {
    let (eff_w, eff_h, vp, scaled_vp) = resolve_scaled_color_layer_vp(state, width, height);
    let contours = crate::render::liquid_glass_pass::collect_active_liquid_glass_contours(state);
    let lg = &state.icon_studio.liquid_glass;
    let mut lg_hasher = DefaultHasher::new();
    (
        crate::core::math_consts::LIQUID_GLASS_CACHE_SALT,
        lg.dome_radius_upm.to_bits(),
        lg.smooth_union_k_upm.to_bits(),
        lg.ior_base.to_bits(),
        lg.chromatic_dispersion.to_bits(),
        lg.specular_intensity.to_bits(),
        lg.tint_rgba,
        contours_fingerprint(&contours),
    )
        .hash(&mut lg_hasher);
    let (gen, fp) = (state.content_generation(), lg_hasher.finish() as usize);
    let key = make_color_tex_key(ctx, gen, eff_w, eff_h, &scaled_vp, fp);
    get_or_upload_cached_rgba_texture(ctx, key, eff_w, eff_h, "grafia_liquid_glass", || {
        let cursor_upm = state.cursor_screen_pos.map(|s| vp.to_font(s));
        crate::render::liquid_glass_pass::rasterize_liquid_glass_rgba(
            &contours,
            lg,
            &scaled_vp,
            cursor_upm,
            state.is_interactive_drag_active(),
            eff_w,
            eff_h,
        )
    })
}
