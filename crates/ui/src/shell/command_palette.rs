//! Command palette overlay: an `egui::Area` drawn after the central panel, gated on
//! `UiState.modal == CommandPalette`. A query field plus a registry-seeded entry
//! list (workspaces and tools are live; actions and the UX examples are mock).
//! Escape closes.

use crate::state::Host;
use crate::state::intent::{Intent, IntentSink};
use crate::state::ui_state::Modal;
use crate::theme::tokens::SurfaceTier;

/// One palette row: its display label and the intent a click re-emits.
struct Entry {
    label: String,
    intent: Intent,
}

/// Draw the palette overlay when the modal is open.
pub fn overlay(host: &mut Host, ui: &mut egui::Ui) {
    if !matches!(host.state.ui.modal, Some(Modal::CommandPalette)) {
        return;
    }

    // Escape closes (read before borrowing the query field mutably).
    if ui.ctx().input(|i| i.key_pressed(egui::Key::Escape)) {
        host.intents.push(Intent::CloseModal);
        return;
    }

    let Host {
        registries,
        state,
        intents,
        theme,
        ..
    } = &mut *host;

    // Seed entries: workspaces (live), tools (live), actions + UX examples (mock).
    let mut entries: Vec<Entry> = Vec::new();
    for ws in registries.workspaces.iter() {
        entries.push(Entry {
            label: format!("Switch to {}", ws.meta().name),
            intent: Intent::SelectWorkspace(ws.id()),
        });
    }
    for tool in registries.tools.iter() {
        entries.push(Entry {
            label: format!("Select {}", tool.meta().label),
            intent: Intent::SelectTool(tool.id()),
        });
    }
    for action in registries.actions.iter() {
        if action.palette_visible {
            entries.push(Entry {
                label: action.label.to_owned(),
                intent: Intent::RunAction(action.id),
            });
        }
    }

    // Filter by the live query (case-insensitive substring). Context-aware ranking
    // (UX 20.3) is a deferred enhancement that arrives with core.
    let query = state.ui.palette_query.to_lowercase();
    let filtered: Vec<&Entry> = entries.iter().filter(|e| query.is_empty() || e.label.to_lowercase().contains(&query)).collect();

    // content_rect is the area available to the UI (screen_rect was split in 0.34).
    let content = ui.ctx().content_rect();
    let area_pos = egui::pos2(content.center().x - 240.0, content.top() + 80.0);

    egui::Area::new(egui::Id::new("pixhaus.command_palette"))
        .order(egui::Order::Foreground)
        .fixed_pos(area_pos)
        .show(ui.ctx(), |ui| {
            let frame = egui::Frame::new()
                .fill(theme.surface(SurfaceTier::Elevated))
                .inner_margin(theme.spacing.md)
                .corner_radius(theme.radius.md)
                .shadow(theme.elevation.overlay);
            frame.show(ui, |ui| {
                ui.set_min_width(480.0);
                let edit = ui.add(
                    egui::TextEdit::singleline(&mut state.ui.palette_query)
                        .hint_text("Type a command")
                        .desired_width(f32::INFINITY),
                );
                edit.request_focus();
                ui.separator();
                egui::ScrollArea::vertical().max_height(360.0).auto_shrink([false, false]).show(ui, |ui| {
                    for entry in filtered {
                        if ui.button(&entry.label).clicked() {
                            reemit(intents, &entry.intent);
                            intents.push(Intent::CloseModal);
                        }
                    }
                });
            });
        });
}

// `Intent` is intentionally not `Clone` (the reserved `Command(Box<dyn Command>)`
// variant would forbid it), so re-emit by matching the palette-reachable variants,
// whose payloads (`WorkspaceId`/`ToolId`/`ActionId`) are all `Copy`.
fn reemit(intents: &mut IntentSink, intent: &Intent) {
    match intent {
        Intent::SelectWorkspace(w) => intents.push(Intent::SelectWorkspace(*w)),
        Intent::SelectTool(t) => intents.push(Intent::SelectTool(*t)),
        Intent::RunAction(a) => intents.push(Intent::RunAction(*a)),
        _ => {}
    }
}
