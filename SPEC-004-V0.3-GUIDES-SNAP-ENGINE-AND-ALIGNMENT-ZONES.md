# SPEC-004: Grafia v0.3.0 — Local & Global Guides, Magnetic `SnapEngine`, Alignment Zone Badges & Canvas Grid

- **Document ID**: `SPEC-004`
- **Target Version**: `grafia v0.3.0` (`grafia-core`, `grafia-egui`, `grafia-headless`)
- **Status**: Normative Specification (ISO/IEC/IEEE 29119-3:2021 Test-First Anchoring)
- **Normative Baseline**: OpenType 1.9 / CAGD / ISO 9241-9 Mathematical & Ergonomic Baseline
- **Predecessors**: `SPEC-001`, `SPEC-002-V0.1-CANVAS-INTERACTION-AND-RENDERING.md`, `SPEC-003-V0.2-TRANSFORMS-LASSO-AND-PATH-OPS.md`

---

## 1. Normative Mathematical & Ergonomic Baseline (OpenType 1.9 / CAGD / ISO 9241-9)

| Subsystem | Normative Domain & Shortcut | Mathematical & Ergonomic Specification |
| :--- | :--- | :--- |
| **Local & Global Guides (`Cmd/Ctrl+Shift+L`)** | Parametric Reference Lines & Smart 2-Node Placement | Infinite guide lines $\mathbf{p}(s) = \mathbf{p}_0 + s(\cos\alpha, \sin\alpha)$ with angle $\alpha \in (-180^\circ, 180^\circ]$, local (`#3E4CCFB3`) vs. global (`#CF4F3EB3`) scope, `locked`, `lock_angle`, and `show_measurement`. **Smart 2-Node Rule:** 0 nodes places $0^\circ$ at cursor; 1 node places $0^\circ$ at $A$; 2 nodes places midpoint $\frac{1}{2}(A+B)$ at angle $\text{atan2}(B_y-A_y, B_x-A_x)$. Double-clicking knob flips $+90^\circ$; dragging line rotates around $\mathbf{p}_0$ (`Shift` snaps $45^\circ$ / $90^\circ - \theta_{\text{italic}}$). |
| **Magnetic `SnapEngine` (`Ctrl` Bypass)** | Multi-Source Geometric Snapping (`5.0 dp` threshold) | Evaluates candidate snappers ($0^\circ$ horizontal, $90^\circ$ vertical, $90^\circ - \theta_{\text{italic}}$ italic, guide rays, and point targets) across unselected `OnCurve`/`OffCurve` nodes, metrics lines, alignment zone bases, and visible guides. Resolves 2-line intersections $L_1 \cap L_2$ from distinct anchors within $\sqrt{2}\cdot\text{tol}$, point targets, or closest single line; bypassed completely when `modifiers.ctrl == true`. Renders `#993333FF` `1.2 dp` snap rays and `8.0 dp` endpoint circles in Pass 4. |
| **Alignment Zone Bands & Pass 4 Node Badges (`Mode 1..4`)** | OpenType 1.9 Overshoot Zones & Node Classification | Pass 1 renders `#CEA77C33` horizontal overshoot bands sheared by $\theta_{\text{italic}}$ around $\frac{1}{2}x_{\text{height}}$. Pass 4 classifies `OnCurve` nodes into Mode `1` (`OnMetricLine` $|y - m| < 0.6$, beige diamond), Mode `2` (`OnGuideLine` $d_\perp < \text{clamp}(0.5g, 0.4, 2.0)$, blue/red diamond), Mode `3` (`InAlignmentZone` overshoot band, beige circle), and Mode `4` (`OnVerticalMetric` $|x' - 0| < 0.6$ or $|x' - W| < 0.6$, sheared beige rectangle). |
| **Two-Tier Canvas Grid & Sloped Grid** | Modular Subdivision & Zoom Visibility Gating | Effective step $\text{sub\_step} = \text{grid\_step\_upm} / \max(\text{grid\_subdivisions}, 1)$. Zoom visibility gates: main grid drawn iff $\Delta_{\text{main,px}} > 8.0\text{ dp}$ (`#80736666`), subdivision grid drawn iff $\text{subdivisions} \ge 2 \land \Delta_{\text{sub,px}} > 4.0\text{ dp}$ (`#40595940`). Supports sloped vertical grid lines sheared by $\Delta x(y) = y \cdot (\text{slope\_run} / \text{slope\_rise})$. |
| **3-Way `AppearanceMode` (`Light`, `Dark`, `System`)** | Host-Controlled Chromatic Theme Resolution | Resolves `AppearanceMode::{Light, Dark, System}` deterministically to `GrafiaTheme::sonoma_default()` (`#FFFFFF` canvas) or `GrafiaTheme::sonoma_dark()` (`#1E1E1E` canvas) with 1-to-1 rendering parity between `painter_pass.rs` and `headless.rs`. |

