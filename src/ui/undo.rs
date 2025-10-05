//! Undo/redo functionality for tracking and reversing user actions.
//!
//! This module provides a comprehensive undo/redo system that can track various
//! types of operations including node movements, property changes, and deletions.

use crate::types::*;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Maximum number of undo actions to keep in history
const MAX_UNDO_HISTORY: usize = 100;

/// Represents different types of actions that can be undone.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum UndoAction {
    /// A node was moved from one position to another
    NodeMoved {
        node_id: NodeId,
        old_position: (f32, f32),
        new_position: (f32, f32),
    },
    /// A node's property was changed (e.g., script, template, parameters)
    PropertyChanged {
        node_id: NodeId,
        old_node_type: NodeType,
        new_node_type: NodeType,
    },
    /// A node was deleted
    NodeDeleted {
        node: FlowchartNode,
        connections: Vec<Connection>,
    },
    /// A connection was deleted
    ConnectionDeleted {
        connection: Connection,
        index: usize,
    },
    /// A node was created
    NodeCreated {
        node_id: NodeId,
    },
    /// A connection was created
    ConnectionCreated {
        from: NodeId,
        to: NodeId,
    },
    /// A node's name was changed
    NodeRenamed {
        node_id: NodeId,
        old_name: String,
        new_name: String,
    },
}

/// Manages undo/redo history for the application.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct UndoHistory {
    /// Stack of actions that can be undone
    #[serde(skip)]
    undo_stack: Vec<UndoAction>,
    /// Stack of actions that can be redone
    #[serde(skip)]
    redo_stack: Vec<UndoAction>,
}

impl UndoHistory {
    /// Creates a new empty undo history.
    pub fn new() -> Self {
        Self {
            undo_stack: Vec::new(),
            redo_stack: Vec::new(),
        }
    }

    /// Adds an action to the undo history.
    ///
    /// This clears the redo stack since a new action invalidates any previously undone actions.
    ///
    /// # Arguments
    ///
    /// * `action` - The action to record
    pub fn push_action(&mut self, action: UndoAction) {
        self.undo_stack.push(action);
        self.redo_stack.clear();

        // Limit undo history size
        if self.undo_stack.len() > MAX_UNDO_HISTORY {
            self.undo_stack.remove(0);
        }
    }

    /// Returns true if there are actions that can be undone.
    pub fn can_undo(&self) -> bool {
        !self.undo_stack.is_empty()
    }

    /// Returns true if there are actions that can be redone.
    pub fn can_redo(&self) -> bool {
        !self.redo_stack.is_empty()
    }

    /// Pops the most recent action from the undo stack.
    ///
    /// # Returns
    ///
    /// The action to undo, or None if the undo stack is empty
    pub fn pop_undo(&mut self) -> Option<UndoAction> {
        self.undo_stack.pop()
    }

    /// Pops the most recent action from the redo stack.
    ///
    /// # Returns
    ///
    /// The action to redo, or None if the redo stack is empty
    pub fn pop_redo(&mut self) -> Option<UndoAction> {
        self.redo_stack.pop()
    }

    /// Pushes an action onto the redo stack.
    ///
    /// # Arguments
    ///
    /// * `action` - The action that was undone
    pub fn push_redo(&mut self, action: UndoAction) {
        self.redo_stack.push(action);
    }

    /// Clears all undo and redo history.
    pub fn clear(&mut self) {
        self.undo_stack.clear();
        self.redo_stack.clear();
    }
}

/// Extension methods for applying undo/redo actions to a flowchart.
pub trait UndoableFlowchart {
    /// Applies an undo action to reverse it.
    fn apply_undo(&mut self, action: &UndoAction) -> Option<UndoAction>;

    /// Applies a redo action to re-apply it.
    fn apply_redo(&mut self, action: &UndoAction) -> Option<UndoAction>;
}

impl UndoableFlowchart for Flowchart {
    fn apply_undo(&mut self, action: &UndoAction) -> Option<UndoAction> {
        match action {
            UndoAction::NodeMoved { node_id, old_position, new_position } => {
                if let Some(node) = self.nodes.get_mut(node_id) {
                    node.position = *old_position;
                    Some(UndoAction::NodeMoved {
                        node_id: *node_id,
                        old_position: *new_position,
                        new_position: *old_position,
                    })
                } else {
                    None
                }
            }
            UndoAction::PropertyChanged { node_id, old_node_type, new_node_type } => {
                if let Some(node) = self.nodes.get_mut(node_id) {
                    node.node_type = old_node_type.clone();
                    Some(UndoAction::PropertyChanged {
                        node_id: *node_id,
                        old_node_type: new_node_type.clone(),
                        new_node_type: old_node_type.clone(),
                    })
                } else {
                    None
                }
            }
            UndoAction::NodeDeleted { node, connections } => {
                // Restore the deleted node
                self.nodes.insert(node.id, node.clone());
                // Restore all connections involving this node
                for conn in connections {
                    self.connections.push(conn.clone());
                }
                Some(UndoAction::NodeCreated { node_id: node.id })
            }
            UndoAction::ConnectionDeleted { connection, index } => {
                // Restore the connection at its original index
                if *index <= self.connections.len() {
                    self.connections.insert(*index, connection.clone());
                } else {
                    self.connections.push(connection.clone());
                }
                Some(UndoAction::ConnectionCreated {
                    from: connection.from,
                    to: connection.to,
                })
            }
            UndoAction::NodeCreated { node_id } => {
                // Remove the created node and its connections
                if let Some(node) = self.nodes.remove(node_id) {
                    let connections: Vec<Connection> = self.connections
                        .iter()
                        .filter(|c| c.from == *node_id || c.to == *node_id)
                        .cloned()
                        .collect();
                    self.connections.retain(|c| c.from != *node_id && c.to != *node_id);
                    Some(UndoAction::NodeDeleted { node, connections })
                } else {
                    None
                }
            }
            UndoAction::ConnectionCreated { from, to } => {
                // Remove the created connection
                if let Some(index) = self.connections.iter().position(|c| c.from == *from && c.to == *to) {
                    let connection = self.connections.remove(index);
                    Some(UndoAction::ConnectionDeleted { connection, index })
                } else {
                    None
                }
            }
            UndoAction::NodeRenamed { node_id, old_name, new_name } => {
                if let Some(node) = self.nodes.get_mut(node_id) {
                    node.name = old_name.clone();
                    Some(UndoAction::NodeRenamed {
                        node_id: *node_id,
                        old_name: new_name.clone(),
                        new_name: old_name.clone(),
                    })
                } else {
                    None
                }
            }
        }
    }

    fn apply_redo(&mut self, action: &UndoAction) -> Option<UndoAction> {
        // Redo is just applying the reverse of an undo
        self.apply_undo(action)
    }
}
