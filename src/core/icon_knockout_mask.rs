//! Live Non-Destructive `Knockout / Slash` Halo-Masks (`SPEC-017 §2.5`).
//!
//! When `SymbolLayerTag.knockout_margin_upm > 0.0` on sublayer $k$, expands sublayer $k$'s
//! already-evaluated contours by `knockout_margin_upm` into a closed knockout halo and
//! non-destructively carves (`subtract_contours`) that halo from all underlying sublayers $j < k$.

use crate::core::bezier_ops::{Node, NodeType};
use crate::core::boolean_ops::{
    contour_signed_area, remove_overlap_in_contours, subtract_contours,
};
use crate::core::color_layers::CanvasColorSublayer;
use crate::core::coordinates::FontPoint;
use crate::core::curve_manipulation::{extract_segments, ContourSegment};
use crate::core::fit_and_stroke::{
    ContourStrokeStyle, LineCapStyle, LineJoinStyle, StrokePosition,
};
use crate::core::glyph_model::{ContourId, GlyphContour};
use crate::core::icon_studio::SymbolLayerTag;
use crate::core::kinetic_draw_trim::{cubic_arc_table, cubic_param_for_arc_fraction};
use crate::core::math_consts::{
    BOOL_CUBIC_SUBDIV_STEPS, FIT_CURVE_MIN_CHORD_LEN_EPS, ICON_STUDIO_MAX_COORD_ABS_UPM,
    KINETIC_BOUNCE_DAMPING, KINETIC_REPEAT_DELAY_MAX_PHASE, KINETIC_SPRING_MAX_FACTOR,
    KINETIC_SPRING_OVERSHOOT_AMP, KNOCKOUT_MARGIN_MAX_UPM, PI_F64,
};
use crate::core::reference_frame::RectUpm;
use crate::core::reverse_contour_nodes;
use crate::core::stroke_expand::expand_single_contour_stroke;
use kurbo::{ParamCurve, ParamCurveDeriv};
use serde::{Deserialize, Serialize};

/// Apple SF Symbols 7 execution and playback modes (`SPEC-017 §2.6`).
#[derive(Debug, Clone, Copy, PartialEq, Default, Serialize, Deserialize)]
pub enum KineticPlaybackMode {
    OneShot,
    Repeating {
        repeat_delay_phase: f64,
    },
    #[default]
    Continuous,
    VariableValue(f64),
}

impl KineticPlaybackMode {
    /// Resolves the effective timeline phase $t_{\text{eff}} \in [0.0, 1.0]$ for `phase_t`.
    #[must_use]
    pub fn effective_phase(self, phase_t: f64) -> f64 {
        let base = if phase_t.is_finite() {
            phase_t.clamp(0.0, 1.0)
        } else {
            0.0
        };
        match self {
            Self::VariableValue(v) => {
                if v.is_finite() {
                    v.clamp(0.0, 1.0)
                } else {
                    0.0
                }
            }
            Self::Repeating { repeat_delay_phase } => {
                let d = if repeat_delay_phase.is_finite() {
                    repeat_delay_phase.clamp(0.0, KINETIC_REPEAT_DELAY_MAX_PHASE)
                } else {
                    0.0
                };
                if d <= FIT_CURVE_MIN_CHORD_LEN_EPS {
                    base
                } else {
                    (base / (1.0 - d)).clamp(0.0, 1.0)
                }
            }
            Self::OneShot | Self::Continuous => base,
        }
    }

    /// Evaluates a critically/under-damped spring overshoot factor backed by `vitrea::DualSpringMorphController` (`SPEC-017 §2.6`).
    #[must_use]
    pub fn evaluate_spring_factor(self, tau: f64) -> f64 {
        let u = if tau.is_finite() {
            tau.clamp(0.0, 1.0)
        } else {
            0.0
        };
        let ctrl = vitrea::DualSpringMorphController::new(0.0, u as f32);
        let dir = if vitrea::DualSpringMorphController::is_moving_right(ctrl.min_x, ctrl.max_x) {
            1.0
        } else {
            -1.0
        };
        let lead = f64::from(ctrl.max_x);
        let overshoot = dir
            * (-KINETIC_BOUNCE_DAMPING * lead).exp()
            * (PI_F64 * lead).sin()
            * KINETIC_SPRING_OVERSHOOT_AMP;
        (lead + overshoot).clamp(0.0, KINETIC_SPRING_MAX_FACTOR)
    }

    /// Returns `true` when kinetic evaluation is active even if `playing == false` and `phase_t == 0.0`.
    #[must_use]
    pub const fn is_always_active(self) -> bool {
        matches!(self, Self::VariableValue(_))
    }

