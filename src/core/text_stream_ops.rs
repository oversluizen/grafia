//! `SPEC-006` (`grafia v0.5.0`) core data structures and geometric operations for
//! Multi-Glyph Horizontal Text Strip (`CanvasTextStream`), Interactive Spacing (`SpacingDelta`),
//! Kerning Delta Emission (`KerningDelta`), Kerning Baseline Highlight Wedges (`KerningIndicatorQuad`),
//! and Horizontal Scanline Metrology (`SlotScanlineMeasurement`).

use crate::core::{
    extract_segments, ContourSegment, FontPoint, Glyph, GlyphContour, Node, NodeType,
};
use crate::tokens::{
    BERNSTEIN_CUBIC_COEFF, TEXT_SCANLINE_CUBIC_SUBDIVISIONS, TEXT_SCANLINE_DEDUP_EPS_UPM,
    TEXT_SCANLINE_T_EPS,
};

/// A single slot in the multi-glyph horizontal text strip (`CanvasTextStream`).
/// Supplied by the Host Application / Consumer Shell; `grafia` computes `origin_x_upm`
/// along the continuous baseline and emits `SpacingDelta` / `KerningDelta` events.
#[derive(Clone, Debug, PartialEq)]
pub struct GlyphSlot {
    pub glyph: Glyph,
    /// Effective kerning applied before this slot (`kern(i-1, i)` in UPM), `0` for slot `0`.
    pub kern_with_prev: i32,
    /// Computed horizontal pen origin `X_i` in UPM along the shared baseline (`y = 0`).
    pub origin_x_upm: f64,
}

impl GlyphSlot {
    #[must_use]
    pub fn new(glyph: Glyph, kern_with_prev: i32) -> Self {
        Self {
            glyph,
            kern_with_prev,
            origin_x_upm: 0.0,
        }
    }
}

/// Pure spacing mutation event emitted by `grafia` for the Host Font Model Backend.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SpacingDelta {
    pub slot_index: usize,
    pub glyph_name: String,
    pub lsb_delta_upm: i32,
    pub rsb_delta_upm: i32,
    pub width_delta_upm: i32,
}

/// Pure kerning mutation event emitted by `grafia` for the Host Font Model Backend.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct KerningDelta {
    pub left_slot_index: usize,
    pub right_slot_index: usize,
    pub left_glyph: String,
    pub right_glyph: String,
    pub delta_upm: i32,
    pub new_kern_upm: i32,
}

/// Kerning highlight wedge geometry produced for adjacent kerned slots `(k, k+1)`.
#[derive(Clone, Debug, PartialEq)]
pub struct KerningIndicatorQuad {
    pub left_slot_index: usize,
    pub right_slot_index: usize,
    pub kerning_value_upm: i32,
    /// Four corners `[bottom_left, bottom_right, top_right, top_left]` in UPM (sheared by italic angle).
    pub corners_upm: [FontPoint; 4],
}

/// Per-slot horizontal scanline measurement results.
#[derive(Clone, Debug, PartialEq)]
pub struct SlotScanlineMeasurement {
    pub slot_index: usize,
    pub y_upm: f64,
    pub lsb_x_upm: f64,
    pub rsb_x_upm: f64,
    /// Sorted contour intersection x-coordinates in global strip UPM space.
    pub contour_hit_xs_upm: Vec<f64>,
    /// Distance from sheared LSB line to first contour hit (`None` if no contour hit).
    pub measured_lsb_upm: Option<f64>,
    /// Distance from last contour hit to sheared RSB line (`None` if no contour hit).
    pub measured_rsb_upm: Option<f64>,
}

/// Multi-glyph horizontal text buffer, caret/selection state, and emitted host delta queue.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct CanvasTextStream {
    pub slots: Vec<GlyphSlot>,
    /// Index of the active editable glyph slot (`0..slots.len()`).
    pub active_slot_index: usize,
    /// Caret index (`0..=slots.len()`). Active glyph for spacing/kerning is the slot right of the caret.
    pub caret_index: usize,
    /// Optional multi-slot selection range `(start_slot, end_slot_exclusive)`.
    pub selection_range: Option<(usize, usize)>,
    /// Emitted spacing delta events ready to be drained by the Host Font Model Backend.
    pub pending_spacing_deltas: Vec<SpacingDelta>,
    /// Emitted kerning delta events ready to be drained by the Host Font Model Backend.
    pub pending_kerning_deltas: Vec<KerningDelta>,
}

