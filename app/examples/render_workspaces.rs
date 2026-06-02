//! Headless render harness: rasterize each shell workspace to a PNG for visual tuning.
//!
//! Run with `cargo run -p pixhaus-app --example render_workspaces`. It builds the
//! full shell `Host` (theme, fonts, shell menus, the five capability modules in the
//! same order `app/src/main.rs` registers them), drives one workspace per off-screen
//! `egui_kittest` frame, and writes `target/ui-snapshots/<workspace>.png`.
//!
//! Why an example, not a test: this is a re-runnable tuning artifact, not a pass/fail
//! gate. After a styling change, re-run it and eyeball the PNGs against the reference
//! screenshots. The wgpu canvas interior is intentionally absent here — the example
//! never installs a [`pixhaus_ui::ViewportRenderer`], so `CanvasCallback` no-ops and
//! the egui-drawn canvas chrome (stage, checkerboard, grid, HUD) is what we capture.
//!
//! The harness builds a fresh `Host` and a fresh `Harness` per workspace. That avoids
//! borrowing one `Host` both inside the `build_ui` closure and across the render loop,
//! and keeps each PNG a clean first-paint of its workspace.

// This is a CLI tool whose whole job is to report the PNG paths it wrote; stdout is the
// interface, so the workspace-wide `print_stdout` deny doesn't apply here.
#![allow(clippy::print_stdout)]

use anyhow::{Context, anyhow};
use eframe::egui;
use egui_kittest::Harness;

use pixhaus_ui::contrib_api::Module;
use pixhaus_ui::contrib_api::ids::WorkspaceId;
use pixhaus_ui::shell::Shell;
use pixhaus_ui::state::Host;

/// Reference screenshots are ~16:10 desktop captures; match that aspect.
const SIZE: egui::Vec2 = egui::vec2(1440.0, 900.0);

/// The five workspace ids, in shell order. Each becomes one PNG.
const WORKSPACES: [&str; 5] = ["draw", "animate", "tiles", "generate", "export"];

/// Build the shell host with the full registration, mirroring `app/src/main.rs`.
///
/// Replicated rather than reused: `build_host` is a private fn in the `pixhaus-app`
/// binary (`main.rs`), and an example can't reach a binary's private items. The body
/// is small and the registration order is the load-bearing part — keep it identical
/// to `main.rs` so the snapshot reflects the real shell. The theme/fonts go on the
/// harness's own `egui::Context`, applied per workspace since each gets a fresh
/// harness.
fn build_host(ctx: &egui::Context) -> Host {
    let mut host = Host::new(&pixhaus_ui::theme::Theme::dark());

    pixhaus_ui::theme::apply_to_visuals(host.theme(), ctx);
    pixhaus_ui::theme::install_fonts(ctx);

    pixhaus_ui::shell::menus::register_shell_menus(&mut host.registrar());

    // sprite-edit FIRST: it registers the shared panels and tools the other modules
    // name by id, so it must run before any layout references them.
    pixhaus_mod_sprite_edit::SpriteEditModule.register(&mut host.registrar());
    pixhaus_mod_animation::AnimationModule.register(&mut host.registrar());
    pixhaus_mod_tiles::TilesModule.register(&mut host.registrar());
    pixhaus_mod_generation::GenerationModule.register(&mut host.registrar());
    pixhaus_mod_export::ExportModule.register(&mut host.registrar());

    host
}

/// Render one workspace to a PNG at `dir/<workspace>.png`. Returns the byte size.
fn render_workspace(workspace: &'static str, dir: &std::path::Path) -> anyhow::Result<u64> {
    // `.wgpu()` installs the off-screen `WgpuTestRenderer` that `Harness::render` needs;
    // without it the default lazy renderer can't rasterize a frame.
    let mut harness = Harness::builder().with_size(SIZE).wgpu().build_ui(move |ui| {
        // A fresh Host per frame: the closure is the only owner, so no borrow escapes
        // it. Theme/fonts are applied to this harness's context inside build_host.
        let mut host = build_host(ui.ctx());
        host.state.session.active_workspace = WorkspaceId(workspace);
        Shell::run(&mut host, ui);
    });

    // Run frames until layout settles (panels resolve sizes, fonts upload). `run`
    // returns the frame count; `run_ok` would return None if it hit the step cap.
    harness.run();

    let image = harness.render().map_err(|e| anyhow!("wgpu render failed for {workspace}: {e}"))?;

    let path = dir.join(format!("{workspace}.png"));
    image.save(&path).with_context(|| format!("failed to write {}", path.display()))?;

    let size = std::fs::metadata(&path).with_context(|| format!("failed to stat {}", path.display()))?.len();
    println!("wrote {} ({size} bytes)", path.display());
    Ok(size)
}

fn main() -> anyhow::Result<()> {
    let out_dir = std::path::Path::new("target").join("ui-snapshots");
    std::fs::create_dir_all(&out_dir).with_context(|| format!("failed to create {}", out_dir.display()))?;

    for workspace in WORKSPACES {
        render_workspace(workspace, &out_dir)?;
    }

    println!("{} workspace snapshots in {}", WORKSPACES.len(), out_dir.display());
    Ok(())
}
