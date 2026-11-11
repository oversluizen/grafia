---
type: specification
id: SPEC-006
title: "grafia v0.5.0 — Multi-Glyph Text Strip, Interactive Kerning & Spacing Mode"
version: 0.5.0
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

# SPEC-006: `grafia v0.5.0` — Multi-Glyph Text Strip, Interactive Kerning & Spacing Mode (`TextTool`)

> **Normative Verification Mandate (`ISO/IEC/IEEE 29119-3:2021` & `IEEE 1012-2016`):** Every constant, modifier rule, kerning indicator color, measurement line threshold, geometric algorithm, and state transition in this specification is anchored in OpenType 1.9 horizontal metrics & pair positioning, CAGD curve-line intersection math, and ISO 9241-9 keyboard/pointer interaction ergonomics. No production code for `v0.5.0` is written before this specification and the failing RED COTS test suite (`tests/v050_complete_tests.rs`) are anchored.
>
> **Architectural Scope Boundary (`ISO/IEC/IEEE 42010:2011`):** `grafia` is a standalone, headless-capable 2D vector & glyph canvas crate. `grafia` positions and renders the supplied `Vec<GlyphSlot>` on a continuous baseline, mutates local active-slot 2D geometry (`LSB` / `RSB` / contour shifts), and emits pure, deterministic `SpacingDelta` and `KerningDelta` events when dragging LSB/RSB/Kerning handles or pressing spacing/kerning keyboard shortcuts. The font-wide Kerning Database, Kerning Class/Group resolution (`@MMK_L_*` / `@MMK_R_*`), and OpenType `GPOS` table compilation reside exclusively in the **Host Font Model Backend** (coordinated by the **Host Application / Consumer Shell**), never inside `grafia`.

---

## 1. Normative Mathematical & Ergonomic Baseline (OpenType 1.9 / CAGD / ISO 9241-9)

All mathematical invariants, defaults, and keyboard/pointer rules for `v0.5.0` are specified below:

| Subsystem | Normative Domain & Shortcut | Mathematical & Ergonomic Specification |
| :--- | :--- | :--- |
| **`TextTool` Registration & Defaults (`T`)** | Multi-Glyph Stream Mode (`T`, `Escape` returns to vector edit) | Shortcut `"t"`. Activating `TextTool` sets `CursorIcon::Text` (I-beam cursor). Normative defaults: `TEXT_MODE_NUMBERS_THRESHOLD_DEFAULT = 100.0`, `TEXT_MEASUREMENT_LINE_DEFAULT_Y_UPM = 200.0`, `SPACING_INCREMENT_LOW_UPM = 1`, `SPACING_INCREMENT_HIGH_UPM = 10`, `KERNING_INCREMENT_LOW_UPM = 1`, `KERNING_INCREMENT_HIGH_UPM = 10`, `show_metrics = true`, `show_text_measurement_line`, and thick caret indicator. |
| **Kerning Highlight Colors** | Visual Pair-Kerning Polarity Indicators | **Negative Kerning** ($K < 0$): `RGBA(0.11, 0.62, 0.82, 0.20)` $\approx$ `[28, 158, 209, 51]` (translucent cyan/blue). **Positive Kerning** ($K > 0$): `RGBA(0.95, 0.86, 0.20, 0.20)` $\approx$ `[242, 219, 51, 51]` (translucent amber/yellow). Both span vertically from `descender` to `ascender`, sheared by `italic_angle_deg` around $\tfrac{1}{2} x_{\text{height}}$, and are clipped to the canvas bounds. |
| **Multi-Glyph Caret, Selection & Double-Click Slot Activation** | Stream Caret & Slot Switching (`click_count == 1` vs `click_count == 2`, `Escape`) | Single click (`click_count == 1`) resolves the nearest slot boundary index for the clicked UPM point and updates the selected slot range (extends selection range if `Shift` is held, else sets zero-length caret at `resolved_index`). Dragging beyond `hypot >= 2.0 dp` sweeps `selection_range`. **Active glyph for spacing/kerning:** the glyph immediately to the right of the caret (`caret_index`, clamped to $N - 1$). **Double-click (`click_count == 2`):** switches the clicked passive neighbor slot into the active editable glyph in `Select` mode. **`Escape`:** switches back to `last_select_tool` (defaulting to `ActiveTool::Select`). |
| **Keyboard Spacing & Kerning Matrix** | Modifier + Arrow Key Metrics Mutation ($\pm 1$ / $\pm 10$ UPM) | Step is `1` UPM without `Shift`, `10` UPM with `Shift`. On `ArrowLeft` ($-\text{step}$) / `ArrowRight` ($+\text{step}$):<br>• **`Ctrl + Left/Right`**: adjusts Left Sidebearing (`LSB`) by $\pm\text{step}$ (shifts all paths/components/anchors/guides by $\pm\text{step}$ and adjusts `advance_width += ±step`, leaving `RSB` unchanged).<br>• **`Cmd + Left/Right`**: adjusts Right Sidebearing (`RSB`) by $\pm\text{step}$ (adjusts `advance_width += ±step` while keeping `LSB` and contour positions fixed).<br>• **`Cmd + Ctrl + Left/Right`** (without `Alt`/`Option`): shifts all contours horizontally by $\pm\text{step}$ within a constant `advance_width` (`LSB += ±step`, `RSB -= ±step`).<br>• **`Ctrl + Opt + Left/Right`**: adjusts left kerning pair `(prev_glyph, active_glyph)` by $\pm\text{step}$.<br>• **`Cmd + Opt + Left/Right`**: adjusts right kerning pair `(active_glyph, next_glyph)` by $\pm\text{step}$.<br>Records atomic undo entry `"Change Metrics"`. |
| **Text-Mode Measurement Line & Sidebearing Numbers** | Horizontal Scanline Metrology ($y = 200.0$ UPM default) | Enabled when `show_text_measurement_line || temp_show_text_measurement_line`. Intersects a horizontal scanline at $y = y_{\text{line}}$ (default `200.0` UPM, vertically draggable via `Cmd+Ctrl+Opt` drag) with each glyph layer's contours and its sheared LSB/RSB boundary lines. Draws circular intersection knobs along the scanline. When $\text{upm} \cdot \text{zoom} > \text{text\_mode\_numbers\_threshold}$ (`100.0`), renders live LSB, RSB, advance width, and effective kerning distance numbers along the baseline/measurement line for every visible slot in the strip. |
| **Host Kerning Delta Contract** | Decoupled Event Emission (`SpacingDelta` & `KerningDelta`) | `grafia` surfaces the active slot's effective `kern_with_prev` / `kern_with_next` values and emits `SpacingDelta` and `KerningDelta` events, while the **Host Font Model Backend** owns the font-wide kerning dictionary, `@MMK_L_*` / `@MMK_R_*` kerning groups, and OpenType `GPOS` compilation. |

