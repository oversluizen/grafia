//! `SPEC-017 §2.9` & `SPEC-018 §3.3` The 5 Artistic Cubic Bézier Hero Icon Builders for `Icon Studio`:
//! 1. `Wi-Fi Signaal` (`wifi.dot`): Station circle + 3 concentric cubic Bézier radio arcs with `36 UPM Round` stroke.
//! 2. `Microfoon (Knockout-Slash)` (`icon.muteslash`): Capsule microphone head + U-cradle arc + stand + diagonal `MuteSlash` (`22 UPM` halo).
//! 3. `Play-Cirkel (Optisch ⊕)` (`icon.play.optical`): Outer badge circle + asymmetric Play triangle showcasing optical centroid $\Delta\mathbf{c}$.
//! 4. `Satelliet-Baan (MotionPath)` (`icon.orbit.motion`): Tilted cubic Bézier orbit + planet core + satellite bound via `motion_path_contour_id`.
//! 5. `6x Kompas-Rozet & Vlakken` (`icon.rosette` / `icon.shapebuilder`): Smooth cubic Bézier petal & overlapping circles.

use grafia::core::{
    CanvasColorSublayer, ColorLayerPaint, ContourId, ContourStrokeStyle, DrawDirection, FontPoint,
    GlyphContour, LineCapStyle, LineJoinStyle, Node, NodeId, NodeType, SmartCornerSpec,
    StrokePosition, SymbolHierarchyLevel, SymbolLayerTag,
};

const KAPPA: f64 = 0.5522847498;

/// Calm editorial Braun / Apple design palette (`SPEC-018 §3.3`): zero screaming `255` RGB channels.
pub const EDITORIAL_SLATE_BLUE: [u8; 4] = [52, 116, 178, 238];
pub const EDITORIAL_STEEL_CYAN: [u8; 4] = [66, 138, 184, 232];
pub const EDITORIAL_SAGE_TEAL: [u8; 4] = [64, 152, 138, 235];
pub const EDITORIAL_WARM_AMBER: [u8; 4] = [214, 122, 58, 238];
pub const EDITORIAL_CRIMSON_CORAL: [u8; 4] = [198, 68, 82, 242];
pub const EDITORIAL_BADGE_MIST: [u8; 4] = [74, 128, 178, 92];

pub type HeroIconSpec = (
    Vec<GlyphContour>,
    Vec<CanvasColorSublayer>,
    Vec<SymbolLayerTag>,
    Vec<(ContourId, ContourStrokeStyle)>,
);

pub type HeroPlaySpec = (
    Vec<GlyphContour>,
    Vec<CanvasColorSublayer>,
    Vec<SymbolLayerTag>,
    Vec<SmartCornerSpec>,
);

/// Builds a closed 4-segment cubic Bézier ellipse/circle contour centered at `(cx, cy)` with radii `(rx, ry)`.
#[must_use]
pub fn make_ellipse_contour(
    cid: usize,
    base: u64,
    cx: f64,
    cy: f64,
    rx: f64,
    ry: f64,
) -> GlyphContour {
    let (kx, ky) = (rx * KAPPA, ry * KAPPA);
    let sm = NodeType::Smooth;
    let off = NodeType::OffCurve;
    GlyphContour::new(
        ContourId(cid),
        vec![
            Node::new(NodeId(base), cx + rx, cy, sm),
            Node::new(NodeId(base + 1), cx + rx, cy + ky, off),
            Node::new(NodeId(base + 2), cx + kx, cy + ry, off),
            Node::new(NodeId(base + 3), cx, cy + ry, sm),
            Node::new(NodeId(base + 4), cx - kx, cy + ry, off),
            Node::new(NodeId(base + 5), cx - rx, cy + ky, off),
            Node::new(NodeId(base + 6), cx - rx, cy, sm),
            Node::new(NodeId(base + 7), cx - rx, cy - ky, off),
            Node::new(NodeId(base + 8), cx - kx, cy - ry, off),
            Node::new(NodeId(base + 9), cx, cy - ry, sm),
            Node::new(NodeId(base + 10), cx + kx, cy - ry, off),
            Node::new(NodeId(base + 11), cx + rx, cy - ky, off),
        ],
    )
}

/// Builds an open cubic Bézier upward radio arc from `(-dx, y0)` over apex `(cx, y_top)` to `(+dx, y0)`.
#[must_use]
pub fn make_radio_arc_contour(
    cid: usize,
    base: u64,
    cx: f64,
    y0: f64,
    dx: f64,
    y_top: f64,
) -> GlyphContour {
    let mut gc = GlyphContour::new(
        ContourId(cid),
        vec![
            Node::new(NodeId(base), cx - dx, y0, NodeType::Corner),
            Node::new(NodeId(base + 1), cx - dx * 0.55, y_top, NodeType::OffCurve),
            Node::new(NodeId(base + 2), cx + dx * 0.55, y_top, NodeType::OffCurve),
            Node::new(NodeId(base + 3), cx + dx, y0, NodeType::Corner),
        ],
    );
    gc.closed = false;
    gc
}

