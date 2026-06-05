//! Tool-options region: the active tool's `options_ui`, rendered with a bare
//! `ContribCtx` (a tool is not a panel - no scratch, no `PanelId`).

use crate::contrib_api::CenterSurface;
use crate::contrib_api::context::ContribCtx;
use crate::region::region_id;
use crate::registry::ResolvedLayout;
use crate::registry::resolve_layout;
use crate::state::Host;
use crate::theme::tokens::SurfaceTier;

/// Whether the tool-options bar must be suppressed for this layout: a workspace with no
/// tools, or one whose center is a full panel (the Codex), shows no pixel-tool chrome.
/// Decided from the resolved layout, not the active tool, so the bar never flashes
/// during a workspace switch.
fn suppresses_tool_chrome(layout: &ResolvedLayout) -> bool {
    layout.primary_tools.is_empty() || matches!(layout.center, CenterSurface::Panel(_))
}

/// Render the active tool's options bar. A no-op for a toolless / panel-center
/// workspace (the Codex).
pub fn show(host: &mut Host, ui: &mut egui::Ui) {
    let layout = resolve_layout(host.state.session.active_workspace, &host.registries);
    if suppresses_tool_chrome(&layout) {
        return;
    }

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

#[cfg(test)]
mod tests {
    use super::suppresses_tool_chrome;
    use crate::contrib_api::{CenterSurface, PanelId, ToolId};
    use crate::registry::ResolvedLayout;

    /// A canvas workspace with tools keeps its tool-options bar.
    #[test]
    fn canvas_with_tools_shows_chrome() {
        let mut layout = ResolvedLayout::empty();
        layout.primary_tools = vec![ToolId("pencil")];
        layout.center = CenterSurface::Canvas;
        assert!(!suppresses_tool_chrome(&layout), "a canvas workspace with tools keeps the tool-options bar");
    }

    /// A toolless, panel-center workspace (the Codex shape) shows no tool chrome.
    #[test]
    fn panel_center_toolless_suppresses_chrome() {
        let mut layout = ResolvedLayout::empty();
        layout.primary_tools = Vec::new();
        layout.center = CenterSurface::Panel(PanelId("codex-editor"));
        assert!(suppresses_tool_chrome(&layout), "the Codex (no tools, panel center) shows no pixel-tool chrome");
    }
}
