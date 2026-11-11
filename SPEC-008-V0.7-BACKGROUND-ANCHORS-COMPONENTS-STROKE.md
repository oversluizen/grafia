---
type: specification
id: SPEC-008
version: 0.7.0
status: normative
parent: SPEC-001
---

# SPEC-008: `grafia v0.7.0` — Background Layer, 2D Anchors, Pre-Resolved Components, Fit Curve, Live Contour Stroke & Master Compatibility

> **Normative References**:
> - `SPEC-001`: `grafia v0.1.0` — Architecture, Coordinate Pipeline & Token System
> - `SPEC-002`: `grafia v0.2.0` — Bézier Topology, Selection & Transform Box
> - `SPEC-004`: `grafia v0.3.0` — Guides, Snap Engine & Alignment Zones
> - `SPEC-005`: `grafia v0.4.0` — Vector Operations, Knife, Annotations & Pixel Tool
> - `SPEC-006`: `grafia v0.5.0` — Multi-Glyph Horizontal Text Stream & Kerning
> - `SPEC-007`: `grafia v0.6.0` — Eraser, TrueType Hinting Overlays & Smart Plumb Lines
> - Standards: `ISO/IEC/IEEE 42010:2011` (Architecture Description), `ISO/IEC/IEEE 29119-3:2021` (Test Documentation), `IEEE 1012-2016` (System & Software Verification and Validation), `ISO/IEC 14496-22:2019` (`OpenType 1.9` — Composite `glyf` / `GPOS` Anchor Attachment & `fvar`/`gvar` Variation Interpolation), `SemVer 2.0.0`

---

## 1. Normative Mathematical & Typographic Foundations (CAGD & OpenType 1.9)

All geometric rules, keyboard shortcuts, affine transformations, and rendering invariants in `SPEC-008` are grounded in Computer-Aided Geometric Design (CAGD) and `ISO/IEC 14496-22:2019` (`OpenType 1.9`):

| Subsystem | Domain Standard / Mathematical Model | Core Invariant / Equation | Verified Canvas Semantics |
| :--- | :--- | :--- | :--- |
| **1. Dual-Buffer Background Layer (`Cmd+B`, `Cmd+J`, `Cmd+K`)** | Non-Destructive Reference Contour Buffering | $C_{\text{bg}} \leftarrow \text{deep\_copy}(C_{\text{fg,sel}})$ | Per-layer reference contour container supporting selective or full copy (`Cmd+J`), append (`Cmd+Opt+J`), atomic foreground/background swap (`Cmd+K`), active editing target switch (`Cmd+B`), visibility toggle (`Cmd+Opt+B`), and Pass-2 hairline wireframe rendering (`1.0 / zoom`). |
| **2. 2D `CanvasAnchor` & Mark Preview Cloud** | `OpenType 1.9` `GPOS` Mark-to-Base (`LookupType 4`) & Mark-to-Mark (`LookupType 6`) | $P_{\text{mark}}' = P_{\text{mark}} + (A_{\text{base}} - A_{\text{mark}})$ | Named 2D attachment point in UPM coordinates rendered in Pass 4 as a rotated diamond handle (`4.5 dp` unselected, `6.0 dp` selected), supporting `Tab`/`Shift+Tab` cycling, `SnapEngine` dragging, deletion, and translucent Pass-2 mark-cloud ghost contour alignment. |
| **3. 2D `CanvasComponent` & Decompose (`Cmd+Shift+D`)** | `OpenType 1.9` Composite `glyf` $2 \times 3$ Affine Matrix | $p' = \begin{bmatrix} a & c \\ b & d \end{bmatrix} p + \begin{bmatrix} t_x \\ t_y \end{bmatrix}$ | Pre-resolved 2D component reference with lock/alignment guards, double-click source navigation request (`CanvasHostRequest::OpenComponentSource`), and atomic decomposition (`Cmd+Shift+D`) that reverses contour node order when $\det(A) = ad - bc < 0$ to preserve winding parity. |
| **4. 8-Step Fit Curve (`Ctrl+Opt+1..8`, `57%..92%`)** | CAGD Unbounded Tangent Ray Intersection & Handle Normalization | $t_k = t_{\min} + (t_{\max} - t_{\min})\frac{k}{7}$, $H_1' = P_0 + t(T - P_0)$ | Places cubic off-curve handles at fraction $t \in [0.57, 0.92]$ (`5%` steps) along the tangent legs toward ray intersection apex $T$, guarded against parallel tangents and exploding chords ($\|T - P_0\|_2 \ge 2\|P_3 - P_0\|_2$), plus $\pm 12.5\%$ span stepping and arithmetic-mean handle ratio normalization. |
| **5. Live Contour Stroke & Offset Expansion** | CAGD Parallel / Normal Offset Curve $C_{\pm}(t) = B(t) \pm d\,\hat{n}(t)$ | $(d_{\text{L}}, d_{\text{R}}) \in \{(\frac{w}{2}, -\frac{w}{2}), (w, 0), (0, -w)\}$ | Non-destructive live stroke attribute (`width`, `height`, `StrokePosition`, `LineCapStyle`, `LineJoinStyle`) generating closed offset contours for open spines and concentric opposite-winding rings for closed spines, plus destructive `expand_selected_strokes()`. |
| **6. Companion Master Layers, Master Compatibility (`Ctrl+Opt+Cmd+N`) & `SelectAllLayers`** | `OpenType 1.9` `gvar` Point-Number & Contour-Order Interpolation Isomorphism | $\forall i, j:\; \text{type}(P_{i,j}^{(A)}) = \text{type}(P_{i,j}^{(B)})$ | Verifies 1-to-1 contour count, closed topology, node count,segment-type sequence, and anchor set parity across companion master layers, renders origin (`index 0`) and anchor rays (`Ctrl+Opt+Cmd+N`), and supports simultaneous multi-layer selection and translation (`SelectAllLayers`). |

