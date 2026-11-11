---
type: specification
id: SPEC-005
title: "grafia v0.4.0 — Specialized Edit Tools, Measurement Coordinates & Canvas Annotations"
version: 0.4.0
status: normative
date: 2026-11-10
baseline: "Normative Mathematical & Ergonomic Baseline (OpenType 1.9 / CAGD / ISO 9241-9)"
parent: SPEC-001
---

# SPEC-005: `grafia v0.4.0` — Specialized Edit Tools (`Knife`, `Primitives`, `Measurement`, `Annotation`, `Pencil`, `Pixel`)

> **Normative Verification Mandate (`ISO/IEC/IEEE 29119-3:2021` & `IEEE 1012-2016`):** Every constant, modifier bitmask, coordinate badge color, geometric algorithm, and state transition in this specification is anchored in Computer-Aided Geometric Design (CAGD), OpenType 1.9 outline topology, and ISO 9241-9 ergonomic interaction standards. No production code for `v0.4.0` is written before this specification and the failing RED COTS test suite (`tests/v040_complete_tests.rs`) are anchored.

---

## 1. Normative Mathematical & Ergonomic Baseline (OpenType 1.9 / CAGD / ISO 9241-9)

All geometric invariants, keyboard shortcuts, and state-machine rules for `v0.4.0` are specified below:

| Subsystem | Normative Domain & Shortcut | Mathematical & Ergonomic Specification |
| :--- | :--- | :--- |
| **`KnifeTool` (`E` / `Shift+E`)** | Vector Path Slicing & Subdivision (`E`, temporary `Cmd` $\to$ `Select`) | Shortcut `"e"`, temporary `Cmd` override delegates to `Select`. Drag sets `start_upm` and `current_upm`. `Shift` constrains `current_upm` horizontally ($\|\Delta x\| > \|\Delta y\|$) or vertically ($\|\Delta x\| \le \|\Delta y\|$). Background pass strokes the knife line in neutral gray (`1.0 / scale`) and fills circles at interior contour intersections (`1 .. count-2`). On `PointerUp`, if $\text{hypot}(\text{current\_upm} - \text{start\_upm}) > 0.5\text{ UPM}$: without `Alt`/`Option`, computes all intersections across all shapes in the layer sorted along the knife ray (`count >= 4` pairs consecutive interior ray intervals $i = 1, 3, \dots$, inserts `OnCurve` nodes on the hit paths, and bridges `(new_node_1, node_2)` and `(node_1, new_node_2)` — merging distinct outer+inner counter rings on the first cut pair and splitting same-ring pairs into separate closed contours; a single intersection opens a closed contour at the cut point); with `Alt`/`Option`, inserts `OnCurve` nodes at all intersections without slicing contours. |
| **Boolean Path Operator (`Cmd+Shift+O` — `Remove Overlap` / `Subtract` / `Intersect`)** | Non-Zero Winding Planar Graph Arrangement (`Cmd+Shift+O`) | Separates open paths (`closed == false`, leaving them untouched) from closed paths (`closed == true`). Extracts cubic and linear parametric segments, computes all pairwise segment intersections and subdivides segments via de Casteljau at sorted parameters. Removes opposite coincident segments, filters interior segments via midpoint Non-Zero Winding evaluation, deduplicates same-direction coincident segments, traces closed boundary cycles with minimal left-turning exterior angles, and merges collinear straight corner segments. Also supports boolean `Subtract` and `Intersect`. |
| **`PrimitivesTool` (`F` / `Shift+F`)** | Parametric Geometric Primitives (`Rectangle` & `Circle`) | Shortcut `"f"` (`Shift+F` cycles `Rectangle` and `Circle`). Minimum drag size `min_size = 5.0` UPM. `PointerDown` clears selection and snaps `start_upm` via `SnapEngine` (unless `Ctrl`). `PointerMove` snaps `current_upm`; `Shift` constrains $\|\Delta x\| = \|\Delta y\| = \max(\|\Delta x\|, \|\Delta y\|)$; `Alt`/`Option` expands symmetrically around `start_upm` as center ($\text{origin} = \text{start} - \|\text{drag} - \text{start}\|$, $\text{size} = 2\|\text{drag} - \text{start}\|$); holding `Space` translates `start_upm` alongside cursor delta; active grid spacing rounds coordinates. Foreground pass strokes preview rectangle/oval (`1.0 dp`) and renders `"W × H"` monospaced dimension badge. `PointerUp` creates a 4-`Line` rectangle contour or a 4-`CurveSmooth` extreme node + 8-`OffCurve` handle circle contour with $\kappa = \frac{4}{3}(\sqrt{2}-1) \approx 0.55228475$. Single click without drag requests `PrimitiveDialog` (`default 100 × 100` UPM). |
| **`MeasurementTool` (`L` / `Cmd+Ctrl+Opt`)** | Ray-Contour Metrology & Coordinate Overlay (`L`, temporary `Cmd+Ctrl+Opt`) | Shortcut `"l"`, temporary `Cmd+Ctrl+Opt` activation from any tool. Snaps `start_upm` and `current_upm` to hit `Node`, `Handle`, or `Anchor` positions. `Shift` constrains horizontal/vertical. Pressing `'G'` while dragging promotes the measurement line (`start_upm`..`current_upm`) into a local `Guide` at `start_upm` with $\text{angle} = \text{atan2}(\Delta y, \Delta x)$, `show_measurement = true`, and `show_guides = true`. Ray intersection solver intersects the ray with all contours plus Left Sidebearing ($x=0$, italic-sheared) and Right Sidebearing ($x=\text{advance\_width}$, italic-sheared), sorted along the ray. Coordinate overlay draws `OnCurve` coordinates in **Blue**, `OffCurve` coordinates in **Green**, and italic-compensated segment $(\Delta X_{\text{italic}}, \Delta Y)$ midpoints in **Red**, plus anchor coordinates and guide angles. Measurement line pass draws monospaced distance badges at the midpoint of every consecutive pair in `intersections` and an angle badge at `current_upm` where vertical upward = $0^\circ$ CCW. |
| **`AnnotationTool` (`A`)** | Canvas Review Annotations (`Text`, `Arrow`, `Circle`, `Plus`, `Minus`) | Shortcut `"a"`, 5 sub-modes: `1 = Text`, `2 = Arrow`, `3 = Circle` (default `radius = 100.0` UPM), `4 = Plus`, `5 = Minus`. Clicking an existing annotation or its control handle selects and grabs it; clicking empty canvas creates a new `CanvasAnnotation` of the active sub-mode at the clicked UPM location, selects it, and grabs it. Dragging an annotation moves `position` (`Shift` constrains axis); dragging its handle adjusts `Text.width` ($\Delta x$), `Arrow.angle` ($\text{atan2}(\Delta y, \Delta x)$), or `Circle.radius` ($-\Delta y$, clamped to $\ge 10.0$ UPM). `Backspace`/`Delete` removes selected annotations; `Cmd+A` in `AnnotationTool` selects all annotations. |
| **`PencilTool` (`B`)** | Freehand Sketching & Schneider Curve Fitting (`B`) | Shortcut `"b"`, temporary `Cmd` override delegates to `Select`. Samples freehand cursor points along the drag trail ($\text{hypot} \ge 2.0 / \text{scale}$), renders live preview polyline + sample dots in the foreground pass, and on `PointerUp` fits a simplified cubic Bézier `Contour` via Ramer-Douglas-Peucker decimation and Catmull-Rom / Schneider tangent estimation (closing automatically when the end point finishes within `close_radius` of the start point). |
| **`PixelTool` (`X`)** | Discrete Cellular Grid Painting & Bresenham Line Rasterization (`X`) | Shortcut `"x"`. Snaps cursor to the pixel grid (`pixel_step = grid_spacing.max(1.0)`, cell $\text{origin} = (\lfloor x / \text{step} \rfloor \cdot \text{step}, \lfloor y / \text{step} \rfloor \cdot \text{step})$). `PointerDown` checks whether a cell contour already exists at `origin`: if present, enters `erase = true` mode for the drag; otherwise enters `erase = false` (paint) mode and inserts a $\text{step} \times \text{step}$ counter-clockwise rectangle contour. Holding `Shift` on `PointerDown` draws a 2D Bresenham raster line of pixel cells from `last_pixel_cell` to the current cell. |

