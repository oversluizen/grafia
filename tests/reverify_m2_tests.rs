//! Empirical Re-Verification Test Suite for Milestone 2 Remediations.
//!
//! Empirically challenges:
//! 1. Marquee 2D isolation: apex of /A selection across all 4 drag diagonal directions,
//!    crossbar isolation, inner counter apex isolation, and disjoint bounding boxes.
//! 2. PenTool mouse hover after single click without button press: zero handle extrusion,
//!    zero node mutation, and rigorous contour closing state machine.
//! 3. Shape thumbnail card and silhouette curve rendering pass output.

use egui::{Context, Event, Modifiers as EguiModifiers, PointerButton, Pos2, RawInput};
use grafia::adapters::{
    CanvasTool, CanvasViewMode, GrafiaCanvasResponse, GrafiaCanvasState, GrafiaCanvasWidget,
};
use grafia::core::{FontPoint, NodeType};
use grafia::interaction::PenToolState;
use grafia::tokens::GrafiaTheme;

fn run_canvas_frame(
    ctx: &Context,
    state: &mut GrafiaCanvasState,
    theme: &GrafiaTheme,
    raw_input: RawInput,
) -> GrafiaCanvasResponse {
    let mut resp = None;
    let mut full_output = ctx.run_ui(raw_input, |ui| {
        resp = Some(GrafiaCanvasWidget::new(state, theme).show(ui));
    });
    full_output.textures_delta.clear();
    resp.expect("GrafiaCanvasWidget must return a response")
}

