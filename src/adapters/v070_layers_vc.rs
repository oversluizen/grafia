//! `SPEC-008` (`grafia v0.7.0`) `CompanionMasterLayer`, Master Compatibility (`Ctrl+Opt+Cmd+N`),
//! `SelectAllLayers`, VC-stekkers (`AnchorSelectionInfo`, `ComponentSelectionInfo`), and keyboard dispatcher.

use crate::adapters::state::GrafiaCanvasState;
use crate::core::{
    compute_master_compatibility_report, select_nodes_in_rect_across_layers, Affine2D,
    AnchorSelectionInfo, CompanionMasterLayer, ComponentSelectionInfo, FontPoint,
    MasterCompatibilityReport, NodeId, SelectionInfoAction,
};
use egui::{Event, Key};
use std::collections::HashSet;

/// Dispatches `SPEC-008` (`v0.7.0`) keyboard shortcuts (`Ctrl+Opt+1..8`, `Cmd+Shift+D`, `Ctrl+Opt+Cmd+N`,
/// plus Anchor `Tab`/`Shift+Tab` and `Delete`/`Backspace`).
pub fn dispatch_v070_keyboard_shortcut(
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
    let cmd = modifiers.command || modifiers.mac_cmd || is_cmd;
    let ctrl = modifiers.ctrl || is_ctrl;

    if ctrl && alt && cmd && !shift && *key == Key::N {
        state.toggle_show_master_compatibility();
        return true;
    }

    if ctrl && alt && !cmd && !shift {
        let col = match key {
            Key::Num1 => Some(0),
            Key::Num2 => Some(1),
            Key::Num3 => Some(2),
            Key::Num4 => Some(3),
            Key::Num5 => Some(4),
            Key::Num6 => Some(5),
            Key::Num7 => Some(6),
            Key::Num8 => Some(7),
            _ => None,
        };
        if let Some(c) = col {
            state.apply_fit_curve_column(c);
            return true;
        }
    }

    if (cmd || ctrl) && shift && !alt && *key == Key::D && state.decompose_selected_components() {
        return true;
    }

    if !cmd
        && !ctrl
        && !alt
        && state.selected_nodes.is_empty()
        && state.anchors.iter().any(|a| a.selected)
    {
        if *key == Key::Tab {
            return state.cycle_selected_anchor(shift);
        }
        if matches!(key, Key::Delete | Key::Backspace) {
            return state.delete_selected_anchors();
        }
    }

    false
}

impl GrafiaCanvasState {
    /// Sets the `CompanionMasterLayer` list for multi-layer viewing/editing and Master Compatibility (`SPEC-008 §2.6`).
    pub fn set_companion_master_layers(&mut self, layers: Vec<CompanionMasterLayer>) {
        self.companion_master_layers = layers;
        self.selected_companion_nodes.clear();
        self.bump_content_generation();
    }

    /// Toggles Master Compatibility visualization (`Ctrl+Opt+Cmd+N`, `SPEC-008 §2.6.2`).
    pub fn toggle_show_master_compatibility(&mut self) {
        self.show_master_compatibility = !self.show_master_compatibility;
    }

    /// Computes the `MasterCompatibilityReport` between the active layer and all `CompanionMasterLayer`s (`SPEC-008 §2.6.2`).
    #[must_use]
    pub fn compute_master_compatibility(&self) -> MasterCompatibilityReport {
        compute_master_compatibility_report(
            &self.glyph.contours,
            &self.anchors,
            &self.companion_master_layers,
        )
    }

    /// Selects nodes within the axis-aligned UPM rectangle `[min, max]` across the active layer and (when `select_all_layers_mode` is true) editable `CompanionMasterLayer`s.
    pub fn select_nodes_in_rect_all_layers(&mut self, min: FontPoint, max: FontPoint) {
        self.pre_all_layers_select_snapshot = Some(self.snapshot());
        let (act, comp) = select_nodes_in_rect_across_layers(
            &self.glyph.contours,
            &self.companion_master_layers,
            self.select_all_layers_mode,
            min,
            max,
        );
        self.selected_nodes = act;
        self.selected_companion_nodes = comp;
    }

