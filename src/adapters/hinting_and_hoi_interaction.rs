//! PostScript Hint Bands, TrueType Visual Instructor, Higher-Order Interpolation (`HOI`),
//! and Pass-1 Pixel Preview / Backdrop Ghost Contours State & Interaction Adapters (`SPEC-010 §2.4..§2.5`).

use crate::adapters::state::{CanvasTool, GrafiaCanvasState};
use crate::core::math_consts::{
    PS_HINT_HIT_TOLERANCE_DP, TT_BADGE_HIT_RADIUS_DP, TT_MAX_PPEM, TT_MIN_PPEM,
};
use crate::core::{
    find_contour_node_upm, truetype_upm_per_px, CanvasHostRequest, FontPoint, HoiCubicLayerSet,
    NodeId, PostScriptHint, PostScriptHintMutation, ScreenPoint, TrueTypeInstructionKind,
    TrueTypeInstructionMutation, TrueTypeRoundingGrid, TrueTypeVisualInstruction,
};

impl GrafiaCanvasState {
    pub fn set_postscript_hints(&mut self, mut hints: Vec<PostScriptHint>) {
        if let Some(sel_id) = self.selected_postscript_hint {
            if !hints.iter().any(|h| h.id == sel_id) {
                self.selected_postscript_hint = None;
            }
        }
        for h in &mut hints {
            h.selected = Some(h.id) == self.selected_postscript_hint;
        }
        self.postscript_hints = hints.clone();
        self.glyph.postscript_hints = hints;
        self.bump_content_generation();
        self.assert_invariants();
    }

    pub fn add_postscript_hint(&mut self, mut hint: PostScriptHint) {
        let before = self.snapshot();
        hint.selected = false;
        self.postscript_hints.push(hint.clone());
        self.glyph.postscript_hints = self.postscript_hints.clone();
        let req = CanvasHostRequest::MutatePostScriptHint(PostScriptHintMutation::Add(hint));
        self.drag_transients.host_requests.push(req);
        self.record_mutation("Add PostScript Hint", before);
        self.assert_invariants();
    }

    pub fn select_postscript_hint(&mut self, hint_id: Option<u64>) {
        let valid = hint_id.filter(|&id| self.postscript_hints.iter().any(|h| h.id == id));
        self.selected_postscript_hint = valid;
        for h in &mut self.postscript_hints {
            h.selected = Some(h.id) == valid;
        }
        self.glyph.postscript_hints = self.postscript_hints.clone();
        if valid.is_some() {
            self.selected_nodes.clear();
            self.selected_guide = None;
            self.drag_transients.selected_gradient_stop = None;
            self.selected_truetype_instruction = None;
        }
        self.assert_invariants();
    }

    pub fn try_click_select_postscript_hint(&mut self, scr_pt: ScreenPoint) -> bool {
        if self.active_tool != CanvasTool::Select || self.postscript_hints.is_empty() {
            return false;
        }
        let vp = self.active_slot_viewport();
        let font_pt = vp.to_font(scr_pt);
        let tol_upm = f64::from(PS_HINT_HIT_TOLERANCE_DP / vp.zoom.max(f32::EPSILON));
        if let Some(hit_id) = self
            .postscript_hints
            .iter()
            .rev()
            .find(|h| h.hit_test_upm(font_pt, tol_upm))
            .map(|h| h.id)
        {
            self.select_postscript_hint(Some(hit_id));
            return true;
        }
        false
    }

    pub fn delete_selected_postscript_hint(&mut self) -> bool {
        let Some(hint_id) = self.selected_postscript_hint else {
            return false;
        };
        if !self.postscript_hints.iter().any(|h| h.id == hint_id) {
            self.selected_postscript_hint = None;
            return false;
        }
        let before = self.snapshot();
        self.postscript_hints.retain(|h| h.id != hint_id);
        self.selected_postscript_hint = None;
        self.glyph.postscript_hints = self.postscript_hints.clone();
        let req =
            CanvasHostRequest::MutatePostScriptHint(PostScriptHintMutation::Delete { hint_id });
        self.drag_transients.host_requests.push(req);
        self.record_mutation("Delete PostScript Hint", before);
        self.assert_invariants();
        true
    }

    pub fn apply_postscript_hint_selection_action(
        &mut self,
        origin_upm: f64,
        width_upm: f64,
        top_ghost: bool,
    ) -> bool {
        let Some(hint_id) = self.selected_postscript_hint else {
            return false;
        };
        if !origin_upm.is_finite()
            || !width_upm.is_finite()
            || !self.postscript_hints.iter().any(|h| h.id == hint_id)
        {
            return false;
        }
        let before = self.snapshot();
        if let Some(h) = self.postscript_hints.iter_mut().find(|h| h.id == hint_id) {
            h.origin_upm = origin_upm;
            h.width_upm = width_upm;
            h.top_ghost = top_ghost;
        }
        self.glyph.postscript_hints = self.postscript_hints.clone();
        let m = PostScriptHintMutation::Update {
            hint_id,
            origin_upm,
            width_upm,
            top_ghost,
        };
        self.drag_transients
            .host_requests
            .push(CanvasHostRequest::MutatePostScriptHint(m));
        self.record_mutation("Change PostScript Hint", before);
        self.assert_invariants();
        true
    }

