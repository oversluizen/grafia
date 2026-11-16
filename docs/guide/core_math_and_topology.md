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

---

## 7. Precision Curve Harmonization ($G^2 / G^3$), Tangent Balance (Tunni) Lines & Outline Audit (`v0.10.0`)

### 7.1 Signed Curvature $\kappa(t)$ & Romer/Henestrosa $G^2 / G^3$ Harmonization (`cubic_signed_curvature`, `harmonize_selected_nodes`)

Two cubic Bézier segments $(Q_0, Q_1, Q_2, P)$ and $(P, R_1, R_2, R_3)$ meeting at a `NodeType::Smooth` anchor $P$ share a collinear tangent ($G^1$ continuity), but can still exhibit a visible jump in curvature $\kappa(t)$:

$$\kappa(t) = \frac{x'(t) y''(t) - y'(t) x''(t)}{\left(x'(t)^2 + y'(t)^2\right)^{3/2}}$$

At the join $P$, the endpoint curvature magnitudes depend on the adjacent handle lengths $a = \|P - Q_2\|_2$, $b = \|R_1 - P\|_2$, and the perpendicular distances $d_{\text{in}}, d_{\text{out}}$ of the far handles $Q_1, R_2$ to the common tangent line:

$$|\kappa_{\text{in}}(1)| = \frac{2 \, d_{\text{in}}}{3 \, a^2}, \qquad |\kappa_{\text{out}}(0)| = \frac{2 \, d_{\text{out}}}{3 \, b^2}, \qquad p_0 = \frac{a}{b} = \sqrt{\frac{d_{\text{in}}}{d_{\text{out}}}}$$

- **`harmonize_selected_nodes` (`Ctrl+Shift+H`):** Relocates $P$ along $[Q_2, R_1]$ to $P' = \frac{Q_2 + p_0 R_1}{1 + p_0}$ so $\kappa_{\text{in}}(1) = \kappa_{\text{out}}(0)$ ($G^2$).
- **`harmonize_handles_keep_node` & `set_node_genius`:** Balances $(a, b)$ while keeping $P$ stationary, or enforces live $G^2$ continuity during interactive handle drags.
- **4-Level Continuity Classifier (`classify_node_continuity`) & $G^3$ Clothoid Lock (`harmonize_selected_nodes_g3`, `set_node_g3_lock`):** Evaluates `ContinuityGrade::{ G0Corner, G1Tangent, G2Curvature, G3Clothoid }` and equalizes the arc-length rate of change of curvature $\frac{d\kappa}{ds} = \frac{\kappa'(t)}{\|B'(t)\|_2}$ across smooth joins.
- **Live 120 fps `CurvatureComb` (`compute_curvature_comb_overlay`):** Evaluates normal comb teeth $C_{\text{tip}}(t) = B(t) + s_{\text{comb}} \kappa(t) \hat{n}(t)$, inflection zero-crossings ($\kappa(t) = 0$), and $|d\kappa/ds|$ dips.

### 7.2 Interactive Tangent Balance (Tunni) Lines, Proportional Handle Servo & Neighbour Nudge

- **Tangent Balance (Tunni) Lines (`compute_tunni_overlays`, `scale_segment_tunni_ratio`, `balance_segment_tunni`):** Connects the two off-curve handles $[P_1, P_2]$ with balance midpoint $M_{\text{tunni}} = \frac{1}{2}(P_1 + P_2)$ whenever the tangent rays intersect forward at apex $T$. Dragging scales both handles along their tangent rays within $[0.20, 0.95]$ (`TUNNI_MIN_RATIO .. TUNNI_MAX_RATIO`), and balancing equalizes $t_1 = t_2 = \bar{t}$ so $[P_1, P_2] \parallel [P_0, P_3]$.
- **Proportional Handle Servo (`servo_translate_on_curve_node`) & Interpolated Neighbour Nudge (`power_nudge_translate_selected`, Shortcut `C`):** Preserves dimensionless off-curve handle ratios along each axis when an extremum moves, and linearly interpolates intermediate unselected contour nodes between translated stems and stationary contour extrema.

