//! Simulation engine and logic for the flowchart tool.
//!
//! This module handles the execution of flowchart simulations, including message
//! generation, consumption, transformation via Lua scripts, and message routing
//! between nodes.

use crate::script_engine::{create_script_engine, JavaScriptEngine};
use crate::types::*;
use serde::{Deserialize, Serialize};

/// Engine responsible for running flowchart simulations.
///
/// The simulation engine processes nodes in sequence, handles message flow,
/// and executes transformation scripts.
#[derive(Serialize, Deserialize)]
pub struct SimulationEngine {
    /// The JavaScript runtime environment for script execution
    #[serde(skip)]
    #[allow(dead_code)]
    script_engine: Option<JavaScriptEngine>,
}

impl Default for SimulationEngine {
    fn default() -> Self {
        Self::new()
    }
}

impl SimulationEngine {
    /// Creates a new simulation engine with a fresh Lua environment.
    pub fn new() -> Self {
        let script_engine = create_script_engine().ok();
        Self { script_engine }
    }

    /// Executes a single simulation step on the given flowchart.
    ///
    /// This method:
    /// 1. Moves messages along connections
    /// 2. Delivers messages that have reached their destinations
    /// 3. Processes each node according to its type
    ///
    /// # Arguments
    ///
    /// * `flowchart` - The flowchart to simulate
    ///
    /// # Returns
    ///
    /// A vector of messages that were delivered during this step.
    pub fn step(&mut self, flowchart: &mut Flowchart) -> Vec<(NodeId, Message)> {
        let mut delivered_messages = Vec::new();

        // Collect all messages for delivery and clear connections
        for connection in &mut flowchart.connections {
            // All messages are delivered immediately in one step
            for message in connection.messages.drain(..) {
                delivered_messages.push((connection.to, message));
            }
        }

        // Process producer nodes and generate messages
        // Collect node IDs first to avoid borrow conflicts
        let node_ids: Vec<_> = flowchart.nodes.keys().cloned().collect();
        let current_step = flowchart.current_step;

        for node_id in node_ids {
            if let Some(node) = flowchart.nodes.get_mut(&node_id) {
                match node.node_type.clone() {
                    NodeType::Producer {
                        message_template,
                        start_step,
                        messages_per_cycle,
                        steps_between_cycles,
                        messages_produced: _,
                    } => {
                        let generated_messages = self.process_producer_node(
                            node,
                            &message_template,
                            start_step,
                            messages_per_cycle,
                            steps_between_cycles,
                            current_step,
                        );

                        // Add generated messages to all outgoing connections
                        if !generated_messages.is_empty() {
                            for connection in &mut flowchart.connections {
                                if connection.from == node_id {
                                    for message in &generated_messages {
                                        connection.messages.push(message.clone());
                                    }
                                }
                            }
                        }
                    }
                    NodeType::Consumer {
                        consumption_rate: _,
                    } => {
                        self.process_consumer_node(node);
                    }
                    NodeType::Transformer { script, .. } => {
                        self.process_transformer_node(node, &script);
                    }
                }
            }
        }

        // Increment step counter
        flowchart.current_step += 1;

        delivered_messages
    }

    /// Processes a producer node by potentially generating new messages.
    ///
    /// # Arguments
    ///
    /// * `node` - The producer node to process
    /// * `message_template` - The JSON template for messages to produce
    /// * `start_step` - Which step to start producing messages
    /// * `messages_per_cycle` - Total number of messages to generate (not per cycle, but in total)
    /// * `steps_between_cycles` - Number of steps to wait between production cycles
    /// * `current_step` - The current simulation step
    ///
    /// # Returns
    ///
    /// A vector of messages that were generated during this step
    fn process_producer_node(
        &self,
        node: &mut FlowchartNode,
        message_template: &serde_json::Value,
        start_step: u64,
        messages_per_cycle: u32,
        steps_between_cycles: u32,
        current_step: u64,
    ) -> Vec<Message> {
        let mut generated_messages = Vec::new();

        // Check if we should produce messages on this step
        if current_step < start_step {
            node.state = NodeState::Idle;
            return generated_messages;
        }

        // Get the messages_produced counter from the node
        let messages_produced = if let NodeType::Producer {
            messages_produced, ..
        } = &node.node_type
        {
            *messages_produced
        } else {
            0
        };

        // Check if we've already produced all messages
        if messages_produced >= messages_per_cycle {
            node.state = NodeState::Idle;
            return generated_messages;
        }

        let steps_since_start = current_step - start_step;

        // Check if this is a production cycle step
        let should_produce = if steps_between_cycles == 0 {
            // Produce every step after start_step
            true
        } else {
            // Produce on start_step, then every steps_between_cycles steps
            steps_since_start.is_multiple_of(steps_between_cycles as u64)
        };

        if should_produce {
            node.state = NodeState::Processing;

            // Generate one message (or however many remain until we hit the total)
            let remaining = messages_per_cycle - messages_produced;
            let to_generate = remaining.min(1); // Generate 1 message per cycle

            for _ in 0..to_generate {
                let message = Message::new(message_template.clone());
                generated_messages.push(message);
            }

            // Update the counter in the node
            if let NodeType::Producer {
                messages_produced: ref mut counter,
                ..
            } = &mut node.node_type
            {
                *counter += to_generate;
            }
        } else {
            node.state = NodeState::Idle;
        }

        generated_messages
    }

