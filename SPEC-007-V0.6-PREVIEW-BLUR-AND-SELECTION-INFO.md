---
type: specification
id: SPEC-007
title: "grafia v0.6.0 — Preview Strip Engine with Gaussian Blur, Headless SelectionInfo Contract, Rulers, Background Layer & Anchors"
version: 0.6.0
status: normative
date: 2026-11-11
standards:
  - "ISO/IEC/IEEE 42010:2011 (Architecture Description)"
  - "ISO/IEC/IEEE 29119-3:2021 (Test Documentation)"
  - "IEEE 1012-2016 (System and Software Verification and Validation)"
  - "OpenType 1.9 / CAGD / ISO 9241-9"
  - "SemVer 2.0.0"
baseline: "Normative Mathematical & Ergonomic Baseline (OpenType 1.9 / CAGD / ISO 9241-9)"
parent: SPEC-001
---

# SPEC-007: `grafia v0.6.0` — Preview Strip Engine with Gaussian Blur, Headless `SelectionInfo` Contract, Canvas Rulers, Background Layer & Anchors

> **Normative Verification Mandate (`ISO/IEC/IEEE 29119-3:2021` & `IEEE 1012-2016`):** Every constant, filter parameter, coordinate precision, transform-grid anchor mapping, and geometric algorithm in this specification is anchored in separable 2D Gaussian signal processing, CAGD Bézier tangent invariants, OpenType 1.9 metrics, and ISO 9241-9 interaction ergonomics. No production code for `v0.6.0` is written before this specification and the failing RED COTS test suite (`tests/v060_complete_tests.rs`) are anchored.
>
> **Architectural Scope Boundary (`ISO/IEC/IEEE 42010:2011`):** `grafia` provides a headless, deterministic `SelectionInfo` query and mutation contract covering strictly **local 2D geometry** (`lsb`, `rsb`, `advance_width`, `center_glyph_in_width`, `SingleOnCurveInfo`, `SingleOffCurveInfo`, `MultiNodeInfo`, and `GuideInfo`). Cross-glyph **Metrics Keys** (such as `=H`, `=n+10`, `=|n`, `=|`) require looking up other glyphs across a font-wide database and therefore belong exclusively in the **Host Font Model Backend** (rendered by the **Host Application / Consumer Shell**), never inside `grafia`.

---

## 1. Normative Mathematical & Ergonomic Baseline (OpenType 1.9 / CAGD / ISO 9241-9)

All mathematical invariants, filter parameters, and selection inspection rules for `v0.6.0` are specified below:

