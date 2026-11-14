//! PostScript Stem/Ghost Hint Bands, TrueType Instructor Instructions & Higher-Order Interpolation (`HOI`)
//! Domain Types & Pure Geometric Projections (`SPEC-010 §2.4..§2.5`).

use crate::core::bezier_ops::{Node, NodeId};
use crate::core::coordinates::FontPoint;
use crate::core::glyph_model::GlyphContour;
use crate::core::lasso_and_zoom::point_in_polygon;
use crate::core::master_compatibility::CompanionMasterLayer;
use crate::core::math_consts::{
    CURVE_FLATTEN_TOLERANCE, DEFAULT_UPM_F64, PIXEL_PREVIEW_MAX_CELLS_PER_AXIS,
    PIXEL_PREVIEW_SAMPLE_OFFSET, PS_GHOST_BOTTOM_WIDTH_UPM, PS_GHOST_TOP_WIDTH_UPM,
    TT_DOUBLE_GRID_SCALE,
};
use crate::core::spline_support::contour_to_bezpath_with_splines;
use serde::{Deserialize, Serialize};

/// Axis orientation for a PostScript stem or ghost hint (`SPEC-010 §2.4.1`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum PostScriptHintAxis {
    #[default]
    Horizontal,
    Vertical,
}

/// PostScript stem or 1-edge ghost hint band (`SPEC-010 §2.4.1`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PostScriptHint {
    pub id: u64,
    pub axis: PostScriptHintAxis,
    pub origin_upm: f64,
    pub width_upm: f64,
    pub is_ghost: bool,
    pub top_ghost: bool,
    pub start_node: Option<NodeId>,
    pub end_node: Option<NodeId>,
    pub selected: bool,
}

impl PostScriptHint {
    #[must_use]
    pub fn effective_width_upm(&self) -> f64 {
        debug_assert!(self.width_upm.is_finite());
        if self.is_ghost {
            if self.width_upm > 0.0 {
                self.width_upm
            } else if self.top_ghost {
                PS_GHOST_TOP_WIDTH_UPM
            } else {
                PS_GHOST_BOTTOM_WIDTH_UPM
            }
        } else {
            self.width_upm
        }
    }

    #[must_use]
    pub fn span_upm(&self) -> (f64, f64) {
        debug_assert!(self.origin_upm.is_finite());
        let w = self.effective_width_upm();
        let a = self.origin_upm;
        let b = if self.is_ghost && self.top_ghost {
            a - w
        } else {
            a + w
        };
        (a.min(b), a.max(b))
    }

    #[must_use]
    pub fn telemetry_pill_label(&self) -> String {
        debug_assert!(self.origin_upm.is_finite());
        if self.is_ghost {
            format!("{:.0}", self.effective_width_upm())
        } else {
            format!("{:.0}, {:.0}", self.origin_upm, self.width_upm)
        }
    }

    #[must_use]
    pub const fn polarity_badge(&self) -> &'static str {
        if self.top_ghost {
            "↑"
        } else {
            "↓"
        }
    }

    #[must_use]
    pub fn hit_test_upm(&self, pt: FontPoint, tol_upm: f64) -> bool {
        debug_assert!(tol_upm >= 0.0);
        let (lo, hi) = self.span_upm();
        let coord = match self.axis {
            PostScriptHintAxis::Horizontal => pt.y.0,
            PostScriptHintAxis::Vertical => pt.x.0,
        };
        coord >= lo - tol_upm && coord <= hi + tol_upm
    }
}

/// Mutation payload for `CanvasHostRequest::MutatePostScriptHint` (`SPEC-010 §2.4.1`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum PostScriptHintMutation {
    Add(PostScriptHint),
    Update {
        hint_id: u64,
        origin_upm: f64,
        width_upm: f64,
        top_ghost: bool,
    },
    Delete {
        hint_id: u64,
    },
}

/// TrueType visual instruction category (`SPEC-010 §2.4.2`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum TrueTypeInstructionKind {
    #[default]
    Touch,
    Anchor,
    Stem,
    Shift,
    Interpolate,
    Align,
    Delta,
}

#[allow(non_upper_case_globals)]
impl TrueTypeInstructionKind {
    pub const Snap: Self = Self::Touch;

    #[must_use]
    pub const fn badge_code(self) -> &'static str {
        match self {
            Self::Touch => "A",
            Self::Anchor => "K",
            Self::Stem => "S",
            Self::Shift => "F",
            Self::Interpolate => "G",
            Self::Align => "L",
            Self::Delta => "E",
        }
    }
}

/// TrueType grid-fitting rounding mode (`ISO/IEC 14496-22` `glyf` rounding state).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum TrueTypeRoundingGrid {
    #[default]
    ToGrid,
    ToHalfGrid,
    ToDoubleGrid,
    DownToGrid,
    UpToGrid,
    Off,
}

