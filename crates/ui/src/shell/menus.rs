//! The shell's always-present top-bar menu groups, as data. Body lands in SHELL.13.

use crate::contrib_api::module::MenuGroup;

/// The menu groups the shell owns (File/Edit/View/Window/Help and the empty
/// module-contributed slots). Body lands in SHELL.13.
pub fn shell_menu_groups() -> Vec<MenuGroup> {
    Vec::new()
}
