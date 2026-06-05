//! [`SetCodexFolderParent`]: move a folder under a new parent, reversibly.
//!
//! A folder cannot become its own descendant: reparenting under itself or any folder
//! beneath it is a [`CommandError::CodexFolderCycle`]. Undo restores the prior parent.

use crate::codex::CodexFolderId;
use crate::command::{Command, CommandError};
use crate::document::Document;

/// Reparents a folder. Rejects cycles and a missing new parent. Undo restores the
/// prior parent.
pub struct SetCodexFolderParent {
    id: CodexFolderId,
    parent: Option<CodexFolderId>,
    // Three states are meaningful: not-yet-applied (`None`), prior parent was the root
    // (`Some(None)`), and prior parent was a folder (`Some(Some(id))`). The outer option
    // is the apply/undo latch; the inner is the parent itself, which is genuinely
    // optional - the lint's own escape hatch for the distinguish-all-three case.
    #[allow(clippy::option_option)]
    prev: Option<Option<CodexFolderId>>,
}

impl SetCodexFolderParent {
    /// A command that will set the folder `id`'s parent to `parent` (the root when
    /// `None`).
    pub fn new(id: CodexFolderId, parent: Option<CodexFolderId>) -> Self {
        Self { id, parent, prev: None }
    }
}

impl Command for SetCodexFolderParent {
    fn apply(&mut self, doc: &mut Document) -> Result<(), CommandError> {
        let codex = doc.codex_mut();
        if codex.folder(self.id).is_none() {
            return Err(CommandError::CodexFolderNotFound(self.id));
        }
        if let Some(parent) = self.parent {
            if codex.folder(parent).is_none() {
                return Err(CommandError::CodexFolderNotFound(parent));
            }
            // The new parent must not be this folder or one of its descendants.
            if codex.folder_is_descendant(parent, self.id) {
                return Err(CommandError::CodexFolderCycle);
            }
        }
        let folder = codex.folder_mut(self.id).ok_or(CommandError::CodexFolderNotFound(self.id))?;
        self.prev = Some(folder.parent);
        folder.parent = self.parent;
        doc.bump_revision();
        Ok(())
    }

    fn undo(&mut self, doc: &mut Document) -> Result<(), CommandError> {
        let prev = self.prev.take().ok_or(CommandError::InvalidState)?;
        let folder = doc.codex_mut().folder_mut(self.id).ok_or(CommandError::CodexFolderNotFound(self.id))?;
        self.parent = std::mem::replace(&mut folder.parent, prev);
        doc.bump_revision();
        Ok(())
    }

    fn label_key(&self) -> &'static str {
        "command.codex.set_folder_parent"
    }

    fn estimated_size_bytes(&self) -> usize {
        std::mem::size_of::<Self>()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::commands::CreateCodexFolder;

    fn folder(doc: &mut Document, parent: Option<CodexFolderId>, name: &str) -> CodexFolderId {
        let mut cmd = CreateCodexFolder::new(parent, name);
        cmd.apply(doc).unwrap();
        cmd.inserted_id().unwrap()
    }

    #[test]
    fn apply_moves_then_undo_restores() {
        let mut doc = Document::new();
        let a = folder(&mut doc, None, "a");
        let b = folder(&mut doc, None, "b");
        let mut cmd = SetCodexFolderParent::new(b, Some(a));

        cmd.apply(&mut doc).unwrap();
        assert_eq!(doc.codex().folder(b).unwrap().parent, Some(a));

        cmd.undo(&mut doc).unwrap();
        assert_eq!(doc.codex().folder(b).unwrap().parent, None);
    }

    #[test]
    fn reparenting_under_self_is_a_cycle() {
        let mut doc = Document::new();
        let a = folder(&mut doc, None, "a");
        let mut cmd = SetCodexFolderParent::new(a, Some(a));
        assert!(matches!(cmd.apply(&mut doc), Err(CommandError::CodexFolderCycle)));
    }

    #[test]
    fn reparenting_under_a_descendant_is_a_cycle() {
        let mut doc = Document::new();
        let a = folder(&mut doc, None, "a");
        let b = folder(&mut doc, Some(a), "b");
        // Moving a under b would put a beneath its own child.
        let mut cmd = SetCodexFolderParent::new(a, Some(b));
        assert!(matches!(cmd.apply(&mut doc), Err(CommandError::CodexFolderCycle)));
    }

    #[test]
    fn missing_parent_errors() {
        let mut doc = Document::new();
        let a = folder(&mut doc, None, "a");
        let mut cmd = SetCodexFolderParent::new(a, Some(CodexFolderId(99)));
        assert!(matches!(cmd.apply(&mut doc), Err(CommandError::CodexFolderNotFound(_))));
    }
}
