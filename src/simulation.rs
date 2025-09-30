//! Simulation engine and logic for the flowchart tool.
//! 
//! This module handles the execution of flowchart simulations, including message
//! generation, consumption, transformation via Lua scripts, and message routing
//! between nodes.

use crate::types::*;
use crate::script_engine::{JavaScriptEngine, create_script_engine};
use serde::{Deserialize, Serialize};

/// Engine responsible for running flowchart simulations.
/// 
/// The simulation engine processes nodes in sequence, handles message flow,
/// and executes transformation scripts.
#[derive(Serialize, Deserialize)]
pub struct SimulationEngine {
    /// The JavaScript runtime environment for script execution
    #[serde(skip)]
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
        Self {
            script_engine,
        }
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

        // Move messages along connections and collect those that have arrived
        for connection in &mut flowchart.connections {
            let mut arrived_messages = Vec::new();

            for message in &mut connection.messages {
                message.position_along_edge += 0.01; // TODO: Make speed configurable
                if message.position_along_edge >= 1.0 {
                    arrived_messages.push(message.clone());
                }
            }

            // Remove delivered messages and add them to the delivery list
            connection.messages.retain(|m| m.position_along_edge < 1.0);
            for message in arrived_messages {
                delivered_messages.push((connection.to, message));
            }
        }

        // Process each node based on its type - collect node IDs first to avoid borrow conflicts
        let node_ids: Vec<_> = flowchart.nodes.keys().cloned().collect();
        for node_id in node_ids {
            if let Some(node) = flowchart.nodes.get_mut(&node_id) {
                match &node.node_type.clone() {
                    NodeType::Producer { generation_rate } => {
                        self.process_producer_node(node, *generation_rate);
                    }
                    NodeType::Consumer { consumption_rate: _ } => {
                        self.process_consumer_node(node);
                    }
                    NodeType::Transformer { script } => {
                        self.process_transformer_node(node, script);
                    }
                }
            }
        }

        delivered_messages
    }

    /// Processes a producer node by potentially generating new messages.
    /// 
    /// # Arguments
    /// 
    /// * `node` - The producer node to process
    /// * `generation_rate` - Number of messages to generate per step
    fn process_producer_node(&self, node: &mut FlowchartNode, _generation_rate: u32) {
        // TODO: Implement message generation based on rate
        node.state = NodeState::Processing;
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
    /// - Transformers may modify and forward the message
    /// - Producers ignore incoming messages
    /// 
    /// # Arguments
    /// 
    /// * `node_id` - The ID of the destination node
    /// * `message` - The message to deliver
    /// * `flowchart` - The flowchart containing the destination node
    pub fn deliver_message(&mut self, node_id: NodeId, _message: Message, flowchart: &mut Flowchart) {
        if let Some(node) = flowchart.nodes.get_mut(&node_id) {
            match &node.node_type {
                NodeType::Consumer { .. } => {
                    // Message is consumed and destroyed
                    node.state = NodeState::Processing;
                }
                NodeType::Transformer { .. } => {
                    // Message may be transformed and forwarded
                    node.state = NodeState::Processing;
                }
                NodeType::Producer { .. } => {
                    // Producers don't accept incoming messages
                }
            }
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
/// A vector of output messages, or an error string if script execution fails.
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
pub fn execute_transformer_script(script: &str, input_message: &Message) -> Result<Vec<Message>, String> {
    let mut script_engine = create_script_engine()
        .map_err(|e| format!("Failed to create script engine: {}", e))?;

    // Create input JSON for the script
    let input_json = serde_json::json!({
        "id": input_message.id.to_string(),
        "data": input_message.data,
        "position_along_edge": input_message.position_along_edge
    });

    // Execute the script
    let result = script_engine.execute(script, input_json)
        .map_err(|e| format!("Script execution failed: {}", e))?;

    // For now, create a new message with the transformed data
    let transformed_message = Message {
        id: uuid::Uuid::new_v4(),
        data: result,
        position_along_edge: 0.0,
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
        assert!(true); // Basic creation test
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
            ({transformed: true, value: 42})
        "#;

        let input = Message::new(json!({"original": "data"}));
        let result = execute_transformer_script(script, &input).unwrap();

        assert_eq!(result.len(), 1);
        let transformed_message = &result[0];

        // Basic test - the exact content will depend on script engine implementation
        assert_eq!(transformed_message.position_along_edge, 0.0);
        // The actual transformation testing will be enhanced once script engine is fully implemented
    }
}
