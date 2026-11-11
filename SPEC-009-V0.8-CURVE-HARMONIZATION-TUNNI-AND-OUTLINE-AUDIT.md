---
type: specification
id: SPEC-009
version: 0.8.0
status: normative
parent: SPEC-001
---

# SPEC-009: `grafia v0.8.0` — Precision Curve Harmonization ($G^2$ Smoothness Continuity, Tangent Balance Lines, Proportional Handle Servo, Interpolated Neighbour Nudge, Master Trajectory Kinks & Proactive Outline Audit)

> **Normative References**:
> - `SPEC-001`: `grafia v0.1.0` — Architecture, Coordinate Pipeline & Token System
> - `SPEC-002`: `grafia v0.2.0` — Bézier Topology, Selection & Transform Box
> - `SPEC-004`: `grafia v0.3.0` — Guides, Snap Engine & Alignment Zones
> - `SPEC-008`: `grafia v0.7.0` — Background Layer, 2D Anchors, Pre-Resolved Components, Fit Curve, Live Contour Stroke & Master Compatibility
> - Standards: `ISO/IEC/IEEE 42010:2011` (Architecture Description), `ISO/IEC/IEEE 29119-3:2021` (Test Documentation), `IEEE 1012-2016` (System & Software Verification and Validation), `ISO/IEC 14496-22:2019` (`OpenType 1.9` Variation Interpolation), `SemVer 2.0.0`
> - Mathematical Literature: Linus Romer (*Harmonized Cubic Bézier Curves*, 2015), Cristóbal Henestrosa & Eduardo Tunni (*Tangent Balance Lines*, 2012), Gerald Farin (*Curves and Surfaces for CAGD*)

---

## 1. Normative Mathematical & CAGD Foundations

All curvature equations, tangent-balance handle ratios, proportional handle servo rules, cross-master kink metrics, and vector outline audit predicates in `SPEC-009` are grounded in Computer-Aided Geometric Design (CAGD) and `ISO/IEC 14496-22:2019` (`OpenType 1.9`):

| Subsystem | Domain Standard / Mathematical Model | Core Invariant / Equation | Verified Canvas Semantics |
| :--- | :--- | :--- | :--- |
| **1. $G^2$ Smoothness Continuity & Harmonize (`Ctrl+Shift+H`)** | Romer / Farin Parametric Curvature Continuity | $\kappa(t) = \frac{x'y'' - y'x''}{(x'^2 + y'^2)^{3/2}}$, $\;p_0 = \frac{a}{b} = \sqrt{\frac{d_{\text{in}}}{d_{\text{out}}}}$ | Equalizes signed curvature $\kappa_{\text{in}}(1) = \kappa_{\text{out}}(0)$ across collinear smooth joins (`Ctrl+Shift+H`), supports fixed-anchor handle balancing (`harmonize_handles_keep_node`), and enforces live $G^2$ continuity on registered `genius_nodes`. |
| **2. Interactive Tangent Balance (Tunni) Lines** | Henestrosa & Tunni Tangent-Triangle Balance Line | $M_{\text{tunni}} = \frac{1}{2}(P_1 + P_2)$, $\;t_i \in [0.20, 0.95]$ | Renders and hit-tests the chord $[P_1, P_2]$ between cubic off-curve handles (`6.0 dp` tolerance), scales both handles along their tangent rays toward apex $T$ within $[0.20, 0.95]$, and equalizes ratios to parallelize $[P_1, P_2] \parallel [P_0, P_3]$. |
| **3. Proportional Handle Servo & Interpolated Neighbour Nudge (`C`)** | Per-Axis Affine Handle Servo & Bounded Linear Weighting | $x_i' = P_{3,x} + (x_i - P_{3,x})\frac{P_{0,x}' - P_{3,x}}{P_{0,x} - P_{3,x}}$ | Preserves dimensionless off-curve handle-to-chord ratios when an extremum moves (`servo_translate_on_curve_node`) and linearly interpolates intermediate unselected contour nodes between translated stems and stationary contour extrema (`power_nudge_translate_selected`, shortcut `C`). |
| **4. Master Trajectory Crossings, Smooth-Node Kink Detection & Master Auto-Matching** | `OpenType 1.9` `gvar` Linear Point Trajectory, Winding Parity & Cyclic Least-Squares Registration | $\vec{T}_k(s) = (1-s)P_k^{(A)} + sP_k^{(B)}$, $\;k^* = \arg\min_k \sum_{i=0}^{N-1} \|P_{(i+k)\bmod N} - Q_i\|^2$ | Supports `MasterCompatibilityMode { OriginAndAnchorRays, FullNodeTrajectories }`, detects intersecting node trajectories $\vec{T}_i \cap \vec{T}_j \neq \emptyset$, flags smooth nodes whose cross-master tangent angle and handle-length ratio simultaneously diverge, and automatically repairs contour order, winding direction, and cyclic `Node 0` start-node alignment via `auto_match_master_compatibility`. |
| **5. Proactive Vector Outline Audit** | Parametric Extrema, Slant Frame & Chord Collinearity Analysis | $x'_{\text{slant}}(t)=0,\; y'(t)=0$, $\;d_\perp(P_{1,2}, [P_0,P_3]) \le 0.05$ | Audits contours for `MissingExtremum` (including italic-slanted extrema), `QuasiOrthogonalStem` ($0.1^\circ \le \Delta\theta \le 2.0^\circ$), `CrossedHandles` ($[P_0,P_1] \cap [P_2,P_3] \neq \emptyset$), and `CollinearCubic` ($\le 0.05\;\text{UPM}$). |
| **6. Off-Axis Handle Warning & Handle-Ratio HUD** | Extremum Tangent Orthogonality & Apex Percentage Telemetry | $\Delta\theta \in [0.05^\circ, 15.0^\circ]$, $\;\text{ratio} = 100 \cdot \frac{\|H - P\|}{\|T - P\|}$ | Flags `OffCurve` handles on `Smooth` extrema that deviate slightly from $0^\circ$, $90^\circ$, or `italic_angle_deg`, and reports `(ratio_in, ratio_out)` handle length percentages relative to tangent intersection apex $T$. |
| **7. Diagonal Leg / Tangent Extension & Virtual Apex Snapping** | Infinite Ray Projection & Pairwise Line-Line Apex Intersection | $P_{\text{proj}} = A + s(B-A)$, $\;V_{m,n} = L_m \cap L_n$ | Extends non-orthogonal `Corner` line legs as infinite rays so truncated diagonal stems (`/A`, `/V`, `/M`, `/K`) snap directly to ray extensions or virtual ray-ray apex intersections within `snap_radius_upm`. |

