//! Undo/redo command pattern, generic over the mutation target.
//!
//! Every reversible mutation is expressed as a [`Command`] over a target
//! `T`: the command carries enough state to [`apply`][Command::apply] the
//! change forward and [`undo`][Command::undo] it backward. Commands are
//! collected in a branching [`History`] that supports unlimited depth
//! (subject to a configurable memory cap) and tree-structured redo.
//!
//! `T` is the type being mutated. `core` exercises the machinery with
//! `History<Project>` in tests; the shell uses `History<DocumentStore>`,
//! because v2 keeps pixel buffers in the shell document rather than in the
//! project, so a pixel-edit command needs the document, not just the
//! project, to apply and reverse.
//!
//! # Branching
//!
//! Undoing and then pushing a new command creates a branch. The previous
//! redo path is preserved as a sibling branch; calling [`History::redo`]
//! continues down the most-recently visited branch.
//!
//! # Coalescing
//!
//! Commands that represent the same logical action (e.g. consecutive brush
//! ticks in one stroke) can opt in to merging by implementing
//! [`Command::coalesce`].

pub mod command;
pub mod error;
pub mod history;

pub use command::{CoalesceResult, Command};
pub use error::{CommandError, CommandResult, Error, Result};
pub use history::{History, HistoryConfig};
