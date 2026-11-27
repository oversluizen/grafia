//! Interactive Host-Port ("Stekkers") Testbank & Mock Font Host for `vector_canvas_demo`.

pub mod cagd_and_controller_presets;
pub mod color_and_hinting_presets;
pub mod demo_panels;
pub mod guided_showcase;
pub mod guided_showcase_cards;
pub mod host_ports;
pub mod icon_hero_templates;
pub mod icon_studio_showcase;
pub mod icon_studio_workbench_ui;
pub mod inspector_ui;
pub mod mobile_thumb_hud;
pub mod presets;
pub mod sonoma_top_bar;
pub mod spline_and_trace_presets;
pub mod toolbar_specs;

pub use guided_showcase::{coach_card, tool_dutch_guide, ShowcaseChapter};
pub use presets::{DemoPreset, StudioThemePreset};
pub use sonoma_top_bar::StudioWorkspaceMode;

use grafia::adapters::GrafiaCanvasState;
use grafia::core::{
    AnchorId, CanvasAnchor, ContourId, FontPoint, GhostMarkPath, Glyph, GlyphContour, GlyphSlot,
    Node, NodeId, NodeType,
};
use grafia::render::{PreviewRenderConfig, PreviewStripCache};
use grafia::tokens::GlyphArchetype;
use std::collections::{BTreeMap, VecDeque};

pub(crate) const MAX_EVENT_LOG_LEN: usize = 8;
pub(crate) const METRIC_STEP_UPM: f64 = 10.0;
const PREVIEW_TEX_WIDTH_PX: u32 = 720;
const PREVIEW_TEX_HEIGHT_PX: u32 = 96;

pub struct MockFontHost {
    pub glyph_library: BTreeMap<String, Glyph>,
    pub kerning_pairs: BTreeMap<(String, String), i32>,
    pub text_buffer: String,
    pub fit_curve_col: usize,
    pub preview_cfg: PreviewRenderConfig,
    pub preview_cache: PreviewStripCache,
    pub event_log: VecDeque<String>,
    pub show_left_sidebar: bool,
    pub show_right_inspector: bool,
    pub responsive_layout_initialized: bool,
    pub initial_canvas_fitted: bool,
    pub last_canvas_size: Option<egui::Vec2>,
    pub active_chapter: ShowcaseChapter,
    pub studio_mode: StudioWorkspaceMode,
    pub studio_theme: StudioThemePreset,
    pub tour_index: usize,
    pub show_coach_banner: bool,
    pub crispness_badge_cache: Option<(u64, bool, egui::TextureHandle)>,
    pub show_mobile_thumb_hud: bool,
    pub thumb_dock_right: bool,
    pub is_fullscreen: bool,
    pub preview_panel_height_dp: f32,
    pub thumb_dial_layer: egui_macview::MacThumbDialLayer,
    pub thumb_latched_mask: egui_macview::MacThumbIntentMask,
    pub thumb_rotary_norm: f32,
    pub edge_wheel_value_pct: f32,
}

impl Default for MockFontHost {
    fn default() -> Self {
        Self::new()
    }
}

