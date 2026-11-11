//! `SPEC-008 §2.5`: Live Contour Stroke offset curve expansion (`Miter`, `Round`, `Bevel` joins
//! and `Butt`, `Round`, `Square` caps) for open and closed spine contours.

use crate::core::bezier_ops::{Node, NodeId, NodeType};
use crate::core::coordinates::FontPoint;
use crate::core::curve_manipulation::{extract_segments, ContourSegment};
use crate::core::fit_and_stroke::{
    ContourStrokeStyle, LineCapStyle, LineJoinStyle, StrokePosition,
};
use crate::core::glyph_model::{ContourId, GlyphContour};
use crate::core::segment_ops::reverse_contour_nodes;
use crate::tokens::{
    FIT_CURVE_MIN_CHORD_LEN_EPS, STROKE_JOIN_COINCIDENT_EPS_UPM, STROKE_MITER_LIMIT,
    STROKE_ROUND_KAPPA,
};
use std::collections::BTreeMap;

/// Expands all stroked contours in `contours` according to `strokes` (`SPEC-008 §2.5.2`).
#[must_use]
pub fn expand_stroked_contours_preview(
    contours: &[GlyphContour],
    strokes: &BTreeMap<ContourId, ContourStrokeStyle>,
) -> Vec<GlyphContour> {
    let mut out = Vec::new();
    let mut next_cid = 20_000usize;
    let mut next_nid = 20_000u64;
    for contour in contours {
        if let Some(style) = strokes.get(&contour.id) {
            let expanded =
                expand_single_contour_stroke(contour, *style, &mut next_cid, &mut next_nid);
            out.extend(expanded);
        }
    }
    out
}

/// Expands a single `GlyphContour` with `style` into 1 closed contour (when `contour.closed == false`)
/// or 2 concentric opposite-winding closed contours (when `contour.closed == true`).
pub fn expand_single_contour_stroke(
    contour: &GlyphContour,
    style: ContourStrokeStyle,
    next_cid: &mut usize,
    next_nid: &mut u64,
) -> Vec<GlyphContour> {
    if style.width <= 0.0 || style.height <= 0.0 {
        return Vec::new();
    }
    let segments = extract_segments(&contour.nodes, contour.closed);
    if segments.is_empty() {
        return Vec::new();
    }
    let (left_factor, right_factor) = match style.position {
        StrokePosition::Center => (0.5, -0.5),
        StrokePosition::InsideLeft => (1.0, 0.0),
        StrokePosition::OutsideRight => (0.0, -1.0),
    };
    let left_nodes = build_offset_rail_nodes(
        &contour.nodes,
        &segments,
        contour.closed,
        style.width * left_factor,
        style.height * left_factor,
        style.join,
        next_nid,
    );
    let right_nodes = build_offset_rail_nodes(
        &contour.nodes,
        &segments,
        contour.closed,
        style.width * right_factor,
        style.height * right_factor,
        style.join,
        next_nid,
    );

    if contour.closed {
        let mut inner_nodes = right_nodes;
        reverse_contour_nodes(&mut inner_nodes, true);
        let c_outer = GlyphContour::new(ContourId(*next_cid), left_nodes);
        *next_cid += 1;
        let c_inner = GlyphContour::new(ContourId(*next_cid), inner_nodes);
        *next_cid += 1;
        vec![c_outer, c_inner]
    } else {
        let t_start = segment_start_tangent(&contour.nodes, &segments[0]);
        let t_end =
            segment_end_tangent(&contour.nodes, segments.last().expect("non-empty segments"));
        let nodes =
            assemble_open_stroke_contour(left_nodes, right_nodes, t_start, t_end, style, next_nid);
        let c = GlyphContour::new(ContourId(*next_cid), nodes);
        *next_cid += 1;
        vec![c]
    }
}

