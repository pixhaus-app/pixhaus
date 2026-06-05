//! Snapshot the resolved layout of the Codex workspace.
//!
//! The Codex is the one non-canvas workspace - a left Navigator, a full-center
//! Entry Editor panel (not the wgpu canvas), a right Inspector, and a
//! Coverage/Test/History bottom tray. Its `ResolvedLayout` is the regression
//! surface for that placement: a moved panel, a renamed workspace, a switched
//! center surface, or a dropped tray tab shows as a snapshot diff. The Codex stays
//! out of the shared `fully_registered_host` (the top-bar smoke test asserts an
//! exact five-workspace tab set), so this test builds its own host.

#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used))]
// insta's `assert_debug_snapshot!` expands to a `.unwrap()` we do not control;
// the disallowed-methods lint fires on that macro-internal call, not on our code.
#![allow(clippy::disallowed_methods)]

use pixhaus_ui::contrib_api::{Module, WorkspaceId};
use pixhaus_ui::registry::resolve_layout;
use pixhaus_ui::state::Host;
use pixhaus_ui::theme::Theme;

/// Build a host with only the Codex module registered. The Codex layout references
/// none of sprite-edit's shared panels or tools (its rail is empty and it owns every
/// panel it names), so it resolves fully on its own.
fn codex_host() -> Host {
    let mut host = Host::new(&Theme::dark());
    pixhaus_mod_codex::CodexModule.register(&mut host.registrar());
    host
}

#[test]
fn codex_layout_is_stable() {
    let host = codex_host();
    let resolved = resolve_layout(WorkspaceId("codex"), &host.registries);
    insta::assert_debug_snapshot!("codex_layout", resolved);
}

#[test]
fn codex_resolves_a_panel_center_and_left_dock() {
    use pixhaus_ui::contrib_api::CenterSurface;

    let host = codex_host();
    let resolved = resolve_layout(WorkspaceId("codex"), &host.registries);
    // The Codex is the only workspace whose center is a panel, not the canvas, and
    // the only one that fills the left dock.
    assert_eq!(resolved.center, CenterSurface::Panel(pixhaus_ui::contrib_api::PanelId("codex-editor")));
    assert!(!resolved.left_dock.is_empty(), "the Codex fills the left dock with its Navigator");
    assert!(!resolved.right_dock.is_empty(), "the Codex fills the right dock with its Inspector");
    assert!(!resolved.bottom_tray.is_empty(), "the Codex has a Coverage/Test/History tray");
    assert!(resolved.primary_tools.is_empty(), "the Codex has no pixel tools");
}
