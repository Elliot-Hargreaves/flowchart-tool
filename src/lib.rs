//! # Flowchart Tool
//! 
//! A visual flowchart editor and simulator for modeling processes with interactive nodes
//! connected by directional arrows. Supports three types of nodes:
//! - **Producers**: Generate messages at specified rates
//! - **Consumers**: Consume and destroy messages
//! - **Transformers**: Execute JavaScript scripts to transform messages
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
//! Flowchart Tool - A visual flowchart editor and simulator
//!
//! This application allows users to create, edit, and simulate flowcharts
//! with different node types (Producer, Consumer, Transformer) and message passing.

pub mod types;
pub mod simulation;
pub mod script_engine;
pub mod ui;


// Re-export public types and functions
pub use types::*;
pub use simulation::*;

#[cfg(target_arch = "wasm32")] // When compiling for web
use {
    eframe::wasm_bindgen::{self, prelude::*, JsCast},
    web_sys::HtmlCanvasElement,
};
use crate::ui::FlowchartApp;

/// Runs the flowchart application with default settings.
/// 
/// wasm function initializes the egui application window and starts the main event loop.
/// 
/// # Returns
/// 
/// Returns `Ok(())` if the application runs successfully, or an `eframe::Error` if
/// initialization fails.
/// 
/// # Example
/// 
/// ```no_run,ignore-wasm32
/// use flowchart_tool::run_app;
/// 
/// fn main() -> Result<(), eframe::Error> {
///     run_app()
/// }
/// ```

#[cfg(target_arch = "wasm32")]
#[wasm_bindgen]
pub async fn run_app(canvas_id: &str) -> Result<(), eframe::wasm_bindgen::JsValue> {
    let document = web_sys::window().unwrap().document().unwrap();
    let canvas = document.get_element_by_id(canvas_id).unwrap();
    let canvas: HtmlCanvasElement = canvas.dyn_into::<HtmlCanvasElement>().unwrap();

    let options = eframe::WebOptions::default();
    eframe::WebRunner::new()
        .start(canvas, options, Box::new(|_cc| Ok(Box::new(FlowchartApp::default()))))
        .await?;
    Ok(())
}

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
#[cfg(not(target_arch = "wasm32"))]
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
        assert_eq!(message.data["test"], "data");
    }
}