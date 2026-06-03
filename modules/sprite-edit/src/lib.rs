//! Pixhaus sprite-editing module: the Draw workspace and the shared editing core.
//!
//! Registers the Draw workspace, the shared Layers/Sprites/Palette/Selection
//! Actions/AI Assistant dock panels, the shared Frames/Assets/Console tray
//! panels, and the 15 shared editing tools (architecture bible sections 7.3,
//! 6.3). This module registers FIRST: its shared panels and tools are the ids the
//! other workspaces reference by value (bible rule 2), so they must exist before
//! any other module's layout names them.
#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used))]

mod draw;
mod tools;

pub use draw::{AI_ASSISTANT, ASSETS, CONSOLE, DRAW, FRAMES, LAYERS, PALETTE, SELECTION_ACTIONS, SPRITES};

use pixhaus_ui::contrib_api::{HostRegistrar, Module};

/// The sprite-editing module. Registers the shared editing surface (the 15 tools
/// and the shared panels) and the Draw workspace.
pub struct SpriteEditModule;

impl Module for SpriteEditModule {
    fn id(&self) -> &'static str {
        "sprite-edit"
    }

    fn register(&self, host: &mut dyn HostRegistrar) {
        // Tools first, then the workspace + shared panels + actions + menus.
        tools::register(host);
        draw::register(host);
        tracing::info!(module = "sprite-edit", "registered the sprite-editing surface");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn module_id_is_sprite_edit() {
        assert_eq!(SpriteEditModule.id(), "sprite-edit");
    }
}
