//! [`CodexEntry`]: a header shared by every type plus a type-specific body.
//!
//! The header carries identity, status, ownership, locks, the descriptive text, the
//! tags, the prompt and negative fragments, the anchors, and the version history
//! (bible 4, 11). The [`details`](CodexEntry::details) field carries the
//! type-specific body. Mutation flows through commands; the fields are public so the
//! layers above can read them, but only a command writes them.

use serde::{Deserialize, Serialize};

use crate::codex::anchor::Anchor;
use crate::codex::coverage::{CoverageSlot, CoverageTemplateId};
use crate::codex::details::EntryDetails;
use crate::codex::entry_type::EntryType;
use crate::codex::folder::CodexFolderId;
use crate::codex::handle::CodexHandle;
use crate::codex::ids::CodexEntryId;
use crate::codex::priority::PromptFragment;
use crate::codex::status::{EntryLocks, EntryStatus, Ownership};

/// One version-history record for an entry (bible 11.7).
///
/// The timestamp is a caller-supplied epoch-millisecond value so `core` stays free of
/// a clock dependency; the layers above stamp it. The author is a free-text source
/// label (e.g. a user name or "ai"), not a localized string.
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub struct EntryVersion {
    /// Monotonic version number, starting at 1.
    pub version: u32,
    /// Wall-clock time in epoch milliseconds, supplied by the layer that records it.
    pub timestamp_ms: u64,
    /// Who or what made the change (a source label, not localized).
    pub author: String,
    /// A short summary of what changed.
    pub summary: String,
}

/// One Codex entry: a shared header plus a type-specific body.
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub struct CodexEntry {
    /// Stable id; references point here, never at the handle or name.
    pub id: CodexEntryId,
    /// The validated `@`-mention handle.
    pub handle: CodexHandle,
    /// Alternate handles that also resolve to this entry (bible 6.4).
    pub aliases: Vec<CodexHandle>,
    /// Display name (project content; may change without breaking references).
    pub name: String,
    /// The entry's namespace.
    pub entry_type: EntryType,
    /// The folder this entry lives in, or `None` when it sits at the codex root.
    #[serde(default)]
    pub folder_id: Option<CodexFolderId>,
    /// How finished the entry is.
    pub status: EntryStatus,
    /// Where the entry came from.
    pub ownership: Ownership,
    /// Which facets are locked against change.
    pub locks: EntryLocks,
    /// Short description (project content).
    pub description: String,
    /// Lore / story description (project content).
    pub lore: String,
    /// Visual description (project content).
    pub visual_description: String,
    /// Free-form tags for search and grouping.
    pub tags: Vec<String>,
    /// Positive prompt fragments with inclusion priorities.
    pub prompt_fragments: Vec<PromptFragment>,
    /// Negative prompt fragments (things to avoid).
    pub negative_fragments: Vec<String>,
    /// The anchors that pin this entry's creative intent.
    pub anchors: Vec<Anchor>,
    /// Version history, oldest first.
    pub version_history: Vec<EntryVersion>,
    /// The project-level coverage templates applied to this entry, in apply order. The
    /// entry's coverage is the union of these templates' slots and its `custom_slots`.
    #[serde(default)]
    pub applied_templates: Vec<CoverageTemplateId>,
    /// Per-entry ad-hoc coverage slots, for one-off needs no template covers.
    #[serde(default)]
    pub custom_slots: Vec<CoverageSlot>,
    /// The type-specific body.
    pub details: EntryDetails,
}

impl CodexEntry {
    /// A new entry with `id`, `handle`, `name`, and `entry_type`, a body defaulted to
    /// match the type, and otherwise-empty header fields at [`EntryStatus::Draft`] /
    /// [`Ownership::ProjectDefined`].
    pub fn new(id: CodexEntryId, handle: CodexHandle, name: impl Into<String>, entry_type: EntryType) -> Self {
        Self {
            id,
            handle,
            aliases: Vec::new(),
            name: name.into(),
            entry_type,
            folder_id: None,
            status: EntryStatus::default(),
            ownership: Ownership::default(),
            locks: EntryLocks::default(),
            description: String::new(),
            lore: String::new(),
            visual_description: String::new(),
            tags: Vec::new(),
            prompt_fragments: Vec::new(),
            negative_fragments: Vec::new(),
            anchors: Vec::new(),
            version_history: Vec::new(),
            applied_templates: Vec::new(),
            custom_slots: Vec::new(),
            details: EntryDetails::default_for(entry_type),
        }
    }

    /// The position of the first anchor of `kind`, if any.
    pub fn anchor_position(&self, kind: crate::codex::AnchorKind) -> Option<usize> {
        self.anchors.iter().position(|a| a.kind == kind)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::codex::AnchorKind;

    fn handle(s: &str) -> CodexHandle {
        CodexHandle::new(s).unwrap()
    }

    #[test]
    fn new_entry_defaults_header_and_body() {
        let e = CodexEntry::new(CodexEntryId(1), handle("bit"), "Bit", EntryType::Character);
        assert_eq!(e.id, CodexEntryId(1));
        assert_eq!(e.handle.as_str(), "bit");
        assert_eq!(e.name, "Bit");
        assert_eq!(e.status, EntryStatus::Draft);
        assert_eq!(e.ownership, Ownership::ProjectDefined);
        assert!(e.anchors.is_empty());
        assert!(e.applied_templates.is_empty());
        assert!(e.custom_slots.is_empty());
        assert!(matches!(e.details, EntryDetails::Character(_)));
    }

    #[test]
    fn anchor_position_finds_first_of_kind() {
        let mut e = CodexEntry::new(CodexEntryId(1), handle("bit"), "Bit", EntryType::Character);
        assert_eq!(e.anchor_position(AnchorKind::Visual), None);
        e.anchors.push(Anchor::new(AnchorKind::Visual, crate::codex::AnchorStrength::Normal, "x"));
        assert_eq!(e.anchor_position(AnchorKind::Visual), Some(0));
    }
}
