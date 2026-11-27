//! Full W3C SVG 1.1 / 2.0 DOM Parser & Exporter backed by COTS `roxmltree` (`SPEC-017 §2.1 & §2.8b`):
//! Path (`M..Z / m..z / A..a` arcs, compact numbers), Basic Shapes (`<rect rx ry>`, `<circle>`,
//! `<ellipse>`, `<line>`, `<polyline>`, `<polygon>`), nested `<g>` groups, top-level shapes,
//! `viewBox` & `transform="translate/scale/rotate/matrix"` normalization, `<defs>` gradients,
//! and `stroke` <-> `ContourStrokeStyle` I/O.

use crate::core::bezier_ops::{Node, NodeId, NodeType};
use crate::core::color_layers::{CanvasColorSublayer, ColorLayerPaint};
use crate::core::coordinates::FontPoint;
use crate::core::fit_and_stroke::{ContourStrokeStyle, LineCapStyle, LineJoinStyle};
use crate::core::glyph_model::{ContourId, GlyphContour};
use crate::core::icon_studio::SymbolLayerTag;
use crate::core::kinetic_svg_io::{export_group_tag_attrs, parse_group_symbol_tag};
use crate::core::math_consts::{
    ICON_STUDIO_MAX_COORD_ABS_UPM, SVG_CLOSE_COINCIDENT_EPS, SVG_DEFAULT_FILL_RGBA,
    SVG_QUAD_TO_CUBIC_RATIO, SVG_SHAPE_TO_PATH_TOL, SVG_VIEWBOX_SIZE_UPM,
};
use crate::core::w3c_svg_defs::{
    format_svg_defs_gradients, inherited_attr_or_style, node_attr_or_style, parse_node_f64,
    parse_svg_defs_gradients_dom, parse_transform_str, resolve_node_paint_and_stroke,
    resolve_svg_viewbox_transform,
};
use kurbo::{Affine, BezPath, Circle, Ellipse, PathEl, Point, Rect, RoundedRect, Shape as _};
use roxmltree::{Document, Node as XmlNode};
use std::collections::BTreeMap;
use std::fmt::Write as _;

/// Complete parsed W3C SVG icon scene (`SPEC-017 §2.1`).
#[derive(Debug, Clone, PartialEq, Default)]
pub struct W3cSvgIconDocument {
    pub sublayers: Vec<CanvasColorSublayer>,
    pub tags: Vec<SymbolLayerTag>,
    pub contour_strokes: BTreeMap<ContourId, ContourStrokeStyle>,
}

fn valid_pt(p: Point) -> bool {
    p.x.is_finite()
        && p.y.is_finite()
        && p.x.abs() <= ICON_STUDIO_MAX_COORD_ABS_UPM
        && p.y.abs() <= ICON_STUDIO_MAX_COORD_ABS_UPM
}

fn push_cubic_nodes(nodes: &mut Vec<Node>, next_nid: &mut u64, c1: Point, c2: Point, p3: Point) {
    for (pt, nt) in [
        (c1, NodeType::OffCurve),
        (c2, NodeType::OffCurve),
        (p3, NodeType::Corner),
    ] {
        let id = NodeId(*next_nid);
        *next_nid += 1;
        nodes.push(Node::new(id, pt.x, pt.y, nt));
    }
}

fn flush_subpath_contour(
    nodes: &mut Vec<Node>,
    closed: &mut bool,
    next_cid: &mut usize,
    out: &mut Vec<GlyphContour>,
) {
    if nodes.len() >= 2 {
        let mut c = GlyphContour::new(ContourId(*next_cid), std::mem::take(nodes));
        c.closed = *closed;
        *next_cid += 1;
        out.push(c);
    } else {
        nodes.clear();
    }
    *closed = false;
}

