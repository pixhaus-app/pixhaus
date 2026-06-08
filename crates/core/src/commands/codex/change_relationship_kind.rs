//! [`ChangeRelationshipKind`]: re-label an existing edge's kind in place, reversibly.
//!
//! A [`Relationship`] carries no stable id — it is keyed by
//! its `(from, kind, to)` triple — so changing its kind is conceptually a remove plus an
//! add. This command fuses both into one undoable step so the in-place edit affordance on
//! a relations row reverses cleanly, instead of leaving the user to remove and re-add by
//! hand. The edge keeps its position in the list. Undo restores the prior kind.

use crate::codex::{CodexEntryId, RelationKind, Relationship};
use crate::command::{Command, CommandError};
use crate::document::Document;

/// Changes the `kind` of the edge `from --old_kind--> to` to `new_kind`, in place. The
/// source edge must exist, and an edge with the new kind must not already exist. Undo
/// restores the prior kind.
pub struct ChangeRelationshipKind {
    from: CodexEntryId,
    to: CodexEntryId,
    /// The kind currently on the edge; becomes the target kind after a successful apply,
    /// so apply and undo are symmetric swaps.
    from_kind: RelationKind,
    to_kind: RelationKind,
}

impl ChangeRelationshipKind {
    /// A command that will retype the edge `from --old_kind--> to` to `new_kind`.
    pub fn new(from: CodexEntryId, old_kind: RelationKind, to: CodexEntryId, new_kind: RelationKind) -> Self {
        Self {
            from,
            to,
            from_kind: old_kind,
            to_kind: new_kind,
        }
    }
}

impl Command for ChangeRelationshipKind {
    fn apply(&mut self, doc: &mut Document) -> Result<(), CommandError> {
        // A no-op kind change is rejected so undo never targets a phantom edge. Guard first,
        // before building the edge values, so the cheap rejection happens up front.
        if self.from_kind == self.to_kind {
            return Err(CommandError::InvalidState);
        }
        let old = Relationship::new(self.from, self.from_kind, self.to);
        let new = Relationship::new(self.from, self.to_kind, self.to);
        let codex = doc.codex_mut();
        let pos = codex.relationships.iter().position(|r| *r == old).ok_or(CommandError::InvalidState)?;
        // Retyping onto an edge that already exists would collapse two edges into one,
        // which undo could not split apart; reject it.
        if codex.relationships.contains(&new) {
            return Err(CommandError::InvalidState);
        }
        codex.relationships[pos] = new;
        doc.bump_revision();
        // Swap so undo (which calls apply) moves the edge back.
        std::mem::swap(&mut self.from_kind, &mut self.to_kind);
        Ok(())
    }

    fn undo(&mut self, doc: &mut Document) -> Result<(), CommandError> {
        // apply and undo are symmetric: each swaps the held kinds and retypes the edge.
        self.apply(doc)
    }

    fn label_key(&self) -> &'static str {
        "command.codex.change_relationship_kind"
    }

    fn estimated_size_bytes(&self) -> usize {
        std::mem::size_of::<Self>()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::codex::EntryType;
    use crate::commands::AddRelationship;
    use crate::test_support::seed_entry;

    // Local wrapper: these relationship tests name each entry after its handle, so
    // they pass the handle twice into the shared four-field builder.
    fn seed(doc: &mut Document, handle: &str, ty: EntryType) -> CodexEntryId {
        seed_entry(doc, handle, handle, ty)
    }

    fn seed_edge(doc: &mut Document) -> (CodexEntryId, CodexEntryId) {
        let bit = seed(doc, "bit", EntryType::Character);
        let pal = seed(doc, "moonlit", EntryType::Palette);
        AddRelationship::new(Relationship::new(bit, RelationKind::Uses, pal)).apply(doc).unwrap();
        (bit, pal)
    }

    #[test]
    fn apply_changes_kind_in_place_then_undo_restores() {
        let mut doc = Document::new();
        let (bit, pal) = seed_edge(&mut doc);
        let mut cmd = ChangeRelationshipKind::new(bit, RelationKind::Uses, pal, RelationKind::Requires);

        cmd.apply(&mut doc).unwrap();
        assert_eq!(doc.codex().relationships(), &[Relationship::new(bit, RelationKind::Requires, pal)]);

        cmd.undo(&mut doc).unwrap();
        assert_eq!(doc.codex().relationships(), &[Relationship::new(bit, RelationKind::Uses, pal)]);
    }

    #[test]
    fn redo_changes_kind_again() {
        let mut doc = Document::new();
        let (bit, pal) = seed_edge(&mut doc);
        let mut cmd = ChangeRelationshipKind::new(bit, RelationKind::Uses, pal, RelationKind::Requires);
        cmd.apply(&mut doc).unwrap();
        cmd.undo(&mut doc).unwrap();
        cmd.apply(&mut doc).unwrap();
        assert_eq!(doc.codex().relationships(), &[Relationship::new(bit, RelationKind::Requires, pal)]);
    }

    #[test]
    fn preserves_position_among_other_edges() {
        let mut doc = Document::new();
        let bit = seed(&mut doc, "bit", EntryType::Character);
        let pal = seed(&mut doc, "moonlit", EntryType::Palette);
        let loc = seed(&mut doc, "forest", EntryType::Location);
        AddRelationship::new(Relationship::new(bit, RelationKind::AppearsIn, loc))
            .apply(&mut doc)
            .unwrap();
        AddRelationship::new(Relationship::new(bit, RelationKind::Uses, pal)).apply(&mut doc).unwrap();

        ChangeRelationshipKind::new(bit, RelationKind::AppearsIn, loc, RelationKind::BelongsTo)
            .apply(&mut doc)
            .unwrap();

        assert_eq!(
            doc.codex().relationships(),
            &[
                Relationship::new(bit, RelationKind::BelongsTo, loc),
                Relationship::new(bit, RelationKind::Uses, pal),
            ]
        );
    }

    #[test]
    fn missing_edge_errors() {
        let mut doc = Document::new();
        let (bit, pal) = seed_edge(&mut doc);
        let mut cmd = ChangeRelationshipKind::new(bit, RelationKind::Requires, pal, RelationKind::BelongsTo);
        assert!(matches!(cmd.apply(&mut doc), Err(CommandError::InvalidState)));
    }

    #[test]
    fn no_op_kind_errors() {
        let mut doc = Document::new();
        let (bit, pal) = seed_edge(&mut doc);
        let mut cmd = ChangeRelationshipKind::new(bit, RelationKind::Uses, pal, RelationKind::Uses);
        assert!(matches!(cmd.apply(&mut doc), Err(CommandError::InvalidState)));
    }

    #[test]
    fn collision_with_existing_edge_errors() {
        let mut doc = Document::new();
        let (bit, pal) = seed_edge(&mut doc);
        AddRelationship::new(Relationship::new(bit, RelationKind::Requires, pal))
            .apply(&mut doc)
            .unwrap();
        let mut cmd = ChangeRelationshipKind::new(bit, RelationKind::Uses, pal, RelationKind::Requires);
        assert!(matches!(cmd.apply(&mut doc), Err(CommandError::InvalidState)));
    }
}
