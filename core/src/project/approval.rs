//! Sprite reference-sheet approval flow.
//!
//! When a user clicks "Approve as canonical" on a [`SheetVariant`] in
//! the history strip, the editor:
//!
//! 1. Moves the chosen variant from `history` into `canonical`.
//! 2. Demotes the previous canonical variant to `history[0]`.
//! 3. Extracts a palette from the new canonical's image bytes (the
//!    [`crate::color::extraction`] module does the work) and writes it
//!    to the new canonical's [`SheetVariant::extracted_palette`].
//!
//! Step 3 only runs when the variant didn't already carry an extracted
//! palette: B10.1 generators may pre-populate it during synthesis, in
//! which case this module preserves what the generator produced.
//!
//! No I/O happens here. The function takes a `&mut Project`, runs in
//! O(history.len()) (one `Vec::remove` + one `Vec::insert(0, _)` over
//! the variant history — bounded to ~10–100 entries per brief), and
//! returns the [`Approval`] receipt the caller can hand back through
//! the IPC bridge.

use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::color::extraction::{ExtractionOptions, extract_palette_from_image_bytes};
use crate::project::id::{EntityId, SheetVariantId};
use crate::project::library::{EntityContent, ReferenceSheet, SheetVariant};

use super::Project;

/// Errors produced by [`approve_sheet_variant`].
#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum ApprovalError {
    /// The named entity was not found in the project library.
    #[error("entity {0} not found in project library")]
    EntityNotFound(u32),

    /// The entity exists but does not carry an embedded sprite reference
    /// sheet.
    #[error("entity {0} has no sprite reference sheet")]
    NoReferenceSheet(u32),

    /// The variant id wasn't in the reference sheet's history *or* the
    /// canonical slot.
    #[error("variant {0} is not present on entity {1}'s sheet")]
    VariantNotFound(u32, u32),
}

/// Receipt returned by a successful approval.
///
/// The shape is small and serializable so the IPC layer can return it
/// to the UI without re-wrapping. The new canonical variant id and the
/// number of palette swatches extracted are the two facts the UI cares
/// about — both feed the toast / status indicator.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct Approval {
    /// The entity whose sheet was approved.
    pub entity_id: EntityId,
    /// The variant now sitting in `canonical`.
    pub canonical_id: SheetVariantId,
    /// The previous canonical variant id, if there was one. Will be
    /// `None` only on first approval of a freshly imported entity that
    /// somehow had no prior canonical (the data model always provides
    /// one, so in practice this is `Some`).
    pub previous_canonical_id: Option<SheetVariantId>,
    /// Number of swatches the extractor produced for the new canonical.
    /// Zero when the variant already carried an extracted palette
    /// (preserved from the generator) or when extraction failed.
    pub palette_size: usize,
}

/// Approves a [`SheetVariant`] as the canonical reference sheet of a
/// sprite entity.
///
/// The approval workflow:
///
/// - Find the entity by `entity_id`. If it does not carry an embedded
///   reference sheet, return [`ApprovalError::NoReferenceSheet`].
/// - If `variant_id` is already canonical, just (re-)extract the palette
///   and return the no-op receipt. This makes the operation idempotent
///   so the UI doesn't have to special-case "already canonical".
/// - Otherwise locate the variant in `history`, swap it into
///   `canonical`, and prepend the displaced canonical to `history`.
/// - Run [`extract_palette_from_image_bytes`] over the new canonical's
///   image bytes. If the variant's `extracted_palette` is non-empty
///   already, leave it alone.
///
/// # Errors
///
/// - [`ApprovalError::EntityNotFound`] — no entity with that id.
/// - [`ApprovalError::NoReferenceSheet`] — entity has no embedded sheet.
/// - [`ApprovalError::VariantNotFound`] — neither the canonical nor any
///   history entry carries `variant_id`.
pub fn approve_sheet_variant(
    project: &mut Project,
    entity_id: EntityId,
    variant_id: SheetVariantId,
    options: ExtractionOptions,
) -> Result<Approval, ApprovalError> {
    let entity = project
        .library
        .entities
        .iter_mut()
        .find(|e| e.id == entity_id)
        .ok_or(ApprovalError::EntityNotFound(entity_id.get()))?;

    let sheet = embedded_sheet_mut(&mut entity.content)
        .ok_or(ApprovalError::NoReferenceSheet(entity_id.get()))?;

    let previous_canonical_id = Some(sheet.canonical.id);

    if sheet.canonical.id == variant_id {
        let palette_size = ensure_extracted_palette(&mut sheet.canonical, options);
        return Ok(Approval {
            entity_id,
            canonical_id: variant_id,
            previous_canonical_id,
            palette_size,
        });
    }

    let pos = sheet
        .history
        .iter()
        .position(|v| v.id == variant_id)
        .ok_or_else(|| ApprovalError::VariantNotFound(variant_id.get(), entity_id.get()))?;

    promote_history_to_canonical(sheet, pos);

    let palette_size = ensure_extracted_palette(&mut sheet.canonical, options);

    Ok(Approval {
        entity_id,
        canonical_id: variant_id,
        previous_canonical_id,
        palette_size,
    })
}

