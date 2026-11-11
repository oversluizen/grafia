//! Authentic Reference Vector vector cursor geometry and hit-testing tokens (`SPEC-002 §3`).
//!
//! Normative cursor geometry and hit-testing parameters:
//! - `DrawTool/Contents/Resources/{drawCursor,drawCursorElement,drawCursorPlus}.tif` (`hotSpot = (6.0, 0.0)`)
//! - `core geometry/Versions/A/Resources/GSArrowCursorElement.pdf` (`_elementCursor` in `SelectTool`)

use serde::{Deserialize, Serialize};

/// Click hit-test radius for `OnCurve` and `OffCurve` nodes in `SelectTool` (`GlyphLayer` at `HandleSize = 1`: `(5.0 + 2.5 * 1) = 7.5` dp).
pub const SELECT_NODE_HIT_RADIUS_DP: f32 = 7.5;

/// Click hit-test radius for `OnCurve` and `OffCurve` nodes in `PenTool` (`CanvasTool` at `HandleSize = 1`: `(5.0 + 2.0 * 1) = 7.0` dp).
pub const PEN_NODE_HIT_RADIUS_DP: f32 = 7.0;

/// Alias for `SELECT_NODE_HIT_RADIUS_DP` (`7.5` dp).
pub const NODE_HIT_TEST_RADIUS_DP: f32 = SELECT_NODE_HIT_RADIUS_DP;

/// Minimum pointer drag distance in screen-space dp required before `PenTool` pulls out Bézier
/// handles (`DraggingHandles`) or converts a newly placed/closed `Corner` node into `Smooth` (`SPEC-002 §3.2.1`).
pub const DRAG_THRESHOLD_DP: f32 = 3.0;

/// Radius multiplier relative to `node_size_dp * 0.5` for the visual closing ring/halo rendered on
/// `P0` when hovering within closing distance in `PenTool` (`SPEC-002 §3.2.4.1`).
pub const PEN_CLOSE_HALO_RADIUS_FACTOR: f32 = 1.65;

/// Authentic Vector Canvas tool cursor variants rendered in Pass 5 (`SPEC-002 §3.2.4.4`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum GlifoCursorKind {
    /// Standard macOS/Vector Canvas sharp black selection arrow with white border (`[NSCursor arrowCursor]`).
    SelectArrow,
    /// Selection arrow + bottom-right `6x6 dp` white-bordered / `4x4 dp` black square node badge
    /// (`_elementCursor` in `SelectTool` when hovering over an `OnCurve` node or `OffCurve` handle).
    SelectElement,
    /// Authentic Vector Canvas vertical fountain-pen nib (`drawCursor.tif`, `15x23 dp`, `hotSpot = (6.0, 0.0)`).
    PenStandard,
    /// Fountain-pen nib + bottom-right `6x6 dp` square node badge (`drawCursorElement.tif`, `21x23 dp`,
    /// used in `CanvasTool::mouseMoved:` when hovering over `P0` to close or over an `OnCurve` node).
    PenElement,
    /// Fountain-pen nib + bottom-right `+` cross badge (`drawCursorPlus.tif`, `21x23 dp`,
    /// used in `CanvasTool::mouseMoved:` when hovering over a path segment to insert a node).
    PenPlus,
    /// Open hand cursor (`openHandCursor`) in `PreviewFilled` / `Space` pan-ready mode.
    HandOpen,
    /// Closed grabbing hand cursor (`closedHandCursor`) during active `PanningViewport`.
    HandClosed,
    /// Magnifier with `+` (`ZoomIn`, `SPEC-003 §2.4`).
    ZoomIn,
    /// Magnifier with `-` when `Alt/Option` is held (`ZoomOut`, `SPEC-003 §2.4`).
    ZoomOut,
    /// Circular rotation cursor (`RotateCursor`, `SPEC-003 §2.1.3 & §2.2`).
    Rotate,
    /// Resize / Scale cursor (`ScaleCursor` / `BBoxResize`, `SPEC-003 §2.1.3 & §2.2`).
    Scale,
    /// Freeform Lasso loop cursor (`LassoCursor`, `SPEC-003 §2.3`).
    Lasso,
    /// Slant / Shear cursor (`BBoxSlant`, `SPEC-003 §2.1.3`).
    BBoxSlant,
}

