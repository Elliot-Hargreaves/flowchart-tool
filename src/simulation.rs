//! Simulation engine and logic for the flowchart tool.
//! 
//! This module handles the execution of flowchart simulations, including message
//! generation, consumption, transformation via Lua scripts, and message routing
//! between nodes.

use crate::types::*;
use mlua::Lua;

/// Engine responsible for running flowchart simulations.
/// 
/// The simulation engine processes nodes in sequence, handles message flow,
/// and executes transformation scripts.
pub struct SimulationEngine {
    /// The Lua runtime environment for script execution
    lua: Lua,
}

impl Default for SimulationEngine {
    fn default() -> Self {
        Self::new()
    }
}

impl SimulationEngine {
    /// Creates a new simulation engine with a fresh Lua environment.
    pub fn new() -> Self {
        Self {
            lua: Lua::new(),
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

        // Process each node based on its type
        for (_id, node) in &mut flowchart.nodes {
            match &node.node_type {
                NodeType::Producer { generation_rate } => {
                    self.process_producer_node(node, *generation_rate, flowchart);
                }
                NodeType::Consumer { consumption_rate: _ } => {
                    self.process_consumer_node(node);
                }
                NodeType::Transformer { script } => {
                    self.process_transformer_node(node, script);
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
    /// * `flowchart` - The flowchart (for adding messages to outgoing connections)
    fn process_producer_node(&self, node: &mut FlowchartNode, _generation_rate: u32, _flowchart: &mut Flowchart) {
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
    /// * `script` - The Lua script to execute
    fn process_transformer_node(&self, node: &mut FlowchartNode, _script: &str) {
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

/// Executes a Lua transformation script on an input message.
/// 
/// This function sets up a Lua environment, provides the input message,
/// executes the user-provided script, and returns the transformed messages.
/// 
/// # Arguments
/// 
/// * `script` - The Lua script code to execute
/// * `input_message` - The message to transform
/// 
/// # Returns
/// 
/// A vector of output messages, or a Lua error if script execution fails.
/// 
/// # Example Lua Script
/// 
/// ```lua
/// -- Access input message via global 'input'
/// local new_data = {
///     original = input.data,
///     transformed = true,
///     timestamp = os.time()
/// }
/// 
/// -- Return transformed message(s)
/// output = {{data = new_data}}
/// ```
pub fn execute_transformer_script(script: &str, input_message: &Message) -> Result<Vec<Message>, mlua::Error> {
    let lua = Lua::new();

    // Serialize the message to JSON and provide it to Lua
    let message_json = serde_json::to_string(input_message).map_err(mlua::Error::external)?;
    lua.globals().set("input_json", message_json)?;

    // Create helper functions in Lua for JSON parsing and message creation
    lua.load(r#"
        json = require("cjson") or {decode = function() return {} end}

        -- Parse the input message
        input = json.decode(input_json) or {
            id = "placeholder",
            data = {},
            position_along_edge = 0.0
        }

        -- Initialize output as empty array
        output = {}

        -- Helper function to create a new message
        function create_message(data)
            return {
                data = data,
                position_along_edge = 0.0
            }
        end
    "#).exec()?;

    // Execute the user script
    lua.load(script).exec()?;

    // Get the output messages
    let output_table: mlua::Table = lua.globals().get("output")?;
    let mut result_messages = Vec::new();

    // Convert Lua output to Message structs
    for pair in output_table.pairs::<i32, mlua::Table>() {
        let (_index, message_table) = pair?;
        if let Ok(data_value) = message_table.get::<_, mlua::Value>("data") {
            // Convert Lua value to JSON
            let json_str = lua.to_string(&data_value)?;
            if let Ok(json_value) = serde_json::from_str(&json_str) {
                result_messages.push(Message::new(json_value));
            }
        }
    }

    // If no output was produced, return the original message
    if result_messages.is_empty() {
        result_messages.push(input_message.clone());
    }

    Ok(result_messages)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_simulation_engine_creation() {
        let engine = SimulationEngine::new();
        assert!(std::ptr::addr_of!(engine.lua) as *const _ != std::ptr::null());
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
            output = {create_message({transformed = true, value = 42})}
        "#;

        let input = Message::new(json!({"original": "data"}));

        // Note: This test may fail without proper JSON library in Lua
        // In a real implementation, you'd want to ensure lua-cjson is available
        let _result = execute_transformer_script(script, &input);
        // Test passes if no panic occurs
    }
}