    /// Processes a consumer node by updating its state.
    ///
    /// # Arguments
    ///
    /// * `node` - The consumer node to process
    fn process_consumer_node(&self, node: &mut FlowchartNode) {
        // TODO: Implement message consumption logic
        node.state = NodeState::Idle;
    }

    /// Processes a transformer node by potentially executing its script.
    ///
    /// # Arguments
    ///
    /// * `node` - The transformer node to process
    /// * `script` - The JavaScript script to execute
    fn process_transformer_node(&self, node: &mut FlowchartNode, _script: &String) {
        // TODO: Implement script execution for transformation
        node.state = NodeState::Idle;
    }

    /// Delivers a message to the specified node.
    ///
    /// This method handles message delivery based on the node type:
    /// - Consumers destroy the message
    /// - Transformers execute JavaScript and forward the result
    /// - Producers ignore incoming messages
    ///
    /// # Arguments
    ///
    /// * `node_id` - The ID of the destination node
    /// * `message` - The message to deliver
    /// * `flowchart` - The flowchart containing the destination node
    ///
    /// # Returns
    ///
    /// Returns `Ok(())` if the message was delivered successfully, or an error message if JavaScript execution failed.
    pub fn deliver_message(
        &mut self,
        node_id: NodeId,
        message: Message,
        flowchart: &mut Flowchart,
    ) -> Result<(), String> {
        if let Some(node) = flowchart.nodes.get_mut(&node_id) {
            match &mut node.node_type {
                NodeType::Consumer { .. } => {
                    // Message is consumed and destroyed
                    node.state = NodeState::Processing;
                    Ok(())
                }
                NodeType::Transformer {
                    script,
                    selected_outputs: _,
                    globals,
                    ..
                } => {
                    // Execute JavaScript to transform the message
                    node.state = NodeState::Processing;
                    let script = script.clone();

                    // Execute the transformation script
                    let mut transformed_messages =
                        match execute_transformer_script_with_globals(&script, &message, globals) {
                            Ok(msgs) => msgs,
                            Err(err) => {
                                // Record error state on the node and propagate the error
                                if let Some(n) = flowchart.nodes.get_mut(&node_id) {
                                    n.state = NodeState::Error(err.clone());
                                }
                                return Err(err);
                            }
                        };

                    // Programmatic routing: each output message may include a special "__targets" field
                    // which is an array of destination node names. If absent or null, broadcast to all.
                    for mut transformed_message in transformed_messages.drain(..) {
                        // Extract routing targets from message control field and strip it from payload
                        let mut routing_targets: Option<Vec<String>> = None; // None = broadcast
                        if let serde_json::Value::Object(ref mut map) = transformed_message.data {
                            if map.contains_key("__targets") {
                                let raw =
                                    map.remove("__targets").unwrap_or(serde_json::Value::Null);
                                match raw {
                                    serde_json::Value::Null => {
                                        routing_targets = None; // broadcast
                                    }
                                    serde_json::Value::Array(arr) => {
                                        let mut names = Vec::new();
                                        for v in arr {
                                            if let serde_json::Value::String(s) = v {
                                                names.push(s);
                                            }
                                        }
                                        routing_targets = Some(names);
                                    }
                                    _ => {
                                        // Invalid type -> treat as broadcast
                                        routing_targets = None;
                                    }
                                }
                            }
                        }

                        for connection in flowchart.connections.iter_mut() {
                            if connection.from != node_id {
                                continue;
                            }
                            let send = match &routing_targets {
                                None => true, // broadcast
                                Some(list) => {
                                    if let Some(dest_node) = flowchart.nodes.get(&connection.to) {
                                        list.iter().any(|name| name == &dest_node.name)
                                    } else {
                                        false
                                    }
                                }
                            };
                            if send {
                                connection.messages.push(transformed_message.clone());
                            }
                        }
                    }

                    Ok(())
                }
                NodeType::Producer { .. } => {
                    // Producers don't accept incoming messages
                    Ok(())
                }
            }
        } else {
            Err(format!("Node {} not found", node_id))
        }
    }
}

