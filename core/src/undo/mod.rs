//! Undo/redo command pattern.
//!
//! Every mutation to a [`crate::project::Project`] flows through a [`Command`]:
//! the command carries enough state to [`apply`][Command::apply] the
//! change forward and [`undo`][Command::undo] it backward. Commands are
//! collected in a branching [`History`] that supports unlimited depth
//! (subject to a configurable memory cap) and tree-structured redo.
//!
//! # Quick start
//!
//! ```rust
//! use pixhaus_core::undo::{Command, History, HistoryConfig};
//! use pixhaus_core::project::Project;
//!
//! struct RenameProject { old: String, new: String }
//!
//! impl Command for RenameProject {
//!     fn label(&self) -> &str { "rename project" }
//!
//!     fn apply(&mut self, p: &mut Project)
//!         -> Result<(), Box<dyn std::error::Error + Send + Sync>>
//!     {
//!         self.old = std::mem::replace(&mut p.metadata.name, self.new.clone());
//!         Ok(())
//!     }
//!
//!     fn undo(&mut self, p: &mut Project)
//!         -> Result<(), Box<dyn std::error::Error + Send + Sync>>
//!     {
//!         p.metadata.name = self.old.clone();
//!         Ok(())
//!     }
//! }
//!
//! let mut history = History::new();
//! let mut project = Project::new("untitled");
//!
//! history.push(
//!     Box::new(RenameProject { old: String::new(), new: "hero".into() }),
//!     &mut project,
//! ).unwrap();
//! assert_eq!(project.metadata.name, "hero");
//!
//! history.undo(&mut project).unwrap();
//! assert_eq!(project.metadata.name, "untitled");
//! ```
//!
//! # Branching
//!
//! Undoing and then pushing a new command creates a branch. The previous
//! redo path is preserved as a sibling branch; calling [`History::redo`]
//! continues down the most-recently visited branch.
//!
//! # Memory cap
//!
//! The stack evicts the oldest commands on the current undo path once the
//! total node count exceeds [`HistoryConfig::max_commands`] or the summed
//! [`Command::estimated_size_bytes`] exceeds [`HistoryConfig::max_bytes`].
//! Evicted nodes become tombstones; their child branches survive until
//! the child is also evicted.
//!
//! # Coalescing
//!
//! Commands that represent the same logical action (e.g. consecutive brush
//! ticks in a single stroke) can opt in to merging by implementing
//! [`Command::coalesce`]. When the top command returns
//! [`CoalesceResult::Merged`], the incoming command is discarded and the
//! top command is updated in place — one history entry covers the whole
//! stroke.

pub mod command;
pub mod error;
pub mod history;

pub use command::{CoalesceResult, Command};
pub use error::{Error, Result};
pub use history::{History, HistoryConfig};
