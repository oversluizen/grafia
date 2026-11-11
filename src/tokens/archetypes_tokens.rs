//! Canonical Typographic Archetype Presets (`/A`, `/n`, `/o`, `/s`) loaded from Token JSON (`SPEC-002 §5`, `DEF-GLIFO-006`).

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
pub struct GlyphPointToken {
    pub x: f64,
    pub y: f64,
}

impl GlyphPointToken {
    #[must_use]
    pub const fn new(x: f64, y: f64) -> Self {
        Self { x, y }
    }
}

impl From<(f64, f64)> for GlyphPointToken {
    fn from((x, y): (f64, f64)) -> Self {
        Self { x, y }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct GlyphAGoldenMaster {
    pub glyphname: String,
    #[serde(alias = "units_per_em", alias = "unitsPerEm", alias = "UPM")]
    pub upm: u16,
    #[serde(alias = "width", alias = "advance_width", alias = "advanceWidth")]
    pub width: f64,
    pub ascender: f64,
    #[serde(alias = "cap_height", alias = "capHeight")]
    pub cap_height: f64,
    #[serde(alias = "x_height", alias = "xHeight")]
    pub x_height: f64,
    pub descender: f64,
    #[serde(alias = "outer_contour", alias = "outerContour")]
    pub outer_contour: Vec<GlyphPointToken>,
    #[serde(alias = "inner_counter", alias = "innerCounter")]
    pub inner_counter: Vec<GlyphPointToken>,
}

impl GlyphAGoldenMaster {
    /// Loads the Golden Master `/A` parameters embedded from `tokens/glyph_a_golden_master.json`.
    #[must_use]
    pub fn load() -> Self {
        let json_str = include_str!("../../tokens/glyph_a_golden_master.json");
        serde_json::from_str(json_str).expect("glyph_a_golden_master.json must be valid JSON")
    }
}

/// Canonical typographic archetypes for polygonal and cubic Bézier verification.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum GlyphArchetype {
    /// Straight-line uppercase `/A` (outer trapezoid + inner triangular counter).
    A,
    /// Lowercase `/n` (straight stem segments + cubic shoulder arch).
    N,
    /// Lowercase `/o` (outer and inner closed cubic ovals with 4 smooth extrema).
    O,
    /// Lowercase `/s` (double-inflection cubic spine with smooth and corner transitions).
    S,
}

impl GlyphArchetype {
    /// Returns all four canonical archetypes in toolbar display order.
    #[inline]
    #[must_use]
    pub const fn all() -> [Self; 4] {
        [Self::A, Self::N, Self::O, Self::S]
    }

    /// Returns the short display label (`"/A"`, `"/n"`, `"/o"`, `"/s"`).
    #[inline]
    #[must_use]
    pub const fn button_label(self) -> &'static str {
        match self {
            Self::A => "/A",
            Self::N => "/n",
            Self::O => "/o",
            Self::S => "/s",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ArchetypeNodePreset {
    pub x: f64,
    pub y: f64,
    pub node_type: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ArchetypeContourPreset {
    pub closed: bool,
    pub nodes: Vec<ArchetypeNodePreset>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ArchetypeGlyphPreset {
    pub glyph_name: String,
    pub advance_width: f64,
    pub upm: f64,
    pub ascender: f64,
    pub cap_height: f64,
    pub x_height: f64,
    pub baseline: f64,
    pub descender: f64,
    pub contours: Vec<ArchetypeContourPreset>,
}

#[derive(Debug, Clone, Deserialize)]
struct CurvedArchetypesCatalog {
    n: ArchetypeGlyphPreset,
    o: ArchetypeGlyphPreset,
    s: ArchetypeGlyphPreset,
}

/// Loads the normative preset for any of the four canonical archetypes (`/A`, `/n`, `/o`, `/s`).
#[must_use]
pub fn load_archetype_preset(archetype: GlyphArchetype) -> ArchetypeGlyphPreset {
    match archetype {
        GlyphArchetype::A => {
            let m = GlyphAGoldenMaster::load();
            ArchetypeGlyphPreset {
                glyph_name: m.glyphname,
                advance_width: m.width,
                upm: f64::from(m.upm),
                ascender: m.ascender,
                cap_height: m.cap_height,
                x_height: m.x_height,
                baseline: 0.0,
                descender: m.descender,
                contours: vec![
                    ArchetypeContourPreset {
                        closed: true,
                        nodes: m
                            .outer_contour
                            .into_iter()
                            .map(|pt| ArchetypeNodePreset {
                                x: pt.x,
                                y: pt.y,
                                node_type: "corner".to_string(),
                            })
                            .collect(),
                    },
                    ArchetypeContourPreset {
                        closed: true,
                        nodes: m
                            .inner_counter
                            .into_iter()
                            .map(|pt| ArchetypeNodePreset {
                                x: pt.x,
                                y: pt.y,
                                node_type: "corner".to_string(),
                            })
                            .collect(),
                    },
                ],
            }
        }
        other => {
            let raw = include_str!("../../tokens/glyph_archetypes_golden.json");
            let catalog: CurvedArchetypesCatalog =
                serde_json::from_str(raw).expect("Valid glyph_archetypes_golden.json");
            match other {
                GlyphArchetype::N => catalog.n,
                GlyphArchetype::O => catalog.o,
                GlyphArchetype::S => catalog.s,
                GlyphArchetype::A => unreachable!(),
            }
        }
    }
}
