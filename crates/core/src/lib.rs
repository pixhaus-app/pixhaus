//! Pixhaus creative core: the domain model and pure operations.
//!
//! `core` owns the authoritative project data — sprites, layers, palettes, the
//! pixel buffers behind them, the typed ids that key them, and the [`Command`] trait
//! through which every mutation flows. It is pure data and operations: no egui, no
//! wgpu, no I/O. Everything else in the workspace depends on it; it depends on
//! nothing in the workspace.
//!
//! The shape, foundation stage:
//! - [`PixelBuffer`] — RGBA8 bytes with explicit stride; the only pixel container.
//! - [`Document`] — the single [`Command`] target, bundling structural data and the
//!   [`PixelBufferStore`] so one command type covers structural and pixel edits.
//! - [`Command`] + [`commands`] — the mutation boundary and the fundamental commands
//!   ([`commands::AddSprite`], [`commands::ApplyGeneratedAsset`]).
//! - [`composite_sprite`] / [`composite_active`] — the pure CPU compositor the
//!   renderer uploads as a texture.

#![cfg_attr(
    test,
    allow(clippy::unwrap_used, clippy::expect_used, clippy::disallowed_methods, clippy::panic, clippy::float_cmp)
)]

pub mod animation;
pub mod buffer_store;
pub mod codex;
pub mod command;
pub mod commands;
pub mod composite;
pub mod document;
pub mod ids;
pub mod pixel;

// Test-only shared builders for the command tests. Gated to test so it never ships
// in the library or counts against the public API; pub(crate) so only this crate's
// own #[cfg(test)] modules can reach it. See test_support.rs for why it exists.
#[cfg(test)]
pub(crate) mod test_support;

pub use animation::{AnimationClip, LoopMode};
pub use buffer_store::PixelBufferStore;
pub use codex::{
    Anchor, AnchorKind, AnchorStrength, Codex, CodexEntry, CodexEntryId, CodexFolder, CodexFolderId, CodexHandle, CoverageItemStatus, CoverageKey,
    CoverageLabel, CoverageSlot, CoverageTemplate, CoverageTemplateId, EntryDetails, EntryLocks, EntryStatus, EntryType, EntryVersion, HandleError,
    InclusionPriority, Ownership, PromptFragment, RelationKind, Relationship,
};
pub use command::{Command, CommandError};
pub use commands::{
    AddCodexAlias, AddCodexEntry, AddCoverageSlot, AddEntryCustomSlot, AddRelationship, AddSprite, ApplyBuiltinCoverageTemplate, ApplyCoverageTemplate,
    ApplyGeneratedAnimation, ApplyGeneratedAsset, BuiltinCoveragePreset, ChangeRelationshipKind, ClearCoverage, CodexEntryDelta, CodexEntryProto,
    CreateCodexFolder, CreateCoverageTemplate, DeleteCodexEntry, DeleteCodexFolder, DeleteCoverageTemplate, DuplicateCodexEntry, GeneratedFrameData,
    RemoveAnchor, RemoveCodexAlias, RemoveCoverageSlot, RemoveEntryCustomSlot, RemoveRelationship, RenameCodexFolder, RenameCoverageSlotLabel,
    RenameCoverageTemplate, RenameEntryCustomSlotLabel, ReorderCoverageSlots, SetAnchor, SetAnimationDetails, SetCharacterDetails, SetCodexEntryFolder,
    SetCodexFolderParent, SetCodexHandle, SetCoverageStatus, SetEntryStatus, SetGenericDetails, SetNegativeFragments, SetPaletteDetails, SetPromptFragments,
    SetStyleDetails, SpriteProto, UpdateCodexEntry,
};
pub use composite::{CompositeError, composite_active, composite_frame, composite_sprite};
pub use document::{DEFAULT_CANVAS_SIZE, DEFAULT_FRAME_DURATION_MS, Document, Frame, Layer, Sprite};
pub use ids::{ClipId, FrameId, IdCounter, LayerId, PixelBufferId, SpriteId};
pub use pixel::{BlendMode, PixelBuffer, PixelError, Rgba};