| Subsystem | Normative Domain & Shortcut | Mathematical & Ergonomic Specification |
| :--- | :--- | :--- |
| **Preview Strip & Separable Gaussian Blur (`0.0..20.0` px)** | Offscreen / Split-View Proofing Strip (`PreviewRenderConfig`) | Default vertical spacing multiplier `extra_spacing = 1.25`. Blur radius `blur_radius_px` lies in $[0.0, 20.0]\text{ px}$, default preset `4.5`. When `blur_radius_px > 0.1`, applies a separable 2-pass 1D Gaussian convolution with $\sigma = \text{blur\_radius\_px}$ and kernel radius $R = \lceil 3\sigma \rceil$; otherwise returns sharp unblurred output. Vertical scaling computes $\text{ascender} = \text{default\_ascender} \cdot 1.25$, $\text{descender} = \|\text{default\_descender} \cdot 1.25\|$, $\text{scale}_0 = H_{\text{safe}} / (\text{ascender} + \text{descender})$, scaled by `0.8` if one of `more_space_above`/`below` is true or `0.7` if both are true. When `flipped == true`, translates Y by $H_{\text{safe}} - y_{\text{baseline}}$ and applies vertical mirror `scale(scale_x, -scale_y)`. When `inverted == true`, swaps foreground/background to light-on-dark polarity. |
| **`SelectionInfo` Mode Switcher** | Contextual 5-Mode Geometric Inspection (`selection_info()`) | Evaluates active selection state and deterministically returns the matching `SelectionInfo` variant: `GlyphBase` (0 nodes/guides selected), `SingleOnCurve` (1 `OnCurve` node), `SingleOffCurve` (1 `OffCurve` handle), `MultiNode` ($\ge 2$ selected nodes), or `Guide` (selected local or global guide). |
| **Mode 1: `GlyphBaseInfo` (Local Glyph Metrics)** | Local 2D Sidebearing, Bounding Box & Advance Width Inspection | Exposes pure 2D geometry (`glyph_name`, `bbox_x`, `bbox_y`, `bbox_w`, `bbox_h`, `lsb`, `rsb`, `advance_width`, `kern_with_prev`, `kern_with_next`, `transform_anchor`, `proportional_scale`, `contour_count`, `total_node_count`). Local 2D metrics (`lsb`, `rsb`, `advance_width`, and `center_glyph_in_width`) are computed and mutated directly on the active glyph layer, whereas Unicode tables, cross-glyph Metrics Key expressions (`=H`, `=n+10`, `=|n`, `=|`), and kerning groups reside exclusively in the **Host Font Model Backend**. |
| **Mode 2: `SingleOnCurveInfo` (Single `OnCurve` Node)** | Coupled Anchor & Handle Translation | Reports `x` and `y` with 3-decimal precision. Mutating `x` or `y` computes $\Delta = \text{new\_coord} - \text{old\_coord}$, translates the `OnCurve` node, translates any immediately adjacent `OffCurve` BCP handles by the same $\Delta$, and enforces smooth tangent collinearity on neighboring smooth nodes. |
| **Mode 3: `SingleOffCurveInfo` (Single `OffCurve` Handle)** | Relative Handle Vector $(\Delta x, \Delta y)$ & Radial Length $L$ | Resolves the tethered `OnCurve` anchor node (`next` node if `OnCurve`, else `prev` node). Computes `delta_x = off.x - anchor.x` (precision `3`), `delta_y = off.y - anchor.y` (precision `3`), and radial handle length $\text{length} = \text{hypot}(\Delta x, \Delta y)$ (precision `1`). Mutating `length` computes the unit vector $\vec{u} = (\text{off} - \text{anchor}) / \text{hypot}(\text{off} - \text{anchor})$, sets $\text{off.position} = \text{anchor} + \vec{u} \cdot \text{new\_length}$, and maintains opposite smooth handle collinearity. |
| **Mode 4: `MultiNodeInfo` & 9-Point `TransformGridAnchor`** | Selection AABB & 9-Point Pin Scaling (`0..8`) | Populates `selected_node_count` (`■`) and `total_layer_node_count` (`□`, formatting $> 9999$ as `"%.1fK"`). Computes selection AABB $(\min X, \min Y, \max X, \max Y)$ and resolves reference origin $(\text{origin\_x}, \text{origin\_y})$ on the 3×3 grid (`0..8`, default center `4`, $\text{row} \cdot 3 + \text{col}$). Mutating `width` or `height` scales all selected nodes around $(\text{origin\_x}, \text{origin\_y})$ via $\text{translate}(\text{origin}) \circ \text{scale}(s_x, s_y) \circ \text{translate}(-\text{origin})$, honoring `proportional_scale`. |
| **Mode 5: `GuideInfo` (Selected Guide)** | Guide Origin, Angle, Alignment, Lock & Measurement | Supports horizontal alignment modes `0 -> Lsb` ($x = 0$), `1 -> Center` ($x = 0.5 \cdot \text{advance\_width}$), `2 -> Rsb` ($x = \text{advance\_width}$), plus `x`, `y`, `angle_deg`, `show_measurement`, and `lock_angle` toggles. |
| **Canvas Rulers (`Cmd+R`), Background Layer (`Cmd+B`/`Cmd+J`/`Cmd+K`) & Anchors (`Cmd+U`)** | Viewport Rulers, Background Layer, Background Projection & Default Anchors Host Port | `Cmd+R` toggles 20 dp top horizontal and left vertical UPM rulers with zoom-adaptive `1–2–5` tick steps (`compute_ruler_tick_steps_upm(zoom)`) and live cursor hairlines; dragging from the top/left ruler onto the canvas spawns a horizontal ($0^\circ$) or vertical ($90^\circ$) local `Guide` (or global `Guide` when `Alt`/`Option` is held). `Cmd+B` switches editing between Foreground and Background contours; `Cmd+J` copies selected foreground contours (or all foreground contours if none selected) into the background layer; `Cmd+K` (and `Cmd+Shift+J`) swaps foreground and background contours; `align_selection_to_background()` projects selected foreground `OnCurve` nodes (with coupled adjacent `OffCurve` handles) onto the nearest point on `background_contours` (`Line` & `Cubic`). `Cmd+U` emits `CanvasHostRequest::ResetDefaultAnchors { glyph_name }` for the **Host Font Model Backend**, which supplies computed anchors via `set_anchors_with_undo`. |
| **Pure 2D `SelectionInfoAction` Port & 5-Mode Grey Info Box HUD** | Pure 2D View-Model & Controller Action Port (`show_selection_info_hud`) | `SelectionInfo` and `SelectionInfoAction` expose 100% pure 2D geometry (`SetLsb`, `SetRsb`, `SetAdvanceWidth`, `CenterInWidth`, `SetOnCurveX/Y`, `ToggleOnCurveSmooth`, `SetOffCurveX/Y`, `SetOffCurveDeltaX/Y`, `SetOffCurveLength`, `SetMultiAnchor`, `SetMultiOriginX/Y`, `SetMultiWidth/Height`, `SetMultiProportional`, `SetGuideX/Y`, `SetGuideAngle`, `SetGuideAlignment`, `SetGuideLockAngle`, `SetGuideShowMeasurement`, plus Anchor and Component actions) with zero font-database pass-through state. The bottom-center Grey Info Box HUD renders all 5 `SelectionInfo` modes in both `egui` and `tiny-skia`, including an interactive 9-dot 3×3 `TransformGridAnchor` matrix in `MultiNode` mode. |

