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

#[test]
fn connection_invalid_when_consumer_is_source() {
    let mut app = FlowchartApp::default();

    // Deterministic canvas
    app.node_counter = 1;
    app.canvas.offset = egui::Vec2::ZERO;
    app.canvas.zoom_factor = 1.0;

    // Consumer (left) and Transformer (right)
    let consumer_id = app.flowchart.add_node(FlowchartNode::new(
        "C".into(),
        (160.0, 120.0),
        NodeType::Consumer { consumption_rate: 1 },
    ));
    let transformer_id = app.flowchart.add_node(FlowchartNode::new(
        "T".into(),
        (360.0, 120.0),
        NodeType::Transformer {
            script: "return msg;".into(),
            selected_outputs: None,
            globals: Default::default(),
            initial_globals: Default::default(),
        },
    ));

    let start = egui::pos2(160.0, 120.0); // on consumer
    let end = egui::pos2(360.0, 120.0); // on transformer

    let ctx = egui::Context::default();

    // Move to start
    let mut raw0 = egui::RawInput::default();
    raw0.screen_rect = Some(egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(1200.0, 800.0)));
    raw0.events = vec![egui::Event::PointerMoved(start)];
    let _ = ctx.run(raw0, |ctx| {
        ctx.set_visuals(egui::Visuals::dark());
        egui::CentralPanel::default().show(ctx, |ui| app.draw_canvas(ui));
    });

    // Press with Shift to start connection from Consumer (should be rejected later)
    let mut raw1 = egui::RawInput::default();
    raw1.screen_rect = Some(egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(1200.0, 800.0)));
    raw1.modifiers = egui::Modifiers { shift: true, ..Default::default() };
    raw1.events = vec![
        egui::Event::PointerButton { pos: start, button: egui::PointerButton::Primary, pressed: true, modifiers: egui::Modifiers::NONE },
    ];
    let _ = ctx.run(raw1, |ctx| {
        ctx.set_visuals(egui::Visuals::dark());
        egui::CentralPanel::default().show(ctx, |ui| app.draw_canvas(ui));
    });

    // Drag with Shift
    let mut raw2 = egui::RawInput::default();
    raw2.screen_rect = Some(egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(1200.0, 800.0)));
    raw2.modifiers = egui::Modifiers { shift: true, ..Default::default() };
    raw2.events = vec![egui::Event::PointerMoved(end)];
    let _ = ctx.run(raw2, |ctx| {
        ctx.set_visuals(egui::Visuals::dark());
        egui::CentralPanel::default().show(ctx, |ui| app.draw_canvas(ui));
    });

    // Release
    let mut raw3 = egui::RawInput::default();
    raw3.screen_rect = Some(egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(1200.0, 800.0)));
    raw3.events = vec![egui::Event::PointerButton { pos: end, button: egui::PointerButton::Primary, pressed: false, modifiers: egui::Modifiers::NONE }];
    let _ = ctx.run(raw3, |ctx| {
        ctx.set_visuals(egui::Visuals::dark());
        egui::CentralPanel::default().show(ctx, |ui| app.draw_canvas(ui));
    });

    // No connections should be created
    assert!(app.flowchart.connections.is_empty(), "Consumer cannot be a source");

    // Silence unused warnings for ids
    let _ = (consumer_id, transformer_id);
}