/// TrueType visual instruction attached to one or two contour nodes (`SPEC-010 §2.4.2`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TrueTypeVisualInstruction {
    pub id: u64,
    pub kind: TrueTypeInstructionKind,
    pub source_node: Option<NodeId>,
    pub target_node: NodeId,
    pub ppem: u16,
    pub round_mode: TrueTypeRoundingGrid,
    pub stem_index: Option<u8>,
    pub zone_index: Option<u8>,
    pub delta_pixels: f64,
    pub x_axis: bool,
    pub selected: bool,
}

/// Mutation payload for `CanvasHostRequest::MutateTrueTypeInstruction` (`SPEC-010 §2.4.2`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum TrueTypeInstructionMutation {
    Add(TrueTypeVisualInstruction),
    Update(TrueTypeVisualInstruction),
    Delete { instruction_id: u64 },
}

/// Computes the UPM-per-pixel step $\frac{\text{UPM}}{\text{PPEM}}$ (`SPEC-010 §2.4.2`).
#[must_use]
pub fn truetype_upm_per_px(upm: u16, ppem: u16) -> f64 {
    let safe_ppem = ppem.max(1);
    debug_assert!(safe_ppem > 0);
    let safe_upm = if upm == 0 {
        DEFAULT_UPM_F64
    } else {
        f64::from(upm)
    };
    safe_upm / f64::from(safe_ppem)
}

/// Grid-fits a single UPM coordinate at `(upm, ppem)` according to `round_mode` (`SPEC-010 §2.4.2`).
#[must_use]
pub fn grid_fit_coord_upm(
    coord_upm: f64,
    upm: u16,
    ppem: u16,
    round_mode: TrueTypeRoundingGrid,
) -> f64 {
    debug_assert!(coord_upm.is_finite());
    let step = truetype_upm_per_px(upm, ppem);
    if step <= 0.0 || !step.is_finite() {
        return coord_upm;
    }
    let px = coord_upm / step;
    let fitted_px = match round_mode {
        TrueTypeRoundingGrid::ToGrid => px.round(),
        TrueTypeRoundingGrid::ToHalfGrid => px.floor() + 0.5,
        TrueTypeRoundingGrid::ToDoubleGrid => {
            (px * TT_DOUBLE_GRID_SCALE).round() / TT_DOUBLE_GRID_SCALE
        }
        TrueTypeRoundingGrid::DownToGrid => px.floor(),
        TrueTypeRoundingGrid::UpToGrid => px.ceil(),
        TrueTypeRoundingGrid::Off => px,
    };
    fitted_px * step
}

/// Projects contour nodes onto the TrueType PPEM grid along `x_axis` (`true` = X, `false` = Y).
#[must_use]
pub fn grid_fit_contour_nodes(
    nodes: &[Node],
    upm: u16,
    ppem: u16,
    x_axis: bool,
    instructions: &[TrueTypeVisualInstruction],
) -> Vec<Node> {
    let safe_ppem = ppem.max(1);
    debug_assert!(safe_ppem > 0);
    let step = truetype_upm_per_px(upm, safe_ppem);
    nodes
        .iter()
        .map(|n| {
            let mut x = n.point.x.0;
            let mut y = n.point.y.0;
            let instr = instructions.iter().find(|ins| ins.target_node == n.id);
            let mode = instr.map_or(TrueTypeRoundingGrid::ToGrid, |i| i.round_mode);
            let delta_upm = instr.map_or(0.0, |i| i.delta_pixels * step);
            let use_x = instr.map_or(x_axis, |i| i.x_axis);
            if use_x {
                x = grid_fit_coord_upm(x, upm, ppem, mode) + delta_upm;
            } else {
                y = grid_fit_coord_upm(y, upm, ppem, mode) + delta_upm;
            }
            Node::new(n.id, x, y, n.node_type)
        })
        .collect()
}

/// Finds a node's UPM position by `NodeId` across `contours`.
#[must_use]
pub fn find_contour_node_upm(contours: &[Vec<Node>], id: NodeId) -> Option<FontPoint> {
    for c in contours {
        for n in c {
            if n.id == id {
                return Some(n.point);
            }
        }
    }
    None
}

/// Sibling contour set across ordered masters for Higher-Order Interpolation (`HOI`, `SPEC-010 §2.5`).
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct HoiSiblingContours {
    pub layer_names: Vec<String>,
    pub contours_per_layer: Vec<Vec<GlyphContour>>,
}

/// Cubic multi-layer control-net configuration and timing handles (`SPEC-010 §2.5`).
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct HoiCubicLayerSet {
    pub c1: FontPoint,
    pub c2: FontPoint,
}

impl Default for HoiCubicLayerSet {
    fn default() -> Self {
        Self {
            c1: FontPoint::new(0.0, 0.0),
            c2: FontPoint::new(1.0, 1.0),
        }
    }
}

/// Visual role for an `HOI` control-polygon diamond marker (`SPEC-010 §2.5.1`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HoiDiamondRole {
    PrevLayer,
    ActiveLayer,
    NextLayer,
}

/// Extracted `HOI` control-net polylines and diamond markers across ordered master layers.
#[derive(Debug, Clone, Default)]
pub struct HoiOverlayGeometry {
    pub polylines: Vec<Vec<FontPoint>>,
    pub diamonds: Vec<(FontPoint, HoiDiamondRole)>,
}

