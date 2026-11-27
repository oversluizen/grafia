//! Interactive Planar Shape Builder (`CanvasTool::ShapeBuilder` — `SPEC-016 §3.1` & `§2.4.1`).
//!
//! Implements iterative boolean atomic cell decomposition on top of `boolean_ops.rs`
//! (`intersect_contours`, `subtract_contours`, `remove_overlap_in_contours`) with deterministic
//! centroid ordering, sliver filtering (`>= SHAPE_BUILDER_MIN_REGION_AREA_UPM2`), and seam-free
//! De Morgan union/subtraction for `ShapeBuilderOp::Merge` and `ShapeBuilderOp::Erase`.

use crate::core::bezier_ops::{Node, NodeId, NodeType};
use crate::core::boolean_ops::{
    contour_signed_area, intersect_contours, remove_overlap_in_contours, subtract_contours,
};
use crate::core::coordinates::FontPoint;
use crate::core::curve_manipulation::{extract_segments, ContourSegment};
use crate::core::glyph_model::{ContourId, GlyphContour};
use crate::core::lasso_and_zoom::point_in_polygon;
use crate::core::math_consts::{
    BOOL_CUBIC_SUBDIV_STEPS, ICON_STUDIO_MAX_COORD_ABS_UPM, SHAPE_BUILDER_MIN_REGION_AREA_UPM2,
};
use crate::core::reverse_contour_nodes;
use crate::core::stem_gauge_and_balance::compute_contour_area_and_centroid;
use kurbo::ParamCurve;
use serde::{Deserialize, Serialize};

/// Operation mode for `CanvasTool::ShapeBuilder` (`SPEC-016 §3.1`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum ShapeBuilderOp {
    #[default]
    Merge,
    Erase,
}

/// Minimal closed planar region produced by decomposing overlapping closed contours (`SPEC-016 §3.1`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ShapeBuilderRegion {
    pub region_id: u32,
    pub contour: GlyphContour,
    pub area_upm2: f64,
    pub centroid: FontPoint,
    pub source_contour_ids: Vec<ContourId>,
}

fn ensure_ccw_nodes(mut nodes: Vec<Node>) -> Vec<Node> {
    if contour_signed_area(&nodes) < 0.0 {
        reverse_contour_nodes(&mut nodes, true);
    }
    nodes
}

fn filter_valid_pieces(pieces: Vec<Vec<Node>>) -> Vec<Vec<Node>> {
    pieces
        .into_iter()
        .filter_map(|nodes| {
            let all_valid = nodes.iter().all(|n| {
                n.point.x.0.is_finite()
                    && n.point.y.0.is_finite()
                    && n.point.x.0.abs() <= ICON_STUDIO_MAX_COORD_ABS_UPM
                    && n.point.y.0.abs() <= ICON_STUDIO_MAX_COORD_ABS_UPM
            });
            if !all_valid {
                return None;
            }
            let on_count = nodes
                .iter()
                .filter(|n| n.node_type != NodeType::OffCurve)
                .count();
            if on_count < 2 {
                return None;
            }
            let (area, centroid) = compute_contour_area_and_centroid(&nodes);
            (area.is_finite()
                && area.abs() >= SHAPE_BUILDER_MIN_REGION_AREA_UPM2
                && centroid.x.0.is_finite()
                && centroid.y.0.is_finite())
            .then(|| ensure_ccw_nodes(nodes))
        })
        .collect()
}

fn sample_region_polygon(nodes: &[Node]) -> Vec<FontPoint> {
    let mut pts = Vec::new();
    let steps = BOOL_CUBIC_SUBDIV_STEPS;
    for seg in extract_segments(nodes, true) {
        match seg {
            ContourSegment::Line { line, .. } => pts.push(FontPoint::new(line.p0.x, line.p0.y)),
            ContourSegment::Cubic { cubic, .. } => {
                for s in 0..steps {
                    let p = cubic.eval(s as f64 / steps as f64);
                    pts.push(FontPoint::new(p.x, p.y));
                }
            }
        }
    }
    pts
}

fn partition_cells_against_contour(
    cells: Vec<(Vec<Node>, Vec<ContourId>)>,
    nodes_k: Vec<Node>,
    cid_k: ContourId,
    next_nid: &mut u64,
) -> Vec<(Vec<Node>, Vec<ContourId>)> {
    let mut next_cells: Vec<(Vec<Node>, Vec<ContourId>)> = Vec::new();
    let mut rem_k: Vec<Vec<Node>> = vec![nodes_k.clone()];

    for (cell_nodes, cell_sources) in cells {
        let inter = filter_valid_pieces(intersect_contours(
            std::slice::from_ref(&cell_nodes),
            std::slice::from_ref(&nodes_k),
            next_nid,
        ));
        if inter.is_empty() {
            next_cells.push((cell_nodes, cell_sources));
        } else {
            let diff_cell = filter_valid_pieces(subtract_contours(
                std::slice::from_ref(&cell_nodes),
                std::slice::from_ref(&nodes_k),
                next_nid,
            ));
            for d in diff_cell {
                next_cells.push((d, cell_sources.clone()));
            }
            let mut merged_sources = cell_sources;
            if !merged_sources.contains(&cid_k) {
                merged_sources.push(cid_k);
            }
            for piece in inter {
                next_cells.push((piece, merged_sources.clone()));
            }
            if !rem_k.is_empty() {
                rem_k = filter_valid_pieces(subtract_contours(
                    &rem_k,
                    std::slice::from_ref(&cell_nodes),
                    next_nid,
                ));
            }
        }
    }
    for r in rem_k {
        next_cells.push((r, vec![cid_k]));
    }
    next_cells
}

