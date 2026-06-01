//! Intents and events: the one write channel and the post-frame notification bus.
//!
//! An [`Intent`] is a requested change; a contributor pushes intents into an
//! [`IntentSink`] and the shell applies them after the frame's region borrows drop
//! (`apply_intent`, defined alongside [`crate::state::Host`]). An [`Event`] is
//! "something happened", produced only inside `apply_intent` and consumed on the
//! next frame - never read by panels during render, so there is no intra-frame event
//! bus and the borrow guarantee has no hole (spec bible 21.1).

use crate::contrib_api::ids::{ActionId, PanelId, ToolId, WorkspaceId};
use crate::state::Host;
use crate::state::session::JobStub;
use crate::state::ui_state::{GridMode, Modal};
use crate::theme::{Theme, ThemeVariant, apply_to_visuals};

/// A requested change to session or UI state. The single write channel for
/// everything except a panel's own scratch text. Applied post-frame.
pub enum Intent {
    /// Switch the active workspace.
    SelectWorkspace(WorkspaceId),
    /// Select a tool in the active workspace's rail.
    SelectTool(ToolId),
    /// Select a tray tab; applies to the active workspace's tray.
    SelectTrayTab(PanelId),
    /// Toggle a panel's collapse flag.
    TogglePanelCollapsed(PanelId),
    /// Set the canvas grid mode.
    SetGrid(GridMode),
    /// Toggle onion skin (Animate).
    ToggleOnionSkin,
    /// Toggle pixel snap.
    ToggleSnap,
    /// Set canvas zoom.
    SetZoom(f32),
    /// Open the command palette modal.
    OpenCommandPalette,
    /// Dismiss any open modal.
    CloseModal,
    /// Change the theme variant; `apply_intent` re-applies it to egui's visuals.
    SetThemeVariant(ThemeVariant),
    /// Run an action. Mock: pushes a `JobStub` and emits an Event. Never mutates the
    /// model (spec invariant) - when `core` lands, model edits route through the
    /// reserved `Command` variant below instead.
    RunAction(ActionId),
    // Reserved, lands with core - the named command-path seam (bible rules 3, 4, 13):
    // Command(Box<dyn core::Command>),
}

/// "Something happened", distinct from a command (spec bible 21.3). Produced only
/// inside `apply_intent`, consumed on the next frame. This round it is a
/// `tracing::debug!` sink.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum Event {
    /// The active workspace changed.
    WorkspaceChanged(WorkspaceId),
    /// The active tool changed.
    ToolChanged(ToolId),
    /// An action was dispatched.
    ActionDispatched(ActionId),
}

/// The write channel a contributor pushes [`Intent`]s into during a frame.
#[derive(Default)]
pub struct IntentSink(pub(crate) Vec<Intent>);

impl IntentSink {
    /// Queue an intent for post-frame application.
    pub fn push(&mut self, i: Intent) {
        self.0.push(i);
    }
}

