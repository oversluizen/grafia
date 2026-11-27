//! De Casteljau Cubic Arc-Length Parameterization & Path Trimming (`SPEC-016 §4.2` #5–#6):
//! `contour_total_arc_length` and `trim_contour_by_arc_ratio` for `Appear` (`DrawOn`) and
//! `Disappear` (`DrawOff`) kinetic symbol effects.

use crate::core::bezier_ops::{Node, NodeId, NodeType};
use crate::core::color_layers::CanvasColorSublayer;
use crate::core::coordinates::FontPoint;
use crate::core::curve_manipulation::{extract_segments, ContourSegment};
use crate::core::glyph_model::GlyphContour;
use crate::core::icon_knockout_mask::evaluate_motion_path_pose_at_ratio;
use crate::core::icon_studio::{KineticSymbolEffect, KineticTimelineState, SymbolLayerTag};
use crate::core::kinetic_magic_replace::{
    evaluate_magic_replace_color_sublayers, evaluate_magic_replace_glyph_contours,
};
use crate::core::kinetic_symbols::{
    apply_layer_kinetic_transform, evaluate_layer_kinetic_transform, resolve_layer_pivot_upm,
    LayerKineticTransform,
};
use crate::core::math_consts::{
    BOOL_CUBIC_SUBDIV_STEPS, FIT_CURVE_MIN_CHORD_LEN_EPS, ICON_STUDIO_MAX_COORD_ABS_UPM,
};
use crate::core::reverse_contour_nodes;
use crate::core::selection_info::CanvasAnchor;
use kurbo::{CubicBez, ParamCurve};

fn valid_contour_coords(contour: &GlyphContour) -> bool {
    let lim = ICON_STUDIO_MAX_COORD_ABS_UPM;
    contour.nodes.len() >= 2
        && contour.nodes.iter().all(|n| {
            n.point.x.0.is_finite()
                && n.point.y.0.is_finite()
                && n.point.x.0.abs() <= lim
                && n.point.y.0.abs() <= lim
        })
}

pub(crate) fn cubic_arc_table(cubic: CubicBez) -> [f64; BOOL_CUBIC_SUBDIV_STEPS + 1] {
    let mut table = [0.0_f64; BOOL_CUBIC_SUBDIV_STEPS + 1];
    let mut prev = cubic.p0;
    let mut cum = 0.0_f64;
    for (step, slot) in table.iter_mut().enumerate().skip(1) {
        let u = (step as f64) / (BOOL_CUBIC_SUBDIV_STEPS as f64);
        let cur = cubic.eval(u);
        cum += (cur.x - prev.x).hypot(cur.y - prev.y);
        *slot = cum;
        prev = cur;
    }
    table
}

pub(crate) fn cubic_param_for_arc_fraction(
    table: &[f64; BOOL_CUBIC_SUBDIV_STEPS + 1],
    frac: f64,
) -> f64 {
    let total = table[BOOL_CUBIC_SUBDIV_STEPS];
    let f = frac.clamp(0.0, 1.0);
    if total <= FIT_CURVE_MIN_CHORD_LEN_EPS || f <= 0.0 {
        return 0.0;
    }
    if f >= 1.0 {
        return 1.0;
    }
    let target = f * total;
    for k in 0..BOOL_CUBIC_SUBDIV_STEPS {
        if target >= table[k] && target <= table[k + 1] {
            let seg = (table[k + 1] - table[k]).max(FIT_CURVE_MIN_CHORD_LEN_EPS);
            let local = ((target - table[k]) / seg).clamp(0.0, 1.0);
            return ((k as f64) + local) / (BOOL_CUBIC_SUBDIV_STEPS as f64);
        }
    }
    1.0
}

fn segment_arc_length(seg: &ContourSegment) -> f64 {
    match *seg {
        ContourSegment::Line { line, .. } => (line.p1.x - line.p0.x).hypot(line.p1.y - line.p0.y),
        ContourSegment::Cubic { cubic, .. } => cubic_arc_table(cubic)[BOOL_CUBIC_SUBDIV_STEPS],
    }
}

