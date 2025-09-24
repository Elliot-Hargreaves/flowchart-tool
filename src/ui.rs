//! User interface components and rendering logic for the flowchart tool.
//! 
//! This module contains all the UI-related code including the main application struct,
//! canvas rendering, property panels, context menus, and user interaction handling.

use crate::types::*;
use crate::simulation::SimulationEngine;
use eframe::egui;

/// The main application structure containing UI state and the flowchart data.
/// 
/// This struct implements the `eframe::App` trait and handles all user interface
/// rendering and interaction logic.
pub struct FlowchartApp {
    /// The flowchart being edited and simulated
    flowchart: Flowchart,
    /// Simulation engine for processing flowchart steps
    simulation_engine: SimulationEngine,
    /// Currently selected node ID, if any
    selected_node: Option<NodeId>,
    /// Whether the simulation is currently running
    is_simulation_running: bool,
    /// Speed multiplier for simulation (currently unused)
    simulation_speed: f32,
    /// Whether the context menu is currently visible
    show_context_menu: bool,
    /// Canvas position where the context menu should appear
    context_menu_pos: (f32, f32),
    /// Counter for generating unique default node names
    node_counter: u32,
    /// Node currently being edited for name changes
    editing_node_name: Option<NodeId>,
    /// Temporary storage for node name while editing
    temp_node_name: String,
    /// Current canvas pan offset for navigation
    canvas_offset: egui::Vec2,
    /// Whether the user is currently panning the canvas
    is_panning: bool,
    /// Last mouse position during panning operation
    last_pan_pos: Option<egui::Pos2>,
    /// Flag to prevent context menu from closing immediately after opening
    context_menu_just_opened: bool,
    /// Flag indicating text should be selected in the name field
    should_select_text: bool,
    /// Node currently being dragged by the user
    dragging_node: Option<NodeId>,
    /// Initial mouse position when drag started
    drag_start_pos: Option<egui::Pos2>,
    /// Offset from mouse to node center during dragging
    node_drag_offset: egui::Vec2,
}

impl Default for FlowchartApp {
    fn default() -> Self {
        Self {
            flowchart: Flowchart::default(),
            simulation_engine: SimulationEngine::new(),
            selected_node: None,
            is_simulation_running: false,
            simulation_speed: 1.0,
            show_context_menu: false,
            context_menu_pos: (0.0, 0.0),
            node_counter: 0,
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
        }
    }
}

impl eframe::App for FlowchartApp {
    /// Main update function called by egui for each frame.
    /// 
    /// This method handles the overall UI layout, including the properties panel,
    /// toolbar, and main canvas area. It also processes simulation steps when running.
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
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
    /// Renders the toolbar with simulation control buttons.
    /// 
    /// The toolbar contains Start, Stop, and Step buttons for controlling
    /// the flowchart simulation.
    fn draw_toolbar(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
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
        // Convert canvas coordinates to screen coordinates for menu positioning
        let screen_pos = egui::pos2(self.context_menu_pos.0, self.context_menu_pos.1) + self.canvas_offset;

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
                                script: "-- Transform the input message\noutput = {create_message(input.data)}".to_string() 
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
            self.context_menu_pos,
            node_type,
        );

        let node_id = new_node.id;
        self.flowchart.add_node(new_node);

        // Select the new node and start editing its name immediately
        self.selected_node = Some(node_id);
        self.start_editing_node_name(node_id, &format!("node{}", self.node_counter));
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

        // Handle node dragging with left mouse button
        self.handle_node_dragging(ui, &response);

        // Render all flowchart elements
        self.render_flowchart_elements(&painter);

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

    /// Handles node dragging functionality with left mouse button.
    fn handle_node_dragging(&mut self, ui: &mut egui::Ui, response: &egui::Response) {
        if ui.input(|i| i.pointer.primary_down()) && !self.is_panning {
            if let Some(current_pos) = response.interact_pointer_pos() {
                let canvas_pos = current_pos - self.canvas_offset;

                if self.dragging_node.is_none() {
                    // Start dragging if over a node
                    if let Some(node_id) = self.find_node_at_position(canvas_pos) {
                        self.start_node_drag(node_id, current_pos, canvas_pos);
                    }
                } else if let Some(dragging_id) = self.dragging_node {
                    // Continue dragging - update node position
                    self.update_dragged_node_position(dragging_id, canvas_pos);
                }
            }
        } else {
            // Stop dragging when mouse released
            self.dragging_node = None;
            self.drag_start_pos = None;
        }
    }

