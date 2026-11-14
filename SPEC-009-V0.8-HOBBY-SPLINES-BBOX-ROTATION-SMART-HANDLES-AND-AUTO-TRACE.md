---
type: specification
id: SPEC-009
version: 0.8.0
status: normative
parent: SPEC-001
---

# SPEC-009: `grafia v0.8.0` — Tension-Controlled Hobby Spline Nodes, 8+4 Bounding-Box Rotation/Shear, Parametric Smart Handles, Virtual `ExtraNode`s, Bitmap Auto-Trace & Unified `GrafiaPreferences` Port

> **Normative References**:
> - `SPEC-001`: `grafia v0.1.0` — Architecture, Coordinate Pipeline & Token System
> - `SPEC-002` & `SPEC-003`: `grafia v0.1.0`..`v0.2.0` — Bézier Topology, 8-Knob Selection Bounding Box & Path Operations
> - `SPEC-004` & `SPEC-005`: `grafia v0.3.0`..`v0.4.0` — `Guide`, `SnapEngine`, Boolean Intersection Graph & Schneider Curve Fitting
> - `SPEC-007` & `SPEC-008`: `grafia v0.6.0`..`v0.7.0` — `CanvasBackgroundImage`, Pre-Resolved `CanvasComponent` & Host Request Bridge
> - Standards: `ISO/IEC/IEEE 42010:2011` (Architecture Description), `ISO/IEC/IEEE 29119-3:2021` (Test Documentation), `IEEE 1012-2016` (System & Software Verification and Validation), `ISO 9241-9` (Input Ergonomics), `SemVer 2.0.0`
> - Mathematical Literature: John D. Hobby (*Smooth, Easy to Compute Interpolating Splines*, Discrete & Computational Geometry, 1986), Philip J. Schneider (*An Algorithm for Automatically Fitting Digitized Curves*, Graphics Gems, 1990), William E. Lorensen & Harvey E. Cline (*Marching Squares* isoline extraction)

---

## 1. Normative Mathematical & CAGD Foundations

All interpolating spline solvers, affine rotation/shear handles, constrained parametric component sliders, virtual intersection markers, raster-to-vector contour extraction pipelines, and host preference/thickness ports in `SPEC-009` are grounded in Computer-Aided Geometric Design (CAGD) and `ISO 9241-9`:

| Subsystem | Domain Standard / Mathematical Model | Core Invariant / Equation | Verified Canvas Semantics |
| :--- | :--- | :--- | :--- |
| **1. Tension-Controlled Hobby Spline Nodes (`NodeType::HobbyStar`)** | John D. Hobby Mock-Curvature Tridiagonal System & Velocity Function $\rho(\theta,\phi)$ | $\rho(\theta,\phi) = \frac{2 + \sqrt{2}(\sin\theta - \frac{1}{16}\sin\phi)(\sin\phi - \frac{1}{16}\sin\theta)(\cos\theta - \cos\phi)}{1 + \frac{1}{2}(\sqrt{5}-1)\cos\theta + \frac{1}{2}(3-\sqrt{5})\cos\phi}$ | Solves handle lengths $r_k = \frac{\|P_{k+1}-P_k\|\,\rho(\theta_k,\phi_k)}{3\,\tau_{\text{out},k}}$ across contiguous `HobbyStar` chains without manual BCP handles, supporting directional constraints (`Free`, `Horizontal`, `Vertical`, `SlantedItalic`, `FollowMetricLine`) and destructive `convert_hobby_to_cubic()`. |
| **2. 8+4 Selection Bounding Box Rotation & Shear** | 2D Affine Rotation $R(\Delta\theta)$ & Axis-Parallel Shear $S_x(\gamma), S_y(\gamma)$ around Pivot $O$ | $p' = O + R(\Delta\theta)(p - O)$, $\;\gamma_x = \frac{\Delta x}{y_{\text{edge}} - O_y}$ | Extends the 8-knob scale frame (`SPEC-003`) with 4 outer-corner rotation zones (`14.0 dp` exterior ring, `Shift` $15^\circ$ snap) and 4 mid-edge `Ctrl/Alt` shear zones around the active `TransformGridAnchor` pivot $O$ inside one atomic Undo transaction. |
| **3. On-Canvas Parametric Smart Handles (`CanvasSmartHandle`)** | Bounded 1D Ray Projection $v' = \operatorname{clamp}\!\big(v_{\min} + \frac{(q - A)\cdot\hat{d}}{s}, v_{\min}, v_{\max}\big)$ | $\hat{d} = (\cos\alpha, \sin\alpha)$, $\;H = A + s(v - v_{\min})\hat{d}$ | Renders magenta diamond parametric handles (`5.5 dp`) on pre-resolved `CanvasComponent` instances, constrains pointer drag onto ray $(A, \hat{d})$, snaps to `[v_min, v_default, v_max]`, and emits `CanvasHostRequest::UpdateSmartComponentAxis`. |
| **4. Self-Intersection Virtual `ExtraNode`s** | Pairwise De Casteljau Subdivision & Stable `(seg_a, seg_b, rank)` Addressing | $B_a(t_a) = B_b(t_b), \;(t_a, t_b) \in (0,1)^2$ | Detects interior segment-segment crossings on overlapping stroke contours without destructive `remove_overlap`, renders hollow diamond `ExtraNode` markers, and emits `CanvasHostRequest::SelectOrMutateExtraNode`. |
| **5. Bitmap Auto-Trace (`AutoTraceConfig`)** | Adaptive Integral-Image Thresholding (Bradley/Sauvola) + Optional 4-Corner Homography Deskew + Sub-Pixel Marching Squares + Schneider Fit | $\tau(x,y) = \mu_W(x,y)\big(1 - k\big(1 - \frac{\sigma_W(x,y)}{R}\big)\big)$, $\;\max_i \|B(t_i) - q_i\|_2 \le \varepsilon_{\text{fit}}$ | Handles both flatbed scans (`GlobalLuminance`) and unevenly lit mobile camera photos of paper drawings (`AdaptiveLocal` + optional 4-corner perspective homography), extracting `CCW` outer and `CW` counter cubic contours in one atomic Undo step. |
| **6. Unified `GrafiaPreferences`, Touch/Loupe Ergonomics, `GuideThickness` & `GrafiaKeymap` Port** | Dual Coordinate Stroke Scaling (`ScreenDp` vs. `FontUnits`), Touch Offset Loupe & Data-Driven Key Dispatch | $w_{\text{px}} = \begin{cases} w_{\text{dp}}, & \text{ScreenDp} \\ w_{\text{upm}} \cdot z, & \text{FontUnits} \end{cases}$ | Exposes a single typed host socket for screen stroke widths, zoom-proportional `GuideThickness::FontUnits(w)` bands, nudge/spacing/kerning steps, touch finger-offset + precision magnifier loupe + 2-finger pan/zoom/undo gestures, and conflict-free shortcut rebinding (`GrafiaKeymap`). |

---

## 2. Normative Mathematical & Functional Requirements

### 2.1 Tension-Controlled Hobby Spline (`NodeType::HobbyStar`) & Clothoid Euler/Spiro Spline (`NodeType::EulerSpiro` — `src/core/hobby_spline.rs`)

1. **Data Representation**:
   - Extends `NodeType` with two interpolating spline node variants (allowing Runebender-style and Metafont-style anchor-only curve drawing without manual BCP handles):
     - `HobbyStar { tension_in: f64, tension_out: f64, dir_constraint: HobbyDirConstraint }` (default tensions `(1.0, 1.0)`, clamped to $[0.75, 4.0]$)
     - `EulerSpiro { dir_constraint: HobbyDirConstraint }` (Raph Levien Clothoid / Euler spiral spline where curvature varies linearly with arc length $\kappa(s) = \kappa_0 + \kappa_1 s$)
     where:
     ```rust
     pub enum HobbyDirConstraint {
         Free,
         Horizontal,
         Vertical,
         SlantedItalic,
         FollowMetricLine,
     }
     ```
