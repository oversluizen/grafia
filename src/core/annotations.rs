//! `CanvasAnnotation` canvas annotation model & hit-testing (`SPEC-005 §2.4`).
//!
//! Extracted 1-to-1 from `AnnotationTool` (`SPEC-005`) in Reference Vector (``).

use crate::core::{FontPoint, ScreenPoint, ViewportTransform};
use crate::tokens::{
    ANNOTATION_BODY_HIT_RADIUS_DP, ANNOTATION_DEFAULT_ARROW_ANGLE_DEG,
    ANNOTATION_DEFAULT_ARROW_LEN_UPM, ANNOTATION_DEFAULT_CIRCLE_RADIUS_UPM,
    ANNOTATION_DEFAULT_TEXT_HEIGHT_UPM, ANNOTATION_DEFAULT_TEXT_WIDTH_UPM,
    ANNOTATION_HANDLE_HIT_RADIUS_DP, ANNOTATION_MIN_CIRCLE_RADIUS_UPM,
    ANNOTATION_MIN_TEXT_WIDTH_UPM, MIN_VIEWPORT_ZOOM,
};
use serde::{Deserialize, Serialize};

/// Type-safe identifier for a `CanvasAnnotation` (`CanvasAnnotation`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct AnnotationId(pub u64);

/// Active sub-mode of `AnnotationTool` (`AnnotationSubTool = 1..5`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum AnnotationSubTool {
    /// `1`: Text note (`@"empty note"` placeholder when empty).
    #[default]
    Text,
    /// `2`: Directional arrow with rotatable tip handle.
    Arrow,
    /// `3`: Circle annotation (`default width = 100.0` UPM at `spec_ref`).
    Circle,
    /// `4`: Plus (`+`) mark.
    Plus,
    /// `5`: Minus (`-`) mark.
    Minus,
}

impl AnnotationSubTool {
    /// Cycles through the 5 Vector Canvas annotation sub-tools (`Shift+A`).
    #[must_use]
    pub const fn cycle_next(self) -> Self {
        match self {
            Self::Text => Self::Arrow,
            Self::Arrow => Self::Circle,
            Self::Circle => Self::Plus,
            Self::Plus => Self::Minus,
            Self::Minus => Self::Text,
        }
    }
}

/// Geometry and payload for the 5 `CanvasAnnotation` kinds (`SPEC-005 §2.4`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum AnnotationKind {
    /// `1 = Text`: text string and box width in UPM.
    Text { text: String, width_upm: f64 },
    /// `2 = Arrow`: direction angle in degrees and shaft length in UPM.
    Arrow { angle_deg: f64, length_upm: f64 },
    /// `3 = Circle`: circle radius in UPM (`CanvasAnnotation.width`, default `100.0` UPM).
    Circle { radius_upm: f64 },
    /// `4 = Plus`: screen-space `+` symbol.
    Plus,
    /// `5 = Minus`: screen-space `-` symbol.
    Minus,
}

/// A canvas annotation attached to a `Glyph` (`layer.annotations`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CanvasAnnotation {
    pub id: AnnotationId,
    pub position: FontPoint,
    pub kind: AnnotationKind,
}

impl CanvasAnnotation {
    /// Creates a new `CanvasAnnotation` with Reference Vector (``) defaults (`spec_ref..spec_ref`).
    #[must_use]
    pub fn new_for_sub_tool(
        id: AnnotationId,
        position: FontPoint,
        sub_tool: AnnotationSubTool,
    ) -> Self {
        let kind = match sub_tool {
            AnnotationSubTool::Text => AnnotationKind::Text {
                text: String::new(),
                width_upm: ANNOTATION_DEFAULT_TEXT_WIDTH_UPM,
            },
            AnnotationSubTool::Arrow => AnnotationKind::Arrow {
                angle_deg: ANNOTATION_DEFAULT_ARROW_ANGLE_DEG,
                length_upm: ANNOTATION_DEFAULT_ARROW_LEN_UPM,
            },
            AnnotationSubTool::Circle => AnnotationKind::Circle {
                radius_upm: ANNOTATION_DEFAULT_CIRCLE_RADIUS_UPM,
            },
            AnnotationSubTool::Plus => AnnotationKind::Plus,
            AnnotationSubTool::Minus => AnnotationKind::Minus,
        };
        Self { id, position, kind }
    }

    /// Returns the UPM position of the annotation's interactive control handle (`annotation.activeHandle`),
    /// if the annotation kind exposes a resize/rotate handle (`Text`, `Arrow`, `Circle`).
    #[must_use]
    pub fn handle_position_upm(&self) -> Option<FontPoint> {
        match &self.kind {
            AnnotationKind::Text { width_upm, .. } => Some(FontPoint::new(
                self.position.x.0 + *width_upm,
                self.position.y.0,
            )),
            AnnotationKind::Arrow {
                angle_deg,
                length_upm,
            } => {
                let rad = angle_deg.to_radians();
                Some(FontPoint::new(
                    self.position.x.0 + *length_upm * rad.cos(),
                    self.position.y.0 + *length_upm * rad.sin(),
                ))
            }
            AnnotationKind::Circle { radius_upm } => Some(FontPoint::new(
                self.position.x.0,
                self.position.y.0 - *radius_upm,
            )),
            AnnotationKind::Plus | AnnotationKind::Minus => None,
        }
    }

