//! [`AddRelationship`]: add a typed edge between two entries, reversibly.

use crate::codex::Relationship;
use crate::command::{Command, CommandError};
use crate::document::Document;

/// Adds a [`Relationship`] to the Codex; undo removes the edge it added. Both
/// endpoints must exist. An identical edge already present is rejected so undo does
/// not remove a pre-existing one.
pub struct AddRelationship {
    relationship: Relationship,
    added: bool,
}

impl AddRelationship {
    /// A command that will add `relationship`.
    pub fn new(relationship: Relationship) -> Self {
        Self { relationship, added: false }
    }
}

impl Command for AddRelationship {
    fn apply(&mut self, doc: &mut Document) -> Result<(), CommandError> {
        let rel = self.relationship;
        let codex = doc.codex_mut();
        if codex.entry(rel.from).is_none() {
            return Err(CommandError::CodexEntryNotFound(rel.from));
        }
        if codex.entry(rel.to).is_none() {
            return Err(CommandError::CodexEntryNotFound(rel.to));
        }
        if codex.relationships.contains(&rel) {
            return Err(CommandError::InvalidState);
        }
        codex.relationships.push(rel);
        doc.bump_revision();
        self.added = true;
        Ok(())
    }

    fn undo(&mut self, doc: &mut Document) -> Result<(), CommandError> {
        if !self.added {
            return Err(CommandError::InvalidState);
        }
        let rel = self.relationship;
        let codex = doc.codex_mut();
        let pos = codex.relationships.iter().position(|r| *r == rel).ok_or(CommandError::InvalidState)?;
        codex.relationships.remove(pos);
        doc.bump_revision();
        self.added = false;
        Ok(())
    }

    fn label_key(&self) -> &'static str {
        "command.codex.add_relationship"
    }

    fn estimated_size_bytes(&self) -> usize {
        std::mem::size_of::<Self>()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::codex::{CodexEntryId, CodexHandle, EntryType, RelationKind};
    use crate::commands::{AddCodexEntry, CodexEntryProto};

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
    fn apply_adds_then_undo_removes() {
        let mut doc = Document::new();
        let bit = seed(&mut doc, "bit", EntryType::Character);
        let pal = seed(&mut doc, "moonlit", EntryType::Palette);
        let mut cmd = AddRelationship::new(Relationship::new(bit, RelationKind::Uses, pal));

        cmd.apply(&mut doc).unwrap();
        assert_eq!(doc.codex().relationships().len(), 1);

        cmd.undo(&mut doc).unwrap();
        assert_eq!(doc.codex().relationships().len(), 0);
    }

    #[test]
    fn missing_endpoint_errors() {
        let mut doc = Document::new();
        let bit = seed(&mut doc, "bit", EntryType::Character);
        let mut cmd = AddRelationship::new(Relationship::new(bit, RelationKind::Uses, CodexEntryId(99)));
        assert!(matches!(cmd.apply(&mut doc), Err(CommandError::CodexEntryNotFound(_))));
    }

    #[test]
    fn duplicate_edge_is_rejected() {
        let mut doc = Document::new();
        let bit = seed(&mut doc, "bit", EntryType::Character);
        let pal = seed(&mut doc, "moonlit", EntryType::Palette);
        let rel = Relationship::new(bit, RelationKind::Uses, pal);
        AddRelationship::new(rel).apply(&mut doc).unwrap();
        let mut dup = AddRelationship::new(rel);
        assert!(matches!(dup.apply(&mut doc), Err(CommandError::InvalidState)));
    }
}
