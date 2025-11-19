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
mod editor;
mod file_ops;
mod highlighters;
mod rendering;
mod state;
mod undo;

#[cfg(target_arch = "wasm32")]
use web_sys;

fn is_macos_platform() -> bool {
    #[cfg(target_arch = "wasm32")]
    {
        if let Some(win) = web_sys::window() {
            let nav = win.navigator();
            let platform = nav.platform();
            if let Ok(platform) = platform {
                if platform.contains("Mac") {
                    return true;
                }
            }
            if let Ok(ua) = nav.user_agent() {
                if ua.contains("Mac OS X") || ua.contains("Macintosh") || ua.contains("Mac OS") {
                    return true;
                }
            }
        }
        false
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        cfg!(target_os = "macos")
    }
}

pub use state::FlowchartApp;
pub use undo::{UndoAction, UndoHistory, UndoableFlowchart};

use self::editor::{handle_code_textedit_keys, simple_js_format, CodeEditOptions, LanguageKind};
use self::state::PendingConfirmAction;
use crate::types::*;
use eframe::egui;
#[cfg(target_arch = "wasm32")]
use eframe::wasm_bindgen::JsCast;

impl eframe::App for FlowchartApp {
    /// Persist entire app state between restarts.
    fn save(&mut self, storage: &mut dyn eframe::Storage) {
        match self.to_json() {
            Ok(json) => {
                storage.set_string("app_state", json);
            }
            Err(err) => {
                eprintln!("Failed to serialize app state: {err}");
            }
        }
    }

