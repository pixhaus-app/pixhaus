//! [`DuplicateCodexEntry`]: deep-copy an entry under a fresh id and unique handle.

use crate::codex::{CodexEntryId, CodexHandle, EntryStatus};
use crate::command::{Command, CommandError};
use crate::document::Document;

/// Deep-copies an existing Codex entry. The clone keeps the source's name, type,
/// folder, description, tags, fragments, anchors, and body, but takes a freshly-minted
/// id, a unique handle derived from the source (`<handle>_copy`, then `_copy_2`, …),
/// an empty version history, and [`EntryStatus::Draft`] status — a duplicate is a new
/// draft, not a promoted entry. Relationships and coverage that reference the source
/// are not copied; the clone starts with neither.
///
/// Undo removes the inserted clone. Apply is faithful across an undo/redo cycle: each
/// re-apply re-derives a unique handle against the current codex.
pub struct DuplicateCodexEntry {
    source: CodexEntryId,
    inserted: Option<CodexEntryId>,
}

impl DuplicateCodexEntry {
    /// A command that will duplicate the entry `source`.
    pub fn new(source: CodexEntryId) -> Self {
        Self { source, inserted: None }
    }

    /// The id assigned to the inserted clone, available after [`apply`](Command::apply).
    pub fn inserted_id(&self) -> Option<CodexEntryId> {
        self.inserted
    }

    /// Mints a handle not yet claimed in `doc`, derived from `base`. Tries
    /// `<base>_copy`, then `<base>_copy_2`, `<base>_copy_3`, … until one is free.
    /// Falls back to the numbered form if `<base>_copy` is somehow not a valid handle.
    fn unique_copy_handle(doc: &Document, base: &CodexHandle) -> Result<CodexHandle, CommandError> {
        let stem = base.as_str();
        let first = format!("{stem}_copy");
        if let Ok(handle) = CodexHandle::new(first.clone())
            && !doc.codex().handle_in_use(&handle)
        {
            return Ok(handle);
        }
        // `_copy` was taken (or invalid); walk the numbered suffixes.
        for n in 2..=u32::MAX {
            let candidate = format!("{stem}_copy_{n}");
            let Ok(handle) = CodexHandle::new(candidate) else {
                continue;
            };
            if !doc.codex().handle_in_use(&handle) {
                return Ok(handle);
            }
        }
        Err(CommandError::CodexHandleInUse)
    }
}

impl Command for DuplicateCodexEntry {
    fn apply(&mut self, doc: &mut Document) -> Result<(), CommandError> {
        if self.inserted.is_some() {
            return Err(CommandError::InvalidState);
        }
        let source = doc.codex().entry(self.source).ok_or(CommandError::CodexEntryNotFound(self.source))?.clone();
        let handle = Self::unique_copy_handle(doc, &source.handle)?;

        let codex = doc.codex_mut();
        let id = codex.mint_entry_id();
        let mut clone = source;
        clone.id = id;
        clone.handle = handle;
        // A duplicate is a fresh draft: it carries no aliases, no history, and is not
        // promoted, so it can never collide with the source's canonical identity.
        clone.aliases.clear();
        clone.version_history.clear();
        clone.status = EntryStatus::Draft;
        codex.insert_entry(clone);
        doc.bump_revision();
        self.inserted = Some(id);
        Ok(())
    }

    fn undo(&mut self, doc: &mut Document) -> Result<(), CommandError> {
        let id = self.inserted.take().ok_or(CommandError::InvalidState)?;
        doc.codex_mut().remove_entry(id).ok_or(CommandError::CodexEntryNotFound(id))?;
        doc.bump_revision();
        Ok(())
    }