/// Computes the total geometric arc length $L_{\text{total}}$ of `contour` in UPM (`SPEC-016 §4.2`).
#[must_use]
pub fn contour_total_arc_length(contour: &GlyphContour) -> f64 {
    if !valid_contour_coords(contour) {
        return 0.0;
    }
    extract_segments(&contour.nodes, contour.closed)
        .iter()
        .map(segment_arc_length)
        .sum()
}

fn push_trimmed_node(out: &mut Vec<Node>, next_nid: &mut u64, x: f64, y: f64, nt: NodeType) {
    if nt != NodeType::OffCurve {
        if let Some(last) = out.last() {
            if last.node_type != NodeType::OffCurve
                && last.point.distance_to(FontPoint::new(x, y)) <= FIT_CURVE_MIN_CHORD_LEN_EPS
            {
                return;
            }
        }
    }
    let id = NodeId(*next_nid);
    *next_nid += 1;
    out.push(Node::new(id, x, y, nt));
}

fn append_trimmed_segment(
    out_nodes: &mut Vec<Node>,
    next_nid: &mut u64,
    seg: &ContourSegment,
    f0: f64,
    f1: f64,
) {
    match *seg {
        ContourSegment::Line { line, .. } => {
            let (p0, p1) = (line.eval(f0), line.eval(f1));
            push_trimmed_node(out_nodes, next_nid, p0.x, p0.y, NodeType::Corner);
            push_trimmed_node(out_nodes, next_nid, p1.x, p1.y, NodeType::Corner);
        }
        ContourSegment::Cubic { cubic, .. } => {
            let table = cubic_arc_table(cubic);
            let u0 = cubic_param_for_arc_fraction(&table, f0);
            let u1 = cubic_param_for_arc_fraction(&table, f1);
            if u1 > u0 + FIT_CURVE_MIN_CHORD_LEN_EPS {
                let sub = cubic.subsegment(u0..u1);
                push_trimmed_node(out_nodes, next_nid, sub.p0.x, sub.p0.y, NodeType::Corner);
                push_trimmed_node(out_nodes, next_nid, sub.p1.x, sub.p1.y, NodeType::OffCurve);
                push_trimmed_node(out_nodes, next_nid, sub.p2.x, sub.p2.y, NodeType::OffCurve);
                push_trimmed_node(out_nodes, next_nid, sub.p3.x, sub.p3.y, NodeType::Corner);
            }
        }
    }
}

fn empty_open_contour(contour: &GlyphContour) -> GlyphContour {
    let mut empty = GlyphContour::new(contour.id, Vec::new());
    empty.closed = false;
    empty.visible = contour.visible;
    empty
}

/// Extracts the exact De Casteljau cubic sub-contour of `contour` covering normalized arc-length
/// `[start_ratio, end_ratio]` in `[0.0, 1.0]` (`SPEC-016 §4.2` #5–#6).
#[must_use]
pub fn trim_contour_by_arc_ratio(
    contour: &GlyphContour,
    start_ratio: f64,
    end_ratio: f64,
    reversed: bool,
) -> GlyphContour {
    if !valid_contour_coords(contour) {
        return empty_open_contour(contour);
    }
    let r0 = if start_ratio.is_finite() {
        start_ratio.clamp(0.0, 1.0)
    } else {
        0.0
    };
    let r1 = if end_ratio.is_finite() {
        end_ratio.clamp(0.0, 1.0)
    } else {
        0.0
    };
    if r1 <= r0 + FIT_CURVE_MIN_CHORD_LEN_EPS {
        return empty_open_contour(contour);
    }
    let mut work_nodes = contour.nodes.clone();
    if reversed {
        reverse_contour_nodes(&mut work_nodes, contour.closed);
    }
    if r0 <= FIT_CURVE_MIN_CHORD_LEN_EPS && r1 >= 1.0 - FIT_CURVE_MIN_CHORD_LEN_EPS {
        let mut full = GlyphContour::new(contour.id, work_nodes);
        full.closed = contour.closed;
        full.visible = contour.visible;
        return full;
    }

    let segs = extract_segments(&work_nodes, contour.closed);
    let seg_lens: Vec<f64> = segs.iter().map(segment_arc_length).collect();
    let total_len: f64 = seg_lens.iter().sum();
    if total_len <= FIT_CURVE_MIN_CHORD_LEN_EPS {
        return empty_open_contour(contour);
    }
    let (s0, s1) = (r0 * total_len, r1 * total_len);
    let mut cum = 0.0_f64;
    let mut out_nodes: Vec<Node> = Vec::new();
    let mut next_nid = work_nodes.iter().map(|n| n.id.0).max().unwrap_or(0) + 1;

    for (seg, &s_len) in segs.iter().zip(&seg_lens) {
        let (a, b) = (cum, cum + s_len);
        cum = b;
        if s_len <= FIT_CURVE_MIN_CHORD_LEN_EPS
            || b <= s0 + FIT_CURVE_MIN_CHORD_LEN_EPS
            || a >= s1 - FIT_CURVE_MIN_CHORD_LEN_EPS
        {
            continue;
        }
        let f0 = ((s0.max(a) - a) / s_len).clamp(0.0, 1.0);
        let f1 = ((s1.min(b) - a) / s_len).clamp(0.0, 1.0);
        if f1 > f0 + FIT_CURVE_MIN_CHORD_LEN_EPS {
            append_trimmed_segment(&mut out_nodes, &mut next_nid, seg, f0, f1);
        }
    }

    let mut out = GlyphContour::new(contour.id, out_nodes);
    out.closed = false;
    out.visible = contour.visible;
    out
}

