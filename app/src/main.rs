//! Pixhaus binary entry point. Delegates to [`pixhaus_app::run`] and surfaces
//! a non-zero exit code on failure.

// Hide the Windows console in release; keep it in debug for stdout/stderr.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
#![allow(clippy::print_stderr)]

use std::process::ExitCode;

fn main() -> ExitCode {
    match pixhaus_app::run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(err) => {
            eprintln!("fatal: {err}");
            ExitCode::FAILURE
        }
    }
}
