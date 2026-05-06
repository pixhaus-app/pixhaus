//! Opt-in crash reporting via Sentry.
//!
//! Controlled at runtime by the user's preference. If no DSN was compiled
//! in via `PIXHAUS_SENTRY_DSN`, the module is a no-op: all public symbols
//! still exist so call sites compile identically across configurations.
//!
//! Call [`init`] once at startup (before async work starts), hold the
//! returned [`Guard`] for the full process lifetime, then call
//! [`set_enabled`] whenever the user preference changes. The Sentry
//! client is initialized regardless so the panic hook is always in place;
//! a `before_send` callback gates all events behind the preference flag.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use sentry::protocol::{Event, Exception, Frame, Stacktrace};

/// Whether the user has opted into crash reporting.
static ENABLED: AtomicBool = AtomicBool::new(false);

/// The Sentry DSN compiled in at build time. Set via `PIXHAUS_SENTRY_DSN`
/// during `cargo build` or `pnpm tauri build`.
const DSN: Option<&str> = option_env!("PIXHAUS_SENTRY_DSN");

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
/// installed early. Returns a [`Guard`] that must be held for the full
/// lifetime of the process.
pub fn init() -> Guard {
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
/// while the flag is `false`. Callers are responsible for persisting the
/// preference (the UI owns persistence via `localStorage`).
pub fn set_enabled(enabled: bool) {
    ENABLED.store(enabled, Ordering::Relaxed);
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
        // Save the prior state to restore after the test.
        let prior = is_enabled();
        set_enabled(true);
        assert!(is_enabled());
        set_enabled(false);
        assert!(!is_enabled());
        // Restore so parallel tests see a consistent baseline.
        set_enabled(prior);
    }
}
