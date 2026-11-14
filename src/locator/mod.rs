//! ShapeLocator micro-viewport vector minimap (SPEC-002 Deel I §5.2, TC-LOCATOR-01).
//!
//! Provides a compact, live vector micro-viewport for glyph contours in sidebar panels.
//! The minimap is strictly NOT a raster thumbnail: it renders the full glyph in a subtle
//! context layer and highlights the target contour in a distinct focus layer.

use crate::core::{
    compute_signed_area, extract_segments, ContourId, ContourSegment, FontPoint, Glyph, Node,
    ShapeBooleanRole,
};
use crate::tokens::{
    locator_cell_bg_color_for_mode, locator_cell_border_color_for_mode,
    locator_context_color_for_mode, locator_default_context_color, locator_default_focus_color,
    locator_default_mask_color, LOCATOR_CELL_BORDER_WIDTH_DP, LOCATOR_CELL_CORNER_RADIUS_DP,
    LOCATOR_DEFAULT_HEIGHT_DP, LOCATOR_DEFAULT_PADDING_DP, LOCATOR_DEFAULT_WIDTH_DP,
    LOCATOR_OPEN_CONTOUR_STROKE_WIDTH_DP,
};
use egui::epaint::{Vertex, WHITE_UV};
use egui::{pos2, Color32, Mesh, Vec2};
use lyon_tessellation::math::point as lyon_point;
use lyon_tessellation::path::Path as LyonPath;
use lyon_tessellation::{
    BuffersBuilder, FillOptions, FillRule, FillTessellator, FillVertex, FillVertexConstructor,
    StrokeOptions, StrokeTessellator, StrokeVertex, StrokeVertexConstructor, VertexBuffers,
};

/// Vertex constructor mapping Lyon points to egui vertices.
struct EguiLocatorVertexCtor(Color32);

impl FillVertexConstructor<Vertex> for EguiLocatorVertexCtor {
    fn new_vertex(&mut self, vertex: FillVertex) -> Vertex {
        let p = vertex.position();
        Vertex {
            pos: pos2(p.x, p.y),
            uv: WHITE_UV,
            color: self.0,
        }
    }
}

impl StrokeVertexConstructor<Vertex> for EguiLocatorVertexCtor {
    fn new_vertex(&mut self, vertex: StrokeVertex) -> Vertex {
        let p = vertex.position();
        Vertex {
            pos: pos2(p.x, p.y),
            uv: WHITE_UV,
            color: self.0,
        }
    }
}

/// Visual styling configuration for the `ShapeLocator` micro-viewport.
#[derive(Debug, Clone, PartialEq)]
pub struct ShapeLocatorStyle {
    pub context_color: Color32,
    pub focus_color: Color32,
    pub mask_color: Color32,
    pub size: Vec2,
    pub padding: f32,
}

impl Default for ShapeLocatorStyle {
    fn default() -> Self {
        Self::for_dark_mode(false)
    }
}

impl ShapeLocatorStyle {
    /// Returns a `ShapeLocatorStyle` calibrated for Light (`is_dark == false`) or Dark (`is_dark == true`) sidebar chrome.
    #[must_use]
    pub fn for_dark_mode(is_dark: bool) -> Self {
        Self {
            context_color: locator_context_color_for_mode(is_dark),
            focus_color: locator_default_focus_color(),
            mask_color: locator_default_mask_color(),
            size: Vec2::new(LOCATOR_DEFAULT_WIDTH_DP, LOCATOR_DEFAULT_HEIGHT_DP),
            padding: LOCATOR_DEFAULT_PADDING_DP,
        }
    }
}

/// Vector renderer projecting glyph geometry to a two-layer micro-viewport mesh.
pub struct ShapeLocatorRenderer<'a> {
    pub glyph: &'a Glyph,
    pub style: ShapeLocatorStyle,
}

impl<'a> ShapeLocatorRenderer<'a> {
    #[must_use]
    pub fn new(glyph: &'a Glyph, style: ShapeLocatorStyle) -> Self {
        Self { glyph, style }
    }

