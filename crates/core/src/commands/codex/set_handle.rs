//! [`SetCodexHandle`]: rename an entry's primary handle, reversibly.
//!
//! References point at the entry id, never the handle, so a rename only changes the
//! display token — it never breaks a reference. The new handle must be free across
//! every entry's primary handle and aliases, and the entry's handle lock must be
//! clear.

use crate::codex::{CodexEntryId, CodexHandle};
use crate::command::{Command, CommandError};
use crate::document::Document;

/// Renames an entry's primary handle. Fails if the handle is already claimed by
/// another entry (or alias), or if the entry's handle facet is locked. Undo restores
/// the prior handle.
pub struct SetCodexHandle {
    id: CodexEntryId,
    new_handle: Option<CodexHandle>,
    prev: Option<CodexHandle>,
}

impl SetCodexHandle {
    /// A command that will set the entry `id`'s primary handle to `new_handle`.
    pub fn new(id: CodexEntryId, new_handle: CodexHandle) -> Self {
        Self {
            id,
            new_handle: Some(new_handle),
            prev: None,
        }
    }
}

impl Command for SetCodexHandle {
    fn apply(&mut self, doc: &mut Document) -> Result<(), CommandError> {
        let new_handle = self.new_handle.take().ok_or(CommandError::InvalidState)?;
        let codex = doc.codex_mut();
        let Some(entry) = codex.entry(self.id) else {
            self.new_handle = Some(new_handle);
            return Err(CommandError::CodexEntryNotFound(self.id));
        };
        // A no-op rename is always allowed; only a real change checks locks and uniqueness.
        if entry.handle == new_handle {
            self.prev = Some(new_handle);
            doc.bump_revision();
            return Ok(());
        }
        if entry.locks.handle {
            self.new_handle = Some(new_handle);
            return Err(CommandError::CodexEntryLocked);
        }
        // The handle must be free across every entry (its own current handle aside).
        if let Some(owner) = codex.resolve_handle(&new_handle)
            && owner != self.id
        {
            self.new_handle = Some(new_handle);
            return Err(CommandError::CodexHandleInUse);
        }
        let entry = codex.entry_mut(self.id).ok_or(CommandError::CodexEntryNotFound(self.id))?;
        self.prev = Some(std::mem::replace(&mut entry.handle, new_handle));
        doc.bump_revision();
        Ok(())
    }

    fn undo(&mut self, doc: &mut Document) -> Result<(), CommandError> {
        let prev = self.prev.take().ok_or(CommandError::InvalidState)?;
        let entry = doc.codex_mut().entry_mut(self.id).ok_or(CommandError::CodexEntryNotFound(self.id))?;
        self.new_handle = Some(std::mem::replace(&mut entry.handle, prev));
        doc.bump_revision();
        Ok(())
    }

    fn label_key(&self) -> &'static str {
        "command.codex.set_handle"
    }

    fn estimated_size_bytes(&self) -> usize {
        std::mem::size_of::<Self>() + self.new_handle.as_ref().map_or(0, |h| h.as_str().len()) + self.prev.as_ref().map_or(0, |h| h.as_str().len())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::codex::EntryType;
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

    fn handle(s: &str) -> CodexHandle {
        CodexHandle::new(s).unwrap()
    }

    #[test]
    fn apply_renames_then_undo_restores() {
        let mut doc = Document::new();
        let id = seed(&mut doc, "bit");
        let mut cmd = SetCodexHandle::new(id, handle("bit_v2"));

        cmd.apply(&mut doc).unwrap();
        assert_eq!(doc.codex().entry(id).unwrap().handle.as_str(), "bit_v2");

        cmd.undo(&mut doc).unwrap();
        assert_eq!(doc.codex().entry(id).unwrap().handle.as_str(), "bit");
    }

    #[test]
    fn redo_re_applies() {
        let mut doc = Document::new();
        let id = seed(&mut doc, "bit");
        let mut cmd = SetCodexHandle::new(id, handle("bit_v2"));
        cmd.apply(&mut doc).unwrap();
        cmd.undo(&mut doc).unwrap();
        cmd.apply(&mut doc).unwrap();
        assert_eq!(doc.codex().entry(id).unwrap().handle.as_str(), "bit_v2");
    }

    #[test]
    fn handle_in_use_is_rejected() {
        let mut doc = Document::new();
        let id = seed(&mut doc, "bit");
        seed(&mut doc, "mossy");
        let mut cmd = SetCodexHandle::new(id, handle("mossy"));
        assert!(matches!(cmd.apply(&mut doc), Err(CommandError::CodexHandleInUse)));
        assert_eq!(doc.codex().entry(id).unwrap().handle.as_str(), "bit");
    }

    #[test]
    fn renaming_to_own_handle_is_a_noop() {
        let mut doc = Document::new();
        let id = seed(&mut doc, "bit");
        let mut cmd = SetCodexHandle::new(id, handle("bit"));
        cmd.apply(&mut doc).unwrap();
        assert_eq!(doc.codex().entry(id).unwrap().handle.as_str(), "bit");
    }

    #[test]
    fn locked_handle_is_rejected() {
        let mut doc = Document::new();
        let id = seed(&mut doc, "bit");
        doc.codex_mut().entry_mut(id).unwrap().locks.handle = true;
        let mut cmd = SetCodexHandle::new(id, handle("bit_v2"));
        assert!(matches!(cmd.apply(&mut doc), Err(CommandError::CodexEntryLocked)));
    }

    #[test]
    fn missing_entry_errors() {
        let mut doc = Document::new();
        let mut cmd = SetCodexHandle::new(CodexEntryId(9), handle("nope"));
        assert!(matches!(cmd.apply(&mut doc), Err(CommandError::CodexEntryNotFound(_))));
    }
}