/// Partitions closed contours into disjoint minimal planar regions sorted by `(centroid.y, centroid.x)` (`SPEC-016 §3.1`).
#[must_use]
pub fn compute_shape_builder_regions(contours: &[GlyphContour]) -> Vec<ShapeBuilderRegion> {
    let max_nid = contours
        .iter()
        .flat_map(|c| c.nodes.iter().map(|n| n.id.0))
        .max()
        .unwrap_or(0);
    let mut next_nid = max_nid.saturating_add(1).max(1);
    let mut cells: Vec<(Vec<Node>, Vec<ContourId>)> = Vec::new();

    for gc in contours {
        if !gc.closed || !gc.visible {
            continue;
        }
        let Some(nodes_k) = filter_valid_pieces(vec![gc.nodes.clone()])
            .into_iter()
            .next()
        else {
            continue;
        };
        if cells.is_empty() {
            cells.push((nodes_k, vec![gc.id]));
        } else {
            cells = partition_cells_against_contour(cells, nodes_k, gc.id, &mut next_nid);
        }
    }

    let mut raw_regions: Vec<(Vec<Node>, f64, FontPoint, Vec<ContourId>)> = cells
        .into_iter()
        .filter_map(|(nodes, sources)| {
            let (area, centroid) = compute_contour_area_and_centroid(&nodes);
            let abs_area = area.abs();
            (abs_area >= SHAPE_BUILDER_MIN_REGION_AREA_UPM2)
                .then_some((nodes, abs_area, centroid, sources))
        })
        .collect();

    raw_regions.sort_by(|a, b| {
        a.2.y
            .0
            .partial_cmp(&b.2.y.0)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| {
                a.2.x
                    .0
                    .partial_cmp(&b.2.x.0)
                    .unwrap_or(std::cmp::Ordering::Equal)
            })
    });

    raw_regions
        .into_iter()
        .enumerate()
        .map(
            |(idx, (nodes, area_upm2, centroid, source_contour_ids))| ShapeBuilderRegion {
                region_id: idx as u32,
                contour: GlyphContour::new(ContourId(idx), nodes),
                area_upm2,
                centroid,
                source_contour_ids,
            },
        )
        .collect()
}

/// Hit-tests `regions` at `pt_upm` in font space, returning the `region_id` of the smallest containing region (`SPEC-016 §3.1`).
#[must_use]
pub fn hit_test_shape_builder_region(
    regions: &[ShapeBuilderRegion],
    pt_upm: FontPoint,
) -> Option<u32> {
    if !pt_upm.x.0.is_finite() || !pt_upm.y.0.is_finite() {
        return None;
    }
    regions
        .iter()
        .filter(|reg| {
            let poly = sample_region_polygon(&reg.contour.nodes);
            poly.len() >= 3 && point_in_polygon(pt_upm, &poly)
        })
        .min_by(|a, b| {
            a.area_upm2
                .partial_cmp(&b.area_upm2)
                .unwrap_or(std::cmp::Ordering::Equal)
        })
        .map(|reg| reg.region_id)
}

fn nodes_to_fresh_closed_contour(mut nodes: Vec<Node>, next_node_id: &mut u64) -> GlyphContour {
    for n in &mut nodes {
        n.id = NodeId(*next_node_id);
        *next_node_id += 1;
    }
    GlyphContour::new(ContourId(0), nodes)
}