---

## 2. Normative Requirements & Subsystems (`grafia v0.4.0`)

### 2.1 `KnifeTool` (`ActiveTool::Knife`, Shortcut `E` / `Shift+E`)

1. **Activation & Drag State:**
   - Pressing `E` or `Shift+E` selects `ActiveTool::Knife`.
   - `PointerDown` (primary button) initializes `KnifeState { active: true, start_upm, current_upm, intersections: Vec<Point>, alt_add_nodes: modifiers.alt }`.
   - `PointerMove` updates `current_upm`:
     - When `modifiers.shift` is active: if `|raw.x - start.x| > |raw.y - start.y|`, `current_upm = Point::new(raw.x, start.y)` (horizontal); otherwise `current_upm = Point::new(start.x, raw.y)` (vertical).
     - Computes all geometric intersections between segment `(start_upm, current_upm)` and all contours in `state.glyph.contours`, sorted by parametric distance $t \in [0, 1]$ along the knife segment, and stores them in `KnifeState.intersections`.
2. **Background Rendering (`draw_knife_overlay` / `draw_knife_overlay_skia`):**
   - While `KnifeState.active` is `true`, draws a `1.0 dp` line from `start_upm` to `current_upm` in `theme.knife_line_color` and filled circles (`KNIFE_INTERSECTION_RADIUS_DP = 3.5`) at every contour intersection point.
