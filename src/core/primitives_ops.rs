//! `CanvasTool` (`F` / `Shift+F`), `PenTool` (`PencilTool` `B`), and `PixelTool` (`X`)
//! geometric construction & curve-fitting operations (`SPEC-005 §2.2, §2.5, §2.6`).
//!
//! Extracted 1-to-1 from `CanvasTool`, `PenTool`, and `PixelTool`
//! in Reference Vector (``).

use crate::core::{FontPoint, Node, NodeId, NodeType};
use crate::tokens::{
    PENCIL_RDP_EPSILON_UPM, PENCIL_TANGENT_HANDLE_FACTOR, PIXEL_CELL_MATCH_TOL_UPM,
    PRIMITIVE_CIRCLE_KAPPA,
};
use serde::{Deserialize, Serialize};

/// Sub-modes of `PrimitivesTool` (`F` / `Shift+F`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum PrimitiveSubTool {
    /// `CanvasTool` (`subGroupID = 10`).
    #[default]
    Rectangle,
    /// `CanvasTool` (`subGroupID = 20`).
    Circle,
}

impl PrimitiveSubTool {
    /// Cycles between `Rectangle` and `Circle` (`Shift+F`).
    #[must_use]
    pub const fn cycle_next(self) -> Self {
        match self {
            Self::Rectangle => Self::Circle,
            Self::Circle => Self::Rectangle,
        }
    }
}

/// Dialog request emitted when a tool is single-clicked on the canvas without dragging
/// (`CanvasTool::makePrimitiveWithDialog:` at `spec_ref`).
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub enum ToolDialogRequest {
    Primitive {
        position_upm: FontPoint,
        sub_tool: PrimitiveSubTool,
        default_width: f64,
        default_height: f64,
    },
}

/// Computes the bounding rectangle `(min_x, min_y, max_x, max_y)` for `PrimitivesTool`
/// (`CanvasTool::rectFromOrigin:drag:modifier:` at `spec_ref`).
/// - `shift`: constrains `|dx| = |dy| = max(|dx|, |dy|)` (square / circle).
/// - `alt`: expands symmetrically around `start` as center (`origin = start - |d|`, `size = 2 * |d|`).
#[must_use]
pub fn compute_primitive_rect_upm(
    start: FontPoint,
    current: FontPoint,
    shift: bool,
    alt: bool,
    grid_step: f64,
    round_to_grid: bool,
) -> (f64, f64, f64, f64) {
    let mut dx = current.x.0 - start.x.0;
    let mut dy = current.y.0 - start.y.0;
    if shift {
        let side = dx.abs().max(dy.abs());
        let sx = if dx < 0.0 { -1.0 } else { 1.0 };
        let sy = if dy < 0.0 { -1.0 } else { 1.0 };
        dx = sx * side;
        dy = sy * side;
    }
    let (mut min_x, mut min_y, mut max_x, mut max_y) = if alt {
        let ax = dx.abs();
        let ay = dy.abs();
        (
            start.x.0 - ax,
            start.y.0 - ay,
            start.x.0 + ax,
            start.y.0 + ay,
        )
    } else {
        let x1 = start.x.0 + dx;
        let y1 = start.y.0 + dy;
        (
            start.x.0.min(x1),
            start.y.0.min(y1),
            start.x.0.max(x1),
            start.y.0.max(y1),
        )
    };
    if round_to_grid && grid_step > 0.0 {
        min_x = (min_x / grid_step).round() * grid_step;
        min_y = (min_y / grid_step).round() * grid_step;
        max_x = (max_x / grid_step).round() * grid_step;
        max_y = (max_y / grid_step).round() * grid_step;
    }
    (min_x, min_y, max_x, max_y)
}

/// Builds a 4-node counter-clockwise `Corner` rectangle contour (`BezierPath::pathWithRect:` at `spec_ref`).
#[must_use]
pub fn build_rectangle_contour_nodes(
    min_x: f64,
    min_y: f64,
    max_x: f64,
    max_y: f64,
    next_node_id: &mut u64,
) -> Vec<Node> {
    let coords = [
        (min_x, min_y),
        (max_x, min_y),
        (max_x, max_y),
        (min_x, max_y),
    ];
    coords
        .into_iter()
        .map(|(x, y)| {
            let id = NodeId(*next_node_id);
            *next_node_id += 1;
            Node::new(id, x, y, NodeType::Corner)
        })
        .collect()
}

