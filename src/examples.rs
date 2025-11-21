//! Built-in example flowcharts that can be quickly loaded from the UI.
//!
//! This module defines a few curated examples ranging from simple to more
//! complex pipelines to help new users get started.

use crate::types::*;
use serde_json::json;
use uuid::Uuid;

/// Recenters all nodes in the given flowchart so that the bounding-box center
/// of the node positions lies at the world origin (0, 0).
fn center_flowchart(mut fc: Flowchart) -> Flowchart {
    if fc.nodes.is_empty() {
        return fc;
    }

    let mut min_x = f32::INFINITY;
    let mut max_x = f32::NEG_INFINITY;
    let mut min_y = f32::INFINITY;
    let mut max_y = f32::NEG_INFINITY;

    for node in fc.nodes.values() {
        let (x, y) = node.position;
        if x < min_x { min_x = x; }
        if x > max_x { max_x = x; }
        if y < min_y { min_y = y; }
        if y > max_y { max_y = y; }
    }

    if !min_x.is_finite() || !max_x.is_finite() || !min_y.is_finite() || !max_y.is_finite() {
        return fc;
    }

    let cx = (min_x + max_x) * 0.5;
    let cy = (min_y + max_y) * 0.5;

    // Translate all nodes so that (cx, cy) maps to (0, 0)
    for node in fc.nodes.values_mut() {
        node.position.0 -= cx;
        node.position.1 -= cy;
    }

    fc
}

/// Kinds of built-in examples available from the UI.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExampleKind {
    /// Producer -> Transformer -> Consumer
    BasicLinear,
    /// Branching based on data, routed to different consumers
    DecisionBranch,
    /// Simple ETL-style pipeline with transform and load stages
    EtlPipeline,
    /// Client talks to Server via a Router, with ACKs and Requests
    SocketClientServer,
    /// Packet traverses switches from Host A to Host B, others drop
    NetworkPacketTraversal,
}

/// Metadata for a single example.
pub struct ExampleInfo {
    /// Stable identifier for the example
    pub kind: ExampleKind,
    /// Human-friendly display name
    pub name: &'static str,
}

/// Returns all examples with their display names.
pub const fn all_examples() -> &'static [ExampleInfo] {
    const EXAMPLES: &[ExampleInfo] = &[
        ExampleInfo {
            kind: ExampleKind::BasicLinear,
            name: "Basic Linear Pipeline",
        },
        ExampleInfo {
            kind: ExampleKind::DecisionBranch,
            name: "Decision Branch (Even/Odd)",
        },
        ExampleInfo {
            kind: ExampleKind::EtlPipeline,
            name: "ETL Pipeline (Extract > Transform > Load)",
        },
        ExampleInfo {
            kind: ExampleKind::SocketClientServer,
            name: "Socket Client ↔ Server via Router",
        },
        ExampleInfo {
            kind: ExampleKind::NetworkPacketTraversal,
            name: "Network Packet Traversal (Switches)",
        },
    ];
    EXAMPLES
}

/// Builds a flowchart instance for the given example kind.
pub fn build_example(kind: ExampleKind) -> Flowchart {
    match kind {
        ExampleKind::BasicLinear => build_basic_linear(),
        ExampleKind::DecisionBranch => build_decision_branch(),
        ExampleKind::EtlPipeline => build_etl_pipeline(),
        ExampleKind::SocketClientServer => build_socket_client_server(),
        ExampleKind::NetworkPacketTraversal => build_network_packet_traversal(),
    }
}

