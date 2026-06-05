//! Entry status, ownership, and lock flags (bible 11.4-11.6).
//!
//! These metadata types describe an entry's place in the creative process — how
//! finished it is, where it came from, and which facets the artist has locked
//! against change. They are stable ids, never display strings.

use serde::{Deserialize, Serialize};

/// How finished an entry is in the creative process (bible 11.4).
#[derive(Copy, Clone, PartialEq, Eq, Hash, Debug, Default, Serialize, Deserialize)]
pub enum EntryStatus {
    /// A work in progress, not yet promoted.
    #[default]
    Draft,
    /// Proposed for canonical status, awaiting the artist's decision.
    Candidate,
    /// The authoritative version generation should preserve.
    Canonical,
    /// Superseded but kept so existing references resolve.
    Deprecated,
    /// Removed from active use but retained in history.
    Archived,
    /// Considered and turned down.
    Rejected,
}

/// Where an entry came from (bible 11.5).
#[derive(Copy, Clone, PartialEq, Eq, Hash, Debug, Default, Serialize, Deserialize)]
pub enum Ownership {
    /// Shipped with Pixhaus.
    BuiltIn,
    /// Defined as part of this project.
    #[default]
    ProjectDefined,
    /// Brought in from an imported pack.
    ImportedPack,
    /// Proposed by AI, awaiting the artist's decision.
    GeneratedSuggestion,
    /// Authored directly by the user.
    UserCreated,
    /// Shared by a team member.
    TeamShared,
}

/// The facets of an entry the artist can lock against change (bible 11.6).
///
/// Each flag is independent; locking is a set of toggles, not a single mode. The
/// model stores the flags; enforcement (refusing an edit, warning on generation)
/// lives in the layers above. The five flags map one-to-one to the bible's five lock
/// types, so they stay separate booleans rather than a packed bitset.
#[allow(clippy::struct_excessive_bools)]
#[derive(Copy, Clone, PartialEq, Eq, Hash, Debug, Default, Serialize, Deserialize)]
pub struct EntryLocks {
    /// The entry's content (description, lore, fields) must not change.
    pub content: bool,
    /// The visual anchor must not change.
    pub visual_anchor: bool,
    /// The palette must not change.
    pub palette: bool,
    /// The name and handle must not change.
    pub handle: bool,
    /// The reference assets must not change.
    pub reference: bool,
}

impl EntryLocks {
    /// A fully-unlocked set of flags.
    pub fn none() -> Self {
        Self::default()
    }

    /// Whether any facet is locked.
    pub fn any(self) -> bool {
        self.content || self.visual_anchor || self.palette || self.handle || self.reference
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_are_draft_project_unlocked() {
        assert_eq!(EntryStatus::default(), EntryStatus::Draft);
        assert_eq!(Ownership::default(), Ownership::ProjectDefined);
        assert!(!EntryLocks::default().any());
    }

    #[test]
    fn locks_any_reports_set_flags() {
        let mut locks = EntryLocks::none();
        assert!(!locks.any());
        locks.palette = true;
        assert!(locks.any());
    }
}
