//! `SPEC-013 §2.5`: Scanline Optical Inter-Shape Rhythm Integrals (`compute_pair_optical_whitespace`)
//! and Clean 1-Axis Live Mirror Drawing (`ActiveMirrorAxis`).

use crate::core::bezier_ops::{Node, NodeId};
use crate::core::coordinates::FontPoint;
use crate::core::glyph_model::{ContourId, GlyphContour};
use crate::core::math_consts::{
    DEFAULT_BASELINE_UPM, DEFAULT_CAP_HEIGHT_UPM, DEG_TO_RAD_F64, OPTICAL_DEFAULT_MAX_CAVITY_UPM,
    OPTICAL_DEFAULT_SAMPLE_COUNT, OPTICAL_MIN_SAMPLE_COUNT,
};
use crate::core::segment_slide::reverse_contour_nodes;
use crate::core::text_stream_ops::intersect_contours_at_horizontal_y;
use serde::{Deserialize, Serialize};

/// Configuration for scanline optical inter-contour whitespace quadrature (`SPEC-013 §2.5.1`).
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct OpticalPairConfig {
    pub y_min_upm: f64,
    pub y_max_upm: f64,
    pub sample_count: usize,
    pub max_cavity_depth_upm: f64,
    pub italic_angle_deg: f64,
}

impl Default for OpticalPairConfig {
    fn default() -> Self {
        Self {
            y_min_upm: DEFAULT_BASELINE_UPM,
            y_max_upm: DEFAULT_CAP_HEIGHT_UPM,
            sample_count: OPTICAL_DEFAULT_SAMPLE_COUNT,
            max_cavity_depth_upm: OPTICAL_DEFAULT_MAX_CAVITY_UPM,
            italic_angle_deg: 0.0,
        }
    }
}

/// Deterministic scanline optical whitespace volume, minimum clearance, and water-fill spans (`SPEC-013 §2.5.1`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct OpticalPairMetrics {
    pub optical_area_upm2: f64,
    pub min_clearance_upm: f64,
    pub min_clearance_y_upm: f64,
    pub scanline_spans: Vec<(f64, f64, f64)>,
}

/// Clean 1-axis live mirror reflection mode for drawing tools (`SPEC-013 §2.5.2`).
#[derive(Debug, Clone, Copy, PartialEq, Default, Serialize, Deserialize)]
pub enum ActiveMirrorAxis {
    #[default]
    None,
    Vertical {
        x_upm: f64,
    },
    Horizontal {
        y_upm: f64,
    },
}