fn build_basic_linear() -> Flowchart {
    let mut fc = Flowchart::new();

    // Producer: emits a simple message every cycle starting at step 0
    let prod = FlowchartNode::new(
        "Producer".into(),
        (100.0, 200.0),
        NodeType::Producer {
            message_template: json!({"value": 1}),
            start_step: 0,
            messages_per_cycle: 1,
            steps_between_cycles: 1,
            messages_produced: 0,
        },
    );
    let prod_id = fc.add_node(prod);

    // Transformer: pass-through script
    let script = r#"function transform(input) {
    // Simply forward the message unchanged
    return input;
}"#
    .to_string();
    let trans = FlowchartNode::new(
        "Transformer".into(),
        (350.0, 200.0),
        NodeType::Transformer {
            script,
            selected_outputs: None,
            globals: Default::default(),
            initial_globals: Default::default(),
        },
    );
    let trans_id = fc.add_node(trans);

    // Consumer
    let cons = FlowchartNode::new(
        "Consumer".into(),
        (600.0, 200.0),
        NodeType::Consumer { consumption_rate: 1 },
    );
    let cons_id = fc.add_node(cons);

    let _ = fc.add_connection(prod_id, trans_id);
    let _ = fc.add_connection(trans_id, cons_id);

    center_flowchart(fc)
}

fn build_decision_branch() -> Flowchart {
    let mut fc = Flowchart::new();

    // Producer of numbers 0.. incrementing via a counter in state
    let prod = FlowchartNode::new(
        "Number Source".into(),
        (60.0, 180.0),
        NodeType::Producer {
            message_template: json!({"n": 0}),
            start_step: 0,
            messages_per_cycle: 10,
            steps_between_cycles: 1,
            messages_produced: 0,
        },
    );
    let prod_id = fc.add_node(prod);

    // Transformer: increments a counter in state and route based on even/odd
    let script = r#"function transform(input) {
    // Increment a counter persisted in state
    state.count++;

    const n = state.count;
    const out = { n };
    // Route based on parity using programmatic targets
    if (n % 2 === 0) {
        out.__targets = ["Even Bin"];
    } else {
        out.__targets = ["Odd Bin"];
    }
    return out;
}"#
    .to_string();
    let mut globals : serde_json::Map<String, serde_json::Value> = serde_json::Map::new();
    globals.insert(
        "count".to_string(),
        serde_json::Value::Number(serde_json::Number::from(0)),
    );
    let branch = FlowchartNode::new(
        "Parity Router".into(),
        (300.0, 180.0),
        NodeType::Transformer {
            script,
            selected_outputs: None,
            globals: globals.clone(),
            initial_globals: globals,
        },
    );
    let branch_id = fc.add_node(branch);

    let even = FlowchartNode::new(
        "Even Bin".into(),
        (550.0, 100.0),
        NodeType::Consumer { consumption_rate: 4 },
    );
    let even_id = fc.add_node(even);

    let odd = FlowchartNode::new(
        "Odd Bin".into(),
        (550.0, 260.0),
        NodeType::Consumer { consumption_rate: 4 },
    );
    let odd_id = fc.add_node(odd);

    let _ = fc.add_connection(prod_id, branch_id);
    let _ = fc.add_connection(branch_id, even_id);
    let _ = fc.add_connection(branch_id, odd_id);

    center_flowchart(fc)
}

