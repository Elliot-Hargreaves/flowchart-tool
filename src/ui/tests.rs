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

#[test]
fn marquee_multi_selects_nodes_inside_rectangle() {
    let mut app = FlowchartApp::default();

    // Ensure deterministic canvas coords
    app.node_counter = 1; // skip auto-centering
    app.canvas.offset = egui::Vec2::ZERO;
    app.canvas.zoom_factor = 1.0;

    // Place two nodes that will be inside the marquee rectangle
    let n1 = app
        .flowchart
        .add_node(FlowchartNode::new(
            "N1".into(),
            (150.0, 120.0),
            NodeType::Consumer { consumption_rate: 1 },
        ));
    let n2 = app
        .flowchart
        .add_node(FlowchartNode::new(
            "N2".into(),
            (280.0, 180.0),
            NodeType::Consumer { consumption_rate: 1 },
        ));

    // Start drag on empty space, drag to cover both nodes, then release
    let start = egui::pos2(100.0, 80.0); // empty space
    let end = egui::pos2(320.0, 220.0); // covers both centers above

    let ctx = egui::Context::default();

    // Frame 0: move to start
    let mut raw0 = egui::RawInput::default();
    raw0.screen_rect = Some(egui::Rect::from_min_size(
        egui::Pos2::ZERO,
        egui::vec2(1200.0, 800.0),
    ));
    raw0.events = vec![egui::Event::PointerMoved(start)];
    let _ = ctx.run(raw0, |ctx| {
        ctx.set_visuals(egui::Visuals::dark());
        egui::CentralPanel::default().show(ctx, |ui| app.draw_canvas(ui));
    });

    // Frame 1: press primary
    let mut raw1 = egui::RawInput::default();
    raw1.screen_rect = Some(egui::Rect::from_min_size(
        egui::Pos2::ZERO,
        egui::vec2(1200.0, 800.0),
    ));
    raw1.events = vec![egui::Event::PointerButton {
        pos: start,
        button: egui::PointerButton::Primary,
        pressed: true,
        modifiers: egui::Modifiers::NONE,
    }];
    let _ = ctx.run(raw1, |ctx| {
        ctx.set_visuals(egui::Visuals::dark());
        egui::CentralPanel::default().show(ctx, |ui| app.draw_canvas(ui));
    });

    // Frame 2: drag to end
    let mut raw2 = egui::RawInput::default();
    raw2.screen_rect = Some(egui::Rect::from_min_size(
        egui::Pos2::ZERO,
        egui::vec2(1200.0, 800.0),
    ));
    raw2.events = vec![egui::Event::PointerMoved(end)];
    let _ = ctx.run(raw2, |ctx| {
        ctx.set_visuals(egui::Visuals::dark());
        egui::CentralPanel::default().show(ctx, |ui| app.draw_canvas(ui));
    });

    // Frame 3: release
    let mut raw3 = egui::RawInput::default();
    raw3.screen_rect = Some(egui::Rect::from_min_size(
        egui::Pos2::ZERO,
        egui::vec2(1200.0, 800.0),
    ));
    raw3.events = vec![egui::Event::PointerButton {
        pos: end,
        button: egui::PointerButton::Primary,
        pressed: false,
        modifiers: egui::Modifiers::NONE,
    }];
    let _ = ctx.run(raw3, |ctx| {
        ctx.set_visuals(egui::Visuals::dark());
        egui::CentralPanel::default().show(ctx, |ui| app.draw_canvas(ui));
    });

    // Assert both nodes are selected; order is not guaranteed
    let mut sel = app.interaction.selected_nodes.clone();
    sel.sort_by_key(|id| id.as_u128());
    let mut expected = vec![n1, n2];
    expected.sort_by_key(|id| id.as_u128());
    assert_eq!(sel, expected, "marquee should select both nodes");

    // Marquee visuals should be cleared
    assert!(app.interaction.marquee_start.is_none());
    assert!(app.interaction.marquee_end.is_none());
}