---

## 2. Normative Requirements & Subsystems (`grafia v0.5.0`)

### 2.1 Multi-Glyph Horizontal Text Strip (`CanvasTextStream` & `GlyphSlot`)

1. **Multi-Glyph Canvas Buffer & Host Boundary (`CanvasTextStream`):**
   - `GrafiaCanvasState` maintains a `text_stream: CanvasTextStream` containing an ordered sequence of `Vec<GlyphSlot>` laid out horizontally along a shared baseline ($y = 0$) in unified UPM coordinates.
   - **Strict Decoupling from Font-Wide Storage (`ISO/IEC/IEEE 42010:2011`):**
     - `grafia` does **not** own a font-wide Kerning Database, Kerning Groups (`@MMK_L_*` / `@MMK_R_*`), or OpenType `GPOS` tables; those belong strictly to the **Host Font Model Backend**.
     - Instead, the **Host Application / Consumer Shell** supplies `Vec<GlyphSlot>` (where each `GlyphSlot` carries its `Glyph` geometry and the pre-resolved effective `kern_with_prev: i32` in UPM for the pair `(slot[k-1], slot[k])`).
     - When `text_stream.slots` is non-empty, `grafia` positions slot $i$ at horizontal pen origin:
       $$X_0 = 0, \qquad X_i = \sum_{k=0}^{i-1} \left(\text{width}_k + \text{kern\_with\_prev}_{k+1}\right)$$
       where $\text{width}_k$ is `slots[k].glyph.advance_width` and $\text{kern\_with\_prev}_{k+1}$ is `slots[k+1].kern_with_prev as f32`.
