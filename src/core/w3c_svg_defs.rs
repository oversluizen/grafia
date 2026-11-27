//! W3C SVG `<defs>` gradient, color, attribute, CSS `style`, `viewBox`, `transform` matrix,
//! and export helpers backed by COTS `roxmltree` (`SPEC-017 §2.1 & §2.8b`).

use crate::core::color_layers::{
    CanvasColorSublayer, ColorLayerPaint, GradientSpread, GradientStop,
};
use crate::core::coordinates::FontPoint;
use crate::core::fit_and_stroke::{
    ContourStrokeStyle, LineCapStyle, LineJoinStyle, StrokePosition,
};
use crate::core::math_consts::{
    FIT_CURVE_MIN_CHORD_LEN_EPS, ICON_STUDIO_MAX_COORD_ABS_UPM, SVG_BLUE_COLOR_RGBA,
    SVG_BYTE_MAX_F32, SVG_DEFAULT_FILL_RGBA, SVG_GREEN_COLOR_RGBA, SVG_HEX_RGBA_LEN,
    SVG_HEX_RGB_LEN, SVG_HEX_SHORT_ALPHA_LEN, SVG_HEX_SHORT_LEN, SVG_MATRIX_ARGS_LEN,
    SVG_MAX_STROKE_WIDTH_UPM, SVG_PERCENT_SCALE, SVG_RADIX_HEX, SVG_RED_COLOR_RGBA,
    SVG_RGBA_ARGS_LEN, SVG_RGB_ARGS_LEN, SVG_VIEWBOX_ARGS_LEN, SVG_VIEWBOX_SIZE_UPM,
};
use kurbo::{Affine, Point};
use roxmltree::Node as XmlNode;
use std::collections::BTreeMap;
use std::fmt::Write as _;

/// Resolves an attribute value directly on `node` or inside its `style="key:val;..."` attribute.
pub(crate) fn node_attr_or_style<'a>(node: XmlNode<'a, '_>, key: &str) -> Option<&'a str> {
    if let Some(val) = node.attribute(key) {
        return Some(val.trim());
    }
    if let Some(style) = node.attribute("style") {
        for decl in style.split(';') {
            if let Some((k, v)) = decl.split_once(':') {
                if k.trim().eq_ignore_ascii_case(key) {
                    return Some(v.trim());
                }
            }
        }
    }
    None
}

/// Resolves an inherited SVG attribute/style walking up from `node` through ancestor `<g>` / `<svg>` elements.
pub(crate) fn inherited_attr_or_style<'a>(node: XmlNode<'a, '_>, key: &str) -> Option<&'a str> {
    let mut cur = Some(node);
    while let Some(n) = cur {
        if n.is_element() {
            if let Some(val) = node_attr_or_style(n, key) {
                return Some(val);
            }
        }
        cur = n.parent();
    }
    None
}

pub(crate) fn parse_node_f64(node: XmlNode<'_, '_>, key: &str, default: f64) -> f64 {
    node_attr_or_style(node, key)
        .and_then(parse_svg_number_or_percent)
        .filter(|v| v.is_finite() && v.abs() <= ICON_STUDIO_MAX_COORD_ABS_UPM)
        .unwrap_or(default)
}

pub(crate) fn parse_svg_number_or_percent(raw: &str) -> Option<f64> {
    let s = raw.trim().trim_end_matches("px");
    if let Some(pct) = s.strip_suffix('%') {
        pct.trim()
            .parse::<f64>()
            .ok()
            .map(|v| v * SVG_PERCENT_SCALE)
    } else {
        s.parse::<f64>().ok()
    }
}

fn parse_rgb_func_rgba(inner: &str, scale_a: impl Fn(u8) -> u8) -> Option<[u8; 4]> {
    let vals: Vec<f64> = inner
        .replace(',', " ")
        .split_whitespace()
        .filter_map(|t| {
            if let Some(p) = t.strip_suffix('%') {
                p.parse::<f64>()
                    .ok()
                    .map(|v| v * SVG_PERCENT_SCALE * f64::from(SVG_BYTE_MAX_F32))
            } else {
                t.parse::<f64>().ok()
            }
        })
        .collect();
    let to_u8 = |v: f64| v.round().clamp(0.0, f64::from(SVG_BYTE_MAX_F32)) as u8;
    match vals.as_slice() {
        [r, g, b] if vals.len() == SVG_RGB_ARGS_LEN => {
            Some([to_u8(*r), to_u8(*g), to_u8(*b), scale_a(u8::MAX)])
        }
        [r, g, b, a, ..] if vals.len() >= SVG_RGBA_ARGS_LEN => {
            let au8 = to_u8(if *a <= 1.0 {
                *a * f64::from(SVG_BYTE_MAX_F32)
            } else {
                *a
            });
            Some([to_u8(*r), to_u8(*g), to_u8(*b), scale_a(au8)])
        }
        _ => None,
    }
}

