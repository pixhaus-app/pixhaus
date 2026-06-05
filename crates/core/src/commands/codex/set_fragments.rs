//! [`SetPromptFragments`] / [`SetNegativeFragments`]: replace an entry's prompt
//! fragment lists wholesale, reversibly.
//!
//! Each command swaps the whole list and keeps the prior list for undo — the simplest
//! faithful reverse for list edits where the UI rebuilds the list each time.

use crate::codex::{CodexEntryId, PromptFragment};
use crate::command::{Command, CommandError};
use crate::document::Document;

/// Replaces an entry's positive prompt fragments. Undo restores the prior list.
pub struct SetPromptFragments {
    id: CodexEntryId,
    fragments: Option<Vec<PromptFragment>>,
}

impl SetPromptFragments {
    /// A command that will set the entry `id`'s prompt fragments to `fragments`.
    pub fn new(id: CodexEntryId, fragments: Vec<PromptFragment>) -> Self {
        Self {
            id,
            fragments: Some(fragments),
        }
    }
}

impl Command for SetPromptFragments {
    fn apply(&mut self, doc: &mut Document) -> Result<(), CommandError> {
        let fragments = self.fragments.take().ok_or(CommandError::InvalidState)?;
        let entry = doc.codex_mut().entry_mut(self.id).ok_or(CommandError::CodexEntryNotFound(self.id))?;
        self.fragments = Some(std::mem::replace(&mut entry.prompt_fragments, fragments));
        doc.bump_revision();
        Ok(())
    }

    fn undo(&mut self, doc: &mut Document) -> Result<(), CommandError> {
        // apply and undo are symmetric: each swaps the held list with the live one.
        self.apply(doc)
    }

    fn label_key(&self) -> &'static str {
        "command.codex.set_prompt_fragments"
    }

    fn estimated_size_bytes(&self) -> usize {
        std::mem::size_of::<Self>() + self.fragments.as_ref().map_or(0, |fs| fs.iter().map(|f| f.text.len()).sum::<usize>())
    }
}

/// Replaces an entry's negative prompt fragments. Undo restores the prior list.
pub struct SetNegativeFragments {
    id: CodexEntryId,
    fragments: Option<Vec<String>>,
}

impl SetNegativeFragments {
    /// A command that will set the entry `id`'s negative fragments to `fragments`.
    pub fn new(id: CodexEntryId, fragments: Vec<String>) -> Self {
        Self {
            id,
            fragments: Some(fragments),
        }
    }
}

impl Command for SetNegativeFragments {
    fn apply(&mut self, doc: &mut Document) -> Result<(), CommandError> {
        let fragments = self.fragments.take().ok_or(CommandError::InvalidState)?;
        let entry = doc.codex_mut().entry_mut(self.id).ok_or(CommandError::CodexEntryNotFound(self.id))?;
        self.fragments = Some(std::mem::replace(&mut entry.negative_fragments, fragments));
        doc.bump_revision();
        Ok(())
    }

    fn undo(&mut self, doc: &mut Document) -> Result<(), CommandError> {
        self.apply(doc)
    }

    fn label_key(&self) -> &'static str {
        "command.codex.set_negative_fragments"
    }

    fn estimated_size_bytes(&self) -> usize {
        std::mem::size_of::<Self>() + self.fragments.as_ref().map_or(0, |fs| fs.iter().map(String::len).sum::<usize>())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::codex::{CodexHandle, EntryType, InclusionPriority};
    use crate::commands::{AddCodexEntry, CodexEntryProto};

    fn seed(doc: &mut Document) -> CodexEntryId {
        let mut add = AddCodexEntry::new(CodexEntryProto {
            handle: CodexHandle::new("bit").unwrap(),
            name: "Bit".to_owned(),
            entry_type: EntryType::Character,
        });
        add.apply(doc).unwrap();
        add.inserted_id().unwrap()
    }

    #[test]
    fn set_prompt_fragments_round_trips() {
        let mut doc = Document::new();
        let id = seed(&mut doc);
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
        let id = seed(&mut doc);
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
