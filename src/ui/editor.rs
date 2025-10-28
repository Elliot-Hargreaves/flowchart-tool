use eframe::egui;

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum LanguageKind {
    Json,
    JavaScript,
}

#[derive(Clone, Debug)]
pub struct CodeEditOptions<'a> {
    pub language: LanguageKind,
    /// Indent unit to insert (e.g., "    " or "\t")
    pub indent: &'a str,
}

impl<'a> Default for CodeEditOptions<'a> {
    fn default() -> Self {
        Self {
            language: LanguageKind::Json,
            indent: "    ",
        }
    }
}

/// A very lightweight JavaScript formatter: reindents lines based on braces/brackets/parentheses
/// and removes trailing whitespace. It doesn't parse JS; it's indentation-aware only.
pub fn simple_js_format(src: &str, indent: &str) -> String {
    let mut out = String::with_capacity(src.len());
    let mut level: i32 = 0;

    for raw_line in src.lines() {
        // Trim trailing whitespace
        let line = raw_line.trim_end().to_string();
        // Determine outdent based on leading closers
        let trimmed = line.trim_start();
        let mut outdent_now = 0i32;
        if let Some(first_non_ws) = trimmed.chars().next() {
            if matches!(first_non_ws, '}' | ']' | ')') {
                outdent_now = 1;
            }
        }
        let effective_level = (level - outdent_now).max(0);
        // Write indentation
        for _ in 0..effective_level {
            out.push_str(indent);
        }
        out.push_str(trimmed);
        out.push('\n');

        // Update level for next line by scanning tokens
        let mut delta = 0i32;
        let mut in_single = false;
        let mut in_double = false;
        let mut in_back = false;
        let mut escape = false;
        for ch in trimmed.chars() {
            if escape {
                escape = false;
                continue;
            }
            match ch {
                '\\' => {
                    if in_single || in_double || in_back {
                        escape = true;
                    }
                }
                '\'' => {
                    if !in_double && !in_back {
                        in_single = !in_single;
                    }
                }
                '"' => {
                    if !in_single && !in_back {
                        in_double = !in_double;
                    }
                }
                '`' => {
                    if !in_single && !in_double {
                        in_back = !in_back;
                    }
                }
                _ => {}
            }
            if in_single || in_double || in_back {
                continue;
            }
            match ch {
                '{' | '[' | '(' => delta += 1,
                '}' | ']' | ')' => delta -= 1,
                _ => {}
            }
        }
        level = (level + delta).max(0);
    }

    // Remove the final extra newline if the original didn't end with one
    if !src.ends_with('\n') && out.ends_with('\n') {
        out.pop();
    }
    out
}