fn collect_group_ranking(len: usize, tags: &[SymbolLayerTag]) -> (Vec<u8>, Vec<usize>) {
    let mut groups: Vec<u8> = (0..len)
        .map(|i| {
            tags.get(i)
                .map_or((i as u8).saturating_add(1), |t| t.layer_group.max(1))
        })
        .collect();
    groups.sort_unstable();
    groups.dedup();
    let var_idx = (0..len)
        .filter(|&i| tags.get(i).is_none_or(|t| t.variable_color_enabled))
        .collect();
    (groups, var_idx)
}

fn eval_slot_tf(
    i: usize,
    tags: &[SymbolLayerTag],
    groups: &[u8],
    var_idx: &[usize],
    kinetic: &KineticTimelineState,
    pivot: FontPoint,
) -> LayerKineticTransform {
    let tag = tags.get(i);
    let lg = tag.map_or((i as u8).saturating_add(1), |t| t.layer_group.max(1));
    let g_idx = groups.binary_search(&lg).unwrap_or(0);
    let total_v = var_idx.len().max(1);
    let v_rank = var_idx
        .iter()
        .position(|&idx| idx == i)
        .map(|p| (p, total_v));
    let eff_t = kinetic.effective_phase_t();
    let offset = tag.map_or(0.0, |t| {
        if t.phase_offset.is_finite() {
            t.phase_offset.clamp(-1.0, 1.0)
        } else {
            0.0
        }
    });
    let local_t = if offset.abs() > FIT_CURVE_MIN_CHORD_LEN_EPS {
        (eff_t + offset).rem_euclid(1.0)
    } else {
        eff_t
    };
    evaluate_layer_kinetic_transform(
        &kinetic.effect,
        local_t,
        g_idx,
        groups.len().max(1),
        v_rank,
        tag,
        pivot,
    )
}

fn apply_motion_path_to_tf(
    tf: &mut LayerKineticTransform,
    tag: Option<&SymbolLayerTag>,
    motion_sources: &[GlyphContour],
    tau: f64,
) {
    let Some(tg) = tag else { return };
    let Some(mp_cid) = tg.motion_path_contour_id else {
        return;
    };
    let Some(mp) = motion_sources.iter().find(|c| c.id == mp_cid) else {
        return;
    };
    if let Some((pt, ang)) = evaluate_motion_path_pose_at_ratio(mp, tau) {
        tf.translation_upm.x.0 += pt.x.0 - tf.pivot_upm.x.0;
        tf.translation_upm.y.0 += pt.y.0 - tf.pivot_upm.y.0;
        if tg.orient_along_motion_path {
            tf.rotation_rad += ang;
        }
    }
}

/// Evaluates all `CanvasColorSublayer`s at `kinetic.phase_t` (`SPEC-016 §4.2`, `SPEC-017 §2.6`).
#[must_use]
pub fn evaluate_kinetic_color_sublayers(
    sublayers: &[CanvasColorSublayer],
    tags: &[SymbolLayerTag],
    anchors: &[CanvasAnchor],
    kinetic: &KineticTimelineState,
) -> Vec<CanvasColorSublayer> {
    evaluate_kinetic_color_sublayers_with_paths(sublayers, tags, anchors, kinetic, &[])
}

