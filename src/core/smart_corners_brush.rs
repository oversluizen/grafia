//! `SPEC-012 §2.1 & §2.2`: Non-Destructive Smart Corners (`radius_upm > 0.0`),
//! Inward Angle-Bisector Ink Traps (`radius_upm < 0.0`), and
//! W3C Pointer Events Level 3 Pressure & Tilt Calligraphic Brush (`fit_calligraphic_brush_stroke`).

use crate::core::bezier_ops::{Node, NodeId, NodeType};
use crate::core::coordinates::FontPoint;
use crate::core::glyph_model::{ContourId, GlyphContour};
use crate::core::math_consts::{
    BRUSH_CATMULL_TANGENT_SIXTH, DEG_TO_RAD_F64, INK_TRAP_LIP_DEPTH_FRAC, INK_TRAP_LIP_EDGE_FRAC,
    ONE_THIRD_F64, PI_F64, SMART_CORNER_KAPPA_FOUR_THIRDS, SMART_CORNER_MAX_EDGE_FRAC,
    SMART_CORNER_PIN_BASE_OFFSET_UPM, SMART_CORNER_QUARTER_ANGLE_SCALE, SOLVER_DET_EPSILON,
};
use crate::core::path_ops::alloc_node;
use serde::{Deserialize, Serialize};

/// Non-destructive Smart Corner (`radius_upm > 0.0`) or Ink Trap (`radius_upm < 0.0`) descriptor (`SPEC-012 §2.1`).
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct SmartCornerSpec {
    pub node_id: NodeId,
    pub radius_upm: f64,
}

/// On-canvas corner-radius pin projected along the interior angle bisector of a `Corner` node (`SPEC-017 §2.7`).
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct SmartCornerRadiusPin {
    pub contour_id: ContourId,
    pub node_id: NodeId,
    pub vertex_pt: FontPoint,
    pub bisector_unit: (f64, f64),
    pub half_angle_rad: f64,
    pub max_radius_upm: f64,
    pub radius_upm: f64,
    pub pin_pt: FontPoint,
}

/// Broad-nib / pressure-sensitive calligraphic brush configuration (`SPEC-012 §2.2`).
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct CalligraphicNibSpec {
    pub min_width_upm: f64,
    pub max_width_upm: f64,
    pub base_angle_deg: f64,
    pub tilt_influence: f64,
}

/// W3C Pointer Events Level 3 stylus sample (`pressure: 0.0..1.0`, `tilt_x`, `tilt_y`, `SPEC-012 §2.2`).
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct CalligraphicSample {
    pub pos: FontPoint,
    pub pressure: f64,
    pub tilt_x: f64,
    pub tilt_y: f64,
}