---

## 2. Normative Mathematical & Functional Requirements

### 2.1 $G^2$ Smoothness Continuity & `harmonize_selected_nodes()` (`Ctrl+Shift+H` — `src/core/curve_harmony.rs`)

1. **Signed Parametric Curvature of a Cubic Bézier Segment**:
   - Given a cubic Bézier curve $B(t) = (1-t)^3 P_0 + 3(1-t)^2 t P_1 + 3(1-t) t^2 P_2 + t^3 P_3$ for $t \in [0, 1]$:
     $$B'(t) = 3(1-t)^2 (P_1 - P_0) + 6(1-t)t (P_2 - P_1) + 3t^2 (P_3 - P_2)$$
     $$B''(t) = 6(1-t)(P_2 - 2P_1 + P_0) + 6t(P_3 - 2P_2 + P_1)$$
   - The signed 2D curvature at parameter $t \in [0, 1]$ (`cubic_signed_curvature(p0, p1, p2, p3, t) -> f64`) is:
     $$\kappa(t) = \frac{x'(t)\, y''(t) - y'(t)\, x''(t)}{\left(x'(t)^2 + y'(t)^2\right)^{3/2}}$$
   - At the endpoints $t = 1$ of an incoming cubic $(Q_0, Q_1, Q_2, P)$ and $t = 0$ of an outgoing cubic $(P, R_1, R_2, R_3)$ sharing a $G^1$-smooth (collinear tangent) on-curve node $P$:
     $$|\kappa_{\text{in}}(1)| = \frac{2 \, d_{\text{in}}}{3 \, a^2}, \qquad |\kappa_{\text{out}}(0)| = \frac{2 \, d_{\text{out}}}{3 \, b^2}$$
     where $a = \|P - Q_2\|_2$ is the incoming handle length, $b = \|R_1 - P\|_2$ is the outgoing handle length, and $d_{\text{in}}, d_{\text{out}}$ are the perpendicular distances from $Q_1$ and $R_2$ to the common tangent line passing through $Q_2, P, R_1$.

2. **Romer / Henestrosa $G^2$ Harmonization (`harmonize_selected_nodes()` — `Ctrl+Shift+H`)**:
   - Equating $\kappa_{\text{in}}(1) = \kappa_{\text{out}}(0)$ yields the fundamental $G^2$ handle-ratio law:
     $$p_0 = \frac{a}{b} = \sqrt{\frac{d_{\text{in}}}{d_{\text{out}}}}$$
   - When `harmonize_selected_nodes()` is invoked (or dispatched via `Ctrl+Shift+H`):
     - For every selected smooth on-curve node $P$ joining two cubic segments $(Q_0, Q_1, Q_2, P)$ and $(P, R_1, R_2, R_3)$ with non-degenerate $d_{\text{in}} > 10^{-6}$ and $d_{\text{out}} > 10^{-6}$:
       1. Projects $Q_2$ and $R_1$ onto the unit tangent direction $\hat{\tau}$ so $Q_2, P, R_1$ are strictly collinear ($G^1$).
       2. Computes $p_0 = \sqrt{d_{\text{in}} / d_{\text{out}}}$.
       3. Relocates the on-curve node $P$ along the handle chord $[Q_2, R_1]$ to the equilibrium point:
          $$P' = \frac{Q_2 + p_0 \, R_1}{1 + p_0}$$
          so that $\|P' - Q_2\|_2 / \|R_1 - P'\|_2 = p_0 = \sqrt{d_{\text{in}} / d_{\text{out}}}$, guaranteeing $|\kappa_{\text{in}}(1) - \kappa_{\text{out}}(0)| < 10^{-5}$.
     - `harmonize_handles_keep_node(node_id: NodeId)` keeps the on-curve anchor $P$ fixed while scaling $(a, b)$ with constant product $a \cdot b = a_{\text{old}} \cdot b_{\text{old}}$ so $a' = \sqrt{a_{\text{old}} b_{\text{old}} p_0}$ and $b' = \sqrt{a_{\text{old}} b_{\text{old}} / p_0}$.
     - Records an atomic Undo snapshot before mutating node coordinates.

3. **Live $G^2$ Smoothness Node Constraint (`genius_nodes: BTreeSet<NodeId>`)**:
   - Any smooth on-curve node can be registered in `state.genius_nodes` via `set_node_genius(node_id: NodeId, enabled: bool)`.
   - When an off-curve handle ($Q_2$ or $R_1$) adjacent to a $G^2$-constrained node $P \in \text{genius\_nodes}$ is translated, `GrafiaCanvasState` automatically enforces collinearity ($G^1$) and updates the opposite handle / join anchor to maintain $G^2$ curvature equality $\kappa_{\text{in}}(1) = \kappa_{\text{out}}(0)$.

