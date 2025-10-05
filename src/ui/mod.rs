//! User interface components and rendering logic for the flowchart tool.
//!
//! This module contains all the UI-related code including the main application struct,
//! canvas rendering, property panels, context menus, and user interaction handling.
//!
//! # Module Organization
//!
//! - `highlighters` - Syntax highlighting for JavaScript and JSON
//! - `state` - Application state structures and the main FlowchartApp
//! - `file_ops` - File save/load operations for native and WASM
//! - `canvas` - Canvas navigation, zooming, panning, and interaction
//! - `rendering` - Drawing nodes, connections, grid, and UI elements

mod canvas;
mod file_ops;
mod highlighters;
mod rendering;
mod state;
mod undo;

pub use state::FlowchartApp;
pub use undo::{UndoAction, UndoHistory, UndoableFlowchart};

use crate::simulation::SimulationEngine;
use crate::types::*;
use eframe::egui;
use egui::TextBuffer;

impl eframe::App for FlowchartApp {
    /// Main update function called by egui for each frame.
    ///
    /// This method handles the overall UI layout, including the properties panel,
    /// toolbar, and main canvas area. It also processes simulation steps when running.
    ///
    /// # Arguments
    ///
    /// * `ctx` - The egui context
    /// * `_frame` - The eframe frame (unused)
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // Handle pending file operations
        self.handle_pending_operations(ctx);

        // Handle undo/redo keyboard shortcuts
        self.handle_undo_redo_keys(ctx);

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
                match self
                    .simulation_engine
                    .deliver_message(node_id, message, &mut self.flowchart)
                {
                    Ok(_) => {}
                    Err(error_msg) => {
                        // Stop simulation on error
                        self.is_simulation_running = false;
                        self.flowchart.simulation_state = SimulationState::Stopped;
                        self.error_node = Some(node_id);
                        eprintln!(
                            "Simulation stopped due to error in node {}: {}",
                            node_id, error_msg
                        );
                    }
                }
            }

            self.frame_counter += 1;
            ctx.request_repaint(); // Keep animating
        } else if self.error_node.is_some() {
            // Keep repainting to show flashing error border
            self.frame_counter += 1;
            ctx.request_repaint();
        }
    }
}

impl FlowchartApp {
    /// Handles undo/redo keyboard shortcuts.
    ///
    /// # Arguments
    ///
    /// * `ctx` - The egui context for checking input
    fn handle_undo_redo_keys(&mut self, ctx: &egui::Context) {
        // Check if any text edit widget wants keyboard focus - if so, don't handle undo/redo
        let is_editing_text = ctx.wants_keyboard_input();

        if !is_editing_text {
            // Ctrl+Z for undo
            if ctx
                .input(|i| i.key_pressed(egui::Key::Z) && i.modifiers.command && !i.modifiers.shift)
            {
                self.perform_undo();
            }
            // Ctrl+Shift+Z or Ctrl+Y for redo
            else if ctx.input(|i| {
                (i.key_pressed(egui::Key::Z) && i.modifiers.command && i.modifiers.shift)
                    || (i.key_pressed(egui::Key::Y) && i.modifiers.command)
            }) {
                self.perform_redo();
            }
        }
    }

