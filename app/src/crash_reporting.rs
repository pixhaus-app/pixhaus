//! Opt-in crash reporting via Sentry.
//!
//! Controlled at runtime by the user's preference. If no DSN was compiled
//! in via `PIXHAUS_SENTRY_DSN`, the module is a no-op: all public symbols
//! still exist so call sites compile identically across configurations,
//! but `init()` returns an empty guard and the Sentry panic hook is
//! never installed.
//!
//! Call [`init`] once at startup (before async work starts), hold the
//! returned [`Guard`] for the full process lifetime, then call
//! [`set_enabled`] whenever the user preference changes. When a DSN is
//! present the Sentry client is initialised unconditionally so the
//! panic hook is always wired up; a `before_send` callback gates all
//! events behind the runtime preference flag so opting out drops
//! events at the client rather than skipping the install entirely.

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use sentry::protocol::{Event, Exception, Frame, Stacktrace};

/// Whether the user has opted into crash reporting.
static ENABLED: AtomicBool = AtomicBool::new(false);

/// The Sentry DSN compiled in at build time. Set via `PIXHAUS_SENTRY_DSN`
/// during `cargo build` or `pnpm tauri build`.
const DSN: Option<&str> = option_env!("PIXHAUS_SENTRY_DSN");

/// Persistence file name within the pixhaus config dir.
const CONFIG_FILE: &str = "crash_reporting.txt";

/// Holds the Sentry client transport open for the process lifetime.
///
/// Drop this value to flush pending events and shut down the transport.
/// The `Guard` must outlive all other use of the crash-reporting module.
#[must_use = "dropping the Guard shuts down the Sentry transport prematurely"]
pub struct Guard {
    // Held only for its Drop effect; never read after construction.
    _inner: Option<sentry::ClientInitGuard>,
}

/// Initialise the crash-reporting subsystem.
///
/// Must be called once before any async work starts so the panic hook is
/// installed early. Seeds the in-process gate from the persisted config
/// file (if any) so the user's preference survives restarts. Returns a
/// [`Guard`] that must be held for the full lifetime of the process.
pub fn init() -> Guard {
    // Seed the atomic from disk before anything else so the panic hook —
    // and any panic that fires during early startup — honours the user's
    // choice even before the UI has wired itself up.
    ENABLED.store(read_persisted(), Ordering::Relaxed);

    let dsn = match DSN {
        Some(s) if !s.is_empty() => s,
        _ => return Guard { _inner: None },
    };

    let guard = sentry::init((
        dsn,
        sentry::ClientOptions {
            release: sentry::release_name!(),
            send_default_pii: false,
            attach_stacktrace: false,
            before_send: Some(Arc::new(before_send)),
            ..Default::default()
        },
    ));

    Guard {
        _inner: Some(guard),
    }
}

/// Enable or disable event forwarding at runtime.
///
/// The Sentry client remains initialized; `before_send` gates all events
/// while the flag is `false`. Writes the new value to the persistence
/// file so the choice survives a restart; the UI's `localStorage` mirror
/// is now a cache rather than the source of truth.
pub fn set_enabled(enabled: bool) {
    ENABLED.store(enabled, Ordering::Relaxed);
    write_persisted(enabled);
    tracing::debug!(
        "crash reporting {}",
        if enabled { "enabled" } else { "disabled" }
    );
}

/// Returns `true` if crash reporting is currently enabled.
pub fn is_enabled() -> bool {
    ENABLED.load(Ordering::Relaxed)
}

