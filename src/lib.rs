//! # Flowchart Tool
//! 
//! A visual flowchart editor and simulator for modeling processes with interactive nodes
//! connected by directional arrows. Supports three types of nodes:
//! - **Producers**: Generate messages at specified rates
//! - **Consumers**: Consume and destroy messages
//! - **Transformers**: Execute Lua scripts to transform messages
//! 
//! ## Features
//! - Interactive node creation, selection, and repositioning
//! - Real-time simulation stepping
//! - Canvas panning and zooming
//! - Node property editing
//! - Context menu for node creation
//! - Message flow visualization

#![warn(missing_docs)]
#![deny(unsafe_code)]

mod types;
mod simulation;
mod ui;

// Re-export public types and functions
pub use types::*;
pub use simulation::*;
use ui::FlowchartApp;

/// Runs the flowchart application with default settings.
/// 
/// This function initializes the egui application window and starts the main event loop.
/// 
/// # Returns
/// 
/// Returns `Ok(())` if the application runs successfully, or an `eframe::Error` if
/// initialization fails.
/// 
/// # Example
/// 
/// ```no_run
/// use flowchart_tool::run_app;
/// 
/// fn main() -> Result<(), eframe::Error> {
///     run_app()
/// }
/// ```
pub fn run_app() -> Result<(), eframe::Error> {
    let options = eframe::NativeOptions::default();
    eframe::run_native(
        "Flowchart Tool",
        options,
        Box::new(|_cc| Ok(Box::new(FlowchartApp::default()))),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_flowchart_default() {
        let flowchart = Flowchart::default();
        assert!(flowchart.nodes.is_empty());
        assert!(flowchart.connections.is_empty());
        assert!(matches!(flowchart.simulation_state, SimulationState::Stopped));
    }

    #[test]
    fn test_message_creation() {
        let message = Message::new(serde_json::json!({"test": "data"}));
        assert_eq!(message.position_along_edge, 0.0);
        assert_eq!(message.data["test"], "data");
    }
}