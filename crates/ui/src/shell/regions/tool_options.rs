//! Tool-options region: the active tool's `options_ui`, rendered with a bare
//! `ContribCtx` (a tool is not a panel - no scratch, no `PanelId`).

use crate::contrib_api::context::ContribCtx;
use crate::region::region_id;
use crate::state::Host;
use crate::theme::tokens::SurfaceTier;

/// Render the active tool's options bar.
pub fn show(host: &mut Host, ui: &mut egui::Ui) {
    let Host {
        registries,
        state,
        intents,
        theme,
        ..
    } = &mut *host;

    let frame = egui::Frame::new().fill(theme.surface(SurfaceTier::Elevated)).inner_margin(theme.spacing.sm);

    egui::Panel::top(region_id::TOOL_OPTIONS).frame(frame).show_inside(ui, |ui| {
        let Some(tool) = registries.tools.get(state.session.active_tool) else {
            return;
        };
        let mut cx = ContribCtx {
            session: &state.session,
            ui_state: &state.ui,
            theme,
            intents,
        };
        tool.options_ui(ui, &mut cx);
    });
}
