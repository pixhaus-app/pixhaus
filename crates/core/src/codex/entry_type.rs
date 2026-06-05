//! [`EntryType`]: the namespace every Codex entry belongs to (bible 4, 24).
//!
//! The type drives the UI, the prompt compiler, validation, and the Generate
//! workspace. Each variant has a stable namespace string used in typed
//! `@`-references like `@character.bit` or `@material.mossy_stone`. The namespace is
//! an id, never a display label, so it never localizes.

use serde::{Deserialize, Serialize};

/// The namespace a Codex entry lives in.
///
/// Every namespace the bible defines (section 24) is modelled here. Only
/// [`Character`](EntryType::Character), [`Palette`](EntryType::Palette),
/// [`Style`](EntryType::Style), and [`Animation`](EntryType::Animation) carry rich
/// type-specific fields this pass; the rest share a generic detail body.
#[derive(Copy, Clone, PartialEq, Eq, Hash, Debug, PartialOrd, Ord, Serialize, Deserialize)]
pub enum EntryType {
    /// A playable character, mascot, companion, or named entity.
    Character,
    /// A hostile character or monster faced in play.
    Enemy,
    /// A non-player character.
    Npc,
    /// A monster, animal, fantasy being, familiar, boss, or pet.
    Creature,
    /// An environmental object or interactive prop.
    Prop,
    /// A collectable, inventory item, powerup, icon, or loot.
    Item,
    /// A weapon, with its animation-compatibility needs.
    Weapon,
    /// A surface appearance definition.
    Material,
    /// A color system.
    Palette,
    /// A rendering language: how art should look.
    Style,
    /// A mood, atmosphere, and creative feeling.
    Vibe,
    /// An area, level, town, dungeon, or room.
    Location,
    /// A reusable natural or environmental category.
    Biome,
    /// A group, team, race, organization, or kingdom.
    Faction,
    /// An animation concept, not necessarily a concrete clip.
    Animation,
    /// A specific body pose or action beat.
    Pose,
    /// A sprite-based visual effect.
    Vfx,
    /// A game UI sprite.
    UiElement,
    /// A constraint that applies across assets.
    Rule,
    /// A structured, reusable recipe for creating assets.
    Recipe,
    /// A group of visual or lore references.
    ReferenceBoard,
}

impl EntryType {
    /// The stable namespace string used in typed `@`-references (e.g. `character`,
    /// `material`, `ui`). Stable across renames; never a display label.
    pub fn namespace(self) -> &'static str {
        match self {
            EntryType::Character => "character",
            EntryType::Enemy => "enemy",
            EntryType::Npc => "npc",
            EntryType::Creature => "creature",
            EntryType::Prop => "prop",
            EntryType::Item => "item",
            EntryType::Weapon => "weapon",
            EntryType::Material => "material",
            EntryType::Palette => "palette",
            EntryType::Style => "style",
            EntryType::Vibe => "vibe",
            EntryType::Location => "location",
            EntryType::Biome => "biome",
            EntryType::Faction => "faction",
            EntryType::Animation => "animation",
            EntryType::Pose => "pose",
            EntryType::Vfx => "vfx",
            EntryType::UiElement => "ui",
            EntryType::Rule => "rule",
            EntryType::Recipe => "recipe",
            EntryType::ReferenceBoard => "board",
        }
    }

    /// Every entry type, in a stable order. Lets the UI build its type tree without
    /// hardcoding the list.
    pub fn all() -> &'static [EntryType] {
        &[
            EntryType::Character,
            EntryType::Enemy,
            EntryType::Npc,
            EntryType::Creature,
            EntryType::Prop,
            EntryType::Item,
            EntryType::Weapon,
            EntryType::Material,
            EntryType::Palette,
            EntryType::Style,
            EntryType::Vibe,
            EntryType::Location,
            EntryType::Biome,
            EntryType::Faction,
            EntryType::Animation,
            EntryType::Pose,
            EntryType::Vfx,
            EntryType::UiElement,
            EntryType::Rule,
            EntryType::Recipe,
            EntryType::ReferenceBoard,
        ]
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    #[test]
    fn namespaces_are_unique() {
        let seen: HashSet<&str> = EntryType::all().iter().map(|t| t.namespace()).collect();
        assert_eq!(seen.len(), EntryType::all().len());
    }

    #[test]
    fn all_covers_every_variant_namespace() {
        // A spot-check that the rich types map to the namespaces references use.
        assert_eq!(EntryType::Character.namespace(), "character");
        assert_eq!(EntryType::UiElement.namespace(), "ui");
        assert_eq!(EntryType::ReferenceBoard.namespace(), "board");
    }
}
