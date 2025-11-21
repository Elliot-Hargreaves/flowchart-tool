//! Export utilities: render the current flowchart to SVG and PNG.
//!
//! Notes:
//! - SVG export is supported on all targets (native + wasm).
//! - PNG export is supported on native targets only (wasm skipped).

use crate::constants;
use crate::types::*;
#[cfg(not(target_arch = "wasm32"))]
use std::sync::Arc;

use super::state::{ConnectionStyle, ExportOptions, ExportScope, FlowchartApp, TextWrappingMode};

impl FlowchartApp {
    /// Export with options to SVG: open a save dialog (native) or trigger a download (wasm).
    pub fn export_svg_with_options(&mut self, ctx: &eframe::egui::Context, options: &ExportOptions) {
        let (svg, _w, _h) = self.build_svg_with_options(ctx, options);

        #[cfg(target_arch = "wasm32")]
        {
            if let Err(e) = Self::trigger_download("flowchart.svg", &svg) {
                eprintln!("Failed to start SVG download: {}", e);
            }
        }

        #[cfg(not(target_arch = "wasm32"))]
        {
            let svg_copy = svg.clone();
            tokio::spawn(async move {
                if let Some(handle) = rfd::AsyncFileDialog::new()
                    .add_filter("SVG", &["svg"])
                    .set_file_name("flowchart.svg")
                    .save_file()
                    .await
                {
                    let path = handle.path();
                    if let Err(e) = std::fs::write(path, svg_copy.as_bytes()) {
                        eprintln!("Failed to save SVG: {}", e);
                    }
                }
            });
        }
    }

