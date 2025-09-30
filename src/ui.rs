//! User interface components and rendering logic for the flowchart tool.
//! 
//! This module contains all the UI-related code including the main application struct,
//! canvas rendering, property panels, context menus, and user interaction handling.

use crate::types::*;
use crate::simulation::SimulationEngine;
use eframe::egui;
use eframe::epaint::StrokeKind;
use serde::{Deserialize, Serialize};
use std::sync::mpsc::{channel, Sender, Receiver};

/// State related to canvas navigation and display
#[derive(Serialize, Deserialize)]
struct CanvasState {
    /// Current canvas pan offset for navigation
    #[serde(skip)]
    offset: egui::Vec2,
    /// Current zoom level (1.0 = normal, 2.0 = 2x zoom, 0.5 = 50% zoom)
    zoom_factor: f32,
    /// Whether the grid should be displayed on the canvas
    show_grid: bool,
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

/// State related to user interactions with nodes and canvas
#[derive(Serialize, Deserialize)]
struct InteractionState {
    /// Currently selected node ID, if any
    #[serde(skip)]
    selected_node: Option<NodeId>,
    /// Node currently being edited for name changes
    #[serde(skip)]
    editing_node_name: Option<NodeId>,
    /// Temporary storage for node name while editing
    #[serde(skip)]
    temp_node_name: String,
    /// Flag indicating text should be selected in the name field
    #[serde(skip)]
    should_select_text: bool,
    /// Node currently being dragged by the user
    #[serde(skip)]
    dragging_node: Option<NodeId>,
    /// Initial mouse position when drag started
    #[serde(skip)]
    drag_start_pos: Option<egui::Pos2>,
    /// Offset from mouse to node center during dragging
    #[serde(skip)]
    node_drag_offset: egui::Vec2,
    /// Whether the user is currently panning the canvas
    #[serde(skip)]
    is_panning: bool,
    /// Last mouse position during panning operation
    #[serde(skip)]
    last_pan_pos: Option<egui::Pos2>,
    /// Node from which a connection is being drawn (shift-click drag)
    #[serde(skip)]
    drawing_connection_from: Option<NodeId>,
    /// Current mouse position while drawing connection
    #[serde(skip)]
    connection_draw_pos: Option<egui::Pos2>,
    /// Currently selected connection index, if any
    #[serde(skip)]
    selected_connection: Option<usize>,
    /// Temporary storage for producer properties while editing
    #[serde(skip)]
    temp_producer_start_step: String,
    #[serde(skip)]
    temp_producer_messages_per_cycle: String,
    #[serde(skip)]
    temp_producer_steps_between: String,
    #[serde(skip)]
    temp_producer_message_template: String,
}

impl Default for InteractionState {
    fn default() -> Self {
        Self {
            selected_node: None,
            editing_node_name: None,
            temp_node_name: String::new(),
            should_select_text: false,
            dragging_node: None,
            drag_start_pos: None,
            node_drag_offset: egui::Vec2::ZERO,
            is_panning: false,
            last_pan_pos: None,
            drawing_connection_from: None,
            connection_draw_pos: None,
            selected_connection: None,
            temp_producer_start_step: String::new(),
            temp_producer_messages_per_cycle: String::new(),
            temp_producer_steps_between: String::new(),
            temp_producer_message_template: String::new(),
        }
    }
}

/// State related to context menu display and interaction
#[derive(Serialize, Deserialize)]
struct ContextMenuState {
    /// Whether the context menu is currently visible
    #[serde(skip)]
    show: bool,
    /// Screen position where the context menu should appear
    #[serde(skip)]
    screen_pos: (f32, f32),
    /// World position where nodes should be created from context menu
    #[serde(skip)]
    world_pos: (f32, f32),
    /// Flag to prevent context menu from closing immediately after opening
    #[serde(skip)]
    just_opened: bool,
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

/// State related to file operations and persistence
#[derive(Serialize, Deserialize)]
struct FileState {
    /// Current file path for save/load operations
    #[serde(skip)]
    current_path: Option<String>,
    /// Flag indicating if the flowchart has unsaved changes
    #[serde(skip)]
    has_unsaved_changes: bool,
    /// Pending file operations for WASM compatibility
    #[serde(skip)]
    pending_save_operation: Option<PendingSaveOperation>,
    #[serde(skip)]
    pending_load_operation: Option<PendingLoadOperation>,
    /// Channel for receiving file operation results from async contexts
    #[serde(skip)]
    file_operation_sender: Option<Sender<FileOperationResult>>,
    #[serde(skip)]
    file_operation_receiver: Option<Receiver<FileOperationResult>>,
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

/// The main application structure containing UI state and the flowchart data.
///
/// This struct implements the `eframe::App` trait and handles all user interface
/// rendering and interaction logic.
#[derive(Serialize, Deserialize)]
pub struct FlowchartApp {
    /// The flowchart being edited and simulated
    flowchart: Flowchart,
    /// Simulation engine for processing flowchart steps
    #[serde(skip)]
    simulation_engine: SimulationEngine,
    /// Whether the simulation is currently running
    #[serde(skip)]
    is_simulation_running: bool,
    /// Speed multiplier for simulation (currently unused)
    simulation_speed: f32,
    /// Counter for generating unique default node names
    node_counter: u32,
    /// Canvas navigation and display state
    canvas: CanvasState,
    /// User interaction state
    interaction: InteractionState,
    /// Context menu state
    context_menu: ContextMenuState,
    /// File operations state
    file: FileState,
}

#[derive(Debug)]
enum PendingSaveOperation {
    SaveAs,
    Save,
}

#[derive(Debug)]
enum PendingLoadOperation {
    Load,
}

/// Messages sent from async file operations back to the main app
#[derive(Debug)]
enum FileOperationResult {
    SaveCompleted(String), // path
    LoadCompleted(String, String), // path, content
    OperationFailed(String), // error message
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
        }
    }
}

impl eframe::App for FlowchartApp {
    /// Main update function called by egui for each frame.
    ///
    /// This method handles the overall UI layout, including the properties panel,
    /// toolbar, and main canvas area. It also processes simulation steps when running.
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // Handle pending file operations
        self.handle_pending_operations(ctx);

        // Handle delete key for removing selected objects
        self.handle_delete_key(ctx);

        // Properties panel on the right side
        egui::SidePanel::right("properties_panel")
            .resizable(true)
            .default_width(200.0)
            .show(ctx, |ui| {
                self.draw_properties_panel(ui);
            });

        // Main content area
        egui::CentralPanel::default().show(ctx, |ui| {
            ui.vertical(|ui| {
                // Toolbar at the top
                self.draw_toolbar(ui);
                ui.separator();

                // Canvas takes remaining space
                self.draw_canvas(ui);
            });
        });

        // Process simulation if running
        if self.is_simulation_running {
            let delivered_messages = self.simulation_engine.step(&mut self.flowchart);

            // Handle delivered messages
            for (node_id, message) in delivered_messages {
                self.simulation_engine.deliver_message(node_id, message, &mut self.flowchart);
            }

            ctx.request_repaint(); // Keep animating
        }
    }
}

