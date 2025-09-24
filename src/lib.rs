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
            ui.label("Properties");
            if let Some(selected_id) = self.selected_node {
                if let Some(node) = self.flowchart.nodes.get(&selected_id) {
                    ui.label(format!("Selected Node: {:?}", node.node_type));
                }
            }
        });
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
            // Handle node selection, creation, etc.
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
        let size = egui::vec2(80.0, 60.0);
        let rect = egui::Rect::from_center_size(pos, size);

        let color = match node.node_type {
            NodeType::Producer { .. } => egui::Color32::LIGHT_GREEN,
            NodeType::Consumer { .. } => egui::Color32::LIGHT_RED,
            NodeType::Transformer { .. } => egui::Color32::LIGHT_BLUE,
        };

        painter.rect_filled(rect, 5.0, color);
        painter.rect_stroke(rect, 5.0, egui::Stroke::new(2.0, egui::Color32::BLACK));
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