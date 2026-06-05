//! The mutation boundary: the [`Command`] trait.
//!
//! Every change to project state is a `Command` (bible 12). A command applies a
//! mutation, capturing enough to reverse it, and `undo` restores the prior state
//! exactly. Commands are testable without any UI and carry an i18n *key* for their
//! user-facing history label — never display text, which lives in `services`.

use thiserror::Error;

use crate::codex::{CodexEntryId, CodexFolderId, CoverageTemplateId};
use crate::document::Document;
use crate::ids::SpriteId;
use crate::pixel::PixelError;

/// An undoable mutation to a [`Document`].
///
/// A command is applied at most once before being undone, and re-applied after an
/// undo (redo). `apply` captures whatever `undo` needs; `undo` reclaims whatever a
/// later `apply` needs, so a command can cycle apply/undo/apply indefinitely.
pub trait Command: Send {
    /// Applies the mutation, capturing the state needed to [`undo`](Command::undo).
    ///
    /// # Errors
    /// Returns a [`CommandError`] if the document is not in the expected state.
    fn apply(&mut self, doc: &mut Document) -> Result<(), CommandError>;

    /// Reverses a prior [`apply`](Command::apply), restoring the document exactly.
    ///
    /// # Errors
    /// Returns a [`CommandError`] if called before `apply` or if the document was
    /// mutated out from under the command.
    fn undo(&mut self, doc: &mut Document) -> Result<(), CommandError>;

    /// Stable i18n key for the user-facing history label (e.g.
    /// `"command.apply_generated_asset"`). The shell resolves it to text; core never
    /// stores display strings.
    fn label_key(&self) -> &'static str;

    /// Estimated heap bytes of undo/redo data this command currently holds, for the
    /// memory-capped history. Pixels held by the live document don't count here —
    /// only what the command itself owns.
    fn estimated_size_bytes(&self) -> usize;
}

/// Why a [`Command`] could not apply or undo.
#[derive(Debug, Error)]
pub enum CommandError {
    /// A referenced sprite is not present in the document.
    #[error("sprite {0:?} not found")]
    SpriteNotFound(SpriteId),
    /// The command was applied or undone out of order (e.g. undo before apply).
    #[error("command applied or undone out of order")]
    InvalidState,
    /// A pixel-buffer invariant was violated while building or restoring data.
    #[error(transparent)]
    Pixel(#[from] PixelError),
    /// A referenced Codex entry is not present in the document.
    #[error("codex entry {0:?} not found")]
    CodexEntryNotFound(CodexEntryId),
    /// A Codex handle is already claimed by another entry.
    #[error("codex handle is already in use")]
    CodexHandleInUse,
    /// A facet of the entry that the command would change is locked against change.
    #[error("codex entry facet is locked against change")]
    CodexEntryLocked,
    /// A referenced Codex folder is not present in the document.
    #[error("codex folder {0:?} not found")]
    CodexFolderNotFound(CodexFolderId),
    /// Reparenting a folder would make it its own ancestor.
    #[error("codex folder reparent would create a cycle")]
    CodexFolderCycle,
    /// A referenced coverage template is not present in the Codex.
    #[error("coverage template {0:?} not found")]
    CoverageTemplateNotFound(CoverageTemplateId),
}