#[test]
fn test_empirical_marquee_2d_isolation_all_directions_and_subsets() {
    let ctx = Context::default();
    let theme = GrafiaTheme::sonoma_default();

    // The apex nodes of glyph /A in outer contour:
    // Apex 1: (300.0, 700.0) -> node index 1
    // Apex 2: (400.0, 700.0) -> node index 2
    // Lower nodes sharing horizontal X range [200..500]:
    // Crossbar: (470.0, 180.0), (230.0, 180.0)
    // Inner counter: (350.0, 520.0), (260.0, 260.0), (440.0, 260.0)
    // Base: (160.0, 0.0), (540.0, 0.0)

    // Test marquee dragging across all 4 diagonal drag orientations
    let test_diagonals = [
        // (start, end) in font coordinates
        (FontPoint::new(200.0, 750.0), FontPoint::new(500.0, 650.0)), // top-left to bottom-right
        (FontPoint::new(500.0, 650.0), FontPoint::new(200.0, 750.0)), // bottom-right to top-left
        (FontPoint::new(500.0, 750.0), FontPoint::new(200.0, 650.0)), // top-right to bottom-left
        (FontPoint::new(200.0, 650.0), FontPoint::new(500.0, 750.0)), // bottom-left to top-right
    ];

    for (dir_idx, (f_start, f_end)) in test_diagonals.iter().enumerate() {
        let mut state = GrafiaCanvasState::init_glyph_a();
        let apex1_id = state.contours[0][1].id;
        let apex2_id = state.contours[0][2].id;
        let crossbar1_id = state.contours[0][5].id;
        let crossbar2_id = state.contours[0][6].id;
        let inner_apex_id = state.contours[1][2].id;

        let scr_start = state.viewport.to_screen(*f_start);
        let scr_end = state.viewport.to_screen(*f_end);
        let pos_start = Pos2::new(scr_start.x.0, scr_start.y.0);
        let pos_end = Pos2::new(scr_end.x.0, scr_end.y.0);

        // 1. Mouse down
        let mut input = RawInput::default();
        input.events.push(Event::PointerButton {
            pos: pos_start,
            button: PointerButton::Primary,
            pressed: true,
            modifiers: EguiModifiers::NONE,
        });
        let _ = run_canvas_frame(&ctx, &mut state, &theme, input);

        // 2. Mouse move
        let mut input = RawInput::default();
        input.events.push(Event::PointerMoved(pos_end));
        let _ = run_canvas_frame(&ctx, &mut state, &theme, input);

        // 3. Mouse release
        let mut input = RawInput::default();
        input.events.push(Event::PointerButton {
            pos: pos_end,
            button: PointerButton::Primary,
            pressed: false,
            modifiers: EguiModifiers::NONE,
        });
        let _ = run_canvas_frame(&ctx, &mut state, &theme, input);

        assert_eq!(
            state.selected_nodes.len(),
            2,
            "Direction {dir_idx}: Marquee must select EXACTLY 2 apex nodes, got {:?}",
            state.selected_nodes
        );
        assert!(
            state.selected_nodes.contains(&apex1_id),
            "Direction {dir_idx}: Apex 1 (300, 700) must be selected"
        );
        assert!(
            state.selected_nodes.contains(&apex2_id),
            "Direction {dir_idx}: Apex 2 (400, 700) must be selected"
        );
        assert!(
            !state.selected_nodes.contains(&crossbar1_id),
            "Direction {dir_idx}: Crossbar 1 (470, 180) must NOT be selected"
        );
        assert!(
            !state.selected_nodes.contains(&crossbar2_id),
            "Direction {dir_idx}: Crossbar 2 (230, 180) must NOT be selected"
        );
        assert!(
            !state.selected_nodes.contains(&inner_apex_id),
            "Direction {dir_idx}: Inner counter apex (350, 520) must NOT be selected"
        );
    }

    // Isolated test: Marquee targeting ONLY the crossbar [Font X 200..500, Font Y 150..200]
    {
        let mut state = GrafiaCanvasState::init_glyph_a();
        let apex1_id = state.contours[0][1].id;
        let apex2_id = state.contours[0][2].id;
        let crossbar1_id = state.contours[0][5].id;
        let crossbar2_id = state.contours[0][6].id;

        let p_start = state.viewport.to_screen(FontPoint::new(200.0, 200.0));
        let p_end = state.viewport.to_screen(FontPoint::new(500.0, 150.0));

        let mut input = RawInput::default();
        input.events.push(Event::PointerButton {
            pos: Pos2::new(p_start.x.0, p_start.y.0),
            button: PointerButton::Primary,
            pressed: true,
            modifiers: EguiModifiers::NONE,
        });
        let _ = run_canvas_frame(&ctx, &mut state, &theme, input);

        let mut input = RawInput::default();
        input
            .events
            .push(Event::PointerMoved(Pos2::new(p_end.x.0, p_end.y.0)));
        let _ = run_canvas_frame(&ctx, &mut state, &theme, input);

        let mut input = RawInput::default();
        input.events.push(Event::PointerButton {
            pos: Pos2::new(p_end.x.0, p_end.y.0),
            button: PointerButton::Primary,
            pressed: false,
            modifiers: EguiModifiers::NONE,
        });
        let _ = run_canvas_frame(&ctx, &mut state, &theme, input);

        assert_eq!(
            state.selected_nodes.len(),
            2,
            "Crossbar marquee must select exactly 2 crossbar nodes, got {:?}",
            state.selected_nodes
        );
        assert!(state.selected_nodes.contains(&crossbar1_id));
        assert!(state.selected_nodes.contains(&crossbar2_id));
        assert!(
            !state.selected_nodes.contains(&apex1_id),
            "Apex nodes must NOT be selected by crossbar marquee"
        );
        assert!(
            !state.selected_nodes.contains(&apex2_id),
            "Apex nodes must NOT be selected by crossbar marquee"
        );
    }

    // Isolated test: Marquee targeting ONLY inner triangle apex [Font X 330..370, Font Y 500..540]
    {
        let mut state = GrafiaCanvasState::init_glyph_a();
        let apex1_id = state.contours[0][1].id;
        let inner_apex_id = state.contours[1][2].id;

        let p_start = state.viewport.to_screen(FontPoint::new(330.0, 540.0));
        let p_end = state.viewport.to_screen(FontPoint::new(370.0, 500.0));

        let mut input = RawInput::default();
        input.events.push(Event::PointerButton {
            pos: Pos2::new(p_start.x.0, p_start.y.0),
            button: PointerButton::Primary,
            pressed: true,
            modifiers: EguiModifiers::NONE,
        });
        let _ = run_canvas_frame(&ctx, &mut state, &theme, input);

        let mut input = RawInput::default();
        input
            .events
            .push(Event::PointerMoved(Pos2::new(p_end.x.0, p_end.y.0)));
        let _ = run_canvas_frame(&ctx, &mut state, &theme, input);

        let mut input = RawInput::default();
        input.events.push(Event::PointerButton {
            pos: Pos2::new(p_end.x.0, p_end.y.0),
            button: PointerButton::Primary,
            pressed: false,
            modifiers: EguiModifiers::NONE,
        });
        let _ = run_canvas_frame(&ctx, &mut state, &theme, input);

        assert_eq!(
            state.selected_nodes.len(),
            1,
            "Inner apex marquee must select exactly 1 node (inner apex), got {:?}",
            state.selected_nodes
        );
        assert!(state.selected_nodes.contains(&inner_apex_id));
        assert!(!state.selected_nodes.contains(&apex1_id));
    }
}

