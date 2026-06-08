//! The Pixhaus localization service — the one façade over `rust-i18n`.
//!
//! No other crate calls `rust-i18n` directly. Strings are addressed by stable
//! key (`panel.layers.title`, `app.menu.file.title`, ...) and resolved to display
//! text here, at render time, in the active language. Keeping the dependency
//! behind this boundary means swapping the backing crate is a change confined to
//! this module (architecture bible section 32.1).
//!
//! The active language and the dev key-display toggle are process-global: there
//! is one active language for the whole app, which `rust-i18n` enforces, so the
//! service is a set of free functions over that global rather than an owned
//! handle. `app` sets the language once at boot; the shell calls [`tr`] every
//! frame.
//!
//! # Missing keys
//!
//! A key with no translation in the active language nor the `en` fallback
//! resolves to the key text itself and emits a `tracing::warn!` on the
//! `pixhaus::i18n` target — never on the hit path, so a 60fps render loop does
//! not flood the log.

use std::sync::atomic::{AtomicBool, Ordering};

/// Dev key-display toggle. When set, [`tr`] / [`tr_args`] / [`tr_plural`] return
/// the raw key instead of its translation — a built-in lint for hardcoded
/// strings (anything that does not turn into a key when the toggle is on was
/// never routed through the service). Bible section 32.3.
static SHOW_KEYS: AtomicBool = AtomicBool::new(false);

/// Resolve a stable key to display text in the active language.
///
/// Falls back to the `en` bundle, then to the key text itself (logging a warning)
/// if the key is unknown everywhere.
pub fn tr(key: &str) -> String {
    if show_keys() {
        return key.to_owned();
    }
    let translated = rust_i18n::t!(key);
    if translated.as_ref() == key {
        miss(key);
    }
    translated.into_owned()
}

/// Resolve a key and substitute `%{name}` interpolation slots from `args`.
///
/// `tr_args("app.ui.palette.switch_to", &[("name", "Draw")])` turns the template
/// `"Switch to %{name}"` into `"Switch to Draw"`. The slice form (rather than the
/// `t!` macro's literal named args) is what lets callers pass runtime values.
pub fn tr_args(key: &str, args: &[(&str, &str)]) -> String {
    if show_keys() {
        return key.to_owned();
    }
    let template = rust_i18n::t!(key);
    if template.as_ref() == key {
        miss(key);
        return key.to_owned();
    }
    interpolate(template.as_ref(), args)
}

/// Resolve a count-dependent key by selecting its `.one` / `.other` form, with
/// `%{count}` interpolated.
///
/// `tr_plural("panel.layers.count", 1)` resolves `panel.layers.count.one`
/// (`"%{count} layer"` -> `"1 layer"`); a count of 3 resolves `.other`
/// (`"3 layers"`). Form selection lives here, inside the service boundary,
/// because `rust-i18n` has no CLDR plural rules — only the English/Spanish-style
/// "one vs. other" split. A backing crate with real plural categories would
/// change only this function.
pub fn tr_plural(base_key: &str, count: i64) -> String {
    let suffix = if count == 1 { "one" } else { "other" };
    let key = format!("{base_key}.{suffix}");
    let count = count.to_string();
    tr_args(&key, &[("count", &count)])
}

/// Switch the active language at runtime (e.g. `"en"`, `"es"`). Takes effect on
/// the next [`tr`]; the egui loop re-renders every label the following frame.
pub fn set_language(language: &str) {
    rust_i18n::set_locale(language);
}

/// The active language tag (e.g. `"en"`).
pub fn current_language() -> String {
    rust_i18n::locale().to_string()
}

/// Every language the embedded bundles can serve.
pub fn available_languages() -> Vec<String> {
    rust_i18n::available_locales!().into_iter().map(std::borrow::Cow::into_owned).collect()
}

/// Flip the dev key-display toggle (the process-global `SHOW_KEYS` flag).
pub fn set_show_keys(enabled: bool) {
    SHOW_KEYS.store(enabled, Ordering::Relaxed);
}

