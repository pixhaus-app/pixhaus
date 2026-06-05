//! [`RemoveEntryCustomSlot`]: drop a per-entry ad-hoc coverage slot, reversibly.
//!
//! Removes the custom slot keyed `key` from one entry, restoring it at its prior
//! position on undo. The slot's coverage-state cell is also removed and restored, so the
//! status survives an undo.

use crate::codex::{CodexEntryId, CoverageItemStatus, CoverageKey, CoverageSlot};
use crate::command::{Command, CommandError};
use crate::document::Document;

/// Removes the custom slot keyed `key` from one entry. Undo restores the slot at its
/// prior position and any coverage status it held.
pub struct RemoveEntryCustomSlot {
    entry: CodexEntryId,
    key: String,
    /// The removed slot and its prior index, captured for undo.
    removed_slot: Option<(usize, CoverageSlot)>,
    /// The coverage status the cell held before removal, if any.
    removed_status: Option<CoverageItemStatus>,
}

impl RemoveEntryCustomSlot {
    /// A command that will remove the custom slot keyed `key` from the entry `entry_id`.
    pub fn new(entry_id: CodexEntryId, key: impl Into<String>) -> Self {
        Self {
            entry: entry_id,
            key: key.into(),
            removed_slot: None,
            removed_status: None,
        }
    }
}

impl Command for RemoveEntryCustomSlot {
    fn apply(&mut self, doc: &mut Document) -> Result<(), CommandError> {
        let codex = doc.codex_mut();
        let Some(entry) = codex.entry_mut(self.entry) else {
            return Err(CommandError::CodexEntryNotFound(self.entry));
        };
        let pos = entry.custom_slots.iter().position(|s| s.key == self.key).ok_or(CommandError::InvalidState)?;
        let slot = entry.custom_slots.remove(pos);

        let removed_status = codex.coverage_state.remove(&CoverageKey::new(self.entry, self.key.clone()));

        self.removed_slot = Some((pos, slot));
        self.removed_status = removed_status;
        doc.bump_revision();
        Ok(())
    }

    fn undo(&mut self, doc: &mut Document) -> Result<(), CommandError> {
        let (pos, slot) = self.removed_slot.take().ok_or(CommandError::InvalidState)?;
        let codex = doc.codex_mut();
        if let Some(entry) = codex.entry_mut(self.entry) {
            let pos = pos.min(entry.custom_slots.len());
            entry.custom_slots.insert(pos, slot);
        }
        if let Some(status) = self.removed_status.take() {
            codex.coverage_state.insert(CoverageKey::new(self.entry, self.key.clone()), status);
        }
        doc.bump_revision();
        Ok(())
    }

    fn label_key(&self) -> &'static str {
        "command.codex.remove_entry_custom_slot"
    }

    fn estimated_size_bytes(&self) -> usize {
        std::mem::size_of::<Self>() + self.key.len() + self.removed_slot.as_ref().map_or(0, |(_, s)| s.key.len())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::codex::{CodexHandle, EntryType};
    use crate::commands::{AddCodexEntry, AddEntryCustomSlot, CodexEntryProto, SetCoverageStatus};

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
    fn apply_removes_then_undo_restores_slot_and_status() {
        let mut doc = Document::new();
        let id = seed(&mut doc);
        AddEntryCustomSlot::new(id, CoverageSlot::custom("victory", "Victory")).apply(&mut doc).unwrap();
        SetCoverageStatus::new(id, "victory", CoverageItemStatus::Approved).apply(&mut doc).unwrap();

        let mut cmd = RemoveEntryCustomSlot::new(id, "victory");
        cmd.apply(&mut doc).unwrap();
        assert!(doc.codex().entry(id).unwrap().custom_slots.is_empty());
        assert_eq!(doc.codex().coverage_status(id, "victory"), CoverageItemStatus::Missing);

        cmd.undo(&mut doc).unwrap();
        assert_eq!(doc.codex().entry(id).unwrap().custom_slots.len(), 1);
        assert_eq!(doc.codex().coverage_status(id, "victory"), CoverageItemStatus::Approved);
    }

    #[test]
    fn missing_slot_errors() {
        let mut doc = Document::new();
        let id = seed(&mut doc);
        let mut cmd = RemoveEntryCustomSlot::new(id, "nope");
        assert!(matches!(cmd.apply(&mut doc), Err(CommandError::InvalidState)));
    }

    #[test]
    fn missing_entry_errors() {
        let mut doc = Document::new();
        let mut cmd = RemoveEntryCustomSlot::new(CodexEntryId(99), "x");
        assert!(matches!(cmd.apply(&mut doc), Err(CommandError::CodexEntryNotFound(_))));
    }
}