fn unit_tangent(a: FontPoint, b: FontPoint) -> (f64, f64) {
    let dx = b.x.0 - a.x.0;
    let dy = b.y.0 - a.y.0;
    let len = (dx * dx + dy * dy).sqrt();
    if len <= FIT_CURVE_MIN_CHORD_LEN_EPS {
        (1.0, 0.0)
    } else {
        (dx / len, dy / len)
    }
}

fn segment_start_tangent(nodes: &[Node], seg: &ContourSegment) -> (f64, f64) {
    match *seg {
        ContourSegment::Line {
            start_idx, end_idx, ..
        } => unit_tangent(nodes[start_idx].point, nodes[end_idx].point),
        ContourSegment::Cubic {
            start_idx,
            h1_idx,
            end_idx,
            ..
        } => {
            let p0 = nodes[start_idx].point;
            let p1 = nodes[h1_idx].point;
            if p0.distance_to(p1) > FIT_CURVE_MIN_CHORD_LEN_EPS {
                unit_tangent(p0, p1)
            } else {
                unit_tangent(p0, nodes[end_idx].point)
            }
        }
    }
}

fn segment_end_tangent(nodes: &[Node], seg: &ContourSegment) -> (f64, f64) {
    match *seg {
        ContourSegment::Line {
            start_idx, end_idx, ..
        } => unit_tangent(nodes[start_idx].point, nodes[end_idx].point),
        ContourSegment::Cubic {
            start_idx,
            h2_idx,
            end_idx,
            ..
        } => {
            let p2 = nodes[h2_idx].point;
            let p3 = nodes[end_idx].point;
            if p2.distance_to(p3) > FIT_CURVE_MIN_CHORD_LEN_EPS {
                unit_tangent(p2, p3)
            } else {
                unit_tangent(nodes[start_idx].point, p3)
            }
        }
    }
}

fn push_node(out: &mut Vec<Node>, next_nid: &mut u64, pt: FontPoint, node_type: NodeType) {
    let id = NodeId(*next_nid);
    *next_nid += 1;
    out.push(Node::new(id, pt.x.0, pt.y.0, node_type));
}

#[allow(clippy::too_many_arguments)]
fn emit_corner_join(
    out: &mut Vec<Node>,
    next_nid: &mut u64,
    vertex: FontPoint,
    t1: (f64, f64),
    t2: (f64, f64),
    offset_x: f64,
    offset_y: f64,
    join: LineJoinStyle,
) {
    let n1 = (-t1.1, t1.0);
    let n2 = (-t2.1, t2.0);
    let q1 = FontPoint::new(vertex.x.0 + n1.0 * offset_x, vertex.y.0 + n1.1 * offset_y);
    let q2 = FontPoint::new(vertex.x.0 + n2.0 * offset_x, vertex.y.0 + n2.1 * offset_y);
    if q1.distance_to(q2) <= STROKE_JOIN_COINCIDENT_EPS_UPM {
        push_node(out, next_nid, q1, NodeType::Smooth);
        return;
    }
    let bx = n1.0 + n2.0;
    let by = n1.1 + n2.1;
    let blen = (bx * bx + by * by).sqrt();
    let cross = t1.0 * t2.1 - t1.1 * t2.0;
    let is_inner_corner = cross * (offset_x + offset_y) > 0.0;
    if is_inner_corner || join == LineJoinStyle::Miter {
        if blen > FIT_CURVE_MIN_CHORD_LEN_EPS {
            let (ux, uy) = (bx / blen, by / blen);
            let cos_half = ux * n1.0 + uy * n1.1;
            if cos_half > 0.0 && (1.0 / cos_half) <= STROKE_MITER_LIMIT {
                let scale = 1.0 / cos_half;
                let m = FontPoint::new(
                    vertex.x.0 + ux * scale * offset_x,
                    vertex.y.0 + uy * scale * offset_y,
                );
                push_node(out, next_nid, m, NodeType::Corner);
                return;
            }
        }
        push_node(out, next_nid, q1, NodeType::Corner);
        push_node(out, next_nid, q2, NodeType::Corner);
        return;
    }
    match join {
        LineJoinStyle::Miter => unreachable!(),
        LineJoinStyle::Bevel => {
            push_node(out, next_nid, q1, NodeType::Corner);
            push_node(out, next_nid, q2, NodeType::Corner);
        }
        LineJoinStyle::Round => {
            let chord = q1.distance_to(q2);
            let h_len = STROKE_ROUND_KAPPA * chord * (0.5_f64).sqrt();
            let h1 = FontPoint::new(q1.x.0 + t1.0 * h_len, q1.y.0 + t1.1 * h_len);
            let h2 = FontPoint::new(q2.x.0 - t2.0 * h_len, q2.y.0 - t2.1 * h_len);
            push_node(out, next_nid, q1, NodeType::Smooth);
            push_node(out, next_nid, h1, NodeType::OffCurve);
            push_node(out, next_nid, h2, NodeType::OffCurve);
            push_node(out, next_nid, q2, NodeType::Smooth);
        }
    }
}

