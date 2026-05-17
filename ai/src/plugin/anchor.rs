//! Anchor payload — the consistency reference an AI verb consumes when
//! invoked against a sprite entity that has an embedded reference sheet.
//!
//! The mechanic:
//!
//! - A `Custom`-kind library entity may carry an optional
//!   [`ReferenceSheet`](pixhaus_core::project::ReferenceSheet) inside
//!   `EntityContent::Sprites`.
//! - When an AI verb is invoked against such an entity, the host
//!   builds an [`AnchorPayload`] from its canonical sheet and attaches it to
//!   [`crate::plugin::context::VerbContext::anchor`].
//! - Image-generation verbs read the anchor's image bytes and pass them
//!   as `style_image` on their backend request, plus the anchor's palette
//!   and (if the project trained one) the per-entity `LoRA` path.
//!
//! Verbs that don't naturally consume an anchor (Critique, Conversational)
//! still see the field but can ignore it. The `anchor` field is
//! `Option<...>` so verbs that call `ctx.anchor.as_ref()` get an obvious
//! `None` for the unanchored case.

use base64::Engine as _;
use serde::{Deserialize, Serialize};

use pixhaus_core::project::{
    Entity, EntityContent, EntityId, PaletteEntry, SheetComposition, SheetVariant,
};

/// The payload an AI verb consumes when it runs against an anchored
/// entity.
///
/// Constructed by [`AnchorPayload::from_sprite_entity`]. The host
/// (the IPC layer) calls that constructor when packaging a
/// [`crate::plugin::context::VerbContext`] for invocation, then drops
/// the result into [`crate::plugin::context::VerbContext::anchor`].
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct AnchorPayload {
    /// `EntityId` of the sprite entity the anchor was derived from.
    /// Lets verbs report which sheet drove their generation.
    pub reference_entity_id: EntityId,

    /// Stable hash of the canonical sheet image bytes. Used by the
    /// host's anchor cache to detect when a re-resolve is needed —
    /// equal hash means the cache hit is valid. Verbs can also log it
    /// for traceability.
    pub canonical_hash: u64,

    /// MIME type of the reference image (typically `image/png`).
    pub mime: String,

    /// Raw bytes of the canonical sheet image. Verbs that prefer raw
    /// bytes (and most backend `ImageGenRequest::style_image` fields do)
    /// pass these directly to the backend.
    ///
    /// `Vec<u8>` rather than `&[u8]` so the payload can travel by value
    /// across `tokio::task::spawn` boundaries without lifetime
    /// gymnastics. The host clones the bytes once at resolve time;
    /// subsequent verb invocations against the same anchor reuse the
    /// host's cached payload.
    pub image_bytes: Vec<u8>,

    /// The same image bytes, base64-encoded. Backends that accept
    /// reference images via JSON multipart (Anthropic, `OpenAI`) prefer
    /// this form; computing it at resolve time means each verb that
    /// needs it doesn't pay the encode cost.
    pub image_b64: String,

    /// Palette extracted from the canonical sheet. Verbs may supply
    /// this to backends that accept palette constraints; backends
    /// without palette support ignore it.
    pub palette: Vec<PaletteEntry>,

    /// Panel layout extracted from the canonical sheet. Verbs that can
    /// crop a specific panel (an "iterate just the happy expression"
    /// flow) read the rectangles here.
    pub composition: SheetComposition,

    /// Project-level `LoRA` path threaded through from
    /// `ProjectAi::project_lora_path`. Per-entity `LoRA` training is a
    /// B10.5 follow-up; until that lands, every anchor for the same
    /// project carries the same `lora_path` value (or `None`).
    pub lora_path: Option<String>,

    /// Anchor strength in `0.0..=1.0`. Default `0.7` per the brief —
    /// strong but not rigid. Verbs that hand the value to a backend
    /// pass it as IP-Adapter strength or the equivalent.
    pub strength: f32,
}

/// Default IP-Adapter strength when the project doesn't override it.
///
/// The B10 brief specifies `0.7` as "strong but not rigid". Visible to
/// verb code that wants to compare against the default.
pub const DEFAULT_ANCHOR_STRENGTH: f32 = 0.7;

