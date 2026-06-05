//! [`AddCodexEntry`]: insert a new entry into the Codex.

use crate::codex::{CodexEntry, CodexEntryId, CodexHandle, EntryType};
use crate::command::{Command, CommandError};
use crate::document::Document;

/// The parameters for a new Codex entry: enough to build the header, with a body
/// defaulted from the type.
#[derive(Clone, Debug)]
pub struct CodexEntryProto {
    /// The validated `@`-mention handle.
    pub handle: CodexHandle,
    /// Display name (project content).
    pub name: String,
    /// The entry's namespace.
    pub entry_type: EntryType,
}

/// Records what [`AddCodexEntry::apply`] inserted, so undo removes exactly that.
struct Inserted {
    id: CodexEntryId,
}

/// Adds a new Codex entry with a defaulted body and returns it via the Codex. Fails
/// if the handle is already claimed by another entry.
pub struct AddCodexEntry {
    proto: Option<CodexEntryProto>,
    inserted: Option<Inserted>,
}

impl AddCodexEntry {
    /// A command that will add a Codex entry described by `proto`.
    pub fn new(proto: CodexEntryProto) -> Self {
        Self {
            proto: Some(proto),
            inserted: None,
        }
    }

    /// The id assigned to the inserted entry, available after [`apply`](Command::apply).
    pub fn inserted_id(&self) -> Option<CodexEntryId> {
        self.inserted.as_ref().map(|i| i.id)
    }
}

impl Command for AddCodexEntry {
    fn apply(&mut self, doc: &mut Document) -> Result<(), CommandError> {
        let proto = self.proto.take().ok_or(CommandError::InvalidState)?;
        let codex = doc.codex_mut();
        if codex.handle_in_use(&proto.handle) {
            // Restore the proto so the caller can fix the handle and retry.
            self.proto = Some(proto);
            return Err(CommandError::CodexHandleInUse);
        }
        let id = codex.mint_entry_id();
        let entry = CodexEntry::new(id, proto.handle, proto.name, proto.entry_type);
        codex.insert_entry(entry);
        doc.bump_revision();
        self.inserted = Some(Inserted { id });
        Ok(())
    }

    fn undo(&mut self, doc: &mut Document) -> Result<(), CommandError> {
        let inserted = self.inserted.take().ok_or(CommandError::InvalidState)?;
        let entry = doc.codex_mut().remove_entry(inserted.id).ok_or(CommandError::CodexEntryNotFound(inserted.id))?;
        doc.bump_revision();
        // Restore the proto so a redo re-adds the same entry.
        self.proto = Some(CodexEntryProto {
            handle: entry.handle,
            name: entry.name,
            entry_type: entry.entry_type,
        });
        Ok(())
    }

    fn label_key(&self) -> &'static str {
        "command.codex.add_entry"
    }

    fn estimated_size_bytes(&self) -> usize {
        std::mem::size_of::<Self>()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn proto(handle: &str) -> CodexEntryProto {
        CodexEntryProto {
            handle: CodexHandle::new(handle).unwrap(),
            name: "Bit".to_owned(),
            entry_type: EntryType::Character,
        }
    }

    #[test]
    fn apply_adds_entry_then_undo_removes_it() {
        let mut doc = Document::new();
        let mut cmd = AddCodexEntry::new(proto("bit"));
        cmd.apply(&mut doc).unwrap();
        let id = cmd.inserted_id().unwrap();
        assert_eq!(doc.codex().entries().len(), 1);
        assert!(doc.codex().entry(id).is_some());

        cmd.undo(&mut doc).unwrap();
        assert_eq!(doc.codex().entries().len(), 0);
    }

    #[test]
    fn redo_after_undo_re_adds() {
        let mut doc = Document::new();
        let mut cmd = AddCodexEntry::new(proto("bit"));
        cmd.apply(&mut doc).unwrap();
        cmd.undo(&mut doc).unwrap();
        cmd.apply(&mut doc).unwrap();
        assert_eq!(doc.codex().entries().len(), 1);
    }

    #[test]
    fn duplicate_handle_is_rejected() {
        let mut doc = Document::new();
        AddCodexEntry::new(proto("bit")).apply(&mut doc).unwrap();
        let mut dup = AddCodexEntry::new(proto("bit"));
        assert!(matches!(dup.apply(&mut doc), Err(CommandError::CodexHandleInUse)));
        assert_eq!(doc.codex().entries().len(), 1);
    }

    #[test]
    fn undo_before_apply_is_invalid_state() {
        let mut doc = Document::new();
        let mut cmd = AddCodexEntry::new(proto("bit"));
        assert!(matches!(cmd.undo(&mut doc), Err(CommandError::InvalidState)));
    }
}
