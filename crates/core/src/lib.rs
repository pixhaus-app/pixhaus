//! Pixhaus creative core: the domain model and pure operations.
//!
//! `core` owns the authoritative project data — projects, documents, sprites,
//! layers, frames, cels, palettes, selections, art-mode metadata, the typed ids
//! that key them, and the `Command` trait through which every mutation flows. It
//! is pure data and operations: no egui, no wgpu, no I/O. Everything else in the
//! workspace depends on it; it depends on nothing in the workspace.
//!
//! Scaffold stage: the types land as the shared sprite-editing core is built
//! (architecture bible sections 9 and 12). This crate is currently a stub.