    /// Translates selected nodes across both the active layer and editable `CompanionMasterLayer`s (`SPEC-008 §2.6.3`).
    pub fn translate_selected_across_all_layers(&mut self, delta: FontPoint) {
        let before = self
            .pre_all_layers_select_snapshot
            .take()
            .unwrap_or_else(|| self.snapshot());
        let sel_set: HashSet<NodeId> = self.selected_nodes.iter().copied().collect();
        for contour in &mut self.contours {
            for node in contour {
                if sel_set.contains(&node.id) {
                    node.point.x.0 += delta.x.0;
                    node.point.y.0 += delta.y.0;
                }
            }
        }
        if self.select_all_layers_mode {
            for layer in self
                .companion_master_layers
                .iter_mut()
                .filter(|l| l.editable)
            {
                for contour in &mut layer.contours {
                    for node in contour {
                        if self.selected_companion_nodes.contains(&(layer.id, node.id)) {
                            node.point.x.0 += delta.x.0;
                            node.point.y.0 += delta.y.0;
                        }
                    }
                }
            }
        }
        self.sync_paths_from_contours();
        self.record_mutation("Translate Across All Layers", before);
        self.assert_invariants();
    }

    /// Returns VC-stekker telemetry for the currently selected `CanvasAnchor`, if any.
    #[must_use]
    pub fn selected_anchor_info(&self) -> Option<AnchorSelectionInfo> {
        self.anchors
            .iter()
            .find(|a| a.selected)
            .map(|a| AnchorSelectionInfo {
                id: a.id,
                name: a.name.clone(),
                x: a.position.x.0,
                y: a.position.y.0,
            })
    }

    /// Returns VC-stekker telemetry for the currently selected `CanvasComponent`, if any.
    #[must_use]
    pub fn selected_component_info(&self) -> Option<ComponentSelectionInfo> {
        self.components
            .iter()
            .find(|c| c.selected)
            .map(|c| ComponentSelectionInfo {
                id: c.id,
                component_name: c.component_name.clone(),
                transform: c.transform,
                locked: c.locked,
                automatic_alignment: c.automatic_alignment,
            })
    }

    /// Sets the position of all selected `CanvasAnchor`s (VC-stekker mutator).
    pub fn set_selected_anchor_position(&mut self, pos: FontPoint) {
        if !pos.x.0.is_finite() || !pos.y.0.is_finite() {
            return;
        }
        let before = self.snapshot();
        for a in self.anchors.iter_mut().filter(|a| a.selected) {
            a.position = pos;
        }
        self.glyph.anchors = self.anchors.clone();
        self.record_mutation("Set Anchor Position", before);
    }

    /// Renames the primary selected `CanvasAnchor` (VC-stekker mutator).
    pub fn set_selected_anchor_name(&mut self, name: String) {
        let trimmed = name.trim().to_string();
        if trimmed.is_empty() {
            return;
        }
        let before = self.snapshot();
        if let Some(a) = self.anchors.iter_mut().find(|a| a.selected) {
            a.name = trimmed;
        }
        self.glyph.anchors = self.anchors.clone();
        self.record_mutation("Rename Anchor", before);
    }

    /// Sets the `Affine2D` transform on the primary selected unlocked `CanvasComponent` (VC-stekker mutator).
    pub fn set_selected_component_transform(&mut self, transform: Affine2D) {
        let before = self.snapshot();
        for c in self
            .components
            .iter_mut()
            .filter(|c| c.selected && !c.locked)
        {
            c.transform = transform;
        }
        self.record_mutation("Set Component Transform", before);
    }

    /// Sets the `locked` flag on all selected `CanvasComponent`s (VC-stekker mutator).
    pub fn set_selected_component_locked(&mut self, locked: bool) {
        let before = self.snapshot();
        for c in self.components.iter_mut().filter(|c| c.selected) {
            c.locked = locked;
        }
        self.record_mutation("Set Component Locked", before);
    }

    /// Sets the `automatic_alignment` flag on all selected `CanvasComponent`s (VC-stekker mutator).
    pub fn set_selected_component_auto_alignment(&mut self, auto_align: bool) {
        let before = self.snapshot();
        for c in self.components.iter_mut().filter(|c| c.selected) {
            c.automatic_alignment = auto_align;
        }
        self.record_mutation("Set Component Auto Alignment", before);
    }

