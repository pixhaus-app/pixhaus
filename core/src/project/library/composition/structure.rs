//! The layout contract. A Structure defines the canvas and panels; the
//! `ai::compose` resolver derives both layout prose and slice rectangles
//! from it, so prose and geometry cannot desync.

use serde::{Deserialize, Serialize};
use ts_rs::TS;

use super::Dimensions;

/// Stable id for a Structure. Built-ins use reverse-DNS
/// (`pixhaus.builtin.structure.character`); a project record reuses that id
/// to shadow the built-in, or takes a fresh project slug.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct StructureId(pub String);

/// Canvas layout contract for AI generation.
#[allow(missing_docs)]
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct Structure {
    pub id: StructureId,
    pub name: String,
    pub output: StructureOutput,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub layout_negatives: String,
}

/// Output shape of a Structure.
#[allow(missing_docs)]
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export)]
pub enum StructureOutput {
    /// One free-composition image; no panels.
    Single,
    /// Structured multi-panel sheet.
    Paneled {
        canvas: Dimensions,
        panels: Vec<StructurePanel>,
    },
}

/// One named panel within a paneled structure.
#[allow(missing_docs)]
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct StructurePanel {
    pub label: String,
    pub rect: PanelRect,
    /// Prose with `{canvas_w}`, `{canvas_h}`, `{panel_w}`, `{panel_h}`,
    /// `{label}` tokens interpolated by the resolver.
    pub prose_fragment: String,
    pub slot: PanelSlot,
}

/// Pixel bounding box for a panel within the canvas.
#[allow(missing_docs)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct PanelRect {
    pub x: u32,
    pub y: u32,
    pub w: u32,
    pub h: u32,
}

/// Semantic role of a panel in a structure.
#[allow(missing_docs)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export)]
pub enum PanelSlot {
    View,
    Expression,
    Callout,
    Outfit,
    PaletteSwatch,
    Generic,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample() -> Structure {
        Structure {
            id: StructureId("test.s".into()),
            name: "Test".into(),
            output: StructureOutput::Paneled {
                canvas: Dimensions {
                    width: 100,
                    height: 200,
                },
                panels: vec![StructurePanel {
                    label: "front".into(),
                    rect: PanelRect {
                        x: 0,
                        y: 0,
                        w: 50,
                        h: 100,
                    },
                    prose_fragment: "front view {panel_w}x{panel_h}".into(),
                    slot: PanelSlot::View,
                }],
            },
            layout_negatives: "overlapping views".into(),
        }
    }

    #[test]
    fn structure_round_trips() {
        let s = sample();
        let json = serde_json::to_string(&s).unwrap();
        let back: Structure = serde_json::from_str(&json).unwrap();
        assert_eq!(s, back);
    }

    #[test]
    fn single_output_serializes_as_snake_case() {
        let json = serde_json::to_string(&StructureOutput::Single).unwrap();
        assert_eq!(json, r#""single""#);
    }

    #[test]
    fn panel_slot_serializes_as_snake_case() {
        assert_eq!(
            serde_json::to_string(&PanelSlot::PaletteSwatch).unwrap(),
            r#""palette_swatch""#
        );
    }
}
