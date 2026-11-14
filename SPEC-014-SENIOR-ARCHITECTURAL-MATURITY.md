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
   - No domain, token, or undo-snapshot `struct` in `src/` (including `GlyphSnapshot` and `GrafiaTheme`) may exceed `35` fields (`<= 35`), and `GrafiaCanvasState` MUST NOT exceed `85` top-level fields.
2. **Cohesive Transient Sub-Structs on `GrafiaCanvasState`**:
   - Internal pointer, pen, marquee, and text-drag transient fields on `GrafiaCanvasState` MUST be grouped into cohesive sub-structs:
     - `PenInteractionTransients` (`drag_last_pt`, `handle_delta`, `alt_frozen_bcp_in`, `seg_has_explicit_h1`, `resuming_corner_line_end`, `join_b_had_out_handles`, `join_b_was_smooth`).
     - `DragSessionTransients` (`drag_start_font_positions`, `drag_start_node_types`, `drag_start_contours`, `last_click_time_pos`, `last_double_click_time_pos`, `marquee_pre_selection`, `marquee_modifiers`, `alt_marquee_pending_start`, `active_drag_before_snapshot`, `pending_segment_click`, `alt_duplicate_pending`, `temp_tool_before_override`, `last_modifiers`, `temp_zoom_via_key_space`, `ruler_drag_is_horizontal`, `pre_all_layers_select_snapshot`, `saved_viewport`, `prev_view_mode`).
     - `TextInteractionTransients` (`temp_show_text_measurement_line`, `text_mode_numbers_threshold`, `last_select_tool`, `text_drag_anchor_caret`, `suppress_text_stream_sync`, `text_tool_activated_in_frame`).
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

### 2.4 Host Application Document Scene API (`CanvasDocumentScene` — `TC-MATURITY-005`)

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
2. **Bidirectional Host Roundtrip Methods on `GrafiaCanvasState`**:
   - `pub fn export_document_scene(&self) -> CanvasDocumentScene` extracts the complete glyph document scene from `GrafiaCanvasState`.
   - `pub fn load_document_scene(&mut self, scene: CanvasDocumentScene)` atomically populates `GrafiaCanvasState`, synchronizes `self.contours` and `self.paths`, bumps `content_generation`, and validates state invariants (`self.assert_invariants()`).

---

## 3. Verification & Test Case Traceability Matrix

| Test Case ID | Requirement | Verification Target | Expected Outcome |
| :--- | :--- | :--- | :--- |
| **`tc_arch_016`** | `§2.1` & `§2.2` | `tests/architecture_guard_tests.rs` | Verifies `too_many_arguments = "deny"`, zero $\ge 3$-element tuple parameters in `src/`, and struct field ceilings across `src/`. |
| **`TC-MATURITY-001`** | `§2.2` | `tests/v091_senior_maturity_tests.rs` | Verifies `PenInteractionTransients`, `DragSessionTransients`, `TextInteractionTransients`, and `GlyphSnapshot` sub-snapshots preserve bit-exact `Undo -> Redo` roundtrips across complex multi-subsystem edits. |
| **`TC-MATURITY-002`** | `§2.1` | `tests/v091_senior_maturity_tests.rs` | Verifies named parameter context structs (`ReferenceFramePalette`, `RulerTickRangeUpm`, `SegmentBellyDragTarget`) behave deterministically. |
| **`TC-MATURITY-003`** | `§2.3` | `tests/v091_senior_maturity_tests.rs` | Verifies that during an active node or gradient drag on a 4K (`3840 by 2160`) viewport, `get_or_upload_color_sublayers_texture` caps the texture dimensions to `[512, 288]` (`MAX_INTERACTIVE_DRAG_PIXMAP_DIM`). |
| **`TC-MATURITY-004`** | `§2.3` | `tests/v091_senior_maturity_tests.rs` | Verifies that upon pointer release (`SelectToolState::Idle`), `get_or_upload_color_sublayers_texture` immediately restores full native `[3840, 2160]` texture resolution and caches it across subsequent idle frames with `0` extra GPU uploads. |
| **`TC-MATURITY-005`** | `§2.4` | `tests/v091_senior_maturity_tests.rs` | Verifies `CanvasDocumentScene` builder, `load_document_scene`, and `export_document_scene` roundtrip all glyph contours, metrics, guides, anchors, components, color sublayers, hinting instructions, and reference frames bit-identically. |
| **`TC-MATURITY-006`** | `§2.5` | `tests/v091_senior_maturity_tests.rs` | Verifies that ruler, text-stream, and kerning visual tokens live in `GrafiaTheme` (`RulerThemeTokens`, `TextStreamThemeTokens`) with backward-compatible defaults and can be customized at runtime. |
| **`TC-MATURITY-007`** | `§2.5` | `tests/v091_senior_maturity_tests.rs` | Verifies that Pen/Select hit-test radii, segment hit-test tolerances, kerning/spacing keyboard increments, and PostScript ghost-hint widths respect runtime `state.preferences` and custom `width_upm` values instead of hardcoded constants. |