#[must_use]
pub fn round_stroke(width_upm: f64) -> ContourStrokeStyle {
    ContourStrokeStyle {
        width: width_upm,
        height: width_upm,
        position: StrokePosition::Center,
        cap_start: LineCapStyle::Round,
        cap_end: LineCapStyle::Round,
        join: LineJoinStyle::Round,
    }
}

#[must_use]
pub fn solid_sublayer(id: u64, name: &str, c: GlyphContour, rgba: [u8; 4]) -> CanvasColorSublayer {
    CanvasColorSublayer {
        id,
        name: name.into(),
        contours: vec![c],
        paint: ColorLayerPaint::Solid { rgba },
        opacity: 1.0,
        visible: true,
        locked: false,
    }
}

/// 1. Hero Icon: `Wi-Fi Signaal` (Station dot + 3 concentric cubic Bézier radio arcs).
#[must_use]
pub fn build_wifi_hero_icon() -> HeroIconSpec {
    let dot = make_ellipse_contour(0, 300, 400.0, 195.0, 44.0, 44.0);
    let arc1 = make_radio_arc_contour(1, 320, 400.0, 315.0, 125.0, 435.0);
    let arc2 = make_radio_arc_contour(2, 330, 400.0, 420.0, 215.0, 585.0);
    let arc3 = make_radio_arc_contour(3, 340, 400.0, 525.0, 305.0, 735.0);
    let contours = vec![dot.clone(), arc1.clone(), arc2.clone(), arc3.clone()];
    let sublayers = vec![
        solid_sublayer(1, "StationDot", dot, EDITORIAL_SLATE_BLUE),
        solid_sublayer(2, "WaveInner", arc1, EDITORIAL_STEEL_CYAN),
        solid_sublayer(3, "WaveMid", arc2, EDITORIAL_SAGE_TEAL),
        solid_sublayer(4, "WaveOuter", arc3, EDITORIAL_WARM_AMBER),
    ];
    let mk_tag = |grp, lvl, dir| SymbolLayerTag {
        layer_group: grp,
        hierarchy_level: lvl,
        draw_direction: dir,
        ..Default::default()
    };
    let tags = vec![
        mk_tag(1, SymbolHierarchyLevel::Primary, DrawDirection::CenterOut),
        mk_tag(1, SymbolHierarchyLevel::Primary, DrawDirection::CenterOut),
        mk_tag(2, SymbolHierarchyLevel::Secondary, DrawDirection::CenterOut),
        mk_tag(3, SymbolHierarchyLevel::Tertiary, DrawDirection::CenterOut),
    ];
    let st = round_stroke(40.0);
    let strokes = vec![(ContourId(1), st), (ContourId(2), st), (ContourId(3), st)];
    (contours, sublayers, tags, strokes)
}

/// 2. Hero Icon: `Microfoon (Knockout-Slash)` (Capsule head + U-cradle + stem + diagonal knockout slash).
#[must_use]
pub fn build_microphone_knockout_hero_icon() -> HeroIconSpec {
    let head = make_ellipse_contour(0, 400, 400.0, 495.0, 88.0, 155.0);
    let mut cradle = GlyphContour::new(
        ContourId(1),
        vec![
            Node::new(NodeId(420), 235.0, 470.0, NodeType::Corner),
            Node::new(NodeId(421), 235.0, 235.0, NodeType::OffCurve),
            Node::new(NodeId(422), 565.0, 235.0, NodeType::OffCurve),
            Node::new(NodeId(423), 565.0, 470.0, NodeType::Corner),
        ],
    );
    cradle.closed = false;
    let mut stand = GlyphContour::new(
        ContourId(2),
        vec![
            Node::new(NodeId(430), 400.0, 285.0, NodeType::Corner),
            Node::new(NodeId(431), 400.0, 155.0, NodeType::Corner),
        ],
    );
    stand.closed = false;
    let mut slash = GlyphContour::new(
        ContourId(3),
        vec![
            Node::new(NodeId(440), 195.0, 645.0, NodeType::Corner),
            Node::new(NodeId(441), 605.0, 195.0, NodeType::Corner),
        ],
    );
    slash.closed = false;
    let contours = vec![head.clone(), cradle.clone(), stand.clone(), slash.clone()];
    let sublayers = vec![
        solid_sublayer(1, "MicCapsule", head, EDITORIAL_SLATE_BLUE),
        solid_sublayer(2, "MicCradle", cradle, EDITORIAL_SAGE_TEAL),
        solid_sublayer(3, "MicStand", stand, EDITORIAL_SAGE_TEAL),
        solid_sublayer(4, "MuteSlash", slash, EDITORIAL_CRIMSON_CORAL),
    ];
    let sec = SymbolLayerTag {
        layer_group: 1,
        hierarchy_level: SymbolHierarchyLevel::Secondary,
        ..Default::default()
    };
    let tags = vec![
        SymbolLayerTag {
            layer_group: 1,
            hierarchy_level: SymbolHierarchyLevel::Primary,
            ..Default::default()
        },
        sec,
        sec,
        SymbolLayerTag {
            layer_group: 2,
            hierarchy_level: SymbolHierarchyLevel::Primary,
            draw_direction: DrawDirection::StartToEnd,
            knockout_margin_upm: 22.0,
            ..Default::default()
        },
    ];
    let strokes = vec![
        (ContourId(1), round_stroke(34.0)),
        (ContourId(2), round_stroke(34.0)),
        (ContourId(3), round_stroke(38.0)),
    ];
    (contours, sublayers, tags, strokes)
}

