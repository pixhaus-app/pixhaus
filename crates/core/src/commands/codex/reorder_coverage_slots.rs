//! [`ReorderCoverageSlots`]: move a slot within a project template, reversibly.
//!
//! Moves the slot at index `from` to index `to`, shifting the rest. Slot keys and labels
//! are untouched, so coverage status survives. Undo moves it back.

use crate::codex::CoverageTemplateId;
use crate::command::{Command, CommandError};
use crate::document::Document;

/// Moves the slot at index `from` to index `to` within a project coverage template. Undo
/// reverses the move.
pub struct ReorderCoverageSlots {
    template: CoverageTemplateId,
    from: usize,
    to: usize,
    /// Set once apply has run, so undo can detect being called before apply.
    applied: bool,
}

impl ReorderCoverageSlots {
    /// A command that will move the slot at index `from` to index `to` in `template_id`.
    pub fn new(template_id: CoverageTemplateId, from: usize, to: usize) -> Self {
        Self {
            template: template_id,
            from,
            to,
            applied: false,
        }
    }

    fn move_slot(doc: &mut Document, template: CoverageTemplateId, from: usize, to: usize) -> Result<(), CommandError> {
        let t = doc
            .codex_mut()
            .coverage_template_mut(template)
            .ok_or(CommandError::CoverageTemplateNotFound(template))?;
        if from >= t.slots.len() || to >= t.slots.len() {
            return Err(CommandError::InvalidState);
        }
        let slot = t.slots.remove(from);
        t.slots.insert(to, slot);
        doc.bump_revision();
        Ok(())
    }
}

impl Command for ReorderCoverageSlots {
    fn apply(&mut self, doc: &mut Document) -> Result<(), CommandError> {
        Self::move_slot(doc, self.template, self.from, self.to)?;
        self.applied = true;
        Ok(())
    }

    fn undo(&mut self, doc: &mut Document) -> Result<(), CommandError> {
        if !self.applied {
            return Err(CommandError::InvalidState);
        }
        // Reverse the move: what went from `from` to `to` goes back from `to` to `from`.
        Self::move_slot(doc, self.template, self.to, self.from)?;
        self.applied = false;
        Ok(())
    }

    fn label_key(&self) -> &'static str {
        "command.codex.reorder_coverage_slots"
    }

    fn estimated_size_bytes(&self) -> usize {
        std::mem::size_of::<Self>()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::codex::CoverageSlot;
    use crate::commands::CreateCoverageTemplate;

    fn seed(doc: &mut Document) -> CoverageTemplateId {
        let slots = vec![CoverageSlot::custom("a", "A"), CoverageSlot::custom("b", "B"), CoverageSlot::custom("c", "C")];
        let mut cmd = CreateCoverageTemplate::new("custom", slots);
        cmd.apply(doc).unwrap();
        cmd.inserted_id().unwrap()
    }

    fn keys(doc: &Document, id: CoverageTemplateId) -> Vec<String> {
        doc.codex().coverage_template(id).unwrap().slots.iter().map(|s| s.key.clone()).collect()
    }

    #[test]
    fn apply_moves_then_undo_restores() {
        let mut doc = Document::new();
        let id = seed(&mut doc);
        let mut cmd = ReorderCoverageSlots::new(id, 0, 2);

        cmd.apply(&mut doc).unwrap();
        assert_eq!(keys(&doc, id), vec!["b", "c", "a"]);

        cmd.undo(&mut doc).unwrap();
        assert_eq!(keys(&doc, id), vec!["a", "b", "c"]);
    }

    #[test]
    fn out_of_range_errors() {
        let mut doc = Document::new();
        let id = seed(&mut doc);
        let mut cmd = ReorderCoverageSlots::new(id, 0, 9);
        assert!(matches!(cmd.apply(&mut doc), Err(CommandError::InvalidState)));
    }

    #[test]
    fn missing_template_errors() {
        let mut doc = Document::new();
        let mut cmd = ReorderCoverageSlots::new(CoverageTemplateId(99), 0, 1);
        assert!(matches!(cmd.apply(&mut doc), Err(CommandError::CoverageTemplateNotFound(_))));
    }
}
