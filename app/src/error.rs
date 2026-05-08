//! Typed errors for Tauri command results.
//!
//! Bedrock B4's acceptance criteria mandate typed errors crossing the
//! IPC boundary — never raw strings. [`AppCommandError`] is the single
//! shape every command rejects with; the TS side imports the matching
//! generated type from `ui/src/lib/types/AppCommandError.ts`.
//!
//! # Wire format
//!
//! `serde` serialises the enum as a discriminated union with shape
//! `{ "kind": "<variant>", "message": "<human-readable>" }`. The
//! variant tag drives programmatic dispatch (e.g. "show the
//! reconnect dialog on `NoActiveProject`"); the message is for
//! diagnostics — UI surfaces should localise off `kind` rather than
//! string-matching the message.

use pixhaus_core::undo::Error as UndoError;
use serde::{Deserialize, Serialize};
use thiserror::Error;
use tokio::task::JoinError;
use ts_rs::TS;

/// Errors surfaced by Tauri commands in `pixhaus-app`.
///
/// Every command in [`crate::commands`] returns
/// `Result<T, AppCommandError>` so the IPC contract is uniform across
/// the catalog. Variants are tagged on the wire (`kind`) so the UI can
/// switch on them without parsing the diagnostic string.
#[derive(Clone, Debug, Eq, PartialEq, Error, Serialize, Deserialize, TS)]
#[serde(tag = "kind", content = "message", rename_all = "snake_case")]
#[ts(export)]
pub enum AppCommandError {
    /// No project is currently open. The host should prompt the user
    /// to create or open one.
    #[error("no active project")]
    NoActiveProject,

    /// An entity was looked up by ID and not found.
    #[error("{entity} {id} not found")]
    NotFound {
        /// Entity kind: `"sprite"`, `"layer"`, `"frame"`, `"palette"`,
        /// `"tileset"`, `"slice"`, `"frame_tag"`, etc.
        entity: String,
        /// The ID the caller supplied.
        id: u64,
    },

    /// A name-keyed entity was looked up and not found.
    #[error("{entity} {name:?} not found")]
    NotFoundByName {
        /// Entity kind: `"frame_tag"`, `"animation"`, etc.
        entity: String,
        /// The name the caller supplied.
        name: String,
    },

    /// An index or position was outside the valid range.
    #[error("{detail}")]
    OutOfRange {
        /// What was out of range, in plain text.
        detail: String,
    },

    /// An entity collides with an existing one (duplicate name, etc).
    #[error("{detail}")]
    Conflict {
        /// What conflicted, in plain text.
        detail: String,
    },

    /// The command is a stub awaiting a later stream's implementation.
    #[error("not yet implemented: requires {stream}")]
    Unimplemented {
        /// The stream that will implement this command (e.g. "S01",
        /// "S06"). Lets the UI surface a "coming soon" message keyed
        /// to the dependency.
        stream: String,
    },

    /// Argument validation failure (malformed input, mutually exclusive
    /// fields set, etc.).
    #[error("{detail}")]
    Validation {
        /// What was invalid, in plain text.
        detail: String,
    },

    /// `undo` was called but the history is at the root state.
    ///
    /// Distinct from [`Self::Validation`] so the UI can disable the
    /// undo menu item by switching on `kind` rather than parsing a
    /// localised message.
    #[error("nothing to undo")]
    NothingToUndo,

    /// `redo` was called but no redo branch exists at the current node.
    #[error("nothing to redo")]
    NothingToRedo,

    /// The undo history was poisoned by a prior failed `apply`/`undo`,
    /// or a command's `apply`/`undo` failed during this operation.
    ///
    /// The project state may be inconsistent. The host should surface
    /// "history corrupted, please reload" and disable further edits
    /// until a fresh project is loaded.
    #[error("history corrupted: {detail}")]
    HistoryCorrupted {
        /// Human-readable description of the underlying failure (the
        /// command label and the wrapped error from
        /// `core::undo::Error`).
        detail: String,
    },

