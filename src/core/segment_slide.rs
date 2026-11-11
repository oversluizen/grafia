//! Contour reversal, start-node rotation, and De Casteljau `OnCurve` node sliding along contours (`SPEC-002`).

use crate::core::bezier_ops::{Node, NodeId, NodeType};
use crate::core::coordinates::FontPoint;
use crate::core::curve_manipulation::{extract_segments, ContourSegment};
use crate::core::handle_constraints::fit_cubic_to_segment_chain;
use crate::tokens::{
    COLLINEAR_MIN_LEN_EPSILON, CURVE_SOLVE_ACCURACY, SPLIT_RATIO_MAX, SPLIT_RATIO_MIN,
};
use kurbo::{ParamCurve, ParamCurveNearest, Point};

/// Reverses the winding direction of a contour while keeping the first `OnCurve` node as
/// the starting anchor on closed contours (`SPEC-002 §5.3`).
pub fn reverse_contour_nodes(contour: &mut [Node], closed: bool) {
    if contour.len() <= 1 {
        return;
    }
    if !closed {
        contour.reverse();
        return;
    }
    let first_on_id = contour
        .iter()
        .find(|n| n.node_type != NodeType::OffCurve)
        .map(|n| n.id);
    contour.reverse();
    if let Some(target_id) = first_on_id {
        if let Some(pos) = contour.iter().position(|n| n.id == target_id) {
            contour.rotate_left(pos);
        }
    }
}

/// Rotates a closed contour so that the first selected `OnCurve` node becomes `contour[0]`
/// (`Make Node First` / `Set Start Node`, `SPEC-002 §5.3`).
pub fn make_selected_on_curve_node_first(
    contour: &mut [Node],
    closed: bool,
    selected: &[NodeId],
) -> bool {
    if !closed || contour.len() < 2 {
        return false;
    }
    if let Some(pos) = contour
        .iter()
        .position(|n| selected.contains(&n.id) && n.node_type != NodeType::OffCurve)
    {
        if pos > 0 {
            contour.rotate_left(pos);
            return true;
        }
    }
    false
}

