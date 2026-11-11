---
type: specification
id: SPEC-010
version: 0.9.0
status: normative
parent: SPEC-001
---

# SPEC-010: `grafia v0.9.0` — Non-Destructive Smart Corners, Ink Traps, Calligraphic Brush, Curvature Heatmap Envelope, Orthogonal Stem Gauge, Green's Theorem Counter Balance & Superellipse / Symmetry Overlays

> **Normative References**:
> - `SPEC-001`: `grafia v0.1.0` — Architecture, Coordinate Pipeline & Token System
> - `SPEC-002`: `grafia v0.2.0` — Bézier Topology, Selection & Transform Box
> - `SPEC-008`: `grafia v0.7.0` — Background Layer, 2D Anchors, Pre-Resolved Components, Fit Curve, Live Contour Stroke & Master Compatibility
> - `SPEC-009`: `grafia v0.8.0` — Precision Curve Harmonization, Tangent Balance Lines, Handle Servo, Neighbour Nudge & Outline Audit
> - Standards: `ISO/IEC/IEEE 42010:2011` (Architecture Description), `ISO/IEC/IEEE 29119-3:2021` (Test Documentation), `IEEE 1012-2016` (System & Software Verification and Validation), `W3C Pointer Events Level 3` (`pressure`, `tiltX`, `tiltY`), `SemVer 2.0.0`
> - Mathematical Literature: Gabriel Lamé (*Superellipse Curves*, 1818), George Green (*Integral Theorem for Planar Domains*, 1828), Gerald Farin (*Curves and Surfaces for CAGD*)

---

## 1. Normative Mathematical & CAGD Foundations

All parametric corner fillets, ink-trap wedges, stylus brush envelopes, curvature heatmaps, normal ray-cast stem gauges, Green's theorem area/centroid integrals, and Lamé superellipse equations in `SPEC-010` are grounded in Computer-Aided Geometric Design (CAGD) and `W3C Pointer Events Level 3`:

| Subsystem | Domain Standard / Mathematical Model | Core Invariant / Equation | Verified Canvas Semantics |
| :--- | :--- | :--- | :--- |
| **1. Non-Destructive Smart Corners & Ink Traps** | Parametric Tangent Circular Arc Fillet & Angle-Bisector Notch | $d_{\text{trim}} = \min\!\big(\frac{R}{\tan(\theta/2)}, 0.48 L\big)$, $\;\kappa_{\text{arc}} = \frac{4}{3}\tan\!\big(\frac{\pi-\theta}{4}\big)$ | Evaluates live circular-arc cubic fillets (`radius_upm > 0`) and inward angle-bisector ink-trap notches (`radius_upm < 0`) non-destructively in Pass 2, with atomic destructive commit via `bake_smart_corners()`. |
| **2. Pressure & Tilt Calligraphic Brush** | `W3C Pointer Events Level 3` Broad-Nib Envelope Offset | $h_i = \frac{1}{2}(w_{\min} + p_i(w_{\max}-w_{\min}))$, $\;\phi_i = \phi_0 + \alpha_{\text{tilt}}\operatorname{atan2}(\theta_y, \theta_x)$ | Maps stylus `(pos, pressure, tilt_x, tilt_y)` samples into left/right cross-section rails $L_i, R_i$ and fits a closed cubic Bézier contour inside an atomic Undo transaction. |
| **3. Live Curvature Comb, Heatmap Envelope & Inflection Markers** | Differential Geometry Signed Curvature & Cross-Product Inflection Roots | $\kappa(t) = \frac{x'y'' - y'x''}{(x'^2+y'^2)^{3/2}}$, $\;x'(t)y''(t) - y'(t)x''(t) = 0$ | Renders normal curvature spines (`CurvatureCombStyle::SpinesOnly`) or closed normalized intensity polygons (`CurvatureCombStyle::HeatmapEnvelope` with $|\kappa(t)|/\kappa_{\max}$), and solves exact interior cubic inflection points (`find_cubic_inflection_points`). |
| **4. Live Orthogonal Stem Thickness Band & Reference Stem Gauge** | Inward Unit Normal Ray-Casting & Reference Stem Tolerance Classification | $R(s) = p_{\text{entry}} + s\,\hat{N}_{\text{in}}$, $\;|d_\perp - S^*| \le \tau_{\text{stem}}$ | Casts an inward normal ray from any contour point across the filled interior to the opposite wall, measures perpendicular thickness $d_\perp$, and classifies against `reference_stems_upm` within `stem_tolerance_upm` (`Matched`, `Thinner`, `Thicker`). |
| **5. Closed-Form Green's Theorem Counter Area & Visual Centroid $\oplus$** | Planar Line Integral of Signed Area & First Moments | $A = \frac{1}{2}\oint (x\,dy - y\,dx)$, $\;(C_x, C_y) = \frac{1}{6A}\big(\oint x^2 dy, -\oint y^2 dx\big)$ | Computes exact signed area and visual center of mass $\oplus$ for closed cubic/line contours, separating outer black stems from inner white counters and reporting `black_to_white_ratio`. |
| **6. Superellipse / Squircle Generator & Optical Symmetry Ghost Overlay** | Lamé Curve $\big|\frac{x-c_x}{a}\big|^n + \big|\frac{y-c_y}{b}\big|^n = 1$ & Reflection/Point-Inversion Group | $\kappa(n) = \frac{2^{-1/n} - 0.125}{0.75}$, $\;\mathcal{R}_x, \mathcal{R}_y, \mathcal{I}_{(m_x,m_y)}$ | Generates closed superellipse contours with tunable Lamé exponent $n \ge 2.0$ and renders live reflected or $180^\circ$-point-inverted ghost contours (`SymmetryOverlayMode`) to inspect optical balance. |