---

## 2. Normative Functional & Architectural Requirements

### 2.1 Background Layer (`Cmd+B`, `Cmd+J`, `Cmd+K` — `src/core/background_layer.rs`)

1. **Dual-Buffer Layer Topology**:
   - In addition to the primary foreground contours (`state.glyph.contours`), `GlifoCanvasState` maintains a background contour buffer (`state.background_contours: Vec<GlyphContour>`) and two independent boolean flags:
     - `show_background: bool` (default `true`): controls whether inactive background contours are rendered as a reference wireframe in Pass 2.
     - `editing_background: bool` (default `false`): indicates whether the user is currently editing the background layer instead of the foreground layer.
2. **Selection to Background (`Cmd+J` / `Cmd+Opt+J` — `copy_selection_to_background(keep_old: bool)`)**:
   - Dispatched via `Cmd+J` (`keep_old = false`) or `Cmd+Opt+J` (`keep_old = true`):
     - If one or more foreground nodes are selected (`!state.selected_nodes.is_empty()`), every foreground contour containing at least one selected node is deep-copied (with deterministic fresh `ContourId` and `NodeId` values) into `state.background_contours`.
     - If no foreground nodes are selected (`state.selected_nodes.is_empty()`), **all** foreground contours (`state.glyph.contours`) are deep-copied into `state.background_contours`.
     - When `keep_old == false`, existing `state.background_contours` are replaced; when `keep_old == true`, the copied contours are appended.
     - The operation records an atomic Undo snapshot before mutating `state.background_contours`.
3. **Swap Foreground with Background (`Cmd+K` — `swap_foreground_with_background()`)**:
   - Dispatched via `Cmd+K`:
     - Atomically exchanges `state.glyph.contours` with `state.background_contours`.
     - Clears `state.selected_nodes` and `state.selected_segments` so no stale node references remain across buffers.
     - Records a single atomic Undo snapshot (`Undo` + `Redo` bit-exact restoration).
4. **Toggle Background Editing / Visibility (`Cmd+B` — `toggle_background_editing()`)**:
   - Dispatched via `Cmd+B`:
     - Toggles `state.editing_background` between `false` (Foreground active) and `true` (Background active) by swapping the active editable contour buffer with the reference buffer while preserving `show_background = true`.
     - Clears active node/segment selection on mode switch.
     - `Cmd+Opt+B` (`toggle_show_background()`) toggles `state.show_background` without switching the active editing target.
5. **Pass-2 Reference Wireframe Rendering**:
   - When `state.show_background == true` and the reference buffer is non-empty, its contours are stroked in Pass 2 with a hairline width of `1.0 / viewport.zoom` (`BACKGROUND_STROKE_WIDTH_DP = 1.0`) using semantic theme token `theme.background_layer_stroke` (Light mode: rose-burgundy `rgba(179, 77, 102, 166)`; Dark mode: warm rose `rgba(217, 115, 140, 166)`), without node handles.

---

### 2.2 2D `CanvasAnchor` Primitives & Mark Cloud Ghost Paths (`src/core/anchors_components.rs`)

1. **Strict Architectural Boundary (Font-Database-Agnostic Canvas)**:
   - `grafia` is strictly a 2D vector canvas and interaction engine; it has zero dependency on Unicode character databases, glyph naming tables, or font-wide anchor recipes.
   - Anchor recipe generation (`Cmd+U` *Set Anchors*) and resolving which diacritic marks belong to a Mark Preview Cloud (`top`, `bottom`, `ogonek`, etc.) are the exclusive responsibility of the **Host Font Model Backend**, which supplies `anchors: Vec<CanvasAnchor>` and `ghost_paths: Vec<GhostMarkPath>` to `GlifoCanvasState`.