    pub fn set_truetype_instructor_ppem(&mut self, ppem: u16) {
        self.truetype_instructor_ppem = ppem.clamp(TT_MIN_PPEM, TT_MAX_PPEM);
        self.bump_content_generation();
        self.assert_invariants();
    }

    #[must_use]
    pub fn truetype_upm_per_px(&self) -> f64 {
        truetype_upm_per_px(self.glyph.upm, self.truetype_instructor_ppem)
    }

    #[must_use]
    pub const fn truetype_projection_axis(&self) -> (f64, f64) {
        if self
            .drag_transients
            .session_config
            .truetype_x_axis_projection
        {
            (1.0, 0.0)
        } else {
            (0.0, 1.0)
        }
    }

    pub fn toggle_truetype_projection_axis(&mut self) {
        self.drag_transients.truetype_x_axis_projection =
            !self.drag_transients.truetype_x_axis_projection;
        self.bump_content_generation();
    }

    pub fn set_truetype_instructions(&mut self, mut instructions: Vec<TrueTypeVisualInstruction>) {
        for ins in &mut instructions {
            ins.ppem = ins.ppem.max(1);
            ins.selected = Some(ins.id) == self.selected_truetype_instruction;
        }
        self.truetype_instructions = instructions.clone();
        self.glyph.truetype_instructions = instructions;
        self.bump_content_generation();
        self.assert_invariants();
    }

    pub fn add_truetype_instruction(&mut self, mut instruction: TrueTypeVisualInstruction) {
        let before = self.snapshot();
        instruction.ppem = instruction.ppem.max(1);
        instruction.selected = false;
        self.truetype_instructions.push(instruction.clone());
        self.glyph.truetype_instructions = self.truetype_instructions.clone();
        let req = CanvasHostRequest::MutateTrueTypeInstruction(TrueTypeInstructionMutation::Add(
            instruction,
        ));
        self.drag_transients.host_requests.push(req);
        self.record_mutation("Add TrueType Instruction", before);
        self.assert_invariants();
    }

    pub fn select_truetype_instruction(&mut self, instruction_id: Option<u64>) {
        let valid =
            instruction_id.filter(|&id| self.truetype_instructions.iter().any(|i| i.id == id));
        self.selected_truetype_instruction = valid;
        for ins in &mut self.truetype_instructions {
            ins.selected = Some(ins.id) == valid;
        }
        self.glyph.truetype_instructions = self.truetype_instructions.clone();
        if valid.is_some() {
            self.selected_nodes.clear();
            self.selected_guide = None;
            self.drag_transients.selected_gradient_stop = None;
            self.selected_postscript_hint = None;
        }
        self.assert_invariants();
    }

    pub fn try_click_select_truetype_instruction(&mut self, scr_pt: ScreenPoint) -> bool {
        if self.truetype_instructions.is_empty() {
            return false;
        }
        let vp = self.active_slot_viewport();
        let hit_r_sq = f64::from(TT_BADGE_HIT_RADIUS_DP) * f64::from(TT_BADGE_HIT_RADIUS_DP);
        let mut best: Option<(u64, f64)> = None;
        for ins in &self.truetype_instructions {
            let Some(target_upm) = find_contour_node_upm(&self.contours, ins.target_node) else {
                continue;
            };
            let badge_scr = vp.to_screen(target_upm);
            let dx = f64::from(scr_pt.x.0 - badge_scr.x.0);
            let dy = f64::from(scr_pt.y.0 - badge_scr.y.0);
            let d_sq = dx * dx + dy * dy;
            if d_sq <= hit_r_sq && best.is_none_or(|(_, b_sq)| d_sq < b_sq) {
                best = Some((ins.id, d_sq));
            }
        }
        if let Some((id, _)) = best {
            self.select_truetype_instruction(Some(id));
            return true;
        }
        false
    }

