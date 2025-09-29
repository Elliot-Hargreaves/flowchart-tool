//! Core data types and structures for the flowchart tool.
//! 
//! This module defines all the fundamental data structures used throughout the application,
//! including nodes, connections, messages, and the main flowchart structure.

use std::collections::HashMap;
use serde::{Deserialize, Serialize};
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
        /// Number of messages to generate per simulation step
        generation_rate: u32 
    },
    /// A node that consumes and destroys incoming messages
    Consumer { 
        /// Maximum number of messages to consume per simulation step
        consumption_rate: u32 
    },
    /// A node that transforms messages using a Lua script
    Transformer { 
        /// JavaScript code for message transformation
        script: String 
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
    /// Animation position along the connection edge (0.0 = start, 1.0 = end)
    pub position_along_edge: f32,
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
            position_along_edge: 0.0,
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
}

impl Default for Flowchart {
    /// Creates a new empty flowchart with no nodes or connections.
    fn default() -> Self {
        Self {
            nodes: HashMap::new(),
            connections: Vec::new(),
            simulation_state: SimulationState::Stopped,
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
            self.connections.retain(|conn| conn.from != *node_id && conn.to != *node_id);
        }
        removed
    }
}