2. **Tridiagonal Mock-Curvature & Clothoid Euler/Spiro Solver (`evaluate_hobby_chain_to_cubics` & `evaluate_euler_spiro_chain_to_cubics`)**:
   - For any closed cycle of `HobbyStar` / `EulerSpiro` nodes or open sub-chain bounded by fixed incoming/outgoing tangents at adjacent `Corner`/`Smooth` nodes, solves the linear mock-curvature continuity system $\hat{\kappa}_{\text{in}}(P_k) = \hat{\kappa}_{\text{out}}(P_k)$ (or exact Euler clothoid polynomial $\theta(s) = \theta_0 + \kappa_0 s + \frac{1}{2}\kappa_1 s^2$ via Newton-Raphson `kurbo` fitting for `EulerSpiro` nodes) for turning angles $(\theta_k, \phi_k)$.
   - When `dir_constraint != Free` on node $P_k$, pins the unit tangent direction $\hat{T}_k$ to $(1, 0)$, $(0, 1)$, $(\sin\alpha_{\text{italic}}, \cos\alpha_{\text{italic}})$, or the tangent of the nearest horizontal metric line, decoupling the chain into independent sub-tridiagonal blocks at $P_k$.
   - Computes virtual off-curve control points $H_{\text{out},k} = P_k + r_{\text{out},k}\,\hat{T}_k$ and $H_{\text{in},k+1} = P_{k+1} - r_{\text{in},k+1}\,\hat{T}_{k+1}$ dynamically prior to Pass-2 tessellation, boolean intersection, and offscreen `tiny-skia` rasterization, while Pass 4 renders a distinct 5-pointed `★` star marker (`6.0 dp`) for `HobbyStar` and a `◎` spiral-circle marker (`6.0 dp`) for `EulerSpiro` without manual BCP handles.
3. **Explicit Curve Conversions (`Hobby <-> Cubic`, `Quadratic <-> Cubic`, Off-Curve Insertion/Removal), All-Layers Path Operations & `SelectionInfo` Port (`HobbyNodeInfo` & `BackgroundImageInfo`)**:
   - `convert_selected_hobby_nodes_to_cubic()` (and `convert_selected_euler_spiro_nodes_to_cubic()`) materializes the solved virtual BCP handles into standard `Smooth` + `OffCurve` nodes inside an atomic Undo entry `"Convert Hobby/Euler Splines"`.
   - `convert_selected_cubic_nodes_to_hobby()` converts selected `OnCurve` (`Corner`/`Smooth`) nodes into `NodeType::HobbyStar { tension_in: 1.0, tension_out: 1.0, dir_constraint: HobbyDirConstraint::Free }` and strips intermediate `OffCurve` BCP handles between converted neighbors (`Undo` entry `"Convert to Hobby"`).
   - Quadratic/Cubic curve degree conversions on selected contours or segments:
     - `convert_selected_contours_to_quadratic()` approximates cubic segments with quadratic off-curve control points within `0.5 UPM` tolerance, and `convert_selected_contours_to_cubic()` performs exact degree elevation $H_1 = \frac{1}{3}P_0 + \frac{2}{3}Q_1,\; H_2 = \frac{2}{3}Q_1 + \frac{1}{3}P_2$.
     - `add_offcurve_to_selected_segments()` inserts quadratic/cubic off-curve control points at $\frac{1}{3}, \frac{2}{3}$ along selected line segments (or splits a single quadratic off-curve into two with an implied midpoint), and `remove_offcurve_from_selected_segments()` collapses off-curve handles on selected segments back to straight line segments.
   - Multi-layer batch path operation port `apply_path_op_across_all_layers(&mut self, op: LayerPathOp)` executes `LayerPathOp::{ ReverseContours, CorrectPathDirection, TidyUpPaths, AddExtremes { force: bool }, RemoveOverlap, RoundCoordinates }` simultaneously across `state.glyph.contours` and all `editable == true` `CompanionMasterLayer`s inside one atomic Undo entry.
   - Extends `SelectionInfo` / `SelectionInfoAction` (`SPEC-007 §2.2`) with two pure 2D variants:
     - `SelectionInfo::HobbyNode { node_id, x, y, tension_in, tension_out, dir_constraint }` and `SelectionInfoAction::SetHobbyNode { tension_in, tension_out, dir_constraint }`.
     - `SelectionInfo::BackgroundImage { x, y, scale_x_pct, scale_y_pct, rotation_deg, alpha, locked, crop_rect_px }` and `SelectionInfoAction::MutateBackgroundImage(...)` + `ResetBackgroundImageTransform`.
   - Extends `GrafiaCanvasState` with `interpolate_with_background(&mut self, t: f64)` ($t \in [-0.5, 1.5]$), computing the point-wise convex combination $P_k' = (1 - t) P_{\text{fg},k} + t P_{\text{bg},k}$ when `state.glyph.contours` and `state.background_contours` are topologically compatible (`Undo` entry `"Interpolate with Background"`).

