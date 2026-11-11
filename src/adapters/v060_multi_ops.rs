//! `SPEC-007` (`grafia v0.6.0`) multi-node selection mutators (`TransformGridAnchor`, origin, scale).

use crate::adapters::state::GrafiaCanvasState;
use crate::core::{
    compute_selection_bounds, expanded_nodes_with_attached_handles, FontPoint, NodeId,
    TransformGridAnchor,
};
use crate::tokens::MULTI_SCALE_SPAN_EPS_UPM;
use std::collections::HashSet;

impl GrafiaCanvasState {
    /// Selects all nodes across all contours in the active layer.
    pub fn select_all_nodes(&mut self) {
        self.selected_guide = None;
        self.selected_nodes = self
            .contours
            .iter()
            .flat_map(|c| c.iter().map(|n| n.id))
            .collect();
        self.assert_invariants();
    }

    /// Sets the 9-point `TransformGridAnchor` reference for multi-node selection bounds.
    pub fn set_multi_selection_anchor(&mut self, anchor: TransformGridAnchor) {
        let before = self.snapshot();
        self.transform_grid_anchor = anchor;
        self.record_mutation("Set Transform Anchor", before);
        self.assert_invariants();
    }

    /// Sets whether `set_multi_selection_size` preserves aspect ratio (`proportional_scale`).
    pub fn set_multi_selection_proportional(&mut self, proportional: bool) {
        let before = self.snapshot();
        self.multi_selection_proportional = proportional;
        self.record_mutation("Set Proportional Scale", before);
        self.assert_invariants();
    }

    /// Translates all selected nodes so the active `TransformGridAnchor` origin moves to `new_origin`.
    pub fn set_multi_selection_origin(&mut self, new_origin: FontPoint) {
        if self.selected_nodes.len() < 2
            || !new_origin.x.0.is_finite()
            || !new_origin.y.0.is_finite()
        {
            return;
        }
        let Some(bounds) = compute_selection_bounds(
            &self.contours,
            &self.selected_nodes,
            0.0,
            self.metrics.x_height * 0.5,
        ) else {
            return;
        };
        let cur_origin = bounds.aligned_point(self.transform_grid_anchor.index());
        let (dx, dy) = (
            new_origin.x.0 - cur_origin.x.0,
            new_origin.y.0 - cur_origin.y.0,
        );
        let before = self.snapshot();
        let closed_flags = self.closed_flags();
        let sel_set: HashSet<NodeId> = expanded_nodes_with_attached_handles(
            &self.contours,
            &closed_flags,
            &self.selected_nodes,
            true,
        )
        .into_iter()
        .collect();
        for n in self
            .contours
            .iter_mut()
            .flatten()
            .filter(|n| sel_set.contains(&n.id))
        {
            n.point.x.0 += dx;
            n.point.y.0 += dy;
        }
        self.sync_paths_from_contours();
        self.record_mutation("Move Selection Origin", before);
        self.assert_invariants();
    }

    /// Scales all selected nodes relative to the active `TransformGridAnchor` origin (`SPEC-007 §2.2.4`).
    pub fn set_multi_selection_size(&mut self, new_width: Option<f64>, new_height: Option<f64>) {
        if self.selected_nodes.len() < 2
            || (new_width.is_none() && new_height.is_none())
            || new_width.is_some_and(|w| !w.is_finite())
            || new_height.is_some_and(|h| !h.is_finite())
        {
            return;
        }
        let Some(bounds) = compute_selection_bounds(
            &self.contours,
            &self.selected_nodes,
            0.0,
            self.metrics.x_height * 0.5,
        ) else {
            return;
        };
        let (cur_w, cur_h) = (bounds.width(), bounds.height());
        let mut sx = new_width.map_or(1.0, |w| {
            if cur_w > MULTI_SCALE_SPAN_EPS_UPM {
                w / cur_w
            } else {
                1.0
            }
        });
        let mut sy = new_height.map_or(1.0, |h| {
            if cur_h > MULTI_SCALE_SPAN_EPS_UPM {
                h / cur_h
            } else {
                1.0
            }
        });
        if self.multi_selection_proportional {
            match (new_width.is_some(), new_height.is_some()) {
                (true, false) => sy = sx,
                (false, true) => sx = sy,
                _ => {}
            }
        }
        if !sx.is_finite() || !sy.is_finite() {
            return;
        }
        let pivot = bounds.aligned_point(self.transform_grid_anchor.index());
        let before = self.snapshot();
        let closed_flags = self.closed_flags();
        let sel_set: HashSet<NodeId> = expanded_nodes_with_attached_handles(
            &self.contours,
            &closed_flags,
            &self.selected_nodes,
            true,
        )
        .into_iter()
        .collect();
        for n in self
            .contours
            .iter_mut()
            .flatten()
            .filter(|n| sel_set.contains(&n.id))
        {
            n.point.x.0 = pivot.x.0 + (n.point.x.0 - pivot.x.0) * sx;
            n.point.y.0 = pivot.y.0 + (n.point.y.0 - pivot.y.0) * sy;
        }
        self.sync_paths_from_contours();
        self.record_mutation("Scale Selection", before);
        self.assert_invariants();
    }
}
