---
type: specification
id: SPEC-013
version: 0.12.0
status: normative
parent: SPEC-001
---

# SPEC-013: `grafia v0.12.0` — Interactive Structural `Stem` Tool, Curved Master Trajectories, Local `Trowel` & `Cleaner` Brushes, `QuickPreviewOnDrag` & Scanline Optical Rhythm Integrals

> **Normative References**:
> - `SPEC-005` & `SPEC-006`: `grafia v0.4.0`..`v0.5.0` — Schneider Cubic Curve Fitting, Multi-Glyph `TextTool` Stream & Scanline Rays
> - `SPEC-008`: `grafia v0.7.0` — `CompanionMasterLayer` Point-Number Compatibility
> - `SPEC-011`: `grafia v0.10.0` — $G^2$ Curve Harmonization, Proportional Handle Servo & Master Trajectory Kinks
> - `SPEC-012`: `grafia v0.11.0` — Orthogonal Stem Thickness Gauge, Green's Theorem Area & Symmetry Overlays
> - Standards: `ISO/IEC/IEEE 42010:2011` (Architecture Description), `ISO/IEC/IEEE 29119-3:2021` (Test Documentation), `IEEE 1012-2016` (System & Software Verification and Validation), `SemVer 2.0.0`
> - Mathematical Literature: Gerald Farin (*Curves and Surfaces for CAGD — Non-Linear Point Trajectories*), Cavalieri / Simpson Scanline Area Quadrature for Planar Inter-Contour Channels

---

## 1. Normative Mathematical & CAGD Foundations

All paired-wall structural stem transformations, quadratic/cubic cross-master point trajectories, local brush refitting operators, and scanline inter-contour white-space integrals in `SPEC-013` are grounded in Computational Geometry and CAGD while keeping the 2D contour model 100% pure (zero procedural CAD modifier trees):

| Subsystem | Domain Standard / Mathematical Model | Core Invariant / Equation | Verified Canvas Semantics |
| :--- | :--- | :--- | :--- |
| **1. Interactive Structural `Stem` Tool & Measurement Rainbow** | Anti-Parallel Normal Wall Pairing & Servo-Coupled Offset Translation | $\hat{n}_L \cdot \hat{n}_R \le -\cos 12^\circ$, $\;d_\perp' = d_\perp + \Delta w$ | Pairs opposing horizontal, vertical, and diagonal stem walls (`A`, `V`, `W`, `H`, `O`), adjusts thickness/position/angle directly on real Bézier contours while servo-scaling adjacent curve handles without kinks, shades continuous interior thickness rainbows, and emits `PropagateStemDelta`. |
| **2. Non-Linear Curved Master Trajectories (`CurvedTrajectoryLink`)** | Quadratic $B_2(t)$ & Cubic $B_3(t)$ Cross-Master Point Migration | $P(t) = (1-t)^2 P^{(A)} + 2(1-t)t C_1 + t^2 P^{(B)}$ | Replaces linear $(1-t)P^{(A)} + t P^{(B)}$ node interpolation with interactive 1- or 2-control-point Bézier trajectories (`control1`, `control2`) so rotating or diagonal strokes preserve optical stem weight at intermediate $t \in (0,1)$. |
| **3. Local `Trowel` Smoothing & `Cleaner` Micro-Loop Pruning Brushes** | Windowed Least-Squares Schneider Refit & Winding Micro-Cycle Elimination | $\forall P_k \notin \mathcal{B}(c, R):\; P_k' \equiv P_k$, $\;|A(\gamma_{\text{loop}})| < \tau_{\text{clean}}$ | `Trowel` refits and simplifies only the contiguous sub-chain touched by brush disc $\mathcal{B}(c, R)$ while leaving outside nodes bit-identical; `Cleaner` heals self-intersecting micro-loops, spikes, and near-duplicate nodes on sweep. |
| **4. `QuickPreviewOnDrag` & Subtle Metric Overshoot Bands** | Pointer-Down Wireframe Suppression & Translucent Zone Ribbons | $y \in [M, M + o_{\text{round}}]$ for $M \in \{\text{cap}, \text{x-height}\}$, $[-\!o_{\text{round}}, 0]$ | Suppresses Pass-3/Pass-4 wireframe handles/badges while primary mouse button is held (`quick_preview_on_drag == true`), and draws low-opacity overshoot ribbons (`round_overshoot_upm`) even when full alignment zones are hidden. |
| **5. Scanline Optical Inter-Shape Whitespace Integrals & Live Axis Mirror** | Weighted Horizontal Scanline Channel Quadrature $V_{\text{opt}} = \int_{y_0}^{y_1} w(y)\,\min(g(y), d_{\max})\,dy$ | $g(y) = x_{\min}^{(R)}(y) - x_{\max}^{(L)}(y)$ | Computes deterministic optical inter-slot whitespace volume, minimum collision clearance, and translucent "water-fill" rhythm overlays across `GlyphSlot` pairs in `TextTool` mode, plus clean 1-axis live mirror drawing (`VerticalMirror` / `HorizontalMirror`). |