3. **Commit on `PointerUp` (`cut_paths_in_layer` / `add_nodes_in_layer`):**
   - If `start_upm.distance(current_upm) <= KNIFE_MIN_DRAG_UPM` (`0.5` UPM), cancels drag without mutating the glyph.
   - **Alt/Option Mode (`modifiers.alt == true`):**
     - Records Undo snapshot, inserts `OnCurve` (`CurveSmooth` on cubic segments, `Line` on line segments via Exact de Casteljau / linear subdivision) at every intersection point across all hit contours without splitting or opening any contour, selects the newly inserted nodes, and clears `KnifeState`.
   - **Standard Cut Mode (`modifiers.alt == false`):**
     - Records Undo snapshot.
     - For each contour hit by the segment `(start_upm, current_upm)`:
       - **Case A ($\ge 2$ intersections across closed contours — layer-wide pairwise slice):**
         - Collects all intersections across all closed contours in the layer, sorted by parametric distance $u \in [0, 1]$ along the knife ray `(start_upm, current_upm)`, and inserts cut `OnCurve` nodes on the hit segments.
         - Pairs consecutive interior entry/exit intervals `(hit[0], hit[1])`, `(hit[2], hit[3])`, $\dots$ along the sorted knife ray:
           - **Same-Ring Pair (`ring_a == ring_b`):** When both cut nodes of a pair lie on the same closed contour ring (e.g., slicing across both stems of a single U-shaped or `/n` contour, or the second cut pair on a merged outer+inner ring), splits that closed ring at the two cut nodes into two valid closed contours joined by straight `Line` cut edges without cross-stem self-intersections.
           - **Outer + Inner Counter Ring Pair (`ring_a != ring_b`):** When the two cut nodes of a pair lie on distinct closed rings (e.g., the first cut pair `(hit[0], hit[1])` when slicing horizontally across `/o` or `/A`, hitting the outer CCW contour at `hit[0]` and the inner CW counter at `hit[1]`), divides both closed rings at their respective cut nodes, ensures opposite crossing orientation across the knife line, and bridges them (`new_node_1 -> node_2` and `node_1 -> new_node_2`) into a single merged closed contour so that the subsequent pair `(hit[2], hit[3])` cleanly splits the merged ring into non-overlapping top and bottom closed arch contours.
       - **Case B ($1$ intersection on a closed contour):**
         - Subdivides the contour at the single intersection parameter $t_1$, rotates the contour's node list so the cut node is at the boundary, and opens the contour (`closed = false`) with duplicated start/end `Line` nodes at the cut point.
       - **Case C ($\ge 1$ intersection on an open contour):**
         - Splits the open contour at each intersection parameter into independent open sub-contours.

---

### 2.2 `PrimitivesTool` (`ActiveTool::Primitives`, Shortcut `F` / `Shift+F`)

1. **Sub-Modes & Cycling:**
   - `PrimitiveSubTool::{Rectangle, Circle}` stored on `EditorState.primitive_sub_tool` (default `Rectangle`).
   - Pressing `F` activates `ActiveTool::Primitives` (or cycles `Rectangle <-> Circle` when `Shift+F` is pressed or when `F` is pressed while already in `Primitives`).
2. **Interactive Drag & Modifiers:**
   - `PointerDown` clears selection, snaps `start_upm` via `SnapEngine` (unless `modifiers.ctrl`), and sets `PrimitiveDragState { active: true, start_upm, current_upm: start_upm, last_raw_upm: raw_upm, sub_tool }`.
   - `PointerMove`:
     - If `state.space_pressed`, translates `start_upm` by `raw_upm - last_raw_upm` so the entire primitive pans with the cursor.
     - Snaps `current_upm` via `SnapEngine` (unless `modifiers.ctrl`) and rounds to active grid step (`round_to_grid`).
     - Bounding rectangle computation (`compute_primitive_rect(start, current, shift, alt)`):
       - Let $\Delta x = \text{current}.x - \text{start}.x$, $\Delta y = \text{current}.y - \text{start}.y$.
       - If `shift`: let $s = \max(|\Delta x|, |\Delta y|)$; set $\Delta x = \text{sign}(\Delta x) \cdot s$, $\Delta y = \text{sign}(\Delta y) \cdot s$.
       - If `alt` (center origin): `min_x = start.x - |dx|`, `max_x = start.x + |dx|`, `min_y = start.y - |dy|`, `max_y = start.y + |dy|`.
       - Else (corner origin): `min_x = min(start.x, start.x + dx)`, `max_x = max(start.x, start.x + dx)`, `min_y = min(start.y, start.y + dy)`, `max_y = max(start.y, start.y + dy)`.
3. **Foreground Preview (`draw_primitive_preview`):**
   - While dragging, strokes the preview rectangle or cubic-kappa oval in `theme.primitive_preview_stroke` (`1.0 dp`) and draws a monospaced `"W × H"` dimension pill badge at the bottom-right corner.
