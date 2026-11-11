//! `SPEC-007` (`grafia v0.6.0`) Canvas Rulers (`Cmd+R`), Background Reference Layer (`Cmd+B`, `Cmd+J`,
//! `Cmd+K`/`Cmd+Shift+J`, `Cmd+Opt+B`), Default Glyph Anchors (`Cmd+U`), and keyboard/pointer handlers.

use crate::adapters::state::GrafiaCanvasState;
use crate::core::{CanvasAnchor, CanvasHostRequest, ContourId, GlyphContour, NodeId, ScreenPoint};
use crate::snapping::{Guide, GuideId, SelectedGuideRef};
use crate::tokens::{
    BACKGROUND_CONTOUR_ID_OFFSET, RULER_GUIDE_HORIZONTAL_ANGLE_DEG, RULER_GUIDE_VERTICAL_ANGLE_DEG,
    RULER_THICKNESS_DP,
};
use egui::{Event, Key, Rect};
use std::collections::HashSet;

/// Dispatches `SPEC-007` (`v0.6.0`) keyboard shortcuts (`Cmd+R`, `Cmd+B`, `Cmd+J`, `Cmd+K`/`Cmd+Shift+J`, `Cmd+U`).
pub fn dispatch_v060_keyboard_shortcut(
    state: &mut GrafiaCanvasState,
    event: &Event,
    is_shift: bool,
    is_alt: bool,
    is_cmd: bool,
    is_ctrl: bool,
) -> bool {
    let Event::Key {
        key,
        pressed: true,
        modifiers,
        ..
    } = event
    else {
        return false;
    };
    let shift = modifiers.shift || is_shift;
    let alt = modifiers.alt || is_alt;
    let cmd = modifiers.command || modifiers.mac_cmd || is_cmd || modifiers.ctrl || is_ctrl;
    if !cmd {
        return false;
    }
    match key {
        Key::R if !shift && !alt => {
            state.toggle_rulers();
            true
        }
        Key::B if !shift && !alt => {
            state.toggle_editing_background_layer();
            true
        }
        Key::B if !shift && alt => {
            state.toggle_show_background();
            true
        }
        Key::J if !shift => {
            state.copy_selection_to_background_with_mode(alt);
            true
        }
        Key::J if shift && !alt => {
            state.swap_foreground_and_background();
            true
        }
        Key::K if !shift && !alt => {
            state.swap_foreground_and_background();
            true
        }
        Key::U if !shift && !alt => {
            state.reset_default_anchors();
            true
        }
        _ => false,
    }
}

/// Handles pointer press on the 20 dp top/left ruler bars when `state.show_rulers` is true.
pub fn handle_v060_ruler_pointer_press(
    state: &mut GrafiaCanvasState,
    scr_pt: ScreenPoint,
    canvas_rect: Rect,
) -> bool {
    if !state.show_rulers {
        return false;
    }
    let in_top = scr_pt.y.0 >= canvas_rect.min.y
        && scr_pt.y.0 <= canvas_rect.min.y + RULER_THICKNESS_DP
        && scr_pt.x.0 >= canvas_rect.min.x + RULER_THICKNESS_DP;
    let in_left = scr_pt.x.0 >= canvas_rect.min.x
        && scr_pt.x.0 <= canvas_rect.min.x + RULER_THICKNESS_DP
        && scr_pt.y.0 >= canvas_rect.min.y + RULER_THICKNESS_DP;
    if in_top {
        state.ruler_drag_is_horizontal = Some(true);
        return true;
    }
    if in_left {
        state.ruler_drag_is_horizontal = Some(false);
        return true;
    }
    false
}

/// Completes a drag-from-ruler gesture on pointer release, spawning a local or global guide.
pub fn handle_v060_ruler_pointer_release(
    state: &mut GrafiaCanvasState,
    scr_pt: ScreenPoint,
    is_global: bool,
) -> bool {
    let Some(is_horiz) = state.ruler_drag_is_horizontal.take() else {
        return false;
    };
    if is_horiz {
        state.drag_guide_from_top_ruler(scr_pt, is_global);
    } else {
        state.drag_guide_from_left_ruler(scr_pt, is_global);
    }
    true
}

