//! Deterministic Headless Full-Window RGBA8 Framebuffer Rasterizer for `vector_canvas_demo` (`TC-V0180-006`).
//!
//! Rasterizes the complete `egui::Context::tessellate` mesh stream and all registered `egui::TextureId`
//! atlases (`Inter`, `CupertinoIcons`, `MacContextThumbDial` raytraced rosette, `PreviewStripCache`, and
//! `GrafiaCanvasWidget`) at exact screen resolutions (`1980×1080` Het Baken, `1480×924` S9 Ultra, `412×915` S25 Ultra).

#[cfg(not(target_arch = "wasm32"))]
use egui::{pos2, Color32, Context, Pos2, RawInput, Rect, TextureId, Vec2};
#[cfg(not(target_arch = "wasm32"))]
use std::collections::BTreeMap;

#[cfg(not(target_arch = "wasm32"))]
type TextureAtlasMap = BTreeMap<TextureId, (usize, usize, Vec<[u8; 4]>)>;
#[cfg(not(target_arch = "wasm32"))]
const TRIANGLE_MIN_AREA_EPS: f32 = 1.0e-5;
#[cfg(not(target_arch = "wasm32"))]
const BARYCENTRIC_EDGE_EPS: f32 = -1.0e-4;
#[cfg(not(target_arch = "wasm32"))]
const BAKEN_SCREEN_W_DP: f32 = 1980.0;
#[cfg(not(target_arch = "wasm32"))]
const BAKEN_SCREEN_H_DP: f32 = 1080.0;
#[cfg(not(target_arch = "wasm32"))]
const S9_ULTRA_BROWSER_W_DP: f32 = 1480.0;
#[cfg(not(target_arch = "wasm32"))]
const S9_ULTRA_BROWSER_H_DP: f32 = 924.0;

#[cfg(not(target_arch = "wasm32"))]
#[rustfmt::skip]
pub fn verify_and_save_baken_1980_and_s9_ultra_renders(ctx: &Context) {
    use super::super::GrafiaVectorCanvasDemo;
    use super::{DemoPreset as P, StudioSubWorkspace as W, StudioThemePreset};
    use std::path::PathBuf;

    let mut mob = GrafiaVectorCanvasDemo::new();
    let mob_rect = Rect::from_min_size(Pos2::ZERO, Vec2::new(390.0, 844.0));
    let mut mob_out = ctx.run_ui(RawInput { screen_rect: Some(mob_rect), ..Default::default() }, |ui| mob.render_ui(ui));
    mob_out.textures_delta.clear();
    assert!(!mob.host.show_left_sidebar && mob.host.show_mobile_thumb_hud);
    assert!(mob.host.glyph_library.len() >= 44 && mob.host.open_tabs.len() >= 5);
    let new_g = mob.host.create_new_glyph_or_icon(false, &mut mob.state, &mut mob.theme);
    let new_ic = mob.host.create_new_glyph_or_icon(true, &mut mob.state, &mut mob.theme);
    assert!(mob.host.glyph_library.contains_key(&new_g) && mob.host.glyph_library.contains_key(&new_ic));
    assert!(mob.host.export_active_to_svg_buffer(&mob.state).contains("<svg"));
    assert_eq!(mob.state.preferences.touch_finger_offset_dp, 0.0);
    assert_eq!(super::sonoma_top_bar::TOOL_CATEGORIES.len(), 4);

    let out_dir = PathBuf::from("target/visual_verification");
    let _ = std::fs::create_dir_all(&out_dir);
    let s9 = Vec2::new(S9_ULTRA_BROWSER_W_DP, S9_ULTRA_BROWSER_H_DP);
    let bk = Vec2::new(BAKEN_SCREEN_W_DP, BAKEN_SCREEN_H_DP);
    for (file_name, vp, sub_ws, preset, show_bottom) in [
        ("render_ws0_font_and_icon_grid_s9_ultra_1480_by_924.png", s9, W::LetterContourAndSpacing, P::EditorialContoursAndInfoBox, false),
        ("render_ws1_letter_contour_s9_ultra_1480_by_924.png", s9, W::LetterContourAndSpacing, P::TextStreamAovaKerning, false),
        ("render_ws2_letter_cagd_bochten_s9_ultra_1480_by_924.png", s9, W::LetterCagdAndStems, P::CurvatureCombSpikesAndHeatmap, false),
        ("render_ws3_letter_masters_hinting_s9_ultra_1480_by_924.png", s9, W::LetterMastersAndHinting, P::MultiMasterCompatAndAutoMatch, false),
        ("render_ws4_icon_vector_glass_s9_ultra_1480_by_924.png", s9, W::IconVectorAndGlass, P::IconLiquidGlassAndBakeColrV1, false),
        ("render_ws5_icon_kinetic_touch_s9_ultra_1480_by_924.png", s9, W::IconKineticAndTouch, P::KineticWifiBounceAndColorModes, true),
        ("render_baken_1980_by_1080_letter_studio_aova.png", bk, W::LetterContourAndSpacing, P::TextStreamAovaKerning, false),
        ("render_baken_1980_by_1080_icon_studio_wifi_keylines.png", bk, W::IconKineticAndTouch, P::KineticWifiBounceAndColorModes, true),
    ] {
        let render_ctx = Context::default();
        super::toolbar_specs::configure_demo_fonts(&render_ctx);
        let mut app = GrafiaVectorCanvasDemo::new();
        app.host.apply_studio_theme(StudioThemePreset::PaperLight, &mut app.state, &mut app.theme);
        app.host.activate_sub_workspace(sub_ws, &mut app.state, &mut app.theme);
        app.host.apply_preset(preset, &mut app.state, &mut app.theme);
        app.host.show_font_grid_view = file_name.starts_with("render_ws0_");
        app.show_preview_panel = show_bottom;
        (app.host.show_left_sidebar, app.host.show_right_inspector) = (false, true);
        app.host.initial_canvas_fitted = false;
        app.host.thumb_latched_mask = egui_macview::MacThumbIntentMask::CONSTRAIN;
        let bg = app.host.active_mac_theme().window_background();
        let img = DemoCpuFramebuffer::render_ui_to_rgba(&render_ctx, vp, bg, |ui| app.render_ui(ui));
        assert_eq!((img.width(), img.height()), (vp.x as u32, vp.y as u32));
        let _ = img.save(out_dir.join(file_name));
    }
}