---

### 2.2 Interactive Tangent Balance (Tunni) Lines (`show_tunni_lines: bool` — `src/core/curve_harmony.rs`)

1. **Tangent Balance Line & Midpoint Geometry**:
   - For every cubic Bézier segment $(P_0, P_1, P_2, P_3)$ whose tangent rays $\vec{u} = P_1 - P_0$ and $\vec{v} = P_2 - P_3$ intersect forward at apex $T$:
     - The **Tangent Balance (Tunni) Line** is the line segment $[P_1, P_2]$ connecting the two off-curve control points.
     - The **Balance Midpoint** $M_{\text{tunni}} = \frac{1}{2}(P_1 + P_2)$ is rendered on the balance line when `state.show_tunni_lines == true`.
     - The segment's handle ratios are $t_1 = \|P_1 - P_0\|_2 / \|T - P_0\|_2$ and $t_2 = \|P_2 - P_3\|_2 / \|T - P_3\|_2$, with mean ratio $t_{\text{tunni}} = \frac{1}{2}(t_1 + t_2)$.
2. **Interactive Hit-Testing, Dragging & Clamping (`[0.20, 0.95]`)**:
   - When `state.show_tunni_lines == true`:
     - `hit_test_tunni_line_at_upm(pt: FontPoint) -> Option<(ContourId, usize)>` tests whether `pt` lies within `TUNNI_HIT_RADIUS_DP = 6.0` screen dp (`6.0 / viewport.zoom` UPM) of the segment $[P_1, P_2]$ or midpoint $M_{\text{tunni}}$.
     - Dragging a balance line (`drag_tunni_line_to_upm(contour_id, seg_index, target_upm)` or `scale_segment_tunni_ratio(contour_id, seg_index, ratio)`) scales both $(P_1, P_2)$ along their tangent rays toward/away from $T$, clamping each handle ratio into $[t_{\min}, t_{\max}] = [0.20, 0.95]$ (`TUNNI_MIN_RATIO = 0.20`, `TUNNI_MAX_RATIO = 0.95`):
       $$P_1' = P_0 + \text{clamp}(t_1', 0.20, 0.95) \cdot (T - P_0), \qquad P_2' = P_3 + \text{clamp}(t_2', 0.20, 0.95) \cdot (T - P_3)$$
     - `balance_segment_tunni(contour_id, seg_index)` sets both handles to their arithmetic mean ratio $\bar{t} = \text{clamp}(\frac{1}{2}(t_1 + t_2), 0.20, 0.95)$, making the balance line $[P_1', P_2']$ strictly parallel to the segment chord $[P_0, P_3]$.

---

### 2.3 Proportional Handle Servo & Interpolated Neighbour Nudge (`power_nudge_enabled: bool`, Shortcut `C` — `src/core/curve_harmony.rs`)

1. **Proportional Handle Servo (`servo_translate_on_curve_node`)**:
   - When `state.servo_scaling_enabled == true` (or `state.power_nudge_enabled == true`), translating an on-curve endpoint $P_0 \to P_0'$ of an unselected-opposite-endpoint cubic segment $(P_0, P_1, P_2, P_3)$ scales the intermediate off-curve handles $P_1, P_2$ proportionally along each axis with non-degenerate span ($|\Delta X_0| > 10^{-3}$, $|\Delta Y_0| > 10^{-3}$):
     $$x_i' = P_{3,x} + (x_i - P_{3,x}) \cdot \frac{P_{0,x}' - P_{3,x}}{P_{0,x} - P_{3,x}}, \qquad y_i' = P_{3,y} + (y_i - P_{3,y}) \cdot \frac{P_{0,y}' - P_{3,y}}{P_{0,y} - P_{3,y}} \quad (i \in \{1, 2\})$$
   - This preserves the dimensionless handle-to-chord ratios $(r_{1,x}, r_{1,y})$ and $(r_{2,x}, r_{2,y})$ across any extremum drag or nudge.
2. **Interpolated Neighbour Nudge (`power_nudge_enabled: bool`, Shortcut `C`)**:
   - Pressing `C` (without modifiers) toggles `state.power_nudge_enabled` (`toggle_power_nudge()`).
   - When `state.power_nudge_enabled == true` and a subset of contour on-curve nodes is translated by $\Delta = (\Delta x, \Delta y)$ via `power_nudge_translate_selected(delta: FontPoint)`:
     - Selected on-curve nodes translate by full $\Delta$ (and their directly attached off-curve handles follow via Proportional Handle Servo).
     - Unselected nodes lying strictly between the selection's pre-move bounding interval and the stationary contour extrema $[X_{\min}, X_{\max}] \times [Y_{\min}, Y_{\max}]$ receive a linearly interpolated displacement $(w_x \Delta x, w_y \Delta y)$ where $w_x, w_y \in [0.0, 1.0]$ equals their normalized distance from the stationary extremum anchor to the moved boundary.
     - Stationary outer extremum nodes at the opposite boundary of the contour have weight $w = 0.0$ and remain fixed.
     - Records a single atomic Undo snapshot.

---

### 2.4 `MasterCompatibilityMode`, Trajectory Crossing Detection & Smooth-Node Interpolation Kink Detector (`src/core/curve_harmony.rs`)