    /// Sanitizes an external/deserialized `KineticPlaybackMode`.
    #[must_use]
    pub fn sanitized(self) -> Self {
        match self {
            Self::Repeating { repeat_delay_phase } => Self::Repeating {
                repeat_delay_phase: if repeat_delay_phase.is_finite() {
                    repeat_delay_phase.clamp(0.0, KINETIC_REPEAT_DELAY_MAX_PHASE)
                } else {
                    0.0
                },
            },
            Self::VariableValue(v) => Self::VariableValue(if v.is_finite() {
                v.clamp(0.0, 1.0)
            } else {
                0.0
            }),
            other => other,
        }
    }
}

/// Evaluates the position and analytical tangent angle $\theta(u) = \text{atan2}(y'(u), x'(u))$
/// at normalized arc-length ratio `ratio` in `[0.0, 1.0]` along `motion_path` (`SPEC-017 §2.6`).
#[must_use]
pub fn evaluate_motion_path_pose_at_ratio(
    motion_path: &GlyphContour,
    ratio: f64,
) -> Option<(FontPoint, f64)> {
    if motion_path.nodes.len() < 2
        || !motion_path.nodes.iter().all(|n| {
            n.point.x.0.is_finite()
                && n.point.y.0.is_finite()
                && n.point.x.0.abs() <= ICON_STUDIO_MAX_COORD_ABS_UPM
                && n.point.y.0.abs() <= ICON_STUDIO_MAX_COORD_ABS_UPM
        })
    {
        return None;
    }
    let u = if ratio.is_finite() {
        ratio.clamp(0.0, 1.0)
    } else {
        0.0
    };
    let segs = extract_segments(&motion_path.nodes, motion_path.closed);
    if segs.is_empty() {
        return None;
    }
    let seg_lens: Vec<f64> = segs
        .iter()
        .map(|s| match *s {
            ContourSegment::Line { line, .. } => {
                (line.p1.x - line.p0.x).hypot(line.p1.y - line.p0.y)
            }
            ContourSegment::Cubic { cubic, .. } => cubic_arc_table(cubic)[BOOL_CUBIC_SUBDIV_STEPS],
        })
        .collect();
    let total_len: f64 = seg_lens.iter().sum();
    if total_len <= FIT_CURVE_MIN_CHORD_LEN_EPS {
        return None;
    }
    let target_s = u * total_len;
    let mut cum = 0.0_f64;
    for (idx, (seg, &s_len)) in segs.iter().zip(&seg_lens).enumerate() {
        let next_cum = cum + s_len;
        if target_s <= next_cum || idx + 1 == segs.len() {
            let local_f = if s_len > FIT_CURVE_MIN_CHORD_LEN_EPS {
                ((target_s - cum) / s_len).clamp(0.0, 1.0)
            } else {
                0.0
            };
            return Some(match *seg {
                ContourSegment::Line { line, .. } => {
                    let pt = line.eval(local_f);
                    let ang = (line.p1.y - line.p0.y).atan2(line.p1.x - line.p0.x);
                    (FontPoint::new(pt.x, pt.y), ang)
                }
                ContourSegment::Cubic { cubic, .. } => {
                    let table = cubic_arc_table(cubic);
                    let t = cubic_param_for_arc_fraction(&table, local_f);
                    let pt = cubic.eval(t);
                    let d = cubic.deriv().eval(t);
                    let ang = if d.x.hypot(d.y) > FIT_CURVE_MIN_CHORD_LEN_EPS {
                        d.y.atan2(d.x)
                    } else {
                        (cubic.p3.y - cubic.p0.y).atan2(cubic.p3.x - cubic.p0.x)
                    };
                    (FontPoint::new(pt.x, pt.y), ang)
                }
            });
        }
        cum = next_cum;
    }
    None
}

fn is_valid_closed_contour(c: &GlyphContour) -> bool {
    c.visible
        && c.closed
        && c.nodes
            .iter()
            .filter(|n| n.node_type != NodeType::OffCurve)
            .count()
            >= 3
        && c.nodes.iter().all(|n| {
            n.point.x.0.is_finite()
                && n.point.y.0.is_finite()
                && n.point.x.0.abs() <= ICON_STUDIO_MAX_COORD_ABS_UPM
                && n.point.y.0.abs() <= ICON_STUDIO_MAX_COORD_ABS_UPM
        })
}

fn contour_bbox(nodes: &[Node]) -> RectUpm {
    let (mut x0, mut y0, mut x1, mut y1) = (
        f64::INFINITY,
        f64::INFINITY,
        f64::NEG_INFINITY,
        f64::NEG_INFINITY,
    );
    for n in nodes {
        x0 = x0.min(n.point.x.0);
        y0 = y0.min(n.point.y.0);
        x1 = x1.max(n.point.x.0);
        y1 = y1.max(n.point.y.0);
    }
    RectUpm::new(x0, y0, x1, y1)
}

fn bboxes_overlap(a: RectUpm, b: RectUpm) -> bool {
    a.min_x <= b.max_x && a.max_x >= b.min_x && a.min_y <= b.max_y && a.max_y >= b.min_y
}

