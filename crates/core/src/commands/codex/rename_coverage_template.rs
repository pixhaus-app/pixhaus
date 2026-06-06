//! [`RenameCoverageTemplate`]: change a template's display name, reversibly.

use crate::codex::CoverageTemplateId;
use crate::command::CommandError;
use crate::commands::macros::swap_field_command;

// The exact single-field swap: replace the template's display name, keep the prior name
// for undo. The id and slots are untouched, so entries that reference the template keep
// their coverage. The macro's `new` takes `impl Into<String>`, preserving the
// `&str`-accepting signature.
swap_field_command!(
    /// Renames a project coverage template. Undo restores the prior name. The id and slots
    /// are untouched, so entries that reference the template keep their coverage.
    RenameCoverageTemplate,
    ctor: into,
    id: CoverageTemplateId,
    value: String,
    accessor: coverage_template_mut,
    not_found: CommandError::CoverageTemplateNotFound,
    field: name,
    label: "command.codex.rename_coverage_template",
    held_size: String::len,
);

#[cfg(test)]
mod tests {
    use super::*;
    use crate::command::Command;
    use crate::commands::CreateCoverageTemplate;
    use crate::document::Document;

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
