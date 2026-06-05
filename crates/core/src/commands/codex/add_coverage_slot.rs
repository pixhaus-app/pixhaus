//! [`AddCoverageSlot`]: append a slot to a project template, reversibly.

use crate::codex::{CoverageSlot, CoverageTemplateId};
use crate::command::{Command, CommandError};
use crate::document::Document;

/// Appends a slot to a project coverage template. The slot's key must not already be
/// present in the template. Undo removes the slot. The slot is added only to the
/// template; entries pick it up the next time their coverage is reported.
pub struct AddCoverageSlot {
    template: CoverageTemplateId,
    slot: Option<CoverageSlot>,
    /// Set once apply has run, so undo can detect being called before apply.
    applied: bool,
}

impl AddCoverageSlot {
    /// A command that will add `slot` to the template `template_id`.
    pub fn new(template_id: CoverageTemplateId, slot: CoverageSlot) -> Self {
        Self {
            template: template_id,
            slot: Some(slot),
            applied: false,
        }
    }
}

impl Command for AddCoverageSlot {
    fn apply(&mut self, doc: &mut Document) -> Result<(), CommandError> {
        let slot = self.slot.take().ok_or(CommandError::InvalidState)?;
        let Some(template) = doc.codex_mut().coverage_template_mut(self.template) else {
            self.slot = Some(slot);
            return Err(CommandError::CoverageTemplateNotFound(self.template));
        };
        if template.slots.iter().any(|s| s.key == slot.key) {
            // The key already exists; nothing to add. Keep the payload and report
            // invalid state so the caller knows the add was a no-op duplicate.
            self.slot = Some(slot);
            return Err(CommandError::InvalidState);
        }
        template.slots.push(slot.clone());
        self.slot = Some(slot);
        self.applied = true;
        doc.bump_revision();
        Ok(())
    }

    fn undo(&mut self, doc: &mut Document) -> Result<(), CommandError> {
        if !self.applied {
            return Err(CommandError::InvalidState);
        }
        let slot = self.slot.as_ref().ok_or(CommandError::InvalidState)?;
        let template = doc
            .codex_mut()
            .coverage_template_mut(self.template)
            .ok_or(CommandError::CoverageTemplateNotFound(self.template))?;
        template.slots.retain(|s| s.key != slot.key);
        self.applied = false;
        doc.bump_revision();
        Ok(())
    }

    fn label_key(&self) -> &'static str {
        "command.codex.add_coverage_slot"
    }

    fn estimated_size_bytes(&self) -> usize {
        std::mem::size_of::<Self>() + self.slot.as_ref().map_or(0, |s| s.key.len())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::commands::CreateCoverageTemplate;

    fn seed(doc: &mut Document) -> CoverageTemplateId {
        let mut cmd = CreateCoverageTemplate::new("custom", vec![CoverageSlot::custom("a", "A")]);
        cmd.apply(doc).unwrap();
        cmd.inserted_id().unwrap()
    }

    #[test]
    fn apply_adds_then_undo_removes() {
        let mut doc = Document::new();
        let id = seed(&mut doc);
        let mut cmd = AddCoverageSlot::new(id, CoverageSlot::custom("b", "B"));

        cmd.apply(&mut doc).unwrap();
        assert_eq!(doc.codex().coverage_template(id).unwrap().slots.len(), 2);

        cmd.undo(&mut doc).unwrap();
        assert_eq!(doc.codex().coverage_template(id).unwrap().slots.len(), 1);
        assert_eq!(doc.codex().coverage_template(id).unwrap().slots[0].key, "a");
    }

    #[test]
    fn duplicate_key_errors() {
        let mut doc = Document::new();
        let id = seed(&mut doc);
        let mut cmd = AddCoverageSlot::new(id, CoverageSlot::custom("a", "A again"));
        assert!(matches!(cmd.apply(&mut doc), Err(CommandError::InvalidState)));
    }

    #[test]
    fn missing_template_errors() {
        let mut doc = Document::new();
        let mut cmd = AddCoverageSlot::new(CoverageTemplateId(99), CoverageSlot::custom("b", "B"));
        assert!(matches!(cmd.apply(&mut doc), Err(CommandError::CoverageTemplateNotFound(_))));
    }
}
