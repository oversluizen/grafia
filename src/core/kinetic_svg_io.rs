//! Lossless Layered Kinetic SVG Import & Export (`SPEC-016 §5.3` & `SPEC-017 §2.1`).
//!
//! Exports and imports `CanvasColorSublayer` and `SymbolLayerTag` stacks as W3C-compatible
//! SVG `<g>` and `<path d="M... L... C... Z"/>` elements with `data-layer-group`,
//! `data-motion-anchor`, `data-pivot-x`, `data-pivot-y`, `data-variable-color`,
//! `data-pulse-participates`, and `data-draw-order` attributes.

use crate::core::bezier_ops::{Node, NodeId, NodeType};
use crate::core::color_layers::{CanvasColorSublayer, ColorLayerPaint};
use crate::core::coordinates::FontPoint;
use crate::core::glyph_model::{ContourId, GlyphContour};
use crate::core::icon_color_modes::{DrawDirection, SymbolHierarchyLevel};
use crate::core::icon_studio::SymbolLayerTag;
use crate::core::math_consts::{
    ICON_STUDIO_MAX_COORD_ABS_UPM, KNOCKOUT_MARGIN_MAX_UPM, RADIAL_SYMMETRY_MAX_FOLDS,
    SVG_BYTE_MAX_F32, SVG_CUBIC_ARGS_LEN, SVG_DEFAULT_FILL_RGBA,
};
use crate::core::selection_info::AnchorId;
use crate::core::w3c_svg_defs::node_attr_or_style;
use crate::core::w3c_svg_io::{export_w3c_icon_svg, import_w3c_icon_svg};
use roxmltree::Node as XmlNode;
use std::collections::BTreeMap;
use std::fmt::Write as _;

pub(crate) fn contour_to_svg_path_d(contour: &GlyphContour) -> String {
    if contour.nodes.is_empty() {
        return String::new();
    }
    let mut d = String::new();
    let first = &contour.nodes[0];
    let _ = write!(d, "M {:.4} {:.4}", first.point.x.0, first.point.y.0);
    let mut i = 1;
    while i < contour.nodes.len() {
        if contour.nodes[i].node_type == NodeType::OffCurve
            && i + 2 < contour.nodes.len()
            && contour.nodes[i + 1].node_type == NodeType::OffCurve
        {
            let (h1, h2, p3) = (
                &contour.nodes[i],
                &contour.nodes[i + 1],
                &contour.nodes[i + 2],
            );
            let tag = if p3.node_type == NodeType::Smooth {
                "CS"
            } else {
                "C"
            };
            let _ = write!(
                d,
                " {tag} {:.4} {:.4} {:.4} {:.4} {:.4} {:.4}",
                h1.point.x.0, h1.point.y.0, h2.point.x.0, h2.point.y.0, p3.point.x.0, p3.point.y.0
            );
            i += 3;
        } else {
            let n = &contour.nodes[i];
            let cmd = match n.node_type {
                NodeType::OffCurve => "O",
                NodeType::Smooth => "S",
                _ => "L",
            };
            let _ = write!(d, " {cmd} {:.4} {:.4}", n.point.x.0, n.point.y.0);
            i += 1;
        }
    }
    if contour.closed {
        d.push_str(" Z");
    }
    d
}

fn valid_f64(v: f64) -> Option<f64> {
    (v.is_finite() && v.abs() <= ICON_STUDIO_MAX_COORD_ABS_UPM).then_some(v)
}

fn next_valid_coord<'a>(tokens: &mut impl Iterator<Item = &'a str>) -> Result<f64, String> {
    let raw = tokens
        .next()
        .ok_or("Missing SVG coordinate")?
        .parse::<f64>()
        .map_err(|_| "Bad SVG coordinate")?;
    valid_f64(raw).ok_or_else(|| "Coordinate out of bounds".into())
}