impl AnchorPayload {
    /// Builds an [`AnchorPayload`] from a sprite entity's embedded
    /// reference sheet.
    ///
    /// Returns `None` when the entity has no embedded sheet or the sheet
    /// has no approved canonical variant yet.
    ///
    /// `strength` is clamped to `0.0..=1.0`. Pass
    /// [`DEFAULT_ANCHOR_STRENGTH`] when the host has no per-invocation
    /// override.
    #[must_use]
    pub fn from_sprite_entity(
        entity: &Entity,
        strength: f32,
        lora_path: Option<String>,
    ) -> Option<Self> {
        let sheet = match &entity.content {
            EntityContent::Sprites {
                reference_sheet: Some(sheet),
                ..
            } => sheet.as_ref(),
            _ => return None,
        };
        let canonical = sheet.canonical.as_ref()?;
        Some(Self::from_canonical_variant(
            entity.id, canonical, strength, lora_path,
        ))
    }

    /// Builds an [`AnchorPayload`] directly from a canonical
    /// [`SheetVariant`].
    ///
    /// Lower-level constructor used by `from_sprite_entity` and by
    /// host code that already has the canonical variant in hand.
    #[must_use]
    pub fn from_canonical_variant(
        entity_id: EntityId,
        variant: &SheetVariant,
        strength: f32,
        lora_path: Option<String>,
    ) -> Self {
        let canonical_hash = stable_hash(&variant.image.bytes);
        let image_b64 = base64::engine::general_purpose::STANDARD.encode(&variant.image.bytes);
        Self {
            reference_entity_id: entity_id,
            canonical_hash,
            mime: variant.image.mime.clone(),
            image_bytes: variant.image.bytes.clone(),
            image_b64,
            palette: variant.extracted_palette.clone(),
            composition: variant.composition.clone(),
            lora_path,
            strength: strength.clamp(0.0, 1.0),
        }
    }

    /// Convenience: returns `image_bytes` when present, suitable for
    /// passing as `style_image` on an `ImageGenRequest`.
    ///
    /// Returns `None` when the canonical sheet is empty bytes (an
    /// invariant the data model doesn't enforce; better to be defensive
    /// here than have the backend choke on a zero-length image).
    #[must_use]
    pub fn style_image_bytes(&self) -> Option<Vec<u8>> {
        if self.image_bytes.is_empty() {
            None
        } else {
            Some(self.image_bytes.clone())
        }
    }
}