---

## 2. Normative Requirements & Subsystems (`grafia v0.6.0`)

### 2.1 Preview Strip Engine with Gaussian Blur

1. **Configuration Contract (`PreviewRenderConfig`):**
   - `blur_radius_px: f32`: clamped to $[0.0, 20.0]\text{ px}$ (`PREVIEW_BLUR_MIN_RADIUS_PX = 0.0`, `PREVIEW_BLUR_MAX_RADIUS_PX = 20.0`, default slider preset `PREVIEW_BLUR_DEFAULT_RADIUS_PX = 4.5` when toggled on).
   - `flipped: bool`: vertical mirror toggle. When `true`, flips the rendered glyphs upside-down via `scale(1.0, -1.0)` around the vertical center of the preview safe area.
   - `inverted: bool`: negative polarity toggle. When `true`, swaps foreground and background colors (`[235, 235, 240, 255]` white glyphs on `[24, 24, 26, 255]` black background).
   - `extra_spacing: f32`: vertical spacing multiplier (default `1.25`).
   - `more_space_above: bool` & `more_space_below: bool`: when one is `true`, multiplies base scale by `0.8`; when both are `true`, multiplies base scale by `0.7`.
2. **Vertical Scaling Formula:**
   - Effective preview ascender: $A_{\text{prev}} = \text{metrics.ascender} \cdot \text{extra\_spacing}$.
   - Effective preview descender: $D_{\text{prev}} = |\text{metrics.descender} \cdot \text{extra\_spacing}|$.
   - Base scale:
     $$S_0 = \frac{H_{\text{safe}}}{A_{\text{prev}} + D_{\text{prev}}}$$
   - Space factor: $f_{\text{space}} = 0.7$ if `more_space_above && more_space_below`, $0.8$ if `more_space_above ^ more_space_below`, else $1.0$.
   - Final preview scale: $S = S_0 \cdot f_{\text{space}}$.
3. **Separable 2-Pass 1D Gaussian Blur:**
   - Active threshold: when `blur_radius_px <= PREVIEW_BLUR_ACTIVE_THRESHOLD_PX` (`0.1` px), the rasterized RGBA buffer is returned sharp without convolution.
   - When `blur_radius_px > 0.1`:
     - Applies a deterministic, separable 2-pass (horizontal then vertical) 1D Gaussian convolution with standard deviation $\sigma = \text{blur\_radius\_px}$ and kernel radius $R = \lceil 3\sigma \rceil$:
       $$w(k) = \frac{1}{\sqrt{2\pi}\sigma}\exp\!\left(-\frac{k^2}{2\sigma^2}\right), \qquad k \in [-R, R]$$
       normalized such that $\sum_{k=-R}^{R} w(k) = 1.0$, with clamp-to-edge boundary handling.
4. **Battery-Efficient Dirty-State Caching (`PreviewStripCache` & `content_generation()`):**
   - `GrafiaCanvasState::content_generation(&self) -> u64` maintains a monotonically increasing content revision counter incremented whenever geometry, text stream slots, anchors, components, live strokes, companion master layers, or light/dark appearance change, while remaining constant during idle pointer hover, viewport pan/zoom, and node selection changes.
   - `PreviewStripCache` caches the rasterized RGBA buffer and optional `egui::TextureHandle` keyed by `(state.content_generation(), state.theme.is_dark(), cfg)` so `render_preview_strip_rgba` and GPU texture upload (`TextureHandle::set`) execute only on cache miss (0.0 ms CPU and 0 GPU uploads on cache hit).

---

### 2.2 Headless `SelectionInfo` Query & Local 2D Mutation Contract

1. **Architectural Separation (`ISO/IEC/IEEE 42010:2011`):**
   - `GrafiaCanvasState::selection_info(&self) -> SelectionInfo` computes a pure, allocation-light value enum describing the current canvas selection so any **Host Application / Consumer Shell** can render a floating bottom-right Inspector box or sidebar inspector without duplicating geometric math.
   - Cross-glyph **Metrics Keys** (`=H`, `=n+10`, `=|n`, `=|`) require looking up other glyphs across the font-wide database and therefore reside exclusively in the **Host Font Model Backend**, never inside `grafia`.
