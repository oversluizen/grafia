//! Host Inspector Sidebar, Stekker Preset Bar, and Cached Bottom Preview Strip Panel for `MockFontHost`.

use super::{DemoPreset, MockFontHost, METRIC_STEP_UPM};
use egui::{Grid, Image, RichText, ScrollArea, Separator, Slider, Ui};
use grafia::adapters::GrafiaCanvasState;
use grafia::core::{SelectionInfo, SelectionInfoAction, TransformGridAnchor};
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
        Self::render_selection_info_summary(ui, state);
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

    fn render_selection_info_summary(ui: &mut Ui, state: &mut GrafiaCanvasState) {
        match state.selection_info() {
            SelectionInfo::GlyphBase(g) => {
                Self::render_glyph_base_selection_info(ui, state, &g);
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
            SelectionInfo::HobbyNode(h) => {
                ui.label(format!(
                    "Hobby {:?} ({:.1},{:.1}) t=({:.2},{:.2})",
                    h.node_id, h.x, h.y, h.tension_in, h.tension_out
                ));
            }
            SelectionInfo::BackgroundImage(bg) => {
                ui.label(format!(
                    "BgImage ({:.0},{:.0}) {:.0}%x{:.0}%",
                    bg.x, bg.y, bg.scale_x_pct, bg.scale_y_pct
                ));
            }
            other => {
                ui.label(format!("{other:?}"));
            }
        }
    }

    fn render_glyph_base_selection_info(
        ui: &mut Ui,
        state: &mut GrafiaCanvasState,
        g: &grafia::core::GlyphBaseInfo,
    ) {
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