`grafia v0.3.0` delivers complete mathematical, ergonomic, and visual support for:
1. **Local & Global Guides**:
   - Infinite and bounded guide lines with position, angle (`(-180.0, 180.0]`), `is_global` (glyph-local guides vs. master-global guides), `locked`, `lock_angle`, `show_measurement`, and optional `name`.
   - `Add Guide` (Local Blue `#3E4CCFB3`) and `Add Global Guide` (Global Red `#CF4F3EB3`) with **0-node**, **1-node**, and **Smart 2-Node Rule**.
   - Guide knob selection, drag-to-move (`Option`-drag to duplicate), arrow-key nudge (`1` / `10` / `100 UPM`), `Tab` cycling, **Double-Click on Guide Knob** (`+90.0°` perpendicular flip), continuous drag-on-line rotation around `guide.position` (`Shift` $45^\circ$ / italic angle snap), `lock_angle` enforcement, and `lock_guides` padlock rendering.
   - **Measurement Guide Mode (`show_measurement == true`)**: permanent ray-contour intersection solver & stem distance labels in Pass 4 even when `SelectTool` (`V`) or `PenTool` (`P`) is active.
2. **Magnetic `SnapEngine`**:
   - Real-time magnetic snapping during `SelectTool` node/handle dragging and `PenTool` point placement.
   - Candidate snappers: horizontal ($0^\circ$), vertical ($90^\circ$), italic-angle ($90^\circ - \text{italic\_angle}$), point-to-point, and line-to-line intersections against unselected `OnCurve`/`OffCurve` nodes, metrics lines (`Baseline`, `x-Height`, `Cap Height`, `Ascender`, `Descender`, `LSB`, `RSB`), alignment zone boundaries (`position` and `position + size`), and visible Local/Global guides.
   - `Control` key (`modifiers.ctrl`) live snap bypass.
   - Pass 4 visual indicator rendering: `#993333FF` (`rgba(153, 51, 51, 255)`) `1.2 dp` lines connecting `snapper.from` $\to$ `snapper.to` with `8.0 dp` diameter (`4.0 dp` radius) endpoint circles.
3. **Alignment Zone Bands & Pass 4 Alignment Indicators**:
   - Pass 1 translucent horizontal bands (`#CEA77C33`, `rgba(206, 167, 124, 51)`) for every `AlignmentZone { position, size }` in `Metrics`.
   - Pass 4 alignment badges:
     - **Mode `1`** (On Horizontal Metric Line or Zone Base `|y - pos| < 0.6`): Beige Diamond (`#CEA77C80` fill + `#8C8C8CCC` `1.0 dp` stroke).
     - **Mode `3`** (Inside Alignment Zone Overshoot Band): Beige Circle (`#CEA77C80` fill + `#8C8C8CCC` `1.0 dp` stroke).
     - **Mode `4`** (On Vertical LSB / RSB Line `|x' - 0| < 0.6` or `|x' - advance_width| < 0.6` after unslanting by `italic_angle`): Sheared Beige Rectangle (`#CEA77C80` fill + `#8C8C8CCC` `1.0 dp` stroke).
     - **Mode `2`** (On Visible Local or Global Guide within $\text{clamp}(0.5 \cdot \text{grid\_length}, 0.4, 2.0)$): Local Guide Blue Diamond (`#3E4CCF66`) or Global Guide Red Diamond (`#CF4F3E66`).
4. **Canvas Grid**:
   - Pass 1 main grid (`grid_step_upm`) and subdivision grid (`grid_subdivisions`) lines with optional sloped vertical lines (`grid_slope_rise`, `grid_slope_run`), zoom-dependent visibility thresholds (`main_step_px > 8.0 dp`, `sub_step_px > 4.0 dp` when `grid_subdivisions >= 2`), `show_grid` toggle (`View > Show Grid`), and `effective_grid_step = grid_step_upm / grid_subdivisions.max(1)` rounding parity.

All modules must strictly obey clean-room software engineering standards:
- Maximum `399` effective SLOC per `.rs` file (`tests/architecture_guard_tests.rs`).
- Zero magic numbers or hardcoded `Color32` / `rgba` literals outside `src/tokens/`.
- 1-to-1 pixel/primitive parity between `grafia-egui` (`painter_pass.rs`) and `grafia-headless` (`headless.rs`).
- Full `CommandHistory` (`Cmd/Ctrl+Z` / `Cmd/Ctrl+Shift+Z`) snapshot coverage for all guide and snapping mutations.

---

## 2. Normative Mathematical & Ergonomic Baseline (OpenType 1.9 / CAGD / ISO 9241-9)

### 2.1 Exact Color Tokens