---

## 2. Normative Mathematical & Functional Requirements

### 2.1 Non-Destructive Smart Corners & Ink Traps (`SmartCornerSpec` — `src/core/smart_corners_brush.rs`)

1. **Smart Corner Specification (`SmartCornerSpec { node_id: NodeId, radius_upm: f64 }`)**:
   - Stored in `state.smart_corners: Vec<SmartCornerSpec>` (keyed uniquely by `node_id` via `set_smart_corner(spec: SmartCornerSpec)` and removed via `remove_smart_corner(node_id: NodeId)`).
   - Applies to any on-curve corner vertex $V$ with incoming unit tangent $\hat{u}_{\text{in}}$ (pointing from $V$ toward the previous vertex $V_{\text{prev}}$) and outgoing unit tangent $\hat{u}_{\text{out}}$ (pointing from $V$ toward the next vertex $V_{\text{next}}$), with interior corner angle $\theta = \arccos(\text{clamp}(\hat{u}_{\text{in}} \cdot \hat{u}_{\text{out}}, -1.0, 1.0)) \in (0, \pi)$.

2. **Positive Radius (`radius_upm > 0.0`) — Tangent Circular / Cubic Fillet**:
   - For $R = \text{radius\_upm} > 0.0$:
     - The ideal tangent trim distance from vertex $V$ along the incoming and outgoing edges is:
       $$d_{\text{trim}} = \min\!\left(\frac{R}{\tan(\theta / 2)},\; 0.48 \cdot L_{\text{in}},\; 0.48 \cdot L_{\text{out}}\right)$$
       where $L_{\text{in}} = \|V - V_{\text{prev}}\|_2$ and $L_{\text{out}} = \|V_{\text{next}} - V\|_2$.
     - The single corner vertex $V$ is replaced in the evaluated contour by a 4-node cubic circular-arc fillet $(T_{\text{in}}, H_{\text{in}}, H_{\text{out}}, T_{\text{out}})$:
       $$T_{\text{in}} = V + d_{\text{trim}}\, \hat{u}_{\text{in}}, \qquad T_{\text{out}} = V + d_{\text{trim}}\, \hat{u}_{\text{out}}$$
       $$H_{\text{in}} = T_{\text{in}} - \kappa_{\text{arc}} \, d_{\text{trim}}\, \hat{u}_{\text{in}}, \qquad H_{\text{out}} = T_{\text{out}} - \kappa_{\text{arc}} \, d_{\text{trim}}\, \hat{u}_{\text{out}}$$
       where $\kappa_{\text{arc}} = \frac{4}{3} \tan\!\left(\frac{\pi - \theta}{4}\right)$ (reducing to $0.55228475$ for a $90^\circ$ corner $\theta = \pi/2$).

3. **Negative Radius (`radius_upm < 0.0`) — Inward Ink Trap Notch**:
   - For $R = \text{radius\_upm} < 0.0$ with depth $D = |R| = -\text{radius\_upm}$:
     - Let $\hat{b} = \frac{\hat{u}_{\text{in}} + \hat{u}_{\text{out}}}{\|\hat{u}_{\text{in}} + \hat{u}_{\text{out}}\|_2}$ be the unit interior angle bisector pointing into the glyph body, and let $d_{\text{lip}} = \min(0.35 \cdot D, 0.30 \cdot \min(L_{\text{in}}, L_{\text{out}}))$.
     - The corner vertex $V$ is replaced by a 4-vertex inward ink-trap recess $(T_{\text{in}}, N_{\text{in}}, N_{\text{out}}, T_{\text{out}})$:
       $$T_{\text{in}} = V + d_{\text{lip}}\, \hat{u}_{\text{in}}, \qquad T_{\text{out}} = V + d_{\text{lip}}\, \hat{u}_{\text{out}}$$
       $$N_{\text{in}} = T_{\text{in}} + D\, \hat{b}, \qquad N_{\text{out}} = T_{\text{out}} + D\, \hat{b}$$
     - This cuts an inward ink-trap notch of depth $D$ into the acute vertex, reducing the enclosed contour area while keeping all edges valid `Line` segments.

