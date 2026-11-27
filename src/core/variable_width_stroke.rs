//! Variable-Width Skeleton Stroke (*Width Tool* — `SPEC-016 §3.2` & `§2.4.3`).

use crate::core::bezier_ops::{Node, NodeId, NodeType};
use crate::core::boolean_ops::{contour_signed_area, remove_overlap_in_contours};
use crate::core::coordinates::FontPoint;
use crate::core::curve_manipulation::{extract_segments, ContourSegment};
use crate::core::fit_and_stroke::LineCapStyle;
use crate::core::glyph_model::{ContourId, GlyphContour};
use crate::core::icon_studio::{VariableWidthStrokeSpec, WidthNodeSample};
use crate::core::math_consts::{
    BOOL_CUBIC_SUBDIV_STEPS, FIT_CURVE_MIN_CHORD_LEN_EPS, GREEN_CENTROID_THREE,
    ICON_STUDIO_MAX_COORD_ABS_UPM, KINETIC_SMOOTHSTEP_C2, STROKE_ROUND_KAPPA,
    VARIABLE_WIDTH_MAX_UPM,
};
use crate::core::reverse_contour_nodes;
use crate::core::stem_gauge_and_balance::compute_contour_area_and_centroid;
use kurbo::{ParamCurve, ParamCurveDeriv};

/// Visual and hit-testable width-handle crossbar on a variable-width skeleton contour (`SPEC-016 §3.2`).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct VariableWidthHandlePin {
    pub contour_id: ContourId,
    pub sample_index: usize,
    pub arc_ratio_t: f64,
    pub spine_pt: FontPoint,
    pub left_pt: FontPoint,
    pub right_pt: FontPoint,
}

fn sanitize_width_samples(raw: &[WidthNodeSample]) -> Vec<WidthNodeSample> {
    let mut clean: Vec<WidthNodeSample> = Vec::new();
    for s in raw {
        if s.arc_ratio_t.is_finite()
            && s.width_left_upm.is_finite()
            && s.width_right_upm.is_finite()
        {
            clean.push(WidthNodeSample {
                arc_ratio_t: s.arc_ratio_t.clamp(0.0, 1.0),
                width_left_upm: s.width_left_upm.clamp(0.0, VARIABLE_WIDTH_MAX_UPM),
                width_right_upm: s.width_right_upm.clamp(0.0, VARIABLE_WIDTH_MAX_UPM),
            });
        }
    }
    clean.sort_by(|a, b| a.arc_ratio_t.total_cmp(&b.arc_ratio_t));
    let mut dedup: Vec<WidthNodeSample> = Vec::with_capacity(clean.len());
    for s in clean {
        if let Some(last) = dedup.last_mut() {
            if (s.arc_ratio_t - last.arc_ratio_t).abs() <= FIT_CURVE_MIN_CHORD_LEN_EPS {
                *last = s;
                continue;
            }
        }
        dedup.push(s);
    }
    dedup
}