| Token Name | Normative Role | Exact Float RGBA | Exact 8-bit `Rgba` (`#RRGGBBAA`) | Purpose |
| :--- | :--- | :--- | :--- | :--- |
| `local_guide_color` | Local Guide Default | `(0.243, 0.298, 0.812, 0.700)` | `Rgba::new(62, 76, 207, 179)` (`#3E4CCFB3`) | Local guide line & knob fill |
| `global_guide_color` | Global Guide Default | `(0.812, 0.310, 0.243, 0.700)` | `Rgba::new(207, 79, 62, 179)` (`#CF4F3EB3`) | Global guide line & knob fill |
| `alignment_zone_fill` | Alignment Zone Band | `(0.808, 0.655, 0.486, 0.200)` | `Rgba::new(206, 167, 124, 51)` (`#CEA77C33`) | Pass 1 alignment zone band fill |
| `alignment_badge_metric_fill` | Metric / Zone Badge Fill (`alpha: 0.5`) | `(0.808, 0.655, 0.486, 0.500)` | `Rgba::new(206, 167, 124, 128)` (`#CEA77C80`) | Pass 4 badge fill for `mode = 1, 3, 4` |
| `alignment_badge_local_guide_fill` | Local Guide Badge Fill (`alpha: 0.4`) | `(0.243, 0.298, 0.812, 0.400)` | `Rgba::new(62, 76, 207, 102)` (`#3E4CCF66`) | Pass 4 badge fill for `mode = 2` (Local Guide) |
| `alignment_badge_global_guide_fill` | Global Guide Badge Fill (`alpha: 0.4`) | `(0.812, 0.310, 0.243, 0.400)` | `Rgba::new(207, 79, 62, 102)` (`#CF4F3E66`) | Pass 4 badge fill for `mode = 2` (Global Guide) |
| `alignment_badge_stroke` | Neutral Gray Outline (`white: 0.55, alpha: 0.8`) | `(0.550, 0.550, 0.550, 0.800)` | `Rgba::new(140, 140, 140, 204)` (`#8C8C8CCC`) | Pass 4 badge `1.0 dp` outline stroke |
| `snap_indicator_color` | Magnetic Snapper Line | `(0.600, 0.200, 0.200, 1.000)` | `Rgba::new(153, 51, 51, 255)` (`#993333FF`) | Active magnetic snapper line & endpoint circles |
| `grid_main_color` | Main Grid Line | `(0.500, 0.450, 0.400, 0.400)` | `Rgba::new(128, 115, 102, 102)` (`#80736666`) | Pass 1 main grid lines (`0.5 dp` ortho / `0.7 dp` sloped) |
| `grid_sub_color` | Subdivision Grid Line | `(0.250, 0.350, 0.350, 0.250)` | `Rgba::new(64, 89, 89, 64)` (`#40595940`) | Pass 1 subdivision grid lines (`0.4 dp` ortho / `0.6 dp` sloped) |

### 2.2 Guide Geometry & Data Model

1. **Angle Normalization**:
   - Any angle $\alpha$ is normalized into $(-180.0^\circ, 180.0^\circ]$:
     - While $\alpha > 180.0^\circ$: $\alpha \leftarrow \alpha - 360.0^\circ$.
     - While $\alpha \le -180.0^\circ$: $\alpha \leftarrow \alpha + 360.0^\circ$.
   - If `guide.lock_angle == true` or `guide.locked == true`, attempts to mutate `angle_deg` are ignored (no-op).
2. **Infinite Line Ray Endpoints**:
   - With `GUIDE_INFINITE_HALF_EXTENT_UPM = 200_000.0`:
     $$\mathbf{u} = (\cos(\alpha \cdot \tfrac{\pi}{180}),\; \sin(\alpha \cdot \tfrac{\pi}{180}))$$
     $$\mathbf{p}_{\text{begin}} = \mathbf{p}_0 - 200000.0 \cdot \mathbf{u}, \qquad \mathbf{p}_{\text{end}} = \mathbf{p}_0 + 200000.0 \cdot \mathbf{u}$$
3. **Guide Hit-Testing**:
   - Knob hit-test: Euclidean distance from cursor $\mathbf{q}$ to `guide.position` $\le \text{guide\_knob\_hit\_radius\_upm}$ (`7.0 / scale`).
   - Line hit-test: perpendicular distance from $\mathbf{q}$ to line through `guide.position` at `guide.angle_deg`:
     $$d_\perp(\mathbf{q}) = |(\mathbf{q}_x - p_{0,x})(-\sin\theta) + (\mathbf{q}_y - p_{0,y})\cos\theta| < \text{tolerance}$$
