//! Spec test 2: snapshot the resolved layout of every workspace.
//!
//! With all five modules registered, each workspace's `ResolvedLayout` is the
//! single regression surface for placement - a moved panel, a renamed
//! workspace, or a dropped tray tab is a snapshot diff. An unregistered panel
//! reference shows as a gap here (and `resolve_layout` logs a warn).

#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used))]

mod support;

use pixhaus_ui::contrib_api::WorkspaceId;
use pixhaus_ui::registry::resolve_layout;

use support::{WORKSPACE_IDS, fully_registered_host};

#[test]
fn draw_layout_is_stable() {
    let host = fully_registered_host();
    let resolved = resolve_layout(WorkspaceId(WORKSPACE_IDS[0]), &host.registries);
    insta::assert_debug_snapshot!("draw_layout", resolved);
}

#[test]
fn animate_layout_is_stable() {
    let host = fully_registered_host();
    let resolved = resolve_layout(WorkspaceId(WORKSPACE_IDS[1]), &host.registries);
    insta::assert_debug_snapshot!("animate_layout", resolved);
}

#[test]
fn tiles_layout_is_stable() {
    let host = fully_registered_host();
    let resolved = resolve_layout(WorkspaceId(WORKSPACE_IDS[2]), &host.registries);
    insta::assert_debug_snapshot!("tiles_layout", resolved);
}

#[test]
fn generate_layout_is_stable() {
    let host = fully_registered_host();
    let resolved = resolve_layout(WorkspaceId(WORKSPACE_IDS[3]), &host.registries);
    insta::assert_debug_snapshot!("generate_layout", resolved);
}

#[test]
fn export_layout_is_stable() {
    let host = fully_registered_host();
    let resolved = resolve_layout(WorkspaceId(WORKSPACE_IDS[4]), &host.registries);
    insta::assert_debug_snapshot!("export_layout", resolved);
}