#[test]
fn connection_invalid_when_target_is_producer() {
    let mut app = FlowchartApp::default();

    app.node_counter = 1;
    app.canvas.offset = egui::Vec2::ZERO;
    app.canvas.zoom_factor = 1.0;

    // Transformer (left) and Producer (right)
    let transformer_id = app.flowchart.add_node(FlowchartNode::new(
        "T".into(),
        (160.0, 120.0),
        NodeType::Transformer { script: "return msg;".into(), selected_outputs: None, globals: Default::default(), initial_globals: Default::default() },
    ));
    let producer_id = app.flowchart.add_node(FlowchartNode::new(
        "P".into(),
        (360.0, 120.0),
        NodeType::Producer {
            message_template: serde_json::json!({}),
            start_step: 0,
            messages_per_cycle: 1,
            steps_between_cycles: 1,
            messages_produced: 0,
        },
    ));

    let start = egui::pos2(160.0, 120.0);
    let end = egui::pos2(360.0, 120.0);

    let ctx = egui::Context::default();

    // Move to start
    let mut raw0 = egui::RawInput::default();
    raw0.screen_rect = Some(egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(1200.0, 800.0)));
    raw0.events = vec![egui::Event::PointerMoved(start)];
    let _ = ctx.run(raw0, |ctx| {
        ctx.set_visuals(egui::Visuals::dark());
        egui::CentralPanel::default().show(ctx, |ui| app.draw_canvas(ui));
    });

    // Press with Shift to start connection
    let mut raw1 = egui::RawInput::default();
    raw1.screen_rect = Some(egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(1200.0, 800.0)));
    raw1.modifiers = egui::Modifiers { shift: true, ..Default::default() };
    raw1.events = vec![egui::Event::PointerButton { pos: start, button: egui::PointerButton::Primary, pressed: true, modifiers: egui::Modifiers::NONE }];
    let _ = ctx.run(raw1, |ctx| {
        ctx.set_visuals(egui::Visuals::dark());
        egui::CentralPanel::default().show(ctx, |ui| app.draw_canvas(ui));
    });

    // Drag with Shift to Producer (invalid target)
    let mut raw2 = egui::RawInput::default();
    raw2.screen_rect = Some(egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(1200.0, 800.0)));
    raw2.modifiers = egui::Modifiers { shift: true, ..Default::default() };
    raw2.events = vec![egui::Event::PointerMoved(end)];
    let _ = ctx.run(raw2, |ctx| {
        ctx.set_visuals(egui::Visuals::dark());
        egui::CentralPanel::default().show(ctx, |ui| app.draw_canvas(ui));
    });

    // Release
    let mut raw3 = egui::RawInput::default();
    raw3.screen_rect = Some(egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(1200.0, 800.0)));
    raw3.events = vec![egui::Event::PointerButton { pos: end, button: egui::PointerButton::Primary, pressed: false, modifiers: egui::Modifiers::NONE }];
    let _ = ctx.run(raw3, |ctx| {
        ctx.set_visuals(egui::Visuals::dark());
        egui::CentralPanel::default().show(ctx, |ui| app.draw_canvas(ui));
    });

    assert!(app.flowchart.connections.is_empty(), "Producer cannot be a target");

    let _ = (transformer_id, producer_id);
}

#[test]
fn connection_rejects_self_connection() {
    let mut app = FlowchartApp::default();

    app.node_counter = 1;
    app.canvas.offset = egui::Vec2::ZERO;
    app.canvas.zoom_factor = 1.0;

    // A single transformer node
    let node_id = app.flowchart.add_node(FlowchartNode::new(
        "T".into(),
        (260.0, 140.0),
        NodeType::Transformer { script: "return msg".into(), selected_outputs: None, globals: Default::default(), initial_globals: Default::default() },
    ));

    let p = egui::pos2(260.0, 140.0);
    let ctx = egui::Context::default();

    // Hover
    let mut raw0 = egui::RawInput::default();
    raw0.screen_rect = Some(egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(1200.0, 800.0)));
    raw0.events = vec![egui::Event::PointerMoved(p)];
    let _ = ctx.run(raw0, |ctx| { ctx.set_visuals(egui::Visuals::dark()); egui::CentralPanel::default().show(ctx, |ui| app.draw_canvas(ui)); });

    // Press with Shift to start connection
    let mut raw1 = egui::RawInput::default();
    raw1.screen_rect = Some(egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(1200.0, 800.0)));
    raw1.modifiers = egui::Modifiers { shift: true, ..Default::default() };
    raw1.events = vec![egui::Event::PointerButton { pos: p, button: egui::PointerButton::Primary, pressed: true, modifiers: egui::Modifiers::NONE }];
    let _ = ctx.run(raw1, |ctx| { ctx.set_visuals(egui::Visuals::dark()); egui::CentralPanel::default().show(ctx, |ui| app.draw_canvas(ui)); });

    // Release immediately over same node
    let mut raw2 = egui::RawInput::default();
    raw2.screen_rect = Some(egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(1200.0, 800.0)));
    raw2.events = vec![egui::Event::PointerButton { pos: p, button: egui::PointerButton::Primary, pressed: false, modifiers: egui::Modifiers::NONE }];
    let _ = ctx.run(raw2, |ctx| { ctx.set_visuals(egui::Visuals::dark()); egui::CentralPanel::default().show(ctx, |ui| app.draw_canvas(ui)); });

    assert!(app.flowchart.connections.is_empty(), "self-connection must be rejected");

    let _ = node_id;
}

