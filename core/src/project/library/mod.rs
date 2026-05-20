//! Project library: entities, groups, tags, and AI metadata.
//!
//! A Pixhaus project is a [`Library`] of named [`Entity`] values. The
//! kind of an entity determines its content shape: a `Tileset` entity
//! holds a single tileset, a `Tilemap` entity holds a level scene that
//! references one or more tilesets, and a `Custom` entity is the user's
//! free-form kind (Hero, Goblin, Treasure-Chest, Vehicle, ...) and holds
//! named states each backed by a [`Sprite`](super::Sprite). Custom
//! entities may also carry a structured reference sheet that AI verbs use
//! as the sprite's consistency anchor.
//!
//! # Module layout
//!
//! The model is split by concern so each file stays focused:
//!
//! - [`core`] — the [`Library`] container, [`Entity`], content/kind,
//!   groups, tilemap scenes, and [`ActiveTarget`].
//! - [`tags`] — [`TagDefinition`].
//! - [`ai`] — [`ProjectAi`], [`AiMetadata`], model-routing enums, and
//!   `LoRA` training jobs.
//! - [`reference_sheets`] — [`ReferenceSheet`], [`SheetVariant`],
//!   templates, panel composition, refinement, and prompt provenance.
//! - [`assets`] — the reusable [`AssetLibrary`] (saved references,
//!   character cards, style swatches, trained `LoRA`s).
//!
//! Palettes are not a submodule here: [`Palette`](super::Palette) and
//! its parts already live in `project::palette`; the library only holds a
//! `Vec<Palette>`.
//!
//! # Design notes
//!
//! - The kind enum is exactly three variants. The data model deliberately
//!   does not bake game-genre taxonomy — "Character", "Enemy", "Hero" all
//!   live in the user-typed string carried by `EntityKind::Custom`.
//! - Groups are optional and never auto-created. A pristine project has
//!   no entities and no groups.
//! - Tilesets are project-level so a single Forest tileset can back
//!   multiple Forest-1, Forest-2, Boss-Arena tilemap scenes (the Tiled
//!   `firstgid` model).
//! - Custom-kind entities carry an optional [`ReferenceSheet`] inside
//!   [`EntityContent::Sprites`]. When present, that sheet is the AI
//!   generation anchor for every state in the entity.

pub mod ai;
pub mod assets;
pub mod core;
pub mod reference_sheets;
pub mod tags;