4. **Adding Guides & Smart 2-Node Rule**:
   - **0 selected nodes (or $>2$ selected nodes)**: `position = cursor_or_viewport_center`, `angle_deg = 0.0`.
   - **1 selected node $A$**: `position = A.position`, `angle_deg = 0.0`.
   - **2 selected nodes $A, B$ (Smart 2-Node Rule)**:
     $$\text{position} = \left(\frac{A_x + B_x}{2.0},\; \frac{A_y + B_y}{2.0}\right), \qquad \text{angle\_deg} = \text{normalize\_angle}\!\left(\text{atan2}(B_y - A_y,\; B_x - A_x) \cdot \frac{180}{\pi}\right)$$
   - Adding a guide automatically sets `show_guides = true`, clears node selection, and selects the newly created guide (`SelectedGuideRef`).
5. **Guide Knob Interaction & Rotation**:
   - **Click on Guide Knob**: selects the guide.
   - **Drag on Guide Knob**: translates `guide.position` (unless `lock_guides == true` or `guide.locked == true`). Holding `Alt`/`Option` on initial drag duplicates the guide and drags the duplicate.
   - **Double-Click on Guide Knob**: flips `guide.angle_deg` by $+90.0^\circ$ perpendicular (`normalize_angle(angle_deg + 90.0)`), unless `guide.lock_angle == true` or `lock_guides == true`.
   - **Drag on Selected Guide Line (away from Knob)**: continuously rotates `guide.angle_deg` around `guide.position` toward the cursor:
     $$\alpha = \text{normalize\_angle}\!\left(\text{atan2}(q_y - p_{0,y},\; q_x - p_{0,x}) \cdot \frac{180}{\pi}\right)$$
     When `Shift` is held during guide-line rotation, $\alpha$ snaps to the nearest $45^\circ$ increment (or `90.0 - italic_angle_deg`).
   - **Arrow-Key Nudge**: when a guide is selected and `lock_guides == false`, Arrow keys translate `guide.position` by `1.0` UPM (`Shift`: `10.0` UPM, `Cmd/Ctrl`: `100.0` UPM).
   - **`Backspace` / `Delete`**: deletes the selected guide (unless `lock_guides == true`).
   - **`Tab` Cycling**: when a guide is selected (or when cycling guides), advances selection to the next guide in `[local_guides..., global_guides...]`.
6. **Measurement Guide Mode**:
   - When `guide.show_measurement == true`, computes all ray intersections of the infinite guide line with the glyph's closed contours (`compute_measurement_ray`), sorted along the guide's direction vector $\mathbf{u} = (\cos\theta, \sin\theta)$.
   - Renders intersection dots (`3.0 dp` radius), interior/exterior segment distance pills, and perpendicular tick marks permanently in Pass 4 regardless of the active tool (`SelectTool`, `PenTool`, `MeasurementTool`, etc.).

### 2.3 Magnetic `SnapEngine`

1. **Snap Threshold**:
   - `snap_threshold_dp = 5.0` screen points (`tolerance_upm = snap_threshold_dp / zoom`).
2. **Control Key Bypass**:
   - If `modifiers.ctrl == true`, `SnapEngine::snap_point` immediately returns the unsnapped input point with `active_snappers = []`.
3. **Candidate Targets**:
   - **Unselected Contour Nodes**:
     - Every unselected `OnCurve` and `OffCurve` node (excluding `OffCurve` handles belonging to a selected smooth `OnCurve` node whose opposite handle is also moving).
     - Generates:
       - Point snapper at $(N_x, N_y)$.
       - Horizontal alignment line ($0^\circ$) at $y = N_y$.
       - Vertical alignment line ($90^\circ$) at $x = N_x$.
       - Italic-angle alignment line ($90^\circ - \text{italic\_angle}$) through $(N_x, N_y)$ when `italic_angle != 0.0`.
   - **Metrics & Alignment Zones**:
     - Horizontal lines ($0^\circ$) at `Baseline (0.0)`, `x_height`, `cap_height`, `ascender`, `descender`, and for each `AlignmentZone`: `zone.position` (while `[zone.position, zone.position + zone.size]` is the continuous overshoot capture zone for Mode `3` badges).
     - Vertical/Slanted sidebearing lines ($90^\circ - \text{italic\_angle}$) at `LSB` ($x = 0$) and `RSB` ($x = \text{advance\_width}$).
   - **Visible Local & Global Guides** (when `show_guides == true`):
     - Guide line through `guide.position` at `guide.angle_deg`.
     - Guide knob point at `guide.position`.
