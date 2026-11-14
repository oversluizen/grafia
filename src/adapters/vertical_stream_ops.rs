//! Vertical `WritingDirection::TopToBottom` stream layout, caret navigation,
//! `TSB` / `BSB` spacing adjustments, and vertical kerning shortcuts (`SPEC-010 §2.2`).

use crate::adapters::state::{CanvasTool, GrafiaCanvasState};
use crate::core::math_consts::{TT_MAX_PPEM, TT_MIN_PPEM};
use crate::core::{
    compute_glyph_tsb_and_bsb, GlyphContour, HoiCubicLayerSet, HoiSiblingContours, KerningDelta,
    MetricsAxis, PostScriptHintAxis, SelectionInfo, SpacingDelta, WritingDirection,
};
use egui::Key;

impl GrafiaCanvasState {
    /// Sets the `WritingDirection` (`LeftToRight`, `RightToLeft`, or `TopToBottom`) of `text_stream`,
    /// recomputes slot origins, and bumps `content_generation`.
    pub fn set_writing_direction(&mut self, direction: WritingDirection) {
        if self.text_stream.writing_direction != direction {
            self.text_stream.writing_direction = direction;
            self.text_stream.recompute_origins();
            self.bump_content_generation();
        }
        self.assert_invariants();
    }

    /// Returns the vertical pen origin `Y_i` in UPM of the active slot (`0.0` when empty).
    #[must_use]
    pub fn active_slot_origin_y_upm(&self) -> f64 {
        self.text_stream
            .slots
            .get(self.text_stream.active_slot_index)
            .map_or(0.0, |s| s.origin_y_upm)
    }

    fn active_vertical_metrics_params(&self) -> (f64, f64) {
        if let Some(slot) = self
            .text_stream
            .slots
            .get(self.text_stream.active_slot_index)
        {
            (
                slot.effective_advance_height(),
                slot.effective_vert_origin_y(),
            )
        } else {
            let adv_h = if self.glyph.height > 0.0 {
                self.glyph.height
            } else {
                f64::from(self.glyph.upm.max(1))
            };
            (adv_h, self.metrics.ascender)
        }
    }

    /// Returns the active glyph's Top Sidebearing (`TSB`) in UPM (`SPEC-010 §2.2`).
    #[must_use]
    pub fn active_glyph_tsb(&self) -> f64 {
        let (adv_h, vert_origin_y) = self.active_vertical_metrics_params();
        compute_glyph_tsb_and_bsb(&self.export_glyph(), vert_origin_y, adv_h).0
    }

    /// Returns the active glyph's Bottom Sidebearing (`BSB`) in UPM (`SPEC-010 §2.2`).
    #[must_use]
    pub fn active_glyph_bsb(&self) -> f64 {
        let (adv_h, vert_origin_y) = self.active_vertical_metrics_params();
        compute_glyph_tsb_and_bsb(&self.export_glyph(), vert_origin_y, adv_h).1
    }

    pub(crate) fn shift_all_glyph_elements_y(&mut self, dy: f64) {
        for contour in &mut self.contours {
            for node in contour {
                node.point.y.0 += dy;
            }
        }
        for guide in &mut self.guides {
            guide.position.y.0 += dy;
        }
        for ann in &mut self.glyph.annotations {
            ann.position.y.0 += dy;
        }
        for anchor in &mut self.anchors {
            anchor.position.y.0 += dy;
        }
    }

    fn update_active_slot_advance_height(&mut self, dy: f64) {
        let (cur_h, _) = self.active_vertical_metrics_params();
        let next_h = (cur_h + dy).max(0.0);
        self.glyph.height = next_h;
        if self.text_stream.slots.is_empty() {
            return;
        }
        let active_idx = self
            .text_stream
            .active_slot_index
            .min(self.text_stream.slots.len() - 1);
        let active_name = self.glyph.name.clone();
        self.text_stream.slots[active_idx].advance_height = Some(next_h);
        if !active_name.is_empty() {
            for (idx, slot) in self.text_stream.slots.iter_mut().enumerate() {
                if idx != active_idx && slot.glyph.name == active_name {
                    slot.advance_height = Some(next_h);
                }
            }
        }
    }

    fn record_vertical_spacing_delta(
        &mut self,
        tsb_delta: i32,
        bsb_delta: i32,
        adv_h_delta: i32,
        before: crate::interaction::GlyphSnapshot,
    ) -> bool {
        self.sync_paths_from_contours();
        self.text_stream.pending_spacing_deltas.push(SpacingDelta {
            slot_index: self.text_stream.active_slot_index,
            glyph_name: self.glyph.name.clone(),
            axis: MetricsAxis::Vertical,
            lsb_delta_upm: 0,
            rsb_delta_upm: 0,
            width_delta_upm: 0,
            tsb_delta_upm: tsb_delta,
            bsb_delta_upm: bsb_delta,
            advance_height_delta_upm: adv_h_delta,
        });
        self.record_mutation("Change Vertical Metrics", before);
        self.assert_invariants();
        true
    }

