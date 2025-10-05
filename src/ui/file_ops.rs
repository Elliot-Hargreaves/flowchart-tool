//! File operations for saving and loading flowcharts.
//!
//! This module handles all file I/O operations including native file dialogs
//! and WASM-compatible browser-based file operations.

use super::state::{FlowchartApp, FileOperationResult, PendingSaveOperation, PendingLoadOperation};
use crate::types::Flowchart;
use eframe::egui;

impl FlowchartApp {
    /// Handles pending file operations for both native and WASM platforms.
    ///
    /// This method processes completed async file operations and initiates new ones.
    /// It handles the differences between native file dialogs and browser-based file operations.
    ///
    /// # Arguments
    ///
    /// * `ctx` - The egui context for requesting repaints
    pub fn handle_pending_operations(&mut self, ctx: &egui::Context) {
        // First, process any completed file operations from the channel
        if let Some(receiver) = &self.file.file_operation_receiver {
            while let Ok(result) = receiver.try_recv() {
                match result {
                    FileOperationResult::SaveCompleted(path) => {
                        self.file.current_path = Some(path);
                        self.file.has_unsaved_changes = false;
                        println!("File saved successfully");
                    }
                    FileOperationResult::LoadCompleted(path, content) => {
                        match Flowchart::from_json(&content) {
                            Ok(flowchart) => {
                                self.flowchart = flowchart;
                                self.file.current_path = Some(path);
                                self.file.has_unsaved_changes = false;
                                self.interaction.selected_node = None;
                                self.interaction.editing_node_name = None;
                                // Update node counter to avoid ID conflicts
                                self.node_counter = self.flowchart.nodes.len() as u32;
                                println!("File loaded successfully");
                            }
                            Err(e) => {
                                eprintln!("Failed to parse flowchart: {}", e);
                            }
                        }
                    }
                    FileOperationResult::OperationFailed(error) => {
                        eprintln!("File operation failed: {}", error);
                    }
                }
            }
        }

        // Handle pending save operations
        if let Some(save_op) = self.file.pending_save_operation.take() {
            let ctx = ctx.clone();
            let flowchart_json = self.flowchart.to_json().unwrap_or_default();
            let sender = self.file.file_operation_sender.clone();

            match save_op {
                PendingSaveOperation::SaveAs => {
                    #[cfg(target_arch = "wasm32")]
                    {
                        // Use synchronous download for Firefox compatibility
                        match Self::trigger_download("flowchart.json", &flowchart_json) {
                            Ok(_) => {
                                if let Some(tx) = sender {
                                    let _ = tx.send(FileOperationResult::SaveCompleted("flowchart.json".to_string()));
                                }
                            }
                            Err(e) => {
                                if let Some(tx) = sender {
                                    let _ = tx.send(FileOperationResult::OperationFailed(e));
                                }
                            }
                        }
                        ctx.request_repaint();
                    }

                    #[cfg(not(target_arch = "wasm32"))]
                    {
                        tokio::spawn(async move {
                            if let Some(handle) = rfd::AsyncFileDialog::new()
                                .add_filter("JSON", &["json"])
                                .set_file_name("flowchart.json")
                                .save_file()
                                .await
                            {
                                let path = handle.path();
                                match std::fs::write(path, flowchart_json) {
                                    Ok(_) => {
                                        if let Some(tx) = sender {
                                            let _ = tx.send(FileOperationResult::SaveCompleted(
                                                path.display().to_string()
                                            ));
                                        }
                                    }
                                    Err(e) => {
                                        if let Some(tx) = sender {
                                            let _ = tx.send(FileOperationResult::OperationFailed(
                                                format!("Failed to save file: {}", e)
                                            ));
                                        }
                                    }
                                }
                            }
                            ctx.request_repaint();
                        });
                    }
                }
                PendingSaveOperation::Save => {
                    if let Some(path) = self.file.current_path.clone() {
                        #[cfg(not(target_arch = "wasm32"))]
                        {
                            tokio::spawn(async move {
                                match std::fs::write(&path, flowchart_json) {
                                    Ok(_) => {
                                        if let Some(tx) = sender {
                                            let _ = tx.send(FileOperationResult::SaveCompleted(path));
                                        }
                                    }
                                    Err(e) => {
                                        if let Some(tx) = sender {
                                            let _ = tx.send(FileOperationResult::OperationFailed(
                                                format!("Failed to save file: {}", e)
                                            ));
                                        }
                                    }
                                }
                                ctx.request_repaint();
                            });
                        }

                        #[cfg(target_arch = "wasm32")]
                        {
                            // For WASM, we can't "save" to a previous path without user interaction
                            // Fall back to Save As
                            self.file.pending_save_operation = Some(PendingSaveOperation::SaveAs);
                        }
                    } else {
                        self.file.pending_save_operation = Some(PendingSaveOperation::SaveAs);
                    }
                }
            }
        }

        // Handle pending load operations
        if let Some(_load_op) = self.file.pending_load_operation.take() {
            let ctx = ctx.clone();
            let sender = self.file.file_operation_sender.clone();

            #[cfg(target_arch = "wasm32")]
            {
                wasm_bindgen_futures::spawn_local(async move {
                    match Self::show_open_file_picker().await {
                        Some(file) => {
                            let filename = file.name();
                            match Self::read_file(file).await {
                                Ok(content) => {
                                    if let Some(tx) = sender {
                                        let _ = tx.send(FileOperationResult::LoadCompleted(filename, content));
                                    }
                                }
                                Err(e) => {
                                    if let Some(tx) = sender {
                                        let _ = tx.send(FileOperationResult::OperationFailed(e));
                                    }
                                }
                            }
                        }
                        None => {
                            eprintln!("Open dialog cancelled or API not supported");
                        }
                    }
                    ctx.request_repaint();
                });
            }

            #[cfg(not(target_arch = "wasm32"))]
            {
                tokio::spawn(async move {
                    if let Some(handle) = rfd::AsyncFileDialog::new()
                        .add_filter("JSON", &["json"])
                        .pick_file()
                        .await
                    {
                        let path = handle.path();
                        match std::fs::read_to_string(path) {
                            Ok(json) => {
                                if let Some(tx) = sender {
                                    let _ = tx.send(FileOperationResult::LoadCompleted(
                                        path.display().to_string(),
                                        json
                                    ));
                                }
                            }
                            Err(e) => {
                                if let Some(tx) = sender {
                                    let _ = tx.send(FileOperationResult::OperationFailed(
                                        format!("Failed to read file: {}", e)
                                    ));
                                }
                            }
                        }
                    }
                    ctx.request_repaint();
                });
            }
        }
    }