/// Builds `HoiOverlayGeometry` from `HoiSiblingContours` or `CompanionMasterLayer`s (`SPEC-010 §2.5.1`).
#[must_use]
pub fn compute_hoi_overlay_geometry(
    companion_layers: &[CompanionMasterLayer],
    sibling_contours: Option<&HoiSiblingContours>,
) -> HoiOverlayGeometry {
    let mut layer_sets: Vec<Vec<&[Node]>> = Vec::new();
    if let Some(sib) = sibling_contours {
        for layer in &sib.contours_per_layer {
            if !layer.is_empty() {
                layer_sets.push(layer.iter().map(|gc| gc.nodes.as_slice()).collect());
            }
        }
    }
    if layer_sets.len() < 2 {
        layer_sets.clear();
        for comp in companion_layers {
            if !comp.contours.is_empty() {
                layer_sets.push(comp.contours.iter().map(Vec::as_slice).collect());
            }
        }
    }
    debug_assert!(layer_sets.len() <= 1024);
    let mut out = HoiOverlayGeometry::default();
    if layer_sets.len() < 2 {
        return out;
    }
    let first_layer = &layer_sets[0];
    let last_layer_idx = layer_sets.len() - 1;
    for (c_idx, base_nodes) in first_layer.iter().enumerate() {
        for n_idx in 0..base_nodes.len() {
            let mut poly = Vec::with_capacity(layer_sets.len());
            for (l_idx, l_contours) in layer_sets.iter().enumerate() {
                let Some(c_nodes) = l_contours.get(c_idx) else {
                    break;
                };
                let Some(node) = c_nodes.get(n_idx) else {
                    break;
                };
                let pt = node.point;
                poly.push(pt);
                let role = if l_idx == 0 {
                    HoiDiamondRole::PrevLayer
                } else if l_idx == last_layer_idx {
                    HoiDiamondRole::NextLayer
                } else {
                    HoiDiamondRole::ActiveLayer
                };
                out.diamonds.push((pt, role));
            }
            if poly.len() >= 2 {
                out.polylines.push(poly);
            }
        }
    }
    out
}

/// Computes Pass-1 low-resolution pixel preview cell bounds `(x0, y0, x1, y1)` in UPM at `ppem` (`SPEC-010 §2.5.2`).
#[must_use]
pub fn compute_pixel_preview_cells_upm(
    contours: &[GlyphContour],
    upm: u16,
    ppem: u16,
) -> Vec<(f64, f64, f64, f64)> {
    let safe_ppem = ppem.max(1);
    debug_assert!(safe_ppem > 0);
    let step = truetype_upm_per_px(upm, safe_ppem);
    if step <= 0.0 || !step.is_finite() || contours.is_empty() {
        return Vec::new();
    }
    let mut polys: Vec<Vec<FontPoint>> = Vec::new();
    let mut min_x = f64::INFINITY;
    let mut min_y = f64::INFINITY;
    let mut max_x = f64::NEG_INFINITY;
    let mut max_y = f64::NEG_INFINITY;
    for gc in contours {
        if !gc.visible || gc.nodes.len() < 2 {
            continue;
        }
        let bp = contour_to_bezpath_with_splines(&gc.nodes, gc.closed, 0.0, &[]);
        let mut poly = Vec::new();
        kurbo::flatten(bp, CURVE_FLATTEN_TOLERANCE, |el| match el {
            kurbo::PathEl::MoveTo(p) | kurbo::PathEl::LineTo(p) => {
                min_x = min_x.min(p.x);
                min_y = min_y.min(p.y);
                max_x = max_x.max(p.x);
                max_y = max_y.max(p.y);
                poly.push(FontPoint::new(p.x, p.y));
            }
            _ => {}
        });
        if poly.len() >= 3 {
            polys.push(poly);
        }
    }
    if polys.is_empty() || !min_x.is_finite() {
        return Vec::new();
    }
    let gx0 = (min_x / step).floor() as i32;
    let gy0 = (min_y / step).floor() as i32;
    let gx1 = ((max_x / step).ceil() as i32).min(gx0 + PIXEL_PREVIEW_MAX_CELLS_PER_AXIS);
    let gy1 = ((max_y / step).ceil() as i32).min(gy0 + PIXEL_PREVIEW_MAX_CELLS_PER_AXIS);
    let mut cells = Vec::new();
    for gy in gy0..gy1 {
        let y0 = f64::from(gy) * step;
        let cy = (f64::from(gy) + PIXEL_PREVIEW_SAMPLE_OFFSET) * step;
        for gx in gx0..gx1 {
            let x0 = f64::from(gx) * step;
            let cx = (f64::from(gx) + PIXEL_PREVIEW_SAMPLE_OFFSET) * step;
            let sample = FontPoint::new(cx, cy);
            let mut inside = false;
            for poly in &polys {
                if point_in_polygon(sample, poly) {
                    inside = !inside;
                }
            }
            if inside {
                cells.push((x0, y0, x0 + step, y0 + step));
            }
        }
    }
    cells
}