2. **2D `CanvasAnchor` Data Model & Diamond Handle Geometry**:
   - Each `CanvasAnchor` stores `id: AnchorId`, `name: String`, `position: FontPoint`, and `selected: bool`.
   - Rendered in Pass 4 as a 4-vertex rotated diamond (rhombus) centered at `anchor.position` with half-diagonal radius:
     - Unselected: `ANCHOR_DIAMOND_RADIUS_DP = 4.5` screen dp (`4.5 / viewport.zoom` in UPM).
     - Selected: `ANCHOR_DIAMOND_SELECTED_RADIUS_DP = 6.0` screen dp (`6.0 / viewport.zoom` in UPM).
   - Fill & stroke tokens: `theme.anchor_fill` (Light: `rgba(230, 115, 38, 230)`, Dark: `rgba(245, 140, 64, 240)`) and `theme.anchor_selected_fill` (`rgba(255, 69, 0, 255)`).
3. **Anchor Hit-Testing, `Tab` Cycling, Snapped Dragging & Deletion**:
   - Clicking within `ANCHOR_HIT_RADIUS_DP = 7.0` screen dp selects the hit `CanvasAnchor` (clearing node selection unless `Shift` is held).
   - Pressing `Tab` while one or more `CanvasAnchor`s are selected (and no contour nodes are selected) cycles selection deterministically to the next anchor in `state.anchors` (`Shift+Tab` cycles backward).
   - Dragging a selected anchor translates `anchor.position` through `SnapEngine` (`SPEC-004`), snapping to metrics lines, guides, and contour nodes/bounds (`Ctrl` suspends snapping), and commits an atomic Undo snapshot on pointer release.
   - Pressing `Delete` / `Backspace` with selected anchor(s) removes them from `state.anchors` inside an atomic Undo transaction.
4. **Mark Preview Cloud (`ghost_paths`)**:
   - When `state.show_mark_cloud == true` and at least one `CanvasAnchor` is selected, every `GhostMarkPath` in `state.ghost_paths` whose `anchor_name` matches a selected anchor's `name` is translated so its attachment origin coincides with `anchor.position` and rendered in Pass 2 with `theme.mark_cloud_fill` (Light: `rgba(102, 115, 140, 64)`, Dark: `rgba(166, 179, 204, 64)`).

---

### 2.3 2D Pre-Resolved `CanvasComponent` Primitives & `Decompose` (`src/core/anchors_components.rs`)

1. **Strict Architectural Boundary (Pre-Resolved 2D Component Shapes)**:
   - Component Picker modal dialogs (`Cmd+Shift+C`), cross-font Component DAG traversal, and cross-glyph Automatic Alignment rules reside in the **Host Application / Consumer Shell** and **Host Font Model Backend**.
   - The Host supplies pre-resolved `CanvasComponent` primitives (`id`, `component_name`, `contours: Vec<GlyphContour>`, `transform: Affine2D`, `locked: bool`, `automatic_alignment: bool`, `selected: bool`) to `grafia`.
2. **2D Affine Transformation (`Affine2D`) & Winding Parity**:
   - Each `CanvasComponent` carries a 2D affine matrix $[a, b, c, d, t_x, t_y]$ mapping local component UPM coordinates $(x, y)$ to layer UPM coordinates:
     $$\begin{bmatrix} x' \\ y' \end{bmatrix} = \begin{bmatrix} a & c \\ b & d \end{bmatrix} \begin{bmatrix} x \\ y \end{bmatrix} + \begin{bmatrix} t_x \\ t_y \end{bmatrix}$$
   - Determinant $\det(A) = a \cdot d - b \cdot c$: when $\det(A) < 0.0$ (reflection / negative scale), transformed contour winding is inverted during decomposition so counter/outer path direction remains valid.
3. **Pass-2 Tinted Component Rendering, Selection, Dragging & Double-Click Host Request**:
   - Pre-resolved components are rendered in Pass 2 using `theme.component_fill` (unselected) or `theme.component_selected_fill` (selected), or `theme.component_locked_fill` when `locked == true`.
   - Clicking inside the transformed component silhouette (even-odd / non-zero winding test) selects the component when `locked == false`.
   - Dragging a selected component translates `(t_x, t_y)` by $(\Delta x, \Delta y)$ when `locked == false` and `automatic_alignment == false` (when `automatic_alignment == true`, horizontal/vertical translation is locked unless alignment is disabled).
   - Double-clicking a component emits `CanvasHostRequest::OpenComponentSource { id, component_name }` in `state.host_requests` so the Host Application can navigate to or open the referenced source glyph.
4. **Decompose Selected Components (`Cmd+Shift+D` — `decompose_selected_components()`)**:
   - For every selected `CanvasComponent` with `locked == false`:
     1. Applies `component.transform` to every `Node` (`NodeType::Corner`, `NodeType::Smooth`, `NodeType::OffCurve`) in `component.contours`.
     2. If $\det(\text{transform}) < 0.0$, reverses each decomposed contour's node order while preserving start-node cubic topology.
     3. Assigns fresh, unique `ContourId` and `NodeId` values and appends the resulting `GlyphContour`s to `state.glyph.contours`.
   - Removes all decomposed components from `state.components`, selects all newly created contour nodes, and records a single atomic Undo snapshot.

