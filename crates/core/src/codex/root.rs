//! [`Codex`]: a project's one creative bible (bible 11.1).
//!
//! The Codex owns the entries, the typed relationships between them, the coverage
//! templates, and the per-entry/per-slot coverage state. Entries live in a
//! [`BTreeMap`] keyed by [`CodexEntryId`] for stable iteration order. There is no
//! search index here — that is a services-side cache (bible 12.3). Read accessors are
//! public; mutation is `pub(crate)` and flows through the codex commands.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::codex::coverage::{CoverageItemStatus, CoverageTemplate, CoverageTemplateId};
use crate::codex::entry::CodexEntry;
use crate::codex::folder::{CodexFolder, CodexFolderId};
use crate::codex::handle::CodexHandle;
use crate::codex::ids::CodexEntryId;
use crate::codex::relationship::Relationship;
use crate::ids::IdCounter;

/// The key for one coverage cell: an entry and a slot key.
#[derive(Clone, PartialEq, Eq, Hash, Debug, PartialOrd, Ord, Serialize, Deserialize)]
pub struct CoverageKey {
    /// The entry the coverage cell belongs to.
    pub entry: CodexEntryId,
    /// The slot key within that entry's coverage.
    pub slot: String,
}

impl CoverageKey {
    /// A coverage key for `entry` and `slot`.
    pub fn new(entry: CodexEntryId, slot: impl Into<String>) -> Self {
        Self { entry, slot: slot.into() }
    }
}

/// A project's one creative bible.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct Codex {
    pub(crate) entries: BTreeMap<CodexEntryId, CodexEntry>,
    pub(crate) id_counter: IdCounter,
    pub(crate) relationships: Vec<Relationship>,
    #[serde(default)]
    pub(crate) coverage_templates: BTreeMap<CoverageTemplateId, CoverageTemplate>,
    #[serde(default)]
    pub(crate) coverage_template_counter: IdCounter,
    pub(crate) coverage_state: BTreeMap<CoverageKey, CoverageItemStatus>,
    #[serde(default)]
    pub(crate) folders: BTreeMap<CodexFolderId, CodexFolder>,
    #[serde(default)]
    pub(crate) folder_counter: IdCounter,
}

impl Codex {
    /// An empty Codex.
    pub fn new() -> Self {
        Self::default()
    }

    /// The entries, keyed by id, in id order.
    pub fn entries(&self) -> &BTreeMap<CodexEntryId, CodexEntry> {
        &self.entries
    }

    /// Borrows the entry for `id`, or `None` if absent.
    pub fn entry(&self, id: CodexEntryId) -> Option<&CodexEntry> {
        self.entries.get(&id)
    }

    /// Resolves a handle (matching either the primary handle or an alias) to an entry
    /// id (bible 6.5). Returns the first match in id order, or `None`.
    pub fn resolve_handle(&self, handle: &CodexHandle) -> Option<CodexEntryId> {
        self.entries
            .iter()
            .find(|(_, e)| &e.handle == handle || e.aliases.contains(handle))
            .map(|(id, _)| *id)
    }

    /// Whether any entry already claims `handle` as its primary handle or an alias.
    pub fn handle_in_use(&self, handle: &CodexHandle) -> bool {
        self.resolve_handle(handle).is_some()
    }

    /// The typed relationships, in insertion order.
    pub fn relationships(&self) -> &[Relationship] {
        &self.relationships
    }

    /// The coverage templates the project has defined, in id order.
    pub fn coverage_templates(&self) -> impl Iterator<Item = &CoverageTemplate> {
        self.coverage_templates.values()
    }

    /// Borrows the coverage template for `id`, or `None` if absent.
    pub fn coverage_template(&self, id: CoverageTemplateId) -> Option<&CoverageTemplate> {
        self.coverage_templates.get(&id)
    }