2. **Active Slot vs. Passive Context Slots:**
   - **In Edit Modes (`ActiveTool::Select`, `Pen`, `Knife`, `Primitives`, `Measurement`, `Hand`, etc.):**
     - `text_stream.active_slot_index` identifies the single active glyph slot being edited.
     - All passive neighbor slots ($j \ne \text{active\_slot\_index}$) render as solid filled shapes (`theme.passive_glyph_fill` — dark charcoal in Light mode, crisp off-white in Dark mode) at their respective horizontal pen origins $(X_j, 0)$ without nodes, BCP handles, or anchor overlays.
     - The active slot ($i = \text{active\_slot\_index}$) renders with full editable vector paths, nodes, BCP handles, anchors, guides, sidebearing lines ($x = X_i$ and $x = X_i + \text{width}_i$), and metrics frame at pen origin $(X_i, 0)$.
     - Any mutation to the active glyph's `advance_width`, `LSB`, `RSB`, or slot `kern_with_prev` immediately recomputes all downstream slot origins $X_{k}$ ($k > i$) in real time, and synchronizes any repeated occurrences of the same `glyph_id` / `glyph.name` within `text_stream.slots`.
   - **Double-Click Passive Slot Activation:**
     - Double-clicking (`click_count == 2`) on any passive glyph slot $j$ in the strip (whether in `ActiveTool::Text` or `ActiveTool::Select`) switches `text_stream.active_slot_index = j`, synchronizes the active editable `Glyph` with slot $j$, sets `text_stream.caret_index = j`, and activates `ActiveTool::Select` so the user can immediately edit slot $j$'s contours.
   - **Stepping Between Glyph Slots (`Home` / `End` / `step_active_slot`):**
     - Pressing `Home` (`Fn+Left`) or calling `state.step_active_slot(-1)` moves `active_slot_index` (and `caret_index`) to the previous slot $(i - 1)\max 0$.
     - Pressing `End` (`Fn+Right`) or calling `state.step_active_slot(+1)` moves `active_slot_index` (and `caret_index`) to the next slot $\min(i + 1, N - 1)$.

---

### 2.2 `TextTool` (`ActiveTool::Text`, Shortcut `T`) & Caret Semantics

1. **Activation & Escape Return:**
   - Pressing `T` activates `ActiveTool::Text` (saving the prior vector tool in `state.last_select_tool`) and requests `CursorIcon::Text`.
   - Pressing `Escape` while in `ActiveTool::Text` clears any multi-slot text selection range and returns to `state.last_select_tool` (defaulting to `ActiveTool::Select`).
2. **Caret Placement, Selection Sweep & Active Glyph Rule:**
   - `text_stream.caret_index` lies in $0 \dots N$ (where $N = \text{slots.len()}$).
   - **Authoritative Active Glyph Rule:** The active glyph targeted by all spacing (`LSB`/`RSB`) and kerning (`Ctrl+Opt` / `Cmd+Opt`) keyboard shortcuts in `ActiveTool::Text` is the glyph **immediately to the right of the caret** (`slot_index = caret_index`, clamped to $N - 1$ when `caret_index == N` and $N > 0$).
   - **Single Click (`PointerDown` with `click_count == 1`):**
     - Resolves the nearest slot boundary index (`layer_index_for_point(upm_pt)`): if `upm_pt.x` falls before the horizontal midpoint $X_k + \tfrac{1}{2}\text{width}_k$ of slot $k$, resolves to index $k$; otherwise $k + 1$.
     - Without `Shift`: sets `caret_index = resolved_index`, `selection_range = None`, and updates `active_slot_index = resolved_index.min(N.saturating_sub(1))`.
     - With `Shift`: extends `selection_range` from the anchor caret index to `resolved_index`.
   - **Pointer Drag (`PointerMove` with `hypot >= 2.0` dp):**
     - Continuously updates `selection_range = Some((anchor_index.min(curr), anchor_index.max(curr)))` across the swept glyph slots.
   - **Text Stream Editing Events in `ActiveTool::Text`:**
     - `Backspace` deletes the selected slot range or the slot at `caret_index - 1`; `Delete` deletes the selected slot range or the slot at `caret_index`.
     - `Left` / `Right` arrow keys without `Ctrl`/`Cmd`/`Opt` move `caret_index` by $\mp 1$ (or collapse/extend `selection_range` with `Shift`).
     - `Cmd+A` selects all slots `0..N`.

---

### 2.3 Interactive Spacing (`LSB` / `RSB`) & Kerning Keyboard Matrix + Event Emission

In both `ActiveTool::Text` and `ActiveTool::Select` (when spacing/kerning modifiers are pressed), `grafia` implements the five normative spacing and kerning modifier combinations on the active glyph slot $i$ and emits structured `SpacingDelta` / `KerningDelta` events for the **Host Font Model Backend**:

1. **Increment Step:**
   - Base step: $\Delta = 1\text{ UPM}$ when `modifiers.shift == false`.
   - High step: $\Delta = 10\text{ UPM}$ when `modifiers.shift == true`.
   - Direction sign: $s = -1$ for `ArrowLeft`, $s = +1$ for `ArrowRight`. Signed delta: $d = s \cdot \Delta$.
2. **Five Normative Modifier Combinations:**

