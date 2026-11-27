//! GPU triangle mesh tessellation via `lyon_tessellation` (STD-ARCH, SPEC-002 §2.1 & §5.2).
//!
//! Provides Non-Zero Winding (`FillRule::NonZero`) fill tessellation for multi-contour compound glyphs with counter-holes.

use crate::core::math_consts::LYON_FILL_TESSELLATION_TOLERANCE_DP;
use crate::core::{FontPoint, Node, ViewportTransform};
use egui::epaint::{Vertex, WHITE_UV};
use egui::{pos2, Mesh};
use lyon_tessellation::math::point as lyon_point;
use lyon_tessellation::path::Path as LyonPath;
use lyon_tessellation::{
    BuffersBuilder, FillOptions, FillRule, FillTessellator, FillVertex, FillVertexConstructor,
    VertexBuffers,
};

/// Vertex constructor mapping Lyon tessellated points to `egui::epaint::Vertex`.
struct EguiFillVertexCtor(egui::Color32);

impl FillVertexConstructor<Vertex> for EguiFillVertexCtor {
    fn new_vertex(&mut self, vertex: FillVertex) -> Vertex {
        let p = vertex.position();
        Vertex {
            pos: pos2(p.x, p.y),
            uv: WHITE_UV,
            color: self.0,
        }
    }
}

