//! [`DeleteCodexEntry`]: remove an entry, restoring the whole entry on undo.

use crate::codex::{CodexEntry, CodexEntryId};
use crate::command::{Command, CommandError};
use crate::document::Document;

/// Removes a Codex entry. Undo restores the exact entry that was removed. Relationships
/// and coverage that reference the entry are not touched here; cascading removal is a
/// transaction over this and the relationship/coverage commands.
pub struct DeleteCodexEntry {
    id: CodexEntryId,
    removed: Option<CodexEntry>,
}

impl DeleteCodexEntry {
    /// A command that will delete the entry `id`.
    pub fn new(id: CodexEntryId) -> Self {
        Self { id, removed: None }
    }
}

impl Command for DeleteCodexEntry {
    fn apply(&mut self, doc: &mut Document) -> Result<(), CommandError> {
        let removed = doc.codex_mut().remove_entry(self.id).ok_or(CommandError::CodexEntryNotFound(self.id))?;
        doc.bump_revision();
        self.removed = Some(removed);
        Ok(())
    }

    fn undo(&mut self, doc: &mut Document) -> Result<(), CommandError> {
        let removed = self.removed.take().ok_or(CommandError::InvalidState)?;
        doc.codex_mut().insert_entry(removed);
        doc.bump_revision();
        Ok(())
    }

    fn label_key(&self) -> &'static str {
        "command.codex.delete_entry"
    }

    fn estimated_size_bytes(&self) -> usize {
        std::mem::size_of::<Self>()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::codex::{CodexHandle, EntryType};
    use crate::commands::{AddCodexEntry, CodexEntryProto};

    fn seed(doc: &mut Document, handle: &str) -> CodexEntryId {
        let mut add = AddCodexEntry::new(CodexEntryProto {
            handle: CodexHandle::new(handle).unwrap(),
            name: "Bit".to_owned(),
            entry_type: EntryType::Character,
        });
        add.apply(doc).unwrap();
        add.inserted_id().unwrap()
    }

    #[test]
    fn apply_removes_then_undo_restores() {
        let mut doc = Document::new();
        let id = seed(&mut doc, "bit");
        let mut cmd = DeleteCodexEntry::new(id);

        cmd.apply(&mut doc).unwrap();
        assert!(doc.codex().entry(id).is_none());

        cmd.undo(&mut doc).unwrap();
        let restored = doc.codex().entry(id).unwrap();
        assert_eq!(restored.handle.as_str(), "bit");
        assert_eq!(restored.name, "Bit");
    }

    #[test]
    fn missing_entry_errors() {
        let mut doc = Document::new();
        let mut cmd = DeleteCodexEntry::new(CodexEntryId(7));
        assert!(matches!(cmd.apply(&mut doc), Err(CommandError::CodexEntryNotFound(_))));
    }

    #[test]
    fn undo_before_apply_is_invalid_state() {
        let mut doc = Document::new();
        let mut cmd = DeleteCodexEntry::new(CodexEntryId(1));
        assert!(matches!(cmd.undo(&mut doc), Err(CommandError::InvalidState)));
    }
}