impl CanvasTextStream {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Recomputes cumulative horizontal pen origins `X_0 = 0.0`, `X_i = X_{i-1} + width_{i-1} + kern_i`.
    pub fn recompute_origins(&mut self) {
        if self.slots.is_empty() {
            self.active_slot_index = 0;
            self.caret_index = 0;
            self.selection_range = None;
            return;
        }
        self.slots[0].kern_with_prev = 0;
        self.slots[0].origin_x_upm = 0.0;
        for i in 1..self.slots.len() {
            let prev_end = self.slots[i - 1].origin_x_upm + self.slots[i - 1].glyph.width;
            self.slots[i].origin_x_upm = prev_end + f64::from(self.slots[i].kern_with_prev);
        }
        let max_slot = self.slots.len() - 1;
        if self.active_slot_index > max_slot {
            self.active_slot_index = max_slot;
        }
        if self.caret_index > self.slots.len() {
            self.caret_index = self.slots.len();
        }
        if let Some((s, e)) = self.selection_range {
            let cs = s.min(self.slots.len());
            let ce = e.min(self.slots.len());
            self.selection_range = if cs < ce { Some((cs, ce)) } else { None };
        }
    }

    /// Resolves the nearest slot boundary index (`0..=slots.len()`) for a UPM point (`SPEC-006 §2.2.2`).
    #[must_use]
    pub fn resolve_caret_index_at_upm(&self, pt_upm: FontPoint) -> usize {
        for (idx, slot) in self.slots.iter().enumerate() {
            let mid_x = slot.origin_x_upm + 0.5 * slot.glyph.width;
            if pt_upm.x.0 < mid_x {
                return idx;
            }
        }
        self.slots.len()
    }

    /// Resolves the slot index (`0..slots.len()`) hit by a double-click at `pt_upm` (`SPEC-006 §2.1.2`).
    #[must_use]
    pub fn resolve_slot_at_upm(&self, pt_upm: FontPoint) -> Option<usize> {
        if self.slots.is_empty() {
            return None;
        }
        let mut best_containing: Option<(f64, usize)> = None;
        let mut best_any: Option<(f64, usize)> = None;
        for (idx, slot) in self.slots.iter().enumerate() {
            let start_x = slot.origin_x_upm;
            let end_x = start_x + slot.glyph.width;
            let mid_x = start_x + 0.5 * slot.glyph.width;
            let dist = (pt_upm.x.0 - mid_x).abs();
            if pt_upm.x.0 >= start_x
                && pt_upm.x.0 <= end_x
                && best_containing.is_none_or(|(b_dist, _)| dist < b_dist)
            {
                best_containing = Some((dist, idx));
            }
            if best_any.is_none_or(|(b_dist, _)| dist < b_dist) {
                best_any = Some((dist, idx));
            }
        }
        best_containing.or(best_any).map(|(_, idx)| idx)
    }

    /// Computes kerning highlight quads for all adjacent slot pairs `(k, k+1)` with `kern_with_prev != 0`.
    #[must_use]
    pub fn compute_kerning_indicator_quads(
        &self,
        descender: f64,
        ascender: f64,
        x_height: f64,
        italic_angle_deg: f64,
    ) -> Vec<KerningIndicatorQuad> {
        if self.slots.len() < 2 {
            return Vec::new();
        }
        let half_xh = x_height * 0.5;
        let tan_a = italic_angle_deg.to_radians().tan();
        let dx_bot = (descender - half_xh) * tan_a;
        let dx_top = (ascender - half_xh) * tan_a;
        let mut quads = Vec::new();

        for k in 0..(self.slots.len() - 1) {
            let kern = self.slots[k + 1].kern_with_prev;
            if kern == 0 {
                continue;
            }
            let unkerned_edge = self.slots[k].origin_x_upm + self.slots[k].glyph.width;
            let kerned_origin = self.slots[k + 1].origin_x_upm;
            let x_start = unkerned_edge.min(kerned_origin);
            let x_end = unkerned_edge.max(kerned_origin);
            quads.push(KerningIndicatorQuad {
                left_slot_index: k,
                right_slot_index: k + 1,
                kerning_value_upm: kern,
                corners_upm: [
                    FontPoint::new(x_start + dx_bot, descender),
                    FontPoint::new(x_end + dx_bot, descender),
                    FontPoint::new(x_end + dx_top, ascender),
                    FontPoint::new(x_start + dx_top, ascender),
                ],
            });
        }
        quads
    }

    /// Computes horizontal scanline intersections and sheared LSB/RSB distances at `y_upm` for all slots.
    #[must_use]
    pub fn compute_scanline_measurements(
        &self,
        y_upm: f64,
        x_height: f64,
        italic_angle_deg: f64,
    ) -> Vec<SlotScanlineMeasurement> {
        let half_xh = x_height * 0.5;
        let shear_dx = (y_upm - half_xh) * italic_angle_deg.to_radians().tan();
        self.slots
            .iter()
            .enumerate()
            .map(|(slot_index, slot)| {
                let lsb_x_upm = slot.origin_x_upm + shear_dx;
                let rsb_x_upm = slot.origin_x_upm + slot.glyph.width + shear_dx;
                let contour_hit_xs_upm = intersect_contours_at_horizontal_y(
                    &slot.glyph.contours,
                    slot.origin_x_upm,
                    y_upm,
                );
                let measured_lsb_upm = contour_hit_xs_upm.first().map(|&first| first - lsb_x_upm);
                let measured_rsb_upm = contour_hit_xs_upm.last().map(|&last| rsb_x_upm - last);
                SlotScanlineMeasurement {
                    slot_index,
                    y_upm,
                    lsb_x_upm,
                    rsb_x_upm,
                    contour_hit_xs_upm,
                    measured_lsb_upm,
                    measured_rsb_upm,
                }
            })
            .collect()
    }
}