#[cfg(not(target_arch = "wasm32"))]
pub struct DemoCpuFramebuffer {
    pub width: usize,
    pub height: usize,
    pub pixels: Vec<[u8; 4]>,
}

#[cfg(not(target_arch = "wasm32"))]
impl DemoCpuFramebuffer {
    pub fn render_ui_to_rgba(
        ctx: &Context,
        viewport: Vec2,
        bg: Color32,
        mut ui_fn: impl FnMut(&mut egui::Ui),
    ) -> image::RgbaImage {
        let mut tex_map: TextureAtlasMap = BTreeMap::new();
        let mut shapes_out = Vec::new();
        let screen_rect = Rect::from_min_size(Pos2::ZERO, viewport);
        for _pass in 0..3 {
            let input = RawInput {
                screen_rect: Some(screen_rect),
                ..Default::default()
            };
            let mut full = ctx.run_ui(input, &mut ui_fn);
            shapes_out = full.shapes;
            let _ = ctx.tessellate(shapes_out.clone(), 1.0);
            Self::ingest_texture_deltas(&mut tex_map, &full.textures_delta);
            full.textures_delta.clear();
        }
        ctx.fonts(|f| {
            let img = f.image();
            let [w, h] = img.size;
            let rgba: Vec<[u8; 4]> = img.pixels.iter().map(|p| p.to_array()).collect();
            tex_map.insert(TextureId::default(), (w, h, rgba));
        });
        let w_px = viewport.x.round() as usize;
        let h_px = viewport.y.round() as usize;
        let mut fb = Self {
            width: w_px,
            height: h_px,
            pixels: vec![bg.to_array(); w_px * h_px],
        };
        for prim in ctx.tessellate(shapes_out, 1.0) {
            let clip = prim.clip_rect;
            let egui::epaint::Primitive::Mesh(mesh) = prim.primitive else {
                continue;
            };
            let tex = tex_map
                .get(&mesh.texture_id)
                .map(|(w, h, b)| (*w, *h, b.as_slice()));
            for &[i0, i1, i2] in mesh.indices.as_chunks::<3>().0 {
                let (v0, v1, v2) = (
                    mesh.vertices[i0 as usize],
                    mesh.vertices[i1 as usize],
                    mesh.vertices[i2 as usize],
                );
                fb.rasterize_triangle(v0, v1, v2, clip, tex);
            }
        }
        let flat: Vec<u8> = fb.pixels.into_iter().flatten().collect();
        image::RgbaImage::from_raw(w_px as u32, h_px as u32, flat)
            .unwrap_or_else(|| image::RgbaImage::new(1, 1))
    }

