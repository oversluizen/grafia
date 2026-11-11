# SPEC-001: Universal Interactive Glyph Canvas & Interaction Engine (`grafia`)

- **Document ID:** SPEC-001
- **Crate Naam:** `grafia`
- **Status:** Approved / Normative
- **Versie:** 0.9.0
- **Positionering in Ecosysteem:** Standalone interactieve View & Controller crate bovenop het Linebender / Google Fonts ecosysteem (`kurbo`, `norad`, `skrifa`, `vello`)
- **Normatieve Standaarden:** ISO/IEC/IEEE 42010:2011, ISO/IEC/IEEE 29119-3:2021, ISO/IEC/IEEE 29148:2018, IEEE 1012-2016, OpenType 1.9, Computer-Aided Geometric Design (CAGD), ISO 9241-9 (Ergonomics of Human-System Interaction), W3C Pointer Events Level 3 (Stylus Pressure/Tilt), SemVer 2.0.0.

---

## 🎯 1. Doelstelling & Scope-Afbakening

### 1.1 De Missie van `grafia`
`grafia` vult de ontbrekende schakel in het wereldwijde open-source font-ecosysteem. Waar Google en Linebender zuivere reken- en bestandsbibliotheken leveren (`kurbo` voor curven, `norad` voor UFO, `skrifa` voor rastering), ontbrak een universele, hoogwaardige **interactieve Canvas- en Gereedschapsmotor**.

`grafia` levert een complete, multiplatform, hardware-onafhankelijke **View & Controller**-laag voor typografische desktop-, tablet- en web-applicaties. Het combineert vloeiende directe vector-ergonomie (ISO 9241-9) met exacte CAGD-geometrie ($G^1/G^2$ continuïteit, raaklijn-balanslijnen en proportionele servo-schaling), verrijkt met **drukgevoelige W3C Pointer Events Level 3 stylus-calligrafie**.

### 1.2 Strikte Scope-Grenzen (Preventie van Over- en Under-engineering)

```
+---------------------------------------------------------------------------------------+
|                           100% BINNEN LEAN GRAFIA (CANVAS VC)                         |
|  - Viewport & Tekenkoördinaten (ScreenDp <-> FontUnits, Y-Up projectie)              |
|  - Input Normalizer (Muis, Toetsenborden, Touch, W3C Stylus druk/kanteling)           |
|  - Gereedschappen State Machine (Select V, Lasso Shift+V, Pen P, Pencil B, Pixel X,   |
|    Knife E, Rotate R, Scale S, Primitives F, Annotation A, Text/Kerning T,            |
|    Measurement L / Cmd+Ctrl+Opt, Hand H / Space, Zoom Z / Cmd+Space, Brush)           |
|  - Doorlopende Tekststrook (CanvasTextStream: actieve letter vs passieve context)    |
|  - Spatiëring & Kerning interactie (LSB/RSB & kerning slepen, Intersection Knobs,     |
|    geel/lichtblauw kerning-highlights, Ctrl/Cmd+Pijltjes sneltoetsen)                 |
|  - SnapEngine & Guides (Lokale blauwe & globale rode guides Cmd+Shift+L, Measurement  |
|    Guides, magnetische rode snap-lijnen met Ctrl-bypass, Alignment Zone badges)       |
|  - Preview Strip Render Engine (PreviewRenderConfig: Gaussian Blur σ ∈ [0.0, 20.0],   |
|    Flip verticaal ondersteboven, Negative wit-op-zwart, extra_spacing 1.25)           |
|  - Grafische Canvas Overlays (Metrieken, Curven, Nodes, Annotaties, Meting-getallen,  |
|    Curvature Comb & Heatmap, Stem Thickness Gauge, Counter Area/Centroid, Symmetry)   |
|  - Pure 2D Geometrische Primitieven (Boolean Remove Overlap, Fit Curve, Stroke Offset,|
|    Background Contours, CanvasAnchor punten, voor-opgeloste CanvasComponent paden,    |
|    G2 Curve Harmonization, Tangent Balance Lines, Smart Corners & Ink Traps)          |
|  - Headless SelectionInfo & Mutatie-Contract (Zuiver geometrische API voor 5 modi)    |
|  - Canvas Undo/Redo Transactiestack (O(1) CoW snapshot coalescing)                   |
+---------------------------------------------------------------------------------------+
                                           |
                                           v
+---------------------------------------------------------------------------------------+
|                   100% BUITEN GRAFIA (HOST APPLICATION & FONT MODEL BACKEND)          |
|  - Zwevende Info-Box UI (Cmd+Shift+I), Toolbars, Sidebars, Palettes (-> Host UI)      |
|  - Split-view balk en UI-knoppen/slider van het Preview-paneel (-> Host UI)           |
|  - Invoerdialogen & Pickers (Primitives 100x100, Component Picker, Text) (-> Host UI) |
|  - Font-Brede Databases: Kerning-groepen (@MMK), Metrics Keys (=n+10) (-> Host Model) |
|  - Unicode & GlyphData: Standaard anker-recepten (Cmd+U) & Mark Cloud (-> Host Model) |
|  - Component-Graaf (DAG) & Automatic Alignment over het gehele font (-> Host Model)   |
|  - Font I/O: Lezen/schrijven van .ufo, .otf, .ttf, ontwerpbestanden (-> Host Backend) |
|  - OpenType Features: GSUB, GPOS, FEA compilatie (-> Host OpenType Compiler)          |
|  - Auto-Kerning Wiskunde: Optische kerning-optimalisatie (-> Host Spacing Engine)     |
|  - Platform Shell: Vensterbeheer, native menu's, OS styling (-> Host Application)     |
+---------------------------------------------------------------------------------------+
```

