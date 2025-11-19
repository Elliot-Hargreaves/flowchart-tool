//! Shared application-wide constants.
//! Centralizes tweakable values used across UI rendering and interactions.

// Group rendering
/// Padding (in world units) added around the union of member nodes when computing a group's rect.
pub const GROUP_PADDING: f32 = 25.0;
/// Corner radius for group rectangles (in screen pixels after transform).
pub const GROUP_CORNER_RADIUS: f32 = 8.0;
/// Stroke width for group rectangle outlines (in screen pixels).
pub const GROUP_STROKE_WIDTH: f32 = 1.5;
/// Base padding for positioning the group label inside the rect. Scaled by zoom.
pub const GROUP_LABEL_PADDING_BASE: f32 = 6.0;

// Node dimensions
/// Default node width in world units.
pub const NODE_WIDTH: f32 = 100.0;
/// Default node height in world units.
pub const NODE_HEIGHT: f32 = 70.0;

// Grid/drawing
/// Grid cell size in world units.
pub const GRID_SIZE: f32 = 20.0;
/// Number of grid cells between thicker grid lines.
pub const GRID_WIDTH: usize = 5;
/// Spacing between minor grid dots (in world units, used for dot-style grids).
pub const DOT_SPACING: f32 = 8.0;
/// Radius of minor grid dots (in screen pixels).
pub const DOT_RADIUS: f32 = 3.0;

// Canvas interactions
/// Click threshold in world units used for distinguishing click vs drag.
pub const CLICK_THRESHOLD: f32 = 10.0;

// Undo/redo
/// Maximum number of undo history entries to retain.
pub const MAX_UNDO_HISTORY: usize = 100;
