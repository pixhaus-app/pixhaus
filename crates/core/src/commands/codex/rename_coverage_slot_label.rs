//! [`RenameCoverageSlotLabel`]: change a slot's label, never its key, reversibly.
//!
//! Only the label changes; the stable slot key is left untouched, so every coverage
//! status cell keyed on that slot survives the rename. Undo restores the prior label.

use crate::codex::{CoverageLabel, CoverageTemplateId};
use crate::command::{Command, CommandError};
use crate::document::Document;

/// Renames the label of the slot keyed `key` in a project coverage template. The slot
/// key never changes. Undo restores the prior label.
pub struct RenameCoverageSlotLabel {
    template: CoverageTemplateId,
    key: String,
    label: Option<CoverageLabel>,
}

impl RenameCoverageSlotLabel {
    /// A command that will set the label of slot `key` in `template_id` to `label`.
    pub fn new(template_id: CoverageTemplateId, key: impl Into<String>, label: CoverageLabel) -> Self {
        Self {
            template: template_id,
            key: key.into(),
            label: Some(label),
        }
    }
}

impl Command for RenameCoverageSlotLabel {
    fn apply(&mut self, doc: &mut Document) -> Result<(), CommandError> {
        let label = self.label.take().ok_or(CommandError::InvalidState)?;
        let template = doc.codex_mut().coverage_template_mut(self.template);
        let Some(template) = template else {
            self.label = Some(label);
            return Err(CommandError::CoverageTemplateNotFound(self.template));
        };
        let Some(slot) = template.slots.iter_mut().find(|s| s.key == self.key) else {
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
        "command.codex.rename_coverage_slot_label"
    }

    fn estimated_size_bytes(&self) -> usize {
        let label_len = self.label.as_ref().map_or(0, |l| match l {
            CoverageLabel::Key(s) | CoverageLabel::Literal(s) => s.len(),
        });
        std::mem::size_of::<Self>() + self.key.len() + label_len
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::codex::CoverageSlot;
    use crate::commands::CreateCoverageTemplate;

    fn seed(doc: &mut Document) -> CoverageTemplateId {
        let mut cmd = CreateCoverageTemplate::new("custom", vec![CoverageSlot::custom("a", "A")]);
        cmd.apply(doc).unwrap();
        cmd.inserted_id().unwrap()
    }

    #[test]
    fn apply_relabels_keeping_key_then_undo_restores() {
        let mut doc = Document::new();
        let id = seed(&mut doc);
        let mut cmd = RenameCoverageSlotLabel::new(id, "a", CoverageLabel::Literal("Renamed".to_owned()));

        cmd.apply(&mut doc).unwrap();
        let slot = &doc.codex().coverage_template(id).unwrap().slots[0];
        assert_eq!(slot.key, "a");
        assert_eq!(slot.label, CoverageLabel::Literal("Renamed".to_owned()));

        cmd.undo(&mut doc).unwrap();
        let slot = &doc.codex().coverage_template(id).unwrap().slots[0];
        assert_eq!(slot.key, "a");
        assert_eq!(slot.label, CoverageLabel::Literal("A".to_owned()));
    }

    #[test]
    fn missing_slot_errors() {
        let mut doc = Document::new();
        let id = seed(&mut doc);
        let mut cmd = RenameCoverageSlotLabel::new(id, "nope", CoverageLabel::Literal("x".to_owned()));
        assert!(matches!(cmd.apply(&mut doc), Err(CommandError::InvalidState)));
    }

    #[test]
    fn missing_template_errors() {
        let mut doc = Document::new();
        let mut cmd = RenameCoverageSlotLabel::new(CoverageTemplateId(99), "a", CoverageLabel::Literal("x".to_owned()));
        assert!(matches!(cmd.apply(&mut doc), Err(CommandError::CoverageTemplateNotFound(_))));
    }
}
