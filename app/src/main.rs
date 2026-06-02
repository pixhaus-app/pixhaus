//! Pixhaus application binary: the eframe + egui host shell.
//!
//! The Host App layer (architecture bible section 4.1). It owns the single tokio
//! runtime, boots the window, builds the [`pixhaus_ui::state::Host`], registers the
//! shell menus and the capability modules, and runs the egui loop; the canvas is
//! drawn by `render` through the egui-wgpu paint callback installed at startup.
//!
//! It also owns the one tracing subscriber ([`diagnostics`]): libraries emit, the
//! binary configures. The subscriber is built before the runtime so startup itself
//! is logged.

#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used, clippy::panic))]

mod diagnostics;

use std::sync::Arc;

use anyhow::Context;
use eframe::egui;

use pixhaus_ui::contrib_api::Module;
use pixhaus_ui::state::Host;

/// Build the shell host: theme, fonts, shell menus, and the capability modules.
///
/// Registration order is load-bearing. `register_shell_menus` runs first so the
/// shell's File/Edit/Select/View/Window/Help groups precede the module-contributed
/// Sprite/Layer/Frame groups. `SpriteEditModule` registers next because it
/// owns the shared panel and tool ids the other workspaces reference by value
/// (bible rule 2); the remaining modules append their workspaces after it.
fn build_host(ctx: &egui::Context) -> Host {
    let mut host = Host::new(&pixhaus_ui::theme::Theme::dark());

    pixhaus_ui::theme::apply_to_visuals(host.theme(), ctx);
    pixhaus_ui::theme::install_fonts(ctx);
    // Install egui_extras' image loaders so the baked-in brand PNGs render.
    pixhaus_ui::install_image_loaders(ctx);

    pixhaus_ui::shell::menus::register_shell_menus(&mut host.registrar());

    // sprite-edit FIRST: it registers the shared panels and tools the other
    // modules name by id, so it must run before any layout references them.
    pixhaus_mod_sprite_edit::SpriteEditModule.register(&mut host.registrar());
    pixhaus_mod_animation::AnimationModule.register(&mut host.registrar());
    pixhaus_mod_tiles::TilesModule.register(&mut host.registrar());
    pixhaus_mod_generation::GenerationModule.register(&mut host.registrar());
    pixhaus_mod_export::ExportModule.register(&mut host.registrar());

    // The count is the five module registrations above; bump it when the list grows.
    tracing::info!(modules = 5, "registered capability modules");

    host
}

/// Decode the brand icon PNG into an `egui::IconData` for the OS window/taskbar.
///
/// Returns `None` if the bundled PNG fails to decode - the boot stays infallible and
/// eframe falls back to its default icon. The app owns this because it owns eframe and
/// the window; `ui` only exposes the raw bytes ([`pixhaus_ui::brand::ICON_PNG`]).
fn window_icon() -> Option<egui::IconData> {
    let rgba = image::load_from_memory(pixhaus_ui::brand::ICON_PNG).ok()?.into_rgba8();
    let (width, height) = rgba.dimensions();
    Some(egui::IconData {
        rgba: rgba.into_raw(),
        width,
        height,
    })
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
            // The render state only exists here, not in main — so the chosen backend
            // and adapter are logged at the spot they become known.
            let info = render_state.adapter.get_info();
            tracing::info!(backend = ?info.backend, adapter = %info.name, "renderer initialized");
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
    // Order matters: resolve the log dir, install the subscriber, then log startup —
    // so the first events land in the file. The guard MUST outlive all of main; on
    // its drop the non-blocking file writer flushes, so a `let _ =` would lose the tail.
    let log_dir = pixhaus_platform::log_dir().context("failed to resolve the log directory")?;
    let _guard = diagnostics::init_tracing(&log_dir);
    tracing::info!(
        version = env!("CARGO_PKG_VERSION"),
        os = std::env::consts::OS,
        arch = std::env::consts::ARCH,
        "Pixhaus starting"
    );

    // Localization: launch in the OS language, truncated to the primary subtag
    // ("es-ES" -> "es") to match the locale codes the bundles use. The service falls
    // back to en for any key a language is missing. A saved Prefs.language override
    // will take precedence once Prefs persistence and an in-app picker land (deferred,
    // spec open decision 5). The app owns this, the string-side parallel to owning the
    // one tracing subscriber.
    let language = sys_locale::get_locale().map_or_else(|| "en".to_owned(), |tag| tag.split(['-', '_']).next().unwrap_or("en").to_ascii_lowercase());
    pixhaus_services::i18n::set_language(&language);
    tracing::info!(language = %language, "localization language set");

    // The binary owns the single tokio runtime; entering it makes tokio::spawn
    // available to the egui loop for the background work the editor will grow.
    let runtime = tokio::runtime::Runtime::new().context("failed to start the tokio runtime")?;
    let _rt_guard = runtime.enter();

    let mut viewport = egui::ViewportBuilder::default()
        .with_title("Pixhaus")
        .with_inner_size([1280.0, 800.0])
        .with_min_inner_size([640.0, 480.0]);
    if let Some(icon) = window_icon() {
        viewport = viewport.with_icon(Arc::new(icon));
    }

    let options = eframe::NativeOptions {
        renderer: eframe::Renderer::Wgpu,
        viewport,
        ..Default::default()
    };

    eframe::run_native("pixhaus", options, Box::new(|cc| Ok(Box::new(PixhausApp::new(cc)))))?;

    // run_native returns when the window closes; _guard drops just after this, flushing
    // the file writer, so the shutdown line still reaches the log.
    tracing::info!("Pixhaus shutting down");
    Ok(())
}
