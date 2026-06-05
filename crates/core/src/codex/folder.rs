//! Folders: a way to organize Codex entries into a tree (bible 11.1).
//!
//! A folder groups entries and other folders so a large Codex stays navigable. The
//! tree is flat in storage — every folder knows its parent — and an entry points at
//! the folder it lives in (or `None` for the codex root). Folders carry stable ids
//! and project-content names; they never localize. Mutation flows through the codex
//! folder commands.

use serde::{Deserialize, Serialize};

/// Stable identifier for a [`CodexFolder`] within one
/// [`Codex`](crate::codex::Codex).
///
/// Like every model id, this is a monotonic `u64` newtype minted from an
/// [`IdCounter`](crate::ids::IdCounter) the [`Codex`](crate::codex::Codex) owns.
/// Entries and child folders point at this id, so renaming a folder never breaks the
/// tree.
#[derive(Copy, Clone, PartialEq, Eq, Hash, Debug, PartialOrd, Ord, Serialize, Deserialize)]
pub struct CodexFolderId(pub u64);

/// One folder in the Codex tree: an id, a project-content name, an optional parent,
/// and a description.
///
/// A folder with `parent == None` sits at the codex root. The fields are public so
/// the layers above can read them, but only a command writes them.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct CodexFolder {
    /// Stable id; the tree references this, never the name.
    pub id: CodexFolderId,
    /// Display name (project content; may change without breaking references).
    pub name: String,
    /// The parent folder, or `None` when the folder sits at the codex root.
    pub parent: Option<CodexFolderId>,
    /// Free-text description of what the folder groups (project content).
    pub description: String,
}

impl CodexFolder {
    /// A folder with `id` and `name` under `parent` (or the root when `None`), with an
    /// empty description.
    pub fn new(id: CodexFolderId, name: impl Into<String>, parent: Option<CodexFolderId>) -> Self {
        Self {
            id,
            name: name.into(),
            parent,
            description: String::new(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ids_compare_by_value() {
        assert_eq!(CodexFolderId(3), CodexFolderId(3));
        assert!(CodexFolderId(1) < CodexFolderId(2));
    }

    #[test]
    fn new_folder_defaults_empty_description() {
        let f = CodexFolder::new(CodexFolderId(1), "Heroes", None);
        assert_eq!(f.id, CodexFolderId(1));
        assert_eq!(f.name, "Heroes");
        assert_eq!(f.parent, None);
        assert!(f.description.is_empty());
    }

    #[test]
    fn new_folder_keeps_parent() {
        let f = CodexFolder::new(CodexFolderId(2), "Bosses", Some(CodexFolderId(1)));
        assert_eq!(f.parent, Some(CodexFolderId(1)));
    }
}
