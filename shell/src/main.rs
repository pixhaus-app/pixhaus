//! Pixhaus native shell — eframe + egui binary.
//!
//! Owns the application state ([`app::DocumentStore`]), a tokio runtime for AI
//! work, and a results channel pump drained once per frame. The vertical-slice
//! flow (create sprite -> reference sheet -> animation -> playback) is built up
//! over phases P0-P6; see `docs/native-ui-vertical-slice-plan.md`.

#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::disallowed_methods))]
// Immediate-mode UI and canvas-coordinate code casts between integer pixel
// coordinates and float screen space constantly; the editing panels carry wide
// match arms and a few wide-signature helpers. These pedantic lints are noise
// here, so they are allowed crate-wide for the shell binary.
#![allow(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    clippy::cast_precision_loss,
    clippy::cast_possible_wrap,
    clippy::cast_lossless,
    clippy::too_many_lines,
    clippy::too_many_arguments,
    clippy::struct_excessive_bools,
    clippy::needless_range_loop,
    clippy::many_single_char_names,
    clippy::items_after_statements
)]

mod ai;
mod anim;
mod anim_jobs;
mod anim_set;
mod app;
mod asset_library;
mod bg_removal;
mod canvas;
mod cockpit;
mod color_picker;
mod commands;
mod context_bar;
mod dnd;
mod document;
mod editor;
mod export;
mod gizmo;
mod gpu;
mod headless;
mod icons;
mod keymap;
mod layer_ops;
mod layers_panel;
mod library;
mod lospec;
mod palette_anim;
mod palette_panel;
mod palette_tools;
mod reveal;
mod settings;
mod studio;
mod theme;
mod timeline_panel;
mod tools_panel;

use app::ShellApp;

fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")))
        .init();

    // Headless subcommands (demo / gen) produce a looping sprite to disk without
    // a window. When none match, fall through to the GUI.
    let args: Vec<String> = std::env::args().skip(1).collect();
    if headless::run(&args)? {
        return Ok(());
    }

    // The shell owns one tokio runtime; every AI verb invocation runs on it and
    // returns over a channel. Build it before the window so a failure here is a
    // clean startup error rather than a panic inside the event loop.
    let runtime = tokio::runtime::Builder::new_multi_thread().enable_all().build()?;

    // A saved GPU preference pins a specific adapter (chosen in Settings). It is
    // applied before the window exists; absent one, egui-wgpu picks the default.
    let saved_gpu = gpu::load();
    let selector = saved_gpu.clone().map(gpu::adapter_selector);

    match run_gui(runtime, selector) {
        Ok(()) => Ok(()),
        // A pinned adapter the machine can't actually drive would otherwise brick
        // every launch. Clear it and retry once on the default adapter.
        Err(err) if saved_gpu.is_some() => {
            tracing::warn!(%err, "launch failed with the saved GPU preference; clearing it and retrying on the default adapter");
            let _ = gpu::save(None);
            let runtime = tokio::runtime::Builder::new_multi_thread().enable_all().build()?;
            run_gui(runtime, None).map_err(|err| anyhow::anyhow!("eframe failed to launch: {err}"))
        }
        Err(err) => Err(anyhow::anyhow!("eframe failed to launch: {err}")),
    }
}

/// Launches the GUI, optionally pinning the wgpu adapter via `selector`. Takes
/// the tokio runtime by value because the app owns it; a retry builds a fresh one.
fn run_gui(runtime: tokio::runtime::Runtime, selector: Option<egui_wgpu::NativeAdapterSelectorMethod>) -> eframe::Result<()> {
    let mut options = eframe::NativeOptions {
        renderer: eframe::Renderer::Wgpu,
        viewport: egui::ViewportBuilder::default()
            .with_title("Pixhaus")
            .with_inner_size([1400.0, 900.0])
            .with_min_inner_size([900.0, 600.0]),
        ..Default::default()
    };
    if let Some(selector) = selector {
        // Keep eframe's WgpuConfiguration defaults (present mode, surface-error
        // policy); only swap in a CreateNew setup that pins the adapter. eframe
        // injects the display handle when it builds the instance.
        let mut setup = egui_wgpu::WgpuSetupCreateNew::without_display_handle();
        setup.native_adapter_selector = Some(selector);
        options.wgpu_options.wgpu_setup = egui_wgpu::WgpuSetup::CreateNew(setup);
    }
    eframe::run_native("pixhaus", options, Box::new(move |cc| Ok(Box::new(ShellApp::new(cc, runtime)))))
}
