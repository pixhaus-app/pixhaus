//! [`RenameCoverageTemplate`]: change a template's display name, reversibly.

use crate::codex::CoverageTemplateId;
use crate::command::{Command, CommandError};
use crate::document::Document;

/// Renames a project coverage template. Undo restores the prior name. The id and slots
/// are untouched, so entries that reference the template keep their coverage.
pub struct RenameCoverageTemplate {
    id: CoverageTemplateId,
    name: Option<String>,
}

impl RenameCoverageTemplate {
    /// A command that will rename the template `id` to `name`.
    pub fn new(id: CoverageTemplateId, name: impl Into<String>) -> Self {
        Self { id, name: Some(name.into()) }
    }
}

impl Command for RenameCoverageTemplate {
    fn apply(&mut self, doc: &mut Document) -> Result<(), CommandError> {
        let name = self.name.take().ok_or(CommandError::InvalidState)?;
        let template = doc
            .codex_mut()
            .coverage_template_mut(self.id)
            .ok_or(CommandError::CoverageTemplateNotFound(self.id))?;
        self.name = Some(std::mem::replace(&mut template.name, name));
        doc.bump_revision();
        Ok(())
    }

    fn undo(&mut self, doc: &mut Document) -> Result<(), CommandError> {
        // apply and undo are symmetric: each swaps the held name with the live one.
        self.apply(doc)
    }

    fn label_key(&self) -> &'static str {
        "command.codex.rename_coverage_template"
    }

    fn estimated_size_bytes(&self) -> usize {
        std::mem::size_of::<Self>() + self.name.as_ref().map_or(0, String::len)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::commands::CreateCoverageTemplate;

    fn seed(doc: &mut Document) -> CoverageTemplateId {
        let mut cmd = CreateCoverageTemplate::new("custom", vec![]);
        cmd.apply(doc).unwrap();
        cmd.inserted_id().unwrap()
    }

    #[test]
    fn apply_renames_then_undo_restores() {
        let mut doc = Document::new();
        let id = seed(&mut doc);
        let mut cmd = RenameCoverageTemplate::new(id, "renamed");

        cmd.apply(&mut doc).unwrap();
        assert_eq!(doc.codex().coverage_template(id).unwrap().name, "renamed");

        cmd.undo(&mut doc).unwrap();
        assert_eq!(doc.codex().coverage_template(id).unwrap().name, "custom");
    }

    #[test]
    fn missing_template_errors() {
        let mut doc = Document::new();
        let mut cmd = RenameCoverageTemplate::new(CoverageTemplateId(99), "x");
        assert!(matches!(cmd.apply(&mut doc), Err(CommandError::CoverageTemplateNotFound(_))));
    }
}
