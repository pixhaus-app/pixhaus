//! Pixhaus platform layer: the OS-facing edges.
//!
//! `platform` owns native dialogs, recent-files tracking, OS settings paths, GPU
//! capability detection, and external-process supervision. It depends on `core`
//! and keeps OS specifics out of the rest of the workspace.
//!
//! Scaffold stage: a stub. Capabilities land per architecture bible section 4.6.
