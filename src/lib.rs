use eframe::egui;
use std::collections::HashMap;
use serde::{Deserialize, Serialize};
use mlua::Lua;
use uuid::Uuid;

// Type definitions
pub type NodeId = Uuid;
pub type MessageId = Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum NodeState {
    Idle,
    Processing,
    Error(String),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum SimulationState {
    Stopped,
    Running,
    Paused,
}

// Core data structures
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FlowchartNode {
    pub id: NodeId,
    pub name: String,
    pub position: (f32, f32),
    pub node_type: NodeType,
    pub state: NodeState,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum NodeType {
    Producer { generation_rate: u32 },
    Consumer { consumption_rate: u32 },
    Transformer { script: String }, // Lua script
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Connection {
    pub from: NodeId,
    pub to: NodeId,
    pub messages: Vec<Message>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Message {
    pub id: MessageId,
    pub data: serde_json::Value,
    pub position_along_edge: f32, // 0.0 to 1.0 for animation
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Flowchart {
    pub nodes: HashMap<NodeId, FlowchartNode>,
    pub connections: Vec<Connection>,
    pub simulation_state: SimulationState,
}

impl Default for Flowchart {
    fn default() -> Self {
        Self {
            nodes: HashMap::new(),
            connections: Vec::new(),
            simulation_state: SimulationState::Stopped,
        }
    }
}

#[derive(Default)]
struct FlowchartApp {
    flowchart: Flowchart,
    selected_node: Option<NodeId>,
    is_simulation_running: bool,
    simulation_speed: f32,
    show_context_menu: bool,
    context_menu_pos: (f32, f32),
    node_counter: u32,
    editing_node_name: Option<NodeId>,
    temp_node_name: String,
    canvas_offset: egui::Vec2,
    is_panning: bool,
    last_pan_pos: Option<egui::Pos2>,
    context_menu_just_opened: bool,
    should_select_text: bool,
}

impl eframe::App for FlowchartApp {
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

        if self.is_simulation_running {
            self.step_simulation();
            ctx.request_repaint(); // Keep animating
        }
    }
}

impl FlowchartApp {
    fn draw_toolbar(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            if ui.button("Start").clicked() {
                self.is_simulation_running = true;
            }
            if ui.button("Stop").clicked() {
                self.is_simulation_running = false;
            }
            if ui.button("Step").clicked() {
                self.step_simulation();
            }
        });
    }

    fn draw_properties_panel(&mut self, ui: &mut egui::Ui) {
        ui.vertical(|ui| {
            ui.heading("Properties");
            ui.separator();

            if let Some(selected_id) = self.selected_node {
                if let Some(node) = self.flowchart.nodes.get(&selected_id).cloned() {
                    // Node name editing
                    ui.label("Name:");

                    if self.editing_node_name == Some(selected_id) {
                        // Show text edit field
                        let response = ui.text_edit_singleline(&mut self.temp_node_name);

                        // Request focus and handle text selection
                        response.request_focus();

                        if self.should_select_text && response.has_focus() {
                            // Select all text by setting cursor to end and selection to start
                            self.should_select_text = false;
                            // In many cases, the text is automatically selected when focused
                        }

                        // Check for Enter key press while the field has focus
                        if response.has_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                            // Save the name change
                            if let Some(node) = self.flowchart.nodes.get_mut(&selected_id) {
                                node.name = self.temp_node_name.clone();
                            }
                            self.editing_node_name = None;
                        } else if response.lost_focus() {
                            // Cancel editing if focus lost without Enter
                            self.editing_node_name = None;
                        }
                    } else {
                        // Show name as clickable label
                        if ui.button(&node.name).clicked() {
                            self.editing_node_name = Some(selected_id);
                            self.temp_node_name = node.name.clone();
                            self.should_select_text = true;
                        }
                    }

                    ui.separator();

                    // Node type
                    ui.label(format!("Type: {}", match &node.node_type {
                        NodeType::Producer { .. } => "Producer",
                        NodeType::Consumer { .. } => "Consumer",
                        NodeType::Transformer { .. } => "Transformer",
                    }));

                    // Node type specific properties
                    match &node.node_type {
                        NodeType::Producer { generation_rate } => {
                            ui.label(format!("Generation Rate: {}", generation_rate));
                        }
                        NodeType::Consumer { consumption_rate } => {
                            ui.label(format!("Consumption Rate: {}", consumption_rate));
                        }
                        NodeType::Transformer { script } => {
                            ui.label("Script:");
                            ui.text_edit_multiline(&mut script.clone());
                        }
                    }

                    ui.separator();
                    ui.label(format!("State: {:?}", node.state));
                    ui.label(format!("Position: ({:.1}, {:.1})", node.position.0, node.position.1));
                } else {
                    ui.label("Node not found");
                }
            } else {
                ui.label("No node selected");
                ui.separator();
                ui.label("Left-click on a node to select it");
            }
        });
    }

    fn draw_context_menu(&mut self, ui: &mut egui::Ui) {
        // Convert canvas coordinates to screen coordinates
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
                                script: "-- Lua transformation script\nreturn input".to_string() 
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

        // Only check for click-outside after the menu has been open for at least one frame
        if !self.context_menu_just_opened {
            // Close menu if clicked elsewhere (but not on secondary click to avoid conflicts)
            if ui.input(|i| i.pointer.primary_clicked()) {
                // Check if click was outside the menu area using the actual menu response
                if let Some(click_pos) = ui.input(|i| i.pointer.interact_pos()) {
                    if !area_response.response.rect.contains(click_pos) {
                        self.show_context_menu = false;
                    }
                }
            }
        }

        // Reset the just_opened flag after the first frame
        self.context_menu_just_opened = false;
    }

    fn create_node_at_pos(&mut self, node_type: NodeType) {
        self.node_counter += 1;
        let node_id = uuid::Uuid::new_v4();

        let new_node = FlowchartNode {
            id: node_id,
            name: format!("node{}", self.node_counter),
            position: self.context_menu_pos,
            node_type,
            state: NodeState::Idle,
        };

        self.flowchart.nodes.insert(new_node.id, new_node);

        // Select the new node and start editing its name immediately
        self.selected_node = Some(node_id);
        self.editing_node_name = Some(node_id);
        self.temp_node_name = format!("node{}", self.node_counter);
        self.should_select_text = true;
    }

    fn find_node_at_position(&self, pos: egui::Pos2) -> Option<NodeId> {
        for (id, node) in &self.flowchart.nodes {
            let node_pos = egui::pos2(node.position.0, node.position.1);
            let size = egui::vec2(100.0, 70.0);
            let rect = egui::Rect::from_center_size(node_pos, size);

            if rect.contains(pos) {
                return Some(*id);
            }
        }
        None
    }

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

            let text_width = painter.fonts(|f| f.layout_no_wrap(test_line.clone(), font_id.clone(), egui::Color32::BLACK).size().x);

            if text_width <= max_width {
                current_line = test_line;
            } else {
                if !current_line.is_empty() {
                    lines.push(current_line);
                    current_line = word.to_string();
                } else {
                    // Single word is too long, just add it anyway
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

    fn draw_canvas(&mut self, ui: &mut egui::Ui) {
        let (response, painter) = ui.allocate_painter(
            ui.available_size(),
            egui::Sense::click_and_drag()
        );

        // Handle middle-click panning
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

        // Draw connections (arrows)
        for connection in &self.flowchart.connections {
            self.draw_connection(&painter, connection);
        }

        // Draw nodes
        for (_id, node) in &self.flowchart.nodes {
            self.draw_node(&painter, node);
        }

        // Handle interactions
        if response.clicked() && !self.is_panning {
            // Check if we clicked on a node
            if let Some(pos) = response.interact_pointer_pos() {
                let canvas_pos = pos - self.canvas_offset;
                self.selected_node = self.find_node_at_position(canvas_pos);
                self.editing_node_name = None; // Stop editing if we click elsewhere
            }
        }

        // Handle right-click for context menu
        if response.secondary_clicked() && !self.is_panning {
            if let Some(pos) = response.interact_pointer_pos() {
                let canvas_pos = pos - self.canvas_offset;
                self.context_menu_pos = (canvas_pos.x, canvas_pos.y);
                self.show_context_menu = true;
                self.context_menu_just_opened = true;
            }
        }

        // Show context menu
        if self.show_context_menu {
            self.draw_context_menu(ui);
        }
    }

    fn draw_connection(&self, painter: &egui::Painter, connection: &Connection) {
        // Find start and end positions based on node positions with canvas offset
        let start_pos = self.flowchart.nodes.get(&connection.from)
            .map(|n| egui::pos2(n.position.0, n.position.1) + self.canvas_offset)
            .unwrap_or(egui::pos2(0.0, 0.0) + self.canvas_offset);

        let end_pos = self.flowchart.nodes.get(&connection.to)
            .map(|n| egui::pos2(n.position.0, n.position.1) + self.canvas_offset)
            .unwrap_or(egui::pos2(100.0, 100.0) + self.canvas_offset);

        painter.line_segment(
            [start_pos, end_pos],
            egui::Stroke::new(2.0, egui::Color32::DARK_GRAY)
        );

        // Draw messages on the connection
        for message in &connection.messages {
            let msg_pos = start_pos + (end_pos - start_pos) * message.position_along_edge;
            painter.circle_filled(msg_pos, 3.0, egui::Color32::YELLOW);
        }
    }

    fn draw_node(&self, painter: &egui::Painter, node: &FlowchartNode) {
        // Apply canvas offset
        let pos = egui::pos2(node.position.0, node.position.1) + self.canvas_offset;
        let size = egui::vec2(100.0, 70.0);
        let rect = egui::Rect::from_center_size(pos, size);

        let color = match node.node_type {
            NodeType::Producer { .. } => egui::Color32::LIGHT_GREEN,
            NodeType::Consumer { .. } => egui::Color32::LIGHT_RED,
            NodeType::Transformer { .. } => egui::Color32::LIGHT_BLUE,
        };

        painter.rect_filled(rect, 5.0, color);

        // Draw selection highlight
        let stroke_color = if Some(node.id) == self.selected_node {
            egui::Color32::YELLOW
        } else {
            egui::Color32::BLACK
        };
        let stroke_width = if Some(node.id) == self.selected_node { 3.0 } else { 2.0 };

        painter.rect_stroke(rect, 5.0, egui::Stroke::new(stroke_width, stroke_color));

        // Draw node name with text wrapping
        let text_rect = egui::Rect::from_center_size(
            egui::pos2(pos.x, pos.y - 5.0),
            egui::vec2(size.x - 10.0, size.y - 20.0) // Leave some padding
        );

        // Split text into lines that fit within the node width
        let font_id = egui::FontId::default();
        let max_width = text_rect.width();
        let wrapped_text = self.wrap_text(&node.name, max_width, &font_id, painter);

        // Draw each line of wrapped text
        let line_height = painter.fonts(|f| f.row_height(&font_id));
        let total_height = line_height * wrapped_text.len() as f32;
        let start_y = text_rect.center().y - total_height / 2.0;

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

    fn deliver_message_to_node(&mut self, node_id: NodeId, _message: Message) {
        // Handle message delivery to the target node
        if let Some(node) = self.flowchart.nodes.get_mut(&node_id) {
            match &node.node_type {
                NodeType::Consumer { .. } => {
                    // Consume the message
                    node.state = NodeState::Processing;
                }
                NodeType::Transformer { .. } => {
                    // Transform the message (placeholder)
                    node.state = NodeState::Processing;
                }
                _ => {}
            }
        }
    }

    fn step_simulation(&mut self) {
        // Collect messages that need to be delivered
        let mut messages_to_deliver = Vec::new();

        // Move messages along connections
        for connection in &mut self.flowchart.connections {
            for message in &mut connection.messages {
                message.position_along_edge += 0.01; // Adjust speed
                if message.position_along_edge >= 1.0 {
                    // Message reached destination - collect it for delivery
                    messages_to_deliver.push((connection.to, message.clone()));
                }
            }
            // Remove delivered messages
            connection.messages.retain(|m| m.position_along_edge < 1.0);
        }

        // Now deliver all collected messages
        for (node_id, message) in messages_to_deliver {
            self.deliver_message_to_node(node_id, message);
        }

        // Process nodes
        for (_id, node) in &mut self.flowchart.nodes {
            match &node.node_type {
                NodeType::Producer { generation_rate: _ } => {
                    // Generate new messages based on rate
                    node.state = NodeState::Processing;
                }
                NodeType::Consumer { consumption_rate: _ } => {
                    // Consume incoming messages
                    node.state = NodeState::Idle;
                }
                NodeType::Transformer { script: _ } => {
                    // Execute Lua script to transform messages
                    node.state = NodeState::Idle;
                }
            }
        }
    }
}

fn execute_transformer_script(script: &str, input_message: &Message) -> Result<Vec<Message>, mlua::Error> {
    let lua = Lua::new();

    // Serialize the message to Lua
    let message_json = serde_json::to_string(input_message).map_err(mlua::Error::external)?;
    lua.globals().set("input_json", message_json)?;

    // Create a helper function in Lua to parse JSON
    lua.load(r#"
        function parse_input()
            -- This is a placeholder - in real implementation you'd parse JSON
            return {id = "test", data = {}, position_along_edge = 0.0}
        end
        input = parse_input()
    "#).exec()?;

    // Execute the user script
    lua.load(script).exec()?;

    // Get the result (placeholder implementation)
    let _result: mlua::Value = lua.globals().get("output")?;

    // For now, return the original message as a placeholder
    Ok(vec![input_message.clone()])
}

// Main function to run the app
pub fn run_app() -> Result<(), eframe::Error> {
    let options = eframe::NativeOptions::default();
    eframe::run_native(
        "Flowchart Tool",
        options,
        Box::new(|_cc| Ok(Box::new(FlowchartApp::default()))),
    )
}