fn build_etl_pipeline() -> Flowchart {
    let mut fc = Flowchart::new();

    // Producer: emits records to extract
    let prod = FlowchartNode::new(
        "Source".into(),
        (40.0, 260.0),
        NodeType::Producer {
            message_template: json!({"record": {"id": 1, "raw": true}}),
            start_step: 0,
            messages_per_cycle: 6,
            steps_between_cycles: 2,
            messages_produced: 0,
        },
    );
    let prod_id = fc.add_node(prod);

    // Extract stage (no-op here)
    let extract = FlowchartNode::new(
        "Extract".into(),
        (240.0, 260.0),
        NodeType::Transformer {
            script: r#"function transform(input) {
    return input;
}"#.into(),
            selected_outputs: None,
            globals: Default::default(),
            initial_globals: Default::default(),
        },
    );
    let extract_id = fc.add_node(extract);

    // Transform stage: normalize shape
    let transform_script = r#"function transform(input) {
    const rec = input.record;
    return {
        record: {
            id: rec.id,
            raw: false,
            ts: Date.now()
        }
    };
}"#
    .to_string();
    let transform = FlowchartNode::new(
        "Transform".into(),
        (460.0, 260.0),
        NodeType::Transformer {
            script: transform_script,
            selected_outputs: None,
            globals: Default::default(),
            initial_globals: Default::default(),
        },
    );
    let transform_id = fc.add_node(transform);

    // Load stage: simulate success/failure routing using state.retry toggling
    let load_script = r#"function transform(input) {
    const out = { record: input.record };
    if (state.retries % 2 === 0) {
        // first attempt goes to Retry Queue
        out.__targets = ["Retry Queue"];
    } else {
        out.__targets = ["Warehouse"];
    }
    state.retries++;
    return out;
}"#
    .to_string();
    let mut globals : serde_json::Map<String, serde_json::Value> = serde_json::Map::new();
    globals.insert(
        "retries".to_string(),
        serde_json::Value::Number(serde_json::Number::from(0)),
    );
    let load = FlowchartNode::new(
        "Load".into(),
        (680.0, 260.0),
        NodeType::Transformer {
            script: load_script,
            selected_outputs: None,
            globals: globals.clone(),
            initial_globals: globals,
        },
    );
    let load_id = fc.add_node(load);

    let success = FlowchartNode::new(
        "Warehouse".into(),
        (900.0, 200.0),
        NodeType::Consumer { consumption_rate: 8 },
    );
    let success_id = fc.add_node(success);

    let retry = FlowchartNode::new(
        "Retry Queue".into(),
        (900.0, 320.0),
        NodeType::Consumer { consumption_rate: 2 },
    );
    let retry_id = fc.add_node(retry);

    let _ = fc.add_connection(prod_id, extract_id);
    let _ = fc.add_connection(extract_id, transform_id);
    let _ = fc.add_connection(transform_id, load_id);
    let _ = fc.add_connection(load_id, success_id);
    let _ = fc.add_connection(load_id, retry_id);

    center_flowchart(fc)
}

