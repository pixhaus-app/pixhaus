//! [`ApplyBuiltinCoverageTemplate`]: pick a built-in preset and apply it in one step.
//!
//! The convenience the picker reaches for: if the project does not yet have a template
//! matching the chosen [`BuiltinCoveragePreset`], one is created from the preset's data;
//! then it is applied to the entry. Both halves are one undo step — undo detaches and
//! seeds exactly what apply did, and removes the template only if this apply created it.
//! The match is by the preset's stable template name, so picking the same preset twice
//! reuses the one template.

use crate::codex::{CodexEntryId, CoverageItemStatus, CoverageKey, CoverageTemplate, CoverageTemplateId};
use crate::command::{Command, CommandError};
use crate::document::Document;

/// Which built-in coverage preset to apply.
#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub enum BuiltinCoveragePreset {
    /// idle, walk, run, jump, fall, land, attack, hurt, death.
    PlatformerCharacter,
    /// idle, walk up/down/left/right, attack.
    TopDownFourDirection,
    /// normal, hover, pressed, disabled.
    UiButtonStates,
}

impl BuiltinCoveragePreset {
    /// The template data for this preset (carrying the unassigned id until registered).
    fn template(self) -> CoverageTemplate {
        match self {
            Self::PlatformerCharacter => CoverageTemplate::platformer_character(),
            Self::TopDownFourDirection => CoverageTemplate::top_down_four_direction(),
            Self::UiButtonStates => CoverageTemplate::ui_button_states(),
        }
    }
}

/// What apply did, captured so undo reverses it exactly.
struct Applied {
    /// The template the entry was attached to.
    template: CoverageTemplateId,
    /// Whether this apply created the template (for undo to remove it).
    created_template: bool,
    /// Whether this apply attached the id to the entry (for undo to detach it).
    attached: bool,
    /// The slot keys this apply seeded (for undo to remove exactly those).
    seeded_slots: Vec<String>,
}

/// Applies a built-in coverage preset to an entry, creating the project template first
/// if it is not already present. One undo step.
pub struct ApplyBuiltinCoverageTemplate {
    entry: CodexEntryId,
    preset: BuiltinCoveragePreset,
    applied: Option<Applied>,
}

impl ApplyBuiltinCoverageTemplate {
    /// A command that will apply `preset` to the entry `entry_id`.
    pub fn new(entry_id: CodexEntryId, preset: BuiltinCoveragePreset) -> Self {
        Self {
            entry: entry_id,
            preset,
            applied: None,
        }
    }
}

impl Command for ApplyBuiltinCoverageTemplate {
    fn apply(&mut self, doc: &mut Document) -> Result<(), CommandError> {
        let preset = self.preset.template();
        let codex = doc.codex_mut();
        if codex.entry(self.entry).is_none() {
            return Err(CommandError::CodexEntryNotFound(self.entry));
        }

        // Reuse an existing template with the preset's name, or create one.
        let existing = codex.coverage_templates().find(|t| t.name == preset.name).map(|t| t.id);
        let (template_id, created_template) = if let Some(id) = existing {
            (id, false)
        } else {
            let id = codex.mint_coverage_template_id();
            let mut template = preset.clone();
            template.id = id;
            codex.insert_coverage_template(template);
            (id, true)
        };

        // Seed the slots the entry does not already cover.
        let slot_keys: Vec<String> = preset.slots.iter().map(|s| s.key.clone()).collect();
        let mut seeded_slots = Vec::new();
        for slot in &slot_keys {
            let key = CoverageKey::new(self.entry, slot.clone());
            if let std::collections::btree_map::Entry::Vacant(vacant) = codex.coverage_state.entry(key) {
                vacant.insert(CoverageItemStatus::Missing);
                seeded_slots.push(slot.clone());
            }
        }

        // Attach the template id to the entry if absent.
        let mut attached = false;
        if let Some(entry) = codex.entry_mut(self.entry)
            && !entry.applied_templates.contains(&template_id)
        {
            entry.applied_templates.push(template_id);
            attached = true;
        }

        self.applied = Some(Applied {
            template: template_id,
            created_template,
            attached,
            seeded_slots,
        });
        doc.bump_revision();
        Ok(())
    }

    fn undo(&mut self, doc: &mut Document) -> Result<(), CommandError> {
        let applied = self.applied.take().ok_or(CommandError::InvalidState)?;
        let codex = doc.codex_mut();
        for slot in applied.seeded_slots {
            codex.coverage_state.remove(&CoverageKey::new(self.entry, slot));
        }
        if applied.attached
            && let Some(entry) = codex.entry_mut(self.entry)
        {
            entry.applied_templates.retain(|t| *t != applied.template);
        }
        // Removing the template here is safe only because the command history is strict
        // LIFO (services/history.rs pops newest-first, Transaction::undo reverses in that
        // order). Any later command that attached this same template to another entry
        // recorded created_template == false and is undone before this one runs, so no live
        // entry can still reference the id when we remove it. Do not add a scan-all-entries
        // guard: it would defend against an out-of-order undo the history never produces.
        if applied.created_template {
            codex.remove_coverage_template(applied.template);
        }
        doc.bump_revision();
        Ok(())
    }

