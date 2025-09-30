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
    /// Currently selected node ID, if any
    #[serde(skip)]
    selected_node: Option<NodeId>,
    /// Whether the simulation is currently running
    #[serde(skip)]
    is_simulation_running: bool,
    /// Speed multiplier for simulation (currently unused)
    simulation_speed: f32,
    /// Whether the context menu is currently visible
    #[serde(skip)]
    show_context_menu: bool,
    /// Whether the grid should be displayed on the canvas
    show_grid: bool,
    /// Current zoom level (1.0 = normal, 2.0 = 2x zoom, 0.5 = 50% zoom)
    zoom_factor: f32,
    /// Screen position where the context menu should appear
    #[serde(skip)]
    context_menu_screen_pos: (f32, f32),
    /// World position where nodes should be created from context menu
    #[serde(skip)]
    context_menu_world_pos: (f32, f32),
    /// Counter for generating unique default node names
    node_counter: u32,
    /// Current file path for save/load operations
    #[serde(skip)]
    current_file_path: Option<String>,
    /// Flag indicating if the flowchart has unsaved changes
    #[serde(skip)]
    has_unsaved_changes: bool,
    /// Node currently being edited for name changes
    #[serde(skip)]
    editing_node_name: Option<NodeId>,
    /// Temporary storage for node name while editing
    #[serde(skip)]
    temp_node_name: String,
    /// Current canvas pan offset for navigation
    #[serde(skip)]
    canvas_offset: egui::Vec2,
    /// Whether the user is currently panning the canvas
    #[serde(skip)]
    is_panning: bool,
    /// Last mouse position during panning operation
    #[serde(skip)]
    last_pan_pos: Option<egui::Pos2>,
    /// Flag to prevent context menu from closing immediately after opening
    #[serde(skip)]
    context_menu_just_opened: bool,
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
        let (sender, receiver) = channel();
        Self {
            flowchart: Flowchart::default(),
            simulation_engine: SimulationEngine::new(),
            selected_node: None,
            is_simulation_running: false,
            simulation_speed: 1.0,
            show_context_menu: false,
            show_grid: true, // Grid enabled by default
            zoom_factor: 1.0, // Normal zoom level
            context_menu_screen_pos: (0.0, 0.0),
            context_menu_world_pos: (0.0, 0.0),
            node_counter: 0,
            current_file_path: None,
            has_unsaved_changes: false,
            editing_node_name: None,
            temp_node_name: String::new(),
            canvas_offset: egui::Vec2::ZERO,
            is_panning: false,
            last_pan_pos: None,
            context_menu_just_opened: false,
            should_select_text: false,
            dragging_node: None,
            drag_start_pos: None,
            node_drag_offset: egui::Vec2::ZERO,
            pending_save_operation: None,
            pending_load_operation: None,
            file_operation_sender: Some(sender),
            file_operation_receiver: Some(receiver),
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
    /// Handle pending file operations for WASM compatibility
    fn handle_pending_operations(&mut self, ctx: &egui::Context) {
        // First, process any completed file operations from the channel
        if let Some(receiver) = &self.file_operation_receiver {
            while let Ok(result) = receiver.try_recv() {
                match result {
                    FileOperationResult::SaveCompleted(path) => {
                        self.current_file_path = Some(path);
                        self.has_unsaved_changes = false;
                        println!("File saved successfully");
                    }
                    FileOperationResult::LoadCompleted(path, content) => {
                        match Flowchart::from_json(&content) {
                            Ok(flowchart) => {
                                self.flowchart = flowchart;
                                self.current_file_path = Some(path);
                                self.has_unsaved_changes = false;
                                self.selected_node = None;
                                self.editing_node_name = None;
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
        if let Some(save_op) = self.pending_save_operation.take() {
            let ctx = ctx.clone();
            let flowchart_json = self.flowchart.to_json().unwrap_or_default();
            let sender = self.file_operation_sender.clone();

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
                    if let Some(ref path) = self.current_file_path.clone() {
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
                        self.pending_save_operation = Some(PendingSaveOperation::SaveAs);
                    }
                }
            }
        }

        // Handle pending load operations
        if let Some(_load_op) = self.pending_load_operation.take() {
            let ctx = ctx.clone();
            let sender = self.file_operation_sender.clone();

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
            }
            if ui.button("Step").clicked() {
                let delivered_messages = self.simulation_engine.step(&mut self.flowchart);
                for (node_id, message) in delivered_messages {
                    self.simulation_engine.deliver_message(node_id, message, &mut self.flowchart);
                }
            }

            ui.separator();

            // View options
            ui.checkbox(&mut self.show_grid, "Show Grid");

            // Show current file and unsaved changes indicator
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if let Some(file_path) = &self.current_file_path {
                    let status = if self.has_unsaved_changes { "*" } else { "" };
                    ui.label(format!("{}{}", file_path, status));
                } else {
                    let status = if self.has_unsaved_changes { "Untitled*" } else { "Untitled" };
                    ui.label(status);
                }

                ui.label(format!("Zoom: {:.0}%", self.zoom_factor * 100.0));
            });
        });
    }

    /// Renders the properties panel showing details of the selected node.
    /// 
    /// The panel displays node information and allows editing of node properties
    /// including name, type-specific settings, and current state.
    fn draw_properties_panel(&mut self, ui: &mut egui::Ui) {
        ui.vertical(|ui| {
            ui.heading("Properties");
            ui.separator();

            if let Some(selected_id) = self.selected_node {
                if let Some(node) = self.flowchart.nodes.get(&selected_id).cloned() {
                    // Node name editing
                    ui.label("Name:");

                    if self.editing_node_name == Some(selected_id) {
                        self.draw_name_editor(ui, selected_id);
                    } else {
                        // Show name as clickable button
                        if ui.button(&node.name).clicked() {
                            self.start_editing_node_name(selected_id, &node.name);
                        }
                    }

                    ui.separator();

                    // Node type display
                    self.draw_node_type_info(ui, &node);

                    ui.separator();

                    // Node state and position
                    self.draw_node_status_info(ui, &node);
                } else {
                    ui.label("Node not found");
                }
            } else {
                self.draw_no_selection_info(ui);
            }
        });
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
        let response = ui.text_edit_singleline(&mut self.temp_node_name);

        // Auto-focus the text field
        response.request_focus();

        // Select all text when flag is set and field has focus
        if self.should_select_text && response.has_focus() {
            self.should_select_text = false;
            self.select_all_text_in_field(ui, response.id);
        }

        // Handle Enter key to save changes
        if response.has_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
            self.save_node_name_change(selected_id);
        } else if response.lost_focus() {
            // Cancel editing if focus lost without Enter
            self.editing_node_name = None;
        }
    }

    /// Selects all text in a text edit field using egui's internal state.
    fn select_all_text_in_field(&self, ui: &mut egui::Ui, field_id: egui::Id) {
        ui.memory_mut(|mem| {
            let state = mem.data.get_temp_mut_or_default::<egui::text_edit::TextEditState>(field_id);
            let text_len = self.temp_node_name.len();
            state.cursor.set_char_range(Some(egui::text::CCursorRange::two(
                egui::text::CCursor::new(0),
                egui::text::CCursor::new(text_len),
            )));
        });
    }

    /// Starts editing the name of the specified node.
    fn start_editing_node_name(&mut self, node_id: NodeId, current_name: &str) {
        self.editing_node_name = Some(node_id);
        self.temp_node_name = current_name.to_string();
        self.should_select_text = true;
    }

    /// Saves the current name edit to the selected node.
    fn save_node_name_change(&mut self, node_id: NodeId) {
        if let Some(node) = self.flowchart.nodes.get_mut(&node_id) {
            node.name = self.temp_node_name.clone();
            self.has_unsaved_changes = true;
        }
        self.editing_node_name = None;
    }

    /// Renders node type information and type-specific properties.
    fn draw_node_type_info(&self, ui: &mut egui::Ui, node: &FlowchartNode) {
        ui.label(format!("Type: {}", match &node.node_type {
            NodeType::Producer { .. } => "Producer",
            NodeType::Consumer { .. } => "Consumer",
            NodeType::Transformer { .. } => "Transformer",
        }));

        // Type-specific properties
        match &node.node_type {
            NodeType::Producer { generation_rate } => {
                ui.label(format!("Generation Rate: {} msg/step", generation_rate));
            }
            NodeType::Consumer { consumption_rate } => {
                ui.label(format!("Consumption Rate: {} msg/step", consumption_rate));
            }
            NodeType::Transformer { script } => {
                ui.label("Lua Script:");
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
        let screen_pos = egui::pos2(self.context_menu_screen_pos.0, self.context_menu_screen_pos.1);

        let area_response = egui::Area::new(egui::Id::new("context_menu"))
            .fixed_pos(screen_pos)
            .show(ui.ctx(), |ui| {
                egui::Frame::popup(ui.style()).show(ui, |ui| {
                    ui.vertical(|ui| {
                        ui.label("Create Node:");
                        ui.separator();

                        if ui.button("Producer").clicked() {
                            self.create_node_at_pos(NodeType::Producer { generation_rate: 1 });
                            self.show_context_menu = false;
                        }

                        if ui.button("Consumer").clicked() {
                            self.create_node_at_pos(NodeType::Consumer { consumption_rate: 1 });
                            self.show_context_menu = false;
                        }

                        if ui.button("Transformer").clicked() {
                            self.create_node_at_pos(NodeType::Transformer {
                                script: "// Transform the input message\nfunction transform(input) {\n    return { data: input.data };\n}".to_string()
                            });
                            self.show_context_menu = false;
                        }

                        ui.separator();
                        if ui.button("Cancel").clicked() {
                            self.show_context_menu = false;
                        }
                    });
                })
            });

        // Handle click-outside-to-close after the first frame
        if !self.context_menu_just_opened {
            if ui.input(|i| i.pointer.primary_clicked()) {
                if let Some(click_pos) = ui.input(|i| i.pointer.interact_pos()) {
                    if !area_response.response.rect.contains(click_pos) {
                        self.show_context_menu = false;
                    }
                }
            }
        }

        self.context_menu_just_opened = false;
    }

    /// Creates a new node at the context menu position.
    fn create_node_at_pos(&mut self, node_type: NodeType) {
        self.node_counter += 1;

        let new_node = FlowchartNode::new(
            format!("node{}", self.node_counter),
            self.context_menu_world_pos,
            node_type,
        );

        let node_id = new_node.id;
        self.flowchart.add_node(new_node);

        // Select the new node and start editing its name immediately
        self.selected_node = Some(node_id);
        self.start_editing_node_name(node_id, &format!("node{}", self.node_counter));

        // Mark as having unsaved changes
        self.has_unsaved_changes = true;
    }

    /// Opens a file dialog to save the flowchart with a new name.
    fn save_as_flowchart(&mut self) {
        #[cfg(target_arch = "wasm32")]
        {
            self.pending_save_operation = Some(PendingSaveOperation::SaveAs);
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
                        self.current_file_path = Some(path.to_string());
                        self.has_unsaved_changes = false;
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
            self.pending_load_operation = Some(PendingLoadOperation::Load);
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
                                    self.current_file_path = Some(filename);
                                    self.has_unsaved_changes = false;
                                    self.selected_node = None;
                                    self.editing_node_name = None;
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
        if self.current_file_path.is_some() {
            #[cfg(target_arch = "wasm32")]
            {
                self.pending_save_operation = Some(PendingSaveOperation::Save);
            }

            #[cfg(not(target_arch = "wasm32"))]
            {
                if let Some(file_path) = &self.current_file_path.clone() {
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
        self.current_file_path = None;
        self.has_unsaved_changes = false;
        self.selected_node = None;
        self.editing_node_name = None;
        self.node_counter = 0;
        self.canvas_offset = egui::Vec2::ZERO;
        self.zoom_factor = 1.0;
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

        // Handle canvas panning with middle mouse button
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
        if self.show_context_menu {
            self.draw_context_menu(ui);
        }
    }

    /// Handles middle-click canvas panning functionality.
    fn handle_canvas_panning(&mut self, ui: &mut egui::Ui, response: &egui::Response) {
        if ui.input(|i| i.pointer.middle_down()) {
            if let Some(current_pos) = response.interact_pointer_pos() {
                if !self.is_panning {
                    self.is_panning = true;
                    self.last_pan_pos = Some(current_pos);
                } else if let Some(last_pos) = self.last_pan_pos {
                    let delta = current_pos - last_pos;
                    self.canvas_offset += delta;
                    self.last_pan_pos = Some(current_pos);
                }
            }
        } else {
            self.is_panning = false;
            self.last_pan_pos = None;
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
                let old_zoom = self.zoom_factor;
                self.zoom_factor = (self.zoom_factor + zoom_delta).clamp(0.25, 5.0);

                // Only adjust offset if zoom actually changed
                if (self.zoom_factor - old_zoom).abs() > f32::EPSILON {
                    // Calculate where that world position should appear on screen after zoom
                    let world_pos_after_zoom = self.world_to_screen(world_pos_before_zoom);

                    // Adjust canvas offset to keep the world position under the mouse cursor
                    let offset_adjustment = mouse_pos - world_pos_after_zoom;
                    self.canvas_offset += offset_adjustment;
                }
            }
        }
    }

    /// Handles node dragging functionality with left mouse button.
    fn handle_node_dragging(&mut self, ui: &mut egui::Ui, response: &egui::Response) {
        if ui.input(|i| i.pointer.primary_down()) && !self.is_panning {
            if let Some(current_pos) = response.interact_pointer_pos() {
                let world_pos = self.screen_to_world(current_pos);

                if self.dragging_node.is_none() {
                    // Start dragging if over a node
                    if let Some(node_id) = self.find_node_at_position(world_pos) {
                        self.start_node_drag(node_id, current_pos, world_pos);
                    }
                } else if let Some(dragging_id) = self.dragging_node {
                    // Continue dragging - update node position with grid snapping support
                    self.update_dragged_node_position(dragging_id, world_pos, ui);
                }
            }
        } else {
            // Stop dragging when mouse released
            self.dragging_node = None;
            self.drag_start_pos = None;
        }
    }

    /// Starts dragging the specified node.
    fn start_node_drag(&mut self, node_id: NodeId, current_pos: egui::Pos2, world_pos: egui::Pos2) {
        self.dragging_node = Some(node_id);
        self.drag_start_pos = Some(current_pos);

        // Calculate offset from node center to mouse position for smooth dragging
        if let Some(node) = self.flowchart.nodes.get(&node_id) {
            let node_center = egui::pos2(node.position.0, node.position.1);
            self.node_drag_offset = node_center - world_pos;
        }
    }

    /// Updates the position of the currently dragged node.
    fn update_dragged_node_position(&mut self, node_id: NodeId, world_pos: egui::Pos2, ui: &egui::Ui) {
        let mut new_world_pos = world_pos + self.node_drag_offset;

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
        (screen_pos - self.canvas_offset) / self.zoom_factor
    }

    /// Converts world coordinates to screen coordinates accounting for zoom and pan.
    fn world_to_screen(&self, world_pos: egui::Pos2) -> egui::Pos2 {
        world_pos * self.zoom_factor + self.canvas_offset
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
        let screen_grid_size = GRID_SIZE * self.zoom_factor;
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
        if self.show_grid {
            self.draw_grid(painter, canvas_rect);
        }

        // Draw connections second (behind nodes)
        for connection in &self.flowchart.connections {
            self.draw_connection(painter, connection);
        }

        // Draw nodes on top
        for (_id, node) in &self.flowchart.nodes {
            self.draw_node(painter, node);
        }
    }

    /// Handles canvas click interactions for selection and context menu.
    fn handle_canvas_interactions(&mut self, _ui: &mut egui::Ui, response: &egui::Response) {
        // Left-click for selection (only if not dragging or panning)
        if response.clicked() && !self.is_panning && self.dragging_node.is_none() {
            if let Some(pos) = response.interact_pointer_pos() {
                let world_pos = self.screen_to_world(pos);
                self.selected_node = self.find_node_at_position(world_pos);
                self.editing_node_name = None; // Stop editing on click elsewhere
            }
        }

        // Right-click for context menu
        if response.secondary_clicked() && !self.is_panning && self.dragging_node.is_none() {
            if let Some(screen_pos) = response.interact_pointer_pos() {
                let world_pos = self.screen_to_world(screen_pos);
                self.context_menu_screen_pos = (screen_pos.x, screen_pos.y);
                self.context_menu_world_pos = (world_pos.x, world_pos.y);
                self.show_context_menu = true;
                self.context_menu_just_opened = true;
            }
        }
    }

    /// Renders a connection between two nodes with animated messages.
    fn draw_connection(&self, painter: &egui::Painter, connection: &Connection) {
        // Get node positions with zoom and canvas offset applied
        let start_world = self.flowchart.nodes.get(&connection.from)
            .map(|n| egui::pos2(n.position.0, n.position.1))
            .unwrap_or_else(|| egui::pos2(0.0, 0.0));
        let start_pos = self.world_to_screen(start_world);

        let end_world = self.flowchart.nodes.get(&connection.to)
            .map(|n| egui::pos2(n.position.0, n.position.1))
            .unwrap_or_else(|| egui::pos2(100.0, 100.0));
        let end_pos = self.world_to_screen(end_world);

        // Draw the connection line
        painter.line_segment(
            [start_pos, end_pos],
            egui::Stroke::new(2.0, egui::Color32::DARK_GRAY)
        );

        // Draw messages as animated dots along the connection
        for message in &connection.messages {
            let msg_pos = start_pos + (end_pos - start_pos) * message.position_along_edge;
            let scaled_radius = 3.0 * self.zoom_factor;
            painter.circle_filled(msg_pos, scaled_radius, egui::Color32::YELLOW);
        }
    }

    /// Renders a single flowchart node with appropriate styling and text.
    fn draw_node(&self, painter: &egui::Painter, node: &FlowchartNode) {
        const NODE_SIZE: egui::Vec2 = egui::Vec2::new(100.0, 70.0);

        // Apply zoom and canvas offset for proper positioning
        let world_pos = egui::pos2(node.position.0, node.position.1);
        let screen_pos = self.world_to_screen(world_pos);
        let scaled_size = NODE_SIZE * self.zoom_factor;
        let rect = egui::Rect::from_center_size(screen_pos, scaled_size);

        // Determine node color based on type
        let mut color = match node.node_type {
            NodeType::Producer { .. } => egui::Color32::LIGHT_GREEN,
            NodeType::Consumer { .. } => egui::Color32::LIGHT_RED,
            NodeType::Transformer { .. } => egui::Color32::LIGHT_BLUE,
        };

        // Darken color if being dragged
        if Some(node.id) == self.dragging_node {
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
        let (stroke_color, stroke_width) = if Some(node.id) == self.dragging_node {
            (egui::Color32::from_rgb(255, 165, 0), 4.0) // Orange for dragging
        } else if Some(node.id) == self.selected_node {
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
            egui::pos2(pos.x, pos.y - 5.0 * self.zoom_factor),
            egui::vec2(size.x - 10.0 * self.zoom_factor, size.y - 20.0 * self.zoom_factor) // Leave padding
        );

        // Create zoom-aware font size
        let base_font_size = 12.0;
        let scaled_font_size = (base_font_size * self.zoom_factor).clamp(8.0, 48.0);
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
