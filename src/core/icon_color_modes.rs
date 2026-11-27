//! Apple SF Symbols 4-Mode Color Engine (`Monochrome`, `Hierarchical`, `Palette`, `Multicolor`)
//! and `SymbolHierarchyLevel` (`Primary`, `Secondary`, `Tertiary`) (`SPEC-017 §2.3`).

use crate::core::color_layers::{CanvasColorSublayer, ColorLayerPaint};
use crate::core::fit_and_stroke::ContourStrokeStyle;
use crate::core::glyph_model::{ContourId, GlyphContour};
use crate::core::icon_studio::{
    BounceDirection, KineticLayerScope, KineticSymbolEffect, ReplaceFallback, ScaleDirection,
    SymbolLayerTag, VariableWidthStrokeSpec, WiggleAxis,
};
use crate::core::math_consts::{
    DEFAULT_HIER_PRIMARY_RGBA, DEFAULT_MONO_TINT_RGBA, DEFAULT_PALETTE_COLORS_RGBA,
    HIERARCHICAL_SECONDARY_ALPHA, HIERARCHICAL_TERTIARY_ALPHA, SVG_MAX_STROKE_WIDTH_UPM,
};
use crate::core::smart_corners_brush::{evaluate_smart_corner_contours, SmartCornerSpec};
use crate::core::stroke_expand::expand_single_contour_stroke;
use crate::core::variable_width_stroke::expand_variable_width_contour;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// Apple SF Symbols 7 centerline draw-trim direction for `Appear (DrawOn)` and `Disappear (DrawOff)` (`SPEC-017 §2.4`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum DrawDirection {
    #[default]
    StartToEnd,
    EndToStart,
    CenterOut,
}

impl DrawDirection {
    /// Returns the canonical lowercase SVG `data-draw-direction` attribute string.
    #[must_use]
    pub const fn as_svg_attr(self) -> &'static str {
        match self {
            Self::StartToEnd => "start-to-end",
            Self::EndToStart => "end-to-start",
            Self::CenterOut => "center-out",
        }
    }

    /// Parses a `DrawDirection` from an optional `data-draw-direction` attribute.
    #[must_use]
    pub fn from_svg_attr(draw_dir: Option<&str>) -> Self {
        match draw_dir.map(str::trim) {
            Some(s) if s.eq_ignore_ascii_case("end-to-start") => Self::EndToStart,
            Some(s) if s.eq_ignore_ascii_case("center-out") => Self::CenterOut,
            _ => Self::StartToEnd,
        }
    }

    /// Computes normalized arc-length interval `(r0, r1)` in `[0.0, 1.0]` for `Appear (DrawOn)` at progress `s`.
    #[must_use]
    pub fn appear_arc_range(self, s: f64) -> (f64, f64) {
        let u = if s.is_finite() {
            s.clamp(0.0, 1.0)
        } else {
            0.0
        };
        match self {
            Self::StartToEnd => (0.0, u),
            Self::EndToStart => ((1.0 - u).clamp(0.0, 1.0), 1.0),
            Self::CenterOut => {
                let half = 0.5 * u;
                ((0.5 - half).clamp(0.0, 1.0), (0.5 + half).clamp(0.0, 1.0))
            }
        }
    }

    /// Computes normalized arc-length interval `(r0, r1)` in `[0.0, 1.0]` for `Disappear (DrawOff)` at progress `s`.
    #[must_use]
    pub fn disappear_arc_range(self, s: f64) -> (f64, f64) {
        let u = if s.is_finite() {
            s.clamp(0.0, 1.0)
        } else {
            0.0
        };
        match self {
            Self::StartToEnd => (u, 1.0),
            Self::EndToStart => (0.0, (1.0 - u).clamp(0.0, 1.0)),
            Self::CenterOut => {
                let rem = 1.0 - u;
                let half = 0.5 * rem;
                ((0.5 - half).clamp(0.0, 1.0), (0.5 + half).clamp(0.0, 1.0))
            }
        }
    }
}