1. **Master Compatibility Overlay Modes (`MasterCompatibilityMode`)**:
   - `state.master_compatibility_mode: MasterCompatibilityMode` (configured via `set_master_compatibility_mode(mode: MasterCompatibilityMode)`):
     - `MasterCompatibilityMode::OriginAndAnchorRays` (default): renders start-node (`index 0`) and matching anchor connecting vectors.
     - `MasterCompatibilityMode::FullNodeTrajectories`: evaluates and renders node-by-node linear interpolation trajectories $\vec{T}_{i,k}(s) = (1 - s) P_{i,k}^{(A)} + s P_{i,k}^{(B)}$ ($s \in [0, 1]$) for every compatible node index $k$ across contour $i$ of the active layer $A$ and each `CompanionMasterLayer` $B$.
2. **Trajectory Crossing Detection (`TrajectoryCrossing`)**:
   - Within each compatible contour $i$ and companion master $B$, `compute_master_kinks_and_crossings(&self) -> MasterKinkAndCrossingReport` tests every pair of distinct node trajectories $\vec{T}_{i,j} = [P_{i,j}^{(A)}, P_{i,j}^{(B)}]$ and $\vec{T}_{i,k} = [P_{i,k}^{(A)}, P_{i,k}^{(B)}]$ ($0 \le j < k < N_i$).
   - If the open segments $\vec{T}_{i,j}$ and $\vec{T}_{i,k}$ intersect at interior parameters $(s^*, u^*) \in (10^{-5}, 1 - 10^{-5})^2$ (indicating a shifted start node or reversed contour winding between masters), emits `TrajectoryCrossing { contour_idx: i, companion_layer_id: B.id, node_index_a: j, node_index_b: k, intersection }`.
3. **Smooth-Node Interpolation Kink Detector (`KinkWarning`)**:
   - At every `NodeType::Smooth` vertex $P_k$ flanked by off-curve handles $H_{\text{in}} = P_{k-1}$ and $H_{\text{out}} = P_{k+1}$ in both the active layer $A$ and companion master $B$:
     - Let $\hat{\tau}^{(A)}$ and $\hat{\tau}^{(B)}$ be the unit tangent vectors at $P_k$, with angular divergence:
       $$\Delta\theta = \arccos\!\Big(\text{clamp}\big(\hat{\tau}^{(A)} \cdot \hat{\tau}^{(B)}, -1.0, 1.0\big)\Big) \cdot \frac{180^\circ}{\pi}$$
     - Let the normalized handle-length ratio in master $M \in \{A, B\}$ be:
       $$r_M = \frac{\|H_{\text{in}}^{(M)} - P_k^{(M)}\|_2}{\|H_{\text{in}}^{(M)} - P_k^{(M)}\|_2 + \|H_{\text{out}}^{(M)} - P_k^{(M)}\|_2}$$
     - When $\Delta\theta > 1.0^\circ$ **and** $|r_A - r_B| > 0.05$, linear interpolation produces an interior tangent cusp (kink) at intermediate designspace instances; `compute_master_kinks_and_crossings` emits `KinkWarning { contour_idx, node_id: P_k.id, companion_layer_id: B.id, severity: delta_theta_deg * (r_a - r_b).abs() }`.

---

4. **Automatic Cross-Master Contour Order, Winding & Start-Node Matching (`auto_match_master_compatibility(companion_layer_id: u32) -> bool`)**:
   - When the active foreground layer $A$ and a target `CompanionMasterLayer` $B$ (`companion_layer_id`) contain compatible contour sets (matching contour count and a bijection of `(node_count, closed)` signatures), `auto_match_master_compatibility` deterministically aligns layer $A$'s contours to reference master $B$ in three sequential steps:
     1. **Contour Pairing (Order Matching)**:
        - Pairs each reference contour $C_j^{(B)}$ ($j \in \{0, \dots, M-1\}$) with an unassigned foreground contour $C_{\pi(j)}^{(A)}$ having identical `(nodes.len(), closed)` signature that minimizes the sum of squared Euclidean distances between contour bounding-box centers (centroids) and bounding-box dimensions (width, height), disambiguating concentric contours that share an identical center:
          $$\pi(j) = \arg\min_{m \in \text{unassigned}} \left( \left\| \text{center}\!\left(C_m^{(A)}\right) - \text{center}\!\left(C_j^{(B)}\right) \right\|_2^2 + \left\| \text{size}\!\left(C_m^{(A)}\right) - \text{size}\!\left(C_j^{(B)}\right) \right\|_2^2 \right)$$
        - Reorders `state.contours` and `state.glyph.contours` into the matched reference sequence $(C_{\pi(0)}^{(A)}, \dots, C_{\pi(M-1)}^{(A)})$.
     2. **Winding & Open-Path Direction Alignment (`reverse_contour_nodes`)**:
        - For each paired closed contour $(C^{(A)}, C^{(B)})$ with non-zero signed shoelace area, compares $\operatorname{sgn}\!\big(\text{Area}(C^{(A)})\big)$ against $\operatorname{sgn}\!\big(\text{Area}(C^{(B)})\big)$.
        - If $\operatorname{sgn}\!\big(\text{Area}(C^{(A)})\big) \neq \operatorname{sgn}\!\big(\text{Area}(C^{(B)})\big)$, reverses $C^{(A)}$ via `reverse_contour_nodes(&mut C_a, true)` so both masters share identical outer/counter winding orientation.
        - For each paired open contour (`closed == false`), compares the forward node sequence against the reversed node sequence (`reverse_contour_nodes(&mut C_a, false)`): if the reversed sequence preserves the `OnCurve` / `OffCurve` pattern of $C^{(B)}$ and yields a strictly smaller $\sum_{i=0}^{N-1} \|P_i^{(A)} - Q_i^{(B)}\|_2^2$, reverses $C^{(A)}$.
     3. **Optimal Cyclic Start-Node Shift (`Node 0` Least-Squares Registration)**:
        - For each paired closed contour of $N$ nodes after winding alignment, evaluates all cyclic node shifts $k \in \{0, \dots, N-1\}$ that preserve the topological `OnCurve` / `OffCurve` sequence (`(P_{(i+k) \bmod N}.node_type == NodeType::OffCurve) == (Q_i.node_type == NodeType::OffCurve)` for all $i \in \{0, \dots, N-1\}$) and where `P_k` is an on-curve node (`P_k.node_type != NodeType::OffCurve`).
        - Selects the shift $k^*$ that minimizes total squared Euclidean trajectory length across all $N$ nodes:
          $$k^* = \arg\min_{k} \sum_{i=0}^{N-1} \left\| P_{(i+k)\bmod N}^{(A)} - Q_i^{(B)} \right\|_2^2$$
        - Cyclically rotates the foreground contour by $k^*$ so `Node 0` aligns with $Q_0^{(B)}$.
   - Records an atomic Undo snapshot before mutating `state.contours` / `state.glyph.contours` (only when at least one contour order, winding direction, or start-node index changes) and increments `content_generation`. Returns `true` if the layer was matched (or already aligned) against a topologically compatible companion master, and `false` if `companion_layer_id` does not exist or has incompatible contour counts/node signatures.

