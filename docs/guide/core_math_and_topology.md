# Core Mathematical, Spline, Topology & Auto-Trace Engine (`grafia::core`)

This module provides the pure, deterministic 2D mathematical foundation of `grafia`. Every algorithm in `grafia::core` operates on strongly-typed coordinates without coupling to any windowing system or disk format, making it equally suitable for interactive `egui` viewports, CLI batch pipelines, and headless verification suites.

---

## 1. Coordinate Spaces (`FontPoint` vs. `ScreenPoint`)

Vector design and typography use a **Cartesian Y-up** coordinate system (`FontUnit`, `FontPoint`), whereas GPU framebuffers and `egui` use a **Top-Left Y-down** screen coordinate system (`ScreenDp`, `ScreenPoint`).

- **`FontPoint` (`FontUnit` in `f64`):** High-precision design-space coordinates (often called UPM — *Units Per Em*, typically `1000` or `2048` units per em-square, though any positive scale is supported). Increasing $y$ moves **upward** from the baseline ($y = 0$) toward the cap height.
- **`ScreenPoint` (`ScreenDp` in `f32`):** Logical device-independent pixel coordinates inside the `egui` viewport. Increasing $y$ moves **downward** from the top-left corner of the widget.
- **`ViewportTransform`:** Performs the exact bijective affine mapping between `FontPoint` and `ScreenPoint`, accounting for viewport zoom $s$, pan offset $(t_x, t_y)$, and optional italic slant angle $\theta$:

$$x_{\text{screen}} = t_x + s \cdot x_{\text{font}}, \qquad y_{\text{screen}} = t_y - s \cdot y_{\text{font}}$$

Because `FontPoint` and `ScreenPoint` are distinct Rust newtypes, the compiler prevents accidentally adding a screen pixel offset to a design-space Bézier node.

---

## 2. Choosing the Right Curve: Visual Intuition & CAGD Mathematics

A `GlyphContour` is a sequence of `Node` items. Unlike basic path libraries that only store raw cubic segments, `grafia` supports **mixed-spline contours** where standard cubic Béziers, tension-controlled Hobby splines, and $G^2$ Euler/Spiro spirals coexist and convert cleanly into cubic `kurbo::BezPath` segments via `contour_to_bezpath_with_splines`.

### 2.1 Cubic Bézier (`NodeType::Corner`, `NodeType::Smooth`, `NodeType::OffCurve`)

A cubic Bézier segment is defined by two on-curve endpoints ($P_0, P_3$) and two off-curve control handles ($P_1, P_2$) evaluated over parameter $t \in (0, 1)$ using the Bernstein basis:

$$B(t) = (1 - t)^3 P_0 + 3(1 - t)^2 t P_1 + 3(1 - t) t^2 P_2 + t^3 P_3$$

- **`NodeType::Corner`:** Incoming and outgoing off-curve handles move independently ($G^0$ positional continuity), producing a sharp corner or cusp.
- **`NodeType::Smooth`:** Incoming and outgoing off-curve handles are kept strictly collinear across the on-curve anchor ($G^1$ tangent continuity via `align_collinear_opposite`), ensuring a smooth transition without a corner kink.
- **Extremum Placement (`add_extremes_to_contours`):** In typeface and clean vector engineering, on-curve nodes are placed at horizontal and vertical extrema where $\frac{dx}{dt} = 0$ or $\frac{dy}{dt} = 0$ so bounding boxes and stem hints align with explicit on-curve anchors.

### 2.2 Superellipse / Fit Curve (`FitCurveConfig`, `57%..92%` vs. Circle Constant $\kappa$)

When two orthogonal handles of length $L$ emanate from a quarter-turn corner of radius $R$, the ratio $\tau = L / R$ controls the **shoulder fullness** (superellipse exponent) of the curve:

- **Circular Arc ($\kappa = \frac{4}{3}(\sqrt{2} - 1) \approx 0.55228475$):** Approximates a true circle within $0.027\%$ radial error, used by `build_circle_contour_nodes`.
- **Superellipse / Squircle Range (`57%..92%` via `FitCurveConfig`):** Increasing handle ratios from `0.57` toward `0.92` pushes the curve's belly outward toward the intersection of the two tangent rays, eliminating the optical "flat spot" where a circular arc meets a straight stem. `apply_fit_curve_ratio_to_contours` and `apply_fit_curve_delta_to_contours` (exposed on `GrafiaCanvasState` as `fit_selected_curves_step` / `fit_selected_curves_ratio`) provide 8 calibrated steps across this range.

### 2.3 Tension-Controlled Hobby Splines (`NodeType::HobbyStar`)

Drawing smooth curves by manually dragging two off-curve handles per segment can be tedious. **John Hobby's interpolating spline** (originally created for METAFONT) lets you place **only on-curve points** (`NodeType::HobbyStar`) while `grafia` automatically computes the ideal off-curve cubic Bézier handles (`evaluate_hobby_chain_to_cubics`).

- **How it works:** Across a chain of `HobbyStar` nodes, `grafia` solves a cyclic tridiagonal linear system that equalizes *mock curvature* on both sides of every node, then converts the resulting tangent angles $\theta_i, \phi_i$ into cubic handle lengths using Hobby's velocity function (`hobby_velocity_rho`):

$$\rho(\theta, \phi) = \frac{2 + \sqrt{2}\left(\sin\theta - \frac{1}{16}\sin\phi\right)\left(\sin\phi - \frac{1}{16}\sin\theta\right)(\cos\theta - \cos\phi)}{1 + \frac{1}{2}(\sqrt{5} - 1)\cos\theta + \frac{1}{2}(3 - \sqrt{5})\cos\phi}$$

- **Visualizing `tension_in` and `tension_out` ($\tau \in (0.75, 4.0)$):**
  - $\tau = 1.0$: Produces a natural, round, circular-like arc through the points.
  - $\tau > 1.0$ (up to $4.0$): Tightens the curve, pulling the cubic handles shorter so the path hugs the straight polygon chords with brisk, high-curvature turns near the nodes.
  - $\tau \approx 0.75$: Relaxes the curve into a fuller, wider arch.
- **Directional Constraints (`HobbyDirConstraint`):** Lock the tangent at any Hobby node to `Free`, `Horizontal`, `Vertical`, `SlantedItalic`, or `FollowMetricLine`.

### 2.4 Euler / Spiro ($G^2$) Clothoid Splines (`NodeType::EulerSpiro`)

Even when two cubic segments share a collinear tangent ($G^1$ continuity), the **radius of curvature** can jump abruptly across the node, which the human eye perceives as a subtle flat-to-round inflection on glossy industrial surfaces or large display typography.

- **`NodeType::EulerSpiro` (`evaluate_euler_spiro_chain_to_cubics`):** Solves a damped Newton-Raphson system so that **curvature $\kappa(s)$ varies linearly with arc length $s$** ($G^2$ curvature continuity — an Euler spiral or clothoid) across every interior node, then fits high-accuracy cubic Bézier segments to the solved spiral.

| Curve Representation | What You Place | Continuity Guarantee | Best Used For |
| :--- | :--- | :--- | :--- |
| **Cubic Bézier (`Corner` / `Smooth`)** | On-curve anchors + 2 off-curve handles | $G^0$ or $G^1$ | Direct fine-grained control, final font outlines, SVG export |
| **Hobby Spline (`HobbyStar`)** | On-curve nodes + tension $\tau$ | Mock-$G^2$ with tension control | Fast calligraphic sketching, adjustable curve tightness |
| **Euler / Spiro (`EulerSpiro`)** | On-curve nodes only | True $G^2$ (continuous curvature) | Industrial CAD profiles, ultra-smooth logos, expressive display curves |

---

## 3. Direct Curve Manipulation (`apply_cubic_segment_belly_drag` & `heal_delete_nodes`)

### 3.1 Four-Case Inverse-Bernstein Belly Dragging (`apply_cubic_segment_belly_drag`, `apply_cubic_segment_belly_drag_with_alt`)

