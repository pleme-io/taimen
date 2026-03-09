//! GPU TUI rendering architecture for Taimen.
//!
//! # Overview
//!
//! The Taimen client renders directly to the terminal using GPU-accelerated
//! drawing provided by the pleme-io graphics stack.  This module documents the
//! rendering architecture; actual widget implementations live in dedicated
//! sub-modules (not yet extracted).
//!
//! # pleme-io crate roles
//!
//! | Crate      | Role                                                   |
//! |------------|--------------------------------------------------------|
//! | **madori** | Application framework (event loop, lifecycle, layout)  |
//! | **garasu** | GPU rendering backend (wgpu abstraction for the TUI)   |
//! | **egaku** | Widget library (grids, sidebars, buttons, controls)    |
//! | **irodzuki** | Theming engine (colour palettes, font weights, spacing) |
//!
//! # Layout
//!
//! ```text
//! +-------------------------------------------------------+
//! |                   Participant Grid                     |
//! |  +----------+  +----------+  +----------+             |
//! |  |  Video 1 |  |  Video 2 |  |  Video 3 |   ...      |
//! |  | (name)   |  | (name)   |  | (name)   |             |
//! |  +----------+  +----------+  +----------+             |
//! |                                                       |
//! +-------------------------------------------+-----------+
//! |              Controls Bar                  |  Chat     |
//! |  [Mic] [Cam] [Share] [Hand] [Leave]       |  Sidebar  |
//! +-------------------------------------------+-----------+
//! ```
//!
//! ## Participant grid
//!
//! Rendered by `egaku::Grid`.  Each cell decodes a video frame from the
//! corresponding WebRTC track and blits it onto a GPU texture.  The grid
//! auto-reflows based on participant count (1 = full screen, 2 = side-by-side,
//! 3-4 = 2x2, etc.).
//!
//! ## Controls bar
//!
//! A horizontal `egaku::Bar` with icon buttons for microphone, camera,
//! screen-share, hand-raise, and leave-room.  Keyboard shortcuts are bound
//! through `madori::Keybindings`.
//!
//! ## Chat sidebar
//!
//! An `egaku::Sidebar` that displays in-room chat messages and an input field.
//! Can be toggled open/closed with a keybinding.
//!
//! # Theming
//!
//! `irodzuki` provides a `Theme` that controls colours, spacing, border
//! radius, and font weights across the entire UI.  Taimen ships a default dark
//! theme; users can override it via `TaimenConfig::theme`.

/// Placeholder constant for the default grid column count before auto-reflow
/// takes over.
pub const DEFAULT_GRID_COLUMNS: u32 = 3;

/// Placeholder constant for the controls-bar height in terminal rows.
pub const CONTROLS_BAR_HEIGHT: u32 = 3;

/// Placeholder constant for the chat sidebar width in terminal columns.
pub const CHAT_SIDEBAR_WIDTH: u32 = 40;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn layout_constants_are_positive() {
        assert!(DEFAULT_GRID_COLUMNS > 0);
        assert!(CONTROLS_BAR_HEIGHT > 0);
        assert!(CHAT_SIDEBAR_WIDTH > 0);
    }
}
