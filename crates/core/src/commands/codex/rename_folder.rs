//! [`RenameCodexFolder`]: change a folder's display name, reversibly.

use crate::codex::CodexFolderId;
use crate::command::{Command, CommandError};
use crate::document::Document;

/// Renames a folder. Undo restores the prior name.
pub struct RenameCodexFolder {
    id: CodexFolderId,
    name: Option<String>,
}

impl RenameCodexFolder {
    /// A command that will rename the folder `id` to `name`.
    pub fn new(id: CodexFolderId, name: impl Into<String>) -> Self {
        Self { id, name: Some(name.into()) }
    }
}

impl Command for RenameCodexFolder {
    fn apply(&mut self, doc: &mut Document) -> Result<(), CommandError> {
        let name = self.name.take().ok_or(CommandError::InvalidState)?;
        let folder = doc.codex_mut().folder_mut(self.id).ok_or(CommandError::CodexFolderNotFound(self.id))?;
        self.name = Some(std::mem::replace(&mut folder.name, name));
        doc.bump_revision();
        Ok(())
    }

    fn undo(&mut self, doc: &mut Document) -> Result<(), CommandError> {
        // apply and undo are symmetric: each swaps the held name with the live one.
        self.apply(doc)
    }

    fn label_key(&self) -> &'static str {
        "command.codex.rename_folder"
    }

    fn estimated_size_bytes(&self) -> usize {
        std::mem::size_of::<Self>() + self.name.as_ref().map_or(0, String::len)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::commands::CreateCodexFolder;

    fn seed(doc: &mut Document) -> CodexFolderId {
        let mut cmd = CreateCodexFolder::new(None, "Heroes");
        cmd.apply(doc).unwrap();
        cmd.inserted_id().unwrap()
    }

    #[test]
    fn apply_renames_then_undo_restores() {
        let mut doc = Document::new();
        let id = seed(&mut doc);
        let mut cmd = RenameCodexFolder::new(id, "Protagonists");

        cmd.apply(&mut doc).unwrap();
        assert_eq!(doc.codex().folder(id).unwrap().name, "Protagonists");

        cmd.undo(&mut doc).unwrap();
        assert_eq!(doc.codex().folder(id).unwrap().name, "Heroes");
    }

    #[test]
    fn missing_folder_errors() {
        let mut doc = Document::new();
        let mut cmd = RenameCodexFolder::new(CodexFolderId(9), "x");
        assert!(matches!(cmd.apply(&mut doc), Err(CommandError::CodexFolderNotFound(_))));
    }
}
