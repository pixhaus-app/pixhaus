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

use pixhaus_ui::contrib_api::MsgKey;

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

#[test]
fn every_registered_key_resolves() {
    let host = fully_registered_host();

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
    }
    for action in host.registries.actions.iter() {
        assert_resolves(action.label);
    }
}