---

### 2.4 Fit Curve (`Ctrl+Opt+1..8`, `57%..92%`) & Handle Normalization (`src/core/fit_and_stroke.rs`)

1. **8-Step Tangent Percentage Ladder**:
   - Given `FitCurveConfig { min_ratio, max_ratio }` with default `min_ratio = 0.57` (`57%`) and `max_ratio = 0.92` (`92%`), column index $k \in \{0, 1, \dots, 7\}$ (triggered by keyboard shortcuts `Ctrl+Opt+1` through `Ctrl+Opt+8`) evaluates to:
     $$t_k = \text{min\_ratio} + (\text{max\_ratio} - \text{min\_ratio}) \cdot \frac{k}{7.0}$$
   - For the default `[0.57, 0.92]` configuration, the 8 steps are exact multiples of `0.05` (`5%`):
     $$t_0 = 0.57,\; t_1 = 0.62,\; t_2 = 0.67,\; t_3 = 0.72,\; t_4 = 0.77,\; t_5 = 0.82,\; t_6 = 0.87,\; t_7 = 0.92$$
   - Custom limit validation enforces $0.0 \le \text{min\_ratio} \le 1.0$, $\text{max\_ratio} \le 1.5$, and $\text{max\_ratio} \ge \text{min\_ratio} + 0.125$.
2. **Unbounded Tangent Intersection & Handle Placement**:
   - For every cubic Bézier segment $(P_0, H_1, H_2, P_3)$ in `state.glyph.contours` where at least one of $\{P_0, H_1, H_2, P_3\}$ is in `state.selected_nodes`:
     - Let $\vec{u} = H_1 - P_0$ and $\vec{v} = H_2 - P_3$ be the outgoing tangent direction vectors from $P_0$ and $P_3$.
     - Compute the unbounded line-line intersection $T$ of ray $P_0 + s \vec{u}$ and ray $P_3 + r \vec{v}$:
       $$\Delta = u_x (-v_y) - u_y (-v_x) = v_x u_y - u_x v_y$$
     - **Safety Guards**:
       1. If $|\Delta| < 10^{-7}$ (parallel or collinear tangents), or if the intersection lies behind either tangent ray ($s \le 0$ or $r \le 0$), leave $(H_1, H_2)$ unchanged.
       2. If $\|T - P_0\|_2 \ge 2.0 \cdot \|P_3 - P_0\|_2$ (`FIT_CURVE_MAX_INTERSECTION_CHORD_MULTIPLE = 2.0`), leave $(H_1, H_2)$ unchanged to prevent handle explosion on near-inflection curves.
     - Otherwise place both off-curve handles at fraction $t$ along their tangent legs toward $T$:
       $$H_1' = P_0 + t \cdot (T - P_0), \qquad H_2' = P_3 + t \cdot (T - P_3)$$
3. **Make Shorter / Make Longer & Handle Ratio Normalization**:
   - `fit_curve_step_delta(longer: bool)` shifts each selected cubic segment's current handle ratios $(t_1, t_2)$ by $\Delta t = \pm 0.125 \cdot (\text{max\_ratio} - \text{min\_ratio})$, clamped to $[\text{min\_ratio}, \text{max\_ratio}]$.
   - `normalize_selected_cubic_handles()` computes the current tangent leg ratios $t_1 = \|H_1 - P_0\|_2 / \|T - P_0\|_2$ and $t_2 = \|H_2 - P_3\|_2 / \|T - P_3\|_2$, sets both handles to their arithmetic mean $\bar{t} = 0.5 \cdot (t_1 + t_2)$, and records an atomic Undo snapshot.

---

### 2.5 Live Contour Stroke & Offset (`src/core/fit_and_stroke.rs`)

1. **Per-Contour Live Stroke Attributes**:
   - Each `GlyphContour` (or `state.contour_strokes: BTreeMap<ContourId, ContourStrokeStyle>`) can carry a `ContourStrokeStyle`:
     - `width: f64` ($\ge 0.0$ UPM; `0.0` means standard unstroked filled/wireframe contour).
     - `height: f64` ($\ge 0.0$ UPM; defaults to `width` for isotropic stroke, or distinct for calligraphic nib height).
     - `position: StrokePosition` (`Center = 0`, `InsideLeft = 1`, `OutsideRight = 2`).
     - `cap_start: LineCapStyle` (`Butt`, `Round`, `Square`).
     - `cap_end: LineCapStyle` (`Butt`, `Round`, `Square`).
     - `join: LineJoinStyle` (`Miter`, `Round`, `Bevel`).
