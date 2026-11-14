# Hexagonal Host-Port ("Plug-and-Play") Architecture (`grafia::adapters`)

`grafia::adapters` bridges the pure mathematical core (`grafia::core`, `grafia::snapping`, `grafia::interaction`, `grafia::render`) to [`egui`] for live interactive desktop/web apps and to [`tiny_skia`] for deterministic headless verification.

---

## 1. The Hexagonal Data-In / Event-Out Contract

`grafia` is architected around the **Hexagonal Ports & Adapters** pattern. The canvas widget never reads or writes font/vector files (`UFO`, `OTF`, `TTF`, `SVG`, `DXF`) directly and never mutates global application state behind your back. Instead:

1. **Data-In Ports (Host $\rightarrow$ `GrafiaCanvasState`):**
   - **Single-Glyph / Vector Mode:** Call [`GrafiaCanvasState::load_glyph`] with a [`crate::core::Glyph`] and [`crate::snapping::GlyphMetrics`], or load a complete multi-layer scene atomically via [`GrafiaCanvasState::load_document_scene`] (`CanvasDocumentScene`).
   - **Multi-Glyph Text Stream Mode:** Populate `state.text_stream` (`CanvasTextStream`) with `GlyphSlot` items and choose `WritingDirection::LeftToRight`, `WritingDirection::RightToLeft`, or `WritingDirection::TopToBottom`.
   - **Reference Frames, Color Layers & Preferences:** Configure `state.custom_reference_frame` (`CustomReferenceFrame` for icon keylines or CJK ideographic em-boxes), `state.color_sublayers` (`CanvasColorSublayer`), `state.apply_preferences(GrafiaPreferences)`, and `state.set_keymap(GrafiaKeymap)`.

2. **Interactive Frame Execution (`GrafiaCanvasWidget`):**
   - Inside any `egui::Ui`, call `GrafiaCanvasWidget::new(&mut state, &theme).show(ui)` (or `GrafiaCanvasWidget::from_state(&mut state).show(ui)`).
   - `InputDispatcher` routes pointer, wheel, multi-touch pinch/pan, and keyboard chord events through the active tool state machine, records mutations in `CommandHistory`, and returns a [`GrafiaCanvasResponse`] indicating whether geometry or selection changed on that frame.

3. **Event-Out Ports (`GrafiaCanvasState` $\rightarrow$ Host):**
   - **`state.drain_host_requests()` $\rightarrow$ `Vec<CanvasHostRequest>`:** High-level semantic actions requested by the user inside the canvas, such as:
     - `OpenComponentSource { id, component_name }` (user double-clicked a component instance to jump to its source glyph),
     - `ResetDefaultAnchors` (user requested default anchor dictionary population),
     - `UpdateSmartComponentAxis { component_id, axis_name, new_value }` (user dragged a parametric smart-component slider ray on canvas),
     - `MutateColorSublayer`, `MutatePostScriptHint`, `MutateTrueTypeInstruction`, `MutateHoiLayerNode`, and `SelectOrMutateExtraNode`.
   - **`state.drain_spacing_deltas()` & `state.drain_kerning_deltas()`:** Live horizontal (`LSB`/`RSB`) or vertical (`TSB`/`BSB`) spacing and pair-kerning deltas emitted while editing in `CanvasTool::Text`.
   - **`state.drain_text_stream_edit_requests()`:** Character insertion, deletion, or slot navigation requests in text-stream mode.
   - **`state.export_glyph()` & `state.export_document_scene()`:** Snapshots the edited foreground/background contours, anchors, hints, and attachments back into a serializable value struct whenever `response.changed` is `true`.

---

## 2. Building Custom Inspector Panels with `SelectionInfo` & `SelectionInfoAction`

Rather than forcing a hardcoded sidebar UI onto your application, `grafia` exposes a pure 2D projection of the current selection via `state.compute_selection_info()`:

- **`SelectionInfo::EmptyGlyph`:** Glyph name, advance width, LSB, RSB, TSB, BSB, and total contour/node counts.
- **`SelectionInfo::SingleOnCurve` / `SelectionInfo::SingleOffCurve` / `SelectionInfo::HobbyNode`:** Exact node coordinates, segment angles, handle lengths/ratios, and Hobby tension $(\tau_{\text{in}}, \tau_{\text{out}})$.
- **`SelectionInfo::MultiNode`:** Selection bounding box (`SelectionBounds`), width/height dimensions, and node distribution.
- **`SelectionInfo::Anchor` / `SelectionInfo::Component` / `SelectionInfo::Guide` / `SelectionInfo::BackgroundImage` / `SelectionInfo::GradientStop` / `SelectionInfo::PostScriptHint` / `SelectionInfo::TrueTypeInstruction` / `SelectionInfo::HoiTiming`:** Domain-specific inspector projections for every specialized canvas entity.

When a user edits a numeric field in your host application's inspector panel, simply pass a [`crate::core::SelectionInfoAction`] to `state.apply_selection_info_action(action)` — `grafia` applies the mutation, updates dependent splines/handles, pushes an undo snapshot, and bumps `content_generation()`.

---

## 3. 1:1 Headless Offscreen Verification (`headless`)

GUI canvases are notoriously difficult to test in CI without a GPU display server. `grafia::adapters::headless` (`rasterize_canvas_subregion` and `pixmap_to_rgba_image`) renders the exact same 5-pass scene geometry into an in-memory `tiny_skia::Pixmap` RGBA buffer, enabling deterministic pixel-diff and Structural Similarity (`SSIM`) regression tests on headless Linux/Windows/macOS runners.
