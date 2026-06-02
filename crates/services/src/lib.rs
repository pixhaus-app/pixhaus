//! Pixhaus service layer: command execution, jobs, dispatch, and localization.
//!
//! `services` owns the shared behavior that sits above the domain model and
//! below the UI — command execution and undo/redo, transactions, the background
//! job system, asset indexing, provider dispatch, and the localization service.
//! It depends on `core` and never on egui.
//!
//! Scaffold stage: the command and job systems are still stubs (they land per
//! architecture bible sections 12 and 13). The localization service ([`i18n`])
//! is live — it is the one place rust-i18n is wired up, the string-side parallel
//! to the binary owning the one tracing subscriber.

// The one `i18n!` for the whole app: embeds every `locales/*.yaml` bundle at
// compile time and generates the crate-local `t!` machinery the `i18n` module
// wraps. `en` is the fallback when a key is missing in the active language.
rust_i18n::i18n!("locales", fallback = "en");

pub mod i18n;
