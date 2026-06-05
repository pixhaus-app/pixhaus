//! Headless render harness: rasterize each shell workspace to a PNG for visual tuning.
//!
//! Run with `cargo run -p pixhaus-app --example render_workspaces`. It builds the
//! full shell `Host` (theme, fonts, shell menus, the five capability modules in the
//! same order `app/src/main.rs` registers them), drives one workspace per off-screen
//! `egui_kittest` frame, and writes `target/ui-snapshots/<workspace>.png`.
//!
//! Why an example, not a test: this is a re-runnable tuning artifact, not a pass/fail
//! gate. After a styling change, re-run it and eyeball the PNGs against the reference
//! screenshots. The wgpu canvas renderer is installed into the harness `RenderState`
//! (so the GPU-drawn checkerboard, grid, and sprite show in the captures, the same as
//! the real app); the stage backdrop, board frame, and HUD are still egui-drawn.
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
use pixhaus_ui::state::ui_state::{Modal, SplashPhase};

/// Reference screenshots are ~16:10 desktop captures; match that aspect.
const SIZE: egui::Vec2 = egui::vec2(1440.0, 900.0);

/// The workspace ids, in shell order. Each becomes one PNG.
const WORKSPACES: [&str; 6] = ["draw", "animate", "tiles", "generate", "export", "codex"];

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
    // Install egui_extras' image loaders so the baked-in brand PNGs render in snapshots.
    pixhaus_ui::install_image_loaders(ctx);

    pixhaus_ui::shell::menus::register_shell_menus(&mut host.registrar());

    // sprite-edit FIRST: it registers the shared panels and tools the other modules
    // name by id, so it must run before any layout references them.
    pixhaus_mod_sprite_edit::SpriteEditModule.register(&mut host.registrar());
    pixhaus_mod_animation::AnimationModule.register(&mut host.registrar());
    pixhaus_mod_tiles::TilesModule.register(&mut host.registrar());
    pixhaus_mod_generation::GenerationModule.register(&mut host.registrar());
    pixhaus_mod_export::ExportModule.register(&mut host.registrar());
    pixhaus_mod_codex::CodexModule.register(&mut host.registrar());

    host
}

/// Render one snapshot to `dir/<name>.png`, applying `setup` to the fresh host each
/// frame before `Shell::run`. Returns the byte size.
///
/// `setup` runs every frame because the host is rebuilt every frame (the closure is
/// the only owner, so no borrow escapes it). That makes any state the snapshot needs -
/// the active workspace, the splash phase, an open modal - sticky across the settle
/// loop even though the host itself is not persisted.
fn render_snapshot(name: &str, dir: &std::path::Path, setup: impl Fn(&mut Host) + 'static) -> anyhow::Result<u64> {
    // Build the off-screen wgpu renderer that `Harness::render` needs, then install the
    // Pixhaus canvas renderer into its `RenderState` so the GPU canvas (checkerboard,
    // grid, sprite) shows in the capture — the chrome moved onto the GPU, so without this
    // the stage interior would be blank. `from_render_state` only rejects a state that has
    // already created a managed texture; our renderer goes into `callback_resources`, so a
    // fresh, install-only state is still accepted. A small `step_dt` with a generous
    // `max_steps` keeps the egui frame-clock (which the splash timer reads) below the 1.8s
    // dismiss threshold across the settle loop, while still giving egui_extras' async image
    // decode the wall-clock time it needs to land the brand textures.
    let render_state = egui_kittest::wgpu::create_render_state(egui_kittest::wgpu::default_wgpu_setup());
    pixhaus_ui::install_canvas_renderer(&render_state);
    let canvas_renderer = egui_kittest::wgpu::WgpuTestRenderer::from_render_state(render_state);

    let mut harness = Harness::builder()
        .with_size(SIZE)
        .with_step_dt(0.05)
        .with_max_steps(30)
        .renderer(canvas_renderer)
        .build_ui(move |ui| {
            let mut host = build_host(ui.ctx());
            setup(&mut host);
            Shell::run(&mut host, ui);
        });

    // `try_run_realtime` sleeps `step_dt` of real time between frames so the brand PNGs
    // finish decoding and upload before the capture. The splash overlay requests a
    // repaint every active frame, so that snapshot rides the loop to the step cap; we
    // tolerate that here - the frames still ran and the textures still loaded.
    let _ = harness.try_run_realtime();

    let image = harness.render().map_err(|e| anyhow!("wgpu render failed for {name}: {e}"))?;

    let path = dir.join(format!("{name}.png"));
    image.save(&path).with_context(|| format!("failed to write {}", path.display()))?;

    let size = std::fs::metadata(&path).with_context(|| format!("failed to stat {}", path.display()))?.len();
    println!("wrote {} ({size} bytes)", path.display());
    Ok(size)
}

