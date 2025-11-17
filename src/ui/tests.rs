use super::*;
use crate::types::{FlowchartNode, NodeType};
use eframe::egui;

/// Run a single headless egui frame with the provided input events and closure.
fn run_ui_with(events: Vec<egui::Event>, mut f: impl FnMut(&egui::Context)) -> egui::FullOutput {
    let mut raw = egui::RawInput::default();
    raw.screen_rect = Some(egui::Rect::from_min_size(
        egui::Pos2::ZERO,
        egui::vec2(1200.0, 800.0),
    ));
    raw.events = events;

    let ctx = egui::Context::default();
    ctx.run(raw, |ctx| {
        ctx.set_visuals(egui::Visuals::dark());
        f(ctx);
    })
}

#[test]
fn undo_operation_removes_last_created_node() {
    let mut app = FlowchartApp::default();

    // Arrange: ensure a deterministic canvas state
    app.canvas.offset = egui::Vec2::ZERO;
    app.canvas.zoom_factor = 1.0;

    // Seed a world position where the context menu would create a node
    app.context_menu.world_pos = (100.0, 100.0);

    // Create a node via the UI helper so it records an undo action
    app.create_node_at_pos(NodeType::Consumer { consumption_rate: 1 });
    let created_id = app.interaction.selected_node.expect("node should be selected after creation");
    assert!(app.flowchart.nodes.contains_key(&created_id));

    // Directly invoke undo logic (unit test for state change)
    app.perform_undo();

    // The node should be gone after undo
    assert!(!app.flowchart.nodes.contains_key(&created_id));
}

#[test]
fn clicking_canvas_selects_node() {
    let mut app = FlowchartApp::default();

    // Ensure no auto-centering changes offset during first draw
    app.node_counter = 1; // skip auto-centering condition
    app.canvas.offset = egui::Vec2::ZERO; // screen == world
    app.canvas.zoom_factor = 1.0;

    // Add a node at a known world position
    let world_pos = (200.0_f32, 150.0_f32);
    let node_id = app
        .flowchart
        .add_node(FlowchartNode::new(
            "A".into(),
            world_pos,
            NodeType::Consumer { consumption_rate: 1 },
        ));

    let click_pos = egui::pos2(world_pos.0, world_pos.1);

    // Drive multiple frames on the same egui Context so interaction state persists.
    let ctx = egui::Context::default();

    // First frame: move cursor over the node to establish hover
    let mut raw0 = egui::RawInput::default();
    raw0.screen_rect = Some(egui::Rect::from_min_size(
        egui::Pos2::ZERO,
        egui::vec2(1200.0, 800.0),
    ));
    raw0.events = vec![egui::Event::PointerMoved(click_pos)];
    let _ = ctx.run(raw0, |ctx| {
        ctx.set_visuals(egui::Visuals::dark());
        egui::CentralPanel::default().show(ctx, |ui| {
            app.draw_canvas(ui);
        });
    });

    // Second frame: press the primary button over the node center starts a drag and selects it
    let start = click_pos;
    let mut raw1 = egui::RawInput::default();
    raw1.screen_rect = Some(egui::Rect::from_min_size(
        egui::Pos2::ZERO,
        egui::vec2(1200.0, 800.0),
    ));
    raw1.events = vec![
        egui::Event::PointerMoved(start),
        egui::Event::PointerButton {
            pos: start,
            button: egui::PointerButton::Primary,
            pressed: true,
            modifiers: egui::Modifiers::NONE,
        },
    ];
    let _ = ctx.run(raw1, |ctx| {
        ctx.set_visuals(egui::Visuals::dark());
        egui::CentralPanel::default().show(ctx, |ui| {
            app.draw_canvas(ui);
        });
    });

    // We only need the press frame; selection should be set during drag start.

    assert_eq!(app.interaction.selected_node, Some(node_id));
}

#[test]
fn drawing_canvas_with_node_produces_shapes() {
    let mut app = FlowchartApp::default();
    app.node_counter = 1; // skip auto-centering
    app.canvas.offset = egui::Vec2::ZERO;
    app.canvas.zoom_factor = 1.0;
    app.canvas.show_grid = false; // reduce variability in shape count

    // Add a node so there is something to paint
    app.flowchart.add_node(FlowchartNode::new(
        "B".into(),
        (50.0, 50.0),
        NodeType::Consumer { consumption_rate: 1 },
    ));

    let out = run_ui_with(vec![], |ctx| {
        egui::CentralPanel::default().show(ctx, |ui| {
            app.draw_canvas(ui);
        });
    });

    // We don't assert an exact number, just that something was painted.
    assert!(out.shapes.len() > 0, "expected some shapes to be painted");
}
