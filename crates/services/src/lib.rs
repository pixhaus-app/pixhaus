//! Pixhaus service layer: command execution, jobs, and dispatch.
//!
//! `services` owns the shared behavior that sits above the domain model and
//! below the UI — command execution and undo/redo, transactions, the background
//! job system, asset indexing, and provider dispatch. It depends on `core` and
//! never on egui.
//!
//! Scaffold stage: a stub. The command and job systems land per architecture
//! bible sections 12 and 13.
