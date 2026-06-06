//! Concrete undoable commands over the [`Document`](crate::Document).
//!
//! These are the fundamental commands the shared core ships. Module-specific
//! commands live in their owning module, but anything that mutates the base
//! sprite/layer/buffer model belongs here so it stays pure and testable.

mod add_sprite;
mod apply_generated_animation;
mod apply_generated_asset;
mod codex;
mod macros;

pub use add_sprite::{AddSprite, SpriteProto};
pub use apply_generated_animation::{ApplyGeneratedAnimation, GeneratedFrameData};
pub use apply_generated_asset::ApplyGeneratedAsset;
pub use codex::{
    AddCodexAlias, AddCodexEntry, AddCoverageSlot, AddEntryCustomSlot, AddRelationship, ApplyBuiltinCoverageTemplate, ApplyCoverageTemplate,
    BuiltinCoveragePreset, ChangeRelationshipKind, ClearCoverage, CodexEntryDelta, CodexEntryProto, CreateCodexFolder, CreateCoverageTemplate,
    DeleteCodexEntry, DeleteCodexFolder, DeleteCoverageTemplate, DuplicateCodexEntry, RemoveAnchor, RemoveCodexAlias, RemoveCoverageSlot,
    RemoveEntryCustomSlot, RemoveRelationship, RenameCodexFolder, RenameCoverageSlotLabel, RenameCoverageTemplate, RenameEntryCustomSlotLabel,
    ReorderCoverageSlots, SetAnchor, SetAnimationDetails, SetCharacterDetails, SetCodexEntryFolder, SetCodexFolderParent, SetCodexHandle, SetCoverageStatus,
    SetEntryStatus, SetGenericDetails, SetNegativeFragments, SetPaletteDetails, SetPromptFragments, SetStyleDetails, UpdateCodexEntry,
};
