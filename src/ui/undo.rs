//! Undo/redo functionality for tracking and reversing user actions.
//!
//! This module provides a comprehensive undo/redo system that can track various
//! types of operations including node movements, property changes, and deletions.

use crate::types::*;
use serde::{Deserialize, Serialize};

/// Maximum number of undo actions to keep in history
const MAX_UNDO_HISTORY: usize = 100;

/// Represents different types of actions that can be undone.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum UndoAction {
    /// A node was moved from one position to another
    NodeMoved {
        /// The unique identifier of the node that moved
        node_id: NodeId,
        /// The previous position of the node as (x, y)
        old_position: (f32, f32),
        /// The new position of the node as (x, y)
        new_position: (f32, f32),
    },
    /// Multiple nodes were moved (e.g., during auto-layout or multi-drag)
    MultipleNodesMoved {
        /// Original positions for each affected node
        old_positions: Vec<(NodeId, (f32, f32))>,
        /// New positions for each affected node
        new_positions: Vec<(NodeId, (f32, f32))>,
    },
    /// A node's property was changed (e.g., script, template, parameters)
    PropertyChanged {
        /// The node whose property changed
        node_id: NodeId,
        /// The node's previous type/state before the change
        old_node_type: NodeType,
        /// The node's new type/state after the change
        new_node_type: NodeType,
    },
    /// A node was deleted
    NodeDeleted {
        /// The full node data that was deleted
        node: FlowchartNode,
        /// Any connections that were removed along with the node
        connections: Vec<Connection>,
    },
    /// Multiple nodes were deleted together
    MultipleNodesDeleted {
        /// The full node data for each deleted node
        nodes: Vec<FlowchartNode>,
        /// All connections that were removed as a result
        connections: Vec<Connection>,
    },
    /// A connection was deleted
    ConnectionDeleted {
        /// The removed connection data
        connection: Connection,
        /// The index where the connection existed in the list
        index: usize,
    },
    /// A node was created
    NodeCreated { 
        /// The unique identifier of the new node
        node_id: NodeId 
    },
    /// A connection was created
    ConnectionCreated { 
        /// Source node id
        from: NodeId, 
        /// Destination node id
        to: NodeId 
    },
    /// A node's name was changed
    NodeRenamed {
        /// The node whose name changed
        node_id: NodeId,
        /// The previous node name
        old_name: String,
        /// The new node name
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

    /// Pushes an undo action onto the undo stack.
    ///
    /// This method takes an `UndoAction` as input and appends it to the end of the
    /// undo stack, allowing the user to undo the corresponding action if needed.
    ///
    /// # Arguments
    ///
    /// * `action` - The undo action to be pushed onto the stack. It represents a
    ///   specific change that can be undone by the system.
    ///
    /// # Examples
    ///
    /// ```ignore
    /// let mut editor = Editor::new();
    /// editor.push_undo(UndoAction::NodeCreated {
    ///     node_id: Uuid::new_v4()
    /// });
    /// ```
    pub fn push_undo(&mut self, action: UndoAction) {
        self.undo_stack.push(action);
    }

    /// Pushes an undo action onto the redo stack.
    ///
    /// This method takes an `UndoAction` as input and adds it to the end of
    /// the internal redo stack. The redo stack keeps track of actions that
    /// can be undone if necessary.
    ///
    /// # Arguments
    /// - `action`: An instance of `UndoAction` representing the action to be
    ///   pushed onto the redo stack.
    ///
    /// # Examples
    /// ```ignore
    /// # use uuid::Uuid;
    /// let mut editor = Editor::new();
    /// let action1 = UndoAction::NodeCreated {
    ///     node_id: Uuid::new_v4()
    /// };
    /// editor.push_redo(action1);
    ///
    /// let action2 = UndoAction::NodeCreated {
    ///     node_id: Uuid::new_v4()
    /// };
    /// editor.push_redo(action2);
    ///
    /// // The redo stack now contains: [action2, action1]
    /// ```
    pub fn push_redo(&mut self, action: UndoAction) {
        self.redo_stack.push(action);
    }

    /// Pops the most recent action from the redo stack.
    ///
    /// # Returns
    ///
    /// The action to redo, or None if the redo stack is empty
    pub fn pop_redo(&mut self) -> Option<UndoAction> {
        self.redo_stack.pop()
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
}

impl UndoableFlowchart for Flowchart {
    fn apply_undo(&mut self, action: &UndoAction) -> Option<UndoAction> {
        match action {
            UndoAction::NodeMoved {
                node_id,
                old_position,
                new_position,
            } => {
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
            UndoAction::MultipleNodesMoved { old_positions, new_positions } => {
                // Restore old positions
                for (node_id, old_position) in old_positions {
                    if let Some(node) = self.nodes.get_mut(node_id) {
                        node.position = *old_position;
                    }
                }
                Some(UndoAction::MultipleNodesMoved { 
                    old_positions: new_positions.clone(),
                    new_positions: old_positions.clone(),
                })
            }
            UndoAction::PropertyChanged {
                node_id,
                old_node_type,
                new_node_type,
            } => {
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
            UndoAction::MultipleNodesDeleted { nodes, connections } => {
                // Restore all nodes
                for node in nodes {
                    self.nodes.insert(node.id, node.clone());
                }
                // Restore all connections
                for conn in connections {
                    self.connections.push(conn.clone());
                }
                // Redo would delete them again
                Some(UndoAction::MultipleNodesDeleted {
                    nodes: nodes.clone(),
                    connections: connections.clone(),
                })
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
                    let connections: Vec<Connection> = self
                        .connections
                        .iter()
                        .filter(|c| c.from == *node_id || c.to == *node_id)
                        .cloned()
                        .collect();
                    self.connections
                        .retain(|c| c.from != *node_id && c.to != *node_id);
                    Some(UndoAction::NodeDeleted { node, connections })
                } else {
                    None
                }
            }
            UndoAction::ConnectionCreated { from, to } => {
                // Remove the created connection
                if let Some(index) = self
                    .connections
                    .iter()
                    .position(|c| c.from == *from && c.to == *to)
                {
                    let connection = self.connections.remove(index);
                    Some(UndoAction::ConnectionDeleted { connection, index })
                } else {
                    None
                }
            }
            UndoAction::NodeRenamed {
                node_id,
                old_name,
                new_name,
            } => {
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
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::NodeType;
    use uuid::Uuid;

    #[test]
    fn test_undo_history_new() {
        let history = UndoHistory::new();
        assert!(!history.can_undo());
        assert!(!history.can_redo());
    }

    #[test]
    fn test_push_action_node_created() {
        let mut history = UndoHistory::new();
        let node_id = Uuid::new_v4();

        history.push_action(UndoAction::NodeCreated { node_id });

        assert!(history.can_undo());
        assert!(!history.can_redo());
    }

    #[test]
    fn test_push_action_clears_redo_stack() {
        let mut history = UndoHistory::new();
        let node_id1 = Uuid::new_v4();
        let node_id2 = Uuid::new_v4();

        history.push_action(UndoAction::NodeCreated { node_id: node_id1 });
        let action = history.pop_undo().unwrap();
        history.push_redo(action);
        assert!(history.can_redo());

        history.push_action(UndoAction::NodeCreated { node_id: node_id2 });
        assert!(!history.can_redo());
    }

    #[test]
    fn test_pop_undo() {
        let mut history = UndoHistory::new();
        let node_id = Uuid::new_v4();

        history.push_action(UndoAction::NodeCreated { node_id });

        let action = history.pop_undo();
        assert!(action.is_some());

        if let Some(UndoAction::NodeCreated { node_id: id }) = action {
            assert_eq!(id, node_id);
        } else {
            panic!("Expected NodeCreated action");
        }

        assert!(!history.can_undo());
    }

    #[test]
    fn test_pop_undo_empty() {
        let mut history = UndoHistory::new();
        let action = history.pop_undo();
        assert!(action.is_none());
    }

    #[test]
    fn test_pop_redo() {
        let mut history = UndoHistory::new();
        let node_id = Uuid::new_v4();

        history.push_action(UndoAction::NodeCreated { node_id });

        let action = history.pop_undo().unwrap();
        history.push_redo(action);

        let action = history.pop_redo();
        assert!(action.is_some());

        if let Some(UndoAction::NodeCreated { node_id: id }) = action {
            assert_eq!(id, node_id);
        } else {
            panic!("Expected NodeCreated action");
        }

        assert!(!history.can_redo());
    }

    #[test]
    fn test_pop_redo_empty() {
        let mut history = UndoHistory::new();
        let action = history.pop_redo();
        assert!(action.is_none());
    }

    #[test]
    fn test_multiple_undo_redo() {
        let mut history = UndoHistory::new();
        let id1 = Uuid::new_v4();
        let id2 = Uuid::new_v4();
        let id3 = Uuid::new_v4();

        history.push_action(UndoAction::NodeCreated { node_id: id1 });
        history.push_action(UndoAction::NodeCreated { node_id: id2 });
        history.push_action(UndoAction::NodeCreated { node_id: id3 });

        assert!(history.can_undo());
        let action = history.pop_undo().unwrap();
        history.push_redo(action);
        let action = history.pop_undo().unwrap();
        history.push_redo(action);
        assert!(history.can_undo());
        assert!(history.can_redo());

        history.pop_redo();
        assert!(history.can_redo());
    }

    #[test]
    fn test_node_moved_action() {
        let mut history = UndoHistory::new();
        let node_id = Uuid::new_v4();
        let old_pos = (10.0, 20.0);
        let new_pos = (50.0, 60.0);

        history.push_action(UndoAction::NodeMoved {
            node_id,
            old_position: old_pos,
            new_position: new_pos,
        });

        let action = history.pop_undo().unwrap();
        if let UndoAction::NodeMoved {
            node_id: id,
            old_position,
            new_position,
        } = action
        {
            assert_eq!(id, node_id);
            assert_eq!(old_position, old_pos);
            assert_eq!(new_position, new_pos);
        } else {
            panic!("Expected NodeMoved action");
        }
    }

    #[test]
    fn test_connection_created_action() {
        let mut history = UndoHistory::new();
        let from = Uuid::new_v4();
        let to = Uuid::new_v4();

        history.push_action(UndoAction::ConnectionCreated { from, to });

        let action = history.pop_undo().unwrap();
        if let UndoAction::ConnectionCreated { from: f, to: t } = action {
            assert_eq!(f, from);
            assert_eq!(t, to);
        } else {
            panic!("Expected ConnectionCreated action");
        }
    }

    #[test]
    fn test_connection_deleted_action() {
        let mut history = UndoHistory::new();
        let from_id = Uuid::new_v4();
        let to_id = Uuid::new_v4();
        let connection = Connection::new(from_id, to_id);

        history.push_action(UndoAction::ConnectionDeleted {
            connection: connection.clone(),
            index: 0,
        });

        let action = history.pop_undo().unwrap();
        if let UndoAction::ConnectionDeleted {
            connection: c,
            index,
        } = action
        {
            assert_eq!(c.from, from_id);
            assert_eq!(c.to, to_id);
            assert_eq!(index, 0);
        } else {
            panic!("Expected ConnectionDeleted action");
        }
    }

    #[test]
    fn test_node_deleted_action() {
        let mut history = UndoHistory::new();
        let node = FlowchartNode::new(
            "Test Node".to_string(),
            (100.0, 200.0),
            NodeType::Consumer {
                consumption_rate: 5,
            },
        );
        let node_id = node.id;

        history.push_action(UndoAction::NodeDeleted {
            node: node.clone(),
            connections: vec![],
        });

        let action = history.pop_undo().unwrap();
        if let UndoAction::NodeDeleted { node, connections } = action {
            assert_eq!(node.id, node_id);
            assert_eq!(node.name, "Test Node");
            assert_eq!(node.position, (100.0, 200.0));
            assert!(connections.is_empty());
        } else {
            panic!("Expected NodeDeleted action");
        }
    }

    #[test]
    fn test_clear_history() {
        let mut history = UndoHistory::new();
        let node_id = Uuid::new_v4();

        history.push_action(UndoAction::NodeCreated { node_id });
        history.push_redo(UndoAction::NodeCreated { node_id });


        assert!(history.can_redo());

        history.clear();

        assert!(!history.can_undo());
        assert!(!history.can_redo());
    }

    #[test]
    fn test_max_history_limit() {
        let mut history = UndoHistory::new();

        // Push more actions than max_history (100)
        for _i in 0..150 {
            let node_id = Uuid::new_v4();
            history.push_action(UndoAction::NodeCreated { node_id });
        }

        // Count how many we can undo
        let mut count = 0;
        while history.can_undo() {
            history.pop_undo();
            count += 1;
        }

        assert_eq!(count, 100);
    }

    #[test]
    fn test_undo_redo_sequence() {
        let mut history = UndoHistory::new();
        let id1 = Uuid::new_v4();
        let id2 = Uuid::new_v4();

        // Add two actions
        history.push_action(UndoAction::NodeCreated { node_id: id1 });
        history.push_action(UndoAction::NodeCreated { node_id: id2 });

        // Undo twice
        let action2 = history.pop_undo().unwrap();
        let action1 = history.pop_undo().unwrap();

        // Verify order
        if let UndoAction::NodeCreated { node_id } = action2 {
            history.push_redo(action2);
            assert_eq!(node_id, id2);
        }
        if let UndoAction::NodeCreated { node_id } = action1 {
            history.push_redo(action1);
            assert_eq!(node_id, id1);
        }

        // Redo twice
        let redo1 = history.pop_redo().unwrap();
        let redo2 = history.pop_redo().unwrap();

        // Verify order
        if let UndoAction::NodeCreated { node_id } = redo1 {
            assert_eq!(node_id, id1);
        }
        if let UndoAction::NodeCreated { node_id } = redo2 {
            assert_eq!(node_id, id2);
        }
    }
}
