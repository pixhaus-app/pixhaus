//! The shell's always-present top-bar menu groups, as data.
//!
//! The shell owns Pixhaus/File/Edit/Select/View/Window/Help; modules contribute
//! Sprite/Layer/Frame into `Registries.menus`. Together they make the spec's menu
//! set (File Edit Sprite Layer Frame Select View Window Help) - Select is a
//! selection-level concern the shell owns, not a module one. Most items emit a
//! mock `Intent::RunAction`; the View/Window items named below are wired live in
//! the top-bar render (SHELL.4): View > Theme, View > Toggle Grid, Window >
//! Command Palette.

use crate::contrib_api::ids::{ActionId, MsgKey};
use crate::contrib_api::module::{HostRegistrar, MenuGroup, MenuItem};

// Stable action ids the top-bar render special-cases. Inert items carry their own
// "<group>.<verb>" ids and route to the mock RunAction toast.
/// `Pixhaus > About Pixhaus`; the render maps it to `Intent::OpenAbout`.
pub const ACTION_PIXHAUS_ABOUT: ActionId = ActionId("pixhaus.about");
/// `View > Theme` submenu root; the render expands it to Dark/Light/Accent.
pub const ACTION_VIEW_THEME: ActionId = ActionId("view.theme");
/// `View > Toggle Grid`; the render maps it to `Intent::SetGrid`.
pub const ACTION_VIEW_TOGGLE_GRID: ActionId = ActionId("view.toggle_grid");
/// `Window > Command Palette`; the render maps it to `Intent::OpenCommandPalette`.
pub const ACTION_WINDOW_COMMAND_PALETTE: ActionId = ActionId("window.command_palette");

/// The shell's canonical menu-bar order, left to right.
///
/// The registry stores groups in contribution order (shell groups, then each
/// module's groups as it registers), which is not the order the bar should read.
/// The shell - not the modules - decides display order: the render walks this
/// sequence, drawing each registered group whose label matches, then appends any
/// group whose label is not listed (so a future module's group still shows up).
pub const CANONICAL_MENU_ORDER: [MsgKey; 10] = [
    MsgKey("app.menu.pixhaus"),
    MsgKey("app.menu.file"),
    MsgKey("app.menu.edit"),
    MsgKey("app.menu.sprite"),
    MsgKey("app.menu.layer"),
    MsgKey("app.menu.frame"),
    MsgKey("app.menu.select"),
    MsgKey("app.menu.view"),
    MsgKey("app.menu.window"),
    MsgKey("app.menu.help"),
];

/// Order the registered menu groups for display: canonical keys first in their
/// fixed order, then any unlisted group appended in registration order.
///
/// Matching is on the group's stable `MsgKey` identity, not its display text -
/// so the bar stays in order regardless of the active language. Borrows from
/// `groups`; the shell owns the order here, modules keep contributing through
/// `add_menu_group` unchanged.
pub fn ordered_menu_groups(groups: &[MenuGroup]) -> Vec<&MenuGroup> {
    let mut ordered: Vec<&MenuGroup> = CANONICAL_MENU_ORDER.iter().filter_map(|key| groups.iter().find(|g| g.label == *key)).collect();
    ordered.extend(groups.iter().filter(|g| !CANONICAL_MENU_ORDER.contains(&g.label)));
    ordered
}

fn item(label: MsgKey, action: ActionId) -> MenuItem {
    MenuItem { label, shortcut: None, action }
}