/// Returns true if the text was modified.
pub fn handle_code_textedit_keys(
    ui: &mut egui::Ui,
    response: &egui::Response,
    text: &mut String,
    options: &CodeEditOptions,
) -> bool {
    if !response.has_focus() {
        return false;
    }

    let mut changed = false;

    // Read selection/caret state
    let (mut sel_start_char, mut sel_end_char) = get_selection_char_range(ui, response.id)
        .map(|r| (r.min, r.max))
        .unwrap_or_else(|| {
            let len = text.chars().count();
            (len, len)
        });

    // Helper to refresh state after mutation
    let set_caret = |ui: &mut egui::Ui, idx_char: usize| {
        set_selection_char_range(ui, response.id, idx_char, idx_char);
    };

    // Tab / Shift+Tab
    let tab_pressed = ui.input(|i| i.key_pressed(egui::Key::Tab));
    let shift = ui.input(|i| i.modifiers.shift);

    // Enter
    let enter_pressed = ui.input(|i| i.key_pressed(egui::Key::Enter));

    // Snapshot from previous frame (before egui consumed selection on Tab)
    let prev_snapshot = get_prev_snapshot(ui, response.id);

    // If Tab was pressed, egui may have already replaced the selection with a literal '\t'.
    // To preserve selected text, restore from the previous snapshot when we detect that the
    // prior frame had a non-empty selection.
    let mut restored_from_snapshot = false;
    if tab_pressed {
        if let Some(prev) = &prev_snapshot {
            if prev.sel_start < prev.sel_end {
                // If selection collapsed now, likely egui replaced the range with a tab.
                let selection_collapsed_now = sel_start_char == sel_end_char;
                if selection_collapsed_now {
                    // Additionally check if current text equals prev_before + "\t" + prev_after
                    let before_b = char_to_byte_idx(&prev.text, prev.sel_start);
                    let after_b = char_to_byte_idx(&prev.text, prev.sel_end);
                    let mut candidate =
                        String::with_capacity(prev.text.len() - (after_b - before_b) + 1);
                    candidate.push_str(&prev.text[..before_b]);
                    candidate.push('\t');
                    candidate.push_str(&prev.text[after_b..]);
                    if *text == candidate || *text != prev.text {
                        *text = prev.text.clone();
                        sel_start_char = prev.sel_start;
                        sel_end_char = prev.sel_end;
                        set_selection_char_range(ui, response.id, sel_start_char, sel_end_char);
                        restored_from_snapshot = true;
                        changed = true; // we restored the original content
                    }
                }
            }
        }
    }

    // If Tab was pressed, normalize any default tab insertion to spaces before indenting.
    let mut normalized_default_tab = false;
    if tab_pressed && !restored_from_snapshot {
        if sel_start_char == sel_end_char {
            // Collapsed caret: if the previous char is a tab, replace it with indent spaces.
            if sel_start_char > 0 {
                let prev_byte = char_to_byte_idx(text, sel_start_char - 1);
                if text[prev_byte..].starts_with('\t') {
                    // Remove the tab and insert spaces
                    text.replace_range(prev_byte..prev_byte + 1, options.indent);
                    let delta_chars = options.indent.chars().count() - 1; // replaced 1 char with N
                    let new_pos = sel_start_char + delta_chars;
                    set_selection_char_range(ui, response.id, new_pos, new_pos);
                    changed = true;
                    normalized_default_tab = true; // don't insert another indent below
                }
            }
        } else {
            // Selection case: if a stray tab was inserted at primary caret, remove it.
            // We normalize by removing any single '\t' immediately before sel_end if present.
            if sel_end_char > 0 {
                let maybe_tab_b = char_to_byte_idx(text, sel_end_char - 1);
                if text[maybe_tab_b..].starts_with('\t') {
                    text.replace_range(maybe_tab_b..maybe_tab_b + 1, "");
                    sel_end_char -= 1;
                    changed = true;
                    // In selection case we still want to apply our indent logic below
                }
            }
        }
    }

    if tab_pressed {
        if shift {
            // Unindent current or selected lines (do not insert characters)
            let was_changed =
                unindent_selection(text, options.indent, &mut sel_start_char, &mut sel_end_char);
            if was_changed {
                set_selection_char_range(ui, response.id, sel_start_char, sel_end_char);
                changed = true;
            }
        } else if sel_start_char == sel_end_char {
            if !normalized_default_tab {
                // No selection: insert indent at caret (only spaces, no tabs)
                let caret_byte = char_to_byte_idx(text, sel_start_char);
                text.insert_str(caret_byte, options.indent);
                let added = options.indent.chars().count();
                let new_pos = sel_start_char + added;
                set_selection_char_range(ui, response.id, new_pos, new_pos);
                changed = true;
            }
        } else {
            // Selection: indent all covered lines
            let was_changed =
                indent_selection(text, options.indent, &mut sel_start_char, &mut sel_end_char);
            if was_changed {
                set_selection_char_range(ui, response.id, sel_start_char, sel_end_char);
                changed = true;
            }
        }
    }

    if enter_pressed {
        // Handle cases where egui already inserted a newline: if the char before caret is '\n',
        // only insert indentation on the new line instead of adding another newline.
        let caret_char = sel_start_char; // collapse selection (if any) to start
        let caret_byte = char_to_byte_idx(text, caret_char);
        let already_newline = if caret_char > 0 {
            let prev_b = char_to_byte_idx(text, caret_char - 1);
            text[prev_b..].starts_with('\n')
        } else {
            false
        };

        // Determine current line start (before caret) and indentation
        let line_start_byte = find_line_start_byte(text, caret_byte);
        // If egui already inserted a newline, the "current line" for indent purposes is the previous line
        let ref_byte = if already_newline && line_start_byte > 0 {
            find_line_start_byte(text, line_start_byte - 1)
        } else {
            line_start_byte
        };
        let ref_end = if already_newline {
            line_start_byte.saturating_sub(1)
        } else {
            caret_byte
        };
        let current_line = &text[ref_byte..ref_end];
        let indent_ws = current_line
            .chars()
            .take_while(|c| *c == ' ' || *c == '\t')
            .collect::<String>();

        // Smart indent if previous non-space char is an opener
        let mut extra = String::new();
        let before_iter_end = if already_newline { ref_end } else { caret_byte };
        let before = text[..before_iter_end]
            .chars()
            .rev()
            .find(|c| !c.is_whitespace());
        if matches!(
            (options.language, before),
            (LanguageKind::Json, Some('{'))
                | (LanguageKind::Json, Some('['))
                | (LanguageKind::JavaScript, Some('{'))
                | (LanguageKind::JavaScript, Some('['))
                | (LanguageKind::JavaScript, Some('('))
        ) {
            extra.push_str(options.indent);
        }

        if already_newline {
            // Only insert indentation at caret
            let insert_str = format!("{}{}", indent_ws, extra);
            text.insert_str(caret_byte, &insert_str);
            let new_caret_char = caret_char + insert_str.chars().count();
            set_caret(ui, new_caret_char);
        } else {
            // Insert newline + indentation
            let insert_str = format!("\n{}{}", indent_ws, extra);
            text.insert_str(caret_byte, &insert_str);
            let new_caret_char = caret_char + insert_str.chars().count();
            set_caret(ui, new_caret_char);
        }
        changed = true;
    }

    // Update snapshot at end so next frame can restore if needed
    let (final_sel_start, final_sel_end) = get_selection_char_range(ui, response.id)
        .map(|r| (r.min, r.max))
        .unwrap_or_else(|| {
            let len = text.chars().count();
            (len, len)
        });
    set_prev_snapshot(ui, response.id, text, final_sel_start, final_sel_end);

    changed
}