    /// Triggers a file download in the browser (WASM only, Firefox-compatible).
    ///
    /// Creates a temporary anchor element with a blob URL and triggers a download.
    ///
    /// # Arguments
    ///
    /// * `filename` - The name to give the downloaded file
    /// * `content` - The content to write to the file
    ///
    /// # Returns
    ///
    /// `Ok(())` if successful, or an error message if the operation fails.
    #[cfg(target_arch = "wasm32")]
    fn trigger_download(filename: &str, content: &str) -> Result<(), String> {
        use wasm_bindgen::JsCast;

        let window = web_sys::window().ok_or("No window found")?;
        let document = window.document().ok_or("No document found")?;

        // Create a Blob containing the file content
        let blob_parts = js_sys::Array::new();
        blob_parts.push(&wasm_bindgen::JsValue::from_str(content));

        let mut blob_options = web_sys::BlobPropertyBag::new();
        blob_options.type_("application/json");

        let blob = web_sys::Blob::new_with_str_sequence_and_options(&blob_parts, &blob_options)
            .map_err(|_| "Failed to create blob")?;

        // Create object URL for the blob
        let url = web_sys::Url::create_object_url_with_blob(&blob)
            .map_err(|_| "Failed to create object URL")?;

        // Create a temporary anchor element and trigger download
        let anchor = document
            .create_element("a")
            .map_err(|_| "Failed to create anchor element")?
            .dyn_into::<web_sys::HtmlAnchorElement>()
            .map_err(|_| "Failed to cast to anchor element")?;

        anchor.set_href(&url);
        anchor.set_download(filename);
        anchor.style().set_property("display", "none").ok();

        document.body()
            .ok_or("No body found")?
            .append_child(&anchor)
            .map_err(|_| "Failed to append anchor")?;

        anchor.click();

        document.body()
            .ok_or("No body found")?
            .remove_child(&anchor)
            .map_err(|_| "Failed to remove anchor")?;

        // Clean up the object URL
        web_sys::Url::revoke_object_url(&url)
            .map_err(|_| "Failed to revoke object URL")?;

        Ok(())
    }