/// Executes a JavaScript transformation script on an input message.
///
/// This function uses the cross-platform script engine to execute JavaScript code,
/// providing the input message and returning the transformed messages.
///
/// # Arguments
///
/// * `script` - The JavaScript script code to execute
/// * `input_message` - The message to transform
///
/// # Returns
///
/// A vector of output messages or an error string if script execution fails.
///
/// # Example JavaScript Script
///
/// ```javascript
/// // Access input message via global 'input'
/// const new_data = {
///     original: input.data,
///     transformed: true,
///     timestamp: Date.now()
/// };
///
/// // Return transformed message(s)
/// new_data;
/// ```
pub fn execute_transformer_script_with_globals(
    script: &str,
    input_message: &Message,
    globals: &mut serde_json::Map<String, serde_json::Value>,
) -> Result<Vec<Message>, String> {
    let mut script_engine =
        create_script_engine().map_err(|e| format!("Failed to create script engine: {}", e))?;

    // Provide per-node persistent globals as `globalThis.state`
    let state_value = serde_json::Value::Object(globals.clone());
    script_engine
        .set_global_json("state", &state_value)
        .map_err(|e| format!("Failed to set state: {}", e))?;

    // Create input JSON for the script
    let input_json = serde_json::json!(input_message.data);

    // Execute the script to define the transform function
    script_engine
        .execute_script(script)
        .map_err(|e| format!("Failed to execute script: {}", e))?;

    // Call the transform function with the input
    let result = script_engine
        .call_function("transform", input_json)
        .map_err(|e| format!("Failed to call transform function: {}", e))?;

    // Read back potentially mutated state and persist it for the node
    if let Ok(new_state) = script_engine.get_global_json("state") {
        if let serde_json::Value::Object(obj) = new_state {
            *globals = obj;
        }
    }

    // Create a new message with the transformed data
    let transformed_message = Message {
        id: uuid::Uuid::new_v4(),
        data: result,
    };

    Ok(vec![transformed_message])
}