    /// An AI verb invocation failed.
    ///
    /// Covers validation errors (bad input schema), capability errors
    /// (no backend satisfies the verb's requirements), and runtime
    /// errors returned by the verb itself.
    #[error("verb error: {message}")]
    VerbError {
        /// Human-readable error from the verb runtime.
        message: String,
    },

    /// A write operation targeted a locked layer (or a layer whose
    /// ancestor group is locked). The UI surfaces this with a toast
    /// rather than a modal — paint attempts on locked layers are
    /// expected user noise, not exceptional state.
    #[error("layer {layer_id} is locked")]
    LayerLocked {
        /// The layer the caller targeted. May be a child whose
        /// containing group is locked rather than the layer itself.
        layer_id: u32,
    },
}

/// Crate-local result alias.
pub type CommandResult<T> = std::result::Result<T, AppCommandError>;

/// Maps `core::undo::Error` onto the IPC error contract.
///
/// `NothingToUndo` / `NothingToRedo` keep their own kinds so the UI can
/// disable menu items via `kind`. `CommandFailed` and `Poisoned` collapse
/// to `HistoryCorrupted` — both indicate the project state may no longer
/// be trustworthy. Anything else collapses to `Validation`.
impl From<UndoError> for AppCommandError {
    fn from(err: UndoError) -> Self {
        match err {
            UndoError::NothingToUndo => AppCommandError::NothingToUndo,
            UndoError::NothingToRedo => AppCommandError::NothingToRedo,
            UndoError::CommandFailed { .. } | UndoError::Poisoned { .. } => {
                AppCommandError::HistoryCorrupted {
                    detail: err.to_string(),
                }
            }
            other => AppCommandError::Validation {
                detail: other.to_string(),
            },
        }
    }
}

/// Maps a `tokio::task::JoinError` from a `spawn_blocking` to the IPC
/// error contract. `op` names the operation for diagnostics.
#[must_use]
pub fn join_error_to_command_error(op: &str, err: &JoinError) -> AppCommandError {
    let detail = if err.is_panic() {
        format!("{op} task panicked")
    } else if err.is_cancelled() {
        format!("{op} task cancelled (runtime shutdown)")
    } else {
        format!("{op} task did not complete: {err}")
    };
    AppCommandError::Validation { detail }
}

/// Maps `pixhaus_io::Error` onto the IPC error contract.
///
/// Every variant currently lands on [`AppCommandError::Validation`] —
/// the user-facing message preserves the source's `Display`, which
/// already includes the file/format context (magic, version, etc.).
/// A future cut may split into dedicated `IoFailure` /
/// `IncompatibleFile` variants so the UI can branch on them, but the
/// single mapping keeps every fallible IO command (`project_open`,
/// later `project_save`, future palette load/save) consistent today.
impl From<pixhaus_io::Error> for AppCommandError {
    fn from(err: pixhaus_io::Error) -> Self {
        AppCommandError::Validation {
            detail: err.to_string(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn no_active_project_round_trips() {
        let err = AppCommandError::NoActiveProject;
        let json = serde_json::to_string(&err).expect("serialize");
        // `kind` only — no `message` payload because the unit variant
        // serialises with just the tag.
        assert_eq!(json, r#"{"kind":"no_active_project"}"#);
        let back: AppCommandError = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(back, err);
    }

    #[test]
    fn not_found_round_trips() {
        let err = AppCommandError::NotFound {
            entity: "sprite".into(),
            id: 42,
        };
        let json = serde_json::to_string(&err).expect("serialize");
        let back: AppCommandError = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(back, err);
    }

    #[test]
    fn unimplemented_carries_stream_id() {
        let err = AppCommandError::Unimplemented {
            stream: "S01".into(),
        };
        assert_eq!(err.to_string(), "not yet implemented: requires S01");
    }

    #[test]
    fn layer_locked_round_trips() {
        let err = AppCommandError::LayerLocked { layer_id: 42 };
        assert_eq!(err.to_string(), "layer 42 is locked");
        let json = serde_json::to_string(&err).expect("serialize");
        let back: AppCommandError = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(back, err);
    }
}