fn bezpath_to_contours(
    bez: &BezPath,
    affine: Affine,
    next_cid: &mut usize,
    next_nid: &mut u64,
) -> Vec<GlyphContour> {
    let transformed = affine * bez.clone();
    let mut out = Vec::new();
    let mut nodes: Vec<Node> = Vec::new();
    let mut closed = false;
    for el in transformed.elements() {
        match *el {
            PathEl::MoveTo(p) => {
                flush_subpath_contour(&mut nodes, &mut closed, next_cid, &mut out);
                if !valid_pt(p) {
                    return Vec::new();
                }
                let id = NodeId(*next_nid);
                *next_nid += 1;
                nodes.push(Node::new(id, p.x, p.y, NodeType::Corner));
            }
            PathEl::LineTo(p) => {
                if !valid_pt(p) {
                    return Vec::new();
                }
                let is_start = nodes.first().is_some_and(|f| {
                    (f.point.x.0 - p.x).hypot(f.point.y.0 - p.y) < SVG_CLOSE_COINCIDENT_EPS
                });
                if is_start && nodes.len() >= 3 {
                    closed = true;
                    continue;
                }
                let id = NodeId(*next_nid);
                *next_nid += 1;
                nodes.push(Node::new(id, p.x, p.y, NodeType::Corner));
            }
            PathEl::QuadTo(q1, p2) => {
                if !valid_pt(q1) || !valid_pt(p2) {
                    return Vec::new();
                }
                let p0 = nodes.last().map_or(Point::new(q1.x, q1.y), |n| {
                    Point::new(n.point.x.0, n.point.y.0)
                });
                let c1 = p0 + SVG_QUAD_TO_CUBIC_RATIO * (q1 - p0);
                let c2 = p2 + SVG_QUAD_TO_CUBIC_RATIO * (q1 - p2);
                push_cubic_nodes(&mut nodes, next_nid, c1, c2, p2);
            }
            PathEl::CurveTo(c1, c2, p3) => {
                if !valid_pt(c1) || !valid_pt(c2) || !valid_pt(p3) {
                    return Vec::new();
                }
                push_cubic_nodes(&mut nodes, next_nid, c1, c2, p3);
            }
            PathEl::ClosePath => {
                closed = true;
                flush_subpath_contour(&mut nodes, &mut closed, next_cid, &mut out);
            }
        }
    }
    flush_subpath_contour(&mut nodes, &mut closed, next_cid, &mut out);
    out
}

fn parse_points_bezpath(pts_str: &str, closed: bool) -> Option<BezPath> {
    let nums: Vec<f64> = pts_str
        .replace(',', " ")
        .split_whitespace()
        .filter_map(|s| s.parse::<f64>().ok())
        .collect();
    if nums.len() < 4 {
        return None;
    }
    let mut bp = BezPath::new();
    bp.move_to((nums[0], nums[1]));
    let mut i = 2;
    while i + 1 < nums.len() {
        bp.line_to((nums[i], nums[i + 1]));
        i += 2;
    }
    if closed {
        bp.close_path();
    }
    Some(bp)
}

fn is_svg_shape_tag(tag_name: &str) -> bool {
    matches!(
        tag_name,
        "path" | "rect" | "circle" | "ellipse" | "line" | "polygon" | "polyline"
    )
}

fn build_primitive_shape_bezpath(node: XmlNode<'_, '_>) -> Result<BezPath, String> {
    let f = |k: &str| parse_node_f64(node, k, 0.0);
    match node.tag_name().name() {
        "rect" => {
            let (x, y, w, h) = (f("x"), f("y"), f("width"), f("height"));
            if w <= 0.0 || h <= 0.0 {
                return Err("Invalid <rect> dimensions".into());
            }
            let rx = parse_node_f64(node, "rx", f("ry")).max(0.0);
            let rect = Rect::new(x, y, x + w, y + h);
            Ok(if rx > 0.0 {
                RoundedRect::from_rect(rect, rx).to_path(SVG_SHAPE_TO_PATH_TOL)
            } else {
                rect.to_path(SVG_SHAPE_TO_PATH_TOL)
            })
        }
        "circle" => {
            let r = f("r");
            (r > 0.0)
                .then(|| Circle::new((f("cx"), f("cy")), r).to_path(SVG_SHAPE_TO_PATH_TOL))
                .ok_or_else(|| "Invalid <circle> radius".into())
        }
        "ellipse" => {
            let (rx, ry) = (f("rx"), f("ry"));
            (rx > 0.0 && ry > 0.0)
                .then(|| {
                    Ellipse::new((f("cx"), f("cy")), (rx, ry), 0.0).to_path(SVG_SHAPE_TO_PATH_TOL)
                })
                .ok_or_else(|| "Invalid <ellipse> radii".into())
        }
        "line" => {
            let mut b = BezPath::new();
            b.move_to((f("x1"), f("y1")));
            b.line_to((f("x2"), f("y2")));
            Ok(b)
        }
        "polygon" | "polyline" => {
            let pts = node_attr_or_style(node, "points").ok_or("Missing points")?;
            parse_points_bezpath(pts, node.tag_name().name() == "polygon")
                .ok_or_else(|| "Bad points".into())
        }
        _ => Err("Unsupported shape".into()),
    }
}