4. **Non-Destructive Live Preview vs. Destructive `bake_smart_corners()`**:
   - `compute_smart_corner_preview_contours(&self) -> Vec<GlyphContour>` evaluates all `SmartCornerSpec` entries non-destructively for Pass-2 / `PreviewFilled` rendering while leaving `state.glyph.contours` untouched so the underlying sharp corner node $V$ can still be dragged directly.
   - `bake_smart_corners(&mut self)` destructively commits the evaluated Smart Corner and Ink Trap geometry into `state.glyph.contours` with fresh deterministic `NodeId`s, clears `state.smart_corners`, and records an atomic Undo snapshot.

---

### 2.2 Pressure & Tilt Calligraphic Brush (`CalligraphicNibSpec`, `CalligraphicSample` — `src/core/smart_corners_brush.rs`)

1. **W3C Pointer Events Level 3 Stylus Sample Stream**:
   - Each `CalligraphicSample` carries:
     - `pos: FontPoint`: 2D stylus position in UPM coordinates.
     - `pressure: f64`: normalized stylus pressure in $[0.0, 1.0]$ (clamped if out-of-range).
     - `tilt_x: f64`, `tilt_y: f64`: stylus plane tilt angles in degrees $\in [-90.0, 90.0]$.
   - `CalligraphicNibSpec` defines:
     - `min_width_upm: f64` ($> 0.0$): stroke width at `pressure = 0.0`.
     - `max_width_upm: f64` ($\ge \text{min\_width\_upm}$): stroke width at `pressure = 1.0`.
     - `base_angle_deg: f64`: broad-nib orientation angle in degrees.
     - `tilt_influence: f64` ($\in [0.0, 1.0]$): modulation factor mapping $(\text{tilt\_x}, \text{tilt\_y})$ onto nib rotation.

2. **Left/Right Calligraphic Envelope Construction (`fit_calligraphic_brush_stroke`)**:
   - Given $N \ge 2$ distinct trajectory samples $\{S_0, S_1, \dots, S_{N-1}\}$:
     - For each sample $S_i$:
       1. Clamps $p_i = \text{clamp}(S_i.\text{pressure}, 0.0, 1.0)$ and computes half-width:
          $$h_i = \frac{1}{2}\Big(\text{min\_width\_upm} + p_i \cdot (\text{max\_width\_upm} - \text{min\_width\_upm})\Big)$$
       2. Computes instantaneous nib angle:
          $$\phi_i = \text{deg2rad}(\text{base\_angle\_deg}) + \text{tilt\_influence} \cdot \operatorname{atan2}(S_i.\text{tilt\_y}, S_i.\text{tilt\_x})$$
       3. Computes the effective cross-section offset vector $\vec{\delta}_i = h_i \, (\cos\phi_i, \sin\phi_i)$ (oriented consistently with the stroke tangent $\vec{\tau}_i$ so left and right rails never cross):
          $$L_i = S_i.\text{pos} + \vec{\delta}_i, \qquad R_i = S_i.\text{pos} - \vec{\delta}_i$$
     - Connects the forward left rail $(L_0, \dots, L_{N-1})$ and the reverse right rail $(R_{N-1}, \dots, R_0)$ with smooth cubic Bézier segments (`NodeType::OffCurve`, `NodeType::Smooth`, `NodeType::Corner`) and end caps into a single **closed** `GlyphContour` (`closed == true`).
   - `apply_calligraphic_brush_stroke(&mut self, nib: CalligraphicNibSpec, samples: &[CalligraphicSample]) -> Option<ContourId>` appends the fitted contour to `state.glyph.contours` inside an atomic Undo transaction.

---

### 2.3 Live Curvature Comb, Gradient-Filled Curvature Heatmap Envelope & Inflection Point Markers (`src/core/smart_corners_brush.rs`)

1. **Analytical Curvature Comb Spines (`compute_curvature_comb_spines`)**:
   - Toggled via `toggle_show_curvature_comb()` (`state.show_curvature_comb: bool`).
   - `compute_curvature_comb_spines(&self, samples_per_segment: usize, scale_upm2: f64) -> Vec<Vec<CurvatureCombSpine>>`:
     - For every cubic Bézier segment $(P_0, P_1, P_2, P_3)$ in `state.glyph.contours` and sample index $k \in 0..=\max(\text{samples\_per\_segment}, 2)$ with $t_k = k / N$:
       - Evaluates $B(t_k)$, first derivative $B'(t_k) = (x'(t_k), y'(t_k))$, and signed curvature $\kappa(t_k) = \frac{x' y'' - y' x''}{(x'^2 + y'^2)^{3/2}}$.
       - Computes the outward/left unit normal $\hat{n}(t_k) = \frac{(-y'(t_k),\, x'(t_k))}{\sqrt{x'(t_k)^2 + y'(t_k)^2}}$.
       - Emits `CurvatureCombSpine { base_point: B(t_k), tip_point: B(t_k) + (scale_upm2 * kappa(t_k)) * n(t_k), signed_curvature: kappa(t_k) }`.
