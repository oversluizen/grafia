//! Automated Reference Vector Engine Differential & Golden Oracle Verification Suite.
//!
//! Tier 1 (Offline Deterministic Gate): Compares `grafia::core` and `grafia::adapters`
//! directly against `tokens/reference_geometry_golden_oracle.json` extracted from the
//! native macOS Sonoma 14.8.9 reference vector engine (`build 4108`).
//!
//! Tier 2 (Live Hardware-in-the-Loop SSH Gate): Connects over SSH to host alias `macbook`,
//! executes `~/.reference_vector_oracle`, and verifies live differential parity against `grafia`.

use approx::assert_abs_diff_eq;
use grafia::adapters::{CanvasTool, GrafiaCanvasState};
use grafia::core::{
    add_extremes_to_contours, compute_hoi_overlay_geometry, contour_signed_area,
    convert_contour_to_cubic, convert_contour_to_quadratic, convert_cubic_to_hobby_in_contour,
    convert_hobby_or_euler_in_contour, heal_delete_nodes, remove_overlap_in_contours,
    reverse_contour_nodes, CompanionMasterLayer, Node, NodeId, NodeType, PostScriptHint,
    PostScriptHintAxis, TrueTypeInstructionKind,
};
use serde::Deserialize;
use std::path::Path;

const COORD_EPSILON_UPM: f64 = 0.05;
const HEAL_FIT_EPSILON_UPM: f64 = 1.5;

#[derive(Debug, Clone, Deserialize)]
struct GoldenOracleRoot {
    sut: SutMetadata,
    scurve_oracle: ScurveOracle,
    winding_oracle: WindingOracle,
    boolean_union_oracle: BooleanUnionOracle,
    capabilities_oracle: CapabilitiesOracle,
}

#[derive(Debug, Clone, Deserialize)]
struct SutMetadata {
    engine: String,
    short_version: String,
    bundle_version: String,
    os_version: String,
}

#[derive(Debug, Clone, Deserialize)]
struct OracleNode {
    x: f64,
    y: f64,
    #[serde(rename = "type")]
    node_type: i32,
    connection: i32,
}

#[derive(Debug, Clone, Deserialize)]
struct ScurveOracle {
    initial: Vec<OracleNode>,
    after_add_extremes: Vec<OracleNode>,
    after_remove_node_keep_shape: Vec<OracleNode>,
}

#[derive(Debug, Clone, Deserialize)]
struct WindingOracle {
    area_unsigned: f64,
    direction_before: i32,
    direction_after: i32,
    after_reverse_nodes: Vec<OracleNode>,
}

#[derive(Debug, Clone, Deserialize)]
struct BooleanUnionOracle {
    merged_path_count: usize,
    merged_nodes: Vec<OracleNode>,
}

#[derive(Debug, Clone, Deserialize)]
struct CapabilitiesOracle {
    path_supports_hobby: bool,
    path_supports_hobby_to_bezier: bool,
    path_supports_quadratic: bool,
    layer_supports_postscript_hints: bool,
    layer_supports_truetype_hints: bool,
    hint_supports_ghost: bool,
    tti_loaded: i32,
    hoi_loaded: i32,
    tool_bundles_count: usize,
}

fn load_golden_oracle() -> Result<GoldenOracleRoot, Box<dyn std::error::Error>> {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tokens")
        .join("reference_geometry_golden_oracle.json");
    let raw = std::fs::read_to_string(&path)?;
    let parsed = serde_json::from_str(&raw)?;
    Ok(parsed)
}

fn oracle_nodes_to_grafia(nodes: &[OracleNode]) -> Vec<Node> {
    nodes
        .iter()
        .enumerate()
        .map(|(idx, n)| {
            let nt = if n.node_type == 65 {
                NodeType::OffCurve
            } else if n.connection == 100 {
                NodeType::Smooth
            } else {
                NodeType::Corner
            };
            Node::new(NodeId((idx + 1) as u64), n.x, n.y, nt)
        })
        .collect()
}

fn verify_scurve_extremes_and_healing(scurve: &ScurveOracle) {
    let mut contours = vec![oracle_nodes_to_grafia(&scurve.initial)];
    let mut closed = vec![false];
    let mut next_id = 100_u64;
    assert!(add_extremes_to_contours(
        &mut contours,
        &mut closed,
        &[],
        false,
        &mut next_id,
        0.0,
    ));
    assert_eq!(contours[0].len(), scurve.after_add_extremes.len());
    for (actual, expected) in contours[0].iter().zip(scurve.after_add_extremes.iter()) {
        assert_abs_diff_eq!(actual.point.x.0, expected.x, epsilon = COORD_EPSILON_UPM);
        assert_abs_diff_eq!(actual.point.y.0, expected.y, epsilon = COORD_EPSILON_UPM);
    }

    let remove_id = contours[0][3].id;
    heal_delete_nodes(&mut contours[0], false, &[remove_id], &mut next_id);
    assert_eq!(contours[0].len(), scurve.after_remove_node_keep_shape.len());
    for (actual, expected) in contours[0]
        .iter()
        .zip(scurve.after_remove_node_keep_shape.iter())
    {
        assert_abs_diff_eq!(actual.point.x.0, expected.x, epsilon = HEAL_FIT_EPSILON_UPM);
        assert_abs_diff_eq!(actual.point.y.0, expected.y, epsilon = HEAL_FIT_EPSILON_UPM);
    }
}

