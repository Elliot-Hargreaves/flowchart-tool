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

pub mod script_engine;
pub mod simulation;
pub mod types;
pub mod ui;
pub mod constants;

// Re-export public types and functions
pub use simulation::*;
pub use types::*;

use crate::ui::FlowchartApp;
#[cfg(target_arch = "wasm32")] // When compiling for web
use {
    eframe::wasm_bindgen::{self, prelude::*, JsCast},
    web_sys::HtmlCanvasElement,
};

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
    let window = web_sys::window().unwrap();
    let document = window.document().unwrap();
    let canvas = document.get_element_by_id(canvas_id).unwrap();
    let canvas: HtmlCanvasElement = canvas.dyn_into::<HtmlCanvasElement>().unwrap();

    // Ensure a favicon is set for the web build
    if let Some(head) = document.head() {
        let link = document.create_element("link").unwrap();
        link.set_attribute("rel", "icon").ok();
        link.set_attribute("type", "image/svg+xml").ok();
        // Simple inlined SVG circle icon
        let svg = r#"<svg xmlns='http://www.w3.org/2000/svg' viewBox='0 0 64 64'>
<circle cx='32' cy='32' r='26' fill='#4287F5' stroke='#2a5db0' stroke-width='4'/>
<path d='M20 28h24v8H20z' fill='white'/>
</svg>"#;
        let data_url = format!(
            "data:image/svg+xml;utf8,{}",
            js_sys::encode_uri_component(svg)
        );
        link.set_attribute("href", &data_url).ok();
        head.append_child(&link).ok();
    }

    let options = eframe::WebOptions::default();
    eframe::WebRunner::new()
        .start(
            canvas,
            options,
            Box::new(|cc| {
                if let Some(storage) = cc.storage {
                    if let Some(json) = storage.get_string("app_state") {
                        match FlowchartApp::from_json(&json) {
                            Ok(mut app) => {
                                app.reset_non_ui_fields();
                                eprintln!("Loaded app_state from storage (web)");
                                return Ok(Box::new(app));
                            }
                            Err(err) => {
                                eprintln!("Failed to parse app_state (web): {err}");
                            }
                        }
                    } else {
                        eprintln!("No app_state found in storage (web)");
                    }
                } else {
                    eprintln!("No storage available (web)");
                }
                Ok(Box::new(FlowchartApp::default()))
            }),
        )
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
fn generate_app_icon() -> egui::IconData {
    // Generate a simple 64x64 RGBA icon (blue circle on transparent background)
    let size: usize = 64;
    let mut rgba: Vec<u8> = vec![0; size * size * 4];
    let center = (size as f32 - 1.0) / 2.0;
    let radius = 24.0;
    for y in 0..size {
        for x in 0..size {
            let dx = x as f32 - center;
            let dy = y as f32 - center;
            let dist = (dx * dx + dy * dy).sqrt();
            let idx = (y * size + x) * 4;
            if dist <= radius {
                let edge = (radius - dist).clamp(0.0, 1.0);
                rgba[idx] = 66; // R
                rgba[idx + 1] = 135; // G
                rgba[idx + 2] = 245; // B
                rgba[idx + 3] = (200.0 * edge + 55.0) as u8; // A
            } else {
                rgba[idx + 3] = 0; // transparent
            }
        }
    }
    egui::IconData {
        rgba,
        width: size as u32,
        height: size as u32,
    }
}

/// Entrypoint for the desktop app
#[cfg(not(target_arch = "wasm32"))]
pub fn run_app() -> Result<(), eframe::Error> {
    let icon = generate_app_icon();
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_app_id("flowchart_tool")
            .with_icon(icon)
            // Default window size tuned for 1080p displays (leaves room for taskbar)
            .with_inner_size(egui::vec2(1720.0, 980.0)),
        ..Default::default()
    };
    eframe::run_native(
        "Flowchart Tool",
        options,
        Box::new(|cc| {
            if let Some(storage) = cc.storage {
                if let Some(json) = storage.get_string("app_state") {
                    match FlowchartApp::from_json(&json) {
                        Ok(mut app) => {
                            app.reset_non_ui_fields();
                            eprintln!("Loaded app_state from storage (native)");
                            return Ok(Box::new(app));
                        }
                        Err(err) => {
                            eprintln!("Failed to parse app_state (native): {err}");
                        }
                    }
                } else {
                    eprintln!("No app_state found in storage (native)");
                }
            } else {
                eprintln!("No storage available (native)");
            }
            Ok(Box::new(FlowchartApp::default()))
        }),
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
        assert!(matches!(
            flowchart.simulation_state,
            SimulationState::Stopped
        ));
    }

    #[test]
    fn test_message_creation() {
        let message = Message::new(serde_json::json!({"test": "data"}));
        assert_eq!(message.data["test"], "data");
    }
}