| Action | macOS Shortcut | Windows / Linux / Cross-Platform Equivalent | Normative Invariant | Exact Geometric Mutation & Emitted Host Event |
| :--- | :--- | :--- | :--- | :--- |
| **Adjust Left Sidebearing (`LSB`)** | `Ctrl + Left/Right` (`+Shift` = $\pm 10$) | `Ctrl + Left/Right` (`modifiers.ctrl && !modifiers.cmd && !modifiers.alt`) | $\Delta\text{LSB} = d,\; \Delta\text{RSB} = 0,\; \Delta W = d$ | Translates all contours, nodes (OnCurve + OffCurve), anchors, components, and glyph-local guides of active glyph $i$ horizontally by $\Delta x = d$, increments `advance_width += d` ($\text{LSB}_i' = \text{LSB}_i + d$, $\text{RSB}_i' = \text{RSB}_i$), and emits `SpacingDelta { slot_index: i, glyph_name, lsb_delta: d, rsb_delta: 0, width_delta: d }`. |
| **Adjust Right Sidebearing (`RSB`)** | `Cmd + Left/Right` (`+Shift` = $\pm 10$) | `Cmd + Left/Right` (`modifiers.cmd && !modifiers.ctrl && !modifiers.alt`) | $\Delta\text{LSB} = 0,\; \Delta\text{RSB} = d,\; \Delta W = d$ | Keeps all contour, node, anchor, and component coordinates fixed, increments `advance_width += d` (clamped to $\ge 0$, $\text{RSB}_i' = \text{RSB}_i + d$, $\text{LSB}_i' = \text{LSB}_i$), and emits `SpacingDelta { slot_index: i, glyph_name, lsb_delta: 0, rsb_delta: d, width_delta: d }`. |
| **Shift Contours Within Constant Advance Width** | `Cmd + Ctrl + Left/Right` (`+Shift` = $\pm 10$) | `Cmd + Ctrl + Left/Right` (`modifiers.cmd && modifiers.ctrl && !modifiers.alt`) | $\Delta\text{LSB} = d,\; \Delta\text{RSB} = -d,\; \Delta W = 0$ | Translates all contours, nodes, anchors, and components of active glyph $i$ horizontally by $\Delta x = d$ while keeping `advance_width` strictly constant ($\text{LSB}_i' = \text{LSB}_i + d$, $\text{RSB}_i' = \text{RSB}_i - d$), and emits `SpacingDelta { slot_index: i, glyph_name, lsb_delta: d, rsb_delta: -d, width_delta: 0 }`. |
| **Adjust Left Kerning Pair `(prev, active)`** | `Ctrl + Opt + Left/Right` (`+Shift` = $\pm 10$) | `Ctrl + Alt + Left/Right` (`modifiers.ctrl && modifiers.alt && !modifiers.cmd`) | $\Delta K_{i-1,i} = d$ | Requires $i > 0$. Updates `slots[i].kern_with_prev += d`, recomputes slot origins $X_i \dots X_{N-1}$, and emits `KerningDelta { left_slot_index: i - 1, right_slot_index: i, left_glyph: slots[i-1].glyph.name, right_glyph: slots[i].glyph.name, delta_upm: d, new_kern_upm: slots[i].kern_with_prev }` for the **Host Font Model Backend**. |
| **Adjust Right Kerning Pair `(active, next)`** | `Cmd + Opt + Left/Right` (`+Shift` = $\pm 10$) | `Cmd + Alt + Left/Right` (`modifiers.cmd && modifiers.alt && !modifiers.ctrl`) | $\Delta K_{i,i+1} = d$ | Requires $i + 1 < N$. Updates `slots[i + 1].kern_with_prev += d`, recomputes slot origins $X_{i+1} \dots X_{N-1}$, and emits `KerningDelta { left_slot_index: i, right_slot_index: i + 1, left_glyph: slots[i].glyph.name, right_glyph: slots[i+1].glyph.name, delta_upm: d, new_kern_upm: slots[i+1].kern_with_prev }` for the **Host Font Model Backend**. |

3. **Interactive Canvas Kerning & Sidebearing Drag:**
   - In `ActiveTool::Text` (or when dragging the LSB/RSB boundary lines on the baseline), dragging horizontally with `Ctrl+Alt` (left kerning) or `Cmd+Alt` (right kerning) or dragging the active glyph's LSB/RSB metric lines directly on the canvas updates the active slot's metrics/kerning in real time, records an atomic Undo snapshot (`"Change Metrics"`), and emits the corresponding `SpacingDelta` or `KerningDelta` event.

---

### 2.4 Kerning Baseline Highlight Wedges

