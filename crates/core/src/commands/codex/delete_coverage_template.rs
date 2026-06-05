//! [`DeleteCoverageTemplate`]: remove a project template, reversibly.
//!
//! Deleting a template detaches it from every entry that applied it. Undo restores the
//! template and re-attaches it to exactly those entries, at their prior positions. The
//! per-slot coverage state cells are left untouched — detaching a template does not
//! clear an entry's status (that stays a separate, explicit action).

use crate::codex::{CodexEntryId, CoverageTemplate, CoverageTemplateId};
use crate::command::{Command, CommandError};
use crate::document::Document;

/// What apply removed, captured so undo restores it exactly.
struct Undo {
    /// The deleted template itself.
    template: CoverageTemplate,
    /// Entries the id was detached from: `(entry, position it held in applied_templates)`.
    detached: Vec<(CodexEntryId, usize)>,
}

/// Deletes a project coverage template and detaches it from every entry. Undo restores
/// the template and re-attaches it where it was.
pub struct DeleteCoverageTemplate {
    id: CoverageTemplateId,
    undo: Option<Undo>,
}

impl DeleteCoverageTemplate {
    /// A command that will delete the template `id`.
    pub fn new(id: CoverageTemplateId) -> Self {
        Self { id, undo: None }
    }
}

impl Command for DeleteCoverageTemplate {
    fn apply(&mut self, doc: &mut Document) -> Result<(), CommandError> {
        let codex = doc.codex_mut();
        if codex.coverage_template(self.id).is_none() {
            return Err(CommandError::CoverageTemplateNotFound(self.id));
        }

        // Detach from every entry that applied it, capturing the prior position.
        let entry_ids: Vec<CodexEntryId> = codex.entries().keys().copied().collect();
        let mut detached = Vec::new();
        for entry_id in entry_ids {
            if let Some(entry) = codex.entry_mut(entry_id)
                && let Some(pos) = entry.applied_templates.iter().position(|t| *t == self.id)
            {
                entry.applied_templates.remove(pos);
                detached.push((entry_id, pos));
            }
        }

        let template = codex.remove_coverage_template(self.id).ok_or(CommandError::CoverageTemplateNotFound(self.id))?;
        self.undo = Some(Undo { template, detached });
        doc.bump_revision();
        Ok(())
    }

    fn undo(&mut self, doc: &mut Document) -> Result<(), CommandError> {
        let undo = self.undo.take().ok_or(CommandError::InvalidState)?;
        let codex = doc.codex_mut();
        codex.insert_coverage_template(undo.template);
        for (entry_id, pos) in undo.detached {
            if let Some(entry) = codex.entry_mut(entry_id) {
                let pos = pos.min(entry.applied_templates.len());
                entry.applied_templates.insert(pos, self.id);
            }
        }
        doc.bump_revision();
        Ok(())
    }

    fn label_key(&self) -> &'static str {
        "command.codex.delete_coverage_template"
    }

    fn estimated_size_bytes(&self) -> usize {
        let undo = self.undo.as_ref();
        std::mem::size_of::<Self>()
            + undo.map_or(0, |u| {
                u.template.name.len()
                    + u.template.slots.iter().map(|s| s.key.len()).sum::<usize>()
                    + u.detached.len() * std::mem::size_of::<(CodexEntryId, usize)>()
            })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::codex::{CodexHandle, EntryType};
    use crate::commands::{AddCodexEntry, ApplyCoverageTemplate, CodexEntryProto, CreateCoverageTemplate};

    fn seed_entry(doc: &mut Document, handle: &str) -> CodexEntryId {
        let mut add = AddCodexEntry::new(CodexEntryProto {
            handle: CodexHandle::new(handle).unwrap(),
            name: "x".to_owned(),
            entry_type: EntryType::Character,
        });
        add.apply(doc).unwrap();
        add.inserted_id().unwrap()
    }

    fn seed_template(doc: &mut Document) -> CoverageTemplateId {
        let mut cmd = CreateCoverageTemplate::new("custom", vec![crate::codex::CoverageSlot::custom("a", "A")]);
        cmd.apply(doc).unwrap();
        cmd.inserted_id().unwrap()
    }

    #[test]
    fn delete_detaches_from_entries_then_undo_restores() {
        let mut doc = Document::new();
        let id = seed_entry(&mut doc, "bit");
        let template = seed_template(&mut doc);
        ApplyCoverageTemplate::new(id, template).apply(&mut doc).unwrap();
        assert_eq!(doc.codex().entry(id).unwrap().applied_templates, vec![template]);

        let mut cmd = DeleteCoverageTemplate::new(template);
        cmd.apply(&mut doc).unwrap();
        assert!(doc.codex().coverage_template(template).is_none());
        assert!(doc.codex().entry(id).unwrap().applied_templates.is_empty());

        cmd.undo(&mut doc).unwrap();
        assert!(doc.codex().coverage_template(template).is_some());
        assert_eq!(doc.codex().entry(id).unwrap().applied_templates, vec![template]);
    }

    #[test]
    fn undo_restores_position_among_other_templates() {
        let mut doc = Document::new();
        let id = seed_entry(&mut doc, "bit");
        let first = seed_template(&mut doc);
        let second = seed_template(&mut doc);
        let third = seed_template(&mut doc);
        ApplyCoverageTemplate::new(id, first).apply(&mut doc).unwrap();
        ApplyCoverageTemplate::new(id, second).apply(&mut doc).unwrap();
        ApplyCoverageTemplate::new(id, third).apply(&mut doc).unwrap();

        let mut cmd = DeleteCoverageTemplate::new(second);
        cmd.apply(&mut doc).unwrap();
        assert_eq!(doc.codex().entry(id).unwrap().applied_templates, vec![first, third]);
        cmd.undo(&mut doc).unwrap();
        assert_eq!(doc.codex().entry(id).unwrap().applied_templates, vec![first, second, third]);
    }

    #[test]
    fn missing_template_errors() {
        let mut doc = Document::new();
        let mut cmd = DeleteCoverageTemplate::new(CoverageTemplateId(99));
        assert!(matches!(cmd.apply(&mut doc), Err(CommandError::CoverageTemplateNotFound(_))));
    }
}
