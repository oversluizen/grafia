//! $N$-Fold Radial Symmetry, Stem-Preserving Optical Oblique & 5-Stage Multi-Scale Icon Normalizer
//! (`SPEC-016 §3.3` & `§3.4`).

use crate::core::bezier_ops::{Node, NodeId, NodeType};
use crate::core::boolean_ops::remove_overlap_in_contours;
use crate::core::contour_order_ops::correct_path_direction_layer;
use crate::core::coordinates::FontPoint;
use crate::core::glyph_model::{ContourId, GlyphContour};
use crate::core::icon_studio::RadialSymmetrySpec;
use crate::core::math_consts::{
    DEG_TO_RAD_F64, FIT_CURVE_MIN_CHORD_LEN_EPS, ICON_NORMALIZE_MIN_SEGMENT_EPS_UPM,
    ICON_STUDIO_MAX_COORD_ABS_UPM, RADIAL_SYMMETRY_MAX_FOLDS, RADIAL_SYMMETRY_MIN_FOLDS,
    ROTARY_ITALIC_MAX_ABS_DEG, TAU_F64, VARIABLE_WIDTH_MAX_UPM,
};
use crate::core::path_ops::{add_extremes_to_contours, round_grid_pt};
use crate::core::reverse_contour_nodes;
use crate::core::stem_gauge_and_balance::compute_contour_area_and_centroid;
use crate::core::tidy_align_connect::tidy_up_contours;

fn is_valid_contour(c: &GlyphContour) -> bool {
    c.visible
        && c.nodes.len() >= 2
        && c.nodes.iter().all(|n| {
            n.point.x.0.is_finite()
                && n.point.y.0.is_finite()
                && n.point.x.0.abs() <= ICON_STUDIO_MAX_COORD_ABS_UPM
                && n.point.y.0.abs() <= ICON_STUDIO_MAX_COORD_ABS_UPM
        })
}

fn sanitize_center(pt: FontPoint) -> FontPoint {
    let lim = ICON_STUDIO_MAX_COORD_ABS_UPM;
    let cx = if pt.x.0.is_finite() {
        pt.x.0.clamp(-lim, lim)
    } else {
        0.0
    };
    let cy = if pt.y.0.is_finite() {
        pt.y.0.clamp(-lim, lim)
    } else {
        0.0
    };
    FontPoint::new(cx, cy)
}

fn transform_contour_sector(
    contour: &GlyphContour,
    center: FontPoint,
    cos_a: f64,
    sin_a: f64,
    mirror_y: bool,
    cid: usize,
    next_nid: &mut u64,
) -> GlyphContour {
    let mut nodes: Vec<Node> = contour
        .nodes
        .iter()
        .map(|n| {
            let dx = n.point.x.0 - center.x.0;
            let dy = if mirror_y {
                -(n.point.y.0 - center.y.0)
            } else {
                n.point.y.0 - center.y.0
            };
            let rx = center.x.0 + dx * cos_a - dy * sin_a;
            let ry = center.y.0 + dx * sin_a + dy * cos_a;
            let id = NodeId(*next_nid);
            *next_nid += 1;
            Node::new(id, rx, ry, n.node_type)
        })
        .collect();
    if mirror_y {
        reverse_contour_nodes(&mut nodes, contour.closed);
    }
    let mut gc = GlyphContour::new(ContourId(cid), nodes);
    gc.closed = contour.closed;
    gc
}

/// Generates $N$-fold rotational (and optional dihedral mirror) symmetry copies around `spec.center` (`SPEC-016 §3.3.1`).
#[must_use]
pub fn compute_radial_symmetry_contours(
    contours: &[GlyphContour],
    spec: &RadialSymmetrySpec,
    next_node_id: &mut u64,
) -> Vec<GlyphContour> {
    let folds = spec
        .fold_count
        .clamp(RADIAL_SYMMETRY_MIN_FOLDS, RADIAL_SYMMETRY_MAX_FOLDS);
    let center = sanitize_center(spec.center);
    let mut out = Vec::new();
    for k in 0..folds {
        let theta = (f64::from(k) * TAU_F64) / f64::from(folds);
        let (sin_a, cos_a) = theta.sin_cos();
        for c in contours {
            if !is_valid_contour(c) {
                continue;
            }
            let cid = out.len();
            out.push(transform_contour_sector(
                c,
                center,
                cos_a,
                sin_a,
                false,
                cid,
                next_node_id,
            ));
            if spec.dihedral_mirror {
                let m_cid = out.len();
                out.push(transform_contour_sector(
                    c,
                    center,
                    cos_a,
                    sin_a,
                    true,
                    m_cid,
                    next_node_id,
                ));
            }
        }
    }
    out
}

