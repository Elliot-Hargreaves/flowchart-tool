//! Core data types and structures for the flowchart tool.
//!
//! This module defines all the fundamental data structures used throughout the application,
//! including nodes, connections, messages, and the main flowchart structure.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use uuid::Uuid;

/// Unique identifier for flowchart nodes.
pub type NodeId = Uuid;

/// Unique identifier for messages flowing through the system.
pub type MessageId = Uuid;

/// Represents the current state of a flowchart node during simulation.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum NodeState {
    /// Node is idle and not processing anything
    Idle,
    /// Node is currently processing a message
    Processing,
    /// Node has encountered an error with the given message
    Error(String),
}

/// Represents the overall state of the flowchart simulation.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum SimulationState {
    /// Simulation is stopped
    Stopped,
    /// Simulation is running continuously
    Running,
    /// Simulation is temporarily paused
    Paused,
}

/// Defines the different types of nodes available in the flowchart.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum NodeType {
    /// A node that generates messages at a specified rate
    Producer {
        /// JSON template for the message data to produce
        message_template: serde_json::Value,
        /// Which step to start producing messages on (0-based)
        start_step: u64,
        /// Total number of messages to generate across all cycles
        messages_per_cycle: u32,
        /// Number of steps to wait between production cycles
        steps_between_cycles: u32,
        /// Counter tracking how many messages have been produced so far
        #[serde(default)]
        messages_produced: u32,
    },
    /// A node that consumes and destroys incoming messages
    Consumer {
        /// Maximum number of messages to consume per simulation step
        consumption_rate: u32,
    },
    /// A node that transforms messages using JavaScript
    Transformer {
        /// JavaScript script code for message transformation
        script: String,
        /// Optional list of destination node names to send to; None means broadcast to all
        #[serde(default)]
        selected_outputs: Option<Vec<String>>,
        /// Persistent per-node global state exposed to scripts as `globalThis.state`
        #[serde(default)]
        globals: serde_json::Map<String, serde_json::Value>,
        /// Initial values for globals; used to reset on Stop
        #[serde(default)]
        initial_globals: serde_json::Map<String, serde_json::Value>,
    },
}

/// Represents a single node in the flowchart.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FlowchartNode {
    /// Unique identifier for this node
    pub id: NodeId,
    /// User-displayable name of the node
    pub name: String,
    /// Position on the canvas as (x, y) coordinates
    pub position: (f32, f32),
    /// The type and configuration of this node
    pub node_type: NodeType,
    /// Current processing state of the node
    pub state: NodeState,
}

impl FlowchartNode {
    /// Creates a new flowchart node with the given parameters.
    ///
    /// # Arguments
    ///
    /// * `name` - The display name for the node
    /// * `position` - The (x, y) position on the canvas
    /// * `node_type` - The type and configuration of the node
    ///
    /// # Returns
    ///
    /// A new `FlowchartNode` with a unique ID and idle state.
    pub fn new(name: String, position: (f32, f32), node_type: NodeType) -> Self {
        Self {
            id: Uuid::new_v4(),
            name,
            position,
            node_type,
            state: NodeState::Idle,
        }
    }
}

/// Represents a directional connection between two nodes.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Connection {
    /// ID of the source node
    pub from: NodeId,
    /// ID of the destination node  
    pub to: NodeId,
    /// Messages currently traveling along this connection
    pub messages: Vec<Message>,
}

impl Connection {
    /// Creates a new connection between two nodes.
    ///
    /// # Arguments
    ///
    /// * `from` - The ID of the source node
    /// * `to` - The ID of the destination node
    ///
    /// # Returns
    ///
    /// A new empty connection between the specified nodes.
    pub fn new(from: NodeId, to: NodeId) -> Self {
        Self {
            from,
            to,
            messages: Vec::new(),
        }
    }
}