---

### 2.5 Proactive Vector Outline Audit (`audit_layer_outlines` — `src/core/curve_harmony.rs`)

1. **Four-Predicate Contour Defect Audit (`audit_layer_outlines(&self) -> Vec<OutlineAuditFinding>`)**:
   - Inspects every contour and segment in `state.glyph.contours` and reports:
     1. **`OutlineAuditKind::MissingExtremum { contour_idx, seg_idx, t, point }`**:
        - Solves $x'_{\text{slant}}(t) = 0$ and $y'(t) = 0$ for $t \in (10^{-3}, 1 - 10^{-3})$ on each cubic Bézier segment $B(t)$, where $x_{\text{slant}}(t) = x(t) - y(t) \tan(\text{italic\_angle\_deg} \cdot \pi / 180)$ accounts for slanted extrema when `state.italic_angle_deg != 0.0`.
        - Emits a finding for each interior extremum whose coordinate excursion from both segment endpoints exceeds $0.5\;\text{UPM}$.
     2. **`OutlineAuditKind::QuasiOrthogonalStem { contour_idx, seg_idx, angle_deviation_deg }`**:
        - For each line segment $[P_0, P_3]$ of length $\ge 5.0\;\text{UPM}$, measures the smallest angular deviation $\Delta\theta$ from $0^\circ$ (horizontal), $90^\circ$ (vertical), or $90^\circ - \text{italic\_angle\_deg}$ (slanted vertical).
        - Flags segments with $0.1^\circ \le \Delta\theta \le 2.0^\circ$.
     3. **`OutlineAuditKind::CrossedHandles { contour_idx, seg_idx }`**:
        - Flags any cubic segment $(P_0, P_1, P_2, P_3)$ whose off-curve handle chords $[P_0, P_1]$ and $[P_2, P_3]$ intersect at parameters $(u, v) \in (0, 1)^2$.
     4. **`OutlineAuditKind::CollinearCubic { contour_idx, seg_idx }`**:
        - Flags any cubic segment $(P_0, P_1, P_2, P_3)$ with $\|P_3 - P_0\|_2 > 10^{-3}$ where both off-curve handles $P_1, P_2$ lie within perpendicular distance $d_\perp \le 0.05\;\text{UPM}$ of the line through $[P_0, P_3]$.

---

### 2.6 Off-Axis Handle Warning & Handle-Ratio HUD (`compute_angled_handle_warnings`, `compute_segment_handle_ratios` — `src/core/curve_harmony.rs`)

1. **Off-Axis Handle Warning (`compute_angled_handle_warnings(&self) -> Vec<AngledHandleWarning>`)**:
   - Inspects every `NodeType::OffCurve` handle attached to a `NodeType::Smooth` extremum node.
   - If the handle direction vector $\vec{h} = P_{\text{handle}} - P_{\text{smooth}}$ deviates by $0.05^\circ \le \Delta\theta \le 15.0^\circ$ from the nearest reference axis ($0^\circ$, $90^\circ$, or $90^\circ - \text{italic\_angle\_deg}$), emits `AngledHandleWarning { contour_idx, smooth_node_id, handle_node_id, deviation_deg }`.
2. **Handle-Ratio Percentage HUD (`compute_segment_handle_ratios(&self) -> Vec<SegmentHandleRatioHud>`)**:
   - For each cubic segment $(P_0, P_1, P_2, P_3)$ whose tangent rays meet forward at apex $T$, computes the handle length percentages relative to $T$:
     $$\text{ratio\_out} = 100.0 \cdot \frac{\|P_1 - P_0\|_2}{\|T - P_0\|_2}, \qquad \text{ratio\_in} = 100.0 \cdot \frac{\|P_2 - P_3\|_2}{\|T - P_3\|_2}$$
   - Returns `SegmentHandleRatioHud { contour_idx, seg_idx, apex: T, ratio_in, ratio_out }`.

---

### 2.7 Diagonal Leg / Tangent Extension & Virtual Apex Snapping (`compute_tangent_extension_snaps` — `src/core/curve_harmony.rs`)