1. **Normative Geometry & Color Rule:**
   - For every adjacent pair of slots $(k, k+1)$ in `text_stream.slots` whose effective kerning value $K = \text{slots}[k+1].\text{kern\_with\_prev} \ne 0$:
     - The kerning indicator region spans horizontally at the boundary between slot $k$'s unkerned advance edge $X_k + \text{width}_k$ and slot $k+1$'s kerned origin $X_{k+1} = X_k + \text{width}_k + K$:
       - $x_{\text{start}} = \min(X_k + \text{width}_k, X_{k+1})$
       - $x_{\text{end}} = \max(X_k + \text{width}_k, X_{k+1})$
     - Vertically, the indicator spans from `metrics.descender` to `metrics.ascender`, sheared by `metrics.italic_angle_deg` around `half_x_height = metrics.x_height * 0.5`.
     - **Fill Color:**
       - When $K < 0$ (**Negative Kerning** — glyphs pulled closer together): filled with `theme.kerning_negative_fill` (`RGBA(0.11, 0.62, 0.82, 0.20)` = `[28, 158, 209, 51]`, translucent cyan/blue).
       - When $K > 0$ (**Positive Kerning** — glyphs pushed further apart): filled with `theme.kerning_positive_fill` (`RGBA(0.95, 0.86, 0.20, 0.20)` = `[242, 219, 51, 51]`, translucent amber/yellow).

---

### 2.5 Text-Mode Measurement Line & Sidebearing Numbers

1. **Text-Mode Measurement Line:**
   - Controlled by `state.show_text_measurement_line: bool` or temporary `state.temp_show_text_measurement_line: bool` (held via `Cmd+Ctrl+Opt` while in `ActiveTool::Text`).
   - Horizontal scanline position in UPM: `state.text_measurement_y_upm: f32` (default `200.0` UPM).
   - Dragging vertically in `ActiveTool::Text` while holding `Cmd+Ctrl+Opt` (or calling `state.set_text_measurement_y(y_upm)`) continuously repositions `text_measurement_y_upm` across the entire multi-glyph strip.
   - **Per-Slot Scanline Intersections:**
     - For each slot $k$ at horizontal offset $X_k$:
       1. Computes the italic-sheared Left Sidebearing x-coordinate at $y = y_{\text{line}}$:
          $$x_{\text{LSB},k} = X_k + \left(y_{\text{line}} - \tfrac{1}{2}x_{\text{height}}\right)\cdot\tan(\theta_{\text{italic}})$$
       2. Computes the italic-sheared Right Sidebearing x-coordinate at $y = y_{\text{line}}$:
          $$x_{\text{RSB},k} = X_k + \text{width}_k + \left(y_{\text{line}} - \tfrac{1}{2}x_{\text{height}}\right)\cdot\tan(\theta_{\text{italic}})$$
       3. Intersects the horizontal scanline $y = y_{\text{line}}$ with all contour segments of slot $k$ (shifted by $+X_k$), sorting all contour intersections $c_{k,1} \le c_{k,2} \le \dots \le c_{k,m}$.
       4. When $m \ge 1$, the measured left distance at $y_{\text{line}}$ is $d_{\text{left},k} = c_{k,1} - x_{\text{LSB},k}$ and the measured right distance at $y_{\text{line}}$ is $d_{\text{right},k} = x_{\text{RSB},k} - c_{k,m}$, plus interior stem/counter intervals $c_{k,r+1} - c_{k,r}$.
   - **Visual Rendering:**
     - Strokes the horizontal scanline across the full strip width in `theme.text_measurement_line_color` (`1.0 dp`).
     - Draws circular intersection knobs (`3.0 dp` radius) at every contour and sidebearing intersection along $y = y_{\text{line}}$.
     - When `metrics.upm as f32 * viewport.zoom > state.text_mode_numbers_threshold` (default `100.0`), renders monospaced numeric labels for the measured left/right sidebearing distances along $y = y_{\text{line}}$ and baseline LSB / width / RSB / kerning numbers below the descender line.

---

## 3. Light & Dark Appearance Tokens (`src/tokens/v050_tokens.rs`)

All `v0.5.0` visual overlays support both `GrafiaTheme::sonoma_default()` (Light) and `GrafiaTheme::sonoma_dark()` (Dark) with zero hardcoded RGBA literals outside `src/tokens/`:

| Token Name | Light Mode (`sonoma_default`) | Dark Mode (`sonoma_dark`) | Semantic Role |
| :--- | :--- | :--- | :--- |
| `kerning_negative_fill` | `[28, 158, 209, 51]` (`0.11, 0.62, 0.82, 0.20`) | `[28, 158, 209, 51]` (`0.11, 0.62, 0.82, 0.20`) | Negative kerning highlight quad fill |
| `kerning_positive_fill` | `[242, 219, 51, 51]` (`0.95, 0.86, 0.20, 0.20`) | `[242, 219, 51, 51]` (`0.95, 0.86, 0.20, 0.20`) | Positive kerning highlight quad fill |
| `passive_glyph_fill` | `[28, 28, 30, 255]` | `[235, 235, 240, 255]` | Passive neighbor glyph silhouette fill |
| `text_caret_color` | `[30, 30, 32, 240]` | `[240, 240, 245, 240]` | Multi-glyph stream text caret stroke |
| `text_selection_fill` | `[46, 117, 219, 55]` | `[74, 144, 242, 70]` | Multi-slot text selection highlight fill |
| `text_measurement_line_color` | `[120, 120, 128, 200]` | `[165, 165, 175, 200]` | Horizontal text-mode scanline stroke |
| `text_measurement_knob_fill` | `[90, 90, 98, 235]` | `[190, 190, 200, 235]` | Scanline contour/sidebearing intersection knobs |
| `text_metrics_number_color` | `[100, 100, 108, 235]` | `[175, 175, 185, 235]` | Live LSB/RSB/width/kerning numeric labels |

---

## 4. Rust Data Structures & API Contracts

```rust
/// Default zoom-times-UPM threshold for displaying live metrics numbers in text mode.
pub const TEXT_MODE_NUMBERS_THRESHOLD_DEFAULT: f64 = 100.0;

/// Default vertical scanline position in UPM for the text-mode measurement line.
pub const TEXT_MEASUREMENT_LINE_DEFAULT_Y_UPM: f64 = 200.0;

/// Low and high step increments for keyboard spacing and kerning adjustments.
pub const SPACING_INCREMENT_LOW_UPM: i32 = 1;
pub const SPACING_INCREMENT_HIGH_UPM: i32 = 10;
pub const KERNING_INCREMENT_LOW_UPM: i32 = 1;
pub const KERNING_INCREMENT_HIGH_UPM: i32 = 10;

/// Minimum pointer drag distance in dp before sweeping a text selection range.
pub const TEXT_SELECTION_DRAG_THRESHOLD_DP: f32 = 2.0;

/// A single slot in the multi-glyph horizontal text strip (`CanvasTextStream`).
/// Supplied by the Host Application / Consumer Shell; `grafia` computes `origin_x_upm`
/// along the continuous baseline and emits `SpacingDelta` / `KerningDelta` events.
#[derive(Clone, Debug, PartialEq)]
pub struct GlyphSlot {
    pub glyph: Glyph,
    /// Effective kerning applied before this slot (`kern(i-1, i)` in UPM), `0` for slot `0`.
    pub kern_with_prev: i32,
    /// Computed horizontal pen origin `X_i` in UPM along the shared baseline (`y = 0`).
    pub origin_x_upm: f64,
}

/// Pure spacing mutation event emitted by `grafia` for the Host Font Model Backend.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SpacingDelta {
    pub slot_index: usize,
    pub glyph_name: String,
    pub lsb_delta_upm: i32,
    pub rsb_delta_upm: i32,
    pub width_delta_upm: i32,
}

/// Pure kerning mutation event emitted by `grafia` for the Host Font Model Backend.
/// The Host Font Model Backend resolves `@MMK_L_*` / `@MMK_R_*` kerning groups and `GPOS` storage.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct KerningDelta {
    pub left_slot_index: usize,
    pub right_slot_index: usize,
    pub left_glyph: String,
    pub right_glyph: String,
    pub delta_upm: i32,
    pub new_kern_upm: i32,
}

/// Kerning highlight wedge geometry produced for adjacent kerned slots `(k, k+1)`.
#[derive(Clone, Debug, PartialEq)]
pub struct KerningIndicatorQuad {
    pub left_slot_index: usize,
    pub right_slot_index: usize,
    pub kerning_value_upm: i32,
    /// Four corners `[bottom_left, bottom_right, top_right, top_left]` in UPM (sheared by italic angle).
    pub corners_upm: [FontPoint; 4],
}

/// Per-slot horizontal scanline measurement results.
#[derive(Clone, Debug, PartialEq)]
pub struct SlotScanlineMeasurement {
    pub slot_index: usize,
    pub y_upm: f64,
    pub lsb_x_upm: f64,
    pub rsb_x_upm: f64,
    /// Sorted contour intersection x-coordinates in global strip UPM space.
    pub contour_hit_xs_upm: Vec<f64>,
    /// Distance from sheared LSB line to first contour hit (`None` if no contour hit).
    pub measured_lsb_upm: Option<f64>,
    /// Distance from last contour hit to sheared RSB line (`None` if no contour hit).
    pub measured_rsb_upm: Option<f64>,
}

/// Multi-glyph horizontal text buffer, caret/selection state, and emitted host delta queue.
#[derive(Clone, Debug, PartialEq)]
pub struct CanvasTextStream {
    pub slots: Vec<GlyphSlot>,
    /// Index of the active editable glyph slot (`0..slots.len()`).
    pub active_slot_index: usize,
    /// Caret index (`0..=slots.len()`). Active glyph for spacing/kerning is the slot right of the caret.
    pub caret_index: usize,
    /// Optional multi-slot selection range `(start_slot, end_slot_exclusive)`.
    pub selection_range: Option<(usize, usize)>,
    /// Emitted spacing delta events ready to be drained by the Host Font Model Backend.
    pub pending_spacing_deltas: Vec<SpacingDelta>,
    /// Emitted kerning delta events ready to be drained by the Host Font Model Backend.
    pub pending_kerning_deltas: Vec<KerningDelta>,
}
```