pub(crate) fn parse_svg_color_rgba(raw: &str, alpha_mul: f32) -> Option<[u8; 4]> {
    let s = raw.trim();
    if s.eq_ignore_ascii_case("none") || s.eq_ignore_ascii_case("transparent") {
        return None;
    }
    let scale_a = |a: u8| (f32::from(a) * alpha_mul.clamp(0.0, 1.0)).round() as u8;
    if let Some(hex) = s.strip_prefix('#') {
        let h = hex.trim();
        let pair = |i: usize| u8::from_str_radix(&h[i..i + 2], SVG_RADIX_HEX).ok();
        let nib = |i: usize| u8::from_str_radix(&h[i..=i].repeat(2), SVG_RADIX_HEX).ok();
        return match h.len() {
            SVG_HEX_SHORT_LEN => Some([nib(0)?, nib(1)?, nib(2)?, scale_a(u8::MAX)]),
            SVG_HEX_SHORT_ALPHA_LEN => Some([nib(0)?, nib(1)?, nib(2)?, scale_a(nib(3)?)]),
            SVG_HEX_RGB_LEN => Some([pair(0)?, pair(2)?, pair(4)?, scale_a(u8::MAX)]),
            SVG_HEX_RGBA_LEN => Some([pair(0)?, pair(2)?, pair(4)?, scale_a(pair(6)?)]),
            _ => None,
        };
    }
    let lower = s.to_ascii_lowercase();
    if let Some(inner) = lower
        .strip_prefix("rgba(")
        .or_else(|| lower.strip_prefix("rgb("))
        .and_then(|r| r.strip_suffix(')'))
    {
        return parse_rgb_func_rgba(inner, scale_a);
    }
    match lower.as_str() {
        "black" | "currentcolor" => Some([0, 0, 0, scale_a(u8::MAX)]),
        "white" => Some([u8::MAX, u8::MAX, u8::MAX, scale_a(u8::MAX)]),
        "red" => Some(SVG_RED_COLOR_RGBA),
        "green" => Some(SVG_GREEN_COLOR_RGBA),
        "blue" => Some(SVG_BLUE_COLOR_RGBA),
        _ => Some(SVG_DEFAULT_FILL_RGBA),
    }
}

/// Resolves the root `<svg viewBox="min_x min_y w h">` normalization transform (`(Affine, stroke_scale)`).
pub(crate) fn resolve_svg_viewbox_transform(svg_root: XmlNode<'_, '_>) -> (Affine, f64) {
    let target = f64::from(SVG_VIEWBOX_SIZE_UPM);
    if let Some(vb) = svg_root.attribute("viewBox") {
        let nums: Vec<f64> = vb
            .replace(',', " ")
            .split_whitespace()
            .filter_map(|s| s.parse::<f64>().ok())
            .filter(|v| v.is_finite() && v.abs() <= ICON_STUDIO_MAX_COORD_ABS_UPM)
            .collect();
        if nums.len() >= SVG_VIEWBOX_ARGS_LEN
            && nums[2] > FIT_CURVE_MIN_CHORD_LEN_EPS
            && nums[3] > FIT_CURVE_MIN_CHORD_LEN_EPS
        {
            let (min_x, min_y, w, h) = (nums[0], nums[1], nums[2], nums[3]);
            if (min_x.abs() < FIT_CURVE_MIN_CHORD_LEN_EPS)
                && (min_y.abs() < FIT_CURVE_MIN_CHORD_LEN_EPS)
                && ((w - target).abs() < FIT_CURVE_MIN_CHORD_LEN_EPS)
                && ((h - target).abs() < FIT_CURVE_MIN_CHORD_LEN_EPS)
            {
                return (Affine::IDENTITY, 1.0);
            }
            let (sx, sy) = (target / w, target / h);
            let s = sx.min(sy);
            return (Affine::scale(s) * Affine::translate((-min_x, -min_y)), s);
        }
    }
    (Affine::IDENTITY, 1.0)
}

fn parse_gradient_stops_node(grad_node: XmlNode<'_, '_>) -> Vec<GradientStop> {
    let mut stops = Vec::new();
    for child in grad_node.children().filter(XmlNode::is_element) {
        if !child.tag_name().name().eq_ignore_ascii_case("stop") {
            continue;
        }
        let offset = node_attr_or_style(child, "offset")
            .and_then(parse_svg_number_or_percent)
            .filter(|v| v.is_finite())
            .unwrap_or(0.0)
            .clamp(0.0, 1.0);
        let op = node_attr_or_style(child, "stop-opacity")
            .and_then(|s| s.parse::<f32>().ok())
            .filter(|v| v.is_finite())
            .unwrap_or(1.0);
        let col = node_attr_or_style(child, "stop-color")
            .and_then(|c| parse_svg_color_rgba(c, op))
            .unwrap_or(SVG_DEFAULT_FILL_RGBA);
        stops.push(GradientStop::new(offset, col));
    }
    if stops.len() < 2 {
        vec![
            GradientStop::new(0.0, SVG_DEFAULT_FILL_RGBA),
            GradientStop::new(1.0, [u8::MAX, u8::MAX, u8::MAX, u8::MAX]),
        ]
    } else {
        stops
    }
}