1. **Infinite Diagonal Ray Extension & Virtual Apex Intersection (`compute_tangent_extension_snaps(&self, query_pt: FontPoint, snap_radius_upm: f64) -> Option<TangentExtensionSnap>`)**:
   - Collects all non-orthogonal line segments $[A_m, B_m]$ (`Corner`-to-`Corner` line segments deviating by $> 2.0^\circ$ from $0^\circ$ and $90^\circ$, such as the diagonal legs of `/A`, `/V`, `/M`, `/K`).
   - **Virtual Apex Intersection (`TangentExtensionSnapKind::VirtualApex`)**:
     - For every pair of non-parallel diagonal line segments $m < n$, computes the unbounded ray-ray intersection $V_{m,n}$. If $\|V_{m,n} - \text{query\_pt}\|_2 \le \text{snap\_radius\_upm}$, records $V_{m,n}$ as a virtual apex snap candidate.
   - **Ray Extension Projection (`TangentExtensionSnapKind::RayExtension`)**:
     - Projects `query_pt` orthogonally onto the infinite line extending each diagonal segment $[A_m, B_m]$. If $\|P_{\text{proj}} - \text{query\_pt}\|_2 \le \text{snap\_radius\_upm}$, records $P_{\text{proj}}$ as a ray-extension snap candidate so truncated diagonal stems can slide along their exact slope without changing angle.
   - Returns the matching `TangentExtensionSnap` (`VirtualApex` prioritized when within `snap_radius_upm`, otherwise nearest candidate).

---

## 3. Rust Data Structures & Public API Contracts

```rust
pub const TUNNI_MIN_RATIO: f64 = 0.20;
pub const TUNNI_MAX_RATIO: f64 = 0.95;
pub const TUNNI_HIT_RADIUS_DP: f32 = 6.0;

/// Tangent Balance (Tunni) geometry for a single cubic Bézier segment `(P0, P1, P2, P3)`.
#[derive(Debug, Clone, Copy, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct TunniSegmentOverlay {
    pub contour_id: ContourId,
    pub segment_index: usize,
    pub p1: FontPoint,
    pub p2: FontPoint,
    pub tunni_point: FontPoint,
    pub apex: FontPoint,
    pub ratio_p1: f64,
    pub ratio_p2: f64,
}

/// Overlay granularity for cross-master compatibility diagnostics.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum MasterCompatibilityMode {
    OriginAndAnchorRays,
    FullNodeTrajectories,
}

/// Cross-master trajectory intersection indicating a shifted start node or reversed winding.
#[derive(Debug, Clone, Copy, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct TrajectoryCrossing {
    pub contour_idx: usize,
    pub companion_layer_id: u32,
    pub node_index_a: usize,
    pub node_index_b: usize,
    pub intersection: FontPoint,
}

/// Smooth-node interpolation kink warning when tangent angle and handle ratio diverge across masters.
#[derive(Debug, Clone, Copy, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct KinkWarning {
    pub contour_idx: usize,
    pub node_id: NodeId,
    pub companion_layer_id: u32,
    pub severity: f64,
}

/// Combined report of full node trajectories, crossings, and smooth-node kinks.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct MasterKinkAndCrossingReport {
    pub node_trajectories: Vec<(FontPoint, FontPoint)>,
    pub crossings: Vec<TrajectoryCrossing>,
    pub kink_warnings: Vec<KinkWarning>,
}

/// Classification of the 4 classic vector outline defects on the 2D canvas.
#[derive(Debug, Clone, Copy, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum OutlineAuditKind {
    MissingExtremum {
        contour_idx: usize,
        seg_idx: usize,
        t: f64,
        point: FontPoint,
    },
    QuasiOrthogonalStem {
        contour_idx: usize,
        seg_idx: usize,
        angle_deviation_deg: f64,
    },
    CrossedHandles {
        contour_idx: usize,
        seg_idx: usize,
    },
    CollinearCubic {
        contour_idx: usize,
        seg_idx: usize,
    },
}

/// Single finding emitted by `audit_layer_outlines`.
#[derive(Debug, Clone, Copy, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct OutlineAuditFinding {
    pub contour_id: ContourId,
    pub kind: OutlineAuditKind,
}

/// Warning for an `OffCurve` handle attached to a `Smooth` extremum that deviates from orthogonal/italic axes.
#[derive(Debug, Clone, Copy, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct AngledHandleWarning {
    pub contour_idx: usize,
    pub smooth_node_id: NodeId,
    pub handle_node_id: NodeId,
    pub deviation_deg: f64,
}

/// Handle-length percentages `(ratio_in, ratio_out)` relative to tangent intersection apex `T`.
#[derive(Debug, Clone, Copy, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct SegmentHandleRatioHud {
    pub contour_idx: usize,
    pub seg_idx: usize,
    pub apex: FontPoint,
    pub ratio_in: f64,
    pub ratio_out: f64,
}

/// Snap target classification for diagonal leg ray extensions and virtual apex intersections.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum TangentExtensionSnapKind {
    RayExtension {
        contour_idx: usize,
        seg_idx: usize,
    },
    VirtualApex {
        contour_idx_a: usize,
        seg_idx_a: usize,
        contour_idx_b: usize,
        seg_idx_b: usize,
    },
}

/// Result of `compute_tangent_extension_snaps`.
#[derive(Debug, Clone, Copy, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct TangentExtensionSnap {
    pub snap_point: FontPoint,
    pub distance_upm: f64,
    pub kind: TangentExtensionSnapKind,
}

/// Evaluates the signed parametric curvature $\kappa(t)$ of cubic Bézier $(P_0, P_1, P_2, P_3)$ at $t \in [0.0, 1.0]$.
pub fn cubic_signed_curvature(
    p0: FontPoint,
    p1: FontPoint,
    p2: FontPoint,
    p3: FontPoint,
    t: f64,
) -> f64;

impl GrafiaCanvasState {
    // 2.1 G2 Smoothness Continuity & Harmonize (Ctrl+Shift+H)
    pub fn set_node_genius(&mut self, node_id: NodeId, enabled: bool);
    pub fn harmonize_selected_nodes(&mut self);
    pub fn harmonize_handles_keep_node(&mut self, node_id: NodeId);

    // 2.2 Interactive Tangent Balance (Tunni) Lines
    pub fn toggle_show_tunni_lines(&mut self);
    pub fn compute_tunni_overlays(&self) -> Vec<TunniSegmentOverlay>;
    pub fn hit_test_tunni_line_at_upm(&self, pt: FontPoint) -> Option<(ContourId, usize)>;
    pub fn scale_segment_tunni_ratio(&mut self, contour_id: ContourId, segment_index: usize, ratio: f64);
    pub fn drag_tunni_line_to_upm(&mut self, contour_id: ContourId, segment_index: usize, target_upm: FontPoint);
    pub fn balance_segment_tunni(&mut self, contour_id: ContourId, segment_index: usize);

    // 2.3 Proportional Handle Servo & Interpolated Neighbour Nudge (Shortcut C)
    pub fn toggle_power_nudge(&mut self);
    pub fn servo_translate_on_curve_node(&mut self, node_id: NodeId, delta: FontPoint);
    pub fn power_nudge_translate_selected(&mut self, delta: FontPoint);

    // 2.4 Master Trajectory Crossings, Smooth-Node Interpolation Kink Detector & Auto-Match
    pub fn set_master_compatibility_mode(&mut self, mode: MasterCompatibilityMode);
    pub fn compute_master_kinks_and_crossings(&self) -> MasterKinkAndCrossingReport;
    pub fn auto_match_master_compatibility(&mut self, companion_layer_id: u32) -> bool;

    // 2.5 Proactive Vector Outline Audit
    pub fn audit_layer_outlines(&self) -> Vec<OutlineAuditFinding>;

    // 2.6 Off-Axis Handle Warning & Handle-Ratio HUD
    pub fn compute_angled_handle_warnings(&self) -> Vec<AngledHandleWarning>;
    pub fn compute_segment_handle_ratios(&self) -> Vec<SegmentHandleRatioHud>;

    // 2.7 Diagonal Leg / Tangent Extension & Virtual Apex Snapping
    pub fn compute_tangent_extension_snaps(
        &self,
        query_pt: FontPoint,
        snap_radius_upm: f64,
    ) -> Option<TangentExtensionSnap>;
}
```