fn eval_fritsch_carlson_1d(ts: &[f64], ys: &[f64], t: f64) -> f64 {
    let n = ts.len();
    if n == 0 {
        return 0.0;
    }
    if n == 1 || t <= ts[0] {
        return ys[0];
    }
    if t >= ts[n - 1] {
        return ys[n - 1];
    }
    let mut secants = Vec::with_capacity(n - 1);
    for k in 0..(n - 1) {
        secants.push((ys[k + 1] - ys[k]) / (ts[k + 1] - ts[k]).max(FIT_CURVE_MIN_CHORD_LEN_EPS));
    }
    let mut tangents = vec![0.0_f64; n];
    tangents[0] = secants[0];
    tangents[n - 1] = secants[n - 2];
    for k in 1..(n - 1) {
        if secants[k - 1] * secants[k] > 0.0 {
            tangents[k] = 0.5 * (secants[k - 1] + secants[k]);
        }
    }
    let r_lim = GREEN_CENTROID_THREE * GREEN_CENTROID_THREE;
    for (k, &dk) in secants.iter().enumerate() {
        if dk.abs() <= FIT_CURVE_MIN_CHORD_LEN_EPS {
            tangents[k] = 0.0;
            tangents[k + 1] = 0.0;
        } else {
            let (a, b) = (tangents[k] / dk, tangents[k + 1] / dk);
            let m2 = a * a + b * b;
            if m2 > r_lim {
                let tau = GREEN_CENTROID_THREE / m2.sqrt();
                tangents[k] = tau * a * dk;
                tangents[k + 1] = tau * b * dk;
            }
        }
    }
    let mut seg = 0;
    for k in 0..(n - 1) {
        if t >= ts[k] && t <= ts[k + 1] {
            seg = k;
            break;
        }
    }
    let h = (ts[seg + 1] - ts[seg]).max(FIT_CURVE_MIN_CHORD_LEN_EPS);
    let u = ((t - ts[seg]) / h).clamp(0.0, 1.0);
    let (u2, u3, y0, y1) = (u * u, u * u * u, ys[seg], ys[seg + 1]);
    let val = (2.0 * u3 - KINETIC_SMOOTHSTEP_C2 * u2 + 1.0) * y0
        + (u3 - 2.0 * u2 + u) * h * tangents[seg]
        + (-2.0 * u3 + KINETIC_SMOOTHSTEP_C2 * u2) * y1
        + (u3 - u2) * h * tangents[seg + 1];
    val.clamp(y0.min(y1), y0.max(y1))
}

/// Evaluates `(width_left_upm, width_right_upm)` at normalized arc-length parameter `t` in `[0.0, 1.0]`
/// using Fritsch-Carlson monotone cubic Hermite interpolation (`SPEC-016 §3.2`).
#[must_use]
pub fn evaluate_variable_width_at_t(spec: &VariableWidthStrokeSpec, t: f64) -> (f64, f64) {
    let tc = if t.is_finite() { t } else { 0.0 }.clamp(0.0, 1.0);
    let clean = sanitize_width_samples(&spec.samples);
    if clean.is_empty() {
        return (0.0, 0.0);
    }
    let ts: Vec<f64> = clean.iter().map(|s| s.arc_ratio_t).collect();
    let ls: Vec<f64> = clean.iter().map(|s| s.width_left_upm).collect();
    let rs: Vec<f64> = clean.iter().map(|s| s.width_right_upm).collect();
    let l = eval_fritsch_carlson_1d(&ts, &ls, tc);
    let r = eval_fritsch_carlson_1d(&ts, &rs, tc);
    (l, r)
}

#[derive(Clone, Copy)]
struct SpineSample {
    pt: FontPoint,
    tangent: (f64, f64),
    arc_len: f64,
}

fn unit_vec(dx: f64, dy: f64, fb: (f64, f64)) -> (f64, f64) {
    let l = dx.hypot(dy);
    if l <= FIT_CURVE_MIN_CHORD_LEN_EPS {
        fb
    } else {
        (dx / l, dy / l)
    }
}

fn sample_contour_spine(contour: &GlyphContour) -> Vec<SpineSample> {
    let segs = extract_segments(&contour.nodes, contour.closed);
    let mut samples: Vec<SpineSample> = Vec::new();
    let (mut cum, mut tan) = (0.0_f64, (1.0_f64, 0.0_f64));
    let mut push = |x: f64, y: f64, t: (f64, f64)| {
        let pt = FontPoint::new(x, y);
        if let Some(prev) = samples.last() {
            cum += prev.pt.distance_to(pt);
        }
        samples.push(SpineSample {
            pt,
            tangent: t,
            arc_len: cum,
        });
    };
    for (si, seg) in segs.iter().enumerate() {
        match *seg {
            ContourSegment::Line { line, .. } => {
                tan = unit_vec(line.p1.x - line.p0.x, line.p1.y - line.p0.y, tan);
                if si == 0 {
                    push(line.p0.x, line.p0.y, tan);
                }
                push(line.p1.x, line.p1.y, tan);
            }
            ContourSegment::Cubic { cubic, .. } => {
                let deriv = cubic.deriv();
                for step in usize::from(si > 0)..=BOOL_CUBIC_SUBDIV_STEPS {
                    let u = (step as f64) / (BOOL_CUBIC_SUBDIV_STEPS as f64);
                    let (p, d) = (cubic.eval(u), deriv.eval(u));
                    tan = unit_vec(d.x, d.y, tan);
                    push(p.x, p.y, tan);
                }
            }
        }
    }
    samples
}

