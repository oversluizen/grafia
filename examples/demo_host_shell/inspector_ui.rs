//! Host Inspector Sidebar, Stekker Preset Bar, and Cached Bottom Preview Strip Panel for `MockFontHost`.

use super::{DemoPreset, MockFontHost, METRIC_STEP_UPM};
use egui::{Grid, Image, RichText, ScrollArea, Separator, Slider, Ui};
use grafia::adapters::GrafiaCanvasState;
use grafia::core::{
    CanvasHostRequest, SelectionInfo, SelectionInfoAction, TextStreamEditKind, TransformGridAnchor,
};
use grafia::tokens::{GrafiaTheme, PREVIEW_BLUR_MAX_RADIUS_PX, PREVIEW_BLUR_MIN_RADIUS_PX};

const PIN_GRID_ROWS: [[TransformGridAnchor; 3]; 3] = [
    [
        TransformGridAnchor::TopLeft,
        TransformGridAnchor::TopCenter,
        TransformGridAnchor::TopRight,
    ],
    [
        TransformGridAnchor::MiddleLeft,
        TransformGridAnchor::Center,
        TransformGridAnchor::MiddleRight,
    ],
    [
        TransformGridAnchor::BottomLeft,
        TransformGridAnchor::BottomCenter,
        TransformGridAnchor::BottomRight,
    ],
];

