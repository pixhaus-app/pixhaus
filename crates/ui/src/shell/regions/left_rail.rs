//! Left-rail region: the tool rail. Tools come from the active workspace's
//! resolved layout; the active tool gets the accent tint + left line, AI tools the
//! sparkle.

use crate::region::region_id;
use crate::registry::resolve_layout;
use crate::state::Host;
use crate::state::intent::Intent;
use crate::theme::tokens::SurfaceTier;
use crate::widgets;

/// Render the left tool rail. A no-op when the active workspace has no tools (the
/// Codex), so no empty 48px strip renders beside its Navigator.
pub fn show(host: &mut Host, ui: &mut egui::Ui) {
    // Resolve tool ids by value first; the &registries/&state borrows end here.
    let tool_ids = resolve_layout(host.state.session.active_workspace, &host.registries).primary_tools;
    if tool_ids.is_empty() {
        return;
    }

    let Host {
        registries,
        state,
        intents,
        theme,
        ..
    } = &mut *host;

    let frame = egui::Frame::new().fill(theme.surface(SurfaceTier::Panel));
    let active = state.session.active_tool;

    egui::Panel::left(region_id::LEFT_RAIL)
        .resizable(false)
        .exact_size(48.0)
        .frame(frame)
        .show_inside(ui, |ui| {
            ui.vertical_centered(|ui| {
                for id in tool_ids {
                    let Some(tool) = registries.tools.get(id) else {
                        continue;
                    };
                    let meta = tool.meta();
                    if widgets::tool_button(ui, theme, &meta, id == active).clicked() {
                        intents.push(Intent::SelectTool(id));
                    }
                }
            });
        });
}
