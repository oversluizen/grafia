---
type: specification
id: SPEC-010
version: 0.9.0
status: normative
parent: SPEC-001
---

# SPEC-010: `grafia v0.9.0` — Multi-Layer Vector Color/Gradients/Bitmaps, Vertical `TopToBottom` Text Stream, Domain-Agnostic `CustomReferenceFrame`, PostScript/TrueType Hinting & Higher-Order Interpolation Overlays

> **Normative References**:
> - `SPEC-001` & `SPEC-002`: `grafia v0.1.0` — 5-Pass Rendering Pipeline, `ShapeThumbnailProjection` & Offscreen `tiny-skia` Parity
> - `SPEC-004` & `SPEC-006`: `grafia v0.3.0`..`v0.5.0` — `Guide`, Alignment Zones, `TextTool` Stream & Sidebearing/Kerning Ports
> - `SPEC-008` & `SPEC-009`: `grafia v0.7.0`..`v0.8.0` — `CompanionMasterLayer`, `CanvasAnchor`, `GuideThickness` & `GrafiaPreferences`
> - Standards: `ISO/IEC 14496-22:2019` (`OpenType 1.9` — `COLR v1`, `CPAL`, `sbix`, `BASE`, `vhea`/`vmtx`, `glyf` Grid-Fitting), `ISO/IEC/IEEE 42010:2011`, `ISO/IEC/IEEE 29119-3:2021`, `IEEE 1012-2016`, `SemVer 2.0.0`

---

## 1. Normative Mathematical & Geometric Foundations

All color paint shaders, vertical text stream metrics, domain-agnostic reference boxes/subdivision grids/parallel bands, pixel-grid hinting projections, and higher-order interpolation overlays in `SPEC-010` are grounded in `ISO/IEC 14496-22:2019` and 2D projective geometry while keeping `grafia` 100% free of script- or domain-specific assumptions:

| Subsystem | Domain Standard / Mathematical Model | Core Invariant / Equation | Verified Canvas Semantics |
| :--- | :--- | :--- | :--- |
| **1. Multi-Layer Color, Gradients, Bitmaps & Colored Thumbnails** | `OpenType 1.9` `COLR v1` Paint Stack & Porter-Duff `SrcOver` Compositing | $C_{\text{out}} = C_{\text{src}} + (1 - \alpha_{\text{src}})C_{\text{dst}}$ | Composites ordered `CanvasColorSublayer` items (`Solid`, `LinearGradient`, `RadialGradient`, `SweepGradient`, `Bitmap`, `ClippingMask`) in Pass 2, exposes draggable gradient-stop handles in Pass 4, and renders multi-color grid thumbnails (`render_colored_thumbnail`). |
| **2. Vertical `TopToBottom` Text Stream & Vertical Metrics Ports** | `OpenType 1.9` `vhea`/`vmtx`/`VORG` Vertical Advance Geometry | $y_{\text{cursor},k+1} = y_{\text{cursor},k} - \text{advance\_height}_k - K_V(g_k, g_{k+1})$ | Advances `GlyphSlot`s vertically along center axis $x_c = \frac{1}{2}W$, exposes draggable/keyboard `TSB`, `BSB`, `vert_origin_y`, and `advance_height` handles, and emits `SpacingDelta` / `KerningDelta` on `MetricsAxis::Vertical`. |
| **3. Domain-Agnostic `CustomReferenceFrame` (Boxes, Subdivisions & Parallel Bands)** | Nested 2D Bounding Boxes, $N_x \times N_y$ Lattice & `GuideThickness` Parallel Lines | $x_i = x_0 + \frac{i}{N_x}(x_1 - x_0)$, $\;y_j = y_0 + \frac{j}{N_y}(y_1 - y_0)$ | Renders arbitrary host-supplied `outer_box`, `inner_box`, $N_x \times N_y$ composition subdivisions, concentric keyline circles/squares, and parallel `ReferenceBandLine`s with `GuideThickness::{ ScreenDp, FontUnits(w) }` — covering ideographic em/ICF boxes, multi-line staves, and icon keyline templates through one pure geometric primitive. |
| **4. PostScript Stem/Ghost Bands & TrueType Instructor Canvas Mode** | Stem/Ghost Zones & `glyf` PPEM Grid-Fitting $x_{\text{px}} = \frac{x_{\text{upm}} \cdot \text{PPEM}}{\text{UPM}}$ | $\Delta_{\text{grid}}(p) = \frac{\operatorname{round}(p \cdot s)}{s} - p, \;s = \frac{\text{PPEM}}{\text{UPM}}$ | Renders horizontal/vertical PS stem bands and 1-edge ghost hints, and provides `CanvasTool::TrueTypeInstructor` displaying master + pre-hinted + grid-fitted outlines over the PPEM pixel grid with interactive `Snap`/`Stem`/`Shift`/`Interpolate`/`Delta` badges. |
| **5. Higher-Order Interpolation (`HOI`) Diamond Overlay** | Multi-Layer Cubic/Multilinear Master Control Net | $B_3(t) = (1-t)^3 P_0 + 3(1-t)^2 t P_1 + 3(1-t)t^2 P_2 + t^3 P_3$ | Renders per-node interpolation control-polygon polylines and color-coded diamond markers (`prev_layer`, `current`, `next_layer`) across ordered `CompanionMasterLayer`s. |

