use flowchart_tool;

fn main() -> Result<(), eframe::Error> {
    // Set up logging for development
    env_logger::init();

    // Run the flowchart application
    flowchart_tool::run_app()
}