/// Evaluates all `CanvasColorSublayer`s with optional reference `motion_contours` for `motion_path_contour_id` (`SPEC-017 §2.6`).
#[must_use]
pub fn evaluate_kinetic_color_sublayers_with_paths(
    sublayers: &[CanvasColorSublayer],
    tags: &[SymbolLayerTag],
    anchors: &[CanvasAnchor],
    kinetic: &KineticTimelineState,
    motion_contours: &[GlyphContour],
) -> Vec<CanvasColorSublayer> {
    if sublayers.is_empty() {
        return Vec::new();
    }
    if let KineticSymbolEffect::MagicReplace {
        fallback,
        scope,
        ref target_contours,
    } = kinetic.effect
    {
        return evaluate_magic_replace_color_sublayers(
            sublayers,
            target_contours,
            fallback,
            scope,
            tags,
            anchors,
            kinetic,
        );
    }
    let (groups, var_idx) = collect_group_ranking(sublayers.len(), tags);
    let eff_t = kinetic.effective_phase_t();
    sublayers
        .iter()
        .enumerate()
        .map(|(i, sub)| {
            let pivot = resolve_layer_pivot_upm(&sub.contours, tags.get(i), anchors);
            let mut tf = eval_slot_tf(i, tags, &groups, &var_idx, kinetic, pivot);
            apply_motion_path_to_tf(&mut tf, tags.get(i), motion_contours, eff_t);
            let mut eval_sub = sub.clone();
            eval_sub.contours = sub
                .contours
                .iter()
                .map(|c| apply_layer_kinetic_transform(c, &tf))
                .collect();
            eval_sub.opacity = (sub.opacity * tf.opacity_factor).clamp(0.0, 1.0);
            eval_sub
        })
        .collect()
}

/// Evaluates primary `GlyphContour`s at `kinetic.phase_t`, sharing group pivots across `layer_group` (`SPEC-016 §4.2`).
#[must_use]
pub fn evaluate_kinetic_glyph_contours(
    contours: &[GlyphContour],
    tags: &[SymbolLayerTag],
    anchors: &[CanvasAnchor],
    kinetic: &KineticTimelineState,
) -> Vec<(GlyphContour, f32)> {
    if contours.is_empty() {
        return Vec::new();
    }
    if let KineticSymbolEffect::MagicReplace {
        fallback,
        scope,
        ref target_contours,
    } = kinetic.effect
    {
        return evaluate_magic_replace_glyph_contours(
            contours,
            target_contours,
            fallback,
            scope,
            tags,
            anchors,
            kinetic.phase_t,
        );
    }
    let (groups, var_idx) = collect_group_ranking(contours.len(), tags);
    let tag_lg = |i: usize| {
        tags.get(i)
            .map_or((i as u8).saturating_add(1), |t| t.layer_group.max(1))
    };
    let pivots: Vec<FontPoint> = groups
        .iter()
        .map(|&lg| {
            let gc_vec: Vec<GlyphContour> = contours
                .iter()
                .enumerate()
                .filter(|&(i, _)| tag_lg(i) == lg)
                .map(|(_, c)| c.clone())
                .collect();
            let first_tag = (0..contours.len())
                .find(|&i| tag_lg(i) == lg)
                .and_then(|i| tags.get(i));
            resolve_layer_pivot_upm(&gc_vec, first_tag, anchors)
        })
        .collect();

    let eff_t = kinetic.effective_phase_t();
    contours
        .iter()
        .enumerate()
        .map(|(i, c)| {
            let g_idx = groups.binary_search(&tag_lg(i)).unwrap_or(0);
            let pivot = pivots.get(g_idx).copied().unwrap_or_default();
            let mut tf = eval_slot_tf(i, tags, &groups, &var_idx, kinetic, pivot);
            apply_motion_path_to_tf(&mut tf, tags.get(i), contours, eff_t);
            (apply_layer_kinetic_transform(c, &tf), tf.opacity_factor)
        })
        .collect()
}
