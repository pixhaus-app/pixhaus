//! IPC command catalog.
//!
//! All Tauri commands live here, grouped by category. The
//! `invoke_handler` list in `crate::run` is the canonical sorted
//! catalog. Add commands there when adding them to a module.

pub mod canvas;
pub mod frames;
pub mod layers;
pub mod palette;
pub mod project;
pub mod tiles;
pub mod verbs;
