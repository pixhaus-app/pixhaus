//! [`SetCoverageStatus`]: set one coverage cell's status, reversibly.

use crate::codex::{CodexEntryId, CoverageItemStatus, CoverageKey};
use crate::command::{Command, CommandError};
use crate::document::Document;

/// What the target cell held before apply, captured so undo restores it.
enum PriorCoverage {
    /// Apply has not run yet.
    Unset,
    /// The cell did not exist before apply.
    WasAbsent,
    /// The cell held this status before apply.
    WasPresent(CoverageItemStatus),
}

/// Sets the coverage status for one `(entry, slot)` cell; undo restores the prior
/// status, or removes the cell if there was none. The entry must exist.
pub struct SetCoverageStatus {
    entry: CodexEntryId,
    slot: String,
    status: CoverageItemStatus,
    /// What the target cell held before apply.
    prev: PriorCoverage,
}

impl SetCoverageStatus {
    /// A command that will set the `(id, slot)` coverage cell to `status`.
    pub fn new(id: CodexEntryId, slot: impl Into<String>, status: CoverageItemStatus) -> Self {
        Self {
            entry: id,
            slot: slot.into(),
            status,
            prev: PriorCoverage::Unset,
        }
    }
}

impl Command for SetCoverageStatus {
    fn apply(&mut self, doc: &mut Document) -> Result<(), CommandError> {
        let codex = doc.codex_mut();
        if codex.entry(self.entry).is_none() {
            return Err(CommandError::CodexEntryNotFound(self.entry));
        }
        let key = CoverageKey::new(self.entry, self.slot.clone());
        self.prev = match codex.coverage_state.insert(key, self.status) {
            Some(status) => PriorCoverage::WasPresent(status),
            None => PriorCoverage::WasAbsent,
        };
        doc.bump_revision();
        Ok(())
    }

    fn undo(&mut self, doc: &mut Document) -> Result<(), CommandError> {
        let prev = std::mem::replace(&mut self.prev, PriorCoverage::Unset);
        let codex = doc.codex_mut();
        let key = CoverageKey::new(self.entry, self.slot.clone());
        match prev {
            PriorCoverage::WasPresent(status) => {
                codex.coverage_state.insert(key, status);
            }
            PriorCoverage::WasAbsent => {
                codex.coverage_state.remove(&key);
            }
            PriorCoverage::Unset => return Err(CommandError::InvalidState),
        }
        doc.bump_revision();
        Ok(())
    }

    fn label_key(&self) -> &'static str {
        "command.codex.set_coverage_status"
    }

    fn estimated_size_bytes(&self) -> usize {
        std::mem::size_of::<Self>() + self.slot.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::codex::{CodexHandle, EntryType};
    use crate::commands::{AddCodexEntry, CodexEntryProto};

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
    fn apply_sets_then_undo_removes_when_no_prior() {
        let mut doc = Document::new();
        let id = seed(&mut doc);
        let mut cmd = SetCoverageStatus::new(id, "idle", CoverageItemStatus::Approved);

        cmd.apply(&mut doc).unwrap();
        assert_eq!(doc.codex().coverage_status(id, "idle"), CoverageItemStatus::Approved);

        cmd.undo(&mut doc).unwrap();
        assert_eq!(doc.codex().coverage_status(id, "idle"), CoverageItemStatus::Missing);
        assert_eq!(doc.codex().coverage_state().len(), 0);
    }

    #[test]
    fn undo_restores_prior_status() {
        let mut doc = Document::new();
        let id = seed(&mut doc);
        SetCoverageStatus::new(id, "walk", CoverageItemStatus::Draft).apply(&mut doc).unwrap();
        let mut cmd = SetCoverageStatus::new(id, "walk", CoverageItemStatus::Approved);

        cmd.apply(&mut doc).unwrap();
        assert_eq!(doc.codex().coverage_status(id, "walk"), CoverageItemStatus::Approved);

        cmd.undo(&mut doc).unwrap();
        assert_eq!(doc.codex().coverage_status(id, "walk"), CoverageItemStatus::Draft);
    }

    #[test]
    fn missing_entry_errors() {
        let mut doc = Document::new();
        let mut cmd = SetCoverageStatus::new(CodexEntryId(4), "idle", CoverageItemStatus::Draft);
        assert!(matches!(cmd.apply(&mut doc), Err(CommandError::CodexEntryNotFound(_))));
    }
}