/// Client/Server communication through a Router.
///
/// Flow:
/// - Client App (Producer) emits a SYN to start a session to Server.
/// - Router (Transformer) forwards by dst to Server or Client Handler.
/// - Server (Transformer) on SYN responds SYN-ACK back to Client; on REQUEST responds RESPONSE.
/// - Client Handler (Transformer) on SYN-ACK sends ACK + REQUEST to Server; on RESPONSE forwards to Client Sink.
/// - Client Sink (Consumer) represents the application receiving the server response.
fn build_socket_client_server() -> Flowchart {
    let mut fc = Flowchart::new();

    // Producer: Client App initiates a SYN toward Server via Router
    let client_app = FlowchartNode::new(
        "Client App".into(),
        (80.0, 180.0),
        NodeType::Producer {
            message_template: json!({
                "type": "SYN",
                "src": "Client",
                "dst": "Server",
                "seq": 1
            }),
            start_step: 0,
            messages_per_cycle: 1,
            steps_between_cycles: 6,
            messages_produced: 0,
        },
    );
    let client_app_id = fc.add_node(client_app);

    // Router: forwards based on dst field to either Server or Client Handler
    let router_script = r#"function transform(input) {
    // Forward based on destination; default broadcast if unknown
    const dst = input.dst;
    const out = Object.assign({}, input);
    if (dst === "Server") {
        out.__targets = ["Server"];
    } else if (dst === "Client") {
        out.__targets = ["Client Handler"];
    } else {
        // If unknown, broadcast to all connected
        out.__targets = null;
    }
    return out;
}"#
    .to_string();
    let router = FlowchartNode::new(
        "Router".into(),
        (320.0, 180.0),
        NodeType::Transformer {
            script: router_script,
            selected_outputs: None,
            globals: Default::default(),
            initial_globals: Default::default(),
        },
    );
    let router_id = fc.add_node(router);

    // Server: responds to SYN with SYN-ACK (dst=Client), and to REQUEST with RESPONSE
    let server_script = r#"function transform(input) {
    const out = { };
    if (input.type === "SYN") {
        out.type = "SYN-ACK";
        out.src = "Server";
        out.dst = "Client";
        out.ack = input.seq + 1;
    } else if (input.type === "REQUEST") {
        out.type = "RESPONSE";
        out.src = "Server";
        out.dst = "Client";
        out.data = { ok: true };
    } else if (input.type === "ACK") {
        // Ignore terminal ACKs at server
        return { note: "ACK received at server" };
    } else {
        return { note: "Unknown at server" };
    }
    // Send back via Router by targeting destination name
    out.__targets = ["Router"];
    return out;
}"#
    .to_string();
    let server = FlowchartNode::new(
        "Server".into(),
        (560.0, 120.0),
        NodeType::Transformer {
            script: server_script,
            selected_outputs: None,
            globals: Default::default(),
            initial_globals: Default::default(),
        },
    );
    let server_id = fc.add_node(server);

    // Client Handler: reacts to SYN-ACK with ACK+REQUEST to Server, and forwards RESPONSE to Client Sink
    let client_handler_script = r#"function transform(input) {
    // Client-side protocol handler
    if (input.type === "SYN-ACK") {
        // Respond with ACK and send a REQUEST
        const ack = { type: "ACK", src: "Client", dst: "Server", ack: input.ack };
        const req = { type: "REQUEST", src: "Client", dst: "Server", path: "/hello" };
        // Send both via Router
        ack.__targets = ["Router"]; req.__targets = ["Router"]; 
        return [ack, req];
    }
    if (input.type === "RESPONSE") {
        // Deliver to Client Sink
        const out = Object.assign({}, input);
        out.__targets = ["Client Sink"];
        return out;
    }
    // Ignore other messages
    return { note: "Client handler idle" };
}"#
    .to_string();
    let client_handler = FlowchartNode::new(
        "Client Handler".into(),
        (560.0, 240.0),
        NodeType::Transformer {
            script: client_handler_script,
            selected_outputs: None,
            globals: Default::default(),
            initial_globals: Default::default(),
        },
    );
    let client_handler_id = fc.add_node(client_handler);

    // Client Sink: final consumer of the server's response
    let client_sink = FlowchartNode::new(
        "Client Sink".into(),
        (800.0, 240.0),
        NodeType::Consumer { consumption_rate: 4 },
    );
    let client_sink_id = fc.add_node(client_sink);

    // Wire up connections
    let _ = fc.add_connection(client_app_id, router_id);     // Client App -> Router
    let _ = fc.add_connection(router_id, server_id);         // Router -> Server
    let _ = fc.add_connection(router_id, client_handler_id); // Router -> Client Handler
    let _ = fc.add_connection(server_id, router_id);         // Server -> Router
    let _ = fc.add_connection(client_handler_id, router_id); // Client Handler -> Router
    let _ = fc.add_connection(client_handler_id, client_sink_id); // Client Handler -> Client Sink

    center_flowchart(fc)
}