---

## 2. Normative Mathematical & Functional Requirements

### 2.1 Multi-Layer Color, Gradients, Bitmaps & Colored Thumbnails (`src/render/color_layers.rs`)

1. **Color Sublayer Stack (`Vec<CanvasColorSublayer>`)**:
   ```rust
   pub enum ColorLayerPaint {
       Solid { rgba: [u8; 4] },
       LinearGradient { p0: FontPoint, p1: FontPoint, stops: Vec<GradientStop>, spread: GradientSpread },
       RadialGradient { c0: FontPoint, r0: f64, c1: FontPoint, r1: f64, stops: Vec<GradientStop>, spread: GradientSpread },
       SweepGradient { center: FontPoint, start_deg: f64, end_deg: f64, stops: Vec<GradientStop> },
       Bitmap { image: CanvasBackgroundImage, ppem_strike: Option<u16> },
       ClippingMask,
   }
   ```
   - Each `CanvasColorSublayer` holds `contours: Vec<GlyphContour>`, `paint: ColorLayerPaint`, `opacity: f32`, `visible: bool`, and `locked: bool`.
2. **Interactive Gradient Editing, `SelectionInfo::GradientStop` & Colored Thumbnail Projection**:
   - Selecting an unlocked gradient sublayer exposes on-canvas `p0`/`p1` (linear), two-circle `(c0, r0)`/`(c1, r1)` (radial), or `(center, start_deg, end_deg)` (sweep) handles plus intermediate `GradientStop` ticks (`[0.0, 1.0]`) inside atomic Undo transactions, and reports `SelectionInfo::GradientStop { sublayer_index, stop_index, offset_pct, rgba }` + `SelectionInfoAction::MutateGradientStop { offset_pct, rgba }`.
   - `render_colored_thumbnail(&[CanvasColorSublayer], bounds_upm, size_px)` composites all visible sublayers for multi-color grid cells and preview strips (`egui` $\equiv$ `tiny-skia`, `SSIM >= 0.985`).

---

### 2.2 Vertical `TopToBottom` Text Stream & Vertical Metrics Ports (`src/render/vertical_stream.rs`)

1. **Vertical Stream Layout & Metrics Ports (`WritingDirection::TopToBottom`)**:
   - Extends `TextStreamConfig` with `writing_direction: WritingDirection { LeftToRight, RightToLeft, TopToBottom }`.
   - In `TopToBottom` mode, `GlyphSlot`s advance downward from `vert_origin_y` by `advance_height` plus vertical pair kerning; dragging or nudging top/bottom metrics lines (using the configured `GrafiaPreferences` spacing/kerning steps from `SPEC-009`) emits `SpacingDelta { axis: MetricsAxis::Vertical, tsb_delta, bsb_delta, advance_height_delta }` and `KerningDelta { axis: MetricsAxis::Vertical, .. }`.

---

### 2.3 Domain-Agnostic `CustomReferenceFrame` (`src/render/reference_frame.rs`)

1. **Pure 2D Reference Frame Contract**:
   ```rust
   pub struct CustomReferenceFrame {
       pub outer_box: Option<RectUpm>,
       pub inner_box: Option<RectUpm>,
       pub inner_grid_subdivisions: Option<(u8, u8)>,
       pub center_crosshair: bool,
       pub keyline_circles_radii_upm: Vec<f64>,
       pub parallel_lines: Vec<ReferenceBandLine>,
   }

   pub struct ReferenceBandLine {
       pub y_or_x_upm: f64,
       pub horizontal: bool,
       pub thickness: GuideThickness, // ScreenDp hairline or FontUnits(w) zoom-proportional band
       pub dashed: bool,
       pub snap_enabled: bool,
   }
   ```
   - Pass 1 renders `outer_box`, `inner_box`, `inner_grid_subdivisions`, `keyline_circles_radii_upm`, and each `ReferenceBandLine` (honoring `GuideThickness::ScreenDp` via `guide_stroke_width_dp` or `GuideThickness::FontUnits(w)` as a zoom-scaling translucent band), while `SnapEngine` snaps to all active frame edges, subdivisions, and `snap_enabled` band centerlines/edges.