2. **Non-Destructive Live Stroke Preview**:
   - When a contour has `stroke.width > 0.0`:
     - Let $(d_{\text{left}}, d_{\text{right}})$ be the left/right normal offset distances in UPM:
       - `StrokePosition::Center`: $(+w / 2, -w / 2)$
       - `StrokePosition::InsideLeft`: $(+w, 0.0)$
       - `StrokePosition::OutsideRight`: $(0.0, -w)$
     - For an **open** spine contour ($N \ge 2$ on-curve nodes), `compute_expanded_stroke_contours` builds a single **closed** outline contour wrapping the left offset curve forward, the end cap (`Butt`, `Round`, or `Square`), the right offset curve backward, and the start cap.
     - For a **closed** spine contour, `compute_expanded_stroke_contours` builds **two** closed concentric contours (outer boundary and inner counter with opposite winding directions) when both offset distances are non-degenerate.
   - In Pass 2 and `PreviewFilled` mode, the expanded stroke outlines are tessellated and rendered live while the underlying spine contour remains directly editable with `SelectTool` / `PenTool`.
3. **Destructive Expand Outline (`expand_selected_strokes()`)**:
   - Replaces every selected contour (or all stroked contours if no nodes are selected) that has `stroke.width > 0.0` with the closed `GlyphContour`(s) produced by `compute_expanded_stroke_contours`, removes their entries from `state.contour_strokes`, and records an atomic Undo snapshot.

---

### 2.6 `CompanionMasterLayer`, Master Compatibility View (`Ctrl+Opt+Cmd+N`) & `SelectAllLayers` Multi-Layer Editing (`src/core/master_compatibility.rs`)

1. **Strict Architectural Boundary (2D Companion Master Layers on Canvas)**:
   - Designspace axis management, variable font instance generation, and cross-font master tables reside in the **Host Font Model Backend**.
   - The Host supplies zero or more `CompanionMasterLayer` records (`id: u32`, `name: String`, `color_rgba: [u8; 4]`, `contours: Vec<Vec<Node>>`, `closed_flags: Vec<bool>`, `anchors: Vec<CanvasAnchor>`, `editable: bool`) to `GlifoCanvasState` so multiple font masters (`Regular`, `Bold`, `Condensed`, etc.) can be overlaid, verified for interpolation compatibility, and edited simultaneously on the 2D canvas.
2. **Master Compatibility Evaluation (`compute_master_compatibility(&self) -> MasterCompatibilityReport`)**:
   - Toggled for Pass-4 overlay rendering via keyboard shortcut `Ctrl+Opt+Cmd+N` (`toggle_show_master_compatibility()`, setting `state.show_master_compatibility: bool`).
   - Compares the active foreground layer (`state.glyph.contours` and `state.anchors`) against each `CompanionMasterLayer` in `state.companion_master_layers`:
     - **Contour Count Check**: `active_contours.len() == companion.contours.len()`. If unequal, any excess or missing contour index $i$ is recorded in `incompatible_contour_indices` and `is_compatible = false`.
     - **Per-Contour Topology Check ($0 \le i < \min(N_{\text{active}}, N_{\text{comp}})$)**:
       1. `active_contours[i].closed == companion.closed_flags.get(i).copied().unwrap_or(false)`.
       2. `active_contours[i].nodes.len() == companion.contours[i].len()`.
       3. Exact node-type / segment-type sequence parity (`NodeType::Corner` / `NodeType::Smooth` on-curve vs. `NodeType::OffCurve` cubic sequence) at every index $j \in 0..n$.
       4. When contour $i$ is compatible (or has at least 1 node in both layers), records a start-node (`index 0`) connecting ray `(active_contours[i].nodes[0].point, companion.contours[i][0].point)` in `start_node_vectors`. When contour $i$ fails any check (1)–(3), index $i$ is added to `incompatible_contour_indices` and `is_compatible = false`.
     - **Anchor Compatibility & Connecting Vectors**:
       - If the set of anchor names in `companion.anchors` differs from `state.anchors`, `is_compatible = false`.
       - For every anchor name present in both `state.anchors` and `companion.anchors`, records the connecting ray `(active_anchor.position, companion_anchor.position)` in `anchor_vectors`.
3. **`SelectAllLayers` Multi-Layer Node Selection & Simultaneous Editing (`select_all_layers_mode: bool`)**:
   - When `state.select_all_layers_mode == true`:
     - Clicking or marquee-selecting in UPM space selects matching nodes in `state.glyph.contours` **and** any node inside an `editable == true` `CompanionMasterLayer` whose position falls within the hit radius or marquee rectangle (`selected_companion_nodes: BTreeSet<(u32, NodeId)>`).
     - Translating or nudging the active selection (`translate_selected_across_all_layers(delta: FontPoint)`) moves all selected foreground nodes **and** all selected nodes in every `editable == true` `CompanionMasterLayer` (`editable == false` companion layers remain untouched), recording an atomic Undo snapshot that restores both foreground and companion master layer nodes.

---

## 3. Rust Data Structures & Public API Contracts

