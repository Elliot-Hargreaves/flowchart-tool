# Flowchart Tool

A cross‑platform desktop application written in Rust (egui/eframe) for creating, editing, and managing flowcharts. It provides an interactive canvas with node creation, selection, dragging, multi‑select, grid snapping, and connection wiring between nodes with validation rules (e.g., Consumers cannot send; Producers cannot receive). Undo/redo, file operations, and JSON serialization are supported.

A web version can be found at [flowchart.coffy.dev](https://flowchart.coffy.dev/).

## Features
- Interactive canvas using egui/eframe
- Create, move, and multi‑select nodes
- Grid snapping (hold Shift while dragging)
- Connect nodes with rules enforced between Producer/Consumer/Processor types
- Undo/redo history
- Open/Save flowcharts to JSON
- Cross‑platform## Getting St builds (Windows, macOS, Linux)

## Getting Started

### Prerequisites
- Rust (stable). Install via https://rustup.rs
- A recent OS (Windows, macOS, or Linux). GitHub Actions examples below cover all three.

> Note for Linux desktop environments: most systems work out of the box with egui/winit. If you run into windowing issues, ensure standard X11/Wayland libraries are installed.

### Build and Run (local)

Debug build and run:

```
cargo run
```

Release build:

```
cargo build --release
```

The produced binary will be at:
- Linux/macOS: `target/release/flowchart_tool`
- Windows: `target/release/flowchart_tool.exe`

### Usage Tips
- Drag nodes with the mouse. Hold Shift while dragging to snap to the grid.
- Multi‑select by selecting multiple nodes (drag or use your app UI’s selection features), then drag to move them together.
- Create connections by starting from one node and releasing over another. Invalid connections (e.g., Consumer ➜ anything or anything ➜ Producer) are prevented.

## Project Layout
- `src/main.rs`: Desktop entry point (Tokio runtime + eframe app launcher)
- `src/lib.rs`: Main app module and app wiring
- `src/ui/*.rs`: UI modules (canvas rendering, file ops, undo, etc.)
- `Cargo.toml`: Rust package configuration

## Releasing via GitHub Actions
This repository includes a GitHub Actions workflow that:
- Builds binaries for Windows, macOS, and Linux
- Uploads build artifacts for each platform
- On tag pushes (e.g., `v0.1.0`), creates a GitHub Release and attaches the built binaries

### How to trigger a release
1. Commit and push your changes.
2. Create a version tag and push it, for example:
   ```bash
   git tag v0.1.0
   git push origin v0.1.0
   ```
3. The workflow will build for all three platforms and publish a GitHub Release with the artifacts.

Artifacts are packaged as:
- Linux/macOS: `.tar.gz`
- Windows: `.zip`

## AI assistance and authorship
I started this project because I wanted to build a simple flowchart tool, and I used it as a chance to experiment with AI‑assisted coding. A lot of the code was written with help from AI tools(primarily Anthropic Claude Sonnet 4 and 4.5, and JetBrains Junie). This is a fun side project; not everything has been carefully reviewed, and the code quality may be questionable in places.

## License
This project is dual‑licensed under either of the following, at your option:
- MIT License (see LICENSE-MIT)
- Apache License, Version 2.0 (see LICENSE-APACHE)

Unless you explicitly state otherwise, any contribution intentionally submitted for inclusion in this project by you, as defined in the Apache‑2.0 license, shall be dual‑licensed as above, without any additional terms or conditions.