    /// Opens a file picker dialog in the browser (WASM only, Firefox-compatible).
    ///
    /// Creates a temporary file input element and waits for the user to select a file.
    ///
    /// # Returns
    ///
    /// The selected `File` object, or `None` if the user cancelled or the operation failed.
    #[cfg(target_arch = "wasm32")]
    async fn show_open_file_picker() -> Option<web_sys::File> {
        use wasm_bindgen::JsCast;
        use wasm_bindgen::closure::Closure;

        let window = web_sys::window()?;
        let document = window.document()?;

        // Create a file input element
        let input = document
            .create_element("input")
            .ok()?
            .dyn_into::<web_sys::HtmlInputElement>()
            .ok()?;

        input.set_type("file");
        input.set_accept(".json,application/json");
        input.style().set_property("display", "none").ok()?;

        // Create a promise to wait for file selection
        let (sender, receiver) = futures::channel::oneshot::channel::<Option<web_sys::File>>();
        let sender = std::rc::Rc::new(std::cell::RefCell::new(Some(sender)));

        let onchange = Closure::wrap(Box::new(move |event: web_sys::Event| {
            let input = event.target()
                .and_then(|t| t.dyn_into::<web_sys::HtmlInputElement>().ok());

            if let Some(input) = input {
                let file = input.files()
                    .and_then(|files| files.get(0));

                if let Some(sender) = sender.borrow_mut().take() {
                    let _ = sender.send(file);
                }
            }
        }) as Box<dyn FnMut(_)>);

        input.set_onchange(Some(onchange.as_ref().unchecked_ref()));
        onchange.forget();

        // Append to body and trigger click
        document.body()?.append_child(&input).ok()?;
        input.click();

        // Wait for file selection
        let file = receiver.await.ok()??;

        // Clean up
        document.body()?.remove_child(&input).ok()?;

        Some(file)
    }

    /// Reads content from a File object (WASM only).
    ///
    /// Uses the FileReader API to asynchronously read the file contents as text.
    ///
    /// # Arguments
    ///
    /// * `file` - The web_sys::File object to read
    ///
    /// # Returns
    ///
    /// The file content as a string, or an error message if reading fails.
    #[cfg(target_arch = "wasm32")]
    async fn read_file(file: web_sys::File) -> Result<String, String> {
        use wasm_bindgen::JsCast;
        use wasm_bindgen::JsValue;

        let file_reader = web_sys::FileReader::new()
            .map_err(|_| "Failed to create FileReader".to_string())?;

        let promise = js_sys::Promise::new(&mut |resolve, reject| {
            let reader = file_reader.clone();

            let onload = wasm_bindgen::closure::Closure::wrap(Box::new(move |_event: web_sys::ProgressEvent| {
                if let Ok(result) = reader.result() {
                    let _ = resolve.call1(&JsValue::NULL, &result);
                }
            }) as Box<dyn FnMut(_)>);

            file_reader.set_onload(Some(onload.as_ref().unchecked_ref()));
            onload.forget();

            let onerror = wasm_bindgen::closure::Closure::wrap(Box::new(move |_event: web_sys::ProgressEvent| {
                let _ = reject.call1(&JsValue::NULL, &JsValue::from_str("Failed to read file"));
            }) as Box<dyn FnMut(_)>);

            file_reader.set_onerror(Some(onerror.as_ref().unchecked_ref()));
            onerror.forget();
        });

        file_reader.read_as_text(&file)
            .map_err(|_| "Failed to start reading file".to_string())?;

        let result = wasm_bindgen_futures::JsFuture::from(promise).await
            .map_err(|e| format!("Failed to read file: {:?}", e))?;

        result.as_string()
            .ok_or_else(|| "File content is not a string".to_string())
    }

    /// Opens a file dialog to save the flowchart with a new name.
    pub fn save_as_flowchart(&mut self) {
        self.file.pending_save_operation = Some(PendingSaveOperation::SaveAs);
    }

    /// Saves the flowchart to the current file path, or triggers "Save As" if no path is set.
    pub fn save_flowchart(&mut self) {
        if self.file.current_path.is_some() {
            self.file.pending_save_operation = Some(PendingSaveOperation::Save);
        } else {
            self.save_as_flowchart();
        }
    }

    /// Opens a file dialog to load a flowchart from disk or browser storage.
    pub fn load_flowchart(&mut self) {
        self.file.pending_load_operation = Some(PendingLoadOperation::Load);
    }

    /// Creates a new empty flowchart, resetting all state.
    pub fn new_flowchart(&mut self) {
        self.flowchart = Flowchart::new();
        self.flowchart.current_step = 0;
        self.file.current_path = None;
        self.file.has_unsaved_changes = false;
        self.interaction.selected_node = None;
        self.interaction.editing_node_name = None;
        self.node_counter = 0;
        self.canvas.offset = egui::Vec2::ZERO;
        self.canvas.zoom_factor = 1.0;
    }
}
