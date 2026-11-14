//! Generation-keyed render cache for GPU background textures, expanded stroke outlines,
//! cubic intersection extra nodes, contour `BezPath` conversion, and preview strip rasterization.

use crate::adapters::state::GrafiaCanvasState;
use crate::core::{ContourId, ContourStrokeStyle, GlyphContour, Node};
use crate::render::PreviewStripCache;
use crate::tokens::{CanvasBackgroundImage, VirtualExtraNode};
use egui::{ColorImage, TextureHandle, TextureOptions};
use kurbo::BezPath;
use std::cell::RefCell;
use std::collections::BTreeMap;

const MAX_CACHED_BEZPATHS: usize = 128;
const MAX_CACHED_TEXTURES: usize = 16;
const HASH_MUL: u64 = 131;
const HASH_ADD: u64 = 17;

type BgTextureKey = (usize, u64, u32, u32, [u32; 4], usize, u8, u8);
type ColorSublayersTextureKey = (usize, u64, u32, u32, u32, u32, u32, usize);

struct CanvasRenderCache {
    bg_textures: BTreeMap<BgTextureKey, TextureHandle>,
    color_layer_textures: BTreeMap<ColorSublayersTextureKey, TextureHandle>,
    extra_nodes_key: Option<(u64, u64)>,
    extra_nodes: Vec<VirtualExtraNode>,
    expanded_strokes_key: Option<(u64, u64, u64)>,
    expanded_strokes: Vec<GlyphContour>,
    bezpath_entries: BTreeMap<u64, BezPath>,
    preview_strip_cache: PreviewStripCache,
}

impl CanvasRenderCache {
    fn new() -> Self {
        Self {
            bg_textures: BTreeMap::new(),
            color_layer_textures: BTreeMap::new(),
            extra_nodes_key: None,
            extra_nodes: Vec::new(),
            expanded_strokes_key: None,
            expanded_strokes: Vec::new(),
            bezpath_entries: BTreeMap::new(),
            preview_strip_cache: PreviewStripCache::new(),
        }
    }
}

thread_local! {
    static RENDER_CACHE: RefCell<CanvasRenderCache> = RefCell::new(CanvasRenderCache::new());
}

fn nodes_slice_fingerprint(nodes: &[Node], closed: bool) -> u64 {
    let mut acc = u64::from(closed).wrapping_add((nodes.len() as u64).wrapping_mul(HASH_MUL));
    for n in nodes {
        acc = (acc.wrapping_mul(HASH_MUL).wrapping_add(n.id.0) ^ n.point.x.0.to_bits())
            .wrapping_mul(HASH_MUL)
            ^ n.point.y.0.to_bits();
        let tag = match n.node_type {
            crate::core::NodeType::Corner => 1_u64,
            crate::core::NodeType::Smooth => 2_u64,
            crate::core::NodeType::OffCurve => 3_u64,
            crate::core::NodeType::HobbyStar {
                tension_in,
                tension_out,
                ..
            } => 5_u64.wrapping_add(tension_in.to_bits()) ^ tension_out.to_bits(),
            crate::core::NodeType::EulerSpiro { .. } => 6_u64,
        };
        acc = acc.wrapping_mul(HASH_MUL).wrapping_add(tag);
    }
    acc
}

fn contours_fingerprint(contours: &[GlyphContour]) -> u64 {
    let mut acc = (contours.len() as u64).wrapping_mul(HASH_MUL);
    for gc in contours {
        acc = acc
            .wrapping_mul(HASH_MUL)
            .wrapping_add(gc.id.0 as u64)
            .wrapping_add(u64::from(gc.visible))
            .wrapping_add(nodes_slice_fingerprint(&gc.nodes, gc.closed));
    }
    acc
}

fn strokes_fingerprint(strokes: &BTreeMap<ContourId, ContourStrokeStyle>) -> u64 {
    let mut acc = (strokes.len() as u64).wrapping_mul(HASH_MUL);
    for (cid, st) in strokes {
        acc = (acc.wrapping_mul(HASH_MUL).wrapping_add(cid.0 as u64) ^ st.width.to_bits())
            .wrapping_mul(HASH_MUL)
            ^ st.height.to_bits().wrapping_add(HASH_ADD);
    }
    acc
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
        if cache.extra_nodes_key == Some(key) {
            return cache.extra_nodes.clone();
        }
        let computed = state.compute_active_extra_nodes();
        cache.extra_nodes_key = Some(key);
        cache.extra_nodes.clone_from(&computed);
        computed
    })
}

/// Returns cached expanded stroke contours for `state`, recomputing only when `content_generation`,
/// contours, or `contour_strokes` change.
#[must_use]
pub fn cached_expanded_stroke_contours(state: &GrafiaCanvasState) -> Vec<GlyphContour> {
    let key = (
        state.content_generation(),
        contours_fingerprint(&state.glyph.contours),
        strokes_fingerprint(&state.contour_strokes),
    );
    RENDER_CACHE.with(|cell| {
        let mut cache = cell.borrow_mut();
        if cache.expanded_strokes_key == Some(key) {
            return cache.expanded_strokes.clone();
        }
        let computed = state.compute_expanded_stroke_contours();
        cache.expanded_strokes_key = Some(key);
        cache.expanded_strokes.clone_from(&computed);
        computed
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

/// Returns a cached `egui::TextureHandle` for `state.color_sublayers` rasterized at `(width, height)`.
#[must_use = "cached GPU texture handle must be used for painting"]
pub fn get_or_upload_color_sublayers_texture(
    ctx: &egui::Context,
    state: &GrafiaCanvasState,
    width: u32,
    height: u32,
) -> TextureHandle {
    let ctx_id = std::sync::Arc::as_ptr(&ctx.tex_manager()) as usize;
    let vp = state.active_slot_viewport();
    let (eff_w, eff_h, scale) = state.effective_color_layer_pixmap_dimensions(width, height);
    let mut scaled_vp = vp;
    if scale < 1.0 {
        scaled_vp.zoom *= scale;
        scaled_vp.pan_offset.x.0 *= scale;
        scaled_vp.pan_offset.y.0 *= scale;
    }
    let key: ColorSublayersTextureKey = (
        ctx_id,
        state.content_generation(),
        eff_w,
        eff_h,
        scaled_vp.zoom.to_bits(),
        scaled_vp.pan_offset.x.0.to_bits(),
        scaled_vp.pan_offset.y.0.to_bits(),
        state.color_sublayers.len(),
    );
    RENDER_CACHE.with(|cell| {
        let mut cache = cell.borrow_mut();
        if let Some(handle) = cache.color_layer_textures.get(&key) {
            return handle.clone();
        }
        if cache.color_layer_textures.len() >= MAX_CACHED_TEXTURES {
            let _ = cache.color_layer_textures.pop_first();
        }
        let rgba = crate::render::color_layers_pixmap_pass::rasterize_color_sublayers_rgba(
            &state.color_sublayers,
            &scaled_vp,
            eff_w,
            eff_h,
        );
        let color_img = ColorImage::from_rgba_unmultiplied([eff_w as usize, eff_h as usize], &rgba);
        let tex = ctx.load_texture("grafia_color_sublayers", color_img, TextureOptions::LINEAR);
        cache.color_layer_textures.insert(key, tex.clone());
        tex
    })
}