fn expand_single_corner_or_trap(
    v_prev: FontPoint,
    v_node: &Node,
    v_next: FontPoint,
    radius_upm: f64,
    next_node_id: &mut u64,
) -> Vec<Node> {
    let v = v_node.point;
    let (dx_in, dy_in) = (v_prev.x.0 - v.x.0, v_prev.y.0 - v.y.0);
    let (dx_out, dy_out) = (v_next.x.0 - v.x.0, v_next.y.0 - v.y.0);
    let l_in = dx_in.hypot(dy_in);
    let l_out = dx_out.hypot(dy_out);
    if l_in <= SOLVER_DET_EPSILON
        || l_out <= SOLVER_DET_EPSILON
        || !radius_upm.is_finite()
        || radius_upm.abs() <= SOLVER_DET_EPSILON
    {
        return vec![v_node.clone()];
    }
    let u_in = (dx_in / l_in, dy_in / l_in);
    let u_out = (dx_out / l_out, dy_out / l_out);
    let dot = (u_in.0 * u_out.0 + u_in.1 * u_out.1).clamp(-1.0, 1.0);
    let theta = dot.acos();
    if theta <= SOLVER_DET_EPSILON || (PI_F64 - theta) <= SOLVER_DET_EPSILON {
        return vec![v_node.clone()];
    }

    if radius_upm > 0.0 {
        let half_tan = (0.5 * theta).tan().max(SOLVER_DET_EPSILON);
        let d_trim = (radius_upm / half_tan)
            .min(SMART_CORNER_MAX_EDGE_FRAC * l_in)
            .min(SMART_CORNER_MAX_EDGE_FRAC * l_out);
        let kappa_arc = SMART_CORNER_KAPPA_FOUR_THIRDS
            * (SMART_CORNER_QUARTER_ANGLE_SCALE * (PI_F64 - theta)).tan();
        let t_in = FontPoint::new(v.x.0 + d_trim * u_in.0, v.y.0 + d_trim * u_in.1);
        let t_out = FontPoint::new(v.x.0 + d_trim * u_out.0, v.y.0 + d_trim * u_out.1);
        let h_in = FontPoint::new(
            t_in.x.0 - kappa_arc * d_trim * u_in.0,
            t_in.y.0 - kappa_arc * d_trim * u_in.1,
        );
        let h_out = FontPoint::new(
            t_out.x.0 - kappa_arc * d_trim * u_out.0,
            t_out.y.0 - kappa_arc * d_trim * u_out.1,
        );
        vec![
            alloc_node(next_node_id, t_in, NodeType::Smooth),
            alloc_node(next_node_id, h_in, NodeType::OffCurve),
            alloc_node(next_node_id, h_out, NodeType::OffCurve),
            alloc_node(next_node_id, t_out, NodeType::Smooth),
        ]
    } else {
        let depth = (-radius_upm).min(SMART_CORNER_MAX_EDGE_FRAC * l_in.min(l_out));
        let (bx, by) = (u_in.0 + u_out.0, u_in.1 + u_out.1);
        let b_len = bx.hypot(by);
        if b_len <= SOLVER_DET_EPSILON {
            return vec![v_node.clone()];
        }
        let b_hat = (bx / b_len, by / b_len);
        let d_lip = (INK_TRAP_LIP_DEPTH_FRAC * depth).min(INK_TRAP_LIP_EDGE_FRAC * l_in.min(l_out));
        let t_in = FontPoint::new(v.x.0 + d_lip * u_in.0, v.y.0 + d_lip * u_in.1);
        let t_out = FontPoint::new(v.x.0 + d_lip * u_out.0, v.y.0 + d_lip * u_out.1);
        let n_in = FontPoint::new(t_in.x.0 + depth * b_hat.0, t_in.y.0 + depth * b_hat.1);
        let n_out = FontPoint::new(t_out.x.0 + depth * b_hat.0, t_out.y.0 + depth * b_hat.1);
        vec![
            alloc_node(next_node_id, t_in, NodeType::Corner),
            alloc_node(next_node_id, n_in, NodeType::Corner),
            alloc_node(next_node_id, n_out, NodeType::Corner),
            alloc_node(next_node_id, t_out, NodeType::Corner),
        ]
    }
}

/// Evaluates all `SmartCornerSpec` fillets and ink-trap notches across `contours` (`SPEC-012 §2.1`).
#[must_use]
pub fn evaluate_smart_corner_contours(
    contours: &[GlyphContour],
    specs: &[SmartCornerSpec],
    next_node_id: &mut u64,
) -> Vec<GlyphContour> {
    debug_assert!(*next_node_id >= 1);
    if specs.is_empty() {
        return contours.to_vec();
    }
    let mut out = Vec::with_capacity(contours.len());
    for contour in contours {
        let n = contour.nodes.len();
        if n < 3 {
            out.push(contour.clone());
            continue;
        }
        let mut rebuilt = Vec::with_capacity(n + specs.len() * 3);
        for i in 0..n {
            let node = &contour.nodes[i];
            let is_endpoint = !contour.closed && (i == 0 || i + 1 == n);
            let spec_opt = if node.node_type != NodeType::OffCurve && !is_endpoint {
                specs.iter().find(|s| s.node_id == node.id).copied()
            } else {
                None
            };
            if let Some(spec) = spec_opt {
                let v_prev = (1..n)
                    .map(|step| contour.nodes[(i + n - step) % n].point)
                    .find(|pt| pt.distance_to(node.point) > SOLVER_DET_EPSILON)
                    .unwrap_or(contour.nodes[(i + n - 1) % n].point);
                let v_next = (1..n)
                    .map(|step| contour.nodes[(i + step) % n].point)
                    .find(|pt| pt.distance_to(node.point) > SOLVER_DET_EPSILON)
                    .unwrap_or(contour.nodes[(i + 1) % n].point);
                let expanded = expand_single_corner_or_trap(
                    v_prev,
                    node,
                    v_next,
                    spec.radius_upm,
                    next_node_id,
                );
                rebuilt.extend(expanded);
            } else {
                rebuilt.push(node.clone());
            }
        }
        let mut gc = contour.clone();
        gc.nodes = rebuilt;
        out.push(gc);
    }
    out
}