---

### 2.2 8+4 Selection Bounding Box Rotation & Shear (`src/core/transform_ops.rs`)

1. **Outer-Corner Rotation Ring**:
   - When `show_bounding_box == true` and $\ge 2$ nodes (or $\ge 1$ contour/component) are selected, hovering within `[6.0 dp, 18.0 dp]` radially outside any of the 4 corner knobs switches the cursor to rotational mode and rotates all selected nodes (together with coupled `OffCurve` handles) around the active `TransformGridAnchor` pivot $O$:
     - Holding `Shift` quantizes $\Delta\theta$ to $15^\circ$ increments.
     - Displays a live `"∠ +24.5°"` telemetry pill near the active corner knob during drag.
2. **Mid-Edge Shear Interaction**:
   - Holding `Ctrl+Alt` (or `Cmd+Opt`) while dragging any of the 4 mid-edge knobs applies horizontal shear (top/bottom edges) or vertical shear (left/right edges) relative to the opposite edge or active pivot $O$, preserving collinear smooth-node tangents and recording one atomic Undo entry `"Transform Selection"`.

---

### 2.3 On-Canvas Parametric Smart Handles (`CanvasSmartHandle` — `src/core/smart_handles.rs`)

1. **Data Contract**:
   ```rust
   pub struct CanvasSmartHandle {
       pub component_id: ComponentId,
       pub axis_id: String,
       pub axis_name: String,
       pub anchor_upm: FontPoint,
       pub direction_unit: (f64, f64),
       pub scale_upm_per_unit: f64,
       pub min_val: f64,
       pub default_val: f64,
       pub max_val: f64,
       pub current_val: f64,
   }
   ```
2. **Interaction & Host Notification**:
   - When a `CanvasComponent` is selected, Pass 4 renders each associated `CanvasSmartHandle` at $H = \text{anchor\_upm} + \text{scale\_upm\_per\_unit}\cdot(\text{current\_val} - \text{min\_val})\cdot\hat{d}$ as a magenta diamond (`5.5 dp`) with a subtle guide track spanning `[min_val, max_val]` and hover/drag value pill `"width: 72.0"`.
   - Dragging projects the pointer onto ray $(\text{anchor\_upm}, \hat{d})$, clamps `current_val` to `[min_val, max_val]`, snaps within `4.0 dp` to `default_val`, and emits `CanvasHostRequest::UpdateSmartComponentAxis { component_id, axis_id, new_value }`.

