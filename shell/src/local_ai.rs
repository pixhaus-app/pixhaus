//! Local-model UI plumbing — download spawns, the Generate menu, and the
//! editor-mode generation actions (text-to-image, image-to-image, inpaint).
//!
//! Models the off-thread shapes on `spawn_backend_key_op` (blocking key ops) and
//! `spawn_clip` (cancel-token + progress closure). The download driver, the
//! `ShellMsg` variants, and the `ShellApp` impl block land in the settings /
//! editor-action phases. Kept as an empty, always-compiling module for now.