4. **Commit on `PointerUp` & Single-Click Dialog:**
   - If `width >= PRIMITIVE_MIN_SIZE_UPM` (`5.0`) and `height >= PRIMITIVE_MIN_SIZE_UPM` (`5.0`):
     - Records Undo snapshot, appends the new contour (`make_rect_contour` with 4 `Line` nodes or `make_oval_contour` with 4 `CurveSmooth` extreme nodes + 8 `OffCurve` handles using $\kappa = 0.55228475$), and selects all nodes of the new contour.
   - If not dragged (`width < 5.0 && height < 5.0`):
     - Emits `ToolDialogRequest::Primitive { position_upm: start_upm, sub_tool, default_width: 100.0, default_height: 100.0 }`.
     - Host or programmatic caller can invoke `state.insert_primitive(sub_tool, origin_upm, width_upm, height_upm)` to commit with Undo.

---

### 2.3 `MeasurementTool` (`ActiveTool::Measurement`, Shortcut `L` & Temporary `Cmd+Ctrl+Opt`)

1. **Activation & Temporary Modifier Override:**
   - Pressing `L` selects `ActiveTool::Measurement`.
   - Holding `Cmd + Ctrl + Alt` (`modifiers.ctrl && modifiers.alt && modifiers.shift_or_cmd` / dedicated `state.set_temp_measurement_modifier(bool)` or `Modifiers { ctrl: true, alt: true, .. }` with `cmd`) temporarily switches any active tool into `ActiveTool::Measurement` and restores `previous_tool` on release.
2. **Coordinate Overlay Mode:**
   - Whenever `ActiveTool::Measurement` is active (even before dragging!), `grafia` renders the full coordinate & delta overlay across the active glyph:
     - **Blue Labels (`theme.measurement_oncurve_label_color`):** `"x, y"` integer/rounded coordinate label next to every `OnCurve` node (`Line`, `Curve`, `CurveSmooth`).
     - **Green Labels (`theme.measurement_offcurve_label_color`):** `"x, y"` coordinate label next to every `OffCurve` BCP handle.
     - **Red Segment Delta Labels (`theme.measurement_segment_delta_color`):** `"Δx, Δy"` label at the geometric midpoint $t = 0.5$ of every contour segment between consecutive `OnCurve` nodes, where $\Delta x = \text{round}((x_2 - x_1) - (y_2 - y_1) \cdot \tan(\text{italic\_angle}))$ and $\Delta y = \text{round}(y_2 - y_1)$.
     - **Anchor Labels:** `"x, y"` coordinate label next to every visible anchor when `state.show_anchors` is `true`.
     - **Guide Angle Labels:** `"θ°"` angle label next to every visible guide origin when `state.show_guides` is `true`.
3. **Measurement Ray, Sidebearing Intersections & Badges:**
   - `PointerDown` snaps `start_upm` to any hit `Node` or `Anchor` within hit radius; `PointerMove` snaps `current_upm` to any hit `Node` or `Anchor`, or constrains horizontally/vertically when `Shift` is held.
   - `compute_measurement_intersections(glyph, metrics, start_upm, end_upm)`:
     - Intersects the infinite line / clipped ray from `start_upm` to `end_upm` with:
       1. Every contour segment in `glyph.contours`.
       2. The Left Sidebearing line ($x = 0$, sheared by `metrics.italic_angle_deg` around `y = metrics.x_height * 0.5`).
       3. The Right Sidebearing line ($x = \text{glyph.advance\_width}$, sheared by `metrics.italic_angle_deg` around `y = metrics.x_height * 0.5`).
     - Includes `start_upm` and `end_upm`, sorts all points along the ray direction, and deduplicates points closer than `0.05` UPM.
   - **Visual Overlay while Dragging:**
     - Strokes the measurement line in `theme.measurement_line_color` and fills intersection dots (`3.5 dp`) at every point in `intersections`.
     - At the midpoint of every consecutive pair `(p[i], p[i+1])` in `intersections`, renders a monospaced pill badge showing the Euclidean segment distance `p[i].distance(p[i+1])` rounded to `0.1` UPM (integer if `.0`).
     - At `end_upm`, renders a monospaced angle badge showing the typographic angle (`vertical upward = 0°`, positive counter-clockwise, normalized to `[-90°, 180°]` via `asin(sin(rad)) / deg2rad`).
4. **Promote Measurement Line to Guide (`'G'` Key):**
   - Pressing `G` while `MeasurementDragState.active` is `true` (and `start_upm.distance(end_upm) > 0.5`):
     - Records Undo snapshot, adds a local `Guide` to `state.local_guides` at `position = start_upm` with `angle_deg = (end_upm.y - start_upm.y).atan2(end_upm.x - start_upm.x).to_degrees()`, sets `guide.show_measurement = true`, enables `state.show_guides = true`, and selects the newly created guide.

