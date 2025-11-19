//! Canvas rendering functionality for nodes, connections, and grid.
//!
//! This module handles all drawing operations including grid background,
//! connection lines with arrows and messages, and node visualization.

use super::highlighters;
use super::state::FlowchartApp;
use crate::types::*;
use eframe::egui;
use eframe::epaint::StrokeKind;

impl FlowchartApp {
    /// Renders all flowchart elements (grid, connections, and nodes) on the canvas.
    ///
    /// Elements are drawn in layers: grid first (background), then connections,
    /// then nodes (foreground), ensuring proper visual hierarchy.
    ///
    /// # Arguments
    ///
    /// * `painter` - The egui painter for drawing operations
    /// * `canvas_rect` - The screen-space rectangle of the canvas area
    pub fn render_flowchart_elements(&self, painter: &egui::Painter, canvas_rect: egui::Rect) {
        // Draw grid first (behind everything) if enabled
        if self.canvas.show_grid {
            self.draw_grid(painter, canvas_rect);
        }

        // Draw group background rectangles behind connections and nodes
        // so they appear as faint background grouping hints, and render the group name
        for (gid, group) in &self.flowchart.groups {
            if let Some(world_rect) = self.group_world_rect(*gid) {
                // Convert to screen rect with current zoom/pan
                let min = self.world_to_screen(world_rect.min);
                let max = self.world_to_screen(world_rect.max);
                let screen_rect = egui::Rect::from_min_max(min, max);

                let is_selected = self.interaction.selected_group == Some(*gid);
                let fill = if is_selected {
                    egui::Color32::from_rgba_unmultiplied(100, 150, 255, 32)
                } else {
                    egui::Color32::from_rgba_unmultiplied(128, 128, 128, 20)
                };
                let stroke_color = if is_selected {
                    egui::Color32::from_rgb(100, 150, 255)
                } else {
                    egui::Color32::from_rgba_unmultiplied(128, 128, 128, 128)
                };
                painter.rect_filled(
                    screen_rect,
                    crate::constants::GROUP_CORNER_RADIUS,
                    fill,
                );
                painter.rect_stroke(
                    screen_rect,
                    crate::constants::GROUP_CORNER_RADIUS,
                    egui::Stroke::new(crate::constants::GROUP_STROKE_WIDTH, stroke_color),
                    StrokeKind::Inside,
                );

                // Draw the group name in the bottom-left corner of the rectangle
                // Scale font size with zoom for readability
                let mut text = group.name.as_str();
                if text.is_empty() {
                    text = "Unnamed Group";
                }
                let padding = crate::constants::GROUP_LABEL_PADDING_BASE
                    * self.canvas.zoom_factor.max(0.5);
                let pos = egui::pos2(screen_rect.min.x + padding, screen_rect.max.y - padding);
                // Choose a subtle but readable color; use stroke_color for contrast
                let text_color = if self.dark_mode {
                    egui::Color32::from_gray(220)
                } else {
                    egui::Color32::from_gray(40)
                };
                let font_size = (12.0 * self.canvas.zoom_factor).clamp(8.0, 24.0);
                let font = egui::FontId::proportional(font_size);
                painter.text(
                    pos,
                    egui::Align2::LEFT_BOTTOM,
                    text,
                    font,
                    text_color,
                );
            }
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
        for node in self.flowchart.nodes.values() {
            self.draw_node(painter, node);
        }

        // Draw marquee selection rectangle if active
        if let (Some(start), Some(end)) =
            (self.interaction.marquee_start, self.interaction.marquee_end)
        {
            let rect = egui::Rect::from_two_pos(start, end);
            let fill = egui::Color32::from_rgba_unmultiplied(100, 150, 255, 40);
            let stroke = egui::Stroke::new(1.5, egui::Color32::from_rgb(100, 150, 255));
            painter.rect_filled(rect, 0.0, fill);
            painter.rect_stroke(rect, 0.0, stroke, StrokeKind::Inside);
        }
    }

    /// Draws a zoom-aware grid on the canvas for visual reference.
    ///
    /// Grid lines are drawn every 20 world units. The grid automatically adjusts
    /// for zoom level and only draws when the grid spacing is visible.
    /// Axis lines (x=0, y=0) are drawn more prominently at higher zoom levels.
    ///
    /// # Arguments
    ///
    /// * `painter` - The egui painter for drawing operations
    /// * `canvas_rect` - The screen-space rectangle defining visible area
    pub fn draw_grid(&self, painter: &egui::Painter, canvas_rect: egui::Rect) {
        let grid_size = crate::constants::GRID_SIZE;
        let grid_color = egui::Color32::from_rgba_unmultiplied(128, 128, 128, 32);
        let stroke = egui::Stroke::new(1.0, grid_color);

        // Calculate world space bounds from screen space
        let top_left_world = self.screen_to_world(canvas_rect.min);
        let bottom_right_world = self.screen_to_world(canvas_rect.max);

        // Calculate grid range in world coordinates
        let start_x = (top_left_world.x / grid_size).floor() * grid_size;
        let end_x = (bottom_right_world.x / grid_size).ceil() * grid_size;
        let start_y = (top_left_world.y / grid_size).floor() * grid_size;
        let end_y = (bottom_right_world.y / grid_size).ceil() * grid_size;

        // Only draw grid if zoom level makes it reasonable to see
        let screen_grid_size = grid_size * self.canvas.zoom_factor;
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
                    [
                        egui::pos2(screen_x, canvas_rect.min.y),
                        egui::pos2(screen_x, canvas_rect.max.y),
                    ],
                    stroke,
                );
            }
            x += grid_size;
        }

        // Draw horizontal grid lines
        let mut y = start_y;
        while y <= end_y {
            let world_pos = egui::pos2(0.0, y);
            let screen_y = self.world_to_screen(world_pos).y;

            if screen_y >= canvas_rect.min.y && screen_y <= canvas_rect.max.y {
                painter.line_segment(
                    [
                        egui::pos2(canvas_rect.min.x, screen_y),
                        egui::pos2(canvas_rect.max.x, screen_y),
                    ],
                    stroke,
                );
            }
            y += grid_size;
        }

        // Draw axis lines more prominently when zoomed in
        if screen_grid_size > 10.0 {
            let axis_color = egui::Color32::from_rgba_unmultiplied(128, 128, 128, 80);
            let axis_stroke = egui::Stroke::new(1.5, axis_color);

            // Draw X axis (y=0)
            let x_axis_screen_y = self.world_to_screen(egui::pos2(0.0, 0.0)).y;
            if x_axis_screen_y >= canvas_rect.min.y && x_axis_screen_y <= canvas_rect.max.y {
                painter.line_segment(
                    [
                        egui::pos2(canvas_rect.min.x, x_axis_screen_y),
                        egui::pos2(canvas_rect.max.x, x_axis_screen_y),
                    ],
                    axis_stroke,
                );
            }

            // Draw Y axis (x=0)
            let y_axis_screen_x = self.world_to_screen(egui::pos2(0.0, 0.0)).x;
            if y_axis_screen_x >= canvas_rect.min.x && y_axis_screen_x <= canvas_rect.max.x {
                painter.line_segment(
                    [
                        egui::pos2(y_axis_screen_x, canvas_rect.min.y),
                        egui::pos2(y_axis_screen_x, canvas_rect.max.y),
                    ],
                    axis_stroke,
                );
            }
        }
    }

    /// Renders a connection between two nodes with animated messages and directional arrow.
    ///
    /// Connections are drawn as lines with arrows indicating direction. Messages in
    /// transit are shown as a grid of dots near the arrow.
    ///
    /// # Arguments
    ///
    /// * `painter` - The egui painter for drawing operations
    /// * `connection` - The connection to render
    /// * `is_selected` - Whether this connection is currently selected
    pub fn draw_connection(
        &self,
        painter: &egui::Painter,
        connection: &Connection,
        is_selected: bool,
    ) {
        // Get node positions with zoom and canvas offset applied
        let start_world = self
            .flowchart
            .nodes
            .get(&connection.from)
            .map(|n| egui::pos2(n.position.0, n.position.1))
            .unwrap_or_else(|| egui::pos2(0.0, 0.0));
        let start_pos = self.world_to_screen(start_world);

        let end_world = self
            .flowchart
            .nodes
            .get(&connection.to)
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
            egui::Stroke::new(line_width, line_color),
        );

        // Draw arrow at the center of the connection
        self.draw_arrow_at_center(painter, start_pos, end_pos, line_color);

        // Draw messages as a grid next to the arrow
        if !connection.messages.is_empty() {
            self.draw_message_grid(painter, start_pos, end_pos, connection.messages.len());
        }
    }

    /// Draws a directional arrow at the center of a connection line.
    ///
    /// The arrow is rendered as a filled triangle pointing from source to destination.
    /// Arrow size scales with the current zoom level.
    ///
    /// # Arguments
    ///
    /// * `painter` - The egui painter for drawing operations
    /// * `start` - Start position of the connection in screen space
    /// * `end` - End position of the connection in screen space
    /// * `color` - Color for the arrow
    fn draw_arrow_at_center(
        &self,
        painter: &egui::Painter,
        start: egui::Pos2,
        end: egui::Pos2,
        color: egui::Color32,
    ) {
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

    /// Draws a grid of dots representing messages in transit next to the connection arrow.
    ///
    /// Messages are displayed in a 5-column grid with unlimited rows. Each message
    /// is represented by a yellow dot with a gray outline. The grid is positioned
    /// perpendicular to the connection line for visibility.
    ///
    /// # Arguments
    ///
    /// * `painter` - The egui painter for drawing operations
    /// * `start` - Start position of the connection in screen space
    /// * `end` - End position of the connection in screen space
    /// * `message_count` - Number of messages to visualize
    fn draw_message_grid(
        &self,
        painter: &egui::Painter,
        start: egui::Pos2,
        end: egui::Pos2,
        message_count: usize,
    ) {
        let grid_width = crate::constants::GRID_WIDTH;
        let dot_spacing = crate::constants::DOT_SPACING;
        let dot_radius = crate::constants::DOT_RADIUS;

        // Calculate center point of the connection
        let center = start + (end - start) * 0.5;

        // Calculate direction and perpendicular vectors
        let direction = (end - start).normalized();
        let perpendicular = egui::vec2(-direction.y, direction.x);

        // Offset the grid to the side of the arrow
        let grid_offset = perpendicular * 15.0 * self.canvas.zoom_factor;

        // Calculate grid dimensions
        let _rows = message_count.div_ceil(grid_width);
        let _cols = usize::min(grid_width, message_count);

        // Calculate starting position (offset from center)
        let grid_width_pixels = -(grid_width as f32) * dot_spacing * self.canvas.zoom_factor;
        let grid_height_pixels = (grid_width - 1) as f32 * dot_spacing * self.canvas.zoom_factor;

        let grid_start = center + grid_offset
            - perpendicular * grid_width_pixels * 0.5
            - direction * grid_height_pixels * 0.5;

        // Draw each dot in the grid
        for i in 0..message_count {
            let row = i / grid_width;
            let col = i % grid_width;

            let dot_pos = grid_start
                + perpendicular * (row as f32 * dot_spacing * self.canvas.zoom_factor)
                + direction * (col as f32 * dot_spacing * self.canvas.zoom_factor);

            let scaled_radius = dot_radius * self.canvas.zoom_factor;
            painter.circle_filled(dot_pos, scaled_radius, egui::Color32::YELLOW);
            painter.circle_stroke(
                dot_pos,
                scaled_radius,
                egui::Stroke::new(1.0, egui::Color32::DARK_GRAY),
            );
        }
    }

    /// Renders a preview of the connection being drawn during shift-click drag.
    ///
    /// Shows a line from the source node to the current mouse position. The line
    /// is colored blue if the target is valid, red if invalid (e.g., self-connection,
    /// Consumer as source, Producer as target).
    ///
    /// # Arguments
    ///
    /// * `painter` - The egui painter for drawing operations
    /// * `from_node_id` - ID of the source node
    /// * `to_screen_pos` - Current mouse position in screen space
    pub fn draw_connection_preview(
        &self,
        painter: &egui::Painter,
        from_node_id: NodeId,
        to_screen_pos: egui::Pos2,
    ) {
        if let Some(from_node) = self.flowchart.nodes.get(&from_node_id) {
            let from_world = egui::pos2(from_node.position.0, from_node.position.1);
            let from_screen = self.world_to_screen(from_world);

            // Check if hovering over a valid target node
            let to_world_pos = self.screen_to_world(to_screen_pos);
            let is_valid = if let Some(to_node_id) = self.find_node_at_position(to_world_pos) {
                if to_node_id == from_node_id {
                    // Self-connection is invalid
                    false
                } else if let Some(to_node) = self.flowchart.nodes.get(&to_node_id) {
                    // Check if connection is allowed based on node types
                    match (&from_node.node_type, &to_node.node_type) {
                        // Consumer cannot send (cannot be source)
                        (NodeType::Consumer { .. }, _) => false,
                        // Producer cannot receive (cannot be target)
                        (_, NodeType::Producer { .. }) => false,
                        // All other combinations are valid
                        _ => true,
                    }
                } else {
                    true // Unknown node, assume valid
                }
            } else {
                true // No target node, show as potentially valid
            };

            // Choose color based on validity
            let color = if is_valid {
                egui::Color32::from_rgb(100, 150, 255) // Blue for valid
            } else {
                egui::Color32::from_rgb(255, 80, 80) // Red for invalid
            };

            // Draw line for preview
            let stroke = egui::Stroke::new(2.0, color);
            painter.line_segment([from_screen, to_screen_pos], stroke);

            // Draw small circle at the end to indicate connection point
            painter.circle_filled(to_screen_pos, 4.0, color);
        }
    }

    /// Renders a single flowchart node with appropriate styling and text.
    ///
    /// Nodes are color-coded by type (green=Producer, red=Consumer, blue=Transformer).
    /// Selected nodes have a yellow border, dragged nodes have an orange border,
    /// and error nodes have a flashing red border.
    ///
    /// # Arguments
    ///
    /// * `painter` - The egui painter for drawing operations
    /// * `node` - The node to render
    pub fn draw_node(&self, painter: &egui::Painter, node: &FlowchartNode) {
        let node_size = egui::vec2(crate::constants::NODE_WIDTH, crate::constants::NODE_HEIGHT);

        // Apply zoom and canvas offset for proper positioning
        let world_pos = egui::pos2(node.position.0, node.position.1);
        let screen_pos = self.world_to_screen(world_pos);
        let scaled_size = node_size * self.canvas.zoom_factor;
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
        let (stroke_color, stroke_width) = if Some(node.id) == self.error_node {
            // Flashing red border for error nodes (flash every 15 frames)
            let flash_on = (self.frame_counter / 15).is_multiple_of(2);
            if flash_on {
                (egui::Color32::from_rgb(255, 0, 0), 5.0) // Bright red for error
            } else {
                (egui::Color32::from_rgb(180, 0, 0), 5.0) // Dark red for error
            }
        } else if Some(node.id) == self.interaction.dragging_node {
            (egui::Color32::from_rgb(255, 165, 0), 4.0) // Orange for dragging
        } else if Some(node.id) == self.interaction.selected_node
            || self.interaction.selected_nodes.contains(&node.id)
        {
            (egui::Color32::YELLOW, 3.0) // Yellow for selected
        } else {
            (egui::Color32::BLACK, 2.0) // Black for normal
        };

        painter.rect_stroke(
            rect,
            5.0,
            egui::Stroke::new(stroke_width, stroke_color),
            StrokeKind::Outside,
        );

        // Render wrapped node name text
        self.draw_node_text(painter, node, screen_pos, scaled_size);
    }

    /// Renders the node's name text with proper wrapping and positioning.
    ///
    /// Text is automatically wrapped to fit within the node bounds and vertically
    /// centered. Font size scales with zoom level for readability.
    ///
    /// # Arguments
    ///
    /// * `painter` - The egui painter for drawing operations
    /// * `node` - The node whose name to render
    /// * `pos` - Center position of the node in screen space
    /// * `size` - Size of the node in screen space
    fn draw_node_text(
        &self,
        painter: &egui::Painter,
        node: &FlowchartNode,
        pos: egui::Pos2,
        size: egui::Vec2,
    ) {
        let text_rect = egui::Rect::from_center_size(
            egui::pos2(pos.x, pos.y - 5.0 * self.canvas.zoom_factor),
            egui::vec2(
                size.x - 10.0 * self.canvas.zoom_factor,
                size.y - 20.0 * self.canvas.zoom_factor,
            ),
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
            let line_pos = egui::pos2(text_rect.center().x, start_y + i as f32 * line_height);
            painter.text(
                line_pos,
                egui::Align2::CENTER_CENTER,
                line,
                font_id.clone(),
                egui::Color32::BLACK,
            );
        }
    }

    /// Wraps text to fit within the specified width, returning a vector of lines.
    ///
    /// Breaks text at word boundaries to fit within the maximum width. If a single
    /// word is too long, it's placed on its own line anyway.
    ///
    /// # Arguments
    ///
    /// * `text` - The text to wrap
    /// * `max_width` - Maximum width in pixels
    /// * `font_id` - Font to use for measuring text width
    /// * `painter` - The egui painter for measuring text
    ///
    /// # Returns
    ///
    /// A vector of lines that fit within the maximum width
    pub fn wrap_text(
        &self,
        text: &str,
        max_width: f32,
        font_id: &egui::FontId,
        painter: &egui::Painter,
    ) -> Vec<String> {
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
                f.layout_no_wrap(test_line.clone(), font_id.clone(), egui::Color32::BLACK)
                    .size()
                    .x
            });

            if text_width <= max_width {
                current_line = test_line;
            } else if !current_line.is_empty() {
                lines.push(current_line);
                current_line = word.to_string();
            } else {
                // Single word too long, add it anyway
                lines.push(word.to_string());
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
}

