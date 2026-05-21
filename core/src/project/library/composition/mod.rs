//! User-managed composition library: Structures, Styles, and Prompts that
//! drive AI generation. See docs/planning/work/prompt-style-structure-library.md.

mod prompt;
mod structure;
mod style;

pub use prompt::{PromptId, PromptTemplate, PromptVariable};
pub use structure::{
    PanelRect, PanelSlot, Structure, StructureId, StructureOutput, StructurePanel,
};
pub use style::{Style, StyleId};

use serde::{Deserialize, Serialize};
use ts_rs::TS;

/// Pixel canvas size for a paneled structure.
#[allow(missing_docs)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct Dimensions {
    pub width: u32,
    pub height: u32,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dimensions_round_trip() {
        let d = Dimensions {
            width: 1024,
            height: 1536,
        };
        let json = serde_json::to_string(&d).unwrap();
        let back: Dimensions = serde_json::from_str(&json).unwrap();
        assert_eq!(d, back);
    }

    #[test]
    fn export_bindings_composition() {
        use ts_rs::Config;
        let cfg = Config::from_env();
        Dimensions::export_all(&cfg).expect("export Dimensions");
        Structure::export_all(&cfg).expect("export Structure");
        Style::export_all(&cfg).expect("export Style");
        PromptTemplate::export_all(&cfg).expect("export PromptTemplate");
    }
}
