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
        host.state.session.result_kinds = host.edit.results.kinds_summary();
        ctx.request_repaint();
    }
}

/// Refresh the read-only playback mirror the Animate panels render from (they cannot
/// reach the document). Rebuilds the document-derived shape — frame count, clips, the
/// resolved play range — and re-derives the playhead offset from the transient
/// playback clock. Runs once per frame in [`Shell::run`]; the rebuilt clip rows are a
/// handful of small allocations (the Animate UI, not the pixel hot path).
pub fn sync_playback_mirror(host: &mut Host) {
    use crate::state::session::{ClipRow, PlaybackMirror};

    let clip = host.state.ui.playback.clip;
    let seconds = host.state.ui.playback.playhead_seconds;

    let mirror = host.edit.document.active_sprite().and_then(|id| host.edit.document.sprite(id)).map(|sprite| {
        let range = crate::playback::resolve_range(sprite, clip);
        let frame_count = u32::try_from(sprite.frames().len()).unwrap_or(u32::MAX);
        let clips = sprite
            .clips()
            .iter()
            .map(|clip| ClipRow {
                id: clip.id,
                name: clip.name.clone(),
                start: clip.start,
                end: clip.end,
                fps: clip.fps,
            })
            .collect();
        PlaybackMirror {
            frame_count,
            clips,
            range_start: range.start,
            range_fps: range.fps,
            playhead_offset: crate::playback::playhead_index(seconds, range.fps, range.frame_count, range.loop_mode),
            // A single-frame sprite has nothing to play; transport stays disabled.
            playable: frame_count > 1,
        }
    });
    host.state.session.playback = mirror.unwrap_or_default();

    // Self-heal: if the active sprite can no longer play (e.g. an undo took the
    // document back to a still or empty sprite), clear the transient playing flag.
    // Otherwise the loop would keep requesting repaints with the transport disabled
    // (it gates on `playable`), and there would be no in-UI way to stop it.
    if !host.state.session.playback.playable {
        host.state.ui.playback.playing = false;
    }
}
