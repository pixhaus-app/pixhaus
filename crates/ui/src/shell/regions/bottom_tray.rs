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

    // Cap the tray so a tall tray panel (e.g. the Codex Coverage editor, which has no
    // scroll of its own) can never grow the resizable bottom panel until it swallows the
    // center stage. The panel keeps its stored default height and stays user-resizable
    // between a sane floor and at most ~55% of the window; its content scrolls inside.
    let available_height = ui.available_height();
    let min_height = 96.0_f32;
    let max_height = (available_height * 0.55).max(min_height);
    let default_height = state.ui.bottom_tray_height.clamp(min_height, max_height);

    egui::Panel::bottom(region_id::BOTTOM_TRAY)
        .resizable(true)
        .default_size(default_height)
        .size_range(min_height..=max_height)
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

            // Selected tray panel, via the disjoint-field + push_id path. The content
            // scrolls so it fits the capped tray height instead of forcing the panel taller.
            if let Some(panel) = registries.panels.get(selected) {
                egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
                    ui.push_id(selected, |ui| {
                        let buf = scratch.entry(selected).or_insert_with(|| panel.default_scratch().unwrap_or_default());
                        let mut scope = panel_scope(&state.session, &state.ui, theme, &mut *intents, selected, buf);
                        panel.ui(ui, &mut scope);
                    });
                });
            }
        });
}
