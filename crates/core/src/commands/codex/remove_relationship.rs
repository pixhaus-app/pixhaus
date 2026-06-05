//! [`RemoveRelationship`]: remove a typed edge between two entries, reversibly.

use crate::codex::Relationship;
use crate::command::{Command, CommandError};
use crate::document::Document;

/// Removes a [`Relationship`] from the Codex; undo restores it at its prior position.
/// Removing an edge that is not present is an error.
pub struct RemoveRelationship {
    relationship: Relationship,
    /// The index the edge was removed from, captured on apply for undo.
    removed_index: Option<usize>,
}

impl RemoveRelationship {
    /// A command that will remove `relationship`.
    pub fn new(relationship: Relationship) -> Self {
        Self {
            relationship,
            removed_index: None,
        }
    }
}

impl Command for RemoveRelationship {
    fn apply(&mut self, doc: &mut Document) -> Result<(), CommandError> {
        let rel = self.relationship;
        let codex = doc.codex_mut();
        let pos = codex.relationships.iter().position(|r| *r == rel).ok_or(CommandError::InvalidState)?;
        codex.relationships.remove(pos);
        doc.bump_revision();
        self.removed_index = Some(pos);
        Ok(())
    }

    fn undo(&mut self, doc: &mut Document) -> Result<(), CommandError> {
        let pos = self.removed_index.take().ok_or(CommandError::InvalidState)?;
        let codex = doc.codex_mut();
        let pos = pos.min(codex.relationships.len());
        codex.relationships.insert(pos, self.relationship);
        doc.bump_revision();
        Ok(())
    }

    fn label_key(&self) -> &'static str {
        "command.codex.remove_relationship"
    }

    fn estimated_size_bytes(&self) -> usize {
        std::mem::size_of::<Self>()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::codex::{CodexEntryId, CodexHandle, EntryType, RelationKind};
    use crate::commands::{AddCodexEntry, AddRelationship, CodexEntryProto};

    fn seed(doc: &mut Document, handle: &str, ty: EntryType) -> CodexEntryId {
        let mut add = AddCodexEntry::new(CodexEntryProto {
            handle: CodexHandle::new(handle).unwrap(),
            name: handle.to_owned(),
            entry_type: ty,
        });
        add.apply(doc).unwrap();
        add.inserted_id().unwrap()
    }

    #[test]
    fn apply_removes_then_undo_restores() {
        let mut doc = Document::new();
        let bit = seed(&mut doc, "bit", EntryType::Character);
        let pal = seed(&mut doc, "moonlit", EntryType::Palette);
        let rel = Relationship::new(bit, RelationKind::Uses, pal);
        AddRelationship::new(rel).apply(&mut doc).unwrap();
        let mut cmd = RemoveRelationship::new(rel);

        cmd.apply(&mut doc).unwrap();
        assert_eq!(doc.codex().relationships().len(), 0);

        cmd.undo(&mut doc).unwrap();
        assert_eq!(doc.codex().relationships(), &[rel]);
    }

    #[test]
    fn removing_absent_edge_errors() {
        let mut doc = Document::new();
        let bit = seed(&mut doc, "bit", EntryType::Character);
        let mut cmd = RemoveRelationship::new(Relationship::new(bit, RelationKind::Uses, CodexEntryId(99)));
        assert!(matches!(cmd.apply(&mut doc), Err(CommandError::InvalidState)));
    }
}
