//! The `Module` trait, the `HostRegistrar` it registers through, and the
//! by-value descriptors a module contributes.
//!
//! A module is the only path a capability enters the shell (bible rule:
//! capabilities are registered by internal modules through registries, no
//! external dynamic plugins). It registers through [`HostRegistrar`], a `dyn`
//! trait, so a module never sees the concrete `Registries`.

use crate::contrib_api::ids::ActionId;
use crate::contrib_api::panel::Panel;
use crate::contrib_api::tool::Tool;
use crate::contrib_api::workspace::Workspace;

/// The registration front door handed to each [`Module`].
///
/// # Object safety
///
/// `&mut self`, no generics, no `-> Self`: dyn-compatible, passed as
/// `&mut dyn HostRegistrar` so the module is decoupled from the concrete
/// registry storage.
pub trait HostRegistrar {
    /// Register a panel; its key is `panel.id()`.
    fn add_panel(&mut self, panel: Box<dyn Panel>);
    /// Register a tool; its key is `tool.id()`.
    fn add_tool(&mut self, tool: Box<dyn Tool>);
    /// Register a workspace; its key is `ws.id()`.
    fn add_workspace(&mut self, ws: Box<dyn Workspace>);
    /// Register an action (a menu item / command-palette entry).
    fn add_action(&mut self, action: ActionDesc);
    /// Contribute a top-bar menu group (Sprite/Layer/Frame menus, ...).
    fn add_menu_group(&mut self, group: MenuGroup);
    // add_importer/exporter/provider/validator land with their registries later.
}

/// A registerable action: id, label, icon, and command-palette visibility.
pub struct ActionDesc {
    /// Stable id - also the action registry key.
    pub id: ActionId,
    /// Display label.
    pub label: &'static str,
    /// Phosphor glyph from `crate::icons`.
    pub icon: char,
    /// Whether this action appears in the Ctrl/Cmd+K command palette.
    pub palette_visible: bool,
}

/// A top-bar menu group, e.g. "Sprite" with its items.
pub struct MenuGroup {
    /// The menu button label, e.g. "Sprite".
    pub label: &'static str,
    /// The items under this group, rendered top to bottom.
    pub items: Vec<MenuItem>,
}

/// A single menu item: a label, an optional accelerator, and the action it fires.
pub struct MenuItem {
    /// Display label, e.g. "New".
    pub label: &'static str,
    /// Optional accelerator shown beside the label; `None` means no shortcut.
    pub shortcut: Option<egui::KeyboardShortcut>,
    /// The action dispatched when the item is clicked.
    pub action: ActionId,
}

/// A compiled-in capability bundle: a workspace and its panels/tools/menus.
///
/// # Object safety
///
/// `&self`, no generics, no `-> Self`: dyn-compatible, boxed in `app`'s module
/// list. `register` is the only path a module's capabilities enter the shell.
pub trait Module {
    /// The module's stable id, e.g. "sprite-edit".
    fn id(&self) -> &'static str;

    /// Register every capability this module contributes, through `host`.
    fn register(&self, host: &mut dyn HostRegistrar);
}
