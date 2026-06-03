//! Shared egui-drawing helpers for the Pixhaus shell.
//!
//! These are presentation primitives, nothing more: a card frame, the rail tool
//! button, workspace and tray tabs, a section header, and the placeholder mocks
//! that stand in for real panel content this round. They paint with theme tokens
//! only - never a hex literal - so a theme-variant swap recolors them for free.
//!
//! Concrete `Panel`/`Tool`/`Workspace` impls do NOT belong here; they live in the
//! `modules/*` crates. This module is shared chrome the regions and panels call.

mod busy;
mod card;
mod placeholder;
mod section_header;
mod tool_button;
mod tray_tab;
mod workspace_tab;

pub use busy::busy_indicator;
pub use card::card;
pub use placeholder::{mock_log, mock_row, mock_thumbnail_grid};
pub use section_header::section_header;
pub use tool_button::tool_button;
pub use tray_tab::tray_tab;
pub use workspace_tab::workspace_tab;