### 1.3 Normatieve Scheidslijn: 2D Canvas Primitief (`grafia`) vs. Font Model Backend & Host UI

De fundamentele architectuurwet van `grafia` luidt:
> **`grafia` kent uitsluitend de 2D-geometrie die op het huidige canvas aanwezig is.** De crate heeft **nul kennis** van het omliggende font-bestand als geheel (geen lijst van alle overige glyphs in het font, geen Unicode/alfabet-database, geen kerning-groepen en geen zijpaneel- of dialoog-UI). Alles wat een lookup in een font-brede collectie of een venster-formulier vereist, is de verantwoordelijkheid van de **Host Application / Font Model Backend**.

| Domein | ✅ Binnen `grafia` (2D Canvas View & Controller Primitief) | ❌ Buiten `grafia` (Host Application & Font Model Backend) |
| :--- | :--- | :--- |
| **Multi-Glyph Tekst & Kerning (`v0.5.0`)** | Positioneert en rendert een aangeleverde reeks `GlyphSlot`s op één doorlopende basislijn (Pass 2 passief gevuld, Pass 3 actief bewerkbaar); berekent `Measurement Line` met `Intersection Knobs`; toont gele (`>0`) en lichtblauwe (`<0`) kerning-highlights; stuurt bij slepen of sneltoetsen pure `SpacingDelta` / `KerningDelta` events terug. | Beheert de font-brede **Kerning-Database**, kerning-klassen (`@MMK_L_*`, `@MMK_R_*`), groeps-kerning vs. paar-uitzonderingen, OpenType `GPOS`-generatie en het *Kerning* tabelvenster. |
| **`SelectionInfo` & Metrics Keys (`v0.6.0`)** | Berekent uitsluitend de **zuivere geometrische feiten** van de actieve canvas-selectie: `lsb`, `rsb`, `advance_width`, $(X, Y)$, `OffCurve` $(\Delta X, \Delta Y, L)$ met radiale lengte-mutator, 9-punts `TransformOrigin` bounding-box $(X, Y, W, H)$, `■/□` knoop-telling en `GuideInfo`. | Beheert de **Metrics Keys formule-evaluator (`=H`, `=n+10`, `=|n`)** en de font-brede **Metrics-DAG** (omdat `=n+10` de breedte van glyph `/n` elders in het font moet opzoeken), plus het zwevende Info-Box UI-formulier (`Cmd+Shift+I`). |
| **Preview Strip (`v0.6.0`)** | Zuivere offscreen/canvas raster-functie `render_preview_strip(slots, config)` met `inverted` (wit-op-zwart), `flipped` (verticaal gespiegeld) en een scheidbare 2-pass 1D **Gaussian Blur ($\sigma \in [0.0, 20.0]\text{ px}$)**. | De split-view scheidingsbalk, de `Black`/`Flip`-knoppen en het `Blur`-schuifregelaar-widget in het host-venster. |
| **Anchors & Mark Cloud (`v0.7.0`)** | Tekent, selecteert en versleept een lijst `Vec<CanvasAnchor { id, name, pos, selected }>` (oranje ruitjes met `SnapEngine`-koppeling) en rendert optioneel aangeleverde `ghost_paths: Vec<BezPath>` in Pass 1/2. | Kent de Unicode/GlyphData-regels voor `Cmd+U` (*Set Anchors*: welke ankers een letter standaard hoort te krijgen) en zoekt in de font-collectie de bijbehorende accent-glyphs op om hun contouren als `ghost_paths` aan `grafia` mee te geven (*Mark Preview Cloud*). |
| **Components (`v0.7.0`)** | Tekent, selecteert en transformeert voor-opgeloste `CanvasComponent { id, name, outline: BezPath, transform: Affine, selected }` referenties op het canvas en zet deze bij `Decompose` (`Cmd+Shift+D`) om in bewerkbare `Contour`s. | Beheert de *Add Component* zoekdialoog (`Cmd+Shift+C`), de font-brede **Component Dependency Graph (DAG)** en de automatische uitlijning (*Automatic Alignment*) tussen bron-glyphs en samengestelde glyphs. |
| **Background Layer, Fit Curve & Live Stroke (`v0.7.0`)** | Beheert `background_contours: Vec<Contour>` (`Cmd+B` toggle, `Cmd+J` copy to background, `Cmd+K` swap) met haarlijn-weergave in Pass 1; voert de wiskundige kubische `fit_selected_curves(tension)` (`Ctrl+Opt+1..8`) uit; rendert live `stroke_width` / `stroke_height` contour-offsets. | Bouwt de omliggende Inspector-zijpanelen (*Layers Palette*, *Fit Curve Palette* met de 8 knoppen $57\%..92\%$, en *Stroke Palette* met invoervelden). |
| **$G^2$ Curve Harmonization, Balance Lines, Servo, Trajectory & Outline Audit (`v0.8.0`)** | Voert $G^2$-krommingsegalisatie uit op gladde knooppunten, berekent en versleept raaklijn-balanslijnen rond de raaklijn-driehoek-apex, past proportionele servo-schaling en geïnterpoleerde knoop-nudge toe, analyseert multi-master knooptrajecten op hoek-/verhoudingsknikken, en detecteert contour-defecten. | Beheert de multi-master font-collectie, designspace-asdefinities en externe audit-overzichtspanelen. |
| **Smart Corners, Ink Traps, Stylus Brush & Optische Diagnostiek (`v0.9.0`)** | Evalueert niet-destructieve parametrische hoekstralen ($R > 0$ afronding, $R < 0$ omgekeerde boog, `is_ink_trap` wig-inkeping), W3C Pointer Events Level 3 druk-/kantelings-calligrafie, krommings-heatmap, orthogonale stamdiktemeter, counter-oppervlakte/zwaartepunt en symmetrie-overlay. | Beheert externe penseel-bibliotheken, export-compilatiepijplijnen en zijpaneel-instellingen. |

