//! [`RenameCodexFolder`]: change a folder's display name, reversibly.

use crate::codex::CodexFolderId;
use crate::command::CommandError;
use crate::commands::macros::swap_field_command;

// The exact single-field swap: replace the folder's display name, keep the prior name
// for undo. The macro's `new` takes `impl Into<String>`, so the `&str`-accepting
// signature this command had is preserved.
swap_field_command!(
    /// Renames a folder. Undo restores the prior name.
    RenameCodexFolder,
    ctor: into,
    id: CodexFolderId,
    value: String,
    accessor: folder_mut,
    not_found: CommandError::CodexFolderNotFound,
    field: name,
    label: "command.codex.rename_folder",
    held_size: String::len,
);

#[cfg(test)]
mod tests {
    use super::*;
    use crate::command::Command;
    use crate::commands::CreateCodexFolder;
    use crate::document::Document;

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
