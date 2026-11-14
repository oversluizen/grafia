//! `SPEC-009 §2.4` Non-destructive interior segment-segment intersection solver producing
//! deterministic `VirtualExtraNode` markers with canonical `ExtraNodeKey` addressing.

use crate::core::boolean_geom::{extract_bool_segs, intersect_pair, BoolSeg};
use crate::core::canvas_entities::{ExtraNodeKey, VirtualExtraNode};
use crate::core::math_consts::{
    EXTRA_NODE_DEDUP_DIST_UPM, EXTRA_NODE_DEDUP_T_EPS, EXTRA_NODE_HIT_RADIUS_DP,
    EXTRA_NODE_INTERIOR_T_MAX, EXTRA_NODE_INTERIOR_T_MIN,
};
use crate::core::{FontPoint, GlyphContour, ScreenPoint, ViewportTransform};

#[must_use]
pub fn compute_layer_extra_nodes(contours: &[GlyphContour]) -> Vec<VirtualExtraNode> {
    let all_segs: Vec<Vec<BoolSeg>> = contours
        .iter()
        .map(|c| {
            if c.visible && c.nodes.len() >= 2 {
                extract_bool_segs(&c.nodes, c.closed)
            } else {
                Vec::new()
            }
        })
        .collect();
    let mut results = Vec::new();

    for (ca, segs_a) in all_segs.iter().enumerate() {
        for (sa, &seg_a) in segs_a.iter().enumerate() {
            for (cb, segs_b) in all_segs.iter().enumerate().skip(ca) {
                let sb_start = if cb == ca { sa + 1 } else { 0 };
                for (sb, &seg_b) in segs_b.iter().enumerate().skip(sb_start) {
                    let raw_hits = intersect_pair(seg_a, seg_b);
                    let mut pair_hits: Vec<(f64, f64, FontPoint)> = Vec::new();
                    for (ta, tb) in raw_hits {
                        if !(ta > EXTRA_NODE_INTERIOR_T_MIN
                            && ta < EXTRA_NODE_INTERIOR_T_MAX
                            && tb > EXTRA_NODE_INTERIOR_T_MIN
                            && tb < EXTRA_NODE_INTERIOR_T_MAX)
                        {
                            continue;
                        }
                        let pa = seg_a.eval(ta);
                        let pb = seg_b.eval(tb);
                        let pos = FontPoint::new(0.5 * (pa.x.0 + pb.x.0), 0.5 * (pa.y.0 + pb.y.0));
                        let is_dup = pair_hits.iter().any(|(ea, eb, ep)| {
                            ((ta - *ea).abs() <= EXTRA_NODE_DEDUP_T_EPS
                                && (tb - *eb).abs() <= EXTRA_NODE_DEDUP_T_EPS)
                                || pos.distance_to(*ep) <= EXTRA_NODE_DEDUP_DIST_UPM
                        });
                        if !is_dup {
                            pair_hits.push((ta, tb, pos));
                        }
                    }
                    pair_hits.sort_by(|x, y| x.0.total_cmp(&y.0).then_with(|| x.1.total_cmp(&y.1)));
                    for (rank, (t_a, t_b, position)) in pair_hits.into_iter().enumerate() {
                        results.push(VirtualExtraNode {
                            key: ExtraNodeKey {
                                contour_a: ca,
                                seg_a: sa,
                                contour_b: cb,
                                seg_b: sb,
                                crossing_rank: rank,
                            },
                            position,
                            t_a,
                            t_b,
                        });
                    }
                }
            }
        }
    }
    results
}

#[must_use]
pub fn hit_test_extra_nodes(
    extra_nodes: &[VirtualExtraNode],
    viewport: &ViewportTransform,
    cursor_scr: ScreenPoint,
) -> Option<ExtraNodeKey> {
    debug_assert!(viewport.zoom.is_finite());
    let mut best: Option<(f32, ExtraNodeKey)> = None;
    for en in extra_nodes {
        let scr = viewport.to_screen(en.position);
        let d = scr.distance_to(cursor_scr);
        if d <= EXTRA_NODE_HIT_RADIUS_DP && best.is_none_or(|(bd, _)| d < bd) {
            best = Some((d, en.key));
        }
    }
    best.map(|(_, key)| key)
}