    /// Handles delete key presses to remove selected nodes or connections.
    ///
    /// # Arguments
    ///
    /// * `ctx` - The egui context for checking input
    fn handle_delete_key(&mut self, ctx: &egui::Context) {
        // Check if any text edit widget wants keyboard focus - if so, don't handle delete
        let is_editing_text = ctx.wants_keyboard_input();

        if ctx.input(|i| i.key_pressed(egui::Key::Delete)) && !is_editing_text {
            if let Some(selected_node) = self.interaction.selected_node {
                // Store node and its connections for undo
                if let Some(node) = self.flowchart.nodes.get(&selected_node).cloned() {
                    let connections: Vec<Connection> = self
                        .flowchart
                        .connections
                        .iter()
                        .filter(|c| c.from == selected_node || c.to == selected_node)
                        .cloned()
                        .collect();

                    // Record undo action before deletion
                    self.undo_history
                        .push_action(UndoAction::NodeDeleted { node, connections });
                }

                // Remove the selected node
                self.flowchart.nodes.remove(&selected_node);

                // Remove all connections involving this node
                self.flowchart
                    .connections
                    .retain(|c| c.from != selected_node && c.to != selected_node);

                // Clear selection
                self.interaction.selected_node = None;
                self.interaction.editing_node_name = None;
                self.file.has_unsaved_changes = true;
            } else if let Some(conn_idx) = self.interaction.selected_connection {
                // Remove the selected connection
                if conn_idx < self.flowchart.connections.len() {
                    let connection = self.flowchart.connections[conn_idx].clone();

                    // Record undo action before deletion
                    self.undo_history
                        .push_action(UndoAction::ConnectionDeleted {
                            connection,
                            index: conn_idx,
                        });

                    self.flowchart.connections.remove(conn_idx);
                    self.interaction.selected_connection = None;
                    self.file.has_unsaved_changes = true;
                }
            }
        }
    }

    /// Renders the toolbar with file operations, simulation controls, and view options.
    ///
    /// # Arguments
    ///
    /// * `ui` - The egui UI context
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

            // Undo/Redo operations
            ui.add_enabled_ui(self.undo_history.can_undo(), |ui| {
                if ui.button("⟲ Undo").clicked() {
                    self.perform_undo();
                }
            });
            ui.add_enabled_ui(self.undo_history.can_redo(), |ui| {
                if ui.button("⟳ Redo").clicked() {
                    self.perform_redo();
                }
            });

            ui.separator();

            // Simulation controls
            if self.is_simulation_running {
                if ui.button("Pause").clicked() {
                    self.is_simulation_running = false;
                    self.flowchart.simulation_state = SimulationState::Paused;
                }
            } else if ui.button("Start").clicked() {
                self.is_simulation_running = true;
                self.flowchart.simulation_state = SimulationState::Running;
            }
            if ui.button("Stop").clicked() {
                self.is_simulation_running = false;
                self.flowchart.simulation_state = SimulationState::Stopped;
                self.flowchart.current_step = 0;
                self.error_node = None;
                // Clear all messages from connections
                for connection in &mut self.flowchart.connections {
                    connection.messages.clear();
                }
                // Reset producer counters and node states
                for node in self.flowchart.nodes.values_mut() {
                    node.state = NodeState::Idle;
                    if let NodeType::Producer {
                        messages_produced, ..
                    } = &mut node.node_type
                    {
                        *messages_produced = 0;
                    }
                }
            }
            if ui.button("Step").clicked() {
                let delivered_messages = self.simulation_engine.step(&mut self.flowchart);
                for (node_id, message) in delivered_messages {
                    match self.simulation_engine.deliver_message(
                        node_id,
                        message,
                        &mut self.flowchart,
                    ) {
                        Ok(_) => {}
                        Err(error_msg) => {
                            self.error_node = Some(node_id);
                            eprintln!("Error in node {}: {}", node_id, error_msg);
                        }
                    }
                }
            }

            ui.separator();