/// Helper function to create a JavaScript syntax highlighting layouter for text editors.
///
/// # Arguments
///
/// * `temp_script` - Reference to the script string being edited
///
/// # Returns
///
/// A closure that can be used as a layouter for egui::TextEdit
pub fn create_js_layouter(
    temp_script: &str,
) -> impl FnMut(&egui::Ui, &dyn egui::TextBuffer, f32) -> std::sync::Arc<egui::Galley> + '_ {
    move |ui: &egui::Ui, _text: &dyn egui::TextBuffer, wrap_width: f32| {
        let font_id = egui::TextStyle::Monospace.resolve(ui.style());
        let mut layout_job =
            highlighters::highlight_javascript(temp_script, font_id, ui.visuals().dark_mode);
        layout_job.wrap.max_width = wrap_width;
        ui.fonts(|f| f.layout_job(layout_job))
    }
}

/// Helper function to create a JSON syntax highlighting layouter for text editors.
///
/// # Arguments
///
/// * `temp_json` - Reference to the JSON string being edited
///
/// # Returns
///
/// A closure that can be used as a layouter for egui::TextEdit
pub fn create_json_layouter(
    temp_json: &str,
) -> impl FnMut(&egui::Ui, &dyn egui::TextBuffer, f32) -> std::sync::Arc<egui::Galley> + '_ {
    move |ui: &egui::Ui, _text: &dyn egui::TextBuffer, wrap_width: f32| {
        let font_id = egui::TextStyle::Monospace.resolve(ui.style());
        let mut layout_job =
            highlighters::highlight_json(temp_json, font_id, ui.visuals().dark_mode);
        layout_job.wrap.max_width = wrap_width;
        ui.fonts(|f| f.layout_job(layout_job))
    }
}