2. **Exact Cubic Inflection Point Solver (`find_cubic_inflection_points`)**:
   - Given cubic $(P_0, P_1, P_2, P_3)$ with $A = P_1 - P_0$, $B_d = P_2 - 2P_1 + P_0$, and $C = P_3 - 3P_2 + 3P_1 - P_0$, the vanishing cross-product condition:
     $$x'(t)\,y''(t) - y'(t)\,x''(t) = 18 \Big[(A \times B_d) + (A \times C)\,t + (B_d \times C)\,t^2\Big] = 0$$
     reduces to the scalar quadratic $a t^2 + b t + c = 0$ with $a = B_d \times C$, $b = A \times C$, $c = A \times B_d$.
   - `find_cubic_inflection_points(contour_idx, seg_idx, p0, p1, p2, p3) -> Vec<InflectionMarker>` solves for all distinct real roots $t \in (10^{-5}, 1 - 10^{-5})$ in ascending order where curvature changes sign, returning `InflectionMarker { contour_idx, seg_idx, t, point: B(t) }`.
3. **Curvature Heatmap Envelope (`CurvatureCombStyle { SpinesOnly, HeatmapEnvelope }`, `compute_curvature_heatmap_envelopes`)**:
   - Configured via `set_curvature_comb_style(style: CurvatureCombStyle)`.
   - `compute_curvature_heatmap_envelopes(&self, samples_per_segment: usize, scale_upm2: f64) -> Vec<CurvatureHeatmapEnvelope>` computes for each contour:
     - `samples`: `Vec<CurvatureHeatmapSample>` where each sample carries `normalized_intensity = |\kappa(t)| / \max(\kappa_{\max}, 10^{-9}) \in [0.0, 1.0]` and `polarity_sign = \operatorname{signum}(\kappa(t)) \in \{-1, 0, +1\}`.
     - `closed_polygon`: closed ring of vertices tracing the base curve forward `B(t_0)..B(t_M)` and the displaced curvature envelope backward `tip(t_M)..tip(t_0)`.
     - `inflections`: all `InflectionMarker` points along the contour.

---

### 2.4 Live Orthogonal Stem Thickness Band & Reference Stem Gauge (`measure_orthogonal_stem_thickness` — `src/core/smart_corners_brush.rs`)

