//! The tracing subscriber: the one place Pixhaus configures logging.
//!
//! The binary owns the single subscriber (architecture bible / runtime companion:
//! "structured tracing from the beginning"). Libraries only ever *emit* events and
//! spans through `tracing`; they never install a subscriber. [`init_tracing`] builds
//! the layered subscriber — a console sink on stderr plus a rolling daily file under
//! the OS log dir — bridges the `log` crate so wgpu/winit/naga records are captured,
//! and registers it as the process-global default.
//!
//! The returned [`WorkerGuard`] flushes the non-blocking file writer on drop, so the
//! caller must hold it for the whole life of `main` — see the lifetime note on
//! [`init_tracing`].

use std::path::Path;

use tracing_appender::non_blocking::WorkerGuard;
use tracing_log::LogTracer;
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::{EnvFilter, Registry, fmt};

/// The default env-filter directive when `RUST_LOG` is unset.
///
/// Targets use underscores (`pixhaus_app`, not `pixhaus-app`) — that is the crate
/// name tracing sees. Debug builds raise every Pixhaus crate to `debug` while keeping
/// the noisy GPU/windowing stack at `warn`; release builds log Pixhaus at `info`.
/// `RUST_LOG` overrides this entirely (see [`init_tracing`]).
const DEBUG_DEFAULT: &str = "info,pixhaus_app=debug,pixhaus_ui=debug,pixhaus_core=debug,\
     pixhaus_render=debug,pixhaus_io=debug,pixhaus_services=debug,pixhaus_platform=debug,\
     wgpu=warn,wgpu_core=warn,wgpu_hal=warn,naga=warn,winit=warn";

/// The release-build default env-filter directive.
const RELEASE_DEFAULT: &str = "info,wgpu=warn,wgpu_core=warn,wgpu_hal=warn,naga=warn,winit=warn";

/// Build the layered subscriber and the file-writer guard, without installing it.
///
/// Split out from [`init_tracing`] so a test can exercise the layering against a
/// throwaway directory without the process-global `.init()` (which can only run once
/// per process). Returns the composed subscriber and the [`WorkerGuard`] whose drop
/// flushes the file writer.
fn build_subscriber(log_dir: &Path) -> (impl tracing::Subscriber + Send + Sync, WorkerGuard) {
    let default = if cfg!(debug_assertions) { DEBUG_DEFAULT } else { RELEASE_DEFAULT };
    // RUST_LOG, when set, overrides the built-in default entirely.
    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new(default));

    let (file_writer, guard) = tracing_appender::non_blocking(tracing_appender::rolling::daily(log_dir, "pixhaus.log"));

    // One shared EnvFilter gates both sinks. Console keeps ANSI for readability; the
    // file drops it because escape codes corrupt a log file.
    let subscriber = Registry::default()
        .with(filter)
        .with(fmt::layer().with_writer(std::io::stderr).with_ansi(true))
        .with(fmt::layer().with_writer(file_writer).with_ansi(false));

    (subscriber, guard)
}

/// Install the process-global tracing subscriber and return its file-writer guard.
///
/// Logs go to two sinks gated by one `EnvFilter`: the console (stderr, with ANSI)
/// and a rolling daily `pixhaus.log` under `log_dir`. `RUST_LOG` overrides the
/// built-in default filter. The `log` crate is bridged into `tracing` so records
/// from wgpu, winit, and naga land in the same place.
///
/// The returned [`WorkerGuard`] MUST be held for the whole life of `main`
/// (`let _guard = init_tracing(...)`, never `let _ = ...`): dropping it flushes and
/// shuts down the non-blocking file writer, so dropping it early truncates the log
/// tail. This is the highest-risk detail in the logging setup.
///
/// Call exactly once at startup. A second call is a no-op for the global default
/// (which can only be set once) but would still allocate a new writer.
pub fn init_tracing(log_dir: &Path) -> WorkerGuard {
    // Bridge `log`-crate records (wgpu/winit/naga) into tracing. We install the
    // LogTracer ourselves rather than letting `SubscriberInitExt::init` do it, then
    // set the subscriber with `set_global_default` below — `init` would try to
    // install the LogTracer a second time and panic with SetLoggerError. The first
    // call wins; a redundant later call is ignored.
    let _ = LogTracer::init();

    let (subscriber, guard) = build_subscriber(log_dir);
    // A second call returns Err (the default is already set); ignore it so a stray
    // re-init never brings the app down over logging.
    let _ = tracing::subscriber::set_global_default(subscriber);
    guard
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn build_subscriber_against_a_tempdir_yields_a_guard_and_does_not_panic() {
        // Use a throwaway dir so the test never writes the real OS log dir, and avoid
        // the global `.init()` so the test is repeatable within the process.
        let tmp = match tempfile::tempdir() {
            Ok(tmp) => tmp,
            Err(err) => panic!("tempdir: {err}"),
        };
        let (subscriber, guard) = build_subscriber(tmp.path());

        // Drive the subscriber locally (no global install) to prove the layering is
        // wired and an event flows without panicking.
        tracing::subscriber::with_default(subscriber, || {
            tracing::info!(test = true, "diagnostics smoke event");
        });

        // Dropping the guard flushes the file writer; the daily file should appear.
        drop(guard);
        let wrote_a_log = match std::fs::read_dir(tmp.path()) {
            Ok(entries) => entries.flatten().any(|e| e.file_name().to_string_lossy().starts_with("pixhaus.log")),
            Err(err) => panic!("read_dir: {err}"),
        };
        assert!(wrote_a_log, "the rolling appender should have written a pixhaus.log file");
    }
}
