//! Layout resolution: turn a workspace's authored layout into a [`ResolvedLayout`]
//! filtered to actually-registered ids. See [`resolve_layout`].

use crate::contrib_api::{PanelId, StatusItem, ToolId, WorkspaceId};
use crate::registry::Registries;

/// A workspace layout with panel ids filtered down to the ones actually registered.
///
/// The shell renders straight from this: `right_dock` is the card stack
/// top-to-bottom, `bottom_tray` is the tray tabs left-to-right (first is the
/// default tab). Unregistered ids have already been dropped (with a warn) so the
/// shell never has to handle a dangling reference.
#[derive(Clone, PartialEq, Debug)]
pub struct ResolvedLayout {
    /// Right-dock card stack, top-to-bottom, filtered to registered ids.
    pub right_dock: Vec<PanelId>,
    /// Bottom-tray tabs, left-to-right, filtered to registered ids.
    pub bottom_tray: Vec<PanelId>,
    /// The tools shown in the left rail, in order.
    pub primary_tools: Vec<ToolId>,
    /// The tool selected when the workspace activates.
    pub default_tool: ToolId,
    /// Workspace-specific status-bar entries.
    pub status_items: Vec<StatusItem>,
}

impl ResolvedLayout {
    /// The fallback layout for an unknown workspace id: everything empty.
    ///
    /// `default_tool` is a sentinel `ToolId("")`; an unknown workspace renders no
    /// rail, so it is never read.
    pub fn empty() -> Self {
        Self {
            right_dock: Vec::new(),
            bottom_tray: Vec::new(),
            primary_tools: Vec::new(),
            default_tool: ToolId(""),
            status_items: Vec::new(),
        }
    }
}

/// Resolves a workspace's authored layout against the registries.
///
/// Returns [`ResolvedLayout::empty`] for an unknown workspace. Otherwise filters
/// the authored `right_dock` and `bottom_tray` to ids that are actually
/// registered: a present panel `debug_assert`s its `relevant_in` answer (a
/// workspace listing an irrelevant panel is an authoring bug), and an absent id
/// is dropped with a `tracing::warn!` rather than degrading silently.
pub fn resolve_layout(ws: WorkspaceId, r: &Registries) -> ResolvedLayout {
    let Some(workspace) = r.workspaces.get(ws) else {
        return ResolvedLayout::empty();
    };
    let layout = workspace.layout();
    let keep_panel = |id: &PanelId| match r.panels.get(*id) {
        Some(panel) => {
            debug_assert!(panel.relevant_in(ws), "workspace listed an irrelevant panel");
            true
        }
        None => {
            tracing::warn!(?id, "workspace references an unregistered panel; skipping");
            false
        }
    };
    ResolvedLayout {
        right_dock: layout.right_dock.iter().copied().filter(keep_panel).collect(),
        bottom_tray: layout.bottom_tray.iter().copied().filter(keep_panel).collect(),
        primary_tools: layout.primary_tools,
        default_tool: layout.default_tool,
        status_items: layout.status_items,
    }
}

#[cfg(test)]
mod tests {
    use super::{ResolvedLayout, resolve_layout};
    use crate::contrib_api::{HostRegistrar, PanelId, ToolId, WorkspaceId, WorkspaceLayout};
    use crate::registry::Registries;
    use crate::registry::fixtures::{FakePanel, FakeWorkspace};

    /// A registered panel survives resolution; a referenced-but-unregistered panel
    /// is filtered out (and warns) rather than producing a dangling id.
    #[test]
    fn resolve_filters_unregistered_panels() {
        let mut registries = Registries::default();
        {
            let mut registrar = registries.registrar();
            registrar.add_panel(Box::new(FakePanel {
                id: PanelId("layers"),
                relevant: true,
            }));
            // "ghost" is referenced by the layout below but never registered.
            registrar.add_workspace(Box::new(FakeWorkspace {
                id: WorkspaceId("draw"),
                layout: WorkspaceLayout {
                    right_dock: vec![PanelId("layers"), PanelId("ghost")],
                    bottom_tray: vec![PanelId("ghost")],
                    primary_tools: vec![ToolId("pencil")],
                    default_tool: ToolId("pencil"),
                    status_items: Vec::new(),
                },
            }));
        }

        let resolved = resolve_layout(WorkspaceId("draw"), &registries);
        assert_eq!(resolved.right_dock, vec![PanelId("layers")]);
        assert!(resolved.bottom_tray.is_empty(), "the only tray ref was unregistered");
        assert_eq!(resolved.primary_tools, vec![ToolId("pencil")]);
        assert_eq!(resolved.default_tool, ToolId("pencil"));
    }

    /// An unknown workspace id resolves to the empty layout, never a panic.
    #[test]
    fn resolve_unknown_workspace_is_empty() {
        let registries = Registries::default();
        let resolved = resolve_layout(WorkspaceId("nope"), &registries);
        assert_eq!(resolved, ResolvedLayout::empty());
    }
}