/// Evaluates scanline optical inter-shape whitespace area and minimum clearance between `(left_contours, right_contours)` (`SPEC-013 §2.5.1`).
#[must_use]
pub fn compute_pair_optical_whitespace(
    left_contours: &[GlyphContour],
    left_advance_upm: f64,
    right_contours: &[GlyphContour],
    right_offset_x_upm: f64,
    cfg: &OpticalPairConfig,
) -> OpticalPairMetrics {
    debug_assert!(left_advance_upm.is_finite() && right_offset_x_upm.is_finite());
    let y0 = cfg.y_min_upm.min(cfg.y_max_upm);
    let y1 = cfg.y_min_upm.max(cfg.y_max_upm);
    let n = cfg.sample_count.max(OPTICAL_MIN_SAMPLE_COUNT);
    let max_depth = cfg.max_cavity_depth_upm.abs();
    let tan_italic = (cfg.italic_angle_deg * DEG_TO_RAD_F64).tan();

    let mut raw_rows: Vec<(f64, Option<f64>, Option<f64>)> = Vec::with_capacity(n);
    let mut global_l_max = f64::NEG_INFINITY;
    let mut global_r_min = f64::INFINITY;
    let mut min_clearance_upm = f64::INFINITY;
    let mut min_clearance_y_upm = y0;

    for i in 0..n {
        let frac = (i as f64) / ((n - 1) as f64);
        let y = y0 + frac * (y1 - y0);
        let shear_dx = y * tan_italic;
        let l_hits = intersect_contours_at_horizontal_y(left_contours, -shear_dx, y);
        let r_hits =
            intersect_contours_at_horizontal_y(right_contours, right_offset_x_upm - shear_dx, y);
        let l_exit_raw = l_hits.last().copied();
        let r_entry_raw = r_hits.first().copied();
        if let Some(lx) = l_exit_raw {
            global_l_max = global_l_max.max(lx);
        }
        if let Some(rx) = r_entry_raw {
            global_r_min = global_r_min.min(rx);
        }
        if let (Some(lx), Some(rx)) = (l_exit_raw, r_entry_raw) {
            let clearance = rx - lx;
            if clearance < min_clearance_upm {
                min_clearance_upm = clearance;
                min_clearance_y_upm = y;
            }
        }
        raw_rows.push((y, l_exit_raw, r_entry_raw));
    }

    if !global_l_max.is_finite() {
        global_l_max = left_advance_upm;
    }
    if !global_r_min.is_finite() {
        global_r_min = right_offset_x_upm;
    }
    if !min_clearance_upm.is_finite() {
        min_clearance_upm = global_r_min - global_l_max;
    }

    let l_floor = global_l_max - max_depth;
    let r_ceil = global_r_min + max_depth;
    let mut scanline_spans: Vec<(f64, f64, f64)> = Vec::with_capacity(n);
    for (y, l_raw, r_raw) in raw_rows {
        let shear_dx = y * tan_italic;
        let lx = l_raw.unwrap_or(l_floor).max(l_floor) + shear_dx;
        let rx = r_raw.unwrap_or(r_ceil).min(r_ceil) + shear_dx;
        scanline_spans.push((y, lx, rx));
    }

    let mut optical_area_upm2 = 0.0;
    for pair in scanline_spans.windows(2) {
        let (ya, lxa, rxa) = pair[0];
        let (yb, lxb, rxb) = pair[1];
        let wa = (rxa - lxa).max(0.0);
        let wb = (rxb - lxb).max(0.0);
        optical_area_upm2 += 0.5 * (wa + wb) * (yb - ya).abs();
    }

    OpticalPairMetrics {
        optical_area_upm2,
        min_clearance_upm,
        min_clearance_y_upm,
        scanline_spans,
    }
}

/// Reflects `contour` across `axis` with fresh node IDs and winding-preserving orientation (`SPEC-013 §2.5.2`).
#[must_use]
pub fn reflect_contour_across_mirror_axis(
    contour: &GlyphContour,
    axis: ActiveMirrorAxis,
    next_node_id: &mut u64,
) -> Option<GlyphContour> {
    if axis == ActiveMirrorAxis::None || contour.nodes.is_empty() {
        return None;
    }
    let mut reflected_nodes: Vec<Node> = contour
        .nodes
        .iter()
        .map(|n| {
            let id = NodeId(*next_node_id);
            *next_node_id += 1;
            let pt = match axis {
                ActiveMirrorAxis::None => n.point,
                ActiveMirrorAxis::Vertical { x_upm } => {
                    FontPoint::new(2.0 * x_upm - n.point.x.0, n.point.y.0)
                }
                ActiveMirrorAxis::Horizontal { y_upm } => {
                    FontPoint::new(n.point.x.0, 2.0 * y_upm - n.point.y.0)
                }
            };
            Node::new(id, pt.x.0, pt.y.0, n.node_type)
        })
        .collect();
    if contour.closed {
        reverse_contour_nodes(&mut reflected_nodes, true);
    }
    let mut mirrored = GlyphContour::new(ContourId(contour.id.0 + 1), reflected_nodes);
    mirrored.closed = contour.closed;
    mirrored.role = contour.role;
    mirrored.visible = contour.visible;
    mirrored.locked = contour.locked;
    Some(mirrored)
}
