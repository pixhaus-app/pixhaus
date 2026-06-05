//! Undoable commands over the [`Codex`](crate::codex::Codex).
//!
//! Every Codex mutation is one of these commands: adding, editing, and deleting
//! entries; promoting their status; setting and removing anchors; wiring and
//! unwiring relationships; and seeding and updating coverage. Each captures enough on
//! apply to reverse itself, bumps the document revision on both apply and undo, and
//! carries a stable `command.codex.*` label key.

mod add_alias;
mod add_coverage_slot;
mod add_entry;
mod add_entry_custom_slot;
mod add_relationship;
mod apply_builtin_coverage_template;
mod apply_coverage_template;
mod change_relationship_kind;
mod clear_coverage;
mod create_coverage_template;
mod create_folder;
mod delete_coverage_template;
mod delete_entry;
mod delete_folder;
mod duplicate_entry;
mod remove_alias;
mod remove_anchor;
mod remove_coverage_slot;
mod remove_entry_custom_slot;
mod remove_relationship;
mod rename_coverage_slot_label;
mod rename_coverage_template;
mod rename_entry_custom_slot;
mod rename_folder;
mod reorder_coverage_slots;
mod set_anchor;
mod set_coverage_status;
mod set_details;
mod set_entry_folder;
mod set_folder_parent;
mod set_fragments;
mod set_handle;
mod set_status;
mod update_entry;

pub use add_alias::AddCodexAlias;
pub use add_coverage_slot::AddCoverageSlot;
pub use add_entry::{AddCodexEntry, CodexEntryProto};
pub use add_entry_custom_slot::AddEntryCustomSlot;
pub use add_relationship::AddRelationship;
pub use apply_builtin_coverage_template::{ApplyBuiltinCoverageTemplate, BuiltinCoveragePreset};
pub use apply_coverage_template::ApplyCoverageTemplate;
pub use change_relationship_kind::ChangeRelationshipKind;
pub use clear_coverage::ClearCoverage;
pub use create_coverage_template::CreateCoverageTemplate;
pub use create_folder::CreateCodexFolder;
pub use delete_coverage_template::DeleteCoverageTemplate;
pub use delete_entry::DeleteCodexEntry;
pub use delete_folder::DeleteCodexFolder;
pub use duplicate_entry::DuplicateCodexEntry;
pub use remove_alias::RemoveCodexAlias;
pub use remove_anchor::RemoveAnchor;
pub use remove_coverage_slot::RemoveCoverageSlot;
pub use remove_entry_custom_slot::RemoveEntryCustomSlot;
pub use remove_relationship::RemoveRelationship;
pub use rename_coverage_slot_label::RenameCoverageSlotLabel;
pub use rename_coverage_template::RenameCoverageTemplate;
pub use rename_entry_custom_slot::RenameEntryCustomSlotLabel;
pub use rename_folder::RenameCodexFolder;
pub use reorder_coverage_slots::ReorderCoverageSlots;
pub use set_anchor::SetAnchor;
pub use set_coverage_status::SetCoverageStatus;
pub use set_details::{SetAnimationDetails, SetCharacterDetails, SetGenericDetails, SetPaletteDetails, SetStyleDetails};
pub use set_entry_folder::SetCodexEntryFolder;
pub use set_folder_parent::SetCodexFolderParent;
pub use set_fragments::{SetNegativeFragments, SetPromptFragments};
pub use set_handle::SetCodexHandle;
pub use set_status::SetEntryStatus;
pub use update_entry::{CodexEntryDelta, UpdateCodexEntry};