    fn ingest_texture_deltas(tex_map: &mut TextureAtlasMap, delta_set: &egui::TexturesDelta) {
        let mut entries: Vec<_> = delta_set.set.iter().collect();
        entries.sort_by_key(|(id, _)| *id);
        for (&id, deltas) in entries {
            for delta in deltas {
                let [w, h] = delta.image.size();
                let rgba: Vec<[u8; 4]> = match &delta.image {
                    egui::ImageData::Color(c) => c.pixels.iter().map(|p| p.to_array()).collect(),
                };
                if let Some([ox, oy]) = delta.pos {
                    if let Some((tw, _, buf)) = tex_map.get_mut(&id) {
                        for r in 0..h {
                            for c in 0..w {
                                let idx = (oy + r) * *tw + (ox + c);
                                if idx < buf.len() {
                                    buf[idx] = rgba[r * w + c];
                                }
                            }
                        }
                    }
                } else {
                    tex_map.insert(id, (w, h, rgba));
                }
            }
        }
    }

    fn rasterize_triangle(
        &mut self,
        v0: egui::epaint::Vertex,
        v1: egui::epaint::Vertex,
        v2: egui::epaint::Vertex,
        clip: Rect,
        tex: Option<(usize, usize, &[[u8; 4]])>,
    ) {
        let min_x = v0
            .pos
            .x
            .min(v1.pos.x)
            .min(v2.pos.x)
            .max(clip.min.x)
            .floor()
            .max(0.0) as usize;
        let max_x =
            (v0.pos.x.max(v1.pos.x).max(v2.pos.x).min(clip.max.x).ceil() as usize).min(self.width);
        let min_y = v0
            .pos
            .y
            .min(v1.pos.y)
            .min(v2.pos.y)
            .max(clip.min.y)
            .floor()
            .max(0.0) as usize;
        let max_y =
            (v0.pos.y.max(v1.pos.y).max(v2.pos.y).min(clip.max.y).ceil() as usize).min(self.height);
        let area = (v1.pos.x - v0.pos.x) * (v2.pos.y - v0.pos.y)
            - (v1.pos.y - v0.pos.y) * (v2.pos.x - v0.pos.x);
        if area.abs() < TRIANGLE_MIN_AREA_EPS {
            return;
        }
        let inv = 1.0 / area;
        for py in min_y..max_y {
            for px in min_x..max_x {
                let p = pos2(px as f32 + 0.5, py as f32 + 0.5);
                if !clip.contains(p) {
                    continue;
                }
                let w0 = ((v1.pos.x - p.x) * (v2.pos.y - p.y)
                    - (v1.pos.y - p.y) * (v2.pos.x - p.x))
                    * inv;
                let w1 = ((v2.pos.x - p.x) * (v0.pos.y - p.y)
                    - (v2.pos.y - p.y) * (v0.pos.x - p.x))
                    * inv;
                let w2 = 1.0 - w0 - w1;
                if w0 < BARYCENTRIC_EDGE_EPS
                    || w1 < BARYCENTRIC_EDGE_EPS
                    || w2 < BARYCENTRIC_EDGE_EPS
                {
                    continue;
                }
                Self::blend_pixel(
                    &mut self.pixels[py * self.width + px],
                    (v0, v1, v2),
                    (w0, w1, w2),
                    tex,
                );
            }
        }
    }

    fn blend_pixel(
        dst: &mut [u8; 4],
        (v0, v1, v2): (
            egui::epaint::Vertex,
            egui::epaint::Vertex,
            egui::epaint::Vertex,
        ),
        (w0, w1, w2): (f32, f32, f32),
        tex: Option<(usize, usize, &[[u8; 4]])>,
    ) {
        let max_ch = f32::from(u8::MAX);
        let max_u16 = u16::from(u8::MAX);
        let (c0, c1, c2) = (
            v0.color.to_array(),
            v1.color.to_array(),
            v2.color.to_array(),
        );
        let mut rgba = [0, 1, 2, 3].map(|i| {
            (w0 * f32::from(c0[i]) + w1 * f32::from(c1[i]) + w2 * f32::from(c2[i]))
                .clamp(0.0, max_ch) as u8
        });
        if let Some((tw, th, tbuf)) = tex {
            let u = (w0 * v0.uv.x + w1 * v1.uv.x + w2 * v2.uv.x).clamp(0.0, 1.0);
            let v = (w0 * v0.uv.y + w1 * v1.uv.y + w2 * v2.uv.y).clamp(0.0, 1.0);
            if u > 0.0 || v > 0.0 {
                let tx = ((u * tw as f32) as usize).min(tw.saturating_sub(1));
                let ty = ((v * th as f32) as usize).min(th.saturating_sub(1));
                let tcol = tbuf[ty * tw + tx];
                for ch in 0..4 {
                    rgba[ch] = ((u16::from(rgba[ch]) * u16::from(tcol[ch])) / max_u16) as u8;
                }
            }
        }
        let a = u16::from(rgba[3]);
        if a > 0 {
            for ch in 0..3 {
                dst[ch] = ((u16::from(rgba[ch]) * a + u16::from(dst[ch]) * (max_u16 - a)) / max_u16)
                    as u8;
            }
            dst[3] = u8::MAX;
        }
    }
}