fn tessellate_node_slices_nonzero<'a, I>(
    contours: I,
    viewport: &ViewportTransform,
    fill_color: egui::Color32,
) -> egui::Mesh
where
    I: IntoIterator<Item = (&'a [Node], bool)>,
{
    let mut mesh = Mesh::default();
    let mut builder = LyonPath::builder();
    let mut has_geometry = false;

    for (contour, is_closed) in contours {
        if !is_closed || contour.len() < 2 {
            continue;
        }
        let bez = crate::render::canvas_render_cache::cached_contour_bezpath(contour, true);
        let mut opened = false;
        for el in bez.elements() {
            match *el {
                kurbo::PathEl::MoveTo(pt) => {
                    if opened {
                        builder.end(true);
                    }
                    let p0 = viewport.to_screen(FontPoint::new(pt.x, pt.y));
                    builder.begin(lyon_point(p0.x.0, p0.y.0));
                    opened = true;
                    has_geometry = true;
                }
                kurbo::PathEl::LineTo(pt) if opened => {
                    let p = viewport.to_screen(FontPoint::new(pt.x, pt.y));
                    builder.line_to(lyon_point(p.x.0, p.y.0));
                }
                kurbo::PathEl::QuadTo(q1, p2) if opened => {
                    let c1 = viewport.to_screen(FontPoint::new(q1.x, q1.y));
                    let p = viewport.to_screen(FontPoint::new(p2.x, p2.y));
                    builder
                        .quadratic_bezier_to(lyon_point(c1.x.0, c1.y.0), lyon_point(p.x.0, p.y.0));
                }
                kurbo::PathEl::CurveTo(q1, q2, p3) if opened => {
                    let h1 = viewport.to_screen(FontPoint::new(q1.x, q1.y));
                    let h2 = viewport.to_screen(FontPoint::new(q2.x, q2.y));
                    let p = viewport.to_screen(FontPoint::new(p3.x, p3.y));
                    builder.cubic_bezier_to(
                        lyon_point(h1.x.0, h1.y.0),
                        lyon_point(h2.x.0, h2.y.0),
                        lyon_point(p.x.0, p.y.0),
                    );
                }
                kurbo::PathEl::ClosePath if opened => {
                    builder.end(true);
                    opened = false;
                }
                _ => {}
            }
        }
        if opened {
            builder.end(true);
        }
    }

    if !has_geometry {
        return mesh;
    }

    let lyon_path = builder.build();
    let mut buffers: VertexBuffers<Vertex, u32> = VertexBuffers::new();
    let mut tessellator = FillTessellator::new();
    let options = FillOptions::default()
        .with_fill_rule(FillRule::NonZero)
        .with_tolerance(LYON_FILL_TESSELLATION_TOLERANCE_DP);

    if tessellator
        .tessellate_path(
            &lyon_path,
            &options,
            &mut BuffersBuilder::new(&mut buffers, EguiFillVertexCtor(fill_color)),
        )
        .is_ok()
    {
        mesh.vertices = buffers.vertices;
        mesh.indices = buffers.indices;
    }

    mesh
}

/// Tessellates closed glyph contours into an `egui::Mesh` using `FillRule::NonZero` (`SPEC-002 §5.2`).
///
/// Open contours (`closed_flags[i] == false`) are excluded from the filled silhouette, matching
/// `GlyphLayer::completeBezierPath`. Inner counter-holes with opposite winding direction (`CW` inside `CCW`)
/// are naturally subtracted by the topological Non-Zero winding rule, while same-winding contours fill solid.
#[must_use]
pub fn tessellate_closed_contours_nonzero(
    contours: &[Vec<Node>],
    closed_flags: &[bool],
    viewport: &ViewportTransform,
    fill_color: egui::Color32,
) -> egui::Mesh {
    let iter = contours.iter().enumerate().map(|(c_idx, contour)| {
        let is_closed = closed_flags.get(c_idx).copied().unwrap_or(true);
        (contour.as_slice(), is_closed)
    });
    tessellate_node_slices_nonzero(iter, viewport, fill_color)
}

/// Tessellates a slice of `GlyphContour` directly into an `egui::Mesh` using `FillRule::NonZero`
/// without heap-cloning `Vec<Node>` buffers (`SPEC-008`).
#[must_use]
pub fn tessellate_glyph_contours_nonzero(
    contours: &[crate::core::GlyphContour],
    viewport: &ViewportTransform,
    fill_color: egui::Color32,
) -> egui::Mesh {
    let iter = contours
        .iter()
        .filter(|c| c.visible)
        .map(|c| (c.nodes.as_slice(), c.closed));
    tessellate_node_slices_nonzero(iter, viewport, fill_color)
}

/// Tessellates multiple closed glyph contours into an `egui::Mesh` using `FillRule::NonZero` (`SPEC-002 §5.2`).
#[must_use]
pub fn tessellate_contours_nonzero(
    contours: &[Vec<Node>],
    viewport: &ViewportTransform,
    fill_color: egui::Color32,
) -> egui::Mesh {
    let all_closed = vec![true; contours.len()];
    tessellate_closed_contours_nonzero(contours, &all_closed, viewport, fill_color)
}

/// Backwards-compatible alias for [`tessellate_contours_nonzero`].
#[must_use]
pub fn tessellate_contours_even_odd(
    contours: &[Vec<Node>],
    viewport: &ViewportTransform,
    fill_color: egui::Color32,
) -> egui::Mesh {
    tessellate_contours_nonzero(contours, viewport, fill_color)
}

/// Tessellates a closed `kurbo::BezPath` translated by `offset` into an `egui::Mesh` using `FillRule::NonZero`.
#[must_use]
pub fn tessellate_bez_path_nonzero(
    path: &kurbo::BezPath,
    offset: egui::Pos2,
    fill_color: egui::Color32,
) -> egui::Mesh {
    let mut mesh = Mesh::default();
    let mut builder = LyonPath::builder();
    let mut in_subpath = false;
    let mut has_geometry = false;
    let to_pt = |p: kurbo::Point| lyon_point(offset.x + p.x as f32, offset.y + p.y as f32);

    for el in path.elements() {
        match *el {
            kurbo::PathEl::MoveTo(p) => {
                if in_subpath {
                    builder.end(true);
                }
                builder.begin(to_pt(p));
                in_subpath = true;
            }
            kurbo::PathEl::LineTo(p) => {
                if in_subpath {
                    builder.line_to(to_pt(p));
                    has_geometry = true;
                }
            }
            kurbo::PathEl::QuadTo(p1, p2) => {
                if in_subpath {
                    builder.quadratic_bezier_to(to_pt(p1), to_pt(p2));
                    has_geometry = true;
                }
            }
            kurbo::PathEl::CurveTo(p1, p2, p3) => {
                if in_subpath {
                    builder.cubic_bezier_to(to_pt(p1), to_pt(p2), to_pt(p3));
                    has_geometry = true;
                }
            }
            kurbo::PathEl::ClosePath => {
                if in_subpath {
                    builder.end(true);
                    in_subpath = false;
                }
            }
        }
    }
    if in_subpath {
        builder.end(true);
    }
    if !has_geometry {
        return mesh;
    }

    let lyon_path = builder.build();
    let mut buffers: VertexBuffers<Vertex, u32> = VertexBuffers::new();
    let mut tessellator = FillTessellator::new();
    let options = FillOptions::default()
        .with_fill_rule(FillRule::NonZero)
        .with_tolerance(LYON_FILL_TESSELLATION_TOLERANCE_DP);
    if tessellator
        .tessellate_path(
            &lyon_path,
            &options,
            &mut BuffersBuilder::new(&mut buffers, EguiFillVertexCtor(fill_color)),
        )
        .is_ok()
    {
        mesh.vertices = buffers.vertices;
        mesh.indices = buffers.indices;
    }
    mesh
}

/// Returns `true` if the closed polyline `points` is strictly convex with respect to both edge turns and fan origin.
#[must_use]
pub fn is_polygon_convex(points: &[egui::Pos2]) -> bool {
    let mut clean: Vec<egui::Pos2> = Vec::with_capacity(points.len());
    for &p in points {
        if clean
            .last()
            .is_none_or(|last| (*last - p).length_sq() > 0.5)
        {
            clean.push(p);
        }
    }
    if clean.len() >= 2
        && clean
            .last()
            .is_some_and(|last| (clean[0] - *last).length_sq() <= 0.5)
    {
        clean.pop();
    }
    let n = clean.len();
    if n < 3 {
        return true;
    }
    let p0 = clean[0];
    let mut fan_pos = false;
    let mut fan_neg = false;
    for i in 2..n {
        let u = clean[i - 1] - p0;
        let v = clean[i] - p0;
        let fan_cross = u.x * v.y - u.y * v.x;
        if fan_cross > 0.5 {
            fan_pos = true;
        } else if fan_cross < -0.5 {
            fan_neg = true;
        }
        if fan_pos && fan_neg {
            return false;
        }
    }
    let mut turn_pos = false;
    let mut turn_neg = false;
    for i in 0..n {
        let u = clean[(i + 1) % n] - clean[i];
        let v = clean[(i + 2) % n] - clean[(i + 1) % n];
        let cross = u.x * v.y - u.y * v.x;
        if cross > 0.0 {
            turn_pos = true;
        } else if cross < 0.0 {
            turn_neg = true;
        }
        if turn_pos && turn_neg {
            return false;
        }
    }
    true
}