/// Network packet traversal with branching subnets and multiple sources.
///
/// Topology:
/// - Core Switch connects three access switches: Switch A, Switch B, Switch C (three subnets)
/// - Subnet A hosts: A1 (producer), A2 (consumer), A3 (consumer)
/// - Subnet B hosts: B1 (producer), B2 (consumer), B3 (consumer)
/// - Subnet C hosts: C1 (consumer), C2 (consumer), C3 (consumer)
///
/// Traffic:
/// - A1 periodically sends to B2
/// - B1 periodically sends to C3
///
/// Routing:
/// - Access switches forward off-subnet traffic upstream to Core Switch.
/// - Core Switch forwards by first letter of `dst` to the correct access switch.
/// - Access switches deliver to local hosts only if `dst` matches an attached host name; otherwise drop.
fn build_network_packet_traversal() -> Flowchart {
    let mut fc = Flowchart::new();

    // Core Switch: routes by first letter of dst to a subnet access switch
    let core_script = r#"function transform(input) {
    const dst = String(input.dst || "");
    const out = Object.assign({}, input);
    if (dst.startsWith("A")) {
        out.__targets = ["Switch A"]; // toward subnet A
    } else if (dst.startsWith("B")) {
        out.__targets = ["Switch B"]; // toward subnet B
    } else if (dst.startsWith("C")) {
        out.__targets = ["Switch C"]; // toward subnet C
    } else {
        return { note: "unknown network at core" };
    }
    return out;
}"#
    .to_string();
    let core = FlowchartNode::new(
        "Core Switch".into(),
        // Start a bit more spaced out – auto-centering will normalize later
        (0.0, -140.0),
        NodeType::Transformer {
            script: core_script,
            selected_outputs: None,
            globals: Default::default(),
            initial_globals: Default::default(),
        },
    );
    let core_id = fc.add_node(core);

    // Access Switch A: local deliver to A2/A3, otherwise upstream to Core
    let switch_a_script = r#"function transform(input) {
    const dst = String(input.dst || "");
    const out = Object.assign({}, input);
    if (dst.startsWith("A")) {
        if (dst === "A2") { out.__targets = ["A2"]; return out; }
        if (dst === "A3") { out.__targets = ["A3"]; return out; }
        return { note: "unknown host on subnet A" };
    }
    out.__targets = ["Core Switch"]; // off-subnet
    return out;
}"#
    .to_string();
    let switch_a = FlowchartNode::new(
        "Switch A".into(),
        (-400.0, 80.0),
        NodeType::Transformer {
            script: switch_a_script,
            selected_outputs: None,
            globals: Default::default(),
            initial_globals: Default::default(),
        },
    );
    let switch_a_id = fc.add_node(switch_a);

    // Access Switch B: local deliver to B2/B3, otherwise upstream to Core
    let switch_b_script = r#"function transform(input) {
    const dst = String(input.dst || "");
    const out = Object.assign({}, input);
    if (dst.startsWith("B")) {
        if (dst === "B2") { out.__targets = ["B2"]; return out; }
        if (dst === "B3") { out.__targets = ["B3"]; return out; }
        return { note: "unknown host on subnet B" };
    }
    out.__targets = ["Core Switch"]; // off-subnet
    return out;
}"#
    .to_string();
    let switch_b = FlowchartNode::new(
        "Switch B".into(),
        (0.0, 120.0),
        NodeType::Transformer {
            script: switch_b_script,
            selected_outputs: None,
            globals: Default::default(),
            initial_globals: Default::default(),
        },
    );
    let switch_b_id = fc.add_node(switch_b);

    // Access Switch C: local deliver to C1/C2/C3, otherwise upstream to Core
    let switch_c_script = r#"function transform(input) {
    const dst = String(input.dst || "");
    const out = Object.assign({}, input);
    if (dst.startsWith("C")) {
        if (dst === "C1") { out.__targets = ["C1"]; return out; }
        if (dst === "C2") { out.__targets = ["C2"]; return out; }
        if (dst === "C3") { out.__targets = ["C3"]; return out; }
        return { note: "unknown host on subnet C" };
    }
    out.__targets = ["Core Switch"]; // off-subnet
    return out;
}"#
    .to_string();
    let switch_c = FlowchartNode::new(
        "Switch C".into(),
        (400.0, 80.0),
        NodeType::Transformer {
            script: switch_c_script,
            selected_outputs: None,
            globals: Default::default(),
            initial_globals: Default::default(),
        },
    );
    let switch_c_id = fc.add_node(switch_c);

    // Subnet A hosts
    let a1 = FlowchartNode::new(
        "A1".into(),
        (-560.0, 220.0),
        NodeType::Producer {
            message_template: json!({
                "src": "A1",
                "dst": "B2",
                "payload": {"msg": "ping from A1"}
            }),
            start_step: 0,
            messages_per_cycle: 1,
            steps_between_cycles: 10,
            messages_produced: 0,
        },
    );
    let a1_id = fc.add_node(a1);

    let a2 = FlowchartNode::new(
        "A2".into(),
        (-400.0, 240.0),
        NodeType::Consumer { consumption_rate: 6 },
    );
    let a2_id = fc.add_node(a2);

    let a3 = FlowchartNode::new(
        "A3".into(),
        (-400.0, 300.0),
        NodeType::Consumer { consumption_rate: 6 },
    );
    let a3_id = fc.add_node(a3);

    // Subnet B hosts
    let b1 = FlowchartNode::new(
        "B1".into(),
        (0.0, 230.0),
        NodeType::Producer {
            message_template: json!({
                "src": "B1",
                "dst": "C3",
                "payload": {"msg": "hello from B1"}
            }),
            start_step: 2,
            messages_per_cycle: 1,
            steps_between_cycles: 12,
            messages_produced: 0,
        },
    );
    let b1_id = fc.add_node(b1);

    let b2 = FlowchartNode::new(
        "B2".into(),
        (0.0, 270.0),
        NodeType::Consumer { consumption_rate: 6 },
    );
    let b2_id = fc.add_node(b2);

    let b3 = FlowchartNode::new(
        "B3".into(),
        (0.0, 330.0),
        NodeType::Consumer { consumption_rate: 6 },
    );
    let b3_id = fc.add_node(b3);

    // Subnet C hosts
    let c1 = FlowchartNode::new(
        "C1".into(),
        (400.0, 220.0),
        NodeType::Consumer { consumption_rate: 6 },
    );
    let c1_id = fc.add_node(c1);

    let c2 = FlowchartNode::new(
        "C2".into(),
        (400.0, 280.0),
        NodeType::Consumer { consumption_rate: 6 },
    );
    let c2_id = fc.add_node(c2);

    let c3 = FlowchartNode::new(
        "C3".into(),
        (400.0, 340.0),
        NodeType::Consumer { consumption_rate: 6 },
    );
    let c3_id = fc.add_node(c3);

    // Connections
    // Producers to their access switches
    let _ = fc.add_connection(a1_id, switch_a_id);
    let _ = fc.add_connection(b1_id, switch_b_id);

    // Access switches <-> Core
    let _ = fc.add_connection(switch_a_id, core_id);
    let _ = fc.add_connection(core_id, switch_a_id);

    let _ = fc.add_connection(switch_b_id, core_id);
    let _ = fc.add_connection(core_id, switch_b_id);

    let _ = fc.add_connection(switch_c_id, core_id);
    let _ = fc.add_connection(core_id, switch_c_id);

    // Access switches to local hosts (consumers only)
    let _ = fc.add_connection(switch_a_id, a2_id);
    let _ = fc.add_connection(switch_a_id, a3_id);

    let _ = fc.add_connection(switch_b_id, b2_id);
    let _ = fc.add_connection(switch_b_id, b3_id);

    let _ = fc.add_connection(switch_c_id, c1_id);
    let _ = fc.add_connection(switch_c_id, c2_id);
    let _ = fc.add_connection(switch_c_id, c3_id);

    // Add visual groups for subnets and core
    let subnet_a = Group {
        id: Uuid::new_v4(),
        name: "Subnet A".into(),
        members: vec![switch_a_id, a1_id, a2_id, a3_id],
        drawing: GroupDrawingMode::Rectangle,
    };
    let subnet_b = Group {
        id: Uuid::new_v4(),
        name: "Subnet B".into(),
        members: vec![switch_b_id, b1_id, b2_id, b3_id],
        drawing: GroupDrawingMode::Rectangle,
    };
    let subnet_c = Group {
        id: Uuid::new_v4(),
        name: "Subnet C".into(),
        members: vec![switch_c_id, c1_id, c2_id, c3_id],
        drawing: GroupDrawingMode::Rectangle,
    };
    let core_grp = Group {
        id: Uuid::new_v4(),
        name: "Core".into(),
        members: vec![core_id],
        drawing: GroupDrawingMode::Rectangle,
    };

    fc.groups.insert(subnet_a.id, subnet_a);
    fc.groups.insert(subnet_b.id, subnet_b);
    fc.groups.insert(subnet_c.id, subnet_c);
    fc.groups.insert(core_grp.id, core_grp);

    center_flowchart(fc)
}