4. **Snap Resolution & Intersection Priority**:
   - Separates directional line snappers (e.g., horizontal/guide vs. vertical/italic/guide) and checks if the best X-axis/angled snapper and best Y-axis/angled snapper from **distinct anchor sources** (`anchor_1 != anchor_2`) both lie within `tolerance_upm`:
     - If two non-parallel active snap lines $L_1, L_2$ from distinct anchors both lie within `tolerance_upm` and their intersection $L_1 \cap L_2$ is within $\sqrt{2} \cdot \text{tolerance\_upm}$ of the query point, snaps to their exact intersection point $L_1 \cap L_2$ (`intersect_line_line_unlimited`) and records both in `active_snappers` (max 2 snappers). Two lines originating from the same node (e.g., $90^\circ$ vertical and $90^\circ - \text{italic\_angle}$ through the same node) never self-intersect when the query point is outside that node's point-snap radius.
     - Otherwise, if a point target (guide knob or unselected node) lies within `tolerance_upm`, snaps directly to that point target (`is_point_target = true`); otherwise snaps to the closest single line within `tolerance_upm`.
5. **Pass 4 Rendering**:
   - For each `ActiveSnapper { from, to, is_point_target }` in `state.active_snappers`:
     - Draws a `1.2 dp` solid line from `from` to `to` in `snap_indicator_color` (`#993333FF`).
     - Draws a `4.0 dp` radius (`8.0 dp` diameter) circle at `from` and at `to` in `snap_indicator_color` (`#993333FF`).

### 2.4 Alignment Zone Bands & Pass 4 Badges

1. **Default Glyph Metrics & Alignment Zones**:
   - Standard Latin default zones on `Metrics::default()`:
     - Cap Height overshoot: `AlignmentZone { position: 700.0, size: 16.0 }`
     - x-Height overshoot: `AlignmentZone { position: 500.0, size: 16.0 }`
     - Baseline overshoot: `AlignmentZone { position: 0.0, size: -16.0 }`
     - Descender overshoot: `AlignmentZone { position: -200.0, size: -16.0 }`
2. **Pass 1 Alignment Zone Bands**:
   - When `show_metrics == true`, each `AlignmentZone` renders a horizontal band spanning $x \in [0, \text{advance\_width}]$ (sheared by `italic_angle_deg` around `x_height * 0.5`) and $y \in [\min(\text{pos}, \text{pos}+\text{size}), \max(\text{pos}, \text{pos}+\text{size})]$ filled with `alignment_zone_fill` (`#CEA77C33`).
3. **Pass 4 Node Classification**:
   - Evaluated for every `OnCurve` node $P = (x, y)$:
     - When `show_metrics == true`:
       - **Mode `1` (`OnMetricLine`)**: $|y - 0.0| < 0.6$, or $|y - m| < 0.6$ for $m \in \{\text{x\_height}, \text{cap\_height}, \text{ascender}, \text{descender}\}$, or $|y - \text{zone.position}| < 0.6$ for any `AlignmentZone`.
       - **Mode `3` (`InAlignmentZone`)**: Not Mode `1`, and for some `AlignmentZone`:
         - If `size > 0`: $\text{pos} \le y \le \text{pos} + \text{size}$.
         - If `size < 0`: $\text{pos} + \text{size} \le y \le \text{pos}$.
       - **Mode `4` (`OnVerticalMetric`)**: Not Mode `1` or `3`, and after unslanting $x' = x - (y - \tfrac{\text{x\_height}}{2}) \tan(\text{italic\_angle})$:
         - $|x' - 0.0| < 0.6$ (LSB) or $|x' - \text{advance\_width}| < 0.6$ (RSB).
     - **Mode `2` (`OnGuideLine`)**: Evaluated when `show_guides == true`: perpendicular distance from $P$ to a Local Guide (`is_global = false`) or Global Guide (`is_global = true`) is $< \text{clamp}(0.5 \cdot \text{grid\_length}, 0.4, 2.0)$ (`1.0` UPM default).
4. **Pass 4 Badge Geometry**:
   - Base screen radius $R_{\text{dp}}$ (for default `handle_size = 1.0`, `interface_scale = 1.0`):
     $$\text{scale\_factor} = \text{clamp}(\text{zoom}^{0.1}, 0.85, 1.35)$$
     - For **Mode `1`** (`OnMetricLine`): unselected $R_0 = 8.5\text{ dp}$, selected $R_0 = 10.5\text{ dp}$ $\implies$ **Diamond** of half-extent $R = R_0 \cdot \text{scale\_factor}$.
     - For **Mode `2`, `3`, `4`**: unselected $R_0 = 7.8\text{ dp}$, selected $R_0 = 10.2\text{ dp}$, $R = R_0 \cdot \text{scale\_factor}$:
       - **Mode `2`** (`OnGuideLine`): **Diamond** filled with `#3E4CCF66` (Local) or `#CF4F3E66` (Global) + `1.0 dp` stroke `#8C8C8CCC`.
       - **Mode `3`** (`InAlignmentZone`): **Circle** of radius $R$ filled with `#CEA77C80` + `1.0 dp` stroke `#8C8C8CCC`.
       - **Mode `4`** (`OnVerticalMetric`): **Rectangle** (sheared by `italic_angle_deg`) filled with `#CEA77C80` + `1.0 dp` stroke `#8C8C8CCC`.