---

### 2.4 PostScript Stem/Ghost Bands & TrueType Instructor Canvas Mode (`src/render/hinting_overlays.rs`)

1. **PostScript Stem & Ghost Hint Bands (`SelectionInfo::PostScriptHint`)**:
   - Renders translucent horizontal stem bands, vertical stem bands, and 1-edge ghost bands (`20`/`21` unit width with `↑`/`↓` polarity badge and `"y, dy"` telemetry pill), supporting click-selection, drag-reanchoring to contour nodes, `SelectionInfo::PostScriptHint { hint_id, horizontal, origin_upm, width_upm, is_ghost, top_ghost }` + `SelectionInfoAction::MutatePostScriptHint { origin_upm, width_upm, top_ghost }`, and `Delete` (`CanvasHostRequest::MutatePostScriptHint`).
2. **TrueType Instructor Mode (`CanvasTool::TrueTypeInstructor` & `SelectionInfo::TrueTypeInstruction`)**:
   - Displays (a) master vector outline, (b) pre-hinted target-PPEM outline, (c) grid-fitted hinted outline over the `ppem` pixel grid + rasterized pixel squares, and (d) interactive instruction arrows/badges (`Snap` `A`, `Stem` `S`, `Shift` `F`, `Interpolate` `G`, `Delta` `E`, X/Y projection axis toggle `X`), exposing `SelectionInfo::TrueTypeInstruction { instruction_id, kind, ppem, round_mode, stem_index, zone_index }` + `SelectionInfoAction::MutateTrueTypeInstruction` and emitting `CanvasHostRequest::MutateTrueTypeInstruction`.

---

### 2.5 Higher-Order Interpolation (`HOI`) Diamond Overlay, `PixelPreviewOverlay` & `BackdropGhostContours` (`src/render/hoi_overlay.rs`)

1. **Control-Polygon Visualization & `HOITiming` Port**:
   - When `show_hoi_diamonds == true` and $\ge 2$ compatible `CompanionMasterLayer`s are loaded, Pass 4 connects corresponding node indices $P_{c,i}^{(0)} \to P_{c,i}^{(1)} \to \dots \to P_{c,i}^{(M-1)}$ with dashed control-net polylines and distinct diamond markers for `prev_layer`, `active_layer`, and `next_layer` so non-linear cubic master tangents are directly inspectable on canvas, and exposes `SelectionInfo::HoiTiming { c1_x, c1_y, c2_x, c2_y }` when the `HOI` tool is active.
2. **Domain-Agnostic `show_pixel_preview_ppem: Option<u16>` & `backdrop_ghost_contours: Vec<GlyphContour>`**:
   - When `show_pixel_preview_ppem == Some(ppem)` (independent of `CanvasTool::TrueTypeInstructor`), Pass 1 rasterizes the active layer at `ppem` and draws translucent pixel squares behind the vector outline for live low-resolution proofing while drawing with any tool.
   - `backdrop_ghost_contours: Vec<GlyphContour>` renders host-supplied read-only reference silhouettes in Pass 1 using `theme.backdrop_ghost_fill` (enabling host applications to display kerning-group peers or reference shapes without introducing font-database concepts into `grafia`).

---

## 3. Complete Normative Test Matrix (`TC-V090-001` .. `TC-V090-013`)

All 13 normative test cases are anchored in `tests/v090_complete_tests.rs`:

| Test ID | Subsystem | Verification Scenario | Expected Invariant |
| :--- | :--- | :--- | :--- |
| **`TC-V090-001`** | Multi-Layer `Solid` + `LinearGradient` + `ClippingMask` Compositing | Stack 3 `CanvasColorSublayer`s (clipped linear gradient + solid highlight) and rasterize via `egui` and `tiny-skia`. | Pixels outside `ClippingMask` remain transparent; `egui` vs. `tiny-skia` `SSIM >= 0.985`. |
| **`TC-V090-002`** | `RadialGradient`, `SweepGradient` & `Bitmap` Sublayer Handles | Drag radial `r1` handle and intermediate `GradientStop` (`0.35 -> 0.65`) + `Undo`. | Updates stop offset clamped to `[0.0, 1.0]`, updates rendered gradient smoothly, and restores exact prior stops on `Undo`. |
| **`TC-V090-003`** | `render_colored_thumbnail` Multi-Layer Projection | Render $64 \times 64$ thumbnail of a 4-layer colored glyph in Light & Dark themes. | Non-empty RGBA buffer matching full canvas aspect & layer z-order without panics. |
| **`TC-V090-004`** | Vertical `TopToBottom` Caret & Slot Advancement | Load 3 `GlyphSlot`s (`advance_height = 1000`) in `TopToBottom` mode and step caret with `ArrowDown`/`ArrowUp`. | Slots stack along $x_c = 500$, Y-origins step by `-1000`, and double-click activates target slot. |
| **`TC-V090-005`** | Vertical `TSB`/`BSB` & Vertical Kerning Shortcuts | Dispatch vertical spacing/kerning arrow shortcuts in `TopToBottom` mode. | Emits `SpacingDelta` and `KerningDelta` with `axis == MetricsAxis::Vertical` and atomic `"Change Vertical Metrics"` Undo. |
| **`TC-V090-006`** | `CustomReferenceFrame` Outer/Inner Boxes & $3 \times 3$ Subdivisions | Configure `outer_box = [0, -120, 1000, 880]`, `inner_box = [60, -60, 940, 820]`, `inner_grid_subdivisions = (3, 3)` and snap node. | Renders outer/inner boxes + $3 \times 3$ lattice and snaps within `snap_radius_dp` to inner-box & subdivision intersections. |
| **`TC-V090-007`** | `CustomReferenceFrame` Parallel `FontUnits` Bands & Keyline Circles | Configure 5 horizontal `ReferenceBandLine`s (`y = 0, 250, 500, 750, 1000`, `GuideThickness::FontUnits(32.5)`) + 2 dashed lines (`-250, 1250`) + keyline circle. | Bands scale proportionally with zoom (`32.5 * zoom` px), `SnapEngine` snaps to band centers & $\pm 16.25\;\text{UPM}$ edges, and `egui` $\equiv$ `tiny-skia` (`SSIM >= 0.985`). |
| **`TC-V090-008`** | PostScript Horizontal/Vertical Stem & Ghost Hint Bands | Attach 2 horizontal stem hints + 1 ghost hint (`width = 21`), select and press `Delete`. | Renders distinct horizontal/vertical/ghost bands with telemetry pills and emits `MutatePostScriptHint` on delete. |
| **`TC-V090-009`** | `TrueTypeInstructor` PPEM Grid, `X` Axis Toggle & Instruction Badges | Enter `CanvasTool::TrueTypeInstructor` at `16 PPEM`, toggle `X` axis, and add `Stem` (`S`) + `Delta` (`E`) link between 2 nodes. | Projects exact `16 PPEM` pixel grid (`62.5 UPM/px` at `1000 UPM`), toggles projection vector `(1,0) <-> (0,1)`, and emits `MutateTrueTypeInstruction`. |
| **`TC-V090-010`** | `HOI` Control-Polygon Diamond Overlay | Load 4 compatible `CompanionMasterLayer`s forming a non-linear cubic master trajectory with `show_hoi_diamonds = true`. | Renders 3-segment control-net polyline per node and distinct `prev`/`current`/`next` diamond markers without panics. |
| **`TC-V090-011`** | Property-Based `proptest` Color Stops, PPEM Scales & Vertical Metrics | Fuzz gradient stop counts (`2..8`), `ppem` (`8..144`), and `advance_height` (`200..2000`). | Zero `NaN`/`Inf` screen projections or division-by-zero panics across all zoom levels. |
| **`TC-V090-012`** | Stateful `arbtest` Color, Vertical Metrics & Hinting Undo/Redo | Fuzz gradient drags, sublayer visibility toggles, vertical sidebearing edits, and `Undo`/`Redo`. | `state.validate_invariants()` and bit-exact Undo/Redo roundtrips hold at every step. |
| **`TC-V090-013`** | `show_pixel_preview_ppem` & `backdrop_ghost_contours` Pass-1 Overlays | Enable `show_pixel_preview_ppem = Some(24)` + load 2 `backdrop_ghost_contours` while `CanvasTool::Pen` is active. | Renders 24 PPEM raster grid + ghost silhouettes in Pass 1 without blocking `Pen` interaction (`SSIM >= 0.985`). |
