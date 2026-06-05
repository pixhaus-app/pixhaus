//! The Codex: a project's creative bible (bible 3, 4, 5, 11, 15, 24).
//!
//! The Codex remembers what a project's characters, palettes, styles, animations,
//! props, locations, materials, rules, and recipes are; AI proposes; the artist
//! decides. It is a graph of typed [`entry`]s connected by typed
//! [`relationship`]s, each entry pinned by [`anchor`]s that lock creative intent, and
//! tracked against [`coverage`] templates that say which assets a project still
//! needs.
//!
//! This module owns the data model only: every mutation flows through a codex command
//! in [`crate::commands`]. It stores stable ids, handles, and keys, never display
//! text — the Codex outlives any rename.

pub mod anchor;
pub mod coverage;
pub mod details;
pub mod entry;
pub mod entry_type;
pub mod folder;
pub mod handle;
pub mod ids;
pub mod priority;
pub mod relationship;
pub mod root;
pub mod status;

pub use anchor::{Anchor, AnchorKind, AnchorStrength};
pub use coverage::{CoverageItemStatus, CoverageLabel, CoverageSlot, CoverageTemplate, CoverageTemplateId};
pub use details::{
    AnimationDetails, AntiAliasingRule, CharacterDetails, ColorRole, DetailLevel, EntryDetails, GenericDetails, GenericField, LineTreatment, LoopBehavior,
    PaletteColor, PaletteDetails, PaletteRamp, PoseBeat, StyleDetails,
};
pub use entry::{CodexEntry, EntryVersion};
pub use entry_type::EntryType;
pub use folder::{CodexFolder, CodexFolderId};
pub use handle::{CodexHandle, HandleError};
pub use ids::CodexEntryId;
pub use priority::{InclusionPriority, PromptFragment};
pub use relationship::{RelationKind, Relationship};
pub use root::{Codex, CoverageKey};
pub use status::{EntryLocks, EntryStatus, Ownership};