pub(crate) fn parse_legacy_kinetic_path_d(
    d_attr: &str,
    contour_id: ContourId,
    next_nid: &mut u64,
) -> Result<GlyphContour, String> {
    let normalized = d_attr.replace(',', " ");
    let mut tokens = normalized.split_whitespace();
    let mut nodes = Vec::new();
    let mut closed = false;
    let mut push_node = |x: f64, y: f64, nt: NodeType| {
        let id = NodeId(*next_nid);
        *next_nid += 1;
        nodes.push(Node::new(id, x, y, nt));
    };
    while let Some(tok) = tokens.next() {
        match tok {
            "M" | "L" | "S" | "O" => {
                let (x, y) = (
                    next_valid_coord(&mut tokens)?,
                    next_valid_coord(&mut tokens)?,
                );
                let nt = match tok {
                    "S" => NodeType::Smooth,
                    "O" => NodeType::OffCurve,
                    _ => NodeType::Corner,
                };
                push_node(x, y, nt);
            }
            "C" | "CS" => {
                let mut coords = [0.0_f64; SVG_CUBIC_ARGS_LEN];
                for val in &mut coords {
                    *val = next_valid_coord(&mut tokens)?;
                }
                push_node(coords[0], coords[1], NodeType::OffCurve);
                push_node(coords[2], coords[3], NodeType::OffCurve);
                let end_nt = if tok == "CS" {
                    NodeType::Smooth
                } else {
                    NodeType::Corner
                };
                push_node(coords[4], coords[5], end_nt);
            }
            "Z" | "z" => closed = true,
            _ => return Err(format!("Unsupported legacy SVG path command: {tok}")),
        }
    }
    if nodes.len() < 2 {
        return Err("SVG path has fewer than 2 nodes".into());
    }
    let mut c = GlyphContour::new(contour_id, nodes);
    c.closed = closed;
    Ok(c)
}

pub(crate) fn try_parse_data_rgba(node: XmlNode<'_, '_>) -> Option<[u8; 4]> {
    let raw = node_attr_or_style(node, "data-rgba")?;
    let parts: Vec<u8> = raw
        .split(',')
        .filter_map(|s| s.trim().parse::<u8>().ok())
        .collect();
    if let [r, g, b, a] = parts.as_slice() {
        Some([*r, *g, *b, *a])
    } else {
        None
    }
}

pub(crate) fn parse_group_symbol_tag(node: XmlNode<'_, '_>) -> SymbolLayerTag {
    let layer_group = node_attr_or_style(node, "data-layer-group")
        .and_then(|s| s.parse::<u8>().ok())
        .unwrap_or(1)
        .clamp(1, RADIAL_SYMMETRY_MAX_FOLDS);
    let motion_anchor_id = node_attr_or_style(node, "data-motion-anchor")
        .and_then(|s| s.parse::<u64>().ok())
        .map(AnchorId);
    let custom_pivot_upm = match (
        node_attr_or_style(node, "data-pivot-x")
            .and_then(|s| s.parse::<f64>().ok())
            .and_then(valid_f64),
        node_attr_or_style(node, "data-pivot-y")
            .and_then(|s| s.parse::<f64>().ok())
            .and_then(valid_f64),
    ) {
        (Some(px), Some(py)) => Some(FontPoint::new(px, py)),
        _ => None,
    };
    let draw_order_attr = node_attr_or_style(node, "data-draw-order");
    let draw_direction =
        DrawDirection::from_svg_attr(node_attr_or_style(node, "data-draw-direction"));
    let knockout_margin_upm = node_attr_or_style(node, "data-knockout-margin")
        .and_then(|s| s.parse::<f64>().ok())
        .filter(|v| v.is_finite())
        .map_or(0.0, |v| v.clamp(0.0, KNOCKOUT_MARGIN_MAX_UPM));
    let phase_offset = node_attr_or_style(node, "data-phase-offset")
        .and_then(|s| s.parse::<f64>().ok())
        .filter(|v| v.is_finite())
        .map_or(0.0, |v| v.clamp(-1.0, 1.0));
    let motion_path_contour_id = node_attr_or_style(node, "data-motion-path")
        .and_then(|s| s.parse::<usize>().ok())
        .map(ContourId);
    let orient_along_motion_path =
        node_attr_or_style(node, "data-orient-motion-path") == Some("true");
    SymbolLayerTag {
        layer_group,
        motion_anchor_id,
        custom_pivot_upm,
        draw_order_reversed: draw_order_attr == Some("reversed"),
        variable_color_enabled: node_attr_or_style(node, "data-variable-color") != Some("false"),
        pulse_participates: node_attr_or_style(node, "data-pulse-participates") != Some("false"),
        hierarchy_level: SymbolHierarchyLevel::from_svg_attr(node_attr_or_style(
            node,
            "data-hierarchy",
        )),
        draw_direction,
        knockout_margin_upm,
        phase_offset,
        motion_path_contour_id,
        orient_along_motion_path,
    }
}