```rust
/// Unique identifier for a 2D canvas anchor.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, serde::Serialize, serde::Deserialize)]
pub struct AnchorId(pub u64);

/// 2D anchor primitive rendered in Pass 4.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct CanvasAnchor {
    pub id: AnchorId,
    pub name: String,
    pub position: FontPoint,
    pub selected: bool,
}

/// Host-supplied translucent mark-preview outline.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct GhostMarkPath {
    pub anchor_name: String,
    pub contours: Vec<GlyphContour>,
}

/// Unique identifier for a 2D pre-resolved component.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, serde::Serialize, serde::Deserialize)]
pub struct ComponentId(pub u64);

/// 2D affine transformation matrix `[a, b, c, d, tx, ty]` in UPM space.
#[derive(Debug, Clone, Copy, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Affine2D {
    pub a: f64,
    pub b: f64,
    pub c: f64,
    pub d: f64,
    pub tx: f64,
    pub ty: f64,
}

/// Pre-resolved 2D component reference.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct CanvasComponent {
    pub id: ComponentId,
    pub component_name: String,
    pub contours: Vec<GlyphContour>,
    pub transform: Affine2D,
    pub locked: bool,
    pub automatic_alignment: bool,
    pub selected: bool,
}

/// Outbound notifications emitted by `grafia` to the Host Application / Consumer Shell.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum CanvasHostRequest {
    OpenComponentSource {
        id: ComponentId,
        component_name: String,
    },
}

/// Configuration for the 8-step Fit Curve ladder.
#[derive(Debug, Clone, Copy, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct FitCurveConfig {
    pub min_ratio: f64, // Default: 0.57 (57%)
    pub max_ratio: f64, // Default: 0.92 (92%)
}

/// Stroke alignment relative to the spine contour.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum StrokePosition {
    Center,
    InsideLeft,
    OutsideRight,
}

/// Endpoint cap style for open stroked contours.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum LineCapStyle {
    Butt,
    Round,
    Square,
}

/// Corner join style for stroked contours.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum LineJoinStyle {
    Miter,
    Round,
    Bevel,
}

/// Live stroke parameters attached to a contour.
#[derive(Debug, Clone, Copy, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct ContourStrokeStyle {
    pub width: f64,
    pub height: f64,
    pub position: StrokePosition,
    pub cap_start: LineCapStyle,
    pub cap_end: LineCapStyle,
    pub join: LineJoinStyle,
}

/// Companion font master layer rendered and optionally edited on canvas.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct CompanionMasterLayer {
    pub id: u32,
    pub name: String,
    pub color_rgba: [u8; 4],
    pub contours: Vec<Vec<Node>>,
    pub closed_flags: Vec<bool>,
    pub anchors: Vec<CanvasAnchor>,
    pub editable: bool,
}

/// Result of `compute_master_compatibility`.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct MasterCompatibilityReport {
    pub is_compatible: bool,
    pub start_node_vectors: Vec<(FontPoint, FontPoint)>,
    pub anchor_vectors: Vec<(FontPoint, FontPoint)>,
    pub incompatible_contour_indices: Vec<usize>,
}

pub const BACKGROUND_STROKE_WIDTH_DP: f32 = 1.0;
pub const ANCHOR_DIAMOND_RADIUS_DP: f32 = 4.5;
pub const ANCHOR_DIAMOND_SELECTED_RADIUS_DP: f32 = 6.0;
pub const ANCHOR_HIT_RADIUS_DP: f32 = 7.0;
pub const FIT_CURVE_DEFAULT_MIN_RATIO: f64 = 0.57;
pub const FIT_CURVE_DEFAULT_MAX_RATIO: f64 = 0.92;
pub const FIT_CURVE_MAX_INTERSECTION_CHORD_MULTIPLE: f64 = 2.0;

impl Affine2D {
    pub const fn identity() -> Self;
    pub const fn translation(tx: f64, ty: f64) -> Self;
    pub fn determinant(&self) -> f64;
    pub fn transform_point(&self, pt: FontPoint) -> FontPoint;
}

impl FitCurveConfig {
    pub fn ratio_for_column(&self, col: usize) -> f64;
}

impl GlifoCanvasState {
    // 2.1 Background Layer (Cmd+B, Cmd+J, Cmd+K, Cmd+Opt+B)
    pub fn copy_selection_to_background(&mut self, keep_old: bool);
    pub fn swap_foreground_with_background(&mut self);
    pub fn toggle_background_editing(&mut self);
    pub fn toggle_show_background(&mut self);

    // 2.2 2D CanvasAnchor & Mark Preview Cloud
    pub fn set_anchors(&mut self, anchors: Vec<CanvasAnchor>);
    pub fn set_ghost_mark_paths(&mut self, paths: Vec<GhostMarkPath>);
    pub fn select_anchor_at_upm(&mut self, pt: FontPoint, extend: bool) -> bool;
    pub fn cycle_selected_anchor(&mut self, reverse: bool);
    pub fn drag_selected_anchors_to_upm(&mut self, pt: FontPoint, bypass_snap: bool);
    pub fn delete_selected_anchors(&mut self);
    pub fn compute_active_mark_cloud_contours(&self) -> Vec<GlyphContour>;

    // 2.3 2D Pre-Resolved CanvasComponent & Decompose (Cmd+Shift+D)
    pub fn set_components(&mut self, components: Vec<CanvasComponent>);
    pub fn select_component_at_upm(&mut self, pt: FontPoint, extend: bool) -> bool;
    pub fn translate_selected_components(&mut self, delta: FontPoint);
    pub fn double_click_component_at_upm(&mut self, pt: FontPoint) -> bool;
    pub fn decompose_selected_components(&mut self);
    pub fn drain_host_requests(&mut self) -> Vec<CanvasHostRequest>;

    // 2.4 Fit Curve (Ctrl+Opt+1..8) & Handle Normalization
    pub fn apply_fit_curve_column(&mut self, col: usize);
    pub fn fit_selected_curves(&mut self, ratio: f64);
    pub fn fit_curve_step_delta(&mut self, longer: bool);
    pub fn normalize_selected_cubic_handles(&mut self);

    // 2.5 Live Contour Stroke & Destructive Expand Outline
    pub fn set_contour_stroke(&mut self, contour_id: ContourId, style: ContourStrokeStyle);
    pub fn compute_expanded_stroke_contours(&self) -> Vec<GlyphContour>;
    pub fn expand_selected_strokes(&mut self);

    // 2.6 CompanionMasterLayer, Master Compatibility (Ctrl+Opt+Cmd+N) & SelectAllLayers
    pub fn set_companion_master_layers(&mut self, layers: Vec<CompanionMasterLayer>);
    pub fn toggle_show_master_compatibility(&mut self);
    pub fn compute_master_compatibility(&self) -> MasterCompatibilityReport;
    pub fn select_nodes_in_rect_all_layers(&mut self, min: FontPoint, max: FontPoint);
    pub fn translate_selected_across_all_layers(&mut self, delta: FontPoint);
}
```