fn push_node(out: &mut Vec<Node>, next_nid: &mut u64, x: f64, y: f64, nt: NodeType) {
    out.push(Node::new(NodeId(*next_nid), x, y, nt));
    *next_nid += 1;
}

fn append_round_cap(out: &mut Vec<Node>, nid: &mut u64, a: FontPoint, b: FontPoint, d: (f64, f64)) {
    let (mx, my, r) = (
        0.5 * (a.x.0 + b.x.0),
        0.5 * (a.y.0 + b.y.0),
        0.5 * a.distance_to(b),
    );
    if r > FIT_CURVE_MIN_CHORD_LEN_EPS {
        let (k, ax, ay, oc, sm) = (
            STROKE_ROUND_KAPPA,
            mx + r * d.0,
            my + r * d.1,
            NodeType::OffCurve,
            NodeType::Smooth,
        );
        push_node(out, nid, a.x.0 + k * r * d.0, a.y.0 + k * r * d.1, oc);
        push_node(out, nid, ax + k * (a.x.0 - mx), ay + k * (a.y.0 - my), oc);
        push_node(out, nid, ax, ay, sm);
        push_node(out, nid, ax + k * (b.x.0 - mx), ay + k * (b.y.0 - my), oc);
        push_node(out, nid, b.x.0 + k * r * d.0, b.y.0 + k * r * d.1, oc);
        push_node(out, nid, b.x.0, b.y.0, sm);
    }
}

fn assemble_open_variable_outline(
    mut left: Vec<Node>,
    mut right: Vec<Node>,
    t0: (f64, f64),
    t1: (f64, f64),
    cap: LineCapStyle,
    nid: &mut u64,
) -> Vec<Node> {
    reverse_contour_nodes(&mut right, false);
    if !left.is_empty() && !right.is_empty() {
        let (ll, rl) = (left.len() - 1, right.len() - 1);
        if cap == LineCapStyle::Square {
            let e0 = 0.5 * left[0].point.distance_to(right[rl].point);
            let e1 = 0.5 * left[ll].point.distance_to(right[0].point);
            left[0].point.x.0 -= e0 * t0.0;
            left[0].point.y.0 -= e0 * t0.1;
            right[rl].point.x.0 -= e0 * t0.0;
            right[rl].point.y.0 -= e0 * t0.1;
            left[ll].point.x.0 += e1 * t1.0;
            left[ll].point.y.0 += e1 * t1.1;
            right[0].point.x.0 += e1 * t1.0;
            right[0].point.y.0 += e1 * t1.1;
        } else if cap == LineCapStyle::Round {
            let (le, rs) = (left[ll].point, right[0].point);
            append_round_cap(&mut left, nid, le, rs, t1);
        }
    }
    left.extend(right);
    if cap == LineCapStyle::Round && left.len() >= 2 {
        let (re, ls) = (left[left.len() - 1].point, left[0].point);
        append_round_cap(&mut left, nid, re, ls, (-t0.0, -t0.1));
    }
    left
}

