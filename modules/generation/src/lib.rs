//! Pixhaus generation module: the AI-forward Generate workspace.
//!
//! Registers the Generate workspace, the prompt composer and recipe/structure/
//! style panels, palette-behavior and advanced settings, and the Results and
//! History tray panels (architecture bible sections 7.3, 6.5, 14). AI proposes;
//! applying a result is a command - this round the panels render mock content and
//! dispatch `gen.*` actions through the intent sink. Provider dispatch arrives with
//! the services layer.
#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used))]

mod generate;

use pixhaus_ui::contrib_api::{HostRegistrar, Module};

/// The generation module. Registers the Generate workspace, its six dock panels,
/// the Results/History tray panels, and the `gen.*` actions. It reuses sprite-edit's
/// shared Console panel and tools by id, never re-registering them.
pub struct GenerationModule;

impl Module for GenerationModule {
    fn id(&self) -> &'static str {
        "generation"
    }

    fn register(&self, host: &mut dyn HostRegistrar) {
        generate::register(host);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn module_id_is_generation() {
        assert_eq!(GenerationModule.id(), "generation");
    }
}