/// Semantic hierarchy tier for an icon sublayer in `Hierarchical` and `Palette` rendering modes (`SPEC-017 §2.3`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum SymbolHierarchyLevel {
    #[default]
    Primary,
    Secondary,
    Tertiary,
}

impl SymbolHierarchyLevel {
    /// Returns the canonical lowercase SVG `data-hierarchy` attribute string.
    #[must_use]
    pub const fn as_svg_attr(self) -> &'static str {
        match self {
            Self::Primary => "primary",
            Self::Secondary => "secondary",
            Self::Tertiary => "tertiary",
        }
    }

    /// Parses a `SymbolHierarchyLevel` from an optional SVG `data-hierarchy` attribute value.
    #[must_use]
    pub fn from_svg_attr(raw: Option<&str>) -> Self {
        match raw.map(str::trim) {
            Some(s) if s.eq_ignore_ascii_case("secondary") => Self::Secondary,
            Some(s) if s.eq_ignore_ascii_case("tertiary") => Self::Tertiary,
            _ => Self::Primary,
        }
    }

    /// Returns the opacity multiplier for this tier in `SymbolColorRenderingMode::Hierarchical`.
    #[must_use]
    pub const fn hierarchical_opacity_factor(self) -> f32 {
        match self {
            Self::Primary => 1.0,
            Self::Secondary => HIERARCHICAL_SECONDARY_ALPHA,
            Self::Tertiary => HIERARCHICAL_TERTIARY_ALPHA,
        }
    }
}

/// Apple SF Symbols 4-Mode Color Engine rendering mode (`SPEC-017 §2.3`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum SymbolColorRenderingMode {
    Monochrome {
        tint_rgba: [u8; 4],
    },
    Hierarchical {
        primary_rgba: [u8; 4],
    },
    Palette {
        colors_rgba: [[u8; 4]; 3],
    },
    #[default]
    Multicolor,
}

impl SymbolColorRenderingMode {
    /// Returns the `(short_label, preset_mode)` for index `0..=3` (`Mono`, `Hier`, `Pal`, `Multi`).
    #[must_use]
    pub const fn preset_by_index(idx: usize) -> (&'static str, Self) {
        match idx {
            0 => (
                "Mono",
                Self::Monochrome {
                    tint_rgba: DEFAULT_MONO_TINT_RGBA,
                },
            ),
            1 => (
                "Hier",
                Self::Hierarchical {
                    primary_rgba: DEFAULT_HIER_PRIMARY_RGBA,
                },
            ),
            2 => (
                "Pal",
                Self::Palette {
                    colors_rgba: DEFAULT_PALETTE_COLORS_RGBA,
                },
            ),
            _ => ("Multi", Self::Multicolor),
        }
    }
}

impl KineticSymbolEffect {
    /// Constructs a canonical preset `KineticSymbolEffect` for index `0..=9` (`SPEC-016 §4.2`).
    #[must_use]
    pub fn preset_by_index(idx: usize, target_contours: Vec<GlyphContour>) -> Self {
        let (scope, direction) = (KineticLayerScope::ByLayer, BounceDirection::Up);
        match idx {
            0 => Self::Pulse { scope },
            1 => Self::Bounce { direction, scope },
            2 => Self::VariableColor {
                cumulative: false,
                reversing: true,
                dim_inactive: true,
            },
            3 => Self::Scale {
                direction: ScaleDirection::Up,
                scope,
            },
            4 => Self::Appear {
                direction,
                draw_on: true,
                scope,
            },
            5 => Self::Disappear {
                direction: BounceDirection::Down,
                draw_off: true,
                scope,
            },
            6 => Self::Wiggle {
                axis: WiggleAxis::Clockwise,
                scope,
            },
            7 => Self::Rotate {
                clockwise: true,
                scope,
            },
            8 => Self::Breathe {
                pulse_opacity: true,
                scope,
            },
            _ => Self::MagicReplace {
                fallback: ReplaceFallback::DownUp,
                scope,
                target_contours,
            },
        }
    }
}

