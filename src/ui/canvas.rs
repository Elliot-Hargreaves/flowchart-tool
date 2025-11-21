//! Canvas interaction and navigation functionality.
//!
//! This module handles canvas panning, zooming, node dragging, connection drawing,
//! and coordinate transformations between screen and world space.

use super::state::FlowchartApp;
use crate::types::*;
use crate::ui::UndoAction;
use eframe::egui;

impl FlowchartApp {
    /// Converts screen coordinates to world coordinates accounting for zoom and pan.
    ///
    /// # Arguments
    ///
    /// * `screen_pos` - Position in screen space (pixels)
    ///
    /// # Returns
    ///
    /// The corresponding position in world space
    pub fn screen_to_world(&self, screen_pos: egui::Pos2) -> egui::Pos2 {
        (screen_pos - self.canvas.offset) / self.canvas.zoom_factor
    }

    /// Converts world coordinates to screen coordinates accounting for zoom and pan.
    ///
    /// # Arguments
    ///
    /// * `world_pos` - Position in world space
    ///
    /// # Returns
    ///
    /// The corresponding position in screen space (pixels)
    pub fn world_to_screen(&self, world_pos: egui::Pos2) -> egui::Pos2 {
        world_pos * self.canvas.zoom_factor + self.canvas.offset
    }

    /// Snaps a position to the nearest grid point.
    ///
    /// Grid spacing is 20 world units. Useful for aligning nodes when shift-dragging.
    ///
    /// # Arguments
    ///
    /// * `pos` - Position to snap
    ///
    /// # Returns
    ///
    /// The snapped position on the grid
    pub fn snap_to_grid(&self, pos: egui::Pos2) -> egui::Pos2 {
        let grid = crate::constants::GRID_SIZE;
        egui::pos2(
            (pos.x / grid).round() * grid,
            (pos.y / grid).round() * grid,
        )
    }