/// Stable, deterministic hash for the cache key.
///
/// FNV-1a over the bytes. Cheap and deterministic; collision rate is
/// acceptable for a change-detection cache key — we only need to
/// distinguish equal vs. not-equal byte sequences, not survive an
/// adversary. `std::hash::DefaultHasher` is `RandomState` by default
/// in some environments, which is why we don't use it. Switch to a
/// stronger hash only if a future cache implementation needs
/// cryptographic collision resistance.
///
/// Exposed publicly so the app-side anchor cache (in
/// `app/src/commands/verbs.rs` and `library.rs`) can compute the same
/// hash without going through `AnchorPayload::from_sprite_entity`,
/// avoiding the base64-encode cost on cache hits.
pub fn stable_hash(bytes: &[u8]) -> u64 {
    const FNV_OFFSET: u64 = 0xcbf2_9ce4_8422_2325;
    const FNV_PRIME: u64 = 0x0000_0100_0000_01b3;
    let mut h = FNV_OFFSET;
    for &b in bytes {
        h ^= u64::from(b);
        h = h.wrapping_mul(FNV_PRIME);
    }
    h
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use pixhaus_core::project::PaletteEntry;
    use pixhaus_core::project::{
        AiMetadata, AssetInfo, Entity, EntityContent, EntityDefaults, EntityKind, NamedSprite,
        ReferenceImage, ReferenceSheet, Rgba, SheetComposition, SheetVariant, SheetVariantId,
        UserData,
    };

    use super::*;

    fn make_sprite_entity(bytes: Vec<u8>) -> Entity {
        Entity {
            id: EntityId::new(11),
            kind: EntityKind::Custom("Character".into()),
            name: "Hero".into(),
            group_id: None,
            tags: Vec::new(),
            defaults: EntityDefaults::default(),
            content: EntityContent::Sprites {
                states: Vec::new(),
                reference_sheet: Some(Box::new(ReferenceSheet {
                    canonical: Some(SheetVariant {
                        id: SheetVariantId::new(1),
                        generated_at: 0,
                        image: ReferenceImage {
                            bytes,
                            mime: "image/png".into(),
                        },
                        composition: SheetComposition::default(),
                        generation: None,
                        extracted_palette: vec![PaletteEntry::new(Rgba::opaque(1, 2, 3))],
                    }),
                    history: Vec::new(),
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
        }
    }

    #[test]
    fn from_sprite_entity_extracts_canonical() {
        let entity = make_sprite_entity(vec![0x89, 0x50, 0x4E, 0x47]);
        let p = AnchorPayload::from_sprite_entity(&entity, 0.5, None).unwrap();
        assert_eq!(p.reference_entity_id, EntityId::new(11));
        assert_eq!(p.image_bytes, vec![0x89, 0x50, 0x4E, 0x47]);
        assert_eq!(p.mime, "image/png");
        assert_eq!(p.strength, 0.5);
        assert_eq!(p.palette.len(), 1);
        assert!(!p.image_b64.is_empty());
    }

    #[test]
    fn from_sprite_without_reference_sheet_returns_none() {
        let entity = Entity {
            id: EntityId::new(1),
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
        };
        assert!(AnchorPayload::from_sprite_entity(&entity, 0.7, None).is_none());
    }

    #[test]
    fn from_sprite_with_draft_only_sheet_returns_none() {
        let mut entity = make_sprite_entity(vec![1, 2, 3]);
        if let EntityContent::Sprites {
            reference_sheet: Some(sheet),
            ..
        } = &mut entity.content
        {
            let canonical = sheet.canonical.take().unwrap();
            sheet.history.push(canonical);
        }
        assert!(AnchorPayload::from_sprite_entity(&entity, 0.7, None).is_none());
    }

    #[test]
    fn strength_clamped_to_unit_interval() {
        let entity = make_sprite_entity(vec![1, 2, 3]);
        let high = AnchorPayload::from_sprite_entity(&entity, 5.0, None).unwrap();
        let low = AnchorPayload::from_sprite_entity(&entity, -1.0, None).unwrap();
        assert_eq!(high.strength, 1.0);
        assert_eq!(low.strength, 0.0);
    }

    #[test]
    fn b64_round_trips_image_bytes() {
        let bytes = vec![1, 2, 3, 4, 5, 200, 255];
        let entity = make_sprite_entity(bytes.clone());
        let p = AnchorPayload::from_sprite_entity(&entity, 0.7, None).unwrap();
        let decoded = base64::engine::general_purpose::STANDARD
            .decode(p.image_b64)
            .unwrap();
        assert_eq!(decoded, bytes);
    }

    #[test]
    fn lora_path_round_trips() {
        let entity = make_sprite_entity(vec![1]);
        let p = AnchorPayload::from_sprite_entity(
            &entity,
            0.7,
            Some(".pixhaus_lora/hero.safetensors".into()),
        )
        .unwrap();
        assert_eq!(
            p.lora_path.as_deref(),
            Some(".pixhaus_lora/hero.safetensors")
        );
    }

    #[test]
    fn empty_image_bytes_returns_none_for_style_image() {
        let entity = make_sprite_entity(Vec::new());
        let p = AnchorPayload::from_sprite_entity(&entity, 0.7, None).unwrap();
        assert!(p.style_image_bytes().is_none());
    }

    #[test]
    fn non_empty_image_bytes_returns_clone_for_style_image() {
        let entity = make_sprite_entity(vec![1, 2, 3]);
        let p = AnchorPayload::from_sprite_entity(&entity, 0.7, None).unwrap();
        assert_eq!(p.style_image_bytes(), Some(vec![1, 2, 3]));
    }

    #[test]
    fn canonical_hash_is_deterministic() {
        let a = make_sprite_entity(vec![10, 20, 30]);
        let b = make_sprite_entity(vec![10, 20, 30]);
        let c = make_sprite_entity(vec![10, 20, 31]);
        let pa = AnchorPayload::from_sprite_entity(&a, 0.7, None).unwrap();
        let pb = AnchorPayload::from_sprite_entity(&b, 0.7, None).unwrap();
        let pc = AnchorPayload::from_sprite_entity(&c, 0.7, None).unwrap();
        assert_eq!(pa.canonical_hash, pb.canonical_hash);
        assert_ne!(pa.canonical_hash, pc.canonical_hash);
    }

    #[test]
    fn anchor_payload_round_trips_through_json() {
        let entity = make_sprite_entity(vec![1, 2, 3]);
        let p = AnchorPayload::from_sprite_entity(&entity, 0.6, Some("path.safetensors".into()))
            .unwrap();
        let json = serde_json::to_string(&p).unwrap();
        let back: AnchorPayload = serde_json::from_str(&json).unwrap();
        assert_eq!(back, p);
    }

    // Suppress dead-code on NamedSprite since it's only useful for context.
    #[allow(dead_code)]
    fn _imports_used() -> NamedSprite {
        unreachable!()
    }
}