fn build_offset_rail_nodes(
    nodes: &[Node],
    segments: &[ContourSegment],
    closed: bool,
    offset_x: f64,
    offset_y: f64,
    join: LineJoinStyle,
    next_nid: &mut u64,
) -> Vec<Node> {
    let mut out = Vec::new();
    let seg_count = segments.len();
    for (si, seg) in segments.iter().enumerate() {
        let start_idx = match *seg {
            ContourSegment::Line { start_idx, .. } | ContourSegment::Cubic { start_idx, .. } => {
                start_idx
            }
        };
        let t_out = segment_start_tangent(nodes, seg);
        if !closed && si == 0 {
            let n = (-t_out.1, t_out.0);
            let q = FontPoint::new(
                nodes[start_idx].point.x.0 + n.0 * offset_x,
                nodes[start_idx].point.y.0 + n.1 * offset_y,
            );
            push_node(&mut out, next_nid, q, NodeType::Corner);
        } else {
            let prev_seg = if si == 0 {
                &segments[seg_count - 1]
            } else {
                &segments[si - 1]
            };
            let t_in = segment_end_tangent(nodes, prev_seg);
            emit_corner_join(
                &mut out,
                next_nid,
                nodes[start_idx].point,
                t_in,
                t_out,
                offset_x,
                offset_y,
                join,
            );
        }
        if let ContourSegment::Cubic { h1_idx, h2_idx, .. } = *seg {
            let t_in_end = segment_end_tangent(nodes, seg);
            let n_start = (-t_out.1, t_out.0);
            let n_end = (-t_in_end.1, t_in_end.0);
            let h1 = FontPoint::new(
                nodes[h1_idx].point.x.0 + n_start.0 * offset_x,
                nodes[h1_idx].point.y.0 + n_start.1 * offset_y,
            );
            let h2 = FontPoint::new(
                nodes[h2_idx].point.x.0 + n_end.0 * offset_x,
                nodes[h2_idx].point.y.0 + n_end.1 * offset_y,
            );
            push_node(&mut out, next_nid, h1, NodeType::OffCurve);
            push_node(&mut out, next_nid, h2, NodeType::OffCurve);
        }
    }
    if !closed {
        let last_seg = &segments[seg_count - 1];
        let end_idx = match *last_seg {
            ContourSegment::Line { end_idx, .. } | ContourSegment::Cubic { end_idx, .. } => end_idx,
        };
        let t_end = segment_end_tangent(nodes, last_seg);
        let n_end = (-t_end.1, t_end.0);
        let q_end = FontPoint::new(
            nodes[end_idx].point.x.0 + n_end.0 * offset_x,
            nodes[end_idx].point.y.0 + n_end.1 * offset_y,
        );
        push_node(&mut out, next_nid, q_end, NodeType::Corner);
    }
    out
}