    /// Clears `selected` on all `CanvasAnchor`s, `CanvasComponent`s, and `selected_companion_nodes`.
    pub fn clear_v070_selections(&mut self) {
        for a in &mut self.anchors {
            a.selected = false;
        }
        self.glyph.anchors = self.anchors.clone();
        for c in &mut self.components {
            c.selected = false;
        }
        self.selected_companion_nodes.clear();
    }

    pub(crate) fn try_select_v070_anchor_or_component_on_press(
        &mut self,
        font_pt: FontPoint,
        is_shift: bool,
    ) -> bool {
        if self.select_anchor_at_upm(font_pt, is_shift) {
            if !is_shift {
                for c in &mut self.components {
                    c.selected = false;
                }
            }
            self.select_state = crate::interaction::SelectToolState::Idle;
            self.active_drag_before_snapshot = None;
            return true;
        }
        if self.select_component_at_upm(font_pt, is_shift) {
            if !is_shift {
                for a in &mut self.anchors {
                    a.selected = false;
                }
                self.glyph.anchors = self.anchors.clone();
            }
            self.select_state = crate::interaction::SelectToolState::Idle;
            self.active_drag_before_snapshot = None;
            return true;
        }
        false
    }

    pub(crate) fn sync_companion_marquee_selection(&mut self, rect: kurbo::Rect) {
        if self.select_all_layers_mode {
            let f_min = self.viewport.to_font(crate::core::ScreenPoint::new(
                rect.x0 as f32,
                rect.y1 as f32,
            ));
            let f_max = self.viewport.to_font(crate::core::ScreenPoint::new(
                rect.x1 as f32,
                rect.y0 as f32,
            ));
            let (_, comp) = select_nodes_in_rect_across_layers(
                &self.glyph.contours,
                &self.companion_master_layers,
                true,
                f_min,
                f_max,
            );
            self.selected_companion_nodes = comp;
        }
    }

    /// Nudges selected `CanvasAnchor`s or unlocked non-auto-aligned `CanvasComponent`s by `(dx, dy)` UPM.
    pub fn nudge_selected_anchors_or_components(&mut self, dx: f64, dy: f64) -> bool {
        if self.anchors.iter().any(|a| a.selected) {
            let before = self.snapshot();
            for a in self.anchors.iter_mut().filter(|a| a.selected) {
                a.position.x.0 += dx;
                a.position.y.0 += dy;
            }
            self.glyph.anchors = self.anchors.clone();
            self.record_mutation("Nudge Anchor", before);
            return true;
        }
        self.translate_selected_components(FontPoint::new(dx, dy))
    }

    pub(crate) fn apply_v070_selection_info_action(&mut self, action: SelectionInfoAction) -> bool {
        match action {
            SelectionInfoAction::SetAnchorPosition(pos) => {
                if !pos.x.0.is_finite()
                    || !pos.y.0.is_finite()
                    || !self.anchors.iter().any(|a| a.selected)
                {
                    return false;
                }
                self.set_selected_anchor_position(pos);
                true
            }
            SelectionInfoAction::SetAnchorName(name) => {
                if name.trim().is_empty() || !self.anchors.iter().any(|a| a.selected) {
                    return false;
                }
                self.set_selected_anchor_name(name);
                true
            }
            SelectionInfoAction::SetComponentTransform(tf) => {
                if !self.components.iter().any(|c| c.selected && !c.locked) {
                    return false;
                }
                self.set_selected_component_transform(tf);
                true
            }
            SelectionInfoAction::SetComponentLocked(locked) => {
                if !self.components.iter().any(|c| c.selected) {
                    return false;
                }
                self.set_selected_component_locked(locked);
                true
            }
            SelectionInfoAction::SetComponentAutoAlignment(auto_align) => {
                if !self.components.iter().any(|c| c.selected) {
                    return false;
                }
                self.set_selected_component_auto_alignment(auto_align);
                true
            }
            SelectionInfoAction::DecomposeComponents => self.decompose_selected_components(),
            SelectionInfoAction::ResetDefaultAnchors => {
                self.reset_default_anchors();
                true
            }
            _ => false,
        }
    }
}
