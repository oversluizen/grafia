# Deterministic Tool State Machines & Atomic Undo/Redo History (`grafia::interaction`)

This module contains the pure finite-state machines (`SelectToolState`, `PenToolState`), normalized input gestures (`CanvasGesture`, `Modifiers`), and transactional undo/redo history (`CommandHistory`, `GlyphSnapshot`).

---

## 1. Finite-State Machines (`SelectToolState` & `PenToolState`)

Pointer and keyboard interactions in a professional vector editor are highly modal: a pointer-down event might start a single-node drag, an inverse-Bernstein curve-belly drag, a rubber-band marquee selection, an `Alt`-duplicated contour drag, or a `Space`-bar viewport pan.

By modeling tool lifecycles as explicit Rust enums (`SelectToolState` and `PenToolState`), invalid intermediate states are unrepresentable:

- **`SelectToolState`:**
  - `Idle`: Waiting for pointer or keyboard input.
  - `MarqueeSelecting { start, current }`: Dragging a rectangular selection box (supporting `Shift` XOR toggle and `Alt` segment-select).
  - `DraggingNodes { last_pos }`: Translating selected on-curve and off-curve nodes with magnetic snapping and $45^\circ$ `Shift`-axis constraints.
  - `DraggingSegment { contour_idx, segment, t, last_pos }`: Live 4-case inverse-Bernstein curve-belly or parallel line-segment dragging.
  - `SlidingNodeOnCurve { ... }`: Sliding an on-curve node smoothly along its adjacent curve trajectory (`Cmd/Ctrl` + drag).
  - `PanningViewport { last_pos }`: Temporary `Space`-bar hand panning without losing the active selection.
- **`PenToolState`:**
  - `Idle`: Ready to start a new contour or resume an existing open contour endpoint.
  - `PlacingPoint`: Pointer pressed at a new anchor position; dragging pulls out symmetric $G^1$ cubic handles (`Smooth`), while holding `Alt` decouples the outgoing handle (`Corner` cusp) and holding `Space` repositions the anchor on the fly.
  - `DraggingHandles { node_id }`: Pulling handles out of a newly placed on-curve node before pointer release.

---

## 2. Transactional Bit-Exact Undo & Redo (`CommandHistory`, `GlyphSnapshot`)

Every mutating action in `grafia` obeys the **Universal Bit-Exact Undo/Redo Invariant**:

$$\text{State}_0 \xrightarrow{\text{Action}} \text{State}_1 \xrightarrow{\text{Undo}} \text{State}_0 \xrightarrow{\text{Redo}} \text{State}_1$$

- **Continuous Gesture Coalescing:** When a user drags a node or bounding box across 120 frames, `grafia` captures a single `before` `GlyphSnapshot` at drag start and commits **one atomic `HistoryTransaction`** when the pointer is released—never flooding the undo stack with per-frame micro-steps.
- **Complete Scene Snapshots (`GlyphSnapshot`):** Captures foreground and background contours, closed flags, active selection, annotations, guides, 2D anchors, components, smart handles, color sublayers, PostScript hints, TrueType instructions, and HOI layers using structural `Arc` sharing (`SnapshotLayerAttachments`, `SnapshotHintingAndHoiState`) so snapshots remain fast and memory-light.
