//! Project-scoped reusable AI assets: saved reference images, character
//! cards, style swatches, and trained `LoRA` registrations.

use serde::{Deserialize, Serialize};

use crate::project::id::{AssetId, SheetVariantId};

use super::ai::ModelId;
use super::reference_sheets::{ReferenceImage, ReferenceRole};

/// Project-scoped reusable assets for reference-sheet workflows.
#[allow(missing_docs)]
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct AssetLibrary {
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub references: Vec<ReferenceAsset>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub character_cards: Vec<CharacterCard>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub style_swatches: Vec<StyleSwatch>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub loras: Vec<LoraAsset>,
}

impl AssetLibrary {
    #[allow(missing_docs)]
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.references.is_empty()
            && self.character_cards.is_empty()
            && self.style_swatches.is_empty()
            && self.loras.is_empty()
    }
}

/// Single image saved to the project asset library.
#[allow(missing_docs)]
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ReferenceAsset {
    pub id: AssetId,
    pub image: ReferenceImage,
    #[serde(default)]
    pub default_role: ReferenceRole,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tags: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_variant_id: Option<SheetVariantId>,
    pub created_at: i64,
}

/// Bundle of references and style notes representing one character.
#[allow(missing_docs)]
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct CharacterCard {
    pub id: AssetId,
    pub name: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub references: Vec<AssetId>,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub style_notes: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub associated_lora: Option<AssetId>,
    pub created_at: i64,
}

/// Bundle of references and style notes representing a visual style.
#[allow(missing_docs)]
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct StyleSwatch {
    pub id: AssetId,
    pub name: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub references: Vec<AssetId>,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub style_notes: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub associated_lora: Option<AssetId>,
    pub created_at: i64,
}

/// Kind of `LoRA` training asset.
#[allow(missing_docs)]
#[derive(Copy, Clone, Debug, Default, Eq, PartialEq, Ord, PartialOrd, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LoraKind {
    Style,
    #[default]
    Character,
    KontextPair,
}

/// Trained `LoRA` registered in the project asset library.
#[allow(missing_docs)]
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct LoraAsset {
    pub id: AssetId,
    pub name: String,
    #[serde(default)]
    pub kind: LoraKind,
    pub trigger_word: String,
    pub target_model: ModelId,
    pub fal_lora_url: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub training_data_thumbnails: Vec<ReferenceImage>,
    pub created_at: i64,
}
