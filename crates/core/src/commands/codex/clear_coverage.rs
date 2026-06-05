//! [`ClearCoverage`]: drop every coverage cell for one entry, reversibly.
//!
//! Undo restores the exact set of `(slot, status)` cells that were removed. The entry
//! itself is left untouched; this only clears its coverage state.

use crate::codex::{CodexEntryId, CoverageItemStatus};
use crate::command::{Command, CommandError};
use crate::document::Document;

/// Removes all coverage cells belonging to one entry. Undo restores them.
pub struct ClearCoverage {
    entry: CodexEntryId,
    /// The `(slot, status)` cells removed on apply, captured for undo.
    removed: Option<Vec<(String, CoverageItemStatus)>>,
}

impl ClearCoverage {
    /// A command that will clear all coverage for the entry `id`.
    pub fn new(id: CodexEntryId) -> Self {
        Self { entry: id, removed: None }
    }
}

impl Command for ClearCoverage {
    fn apply(&mut self, doc: &mut Document) -> Result<(), CommandError> {
        let codex = doc.codex_mut();
        if codex.entry(self.entry).is_none() {
            return Err(CommandError::CodexEntryNotFound(self.entry));
        }
        let keys: Vec<_> = codex.coverage_state.keys().filter(|k| k.entry == self.entry).cloned().collect();
        let mut removed = Vec::with_capacity(keys.len());
        for key in keys {
            if let Some(status) = codex.coverage_state.remove(&key) {
                removed.push((key.slot, status));
            }
        }
        self.removed = Some(removed);
        doc.bump_revision();
        Ok(())
    }

    fn undo(&mut self, doc: &mut Document) -> Result<(), CommandError> {
        let removed = self.removed.take().ok_or(CommandError::InvalidState)?;
        let codex = doc.codex_mut();
        for (slot, status) in removed {
            codex.coverage_state.insert(crate::codex::CoverageKey::new(self.entry, slot), status);
        }
        doc.bump_revision();
        Ok(())
    }

    fn label_key(&self) -> &'static str {
        "command.codex.clear_coverage"
    }

    fn estimated_size_bytes(&self) -> usize {
        std::mem::size_of::<Self>() + self.removed.as_ref().map_or(0, |cells| cells.iter().map(|(slot, _)| slot.len()).sum::<usize>())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::codex::{CodexHandle, EntryType};
    use crate::commands::{AddCodexEntry, CodexEntryProto, SetCoverageStatus};

    fn seed(doc: &mut Document) -> CodexEntryId {
        let mut add = AddCodexEntry::new(CodexEntryProto {
            handle: CodexHandle::new("bit").unwrap(),
            name: "Bit".to_owned(),
            entry_type: EntryType::Character,
        });
        add.apply(doc).unwrap();
        add.inserted_id().unwrap()
    }

    #[test]
    fn apply_clears_then_undo_restores() {
        let mut doc = Document::new();
        let id = seed(&mut doc);
        SetCoverageStatus::new(id, "idle", CoverageItemStatus::Approved).apply(&mut doc).unwrap();
        SetCoverageStatus::new(id, "walk", CoverageItemStatus::Draft).apply(&mut doc).unwrap();
        let mut cmd = ClearCoverage::new(id);

        cmd.apply(&mut doc).unwrap();
        assert_eq!(doc.codex().coverage_status(id, "idle"), CoverageItemStatus::Missing);
        assert_eq!(doc.codex().coverage_state().len(), 0);

        cmd.undo(&mut doc).unwrap();
        assert_eq!(doc.codex().coverage_status(id, "idle"), CoverageItemStatus::Approved);
        assert_eq!(doc.codex().coverage_status(id, "walk"), CoverageItemStatus::Draft);

        // Redo: a second apply clears the cells back to Missing again.
        cmd.apply(&mut doc).unwrap();
        assert_eq!(doc.codex().coverage_status(id, "idle"), CoverageItemStatus::Missing);
        assert_eq!(doc.codex().coverage_state().len(), 0);
    }

    #[test]
    fn clears_only_the_target_entry() {
        let mut doc = Document::new();
        let id = seed(&mut doc);
        let mut other = AddCodexEntry::new(CodexEntryProto {
            handle: CodexHandle::new("mossy").unwrap(),
            name: "Mossy".to_owned(),
            entry_type: EntryType::Material,
        });
        other.apply(&mut doc).unwrap();
        let other_id = other.inserted_id().unwrap();
        SetCoverageStatus::new(id, "idle", CoverageItemStatus::Approved).apply(&mut doc).unwrap();
        SetCoverageStatus::new(other_id, "idle", CoverageItemStatus::Draft).apply(&mut doc).unwrap();

        ClearCoverage::new(id).apply(&mut doc).unwrap();
        assert_eq!(doc.codex().coverage_status(other_id, "idle"), CoverageItemStatus::Draft);
    }

    #[test]
    fn missing_entry_errors() {
        let mut doc = Document::new();
        let mut cmd = ClearCoverage::new(CodexEntryId(9));
        assert!(matches!(cmd.apply(&mut doc), Err(CommandError::CodexEntryNotFound(_))));
    }
}
