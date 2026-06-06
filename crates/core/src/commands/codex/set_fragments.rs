//! [`SetPromptFragments`] / [`SetNegativeFragments`]: replace an entry's prompt
//! fragment lists wholesale, reversibly.
//!
//! Each command swaps the whole list and keeps the prior list for undo — the simplest
//! faithful reverse for list edits where the UI rebuilds the list each time.

use crate::codex::{CodexEntryId, PromptFragment};
use crate::command::CommandError;
use crate::commands::macros::swap_field_command;

// Both commands are the exact single-field swap: take the submitted list, swap it into
// the entry, keep the prior list for undo. The only per-command differences the macro
// threads are the held type, the target field, the label key, and how a held list
// contributes to the size estimate. `ctor: value` keeps the by-value `new` so a
// `.collect()` argument at the call site still infers its target type.
swap_field_command!(
    /// Replaces an entry's positive prompt fragments. Undo restores the prior list.
    SetPromptFragments,
    ctor: value,
    id: CodexEntryId,
    value: Vec<PromptFragment>,
    accessor: entry_mut,
    not_found: CommandError::CodexEntryNotFound,
    field: prompt_fragments,
    label: "command.codex.set_prompt_fragments",
    held_size: |fs: &Vec<PromptFragment>| fs.iter().map(|f| f.text.len()).sum::<usize>(),
);

swap_field_command!(
    /// Replaces an entry's negative prompt fragments. Undo restores the prior list.
    SetNegativeFragments,
    ctor: value,
    id: CodexEntryId,
    value: Vec<String>,
    accessor: entry_mut,
    not_found: CommandError::CodexEntryNotFound,
    field: negative_fragments,
    label: "command.codex.set_negative_fragments",
    held_size: |fs: &Vec<String>| fs.iter().map(String::len).sum::<usize>(),
);

#[cfg(test)]
mod tests {
    use super::*;
    use crate::codex::InclusionPriority;
    use crate::command::Command;
    use crate::document::Document;
    use crate::test_support::seed_bit;

    #[test]
    fn set_prompt_fragments_round_trips() {
        let mut doc = Document::new();
        let id = seed_bit(&mut doc);
        let frags = vec![PromptFragment::new("round head", InclusionPriority::Critical)];
        let mut cmd = SetPromptFragments::new(id, frags);

        cmd.apply(&mut doc).unwrap();
        assert_eq!(doc.codex().entry(id).unwrap().prompt_fragments.len(), 1);

        cmd.undo(&mut doc).unwrap();
        assert!(doc.codex().entry(id).unwrap().prompt_fragments.is_empty());

        cmd.apply(&mut doc).unwrap();
        assert_eq!(doc.codex().entry(id).unwrap().prompt_fragments[0].text, "round head");
    }

    #[test]
    fn set_negative_fragments_round_trips() {
        let mut doc = Document::new();
        let id = seed_bit(&mut doc);
        let mut cmd = SetNegativeFragments::new(id, vec!["extra limbs".to_owned()]);

        cmd.apply(&mut doc).unwrap();
        assert_eq!(doc.codex().entry(id).unwrap().negative_fragments, vec!["extra limbs".to_owned()]);

        cmd.undo(&mut doc).unwrap();
        assert!(doc.codex().entry(id).unwrap().negative_fragments.is_empty());
    }

    #[test]
    fn missing_entry_errors() {
        let mut doc = Document::new();
        let mut cmd = SetPromptFragments::new(CodexEntryId(9), Vec::new());
        assert!(matches!(cmd.apply(&mut doc), Err(CommandError::CodexEntryNotFound(_))));
    }
}