            // Layout operations
            if ui.button("Auto Layout").clicked() {
                self.auto_layout_graph();
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
                    let status = if self.file.has_unsaved_changes {
                        "*"
                    } else {
                        ""
                    };
                    ui.label(format!("{}{}", file_path, status));
                } else {
                    let status = if self.file.has_unsaved_changes {
                        "Untitled*"
                    } else {
                        "Untitled"
                    };
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
    ///
    /// # Arguments
    ///
    /// * `ui` - The egui UI context
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
    ///
    /// # Arguments
    ///
    /// * `ui` - The egui UI context
    /// * `connection` - The connection to display properties for
    fn draw_connection_properties(&self, ui: &mut egui::Ui, connection: &Connection) {
        ui.label("Type: Connection");
        ui.separator();

        // Show from and to node names
        if let Some(from_node) = self.flowchart.nodes.get(&connection.from) {
            ui.label(format!("From: {}", from_node.name));
        } else {
            ui.label("From: (node not found)");
        }

        if let Some(to_node) = self.flowchart.nodes.get(&connection.to) {
            ui.label(format!("To: {}", to_node.name));
        } else {
            ui.label("To: (node not found)");
        }

        ui.separator();
        ui.label(format!(
            "Messages in transit: {}",
            connection.messages.len()
        ));

        // Show message contents
        if !connection.messages.is_empty() {
            ui.separator();
            ui.label("Message Contents:");

            egui::ScrollArea::vertical()
                .max_height(300.0)
                .show(ui, |ui| {
                    for (idx, message) in connection.messages.iter().enumerate() {
                        ui.push_id(idx, |ui| {
                            egui::CollapsingHeader::new(format!("Message {}", idx + 1))
                                .default_open(false)
                                .show(ui, |ui| {
                                    // Display message as formatted JSON
                                    let json_str = serde_json::to_string_pretty(&message.data)
                                        .unwrap_or_else(|_| format!("{:?}", message.data));

                                    ui.add(
                                        egui::TextEdit::multiline(&mut json_str.as_str())
                                            .desired_rows(5)
                                            .desired_width(f32::INFINITY)
                                            .code_editor()
                                            .interactive(false),
                                    );
                                });
                        });
                    }
                });
        }

        ui.separator();
        ui.colored_label(egui::Color32::GRAY, "Press Delete to remove");
    }

    /// Renders the name editing field for a node.
    ///
    /// # Arguments
    ///
    /// * `ui` - The egui UI context
    /// * `selected_id` - ID of the node being edited
    fn draw_name_editor(&mut self, ui: &mut egui::Ui, selected_id: NodeId) {
        let response = ui.text_edit_singleline(&mut self.interaction.temp_node_name);

        // Only request focus on the first frame of editing
        if !self.interaction.focus_requested_for_edit {
            response.request_focus();
            self.interaction.focus_requested_for_edit = true;
        }

        // Select all text when flag is set and field has focus
        if self.interaction.should_select_text && response.has_focus() {
            self.interaction.should_select_text = false;
            self.select_all_text_in_field(ui, response.id);
        }

        // Handle Enter key to save changes
        if response.has_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
            self.save_node_name_change(selected_id);
        }

        // Check if focus was lost (but not due to Enter key which we handle above)
        if response.lost_focus() && !ui.input(|i| i.key_pressed(egui::Key::Enter)) {
            // Save changes when focus is lost
            self.save_node_name_change(selected_id);
        }
    }

    /// Selects all text in a text edit field using egui's internal state.
    ///
    /// # Arguments
    ///
    /// * `ui` - The egui UI context
    /// * `field_id` - The ID of the text field
    fn select_all_text_in_field(&self, ui: &mut egui::Ui, field_id: egui::Id) {
        ui.memory_mut(|mem| {
            let state = mem
                .data
                .get_temp_mut_or_default::<egui::text_edit::TextEditState>(field_id);
            let text_len = self.interaction.temp_node_name.len();
            state
                .cursor
                .set_char_range(Some(egui::text::CCursorRange::two(
                    egui::text::CCursor::new(0),
                    egui::text::CCursor::new(text_len),
                )));
        });
    }

    /// Starts editing the name of the specified node.
    ///
    /// # Arguments
    ///
    /// * `node_id` - ID of the node to edit
    /// * `current_name` - Current name of the node
    fn start_editing_node_name(&mut self, node_id: NodeId, current_name: &str) {
        self.interaction.editing_node_name = Some(node_id);
        self.interaction.temp_node_name = current_name.to_string();
        self.interaction.should_select_text = true;
        self.interaction.focus_requested_for_edit = false;
    }

    /// Saves the current name edit to the selected node.
    ///
    /// # Arguments
    ///
    /// * `node_id` - ID of the node to update
    fn save_node_name_change(&mut self, node_id: NodeId) {
        if let Some(node) = self.flowchart.nodes.get_mut(&node_id) {
            let old_name = node.name.clone();
            let new_name = self.interaction.temp_node_name.clone();

            // Only record undo if name actually changed
            if old_name != new_name {
                self.undo_history.push_action(UndoAction::NodeRenamed {
                    node_id,
                    old_name,
                    new_name: new_name.clone(),
                });
                node.name = new_name;
                self.file.has_unsaved_changes = true;
            }
        }
        self.interaction.editing_node_name = None;
    }