    fn label_key(&self) -> &'static str {
        "command.codex.duplicate_entry"
    }

    fn estimated_size_bytes(&self) -> usize {
        std::mem::size_of::<Self>()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::codex::EntryType;
    use crate::codex::details::{GenericDetails, GenericField};
    use crate::commands::{SetEntryStatus, SetGenericDetails};
    use crate::test_support::seed_entry;

    // Local wrapper: these tests fix the name to "Bit" and vary only the type, so
    // they pin that name into the shared four-field builder.
    fn seed(doc: &mut Document, handle: &str, ty: EntryType) -> CodexEntryId {
        seed_entry(doc, handle, "Bit", ty)
    }

    #[test]
    fn apply_clones_under_fresh_id_and_handle_then_undo_removes() {
        let mut doc = Document::new();
        let src = seed(&mut doc, "bit", EntryType::Character);
        let mut cmd = DuplicateCodexEntry::new(src);

        cmd.apply(&mut doc).unwrap();
        let clone_id = cmd.inserted_id().unwrap();
        assert_ne!(clone_id, src);
        assert_eq!(doc.codex().entries().len(), 2);
        let clone = doc.codex().entry(clone_id).unwrap();
        assert_eq!(clone.handle.as_str(), "bit_copy");
        assert_eq!(clone.name, "Bit");
        assert_eq!(clone.entry_type, EntryType::Character);

        cmd.undo(&mut doc).unwrap();
        assert_eq!(doc.codex().entries().len(), 1);
        assert!(doc.codex().entry(clone_id).is_none());
    }

    #[test]
    fn clone_is_a_fresh_draft_without_aliases_or_history() {
        let mut doc = Document::new();
        let src = seed(&mut doc, "bit", EntryType::Character);
        // Promote the source and give it an alias + history so we can prove the clone drops them.
        SetEntryStatus::new(src, EntryStatus::Canonical).apply(&mut doc).unwrap();
        {
            let entry = doc.codex_mut().entry_mut(src).unwrap();
            entry.aliases.push(CodexHandle::new("mascot").unwrap());
            entry.version_history.push(crate::codex::EntryVersion {
                version: 1,
                timestamp_ms: 10,
                author: "luis".to_owned(),
                summary: "created".to_owned(),
            });
        }

        let mut cmd = DuplicateCodexEntry::new(src);
        cmd.apply(&mut doc).unwrap();
        let clone = doc.codex().entry(cmd.inserted_id().unwrap()).unwrap();
        assert_eq!(clone.status, EntryStatus::Draft);
        assert!(clone.aliases.is_empty());
        assert!(clone.version_history.is_empty());
    }

    #[test]
    fn clone_deep_copies_the_body() {
        let mut doc = Document::new();
        let src = seed(&mut doc, "potion", EntryType::Item);
        SetGenericDetails::new(
            src,
            GenericDetails {
                fields: vec![GenericField {
                    key: "rarity".to_owned(),
                    value: "rare".to_owned(),
                }],
            },
        )
        .apply(&mut doc)
        .unwrap();

        let mut cmd = DuplicateCodexEntry::new(src);
        cmd.apply(&mut doc).unwrap();
        let clone = doc.codex().entry(cmd.inserted_id().unwrap()).unwrap();
        match &clone.details {
            crate::codex::EntryDetails::Generic(g) => {
                assert_eq!(g.fields.len(), 1);
                assert_eq!(g.fields[0].value, "rare");
            }
            other => panic!("expected Generic body, got {other:?}"),
        }
    }

    #[test]
    fn second_duplicate_takes_a_numbered_handle() {
        let mut doc = Document::new();
        let src = seed(&mut doc, "bit", EntryType::Character);
        DuplicateCodexEntry::new(src).apply(&mut doc).unwrap();
        let mut second = DuplicateCodexEntry::new(src);
        second.apply(&mut doc).unwrap();
        let handle = doc.codex().entry(second.inserted_id().unwrap()).unwrap().handle.as_str().to_owned();
        assert_eq!(handle, "bit_copy_2");
        assert_eq!(doc.codex().entries().len(), 3);
    }

    #[test]
    fn redo_after_undo_re_adds_with_a_unique_handle() {
        let mut doc = Document::new();
        let src = seed(&mut doc, "bit", EntryType::Character);
        let mut cmd = DuplicateCodexEntry::new(src);
        cmd.apply(&mut doc).unwrap();
        cmd.undo(&mut doc).unwrap();
        cmd.apply(&mut doc).unwrap();
        assert_eq!(doc.codex().entries().len(), 2);
        // The freed `bit_copy` is available again after undo.
        assert_eq!(doc.codex().entry(cmd.inserted_id().unwrap()).unwrap().handle.as_str(), "bit_copy");
    }

    #[test]
    fn missing_source_errors() {
        let mut doc = Document::new();
        let mut cmd = DuplicateCodexEntry::new(CodexEntryId(99));
        assert!(matches!(cmd.apply(&mut doc), Err(CommandError::CodexEntryNotFound(_))));
    }

    #[test]
    fn undo_before_apply_is_invalid_state() {
        let mut doc = Document::new();
        let mut cmd = DuplicateCodexEntry::new(CodexEntryId(1));
        assert!(matches!(cmd.undo(&mut doc), Err(CommandError::InvalidState)));
    }
}
