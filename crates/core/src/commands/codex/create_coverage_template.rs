//! [`CreateCoverageTemplate`]: mint a project coverage template, reversibly.
//!
//! The command takes a display name and the slot list (each slot's label already a
//! [`CoverageLabel`](crate::codex::CoverageLabel)) and mints a stable
//! [`CoverageTemplateId`]. Undo removes the exact template that was minted.

use crate::codex::{CoverageSlot, CoverageTemplate, CoverageTemplateId};
use crate::command::{Command, CommandError};
use crate::document::Document;

/// Records what [`CreateCoverageTemplate::apply`] inserted, so undo removes exactly that.
struct Inserted {
    id: CoverageTemplateId,
}

/// Creates a project coverage template named `name` with `slots`. The minted id is
/// available after apply via [`inserted_id`](Self::inserted_id).
pub struct CreateCoverageTemplate {
    name: String,
    slots: Vec<CoverageSlot>,
    inserted: Option<Inserted>,
}

impl CreateCoverageTemplate {
    /// A command that will create a template named `name` with `slots`.
    pub fn new(name: impl Into<String>, slots: Vec<CoverageSlot>) -> Self {
        Self {
            name: name.into(),
            slots,
            inserted: None,
        }
    }

    /// The id assigned to the created template, available after [`apply`](Command::apply).
    pub fn inserted_id(&self) -> Option<CoverageTemplateId> {
        self.inserted.as_ref().map(|i| i.id)
    }
}

impl Command for CreateCoverageTemplate {
    fn apply(&mut self, doc: &mut Document) -> Result<(), CommandError> {
        let codex = doc.codex_mut();
        let id = codex.mint_coverage_template_id();
        let mut template = CoverageTemplate::new(self.name.clone(), self.slots.clone());
        template.id = id;
        codex.insert_coverage_template(template);
        self.inserted = Some(Inserted { id });
        doc.bump_revision();
        Ok(())
    }

    fn undo(&mut self, doc: &mut Document) -> Result<(), CommandError> {
        let inserted = self.inserted.take().ok_or(CommandError::InvalidState)?;
        doc.codex_mut()
            .remove_coverage_template(inserted.id)
            .ok_or(CommandError::CoverageTemplateNotFound(inserted.id))?;
        doc.bump_revision();
        Ok(())
    }

    fn label_key(&self) -> &'static str {
        "command.codex.create_coverage_template"
    }

    fn estimated_size_bytes(&self) -> usize {
        std::mem::size_of::<Self>() + self.name.len() + self.slots.iter().map(|s| s.key.len()).sum::<usize>()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::codex::CoverageSlot;

    #[test]
    fn apply_creates_then_undo_removes() {
        let mut doc = Document::new();
        let mut cmd = CreateCoverageTemplate::new("custom", vec![CoverageSlot::custom("crouch", "Crouch")]);

        cmd.apply(&mut doc).unwrap();
        let id = cmd.inserted_id().unwrap();
        let template = doc.codex().coverage_template(id).unwrap();
        assert_eq!(template.name, "custom");
        assert_eq!(template.id, id);
        assert_eq!(template.slots.len(), 1);

        cmd.undo(&mut doc).unwrap();
        assert!(doc.codex().coverage_template(id).is_none());
    }

    #[test]
    fn distinct_creates_get_distinct_ids() {
        let mut doc = Document::new();
        let mut a = CreateCoverageTemplate::new("a", vec![]);
        a.apply(&mut doc).unwrap();
        let mut b = CreateCoverageTemplate::new("b", vec![]);
        b.apply(&mut doc).unwrap();
        assert_ne!(a.inserted_id().unwrap(), b.inserted_id().unwrap());
    }

    #[test]
    fn undo_before_apply_errors() {
        let mut doc = Document::new();
        let mut cmd = CreateCoverageTemplate::new("custom", vec![]);
        assert!(matches!(cmd.undo(&mut doc), Err(CommandError::InvalidState)));
    }
}