/// Slides a selected `OnCurve` node along its adjacent `Line` or `Cubic` segments while preserving
/// curvature via De Casteljau subdivision (`SPEC-002 §3.1.3`).
pub fn slide_on_curve_node_along_contour(
    contour: &mut [Node],
    closed: bool,
    node_id: NodeId,
    target_pt: FontPoint,
) -> bool {
    let Some(node_idx) = contour.iter().position(|n| n.id == node_id) else {
        return false;
    };
    if contour[node_idx].node_type == NodeType::OffCurve {
        return false;
    }
    let segments = extract_segments(contour, closed);
    let seg_in = segments.iter().copied().find(|s| match s {
        ContourSegment::Line { end_idx, .. } | ContourSegment::Cubic { end_idx, .. } => {
            *end_idx == node_idx
        }
    });
    let seg_out = segments.iter().copied().find(|s| match s {
        ContourSegment::Line { start_idx, .. } | ContourSegment::Cubic { start_idx, .. } => {
            *start_idx == node_idx
        }
    });
    let target = Point::new(target_pt.x.0, target_pt.y.0);
    let eval_seg = |s: ContourSegment| -> (f64, f64) {
        let res = match s {
            ContourSegment::Line { line, .. } => line.nearest(target, CURVE_SOLVE_ACCURACY),
            ContourSegment::Cubic { cubic, .. } => cubic.nearest(target, CURVE_SOLVE_ACCURACY),
        };
        (res.distance_sq.sqrt(), res.t)
    };

    let in_cand = seg_in.map(eval_seg);
    let out_cand = seg_out.map(eval_seg);
    let use_in = match (in_cand, out_cand) {
        (Some((d_in, _)), Some((d_out, _))) => d_in <= d_out,
        (Some(_), None) => true,
        (None, Some(_)) => false,
        (None, None) => return false,
    };

    if use_in {
        let (Some(s_in), Some((_, raw_t))) = (seg_in, in_cand) else {
            return false;
        };
        let u = raw_t.clamp(SPLIT_RATIO_MIN, 1.0);
        match s_in {
            ContourSegment::Line { line, .. } => {
                let p = line.eval(u);
                let old = contour[node_idx].point;
                contour[node_idx].point = FontPoint::new(p.x, p.y);
                if let Some(ContourSegment::Cubic { h1_idx, .. }) = seg_out {
                    contour[h1_idx].point.x.0 += p.x - old.x.0;
                    contour[h1_idx].point.y.0 += p.y - old.y.0;
                }
            }
            ContourSegment::Cubic {
                h1_idx,
                h2_idx,
                cubic,
                ..
            } => {
                let left = cubic.subsegment(0.0..u);
                let rem = cubic.subsegment(u..1.0);
                contour[h1_idx].point = FontPoint::new(left.p1.x, left.p1.y);
                contour[h2_idx].point = FontPoint::new(left.p2.x, left.p2.y);
                contour[node_idx].point = FontPoint::new(left.p3.x, left.p3.y);
                if (1.0 - u) > COLLINEAR_MIN_LEN_EPSILON {
                    if let Some(ContourSegment::Cubic {
                        h1_idx: oh1,
                        h2_idx: oh2,
                        cubic: oc,
                        ..
                    }) = seg_out
                    {
                        let chain = [
                            ContourSegment::Cubic {
                                start_idx: 0,
                                h1_idx: 1,
                                h2_idx: 2,
                                end_idx: 3,
                                cubic: rem,
                            },
                            ContourSegment::Cubic {
                                start_idx: 3,
                                h1_idx: 4,
                                h2_idx: 5,
                                end_idx: 6,
                                cubic: oc,
                            },
                        ];
                        let (fh1, fh2) = fit_cubic_to_segment_chain(&chain);
                        contour[oh1].point = fh1;
                        contour[oh2].point = fh2;
                    }
                }
            }
        }
    } else {
        let (Some(s_out), Some((_, raw_t))) = (seg_out, out_cand) else {
            return false;
        };
        let v = raw_t.clamp(0.0, SPLIT_RATIO_MAX);
        match s_out {
            ContourSegment::Line { line, .. } => {
                let p = line.eval(v);
                let old = contour[node_idx].point;
                contour[node_idx].point = FontPoint::new(p.x, p.y);
                if let Some(ContourSegment::Cubic { h2_idx, .. }) = seg_in {
                    contour[h2_idx].point.x.0 += p.x - old.x.0;
                    contour[h2_idx].point.y.0 += p.y - old.y.0;
                }
            }
            ContourSegment::Cubic {
                h1_idx,
                h2_idx,
                cubic,
                ..
            } => {
                let rem = cubic.subsegment(0.0..v);
                let right = cubic.subsegment(v..1.0);
                contour[node_idx].point = FontPoint::new(right.p0.x, right.p0.y);
                contour[h1_idx].point = FontPoint::new(right.p1.x, right.p1.y);
                contour[h2_idx].point = FontPoint::new(right.p2.x, right.p2.y);
                if v > COLLINEAR_MIN_LEN_EPSILON {
                    if let Some(ContourSegment::Cubic {
                        h1_idx: ih1,
                        h2_idx: ih2,
                        cubic: ic,
                        ..
                    }) = seg_in
                    {
                        let chain = [
                            ContourSegment::Cubic {
                                start_idx: 0,
                                h1_idx: 1,
                                h2_idx: 2,
                                end_idx: 3,
                                cubic: ic,
                            },
                            ContourSegment::Cubic {
                                start_idx: 3,
                                h1_idx: 4,
                                h2_idx: 5,
                                end_idx: 6,
                                cubic: rem,
                            },
                        ];
                        let (fh1, fh2) = fit_cubic_to_segment_chain(&chain);
                        contour[ih1].point = fh1;
                        contour[ih2].point = fh2;
                    }
                }
            }
        }
    }
    true
}
