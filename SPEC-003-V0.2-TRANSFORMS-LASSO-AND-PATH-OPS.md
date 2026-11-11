# SPEC-003 — `grafia v0.2.0`: Transform Tools, 8-Knob Bounding Box, Lasso, Zoom, Clipboard & Path Operations

- **Status:** Normative Specification (Grounded in OpenType 1.9, Computer-Aided Geometric Design (CAGD) & ISO 9241-9 Ergonomic Standards)
- **Target Crate:** `grafia`
- **Version Target:** `v0.2.0`
- **Depends On:** `SPEC-001` (Universal Glyph Canvas Engine), `SPEC-002-V0.1-CANVAS-INTERACTION-AND-RENDERING.md`
- **Normative Standards:** ISO/IEC/IEEE 42010:2011, ISO/IEC/IEEE 29119-3:2021, IEEE 1012-2016, OpenType 1.9, CAGD, ISO 9241-9

---

## 1. Normative Mathematical & Ergonomic Baseline (OpenType 1.9 / CAGD / ISO 9241-9)

| Subsystem | Normative Domain & Shortcut | Mathematical & Ergonomic Specification |
| :--- | :--- | :--- |
| **8-Knob Selection Bounding Box** | Affine Bounding-Box Manipulation (`Cmd/Ctrl+Opt+Shift+B`) | Computed strictly from explicitly selected nodes (unselected attached `OffCurve` handles and cubic curve extrema are excluded), unslanted by $\alpha = \text{italic\_angle\_deg}$ when slanted mode is active. 9-point alignment grid (`0..8`), opposite knob $8 - k$, active knob subset `[1, 7]` when $w \le 1.0\text{ UPM}$ or `[3, 5]` when $h \le 1.0\text{ UPM}$, outward knob offset $1.5 \cdot \text{handle\_size\_dp} / \text{zoom}$, 3-zone ray-projection hit-testing ($t \le 0.7$ miss, $0.7 < t \le 1.7$ scale, $t > 1.7$ rotate/slant), and live Width/Height capsule pill badges. |
| **Dedicated `RotateTool` (`R`) & `ScaleTool` (`S`)** | Explicit Pivot Affine Transformations (`R`, `S`, temporary `Cmd` $\to$ `Select`) | Shared `transform_origin` initialized to the 9-point anchor (`4` = center) of `selection_bounds` and automatically updated when selection changes via temporary `Cmd`-Select override. Click ($\le 2.0\text{ dp}$) relocates `transform_origin`; drag ($< 3.0\text{ dp}$) on `transform_origin` moves it live; drag ($> 2.0\text{ dp}$ and $> 5.0\text{ UPM}$ from pivot) transforms around `transform_origin`. `Shift` snaps rotation to $45^\circ$ increments in `RotateTool` and applies 3-zone drag-angle constrain ($\|\sin\theta\| < 0.2$ horizontal, $\|\sin\theta\| > 0.86$ vertical, $0.2 \le \|\sin\theta\| \le 0.86$ uniform $\min/\max$) in `ScaleTool`. |
| **`CanvasTool::Lasso` (`Shift+V`)** | Freeform Polygon Selection (`Shift+V`) | Disables path-segment drag interaction (`allows_path_segment_interaction == false`) so clicking/dragging on a curve or line segment initiates a freeform lasso polygon (`> 2.0 dp` sampling step). Evaluates even-odd point-in-polygon selection with full modifier support (None = replace `OnCurve`+`OffCurve`, `Alt` = `OnCurve` only, `Shift` = XOR toggle, `Shift+Alt` = XOR `OnCurve` only). |
| **`ZoomTool` (`Z`, `Cmd/Ctrl+Space`)** | Discrete Step & Marquee Viewport Zoom | `10.0 dp` marquee threshold ($w_{\text{scr}} + h_{\text{scr}} \ge 10.0\text{ dp}$): marquee zoom-in ($\text{zoom} \cdot \min(V_w/w_{\text{scr}}, V_h/h_{\text{scr}})$) or `Alt` marquee zoom-out ($\text{zoom} \cdot \min(w_{\text{scr}}/V_w, h_{\text{scr}}/V_h)$). Below $10.0\text{ dp}$, steps through the 23 discrete zoom levels `[0.008 .. 40.0]` (`> 1.08x` zoom-in or `< 0.92x` zoom-out with `Alt`) anchored at cursor. |
| **Canvas Outline Clipboard (`Cmd/Ctrl+C/X/V`)** | Topological Sub-Path Extraction, Collision Offset & Splice | Full-contour vs. partial-chain extraction (including seamless wrap-around across `P0` on closed contours and 0-contour no-op when no `OnCurve` nodes are selected). `Paste` Mode A pastes at `(0, 0)` or `(+10, +10)` UPM collision offset when an identical contour already exists at the exact coordinates; `Paste` Mode B splices a single open clipboard contour into a contiguous $\ge 2$-node selected span on a target contour. `Cut` executes `Copy` followed by `cut_open_delete` (`Shift+Delete`). |
| **7 Topological & Geometric Path Operations** | Contour Orientation, Extremum Subdivision & Path Cleaning | `reverse_selected_contours`, `make_node_first`, `correct_path_direction` (`Cmd/Ctrl+Shift+R`, even-odd nesting depth in italic-unsheared space + lowest-$(y,x)$ start node), `add_extremes(force)` ($d_0, d_3 \ge 0.8\text{ UPM}$ filter + near-endpoint smooth absorption for $0.8 \le d < 30.0\text{ UPM}$ when `force == false`) & `add_inflection_points` ($d_0, d_3 > 20.0\text{ UPM}$), `tidy_up_paths` (`Cmd/Ctrl+Shift+T` + red/yellow coincident-node warning circles), `align_selection` (`Cmd/Ctrl+Shift+A`), and `connect_selected_open_endpoints` ($< 1.0\text{ UPM}$ merge vs. $\ge 1.0\text{ UPM}$ straight line bridge). |

### 1.1 Explicit Non-Goals for `v0.2.0` (Deferred to `v0.3.0+`)
- Measurement Tool (`L`), Knife/Eraser (`E`), Primitives (`F`), Pencil (`B`), Boolean path operations (`Remove Overlap`, `Subtract`, `Intersect`), OpenType compiler (`fea`), and multi-master variable font interpolation.

---

## 2. Normative Mathematical & Behavioral Specifications

### 2.1 8-Knob Selection Bounding Box (`Cmd/Ctrl+Opt+Shift+B`)

#### 2.1.1 Calculation of `selection_bounds`
1. Let $S = \text{selected\_nodes}$ be the ordered list of explicitly selected node IDs in the active glyph layer, and let $\alpha = \text{selection\_angle\_deg}$ be `italic_angle_deg` if `show_bounding_box_slanted == true` and `0.0` otherwise.
2. If $S = \emptyset$, `selection_bounds` is `None`.
3. For every node $n \in S$ (whether `OnCurve` or explicitly selected `OffCurve`), let $p' = \text{unslant}(n.\text{point}, \alpha)$ where:
   $$\text{unslant}(x, y, \alpha) = \left(x - (y - \tfrac{1}{2} x_{\text{height}}) \tan\!\left(\alpha \frac{\pi}{180}\right),\; y\right)$$