---

## 2. Normative Mathematical & Functional Requirements

### 2.1 Interactive Structural `Stem` Tool & Measurement Rainbow (`src/core/stem_tool.rs`)

1. **Paired-Wall Stem Detection (`detect_structural_stems`)**:
   - Pairs opposing contour segments $(S_L, S_R)$ whose inward unit normals satisfy $\hat{n}_L \cdot \hat{n}_R \le -\cos 12^\circ$ with mutual tangential overlap $> 1.2 \cdot d_\perp$, supporting horizontal, vertical, slanted-italic, and diagonal stems directly on standard contours without introducing opaque wrapper objects.
2. **Topology-Preserving Stem Pose & Thickness Editing (`CanvasTool::Stem`)**:
   - Dragging a detected stem or entering an exact UPM / `%` delta adjusts stem thickness (`SymmetricCenter`, `LockLeft`, `LockRight`) or stem slope angle while proportionally scaling incident `OffCurve` handles via the `SPEC-011` servo equations so arches and shoulders remain $G^1$/$G^2$-smooth.
   - When `propagate_across_glyphs == true`, emits `CanvasHostRequest::PropagateStemDelta { orientation, old_width_upm, new_width_upm, angle_delta_deg }` inside one Undo transaction `"Adjust Stem"`.
3. **Measurement Rainbow Heatmap**:
   - When `show_measurement_rainbow == true`, Pass 2 shades interior cross-sections between paired contour walls with a continuous spectral color ramp mapped to perpendicular thickness $d_\perp(s)$, immediately revealing unintentional pinching or swelling in curved bowls and diagonals.

---

### 2.2 Non-Linear Curved Master Trajectories (`CurvedTrajectoryLink` — `src/core/curved_trajectory.rs`)

1. **Data Contract & Evaluation (`evaluate_curved_master_point`)**:
   ```rust
   pub struct CurvedTrajectoryLink {
       pub node_id: NodeId,
       pub companion_layer_index: usize,
       pub control1: FontPoint,
       pub control2: Option<FontPoint>, // None = quadratic B2, Some = cubic B3
   }
   ```
   - Evaluates intermediate node position at normalized axis parameter $t \in [0, 1]$ along quadratic $B_2(P^{(A)}, C_1, P^{(B)}; t)$ or cubic $B_3(P^{(A)}, C_1, C_2, P^{(B)}; t)$, falling back to linear interpolation when no `CurvedTrajectoryLink` is registered for `node_id`.
2. **Interactive Trajectory Handle Editing (`CanvasTool::CurvedTrajectory`)**:
   - Renders the curved migration arc between `active_layer` and `companion_layer_index`, interactive `control1`/`control2` handles, and a live translucent interpolated ghost contour at preview $t \in (0, 1)$ (default `0.5`), recording atomic Undo entry `"Edit Master Trajectory"`.

---

### 2.3 Local `Trowel` & `Cleaner` Brushes (`src/core/local_brushes.rs`)

1. **`CanvasTool::Trowel` (Windowed Contour Smoothing & Node Reduction)**:
   - Sweeping a circular screen-space brush of radius $R_{\text{dp}}$ collects the contiguous chain of nodes/segments intersecting $\mathcal{B}(c, R_{\text{upm}})$, samples the existing curve densely, and refits a minimal cubic chain within `trowel_fit_tolerance_upm` clamped to the exact boundary positions and tangents at the chain entry/exit nodes so every node outside $\mathcal{B}(c, R_{\text{upm}})$ remains 100% bit-identical.
