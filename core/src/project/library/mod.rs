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
//! - [`core`] — the [`Library`] container, [`Entity`], content/kind,
//!   groups, tilemap scenes, and [`ActiveTarget`].
//! - [`tags`] — [`TagDefinition`].
//! - [`ai`] — [`ProjectAi`], [`AiMetadata`], model-routing enums, and
//!   `LoRA` training jobs.
//! - [`reference_sheets`] — [`ReferenceSheet`], [`SheetVariant`],
//!   templates, panel composition, refinement, and prompt provenance.
//! - [`assets`] — the reusable [`AssetLibrary`] (saved references,
//!   character cards, style swatches, trained `LoRA`s).

pub mod ai;
pub mod assets;
pub mod composition;
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
    AnchorDirection, AnimationKind, AssetInfo, CharacterAnchor, ChatTranscript, ChatTurn,
    DerivedSheet, DirectionalAnchors, GenerationProvenance, PromptEntry, PromptResult,
    ReferenceImage, ReferenceRole, ReferenceSheet, ReferenceSheetTemplateDefinition,
    ReferenceSheetTemplateId, ReferenceSlot, RefinementKind, RegionDefinition, SheetComposition,
    SheetDimensions, SheetPanel, SheetVariant, VariantOrigin, built_in_reference_sheet_templates,
};
pub use tags::TagDefinition;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::project::color::Rgba;
    use crate::project::id::SheetVariantId;
    use crate::project::sprite::Sprite;

    #[test]
    fn empty_library_round_trips() {
        let l = Library::default();
        assert!(l.is_empty());
        let json = serde_json::to_string(&l).unwrap();
        let back: Library = serde_json::from_str(&json).unwrap();
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
        // Non-image bytes fall back to the template default dimensions.
        assert_eq!((variant.width, variant.height), (2048, 1024));
    }

    #[test]
    fn sheet_variant_from_image_reads_png_dimensions() {
        use image::{ImageFormat, RgbaImage};
        use std::io::Cursor;

        let mut bytes = Vec::new();
        RgbaImage::new(7, 3)
            .write_to(&mut Cursor::new(&mut bytes), ImageFormat::Png)
            .unwrap();
        let variant = SheetVariant::from_image(
            SheetVariantId::new(1),
            0,
            ReferenceImage {
                bytes,
                mime: "image/png".into(),
            },
        );

        assert_eq!(
            (variant.width, variant.height),
            (7, 3),
            "from_image should read real dimensions from the PNG header"
        );
    }

    /// Pins the boxing decision for [`EntityContent`].
    ///
    /// `ReferenceSheet` is boxed inside `EntityContent::Sprites` so
    /// embedding optional reference sheets does not drag the enum's
    /// stack footprint up to the full sheet size.
    #[test]
    fn entity_content_size_is_bounded() {
        use std::mem::size_of;

        let sprite = size_of::<Sprite>();
        let sheet = size_of::<ReferenceSheet>();
        let content = size_of::<EntityContent>();

        assert!(
            sheet > sprite,
            "ReferenceSheet ({sheet} bytes) shrank below Sprite ({sprite} bytes); \
             consider un-boxing the embedded reference_sheet"
        );

        let cap = sprite + sprite / 2;
        assert!(
            content <= cap,
            "EntityContent grew to {content} bytes; cap is {cap} \
             (1.5 * size_of::<Sprite>() = 1.5 * {sprite})."
        );
    }
}
