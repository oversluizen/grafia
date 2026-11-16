//! Host-Port ("Stekkers") request, spacing, and kerning event pump for `MockFontHost`.

use super::MockFontHost;
use grafia::adapters::GrafiaCanvasState;
use grafia::core::{CanvasHostRequest, TextStreamEditKind, TextStreamEditRequest};

impl MockFontHost {
    pub fn poll_host_ports(&mut self, state: &mut GrafiaCanvasState) {
        for req in state.drain_host_requests() {
            self.handle_canvas_host_request(state, req);
        }
        for sp in state.drain_spacing_deltas() {
            self.glyph_library
                .insert(state.glyph.name.clone(), state.glyph.clone());
            self.push_log(format!(
                "HostPort: SpacingDelta(slot={}, dLSB={:+}, dRSB={:+})",
                sp.slot_index, sp.lsb_delta_upm, sp.rsb_delta_upm
            ));
        }
        for kd in state.drain_kerning_deltas() {
            self.kerning_pairs.insert(
                (kd.left_glyph.clone(), kd.right_glyph.clone()),
                kd.new_kern_upm,
            );
            self.push_log(format!(
                "HostPort: KerningDelta({},{}: {:+} -> {})",
                kd.left_glyph, kd.right_glyph, kd.delta_upm, kd.new_kern_upm
            ));
        }
        for hap in state.drain_haptic_events() {
            self.push_log(format!("HapticPort: {hap:?}"));
        }
    }

    fn handle_canvas_host_request(
        &mut self,
        state: &mut GrafiaCanvasState,
        req: CanvasHostRequest,
    ) {
        match req {
            CanvasHostRequest::ResetDefaultAnchors { glyph_name } => {
                let anchors = self.default_anchors_for(&glyph_name, state.glyph.width);
                let marks = self.default_ghost_marks();
                let (na, nm) = (anchors.len(), marks.len());
                state.set_anchors_with_undo(anchors);
                state.set_ghost_mark_paths(marks);
                state.show_mark_cloud = true;
                self.push_log(format!(
                    "HostPort: ResetDefaultAnchors({glyph_name}) -> {na} anchors + {nm} ghost marks injected"
                ));
            }
            CanvasHostRequest::EditTextStream(edit_req) => {
                self.handle_edit_text_stream_request(state, edit_req);
            }
            CanvasHostRequest::OpenComponentSource { component_name, .. } => {
                if let Some(src) = self.glyph_library.get(&component_name).cloned() {
                    let before = state.snapshot();
                    state.glyph = src.clone();
                    state.contours = src.contours.iter().map(|c| c.nodes.clone()).collect();
                    state.components.clear();
                    state.selected_nodes.clear();
                    state.set_anchors(self.default_anchors_for(&component_name, src.width));
                    state.sync_paths_from_contours();
                    state.record_mutation("Open Component Source", before);
                    state.assert_invariants();
                }
                self.push_log(format!("HostPort: OpenComponentSource({component_name})"));
            }
            CanvasHostRequest::UpdateSmartComponentAxis {
                axis_id, new_value, ..
            } => {
                self.push_log(format!("HostPort: SmartAxis({axis_id}={new_value:.1})"));
            }
            CanvasHostRequest::SelectOrMutateExtraNode { key } => {
                self.push_log(format!("HostPort: ExtraNode({key:?})"));
            }
            CanvasHostRequest::MutateColorSublayer {
                sublayer_index,
                mutation,
            } => {
                self.push_log(format!(
                    "HostPort: ColorSublayer({sublayer_index}, {mutation:?})"
                ));
            }
            CanvasHostRequest::MutatePostScriptHint(m) => {
                self.push_log(format!("HostPort: PostScriptHint({m:?})"));
            }
            CanvasHostRequest::MutateTrueTypeInstruction(m) => {
                self.push_log(format!("HostPort: TrueTypeInstruction({m:?})"));
            }
            CanvasHostRequest::MutateHoiLayerNode {
                layer_index,
                contour_index,
                node_id,
                new_pos,
            } => {
                self.push_log(format!(
                    "HostPort: HoiLayerNode(L{layer_index}:C{contour_index}:{node_id:?} -> {:.1},{:.1})",
                    new_pos.x.0, new_pos.y.0
                ));
            }
            CanvasHostRequest::PropagateStemDelta {
                orientation,
                old_width_upm,
                new_width_upm,
                angle_delta_deg,
            } => {
                self.push_log(format!(
                    "HostPort: PropagateStemDelta({orientation:?}: {old_width_upm:.1} -> {new_width_upm:.1}, dAngle={angle_delta_deg:+.1}°)"
                ));
            }
        }
    }

    fn handle_edit_text_stream_request(
        &mut self,
        state: &mut GrafiaCanvasState,
        edit_req: TextStreamEditRequest,
    ) {
        let mut chars: Vec<char> = if !state.text_stream.slots.is_empty()
            && state.text_stream.slots.len() != self.text_buffer.chars().count()
        {
            state
                .text_stream
                .slots
                .iter()
                .map(|s| s.glyph.name.chars().next().unwrap_or('?'))
                .collect()
        } else {
            self.text_buffer.chars().collect()
        };
        let mut caret = edit_req.caret_index.min(chars.len());
        if let Some((s0, s1)) = edit_req.selection_range {
            let (lo, hi) = (s0.min(s1).min(chars.len()), s0.max(s1).min(chars.len()));
            if lo < hi {
                chars.drain(lo..hi);
                caret = lo;
            }
        }
        match &edit_req.edit {
            TextStreamEditKind::InsertText(t) => {
                for ch in t.chars() {
                    chars.insert(caret, ch);
                    caret += 1;
                }
            }
            TextStreamEditKind::Backspace if edit_req.selection_range.is_none() && caret > 0 => {
                caret -= 1;
                chars.remove(caret);
            }
            TextStreamEditKind::DeleteForward
                if edit_req.selection_range.is_none() && caret < chars.len() =>
            {
                chars.remove(caret);
            }
            _ => {}
        }
        self.text_buffer = chars.iter().collect();
        let slots = self.build_slots_for_text(&self.text_buffer);
        let active = caret.saturating_sub(1).min(slots.len().saturating_sub(1));
        state.set_text_stream_slots_with_caret(slots, active, caret);
        self.push_log(format!(
            "HostPort: EditTextStream({:?}) -> \"{}\"",
            edit_req.edit, self.text_buffer
        ));
    }
}