/// Outer white border polygon of the Vector Canvas `SelectArrow` cursor relative to tip `(0.0, 0.0)`
/// (`GSArrowCursorElement.pdf` path 1).
pub const SELECT_ARROW_OUTER_POLYGON_DP: &[(f32, f32)] = &[
    (-1.0, -2.43),
    (-1.0, 14.02),
    (2.22, 11.17),
    (4.36, 16.29),
    (8.21, 14.75),
    (6.09, 9.60),
    (10.89, 9.60),
];

/// Inner black fill polygon of the Vector Canvas `SelectArrow` cursor relative to tip `(0.0, 0.0)`
/// (`GSArrowCursorElement.pdf` path 2).
pub const SELECT_ARROW_INNER_POLYGON_DP: &[(f32, f32)] = &[
    (0.0, 0.0),
    (0.0, 11.8),
    (2.6, 9.5),
    (4.9, 15.0),
    (6.9, 14.2),
    (4.6, 8.6),
    (8.5, 8.6),
];

/// Outer white rect `[min_x, min_y, max_x, max_y]` for the `SelectElement` node badge (`GSArrowCursorElement.pdf`).
pub const SELECT_ELEMENT_BADGE_OUTER_RECT_DP: [f32; 4] = [10.0, 11.0, 16.0, 17.0];

/// Inner black rect `[min_x, min_y, max_x, max_y]` for the `SelectElement` node badge (`GSArrowCursorElement.pdf`).
pub const SELECT_ELEMENT_BADGE_INNER_RECT_DP: [f32; 4] = [11.0, 12.0, 15.0, 16.0];

/// Outer white halo polygon of the Vector Canvas fountain-pen nib (`drawCursor.tif`, `hotSpot = (6.0, 0.0)`).
pub const PEN_NIB_OUTER_POLYGON_DP: &[(f32, f32)] = &[
    (0.0, -0.8),
    (-4.8, 11.0),
    (-3.2, 14.5),
    (-3.2, 19.5),
    (3.2, 19.5),
    (3.2, 14.5),
    (4.8, 11.0),
];

/// Inner black body polygon of the Vector Canvas fountain-pen nib (`drawCursor.tif`, `hotSpot = (6.0, 0.0)`).
pub const PEN_NIB_INNER_POLYGON_DP: &[(f32, f32)] = &[
    (0.0, 0.0),
    (-3.6, 11.0),
    (-2.2, 14.0),
    (2.2, 14.0),
    (3.6, 11.0),
];

/// Lower black collar cuff `[min_x, min_y, max_x, max_y]` of the Vector Canvas fountain-pen nib (`y = 15..18.5`).
pub const PEN_NIB_COLLAR_CUFF_RECT_DP: [f32; 4] = [-2.2, 15.2, 2.2, 18.5];

/// Vertical slit segment `[y_start, y_end, stroke_width]` along `x = 0.0` of the fountain-pen nib.
pub const PEN_NIB_SLIT_Y_RANGE_AND_WIDTH_DP: [f32; 3] = [2.2, 9.5, 1.1];

/// Center `(x, y)` and radius `r` of the circular breather hole in the fountain-pen nib.
pub const PEN_NIB_BREATHER_HOLE_DP: [f32; 3] = [0.0, 9.8, 1.15];

/// Outer white rect `[min_x, min_y, max_x, max_y]` of the `PenElement` node badge (`drawCursorElement.tif`).
pub const PEN_ELEMENT_BADGE_OUTER_RECT_DP: [f32; 4] = [6.0, 14.0, 12.0, 20.0];

/// Inner black rect `[min_x, min_y, max_x, max_y]` of the `PenElement` node badge (`drawCursorElement.tif`).
pub const PEN_ELEMENT_BADGE_INNER_RECT_DP: [f32; 4] = [7.0, 15.0, 11.0, 19.0];