/// Whether the dev key-display toggle is currently on.
pub fn show_keys() -> bool {
    SHOW_KEYS.load(Ordering::Relaxed)
}

/// Substitute every `%{name}` slot in `template` with its value from `args`.
fn interpolate(template: &str, args: &[(&str, &str)]) -> String {
    let mut out = template.to_owned();
    for &(name, value) in args {
        out = out.replace(&format!("%{{{name}}}"), value);
    }
    out
}

/// Warn that a key resolved to nothing. `#[cold]` and off the hit path: only a
/// genuine miss reaches it, so it never floods the per-frame render loop.
#[cold]
fn miss(key: &str) {
    tracing::warn!(
        target: "pixhaus::i18n",
        missing_key = key,
        language = %current_language(),
        "translation missing; falling back to the key"
    );
}

#[cfg(test)]
mod tests {
    use super::{available_languages, current_language, set_language, set_show_keys, show_keys, tr, tr_args, tr_plural};
    use serial_test::serial;

    // rust-i18n's active locale is process-global, so every test that reads or
    // writes it is `#[serial]` and sets its own starting language — otherwise
    // nextest's parallelism would let one test's `set_language` bleed into
    // another that assumes `en`.

    #[test]
    #[serial]
    fn tr_resolves_a_known_key_in_en() {
        set_language("en");
        assert_eq!(tr("panel.layers.title"), "Layers");
    }

    #[test]
    #[serial]
    fn tr_falls_back_to_the_key_when_missing() {
        set_language("en");
        assert_eq!(tr("panel.does_not_exist.title"), "panel.does_not_exist.title");
    }

    #[test]
    #[serial]
    fn set_language_switches_resolved_text() {
        set_language("en");
        let english = tr("panel.layers.title");
        set_language("es");
        let spanish = tr("panel.layers.title");
        set_language("en");
        assert_eq!(english, "Layers");
        assert_eq!(spanish, "Capas");
        assert_ne!(english, spanish);
    }

    #[test]
    #[serial]
    fn unknown_language_falls_back_to_en() {
        set_language("zz");
        assert_eq!(tr("panel.layers.title"), "Layers");
        set_language("en");
    }

    #[test]
    #[serial]
    fn dev_toggle_returns_the_raw_key() {
        set_language("en");
        set_show_keys(true);
        assert_eq!(tr("panel.layers.title"), "panel.layers.title");
        set_show_keys(false);
        assert_eq!(tr("panel.layers.title"), "Layers");
    }

    #[test]
    #[serial]
    fn show_keys_reports_the_stored_flag() {
        set_show_keys(true);
        assert!(show_keys(), "show_keys must report the flag set_show_keys stored");
        set_show_keys(false);
        assert!(!show_keys());
    }

    #[test]
    #[serial]
    fn tr_args_substitutes_named_slots() {
        set_language("en");
        assert_eq!(tr_args("app.ui.palette.switch_to", &[("name", "Draw")]), "Switch to Draw");
    }

    #[test]
    #[serial]
    fn tr_plural_selects_one_versus_other() {
        set_language("en");
        assert_eq!(tr_plural("panel.layers.count", 1), "1 layer");
        assert_eq!(tr_plural("panel.layers.count", 3), "3 layers");
    }

    #[test]
    #[serial]
    fn available_languages_lists_the_embedded_bundles() {
        // Reads the embedded bundle set, not the active locale, but stays #[serial]
        // with the rest since it shares rust-i18n's process-global state.
        let langs = available_languages();
        assert!(langs.iter().any(|l| l == "en"), "en bundle is available");
        assert!(langs.iter().any(|l| l == "es"), "es bundle is available");
    }

    #[test]
    #[serial]
    fn current_language_reports_the_active_locale() {
        set_language("es");
        assert_eq!(current_language(), "es");
        set_language("en");
        assert_eq!(current_language(), "en");
    }
}