#[test]
fn connection_duplicate_is_prevented() {
    let mut app = FlowchartApp::default();

    app.node_counter = 1;
    app.canvas.offset = egui::Vec2::ZERO;
    app.canvas.zoom_factor = 1.0;

    let producer_id = app.flowchart.add_node(FlowchartNode::new(
        "P".into(),
        (150.0, 100.0),
        NodeType::Producer { message_template: serde_json::json!({}), start_step: 0, messages_per_cycle: 1, steps_between_cycles: 1, messages_produced: 0 },
    ));
    let consumer_id = app.flowchart.add_node(FlowchartNode::new(
        "C".into(),
        (350.0, 100.0),
        NodeType::Consumer { consumption_rate: 1 },
    ));

    let start = egui::pos2(150.0, 100.0);
    let end = egui::pos2(350.0, 100.0);

    let ctx = egui::Context::default();

    // First creation
    for (mods_shift, events) in [
        (true, vec![egui::Event::PointerMoved(start)]),
        (true, vec![egui::Event::PointerButton { pos: start, button: egui::PointerButton::Primary, pressed: true, modifiers: egui::Modifiers::NONE }]),
        (true, vec![egui::Event::PointerMoved(end)]),
        (false, vec![egui::Event::PointerButton { pos: end, button: egui::PointerButton::Primary, pressed: false, modifiers: egui::Modifiers::NONE }]),
    ] {
        let mut raw = egui::RawInput::default();
        raw.screen_rect = Some(egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(1200.0, 800.0)));
        if mods_shift { raw.modifiers = egui::Modifiers { shift: true, ..Default::default() }; }
        raw.events = events;
        let _ = ctx.run(raw, |ctx| { ctx.set_visuals(egui::Visuals::dark()); egui::CentralPanel::default().show(ctx, |ui| app.draw_canvas(ui)); });
    }

    assert_eq!(app.flowchart.connections.len(), 1);

    // Attempt to create duplicate
    for (mods_shift, events) in [
        (true, vec![egui::Event::PointerMoved(start)]),
        (true, vec![egui::Event::PointerButton { pos: start, button: egui::PointerButton::Primary, pressed: true, modifiers: egui::Modifiers::NONE }]),
        (true, vec![egui::Event::PointerMoved(end)]),
        (false, vec![egui::Event::PointerButton { pos: end, button: egui::PointerButton::Primary, pressed: false, modifiers: egui::Modifiers::NONE }]),
    ] {
        let mut raw = egui::RawInput::default();
        raw.screen_rect = Some(egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(1200.0, 800.0)));
        if mods_shift { raw.modifiers = egui::Modifiers { shift: true, ..Default::default() }; }
        raw.events = events;
        let _ = ctx.run(raw, |ctx| { ctx.set_visuals(egui::Visuals::dark()); egui::CentralPanel::default().show(ctx, |ui| app.draw_canvas(ui)); });
    }

    assert_eq!(app.flowchart.connections.len(), 1, "duplicate connection must not be added");

    let _ = (producer_id, consumer_id);
}

#[test]
fn click_near_connection_selects_it() {
    let mut app = FlowchartApp::default();

    app.node_counter = 1;
    app.canvas.offset = egui::Vec2::ZERO;
    app.canvas.zoom_factor = 1.0;

    // Build a connection first
    let producer_id = app.flowchart.add_node(FlowchartNode::new(
        "P".into(),
        (200.0, 200.0),
        NodeType::Producer { message_template: serde_json::json!({}), start_step: 0, messages_per_cycle: 1, steps_between_cycles: 1, messages_produced: 0 },
    ));
    let consumer_id = app.flowchart.add_node(FlowchartNode::new(
        "C".into(),
        (400.0, 220.0),
        NodeType::Consumer { consumption_rate: 1 },
    ));

    // Create the connection via shift-drag
    let start = egui::pos2(200.0, 200.0);
    let end = egui::pos2(400.0, 220.0);
    let ctx = egui::Context::default();

    for (mods_shift, events) in [
        (true, vec![egui::Event::PointerMoved(start)]),
        (true, vec![egui::Event::PointerButton { pos: start, button: egui::PointerButton::Primary, pressed: true, modifiers: egui::Modifiers::NONE }]),
        (true, vec![egui::Event::PointerMoved(end)]),
        (false, vec![egui::Event::PointerButton { pos: end, button: egui::PointerButton::Primary, pressed: false, modifiers: egui::Modifiers::NONE }]),
    ] {
        let mut raw = egui::RawInput::default();
        raw.screen_rect = Some(egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(1200.0, 800.0)));
        if mods_shift { raw.modifiers = egui::Modifiers { shift: true, ..Default::default() }; }
        raw.events = events;
        let _ = ctx.run(raw, |ctx| { ctx.set_visuals(egui::Visuals::dark()); egui::CentralPanel::default().show(ctx, |ui| app.draw_canvas(ui)); });
    }

    assert_eq!(app.flowchart.connections.len(), 1);

    // Compute a click point near the mid-point of the connection, offset by 5px perpendicular
    let mid = egui::pos2((200.0 + 400.0) * 0.5, (200.0 + 220.0) * 0.5);
    let dir = egui::vec2(400.0 - 200.0, 220.0 - 200.0).normalized();
    let normal = egui::vec2(-dir.y, dir.x); // perpendicular
    let near_point = mid + normal * 5.0; // within CLICK_THRESHOLD=10.0

    // Click sequence: move -> press -> release on near_point
    // Use multi-frame and rely on response.clicked() path in handle_canvas_interactions
    // Frame: move
    let mut r0 = egui::RawInput::default();
    r0.screen_rect = Some(egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(1200.0, 800.0)));
    r0.events = vec![egui::Event::PointerMoved(near_point)];
    let _ = ctx.run(r0, |ctx| { ctx.set_visuals(egui::Visuals::dark()); egui::CentralPanel::default().show(ctx, |ui| app.draw_canvas(ui)); });

    // Frame: press
    let mut r1 = egui::RawInput::default();
    r1.screen_rect = Some(egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(1200.0, 800.0)));
    r1.events = vec![egui::Event::PointerButton { pos: near_point, button: egui::PointerButton::Primary, pressed: true, modifiers: egui::Modifiers::NONE }];
    let _ = ctx.run(r1, |ctx| { ctx.set_visuals(egui::Visuals::dark()); egui::CentralPanel::default().show(ctx, |ui| app.draw_canvas(ui)); });

    // Frame: release
    let mut r2 = egui::RawInput::default();
    r2.screen_rect = Some(egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(1200.0, 800.0)));
    r2.events = vec![egui::Event::PointerButton { pos: near_point, button: egui::PointerButton::Primary, pressed: false, modifiers: egui::Modifiers::NONE }];
    let _ = ctx.run(r2, |ctx| { ctx.set_visuals(egui::Visuals::dark()); egui::CentralPanel::default().show(ctx, |ui| app.draw_canvas(ui)); });

    assert_eq!(app.interaction.selected_connection, Some(0), "click near line should select it");

    let _ = (producer_id, consumer_id);
}