---

### 2.4 Self-Intersection Virtual `ExtraNode`s (`src/core/extra_nodes.rs`)

1. **Non-Destructive Crossing Indexing**:
   - `compute_layer_extra_nodes(&[GlyphContour]) -> Vec<VirtualExtraNode>` solves all interior segment-segment intersections $(s_a, s_b)$ with $t_a, t_b \in (10^{-4}, 1 - 10^{-4})$ using the `SPEC-005` De Casteljau intersection solver and assigns deterministic canonical keys `ExtraNodeKey { contour_a, seg_a, contour_b, seg_b, crossing_rank }` ordered lexicographically along $t_a$.
2. **Selection & Host Event**:
   - When `show_extra_nodes == true`, Pass 4 renders hollow diamond markers (`5.0 dp`) at each crossing; clicking or invoking context actions emits `CanvasHostRequest::SelectOrMutateExtraNode { key }`.

---

### 2.5 Bitmap & Camera Auto-Trace on `CanvasBackgroundImage` (`AutoTraceConfig` — `src/core/auto_trace.rs`)

1. **Configuration & Pipeline (`trace_background_image(&CanvasBackgroundImage, &AutoTraceConfig) -> Vec<GlyphContour>`)**:
   ```rust
   pub enum AutoTraceThresholdMode {
       GlobalLuminance(u8),                                 // [1, 254], default 128 for clean flatbed scans
       AdaptiveLocal { window_radius_px: u16, bias_k: f32 }, // Integral-image local Sauvola/Bradley threshold (e.g. radius 24, k = 0.18) for shadowed camera photos of paper drawings
   }

   pub struct AutoTraceConfig {
       pub threshold_mode: AutoTraceThresholdMode,
       pub perspective_quad_px: Option<[(f64, f64); 4]>,    // Optional 4-corner planar homography deskew before tracing
       pub invert_polarity: bool,                           // default false (dark ink on light paper/scan)
       pub min_speckle_area_px2: f64,                       // default 9.0
       pub rdp_tolerance_px: f64,                           // default 0.85
       pub corner_angle_deg: f64,                           // default 125.0
       pub fit_error_upm: f64,                              // default 2.5
       pub snap_extrema: bool,                              // default true
   }
   ```
2. **Topological, Illumination & Geometric Guarantees**:
   - When `perspective_quad_px` is `Some(quad)`, rectifies the tilted camera sheet via a projective $3 \times 3$ homography prior to binarization; when `AdaptiveLocal` is active, computes summed-area tables ($O(1)$ per pixel) so uneven hand or room shadows across paper never merge strokes or erase thin hairlines.
   - Sub-pixel bilinear Marching Squares extracts closed pixel-space polylines, maps them through `CanvasBackgroundImage`'s affine UPM transform, discards speckles below `min_speckle_area_px2`, classifies nesting depth via ray-casting so outer boundaries wind `CCW` ($A > 0$) and inner counters wind `CW` ($A < 0$), splits vertices sharper than `corner_angle_deg`, fits Schneider cubic segments within `fit_error_upm`, inserts horizontal/vertical extrema when `snap_extrema == true`, and commits via `trace_active_background_image(&AutoTraceConfig)` as a single Undo step `"Trace Background Image"`.

---

### 2.6 Unified `GrafiaPreferences`, `GuideThickness` & Rebindable `GrafiaKeymap` Port (`src/tokens/preferences.rs`)

1. **Dual-Mode `GuideThickness` on `Guide`**:
   ```rust
   #[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, Default)]
   pub enum GuideThickness {
       #[default]
       ScreenDp,          // Uses theme.edit_canvas.guide_stroke_width_dp (zoom-invariant hairline)
       FontUnits(f64),    // Zoom-proportional filled band [pos - w/2, pos + w/2] in UPM space
   }
   ```
   - When `Guide::thickness == GuideThickness::FontUnits(w)` with $w > 0$, Pass 1 renders a zoom-proportional translucent band of perpendicular thickness $w$ UPM along the guide ray (in addition to its center axis hairline), and `SnapEngine` snaps to both the centerline and the two band edges $\pm \frac{1}{2}w$.
