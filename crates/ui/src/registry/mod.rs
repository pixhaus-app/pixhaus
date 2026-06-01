//! The capability registries and the dyn registrar a module registers through.
//!
//! [`Registry`] is an insertion-ordered keyed store (the order is the rail/tab
//! display order); [`Registries`] bundles the per-kind registries; a thin wrapper
//! over `&mut Registries` implements the [`HostRegistrar`](crate::contrib_api::HostRegistrar)
//! a [`Module`](crate::contrib_api::Module) sees. [`resolve_layout`] turns a
//! workspace's authored layout into a [`ResolvedLayout`] filtered to registered ids.

use std::collections::HashMap;
use std::hash::Hash;

use crate::contrib_api::{ActionDesc, ActionId, HostRegistrar, MenuGroup, Panel, PanelId, Tool, ToolId, Workspace, WorkspaceId};

mod resolve;

pub use resolve::{ResolvedLayout, resolve_layout};

/// An insertion-ordered, key-indexed capability store.
///
/// `items` keeps registration order, which is the display order for the tool
/// rail and tray tabs; `index` maps an id to its slot for O(1) `get`. A module
/// registering a duplicate id is a programming error, loud in debug (the
/// `debug_assert` in [`Registry::insert`]) and last-value-wins in release.
pub struct Registry<K: Copy + Eq + Hash, V> {
    items: Vec<V>,
    index: HashMap<K, usize>,
}

impl<K: Copy + Eq + Hash, V> Default for Registry<K, V> {
    fn default() -> Self {
        Self {
            items: Vec::new(),
            index: HashMap::new(),
        }
    }
}

impl<K: Copy + Eq + Hash, V> Registry<K, V> {
    /// Registers `value` under `key`, appending it in display order.
    ///
    /// A duplicate `key` is a programming error: it `debug_assert`-panics in debug
    /// and overwrites the existing slot (last value wins) in release.
    fn insert(&mut self, key: K, value: V) {
        debug_assert!(!self.index.contains_key(&key), "duplicate registry id");
        match self.index.get(&key).copied() {
            Some(i) => self.items[i] = value,
            None => {
                self.index.insert(key, self.items.len());
                self.items.push(value);
            }
        }
    }

    /// Returns the value registered under `key`, or `None` if absent.
    pub fn get(&self, key: K) -> Option<&V> {
        self.index.get(&key).map(|&i| &self.items[i])
    }

    /// Iterates the registered values in registration (display) order.
    pub fn iter(&self) -> impl Iterator<Item = &V> {
        self.items.iter()
    }
}

/// The panel registry: registered panels keyed by [`PanelId`].
pub type PanelRegistry = Registry<PanelId, Box<dyn Panel>>;
/// The tool registry: registered tools keyed by [`ToolId`].
pub type ToolRegistry = Registry<ToolId, Box<dyn Tool>>;
/// The workspace registry: registered workspaces keyed by [`WorkspaceId`].
pub type WorkspaceRegistry = Registry<WorkspaceId, Box<dyn Workspace>>;

/// The full set of capability registries a [`Module`](crate::contrib_api::Module)
/// contributes into and the shell reads each frame.
#[derive(Default)]
pub struct Registries {
    /// Registered panels, in registration order.
    pub panels: PanelRegistry,
    /// Registered tools, in registration order.
    pub tools: ToolRegistry,
    /// Registered workspaces, in registration order.
    pub workspaces: WorkspaceRegistry,
    /// Registered actions (palette / menu targets), in registration order.
    pub actions: Registry<ActionId, ActionDesc>,
    /// Top-bar menu groups, in contribution order.
    pub menus: Vec<MenuGroup>,
}

// A compile-time guard: if any registry trait regresses out of dyn-compatibility
// (a generic method, a `-> Self`, a non-`&self` receiver), this stops compiling.
const _: () = {
    fn _assert_boxable(_: Box<dyn Panel>, _: Box<dyn Tool>, _: Box<dyn Workspace>, _: Box<dyn crate::contrib_api::Module>) {}
};

impl Registries {
    /// Borrows these registries as the `dyn HostRegistrar` a module registers
    /// through. A module never sees the concrete `Registries`; it only adds
    /// capabilities, each keyed by its own `id()`.
    pub fn registrar(&mut self) -> RegistrarWrapper<'_> {
        RegistrarWrapper(self)
    }
}

/// A thin `&mut Registries` wrapper that implements [`HostRegistrar`].
///
/// Each `add_*` keys the value by its own `id()`, so a module cannot register a
/// capability under a key that disagrees with the capability's identity.
pub struct RegistrarWrapper<'a>(&'a mut Registries);