/// Seed the canonical Bit demo Codex so the workspace snapshot exercises every region.
///
/// The world itself - 36 entries across 8 folders, rich bodies, every anchor kind,
/// typed relationships, prompt/negative fragments, and per-entry coverage - is built
/// by the shared `pixhaus_services::build_bit_demo_codex` fixture, the same data the
/// app boots into. The example only transplants that document into the host and, for
/// the snapshot, focuses the editor on Bit and pins it to the generation context stack
/// so the Navigator's Pinned section shows a real entry.
///
/// Idempotent: returns once the codex has entries (the host is rebuilt every frame).
/// On the (not-expected) build error path it leaves the codex empty rather than
/// panicking the harness.
fn seed_codex(host: &mut Host) {
    use pixhaus_core::AnchorStrength;

    if !host.edit.document.codex().entries().is_empty() {
        // Already seeded; just keep the editor focused on the first entry.
        if host.state.ui.codex.selected.is_none() {
            let first = host.edit.document.codex().entries().keys().next().copied();
            host.state.ui.codex.selected = first;
        }
        return;
    }

    match pixhaus_services::build_bit_demo_codex() {
        Ok(doc) => host.edit.document = doc,
        Err(err) => {
            tracing::warn!(%err, "failed to build the Bit demo codex; leaving it empty");
            return;
        }
    }

    // Focus the editor on Bit and pin it to the generation context stack.
    let bit = pixhaus_core::CodexHandle::new("bit")
        .ok()
        .and_then(|h| host.edit.document.codex().resolve_handle(&h));
    if let Some(id) = bit {
        host.state.ui.codex.selected = Some(id);
        if !host.state.ui.codex.context.iter().any(|c| c.entry == id) {
            host.state.ui.codex.context.push(pixhaus_ui::state::ui_state::ContextRef {
                entry: id,
                strength: AnchorStrength::default(),
            });
        }
    }
}

fn main() -> anyhow::Result<()> {
    let out_dir = std::path::Path::new("target").join("ui-snapshots");
    std::fs::create_dir_all(&out_dir).with_context(|| format!("failed to create {}", out_dir.display()))?;

    // Workspace snapshots: dismiss the splash so the shell chrome (and the brand mark
    // in the top bar) is what we capture, not the launch splash.
    for workspace in WORKSPACES {
        render_snapshot(workspace, &out_dir, move |host| {
            host.state.ui.splash = SplashPhase::Done;
            host.state.session.active_workspace = WorkspaceId(workspace);
            // The Codex starts empty; seed a few entries so its Navigator, editor, and
            // inspector have content in the snapshot. Idempotent (the host is rebuilt
            // every frame, so guard on the empty codex).
            if workspace == "codex" {
                seed_codex(host);
            }
        })?;
    }

    // About snapshot: splash done, the About modal open over the Draw workspace.
    render_snapshot("about", &out_dir, |host| {
        host.state.ui.splash = SplashPhase::Done;
        host.state.ui.modal = Some(Modal::About);
    })?;

    // Splash snapshot: stamp the start time at 0.0 so elapsed stays below the dismiss
    // threshold across the few settle frames and the logo keeps painting.
    render_snapshot("splash", &out_dir, |host| {
        host.state.ui.splash = SplashPhase::Active { since: Some(0.0) };
    })?;

    println!("{} snapshots in {}", WORKSPACES.len() + 2, out_dir.display());
    Ok(())
}
