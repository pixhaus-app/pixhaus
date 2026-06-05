//! [`RenameEntryCustomSlotLabel`]: relabel a per-entry custom coverage slot, reversibly.
//!
//! The companion to [`RenameCoverageSlotLabel`](super::RenameCoverageSlotLabel), but
//! scoped to an entry's `custom_slots` rather than a project template's slots. Only the
//! label changes; the stable slot key is left untouched, so the coverage status cell
//! keyed on that slot survives the rename. Undo restores the prior label.

use crate::codex::{CodexEntryId, CoverageLabel};
use crate::command::{Command, CommandError};
use crate::document::Document;

/// Renames the label of the custom slot keyed `key` on one entry. The slot key never
/// changes. Undo restores the prior label.
pub struct RenameEntryCustomSlotLabel {
    entry: CodexEntryId,
    key: String,
    label: Option<CoverageLabel>,
}

impl RenameEntryCustomSlotLabel {
    /// A command that will set the label of the custom slot `key` on `entry_id` to `label`.
    pub fn new(entry_id: CodexEntryId, key: impl Into<String>, label: CoverageLabel) -> Self {
        Self {
            entry: entry_id,
            key: key.into(),
            label: Some(label),
        }
    }
}

impl Command for RenameEntryCustomSlotLabel {
    fn apply(&mut self, doc: &mut Document) -> Result<(), CommandError> {
        let label = self.label.take().ok_or(CommandError::InvalidState)?;
        let Some(entry) = doc.codex_mut().entry_mut(self.entry) else {
            self.label = Some(label);
            return Err(CommandError::CodexEntryNotFound(self.entry));
        };
        let Some(slot) = entry.custom_slots.iter_mut().find(|s| s.key == self.key) else {
            self.label = Some(label);
            return Err(CommandError::InvalidState);
        };
        self.label = Some(std::mem::replace(&mut slot.label, label));
        doc.bump_revision();
        Ok(())
    }

    fn undo(&mut self, doc: &mut Document) -> Result<(), CommandError> {
        // apply and undo are symmetric: each swaps the held label with the live one.
        self.apply(doc)
    }

    fn label_key(&self) -> &'static str {
        "command.codex.rename_entry_custom_slot"
    }

    fn estimated_size_bytes(&self) -> usize {
        let label_len = self.label.as_ref().map_or(0, CoverageLabel::text_len);
        std::mem::size_of::<Self>() + self.key.len() + label_len
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::codex::{CodexHandle, CoverageItemStatus, CoverageSlot, EntryType};
    use crate::commands::{AddCodexEntry, AddEntryCustomSlot, CodexEntryProto, SetCoverageStatus};

    fn seed(doc: &mut Document) -> CodexEntryId {
        let mut add = AddCodexEntry::new(CodexEntryProto {
            handle: CodexHandle::new("bit").unwrap(),
            name: "Bit".to_owned(),
            entry_type: EntryType::Character,
        });
        add.apply(doc).unwrap();
        let id = add.inserted_id().unwrap();
        AddEntryCustomSlot::new(id, CoverageSlot::custom("victory", "Victory")).apply(doc).unwrap();
        id
    }

    #[test]
    fn apply_relabels_keeping_key_then_undo_restores() {
        let mut doc = Document::new();
        let id = seed(&mut doc);
        let mut cmd = RenameEntryCustomSlotLabel::new(id, "victory", CoverageLabel::Literal("Win Pose".to_owned()));

        cmd.apply(&mut doc).unwrap();
        let slot = &doc.codex().entry(id).unwrap().custom_slots[0];
        assert_eq!(slot.key, "victory");
        assert_eq!(slot.label, CoverageLabel::Literal("Win Pose".to_owned()));

        cmd.undo(&mut doc).unwrap();
        let slot = &doc.codex().entry(id).unwrap().custom_slots[0];
        assert_eq!(slot.key, "victory");
        assert_eq!(slot.label, CoverageLabel::Literal("Victory".to_owned()));
    }

    #[test]
    fn redo_relabels_again() {
        let mut doc = Document::new();
        let id = seed(&mut doc);
        let mut cmd = RenameEntryCustomSlotLabel::new(id, "victory", CoverageLabel::Literal("Win Pose".to_owned()));
        cmd.apply(&mut doc).unwrap();
        cmd.undo(&mut doc).unwrap();
        cmd.apply(&mut doc).unwrap();
        let slot = &doc.codex().entry(id).unwrap().custom_slots[0];
        assert_eq!(slot.label, CoverageLabel::Literal("Win Pose".to_owned()));
    }

    #[test]
    fn rename_keeps_coverage_status_stable() {
        let mut doc = Document::new();
        let id = seed(&mut doc);
        SetCoverageStatus::new(id, "victory", CoverageItemStatus::Approved).apply(&mut doc).unwrap();

        RenameEntryCustomSlotLabel::new(id, "victory", CoverageLabel::Literal("Win Pose".to_owned()))
            .apply(&mut doc)
            .unwrap();

        // The status cell is keyed on the slot key, which the rename leaves untouched.
        assert_eq!(doc.codex().coverage_status(id, "victory"), CoverageItemStatus::Approved);
    }

    #[test]
    fn missing_slot_errors() {
        let mut doc = Document::new();
        let id = seed(&mut doc);
        let mut cmd = RenameEntryCustomSlotLabel::new(id, "nope", CoverageLabel::Literal("x".to_owned()));
        assert!(matches!(cmd.apply(&mut doc), Err(CommandError::InvalidState)));
    }

    #[test]
    fn missing_entry_errors() {
        let mut doc = Document::new();
        let mut cmd = RenameEntryCustomSlotLabel::new(CodexEntryId(99), "victory", CoverageLabel::Literal("x".to_owned()));
        assert!(matches!(cmd.apply(&mut doc), Err(CommandError::CodexEntryNotFound(_))));
    }
}