fn merge_selected_shape_regions(
    contours: &[GlyphContour],
    regions: &[ShapeBuilderRegion],
    selected: &[&ShapeBuilderRegion],
    region_ids: &[u32],
    next_node_id: &mut u64,
) -> Vec<GlyphContour> {
    let mut affected_sources: Vec<ContourId> = Vec::new();
    for r in selected {
        let is_overlapping_region = r.source_contour_ids.len() > 1
            || r.source_contour_ids.iter().any(|sid| {
                regions
                    .iter()
                    .filter(|reg| reg.source_contour_ids.contains(sid))
                    .count()
                    > 1
            });
        if !is_overlapping_region {
            continue;
        }
        for &sid in &r.source_contour_ids {
            if !affected_sources.contains(&sid) {
                affected_sources.push(sid);
            }
        }
    }
    if affected_sources.is_empty() {
        for r in selected {
            for &sid in &r.source_contour_ids {
                if !affected_sources.contains(&sid) {
                    affected_sources.push(sid);
                }
            }
        }
    }
    let mut source_union_nodes: Vec<Vec<Node>> = contours
        .iter()
        .filter(|c| c.closed && affected_sources.contains(&c.id))
        .map(|c| c.nodes.clone())
        .collect();
    let mut closed_flags = vec![true; source_union_nodes.len()];
    let _ = remove_overlap_in_contours(
        &mut source_union_nodes,
        &mut closed_flags,
        &[],
        next_node_id,
    );
    let unselected_in_affected: Vec<Vec<Node>> = regions
        .iter()
        .filter(|r| {
            !region_ids.contains(&r.region_id)
                && r.source_contour_ids
                    .iter()
                    .any(|sid| affected_sources.contains(sid))
        })
        .map(|r| r.contour.nodes.clone())
        .collect();

    let mut synth_nodes = if unselected_in_affected.is_empty() {
        source_union_nodes
    } else {
        filter_valid_pieces(subtract_contours(
            &source_union_nodes,
            &unselected_in_affected,
            next_node_id,
        ))
    };
    synth_nodes.extend(unselected_in_affected);
    let mut out: Vec<GlyphContour> = synth_nodes
        .into_iter()
        .map(|nodes| nodes_to_fresh_closed_contour(nodes, next_node_id))
        .collect();
    for c in contours {
        if !c.closed || !affected_sources.contains(&c.id) {
            out.push(c.clone());
        }
    }
    out
}

fn erase_selected_shape_regions(
    contours: &[GlyphContour],
    regions: &[ShapeBuilderRegion],
    region_ids: &[u32],
    next_node_id: &mut u64,
) -> Vec<GlyphContour> {
    let erased_contours: Vec<Vec<Node>> = regions
        .iter()
        .filter(|r| region_ids.contains(&r.region_id))
        .map(|r| r.contour.nodes.clone())
        .collect();
    let erased_sources: Vec<ContourId> = regions
        .iter()
        .filter(|r| region_ids.contains(&r.region_id))
        .flat_map(|r| r.source_contour_ids.iter().copied())
        .collect();

    let mut out: Vec<GlyphContour> = Vec::new();
    let mut intact_closed: Vec<Vec<Node>> = Vec::new();
    let mut covered_region_ids: Vec<u32> = Vec::new();

    for c in contours {
        if !c.closed || !erased_sources.contains(&c.id) {
            out.push(c.clone());
            if c.closed {
                intact_closed.push(c.nodes.clone());
            }
            for r in regions {
                if r.source_contour_ids.contains(&c.id)
                    && !covered_region_ids.contains(&r.region_id)
                {
                    covered_region_ids.push(r.region_id);
                }
            }
        }
    }

    for c in contours {
        if !c.closed || !erased_sources.contains(&c.id) {
            continue;
        }
        let has_uncovered = regions.iter().any(|r| {
            r.source_contour_ids.contains(&c.id)
                && !region_ids.contains(&r.region_id)
                && !covered_region_ids.contains(&r.region_id)
        });
        if !has_uncovered {
            continue;
        }
        let mut clip = erased_contours.clone();
        clip.extend(intact_closed.iter().cloned());
        let diff = filter_valid_pieces(subtract_contours(
            std::slice::from_ref(&c.nodes),
            &clip,
            next_node_id,
        ));
        for piece in diff {
            intact_closed.push(piece.clone());
            out.push(nodes_to_fresh_closed_contour(piece, next_node_id));
        }
        for r in regions {
            if r.source_contour_ids.contains(&c.id)
                && !region_ids.contains(&r.region_id)
                && !covered_region_ids.contains(&r.region_id)
            {
                covered_region_ids.push(r.region_id);
            }
        }
    }
    out
}

/// Applies `ShapeBuilderOp::Merge` or `ShapeBuilderOp::Erase` across `region_ids` on `contours` (`SPEC-016 §3.1`).
pub fn apply_shape_builder_regions_to_contours(
    contours: &[GlyphContour],
    region_ids: &[u32],
    op: ShapeBuilderOp,
    next_node_id: &mut u64,
) -> Option<Vec<GlyphContour>> {
    if region_ids.is_empty() {
        return None;
    }
    let regions = compute_shape_builder_regions(contours);
    let selected: Vec<&ShapeBuilderRegion> = regions
        .iter()
        .filter(|r| region_ids.contains(&r.region_id))
        .collect();
    if selected.is_empty() || (op == ShapeBuilderOp::Merge && selected.len() < 2) {
        return None;
    }

    let mut result: Vec<GlyphContour> = match op {
        ShapeBuilderOp::Erase => {
            erase_selected_shape_regions(contours, &regions, region_ids, next_node_id)
        }
        ShapeBuilderOp::Merge => {
            merge_selected_shape_regions(contours, &regions, &selected, region_ids, next_node_id)
        }
    };
    for (idx, c) in result.iter_mut().enumerate() {
        c.id = ContourId(idx);
    }
    Some(result)
}
