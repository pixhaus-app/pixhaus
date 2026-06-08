//! Folder-aware read queries over a [`Codex`] (bible 11.1, 12.3).
//!
//! `core` owns the flat folder storage and the commands that mutate it, plus the
//! primitive accessors ([`Codex::child_folders`], [`Codex::entries_in_folder`],
//! [`Codex::folder_is_descendant`]). This module composes those primitives into the
//! read-side shapes the Navigator needs: a recursive folder tree, the entries within
//! a folder (optionally filtered by a search query), and a cycle guard that pre-checks
//! a proposed reparent so the UI can disable an illegal move before dispatching the
//! command.
//!
//! Every function here is a pure read over `&Codex`: it mints nothing and mutates
//! nothing.

use std::collections::HashSet;

use pixhaus_core::{Codex, CodexEntryId, CodexFolderId};

use crate::codex::search::{SearchContext, suggest};

/// The number of ranked entries [`search_in_folder`] draws on before filtering to a
/// folder. Set high enough that a folder filter does not discard a relevant hit that
/// merely fell outside a small autocomplete limit.
const SEARCH_IN_FOLDER_LIMIT: usize = 256;

/// One node in the recursive folder tree: a folder, the entries directly in it, and
/// its child folders (themselves nodes), all in id order.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct FolderNode {
    /// The folder this node represents.
    pub folder: CodexFolderId,
    /// The entries that live directly in this folder, in id order.
    pub entries: Vec<CodexEntryId>,
    /// The child folders, each fully expanded, in id order.
    pub children: Vec<FolderNode>,
}

/// The whole Codex as a tree: the entries and folders sitting at the root (no
/// parent), with every folder expanded recursively.
///
/// The root is not itself a [`FolderNode`] because the codex root has no folder id;
/// it is the pair of "root-level entries" and "root-level folder subtrees".
#[derive(Clone, PartialEq, Eq, Debug, Default)]
pub struct FolderTree {
    /// The entries that sit at the codex root (no folder), in id order.
    pub root_entries: Vec<CodexEntryId>,
    /// The folder subtrees that sit at the codex root, in id order.
    pub roots: Vec<FolderNode>,
}

/// Builds the full recursive folder tree for `codex`.
///
/// Root-level entries and folders land in [`FolderTree`]; each folder is expanded
/// into a [`FolderNode`] carrying its own entries and child subtrees. Iteration order
/// is id order at every level, matching [`Codex::child_folders`] and
/// [`Codex::entries_in_folder`]. The storage is acyclic by construction (the reparent
/// command rejects cycles), so the recursion terminates.
#[tracing::instrument(level = "debug", skip(codex), fields(folders = codex.folders().len()))]
pub fn folder_tree(codex: &Codex) -> FolderTree {
    FolderTree {
        root_entries: codex.entries_in_folder(None),
        roots: codex.child_folders(None).into_iter().map(|id| build_node(codex, id)).collect(),
    }
}

/// Expands one folder id into a [`FolderNode`], recursing into its children.
fn build_node(codex: &Codex, folder: CodexFolderId) -> FolderNode {
    FolderNode {
        folder,
        entries: codex.entries_in_folder(Some(folder)),
        children: codex.child_folders(Some(folder)).into_iter().map(|id| build_node(codex, id)).collect(),
    }
}

/// The entries directly inside `folder` (the codex root when `None`), in id order.
///
/// A thin pass-through over [`Codex::entries_in_folder`], named on the services side
/// so the Navigator reaches for one read surface. It does not recurse into child
/// folders; use [`folder_tree`] for the nested view.
pub fn entries_in_folder(codex: &Codex, folder: Option<CodexFolderId>) -> Vec<CodexEntryId> {
    codex.entries_in_folder(folder)
}