    /// Starts dragging the specified node.
    fn start_node_drag(&mut self, node_id: NodeId, current_pos: egui::Pos2, canvas_pos: egui::Pos2) {
        self.dragging_node = Some(node_id);
        self.drag_start_pos = Some(current_pos);

        // Calculate offset from node center to mouse position for smooth dragging
        if let Some(node) = self.flowchart.nodes.get(&node_id) {
            let node_center = egui::pos2(node.position.0, node.position.1);
            self.node_drag_offset = node_center - canvas_pos;
        }
    }

    /// Updates the position of the currently dragged node.
    fn update_dragged_node_position(&mut self, node_id: NodeId, canvas_pos: egui::Pos2) {
        let new_canvas_pos = canvas_pos + self.node_drag_offset;
        if let Some(node) = self.flowchart.nodes.get_mut(&node_id) {
            node.position = (new_canvas_pos.x, new_canvas_pos.y);
        }
    }

    /// Renders all flowchart elements (connections and nodes).
    fn render_flowchart_elements(&self, painter: &egui::Painter) {
        // Draw connections first (behind nodes)
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
                let canvas_pos = pos - self.canvas_offset;
                self.selected_node = self.find_node_at_position(canvas_pos);
                self.editing_node_name = None; // Stop editing on click elsewhere
            }
        }

        // Right-click for context menu
        if response.secondary_clicked() && !self.is_panning && self.dragging_node.is_none() {
            if let Some(pos) = response.interact_pointer_pos() {
                let canvas_pos = pos - self.canvas_offset;
                self.context_menu_pos = (canvas_pos.x, canvas_pos.y);
                self.show_context_menu = true;
                self.context_menu_just_opened = true;
            }
        }
    }

    /// Renders a connection between two nodes with animated messages.
    fn draw_connection(&self, painter: &egui::Painter, connection: &Connection) {
        // Get node positions with canvas offset applied
        let start_pos = self.flowchart.nodes.get(&connection.from)
            .map(|n| egui::pos2(n.position.0, n.position.1) + self.canvas_offset)
            .unwrap_or_else(|| egui::pos2(0.0, 0.0) + self.canvas_offset);

        let end_pos = self.flowchart.nodes.get(&connection.to)
            .map(|n| egui::pos2(n.position.0, n.position.1) + self.canvas_offset)
            .unwrap_or_else(|| egui::pos2(100.0, 100.0) + self.canvas_offset);

        // Draw the connection line
        painter.line_segment(
            [start_pos, end_pos],
            egui::Stroke::new(2.0, egui::Color32::DARK_GRAY)
        );

        // Draw messages as animated dots along the connection
        for message in &connection.messages {
            let msg_pos = start_pos + (end_pos - start_pos) * message.position_along_edge;
            painter.circle_filled(msg_pos, 3.0, egui::Color32::YELLOW);
        }
    }

    /// Renders a single flowchart node with appropriate styling and text.
    fn draw_node(&self, painter: &egui::Painter, node: &FlowchartNode) {
        const NODE_SIZE: egui::Vec2 = egui::Vec2::new(100.0, 70.0);

        // Apply canvas offset for proper positioning
        let pos = egui::pos2(node.position.0, node.position.1) + self.canvas_offset;
        let rect = egui::Rect::from_center_size(pos, NODE_SIZE);

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

        painter.rect_stroke(rect, 5.0, egui::Stroke::new(stroke_width, stroke_color));

        // Render wrapped node name text
        self.draw_node_text(painter, node, pos, NODE_SIZE);
    }

    /// Renders the node's name text with proper wrapping and positioning.
    fn draw_node_text(&self, painter: &egui::Painter, node: &FlowchartNode, pos: egui::Pos2, size: egui::Vec2) {
        let text_rect = egui::Rect::from_center_size(
            egui::pos2(pos.x, pos.y - 5.0),
            egui::vec2(size.x - 10.0, size.y - 20.0) // Leave padding
        );

        let font_id = egui::FontId::default();
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
