# `grafia` — Universal Interactive Glyph & Cubic Bézier Vector Canvas Engine (`v0.7.0`)

`grafia` is a standalone, application-agnostic Rust 2D cubic Bézier contour, glyph, and vector canvas engine built on Linebender (`kurbo`), `lyon_tessellation`, `egui`, and `tiny-skia`.

---

## 1. Architecture & Scope (`SPEC-001` .. `SPEC-010`)

`grafia` serves as a dedicated **2D Vector Canvas View & Controller** component (`GrafiaCanvasState`, `GrafiaCanvasWidget`, `GrafiaPreviewWidget`, `PreviewStripCache`, `GrafiaTheme`) with a strict **Hexagonal Data-In / Event-Out Host Bridge** (`load_glyph`, `export_glyph`, `drain_host_requests`, `drain_spacing_deltas`, `drain_kerning_deltas`, `compute_selection_info`, `apply_selection_info_action`, `GrafiaCanvasResponse`).

- **Implemented in `v0.1.0` through `v0.7.0`**:
  - **Background Layer, 2D Anchors, Pre-Resolved Components, Fit Curve, Live Contour Stroke & `CompanionMasterLayer` (`v0.7.0` — `SPEC-008`)**:
    - Foreground/Background contour layer workflow (`Cmd/Ctrl + J` selection-to-background, `Cmd/Ctrl + K` layer swap, background editing toggle).
    - 2D `CanvasAnchor` placement, `Tab` cycling, magnetic `SnapEngine` integration, `GhostMarkPath` mark cloud overlay, and `CanvasHostRequest::ResetDefaultAnchors`.
    - Pre-resolved `CanvasComponent` rendering, locking, auto-alignment, `decompose_selected_components` (`Cmd/Ctrl + Shift + D`), and double-click `CanvasHostRequest::OpenComponentSource`.
    - 8-step `fit_selected_curves_by_step` (`57%..92%`), live `ContourStrokeStyle` offset preview, destructive `expand_selected_contour_strokes`, and `CompanionMasterLayer` interpolation compatibility diagnostics.
  - **Separable Blur Preview Strip, `PreviewStripCache`, Pure 2D `SelectionInfo`, Rulers & Background Image (`v0.6.0` — `SPEC-007`)**:
    - `render_preview_strip_rgba` and generation-gated `PreviewStripCache` (`GrafiaCanvasState::content_generation()`) for zero-CPU / zero-GPU idle frame overhead.
    - Pure 2D 5-mode `SelectionInfo` inspector projection, `SelectionInfoAction`, top/left UPM rulers (`Cmd/Ctrl + R`), and `CanvasBackgroundImage`.
  - **Multi-Glyph Horizontal Text Stream, Caret Navigation & Interactive Spacing/Kerning (`v0.5.0` — `SPEC-006`)**:
    - `CanvasTool::Text` (`T`), `GlyphSlot` horizontal text run, double-click slot activation, and `SpacingDelta` / `KerningDelta` / `TextStreamEditRequest` host ports.
  - Interactive single-glyph vector editing with `Select` (`V`), `Pen` (`P`), `Rotate` (`R`), `Scale` (`S`), `CanvasTool::Lasso` (`Shift + V`), `Zoom` (`Z` / `Cmd/Ctrl + Space`), `Knife` (`E` / `Shift + E`), `Primitives` (`F` / `Shift + F`), `Measurement` (`L` / `Cmd + Ctrl + Opt`), `Annotation` (`A` / `Shift + A`), `Pencil` (`B`), `Pixel` (`X`), and `Text` (`T`).
  - **Specialized Edit View Tools, Boolean Path Operations & Canvas Annotations (`v0.4.0` — `SPEC-005`)**:
    - **Boolean Path Operations (`remove_overlap_in_contours` `Cmd/Ctrl + Shift + O`, `subtract_contours`, `intersect_contours`)**: Cubic Bézier and line segment intersection graph solver, cycle extraction, inner-segment pruning, and Non-Zero Winding outer/counter reconstruction.
    - **`Knife` (`E` / `Shift + E`)**: Recursive De Casteljau line-segment and line-cubic intersection solver, multi-hit and counter-ring closed-contour slicing (`knife_cut_paths_in_layer`), single-hit contour opening, `Alt/Option` node insertion without slicing (`knife_add_nodes_in_layer`), `Shift` $15^\circ$ angle constraint, and orange slice-line + intersection dot overlay (`r = 3.0 dp`).
    - **`Primitives` (`F` / `Shift + F`)**: Rectangle and 4-anchor $\kappa \approx 0.55228475$ circle/ellipse placement with `Shift` 1:1 aspect lock, `Alt/Option` center-outward expansion, `Space` origin panning during drag, `"W × H"` dimension pill, and single-click `ToolDialogRequest::Primitive` (`100 × 100 UPM`).
    - **`Measurement` (`L` & temporary `Cmd + Ctrl + Opt`)**: Selected-node Blue `OnCurve` `(x, y)` labels, Green `OffCurve` `(Δx, Δy)` handle offset labels, Red segment-midpoint `Δx, Δy` span labels, active ruler ray intersecting LSB/RSB and all contours, node snapping, `Shift` $15^\circ$ constraint, typographic vertical angle (`atan2(dx, dy)` in `(-180°, 180°]`), and `'G'` *Add Measurement Line as Guide*.
    - **`Annotation` (`A` / `Shift + A` & `CanvasAnnotation`)**: All 5 annotation kinds (`Text`, `Arrow`, `Circle`, `Plus`, `Minus`), interactive handle manipulation, `Shift`-axis-constrained dragging, `Delete`/`Backspace`, and `Cmd/Ctrl + X/C/V` clipboard support.
    - **`Pencil` (`B`)**: Freehand sketching with Ramer-Douglas-Peucker simplification, automatic closure detection on `P0`, and least-squares Schneider cubic Bézier curve fitting (`fit_freehand_contour_nodes`).
    - **`Pixel` (`X`)**: Grid-cell stamping and erasing (`remove` toggle on occupied cells) plus `Shift + Click` Bresenham line placement (`bresenham_pixel_cells`).
  - **Canvas Grid, Local & Global Guides, Magnetic `SnapEngine`, Alignment Zones & Light/Dark/System Appearance (`v0.3.0` — `SPEC-004`)**.
  - **8-Knob Selection Bounding Box (`Cmd/Ctrl + Alt + Shift + B`)**, **Canvas Outline Clipboard (`Cmd/Ctrl + C/X/V`)**, and **Topological Path Operations (`v0.2.0` — `SPEC-003`)**: `reverse_selected_contours`, `make_node_first`, `correct_path_direction` (`Cmd/Ctrl + Shift + R`), `add_extremes`, `add_inflection_points`, `tidy_up_contours` (`Cmd/Ctrl + Shift + T`), `align_selected_nodes` (`Cmd/Ctrl + Shift + A`), `connect_selected_open_endpoints_in_contours`, `close_open_contours_in_layer`, duplicate-node warning rings, and `TransformGridAnchor` (`0..8`) / `MetricsOrigin` (`0..4`) transforms.
  - **Core Cubic Bézier Topology & 5-Pass Rendering (`v0.1.0` — `SPEC-002`)**: Persistent cubic Bézier construction, De Casteljau segment splitting, extremum snapping ($dx/dt = 0, dy/dt = 0$), curve-healing node deletion (`heal_delete_nodes`), 4-case inverse-Bernstein `drag_cubic_segment` belly dragging, Non-Zero Winding (`FillRule::NonZero`), 3 view modes (`EditWireframe`, `PreviewFilled`, `ShapeThumbnail`), 2-layer `ShapeLocator` minimap, atomic `CommandHistory` (`Undo` / `Redo`), and 1-to-1 rendering parity between `egui::Painter` and `tiny_skia::Pixmap`.