impl GrafiaCanvasState {
    pub(crate) fn init_archetype_corrected(arch: crate::tokens::GlyphArchetype) -> Self {
        let mut s = Self::init_glyph_a();
        s.load_glyph_archetype(arch);
        if s.contours.len() == 2
            && crate::core::contour_signed_area(&s.contours[0]).signum()
                == crate::core::contour_signed_area(&s.contours[1]).signum()
        {
            crate::core::reverse_contour_nodes(&mut s.contours[1], true);
            s.sync_paths_from_contours();
        }
        s.history = crate::interaction::CommandHistory::new();
        s
    }

    /// Toggles the 20 dp top and left canvas rulers (`Cmd+R`).
    pub fn toggle_rulers(&mut self) {
        let before = self.snapshot();
        self.show_rulers = !self.show_rulers;
        self.record_mutation("Toggle Rulers", before);
        self.assert_invariants();
    }

    /// Spawns a horizontal ($0^\circ$) local or global guide at `target_scr` dragged from the top ruler.
    pub fn drag_guide_from_top_ruler(
        &mut self,
        target_scr: ScreenPoint,
        is_global: bool,
    ) -> GuideId {
        let before = self.snapshot();
        let pt = self.active_slot_viewport().to_font(target_scr);
        let id = GuideId(self.next_guide_id);
        self.next_guide_id += 1;
        let guide = Guide::new(id, pt, RULER_GUIDE_HORIZONTAL_ANGLE_DEG, is_global);
        if is_global {
            self.global_guides.push(guide);
        } else {
            self.guides.push(guide);
        }
        self.selected_nodes.clear();
        self.selected_guide = Some(SelectedGuideRef { id, is_global });
        self.record_mutation("Add Guide from Top Ruler", before);
        self.assert_invariants();
        id
    }

    /// Spawns a vertical ($90^\circ$) local or global guide at `target_scr` dragged from the left ruler.
    pub fn drag_guide_from_left_ruler(
        &mut self,
        target_scr: ScreenPoint,
        is_global: bool,
    ) -> GuideId {
        let before = self.snapshot();
        let pt = self.active_slot_viewport().to_font(target_scr);
        let id = GuideId(self.next_guide_id);
        self.next_guide_id += 1;
        let guide = Guide::new(id, pt, RULER_GUIDE_VERTICAL_ANGLE_DEG, is_global);
        if is_global {
            self.global_guides.push(guide);
        } else {
            self.guides.push(guide);
        }
        self.selected_nodes.clear();
        self.selected_guide = Some(SelectedGuideRef { id, is_global });
        self.record_mutation("Add Guide from Left Ruler", before);
        self.assert_invariants();
        id
    }

    /// Copies selected contours (or all contours when none selected) into `background_contours` (`Cmd+J` / `Cmd+Opt+J`).
    pub fn copy_selection_to_background_with_mode(&mut self, append: bool) {
        let before = self.snapshot();
        let sel_set: HashSet<NodeId> = self.selected_nodes.iter().copied().collect();
        let source: Vec<GlyphContour> = if sel_set.is_empty() {
            self.glyph.contours.clone()
        } else {
            let filtered: Vec<GlyphContour> = self
                .glyph
                .contours
                .iter()
                .filter(|gc| gc.nodes.iter().any(|n| sel_set.contains(&n.id)))
                .cloned()
                .collect();
            if filtered.is_empty() {
                self.glyph.contours.clone()
            } else {
                filtered
            }
        };
        if !append {
            self.background_contours.clear();
        }
        let base_idx = self.background_contours.len();
        let mut next_nid = self.next_node_id.max(1);
        for (i, mut gc) in source.into_iter().enumerate() {
            gc.id = ContourId(BACKGROUND_CONTOUR_ID_OFFSET + base_idx + i);
            for n in &mut gc.nodes {
                n.id = NodeId(next_nid);
                next_nid += 1;
            }
            self.background_contours.push(gc);
        }
        self.next_node_id = next_nid;
        self.sync_paths_from_contours();
        self.record_mutation("Selection to Background", before);
        self.assert_invariants();
    }

