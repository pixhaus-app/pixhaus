//! Pixhaus platform layer: the OS-facing edges.
//!
//! `platform` owns native dialogs, recent-files tracking, OS settings paths, GPU
//! capability detection, and external-process supervision. It depends on `core`
//! and keeps OS specifics out of the rest of the workspace.
//!
//! Scaffold stage: the first real capability is app-directory resolution
//! ([`app_dirs`] / [`log_dir`]); the rest land per architecture bible section 4.6.

#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used, clippy::panic))]

pub mod dirs;

pub use dirs::{AppDirs, DirsError, app_dirs, log_dir};
