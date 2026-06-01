//! Shared test support: build the fully-registered Host the way `app` does.
//!
//! Registration order is the contract - `register_shell_menus` first, then
//! sprite-edit, so the other workspaces can reference its shared panels by id
//! (see the design spec, "modules/* and app wiring" and `app::build_host`). The
//! visuals/fonts calls `build_host` makes need an `egui::Context` and touch only
//! the renderer, not the registries, so the headless helper omits them.

#![allow(dead_code)] // each test binary uses a subset of these helpers.

use pixhaus_ui::contrib_api::Module;
use pixhaus_ui::shell::menus::register_shell_menus;
use pixhaus_ui::state::Host;
use pixhaus_ui::theme::Theme;

/// The five workspace ids the wired modules register, in declaration order.
pub const WORKSPACE_IDS: [&str; 5] = ["draw", "animate", "tiles", "generate", "export"];

/// The five workspace display names, for the top-bar tab-set assertion.
pub const WORKSPACE_NAMES: [&str; 5] = ["Draw", "Animate", "Tiles", "Generate", "Export"];

/// Build a Host with all five modules registered, exactly as `app::build_host` does.
///
/// `register_shell_menus` runs first (the menu groups precede the module groups),
/// then sprite-edit (it owns the shared panel and tool ids the other workspaces
/// reference by value), then the remaining four modules in declaration order.
pub fn fully_registered_host() -> Host {
    let mut host = Host::new(Theme::dark());

    register_shell_menus(&mut host.registrar());

    pixhaus_mod_sprite_edit::SpriteEditModule.register(&mut host.registrar());
    pixhaus_mod_animation::AnimationModule.register(&mut host.registrar());
    pixhaus_mod_tiles::TilesModule.register(&mut host.registrar());
    pixhaus_mod_generation::GenerationModule.register(&mut host.registrar());
    pixhaus_mod_export::ExportModule.register(&mut host.registrar());

    host
}