    /// Updates a producer node property from the temporary editing values.
    ///
    /// # Arguments
    ///
    /// * `node_id` - ID of the node to update
    /// * `property` - Name of the property to update
    fn update_producer_property(&mut self, node_id: NodeId, property: &str) {
        if let Some(node) = self.flowchart.nodes.get(&node_id) {
            let old_node_type = node.node_type.clone();
            let mut changed = false;

            if let NodeType::Producer {
                mut message_template,
                mut start_step,
                mut messages_per_cycle,
                mut steps_between_cycles,
                messages_produced,
            } = node.node_type.clone()
            {
                match property {
                    "start_step" => {
                        if let Ok(value) = self.interaction.temp_producer_start_step.parse::<u64>()
                        {
                            if start_step != value {
                                start_step = value;
                                changed = true;
                            }
                        }
                    }
                    "messages_per_cycle" => {
                        if let Ok(value) = self
                            .interaction
                            .temp_producer_messages_per_cycle
                            .parse::<u32>()
                        {
                            if messages_per_cycle != value {
                                messages_per_cycle = value;
                                changed = true;
                            }
                        }
                    }
                    "steps_between_cycles" => {
                        if let Ok(value) =
                            self.interaction.temp_producer_steps_between.parse::<u32>()
                        {
                            if steps_between_cycles != value {
                                steps_between_cycles = value;
                                changed = true;
                            }
                        }
                    }
                    "message_template" => {
                        if let Ok(value) = serde_json::from_str::<serde_json::Value>(
                            &self.interaction.temp_producer_message_template,
                        ) {
                            if message_template != value {
                                message_template = value;
                                changed = true;
                            }
                        }
                    }
                    _ => {}
                }

                if changed {
                    let new_node_type = NodeType::Producer {
                        message_template,
                        start_step,
                        messages_per_cycle,
                        steps_between_cycles,
                        messages_produced,
                    };

                    // Record undo action
                    self.undo_history.push_action(UndoAction::PropertyChanged {
                        node_id,
                        old_node_type,
                        new_node_type: new_node_type.clone(),
                    });

                    // Apply the change
                    if let Some(node) = self.flowchart.nodes.get_mut(&node_id) {
                        node.node_type = new_node_type;
                    }
                    self.file.has_unsaved_changes = true;
                }
            }
        }
    }

    /// Updates a transformer node property from the temporary editing values.
    ///
    /// # Arguments
    ///
    /// * `node_id` - ID of the node to update
    /// * `property` - Name of the property to update
    fn update_transformer_property(&mut self, node_id: NodeId, property: &str) {
        if let Some(node) = self.flowchart.nodes.get(&node_id) {
            if let NodeType::Transformer { script } = &node.node_type {
                if property == "script" {
                    let new_script = self
                        .interaction
                        .temp_transformer_script
                        .replace("\t", "    ");
                    // Only record undo if script actually changed
                    if script != &new_script {
                        let old_node_type = node.node_type.clone();
                        let new_node_type = NodeType::Transformer { script: new_script };

                        // Record undo action
                        self.undo_history.push_action(UndoAction::PropertyChanged {
                            node_id,
                            old_node_type,
                            new_node_type: new_node_type.clone(),
                        });

                        // Apply the change
                        if let Some(node) = self.flowchart.nodes.get_mut(&node_id) {
                            node.node_type = new_node_type;
                        }
                        self.file.has_unsaved_changes = true;
                    }
                }
            }
        }
    }