impl MockFontHost {
    #[must_use]
    pub fn new() -> Self {
        let glyph_library = Self::build_default_glyph_library();
        let mut kerning_pairs = BTreeMap::new();
        for ((l, r), v) in [
            (("A", "O"), -35),
            (("O", "A"), -35),
            (("A", "S"), -20),
            (("A", "V"), -70),
            (("V", "A"), -70),
        ] {
            kerning_pairs.insert((l.to_owned(), r.to_owned()), v);
        }
        let mut host = Self {
            glyph_library,
            kerning_pairs,
            text_buffer: "AOVA".to_owned(),
            fit_curve_col: 0,
            preview_cfg: PreviewRenderConfig::default_for_size(
                PREVIEW_TEX_WIDTH_PX,
                PREVIEW_TEX_HEIGHT_PX,
            ),
            preview_cache: PreviewStripCache::new(),
            event_log: VecDeque::new(),
            show_left_sidebar: true,
            show_right_inspector: false,
            responsive_layout_initialized: false,
            initial_canvas_fitted: false,
            last_canvas_size: None,
            active_chapter: ShowcaseChapter::BasisTekenen,
            studio_mode: StudioWorkspaceMode::LetterStudio,
            studio_theme: StudioThemePreset::PaperLight,
            tour_index: 0,
            show_coach_banner: true,
            crispness_badge_cache: None,
            show_mobile_thumb_hud: true,
            thumb_dock_right: false,
            is_fullscreen: false,
            preview_panel_height_dp: 200.0,
            thumb_dial_layer: egui_macview::MacThumbDialLayer::CompassModifiers,
            thumb_latched_mask: egui_macview::MacThumbIntentMask::NONE,
            thumb_rotary_norm: 0.5,
            edge_wheel_value_pct: 48.0,
        };
        host.push_log(
            "HostPort Ready: Cmd+U, Text Tool (T), Components, Spacing/Kerning connected",
        );
        host
    }

    pub fn fit_canvas_viewport(&self, state: &mut GrafiaCanvasState) {
        if let Some(sz) = self.last_canvas_size.filter(|s| s.x > 64.0 && s.y > 64.0) {
            state.fit_viewport_to_glyph(sz.x, sz.y);
        } else {
            self::toolbar_specs::fit_demo_canvas_viewport(state);
        }
    }

    fn build_default_glyph_library() -> BTreeMap<String, Glyph> {
        let mut glyph_library = BTreeMap::new();
        let archetypes = [
            ("A", GlyphArchetype::A),
            ("O", GlyphArchetype::O),
            ("N", GlyphArchetype::N),
            ("S", GlyphArchetype::S),
        ];
        for (name, arch) in archetypes {
            let mut st = GrafiaCanvasState::init_glyph_a();
            st.load_glyph_archetype(arch);
            st.glyph.name = name.to_owned();
            glyph_library.insert(name.to_owned(), st.glyph);
        }
        glyph_library.insert(
            "V".to_owned(),
            Self::make_box_glyph(
                "V",
                640.0,
                vec![Self::make_quad_contour([
                    (80.0, 700.0),
                    (320.0, 0.0),
                    (560.0, 700.0),
                ])],
            ),
        );
        glyph_library.insert(
            "acutecomb".to_owned(),
            Self::make_box_glyph(
                "acutecomb",
                0.0,
                vec![Self::make_rect_contour(1, 20, -25.0, 30.0, 45.0, 140.0)],
            ),
        );
        glyph_library.insert(
            "dieresiscomb".to_owned(),
            Self::make_box_glyph(
                "dieresiscomb",
                0.0,
                vec![
                    Self::make_rect_contour(1, 30, -55.0, 40.0, -15.0, 90.0),
                    Self::make_rect_contour(2, 40, 15.0, 40.0, 55.0, 90.0),
                ],
            ),
        );
        glyph_library.insert(
            "cedillacomb".to_owned(),
            Self::make_box_glyph(
                "cedillacomb",
                0.0,
                vec![Self::make_rect_contour(1, 50, -20.0, -140.0, 25.0, 0.0)],
            ),
        );
        glyph_library
    }

    fn make_rect_contour(
        cid: usize,
        base_nid: u64,
        x0: f64,
        y0: f64,
        x1: f64,
        y1: f64,
    ) -> GlyphContour {
        GlyphContour::new(
            ContourId(cid),
            vec![
                Node::new(NodeId(base_nid), x0, y0, NodeType::Corner),
                Node::new(NodeId(base_nid + 1), x1, y0, NodeType::Corner),
                Node::new(NodeId(base_nid + 2), x1, y1, NodeType::Corner),
                Node::new(NodeId(base_nid + 3), x0, y1, NodeType::Corner),
            ],
        )
    }

