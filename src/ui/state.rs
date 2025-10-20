//! Application state management structures.
//!
//! This module contains all the state structures that track the application's
//! current UI state, including canvas navigation, user interactions, context menus,
//! and file operations.

use super::undo::UndoHistory;
use crate::simulation::SimulationEngine;
use crate::types::*;
use eframe::egui;
use serde::{Deserialize, Serialize};
use std::sync::mpsc::{channel, Receiver, Sender};

/// State related to canvas navigation and display.
///
/// Tracks the current pan offset, zoom level, and display options for the canvas.
#[derive(Serialize, Deserialize)]
pub struct CanvasState {
    /// Current canvas pan offset for navigation (in screen space)
    #[serde(skip)]
    pub offset: egui::Vec2,
    /// Current zoom level (1.0 = normal, 2.0 = 2x zoom, 0.5 = 50% zoom)
    pub zoom_factor: f32,
    /// Whether the grid should be displayed on the canvas
    pub show_grid: bool,
}

impl Default for CanvasState {
    fn default() -> Self {
        Self {
            offset: egui::Vec2::ZERO,
            zoom_factor: 1.0,
            show_grid: true,
        }
    }
}

/// State related to user interactions with nodes and canvas.
///
/// Tracks selection, dragging, editing, and connection drawing operations.
#[derive(Serialize, Deserialize)]
pub struct InteractionState {
    /// Currently selected node ID, if any (kept for backward compatibility when exactly one node is selected)
    #[serde(skip)]
    pub selected_node: Option<NodeId>,
    /// Currently selected multiple nodes (if empty, no node selected)
    #[serde(skip)]
    pub selected_nodes: Vec<NodeId>,
    /// Node currently being edited for name changes
    #[serde(skip)]
    pub editing_node_name: Option<NodeId>,
    /// Temporary storage for node name while editing
    #[serde(skip)]
    pub temp_node_name: String,
    /// Flag indicating text should be selected in the name field
    #[serde(skip)]
    pub should_select_text: bool,
    /// Flag to track if focus was already requested for the current edit session
    #[serde(skip)]
    pub focus_requested_for_edit: bool,
    /// Node currently being dragged by the user
    #[serde(skip)]
    pub dragging_node: Option<NodeId>,
    /// Initial mouse position when drag started
    #[serde(skip)]
    pub drag_start_pos: Option<egui::Pos2>,
    /// Original node position before drag started (for undo)
    #[serde(skip)]
    pub drag_original_position: Option<(f32, f32)>,
    /// Map of original positions for multi-node drag (for undo)
    #[serde(skip)]
    pub drag_original_positions_multi: Vec<(NodeId, (f32, f32))>,
    /// Offset from mouse to node center during dragging
    #[serde(skip)]
    pub node_drag_offset: egui::Vec2,
    /// Whether the user is currently panning the canvas
    #[serde(skip)]
    pub is_panning: bool,
    /// Last mouse position during panning operation
    #[serde(skip)]
    pub last_pan_pos: Option<egui::Pos2>,
    /// Marquee selection state: start and current end positions in screen space
    #[serde(skip)]
    pub marquee_start: Option<egui::Pos2>,
    #[serde(skip)]
    pub marquee_end: Option<egui::Pos2>,
    /// Node from which a connection is being drawn (shift-click drag)
    #[serde(skip)]
    pub drawing_connection_from: Option<NodeId>,
    /// Current mouse position while drawing connection
    #[serde(skip)]
    pub connection_draw_pos: Option<egui::Pos2>,
    /// Currently selected connection index, if any
    #[serde(skip)]
    pub selected_connection: Option<usize>,
    /// Temporary storage for producer properties while editing
    #[serde(skip)]
    pub temp_producer_start_step: String,
    #[serde(skip)]
    pub temp_producer_messages_per_cycle: String,
    #[serde(skip)]
    pub temp_producer_steps_between: String,
    #[serde(skip)]
    pub temp_producer_message_template: String,
    /// Temporary storage for transformer script while editing
    #[serde(skip)]
    pub temp_transformer_script: String,
}

impl Default for InteractionState {
    fn default() -> Self {
        Self {
            selected_node: None,
            selected_nodes: Vec::new(),
            editing_node_name: None,
            temp_node_name: String::new(),
            should_select_text: false,
            focus_requested_for_edit: false,
            dragging_node: None,
            drag_start_pos: None,
            drag_original_position: None,
            drag_original_positions_multi: Vec::new(),
            node_drag_offset: egui::Vec2::ZERO,
            is_panning: false,
            last_pan_pos: None,
            marquee_start: None,
            marquee_end: None,
            drawing_connection_from: None,
            connection_draw_pos: None,
            selected_connection: None,
            temp_producer_start_step: String::new(),
            temp_producer_messages_per_cycle: String::new(),
            temp_producer_steps_between: String::new(),
            temp_producer_message_template: String::new(),
            temp_transformer_script: String::new(),
        }
    }
}