/// The entries inside `folder` whose handle, name, aliases, or tags match `query`,
/// ranked best-first by the same fuzzy matcher `@`-autocomplete uses.
///
/// An empty or whitespace-only `query` returns every entry in the folder, in id
/// order, so the caller can use one path for "show all in this folder" and "search
/// within this folder". A non-empty query ranks the folder's entries with
/// [`suggest`] and drops non-matches. Like [`entries_in_folder`], this does not
/// recurse into child folders.
#[tracing::instrument(level = "debug", skip(codex, query), fields(folder = ?folder))]
pub fn search_in_folder(codex: &Codex, folder: Option<CodexFolderId>, query: &str) -> Vec<CodexEntryId> {
    let in_folder = codex.entries_in_folder(folder);
    if query.trim().is_empty() {
        return in_folder;
    }
    // Rank across the whole codex, then keep only the hits that live in this folder,
    // preserving the ranked order.
    let ctx = SearchContext::default();
    let ranked = suggest(codex, query, None, &ctx, SEARCH_IN_FOLDER_LIMIT);
    // Membership-test the ranked hits against a set, not the Vec, so the filter is O(ranked)
    // rather than O(ranked * folder_size). Shadowing keeps the empty-query early return above
    // returning the ordered Vec.
    let in_folder: HashSet<CodexEntryId> = in_folder.into_iter().collect();
    ranked.into_iter().map(|s| s.entry).filter(|id| in_folder.contains(id)).collect()
}

/// Whether moving `folder` under `new_parent` would create a cycle — the UI-side
/// mirror of the `SetCodexFolderParent` rejection so a move can be disabled before
/// it is dispatched.
///
/// A move is a cycle when `new_parent` is `folder` itself or any descendant of
/// `folder`: reparenting onto your own subtree detaches it from the root. Moving to
/// the codex root (`None`) is never a cycle. A `new_parent` that does not exist is
/// not a cycle here — that is a separate `CodexFolderNotFound` rejection the command
/// reports; this guard answers only the cycle question.
pub fn reparent_creates_cycle(codex: &Codex, folder: CodexFolderId, new_parent: Option<CodexFolderId>) -> bool {
    match new_parent {
        None => false,
        Some(parent) => codex.folder_is_descendant(parent, folder),
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::disallowed_methods, clippy::panic)]
mod tests {
    use super::*;
    use pixhaus_core::{AddCodexEntry, CodexEntryProto, CodexHandle, Command, CreateCodexFolder, Document, EntryType, SetCodexEntryFolder};

    fn handle(s: &str) -> CodexHandle {
        CodexHandle::new(s).unwrap()
    }

    fn add(doc: &mut Document, h: &str, entry_type: EntryType) -> CodexEntryId {
        let proto = CodexEntryProto {
            handle: handle(h),
            name: h.to_owned(),
            entry_type,
        };
        let mut cmd = AddCodexEntry::new(proto);
        cmd.apply(doc).unwrap();
        cmd.inserted_id().unwrap()
    }

    fn folder(doc: &mut Document, parent: Option<CodexFolderId>, name: &str) -> CodexFolderId {
        let mut cmd = CreateCodexFolder::new(parent, name);
        cmd.apply(doc).unwrap();
        cmd.inserted_id().unwrap()
    }

    fn move_entry(doc: &mut Document, entry: CodexEntryId, into: Option<CodexFolderId>) {
        let mut cmd = SetCodexEntryFolder::new(entry, into);
        cmd.apply(doc).unwrap();
    }

    #[test]
    fn empty_codex_has_empty_tree() {
        let doc = Document::new();
        let tree = folder_tree(doc.codex());
        assert!(tree.root_entries.is_empty());
        assert!(tree.roots.is_empty());
    }