1. **Inward Normal Ray-Cast & Stem Thickness Measurement**:
   - Given `(contour_idx, seg_idx, t)` on `state.glyph.contours`:
     - Evaluates entry point $p_{\text{entry}} = B(t)$ and the unit inward normal $\hat{N}_{\text{in}}(t)$ pointing into the filled glyph interior (determined by the contour's signed winding orientation).
     - Casts ray $R(s) = p_{\text{entry}} + s\,\hat{N}_{\text{in}}(t)$ for $s > 10^{-3}$ against all contour segments in `state.glyph.contours` and finds the nearest positive intersection $d_\perp = s_{\min} > 0$ at $p_{\text{exit}} = p_{\text{entry}} + d_\perp\,\hat{N}_{\text{in}}(t)$.
2. **Reference Stem Gauge Classification (`StemGaugeMatch { Matched, Thinner, Thicker }`)**:
   - `state.reference_stems_upm: Vec<f64>` and `state.stem_tolerance_upm: f64` (configured via `set_reference_stems(stems_upm: Vec<f64>, tolerance_upm: f64)`).
   - Selects the closest reference stem $S^* = \arg\min_{S \in \text{reference\_stems\_upm}} |d_\perp - S|$:
     - If $|d_\perp - S^*| \le \text{stem\_tolerance\_upm}$: `StemGaugeMatch::Matched` (green band).
     - If $d_\perp < S^* - \text{stem\_tolerance\_upm}$: `StemGaugeMatch::Thinner` (cyan/blue warning band).
     - If $d_\perp > S^* + \text{stem\_tolerance\_upm}$: `StemGaugeMatch::Thicker` (red/amber warning band).
   - Returns `Some(StemThicknessMeasurement { p_entry, p_exit, thickness_upm: d_perp, nearest_reference_upm, gauge_match })`.

---

### 2.5 Closed-Form Green's Theorem Counter Area & Visual Centroid $\oplus$ (`compute_contour_area_and_centroid`, `compute_layer_counter_balance` — `src/core/smart_corners_brush.rs`)

1. **Exact Planar Area & First-Moment Integrals (`compute_contour_area_and_centroid(nodes: &[Node]) -> (f64, FontPoint)`)**:
   - By Green's theorem over a closed piecewise-cubic/linear boundary $\partial\Omega$:
     $$A = \frac{1}{2}\oint_{\partial\Omega} \big(x(t)\,y'(t) - y(t)\,x'(t)\big)\,dt$$
     $$C_x = \frac{1}{6A}\oint_{\partial\Omega} \big(x(t)^2\,y'(t) - x(t)\,y(t)\,x'(t)\big)\,dt, \qquad C_y = \frac{1}{6A}\oint_{\partial\Omega} \big(x(t)\,y(t)\,y'(t) - y(t)^2\,x'(t)\big)\,dt$$
   - Evaluates these line integrals exactly over every linear and cubic segment using 5-point Gauss-Legendre quadrature (exact up to polynomial degree 9).
2. **Layer Counter Balance Report (`compute_layer_counter_balance(&self) -> CounterBalanceReport`)**:
   - Classifies each closed contour as an outer stem (`is_counter = false`) or inner counter (`is_counter = true`, opposite winding sign to the largest-magnitude outer contour), and computes:
     - `net_black_area_upm2` $= \sum_{\text{outer}} |A_i| - \sum_{\text{counter}} |A_j|$
     - `total_counter_area_upm2` $= \sum_{\text{counter}} |A_j|$
     - `black_to_white_ratio` $= \text{net\_black\_area\_upm2} / \max(\text{total\_counter\_area\_upm2}, 10^{-6})$
     - `overall_centroid`: area-weighted visual center of mass $\oplus$ of the net filled glyph shape.

---

### 2.6 Superellipse / Squircle Generator & Live Optical Symmetry Ghost Overlay (`build_superellipse_contour_nodes`, `compute_symmetry_ghost_contours` — `src/core/smart_corners_brush.rs`)

1. **Lamé Superellipse / Squircle Generator (`build_superellipse_contour_nodes(rect: (FontPoint, FontPoint), exponent_n: f64) -> Vec<Node>`)**:
   - Given bounding box $((x_{\min}, y_{\min}), (x_{\max}, y_{\max}))$ with center $(c_x, c_y)$, semi-axes $a = \frac{1}{2}|x_{\max} - x_{\min}|$, $b = \frac{1}{2}|y_{\max} - y_{\min}|$, and Lamé exponent $n = \text{clamp}(\text{exponent\_n}, 2.0, 20.0)$ satisfying:
     $$\left|\frac{x - c_x}{a}\right|^n + \left|\frac{y - c_y}{b}\right|^n = 1$$
   - Generates a closed 12-node contour (4 `NodeType::Smooth` cardinal extrema at $(c_x, y_{\min})$, $(x_{\max}, c_y)$, $(c_x, y_{\max})$, $(x_{\min}, c_y)$, each flanked by 2 orthogonal `NodeType::OffCurve` handles) with handle ratio calibrated so the cubic midpoint at $t = 0.5$ passes through the exact Lamé diagonal point $(c_x \pm a\,2^{-1/n},\; c_y \pm b\,2^{-1/n})$:
     $$\kappa(n) = \text{clamp}\!\left(\frac{2^{-1/n} - 0.125}{0.75},\; 0.55228475,\; 0.95\right)$$
2. **Live Optical Symmetry Ghost Overlay (`SymmetryOverlayMode`, `compute_symmetry_ghost_contours(&self) -> Vec<Vec<Node>>`)**:
   - Configured via `set_symmetry_overlay_mode(mode: SymmetryOverlayMode)`:
     - `SymmetryOverlayMode::Off`: returns an empty vector.
     - `SymmetryOverlayMode::HorizontalAxis`: reflects all contour nodes across the layer bounding-box horizontal midline $y = m_y$, mapping $(x, y) \mapsto (x, 2 m_y - y)$.
     - `SymmetryOverlayMode::VerticalAxis`: reflects all contour nodes across the layer bounding-box vertical midline $x = m_x$, mapping $(x, y) \mapsto (2 m_x - x, y)$.
     - `SymmetryOverlayMode::PointCenter`: applies $180^\circ$ point-inversion through $(m_x, m_y)$, mapping $(x, y) \mapsto (2 m_x - x, 2 m_y - y)$.

---

## 3. Rust Data Structures & Public API Contracts

```rust
/// Non-destructive Smart Corner (`radius_upm > 0.0`) or Ink Trap (`radius_upm < 0.0`) descriptor.
#[derive(Debug, Clone, Copy, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct SmartCornerSpec {
    pub node_id: NodeId,
    pub radius_upm: f64,
}

/// Broad-nib / pressure-sensitive calligraphic brush configuration.
#[derive(Debug, Clone, Copy, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct CalligraphicNibSpec {
    pub min_width_upm: f64,
    pub max_width_upm: f64,
    pub base_angle_deg: f64,
    pub tilt_influence: f64,
}

/// W3C Pointer Events Level 3 stylus sample (`pressure: 0.0..1.0`, `tilt_x`, `tilt_y`).
#[derive(Debug, Clone, Copy, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct CalligraphicSample {
    pub pos: FontPoint,
    pub pressure: f64,
    pub tilt_x: f64,
    pub tilt_y: f64,
}

/// Single normal spine of a Pass-4 Curvature Comb.
#[derive(Debug, Clone, Copy, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct CurvatureCombSpine {
    pub base_point: FontPoint,
    pub tip_point: FontPoint,
    pub signed_curvature: f64,
}

/// Rendering style for the Pass-4 curvature diagnostic overlay.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum CurvatureCombStyle {
    SpinesOnly,
    HeatmapEnvelope,
}

/// Exact cubic inflection point marker where $x'(t)y''(t) - y'(t)x''(t) = 0$ for $t \in (0, 1)$.
#[derive(Debug, Clone, Copy, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct InflectionMarker {
    pub contour_idx: usize,
    pub seg_idx: usize,
    pub t: f64,
    pub point: FontPoint,
}

/// Normalized curvature heatmap sample with intensity $|\kappa(t)| / \kappa_{\max} \in [0, 1]$ and polarity sign.
#[derive(Debug, Clone, Copy, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct CurvatureHeatmapSample {
    pub base_point: FontPoint,
    pub tip_point: FontPoint,
    pub signed_curvature: f64,
    pub normalized_intensity: f64,
    pub polarity_sign: i8,
}

/// Closed curvature heatmap envelope and inflection markers for a contour.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct CurvatureHeatmapEnvelope {
    pub contour_idx: usize,
    pub samples: Vec<CurvatureHeatmapSample>,
    pub closed_polygon: Vec<FontPoint>,
    pub inflections: Vec<InflectionMarker>,
}

/// Classification of an orthogonal stem thickness measurement against reference stems.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum StemGaugeMatch {
    Matched,
    Thinner,
    Thicker,
}

/// Result of casting an inward unit normal ray across a filled stem.
#[derive(Debug, Clone, Copy, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct StemThicknessMeasurement {
    pub p_entry: FontPoint,
    pub p_exit: FontPoint,
    pub thickness_upm: f64,
    pub nearest_reference_upm: Option<f64>,
    pub gauge_match: StemGaugeMatch,
}

/// Green's theorem area and centroid entry for a single closed contour.
#[derive(Debug, Clone, Copy, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct ContourAreaCentroid {
    pub contour_id: ContourId,
    pub signed_area_upm2: f64,
    pub centroid: FontPoint,
    pub is_counter: bool,
}

/// Layer-wide Green's theorem stem/counter area balance and visual centroid report.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct CounterBalanceReport {
    pub contours: Vec<ContourAreaCentroid>,
    pub net_black_area_upm2: f64,
    pub total_counter_area_upm2: f64,
    pub black_to_white_ratio: f64,
    pub overall_centroid: FontPoint,
}

/// Live optical symmetry ghost overlay mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum SymmetryOverlayMode {
    Off,
    HorizontalAxis,
    VerticalAxis,
    PointCenter,
}

/// Converts a stream of W3C stylus samples into a closed cubic Bézier calligraphic contour.
pub fn fit_calligraphic_brush_stroke(
    nib: CalligraphicNibSpec,
    samples: &[CalligraphicSample],
) -> Option<GlyphContour>;

/// Solves $x'(t)y''(t) - y'(t)x''(t) = 0$ for $t \in (0, 1)$ on a cubic Bézier segment.
pub fn find_cubic_inflection_points(
    contour_idx: usize,
    seg_idx: usize,
    p0: FontPoint,
    p1: FontPoint,
    p2: FontPoint,
    p3: FontPoint,
) -> Vec<InflectionMarker>;

/// Evaluates exact Green's theorem signed area $A$ and visual centroid $(C_x, C_y)$ of a closed contour.
pub fn compute_contour_area_and_centroid(nodes: &[Node]) -> (f64, FontPoint);

/// Generates a closed 12-node superellipse / squircle contour for Lamé exponent $n \in [2.0, 20.0]$.
pub fn build_superellipse_contour_nodes(
    rect: (FontPoint, FontPoint),
    exponent_n: f64,
) -> Vec<Node>;

impl GrafiaCanvasState {
    // 2.1 Non-Destructive Smart Corners & Ink Traps
    pub fn set_smart_corner(&mut self, spec: SmartCornerSpec);
    pub fn remove_smart_corner(&mut self, node_id: NodeId);
    pub fn compute_smart_corner_preview_contours(&self) -> Vec<GlyphContour>;
    pub fn bake_smart_corners(&mut self);

    // 2.2 Pressure & Tilt Calligraphic Brush
    pub fn apply_calligraphic_brush_stroke(
        &mut self,
        nib: CalligraphicNibSpec,
        samples: &[CalligraphicSample],
    ) -> Option<ContourId>;

    // 2.3 Live Curvature Comb, Heatmap Envelope & Inflection Markers
    pub fn toggle_show_curvature_comb(&mut self);
    pub fn set_curvature_comb_style(&mut self, style: CurvatureCombStyle);
    pub fn compute_curvature_comb_spines(
        &self,
        samples_per_segment: usize,
        scale_upm2: f64,
    ) -> Vec<Vec<CurvatureCombSpine>>;
    pub fn compute_layer_inflection_markers(&self) -> Vec<InflectionMarker>;
    pub fn compute_curvature_heatmap_envelopes(
        &self,
        samples_per_segment: usize,
        scale_upm2: f64,
    ) -> Vec<CurvatureHeatmapEnvelope>;

    // 2.4 Live Orthogonal Stem Thickness Band & Reference Stem Gauge
    pub fn set_reference_stems(&mut self, stems_upm: Vec<f64>, tolerance_upm: f64);
    pub fn measure_orthogonal_stem_thickness(
        &self,
        contour_idx: usize,
        seg_idx: usize,
        t: f64,
    ) -> Option<StemThicknessMeasurement>;

    // 2.5 Closed-Form Green's Theorem Counter Area & Visual Centroid
    pub fn compute_layer_counter_balance(&self) -> CounterBalanceReport;

    // 2.6 Optical Symmetry Ghost Overlay
    pub fn set_symmetry_overlay_mode(&mut self, mode: SymmetryOverlayMode);
    pub fn compute_symmetry_ghost_contours(&self) -> Vec<Vec<Node>>;
}
```

---

## 4. Complete Normative Test Matrix (`TC-V090-001` .. `TC-V090-016`)

All 16 test cases are implemented under `#![cfg(feature = "v090_red")]`:

| Test Case ID | Requirement / Mathematical Contract | Input / Action | Expected Normative Outcome |
| :--- | :--- | :--- | :--- |
| **`TC-V090-001`** | Non-Destructive Smart Corner Fillet (`radius_upm > 0.0` — `compute_smart_corner_preview_contours`) | Attach `SmartCornerSpec { node_id: NodeId(10), radius_upm: 30.0 }` to the $(0, 0)$ corner of a $200 \times 200$ square; call `compute_smart_corner_preview_contours()`. | Underlying `state.glyph.contours` stays untouched (4 nodes); preview contour replaces $(0, 0)$ with a tangent cubic arc starting at $(30, 0)$ and $(0, 30)$ with $\kappa_{\text{arc}} \approx 0.55228$. |
| **`TC-V090-002`** | Smart Corner Max-Radius Clamp on Short Edges (`0.48 * min(L_in, L_out)`) | Parameterized test with `radius_upm = 20.0`, `50.0`, and `500.0` (exceeding edge length `200.0`). | Trim distance clamps at $0.48 \times 200 = 96.0\;\text{UPM}$ so adjacent fillets never self-intersect or invert edge direction. |
| **`TC-V090-003`** | Non-Destructive Ink Trap Notch (`radius_upm < 0.0`) | Attach `SmartCornerSpec { node_id: NodeId(10), radius_upm: -25.0 }` to a corner vertex; call `compute_smart_corner_preview_contours()`. | Preview contour cuts an inward 4-vertex notch along the interior angle bisector of depth $25.0\;\text{UPM}$, decreasing enclosed contour area. |
| **`TC-V090-004`** | Destructive `bake_smart_corners()` with Atomic Undo / Redo | Attach both a positive fillet (`+30.0`) and a negative ink trap (`-20.0`), invoke `bake_smart_corners()`, then `Undo` and `Redo`. | `bake_smart_corners()` commits the geometry into `state.glyph.contours` and clears `smart_corners`; `Undo` restores the 4-node sharp contour and `smart_corners` list bit-exactly. |
| **`TC-V090-005`** | Calligraphic Brush Variable Pressure Width (`fit_calligraphic_brush_stroke`) | Feed a horizontal sample stream from $x=0$ (`pressure = 0.0`) to $x=300$ (`pressure = 1.0`) with `min_width_upm = 20.0`, `max_width_upm = 80.0`, `base_angle_deg = 90.0`. | Produces a closed cubic contour whose vertical thickness widens monotonically from $20.0\;\text{UPM}$ at $x=0$ to $80.0\;\text{UPM}$ at $x=300$. |
| **`TC-V090-006`** | Calligraphic Brush Stylus Tilt Modulation (`tilt_x`, `tilt_y`) & Out-of-Bounds Pressure Clamp | Compare strokes with `(tilt_x: 30, tilt_y: 0)` vs. `(tilt_x: 0, tilt_y: 60)` under `tilt_influence = 0.5`, and test `pressure = -0.5` / `1.8`. | Stylus tilt rotates the nib cross-section envelope; out-of-range pressures clamp safely to $[0.0, 1.0]$ without NaN or self-intersection. |
| **`TC-V090-007`** | `apply_calligraphic_brush_stroke` Canvas Integration & Atomic Undo | Call `apply_calligraphic_brush_stroke` on `GrafiaCanvasState` and verify `Undo` / `Redo` and degenerate (< 2 sample) rejection. | Returns `Some(contour_id)` for $\ge 2$ samples, appends closed contour, round-trips `Undo`/`Redo`, and returns `None` for $< 2$ samples. |
| **`TC-V090-008`** | Curvature Comb Analytical Spines (`compute_curvature_comb_spines`) | Compute 16-sample curvature comb spines (`scale_upm2 = 5000.0`) on a radius-$100$ quarter-circle cubic segment. | Returns 17 spines whose `base_point` lies on the arc and whose spine length $\| \text{tip} - \text{base} \|_2 = 5000 \times 0.01 = 50.0\;\text{UPM}$ along the radial normal. |
| **`TC-V090-009`** | Curvature Comb Inflection Sign Flip on S-Curve Cubic | Evaluate `compute_curvature_comb_spines` on a symmetric S-curve cubic with an inflection point at $t = 0.5$. | `signed_curvature` has opposite signs at $t = 0.0$ and $t = 1.0$, and vanishes ($\approx 0.0$) at the inflection midpoint $t = 0.5$. |
| **`TC-V090-010`** | Pass-2 & Pass-4 Headless Light/Dark Rendering with Smart Corners & Curvature Comb | Render canvas with active Smart Corners, Ink Traps, Calligraphic Brush contour, and `show_curvature_comb = true` in Light and Dark modes. | Produces deterministic, distinct pixel buffers in Light vs. Dark modes with zero panics. |
| **`TC-V090-011`** | Property-Based Invariant Verification (`proptest`) Across Smart Corner Radii & Brush Pressures | Run `proptest` over arbitrary corner radii $R \in [-80.0, 120.0]$ and stylus pressures $p \in [0.0, 1.0]$. | All preview/baked contours and fitted brush strokes remain closed, finite, and free of NaN/Inf coordinates. |
| **`TC-V090-012`** | Stateful Pull-Based `arbtest` Multi-Tool Fuzzing (`SmartCorners + InkTraps + Bake + CalligraphicBrush + CurvatureComb + Undo/Redo`) | Run 250 ms pull-based `arbtest` action sequences mixing Smart Corner edits, Ink Traps, `bake_smart_corners`, brush strokes, Curvature Comb toggles, and `Undo`/`Redo`. | `state.validate_invariants()` holds after every step and `Undo`/`Redo` restores state bit-exactly. |
| **`TC-V090-013`** | Curvature Heatmap Envelope & Exact Inflection Point Markers (`CurvatureCombStyle::HeatmapEnvelope`, `find_cubic_inflection_points`) | Solve inflection points on a symmetric S-curve cubic ($t = 0.5$) and compute `compute_curvature_heatmap_envelopes(12, 3000.0)`. | `find_cubic_inflection_points` returns `t ≈ 0.50` at $(50, 0)$; heatmap envelope builds a closed polygon with `normalized_intensity` peaking at $1.0$ and opposite `polarity_sign` across $t = 0.5$. |
| **`TC-V090-014`** | Live Orthogonal Stem Thickness Band & Reference Stem Gauge (`measure_orthogonal_stem_thickness`, `StemGaugeMatch`) | Set `reference_stems_upm = vec![80.0]` (`stem_tolerance_upm = 2.0`) and measure stems of width `81.0` (`Matched`), `68.0` (`Thinner`), and `94.0` (`Thicker`). | Returns exact `[p_entry, p_exit]` chord, perpendicular thickness $d_\perp$, and classifies `Matched`, `Thinner`, and `Thicker` accurately. |
| **`TC-V090-015`** | Closed-Form Green's Theorem Counter Area & Visual Centroid $\oplus$ (`compute_contour_area_and_centroid`, `compute_layer_counter_balance`) | Evaluate a $200 \times 300$ outer stem ring ($60{,}000\;\text{UPM}^2$) enclosing a $100 \times 150$ counter ($15{,}000\;\text{UPM}^2$) centered at $(100, 150)$. | Reports `net_black_area_upm2 = 45_000.0`, `total_counter_area_upm2 = 15_000.0`, `black_to_white_ratio = 3.0`, and `overall_centroid = (100.0, 150.0)`. |
| **`TC-V090-016`** | Superellipse / Squircle Generator & Live Optical Symmetry Ghost Overlay (`build_superellipse_contour_nodes`, `SymmetryOverlayMode`, `compute_symmetry_ghost_contours`) | Generate squircles with $n = 2.0$ and $n = 4.0$ on $[0, 200]^2$, and evaluate `compute_symmetry_ghost_contours()` under `HorizontalAxis`, `VerticalAxis`, and `PointCenter`. | Squircle $n = 4.0$ has 12 nodes and strictly larger enclosed area than ellipse $n = 2.0$; symmetry overlay modes reflect and point-invert coordinates around the bounding-box center $(m_x, m_y)$. |