- **Normatively Specified & Anchored with RED COTS Tests (`v0.5.0` through `v0.9.0`)**:
  - `SPEC-006` (`v0.5.0` — Multi-Glyph Horizontal Text Stream & Kerning)
  - `SPEC-007` (`v0.6.0` — Separable Gaussian Blur Preview, 5-Mode `SelectionInfo` & Canvas Rulers)
  - `SPEC-008` (`v0.7.0` — Background Layer, 2D Anchors, Pre-Resolved Components, Fit Curve, Live Contour Stroke & `CompanionMasterLayer` Compatibility)
  - `SPEC-009` (`v0.8.0` — $G^2$ Curve Harmonization, Tangent Balance Lines, Proportional Handle Servo, Interpolated Nudge, Master Trajectory Kinks & Proactive Outline Audit)
  - `SPEC-010` (`v0.9.0` — Non-Destructive Smart Corners, Ink Traps, W3C Stylus Calligraphic Brush, Curvature Heatmap Envelope, Orthogonal Stem Thickness Gauge, Green's Theorem Counter Balance & Symmetry Overlays)

---

## 2. Module Overview (`src/`)

| Module | Responsibility |
| :--- | :--- |
| `src/core/` | Type-safe coordinates (`FontUnit`, `FontPoint`, `ScreenDp`, `ScreenPoint`, `ViewportTransform`), cubic Bézier math, `boolean_ops`, `knife_ops`, `primitives_ops`, `measurement_ops`, `annotations`, `extract_segments`, `heal_delete_nodes`, `segment_ops`, `nudge_ops`, and `transform_ops`. |
| `src/interaction/` | Finite state machines (`SelectToolState`, `PenToolState`), `Modifiers`, and atomic `CommandHistory` (`GlyphSnapshot`). |
| `src/render/` | GPU tessellation (`tessellate_contours_nonzero`), projections (`ProjectedNode`, `PreviewFilledProjection`, `ShapeThumbnailProjection`), start-arrow and open-endpoint geometry, and `v020`/`v030`/`v040` overlays. |
| `src/locator/` | `ShapeLocatorRenderer` & `ShapeLocatorWidget` 2-layer vector minimap with Non-Zero Winding and `CCW`/`CW` direction indicators. |
| `src/adapters/` | `GrafiaCanvasState` (with `load_glyph` / `export_glyph` host bridge), `GrafiaCanvasWidget`, `CanvasPainter` (5-pass `egui` pipeline), `InputDispatcher`, `invariants` (`validate_invariants`), `cursor_overlay`, `keyboard`, and `headless` (`tiny-skia` offscreen verifier). |
| `src/tokens/` | Data-driven theme, color, metric, cursor, and interaction tokens (`Zero Magic Numbers`, `Zero Hardcoded Colors`). |

---

## 3. Verification & Test Commands

```powershell
# Full COTS test suite:
cargo test

# Architecture & Clean-Room guardrails:
cargo test --test architecture_guard_tests

# Live egui <-> headless tiny-skia visual parity & host bridge:
cargo test --test egui_adapter_tests

# Strict static analysis (0 warnings):
cargo clippy --all-targets -- -D warnings

# Launch interactive desktop vector canvas demo:
cargo run --example vector_canvas_demo
```