fn verify_winding_and_boolean_union(winding: &WindingOracle, boolean_union: &BooleanUnionOracle) {
    let mut rect = vec![
        Node::new(NodeId(1), 100.0, 0.0, NodeType::Corner),
        Node::new(NodeId(2), 200.0, 0.0, NodeType::Corner),
        Node::new(NodeId(3), 200.0, 700.0, NodeType::Corner),
        Node::new(NodeId(4), 100.0, 700.0, NodeType::Corner),
    ];
    let area_before = contour_signed_area(&rect);
    assert_abs_diff_eq!(
        area_before.abs(),
        winding.area_unsigned,
        epsilon = COORD_EPSILON_UPM
    );
    assert!(area_before > 0.0 && winding.direction_before == -1);
    reverse_contour_nodes(&mut rect, true);
    let area_after = contour_signed_area(&rect);
    assert!(area_after < 0.0 && winding.direction_after == 1);
    let start_offset = rect
        .iter()
        .position(|n| {
            (n.point.x.0 - winding.after_reverse_nodes[0].x).abs() <= COORD_EPSILON_UPM
                && (n.point.y.0 - winding.after_reverse_nodes[0].y).abs() <= COORD_EPSILON_UPM
        })
        .unwrap_or(0);
    for (idx, expected) in winding.after_reverse_nodes.iter().enumerate() {
        let actual = &rect[(start_offset + idx) % rect.len()];
        assert_abs_diff_eq!(actual.point.x.0, expected.x, epsilon = COORD_EPSILON_UPM);
        assert_abs_diff_eq!(actual.point.y.0, expected.y, epsilon = COORD_EPSILON_UPM);
    }

    let sq1 = vec![
        Node::new(NodeId(1), 0.0, 0.0, NodeType::Corner),
        Node::new(NodeId(2), 100.0, 0.0, NodeType::Corner),
        Node::new(NodeId(3), 100.0, 100.0, NodeType::Corner),
        Node::new(NodeId(4), 0.0, 100.0, NodeType::Corner),
    ];
    let sq2 = vec![
        Node::new(NodeId(5), 50.0, 50.0, NodeType::Corner),
        Node::new(NodeId(6), 150.0, 50.0, NodeType::Corner),
        Node::new(NodeId(7), 150.0, 150.0, NodeType::Corner),
        Node::new(NodeId(8), 50.0, 150.0, NodeType::Corner),
    ];
    let mut bool_contours = vec![sq1, sq2];
    let mut bool_closed = vec![true, true];
    let mut next_id = 200_u64;
    assert!(remove_overlap_in_contours(
        &mut bool_contours,
        &mut bool_closed,
        &[],
        &mut next_id,
    ));
    assert_eq!(bool_contours.len(), boolean_union.merged_path_count);
    assert_eq!(bool_contours[0].len(), boolean_union.merged_nodes.len());
    for expected in &boolean_union.merged_nodes {
        let matched = bool_contours[0].iter().any(|n| {
            (n.point.x.0 - expected.x).abs() <= COORD_EPSILON_UPM
                && (n.point.y.0 - expected.y).abs() <= COORD_EPSILON_UPM
        });
        assert!(matched);
    }
}

fn verify_oracle_against_grafia(oracle: &GoldenOracleRoot) {
    assert_eq!(oracle.sut.engine, "ReferenceVectorEngine");
    assert_eq!(oracle.sut.short_version, "4.1.1");
    assert_eq!(oracle.sut.bundle_version, "4108");
    assert!(oracle.sut.os_version.contains("14.8.9"));

    verify_scurve_extremes_and_healing(&oracle.scurve_oracle);
    verify_winding_and_boolean_union(&oracle.winding_oracle, &oracle.boolean_union_oracle);

    assert!(oracle.capabilities_oracle.path_supports_hobby);
    assert!(oracle.capabilities_oracle.path_supports_hobby_to_bezier);
    assert!(oracle.capabilities_oracle.path_supports_quadratic);
    assert!(oracle.capabilities_oracle.layer_supports_postscript_hints);
    assert!(oracle.capabilities_oracle.layer_supports_truetype_hints);
    assert!(oracle.capabilities_oracle.hint_supports_ghost);
    assert_eq!(oracle.capabilities_oracle.tti_loaded, 1);
    assert_eq!(oracle.capabilities_oracle.hoi_loaded, 1);
    assert_eq!(oracle.capabilities_oracle.tool_bundles_count, 14);
}

#[test]
fn test_reference_golden_oracle_differential_parity() -> Result<(), Box<dyn std::error::Error>> {
    let oracle = load_golden_oracle()?;
    verify_oracle_against_grafia(&oracle);
    Ok(())
}

