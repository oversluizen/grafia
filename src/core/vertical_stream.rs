//! Vertical `TopToBottom` Text Stream & Vertical Metrics Math (`SPEC-010 §2.2`,
//! `ISO/IEC 14496-22:2019` `vhea` / `vmtx` / `VORG`).

use crate::core::bezier_ops::NodeType;
use crate::core::coordinates::FontPoint;
use crate::core::glyph_model::Glyph;
use crate::core::math_consts::{DEFAULT_ADVANCE_HEIGHT_UPM, DEFAULT_VERT_ORIGIN_Y_UPM};
use crate::core::text_stream_ops::{GlyphSlot, KerningIndicatorQuad};
use serde::{Deserialize, Serialize};

/// Stream layout progression direction (`SPEC-010 §2.2`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum WritingDirection {
    #[default]
    LeftToRight,
    RightToLeft,
    TopToBottom,
}

/// Axis along which spacing or kerning deltas apply (`SPEC-010 §2.2`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum MetricsAxis {
    #[default]
    Horizontal,
    Vertical,
}

/// Computes `(tsb_upm, bsb_upm)` for `glyph` given `vert_origin_y` and `advance_height`.
#[must_use]
pub fn compute_glyph_tsb_and_bsb(
    glyph: &Glyph,
    vert_origin_y: f64,
    advance_height: f64,
) -> (f64, f64) {
    debug_assert!(vert_origin_y.is_finite() && advance_height.is_finite());
    let bottom_y = vert_origin_y - advance_height;
    let mut min_y = f64::INFINITY;
    let mut max_y = f64::NEG_INFINITY;
    for contour in &glyph.contours {
        if !contour.visible {
            continue;
        }
        for node in &contour.nodes {
            if node.node_type != NodeType::OffCurve {
                min_y = min_y.min(node.point.y.0);
                max_y = max_y.max(node.point.y.0);
            }
        }
    }
    if !min_y.is_finite() || !max_y.is_finite() {
        for contour in &glyph.contours {
            for node in &contour.nodes {
                min_y = min_y.min(node.point.y.0);
                max_y = max_y.max(node.point.y.0);
            }
        }
    }
    if !min_y.is_finite() || !max_y.is_finite() {
        return (0.0, advance_height);
    }
    (vert_origin_y - max_y, min_y - bottom_y)
}

/// Recomputes slot `(origin_x_upm, origin_y_upm)` for `WritingDirection::TopToBottom`.
pub fn recompute_vertical_slot_origins(slots: &mut [GlyphSlot]) {
    let Some(first) = slots.first() else {
        return;
    };
    let axis_center_x = 0.5 * first.glyph.width;
    let mut acc_y = 0.0_f64;
    for idx in 0..slots.len() {
        if idx > 0 {
            let prev_h = slots[idx - 1].effective_advance_height();
            let kern = f64::from(slots[idx].kern_with_prev);
            acc_y -= prev_h + kern;
        }
        let w = slots[idx].glyph.width;
        slots[idx].origin_x_upm = axis_center_x - 0.5 * w;
        slots[idx].origin_y_upm = acc_y;
        debug_assert!(slots[idx].origin_x_upm.is_finite() && slots[idx].origin_y_upm.is_finite());
    }
}

/// Resolves the nearest caret boundary index `0..=slots.len()` for a vertical stream at `y_upm`.
#[must_use]
pub fn resolve_vertical_caret_index_at_upm(slots: &[GlyphSlot], y_upm: f64) -> usize {
    debug_assert!(y_upm.is_finite());
    if slots.is_empty() {
        return 0;
    }
    for (idx, slot) in slots.iter().enumerate() {
        let top_y = slot.origin_y_upm + slot.effective_vert_origin_y();
        let mid_y = top_y - 0.5 * slot.effective_advance_height();
        if y_upm >= mid_y {
            return idx;
        }
    }
    slots.len()
}

/// Resolves which vertical `GlyphSlot` contains or is closest to `pt_upm` (`SPEC-010 §2.2`).
#[must_use]
pub fn resolve_vertical_slot_at_upm(slots: &[GlyphSlot], pt_upm: FontPoint) -> Option<usize> {
    debug_assert!(pt_upm.x.0.is_finite() && pt_upm.y.0.is_finite());
    if slots.is_empty() {
        return None;
    }
    let mut best_containing: Option<(f64, usize)> = None;
    let mut best_any: Option<(f64, usize)> = None;
    for (idx, slot) in slots.iter().enumerate() {
        let x0 = slot.origin_x_upm;
        let x1 = x0 + slot.glyph.width;
        let top_y = slot.origin_y_upm + slot.effective_vert_origin_y();
        let bot_y = top_y - slot.effective_advance_height();
        let y_low = bot_y.min(slot.origin_y_upm);
        let y_high = top_y.max(slot.origin_y_upm + slot.glyph.height);
        let mid_y = 0.5 * (y_low + y_high);
        let dist = (pt_upm.y.0 - mid_y).abs();
        if pt_upm.x.0 >= x0
            && pt_upm.x.0 <= x1
            && pt_upm.y.0 >= y_low
            && pt_upm.y.0 <= y_high
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

/// Computes horizontal highlight quads between vertically stacked slots with non-zero kerning.
#[must_use]
pub fn compute_vertical_kerning_indicator_quads(slots: &[GlyphSlot]) -> Vec<KerningIndicatorQuad> {
    let mut out = Vec::new();
    for (idx, slot) in slots.iter().enumerate().skip(1) {
        let k = slot.kern_with_prev;
        if k == 0 {
            continue;
        }
        let x0 = slot.origin_x_upm;
        let x1 = x0 + slot.glyph.width;
        let seam_y = slot.origin_y_upm + slot.effective_vert_origin_y();
        let k_f = f64::from(k);
        let (y_top, y_bot) = if k < 0 {
            (seam_y - k_f, seam_y)
        } else {
            (seam_y, seam_y - k_f)
        };
        out.push(KerningIndicatorQuad {
            left_slot_index: idx - 1,
            right_slot_index: idx,
            kerning_value_upm: k,
            corners_upm: [
                FontPoint::new(x0, y_bot),
                FontPoint::new(x1, y_bot),
                FontPoint::new(x1, y_top),
                FontPoint::new(x0, y_top),
            ],
        });
    }
    out
}

/// Default vertical advance height helper.
#[must_use]
pub const fn default_advance_height_upm() -> f64 {
    DEFAULT_ADVANCE_HEIGHT_UPM
}

/// Default vertical origin Y helper.
#[must_use]
pub const fn default_vert_origin_y_upm() -> f64 {
    DEFAULT_VERT_ORIGIN_Y_UPM
}