2. **Five Normative `SelectionInfo` Variants & Mutators:**
   - **Variant 1 — `SelectionInfo::GlyphBase(GlyphBaseInfo)` (No node or guide selected):**
     - Reports strictly local 2D metrics of the active glyph layer:
       - `glyph_name: String`
       - `lsb: f64` (distance from $x = 0$ to contour minimum $x$, or $0.0$ if glyph has no contours; italic-compensated when `italic_angle_deg != 0.0`)
       - `rsb: f64` (`advance_width - contour_max_x`, or `advance_width` if empty)
       - `advance_width: f64`
       - `kern_with_prev: Option<i32>` and `kern_with_next: Option<i32>` (when in a multi-glyph `CanvasTextStream`)
     - Local 2D mutators on `GrafiaCanvasState` (each recording an Undo snapshot):
       - `set_lsb(&mut self, new_lsb: f64)`: shifts all contours, anchors, and local guides horizontally by `new_lsb - current_lsb` and updates `advance_width += new_lsb - current_lsb` so `rsb` stays invariant.
       - `set_rsb(&mut self, new_rsb: f64)`: keeps contours fixed and updates `advance_width = contour_max_x + new_rsb`.
       - `set_advance_width(&mut self, new_width: f64)`: updates `advance_width = new_width.max(0.0)` while keeping contours and `lsb` fixed.
       - `center_glyph_in_width(&mut self)` (local geometric equivalent of `=|` centering): computes $M = \text{lsb} + \text{rsb} = \text{advance\_width} - (\text{max\_x} - \text{min\_x})$ and translates all contours/anchors horizontally so $\text{lsb}' = \text{rsb}' = \tfrac{1}{2}M$ while `advance_width` remains constant.
   - **Variant 2 — `SelectionInfo::SingleOnCurve(SingleOnCurveInfo)` (1 `OnCurve` node selected):**
     - Reports `node_id: NodeId`, `x: f64`, `y: f64`, `node_type: NodeType`.
     - Mutator `set_selected_oncurve_position(&mut self, new_pos: FontPoint)`:
       - Computes $\Delta = \text{new\_pos} - \text{old\_pos}$.
       - Translates the selected `OnCurve` node to `new_pos` AND translates its immediately adjacent `OffCurve` BCP handles on the same contour by $\Delta$, preserving smooth tangent collinearity on neighboring smooth nodes.
   - **Variant 3 — `SelectionInfo::SingleOffCurve(SingleOffCurveInfo)` (1 `OffCurve` BCP handle selected):**
     - Resolves the tethered `OnCurve` anchor node (`next` node if `OnCurve`, else `prev` node on the contour).
     - Reports:
       - `node_id: NodeId`, `anchor_node_id: NodeId`
       - `x: f64`, `y: f64` (absolute UPM coordinates)
       - `delta_x: f64` (`x - anchor.x`), `delta_y: f64` (`y - anchor.y`)
       - `length: f64` ($\sqrt{\Delta x^2 + \Delta y^2}$)
     - Mutators:
       - `set_selected_offcurve_position(&mut self, new_pos: FontPoint)` / `set_selected_offcurve_delta(&mut self, dx: f64, dy: f64)`: moves the handle to `new_pos` (or `anchor + (dx, dy)`) and enforces opposite smooth handle collinearity if `anchor` is `NodeType::Smooth`.
       - `set_selected_handle_length(&mut self, new_length: f64)`: scales the handle radially along its existing angle from `anchor`:
         $$\vec{u} = \frac{\vec{p}_{\text{off}} - \vec{p}_{\text{anchor}}}{\|\vec{p}_{\text{off}} - \vec{p}_{\text{anchor}}\|}, \qquad \vec{p}_{\text{off}}' = \vec{p}_{\text{anchor}} + \max(\text{new\_length}, 0.0)\cdot\vec{u}$$
         (when current length is $0.0$, defaults $\vec{u} = (1.0, 0.0)$).
   - **Variant 4 — `SelectionInfo::MultiNode(MultiNodeInfo)` ($\ge 2$ nodes selected):**
     - Reports:
       - `origin_anchor: TransformGridAnchor` (9-point 3×3 pin grid `0..8`, default `Center` = `4`)
       - `origin_x: f64`, `origin_y: f64` (resolved on the selection AABB: Left/CenterX/Right $\times$ Bottom/CenterY/Top)
       - `width: f64` (`max_x - min_x`), `height: f64` (`max_y - min_y`)
       - `proportional_scale: bool`
       - `selected_node_count: usize` (`■`)
       - `total_layer_node_count: usize` (`□`)
     - Mutators:
       - `set_multi_selection_origin(&mut self, new_origin: FontPoint)`: translates all selected nodes (and coupled BCPs) by `new_origin - current_origin`.
       - `set_multi_selection_size(&mut self, new_width: Option<f64>, new_height: Option<f64>)`: scales all selected nodes around `(origin_x, origin_y)`. When `proportional_scale == true`, scaling width by $s_x = W'/W$ also sets $s_y = s_x$ (and vice versa).
   - **Variant 5 — `SelectionInfo::Guide(GuideInfo)` (Selected local or global `Guide`):**
     - Reports:
       - `guide_ref: SelectedGuideRef`
       - `x: f64`, `y: f64`, `angle_deg: f64`
       - `alignment: GuideAlignment` (`Lsb`, `Center`, `Rsb`)
       - `lock_angle: bool`
       - `show_measurement: bool`
     - Mutators:
       - `set_selected_guide_origin(&mut self, pos: FontPoint)`, `set_selected_guide_angle(&mut self, angle_deg: f64)`, `set_selected_guide_alignment(&mut self, alignment: GuideAlignment)` (updates `guide.origin.x` to `0.0` for `Lsb`, `advance_width * 0.5` for `Center`, or `advance_width` for `Rsb`), `set_selected_guide_lock_angle(&mut self, locked: bool)`, `set_selected_guide_measurement(&mut self, show: bool)`.

