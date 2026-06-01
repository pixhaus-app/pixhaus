//! Pixhaus application binary: the eframe + egui host shell.
//!
//! The Host App layer (architecture bible section 4.1). It owns the single tokio
//! runtime, boots the window, builds the [`pixhaus_ui::state::Host`], registers the
//! shell menus and the capability modules, and runs the egui loop; the canvas is
//! drawn by `render` through the egui-wgpu paint callback installed at startup.

use anyhow::Context;
use eframe::egui;

use pixhaus_ui::contrib_api::Module;
use pixhaus_ui::state::Host;

/// Build the shell host: theme, fonts, shell menus, and the capability modules.
///
/// Registration order is load-bearing. `register_shell_menus` runs first so the
/// shell's File/Edit/View/Window/Help groups precede the module-contributed
/// Sprite/Layer/Frame/Select groups. `SpriteEditModule` registers next because it
/// owns the shared panel and tool ids the other workspaces reference by value
/// (bible rule 2); the remaining modules append their workspaces after it.
fn build_host(ctx: &egui::Context) -> Host {
    let mut host = Host::new(pixhaus_ui::theme::Theme::dark());

    pixhaus_ui::theme::apply_to_visuals(host.theme(), ctx);
    pixhaus_ui::theme::install_fonts(ctx);

    pixhaus_ui::shell::menus::register_shell_menus(&mut host.registrar());

    // sprite-edit FIRST: it registers the shared panels and tools the other
    // modules name by id, so it must run before any layout references them.
    pixhaus_mod_sprite_edit::SpriteEditModule.register(&mut host.registrar());
    pixhaus_mod_animation::AnimationModule.register(&mut host.registrar());
    pixhaus_mod_tiles::TilesModule.register(&mut host.registrar());
    pixhaus_mod_generation::GenerationModule.register(&mut host.registrar());
    pixhaus_mod_export::ExportModule.register(&mut host.registrar());

    host
}

/// Top-level application state, owned across frames by the eframe loop.
///
/// The [`Host`] is the single owner of every piece of shell-level mutable state
/// (registries, session/UI state, the intent sink, theme). The eframe loop drives
/// it: `logic` drains background results, `ui` composes the regions.
struct PixhausApp {
    host: Host,
}

impl PixhausApp {
    fn new(cc: &eframe::CreationContext<'_>) -> Self {
        // Install the canvas renderer into egui-wgpu's resource store before the
        // first paint; the paint callback retrieves it from there each frame.
        if let Some(render_state) = cc.wgpu_render_state.as_ref() {
            pixhaus_ui::install_canvas_renderer(render_state);
        }
        let host = build_host(&cc.egui_ctx);
        Self { host }
    }
}

impl eframe::App for PixhausApp {
    fn logic(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        pixhaus_ui::shell::drain_background(&mut self.host, ctx);
    }

    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        pixhaus_ui::shell::Shell::run(&mut self.host, ui);
    }
}

fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .init();

    // The binary owns the single tokio runtime; entering it makes tokio::spawn
    // available to the egui loop for the background work the editor will grow.
    let runtime = tokio::runtime::Runtime::new().context("failed to start the tokio runtime")?;
    let _guard = runtime.enter();

    let options = eframe::NativeOptions {
        renderer: eframe::Renderer::Wgpu,
        viewport: egui::ViewportBuilder::default()
            .with_title("Pixhaus")
            .with_inner_size([1280.0, 800.0])
            .with_min_inner_size([640.0, 480.0]),
        ..Default::default()
    };

    eframe::run_native("pixhaus", options, Box::new(|cc| Ok(Box::new(PixhausApp::new(cc)))))?;

    Ok(())
}