### 2.5 Canvas Grid

1. **Grid Configuration & Subdivision Step**:
   - `grid_step_upm: f64` (`main_horizontal` / `main_vertical`, default `1.0` UPM).
   - `grid_subdivisions: u32` (`subdivision_horizontal` / `subdivision_vertical`, default `1`).
   - `grid_slope_rise: i32` (`slope_rise`, default `0`), `grid_slope_run: i32` (`slope_run`, default `0`).
   - `show_grid: bool` (`View > Show Grid`, default `true`).
   - **Effective Subdivision Step**:
     $$\text{sub\_step\_upm} = \frac{\text{grid\_step\_upm}}{\max(\text{grid\_subdivisions}, 1)}$$
     When `round_to_grid == true` and `grid_step_upm > 0.0`, coordinate rounding snaps to `sub_step_upm`.
2. **Screen-Space Step Scaling & Zoom Visibility Thresholds**:
   - Let $\Delta_{\text{main,px}} = \text{grid\_step\_upm} \cdot \text{zoom}$ and $\Delta_{\text{sub,px}} = \text{sub\_step\_upm} \cdot \text{zoom}$.
   - **Subdivision Grid Visibility (`draw_sub`)**:
     $$\text{draw\_sub} \iff \text{show\_grid} \land (\text{grid\_subdivisions} \ge 2) \land (\Delta_{\text{sub,px}} > 4.0\text{ dp})$$
   - **Main Grid Visibility (`draw_main`)**:
     $$\text{draw\_main} \iff \text{show\_grid} \land (\Delta_{\text{main,px}} > 8.0\text{ dp})$$
     Consequently, for a standard `1.0 UPM` grid (`grid_step_upm = 1.0`), the main grid is hidden at `zoom = 1.0` (`1.0 dp <= 8.0 dp`) and automatically appears when zoomed in beyond `8.0` (`800%` zoom), whereas for a modular grid (e.g., `grid_step_upm = 50.0`, `grid_subdivisions = 5`), both main (`50.0 dp > 8.0 dp`) and subdivision (`10.0 dp > 4.0 dp`) grids are visible at `zoom = 1.0`.
3. **Pass 1 Rendering & Sloped Grid**:
   - Rendered in Pass 1 (`pass_1_metrics` / `render_metrics_box`) across the visible canvas viewport aligned to the glyph origin `(0.0, 0.0)`.
   - Subdivision grid (`grid_sub_color = #40595940`) is drawn first, followed by main grid (`grid_main_color = #80736666`) on top.
   - **Sloped Grid**:
     - Active when $0 < \text{grid\_slope\_rise} < 100000$ and $\text{grid\_slope\_run} \ne 0 \land |\text{grid\_slope\_run}| < 100000$ (supporting both right-leaning $>0$ and backslanted $<0$ grids).
     - Horizontal grid lines remain at $y = k \cdot \text{step\_upm}$; vertical grid lines anchored at $(x_k, 0)$ are sheared by $\Delta x(y) = y \cdot \frac{\text{grid\_slope\_run}}{\text{grid\_slope\_rise}}$.
   - **Stroke Widths**:
     - Subdivision grid: `0.4 dp` horizontal; `0.4 dp` vertical when orthogonal, or `0.6 dp` when sloped.
     - Main grid: `0.5 dp` horizontal; `0.5 dp` vertical when orthogonal, or `0.7 dp` when sloped.

---

## 3. Normative Test Matrix (`TC-V030-001` .. `TC-V030-009`)