---

## 🏛️ 2. Modulaire Crate Architectuur

`grafia` is opgedeeld in strikt ontkoppelde submodules conform de NASA/Google eSLOC standaarden ($\le 300$ regels per module):

```
grafia/
├── Cargo.toml
├── SPEC-001..010-UNIVERSAL-GLYPH-CANVAS-ENGINE.md
├── ARCHITECTURE.md
├── src/
│   ├── lib.rs                  // Publieke re-exports
│   ├── core/                   // 1. Pure Wiskunde & Scene Graph (0% GUI)
│   │   ├── mod.rs
│   │   ├── coordinates.rs      // FontUnit, FontPoint, ScreenDp, ScreenPoint
│   │   ├── viewport_matrix.rs  // Affine projecties, Zoom, Pan
│   │   ├── scene_graph.rs      // Universele vector-boom (Paden, Componenten)
│   │   └── bezier_ops.rs       // Collineaire BCP raaklijnen, smooth/corner conversie
│   │
│   ├── interaction/            // 2. Hardware Normalizer & State Machine
│   │   ├── mod.rs
│   │   ├── gestures.rs         // CanvasGesture (Muis, Toetsen, W3C Stylus)
│   │   ├── tool_state.rs       // HFSM: SelectState, PenState, KnifeState
│   │   ├── keymap.rs           // Configureerbare ActionMap sneltoets-profielen
│   │   └── history.rs          // Undo/Redo CoW memento command coalescing
│   │
│   ├── stream/                 // 3. Typografische Tekststrook (Edit Tab)
│   │   ├── mod.rs
│   │   ├── text_stream.rs      // Doorlopende basislijn, actieve vs passieve glyphs
│   │   └── spacing_kerning.rs  // LSB/RSB interactie, live drag, paar-kerning
│   │
│   ├── snapping/               // 4. Magnetische Referenties
│   │   ├── mod.rs
│   │   ├── snap_engine.rs      // Afstandsberekening en magnetische snap
│   │   ├── metrics_lines.rs    // Baseline, x-Height, Cap-Height, Alignment Zones
│   │   └── guides.rs           // Lokale, globale en gehoekte hulplijnen
│   │
│   ├── hierarchy/              // 5. Lagen-, Vormen- en Thumbnail-Model
│   │   ├── mod.rs
│   │   ├── layer_model.rs      // Contouren, componenten, maskers, volgorde
│   │   ├── winding.rs          // Shoelace signed area (Clockwise vs Counter)
│   │   └── thumbnail.rs        // 24x16 dp vector thumbnail bbox projectie
│   │
│   ├── overlays/               // 6. Gelaagde Render- & Analyse-Pijplijn
│   │   ├── mod.rs
│   │   ├── pipeline.rs         // 5-pass canvas rendering
│   │   ├── curvature_comb.rs   // Curvature comb (2e afgeleide versnellingsanalyse)
│   │   └── measurement.rs      // Snijdende linialen en diktemetingen
│   │
│   └── adapters/               // 7. GUI & Platform Koppelingen
│       ├── mod.rs
│       └── egui_adapter.rs     // Drop-in GrafiaCanvasWidget voor egui host-applicaties
```