---

### 2.3 Canvas Rulers (`Cmd+R`), Background Layer (`Cmd+B` / `Cmd+J` / `Cmd+K` / `Cmd+Shift+J`) & Default Anchors (`Cmd+U`)

1. **Canvas Rulers (`Cmd+R` — *View → Show Rulers*):**
   - Toggled via `Cmd+R` (`Ctrl+R` on Windows/Linux) or `state.show_rulers: bool`.
   - When `state.show_rulers == true`:
     - Renders a `RULER_THICKNESS_DP = 20.0` horizontal ruler along the top edge and a `20.0 dp` vertical ruler along the left edge of the canvas viewport, with zoom-adaptive major/minor UPM tick marks, monospaced UPM coordinate labels, and a `20.0 × 20.0 dp` corner origin box.
     - Draws live cursor position hairlines inside both rulers tracking `state.hover_upm`.
     - **Drag-from-Ruler Guide Creation:**
       - Pressing and dragging downward from the top horizontal ruler into the canvas spawns a horizontal ($0^\circ$) `Guide` tracking the cursor's $y_{\text{upm}}$ coordinate (local guide into `state.guides` by default; global guide into `state.global_guides` when `modifiers.alt` is held) and commits it with Undo on `PointerUp`.
       - Pressing and dragging rightward from the left vertical ruler into the canvas spawns a vertical ($90^\circ$) `Guide` tracking the cursor's $x_{\text{upm}}$ coordinate (local into `state.guides` by default; global into `state.global_guides` with `modifiers.alt`) and commits it with Undo on `PointerUp`.
2. **Background Layer Editing (`Cmd+B`, `Cmd+J`, `Cmd+K` / `Cmd+Shift+J`):**
   - `Glyph` / `GrafiaCanvasState` maintains `background_contours: Vec<GlyphContour>` (rendered behind the foreground layer in `theme.background_layer_stroke` when `state.show_background_layer == true`).
   - **`Cmd+B` (`toggle_editing_background_layer`):** switches active editing between the Foreground layer and the Background layer (preserving Undo/Redo invariants).
   - **`Cmd+J` (`selection_to_background`):** copies the currently selected foreground contours (or all foreground contours when no nodes are selected) into `background_contours` with Undo.
   - **`Cmd+K` / `Cmd+Shift+J` (`swap_foreground_and_background`):** atomically swaps the foreground `contours` and `background_contours` with Undo.
3. **Default Anchor Request Port (`Cmd+U` — `reset_default_anchors` & `set_anchors_with_undo`):**
   - Full font-wide Unicode/glyph-database Anchor Recipes reside exclusively in the **Host Font Model Backend**, never inside `grafia`.
   - Pressing `Cmd+U` (`Ctrl+U` on Windows/Linux) or calling `state.reset_default_anchors()` pushes `CanvasHostRequest::ResetDefaultAnchors { glyph_name: self.glyph.name.clone() }` into `state.host_requests` (returned by `state.drain_host_requests()`).
   - The **Host Font Model Backend** computes the anchor recipe for `glyph_name` and injects it via `state.set_anchors_with_undo(anchors)`, which records an atomic Undo snapshot, updates `state.anchors` and `state.glyph.anchors`, and clears active anchor selection.

---

## 3. Light & Dark Appearance Tokens (`src/tokens/v060_tokens.rs`)

All `v0.6.0` visual overlays support both `GrafiaTheme::sonoma_default()` (Light) and `GrafiaTheme::sonoma_dark()` (Dark) with zero hardcoded RGBA literals outside `src/tokens/`:

| Token Name | Light Mode (`sonoma_default`) | Dark Mode (`sonoma_dark`) | Semantic Role |
| :--- | :--- | :--- | :--- |
| `preview_bg_normal` | `[250, 248, 245, 255]` | `[30, 30, 32, 255]` | Preview strip normal background |
| `preview_fg_normal` | `[24, 24, 26, 255]` | `[235, 235, 240, 255]` | Preview strip normal glyph fill |
| `preview_bg_inverted` | `[24, 24, 26, 255]` | `[250, 248, 245, 255]` | Preview strip inverted background |
| `preview_fg_inverted` | `[235, 235, 240, 255]` | `[24, 24, 26, 255]` | Preview strip inverted glyph fill |
| `ruler_bg_fill` | `[242, 240, 236, 245]` | `[38, 38, 42, 245]` | Canvas ruler bar background fill |
| `ruler_border_stroke` | `[205, 203, 198, 255]` | `[62, 62, 68, 255]` | Canvas ruler divider hairline |
| `ruler_tick_color` | `[115, 115, 122, 255]` | `[165, 165, 175, 255]` | Canvas ruler hash marks & labels |
| `ruler_hairline_color` | `[46, 117, 219, 220]` | `[74, 144, 242, 230]` | Live cursor marker inside rulers |
| `background_layer_stroke` | `[185, 115, 115, 170]` | `[215, 130, 130, 175]` | Background layer contour hairline stroke |

