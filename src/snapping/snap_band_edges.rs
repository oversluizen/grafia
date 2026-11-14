//! `SPEC-009` (`grafia v0.8.0` Part B) extended `SnapEngine` with `GuideThickness::FontUnits`
//! band-edge snapping, `SnapTargetMask` filtering, and custom `snap_radius_dp`.

use crate::core::host_requests::SnapTargetMask;
use crate::core::math_consts::{
    DEG_TO_RAD_F64, MAX_ACTIVE_SNAPPERS, PREF_MIN_RADIUS_DP, RIGHT_ANGLE_DEG_F64,
    SNAP_LINE_INTERSECT_DET_EPS,
};
use crate::core::{
    collect_reference_frame_snap_lines, CustomReferenceFrame, FontPoint, GlyphContour, Node,
    NodeId, NodeType, SnapTargetKind,
};
use crate::snapping::guides::{Guide, GuideThickness};
use crate::snapping::snap_engine::{ActiveSnapper, GlyphMetrics, SnapEngine, SnapResult};

#[derive(Debug, Clone, Copy)]
struct CandidateLineSnapper {
    anchor: FontPoint,
    angle_deg: f64,
    proj: FontPoint,
    dist: f64,
    target_kind: Option<SnapTargetKind>,
}

fn project_onto_angled_line(pt: FontPoint, anchor: FontPoint, angle_deg: f64) -> (FontPoint, f64) {
    let rad = angle_deg * DEG_TO_RAD_F64;
    let (ux, uy) = (rad.cos(), rad.sin());
    let dx = pt.x.0 - anchor.x.0;
    let dy = pt.y.0 - anchor.y.0;
    let t = dx * ux + dy * uy;
    let proj = FontPoint::new(anchor.x.0 + t * ux, anchor.y.0 + t * uy);
    (proj, pt.distance_to(proj))
}

fn intersect_two_infinite_lines(
    a1: FontPoint,
    angle1_deg: f64,
    a2: FontPoint,
    angle2_deg: f64,
) -> Option<FontPoint> {
    let r1 = angle1_deg * DEG_TO_RAD_F64;
    let r2 = angle2_deg * DEG_TO_RAD_F64;
    let (u1x, u1y) = (r1.cos(), r1.sin());
    let (u2x, u2y) = (r2.cos(), r2.sin());
    let det = u1x * (-u2y) - u1y * (-u2x);
    if det.abs() <= SNAP_LINE_INTERSECT_DET_EPS {
        return None;
    }
    let dx = a2.x.0 - a1.x.0;
    let dy = a2.y.0 - a1.y.0;
    let t1 = (dx * (-u2y) - dy * (-u2x)) / det;
    Some(FontPoint::new(a1.x.0 + t1 * u1x, a1.y.0 + t1 * u1y))
}

/// Extended input bundle for preference-aware magnetic snapping (`SPEC-009 §2.6.1 & §2.6.3`).
pub struct SnapQueryConfig<'a> {
    pub contours: &'a [Vec<Node>],
    pub excluded_nodes: &'a [NodeId],
    pub background_contours: &'a [GlyphContour],
    pub show_background: bool,
    pub metrics: &'a GlyphMetrics,
    pub show_metrics: bool,
    pub advance_width: f64,
    pub italic_angle_deg: f64,
    pub show_guides: bool,
    pub local_guides: &'a [Guide],
    pub global_guides: &'a [Guide],
    pub show_grid: bool,
    pub grid_step_upm: f64,
    pub zoom: f32,
    pub snap_radius_dp: f32,
    pub targets: SnapTargetMask,
    pub custom_reference_frame: Option<&'a CustomReferenceFrame>,
    pub ctrl_bypass: bool,
}

struct SnapCollector<'a> {
    query: FontPoint,
    tolerance_upm: f64,
    best_point: Option<(FontPoint, f64)>,
    line_candidates: Vec<CandidateLineSnapper>,
    cfg: &'a SnapQueryConfig<'a>,
    slant_line_angle: f64,
}

impl<'a> SnapCollector<'a> {
    fn push_line(&mut self, anchor: FontPoint, angle_deg: f64) {
        self.push_line_with_kind(anchor, angle_deg, None);
    }

    fn push_line_with_kind(
        &mut self,
        anchor: FontPoint,
        angle_deg: f64,
        target_kind: Option<SnapTargetKind>,
    ) {
        let (proj, dist) = project_onto_angled_line(self.query, anchor, angle_deg);
        if dist <= self.tolerance_upm {
            self.line_candidates.push(CandidateLineSnapper {
                anchor,
                angle_deg,
                proj,
                dist,
                target_kind,
            });
        }
    }