fn parse_shape_node_contours(
    node: XmlNode<'_, '_>,
    parent_aff: Affine,
    next_cid: &mut usize,
    next_nid: &mut u64,
) -> Result<Vec<GlyphContour>, String> {
    let aff = parent_aff * parse_transform_str(node_attr_or_style(node, "transform"));
    if node.tag_name().name() == "path" {
        let d = node_attr_or_style(node, "d").ok_or("Missing d attribute")?;
        if d.contains(" CS ") || d.contains(" O ") || d.contains(" S ") {
            if let Ok(mut c) = crate::core::kinetic_svg_io::parse_legacy_kinetic_path_d(
                d,
                ContourId(*next_cid),
                next_nid,
            ) {
                for n in &mut c.nodes {
                    let p = aff * Point::new(n.point.x.0, n.point.y.0);
                    n.point = FontPoint::new(p.x, p.y);
                }
                *next_cid += 1;
                return Ok(vec![c]);
            }
        }
        let bp = BezPath::from_svg(d).map_err(|_| "Invalid W3C SVG path d")?;
        let cs = bezpath_to_contours(&bp, aff, next_cid, next_nid);
        return (!cs.is_empty())
            .then_some(cs)
            .ok_or_else(|| "Empty SVG path".into());
    }
    let bp = build_primitive_shape_bezpath(node)?;
    let cs = bezpath_to_contours(&bp, aff, next_cid, next_nid);
    (!cs.is_empty())
        .then_some(cs)
        .ok_or_else(|| "Empty shape".into())
}

struct DomWalkContext<'a> {
    defs: &'a BTreeMap<String, ColorLayerPaint>,
    stroke_scale: f64,
    doc: &'a mut W3cSvgIconDocument,
    next_cid: &'a mut usize,
    next_nid: &'a mut u64,
}

impl DomWalkContext<'_> {
    fn emit_layer_from_shapes(
        &mut self,
        layer_node: XmlNode<'_, '_>,
        shape_nodes: &[XmlNode<'_, '_>],
        parent_aff: Affine,
    ) -> Result<(), String> {
        let mut contours = Vec::new();
        let mut layer_paint = None;
        for &sn in shape_nodes {
            let cs = parse_shape_node_contours(sn, parent_aff, self.next_cid, self.next_nid)?;
            let (p_opt, st_opt) = resolve_node_paint_and_stroke(sn, self.defs, self.stroke_scale);
            if layer_paint.is_none() {
                layer_paint = p_opt;
            }
            if let Some(st) = st_opt {
                for c in &cs {
                    self.doc.contour_strokes.insert(c.id, st);
                }
            }
            contours.extend(cs);
        }
        if contours.is_empty() {
            return Err("SVG layer contains no valid shape contours".into());
        }
        let tag = parse_group_symbol_tag(layer_node);
        let opacity = inherited_attr_or_style(layer_node, "data-opacity")
            .or_else(|| inherited_attr_or_style(layer_node, "opacity"))
            .and_then(|s| s.parse::<f32>().ok())
            .filter(|v| v.is_finite())
            .unwrap_or(1.0)
            .clamp(0.0, 1.0);
        let default_name = format!("Layer{}", self.doc.sublayers.len() + 1);
        let name = layer_node
            .attribute("id")
            .unwrap_or(&default_name)
            .to_string();
        self.doc.sublayers.push(CanvasColorSublayer {
            id: (self.doc.sublayers.len() + 1) as u64,
            name,
            contours,
            paint: layer_paint.unwrap_or(ColorLayerPaint::Solid {
                rgba: SVG_DEFAULT_FILL_RGBA,
            }),
            opacity,
            visible: true,
            locked: false,
        });
        self.doc.tags.push(tag);
        Ok(())
    }

    fn walk_container(
        &mut self,
        container: XmlNode<'_, '_>,
        parent_aff: Affine,
    ) -> Result<(), String> {
        let cur_aff = parent_aff * parse_transform_str(node_attr_or_style(container, "transform"));
        let is_group = container.tag_name().name().eq_ignore_ascii_case("g");
        let has_child_groups = container
            .children()
            .filter(XmlNode::is_element)
            .any(|c| c.tag_name().name().eq_ignore_ascii_case("g"));
        if is_group && !has_child_groups {
            let shapes: Vec<_> = container
                .children()
                .filter(|c| c.is_element() && is_svg_shape_tag(c.tag_name().name()))
                .collect();
            if !shapes.is_empty() {
                self.emit_layer_from_shapes(container, &shapes, cur_aff)?;
            }
            return Ok(());
        }
        for child in container.children().filter(XmlNode::is_element) {
            let tname = child.tag_name().name();
            if tname.eq_ignore_ascii_case("defs") {
                continue;
            }
            if tname.eq_ignore_ascii_case("g") {
                self.walk_container(child, cur_aff)?;
            } else if is_svg_shape_tag(tname) {
                self.emit_layer_from_shapes(child, &[child], cur_aff)?;
            }
        }
        Ok(())
    }
}