---

## 4. Rust Data Structures & API Contracts

```rust
/// Preview blur slider bounds and defaults.
pub const PREVIEW_BLUR_MIN_RADIUS_PX: f32 = 0.0;
pub const PREVIEW_BLUR_MAX_RADIUS_PX: f32 = 20.0;
pub const PREVIEW_BLUR_DEFAULT_RADIUS_PX: f32 = 4.5;
pub const PREVIEW_BLUR_ACTIVE_THRESHOLD_PX: f32 = 0.1;

/// Default vertical spacing multiplier.
pub const PREVIEW_DEFAULT_EXTRA_SPACING: f64 = 1.25;
pub const PREVIEW_ONE_EXTRA_SPACE_SCALE: f64 = 0.8;
pub const PREVIEW_BOTH_EXTRA_SPACE_SCALE: f64 = 0.7;

/// Canvas ruler bar thickness in screen dp (`Cmd+R`).
pub const RULER_THICKNESS_DP: f32 = 20.0;

/// Configuration for the preview strip renderer (`v0.6.0`).
#[derive(Clone, Debug, PartialEq)]
pub struct PreviewRenderConfig {
    pub width_px: u32,
    pub height_px: u32,
    /// Gaussian blur sigma in pixels (`0.0..=20.0`, active when `> 0.1`).
    pub blur_radius_px: f32,
    /// Vertical mirror toggle (`scale(1.0, -1.0)`).
    pub flipped: bool,
    /// Negative white-on-black polarity toggle.
    pub inverted: bool,
    /// Extra vertical spacing multiplier (default `1.25`).
    pub extra_spacing: f64,
    pub more_space_above: bool,
    pub more_space_below: bool,
}

/// Guide horizontal alignment mode.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GuideAlignment {
    Lsb,
    Center,
    Rsb,
}

/// Mode 1: Active glyph base local 2D metrics and bounding box (100% pure 2D geometry).
#[derive(Clone, Debug, PartialEq)]
pub struct GlyphBaseInfo {
    pub glyph_name: String,
    pub bbox_x: f64,
    pub bbox_y: f64,
    pub bbox_w: f64,
    pub bbox_h: f64,
    pub lsb: f64,
    pub rsb: f64,
    pub advance_width: f64,
    pub kern_with_prev: Option<i32>,
    pub kern_with_next: Option<i32>,
    pub transform_anchor: TransformGridAnchor,
    pub proportional_scale: bool,
    pub contour_count: usize,
    pub total_node_count: usize,
}

/// Mode 2: Single selected `OnCurve` node.
#[derive(Clone, Debug, PartialEq)]
pub struct SingleOnCurveInfo {
    pub node_id: NodeId,
    pub x: f64,
    pub y: f64,
    pub node_type: NodeType,
}

/// Mode 3: Single selected `OffCurve` BCP handle.
#[derive(Clone, Debug, PartialEq)]
pub struct SingleOffCurveInfo {
    pub node_id: NodeId,
    pub anchor_node_id: NodeId,
    pub x: f64,
    pub y: f64,
    pub delta_x: f64,
    pub delta_y: f64,
    pub length: f64,
}

/// Mode 4: Multiple selected nodes.
#[derive(Clone, Debug, PartialEq)]
pub struct MultiNodeInfo {
    pub origin_anchor: TransformGridAnchor,
    pub origin_x: f64,
    pub origin_y: f64,
    pub width: f64,
    pub height: f64,
    pub proportional_scale: bool,
    /// `■` selected node count.
    pub selected_node_count: usize,
    /// `□` total node count in the active layer.
    pub total_layer_node_count: usize,
}

/// Mode 5: Selected local or global guide.
#[derive(Clone, Debug, PartialEq)]
pub struct GuideInfo {
    pub guide_ref: SelectedGuideRef,
    pub x: f64,
    pub y: f64,
    pub angle_deg: f64,
    pub alignment: GuideAlignment,
    pub lock_angle: bool,
    pub show_measurement: bool,
}

/// Headless 5-mode selection inspection contract.
#[derive(Clone, Debug, PartialEq)]
pub enum SelectionInfo {
    GlyphBase(GlyphBaseInfo),
    SingleOnCurve(SingleOnCurveInfo),
    SingleOffCurve(SingleOffCurveInfo),
    MultiNode(MultiNodeInfo),
    Guide(GuideInfo),
}
```

---