| Test ID | Feature Under Test | Normative Assertions |
| :--- | :--- | :--- |
| **`TC-V030-001`** | Local & Global Guide Creation + Smart 2-Node Rule + Visibility Toggle | `add_guide` (Local Blue `#3E4CCFB3`) & `add_global_guide` (Global Red `#CF4F3EB3`); 0-node ($0^\circ$ at cursor), 1-node ($0^\circ$ at node), and **2-node Smart Rule** (midpoint $\frac{A+B}{2}$ and exact $\text{atan2}(B_y-A_y, B_x-A_x)$ angle); `Cmd/Ctrl+Shift+L` toggles `show_guides`. |
| **`TC-V030-002`** | Guide Knob Selection, Move, Duplicate, Double-Click $90^\circ$ Flip, Line-Drag Rotation, Lock Angle & Lock Guides | Selecting guide knob, dragging knob to translate `position`, `Alt`-drag duplicating guide, arrow-key nudge (`1`/`10`/`100 UPM`), `Tab` cycling guides, double-clicking knob rotates $+90^\circ$, dragging guide line away from knob rotates `angle_deg` (`Shift` snaps to $45^\circ$ and `90.0 - italic_angle_deg`), `lock_angle` blocks rotation, `lock_guides` blocks move/rotation/delete and draws padlock icon (body + shackle) in Pass 4. |
| **`TC-V030-003`** | Measurement Guide Mode (`show_measurement`) | Toggling `show_measurement` on a guide invokes `calculate_guide_intersections`, finding exact sorted contour intersections and interior stem distances on `/o` and `/A`, and rendering measurement dots + distance labels permanently in Pass 4 even when `SelectTool` (`V`) or `PenTool` (`P`) is active. |
| **`TC-V030-004`** | Magnetic `SnapEngine` & Pass 4 `#993333FF` Indicators | Dragging a node/handle or placing a `PenTool` point within `5.0 dp` snaps horizontally ($0^\circ$), vertically ($90^\circ$), along `italic_angle`, and onto visible guides, metrics lines, alignment zones, and unselected nodes (including 2-line intersection $L_1 \cap L_2$), populating `active_snappers` (max 2) and rendering `#993333FF` `1.2 dp` lines + `8.0 dp` diameter endpoint circles in Pass 4; clears `active_snappers` on mouse release. |
| **`TC-V030-005`** | `Control` Key Magnetic Snap Bypass | Holding `Ctrl` (`modifiers.ctrl = true`) during node/handle drag or `PenTool` placement bypasses `SnapEngine` completely, leaving coordinates at the raw cursor position and `active_snappers` empty. |
| **`TC-V030-006`** | Alignment Zone Bands & Pass 4 Badges (`Mode 1, 2, 3, 4`) | Pass 1 renders `#CEA77C33` alignment zone bands across $x \in [0, \text{advance\_width}]$ (sheared by `italic_angle_deg`); Pass 4 classifies `OnCurve` nodes via `classify_node_alignment` (`Mode 1` Diamond on horizontal metric/zone base, `Mode 3` Circle inside overshoot zone e.g. $y=-10$ or $y=510$, `Mode 4` Sheared Rect on LSB/RSB $x=0$ / $x=\text{width}$, `Mode 2` Blue/Red Diamond on Local/Global guide) with exact unselected vs. selected dimensions. |
| **`TC-V030-007`** | `egui` $\leftrightarrow$ `headless` 1-to-1 Visual Parity & Full Undo/Redo Bit-Exactness | Every `v0.3.0` guide and snapping mutation is Undo/Redo bit-exact via `CommandHistory`, and `grafia-egui` (`painter_pass.rs`) and `grafia-headless` (`headless.rs`) emit 1-to-1 matching Pass 1 grid & alignment zones and Pass 4 guides, selected ticks, locks (body + shackle), measurement pills, magnetic snappers, and alignment badges. |
| **`TC-V030-008`** | Canvas Grid (Main Grid, Subdivisions, Sloped Grid, Zoom Thresholds & `show_grid`) | Verifies `grid_main_color` (`#80736666`) and `grid_sub_color` (`#40595940`) tokens; zoom threshold suppression (`1 UPM` at `zoom = 1.0` hidden, visible at `zoom = 12.0 > 8.0`); modular grid (`grid_step_upm = 50.0`, `grid_subdivisions = 5`) rendering both main (`0.5 dp`) and sub (`0.4 dp`) lines and snapping to `effective_grid_step() == 10.0`; sloped grid (`grid_slope_rise`, `grid_slope_run`) shearing vertical lines with `0.7 dp`/`0.6 dp` strokes; `show_grid = false` hiding grid. |
| **`TC-V030-009`** | 3-Way `AppearanceMode` (`Light` / `Dark` / `System`) & Dark Mode Rendering Parity | Verifies `AppearanceMode::{Light, Dark, System}` resolution (`set_appearance_mode`, `set_system_dark_mode`, `GrafiaTheme::for_appearance`), preservation across `load_glyph_archetype`, `GrafiaCanvasWidget::from_state(&mut state)` & `GrafiaCanvasWidget::new` honoring `state.theme` and painting the `#1E1E1E` / `#FFFFFF` canvas background rect in `egui`, `ShapeLocatorWidget` dark mode contrast, and 1-to-1 Dark Mode rendering across `grafia-egui` and `grafia-headless` without grid/guide bleed into Sidebar/InfoBox (`frame_47_v030_dark_mode_full_canvas.png`). |

---

## 4. Dark Mode Theme Tokens (`GrafiaTheme::sonoma_dark()`) & 3-Way Host Appearance Switch (`AppearanceMode { Light, Dark, System }`)

### 4.1 Normative Dark Mode Color Tokens