/// Applies horizontal oblique shear by `slant_deg` while optionally compensating horizontal stem
/// offsets by $\sec(\alpha)$ to preserve perpendicular stem thickness (`SPEC-016 §3.3.2`).
#[must_use]
pub fn compute_optical_oblique_contours(
    contours: &[GlyphContour],
    slant_deg: f64,
    preserve_stem_width: bool,
    next_node_id: &mut u64,
) -> Vec<GlyphContour> {
    let deg = if slant_deg.is_finite() {
        slant_deg.clamp(-ROTARY_ITALIC_MAX_ABS_DEG, ROTARY_ITALIC_MAX_ABS_DEG)
    } else {
        0.0
    };
    let alpha = deg * DEG_TO_RAD_F64;
    let tan_a = alpha.tan();
    let sec_a = if preserve_stem_width {
        1.0 / alpha.cos().max(FIT_CURVE_MIN_CHORD_LEN_EPS)
    } else {
        1.0
    };
    let mut sum_w = 0.0_f64;
    let (mut sum_x, mut sum_y) = (0.0_f64, 0.0_f64);
    for c in contours {
        if is_valid_contour(c) {
            let (area, cent) = compute_contour_area_and_centroid(&c.nodes);
            let w = area.abs();
            if w > FIT_CURVE_MIN_CHORD_LEN_EPS && cent.x.0.is_finite() && cent.y.0.is_finite() {
                sum_w += w;
                sum_x += w * cent.x.0;
                sum_y += w * cent.y.0;
            }
        }
    }
    let (shared_cx, shared_cy) = if sum_w > FIT_CURVE_MIN_CHORD_LEN_EPS {
        (sum_x / sum_w, sum_y / sum_w)
    } else {
        contours
            .iter()
            .find(|c| is_valid_contour(c))
            .map_or((0.0, 0.0), |c| (c.nodes[0].point.x.0, c.nodes[0].point.y.0))
    };
    let mut out = Vec::with_capacity(contours.len());
    for c in contours {
        if !is_valid_contour(c) {
            continue;
        }
        let nodes: Vec<Node> = c
            .nodes
            .iter()
            .map(|n| {
                let dx = n.point.x.0 - shared_cx;
                let dy = n.point.y.0 - shared_cy;
                let nx = shared_cx + dx * sec_a + dy * tan_a;
                let ny = n.point.y.0;
                let id = if n.id.0 > 0 {
                    *next_node_id = (*next_node_id).max(n.id.0 + 1);
                    n.id
                } else {
                    let id = NodeId(*next_node_id);
                    *next_node_id += 1;
                    id
                };
                Node::new(id, nx, ny, n.node_type)
            })
            .collect();
        let mut gc = GlyphContour::new(ContourId(out.len()), nodes);
        gc.closed = c.closed;
        out.push(gc);
    }
    out
}

fn heal_degenerate_short_segments(nodes: &mut Vec<Node>, closed: bool) {
    let min_on = if closed { 3 } else { 2 };
    let mut i = 0usize;
    while i < nodes.len() {
        if nodes[i].node_type == NodeType::OffCurve {
            i += 1;
            continue;
        }
        let on_count = nodes
            .iter()
            .filter(|n| n.node_type != NodeType::OffCurve)
            .count();
        if on_count <= min_on {
            break;
        }
        let next_i = (i + 1) % nodes.len();
        if (!closed && i + 1 >= nodes.len()) || nodes[next_i].node_type == NodeType::OffCurve {
            i += 1;
            continue;
        }
        if nodes[i].point.distance_to(nodes[next_i].point) <= ICON_NORMALIZE_MIN_SEGMENT_EPS_UPM {
            nodes.remove(next_i);
            continue;
        }
        i += 1;
    }
}

