# 5-Pass Rendering Pipeline, GPU Tessellation & Zero-CPU Idle Caching (`grafia::render`)

This module contains the multi-pass visual projections for `grafia`, bridging mathematical vector contours to both live `egui::Painter` GPU meshes and offscreen `tiny_skia::Pixmap` buffers with **1:1 visual parity**.

---

## 1. The 5-Pass Canvas Rendering Architecture

Every frame rendered in `CanvasViewMode::EditWireframe` executes five ordered visual passes from back to front so guides, fills, wireframes, and interactive HUD widgets never obscure each other:

1. **Pass 1 — Background & Reference Sub-Layers:**
   - Canvas background fill (`Light` or `Dark` appearance token), optional `CanvasBackgroundImage` with opacity/crop/tint, low-resolution PPEM pixel-preview proofing grid (`show_pixel_preview_ppem`), `CustomReferenceFrame` (outer/inner em-boxes, $N_x \times N_y$ subdivisions, keyline circles, and stem reference bands), canvas grid lines (`paint_canvas_grid_egui`), typographic metric lines, `AlignmentZone` bands, PostScript stem/ghost hint bands, and local/global `Guide` rays.
2. **Pass 2 — Multi-Layer Vector Fills, Strokes & Components:**
   - COLRv1-style `CanvasColorSublayer` stack (`Solid`, `LinearGradient`, `RadialGradient`, `SweepGradient`, `Bitmap`, `ClippingMask` composited with Porter-Duff `SrcOver`), passive text-stream neighbour slots, pre-resolved `CanvasComponent` instances, `GhostMarkPath` diacritic clouds, `backdrop_ghost_contours`, `background_contours`, `CompanionMasterLayer` compatibility overlays, and live `ContourStrokeStyle` expanded outlines.
3. **Pass 3 — Active Contour Wireframe & Spline Overlays:**
   - Anti-aliased cubic Bézier & solved Hobby/Euler-Spiro contour outlines, start-node winding direction arrows (`CCW`/`CW`), open-endpoint ticks, off-curve handle control lines (`BCP` rays), Higher-Order Interpolation (`HOI`) control polygons, and TrueType visual instruction arrows.
4. **Pass 4 — Interactive Nodes, Anchors & Smart Handles:**
   - Color-coded `Corner` (square), `Smooth` (circle), `OffCurve` (handle knob), `HobbyStar` ($\star$ badge), and `EulerSpiro` ($G^2$ spiral badge) nodes, virtual self-intersection `ExtraNode` diamonds, named 2D `CanvasAnchor` diamonds, parametric `CanvasSmartHandle` slider tracks, gradient stop handles, and duplicate-node warning rings.
5. **Pass 5 — Active Tool Reticles, Rulers & Telemetry HUD:**
   - 8+4 Selection Bounding Box (`BBox`) with outer-corner rotation and mid-edge shear handles, `Knife` slice line and intersection dots, `Measurement` ray with live distance/angle pills, `CanvasAnnotation` callouts, precision zoom loupe, top/left UPM rulers with cursor hairlines, and the bottom `SelectionInfo` HUD bar.

---

## 2. Generation-Gated Caching (`content_generation()`) for 0% Idle CPU

Complex vector canvases with hundreds of cubic segments, live boolean strokes, self-intersection solvers, and blurred preview strips can drain laptop batteries if recomputed at 60–120 Hz when nothing has changed.

`grafia` solves this with a monotonically increasing 64-bit **`content_generation()`** counter on [`crate::adapters::GrafiaCanvasState`]:

- **Geometry & Solver Cache (`CanvasRenderCache`):** Expensive $O(S^2)$ cubic self-intersection detection (`compute_layer_extra_nodes`), live stroke offset expansion, spline-to-cubic solving (`contour_to_bezpath_with_splines`), and GPU texture uploads are keyed by `(content_generation, viewport_key)`. During static idle frames or pure pointer hover movements, zero solver allocations and zero GPU texture uploads occur.
- **Interactive Drag Downsampling (`effective_color_layer_pixmap_dimensions`):** While actively dragging gradient stops or nodes on high-DPI viewports, multi-layer software rasterization automatically caps intermediate pixmap dimensions to `MAX_INTERACTIVE_DRAG_PIXMAP_DIM` and restores full native resolution on pointer release.
- **Preview Strip Cache (`PreviewStripCache` & `GrafiaPreviewWidget`):** The bottom multi-glyph preview panel (including separable horizontal + vertical Gaussian blur for optical weight proofing) caches its RGBA buffer and `egui::TextureHandle` by `(content_generation, PreviewPanelConfig)`, achieving `0.0 ms` CPU cost on unchanged frames.