// --- helpers ---

#[derive(Clone, Debug)]
struct PrevEditSnapshot {
    text: String,
    sel_start: usize,
    sel_end: usize,
}

fn get_prev_snapshot(ui: &egui::Ui, id: egui::Id) -> Option<PrevEditSnapshot> {
    ui.memory(|mem| mem.data.get_temp::<PrevEditSnapshot>(id))
}

fn set_prev_snapshot(
    ui: &mut egui::Ui,
    id: egui::Id,
    text: &str,
    sel_start: usize,
    sel_end: usize,
) {
    ui.memory_mut(|mem| {
        mem.data.insert_temp(
            id,
            PrevEditSnapshot {
                text: text.to_owned(),
                sel_start,
                sel_end,
            },
        );
    });
}

struct CharRange {
    min: usize,
    max: usize,
}

fn get_selection_char_range(ui: &egui::Ui, id: egui::Id) -> Option<CharRange> {
    ui.memory(|mem| {
        mem.data
            .get_temp::<egui::text_edit::TextEditState>(id)
            .and_then(|s| s.cursor.char_range())
            .map(|r| {
                let a = r.primary.index;
                let b = r.secondary.index;
                CharRange {
                    min: a.min(b),
                    max: a.max(b),
                }
            })
    })
}

fn set_selection_char_range(ui: &mut egui::Ui, id: egui::Id, start_char: usize, end_char: usize) {
    ui.memory_mut(|mem| {
        let state = mem
            .data
            .get_temp_mut_or_default::<egui::text_edit::TextEditState>(id);
        use egui::text::{CCursor, CCursorRange};
        state.cursor.set_char_range(Some(CCursorRange::two(
            CCursor::new(start_char),
            CCursor::new(end_char),
        )));
    });
}

fn char_to_byte_idx(s: &str, char_idx: usize) -> usize {
    if char_idx == 0 {
        return 0;
    }
    for (count, (byte_idx, _ch)) in s.char_indices().enumerate() {
        if count == char_idx {
            return byte_idx;
        }
    }
    s.len()
}

fn byte_to_char_idx(s: &str, byte_idx: usize) -> usize {
    s[..byte_idx].chars().count()
}

fn find_line_start_byte(s: &str, caret_byte: usize) -> usize {
    let prefix = &s[..caret_byte];
    match prefix.rfind('\n') {
        Some(pos) => pos + 1,
        None => 0,
    }
}

fn find_line_end_byte(s: &str, caret_byte: usize) -> usize {
    match s[caret_byte..].find('\n') {
        Some(off) => caret_byte + off,
        None => s.len(),
    }
}

