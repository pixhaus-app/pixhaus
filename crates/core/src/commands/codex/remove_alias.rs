//! [`RemoveCodexAlias`]: drop an alternate handle from an entry, reversibly.
//!
//! Undo restores the alias at the position it held, so a redo/undo cycle keeps the
//! alias order stable.

use crate::codex::{CodexEntryId, CodexHandle};
use crate::command::{Command, CommandError};
use crate::document::Document;

/// Removes an alias handle from an entry. Fails if the entry has no such alias. Undo
/// restores it at its prior position.
pub struct RemoveCodexAlias {
    id: CodexEntryId,
    alias: CodexHandle,
    /// The index the alias held before removal, captured so undo restores order.
    removed_at: Option<usize>,
}

impl RemoveCodexAlias {
    /// A command that will remove `alias` from the entry `id`.
    pub fn new(id: CodexEntryId, alias: CodexHandle) -> Self {
        Self { id, alias, removed_at: None }
    }
}

impl Command for RemoveCodexAlias {
    fn apply(&mut self, doc: &mut Document) -> Result<(), CommandError> {
        let entry = doc.codex_mut().entry_mut(self.id).ok_or(CommandError::CodexEntryNotFound(self.id))?;
        let pos = entry.aliases.iter().position(|a| a == &self.alias).ok_or(CommandError::InvalidState)?;
        entry.aliases.remove(pos);
        self.removed_at = Some(pos);
        doc.bump_revision();
        Ok(())
    }

    fn undo(&mut self, doc: &mut Document) -> Result<(), CommandError> {
        let pos = self.removed_at.take().ok_or(CommandError::InvalidState)?;
        let entry = doc.codex_mut().entry_mut(self.id).ok_or(CommandError::CodexEntryNotFound(self.id))?;
        let pos = pos.min(entry.aliases.len());
        entry.aliases.insert(pos, self.alias.clone());
        doc.bump_revision();
        Ok(())
    }

    fn label_key(&self) -> &'static str {
        "command.codex.remove_alias"
    }

    fn estimated_size_bytes(&self) -> usize {
        std::mem::size_of::<Self>() + self.alias.as_str().len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::codex::EntryType;
    use crate::commands::{AddCodexAlias, AddCodexEntry, CodexEntryProto};

    fn seed(doc: &mut Document, handle: &str) -> CodexEntryId {
        let mut add = AddCodexEntry::new(CodexEntryProto {
            handle: CodexHandle::new(handle).unwrap(),
            name: "Bit".to_owned(),
            entry_type: EntryType::Character,
        });
        add.apply(doc).unwrap();
        add.inserted_id().unwrap()
    }

    fn handle(s: &str) -> CodexHandle {
        CodexHandle::new(s).unwrap()
    }

    #[test]
    fn apply_removes_then_undo_restores() {
        let mut doc = Document::new();
        let id = seed(&mut doc, "bit");
        AddCodexAlias::new(id, handle("mascot")).apply(&mut doc).unwrap();
        let mut cmd = RemoveCodexAlias::new(id, handle("mascot"));

        cmd.apply(&mut doc).unwrap();
        assert!(doc.codex().entry(id).unwrap().aliases.is_empty());

        cmd.undo(&mut doc).unwrap();
        assert_eq!(doc.codex().resolve_handle(&handle("mascot")), Some(id));
    }

    #[test]
    fn absent_alias_errors() {
        let mut doc = Document::new();
        let id = seed(&mut doc, "bit");
        let mut cmd = RemoveCodexAlias::new(id, handle("nope"));
        assert!(matches!(cmd.apply(&mut doc), Err(CommandError::InvalidState)));
    }
}