// Helper functions removed - now handled by the script engine

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_simulation_engine_creation() {
        let engine = SimulationEngine::new();
        // Engine should be created successfully
        assert!(engine.script_engine.is_some()); // Basic creation test
    }

    #[test]
    fn test_empty_flowchart_step() {
        let mut engine = SimulationEngine::new();
        let mut flowchart = Flowchart::new();

        let delivered = engine.step(&mut flowchart);
        assert!(delivered.is_empty());
    }

    #[test]
    fn test_message_script_execution() {
        let script = r#"
            // Simple transformation script
            function transform(input) {
                return {transformed: true, value: 42};
            }
        "#;

        let input = Message::new(json!({"original": "data"}));
        let mut globals = Default::default();
        let result = execute_transformer_script_with_globals(script, &input, &mut globals).unwrap();

        assert_eq!(result.len(), 1);
        let transformed_message = &result[0];

        // Verify the transformed data contains our expected values
        assert_eq!(
            transformed_message.data.get("transformed"),
            Some(&json!(true))
        );
        assert_eq!(transformed_message.data.get("value"), Some(&json!(42)));
    }

    #[test]
    fn test_transformer_returns_object() {
        // Test that returning a JavaScript object properly converts to JSON
        let script = r#"
            function transform(input) {
                return {
                    status: "processed",
                    count: 123,
                    nested: {
                        field: "value"
                    }
                };
            }
        "#;

        let input = Message::new(json!({"input": "test"}));
        let result = execute_transformer_script(script, &input).unwrap();

        assert_eq!(result.len(), 1);
        let msg = &result[0];

        // Verify object structure is preserved
        assert_eq!(msg.data.get("status"), Some(&json!("processed")));
        assert_eq!(msg.data.get("count"), Some(&json!(123)));

        // Verify nested object
        let nested = msg.data.get("nested").expect("nested object should exist");
        assert_eq!(nested.get("field"), Some(&json!("value")));
    }

    #[test]
    fn test_transformer_accesses_input() {
        // Test that the script can access the input message data
        let script = r#"
            function transform(input) {
                return {
                    original: input.value,
                    doubled: input.value * 2
                };
            }
        "#;

        let input = Message::new(json!({"value": 21}));
        let result = execute_transformer_script(script, &input).unwrap();

        assert_eq!(result.len(), 1);
        let msg = &result[0];

        assert_eq!(msg.data.get("original"), Some(&json!(21)));
        assert_eq!(msg.data.get("doubled"), Some(&json!(42)));
    }

    #[test]
    fn test_transformer_returns_array() {
        // Test that returning an array works correctly
        let script = r#"
            function transform(input) {
                return [1, 2, 3];
            }
        "#;

        let input = Message::new(json!({}));
        let result = execute_transformer_script(script, &input).unwrap();

        assert_eq!(result.len(), 1);
        let msg = &result[0];

        // Should have an array in the data field
        assert!(msg.data.is_array());
        assert_eq!(msg.data.as_array().unwrap().len(), 3);
    }

    #[test]
    fn test_transformer_returns_string() {
        // Test that returning a string works correctly
        let script = r#"
            function transform(input) {
                return "hello world";
            }
        "#;

        let input = Message::new(json!({}));
        let result = execute_transformer_script(script, &input).unwrap();

        assert_eq!(result.len(), 1);
        let msg = &result[0];

        assert_eq!(msg.data, json!("hello world"));
    }

    #[test]
    fn test_transformer_returns_number() {
        // Test that returning a number works correctly
        let script = r#"
            function transform(input) {
                return 42;
            }
        "#;

        let input = Message::new(json!({}));
        let result = execute_transformer_script(script, &input).unwrap();

        assert_eq!(result.len(), 1);
        let msg = &result[0];

        assert_eq!(msg.data, json!(42));
    }

    #[test]
    fn test_transformer_complex_transformation() {
        // Test a more complex transformation
        let script = r#"
            function transform(input) {
                const result = {
                    timestamp: Date.now(),
                    items: input.items.map(x => x * 2),
                    metadata: {
                        processed: true,
                        originalCount: input.items.length
                    }
                };
                return result;
            }
        "#;

        let input = Message::new(json!({
            "items": [1, 2, 3, 4, 5]
        }));
        let result = execute_transformer_script(script, &input).unwrap();

        assert_eq!(result.len(), 1);
        let msg = &result[0];

        // Verify transformed array
        let items = msg.data.get("items").expect("items should exist");
        assert_eq!(items.as_array().unwrap().len(), 5);
        assert_eq!(items.get(0), Some(&json!(2)));
        assert_eq!(items.get(1), Some(&json!(4)));

        // Verify metadata
        let metadata = msg.data.get("metadata").expect("metadata should exist");
        assert_eq!(metadata.get("processed"), Some(&json!(true)));
        assert_eq!(metadata.get("originalCount"), Some(&json!(5)));
    }

    #[test]
    fn test_transformer_error_handling() {
        // Test that script errors are properly caught
        let script = r#"
            function transform(input) {
                throw new Error("Test error");
            }
        "#;

        let input = Message::new(json!({}));
        let result = execute_transformer_script(script, &input);

        assert!(result.is_err());
        let error_result = result.unwrap_err();
        assert!(
            error_result.contains("Function call failed") || error_result.contains("Test error")
        );
    }

    #[test]
    fn test_transformer_missing_function() {
        // Test that a missing transform function is caught
        let script = r#"
            // Script doesn't define transform function
            const x = 42;
        "#;

        let input = Message::new(json!({}));
        let result = execute_transformer_script(script, &input);

        assert!(result.is_err());
        assert!(result.unwrap_err().contains("not a function"));
    }

    #[test]
    fn test_transformer_node_delivery() {
        // Test the complete flow of delivering a message to a transformer node
        let mut engine = SimulationEngine::new();
        let mut flowchart = Flowchart::new();

        // Create a transformer node
        let transformer = FlowchartNode::new(
            "Transformer".to_string(),
            (0.0, 0.0),
            NodeType::Transformer {
                script: r#"
                    function transform(input) {
                        return {
                            value: input.x + 10,
                            processed: true
                        };
                    }
                "#
                .to_string(),
                selected_outputs: None,
                globals: Default::default(),
                initial_globals: Default::default(),
            },
        );
        let transformer_id = transformer.id;
        flowchart.add_node(transformer);

        // Create a consumer node to receive the output
        let consumer = FlowchartNode::new(
            "Consumer".to_string(),
            (100.0, 0.0),
            NodeType::Consumer {
                consumption_rate: 1,
            },
        );
        let consumer_id = consumer.id;
        flowchart.add_node(consumer);

        // Connect transformer to consumer
        flowchart
            .add_connection(transformer_id, consumer_id)
            .unwrap();

        // Deliver a message to the transformer
        let input_message = Message::new(json!({"x": 5}));
        let result = engine.deliver_message(transformer_id, input_message, &mut flowchart);

        assert!(result.is_ok());

        // Check that the transformer created a message on the outgoing connection
        let connection = flowchart
            .connections
            .iter()
            .find(|c| c.from == transformer_id)
            .expect("Connection should exist");

        assert_eq!(connection.messages.len(), 1);
        let output_message = &connection.messages[0];

        // Verify the transformation happened correctly
        assert_eq!(output_message.data.get("value"), Some(&json!(15)));
        assert_eq!(output_message.data.get("processed"), Some(&json!(true)));
    }

    #[test]
    fn test_transformer_node_empty_result() {
        // Test that returning null/undefined doesn't crash
        let mut engine = SimulationEngine::new();
        let mut flowchart = Flowchart::new();

        let transformer = FlowchartNode::new(
            "Transformer".to_string(),
            (0.0, 0.0),
            NodeType::Transformer {
                script: r#"
                    function transform(input) {
                        return null;
                    }
                "#
                .to_string(),
                selected_outputs: None,
                globals: Default::default(),
                initial_globals: Default::default(),
            },
        );
        let transformer_id = transformer.id;
        flowchart.add_node(transformer);

        let input_message = Message::new(json!({"test": "data"}));
        let result = engine.deliver_message(transformer_id, input_message, &mut flowchart);

        // Should succeed, producing a message with null data
        assert!(result.is_ok());
    }
}