/// Computes `(lsb_upm, rsb_upm)` for the given contours and `advance_width`.
#[must_use]
pub fn compute_glyph_lsb_and_rsb(contours: &[Vec<Node>], advance_width: f64) -> (f64, f64) {
    let mut min_x = f64::INFINITY;
    let mut max_x = f64::NEG_INFINITY;
    for contour in contours {
        for node in contour {
            if node.node_type != NodeType::OffCurve {
                min_x = min_x.min(node.point.x.0);
                max_x = max_x.max(node.point.x.0);
            }
        }
    }
    if !min_x.is_finite() || !max_x.is_finite() {
        for contour in contours {
            for node in contour {
                min_x = min_x.min(node.point.x.0);
                max_x = max_x.max(node.point.x.0);
            }
        }
    }
    if !min_x.is_finite() || !max_x.is_finite() {
        return (0.0, advance_width);
    }
    (min_x, advance_width - max_x)
}

fn eval_cubic_pt(p0: FontPoint, p1: FontPoint, p2: FontPoint, p3: FontPoint, t: f64) -> FontPoint {
    let mt = 1.0 - t;
    let mt2 = mt * mt;
    let t2 = t * t;
    let c0 = mt2 * mt;
    let c1 = BERNSTEIN_CUBIC_COEFF * mt2 * t;
    let c2 = BERNSTEIN_CUBIC_COEFF * mt * t2;
    let c3 = t2 * t;
    FontPoint::new(
        c0 * p0.x.0 + c1 * p1.x.0 + c2 * p2.x.0 + c3 * p3.x.0,
        c0 * p0.y.0 + c1 * p1.y.0 + c2 * p2.y.0 + c3 * p3.y.0,
    )
}

fn intersect_segment_at_y(
    a: FontPoint,
    b: FontPoint,
    origin_x_upm: f64,
    y_upm: f64,
    hits: &mut Vec<f64>,
) {
    let dy = b.y.0 - a.y.0;
    if dy.abs() <= TEXT_SCANLINE_T_EPS {
        return;
    }
    let y_min = a.y.0.min(b.y.0);
    let y_max = a.y.0.max(b.y.0);
    if y_upm >= y_min - TEXT_SCANLINE_T_EPS && y_upm <= y_max + TEXT_SCANLINE_T_EPS {
        let t = ((y_upm - a.y.0) / dy).clamp(0.0, 1.0);
        let x = origin_x_upm + a.x.0 + t * (b.x.0 - a.x.0);
        hits.push(x);
    }
}

/// Computes sorted, deduplicated horizontal scanline intersections at `y_upm` for a glyph's contours.
#[must_use]
pub fn intersect_contours_at_horizontal_y(
    contours: &[GlyphContour],
    origin_x_upm: f64,
    y_upm: f64,
) -> Vec<f64> {
    let mut hits = Vec::new();
    let n_steps = TEXT_SCANLINE_CUBIC_SUBDIVISIONS as f64;
    for gc in contours {
        if !gc.visible || gc.nodes.len() < 2 {
            continue;
        }
        for seg in extract_segments(&gc.nodes, gc.closed) {
            match seg {
                ContourSegment::Line {
                    start_idx, end_idx, ..
                } => {
                    intersect_segment_at_y(
                        gc.nodes[start_idx].point,
                        gc.nodes[end_idx].point,
                        origin_x_upm,
                        y_upm,
                        &mut hits,
                    );
                }
                ContourSegment::Cubic {
                    start_idx,
                    h1_idx,
                    h2_idx,
                    end_idx,
                    ..
                } => {
                    let p0 = gc.nodes[start_idx].point;
                    let p1 = gc.nodes[h1_idx].point;
                    let p2 = gc.nodes[h2_idx].point;
                    let p3 = gc.nodes[end_idx].point;
                    for i in 0..TEXT_SCANLINE_CUBIC_SUBDIVISIONS {
                        let t0 = (i as f64) / n_steps;
                        let t1 = ((i + 1) as f64) / n_steps;
                        let a = eval_cubic_pt(p0, p1, p2, p3, t0);
                        let b = eval_cubic_pt(p0, p1, p2, p3, t1);
                        intersect_segment_at_y(a, b, origin_x_upm, y_upm, &mut hits);
                    }
                }
            }
        }
    }
    hits.sort_by(f64::total_cmp);
    let mut deduped: Vec<f64> = Vec::with_capacity(hits.len());
    for x in hits {
        if deduped
            .last()
            .is_none_or(|&prev| (x - prev).abs() > TEXT_SCANLINE_DEDUP_EPS_UPM)
        {
            deduped.push(x);
        }
    }
    deduped
}