    /// Handles middle-click or Cmd/Ctrl+left-click canvas panning functionality.
    ///
    /// Uses Cmd on macOS and Ctrl on other platforms for modifier-based panning.
    ///
    /// # Arguments
    ///
    /// * `ui` - The egui UI context
    /// * `response` - The response from the canvas widget
    pub fn handle_canvas_panning(&mut self, ui: &mut egui::Ui, response: &egui::Response) {
        // Check for middle mouse button OR Cmd/Ctrl+left mouse button
        // modifiers.command automatically uses Cmd on macOS and Ctrl elsewhere
        let should_pan = ui.input(|i| {
            i.pointer.middle_down() || (i.pointer.primary_down() && i.modifiers.command)
        });

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
    ///
    /// Zooms in/out while keeping the mouse cursor position fixed in world space.
    /// Zoom range is clamped between 0.25x and 5.0x.
    /// Only zooms if the cursor is over the canvas.
    ///
    /// # Arguments
    ///
    /// * `ui` - The egui UI context
    /// * `response` - The response from the canvas widget
    pub fn handle_canvas_zoom(&mut self, ui: &mut egui::Ui, response: &egui::Response) {
        let scroll_delta = ui.input(|i| i.smooth_scroll_delta.y);

        if scroll_delta != 0.0 {
            // Use hover position if available, otherwise use response position
            let mouse_pos = ui
                .input(|i| i.pointer.hover_pos())
                .or_else(|| response.interact_pointer_pos());

            if let Some(mouse_pos) = mouse_pos {
                // Only zoom if the cursor is over the canvas
                if !response.rect.contains(mouse_pos) {
                    return;
                }

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
    ///
    /// Supports both normal dragging and shift+drag for grid-snapped movement.
    /// Also handles shift+drag from a node to create connections.
    ///
    /// # Arguments
    ///
    /// * `ui` - The egui UI context
    /// * `response` - The response from the canvas widget
    pub fn handle_node_dragging(&mut self, ui: &mut egui::Ui, response: &egui::Response) {
        // If a marquee selection is active, it takes priority over starting any node drag or connection
        if self.interaction.marquee_start.is_some() {
            return;
        }
        if ui.input(|i| i.pointer.primary_down()) && !self.interaction.is_panning {
            if let Some(current_pos) = response.interact_pointer_pos() {
                let world_pos = self.screen_to_world(current_pos);
                let shift_held = ui.input(|i| i.modifiers.shift);

                // Check if we're starting a new interaction
                if self.interaction.dragging_node.is_none()
                    && self.interaction.drawing_connection_from.is_none()
                    && self.interaction.pending_shift_connection_from.is_none()
                {
                    // Check if clicking on a node
                    if let Some(node_id) = self.find_node_at_position(world_pos) {
                        if shift_held {
                            // Shift-press on node: defer connection start until drag threshold is exceeded.
                            // If no drag happens and mouse is released, treat as additive selection.
                            if self.interaction.pending_shift_connection_from.is_none() {
                                self.interaction.pending_shift_connection_from = Some(node_id);
                                self.interaction.pending_shift_start_screen_pos = Some(current_pos);
                            }
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
                } else if let (Some(from_id), Some(start_pos)) = (
                    self.interaction.pending_shift_connection_from,
                    self.interaction.pending_shift_start_screen_pos,
                ) {
                    // Evaluate whether we've dragged far enough to start a connection preview
                    let start_world = self.screen_to_world(start_pos);
                    let cur_world = self.screen_to_world(current_pos);
                    let dist_world = (cur_world - start_world).length();
                    if dist_world >= crate::constants::CLICK_THRESHOLD {
                        // Begin connection drawing from the originally pressed node
                        self.interaction.drawing_connection_from = Some(from_id);
                        self.interaction.connection_draw_pos = Some(current_pos);
                        // Clear pending state now that we've committed to connection drawing
                        self.interaction.pending_shift_connection_from = None;
                        self.interaction.pending_shift_start_screen_pos = None;
                    }
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

            // If there was a pending shift-click on a node and we never started drawing a connection,
            // interpret this as a toggle selection of that node.
            if let Some(node_id) = self.interaction.pending_shift_connection_from.take() {
                if let Some(pos) = self
                    .interaction
                    .selected_nodes
                    .iter()
                    .position(|id| *id == node_id)
                {
                    self.interaction.selected_nodes.remove(pos);
                } else {
                    self.interaction.selected_nodes.push(node_id);
                }
                // Clear other selection kinds and sync single-selection helper
                match self.interaction.selected_nodes.as_slice() {
                    [only] => self.interaction.selected_node = Some(*only),
                    _ => self.interaction.selected_node = None,
                }
                self.interaction.selected_group = None;
                self.interaction.selected_connection = None;
                self.interaction.editing_node_name = None;
                self.clear_temp_editing_values();
            }
            // Always clear pending start position on release
            self.interaction.pending_shift_start_screen_pos = None;

            // Record undo for node movement when drag ends
            if let Some(dragging_id) = self.interaction.dragging_node {
                if self.interaction.selected_nodes.len() > 1 {
                    // Multi-drag: record MultipleNodesMoved
                    let old_positions = self.interaction.drag_original_positions_multi.clone();
                    let mut new_positions: Vec<(NodeId, (f32, f32))> = Vec::new();
                    for (id, _) in &old_positions {
                        if let Some(n) = self.flowchart.nodes.get(id) {
                            new_positions.push((*id, n.position));
                        }
                    }
                    if !old_positions.is_empty() && old_positions != new_positions {
                        self.undo_history
                            .push_action(UndoAction::MultipleNodesMoved {
                                old_positions,
                                new_positions,
                            });
                        self.file.has_unsaved_changes = true;
                    }
                } else if let Some(old_pos) = self.interaction.drag_original_position {
                    self.record_node_movement(dragging_id, old_pos);
                }
            }

            // Stop all dragging/drawing operations when mouse released
            self.interaction.dragging_node = None;
            self.interaction.drag_start_pos = None;
            self.interaction.drag_original_position = None;
            self.interaction.drag_original_positions_multi.clear();
            self.interaction.drawing_connection_from = None;
            self.interaction.connection_draw_pos = None;
        }
    }

    /// Starts dragging the specified node.
    ///
    /// Records the initial drag position and calculates the offset from the mouse
    /// to the node center for smooth dragging.
    ///
    /// # Arguments
    ///
    /// * `node_id` - ID of the node to start dragging
    /// * `current_pos` - Current mouse position in screen space
    /// * `world_pos` - Current mouse position in world space
    fn start_node_drag(&mut self, node_id: NodeId, current_pos: egui::Pos2, world_pos: egui::Pos2) {
        self.interaction.dragging_node = Some(node_id);
        self.interaction.drag_start_pos = Some(current_pos);

        // Ensure selection includes the dragged node; if no multi-selection, select only this node
        if !self.interaction.selected_nodes.contains(&node_id) {
            self.interaction.selected_nodes.clear();
            self.interaction.selected_nodes.push(node_id);
            self.interaction.selected_node = Some(node_id);
            self.interaction.selected_connection = None;
            // Selection changed as part of starting a drag; clear temp editors so UI repopulates for the new node
            self.interaction.editing_node_name = None;
            self.clear_temp_editing_values();
        }

        // Prepare original positions for multi-drag undo
        self.interaction.drag_original_positions_multi = self
            .interaction
            .selected_nodes
            .iter()
            .filter_map(|id| self.flowchart.nodes.get(id).map(|n| (*id, n.position)))
            .collect();

        // Calculate offset from node center to mouse position for smooth dragging
        if let Some(node) = self.flowchart.nodes.get(&node_id) {
            let node_center = egui::pos2(node.position.0, node.position.1);
            self.interaction.node_drag_offset = node_center - world_pos;
            // Store original position for undo (single)
            self.interaction.drag_original_position = Some(node.position);
        }
    }

    /// Updates the position of the currently dragged node.
    ///
    /// Supports grid snapping when Shift is held during dragging.
    ///
    /// # Arguments
    ///
    /// * `node_id` - ID of the node being dragged
    /// * `world_pos` - Current mouse position in world space
    /// * `ui` - The egui UI context for checking modifiers
    fn update_dragged_node_position(
        &mut self,
        node_id: NodeId,
        world_pos: egui::Pos2,
        ui: &egui::Ui,
    ) {
        let mut new_world_pos = world_pos + self.interaction.node_drag_offset;

        // Check if Shift is held for grid snapping
        if ui.input(|i| i.modifiers.shift) {
            new_world_pos = self.snap_to_grid(new_world_pos);
        }

        // Compute delta to apply to all selected nodes if multi-drag
        if let Some(dragged_node) = self.flowchart.nodes.get(&node_id).cloned() {
            let delta = egui::vec2(
                new_world_pos.x - dragged_node.position.0,
                new_world_pos.y - dragged_node.position.1,
            );
            if self.interaction.selected_nodes.len() > 1 {
                for id in self.interaction.selected_nodes.clone() {
                    if let Some(n) = self.flowchart.nodes.get_mut(&id) {
                        n.position.0 += delta.x;
                        n.position.1 += delta.y;
                    }
                }
                return;
            }
        }

        if let Some(node) = self.flowchart.nodes.get_mut(&node_id) {
            node.position = (new_world_pos.x, new_world_pos.y);
        }
    }

    /// Records undo action for node movement when drag ends.
    ///
    /// # Arguments
    ///
    /// * `node_id` - ID of the node that was dragged
    /// * `old_position` - Position before drag started
    fn record_node_movement(&mut self, node_id: NodeId, old_position: (f32, f32)) {
        if let Some(node) = self.flowchart.nodes.get(&node_id) {
            let new_position = node.position;
            // Only record if position actually changed
            if old_position != new_position {
                self.undo_history.push_action(UndoAction::NodeMoved {
                    node_id,
                    old_position,
                    new_position,
                });
                self.file.has_unsaved_changes = true;
            }
        }
    }

    /// Finalizes connection creation when mouse is released.
    ///
    /// Validates the connection based on node types and creates it if valid.
    /// Prevents self-connections and enforces rules like "Consumer cannot send".
    ///
    /// # Arguments
    ///
    /// * `world_pos` - Final mouse position in world space
    fn finalize_connection(&mut self, world_pos: egui::Pos2) {
        if let Some(from_node_id) = self.interaction.drawing_connection_from {
            if let Some(to_node_id) = self.find_node_at_position(world_pos) {
                // Don't create self-connections
                if from_node_id != to_node_id {
                    // Validate node types for connection rules
                    let from_node = self.flowchart.nodes.get(&from_node_id);
                    let to_node = self.flowchart.nodes.get(&to_node_id);

                    if let (Some(from), Some(to)) = (from_node, to_node) {
                        // Check if connection is allowed based on node types
                        let is_valid_connection = match (&from.node_type, &to.node_type) {
                            // Consumer cannot send (cannot be source)
                            (NodeType::Consumer { .. }, _) => false,
                            // Producer cannot receive (cannot be target)
                            (_, NodeType::Producer { .. }) => false,
                            // All other combinations are valid
                            _ => true,
                        };

                        if !is_valid_connection {
                            return;
                        }

                        // Check if connection already exists
                        let connection_exists = self
                            .flowchart
                            .connections
                            .iter()
                            .any(|c| c.from == from_node_id && c.to == to_node_id);

                        if !connection_exists {
                            // Create new connection
                            let connection = Connection::new(from_node_id, to_node_id);
                            self.flowchart.connections.push(connection);

                            // Record undo action for connection creation
                            self.undo_history
                                .push_action(UndoAction::ConnectionCreated {
                                    from: from_node_id,
                                    to: to_node_id,
                                });

                            self.file.has_unsaved_changes = true;
                        }
                    }
                }
            }
        }
    }

    /// Finds the node at the given canvas position, if any.
    ///
    /// # Arguments
    ///
    /// * `pos` - Position in world space to check
    ///
    /// # Returns
    ///
    /// The ID of the node at that position, or `None` if no node is there
    pub fn find_node_at_position(&self, pos: egui::Pos2) -> Option<NodeId> {
        let node_size = egui::vec2(crate::constants::NODE_WIDTH, crate::constants::NODE_HEIGHT);

        for (id, node) in &self.flowchart.nodes {
            let node_pos = egui::pos2(node.position.0, node.position.1);
            let rect = egui::Rect::from_center_size(node_pos, node_size);

            if rect.contains(pos) {
                return Some(*id);
            }
        }
        None
    }

    /// Finds the connection at the given world position, if any.
    ///
    /// Uses distance-to-line-segment calculation with a threshold for hit detection.
    ///
    /// # Arguments
    ///
    /// * `pos` - Position in world space to check
    ///
    /// # Returns
    ///
    /// The index of the connection in the connections vector, or `None` if no connection is there
    pub fn find_connection_at_position(&self, pos: egui::Pos2) -> Option<usize> {
        self.find_connections_at_position(pos).into_iter().next()
    }

    /// Finds all connections at the given world position, if any.
    ///
    /// Returns a list of indices into the connections vector that are within the
    /// click threshold from the given point. The order is the same as the
    /// rendering order (increasing index).
    pub fn find_connections_at_position(&self, pos: egui::Pos2) -> Vec<usize> {
        let click_threshold = crate::constants::CLICK_THRESHOLD; // pixels in world space
        let mut hits: Vec<usize> = Vec::new();
        for (idx, connection) in self.flowchart.connections.iter().enumerate() {
            if let (Some(from_node), Some(to_node)) = (
                self.flowchart.nodes.get(&connection.from),
                self.flowchart.nodes.get(&connection.to),
            ) {
                let start = egui::pos2(from_node.position.0, from_node.position.1);
                let end = egui::pos2(to_node.position.0, to_node.position.1);
                let distance = self.point_to_line_distance(pos, start, end);
                if distance < click_threshold {
                    hits.push(idx);
                }
            }
        }
        hits
    }

    /// Calculates the distance from a point to a line segment.
    ///
    /// Uses vector projection to find the closest point on the line segment.
    ///
    /// # Arguments
    ///
    /// * `point` - The point to measure from
    /// * `line_start` - Start of the line segment
    /// * `line_end` - End of the line segment
    ///
    /// # Returns
    ///
    /// The minimum distance from the point to the line segment
    fn point_to_line_distance(
        &self,
        point: egui::Pos2,
        line_start: egui::Pos2,
        line_end: egui::Pos2,
    ) -> f32 {
        let line_vec = line_end - line_start;
        let point_vec = point - line_start;
        let line_len_sq = line_vec.length_sq();

        if line_len_sq < 0.0001 {
            // Line segment is essentially a point
            return point_vec.length();
        }

        // Project point onto line segment (clamped to segment endpoints)
        let t = (point_vec.dot(line_vec) / line_len_sq).clamp(0.0, 1.0);
        let projection = line_start + line_vec * t;

        (point - projection).length()
    }
}
