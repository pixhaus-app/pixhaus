//! The command executor and the undo/redo stack.
//!
//! [`History`] is the one entry point for mutating a [`Document`]: a command is
//! applied through [`History::execute`], which records it for undo. The stack is
//! linear (no branching) and memory-capped across both the undo and redo stacks —
//! when the command data they hold exceeds the cap, redo entries are shed first and
//! then the oldest undo entries (the action stays done, it just falls out of the undo
//! window). Branching undo is a documented later refinement.

use pixhaus_core::{Command, Document};

use crate::error::ServiceError;

/// Default cap on history memory: 256 MiB of command-held undo/redo data.
const DEFAULT_CAP_BYTES: usize = 256 * 1024 * 1024;

/// The command executor plus a linear, memory-capped undo/redo stack over one
/// [`Document`].
pub struct History {
    /// Applied commands, oldest first; the top is the next undo target.
    done: Vec<Box<dyn Command>>,
    /// Undone commands, available to redo; cleared on a fresh `execute`.
    undone: Vec<Box<dyn Command>>,
    /// Running sum of `estimated_size_bytes` over both `done` and `undone` — the total
    /// command memory the history holds — measured against the cap.
    bytes: usize,
    /// Eviction threshold for the total command memory held across undo and redo.
    cap_bytes: usize,
}

impl Default for History {
    fn default() -> Self {
        Self::with_cap(DEFAULT_CAP_BYTES)
    }
}

impl History {
    /// A history with the default memory cap.
    pub fn new() -> Self {
        Self::default()
    }

    /// A history with an explicit memory cap (bytes of undo data to retain).
    pub fn with_cap(cap_bytes: usize) -> Self {
        Self {
            done: Vec::new(),
            undone: Vec::new(),
            bytes: 0,
            cap_bytes,
        }
    }

    /// Applies `cmd` to `doc` and records it for undo, clearing the redo stack.
    ///
    /// This is the single mutation entry point for project state (bible 12.1).
    ///
    /// # Errors
    /// Returns [`ServiceError::Command`] if the command fails to apply; the document
    /// is left as the command's own rollback leaves it.
    #[tracing::instrument(skip_all, fields(command = cmd.label_key()))]
    pub fn execute(&mut self, doc: &mut Document, mut cmd: Box<dyn Command>) -> Result<(), ServiceError> {
        cmd.apply(doc)?;
        // A fresh edit invalidates any redo path; its commands stop counting too.
        for undone in self.undone.drain(..) {
            self.bytes = self.bytes.saturating_sub(undone.estimated_size_bytes());
        }
        self.bytes = self.bytes.saturating_add(cmd.estimated_size_bytes());
        self.done.push(cmd);
        self.evict_over_cap();
        Ok(())
    }

    /// Undoes the most recent command, moving it to the redo stack.
    ///
    /// # Errors
    /// Returns [`ServiceError::NothingToUndo`] if the undo stack is empty, or
    /// [`ServiceError::Command`] if the command's `undo` fails.
    pub fn undo(&mut self, doc: &mut Document) -> Result<(), ServiceError> {
        let mut cmd = self.done.pop().ok_or(ServiceError::NothingToUndo)?;
        cmd.undo(doc)?;
        // The command stays resident on the redo stack, so the total is unchanged — the
        // cap counts both stacks (see `evict_over_cap`).
        self.undone.push(cmd);
        Ok(())
    }

    /// Re-applies the most recently undone command, moving it back to the undo stack.
    ///
    /// # Errors
    /// Returns [`ServiceError::NothingToRedo`] if the redo stack is empty, or
    /// [`ServiceError::Command`] if the command's `apply` fails.
    pub fn redo(&mut self, doc: &mut Document) -> Result<(), ServiceError> {
        let mut cmd = self.undone.pop().ok_or(ServiceError::NothingToRedo)?;
        cmd.apply(doc)?;
        // Moving back onto the undo stack leaves the total unchanged.
        self.done.push(cmd);
        Ok(())
    }

    /// Whether there is a command to undo.
    pub fn can_undo(&self) -> bool {
        !self.done.is_empty()
    }

    /// Whether there is a command to redo.
    pub fn can_redo(&self) -> bool {
        !self.undone.is_empty()
    }

