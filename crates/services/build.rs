//! Build script: re-embed the localization bundles when a locale file changes.
//!
//! rust-i18n's `i18n!` macro reads `locales/` at compile time, but the macro alone
//! gives cargo no reason to recompile this crate when a bundle is added or edited -
//! so a freshly added `locales/*.yaml` would silently resolve to its raw key until a
//! manual `cargo clean`. This `rerun-if-changed` ties the crate's freshness to the
//! directory, so adding or editing a bundle recompiles and re-embeds it.
fn main() {
    println!("cargo:rerun-if-changed=locales");
}