pub(crate) fn parse_svg_defs_gradients_dom(
    root: XmlNode<'_, '_>,
    vb_aff: Affine,
    vb_scale: f64,
) -> BTreeMap<String, ColorLayerPaint> {
    let mut map = BTreeMap::new();
    let max_upm = f64::from(SVG_VIEWBOX_SIZE_UPM);
    for node in root.descendants().filter(XmlNode::is_element) {
        let tag = node.tag_name().name();
        let Some(id) = node.attribute("id") else {
            continue;
        };
        if tag.eq_ignore_ascii_case("linearGradient") {
            let span = max_upm / vb_scale.max(FIT_CURVE_MIN_CHORD_LEN_EPS);
            let p0 = vb_aff
                * Point::new(
                    parse_node_f64(node, "x1", 0.0),
                    parse_node_f64(node, "y1", 0.0),
                );
            let p1 = vb_aff
                * Point::new(
                    parse_node_f64(node, "x2", span),
                    parse_node_f64(node, "y2", span),
                );
            map.insert(
                id.to_string(),
                ColorLayerPaint::LinearGradient {
                    p0: FontPoint::new(p0.x, p0.y),
                    p1: FontPoint::new(p1.x, p1.y),
                    stops: parse_gradient_stops_node(node),
                    spread: GradientSpread::Pad,
                },
            );
        } else if tag.eq_ignore_ascii_case("radialGradient") {
            let half = 0.5 * max_upm / vb_scale.max(FIT_CURVE_MIN_CHORD_LEN_EPS);
            let (cx, cy) = (
                parse_node_f64(node, "cx", half),
                parse_node_f64(node, "cy", half),
            );
            let r1 = (parse_node_f64(node, "r", half) * vb_scale).max(1.0);
            let (fx, fy) = (
                parse_node_f64(node, "fx", cx),
                parse_node_f64(node, "fy", cy),
            );
            let r0 = (parse_node_f64(node, "fr", 0.0) * vb_scale).max(0.0);
            let (c0, c1) = (vb_aff * Point::new(fx, fy), vb_aff * Point::new(cx, cy));
            map.insert(
                id.to_string(),
                ColorLayerPaint::RadialGradient {
                    c0: FontPoint::new(c0.x, c0.y),
                    r0,
                    c1: FontPoint::new(c1.x, c1.y),
                    r1,
                    stops: parse_gradient_stops_node(node),
                    spread: GradientSpread::Pad,
                },
            );
        }
    }
    map
}

pub(crate) fn parse_transform_str(raw_opt: Option<&str>) -> Affine {
    let Some(raw) = raw_opt else {
        return Affine::IDENTITY;
    };
    let mut aff = Affine::IDENTITY;
    let mut rest = raw.trim();
    while let Some(open) = rest.find('(') {
        let name = rest[..open].trim();
        let Some(close) = rest[open..].find(')') else {
            break;
        };
        let args_str = &rest[open + 1..open + close];
        rest = rest[open + close + 1..].trim_start_matches([',', ' ']);
        let nums: Vec<f64> = args_str
            .replace(',', " ")
            .split_whitespace()
            .filter_map(|s| s.parse::<f64>().ok())
            .filter(|v| v.is_finite() && v.abs() <= ICON_STUDIO_MAX_COORD_ABS_UPM)
            .collect();
        let next = match (name, nums.as_slice()) {
            ("translate", [tx]) => Affine::translate((*tx, 0.0)),
            ("translate", [tx, ty, ..]) => Affine::translate((*tx, *ty)),
            ("scale", [s]) => Affine::scale(*s),
            ("scale", [sx, sy, ..]) => Affine::scale_non_uniform(*sx, *sy),
            ("rotate", [deg]) => Affine::rotate(deg.to_radians()),
            ("rotate", [deg, cx, cy, ..]) => {
                Affine::translate((*cx, *cy))
                    * Affine::rotate(deg.to_radians())
                    * Affine::translate((-*cx, -*cy))
            }
            ("matrix", [a, b, c, d, e, f, ..]) if nums.len() >= SVG_MATRIX_ARGS_LEN => {
                Affine::new([*a, *b, *c, *d, *e, *f])
            }
            _ => Affine::IDENTITY,
        };
        aff *= next;
    }
    aff
}

