//! Type-safe coordinate systems separating Font Units from Screen Logical Points (STD-ARCH Rule 1).

use serde::{Deserialize, Serialize};

/// Coordinate in font design units (Cartesian, Y-up, integers/f64).
#[derive(Debug, Clone, Copy, Default, PartialEq, PartialOrd, Serialize, Deserialize)]
#[repr(transparent)]
pub struct FontUnit(pub f64);

/// 2D point in font design space.
#[derive(Debug, Clone, Copy, Default, PartialEq, Serialize, Deserialize)]
pub struct FontPoint {
    pub x: FontUnit,
    pub y: FontUnit,
}

impl FontPoint {
    #[must_use]
    pub const fn new(x: f64, y: f64) -> Self {
        Self {
            x: FontUnit(x),
            y: FontUnit(y),
        }
    }

    #[must_use]
    pub fn distance_to(&self, other: Self) -> f64 {
        let dx = self.x.0 - other.x.0;
        let dy = self.y.0 - other.y.0;
        dx.hypot(dy)
    }

    /// Rounds this font point to the nearest `1.0 UPM` integer grid (`TC-GRAFIA-015`).
    #[must_use]
    pub fn round_to_grid(self) -> Self {
        Self::new(self.x.0.round(), self.y.0.round())
    }

    /// Rounds this font point to a configurable UPM grid step (`grid_step_upm`; values `<= 0.0` preserve sub-unit coordinates).
    #[must_use]
    pub fn round_to_grid_step(self, grid_step_upm: f64) -> Self {
        if grid_step_upm <= 0.0 {
            self
        } else {
            Self::new(
                (self.x.0 / grid_step_upm).round() * grid_step_upm,
                (self.y.0 / grid_step_upm).round() * grid_step_upm,
            )
        }
    }
}

/// Coordinate on screen in logical density-independent pixels (Y-down, f32).
#[derive(Debug, Clone, Copy, PartialEq, PartialOrd, Serialize, Deserialize)]
#[repr(transparent)]
pub struct ScreenDp(pub f32);

/// 2D point on screen in viewport space.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct ScreenPoint {
    pub x: ScreenDp,
    pub y: ScreenDp,
}

impl ScreenPoint {
    #[must_use]
    pub const fn new(x: f32, y: f32) -> Self {
        Self {
            x: ScreenDp(x),
            y: ScreenDp(y),
        }
    }

    #[must_use]
    pub fn distance_to(&self, other: Self) -> f32 {
        let dx = self.x.0 - other.x.0;
        let dy = self.y.0 - other.y.0;
        dx.hypot(dy)
    }
}

/// Affine viewport transformation between FontUnits and ScreenDp.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct ViewportTransform {
    pub zoom: f32,
    pub pan_offset: ScreenPoint,
    pub upm: u16,
}

impl ViewportTransform {
    #[must_use]
    pub fn new(zoom: f32, pan_offset: ScreenPoint, upm: u16) -> Self {
        Self {
            zoom: zoom.max(crate::tokens::MIN_VIEWPORT_ZOOM),
            pan_offset,
            upm,
        }
    }

    /// Returns a copy of this viewport transform shifted horizontally by `origin_x_upm` in UPM.
    #[must_use]
    pub fn with_origin_x_upm(self, origin_x_upm: f64) -> Self {
        Self {
            zoom: self.zoom,
            pan_offset: ScreenPoint::new(
                self.pan_offset.x.0 + (origin_x_upm as f32 * self.zoom),
                self.pan_offset.y.0,
            ),
            upm: self.upm,
        }
    }

    /// Converts a point in font design space to screen viewport coordinates.
    #[must_use]
    pub fn to_screen(&self, pt: FontPoint) -> ScreenPoint {
        let sx = (pt.x.0 as f32 * self.zoom) + self.pan_offset.x.0;
        let sy = self.pan_offset.y.0 - (pt.y.0 as f32 * self.zoom);
        ScreenPoint::new(sx, sy)
    }

    /// Converts a point on the screen viewport back to font design coordinates.
    #[must_use]
    pub fn to_font(&self, pt: ScreenPoint) -> FontPoint {
        let fx = ((pt.x.0 - self.pan_offset.x.0) / self.zoom) as f64;
        let fy = ((self.pan_offset.y.0 - pt.y.0) / self.zoom) as f64;
        FontPoint::new(fx, fy)
    }

    /// Zooms the viewport by `factor` while keeping `cursor_scr` pinned to the same font coordinate.
    pub fn zoom_at_cursor(&mut self, cursor_scr: ScreenPoint, factor: f32) {
        let anchor_font = self.to_font(cursor_scr);
        self.zoom = (self.zoom * factor).clamp(
            crate::tokens::MIN_VIEWPORT_ZOOM,
            crate::tokens::MAX_VIEWPORT_ZOOM,
        );
        let pan_x = cursor_scr.x.0 - (anchor_font.x.0 as f32 * self.zoom);
        let pan_y = cursor_scr.y.0 + (anchor_font.y.0 as f32 * self.zoom);
        self.pan_offset = ScreenPoint::new(pan_x, pan_y);
    }
}