## 5. Normative COTS Verification Matrix (`tests/v060_complete_tests.rs`)

| Test ID | Requirement Under Test | Expected Outcome |
| :--- | :--- | :--- |
| **`TC-V060-001`** | Preview strip vertical scaling formula (`extra_spacing = 1.25`, `more_space_above/below` `0.8` & `0.7`) | Preview scale equals `safe_height / ((ascender + |descender|) * 1.25)` multiplied by `1.0`, `0.8` (one extra-space flag), or `0.7` (both extra-space flags). |
| **`TC-V060-002`** | Preview strip separable 2-pass Gaussian blur (`blur_radius_px` $\in [0.0, 20.0]$, default `4.5`, threshold `> 0.1`) | Rendering with `blur_radius_px = 0.0` (or `0.05`) produces sharp unblurred edges; rendering with `blur_radius_px = 4.5` or `12.0` produces smooth, monotonically diffused anti-aliased luminance profiles across stem boundaries. |
| **`TC-V060-003`** | Preview strip vertical flip (`scale(1.0, -1.0)`) & negative white-on-black polarity | Enabling `flipped = true` mirrors the rendered preview buffer vertically around the safe-area midline; enabling `inverted = true` renders light glyphs on a dark background (`preview_bg_inverted`). |
| **`TC-V060-004`** | `SelectionInfo::GlyphBase` local 2D `lsb`, `rsb`, `advance_width`, and `center_glyph_in_width()` | With zero nodes/guides selected, `selection_info()` returns `SelectionInfo::GlyphBase` with exact `lsb`, `rsb`, and `advance_width`; calling `set_lsb`, `set_rsb`, `set_advance_width`, or `center_glyph_in_width()` mutates local geometry and records Undo. |
| **`TC-V060-005`** | `SelectionInfo::SingleOnCurve` & coupled BCP translation | Selecting 1 `OnCurve` node returns `SelectionInfo::SingleOnCurve`; calling `set_selected_oncurve_position` translates both the `OnCurve` node and its adjacent `OffCurve` handles by the exact same $(\Delta x, \Delta y)$ delta while preserving smooth tangent collinearity. |
| **`TC-V060-006`** | `SelectionInfo::SingleOffCurve` relative $(\Delta x, \Delta y)$ and Euclidean `length = hypot(dx, dy)` | Selecting 1 `OffCurve` handle returns `SelectionInfo::SingleOffCurve` with its tethered `anchor_node_id`, exact `delta_x = x - anchor.x`, `delta_y = y - anchor.y`, and `length = hypot(delta_x, delta_y)`. |
| **`TC-V060-007`** | `SingleOffCurve` radial handle length mutation (`set_selected_handle_length`) | Calling `set_selected_handle_length(new_len)` scales the selected `OffCurve` handle radially along its existing angle from its tethered `OnCurve` anchor without altering the handle's angle, and maintains smooth opposite handle collinearity. |
| **`TC-V060-008`** | `SelectionInfo::MultiNode` & 9-point `TransformGridAnchor` (`0..8`, center `4`) | Selecting $\ge 2$ nodes returns `SelectionInfo::MultiNode` with `selected_node_count` (`■`), `total_layer_node_count` (`□`), selection AABB `(width, height)`, and `origin_x/y` matching the active 9-point `TransformGridAnchor`. |
| **`TC-V060-009`** | `MultiNode` bounding-box origin shift & proportional/non-proportional `width`/`height` scaling | Calling `set_multi_selection_origin` shifts all selected nodes; calling `set_multi_selection_size` scales selected nodes around the chosen 9-point anchor (scaling both axes uniformly when `proportional_scale == true`). |
| **`TC-V060-010`** | `SelectionInfo::Guide` & `GuideAlignment::{Lsb, Center, Rsb}` | Selecting a guide returns `SelectionInfo::Guide`; mutating `GuideAlignment` snaps the guide's $x$ coordinate to `0.0` (`Lsb`), `advance_width * 0.5` (`Center`), or `advance_width` (`Rsb`), and toggles `lock_angle` / `show_measurement` with Undo. |
| **`TC-V060-011`** | Canvas Rulers (`Cmd+R`) 20 dp top/left bars, live cursor hairlines, and drag-from-ruler to spawn local or `Option` global `Guide` | Pressing `Cmd+R` toggles `show_rulers`; dragging from the top ruler $(y < 20\text{ dp})$ into the canvas creates a horizontal ($0^\circ$) guide; dragging from the left ruler $(x < 20\text{ dp})$ creates a vertical ($90^\circ$) guide (local by default, global with `Alt`). |
| **`TC-V060-012`** | Background Layer toggling (`Cmd+B`) & independent background contour editing | Pressing `Cmd+B` switches editing to `background_contours` so tool mutations edit the background layer while foreground contours remain untouched; pressing `Cmd+B` again returns to the foreground layer. |
| **`TC-V060-013`** | Selection to Background (`Cmd+J`) & Swap Foreground with Background (`Cmd+K` / `Cmd+Shift+J`) | Pressing `Cmd+J` copies selected foreground contours (or all foreground contours if none selected) into `background_contours`; pressing `Cmd+K` (or `Cmd+Shift+J`) atomically swaps `contours` and `background_contours` with bit-exact Undo/Redo. |
| **`TC-V060-014`** | Default Anchor Request Port (`Cmd+U` — `reset_default_anchors` & `set_anchors_with_undo`) | Pressing `Cmd+U` on `/A` pushes `CanvasHostRequest::ResetDefaultAnchors { glyph_name: "A" }` into `drain_host_requests()`; injecting the computed anchors via `set_anchors_with_undo` updates `anchors` with bit-exact Undo/Redo. |
| **`TC-V060-015`** | Automatic `SelectionInfo` mode transitions across canvas interactions | Clicking empty canvas (`GlyphBase`) $\to$ clicking 1 `OnCurve` node (`SingleOnCurve`) $\to$ clicking 1 `OffCurve` handle (`SingleOffCurve`) $\to$ marquee-selecting 3 nodes (`MultiNode`) $\to$ clicking a guide (`Guide`) transitions `selection_info()` deterministically. |
| **`TC-V060-016`** | Universal `State_0 -> Mutation -> State_1 -> Undo == State_0 -> Redo == State_1` bit-exact roundtrip across all `v0.6.0` mutators | Every `SelectionInfo` mutator, ruler guide drag, background layer operation (`Cmd+B`/`Cmd+J`/`Cmd+Shift+J`), and `set_anchors_with_undo` satisfies bit-exact Undo/Redo roundtrip and `validate_invariants()`. |
| **`TC-V060-017`** | 1-to-1 `egui` $\leftrightarrow$ `tiny-skia` headless parity & Light/Dark (`AppearanceMode`) rendering for Preview Blur, Rulers, and Background Layer | Rendering the preview strip (normal, flipped, inverted, blurred), canvas rulers (`20 dp`), and background layer produces deterministic, non-empty pixel output across both `sonoma_default()` (Light) and `sonoma_dark()` (Dark) themes. |
| **`TC-V060-018`** | State-dependent pull-based `arbtest` fuzzing (`> 200` seeds, `12..=25` steps per chain) across `SelectionInfo` mutators, Rulers, Background Layer, Anchors & Preview | Arbitrary sequences of `SelectionInfo` mutations, 9-point anchor switches, handle radial length changes, ruler guide drags, `Cmd+B`/`Cmd+J`/`Cmd+Shift+J` background layer ops, `set_anchors_with_undo`, preview blur renders, and `Undo`/`Redo` preserve 100% of invariants. |
| **`TC-V060-019`** | Pure 2D `SelectionInfoAction` VC-Contract & interactive 3×3 HUD grid click | `SelectionInfo::GlyphBase` reports pure 2D `bbox_x/y/w/h`, `lsb`, `rsb`, `advance_width`, `transform_anchor`, and `proportional_scale`; `apply_selection_info_action` executes all local 2D mutations with Undo/Redo; clicking the 9 dots of the 3×3 Transform-Grid in the HUD updates `origin_anchor`. |
| **`TC-V060-020`** | `PreviewRenderConfig` & `render_preview_strip_rgba` across blur, flip, invert, and spacing combinations | `render_preview_strip_rgba` renders sharp, blurred (`0..20 px`), vertically flipped, and polarity-inverted proofing strips deterministically without UI-chrome pass-through state in `grafia`. |
| **`TC-V060-021`** | Zoom-adaptive ruler tick steps (`compute_ruler_tick_steps_upm`) across zoom levels | `compute_ruler_tick_steps_upm(zoom)` selects `1–2–5` major UPM intervals keeping `major_step_upm * zoom` within `[50.0, 140.0] dp` and `minor_step_upm == major_step_upm / 5.0` across zoom levels (`12.0` $\to$ `(1.0, 5.0)`, `6.0` $\to$ `(2.0, 10.0)`, `0.25` $\to$ `(50.0, 250.0)`, `0.15` $\to$ `(100.0, 500.0)`). |
| **`TC-V060-022`** | `align_selection_to_background` nearest-point projection onto `Line` and `Cubic` background segments | `align_selection_to_background()` projects every selected foreground `OnCurve` node (and translates its adjacent `OffCurve` handles by the same delta) onto the closest point on `background_contours` (`Line` and `Cubic`) with atomic Undo/Redo. |
| **`TC-V060-023`** | Battery-efficient `content_generation()` & `PreviewStripCache` (`get_or_render_rgba`, `get_or_update_texture`) | Hover, viewport pan/zoom, and node selection keep `content_generation()` unchanged and hit `PreviewStripCache` (`recomputed == false`); geometry, text-stream, anchor, component, stroke, theme, and Undo/Redo mutations bump `content_generation()` and trigger cache recomputation (`recomputed == true`). |
