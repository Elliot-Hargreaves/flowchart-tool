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

                        if response.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                            // Save the name change
                            if let Some(node) = self.flowchart.nodes.get_mut(&selected_id) {
                                node.name = self.temp_node_name.clone();
                            }
                            self.editing_node_name = None;
                        } else if response.lost_focus() {
                            // Cancel editing if focus lost without Enter
                            self.editing_node_name = None;
                        }

                        if response.gained_focus() {
                            // Auto-select all text when starting to edit
                            response.request_focus();
                        }
                    } else {
                        // Show name as clickable label
                        if ui.button(&node.name).clicked() {
                            self.editing_node_name = Some(selected_id);
                            self.temp_node_name = node.name.clone();
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
        let menu_pos = egui::pos2(self.context_menu_pos.0, self.context_menu_pos.1);

        egui::Area::new(egui::Id::new("context_menu"))
            .fixed_pos(menu_pos)
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
                });
            });

        // Close menu if clicked elsewhere
        if ui.input(|i| i.pointer.any_click()) {
            // Check if click was outside the menu area
            if let Some(pos) = ui.input(|i| i.pointer.interact_pos()) {
                let menu_rect = egui::Rect::from_min_size(
                    menu_pos, 
                    egui::vec2(120.0, 100.0) // Approximate menu size
                );
                if !menu_rect.contains(pos) {
                    self.show_context_menu = false;
                }
            }
        }
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

    fn draw_canvas(&mut self, ui: &mut egui::Ui) {
        let (response, painter) = ui.allocate_painter(
            ui.available_size(),
            egui::Sense::click_and_drag()
        );

        // Draw connections (arrows)
        for connection in &self.flowchart.connections {
            self.draw_connection(&painter, connection);
        }

        // Draw nodes
        for (_id, node) in &self.flowchart.nodes {
            self.draw_node(&painter, node);
        }

        // Handle interactions
        if response.clicked() {
            // Check if we clicked on a node
            if let Some(pos) = response.interact_pointer_pos() {
                self.selected_node = self.find_node_at_position(pos);
                self.editing_node_name = None; // Stop editing if we click elsewhere
            }
        }

        // Handle right-click for context menu
        if response.secondary_clicked() {
            if let Some(pos) = response.interact_pointer_pos() {
                self.context_menu_pos = (pos.x, pos.y);
                self.show_context_menu = true;
            }
        }

        // Show context menu
        if self.show_context_menu {
            self.draw_context_menu(ui);
        }
    }

    fn draw_connection(&self, painter: &egui::Painter, connection: &Connection) {
        // Find start and end positions based on node positions
        let start_pos = self.flowchart.nodes.get(&connection.from)
            .map(|n| egui::pos2(n.position.0, n.position.1))
            .unwrap_or(egui::pos2(0.0, 0.0));

        let end_pos = self.flowchart.nodes.get(&connection.to)
            .map(|n| egui::pos2(n.position.0, n.position.1))
            .unwrap_or(egui::pos2(100.0, 100.0));

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
        let pos = egui::pos2(node.position.0, node.position.1);
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

        // Draw node name
        let text_pos = egui::pos2(pos.x, pos.y - 5.0);
        painter.text(
            text_pos,
            egui::Align2::CENTER_CENTER,
            &node.name,
            egui::FontId::default(),
            egui::Color32::BLACK,
        );
    }

    fn deliver_message_to_node(&mut self, node_id: NodeId, message: Message) {
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