# Magnetic Snapping, Guides, Alignment Zones & Reference Frames (`grafia::snapping`)

This module implements the deterministic spatial alignment engine (`SnapEngine`), local and global `Guide` geometry, typographic `AlignmentZone` overshoot bands, and `CustomReferenceFrame` snap integration.

---

## 1. Priority-Based Magnetic `SnapEngine`

During interactive node dragging, pen placement, primitive drawing, or bounding-box transforms, `SnapEngine` evaluates candidate coordinates against active spatial targets within a configurable screen-space capture radius (`snap_radius_dp`, converted to design-space `FontUnit` via the current viewport zoom).

To keep snapping predictable and configurable by the host application, every snap query filters targets through `SnapTargetMask` (`GrafiaPreferences::snap_mask`):

| Priority | `SnapTargetKind` | Description |
| :---: | :--- | :--- |
| **1 (Highest)** | `Grid` | Quantizes coordinates to the active canvas grid step when grid snapping is enabled. |
| **2** | `Metrics` | Snaps to primary horizontal/vertical typographic metric lines (`baseline`, `x_height`, `cap_height`, `ascender`, `descender`, and sidebearings `x = 0` / `x = advance_width`). |
| **3** | `AlignmentZone` | Snaps to the flat edge or outer overshoot edge of horizontal `AlignmentZone` bands. |
| **4** | `Guide` | Snaps to local and global `Guide` rays (both centerlines and band edges when `GuideThickness::FontUnits(w)` is active). |
| **5** | `ReferenceFrame` | Snaps to `CustomReferenceFrame` outer/inner bounds, $N_x \times N_y$ grid subdivisions, center crosshairs, and `ReferenceBandLine`s. |
| **6** | `Anchor` | Snaps horizontally/vertically or point-to-point onto named 2D `CanvasAnchor` positions. |
| **7** | `Node` / `Background` | Snaps to unselected foreground `Node` coordinates and `background_contours` reference nodes. |

Horizontal ($X$) and vertical ($Y$) axes are resolved independently so a dragged node can simultaneously lock its $X$ coordinate to an existing stem node and its $Y$ coordinate to the `x_height` metric line.

---

## 2. Local & Global Guides and Zoom-Scaling Band Thickness (`Guide`, `GuideThickness`)

A `Guide` is defined by an origin `FontPoint` and an angle in degrees (`normalize_guide_angle`), supporting horizontal ($0^\circ$), vertical ($90^\circ$), italic-slanted, and arbitrary angled construction lines:

- **Two-Node Smart Guides (`smart_two_node_guide_geometry`):** Selecting two on-curve nodes and adding a guide automatically aligns the guide ray through both nodes and activates live contour intersection measurement (`GuideMeasurementRay` / `GuideMeasurementSegment`).
- **Guide Intersections (`calculate_guide_intersections`):** Computes exact pairwise intersection points between non-parallel guides for construction-point snapping.
- **Screen Hairlines vs. Design-Space Stems (`GuideThickness`):**
  - `GuideThickness::ScreenDp(f32)`: Renders as a constant-width hairline in screen pixels regardless of zoom level.
  - `GuideThickness::FontUnits(f64)`: Renders as a semi-transparent **band in design space** whose visual width scales with viewport zoom (for example, an `80 UPM` stem-thickness reference band). `SnapEngine` automatically snaps to both the **centerline** ($d = 0$) and the **two outer band edges** ($d = \pm \frac{w}{2}$).

---

## 3. Alignment Zones & Node Alignment Badges (`AlignmentZone`, `classify_node_alignment`)

In typeface design, round contours (`'O'`, `'e'`, `'s'`) and pointed apexes (`'A'`, `'V'`) must extend slightly past flat metric lines (`overshoot`, typically `12..20 UPM`) to appear optically aligned with flat-topped letters (`'H'`, `'x'`).

- **`AlignmentZone`:** Defines a `position` (the flat metric reference) and a signed `size` (positive extending upward for top zones like cap-height/x-height, negative extending downward for baseline/descender zones).
- **Live Alignment Badges (`classify_node_alignment`, `AlignmentBadgeMode`):** When an extremum or corner on-curve node lies inside an `AlignmentZone` or directly on a metric line, `grafia` renders a visual badge indicator (`circle` or `diamond`) confirming that the contour's optical overshoot is properly seated inside the zone.

---

## 4. Custom Reference Frames (`CustomReferenceFrame`, `ReferenceBandLine`)

Not every vector design fits the Western Latin baseline/x-height/cap-height triad. Mobile app icon systems use concentric keyline grids, while CJK (Chinese, Japanese, Korean) typefaces use an ideographic **Em-Box** (`ICF` inner body frame + outer body frame).

`CustomReferenceFrame` provides a domain-agnostic spatial scaffold rendered in Pass 1 and integrated directly into `SnapEngine` (`collect_reference_frame_snap_lines`):

- **Outer & Inner Bounding Frames (`outer_bounds`, `inner_bounds` as `RectUpm`):** Models the outer em-square (or icon artboard) and an inset inner safe-zone / ideographic body frame (`second_rect`).
- **Subdivision Grids & Center Crosshairs (`grid_divisions_x`, `grid_divisions_y`, `show_center_lines`):** Divides the reference box into $N_x \times N_y$ equal cells (`compute_subdivision_grid_lines_upm`) with optional horizontal/vertical center crosshairs (`reference_frame_center_upm`).
- **Keyline Circles (`circle_radii_upm`):** Renders concentric reference circles centered in the frame for icon keyline geometry and radial symmetry checks.
- **Stem Reference Bands (`band_lines: Vec<ReferenceBandLine>`):** Horizontal or vertical reference lines with a design-space `thickness_upm` that scale with viewport zoom and expose both their centerline and outer band edges ($\pm \frac{1}{2}\text{thickness}$) as magnetic snap targets.