fn line_bounds_covering_selection(
    s: &str,
    sel_start_char: usize,
    sel_end_char: usize,
) -> (usize, usize) {
    let start_byte = char_to_byte_idx(s, sel_start_char);
    let end_byte = char_to_byte_idx(s, sel_end_char);

    // If there's no selection, operate on the current line only.
    if sel_start_char == sel_end_char {
        let line_start = find_line_start_byte(s, start_byte);
        let line_end = find_line_end_byte(s, start_byte);
        return (line_start, line_end);
    }

    let line_start = find_line_start_byte(s, start_byte);

    // Non-empty selection: treat end as exclusive. If the end sits exactly at the
    // start of the next line, do NOT include that next line.
    let end_is_line_start = find_line_start_byte(s, end_byte) == end_byte;

    let line_end = if end_is_line_start && end_byte > 0 {
        // Use the end of the previous line (i.e., the line that actually contains
        // selected content). We look at end_byte - 1 to be inside the previous line.
        find_line_end_byte(s, end_byte - 1)
    } else {
        // Normal case: extend to end of the line containing end_byte
        find_line_end_byte(s, end_byte)
    };

    (line_start, line_end)
}

fn indent_selection(
    s: &mut String,
    indent: &str,
    sel_start_char: &mut usize,
    sel_end_char: &mut usize,
) -> bool {
    // Expand to full lines (end-exclusive selection already handled inside)
    let (line_start_b, line_end_b) =
        line_bounds_covering_selection(s, *sel_start_char, *sel_end_char);

    // Precompute all affected line start byte indices from the ORIGINAL string slice
    let mut line_starts: Vec<usize> = Vec::new();
    let mut cursor = line_start_b;
    line_starts.push(cursor);
    while cursor < line_end_b {
        if let Some(off) = s[cursor..line_end_b].find('\n') {
            cursor = cursor + off + 1; // start of next line within the bounded region
            if cursor <= line_end_b {
                line_starts.push(cursor);
            }
        } else {
            break;
        }
    }

    // Insert indent at each recorded line start, tracking cumulative shift
    let mut byte_shift = 0isize;
    for &orig_start in &line_starts {
        let idx = (orig_start as isize + byte_shift) as usize;
        s.insert_str(idx, indent);
        byte_shift += indent.len() as isize;
    }

    if !line_starts.is_empty() {
        // Compute new selection bounds in chars: start is original line_start_b, end is original line_end_b shifted by total inserted bytes
        let new_start_char = byte_to_char_idx(s, line_start_b); // start unchanged in bytes
        let new_end_char = byte_to_char_idx(s, (line_end_b as isize + byte_shift) as usize);
        *sel_start_char = new_start_char;
        *sel_end_char = new_end_char;
        return true;
    }
    false
}

fn unindent_selection(
    s: &mut String,
    indent: &str,
    sel_start_char: &mut usize,
    sel_end_char: &mut usize,
) -> bool {
    let (line_start_b, line_end_b) =
        line_bounds_covering_selection(s, *sel_start_char, *sel_end_char);

    // Precompute affected line starts from the ORIGINAL slice
    let mut line_starts: Vec<usize> = Vec::new();
    let mut cursor = line_start_b;
    line_starts.push(cursor);
    while cursor < line_end_b {
        if let Some(off) = s[cursor..line_end_b].find('\n') {
            cursor = cursor + off + 1; // start of next line within region
            if cursor <= line_end_b {
                line_starts.push(cursor);
            }
        } else {
            break;
        }
    }

    // For each line, remove up to one indent unit (or partial spaces)
    let mut byte_shift_total: isize = 0;
    for &orig_start in &line_starts {
        let mut idx = (orig_start as isize + byte_shift_total) as usize;
        // Safety clamp
        if idx > s.len() {
            idx = s.len();
        }
        let remaining = &s[idx..];
        if remaining.starts_with(indent) {
            s.drain(idx..idx + indent.len());
            byte_shift_total -= indent.len() as isize;
        } else {
            let mut removed = 0usize;
            for _ch in remaining.chars().take_while(|c| *c == ' ') {
                removed += 1;
                if removed == indent.len() {
                    break;
                }
            }
            if removed > 0 {
                s.drain(idx..idx + removed);
                byte_shift_total -= removed as isize;
            }
        }
    }

    if byte_shift_total != 0 {
        let new_start_char = byte_to_char_idx(s, line_start_b);
        let new_end_char = byte_to_char_idx(s, (line_end_b as isize + byte_shift_total) as usize);
        *sel_start_char = new_start_char;
        *sel_end_char = new_end_char;
        return true;
    }
    false
}