pub use ai::{
    AiMetadata, ModelId, OperationKind, ProjectAi, PromptHistoryEntry, Quality, TrainingJob,
    TrainingStatus, default_reference_chroma,
};
pub use assets::{AssetLibrary, CharacterCard, LoraAsset, LoraKind, ReferenceAsset, StyleSwatch};
pub use core::{
    ActiveTarget, Entity, EntityContent, EntityDefaults, EntityGroup, EntityKind, Library,
    NamedSprite, TilemapLayer, TilemapScene, TilesetReference,
};
pub use reference_sheets::{
    AssetInfo, ChatTranscript, ChatTurn, GenerationProvenance, PromptEntry, PromptResult,
    ReferenceImage, ReferenceRole, ReferenceSheet, ReferenceSheetTemplateDefinition,
    ReferenceSheetTemplateId, ReferenceSlot, RefinementKind, RegionDefinition, SheetComposition,
    SheetDimensions, SheetPanel, SheetVariant, VariantOrigin, built_in_reference_sheet_templates,
};
pub use tags::TagDefinition;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::project::color::Rgba;
    use crate::project::id::{EntityId, SheetVariantId, StateId};
    use crate::project::sprite::Sprite;
    use serde::{Deserialize, Serialize};
    use std::collections::BTreeMap;
    use ts_rs::{Config, TS};

    fn round_trip<T>(value: &T) -> T
    where
        T: Serialize + for<'de> Deserialize<'de>,
    {
        let bytes = rmp_serde::to_vec_named(value).unwrap();
        rmp_serde::from_slice(&bytes).unwrap()
    }

    #[test]
    fn empty_library_round_trips() {
        let l = Library::default();
        assert!(l.is_empty());
        let back: Library = round_trip(&l);
        assert_eq!(l, back);
    }

    #[test]
    fn entity_kind_custom_carries_string() {
        let k = EntityKind::Custom("Character".into());
        let json = serde_json::to_string(&k).unwrap();
        assert_eq!(json, r#"{"kind":"Custom","value":"Character"}"#);
        let back: EntityKind = serde_json::from_str(&json).unwrap();
        assert_eq!(k, back);
    }

    #[test]
    fn export_bindings_entitycontent() {
        EntityContent::export_all(&Config::from_env()).expect("export EntityContent binding");
    }

    #[test]
    fn export_bindings_referencesheet() {
        ReferenceSheet::export_all(&Config::from_env()).expect("export ReferenceSheet binding");
    }

    #[test]
    fn entity_kind_unit_variants_serialise_compactly() {
        let k = EntityKind::Tileset;
        let json = serde_json::to_string(&k).unwrap();
        assert_eq!(json, r#"{"kind":"Tileset"}"#);
    }

    #[test]
    fn active_target_none_is_default() {
        assert!(ActiveTarget::default().is_none());
    }

    #[test]
    fn active_target_state_round_trips() {
        let t = ActiveTarget::State {
            entity_id: EntityId::new(7),
            state_id: StateId::new(3),
        };
        let back: ActiveTarget = round_trip(&t);
        assert_eq!(t, back);
    }

    #[test]
    fn empty_helpers_match_default() {
        assert!(EntityDefaults::default().is_empty());
        assert!(AiMetadata::default().is_empty());
        assert!(ProjectAi::default().is_empty());
        assert!(SheetComposition::default().is_empty());
        assert!(AssetInfo::default().is_empty());
    }

    #[test]
    fn default_reference_chroma_is_magenta() {
        assert_eq!(default_reference_chroma(), Rgba::opaque(255, 0, 255));
    }

    #[test]
    fn built_in_templates_include_expected_turnaround_defaults() {
        let templates = built_in_reference_sheet_templates();
        let turnaround = templates
            .iter()
            .find(|template| template.id == ReferenceSheetTemplateId::Turnaround4View)
            .expect("turnaround template");

        assert_eq!(
            turnaround.default_dimensions,
            SheetDimensions {
                width: 2048,
                height: 1024,
            }
        );
        assert_eq!(turnaround.default_chroma, default_reference_chroma());
        assert!(!turnaround.benefits_from_text_labels);
        assert!(
            turnaround
                .allowed_dimensions
                .contains(&turnaround.default_dimensions)
        );
    }

    #[test]
    fn sheet_variant_from_image_uses_manual_import_defaults() {
        let image = ReferenceImage {
            bytes: vec![1, 2, 3],
            mime: "image/png".into(),
        };
        let variant = SheetVariant::from_image(SheetVariantId::new(7), 123, image.clone());

        assert_eq!(variant.id, SheetVariantId::new(7));
        assert_eq!(variant.created_at, 123);
        assert_eq!(variant.image, image);
        assert_eq!(variant.template, ReferenceSheetTemplateId::Custom);
        assert_eq!(variant.chroma_color, default_reference_chroma());
        assert_eq!(variant.model, ModelId::Auto);
        assert_eq!(variant.quality, Quality::Medium);
        assert_eq!(variant.origin, VariantOrigin::ManualImport);
        assert!(variant.references.is_empty());
        assert!(!variant.promotion);
    }

    /// Pins the boxing decision for [`EntityContent`].
    ///
    /// `ReferenceSheet` is boxed inside `EntityContent::Sprites` so
    /// embedding optional reference sheets does not drag the enum's
    /// stack footprint up to the full sheet size.
    ///
    /// The cap is `1.5 * size_of::<Sprite>()` — generous enough to
    /// absorb small additions without churning the boxing decision,
    /// tight enough to fire if a variant payload starts dominating the
    /// enum again. When the test fires: either box the new offender or
    /// update both the cap and the rustdoc above `EntityContent` with
    /// the new measurement.
    #[test]
    fn entity_content_size_is_bounded() {
        use std::mem::size_of;

        let sprite = size_of::<Sprite>();
        let sheet = size_of::<ReferenceSheet>();
        let content = size_of::<EntityContent>();

        // ReferenceSheet larger than Sprite -> keep it boxed inside
        // the Sprites variant's optional reference slot.
        assert!(
            sheet > sprite,
            "ReferenceSheet ({sheet} bytes) shrank below Sprite ({sprite} bytes); \
             consider un-boxing the embedded reference_sheet"
        );

        let cap = sprite + sprite / 2;
        assert!(
            content <= cap,
            "EntityContent grew to {content} bytes; cap is {cap} \
             (1.5 * size_of::<Sprite>() = 1.5 * {sprite}). Box the \
             outgrown variant or update the cap with a recorded \
             measurement."
        );
    }

    /// Pins the `MessagePack` wire shape for [`EntityKind::Custom`].
    ///
    /// The JSON-side counterpart at `entity_kind_custom_carries_string`
    /// already covers the human-readable form. `rmp-serde` with
    /// `to_vec_named` writes a map of string keys to string values;
    /// decoding back into `BTreeMap<String, String>` and asserting both
    /// the `"kind"` and `"value"` entries pin the wire format against
    /// silent serde-attribute drift (`tag` / `content` rename).
    #[test]
    fn entity_kind_custom_messagepack_shape() {
        let k = EntityKind::Custom("Character".into());
        let bytes = rmp_serde::to_vec_named(&k).expect("encode");
        let decoded: BTreeMap<String, String> =
            rmp_serde::from_slice(&bytes).expect("decode generic");

        assert_eq!(decoded.len(), 2);
        assert_eq!(decoded.get("kind").map(String::as_str), Some("Custom"));
        assert_eq!(decoded.get("value").map(String::as_str), Some("Character"));
    }
}