fn before_send(mut event: Event<'static>) -> Option<Event<'static>> {
    if !ENABLED.load(Ordering::Relaxed) {
        return None;
    }
    scrub(&mut event);
    Some(event)
}

/// Remove PII from an outbound event.
///
/// - `server_name` is cleared to prevent hostname leakage.
/// - Stack frame `abs_path` values that begin with the user's home
///   directory have that prefix replaced with `<user>`.
fn scrub(event: &mut Event<'_>) {
    event.server_name = None;

    let prefix = home_prefix();
    if let Some(home) = &prefix {
        for exc in &mut event.exception.values {
            scrub_exception(exc, home);
        }
    }
}

fn scrub_exception(exc: &mut Exception, home: &str) {
    let Some(st) = &mut exc.stacktrace else {
        return;
    };
    scrub_stacktrace(st, home);
}

fn scrub_stacktrace(st: &mut Stacktrace, home: &str) {
    for frame in &mut st.frames {
        scrub_frame(frame, home);
    }
}

fn scrub_frame(frame: &mut Frame, home: &str) {
    if let Some(path) = &frame.abs_path {
        if let Some(rest) = path.strip_prefix(home) {
            frame.abs_path = Some(format!("<user>{rest}"));
        }
    }
}

/// Best-effort home directory prefix used for PII scrubbing.
///
/// Returns `None` on platforms where the home directory cannot be
/// determined. The deprecated `std::env::home_dir` is used here because
/// it is correct on our supported platforms (Windows 10+, macOS 11+,
/// Linux); the only documented incorrect behavior was on Windows 9x/ME.
fn home_prefix() -> Option<String> {
    #[allow(deprecated)]
    std::env::home_dir().map(|p| p.to_string_lossy().into_owned())
}

/// Resolves the path to the crash-reporting persistence file under the
/// OS config directory. Returns `None` on platforms where the config dir
/// cannot be determined, in which case persistence degrades to in-memory
/// only (the user's choice still works in-session but is lost on quit).
fn config_path() -> Option<PathBuf> {
    dirs::config_dir().map(|d| d.join("pixhaus").join(CONFIG_FILE))
}

/// Reads the persisted opt-in value from the resolved config path. Any
/// failure — missing file, garbage contents, I/O error — falls back to
/// `false`. Errors other than "not found" are logged at warn level so a
/// filesystem regression is visible.
fn read_persisted() -> bool {
    let Some(path) = config_path() else {
        return false;
    };
    read_persisted_from(&path)
}

/// Path-injected variant for tests. Same semantics as [`read_persisted`].
fn read_persisted_from(path: &Path) -> bool {
    match std::fs::read_to_string(path) {
        Ok(s) => matches!(s.trim(), "true" | "1"),
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => false,
        Err(err) => {
            tracing::warn!(
                path = %path.display(),
                error = %err,
                "crash-reporting config read failed",
            );
            false
        }
    }
}

/// Writes the opt-in value to the resolved config path. Failure is logged
/// and swallowed — the in-memory atomic remains authoritative for the
/// current session, only the across-restart persistence is lost.
fn write_persisted(enabled: bool) {
    let Some(path) = config_path() else {
        return;
    };
    write_persisted_to(&path, enabled);
}

/// Path-injected variant for tests. Same semantics as [`write_persisted`].
fn write_persisted_to(path: &Path, enabled: bool) {
    if let Some(parent) = path.parent() {
        if let Err(err) = std::fs::create_dir_all(parent) {
            tracing::warn!(
                path = %parent.display(),
                error = %err,
                "crash-reporting config dir create failed",
            );
            return;
        }
    }
    let body = if enabled { "true" } else { "false" };
    if let Err(err) = std::fs::write(path, body) {
        tracing::warn!(
            path = %path.display(),
            error = %err,
            "crash-reporting config write failed",
        );
    }
}

/// Serialises tests across this crate that mutate the global `ENABLED`
/// atomic. `cargo test --lib` runs tests in parallel threads inside a
/// single process; without this lock, one test's `set_enabled(false)`
/// can race a parallel test's `set_enabled(true)` between the
/// set-and-assert window. Nextest sidesteps the race by giving each
/// test its own process, but CI uses `cargo test`. Tests use
/// `.unwrap_or_else(|p| p.into_inner())` so a panic-poisoned mutex
/// doesn't cascade into spurious failures on later tests.
#[cfg(test)]
pub(crate) static TEST_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scrub_clears_server_name() {
        let mut event = Event {
            server_name: Some("my-machine".into()),
            ..Default::default()
        };
        scrub(&mut event);
        assert!(event.server_name.is_none());
    }

    #[test]
    fn scrub_frame_replaces_home_prefix() {
        let home = "/home/user";
        let mut frame = Frame {
            abs_path: Some("/home/user/code/pixhaus/app/src/lib.rs".to_owned()),
            ..Default::default()
        };
        scrub_frame(&mut frame, home);
        assert_eq!(
            frame.abs_path.as_deref(),
            Some("<user>/code/pixhaus/app/src/lib.rs"),
        );
    }

    #[test]
    fn scrub_frame_leaves_unrelated_path_unchanged() {
        let home = "/home/user";
        let mut frame = Frame {
            abs_path: Some("/usr/lib/rust/libstd.rlib".to_owned()),
            ..Default::default()
        };
        scrub_frame(&mut frame, home);
        assert_eq!(frame.abs_path.as_deref(), Some("/usr/lib/rust/libstd.rlib"),);
    }

    #[test]
    fn scrub_frame_no_path_is_noop() {
        let home = "/home/user";
        let mut frame = Frame {
            abs_path: None,
            ..Default::default()
        };
        scrub_frame(&mut frame, home);
        assert!(frame.abs_path.is_none());
    }

    #[test]
    fn scrub_stacktrace_processes_all_frames() {
        let home = "/home/user";
        let frames = vec![
            Frame {
                abs_path: Some("/home/user/src/main.rs".to_owned()),
                ..Default::default()
            },
            Frame {
                abs_path: Some("/usr/lib/libstd.rlib".to_owned()),
                ..Default::default()
            },
        ];
        let mut st = Stacktrace {
            frames,
            ..Default::default()
        };
        scrub_stacktrace(&mut st, home);
        assert_eq!(st.frames[0].abs_path.as_deref(), Some("<user>/src/main.rs"));
        assert_eq!(
            st.frames[1].abs_path.as_deref(),
            Some("/usr/lib/libstd.rlib"),
        );
    }

    #[test]
    fn is_enabled_reflects_set_enabled() {
        let _lock = TEST_LOCK
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        // Save the prior state to restore after the test.
        let prior = is_enabled();
        set_enabled(true);
        assert!(is_enabled());
        set_enabled(false);
        assert!(!is_enabled());
        // Restore so parallel tests see a consistent baseline.
        set_enabled(prior);
    }

    #[test]
    fn read_persisted_returns_false_when_missing() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("pixhaus").join(CONFIG_FILE);
        assert!(!read_persisted_from(&path));
    }

    #[test]
    fn write_then_read_round_trips_true() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("pixhaus").join(CONFIG_FILE);
        write_persisted_to(&path, true);
        assert!(read_persisted_from(&path));
    }

    #[test]
    fn write_then_read_round_trips_false() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("pixhaus").join(CONFIG_FILE);
        // Write true first so the file exists and is non-trivial.
        write_persisted_to(&path, true);
        write_persisted_to(&path, false);
        assert!(!read_persisted_from(&path));
    }

    #[test]
    fn read_persisted_returns_false_on_garbage() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("pixhaus").join(CONFIG_FILE);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(&path, "banana").unwrap();
        assert!(!read_persisted_from(&path));
    }

    #[test]
    fn read_persisted_accepts_one_as_truthy() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("pixhaus").join(CONFIG_FILE);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(&path, "1\n").unwrap();
        assert!(read_persisted_from(&path));
    }
}