/// Parses a full W3C SVG string into `W3cSvgIconDocument` via COTS `roxmltree` (`SPEC-017 §2.1 & §2.8b`).
pub fn import_w3c_icon_svg(svg_xml: &str) -> Result<W3cSvgIconDocument, String> {
    let xml_doc = Document::parse(svg_xml).map_err(|e| format!("Invalid SVG XML: {e}"))?;
    let svg_root = xml_doc
        .descendants()
        .find(|n| n.is_element() && n.tag_name().name().eq_ignore_ascii_case("svg"))
        .ok_or_else(|| "Missing <svg> root element".to_string())?;
    let (vb_aff, stroke_scale) = resolve_svg_viewbox_transform(svg_root);
    let defs = parse_svg_defs_gradients_dom(svg_root, vb_aff, stroke_scale);
    let mut doc = W3cSvgIconDocument::default();
    let (mut next_nid, mut next_cid) = (1_u64, 0_usize);
    let mut ctx = DomWalkContext {
        defs: &defs,
        stroke_scale,
        doc: &mut doc,
        next_cid: &mut next_cid,
        next_nid: &mut next_nid,
    };
    ctx.walk_container(svg_root, vb_aff)?;
    (!doc.sublayers.is_empty())
        .then_some(doc)
        .ok_or_else(|| "SVG contains no valid shape layers".into())
}

/// Exports `sublayers`, `tags`, and `contour_strokes` to a W3C SVG with `<defs>` gradients (`SPEC-017 §2.1`).
#[must_use]
pub fn export_w3c_icon_svg(
    sublayers: &[CanvasColorSublayer],
    tags: &[SymbolLayerTag],
    contour_strokes: &BTreeMap<ContourId, ContourStrokeStyle>,
) -> String {
    let mut out = format!(
        "<svg xmlns=\"http://www.w3.org/2000/svg\" viewBox=\"0 0 {SVG_VIEWBOX_SIZE_UPM} {SVG_VIEWBOX_SIZE_UPM}\">\n"
    );
    let defs_str = format_svg_defs_gradients(sublayers);
    if !defs_str.is_empty() {
        out.push_str("  <defs>\n");
        out.push_str(&defs_str);
        out.push_str("  </defs>\n");
    }
    for (idx, sub) in sublayers.iter().enumerate() {
        let tag = tags.get(idx).copied().unwrap_or_default();
        export_group_tag_attrs(&mut out, idx, sub, tag);
        for c in &sub.contours {
            let d = crate::core::kinetic_svg_io::contour_to_svg_path_d(c);
            if !d.is_empty() {
                if let Some(st) = contour_strokes.get(&c.id) {
                    let cap = match st.cap_start {
                        LineCapStyle::Round => "round",
                        LineCapStyle::Square => "square",
                        LineCapStyle::Butt => "butt",
                    };
                    let join = match st.join {
                        LineJoinStyle::Round => "round",
                        LineJoinStyle::Bevel => "bevel",
                        LineJoinStyle::Miter => "miter",
                    };
                    let _ = writeln!(
                        out,
                        "    <path d=\"{d}\" stroke=\"#000000\" stroke-width=\"{:.4}\" stroke-linecap=\"{cap}\" stroke-linejoin=\"{join}\"/>",
                        st.width
                    );
                } else {
                    let _ = writeln!(out, "    <path d=\"{d}\"/>");
                }
            }
        }
        out.push_str("  </g>\n");
    }
    out.push_str("</svg>");
    out
}