/// State related to context menu display and interaction.
///
/// Manages the right-click context menu for creating new nodes.
#[derive(Serialize, Deserialize)]
pub struct ContextMenuState {
    /// Whether the context menu is currently visible
    #[serde(skip)]
    pub show: bool,
    /// Screen position where the context menu should appear
    #[serde(skip)]
    pub screen_pos: (f32, f32),
    /// World position where nodes should be created from context menu
    #[serde(skip)]
    pub world_pos: (f32, f32),
    /// Flag to prevent context menu from closing immediately after opening
    #[serde(skip)]
    pub just_opened: bool,
}

impl Default for ContextMenuState {
    fn default() -> Self {
        Self {
            show: false,
            screen_pos: (0.0, 0.0),
            world_pos: (0.0, 0.0),
            just_opened: false,
        }
    }
}

/// State related to file operations and persistence.
///
/// Manages file paths, unsaved changes tracking, and async file operations.
#[derive(Serialize, Deserialize)]
pub struct FileState {
    /// Current file path for save/load operations
    #[serde(skip)]
    pub current_path: Option<String>,
    /// Flag indicating if the flowchart has unsaved changes
    #[serde(skip)]
    pub has_unsaved_changes: bool,
    /// Pending file operations for WASM compatibility
    #[serde(skip)]
    pub pending_save_operation: Option<PendingSaveOperation>,
    #[serde(skip)]
    pub pending_load_operation: Option<PendingLoadOperation>,
    /// Channel for receiving file operation results from async contexts
    #[serde(skip)]
    pub file_operation_sender: Option<Sender<FileOperationResult>>,
    #[serde(skip)]
    pub file_operation_receiver: Option<Receiver<FileOperationResult>>,
}

impl Default for FileState {
    fn default() -> Self {
        let (sender, receiver) = channel();
        Self {
            current_path: None,
            has_unsaved_changes: false,
            pending_save_operation: None,
            pending_load_operation: None,
            file_operation_sender: Some(sender),
            file_operation_receiver: Some(receiver),
        }
    }
}

/// Represents a pending save operation type.
#[derive(Debug)]
pub enum PendingSaveOperation {
    /// Save with a new file path (show file picker)
    SaveAs,
    /// Save to the existing file path
    Save,
}

/// Represents a pending load operation type.
#[derive(Debug)]
pub enum PendingLoadOperation {
    /// Load from a file (show file picker)
    Load,
}

/// Messages sent from async file operations back to the main app.
#[derive(Debug)]
pub enum FileOperationResult {
    /// Save operation completed successfully with the given path
    SaveCompleted(String),
    /// Load operation completed successfully with path and content
    LoadCompleted(String, String),
    /// Operation failed with an error message
    OperationFailed(String),
}

/// The main application structure containing UI state and the flowchart data.
///
/// This struct implements the `eframe::App` trait and handles all user interface
/// rendering and interaction logic.
#[derive(Serialize, Deserialize)]
pub struct FlowchartApp {
    /// The flowchart being edited and simulated
    pub flowchart: Flowchart,
    /// Simulation engine for processing flowchart steps
    #[serde(skip)]
    pub simulation_engine: SimulationEngine,
    /// Whether the simulation is currently running
    #[serde(skip)]
    pub is_simulation_running: bool,
    /// Speed multiplier for simulation (currently unused)
    pub simulation_speed: f32,
    /// Counter for generating unique default node names
    pub node_counter: u32,
    /// Canvas navigation and display state
    pub canvas: CanvasState,
    /// User interaction state
    pub interaction: InteractionState,
    /// Context menu state
    pub context_menu: ContextMenuState,
    /// File operations state
    pub file: FileState,
    /// Node that encountered a script error, if any
    #[serde(skip)]
    pub error_node: Option<NodeId>,
    /// Frame counter for animation effects (e.g., flashing error borders)
    #[serde(skip)]
    pub frame_counter: u64,
    /// Undo/redo history for tracking and reversing actions
    pub undo_history: UndoHistory,
}

impl Default for FlowchartApp {
    fn default() -> Self {
        Self {
            flowchart: Flowchart::default(),
            simulation_engine: SimulationEngine::new(),
            is_simulation_running: false,
            simulation_speed: 1.0,
            node_counter: 0,
            canvas: CanvasState::default(),
            interaction: InteractionState::default(),
            context_menu: ContextMenuState::default(),
            file: FileState::default(),
            error_node: None,
            frame_counter: 0,
            undo_history: UndoHistory::new(),
        }
    }
}

impl FlowchartApp {
    /// Serializes the application state to JSON.
    ///
    /// # Returns
    ///
    /// A JSON string representation of the app state, or an error if serialization fails.
    pub fn to_json(&self) -> Result<String, serde_json::Error> {
        serde_json::to_string_pretty(self)
    }

    /// Deserializes application state from JSON.
    ///
    /// # Arguments
    ///
    /// * `json` - JSON string containing the serialized app state
    ///
    /// # Returns
    ///
    /// A `FlowchartApp` instance, or an error if deserialization fails.
    pub fn from_json(json: &str) -> Result<Self, serde_json::Error> {
        serde_json::from_str(json)
    }
}
