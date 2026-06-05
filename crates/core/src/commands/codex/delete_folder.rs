//! [`DeleteCodexFolder`]: remove a folder, reparenting its contents, reversibly.
//!
//! Deleting a folder never orphans anything: its child folders and its entries move up
//! to the deleted folder's own parent. Undo restores the folder and the prior parent
//! of every folder it reparented and the prior `folder_id` of every entry it moved.

use crate::codex::{CodexEntryId, CodexFolder, CodexFolderId};
use crate::command::{Command, CommandError};
use crate::document::Document;

/// What apply removed and moved, captured so undo restores it exactly.
struct Undo {
    /// The deleted folder itself.
    folder: CodexFolder,
    /// Child folders that were reparented: `(folder, prior parent)`.
    moved_folders: Vec<(CodexFolderId, Option<CodexFolderId>)>,
    /// Entries that were moved: `(entry, prior folder_id)`.
    moved_entries: Vec<(CodexEntryId, Option<CodexFolderId>)>,
}

/// Deletes a folder, reparenting its child folders and entries to the deleted
/// folder's parent. Undo restores the folder and everything it touched.
pub struct DeleteCodexFolder {
    id: CodexFolderId,
    undo: Option<Undo>,
}

impl DeleteCodexFolder {
    /// A command that will delete the folder `id`.
    pub fn new(id: CodexFolderId) -> Self {
        Self { id, undo: None }
    }
}

impl Command for DeleteCodexFolder {
    fn apply(&mut self, doc: &mut Document) -> Result<(), CommandError> {
        let codex = doc.codex_mut();
        let Some(folder) = codex.folder(self.id) else {
            return Err(CommandError::CodexFolderNotFound(self.id));
        };
        let new_parent = folder.parent;

        // Reparent child folders up to the deleted folder's parent.
        let child_ids = codex.child_folders(Some(self.id));
        let mut moved_folders = Vec::with_capacity(child_ids.len());
        for child in child_ids {
            if let Some(f) = codex.folder_mut(child) {
                moved_folders.push((child, f.parent));
                f.parent = new_parent;
            }
        }

        // Move contained entries up to the deleted folder's parent.
        let entry_ids = codex.entries_in_folder(Some(self.id));
        let mut moved_entries = Vec::with_capacity(entry_ids.len());
        for entry in entry_ids {
            if let Some(e) = codex.entry_mut(entry) {
                moved_entries.push((entry, e.folder_id));
                e.folder_id = new_parent;
            }
        }

        let folder = codex.remove_folder(self.id).ok_or(CommandError::CodexFolderNotFound(self.id))?;
        self.undo = Some(Undo {
            folder,
            moved_folders,
            moved_entries,
        });
        doc.bump_revision();
        Ok(())
    }

    fn undo(&mut self, doc: &mut Document) -> Result<(), CommandError> {
        let undo = self.undo.take().ok_or(CommandError::InvalidState)?;
        let codex = doc.codex_mut();
        codex.insert_folder(undo.folder);
        for (child, parent) in undo.moved_folders {
            if let Some(f) = codex.folder_mut(child) {
                f.parent = parent;
            }
        }
        for (entry, folder_id) in undo.moved_entries {
            if let Some(e) = codex.entry_mut(entry) {
                e.folder_id = folder_id;
            }
        }
        doc.bump_revision();
        Ok(())
    }

    fn label_key(&self) -> &'static str {
        "command.codex.delete_folder"
    }

    fn estimated_size_bytes(&self) -> usize {
        let undo = self.undo.as_ref();
        std::mem::size_of::<Self>()
            + undo.map_or(0, |u| {
                u.folder.name.len()
                    + u.folder.description.len()
                    + u.moved_folders.len() * std::mem::size_of::<(CodexFolderId, Option<CodexFolderId>)>()
                    + u.moved_entries.len() * std::mem::size_of::<(CodexEntryId, Option<CodexFolderId>)>()
            })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::codex::{CodexHandle, EntryType};
    use crate::commands::{AddCodexEntry, CodexEntryProto, CreateCodexFolder, SetCodexEntryFolder};

    fn folder(doc: &mut Document, parent: Option<CodexFolderId>, name: &str) -> CodexFolderId {
        let mut cmd = CreateCodexFolder::new(parent, name);
        cmd.apply(doc).unwrap();
        cmd.inserted_id().unwrap()
    }

    fn entry(doc: &mut Document, handle: &str) -> CodexEntryId {
        let mut add = AddCodexEntry::new(CodexEntryProto {
            handle: CodexHandle::new(handle).unwrap(),
            name: "x".to_owned(),
            entry_type: EntryType::Character,
        });
        add.apply(doc).unwrap();
        add.inserted_id().unwrap()
    }

    #[test]
    fn delete_reparents_children_then_undo_restores() {
        let mut doc = Document::new();
        let root = folder(&mut doc, None, "root");
        let mid = folder(&mut doc, Some(root), "mid");
        let leaf = folder(&mut doc, Some(mid), "leaf");
        let e = entry(&mut doc, "bit");
        SetCodexEntryFolder::new(e, Some(mid)).apply(&mut doc).unwrap();

        let mut cmd = DeleteCodexFolder::new(mid);
        cmd.apply(&mut doc).unwrap();
        // mid is gone; leaf and the entry moved up to root.
        assert!(doc.codex().folder(mid).is_none());
        assert_eq!(doc.codex().folder(leaf).unwrap().parent, Some(root));
        assert_eq!(doc.codex().entry(e).unwrap().folder_id, Some(root));

        cmd.undo(&mut doc).unwrap();
        assert_eq!(doc.codex().folder(mid).unwrap().parent, Some(root));
        assert_eq!(doc.codex().folder(leaf).unwrap().parent, Some(mid));
        assert_eq!(doc.codex().entry(e).unwrap().folder_id, Some(mid));
    }

    #[test]
    fn deleting_a_root_folder_moves_contents_to_root() {
        let mut doc = Document::new();
        let root = folder(&mut doc, None, "root");
        let e = entry(&mut doc, "bit");
        SetCodexEntryFolder::new(e, Some(root)).apply(&mut doc).unwrap();

        DeleteCodexFolder::new(root).apply(&mut doc).unwrap();
        assert_eq!(doc.codex().entry(e).unwrap().folder_id, None);
    }

    #[test]
    fn missing_folder_errors() {
        let mut doc = Document::new();
        let mut cmd = DeleteCodexFolder::new(CodexFolderId(9));
        assert!(matches!(cmd.apply(&mut doc), Err(CommandError::CodexFolderNotFound(_))));
    }
}