pub(crate) fn resolve_node_paint_and_stroke(
    el_node: XmlNode<'_, '_>,
    defs: &BTreeMap<String, ColorLayerPaint>,
    stroke_scale: f64,
) -> (Option<ColorLayerPaint>, Option<ContourStrokeStyle>) {
    let fill_raw = inherited_attr_or_style(el_node, "fill");
    let paint = if let Some(f) = fill_raw {
        if let Some(id) = f
            .trim()
            .strip_prefix("url(#")
            .and_then(|s| s.strip_suffix(')'))
        {
            defs.get(id).cloned()
        } else {
            parse_svg_color_rgba(f, 1.0).map(|rgba| ColorLayerPaint::Solid { rgba })
        }
    } else {
        el_node
            .ancestors()
            .filter(|n| n.is_element())
            .find_map(crate::core::kinetic_svg_io::try_parse_data_rgba)
            .map(|rgba| ColorLayerPaint::Solid { rgba })
    };

    let stroke_raw = inherited_attr_or_style(el_node, "stroke");
    let stroke_col = stroke_raw.and_then(|s| parse_svg_color_rgba(s, 1.0));
    let sw = inherited_attr_or_style(el_node, "stroke-width")
        .and_then(parse_svg_number_or_percent)
        .map(|v| v * stroke_scale)
        .filter(|v| v.is_finite() && *v > 0.0 && *v <= SVG_MAX_STROKE_WIDTH_UPM);
    let stroke_style = if stroke_col.is_some() || sw.is_some() {
        let w = sw.unwrap_or(stroke_scale.clamp(1.0, SVG_MAX_STROKE_WIDTH_UPM));
        let cap = match inherited_attr_or_style(el_node, "stroke-linecap") {
            Some(s) if s.eq_ignore_ascii_case("round") => LineCapStyle::Round,
            Some(s) if s.eq_ignore_ascii_case("square") => LineCapStyle::Square,
            _ => LineCapStyle::Butt,
        };
        let join = match inherited_attr_or_style(el_node, "stroke-linejoin") {
            Some(s) if s.eq_ignore_ascii_case("round") => LineJoinStyle::Round,
            Some(s) if s.eq_ignore_ascii_case("bevel") => LineJoinStyle::Bevel,
            _ => LineJoinStyle::Miter,
        };
        Some(ContourStrokeStyle {
            width: w,
            height: w,
            position: StrokePosition::Center,
            cap_start: cap,
            cap_end: cap,
            join,
        })
    } else {
        None
    };
    let final_paint = paint.or_else(|| {
        stroke_col
            .map(|rgba| ColorLayerPaint::Solid { rgba })
            .or(Some(ColorLayerPaint::Solid {
                rgba: SVG_DEFAULT_FILL_RGBA,
            }))
    });
    (final_paint, stroke_style)
}

pub(crate) fn format_svg_defs_gradients(sublayers: &[CanvasColorSublayer]) -> String {
    let mut defs_str = String::new();
    let write_stops = |out: &mut String, stops: &[GradientStop]| {
        for st in stops {
            let [r, g, b, a] = st.rgba;
            let _ = writeln!(
                out,
                "      <stop offset=\"{:.4}\" stop-color=\"#{r:02x}{g:02x}{b:02x}\" stop-opacity=\"{:.4}\"/>",
                st.offset,
                f32::from(a) / SVG_BYTE_MAX_F32
            );
        }
    };
    for (idx, sub) in sublayers.iter().enumerate() {
        match &sub.paint {
            ColorLayerPaint::LinearGradient { p0, p1, stops, .. } => {
                let _ = writeln!(
                    defs_str,
                    "    <linearGradient id=\"grad_{idx}\" x1=\"{:.4}\" y1=\"{:.4}\" x2=\"{:.4}\" y2=\"{:.4}\">",
                    p0.x.0, p0.y.0, p1.x.0, p1.y.0
                );
                write_stops(&mut defs_str, stops);
                defs_str.push_str("    </linearGradient>\n");
            }
            ColorLayerPaint::RadialGradient {
                c0,
                r0,
                c1,
                r1,
                stops,
                ..
            } => {
                let _ = writeln!(
                    defs_str,
                    "    <radialGradient id=\"grad_{idx}\" cx=\"{:.4}\" cy=\"{:.4}\" r=\"{:.4}\" fx=\"{:.4}\" fy=\"{:.4}\" fr=\"{:.4}\">",
                    c1.x.0, c1.y.0, r1, c0.x.0, c0.y.0, r0
                );
                write_stops(&mut defs_str, stops);
                defs_str.push_str("    </radialGradient>\n");
            }
            _ => {}
        }
    }
    defs_str
}
