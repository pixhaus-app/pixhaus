//! [`RemoveAnchor`]: remove an entry's anchor of a given kind, reversibly.

use crate::codex::{Anchor, AnchorKind, CodexEntryId};
use crate::command::{Command, CommandError};
use crate::document::Document;

/// Removes the anchor of a given kind from an entry; undo restores it at its prior
/// position. A no-op apply (no anchor of that kind) is an error so the caller does not
/// silently undo nothing.
pub struct RemoveAnchor {
    id: CodexEntryId,
    kind: AnchorKind,
    /// The removed anchor and its index, captured on apply for undo.
    removed: Option<(usize, Anchor)>,
}

impl RemoveAnchor {
    /// A command that will remove the `kind` anchor from the entry `id`.
    pub fn new(id: CodexEntryId, kind: AnchorKind) -> Self {
        Self { id, kind, removed: None }
    }
}

impl Command for RemoveAnchor {
    fn apply(&mut self, doc: &mut Document) -> Result<(), CommandError> {
        let entry = doc.codex_mut().entry_mut(self.id).ok_or(CommandError::CodexEntryNotFound(self.id))?;
        let pos = entry.anchor_position(self.kind).ok_or(CommandError::InvalidState)?;
        let anchor = entry.anchors.remove(pos);
        doc.bump_revision();
        self.removed = Some((pos, anchor));
        Ok(())
    }

    fn undo(&mut self, doc: &mut Document) -> Result<(), CommandError> {
        let (pos, anchor) = self.removed.take().ok_or(CommandError::InvalidState)?;
        let entry = doc.codex_mut().entry_mut(self.id).ok_or(CommandError::CodexEntryNotFound(self.id))?;
        let pos = pos.min(entry.anchors.len());
        entry.anchors.insert(pos, anchor);
        doc.bump_revision();
        Ok(())
    }

    fn label_key(&self) -> &'static str {
        "command.codex.remove_anchor"
    }

    fn estimated_size_bytes(&self) -> usize {
        std::mem::size_of::<Self>()
            + self
                .removed
                .as_ref()
                .map_or(0, |(_, a)| a.statement.len() + a.structured.iter().map(String::len).sum::<usize>())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::codex::{Anchor, AnchorStrength, CodexHandle, EntryType};
    use crate::commands::{AddCodexEntry, CodexEntryProto, SetAnchor};

    fn seed_with_anchor(doc: &mut Document) -> CodexEntryId {
        let mut add = AddCodexEntry::new(CodexEntryProto {
            handle: CodexHandle::new("bit").unwrap(),
            name: "Bit".to_owned(),
            entry_type: EntryType::Character,
        });
        add.apply(doc).unwrap();
        let id = add.inserted_id().unwrap();
        SetAnchor::new(id, Anchor::new(AnchorKind::Visual, AnchorStrength::Strong, "round head"))
            .apply(doc)
            .unwrap();
        id
    }

    #[test]
    fn apply_removes_then_undo_restores() {
        let mut doc = Document::new();
        let id = seed_with_anchor(&mut doc);
        let mut cmd = RemoveAnchor::new(id, AnchorKind::Visual);

        cmd.apply(&mut doc).unwrap();
        assert!(doc.codex().entry(id).unwrap().anchors.is_empty());

        cmd.undo(&mut doc).unwrap();
        let anchors = &doc.codex().entry(id).unwrap().anchors;
        assert_eq!(anchors.len(), 1);
        assert_eq!(anchors[0].statement, "round head");
    }

    #[test]
    fn removing_absent_anchor_errors() {
        let mut doc = Document::new();
        let id = seed_with_anchor(&mut doc);
        let mut cmd = RemoveAnchor::new(id, AnchorKind::Palette);
        assert!(matches!(cmd.apply(&mut doc), Err(CommandError::InvalidState)));
    }
}