    pub fn handle_truetype_instructor_key(&mut self, key: egui::Key) -> bool {
        if self.active_tool != CanvasTool::TrueTypeInstructor {
            return false;
        }
        if key == egui::Key::X {
            self.toggle_truetype_projection_axis();
            return true;
        }
        let kind = match key {
            egui::Key::A => TrueTypeInstructionKind::Touch,
            egui::Key::S => TrueTypeInstructionKind::Stem,
            egui::Key::F => TrueTypeInstructionKind::Shift,
            egui::Key::G => TrueTypeInstructionKind::Interpolate,
            egui::Key::E => TrueTypeInstructionKind::Delta,
            _ => return false,
        };
        let Some(&first_node) = self.selected_nodes.first() else {
            return false;
        };
        let (source_node, target_node) = if self.selected_nodes.len() >= 2 {
            (Some(first_node), self.selected_nodes[1])
        } else {
            (None, first_node)
        };
        let next_id = self
            .truetype_instructions
            .iter()
            .map(|i| i.id)
            .max()
            .unwrap_or(0)
            + 1;
        let delta_pixels = if kind == TrueTypeInstructionKind::Delta {
            1.0
        } else {
            0.0
        };
        self.add_truetype_instruction(TrueTypeVisualInstruction {
            id: next_id,
            kind,
            source_node,
            target_node,
            ppem: self.truetype_instructor_ppem,
            round_mode: TrueTypeRoundingGrid::ToGrid,
            stem_index: None,
            zone_index: None,
            delta_pixels,
            x_axis: self.drag_transients.truetype_x_axis_projection,
            selected: false,
        });
        true
    }

    pub fn apply_truetype_instruction_selection_action(
        &mut self,
        kind: TrueTypeInstructionKind,
        ppem: u16,
        round_mode: TrueTypeRoundingGrid,
        stem_index: Option<u8>,
        zone_index: Option<u8>,
    ) -> bool {
        let Some(ins_id) = self.selected_truetype_instruction else {
            return false;
        };
        if ppem == 0 || !self.truetype_instructions.iter().any(|i| i.id == ins_id) {
            return false;
        }
        let before = self.snapshot();
        if let Some(ins) = self
            .truetype_instructions
            .iter_mut()
            .find(|i| i.id == ins_id)
        {
            ins.kind = kind;
            ins.ppem = ppem;
            ins.round_mode = round_mode;
            ins.stem_index = stem_index;
            ins.zone_index = zone_index;
            let req = CanvasHostRequest::MutateTrueTypeInstruction(
                TrueTypeInstructionMutation::Update(ins.clone()),
            );
            self.drag_transients.host_requests.push(req);
        }
        self.glyph.truetype_instructions = self.truetype_instructions.clone();
        self.record_mutation("Change TrueType Instruction", before);
        self.assert_invariants();
        true
    }

    fn set_primary_hoi_cubic_set(&mut self, c1: FontPoint, c2: FontPoint) {
        let next_set = HoiCubicLayerSet { c1, c2 };
        if let Some(first) = self.drag_transients.hoi_layer_sets.first_mut() {
            *first = next_set;
        } else {
            self.drag_transients.hoi_layer_sets.push(next_set);
        }
        self.glyph.hoi_layer_sets = self.drag_transients.hoi_layer_sets.clone();
    }

    pub fn apply_hoi_timing_selection_action(
        &mut self,
        c1_x: f64,
        c1_y: f64,
        c2_x: f64,
        c2_y: f64,
    ) -> bool {
        if !c1_x.is_finite() || !c1_y.is_finite() || !c2_x.is_finite() || !c2_y.is_finite() {
            return false;
        }
        let before = self.snapshot();
        self.set_primary_hoi_cubic_set(FontPoint::new(c1_x, c1_y), FontPoint::new(c2_x, c2_y));
        self.record_mutation("Change HOI Timing", before);
        self.assert_invariants();
        true
    }

    pub fn apply_mutate_hoi_node_selection_action(
        &mut self,
        layer_index: usize,
        contour_index: usize,
        node_id: NodeId,
        new_pos: FontPoint,
        c1: FontPoint,
        c2: FontPoint,
    ) -> bool {
        if ![new_pos.x.0, new_pos.y.0, c1.x.0, c1.y.0, c2.x.0, c2.y.0]
            .iter()
            .all(|v| v.is_finite())
        {
            return false;
        }
        let before = self.snapshot();
        if let Some(node) = self
            .companion_master_layers
            .get_mut(layer_index)
            .and_then(|l| l.contours.get_mut(contour_index))
            .and_then(|c| c.iter_mut().find(|n| n.id == node_id))
        {
            node.point = new_pos;
        }
        if let Some(node) = self
            .drag_transients
            .hoi_sibling_contours
            .as_mut()
            .and_then(|s| s.contours_per_layer.get_mut(layer_index))
            .and_then(|l| l.get_mut(contour_index))
            .and_then(|c| c.nodes.iter_mut().find(|n| n.id == node_id))
        {
            node.point = new_pos;
        }
        self.set_primary_hoi_cubic_set(c1, c2);
        self.glyph.hoi_sibling_contours = self.drag_transients.hoi_sibling_contours.clone();
        let req = CanvasHostRequest::MutateHoiLayerNode {
            layer_index,
            contour_index,
            node_id,
            new_pos,
        };
        self.drag_transients.host_requests.push(req);
        self.record_mutation("Change HOI Layer Node", before);
        self.assert_invariants();
        true
    }
}
