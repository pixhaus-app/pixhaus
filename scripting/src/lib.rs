//! Pixhaus scripting: Lua bindings via `mlua`, with hot reload.
//!
//! Surface follows Aseprite's Lua API where it makes sense, so existing scripts
//! have a migration path. Plugin-defined custom UI panels, custom tools, and
//! custom AI verbs route through this crate.

#![cfg_attr(
    test,
    allow(
        clippy::unwrap_used,
        clippy::expect_used,
        clippy::panic,
        clippy::missing_panics_doc
    )
)]

/// Returns the crate name. Placeholder until scripting lands.
#[must_use]
pub fn crate_name() -> &'static str {
    "pixhaus-scripting"
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn crate_name_is_stable() {
        assert_eq!(crate_name(), "pixhaus-scripting");
    }
}
