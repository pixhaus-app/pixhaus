//! Shared builders for the in-crate command tests.
//!
//! Every Codex command test seeds a document with one or more entries before
//! exercising the command. The `seed` helper was copy-pasted into ~28 `#[cfg(test)]`
//! modules; that duplication drifts (the seeded `name` quietly diverged to "Bit",
//! the handle, or "x" across files) and re-typing it per file buries the one line
//! that varies. This module is the single source for those builders.
//!
//! `pub(crate)` and `#[cfg(test)]`: this is test scaffolding, never part of the
//! public API or a shipped binary. The crate's own command tests reach for it; no
//! external crate sees it.
//!
//! Behavior note: the seeded `name` is observable (some tests assert on it, e.g.
//! [`crate::commands::UpdateCodexEntry`] checks the post-update name against the
//! seed). So [`seed_entry`] takes the name explicitly and the convenience wrappers
//! pin the exact literal each former call site used — extracting the helper changed
//! no test's inputs.

use crate::codex::{CodexEntryId, CodexHandle, EntryType};
use crate::command::Command;
use crate::commands::{AddCodexEntry, CodexEntryProto};
use crate::document::Document;

/// Inserts one Codex entry and returns its id. The general builder behind the
/// convenience wrappers: callers that need a specific name or type spell all four
/// fields here so the seeded data stays explicit at the call site.
pub(crate) fn seed_entry(doc: &mut Document, handle: &str, name: &str, entry_type: EntryType) -> CodexEntryId {
    let mut add = AddCodexEntry::new(CodexEntryProto {
        handle: CodexHandle::new(handle).unwrap(),
        name: name.to_owned(),
        entry_type,
    });
    add.apply(doc).unwrap();
    add.inserted_id().unwrap()
}

/// Seeds the canonical mascot entry — handle "bit", name "Bit", a Character. The
/// fixed-fixture case used by tests that need one entry and don't care which.
pub(crate) fn seed_bit(doc: &mut Document) -> CodexEntryId {
    seed_entry(doc, "bit", "Bit", EntryType::Character)
}

/// Seeds a Character named "Bit" under the given handle. For tests that need a
/// specific handle (alias, rename, delete by handle) but a fixed name and type.
pub(crate) fn seed_handle(doc: &mut Document, handle: &str) -> CodexEntryId {
    seed_entry(doc, handle, "Bit", EntryType::Character)
}

/// Builds a validated [`CodexHandle`] from a string. The other line every
/// alias/rename test re-declared next to its `seed`.
pub(crate) fn handle(s: &str) -> CodexHandle {
    CodexHandle::new(s).unwrap()
}
