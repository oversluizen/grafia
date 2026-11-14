# Changelog

All notable changes to `grafia` are documented in this file in accordance with [Keep a Changelog](https://keepachangelog.com/en/1.1.0/) and Semantic Versioning (`SemVer 2.0.0`).

## [0.9.0] - 2026-11-14

### Added
- **Multi-Layer Vector Color/Gradients/Bitmaps, Vertical `TopToBottom` Text Stream, Domain-Agnostic `CustomReferenceFrame`, PostScript/TrueType Hinting & Higher-Order Interpolation Overlays (`SPEC-010` / `v0.9.0`)**:
  - Multi-layer vector color stack (`CanvasColorSublayer`, `ColorLayerPaint::{ Solid, LinearGradient, RadialGradient, SweepGradient, Bitmap, ClippingMask }`, `GradientExtend` / `GradientSpread`, `GradientStop`) with Porter-Duff `SrcOver` compositing (`porter_duff_src_over`), interactive on-canvas gradient handle and stop-tick dragging (`"Change Color Gradient"` Undo), `SelectionInfo::GradientStop`, `SelectionInfoAction::MutateGradientStop`, `CanvasHostRequest::MutateColorSublayer`, bounded `BTreeMap` multi-window texture caching in `CanvasRenderCache`, and `render_colored_thumbnail` multi-layer RGBA projection with monochrome fallback.
  - Vertical `WritingDirection::TopToBottom` text stream layout (`recompute_vertical_slot_origins`, vertical center axis $x_c = \frac{1}{2}W$, `vert_origin_y` and `advance_height` progression), `ArrowUp`/`ArrowDown` vertical caret stepping, double-click slot activation, and `TSB`/`BSB` & vertical kerning shortcuts emitting `SpacingDelta` and `KerningDelta` on `MetricsAxis::Vertical` with `"Change Vertical Metrics"` Undo.
  - Domain-agnostic `CustomReferenceFrame` (`outer_box`, `inner_box`, $N_x \times N_y$ `inner_grid_subdivisions`, `center_crosshair`, `keyline_circles_radii_upm`, and parallel `ReferenceBandLine`s supporting `GuideThickness::ScreenDp` and zoom-proportional `GuideThickness::FontUnits(w)`) with magnetic `SnapEngine` integration (`SnapTargetKind::ReferenceFrame` and `SnapTargetMask::reference_frame`).
  - PostScript stem and 1-edge ghost hint bands (`PostScriptHint`, `PostScriptHintAxis`, `20`/`21` UPM ghost widths, `↑`/`↓` polarity badges, `"y, dy"` telemetry pills, click selection, `SelectionInfo::PostScriptHint`, `SelectionInfoAction::MutatePostScriptHint`, and `Delete`/`Backspace` removal with `CanvasHostRequest::MutatePostScriptHint`).
  - TrueType Visual Instructor canvas mode (`CanvasTool::TrueTypeInstructor`, `TrueTypeVisualInstruction`, `TrueTypeInstructionKind::{ Touch/Snap, Anchor, Stem, Shift, Interpolate, Align, Delta }`, `TrueTypeRoundingGrid`, `truetype_upm_per_px`, `grid_fit_contour_nodes`, `X` key projection axis toggle, interactive badge/arrow overlays, `SelectionInfo::TrueTypeInstruction`, `SelectionInfoAction::MutateTrueTypeInstruction`, and `CanvasHostRequest::MutateTrueTypeInstruction`).
  - Higher-Order Interpolation (`HOI`) control-polygon diamond overlay (`show_hoi_diamonds`, `CanvasTool::Hoi`, `HoiSiblingContours`, `HoiCubicLayerSet`, `compute_hoi_overlay_geometry`, `SelectionInfo::HoiTiming`, `SelectionInfoAction::MutateHoiNode`, `CanvasHostRequest::MutateHoiLayerNode`), plus Pass-1 `show_pixel_preview_ppem: Option<u16>` low-resolution proofing grid (`compute_pixel_preview_cells_upm`) and `backdrop_ghost_contours: Vec<GlyphContour>` reference silhouettes.

## [0.8.0] - 2026-11-14

### Added
- **Hobby & Euler/Spiro Interpolating Splines, 8+4 BBox Rotation/Shear, Parametric `CanvasSmartHandle`s, Virtual `ExtraNode`s, Bitmap Auto-Trace & `GrafiaPreferences` (`SPEC-009` / `v0.8.0`)**:
  - `NodeType::HobbyStar` (cyclic Sherman-Morrison tridiagonal mock-curvature solver with incoming/outgoing tensions $\tau \in [0.75, 4.0]$) and `NodeType::EulerSpiro` ($G^2$ damped Newton-Raphson clothoid solver), `HobbyDirConstraint` (`Free`, `Horizontal`, `Vertical`, `SlantedItalic`, `FollowMetricLine`), and curve conversion helpers (`convert_selected_hobby_nodes_to_cubic`, `convert_selected_euler_spiro_nodes_to_cubic`, `convert_selected_cubic_nodes_to_hobby`, `convert_selected_contours_to_quadratic`, `convert_selected_contours_to_cubic`, `add_offcurve_to_selected_segments`, `remove_offcurve_from_selected_segments`).
  - 8+4 Selection Bounding Box outer-corner rotation zone (`[6.0 dp, 18.0 dp]`) with `Shift` $15^\circ$ quantization, live `"∠ ..."` telemetry pill, and `Ctrl+Alt` mid-edge horizontal/vertical shear preserving `Smooth`-node tangent collinearity.
  - Parametric `CanvasSmartHandle` ray sliders on selected `CanvasComponent` instances with `[min_val, max_val]` clamping, `4.0 dp` magnetic snap to `default_val`, and `CanvasHostRequest::UpdateSmartComponentAxis`.
  - Self-intersection virtual `ExtraNode` detection (`compute_layer_extra_nodes`), topological `ExtraNodeKey` stability under rigid translation, hollow diamond overlay markers, and `CanvasHostRequest::SelectOrMutateExtraNode`.
  - Deterministic 7-stage bitmap and shadowed-camera auto-trace (`trace_background_image`, `trace_active_background_image`, `AutoTraceConfig`, `AutoTraceThresholdMode::GlobalLuminance` and `AdaptiveLocal` Sauvola integral-image thresholding, 4-corner $3\times 3$ homography perspective deskew, sub-pixel marching squares, Ramer-Douglas-Peucker simplification, corner/extremum detection, and Schneider least-squares cubic Bézier fitting).
  - `GuideThickness::ScreenDp` vs. zoom-scaling `GuideThickness::FontUnits(f64)` translucent guide bands with centerline and $\pm w_{\text{upm}}/2$ band-edge snapping in `SnapEngine`.
  - `GrafiaPreferences` (custom small/medium/large UPM nudge steps, kerning steps, `round_to_grid`, `snap_radius_dp`, `SnapTargetMask`, `touch_finger_offset_dp`, `show_touch_precision_loupe`, and multi-touch gesture toggles) and `GrafiaKeymap` runtime shortcut rebinding.
  - `SelectionInfo::HobbyNode`, `SelectionInfo::BackgroundImage` (`BackgroundImageMutation`, `ResetBackgroundImageTransform`), `interpolate_with_background(factor)`, `apply_path_op_across_all_layers`, viewport bookmark swap (`save_viewport_bookmark`, `toggle_saved_viewport_bookmark`), granular node/overlay visibility toggles, and `CanvasRenderCache` idle-frame texture and extra-node caching.

## [0.7.0] - 2026-11-11

### Added
- **Background Layer, 2D Anchors, Pre-Resolved Components, Fit Curve, Live Contour Stroke & `CompanionMasterLayer` (`SPEC-008` / `v0.7.0`)**:
  - Background layer editing (`selection_to_background` `Cmd/Ctrl + J`, `swap_with_background` `Cmd/Ctrl + K`, `toggle_editing_background`, `toggle_show_background`) with full `CommandHistory` Undo/Redo snapshots.
  - 2D `CanvasAnchor` model (`add_anchor`, `delete_selected_anchors`, `Tab`/`Shift + Tab` cycling, magnetic `SnapTargetKind::Anchor`, and `GhostMarkPath` mark-to-base cloud preview).
  - Pre-resolved `CanvasComponent` instances (`add_component`, `set_component_locked`, `set_component_alignment`, `decompose_selected_components` `Cmd/Ctrl + Shift + D`, and double-click `CanvasHostRequest::OpenComponentSource`).
  - 8-step `fit_selected_curves_by_step` (`57%..92%` handle tension ladder) and `fit_selected_curves_by_ratio`.
  - Non-destructive `ContourStrokeStyle` (`Center`, `Inside`, `Outside`, `Butt`/`Round`/`Square` caps, `Miter`/`Round`/`Bevel` joins) and destructive `expand_selected_contour_strokes`.
  - `CompanionMasterLayer` compatibility overlay (green compatible vs. red incompatible contour/node badges and `select_all_layers`).
- **Separable Gaussian Blur Preview, Pure 2D `SelectionInfo`, Rulers & Background Image (`SPEC-007` / `v0.6.0`)**:
  - `render_preview_strip_rgba` (`tiny-skia` Non-Zero Winding + 3-pass separable box blur approximating Gaussian $\sigma \in [0, 20]$, invert toggle, and aspect-ratio containment).
  - Battery-efficient `PreviewStripCache` and `GrafiaCanvasState::content_generation()` ensuring zero redundant CPU rasterization or GPU texture uploads on idle frames, hover, pan, zoom, or selection changes.
  - Pure 2D `SelectionInfo` model (`Empty`, `SingleNode`, `SingleAnchor`, `SingleComponent`, `MultipleSelection`) and `apply_selection_info_action`, plus top/left canvas rulers (`toggle_rulers` `Cmd/Ctrl + R`) and `CanvasBackgroundImage`.
- **Multi-Glyph Horizontal Text Stream, Caret Navigation & Interactive Spacing/Kerning (`SPEC-006` / `v0.5.0`)**:
  - `CanvasTool::Text` (`T`), `GlyphSlot` horizontal strip layout, caret navigation, double-click slot activation, and live `SpacingDelta` / `KerningDelta` / `TextStreamEditRequest` host-port emissions.
- **Hexagonal Host-Port ("Stekkers") Testbank (`examples/vector_canvas_demo.rs` & `examples/demo_host_shell/`)**:
  - Interactive `MockFontHost` exercising all 10 Data-In / Event-Out ports (`CanvasHostRequest`, `SpacingDelta`, `KerningDelta`, `SelectionInfoAction`, `PreviewStripCache`, `CompanionMasterLayer`, `GhostMarkPath`, `CanvasComponent`, `CanvasAnchor`, and `Glyph`).

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