    /// Renders node type information and type-specific properties.
    ///
    /// # Arguments
    ///
    /// * `ui` - The egui UI context
    /// * `node` - The node to display information for
    fn draw_node_type_info(&mut self, ui: &mut egui::Ui, node: &FlowchartNode) {
        ui.label(format!(
            "Type: {}",
            match &node.node_type {
                NodeType::Producer { .. } => "Producer",
                NodeType::Consumer { .. } => "Consumer",
                NodeType::Transformer { .. } => "Transformer",
            }
        ));

        // Type-specific properties
        match &node.node_type {
            NodeType::Producer {
                message_template,
                start_step,
                messages_per_cycle,
                steps_between_cycles,
                messages_produced,
            } => {
                // Initialize temp values if empty
                if self.interaction.temp_producer_start_step.is_empty() {
                    self.interaction.temp_producer_start_step = start_step.to_string();
                }
                if self.interaction.temp_producer_messages_per_cycle.is_empty() {
                    self.interaction.temp_producer_messages_per_cycle =
                        messages_per_cycle.to_string();
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
                if ui
                    .text_edit_singleline(&mut self.interaction.temp_producer_start_step)
                    .changed()
                {
                    self.update_producer_property(node.id, "start_step");
                }

                ui.label("Total Messages:");
                if ui
                    .text_edit_singleline(&mut self.interaction.temp_producer_messages_per_cycle)
                    .changed()
                {
                    self.update_producer_property(node.id, "messages_per_cycle");
                }

                ui.label(format!(
                    "Messages Produced: {}/{}",
                    messages_produced, messages_per_cycle
                ));

                ui.label("Steps Between Cycles:");
                if ui
                    .text_edit_singleline(&mut self.interaction.temp_producer_steps_between)
                    .changed()
                {
                    self.update_producer_property(node.id, "steps_between_cycles");
                }

                ui.separator();
                ui.label("Message Template (JSON):");

                // Store a reference for the layouter and a mutable copy for editing
                let layouter_ref = self.interaction.temp_producer_message_template.clone();
                let mut layouter = rendering::create_json_layouter(&layouter_ref);

                let text_edit_response = ui.add(
                    egui::TextEdit::multiline(&mut self.interaction.temp_producer_message_template)
                        .desired_rows(5)
                        .desired_width(f32::INFINITY)
                        .font(egui::TextStyle::Monospace)
                        .lock_focus(true)
                        .layouter(&mut layouter),
                );

                // Handle Tab key to insert 4 spaces
                if text_edit_response.has_focus() && ui.input(|i| i.key_pressed(egui::Key::Tab)) {
                    // Get cursor position from text edit state
                    let cursor_pos = ui
                        .memory(|mem| {
                            mem.data
                                .get_temp::<egui::text_edit::TextEditState>(text_edit_response.id)
                                .and_then(|state| state.cursor.char_range())
                                .map(|range| range.primary.index)
                        })
                        .unwrap_or(self.interaction.temp_producer_message_template.len());

                    self.interaction
                        .temp_producer_message_template
                        .insert_str(cursor_pos, "    ");
                    self.update_producer_property(node.id, "message_template");
                } else if text_edit_response.changed() {
                    self.update_producer_property(node.id, "message_template");
                }
            }
            NodeType::Consumer { consumption_rate } => {
                ui.label(format!("Consumption Rate: {} msg/step", consumption_rate));
            }
            NodeType::Transformer { script } => {
                // Initialize temp value if empty
                if self.interaction.temp_transformer_script.is_empty() {
                    self.interaction.temp_transformer_script = script.clone();
                }

                ui.label("JavaScript Script:");

                // Store a reference for the layouter and a mutable copy for editing
                let layouter_ref = self.interaction.temp_transformer_script.clone();
                let mut layouter = rendering::create_js_layouter(&layouter_ref);

                let text_edit_response = ui.add(
                    egui::TextEdit::multiline(&mut self.interaction.temp_transformer_script)
                        .desired_rows(10)
                        .desired_width(f32::INFINITY)
                        .font(egui::TextStyle::Monospace)
                        .lock_focus(true)
                        .layouter(&mut layouter),
                );

                if text_edit_response.changed() {
                    self.update_transformer_property(node.id, "script");
                }
            }
        }
    }

    /// Renders node status information including state and position.
    ///
    /// # Arguments
    ///
    /// * `ui` - The egui UI context
    /// * `node` - The node to display status for
    fn draw_node_status_info(&self, ui: &mut egui::Ui, node: &FlowchartNode) {
        ui.label(format!("State: {:?}", node.state));
        ui.label(format!(
            "Position: ({:.1}, {:.1})",
            node.position.0, node.position.1
        ));
    }

    /// Renders information shown when no node is selected.
    ///
    /// # Arguments
    ///
    /// * `ui` - The egui UI context
    fn draw_no_selection_info(&self, ui: &mut egui::Ui) {
        ui.label("No node selected");
        ui.separator();
        ui.label("Left-click on a node to select it");
        ui.label("Right-click on canvas to create nodes");
        ui.label("Middle-click and drag to pan");
    }

    /// Renders the right-click context menu for creating nodes.
    ///
    /// # Arguments
    ///
    /// * `ui` - The egui UI context
    fn draw_context_menu(&mut self, ui: &mut egui::Ui) {
        // Use the stored screen coordinates for menu positioning
        let screen_pos = egui::pos2(
            self.context_menu.screen_pos.0,
            self.context_menu.screen_pos.1,
        );

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
                                steps_between_cycles: 1,
                                messages_produced: 0,
                            });
                            self.context_menu.show = false;
                        }

                        if ui.button("Consumer").clicked() {
                            self.create_node_at_pos(NodeType::Consumer { consumption_rate: 1 });
                            self.context_menu.show = false;
                        }

                        if ui.button("Transformer").clicked() {
                            self.create_node_at_pos(NodeType::Transformer {
                                script: "// Transform the input message\nfunction transform(input) {\n    //Just forward it on.\n    return input;\n}".to_string()
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
        if !self.context_menu.just_opened && ui.input(|i| i.pointer.primary_clicked()) {
            if let Some(click_pos) = ui.input(|i| i.pointer.interact_pos()) {
                if !area_response.response.rect.contains(click_pos) {
                    self.context_menu.show = false;
                }
            }
        }

        self.context_menu.just_opened = false;
    }

    /// Creates a new node at the context menu position.
    ///
    /// # Arguments
    ///
    /// * `node_type` - The type of node to create
    fn create_node_at_pos(&mut self, node_type: NodeType) {
        self.node_counter += 1;

        let new_node = FlowchartNode::new(
            format!("node{}", self.node_counter),
            self.context_menu.world_pos,
            node_type,
        );

        let node_id = new_node.id;
        self.flowchart.add_node(new_node);

        // Record undo action for node creation
        self.undo_history
            .push_action(UndoAction::NodeCreated { node_id });

        // Select the new node and start editing its name immediately
        self.interaction.selected_node = Some(node_id);
        self.start_editing_node_name(node_id, &format!("node{}", self.node_counter));

        // Mark as having unsaved changes
        self.file.has_unsaved_changes = true;
    }

    /// Renders the main canvas area with nodes, connections, and handles user interactions.
    ///
    /// # Arguments
    ///
    /// * `ui` - The egui UI context
    fn draw_canvas(&mut self, ui: &mut egui::Ui) {
        let (response, painter) =
            ui.allocate_painter(ui.available_size(), egui::Sense::click_and_drag());

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

    /// Handles canvas click interactions for selection and context menu.
    ///
    /// # Arguments
    ///
    /// * `_ui` - The egui UI context (unused)
    /// * `response` - The canvas response
    fn handle_canvas_interactions(&mut self, _ui: &mut egui::Ui, response: &egui::Response) {
        // Left-click for selection (only if not dragging or panning)
        if response.clicked()
            && !self.interaction.is_panning
            && self.interaction.dragging_node.is_none()
        {
            if let Some(pos) = response.interact_pointer_pos() {
                let world_pos = self.screen_to_world(pos);

                // First try to select a node
                if let Some(node_id) = self.find_node_at_position(world_pos) {
                    self.interaction.selected_node = Some(node_id);
                    self.interaction.selected_connection = None;
                    self.interaction.editing_node_name = None;
                    // Clear temp producer values to reload from selected node
                    self.clear_temp_editing_values();
                } else {
                    // Try to select a connection
                    if let Some(conn_idx) = self.find_connection_at_position(world_pos) {
                        self.interaction.selected_connection = Some(conn_idx);
                        self.interaction.selected_node = None;
                        self.interaction.editing_node_name = None;
                        self.clear_temp_editing_values();
                    } else {
                        // Clear selection if clicking on empty space
                        self.interaction.selected_node = None;
                        self.interaction.selected_connection = None;
                        self.interaction.editing_node_name = None;
                        self.clear_temp_editing_values();
                    }
                }
            }
        }

        // Right-click for context menu
        if response.secondary_clicked()
            && !self.interaction.is_panning
            && self.interaction.dragging_node.is_none()
        {
            if let Some(screen_pos) = response.interact_pointer_pos() {
                let world_pos = self.screen_to_world(screen_pos);
                self.context_menu.screen_pos = (screen_pos.x, screen_pos.y);
                self.context_menu.world_pos = (world_pos.x, world_pos.y);
                self.context_menu.show = true;
                self.context_menu.just_opened = true;
            }
        }
    }

    /// Clears all temporary editing values for node properties.
    fn clear_temp_editing_values(&mut self) {
        self.interaction.temp_producer_start_step.clear();
        self.interaction.temp_producer_messages_per_cycle.clear();
        self.interaction.temp_producer_steps_between.clear();
        self.interaction.temp_producer_message_template.clear();
        self.interaction.temp_transformer_script.clear();
    }

    /// Performs an undo operation.
    fn perform_undo(&mut self) {
        if let Some(action) = self.undo_history.pop_undo() {
            if let Some(redo_action) = self.flowchart.apply_undo(&action) {
                self.file.has_unsaved_changes = true;

                // Clear selection and temp values to refresh UI
                self.interaction.selected_node = None;
                self.interaction.selected_connection = None;
                self.clear_temp_editing_values();
            }
        }
    }

    /// Performs a redo operation.
    fn perform_redo(&mut self) {
        if let Some(action) = self.undo_history.pop_redo() {
            if let Some(undo_action) = self.flowchart.apply_redo(&action) {
                self.undo_history.push_action(undo_action);
                self.file.has_unsaved_changes = true;

                // Clear selection and temp values to refresh UI
                self.interaction.selected_node = None;
                self.interaction.selected_connection = None;
                self.clear_temp_editing_values();
            }
        }
    }

    /// Automatically organizes nodes using a force-directed layout algorithm.
    /// 
    /// This method applies forces to nodes to create an aesthetically pleasing layout:
    /// - Repulsion between all nodes (to prevent overlap)
    /// - Attraction along connections (to keep connected nodes together)
    /// - Centers the final layout around the origin (0, 0)
    /// 
    /// The algorithm accounts for node size (100x70) and adds extra spacing
    /// to ensure connections are visible between nodes.
    fn auto_layout_graph(&mut self) {
        if self.flowchart.nodes.is_empty() {
            return;
        }

        // Store original positions for undo
        let original_positions: Vec<(NodeId, (f32, f32))> = self
            .flowchart
            .nodes
            .iter()
            .map(|(id, node)| (*id, node.position))
            .collect();

        // Constants for the force-directed algorithm
        const ITERATIONS: usize = 500;
        const REPULSION_STRENGTH: f32 = 50000.0;
        const ATTRACTION_STRENGTH: f32 = 0.08;
        const DAMPING: f32 = 0.85;

        // Node dimensions and spacing
        const NODE_WIDTH: f32 = 100.0;
        const NODE_HEIGHT: f32 = 70.0;
        const SPACING_BUFFER: f32 = 10.0; // Extra space between nodes for connections

        // Calculate minimum safe distance between node centers
        // Using diagonal distance plus buffer for more natural spacing
        let min_distance: f32 = ((NODE_WIDTH * NODE_WIDTH + NODE_HEIGHT * NODE_HEIGHT).sqrt()
            + SPACING_BUFFER * 2.0);

        // Initialize velocities for all nodes
        let mut velocities: std::collections::HashMap<NodeId, (f32, f32)> =
            std::collections::HashMap::new();
        for node_id in self.flowchart.nodes.keys() {
            velocities.insert(*node_id, (0.0, 0.0));
        }

        // Run simulation iterations
        for _ in 0..ITERATIONS {
            // Calculate forces for each node
            let mut forces: std::collections::HashMap<NodeId, (f32, f32)> =
                std::collections::HashMap::new();

            // Initialize all forces to zero
            for node_id in self.flowchart.nodes.keys() {
                forces.insert(*node_id, (0.0, 0.0));
            }

            // Repulsion forces between all pairs of nodes
            let node_ids: Vec<NodeId> = self.flowchart.nodes.keys().copied().collect();
            for i in 0..node_ids.len() {
                for j in (i + 1)..node_ids.len() {
                    let id1 = node_ids[i];
                    let id2 = node_ids[j];

                    if let (Some(node1), Some(node2)) = 
                        (self.flowchart.nodes.get(&id1), self.flowchart.nodes.get(&id2)) {
                        let dx = node1.position.0 - node2.position.0;
                        let dy = node1.position.1 - node2.position.1;
                        let distance = (dx * dx + dy * dy).sqrt().max(1.0);

                        // Stronger repulsion force when nodes are closer than minimum distance
                        let force_magnitude = if distance < min_distance {
                            // Extra strong repulsion to prevent overlaps
                            REPULSION_STRENGTH / (distance * distance) * 2.0
                        } else {
                            REPULSION_STRENGTH / (distance * distance)
                        };

                        let fx = (dx / distance) * force_magnitude;
                        let fy = (dy / distance) * force_magnitude;

                        // Apply equal and opposite forces
                        let force1 = forces.get(&id1).unwrap();
                        forces.insert(id1, (force1.0 + fx, force1.1 + fy));

                        let force2 = forces.get(&id2).unwrap();
                        forces.insert(id2, (force2.0 - fx, force2.1 - fy));
                    }
                }
            }

            // Attraction forces along connections
            for connection in &self.flowchart.connections {
                if let (Some(from_node), Some(to_node)) = (
                    self.flowchart.nodes.get(&connection.from),
                    self.flowchart.nodes.get(&connection.to),
                ) {
                    let dx = to_node.position.0 - from_node.position.0;
                    let dy = to_node.position.1 - from_node.position.1;
                    let distance = (dx * dx + dy * dy).sqrt().max(1.0);

                    // Spring force proportional to distance, but weaker for very close nodes
                    let ideal_distance = min_distance * 1.5; // Prefer nodes to be a bit farther than minimum
                    let displacement = distance - ideal_distance;
                    let fx = (dx / distance) * displacement * ATTRACTION_STRENGTH;
                    let fy = (dy / distance) * displacement * ATTRACTION_STRENGTH;

                    // Apply forces
                    let force_from = forces.get(&connection.from).unwrap();
                    forces.insert(connection.from, (force_from.0 + fx, force_from.1 + fy));

                    let force_to = forces.get(&connection.to).unwrap();
                    forces.insert(connection.to, (force_to.0 - fx, force_to.1 - fy));
                }
            }

            // Update velocities and positions
            for (node_id, force) in &forces {
                if let Some(node) = self.flowchart.nodes.get_mut(node_id) {
                    let velocity = velocities.get_mut(node_id).unwrap();

                    // Update velocity with damping
                    velocity.0 = (velocity.0 + force.0) * DAMPING;
                    velocity.1 = (velocity.1 + force.1) * DAMPING;

                    // Update position
                    node.position.0 += velocity.0;
                    node.position.1 += velocity.1;
                }
            }
        }

        // Center the layout around the origin
        if !self.flowchart.nodes.is_empty() {
            // Calculate center of mass
            let mut center_x = 0.0;
            let mut center_y = 0.0;
            let node_count = self.flowchart.nodes.len() as f32;

            for node in self.flowchart.nodes.values() {
                center_x += node.position.0;
                center_y += node.position.1;
            }

            center_x /= node_count;
            center_y /= node_count;

            // Shift all nodes to center the layout at origin
            for node in self.flowchart.nodes.values_mut() {
                node.position.0 -= center_x;
                node.position.1 -= center_y;
            }
        }

        // Record undo action for the layout operation
        self.undo_history.push_action(UndoAction::MultipleNodesMoved {
            moves: original_positions,
        });

        self.file.has_unsaved_changes = true;
    }
}
