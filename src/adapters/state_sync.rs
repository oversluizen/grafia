//! Archetype loading, viewport fitting, contour-to-path synchronization, and node lookup for `GrafiaCanvasState`.

use crate::adapters::state::GrafiaCanvasState;
use crate::core::{Node, NodeId, NodeType, ScreenPoint};
use crate::interaction::{PenToolState, SelectToolState};
use crate::tokens::{
    load_archetype_preset, GlyphAGoldenMaster, GlyphArchetype, GrafiaTheme, AUTOFIT_MARGIN_RATIO,
    DEFAULT_CANVAS_HEIGHT_DP, DEFAULT_CANVAS_WIDTH_DP,
};
use kurbo::BezPath;

impl GrafiaCanvasState {
    #[must_use]
    pub fn init_glyph_a() -> Self {
        let master = GlyphAGoldenMaster::load();
        let mut state = Self::new(GrafiaTheme::sonoma_default());
        state.viewport.upm = master.upm;
        let zoom = ((DEFAULT_CANVAS_HEIGHT_DP * 0.5) / master.cap_height as f32) * 0.5;
        let pan_x = (DEFAULT_CANVAS_WIDTH_DP - (master.width as f32 * zoom)) * 0.5;
        let pan_y = DEFAULT_CANVAS_HEIGHT_DP * 0.5 + (master.cap_height as f32 * zoom * 0.5);
        state.viewport.zoom = zoom;
        state.viewport.pan_offset = ScreenPoint::new(pan_x, pan_y);

        let outer: Vec<Node> = master
            .outer_contour
            .iter()
            .map(|pt| {
                let n = Node::new(NodeId(state.next_node_id), pt.x, pt.y, NodeType::Corner);
                state.next_node_id += 1;
                n
            })
            .collect();
        let inner: Vec<Node> = master
            .inner_counter
            .iter()
            .map(|pt| {
                let n = Node::new(NodeId(state.next_node_id), pt.x, pt.y, NodeType::Corner);
                state.next_node_id += 1;
                n
            })
            .collect();

        state.contours = vec![outer.clone(), inner.clone()];
        state.glyph = crate::core::Glyph {
            name: master.glyphname,
            upm: master.upm,
            width: master.width,
            height: master.cap_height,
            contours: vec![
                crate::core::GlyphContour::new(crate::core::ContourId(0), outer),
                crate::core::GlyphContour::new(crate::core::ContourId(1), inner),
            ],
            annotations: Vec::new(),
            background_contours: Vec::new(),
            anchors: Vec::new(),
            postscript_hints: Vec::new(),
            truetype_instructions: Vec::new(),
            hoi_layer_sets: Vec::new(),
            hoi_sibling_contours: None,
            backdrop_ghost_contours: Vec::new(),
        };
        state.active_contour_closed = true;
        state.sync_paths_from_contours();
        state.assert_invariants();
        state
    }

    #[must_use]
    pub fn init_glyph_o() -> Self {
        Self::init_archetype_corrected(GlyphArchetype::O)
    }
    #[must_use]
    pub fn init_glyph_n() -> Self {
        Self::init_archetype_corrected(GlyphArchetype::N)
    }
    #[must_use]
    pub fn init_glyph_s() -> Self {
        Self::init_archetype_corrected(GlyphArchetype::S)
    }

    pub fn load_glyph_archetype(&mut self, archetype: GlyphArchetype) {
        self.text_stream = crate::core::CanvasTextStream::new();
        self.active_archetype = archetype;
        self.selected_nodes.clear();
        self.hovered_segment_hit = None;
        self.select_state = SelectToolState::Idle;
        self.pen_state = PenToolState::Idle;
        self.active_open_contour = None;
        self.pending_pen_bcp_out = None;
        self.pending_pen_first_bcp_in = None;
        self.hovered_pen_close_target = None;
        if archetype == GlyphArchetype::A {
            let (theme, mode, sys_dark) = (
                self.theme.clone(),
                self.appearance_mode,
                self.system_dark_mode,
            );
            *self = Self::init_glyph_a();
            (self.theme, self.appearance_mode, self.system_dark_mode) = (theme, mode, sys_dark);
            return;
        }
        let preset = load_archetype_preset(archetype);
        self.viewport.upm = preset.upm as u16;
        self.contours.clear();
        let mut g_contours = Vec::new();
        for (c_idx, cp) in preset.contours.iter().enumerate() {
            let nodes: Vec<Node> = cp
                .nodes
                .iter()
                .map(|np| {
                    let ntype = match np.node_type.as_str() {
                        "smooth" => NodeType::Smooth,
                        "off_curve" => NodeType::OffCurve,
                        _ => NodeType::Corner,
                    };
                    let n = Node::new(NodeId(self.next_node_id), np.x, np.y, ntype);
                    self.next_node_id += 1;
                    n
                })
                .collect();
            self.contours.push(nodes.clone());
            let mut gc = crate::core::GlyphContour::new(crate::core::ContourId(c_idx), nodes);
            gc.closed = cp.closed;
            g_contours.push(gc);
        }
        self.glyph = crate::core::Glyph {
            name: preset.glyph_name,
            upm: preset.upm as u16,
            width: preset.advance_width,
            height: preset.cap_height,
            contours: g_contours,
            annotations: Vec::new(),
            background_contours: Vec::new(),
            anchors: Vec::new(),
            postscript_hints: Vec::new(),
            truetype_instructions: Vec::new(),
            hoi_layer_sets: Vec::new(),
            hoi_sibling_contours: None,
            backdrop_ghost_contours: Vec::new(),
        };
        self.background_contours.clear();
        self.anchors.clear();
        self.editing_background_layer = false;
        self.active_contour_closed = true;
        self.sync_paths_from_contours();
        self.assert_invariants();
    }