| Token / Layer | Semantic Key | Light Mode (`sonoma_default`) | Dark Mode (`sonoma_dark`) | Chromatic Specification |
| :--- | :--- | :--- | :--- | :--- |
| **Canvas Background** | `canvas_background` | `#FAFAFA` (`#FFFFFF` raw) | `#1E1E1E` (`rgba(30, 30, 30, 255)`) | Dark neutral workspace surface |
| **Sidebar (`Shapes`) Background** | `sidebar_background` | `#F2F2F7` | `#262626` (`rgba(38, 38, 38, 255)`) | Secondary panel surface |
| **InfoBox / Pill Background** | `hud_background` | `#FFFFFF` | `#2C2C2E` (`rgba(44, 44, 46, 255)`) | Elevated floating HUD surface |
| **Splitter / Divider** | `divider_stroke` | `#D1D1D6` | `#38383A` (`rgba(56, 56, 58, 255)`) | Hairline separator |
| **Foreground Stroke & Fill** | `foreground_stroke` | `#1D1D1F` (`#000000` raw) | `#FFFFFF` (`rgba(255, 255, 255, 255)`) | High-contrast outline & fill |
| **Corner Node (`OnCurve` Corner)** | `node_corner` | `#007AFF` (`#4D83CB` raw) | `#5693E0` (`rgba(86, 147, 224, 255)`) | `H=0.60, S=0.62, B=0.88` |
| **Smooth Node (`OnCurve` Smooth)** | `node_smooth` | `#34C759` (`#48B879` raw) | `#52D189` (`rgba(82, 209, 137, 255)`) | `H=0.38, S=0.61, B=0.82` |
| **Off-Curve Handle (`BCP`)** | `node_offcurve` | `#8E8E93` | `#AC8E68` (`rgba(172, 142, 104, 255)`) | Warm ochre off-curve handle fill |
| **Handle Line (`BCP` Tangent)** | `handle_line` | `#C7C7CC` | `#636366` (`rgba(99, 99, 102, 255)`) | Dark canvas tangent contrast |
| **Selected Node Glow** | `selected_glow` | `#007AFF66` | `#0A84FF80` (`rgba(10, 132, 255, 128)`) | Accent selection halo |
| **Rubberband Fill / Stroke** | `marquee_selection` | `#007AFF1A` / `#007AFF` | `#0A84FF26` / `#0A84FF` | Translucent selection marquee |
| **Metrics Guide Line & Label** | `metrics_line` | `#D6D6D6` / `#8E8E93` | `#555558` / `#98989D` (`#CEA77C` raw) | Metric frame & margin label |
| **Alignment Zone Fill** | `alignment_zone_fill` | `#CEA77C33` | `#CEA77C33` (`rgba(206, 167, 124, 51)`) | Identical in Light & Dark |
| **Local Guide Line** | `local_guide_color` | `#3E4CCFB3` | `#3E4CCFB3` (`rgba(62, 76, 207, 179)`) | Identical in Light & Dark |
| **Global Guide Line** | `global_guide_color` | `#CF4F3EB3` | `#CF4F3EB3` (`rgba(207, 79, 62, 179)`) | Identical in Light & Dark |
| **Main Grid & Sub Grid** | `grid_main` / `grid_sub` | `#80736666` / `#40595940` | `#80736666` / `#40595940` | Identical RGBA in Light & Dark |

### 4.2 3-Way Host Appearance Switch (`AppearanceMode`)
To enable the host application to drive `grafia`'s appearance via a single 3-way preference switch without coupling `grafia`'s toolbar UI:
```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum AppearanceMode {
    #[default]
    Light,
    Dark,
    System,
}
```
1. **`AppearanceMode::Light`** (Explicit User Choice — Light): Always resolves to `GrafiaTheme::sonoma_default()` (`is_dark() == false`).
2. **`AppearanceMode::Dark`** (Explicit User Choice — Dark): Always resolves to `GrafiaTheme::sonoma_dark()` (`is_dark() == true`).
3. **`AppearanceMode::System`** (System Preference): Resolves to `GrafiaTheme::sonoma_dark()` when `system_dark_mode == true` (or `ui.ctx().style().visuals.dark_mode == true` in `grafia-egui`) and to `GrafiaTheme::sonoma_default()` when `system_dark_mode == false`, automatically updating `state.theme` whenever the OS/host system appearance changes.
4. **Full Pass 1–4 Dark Mode Parity**: Every rendering pass (`v0.1.0` metrics, silhouette fill, wireframe/tangents, nodes, preview strip, shape sidebar; `v0.2.0` knife line, pen ghost segment, bounding box pills, measurement line pills; `v0.3.0` canvas grid, alignment zones, guides, guide locks, measurement pills, magnetic snappers, and alignment badges) respects `state.theme` in both `grafia-egui` (`painter_pass.rs`) and `grafia-headless` (`headless.rs`).