When a user clicks directly on a cubic curve segment at parameter $t \in (0, 1)$ and drags it by a displacement vector $\Delta P$, `grafia` keeps the on-curve endpoints $P_0, P_3$ fixed and solves for the handle adjustments $\Delta P_1, \Delta P_2$ that move $B(t)$ by $\Delta P$ according to the Bernstein basis weights $w_1 = 3(1-t)^2 t$ and $w_2 = 3(1-t)t^2$:

1. **Both endpoints `Corner` (Free-Free):** Distributes displacement proportionally between $P_1$ and $P_2$ along the drag vector while preserving handle angles when appropriate.
2. **One `Smooth`, one `Corner`:** Constrains the `Smooth` endpoint's handle to slide strictly along its existing tangent ray (preserving $G^1$ continuity with the adjacent segment) while the `Corner` handle absorbs the remaining vector component.
3. **Both endpoints `Smooth` (Locked Tangents):** Projects $\Delta P$ onto the two fixed tangent directions $(u_1, u_2)$ and solves the $2\times 2$ linear system $w_1 \alpha_1 u_1 + w_2 \alpha_2 u_2 = \Delta P$ so dragging the belly never breaks tangent smoothness on neighboring segments.
4. **Degenerate / Collinear Tangents:** Falls back to minimum-norm least-squares distribution without numerical blow-up.

### 3.2 De Casteljau Splitting & Curve-Healing Deletion

- **Exact Subdivision (`split_contour_segment`):** Splits a cubic segment at any $t \in (0, 1)$ into two geometrically identical cubic segments using De Casteljau's triangle interpolation.
- **Curve-Healing Node Deletion (`heal_delete_nodes`):** Removing an interior `Smooth` on-curve node between two cubic segments does not collapse the path into a straight line; instead, `grafia` reconstructs a single cubic Bézier that best approximates the combined two-segment trajectory.

---

## 4. Topology, Winding Direction & Boolean Pathfinder

### 4.1 Signed Area & Non-Zero Winding (`compute_signed_area`, `correct_path_direction_layer`)

In vector typography and `FillRule::NonZero` rendering, the **winding direction** of a closed contour determines whether it adds ink (an outer shape) or subtracts a hole (an inner counter, such as the inside of an `'O'` or `'A'`):

$$A = \frac{1}{2} \oint (x \, dy - y \, dx)$$

- **Counter-Clockwise (`CCW`, $A > 0$ in Y-up `FontPoint` space):** Outer contour (fills solid).
- **Clockwise (`CW`, $A < 0$ in Y-up `FontPoint` space):** Inner counter / cutout hole (subtracts from the enclosing outer contour).
- **`correct_path_direction_layer`:** Computes the containment nesting depth of all closed contours in a layer using point-in-polygon winding tests and automatically reverses any contour whose signed area does not match its even/odd nesting depth.

### 4.2 Boolean Pathfinder & Knife Slicing

- **`remove_overlap_in_contours` (Union), `subtract_contours` (Difference), `intersect_contours` (Intersection):** Finds all pairwise cubic-cubic and line-cubic intersections via recursive Bézier fat-line / subdivision clipping, splits segments at intersection parameters, builds a planar half-edge graph, prunes interior edges by Non-Zero winding number, and extracts clean outer and counter rings.
- **`knife_cut_paths_in_layer` & `knife_add_nodes_in_layer`:** Intersects an arbitrary slice line with all contours in the layer, either slicing closed rings into independent closed pieces or inserting on-curve nodes at every intersection point.

---

## 5. The 7-Stage Deterministic Bitmap Auto-Trace Pipeline (`trace_background_image`)

`trace_background_image` converts a raster `CanvasBackgroundImage` (from clean monochrome icons to unevenly lit smartphone photos of paper sketches) into editable cubic Bézier `GlyphContour`s through seven deterministic stages:

1. **Stage 1 — Grayscale & Binarization (`AutoTraceThresholdMode`):**
   - `GlobalLuminance`: Fast fixed threshold (`threshold` $\in (0, 255)$) for clean digital bitmaps.
   - `AdaptiveLocal` (**Sauvola Integral-Image Thresholding**): Computes local mean $\mu(x,y)$ and standard deviation $\sigma(x,y)$ in $O(1)$ per pixel using summed-area tables over `adaptive_window_radius`, cleanly separating ink from paper even under heavy camera shadows:

$$T(x, y) = \mu(x, y) \cdot \left(1 + k \cdot \left(\frac{\sigma(x, y)}{R} - 1\right)\right)$$

2. **Stage 2 — Perspective Deskew ($3\times 3$ Homography):** When `perspective_quad` is provided, solves an $8\times 8$ projective homography system with bilinear resampling to rectify tilted camera photos before tracing.
3. **Stage 3 — Morphological Majority Despeckling:** Eliminates isolated single-pixel salt-and-pepper sensor noise.
4. **Stage 4 — Marching Squares Iso-Contour Extraction:** Extracts closed sub-pixel boundary loops with deterministic saddle-point disambiguation and signed area winding (`CCW` outer ink, `CW` inner holes).
5. **Stage 5 — Area & Perimeter Speckle Filtering (`min_area_px`, `min_perimeter_px`):** Discards dust specks smaller than the configured pixel thresholds.
6. **Stage 6 — Ramer-Douglas-Peucker (`RDP`) Polyline Simplification (`rdp_tolerance_px`):** Reduces dense pixel staircases into key vertices while preserving sharp corners (`corner_angle_threshold_deg`).
7. **Stage 7 — Philip Schneider Least-Squares Cubic Bézier Fitting (`curve_fit_error_px`):** Iteratively fits $G^1$-smooth cubic Bézier curves to each smooth vertex chain using chord-length parameterization and Newton-Raphson parameter re-indexing, splitting at maximum-error vertices until the distance error is below `curve_fit_error_px`.

### Tuning `AutoTraceConfig` for Your Input

| Input Source | `threshold_mode` | `rdp_tolerance_px` | `curve_fit_error_px` | `corner_angle_threshold_deg` |
| :--- | :--- | :--- | :--- | :--- |
| **Crisp high-res icon / logo PNG** | `GlobalLuminance` (`threshold: 128`) | `0.8 .. 1.2` | `1.0 .. 2.0` | `55.0 .. 65.0` |
| **Hand-drawn pencil/ink scan** | `AdaptiveLocal` (`sauvola_k: 0.20`) | `1.5 .. 2.5` | `2.5 .. 4.0` | `60.0` |
| **Shadowed smartphone photo** | `AdaptiveLocal` + `perspective_quad` | `2.0 .. 3.0` | `3.0 .. 5.0` | `65.0` |

---

## 6. Multi-Glyph Text Streams, Hinting & Higher-Order Interpolation (`HOI`)

- **Horizontal & Vertical Text Streams (`CanvasTextStream`, `WritingDirection`):** Supports `LeftToRight`, `RightToLeft`, and vertical CJK `TopToBottom` (`recompute_vertical_slot_origins` centering glyphs on $x_c = \frac{1}{2}W$ and advancing downward by `advance_height` + vertical pair kerning). Keyboard adjustments emit `SpacingDelta` and `KerningDelta` on `MetricsAxis::Horizontal` or `MetricsAxis::Vertical`.
- **PostScript & TrueType Hinting (`PostScriptHint`, `TrueTypeVisualInstruction`):** Models horizontal/vertical stem hints, ghost hints (`20`/`21` UPM), and TrueType grid-fitting instructions (`Snap`, `Stem`, `Shift`, `Interpolate`, `Delta`) with live PPEM rounding grids (`truetype_upm_per_px`).
- **Higher-Order Interpolation (`HoiCubicLayerSet`, `compute_hoi_overlay_geometry`):** In standard variable fonts, point coordinates interpolate linearly between masters, which can cause stems to kink or narrow at midpoint weights when rotating shapes. **Higher-Order Interpolation (`HOI`)** treats master coordinates as control points of a **non-linear cubic Bézier trajectory across design space** ($P(u) = (1-u)^3 M_0 + 3(1-u)^2 u M_1 + 3(1-u) u^2 M_2 + u^3 M_3$), visualized on canvas via control-polygon polylines and `prev`/`current`/`next` timing diamonds.