---

### 2.4 `AnnotationTool` & `CanvasAnnotation` (`ActiveTool::Annotation`, Shortcut `A`)

1. **Data Model (`CanvasAnnotation` & `AnnotationKind`):**
   - Stored in `Glyph.annotations: Vec<CanvasAnnotation>` (included in `GlyphSnapshot` for bit-exact Undo/Redo roundtrips) and toggled by `EditorState.show_annotations: bool` (always rendered when `ActiveTool::Annotation` is active, or when `state.show_annotations == true`).
   - Five normative kinds (`1..5`):
     - `AnnotationKind::Text` (`1`): `text: String` (renders `"empty note"` in secondary label color when `text.is_empty()`), `width: f32` (default `160.0` UPM, min `40.0` UPM).
     - `AnnotationKind::Arrow` (`2`): `angle_deg: f32` (default `45.0` deg), `length_upm: f32` (default `80.0` UPM).
     - `AnnotationKind::Circle` (`3`): `radius_upm: f32` (default `100.0` UPM, min `10.0` UPM).
     - `AnnotationKind::Plus` (`4`): screen-space plus symbol (`16.0 dp` arm span).
     - `AnnotationKind::Minus` (`5`): screen-space minus symbol (`16.0 dp` arm span).
2. **Interaction State Machine:**
   - `EditorState.annotation_sub_tool: AnnotationSubTool` (`Text`, `Arrow`, `Circle`, `Plus`, `Minus`; cycled with `Shift+A` or set directly).
   - `PointerDown`:
     - Hit-tests annotation control handles first (`annotation_handle_position_upm`), then annotation bodies (`hit_test_annotation`).
     - If an existing handle or annotation is hit:
       - Unless `modifiers.shift`, clears previous annotation selection (`selected_annotations.clear()`).
       - Selects the hit annotation and begins `AnnotationDragMode::MoveBody` or `AnnotationDragMode::DragHandle { index }`.
     - If empty canvas is clicked:
       - Records Undo snapshot, creates a new `CanvasAnnotation` of `state.annotation_sub_tool` at `click_upm`, appends it to `state.glyph.annotations`, selects it exclusively, and begins `AnnotationDragMode::MoveBody`.
   - `PointerMove`:
     - `MoveBody`: translates all `selected_annotations` by `delta_upm` (`Shift` constrains `delta_upm` horizontally or vertically).
     - `DragHandle { index }`:
       - `Text`: adjusts `width = (initial_width + delta_upm.x).max(ANNOTATION_MIN_TEXT_WIDTH_UPM)`.
       - `Arrow`: adjusts `angle_deg = (current_upm.y - ann.position.y).atan2(current_upm.x - ann.position.x).to_degrees()`.
       - `Circle`: adjusts `radius_upm = (initial_radius - delta_upm.y).max(ANNOTATION_MIN_CIRCLE_RADIUS_UPM)`.
   - `Delete` / `Backspace`: removes all `selected_annotations` with Undo.
   - `Cmd+A`: when `ActiveTool::Annotation` is active, selects all annotations in `state.glyph.annotations`.

---

### 2.5 `PencilTool` (`ActiveTool::Pencil`, Shortcut `B`)

1. **Freehand Sampling:**
   - Pressing `B` activates `ActiveTool::Pencil`.
   - `PointerDown` initializes `PencilDragState { active: true, points_upm: vec![raw_upm] }`.
   - `PointerMove` appends `raw_upm` whenever `raw_upm.distance(*points_upm.last()) >= PENCIL_MIN_SAMPLE_DIST_DP / viewport.zoom`.
2. **Foreground Trail Preview (`draw_pencil_preview`):**
   - Renders the live freehand polyline in `theme.pencil_trail_stroke` (`1.5 dp`) and sample dots (`2.0 dp`) at every recorded sample point.
3. **Curve Fitting on `PointerUp` (`fit_freehand_contour`):**
   - When `points_upm.len() >= 2`:
     - Simplifies the polyline via Ramer-Douglas-Peucker (`PENCIL_RDP_EPSILON_UPM = 2.5`) and fits smooth cubic Bézier segments (`CurveSmooth` + `OffCurve` handles via Catmull-Rom / Schneider tangent estimation).
     - Automatically closes the contour (`closed = true`) if `points_upm.first().distance(*points_upm.last()) <= PENCIL_CLOSE_RADIUS_DP / viewport.zoom`.
     - Records Undo snapshot, appends the fitted `Contour` to `state.glyph.contours`, and selects its nodes.

---

### 2.6 `PixelTool` (`ActiveTool::Pixel`, Shortcut `X`)