/// Outer white vertical bar `[min_x, min_y, max_x, max_y]` of the `PenPlus` badge (`drawCursorPlus.tif`).
pub const PEN_PLUS_OUTER_VBAR_RECT_DP: [f32; 4] = [7.5, 13.0, 11.5, 21.0];

/// Outer white horizontal bar `[min_x, min_y, max_x, max_y]` of the `PenPlus` badge (`drawCursorPlus.tif`).
pub const PEN_PLUS_OUTER_HBAR_RECT_DP: [f32; 4] = [5.5, 15.0, 13.5, 19.0];

/// Inner black vertical bar `[min_x, min_y, max_x, max_y]` of the `PenPlus` badge (`drawCursorPlus.tif`).
pub const PEN_PLUS_INNER_VBAR_RECT_DP: [f32; 4] = [8.5, 14.0, 10.5, 20.0];

/// Inner black horizontal bar `[min_x, min_y, max_x, max_y]` of the `PenPlus` badge (`drawCursorPlus.tif`).
pub const PEN_PLUS_INNER_HBAR_RECT_DP: [f32; 4] = [6.5, 16.0, 12.5, 18.0];

/// Hand cursor palm radius and outline stroke width `[palm_r, outline_w, finger_offset_y]`.
pub const HAND_CURSOR_DIMENSIONS_DP: [f32; 3] = [5.5, 1.5, 3.5];

/// `ZoomIn` / `ZoomOut` magnifier lens concentric radii `[outer_white_r, black_ring_r, inner_lens_white_r]` (`SPEC-003 §2.4`).
pub const ZOOM_CURSOR_LENS_RADII_DP: [f32; 3] = [6.5, 5.2, 3.6];

/// `ZoomIn` / `ZoomOut` magnifier angled handle outer white border polygon (`SPEC-003 §2.4`).
pub const ZOOM_CURSOR_HANDLE_OUTER_POLYGON_DP: &[(f32, f32)] = &[
    (3.0, 4.8),
    (4.8, 3.0),
    (11.2, 9.4),
    (9.4, 11.2),
];

/// `ZoomIn` / `ZoomOut` magnifier angled handle inner black fill polygon (`SPEC-003 §2.4`).
pub const ZOOM_CURSOR_HANDLE_INNER_POLYGON_DP: &[(f32, f32)] = &[
    (3.6, 4.6),
    (4.6, 3.6),
    (10.2, 9.2),
    (9.2, 10.2),
];

/// Horizontal `-` bar `[min_x, min_y, max_x, max_y]` inside the `ZoomIn` / `ZoomOut` magnifier lens.
pub const ZOOM_CURSOR_HBAR_RECT_DP: [f32; 4] = [-2.4, -0.8, 2.4, 0.8];

/// Vertical `|` bar `[min_x, min_y, max_x, max_y]` inside the `ZoomIn` (`+`) magnifier lens.
pub const ZOOM_CURSOR_VBAR_RECT_DP: [f32; 4] = [-0.8, -2.4, 0.8, 2.4];

/// `Rotate` cursor concentric ring radii `[outer_white_r, black_ring_r, inner_cutout_white_r]` (`SPEC-003 §2.1.3 & §2.2`).
pub const ROTATE_CURSOR_RING_RADII_DP: [f32; 3] = [7.0, 5.6, 3.4];

/// `Rotate` cursor bottom-right ring gap cutout rect `[min_x, min_y, max_x, max_y]`.
pub const ROTATE_CURSOR_GAP_RECT_DP: [f32; 4] = [1.5, 0.5, 7.5, 6.5];

/// `Rotate` cursor arrowhead outer white border polygon.
pub const ROTATE_CURSOR_HEAD_OUTER_POLYGON_DP: &[(f32, f32)] = &[
    (1.0, -7.8),
    (8.2, -5.2),
    (3.8, 0.2),
];

/// `Rotate` cursor arrowhead inner black fill polygon.
pub const ROTATE_CURSOR_HEAD_INNER_POLYGON_DP: &[(f32, f32)] = &[
    (2.0, -6.6),
    (6.8, -4.8),
    (3.8, -1.0),
];