/// Computes on-canvas `SmartCornerRadiusPin` handles along the interior angle bisector of selected or active `Corner` nodes (`SPEC-017 §2.7`).
#[must_use]
pub fn compute_smart_corner_radius_pins(
    contours: &[GlyphContour],
    selected_nodes: &[NodeId],
    specs: &[SmartCornerSpec],
) -> Vec<SmartCornerRadiusPin> {
    let mut pins = Vec::new();
    for contour in contours {
        if !contour.visible || contour.nodes.len() < 3 {
            continue;
        }
        let n = contour.nodes.len();
        for i in 0..n {
            let node = &contour.nodes[i];
            let is_endpoint = !contour.closed && (i == 0 || i + 1 == n);
            if node.node_type != NodeType::Corner || is_endpoint {
                continue;
            }
            let spec_r = specs
                .iter()
                .find(|s| s.node_id == node.id)
                .map(|s| s.radius_upm);
            if spec_r.is_none() && !selected_nodes.contains(&node.id) {
                continue;
            }
            let v = node.point;
            let v_prev = (1..n)
                .map(|step| contour.nodes[(i + n - step) % n].point)
                .find(|pt| pt.distance_to(v) > SOLVER_DET_EPSILON)
                .unwrap_or(contour.nodes[(i + n - 1) % n].point);
            let v_next = (1..n)
                .map(|step| contour.nodes[(i + step) % n].point)
                .find(|pt| pt.distance_to(v) > SOLVER_DET_EPSILON)
                .unwrap_or(contour.nodes[(i + 1) % n].point);
            let (dx_in, dy_in) = (v_prev.x.0 - v.x.0, v_prev.y.0 - v.y.0);
            let (dx_out, dy_out) = (v_next.x.0 - v.x.0, v_next.y.0 - v.y.0);
            let l_in = dx_in.hypot(dy_in);
            let l_out = dx_out.hypot(dy_out);
            if l_in <= SOLVER_DET_EPSILON || l_out <= SOLVER_DET_EPSILON {
                continue;
            }
            let u_in = (dx_in / l_in, dy_in / l_in);
            let u_out = (dx_out / l_out, dy_out / l_out);
            let (bx, by) = (u_in.0 + u_out.0, u_in.1 + u_out.1);
            let b_len = bx.hypot(by);
            if b_len <= SOLVER_DET_EPSILON {
                continue;
            }
            let bisector_unit = (bx / b_len, by / b_len);
            let dot = (u_in.0 * u_out.0 + u_in.1 * u_out.1).clamp(-1.0, 1.0);
            let half_angle_rad = 0.5 * dot.acos();
            let half_tan = half_angle_rad.tan().max(SOLVER_DET_EPSILON);
            let max_edge = SMART_CORNER_MAX_EDGE_FRAC * l_in.min(l_out);
            let max_radius_upm = (max_edge * half_tan).max(0.0);
            let radius_upm = spec_r
                .filter(|r| r.is_finite())
                .unwrap_or(0.0)
                .clamp(0.0, max_radius_upm);
            let offset = SMART_CORNER_PIN_BASE_OFFSET_UPM + radius_upm;
            let pin_pt = FontPoint::new(
                v.x.0 + bisector_unit.0 * offset,
                v.y.0 + bisector_unit.1 * offset,
            );
            pins.push(SmartCornerRadiusPin {
                contour_id: contour.id,
                node_id: node.id,
                vertex_pt: v,
                bisector_unit,
                half_angle_rad,
                max_radius_upm,
                radius_upm,
                pin_pt,
            });
        }
    }
    pins
}

/// Hit-tests `SmartCornerRadiusPin` handles in UPM space within `hit_radius_upm` (`SPEC-017 §2.7`).
#[must_use]
pub fn hit_test_smart_corner_radius_pin(
    pins: &[SmartCornerRadiusPin],
    pt_upm: FontPoint,
    hit_radius_upm: f64,
) -> Option<NodeId> {
    if !pt_upm.x.0.is_finite()
        || !pt_upm.y.0.is_finite()
        || !hit_radius_upm.is_finite()
        || hit_radius_upm <= 0.0
    {
        return None;
    }
    let mut best: Option<(NodeId, f64)> = None;
    for pin in pins {
        let d = pin.pin_pt.distance_to(pt_upm);
        if d <= hit_radius_upm
            && d <= pin.vertex_pt.distance_to(pt_upm)
            && best.is_none_or(|(_, bd)| d < bd)
        {
            best = Some((pin.node_id, d));
        }
    }
    best.map(|(id, _)| id)
}