/// Builds a 12-node cubic Bézier circle/oval contour (`BezierPath::circleWithRect:` at `spec_ref`):
/// 4 `Smooth` extreme nodes + 8 `OffCurve` handles with $\kappa = 0.55228475$.
#[must_use]
pub fn build_circle_contour_nodes(
    min_x: f64,
    min_y: f64,
    max_x: f64,
    max_y: f64,
    next_node_id: &mut u64,
) -> Vec<Node> {
    let rx = (max_x - min_x) * 0.5;
    let ry = (max_y - min_y) * 0.5;
    let cx = min_x + rx;
    let cy = min_y + ry;
    let kx = rx * PRIMITIVE_CIRCLE_KAPPA;
    let ky = ry * PRIMITIVE_CIRCLE_KAPPA;

    let spec = [
        (cx, min_y, NodeType::Smooth),
        (cx + kx, min_y, NodeType::OffCurve),
        (max_x, cy - ky, NodeType::OffCurve),
        (max_x, cy, NodeType::Smooth),
        (max_x, cy + ky, NodeType::OffCurve),
        (cx + kx, max_y, NodeType::OffCurve),
        (cx, max_y, NodeType::Smooth),
        (cx - kx, max_y, NodeType::OffCurve),
        (min_x, cy + ky, NodeType::OffCurve),
        (min_x, cy, NodeType::Smooth),
        (min_x, cy - ky, NodeType::OffCurve),
        (cx - kx, min_y, NodeType::OffCurve),
    ];
    spec.into_iter()
        .map(|(x, y, ntype)| {
            let id = NodeId(*next_node_id);
            *next_node_id += 1;
            Node::new(id, x, y, ntype)
        })
        .collect()
}

/// Simplifies a polyline using Ramer-Douglas-Peucker (`FreehandFitter::fitCurve`).
#[must_use]
pub fn simplify_polyline_rdp(points: &[FontPoint], epsilon: f64) -> Vec<FontPoint> {
    if points.len() <= 2 {
        return points.to_vec();
    }
    let first = points[0];
    let last = points[points.len() - 1];
    let dx = last.x.0 - first.x.0;
    let dy = last.y.0 - first.y.0;
    let len_sq = dx * dx + dy * dy;

    let mut max_dist = 0.0;
    let mut split_idx = 0;
    for (i, pt) in points.iter().enumerate().take(points.len() - 1).skip(1) {
        let dist = if len_sq <= 1.0 {
            (pt.x.0 - first.x.0).hypot(pt.y.0 - first.y.0)
        } else {
            let t = (((pt.x.0 - first.x.0) * dx + (pt.y.0 - first.y.0) * dy) / len_sq).clamp(0.0, 1.0);
            let proj_x = first.x.0 + t * dx;
            let proj_y = first.y.0 + t * dy;
            (pt.x.0 - proj_x).hypot(pt.y.0 - proj_y)
        };
        if dist > max_dist {
            max_dist = dist;
            split_idx = i;
        }
    }

    if max_dist > epsilon && split_idx > 0 {
        let mut left = simplify_polyline_rdp(&points[..=split_idx], epsilon);
        let right = simplify_polyline_rdp(&points[split_idx..], epsilon);
        left.pop();
        left.extend(right);
        left
    } else {
        vec![first, last]
    }
}

/// Fits a smooth cubic Bézier contour (`OnCurve` + `OffCurve` handles) to a freehand polyline (`PencilTool` `B`).
#[must_use]
pub fn fit_freehand_contour_nodes(
    raw_points: &[FontPoint],
    closed: bool,
    next_node_id: &mut u64,
) -> Vec<Node> {
    if raw_points.len() < 2 {
        return Vec::new();
    }
    let mut knots = simplify_polyline_rdp(raw_points, PENCIL_RDP_EPSILON_UPM);
    if closed && knots.len() >= 3 {
        let first = knots[0];
        let last = *knots.last().unwrap();
        if (first.x.0 - last.x.0).hypot(first.y.0 - last.y.0) <= PENCIL_RDP_EPSILON_UPM * 2.0 {
            knots.pop();
        }
    }
    if knots.len() < 2 {
        knots = vec![raw_points[0], *raw_points.last().unwrap()];
    }

    let n = knots.len();
    let tangent_at = |idx: usize| -> (f64, f64) {
        if closed {
            let prev = knots[(idx + n - 1) % n];
            let next = knots[(idx + 1) % n];
            ((next.x.0 - prev.x.0) * 0.5, (next.y.0 - prev.y.0) * 0.5)
        } else if idx == 0 {
            (knots[1].x.0 - knots[0].x.0, knots[1].y.0 - knots[0].y.0)
        } else if idx + 1 == n {
            (knots[n - 1].x.0 - knots[n - 2].x.0, knots[n - 1].y.0 - knots[n - 2].y.0)
        } else {
            (
                (knots[idx + 1].x.0 - knots[idx - 1].x.0) * 0.5,
                (knots[idx + 1].y.0 - knots[idx - 1].y.0) * 0.5,
            )
        }
    };

    let mut nodes = Vec::new();
    let seg_count = if closed { n } else { n - 1 };
    for i in 0..n {
        let ntype = if !closed && (i == 0 || i + 1 == n) {
            NodeType::Corner
        } else {
            NodeType::Smooth
        };
        let id_on = NodeId(*next_node_id);
        *next_node_id += 1;
        nodes.push(Node::new(id_on, knots[i].x.0, knots[i].y.0, ntype));

        if i < seg_count {
            let j = (i + 1) % n;
            let (tx0, ty0) = tangent_at(i);
            let (tx1, ty1) = tangent_at(j);
            let h1_x = knots[i].x.0 + tx0 * PENCIL_TANGENT_HANDLE_FACTOR;
            let h1_y = knots[i].y.0 + ty0 * PENCIL_TANGENT_HANDLE_FACTOR;
            let h2_x = knots[j].x.0 - tx1 * PENCIL_TANGENT_HANDLE_FACTOR;
            let h2_y = knots[j].y.0 - ty1 * PENCIL_TANGENT_HANDLE_FACTOR;

            let id_h1 = NodeId(*next_node_id);
            *next_node_id += 1;
            nodes.push(Node::new(id_h1, h1_x, h1_y, NodeType::OffCurve));
            let id_h2 = NodeId(*next_node_id);
            *next_node_id += 1;
            nodes.push(Node::new(id_h2, h2_x, h2_y, NodeType::OffCurve));
        }
    }
    nodes
}