#[test]
fn test_empirical_pentool_click_hover_no_mutation_and_closing_state_machine() {
    let ctx = Context::default();
    let theme = GrafiaTheme::sonoma_default();
    let mut state = GrafiaCanvasState::new(theme.clone());
    state.active_tool = CanvasTool::Pen;

    let click_pos = Pos2::new(250.0, 250.0);

    // 1. Single click to place anchor
    let mut input = RawInput::default();
    input.events.push(Event::PointerButton {
        pos: click_pos,
        button: PointerButton::Primary,
        pressed: true,
        modifiers: EguiModifiers::NONE,
    });
    let _ = run_canvas_frame(&ctx, &mut state, &theme, input);

    assert_eq!(state.contours.len(), 1);
    assert_eq!(state.contours[0].len(), 1);
    let anchor_id = state.contours[0][0].id;
    assert_eq!(state.contours[0][0].node_type, NodeType::Corner);
    assert!(matches!(
        state.pen_state,
        PenToolState::PlacingAnchor { .. }
    ));

    // 2. Button release without drag
    let mut input = RawInput::default();
    input.events.push(Event::PointerButton {
        pos: click_pos,
        button: PointerButton::Primary,
        pressed: false,
        modifiers: EguiModifiers::NONE,
    });
    let _ = run_canvas_frame(&ctx, &mut state, &theme, input);

    assert_eq!(
        state.pen_state,
        PenToolState::Idle,
        "Mouse release must reset pen_state to Idle"
    );

    // 3. Multi-frame mouse hover with varying coordinates, NO buttons pressed
    let hover_positions = [
        Pos2::new(260.0, 270.0),
        Pos2::new(300.0, 320.0),
        Pos2::new(400.0, 150.0),
        Pos2::new(200.0, 200.0),
    ];

    for (h_idx, &h_pos) in hover_positions.iter().enumerate() {
        let mut input = RawInput::default();
        input.events.push(Event::PointerMoved(h_pos));
        let _ = run_canvas_frame(&ctx, &mut state, &theme, input);

        assert_eq!(
            state.pen_state,
            PenToolState::Idle,
            "Hover step {h_idx}: pen_state must stay Idle when mouse button is not held"
        );
        let node = state.find_node(anchor_id).unwrap();
        assert_eq!(
            node.node_type,
            NodeType::Corner,
            "Hover step {h_idx}: node must remain Corner and never be mutated to Smooth on unpressed hover"
        );
        assert_eq!(state.contours[0].len(), 1);
    }

    // 4. Contour closing verification:
    // Place node 2 and node 3 to form a triangle
    let pt2 = Pos2::new(400.0, 250.0);
    let pt3 = Pos2::new(400.0, 400.0);

    for pt in [pt2, pt3] {
        let mut input = RawInput::default();
        input.events.push(Event::PointerButton {
            pos: pt,
            button: PointerButton::Primary,
            pressed: true,
            modifiers: EguiModifiers::NONE,
        });
        input.events.push(Event::PointerButton {
            pos: pt,
            button: PointerButton::Primary,
            pressed: false,
            modifiers: EguiModifiers::NONE,
        });
        let _ = run_canvas_frame(&ctx, &mut state, &theme, input);
    }

    assert_eq!(
        state.contours[0].len(),
        3,
        "Should have 3 nodes before closing"
    );
    assert!(!state.active_contour_closed);

    // Click on node 1 (at click_pos 250, 250) to CLOSE contour
    let mut input = RawInput::default();
    input.events.push(Event::PointerButton {
        pos: click_pos,
        button: PointerButton::Primary,
        pressed: true,
        modifiers: EguiModifiers::NONE,
    });
    input.events.push(Event::PointerButton {
        pos: click_pos,
        button: PointerButton::Primary,
        pressed: false,
        modifiers: EguiModifiers::NONE,
    });
    let _ = run_canvas_frame(&ctx, &mut state, &theme, input);

    assert_eq!(
        state.contours[0].len(),
        3,
        "Closing contour must NOT append a duplicate 4th node"
    );
    assert!(
        state.active_contour_closed,
        "active_contour_closed flag must be true after clicking first anchor"
    );

    // 5. Subsequent click at a new position (500, 500) must start a NEW contour
    let pt_new = Pos2::new(500.0, 500.0);
    let mut input = RawInput::default();
    input.events.push(Event::PointerButton {
        pos: pt_new,
        button: PointerButton::Primary,
        pressed: true,
        modifiers: EguiModifiers::NONE,
    });
    input.events.push(Event::PointerButton {
        pos: pt_new,
        button: PointerButton::Primary,
        pressed: false,
        modifiers: EguiModifiers::NONE,
    });
    let _ = run_canvas_frame(&ctx, &mut state, &theme, input);

    assert_eq!(
        state.contours.len(),
        2,
        "Clicking after closing contour must start a new separate contour"
    );
    assert_eq!(state.contours[0].len(), 3);
    assert_eq!(state.contours[1].len(), 1);
    assert!(!state.active_contour_closed);
}

