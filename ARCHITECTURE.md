# `grafia`: Architecture Overview

`grafia` is a high-performance, modular, multiplatform Rust 2D cubic Bézier contour, glyph, and vector canvas engine for typographic and vector design applications.

It serves as the **View & Controller** layer in typography software, wrapping Linebender and Google Fonts libraries (`kurbo`, `lyon_tessellation`, `egui`, `tiny-skia`) with deterministic, invariant-checked interaction handling.

---

## Architecture Diagram

```mermaid
flowchart TD
    subgraph Consumers ["1. Consumer Shells (Multiplatform Host Applications)"]
        DeskShell["Desktop Font & Vector Editors (macOS / Windows / Linux)"]
        TabShell["Tablet & Stylus Apps (iPadOS / Android / Stylus)"]
        WebShell["Web Vector Editors (WASM / WebGPU)"]
        ExternalApps["Other Typographic & CAD Applications"]
    end

    subgraph Inputs ["2. Hardware Input (Gestures)"]
        MouseKey["Mouse & Keyboard<br/>(Clicks, Drags, Modifiers)"]
        TouchIn["Multi-Touch<br/>(Pinch-Zoom, Pan)"]
        StylusIn["Stylus (W3C Pointer Events L3)<br/>(Pressure 0.0..1.0, Tilt X/Y)"]
    end

    subgraph Grafia_Engine ["3. grafia Engine (View & Controller)"]
        subgraph Interaction_Subsystem ["Interaction & State Machine"]
            Gestures["grafia::interaction::gestures<br/>CanvasGesture Normalizer"]
            HFSM["grafia::interaction::tool_state<br/>HFSM (Select, Pen, Knife, Text, Measure)"]
            Keymap["grafia::adapters::keyboard<br/>Configurable Shortcut Dispatch"]
            History["grafia::interaction::history<br/>Atomic Snapshot Undo/Redo"]
        end

        subgraph Stream_Subsystem ["Typographic Stream & Metrics"]
            TextStream["CanvasTextStream (Continuous Baseline)"]
            ActiveGlyph["Active Editable Glyph<br/>(Contours, Nodes, BCP Handles)"]
            PassiveGlyphs["Passive Context Glyphs<br/>(Filled Silhouette Context)"]
            SpacingKerning["Interactive Spacing & Pair Kerning Deltas"]
        end

        subgraph Structure_Subsystem ["Structure & Snapping"]
            Hierarchy["Glyph & Contour Hierarchy<br/>Non-Zero Winding & Companion Masters"]
            Snapping["grafia::snapping::snap_engine<br/>Grid • Alignment Zones • Angled Guides"]
        end

        subgraph Overlay_Subsystem ["Multi-Pass Render Pipeline (5 Passes)"]
            P1["Pass 1: Background (Metrics, Grid, Guides, Zones)"]
            P2["Pass 2: Context Fills & Companion Master Wireframes"]
            P3["Pass 3: Active Geometry (Bezier Curves & Nodes)"]
            P4["Pass 4: Analytical Overlays (Curvature Heatmap, Stem Gauge, Audit)"]
            P5["Pass 5: Canvas Indicators (Marquee, Snapping Lines, Vector Cursors)"]
            SelectionAPI["Headless SelectionInfo & Host Bridge API<br/>(load_glyph / export_glyph / GrafiaCanvasResponse)"]
        end
    end

    subgraph COTS_Foundation ["4. Mathematical & Rendering Foundation"]
        Kurbo["kurbo (Cubic Bezier Math & Affine Matrices)"]
        Lyon["lyon_tessellation (Non-Zero Winding Triangulation)"]
        Egui["egui (Live Interactive GPU Painter)"]
        TinySkia["tiny-skia (Deterministic Offscreen Pixel Verifier)"]
        ModelBackend["Host Application Font & Document Backend"]
    end

    DeskShell --> Gestures
    TabShell --> Gestures
    WebShell --> Gestures
    ExternalApps --> Gestures

    MouseKey --> Gestures
    TouchIn --> Gestures
    StylusIn --> Gestures

    Gestures --> HFSM
    Keymap --> HFSM
    HFSM --> History
    HFSM --> ActiveGlyph
    HFSM --> SpacingKerning

    TextStream --> ActiveGlyph
    TextStream --> PassiveGlyphs
    SpacingKerning --> TextStream

    Snapping --> ActiveGlyph
    ActiveGlyph --> Hierarchy

    P1 --> P2 --> P3 --> P4 --> P5

    ActiveGlyph --> Kurbo
    Hierarchy --> Lyon
    Overlay_Subsystem --> Egui
    Overlay_Subsystem --> TinySkia
    SelectionAPI --> ModelBackend
```

---

## Specifications

The formal normative specifications live in:
- [`SPEC-001-GLIFO-UNIVERSAL-GLYPH-CANVAS-ENGINE.md`](./SPEC-001-GLIFO-UNIVERSAL-GLYPH-CANVAS-ENGINE.md)
- [`SPEC-002-V0.1-CANVAS-INTERACTION-AND-RENDERING.md`](./SPEC-002-V0.1-CANVAS-INTERACTION-AND-RENDERING.md)
- [`SPEC-003-V0.2-TRANSFORMS-LASSO-AND-PATH-OPS.md`](./SPEC-003-V0.2-TRANSFORMS-LASSO-AND-PATH-OPS.md)
- [`SPEC-004-V0.3-GUIDES-SNAP-ENGINE-AND-ALIGNMENT-ZONES.md`](./SPEC-004-V0.3-GUIDES-SNAP-ENGINE-AND-ALIGNMENT-ZONES.md)
- [`SPEC-005-V0.4-EDIT-TOOLS-AND-ANNOTATIONS.md`](./SPEC-005-V0.4-EDIT-TOOLS-AND-ANNOTATIONS.md)
- [`SPEC-006-V0.5-TEXT-STREAM-AND-KERNING.md`](./SPEC-006-V0.5-TEXT-STREAM-AND-KERNING.md)
- [`SPEC-007-V0.6-PREVIEW-BLUR-AND-SELECTION-INFO.md`](./SPEC-007-V0.6-PREVIEW-BLUR-AND-SELECTION-INFO.md)
- [`SPEC-008-V0.7-BACKGROUND-ANCHORS-COMPONENTS-STROKE.md`](./SPEC-008-V0.7-BACKGROUND-ANCHORS-COMPONENTS-STROKE.md)
- [`SPEC-009-V0.8-CURVE-HARMONIZATION-TUNNI-AND-OUTLINE-AUDIT.md`](./SPEC-009-V0.8-CURVE-HARMONIZATION-TUNNI-AND-OUTLINE-AUDIT.md)
- [`SPEC-010-V0.9-SMART-CORNERS-CALLIGRAPHIC-BRUSH-AND-STEM-GAUGE.md`](./SPEC-010-V0.9-SMART-CORNERS-CALLIGRAPHIC-BRUSH-AND-STEM-GAUGE.md)
