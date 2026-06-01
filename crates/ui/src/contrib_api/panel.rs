//! The `Panel` trait and its metadata.
//!
//! A panel renders representative content into the right dock or bottom tray.
//! It is dyn-compatible on purpose (stored as `Box<dyn Panel>` in the registry):
//! `&self` receivers, no generic methods, no `-> Self`, metadata returned by
//! value. The compile-time guard in `contrib_api::mod` enforces this.

use crate::contrib_api::context::PanelScope;
use crate::contrib_api::ids::PanelId;
use crate::contrib_api::ids::WorkspaceId;
use crate::region::Region;

/// A registered panel: stable identity, metadata, and a render method.
///
/// # Object safety
///
/// Every method takes `&self` and uses no generics or `-> Self`, so `Panel` is
/// dyn-compatible and lives in the registry as `Box<dyn Panel>`. The `&self`
/// receiver is deliberate: a panel holds no mutable state of its own - its
/// collapse flag lives in `UiState`, its draft text in `Host.scratch`. That is
/// what lets the shell iterate `&registry.panels` (a shared borrow) while
/// holding `&mut intents` and `&mut scratch` (sibling `Host` fields) without
/// aliasing.
pub trait Panel {
    /// This panel's stable id - also its registry key.
    fn id(&self) -> PanelId;

    /// Static metadata: title, icon, default placement, default open state.
    fn meta(&self) -> PanelMeta;

    /// Capability predicate: could this panel ever appear in `workspace`?
    ///
    /// The shell uses this only as a `debug_assert` against a workspace's
    /// authored layout - NOT as a runtime placement filter. The
    /// [`WorkspaceLayout`] is the sole placement authority (bible rule 14).
    /// Default: usable anywhere it is listed.
    ///
    /// [`WorkspaceLayout`]: crate::contrib_api::workspace::WorkspaceLayout
    fn relevant_in(&self, _workspace: WorkspaceId) -> bool {
        true
    }

    /// Render representative content.
    ///
    /// Reads through `scope.ctx`; pushes [`Intent`]s into `scope.ctx.intents`;
    /// may edit only `scope.scratch`. Nothing else is mutable.
    ///
    /// [`Intent`]: crate::state::intent::Intent
    fn ui(&self, ui: &mut egui::Ui, scope: &mut PanelScope<'_>);
}

/// Static, by-value metadata describing a panel.
///
/// Returned by value (not borrowed) so [`Panel`] stays dyn-compatible.
pub struct PanelMeta {
    /// Display title shown in the card header.
    pub title: &'static str,
    /// Phosphor glyph from [`crate::icons`].
    pub icon: char,
    /// Where this panel sits unless a workspace places it elsewhere.
    pub default_region: Region,
    /// Whether the panel starts expanded.
    pub default_open: bool,
}
