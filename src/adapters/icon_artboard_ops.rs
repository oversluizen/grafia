//! Dedicated adapter operations for `SPEC-017` Icon Studio Artboard (`§2.1`, `§2.4`, `§2.5`, `§2.6`, `§2.8`, `§2.8b`):
//! Apple/Material 4-shape Icon Keyline Grid, Optical Centroid Balance, `16px/24px` Pixel-Crispness Preview,
//! Sublayer `DrawDirection`, `knockout_margin_upm`, `phase_offset`, `MotionPath`, and W3C SVG I/O.

use crate::adapters::state::GrafiaCanvasState;
use crate::core::math_consts::{
    FIT_CURVE_MIN_CHORD_LEN_EPS, ICON_CRISPNESS_MAX_DIM_PX, ICON_KEYLINE_DEFAULT_SIZE_UPM,
    KNOCKOUT_MARGIN_MAX_UPM,
};
use crate::core::{
    compute_icon_optical_balance, export_w3c_icon_svg, import_w3c_icon_svg, ContourId,
    CustomReferenceFrame, DrawDirection, IconOpticalBalanceReport, KineticPlaybackMode,
    SymbolLayerTag,
};
use crate::tokens::AppearanceMode;

impl GrafiaCanvasState {
    fn ensure_symbol_layer_tag_at(&mut self, idx: usize) {
        while self.icon_studio.symbol_layer_tags.len() <= idx {
            let group = (self.icon_studio.symbol_layer_tags.len() as u8)
                .saturating_add(1)
                .max(1);
            self.icon_studio.symbol_layer_tags.push(SymbolLayerTag {
                layer_group: group,
                ..Default::default()
            });
        }
    }

    /// Sets the `DrawDirection` (`StartToEnd`, `EndToStart`, `CenterOut`) of the selected color sublayer with Undo/Redo (`SPEC-017 §2.4`).
    pub fn set_selected_sublayer_draw_direction(&mut self, dir: DrawDirection) -> bool {
        let Some(idx) = self.selected_color_sublayer_index() else {
            return false;
        };
        let cur_dir = self
            .icon_studio
            .symbol_layer_tags
            .get(idx)
            .map_or(DrawDirection::StartToEnd, |t| t.draw_direction);
        if cur_dir == dir && idx < self.icon_studio.symbol_layer_tags.len() {
            return false;
        }
        let before = self.snapshot();
        self.ensure_symbol_layer_tag_at(idx);
        self.icon_studio.symbol_layer_tags[idx].draw_direction = dir;
        self.icon_studio.symbol_layer_tags[idx].draw_order_reversed =
            dir == DrawDirection::EndToStart;
        self.record_mutation("Set Sublayer Draw Direction", before);
        self.assert_invariants();
        true
    }

    /// Sets the `knockout_margin_upm` (`0.0..=KNOCKOUT_MARGIN_MAX_UPM`) of the selected color sublayer with Undo/Redo (`SPEC-017 §2.5`).
    pub fn set_selected_sublayer_knockout_margin(&mut self, margin_upm: f64) -> bool {
        if !margin_upm.is_finite() {
            return false;
        }
        let Some(idx) = self.selected_color_sublayer_index() else {
            return false;
        };
        let clamped = margin_upm.clamp(0.0, KNOCKOUT_MARGIN_MAX_UPM);
        let cur = self
            .icon_studio
            .symbol_layer_tags
            .get(idx)
            .map_or(0.0, |t| t.knockout_margin_upm);
        if (cur - clamped).abs() <= FIT_CURVE_MIN_CHORD_LEN_EPS
            && idx < self.icon_studio.symbol_layer_tags.len()
        {
            return false;
        }
        let before = self.snapshot();
        self.ensure_symbol_layer_tag_at(idx);
        self.icon_studio.symbol_layer_tags[idx].knockout_margin_upm = clamped;
        self.record_mutation("Set Sublayer Knockout Margin", before);
        self.assert_invariants();
        true
    }

    /// Sets `icon_studio.kinetic.playback_mode` with Undo/Redo (`SPEC-017 §2.6`).
    pub fn set_kinetic_playback_mode(&mut self, mode: KineticPlaybackMode) {
        let clean = mode.sanitized();
        if self.icon_studio.kinetic.playback_mode == clean {
            return;
        }
        let before = self.snapshot();
        self.icon_studio.kinetic.playback_mode = clean;
        self.record_mutation("Set Kinetic Playback Mode", before);
        self.assert_invariants();
    }

    /// Sets `phase_offset` (`-1.0..=1.0`) on the selected color sublayer with Undo/Redo (`SPEC-017 §2.6`).
    pub fn set_selected_sublayer_phase_offset(&mut self, offset: f64) -> bool {
        if !offset.is_finite() {
            return false;
        }
        let Some(idx) = self.selected_color_sublayer_index() else {
            return false;
        };
        let clamped = offset.clamp(-1.0, 1.0);
        let cur = self
            .icon_studio
            .symbol_layer_tags
            .get(idx)
            .map_or(0.0, |t| t.phase_offset);
        if (cur - clamped).abs() <= FIT_CURVE_MIN_CHORD_LEN_EPS
            && idx < self.icon_studio.symbol_layer_tags.len()
        {
            return false;
        }
        let before = self.snapshot();
        self.ensure_symbol_layer_tag_at(idx);
        self.icon_studio.symbol_layer_tags[idx].phase_offset = clamped;
        self.record_mutation("Set Sublayer Phase Offset", before);
        self.assert_invariants();
        true
    }