    fn visible_glyph_bounds(&self) -> (f64, f64, f64, f64) {
        let mut min_x = f64::INFINITY;
        let mut max_x = f64::NEG_INFINITY;
        let mut min_y = f64::INFINITY;
        let mut max_y = f64::NEG_INFINITY;

        for contour in &self.glyph.contours {
            if !contour.visible {
                continue;
            }
            for node in &contour.nodes {
                min_x = min_x.min(node.point.x.0);
                max_x = max_x.max(node.point.x.0);
                min_y = min_y.min(node.point.y.0);
                max_y = max_y.max(node.point.y.0);
            }
        }
        if !min_x.is_finite() || !max_x.is_finite() || !min_y.is_finite() || !max_y.is_finite() {
            let upm = self.glyph.upm as f64;
            (0.0, upm, 0.0, upm)
        } else {
            (min_x, max_x, min_y, max_y)
        }
    }

    fn append_segments_to_lyon_builder(
        builder: &mut lyon_tessellation::path::path::Builder,
        segments: &[ContourSegment],
        closed: bool,
        to_locator_pt: &impl Fn(FontPoint) -> lyon_tessellation::math::Point,
    ) {
        let start_pt = match segments[0] {
            ContourSegment::Line { line, .. } => FontPoint::new(line.p0.x, line.p0.y),
            ContourSegment::Cubic { cubic, .. } => FontPoint::new(cubic.p0.x, cubic.p0.y),
        };
        builder.begin(to_locator_pt(start_pt));
        for seg in segments {
            match seg {
                ContourSegment::Line { line, .. } => {
                    builder.line_to(to_locator_pt(FontPoint::new(line.p1.x, line.p1.y)));
                }
                ContourSegment::Cubic { cubic, .. } => {
                    builder.cubic_bezier_to(
                        to_locator_pt(FontPoint::new(cubic.p1.x, cubic.p1.y)),
                        to_locator_pt(FontPoint::new(cubic.p2.x, cubic.p2.y)),
                        to_locator_pt(FontPoint::new(cubic.p3.x, cubic.p3.y)),
                    );
                }
            }
        }
        builder.end(closed);
    }

    /// Computes the affine micro-viewport projection and generates `(context_mesh, focus_mesh)`.
    #[must_use]
    pub fn render_locator(
        &self,
        target_contour_id: impl Into<ContourId>,
        size: Vec2,
    ) -> (Mesh, Mesh) {
        let (min_x, max_x, min_y, max_y) = self.visible_glyph_bounds();
        let w = (max_x - min_x).max(1.0);
        let h = (max_y - min_y).max(1.0);
        let pad = self.style.padding as f64;
        let avail_w = (size.x as f64 - (pad * 2.0)).max(1.0);
        let avail_h = (size.y as f64 - (pad * 2.0)).max(1.0);
        let scale = (avail_w / w).min(avail_h / h);
        let offset_x = pad + (avail_w - w * scale) * 0.5;
        let offset_y = pad + (avail_h - h * scale) * 0.5;

        let to_locator_pt = |pt: crate::core::FontPoint| -> lyon_tessellation::math::Point {
            let sx = offset_x + (pt.x.0 - min_x) * scale;
            let sy = offset_y + (max_y - pt.y.0) * scale;
            lyon_point(sx as f32, sy as f32)
        };

        let mut context_builder = LyonPath::builder();
        let mut has_context = false;
        for contour in &self.glyph.contours {
            if !contour.visible || !contour.closed || contour.nodes.len() < 2 {
                continue;
            }
            let segments = extract_segments(&contour.nodes, contour.closed);
            if !segments.is_empty() {
                Self::append_segments_to_lyon_builder(
                    &mut context_builder,
                    &segments,
                    contour.closed,
                    &to_locator_pt,
                );
                has_context = true;
            }
        }

        let mut context_mesh = Mesh::default();
        if has_context {
            let path = context_builder.build();
            let mut buffers: VertexBuffers<Vertex, u32> = VertexBuffers::new();
            let mut tessellator = FillTessellator::new();
            let options = FillOptions::default().with_fill_rule(FillRule::NonZero);
            if tessellator
                .tessellate_path(
                    &path,
                    &options,
                    &mut BuffersBuilder::new(
                        &mut buffers,
                        EguiLocatorVertexCtor(self.style.context_color),
                    ),
                )
                .is_ok()
            {
                context_mesh.vertices = buffers.vertices;
                context_mesh.indices = buffers.indices;
            }
        }

        let target_id = target_contour_id.into();
        let focus_mesh = self.tessellate_focus_contour_mesh(target_id, &to_locator_pt);
        (context_mesh, focus_mesh)
    }

