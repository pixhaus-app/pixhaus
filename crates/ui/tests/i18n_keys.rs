//! Dangling-key gate: every registry-level i18n key resolves to a translation.
//!
//! Walks the fully-registered host - menu groups and items, panel titles, tool
//! labels and tooltips, workspace names and purposes, and action labels - and
//! asserts each `MsgKey` resolves to a real string rather than falling back to the
//! raw key. A key referenced in code but missing from a
//! `crates/services/locales/*.yaml` bundle fails here.
//!
//! No `set_language` call is needed: a present key resolves through the `en`
//! fallback whatever the active locale, so the only way a key resolves to itself is
//! that it is in no bundle at all.

#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used))]

mod support;

use pixhaus_ui::contrib_api::{Module, MsgKey, StatusLabel};
use pixhaus_ui::state::Host;
use pixhaus_ui::theme::Theme;

use support::fully_registered_host;

/// Assert `key` resolves to something other than its own text (i.e. it is bundled).
#[track_caller]
fn assert_resolves(key: MsgKey) {
    assert_ne!(
        key.tr().as_str(),
        key.0,
        "i18n key {:?} has no translation - add it to a crates/services/locales/*.yaml bundle",
        key.0,
    );
}

/// Walk every registry-level key on `host` and assert each resolves: menu groups and items,
/// panel titles, tool labels and tooltips, workspace names/purposes, workspace status-bar
/// items (the `StatusLabel::Key` ones; `StatusLabel::Data` is verbatim, never localized), and
/// action labels.
fn assert_all_registry_keys_resolve(host: &Host) {
    for group in &host.registries.menus {
        assert_resolves(group.label);
        for item in &group.items {
            assert_resolves(item.label);
        }
    }
    for panel in host.registries.panels.iter() {
        assert_resolves(panel.meta().title);
    }
    for tool in host.registries.tools.iter() {
        assert_resolves(tool.meta().label);
        assert_resolves(tool.meta().tooltip);
    }
    for ws in host.registries.workspaces.iter() {
        assert_resolves(ws.meta().name);
        assert_resolves(ws.meta().purpose);
        for item in ws.layout().status_items {
            if let StatusLabel::Key { key, .. } = item.label {
                assert_resolves(key);
            }
        }
    }
    for action in host.registries.actions.iter() {
        assert_resolves(action.label);
    }
}

#[test]
fn every_registered_key_resolves() {
    assert_all_registry_keys_resolve(&fully_registered_host());
}

#[test]
fn every_codex_key_resolves() {
    // The Codex module is excluded from fully_registered_host (it is a sixth workspace and the
    // top-bar smoke test asserts exactly five), so its keys - the workspace name/purpose, the
    // six panel titles, the codex.* actions, and the coverage status item - need their own
    // dangling-key gate here.
    let mut host = Host::new(&Theme::dark());
    pixhaus_mod_codex::CodexModule.register(&mut host.registrar());
    assert_all_registry_keys_resolve(&host);
}