/// Apply one intent to the host, after the frame's region borrows have dropped.
///
/// Takes the `egui::Context` because the theme path must re-apply to egui's visuals
/// on a variant change. `RunAction` is a mock UI affordance: it queues a job and logs
/// an event but NEVER mutates project state (spec invariant) - model edits route
/// through the reserved `Command` variant when `core` lands.
///
/// `intent` is taken by value, not by reference: the reserved `Command(Box<dyn
/// core::Command>)` variant will move an owned command out of the intent, so the
/// function must own it and `Intent` cannot be `Copy`. Today's arms only read `Copy`
/// payloads, which is why clippy sees the value as unconsumed.
#[allow(clippy::needless_pass_by_value)]
pub fn apply_intent(host: &mut Host, intent: Intent, ctx: &egui::Context) {
    match intent {
        Intent::SelectWorkspace(w) => {
            host.state.session.active_workspace = w;
            tracing::debug!(?w, "WorkspaceChanged");
        }
        Intent::SelectTool(t) => {
            host.state.session.active_tool = t;
            tracing::debug!(?t, "ToolChanged");
        }
        Intent::SelectTrayTab(p) => {
            let w = host.state.session.active_workspace;
            host.state.ui.tray_tab.insert(w, p);
        }
        Intent::TogglePanelCollapsed(p) => {
            let e = host.state.ui.collapsed.entry(p).or_insert(false);
            *e = !*e;
        }
        Intent::SetGrid(g) => {
            host.state.ui.grid = g;
        }
        Intent::ToggleOnionSkin => {
            host.state.ui.onion_skin = !host.state.ui.onion_skin;
        }
        Intent::ToggleSnap => {
            host.state.ui.snap = !host.state.ui.snap;
        }
        Intent::SetZoom(z) => {
            host.state.ui.zoom = z;
        }
        Intent::OpenCommandPalette => {
            host.state.ui.modal = Some(Modal::CommandPalette);
        }
        Intent::CloseModal => {
            host.state.ui.modal = None;
        }
        Intent::SetThemeVariant(v) => {
            host.theme = Theme::for_variant(v, host.theme.accent_seed());
            apply_to_visuals(&host.theme, ctx);
        }
        Intent::RunAction(a) => {
            host.state.session.jobs.push(JobStub::queued(a));
            tracing::debug!(?a, "ActionDispatched");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::contrib_api::ids::{ActionId, PanelId, ToolId, WorkspaceId};
    use crate::state::Host;
    use crate::state::session::JobState;
    use crate::state::ui_state::{GridMode, Modal};
    use crate::theme::{Theme, ThemeVariant};

    fn host() -> Host {
        Host::new(Theme::dark())
    }

    fn ctx() -> egui::Context {
        // A headless Context: no event loop, no GPU. apply_intent's theme path only
        // touches ctx.style_mut, which a default Context fully supports.
        egui::Context::default()
    }

    #[test]
    fn push_appends_intents_in_order() {
        let mut sink = IntentSink::default();
        sink.push(Intent::SelectWorkspace(WorkspaceId("draw")));
        sink.push(Intent::OpenCommandPalette);
        assert_eq!(sink.0.len(), 2, "both intents are queued");
        assert!(
            matches!(sink.0[0], Intent::SelectWorkspace(WorkspaceId("draw"))),
            "first pushed intent stays first",
        );
        assert!(matches!(sink.0[1], Intent::OpenCommandPalette), "second pushed intent stays second");
    }

    #[test]
    fn select_workspace_flips_active_workspace() {
        let mut host = host();
        apply_intent(&mut host, Intent::SelectWorkspace(WorkspaceId("animate")), &ctx());
        assert_eq!(host.state.session.active_workspace, WorkspaceId("animate"));
    }

    #[test]
    fn select_tool_flips_active_tool() {
        let mut host = host();
        apply_intent(&mut host, Intent::SelectTool(ToolId("eraser")), &ctx());
        assert_eq!(host.state.session.active_tool, ToolId("eraser"));
    }

    #[test]
    fn select_tray_tab_updates_the_active_workspaces_tab() {
        let mut host = host();
        // Default workspace is Draw; the tab should be recorded under "draw".
        apply_intent(&mut host, Intent::SelectTrayTab(PanelId("assets")), &ctx());
        assert_eq!(
            host.state.ui.tray_tab.get(&WorkspaceId("draw")).copied(),
            Some(PanelId("assets")),
            "the tray tab is recorded for the active workspace only",
        );
    }

    #[test]
    fn toggle_panel_collapsed_flips_then_flips_back() {
        let mut host = host();
        let p = PanelId("layers");
        apply_intent(&mut host, Intent::TogglePanelCollapsed(p), &ctx());
        assert_eq!(host.state.ui.collapsed.get(&p).copied(), Some(true), "first toggle collapses");
        apply_intent(&mut host, Intent::TogglePanelCollapsed(p), &ctx());
        assert_eq!(host.state.ui.collapsed.get(&p).copied(), Some(false), "second toggle expands");
    }

    #[test]
    fn set_theme_variant_swaps_the_variant() {
        let mut host = host();
        apply_intent(&mut host, Intent::SetThemeVariant(ThemeVariant::Light), &ctx());
        assert_eq!(host.theme.variant, ThemeVariant::Light, "the variant is swapped on the host theme");
    }

    #[test]
    fn open_command_palette_sets_the_modal() {
        let mut host = host();
        apply_intent(&mut host, Intent::OpenCommandPalette, &ctx());
        assert_eq!(host.state.ui.modal, Some(Modal::CommandPalette));
    }

    #[test]
    fn close_modal_clears_the_modal() {
        let mut host = host();
        apply_intent(&mut host, Intent::OpenCommandPalette, &ctx());
        apply_intent(&mut host, Intent::CloseModal, &ctx());
        assert!(host.state.ui.modal.is_none(), "CloseModal clears whatever was open");
    }

    #[test]
    fn set_grid_changes_the_grid_mode() {
        let mut host = host();
        apply_intent(&mut host, Intent::SetGrid(GridMode::Px16), &ctx());
        assert_eq!(host.state.ui.grid, GridMode::Px16);
    }

    #[test]
    fn toggle_onion_skin_and_snap_flip_their_flags() {
        let mut host = host();
        let snap0 = host.state.ui.snap;
        apply_intent(&mut host, Intent::ToggleOnionSkin, &ctx());
        apply_intent(&mut host, Intent::ToggleSnap, &ctx());
        assert!(host.state.ui.onion_skin, "onion skin starts false and toggles on");
        assert_eq!(host.state.ui.snap, !snap0, "snap flips from its default");
    }

    #[test]
    fn set_zoom_records_the_zoom() {
        let mut host = host();
        apply_intent(&mut host, Intent::SetZoom(16.0), &ctx());
        assert_eq!(host.state.ui.zoom, 16.0);
    }

    #[test]
    fn run_action_pushes_a_queued_job_and_never_mutates_session_dirty() {
        let mut host = host();
        let was_dirty = host.state.session.dirty;
        apply_intent(&mut host, Intent::RunAction(ActionId("ai.fill")), &ctx());
        assert_eq!(host.state.session.jobs.len(), 1, "RunAction pushes exactly one JobStub");
        assert_eq!(host.state.session.jobs[0].state, JobState::Queued, "the job is queued");
        assert_eq!(
            host.state.session.dirty, was_dirty,
            "RunAction is a mock UI affordance and must never mutate project state (spec invariant)",
        );
    }
}