    fn tessellate_focus_contour_mesh(
        &self,
        target_id: ContourId,
        to_locator_pt: &impl Fn(FontPoint) -> lyon_tessellation::math::Point,
    ) -> Mesh {
        let mut focus_mesh = Mesh::default();
        let Some(target_contour) = self.glyph.contours.iter().find(|c| c.id == target_id) else {
            return focus_mesh;
        };
        let segments = extract_segments(&target_contour.nodes, target_contour.closed);
        if !target_contour.visible || segments.is_empty() {
            return focus_mesh;
        }
        let mut focus_builder = LyonPath::builder();
        Self::append_segments_to_lyon_builder(
            &mut focus_builder,
            &segments,
            target_contour.closed,
            to_locator_pt,
        );
        let path = focus_builder.build();
        let color = match target_contour.role {
            ShapeBooleanRole::Mask => self.style.mask_color,
            _ => self.style.focus_color,
        };
        let mut buffers: VertexBuffers<Vertex, u32> = VertexBuffers::new();
        let tessellated_ok = if target_contour.closed {
            let mut tessellator = FillTessellator::new();
            let options = FillOptions::default().with_fill_rule(FillRule::NonZero);
            tessellator
                .tessellate_path(
                    &path,
                    &options,
                    &mut BuffersBuilder::new(&mut buffers, EguiLocatorVertexCtor(color)),
                )
                .is_ok()
        } else {
            let mut tessellator = StrokeTessellator::new();
            let options =
                StrokeOptions::default().with_line_width(LOCATOR_OPEN_CONTOUR_STROKE_WIDTH_DP);
            tessellator
                .tessellate_path(
                    &path,
                    &options,
                    &mut BuffersBuilder::new(&mut buffers, EguiLocatorVertexCtor(color)),
                )
                .is_ok()
        };
        if tessellated_ok {
            focus_mesh.vertices = buffers.vertices;
            focus_mesh.indices = buffers.indices;
        }
        focus_mesh
    }
}

/// Returns `"CW"` or `"CCW"` for the topological winding direction of `nodes` in Y-up font space.
#[must_use]
pub fn contour_winding_label(nodes: &[Node]) -> &'static str {
    let pts: Vec<FontPoint> = nodes.iter().map(|n| n.point).collect();
    if compute_signed_area(&pts) < 0.0 {
        "CW"
    } else {
        "CCW"
    }
}

/// An immediate-mode egui widget rendering a `ShapeLocator` micro-viewport.
pub struct ShapeLocatorWidget<'a> {
    pub glyph: &'a Glyph,
    pub target_contour_id: ContourId,
    pub style: ShapeLocatorStyle,
}

impl<'a> ShapeLocatorWidget<'a> {
    #[must_use]
    pub fn new(glyph: &'a Glyph, target_contour_id: impl Into<ContourId>) -> Self {
        Self {
            glyph,
            target_contour_id: target_contour_id.into(),
            style: ShapeLocatorStyle::default(),
        }
    }

    #[must_use]
    pub fn with_style(mut self, style: ShapeLocatorStyle) -> Self {
        self.style = style;
        self
    }

    pub fn show(mut self, ui: &mut egui::Ui) -> egui::Response {
        let (rect, response) = ui.allocate_exact_size(self.style.size, egui::Sense::click());
        if ui.is_rect_visible(rect) {
            let is_dark = ui.visuals().dark_mode;
            if is_dark && self.style.context_color == locator_default_context_color() {
                self.style.context_color = locator_context_color_for_mode(true);
            }
            ui.painter().rect(
                rect,
                LOCATOR_CELL_CORNER_RADIUS_DP,
                locator_cell_bg_color_for_mode(is_dark),
                egui::Stroke::new(
                    LOCATOR_CELL_BORDER_WIDTH_DP,
                    locator_cell_border_color_for_mode(is_dark),
                ),
                egui::StrokeKind::Inside,
            );
            let renderer = ShapeLocatorRenderer::new(self.glyph, self.style.clone());
            let (mut context_mesh, mut focus_mesh) =
                renderer.render_locator(self.target_contour_id, self.style.size);
            let offset = rect.min.to_vec2();
            if !context_mesh.is_empty() {
                context_mesh.translate(offset);
                ui.painter().add(egui::Shape::mesh(context_mesh));
            }
            if !focus_mesh.is_empty() {
                focus_mesh.translate(offset);
                ui.painter().add(egui::Shape::mesh(focus_mesh));
            }
        }
        response
    }
}

impl egui::Widget for ShapeLocatorWidget<'_> {
    fn ui(self, ui: &mut egui::Ui) -> egui::Response {
        self.show(ui)
    }
}