    /// Attaches or clears `motion_path_contour_id` and `orient_along_motion_path` on the selected sublayer with Undo/Redo (`SPEC-017 §2.6`).
    pub fn set_selected_sublayer_motion_path(
        &mut self,
        motion_path_cid: Option<ContourId>,
        orient: bool,
    ) -> bool {
        let Some(idx) = self.selected_color_sublayer_index() else {
            return false;
        };
        let before = self.snapshot();
        self.ensure_symbol_layer_tag_at(idx);
        let tag = &mut self.icon_studio.symbol_layer_tags[idx];
        if tag.motion_path_contour_id == motion_path_cid && tag.orient_along_motion_path == orient {
            return false;
        }
        tag.motion_path_contour_id = motion_path_cid;
        tag.orient_along_motion_path = orient;
        self.record_mutation("Set Sublayer Motion Path", before);
        self.assert_invariants();
        true
    }

    /// Exports active color sublayers (or active glyph contours) and `SymbolLayerTag`s to SVG (`SPEC-016 §5.3`, `SPEC-017 §2.1`).
    #[must_use]
    pub fn export_kinetic_symbol_to_svg(&self) -> String {
        export_w3c_icon_svg(
            &self.color_sublayers,
            &self.icon_studio.symbol_layer_tags,
            &self.contour_strokes,
        )
    }

    /// Imports a layered kinetic W3C SVG into `color_sublayers`, `glyph.contours`, `contour_strokes`, and `symbol_layer_tags` with Undo/Redo (`SPEC-016 §5.3`, `SPEC-017 §2.1`).
    pub fn import_kinetic_symbol_from_svg(&mut self, svg_xml: &str) -> bool {
        let Ok(doc) = import_w3c_icon_svg(svg_xml) else {
            return false;
        };
        let mut flat_contours = Vec::new();
        let mut max_nid = self.next_node_id.max(1);
        for sub in &doc.sublayers {
            for c in &sub.contours {
                for n in &c.nodes {
                    max_nid = max_nid.max(n.id.0 + 1);
                }
                let mut copy = c.clone();
                copy.id = ContourId(flat_contours.len());
                flat_contours.push(copy);
            }
        }
        if flat_contours.is_empty() {
            return false;
        }
        let before = self.snapshot();
        self.next_node_id = self.next_node_id.max(max_nid);
        self.contours = flat_contours.iter().map(|c| c.nodes.clone()).collect();
        self.active_contour_closed = flat_contours.last().is_none_or(|c| c.closed);
        self.glyph.contours = flat_contours;
        self.selected_nodes.clear();
        self.sync_paths_from_contours();
        self.set_color_sublayers(doc.sublayers);
        self.icon_studio.symbol_layer_tags = doc.tags;
        self.contour_strokes = doc.contour_strokes;
        self.record_mutation("Import Kinetic Symbol SVG", before);
        self.assert_invariants();
        true
    }

    /// Enables or clears the canonical Apple/Material 4-shape icon keyline template on the canvas (`SPEC-017 §2.8`).
    pub fn set_icon_keyline_grid_enabled(&mut self, enabled: bool) {
        let size = if self.glyph.width.is_finite() && self.glyph.width > 0.0 {
            self.glyph.width
        } else {
            ICON_KEYLINE_DEFAULT_SIZE_UPM
        };
        let frame = enabled.then(|| CustomReferenceFrame::icon_keyline_template(size));
        self.set_custom_reference_frame(frame);
    }

    /// Computes the geometric BBox center vs. area-weighted optical centroid for the active icon (`SPEC-017 §2.8`).
    #[must_use]
    pub fn compute_active_icon_optical_balance(&self) -> Option<IconOpticalBalanceReport> {
        let contours = self.evaluate_active_filled_preview_contours();
        compute_icon_optical_balance(&contours)
    }

    /// Translates all contours and synchronized `color_sublayers` by $-\Delta\mathbf{c}$ so the optical centroid aligns with the geometric BBox center (`SPEC-017 §2.8`).
    pub fn align_icon_to_optical_center(&mut self) -> bool {
        let Some(rep) = self.compute_active_icon_optical_balance() else {
            return false;
        };
        let (dx, dy) = (-rep.delta_upm.x.0, -rep.delta_upm.y.0);
        if dx.hypot(dy) <= FIT_CURVE_MIN_CHORD_LEN_EPS {
            return false;
        }
        let mut new_c = self.glyph.contours.clone();
        for c in &mut new_c {
            for n in &mut c.nodes {
                n.point.x.0 += dx;
                n.point.y.0 += dy;
            }
        }
        let next_nid = self.next_node_id.max(1);
        self.apply_icon_contour_mutation(new_c, next_nid, "Align Icon to Optical Center")
    }

    /// Rasterizes a live `dim_px × dim_px` (`16×16` or `24×24`) pixel-crispness RGBA preview of the active icon (`SPEC-017 §2.8`).
    #[must_use]
    pub fn render_icon_crispness_preview_rgba(&self, dim_px: u32) -> Vec<u8> {
        let d = dim_px.clamp(1, ICON_CRISPNESS_MAX_DIM_PX);
        let mode = if self.theme.is_dark() {
            AppearanceMode::Dark
        } else {
            AppearanceMode::Light
        };
        crate::render::render_colored_thumbnail(self, d, d, mode)
    }
}