1. **Grid-Aligned Cell Painting & Erasing:**
   - Pressing `X` activates `ActiveTool::Pixel`.
   - Effective pixel size is `step = state.grid_spacing_upm().max(PIXEL_DEFAULT_STEP_UPM)` (when grid spacing is `<= 1.0`, uses `state.pixel_size_upm` default `50.0` UPM so pixels are visible at standard UPM scales, configurable via `state.pixel_size_upm`).
   - Cell origin for any UPM point $p$: $\text{cell}(p) = (\lfloor p.x / \text{step} \rfloor \cdot \text{step}, \lfloor p.y / \text{step} \rfloor \cdot \text{step})$.
   - `PointerDown`:
     - Records Undo snapshot.
     - Checks if an axis-aligned `step × step` pixel contour already exists at `cell(raw_upm)`.
     - If `modifiers.shift` is held and `state.last_pixel_cell` is `Some(prev_cell)`:
       - Rasterizes a 2D Bresenham line of cells from `prev_cell` to `cell(raw_upm)` applying the current paint/erase mode.
     - Else if cell exists: sets `erase_mode = true` and removes the cell contour.
     - Else: sets `erase_mode = false` and appends a `step × step` `Line` rectangle contour at `cell(raw_upm)`.
   - `PointerMove` while dragging: applies `add_or_remove_pixel_cell(cell(raw_upm), erase_mode)` for each newly entered grid cell.

---

### 2.7 Boolean `Remove Overlap` (`Cmd+Shift+O`), `Subtract` & `Intersect`

1. **Activation & Scope:**
   - Keyboard shortcut `Cmd+Shift+O` (`Ctrl+Shift+O` on Windows/Linux/Android) or calling `GrafiaCanvasState::remove_overlap(&mut self) -> bool` executes true geometric Boolean Union / Overlap Removal across closed contours in the active glyph layer.
   - Open contours (`closed == false`) are preserved untouched in place.
   - If $\ge 2$ closed contours contain selected nodes, `remove_overlap` operates on the selected closed contours; otherwise it operates on all closed contours in the layer.
2. **Exact Segment Subdivision:**
   - Decomposes closed contours into directed parametric segments (`Line` and `CubicBez`).
   - Normalizes top-level positive contours to counter-clockwise (`CCW`, winding $+1$) while preserving enclosed counter contours (and `ShapeBooleanRole::Mask` contours) as clockwise (`CW`, winding $-1$).
   - Computes all pairwise segment intersections (`Line`-`Line`, `Line`-`CubicBez`, `CubicBez`-`CubicBez`) via recursive de Casteljau binary subdivision + 2D Newton-Raphson refinement, plus endpoint-on-segment T-junction projection (for collinear overlapping segments such as adjacent `PixelTool` cells or flush `PrimitivesTool` stems).
   - Subdivides each segment at its sorted intersection parameters $0 = t_0 < t_1 < \dots < t_k = 1$ using exact de Casteljau (`CubicBez::subsegment`) and linear interpolation, snapping endpoints to canonical intersection vertices.
3. **Interior & Coincident Segment Filtering:**
   - Evaluates the midpoint $m = e(0.5)$ and unit normal $\vec{n}_L = (-\tau_y, \tau_x)$ of each elementary sub-segment $e$, probing $p_L = m + \delta\vec{n}_L$ and $p_R = m - \delta\vec{n}_L$ against the exact Non-Zero Winding predicate:
     - **Union (`Remove Overlap`):** $\text{in}(p) \iff W_{\text{pos}}(p) > 0 \land W_{\text{mask}}(p) == 0$ (where enclosed CW holes contribute $-1$ to $W_{\text{pos}}$).
     - **Subtract:** $\text{in}(p) \iff W_A(p) > 0 \land W_B(p) == 0$.
     - **Intersect:** $\text{in}(p) \iff W_A(p) > 0 \land W_B(p) > 0$.
   - Keeps only true boundary segments where $\text{in}(p_L) \ne \text{in}(p_R)$ (reversing $e$ when $\text{in}(p_R)$ is true so the filled interior always lies on the left of every kept directed segment), automatically eliminating interior segments, canceling opposite shared edges (such as shared walls between adjacent `PixelTool` cells), and deduplicating coincident edges.
4. **Closed Cycle Reconstruction & Collinear Corner Removal:**
   - Traces closed boundary cycles from the kept directed segments.
   - Merges consecutive collinear straight `Line` segments so merged rectilinear stems and adjacent pixel cells have minimal clean `Corner` vertices.
   - Preserves `NodeType::Smooth` on original smooth curve junctions whose incoming/outgoing cubic tangents remain collinear, while assigning `NodeType::Corner` at newly formed intersection corners.
   - Enforces canonical path direction (`CCW` outer contours, `CW` enclosed counters), sanitizes `selected_nodes`, records an atomic `"Remove Overlap"` Undo snapshot, and satisfies `assert_invariants()`.

---

## 3. Light & Dark Appearance Tokens (`src/tokens/v040_tokens.rs`)