### 7.3 Master Trajectory Kinks, Auto-Matching & Proactive Vector Outline Audit

- **Cross-Master Kink & Crossing Detection (`compute_master_kinks_and_crossings`) & Auto-Matching (`auto_match_master_compatibility`):** Detects crossed linear point trajectories $\vec{T}_j(s) \cap \vec{T}_k(u) \neq \emptyset$, flags `NodeType::Smooth` joins whose cross-master tangent angle ($\Delta\theta > 1^\circ$) and handle-length ratio ($|r_A - r_B| > 0.05$) simultaneously diverge, and automatically repairs contour order, winding parity, and cyclic `Node 0` alignment via least-squares registration.
- **Proactive Vector Outline Audit (`audit_layer_outlines`), Handle Warnings & Virtual Apex Snapping:** Audits contours for `MissingExtremum` (including italic-slanted extrema), `QuasiOrthogonalStem` ($0.1^\circ \le \Delta\theta \le 2.0^\circ$), `CrossedHandles`, and `CollinearCubic` ($\le 0.05\;\text{UPM}$), flags slightly tilted extremum handles (`compute_angled_handle_warnings`), reports apex handle-length percentages (`compute_segment_handle_ratios`), and snaps truncated diagonal stems (`/A`, `/V`, `/M`, `/K`) to infinite ray extensions or virtual ray-ray apex intersections (`compute_tangent_extension_snaps`).

---

## 8. Non-Destructive Smart Corners, Ink Traps, Calligraphic Brush, Stem Gauge & Green's Theorem Counter Balance (`v0.11.0`)

### 8.1 Non-Destructive Smart Corners & Ink Traps (`SmartCornerSpec`, `compute_smart_corner_preview_contours`, `bake_smart_corners`)

Each `SmartCornerSpec { node_id, radius_upm }` attaches a parametric corner modifier to an on-curve vertex $V$ with incoming unit edge vector $\hat{u}_{\text{in}}$, outgoing unit edge vector $\hat{u}_{\text{out}}$, and interior angle $\theta = \arccos(\hat{u}_{\text{in}} \cdot \hat{u}_{\text{out}})$:

- **Positive Fillet (`radius_upm > 0.0`):** Replaces $V$ non-destructively with a 4-node tangent circular-arc cubic $(T_{\text{in}}, H_{\text{in}}, H_{\text{out}}, T_{\text{out}})$ clamped to $0.48 \cdot \min(L_{\text{in}}, L_{\text{out}})$ with handle factor $\kappa_{\text{arc}} = \frac{4}{3}\tan\!\left(\frac{\pi - \theta}{4}\right)$.
- **Negative Ink Trap Notch (`radius_upm < 0.0`):** Cuts a 4-vertex inward recess $(T_{\text{in}}, N_{\text{in}}, N_{\text{out}}, T_{\text{out}})$ of depth $D = |R|$ along the interior angle bisector $\hat{b} = \frac{\hat{u}_{\text{in}} + \hat{u}_{\text{out}}}{\|\hat{u}_{\text{in}} + \hat{u}_{\text{out}}\|_2}$ to prevent optical ink crowding in acute joins.
- **`bake_smart_corners`:** Atomically commits the evaluated preview contours into `glyph.contours` inside a reversible Undo transaction.

### 8.2 W3C Stylus Calligraphic Brush, Curvature Heatmap Envelope & Inflection Solver