---

## 5. Normative COTS Verification Matrix (`tests/v050_complete_tests.rs`)

| Test ID | Requirement Under Test | Expected Outcome |
| :--- | :--- | :--- |
| **`TC-V050-001`** | `CanvasTextStream` multi-glyph horizontal layout & cumulative origin formula $X_i = \sum_{k=0}^{i-1}(\text{width}_k + \text{kern\_with\_prev}_{k+1})$ | Setting a multi-glyph `Vec<GlyphSlot>` (e.g. `["H", "A", "V", "O"]`) positions each slot at its exact cumulative `advance_width + kern_with_prev` origin along the shared baseline ($y = 0$). |
| **`TC-V050-002`** | Active editable glyph slot vs. passive filled neighbor context slots | In `Select`/`Pen` mode, only `active_slot_index` renders editable nodes, handles, and anchors; all neighbor slots render as solid filled shapes at their respective $(X_j, 0)$ origins. |
| **`TC-V050-003`** | Double-clicking (`click_count == 2`) a passive neighbor slot | Double-clicking passive slot $j$ switches `active_slot_index = j`, loads slot $j$'s glyph as the active editable layer, updates `caret_index = j`, and switches the active tool to `ActiveTool::Select`. |
| **`TC-V050-004`** | `TextTool` (`T`) activation, `IBeam` cursor, caret placement, and `Escape` | Pressing `T` activates `ActiveTool::Text` (`CursorIcon::Text`); single-clicking places `caret_index` at the nearest slot boundary so the glyph immediately to the right of the caret is active; pressing `Escape` restores the previous vector tool (`Select`). |
| **`TC-V050-005`** | `TextTool` pointer drag selection sweep (`hypot >= 2.0` dp), `Shift+Click` range extension, `Cmd+A`, and `Backspace`/`Delete` | Dragging across slots in `TextTool` selects `selection_range`; `Shift+Click` extends the range; `Cmd+A` selects `0..N`; `Backspace`/`Delete` removes the selected slots and updates origins with Undo/Redo. |
| **`TC-V050-006`** | `Home` / `End` (`step_active_slot`) and `Left` / `Right` arrow caret stepping across the multi-glyph strip | `Home`/`End` steps `active_slot_index` backward/forward across `0..N-1`; `Left`/`Right` in `TextTool` moves `caret_index` across `0..=N` and updates the active glyph immediately to the right of the caret. |
| **`TC-V050-007`** | `Ctrl + Left/Right` (`+Shift`) Left Sidebearing adjustment (`±1` / `±10` UPM) & `SpacingDelta` emission | Pressing `Ctrl+Right` (`+1`) or `Ctrl+Shift+Right` (`+10`) shifts all contours, nodes, and anchors of the active glyph by $+d$, increments `advance_width` by $+d$ (`RSB` invariant), shifts downstream slots by $+d$, and emits `SpacingDelta { lsb_delta_upm: d, rsb_delta_upm: 0, width_delta_upm: d }`. |
| **`TC-V050-008`** | `Cmd + Left/Right` (`+Shift`) Right Sidebearing adjustment (`±1` / `±10` UPM) & `SpacingDelta` emission | Pressing `Cmd+Left` (`-1`) or `Cmd+Shift+Right` (`+10`) leaves all contour/node coordinates and `LSB` strictly unchanged, adjusts `advance_width` (and `RSB`) by $\pm d$, shifts downstream slots by $\pm d$, and emits `SpacingDelta { lsb_delta_upm: 0, rsb_delta_upm: d, width_delta_upm: d }`. |
| **`TC-V050-009`** | `Cmd + Ctrl + Left/Right` (`+Shift`) contour shift within constant `advance_width` & `SpacingDelta` emission | Pressing `Cmd+Ctrl+Right` (`+1`) or `Cmd+Ctrl+Shift+Left` (`-10`) translates all contours, nodes, and anchors of the active glyph by $\pm d$ while keeping `advance_width` strictly constant ($\text{LSB}' = \text{LSB} \pm d$, $\text{RSB}' = \text{RSB} \mp d$), and emits `SpacingDelta { lsb_delta_upm: d, rsb_delta_upm: -d, width_delta_upm: 0 }`. |
| **`TC-V050-010`** | `Ctrl + Opt + Left/Right` (`+Shift`) Left Kerning (`direction = -1`) for pair `(prev_glyph, active_glyph)` & `KerningDelta` emission | Pressing `Ctrl+Opt+Left` (`-1`) or `Ctrl+Opt+Shift+Left` (`-10`) updates `slots[i].kern_with_prev += d`, shifts slots $i \dots N-1$ without altering any glyph's `LSB`, `RSB`, or contours, and emits `KerningDelta` for `(slots[i-1], slots[i])`. |
| **`TC-V050-011`** | `Cmd + Opt + Left/Right` (`+Shift`) Right Kerning (`direction = +1`) for pair `(active_glyph, next_glyph)` & `KerningDelta` emission | Pressing `Cmd+Opt+Right` (`+1`) or `Cmd+Opt+Shift+Left` (`-10`) updates `slots[i+1].kern_with_prev += d`, shifts slots $i+1 \dots N-1$ while keeping slot $i$'s origin and metrics fixed, and emits `KerningDelta` for `(slots[i], slots[i+1])`. |
| **`TC-V050-012`** | Interactive canvas LSB/RSB & kerning drag + Host `drain_spacing_deltas()` / `drain_kerning_deltas()` contract | Dragging the active slot's LSB/RSB lines or kerning gap on canvas updates strip geometry with full Undo/Redo (`"Change Metrics"`) and queues exact `SpacingDelta` / `KerningDelta` events for the Host Font Model Backend. |
| **`TC-V050-013`** | Kerning baseline highlight wedges: Negative Cyan `[28, 158, 209, 51]` vs. Positive Yellow `[242, 219, 51, 51]` with italic shear | Negative kerning ($K < 0$) generates a cyan highlight quad (`[28, 158, 209, 51]`) and positive kerning ($K > 0$) generates an amber/yellow highlight quad (`[242, 219, 51, 51]`) spanning `descender..ascender` and sheared by `italic_angle_deg`. |
| **`TC-V050-014`** | Text-Mode Measurement Line (default $y = 200.0$ UPM) & `Cmd+Ctrl+Opt` vertical drag | Enabling the text measurement line computes exact contour intersections and sheared LSB/RSB distances at $y = 200.0$ UPM for every slot in the strip; dragging vertically with `Cmd+Ctrl+Opt` updates $y$ and recomputes all intersection knobs in real time. |
| **`TC-V050-015`** | Zoom-dependent sidebearing & kerning number visibility (`TEXT_MODE_NUMBERS_THRESHOLD_DEFAULT = 100.0`) | When `upm * zoom > 100.0`, live LSB, RSB, advance width, and kerning numeric badges are emitted for visible slots; when zoomed out such that `upm * zoom <= 100.0`, numeric badges are suppressed to prevent visual clutter. |
| **`TC-V050-016`** | Same-glyph repeated slot synchronization (e.g. `"NON"` or `"HAHA"`) | Editing the contours, `LSB`, or `RSB` of `"N"` in slot `0` of `"NON"` immediately updates the geometry and advance width of every occurrence of `"N"` (slot `0` and slot `2`) in the strip. |
| **`TC-V050-017`** | 1-to-1 `egui` $\leftrightarrow$ `tiny-skia` headless parity & Light/Dark (`AppearanceMode`) rendering for multi-glyph strip, kerning wedges, and measurement line | Rendering a multi-glyph strip with negative/positive kerning wedges and the text measurement line produces deterministic, non-empty pixel output across both `sonoma_default()` (Light) and `sonoma_dark()` (Dark) themes. |
| **`TC-V050-018`** | State-dependent pull-based `arbtest` multi-glyph spacing, kerning, slot switching & editing fuzzing (`> 200` seeds, `12..=25` steps per chain) | Arbitrary sequences of slot insertion/deletion, caret moves, double-click slot switches, `LSB`/`RSB`/contour-shift shortcuts, left/right kerning delta mutations, measurement line drags, contour edits, and `Undo`/`Redo` preserve 100% of strip and glyph invariants. |
