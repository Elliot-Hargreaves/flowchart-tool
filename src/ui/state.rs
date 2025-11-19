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

/// Available auto‑arrangement modes for laying out nodes
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum AutoArrangeMode {
    /// Physics-based force-directed layout
    ForceDirected,
    /// Place nodes in a grid
    Grid,
    /// Place nodes in a single horizontal line
    Line,
}

/// State related to canvas navigation and display.
///
/// Tracks the current pan offset, zoom level, and display options for the canvas.
#[derive(Serialize, Deserialize)]
#[serde(default)]
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
#[serde(default)]
pub struct InteractionState {
    /// Currently selected node ID, if any (kept for backward compatibility when exactly one node is selected)
    #[serde(skip)]
    pub selected_node: Option<NodeId>,
    /// Currently selected multiple nodes (if empty, no node selected)
    #[serde(skip)]
    pub selected_nodes: Vec<NodeId>,
    /// Currently selected group, if any
    #[serde(skip)]
    pub selected_group: Option<GroupId>,
    /// Node currently being edited for name changes
    #[serde(skip)]
    pub editing_node_name: Option<NodeId>,
    /// Group currently being edited for name changes
    #[serde(skip)]
    pub editing_group_name: Option<GroupId>,
    /// Temporary storage for node name while editing
    #[serde(skip)]
    pub temp_node_name: String,
    /// Temporary storage for group name while editing
    #[serde(skip)]
    pub temp_group_name: String,
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
    /// Whether the current marquee operation should add to the existing selection (Shift-held)
    #[serde(skip)]
    pub marquee_additive: bool,
    /// Node from which a connection is being drawn (shift-click drag)
    #[serde(skip)]
    pub drawing_connection_from: Option<NodeId>,
    /// Current mouse position while drawing connection
    #[serde(skip)]
    pub connection_draw_pos: Option<egui::Pos2>,
    /// Pending shift-press on a node that may become a connection if dragged beyond threshold
    #[serde(skip)]
    pub pending_shift_connection_from: Option<NodeId>,
    /// Start screen position for pending shift-connection gesture
    #[serde(skip)]
    pub pending_shift_start_screen_pos: Option<egui::Pos2>,
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
    /// Temporary storage for transformer globals editing: per-key JSON strings
    #[serde(skip)]
    pub temp_transformer_globals_edits: std::collections::HashMap<String, String>,
    /// Temporary fields to add a new global key/value
    #[serde(skip)]
    pub temp_new_global_key: String,
    #[serde(skip)]
    pub temp_new_global_value: String,
    /// Track which node's globals are currently loaded in temp_transformer_globals_edits
    #[serde(skip)]
    pub temp_globals_node_id: Option<NodeId>,
}

impl Default for InteractionState {
    fn default() -> Self {
        Self {
            selected_node: None,
            selected_nodes: Vec::new(),
            selected_group: None,
            editing_node_name: None,
            editing_group_name: None,
            temp_node_name: String::new(),
            temp_group_name: String::new(),
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
            marquee_additive: false,
            drawing_connection_from: None,
            connection_draw_pos: None,
            pending_shift_connection_from: None,
            pending_shift_start_screen_pos: None,
            selected_connection: None,
            temp_producer_start_step: String::new(),
            temp_producer_messages_per_cycle: String::new(),
            temp_producer_steps_between: String::new(),
            temp_producer_message_template: String::new(),
            temp_transformer_script: String::new(),
            temp_transformer_globals_edits: Default::default(),
            temp_new_global_key: String::new(),
            temp_new_global_value: String::new(),
            temp_globals_node_id: None,
        }
    }
}

/// State related to context menu display and interaction.
///
/// Manages the right-click context menu for creating new nodes.
#[derive(Serialize, Deserialize)]
#[serde(default)]
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
#[serde(default)]
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
    /// Whether to show an unsaved-changes confirmation dialog
    #[serde(skip)]
    pub show_unsaved_dialog: bool,
    /// The action the user attempted that requires confirmation (e.g., New or Quit)
    #[serde(skip)]
    pub pending_confirm_action: Option<PendingConfirmAction>,
    /// One-shot flag to allow the next close request to proceed after user confirmation (native only)
    #[serde(skip)]
    pub allow_close_on_next_request: bool,
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
            show_unsaved_dialog: false,
            pending_confirm_action: None,
            allow_close_on_next_request: false,
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

/// Pending confirmation actions that may require user approval due to unsaved changes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PendingConfirmAction {
    /// User is attempting to create a new file
    New,
    /// User is attempting to open a file
    Open,
    /// User is attempting to quit the application
    Quit,
}

/// The main application structure containing UI state and the flowchart data.
///
/// This struct implements the `eframe::App` trait and handles all user interface
/// rendering and interaction logic.
#[derive(Serialize, Deserialize)]
#[serde(default)]
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
    /// Whether dark mode visuals are enabled
    pub dark_mode: bool,
    /// Remembered width of the properties panel across sessions
    pub properties_panel_width: f32,
    /// Persisted last known window inner size in logical points (desktop only)
    /// Stored as a simple tuple to avoid depending on serde for egui types
    pub window_inner_size: Option<(f32, f32)>,
    /// Last known window position (desktop only)
    #[serde(skip)]
    pub last_window_pos: Option<egui::Pos2>,
    /// Whether we've already applied the stored window geometry this session
    #[serde(skip)]
    pub applied_viewport_restore: bool,
    /// Selected auto-arrangement mode for the toolbar button
    pub auto_arrange_mode: AutoArrangeMode,
    /// Counter for generating default group names
    pub group_counter: u32,
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
            dark_mode: true,
            properties_panel_width: 300.0,
            window_inner_size: None,
            last_window_pos: None,
            applied_viewport_restore: false,
            auto_arrange_mode: AutoArrangeMode::ForceDirected,
            group_counter: 0,
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

    /// Resets any non-UI related fields in the [FlowchartApp], so that when state is
    /// persisted only settings related to the UI are retained.
    pub fn reset_non_ui_fields(&mut self) {
        *self = Self {
            properties_panel_width: self.properties_panel_width,
            window_inner_size: self.window_inner_size,
            applied_viewport_restore: self.applied_viewport_restore,
            last_window_pos: self.last_window_pos,
            dark_mode: self.dark_mode,
            auto_arrange_mode: self.auto_arrange_mode,
            ..Default::default()
        };
    }
}
