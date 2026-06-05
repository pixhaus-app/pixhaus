//! Typed relationships between Codex entries (bible 11.8).
//!
//! The Codex is a graph: entries reference one another through typed edges. A
//! relationship is a directed `from -> to` edge labelled by a [`RelationKind`]. The
//! model stores edges; interpreting them (resolving a graph, finding "used by") is
//! the layers' job.

use serde::{Deserialize, Serialize};

use crate::codex::ids::CodexEntryId;

/// The kind of a directed edge between two entries (bible 11.8).
#[derive(Copy, Clone, PartialEq, Eq, Hash, Debug, PartialOrd, Ord, Serialize, Deserialize)]
pub enum RelationKind {
    /// The source uses the target (e.g. a character uses a palette).
    Uses,
    /// The source belongs to the target (e.g. a character belongs to a faction).
    BelongsTo,
    /// The source appears in the target (e.g. a prop appears in a location).
    AppearsIn,
    /// The two are compatible.
    CompatibleWith,
    /// The two are incompatible.
    IncompatibleWith,
    /// The source inherits from the target.
    InheritsFrom,
    /// The source is a variant of the target.
    VariantOf,
    /// The source requires the target.
    Requires,
    /// The source contains the target.
    Contains,
    /// The source replaces the target.
    Replaces,
    /// The source is inspired by the target.
    InspiredBy,
}

/// A directed, typed edge between two entries.
#[derive(Copy, Clone, PartialEq, Eq, Hash, Debug, Serialize, Deserialize)]
// Relationships are stored as resolved CodexEntryId edges, not handle text. This is the second
// of two reference mechanisms: prompt fragments keep human-authored @<handle> text resolved at
// compile time, while structural edges bind to ids so renaming an entry preserves referential
// integrity and never silently rewires the graph.
pub struct Relationship {
    /// The edge's source entry.
    pub from: CodexEntryId,
    /// The kind of relationship.
    pub kind: RelationKind,
    /// The edge's target entry.
    pub to: CodexEntryId,
}

impl Relationship {
    /// An edge `from --kind--> to`.
    pub fn new(from: CodexEntryId, kind: RelationKind, to: CodexEntryId) -> Self {
        Self { from, kind, to }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_edge_holds_its_endpoints() {
        let r = Relationship::new(CodexEntryId(1), RelationKind::Uses, CodexEntryId(2));
        assert_eq!(r.from, CodexEntryId(1));
        assert_eq!(r.kind, RelationKind::Uses);
        assert_eq!(r.to, CodexEntryId(2));
    }
}
