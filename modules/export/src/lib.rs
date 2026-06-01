//! Pixhaus export module: the Export workspace and engine-target export.
//!
//! Registers the Export workspace, the Export Format / Engine Preset / Animation
//! Metadata / QA Warnings dock panels, and the Export Log tray panel (architecture
//! bible sections 7.3, 6.7, 19). Export is production discipline: validate before
//! writing. Codecs live in `io`; this module wires the workspace, presets, and the
//! QA checklist. The engine target is Unity only. This round the panels render mock
//! content and dispatch `export.*` actions through the intent sink.
#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used))]

mod export_ws;

use pixhaus_ui::contrib_api::{HostRegistrar, Module};

/// The export module. Registers the Export workspace, its four dock panels, the
/// Export Log tray panel, and the `export.*` actions. It reuses sprite-edit's shared
/// Console panel and tools by id, never re-registering them.
pub struct ExportModule;

impl Module for ExportModule {
    fn id(&self) -> &'static str {
        "export"
    }

    fn register(&self, host: &mut dyn HostRegistrar) {
        export_ws::register(host);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn module_id_is_export() {
        assert_eq!(ExportModule.id(), "export");
    }
}