    /// The i18n key of the next undo target's label, for an "Undo X" menu item.
    pub fn next_undo_label_key(&self) -> Option<&'static str> {
        self.done.last().map(|c| c.label_key())
    }

    /// The i18n key of the next redo target's label.
    pub fn next_redo_label_key(&self) -> Option<&'static str> {
        self.undone.last().map(|c| c.label_key())
    }

    /// Sheds held command memory until under the cap. Redo entries go first (the most
    /// expendable under memory pressure, oldest end first), then the oldest undo
    /// entries; the most recent undo step is always kept.
    fn evict_over_cap(&mut self) {
        while self.bytes > self.cap_bytes {
            if !self.undone.is_empty() {
                let evicted = self.undone.remove(0);
                self.bytes = self.bytes.saturating_sub(evicted.estimated_size_bytes());
            } else if self.done.len() > 1 {
                let evicted = self.done.remove(0);
                self.bytes = self.bytes.saturating_sub(evicted.estimated_size_bytes());
            } else {
                break;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pixhaus_core::{ApplyGeneratedAsset, CommandError, composite_sprite};

    fn red_1x1() -> Box<dyn Command> {
        Box::new(ApplyGeneratedAsset::new("r".to_owned(), 1, 1, 4, vec![255, 0, 0, 255]))
    }

    #[test]
    fn execute_then_undo_reverts_the_document() {
        let mut doc = Document::new();
        let mut history = History::new();

        history.execute(&mut doc, red_1x1()).unwrap();
        assert!(history.can_undo());
        assert_eq!(doc.sprites().len(), 1);
        let id = doc.active_sprite().unwrap();
        assert_eq!(composite_sprite(&doc, id).unwrap().as_bytes(), &[255, 0, 0, 255]);

        history.undo(&mut doc).unwrap();
        assert!(!history.can_undo());
        assert!(history.can_redo());
        assert_eq!(doc.sprites().len(), 0);

        history.redo(&mut doc).unwrap();
        assert_eq!(doc.sprites().len(), 1);
    }

    #[test]
    fn fresh_execute_clears_the_redo_stack() {
        let mut doc = Document::new();
        let mut history = History::new();
        history.execute(&mut doc, red_1x1()).unwrap();
        history.undo(&mut doc).unwrap();
        assert!(history.can_redo());

        history.execute(&mut doc, red_1x1()).unwrap();
        assert!(!history.can_redo());
    }

    #[test]
    fn undo_on_empty_history_errors() {
        let mut doc = Document::new();
        let mut history = History::new();
        assert!(matches!(history.undo(&mut doc), Err(ServiceError::NothingToUndo)));
        assert!(matches!(history.redo(&mut doc), Err(ServiceError::NothingToRedo)));
    }

    #[test]
    fn next_undo_label_key_reports_the_top() {
        let mut doc = Document::new();
        let mut history = History::new();
        assert_eq!(history.next_undo_label_key(), None);
        history.execute(&mut doc, red_1x1()).unwrap();
        assert_eq!(history.next_undo_label_key(), Some("command.apply_generated_asset"));
    }

    // A no-op command with a controllable held size, to test the memory cap in
    // isolation from any real command's accounting.
    struct Heavy {
        bytes: usize,
    }

    impl Command for Heavy {
        fn apply(&mut self, _doc: &mut Document) -> Result<(), CommandError> {
            Ok(())
        }
        fn undo(&mut self, _doc: &mut Document) -> Result<(), CommandError> {
            Ok(())
        }
        fn label_key(&self) -> &'static str {
            "command.test.heavy"
        }
        fn estimated_size_bytes(&self) -> usize {
            self.bytes
        }
    }

    #[test]
    fn cap_evicts_oldest_to_stay_under_budget() {
        let mut doc = Document::new();
        let mut history = History::with_cap(100);
        for _ in 0..5 {
            history.execute(&mut doc, Box::new(Heavy { bytes: 40 })).unwrap();
        }
        assert!(history.bytes <= 100, "held {} bytes", history.bytes);
        // The most recent edit is always retained.
        assert!(history.can_undo());
    }

    #[test]
    fn redo_stack_counts_against_the_cap() {
        // Two heavy commands fit (80 <= 100). Undoing both moves them to the redo
        // stack, where they still hold memory — so the total stays accounted, not
        // reset to zero (the cap covers both stacks).
        let mut doc = Document::new();
        let mut history = History::with_cap(100);
        history.execute(&mut doc, Box::new(Heavy { bytes: 40 })).unwrap();
        history.execute(&mut doc, Box::new(Heavy { bytes: 40 })).unwrap();
        history.undo(&mut doc).unwrap();
        history.undo(&mut doc).unwrap();
        assert_eq!(history.bytes, 80, "redo data stays counted against the cap");
        assert!(history.can_redo());
    }
}
