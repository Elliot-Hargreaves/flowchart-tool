//! Export utilities: render the current flowchart to SVG and PNG.
//!
//! Notes:
//! - SVG export is supported on all targets (native + wasm).
//! - PNG export is supported on native targets only (wasm skipped).

use crate::constants;
use crate::types::*;
#[cfg(not(target_arch = "wasm32"))]
use std::sync::Arc;

use super::state::FlowchartApp;

impl FlowchartApp {
    /// Export the currently visible graph to an SVG file/string and trigger a save dialog.
    pub fn export_svg(&mut self) {
        let (svg, _w, _h) = self.build_svg_string();

        #[cfg(target_arch = "wasm32")]
        {
            // In the browser, trigger a download of the SVG text.
            if let Err(e) = Self::trigger_download("flowchart.svg", &svg) {
                eprintln!("Failed to start SVG download: {}", e);
            }
        }

        #[cfg(not(target_arch = "wasm32"))]
        {
            // Native: open a save dialog and write the file asynchronously to avoid UI stalls
            tokio::spawn(async move {
                if let Some(handle) = rfd::AsyncFileDialog::new()
                    .add_filter("SVG", &["svg"]) 
                    .set_file_name("flowchart.svg")
                    .save_file()
                    .await
                {
                    let path = handle.path();
                    if let Err(e) = std::fs::write(path, svg.as_bytes()) {
                        eprintln!("Failed to save SVG: {}", e);
                    }
                }
            });
        }
    }

    /// Export the current graph to PNG (native builds only). On wasm, this does nothing.
    pub fn export_png(&mut self) {
        #[cfg(not(target_arch = "wasm32"))]
        {
            let (svg, width, height) = self.build_svg_string();

            use usvg;
            use tiny_skia::Pixmap;

            // Parse SVG
            let mut opt = usvg::Options::default();
            // Prepare font database so text renders with system fonts when possible
            let mut db = fontdb::Database::new();
            db.load_system_fonts();
            opt.fontdb = Arc::new(db);

            let tree = match usvg::Tree::from_data(svg.as_bytes(), &opt) {
                Ok(t) => t,
                Err(e) => {
                    eprintln!("Failed to parse SVG for PNG export: {}", e);
                    return;
                }
            };

            // Render into a raster pixmap
            let mut pixmap = match Pixmap::new(width as u32, height as u32) {
                Some(p) => p,
                None => {
                    eprintln!("Failed to create pixmap {}x{}", width, height);
                    return;
                }
            };

            // Render using resvg's default renderer
            let mut pmut = pixmap.as_mut();
            let _ = resvg::render(&tree, tiny_skia::Transform::identity(), &mut pmut);

            // Save via a dialog
            tokio::spawn(async move {
                if let Some(handle) = rfd::AsyncFileDialog::new()
                    .add_filter("PNG", &["png"]) 
                    .set_file_name("flowchart.png")
                    .save_file()
                    .await
                {
                    let path = handle.path();
                    if let Err(e) = pixmap.save_png(path) {
                        eprintln!("Failed to save PNG: {}", e);
                    }
                }
            });
        }
    }