    /// Adjusts the active glyph's Top Sidebearing (`TSB`) by `delta_upm` while preserving `BSB`.
    pub fn adjust_active_tsb(&mut self, delta_upm: i32) -> bool {
        let before = self.snapshot();
        let dy = f64::from(delta_upm);
        self.shift_all_glyph_elements_y(-dy);
        self.update_active_slot_advance_height(dy);
        self.record_vertical_spacing_delta(delta_upm, 0, delta_upm, before)
    }

    /// Adjusts the active glyph's Bottom Sidebearing (`BSB`) by `delta_upm` while preserving `TSB`.
    pub fn adjust_active_bsb(&mut self, delta_upm: i32) -> bool {
        let before = self.snapshot();
        let dy = f64::from(delta_upm);
        self.update_active_slot_advance_height(dy);
        self.record_vertical_spacing_delta(0, delta_upm, delta_upm, before)
    }

    /// Shifts all contours vertically within a constant `advance_height`.
    pub fn shift_active_contours_in_height(&mut self, delta_upm: i32) -> bool {
        let before = self.snapshot();
        self.shift_all_glyph_elements_y(-f64::from(delta_upm));
        self.record_vertical_spacing_delta(delta_upm, -delta_upm, 0, before)
    }

    /// Adjusts vertical kerning at `lower_slot_idx` (`(lower_slot_idx - 1, lower_slot_idx)`) by `delta_upm`.
    pub fn adjust_vertical_kerning_at_slot(
        &mut self,
        lower_slot_idx: usize,
        delta_upm: i32,
    ) -> bool {
        if lower_slot_idx == 0 || lower_slot_idx >= self.text_stream.slots.len() {
            return false;
        }
        let before = self.snapshot();
        self.text_stream.slots[lower_slot_idx].kern_with_prev += delta_upm;
        let new_kern = self.text_stream.slots[lower_slot_idx].kern_with_prev;
        let top_glyph = self.text_stream.slots[lower_slot_idx - 1]
            .glyph
            .name
            .clone();
        let bottom_glyph = self.text_stream.slots[lower_slot_idx].glyph.name.clone();
        self.text_stream.recompute_origins();
        self.text_stream.pending_kerning_deltas.push(KerningDelta {
            axis: MetricsAxis::Vertical,
            left_slot_index: lower_slot_idx - 1,
            right_slot_index: lower_slot_idx,
            left_glyph: top_glyph,
            right_glyph: bottom_glyph,
            delta_upm,
            new_kern_upm: new_kern,
        });
        self.record_mutation("Change Vertical Metrics", before);
        self.assert_invariants();
        true
    }

    /// Dispatches vertical-stream (`WritingDirection::TopToBottom`) keyboard shortcuts (`ArrowUp`, `ArrowDown`, `[`, `]`, `{`, `}`).
    pub(crate) fn dispatch_vertical_text_stream_key(
        &mut self,
        key: Key,
        in_text_tool: bool,
        mods: crate::adapters::keyboard::KeyModifierFlags,
    ) -> bool {
        if self.text_stream.writing_direction != WritingDirection::TopToBottom {
            return false;
        }
        let crate::adapters::keyboard::KeyModifierFlags {
            shift,
            alt,
            cmd,
            ctrl,
        } = mods;
        let sp_low = (self.effective_spacing_step_upm(false).round() as i32).max(1);
        let sp_high = (self.effective_spacing_step_upm(true).round() as i32).max(1);
        let kn_low = (self.effective_kerning_step_upm(false).round() as i32).max(1);
        let kn_high = (self.effective_kerning_step_upm(true).round() as i32).max(1);

        if matches!(key, Key::OpenBracket | Key::CloseBracket) && !alt && !cmd && !ctrl {
            let sign = if key == Key::OpenBracket { -1 } else { 1 };
            return if shift {
                self.adjust_active_bsb(sp_high * sign)
            } else {
                self.adjust_active_tsb(sp_low * sign)
            };
        }

        if !matches!(key, Key::ArrowUp | Key::ArrowDown) {
            return false;
        }
        let sign = if key == Key::ArrowUp { -1 } else { 1 };
        if ctrl || cmd {
            let sp_step = if shift { sp_high } else { sp_low } * sign;
            let kn_step = if shift { kn_high } else { kn_low } * sign;
            return match (ctrl, cmd, alt) {
                (true, false, false) => self.adjust_active_tsb(sp_step),
                (false, true, false) => self.adjust_active_bsb(sp_step),
                (true, true, false) => self.shift_active_contours_in_height(sp_step),
                (true, false, true) => self
                    .adjust_vertical_kerning_at_slot(self.text_stream.active_slot_index, kn_step),
                (false, true, true) => self.adjust_vertical_kerning_at_slot(
                    self.text_stream.active_slot_index + 1,
                    kn_step,
                ),
                _ => false,
            };
        }
        if alt {
            let kn_step = if shift { kn_high } else { kn_low } * sign;
            return self
                .adjust_vertical_kerning_at_slot(self.text_stream.active_slot_index, kn_step);
        }
        if in_text_tool || (self.active_tool == CanvasTool::Text) {
            self.step_vertical_caret(sign, shift);
            return true;
        }
        false
    }

