//! Core bezier curve operations, node types, and tangent alignment.

use crate::core::coordinates::FontPoint;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct NodeId(pub u64);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum NodeType {
    Corner,
    Smooth,
    OffCurve,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Node {
    pub id: NodeId,
    pub point: FontPoint,
    pub node_type: NodeType,
}

impl Node {
    #[must_use]
    pub fn new(id: NodeId, x: f64, y: f64, node_type: NodeType) -> Self {
        Self {
            id,
            point: FontPoint::new(x, y),
            node_type,
        }
    }
}

/// Aligns `bcp_in` so that it lies collinear and 180 degrees opposite to `bcp_out` relative to `anchor`.
#[must_use]
pub fn align_collinear_opposite(anchor: FontPoint, bcp_in: FontPoint, bcp_out: FontPoint) -> FontPoint {
    if anchor.distance_to(bcp_out) <= crate::tokens::COLLINEAR_MIN_LEN_EPSILON {
        return bcp_in;
    }
    let angle_out = (bcp_out.y.0 - anchor.y.0).atan2(bcp_out.x.0 - anchor.x.0);
    let opposing_angle = angle_out + std::f64::consts::PI;
    let in_dist = anchor.distance_to(bcp_in);

    let new_x = anchor.x.0 + in_dist * opposing_angle.cos();
    let new_y = anchor.y.0 + in_dist * opposing_angle.sin();
    FontPoint::new(new_x, new_y)
}

/// Snaps a displacement vector (dx, dy) to 0, 45, 90, or 135 degree axes.
#[must_use]
pub fn snap_vector_45_degrees(dx: f64, dy: f64) -> (f64, f64) {
    if dx.abs() < 1e-6 && dy.abs() < 1e-6 {
        return (0.0, 0.0);
    }
    let rad = dy.atan2(dx);
    let deg = rad.to_degrees().rem_euclid(crate::tokens::FULL_CIRCLE_DEG);
    let step = crate::tokens::DEFAULT_ANGLE_SNAP_DEG;
    let snapped_deg = (deg / step).round() * step;
    let len = dx.hypot(dy);
    let snapped_rad = snapped_deg.to_radians();

    let constrained_dx = (len * snapped_rad.cos()).round();
    let constrained_dy = (len * snapped_rad.sin()).round();
    (constrained_dx, constrained_dy)
}

/// Computes the signed polygon area using the Shoelace formula.
/// In font units (Y-up), negative area indicates clockwise winding.
#[must_use]
pub fn compute_signed_area(points: &[FontPoint]) -> f64 {
    if points.len() < 3 {
        return 0.0;
    }
    let mut area = 0.0;
    let n = points.len();
    for i in 0..n {
        let j = (i + 1) % n;
        area += points[i].x.0 * points[j].y.0;
        area -= points[j].x.0 * points[i].y.0;
    }
    area * 0.5
}