/// The menu groups the shell owns, in display order. Labels are localization
/// keys (`app.menu.*` for groups, `command.*` for items); the shell resolves them
/// at render time.
pub fn shell_menu_groups() -> Vec<MenuGroup> {
    vec![
        MenuGroup {
            label: MsgKey("app.menu.pixhaus"),
            items: vec![
                item(MsgKey("app.menu.pixhaus.about"), ACTION_PIXHAUS_ABOUT),
                item(MsgKey("app.menu.pixhaus.preferences"), ActionId("pixhaus.preferences")),
            ],
        },
        MenuGroup {
            label: MsgKey("app.menu.file"),
            items: vec![
                item(MsgKey("command.file.new"), ActionId("file.new")),
                item(MsgKey("command.file.open"), ActionId("file.open")),
                item(MsgKey("command.file.save"), ActionId("file.save")),
                item(MsgKey("command.file.export"), ActionId("file.export")),
            ],
        },
        MenuGroup {
            label: MsgKey("app.menu.edit"),
            items: vec![
                item(MsgKey("command.edit.undo"), ActionId("edit.undo")),
                item(MsgKey("command.edit.redo"), ActionId("edit.redo")),
                item(MsgKey("command.edit.cut"), ActionId("edit.cut")),
                item(MsgKey("command.edit.copy"), ActionId("edit.copy")),
                item(MsgKey("command.edit.paste"), ActionId("edit.paste")),
            ],
        },
        MenuGroup {
            label: MsgKey("app.menu.select"),
            items: vec![
                item(MsgKey("command.select.all"), ActionId("select.all")),
                item(MsgKey("command.select.none"), ActionId("select.none")),
                item(MsgKey("command.select.inverse"), ActionId("select.inverse")),
            ],
        },
        MenuGroup {
            label: MsgKey("app.menu.view"),
            items: vec![
                item(MsgKey("command.view.theme"), ACTION_VIEW_THEME),
                item(MsgKey("command.view.toggle_grid"), ACTION_VIEW_TOGGLE_GRID),
                item(MsgKey("command.view.zoom_in"), ActionId("view.zoom_in")),
                item(MsgKey("command.view.zoom_out"), ActionId("view.zoom_out")),
            ],
        },
        MenuGroup {
            label: MsgKey("app.menu.window"),
            items: vec![
                item(MsgKey("command.window.command_palette"), ACTION_WINDOW_COMMAND_PALETTE),
                item(MsgKey("command.window.reset_layout"), ActionId("window.reset_layout")),
            ],
        },
        MenuGroup {
            label: MsgKey("app.menu.help"),
            items: vec![
                item(MsgKey("command.help.docs"), ActionId("help.docs")),
                item(MsgKey("command.help.shortcuts"), ActionId("help.shortcuts")),
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
    // so groups are looked up with a let-else + `panic!`. Groups are keyed by their
    // stable `MsgKey` identity now, not display text.
    fn group(groups: &[MenuGroup], key: MsgKey) -> &MenuGroup {
        let Some(g) = groups.iter().find(|g| g.label == key) else {
            panic!("missing menu group: {key:?}");
        };
        g
    }

    #[test]
    fn shell_groups_in_order() {
        let groups = shell_menu_groups();
        let labels: Vec<MsgKey> = groups.iter().map(|g| g.label).collect();
        // The shell owns these; Sprite/Layer/Frame are module-contributed.
        assert_eq!(
            labels,
            vec![
                MsgKey("app.menu.pixhaus"),
                MsgKey("app.menu.file"),
                MsgKey("app.menu.edit"),
                MsgKey("app.menu.select"),
                MsgKey("app.menu.view"),
                MsgKey("app.menu.window"),
                MsgKey("app.menu.help"),
            ]
        );
    }

    #[test]
    fn select_menu_has_the_selection_items() {
        let groups = shell_menu_groups();
        let select = group(&groups, MsgKey("app.menu.select"));
        let item_labels: Vec<MsgKey> = select.items.iter().map(|i| i.label).collect();
        assert_eq!(
            item_labels,
            vec![MsgKey("command.select.all"), MsgKey("command.select.none"), MsgKey("command.select.inverse"),]
        );
    }

    #[test]
    fn view_menu_has_live_theme_and_grid_items() {
        let groups = shell_menu_groups();
        let view = group(&groups, MsgKey("app.menu.view"));
        // The Theme and Toggle Grid items are special-cased by stable action id.
        assert!(view.items.iter().any(|i| i.action == ACTION_VIEW_THEME));
        assert!(view.items.iter().any(|i| i.action == ACTION_VIEW_TOGGLE_GRID));
    }

    #[test]
    fn window_menu_exposes_command_palette() {
        let groups = shell_menu_groups();
        let window = group(&groups, MsgKey("app.menu.window"));
        assert!(window.items.iter().any(|i| i.action == ACTION_WINDOW_COMMAND_PALETTE));
    }

    #[test]
    fn register_shell_menus_populates_the_registry() {
        let mut host = crate::state::Host::new(&crate::theme::Theme::dark());
        register_shell_menus(&mut host.registrar());
        let labels: Vec<MsgKey> = host.registries.menus.iter().map(|g| g.label).collect();
        assert!(labels.contains(&MsgKey("app.menu.file")));
        assert!(labels.contains(&MsgKey("app.menu.view")));
        assert!(labels.contains(&MsgKey("app.menu.window")));
    }

    fn named(label: MsgKey) -> MenuGroup {
        MenuGroup { label, items: vec![] }
    }

    /// Display order is the canonical sequence, not registration order. Modules
    /// contribute Sprite/Layer/Frame after the shell's groups (the registration
    /// order), but the bar must read Pixhaus File Edit Sprite Layer Frame Select
    /// View Window Help.
    #[test]
    fn ordered_menu_groups_follows_the_canonical_sequence() {
        // Registration order: shell groups first, then the module groups appended,
        // exactly as `build_host` produces.
        let mut groups = shell_menu_groups();
        groups.push(named(MsgKey("app.menu.sprite")));
        groups.push(named(MsgKey("app.menu.layer")));
        groups.push(named(MsgKey("app.menu.frame")));

        let ordered: Vec<MsgKey> = ordered_menu_groups(&groups).iter().map(|g| g.label).collect();
        assert_eq!(ordered, CANONICAL_MENU_ORDER.to_vec());
    }

    /// A group whose label is not in the canonical list still renders - appended
    /// after the known groups so a future module's menu is never silently dropped.
    #[test]
    fn ordered_menu_groups_appends_unknown_groups() {
        let groups = vec![named(MsgKey("app.menu.help")), named(MsgKey("mystery")), named(MsgKey("app.menu.file"))];
        let ordered: Vec<MsgKey> = ordered_menu_groups(&groups).iter().map(|g| g.label).collect();
        // File and Help take their canonical slots; mystery falls to the end.
        assert_eq!(ordered, vec![MsgKey("app.menu.file"), MsgKey("app.menu.help"), MsgKey("mystery")]);
    }
}