2. **Extended `EditCanvasTokens` Visual Dimensions**:
   - Adds `guide_stroke_width_dp: f32` (`default 1.0`, clamped `[0.5, 4.0]`), `metric_line_width_dp: f32` (`default 1.0`, clamped `[0.5, 4.0]`), `handle_shape: OffCurveHandleShape { Circle, Square, Cross }`, and `full_crosshair_cursor: bool`, plus theme color tokens `smart_handle_color`, `ps_hint_stem_fill`, `ps_hint_ghost_fill`, and `hoi_diamond_color`.
3. **Typed `GrafiaPreferences` Host Configuration (`state.apply_preferences(&GrafiaPreferences)`)**:
   ```rust
   pub struct GrafiaPreferences {
       pub nudge_step_small_upm: f64,                   // default 1.0 (supports e.g. 0.1 for fine grid)
       pub nudge_step_medium_upm: f64,                  // default 10.0 (Shift + Arrow)
       pub nudge_step_large_upm: f64,                   // default 100.0 (Cmd/Ctrl + Arrow)
       pub spacing_step_low_upm: f64,                   // default 1.0
       pub spacing_step_high_upm: f64,                  // default 10.0
       pub kerning_step_low_upm: f64,                   // default 1.0
       pub kerning_step_high_upm: f64,                  // default 10.0
       pub grid_step_upm: f64,                          // default 1.0 (0.0 = floating-point coordinates)
       pub grid_subdivisions: u32,                      // default 1
       pub round_to_grid: bool,                         // default true when grid_step_upm > 0
       pub snap_radius_dp: f32,                         // default 6.0
       pub snap_targets: SnapTargetMask,                // bitflags/struct: guides, metrics, grid, nodes, background, italic_angle
       pub hit_test_radius_dp: f32,                     // default 6.0
       pub text_mode_numbers_threshold: f64,            // default 100.0
       pub scroll_wheel_zooms: bool,                    // default true (false = scroll pans, Alt+scroll zooms)
       pub invert_zoom_direction: bool,                 // default false
       pub fill_active_glyph_while_editing: bool,       // default false
       pub passive_neighbor_opacity: f32,               // default 0.85
       pub italic_angle_compensates_bbox_and_nudge: bool, // default true
       pub freehand_smoothing_tolerance_px: f64,        // default 0.85
       pub touch_finger_offset_dp: f32,                 // default 0.0 on desktop, e.g. 44.0 dp upward offset on direct finger touch so fingertips never obscure nodes
       pub show_touch_precision_loupe: bool,            // default false on desktop, true on compact touch screens (renders 2.5x circular crosshair loupe above finger contact)
       pub two_finger_pan_zoom_enabled: bool,           // default true (pinch zoom + 2-finger pan while single stylus/finger draws or edits)
       pub two_finger_tap_undo_enabled: bool,           // default true on touch/tablet devices
   }
   ```
4. **Rebindable `GrafiaKeymap` (`state.apply_keymap(&GrafiaKeymap)`), Saved Viewport Bookmark & Granular Node Visibility Flags**:
   - Replaces hardcoded key-switch tables with a serializable `GrafiaKeymap` mapping `CanvasActionId` (all tool activations and path/layer/view commands) to `KeyChord { key, cmd, ctrl, alt, shift }`, shipping `GrafiaKeymap::default()` with 100% backward-compatible shortcuts while allowing host preferences dialogs to rebind any chord at runtime.
   - Adds `save_viewport_bookmark(&mut self)` and `toggle_saved_viewport_bookmark(&mut self)` on `GrafiaCanvasState` (storing `saved_viewport: Option<ViewportTransform>` and swapping between current and bookmarked `(pan_x, pan_y, zoom)`).
   - Adds granular Pass-4 node/overlay visibility toggles on `GrafiaCanvasState`: `show_nodes_oncurve: bool` (`default true`), `show_nodes_offcurve: bool` (`default true`), `show_node_coordinates: bool` (`default false`), `show_background_nodes: bool` (`default false`), and `show_component_piece_frames: bool` (`default false`).