    fn label_key(&self) -> &'static str {
        "command.codex.apply_builtin_coverage_template"
    }

    fn estimated_size_bytes(&self) -> usize {
        std::mem::size_of::<Self>() + self.applied.as_ref().map_or(0, |a| a.seeded_slots.iter().map(String::len).sum::<usize>())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::codex::{CodexHandle, EntryType};
    use crate::commands::{AddCodexEntry, CodexEntryProto};

    fn seed_entry(doc: &mut Document, handle: &str) -> CodexEntryId {
        let mut add = AddCodexEntry::new(CodexEntryProto {
            handle: CodexHandle::new(handle).unwrap(),
            name: "x".to_owned(),
            entry_type: EntryType::Character,
        });
        add.apply(doc).unwrap();
        add.inserted_id().unwrap()
    }

    #[test]
    fn apply_creates_and_seeds_then_undo_reverses() {
        let mut doc = Document::new();
        let id = seed_entry(&mut doc, "bit");
        let mut cmd = ApplyBuiltinCoverageTemplate::new(id, BuiltinCoveragePreset::PlatformerCharacter);

        cmd.apply(&mut doc).unwrap();
        assert_eq!(doc.codex().coverage_templates().count(), 1);
        assert_eq!(doc.codex().coverage_state().len(), 9);
        assert_eq!(doc.codex().entry(id).unwrap().applied_templates.len(), 1);

        cmd.undo(&mut doc).unwrap();
        assert_eq!(doc.codex().coverage_templates().count(), 0);
        assert_eq!(doc.codex().coverage_state().len(), 0);
        assert!(doc.codex().entry(id).unwrap().applied_templates.is_empty());
    }

    #[test]
    fn second_entry_reuses_the_same_template() {
        let mut doc = Document::new();
        let a = seed_entry(&mut doc, "bit");
        let b = seed_entry(&mut doc, "mossy");
        ApplyBuiltinCoverageTemplate::new(a, BuiltinCoveragePreset::PlatformerCharacter)
            .apply(&mut doc)
            .unwrap();
        ApplyBuiltinCoverageTemplate::new(b, BuiltinCoveragePreset::PlatformerCharacter)
            .apply(&mut doc)
            .unwrap();
        // One shared template, both entries reference the same id.
        assert_eq!(doc.codex().coverage_templates().count(), 1);
        let template_id = doc.codex().coverage_templates().next().unwrap().id;
        assert_eq!(doc.codex().entry(a).unwrap().applied_templates, vec![template_id]);
        assert_eq!(doc.codex().entry(b).unwrap().applied_templates, vec![template_id]);
    }

    #[test]
    fn redo_after_undo_restores_state() {
        let mut doc = Document::new();
        let id = seed_entry(&mut doc, "bit");
        let mut cmd = ApplyBuiltinCoverageTemplate::new(id, BuiltinCoveragePreset::UiButtonStates);
        cmd.apply(&mut doc).unwrap();
        cmd.undo(&mut doc).unwrap();
        cmd.apply(&mut doc).unwrap();
        assert_eq!(doc.codex().coverage_state().len(), 4);
    }

    #[test]
    fn missing_entry_errors() {
        let mut doc = Document::new();
        let mut cmd = ApplyBuiltinCoverageTemplate::new(CodexEntryId(99), BuiltinCoveragePreset::PlatformerCharacter);
        assert!(matches!(cmd.apply(&mut doc), Err(CommandError::CodexEntryNotFound(_))));
    }

    #[test]
    fn undo_of_a_reusing_apply_leaves_the_shared_template() {
        // First apply to A creates the template (created_template == true); a second apply
        // of the same preset to B reuses it (created_template == false). Undoing B's command
        // must detach only B, never remove the template A still references.
        let mut doc = Document::new();
        let a = seed_entry(&mut doc, "bit");
        let b = seed_entry(&mut doc, "mossy");
        ApplyBuiltinCoverageTemplate::new(a, BuiltinCoveragePreset::PlatformerCharacter)
            .apply(&mut doc)
            .unwrap();
        let mut cmd_b = ApplyBuiltinCoverageTemplate::new(b, BuiltinCoveragePreset::PlatformerCharacter);
        cmd_b.apply(&mut doc).unwrap();
        let template_id = doc.codex().coverage_templates().next().unwrap().id;

        cmd_b.undo(&mut doc).unwrap();
        assert_eq!(doc.codex().coverage_templates().count(), 1);
        assert!(doc.codex().entry(a).unwrap().applied_templates.contains(&template_id));
        assert!(doc.codex().entry(b).unwrap().applied_templates.is_empty());
    }

    #[test]
    fn top_down_preset_round_trips_apply_undo() {
        let mut doc = Document::new();
        let id = seed_entry(&mut doc, "bit");
        let expected_slots = CoverageTemplate::top_down_four_direction().slots.len();
        let mut cmd = ApplyBuiltinCoverageTemplate::new(id, BuiltinCoveragePreset::TopDownFourDirection);

        cmd.apply(&mut doc).unwrap();
        assert_eq!(doc.codex().coverage_state().len(), expected_slots);
        assert_eq!(doc.codex().entry(id).unwrap().applied_templates.len(), 1);

        cmd.undo(&mut doc).unwrap();
        assert_eq!(doc.codex().coverage_state().len(), 0);
        assert!(doc.codex().entry(id).unwrap().applied_templates.is_empty());
    }
}