/// `Scale` cursor horizontal shaft outer white rect `[min_x, min_y, max_x, max_y]` (`SPEC-003 §2.1.3 & §2.2`).
pub const SCALE_CURSOR_SHAFT_OUTER_RECT_DP: [f32; 4] = [-6.5, -2.0, 6.5, 2.0];

/// `Scale` cursor horizontal shaft inner black rect `[min_x, min_y, max_x, max_y]`.
pub const SCALE_CURSOR_SHAFT_INNER_RECT_DP: [f32; 4] = [-5.0, -0.9, 5.0, 0.9];

/// `Scale` cursor left arrowhead outer white polygon.
pub const SCALE_CURSOR_LEFT_HEAD_OUTER_DP: &[(f32, f32)] = &[
    (-9.2, 0.0),
    (-3.8, -5.0),
    (-3.8, 5.0),
];

/// `Scale` cursor left arrowhead inner black polygon.
pub const SCALE_CURSOR_LEFT_HEAD_INNER_DP: &[(f32, f32)] = &[
    (-7.8, 0.0),
    (-4.6, -3.4),
    (-4.6, 3.4),
];

/// `Scale` cursor right arrowhead outer white polygon.
pub const SCALE_CURSOR_RIGHT_HEAD_OUTER_DP: &[(f32, f32)] = &[
    (9.2, 0.0),
    (3.8, -5.0),
    (3.8, 5.0),
];

/// `Scale` cursor right arrowhead inner black polygon.
pub const SCALE_CURSOR_RIGHT_HEAD_INNER_DP: &[(f32, f32)] = &[
    (7.8, 0.0),
    (4.6, -3.4),
    (4.6, 3.4),
];

/// `Lasso` cursor loop center and concentric radii `[cx, cy, outer_white_r, black_ring_r, inner_white_r]` (`SPEC-003 §2.3`).
pub const LASSO_CURSOR_LOOP_CENTER_AND_RADII_DP: [f32; 5] = [4.0, -3.5, 6.2, 4.8, 3.0];

/// `Lasso` cursor knot outer white rect `[min_x, min_y, max_x, max_y]`.
pub const LASSO_CURSOR_KNOT_OUTER_RECT_DP: [f32; 4] = [-2.2, -2.2, 2.2, 2.2];

/// `Lasso` cursor knot inner black rect `[min_x, min_y, max_x, max_y]`.
pub const LASSO_CURSOR_KNOT_INNER_RECT_DP: [f32; 4] = [-1.2, -1.2, 1.2, 1.2];

/// `Lasso` cursor rope tail outer white polygon.
pub const LASSO_CURSOR_TAIL_OUTER_POLYGON_DP: &[(f32, f32)] = &[
    (-1.2, 1.0),
    (1.2, 1.0),
    (-2.8, 8.5),
    (-5.2, 7.5),
];

/// `Lasso` cursor rope tail inner black polygon.
pub const LASSO_CURSOR_TAIL_INNER_POLYGON_DP: &[(f32, f32)] = &[
    (-0.4, 1.2),
    (0.6, 1.2),
    (-3.2, 7.6),
    (-4.2, 7.0),
];

/// `BBoxSlant` shear cursor outer white parallelogram polygon (`SPEC-003 §2.1.3`).
pub const SLANT_CURSOR_OUTER_POLYGON_DP: &[(f32, f32)] = &[
    (-5.5, -5.5),
    (7.5, -5.5),
    (5.5, 5.5),
    (-7.5, 5.5),
];

/// `BBoxSlant` shear cursor top inner black bar polygon.
pub const SLANT_CURSOR_TOP_BAR_INNER_DP: &[(f32, f32)] = &[
    (-4.0, -4.2),
    (6.0, -4.2),
    (5.2, -1.4),
    (-4.8, -1.4),
];

/// `BBoxSlant` shear cursor bottom inner black bar polygon.
pub const SLANT_CURSOR_BOT_BAR_INNER_DP: &[(f32, f32)] = &[
    (-5.2, 1.4),
    (4.8, 1.4),
    (4.0, 4.2),
    (-6.0, 4.2),
];
