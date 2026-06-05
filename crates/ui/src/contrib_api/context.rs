//! The two context handles carried into a contributor's render code.
//!
//! [`ContribCtx`] is the read view plus the one write channel, shared by tools
//! and (wrapped) by panels. A contributor physically cannot mutate session or
//! UI state through it - the only write path is pushing an [`Intent`] into the
//! sink (bible rules 12/21 enforced by the type system, not by convention).
//!
//! [`PanelScope`] adds what a panel additionally needs: its own [`PanelId`] (so
//! the shell, not the panel, scopes egui ids) and a mutable handle to *this*
//! panel's scratch text buffer only - the single, disjoint exception to
//! "intents are the only write channel", required because [`egui::TextEdit`]
//! needs a live `&mut String` in-frame.
//!
//! [`Intent`]: crate::state::intent::Intent

use crate::contrib_api::ids::PanelId;
use crate::state::intent::IntentSink;
use crate::state::session::SessionState;
use crate::state::ui_state::{CodexEditorDraft, UiState};
use crate::theme::Theme;

/// Read view plus the one write channel. Carried by tools and (wrapped) by panels.
///
/// The borrows are all of sibling `Host` fields the shell destructures once per
/// region per frame: `session`/`ui_state` are shared, `intents` is the sole
/// mutable handle. Reads go through the shared refs; every state change is an
/// `Intent` pushed into `intents` and applied after the frame's borrows drop.
pub struct ContribCtx<'a> {
    /// Read-only session state (active workspace/tool, jobs, AI status).
    pub session: &'a SessionState,
    /// Read-only UI state (collapse map, zoom, grid, modal, ...).
    pub ui_state: &'a UiState,
    /// The active theme, for token lookups in render code.
    pub theme: &'a Theme,
    /// The write channel for everything except this panel's scratch text.
    pub intents: &'a mut IntentSink,
}

/// What a [`Panel`] sees: a [`ContribCtx`] plus the panel's own id and scratch.
///
/// The shell builds one of these per panel per frame, supplying the panel's
/// [`PanelId`] and a `&mut String` borrowed from that panel's slot in
/// `Host.scratch`. `scratch` is the only mutable handle a panel gets beyond the
/// intent sink, it is private to this panel, and it exists solely so a
/// [`egui::TextEdit`] can bind to it. Routing real model mutation through
/// `scratch` instead of an intent is a review failure.
///
/// [`Panel`]: crate::contrib_api::panel::Panel
pub struct PanelScope<'a> {
    /// The shared read view + intent sink.
    pub ctx: ContribCtx<'a>,
    /// This panel's id - the shell uses it to scope egui ids, the panel does not.
    pub id: PanelId,
    /// A mutable handle to THIS panel's scratch text buffer only.
    pub scratch: &'a mut String,
    /// The structured Codex editor draft, supplied ONLY to the Codex Entry Editor
    /// center panel; `None` for every other panel.
    ///
    /// A single scratch `String` cannot drive an entry editor with many fields, so the
    /// shell owns a [`CodexEditorDraft`] and lends it to the editor here. It is the same
    /// in-frame `&mut`-for-`TextEdit` carve-out the scratch is, widened to a struct for
    /// the one panel that needs it; reloading it from the selection stays the shell's
    /// job (in `sync_codex_view`), and commits still flow out as `Intent`s.
    pub draft: Option<&'a mut CodexEditorDraft>,
}