    /// Build an SVG string representing the current flowchart. Returns (svg, width, height).
    fn build_svg_string(&self) -> (String, u32, u32) {
        // Compute world bounds from nodes and groups
        let margin = 20.0_f32; // px
        let mut min_x = f32::INFINITY;
        let mut max_x = f32::NEG_INFINITY;
        let mut min_y = f32::INFINITY;
        let mut max_y = f32::NEG_INFINITY;

        let node_w = constants::NODE_WIDTH;
        let node_h = constants::NODE_HEIGHT;

        // Include nodes
        for node in self.flowchart.nodes.values() {
            let cx = node.position.0;
            let cy = node.position.1;
            min_x = min_x.min(cx - node_w / 2.0);
            max_x = max_x.max(cx + node_w / 2.0);
            min_y = min_y.min(cy - node_h / 2.0);
            max_y = max_y.max(cy + node_h / 2.0);
        }

        // Include groups
        for (gid, group) in &self.flowchart.groups {
            match group.drawing {
                GroupDrawingMode::Rectangle => {
                    if let Some(rect) = self.group_world_rect(*gid) {
                        min_x = min_x.min(rect.min.x);
                        min_y = min_y.min(rect.min.y);
                        max_x = max_x.max(rect.max.x);
                        max_y = max_y.max(rect.max.y);
                    }
                }
                GroupDrawingMode::Polygon => {
                    if let Some(poly) = self.group_world_polygon(*gid) {
                        for p in poly {
                            min_x = min_x.min(p.x);
                            min_y = min_y.min(p.y);
                            max_x = max_x.max(p.x);
                            max_y = max_y.max(p.y);
                        }
                    }
                }
            }
        }

        // Fallback if no nodes/groups: provide a small canvas
        if !min_x.is_finite() || !min_y.is_finite() {
            min_x = 0.0;
            min_y = 0.0;
            max_x = node_w;
            max_y = node_h;
        }

        let width = ((max_x - min_x) + 2.0 * margin).ceil().max(1.0) as u32;
        let height = ((max_y - min_y) + 2.0 * margin).ceil().max(1.0) as u32;

        // Helper closures to map world->svg coordinates
        let map_x = |x: f32| (x - min_x + margin);
        let map_y = |y: f32| (y - min_y + margin);

        let mut out = String::new();
        use std::fmt::Write as _;

        // SVG header with defs for arrow marker
        let _ = writeln!(
            out,
            "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"{}\" height=\"{}\" viewBox=\"0 0 {} {}\">",
            width,
            height,
            width,
            height
        );

        // Background (transparent)
        let _ = writeln!(out, "<defs>");
        let _ = writeln!(out, "  <marker id=\"arrow\" markerWidth=\"10\" markerHeight=\"7\" refX=\"10\" refY=\"3.5\" orient=\"auto\">\n    <polygon points=\"0 0, 10 3.5, 0 7\" fill=\"#555\"/>\n  </marker>");
        let _ = writeln!(out, "</defs>");

        // Optional grid
        if self.canvas.show_grid {
            let grid = constants::GRID_SIZE.max(4.0);
            let color = "#cccccc";
            let opacity = 0.15;
            let start_x = (min_x / grid).floor() * grid;
            let end_x = (max_x / grid).ceil() * grid;
            let start_y = (min_y / grid).floor() * grid;
            let end_y = (max_y / grid).ceil() * grid;
            let _ = writeln!(out, "<g stroke=\"{}\" stroke-opacity=\"{}\" stroke-width=\"1\">", color, opacity);
            let mut x = start_x;
            while x <= end_x {
                let sx = map_x(x);
                let _ = writeln!(
                    out,
                    "  <line x1=\"{sx}\" y1=\"0\" x2=\"{sx}\" y2=\"{h}\" />",
                    sx = sx,
                    h = height
                );
                x += grid;
            }
            let mut y = start_y;
            while y <= end_y {
                let sy = map_y(y);
                let _ = writeln!(
                    out,
                    "  <line x1=\"0\" y1=\"{sy}\" x2=\"{w}\" y2=\"{sy}\" />",
                    sy = sy,
                    w = width
                );
                y += grid;
            }
            let _ = writeln!(out, "</g>");
        }

        // Groups (background)
        for (gid, group) in &self.flowchart.groups {
            match group.drawing {
                GroupDrawingMode::Rectangle => {
                    if let Some(rect) = self.group_world_rect(*gid) {
                        let x = map_x(rect.min.x);
                        let y = map_y(rect.min.y);
                        let w = (rect.max.x - rect.min.x).max(0.0);
                        let h = (rect.max.y - rect.min.y).max(0.0);
                        let radius = constants::GROUP_CORNER_RADIUS;
                        let _ = writeln!(
                            out,
                            "<rect x=\"{x}\" y=\"{y}\" width=\"{w}\" height=\"{h}\" rx=\"{r}\" ry=\"{r}\" fill=\"#808080\" fill-opacity=\"0.08\" stroke=\"#808080\" stroke-opacity=\"0.5\" stroke-width=\"1.5\" />",
                            x = x,
                            y = y,
                            w = w,
                            h = h,
                            r = radius
                        );
                        // Label bottom-left
                        let label = if group.name.is_empty() { "Unnamed Group" } else { &group.name };
                        let pad = constants::GROUP_LABEL_PADDING_BASE;
                        let tx = x + pad;
                        let ty = y + h - pad;
                        let _ = writeln!(out, "<text x=\"{:.1}\" y=\"{:.1}\" font-size=\"12\" fill=\"#444\" dominant-baseline=\"ideographic\">{}</text>", tx, ty, escape_xml(label));
                    }
                }
                GroupDrawingMode::Polygon => {
                    if let Some(poly) = self.group_world_polygon(*gid) {
                        let mut d = String::new();
                        for (i, p) in poly.iter().enumerate() {
                            let x = map_x(p.x);
                            let y = map_y(p.y);
                            if i == 0 {
                                let _ = write!(d, "M{:.1},{:.1}", x, y);
                            } else {
                                let _ = write!(d, " L{:.1},{:.1}", x, y);
                            }
                        }
                        let _ = write!(d, " Z");
                        let _ = writeln!(
                            out,
                            "<path d=\"{}\" fill=\"#8095ff\" fill-opacity=\"0.12\" stroke=\"#8095ff\" stroke-opacity=\"0.8\" stroke-width=\"1.5\" />",
                            d
                        );
                        if !group.name.is_empty() {
                            // Rough centroid for label
                            let (mut sx, mut sy) = (0.0, 0.0);
                            for p in &poly { sx += map_x(p.x); sy += map_y(p.y); }
                            let n = poly.len().max(1) as f32;
                            let cx = sx / n;
                            let cy = sy / n;
                            let _ = writeln!(out, "<text x=\"{:.1}\" y=\"{:.1}\" font-size=\"12\" fill=\"#444\" dominant-baseline=\"middle\" text-anchor=\"middle\">{}</text>", cx, cy, escape_xml(&group.name));
                        }
                    }
                }
            }
        }

        // Connections
        let _ = writeln!(out, "<g stroke=\"#555\" stroke-width=\"2\" fill=\"none\" marker-end=\"url(#arrow)\"> ");
        for conn in &self.flowchart.connections {
            if let (Some(from), Some(to)) = (
                self.flowchart.nodes.get(&conn.from),
                self.flowchart.nodes.get(&conn.to),
            ) {
                let (sx, sy) = (map_x(from.position.0), map_y(from.position.1));
                let (tx, ty) = (map_x(to.position.0), map_y(to.position.1));
                let _ = writeln!(out, "  <line x1=\"{:.1}\" y1=\"{:.1}\" x2=\"{:.1}\" y2=\"{:.1}\" />", sx, sy, tx, ty);
            }
        }
        let _ = writeln!(out, "</g>");

        // Nodes
        for node in self.flowchart.nodes.values() {
            let cx = map_x(node.position.0);
            let cy = map_y(node.position.1);
            let x = cx - node_w / 2.0;
            let y = cy - node_h / 2.0;
            let (fill, stroke) = match node.node_type {
                NodeType::Producer { .. } => ("#90EE90", "#000000"), // lightgreen
                NodeType::Consumer { .. } => ("#FF9999", "#000000"), // light red approx
                NodeType::Transformer { .. } => ("#ADD8E6", "#000000"), // lightblue
            };
            let _ = writeln!(
                out,
                "<rect x=\"{:.1}\" y=\"{:.1}\" width=\"{:.1}\" height=\"{:.1}\" rx=\"5\" ry=\"5\" fill=\"{}\" stroke=\"{}\" stroke-width=\"2\" />",
                x, y, node_w, node_h, fill, stroke
            );
            // Text centered
            let label = escape_xml(&node.name);
            let _ = writeln!(
                out,
                "<text x=\"{:.1}\" y=\"{:.1}\" font-size=\"12\" fill=\"#000\" text-anchor=\"middle\" dominant-baseline=\"central\">{}</text>",
                cx,
                cy,
                label
            );
        }

        // Close SVG
        let _ = writeln!(out, "</svg>");

        (out, width, height)
    }
}

fn escape_xml(input: &str) -> String {
    let mut s = String::with_capacity(input.len());
    for ch in input.chars() {
        match ch {
            '&' => s.push_str("&amp;"),
            '<' => s.push_str("&lt;"),
            '>' => s.push_str("&gt;"),
            '"' => s.push_str("&quot;"),
            '\'' => s.push_str("&apos;"),
            _ => s.push(ch),
        }
    }
    s
}