    /// Updates this annotation's geometry when its control handle is dragged (`spec_ref..spec_ref`).
    pub fn apply_handle_drag(&mut self, initial_kind: &AnnotationKind, current_upm: FontPoint, start_upm: FontPoint) {
        let dx = current_upm.x.0 - start_upm.x.0;
        let dy = current_upm.y.0 - start_upm.y.0;
        match (&mut self.kind, initial_kind) {
            (
                AnnotationKind::Text { width_upm, .. },
                AnnotationKind::Text {
                    width_upm: init_w, ..
                },
            ) => {
                *width_upm = (*init_w + dx).max(ANNOTATION_MIN_TEXT_WIDTH_UPM);
            }
            (AnnotationKind::Arrow { angle_deg, .. }, AnnotationKind::Arrow { .. }) => {
                let vx = current_upm.x.0 - self.position.x.0;
                let vy = current_upm.y.0 - self.position.y.0;
                if vx.hypot(vy) > 0.5 {
                    *angle_deg = vy.atan2(vx).to_degrees();
                }
            }
            (
                AnnotationKind::Circle { radius_upm },
                AnnotationKind::Circle {
                    radius_upm: init_r,
                },
            ) => {
                // `AnnotationTool::moveSelectionLayer:shadowLayer:withPoint:withModifier:` (`spec_ref..spec_ref`):
                // `width = max(shadow.width - dy, minWidth)`
                *radius_upm = (*init_r - dy).max(ANNOTATION_MIN_CIRCLE_RADIUS_UPM);
            }
            _ => {}
        }
    }
}

/// Hit-tests annotation control handles first (`elementAtPoint:atLayer:` `spec_ref`).
#[must_use]
pub fn hit_test_annotation_handle(
    annotations: &[CanvasAnnotation],
    viewport: &ViewportTransform,
    scr: ScreenPoint,
) -> Option<AnnotationId> {
    let hit_r = f64::from(ANNOTATION_HANDLE_HIT_RADIUS_DP);
    for ann in annotations.iter().rev() {
        if let Some(h_upm) = ann.handle_position_upm() {
            let h_scr = viewport.to_screen(h_upm);
            let dist = f64::from(scr.x.0 - h_scr.x.0).hypot(f64::from(scr.y.0 - h_scr.y.0));
            if dist <= hit_r {
                return Some(ann.id);
            }
        }
    }
    None
}

/// Hit-tests annotation bodies (`elementAtPoint:atLayer:` `spec_ref..spec_ref`).
#[must_use]
pub fn hit_test_annotation_body(
    annotations: &[CanvasAnnotation],
    viewport: &ViewportTransform,
    scr: ScreenPoint,
) -> Option<AnnotationId> {
    let hit_r = f64::from(ANNOTATION_BODY_HIT_RADIUS_DP);
    let font_pt = viewport.to_font(scr);
    for ann in annotations.iter().rev() {
        let p_scr = viewport.to_screen(ann.position);
        let center_dist = f64::from(scr.x.0 - p_scr.x.0).hypot(f64::from(scr.y.0 - p_scr.y.0));
        if center_dist <= hit_r {
            return Some(ann.id);
        }
        match &ann.kind {
            AnnotationKind::Text { width_upm, .. } => {
                let in_x = font_pt.x.0 >= ann.position.x.0
                    && font_pt.x.0 <= ann.position.x.0 + *width_upm;
                let in_y = (font_pt.y.0 - ann.position.y.0).abs()
                    <= ANNOTATION_DEFAULT_TEXT_HEIGHT_UPM;
                if in_x && in_y {
                    return Some(ann.id);
                }
            }
            AnnotationKind::Arrow {
                angle_deg,
                length_upm,
            } => {
                let rad = angle_deg.to_radians();
                let tip = FontPoint::new(
                    ann.position.x.0 + *length_upm * rad.cos(),
                    ann.position.y.0 + *length_upm * rad.sin(),
                );
                let tip_scr = viewport.to_screen(tip);
                let vx = f64::from(tip_scr.x.0 - p_scr.x.0);
                let vy = f64::from(tip_scr.y.0 - p_scr.y.0);
                let len_sq = vx * vx + vy * vy;
                if len_sq > 1.0 {
                    let t = ((f64::from(scr.x.0 - p_scr.x.0) * vx
                        + f64::from(scr.y.0 - p_scr.y.0) * vy)
                        / len_sq)
                        .clamp(0.0, 1.0);
                    let proj_x = f64::from(p_scr.x.0) + t * vx;
                    let proj_y = f64::from(p_scr.y.0) + t * vy;
                    if (f64::from(scr.x.0) - proj_x).hypot(f64::from(scr.y.0) - proj_y) <= hit_r {
                        return Some(ann.id);
                    }
                }
            }
            AnnotationKind::Circle { radius_upm } => {
                let d_upm = (font_pt.x.0 - ann.position.x.0).hypot(font_pt.y.0 - ann.position.y.0);
                let tol_upm = hit_r / f64::from(viewport.zoom.max(MIN_VIEWPORT_ZOOM));
                if (d_upm - *radius_upm).abs() <= tol_upm || d_upm <= tol_upm {
                    return Some(ann.id);
                }
            }
            AnnotationKind::Plus | AnnotationKind::Minus => {}
        }
    }
    None
}
