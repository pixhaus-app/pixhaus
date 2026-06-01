//! Shared field-split helpers for the registry-fed regions.
//!
//! The right dock, bottom tray, and tool-options regions all reborrow-then-
//! destructure `&mut *host` into disjoint field bindings before entering a
//! `show_inside` closure (spec "The borrow-safe per-frame loop"). The closure must
//! NEVER capture `host` whole. This module factors the per-panel `PanelScope`
//! construction the dock and tray share, taking the already-split references so the
//! disjointness stays visible at each call site.

use crate::contrib_api::context::{ContribCtx, PanelScope};
use crate::contrib_api::ids::PanelId;
use crate::state::intent::IntentSink;
use crate::state::session::SessionState;
use crate::state::ui_state::UiState;
use crate::theme::Theme;

/// Build a [`PanelScope`] for one panel from the disjoint field bindings.
///
/// `session`/`ui_state`/`theme` are shared borrows of sibling `Host` fields;
/// `intents` is reborrowed per panel; `scratch` is this panel's own buffer. The
/// caller has already destructured `&mut *host`, so these are provably disjoint.
pub(crate) fn panel_scope<'a>(
    session: &'a SessionState,
    ui_state: &'a UiState,
    theme: &'a Theme,
    intents: &'a mut IntentSink,
    id: PanelId,
    scratch: &'a mut String,
) -> PanelScope<'a> {
    PanelScope {
        ctx: ContribCtx {
            session,
            ui_state,
            theme,
            intents,
        },
        id,
        scratch,
    }
}