    /// Main update function called by egui for each frame.
    ///
    /// This method handles the overall UI layout, including the properties panel,
    /// toolbar, and main canvas area. It also processes simulation steps when running.
    ///
    /// # Arguments
    ///
    /// * `ctx` - The egui context
    /// * `frame` - The eframe frame
    fn update(&mut self, ctx: &egui::Context, frame: &mut eframe::Frame) {
        // Apply theme visuals
        let visuals = if self.dark_mode {
            egui::Visuals::dark()
        } else {
            egui::Visuals::light()
        };
        ctx.set_visuals(visuals);

        // Handle pending file operations
        self.handle_pending_operations(ctx);

        // Handle undo/redo keyboard shortcuts
        self.handle_undo_redo_keys(ctx);

        // Handle delete key for removing selected objects
        self.handle_delete_key(ctx);

        // Handle file-related keyboard shortcuts (New/Open/Save)
        self.handle_file_shortcuts(ctx, frame);

        // Handle group-related shortcuts (create/add to group)
        self.handle_group_shortcuts(ctx);

        // Intercept native window close requests (titlebar X)
        #[cfg(not(target_arch = "wasm32"))]
        {
            if ctx.input(|i| i.viewport().close_requested()) {
                if self.file.has_unsaved_changes && !self.file.allow_close_on_next_request {
                    // Abort close and show confirmation dialog
                    ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
                    if !self.file.show_unsaved_dialog {
                        self.file.show_unsaved_dialog = true;
                        self.file.pending_confirm_action = Some(PendingConfirmAction::Quit);
                    }
                } else {
                    // Either no unsaved changes or user confirmed close; allow it and reset the one-shot flag
                    self.file.allow_close_on_next_request = false;
                }
            }
        }

        // Properties panel on the right side
        #[cfg(target_arch = "wasm32")]
        {
            // Update browser beforeunload prompt based on unsaved state
            Self::update_beforeunload(self.file.has_unsaved_changes);
        }

        // Restore native window size once per session (desktop only)
        #[cfg(not(target_arch = "wasm32"))]
        {
            if !self.applied_viewport_restore {
                if let Some((w, h)) = self.window_inner_size {
                    // Apply stored inner size
                    ctx.send_viewport_cmd(egui::ViewportCommand::InnerSize(egui::vec2(w, h)));
                }
                self.applied_viewport_restore = true;
            }
            // Capture current window inner size to persist on save
            let size = ctx.input(|i| i.screen_rect().size());
            self.window_inner_size = Some((size.x, size.y));
        }

        // Top toolbar occupies full width and is independent of the properties panel
        egui::TopBottomPanel::top("top_toolbar").show(ctx, |ui| {
            self.draw_toolbar(ui);
        });

        // Properties panel should only take space from the canvas area below the toolbar
        let viewport_width = ctx.input(|i| i.screen_rect().width());
        // Use remembered width when available, but clamp to viewport
        let clamped_width = self
            .properties_panel_width
            .clamp(180.0, (viewport_width * 0.9).max(180.0));

        // Right-side properties panel lives alongside the canvas (below the toolbar)
        egui::SidePanel::right("properties_panel")
            .resizable(true)
            .default_width(clamped_width)
            .show(ctx, |ui| {
                // Capture the current width each frame so we can remember it
                let current_width = ui.available_width();
                // Only update if within viewport constraints
                let max_allowed = (viewport_width * 0.9).max(180.0);
                self.properties_panel_width = current_width.clamp(180.0, max_allowed);
                self.draw_properties_panel(ui);
            });

        // Central canvas area (below the toolbar)
        egui::CentralPanel::default().show(ctx, |ui| {
            // Canvas takes remaining space
            self.draw_canvas(ui);
        });

        // Unsaved changes confirmation dialog
        if self.file.show_unsaved_dialog {
            let title = match self.file.pending_confirm_action {
                Some(PendingConfirmAction::Quit) => "Unsaved changes — Quit?",
                Some(PendingConfirmAction::New) => "Unsaved changes — Create New?",
                Some(PendingConfirmAction::Open) => "Unsaved changes — Open File?",
                None => "Unsaved changes",
            };
            egui::Window::new(title)
                .collapsible(false)
                .resizable(false)
                .anchor(egui::Align2::CENTER_CENTER, egui::vec2(0.0, 0.0))
                .show(ctx, |ui| {
                    ui.label("You have unsaved changes. Are you sure you want to continue?");
                    ui.horizontal(|ui| {
                        // Primary confirm button depends on action
                        let confirm_label = match self.file.pending_confirm_action {
                            Some(PendingConfirmAction::Quit) => "Discard and Quit",
                            Some(PendingConfirmAction::New) => "Discard and Create New",
                            Some(PendingConfirmAction::Open) => "Discard and Open",
                            None => "Discard",
                        };
                        if ui.button(confirm_label).clicked() {
                            match self.file.pending_confirm_action {
                                Some(PendingConfirmAction::New) => {
                                    self.new_flowchart();
                                }
                                Some(PendingConfirmAction::Open) => {
                                    self.load_flowchart();
                                }
                                Some(PendingConfirmAction::Quit) => {
                                    // Allow one close request to pass without interception
                                    self.file.allow_close_on_next_request = true;
                                    ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                                }
                                None => {}
                            }
                            self.file.show_unsaved_dialog = false;
                            self.file.pending_confirm_action = None;
                        }
                        if ui.button("Cancel").clicked() {
                            self.file.show_unsaved_dialog = false;
                            self.file.pending_confirm_action = None;
                        }
                    });
                });
        }

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
    /// Handle keyboard shortcuts related to groups (Ctrl/Cmd+G to group selected nodes)
    fn handle_group_shortcuts(&mut self, ctx: &egui::Context) {
        // Avoid interfering while editing text fields
        if ctx.wants_keyboard_input() {
            return;
        }
        // Detect Cmd/Ctrl+G in a way that works both in app and in headless tests:
        // 1) Prefer the high-level key/modifier state for normal runtime
        // 2) Fall back to scanning raw input events for a Key::G press with command/ctrl
        let pressed = ctx.input(|i| {
            let high_level = i.key_pressed(egui::Key::G) && (i.modifiers.command || i.modifiers.ctrl);
            if high_level {
                return true;
            }
            // Fallback: scan raw events (useful in tests where i.modifiers may not be updated)
            i.events.iter().any(|ev| match ev {
                egui::Event::Key { key, pressed: true, modifiers, .. } if *key == egui::Key::G => {
                    modifiers.command || modifiers.ctrl
                }
                _ => false,
            })
        });
        if !pressed {
            return;
        }

        // Determine nodes to group: use multi-selection if any; otherwise use single selected node
        let mut nodes_to_group: Vec<NodeId> = if !self.interaction.selected_nodes.is_empty() {
            self.interaction.selected_nodes.clone()
        } else if let Some(id) = self.interaction.selected_node {
            vec![id]
        } else {
            Vec::new()
        };
        nodes_to_group.sort();
        nodes_to_group.dedup();
        if nodes_to_group.is_empty() {
            return;
        }

        // If a group is currently selected, add the nodes to that group; otherwise create a new group
        if let Some(gid) = self.interaction.selected_group {
            if let Some(group) = self.flowchart.groups.get_mut(&gid) {
                for id in nodes_to_group {
                    if !group.members.contains(&id) {
                        group.members.push(id);
                    }
                }
                self.file.has_unsaved_changes = true;
            }
        } else {
            let gid = uuid::Uuid::new_v4();
            let name = format!("Group {}", self.group_counter + 1);
            self.group_counter += 1;
            self.flowchart.groups.insert(
                gid,
                crate::types::Group {
                    id: gid,
                    name,
                    members: nodes_to_group,
                    drawing: crate::types::GroupDrawingMode::Rectangle,
                },
            );
            // Record undo action for group creation
            self
                .undo_history
                .push_action(UndoAction::GroupCreated { group_id: gid });
            // Select the new group
            self.interaction.selected_group = Some(gid);
            self.interaction.selected_node = None;
            self.interaction.selected_nodes.clear();
            self.interaction.selected_connection = None;
            // Start editing the group name immediately (focus and select-all handled in UI)
            if let Some(g) = self.flowchart.groups.get(&gid) {
                self.interaction.editing_group_name = Some(gid);
                self.interaction.temp_group_name = g.name.clone();
                self.interaction.should_select_text = true;
                self.interaction.focus_requested_for_edit = false;
            }
            self.file.has_unsaved_changes = true;
        }
    }

    /// Computes the world-space rect of a node (centered at position) with padding 0.
    fn node_world_rect(&self, node: &FlowchartNode) -> egui::Rect {
        // Keep in sync with rendering node size
        let node_size = egui::vec2(crate::constants::NODE_WIDTH, crate::constants::NODE_HEIGHT);
        let center = egui::pos2(node.position.0, node.position.1);
        egui::Rect::from_center_size(center, node_size)
    }

    /// Computes the world-space bounding rect of a group with padding, depending on drawing mode.
    fn group_world_rect(&self, group_id: crate::types::GroupId) -> Option<egui::Rect> {
        let group = self.flowchart.groups.get(&group_id)?;
        match group.drawing {
            crate::types::GroupDrawingMode::Rectangle => {
                if group.members.is_empty() {
                    return None;
                }
                let mut rect_opt: Option<egui::Rect> = None;
                for nid in &group.members {
                    if let Some(node) = self.flowchart.nodes.get(nid) {
                        let nr = self.node_world_rect(node);
                        rect_opt = Some(if let Some(r) = rect_opt { r.union(nr) } else { nr });
                    }
                }
                rect_opt.map(|r| {
                    let pad = crate::constants::GROUP_PADDING;
                    r.expand2(egui::vec2(pad, pad))
                })
            }
            crate::types::GroupDrawingMode::Polygon => {
                // Use the hull's bounding box
                self.group_world_polygon(group_id)
                    .map(|pts| egui::Rect::from_points(&pts))
            }
        }
    }

    /// Compute shrink-wrapped polygon (convex hull) around the group's member node rects expanded by padding.
    fn group_world_polygon(&self, group_id: crate::types::GroupId) -> Option<Vec<egui::Pos2>> {
        let group = self.flowchart.groups.get(&group_id)?;
        if group.members.is_empty() {
            return None;
        }
        // Regular polygon mode uses full padding and only rectangle corners (convex hull)
        let pad = crate::constants::GROUP_PADDING;
        let mut points: Vec<egui::Pos2> = Vec::new();
        for nid in &group.members {
            if let Some(node) = self.flowchart.nodes.get(nid) {
                let mut r = self.node_world_rect(node);
                r = r.expand2(egui::vec2(pad, pad));
                points.push(r.min);
                points.push(egui::pos2(r.max.x, r.min.y));
                points.push(r.max);
                points.push(egui::pos2(r.min.x, r.max.y));
            }
        }
        if points.len() <= 1 {
            return None;
        }
        // Convex hull via monotone chain
        points.sort_by(|a, b| {
            a.x
                .partial_cmp(&b.x)
                .unwrap_or(std::cmp::Ordering::Equal)
                .then(a.y.partial_cmp(&b.y).unwrap_or(std::cmp::Ordering::Equal))
        });
        fn cross(o: egui::Pos2, a: egui::Pos2, b: egui::Pos2) -> f32 {
            (a.x - o.x) * (b.y - o.y) - (a.y - o.y) * (b.x - o.x)
        }
        let mut lower: Vec<egui::Pos2> = Vec::new();
        for p in &points {
            while lower.len() >= 2
                && cross(lower[lower.len() - 2], lower[lower.len() - 1], *p) <= 0.0
            {
                lower.pop();
            }
            lower.push(*p);
        }
        let mut upper: Vec<egui::Pos2> = Vec::new();
        for p in points.iter().rev() {
            while upper.len() >= 2
                && cross(upper[upper.len() - 2], upper[upper.len() - 1], *p) <= 0.0
            {
                upper.pop();
            }
            upper.push(*p);
        }
        lower.pop();
        upper.pop();
        let mut hull = lower;
        hull.extend(upper);
        if hull.len() < 3 {
            None
        } else {
            Some(hull)
        }
    }


    /// Returns a group id if the world position is inside any group's background shape.
    fn find_group_at_position(&self, world_pos: egui::Pos2) -> Option<crate::types::GroupId> {
        // If multiple hit, prefer the smallest area (innermost group)
        let mut best: Option<(crate::types::GroupId, f32)> = None;
        for (gid, g) in &self.flowchart.groups {
            match g.drawing {
                crate::types::GroupDrawingMode::Rectangle => {
                    if let Some(r) = self.group_world_rect(*gid) {
                        if r.contains(world_pos) {
                            let area = r.area();
                            match best {
                                None => best = Some((*gid, area)),
                                Some((_, best_area)) => {
                                    if area < best_area {
                                        best = Some((*gid, area));
                                    }
                                }
                            }
                        }
                    }
                }
                crate::types::GroupDrawingMode::Polygon => {
                    if let Some(poly) = self.group_world_polygon(*gid) {
                        // Quick bbox reject
                        let bbox = egui::Rect::from_points(&poly);
                        if bbox.contains(world_pos) {
                            // Ray casting point-in-polygon (works for concave/convex)
                            let mut inside = false;
                            let mut j = poly.len() - 1;
                            for i in 0..poly.len() {
                                let pi = poly[i];
                                let pj = poly[j];
                                let intersect = ((pi.y > world_pos.y) != (pj.y > world_pos.y))
                                    && (world_pos.x
                                        < (pj.x - pi.x) * (world_pos.y - pi.y) / ((pj.y - pi.y).max(1e-6f32))
                                            + pi.x);
                                if intersect {
                                    inside = !inside;
                                }
                                j = i;
                            }
                            if inside {
                                let area = bbox.area();
                                match best {
                                    None => best = Some((*gid, area)),
                                    Some((_, best_area)) => {
                                        if area < best_area {
                                            best = Some((*gid, area));
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
        best.map(|(gid, _)| gid)
    }
    #[cfg(target_arch = "wasm32")]
    fn update_beforeunload(has_unsaved_changes: bool) {
        if let Some(window) = web_sys::window() {
            if has_unsaved_changes {
                let closure = eframe::wasm_bindgen::closure::Closure::wrap(Box::new(
                    move |event: web_sys::Event| {
                        event.prevent_default();
                        // Set returnValue to trigger the confirmation dialog in some browsers
                        let _ = js_sys::Reflect::set(
                            event.as_ref(),
                            &eframe::wasm_bindgen::JsValue::from_str("returnValue"),
                            &eframe::wasm_bindgen::JsValue::from_str("unsaved"),
                        );
                    },
                )
                    as Box<dyn FnMut(_)>);
                // SAFETY: forgetting the closure is acceptable here; it lives for the page lifetime.
                window.set_onbeforeunload(Some(closure.as_ref().unchecked_ref()));
                closure.forget();
            } else {
                window.set_onbeforeunload(None);
            }
        }
    }
    /// Handles file-related keyboard shortcuts: New, Open, Save, Save As, and Quit.
    /// Uses the platform-standard Command (macOS) or Control (Windows/Linux) modifier.
    fn handle_file_shortcuts(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        let is_editing_text = ctx.wants_keyboard_input();
        if is_editing_text {
            return;
        }
        #[cfg(target_arch = "wasm32")]
        let request_quit = false;
        #[cfg(not(target_arch = "wasm32"))]
        let mut request_quit = false;
        ctx.input(|i| {
            let cmd = i.modifiers.command;
            let shift = i.modifiers.shift;
            // Save As: Cmd/Ctrl+Shift+S
            if i.key_pressed(egui::Key::S) && cmd && shift {
                self.save_as_flowchart();
            }
            // Save: Cmd/Ctrl+S
            else if i.key_pressed(egui::Key::S) && cmd {
                self.save_flowchart();
            }
            // Open: Cmd/Ctrl+O
            if i.key_pressed(egui::Key::O) && cmd {
                if self.file.has_unsaved_changes {
                    self.file.show_unsaved_dialog = true;
                    self.file.pending_confirm_action = Some(PendingConfirmAction::Open);
                } else {
                    self.load_flowchart();
                }
            }
            // New: Cmd/Ctrl+N
            if i.key_pressed(egui::Key::N) && cmd {
                if self.file.has_unsaved_changes {
                    self.file.show_unsaved_dialog = true;
                    self.file.pending_confirm_action = Some(PendingConfirmAction::New);
                } else {
                    self.new_flowchart();
                }
            }
            // Quit: Cmd/Ctrl+Q (native only)
            #[cfg(not(target_arch = "wasm32"))]
            if i.key_pressed(egui::Key::Q) && cmd {
                if self.file.has_unsaved_changes {
                    self.file.show_unsaved_dialog = true;
                    self.file.pending_confirm_action = Some(PendingConfirmAction::Quit);
                } else {
                    request_quit = true;
                }
            }
        });
        if request_quit {
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
        }
    }

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
            // If a group is selected, delete the group (but keep its nodes and connections)
            if let Some(gid) = self.interaction.selected_group {
                // If we are currently editing this group's name, ignore Delete to avoid accidental removal
                if self.interaction.editing_group_name == Some(gid) {
                    return;
                }
                if let Some(group) = self.flowchart.groups.get(&gid).cloned() {
                    // Record undo action before deletion
                    self.undo_history
                        .push_action(UndoAction::GroupDeleted { group: group.clone() });

                    // Perform deletion of the group only
                    self.flowchart.groups.remove(&gid);

                    // Clear group selection/editing state
                    self.interaction.selected_group = None;
                    if self.interaction.editing_group_name == Some(gid) {
                        self.interaction.editing_group_name = None;
                        self.interaction.temp_group_name.clear();
                    }

                    self.file.has_unsaved_changes = true;
                    return; // handled delete key
                }
            }

            // If multiple nodes are selected, delete them together
            if self.interaction.selected_nodes.len() > 1 {
                // Capture nodes and connections for undo
                let mut nodes: Vec<FlowchartNode> = Vec::new();
                let mut connections: Vec<Connection> = Vec::new();

                let selected_set: std::collections::HashSet<NodeId> =
                    self.interaction.selected_nodes.iter().copied().collect();

                // Collect node data
                for id in &self.interaction.selected_nodes {
                    if let Some(node) = self.flowchart.nodes.get(id).cloned() {
                        nodes.push(node);
                    }
                }
                // Collect all connections involving any selected node
                for c in &self.flowchart.connections {
                    if selected_set.contains(&c.from) || selected_set.contains(&c.to) {
                        connections.push(c.clone());
                    }
                }

                // Record combined undo action
                self.undo_history
                    .push_action(UndoAction::MultipleNodesDeleted {
                        nodes: nodes.clone(),
                        connections: connections.clone(),
                    });

                // Perform deletion
                for id in &self.interaction.selected_nodes {
                    let _ = self.flowchart.remove_node(id);
                }

                // Clear selection
                self.interaction.selected_nodes.clear();
                self.interaction.selected_node = None;
                self.interaction.selected_connection = None;
                self.interaction.selected_group = None;
                self.interaction.editing_node_name = None;
                self.file.has_unsaved_changes = true;
            } else if let Some(selected_node) = self.interaction.selected_node {
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

                // Remove the selected node (also updates groups and connections)
                let _ = self.flowchart.remove_node(&selected_node);

                // Clear selection
                self.interaction.selected_node = None;
                self.interaction.selected_nodes.clear();
                self.interaction.selected_group = None;
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
                if self.file.has_unsaved_changes {
                    self.file.show_unsaved_dialog = true;
                    self.file.pending_confirm_action = Some(PendingConfirmAction::New);
                } else {
                    self.new_flowchart();
                }
            }
            if ui.button("Open").clicked() {
                if self.file.has_unsaved_changes {
                    self.file.show_unsaved_dialog = true;
                    self.file.pending_confirm_action = Some(PendingConfirmAction::Open);
                } else {
                    self.load_flowchart();
                }
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
                    match &mut node.node_type {
                        NodeType::Producer { messages_produced, .. } => {
                            *messages_produced = 0;
                        }
                        NodeType::Transformer { globals, initial_globals, .. } => {
                            // Reset transformer globals to their initial values
                            *globals = initial_globals.clone();
                        }
                        _ => {}
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

            // Auto-arrange apply button + combo box to choose mode
            if ui.button("Auto Layout").clicked() {
                self.apply_auto_arrangement();
            }
            egui::ComboBox::from_id_source("auto_arrange_mode_combo")
                .selected_text(match self.auto_arrange_mode {
                    crate::ui::state::AutoArrangeMode::ForceDirected => "Force-directed",
                    crate::ui::state::AutoArrangeMode::Grid => "Grid",
                    crate::ui::state::AutoArrangeMode::Line => "Line",
                })
                .show_ui(ui, |ui| {
                    ui.selectable_value(
                        &mut self.auto_arrange_mode,
                        crate::ui::state::AutoArrangeMode::ForceDirected,
                        "Force-directed",
                    );
                    ui.selectable_value(
                        &mut self.auto_arrange_mode,
                        crate::ui::state::AutoArrangeMode::Grid,
                        "Grid",
                    );
                    ui.selectable_value(
                        &mut self.auto_arrange_mode,
                        crate::ui::state::AutoArrangeMode::Line,
                        "Line",
                    );
                });

            ui.separator();

            // View options
            ui.checkbox(&mut self.canvas.show_grid, "Show Grid");
            ui.separator();
            ui.checkbox(&mut self.dark_mode, "Dark Mode");

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
        egui::ScrollArea::vertical()
            .auto_shrink([false; 2])
            .show(ui, |ui| {
                ui.vertical(|ui| {
                    ui.heading("Properties");
            ui.separator();

            if let Some(gid) = self.interaction.selected_group {
                ui.label("Type: Group");
                ui.separator();

                if let Some(group) = self.flowchart.groups.get(&gid).cloned() {
                    ui.label("Name:");
                    if self.interaction.editing_group_name == Some(gid) {
                        // Ensure temp is initialized
                        if self.interaction.temp_group_name.is_empty() {
                            self.interaction.temp_group_name = group.name.clone();
                        }
                        let response = ui.text_edit_singleline(&mut self.interaction.temp_group_name);

                        // Only request focus on the first frame of editing
                        if !self.interaction.focus_requested_for_edit {
                            response.request_focus();
                            self.interaction.focus_requested_for_edit = true;
                        }
                        // Select all text when flag is set and field has focus
                        if self.interaction.should_select_text && response.has_focus() {
                            self.interaction.should_select_text = false;
                            self.select_all_text_in_field_with_len(ui, response.id, self.interaction.temp_group_name.len());
                        }

                        // Handle Enter key to save changes (don't require focus in case it's the first frame)
                        if ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                            // Commit change
                            if let Some(g) = self.flowchart.groups.get_mut(&gid) {
                                let old_name = g.name.clone();
                                let new_name = self.interaction.temp_group_name.trim().to_string();
                                if !new_name.is_empty() && new_name != old_name {
                                    g.name = new_name;
                                    // We could add a dedicated undo action in future
                                    self.file.has_unsaved_changes = true;
                                }
                            }
                            self.interaction.editing_group_name = None;
                            self.interaction.temp_group_name.clear();
                        }

                        // Save on focus lost as well
                        if response.lost_focus() && !ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                            if let Some(g) = self.flowchart.groups.get_mut(&gid) {
                                let old_name = g.name.clone();
                                let new_name = self.interaction.temp_group_name.trim().to_string();
                                if !new_name.is_empty() && new_name != old_name {
                                    g.name = new_name;
                                    self.file.has_unsaved_changes = true;
                                }
                            }
                            self.interaction.editing_group_name = None;
                            self.interaction.temp_group_name.clear();
                        }
                    } else if ui.button(&group.name).clicked() {
                        self.interaction.editing_group_name = Some(gid);
                        self.interaction.temp_group_name = group.name.clone();
                        self.interaction.should_select_text = true;
                        self.interaction.focus_requested_for_edit = false;
                    }

                    ui.separator();
                    // Drawing mode selector
                    ui.label("Drawing mode:");
                    let mut drawing = group.drawing;
                    egui::ComboBox::from_id_source("group_drawing_mode")
                        .selected_text(match drawing {
                            crate::types::GroupDrawingMode::Rectangle => "Rectangle",
                            crate::types::GroupDrawingMode::Polygon => "Polygon",
                        })
                        .show_ui(ui, |ui| {
                            ui.selectable_value(
                                &mut drawing,
                                crate::types::GroupDrawingMode::Rectangle,
                                "Rectangle",
                            );
                            ui.selectable_value(
                                &mut drawing,
                                crate::types::GroupDrawingMode::Polygon,
                                "Polygon",
                            );
                        });
                    if drawing != group.drawing {
                        if let Some(g) = self.flowchart.groups.get_mut(&gid) {
                            g.drawing = drawing;
                            self.file.has_unsaved_changes = true;
                        }
                    }
                    ui.separator();
                    // Show member count and names
                    ui.label(format!("Members: {}", group.members.len()));
                    for nid in &group.members {
                        if let Some(n) = self.flowchart.nodes.get(nid) {
                            ui.label(format!("• {}", n.name));
                        }
                    }
                } else {
                    ui.label("Group not found");
                }
            } else if let Some(selected_id) = self.interaction.selected_node {
                if let Some(node) = self.flowchart.nodes.get(&selected_id).cloned() {
                    // If the currently selected node is NOT a Transformer, clear any
                    // Transformer globals staging so edits don't linger across types.
                    if !matches!(node.node_type, NodeType::Transformer { .. }) {
                        if self.interaction.temp_globals_node_id.is_some()
                            || !self.interaction.temp_transformer_globals_edits.is_empty()
                        {
                            self.interaction.temp_transformer_globals_edits.clear();
                            self.interaction.temp_globals_node_id = None;
                        }
                    }

                    ui.label("Type: Node");
                    ui.separator();

                    // Node name editing
                    ui.label("Name:");
                    if self.interaction.editing_node_name == Some(selected_id) {
                        self.draw_name_editor(ui, selected_id);
                    } else if ui.button(&node.name).clicked() {
                        self.start_editing_node_name(selected_id, &node.name);
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
                // Non-transformer selection: clear transformer globals staging
                if self.interaction.temp_globals_node_id.is_some()
                    || !self.interaction.temp_transformer_globals_edits.is_empty()
                {
                    self.interaction.temp_transformer_globals_edits.clear();
                    self.interaction.temp_globals_node_id = None;
                }
                if let Some(connection) = self.flowchart.connections.get(conn_idx) {
                    self.draw_connection_properties(ui, connection);
                } else {
                    ui.label("Connection not found");
                }
            } else {
                // No selection: clear transformer globals staging
                if self.interaction.temp_globals_node_id.is_some()
                    || !self.interaction.temp_transformer_globals_edits.is_empty()
                {
                    self.interaction.temp_transformer_globals_edits.clear();
                    self.interaction.temp_globals_node_id = None;
                }
                self.draw_no_selection_info(ui);
            }
        });
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

    /// Selects all text in a text edit field using a provided length.
    /// Useful for selecting group-name fields whose content is stored separately.
    fn select_all_text_in_field_with_len(
        &self,
        ui: &mut egui::Ui,
        field_id: egui::Id,
        len: usize,
    ) {
        ui.memory_mut(|mem| {
            let state = mem
                .data
                .get_temp_mut_or_default::<egui::text_edit::TextEditState>(field_id);
            state
                .cursor
                .set_char_range(Some(egui::text::CCursorRange::two(
                    egui::text::CCursor::new(0),
                    egui::text::CCursor::new(len),
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
            if let NodeType::Transformer { script, .. } = &node.node_type {
                match property {
                    "script" => {
                        let new_script = self
                            .interaction
                            .temp_transformer_script
                            .replace("\t", "    ");
                        // Only record undo if script actually changed
                        if script != &new_script {
                            let old_node_type = node.node_type.clone();
                            let selected_outputs = if let NodeType::Transformer {
                                selected_outputs,
                                ..
                            } = &node.node_type
                            {
                                selected_outputs.clone()
                            } else {
                                None
                            };
                            let current_globals = if let NodeType::Transformer { globals, .. } = &node.node_type { globals.clone() } else { Default::default() };
                            let current_initial_globals = if let NodeType::Transformer { initial_globals, .. } = &node.node_type { initial_globals.clone() } else { Default::default() };
                            let new_node_type = NodeType::Transformer {
                                script: new_script,
                                selected_outputs,
                                globals: current_globals,
                                initial_globals: current_initial_globals,
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
                                // Clear error state on script edits
                                if let NodeState::Error(_) = node.state {
                                    node.state = NodeState::Idle;
                                }
                            }
                            // Clear global error highlight when script changes
                            self.error_node = None;
                            self.file.has_unsaved_changes = true;
                        }
                    }
                    "globals" => {
                        // Parse temp edits into a map
                        let mut new_map: serde_json::Map<String, serde_json::Value> = serde_json::Map::new();
                        let mut parse_failed = false;
                        for (k, vstr) in &self.interaction.temp_transformer_globals_edits {
                            match serde_json::from_str::<serde_json::Value>(vstr.trim()) {
                                Ok(v) => {
                                    new_map.insert(k.clone(), v);
                                }
                                Err(_) => {
                                    parse_failed = true;
                                    break;
                                }
                            }
                        }
                        if !parse_failed {
                            let old_node_type = node.node_type.clone();
                            let (cur_script, selected_outputs) = if let NodeType::Transformer { script, selected_outputs, .. } = &node.node_type {
                                (script.clone(), selected_outputs.clone())
                            } else {
                                (String::new(), None)
                            };
                            let new_node_type = NodeType::Transformer {
                                script: cur_script,
                                selected_outputs,
                                globals: new_map.clone(),
                                initial_globals: new_map,
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
                    _ => {}
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

                let mut edited = false;

                // Enhanced editing: Tab/Shift+Tab indentation and Enter indentation
                let opts = CodeEditOptions {
                    language: LanguageKind::Json,
                    indent: "    ",
                };
                if handle_code_textedit_keys(
                    ui,
                    &text_edit_response,
                    &mut self.interaction.temp_producer_message_template,
                    &opts,
                ) {
                    edited = true;
                }

                // Pretty‑format JSON on Ctrl+Shift+F (or Cmd+Shift+F)
                let format_shortcut = ui.input(|i| {
                    (i.modifiers.ctrl || i.modifiers.command)
                        && i.modifiers.shift
                        && i.key_pressed(egui::Key::F)
                });
                if text_edit_response.has_focus() && format_shortcut {
                    if let Ok(value) = serde_json::from_str::<serde_json::Value>(
                        &self.interaction.temp_producer_message_template,
                    ) {
                        if let Ok(pretty) = serde_json::to_string_pretty(&value) {
                            self.interaction.temp_producer_message_template = pretty;
                            edited = true;
                        }
                    }
                }

                if edited || text_edit_response.changed() {
                    self.update_producer_property(node.id, "message_template");
                }

                // Hint: formatting shortcut
                let hint = if is_macos_platform() {
                    "Tip: Press Cmd+Shift+F to format JSON."
                } else {
                    "Tip: Press Ctrl+Shift+F to format JSON."
                };
                ui.add(egui::Label::new(egui::RichText::new(hint).small().italics()).wrap());
            }
            NodeType::Consumer { consumption_rate } => {
                ui.label(format!("Consumption Rate: {} msg/step", consumption_rate));
            }
            NodeType::Transformer { script, .. } => {
                // Initialize temp value if empty or if out of sync with selected node
                if self.interaction.temp_transformer_script.is_empty() {
                    self.interaction.temp_transformer_script = script.clone();
                }

                ui.label("JavaScript Script:");

                // Determine a max height of ~50 lines based on monospace row height
                let row_height = ui.text_style_height(&egui::TextStyle::Monospace).max(12.0);
                let max_height = row_height * 50.0 + 8.0; // small padding

                // Store a reference for the layouter and a mutable copy for editing
                let layouter_ref = self.interaction.temp_transformer_script.clone();
                let mut layouter = rendering::create_js_layouter(&layouter_ref);

                egui::ScrollArea::vertical()
                    .max_height(max_height)
                    .show(ui, |ui| {
                        let text_edit_response = ui.add(
                            egui::TextEdit::multiline(
                                &mut self.interaction.temp_transformer_script,
                            )
                            .desired_rows(10)
                            .desired_width(f32::INFINITY)
                            .font(egui::TextStyle::Monospace)
                            .lock_focus(true)
                            .layouter(&mut layouter),
                        );

                        let mut edited = false;
                        let opts = CodeEditOptions {
                            language: LanguageKind::JavaScript,
                            indent: "    ",
                        };
                        if handle_code_textedit_keys(
                            ui,
                            &text_edit_response,
                            &mut self.interaction.temp_transformer_script,
                            &opts,
                        ) {
                            edited = true;
                        }

                        // JS pretty format on Ctrl/Cmd+Shift+F
                        let js_format_shortcut = ui.input(|i| {
                            (i.modifiers.ctrl || i.modifiers.command)
                                && i.modifiers.shift
                                && i.key_pressed(egui::Key::F)
                        });
                        if text_edit_response.has_focus() && js_format_shortcut {
                            let formatted = simple_js_format(
                                &self.interaction.temp_transformer_script,
                                opts.indent,
                            );
                            if formatted != self.interaction.temp_transformer_script {
                                self.interaction.temp_transformer_script = formatted;
                                edited = true;
                            }
                        }

                        if edited || text_edit_response.changed() {
                            self.update_transformer_property(node.id, "script");
                        }

                        // Hint: formatting shortcut
                        let hint = if is_macos_platform() {
                            "Tip: Press Cmd+Shift+F to format JavaScript."
                        } else {
                            "Tip: Press Ctrl+Shift+F to format JavaScript."
                        };
                        ui.add(
                            egui::Label::new(egui::RichText::new(hint).small().italics()).wrap(),
                        );
                    });

                // Show last script error if any
                if let NodeState::Error(msg) = &node.state {
                    ui.separator();
                    ui.colored_label(egui::Color32::RED, format!("Script error: {}", msg));
                }

                ui.separator();
                ui.heading("Global State");
                ui.label("These values are available in scripts as globalThis.state");
                ui.add_space(4.0);

                // Initialize or reload temp globals buffer if empty or if node changed
                if self.interaction.temp_transformer_globals_edits.is_empty()
                    || self.interaction.temp_globals_node_id != Some(node.id) {

                    // Before switching to a new node, save any pending edits from the previous node
                    if let Some(prev_node_id) = self.interaction.temp_globals_node_id {
                        if prev_node_id != node.id && !self.interaction.temp_transformer_globals_edits.is_empty() {
                            // Auto-save the edits to the previous node
                            if let Some(prev_node) = self.flowchart.nodes.get(&prev_node_id) {
                                if let NodeType::Transformer { .. } = &prev_node.node_type {
                                    // Parse and save the temp edits
                                    let mut new_map: serde_json::Map<String, serde_json::Value> = serde_json::Map::new();
                                    let mut all_valid = true;
                                    for (k, vstr) in &self.interaction.temp_transformer_globals_edits {
                                        match serde_json::from_str::<serde_json::Value>(vstr.trim()) {
                                            Ok(v) => {
                                                new_map.insert(k.clone(), v);
                                            }
                                            Err(_) => {
                                                all_valid = false;
                                                break;
                                            }
                                        }
                                    }
                                    // Only save if all values are valid JSON
                                    if all_valid {
                                        let old_node_type = prev_node.node_type.clone();
                                        let (cur_script, selected_outputs, cur_globals) = if let NodeType::Transformer { script, selected_outputs, globals, .. } = &prev_node.node_type {
                                            (script.clone(), selected_outputs.clone(), globals.clone())
                                        } else {
                                            (String::new(), None, Default::default())
                                        };
                                        let new_node_type = NodeType::Transformer {
                                            script: cur_script,
                                            selected_outputs,
                                            globals: cur_globals,
                                            initial_globals: new_map,
                                        };
                                        // Record undo action
                                        self.undo_history.push_action(UndoAction::PropertyChanged {
                                            node_id: prev_node_id,
                                            old_node_type,
                                            new_node_type: new_node_type.clone(),
                                        });
                                        // Apply the change
                                        if let Some(node_mut) = self.flowchart.nodes.get_mut(&prev_node_id) {
                                            node_mut.node_type = new_node_type;
                                        }
                                        self.file.has_unsaved_changes = true;
                                    }
                                }
                            }
                        }
                    }

                    // Now load the new node's globals
                    self.interaction.temp_transformer_globals_edits.clear();
                    self.interaction.temp_globals_node_id = Some(node.id);
                    if let NodeType::Transformer { initial_globals, .. } = &node.node_type {
                        for (k, v) in initial_globals.iter() {
                            let s = serde_json::to_string_pretty(v).unwrap_or_else(|_| "null".to_string());
                            self.interaction
                                .temp_transformer_globals_edits
                                .insert(k.clone(), s);
                        }
                    }
                }

                // New entry inputs - horizontally aligned
                ui.label(egui::RichText::new("Add New Global Variable:").strong());
                let mut global_var_def_error: Option<String> = None;
                ui.horizontal(|ui| {
                    ui.label("Key:");
                    ui.add_sized([100.0, 20.0], egui::TextEdit::singleline(&mut self.interaction.temp_new_global_key));

                    ui.add_space(8.0);

                    ui.label("Initial Value (JSON):");
                    ui.add_sized([120.0, 20.0], egui::TextEdit::singleline(&mut self.interaction.temp_new_global_value));

                    ui.add_space(8.0);

                    if ui.button("Add").clicked() {
                        let key = self.interaction.temp_new_global_key.trim().to_string();
                        if !key.is_empty()
                            && !self
                                .interaction
                                .temp_transformer_globals_edits
                                .contains_key(&key)
                        {
                            // Validate JSON
                            match serde_json::from_str::<serde_json::Value>(
                                self.interaction.temp_new_global_value.trim(),
                            ) {
                                Ok(_) => {
                                    self.interaction
                                        .temp_transformer_globals_edits
                                        .insert(key, self.interaction.temp_new_global_value.trim().to_string());
                                    self.interaction.temp_new_global_key.clear();
                                    self.interaction.temp_new_global_value.clear();
                                }
                                Err(_) => {
                                    // It didn't parse normally, try wrapping it in quotes
                                    let quoted_string = format!("\"{}\"", self.interaction.temp_new_global_value.trim());
                                    match serde_json::from_str::<serde_json::Value>(
                                        &quoted_string,
                                    ) {
                                        Ok(_) => {
                                            self.interaction
                                                .temp_transformer_globals_edits
                                                .insert(key, quoted_string);
                                            self.interaction.temp_new_global_key.clear();
                                            self.interaction.temp_new_global_value.clear();
                                        }
                                        Err(err) => {
                                            global_var_def_error = Some(format!("Invalid JSON: {err}"));
                                        }
                                    }
                                }
                            }
                        }
                    }
                });
                if let Some(var_def_error) = global_var_def_error {
                    ui.colored_label(egui::Color32::RED, var_def_error);
                }

                ui.add_space(8.0);
                ui.separator();
                ui.add_space(4.0);

                // Existing entries table - enhanced with saved vs current value
                ui.label(egui::RichText::new("Global Variables:").strong());
                ui.add_space(4.0);

                // Get current runtime values from node
                let current_values = if let NodeType::Transformer { globals, .. } = &node.node_type {
                    globals.clone()
                } else {
                    Default::default()
                };

                // Stable order
                let mut keys: Vec<String> = self
                    .interaction
                    .temp_transformer_globals_edits
                    .keys()
                    .cloned()
                    .collect();
                keys.sort();

                let mut to_remove: Option<String> = None;

                        egui::Grid::new("transformer_globals_table")
                            .num_columns(4)
                            .striped(true)
                            .spacing([10.0, 4.0])
                            .show(ui, |ui| {
                                // Header row
                                ui.label(egui::RichText::new("Key").strong());
                                ui.label(egui::RichText::new("Saved Value").strong())
                                    .on_hover_text("Initial value used at simulation start");
                                ui.label(egui::RichText::new("Current Value").strong())
                                    .on_hover_text("Value updated during simulation");
                                ui.label("");
                                ui.end_row();

                                for key in &keys {
                                    let saved_val_str = self
                                        .interaction
                                        .temp_transformer_globals_edits
                                        .get_mut(key)
                                        .unwrap();

                                    // Key column (read-only)
                                    ui.label(egui::RichText::new(key).monospace());

                                    // Saved value column (editable)
                                    let saved_response = ui.add_sized(
                                        [150.0, 20.0],
                                        egui::TextEdit::singleline(saved_val_str)
                                            .font(egui::TextStyle::Monospace)
                                            .id_source(format!("global_saved_{}", key))
                                    );

                                    // Validate JSON on change
                                    if saved_response.changed() {
                                        if serde_json::from_str::<serde_json::Value>(saved_val_str.trim()).is_err() {
                                            saved_response.on_hover_text_at_pointer("⚠ Invalid JSON");
                                        }
                                    }

                                    // Current value column (read-only, shows runtime value)
                                    let current_val_str = if let Some(current_val) = current_values.get(key) {
                                        serde_json::to_string_pretty(current_val)
                                            .unwrap_or_else(|_| "null".to_string())
                                    } else {
                                        saved_val_str.clone()
                                    };

                                    // Highlight if current differs from saved
                                    let differs = current_val_str.trim() != saved_val_str.trim();
                                    let current_text = if differs {
                                        egui::RichText::new(current_val_str.lines().next().unwrap_or(""))
                                            .monospace()
                                            .color(egui::Color32::from_rgb(100, 200, 255))
                                    } else {
                                        egui::RichText::new(current_val_str.lines().next().unwrap_or(""))
                                            .monospace()
                                            .color(egui::Color32::GRAY)
                                    };

                                    ui.label(current_text)
                                        .on_hover_text(if differs {
                                            "Runtime value differs from saved"
                                        } else {
                                            "Runtime value matches saved"
                                        });

                                    // Actions column
                                    if ui.button("✖").on_hover_text("Remove variable").clicked() {
                                        to_remove = Some(key.clone());
                                    }

                                    ui.end_row();
                                }
                            });

                if let Some(k) = to_remove {
                    self.interaction.temp_transformer_globals_edits.remove(&k);
                }

                ui.add_space(8.0);
                ui.horizontal(|ui| {
                    if ui.button("💾 Save Changes").clicked() {
                        // Attempt to parse all values and persist
                        let mut new_map: serde_json::Map<String, serde_json::Value> = serde_json::Map::new();
                        let mut parse_error: Option<String> = None;
                        for (k, vstr) in &self.interaction.temp_transformer_globals_edits {
                            match serde_json::from_str::<serde_json::Value>(vstr.trim()) {
                                Ok(v) => {
                                    new_map.insert(k.clone(), v);
                                }
                                Err(err) => {
                                    parse_error = Some(format!("Key '{}': {}", k, err));
                                    break;
                                }
                            }
                        }

                        if let Some(err) = parse_error {
                            ui.colored_label(egui::Color32::RED, format!("Cannot save: {}", err));
                        } else {
                            // Persist via property update
                            self.interaction.temp_new_global_key.clear();
                            self.interaction.temp_new_global_value.clear();
                            self.update_transformer_property(node.id, "globals");
                        }
                    }

                    if ui.button("↻ Reload from Node").clicked() {
                        self.interaction.temp_transformer_globals_edits.clear();
                        if let NodeType::Transformer { initial_globals, .. } = &node.node_type {
                            for (k, v) in initial_globals.iter() {
                                let s = serde_json::to_string_pretty(v).unwrap_or_else(|_| "null".to_string());
                                self.interaction
                                    .temp_transformer_globals_edits
                                    .insert(k.clone(), s);
                            }
                        }
                    }
                });
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
                                script: "// Transform the input message with optional routing via __targets\nfunction transform(input) {\n    // To target specific outputs by node name, include __targets as an array.\n    // For example, send only to node named \"NextNode\":\n    // return { value: input.value, __targets: [\"NextNode\"] };\n    // If __targets is omitted or null, the message is broadcast to all outputs.\n    return input;\n}".to_string(),
                                selected_outputs: None,
                                globals: Default::default(),
                                initial_globals: Default::default(),
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

        // Handle other interactions (selection, context menu, marquee start/update)
        // Run this before node dragging so marquee gets priority over node drag
        self.handle_canvas_interactions(ui, &response);

        // Handle node dragging with left mouse button (respects marquee priority)
        self.handle_node_dragging(ui, &response);

        // Render all flowchart elements (including marquee rectangle if active)
        let canvas_rect = response.rect;
        self.render_flowchart_elements(&painter, canvas_rect);

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
    fn handle_canvas_interactions(&mut self, ui: &mut egui::Ui, response: &egui::Response) {
        // Marquee selection handling: primary down on empty space -> start marquee
        if ui.input(|i| i.pointer.primary_down())
            && !self.interaction.is_panning
            && self.interaction.dragging_node.is_none()
            && self.interaction.drawing_connection_from.is_none()
            && self.interaction.pending_shift_connection_from.is_none()
        {
            if let Some(pos) = response.interact_pointer_pos() {
                // If a marquee is already active, always update its end point regardless of what's under the cursor
                if self.interaction.marquee_start.is_some() {
                    self.interaction.marquee_end = Some(pos);
                } else {
                    let world_pos = self.screen_to_world(pos);
                    // Only start marquee if the press began on empty space (no node/connection)
                    let over_node = self.find_node_at_position(world_pos).is_some();
                    let over_conn = self.find_connection_at_position(world_pos).is_some();
                    if !over_node && !over_conn {
                        self.interaction.marquee_start = Some(pos);
                        self.interaction.marquee_end = Some(pos);
                        // Determine if this marquee should be additive (Shift-held at start)
                        self.interaction.marquee_additive = ui.input(|i| i.modifiers.shift);
                        // Clear existing selection while selecting a new region unless in additive mode
                        if !self.interaction.marquee_additive {
                            self.interaction.selected_nodes.clear();
                            self.interaction.selected_node = None;
                            self.interaction.selected_group = None;
                            self.interaction.selected_connection = None;
                        }
                    }
                }
            }
        } else {
            // On release: finalize marquee selection if active
            if self.interaction.marquee_start.is_some() && self.interaction.marquee_end.is_some() {
                let start_screen = self.interaction.marquee_start.unwrap();
                let end_screen = self.interaction.marquee_end.unwrap();
                let rect_screen = egui::Rect::from_two_pos(start_screen, end_screen);

                // Convert to world rect corners for hit testing by node centers
                let min_world = self.screen_to_world(rect_screen.min);
                let max_world = self.screen_to_world(rect_screen.max);
                let world_rect = egui::Rect::from_min_max(min_world, max_world);

                if !self.interaction.marquee_additive {
                    self.interaction.selected_nodes.clear();
                }
                for (id, node) in &self.flowchart.nodes {
                    let center = egui::pos2(node.position.0, node.position.1);
                    if world_rect.contains(center) {
                        if !self.interaction.selected_nodes.contains(id) {
                            self.interaction.selected_nodes.push(*id);
                        }
                    }
                }
                // Sync single selection convenience field
                if self.interaction.selected_nodes.len() == 1 {
                    self.interaction.selected_node = Some(self.interaction.selected_nodes[0]);
                } else {
                    self.interaction.selected_node = None;
                }

                // Clear marquee visuals
                self.interaction.marquee_start = None;
                self.interaction.marquee_end = None;
                self.interaction.marquee_additive = false;
                self.clear_temp_editing_values();
            }
        }

        // Left-click for selection (only if not dragging or panning) - single click behaviors
        if response.clicked()
            && !self.interaction.is_panning
            && self.interaction.dragging_node.is_none()
        {
            // If a pending shift-click on a node is being handled by node-dragging logic (for connection or additive select),
            // skip default click handling here to avoid double-processing.
            if self.interaction.pending_shift_connection_from.is_some() {
                return;
            }
            if let Some(pos) = response.interact_pointer_pos() {
                let world_pos = self.screen_to_world(pos);

                // First try to select a node
                if let Some(node_id) = self.find_node_at_position(world_pos) {
                    let shift = ui.input(|i| i.modifiers.shift);
                    if shift {
                        // Toggle selection membership on Shift-click
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
                        // Convenience single-selection sync
                        match self.interaction.selected_nodes.as_slice() {
                            [only] => self.interaction.selected_node = Some(*only),
                            _ => self.interaction.selected_node = None,
                        }
                    } else {
                        // Normal single selection
                        self.interaction.selected_node = Some(node_id);
                        self.interaction.selected_nodes.clear();
                        self.interaction.selected_nodes.push(node_id);
                    }
                    self.interaction.selected_group = None;
                    self.interaction.selected_connection = None;
                    self.interaction.editing_node_name = None;
                    // Clear temp producer values to reload from selected node
                    self.clear_temp_editing_values();
                } else {
                    // Try to select a connection
                    if let Some(conn_idx) = self.find_connection_at_position(world_pos) {
                        self.interaction.selected_connection = Some(conn_idx);
                        self.interaction.selected_node = None;
                        self.interaction.selected_nodes.clear();
                        self.interaction.selected_group = None;
                        self.interaction.editing_node_name = None;
                        self.clear_temp_editing_values();
                    } else {
                        // Try to select a group when clicking on empty space inside its rect
                        if let Some(gid) = self.find_group_at_position(world_pos) {
                            self.interaction.selected_group = Some(gid);
                            self.interaction.selected_node = None;
                            self.interaction.selected_nodes.clear();
                            self.interaction.selected_connection = None;
                            self.interaction.editing_node_name = None;
                            self.clear_temp_editing_values();
                        } else {
                            // Clear selection if clicking on empty space
                            self.interaction.selected_group = None;
                            self.interaction.selected_node = None;
                            self.interaction.selected_nodes.clear();
                            self.interaction.selected_connection = None;
                            self.interaction.editing_node_name = None;
                            self.clear_temp_editing_values();
                        }
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
    /// Note: This does NOT clear transformer globals - those are managed separately
    /// based on node selection to preserve unsaved edits when reselecting the same node.
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
                self.undo_history.push_redo(redo_action);
                self.file.has_unsaved_changes = true;

                // Clear selection and temp values to refresh UI
                self.interaction.selected_node = None;
                self.interaction.selected_nodes.clear();
                self.interaction.selected_connection = None;
                self.interaction.selected_group = None;
                self.clear_temp_editing_values();
            }
        }
    }

    /// Performs a redo operation.
    fn perform_redo(&mut self) {
        if let Some(action) = self.undo_history.pop_redo() {
            if let Some(undo_action) = self.flowchart.apply_undo(&action) {
                self.undo_history.push_undo(undo_action);
                // Don't call push_action here as it would clear the redo stack
                self.file.has_unsaved_changes = true;

                // Clear selection and temp values to refresh UI
                self.interaction.selected_node = None;
                self.interaction.selected_nodes.clear();
                self.interaction.selected_connection = None;
                self.interaction.selected_group = None;
                self.clear_temp_editing_values();
            }
        }
    }

    /// Apply the currently selected auto-arrangement mode to nodes.
    /// If a group is selected, only that group's nodes are rearranged.
    /// Otherwise, if any nodes are selected, only those nodes are rearranged.
    fn apply_auto_arrangement(&mut self) {
        match self.auto_arrange_mode {
            crate::ui::state::AutoArrangeMode::ForceDirected => self.auto_layout_graph(),
            crate::ui::state::AutoArrangeMode::Grid => self.grid_layout_selected_or_all(),
            crate::ui::state::AutoArrangeMode::Line => self.line_layout_selected_or_all(),
        }
    }

    /// Determine which node ids should be targeted by layout operations.
    /// Priority: selected group > multi-selection > all nodes.
    fn target_ids_for_layout(&self) -> Vec<NodeId> {
        if let Some(gid) = self.interaction.selected_group {
            if let Some(g) = self.flowchart.groups.get(&gid) {
                let mut v = g.members.clone();
                v.retain(|id| self.flowchart.nodes.contains_key(id));
                return v;
            }
        }
        if self.interaction.selected_nodes.len() > 1 {
            return self.interaction.selected_nodes.clone();
        }
        self.flowchart.nodes.keys().copied().collect()
    }

    /// Compute an adjacency-aware ordering of the given nodes so that connected
    /// nodes are placed next to each other when laid out linearly.
    ///
    /// Strategy:
    /// - Work per connected component (undirected) for determinism.
    /// - Within a component, try a Kahn topological order on the induced directed subgraph.
    /// - If cycles remain, append the remaining nodes using a BFS over the undirected graph.
    /// - Tie-breakers are resolved by NodeId string to keep order stable across runs.
    fn adjacency_aware_order(&self, ids: &[NodeId]) -> Vec<NodeId> {
        use std::collections::{HashMap, HashSet, VecDeque};

        let id_set: HashSet<NodeId> = ids.iter().copied().collect();
        if id_set.is_empty() {
            return Vec::new();
        }

        // Build directed adjacency and in-degree within the target set
        let mut out: HashMap<NodeId, Vec<NodeId>> = HashMap::new();
        let mut in_deg: HashMap<NodeId, usize> = HashMap::new();
        for id in &id_set {
            out.insert(*id, Vec::new());
            in_deg.insert(*id, 0);
        }
        for c in &self.flowchart.connections {
            if id_set.contains(&c.from) && id_set.contains(&c.to) {
                out.get_mut(&c.from).unwrap().push(c.to);
                *in_deg.get_mut(&c.to).unwrap() += 1;
            }
        }

        // Undirected adjacency for components and BFS fallback
        let mut undirected: HashMap<NodeId, Vec<NodeId>> = HashMap::new();
        for id in &id_set {
            undirected.insert(*id, Vec::new());
        }
        for c in &self.flowchart.connections {
            if id_set.contains(&c.from) && id_set.contains(&c.to) {
                undirected.get_mut(&c.from).unwrap().push(c.to);
                undirected.get_mut(&c.to).unwrap().push(c.from);
            }
        }

        // Find connected components (undirected)
        let mut seen: HashSet<NodeId> = HashSet::new();
        let mut components: Vec<Vec<NodeId>> = Vec::new();
        let mut ids_sorted: Vec<NodeId> = id_set.iter().copied().collect();
        ids_sorted.sort_by_key(|id| id.to_string());
        for start in ids_sorted {
            if seen.contains(&start) {
                continue;
            }
            let mut comp = Vec::new();
            let mut q = VecDeque::new();
            seen.insert(start);
            q.push_back(start);
            while let Some(n) = q.pop_front() {
                comp.push(n);
                if let Some(neis) = undirected.get(&n) {
                    for &m in neis {
                        if !seen.contains(&m) {
                            seen.insert(m);
                            q.push_back(m);
                        }
                    }
                }
            }
            comp.sort_by_key(|id| id.to_string());
            components.push(comp);
        }

        // Order components by smallest id for determinism
        components.sort_by_key(|comp| comp.first().map(|id| id.to_string()).unwrap_or_default());

        let mut result = Vec::with_capacity(id_set.len());
        for comp in components {
            // Kahn's algorithm on the component
            let mut local_in: HashMap<NodeId, isize> = HashMap::new();
            for &n in &comp {
                local_in.insert(n, *in_deg.get(&n).unwrap_or(&0) as isize);
            }
            let mut zero: Vec<NodeId> = comp
                .iter()
                .copied()
                .filter(|n| local_in.get(n).copied().unwrap_or(0) == 0)
                .collect();
            zero.sort_by_key(|id| id.to_string());

            while let Some(n) = zero.pop() {
                if result.contains(&n) { // can happen across comps only if overlaps, but safe
                    continue;
                }
                result.push(n);
                if let Some(neis) = out.get(&n) {
                    for &m in neis {
                        if !local_in.contains_key(&m) { continue; }
                        let entry = local_in.get_mut(&m).unwrap();
                        *entry -= 1;
                        if *entry == 0 {
                            // Insert maintaining sort on id string; push then sort as list is small
                            zero.push(m);
                            zero.sort_by_key(|id| id.to_string());
                        }
                    }
                }
            }

            // If there are nodes not yet placed (cycles), do an undirected BFS from the smallest remaining
            let comp_set: HashSet<NodeId> = comp.iter().copied().collect();
            let mut remaining: Vec<NodeId> = comp
                .iter()
                .copied()
                .filter(|n| !result.contains(n))
                .collect();
            remaining.sort_by_key(|id| id.to_string());
            while let Some(seed) = remaining.first().copied() {
                // BFS
                let mut q = VecDeque::new();
                q.push_back(seed);
                let mut local_seen: HashSet<NodeId> = HashSet::new();
                local_seen.insert(seed);
                while let Some(n) = q.pop_front() {
                    if !result.contains(&n) {
                        result.push(n);
                    }
                    if let Some(neis) = undirected.get(&n) {
                        // deterministic neighbor order
                        let mut sorted = neis
                            .iter()
                            .copied()
                            .filter(|m| comp_set.contains(m))
                            .collect::<Vec<_>>();
                        sorted.sort_by_key(|id| id.to_string());
                        for m in sorted {
                            if !local_seen.contains(&m) && !result.contains(&m) {
                                local_seen.insert(m);
                                q.push_back(m);
                            }
                        }
                    }
                }
                remaining = comp
                    .iter()
                    .copied()
                    .filter(|n| !result.contains(n))
                    .collect();
                remaining.sort_by_key(|id| id.to_string());
            }
        }

        // Finally, filter to original ids order domain (result already from ids) and return
        result
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

        // Determine target nodes: selected group > multi-selection > all
        let target_ids: Vec<NodeId> = self.target_ids_for_layout();
        if target_ids.is_empty() {
            return;
        }

        // Store original positions for undo (only for affected nodes)
        let old_positions: Vec<(NodeId, (f32, f32))> = target_ids
            .iter()
            .filter_map(|id| self.flowchart.nodes.get(id).map(|n| (*id, n.position)))
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
        let min_distance: f32 =
            (NODE_WIDTH * NODE_WIDTH + NODE_HEIGHT * NODE_HEIGHT).sqrt() + SPACING_BUFFER * 2.0;

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
            for node_id in &target_ids {
                forces.insert(*node_id, (0.0, 0.0));
            }

            // Repulsion forces between all pairs of nodes
            let node_ids: Vec<NodeId> = target_ids.clone();
            for i in 0..node_ids.len() {
                for j in (i + 1)..node_ids.len() {
                    let id1 = node_ids[i];
                    let id2 = node_ids[j];

                    if let (Some(node1), Some(node2)) = (
                        self.flowchart.nodes.get(&id1),
                        self.flowchart.nodes.get(&id2),
                    ) {
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

            // Attraction forces along connections within the target set
            for connection in &self.flowchart.connections {
                if !target_ids.contains(&connection.from) || !target_ids.contains(&connection.to) {
                    continue;
                }
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

        // Center the affected nodes around their centroid to avoid shifting unrelated nodes
        if !target_ids.is_empty() {
            // Calculate center of mass
            let mut center_x = 0.0;
            let mut center_y = 0.0;
            let node_count = target_ids.len() as f32;

            for node_id in &target_ids {
                if let Some(node) = self.flowchart.nodes.get(node_id) {
                    center_x += node.position.0;
                    center_y += node.position.1;
                }
            }

            center_x /= node_count;
            center_y /= node_count;

            // Shift affected nodes to center around their previous centroid
            for node_id in &target_ids {
                if let Some(node) = self.flowchart.nodes.get_mut(node_id) {
                    node.position.0 -= center_x;
                    node.position.1 -= center_y;
                }
            }
        }

        // Collect new positions after layout
        let new_positions: Vec<(NodeId, (f32, f32))> = target_ids
            .iter()
            .filter_map(|id| self.flowchart.nodes.get(id).map(|n| (*id, n.position)))
            .collect();

        // Record undo action for the layout operation
        self.undo_history
            .push_action(UndoAction::MultipleNodesMoved {
                old_positions,
                new_positions,
            });

        self.file.has_unsaved_changes = true;
    }

    /// Arrange nodes in a grid. Applies to selected nodes if any, otherwise all.
    ///
    /// The grid is anchored around the pre-layout central position of the targeted
    /// nodes to avoid drift when applying the layout multiple times.
    fn grid_layout_selected_or_all(&mut self) {
        if self.flowchart.nodes.is_empty() {
            return;
        }
        let base_ids: Vec<NodeId> = self.target_ids_for_layout();
        let ids = self.adjacency_aware_order(&base_ids);
        if ids.is_empty() {
            return;
        }

        // Constants
        const NODE_WIDTH: f32 = 100.0;
        const NODE_HEIGHT: f32 = 70.0;
        const H_SPACING: f32 = 40.0;
        const V_SPACING: f32 = 40.0;

        // Compute pre-layout center using the centroid (arithmetic mean) of targeted nodes.
        // IMPORTANT: The placement below has an asymmetric mass when the last row is partial
        // (e.g., 3 nodes in a 2x2 with one missing). If we anchor by bounding-box center
        // but post-correct by centroid, repeated application causes drift. Using the centroid
        // both for anchoring and for post-correction makes the operation idempotent.
        let mut cx = 0.0;
        let mut cy = 0.0;
        for id in &ids {
            if let Some(n) = self.flowchart.nodes.get(id) {
                cx += n.position.0;
                cy += n.position.1;
            }
        }
        let denom = ids.len() as f32;
        let cx = if denom > 0.0 { cx / denom } else { 0.0 };
        let cy = if denom > 0.0 { cy / denom } else { 0.0 };

        // grid size
        let n = ids.len();
        let cols = (n as f32).sqrt().ceil() as usize;
        let cols = cols.max(1);
        let rows = ((n + cols - 1) / cols).max(1);

        // grid physical dimensions
        let cell_w = NODE_WIDTH + H_SPACING;
        let cell_h = NODE_HEIGHT + V_SPACING;
        let total_w = (cols as f32 - 1.0) * cell_w;
        let total_h = (rows as f32 - 1.0) * cell_h;
        let origin_x = cx - total_w / 2.0;
        let origin_y = cy - total_h / 2.0;

        let old_positions: Vec<(NodeId, (f32, f32))> = ids
            .iter()
            .filter_map(|id| self.flowchart.nodes.get(id).map(|n| (*id, n.position)))
            .collect();

        for (idx, id) in ids.iter().enumerate() {
            let r = idx / cols;
            let c = idx % cols;
            let c = if r % 2 == 1 { cols - 1 - c } else { c }; // snake order to keep adjacents close between rows
            if let Some(node) = self.flowchart.nodes.get_mut(id) {
                node.position.0 = origin_x + c as f32 * cell_w;
                node.position.1 = origin_y + r as f32 * cell_h;
            }
        }

        // After placement, compute the actual centroid and adjust by any tiny delta
        // to align precisely with the pre-layout center. This guards against tiny
        // floating point accumulation or asymmetries, ensuring idempotency.
        let mut post_cx = 0.0;
        let mut post_cy = 0.0;
        for id in &ids {
            if let Some(n) = self.flowchart.nodes.get(id) {
                post_cx += n.position.0;
                post_cy += n.position.1;
            }
        }
        let count = ids.len() as f32;
        if count > 0.0 {
            post_cx /= count;
            post_cy /= count;
            let dx = cx - post_cx;
            let dy = cy - post_cy;
            if dx.abs() > f32::EPSILON || dy.abs() > f32::EPSILON {
                for id in &ids {
                    if let Some(n) = self.flowchart.nodes.get_mut(id) {
                        n.position.0 += dx;
                        n.position.1 += dy;
                    }
                }
            }
        }

        let new_positions: Vec<(NodeId, (f32, f32))> = ids
            .iter()
            .filter_map(|id| self.flowchart.nodes.get(id).map(|n| (*id, n.position)))
            .collect();

        self.undo_history
            .push_action(UndoAction::MultipleNodesMoved { old_positions, new_positions });
        self.file.has_unsaved_changes = true;
    }

    /// Arrange nodes in a horizontal line. Applies to selected nodes if any, otherwise all.
    fn line_layout_selected_or_all(&mut self) {
        if self.flowchart.nodes.is_empty() {
            return;
        }
        let ids_base: Vec<NodeId> = self.target_ids_for_layout();
        let ids = self.adjacency_aware_order(&ids_base);
        if ids.is_empty() {
            return;
        }

        // Constants
        const NODE_WIDTH: f32 = 100.0;
        const H_SPACING: f32 = 40.0;
        let step = NODE_WIDTH + H_SPACING;

        // centroid
        let mut cx = 0.0;
        let mut cy = 0.0;
        for id in &ids {
            if let Some(n) = self.flowchart.nodes.get(id) {
                cx += n.position.0;
                cy += n.position.1;
            }
        }
        cx /= ids.len() as f32;
        cy /= ids.len() as f32;

        let total_w = step * (ids.len().saturating_sub(1)) as f32;
        let start_x = cx - total_w / 2.0;

        let old_positions: Vec<(NodeId, (f32, f32))> = ids
            .iter()
            .filter_map(|id| self.flowchart.nodes.get(id).map(|n| (*id, n.position)))
            .collect();

        for (i, id) in ids.iter().enumerate() {
            if let Some(node) = self.flowchart.nodes.get_mut(id) {
                node.position.0 = start_x + i as f32 * step;
                node.position.1 = cy;
            }
        }

        let new_positions: Vec<(NodeId, (f32, f32))> = ids
            .iter()
            .filter_map(|id| self.flowchart.nodes.get(id).map(|n| (*id, n.position)))
            .collect();

        self.undo_history
            .push_action(UndoAction::MultipleNodesMoved { old_positions, new_positions });
        self.file.has_unsaved_changes = true;
    }
}

// Test module for headless egui-driven UI unit tests.
// Placed inside the `ui` module so tests can access private methods like
// `draw_canvas` and `handle_undo_redo_keys` without exposing them publicly.
#[cfg(test)]
mod tests;