    fn push_point(&mut self, target: FontPoint) {
        let d = self.query.distance_to(target);
        if d <= self.tolerance_upm && self.best_point.is_none_or(|(_, best_d)| d < best_d) {
            self.best_point = Some((target, d));
        }
    }

    fn collect_nodes_and_background(&mut self) {
        if self.cfg.targets.nodes {
            for contour in self.cfg.contours {
                let n = contour.len();
                for (idx, node) in contour.iter().enumerate() {
                    let is_smooth_partner = self.cfg.excluded_nodes.len() == 1
                        && n >= 3
                        && node.node_type == NodeType::OffCurve
                        && ((contour[(idx + n - 1) % n].node_type == NodeType::Smooth
                            && contour[(idx + n - 2) % n].id == self.cfg.excluded_nodes[0])
                            || (contour[(idx + 1) % n].node_type == NodeType::Smooth
                                && contour[(idx + 2) % n].id == self.cfg.excluded_nodes[0]));
                    if self.cfg.excluded_nodes.contains(&node.id) || is_smooth_partner {
                        continue;
                    }
                    self.push_point(node.point);
                    self.push_line(node.point, 0.0);
                    self.push_line(node.point, RIGHT_ANGLE_DEG_F64);
                    if self.cfg.targets.italic_angle && self.cfg.italic_angle_deg.abs() > 0.0 {
                        self.push_line(node.point, self.slant_line_angle);
                    }
                }
            }
        }
        if self.cfg.targets.background && self.cfg.show_background {
            for bg in self.cfg.background_contours {
                if bg.visible {
                    for node in &bg.nodes {
                        self.push_point(node.point);
                    }
                }
            }
        }
    }

    fn collect_metrics_guides_and_grid(&mut self, radius_dp: f32) {
        if self.cfg.targets.metrics && self.cfg.show_metrics {
            for y_m in self.cfg.metrics.horizontal_metric_lines() {
                self.push_line(FontPoint::new(self.query.x.0, y_m), 0.0);
            }
            for zone in &self.cfg.metrics.alignment_zones {
                self.push_line(FontPoint::new(self.query.x.0, zone.position), 0.0);
            }
            let half_xh = self.cfg.metrics.x_height * 0.5;
            self.push_line(FontPoint::new(0.0, half_xh), self.slant_line_angle);
            if self.cfg.advance_width > 0.0 {
                self.push_line(
                    FontPoint::new(self.cfg.advance_width, half_xh),
                    self.slant_line_angle,
                );
            }
        }
        if self.cfg.targets.guides && self.cfg.show_guides {
            for guide in self
                .cfg
                .local_guides
                .iter()
                .chain(self.cfg.global_guides.iter())
            {
                self.push_point(guide.position);
                self.push_line(guide.position, guide.angle_deg);
                if let GuideThickness::FontUnits(w) = guide.thickness {
                    if w.is_finite() && w > 0.0 {
                        let (ux, uy) = guide.direction_unit();
                        let (nx, ny) = (-uy, ux);
                        let half_w = w * 0.5;
                        for sign in [1.0, -1.0] {
                            self.push_line(
                                FontPoint::new(
                                    guide.position.x.0 + sign * nx * half_w,
                                    guide.position.y.0 + sign * ny * half_w,
                                ),
                                guide.angle_deg,
                            );
                        }
                    }
                }
            }
        }
        if self.cfg.targets.grid
            && self.cfg.show_grid
            && self.cfg.grid_step_upm > 0.0
            && self.cfg.grid_step_upm * f64::from(self.cfg.zoom) >= f64::from(radius_dp) * 2.0
        {
            let gx = (self.query.x.0 / self.cfg.grid_step_upm).round() * self.cfg.grid_step_upm;
            let gy = (self.query.y.0 / self.cfg.grid_step_upm).round() * self.cfg.grid_step_upm;
            self.push_line(FontPoint::new(gx, self.query.y.0), RIGHT_ANGLE_DEG_F64);
            self.push_line(FontPoint::new(self.query.x.0, gy), 0.0);
        }
    }

    fn collect_reference_frame(&mut self) {
        if !self.cfg.targets.reference_frame {
            return;
        }
        let Some(frame) = self.cfg.custom_reference_frame else {
            return;
        };
        let (xs, ys) = collect_reference_frame_snap_lines(frame, self.cfg.advance_width);
        for x in xs {
            self.push_line_with_kind(
                FontPoint::new(x, self.query.y.0),
                RIGHT_ANGLE_DEG_F64,
                Some(SnapTargetKind::ReferenceFrame),
            );
        }
        for y in ys {
            self.push_line_with_kind(
                FontPoint::new(self.query.x.0, y),
                0.0,
                Some(SnapTargetKind::ReferenceFrame),
            );
        }
    }
}

