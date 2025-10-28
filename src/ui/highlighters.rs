//! Syntax highlighters for JavaScript and JSON code editors.
//!
//! This module provides syntax highlighting functionality for code displayed
//! in the properties panel, making it easier to read and edit scripts and templates.

use eframe::egui::{self, Color32};
use eframe::epaint::text::{LayoutJob, TextFormat};

/// Highlights JavaScript code with syntax coloring.
///
/// # Arguments
///
/// * `text` - The JavaScript source code to highlight
/// * `font_id` - The font to use for rendering
///
/// # Returns
///
/// A `LayoutJob` containing the highlighted text with appropriate colors
pub fn highlight_javascript(text: &str, font_id: egui::FontId, dark_mode: bool) -> LayoutJob {
    let mut job = LayoutJob::default();

    // Define colors for different token types, adjusting for dark/light mode
    let (keyword_color, string_color, comment_color, number_color, function_color, default_color) =
        if dark_mode {
            (
                Color32::from_rgb(86, 156, 214),  // Blue
                Color32::from_rgb(206, 145, 120), // Orange
                Color32::from_rgb(106, 153, 85),  // Green
                Color32::from_rgb(181, 206, 168), // Light green
                Color32::from_rgb(220, 220, 170), // Yellow
                Color32::from_rgb(212, 212, 212), // Light gray (default)
            )
        } else {
            (
                Color32::from_rgb(0, 0, 170),   // Dark blue for keywords
                Color32::from_rgb(163, 21, 21), // Dark red/brown for strings
                Color32::from_rgb(0, 128, 0),   // Dark green for comments
                Color32::from_rgb(100, 0, 150), // Purple for numbers
                Color32::from_rgb(0, 102, 153), // Dark teal for function names
                Color32::BLACK,                 // Black default text on light bg
            )
        };

    let keywords = [
        "function",
        "return",
        "if",
        "else",
        "for",
        "while",
        "do",
        "switch",
        "case",
        "break",
        "continue",
        "var",
        "let",
        "const",
        "new",
        "this",
        "typeof",
        "null",
        "undefined",
        "true",
        "false",
        "in",
        "of",
        "try",
        "catch",
        "finally",
        "throw",
        "class",
        "extends",
        "super",
        "static",
        "async",
        "await",
        "yield",
        "import",
        "export",
        "default",
        "from",
        "as",
    ];

    let mut chars = text.char_indices().peekable();

    while let Some((i, c)) = chars.next() {
        // Check for comments
        if c == '/' {
            if let Some(&(_, next_c)) = chars.peek() {
                if next_c == '/' {
                    // Single-line comment
                    let start = i;
                    chars.next(); // consume second '/'
                    while let Some(&(_, ch)) = chars.peek() {
                        if ch == '\n' {
                            break;
                        }
                        chars.next();
                    }
                    let end = chars.peek().map(|&(idx, _)| idx).unwrap_or(text.len());
                    job.append(
                        &text[start..end],
                        0.0,
                        TextFormat::simple(font_id.clone(), comment_color),
                    );
                    continue;
                } else if next_c == '*' {
                    // Multi-line comment
                    let start = i;
                    chars.next(); // consume '*'
                    let mut found_end = false;
                    while let Some((_, ch)) = chars.next() {
                        if ch == '*' {
                            if let Some(&(_, '/')) = chars.peek() {
                                chars.next(); // consume '/'
                                found_end = true;
                                break;
                            }
                        }
                    }
                    let end = if found_end {
                        chars.peek().map(|&(idx, _)| idx).unwrap_or(text.len())
                    } else {
                        text.len()
                    };
                    job.append(
                        &text[start..end],
                        0.0,
                        TextFormat::simple(font_id.clone(), comment_color),
                    );
                    continue;
                }
            }
        }

        // Check for strings
        if c == '"' || c == '\'' || c == '`' {
            let quote = c;
            let start = i;
            let mut escaped = false;

            for (_, ch) in chars.by_ref() {
                if escaped {
                    escaped = false;
                    continue;
                }
                if ch == '\\' {
                    escaped = true;
                    continue;
                }
                if ch == quote {
                    break;
                }
            }

            let end = chars.peek().map(|&(idx, _)| idx).unwrap_or(text.len());
            job.append(
                &text[start..end],
                0.0,
                TextFormat::simple(font_id.clone(), string_color),
            );
            continue;
        }

        // Check for numbers
        if c.is_ascii_digit() {
            let start = i;
            while let Some(&(_, ch)) = chars.peek() {
                if ch.is_ascii_alphanumeric() || ch == '.' || ch == '_' {
                    chars.next();
                } else {
                    break;
                }
            }
            let end = chars.peek().map(|&(idx, _)| idx).unwrap_or(text.len());
            job.append(
                &text[start..end],
                0.0,
                TextFormat::simple(font_id.clone(), number_color),
            );
            continue;
        }

        // Check for identifiers (keywords or function names)
        if c.is_alphabetic() || c == '_' || c == '$' {
            let start = i;
            while let Some(&(_, ch)) = chars.peek() {
                if ch.is_alphanumeric() || ch == '_' || ch == '$' {
                    chars.next();
                } else {
                    break;
                }
            }
            let end = chars.peek().map(|&(idx, _)| idx).unwrap_or(text.len());
            let word = &text[start..end];

            let color = if keywords.contains(&word) {
                keyword_color
            } else if chars.peek().map(|&(_, ch)| ch == '(').unwrap_or(false) {
                function_color
            } else {
                default_color
            };

            job.append(word, 0.0, TextFormat::simple(font_id.clone(), color));
            continue;
        }

        // Default: just add the character
        job.append(
            &text[i..i + c.len_utf8()],
            0.0,
            TextFormat::simple(font_id.clone(), default_color),
        );
    }

    job
}