/// Converts a stream of W3C stylus samples into a closed cubic Bézier calligraphic contour (`SPEC-012 §2.2`).
#[must_use]
pub fn fit_calligraphic_brush_stroke(
    nib: CalligraphicNibSpec,
    samples: &[CalligraphicSample],
) -> Option<GlyphContour> {
    if samples.len() < 2
        || !nib.min_width_upm.is_finite()
        || !nib.max_width_upm.is_finite()
        || !nib.base_angle_deg.is_finite()
        || !nib.tilt_influence.is_finite()
    {
        return None;
    }
    let mut clean: Vec<CalligraphicSample> = Vec::with_capacity(samples.len());
    for s in samples {
        if !s.pos.x.0.is_finite()
            || !s.pos.y.0.is_finite()
            || !s.pressure.is_finite()
            || !s.tilt_x.is_finite()
            || !s.tilt_y.is_finite()
        {
            return None;
        }
        if clean
            .last()
            .is_none_or(|prev| prev.pos.distance_to(s.pos) > SOLVER_DET_EPSILON)
        {
            clean.push(*s);
        }
    }
    if clean.len() < 2 {
        return None;
    }

    let w_min = nib.min_width_upm.max(0.0);
    let w_max = nib.max_width_upm.max(w_min);
    let tilt_k = nib.tilt_influence.clamp(0.0, 1.0);
    let base_rad = nib.base_angle_deg * DEG_TO_RAD_F64;
    let mut left_rail = Vec::with_capacity(clean.len());
    let mut right_rail = Vec::with_capacity(clean.len());
    let mut prev_dir: Option<(f64, f64)> = None;

    for s in &clean {
        let p = s.pressure.clamp(0.0, 1.0);
        let h = 0.5 * (w_min + p * (w_max - w_min));
        let tilt_rad =
            if s.tilt_x.abs() <= SOLVER_DET_EPSILON && s.tilt_y.abs() <= SOLVER_DET_EPSILON {
                0.0
            } else {
                s.tilt_y.atan2(s.tilt_x)
            };
        let phi = base_rad + tilt_k * tilt_rad;
        let (mut ux, mut uy) = (phi.cos(), phi.sin());
        if let Some((px, py)) = prev_dir {
            if ux * px + uy * py < 0.0 {
                ux = -ux;
                uy = -uy;
            }
        }
        prev_dir = Some((ux, uy));
        left_rail.push(FontPoint::new(s.pos.x.0 + h * ux, s.pos.y.0 + h * uy));
        right_rail.push(FontPoint::new(s.pos.x.0 - h * ux, s.pos.y.0 - h * uy));
    }

    let mut next_id = 1_u64;
    let mut nodes = Vec::with_capacity(clean.len() * 6);
    append_cubic_rail_nodes(&left_rail, &mut next_id, &mut nodes);
    right_rail.reverse();
    append_cubic_rail_nodes(&right_rail, &mut next_id, &mut nodes);
    debug_assert!(nodes.len() >= 6);
    Some(GlyphContour::new(ContourId(0), nodes))
}

fn append_cubic_rail_nodes(rail: &[FontPoint], next_id: &mut u64, out: &mut Vec<Node>) {
    let m = rail.len();
    if m == 0 {
        return;
    }
    out.push(alloc_node(next_id, rail[0], NodeType::Corner));
    for k in 0..m.saturating_sub(1) {
        let a = rail[k];
        let b = rail[k + 1];
        let h1 = if k > 0 {
            let prev = rail[k - 1];
            FontPoint::new(
                a.x.0 + BRUSH_CATMULL_TANGENT_SIXTH * (b.x.0 - prev.x.0),
                a.y.0 + BRUSH_CATMULL_TANGENT_SIXTH * (b.y.0 - prev.y.0),
            )
        } else {
            FontPoint::new(
                a.x.0 + ONE_THIRD_F64 * (b.x.0 - a.x.0),
                a.y.0 + ONE_THIRD_F64 * (b.y.0 - a.y.0),
            )
        };
        let h2 = if k + 2 < m {
            let next = rail[k + 2];
            FontPoint::new(
                b.x.0 - BRUSH_CATMULL_TANGENT_SIXTH * (next.x.0 - a.x.0),
                b.y.0 - BRUSH_CATMULL_TANGENT_SIXTH * (next.y.0 - a.y.0),
            )
        } else {
            FontPoint::new(
                b.x.0 - ONE_THIRD_F64 * (b.x.0 - a.x.0),
                b.y.0 - ONE_THIRD_F64 * (b.y.0 - a.y.0),
            )
        };
        let end_ty = if k + 1 < m - 1 {
            NodeType::Smooth
        } else {
            NodeType::Corner
        };
        out.push(alloc_node(next_id, h1, NodeType::OffCurve));
        out.push(alloc_node(next_id, h2, NodeType::OffCurve));
        out.push(alloc_node(next_id, b, end_ty));
    }
}
