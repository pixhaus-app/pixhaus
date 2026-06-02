//! The shell's always-present top-bar menu groups, as data.
//!
//! The shell owns Pixhaus/File/Edit/Select/View/Window/Help; modules contribute
//! Sprite/Layer/Frame into `Registries.menus`. Together they make the spec's menu
//! set (File Edit Sprite Layer Frame Select View Window Help) - Select is a
//! selection-level concern the shell owns, not a module one. Most items emit a
//! mock `Intent::RunAction`; the View/Window items named below are wired live in
//! the top-bar render (SHELL.4): View > Theme, View > Toggle Grid, Window >
//! Command Palette.

use crate::contrib_api::ids::ActionId;
use crate::contrib_api::module::{HostRegistrar, MenuGroup, MenuItem};

// Stable action ids the top-bar render special-cases. Inert items carry their own
// "<group>.<verb>" ids and route to the mock RunAction toast.
/// `View > Theme` submenu root; the render expands it to Dark/Light/Accent.
pub const ACTION_VIEW_THEME: ActionId = ActionId("view.theme");
/// `View > Toggle Grid`; the render maps it to `Intent::SetGrid`.
pub const ACTION_VIEW_TOGGLE_GRID: ActionId = ActionId("view.toggle_grid");
/// `Window > Command Palette`; the render maps it to `Intent::OpenCommandPalette`.
pub const ACTION_WINDOW_COMMAND_PALETTE: ActionId = ActionId("window.command_palette");

fn item(label: &'static str, action: ActionId) -> MenuItem {
    MenuItem { label, shortcut: None, action }
}

/// The menu groups the shell owns, in display order.
pub fn shell_menu_groups() -> Vec<MenuGroup> {
    vec![
        MenuGroup {
            label: "Pixhaus",
            items: vec![
                item("About Pixhaus", ActionId("pixhaus.about")),
                item("Preferences", ActionId("pixhaus.preferences")),
            ],
        },
        MenuGroup {
            label: "File",
            items: vec![
                item("New", ActionId("file.new")),
                item("Open", ActionId("file.open")),
                item("Save", ActionId("file.save")),
                item("Export", ActionId("file.export")),
            ],
        },
        MenuGroup {
            label: "Edit",
            items: vec![
                item("Undo", ActionId("edit.undo")),
                item("Redo", ActionId("edit.redo")),
                item("Cut", ActionId("edit.cut")),
                item("Copy", ActionId("edit.copy")),
                item("Paste", ActionId("edit.paste")),
            ],
        },
        MenuGroup {
            label: "Select",
            items: vec![
                item("Select All", ActionId("select.all")),
                item("Deselect", ActionId("select.none")),
                item("Inverse", ActionId("select.inverse")),
            ],
        },
        MenuGroup {
            label: "View",
            items: vec![
                item("Theme", ACTION_VIEW_THEME),
                item("Toggle Grid", ACTION_VIEW_TOGGLE_GRID),
                item("Zoom In", ActionId("view.zoom_in")),
                item("Zoom Out", ActionId("view.zoom_out")),
            ],
        },
        MenuGroup {
            label: "Window",
            items: vec![
                item("Command Palette", ACTION_WINDOW_COMMAND_PALETTE),
                item("Reset Layout", ActionId("window.reset_layout")),
            ],
        },
        MenuGroup {
            label: "Help",
            items: vec![
                item("Documentation", ActionId("help.docs")),
                item("Keyboard Shortcuts", ActionId("help.shortcuts")),
            ],
        },
    ]
}

/// Register the shell's always-present menu groups through the registrar.
///
/// Called from `build_host` so the always-present groups enter `Registries.menus`
/// by the same path modules use for their Sprite/Layer/Frame/Select groups. Order:
/// call this first so module groups append after the shell's File/Edit/View block.
pub fn register_shell_menus(host: &mut dyn HostRegistrar) {
    for group in shell_menu_groups() {
        host.add_menu_group(group);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // The repo bans `unwrap`/`expect` via clippy disallowed-methods even in tests,
    // so groups are looked up with a let-else + `panic!`.
    fn group<'a>(groups: &'a [MenuGroup], label: &str) -> &'a MenuGroup {
        let Some(g) = groups.iter().find(|g| g.label == label) else {
            panic!("missing menu group: {label}");
        };
        g
    }

    #[test]
    fn shell_groups_in_order() {
        let groups = shell_menu_groups();
        let labels: Vec<&str> = groups.iter().map(|g| g.label).collect();
        // The shell owns these; Sprite/Layer/Frame are module-contributed.
        assert_eq!(labels, vec!["Pixhaus", "File", "Edit", "Select", "View", "Window", "Help"]);
    }

    #[test]
    fn select_menu_has_the_selection_items() {
        let groups = shell_menu_groups();
        let select = group(&groups, "Select");
        let item_labels: Vec<&str> = select.items.iter().map(|i| i.label).collect();
        assert_eq!(item_labels, vec!["Select All", "Deselect", "Inverse"]);
    }

    #[test]
    fn view_menu_has_live_theme_and_grid_items() {
        let groups = shell_menu_groups();
        let view = group(&groups, "View");
        let item_labels: Vec<&str> = view.items.iter().map(|i| i.label).collect();
        assert!(item_labels.contains(&"Toggle Grid"));
        assert!(item_labels.iter().any(|l| l.starts_with("Theme")));
    }

    #[test]
    fn window_menu_exposes_command_palette() {
        let groups = shell_menu_groups();
        let window = group(&groups, "Window");
        assert!(window.items.iter().any(|i| i.label.contains("Command Palette")));
    }

    #[test]
    fn register_shell_menus_populates_the_registry() {
        let mut host = crate::state::Host::new(&crate::theme::Theme::dark());
        register_shell_menus(&mut host.registrar());
        let labels: Vec<&str> = host.registries.menus.iter().map(|g| g.label).collect();
        assert!(labels.contains(&"File"));
        assert!(labels.contains(&"View"));
        assert!(labels.contains(&"Window"));
    }
}