    #[test]
    fn tree_nests_folders_and_places_entries() {
        let mut doc = Document::new();
        // heroes/ with a child bosses/; bit lives in heroes, dragon in bosses, mossy
        // at the root.
        let heroes = folder(&mut doc, None, "Heroes");
        let bosses = folder(&mut doc, Some(heroes), "Bosses");
        let bit = add(&mut doc, "bit", EntryType::Character);
        let dragon = add(&mut doc, "dragon", EntryType::Enemy);
        let mossy = add(&mut doc, "mossy", EntryType::Material);
        move_entry(&mut doc, bit, Some(heroes));
        move_entry(&mut doc, dragon, Some(bosses));

        let tree = folder_tree(doc.codex());
        assert_eq!(tree.root_entries, vec![mossy]);
        assert_eq!(tree.roots.len(), 1);

        let heroes_node = &tree.roots[0];
        assert_eq!(heroes_node.folder, heroes);
        assert_eq!(heroes_node.entries, vec![bit]);
        assert_eq!(heroes_node.children.len(), 1);

        let bosses_node = &heroes_node.children[0];
        assert_eq!(bosses_node.folder, bosses);
        assert_eq!(bosses_node.entries, vec![dragon]);
        assert!(bosses_node.children.is_empty());
    }

    #[test]
    fn entries_in_folder_passes_through() {
        let mut doc = Document::new();
        let heroes = folder(&mut doc, None, "Heroes");
        let bit = add(&mut doc, "bit", EntryType::Character);
        move_entry(&mut doc, bit, Some(heroes));
        assert_eq!(entries_in_folder(doc.codex(), Some(heroes)), vec![bit]);
        assert!(entries_in_folder(doc.codex(), None).is_empty());
    }

    #[test]
    fn search_in_folder_empty_query_returns_all_in_folder() {
        let mut doc = Document::new();
        let heroes = folder(&mut doc, None, "Heroes");
        let bit = add(&mut doc, "bit", EntryType::Character);
        let bolt = add(&mut doc, "bolt", EntryType::Character);
        move_entry(&mut doc, bit, Some(heroes));
        move_entry(&mut doc, bolt, Some(heroes));
        assert_eq!(search_in_folder(doc.codex(), Some(heroes), "   "), vec![bit, bolt]);
    }

    #[test]
    fn search_in_folder_filters_by_query_and_folder() {
        let mut doc = Document::new();
        let heroes = folder(&mut doc, None, "Heroes");
        let bit = add(&mut doc, "bit", EntryType::Character);
        let bolt = add(&mut doc, "bolt", EntryType::Character);
        // An entry that matches the query but lives at the root must not appear.
        let bitter = add(&mut doc, "bitter", EntryType::Material);
        move_entry(&mut doc, bit, Some(heroes));
        move_entry(&mut doc, bolt, Some(heroes));

        let hits = search_in_folder(doc.codex(), Some(heroes), "bit");
        assert!(hits.contains(&bit));
        assert!(!hits.contains(&bitter));
        assert!(!hits.contains(&bolt));
    }

    #[test]
    fn reparent_onto_self_is_a_cycle() {
        let mut doc = Document::new();
        let a = folder(&mut doc, None, "A");
        assert!(reparent_creates_cycle(doc.codex(), a, Some(a)));
    }

    #[test]
    fn reparent_onto_descendant_is_a_cycle() {
        let mut doc = Document::new();
        let a = folder(&mut doc, None, "A");
        let b = folder(&mut doc, Some(a), "B");
        // Moving A under its own child B would detach the subtree.
        assert!(reparent_creates_cycle(doc.codex(), a, Some(b)));
    }

    #[test]
    fn reparent_to_root_or_sibling_is_allowed() {
        let mut doc = Document::new();
        let a = folder(&mut doc, None, "A");
        let b = folder(&mut doc, Some(a), "B");
        let c = folder(&mut doc, None, "C");
        // To the root: fine. Under an unrelated folder: fine.
        assert!(!reparent_creates_cycle(doc.codex(), b, None));
        assert!(!reparent_creates_cycle(doc.codex(), b, Some(c)));
    }
}