/// Represents a message flowing through the flowchart system.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Message {
    /// Unique identifier for this message
    pub id: MessageId,
    /// The data payload of the message
    pub data: serde_json::Value,
}

impl Message {
    /// Creates a new message with the given data payload.
    ///
    /// # Arguments
    ///
    /// * `data` - JSON data to be carried by this message
    ///
    /// # Returns
    ///
    /// A new message with a unique ID, positioned at the start of any connection.
    pub fn new(data: serde_json::Value) -> Self {
        Self {
            id: Uuid::new_v4(),
            data,
        }
    }
}

/// The main flowchart structure containing all nodes, connections, and simulation state.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Flowchart {
    /// Map of all nodes in the flowchart, indexed by their ID
    pub nodes: HashMap<NodeId, FlowchartNode>,
    /// List of all connections between nodes
    pub connections: Vec<Connection>,
    /// Current state of the simulation
    pub simulation_state: SimulationState,
    /// Current simulation step counter
    pub current_step: u64,
}

impl Default for Flowchart {
    /// Creates a new empty flowchart with no nodes or connections.
    fn default() -> Self {
        Self {
            nodes: HashMap::new(),
            connections: Vec::new(),
            simulation_state: SimulationState::Stopped,
            current_step: 0,
        }
    }
}

impl Flowchart {
    /// Creates a new empty flowchart.
    pub fn new() -> Self {
        Self::default()
    }

    /// Serialize the flowchart to JSON string
    pub fn to_json(&self) -> Result<String, serde_json::Error> {
        serde_json::to_string_pretty(self)
    }

    /// Deserialize a flowchart from JSON string
    pub fn from_json(json: &str) -> Result<Self, serde_json::Error> {
        serde_json::from_str(json)
    }

    /// Adds a node to the flowchart.
    ///
    /// # Arguments
    ///
    /// * `node` - The node to add to the flowchart
    ///
    /// # Returns
    ///
    /// The ID of the newly added node.
    pub fn add_node(&mut self, node: FlowchartNode) -> NodeId {
        let id = node.id;
        self.nodes.insert(id, node);
        id
    }

    /// Adds a connection between two existing nodes.
    ///
    /// # Arguments
    ///
    /// * `from` - The ID of the source node
    /// * `to` - The ID of the destination node
    ///
    /// # Returns
    ///
    /// `Ok(())` if the connection was added successfully, or an error message if either node doesn't exist.
    pub fn add_connection(&mut self, from: NodeId, to: NodeId) -> Result<(), String> {
        if !self.nodes.contains_key(&from) {
            return Err("Source node does not exist".to_string());
        }
        if !self.nodes.contains_key(&to) {
            return Err("Destination node does not exist".to_string());
        }

        self.connections.push(Connection::new(from, to));
        Ok(())
    }