/// Resolves `sublayers` according to `tags` and the active `SymbolColorRenderingMode` (`SPEC-017 §2.3`).
#[must_use]
pub fn resolve_sublayers_for_color_mode(
    sublayers: &[CanvasColorSublayer],
    tags: &[SymbolLayerTag],
    mode: SymbolColorRenderingMode,
) -> Vec<CanvasColorSublayer> {
    sublayers
        .iter()
        .enumerate()
        .map(|(idx, sub)| {
            let mut resolved = sub.clone();
            let base_opacity = if sub.opacity.is_finite() {
                sub.opacity.clamp(0.0, 1.0)
            } else {
                1.0
            };
            resolved.opacity = base_opacity;
            if matches!(sub.paint, ColorLayerPaint::ClippingMask) {
                return resolved;
            }
            let level = tags
                .get(idx)
                .map_or(SymbolHierarchyLevel::Primary, |t| t.hierarchy_level);
            match mode {
                SymbolColorRenderingMode::Multicolor => {}
                SymbolColorRenderingMode::Monochrome { tint_rgba } => {
                    resolved.paint = ColorLayerPaint::Solid { rgba: tint_rgba };
                }
                SymbolColorRenderingMode::Hierarchical { primary_rgba } => {
                    resolved.paint = ColorLayerPaint::Solid { rgba: primary_rgba };
                    resolved.opacity =
                        (base_opacity * level.hierarchical_opacity_factor()).clamp(0.0, 1.0);
                }
                SymbolColorRenderingMode::Palette { colors_rgba } => {
                    let rgba = match level {
                        SymbolHierarchyLevel::Primary => colors_rgba[0],
                        SymbolHierarchyLevel::Secondary => colors_rgba[1],
                        SymbolHierarchyLevel::Tertiary => colors_rgba[2],
                    };
                    resolved.paint = ColorLayerPaint::Solid { rgba };
                }
            }
            resolved
        })
        .collect()
}

/// Non-destructively evaluates `SmartCornerSpec` fillets and expands open/stroked `ContourStrokeStyle`
/// and `VariableWidthStrokeSpec` contours into closed vector outlines (`SPEC-017 §2.7`).
#[must_use]
pub fn expand_non_destructive_icon_contours(
    contours: &[GlyphContour],
    smart_corners: &[SmartCornerSpec],
    contour_strokes: &BTreeMap<ContourId, ContourStrokeStyle>,
    variable_width_strokes: &[VariableWidthStrokeSpec],
    next_node_id: &mut u64,
) -> Vec<GlyphContour> {
    let cornered = evaluate_smart_corner_contours(contours, smart_corners, next_node_id);
    if contour_strokes.is_empty() && variable_width_strokes.is_empty() {
        return cornered;
    }
    let mut out = Vec::with_capacity(cornered.len());
    for gc in cornered {
        if !gc.visible || gc.nodes.len() < 2 {
            out.push(gc);
            continue;
        }
        if let Some(vw) = variable_width_strokes
            .iter()
            .find(|s| s.contour_id == gc.id)
        {
            let expanded = expand_variable_width_contour(&gc, vw, next_node_id);
            if !expanded.is_empty() {
                for (k, mut c) in expanded.into_iter().enumerate() {
                    if k == 0 {
                        c.id = gc.id;
                    }
                    out.push(c);
                }
                continue;
            }
        }
        if let Some(st) = contour_strokes.get(&gc.id).filter(|s| {
            s.width.is_finite()
                && s.height.is_finite()
                && s.width > 0.0
                && s.height > 0.0
                && s.width <= SVG_MAX_STROKE_WIDTH_UPM
                && s.height <= SVG_MAX_STROKE_WIDTH_UPM
        }) {
            let mut next_cid = gc.id.0;
            let expanded = expand_single_contour_stroke(&gc, *st, &mut next_cid, next_node_id);
            if !expanded.is_empty() {
                for (k, mut c) in expanded.into_iter().enumerate() {
                    if k == 0 {
                        c.id = gc.id;
                    }
                    out.push(c);
                }
                continue;
            }
        }
        out.push(gc);
    }
    out
}