---

## 4. Complete Normative Test Matrix (`TC-V070-001` .. `TC-V070-017`)

All 17 test cases are implemented under `#![cfg(feature = "v070_red")]`:

| Test Case ID | Requirement / Mathematical Contract | Input / Action | Expected Normative Outcome |
| :--- | :--- | :--- | :--- |
| **`TC-V070-001`** | `Cmd+J` (`copy_selection_to_background`) — Partial Selection & Append (`Cmd+Opt+J`) | Select 1 of 2 foreground contours and dispatch `Cmd+J` (`keep_old = false`), then `Cmd+Opt+J` (`keep_old = true`). | Only the selected contour is deep-copied into `background_contours` with fresh IDs on `Cmd+J`; `Cmd+Opt+J` appends while preserving prior background contours. |
| **`TC-V070-002`** | `Cmd+J` (`copy_selection_to_background`) — Empty Selection | Clear selection and dispatch `Cmd+J`. | All foreground contours are copied into `background_contours`; `Undo` restores prior background state bit-exactly. |
| **`TC-V070-003`** | `Cmd+K` (`swap_foreground_with_background`) | Populate distinct foreground and background contours; dispatch `Cmd+K`. | Foreground and background contour buffers swap atomically; selection clears; `Undo`/`Redo` round-trips bit-exactly. |
| **`TC-V070-004`** | `Cmd+B` (`toggle_background_editing`) & `Cmd+Opt+B` (`toggle_show_background`) | Dispatch `Cmd+B`, translate a node in the active background layer, then press `Cmd+B` again. | `editing_background` toggles to `true`, edits mutate the background contour buffer while foreground serves as reference, and second `Cmd+B` returns to foreground. |
| **`TC-V070-005`** | `CanvasAnchor` Hit-Testing, Selection & `Tab` / `Shift+Tab` Cycling | Add `"top"`, `"bottom"`, `"ogonek"` anchors; click `"top"`, press `Tab` twice, then `Shift+Tab`. | Clicking within `7.0 dp` selects `"top"`; `Tab` cycles `"top" -> "bottom" -> "ogonek"`; `Shift+Tab` returns to `"bottom"`. |
| **`TC-V070-006`** | `CanvasAnchor` Snapped Dragging & `Delete` with Undo | Drag selected `"top"` anchor near a vertical guide at `x = 300`, release, then press `Delete` and `Undo`. | Anchor snaps to `x = 300.0`; `Delete` removes the selected anchor; `Undo` restores the anchor at `x = 300.0`. |
| **`TC-V070-007`** | Mark Preview Cloud (`ghost_paths` — `compute_active_mark_cloud_contours`) | Register `GhostMarkPath` for `"top"`, select `"top"` anchor at `(300, 720)`, enable `show_mark_cloud`. | `compute_active_mark_cloud_contours()` returns the ghost contour translated by `(300, 720)` and renders in Pass 2 only while `"top"` is selected. |
| **`TC-V070-008`** | `CanvasComponent` Selection, Locked Guard & Automatic Alignment Lock | Add unlocked unaligned, unlocked auto-aligned, and locked components; click and drag each. | Unlocked unaligned component selects and translates `(tx, ty)`; `locked == true` rejects hit-selection; `automatic_alignment == true` prevents manual translation until alignment is disabled. |
| **`TC-V070-009`** | `CanvasComponent` Double-Click Host Notification (`OpenComponentSource`) | Double-click inside a `CanvasComponent` (`"acutecomb"`). | Emits `CanvasHostRequest::OpenComponentSource { id, component_name: "acutecomb" }` in `drain_host_requests()`. |
| **`TC-V070-010`** | `Cmd+Shift+D` Decompose Components & Reflection Winding Fix (`decompose_selected_components`) | Select two components (one translated, one horizontally mirrored with $\det(A) < 0$) and dispatch `Cmd+Shift+D`. | Both components convert into editable `GlyphContour`s in `state.glyph.contours` with fresh IDs; mirrored component reverses node order to preserve winding sign; components are removed from `state.components`; `Undo` restores both components. |
| **`TC-V070-011`** | Fit Curve 8-Step Ladder (`Ctrl+Opt+1..8` — `57%..92%`) | Parameterized test across $k \in 0..8$ (`Ctrl+Opt+1`..`8`) on a quarter-circle cubic segment with $P_0=(0,0)$, $P_3=(100,100)$, $T=(0,100)$. | Sets $H_1 = (0, 100 \cdot t_k)$ and $H_2 = (100 \cdot (1 - t_k), 100)$ for $t_k \in \{0.57, 0.62, 0.67, 0.72, 0.77, 0.82, 0.87, 0.92\}$ within $10^{-4}$ UPM. |
| **`TC-V070-012`** | Fit Curve Parallel/Exploding Tangent Guard, Shorter/Longer (`fit_curve_step_delta`) & Handle Normalization (`normalize_selected_cubic_handles`) | Test parallel tangents, $\|T - P_0\| \ge 2 \|P_3 - P_0\|$, `fit_curve_step_delta`, and asymmetric handle normalization $(t_1=0.60, t_2=0.80)$. | Degenerate/exploding segments remain untouched; `fit_curve_step_delta` shifts by $\pm 0.125 \cdot (0.92 - 0.57)$; `normalize_selected_cubic_handles` equalizes both ratios to $\bar{t} = 0.70$. |
| **`TC-V070-013`** | Live Contour Stroke Offset on Open & Closed Contours (`compute_expanded_stroke_contours`) | Attach `ContourStrokeStyle { width: 40.0, position: Center, cap_start: Butt/Round/Square, .. }` to an open line and a closed rectangle. | Open contour expands into 1 closed contour of width `40.0` UPM (with increased length under `Square`/`Round` caps); closed contour expands into 2 concentric contours with opposite signed areas. |
| **`TC-V070-014`** | Destructive Expand Outline (`expand_selected_strokes`) with Atomic Undo | Call `expand_selected_strokes()` on a stroked spine contour, then `Undo` and `Redo`. | Spine contour is replaced by its expanded closed outline contour(s) and stroke attribute is cleared; `Undo` restores the original spine and `ContourStrokeStyle`. |
| **`TC-V070-015`** | Headless Pass-2 & Pass-4 Light/Dark Rendering Parity | Render canvas with active background wireframe, selected `CanvasAnchor` + `GhostMarkPath`, tinted `CanvasComponent`, and live stroked contour in Light & Dark modes. | Both modes rasterize non-empty deterministic pixel buffers with distinct Light vs. Dark theme colors and zero panics. |
| **`TC-V070-016`** | Stateful Pull-Based `arbtest` Multi-Tool Fuzzing (`Background + Anchors + Components + FitCurve + Stroke + Undo/Redo`) | Run 250 ms pull-based `arbtest` action sequences mixing `Cmd+J`, `Cmd+K`, `Cmd+B`, anchor drag/delete, component decompose, Fit Curve `Ctrl+Opt+1..8`, stroke expansion, and `Undo`/`Redo`. | `state.validate_invariants()` holds after every step and `Action -> Undo -> Redo` preserves bit-exact canvas state. |
| **`TC-V070-017`** | `CompanionMasterLayer`, Master Compatibility (`Ctrl+Opt+Cmd+N`) & `SelectAllLayers` Multi-Layer Editing | Register a compatible editable `Bold` `CompanionMasterLayer` and a non-editable `Condensed` layer; toggle `Ctrl+Opt+Cmd+N`, verify `compute_master_compatibility()` on compatible vs. segment-mismatched contours, then enable `select_all_layers_mode` and marquee-select + translate across layers with Undo. | Compatible layers report `is_compatible == true` with start-node (`index 0`) and anchor vectors; mutating a segment type or node count flags the contour index in `incompatible_contour_indices`; `select_all_layers_mode` translates selected nodes in both active and editable `CompanionMasterLayer`s while leaving non-editable companion layers fixed, and `Undo` restores all layers bit-exactly. |