/// Swaps `history[pos]` with the canonical variant; the previous
/// canonical lands at `history[0]` (newest first).
fn promote_history_to_canonical(sheet: &mut ReferenceSheet, pos: usize) {
    let new_canonical = sheet.history.remove(pos);
    let old_canonical = std::mem::replace(&mut sheet.canonical, new_canonical);
    sheet.history.insert(0, old_canonical);
}

/// Runs the eyedropper extractor on the variant's image bytes if the
/// `extracted_palette` field is empty.
///
/// Returns the number of swatches now stored on the variant. A decode
/// failure (corrupt bytes) falls back to `0` rather than surfacing the
/// error: the approval should still succeed because the rest of the
/// workflow (the canonical move) is what matters for consistency.
fn ensure_extracted_palette(variant: &mut SheetVariant, options: ExtractionOptions) -> usize {
    if !variant.extracted_palette.is_empty() {
        return variant.extracted_palette.len();
    }

    let pal = extract_palette_from_image_bytes(&variant.image.bytes, options).unwrap_or_default();
    variant.extracted_palette = pal;
    variant.extracted_palette.len()
}

fn embedded_sheet_mut(content: &mut EntityContent) -> Option<&mut ReferenceSheet> {
    match content {
        EntityContent::Sprites {
            reference_sheet: Some(sheet),
            ..
        } => Some(sheet.as_mut()),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use image::{ImageBuffer, ImageFormat, RgbaImage};

    use crate::project::library::{
        AiMetadata, AssetInfo, Entity, EntityContent, EntityDefaults, EntityKind, ReferenceImage,
        SheetComposition,
    };
    use crate::project::user_data::UserData;

    use super::*;

    fn solid_png(r: u8, g: u8, b: u8) -> Vec<u8> {
        let img: RgbaImage = ImageBuffer::from_pixel(2, 2, image::Rgba([r, g, b, 255]));
        let mut buf = Vec::new();
        img.write_to(&mut std::io::Cursor::new(&mut buf), ImageFormat::Png)
            .unwrap();
        buf
    }

    fn variant(id: u32, png: Vec<u8>) -> SheetVariant {
        SheetVariant {
            id: SheetVariantId::new(id),
            generated_at: 0,
            image: ReferenceImage {
                bytes: png,
                mime: "image/png".into(),
            },
            composition: SheetComposition::default(),
            generation: None,
            extracted_palette: Vec::new(),
        }
    }

    fn build_project_with_sheet(canonical_id: u32, history_ids: &[u32]) -> Project {
        let mut project = Project::new("approval-test");
        let canonical = variant(canonical_id, solid_png(255, 0, 0));
        let history: Vec<SheetVariant> = history_ids
            .iter()
            .copied()
            .map(|i| variant(i, solid_png(0, u8::try_from(i & 0xFF).unwrap_or(0), 0)))
            .collect();
        let entity = Entity {
            id: EntityId::new(1),
            kind: EntityKind::Custom("Character".into()),
            name: "Hero".into(),
            group_id: None,
            tags: Vec::new(),
            defaults: EntityDefaults::default(),
            content: EntityContent::Sprites {
                states: Vec::new(),
                reference_sheet: Some(Box::new(ReferenceSheet {
                    canonical,
                    history,
                    prompts: Vec::new(),
                    info: AssetInfo {
                        fields: BTreeMap::new(),
                        notes: Vec::new(),
                    },
                })),
            },
            ai: AiMetadata::default(),
            user_data: UserData::default(),
            created_at: 0,
            updated_at: 0,
        };
        project.library.entities.push(entity);
        project
    }

    #[test]
    fn promotes_history_variant_to_canonical() {
        let mut project = build_project_with_sheet(10, &[20, 30, 40]);

        let receipt = approve_sheet_variant(
            &mut project,
            EntityId::new(1),
            SheetVariantId::new(30),
            ExtractionOptions::default(),
        )
        .unwrap();

        assert_eq!(receipt.canonical_id, SheetVariantId::new(30));
        assert_eq!(receipt.previous_canonical_id, Some(SheetVariantId::new(10)));

        let entity = &project.library.entities[0];
        let EntityContent::Sprites {
            reference_sheet: Some(sheet),
            ..
        } = &entity.content
        else {
            panic!("expected embedded reference sheet");
        };
        assert_eq!(sheet.canonical.id, SheetVariantId::new(30));
        // Previous canonical is now first in history (newest first).
        assert_eq!(sheet.history[0].id, SheetVariantId::new(10));
        // Other history entries kept their order.
        assert_eq!(sheet.history[1].id, SheetVariantId::new(20));
        assert_eq!(sheet.history[2].id, SheetVariantId::new(40));
    }

    #[test]
    fn extracts_palette_when_variant_has_none() {
        let mut project = build_project_with_sheet(1, &[2]);
        let receipt = approve_sheet_variant(
            &mut project,
            EntityId::new(1),
            SheetVariantId::new(2),
            ExtractionOptions::default(),
        )
        .unwrap();
        assert!(receipt.palette_size >= 1, "palette should have one swatch");

        let entity = &project.library.entities[0];
        let EntityContent::Sprites {
            reference_sheet: Some(sheet),
            ..
        } = &entity.content
        else {
            panic!("expected embedded reference sheet");
        };
        assert!(!sheet.canonical.extracted_palette.is_empty());
    }

    #[test]
    fn preserves_existing_extracted_palette() {
        use crate::project::Rgba;
        use crate::project::palette::PaletteEntry;

        let mut project = build_project_with_sheet(1, &[2]);
        // Pre-populate the to-be-canonical variant's palette.
        if let EntityContent::Sprites {
            reference_sheet: Some(sheet),
            ..
        } = &mut project.library.entities[0].content
        {
            sheet.history[0].extracted_palette = vec![PaletteEntry::new(Rgba::opaque(7, 7, 7))];
        }

        let receipt = approve_sheet_variant(
            &mut project,
            EntityId::new(1),
            SheetVariantId::new(2),
            ExtractionOptions::default(),
        )
        .unwrap();

        let entity = &project.library.entities[0];
        let EntityContent::Sprites {
            reference_sheet: Some(sheet),
            ..
        } = &entity.content
        else {
            panic!("expected embedded reference sheet");
        };
        assert_eq!(receipt.palette_size, 1);
        assert_eq!(
            sheet.canonical.extracted_palette[0].color,
            Rgba::opaque(7, 7, 7),
            "pre-existing palette must be preserved"
        );
    }

    #[test]
    fn approving_already_canonical_variant_is_idempotent() {
        let mut project = build_project_with_sheet(1, &[2]);
        let receipt = approve_sheet_variant(
            &mut project,
            EntityId::new(1),
            SheetVariantId::new(1),
            ExtractionOptions::default(),
        )
        .unwrap();
        assert_eq!(receipt.canonical_id, SheetVariantId::new(1));

        let entity = &project.library.entities[0];
        let EntityContent::Sprites {
            reference_sheet: Some(sheet),
            ..
        } = &entity.content
        else {
            panic!("expected embedded reference sheet");
        };
        assert_eq!(sheet.canonical.id, SheetVariantId::new(1));
        assert_eq!(sheet.history.len(), 1);
        assert_eq!(sheet.history[0].id, SheetVariantId::new(2));
    }

    #[test]
    fn missing_entity_is_an_error() {
        let mut project = Project::new("empty");
        let err = approve_sheet_variant(
            &mut project,
            EntityId::new(99),
            SheetVariantId::new(1),
            ExtractionOptions::default(),
        )
        .unwrap_err();
        assert_eq!(err, ApprovalError::EntityNotFound(99));
    }

    #[test]
    fn non_reference_entity_is_an_error() {
        let mut project = Project::new("non-ref");
        project.library.entities.push(Entity {
            id: EntityId::new(5),
            kind: EntityKind::Custom("Hero".into()),
            name: "Hero".into(),
            group_id: None,
            tags: Vec::new(),
            defaults: EntityDefaults::default(),
            content: EntityContent::Sprites {
                states: Vec::new(),
                reference_sheet: None,
            },
            ai: AiMetadata::default(),
            user_data: UserData::default(),
            created_at: 0,
            updated_at: 0,
        });
        let err = approve_sheet_variant(
            &mut project,
            EntityId::new(5),
            SheetVariantId::new(1),
            ExtractionOptions::default(),
        )
        .unwrap_err();
        assert_eq!(err, ApprovalError::NoReferenceSheet(5));
    }

    #[test]
    fn unknown_variant_id_is_an_error() {
        let mut project = build_project_with_sheet(1, &[2]);
        let err = approve_sheet_variant(
            &mut project,
            EntityId::new(1),
            SheetVariantId::new(99),
            ExtractionOptions::default(),
        )
        .unwrap_err();
        assert_eq!(err, ApprovalError::VariantNotFound(99, 1));
    }

    #[test]
    fn corrupt_image_bytes_yield_zero_swatches_but_no_error() {
        let mut project = build_project_with_sheet(1, &[2]);
        // Stomp the to-be-canonical's image bytes with garbage.
        if let EntityContent::Sprites {
            reference_sheet: Some(sheet),
            ..
        } = &mut project.library.entities[0].content
        {
            sheet.history[0].image.bytes = b"not a png".to_vec();
        }
        let receipt = approve_sheet_variant(
            &mut project,
            EntityId::new(1),
            SheetVariantId::new(2),
            ExtractionOptions::default(),
        )
        .unwrap();
        assert_eq!(receipt.palette_size, 0);
    }
}