    /// Copies selected or all contours to the background layer (`Cmd+J`).
    pub fn copy_selection_to_background(&mut self) {
        self.copy_selection_to_background_with_mode(false);
    }

    fn swap_fg_and_bg_contours_internal(&mut self) {
        let old_fg = self.glyph.contours.clone();
        let old_bg = std::mem::replace(&mut self.background_contours, old_fg);
        self.selected_nodes.clear();
        self.active_open_contour = None;
        self.pending_pen_bcp_out = None;
        self.pending_pen_first_bcp_in = None;
        self.hovered_pen_close_target = None;
        self.hovered_pen_close_node = None;
        self.hovered_segment_hit = None;
        let mut seen = HashSet::new();
        let mut next_id = self.next_node_id.max(1);
        let mut new_contours = Vec::with_capacity(old_bg.len());
        let mut new_g_contours = Vec::with_capacity(old_bg.len());
        for (idx, mut gc) in old_bg.into_iter().enumerate() {
            gc.id = ContourId(idx);
            for n in &mut gc.nodes {
                if n.id.0 == 0 || !seen.insert(n.id) {
                    n.id = NodeId(next_id);
                    seen.insert(n.id);
                    next_id += 1;
                } else if n.id.0 >= next_id {
                    next_id = n.id.0 + 1;
                }
            }
            new_contours.push(gc.nodes.clone());
            new_g_contours.push(gc);
        }
        self.next_node_id = next_id;
        self.contours = new_contours;
        self.active_contour_closed = new_g_contours.last().is_none_or(|gc| gc.closed);
        self.glyph.contours = new_g_contours;
        self.active_open_contour = (0..self.contours.len())
            .rev()
            .find(|&i| !self.is_contour_closed(i));
        self.sync_paths_from_contours();
    }

    /// Swaps the foreground and background contour layers (`Cmd+K` / `Cmd+Shift+J`).
    pub fn swap_foreground_and_background(&mut self) {
        let before = self.snapshot();
        self.swap_fg_and_bg_contours_internal();
        self.record_mutation("Swap with Background", before);
        self.assert_invariants();
    }

    /// Toggles editing of the background reference layer (`Cmd+B`).
    pub fn toggle_editing_background_layer(&mut self) {
        let before = self.snapshot();
        self.editing_background_layer = !self.editing_background_layer;
        self.editing_background = self.editing_background_layer;
        if self.editing_background_layer {
            self.show_background = true;
        }
        self.swap_fg_and_bg_contours_internal();
        self.record_mutation("Toggle Background Layer", before);
        self.assert_invariants();
    }

    /// Toggles visibility of the background reference layer (`Cmd+Opt+B`).
    pub fn toggle_show_background(&mut self) {
        let before = self.snapshot();
        if self.editing_background_layer {
            self.editing_background_layer = false;
            self.editing_background = false;
            self.swap_fg_and_bg_contours_internal();
        }
        self.show_background = !self.show_background;
        self.sync_paths_from_contours();
        self.record_mutation("Toggle Background Visibility", before);
        self.assert_invariants();
    }

    /// Emits `CanvasHostRequest::ResetDefaultAnchors` for the active glyph (`Cmd+U`).
    pub fn reset_default_anchors(&mut self) {
        self.host_requests
            .push(CanvasHostRequest::ResetDefaultAnchors {
                glyph_name: self.glyph.name.clone(),
            });
        self.assert_invariants();
    }

    /// Replaces the active glyph's `CanvasAnchor` list with full Undo/Redo support (`SPEC-007 §2.3.3` & `SPEC-008 §2.2`).
    pub fn set_anchors_with_undo(&mut self, anchors: Vec<CanvasAnchor>) {
        let before = self.snapshot();
        for a in &anchors {
            if a.id.0 >= self.next_anchor_id {
                self.next_anchor_id = a.id.0 + 1;
            }
        }
        self.anchors = anchors;
        self.glyph.anchors = self.anchors.clone();
        self.sync_paths_from_contours();
        self.record_mutation("Set Anchors", before);
        self.assert_invariants();
    }
}
