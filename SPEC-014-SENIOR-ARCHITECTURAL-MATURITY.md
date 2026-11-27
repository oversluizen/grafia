---
type: specification
id: SPEC-014
version: 0.9.1
status: normative
parent: SPEC-001
---

# SPEC-014: `grafia v0.9.1` — Senior Architectural Maturity, Zero Tuple-Parameter Smuggling, Cohesive Sub-State Decomposition, Live-Drag 60fps Pixmap Scaling & Host Document Scene API

> **Normative References**:
> - `SPEC-001`..`SPEC-010`: `grafia v0.1.0`..`v0.9.0` Architecture, Coordinate Pipeline, Bézier Topology, Guides, Tools, Text Stream, Preview, Layers, Splines, Auto-Trace, Color Sublayers & Hinting
> - `STD-ARCH`: Industry Software Engineering & Testing Standards (NASA JPL *Power of Ten*, Google Safe Rust, IBM Cleanroom Software Engineering)
> - Standards: `ISO/IEC/IEEE 42010:2011` (Architecture Description), `ISO/IEC 25010:2023` (Software Product Quality — Maintainability, Modularity & Performance Efficiency), `IEEE 1012-2016` (System & Software Verification and Validation)

---

## 1. Executive Summary & Senior Code Review Findings

Following the completion of `grafia v0.9.0` (`SPEC-010`) and the 15 static architectural guardrails (`tc_arch_001`..`tc_arch_015`), an adversarial Principal/Senior Rust Architecture Review identified four semantic and structural maturity gaps that can slip past purely syntactic line-count guards (*Goodhart's Law*):

| Finding ID | Senior Review Finding | Root Cause | Normative Solution in `SPEC-014` |
| :--- | :--- | :--- | :--- |
| **`FINDING-01`** | **Tuple-Parameter Smuggling (`(A, B, C, ...)` in `fn` signatures)** | To satisfy `clippy::too_many_arguments` (`<= 7`), 29 internal functions bundled 3 to 6 positional primitives (`(bool, bool, bool, bool)`, `(f32, f32, f32, f32, f32, f32)`, `(Color32, Color32, Color32, Color32)`) into unnamed tuples, sacrificing type safety and parameter readability. | Enforce `too_many_arguments = "deny"` in `Cargo.toml` `[lints.clippy]` AND forbid any function parameter with an anonymous tuple of $\ge 3$ elements (`tc_arch_016`). Replace all 29 anonymous tuple bundles with named, self-documenting domain `struct`s. |
| **`FINDING-02`** | **Flat State & Snapshot Accumulation (`GrafiaCanvasState` & `GlyphSnapshot`)** | Across `v0.1.0`..`v0.9.0`, 49 internal interaction/drag/pen transient fields were appended directly onto `GrafiaCanvasState` (reaching 135 flat fields) and `GlyphSnapshot` grew to 39 flat fields. | Extract internal interaction transients into cohesive, typed sub-structs (`PenInteractionTransients`, `DragSessionTransients`, `TextInteractionTransients`, `TouchAndKeymapState`) and group `GlyphSnapshot` sub-domains so no domain/snapshot struct exceeds `MAX_DOMAIN_STRUCT_FIELDS = 35` and `GrafiaCanvasState` eliminates flat transient clutter (`tc_arch_016`). |
| **`FINDING-03`** | **Unbounded 4K CPU Scanline Rasterization During Live Pointer Drag (`COLRv1` / Pixmap Passes)** | While `CanvasRenderCache` prevents GPU texture re-uploads on idle frames (`0` uploads), dragging a node or gradient handle on a 4K (`3840 by 2160`) viewport increments `content_generation` every frame, forcing CPU scanline rasterization of $8.29\times 10^6$ pixels at 60fps. | Introduce `MAX_INTERACTIVE_DRAG_PIXMAP_DIM: u32 = 512`, `GrafiaCanvasState::is_interactive_drag_active(&self) -> bool`, and proportional live-drag downscaling (`effective_color_layer_pixmap_dimensions`). During active pointer drags, rasterize at most `512` px on the longest axis with linear GPU upscaling (`> 56x` pixel reduction on 4K), restoring 100% native resolution on pointer release (`TC-MATURITY-003`, `TC-MATURITY-004`). |
| **`FINDING-04`** | **Host Application Integration Ergonomics (Dogfooding Surface)** | Host applications loading or saving a glyph scene had to read and write dozens of individual fields on `GrafiaCanvasState`. | Provide a cohesive `CanvasDocumentScene` aggregate in `src/core/` with typed domain sections (`glyph`, `metrics`, `guides`, `layers`, `color_layers`, `hinting`, `reference_frame`) and atomic `load_document_scene(&mut self, scene: CanvasDocumentScene)` / `export_document_scene(&self) -> CanvasDocumentScene` roundtrip methods (`TC-MATURITY-005`). |

---

## 2. Normative Architectural & Functional Requirements

### 2.1 Zero Tuple-Parameter Smuggling & Strict Parameter Lint (`tc_arch_016`)

1. **Compiler Lint Enforcement**:
   - `Cargo.toml` `[lints.clippy]` (or `[workspace.lints.clippy]`) MUST explicitly declare `too_many_arguments = "deny"`.
2. **Ban on Anonymous $\ge 3$-Element Tuple Function Parameters**:
   - No `fn` signature in `src/` may declare a parameter whose type is a tuple (or reference to a tuple) containing 3 or more comma-separated elements (`(T1, T2, T3, ...)`).
   - 2-element coordinate or range pairs (such as `(f64, f64)` or `(usize, usize)`) remain permitted where idiomatic, but any grouping of 3 or more values MUST use a dedicated, named `struct` with named fields (e.g., `ReferenceFramePalette`, `RulerTickRangeUpm`, `RulerBarGeometryDp`, `NodeGlyphRadiiDp`, `SegmentBellyDragTarget`).

### 2.2 Cohesive Sub-State Decomposition & Struct Field Ceiling (`tc_arch_016`, `TC-MATURITY-001`)

1. **Domain & Snapshot Struct Field Limit (`MAX_DOMAIN_STRUCT_FIELDS = 35`)**:
   - No domain, token, or undo-snapshot `struct` in `src/` (including `GlyphSnapshot` and `GrafiaTheme`) may exceed `35` fields (`<= 35`), `DragSessionTransients` MUST NOT exceed `25` fields (`<= 25`), and `GrafiaCanvasState` MUST NOT exceed `88` top-level fields (`<= 88`).
2. **Cohesive Transient & Session Sub-Structs on `GrafiaCanvasState`**:
   - Internal pointer, pen, marquee, and text-drag transient fields on `GrafiaCanvasState` MUST be grouped into cohesive sub-structs:
     - `PenInteractionTransients` (`drag_last_pt`, `handle_delta`, `alt_frozen_bcp_in`, `seg_has_explicit_h1`, `resuming_corner_line_end`, `join_b_had_out_handles`, `join_b_was_smooth`).
     - `DragSessionTransients` (`drag_start_font_positions`, `drag_start_node_types`, `drag_start_contours`, `last_click_time_pos`, `last_double_click_time_pos`, `marquee_pre_selection`, `marquee_modifiers`, `alt_marquee_pending_start`, `active_drag_before_snapshot`, `pending_segment_click`, `alt_duplicate_pending`, `temp_tool_before_override`, `last_modifiers`, `temp_zoom_via_key_space`, `ruler_drag_is_horizontal`, `pre_all_layers_select_snapshot`, `saved_viewport`, `prev_view_mode`, `active_smart_handle_drag`, `active_gradient_drag`, `guide_drag`, plus cohesive `session_config: SessionAndHostState`).
     - `SessionAndHostState` (`next_guide_id`, `next_anchor_id`, `selected_companion_nodes`, `selected_color_sublayer`, `selected_gradient_stop`, `truetype_x_axis_projection`, `hoi_layer_sets`, `hoi_sibling_contours`, `touch_state`, `keymap`, `fit_curve_config`, `preview_config`, `host_requests`).
     - `TextInteractionTransients` (`temp_show_text_measurement_line`, `text_mode_numbers_threshold`, `last_select_tool`, `text_drag_anchor_caret`, `suppress_text_stream_sync`, `text_tool_activated_in_frame`).
   - Hardware controller port state (`controller_ports: ControllerPortState`, `SPEC-015`) MUST live on `GrafiaCanvasState` (not inside `DrawingToolState`), and overlay/hinting display flags MUST be grouped into `CanvasOverlayDisplayState` so `GrafiaCanvasState` remains $\le 88$ top-level fields.
   - `GrafiaCanvasState::validate_invariants(&self)` MUST verify `self.smart_corners == self.curve_harmony.smart_corners` so non-destructive Smart Corner specs can never drift out of sync.
3. **Cohesive `GlyphSnapshot` Sub-Structs**:
   - `GlyphSnapshot` in `src/interaction/history.rs` MUST group auxiliary layer, guide, and hinting/HOI snapshot state into typed sub-snapshots (`SnapshotLayerAttachments`, `SnapshotHintingAndHoiState`) so `GlyphSnapshot` has $\le 25$ fields while preserving bit-exact `Undo -> Redo` roundtrips across all canvas mutations.

### 2.3 Live-Drag 60fps Interactive Pixmap Downscaling (`TC-MATURITY-003`, `TC-MATURITY-004`)

1. **Interactive Drag Detection (`GrafiaCanvasState::is_interactive_drag_active(&self) -> bool`)**:
   - Returns `true` when and only when the user is actively dragging a canvas element that mutates scene or viewport geometry in real time:
     - `matches!(self.select_state, SelectToolState::DraggingNodes { .. } | SelectToolState::DraggingSegment { .. })`
     - `matches!(self.pen_state, PenToolState::PlacingAnchor { .. } | PenToolState::DraggingHandle { .. })`
     - `self.bbox_drag != BBoxDragState::Idle`
     - `self.guide_drag != GuideDragState::Idle`
     - `self.active_gradient_drag.is_some()`
     - `self.active_smart_handle_drag.is_some()`
2. **Proportional Live-Drag Pixmap Cap (`MAX_INTERACTIVE_DRAG_PIXMAP_DIM = 512`)**:
   - Define `pub const MAX_INTERACTIVE_DRAG_PIXMAP_DIM: u32 = 512;` in `src/core/math_consts.rs` (re-exported at crate root).
   - Provide `GrafiaCanvasState::effective_color_layer_pixmap_dimensions(&self, viewport_w: u32, viewport_h: u32) -> (u32, u32, f32)`:
     - When `!self.is_interactive_drag_active()` or `viewport_w.max(viewport_h) <= MAX_INTERACTIVE_DRAG_PIXMAP_DIM`, returns `(viewport_w.max(1), viewport_h.max(1), 1.0)`.
     - When `self.is_interactive_drag_active()` and `max_dim = viewport_w.max(viewport_h) > MAX_INTERACTIVE_DRAG_PIXMAP_DIM`, computes `scale = (MAX_INTERACTIVE_DRAG_PIXMAP_DIM as f32) / (max_dim as f32)` and returns proportionally scaled `(eff_w, eff_h, scale)` where `eff_w.max(eff_h) == MAX_INTERACTIVE_DRAG_PIXMAP_DIM` and `eff_w >= 1`, `eff_h >= 1`.
   - `get_or_upload_color_sublayers_texture` in `src/render/canvas_render_cache.rs` MUST use `effective_color_layer_pixmap_dimensions(width, height)` and scale the `ViewportTransform` (`zoom * scale`, `pan_offset * scale`) during rasterization so a 4K (`3840 by 2160`) drag frame rasterizes `512 by 288` pixels instead of `3840 by 2160` (`> 56x` speedup), and immediately restores `3840 by 2160` when the drag ends.

### 2.4 Host Application Document Scene API (`CanvasDocumentScene` — `TC-MATURITY-005`, `TC-MATURITY-009`)

1. **Cohesive `CanvasDocumentScene` Aggregate (`src/core/document_scene.rs`)**:
   - Define `CanvasDocumentScene` with grouped, domain-named sub-structs:
     - `pub glyph: Glyph`
     - `pub metrics: GlyphMetrics`
     - `pub guides: CanvasGuideSet` (`local_guides: Vec<Guide>`, `global_guides: Vec<Guide>`, `show_guides: bool`, `lock_guides: bool`)
     - `pub layers: CanvasLayerAttachments` (`background_contours: Vec<GlyphContour>`, `anchors: Vec<CanvasAnchor>`, `components: Vec<CanvasComponent>`, `contour_strokes: BTreeMap<ContourId, ContourStrokeStyle>`, `companion_master_layers: Vec<CompanionMasterLayer>`, `smart_handles: Vec<CanvasSmartHandle>`, `background_image: Option<CanvasBackgroundImage>`, `backdrop_ghost_contours: Vec<GlyphContour>`)
     - `pub color_layers: Vec<CanvasColorSublayer>`
     - `pub hinting: CanvasHintingAndHoiScene` (`postscript_hints: Vec<PostScriptHint>`, `truetype_instructions: Vec<TrueTypeVisualInstruction>`, `truetype_instructor_ppem: u16`, `hoi_layer_sets: Vec<HoiCubicLayerSet>`, `hoi_sibling_contours: Option<HoiSiblingContours>`, `show_pixel_preview_ppem: Option<u16>`)
     - `pub reference_frame: Option<CustomReferenceFrame>`
     - `pub writing_direction: WritingDirection`
     - `#[serde(default)] pub curve_harmony: CanvasCurveHarmonyScene` (`genius_nodes: BTreeSet<NodeId>`, `g3_nodes: BTreeSet<NodeId>`, `smart_corners: Vec<SmartCornerSpec>`, `reference_stems_upm: Vec<f64>`, `curved_trajectories: Vec<CurvedTrajectoryLink>`, `active_mirror_axis: ActiveMirrorAxis`, `round_overshoot_upm: Option<f64>`)
2. **Bidirectional Host Roundtrip Methods on `GrafiaCanvasState`**:
   - `pub fn export_document_scene(&self) -> CanvasDocumentScene` extracts the complete glyph document scene from `GrafiaCanvasState` (including `curve_harmony`).
   - `pub fn load_document_scene(&mut self, scene: CanvasDocumentScene)` atomically populates `GrafiaCanvasState`, synchronizes `self.contours`, `self.paths`, `self.smart_corners`, and `self.curve_harmony`, bumps `content_generation`, and validates state invariants (`self.assert_invariants()`).

### 2.6 Browser WASM Target (`wasm32-unknown-unknown`) & `grafia_canvas` Entrypoint (`TC-MATURITY-008`)

1. **`#[cfg(target_arch = "wasm32")]` WebRunner Entrypoint (`examples/vector_canvas_demo.rs`)**:
   - Under `#[cfg(target_arch = "wasm32")]`, `main()` MUST bind `eframe::WebRunner::new().start(...)` inside `wasm_bindgen_futures::spawn_local(...)` targeting the `<canvas id="grafia_canvas">` element via `get_element_by_id("grafia_canvas")`.
2. **Non-WASM Guarding of Disk & Environment Operations (`examples/demo_host_shell/toolbar_specs.rs`)**:
   - `install_crash_logger()` and the native disk font probe in `configure_demo_fonts()` MUST guard `std::fs` and `std::env` calls with `#[cfg(not(target_arch = "wasm32"))]` so browser initialization never accesses unavailable host filesystem paths.
   - `[target.'cfg(target_arch = "wasm32")'.dev-dependencies]` in `Cargo.toml` MUST enable `default_fonts` on `eframe` so embedded proportional and monospace fallback fonts (`epaint_default_fonts`) are available alongside `CUPERTINO_ICONS_FONT_BYTES` in the browser.

### 2.7 Orphan-Test Guard (`Cargo.toml` `[[test]]` Registration) & `examples/` UI Magic-Number Hygiene (`tc_arch_016`)

1. **Orphan-Test Prevention under `autotests = false`**:
   - Whenever `autotests = false` is declared in `Cargo.toml`, every `.rs` file directly under `tests/` MUST be registered in a `[[test]]` block in `Cargo.toml` so no test suite can ever be silently skipped.
2. **Strict Sub-Struct Ceilings, Single Source of Truth & Controller Port Separation**:
   - `DragSessionTransients` MUST have $\le 25$ fields, `DrawingToolState` MUST NOT contain `controller_ports`, and `GrafiaCanvasState` MUST have $\le 89$ fields.
   - `GrafiaCanvasState` MUST NOT duplicate `smart_corners` as a top-level struct field alongside `curve_harmony.smart_corners`; `self.curve_harmony.smart_corners` (exposed read-only via `pub fn smart_corners(&self) -> &[SmartCornerSpec]` and mutated via `set_smart_corner` / `remove_smart_corner` / `bake_smart_corners`) is the single canonical source of truth.
3. **`examples/demo_host_shell/` UI Layout Magic-Number Hygiene**:
   - UI layout files in `examples/demo_host_shell/` (`inspector_ui.rs`, `demo_panels.rs`, `desktop_shell.rs` when present) MUST contain zero raw float literals outside `ALLOWED_RATIOS` (`[0.0, 0.5, 1.0, 2.0, -1.0]`) — all UI dimensions (`64.0`, `2048.0`, `320.0`, `110.0`, `44.0`, etc.) MUST come from named constants in `examples/demo_host_shell/toolbar_specs.rs`.
4. **Ban on `.rs` Source-File String-Grepping inside Functional Integration Tests (`tc_arch_016`)**:
   - Functional integration test suites under `tests/*.rs` (every test file other than `tests/architecture_guard_tests.rs`) MUST test runtime behavior via public Rust APIs and shared `tests/common/mod.rs` builders (`canvas_with_closed_contour`, `run_headless_canvas_frame`, `assert_mutation_undo_redo_roundtrip`), and MUST NOT use `read_to_string` on `.rs` source files. Static source/manifest checks belong exclusively in `tests/architecture_guard_tests.rs`.

### 2.8 Guided Showcase, 5 Domain Chapters & Dutch Coach-Card System (`TC-MATURITY-010`)

1. **Five Cohesive Domain Chapters (`ShowcaseChapter`)**:
   - `examples/demo_host_shell/guided_showcase.rs` MUST organize all 18 `DemoPreset` host-port scenarios and all 19 `CanvasTool` modes into 5 clear Dutch chapters:
     - `ShowcaseChapter::BasisTekenen` (`"1. Basis & Vormen"`)
     - `ShowcaseChapter::CurvenEnAudit` (`"2. Curven & Audit"`)
     - `ShowcaseChapter::KalligrafieEnStems` (`"3. Kalligrafie & Stems"`)
     - `ShowcaseChapter::TekstEnMasters` (`"4. Tekst & Masters"`)
     - `ShowcaseChapter::HostStekkersEnHardware` (`"5. Stekkers & Hardware"`)
2. **Self-Documenting Dutch Coach Cards (`ShowcaseCoachCard`) & Step-by-Step Tour**:
   - Every `DemoPreset` variant in `DemoPreset::ALL` MUST map to a `ShowcaseCoachCard` providing non-empty Dutch explanations for:
     - `title`: concise Dutch feature title
     - `what_is_it`: what is visible on the canvas (`"Wat zie je hier?"`)
     - `why_host_plug`: why it is a hexagonal host-port ("stekker") and which API connects it (`"Waarom een stekker?"`)
     - `try_it_yourself`: concrete interactive action for the user (`"Probeer het zelf:"`)
   - Every `CanvasTool` variant MUST map to a Dutch name and actionable usage hint via `tool_dutch_guide(tool: CanvasTool)`.
   - `MockFontHost` MUST support stepping backward (`step_tour_prev`) and forward (`step_tour_next`) through all 18 `DemoPreset` scenarios and rendering a collapsible Coach Banner above the canvas.

### 2.9 Production-Ready Robustness, Zero-Dimension Safety, ID Collision Prevention & Pan-Drag Battery Hardening (`TC-MATURITY-011`)

1. **Zero-Dimension Pixmap Clamping & Zero `unreachable!()` in `src/`**:
   - `alloc_pixmap_or_1x1(width, height)` in `src/adapters/headless.rs` MUST clamp `width.max(1)` and `height.max(1)` without panicking when `width == 0` or `height == 0` (e.g. collapsed split-pane or minimized window), and production modules under `src/` MUST contain zero `unreachable!()` invocations.
2. **Unique NodeId / ContourId Monotonicity Across Mark Clouds & Stroke Expansion**:
   - `compute_mark_cloud_contours` (`src/core/anchors_components.rs`) MUST maintain a single monotonic `nid` counter across all matched `GhostMarkPath` entries so multiple marks on the same anchor never emit duplicate `NodeId`s.
   - `expand_stroked_contours_preview` (`src/core/stroke_expand.rs`) MUST seed `next_cid` and `next_nid` strictly above the maximum existing `ContourId` and `NodeId` in `contours`.
   - `expand_selected_strokes` (`src/adapters/anchor_and_stroke_ops.rs`) MUST re-key remaining unexpanded entries in `self.contour_strokes` when earlier contours expand and shift `ContourId` indices.
3. **Interactive Panning & Drawing-Tool Drag Pixmap Downscaling + Content-Fingerprinted Color-Sublayer Cache**:
   - `GrafiaCanvasState::is_interactive_drag_active(&self)` MUST return `true` during `SelectToolState::PanningViewport { .. }` and active drawing-tool drags (`knife.active`, `primitive_drag.active`, `pencil.active`, `pixel.active`, `brush_stroke_active`, `text_measurement_dragging`) so `effective_color_layer_pixmap_dimensions` caps CPU scanline rasterization to `MAX_INTERACTIVE_DRAG_PIXMAP_DIM` during live viewport panning.
   - `get_or_upload_color_sublayers_texture` (`src/render/canvas_render_cache.rs`) MUST key its texture cache on a full content fingerprint of `state.color_sublayers` (`id`, `visible`, `opacity`, paint parameters/stops, and contour geometry) rather than `color_sublayers.len()` alone.
4. **Defensive External Scene Sanitization in `load_document_scene`**:
   - `GrafiaCanvasState::load_document_scene` (`src/adapters/document_scene_ops.rs`) MUST sanitize untrusted/deserialized `CanvasDocumentScene` fields before `self.assert_invariants()`: deduplicating `GuideId`s and filtering non-finite guide/hint/reference-frame coordinates, clamping `color_sublayers[i].opacity` and gradient stop `offset`s to `[0.0, 1.0]`, clamping `ppem` to `>= 1`, and deduplicating/filtering `curve_harmony.smart_corners`.

---

## 3. Verification & Test Case Traceability Matrix

| Test Case ID | Requirement | Verification Target | Expected Outcome |
| :--- | :--- | :--- | :--- |
| **`tc_arch_016`** | `§2.1`, `§2.2` & `§2.7` | `tests/architecture_guard_tests.rs` | Verifies `too_many_arguments = "deny"`, zero $\ge 3$-element tuple parameters in `src/`, struct field ceilings (`DragSessionTransients <= 25`, `GrafiaCanvasState <= 90`, `DrawingToolState` without `controller_ports`), zero orphan test files under `tests/`, and zero raw float literals in `examples/demo_host_shell/` UI layout modules. |
| **`TC-MATURITY-001`** | `§2.2` | `tests/v091_senior_maturity_tests.rs` | Verifies `PenInteractionTransients`, `DragSessionTransients`, `TextInteractionTransients`, and `GlyphSnapshot` sub-snapshots preserve bit-exact `Undo -> Redo` roundtrips across complex multi-subsystem edits. |
| **`TC-MATURITY-002`** | `§2.1` | `tests/v091_senior_maturity_tests.rs` | Verifies named parameter context structs (`ReferenceFramePalette`, `RulerTickRangeUpm`, `SegmentBellyDragTarget`) behave deterministically. |
| **`TC-MATURITY-003`** | `§2.3` | `tests/v091_senior_maturity_tests.rs` | Verifies that during an active node or gradient drag on a 4K (`3840 by 2160`) viewport, `get_or_upload_color_sublayers_texture` caps the texture dimensions to `[512, 288]` (`MAX_INTERACTIVE_DRAG_PIXMAP_DIM`). |
| **`TC-MATURITY-004`** | `§2.3` | `tests/v091_senior_maturity_tests.rs` | Verifies that upon pointer release (`SelectToolState::Idle`), `get_or_upload_color_sublayers_texture` immediately restores full native `[3840, 2160]` texture resolution and caches it across subsequent idle frames with `0` extra GPU uploads. |
| **`TC-MATURITY-005`** | `§2.4` | `tests/v091_senior_maturity_tests.rs` | Verifies `CanvasDocumentScene` builder, `load_document_scene`, and `export_document_scene` roundtrip all glyph contours, metrics, guides, anchors, components, color sublayers, hinting instructions, and reference frames bit-identically. |
| **`TC-MATURITY-006`** | `§2.5` | `tests/v091_senior_maturity_tests.rs` | Verifies that ruler, text-stream, and kerning visual tokens live in `GrafiaTheme` (`RulerThemeTokens`, `TextStreamThemeTokens`) with backward-compatible defaults and can be customized at runtime. |
| **`TC-MATURITY-007`** | `§2.5` | `tests/v091_senior_maturity_tests.rs` | Verifies that Pen/Select hit-test radii, segment hit-test tolerances, kerning/spacing keyboard increments, and PostScript ghost-hint widths respect runtime `state.preferences` and custom `width_upm` values instead of hardcoded constants. |
| **`TC-MATURITY-008`** | `§2.6` | `tests/v091_senior_maturity_tests.rs` | Verifies `examples/vector_canvas_demo.rs` binds `get_element_by_id("grafia_canvas")`, `examples/demo_host_shell/toolbar_specs.rs` guards `std::fs` under `#[cfg(not(target_arch = "wasm32"))]`, and `Cargo.toml` enables `default_fonts` for `wasm32` `eframe`. |
| **`TC-MATURITY-009`** | `§2.4` | `tests/v091_senior_maturity_tests.rs` | Verifies `CanvasDocumentScene` export/load roundtrip with populated `v0.10.0`–`v0.12.0` `CanvasCurveHarmonyScene` (`genius_nodes`, `g3_nodes`, `smart_corners`, `reference_stems_upm`, `curved_trajectories`, `active_mirror_axis`, `round_overshoot_upm`). |
| **`TC-MATURITY-010`** | `§2.8` | `tests/v091_senior_maturity_tests.rs` & `examples/vector_canvas_demo.rs` | Verifies `guided_showcase.rs` defines all 5 `ShowcaseChapter` domains, complete Dutch `ShowcaseCoachCard` explanations (`what_is_it`, `why_host_plug`, `try_it_yourself`) for all 18 `DemoPreset`s, Dutch tool guides for all 19 `CanvasTool`s, and interactive `◀ Vorige` / `Volgende ▶` tour navigation. |
| **`TC-MATURITY-011`** | `§2.9` | `tests/v091_senior_maturity_tests.rs` | Verifies zero-dimension `rasterize_canvas_subregion(..., 0, 0)` safety, unique `NodeId`s across multi-mark `compute_mark_cloud_contours` and high-ID `expand_stroked_contours_preview`, `contour_strokes` re-keying on partial `expand_selected_strokes`, 4K pixmap downscaling during `SelectToolState::PanningViewport`, and defensive sanitization of out-of-range external `CanvasDocumentScene` inputs in `load_document_scene`. |