---

## 4. Complete Normative Test Matrix (`TC-V080-001` .. `TC-V080-017`)

All 17 test cases are implemented under `#![cfg(feature = "v080_red")]`:

| Test Case ID | Requirement / Mathematical Contract | Input / Action | Expected Normative Outcome |
| :--- | :--- | :--- | :--- |
| **`TC-V080-001`** | Parametric Cubic Signed Curvature $\kappa(t)$ (`cubic_signed_curvature`) | Evaluate $\kappa(0)$, $\kappa(0.5)$, $\kappa(1)$ on a radius-$100$ quarter circle and a reflected clockwise arc. | Quarter circle yields $\kappa \approx \pm 0.01\;\text{UPM}^{-1}$ with opposite sign under reflection; straight collinear cubic yields $\kappa(t) = 0.0$. |
| **`TC-V080-002`** | `Ctrl+Shift+H` Romer/Henestrosa $G^2$ Harmonize (`harmonize_selected_nodes`) | Construct two joined cubics at smooth node $P$ with unequal outer handle heights $d_{\text{in}} = 100, d_{\text{out}} = 25$ ($p_0 = 2.0$); dispatch `Ctrl+Shift+H`. | Relocates $P$ along $[Q_2, R_1]$ so $\|P - Q_2\| / \|R_1 - P\| = 2.0$ and equalizes signed curvature $|\kappa_{\text{in}}(1) - \kappa_{\text{out}}(0)| < 10^{-5}$. |
| **`TC-V080-003`** | Handle-Only $G^2$ Harmonize (`harmonize_handles_keep_node`) | Invoke `harmonize_handles_keep_node` on a smooth node $P$ with asymmetric $d_{\text{in}}, d_{\text{out}}$. | On-curve node $P$ remains at its exact coordinates while adjacent handle lengths $(a, b)$ adjust to satisfy $\kappa_{\text{in}}(1) = \kappa_{\text{out}}(0)$. |
| **`TC-V080-004`** | Live $G^2$ Smoothness Node Enforcement (`set_node_genius`) | Enable $G^2$ constraint on smooth node $P$ (`set_node_genius(P, true)`) and translate one adjacent off-curve handle. | Opposite handle and/or join anchor automatically re-balance to preserve $G^1$ collinearity and $G^2$ curvature continuity $\kappa_{\text{in}}(1) = \kappa_{\text{out}}(0)$. |
| **`TC-V080-005`** | Tangent Balance (Tunni) Overlay Computation & Hit-Testing (`compute_tunni_overlays`, `hit_test_tunni_line_at_upm`) | Enable `show_tunni_lines`, compute `compute_tunni_overlays()`, and query `hit_test_tunni_line_at_upm` near the midpoint $M_{\text{tunni}}$ and far away. | Returns `Some((contour_id, 0))` within `6.0 dp` of $[P_1, P_2]$ and `None` outside hit tolerance; `tunni_point` equals $\frac{1}{2}(P_1 + P_2)$. |
| **`TC-V080-006`** | Interactive Tangent Balance Drag & Clamping (`[0.20, 0.95]` — `scale_segment_tunni_ratio`) | Parameterized test scaling balance ratio to `0.10` (below min), `0.55`, `0.75`, and `0.99` (above max). | Handle ratios clamp strictly into $[0.20, 0.95]$; both handles remain on their original tangent rays toward apex $T$. |
| **`TC-V080-007`** | Tangent Balance / Parallelize (`balance_segment_tunni`) | Call `balance_segment_tunni` on an asymmetric cubic segment with $t_1 = 0.40, t_2 = 0.80$. | Both handle ratios equalize to $\bar{t} = 0.60$, making the balance line $[P_1, P_2]$ parallel to the segment chord $[P_0, P_3]$. |
| **`TC-V080-008`** | Proportional Handle Servo on Extremum Translation (`servo_translate_on_curve_node`) | Translate on-curve node $P_0=(0,0) \to (-50, 0)$ on cubic $(P_0, P_1=(0, 60), P_2=(40, 100), P_3=(100, 100))$ via `servo_translate_on_curve_node`. | Horizontal span grows $1.5\times$ ($100 \to 150$), scaling $P_2.x$ proportionally so $(P_3.x - P_2.x) / (P_3.x - P_0.x) = 0.60$ stays invariant. |
| **`TC-V080-009`** | Interpolated Neighbour Nudge (`C` Shortcut & `power_nudge_translate_selected`) | Toggle Neighbour Nudge with key `C`, select an inner stem node at $x = 200$ between stationary extrema $x = 0$ and $x = 400$ with an unselected intermediate node at $x = 100$, and nudge by $\Delta x = +40$. | Selected node moves by $+40$ ($200 \to 240$), intermediate unselected node at $x = 100$ interpolates by $+20$ ($100 \to 120$), and stationary extremum at $x = 0$ stays at $0.0$. |
| **`TC-V080-010`** | Atomic Undo / Redo Round-Trip Across Harmonize, Tangent Balance & Neighbour Nudge | Execute `harmonize_selected_nodes`, `scale_segment_tunni_ratio`, and `power_nudge_translate_selected`, then `Undo` and `Redo` each step. | Every operation restores prior and post snapshots bit-exactly across `Undo` and `Redo`. |
| **`TC-V080-011`** | Pass-4 Headless Rendering Parity with Tangent Balance Lines & $G^2$ Indicators | Render canvas with `show_tunni_lines = true` and active $G^2$-constrained nodes in Light and Dark appearance modes. | Produces deterministic, distinct non-empty pixel buffers in Light vs. Dark modes with balance line overlays visible. |
| **`TC-V080-012`** | Stateful Pull-Based `arbtest` Curve Math Fuzzing (`G2 + Harmonize + Balance + Servo + Nudge + Undo/Redo`) | Run 250 ms pull-based `arbtest` action sequences mixing `Ctrl+Shift+H`, $G^2$ node drags, ratio scaling/balancing, Servo moves, Neighbour Nudge, and `Undo`/`Redo`. | All coordinates remain finite, balance ratios stay in $[0.20, 0.95]$, and `state.validate_invariants()` holds after every step. |
| **`TC-V080-013`** | `MasterCompatibilityMode::FullNodeTrajectories`, Trajectory Crossing & Smooth-Node Kink Detector (`compute_master_kinks_and_crossings`) | Set `FullNodeTrajectories` mode; test a shifted start-node companion contour (crossed trajectories) and a smooth join with $\Delta\theta = 12^\circ > 1^\circ$ and $|r_A - r_B| = 0.25 > 0.05$. | Emits full node-by-node trajectory vectors, flags the crossed trajectory pair in `crossings`, and reports `KinkWarning` at the smooth node with positive `severity`. |
| **`TC-V080-014`** | Proactive Vector Outline Audit (`audit_layer_outlines` — `MissingExtremum`, `QuasiOrthogonalStem`, `CrossedHandles`, `CollinearCubic`) | Audit a contour containing an unsplit S-arch missing its vertical extremum (plus italic-slanted check), a $89.2^\circ$ near-vertical stem, a self-crossing handle pair, and a flat collinear cubic. | `audit_layer_outlines()` deterministically flags all 4 `OutlineAuditKind` variants with accurate `contour_idx` and `seg_idx`. |
| **`TC-V080-015`** | Off-Axis Handle Warning & Handle-Ratio Percentage HUD (`compute_angled_handle_warnings`, `compute_segment_handle_ratios`) | Attach a $3^\circ$-tilted off-curve handle to a `Smooth` extremum and evaluate `compute_angled_handle_warnings()` and `compute_segment_handle_ratios()` on a $60\% / 75\%$ cubic arc. | Flags the tilted extremum handle with `deviation_deg ≈ 3.0` and reports `(ratio_in, ratio_out) ≈ (75.0, 60.0)` around apex $T$. |
| **`TC-V080-016`** | Diagonal Leg / Tangent Extension & Virtual Apex Snapping (`compute_tangent_extension_snaps`) | Construct truncated diagonal legs of `/A` converging toward virtual apex $(200, 700)$; query `compute_tangent_extension_snaps` near $(200, 700)$ and along a leg ray extension. | Snaps to `TangentExtensionSnapKind::VirtualApex` at $(200, 700)$ near the apex and `TangentExtensionSnapKind::RayExtension` along the extended leg slope. |
| **`TC-V080-017`** | Automatic Cross-Master Contour Order, Winding & Cyclic `Node 0` Matching (`auto_match_master_compatibility`) | Construct a 2-contour foreground glyph where contours 0 and 1 are swapped in order, contour 0 has reversed winding, and contour 1 has its `Node 0` shifted by 2 nodes relative to `CompanionMasterLayer(1)`; invoke `state.auto_match_master_compatibility(1)`. | Reorders contours to match `CompanionMasterLayer(1)`, corrects winding parity, rotates `Node 0` to minimize $\sum \|P_{(i+k)\bmod N} - Q_i\|^2$ eliminating all `TrajectoryCrossing`s, and round-trips cleanly through `Undo` / `Redo`. |