/// 3. Hero Icon: `Play-Cirkel (Optisch ⊕)` (Ring + asymmetric Play triangle with visible optical centroid $\Delta\mathbf{c}$).
#[must_use]
pub fn build_play_optical_hero_icon() -> HeroPlaySpec {
    let ring = make_ellipse_contour(0, 500, 385.0, 400.0, 250.0, 250.0);
    let tri = GlyphContour::new(
        ContourId(1),
        vec![
            Node::new(NodeId(520), 285.0, 255.0, NodeType::Corner),
            Node::new(NodeId(521), 545.0, 400.0, NodeType::Corner),
            Node::new(NodeId(522), 285.0, 545.0, NodeType::Corner),
        ],
    );
    let contours = vec![ring.clone(), tri.clone()];
    let sublayers = vec![
        solid_sublayer(1, "BadgeCircle", ring, EDITORIAL_BADGE_MIST),
        solid_sublayer(2, "PlayTriangle", tri, EDITORIAL_WARM_AMBER),
    ];
    let tags = vec![
        SymbolLayerTag {
            layer_group: 1,
            hierarchy_level: SymbolHierarchyLevel::Tertiary,
            ..Default::default()
        },
        SymbolLayerTag {
            layer_group: 2,
            hierarchy_level: SymbolHierarchyLevel::Primary,
            ..Default::default()
        },
    ];
    let corners = vec![
        SmartCornerSpec {
            node_id: NodeId(520),
            radius_upm: 18.0,
        },
        SmartCornerSpec {
            node_id: NodeId(521),
            radius_upm: 18.0,
        },
        SmartCornerSpec {
            node_id: NodeId(522),
            radius_upm: 18.0,
        },
    ];
    (contours, sublayers, tags, corners)
}

/// 4. Hero Icon: `Satelliet-Baan (MotionPath)` (Cubic orbit path + planet core + satellite following orbit).
#[must_use]
pub fn build_orbit_motion_path_hero_icon() -> HeroIconSpec {
    let orbit = make_ellipse_contour(0, 600, 400.0, 400.0, 265.0, 165.0);
    let core = make_ellipse_contour(1, 620, 400.0, 400.0, 78.0, 78.0);
    let sat = GlyphContour::new(
        ContourId(2),
        vec![
            Node::new(NodeId(640), 355.0, 372.0, NodeType::Corner),
            Node::new(NodeId(641), 455.0, 400.0, NodeType::Corner),
            Node::new(NodeId(642), 355.0, 428.0, NodeType::Corner),
        ],
    );
    let contours = vec![orbit.clone(), core.clone(), sat.clone()];
    let sublayers = vec![
        solid_sublayer(1, "OrbitTrack", orbit, EDITORIAL_SAGE_TEAL),
        solid_sublayer(2, "PlanetCore", core, EDITORIAL_SLATE_BLUE),
        solid_sublayer(3, "Satellite", sat, EDITORIAL_WARM_AMBER),
    ];
    let tags = vec![
        SymbolLayerTag {
            layer_group: 1,
            hierarchy_level: SymbolHierarchyLevel::Tertiary,
            ..Default::default()
        },
        SymbolLayerTag {
            layer_group: 1,
            hierarchy_level: SymbolHierarchyLevel::Secondary,
            ..Default::default()
        },
        SymbolLayerTag {
            layer_group: 2,
            hierarchy_level: SymbolHierarchyLevel::Primary,
            custom_pivot_upm: Some(FontPoint::new(400.0, 400.0)),
            motion_path_contour_id: Some(ContourId(0)),
            orient_along_motion_path: true,
            ..Default::default()
        },
    ];
    let strokes = vec![(ContourId(0), round_stroke(20.0))];
    (contours, sublayers, tags, strokes)
}

/// 5. Smooth cubic Bézier tear-drop petal for the `6x Kompas-Rozet` hero icon.
#[must_use]
pub fn build_compass_rosette_petal() -> GlyphContour {
    GlyphContour::new(
        ContourId(0),
        vec![
            Node::new(NodeId(200), 400.0, 440.0, NodeType::Corner),
            Node::new(NodeId(201), 495.0, 520.0, NodeType::OffCurve),
            Node::new(NodeId(202), 470.0, 665.0, NodeType::OffCurve),
            Node::new(NodeId(203), 400.0, 745.0, NodeType::Corner),
            Node::new(NodeId(204), 330.0, 665.0, NodeType::OffCurve),
            Node::new(NodeId(205), 305.0, 520.0, NodeType::OffCurve),
        ],
    )
}