    pub fn fit_viewport_to_glyph(&mut self, canvas_w: f32, canvas_h: f32) {
        if self.text_stream.slots.len() > 1 {
            let (mut x_min, mut x_max) = (0.0_f64, self.glyph.width.max(1.0));
            let (mut y_min, mut y_max) = (0.0_f64, self.glyph.height.max(1.0));
            for s in &self.text_stream.slots {
                x_min = x_min.min(s.origin_x_upm);
                x_max = x_max.max(s.origin_x_upm + s.glyph.width.max(1.0));
                y_min = y_min.min(s.origin_y_upm);
                y_max = y_max.max(s.origin_y_upm + s.glyph.height.max(1.0));
            }
            let span_w = ((x_max - x_min) as f32).max(1.0);
            let span_h = ((y_max - y_min) as f32).max(1.0);
            let zoom_h = (canvas_h * AUTOFIT_MARGIN_RATIO) / span_h;
            let zoom_w = (canvas_w * AUTOFIT_MARGIN_RATIO) / span_w;
            let zoom = zoom_h.min(zoom_w).max(crate::tokens::MIN_VIEWPORT_ZOOM);
            let pan_x = (canvas_w - span_w * zoom) * 0.5 - (x_min as f32) * zoom;
            let pan_y = canvas_h * 0.5 + (span_h * zoom * 0.5) + (y_min as f32) * zoom;
            self.viewport.zoom = zoom;
            self.viewport.pan_offset = ScreenPoint::new(pan_x, pan_y);
            return;
        }
        let glyph_h = (self.glyph.height as f32).max(1.0);
        let glyph_w = (self.glyph.width as f32).max(1.0);
        let zoom_h = (canvas_h * AUTOFIT_MARGIN_RATIO) / glyph_h;
        let zoom_w = (canvas_w * AUTOFIT_MARGIN_RATIO) / glyph_w;
        let zoom = zoom_h.min(zoom_w).max(crate::tokens::MIN_VIEWPORT_ZOOM);
        let pan_x = (canvas_w - glyph_w * zoom) * 0.5;
        let pan_y = canvas_h * 0.5 + (glyph_h * zoom * 0.5);
        self.viewport.zoom = zoom;
        self.viewport.pan_offset = ScreenPoint::new(pan_x, pan_y);
    }

    #[must_use]
    pub fn is_contour_closed(&self, c_idx: usize) -> bool {
        if let Some(gc) = self.glyph.contours.get(c_idx) {
            gc.closed
        } else if c_idx + 1 == self.contours.len() {
            self.active_contour_closed
        } else {
            true
        }
    }

    #[must_use]
    pub fn closed_flags(&self) -> Vec<bool> {
        (0..self.contours.len())
            .map(|i| self.is_contour_closed(i))
            .collect()
    }

    pub fn set_contour_closed(&mut self, c_idx: usize, closed: bool) {
        while self.glyph.contours.len() <= c_idx && self.glyph.contours.len() < self.contours.len()
        {
            let idx = self.glyph.contours.len();
            let mut gc = crate::core::GlyphContour::new(
                crate::core::ContourId(idx),
                self.contours[idx].clone(),
            );
            gc.closed = if idx + 1 == self.contours.len() {
                self.active_contour_closed
            } else {
                true
            };
            self.glyph.contours.push(gc);
        }
        if let Some(c) = self.glyph.contours.get_mut(c_idx) {
            c.closed = closed;
        }
        if c_idx + 1 == self.contours.len() || self.active_open_contour == Some(c_idx) {
            self.active_contour_closed = closed;
        }
        self.bump_content_generation();
    }

