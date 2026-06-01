//! Pixhaus tiles module: the Tiles workspace and tileset authoring (bible 7.3).
//!
//! Registers the Tiles workspace, the Tileset / Rule Type / Material / Seam QA /
//! AI Tile Assistant dock panels, and the Tile Variants tray panel. Tiles reuses
//! sprite-edit's shared Assets and Console panels by id - tile editing is the
//! shared editing core with tile-aware overlays (bible rule 2), so those panels
//! are referenced by id, never re-registered.
#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used))]

mod tiles_ws;

use pixhaus_ui::contrib_api::{HostRegistrar, Module};

/// The tiles module. Registers the Tiles workspace, its own panels, and the
/// AI-tile actions.
pub struct TilesModule;

impl Module for TilesModule {
    fn id(&self) -> &'static str {
        "tiles"
    }

    fn register(&self, host: &mut dyn HostRegistrar) {
        tiles_ws::register(host);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn module_id_is_tiles() {
        assert_eq!(TilesModule.id(), "tiles");
    }
}