All `v0.4.0` visual overlays support both `GrafiaTheme::sonoma_default()` (Light) and `GrafiaTheme::sonoma_dark()` (Dark) with zero hardcoded RGBA literals outside `src/tokens/`:

| Token Name | Light Mode (`sonoma_default`) | Dark Mode (`sonoma_dark`) | Semantic Role |
| :--- | :--- | :--- | :--- |
| `knife_line_color` | `[128, 128, 128, 230]` | `[165, 165, 170, 230]` | Active knife slicing ray stroke |
| `knife_intersection_fill` | `[128, 128, 128, 255]` | `[185, 185, 190, 255]` | Knife ray-contour intersection dots |
| `primitive_preview_stroke` | `[30, 30, 30, 220]` | `[235, 235, 240, 220]` | Primitive drag preview outline |
| `measurement_line_color` | `[46, 117, 219, 230]` | `[74, 144, 242, 235]` | Measurement ruler ray stroke |
| `measurement_oncurve_label_color` | `[26, 102, 204, 245]` (Blue) | `[90, 165, 255, 250]` | `OnCurve` node coordinate badge |
| `measurement_offcurve_label_color` | `[38, 140, 64, 240]` (Green) | `[82, 196, 110, 245]` | `OffCurve` handle coordinate badge |
| `measurement_segment_delta_color` | `[191, 51, 51, 235]` (Red) | `[242, 95, 95, 245]` | Italic-compensated segment $(\Delta X, \Delta Y)$ badge |
| `annotation_stroke_color` | `[204, 77, 64, 225]` | `[240, 105, 90, 235]` | Unselected canvas annotation stroke |
| `annotation_selected_color` | `[230, 45, 35, 255]` | `[255, 90, 80, 255]` | Selected canvas annotation stroke |
| `pencil_trail_stroke` | `[60, 60, 65, 210]` | `[210, 210, 218, 215]` | Freehand pencil sample trail |
| `pixel_preview_stroke` | `[120, 120, 128, 140]` | `[160, 160, 170, 140]` | Pixel grid cell preview outline |

---

## 4. Normative COTS Verification Matrix (`tests/v040_complete_tests.rs`)