4. `selection_bounds` is the axis-aligned bounding box $[\min x', \max x'] \times [\min y', \max y']$ of $\{p' \mid n \in S\}$, with $w = \max x' - \min x'$ and $h = \max y' - \min y'$.
5. **Crucial Invariant:** Unselected attached `OffCurve` handles and cubic Bézier curve extrema between selected nodes are **never** included in `selection_bounds`.
6. **Display Gate:**
   The bounding box and its knobs are rendered and hit-tested if and only if:
   - `show_bounding_box == true` (toggled via **`Cmd/Ctrl+Alt+Shift+B`**),
   - `S.len() >= 2`, and
   - $w \cdot \text{zoom} > 4.0\text{ dp} \;\lor\; h \cdot \text{zoom} > 4.0\text{ dp}$ (`BBOX_MIN_SCREEN_SPAN_DP = 4.0`).

#### 2.1.2 9-Point Grid (`0..8`), Active Knobs & Outward Knob Offset
1. **9-Point Aligned Bounds Lookup (`aligned_point_for_bounds(index, bounds)`):**
   - `0` (`SW`, Bottom-Left): $(x_0, y_0)$
   - `1` (`S`, Bottom-Center): $(x_0 + 0.5w, y_0)$
   - `2` (`SE`, Bottom-Right): $(x_0 + w, y_0)$
   - `3` (`W`, Middle-Left): $(x_0, y_0 + 0.5h)$
   - `4` (`C`, Center — pivot only, no knob): $(x_0 + 0.5w, y_0 + 0.5h)$
   - `5` (`E`, Middle-Right): $(x_0 + w, y_0 + 0.5h)$
   - `6` (`NW`, Top-Left): $(x_0, y_0 + h)$
   - `7` (`N`, Top-Center): $(x_0 + 0.5w, y_0 + h)$
   - `8` (`NE`, Top-Right): $(x_0 + w, y_0 + h)$
2. **Opposite Knob:**
   $$\text{opposite}(k) = 8 - k \quad (k \in \{0,\dots,8\})$$
3. **Active Knobs:**
   - If $w \le 1.0\text{ UPM}$ and $h \cdot \text{zoom} > 4.0\text{ dp}$: only **`[1, 7]` (`S`, `N`)** are active.
   - Else if $h \le 1.0\text{ UPM}$ and $w \cdot \text{zoom} > 4.0\text{ dp}$: only **`[3, 5]` (`W`, `E`)** are active.
   - Else ($w > 1.0\text{ UPM} \land h > 1.0\text{ UPM}$): all 8 perimeter knobs **`[6, 7, 8, 3, 5, 0, 1, 2]`** are active.
4. **Outward Knob Position:**
   - Let `handle_size_dp = 5.0 * zoom.powf(0.1)` (`BBOX_BASE_HANDLE_SIZE_DP = 5.0`).
   - Before evaluating `aligned_point_for_bounds(k, expanded_bounds)` for knob placement, `bounds` is expanded outward on all 4 sides in font units by:
     $$\Delta_{\text{out}} = \frac{1.5 \cdot \text{handle\_size\_dp}}{\text{zoom}}$$
     (so each knob center $P_{\text{knob}}(k)$ sits **`1.5 * handle_size_dp` (`~7.5 dp`) outside** the dashed rectangle boundary point $P_{\text{box}}(k)$).
5. **Visual Styling & Live On-Canvas Width/Height Pill Badges:**
   - **Dashed Box:** dash pattern `[1.75 dp, 1.75 dp]`, stroke width `1.75 dp`, color `rgba(128, 128, 128, 191)` (`white: 0.5, alpha: 0.75`). When `show_bounding_box_slanted == true` and `italic_angle != 0.0`, the 4 corners of the box are sheared horizontally around $y = 0.5 \cdot x_{\text{height}}$ by $+\tan(\text{italic\_angle})$.
   - **8 Circular Knobs:** diameter `handle_size_dp`, fill `rgba(102, 102, 102, 71)` (`white: 0.4, alpha: 0.28`), stroke width `0.8 dp` in `rgba(102, 102, 102, 84)` (`white: 0.4, alpha: 0.33`).
   - **Live Width & Height Dimension Pill Badges:**
     Whenever the Selection Bounding Box is displayed (both at rest and updating continuously during Bounding-Box Scale/Rotate/Slant, `RotateTool`, and `ScaleTool` drags; note that `TransformTool` does **not** draw a separate floating cursor `°` or `%` badge), two capsule pill badges are drawn:
     - **Width Badge:**
       - Text: `round(w * 10.0) / 10.0` formatted with up to 1 decimal place and trailing `.0` stripped (e.g. `"100"` or `"133.3"`).
       - Anchor Point in Font Space: $(\text{mid}X,\; \min Y - 6.0 / \text{zoom})$ (`alignment = 2`: horizontally centered at $\text{mid}X$, top edge `6.0 dp` below the bottom edge of the bounding box).
     - **Height Badge:**
       - Text: `round(h * 10.0) / 10.0` formatted with up to 1 decimal place and trailing `.0` stripped.
       - Anchor Point in Font Space: $(\max X + 6.0 / \text{zoom},\; \text{mid}Y)$ (`alignment = 3`: left edge `6.0 dp` to the right of the right edge of the bounding box, vertically centered at $\text{mid}Y$).
     - **Capsule Pill Styling:**
       - Font: `11.0 dp` regular monospaced-digit UI font.
       - Text Color: `#FFFFFF` (`rgba(255, 255, 255, 255)`).
       - Pill Background Color: `rgba(153, 153, 153, 230)` (`white: 0.6, alpha: 0.9`).
       - Pill Geometry: full capsule (`corner_radius = pill_height * 0.5`), minimum height `12.0 dp`, horizontal width padding `+7.0 dp`, rect inset `-2.0 dp`.

#### 2.1.3 3-Zone Knob Hit-Testing
For each active knob $k$, let `base_dp = 5.0`, $P_{\text{box}}(k)$ be the boundary point on the unexpanded box ($t = 0$), and $P_{\text{knob}}(k)$ be the knob center at $1.5 \cdot \text{base\_dp}$ outside the box ($t = 1$).
If the cursor screen point $Q$ lies within the `3.0 * base_dp × 3.0 * base_dp` (`15.0 dp × 15.0 dp`) square centered on $P_{\text{knob}}(k)$, project $Q$ onto the line through $P_{\text{box}}(k)$ and $P_{\text{knob}}(k)$:
$$t = \frac{(Q - P_{\text{box}}(k)) \cdot (P_{\text{knob}}(k) - P_{\text{box}}(k))}{\|P_{\text{knob}}(k) - P_{\text{box}}(k)\|^2}$$
1. **$t \le 0.7$**: No knob hit (`None` — falls through to normal node/segment/marquee hit-testing).
2. **$0.7 < t \le 1.7$**: **Bounding-Box SCALE (`BBoxAction::Scale(k)`)**:
   - Cursor: `BBoxResizeNWSE` (`k in {6, 2}`), `BBoxResizeNS` (`k in {7, 1}`), `BBoxResizeNESW` (`k in {8, 0}`), `BBoxResizeEW` (`k in {3, 5}`), hotspot `(9.0, 9.0)`.
3. **$t > 1.7$ (within the `15 dp × 15 dp` outer hit box)**:
   - If **`k == 1` (`S` bottom-center knob)** OR **`(is_ctrl && is_alt && k in {3, 5, 7})`**:
     - **Bounding-Box SLANT / SHEAR (`BBoxAction::Slant(k)`)**, cursor `BBoxSlant`, hotspot `(9.0, 9.0)`.
   - Otherwise (all 4 corner knobs `{0, 2, 6, 8}`, and `{3, 5, 7}` without `Ctrl+Alt`):
     - **Bounding-Box ROTATE (`BBoxAction::Rotate(k)`)**, cursor `RotateCursor`, hotspot `(9.0, 9.0)`.

#### 2.1.4 Bounding-Box Scale, Rotate & Slant Math
All bounding-box drags operate against a frozen `shadow_snapshot` captured on `PointerPress` and **never** trigger `Option`-drag contour duplication (`alt_drag_duplicate` is strictly a `SelectTool` node/segment drag operation):
1. **Pivot (`origin`):**
   - Default (without `Alt`, or when `Ctrl+Alt` is held): **opposite knob** $P_{\text{box}}(8 - k)$ on `shadow_selection_bounds`.
   - With **`Alt`/`Option` (and `!is_ctrl`)**: **center** $P_{\text{box}}(4) = (x_0 + 0.5w, y_0 + 0.5h)$ of `shadow_selection_bounds`, and the drag delta $\Delta = (\Delta x, \Delta y)$ is multiplied by **`2.0`** so the dragged knob tracks the cursor!
2. **Scale (`BBoxAction::Scale(k)`):**
   - Let $(\Delta x, \Delta y)$ be the cursor displacement from `drag_start` in the (optionally unslanted) bounding-box coordinate frame, multiplied by `2.0` if `is_alt && !is_ctrl`.
   - Raw scale factors:
     $$s_x = \begin{cases} \frac{w - \Delta x}{w} & \text{if } k \in \{0, 3, 6\} \land w > 10^{-6} \\ \frac{w + \Delta x}{w} & \text{if } k \in \{2, 5, 8\} \land w > 10^{-6} \\ 1.0 & \text{otherwise} \end{cases}, \qquad s_y = \begin{cases} \frac{h - \Delta y}{h} & \text{if } k \in \{0, 1, 2\} \land h > 10^{-6} \\ \frac{h + \Delta y}{h} & \text{if } k \in \{6, 7, 8\} \land h > 10^{-6} \\ 1.0 & \text{otherwise} \end{cases}$$
   - **With `Shift` (proportional constrain):**
     - Corner knobs $k \in \{0, 2, 6, 8\}$: $s = \max(s_x, s_y),\; s_x = s,\; s_y = s$.
     - Horizontal side knobs $k \in \{3, 5\}$: $s_y = s_x$ (uniform 2D scaling driven by horizontal drag!).
     - Vertical side knobs $k \in \{1, 7\}$: $s_x = s_y$ (uniform 2D scaling driven by vertical drag!).
   - **Negative Scale (Mirroring):** Dragging past the opposite side produces $s_x < 0$ or $s_y < 0$, which mirrors node coordinates geometrically via $p' = \text{pivot} + (s_x (p_x - \text{pivot}_x),\; s_y (p_y - \text{pivot}_y))$ **without** reversing contour winding order (`reverse` is never called).
3. **Rotate (`BBoxAction::Rotate(k)`) & Slant (`BBoxAction::Slant(k)`):**
   - **Minimum pivot distance guard:** If $\text{hypot}(\text{drag\_start} - \text{pivot}) < 5.0\text{ UPM}$, no transformation is applied.
   - Let $\Delta\theta_{\text{deg}} = \text{atan2}(y_{\text{curr}} - \text{pivot}_y, x_{\text{curr}} - \text{pivot}_x) - \text{atan2}(y_{\text{start}} - \text{pivot}_y, x_{\text{start}} - \text{pivot}_x)$ in degrees.
   - **For Rotate (`BBoxAction::Rotate(k)`):**
     - If **`is_shift`**: $\Delta\theta_{\text{deg}} = \text{round}(\Delta\theta_{\text{deg}} / 45.0) \cdot 45.0$.
     - Applies 2D rotation by $\Delta\theta_{\text{deg}}$ around `pivot`.
   - **For Slant (`BBoxAction::Slant(k)`):**
     - If $k \in \{1, 7\}$ (`S` or `N`): horizontal shear around `pivot` ($\tan(-\Delta\theta_{\text{deg}} \cdot \pi / 180)$):
       $$x' = x + (y - \text{pivot}_y) \cdot sh_x, \qquad y' = y$$
     - If $k \in \{3, 5\}$ (`W` or `E`): vertical shear around `pivot` ($\tan(+\Delta\theta_{\text{deg}} \cdot \pi / 180)$):
       $$x' = x, \qquad y' = y + (x - \text{pivot}_x) \cdot sh_y$$

#### 2.1.5 3-Pass Partial-Selection `OffCurve` Handle Alignment & Grid-Rounding
Whenever an affine transformation $T$ (from Bounding-Box Scale/Rotate/Slant, `RotateTool`, `ScaleTool`, or `TransformPanel`) is applied to `selected_nodes` relative to `shadow_contours`:
1. **Pass 1 (Explicitly Selected Nodes):**
   Every node $n \in \text{selected\_nodes}$ (`OnCurve` or `OffCurve`) is transformed directly: $n.\text{point} = T(n_{\text{shadow}}.\text{point})$.
2. **Pass 2 (Proportional Nudge Interpolation when `is_ctrl && is_alt`):**
   For any cubic segment where one `OnCurve` endpoint is in `selected_nodes` and the opposite `OnCurve` endpoint is not, unselected `OffCurve` handles are interpolated via `apply_ctrl_alt_interpolate_nudge`.
3. **Pass 3 (Unselected Attached `OffCurve` Handles when `!(is_ctrl && is_alt)`):**
   *(Note: `RotateTool`, `ScaleTool`, and Bounding-Box scale/transform always run Pass 3 when `!(is_ctrl && is_alt)`, even when `Alt` is held to center the BBox pivot!)*
   For each unselected `OffCurve` node $h$ ($h \notin \text{selected\_nodes}$) attached to an adjacent `OnCurve` node $a$:
   - If $a \notin \text{selected\_nodes}$: $h$ remains at $h_{\text{shadow}}.\text{point}$.
   - If $a \in \text{selected\_nodes}$:
     - Let $\Delta_a = a.\text{point} - a_{\text{shadow}}.\text{point}$. First translate $h$ rigidly with its parent anchor:
       $$h.\text{point} = h_{\text{shadow}}.\text{point} + \Delta_a$$
       *(This preserves the exact relative handle vector $h - a = h_{\text{shadow}} - a_{\text{shadow}}$ in both unrounded and grid-rounded coordinates!)*
     - **Smooth Collinearity Adjustment:** If $a.\text{node\_type} == \text{NodeType::Smooth}$ and the opposite neighbor $b$ across $a$ **was** transformed ($b \in \text{selected\_nodes}$), rotate $h$ around $a.\text{point}$ so that $h.\text{point} - a.\text{point}$ stays collinear (opposite in direction) to $b.\text{point} - a.\text{point}$ while preserving its original length $\|h_{\text{shadow}}.\text{point} - a_{\text{shadow}}.\text{point}\|$!
4. **Grid-Rounding (`position` vs. `position_precise` when `grid_step_upm > 0.0`):**
   - Nodes store unrounded `f64` coordinates in `position_precise`, while effective coordinates round each axis to `grid_length = grid_main / grid_subdivisions` (`round(val / grid_length) * grid_length`) whenever `grid_length > 0.0`.
   - Because all interactive transforms evaluate $T(n_{\text{shadow}}.\text{point})$ from the frozen `shadow_contours` snapshot captured at `PointerPress`, cumulative rounding drift never occurs during a drag; when `round_to_grid` (`grid_step_upm > 0.0`) is enabled, rendered/effective node coordinates snap to the nearest multiple of `grid_step_upm` (`(val / grid_step_upm).round() * grid_step_upm`), while unselected adjacent `OffCurve` handles shifted in Pass 3 preserve their integer relative handle vector $h - a$.

#### 2.1.6 Normative 5-Tier Hit-Test Priority Order
On `PointerPress` / `PointerMove` in `SelectTool` (`V`) and `CanvasTool::Lasso` (`Shift+V`), hit-testing evaluates candidates in this exact priority order:
1. **Tier 1 — Bounding-Box Knobs:**
   - Checked **first** when the bounding-box display gate (§2.1.1) is satisfied.
   - **Inner Corner Fallthrough ($t < 0.7$):** Because knob hit-testing returns `None` whenever $\text{hypot}(Q - P_{\text{box}}(k)) < 0.5 \cdot \text{handle\_size\_dp}$ or $t < 0.7$, clicking directly on a contour `OnCurve` or `OffCurve` node sitting on the bounding-box corner/edge ($t \approx 0.0$) **misses** Tier 1 and falls through cleanly to Tier 2 / Tier 3!
2. **Tier 2 — `OffCurve` BCP Handles (Pass 1):**
   - Checks all visible `NodeType::OffCurve` handles within node hit radius (`selected` handles win tie-breaks at equal distance).
3. **Tier 3 — `OnCurve` Anchor Nodes (Pass 2):**
   - Checks all `NodeType::Corner` and `NodeType::Smooth` nodes within node hit radius.
4. **Tier 4 — Path Segments (Cubic Curves & Straight Lines):**
   - Checked **if and only if** `allows_path_segment_interaction == true`:
     - **`SelectTool` (`V`):** `allows_path_segment_interaction == true` (clicking/dragging a curve or line segment selects/drags the segment, or `Alt`-clicks a line to convert to cubic).
     - **`CanvasTool::Lasso` (`Shift+V`):** `allows_path_segment_interaction == false`! Clicking and dragging on top of a path segment in `CanvasTool::Lasso` skips Tier 4 and starts a freeform Lasso selection!
5. **Tier 5 — Empty Canvas:**
   - Starts rectangular/slanted Marquee selection in `SelectTool` (`V`) or freeform polygon Lasso selection in `CanvasTool::Lasso` (`Shift+V`).

#### 2.1.7 Slanted & Dynamic Marquee Selection + `Space`-Panning Marquee
While dragging a Marquee selection in `SelectTool` (`V`) from `marquee_start` $(x_1, y_1)$ to `marquee_current` $(x_2, y_2)$:
1. **Holding `Space` during an active Marquee drag:**
   - Instead of activating `HandTool` canvas pan, holding `Space` while `SelectState::Marquee` is active translates `marquee_start` by the cursor delta $\Delta = P_{\text{curr}} - P_{\text{prev}}$ (`marquee_start = marquee_start + delta`), panning the marquee box itself across the canvas!
2. **Holding `Alt`/`Option` (without `Ctrl`) during a Marquee drag:**
   - If `selection_angle_deg == 0.0` and `italic_angle_deg != 0.0`, snaps `selection_angle_deg = italic_angle_deg` and sets `show_bounding_box_slanted = true` (and if `Alt` remains held on `PointerRelease`, filters selection to `OnCurve` nodes only).
3. **Holding `Ctrl` during a Marquee drag:**
   - Dynamically steers `selection_angle_deg` from the cursor position relative to `marquee_start`, clamped to $[-60^\circ, +60^\circ]$ ($k = \text{clamp}(\tan(\theta \cdot \pi/180), -\sqrt{3}, +\sqrt{3})$ where $\sqrt{3} \approx 1.7320508$).
   - If `!is_alt` and $|\theta - \text{italic\_angle\_deg}| < 2.0^\circ$, snaps `selection_angle_deg = italic_angle_deg`.
   - If `is_shift`, rounds `selection_angle_deg` to $1.0^\circ$ increments.
   - Sets `show_bounding_box_slanted = (selection_angle_deg != 0.0)`.
4. **Slanted Marquee Parallelogram Geometry:**
   - Let $k = \text{clamp}(\tan(\text{selection\_angle\_deg} \cdot \pi/180), -1.7320508, +1.7320508)$ and $\Delta y = y_2 - y_1$.
   - The 4 vertices of the marquee parallelogram (used for both canvas rendering and point-in-parallelogram selection testing) are:
     $$V_0 = (x_1, y_1),\quad V_1 = (x_2 - \Delta y \cdot k,\; y_1),\quad V_2 = (x_2, y_2),\quad V_3 = (x_1 + \Delta y \cdot k,\; y_2)$$

---

### 2.2 Dedicated `RotateTool` (`R`) & `ScaleTool` (`S`)

1. **Tool Activation, Shared `transform_origin` & Automatic Origin Update on `Cmd`-Select:**
   - Pressing **`R`** activates `CanvasTool::Rotate`; pressing **`S`** activates `CanvasTool::Scale`.
   - When entering `Rotate` or `Scale` from a non-transform tool (`Select`, `Pen`, `Hand`, `Lasso`, `Zoom`), `state.transform_origin` is initialized via `update_transform_origin()` to `aligned_point_for_bounds(state.transform_grid_anchor, selection_bounds)` (default `transform_grid_anchor = 4`, center of `selection_bounds`) if `selected_nodes` is non-empty, or `FontPoint::new(0.0, 0.0)` if `selected_nodes` is empty.
   - Switching directly between `R` and `S` **preserves** a manually placed `state.transform_origin`.
   - **Temporary `Cmd`-Select Override & Origin Recomputation:**
     - Holding `Cmd` (or the temporary-Select modifier) while `RotateTool` (`R`), `ScaleTool` (`S`), `CanvasTool::Lasso` (`Shift+V`), or `ZoomTool` (`Z`) is active temporarily delegates pointer events to `SelectTool` (`V`) so the user can click or marquee-select a different set of nodes without leaving `R` or `S`.
     - **Whenever `selected_nodes` changes while `R` or `S` is active (via temporary `Cmd`-Select), `update_transform_origin()` automatically recomputes `state.transform_origin` to the `transform_grid_anchor` (`4` = center) of the new `selection_bounds`!**
2. **On-Canvas `transform_origin` Reticle & Active Drag Line:**
   - Rendered whenever `active_tool` is `CanvasTool::Rotate` or `CanvasTool::Scale`:
     - **Background Circle:** centered at $P_{\text{scr}} = \text{viewport.to\_screen}(\text{transform\_origin})$, radius **`8.0 dp`** (diameter `16.0 dp`), fill `rgba(0, 0, 0, 51)` (`alpha = 0.20`), stroke width `1.2 dp` in `rgba(128, 128, 128, 255)`.
     - **Foreground Crosshair Ticks:** 4 line segments in `rgba(85, 85, 85, 255)`, width `1.0 dp`:
       - Left: $(P_x - 2.0, P_y) \to (P_x - 5.0, P_y)$
       - Right: $(P_x + 2.0, P_y) \to (P_x + 5.0, P_y)$
       - Top: $(P_x, P_y - 2.0) \to (P_x, P_y - 5.0)$
       - Bottom: $(P_x, P_y + 2.0) \to (P_x, P_y + 5.0)$
     - **Center Square:** filled `3.0 dp × 3.0 dp` rect centered at $P_{\text{scr}}$.
     - **Active Drag Ray Line:** while actively rotating or scaling (`DraggingTransform`), draw a `1.2 dp` solid gray line from $P_{\text{scr}}$ (`transform_origin`) to the current cursor screen position.
3. **Pointer Interaction State Machine:**
   - **Never Duplicates on `Alt`/`Option`:** Holding `Alt`/`Option` during `RotateTool` or `ScaleTool` drag **never** duplicates contours.
   - **`Ctrl` Snap Bypass:** Holding `Ctrl` (`is_ctrl`) disables grid/point snapping of the cursor point during `RotateTool` and `ScaleTool` drags.
   - **Case A — Dragging `transform_origin` directly:** If `PointerPress` occurs within **`3.0 dp`** (`TRANSFORM_ORIGIN_HIT_RADIUS_DP = 3.0`) of `transform_origin`, dragging moves `transform_origin` live (rounded to `grid_step_upm` unless `is_ctrl` is held) without modifying contours or pushing an undo entry.
   - **Case B — Click without drag (`<= 2.0 dp` screen movement before `PointerRelease`):** On `PointerRelease`, if total screen movement from `drag_start_scr` is **`<= 2.0 dp`** (`TRANSFORM_CLICK_THRESHOLD_DP = 2.0`), `state.transform_origin` is set to `drag_start_font` (rounded to `grid_step_upm` unless `is_ctrl` is held) without modifying contours or pushing an undo entry!
   - **Case C — Click + Drag (`> 2.0 dp` screen movement):** Keeps `state.transform_origin` fixed and transforms `selected_nodes` live using the 3-pass pipeline (§2.1.5), recording a single undo entry (`"Rotate Selection"` or `"Scale Selection"`) on `PointerRelease`.
4. **`RotateTool` (`R`) Transformation Formula:**
   - Let $O = \text{transform\_origin}$, $A = \text{drag\_start\_font}$, $B = \text{curr\_font}$.
   - **Minimum Pivot Distance Guard:** If $\text{hypot}(A - O) \le 5.0\text{ UPM}$, no rotation is applied.
   - $\Delta\theta_{\text{deg}} = (\text{atan2}(B_y - O_y, B_x - O_x) - \text{atan2}(A_y - O_y, A_x - O_x)) \cdot \frac{180}{\pi}$.
   - If **`is_shift`**: $\Delta\theta_{\text{deg}} = \text{round}(\Delta\theta_{\text{deg}} / 45.0) \cdot 45.0$.
   - Rotates around $O$ by $\Delta\theta_{\text{deg}}$.
5. **`ScaleTool` (`S`) Transformation Formula:**
   - Let $O = \text{transform\_origin}$, $A = \text{drag\_start\_font}$, $B = \text{curr\_font}$.
   - **Minimum Pivot Axis-Distance Guard:** Requires $|A_x - O_x| > 5.0\text{ UPM} \;\land\; |A_y - O_y| > 5.0\text{ UPM}$ (if either axis distance is $\le 5.0\text{ UPM}$, no scaling is applied).
   - Base scale factors:
     $$s_x = \frac{B_x - O_x}{A_x - O_x}, \qquad s_y = \frac{B_y - O_y}{A_y - O_y}$$
   - **With `Shift` (3-Zone Drag-Angle Constrain):**
     Let $\theta = \text{atan2}(B_y - A_y, B_x - A_x)$ and $u = |\sin(\theta)|$:
     1. **$u < 0.2$** (drag within $\pm 11.54^\circ$ of horizontal): **Horizontal 1-Axis Scale** ($s_y = 1.0$).
     2. **$u > 0.86$** (drag within $\pm 30.68^\circ$ of vertical, $> 59.32^\circ$ from horizontal): **Vertical 1-Axis Scale** ($s_x = 1.0$).
     3. **$0.2 \le u \le 0.86$** (diagonal drag between $11.54^\circ$ and $59.32^\circ$): **Uniform Proportional Scale**:
        $$m = \begin{cases} \min(|s_x|, |s_y|) & \text{if } s_x \le 1.0 \land s_y \le 1.0 \\ \max(|s_x|, |s_y|) & \text{otherwise} \end{cases}, \qquad s_x = \text{sign}(s_x) \cdot m, \qquad s_y = \text{sign}(s_y) \cdot m$$
   - **Minimum Scale Guard:** Requires $|s_x| > 0.01 \;\land\; |s_y| > 0.01$ (`SCALE_MIN_FACTOR = 0.01`); if $|s_x| \le 0.01$ or $|s_y| \le 0.01$, clamp magnitude to `0.01` so the selection never collapses to irreversible zero.

---

### 2.3 `CanvasTool::Lasso` (`Shift+V`)

1. **Tool Switching & Segment-Drag Suppression (`allows_path_segment_interaction == false`):**
   - Pressing **`Shift+V`** activates `CanvasTool::Lasso` (and pressing **`V`** without `Shift` activates `CanvasTool::Select`).
   - Direct clicking/dragging on a bounding-box knob (Tier 1), `OffCurve` handle (Tier 2), or `OnCurve` node (Tier 3) in `CanvasTool::Lasso` inherits `SelectTool` behavior.
   - **Crucial Difference from `SelectTool`:** `allows_path_segment_interaction` is **`false`** in `CanvasTool::Lasso`! Clicking and dragging on a path segment (Tier 4) or on empty canvas (Tier 5) starts a freeform Lasso selection (`LassoState::Dragging { points: Vec<FontPoint>, modifiers }`) instead of dragging the curve segment!
2. **Polyline Sampling:**
   - On `PointerPress` when no knob or node was grabbed: starts `lasso_points = vec![font_pt]`. Unless `is_shift` or `is_alt` is held, clears `selected_nodes`.
   - On `PointerMove`: appends `font_pt` to `lasso_points` if and only if screen distance from the last appended point is **`> 2.0 dp`** (`LASSO_MIN_SAMPLE_STEP_DP = 2.0`), and updates live point-in-polygon selection.
   - Updates `show_bounding_box_slanted = is_ctrl`.
3. **Visual Rendering:**
   - While `lasso_points.len() >= 2`:
     - **Fill:** fills the implicitly closed polygon `lasso_points` (using even-odd / winding triangle fan or ear-clipping) with accent selection color at `alpha = 0.33` (`rgba(0, 122, 255, 84)`).
     - **Stroke:** draws a **solid** `1.5 dp` polyline (`rgba(0, 122, 255, 255)`) along `lasso_points` (plus closing segment from last to first point).
4. **Point-in-Polygon Selection:**
   - Uses even-odd ray-casting point-in-polygon test `point_in_polygon(node.point, &lasso_points)` live during drag and on `PointerRelease` (when `lasso_points.len() >= 3` and start-to-end or max excursion $> 2.0\text{ dp}$):
     - **No Modifiers (`!is_shift && !is_alt`):** `selected_nodes` becomes all `OnCurve` and `OffCurve` nodes inside the polygon.
     - **`Alt`/`Option` (`!is_shift && is_alt`):** `selected_nodes` becomes all **`OnCurve` nodes only** inside the polygon (`OffCurve` handles are ignored).
     - **`Shift` (`is_shift && !is_alt`):** **XOR-toggles** every `OnCurve` and `OffCurve` node inside the polygon against `initial_selected_nodes` captured at `PointerPress`.
     - **`Shift + Alt` (`is_shift && is_alt`):** **XOR-toggles** only **`OnCurve` nodes** inside the polygon against `initial_selected_nodes`.

---

### 2.4 `ZoomTool` (`Z`, `Cmd/Ctrl+Space`, `Cmd/Ctrl+Alt+Space`)

1. **Activation & Temporary Override:**
   - Pressing **`Z`** activates `CanvasTool::Zoom`.
   - Holding **`Cmd+Space` / `Ctrl+Space`** (or **`Cmd+Alt+Space` / `Ctrl+Alt+Space`**) temporarily overrides any active tool with `ZoomTool` (taking priority over `Space` Hand-pan when `Cmd`/`Ctrl` is also held).
   - Cursor: `ZoomIn` magnifier (`+`, hotspot `(7.0, 7.0)`) when `!is_alt`; `ZoomOut` magnifier (`-`, hotspot `(7.0, 7.0)`) when `is_alt`.
2. **Zoom Marquee Visual:**
   - While dragging in `ZoomTool`: draws axis-aligned screen rectangle from `drag_start_scr` to `curr_scr` with fill `rgba(128, 128, 128, 84)` (`alpha: 0.33`) and solid `1.0 dp` stroke `rgba(128, 128, 128, 255)`.
3. **Click vs. Marquee Threshold (`10.0 dp`):**
   On `PointerRelease`, let $w_{\text{scr}} = |x_{\text{curr}} - x_{\text{start}}|$ and $h_{\text{scr}} = |y_{\text{curr}} - y_{\text{start}}|$:
   - **Case 1 — Marquee Zoom ($w_{\text{scr}} + h_{\text{scr}} \ge 10.0\text{ dp}$ with $w_{\text{scr}} > 1.0 \land h_{\text{scr}} > 1.0$):**
     Let $C_{\text{font}}$ be the font-coordinate center of the marquee rectangle and $(V_w, V_h)$ be the canvas viewport dimensions in `dp`:
     - **Without `Alt` (Zoom In to Rect):**
       $$\text{new\_zoom} = \text{clamp}\!\left(\text{zoom} \cdot \min\!\left(\frac{V_w}{w_{\text{scr}}},\; \frac{V_h}{h_{\text{scr}}}\right),\; 0.008,\; 40.0\right)$$
       Centers $C_{\text{font}}$ at the center of the viewport!
     - **With `Alt` (Zoom Out to Fit Rect):**
       $$\text{new\_zoom} = \text{clamp}\!\left(\text{zoom} \cdot \min\!\left(\frac{w_{\text{scr}}}{V_w},\; \frac{h_{\text{scr}}}{V_h}\right),\; 0.008,\; 40.0\right)$$
       Centers $C_{\text{font}}$ at the center of the viewport!
   - **Case 2 — Discrete Step Click Zoom ($w_{\text{scr}} + h_{\text{scr}} < 10.0\text{ dp}$):**
     Uses the 23 discrete zoom steps:
     $$\text{ZOOM\_STEPS} = [0.008, 0.01, 0.012, 0.018, 0.024, 0.034, 0.05, 0.075, 0.1, 0.16, 0.24, 0.34, 0.5, 0.66, 1.0, 1.5, 2.0, 3.0, 4.0, 6.0, 10.0, 20.0, 40.0]$$
     - **Without `Alt` (`next_zoom_step`):** picks the first step $s \in \text{ZOOM\_STEPS}$ with $s > \text{zoom} \cdot 1.08$ (or `40.0`), keeping `drag_start_scr` anchored at the same font coordinate (`zoom_at_cursor`).
     - **With `Alt` (`previous_zoom_step`):** picks the last step $s \in \text{ZOOM\_STEPS}$ with $s < \text{zoom} \cdot 0.92$ (or `0.008`), keeping `drag_start_scr` anchored at the same font coordinate.

---

### 2.5 Canvas Outline Clipboard (`Cmd/Ctrl+C`, `Cmd/Ctrl+X`, `Cmd/Ctrl+V`)

1. **`copy_selection` (`Cmd/Ctrl+C`):**
   - **Empty-Selection Guard:** If `selected_nodes` is empty, `copy_selection` is a complete **no-op** (existing `clipboard_contours` is left untouched).
   - Iterates over each contour $C_i$ (with closed flag `closed_i`):
     - Count total `OnCurve` nodes $N_{\text{on}}$ and selected `OnCurve` nodes $N_{\text{sel\_on}}$ in $C_i$.
     - **Rule 1 (`N_sel_on == 0`):** Even if `OffCurve` handles on $C_i$ are selected, **nothing** is extracted from $C_i$ (`0` contours). If no contour has any selected `OnCurve` node, `copy_selection` leaves `clipboard_contours` unchanged and does not mutate state.
     - **Rule 2 (`N_sel_on == N_on` and `N_on > 0`):** Extracts a full clone of $C_i$ preserving `closed = closed_i` and all `OffCurve` handles of $C_i$.
     - **Rule 3 (`0 < N_sel_on < N_on` — Partial Contour Extraction):**
       - If `closed_i == true`, rotate traversal start index `start_idx` to the first **unselected `OnCurve` node** so that a selected chain crossing the `P0` seam is extracted as a **single contiguous open contour**!
       - Walk nodes in order:
         - While `selected_nodes.contains(node.id)` (or for an `OffCurve` node: either the `OffCurve` node itself is in `selected_nodes` OR both of its bounding `OnCurve` segment endpoints are in `selected_nodes`):
           - If no `sub_path` is open and `node.node_type != NodeType::OffCurve`: start a new open contour (`closed = false`), clone `node` with `node_type = NodeType::Corner`, and push to `sub_path`.
           - Else if `sub_path` is open: push a clone of `node` to `sub_path`.
         - When a node is not selected (breaking the chain):
           - Pop any trailing `NodeType::OffCurve` nodes from the end of `sub_path`, ensure the last remaining `OnCurve` node in `sub_path` has `NodeType::Corner` if it has no outgoing `OffCurve`, finalize `sub_path` (`closed = false`), and reset `sub_path = None`.
2. **`paste_clipboard` (`Cmd/Ctrl+V`):**
   - If `state.clipboard_contours` is empty, does nothing.
   - **Mode B — Open-Contour Splice:**
     - Triggered when `state.clipboard_contours.len() == 1`, `!state.clipboard_contours[0].closed`, and `state.selected_nodes` contains **$\ge 2$ `OnCurve` nodes forming a single contiguous selected span on one target contour**:
     - Replaces the interior of the selected contiguous span on the target contour with the nodes of `clipboard_contours[0]` (matching the orientation whose endpoints are closer to the span endpoints), selects the newly spliced nodes, records `"Paste"`, and asserts invariants!
   - **Mode A — Standard Contour Paste:**
     - Used in all other cases (`selected_nodes.len() < 2`, multiple contours on clipboard, closed contour on clipboard, or non-contiguous selection).
     - Allocates fresh `NodeId`s for every node in `state.clipboard_contours`.
     - **Offset Rule:** Pastes at **`(0.0, 0.0)`** offset (exact stored coordinates), **except** if an identical contour with the exact same node coordinates (`< 0.001 UPM` per node) already exists in `state.contours`, in which case it offsets the pasted contour by **`(+10.0, +10.0)` UPM** (collision offset) so in-place duplicate paste does not create invisible zero-distance node collisions; if `paste_at_exact_origin` is requested or no identical contour is present, offset is `(0.0, 0.0)`.
     - Appends the pasted contours (with their `closed` flags) to `state.contours` / `state.glyph.contours`, replaces `state.selected_nodes` with **exclusively the newly pasted node IDs**, records `"Paste"`, and asserts invariants.
3. **`cut_selection` (`Cmd/Ctrl+X`):**
   - If `selected_nodes` is empty (or contains no `OnCurve` nodes), does nothing (leaving `clipboard_contours` untouched).
   - First runs `copy_selection` (`Cmd/Ctrl+C`).
   - Then executes **`cut_open_delete` (`Shift + Delete`)** on `selected_nodes` (breaking closed contours open rather than curve-healing!), records `"Cut"`, and asserts invariants.

---

### 2.6 The 7 Topological & Geometric Path Operations + Visual Warnings

#### 2.6.1 `reverse_selected_contours`
- **Target Contours:** If `selected_nodes` is non-empty, reverses every contour containing $\ge 1$ node (`OnCurve` or `OffCurve`) in `selected_nodes`; if `selected_nodes` is empty, reverses **all** contours in the glyph.
- **Closed Contour (`closed == true`):** Reverses traversal direction while keeping the **exact same physical `OnCurve` node at index `0` (`P0`)**: `[P0, rest...] -> [P0, rest.reversed()]`.
- **Open Contour (`closed == false`):** Reverses the node vector `[P0, ..., P_last] -> [P_last, ..., P0]` so `P0` and `P_last` swap, and ensures the new `P0` is `NodeType::Corner` if it has no incoming segment.

#### 2.6.2 `make_node_first`
- Operates on the first selected `OnCurve` node `sel_id` (`OffCurve` nodes are ignored):
   - **On a Closed Contour (`closed == true`):** Cyclically rotates `contour` (`contour.rotate_left(pos)`) so `sel_id` becomes index `0` (`P0`).
   - **On an Open Contour (`closed == false`):**
     - If `sel_id` is the **last node (`P_last`)** of the open contour: reverses the open contour (`contour.reverse()`).
     - If `sel_id` is already `P0` or an **interior `OnCurve` node** of an open contour: **no-op** (returns `false` without mutating state or undo history).

#### 2.6.3 `correct_path_direction` (`Cmd/Ctrl+Shift+R`)
1. **Whole-Layer Scope & Open Contours Ignored:** Always operates on the **entire glyph layer** regardless of `selected_nodes`. Contours with `closed == false` are left completely untouched (neither their direction nor their `P0` changes).
2. **Closed Contours Winding Parity (Even-Odd Nesting Depth with Italic Unshearing):**
   - When `italic_angle_deg != 0.0`, closed paths are first evaluated in italic-unsheared space (`shear_x = -tan(italic_angle_deg * pi / 180.0)`) for bounding-box sorting.
   - For each closed contour $C_i$, compute its flattened polygon / bounding box and signed area ($A < 0$ is CW in Y-up font coordinates, $A > 0$ is CCW in Y-up font coordinates).
   - Count nesting depth $k_i = \#\{C_j \text{ closed} \mid j \ne i,\; \text{area}(\text{bbox}(C_j)) > \text{area}(\text{bbox}(C_i)),\; C_j \text{ encloses } C_i\}$.
   - Target direction:
     - **Even depth ($k_i \bmod 2 == 0$, outer contour / island inside counter):** **Counter-Clockwise (`CCW`, positive signed area in Y-up font space)**.
     - **Odd depth ($k_i \bmod 2 == 1$, inner counter):** **Clockwise (`CW`, negative signed area in Y-up font space)**.
   - If $C_i$'s current winding does not match target direction, reverse $C_i$.
3. **Start Node (`P0`) Selection (`find_min_node`):**
   - For each closed contour $C_i$, find the `OnCurve` node minimizing $(y, x)$ with `0.001 UPM` tolerance on $y$:
     - Node $A$ beats current best $B$ if $A_y - B_y < -0.001$, OR ($|A_y - B_y| \le 0.001$ AND $A_x < B_x$).
   - Rotate $C_i$ cyclically so this minimum `OnCurve` node becomes `P0` (index `0`).

#### 2.6.4 `add_extremes(force: bool)` & `add_inflection_points`
1. **`add_extremes(force: bool)` (`force = false` normal *Add Extremes*, `force = true` *Force Extremes* with `Alt`/`Option`):**
   - **Selection Gate:** If `selected_nodes` is non-empty, only cubic segments `[P0, P1, P2, P3]` where **at least one of `{P0, P1, P2, P3}` is in `selected_nodes`** are processed. If `selected_nodes` is empty, **all** cubic segments in all contours are processed.
   - **Root Finding & `0.8 UPM` Distance Filter:**
     - Solve $B'_x(t) = 0$ and $B'_y(t) = 0$ for $t \in (0.001, 0.999)$.
     - Sort distinct roots ascending (deduplicating roots within $\Delta t < 10^{-4}$).
     - Split backwards from largest $t$ to smallest $t$: at each root $t$, evaluate $M = B(t)$, $d_0 = \text{hypot}(M - P_0)$, and $d_3 = \text{hypot}(M - P_3)$.
     - For **both** `force == false` and `force == true`: if $d_0 < 0.8\text{ UPM}$ or $d_3 < 0.8\text{ UPM}$ (`EXTREMUM_MIN_ENDPOINT_DIST_UPM = 0.8`), **skip** the root!
   - **Near-Endpoint Smooth Absorption when `force == false` ($0.8 \le d_0 < 30.0\text{ UPM}$ or $0.8 \le d_3 < 30.0\text{ UPM}$):**
     - If `force == false` and $d_0 < 30.0\text{ UPM}$ (or $d_3 < 30.0\text{ UPM}$), check whether the near endpoint ($P_0$ or $P_3$) is a `NodeType::Smooth` node adjacent to another cubic segment whose split tangent handle $R$ at $M$ is already nearly axis-aligned ($|R_x - M_x| < 0.1 \land |R_x - M_x| / |R_y - M_y|^{0.12} < 1.2$ for a vertical extremum, or symmetric in $x \leftrightarrow y$ for a horizontal extremum). If so, **absorb** the extremum into the existing endpoint (shifting $P_0$ or $P_3$ to $M$ and orthogonalizing its handle) instead of inserting an extra node!
   - **Standard De Casteljau Split & Handle Orthogonalization (`force == true`, or $d_0, d_3 \ge 30.0\text{ UPM}$, or non-absorbable endpoint):**
     - Split the cubic segment via De Casteljau into `[P0, Q0, R0, M(Smooth), R1, Q2, P3]` and **orthogonalize** the adjacent handles $(R_0, R_1)$:
       - If $|R_{1,x} - R_{0,x}| \ge |R_{1,y} - R_{0,y}|$ (horizontal extremum): set $R_{0,y} = M_y$ and $R_{1,y} = M_y$.
       - Else (vertical extremum): set $R_{0,x} = M_x$ and $R_{1,x} = M_x$.
   - **Whole-Layer Post-Tidy:** When `selected_nodes` was empty, `add_extremes` automatically runs `tidy_up_paths` after inserting extremes across the layer.
2. **`add_inflection_points`:**
   - **Selection Gate:** Same as `add_extremes` (selected cubic segments if `selected_nodes` non-empty, else all cubic segments).
   - **Flatness & Endpoint Guard:**
     - Compute perpendicular distances $h_1, h_2$ of $P_1, P_2$ to chord $P_0 P_3$. If $h_1 < 0.5\text{ UPM} \land h_2 < 0.5\text{ UPM}$, skip.
     - Solve $B'_x(t) B''_y(t) - B'_y(t) B''_x(t) = 0$ for $t \in (0.001, 0.999)$.
     - For each root $t$, evaluate $M = B(t)$. If $\text{hypot}(M - P_0) > 20.0\text{ UPM} \land \text{hypot}(M - P_3) > 20.0\text{ UPM}$ (`INFLECTION_MIN_ENDPOINT_DIST_UPM = 20.0`), split via De Casteljau inserting a `NodeType::Smooth` node at $M$.

#### 2.6.5 `tidy_up_paths` (`Cmd/Ctrl+Shift+T`) + Duplicate-Node Warnings
- **Selection Scoping:**
  - If `selected_nodes` is **non-empty**, `tidy_up_paths` operates only on contours/nodes in `selected_nodes`.
  - If `selected_nodes` is **empty**, `tidy_up_paths` operates on the **entire glyph layer** (all contours and nodes).
1. **Degenerate Mini-Contour Removal:**
   - Let `thresh = if grid_step_upm < 0.99 { 0.2 } else { 2.0 }`. Remove any target contour that has `0` `OnCurve` nodes, or `bbox.width < thresh && bbox.height < thresh`, or any closed contour with `< 2` `OnCurve` nodes.
2. **Duplicate Consecutive `OnCurve` Node Merge & Zero-Length Handle Retraction:**
   - **Zero-Length Cubic Handles:** On any target cubic segment `[P0, H1, H2, P3]`, if $\text{hypot}(H_1 - P_0) < 0.15\text{ UPM} \land \text{hypot}(P_3 - H_2) < 0.15\text{ UPM}$ (`TIDY_ZERO_HANDLE_EPS_UPM = 0.15`), remove `H1` and `H2` (converting the segment to a straight `Line`).
   - **Coincident Consecutive `OnCurve` Nodes:** Let `eps_on = if grid_step_upm < 0.99 { 0.01 } else { 0.6 }`. If two consecutive `OnCurve` nodes $A, B$ connected by a straight `Line` have $\text{hypot}(B - A) < \text{eps\_on}$, merge $B$ into $A$ (removing the zero-length segment).
3. **Collinear Interior `Line` Node Removal:**
   - For any 3 consecutive `OnCurve` nodes $(A, B, C)$ where both $A \to B$ and $B \to C$ are straight `Line` segments and $B$ propagates forward ($\text{dot}(B - A, C - B) > 0$): if $|A_x - B_x| < 0.001 \land |B_x - C_x| < 0.001$, OR $|A_y - B_y| < 0.001 \land |B_y - C_y| < 0.001$, OR $|\frac{B_y - A_y}{B_x - A_x} - \frac{C_y - B_y}{C_x - B_x}| < 0.01$, remove interior node $B$.
4. **Open-Contour Auto-Close:**
   - If a target contour is open (`closed == false`), has $\ge 3$ `OnCurve` nodes, and $\text{hypot}(P_{\text{last}} - P_0) < 1.0\text{ UPM}$, merge $P_{\text{last}}$ into $P_0$ and set `closed = true`.
5. **Automatic `Corner` $\leftrightarrow$ `Smooth` Connection Check (`are_collinear_lines`):**
   - For each target `OnCurve` node $B$ (interior on open contours, all `OnCurve` nodes on closed contours) with immediate neighbor points $A = \text{prev\_node}.\text{point}$ and $C = \text{next\_node}.\text{point}$:
     - If **neither** neighbor is `NodeType::OffCurve` (both adjacent segments are straight `Line`s): $B.\text{node\_type}$ MUST be `NodeType::Corner`.
     - If **at least one** neighbor is `NodeType::OffCurve`: let $L_1 = \text{hypot}(B - A)$, $L_2 = \text{hypot}(C - B)$, $L_{AC} = \text{hypot}(C - A)$, and $d_\perp$ be the perpendicular distance from $B$ to the line through $A$ and $C$.
     - `are_collinear_lines(A, B, C)` is `true` iff:
       $$L_1 > 10^{-6} \;\land\; L_2 > 10^{-6} \;\land\; L_1 < L_{AC} \;\land\; L_2 < L_{AC} \;\land\; d_\perp < 0.25 \cdot \sqrt{\min(L_1, L_2)}$$
     - If `are_collinear_lines(A, B, C)` is `true`: set $B.\text{node\_type} = \text{NodeType::Smooth}$ and snap the `OffCurve` handle(s) collinear with the opposite tangent vector.
     - If `are_collinear_lines(A, B, C)` is `false`: set $B.\text{node\_type} = \text{NodeType::Corner}$.
6. **Edit View Visual Warning Circles:**
   - **Red Coincident `OnCurve` Circle:** Whenever two consecutive `OnCurve` nodes on the same contour (including `(P_last, P0)` on a closed contour) have $\text{hypot}(P_{\text{curr}} - P_{\text{prev}}) < 1.0\text{ UPM}$ and neither is currently being dragged, Pass 4 renders a filled red circle (`rgba(229, 57, 53, 153)` / `alpha = 0.60`, radius `node_size_dp * 0.5 + 2.0 dp` = `5.5 dp`) behind the node!
   - **Yellow Coincident `OffCurve` Circle:** Whenever the two `OffCurve` BCP handles `(H1, H2)` of the same cubic segment have $\text{hypot}(H_2 - H_1) < 0.1\text{ UPM}$, Pass 4 renders a filled yellow circle (`rgba(253, 216, 53, 153)` / `alpha = 0.60`, radius `bcp_radius_dp + 1.5 dp` = `4.25 dp`)!

#### 2.6.6 `align_selection` (`Cmd/Ctrl+Shift+A`)
Requires `selected_nodes.len() >= 2` (if `< 2`, no-op).
1. **Branch A — Special 2-Node `1 OnCurve` + `1 Adjacent OffCurve` Selection (`selected_nodes.len() == 2`):**
   - When `selected_nodes` consists of 1 `OnCurve` node $a$ and 1 adjacent `OffCurve` handle $h$ on the same contour:
     - The `OnCurve` node $a$ **never moves** (`a.point` stays fixed).
     - Let $\Delta x = |h.x - a.x|$ and $\Delta y = |h.y - a.y|$:
       - If $\Delta x \le \Delta y$: snaps $h.x = a.x$ (preserving $h.y$).
       - If $\Delta x > \Delta y$: snaps $h.y = a.y$ (preserving $h.x$).
     - If $a$ is `NodeType::Smooth` and its opposite handle $h_{\text{opp}}$ is **not** selected, rotates $h_{\text{opp}}$ around $a$ (`preserve_length`) so $h_{\text{opp}} - a$ remains collinear with $a - h$!
2. **Branch B — Multi-Node Alignment to `aligned_point_for_bounds(transform_grid_anchor, selection_bounds)`:**
   - Compute the bounding box `bounds = [minX, maxX] × [minY, maxY]` of all selected nodes ($\Delta X = \max X - \min X$, $\Delta Y = \max Y - \min Y$).
   - Evaluate `aligned_pt = aligned_point_for_bounds(state.transform_grid_anchor, bounds)` (where `state.transform_grid_anchor` $\in \{0..8\}$ from the 9-point Transform Pin, rounded to `grid_step_upm` if `grid_step_upm > 0.0`):
     - `transform_grid_anchor` $\in \{0, 3, 6\}$ (Left column): `aligned_pt.x = minX`
     - `transform_grid_anchor` $\in \{1, 4, 7\}$ (Center column): `aligned_pt.x = (minX + maxX) * 0.5`
     - `transform_grid_anchor` $\in \{2, 5, 8\}$ (Right column): `aligned_pt.x = maxX`
     - `transform_grid_anchor` $\in \{0, 1, 2\}$ (Bottom row): `aligned_pt.y = minY`
     - `transform_grid_anchor` $\in \{3, 4, 5\}$ (Middle row): `aligned_pt.y = (minY + maxY) * 0.5`
     - `transform_grid_anchor` $\in \{6, 7, 8\}$ (Top row): `aligned_pt.y = maxY`
   - **Axis Selection:**
     - **If $\Delta X > \Delta Y$ (selection is wider than tall $\implies$ align Y to `aligned_pt.y`):**
       - Every selected node $n \in \text{selected\_nodes}$ gets **$n.y = \text{aligned\_pt}.y$** (preserving $n.x$).
       - For every aligned `OnCurve` node $a$, any adjacent **unselected** `OffCurve` handle $h$ shifts rigidly in $Y$ by $\Delta y = \text{aligned\_pt}.y - a_{\text{old}}.y$ ($h.y \mathrel{+}= \Delta y$), OR if $a$ is `NodeType::Smooth` whose opposite handle was in `selected_nodes`, rotates $h$ to maintain smooth collinearity.
     - **If $\Delta X \le \Delta Y$ (selection is taller than wide or square $\implies$ align X to `aligned_pt.x`):**
       - Every selected node $n \in \text{selected\_nodes}$ gets **$n.x = \text{aligned\_pt}.x$** (preserving $n.y$).
       - For every aligned `OnCurve` node $a$, any adjacent **unselected** `OffCurve` handle $h$ shifts rigidly in $X$ by $\Delta x = \text{aligned\_pt}.x - a_{\text{old}}.x$ ($h.x \mathrel{+}= \Delta x$), OR if $a$ is `NodeType::Smooth` whose opposite handle was in `selected_nodes`, rotates $h$ to maintain smooth collinearity.

#### 2.6.7 `connect_selected_open_endpoints` & `close_open_paths`
1. **Context Menu & Action Triggers:**
   - **`connect_selected_open_endpoints` (`"Connect Nodes"`):**
     - Requires an even number ($\ge 2$, typically $2$) of selected `OnCurve` nodes that are open contour endpoints (`P0` or `P_last` of a contour with `closed == false`).
   - **`close_open_paths` (`"Close Open Paths"`):**
     - For every open contour (`closed == false`) that has $\ge 1$ selected node (or all open contours when `selected_nodes` is empty), sets `closed = true`.
2. **Exact Behavior of `connect_selected_open_endpoints(node_a, node_b)` Across All 4 Cases:**
   Let $d = \text{hypot}(\text{node\_b}.\text{point} - \text{node\_a}.\text{point})$ and compare against **`1.0 UPM`** (`CONNECT_MERGE_DIST_UPM = 1.0`):
   - **Case A — Both endpoints belong to the SAME open contour (`contour_a == contour_b`), $d < 1.0\text{ UPM}$:**
     - Sets `closed = true` and ensures both endpoints are `NodeType::Corner` (if dragged onto each other or tidied via `tidy_up_paths`, merges `P_last` into `P0`).
   - **Case B — Both endpoints belong to the SAME open contour (`contour_a == contour_b`), $d \ge 1.0\text{ UPM}$:**
     - Sets `closed = true` and sets both endpoints to `NodeType::Corner`, creating a straight closing `Line` segment from `P_last` to `P0`.
   - **Case C — Endpoints belong to TWO DIFFERENT open contours (`contour_a != contour_b`), $d < 1.0\text{ UPM}$:**
     - Orients `contour_a` so `node_a` is its `P_last` and `contour_b` so `node_b` is its `P0` (reversing `contour_a` and/or `contour_b` as needed).
     - **Eliminates duplicate endpoint `node_b`** (merging `node_b` into `node_a` at `node_a.point`), appends the remaining nodes of `contour_b` to `contour_a`, removes `contour_b` from `state.contours`, and keeps the combined contour **open (`closed = false`)**!
   - **Case D — Endpoints belong to TWO DIFFERENT open contours (`contour_a != contour_b`), $d \ge 1.0\text{ UPM}$:**
     - Orients `contour_a` so `node_a` is its `P_last` and `contour_b` so `node_b` is its `P0` (reversing `contour_a` and/or `contour_b` as needed).
     - **Keeps BOTH endpoints `node_a` and `node_b`** as `NodeType::Corner`, joining them with a straight `Line` bridge `node_a -> node_b`, appends all nodes of `contour_b` to `contour_a`, removes `contour_b` from `state.contours`, and keeps the combined contour **open (`closed = false`)**!

---

### 2.7 Programmatic Transform Operations (`TransformPanel` Contract)

1. **Selection vs. Whole-Layer Rule:**
   - For all 5 programmatic transform operations (`flip_horizontal`, `flip_vertical`, `rotate_by_degrees`, `scale_by_factor`, `slant_by_degrees`):
     - If `selected_nodes` is **non-empty**: transforms only `selected_nodes` (plus unselected attached `OffCurve` handles via the 3-pass pipeline in §2.1.5).
     - If `selected_nodes` is **empty**: transforms **all contours in the glyph layer**!
2. **Pivot Calculation:**
   - **Mode A — `BoundingBoxOrigin` (9-Point Grid Pin `transform_grid_anchor` $\in \{0..8\}$, default `4` = center):**
     - Evaluates `aligned_point_for_bounds(state.transform_grid_anchor, ref_bounds)` where `ref_bounds` is `selection_bounds` when `selected_nodes` is non-empty, or the **whole-layer bounding box (`layer_fast_bounds`)** when `selected_nodes` is empty!
   - **Mode B — `MetricsOrigin` (`MetricsOriginMode` $\in \{0..4\}$):**
     - Base X is half the glyph layer advance width: $x_0 = 0.5 \cdot \text{advance\_width}$.
     - Base Y is selected by `MetricsOriginMode`:
       - `0` (`Baseline` / Half Width): $y = 0.0$
       - `1` (`Ascender`): $y = \text{ascender}$
       - `2` (`CapHeight`): $y = \text{cap\_height}$
       - `3` (`HalfXHeight`): $y = 0.5 \cdot x_{\text{height}}$
       - `4` (`XHeight`): $y = x_{\text{height}}$
     - **Italic Angle Adjustment:** When `italic_angle_deg != 0.0`, slants $x_0$ from the baseline:
       $$\text{pivot} = \left(0.5 \cdot \text{advance\_width} + y \cdot \tan\!\left(\text{italic\_angle\_deg} \cdot \frac{\pi}{180}\right),\; y\right)$$
3. **The 5 Programmatic Operations:**
   - **`flip_horizontal`:** Applies `scale(-1.0, 1.0)` around `pivot`. **Does NOT call `reverse`** on contours (winding order inverts geometrically, matching interactive negative scale).
   - **`flip_vertical`:** Applies `scale(1.0, -1.0)` around `pivot`. **Does NOT call `reverse`** on contours.
   - **`scale_by_factor(sx, sy)`:** Requires $|s_x| > 0.01 \land |s_y| > 0.01$; scales around `pivot` by $(s_x, s_y)$.
   - **`rotate_by_degrees(angle_deg)`:** Rotates around `pivot` by `angle_deg` ($\in [-360^\circ, +360^\circ]$).
   - **`slant_by_degrees(angle_deg, horizontal)`:** Requires $\text{angle\_deg} \in [-89^\circ, +89^\circ]$. When `horizontal == true`, shears $X$ around `pivot.y` by $\tan(\text{angle\_deg} \cdot \pi/180)$; when `horizontal == false`, shears $Y$ around `pivot.x` by $\tan(\text{angle\_deg} \cdot \pi/180)$.

---

## 3. Combinatorial State & Interaction Matrix

| # | Active Tool | Target / Selection State | Modifiers | Pointer / Key Action | Topology (`Open` / `Closed`) | Normative Post-Condition & Visual Feedback |
| :- | :--- | :--- | :--- | :--- | :--- | :--- |
| **M-01** | `Select` (`V`) | `< 2` selected nodes, or screen span $\le 4.0\text{ dp}$ | Any | Idle / Hover | Either | Bounding Box is NOT drawn and its knobs are NOT hit-tested. |
| **M-02** | `Select` (`V`) | $\ge 2$ selected nodes, $w \le 1.0\text{ UPM}$, $h \cdot \text{zoom} > 4.0\text{ dp}$ | None | Idle / Hover | Either | Dashed Bounding Box drawn with **only 2 active knobs `[1, 7]` (`S`, `N`)** at `1.5 * handle_size` outside box + live Width/Height capsule pills. |
| **M-03** | `Select` (`V`) | $\ge 2$ selected nodes, $h \le 1.0\text{ UPM}$, $w \cdot \text{zoom} > 4.0\text{ dp}$ | None | Idle / Hover | Either | Dashed Bounding Box drawn with **only 2 active knobs `[3, 5]` (`W`, `E`)** at `1.5 * handle_size` outside box + live Width/Height capsule pills. |
| **M-04** | `Select` (`V`) | $\ge 2$ selected nodes, $w > 1.0 \land h > 1.0$ | None | Drag on knob $k \in \{0..8\}\setminus\{4\}$ ($0.7 \le t \le 1.7$) | Either | Scales selection relative to **opposite knob $8 - k$**; unselected boundary `OffCurve` handles translate rigidly with parent `OnCurve` (or re-collinearize across `Smooth` node). |
| **M-05** | `Select` (`V`) | $\ge 2$ selected nodes | `Shift` | Drag on knob $k \in \{0..8\}\setminus\{4\}$ ($0.7 \le t \le 1.7$) | Either | Proportional 2D scale ($s_x = s_y = \max(s_x, s_y)$ on corners; $s_y = s_x$ on `W`/`E`; $s_x = s_y$ on `S`/`N`) around opposite knob $8 - k$. |
| **M-06** | `Select` (`V`) | $\ge 2$ selected nodes | `Alt` (`!Ctrl`) | Drag on knob $k$ ($0.7 \le t \le 1.7$) | Either | Scales around **bounding-box center (`index 4`)** with $2\times$ offset multiplier; **never** duplicates contours. |
| **M-07** | `Select` (`V`) | $\ge 2$ selected nodes | `Shift + Alt` | Drag on knob $k$ ($0.7 \le t \le 1.7$) | Either | Proportional scale around **bounding-box center (`index 4`)**. |
| **M-08** | `Select` (`V`) | $\ge 2$ selected nodes | None / `Shift` / `Alt` | Drag knob $k$ past opposite knob ($s_x < 0$ or $s_y < 0$) | Either | Mirrors coordinates geometrically without flipping contour winding (`reverse` is NOT called). |
| **M-09** | `Select` (`V`) | $\ge 2$ selected nodes | None | Drag outside knob $k \in \{0,2,3,5,6,7,8\}$ ($t > 1.7$) | Either | Rotates selection around **opposite knob $8 - k$** (`Alt` switches pivot to center `index 4`; `Shift` snaps angle to **$45^\circ$**). |
| **M-10** | `Select` (`V`) | $\ge 2$ selected nodes | None on `k=1` (`S`), or `Ctrl+Alt` on `k in {3,5,7}` | Drag outside knob ($t > 1.7$) | Either | Slants/shears selection (`shear_x` on `S`/`N`, `shear_y` on `W`/`E`) around opposite knob $8 - k$. |
| **M-11** | `Select` (`V`) | Empty canvas | `Ctrl` / `Alt` / `Space` | Marquee drag | Either | `Ctrl` dynamically steers `selection_angle_deg` in $[-60^\circ, +60^\circ]$ (snapping to `italic_angle` within $2^\circ$) and sets `show_bounding_box_slanted = true`; `Alt` (`!Ctrl`) snaps marquee to `italic_angle`; `Space` pans the active marquee box across the canvas! |
| **M-12** | `Rotate` (`R`) / `Scale` (`S`) | Any selection | None | Click & release ($\le 2.0\text{ dp}$ movement) at $P$ | Either | Relocates `transform_origin` to $P$ (grid-rounded unless `Ctrl` held); selection is unchanged; no undo entry pushed. |
| **M-13** | `Rotate` (`R`) / `Scale` (`S`) | Any selection | None / `Ctrl` | Drag within $3.0\text{ dp}$ of `transform_origin` | Either | Drags `transform_origin` live (`Ctrl` bypasses snap); selection is unchanged. |
| **M-14** | `Rotate` (`R`) | $\ge 1$ selected node | None / `Shift` / `Ctrl` | Drag $> 2.0\text{ dp}$ and $> 5.0\text{ UPM}$ from `transform_origin` | Either | Rotates selection around `transform_origin` (snapped to **$45^\circ$** when `Shift` held; `Ctrl` bypasses snap; `Alt` never duplicates) + draws gray ray from `transform_origin` to cursor. |
| **M-15** | `Scale` (`S`) | $\ge 1$ selected node | `Shift` | Drag $> 2.0\text{ dp}$ with $\|\sin\theta\| < 0.2$ | Either | **1-axis horizontal scale** ($s_y = 1.0$) around `transform_origin`. |
| **M-16** | `Scale` (`S`) | $\ge 1$ selected node | `Shift` | Drag $> 2.0\text{ dp}$ with $\|\sin\theta\| > 0.86$ | Either | **1-axis vertical scale** ($s_x = 1.0$) around `transform_origin`. |
| **M-17** | `Scale` (`S`) | $\ge 1$ selected node | `Shift` | Drag $> 2.0\text{ dp}$ with $0.2 \le \|\sin\theta\| \le 0.86$ | Either | **Uniform proportional scale** ($\min(\|s_x\|, \|s_y\|)$ when $s_x \le 1 \land s_y \le 1$, else $\max(\|s_x\|, \|s_y\|)$) around `transform_origin`. |
| **M-18** | `Lasso` (`Shift+V`) | Empty canvas OR on top of a path segment | None / `Shift` / `Alt` / `Shift+Alt` | Drag freeform loop & release | Either | Because `allows_path_segment_interaction == false`, dragging on a segment or empty canvas starts a Lasso and selects nodes inside closed polygon: None = replace (`OnCurve`+`OffCurve`), `Alt` = `OnCurve` only, `Shift` = XOR toggle, `Shift+Alt` = XOR `OnCurve` only. |
| **M-19** | `Zoom` (`Z` or `Cmd/Ctrl+Space`) | Any canvas point | None / `Alt` | Click or drag with $w + h < 10\text{ dp}$ | Either | Discrete 23-step zoom in (`!Alt`, $> 1.08\times$) or zoom out (`Alt`, $< 0.92\times$) keeping clicked point anchored under cursor. |
| **M-20** | `Zoom` (`Z` or `Cmd/Ctrl+Space`) | Any canvas region | None / `Alt` | Marquee drag with $w + h \ge 10\text{ dp}$ | Either | Without `Alt`: zooms in so marquee fills viewport; with `Alt`: zooms out so viewport fits inside marquee; centers marquee center. |
| **M-21** | Any | All `OnCurve` nodes of contour selected | `Cmd/Ctrl+C` then `Cmd/Ctrl+V` | Keypress | `Open` or `Closed` | Copies full contour preserving `closed` flag and pastes duplicate (with `(+10, +10)` collision offset if an identical contour already sits at those exact coordinates, else `(0, 0)`); only newly pasted nodes are selected. |
| **M-22** | Any | Partial `OnCurve` chain selected (including across `P0` seam) | `Cmd/Ctrl+C` then `Cmd/Ctrl+V` | Keypress | `Closed` | Extracts contiguous chain across `P0` seam as **1 single open contour (`closed = false`)**, strips trailing `OffCurve` handles, sets new `P0` to `Corner`; if $\ge 2$ contiguous nodes are selected on target contour during `Cmd/Ctrl+V`, splices the open contour in place! |
| **M-23** | Any | Empty selection OR only `OffCurve` handle(s) selected | `Cmd/Ctrl+C` / `Cmd/Ctrl+X` | Keypress | Either | **No-op** (existing clipboard is preserved untouched). |
| **M-24** | Any | Selected node(s) on closed contour | `Cmd/Ctrl+X` | Keypress | `Closed` | Copies selection to clipboard AND deletes selected nodes via **`cut_open_delete` (`Shift+Delete`)**, breaking the closed contour open (`closed = false`). |
| **M-25** | Any | 1 `OnCurve` node selected | `make_node_first` | Action | `Closed` vs `Open` | On `Closed`: rotates contour so selected node is `P0`. On `Open`: if selected node is `P_last`, reverses contour; if `P0` or interior node, **no-op**. |
| **M-26** | Any | Outer + inner closed contours + open contour | `Cmd/Ctrl+Shift+R` (`correct_path_direction`) | Keypress | Mixed | Always operates on whole layer; open contour untouched; closed contours unsheared by `italic_angle`, set to even-depth=`CCW`, odd-depth=`CW`, and `P0` rotated to lowest-`Y` (tie-break lowest-`X` within `0.001 UPM`) `OnCurve` node. |
| **M-27** | Any | Cubic segment(s) (selected or all if selection empty) | `add_extremes(force)` / `add_inflection_points` | Action | Either | Splits cubics at extrema ($d_0, d_3 \ge 0.8\text{ UPM}$, near-endpoint smooth absorption for $0.8 \le d < 30\text{ UPM}$ when `force == false`, orthogonalized handles, plus auto `tidy_up_paths` when selection is empty) or inflections ($d_0, d_3 > 20.0\text{ UPM}$). |
| **M-28** | Any | Path with collinear lines, duplicates ($< 0.6\text{ UPM}$), zero handles ($< 0.15\text{ UPM}$), mismatched `Corner`/`Smooth` | `Cmd/Ctrl+Shift+T` (`tidy_up_paths`) | Keypress | Either | Scoped to `selected_nodes` when non-empty or whole layer when empty: removes collinear line nodes, merges duplicate `OnCurve` nodes, retracts zero-length handles, auto-closes open endpoints $< 1.0\text{ UPM}$, and updates `Corner`/`Smooth` via `are_collinear_lines`. |
| **M-29** | Any | $\ge 2$ selected nodes | `Cmd/Ctrl+Shift+A` (`align_selection`) | Keypress | Either | For 2 nodes (`1 OnCurve + 1 adjacent OffCurve`): snaps `OffCurve` to `OnCurve.x` ($\Delta x \le \Delta y$) or `OnCurve.y` ($\Delta x > \Delta y$) and rotates unselected opposite smooth handle; for $\ge 2$ nodes: aligns to `aligned_point_for_bounds(transform_grid_anchor, bounds)` (`min`, `mid`, or `max` along smaller span) and shifts unselected attached `OffCurve` handles. |
| **M-30** | `Select` (`V`) | Node sitting on BBox corner ($t < 0.7$) | None | Click / Drag at $t < 0.7$ | Either | Tier 1 BBox knob returns `None` ($t < 0.7$), falling through cleanly to Tier 2 (`OffCurve`) / Tier 3 (`OnCurve`) node hit-testing. |
| **M-31** | `Rotate` (`R`) / `Scale` (`S`) | Selection changed via temporary `Cmd`-Select override | `Cmd` held then released | Click / Marquee in temp `SelectTool` | Either | `update_transform_origin()` automatically recomputes `transform_origin` to `transform_grid_anchor` (`4` = center) of the new `selection_bounds`. |
| **M-32** | Any (`TransformPanel`) | `selected_nodes` non-empty vs. `selected_nodes` empty | Programmatic action | `flip_horizontal`, `flip_vertical`, `rotate_by_degrees`, `scale_by_factor`, `slant_by_degrees` | Either | Transforms `selected_nodes` around `BoundingBoxOrigin(0..8)` of `selection_bounds` (or `MetricsOrigin(0..4)` with `italic_angle`) when selection is non-empty, or transforms **entire glyph layer** around `BoundingBoxOrigin(0..8)` of `layer_fast_bounds` (or `MetricsOrigin`) when selection is empty (flips do NOT reverse contour winding). |
| **M-33** | Any | 2 open endpoints selected (same open contour vs. 2 different open contours; $d < 1.0\text{ UPM}$ vs. $d \ge 1.0\text{ UPM}$) | Context menu / `connect_selected_open_endpoints` | Action | `Open` | Same contour: closes contour (`closed = true`). Different contours with $d < 1.0\text{ UPM}$: merges endpoints into 1 node in 1 combined open contour (`closed = false`). Different contours with $d \ge 1.0\text{ UPM}$: keeps both endpoints joined by a straight `Line` bridge in 1 combined open contour (`closed = false`). |
| **M-34** | `Select` / `Lasso` / `Rotate` / `Scale` / `Zoom` | Hovering BBox knobs ($0.7 \le t \le 1.7$ vs. $t > 1.7$) or active v0.2.0 tool | None / `Alt` / `Ctrl+Alt` | Hover / Move | Either | `desired_cursor_kind` evaluates live `Ctrl`/`Alt` modifiers (`Ctrl+Alt` on `k in {3,5,7}` -> `BBoxSlant`; `Alt` in `ZoomTool` -> `ZoomOut`), and Pass 5 paints distinct vector cursor geometries for `ZoomIn` (`+` magnifier), `ZoomOut` (`-` magnifier), `Rotate`, `Scale`, `Lasso`, and `BBoxSlant` (never falling back to `SelectArrow`). |
| **M-35** | `Select` (`V`) | Active slanted Marquee (`show_bounding_box_slanted && selection_angle_deg.abs() > 0.0`) | `Ctrl` or `Alt` | Marquee drag | Either | Pass 5 (`painter_pass.rs` & `headless.rs`) renders the slanted 4-vertex parallelogram $[V_0, V_1, V_2, V_3]$ matching `slanted_marquee_parallelogram_screen` rather than an axis-aligned rectangle. |
| **M-36** | Any (`Select`, `Pen`, `Rotate`, `Scale`, `Lasso`) | Any canvas point or marquee | `Cmd/Ctrl + Space` or `Cmd/Ctrl + Alt + Space` | Keypress + Click / Marquee drag | Either | Overrides active tool with `ZoomTool` (taking priority over `Space` Hand-pan AND over `Cmd`-Select override), zooming in (`!Alt`) or out (`Alt`) and restoring the prior tool on release. |
| **M-37** | `Scale` (`S`) | Drag start $A$ with $\text{hypot}(A - O) > 5.0\text{ UPM}$ but $\|A_x - O_x\| \le 5.0\text{ UPM}$ or $\|A_y - O_y\| \le 5.0\text{ UPM}$ | Any | Drag $> 2.0\text{ dp}$ | Either | Per-axis guard keeps `TransformPendingOrDragging.active == false`: no scaling is applied, no active ray line is drawn, and no no-op undo entry is pushed. |
| **M-38** | Demonstrator (`examples/grafia_canvas.rs`) | Interactive toolbar & `TransformPanel` | UI click | All `SPEC-003` Path Ops & Transform Panel actions | Either | Exposes 9-point `TransformGridAnchor` (`0..8`) & `MetricsOrigin`, `Reverse`, `Make First`, `Correct Dir`, `Extremes`, `Inflections`, `Tidy`, `Align`, `Connect`, `Flip H`, `Flip V`, `Rotate`, `Scale`, and `Slant` within $\le 300\text{ eSLOC}$. |

---

## 4. Pre-Code Self-Audit Checklist

1. **Geen samenvattende placeholders ("etc.", "enz.", "vergelijkbaar met"):** Gecontroleerd — alle domeinen, alle drempelwaarden (`4.0 dp`, `0.7..1.7`, `5.0 UPM`, `2.0 dp`, `3.0 dp`, `10.0 dp`, `0.2..0.86`, `0.8 UPM`, `30.0 UPM`, `20.0 UPM`, `0.15 UPM`, `0.6 UPM`, `1.0 UPM`, `0.25 * sqrt(min(L1, L2))`, `11.0 dp` badge font, `12.0 dp` min pill height, `6.0 dp` badge offset) en alle modifier-combinaties zijn expliciet uitgeschreven.
2. **Mid-drag modifier-transities:** Tijdens Bounding-Box Scale/Rotate, `RotateTool` (`R`), `ScaleTool` (`S`), `CanvasTool::Lasso` (`Shift+V`), `ZoomTool` (`Z`) en `SelectTool` (`V`) marquee-drag (`Ctrl`, `Alt`, `Space`) worden modifiers bij elke `PointerMove` live geëvalueerd tegen `shadow_snapshot` / `initial_selected_nodes`.
3. **Open vs. Gesloten topologie:** Expliciet gespecificeerd voor `Copy`/`Cut`/`Paste` (inclusief open-contour splice), `reverse_selected_contours`, `make_node_first`, `correct_path_direction`, `tidy_up_paths`, `connect_selected_open_endpoints`, en `TransformPanel` flips.
4. **Undo/Redo & Centrale Invarianten:** Elke muterende actie in `SPEC-003` moet één atomaire `UndoEntry` vastleggen en `state.assert_invariants()` aanroepen.

---

## 5. Normative COTS Test Matrix (`tests/v020_complete_tests.rs`)

| Test ID | Test Function Name | Verified Scenarios (`M-01` .. `M-38`) |
| :--- | :--- | :--- |
| **`TC-V020-01`** | `test_spec003_bbox_calculation_active_knobs_hit_zones_and_priority_fallthrough` | `M-01`, `M-02`, `M-03`, `M-11`, `M-30` (`selection_bounds` excludes unselected BCPs/extrema, 2-knob degenerate `[1,7]`/`[3,5]`, live Width/Height pill badges, 3-zone ray hit-test $t < 0.7$ node fallthrough, $0.7 \le t \le 1.7$ scale, $t > 1.7$ rotate/slant, slanted & `Space`-panned marquee) |
| **`TC-V020-02`** | `test_spec003_bbox_scale_rotate_slant_pivots_and_partial_bcp_alignment` | `M-04`, `M-05`, `M-06`, `M-07`, `M-08`, `M-09`, `M-10` (opposite-knob vs `Alt`-center $2\times$ pivot without duplicating, `Shift` proportional/45°, negative scale mirror without winding flip, `S`-knob & `Ctrl+Alt` slant, 3-pass `OffCurve` alignment & grid-rounding) |
| **`TC-V020-03`** | `test_spec003_rotate_and_scale_tools_origin_relocation_cmd_update_and_shift_zones` | `M-12`, `M-13`, `M-14`, `M-15`, `M-16`, `M-17`, `M-31` (shared `transform_origin` between `R` & `S`, click $\le 2\text{ dp}$ relocates origin, drag $< 3\text{ dp}$ moves origin, `Cmd`-Select updates `transform_origin`, `R` `Shift` 45° snap & `> 5 UPM` guard, `S` `Shift` 3-zone $\|\sin\theta\| < 0.2$, $> 0.86$, $0.2..0.86$ $\min/\max$) |
| **`TC-V020-04`** | `test_spec003_lasso_tool_freeform_polygon_segment_suppression_and_modifiers` | `M-18` (`Shift+V` Lasso disables segment drag via `allows_path_segment_interaction == false`, sampling $> 2\text{ dp}$, replace, `Alt` `OnCurve`-only, `Shift` XOR, `Shift+Alt` XOR `OnCurve`-only, solid stroke + fill rendering) |
| **`TC-V020-05`** | `test_spec003_zoom_tool_discrete_steps_and_marquee_10dp_threshold` | `M-19`, `M-20` (`Z` and `Cmd/Ctrl+Space` override, 23-step discrete zoom in/out at cursor when $w + h < 10\text{ dp}$, marquee zoom-in and `Alt` marquee zoom-out when $w + h \ge 10\text{ dp}$) |
| **`TC-V020-06`** | `test_spec003_clipboard_copy_cut_paste_closed_seam_collision_offset_and_splice` | `M-21`, `M-22`, `M-23`, `M-24` (full contour copy, wrap-around across `P0` seam as 1 open contour, 2 disjoint chains -> 2 open contours, empty/`OffCurve`-only copy no-op, `Paste` `(+10,+10)` collision offset vs `(0,0)`, open-contour splice, `Cut` breaks closed contour open via `cut_open_delete`) |
| **`TC-V020-07`** | `test_spec003_path_ops_reverse_make_first_correct_direction_extremes_tidy_align_connect_and_panel` | `M-25`, `M-26`, `M-27`, `M-28`, `M-29`, `M-32`, `M-33` (`make_node_first` closed vs open `P_last` vs interior no-op, `correct_path_direction` 3-level nesting + italic unshear + `find_min_node` + open ignored, `add_extremes(force)` near-endpoint absorption vs force + `add_inflection_points`, selection-scoped `tidy_up_paths` + red/yellow duplicate circles, `align_selection` 2-node handle snap + 9-point `0..8` alignment, `connect_selected_open_endpoints` `< 1.0 UPM` vs `>= 1.0 UPM`, `TransformPanel` `BoundingBoxOrigin` & `MetricsOrigin` + `Undo`/`Redo`) |
| **`TC-V020-08`** | `test_spec003_vector_cursors_slanted_marquee_render_cmd_space_zoom_and_scale_axis_guard` | `M-34`, `M-35`, `M-36`, `M-37`, `M-38` (distinct Pass-5 vector cursor rendering & modifier-aware `desired_cursor_kind` for `ZoomIn`/`ZoomOut`/`Rotate`/`Scale`/`Lasso`/`BBoxSlant`, slanted parallelogram marquee rendering in `painter_pass` & `headless`, `Cmd/Ctrl+Space` & `Cmd/Ctrl+Alt+Space` temporary `ZoomTool` override priority over `Space` pan & `Cmd`-Select, `ScaleTool` per-axis $> 5.0\text{ UPM}$ pivot guard, and demonstrator `TransformPanel`/Path Ops parity) |

---

## 6. Normative CAGD Reference Coordinate Vectors

All reference coordinate vectors below are analytically derived from exact de Casteljau cubic subdivision, root-finding, and topological path normalization rules. COTS tests in `tests/v020_complete_tests.rs` MUST assert against these exact coordinate outputs.

### 6.1 `add_extremes(force: bool)` Reference Vectors

#### Reference Vector 6.1A — Tilted / Bowed Cubic (3 Extrema: Left X, Top Y, Right X)
- **Input (`closed = true, count = 4, grid_length = 0.0`):**
  - `[0] (20.0, 0.0) Line Corner`, `[1] (-40.0, 60.0) OffCurve`, `[2] (140.0, 120.0) OffCurve`, `[3] (80.0, 0.0) Cubic Corner`
- **Output (`add_extremes(false)`, `closed = true, count = 13`):**
  - `[0] (20.000000, 0.000000)` `Line` (`Corner`)
  - `[1] (11.213203, 8.786797)` `OffCurve`
  - `[2] (7.573593, 17.573594)` `OffCurve`
  - `[3] (7.573593, 25.795050)` `Cubic` (`Smooth`) — **Left X-Extremum**
  - `[4] (7.573594, 49.985812)` `OffCurve`
  - `[5] (39.084186, 69.282031)` `OffCurve`
  - `[6] (63.700905, 69.282032)` `Cubic` (`Smooth`) — **Top Y-Extremum**
  - `[7] (79.479876, 69.282033)` `OffCurve`
  - `[8] (92.426409, 61.353934)` `OffCurve`
  - `[9] (92.426407, 41.704946)` `Cubic` (`Smooth`) — **Right X-Extremum**
  - `[10] (92.426406, 31.286792)` `OffCurve`
  - `[11] (88.786795, 17.573590)` `OffCurve`
  - `[12] (80.000000, 0.000000)` `Cubic` (`Corner`)

#### Reference Vector 6.1B — Near-Endpoint Extremum ($d = 7.950926\text{ UPM} \in [0.8, 30.0)$): `Corner` vs. `Smooth` & `force=false` vs. `force=true`
- **Input (`closed = true, count = 7`):**
  - `[0] (0, 0) Line Corner`, `[1] (0, 60) OffCurve`, `[2] (50, 103) OffCurve`, `[3] (100, 98) Cubic` (`Corner` or `Smooth`), `[4] (150, 93) OffCurve`, `[5] (200, 50) OffCurve`, `[6] (200, 0) Cubic Corner`
- **Normative Outputs:**
  1. **`[3] (100, 98)` is `Corner`, `force = false`:**
     - **Suppressed (`count = 7`, unchanged)!** Because `[3]` is `Corner` and $d = 7.951 < 30.0\text{ UPM}$, `add_extremes(false)` does not crowd the sharp corner.
  2. **`[3] (100, 98)` is `Corner`, `force = true`:**
     - **Inserts extremum at `(92.058928, 98.395119)` AND keeps `[6] (100, 98)` (`count = 10`):**
       `[0] (0, 0)`, `[1] (0.000000, 56.820595)`, `[2] (44.841390, 98.395118)`, `[3] (92.058928, 98.395119) Smooth`, `[4] (94.700992, 98.395119)`, `[5] (97.350496, 98.264950)`, `[6] (100.000000, 98.000000) Corner`, `[7] (150, 93)`, `[8] (200, 50)`, `[9] (200, 0)`.
  3. **`[3] (100, 98)` is `Smooth`, `force = false` (or `true`):**
     - **Inserts extremum at `(92.058928, 98.395119)` AND deletes-and-heals the old smooth node `(100, 98)` (`count = 7`):**
       `[0] (0, 0)`, `[1] (0.000000, 56.820595)`, `[2] (44.841390, 98.395118)`, `[3] (92.058928, 98.395119) Smooth`, `[4] (144.612188, 98.395119)`, `[5] (200.000000, 52.625000)`, `[6] (200, 0)`.

### 6.2 `add_inflection_points` Reference Vectors
1. **Symmetric S-Curve (`[(0,0) Line, (0,100) OffCurve, (100,0) OffCurve, (100,100) Cubic]`, `closed = false`):**
   - Output (`count = 7`): `[0] (0, 0)`, `[1] (0, 50)`, `[2] (25, 50)`, `[3] (50.000000, 50.000000) Smooth` ($t = 0.5$), `[4] (75, 50)`, `[5] (100, 50)`, `[6] (100, 100)`.
2. **Asymmetric S-Curve (`[(0,0) Line, (20,140) OffCurve, (80,-20) OffCurve, (100,100) Cubic]`, `closed = false`):**
   - Output (`count = 7`): `[0] (0.000000, 0.000000)`, `[1] (10.363877, 72.547140)`, `[2] (31.468749, 64.536818)`, `[3] (52.182781, 56.674839) Smooth` ($t = 0.51819385$), `[4] (71.442268, 49.364930)`, `[5] (90.363877, 42.183263)`, `[6] (100.000000, 100.000000)`.

### 6.3 `tidy_up_paths` & Connection Check Reference Vector
- **Input (`2 contours`):**
  - `Contour 0 (closed = true, count = 9)`: `[0] (0,0) Line Corner`, `[1] (0,0) Line Corner` (duplicate), `[2] (33,0) OffCurve`, `[3] (67,0) OffCurve`, `[4] (100,0) Cubic Corner` (collinear cubic), `[5] (100,100) Line Corner` (collinear with outgoing `(100,150)`), `[6] (100,150) OffCurve`, `[7] (50,200) OffCurve`, `[8] (0,200) Cubic Smooth` ($90^\circ$ corner into closing vertical line `(0,200)->(0,0)` falsely marked `Smooth`).
  - `Contour 1 (closed = false, count = 1)`: `[0] (300,300) Line Corner` (degenerate).
- **Output (`1 contour, closed = true, count = 6`):**
  - `[0] (0.0, 0.0) Line Corner`
  - `[1] (100.0, 0.0) Line Corner` (duplicate `(0,0)` removed; collinear cubic converted to `Line`)
  - `[2] (100.0, 100.0) Line Smooth` (promoted from `Corner` to `Smooth`)
  - `[3] (100.0, 150.0) OffCurve`
  - `[4] (50.0, 200.0) OffCurve`
  - `[5] (0.0, 200.0) Cubic Corner` (demoted from `Smooth` to `Corner`)

### 6.4 `correct_path_direction` Reference Vector (3-Level Nested Contours + Open Contour)
- `Path 0` (Outer `[0,0]..[500,500]`, input CW `[(0,0), (0,500), (500,500), (500,0)]`) $\to$ **CCW (`direction = -1`)**: `[(500,0), (500,500), (0,500), (0,0)]`.
- `Path 1` (Counter `[100,100]..[400,400]`, input CCW `[(100,100), (400,100), (400,400), (100,400)]`) $\to$ **CW (`direction = 1`)**: `[(100,400), (400,400), (400,100), (100,100)]`.
- `Path 2` (Inner Island `[200,200]..[300,300]`, input CW `[(200,200), (200,300), (300,300), (300,200)]`) $\to$ **CCW (`direction = -1`)**: `[(300,200), (300,300), (200,300), (200,200)]`.
- `Path 3` (Open `[(0,600), (500,600)]`) $\to$ **Unchanged (`closed = false`)**: `[(0,600), (500,600)]`.

### 6.5 `align_selection` Reference Vectors
1. **`1 OnCurve (100, 200)` + `1 OffCurve (110, 260)` ($\Delta x = 10 < \Delta y = 60$):**
   - Output: `OnCurve = (100.0, 200.0)` (unchanged), `OffCurve = (100.0, 260.0)` (snapped X to `100.0`).
2. **`3 OnCurve Nodes [(100, 200), (150, 215), (200, 230)]` ($\Delta X = 100 > \Delta Y = 30$) across 9-Point `transform_grid_anchor`:**
   - `transform_grid_anchor` $\in \{0, 1, 2\}$ (Bottom): `[(100, 200.0), (150, 200.0), (200, 200.0)]` (`minY = 200.0`).
   - `transform_grid_anchor` $\in \{3, 4, 5\}$ (Middle): `[(100, 215.0), (150, 215.0), (200, 215.0)]` (`midY = 215.0`).
   - `transform_grid_anchor` $\in \{6, 7, 8\}$ (Top): `[(100, 230.0), (150, 230.0), (200, 230.0)]` (`maxY = 230.0`).
