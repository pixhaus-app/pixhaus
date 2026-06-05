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

    // Shell menus register before any module: a free function, not a Module, so it
    // stays out of the module loop below and runs first to seat the File/Edit/Select/
    // View/Window/Help groups ahead of the module-contributed Sprite/Layer/Frame ones.
    pixhaus_ui::shell::menus::register_shell_menus(&mut host.registrar());

    // sprite-edit FIRST: it registers the shared panels and tools the other modules
    // name by id (bible rule 2), so it must register before any layout references them.
    // The order of this array is therefore load-bearing - keep SpriteEditModule at the
    // head. Driving the count from the array length means the log line below can never
    // drift from the real registration list the way a hand-kept constant did.
    let modules: [&dyn Module; 6] = [
        &pixhaus_mod_sprite_edit::SpriteEditModule,
        &pixhaus_mod_animation::AnimationModule,
        &pixhaus_mod_tiles::TilesModule,
        &pixhaus_mod_generation::GenerationModule,
        &pixhaus_mod_export::ExportModule,
        &pixhaus_mod_codex::CodexModule,
    ];
    for module in modules {
        module.register(&mut host.registrar());
    }
    tracing::info!(modules = modules.len(), "registered capability modules");

    // Register generation providers into the host's provider registry so the Generate
    // workspace can answer prompts. Providers register into `EditSession`, not the
    // capability registries, so this is separate from the module loop above. The app
    // owns the API key: read it from the environment (never logged) and register the
    // real OpenRouter provider FIRST so capability lookups prefer it; the offline mock
    // always registers as the fallback so Generate works without a key.
    //
    // The env-var read is an interim seam, not the destination. Bible section 22.4 and
    // modules/providers/CLAUDE.md put credentials in the OS credential vault (the
    // pixhaus-keyring path) with provider settings holding a key reference, never the
    // raw key. That needs a settings-persistence layer and a key-reference model that
    // do not exist yet; until they land, the env var is the only source. When keyring
    // wiring arrives, demote OPENROUTER_API_KEY to an explicit dev-only override rather
    // than the primary path.
    if let Ok(api_key) = std::env::var("OPENROUTER_API_KEY") {
        pixhaus_mod_providers::register_openrouter(&mut host.edit.providers, api_key);
    } else {
        tracing::info!("OPENROUTER_API_KEY not set; the offline mock provider answers generation");
    }
    pixhaus_mod_providers::register(&mut host.edit.providers);

    // Seed the canonical Bit demo Codex so a fresh launch opens into the demo world. The
    // builder is fallible (a broken spec would error), but the shipped spec is known
    // good; on the not-expected error path, log a warning and continue with the empty
    // codex rather than panicking the boot.
    if host.edit.document.codex().entries().is_empty() {
        match pixhaus_services::build_bit_demo_codex() {
            Ok(doc) => host.edit.document = doc,
            Err(err) => tracing::warn!(%err, "failed to seed the Bit demo codex; starting with an empty codex"),
        }
    }

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

#[cfg(test)]
mod tests {
    use super::*;
    use pixhaus_ui::contrib_api::ids::WorkspaceId;

    /// The bundled brand PNG decodes to a square-buffer `IconData`: a tightly packed
    /// RGBA8 buffer whose length is exactly `width * height * 4`. A regression here means
    /// `with_icon` would hand eframe a malformed buffer.
    #[test]
    fn window_icon_decodes_to_packed_rgba() {
        // disallowed_methods bans .expect() even in tests, so destructure with let-else
        // and panic with a message instead, matching the shell test house style.
        let Some(icon) = window_icon() else {
            panic!("the bundled brand PNG must decode at boot");
        };
        assert!(icon.width > 0 && icon.height > 0, "icon must have non-zero dimensions");
        assert_eq!(
            icon.rgba.len(),
            icon.width as usize * icon.height as usize * 4,
            "icon buffer must be tightly packed RGBA8 (4 bytes per pixel, no stride)"
        );
    }

    /// `build_host` registers every shell workspace, and Draw (sprite-edit) registers
    /// first. The order is load-bearing: sprite-edit owns the shared panel and tool ids
    /// the later workspaces name by value, so Draw must precede every workspace that
    /// reuses them. This pins both membership and that head-of-list position.
    #[test]
    fn build_host_registers_workspaces_with_draw_first() {
        let ctx = egui::Context::default();
        let host = build_host(&ctx);

        let ids: Vec<WorkspaceId> = host.registries.workspaces.iter().map(|w| w.id()).collect();

        // Draw is the shared-id owner; it must register before anyone references it.
        assert_eq!(ids.first(), Some(&WorkspaceId("draw")), "sprite-edit/Draw must register first");

        for expected in ["draw", "animate", "tiles", "generate", "export", "codex"] {
            assert!(
                ids.contains(&WorkspaceId(expected)),
                "expected the {expected} workspace to be registered, got {ids:?}"
            );
        }
    }

    /// `build_host` seeds the canonical Bit demo Codex so a fresh launch opens into the
    /// demo world rather than an empty bible. A failed seed logs and leaves the codex
    /// empty, so a non-empty codex confirms the shipped spec still builds.
    #[test]
    fn build_host_seeds_the_bit_demo_codex() {
        let ctx = egui::Context::default();
        let host = build_host(&ctx);
        assert!(
            !host.edit.document.codex().entries().is_empty(),
            "the Bit demo codex should seed a non-empty set of entries at boot"
        );
    }
}