    /// The coverage status for `entry` and `slot`, defaulting to
    /// [`CoverageItemStatus::Missing`] when unset.
    pub fn coverage_status(&self, entry: CodexEntryId, slot: &str) -> CoverageItemStatus {
        self.coverage_state.get(&CoverageKey::new(entry, slot)).copied().unwrap_or_default()
    }

    /// The full coverage state map.
    pub fn coverage_state(&self) -> &BTreeMap<CoverageKey, CoverageItemStatus> {
        &self.coverage_state
    }

    /// The folders, keyed by id, in id order.
    pub fn folders(&self) -> &BTreeMap<CodexFolderId, CodexFolder> {
        &self.folders
    }

    /// Borrows the folder for `id`, or `None` if absent.
    pub fn folder(&self, id: CodexFolderId) -> Option<&CodexFolder> {
        self.folders.get(&id)
    }

    /// The ids of the folders directly under `parent` (the root when `None`), in id
    /// order.
    pub fn child_folders(&self, parent: Option<CodexFolderId>) -> Vec<CodexFolderId> {
        self.folders.values().filter(|f| f.parent == parent).map(|f| f.id).collect()
    }

    /// The ids of the entries directly inside `folder` (the root when `None`), in id
    /// order.
    pub fn entries_in_folder(&self, folder: Option<CodexFolderId>) -> Vec<CodexEntryId> {
        self.entries.values().filter(|e| e.folder_id == folder).map(|e| e.id).collect()
    }

    /// Whether `candidate` is `folder` or one of its descendants — the cycle guard a
    /// reparent must respect. Returns `false` when `candidate` is unknown.
    pub fn folder_is_descendant(&self, candidate: CodexFolderId, folder: CodexFolderId) -> bool {
        let mut cursor = Some(candidate);
        while let Some(id) = cursor {
            if id == folder {
                return true;
            }
            cursor = self.folders.get(&id).and_then(|f| f.parent);
        }
        false
    }

    // --- command-only mutators (in-crate; mutation flows through a Command) ---

    pub(crate) fn mint_entry_id(&mut self) -> CodexEntryId {
        CodexEntryId(self.id_counter.mint())
    }

    pub(crate) fn entry_mut(&mut self, id: CodexEntryId) -> Option<&mut CodexEntry> {
        self.entries.get_mut(&id)
    }

    pub(crate) fn insert_entry(&mut self, entry: CodexEntry) {
        self.entries.insert(entry.id, entry);
    }

    pub(crate) fn remove_entry(&mut self, id: CodexEntryId) -> Option<CodexEntry> {
        self.entries.remove(&id)
    }

    pub(crate) fn mint_folder_id(&mut self) -> CodexFolderId {
        CodexFolderId(self.folder_counter.mint())
    }

    pub(crate) fn folder_mut(&mut self, id: CodexFolderId) -> Option<&mut CodexFolder> {
        self.folders.get_mut(&id)
    }

    pub(crate) fn insert_folder(&mut self, folder: CodexFolder) {
        self.folders.insert(folder.id, folder);
    }

    pub(crate) fn remove_folder(&mut self, id: CodexFolderId) -> Option<CodexFolder> {
        self.folders.remove(&id)
    }

    pub(crate) fn mint_coverage_template_id(&mut self) -> CoverageTemplateId {
        CoverageTemplateId(self.coverage_template_counter.mint())
    }

    pub(crate) fn coverage_template_mut(&mut self, id: CoverageTemplateId) -> Option<&mut CoverageTemplate> {
        self.coverage_templates.get_mut(&id)
    }

    pub(crate) fn insert_coverage_template(&mut self, template: CoverageTemplate) {
        self.coverage_templates.insert(template.id, template);
    }