impl MockFontHost {
    pub fn poll_host_ports(&mut self, state: &mut GrafiaCanvasState) {
        for req in state.drain_host_requests() {
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
                        TextStreamEditKind::Backspace
                            if edit_req.selection_range.is_none() && caret > 0 =>
                        {
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
            }
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
    }

    pub fn render_preset_bar(
        &mut self,
        ui: &mut Ui,
        state: &mut GrafiaCanvasState,
        theme: &mut GrafiaTheme,
    ) {
        ui.horizontal_wrapped(|ui| {
            ui.strong("Stekker Presets:");
            for (preset, label) in DemoPreset::ALL {
                if ui.button(label).clicked() {
                    self.apply_preset(preset, state, theme);
                }
            }
        });
    }

    pub fn render_host_inspector(&mut self, ui: &mut Ui, state: &mut GrafiaCanvasState) {
        ui.strong("Host-Port Stekker Inspector");
        ui.add(Separator::default().horizontal());
        match state.selection_info() {
            SelectionInfo::GlyphBase(g) => {
                ui.label(format!(
                    "GlyphBase (/{}) | Contours: {} | Nodes: {}",
                    g.glyph_name, g.contour_count, g.total_node_count
                ));
                ui.label(format!(
                    "BBox: ({:.0},{:.0}) {:.0}x{:.0} | LSB:{:.0} RSB:{:.0} W:{:.0}",
                    g.bbox_x, g.bbox_y, g.bbox_w, g.bbox_h, g.lsb, g.rsb, g.advance_width
                ));
                ui.horizontal_wrapped(|ui| {
                    use SelectionInfoAction as Act;
                    let actions = [
                        ("LSB -10", Act::SetLsb(g.lsb - METRIC_STEP_UPM)),
                        ("LSB +10", Act::SetLsb(g.lsb + METRIC_STEP_UPM)),
                        ("RSB -10", Act::SetRsb(g.rsb - METRIC_STEP_UPM)),
                        ("RSB +10", Act::SetRsb(g.rsb + METRIC_STEP_UPM)),
                        (
                            "W +10",
                            Act::SetAdvanceWidth(g.advance_width + METRIC_STEP_UPM),
                        ),
                        ("Center in Width", Act::CenterInWidth),
                    ];
                    for (lbl, act) in actions {
                        if ui.button(lbl).clicked() {
                            let _ = state.apply_selection_info_action(act);
                        }
                    }
                });
            }
            SelectionInfo::SingleOnCurve(n) => {
                ui.label(format!(
                    "SingleOnCurve {:?} ({:.1}, {:.1}) [{:?}]",
                    n.node_id, n.x, n.y, n.node_type
                ));
                if ui.button("Toggle Smooth").clicked() {
                    let _ =
                        state.apply_selection_info_action(SelectionInfoAction::ToggleOnCurveSmooth);
                }
            }
            SelectionInfo::SingleOffCurve(h) => {
                ui.label(format!(
                    "SingleOffCurve {:?} d=({:.1},{:.1}) len={:.1}",
                    h.node_id, h.delta_x, h.delta_y, h.length
                ));
            }
            SelectionInfo::MultiNode(m) => {
                ui.label(format!(
                    "MultiNode ({}/{}) {:.0}x{:.0} @ ({:.0},{:.0})",
                    m.selected_node_count,
                    m.total_layer_node_count,
                    m.width,
                    m.height,
                    m.origin_x,
                    m.origin_y
                ));
            }
            SelectionInfo::Guide(gd) => {
                ui.label(format!(
                    "Guide {:?} ({:.0},{:.0}) {:.1}°",
                    gd.guide_ref.id, gd.x, gd.y, gd.angle_deg
                ));
            }
        }
        ui.horizontal(|ui| {
            Grid::new("host_pin_grid")
                .spacing([2.0, 2.0])
                .show(ui, |g_ui| {
                    for row in PIN_GRID_ROWS {
                        for anchor in row {
                            let is_active = state.transform_grid_anchor == anchor;
                            if g_ui.selectable_label(is_active, "•").clicked() {
                                let _ = state.apply_selection_info_action(
                                    SelectionInfoAction::SetMultiAnchor(anchor),
                                );
                            }
                        }
                        g_ui.end_row();
                    }
                });
            let mut prop = state.multi_selection_proportional;
            if ui.checkbox(&mut prop, "Prop. Scale").changed() {
                let _ = state
                    .apply_selection_info_action(SelectionInfoAction::SetMultiProportional(prop));
            }
            if ui.button("Cmd+U Anchors").clicked() {
                let _ = state.apply_selection_info_action(SelectionInfoAction::ResetDefaultAnchors);
                self.poll_host_ports(state);
            }
        });
        if let Some(a) = state.selected_anchor_info() {
            ui.label(
                RichText::new(format!(
                    "Selected Anchor: '{}' @ ({:.0}, {:.0})",
                    a.name, a.x, a.y
                ))
                .strong(),
            );
        }
        if let Some(c) = state.selected_component_info() {
            ui.horizontal_wrapped(|ui| {
                ui.label(RichText::new(format!("Component: '{}'", c.component_name)).strong());
                if ui.selectable_label(c.locked, "Lock").clicked() {
                    let _ = state.apply_selection_info_action(
                        SelectionInfoAction::SetComponentLocked(!c.locked),
                    );
                }
                if ui
                    .selectable_label(c.automatic_alignment, "Auto-Align")
                    .clicked()
                {
                    let _ = state.apply_selection_info_action(
                        SelectionInfoAction::SetComponentAutoAlignment(!c.automatic_alignment),
                    );
                }
                if ui.button("Decompose").clicked() {
                    let _ =
                        state.apply_selection_info_action(SelectionInfoAction::DecomposeComponents);
                }
            });
        }
        ui.add(Separator::default().horizontal());
        ui.strong("Live Host-Port Event Log");
        ScrollArea::vertical().max_height(110.0).show(ui, |ui| {
            for line in &self.event_log {
                ui.monospace(line);
            }
        });
    }

    pub fn render_preview_strip_panel(&mut self, ui: &mut Ui, state: &GrafiaCanvasState) {
        ui.horizontal(|ui| {
            ui.strong("v0.6 Preview Strip (PreviewStripCache)");
            ui.add(Separator::default().vertical());
            let _ = ui.add(
                Slider::new(
                    &mut self.preview_cfg.blur_radius_px,
                    PREVIEW_BLUR_MIN_RADIUS_PX..=PREVIEW_BLUR_MAX_RADIUS_PX,
                )
                .text("Blur (px)"),
            );
            ui.checkbox(&mut self.preview_cfg.flipped, "Flip Vertical");
            ui.checkbox(&mut self.preview_cfg.inverted, "Invert Polarity");
        });
        let (tex, _) = self
            .preview_cache
            .get_or_update_texture(ui.ctx(), state, &self.preview_cfg);
        let size = egui::Vec2::new(
            self.preview_cfg.width_px as f32,
            self.preview_cfg.height_px as f32,
        );
        ui.add(Image::from_texture((tex.id(), size)));
    }
}