#[test]
fn test_empirical_thumbnail_card_and_silhouette_rendering_pass() {
    let ctx = Context::default();
    let theme = GrafiaTheme::sonoma_default();
    let mut state = GrafiaCanvasState::init_glyph_a();

    // 1. ShapeThumbnail mode: Verify thumbnail rendering generates shapes into painter
    state.view_mode = CanvasViewMode::ShapeThumbnail;
    let mut shape_count = 0;
    let mut has_rect = false;
    let mut has_path = false;

    let mut full_output = ctx.run_ui(RawInput::default(), |ui| {
        let resp = GrafiaCanvasWidget::new(&mut state, &theme).show(ui);
        assert_eq!(resp.view_mode, CanvasViewMode::ShapeThumbnail);
    });

    for clipped_shape in &full_output.shapes {
        match &clipped_shape.shape {
            egui::Shape::Rect(_) => {
                has_rect = true;
                shape_count += 1;
            }
            egui::Shape::Path(_) => {
                has_path = true;
                shape_count += 1;
            }
            _ => {
                shape_count += 1;
            }
        }
    }
    full_output.textures_delta.clear();

    assert!(
        has_rect,
        "ShapeThumbnail view mode must emit rect shapes for thumbnail cards"
    );
    assert!(
        has_path,
        "ShapeThumbnail view mode must emit path shapes for fitted silhouettes"
    );
    assert!(
        shape_count >= 4,
        "ShapeThumbnail mode must render cards and silhouettes for both contours of glyph A"
    );

    // 2. PreviewFilled mode: Verify silhouette fill rendering
    state.view_mode = CanvasViewMode::PreviewFilled;
    let mut full_output = ctx.run_ui(RawInput::default(), |ui| {
        let resp = GrafiaCanvasWidget::new(&mut state, &theme).show(ui);
        assert_eq!(resp.view_mode, CanvasViewMode::PreviewFilled);
    });

    let mut preview_mesh_triangles = 0;
    for clipped_shape in &full_output.shapes {
        if let egui::Shape::Mesh(mesh) = &clipped_shape.shape {
            preview_mesh_triangles += mesh.indices.len() / 3;
        }
    }
    full_output.textures_delta.clear();

    assert!(
        preview_mesh_triangles >= 10,
        "PreviewFilled mode must emit a tessellated Shape::Mesh with at least 10 triangles (outer contour + inner counter hole), got {preview_mesh_triangles}"
    );
}