---

## 📐 3. Gedetailleerde Deelspecificaties

### 3.1 Coördinaten-Integriteit (`grafia::core::coordinates`)
- **`FontUnit` (f64):** Integer of sub-eenheid in font design space. $Y = 0$ is de basislijn, $Y > 0$ groeit omhoog (Cartesiaans).
- **`ScreenDp` (f32):** Logische schermpunten. $Y = 0$ is de bovenrand van het scherm, $Y > 0$ groeit omlaag.
- **Inversie Invariant:** Type-systeem dwingt af dat scherm- en font-eenheden nooit optelbaar zijn zonder `ViewportMatrix::to_font()` of `to_screen()`. Afrondingstolerantie $\epsilon \le 10^{-5}$.

### 3.2 Stylus Calligrafie (`grafia::interaction::gestures`)
- Volgt W3C Pointer Events Level 3 specificatie:
  - `pressure: f32` genormaliseerd van `0.0` (geen druk) tot `1.0` (maximale druk). Muis klik retourneert exact `1.0` of `0.0`.
  - `tilt_x / tilt_y: f32` (-90° tot +90°): Registreert de fysieke hellingshoek van de penpunt voor brede calligrafische stiften (Arabic, Gothic, Indic).
  - `altitude / azimuth`: Berekent de 3D-vector van de stift voor dynamische `kurbo::BezPath` contour-expansie.

### 3.3 De Doorlopende Tekststrook (`grafia::stream::text_stream`)
- De bewerkingsruimte is een oneindig horizontale band met een gemeenschappelijke basislijn.
- Bevat een geordende lijst `Vec<GlyphInstance>`.
- De actieve glyph bevindt zich op `active_index`. Deze glyph toont knooppunten (blauw vierkant, groene cirkel, bruine BCP).
- Alle overige glyphs worden gerenderd in `Pass 2 (Glyph Fill)` via hun advance-widths zonder knooppunten.
- Muis dubbelklik op een passieve glyph transfereert de `active_index` zonder context-verlies.

### 3.4 Spatiëring & Kerning (`grafia::stream::spacing_kerning`)
- LSB-lijn op $X = 0$, RSB-lijn op $X = \text{Advance Width}$.
- Muis-hit op LSB of RSB binnen $6.0\,\text{dp}$ activeert `DragSidebearing`.
- Toetsencombinatie `Ctrl+Links/Rechts` verspringt sidebearing met $1\,\text{unit}$ (`Shift+Ctrl` met $10\,\text{units}$).
- Muisklik tussen twee letters met de Tekst-tool activeert `KerningPairSession`. Slepen past de kerning-delta live toe en schuift alle opvolgende letters op de basislijn realtime mee.