fn assemble_open_stroke_contour(
    mut left_nodes: Vec<Node>,
    mut right_nodes: Vec<Node>,
    t_start: (f64, f64),
    t_end: (f64, f64),
    style: ContourStrokeStyle,
    next_nid: &mut u64,
) -> Vec<Node> {
    reverse_contour_nodes(&mut right_nodes, false);
    if style.cap_start == LineCapStyle::Square {
        let (dx, dy) = (
            -0.5 * style.width * t_start.0,
            -0.5 * style.height * t_start.1,
        );
        if let Some(first_l) = left_nodes.first_mut() {
            first_l.point = FontPoint::new(first_l.point.x.0 + dx, first_l.point.y.0 + dy);
        }
        if let Some(last_r) = right_nodes.last_mut() {
            last_r.point = FontPoint::new(last_r.point.x.0 + dx, last_r.point.y.0 + dy);
        }
    }
    if style.cap_end == LineCapStyle::Square {
        let (dx, dy) = (0.5 * style.width * t_end.0, 0.5 * style.height * t_end.1);
        if let Some(last_l) = left_nodes.last_mut() {
            last_l.point = FontPoint::new(last_l.point.x.0 + dx, last_l.point.y.0 + dy);
        }
        if let Some(first_r) = right_nodes.first_mut() {
            first_r.point = FontPoint::new(first_r.point.x.0 + dx, first_r.point.y.0 + dy);
        }
    }
    let mut out = left_nodes;
    if style.cap_end == LineCapStyle::Round {
        if let (Some(l_end), Some(r_start)) = (
            out.last().map(|n| n.point),
            right_nodes.first().map(|n| n.point),
        ) {
            append_round_cap_cubic(
                &mut out,
                next_nid,
                l_end,
                r_start,
                t_end,
                style.width,
                style.height,
            );
        }
    }
    out.extend(right_nodes);
    if style.cap_start == LineCapStyle::Round {
        if let (Some(r_end), Some(l_start)) =
            (out.last().map(|n| n.point), out.first().map(|n| n.point))
        {
            append_round_cap_cubic(
                &mut out,
                next_nid,
                r_end,
                l_start,
                (-t_start.0, -t_start.1),
                style.width,
                style.height,
            );
        }
    }
    out
}

fn append_round_cap_cubic(
    out: &mut Vec<Node>,
    next_nid: &mut u64,
    from_pt: FontPoint,
    to_pt: FontPoint,
    dir: (f64, f64),
    width: f64,
    height: f64,
) {
    let mid = FontPoint::new(
        0.5 * (from_pt.x.0 + to_pt.x.0),
        0.5 * (from_pt.y.0 + to_pt.y.0),
    );
    let rx = 0.5 * width;
    let ry = 0.5 * height;
    let apex = FontPoint::new(mid.x.0 + rx * dir.0, mid.y.0 + ry * dir.1);
    let h1 = FontPoint::new(
        from_pt.x.0 + STROKE_ROUND_KAPPA * rx * dir.0,
        from_pt.y.0 + STROKE_ROUND_KAPPA * ry * dir.1,
    );
    let h2 = FontPoint::new(
        apex.x.0 + STROKE_ROUND_KAPPA * (from_pt.x.0 - mid.x.0),
        apex.y.0 + STROKE_ROUND_KAPPA * (from_pt.y.0 - mid.y.0),
    );
    let h3 = FontPoint::new(
        apex.x.0 + STROKE_ROUND_KAPPA * (to_pt.x.0 - mid.x.0),
        apex.y.0 + STROKE_ROUND_KAPPA * (to_pt.y.0 - mid.y.0),
    );
    let h4 = FontPoint::new(
        to_pt.x.0 + STROKE_ROUND_KAPPA * rx * dir.0,
        to_pt.y.0 + STROKE_ROUND_KAPPA * ry * dir.1,
    );
    push_node(out, next_nid, h1, NodeType::OffCurve);
    push_node(out, next_nid, h2, NodeType::OffCurve);
    push_node(out, next_nid, apex, NodeType::Smooth);
    push_node(out, next_nid, h3, NodeType::OffCurve);
    push_node(out, next_nid, h4, NodeType::OffCurve);
}