#[cfg(test)]
mod programmatic_routing_tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_transformer_programmatic_routing_with_targets() {
        let mut engine = SimulationEngine::new();
        let mut flowchart = Flowchart::new();

        // Create a transformer node that routes to a specific consumer based on __targets
        let transformer = FlowchartNode::new(
            "Router".to_string(),
            (0.0, 0.0),
            NodeType::Transformer {
                script: r#"
                    function transform(input) {
                        // Route only to node named 'C2'
                        return { value: input.x * 2, __targets: ["C2"] };
                    }
                "#
                .to_string(),
                selected_outputs: None,
                globals: Default::default(),
                initial_globals: Default::default(),
            },
        );
        let transformer_id = transformer.id;
        flowchart.add_node(transformer);

        // Two consumers
        let c1 = FlowchartNode::new(
            "C1".to_string(),
            (150.0, 0.0),
            NodeType::Consumer {
                consumption_rate: 1,
            },
        );
        let c1_id = c1.id;
        flowchart.add_node(c1);

        let c2 = FlowchartNode::new(
            "C2".to_string(),
            (150.0, 100.0),
            NodeType::Consumer {
                consumption_rate: 1,
            },
        );
        let c2_id = c2.id;
        flowchart.add_node(c2);

        // Connect transformer to both consumers
        flowchart.add_connection(transformer_id, c1_id).unwrap();
        flowchart.add_connection(transformer_id, c2_id).unwrap();

        // Deliver a message to the transformer
        let input = Message::new(json!({"x": 7}));
        let res = engine.deliver_message(transformer_id, input, &mut flowchart);
        assert!(res.is_ok());

        // Only the connection to C2 should have a message
        let to_c1 = flowchart
            .connections
            .iter()
            .find(|c| c.from == transformer_id && c.to == c1_id)
            .expect("connection to C1");
        let to_c2 = flowchart
            .connections
            .iter()
            .find(|c| c.from == transformer_id && c.to == c2_id)
            .expect("connection to C2");

        assert_eq!(to_c1.messages.len(), 0);
        assert_eq!(to_c2.messages.len(), 1);

        // Ensure the control field was stripped from payload
        let payload = &to_c2.messages[0].data;
        assert_eq!(payload.get("value"), Some(&json!(14)));
        assert!(payload.get("__targets").is_none());
    }
}


/// Backward-compatible wrapper that executes a transformer script without exposing globals.
/// Internally uses an empty `globals` map.
pub fn execute_transformer_script(
    script: &str,
    input_message: &Message,
) -> Result<Vec<Message>, String> {
    let mut globals: serde_json::Map<String, serde_json::Value> = Default::default();
    execute_transformer_script_with_globals(script, input_message, &mut globals)
}
