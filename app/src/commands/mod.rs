//! IPC command catalog.
//!
//! All Tauri commands live here, grouped by category. The
//! `invoke_handler` list in `crate::run` is the canonical sorted
//! catalog. Add commands there when adding them to a module.

pub mod ai;
pub mod app_info;
pub mod canvas;
pub mod crash_reporting;
pub mod exports;
pub mod frames;
pub mod layers;
pub mod library;
pub mod palette;
pub mod plugins;
pub mod project;
pub mod samples;
pub mod tiles;
pub mod undo;
pub mod updater;
pub mod verbs;
