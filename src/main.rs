#[tokio::main]
async fn main() -> Result<(), eframe::Error> {
    // Set up logging for development
    env_logger::init();

    #[cfg(not(target_arch = "wasm32"))]
    {
        // Run the flowchart application
        return flowchart_tool::run_app();
    }
    #[cfg(target_arch = "wasm32")]
    {
        Ok(())
    }
}
