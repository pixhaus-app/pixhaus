//! [`CreateCodexFolder`]: mint a new folder, reversibly.
//!
//! The parent must exist when given; `None` creates a root folder. Undo removes the
//! exact folder that was minted; a redo re-mints under the same parent with the same
//! name.

use crate::codex::{CodexFolder, CodexFolderId};
use crate::command::{Command, CommandError};
use crate::document::Document;

/// Records what [`CreateCodexFolder::apply`] inserted, so undo removes exactly that.
struct Inserted {
    id: CodexFolderId,
}

/// Creates a folder under `parent` (or the root when `None`). Fails if `parent` is
/// given but absent. The minted id is available after apply via [`inserted_id`](Self::inserted_id).
pub struct CreateCodexFolder {
    parent: Option<CodexFolderId>,
    name: String,
    inserted: Option<Inserted>,
}

impl CreateCodexFolder {
    /// A command that will create a folder named `name` under `parent`.
    pub fn new(parent: Option<CodexFolderId>, name: impl Into<String>) -> Self {
        Self {
            parent,
            name: name.into(),
            inserted: None,
        }
    }

    /// The id assigned to the created folder, available after [`apply`](Command::apply).
    pub fn inserted_id(&self) -> Option<CodexFolderId> {
        self.inserted.as_ref().map(|i| i.id)
    }
}

impl Command for CreateCodexFolder {
    fn apply(&mut self, doc: &mut Document) -> Result<(), CommandError> {
        let codex = doc.codex_mut();
        if let Some(parent) = self.parent
            && codex.folder(parent).is_none()
        {
            return Err(CommandError::CodexFolderNotFound(parent));
        }
        let id = codex.mint_folder_id();
        codex.insert_folder(CodexFolder::new(id, self.name.clone(), self.parent));
        self.inserted = Some(Inserted { id });
        doc.bump_revision();
        Ok(())
    }

    fn undo(&mut self, doc: &mut Document) -> Result<(), CommandError> {
        let inserted = self.inserted.take().ok_or(CommandError::InvalidState)?;
        doc.codex_mut()
            .remove_folder(inserted.id)
            .ok_or(CommandError::CodexFolderNotFound(inserted.id))?;
        doc.bump_revision();
        Ok(())
    }

    fn label_key(&self) -> &'static str {
        "command.codex.create_folder"
    }

    fn estimated_size_bytes(&self) -> usize {
        std::mem::size_of::<Self>() + self.name.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn apply_creates_then_undo_removes() {
        let mut doc = Document::new();
        let mut cmd = CreateCodexFolder::new(None, "Heroes");

        cmd.apply(&mut doc).unwrap();
        let id = cmd.inserted_id().unwrap();
        assert_eq!(doc.codex().folder(id).unwrap().name, "Heroes");

        cmd.undo(&mut doc).unwrap();
        assert!(doc.codex().folder(id).is_none());
    }

    #[test]
    fn creating_under_a_parent_links_it() {
        let mut doc = Document::new();
        let mut root = CreateCodexFolder::new(None, "Heroes");
        root.apply(&mut doc).unwrap();
        let root_id = root.inserted_id().unwrap();
        let mut child = CreateCodexFolder::new(Some(root_id), "Bosses");
        child.apply(&mut doc).unwrap();
        let child_id = child.inserted_id().unwrap();
        assert_eq!(doc.codex().folder(child_id).unwrap().parent, Some(root_id));
    }

    #[test]
    fn missing_parent_errors() {
        let mut doc = Document::new();
        let mut cmd = CreateCodexFolder::new(Some(CodexFolderId(9)), "Orphans");
        assert!(matches!(cmd.apply(&mut doc), Err(CommandError::CodexFolderNotFound(_))));
    }
}