/// Builds the expanded closed knockout halo contours (`Vec<Vec<Node>>`) for `source_contours`
/// expanded outward by `margin_upm` (`SPEC-017 §2.5`).
#[must_use]
pub fn build_expanded_knockout_halo_nodes(
    source_contours: &[GlyphContour],
    margin_upm: f64,
    next_node_id: &mut u64,
) -> Vec<Vec<Node>> {
    if !margin_upm.is_finite() || margin_upm <= FIT_CURVE_MIN_CHORD_LEN_EPS {
        return Vec::new();
    }
    let m = margin_upm.clamp(0.0, KNOCKOUT_MARGIN_MAX_UPM);
    let mut halo_polys = Vec::new();
    let mut next_cid = 50_000_usize;

    for gc in source_contours {
        if !gc.visible || gc.nodes.len() < 2 {
            continue;
        }
        if !gc.closed {
            let st = ContourStrokeStyle {
                width: 2.0 * m,
                height: 2.0 * m,
                position: StrokePosition::Center,
                cap_start: LineCapStyle::Round,
                cap_end: LineCapStyle::Round,
                join: LineJoinStyle::Round,
            };
            for exp in expand_single_contour_stroke(gc, st, &mut next_cid, next_node_id) {
                if is_valid_closed_contour(&exp) {
                    halo_polys.push(exp.nodes);
                }
            }
            continue;
        }
        if !is_valid_closed_contour(gc) {
            continue;
        }
        let mut ccw = gc.clone();
        if contour_signed_area(&ccw.nodes) < 0.0 {
            reverse_contour_nodes(&mut ccw.nodes, true);
        }
        let ring_style = ContourStrokeStyle {
            width: 2.0 * m,
            height: 2.0 * m,
            position: StrokePosition::Center,
            cap_start: LineCapStyle::Round,
            cap_end: LineCapStyle::Round,
            join: LineJoinStyle::Round,
        };
        let rings = expand_single_contour_stroke(&ccw, ring_style, &mut next_cid, next_node_id);
        let best_outer = rings
            .into_iter()
            .filter(is_valid_closed_contour)
            .max_by(|a, b| {
                contour_signed_area(&a.nodes)
                    .abs()
                    .total_cmp(&contour_signed_area(&b.nodes).abs())
            });
        if let Some(outer) = best_outer {
            if contour_signed_area(&outer.nodes).abs() >= contour_signed_area(&ccw.nodes).abs() {
                halo_polys.push(outer.nodes);
                continue;
            }
        }
        halo_polys.push(ccw.nodes);
    }
    if !halo_polys.is_empty() {
        let mut closed_flags = vec![true; halo_polys.len()];
        let _ = remove_overlap_in_contours(&mut halo_polys, &mut closed_flags, &[], next_node_id);
    }
    halo_polys
}

/// Applies non-destructive knockout halo subtraction across `sublayers` for every sublayer $k$
/// where `tags[k].knockout_margin_upm > 0.0` (`SPEC-017 §2.5`).
pub fn apply_knockout_halo_masks_to_sublayers(
    sublayers: &mut [CanvasColorSublayer],
    tags: &[SymbolLayerTag],
    next_node_id: &mut u64,
) {
    if sublayers.len() < 2 {
        return;
    }
    for k in 1..sublayers.len() {
        if !sublayers[k].visible || sublayers[k].opacity <= 0.0 {
            continue;
        }
        let margin = tags.get(k).map_or(0.0, |t| t.knockout_margin_upm);
        if !margin.is_finite() || margin <= FIT_CURVE_MIN_CHORD_LEN_EPS {
            continue;
        }
        let halo_clip =
            build_expanded_knockout_halo_nodes(&sublayers[k].contours, margin, next_node_id);
        if halo_clip.is_empty() {
            continue;
        }
        let clip_bboxes: Vec<_> = halo_clip.iter().map(|c| contour_bbox(c)).collect();

        for sub_j in sublayers.iter_mut().take(k) {
            if !sub_j.visible || sub_j.contours.is_empty() {
                continue;
            }
            let subject: Vec<Vec<Node>> = sub_j
                .contours
                .iter()
                .filter(|c| is_valid_closed_contour(c))
                .map(|c| c.nodes.clone())
                .collect();
            if subject.is_empty() {
                continue;
            }
            let any_hit = subject.iter().any(|s| {
                let sb = contour_bbox(s);
                clip_bboxes.iter().any(|&cb| bboxes_overlap(sb, cb))
            });
            if !any_hit {
                continue;
            }
            let carved = subtract_contours(&subject, &halo_clip, next_node_id);
            let mut next_contours = Vec::with_capacity(carved.len());
            for (idx, nodes) in carved.into_iter().enumerate() {
                let mut gc = GlyphContour::new(ContourId(idx), nodes);
                gc.closed = true;
                next_contours.push(gc);
            }
            for c in &sub_j.contours {
                if !c.closed && c.visible {
                    let mut open_copy = c.clone();
                    open_copy.id = ContourId(next_contours.len());
                    next_contours.push(open_copy);
                }
            }
            sub_j.contours = next_contours;
        }
    }
}
