//! Bottom-tray region: a tab row (selectable chips, active = accent pill) plus the
//! selected tray panel, rendered through the same disjoint-field + `push_id` path as
//! the right dock. Both the tabs and the content swap per workspace.

use crate::region::region_id;
use crate::registry::resolve_layout;
use crate::shell::regions::scope_split::panel_scope;
use crate::state::Host;
use crate::state::intent::Intent;
use crate::theme::tokens::SurfaceTier;
use crate::widgets;

/// Render the bottom tray (tab row + selected panel).
pub fn show(host: &mut Host, ui: &mut egui::Ui) {
    let active_ws = host.state.session.active_workspace;
    let tray = resolve_layout(active_ws, &host.registries).bottom_tray;
    if tray.is_empty() {
        return;
    }
    // Selected tab: the per-workspace stored tab if still present, else the first.
    let selected = host.state.ui.tray_tab.get(&active_ws).copied().filter(|p| tray.contains(p)).unwrap_or(tray[0]);

    let Host {
        registries,
        state,
        intents,
        scratch,
        theme,
        ..
    } = &mut *host;

    let frame = egui::Frame::new().fill(theme.surface(SurfaceTier::Panel));

    egui::Panel::bottom(region_id::BOTTOM_TRAY)
        .resizable(true)
        .default_size(state.ui.bottom_tray_height)
        .frame(frame)
        .show_inside(ui, |ui| {
            // Tab row.
            ui.horizontal(|ui| {
                for &id in &tray {
                    let Some(panel) = registries.panels.get(id) else {
                        continue;
                    };
                    let meta = panel.meta();
                    if widgets::tray_tab(ui, theme, &meta.title.tr(), id == selected).clicked() {
                        intents.push(Intent::SelectTrayTab(id));
                    }
                }
            });
            ui.separator();

            // Selected tray panel, via the disjoint-field + push_id path.
            if let Some(panel) = registries.panels.get(selected) {
                ui.push_id(selected, |ui| {
                    let buf = scratch.entry(selected).or_default();
                    let mut scope = panel_scope(&state.session, &state.ui, theme, &mut *intents, selected, buf);
                    panel.ui(ui, &mut scope);
                });
            }
        });
}