/// Snaps a font coordinate to integer pixel cell indices `(col, row)` (`PixelTool` at `spec_ref`).
#[must_use]
pub fn snap_to_pixel_cell(pt: FontPoint, step: f64) -> (i64, i64) {
    let s = step.max(1.0);
    ((pt.x.0 / s).floor() as i64, (pt.y.0 / s).floor() as i64)
}

/// Returns `(min_x, min_y, max_x, max_y)` in UPM for a pixel cell `(col, row)`.
#[must_use]
pub fn pixel_cell_bounds_upm(cell: (i64, i64), step: f64) -> (f64, f64, f64, f64) {
    let x0 = cell.0 as f64 * step;
    let y0 = cell.1 as f64 * step;
    (x0, y0, x0 + step, y0 + step)
}

/// Checks if an axis-aligned `step × step` pixel contour already exists at `cell`.
#[must_use]
pub fn find_pixel_cell_contour_index(
    contours: &[Vec<Node>],
    closed_flags: &[bool],
    cell: (i64, i64),
    step: f64,
) -> Option<usize> {
    let (min_x, min_y, max_x, max_y) = pixel_cell_bounds_upm(cell, step);
    for (idx, nodes) in contours.iter().enumerate() {
        if !closed_flags.get(idx).copied().unwrap_or(true) || nodes.len() != 4 {
            continue;
        }
        if nodes.iter().any(|n| n.node_type == NodeType::OffCurve) {
            continue;
        }
        let mut xs = [nodes[0].point.x.0, nodes[1].point.x.0, nodes[2].point.x.0, nodes[3].point.x.0];
        let mut ys = [nodes[0].point.y.0, nodes[1].point.y.0, nodes[2].point.y.0, nodes[3].point.y.0];
        xs.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
        ys.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
        if (xs[0] - min_x).abs() <= PIXEL_CELL_MATCH_TOL_UPM
            && (xs[3] - max_x).abs() <= PIXEL_CELL_MATCH_TOL_UPM
            && (ys[0] - min_y).abs() <= PIXEL_CELL_MATCH_TOL_UPM
            && (ys[3] - max_y).abs() <= PIXEL_CELL_MATCH_TOL_UPM
        {
            return Some(idx);
        }
    }
    None
}

/// Computes a 2D Bresenham line of pixel cells between `start` and `end` (`Shift` click in `PixelTool`).
#[must_use]
pub fn bresenham_pixel_cells(start: (i64, i64), end: (i64, i64)) -> Vec<(i64, i64)> {
    let mut cells = Vec::new();
    let mut x0 = start.0;
    let mut y0 = start.1;
    let x1 = end.0;
    let y1 = end.1;
    let dx = (x1 - x0).abs();
    let sx = if x0 < x1 { 1 } else { -1 };
    let dy = -(y1 - y0).abs();
    let sy = if y0 < y1 { 1 } else { -1 };
    let mut err = dx + dy;
    loop {
        cells.push((x0, y0));
        if x0 == x1 && y0 == y1 {
            break;
        }
        let e2 = err * 2;
        if e2 >= dy {
            err += dy;
            x0 += sx;
        }
        if e2 <= dx {
            err += dx;
            y0 += sy;
        }
    }
    cells
}