| Test ID | Requirement Under Test | Expected Outcome |
| :--- | :--- | :--- |
| **`TC-V040-001`** | `KnifeTool` (`E`) pairwise closed-contour slicing & `Shift` axis constrain | Dragging across a closed rectangle splits it into 2 valid closed contours along the cut line; `Shift` constrains the cut horizontally or vertically; `Undo`/`Redo` restores exact initial contour. |
| **`TC-V040-002`** | `KnifeTool` (`E`) `Option`/`Alt` add-nodes mode & single-intersection open cut | Dragging with `Alt` inserts `OnCurve` nodes at all intersections without splitting the contour; a single intersection without `Alt` opens the closed contour (`closed == false`) at the cut point. |
| **`TC-V040-003`** | `PrimitivesTool` (`F` / `Shift+F`) `Rectangle` & `Circle` drag creation with `Shift` (square/circle), `Alt` (center), `Space` (pan), and single-click `PrimitiveDialog` | Dragging creates a 4-Line rectangle or 4-CurveSmooth + 8-OffCurve circle ($\kappa = 0.55228475$); `Shift` forces width == height; `Alt` centers on `start_upm`; `Space` translates `start_upm`; single click emits `ToolDialogRequest::Primitive`. |
| **`TC-V040-004`** | `MeasurementTool` (`L` & `Cmd+Ctrl+Opt` temporary override) coordinate & segment delta labels (`Blue` OnCurve, `Green` OffCurve, `Red` italic-compensated segment deltas) | Activating `L` or holding `Cmd+Ctrl+Opt` computes Blue OnCurve labels, Green OffCurve labels, and Red italic-compensated $(\Delta X, \Delta Y)$ segment midpoint labels; releasing `Cmd+Ctrl+Opt` restores the previous tool. |
| **`TC-V040-005`** | `MeasurementTool` (`L`) ray intersections (contours + Left/Right Sidebearings), distance/angle badges (`0°` = vertical up CCW), node/anchor snap, and `'G'` key promotion to Measurement Guide | Dragging across a contour computes sorted intersections `[LSB, contour_entry, contour_exit, RSB]` with exact pairwise distances, snaps endpoints to hit nodes/anchors, computes CCW angle from vertical, and pressing `'G'` creates a local `Guide` with `show_measurement = true`. |
| **`TC-V040-006`** | `AnnotationTool` (`A` / `Shift+A`) all 5 `CanvasAnnotation` types (`Text`, `Arrow`, `Circle`, `Plus`, `Minus`), handle resize/rotate, body drag (`Shift` constrain), `Cmd+A`, `Delete`, and `Undo`/`Redo` | Clicking empty canvas creates each of the 5 annotation types (`Circle` default radius `100.0` UPM); dragging handle resizes `Circle`/`Text` or rotates `Arrow`; `Cmd+A` selects all annotations; `Delete` removes selected annotations; `Undo`/`Redo` is bit-exact. |
| **`TC-V040-007`** | `PencilTool` (`B`) freehand sampling, trail preview, and automatic cubic Bézier curve fitting (open & closed contours) | Dragging with `PencilTool` records sample points, renders trail preview, and on `PointerUp` fits a smooth cubic contour (`closed = true` when ending near start, `closed = false` otherwise) with full `Undo`/`Redo`. |
| **`TC-V040-008`** | `PixelTool` (`X`) grid-aligned pixel cell placement, automatic erase on occupied cells, and `Shift` Bresenham pixel line | Clicking/dragging empty cells adds `step × step` pixel contours; clicking an occupied cell switches the drag to erase mode; `Shift+click` draws a Bresenham line of pixel cells from the last cell. |
| **`TC-V040-009`** | 1-to-1 `egui` $\leftrightarrow$ `tiny-skia` headless parity & Light/Dark (`AppearanceMode`) rendering across all 6 `v0.4.0` tools | Every `v0.4.0` overlay (`Knife`, `Primitives`, `Measurement`, `Annotation`, `Pencil`, `Pixel`) produces non-empty, distinct, deterministic pixel output in both `sonoma_default()` (Light) and `sonoma_dark()` (Dark) themes. |
| **`TC-V040-019`** | Multi-step Knife open-cut + `Connect Open Endpoints` + `Add Inflections` selection invariant & shared-coordinate open-endpoint hit-test priority | Slicing a closed contour open twice with `KnifeTool`, connecting open endpoints, selecting a contour (including `OffCurve` handles), and invoking `add_inflection_points()` preserves all state invariants; `hit_test_node` prioritizes open endpoints over closed nodes at shared coordinates. |
| **`TC-V040-020`** | State-dependent pull-based `arbtest` multi-tool action chains (`> 200` seeds, `12..=25` steps per chain across `/A`, `/O`, `/n`, `/s`) | Arbitrary sequences of `Knife`, `Remove Overlap`, `Connect Open Endpoints`, `Close Open Paths`, `Add Inflections`, `Add Extremes`, `Tidy Up`, `Align`, `Delete`, `Pixel`, `Primitives`, `Clipboard`, `Transform`, and `Undo`/`Redo` maintain 100% invariant integrity. |
| **`TC-V040-021`** | Boolean `Remove Overlap` (`Cmd+Shift+O`) on overlapping rectilinear stems (`T`/`+`), adjacent `PixelTool` cells, overlapping curved `Circle` contours, and enclosed counters (plus `Subtract` / `Intersect`) | Merging overlapping rectangles (`+` cross -> 12-node CCW octilinear cross), adjacent `PixelTool` cells (shared interior wall removed + collinear outer edges merged into a 4-node rectangle), and overlapping `Circle` cubic contours (with and without an enclosed CW counter hole) produces valid closed contours with exact `Undo`/`Redo` roundtrip. |
| **`TC-V040-022`** | Multi-hit `KnifeTool` (`E`) slicing across $\ge 4$ intersections (both a single 2-stem U/`/n` contour with 4 hits AND an `/o` outer CCW + inner CW counter ring with 4 total hits) | Slicing horizontally across both stems of `/n` (4 hits on 1 contour) produces 3 valid disjoint closed contours; slicing horizontally across `/o` (2 hits on outer contour + 2 hits on inner counter = 4 hits) pairs `(hit[0]..hit[1])` and `(hit[2]..hit[3])` to produce 2 valid disjoint top/bottom arch contours without cross-counter bridges, passing `assert_invariants()`. |

---

## 5. Nice-to-Have Backlog — Tablet & Stylus Ergonomics

- **`NTH-TABLET-001` — Floating Left-Thumb Modifier & Gesture Pad:**
  - When operating `grafia` on a stylus-equipped tablet without a physical keyboard attached, provide a draggable, edge-dockable **Left-Thumb Floating Cluster** on the canvas:
    1. **Momentary & Sticky Thumb Modifiers:** Hold-or-latch thumb pads for `⇧ Shift` ($0^\circ/45^\circ/90^\circ$ constrain & additive selection), `⌥ Alt / Option` (cusp/corner handle breaking, `Line` $\to$ `Cubic` conversion, `Knife` add-nodes mode), `⌃ Ctrl`, `⌘ Cmd` (temporary `Select` override), and `␣ Space` (temporary `PreviewFilled` + hand pan) while the stylus draws or drags Bézier handles simultaneously.
    2. **Quick Thumb Actions:** One-tap `↶ Undo`, `↷ Redo`, and `⌫ Delete` / `✂ Cut Open` directly under the left thumb.
    3. **Palm Rejection & Stylus Hover Priority:** Separate stylus hover/drag pointer stream from two-finger multi-touch canvas pan/pinch-zoom gestures.
