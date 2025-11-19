//! Built-in example flowcharts that can be quickly loaded from the UI.
//!
//! This module defines a few curated examples ranging from simple to more
//! complex pipelines to help new users get started.

use crate::types::*;
use serde_json::json;

/// Kinds of built-in examples available from the UI.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExampleKind {
    /// Producer -> Transformer -> Consumer
    BasicLinear,
    /// Branching based on data, routed to different consumers
    DecisionBranch,
    /// Simple ETL-style pipeline with transform and load stages
    EtlPipeline,
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
            name: "ETL Pipeline (Extract → Transform → Load)",
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

    fc
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

    fc
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
            messages_per_cycle: 1,
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

    fc
}
