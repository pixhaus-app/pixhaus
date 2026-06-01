//! Spec test 7: headless boot smoke test (`egui_kittest` declined).
//!
//! No event loop, no GPU. Boot the Host with all five modules registered and
//! assert every workspace resolves to a non-empty dock and tray, and that the
//! workspace tab set is exactly the five expected names.

#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used))]

mod support;

use pixhaus_ui::contrib_api::WorkspaceId;
use pixhaus_ui::registry::resolve_layout;

use support::{WORKSPACE_IDS, WORKSPACE_NAMES, fully_registered_host};

#[test]
fn every_workspace_resolves_a_non_empty_dock() {
    let host = fully_registered_host();
    for id in WORKSPACE_IDS {
        let resolved = resolve_layout(WorkspaceId(id), &host.registries);
        assert!(!resolved.right_dock.is_empty(), "workspace {id:?} resolved an empty right dock");
    }
}

#[test]
fn every_workspace_resolves_a_non_empty_tray() {
    let host = fully_registered_host();
    for id in WORKSPACE_IDS {
        let resolved = resolve_layout(WorkspaceId(id), &host.registries);
        assert!(!resolved.bottom_tray.is_empty(), "workspace {id:?} resolved an empty bottom tray");
    }
}

#[test]
fn top_bar_tab_set_is_the_five_workspace_names() {
    let host = fully_registered_host();
    let names: Vec<&str> = host.registries.workspaces.iter().map(|ws| ws.meta().name).collect();
    assert_eq!(names.len(), WORKSPACE_NAMES.len(), "expected exactly five registered workspaces, got {names:?}");
    for expected in WORKSPACE_NAMES {
        assert!(names.contains(&expected), "workspace tab set {names:?} is missing {expected:?}");
    }
}
