//! Pixhaus AI — vertical-slice subset.
//!
//! Minimal port from the main checkout's `ai` crate: the verb plugin runtime,
//! the reference-sheet verb and its prompt composition, and the FAL inference
//! backend (the one adapter that covers the whole slice — image generation,
//! image-to-video, and background removal).
//!
//! # Layout
//!
//! - [`plugin`] — the verb protocol: descriptor, context, inputs, output,
//!   progress, preview, the thin backend trait, and the runtime that
//!   registers verbs and backends and dispatches invocations.
//! - [`backends`] — the fat [`backends::InferenceBackend`] trait, the request
//!   and response types, the [`backends::BackendProxy`] bridge, the keychain
//!   [`backends::ApiKeyStore`], and the [`backends::fal::FalBackend`] adapter.
//! - [`compose`] — the prompt-composition resolver and the built-in
//!   composition library (Structures, Styles, example Prompts).
//! - [`verbs`] — the reference-sheet verb plus `ctx_fat_backend`, the helper
//!   that recovers the fat backend from a [`backends::BackendProxy`] stored on
//!   the verb context.

#![cfg_attr(
    test,
    allow(
        clippy::unwrap_used,
        clippy::expect_used,
        clippy::panic,
        clippy::disallowed_methods,
        clippy::cast_lossless,
        clippy::cast_possible_truncation,
        // The ported tests assert on exact cost/strength floats and print
        // skip notes from the keychain integration test; the upstream crate
        // allowed these at the crate root too.
        clippy::float_cmp,
        clippy::missing_panics_doc,
        clippy::print_stderr,
        clippy::print_stdout,
    )
)]

pub mod backends;
pub mod compose;
pub mod plugin;
pub mod verbs;