fn finalize_normalized_contours(
    raw_nodes: Vec<Vec<Node>>,
    closed_flags: &[bool],
    next_node_id: &mut u64,
) -> Vec<GlyphContour> {
    let mut out = Vec::new();
    let mut seen_ids = std::collections::HashSet::new();
    for (idx, mut nodes) in raw_nodes.into_iter().enumerate() {
        let cl = closed_flags.get(idx).copied().unwrap_or(true);
        if cl {
            let (area, _) = compute_contour_area_and_centroid(&nodes);
            if !area.is_finite() || area.abs() <= FIT_CURVE_MIN_CHORD_LEN_EPS {
                continue;
            }
        } else if nodes.len() < 2 {
            continue;
        }
        for n in &mut nodes {
            if n.id.0 == 0 || !seen_ids.insert(n.id) {
                n.id = NodeId(*next_node_id);
                seen_ids.insert(n.id);
                *next_node_id += 1;
            } else {
                *next_node_id = (*next_node_id).max(n.id.0 + 1);
            }
        }
        let mut gc = GlyphContour::new(ContourId(out.len()), nodes);
        gc.closed = cl;
        out.push(gc);
    }
    out
}

/// Deterministic 5-stage *"Clean for Font & Glass"* icon contour normalizer (`SPEC-016 §3.4`):
/// 1. Removes self-overlapping contours (`remove_overlap_in_contours`),
/// 2. Inserts horizontal and vertical extrema (`add_extremes_to_contours`),
/// 3. Corrects outer/counter winding directions (`correct_path_direction_layer`),
/// 4. Heals degenerate segments within `ICON_NORMALIZE_MIN_SEGMENT_EPS_UPM` (`0.25 UPM`),
/// 5. Optionally snaps coordinates to `grid_step_opt`.
#[must_use]
pub fn audit_and_normalize_icon_contours(
    contours: &[GlyphContour],
    grid_step_opt: Option<f64>,
    next_node_id: &mut u64,
) -> Vec<GlyphContour> {
    let step = grid_step_opt
        .filter(|s| {
            s.is_finite() && *s >= FIT_CURVE_MIN_CHORD_LEN_EPS && *s <= VARIABLE_WIDTH_MAX_UPM
        })
        .unwrap_or(0.0);
    let mut raw_nodes: Vec<Vec<Node>> = Vec::new();
    let mut closed_flags: Vec<bool> = Vec::new();
    for c in contours {
        if is_valid_contour(c) {
            raw_nodes.push(c.nodes.clone());
            closed_flags.push(c.closed);
        }
    }
    if raw_nodes.is_empty() {
        return Vec::new();
    }
    if closed_flags.iter().any(|&cl| cl) {
        let _ = remove_overlap_in_contours(&mut raw_nodes, &mut closed_flags, &[], next_node_id);
    }
    let _ = add_extremes_to_contours(
        &mut raw_nodes,
        &mut closed_flags,
        &[],
        true,
        next_node_id,
        0.0,
    );
    for (idx, nodes) in raw_nodes.iter_mut().enumerate() {
        heal_degenerate_short_segments(nodes, closed_flags.get(idx).copied().unwrap_or(true));
    }
    let _ = tidy_up_contours(&mut raw_nodes, &mut closed_flags, &[], 0.0);
    let _ = correct_path_direction_layer(&mut raw_nodes, &closed_flags, 0.0, 0.0);
    if step > 0.0 {
        for n in raw_nodes.iter_mut().flatten() {
            n.point = round_grid_pt(n.point, step);
        }
        let _ = add_extremes_to_contours(
            &mut raw_nodes,
            &mut closed_flags,
            &[],
            true,
            next_node_id,
            step,
        );
        for n in raw_nodes.iter_mut().flatten() {
            n.point = round_grid_pt(n.point, step);
        }
        let _ = correct_path_direction_layer(&mut raw_nodes, &closed_flags, 0.0, 0.0);
    }
    finalize_normalized_contours(raw_nodes, &closed_flags, next_node_id)
}
