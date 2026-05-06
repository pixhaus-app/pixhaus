//! Built-in AI verbs (S23–S36).
//!
//! Each verb lives in its own submodule and is registered with the
//! [`crate::plugin::runtime::VerbRuntime`] at startup. The modules are
//! public so the app crate can instantiate verbs with whatever
//! [`crate::backends::BackendRegistry`] the user has configured.
//!
//! # Verb ID namespace
//!
//! All built-in verbs use the prefix `pixhaus.builtin.`. Third-party
//! plugins use their own reverse-DNS namespace; the runtime does not
//! enforce namespacing but the convention prevents collisions.

pub mod variant;