impl FlowchartApp {
    /// Handles delete key presses to remove selected nodes or connections.
    fn handle_delete_key(&mut self, ctx: &egui::Context) {
        if ctx.input(|i| i.key_pressed(egui::Key::Delete)) {
            if let Some(selected_node) = self.interaction.selected_node {
                // Remove the selected node
                self.flowchart.nodes.remove(&selected_node);

                // Remove all connections involving this node
                self.flowchart.connections.retain(|c| c.from != selected_node && c.to != selected_node);

                // Clear selection
                self.interaction.selected_node = None;
                self.interaction.editing_node_name = None;
                self.file.has_unsaved_changes = true;
            } else if let Some(conn_idx) = self.interaction.selected_connection {
                // Remove the selected connection
                if conn_idx < self.flowchart.connections.len() {
                    self.flowchart.connections.remove(conn_idx);
                    self.interaction.selected_connection = None;
                    self.file.has_unsaved_changes = true;
                }
            }
        }
    }

    /// Handle pending file operations for WASM compatibility
    fn handle_pending_operations(&mut self, ctx: &egui::Context) {
        // First, process any completed file operations from the channel
        if let Some(receiver) = &self.file.file_operation_receiver {
            while let Ok(result) = receiver.try_recv() {
                match result {
                    FileOperationResult::SaveCompleted(path) => {
                        self.file.current_path = Some(path);
                        self.file.has_unsaved_changes = false;
                        println!("File saved successfully");
                    }
                    FileOperationResult::LoadCompleted(path, content) => {
                        match Flowchart::from_json(&content) {
                            Ok(flowchart) => {
                                self.flowchart = flowchart;
                                self.file.current_path = Some(path);
                                self.file.has_unsaved_changes = false;
                                self.interaction.selected_node = None;
                                self.interaction.editing_node_name = None;
                                // Update node counter to avoid ID conflicts
                                self.node_counter = self.flowchart.nodes.len() as u32;
                                println!("File loaded successfully");
                            }
                            Err(e) => {
                                eprintln!("Failed to parse flowchart: {}", e);
                            }
                        }
                    }
                    FileOperationResult::OperationFailed(error) => {
                        eprintln!("File operation failed: {}", error);
                    }
                }
            }
        }

        // Handle pending save operations
        if let Some(save_op) = self.file.pending_save_operation.take() {
            let ctx = ctx.clone();
            let flowchart_json = self.flowchart.to_json().unwrap_or_default();
            let sender = self.file.file_operation_sender.clone();

            match save_op {
                PendingSaveOperation::SaveAs => {
                    wasm_bindgen_futures::spawn_local(async move {
                        if let Some(handle) = rfd::AsyncFileDialog::new()
                            .add_filter("JSON", &["json"])
                            .set_file_name("flowchart.json")
                            .save_file()
                            .await
                        {
                            #[cfg(target_arch = "wasm32")]
                            {
                                match handle.write(flowchart_json.as_bytes()).await {
                                    Ok(_) => {
                                        let filename = handle.file_name();
                                        if let Some(tx) = sender {
                                            let _ = tx.send(FileOperationResult::SaveCompleted(filename));
                                        }
                                    }
                                    Err(e) => {
                                        if let Some(tx) = sender {
                                            let _ = tx.send(FileOperationResult::OperationFailed(
                                                format!("Failed to write file: {}", e)
                                            ));
                                        }
                                    }
                                }
                            }

                            #[cfg(not(target_arch = "wasm32"))]
                            {
                                let path = handle.path();
                                match std::fs::write(path, flowchart_json) {
                                    Ok(_) => {
                                        if let Some(tx) = sender {
                                            let _ = tx.send(FileOperationResult::SaveCompleted(
                                                path.display().to_string()
                                            ));
                                        }
                                    }
                                    Err(e) => {
                                        if let Some(tx) = sender {
                                            let _ = tx.send(FileOperationResult::OperationFailed(
                                                format!("Failed to save file: {}", e)
                                            ));
                                        }
                                    }
                                }
                            }
                        }
                        ctx.request_repaint();
                    });
                }
                PendingSaveOperation::Save => {
                    if let Some(ref path) = self.file.current_path.clone() {
                        let path = path.clone();
                        wasm_bindgen_futures::spawn_local(async move {
                            #[cfg(not(target_arch = "wasm32"))]
                            {
                                match std::fs::write(&path, flowchart_json) {
                                    Ok(_) => {
                                        if let Some(tx) = sender {
                                            let _ = tx.send(FileOperationResult::SaveCompleted(path));
                                        }
                                    }
                                    Err(e) => {
                                        if let Some(tx) = sender {
                                            let _ = tx.send(FileOperationResult::OperationFailed(
                                                format!("Failed to save file: {}", e)
                                            ));
                                        }
                                    }
                                }
                            }
                            ctx.request_repaint();
                        });
                    } else {
                        self.file.pending_save_operation = Some(PendingSaveOperation::SaveAs);
                    }
                }
            }
        }

        // Handle pending load operations
        if let Some(_load_op) = self.file.pending_load_operation.take() {
            let ctx = ctx.clone();
            let sender = self.file.file_operation_sender.clone();

            wasm_bindgen_futures::spawn_local(async move {
                if let Some(handle) = rfd::AsyncFileDialog::new()
                    .add_filter("JSON", &["json"])
                    .pick_file()
                    .await
                {
                    #[cfg(target_arch = "wasm32")]
                    {
                        let content = handle.read().await;
                        match String::from_utf8(content) {
                            Ok(json_str) => {
                                let filename = handle.file_name();
                                if let Some(tx) = sender {
                                    let _ = tx.send(FileOperationResult::LoadCompleted(filename, json_str));
                                }
                            }
                            Err(e) => {
                                if let Some(tx) = sender {
                                    let _ = tx.send(FileOperationResult::OperationFailed(
                                        format!("Failed to read file as UTF-8: {}", e)
                                    ));
                                }
                            }
                        }
                    }

                    #[cfg(not(target_arch = "wasm32"))]
                    {
                        let path = handle.path();
                        match std::fs::read_to_string(path) {
                            Ok(json) => {
                                if let Some(tx) = sender {
                                    let _ = tx.send(FileOperationResult::LoadCompleted(
                                        path.display().to_string(),
                                        json
                                    ));
                                }
                            }
                            Err(e) => {
                                if let Some(tx) = sender {
                                    let _ = tx.send(FileOperationResult::OperationFailed(
                                        format!("Failed to read file: {}", e)
                                    ));
                                }
                            }
                        }
                    }
                }
                ctx.request_repaint();
            });
        }
    }
    /// Renders the toolbar with file operations, simulation controls, and view options.
    /// 
    /// The toolbar contains file operations, simulation controls, and display options.
    fn draw_toolbar(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            // File operations
            if ui.button("New").clicked() {
                self.new_flowchart();
            }
            if ui.button("Open").clicked() {
                self.load_flowchart();
            }
            if ui.button("Save").clicked() {
                self.save_flowchart();
            }
            if ui.button("Save As").clicked() {
                self.save_as_flowchart();
            }

            ui.separator();

            // Simulation controls
            if ui.button("Start").clicked() {
                self.is_simulation_running = true;
                self.flowchart.simulation_state = SimulationState::Running;
            }
            if ui.button("Stop").clicked() {
                self.is_simulation_running = false;
                self.flowchart.simulation_state = SimulationState::Stopped;
                self.flowchart.current_step = 0;
                // Clear all messages from connections
                for connection in &mut self.flowchart.connections {
                    connection.messages.clear();
                }
            }
            if ui.button("Step").clicked() {
                let delivered_messages = self.simulation_engine.step(&mut self.flowchart);
                for (node_id, message) in delivered_messages {
                    self.simulation_engine.deliver_message(node_id, message, &mut self.flowchart);
                }
            }

            ui.separator();

            // View options
            ui.checkbox(&mut self.canvas.show_grid, "Show Grid");

            ui.separator();

            // Show current simulation step
            ui.label(format!("Step: {}", self.flowchart.current_step));

            // Show current file and unsaved changes indicator
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if let Some(file_path) = &self.file.current_path {
                    let status = if self.file.has_unsaved_changes { "*" } else { "" };
                    ui.label(format!("{}{}", file_path, status));
                } else {
                    let status = if self.file.has_unsaved_changes { "Untitled*" } else { "Untitled" };
                    ui.label(status);
                }

                ui.label(format!("Zoom: {:.0}%", self.canvas.zoom_factor * 100.0));
            });
        });
    }

    /// Renders the properties panel showing details of the selected node or connection.
    /// 
    /// The panel displays node/connection information and allows editing of properties
    /// including name, type-specific settings, and current state.
    fn draw_properties_panel(&mut self, ui: &mut egui::Ui) {
        ui.vertical(|ui| {
            ui.heading("Properties");
            ui.separator();

            if let Some(selected_id) = self.interaction.selected_node {
                if let Some(node) = self.flowchart.nodes.get(&selected_id).cloned() {
                    ui.label("Type: Node");
                    ui.separator();

                    // Node name editing
                    ui.label("Name:");

                    if self.interaction.editing_node_name == Some(selected_id) {
                        self.draw_name_editor(ui, selected_id);
                    } else {
                        // Show name as clickable button
                        if ui.button(&node.name).clicked() {
                            self.start_editing_node_name(selected_id, &node.name);
                        }
                    }

                    ui.separator();

                    // Node type display (now mutable)
                    self.draw_node_type_info(ui, &node);

                    ui.separator();

                    // Node state and position
                    self.draw_node_status_info(ui, &node);
                } else {
                    ui.label("Node not found");
                }
            } else if let Some(conn_idx) = self.interaction.selected_connection {
                if let Some(connection) = self.flowchart.connections.get(conn_idx) {
                    self.draw_connection_properties(ui, connection);
                } else {
                    ui.label("Connection not found");
                }
            } else {
                self.draw_no_selection_info(ui);
            }
        });
    }

    /// Renders connection properties in the properties panel.
    fn draw_connection_properties(&self, ui: &mut egui::Ui, connection: &Connection) {
        ui.label("Type: Connection");
        ui.separator();

        // Show from and to node names
        if let Some(from_node) = self.flowchart.nodes.get(&connection.from) {
            ui.label(format!("From: {}", from_node.name));
        } else {
            ui.label(format!("From: (node not found)"));
        }

        if let Some(to_node) = self.flowchart.nodes.get(&connection.to) {
            ui.label(format!("To: {}", to_node.name));
        } else {
            ui.label(format!("To: (node not found)"));
        }

        ui.separator();
        ui.label(format!("Messages in transit: {}", connection.messages.len()));

        ui.separator();
        ui.colored_label(egui::Color32::GRAY, "Press Delete to remove");
    }

    /// Saves the flowchart to a JSON string.
    pub fn to_json(&self) -> Result<String, serde_json::Error> {
        serde_json::to_string_pretty(self)
    }

    /// Loads a flowchart from a JSON string.
    pub fn from_json(json: &str) -> Result<Self, serde_json::Error> {
        serde_json::from_str(json)
    }

    /// Renders the name editing field for a node.
    fn draw_name_editor(&mut self, ui: &mut egui::Ui, selected_id: NodeId) {
        let response = ui.text_edit_singleline(&mut self.interaction.temp_node_name);

        // Auto-focus the text field
        response.request_focus();

        // Select all text when flag is set and field has focus
        if self.interaction.should_select_text && response.has_focus() {
            self.interaction.should_select_text = false;
            self.select_all_text_in_field(ui, response.id);
        }

        // Handle Enter key to save changes
        if response.has_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
            self.save_node_name_change(selected_id);
        } else if response.lost_focus() {
            // Cancel editing if focus lost without Enter
            self.interaction.editing_node_name = None;
        }
    }

    /// Selects all text in a text edit field using egui's internal state.
    fn select_all_text_in_field(&self, ui: &mut egui::Ui, field_id: egui::Id) {
        ui.memory_mut(|mem| {
            let state = mem.data.get_temp_mut_or_default::<egui::text_edit::TextEditState>(field_id);
            let text_len = self.interaction.temp_node_name.len();
            state.cursor.set_char_range(Some(egui::text::CCursorRange::two(
                egui::text::CCursor::new(0),
                egui::text::CCursor::new(text_len),
            )));
        });
    }

    /// Starts editing the name of the specified node.
    fn start_editing_node_name(&mut self, node_id: NodeId, current_name: &str) {
        self.interaction.editing_node_name = Some(node_id);
        self.interaction.temp_node_name = current_name.to_string();
        self.interaction.should_select_text = true;
    }

    /// Saves the current name edit to the selected node.
    fn save_node_name_change(&mut self, node_id: NodeId) {
        if let Some(node) = self.flowchart.nodes.get_mut(&node_id) {
            node.name = self.interaction.temp_node_name.clone();
            self.file.has_unsaved_changes = true;
        }
        self.interaction.editing_node_name = None;
    }

    /// Updates a producer node property from the temporary editing values.
    fn update_producer_property(&mut self, node_id: NodeId, property: &str) {
        if let Some(node) = self.flowchart.nodes.get_mut(&node_id) {
            if let NodeType::Producer { 
                ref mut message_template,
                ref mut start_step,
                ref mut messages_per_cycle,
                ref mut steps_between_cycles,
            } = node.node_type {
                match property {
                    "start_step" => {
                        if let Ok(value) = self.interaction.temp_producer_start_step.parse::<u64>() {
                            *start_step = value;
                            self.file.has_unsaved_changes = true;
                        }
                    }
                    "messages_per_cycle" => {
                        if let Ok(value) = self.interaction.temp_producer_messages_per_cycle.parse::<u32>() {
                            *messages_per_cycle = value;
                            self.file.has_unsaved_changes = true;
                        }
                    }
                    "steps_between_cycles" => {
                        if let Ok(value) = self.interaction.temp_producer_steps_between.parse::<u32>() {
                            *steps_between_cycles = value;
                            self.file.has_unsaved_changes = true;
                        }
                    }
                    "message_template" => {
                        if let Ok(value) = serde_json::from_str::<serde_json::Value>(
                            &self.interaction.temp_producer_message_template
                        ) {
                            *message_template = value;
                            self.file.has_unsaved_changes = true;
                        }
                    }
                    _ => {}
                }
            }
        }
    }

    /// Renders node type information and type-specific properties.
    fn draw_node_type_info(&mut self, ui: &mut egui::Ui, node: &FlowchartNode) {
        ui.label(format!("Type: {}", match &node.node_type {
            NodeType::Producer { .. } => "Producer",
            NodeType::Consumer { .. } => "Consumer",
            NodeType::Transformer { .. } => "Transformer",
        }));

        // Type-specific properties
        match &node.node_type {
            NodeType::Producer { 
                message_template,
                start_step,
                messages_per_cycle,
                steps_between_cycles,
            } => {
                // Initialize temp values if empty
                if self.interaction.temp_producer_start_step.is_empty() {
                    self.interaction.temp_producer_start_step = start_step.to_string();
                }
                if self.interaction.temp_producer_messages_per_cycle.is_empty() {
                    self.interaction.temp_producer_messages_per_cycle = messages_per_cycle.to_string();
                }
                if self.interaction.temp_producer_steps_between.is_empty() {
                    self.interaction.temp_producer_steps_between = steps_between_cycles.to_string();
                }
                if self.interaction.temp_producer_message_template.is_empty() {
                    self.interaction.temp_producer_message_template = 
                        serde_json::to_string_pretty(message_template)
                            .unwrap_or_else(|_| "{}".to_string());
                }

                ui.label("Start Step:");
                if ui.text_edit_singleline(&mut self.interaction.temp_producer_start_step).changed() {
                    self.update_producer_property(node.id, "start_step");
                }

                ui.label("Messages per Cycle:");
                if ui.text_edit_singleline(&mut self.interaction.temp_producer_messages_per_cycle).changed() {
                    self.update_producer_property(node.id, "messages_per_cycle");
                }

                ui.label("Steps Between Cycles:");
                if ui.text_edit_singleline(&mut self.interaction.temp_producer_steps_between).changed() {
                    self.update_producer_property(node.id, "steps_between_cycles");
                }

                ui.separator();
                ui.label("Message Template (JSON):");
                if ui.add(egui::TextEdit::multiline(&mut self.interaction.temp_producer_message_template)
                    .desired_rows(5)
                    .desired_width(f32::INFINITY)
                    .code_editor()).changed() {
                    self.update_producer_property(node.id, "message_template");
                }
            }
            NodeType::Consumer { consumption_rate } => {
                ui.label(format!("Consumption Rate: {} msg/step", consumption_rate));
            }
            NodeType::Transformer { script } => {
                ui.label("JavaScript Script:");
                ui.add(egui::TextEdit::multiline(&mut script.clone())
                    .desired_rows(3)
                    .desired_width(f32::INFINITY));
            }
        }
    }

    /// Renders node status information including state and position.
    fn draw_node_status_info(&self, ui: &mut egui::Ui, node: &FlowchartNode) {
        ui.label(format!("State: {:?}", node.state));
        ui.label(format!("Position: ({:.1}, {:.1})", node.position.0, node.position.1));
    }

    /// Renders information shown when no node is selected.
    fn draw_no_selection_info(&self, ui: &mut egui::Ui) {
        ui.label("No node selected");
        ui.separator();
        ui.label("Left-click on a node to select it");
        ui.label("Right-click on canvas to create nodes");
        ui.label("Middle-click and drag to pan");
    }

    /// Renders the right-click context menu for creating nodes.
    fn draw_context_menu(&mut self, ui: &mut egui::Ui) {
        // Use the stored screen coordinates for menu positioning
        let screen_pos = egui::pos2(self.context_menu.screen_pos.0, self.context_menu.screen_pos.1);

        let area_response = egui::Area::new(egui::Id::new("context_menu"))
            .fixed_pos(screen_pos)
            .show(ui.ctx(), |ui| {
                egui::Frame::popup(ui.style()).show(ui, |ui| {
                    ui.vertical(|ui| {
                        ui.label("Create Node:");
                        ui.separator();

                        if ui.button("Producer").clicked() {
                            self.create_node_at_pos(NodeType::Producer {
                                message_template: serde_json::json!({"value": 0}),
                                start_step: 0,
                                messages_per_cycle: 1,
                                steps_between_cycles: 0,
                            });
                            self.context_menu.show = false;
                        }

                        if ui.button("Consumer").clicked() {
                            self.create_node_at_pos(NodeType::Consumer { consumption_rate: 1 });
                            self.context_menu.show = false;
                        }

                        if ui.button("Transformer").clicked() {
                            self.create_node_at_pos(NodeType::Transformer {
                                script: "// Transform the input message\nfunction transform(input) {\n    return { data: input.data };\n}".to_string()
                            });
                            self.context_menu.show = false;
                        }

                        ui.separator();
                        if ui.button("Cancel").clicked() {
                            self.context_menu.show = false;
                        }
                    });
                })
            });

        // Handle click-outside-to-close after the first frame
        if !self.context_menu.just_opened {
            if ui.input(|i| i.pointer.primary_clicked()) {
                if let Some(click_pos) = ui.input(|i| i.pointer.interact_pos()) {
                    if !area_response.response.rect.contains(click_pos) {
                        self.context_menu.show = false;
                    }
                }
            }
        }

        self.context_menu.just_opened = false;
    }

    /// Creates a new node at the context menu position.
    fn create_node_at_pos(&mut self, node_type: NodeType) {
        self.node_counter += 1;

        let new_node = FlowchartNode::new(
            format!("node{}", self.node_counter),
            self.context_menu.world_pos,
            node_type,
        );

        let node_id = new_node.id;
        self.flowchart.add_node(new_node);

        // Select the new node and start editing its name immediately
        self.interaction.selected_node = Some(node_id);
        self.start_editing_node_name(node_id, &format!("node{}", self.node_counter));

        // Mark as having unsaved changes
        self.file.has_unsaved_changes = true;
    }

    /// Opens a file dialog to save the flowchart with a new name.
    fn save_as_flowchart(&mut self) {
        #[cfg(target_arch = "wasm32")]
        {
            self.file.pending_save_operation = Some(PendingSaveOperation::SaveAs);
        }

        #[cfg(not(target_arch = "wasm32"))]
        {
            let mut file_dialog_future = async {
                if let Some(path) = rfd::AsyncFileDialog::new()
                    .add_filter("JSON", &["json"])
                    .set_file_name("flowchart.json")
                    .save_file().await
                {
                    let filename = path.path().display().to_string();
                    self.save_to_path(&filename);
                }
            };
            futures::executor::block_on(file_dialog_future);
        }
    }

    /// Saves the flowchart to the specified path.
    fn save_to_path(&mut self, path: &str) {
        #[cfg(not(target_arch = "wasm32"))]
        {
            match self.flowchart.to_json() {
                Ok(json) => {
                    if let Err(e) = std::fs::write(&path, json) {
                        eprintln!("Failed to save file: {}", e);
                    } else {
                        self.file.current_path = Some(path.to_string());
                        self.file.has_unsaved_changes = false;
                    }
                }
                Err(e) => {
                    eprintln!("Failed to serialize flowchart: {}", e);
                }
            }
        }

        #[cfg(target_arch = "wasm32")]
        {
            // In WASM, file operations are handled through the pending operations system
            eprintln!("Save operation completed for: {}", path);
        }
    }

    /// Opens a file dialog to load a flowchart.
    fn load_flowchart(&mut self) {
        #[cfg(target_arch = "wasm32")]
        {
            self.file.pending_load_operation = Some(PendingLoadOperation::Load);
        }

        #[cfg(not(target_arch = "wasm32"))]
        {
            let file_dialog_future = async {
                if let Some(path) = rfd::AsyncFileDialog::new()
                    .add_filter("JSON", &["json"])
                    .pick_file().await
                {
                    let filename = path.path().display().to_string();
                    match std::fs::read_to_string(&filename) {
                        Ok(json) => {
                            match Flowchart::from_json(&json) {
                                Ok(flowchart) => {
                                    self.flowchart = flowchart;
                                    self.file.current_path = Some(filename);
                                    self.file.has_unsaved_changes = false;
                                    self.interaction.selected_node = None;
                                    self.interaction.editing_node_name = None;
                                    // Update node counter to avoid ID conflicts
                                    self.node_counter = self.flowchart.nodes.len() as u32;
                                }
                                Err(e) => {
                                    eprintln!("Failed to parse flowchart: {}", e);
                                }
                            }
                        }
                        Err(e) => {
                            eprintln!("Failed to read file: {}", e);
                        }
                    }
                }
            };
            futures::executor::block_on(file_dialog_future);
        }
    }

    /// Saves the current flowchart to a file.
    fn save_flowchart(&mut self) {
        if self.file.current_path.is_some() {
            #[cfg(target_arch = "wasm32")]
            {
                self.file.pending_save_operation = Some(PendingSaveOperation::Save);
            }

            #[cfg(not(target_arch = "wasm32"))]
            {
                if let Some(file_path) = &self.file.current_path.clone() {
                    self.save_to_path(file_path);
                }
            }
        } else {
            self.save_as_flowchart();
        }
    }

    /// Creates a new empty flowchart.
    fn new_flowchart(&mut self) {
        self.flowchart = Flowchart::new();
        self.flowchart.current_step = 0;
        self.file.current_path = None;
        self.file.has_unsaved_changes = false;
        self.interaction.selected_node = None;
        self.interaction.editing_node_name = None;
        self.node_counter = 0;
        self.canvas.offset = egui::Vec2::ZERO;
        self.canvas.zoom_factor = 1.0;
    }

    /// Finds the node at the given canvas position, if any.
    fn find_node_at_position(&self, pos: egui::Pos2) -> Option<NodeId> {
        const NODE_SIZE: egui::Vec2 = egui::Vec2::new(100.0, 70.0);

        for (id, node) in &self.flowchart.nodes {
            let node_pos = egui::pos2(node.position.0, node.position.1);
            let rect = egui::Rect::from_center_size(node_pos, NODE_SIZE);

            if rect.contains(pos) {
                return Some(*id);
            }
        }
        None
    }

    /// Wraps text to fit within the specified width, returning a vector of lines.
    fn wrap_text(&self, text: &str, max_width: f32, font_id: &egui::FontId, painter: &egui::Painter) -> Vec<String> {
        let mut lines = Vec::new();
        let words: Vec<&str> = text.split_whitespace().collect();

        if words.is_empty() {
            return vec![text.to_string()];
        }

        let mut current_line = String::new();

        for word in words {
            let test_line = if current_line.is_empty() {
                word.to_string()
            } else {
                format!("{} {}", current_line, word)
            };

            let text_width = painter.fonts(|f| {
                f.layout_no_wrap(test_line.clone(), font_id.clone(), egui::Color32::BLACK).size().x
            });

            if text_width <= max_width {
                current_line = test_line;
            } else {
                if !current_line.is_empty() {
                    lines.push(current_line);
                    current_line = word.to_string();
                } else {
                    // Single word too long, add it anyway
                    lines.push(word.to_string());
                }
            }
        }

        if !current_line.is_empty() {
            lines.push(current_line);
        }

        if lines.is_empty() {
            lines.push(text.to_string());
        }

        lines
    }

    /// Renders the main canvas area with nodes, connections, and handles user interactions.
    fn draw_canvas(&mut self, ui: &mut egui::Ui) {
        let (response, painter) = ui.allocate_painter(
            ui.available_size(),
            egui::Sense::click_and_drag()
        );

        // Initialize canvas to center the origin on first frame
        if self.canvas.offset == egui::Vec2::ZERO && self.node_counter == 0 {
            let canvas_center = response.rect.center();
            self.canvas.offset = canvas_center.to_vec2();
        }

        // Handle canvas panning with middle mouse button or Ctrl+drag
        self.handle_canvas_panning(ui, &response);

        // Handle scroll wheel zooming
        self.handle_canvas_zoom(ui, &response);

        // Handle node dragging with left mouse button
        self.handle_node_dragging(ui, &response);

        // Render all flowchart elements
        let canvas_rect = response.rect;
        self.render_flowchart_elements(&painter, canvas_rect);

        // Handle other interactions (selection, context menu)
        self.handle_canvas_interactions(ui, &response);

        // Show context menu if active
        if self.context_menu.show {
            self.draw_context_menu(ui);
        }
    }

    /// Handles middle-click or Cmd/Ctrl+left-click canvas panning functionality.
    /// Uses Cmd on macOS and Ctrl on other platforms.
    fn handle_canvas_panning(&mut self, ui: &mut egui::Ui, response: &egui::Response) {
        // Check for middle mouse button OR Cmd/Ctrl+left mouse button
        // modifiers.command automatically uses Cmd on macOS and Ctrl elsewhere
        let should_pan = ui.input(|i| i.pointer.middle_down() || 
                                      (i.pointer.primary_down() && i.modifiers.command));

        if should_pan {
            if let Some(current_pos) = response.interact_pointer_pos() {
                if !self.interaction.is_panning {
                    self.interaction.is_panning = true;
                    self.interaction.last_pan_pos = Some(current_pos);
                } else if let Some(last_pos) = self.interaction.last_pan_pos {
                    let delta = current_pos - last_pos;
                    self.canvas.offset += delta;
                    self.interaction.last_pan_pos = Some(current_pos);
                }
            }
        } else {
            self.interaction.is_panning = false;
            self.interaction.last_pan_pos = None;
        }
    }

    /// Handles scroll wheel zooming functionality.
    fn handle_canvas_zoom(&mut self, ui: &mut egui::Ui, response: &egui::Response) {
        let scroll_delta = ui.input(|i| i.smooth_scroll_delta.y);

        if scroll_delta != 0.0 {
            // Use hover position if available, otherwise use response position
            let mouse_pos = ui.input(|i| i.pointer.hover_pos())
                .or_else(|| response.interact_pointer_pos());

            if let Some(mouse_pos) = mouse_pos {
                // Calculate the world position under the mouse cursor before zoom
                let world_pos_before_zoom = self.screen_to_world(mouse_pos);

                // Apply zoom change with smaller, more precise steps
                let zoom_delta = if scroll_delta > 0.0 { 0.025 } else { -0.025 };
                let old_zoom = self.canvas.zoom_factor;
                self.canvas.zoom_factor = (self.canvas.zoom_factor + zoom_delta).clamp(0.25, 5.0);

                // Only adjust offset if zoom actually changed
                if (self.canvas.zoom_factor - old_zoom).abs() > f32::EPSILON {
                    // Calculate where that world position should appear on screen after zoom
                    let world_pos_after_zoom = self.world_to_screen(world_pos_before_zoom);

                    // Adjust canvas offset to keep the world position under the mouse cursor
                    let offset_adjustment = mouse_pos - world_pos_after_zoom;
                    self.canvas.offset += offset_adjustment;
                }
            }
        }
    }

    /// Handles node dragging functionality with left mouse button.
    fn handle_node_dragging(&mut self, ui: &mut egui::Ui, response: &egui::Response) {
        if ui.input(|i| i.pointer.primary_down()) && !self.interaction.is_panning {
            if let Some(current_pos) = response.interact_pointer_pos() {
                let world_pos = self.screen_to_world(current_pos);
                let shift_held = ui.input(|i| i.modifiers.shift);

                // Check if we're starting a new interaction
                if self.interaction.dragging_node.is_none() && self.interaction.drawing_connection_from.is_none() {
                    // Check if clicking on a node
                    if let Some(node_id) = self.find_node_at_position(world_pos) {
                        if shift_held {
                            // Shift-click on node: start drawing connection
                            self.interaction.drawing_connection_from = Some(node_id);
                            self.interaction.connection_draw_pos = Some(current_pos);
                        } else {
                            // Normal click on node: start dragging
                            self.start_node_drag(node_id, current_pos, world_pos);
                        }
                    }
                    // If not clicking on a node, do nothing (no drag starts)
                } else if let Some(dragging_id) = self.interaction.dragging_node {
                    // Continue dragging node - check shift for grid snapping
                    self.update_dragged_node_position(dragging_id, world_pos, ui);
                } else if self.interaction.drawing_connection_from.is_some() {
                    // Continue drawing connection - update preview position
                    self.interaction.connection_draw_pos = Some(current_pos);
                }
            }
        } else {
            // Mouse released - finalize connection if drawing
            if self.interaction.drawing_connection_from.is_some() {
                if let Some(current_pos) = response.interact_pointer_pos() {
                    let world_pos = self.screen_to_world(current_pos);
                    self.finalize_connection(world_pos);
                }
            }

            // Stop all dragging/drawing operations when mouse released
            self.interaction.dragging_node = None;
            self.interaction.drag_start_pos = None;
            self.interaction.drawing_connection_from = None;
            self.interaction.connection_draw_pos = None;
        }
    }

    /// Finalizes connection creation when mouse is released.
    fn finalize_connection(&mut self, world_pos: egui::Pos2) {
        if let Some(from_node) = self.interaction.drawing_connection_from {
            if let Some(to_node) = self.find_node_at_position(world_pos) {
                // Don't create self-connections
                if from_node != to_node {
                    // Check if connection already exists
                    let connection_exists = self.flowchart.connections.iter().any(|c| {
                        c.from == from_node && c.to == to_node
                    });

                    if !connection_exists {
                        // Create new connection
                        let connection = Connection::new(from_node, to_node);
                        self.flowchart.connections.push(connection);
                        self.file.has_unsaved_changes = true;
                    }
                }
            }
        }
    }

    /// Starts dragging the specified node.
    fn start_node_drag(&mut self, node_id: NodeId, current_pos: egui::Pos2, world_pos: egui::Pos2) {
        self.interaction.dragging_node = Some(node_id);
        self.interaction.drag_start_pos = Some(current_pos);

        // Calculate offset from node center to mouse position for smooth dragging
        if let Some(node) = self.flowchart.nodes.get(&node_id) {
            let node_center = egui::pos2(node.position.0, node.position.1);
            self.interaction.node_drag_offset = node_center - world_pos;
        }
    }

    /// Updates the position of the currently dragged node.
    fn update_dragged_node_position(&mut self, node_id: NodeId, world_pos: egui::Pos2, ui: &egui::Ui) {
        let mut new_world_pos = world_pos + self.interaction.node_drag_offset;

        // Check if Shift is held for grid snapping
        if ui.input(|i| i.modifiers.shift) {
            new_world_pos = self.snap_to_grid(new_world_pos);
        }

        if let Some(node) = self.flowchart.nodes.get_mut(&node_id) {
            node.position = (new_world_pos.x, new_world_pos.y);
        }
    }

    /// Converts screen coordinates to world coordinates accounting for zoom and pan.
    fn screen_to_world(&self, screen_pos: egui::Pos2) -> egui::Pos2 {
        (screen_pos - self.canvas.offset) / self.canvas.zoom_factor
    }

    /// Converts world coordinates to screen coordinates accounting for zoom and pan.
    fn world_to_screen(&self, world_pos: egui::Pos2) -> egui::Pos2 {
        world_pos * self.canvas.zoom_factor + self.canvas.offset
    }

    /// Snaps a position to the nearest grid point.
    /// 
    /// Grid spacing is 20 units.
    fn snap_to_grid(&self, pos: egui::Pos2) -> egui::Pos2 {
        const GRID_SIZE: f32 = 20.0;
        egui::pos2(
            (pos.x / GRID_SIZE).round() * GRID_SIZE,
            (pos.y / GRID_SIZE).round() * GRID_SIZE,
        )
    }

    /// Draws a grid on the canvas for visual reference.
    /// 
    /// Grid lines are drawn every 20 world units with zoom-aware spacing and styling.
    fn draw_grid(&self, painter: &egui::Painter, canvas_rect: egui::Rect) {
        const GRID_SIZE: f32 = 20.0;
        let grid_color = egui::Color32::from_rgba_unmultiplied(128, 128, 128, 32);
        let stroke = egui::Stroke::new(1.0, grid_color);

        // Calculate world space bounds from screen space
        let top_left_world = self.screen_to_world(canvas_rect.min);
        let bottom_right_world = self.screen_to_world(canvas_rect.max);

        // Calculate grid range in world coordinates
        let start_x = (top_left_world.x / GRID_SIZE).floor() * GRID_SIZE;
        let end_x = (bottom_right_world.x / GRID_SIZE).ceil() * GRID_SIZE;
        let start_y = (top_left_world.y / GRID_SIZE).floor() * GRID_SIZE;
        let end_y = (bottom_right_world.y / GRID_SIZE).ceil() * GRID_SIZE;

        // Only draw grid if zoom level makes it reasonable to see
        let screen_grid_size = GRID_SIZE * self.canvas.zoom_factor;
        if screen_grid_size < 2.0 {
            // Grid too small to see clearly, skip drawing
            return;
        }

        // Draw vertical grid lines
        let mut x = start_x;
        while x <= end_x {
            let world_pos = egui::pos2(x, 0.0);
            let screen_x = self.world_to_screen(world_pos).x;

            if screen_x >= canvas_rect.min.x && screen_x <= canvas_rect.max.x {
                painter.line_segment(
                    [egui::pos2(screen_x, canvas_rect.min.y), egui::pos2(screen_x, canvas_rect.max.y)],
                    stroke,
                );
            }
            x += GRID_SIZE;
        }

        // Draw horizontal grid lines
        let mut y = start_y;
        while y <= end_y {
            let world_pos = egui::pos2(0.0, y);
            let screen_y = self.world_to_screen(world_pos).y;

            if screen_y >= canvas_rect.min.y && screen_y <= canvas_rect.max.y {
                painter.line_segment(
                    [egui::pos2(canvas_rect.min.x, screen_y), egui::pos2(canvas_rect.max.x, screen_y)],
                    stroke,
                );
            }
            y += GRID_SIZE;
        }

        // Draw axis lines more prominently when zoomed in
        if screen_grid_size > 10.0 {
            let axis_color = egui::Color32::from_rgba_unmultiplied(128, 128, 128, 80);
            let axis_stroke = egui::Stroke::new(1.5, axis_color);

            // Draw X axis (y=0)
            let x_axis_screen_y = self.world_to_screen(egui::pos2(0.0, 0.0)).y;
            if x_axis_screen_y >= canvas_rect.min.y && x_axis_screen_y <= canvas_rect.max.y {
                painter.line_segment(
                    [egui::pos2(canvas_rect.min.x, x_axis_screen_y), egui::pos2(canvas_rect.max.x, x_axis_screen_y)],
                    axis_stroke,
                );
            }

            // Draw Y axis (x=0)
            let y_axis_screen_x = self.world_to_screen(egui::pos2(0.0, 0.0)).x;
            if y_axis_screen_x >= canvas_rect.min.x && y_axis_screen_x <= canvas_rect.max.x {
                painter.line_segment(
                    [egui::pos2(y_axis_screen_x, canvas_rect.min.y), egui::pos2(y_axis_screen_x, canvas_rect.max.y)],
                    axis_stroke,
                );
            }
        }
    }

    /// Renders all flowchart elements (grid, connections and nodes).
    fn render_flowchart_elements(&self, painter: &egui::Painter, canvas_rect: egui::Rect) {
        // Draw grid first (behind everything) if enabled
        if self.canvas.show_grid {
            self.draw_grid(painter, canvas_rect);
        }

        // Draw connections second (behind nodes)
        for (idx, connection) in self.flowchart.connections.iter().enumerate() {
            let is_selected = self.interaction.selected_connection == Some(idx);
            self.draw_connection(painter, connection, is_selected);
        }

        // Draw connection preview if currently drawing
        if let Some(from_node_id) = self.interaction.drawing_connection_from {
            if let Some(draw_pos) = self.interaction.connection_draw_pos {
                self.draw_connection_preview(painter, from_node_id, draw_pos);
            }
        }

        // Draw nodes on top
        for (_id, node) in &self.flowchart.nodes {
            self.draw_node(painter, node);
        }
    }

    /// Renders a preview of the connection being drawn during shift-click drag.
    fn draw_connection_preview(&self, painter: &egui::Painter, from_node_id: NodeId, to_screen_pos: egui::Pos2) {
        if let Some(from_node) = self.flowchart.nodes.get(&from_node_id) {
            let from_world = egui::pos2(from_node.position.0, from_node.position.1);
            let from_screen = self.world_to_screen(from_world);

            // Draw dashed line for preview
            let stroke = egui::Stroke::new(2.0, egui::Color32::from_rgb(100, 150, 255));
            painter.line_segment([from_screen, to_screen_pos], stroke);

            // Draw small circle at the end to indicate connection point
            painter.circle_filled(to_screen_pos, 4.0, egui::Color32::from_rgb(100, 150, 255));
        }
    }

    /// Handles canvas click interactions for selection and context menu.
    fn handle_canvas_interactions(&mut self, _ui: &mut egui::Ui, response: &egui::Response) {
        // Left-click for selection (only if not dragging or panning)
        if response.clicked() && !self.interaction.is_panning && self.interaction.dragging_node.is_none() {
            if let Some(pos) = response.interact_pointer_pos() {
                let world_pos = self.screen_to_world(pos);

                // First try to select a node
                if let Some(node_id) = self.find_node_at_position(world_pos) {
                    self.interaction.selected_node = Some(node_id);
                    self.interaction.selected_connection = None;
                    self.interaction.editing_node_name = None;
                    // Clear temp producer values to reload from selected node
                    self.interaction.temp_producer_start_step.clear();
                    self.interaction.temp_producer_messages_per_cycle.clear();
                    self.interaction.temp_producer_steps_between.clear();
                    self.interaction.temp_producer_message_template.clear();
                } else {
                    // Try to select a connection
                    if let Some(conn_idx) = self.find_connection_at_position(world_pos) {
                        self.interaction.selected_connection = Some(conn_idx);
                        self.interaction.selected_node = None;
                        self.interaction.editing_node_name = None;
                        // Clear temp producer values
                        self.interaction.temp_producer_start_step.clear();
                        self.interaction.temp_producer_messages_per_cycle.clear();
                        self.interaction.temp_producer_steps_between.clear();
                        self.interaction.temp_producer_message_template.clear();
                    } else {
                        // Clear selection if clicking on empty space
                        self.interaction.selected_node = None;
                        self.interaction.selected_connection = None;
                        self.interaction.editing_node_name = None;
                        // Clear temp producer values
                        self.interaction.temp_producer_start_step.clear();
                        self.interaction.temp_producer_messages_per_cycle.clear();
                        self.interaction.temp_producer_steps_between.clear();
                        self.interaction.temp_producer_message_template.clear();
                    }
                }
            }
        }

        // Right-click for context menu
        if response.secondary_clicked() && !self.interaction.is_panning && self.interaction.dragging_node.is_none() {
            if let Some(screen_pos) = response.interact_pointer_pos() {
                let world_pos = self.screen_to_world(screen_pos);
                self.context_menu.screen_pos = (screen_pos.x, screen_pos.y);
                self.context_menu.world_pos = (world_pos.x, world_pos.y);
                self.context_menu.show = true;
                self.context_menu.just_opened = true;
            }
        }
    }

    /// Finds the connection at the given world position, if any.
    /// Returns the index of the connection in the connections vector.
    fn find_connection_at_position(&self, pos: egui::Pos2) -> Option<usize> {
        const CLICK_THRESHOLD: f32 = 10.0; // pixels in world space

        for (idx, connection) in self.flowchart.connections.iter().enumerate() {
            if let (Some(from_node), Some(to_node)) = (
                self.flowchart.nodes.get(&connection.from),
                self.flowchart.nodes.get(&connection.to),
            ) {
                let start = egui::pos2(from_node.position.0, from_node.position.1);
                let end = egui::pos2(to_node.position.0, to_node.position.1);

                // Calculate distance from point to line segment
                let distance = self.point_to_line_distance(pos, start, end);

                if distance < CLICK_THRESHOLD {
                    return Some(idx);
                }
            }
        }

        None
    }

    /// Calculates the distance from a point to a line segment.
    fn point_to_line_distance(&self, point: egui::Pos2, line_start: egui::Pos2, line_end: egui::Pos2) -> f32 {
        let line_vec = line_end - line_start;
        let point_vec = point - line_start;
        let line_len_sq = line_vec.length_sq();

        if line_len_sq < 0.0001 {
            // Line segment is essentially a point
            return point_vec.length();
        }

        // Project point onto line segment
        let t = (point_vec.dot(line_vec) / line_len_sq).clamp(0.0, 1.0);
        let projection = line_start + line_vec * t;

        (point - projection).length()
    }

    /// Renders a connection between two nodes with animated messages and directional arrow.
    fn draw_connection(&self, painter: &egui::Painter, connection: &Connection, is_selected: bool) {
        // Get node positions with zoom and canvas offset applied
        let start_world = self.flowchart.nodes.get(&connection.from)
            .map(|n| egui::pos2(n.position.0, n.position.1))
            .unwrap_or_else(|| egui::pos2(0.0, 0.0));
        let start_pos = self.world_to_screen(start_world);

        let end_world = self.flowchart.nodes.get(&connection.to)
            .map(|n| egui::pos2(n.position.0, n.position.1))
            .unwrap_or_else(|| egui::pos2(100.0, 100.0));
        let end_pos = self.world_to_screen(end_world);

        // Choose color and width based on selection
        let (line_color, line_width) = if is_selected {
            (egui::Color32::from_rgb(100, 150, 255), 3.0)
        } else {
            (egui::Color32::DARK_GRAY, 2.0)
        };

        // Draw the connection line
        painter.line_segment(
            [start_pos, end_pos],
            egui::Stroke::new(line_width, line_color)
        );

        // Draw arrow at the center of the connection
        self.draw_arrow_at_center(painter, start_pos, end_pos, line_color);

        // Draw messages as a grid next to the arrow
        if !connection.messages.is_empty() {
            self.draw_message_grid(painter, start_pos, end_pos, connection.messages.len());
        }
    }

    /// Draws a grid of dots representing messages in transit next to the connection arrow.
    /// 
    /// The grid is 5 dots wide with unlimited depth.
    fn draw_message_grid(&self, painter: &egui::Painter, start: egui::Pos2, end: egui::Pos2, message_count: usize) {
        const GRID_WIDTH: usize = 5;
        const DOT_SPACING: f32 = 8.0;
        const DOT_RADIUS: f32 = 3.0;

        // Calculate center point of the connection
        let center = start + (end - start) * 0.5;

        // Calculate direction and perpendicular vectors
        let direction = (end - start).normalized();
        let perpendicular = egui::vec2(-direction.y, direction.x);

        // Offset the grid to the side of the arrow
        let grid_offset = perpendicular * 15.0 * self.canvas.zoom_factor;

        // Calculate grid dimensions
        let rows = (message_count + GRID_WIDTH - 1) / GRID_WIDTH; // Ceiling division

        let cols = usize::min(GRID_WIDTH, message_count);

        // Calculate starting position (top-left of grid)
        let grid_width_pixels = (GRID_WIDTH as f32 * -1.0) as f32 * DOT_SPACING * self.canvas.zoom_factor;
        let grid_height_pixels = (GRID_WIDTH -1) as f32 * DOT_SPACING * self.canvas.zoom_factor;

        let grid_start = center + grid_offset 
            - perpendicular * grid_width_pixels * 0.5
            - direction * grid_height_pixels * 0.5;

        // Draw each dot in the grid
        for i in 0..message_count {
            let row = i / GRID_WIDTH;
            let col = i % GRID_WIDTH;

            let dot_pos = grid_start 
                + perpendicular * (row as f32 * DOT_SPACING * self.canvas.zoom_factor)
                + direction * (col as f32 * DOT_SPACING * self.canvas.zoom_factor);

            let scaled_radius = DOT_RADIUS * self.canvas.zoom_factor;
            painter.circle_filled(dot_pos, scaled_radius, egui::Color32::YELLOW);
            painter.circle_stroke(dot_pos, scaled_radius, egui::Stroke::new(1.0, egui::Color32::DARK_GRAY));
        }
    }

    /// Draws a directional arrow at the center of a connection line.
    fn draw_arrow_at_center(&self, painter: &egui::Painter, start: egui::Pos2, end: egui::Pos2, color: egui::Color32) {
        // Calculate center point
        let center = start + (end - start) * 0.5;

        // Calculate direction vector
        let direction = (end - start).normalized();

        // Arrow size scales with zoom
        let arrow_size = 8.0 * self.canvas.zoom_factor;
        let arrow_width = 6.0 * self.canvas.zoom_factor;

        // Calculate perpendicular vector for arrow wings
        let perpendicular = egui::vec2(-direction.y, direction.x);

        // Calculate arrow points (triangle)
        let arrow_tip = center + direction * arrow_size;
        let arrow_left = center - direction * arrow_size + perpendicular * arrow_width;
        let arrow_right = center - direction * arrow_size - perpendicular * arrow_width;

        // Draw filled triangle
        painter.add(egui::Shape::convex_polygon(
            vec![arrow_tip, arrow_left, arrow_right],
            color,
            egui::Stroke::NONE,
        ));
    }

    /// Renders a single flowchart node with appropriate styling and text.
    fn draw_node(&self, painter: &egui::Painter, node: &FlowchartNode) {
        const NODE_SIZE: egui::Vec2 = egui::Vec2::new(100.0, 70.0);

        // Apply zoom and canvas offset for proper positioning
        let world_pos = egui::pos2(node.position.0, node.position.1);
        let screen_pos = self.world_to_screen(world_pos);
        let scaled_size = NODE_SIZE * self.canvas.zoom_factor;
        let rect = egui::Rect::from_center_size(screen_pos, scaled_size);

        // Determine node color based on type
        let mut color = match node.node_type {
            NodeType::Producer { .. } => egui::Color32::LIGHT_GREEN,
            NodeType::Consumer { .. } => egui::Color32::LIGHT_RED,
            NodeType::Transformer { .. } => egui::Color32::LIGHT_BLUE,
        };

        // Darken color if being dragged
        if Some(node.id) == self.interaction.dragging_node {
            color = egui::Color32::from_rgba_unmultiplied(
                (color.r() as f32 * 0.8) as u8,
                (color.g() as f32 * 0.8) as u8,
                (color.b() as f32 * 0.8) as u8,
                color.a(),
            );
        }

        // Draw filled rectangle
        painter.rect_filled(rect, 5.0, color);

        // Draw border with appropriate highlighting
        let (stroke_color, stroke_width) = if Some(node.id) == self.interaction.dragging_node {
            (egui::Color32::from_rgb(255, 165, 0), 4.0) // Orange for dragging
        } else if Some(node.id) == self.interaction.selected_node {
            (egui::Color32::YELLOW, 3.0) // Yellow for selected
        } else {
            (egui::Color32::BLACK, 2.0) // Black for normal
        };

        painter.rect_stroke(rect, 5.0, egui::Stroke::new(stroke_width, stroke_color), StrokeKind::Outside);

        // Render wrapped node name text
        self.draw_node_text(painter, node, screen_pos, scaled_size);
    }

    /// Renders the node's name text with proper wrapping and positioning.
    fn draw_node_text(&self, painter: &egui::Painter, node: &FlowchartNode, pos: egui::Pos2, size: egui::Vec2) {
        let text_rect = egui::Rect::from_center_size(
            egui::pos2(pos.x, pos.y - 5.0 * self.canvas.zoom_factor),
            egui::vec2(size.x - 10.0 * self.canvas.zoom_factor, size.y - 20.0 * self.canvas.zoom_factor) // Leave padding
        );

        // Create zoom-aware font size
        let base_font_size = 12.0;
        let scaled_font_size = (base_font_size * self.canvas.zoom_factor).clamp(8.0, 48.0);
        let font_id = egui::FontId::proportional(scaled_font_size);

        let max_width = text_rect.width();
        let wrapped_text = self.wrap_text(&node.name, max_width, &font_id, painter);

        // Calculate text positioning for vertical centering
        let line_height = painter.fonts(|f| f.row_height(&font_id));
        let total_height = line_height * wrapped_text.len() as f32;
        let start_y = text_rect.center().y - total_height / 2.0;

        // Draw each line of text
        for (i, line) in wrapped_text.iter().enumerate() {
            let line_pos = egui::pos2(
                text_rect.center().x,
                start_y + i as f32 * line_height
            );
            painter.text(
                line_pos,
                egui::Align2::CENTER_CENTER,
                line,
                font_id.clone(),
                egui::Color32::BLACK,
            );
        }
    }
}
