//! Edit canvas geometry projection for nodes, handles, and contours.

use crate::core::coordinates::{FontPoint, ScreenPoint, ViewportTransform};
use crate::core::{extract_segments, ContourSegment, Node, NodeType};
use crate::tokens::{
    COLLINEAR_MIN_LEN_EPSILON, EditCanvasTokens, OPEN_ENDPOINT_TICK_HALF_LEN_DP,
    START_NODE_ARROW_HALF_WIDTH_DP, START_NODE_ARROW_LEN_DP, START_NODE_ARROW_OFFSET_DP,
};
use kurbo::Rect;

#[derive(Debug, Clone, PartialEq)]
pub struct ProjectedNode {
    pub screen_pos: ScreenPoint,
    pub bounds: Rect,
    pub node_type: NodeType,
    pub is_selected: bool,
}

impl ProjectedNode {
    #[must_use]
    pub fn new(font_pt: FontPoint, node_type: NodeType, is_selected: bool, vp: &ViewportTransform, tokens: &EditCanvasTokens) -> Self {
        let screen_pos = vp.to_screen(font_pt);
        let half = (tokens.node_size_dp * 0.5) as f64;
        let bounds = Rect::new(
            screen_pos.x.0 as f64 - half,
            screen_pos.y.0 as f64 - half,
            screen_pos.x.0 as f64 + half,
            screen_pos.y.0 as f64 + half,
        );

        Self {
            screen_pos,
            bounds,
            node_type,
            is_selected,
        }
    }

    #[must_use]
    pub fn contains_screen_pt(&self, pt: ScreenPoint) -> bool {
        self.bounds.contains(kurbo::Point::new(pt.x.0 as f64, pt.y.0 as f64))
    }
}

/// Computes the screen-space triangle vertices `[tip, base_left, base_right]` for the
/// start-node direction indicator pointing along the first segment tangent (`SPEC-002 §5.3`).
#[must_use]
pub fn compute_start_node_arrow_triangle(
    contour: &[Node],
    closed: bool,
    vp: &ViewportTransform,
) -> Option<[ScreenPoint; 3]> {
    let segments = extract_segments(contour, closed);
    let (p0, p1) = if let Some(first_seg) = segments.first() {
        match *first_seg {
            ContourSegment::Line { line, .. } => (
                FontPoint::new(line.p0.x, line.p0.y),
                FontPoint::new(line.p1.x, line.p1.y),
            ),
            ContourSegment::Cubic { cubic, .. } => {
                let target = if (cubic.p1 - cubic.p0).hypot() > COLLINEAR_MIN_LEN_EPSILON {
                    cubic.p1
                } else if (cubic.p2 - cubic.p0).hypot() > COLLINEAR_MIN_LEN_EPSILON {
                    cubic.p2
                } else {
                    cubic.p3
                };
                (
                    FontPoint::new(cubic.p0.x, cubic.p0.y),
                    FontPoint::new(target.x, target.y),
                )
            }
        }
    } else if contour.len() >= 2 {
        (contour[0].point, contour[1].point)
    } else {
        return None;
    };

    let s0 = vp.to_screen(p0);
    let s1 = vp.to_screen(p1);
    let dx = s1.x.0 - s0.x.0;
    let dy = s1.y.0 - s0.y.0;
    let len = dx.hypot(dy);
    if len <= COLLINEAR_MIN_LEN_EPSILON as f32 {
        return None;
    }
    let ux = dx / len;
    let uy = dy / len;
    let nx = -uy;
    let ny = ux;

    let base_cx = s0.x.0 + ux * START_NODE_ARROW_OFFSET_DP;
    let base_cy = s0.y.0 + uy * START_NODE_ARROW_OFFSET_DP;
    let tip = ScreenPoint::new(
        base_cx + ux * START_NODE_ARROW_LEN_DP,
        base_cy + uy * START_NODE_ARROW_LEN_DP,
    );
    let base_left = ScreenPoint::new(
        base_cx + nx * START_NODE_ARROW_HALF_WIDTH_DP,
        base_cy + ny * START_NODE_ARROW_HALF_WIDTH_DP,
    );
    let base_right = ScreenPoint::new(
        base_cx - nx * START_NODE_ARROW_HALF_WIDTH_DP,
        base_cy - ny * START_NODE_ARROW_HALF_WIDTH_DP,
    );
    Some([tip, base_left, base_right])
}

/// Computes perpendicular cross-tick segments `(p_a, p_b)` at the start and end nodes
/// of an open contour (`SPEC-002 §5.3`).
#[must_use]
pub fn compute_open_contour_endpoint_ticks(
    contour: &[Node],
    vp: &ViewportTransform,
) -> Vec<(ScreenPoint, ScreenPoint)> {
    let mut ticks = Vec::new();
    if contour.len() < 2 {
        return ticks;
    }
    let pairs = [
        (contour[0].point, contour[1].point),
        (
            contour[contour.len() - 1].point,
            contour[contour.len() - 2].point,
        ),
    ];
    for (anchor, neighbor) in pairs {
        let s0 = vp.to_screen(anchor);
        let s1 = vp.to_screen(neighbor);
        let dx = s1.x.0 - s0.x.0;
        let dy = s1.y.0 - s0.y.0;
        let len = dx.hypot(dy);
        if len > COLLINEAR_MIN_LEN_EPSILON as f32 {
            let nx = -dy / len;
            let ny = dx / len;
            ticks.push((
                ScreenPoint::new(
                    s0.x.0 - nx * OPEN_ENDPOINT_TICK_HALF_LEN_DP,
                    s0.y.0 - ny * OPEN_ENDPOINT_TICK_HALF_LEN_DP,
                ),
                ScreenPoint::new(
                    s0.x.0 + nx * OPEN_ENDPOINT_TICK_HALF_LEN_DP,
                    s0.y.0 + ny * OPEN_ENDPOINT_TICK_HALF_LEN_DP,
                ),
            ));
        }
    }
    ticks
}
