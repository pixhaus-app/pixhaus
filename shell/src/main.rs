//! Pixhaus native shell — eframe + egui binary.
//!
//! Owns the application state ([`app::DocumentStore`]), a tokio runtime for AI
//! work, and a results channel pump drained once per frame. The vertical-slice
//! flow (create sprite -> reference sheet -> animation -> playback) is built up
//! over phases P0-P6; see `docs/native-ui-vertical-slice-plan.md`.

#![cfg_attr(
    test,
    allow(
        clippy::unwrap_used,
        clippy::expect_used,
        clippy::panic,
        clippy::disallowed_methods,
        clippy::cast_lossless,
        clippy::cast_possible_truncation
    )
)]

mod ai;
mod anim;
mod app;
mod canvas;
mod document;
mod headless;
mod theme;

use app::ShellApp;

fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
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
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()?;

    let options = eframe::NativeOptions {
        renderer: eframe::Renderer::Wgpu,
        viewport: egui::ViewportBuilder::default()
            .with_title("Pixhaus")
            .with_inner_size([1400.0, 900.0])
            .with_min_inner_size([900.0, 600.0]),
        ..Default::default()
    };

    eframe::run_native(
        "pixhaus",
        options,
        Box::new(move |cc| Ok(Box::new(ShellApp::new(cc, runtime)))),
    )
    .map_err(|e| anyhow::anyhow!("eframe failed to launch: {e}"))
}
