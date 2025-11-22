//! Export utilities: render the current flowchart to SVG and PNG.
//!
//! Notes:
//! - SVG export is supported on all targets (native + wasm).
//! - PNG export is supported on all targets; on native it uses resvg/tiny-skia, on wasm it rasterizes via HTML Canvas.

use crate::constants;
use crate::types::*;
#[cfg(not(target_arch = "wasm32"))]
use std::sync::Arc;
#[cfg(target_arch = "wasm32")]
use eframe::wasm_bindgen::JsCast;

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

    /// Export with options to PNG.
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

        #[cfg(target_arch = "wasm32")]
        {
            let (svg, width, height) = self.build_svg_with_options(ctx, options);

            // Prepare DOM handles
            let window = web_sys::window().expect("window");
            let document = window.document().expect("document");

            // Create a Blob for the SVG to avoid data-URL issues
            let parts = js_sys::Array::new();
            parts.push(&eframe::wasm_bindgen::JsValue::from_str(&svg));
            let mut bag = web_sys::BlobPropertyBag::new();
            bag.set_type("image/svg+xml");
            let svg_blob = web_sys::Blob::new_with_str_sequence_and_options(&parts, &bag)
                .expect("blob");
            let svg_url = web_sys::Url::create_object_url_with_blob(&svg_blob).expect("url");

            // Create <img> to decode the SVG
            let img = document
                .create_element("img")
                .expect("img")
                .dyn_into::<web_sys::HtmlImageElement>()
                .expect("HtmlImageElement");

            // Output size with scale
            let scale = options.png_scale.clamp(0.25, 8.0);
            let out_w = ((width as f32) * scale).round().max(1.0) as u32;
            let out_h = ((height as f32) * scale).round().max(1.0) as u32;

            // Create an offscreen canvas
            let canvas = document
                .create_element("canvas")
                .expect("canvas")
                .dyn_into::<web_sys::HtmlCanvasElement>()
                .expect("HtmlCanvasElement");
            canvas.set_width(out_w);
            canvas.set_height(out_h);
            let ctx2d = canvas
                .get_context("2d")
                .expect("2d ctx")
                .expect("2d ctx opt")
                .dyn_into::<web_sys::CanvasRenderingContext2d>()
                .expect("CanvasRenderingContext2d");

            // Optional background fill
            if options.include_background {
                let c = options.background_color;
                let rgba = format!(
                    "rgba({}, {}, {}, {})",
                    c.r(),
                    c.g(),
                    c.b(),
                    (c.a() as f32) / 255.0
                );
                ctx2d.set_fill_style(&eframe::wasm_bindgen::JsValue::from_str(&rgba));
                ctx2d.fill_rect(0.0, 0.0, out_w as f64, out_h as f64);
            }

            // Set up onload to draw and download
            let onload = {
                let ctx2d = ctx2d.clone();
                let canvas = canvas.clone();
                let document = document.clone();
                let svg_url_for_cleanup = svg_url.clone();
                let img_for_draw = img.clone();
                eframe::wasm_bindgen::closure::Closure::wrap(Box::new(move || {
                    // Draw with scaling
                    let _ = ctx2d.save();
                    let _ = ctx2d.scale(scale as f64, scale as f64);
                    let _ = ctx2d.draw_image_with_html_image_element(&img_for_draw, 0.0, 0.0);
                    let _ = ctx2d.restore();

                    // Convert to PNG blob and trigger a download
                    // Clone handles so the outer onload closure doesn't move its captured vars,
                    // allowing it to implement FnMut rather than FnOnce.
                    let document_for_cb = document.clone();
                    let svg_url_for_cb = svg_url_for_cleanup.clone();

                    let callback = eframe::wasm_bindgen::closure::Closure::wrap(Box::new(
                        move |blob_opt: Option<web_sys::Blob>| {
                            if let Some(blob) = blob_opt {
                                if let Ok(url) = web_sys::Url::create_object_url_with_blob(&blob) {
                                    if let Ok(anchor_el) = document_for_cb.create_element("a") {
                                        if let Ok(anchor) = anchor_el
                                            .dyn_into::<web_sys::HtmlAnchorElement>()
                                        {
                                            anchor.set_href(&url);
                                            anchor.set_download("flowchart.png");
                                            // Append to body for broader browser compatibility
                                            if let Some(body) = document_for_cb.body() {
                                                let _ = body.append_child(&anchor);
                                                anchor.click();
                                                let _ = body.remove_child(&anchor);
                                            } else {
                                                anchor.click();
                                            }
                                        }
                                    }
                                    let _ = web_sys::Url::revoke_object_url(&url);
                                }
                            }
                            // Clean up the SVG object URL
                            let _ = web_sys::Url::revoke_object_url(&svg_url_for_cb);
                        },
                    ) as Box<dyn FnMut(Option<web_sys::Blob>)>);

                    // Use the correct web-sys API: to_blob_with_type(callback, mime)
                    let _ = canvas.to_blob_with_type(
                        callback.as_ref().unchecked_ref(),
                        "image/png",
                    );
                    // Keep the closure alive until JS calls back
                    callback.forget();
                }) as Box<dyn FnMut()>)
            };

            img.set_onload(Some(onload.as_ref().unchecked_ref()));
            img.set_src(&svg_url);
            // Prevent Rust from dropping the closure too early
            onload.forget();
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
                        let _ = writeln!(out, "<text x=\"{:.1}\" y=\"{:.1}\" font-size=\"12\" font-family=\"sans-serif\" fill=\"#444\" dominant-baseline=\"ideographic\">{}</text>", tx, ty, escape_xml(label));
                    }
                }
                GroupDrawingMode::Polygon => {
                    if let Some(world_poly) = self.group_world_polygon(*gid) {
                        // Map to SVG pixel space
                        let mut pts_px: Vec<(f32, f32)> = Vec::with_capacity(world_poly.len());
                        for p in &world_poly {
                            pts_px.push((map_x(p.x), map_y(p.y)));
                        }
                        // Apply rounded-corner approximation matching canvas
                        let rounded = round_polygon_points_svg(&pts_px, constants::GROUP_CORNER_RADIUS, 6);

                        // Emit as a polyline path
                        if rounded.len() >= 3 {
                            let mut d = String::new();
                            for (i, (x, y)) in rounded.iter().copied().enumerate() {
                                if i == 0 {
                                    let _ = write!(d, "M{:.1},{:.1}", x, y);
                                } else {
                                    let _ = write!(d, " L{:.1},{:.1}", x, y);
                                }
                            }
                            let _ = write!(d, " Z");
                            let _ = writeln!(
                                out,
                                "<path d=\"{}\" fill=\"#808080\" fill-opacity=\"0.08\" stroke=\"#808080\" stroke-opacity=\"0.5\" stroke-width=\"1.5\" />",
                                d
                            );
                        }

                        // Label at area-weighted centroid (world), mapped to SVG
                        if !group.name.is_empty() {
                            if let Some((cxw, cyw)) = polygon_centroid_world(&world_poly) {
                                let cx = map_x(cxw);
                                let cy = map_y(cyw);
                                let _ = writeln!(out, "<text x=\"{:.1}\" y=\"{:.1}\" font-size=\"12\" font-family=\"sans-serif\" fill=\"#444\" dominant-baseline=\"middle\" text-anchor=\"middle\">{}</text>", cx, cy, escape_xml(&group.name));
                            }
                        }
                    }
                }
            }
        }

        // Connections: draw lines/paths and arrowheads together (no marker-end), below nodes.
        let _ = writeln!(
            out,
            "<g stroke=\"#{:02x}{:02x}{:02x}\" stroke-width=\"{:.1}\" fill=\"none\"> ",
            sc.r(), sc.g(), sc.b(), options.stroke_width
        );
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
                        let _ = writeln!(
                            out,
                            "  <polygon points=\"{:.1},{:.1} {:.1},{:.1} {:.1},{:.1}\" fill=\"#{:02x}{:02x}{:02x}\" />",
                            tipx, tipy, leftx, lefty, rightx, righty, sc.r(), sc.g(), sc.b()
                        );
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
                        let _ = writeln!(
                            out,
                            "  <polygon points=\"{:.1},{:.1} {:.1},{:.1} {:.1},{:.1}\" fill=\"#{:02x}{:02x}{:02x}\" />",
                            tipx, tipy, leftx, lefty, rightx, righty, sc.r(), sc.g(), sc.b()
                        );
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
                        "<text x=\"{:.1}\" y=\"{:.1}\" font-size=\"{}\" font-family=\"sans-serif\" fill=\"#000\" text-anchor=\"middle\" dominant-baseline=\"central\">{}</text>",
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
                    let _ = writeln!(out, "<text x=\"{:.1}\" y=\"{:.1}\" font-size=\"{}\" font-family=\"sans-serif\" fill=\"#000\" text-anchor=\"middle\" >", cx, start_y, base_font_size);
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

        // Arrowheads are already drawn with connections above; nothing else to add here.

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