fn interpolate_spine_at_ratio(spine: &[SpineSample], ratio: f64) -> (FontPoint, (f64, f64)) {
    let total = spine.last().map_or(0.0, |s| s.arc_len);
    if spine.is_empty() {
        return (FontPoint::new(0.0, 0.0), (1.0, 0.0));
    }
    let target = ratio.clamp(0.0, 1.0) * total;
    for w in spine.windows(2) {
        if target >= w[0].arc_len && target <= w[1].arc_len {
            let ds = (w[1].arc_len - w[0].arc_len).max(FIT_CURVE_MIN_CHORD_LEN_EPS);
            let u = ((target - w[0].arc_len) / ds).clamp(0.0, 1.0);
            let (p0, p1, t0, t1) = (w[0].pt, w[1].pt, w[0].tangent, w[1].tangent);
            let pt = FontPoint::new(
                p0.x.0 + u * (p1.x.0 - p0.x.0),
                p0.y.0 + u * (p1.y.0 - p0.y.0),
            );
            let tan = unit_vec(t0.0 + u * (t1.0 - t0.0), t0.1 + u * (t1.1 - t0.1), t0);
            return (pt, tan);
        }
    }
    let last = spine[spine.len() - 1];
    (last.pt, last.tangent)
}

/// Computes visual width-handle crossbar pins for all active `VariableWidthStrokeSpec`s (`SPEC-016 §3.2`).
#[must_use]
pub fn compute_variable_width_handle_pins(
    contours: &[GlyphContour],
    specs: &[VariableWidthStrokeSpec],
) -> Vec<VariableWidthHandlePin> {
    let mut pins = Vec::new();
    for spec in specs {
        for gc in contours {
            if gc.id != spec.contour_id || !gc.visible {
                continue;
            }
            let spine = sample_contour_spine(gc);
            if spine.is_empty() {
                continue;
            }
            for (sample_index, s) in sanitize_width_samples(&spec.samples).iter().enumerate() {
                let (p, tan) = interpolate_spine_at_ratio(&spine, s.arc_ratio_t);
                let (nx, ny, wl, wr) = (-tan.1, tan.0, s.width_left_upm, s.width_right_upm);
                let left_pt = FontPoint::new(p.x.0 + wl * nx, p.y.0 + wl * ny);
                let right_pt = FontPoint::new(p.x.0 - wr * nx, p.y.0 - wr * ny);
                pins.push(VariableWidthHandlePin {
                    contour_id: gc.id,
                    sample_index,
                    arc_ratio_t: s.arc_ratio_t,
                    spine_pt: p,
                    left_pt,
                    right_pt,
                });
            }
        }
    }
    pins
}

/// Hit-tests `pins` at `pointer_upm` within `hit_radius_upm`, returning `(contour_id, sample_index, is_left)` (`SPEC-016 §3.2`).
#[must_use]
pub fn hit_test_variable_width_handle_pin(
    pins: &[VariableWidthHandlePin],
    pt: FontPoint,
    r: f64,
) -> Option<(ContourId, usize, bool)> {
    if !pt.x.0.is_finite() || !pt.y.0.is_finite() || !r.is_finite() || r <= 0.0 {
        return None;
    }
    let mut best: Option<(f64, ContourId, usize, bool)> = None;
    for p in pins {
        for (h, is_l) in [(p.left_pt, true), (p.right_pt, false)] {
            let d = pt.distance_to(h);
            if d <= r && best.is_none_or(|b| d < b.0) {
                best = Some((d, p.contour_id, p.sample_index, is_l));
            }
        }
    }
    best.map(|(_, cid, idx, is_l)| (cid, idx, is_l))
}

