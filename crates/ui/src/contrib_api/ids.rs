//! Stable identity newtypes for the contribution surface.
//!
//! Each wraps a `&'static str` so a `PanelId` can never be confused with a
//! `ToolId` at the type level, and all derive `Copy + Eq + Hash` so they serve
//! directly as registry keys. The inner string is the stable id a module
//! registers under and a workspace layout references by value.

/// Identifies a registered panel (e.g. `PanelId("layers")`).
#[derive(Copy, Clone, PartialEq, Eq, Hash, Debug)]
pub struct PanelId(pub &'static str);

/// Identifies a registered tool (e.g. `ToolId("pencil")`).
#[derive(Copy, Clone, PartialEq, Eq, Hash, Debug)]
pub struct ToolId(pub &'static str);

/// Identifies a registered workspace (e.g. `WorkspaceId("draw")`).
#[derive(Copy, Clone, PartialEq, Eq, Hash, Debug)]
pub struct WorkspaceId(pub &'static str);

/// Identifies a registered action - a menu item or command-palette entry.
#[derive(Copy, Clone, PartialEq, Eq, Hash, Debug)]
pub struct ActionId(pub &'static str);

/// A stable localization key (e.g. `MsgKey("panel.layers.title")`).
///
/// Registries carry `MsgKey`, not display text. A module names a key at
/// registration - `const`, no allocation, no locale touched - and the shell
/// resolves it to text in the active language at render time via [`MsgKey::tr`].
/// The newtype is what turns "display a key without resolving it" into a compile
/// error: a raw `MsgKey` cannot be handed to `ui.label(..)`, only the resolved
/// `String` from `.tr()` can.
#[derive(Copy, Clone, PartialEq, Eq, Hash, Debug)]
pub struct MsgKey(pub &'static str);

impl MsgKey {
    /// Resolve this key to display text in the active language, through the
    /// localization service. Call at render time, never at registration.
    #[must_use]
    pub fn tr(self) -> String {
        pixhaus_services::i18n::tr(self.0)
    }
}

#[cfg(test)]
mod tests {
    use super::{ActionId, PanelId, ToolId, WorkspaceId};

    #[test]
    fn distinct_ids_compare_and_hash_independently() {
        use std::collections::HashSet;
        let mut set = HashSet::new();
        assert!(set.insert(PanelId("layers")));
        // Same string under a different newtype is a different key.
        assert!(set.insert(PanelId("frames")));
        assert!(!set.insert(PanelId("layers")));
        assert_eq!(PanelId("layers"), PanelId("layers"));
        assert_ne!(PanelId("layers"), PanelId("frames"));
    }

    #[test]
    fn ids_are_copy() {
        // Compiles only because every id is `Copy`: each is used after being passed by value.
        let p = PanelId("p");
        let t = ToolId("t");
        let w = WorkspaceId("w");
        let a = ActionId("a");
        let _ = (p, t, w, a);
        let _ = (p, t, w, a);
    }
}