// ===== Helper geometry for export (SVG coordinate space) =====

/// Area-weighted centroid in world space; returns (x, y) or None if degenerate.
fn polygon_centroid_world(points: &[eframe::egui::Pos2]) -> Option<(f32, f32)> {
    if points.len() < 3 {
        return None;
    }
    let mut a = 0.0f32;
    let mut cx = 0.0f32;
    let mut cy = 0.0f32;
    for i in 0..points.len() {
        let p = points[i];
        let q = points[(i + 1) % points.len()];
        let cross = p.x * q.y - q.x * p.y;
        a += cross;
        cx += (p.x + q.x) * cross;
        cy += (p.y + q.y) * cross;
    }
    if a.abs() < 1e-6 {
        return None;
    }
    let inv = 1.0 / (3.0 * a);
    Some((cx * inv, cy * inv))
}

/// Rounded-corner approximation of a closed polygon in SVG pixel space.
/// `points` are (x, y) pixels, Y grows downward like egui's screen space.
fn round_polygon_points_svg(points: &[(f32, f32)], radius_px: f32, segments_per_corner: usize) -> Vec<(f32, f32)> {
    let n = points.len();
    if n < 3 || radius_px <= 0.0 || segments_per_corner == 0 {
        return points.to_vec();
    }

    // Shoelace for orientation; in Y-down, visually CCW yields negative area
    let mut signed_area = 0.0f32;
    for i in 0..n {
        let (px, py) = points[i];
        let (qx, qy) = points[(i + 1) % n];
        signed_area += px * qy - qx * py;
    }
    let ccw = signed_area < 0.0;
    let segs = segments_per_corner.max(1);

    #[inline]
    fn perp_left(vx: f32, vy: f32) -> (f32, f32) { (-vy, vx) }

    #[inline]
    fn normalize(vx: f32, vy: f32) -> (f32, f32) {
        let len = (vx * vx + vy * vy).sqrt().max(1e-6);
        (vx / len, vy / len)
    }

    #[inline]
    fn project_point_on_line(ax: f32, ay: f32, dx: f32, dy: f32, px: f32, py: f32) -> (f32, f32) {
        let dir2 = (dx * dx + dy * dy).max(1e-12);
        let apx = px - ax;
        let apy = py - ay;
        let t = (apx * dx + apy * dy) / dir2;
        (ax + dx * t, ay + dy * t)
    }

    #[inline]
    fn intersect_lines(ax: f32, ay: f32, d1x: f32, d1y: f32, bx: f32, by: f32, d2x: f32, d2y: f32) -> Option<(f32, f32)> {
        let denom = d1x * d2y - d1y * d2x;
        if denom.abs() < 1e-6 { return None; }
        let bax = bx - ax;
        let bay = by - ay;
        let t = (bax * d2y - bay * d2x) / denom;
        Some((ax + d1x * t, ay + d1y * t))
    }

    let mut out: Vec<(f32, f32)> = Vec::with_capacity(n * (segs + 1));

    for i in 0..n {
        let (px, py) = points[(i + n - 1) % n];
        let (cx, cy) = points[i];
        let (nx, ny) = points[(i + 1) % n];

        let e1x = cx - px;
        let e1y = cy - py;
        let e2x = nx - cx;
        let e2y = ny - cy;
        let len1 = (e1x * e1x + e1y * e1y).sqrt().max(1e-6);
        let len2 = (e2x * e2x + e2y * e2y).sqrt().max(1e-6);
        let (d1x, d1y) = (e1x / len1, e1y / len1);
        let (d2x, d2y) = (e2x / len2, e2y / len2);

        // Turn sign; in Y-down, convex test flips based on winding
        let turn = d1x * d2y - d1y * d2x;
        let is_convex = if ccw { turn < 0.0 } else { turn > 0.0 };

        let r_max1 = 0.5 * len1;
        let r_max2 = 0.5 * len2;
        let r = radius_px.min(r_max1).min(r_max2);
        if r <= 0.0 {
            out.push((cx, cy));
            continue;
        }

        if !is_convex {
            out.push((cx, cy));
            continue;
        }

        let side = if ccw { 1.0 } else { -1.0 };
        let (n1x, n1y) = perp_left(d1x, d1y);
        let (n2x, n2y) = perp_left(d2x, d2y);
        let (n1x, n1y) = (n1x * side, n1y * side);
        let (n2x, n2y) = (n2x * side, n2y * side);

        let p1x = cx + n1x * r;
        let p1y = cy + n1y * r;
        let p2x = cx + n2x * r;
        let p2y = cy + n2y * r;

        let center = if let Some((cx2, cy2)) = intersect_lines(p1x, p1y, d1x, d1y, p2x, p2y, d2x, d2y) {
            (cx2, cy2)
        } else {
            out.push((cx, cy));
            continue;
        };

        let (t1x, t1y) = project_point_on_line(cx, cy, d1x, d1y, center.0, center.1);
        let (t2x, t2y) = project_point_on_line(cx, cy, d2x, d2y, center.0, center.1);

        // Clamp tangency distance along edges
        let clamp = |tx: f32, ty: f32, bx: f32, by: f32, maxd: f32| -> (f32, f32) {
            let vx = tx - bx;
            let vy = ty - by;
            let l = (vx * vx + vy * vy).sqrt();
            if l > maxd && l > 1e-6 {
                let f = maxd / l;
                (bx + vx * f, by + vy * f)
            } else {
                (tx, ty)
            }
        };
        let (t1x, t1y) = clamp(t1x, t1y, cx, cy, r_max1);
        let (t2x, t2y) = clamp(t2x, t2y, cx, cy, r_max2);

        let a1 = (t1y - center.1).atan2(t1x - center.0);
        let a2 = (t2y - center.1).atan2(t2x - center.0);
        let mut delta = a2 - a1;
        while delta <= -std::f32::consts::PI { delta += std::f32::consts::TAU; }
        while delta > std::f32::consts::PI { delta -= std::f32::consts::TAU; }
        if delta.abs() < 1e-4 {
            out.push((t1x, t1y));
            continue;
        }
        let start = a1;
        for s in 0..=segs {
            let t = s as f32 / segs as f32;
            let ang = start + delta * t;
            // Match egui Y-down orientation as in canvas method
            out.push((center.0 - ang.cos() * r, center.1 - ang.sin() * r));
        }
    }
    out
}
