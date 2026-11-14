//! Central state invariant validation for `GrafiaCanvasState` (`STD-ARCH §3.5`).

use crate::adapters::state::GrafiaCanvasState;
use crate::core::NodeType;
use crate::interaction::PenToolState;

impl GrafiaCanvasState {
    fn validate_contour_and_node_invariants(
        &self,
    ) -> Result<std::collections::HashSet<crate::core::NodeId>, String> {
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
            Self::validate_single_contour_topology(idx, contour, closed, pen_idle)?;
        }
        Ok(all_ids)
    }

    fn validate_single_contour_topology(
        idx: usize,
        contour: &[crate::core::Node],
        closed: bool,
        pen_idle: bool,
    ) -> Result<(), String> {
        if !pen_idle {
            return Ok(());
        }
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
        if !closed && !contour.is_empty() {
            if contour
                .first()
                .is_some_and(|n| n.node_type == NodeType::OffCurve)
            {
                return Err(format!(
                    "Invariant violated: open contour[{idx}] starts with an OffCurve node"
                ));
            }
            if contour
                .last()
                .is_some_and(|n| n.node_type == NodeType::OffCurve)
            {
                return Err(format!(
                    "Invariant violated: open contour[{idx}] ends with an OffCurve node"
                ));
            }
            let mut off_run = 0usize;
            for node in contour {
                if node.node_type == NodeType::OffCurve {
                    off_run += 1;
                } else {
                    if off_run > 2 {
                        return Err(format!(
                            "Invariant violated: open contour[{idx}] has interior OffCurve run of length {off_run} (expected 0, 1, or 2)"
                        ));
                    }
                    off_run = 0;
                }
            }
        }
        Ok(())
    }

    fn validate_selection_and_pen_pointers(
        &self,
        all_ids: &std::collections::HashSet<crate::core::NodeId>,
    ) -> Result<(), String> {
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
        for (label, idx_opt) in [
            ("active_open_contour", self.active_open_contour),
            (
                "pending_pen_bcp_out.0",
                self.pending_pen_bcp_out.map(|(i, _)| i),
            ),
            (
                "pending_pen_first_bcp_in.0",
                self.pending_pen_first_bcp_in.map(|(i, _)| i),
            ),
        ] {
            if let Some(c_idx) = idx_opt {
                if c_idx >= self.contours.len() {
                    return Err(format!(
                        "Invariant violated: {label} ({c_idx}) >= contours.len() ({})",
                        self.contours.len()
                    ));
                }
                if self.pen_state == PenToolState::Idle && self.is_contour_closed(c_idx) {
                    return Err(format!(
                        "Invariant violated: {label} ({c_idx}) points to a closed contour"
                    ));
                }
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
        if let Some(ref hit) = self.hovered_segment_hit {
            if hit.contour_idx >= self.contours.len() {
                return Err(format!(
                    "Invariant violated: hovered_segment_hit.contour_idx ({}) >= contours.len() ({})",
                    hit.contour_idx,
                    self.contours.len()
                ));
            }
        }
        Ok(())
    }

    fn validate_guides_and_text_stream(&self) -> Result<(), String> {
        let mut seen_guides = std::collections::HashSet::new();
        for g in self.guides.iter().chain(self.global_guides.iter()) {
            if !seen_guides.insert(g.id) {
                return Err(format!("Invariant violated: duplicate GuideId {:?}", g.id));
            }
            if !g.position.x.0.is_finite()
                || !g.position.y.0.is_finite()
                || !g.angle_deg.is_finite()
            {
                return Err(format!(
                    "Invariant violated: non-finite geometry in Guide {:?}",
                    g.id
                ));
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
        if !self.text_stream.slots.is_empty() {
            if self.text_stream.active_slot_index >= self.text_stream.slots.len() {
                return Err(format!(
                    "Invariant violated: active_slot_index ({}) >= slots.len() ({})",
                    self.text_stream.active_slot_index,
                    self.text_stream.slots.len()
                ));
            }
            if self.text_stream.caret_index > self.text_stream.slots.len() {
                return Err(format!(
                    "Invariant violated: caret_index ({}) > slots.len() ({})",
                    self.text_stream.caret_index,
                    self.text_stream.slots.len()
                ));
            }
            let eps = crate::tokens::TEXT_STREAM_ORIGIN_INVARIANT_EPS_UPM;
            if self.text_stream.writing_direction == crate::core::WritingDirection::TopToBottom {
                let mut expected_y = 0.0;
                for (idx, slot) in self.text_stream.slots.iter().enumerate() {
                    if idx > 0 {
                        expected_y -= self.text_stream.slots[idx - 1].effective_advance_height()
                            + f64::from(slot.kern_with_prev);
                    }
                    if (slot.origin_y_upm - expected_y).abs() > eps {
                        return Err(format!(
                            "Invariant violated: slot[{idx}].origin_y_upm ({}) != expected ({expected_y})",
                            slot.origin_y_upm
                        ));
                    }
                }
            } else {
                let mut expected_origin = 0.0;
                for (idx, slot) in self.text_stream.slots.iter().enumerate() {
                    if idx > 0 {
                        expected_origin += self.text_stream.slots[idx - 1].glyph.width
                            + f64::from(slot.kern_with_prev);
                    }
                    if (slot.origin_x_upm - expected_origin).abs() > eps {
                        return Err(format!(
                            "Invariant violated: slot[{idx}].origin_x_upm ({}) != expected ({expected_origin})",
                            slot.origin_x_upm
                        ));
                    }
                }
            }
        }
        Ok(())
    }

    fn validate_color_layers_and_reference_frame(&self) -> Result<(), String> {
        for (idx, sub) in self.color_sublayers.iter().enumerate() {
            if !sub.opacity.is_finite() || !(0.0..=1.0).contains(&sub.opacity) {
                return Err(format!(
                    "Invariant violated: color_sublayers[{idx}].opacity ({}) out of [0.0, 1.0]",
                    sub.opacity
                ));
            }
            if let Some(stops) = crate::core::gradient_stops_slice(&sub.paint) {
                for (s_idx, st) in stops.iter().enumerate() {
                    if !st.offset.is_finite() || !(0.0..=1.0).contains(&st.offset) {
                        return Err(format!(
                            "Invariant violated: color_sublayers[{idx}] stop[{s_idx}].offset ({}) out of [0.0, 1.0]",
                            st.offset
                        ));
                    }
                }
            }
        }
        if let Some(sel_sub) = self.drag_transients.selected_color_sublayer {
            if sel_sub >= self.color_sublayers.len() {
                return Err(format!(
                    "Invariant violated: selected_color_sublayer ({sel_sub}) >= color_sublayers.len()"
                ));
            }
        }
        if let Some((sub_idx, stop_idx)) = self.drag_transients.selected_gradient_stop {
            let valid = self
                .color_sublayers
                .get(sub_idx)
                .and_then(|s| crate::core::gradient_stops_slice(&s.paint))
                .is_some_and(|stops| stop_idx < stops.len());
            if !valid {
                return Err(format!(
                    "Invariant violated: selected_gradient_stop ({sub_idx}, {stop_idx}) out of bounds"
                ));
            }
        }
        if let Some(drag) = self.drag_transients.active_gradient_drag {
            if drag.sublayer_index >= self.color_sublayers.len() {
                return Err(format!(
                    "Invariant violated: active_gradient_drag.sublayer_index ({}) >= color_sublayers.len()",
                    drag.sublayer_index
                ));
            }
        }
        if let Some(ref frame) = self.custom_reference_frame {
            for r in &frame.keyline_circles_radii_upm {
                if !r.is_finite() {
                    return Err("Invariant violated: non-finite keyline circle radius".into());
                }
            }
        }
        Ok(())
    }

    fn validate_hinting_and_hoi(&self) -> Result<(), String> {
        for hint in &self.postscript_hints {
            if !hint.origin_upm.is_finite() || !hint.width_upm.is_finite() {
                return Err("Invariant violated: non-finite PostScriptHint geometry".into());
            }
        }
        if let Some(sel_id) = self.selected_postscript_hint {
            if !self.postscript_hints.iter().any(|h| h.id == sel_id) {
                return Err(format!(
                    "Invariant violated: selected_postscript_hint ({sel_id}) does not exist"
                ));
            }
        }
        if self.truetype_instructor_ppem == 0 {
            return Err("Invariant violated: truetype_instructor_ppem == 0".into());
        }
        for instr in &self.truetype_instructions {
            if instr.ppem == 0 || !instr.delta_pixels.is_finite() {
                return Err("Invariant violated: invalid TrueTypeVisualInstruction".into());
            }
        }
        if let Some(sel_tt) = self.selected_truetype_instruction {
            if !self.truetype_instructions.iter().any(|i| i.id == sel_tt) {
                return Err(format!(
                    "Invariant violated: selected_truetype_instruction ({sel_tt}) does not exist"
                ));
            }
        }
        for set in &self.drag_transients.hoi_layer_sets {
            if !set.c1.x.0.is_finite()
                || !set.c1.y.0.is_finite()
                || !set.c2.x.0.is_finite()
                || !set.c2.y.0.is_finite()
            {
                return Err("Invariant violated: non-finite HoiCubicLayerSet".into());
            }
        }
        if self.show_pixel_preview_ppem == Some(0) {
            return Err("Invariant violated: show_pixel_preview_ppem == Some(0)".into());
        }
        Ok(())
    }

    /// Central State Invariant Checker (`STD-ARCH §3.5`).
    pub fn validate_invariants(&self) -> Result<(), String> {
        let all_ids = self.validate_contour_and_node_invariants()?;
        self.validate_selection_and_pen_pointers(&all_ids)?;
        self.validate_guides_and_text_stream()?;
        self.validate_color_layers_and_reference_frame()?;
        self.validate_hinting_and_hoi()
    }

    /// Panics with a detailed diagnostic message if any canvas state invariant is violated.
    pub fn assert_invariants(&self) {
        let res = self.validate_invariants();
        assert!(res.is_ok(), "{res:?}");
    }
}