    fn step_vertical_caret(&mut self, sign: i32, shift: bool) {
        let n = self.text_stream.slots.len();
        let cur = self.text_stream.caret_index as isize;
        let next = (cur + sign as isize).clamp(0, n as isize) as usize;
        if shift {
            let anchor =
                self.text_stream
                    .selection_range
                    .map_or(self.text_stream.caret_index, |(s, e)| {
                        if self.text_stream.caret_index == e {
                            s
                        } else {
                            e
                        }
                    });
            let (s, e) = (anchor.min(next), anchor.max(next));
            if n > 0 {
                self.switch_active_slot(next.min(n - 1));
            }
            self.text_stream.caret_index = next;
            self.text_stream.selection_range = (s < e).then_some((s, e));
        } else {
            self.set_caret_index(next);
        }
    }

    pub fn set_show_hoi_diamonds(&mut self, show: bool) {
        self.show_hoi_diamonds = show;
        self.bump_content_generation();
    }

    pub fn set_hoi_layer_sets(&mut self, sets: Vec<HoiCubicLayerSet>) {
        self.drag_transients.hoi_layer_sets = sets.clone();
        self.glyph.hoi_layer_sets = sets;
        self.bump_content_generation();
        self.assert_invariants();
    }

    pub fn set_hoi_sibling_contours(&mut self, siblings: Option<HoiSiblingContours>) {
        self.drag_transients.hoi_sibling_contours = siblings.clone();
        self.glyph.hoi_sibling_contours = siblings;
        self.bump_content_generation();
        self.assert_invariants();
    }

    pub fn set_pixel_preview_ppem(&mut self, ppem: Option<u16>) {
        self.show_pixel_preview_ppem = ppem.map(|p| p.clamp(TT_MIN_PPEM, TT_MAX_PPEM));
        self.bump_content_generation();
        self.assert_invariants();
    }

    pub fn set_backdrop_ghost_contours(&mut self, contours: Vec<GlyphContour>) {
        self.backdrop_ghost_contours = contours.clone();
        self.glyph.backdrop_ghost_contours = contours;
        self.bump_content_generation();
        self.assert_invariants();
    }

    #[must_use]
    pub fn resolve_hinting_and_hoi_selection_info(&self) -> Option<SelectionInfo> {
        if let Some(hint_id) = self.selected_postscript_hint {
            if let Some(h) = self.postscript_hints.iter().find(|h| h.id == hint_id) {
                return Some(SelectionInfo::PostScriptHint {
                    hint_id: h.id,
                    horizontal: h.axis == PostScriptHintAxis::Horizontal,
                    origin_upm: h.origin_upm,
                    width_upm: h.width_upm,
                    is_ghost: h.is_ghost,
                    top_ghost: h.top_ghost,
                });
            }
        }
        if let Some(ins_id) = self.selected_truetype_instruction {
            if let Some(ins) = self.truetype_instructions.iter().find(|i| i.id == ins_id) {
                return Some(SelectionInfo::TrueTypeInstruction {
                    instruction_id: ins.id,
                    kind: ins.kind,
                    ppem: ins.ppem,
                    round_mode: ins.round_mode,
                    stem_index: ins.stem_index,
                    zone_index: ins.zone_index,
                });
            }
        }
        if self.active_tool == CanvasTool::Hoi {
            let set = self
                .drag_transients
                .hoi_layer_sets
                .first()
                .copied()
                .unwrap_or_default();
            return Some(SelectionInfo::HoiTiming {
                c1_x: set.c1.x.0,
                c1_y: set.c1.y.0,
                c2_x: set.c2.x.0,
                c2_y: set.c2.y.0,
            });
        }
        None
    }
}
