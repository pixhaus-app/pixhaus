//! Left-dock region: a panel card stack on the window's left edge, inboard of the
//! tool rail. Empty for the canvas workspaces; the Codex uses it for its Navigator.
//! Same borrow-safe loop as the right dock (resolve ids by value, reborrow-then-
//! destructure into disjoint field bindings, `push_id` per panel).

use crate::registry::resolve_layout;
use crate::shell::regions::scope_split::panel_scope;
use crate::state::Host;
use crate::widgets;

/// Stable egui id source for the left-dock side panel.
const LEFT_DOCK_ID: &str = "pixhaus.leftdock";

/// Render the left-dock card stack. A no-op when the active workspace has no left
/// dock, so it adds no panel and the canvas keeps the full width.
pub fn show(host: &mut Host, ui: &mut egui::Ui) {
    // Resolve ids by value FIRST - the &registries/&state borrows end here.
    let ids = resolve_layout(host.state.session.active_workspace, &host.registries).left_dock;
    if ids.is_empty() {
        return;
    }

    let Host {
        registries,
        state,
        intents,
        scratch,
        theme,
        ..
    } = &mut *host;

    egui::Panel::left(LEFT_DOCK_ID)
        .resizable(true)
        .default_size(state.ui.left_dock_width)
        .show_inside(ui, |ui| {
            for id in ids {
                let Some(panel) = registries.panels.get(id) else {
                    continue;
                };
                let meta = panel.meta();
                let collapsed = state.ui.collapsed.get(&id).copied().unwrap_or(!meta.default_open);
                ui.push_id(id, |ui| {
                    let header = widgets::card(ui, theme, &meta, collapsed, |ui| {
                        let buf = scratch.entry(id).or_insert_with(|| panel.default_scratch().unwrap_or_default());
                        let mut scope = panel_scope(&state.session, &state.ui, theme, &mut *intents, id, buf);
                        panel.ui(ui, &mut scope);
                    });
                    if header.clicked() {
                        intents.push(crate::state::intent::Intent::TogglePanelCollapsed(id));
                    }
                });
            }
        });
}
