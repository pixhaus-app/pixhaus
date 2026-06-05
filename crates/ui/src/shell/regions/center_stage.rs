//! Center-stage region: the `CentralPanel` (added last). It draws whatever the active
//! workspace asked for in its center - [`CenterSurface::Canvas`] routes to the
//! navigable wgpu canvas, and [`CenterSurface::Panel`] renders a registered panel
//! full-center (the Codex Entry Editor / Visual Board / Graph). The canvas path is
//! unchanged from before; the panel path is the Codex's non-canvas center.

use crate::contrib_api::CenterSurface;
use crate::registry::resolve_layout;
use crate::shell::regions::canvas_stage;
use crate::state::Host;
use crate::theme::tokens::SurfaceTier;

/// Render the center stage: the canvas, or a full-center panel.
pub fn show(host: &mut Host, ui: &mut egui::Ui) {
    let center = resolve_layout(host.state.session.active_workspace, &host.registries).center;
    match center {
        CenterSurface::Canvas => canvas_stage::show(host, ui),
        CenterSurface::Panel(id) => show_center_panel(host, ui, id),
    }
}

/// Render a registered panel as the full center, inside the `CentralPanel`. The panel
/// reads `scope.ctx` and pushes intents like any docked panel; it is rendered without
/// the card chrome (the center panel owns its own internal layout - tabs, board,
/// graph). The disjoint-field + `push_id` borrow path matches the dock regions.
fn show_center_panel(host: &mut Host, ui: &mut egui::Ui, id: crate::contrib_api::PanelId) {
    use crate::shell::regions::scope_split::panel_scope_with_draft;

    let Host {
        registries,
        state,
        intents,
        scratch,
        codex_draft,
        theme,
        ..
    } = &mut *host;

    let frame = egui::Frame::new().fill(theme.surface(SurfaceTier::Stage));
    egui::CentralPanel::default().frame(frame).show_inside(ui, |ui| {
        let Some(panel) = registries.panels.get(id) else {
            return;
        };
        ui.push_id(id, |ui| {
            let buf = scratch.entry(id).or_insert_with(|| panel.default_scratch().unwrap_or_default());
            // The center panel is the one that may need the structured editor draft (the
            // Codex Entry Editor); lend it here. A center panel that does not use it
            // (none today besides the editor) simply ignores `scope.draft`.
            let mut scope = panel_scope_with_draft(&state.session, &state.ui, theme, &mut *intents, id, buf, codex_draft);
            panel.ui(ui, &mut scope);
        });
    });
}