    /// Removes a node and all its associated connections from the flowchart.
    ///
    /// # Arguments
    ///
    /// * `node_id` - The ID of the node to remove
    ///
    /// # Returns
    ///
    /// `true` if the node was found and removed, `false` if the node didn't exist.
    pub fn remove_node(&mut self, node_id: &NodeId) -> bool {
        let removed = self.nodes.remove(node_id).is_some();
        if removed {
            // Remove all connections involving this node
            self.connections
                .retain(|conn| conn.from != *node_id && conn.to != *node_id);
        }
        removed
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_node_creation() {
        let node = FlowchartNode::new(
            "Test Node".to_string(),
            (100.0, 200.0),
            NodeType::Consumer {
                consumption_rate: 5,
            },
        );

        assert_eq!(node.name, "Test Node");
        assert_eq!(node.position, (100.0, 200.0));
        assert_eq!(node.state, NodeState::Idle);
        assert!(!node.id.is_nil());
    }

    #[test]
    fn test_producer_node_creation() {
        let template = json!({"type": "test", "value": 42});
        let node = FlowchartNode::new(
            "Producer".to_string(),
            (0.0, 0.0),
            NodeType::Producer {
                message_template: template.clone(),
                start_step: 10,
                messages_per_cycle: 3,
                steps_between_cycles: 5,
                messages_produced: 0,
            },
        );

        if let NodeType::Producer {
            message_template,
            start_step,
            messages_per_cycle,
            ..
        } = &node.node_type
        {
            assert_eq!(*message_template, template);
            assert_eq!(*start_step, 10);
            assert_eq!(*messages_per_cycle, 3);
        } else {
            panic!("Expected Producer node type");
        }
    }

    #[test]
    fn test_transformer_node_creation() {
        let script = "return message * 2".to_string();
        let node = FlowchartNode::new(
            "Transformer".to_string(),
            (50.0, 50.0),
            NodeType::Transformer {
                script: script.clone(),
                selected_outputs: None,
                globals: Default::default(),
                initial_globals: Default::default(),
            },
        );

        if let NodeType::Transformer {
            script: node_script,
            ..
        } = &node.node_type
        {
            assert_eq!(*node_script, script);
        } else {
            panic!("Expected Transformer node type");
        }
    }

    #[test]
    fn test_message_creation() {
        let data = json!({"key": "value", "number": 123});
        let message = Message::new(data.clone());

        assert_eq!(message.data, data);
        assert!(!message.id.is_nil());
    }

    #[test]
    fn test_connection_creation() {
        let from_id = Uuid::new_v4();
        let to_id = Uuid::new_v4();
        let connection = Connection::new(from_id, to_id);

        assert_eq!(connection.from, from_id);
        assert_eq!(connection.to, to_id);
        assert!(connection.messages.is_empty());
    }

    #[test]
    fn test_flowchart_default() {
        let flowchart = Flowchart::default();

        assert!(flowchart.nodes.is_empty());
        assert!(flowchart.connections.is_empty());
        assert_eq!(flowchart.simulation_state, SimulationState::Stopped);
        assert_eq!(flowchart.current_step, 0);
    }

    #[test]
    fn test_flowchart_add_node() {
        let mut flowchart = Flowchart::new();
        let node = FlowchartNode::new(
            "Test".to_string(),
            (0.0, 0.0),
            NodeType::Consumer {
                consumption_rate: 1,
            },
        );
        let node_id = node.id;

        let added_id = flowchart.add_node(node);

        assert_eq!(added_id, node_id);
        assert_eq!(flowchart.nodes.len(), 1);
        assert!(flowchart.nodes.contains_key(&node_id));
    }

    #[test]
    fn test_flowchart_add_multiple_nodes() {
        let mut flowchart = Flowchart::new();

        let node1 = FlowchartNode::new(
            "Node1".to_string(),
            (0.0, 0.0),
            NodeType::Producer {
                message_template: json!({}),
                start_step: 0,
                messages_per_cycle: 1,
                steps_between_cycles: 1,
                messages_produced: 0,
            },
        );
        let node2 = FlowchartNode::new(
            "Node2".to_string(),
            (100.0, 0.0),
            NodeType::Consumer {
                consumption_rate: 1,
            },
        );

        flowchart.add_node(node1);
        flowchart.add_node(node2);

        assert_eq!(flowchart.nodes.len(), 2);
    }

    #[test]
    fn test_flowchart_add_connection_success() {
        let mut flowchart = Flowchart::new();

        let node1 = FlowchartNode::new(
            "Node1".to_string(),
            (0.0, 0.0),
            NodeType::Producer {
                message_template: json!({}),
                start_step: 0,
                messages_per_cycle: 1,
                steps_between_cycles: 1,
                messages_produced: 0,
            },
        );
        let node2 = FlowchartNode::new(
            "Node2".to_string(),
            (100.0, 0.0),
            NodeType::Consumer {
                consumption_rate: 1,
            },
        );

        let id1 = flowchart.add_node(node1);
        let id2 = flowchart.add_node(node2);

        let result = flowchart.add_connection(id1, id2);

        assert!(result.is_ok());
        assert_eq!(flowchart.connections.len(), 1);
        assert_eq!(flowchart.connections[0].from, id1);
        assert_eq!(flowchart.connections[0].to, id2);
    }

    #[test]
    fn test_flowchart_add_connection_invalid_source() {
        let mut flowchart = Flowchart::new();

        let node = FlowchartNode::new(
            "Node".to_string(),
            (0.0, 0.0),
            NodeType::Consumer {
                consumption_rate: 1,
            },
        );
        let id = flowchart.add_node(node);
        let invalid_id = Uuid::new_v4();

        let result = flowchart.add_connection(invalid_id, id);

        assert!(result.is_err());
        assert_eq!(result.unwrap_err(), "Source node does not exist");
        assert!(flowchart.connections.is_empty());
    }

    #[test]
    fn test_flowchart_add_connection_invalid_destination() {
        let mut flowchart = Flowchart::new();

        let node = FlowchartNode::new(
            "Node".to_string(),
            (0.0, 0.0),
            NodeType::Producer {
                message_template: json!({}),
                start_step: 0,
                messages_per_cycle: 1,
                steps_between_cycles: 1,
                messages_produced: 0,
            },
        );
        let id = flowchart.add_node(node);
        let invalid_id = Uuid::new_v4();

        let result = flowchart.add_connection(id, invalid_id);

        assert!(result.is_err());
        assert_eq!(result.unwrap_err(), "Destination node does not exist");
        assert!(flowchart.connections.is_empty());
    }

    #[test]
    fn test_flowchart_remove_node() {
        let mut flowchart = Flowchart::new();

        let node = FlowchartNode::new(
            "Test".to_string(),
            (0.0, 0.0),
            NodeType::Consumer {
                consumption_rate: 1,
            },
        );
        let node_id = flowchart.add_node(node);

        let removed = flowchart.remove_node(&node_id);

        assert!(removed);
        assert!(flowchart.nodes.is_empty());
    }

    #[test]
    fn test_flowchart_remove_nonexistent_node() {
        let mut flowchart = Flowchart::new();
        let fake_id = Uuid::new_v4();

        let removed = flowchart.remove_node(&fake_id);

        assert!(!removed);
    }

    #[test]
    fn test_flowchart_remove_node_removes_connections() {
        let mut flowchart = Flowchart::new();

        let node1 = FlowchartNode::new(
            "Node1".to_string(),
            (0.0, 0.0),
            NodeType::Producer {
                message_template: json!({}),
                start_step: 0,
                messages_per_cycle: 1,
                steps_between_cycles: 1,
                messages_produced: 0,
            },
        );
        let node2 = FlowchartNode::new(
            "Node2".to_string(),
            (100.0, 0.0),
            NodeType::Consumer {
                consumption_rate: 1,
            },
        );
        let node3 = FlowchartNode::new(
            "Node3".to_string(),
            (200.0, 0.0),
            NodeType::Consumer {
                consumption_rate: 1,
            },
        );

        let id1 = flowchart.add_node(node1);
        let id2 = flowchart.add_node(node2);
        let id3 = flowchart.add_node(node3);

        flowchart.add_connection(id1, id2).unwrap();
        flowchart.add_connection(id2, id3).unwrap();
        flowchart.add_connection(id1, id3).unwrap();

        assert_eq!(flowchart.connections.len(), 3);

        flowchart.remove_node(&id2);

        assert_eq!(flowchart.connections.len(), 1);
        assert_eq!(flowchart.connections[0].from, id1);
        assert_eq!(flowchart.connections[0].to, id3);
    }

    #[test]
    fn test_flowchart_serialization() {
        let mut flowchart = Flowchart::new();

        let node = FlowchartNode::new(
            "Test Node".to_string(),
            (50.0, 100.0),
            NodeType::Consumer {
                consumption_rate: 5,
            },
        );
        flowchart.add_node(node);

        let json = flowchart.to_json();
        assert!(json.is_ok());

        let json_str = json.unwrap();
        assert!(json_str.contains("Test Node"));
        assert!(json_str.contains("50.0"));
        assert!(json_str.contains("100.0"));
    }

    #[test]
    fn test_flowchart_deserialization() {
        let mut original = Flowchart::new();
        let node = FlowchartNode::new(
            "Test Node".to_string(),
            (50.0, 100.0),
            NodeType::Consumer {
                consumption_rate: 5,
            },
        );
        let node_id = original.add_node(node);

        let json_str = original.to_json().unwrap();
        let deserialized = Flowchart::from_json(&json_str);

        assert!(deserialized.is_ok());
        let flowchart = deserialized.unwrap();
        assert_eq!(flowchart.nodes.len(), 1);
        assert!(flowchart.nodes.contains_key(&node_id));
        assert_eq!(flowchart.nodes[&node_id].name, "Test Node");
        assert_eq!(flowchart.nodes[&node_id].position, (50.0, 100.0));
    }

    #[test]
    fn test_flowchart_roundtrip_serialization() {
        let mut original = Flowchart::new();

        let node1 = FlowchartNode::new(
            "Producer".to_string(),
            (0.0, 0.0),
            NodeType::Producer {
                message_template: json!({"test": "data"}),
                start_step: 5,
                messages_per_cycle: 3,
                steps_between_cycles: 10,
                messages_produced: 0,
            },
        );
        let node2 = FlowchartNode::new(
            "Consumer".to_string(),
            (200.0, 100.0),
            NodeType::Consumer {
                consumption_rate: 2,
            },
        );

        let id1 = original.add_node(node1);
        let id2 = original.add_node(node2);
        original.add_connection(id1, id2).unwrap();

        let json = original.to_json().unwrap();
        let restored = Flowchart::from_json(&json).unwrap();

        assert_eq!(restored.nodes.len(), 2);
        assert_eq!(restored.connections.len(), 1);
        assert_eq!(restored.connections[0].from, id1);
        assert_eq!(restored.connections[0].to, id2);
    }

    #[test]
    fn test_node_state_transitions() {
        let mut node = FlowchartNode::new(
            "Test".to_string(),
            (0.0, 0.0),
            NodeType::Consumer {
                consumption_rate: 1,
            },
        );

        assert_eq!(node.state, NodeState::Idle);

        node.state = NodeState::Processing;
        assert_eq!(node.state, NodeState::Processing);

        node.state = NodeState::Error("Test error".to_string());
        if let NodeState::Error(msg) = &node.state {
            assert_eq!(msg, "Test error");
        } else {
            panic!("Expected Error state");
        }
    }

    #[test]
    fn test_simulation_state_transitions() {
        let mut flowchart = Flowchart::new();

        assert_eq!(flowchart.simulation_state, SimulationState::Stopped);

        flowchart.simulation_state = SimulationState::Running;
        assert_eq!(flowchart.simulation_state, SimulationState::Running);

        flowchart.simulation_state = SimulationState::Paused;
        assert_eq!(flowchart.simulation_state, SimulationState::Paused);

        flowchart.simulation_state = SimulationState::Stopped;
        assert_eq!(flowchart.simulation_state, SimulationState::Stopped);
    }

    #[test]
    fn test_connection_messages() {
        let from_id = Uuid::new_v4();
        let to_id = Uuid::new_v4();
        let mut connection = Connection::new(from_id, to_id);

        assert!(connection.messages.is_empty());

        let message1 = Message::new(json!({"id": 1}));
        let message2 = Message::new(json!({"id": 2}));

        connection.messages.push(message1);
        connection.messages.push(message2);

        assert_eq!(connection.messages.len(), 2);
    }
}
