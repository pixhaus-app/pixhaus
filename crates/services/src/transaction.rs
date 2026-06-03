//! [`Transaction`]: group several commands into one undo step.
//!
//! A transaction is itself a [`Command`], so [`History`](crate::History) stores a
//! group exactly like a single command — one conceptual action, one undo (bible
//! 12.3). Children apply in order and undo in reverse. If a child fails to apply,
//! the children that already applied are rolled back so the document is left
//! unchanged.

use pixhaus_core::{Command, CommandError, Document};

/// Groups commands so the user sees one undo step for one conceptual action.
pub struct Transaction {
    label_key: &'static str,
    children: Vec<Box<dyn Command>>,
}

impl Transaction {
    /// An empty transaction labelled by the i18n `label_key`.
    pub fn new(label_key: &'static str) -> Self {
        Self {
            label_key,
            children: Vec::new(),
        }
    }

    /// Adds a command to the group (applied in push order).
    pub fn push(&mut self, command: Box<dyn Command>) {
        self.children.push(command);
    }

    /// Whether the transaction has no children.
    pub fn is_empty(&self) -> bool {
        self.children.is_empty()
    }

    /// The number of grouped commands.
    pub fn len(&self) -> usize {
        self.children.len()
    }
}

impl Command for Transaction {
    fn apply(&mut self, doc: &mut Document) -> Result<(), CommandError> {
        for index in 0..self.children.len() {
            if let Err(error) = self.children[index].apply(doc) {
                // Roll back the children that already applied, newest-first.
                for prev in self.children[..index].iter_mut().rev() {
                    let _ = prev.undo(doc);
                }
                return Err(error);
            }
        }
        Ok(())
    }

    fn undo(&mut self, doc: &mut Document) -> Result<(), CommandError> {
        for child in self.children.iter_mut().rev() {
            child.undo(doc)?;
        }
        Ok(())
    }

    fn label_key(&self) -> &'static str {
        self.label_key
    }

    fn estimated_size_bytes(&self) -> usize {
        self.children.iter().map(|c| c.estimated_size_bytes()).sum::<usize>() + std::mem::size_of::<Self>()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pixhaus_core::commands::{AddSprite, SpriteProto};

    fn add(name: &str) -> Box<dyn Command> {
        Box::new(AddSprite::new(SpriteProto {
            name: name.to_owned(),
            width: 2,
            height: 2,
        }))
    }

    #[test]
    fn two_commands_apply_and_undo_as_one_step() {
        let mut doc = Document::new();
        let mut tx = Transaction::new("command.test.group");
        tx.push(add("a"));
        tx.push(add("b"));
        assert_eq!(tx.len(), 2);
        assert!(!tx.is_empty());

        tx.apply(&mut doc).unwrap();
        assert_eq!(doc.sprites().len(), 2);

        // One undo removes both children.
        tx.undo(&mut doc).unwrap();
        assert_eq!(doc.sprites().len(), 0);
    }

    #[test]
    fn label_key_is_the_group_key() {
        let tx = Transaction::new("command.test.group");
        assert_eq!(tx.label_key(), "command.test.group");
    }

    /// A command that always fails to apply, to drive the rollback path.
    struct FailingApply;

    impl Command for FailingApply {
        fn apply(&mut self, _doc: &mut Document) -> Result<(), CommandError> {
            Err(CommandError::InvalidState)
        }
        fn undo(&mut self, _doc: &mut Document) -> Result<(), CommandError> {
            Ok(())
        }
        fn label_key(&self) -> &'static str {
            "command.test.failing"
        }
        fn estimated_size_bytes(&self) -> usize {
            0
        }
    }

    #[test]
    fn a_failed_child_rolls_back_the_applied_ones() {
        let mut doc = Document::new();
        let mut tx = Transaction::new("command.test.group");
        tx.push(add("a"));
        tx.push(Box::new(FailingApply));
        tx.push(add("b"));

        // The first child applies, the second fails: the transaction reports the error
        // and rolls the first one back, leaving the document untouched.
        assert!(matches!(tx.apply(&mut doc), Err(CommandError::InvalidState)));
        assert_eq!(doc.sprites().len(), 0, "the applied child was rolled back");
    }
}
