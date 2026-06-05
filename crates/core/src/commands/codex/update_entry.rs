//! [`UpdateCodexEntry`]: apply a header-field delta to an entry, reversibly.

use crate::codex::CodexEntryId;
use crate::command::{Command, CommandError};
use crate::document::Document;

/// A delta over an entry's editable header fields. A `Some` field overwrites; a
/// `None` field is left unchanged. Lets one command type cover any subset of an
/// entry's text edits without a struct per field.
#[derive(Clone, Debug, Default)]
pub struct CodexEntryDelta {
    /// New display name.
    pub name: Option<String>,
    /// New short description.
    pub description: Option<String>,
    /// New lore description.
    pub lore: Option<String>,
    /// New visual description.
    pub visual_description: Option<String>,
    /// New tag list (replaces the existing tags wholesale).
    pub tags: Option<Vec<String>>,
}

impl CodexEntryDelta {
    /// An empty delta (changes nothing).
    pub fn new() -> Self {
        Self::default()
    }
}

/// The previous values of whatever fields the delta touched, captured on apply.
struct Restore {
    name: Option<String>,
    description: Option<String>,
    lore: Option<String>,
    visual_description: Option<String>,
    tags: Option<Vec<String>>,
}

/// Updates an entry's header text fields from a delta and restores the prior values
/// on undo.
pub struct UpdateCodexEntry {
    id: CodexEntryId,
    delta: Option<CodexEntryDelta>,
    restore: Option<Restore>,
}

impl UpdateCodexEntry {
    /// A command that will apply `delta` to the entry `id`.
    pub fn new(id: CodexEntryId, delta: CodexEntryDelta) -> Self {
        Self {
            id,
            delta: Some(delta),
            restore: None,
        }
    }
}

impl Command for UpdateCodexEntry {
    fn apply(&mut self, doc: &mut Document) -> Result<(), CommandError> {
        let delta = self.delta.take().ok_or(CommandError::InvalidState)?;
        let entry = doc.codex_mut().entry_mut(self.id).ok_or(CommandError::CodexEntryNotFound(self.id))?;
        let restore = Restore {
            name: delta.name.as_ref().map(|_| entry.name.clone()),
            description: delta.description.as_ref().map(|_| entry.description.clone()),
            lore: delta.lore.as_ref().map(|_| entry.lore.clone()),
            visual_description: delta.visual_description.as_ref().map(|_| entry.visual_description.clone()),
            tags: delta.tags.as_ref().map(|_| entry.tags.clone()),
        };
        if let Some(name) = delta.name {
            entry.name = name;
        }
        if let Some(description) = delta.description {
            entry.description = description;
        }
        if let Some(lore) = delta.lore {
            entry.lore = lore;
        }
        if let Some(visual_description) = delta.visual_description {
            entry.visual_description = visual_description;
        }
        if let Some(tags) = delta.tags {
            entry.tags = tags;
        }
        doc.bump_revision();
        self.restore = Some(restore);
        Ok(())
    }

    fn undo(&mut self, doc: &mut Document) -> Result<(), CommandError> {
        let restore = self.restore.take().ok_or(CommandError::InvalidState)?;
        let entry = doc.codex_mut().entry_mut(self.id).ok_or(CommandError::CodexEntryNotFound(self.id))?;
        // Rebuild the delta as we restore, so a redo re-applies the same change.
        let mut delta = CodexEntryDelta::new();
        if let Some(name) = restore.name {
            delta.name = Some(std::mem::replace(&mut entry.name, name));
        }
        if let Some(description) = restore.description {
            delta.description = Some(std::mem::replace(&mut entry.description, description));
        }
        if let Some(lore) = restore.lore {
            delta.lore = Some(std::mem::replace(&mut entry.lore, lore));
        }
        if let Some(visual_description) = restore.visual_description {
            delta.visual_description = Some(std::mem::replace(&mut entry.visual_description, visual_description));
        }
        if let Some(tags) = restore.tags {
            delta.tags = Some(std::mem::replace(&mut entry.tags, tags));
        }
        doc.bump_revision();
        self.delta = Some(delta);
        Ok(())
    }

    fn label_key(&self) -> &'static str {
        "command.codex.update_entry"
    }

    fn estimated_size_bytes(&self) -> usize {
        std::mem::size_of::<Self>()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::codex::{CodexHandle, EntryType};
    use crate::commands::{AddCodexEntry, CodexEntryProto};

    fn seed(doc: &mut Document, handle: &str) -> CodexEntryId {
        let mut add = AddCodexEntry::new(CodexEntryProto {
            handle: CodexHandle::new(handle).unwrap(),
            name: "Bit".to_owned(),
            entry_type: EntryType::Character,
        });
        add.apply(doc).unwrap();
        add.inserted_id().unwrap()
    }

    #[test]
    fn apply_updates_then_undo_restores() {
        let mut doc = Document::new();
        let id = seed(&mut doc, "bit");
        let mut delta = CodexEntryDelta::new();
        delta.name = Some("Bit the Mascot".to_owned());
        delta.description = Some("the canonical mascot".to_owned());
        let mut cmd = UpdateCodexEntry::new(id, delta);

        cmd.apply(&mut doc).unwrap();
        assert_eq!(doc.codex().entry(id).unwrap().name, "Bit the Mascot");
        assert_eq!(doc.codex().entry(id).unwrap().description, "the canonical mascot");

        cmd.undo(&mut doc).unwrap();
        assert_eq!(doc.codex().entry(id).unwrap().name, "Bit");
        assert_eq!(doc.codex().entry(id).unwrap().description, "");
    }

    #[test]
    fn redo_re_applies() {
        let mut doc = Document::new();
        let id = seed(&mut doc, "bit");
        let mut delta = CodexEntryDelta::new();
        delta.lore = Some("born in the catacombs".to_owned());
        let mut cmd = UpdateCodexEntry::new(id, delta);
        cmd.apply(&mut doc).unwrap();
        cmd.undo(&mut doc).unwrap();
        cmd.apply(&mut doc).unwrap();
        assert_eq!(doc.codex().entry(id).unwrap().lore, "born in the catacombs");
    }

    #[test]
    fn missing_entry_errors() {
        let mut doc = Document::new();
        let mut cmd = UpdateCodexEntry::new(CodexEntryId(99), CodexEntryDelta::new());
        assert!(matches!(cmd.apply(&mut doc), Err(CommandError::CodexEntryNotFound(_))));
    }
}
