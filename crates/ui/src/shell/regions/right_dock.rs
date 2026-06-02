//! Right-dock region: the panel card stack. The load-bearing borrow loop
//! (spec "The borrow-safe per-frame loop"): resolve ids by value first, reborrow-
//! then-destructure into disjoint field bindings, `push_id` per panel, reborrow the
//! mutable channels each iteration. Provably disjoint; no `RefCell`, no `mem::take`.

use crate::region::region_id;
use crate::registry::resolve_layout;
use crate::shell::regions::scope_split::panel_scope;
use crate::state::Host;
use crate::widgets;

/// Render the right-dock card stack.
pub fn show(host: &mut Host, ui: &mut egui::Ui) {
    // 1. Resolve ids by value FIRST - the &registries/&state borrows end here.
    let ids = resolve_layout(host.state.session.active_workspace, &host.registries).right_dock;

    // 2. Reborrow-then-destructure into disjoint field bindings. Must be `&mut *host`,
    //    not `host` - a by-value field pattern on `&mut Host` is move-out-of-borrow (E0507).
    let Host {
        registries,
        state,
        intents,
        scratch,
        theme,
        ..
    } = &mut *host;

    egui::Panel::right(region_id::RIGHT_DOCK)
        .resizable(true)
        .default_size(state.ui.right_dock_width) // 0.34 Panel API: default_size, not default_width
        .show_inside(ui, |ui| {
            for id in ids {
                let Some(panel) = registries.panels.get(id) else {
                    continue;
                };
                let meta = panel.meta();
                let collapsed = state.ui.collapsed.get(&id).copied().unwrap_or(!meta.default_open);
                // The SHELL scopes ids - not the panel. Distinct call site per PanelId.
                ui.push_id(id, |ui| {
                    let header = widgets::card(ui, theme, &meta, collapsed, |ui| {
                        // &mut String for this panel only; seeded once from the panel's default.
                        let buf = scratch.entry(id).or_insert_with(|| panel.default_scratch().unwrap_or_default());
                        let mut scope = panel_scope(
                            &state.session,
                            &state.ui,
                            theme,
                            &mut *intents, // reborrowed, not moved
                            id,
                            buf,
                        );
                        panel.ui(ui, &mut scope);
                    });
                    // The card returns the header click; map it to a collapse toggle.
                    if header.clicked() {
                        intents.push(crate::state::intent::Intent::TogglePanelCollapsed(id));
                    }
                });
            }
        });
}
