//! [`AddEntryCustomSlot`]: append a per-entry ad-hoc coverage slot, reversibly.
//!
//! A custom slot belongs to one entry, not a template — for a one-off coverage need.
//! Apply also seeds the slot's coverage cell to
//! [`Missing`](crate::codex::CoverageItemStatus::Missing) when vacant. Undo removes the
//! slot and the cell it seeded.

use crate::codex::{CodexEntryId, CoverageItemStatus, CoverageKey, CoverageSlot};
use crate::command::{Command, CommandError};
use crate::document::Document;

/// Adds an ad-hoc coverage slot to one entry. The slot's key must not already be in the
/// entry's custom slots. Undo removes the slot and any coverage cell it seeded.
pub struct AddEntryCustomSlot {
    entry: CodexEntryId,
    slot: Option<CoverageSlot>,
    /// Whether apply seeded the coverage cell (for undo to remove it).
    seeded_cell: bool,
    /// Set once apply has run, so undo can detect being called before apply.
    applied: bool,
}

impl AddEntryCustomSlot {
    /// A command that will add the custom `slot` to the entry `entry_id`.
    pub fn new(entry_id: CodexEntryId, slot: CoverageSlot) -> Self {
        Self {
            entry: entry_id,
            slot: Some(slot),
            seeded_cell: false,
            applied: false,
        }
    }
}

impl Command for AddEntryCustomSlot {
    fn apply(&mut self, doc: &mut Document) -> Result<(), CommandError> {
        let slot = self.slot.take().ok_or(CommandError::InvalidState)?;
        let codex = doc.codex_mut();
        let Some(entry) = codex.entry_mut(self.entry) else {
            self.slot = Some(slot);
            return Err(CommandError::CodexEntryNotFound(self.entry));
        };
        if entry.custom_slots.iter().any(|s| s.key == slot.key) {
            self.slot = Some(slot);
            return Err(CommandError::InvalidState);
        }
        let slot_key = slot.key.clone();
        entry.custom_slots.push(slot.clone());

        let mut seeded_cell = false;
        let key = CoverageKey::new(self.entry, slot_key);
        if let std::collections::btree_map::Entry::Vacant(vacant) = codex.coverage_state.entry(key) {
            vacant.insert(CoverageItemStatus::Missing);
            seeded_cell = true;
        }

        self.slot = Some(slot);
        self.seeded_cell = seeded_cell;
        self.applied = true;
        doc.bump_revision();
        Ok(())
    }

    fn undo(&mut self, doc: &mut Document) -> Result<(), CommandError> {
        if !self.applied {
            return Err(CommandError::InvalidState);
        }
        let slot = self.slot.as_ref().ok_or(CommandError::InvalidState)?;
        let codex = doc.codex_mut();
        if let Some(entry) = codex.entry_mut(self.entry) {
            entry.custom_slots.retain(|s| s.key != slot.key);
        }
        // Removing the seeded Missing cell assumes LIFO undo ordering: any later
        // SetCoverageStatus on this same cell is undone first (restoring it to Missing)
        // before this command's undo runs, so the unconditional remove never drops a value
        // a still-live command edited.
        if self.seeded_cell {
            codex.coverage_state.remove(&CoverageKey::new(self.entry, slot.key.clone()));
        }
        self.seeded_cell = false;
        self.applied = false;
        doc.bump_revision();
        Ok(())
    }

    fn label_key(&self) -> &'static str {
        "command.codex.add_entry_custom_slot"
    }

    fn estimated_size_bytes(&self) -> usize {
        std::mem::size_of::<Self>() + self.slot.as_ref().map_or(0, |s| s.key.len())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::seed_bit;

    #[test]
    fn apply_adds_and_seeds_then_undo_reverses() {
        let mut doc = Document::new();
        let id = seed_bit(&mut doc);
        let mut cmd = AddEntryCustomSlot::new(id, CoverageSlot::custom("victory", "Victory pose"));

        cmd.apply(&mut doc).unwrap();
        assert_eq!(doc.codex().entry(id).unwrap().custom_slots.len(), 1);
        assert_eq!(doc.codex().coverage_status(id, "victory"), CoverageItemStatus::Missing);
        assert_eq!(doc.codex().coverage_state().len(), 1);

        cmd.undo(&mut doc).unwrap();
        assert!(doc.codex().entry(id).unwrap().custom_slots.is_empty());
        assert_eq!(doc.codex().coverage_state().len(), 0);
    }

    #[test]
    fn duplicate_key_errors() {
        let mut doc = Document::new();
        let id = seed_bit(&mut doc);
        AddEntryCustomSlot::new(id, CoverageSlot::custom("victory", "Victory")).apply(&mut doc).unwrap();
        let mut cmd = AddEntryCustomSlot::new(id, CoverageSlot::custom("victory", "again"));
        assert!(matches!(cmd.apply(&mut doc), Err(CommandError::InvalidState)));
    }

    #[test]
    fn missing_entry_errors() {
        let mut doc = Document::new();
        let mut cmd = AddEntryCustomSlot::new(CodexEntryId(99), CoverageSlot::custom("x", "X"));
        assert!(matches!(cmd.apply(&mut doc), Err(CommandError::CodexEntryNotFound(_))));
    }
}