#[test]
fn shift_snap_drag_snaps_to_grid() {
    let mut app = FlowchartApp::default();

    app.node_counter = 1;
    app.canvas.offset = egui::Vec2::ZERO;
    app.canvas.zoom_factor = 1.0;

    // Node at an off-grid center
    let node_id = app.flowchart.add_node(FlowchartNode::new(
        "N".into(),
        (105.0, 95.0),
        NodeType::Consumer { consumption_rate: 1 },
    ));

    let start = egui::pos2(105.0, 95.0);
    let drag_to = egui::pos2(173.0, 162.0);
    let ctx = egui::Context::default();

    // Frame: move to node
    let mut r0 = egui::RawInput::default();
    r0.screen_rect = Some(egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(1200.0, 800.0)));
    r0.events = vec![egui::Event::PointerMoved(start)];
    let _ = ctx.run(r0, |ctx| { ctx.set_visuals(egui::Visuals::dark()); egui::CentralPanel::default().show(ctx, |ui| app.draw_canvas(ui)); });

    // Frame: press primary WITHOUT shift to start drag (shift would start connection)
    let mut r1 = egui::RawInput::default();
    r1.screen_rect = Some(egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(1200.0, 800.0)));
    r1.events = vec![egui::Event::PointerButton { pos: start, button: egui::PointerButton::Primary, pressed: true, modifiers: egui::Modifiers::NONE }];
    let _ = ctx.run(r1, |ctx| { ctx.set_visuals(egui::Visuals::dark()); egui::CentralPanel::default().show(ctx, |ui| app.draw_canvas(ui)); });

    // Frame: drag with Shift held to new location to trigger snapping
    let mut r2 = egui::RawInput::default();
    r2.screen_rect = Some(egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(1200.0, 800.0)));
    r2.modifiers = egui::Modifiers { shift: true, ..Default::default() };
    r2.events = vec![egui::Event::PointerMoved(drag_to)];
    let _ = ctx.run(r2, |ctx| { ctx.set_visuals(egui::Visuals::dark()); egui::CentralPanel::default().show(ctx, |ui| app.draw_canvas(ui)); });

    // Frame: release
    let mut r3 = egui::RawInput::default();
    r3.screen_rect = Some(egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(1200.0, 800.0)));
    r3.events = vec![egui::Event::PointerButton { pos: drag_to, button: egui::PointerButton::Primary, pressed: false, modifiers: egui::Modifiers::NONE }];
    let _ = ctx.run(r3, |ctx| { ctx.set_visuals(egui::Visuals::dark()); egui::CentralPanel::default().show(ctx, |ui| app.draw_canvas(ui)); });

    // Assert snapped to grid of 20x20
    let node = app.flowchart.nodes.get(&node_id).unwrap();
    let (x, y) = node.position;
    assert!((x % 20.0).abs() < 0.001, "x not snapped: {x}");
    assert!((y % 20.0).abs() < 0.001, "y not snapped: {y}");
}

// NOTE: A scroll-driven zoom test is skipped for egui 0.32 because
// the stable way to synthesize scroll that feeds `smooth_scroll_delta`
// differs across versions. We'll add this later once we standardize
// an input helper for scroll on 0.32.