2. **`CanvasTool::Cleaner` (Micro-Loop, Zero-Length & Stray-Spike Healer)**:
   - Sweeping over self-intersecting micro-loops ($|A| < \tau_{\text{clean}}$, default `25.0 UPM²`), collinear redundant nodes, or zero-length degenerate handles prunes the defect loop and heals the remaining contour smoothly inside one Undo entry `"Clean Contour"`.

---

### 2.4 `QuickPreviewOnDrag` & Subtle Metric Overshoot Bands (`src/render/preview_bands.rs`)

1. **`quick_preview_on_drag: bool`**:
   - When enabled, holding the primary pointer button during node, handle, or segment dragging suppresses Pass-3/Pass-4 wireframe nodes, handles, rulers, and badges while rendering crisp filled Non-Zero ink in Pass 2; releasing the pointer button immediately restores full wireframe handles.
2. **Subtle Metric Overshoot Ribbons (`round_overshoot_upm: Option<f64>`)**:
   - Even when full alignment zones are hidden (`show_alignment_zones == false`), if `round_overshoot_upm == Some(o)` with $o > 0$, Pass 1 renders subtle low-opacity (`alpha <= 0.08`) horizontal overshoot bands on $[y_{\text{cap}}, y_{\text{cap}} + o]$, $[y_{\text{x}}, y_{\text{x}} + o]$, and $[-o, 0]$.

---

### 2.5 Scanline Optical Inter-Shape Rhythm Integrals & Clean 1-Axis Live Mirror (`src/core/optical_rhythm.rs`)

1. **Deterministic Inter-Contour Optical Whitespace Profile (`compute_pair_optical_whitespace`)**:
   ```rust
   pub struct OpticalPairConfig {
       pub y_min_upm: f64,
       pub y_max_upm: f64,
       pub sample_count: usize,      // default 64 scanlines
       pub max_cavity_depth_upm: f64, // caps deep open counters (e.g. 'c', 'L', 'T')
       pub italic_angle_deg: f64,
   }

   pub struct OpticalPairMetrics {
       pub optical_area_upm2: f64,
       pub min_clearance_upm: f64,
       pub min_clearance_y_upm: f64,
       pub scanline_spans: Vec<(f64, f64, f64)>, // (y_upm, x_left_exit, x_right_entry)
   }
   ```
   - Casts `sample_count` italic-sheared horizontal scanlines across `[y_min_upm, y_max_upm]` between adjacent shapes $(G_L, G_R)$, clamps open-side cavity penetration to `max_cavity_depth_upm` (preventing deep open pockets like `'L'` or `'c'` from skewing perceived rhythm), integrates `optical_area_upm2` via Simpson/trapezoidal quadrature, and reports exact `min_clearance_upm` collision distance.
   - When `show_optical_rhythm_fill == true` in `TextTool` mode, Pass 1 renders the bounded `scanline_spans` as calm translucent "water-volume" polygons between neighboring `GlyphSlot`s so uneven optical rhythm is immediately visible.
2. **Clean 1-Axis Live Mirror Drawing (`ActiveMirrorAxis::{ None, Vertical { x_upm: f64 }, Horizontal { y_upm: f64 } }`)**:
   - Reflects live `Pen`, `Pencil`, `Primitives`, and `CalligraphicBrush` strokes across a single horizontal or vertical symmetry axis in real time as standard Bézier contours inside one Undo step.

---

## 3. Complete Normative Test Matrix (`TC-V0120-001` .. `TC-V0120-012`)

All 12 test cases are anchored in `tests/v0120_complete_tests.rs` under `#![cfg(feature = "v0120_red")]`:

| Test ID | Subsystem | Verification Scenario | Expected Invariant |
| :--- | :--- | :--- | :--- |
| **`TC-V0120-001`** | Structural `Stem` Detection (Orthogonal & Diagonal) | Detect stems on `'H'` (2 vertical + 1 crossbar) and `'V'` (2 diagonal legs). | Identifies exact opposing wall pairs with perpendicular thickness $d_\perp$ within $0.1\;\text{UPM}$. |
| **`TC-V0120-002`** | Structural `Stem` Thickness Edit, Handle Servo & `PropagateStemDelta` | Widen vertical stem connected to a smooth arch by $+20\;\text{UPM}$ (`SymmetricCenter`) + `Undo`. | Stem width increases by $+20.0\;\text{UPM}$, arch join remains $G^1$-collinear without kinks, emits `PropagateStemDelta`, and round-trips `Undo`. |
| **`TC-V0120-003`** | Measurement Rainbow Heatmap Rendering Parity | Enable `show_measurement_rainbow = true` on curved ring `'O'` and render via `egui` and `tiny-skia`. | Produces non-uniform interior thickness shading with `SSIM >= 0.985` between `egui` and `tiny-skia`. |
| **`TC-V0120-004`** | Quadratic & Cubic `CurvedTrajectoryLink` Evaluation | Register `CurvedTrajectoryLink` (`control1 = (50, 120)`) between $P^{(A)}=(0,0)$ and $P^{(B)}=(100,0)$. | Matches $P^{(A)}$ at $t=0$, $P^{(B)}$ at $t=1$, and evaluates to $(50, 60)$ at $t=0.5$ instead of linear $(50, 0)$. |
| **`TC-V0120-005`** | `CanvasTool::Trowel` Strictly Local Refitting | Sweep `Trowel` (`R = 40 dp`) across top half of a 40-node noisy circle contour. | Reduces node count inside top half while bottom-half node coordinates remain 100% bit-identical (`0.0` delta). |
| **`TC-V0120-006`** | `CanvasTool::Cleaner` Micro-Loop & Duplicate Node Healing | Sweep `Cleaner` over a contour containing a $3 \times 3\;\text{UPM}$ self-intersecting micro-loop and duplicate node. | Eliminates micro-loop and duplicate node, leaving a valid simple closed contour with atomic `Undo`. |
| **`TC-V0120-007`** | `QuickPreviewOnDrag` Wireframe Suppression & Overshoot Ribbons | Render frame during active primary-button node drag with `quick_preview_on_drag = true`, then after pointer release; inspect overshoot bands. | Wireframe handle markers disappear during drag and reappear immediately upon pointer release; overshoot ribbons render at `round_overshoot_upm`. |
| **`TC-V0120-008`** | Scanline Optical Whitespace Quadrature (`HH` vs. `OO` Compensation) | Evaluate `compute_pair_optical_whitespace` on two straight rectangles (`HH`) vs. two circles (`OO`) at equal mechanical sidebearings. | `OO` reports strictly larger `optical_area_upm2` than `HH` due to convex curvature recession, and solving for equal `optical_area_upm2` yields tighter sidebearings on `OO`. |
| **`TC-V0120-009`** | Scanline Cavity Clamping & Minimum Collision Clearance | Evaluate `compute_pair_optical_whitespace` on `'L'` + `'T'` pair with deep open cavities and negative kerning. | `max_cavity_depth_upm` bounds open-pocket area deterministically and `min_clearance_upm` reports exact closest horizontal approach. |
| **`TC-V0120-010`** | Clean 1-Axis Live Mirror Drawing & Rhythm Overlay Parity | Draw 1 stroke under `ActiveMirrorAxis::Vertical { x_upm: 300.0 }` and render `show_optical_rhythm_fill = true` in `egui` & `tiny-skia`. | Emits 2 symmetric contours in one Undo step; rhythm fill achieves `SSIM >= 0.985` parity. |
| **`TC-V0120-011`** | Property-Based `proptest` Stem Deltas, Curved Trajectories & Optical Scans | Fuzz stem deltas $\Delta w \in [-30, 80]$, trajectory $t \in [0, 1]$, and arbitrary pair gap distances. | All outputs remain finite, monotonic in horizontal gap, and invariant-valid (`validate_invariants()`). |
| **`TC-V0120-012`** | Stateful `arbtest` Multi-Tool Fuzzing (`Stem + Trajectory + Trowel + Cleaner + Mirror + Undo`) | Interleave `SPEC-013` operations with `Undo`/`Redo` across 250 ms pull-based sequences. | Bit-exact state restoration across every `Undo`/`Redo` transition. |