impl HostRegistrar for RegistrarWrapper<'_> {
    fn add_panel(&mut self, panel: Box<dyn Panel>) {
        let id = panel.id();
        self.0.panels.insert(id, panel);
    }
    fn add_tool(&mut self, tool: Box<dyn Tool>) {
        let id = tool.id();
        self.0.tools.insert(id, tool);
    }
    fn add_workspace(&mut self, ws: Box<dyn Workspace>) {
        let id = ws.id();
        self.0.workspaces.insert(id, ws);
    }
    fn add_action(&mut self, action: ActionDesc) {
        let id = action.id;
        self.0.actions.insert(id, action);
    }
    fn add_menu_group(&mut self, group: MenuGroup) {
        self.0.menus.push(group);
    }
}

#[cfg(test)]
pub(crate) mod fixtures {
    use crate::contrib_api::{Panel, PanelId, PanelMeta, PanelScope, Workspace, WorkspaceId, WorkspaceLayout, WorkspaceMeta};
    use crate::region::Region;

    /// A minimal panel for registry/resolve tests: it carries an id and a relevance
    /// answer, renders nothing, and depends on no module crate.
    pub struct FakePanel {
        pub id: PanelId,
        pub relevant: bool,
    }

    impl Panel for FakePanel {
        fn id(&self) -> PanelId {
            self.id
        }
        fn meta(&self) -> PanelMeta {
            PanelMeta {
                title: "fake",
                icon: ' ',
                default_region: Region::RightDock,
                default_open: true,
            }
        }
        fn relevant_in(&self, _workspace: WorkspaceId) -> bool {
            self.relevant
        }
        fn ui(&self, _ui: &mut egui::Ui, _scope: &mut PanelScope<'_>) {}
    }

    /// A minimal workspace for resolve tests: it returns a fixed authored layout.
    pub struct FakeWorkspace {
        pub id: WorkspaceId,
        pub layout: WorkspaceLayout,
    }

    impl Workspace for FakeWorkspace {
        fn id(&self) -> WorkspaceId {
            self.id
        }
        fn meta(&self) -> WorkspaceMeta {
            WorkspaceMeta {
                name: "Fake",
                icon: ' ',
                purpose: "test workspace",
                shortcut: egui::KeyboardShortcut::new(egui::Modifiers::COMMAND, egui::Key::Num1),
            }
        }
        fn layout(&self) -> WorkspaceLayout {
            self.layout.clone()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::Registry;
    use super::fixtures::{FakePanel, FakeWorkspace};
    use crate::contrib_api::{HostRegistrar, PanelId, ToolId, WorkspaceId, WorkspaceLayout};

    /// `insert` preserves insertion order in `iter()` and indexes by key for `get`.
    #[test]
    fn insert_keeps_order_and_indexes() {
        let mut reg: Registry<u32, &'static str> = Registry::default();
        reg.insert(10, "first");
        reg.insert(20, "second");
        reg.insert(30, "third");

        let order: Vec<&&str> = reg.iter().collect();
        assert_eq!(order, vec![&"first", &"second", &"third"]);
        assert_eq!(reg.get(20), Some(&"second"));
        assert_eq!(reg.get(99), None);
    }

    /// A duplicate id is a programming error: it must `debug_assert`-panic in debug.
    #[test]
    #[should_panic(expected = "duplicate registry id")]
    fn duplicate_id_panics_in_debug() {
        let mut reg: Registry<u32, &'static str> = Registry::default();
        reg.insert(1, "a");
        reg.insert(1, "b");
    }

    /// `Registries::registrar()` yields a `HostRegistrar`; adding a value keys it by
    /// the value's own `id()`, so the module never names a key twice.
    #[test]
    fn registrar_keys_by_value_id() {
        let mut registries = super::Registries::default();
        {
            let mut registrar = registries.registrar();
            registrar.add_panel(Box::new(FakePanel {
                id: PanelId("layers"),
                relevant: true,
            }));
            registrar.add_workspace(Box::new(FakeWorkspace {
                id: WorkspaceId("draw"),
                layout: WorkspaceLayout {
                    right_dock: vec![PanelId("layers")],
                    bottom_tray: Vec::new(),
                    primary_tools: Vec::new(),
                    default_tool: ToolId("pencil"),
                    status_items: Vec::new(),
                },
            }));
        }
        assert!(registries.panels.get(PanelId("layers")).is_some());
        assert!(registries.workspaces.get(WorkspaceId("draw")).is_some());
    }
}
