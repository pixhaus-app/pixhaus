//! IPC commands for crash-reporting opt-in.
//!
//! The TypeScript side owns persistence (via `localStorage`). These
//! commands synchronise the in-process gate that controls whether events
//! are forwarded to the Sentry endpoint, so the Rust panic hook and the
//! user's stated preference stay in sync.

use tauri::command;

use crate::crash_reporting;

/// Returns `true` if crash reporting is currently enabled.
#[command]
pub fn crash_reporting_get_enabled() -> bool {
    crash_reporting::is_enabled()
}

/// Enable or disable crash reporting.
///
/// Call this on startup (passing the stored preference) and whenever the
/// user changes the preference in the Privacy section of Preferences.
#[command]
pub fn crash_reporting_set_enabled(enabled: bool) {
    crash_reporting::set_enabled(enabled);
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Captures the global `crash_reporting::ENABLED` atomic on
    /// construction and restores it on drop. Both tests in this module
    /// mutate the same global, so without this guard the order in which
    /// nextest scheduled them affected later runs (especially when
    /// run with `--test-threads=8`).
    struct EnabledGuard(bool);
    impl Drop for EnabledGuard {
        fn drop(&mut self) {
            crash_reporting::set_enabled(self.0);
        }
    }

    #[test]
    fn get_enabled_returns_current_state() {
        let _lock = crash_reporting::TEST_LOCK
            .lock()
            .unwrap_or_else(|p| p.into_inner());
        let _guard = EnabledGuard(crash_reporting::is_enabled());
        crash_reporting::set_enabled(false);
        assert!(!crash_reporting_get_enabled());
    }

    #[test]
    fn set_enabled_round_trips() {
        let _lock = crash_reporting::TEST_LOCK
            .lock()
            .unwrap_or_else(|p| p.into_inner());
        let _guard = EnabledGuard(crash_reporting::is_enabled());
        crash_reporting::set_enabled(false);
        crash_reporting_set_enabled(true);
        assert!(crash_reporting::is_enabled());
        crash_reporting_set_enabled(false);
        assert!(!crash_reporting::is_enabled());
    }
}
