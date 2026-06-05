//! [`AddCodexAlias`]: add an alternate handle to an entry, reversibly.
//!
//! An alias is a second handle that resolves to the same entry (bible 6.4). It must
//! be free across every entry's primary handle and aliases. Undo removes exactly the
//! alias that was added.

use crate::codex::{CodexEntryId, CodexHandle};
use crate::command::{Command, CommandError};
use crate::document::Document;

/// Adds an alias handle to an entry. Fails if the handle is already claimed anywhere
/// in the Codex. Undo removes it.
pub struct AddCodexAlias {
    id: CodexEntryId,
    alias: CodexHandle,
    added: bool,
}

impl AddCodexAlias {
    /// A command that will add `alias` to the entry `id`.
    pub fn new(id: CodexEntryId, alias: CodexHandle) -> Self {
        Self { id, alias, added: false }
    }
}

impl Command for AddCodexAlias {
    fn apply(&mut self, doc: &mut Document) -> Result<(), CommandError> {
        let codex = doc.codex_mut();
        if codex.entry(self.id).is_none() {
            return Err(CommandError::CodexEntryNotFound(self.id));
        }
        if codex.handle_in_use(&self.alias) {
            return Err(CommandError::CodexHandleInUse);
        }
        let entry = codex.entry_mut(self.id).ok_or(CommandError::CodexEntryNotFound(self.id))?;
        entry.aliases.push(self.alias.clone());
        self.added = true;
        doc.bump_revision();
        Ok(())
    }

    fn undo(&mut self, doc: &mut Document) -> Result<(), CommandError> {
        if !self.added {
            return Err(CommandError::InvalidState);
        }
        let entry = doc.codex_mut().entry_mut(self.id).ok_or(CommandError::CodexEntryNotFound(self.id))?;
        let pos = entry.aliases.iter().position(|a| a == &self.alias).ok_or(CommandError::InvalidState)?;
        entry.aliases.remove(pos);
        self.added = false;
        doc.bump_revision();
        Ok(())
    }

    fn label_key(&self) -> &'static str {
        "command.codex.add_alias"
    }

    fn estimated_size_bytes(&self) -> usize {
        std::mem::size_of::<Self>() + self.alias.as_str().len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::{handle, seed_handle};

    #[test]
    fn apply_adds_then_undo_removes() {
        let mut doc = Document::new();
        let id = seed_handle(&mut doc, "bit");
        let mut cmd = AddCodexAlias::new(id, handle("mascot"));

        cmd.apply(&mut doc).unwrap();
        assert_eq!(doc.codex().resolve_handle(&handle("mascot")), Some(id));

        cmd.undo(&mut doc).unwrap();
        assert!(doc.codex().entry(id).unwrap().aliases.is_empty());
    }

    #[test]
    fn duplicate_alias_is_rejected() {
        let mut doc = Document::new();
        let id = seed_handle(&mut doc, "bit");
        seed_handle(&mut doc, "mossy");
        let mut cmd = AddCodexAlias::new(id, handle("mossy"));
        assert!(matches!(cmd.apply(&mut doc), Err(CommandError::CodexHandleInUse)));
    }

    #[test]
    fn missing_entry_errors() {
        let mut doc = Document::new();
        let mut cmd = AddCodexAlias::new(CodexEntryId(3), handle("x"));
        assert!(matches!(cmd.apply(&mut doc), Err(CommandError::CodexEntryNotFound(_))));
    }
}
