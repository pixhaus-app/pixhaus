//! [`SetEntryStatus`]: change an entry's status, reversibly.

use crate::codex::{CodexEntryId, EntryStatus};
use crate::command::{Command, CommandError};
use crate::document::Document;

/// Sets an entry's [`EntryStatus`] and restores the prior status on undo.
pub struct SetEntryStatus {
    id: CodexEntryId,
    status: EntryStatus,
    prev: Option<EntryStatus>,
}

impl SetEntryStatus {
    /// A command that will set the entry `id` to `status`.
    pub fn new(id: CodexEntryId, status: EntryStatus) -> Self {
        Self { id, status, prev: None }
    }
}

impl Command for SetEntryStatus {
    fn apply(&mut self, doc: &mut Document) -> Result<(), CommandError> {
        let entry = doc.codex_mut().entry_mut(self.id).ok_or(CommandError::CodexEntryNotFound(self.id))?;
        self.prev = Some(std::mem::replace(&mut entry.status, self.status));
        doc.bump_revision();
        Ok(())
    }

    fn undo(&mut self, doc: &mut Document) -> Result<(), CommandError> {
        let prev = self.prev.take().ok_or(CommandError::InvalidState)?;
        let entry = doc.codex_mut().entry_mut(self.id).ok_or(CommandError::CodexEntryNotFound(self.id))?;
        entry.status = prev;
        doc.bump_revision();
        Ok(())
    }

    fn label_key(&self) -> &'static str {
        "command.codex.set_status"
    }

    fn estimated_size_bytes(&self) -> usize {
        std::mem::size_of::<Self>()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::seed_bit;

    #[test]
    fn apply_sets_then_undo_restores() {
        let mut doc = Document::new();
        let id = seed_bit(&mut doc);
        assert_eq!(doc.codex().entry(id).unwrap().status, EntryStatus::Draft);
        let mut cmd = SetEntryStatus::new(id, EntryStatus::Canonical);

        cmd.apply(&mut doc).unwrap();
        assert_eq!(doc.codex().entry(id).unwrap().status, EntryStatus::Canonical);

        cmd.undo(&mut doc).unwrap();
        assert_eq!(doc.codex().entry(id).unwrap().status, EntryStatus::Draft);
    }

    #[test]
    fn missing_entry_errors() {
        let mut doc = Document::new();
        let mut cmd = SetEntryStatus::new(CodexEntryId(5), EntryStatus::Canonical);
        assert!(matches!(cmd.apply(&mut doc), Err(CommandError::CodexEntryNotFound(_))));
    }
}