    pub(crate) fn remove_coverage_template(&mut self, id: CoverageTemplateId) -> Option<CoverageTemplate> {
        self.coverage_templates.remove(&id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::codex::{CodexEntry, EntryType};

    fn handle(s: &str) -> CodexHandle {
        CodexHandle::new(s).unwrap()
    }

    #[test]
    fn empty_codex_resolves_nothing() {
        let c = Codex::new();
        assert!(c.entries().is_empty());
        assert_eq!(c.resolve_handle(&handle("bit")), None);
        assert!(!c.handle_in_use(&handle("bit")));
    }

    #[test]
    fn insert_then_resolve_by_handle_and_alias() {
        let mut c = Codex::new();
        let id = c.mint_entry_id();
        let mut e = CodexEntry::new(id, handle("bit"), "Bit", EntryType::Character);
        e.aliases.push(handle("mascot"));
        c.insert_entry(e);
        assert_eq!(c.resolve_handle(&handle("bit")), Some(id));
        assert_eq!(c.resolve_handle(&handle("mascot")), Some(id));
        assert!(c.handle_in_use(&handle("bit")));
    }

    #[test]
    fn coverage_status_defaults_missing() {
        let c = Codex::new();
        assert_eq!(c.coverage_status(CodexEntryId(1), "idle"), CoverageItemStatus::Missing);
    }

    #[test]
    fn folder_accessors_track_inserts() {
        let mut c = Codex::new();
        let root = CodexFolderId(c.mint_folder_id().0);
        c.insert_folder(CodexFolder::new(root, "Heroes", None));
        let child = c.mint_folder_id();
        c.insert_folder(CodexFolder::new(child, "Bosses", Some(root)));

        assert_eq!(c.folders().len(), 2);
        assert_eq!(c.folder(root).unwrap().name, "Heroes");
        assert_eq!(c.child_folders(None), vec![root]);
        assert_eq!(c.child_folders(Some(root)), vec![child]);
        assert!(c.remove_folder(child).is_some());
        assert_eq!(c.folders().len(), 1);
    }

    #[test]
    fn entries_in_folder_filters_by_folder() {
        let mut c = Codex::new();
        let folder = c.mint_folder_id();
        c.insert_folder(CodexFolder::new(folder, "Heroes", None));
        let id = c.mint_entry_id();
        let mut e = CodexEntry::new(id, handle("bit"), "Bit", EntryType::Character);
        e.folder_id = Some(folder);
        c.insert_entry(e);
        let rooted = c.mint_entry_id();
        c.insert_entry(CodexEntry::new(rooted, handle("mossy"), "Mossy", EntryType::Material));

        assert_eq!(c.entries_in_folder(Some(folder)), vec![id]);
        assert_eq!(c.entries_in_folder(None), vec![rooted]);
    }

    #[test]
    fn coverage_template_accessors_track_inserts() {
        use crate::codex::coverage::CoverageTemplate;
        let mut c = Codex::new();
        let id = c.mint_coverage_template_id();
        let mut t = CoverageTemplate::platformer_character();
        t.id = id;
        c.insert_coverage_template(t);

        assert_eq!(c.coverage_templates().count(), 1);
        assert_eq!(c.coverage_template(id).unwrap().name, "platformer_character");
        c.coverage_template_mut(id).unwrap().name = "renamed".to_owned();
        assert_eq!(c.coverage_template(id).unwrap().name, "renamed");
        assert!(c.remove_coverage_template(id).is_some());
        assert_eq!(c.coverage_templates().count(), 0);
    }

    #[test]
    fn folder_is_descendant_walks_parents() {
        let mut c = Codex::new();
        let a = c.mint_folder_id();
        let b = c.mint_folder_id();
        let d = c.mint_folder_id();
        c.insert_folder(CodexFolder::new(a, "A", None));
        c.insert_folder(CodexFolder::new(b, "B", Some(a)));
        c.insert_folder(CodexFolder::new(d, "D", Some(b)));

        assert!(c.folder_is_descendant(d, a));
        assert!(c.folder_is_descendant(a, a));
        assert!(!c.folder_is_descendant(a, b));
    }
}