    pub fn sync_paths_from_contours(&mut self) {
        self.paths.clear();
        let metrics_y = [
            crate::tokens::DEFAULT_BASELINE_UPM,
            crate::tokens::DEFAULT_X_HEIGHT_UPM,
            crate::tokens::DEFAULT_CAP_HEIGHT_UPM,
            crate::tokens::DEFAULT_ASCENDER_UPM,
            crate::tokens::DEFAULT_DESCENDER_UPM,
        ];
        for (c_idx, contour) in self.contours.iter().enumerate() {
            if contour.is_empty() {
                self.paths.push(BezPath::new());
                continue;
            }
            let is_closed = self.is_contour_closed(c_idx);
            let path = crate::core::contour_to_bezpath_with_splines(
                contour,
                is_closed,
                self.italic_angle_deg,
                &metrics_y,
            );
            self.paths.push(path);
        }
        let old_closed: Vec<bool> = (0..self.contours.len())
            .map(|i| self.is_contour_closed(i))
            .collect();
        let old_meta: Vec<(crate::core::ShapeBooleanRole, bool, bool)> = (0..self.contours.len())
            .map(|i| {
                self.glyph
                    .contours
                    .get(i)
                    .map_or((crate::core::ShapeBooleanRole::Normal, true, false), |gc| {
                        (gc.role, gc.visible, gc.locked)
                    })
            })
            .collect();
        self.glyph.contours = self
            .contours
            .iter()
            .enumerate()
            .map(|(idx, nodes)| {
                let mut gc =
                    crate::core::GlyphContour::new(crate::core::ContourId(idx), nodes.clone());
                gc.closed = old_closed[idx];
                gc.role = old_meta[idx].0;
                gc.visible = old_meta[idx].1;
                gc.locked = old_meta[idx].2;
                gc
            })
            .collect();
        self.prune_stale_node_and_contour_refs();
        self.sync_color_sublayers_from_glyph_contours();
        self.glyph.background_contours = self.background_contours.clone();
        self.glyph.anchors = self.anchors.clone();
        self.sync_active_glyph_to_text_stream();
        self.bump_content_generation();
    }

    fn prune_stale_node_and_contour_refs(&mut self) {
        let mut valid_ids = std::collections::HashSet::new();
        for contour in &self.contours {
            for n in contour {
                valid_ids.insert(n.id);
                if n.id.0 >= self.next_node_id {
                    self.next_node_id = n.id.0 + 1;
                }
            }
        }
        let mut seen_sel = std::collections::HashSet::with_capacity(self.selected_nodes.len());
        self.selected_nodes
            .retain(|id| valid_ids.contains(id) && seen_sel.insert(*id));
        let (c_len, pen_idle) = (self.contours.len(), self.pen_state == PenToolState::Idle);
        if self
            .active_open_contour
            .is_some_and(|i| i >= c_len || (pen_idle && self.is_contour_closed(i)))
        {
            self.active_open_contour = None;
        }
        if self
            .pending_pen_bcp_out
            .is_some_and(|(i, _)| i >= c_len || (pen_idle && self.is_contour_closed(i)))
        {
            self.pending_pen_bcp_out = None;
        }
        if self
            .pending_pen_first_bcp_in
            .is_some_and(|(i, _)| i >= c_len || (pen_idle && self.is_contour_closed(i)))
        {
            self.pending_pen_first_bcp_in = None;
        }
        if self
            .hovered_pen_close_target
            .is_some_and(|(i, id)| i >= c_len || !valid_ids.contains(&id))
        {
            self.hovered_pen_close_target = None;
        }
        if self
            .hovered_segment_hit
            .as_ref()
            .is_some_and(|h| h.contour_idx >= c_len)
        {
            self.hovered_segment_hit = None;
        }
    }

    #[must_use]
    pub fn find_node(&self, id: NodeId) -> Option<&Node> {
        self.contours
            .iter()
            .flat_map(|c| c.iter())
            .find(|n| n.id == id)
    }

    pub fn find_node_mut(&mut self, id: NodeId) -> Option<&mut Node> {
        self.contours
            .iter_mut()
            .flat_map(|c| c.iter_mut())
            .find(|n| n.id == id)
    }

    #[must_use]
    pub const fn content_generation(&self) -> u64 {
        self.content_generation
    }

    pub fn bump_content_generation(&mut self) {
        self.content_generation = crate::adapters::state::next_content_generation();
    }

    pub fn set_appearance_mode(&mut self, mode: crate::tokens::AppearanceMode) {
        self.set_appearance_with_system(mode, self.system_dark_mode);
    }

    pub fn set_system_dark_mode(&mut self, system_is_dark: bool) {
        self.set_appearance_with_system(self.appearance_mode, system_is_dark);
    }

    pub fn set_appearance_with_system(
        &mut self,
        mode: crate::tokens::AppearanceMode,
        system_is_dark: bool,
    ) {
        if self.appearance_mode != mode || self.system_dark_mode != system_is_dark {
            let prev_guide_w = self.theme.edit_canvas.guide_stroke_width_dp;
            let prev_metric_w = self.theme.edit_canvas.metric_line_width_dp;
            let prev_shape = self.theme.edit_canvas.handle_shape;
            let prev_crosshair = self.theme.edit_canvas.full_crosshair_cursor;
            self.appearance_mode = mode;
            self.system_dark_mode = system_is_dark;
            let mut next_theme =
                GrafiaTheme::for_appearance(self.appearance_mode, self.system_dark_mode);
            next_theme.edit_canvas.guide_stroke_width_dp = prev_guide_w;
            next_theme.edit_canvas.metric_line_width_dp = prev_metric_w;
            next_theme.edit_canvas.handle_shape = prev_shape;
            next_theme.edit_canvas.full_crosshair_cursor = prev_crosshair;
            self.theme = next_theme;
            self.bump_content_generation();
        }
    }
}