#[test]
fn shift_drag_creates_connection_between_nodes() {
    let mut app = FlowchartApp::default();

    // Deterministic canvas
    app.node_counter = 1;
    app.canvas.offset = egui::Vec2::ZERO;
    app.canvas.zoom_factor = 1.0;

    // Create a valid connection pair: Producer -> Consumer
    let producer_id = app.flowchart.add_node(FlowchartNode::new(
        "P".into(),
        (160.0, 120.0),
        NodeType::Producer {
            message_template: serde_json::json!({}),
            start_step: 0,
            messages_per_cycle: 1,
            steps_between_cycles: 1,
            messages_produced: 0,
        },
    ));

    let consumer_id = app.flowchart.add_node(FlowchartNode::new(
        "C".into(),
        (360.0, 120.0),
        NodeType::Consumer { consumption_rate: 1 },
    ));

    let start = egui::pos2(160.0, 120.0);
    let end = egui::pos2(360.0, 120.0);

    let ctx = egui::Context::default();

    // Frame 0: move to start
    let mut raw0 = egui::RawInput::default();
    raw0.screen_rect = Some(egui::Rect::from_min_size(
        egui::Pos2::ZERO,
        egui::vec2(1200.0, 800.0),
    ));
    raw0.events = vec![egui::Event::PointerMoved(start)];
    let _ = ctx.run(raw0, |ctx| {
        ctx.set_visuals(egui::Visuals::dark());
        egui::CentralPanel::default().show(ctx, |ui| app.draw_canvas(ui));
    });

    // Frame 1: keep mouse idle but prepare Shift state in following frame
    let mut raw1 = egui::RawInput::default();
    raw1.screen_rect = Some(egui::Rect::from_min_size(
        egui::Pos2::ZERO,
        egui::vec2(1200.0, 800.0),
    ));
    raw1.events = vec![];
    let _ = ctx.run(raw1, |ctx| {
        ctx.set_visuals(egui::Visuals::dark());
        egui::CentralPanel::default().show(ctx, |ui| app.draw_canvas(ui));
    });

    // Frame 2: press primary over producer to start connection
    let mut raw2 = egui::RawInput::default();
    raw2.screen_rect = Some(egui::Rect::from_min_size(
        egui::Pos2::ZERO,
        egui::vec2(1200.0, 800.0),
    ));
    // Hold Shift during press to trigger connection drawing path
    raw2.modifiers = egui::Modifiers {
        shift: true,
        alt: false,
        ctrl: false,
        mac_cmd: false,
        command: false,
    };
    raw2.events = vec![
        egui::Event::PointerMoved(start),
        egui::Event::PointerButton {
            pos: start,
            button: egui::PointerButton::Primary,
            pressed: true,
            modifiers: egui::Modifiers::NONE,
        },
    ];
    let _ = ctx.run(raw2, |ctx| {
        ctx.set_visuals(egui::Visuals::dark());
        egui::CentralPanel::default().show(ctx, |ui| app.draw_canvas(ui));
    });

    // Frame 3: drag towards consumer (update preview)
    let mut raw3 = egui::RawInput::default();
    raw3.screen_rect = Some(egui::Rect::from_min_size(
        egui::Pos2::ZERO,
        egui::vec2(1200.0, 800.0),
    ));
    // Keep Shift held during drag
    raw3.modifiers = egui::Modifiers {
        shift: true,
        alt: false,
        ctrl: false,
        mac_cmd: false,
        command: false,
    };
    raw3.events = vec![egui::Event::PointerMoved(end)];
    let _ = ctx.run(raw3, |ctx| {
        ctx.set_visuals(egui::Visuals::dark());
        egui::CentralPanel::default().show(ctx, |ui| app.draw_canvas(ui));
    });

    // Frame 4: release primary over consumer to finalize connection
    let mut raw4 = egui::RawInput::default();
    raw4.screen_rect = Some(egui::Rect::from_min_size(
        egui::Pos2::ZERO,
        egui::vec2(1200.0, 800.0),
    ));
    // Shift can be released here; not required for finalize
    raw4.events = vec![egui::Event::PointerButton {
        pos: end,
        button: egui::PointerButton::Primary,
        pressed: false,
        modifiers: egui::Modifiers::NONE,
    }];
    let _ = ctx.run(raw4, |ctx| {
        ctx.set_visuals(egui::Visuals::dark());
        egui::CentralPanel::default().show(ctx, |ui| app.draw_canvas(ui));
    });

    // Assert: exactly one connection Producer -> Consumer exists
    assert_eq!(app.flowchart.connections.len(), 1, "one connection expected");
    let conn = &app.flowchart.connections[0];
    assert_eq!(conn.from, producer_id);
    assert_eq!(conn.to, consumer_id);
}