/// Highlights JSON code with syntax coloring.
///
/// # Arguments
///
/// * `text` - The JSON source code to highlight
/// * `font_id` - The font to use for rendering
///
/// # Returns
///
/// A `LayoutJob` containing the highlighted text with appropriate colors
pub fn highlight_json(text: &str, font_id: egui::FontId, dark_mode: bool) -> LayoutJob {
    let mut job = LayoutJob::default();

    // Define colors for different token types, adjusting for dark/light mode
    let (string_color, number_color, keyword_color, key_color, default_color) = if dark_mode {
        (
            Color32::from_rgb(206, 145, 120), // Orange
            Color32::from_rgb(181, 206, 168), // Light green
            Color32::from_rgb(86, 156, 214),  // Blue (for true/false/null)
            Color32::from_rgb(156, 220, 254), // Light blue (for object keys)
            Color32::from_rgb(212, 212, 212), // Light gray
        )
    } else {
        (
            Color32::from_rgb(163, 21, 21), // Dark red/brown for strings
            Color32::from_rgb(100, 0, 150), // Purple for numbers
            Color32::from_rgb(0, 0, 170),   // Dark blue for true/false/null
            Color32::from_rgb(0, 102, 204), // Strong blue for object keys
            Color32::BLACK,                 // Black default text
        )
    };

    let mut chars = text.char_indices().peekable();
    let mut in_key_position = false; // Track if we're expecting an object key

    while let Some((i, c)) = chars.next() {
        // Check for strings
        if c == '"' {
            let start = i;
            let mut escaped = false;

            for (_, ch) in chars.by_ref() {
                if escaped {
                    escaped = false;
                    continue;
                }
                if ch == '\\' {
                    escaped = true;
                    continue;
                }
                if ch == '"' {
                    break;
                }
            }

            let end = chars.peek().map(|&(idx, _)| idx).unwrap_or(text.len());
            let string_text = &text[start..end];

            // Check if this is an object key (followed by ':')
            let mut peek_chars = text[end..].chars();
            let is_key = loop {
                match peek_chars.next() {
                    Some(ch) if ch.is_whitespace() => continue,
                    Some(':') => break true,
                    _ => break false,
                }
            };

            let color = if is_key || in_key_position {
                in_key_position = false;
                key_color
            } else {
                string_color
            };

            job.append(string_text, 0.0, TextFormat::simple(font_id.clone(), color));
            continue;
        }

        // Check for numbers (including negative)
        if c.is_ascii_digit()
            || (c == '-'
                && chars
                    .peek()
                    .map(|&(_, ch)| ch.is_ascii_digit())
                    .unwrap_or(false))
        {
            let start = i;
            if c == '-' {
                chars.next(); // consume the digit after '-'
            }
            while let Some(&(_, ch)) = chars.peek() {
                if ch.is_ascii_digit()
                    || ch == '.'
                    || ch == 'e'
                    || ch == 'E'
                    || ch == '+'
                    || ch == '-'
                {
                    chars.next();
                } else {
                    break;
                }
            }
            let end = chars.peek().map(|&(idx, _)| idx).unwrap_or(text.len());
            job.append(
                &text[start..end],
                0.0,
                TextFormat::simple(font_id.clone(), number_color),
            );
            continue;
        }

        // Check for keywords (true, false, null)
        if c.is_alphabetic() {
            let start = i;
            while let Some(&(_, ch)) = chars.peek() {
                if ch.is_alphanumeric() {
                    chars.next();
                } else {
                    break;
                }
            }
            let end = chars.peek().map(|&(idx, _)| idx).unwrap_or(text.len());
            let word = &text[start..end];

            let color = if word == "true" || word == "false" || word == "null" {
                keyword_color
            } else {
                default_color
            };

            job.append(word, 0.0, TextFormat::simple(font_id.clone(), color));
            continue;
        }

        // Track structural characters that indicate we're expecting a key next
        if c == '{' || c == ',' {
            in_key_position = true;
        }

        // Default: just add the character
        job.append(
            &text[i..i + c.len_utf8()],
            0.0,
            TextFormat::simple(font_id.clone(), default_color),
        );
    }

    job
}