    /// Export with options to PNG (native builds only).
    pub fn export_png_with_options(&mut self, ctx: &eframe::egui::Context, options: &ExportOptions) {
        #[cfg(not(target_arch = "wasm32"))]
        {
            let (svg, width, height) = self.build_svg_with_options(ctx, options);

            use tiny_skia::Pixmap;
            use usvg;

            // Parse SVG
            let mut opt = usvg::Options::default();
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

            // Scale handling
            let scale = options.png_scale.clamp(0.25, 8.0);
            let out_w = ((width as f32) * scale).round().max(1.0) as u32;
            let out_h = ((height as f32) * scale).round().max(1.0) as u32;

            // Render into a raster pixmap
            let mut pixmap = match Pixmap::new(out_w, out_h) {
                Some(p) => p,
                None => {
                    eprintln!("Failed to create pixmap {}x{}", out_w, out_h);
                    return;
                }
            };

            // Optional background fill
            if options.include_background {
                let c = options.background_color;
                pixmap.fill(tiny_skia::Color::from_rgba8(c.r(), c.g(), c.b(), c.a()));
            }

            // Render using resvg's default renderer with scaling
            let mut pmut = pixmap.as_mut();
            let transform = tiny_skia::Transform::from_scale(scale, scale);
            let _ = resvg::render(&tree, transform, &mut pmut);

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

    /// Build an SVG string for the given options. Returns (svg, width, height).
    fn build_svg_with_options(
        &self,
        ctx: &eframe::egui::Context,
        options: &ExportOptions,
    ) -> (String, u32, u32) {
        // Compute world bounds from nodes and groups
        let margin = options.margin_px.max(0.0);
        let mut min_x = f32::INFINITY;
        let mut max_x = f32::NEG_INFINITY;
        let mut min_y = f32::INFINITY;
        let mut max_y = f32::NEG_INFINITY;

        let node_w = constants::NODE_WIDTH;
        let node_h = constants::NODE_HEIGHT;

        // Determine filtered elements per scope
        let mut included_node_ids: std::collections::HashSet<NodeId> = std::collections::HashSet::new();
        match options.scope {
            ExportScope::WholeGraph => {
                for id in self.flowchart.nodes.keys() {
                    included_node_ids.insert(*id);
                }
            }
            ExportScope::SelectionOnly => {
                if let Some(id) = self.interaction.selected_node {
                    included_node_ids.insert(id);
                }
                for id in &self.interaction.selected_nodes {
                    included_node_ids.insert(*id);
                }
            }
        }

        // Include nodes
        for (id, node) in &self.flowchart.nodes {
            if !included_node_ids.is_empty() && !included_node_ids.contains(id) {
                continue;
            }
            let cx = node.position.0;
            let cy = node.position.1;
            min_x = min_x.min(cx - node_w / 2.0);
            max_x = max_x.max(cx + node_w / 2.0);
            min_y = min_y.min(cy - node_h / 2.0);
            max_y = max_y.max(cy + node_h / 2.0);
        }

        // Include groups
        for (gid, group) in &self.flowchart.groups {
            // Skip groups with no included members when exporting selection only
            if !included_node_ids.is_empty() {
                let has_member = group.members.iter().any(|m| included_node_ids.contains(m));
                if !has_member {
                    continue;
                }
            }
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

        // SVG header
        let _ = writeln!(
            out,
            "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"{}\" height=\"{}\" viewBox=\"0 0 {} {}\">",
            width,
            height,
            width,
            height
        );

        // Background (optional solid)
        if options.include_background {
            let c = options.background_color;
            let _ = writeln!(
                out,
                "<rect x=\"0\" y=\"0\" width=\"{}\" height=\"{}\" fill=\"#{:02x}{:02x}{:02x}\" fill-opacity=\"{}\" />",
                width,
                height,
                c.r(),
                c.g(),
                c.b(),
                (c.a() as f32) / 255.0
            );
        }

        // Stroke color for connections and arrows
        let sc = options.stroke_color;

        // Optional grid (independent of current canvas setting)
        if options.include_grid {
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
            if !included_node_ids.is_empty() {
                let has_member = group.members.iter().any(|m| included_node_ids.contains(m));
                if !has_member { continue; }
            }
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

        // Connections: draw lines/paths only (no marker-end). Arrowheads are drawn later above nodes.
        let _ = writeln!(
            out,
            "<g stroke=\"#{:02x}{:02x}{:02x}\" stroke-width=\"{:.1}\" fill=\"none\"> ",
            sc.r(), sc.g(), sc.b(), options.stroke_width
        );
        // Collect arrow polygons to render after nodes to avoid occlusion
        let mut arrow_polys: Vec<String> = Vec::new();
        for conn in &self.flowchart.connections {
            if !included_node_ids.is_empty() {
                if !(included_node_ids.contains(&conn.from) && included_node_ids.contains(&conn.to)) {
                    continue;
                }
            }
            if let (Some(from), Some(to)) = (
                self.flowchart.nodes.get(&conn.from),
                self.flowchart.nodes.get(&conn.to),
            ) {
                let (sx, sy) = (map_x(from.position.0), map_y(from.position.1));
                let (tx, ty) = (map_x(to.position.0), map_y(to.position.1));
                // Arrow shape parameters derived from stroke width
                let arrow_len = 6.0 + options.stroke_width * 2.0;
                let arrow_half_w = arrow_len * 0.6;
                match options.connection_style {
                    ConnectionStyle::Straight => {
                        let _ = writeln!(
                            out,
                            "  <line x1=\"{:.1}\" y1=\"{:.1}\" x2=\"{:.1}\" y2=\"{:.1}\" />",
                            sx, sy, tx, ty
                        );
                        // Mid-point arrow polygon oriented along the line
                        let dx = tx - sx;
                        let dy = ty - sy;
                        let dist = (dx * dx + dy * dy).sqrt().max(1e-6);
                        let ux = dx / dist;
                        let uy = dy / dist;
                        let px = -uy;
                        let py = ux;
                        let cx = sx + dx * 0.5;
                        let cy = sy + dy * 0.5;
                        let tipx = cx + ux * arrow_len;
                        let tipy = cy + uy * arrow_len;
                        let leftx = cx - ux * arrow_len + px * arrow_half_w;
                        let lefty = cy - uy * arrow_len + py * arrow_half_w;
                        let rightx = cx - ux * arrow_len - px * arrow_half_w;
                        let righty = cy - uy * arrow_len - py * arrow_half_w;
                        arrow_polys.push(format!(
                            "  <polygon points=\"{:.1},{:.1} {:.1},{:.1} {:.1},{:.1}\" fill=\"#{:02x}{:02x}{:02x}\" />",
                            tipx, tipy, leftx, lefty, rightx, righty, sc.r(), sc.g(), sc.b()
                        ));
                    }
                    ConnectionStyle::Curved => {
                        // Simple cubic bezier with perpendicular offset
                        let dx = tx - sx;
                        let dy = ty - sy;
                        let dist = (dx * dx + dy * dy).sqrt();
                        let nx = if dist > 0.0 { -dy / dist } else { 0.0 };
                        let ny = if dist > 0.0 { dx / dist } else { 0.0 };
                        let offset = 0.2 * dist;
                        let c1x = sx + dx * 0.25 + nx * offset;
                        let c1y = sy + dy * 0.25 + ny * offset;
                        let c2x = sx + dx * 0.75 - nx * offset;
                        let c2y = sy + dy * 0.75 - ny * offset;
                        let _ = writeln!(
                            out,
                            "  <path d=\"M{:.1},{:.1} C{:.1},{:.1} {:.1},{:.1} {:.1},{:.1}\" />",
                            sx, sy, c1x, c1y, c2x, c2y, tx, ty
                        );
                        // Arrow at t=0.5 along the bezier using tangent for orientation
                        let t = 0.5_f32;
                        let omt = 1.0 - t;
                        // Position on curve B(t)
                        let bx = omt * omt * omt * sx
                            + 3.0 * omt * omt * t * c1x
                            + 3.0 * omt * t * t * c2x
                            + t * t * t * tx;
                        let by = omt * omt * omt * sy
                            + 3.0 * omt * omt * t * c1y
                            + 3.0 * omt * t * t * c2y
                            + t * t * t * ty;
                        // Tangent B'(t)
                        let dxb = 3.0 * omt * omt * (c1x - sx)
                            + 6.0 * omt * t * (c2x - c1x)
                            + 3.0 * t * t * (tx - c2x);
                        let dyb = 3.0 * omt * omt * (c1y - sy)
                            + 6.0 * omt * t * (c2y - c1y)
                            + 3.0 * t * t * (ty - c2y);
                        let dlen = (dxb * dxb + dyb * dyb).sqrt().max(1e-6);
                        let ux = dxb / dlen;
                        let uy = dyb / dlen;
                        let px = -uy;
                        let py = ux;
                        let tipx = bx + ux * arrow_len;
                        let tipy = by + uy * arrow_len;
                        let leftx = bx - ux * arrow_len + px * arrow_half_w;
                        let lefty = by - uy * arrow_len + py * arrow_half_w;
                        let rightx = bx - ux * arrow_len - px * arrow_half_w;
                        let righty = by - uy * arrow_len - py * arrow_half_w;
                        arrow_polys.push(format!(
                            "  <polygon points=\"{:.1},{:.1} {:.1},{:.1} {:.1},{:.1}\" fill=\"#{:02x}{:02x}{:02x}\" />",
                            tipx, tipy, leftx, lefty, rightx, righty, sc.r(), sc.g(), sc.b()
                        ));
                    }
                }
            }
        }
        let _ = writeln!(out, "</g>");

        // Nodes
        for (id, node) in &self.flowchart.nodes {
            if !included_node_ids.is_empty() && !included_node_ids.contains(id) {
                continue;
            }
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
            // Text centered with optional wrapping using egui metrics
            let label = escape_xml(&node.name);
            // Font size: derive from canvas drawing baseline (12) but it's vector; use 12 like UI
            let base_font_size = 12.0;
            match options.text_wrapping {
                TextWrappingMode::Simple => {
                    let _ = writeln!(
                        out,
                        "<text x=\"{:.1}\" y=\"{:.1}\" font-size=\"{}\" fill=\"#000\" text-anchor=\"middle\" dominant-baseline=\"central\">{}</text>",
                        cx,
                        cy,
                        base_font_size,
                        label
                    );
                }
                TextWrappingMode::CanvasLike => {
                    // Compute wrapping using egui fonts
                    let font_id = eframe::egui::FontId::proportional(base_font_size);
                    let line_height = ctx.fonts_mut(|f| f.row_height(&font_id));
                    // Max width is node_w - horizontal padding
                    let max_w = (node_w - 10.0).max(10.0);
                    let lines = wrap_text_with_egui(ctx, &label, max_w, &font_id);
                    let total_h = line_height * lines.len() as f32;
                    let start_y = cy - total_h / 2.0 + line_height * 0.5; // center baseline per line
                    let _ = writeln!(out, "<text x=\"{:.1}\" y=\"{:.1}\" font-size=\"{}\" fill=\"#000\" text-anchor=\"middle\" >", cx, start_y, base_font_size);
                    for (i, line) in lines.iter().enumerate() {
                        if i == 0 {
                            let _ = writeln!(out, "  <tspan x=\"{:.1}\" dy=\"0\">{}</tspan>", cx, line);
                        } else {
                            let _ = writeln!(out, "  <tspan x=\"{:.1}\" dy=\"{:.1}\">{}</tspan>", cx, line_height, line);
                        }
                    }
                    let _ = writeln!(out, "</text>");
                }
            }
        }

        // Arrowheads overlay: draw after nodes so they are not obscured by node rectangles
        let _ = writeln!(out, "<g>");
        for poly in arrow_polys {
            let _ = writeln!(out, "{}", poly);
        }
        let _ = writeln!(out, "</g>");

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

fn wrap_text_with_egui(
    ctx: &eframe::egui::Context,
    text: &str,
    max_width: f32,
    font_id: &eframe::egui::FontId,
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
        let width = ctx.fonts_mut(|f| {
            f.layout_no_wrap(test_line.clone(), font_id.clone(), eframe::egui::Color32::BLACK)
                .size()
                .x
        });
        if width <= max_width {
            current_line = test_line;
        } else if !current_line.is_empty() {
            lines.push(current_line);
            current_line = word.to_string();
        } else {
            // A single word longer than max width – put it as a line by itself
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