pub(crate) fn export_group_tag_attrs(
    out: &mut String,
    idx: usize,
    sub: &CanvasColorSublayer,
    tag: SymbolLayerTag,
) {
    let rgba = match &sub.paint {
        ColorLayerPaint::Solid { rgba } => *rgba,
        _ => SVG_DEFAULT_FILL_RGBA,
    };
    let draw_order = if tag.draw_order_reversed {
        "reversed"
    } else {
        "forward"
    };
    let ko = if tag.knockout_margin_upm.is_finite() {
        tag.knockout_margin_upm.clamp(0.0, KNOCKOUT_MARGIN_MAX_UPM)
    } else {
        0.0
    };
    let po = if tag.phase_offset.is_finite() {
        tag.phase_offset.clamp(-1.0, 1.0)
    } else {
        0.0
    };
    let _ = write!(
        out,
        "  <g id=\"{}\" data-layer-group=\"{}\" data-hierarchy=\"{}\" data-draw-direction=\"{}\" data-knockout-margin=\"{:.4}\" data-phase-offset=\"{:.4}\" data-variable-color=\"{}\" data-pulse-participates=\"{}\" data-draw-order=\"{}\" data-opacity=\"{:.4}\" data-rgba=\"{},{},{},{}\"",
        sub.name,
        tag.layer_group,
        tag.hierarchy_level.as_svg_attr(),
        tag.draw_direction.as_svg_attr(),
        ko,
        po,
        tag.variable_color_enabled,
        tag.pulse_participates,
        draw_order,
        sub.opacity.clamp(0.0, 1.0),
        rgba[0],
        rgba[1],
        rgba[2],
        rgba[3],
    );
    if let Some(mp_cid) = tag.motion_path_contour_id {
        let _ = write!(
            out,
            " data-motion-path=\"{}\" data-orient-motion-path=\"{}\"",
            mp_cid.0, tag.orient_along_motion_path
        );
    }
    if let Some(aid) = tag.motion_anchor_id {
        let _ = write!(out, " data-motion-anchor=\"{}\"", aid.0);
    }
    if let Some(piv) = tag.custom_pivot_upm {
        let _ = write!(
            out,
            " data-pivot-x=\"{:.4}\" data-pivot-y=\"{:.4}\"",
            piv.x.0, piv.y.0
        );
    }
    match &sub.paint {
        ColorLayerPaint::LinearGradient { .. } | ColorLayerPaint::RadialGradient { .. } => {
            let _ = writeln!(out, " fill=\"url(#grad_{idx})\">");
        }
        _ => {
            let _ = writeln!(
                out,
                " fill=\"#{:02x}{:02x}{:02x}\" fill-opacity=\"{:.4}\">",
                rgba[0],
                rgba[1],
                rgba[2],
                (f32::from(rgba[3]) / SVG_BYTE_MAX_F32) * sub.opacity.clamp(0.0, 1.0)
            );
        }
    }
}

/// Exports `sublayers` and `tags` to a W3C-compatible layered kinetic SVG string (`SPEC-016 §5.3`).
#[must_use]
pub fn export_kinetic_symbol_svg(
    sublayers: &[CanvasColorSublayer],
    tags: &[SymbolLayerTag],
) -> String {
    export_w3c_icon_svg(sublayers, tags, &BTreeMap::new())
}

/// Parses a layered kinetic SVG string back into `(Vec<CanvasColorSublayer>, Vec<SymbolLayerTag>)` (`SPEC-016 §5.3`).
pub fn import_kinetic_symbol_svg(
    svg_xml: &str,
) -> Result<(Vec<CanvasColorSublayer>, Vec<SymbolLayerTag>), String> {
    let doc = import_w3c_icon_svg(svg_xml)?;
    Ok((doc.sublayers, doc.tags))
}
