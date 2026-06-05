//! Anchors: the stable creative references generation must preserve (bible 5).
//!
//! An anchor answers "what must remain consistent?" — a character's silhouette, a
//! palette, a walk-cycle timing. It carries a kind, a strength, a free-text
//! statement, and optional structured content. Anchors are the model's defence
//! against generation drift; enforcement lives in the prompt compiler and Generate
//! workspace, not here.

use serde::{Deserialize, Serialize};

/// What an anchor pins down (bible 5.2).
#[derive(Copy, Clone, PartialEq, Eq, Hash, Debug, PartialOrd, Ord, Serialize, Deserialize)]
pub enum AnchorKind {
    /// Who or what an entity is: canonical description, must-haves, forbidden changes.
    Identity,
    /// Visual appearance: silhouette, shape language, line treatment, proportions.
    Visual,
    /// Colors that must be respected: required, optional, forbidden, role mapping.
    Palette,
    /// Rendering language: shading, edge treatment, anti-aliasing, lighting.
    Style,
    /// Motion consistency: pose beats, timing, looping, motion arcs, contact rules.
    Animation,
    /// Size and proportion relationships: sprite size, relative scale, grid size.
    Scale,
    /// Story, personality, and world constraints.
    Lore,
    /// What must not happen (e.g. "do not add extra limbs").
    Negative,
}

/// How firmly an anchor holds (bible 5.3).
#[derive(Copy, Clone, PartialEq, Eq, Hash, Debug, PartialOrd, Ord, Default, Serialize, Deserialize)]
pub enum AnchorStrength {
    /// Influence only.
    Loose,
    /// Prefer strongly.
    #[default]
    Normal,
    /// Preserve unless impossible.
    Strong,
    /// Must not change.
    Locked,
}

/// A stable creative reference generation should preserve.
///
/// `statement` is the artist-authored sentence the anchor enforces. `structured`
/// holds optional machine-usable detail (e.g. required colors, pose beats) the
/// prompt compiler can fold in; it stays free-form text this pass so every anchor
/// kind shares one shape.
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub struct Anchor {
    /// What this anchor pins down.
    pub kind: AnchorKind,
    /// How firmly it holds.
    pub strength: AnchorStrength,
    /// The artist-authored constraint, in plain words.
    pub statement: String,
    /// Optional structured detail the prompt compiler can use; empty when unused.
    pub structured: Vec<String>,
}

impl Anchor {
    /// An anchor of `kind` and `strength` carrying `statement`, with no structured
    /// detail.
    pub fn new(kind: AnchorKind, strength: AnchorStrength, statement: impl Into<String>) -> Self {
        Self {
            kind,
            strength,
            statement: statement.into(),
            structured: Vec::new(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_anchor_holds_its_fields() {
        let a = Anchor::new(AnchorKind::Visual, AnchorStrength::Strong, "keep the round head");
        assert_eq!(a.kind, AnchorKind::Visual);
        assert_eq!(a.strength, AnchorStrength::Strong);
        assert_eq!(a.statement, "keep the round head");
        assert!(a.structured.is_empty());
    }

    #[test]
    fn strength_defaults_to_normal_and_orders() {
        assert_eq!(AnchorStrength::default(), AnchorStrength::Normal);
        assert!(AnchorStrength::Loose < AnchorStrength::Locked);
    }
}
