//! [`SetAnchor`]: add or replace an entry's anchor of a given kind, reversibly.
//!
//! At most one anchor of a given [`AnchorKind`] is kept per
//! entry: setting an anchor of a kind that already exists replaces it. Undo restores
//! whatever was there before — a prior anchor or its absence. [`RemoveAnchor`](super::RemoveAnchor) lives
//! in its own module.

use crate::codex::{Anchor, AnchorKind, CodexEntryId};
use crate::command::{Command, CommandError};
use crate::document::Document;

/// What occupied the target kind's slot before apply, captured so undo restores it.
enum PriorAnchor {
    /// Apply has not run yet.
    Unset,
    /// The slot was empty; apply pushed a new anchor.
    WasAbsent,
    /// The slot held this anchor; apply replaced it.
    WasPresent(Anchor),
}

/// Adds or replaces the anchor of a given kind on an entry; undo restores the prior
/// anchor of that kind (or its absence).
pub struct SetAnchor {
    id: CodexEntryId,
    /// The kind this command targets, kept so undo can find the applied anchor.
    kind: AnchorKind,
    anchor: Option<Anchor>,
    /// What the target slot held before apply.
    prev: PriorAnchor,
}

impl SetAnchor {
    /// A command that will set `anchor` on the entry `id`, replacing any existing
    /// anchor of the same kind.
    pub fn new(id: CodexEntryId, anchor: Anchor) -> Self {
        Self {
            id,
            kind: anchor.kind,
            anchor: Some(anchor),
            prev: PriorAnchor::Unset,
        }
    }
}

impl Command for SetAnchor {
    fn apply(&mut self, doc: &mut Document) -> Result<(), CommandError> {
        let anchor = self.anchor.take().ok_or(CommandError::InvalidState)?;
        let entry = doc.codex_mut().entry_mut(self.id).ok_or(CommandError::CodexEntryNotFound(self.id))?;
        let prev = if let Some(pos) = entry.anchor_position(self.kind) {
            PriorAnchor::WasPresent(std::mem::replace(&mut entry.anchors[pos], anchor))
        } else {
            entry.anchors.push(anchor);
            PriorAnchor::WasAbsent
        };
        doc.bump_revision();
        self.prev = prev;
        Ok(())
    }

    fn undo(&mut self, doc: &mut Document) -> Result<(), CommandError> {
        let prev = std::mem::replace(&mut self.prev, PriorAnchor::Unset);
        let entry = doc.codex_mut().entry_mut(self.id).ok_or(CommandError::CodexEntryNotFound(self.id))?;
        let pos = entry.anchor_position(self.kind).ok_or(CommandError::InvalidState)?;
        let applied = match prev {
            // There was a prior anchor of this kind: put it back, reclaim what we set.
            PriorAnchor::WasPresent(prior) => std::mem::replace(&mut entry.anchors[pos], prior),
            // Apply had pushed a new anchor: remove it and reclaim it.
            PriorAnchor::WasAbsent => entry.anchors.remove(pos),
            // Undo before apply.
            PriorAnchor::Unset => return Err(CommandError::InvalidState),
        };
        doc.bump_revision();
        self.anchor = Some(applied);
        Ok(())
    }

    fn label_key(&self) -> &'static str {
        "command.codex.set_anchor"
    }

    fn estimated_size_bytes(&self) -> usize {
        std::mem::size_of::<Self>()
            + self
                .anchor
                .as_ref()
                .map_or(0, |a| a.statement.len() + a.structured.iter().map(String::len).sum::<usize>())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::codex::AnchorStrength;
    use crate::test_support::seed_bit;

    #[test]
    fn set_new_anchor_then_undo_removes_it() {
        let mut doc = Document::new();
        let id = seed_bit(&mut doc);
        let mut cmd = SetAnchor::new(id, Anchor::new(AnchorKind::Visual, AnchorStrength::Strong, "round head"));
        cmd.apply(&mut doc).unwrap();
        assert_eq!(doc.codex().entry(id).unwrap().anchors.len(), 1);
        assert_eq!(doc.codex().entry(id).unwrap().anchors[0].strength, AnchorStrength::Strong);

        cmd.undo(&mut doc).unwrap();
        assert_eq!(doc.codex().entry(id).unwrap().anchors.len(), 0);
    }

    #[test]
    fn set_over_existing_anchor_replaces_and_undo_restores() {
        let mut doc = Document::new();
        let id = seed_bit(&mut doc);
        SetAnchor::new(id, Anchor::new(AnchorKind::Visual, AnchorStrength::Normal, "old"))
            .apply(&mut doc)
            .unwrap();
        let mut cmd = SetAnchor::new(id, Anchor::new(AnchorKind::Visual, AnchorStrength::Locked, "new"));

        cmd.apply(&mut doc).unwrap();
        let anchors = &doc.codex().entry(id).unwrap().anchors;
        assert_eq!(anchors.len(), 1);
        assert_eq!(anchors[0].statement, "new");
        assert_eq!(anchors[0].strength, AnchorStrength::Locked);

        cmd.undo(&mut doc).unwrap();
        let anchors = &doc.codex().entry(id).unwrap().anchors;
        assert_eq!(anchors.len(), 1);
        assert_eq!(anchors[0].statement, "old");
        assert_eq!(anchors[0].strength, AnchorStrength::Normal);
    }

    #[test]
    fn redo_re_applies() {
        let mut doc = Document::new();
        let id = seed_bit(&mut doc);
        let mut cmd = SetAnchor::new(id, Anchor::new(AnchorKind::Palette, AnchorStrength::Strong, "keep blues"));
        cmd.apply(&mut doc).unwrap();
        cmd.undo(&mut doc).unwrap();
        cmd.apply(&mut doc).unwrap();
        assert_eq!(doc.codex().entry(id).unwrap().anchors.len(), 1);
        assert_eq!(doc.codex().entry(id).unwrap().anchors[0].statement, "keep blues");
    }

    #[test]
    fn missing_entry_errors() {
        let mut doc = Document::new();
        let mut cmd = SetAnchor::new(CodexEntryId(9), Anchor::new(AnchorKind::Visual, AnchorStrength::Normal, "x"));
        assert!(matches!(cmd.apply(&mut doc), Err(CommandError::CodexEntryNotFound(_))));
    }
}
