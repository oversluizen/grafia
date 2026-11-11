# Changelog

All notable changes to `grafia` are documented in this file in accordance with [Keep a Changelog](https://keepachangelog.com/en/1.1.0/) and Semantic Versioning (`SemVer 2.0.0`).

## [0.4.0] - 2026-11-11

### Added
- **Data-In / Event-Out Host Bridge (`GrafiaCanvasState::load_glyph` & `export_glyph`)**:
  - Application-agnostic glyph and metric loading/exporting with automatic degenerate `OffCurve` normalization and invariant verification.
- **Normative Specification (`SPEC-005-V0.4-EDIT-TOOLS-AND-ANNOTATIONS.md`) & Specialized Edit Tools**:
  - **Boolean Path Operations (`remove_overlap_in_contours` `Cmd/Ctrl + Shift + O`, `subtract_contours`, `intersect_contours`)**: True cubic Bézier and line segment intersection graph solver, self-intersection splitting, inner-segment pruning, and Non-Zero Winding contour reconstruction.
  - **`Knife` (`E` / `Shift + E`)**: Line-segment and recursive De Casteljau line-cubic intersection solver, multi-hit and counter-ring closed-contour slicing (`knife_cut_paths_in_layer`), single-hit contour opening, `Alt/Option` node insertion without slicing (`knife_add_nodes_in_layer`), `Shift` $15^\circ$ angle constraint, and orange slice-line + intersection dot overlay.
  - **`Primitives` (`F` / `Shift + F`)**: Rectangle and 4-anchor cubic Circle ($\kappa = 0.5522847498307936$) placement, `Shift` 1:1 aspect lock, `Alt/Option` center-outward expansion, `Space` origin panning during drag, live `"W × H"` dimension pill, and single-click `ToolDialogRequest::Primitive` (`100 × 100 UPM`).
  - **`Measurement` (`L` & temporary `Cmd + Ctrl + Opt`)**: Selected-node Blue `OnCurve` `(x, y)` labels, Green `OffCurve` `(Δx, Δy)` handle offset labels, Red segment-midpoint `Δx, Δy` span labels, active ruler ray intersecting LSB/RSB and all contours, node snapping, `Shift` $15^\circ$ constraint, typographic vertical angle (`atan2(dx, dy)` in `(-180°, 180°]`), and `'G'` *Add Measurement Line as Guide*.
  - **`Annotation` (`A` / `Shift + A`) & `CanvasAnnotation` Model**: All 5 annotation variants (`Text`, `Arrow`, `Circle`, `Plus`, `Minus`), interactive handle manipulation (circle radius, arrow angle/length, text width), `Shift`-constrained body dragging, `Delete`/`Backspace`, and `Cmd/Ctrl + X/C/V` annotation clipboard operations with full `CommandHistory` Undo/Redo support.
  - **`Pencil` (`B`)**: Freehand path sampling (`PENCIL_MIN_SAMPLE_DIST_DP`), automatic start-point closure within `PENCIL_CLOSE_RADIUS_DP`, and least-squares Schneider cubic Bézier curve fitting (`fit_freehand_contour_nodes`).
  - **`Pixel` (`X`)**: Grid-cell contour stamping (`build_rectangle_contour_nodes`), automatic erase-mode toggle when clicking an occupied cell, and `Shift + Click` Bresenham line placement (`bresenham_pixel_cells`).
  - Full Light (`GrafiaTheme::sonoma_default()`) and Dark (`GrafiaTheme::sonoma_dark()`) mode rendering parity across both `egui::Painter` and `tiny_skia::Pixmap`.
- **Forward Specifications & RED COTS Suites (`v0.5.0` through `v0.9.0`)**:
  - Authored `SPEC-006` (`v0.5.0`), `SPEC-007` (`v0.6.0`), `SPEC-008` (`v0.7.0`), `SPEC-009` (`v0.8.0`), and `SPEC-010` (`v0.9.0`) along with their gated RED COTS verification suites (`tests/v050_complete_tests.rs` .. `tests/v090_complete_tests.rs`).

## [0.1.0] - 2026-11-08

### Added
- **Core Vector & Coordinate Engine (`src/core/`)**:
  - Type-safe `FontUnit`, `FontPoint`, `ScreenDp`, `ScreenPoint`, and `ViewportTransform` with cursor-centered zoom (`zoom_at_cursor`).
  - Cubic Bézier segment extraction (`extract_segments`), De Casteljau subdivision (`split_contour_segment`), horizontal/vertical extremum solver (`find_nearest_cubic_extremum`), and topological curve-fitting node deletion (`heal_delete_nodes`, `cut_open_delete_nodes`).
  - 4-case inverse-Bernstein cubic belly dragging (`apply_cubic_segment_belly_drag_with_alt`) with `Smooth` tangent line projection and $G^1$ collinear opposite-handle alignment.
  - Non-Zero Winding contour direction operations: `reverse_contour_nodes` (`Cmd/Ctrl + Shift + R`) and `make_selected_on_curve_node_first`.
- **Interactive Tool State Machines & Atomic History (`src/interaction/`)**:
  - `Select` (`V`) supporting single/multi-selection, `Shift` XOR marquee, `Alt` on-curve-only marquee, double-click contour selection, triple-click select-all-contours, two-stage `Cmd/Ctrl + A`, `Alt + Drag` single-anchor decoupling, `Alt + Drag` contour duplication (`alt_drag_duplicate`), and `Ctrl + Alt + Drag` proportional handle tension.
  - `Pen` (`P`) supporting persistent cubic Bézier construction (`bcp_in`/`bcp_out`), `Alt` cusp-breaking during handle drag, `Shift` extremum-snapping on curve segments, open contour resumption/closing, and temporary `Cmd` override to `Select`.
  - Atomic `CommandHistory` (`Undo` / `Redo`) capturing full `GlyphSnapshot` (`contours`, `closed_flags`, `paths`, `selected_nodes`) across all mouse and keyboard mutations.
- **Multi-Pass Rendering & Live `egui` / Headless `tiny-skia` Parity (`src/render/`, `src/adapters/`, `src/locator/`)**:
  - 5-pass `CanvasPainter` (`egui`) and `headless` (`tiny-skia`) with 1-to-1 parity across `EditWireframe`, `PreviewFilled` (Non-Zero Winding `FillRule::NonZero` / `FillRule::Winding`), and `ShapeThumbnail`.
  - Full typographic metrics box (`LSB`, `RSB`, `Ascender` $800$, `Cap Height` $700$, `x-Height` $500$, `Baseline` $0$, `Descender` $-200$), blue start-node direction arrow triangle, perpendicular open-contour endpoint ticks, single-point `X` cross, and `Pen` ghost-node preview circle.
  - Two-layer `ShapeLocator` vector minimap (`ShapeLocatorRenderer` & `ShapeLocatorWidget`) with Non-Zero Winding and `CCW`/`CW` winding labels.
- **Architecture & Clean-Room Guardrails (`tests/architecture_guard_tests.rs`)**:
  - Automated COTS enforcement of Zero Magic Numbers, Modularity (< 400 eSLOC / < 599 physical lines), Zero Unused Dependencies in `Cargo.toml`, Windows GUI Subsystem attribute, Zero Hardcoded Colors, and 100% Clean-Room repository hygiene (`tc_arch_008_clean_room_source_and_tests_guard`).