    fn make_quad_contour(pts: [(f64, f64); 3]) -> GlyphContour {
        GlyphContour::new(
            ContourId(1),
            vec![
                Node::new(NodeId(10), pts[0].0, pts[0].1, NodeType::Corner),
                Node::new(NodeId(11), pts[1].0, pts[1].1, NodeType::Corner),
                Node::new(NodeId(12), pts[2].0, pts[2].1, NodeType::Corner),
            ],
        )
    }

    fn make_box_glyph(name: &str, width: f64, contours: Vec<GlyphContour>) -> Glyph {
        let mut g = GrafiaCanvasState::init_glyph_a().glyph;
        g.name = name.to_owned();
        g.width = width;
        g.contours = contours;
        g.annotations.clear();
        g
    }

    pub fn push_log(&mut self, entry: impl Into<String>) {
        if self.event_log.len() >= MAX_EVENT_LOG_LEN {
            self.event_log.pop_front();
        }
        self.event_log.push_back(entry.into());
    }

    #[must_use]
    pub fn default_anchors_for(&self, glyph_name: &str, width: f64) -> Vec<CanvasAnchor> {
        let cx = (width * 0.5).round();
        let anchor = |id, name: &str, x, y, selected| CanvasAnchor {
            id: AnchorId(id),
            name: name.into(),
            position: FontPoint::new(x, y),
            selected,
        };
        if glyph_name == "cedillacomb" {
            return vec![
                anchor(1, "_bottom", 0.0, 0.0, true),
                anchor(2, "bottom", 0.0, -140.0, false),
            ];
        }
        if glyph_name.ends_with("comb") {
            return vec![
                anchor(1, "_top", 0.0, 500.0, true),
                anchor(2, "top", 0.0, 680.0, false),
            ];
        }
        let is_lower = glyph_name.chars().next().is_some_and(char::is_lowercase);
        let top_y = if is_lower { 500.0 } else { 700.0 };
        vec![
            anchor(1, "top", cx, top_y, true),
            anchor(2, "bottom", cx, 0.0, false),
            anchor(3, "ogonek", (width - 90.0).max(cx), 0.0, false),
        ]
    }

    #[must_use]
    pub fn default_ghost_marks(&self) -> Vec<GhostMarkPath> {
        let get_contours = |key: &str| {
            self.glyph_library
                .get(key)
                .map_or_else(Vec::new, |g| g.contours.clone())
        };
        let cedilla = get_contours("cedillacomb");
        vec![
            GhostMarkPath {
                anchor_name: "top".into(),
                contours: get_contours("acutecomb"),
            },
            GhostMarkPath {
                anchor_name: "top".into(),
                contours: get_contours("dieresiscomb"),
            },
            GhostMarkPath {
                anchor_name: "bottom".into(),
                contours: cedilla.clone(),
            },
            GhostMarkPath {
                anchor_name: "ogonek".into(),
                contours: cedilla,
            },
        ]
    }

    #[must_use]
    pub fn resolve_glyph(&self, ch: char) -> Glyph {
        let key = ch.to_uppercase().to_string();
        self.glyph_library.get(&key).cloned().unwrap_or_else(|| {
            Self::make_box_glyph(
                ".notdef",
                500.0,
                vec![Self::make_rect_contour(1, 90, 60.0, 0.0, 440.0, 700.0)],
            )
        })
    }

    #[must_use]
    pub fn build_slots_for_text(&self, text: &str) -> Vec<GlyphSlot> {
        let mut slots = Vec::new();
        let mut prev_name: Option<String> = None;
        for ch in text.chars() {
            let glyph = self.resolve_glyph(ch);
            let kern = prev_name
                .as_ref()
                .and_then(|l| {
                    self.kerning_pairs
                        .get(&(l.clone(), glyph.name.clone()))
                        .copied()
                })
                .unwrap_or(0);
            prev_name = Some(glyph.name.clone());
            slots.push(GlyphSlot::new(glyph, kern));
        }
        slots
    }
}