- **Pressure & Tilt Calligraphic Brush (`CalligraphicNibSpec`, `CalligraphicSample`, `fit_calligraphic_brush_stroke`, `apply_calligraphic_brush_stroke`):** Maps `W3C Pointer Events Level 3` stylus streams `(pos, pressure, tilt_x, tilt_y)` into half-width $h_i = \frac{1}{2}(w_{\min} + p_i(w_{\max} - w_{\min}))$ and instantaneous nib orientation $\phi_i = \phi_0 + \alpha_{\text{tilt}} \operatorname{atan2}(\theta_y, \theta_x)$, constructing a closed cubic Bézier envelope contour.
- **Curvature Heatmap Envelope & Inflection Roots (`CurvatureCombStyle::HeatmapEnvelope`, `compute_curvature_heatmap_envelopes`, `find_cubic_inflection_points`):** Solves the cross-product quadratic $(B_d \times C)t^2 + (A \times C)t + (A \times B_d) = 0$ for exact interior cubic inflection points $t \in (0, 1)$ and builds closed normalized curvature envelopes ($|\kappa(t)| / \kappa_{\max} \in [0, 1]$).

### 8.3 Orthogonal Stem Gauge, Green's Theorem Counter Balance & Lamé Superellipse / Symmetry Overlays

- **Orthogonal Stem Thickness Gauge (`measure_orthogonal_stem_thickness`, `StemGaugeMatch`):** Casts an inward unit normal ray $R(s) = p_{\text{entry}} + s\,\hat{N}_{\text{in}}$ across the filled interior to the opposite wall and classifies perpendicular stem thickness $d_\perp$ against `reference_stems_upm` within `stem_tolerance_upm` (`Matched`, `Thinner`, `Thicker`).
- **Closed-Form Green's Theorem Area & Visual Centroid $\oplus$ (`compute_contour_area_and_centroid`, `compute_layer_counter_balance`):** Evaluates exact 5-point Gauss-Legendre line integrals for signed area $A = \frac{1}{2}\oint (x\,dy - y\,dx)$ and visual center of mass $(C_x, C_y)$, separating outer black stems from inner white counters and reporting `black_to_white_ratio`.
- **Lamé Superellipse Generator & Optical Symmetry Ghost Overlays (`build_superellipse_contour_nodes`, `SymmetryOverlayMode`, `compute_symmetry_ghost_contours`):** Generates 12-node closed superellipse/squircle contours for $\left|\frac{x-c_x}{a}\right|^n + \left|\frac{y-c_y}{b}\right|^n = 1$ ($n \in [2, 20]$) and projects live horizontal, vertical, or $180^\circ$ point-inverted ghost contours around the layer bounding-box center.

---

## 9. Interactive Structural `Stem` Tool, Curved Master Trajectories, Local Brushes & Scanline Optical Rhythm Integrals (`v0.12.0`)

### 9.1 Paired-Wall Structural `Stem` Detection, Servo-Coupled Editing & Measurement Rainbow (`detect_structural_stems`, `adjust_structural_stem`)

- **Anti-Parallel Wall Pairing (`detect_structural_stems_in_contours`):** Pairs opposing straight or quasi-linear contour walls $(S_L, S_R)$ whose inward unit normals satisfy $\hat{n}_L \cdot \hat{n}_R \le -\cos 12^\circ$ with mutual tangential projection overlap and zero intervening contour intersections, classifying each stem into `StemOrientation::{ Horizontal, Vertical, Diagonal }` with exact perpendicular thickness $d_\perp = |(P_R - P_L) \cdot \hat{n}_L|$.
- **Servo-Coupled Thickness & Angle Editing (`adjust_structural_stem`):** Translates paired stem walls along $\pm\hat{n}_{LR}$ under `StemAdjustmentMode::{ SymmetricCenter, LockLeft, LockRight }` while proportionally scaling incident `OffCurve` handles on connected arches (`servo_translate_node_in_contours`) so smooth transitions remain $G^1$-collinear without kinks, and optionally emits `CanvasHostRequest::PropagateStemDelta`.
- **Measurement Rainbow Heatmap (`compute_measurement_rainbow_quads`, `set_show_measurement_rainbow`):** Shades interior cross-sections between opposing contour walls in Pass 2 with a continuous spectral ramp mapped to local perpendicular thickness $d_\perp(s)$.

