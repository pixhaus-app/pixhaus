//! [`ApplyCoverageTemplate`]: attach a project template to an entry, reversibly.

use crate::codex::{CodexEntryId, CoverageItemStatus, CoverageKey, CoverageTemplateId};
use crate::command::{Command, CommandError};
use crate::document::Document;

/// Applies a project coverage template to an entry: the template id is appended to the
/// entry's `applied_templates` (if not already there), and every slot the entry does
/// not already cover is seeded to [`CoverageItemStatus::Missing`]. The template must
/// already exist on the Codex. Undo detaches the id (only if this apply attached it)
/// and removes only the slots this apply seeded.
pub struct ApplyCoverageTemplate {
    entry: CodexEntryId,
    template: CoverageTemplateId,
    /// Whether this apply appended the id to the entry (for undo to detach it).
    attached: bool,
    /// The slot keys this apply seeded (for undo to remove exactly those).
    seeded_slots: Vec<String>,
    /// Set once apply has run, so undo can detect being called before apply.
    applied: bool,
}

impl ApplyCoverageTemplate {
    /// A command that will apply the template `template_id` to the entry `entry_id`.
    pub fn new(entry_id: CodexEntryId, template_id: CoverageTemplateId) -> Self {
        Self {
            entry: entry_id,
            template: template_id,
            attached: false,
            seeded_slots: Vec::new(),
            applied: false,
        }
    }
}

impl Command for ApplyCoverageTemplate {
    fn apply(&mut self, doc: &mut Document) -> Result<(), CommandError> {
        let codex = doc.codex_mut();
        if codex.entry(self.entry).is_none() {
            return Err(CommandError::CodexEntryNotFound(self.entry));
        }
        let Some(template) = codex.coverage_template(self.template) else {
            return Err(CommandError::CoverageTemplateNotFound(self.template));
        };
        let slot_keys: Vec<String> = template.slots.iter().map(|s| s.key.clone()).collect();

        let mut seeded_slots = Vec::new();
        for slot in &slot_keys {
            let key = CoverageKey::new(self.entry, slot.clone());
            if let std::collections::btree_map::Entry::Vacant(vacant) = codex.coverage_state.entry(key) {
                vacant.insert(CoverageItemStatus::Missing);
                seeded_slots.push(slot.clone());
            }
        }

        let mut attached = false;
        if let Some(entry) = codex.entry_mut(self.entry)
            && !entry.applied_templates.contains(&self.template)
        {
            entry.applied_templates.push(self.template);
            attached = true;
        }

        doc.bump_revision();
        self.attached = attached;
        self.seeded_slots = seeded_slots;
        self.applied = true;
        Ok(())
    }

    fn undo(&mut self, doc: &mut Document) -> Result<(), CommandError> {
        if !self.applied {
            return Err(CommandError::InvalidState);
        }
        let codex = doc.codex_mut();
        for slot in std::mem::take(&mut self.seeded_slots) {
            codex.coverage_state.remove(&CoverageKey::new(self.entry, slot));
        }
        if self.attached
            && let Some(entry) = codex.entry_mut(self.entry)
        {
            entry.applied_templates.retain(|t| *t != self.template);
        }
        self.attached = false;
        self.applied = false;
        doc.bump_revision();
        Ok(())
    }

    fn label_key(&self) -> &'static str {
        "command.codex.apply_coverage_template"
    }

    fn estimated_size_bytes(&self) -> usize {
        std::mem::size_of::<Self>() + self.seeded_slots.iter().map(String::len).sum::<usize>()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::codex::{CodexHandle, EntryType};
    use crate::commands::{AddCodexEntry, CodexEntryProto, CreateCoverageTemplate};

    fn seed_entry(doc: &mut Document) -> CodexEntryId {
        let mut add = AddCodexEntry::new(CodexEntryProto {
            handle: CodexHandle::new("bit").unwrap(),
            name: "Bit".to_owned(),
            entry_type: EntryType::Character,
        });
        add.apply(doc).unwrap();
        add.inserted_id().unwrap()
    }

    fn seed_template(doc: &mut Document) -> CoverageTemplateId {
        let t = crate::codex::CoverageTemplate::platformer_character();
        let mut create = CreateCoverageTemplate::new(t.name.clone(), t.slots);
        create.apply(doc).unwrap();
        create.inserted_id().unwrap()
    }

    #[test]
    fn apply_attaches_and_seeds_then_undo_reverses() {
        let mut doc = Document::new();
        let id = seed_entry(&mut doc);
        let template = seed_template(&mut doc);
        let mut cmd = ApplyCoverageTemplate::new(id, template);

        cmd.apply(&mut doc).unwrap();
        assert_eq!(doc.codex().coverage_status(id, "idle"), CoverageItemStatus::Missing);
        assert_eq!(doc.codex().coverage_state().len(), 9);
        assert_eq!(doc.codex().entry(id).unwrap().applied_templates, vec![template]);

        cmd.undo(&mut doc).unwrap();
        assert_eq!(doc.codex().coverage_state().len(), 0);
        assert!(doc.codex().entry(id).unwrap().applied_templates.is_empty());
    }

    #[test]
    fn redo_re_seeds() {
        let mut doc = Document::new();
        let id = seed_entry(&mut doc);
        let template = seed_template(&mut doc);
        let mut cmd = ApplyCoverageTemplate::new(id, template);
        cmd.apply(&mut doc).unwrap();
        cmd.undo(&mut doc).unwrap();
        cmd.apply(&mut doc).unwrap();
        assert_eq!(doc.codex().coverage_state().len(), 9);
        assert_eq!(doc.codex().entry(id).unwrap().applied_templates, vec![template]);
    }

    #[test]
    fn missing_entry_errors() {
        let mut doc = Document::new();
        let template = seed_template(&mut doc);
        let mut cmd = ApplyCoverageTemplate::new(CodexEntryId(99), template);
        assert!(matches!(cmd.apply(&mut doc), Err(CommandError::CodexEntryNotFound(_))));
    }

    #[test]
    fn missing_template_errors() {
        let mut doc = Document::new();
        let id = seed_entry(&mut doc);
        let mut cmd = ApplyCoverageTemplate::new(id, CoverageTemplateId(99));
        assert!(matches!(cmd.apply(&mut doc), Err(CommandError::CoverageTemplateNotFound(_))));
    }

    #[test]
    fn applying_to_one_entry_does_not_bleed_to_another() {
        let mut doc = Document::new();
        let a = seed_entry(&mut doc);
        let mut add = AddCodexEntry::new(CodexEntryProto {
            handle: CodexHandle::new("mossy").unwrap(),
            name: "Mossy".to_owned(),
            entry_type: EntryType::Material,
        });
        add.apply(&mut doc).unwrap();
        let b = add.inserted_id().unwrap();
        let template = seed_template(&mut doc);

        ApplyCoverageTemplate::new(a, template).apply(&mut doc).unwrap();

        // B has no applied templates and no seeded coverage cells.
        assert!(doc.codex().entry(b).unwrap().applied_templates.is_empty());
        assert!(doc.codex().coverage_state().keys().all(|k| k.entry == a));
    }
}
