//! Central state invariant validation for `GlifoCanvasState` (`STD-ARCH §3.5`).

use crate::adapters::state::GlifoCanvasState;
use crate::core::NodeType;
use crate::interaction::PenToolState;

impl GlifoCanvasState {
    /// Central State Invariant Checker (`STD-ARCH §3.5`).
    ///
    /// Verifies that:
    /// 1. Parallel collections (`contours`, `paths`, `glyph.contours`) are synchronized in length,
    ///    closed flags, and node lists.
    /// 2. Any contour with `<= 1` `OnCurve` node (when `pen_state == PenToolState::Idle`) is
    ///    unclosed (`closed == false`) and has `0` `OffCurve` nodes.
    /// 3. Every `NodeId` in `selected_nodes` exists in `contours`, `selected_nodes` has no
    ///    duplicate `NodeId`s, and `contours` have no duplicate `NodeId`s.
    /// 4. `active_open_contour` (if `Some(idx)`) is in bounds and (when `pen_state == PenToolState::Idle`)
    ///    points to an open contour (`!self.is_contour_closed(idx)`).
    /// 5. For every non-empty open contour (when `pen_state == PenToolState::Idle` and
    ///    `pending_pen_bcp_out.is_none()`), its first and last nodes are `OnCurve`, and every
    ///    interior run of `OffCurve` nodes between consecutive `OnCurve` nodes has length `0` or `2`.
    pub fn validate_invariants(&self) -> Result<(), String> {
        if self.contours.len() != self.paths.len() {
            return Err(format!(
                "Invariant violated: contours.len() ({}) != paths.len() ({})",
                self.contours.len(),
                self.paths.len()
            ));
        }
        if self.contours.len() != self.glyph.contours.len() {
            return Err(format!(
                "Invariant violated: contours.len() ({}) != glyph.contours.len() ({})",
                self.contours.len(),
                self.glyph.contours.len()
            ));
        }

        let pen_idle = self.pen_state == PenToolState::Idle;

        let mut all_ids = std::collections::HashSet::new();
        for (idx, (contour, glyph_contour)) in self
            .contours
            .iter()
            .zip(self.glyph.contours.iter())
            .enumerate()
        {
            let closed = self.is_contour_closed(idx);
            if closed != glyph_contour.closed {
                return Err(format!(
                    "Invariant violated: contour[{idx}] is_contour_closed ({closed}) != glyph.contours[{idx}].closed ({})",
                    glyph_contour.closed
                ));
            }
            if contour.len() != glyph_contour.nodes.len() || contour != &glyph_contour.nodes {
                return Err(format!(
                    "Invariant violated: contours[{idx}] out of sync with glyph.contours[{idx}].nodes"
                ));
            }
            for node in contour {
                if !all_ids.insert(node.id) {
                    return Err(format!(
                        "Invariant violated: duplicate NodeId {:?} across contours",
                        node.id
                    ));
                }
            }

            if self.pen_state == PenToolState::Idle {
                let on_curve_count = contour
                    .iter()
                    .filter(|n| n.node_type != NodeType::OffCurve)
                    .count();
                let off_curve_count = contour.len().saturating_sub(on_curve_count);
                if on_curve_count <= 1 {
                    if closed {
                        return Err(format!(
                            "Invariant violated: contour[{idx}] has {on_curve_count} OnCurve node(s) but closed == true"
                        ));
                    }
                    if off_curve_count > 0 {
                        return Err(format!(
                            "Invariant violated: contour[{idx}] has {on_curve_count} OnCurve node(s) but {off_curve_count} OffCurve node(s)"
                        ));
                    }
                }
            }

            if pen_idle && !closed && !contour.is_empty() {
                if contour.first().is_some_and(|n| n.node_type == NodeType::OffCurve) {
                    return Err(format!(
                        "Invariant violated: open contour[{idx}] starts with an OffCurve node"
                    ));
                }
                if contour.last().is_some_and(|n| n.node_type == NodeType::OffCurve) {
                    return Err(format!(
                        "Invariant violated: open contour[{idx}] ends with an OffCurve node"
                    ));
                }
                let mut off_run = 0usize;
                for node in contour {
                    if node.node_type == NodeType::OffCurve {
                        off_run += 1;
                    } else {
                        if off_run != 0 && off_run != 2 {
                            return Err(format!(
                                "Invariant violated: open contour[{idx}] has interior OffCurve run of length {off_run} (expected 0 or 2)"
                            ));
                        }
                        off_run = 0;
                    }
                }
            }
        }

        let mut seen_selected = std::collections::HashSet::with_capacity(self.selected_nodes.len());
        for selected_id in &self.selected_nodes {
            if !seen_selected.insert(*selected_id) {
                return Err(format!(
                    "Invariant violated: duplicate NodeId {selected_id:?} in selected_nodes"
                ));
            }
            if !all_ids.contains(selected_id) {
                return Err(format!(
                    "Invariant violated: selected NodeId {selected_id:?} does not exist in contours"
                ));
            }
        }

        if let Some(open_idx) = self.active_open_contour {
            if open_idx >= self.contours.len() {
                return Err(format!(
                    "Invariant violated: active_open_contour ({open_idx}) >= contours.len() ({})",
                    self.contours.len()
                ));
            }
            if self.pen_state == PenToolState::Idle && self.is_contour_closed(open_idx) {
                return Err(format!(
                    "Invariant violated: active_open_contour ({open_idx}) points to a closed contour"
                ));
            }
        }

        if let Some((bcp_idx, _)) = self.pending_pen_bcp_out {
            if bcp_idx >= self.contours.len() {
                return Err(format!(
                    "Invariant violated: pending_pen_bcp_out.0 ({bcp_idx}) >= contours.len() ({})",
                    self.contours.len()
                ));
            }
            if self.pen_state == PenToolState::Idle && self.is_contour_closed(bcp_idx) {
                return Err(format!(
                    "Invariant violated: pending_pen_bcp_out.0 ({bcp_idx}) points to a closed contour"
                ));
            }
        }

        if let Some((first_in_idx, _)) = self.pending_pen_first_bcp_in {
            if first_in_idx >= self.contours.len() {
                return Err(format!(
                    "Invariant violated: pending_pen_first_bcp_in.0 ({first_in_idx}) >= contours.len() ({})",
                    self.contours.len()
                ));
            }
            if self.pen_state == PenToolState::Idle && self.is_contour_closed(first_in_idx) {
                return Err(format!(
                    "Invariant violated: pending_pen_first_bcp_in.0 ({first_in_idx}) points to a closed contour"
                ));
            }
        }

        if let Some((close_c_idx, close_id)) = self.hovered_pen_close_target {
            if close_c_idx >= self.contours.len() {
                return Err(format!(
                    "Invariant violated: hovered_pen_close_target.0 ({close_c_idx}) >= contours.len() ({})",
                    self.contours.len()
                ));
            }
            if !all_ids.contains(&close_id) {
                return Err(format!(
                    "Invariant violated: hovered_pen_close_target.1 ({close_id:?}) does not exist in contours"
                ));
            }
        }

        if let Some(close_node_id) = self.hovered_pen_close_node {
            if !all_ids.contains(&close_node_id) {
                return Err(format!(
                    "Invariant violated: hovered_pen_close_node ({close_node_id:?}) does not exist in contours"
                ));
            }
        }

        if let Some(ref hit) = self.hovered_segment_hit {
            if hit.contour_idx >= self.contours.len() {
                return Err(format!(
                    "Invariant violated: hovered_segment_hit.contour_idx ({}) >= contours.len() ({})",
                    hit.contour_idx,
                    self.contours.len()
                ));
            }
        }

        let mut seen_guides = std::collections::HashSet::new();
        for g in self.guides.iter().chain(self.global_guides.iter()) {
            if !seen_guides.insert(g.id) {
                return Err(format!("Invariant violated: duplicate GuideId {:?}", g.id));
            }
            if !g.position.x.0.is_finite() || !g.position.y.0.is_finite() || !g.angle_deg.is_finite() {
                return Err(format!("Invariant violated: non-finite geometry in Guide {:?}", g.id));
            }
        }
        if let Some(sel_g) = self.selected_guide {
            if !seen_guides.contains(&sel_g.id) {
                return Err(format!(
                    "Invariant violated: selected_guide {:?} does not exist in guides or global_guides",
                    sel_g
                ));
            }
        }

        Ok(())
    }

    /// Panics with a detailed diagnostic message if any canvas state invariant is violated.
    pub fn assert_invariants(&self) {
        if let Err(err) = self.validate_invariants() {
            panic!("{err}");
        }
    }
}