### 9.2 Non-Linear Quadratic & Cubic Master Trajectories (`CurvedTrajectoryLink`, `evaluate_curved_master_point`)

When a stroke rotates or shifts diagonally between two compatible masters $(P^{(A)}, P^{(B)})$, linear interpolation $(1-t)P^{(A)} + t P^{(B)}$ can cause optical stem weight to sag at intermediate weights $t \in (0, 1)$. `CurvedTrajectoryLink` replaces the straight chord with a quadratic $B_2(t)$ (`control2 == None`) or cubic $B_3(t)$ (`control2 == Some(C_2)`) Bézier trajectory:

$$B_2(t) = (1-t)^2 P^{(A)} + 2(1-t)t C_1 + t^2 P^{(B)}, \qquad B_3(t) = (1-t)^3 P^{(A)} + 3(1-t)^2 t C_1 + 3(1-t)t^2 C_2 + t^3 P^{(B)}$$

### 9.3 Windowed `Trowel` Smoothing & `Cleaner` Micro-Loop Pruning Brushes (`apply_trowel_brush_at_upm`, `apply_cleaner_brush_at_upm`)

- **`CanvasTool::Trowel` (`apply_trowel_brush_in_contours`):** Collects only the contiguous sub-chain of nodes intersecting the circular brush disc $\mathcal{B}(c, R_{\text{upm}})$ and refits/simplifies that interior window within `fit_tolerance_upm` while keeping the boundary entry/exit nodes and every node $P_k \notin \mathcal{B}(c, R_{\text{upm}})$ 100% bit-identical ($\Delta P_k \equiv 0$).
- **`CanvasTool::Cleaner` (`apply_cleaner_brush_in_contours`):** Sweeps over $\mathcal{B}(c, R_{\text{upm}})$ to eliminate near-duplicate nodes, self-intersecting micro-loops with $|A(\gamma_{\text{loop}})| < \text{max\_loop\_area\_upm2}$, and collinear or hairpin-spike redundancies inside one atomic `"Clean Contour"` Undo step.

### 9.4 Scanline Optical Inter-Shape Rhythm Quadrature & Clean 1-Axis Live Mirror (`compute_pair_optical_whitespace`, `ActiveMirrorAxis`)

Mechanical sidebearings measure only the single closest bounding-box extremum, ignoring how convex bowls (`'O'`) recede away from the sidebearing while flat stems (`'H'`) occupy the full vertical height. `compute_pair_optical_whitespace` casts $N$ italic-sheared horizontal scanlines $y_k \in [y_{\min}, y_{\max}]$ across adjacent shapes $(G_L, G_R)$, clamps deep open counters (such as `'L'`, `'T'`, `'c'`) to `max_cavity_depth_upm` ($d_{\max}$):

$$x_{L,\text{exit}}(y) = \max\!\big(x_{L,\text{raw}}(y),\; X_{\max}^{(L)} - d_{\max}\big), \qquad x_{R,\text{entry}}(y) = \min\!\big(x_{R,\text{raw}}(y),\; X_{\min}^{(R)} + d_{\max}\big)$$

and integrates the bounded inter-contour optical channel volume via trapezoidal quadrature:

$$V_{\text{opt}} = \int_{y_{\min}}^{y_{\max}} \max\!\big(0,\; x_{R,\text{entry}}(y) - x_{L,\text{exit}}(y)\big)\,dy$$

while reporting the exact mechanical collision distance `min_clearance_upm` and `scanline_spans` for Pass-1 translucent "water-volume" rhythm overlays (`set_show_optical_rhythm_fill`). Combined with `ActiveMirrorAxis::{ Vertical, Horizontal }` (`reflect_contour_across_mirror_axis`, `add_contour_with_active_mirror`), live drawing strokes automatically emit winding-corrected symmetric contours in a single Undo step.