---

## 3. Complete Normative Test Matrix (`TC-V080-001` .. `TC-V080-014`)

All 14 test cases are anchored in `tests/v080_complete_tests.rs` under `#![cfg(feature = "v080_red")]`:

| Test ID | Subsystem | Verification Scenario | Expected Invariant |
| :--- | :--- | :--- | :--- |
| **`TC-V080-001`** | Hobby Closed Cycle Symmetry | 4 `HobbyStar` nodes on a $200 \times 200$ diamond with unit tension $(1.0, 1.0)$. | Solved cubic chain is mock-curvature symmetric across all 4 quadrants and matches circular curvature within $10^{-3}$. |
| **`TC-V080-002`** | Hobby Directional Constraints & `convert_hobby_to_cubic` | Mixed `HobbyStar` chain with `Horizontal` and `Vertical` constraints, followed by `convert_selected_hobby_nodes_to_cubic()` + `Undo`. | Constrained tangents align to $0^\circ / 90^\circ$ within $10^{-6}\;\text{rad}$; conversion preserves rendered pixels (`SSIM >= 0.999`) and `Undo` restores `HobbyStar` topology. |
| **`TC-V080-003`** | 8+4 BBox Outer-Corner Rotation & `Shift` $15^\circ$ Snap | Drag outer top-right corner knob with and without `Shift` around center pivot. | Quantizes $\Delta\theta$ to $15^\circ$ multiples with `Shift`, rotates `OnCurve` and `OffCurve` nodes rigidly, and round-trips via `Undo`/`Redo`. |
| **`TC-V080-004`** | 8+4 BBox Mid-Edge Shear | `Ctrl+Alt` drag top mid-edge knob horizontally by $+40\;\text{UPM}$ relative to bottom edge. | Bottom edge remains fixed ($y = y_{\min}$), top edge shifts by $+40\;\text{UPM}$, and smooth-node collinearity holds. |
| **`TC-V080-005`** | `CanvasSmartHandle` Ray Projection & Clamping | Drag smart handle along angled ray (`30°`) past `min_val`, near `default_val`, and past `max_val`. | Clamps strictly to `[min_val, max_val]`, snaps within `4.0 dp` to `default_val`, and emits `UpdateSmartComponentAxis`. |
| **`TC-V080-006`** | Virtual `ExtraNode` Intersection Stability | Two overlapping rectangular stroke contours translated by $(+10, +5)\;\text{UPM}$. | Returns 4 deterministic `ExtraNodeKey` crossings whose order and `(seg_a, seg_b, rank)` keys remain stable under translation. |
| **`TC-V080-007`** | Bitmap & Shadowed-Camera Auto-Trace Winding & Counter Recovery | Synthetic $256 \times 256$ rasterized glyph `'O'` under both uniform illumination (`GlobalLuminance`) and a strong $40\% \rightarrow 95\%$ diagonal shadow gradient (`AdaptiveLocal`) + 2px speckle. | Rejects 2px speckle under illumination gradient, emits 1 `CCW` outer contour + 1 `CW` inner counter contour with finite coordinates and extremum nodes. |
| **`TC-V080-008`** | Bitmap Auto-Trace Corner Detection, Homography & Undo | Perspective-tilted synthetic `'H'` (`90°` corners) traced via `trace_active_background_image` (`perspective_quad_px = Some(...)`) + `Undo`. | Rectifies quad, preserves sharp `Corner` nodes at stem junctions ($\ge 12$ corners), and restores empty foreground on `Undo`. |
| **`TC-V080-009`** | `GuideThickness::ScreenDp` vs. `GuideThickness::FontUnits(f64)` | Render guide A (`ScreenDp`, `guide_stroke_width_dp = 2.0`) and guide B (`FontUnits(32.5)`) at zoom `0.5x` and `2.0x`. | Guide A maintains constant `2.0 px` screen thickness across zoom; Guide B scales from `16.25 px` to `65.0 px` and snaps at center & $\pm 16.25\;\text{UPM}$ edges. |
| **`TC-V080-010`** | `GrafiaPreferences` Custom Nudge/Kerning Steps & `GrafiaKeymap` Rebind | Set `nudge_step_small_upm = 0.25`, `nudge_step_medium_upm = 5.0`, `snap_targets.guides = false`, and rebind `CanvasTool::Knife` to `'K'`. | Arrow / `Shift+Arrow` nudge by `0.25` / `5.0 UPM`, guide snap is bypassed while metric snap stays active, and pressing `'K'` activates `Knife`. |
| **`TC-V080-011`** | `egui` vs. `tiny-skia` Rendering Parity (`v0.8.0` Overlays & Guide Bands) | Render `HobbyStar` markers, BBox rotation pill, `CanvasSmartHandle`, `ExtraNode`s, and `FontUnits(32.5)` guide bands in Light & Dark modes. | Live `egui` mesh and headless `tiny-skia` pixmap achieve `SSIM >= 0.985` with zero panics. |
| **`TC-V080-012`** | Property-Based `proptest` Hobby Tension & BBox Affine Stability | Fuzz tensions $\tau \in [0.75, 4.0]$, rotation $\theta \in [-180^\circ, 180^\circ]$, and shear $\gamma \in [-1.0, 1.0]$. | Zero `NaN`/`Inf` coordinates; `state.validate_invariants()` holds for all inputs. |
| **`TC-V080-013`** | Property-Based `proptest` Auto-Trace & Preferences Ranges | Fuzz `luminance_threshold` $\in [16, 240]$, `nudge_step_small_upm` $\in [0.05, 50.0]$, and `guide_stroke_width_dp` $\in [0.5, 4.0]$. | All traced contours and nudged nodes remain finite, valid, and panic-free. |
| **`TC-V080-014`** | Stateful `arbtest` Multi-Feature Undo/Redo Fuzzing | Interleave `HobbyStar` edits, BBox rotate/shear, smart-handle drags, auto-trace, preference updates, and `Undo`/`Redo`. | Bit-exact snapshot restoration across all `Undo`/`Redo` transitions. |
| **`TC-V080-015`** | `HobbyNode` & `BackgroundImage` `SelectionInfo` + `interpolate_with_background(t)` | Query/mutate `SelectionInfo::HobbyNode` and `SelectionInfo::BackgroundImage`, and run `interpolate_with_background(0.5)` + `Undo`. | Mutates tensions/image transform losslessly; `t = 0.5` places every node at exact midpoint $\frac{1}{2}(P_{\text{fg}} + P_{\text{bg}})$ and restores on `Undo`. |
| **`TC-V080-016`** | `convert_selected_cubic_nodes_to_hobby`, Quadratic/Cubic Degree Conversions, `apply_path_op_across_all_layers` & Viewport Bookmark | Convert cubic $\leftrightarrow$ Hobby, elevate quadratic $\to$ cubic, add/remove off-curve handles, run `apply_path_op_across_all_layers(LayerPathOp::ReverseContours)`, and toggle `save_viewport_bookmark` / `toggle_saved_viewport_bookmark`. | All curve conversions and multi-layer path operations round-trip bit-exactly via `Undo`/`Redo`; `toggle_saved_viewport_bookmark` swaps between current and bookmarked `ViewportTransform`. |
