//! Interactive Host-Port ("Stekkers") Testbank & Mock Font Host for `vector_canvas_demo`.

pub mod demo_panels;
pub mod inspector_ui;
pub mod presets;
pub mod toolbar_specs;

pub use presets::DemoPreset;

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
}

impl Default for MockFontHost {
    fn default() -> Self {
        Self::new()
    }
}

impl MockFontHost {
    #[must_use]
    pub fn new() -> Self {
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
        };
        host.push_log(
            "HostPort Ready: Cmd+U, Text Tool (T), Components, Spacing/Kerning connected",
        );
        host
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
        if glyph_name == "cedillacomb" {
            return vec![
                CanvasAnchor {
                    id: AnchorId(1),
                    name: "_bottom".into(),
                    position: FontPoint::new(0.0, 0.0),
                    selected: true,
                },
                CanvasAnchor {
                    id: AnchorId(2),
                    name: "bottom".into(),
                    position: FontPoint::new(0.0, -140.0),
                    selected: false,
                },
            ];
        }
        if glyph_name.ends_with("comb") {
            return vec![
                CanvasAnchor {
                    id: AnchorId(1),
                    name: "_top".into(),
                    position: FontPoint::new(0.0, 500.0),
                    selected: true,
                },
                CanvasAnchor {
                    id: AnchorId(2),
                    name: "top".into(),
                    position: FontPoint::new(0.0, 680.0),
                    selected: false,
                },
            ];
        }
        let is_lower = glyph_name.chars().next().is_some_and(char::is_lowercase);
        let top_y = if is_lower { 500.0 } else { 700.0 };
        vec![
            CanvasAnchor {
                id: AnchorId(1),
                name: "top".into(),
                position: FontPoint::new(cx, top_y),
                selected: true,
            },
            CanvasAnchor {
                id: AnchorId(2),
                name: "bottom".into(),
                position: FontPoint::new(cx, 0.0),
                selected: false,
            },
            CanvasAnchor {
                id: AnchorId(3),
                name: "ogonek".into(),
                position: FontPoint::new((width - 90.0).max(cx), 0.0),
                selected: false,
            },
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
