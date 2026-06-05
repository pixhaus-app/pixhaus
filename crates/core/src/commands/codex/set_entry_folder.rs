//! [`SetCodexEntryFolder`]: move an entry into a folder (or the root), reversibly.

use crate::codex::{CodexEntryId, CodexFolderId};
use crate::command::{Command, CommandError};
use crate::document::Document;

/// Moves an entry into `folder` (the root when `None`). Fails if the target folder is
/// given but absent. Undo restores the entry's prior folder.
pub struct SetCodexEntryFolder {
    entry: CodexEntryId,
    folder: Option<CodexFolderId>,
    // Three meaningful states: not-yet-applied (`None`), prior folder was the root
    // (`Some(None)`), and prior folder was a real folder (`Some(Some(id))`). The outer
    // option is the apply/undo latch; the inner is the folder, which is genuinely
    // optional - the lint's own escape hatch for the distinguish-all-three case.
    #[allow(clippy::option_option)]
    prev: Option<Option<CodexFolderId>>,
}

impl SetCodexEntryFolder {
    /// A command that will move the entry `entry` into `folder`.
    pub fn new(entry: CodexEntryId, folder: Option<CodexFolderId>) -> Self {
        Self { entry, folder, prev: None }
    }
}

impl Command for SetCodexEntryFolder {
    fn apply(&mut self, doc: &mut Document) -> Result<(), CommandError> {
        let codex = doc.codex_mut();
        if codex.entry(self.entry).is_none() {
            return Err(CommandError::CodexEntryNotFound(self.entry));
        }
        if let Some(folder) = self.folder
            && codex.folder(folder).is_none()
        {
            return Err(CommandError::CodexFolderNotFound(folder));
        }
        let entry = codex.entry_mut(self.entry).ok_or(CommandError::CodexEntryNotFound(self.entry))?;
        self.prev = Some(entry.folder_id);
        entry.folder_id = self.folder;
        doc.bump_revision();
        Ok(())
    }

    fn undo(&mut self, doc: &mut Document) -> Result<(), CommandError> {
        let prev = self.prev.take().ok_or(CommandError::InvalidState)?;
        let entry = doc.codex_mut().entry_mut(self.entry).ok_or(CommandError::CodexEntryNotFound(self.entry))?;
        self.folder = std::mem::replace(&mut entry.folder_id, prev);
        doc.bump_revision();
        Ok(())
    }

    fn label_key(&self) -> &'static str {
        "command.codex.set_entry_folder"
    }

    fn estimated_size_bytes(&self) -> usize {
        std::mem::size_of::<Self>()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::codex::{CodexHandle, EntryType};
    use crate::commands::{AddCodexEntry, CodexEntryProto, CreateCodexFolder};

    fn entry(doc: &mut Document) -> CodexEntryId {
        let mut add = AddCodexEntry::new(CodexEntryProto {
            handle: CodexHandle::new("bit").unwrap(),
            name: "Bit".to_owned(),
            entry_type: EntryType::Character,
        });
        add.apply(doc).unwrap();
        add.inserted_id().unwrap()
    }

    fn folder(doc: &mut Document) -> CodexFolderId {
        let mut cmd = CreateCodexFolder::new(None, "Heroes");
        cmd.apply(doc).unwrap();
        cmd.inserted_id().unwrap()
    }

    #[test]
    fn apply_moves_then_undo_restores() {
        let mut doc = Document::new();
        let e = entry(&mut doc);
        let f = folder(&mut doc);
        let mut cmd = SetCodexEntryFolder::new(e, Some(f));

        cmd.apply(&mut doc).unwrap();
        assert_eq!(doc.codex().entry(e).unwrap().folder_id, Some(f));

        cmd.undo(&mut doc).unwrap();
        assert_eq!(doc.codex().entry(e).unwrap().folder_id, None);
    }

    #[test]
    fn missing_folder_errors() {
        let mut doc = Document::new();
        let e = entry(&mut doc);
        let mut cmd = SetCodexEntryFolder::new(e, Some(CodexFolderId(99)));
        assert!(matches!(cmd.apply(&mut doc), Err(CommandError::CodexFolderNotFound(_))));
    }

    #[test]
    fn missing_entry_errors() {
        let mut doc = Document::new();
        let mut cmd = SetCodexEntryFolder::new(CodexEntryId(99), None);
        assert!(matches!(cmd.apply(&mut doc), Err(CommandError::CodexEntryNotFound(_))));
    }
}
