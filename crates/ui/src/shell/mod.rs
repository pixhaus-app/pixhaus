//! The application shell: per-frame region composition, the command palette,
//! shortcut routing, and the menu structure (architecture bible section 8).
//!
//! `Shell::run` is called from `App::ui`; `drain_background` from `App::logic`.

pub mod about;
pub mod command_palette;
pub mod menus;
pub mod regions;
pub mod runtime;
pub mod shortcuts;
pub mod splash;

pub use runtime::Shell;

use pixhaus_services::{JobMsg, JobStatus};

use crate::state::session::AiStatus;
use crate::state::{BackgroundMsg, Host};

/// Drain background-channel and job results into session state, from `App::logic`.
///
/// This is the single mpsc-drain front door (spec "Region composition and the shell
/// runtime"). It drains two channels: the bootstrap `BackgroundMsg` channel, and the
/// `EditSession`'s job channel — a completed generation job's asset is already in the
/// `ResultStore`, so a [`JobMsg`] only refreshes the read-mirror, AI status, and
/// requests a repaint. It runs in `logic`, not `ui`, because `logic` runs even when
/// the window is occluded but a repaint was requested; if anything landed it requests
/// a repaint so the new state shows immediately.
pub fn drain_background(host: &mut Host, ctx: &egui::Context) {
    let mut landed = false;

    // `try_recv` returns `Err` on both an empty and a disconnected channel, so a
    // `while let Ok` cleanly stops draining in either case.
    while let Ok(msg) = host.bg.rx.try_recv() {
        match msg {
            BackgroundMsg::AiStatusChanged(status) => {
                host.state.session.ai_status = status;
                landed = true;
            }
        }
    }

    // Drain job notifications into a buffer first so the channel borrow ends before we
    // mutate the job manager / session in the match.
    let mut job_msgs = Vec::new();
    while let Ok(msg) = host.edit.job_rx.try_recv() {
        job_msgs.push(msg);
    }
    for msg in job_msgs {
        landed = true;
        match msg {
            JobMsg::Status { job, status } => {
                if matches!(status, JobStatus::Cancelled) {
                    host.state.session.ai_status = AiStatus::Ready;
                }
                host.edit.jobs.set_status(job, status);
            }
            JobMsg::Completed { .. } => {
                host.state.session.ai_status = AiStatus::Ready;
            }
            JobMsg::Failed { job, error } => {
                tracing::warn!(?job, %error, "generation job failed");
                host.state.session.ai_status = AiStatus::Ready;
            }
        }
    }

    if landed {
        host.state.session.result_count = host.edit.results.len();
        host.state.session.selected_result = host.edit.results.selected_index();
        ctx.request_repaint();
    }
}