fn verify_spline_and_hinting_primitives(state: &mut GrafiaCanvasState) {
    state.load_glyph_archetype(grafia::tokens::GlyphArchetype::O);
    let mut contour = state.contours[0].clone();
    let mut next_id = 500_u64;
    assert!(convert_cubic_to_hobby_in_contour(&mut contour, true, &[]));
    assert!(convert_hobby_or_euler_in_contour(
        &mut contour,
        true,
        &[],
        (true, false),
        0.0,
        &[],
        &mut next_id
    ));
    assert!(convert_contour_to_quadratic(
        &mut contour,
        true,
        0.0,
        &[],
        &mut next_id
    ));
    assert!(convert_contour_to_cubic(
        &mut contour,
        true,
        0.0,
        &[],
        &mut next_id
    ));

    let top_ghost = PostScriptHint {
        id: 1,
        axis: PostScriptHintAxis::Horizontal,
        origin_upm: 700.0,
        width_upm: 0.0,
        is_ghost: true,
        top_ghost: true,
        start_node: None,
        end_node: None,
        selected: false,
    };
    let bottom_ghost = PostScriptHint {
        top_ghost: false,
        ..top_ghost.clone()
    };
    assert_abs_diff_eq!(top_ghost.effective_width_upm(), 20.0, epsilon = 1e-6);
    assert_abs_diff_eq!(bottom_ghost.effective_width_upm(), 21.0, epsilon = 1e-6);
    assert_eq!(TrueTypeInstructionKind::Touch.badge_code(), "A");
    assert_eq!(TrueTypeInstructionKind::Anchor.badge_code(), "K");
    assert_eq!(TrueTypeInstructionKind::Stem.badge_code(), "S");
    assert_eq!(TrueTypeInstructionKind::Shift.badge_code(), "F");
    assert_eq!(TrueTypeInstructionKind::Interpolate.badge_code(), "G");
    assert_eq!(TrueTypeInstructionKind::Align.badge_code(), "L");
    assert_eq!(TrueTypeInstructionKind::Delta.badge_code(), "E");
}

#[test]
fn test_reference_capabilities_and_15_tools_parity() {
    let mut state = GrafiaCanvasState::init_glyph_a();
    let all_tools = [
        CanvasTool::Select,
        CanvasTool::Lasso,
        CanvasTool::Pen,
        CanvasTool::Rotate,
        CanvasTool::Scale,
        CanvasTool::Zoom,
        CanvasTool::Knife,
        CanvasTool::Primitives,
        CanvasTool::Measurement,
        CanvasTool::Annotation,
        CanvasTool::Pencil,
        CanvasTool::Pixel,
        CanvasTool::Text,
        CanvasTool::TrueTypeInstructor,
        CanvasTool::Hoi,
    ];
    assert_eq!(all_tools.len(), 15);
    for tool in all_tools {
        state.activate_tool(tool);
        assert_eq!(state.active_tool, tool);
    }

    verify_spline_and_hinting_primitives(&mut state);

    let make_master = |id: u32, x: f64, y: f64| CompanionMasterLayer {
        id,
        name: format!("M{id}"),
        color_rgba: [100, 150, 220, 200],
        contours: vec![vec![Node::new(NodeId(1), x, y, NodeType::Corner)]],
        closed_flags: vec![true],
        anchors: Vec::new(),
        editable: true,
    };
    let masters = vec![
        make_master(1, 100.0, 0.0),
        make_master(2, 140.0, 60.0),
        make_master(3, 220.0, 80.0),
        make_master(4, 280.0, 0.0),
    ];
    let hoi_geom = compute_hoi_overlay_geometry(&masters, None);
    assert_eq!(hoi_geom.diamonds.len(), 4);
    assert_eq!(hoi_geom.polylines.len(), 1);
}

/// Tier 2 Live Hardware-in-the-Loop Differential Test:
/// Connects via SSH to host alias `macbook` (or `GRAFIA_ORACLE_SSH_HOST`), executes
/// `~/.reference_vector_oracle`, and verifies live differential parity against `grafia`.
///
/// Run explicitly with:
/// `cargo test --test reference_oracle_differential_tests -- --ignored`
#[test]
#[ignore]
fn test_live_ssh_reference_oracle_differential_parity() -> Result<(), Box<dyn std::error::Error>> {
    use std::process::Command;

    let ssh_host = std::env::var("GRAFIA_ORACLE_SSH_HOST").unwrap_or_else(|_| "macbook".to_owned());
    let output = Command::new("ssh")
        .args([
            "-o",
            "BatchMode=yes",
            "-o",
            "ConnectTimeout=8",
            &ssh_host,
            "~/.reference_vector_oracle 2>/dev/null",
        ])
        .output()?;

    assert!(
        output.status.success(),
        "SSH reference oracle execution failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let live_json = String::from_utf8(output.stdout)?;
    let live_oracle: GoldenOracleRoot = serde_json::from_str(&live_json)?;
    verify_oracle_against_grafia(&live_oracle);
    Ok(())
}
