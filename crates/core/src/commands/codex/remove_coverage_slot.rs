//! [`RemoveCoverageSlot`]: remove a slot from a project template, reversibly.

use crate::codex::{CoverageSlot, CoverageTemplateId};
use crate::command::{Command, CommandError};
use crate::document::Document;

/// Removes the slot with `key` from a project coverage template. Undo restores the slot
/// at its prior position. The slot's coverage-state cells on individual entries are left
/// untouched.
pub struct RemoveCoverageSlot {
    template: CoverageTemplateId,
    key: String,
    /// The removed slot and its prior index, captured for undo.
    removed: Option<(usize, CoverageSlot)>,
}

impl RemoveCoverageSlot {
    /// A command that will remove the slot keyed `key` from the template `template_id`.
    pub fn new(template_id: CoverageTemplateId, key: impl Into<String>) -> Self {
        Self {
            template: template_id,
            key: key.into(),
            removed: None,
        }
    }
}

impl Command for RemoveCoverageSlot {
    fn apply(&mut self, doc: &mut Document) -> Result<(), CommandError> {
        let template = doc
            .codex_mut()
            .coverage_template_mut(self.template)
            .ok_or(CommandError::CoverageTemplateNotFound(self.template))?;
        let pos = template.slots.iter().position(|s| s.key == self.key).ok_or(CommandError::InvalidState)?;
        let slot = template.slots.remove(pos);
        self.removed = Some((pos, slot));
        doc.bump_revision();
        Ok(())
    }

    fn undo(&mut self, doc: &mut Document) -> Result<(), CommandError> {
        let (pos, slot) = self.removed.take().ok_or(CommandError::InvalidState)?;
        let template = doc
            .codex_mut()
            .coverage_template_mut(self.template)
            .ok_or(CommandError::CoverageTemplateNotFound(self.template))?;
        let pos = pos.min(template.slots.len());
        template.slots.insert(pos, slot);
        doc.bump_revision();
        Ok(())
    }

    fn label_key(&self) -> &'static str {
        "command.codex.remove_coverage_slot"
    }

    fn estimated_size_bytes(&self) -> usize {
        std::mem::size_of::<Self>() + self.key.len() + self.removed.as_ref().map_or(0, |(_, s)| s.key.len())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::commands::CreateCoverageTemplate;

    fn seed(doc: &mut Document) -> CoverageTemplateId {
        let mut cmd = CreateCoverageTemplate::new("custom", vec![CoverageSlot::custom("a", "A"), CoverageSlot::custom("b", "B")]);
        cmd.apply(doc).unwrap();
        cmd.inserted_id().unwrap()
    }

    #[test]
    fn apply_removes_then_undo_restores_position() {
        let mut doc = Document::new();
        let id = seed(&mut doc);
        let mut cmd = RemoveCoverageSlot::new(id, "a");

        cmd.apply(&mut doc).unwrap();
        let keys: Vec<String> = doc.codex().coverage_template(id).unwrap().slots.iter().map(|s| s.key.clone()).collect();
        assert_eq!(keys, vec!["b"]);

        cmd.undo(&mut doc).unwrap();
        let keys: Vec<String> = doc.codex().coverage_template(id).unwrap().slots.iter().map(|s| s.key.clone()).collect();
        assert_eq!(keys, vec!["a", "b"]);
    }

    #[test]
    fn missing_slot_errors() {
        let mut doc = Document::new();
        let id = seed(&mut doc);
        let mut cmd = RemoveCoverageSlot::new(id, "nope");
        assert!(matches!(cmd.apply(&mut doc), Err(CommandError::InvalidState)));
    }

    #[test]
    fn missing_template_errors() {
        let mut doc = Document::new();
        let mut cmd = RemoveCoverageSlot::new(CoverageTemplateId(99), "a");
        assert!(matches!(cmd.apply(&mut doc), Err(CommandError::CoverageTemplateNotFound(_))));
    }
}
