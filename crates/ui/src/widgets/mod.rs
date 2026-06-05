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
mod codex;
mod layout;
mod placeholder;
mod section_header;
mod tool_button;
mod tray_tab;
mod workspace_tab;

pub use busy::busy_indicator;
pub use card::card;
pub use codex::{
    ColorRowAction, DetailTab, EntryCardData, FolderRowAction, HoverAnchor, HoverEntry, ListAction, NavNodeData, NavNodeResponse, SlotEditAction,
    add_anchor_picker, anchor_badge, asset_thumbnail_strip, chip, count_pill, coverage_checklist, coverage_slot_row, detail_tab_bar, editable_list, entry_card,
    folder_row, handle_chip, history_timeline_row, hover_card, info_card, info_strip, nav_tree_node, palette_color_row, quick_link_chip, ratio_color,
    reference_chip, resolve_coverage_label, score_ring, slot_editor_row, stat_row, status_badge, status_check_row, status_color, strength_color,
    strength_selector, type_chip, type_color, type_icon, used_in_row,
};
pub use layout::{THREE_COLUMN_MIN_WIDTH, TWO_COLUMN_MIN_WIDTH, column_count, distribute};
pub use placeholder::{mock_log, mock_row, mock_thumbnail_grid};
pub use section_header::section_header;
pub use tool_button::tool_button;
pub use tray_tab::tray_tab;
pub use workspace_tab::workspace_tab;
