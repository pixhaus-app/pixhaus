//! Session state: the per-session, non-durable model the shell owns.
//!
//! Minimal this round. `active_document` / `selection` / `undo_stack` are reserved
//! seams that arrive with `core` (spec "Owners, no overlap"); they are intentionally
//! absent, not forgotten.

use pixhaus_core::ClipId;
use pixhaus_services::ResultKind;

use crate::contrib_api::ids::{ActionId, ToolId, WorkspaceId};

/// Non-durable session state owned by [`crate::state::Host`].
///
/// Reserved for `core`: `active_document`, `selection`, `undo_stack`. They join when
/// `core` has types; until then they would be fake state, so they are left out.
pub struct SessionState {
    /// The workspace currently shown (Draw, Animate, Tiles, Generate, Export).
    pub active_workspace: WorkspaceId,
    /// The tool currently selected in the active workspace's rail.
    pub active_tool: ToolId,
    /// Whether the (future) document has unsaved edits. Mock this round.
    pub dirty: bool,
    /// Mock job entries so the status dot and the console panel have content.
    pub jobs: Vec<JobStub>,
    /// Drives the status-bar AI dot.
    pub ai_status: AiStatus,
    /// Read-only mirror of `EditSession.results.len()`, refreshed post-frame in
    /// `drain_background`. Panels read this; they never touch the result store.
    pub result_count: usize,
    /// Read-only mirror of the selected result index, refreshed post-frame.
    pub selected_result: Option<usize>,
    /// Read-only mirror of each visible result's kind (still anchor vs animation,
    /// with the animation's frame count), refreshed post-frame in `drain_background`.
    /// Panels read this to gate the idle button and draw a frame-count badge without
    /// reaching into the result store. Capped to the visible tray cards.
    pub result_kinds: Vec<ResultKind>,
    /// The last prompt submitted, so "Generate more" can resubmit it without the
    /// Results panel reaching into the Prompt panel's scratch.
    pub last_prompt: String,
    /// Read-only mirror of the active sprite's animation shape, for the Animate
    /// panels (which cannot reach the document). Document-derived fields refresh when
    /// the document changes; `playhead_offset` refreshes each frame from the playhead.
    pub playback: PlaybackMirror,
}

/// What the Animate panels need to render the timeline without touching the
/// document: the active sprite's frame count, its clips, the resolved play range,
/// and the live playhead offset. Panels read playback play/clip state directly from
/// [`UiState`](crate::state::ui_state::UiState); this mirror carries only the
/// document-derived data they cannot reach.
#[derive(Clone, Default, PartialEq, Debug)]
pub struct PlaybackMirror {
    /// Frame count of the active sprite (0 if none). Sizes the timeline ruler.
    pub frame_count: u32,
    /// The active sprite's clips as plain rows (only `ClipId` is echoed back, via
    /// `SelectClip`).
    pub clips: Vec<ClipRow>,
    /// First frame index of the resolved play range (clip start, or 0).
    pub range_start: u32,
    /// Playback rate of the resolved range, for the FPS readout.
    pub range_fps: u16,
    /// The playhead's frame offset within the resolved range, derived each frame from
    /// `UiState::playback.playhead_seconds`. The absolute ruler frame is
    /// `range_start + playhead_offset`.
    pub playhead_offset: u32,
    /// False when there is nothing to play (no active sprite, or a single frame).
    pub playable: bool,
}

/// One clip mirrored for the Animate panels. `name` is project content, not a key.
#[derive(Clone, PartialEq, Debug)]
pub struct ClipRow {
    /// Stable id, echoed back into `Intent::SelectClip`.
    pub id: ClipId,
    /// User-facing clip name.
    pub name: String,
    /// First frame index (inclusive).
    pub start: u32,
    /// Last frame index (inclusive).
    pub end: u32,
    /// Playback rate in frames per second.
    pub fps: u16,
}

impl SessionState {
    /// Whether a generation job is in flight. Panels call this to disable the
    /// job-submitting buttons and show the in-progress indicator.
    pub fn is_generating(&self) -> bool {
        matches!(self.ai_status, AiStatus::Working)
    }

    /// Whether the selected result is a still anchor - the precondition for driving
    /// an idle-animation pass. Panels call this to gate the idle button.
    pub fn selected_is_anchor(&self) -> bool {
        self.selected_result
            .and_then(|i| self.result_kinds.get(i))
            .is_some_and(|kind| matches!(kind, ResultKind::Sprite))
    }

    /// Whether the selected result is an animation (eligible to insert as an
    /// animated sprite).
    pub fn selected_is_animation(&self) -> bool {
        self.selected_result
            .and_then(|i| self.result_kinds.get(i))
            .is_some_and(|kind| matches!(kind, ResultKind::Animation { .. }))
    }

    /// The frame count of the result at `index` if it is an animation, else `None`.
    /// Panels use this to draw a frame-count badge on animation cards.
    pub fn result_frame_count(&self, index: usize) -> Option<u32> {
        match self.result_kinds.get(index) {
            Some(ResultKind::Animation { frames }) => Some(*frames),
            _ => None,
        }
    }
}

/// A stand-in for a real job (spec: bible rule 5). Carries only what the mock
/// status dot and console need; the real job system lands in `services`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct JobStub {
    /// The action that queued this job.
    pub action: ActionId,
    /// Where the job is in its (mock) lifecycle.
    pub state: JobState,
}

/// Mock lifecycle for a [`JobStub`].
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum JobState {
    /// Queued, not yet running.
    Queued,
    /// Finished (mock).
    Done,
}

impl JobStub {
    /// A freshly queued job for `action`.
    pub fn queued(action: ActionId) -> Self {
        Self {
            action,
            state: JobState::Queued,
        }
    }
}

/// The AI runtime status surfaced by the status-bar dot (spec UX 27).
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum AiStatus {
    /// Idle and available (success-colored dot).
    Ready,
    /// A job is running (warning-colored dot).
    Working,
    /// No backend (disabled-colored dot).
    Offline,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn queued_job_carries_its_action_and_is_queued() {
        let action = ActionId("ai.fill");
        let job = JobStub::queued(action);
        assert_eq!(job.action, action, "queued() must record the action it was given");
        assert_eq!(job.state, JobState::Queued, "a fresh job starts Queued");
    }
}