fn build_closed_rings(left: Vec<Node>, right: Vec<Node>, next_nid: &mut u64) -> Vec<GlyphContour> {
    let mut clean_ring = |mut nodes: Vec<Node>| -> Vec<Node> {
        if nodes.len() > 2 {
            nodes.pop();
        }
        let mut raw = vec![nodes];
        let _ = remove_overlap_in_contours(&mut raw, &mut vec![true], &[], next_nid);
        raw.into_iter().next().unwrap_or_default()
    };
    let (cl, cr) = (clean_ring(left), clean_ring(right));
    let al = compute_contour_area_and_centroid(&cl).0.abs();
    let ar = compute_contour_area_and_centroid(&cr).0.abs();
    let (mut outer, mut inner) = if al >= ar { (cl, cr) } else { (cr, cl) };
    if contour_signed_area(&outer) < 0.0 {
        reverse_contour_nodes(&mut outer, true);
    }
    if contour_signed_area(&inner) > 0.0 {
        reverse_contour_nodes(&mut inner, true);
    }
    let mut res = Vec::new();
    for (i, n) in [outer, inner].into_iter().enumerate() {
        let a = compute_contour_area_and_centroid(&n).0;
        if a.is_finite() && a.abs() > FIT_CURVE_MIN_CHORD_LEN_EPS {
            res.push(GlyphContour::new(ContourId(i), n));
        }
    }
    res
}

/// Expands a skeleton `contour` using `spec` into closed, self-intersection-free outlines (`SPEC-016 §3.2`).
pub fn expand_variable_width_contour(
    contour: &GlyphContour,
    spec: &VariableWidthStrokeSpec,
    nid: &mut u64,
) -> Vec<GlyphContour> {
    let lim = ICON_STUDIO_MAX_COORD_ABS_UPM;
    for n in &contour.nodes {
        let (x, y) = (n.point.x.0, n.point.y.0);
        if !x.is_finite() || !y.is_finite() || x.abs() > lim || y.abs() > lim {
            return Vec::new();
        }
    }
    let mut clean = sanitize_width_samples(&spec.samples);
    let eps = FIT_CURVE_MIN_CHORD_LEN_EPS;
    if contour.closed && !clean.is_empty() && (1.0 - clean[clean.len() - 1].arc_ratio_t).abs() > eps
    {
        clean.push(WidthNodeSample {
            arc_ratio_t: 1.0,
            ..clean[0]
        });
    }
    let mut max_w = 0.0_f64;
    for s in &clean {
        max_w = max_w.max(s.width_left_upm).max(s.width_right_upm);
    }
    let eff = VariableWidthStrokeSpec {
        contour_id: spec.contour_id,
        samples: clean,
        cap_style: spec.cap_style,
    };
    let spine = sample_contour_spine(contour);
    let total = spine.last().map_or(0.0, |s| s.arc_len);
    if max_w <= eps || spine.len() < 2 || total <= eps {
        return Vec::new();
    }
    let mut left = Vec::with_capacity(spine.len());
    let mut right = Vec::with_capacity(spine.len());
    for s in &spine {
        let (wl, wr) = evaluate_variable_width_at_t(&eff, (s.arc_len / total).clamp(0.0, 1.0));
        let (nx, ny, x, y) = (-s.tangent.1, s.tangent.0, s.pt.x.0, s.pt.y.0);
        push_node(&mut left, nid, x + wl * nx, y + wl * ny, NodeType::Corner);
        push_node(&mut right, nid, x - wr * nx, y - wr * ny, NodeType::Corner);
    }
    if contour.closed {
        return build_closed_rings(left, right, nid);
    }
    let last_t = spine[spine.len() - 1].tangent;
    let open =
        assemble_open_variable_outline(left, right, spine[0].tangent, last_t, spec.cap_style, nid);
    let mut raw = vec![open];
    let _ = remove_overlap_in_contours(&mut raw, &mut vec![true], &[], nid);
    let mut res = Vec::new();
    for (idx, mut nodes) in raw.into_iter().enumerate() {
        let a = compute_contour_area_and_centroid(&nodes).0;
        if a.is_finite() && a.abs() > eps {
            if idx == 0 && contour_signed_area(&nodes) < 0.0 {
                reverse_contour_nodes(&mut nodes, true);
            }
            res.push(GlyphContour::new(ContourId(idx), nodes));
        }
    }
    res
}
