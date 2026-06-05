//! Pixhaus Codex module: the project-level creative bible as a workspace.
//!
//! Registers the Codex workspace (a non-canvas layout - a left Navigator, a
//! full-center Entry Editor / Visual Board / Graph surface, a right Inspector, and a
//! Coverage / Test Generation / History bottom strip), its panels, and the
//! `codex.*` actions (architecture bible, Codex bible sections 8, 9.2, 14, 21). The
//! rule is the Codex remembers, AI proposes, the artist decides: every model change
//! routes through a core command via an `Intent`; the panels read the read-only
//! `CodexView` mirror the shell rebuilds each frame and never touch the document.
#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used, clippy::disallowed_methods))]

mod codex_ws;

use pixhaus_ui::contrib_api::{HostRegistrar, Module};

/// The Codex module. Registers the Codex workspace, its Navigator/Editor/Inspector
/// panels, the Coverage/Test/History tray panels, and the `codex.*` actions.
pub struct CodexModule;

impl Module for CodexModule {
    fn id(&self) -> &'static str {
        "codex"
    }

    fn register(&self, host: &mut dyn HostRegistrar) {
        codex_ws::register(host);
        tracing::info!(module = "codex", "registered the Codex workspace");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn module_id_is_codex() {
        assert_eq!(CodexModule.id(), "codex");
    }
}