### 3.5 Gelaagde Overlay-Pijplijn & Analyse (`grafia::overlays`)
De canvas-weergave doorloopt 5 strikte render-passes:
1. **Pass 1 (Background):** Metrieklijnen (Baseline, x-Height, Ascender, Descender), rasterstippen, globale en lokale hulplijnen.
2. **Pass 2 (Context Fills):** Gevulde zwarte weergave van passieve buurletters in de tekststroom.
3. **Pass 3 (Active Geometry):** Het actieve bezier-pad, on-curve knooppunten, off-curve BCP raaklijnen, selectiekaders.
4. **Pass 4 (Analytical Overlays):** 
   - **Curvature Comb & Heatmap:** Tekent krommingskammen loodrecht op de curve; kleur verloopt van groen ($C^2$ continuïteit) via oranje naar rood ($C^0$ knik).
   - **Measurement:** Tekent snijdende liniaallijnen en toont stamdiktes in UPM.
5. **Pass 5 (Canvas Indicators & Rubberband):** Marquee-selectiekaders, magnetische snapping-lijnen en puur grafische on-canvas badges. *(Alle formulier-UI met tekstinvoervelden zoals de Info-Box `Cmd+Shift+I`, toolbars en popovers vallen buiten `grafia` en worden door de consumer-shell / host-applicatie gebouwd bovenop `grafia`'s headless `SelectionInfo` data-API).*

---

## 🧪 4. Traceability & Verificatie Matrix (`TC-GRAFIA-001` t/m `TC-GRAFIA-025`)

| Test Case ID | Doel / Component | Test Invariant & Norm |
| :--- | :--- | :--- |
| **`TC-GRAFIA-001`** | `CoordinateInversionRoundtrip` | `to_font(to_screen(pt))` roundtrip afwijking $< 10^{-5}$ UPM. |
| **`TC-GRAFIA-002`** | `StylusPressureMapping` | Stylus druk `0.5` schaalt streekdikte exact evenredig met $50\%$. |
| **`TC-GRAFIA-003`** | `StylusTiltAzimuthVector` | Hoek $45^\circ$ genereert een asymmetrische calligrafische ellips-loodlijn. |
| **`TC-GRAFIA-004`** | `SpacebarTemporaryPanOverride` | Spatiebalk schakelt om naar `HandPan`, herstelt basistool bij release. |
| **`TC-GRAFIA-005`** | `CmdTemporarySelectOverride` | `Cmd` in Pen-modus stuurt gestures naar Select zonder de actieve pen-contour te beëindigen. |
| **`TC-GRAFIA-006`** | `OptionClickSmoothToCorner` | `Option`-klik op Smooth knooppunt transformeert naar Corner met vrije BCP's. |
| **`TC-GRAFIA-007`** | `ShiftDragAxisSnap45` | `Shift`-drag projecteert bewegingsvector op $0^\circ, 45^\circ, 90^\circ$ of $135^\circ$. |
| **`TC-GRAFIA-008`** | `CollinearSmoothHandleLock` | Verslepen van BCP-out roteert BCP-in automatisch op de $180^\circ$ tegenoverliggende lijn. |
| **`TC-GRAFIA-009`** | `BrokenHandleOptionDrag` | Verslepen met `Option` verbreekt de collineaire vergrendeling; buurhandle blijft stil. |
| **`TC-GRAFIA-010`** | `ContinuousBaselineStreamOffset` | Positie van glyph $N$ is exact $\sum_{i=0}^{N-1} (\text{width}_i + \text{kern}_{i, i+1})$. |
| **`TC-GRAFIA-011`** | `SidebearingDragAdvanceWidth` | Slepen van RSB met $+25\,\text{dp}$ verhoogt advance width met exact 25 UPM en schuift buren op. |
| **`TC-GRAFIA-012`** | `InteractiveKerningAdjustment` | Wijzigen van kerningpaar `A-V` met $-40$ schuift de `V` en alle opvolgende letters $40\,\text{UPM}$ naar links. |
| **`TC-GRAFIA-013`** | `SnapToBaselineMetrics` | Muis drag binnen $8\,\text{dp}$ van Baseline ($Y=0$) snapt Y-coördinaat exact op $0.0$. |
| **`TC-GRAFIA-014`** | `SnapToAngledGuide` | Gehoekte hulplijn op $12^\circ$ trekt naburig knooppunt loodrecht naar de lijn toe. |
| **`TC-GRAFIA-015`** | `IntegerGridRounding` | Met grid-snap actief worden knooppuntcoördinaten afgerond op gehele getallen (`f64::round()`). |
| **`TC-GRAFIA-016`** | `WindingClockwiseDetection` | Gesloten contour met kloksgewijze volgorde rapporteert `is_clockwise == true`. |
| **`TC-GRAFIA-017`** | `ReverseWindingDirection` | Winding-omkeer actie keert de knooppuntenvolgorde om en inverteert het Shoelace-oppervlak. |
| **`TC-GRAFIA-018`** | `ThumbnailFitBox24x16` | Bounding box van willekeurige bezier-contour wordt geschaald binnen 20×12 dp netto kader. |
| **`TC-GRAFIA-019`** | `CurvatureCombComputation`| $C^2$-gladde kromme genereert continu verlopende kammen; scherpe knik genereert kam-discontinuïteit. |
| **`TC-GRAFIA-020`** | `MeasurementRulerStemIntersection` | Horizontale meetlijn over twee verticale stammen toont de exacte tussenafstand in UPM. |
| **`TC-GRAFIA-021`** | `GhostFilterPreviewOverlay` | Actief filter toont gefilterde curven in preview-stijl zonder het originele pad te overschrijven. |
| **`TC-GRAFIA-022`** | `FilterCancelLeavesModelPure` | Annuleren van filter verwijdert de ghost layer; originele data is bit-identiek ongewijzigd. |
| **`TC-GRAFIA-023`** | `UndoRedo120HzCoalescing` | Continue muis-sleep (120 bewegingen) wordt samengevoegd tot exact 1 atomaire Undo-stap. |
| **`TC-GRAFIA-024`** | `ActiveGlyphSwitchOnDoubleClick`| Dubbelklikken op buurletter `H` in de tekststroom activeert `H` als actieve bewerkings-glyph. |
| **`TC-GRAFIA-025`** | `ZeroAllocationsInIdleLoop` | Tijdens muisbewegingen in toestand `Idle` vinden nul heap-allocaties plaats. |

---

## 🚀 5. Versie-Roadmap (`v0.1.0` t/m `v0.9.0` — Geïsoleerde SemVer Releases)

Om spaghetti te voorkomen en elk deelsysteem volledig geïsoleerd te specificeren, te testen (`cargo test`) en visueel te verifiëren (PNG/GIF), is de ontwikkeling van `grafia` opgedeeld in **9 zelfstandige SemVer-versies** (volledig uitgewerkt in [`ROADMAP-V0.2.0-PREVIEW.md`](./ROADMAP-V0.2.0-PREVIEW.md)):

| Versie | Normatieve Specificatie | Kernfunctionaliteit & Deelsystemen |
| :--- | :--- | :--- |
| **`v0.1.0`** | `SPEC-002` — Single-Glyph Vector Fundament | `SelectTool` (`V`), `PenTool` (`P`), `Hand` (`Space`) en viewport-zoom; exacte segment-klik/drag (`drag_cubic_segment` met `Smooth` raaklijnprojectie); Curve-Healing (`Delete`), Cut-Open (`Shift+Delete`), `OffCurve` delete $\to$ rechte lijn; volledige `GlyphSnapshot` Undo/Redo; 5-pass weergave met metrieken-box, richtingsdriehoek en open-pad crossbars. |
| **`v0.2.0`** | `SPEC-003` — Transformaties, `Lasso`, `Zoom` & Topologische Pad-Operaties | 8-knob Selectie Bounding-Box (`Cmd+Opt+Shift+B`), `RotateTool` (`R`), `ScaleTool` (`S`), `CanvasTool::Lasso` (`Shift+V`), `ZoomTool` (`Z` marquee-zoom), en de 7 pad-operaties (`reverse_selected_contours`, `correct_path_direction` `Cmd+Shift+R`, `make_node_first`, `add_extremes` + inflection points, `tidy_up_paths` `Cmd+Shift+T`, `align_selection` `Cmd+Shift+A`, `connect_selected_open_endpoints`). |
| **`v0.3.0`** | `SPEC-004` — Guides, Magnetische `SnapEngine`, Grid & Light/Dark Mode | Lokale (blauwe) en globale (rode) guides (`Cmd+Shift+L`, *Smart 2-Node* plaatsing, $+90^\circ$ dubbelklik-flip, permanente `Measurement Guide`), magnetische rode snap-lijnen (`Ctrl` = bypass), beige ruit/cirkel badges op metrieklijnen en Alignment Zones, tweedelig canvas-grid (`main` + `subdivision`) en `AppearanceMode` (`Light` / `Dark` / `System`). |
| **`v0.4.0`** | `SPEC-005` — Extra Edit View Tools, Annotaties & Boolean `Remove Overlap` | `KnifeTool` (`E`/`Shift+E` inclusief multi-hit $\ge 4$ snijpunten over meerdere stammen en counters), `PrimitivesTool` (`F`/`Shift+F` Rect & Circle), `MeasurementTool` (`L` / tijdelijk `Cmd+Ctrl+Opt` met blauw/groen/rode coördinaten + `G` naar Guide), `AnnotationTool` (`A` met alle 5 `CanvasAnnotation` typen: `Text`, `Arrow`, `Circle`, `Plus`, `Minus`), `PencilTool` (`B` vrije-hand curve-fit), `PixelTool` (`X`) en kubische Bézier **Boolean `Remove Overlap` (`Cmd+Shift+O`)**. |
| **`v0.5.0`** | `SPEC-006` — Multi-Glyph Text Strip & Interactieve Kerning / Spacing Mode | Doorlopende `CanvasTextStream` (`T` / `Escape`, actieve vs. passieve buurglyphs), direct slepen van LSB/RSB-lijnen en paar-kerning op het canvas (met `SpacingDelta` / `KerningDelta` events voor de Host Font Backend), `Measurement Line` met `Intersection Knobs`, gele (`>0`) en lichtblauwe (`<0`) kerning-highlights, en alle `Ctrl`/`Cmd`/`Opt` + Pijltjes sneltoetsen. |
| **`v0.6.0`** | `SPEC-007` — Preview Strip Engine met Gaussian Blur, Canvas Rulers & Headless `SelectionInfo` API | `PreviewRenderConfig` & `render_preview_strip` (met scheidbare 2-pass 1D Gaussian Blur $\sigma \in [0.0, 20.0]\text{ px}$, `flipped` ondersteboven, `inverted` wit-op-zwart), **Canvas Rulers (`Cmd+R`)** waaruit direct guides op het canvas gesleept worden, en het puur geometrische headless `SelectionInfo` & mutatie-contract voor de 5 externe Info-Box modi (9-punts `TransformOrigin`, `OffCurve` $\Delta X, \Delta Y, L$, `■/□` knoop-telling en `GuideInfo` — zonder font-brede database-lookups). |
| **`v0.7.0`** | `SPEC-008` — Background Layer, Canvas Anchors & Components, Fit Curve & Live Stroke Primitives | Ingebouwde `background_contours` (`Cmd+B` *Edit Background*, `Cmd+J` *Selection to Background*, `Cmd+K` *Swap with Background*), lichtgewicht 2D `CanvasAnchor` ruitpunten + `ghost_paths` weergave en voor-opgeloste `CanvasComponent` shapes (`Cmd+Shift+D` *Decompose*), plus wiskundige `fit_selected_curves` (`Ctrl+Opt+1..8`, $57\%..92\%$) en live contour `stroke_width` / `stroke_height` offset-rendering. |
| **`v0.8.0`** | `SPEC-009` — Curve Harmonization, Tangent Balance Lines, Proportional Servo, Interpolated Nudge, Master Trajectory & Outline Audit | $G^2$ Curve Harmonization op gladde `OnCurve` knopen, interactieve Tangent Balance Lines (proportionele verschuiving richting de raaklijn-driehoek-apex), Proportional Handle Servo Scaling, Interpolated Nudge (`C`-toets toggle & `Ctrl+Alt`), Multi-Master Node Trajectory & Kink Analysis (hoek- en lengteverhouding-afwijkingen), en geautomatiseerde Outline Audit (`OutlineIssue` diagnostiek). |
| **`v0.9.0`** | `SPEC-010` — Non-Destructive Smart Corners, Ink Traps, W3C Stylus Calligraphic Brush, Curvature Heatmap, Stem Gauge, Counter Area & Symmetry | Niet-destructieve parametrische Smart Corners ($R > 0$ convexe fillet, $R < 0$ concave cove) en Ink Traps (`is_ink_trap` wig-inkepingen), druk- en kantelingsgevoelige W3C Stylus Calligraphic Brush, Curvature Heatmap & Comb, Orthogonal Stem Thickness Gauge, Counter Area & Centroid berekening (wet van Green) en live Symmetry Overlay. |