fn try_resolve_line_intersection(
    query: FontPoint,
    tolerance_upm: f64,
    best_point: Option<(FontPoint, f64)>,
    line_candidates: &[CandidateLineSnapper],
) -> Option<SnapResult> {
    let first = *line_candidates.first()?;
    let second = *line_candidates.iter().skip(1).find(|cand| {
        first.anchor.distance_to(cand.anchor) > SNAP_LINE_INTERSECT_DET_EPS
            && intersect_two_infinite_lines(
                first.anchor,
                first.angle_deg,
                cand.anchor,
                cand.angle_deg,
            )
            .is_some()
    })?;
    let inter = intersect_two_infinite_lines(
        first.anchor,
        first.angle_deg,
        second.anchor,
        second.angle_deg,
    )?;
    if query.distance_to(inter) <= tolerance_upm * 2.0
        && best_point.is_none_or(|(_, pt_dist)| {
            query.distance_to(inter) + SNAP_LINE_INTERSECT_DET_EPS < pt_dist
        })
    {
        let mut active = vec![
            ActiveSnapper {
                from: first.anchor,
                to: inter,
                angle_deg: Some(first.angle_deg),
                is_point_target: false,
                target_kind: first.target_kind,
            },
            ActiveSnapper {
                from: second.anchor,
                to: inter,
                angle_deg: Some(second.angle_deg),
                is_point_target: false,
                target_kind: second.target_kind,
            },
        ];
        active.truncate(MAX_ACTIVE_SNAPPERS);
        Some(SnapResult {
            snapped_point: inter,
            delta: (inter.x.0 - query.x.0, inter.y.0 - query.y.0),
            active_snappers: active,
        })
    } else {
        None
    }
}

impl SnapEngine {
    #[must_use]
    pub fn snap_point_with_config(query: FontPoint, cfg: &SnapQueryConfig<'_>) -> SnapResult {
        if cfg.ctrl_bypass {
            return SnapResult {
                snapped_point: query,
                delta: (0.0, 0.0),
                active_snappers: Vec::new(),
            };
        }
        let radius_dp = cfg.snap_radius_dp.max(PREF_MIN_RADIUS_DP);
        let tolerance_upm = f64::from(radius_dp) / f64::from(cfg.zoom.max(f32::EPSILON));
        let slant_line_angle = if cfg.targets.italic_angle {
            RIGHT_ANGLE_DEG_F64 - cfg.italic_angle_deg
        } else {
            RIGHT_ANGLE_DEG_F64
        };
        let mut collector = SnapCollector {
            query,
            tolerance_upm,
            best_point: None,
            line_candidates: Vec::new(),
            cfg,
            slant_line_angle,
        };
        collector.collect_nodes_and_background();
        collector.collect_metrics_guides_and_grid(radius_dp);
        collector.collect_reference_frame();
        collector.line_candidates.sort_by(|a, b| {
            a.dist
                .partial_cmp(&b.dist)
                .unwrap_or(std::cmp::Ordering::Equal)
        });

        if let Some(inter_res) = try_resolve_line_intersection(
            query,
            tolerance_upm,
            collector.best_point,
            &collector.line_candidates,
        ) {
            return inter_res;
        }
        if let Some((pt_target, _)) = collector.best_point {
            return SnapResult {
                snapped_point: pt_target,
                delta: (pt_target.x.0 - query.x.0, pt_target.y.0 - query.y.0),
                active_snappers: vec![ActiveSnapper {
                    from: pt_target,
                    to: pt_target,
                    angle_deg: None,
                    is_point_target: true,
                    target_kind: Some(SnapTargetKind::Node),
                }],
            };
        }
        if let Some(&best_line) = collector.line_candidates.first() {
            return SnapResult {
                snapped_point: best_line.proj,
                delta: (
                    best_line.proj.x.0 - query.x.0,
                    best_line.proj.y.0 - query.y.0,
                ),
                active_snappers: vec![ActiveSnapper {
                    from: best_line.anchor,
                    to: best_line.proj,
                    angle_deg: Some(best_line.angle_deg),
                    is_point_target: false,
                    target_kind: best_line.target_kind,
                }],
            };
        }
        SnapResult {
            snapped_point: query,
            delta: (0.0, 0.0),
            active_snappers: Vec::new(),
        }
    }
}
