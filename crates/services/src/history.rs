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
    /// This is the single mutation entry point for project state (bible 12.1). Use it
    /// when the caller needs nothing back from the command; when the caller must read a
    /// value the command computed during apply (e.g. a freshly minted id), reach for
    /// [`execute_returning`](Self::execute_returning) instead of re-deriving it.
    ///
    /// # Errors
    /// Returns [`ServiceError::Command`] if the command fails to apply; the document
    /// is left as the command's own rollback leaves it.
    #[tracing::instrument(skip_all, fields(command = cmd.label_key()))]
    pub fn execute(&mut self, doc: &mut Document, mut cmd: Box<dyn Command>) -> Result<(), ServiceError> {
        cmd.apply(doc)?;
        self.record(cmd);
        Ok(())
    }

    /// Applies a concrete `cmd` to `doc`, records it for undo, and returns a value read
    /// from the executed command via `read` — run after apply, so it sees whatever the
    /// command captured (an inserted id, an assigned handle).
    ///
    /// This is the typed counterpart to [`execute`](Self::execute). The history stores
    /// commands as `Box<dyn Command>` for undo, which erases the concrete type and so
    /// hides per-command accessors like `inserted_id`. Taking the command by its real
    /// type `C` and reading from it *before* it is boxed lets the caller pull that value
    /// out directly, instead of recovering it by diffing document state before and after
    /// (the old set-diff / handle-re-resolve workaround in the Codex intent handlers).
    /// `read` runs only on success; on an apply error the command is dropped and the
    /// closure never runs, matching `execute`'s no-record-on-failure contract.
    ///
    /// # Errors
    /// Returns [`ServiceError::Command`] if the command fails to apply; the document
    /// is left as the command's own rollback leaves it and nothing is recorded.
    #[tracing::instrument(skip_all, fields(command = cmd.label_key()))]
    pub fn execute_returning<C, R>(&mut self, doc: &mut Document, mut cmd: C, read: impl FnOnce(&C) -> R) -> Result<R, ServiceError>
    where
        C: Command + 'static,
    {
        cmd.apply(doc)?;
        // Read the typed result while the concrete type is still in hand; once boxed
        // for the undo stack the type is erased and the accessor is gone.
        let result = read(&cmd);
        self.record(Box::new(cmd));
        Ok(result)
    }

    /// Records an already-applied command on the undo stack: clears the redo path,
    /// accounts the command's memory, and evicts down to the cap. Shared by `execute`
    /// and `execute_returning` so both follow the identical post-apply bookkeeping.
    fn record(&mut self, cmd: Box<dyn Command>) {
        // A fresh edit invalidates any redo path; its commands stop counting too.
        for undone in self.undone.drain(..) {
            self.bytes = self.bytes.saturating_sub(undone.estimated_size_bytes());
        }
        self.bytes = self.bytes.saturating_add(cmd.estimated_size_bytes());
        self.done.push(cmd);
        self.evict_over_cap();
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
    use pixhaus_core::{AddCodexEntry, ApplyGeneratedAsset, CodexEntryProto, CodexHandle, CommandError, EntryType, composite_sprite};

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
        // With a command on the redo stack, the redo label reports its top.
        assert_eq!(history.next_redo_label_key(), Some("command.apply_generated_asset"));

        history.redo(&mut doc).unwrap();
        assert_eq!(doc.sprites().len(), 1);
        // Nothing left to redo once it is re-applied.
        assert_eq!(history.next_redo_label_key(), None);
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
    fn execute_returning_hands_back_the_executed_commands_minted_id() {
        // The typed path must surface the id the command minted during apply, and the
        // returned id must be the one actually inserted into the document — proving the
        // caller no longer needs to re-resolve it by handle or diff the entry set.
        let mut doc = Document::new();
        let mut history = History::new();
        let handle = CodexHandle::new("hero").unwrap();
        let proto = CodexEntryProto {
            handle: handle.clone(),
            name: "Hero".to_owned(),
            entry_type: EntryType::Character,
        };

        let minted = history
            .execute_returning(&mut doc, AddCodexEntry::new(proto), AddCodexEntry::inserted_id)
            .unwrap()
            .expect("apply assigns an inserted id");

        // The returned id resolves to the entry that was actually inserted.
        assert_eq!(doc.codex().resolve_handle(&handle), Some(minted));
        assert!(history.can_undo());

        // The command stayed on the undo stack: undo removes exactly that entry.
        history.undo(&mut doc).unwrap();
        assert_eq!(doc.codex().resolve_handle(&handle), None);
    }

    #[test]
    fn execute_returning_records_nothing_on_apply_failure() {
        // A failing apply must not record the command, and the read closure must not run
        // — same no-record-on-failure contract as `execute`.
        let mut doc = Document::new();
        let mut history = History::new();
        let handle = CodexHandle::new("hero").unwrap();
        history
            .execute(
                &mut doc,
                Box::new(AddCodexEntry::new(CodexEntryProto {
                    handle: handle.clone(),
                    name: "Hero".to_owned(),
                    entry_type: EntryType::Character,
                })),
            )
            .unwrap();

        // A second entry claiming the same handle fails to apply.
        let mut closure_ran = false;
        let result = history.execute_returning(
            &mut doc,
            AddCodexEntry::new(CodexEntryProto {
                handle,
                name: "Hero II".to_owned(),
                entry_type: EntryType::Character,
            }),
            |cmd| {
                closure_ran = true;
                cmd.inserted_id()
            },
        );
        assert!(matches!(result, Err(ServiceError::Command(_))));
        assert!(!closure_ran, "the read closure must not run when apply fails");
        // Only the first, successful command is on the undo stack.
        assert_eq!(doc.codex().entries().len(), 1);
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
