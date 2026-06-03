//! Intents and events: the one write channel and the post-frame notification bus.
//!
//! An [`Intent`] is a requested change; a contributor pushes intents into an
//! [`IntentSink`] and the shell applies them after the frame's region borrows drop
//! (`apply_intent`, defined alongside [`crate::state::Host`]). An [`Event`] is
//! "something happened", produced only inside `apply_intent` and consumed on the
//! next frame - never read by panels during render, so there is no intra-frame event
//! bus and the borrow guarantee has no hole (spec bible 21.1).

use pixhaus_core::ClipId;
use pixhaus_core::commands::{ApplyGeneratedAnimation, ApplyGeneratedAsset, GeneratedFrameData};
use pixhaus_services::{GenerationContext, GenerationJobInput, GenerationKind, Grid, ProviderCapability, ReferenceImage, i18n};

use crate::contrib_api::ids::{ActionId, PanelId, ToolId, WorkspaceId};
use crate::state::Host;
use crate::state::session::{AiStatus, JobStub};
use crate::state::ui_state::{GridMode, Modal, SplashPhase};
use crate::theme::{Theme, ThemeVariant, apply_to_visuals};

/// A requested change to session or UI state. The single write channel for
/// everything except a panel's own scratch text. Applied post-frame.
pub enum Intent {
    /// Switch the active workspace.
    SelectWorkspace(WorkspaceId),
    /// Select a tool in the active workspace's rail.
    SelectTool(ToolId),
    /// Select a tray tab; applies to the active workspace's tray.
    SelectTrayTab(PanelId),
    /// Toggle a panel's collapse flag.
    TogglePanelCollapsed(PanelId),
    /// Set the canvas grid mode.
    SetGrid(GridMode),
    /// Toggle onion skin (Animate).
    ToggleOnionSkin,
    /// Toggle pixel snap.
    ToggleSnap,
    /// Set canvas zoom.
    SetZoom(f32),
    /// Open the command palette modal.
    OpenCommandPalette,
    /// Open the About Pixhaus modal.
    OpenAbout,
    /// Dismiss any open modal.
    CloseModal,
    /// Stamp the splash start time (frame-clock seconds), once, on its first frame.
    SetSplashStart(f64),
    /// Dismiss the startup splash (timed out or skipped).
    DismissSplash,
    /// Change the theme variant; `apply_intent` re-applies it to egui's visuals.
    SetThemeVariant(ThemeVariant),
    /// Toggle the dev key-display mode: render raw i18n keys instead of their
    /// translations, a built-in lint for hardcoded strings (anything that does not
    /// turn into a key was never routed through the localization service). Flips the
    /// service's process-global flag (bible 32.3).
    ToggleI18nKeys,
    /// Run an action. Mock UI affordance: pushes a `JobStub` and logs an event. Never
    /// mutates project state - model edits route through `Command` below.
    RunAction(ActionId),
    /// Execute an undoable command against the live document through the history (the
    /// named command-path seam, bible rules 3, 4, 13).
    Command(Box<dyn pixhaus_core::Command>),
    /// Undo the most recent command.
    Undo,
    /// Redo the most recently undone command.
    Redo,
    /// Submit an anchor generation job: dispatch the assembled `prompt` through a
    /// provider that offers anchor generation. The first pass of the sprite pipeline.
    SubmitAnchorJob {
        /// The assembled anchor prompt (content, never a localization key).
        prompt: String,
    },
    /// Submit an idle-animation job conditioned on a previously generated anchor.
    /// The second pass: `from_result` is the tray index of the anchor to use as the
    /// reference image; `prompt` is the assembled idle prompt; the grid/timing come
    /// from the generation module's idle defaults.
    SubmitIdleAnimationJob {
        /// The assembled idle prompt (content, never a localization key).
        prompt: String,
        /// Tray index of the anchor result used as the reference image.
        from_result: usize,
        /// Sheet columns to request and slice.
        cols: u32,
        /// Sheet rows to request and slice.
        rows: u32,
        /// Playback rate for the resulting clip.
        fps: u16,
        /// The clip name the result should carry (content, e.g. "idle").
        clip_name: String,
    },
    /// Apply the selected generation result as a new still sprite (an undoable command).
    InsertSelectedResultAsSprite,
    /// Apply the selected animation result as a new animated sprite with a clip (an
    /// undoable command).
    InsertSelectedAsAnimatedSprite,
    /// Select a generation result by its tray index.
    SelectResult(usize),
    /// Toggle animation playback (play/pause). Transient view state; never a command.
    TogglePlayback,
    /// Stop playback: freeze and snap the playhead to the range start.
    StopPlayback,
    /// Scrub the playhead to a frame offset within the active range; pauses playback.
    ScrubToFrame(u32),
    /// Select which clip plays (`None` = the sprite's default range). Resets the clock.
    SelectClip(Option<ClipId>),
}

/// "Something happened", distinct from a command (spec bible 21.3). Produced only
/// inside `apply_intent`, consumed on the next frame. This round it is a
/// `tracing::debug!` sink.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum Event {
    /// The active workspace changed.
    WorkspaceChanged(WorkspaceId),
    /// The active tool changed.
    ToolChanged(ToolId),
    /// An action was dispatched.
    ActionDispatched(ActionId),
}

/// The write channel a contributor pushes [`Intent`]s into during a frame.
#[derive(Default)]
pub struct IntentSink(pub(crate) Vec<Intent>);

impl IntentSink {
    /// Queue an intent for post-frame application.
    pub fn push(&mut self, i: Intent) {
        self.0.push(i);
    }
}

/// Apply one intent to the host, after the frame's region borrows have dropped.
///
/// Takes the `egui::Context` because the theme path must re-apply to egui's visuals
/// on a variant change. `RunAction` is a mock UI affordance: it queues a job and logs
/// an event but NEVER mutates project state (spec invariant) - model edits route
/// through the reserved `Command` variant when `core` lands.
///
/// `intent` is taken by value, not by reference: the reserved `Command(Box<dyn
/// core::Command>)` variant will move an owned command out of the intent, so the
/// function must own it and `Intent` cannot be `Copy`. Today's arms only read `Copy`
/// payloads, which is why clippy sees the value as unconsumed.
// apply_intent is the single intent-dispatch match; its length grows with the
// intent set, so the line-count lint does not apply.
#[allow(clippy::needless_pass_by_value, clippy::too_many_lines)]
pub fn apply_intent(host: &mut Host, intent: Intent, ctx: &egui::Context) {
    match intent {
        Intent::SelectWorkspace(w) => {
            // Switching workspaces pauses playback: the canvas is shared, so a running
            // clock would animate the other workspaces' stages too. The playhead
            // position is kept, so returning to Animate and pressing play resumes it.
            host.state.ui.playback.playing = false;
            host.state.session.active_workspace = w;
            tracing::debug!(?w, "WorkspaceChanged");
        }
        Intent::SelectTool(t) => {
            host.state.session.active_tool = t;
            tracing::debug!(?t, "ToolChanged");
        }
        Intent::SelectTrayTab(p) => {
            let w = host.state.session.active_workspace;
            host.state.ui.tray_tab.insert(w, p);
        }
        Intent::TogglePanelCollapsed(p) => {
            let e = host.state.ui.collapsed.entry(p).or_insert(false);
            *e = !*e;
        }
        Intent::SetGrid(g) => {
            host.state.ui.grid = g;
        }
        Intent::ToggleOnionSkin => {
            host.state.ui.onion_skin = !host.state.ui.onion_skin;
        }
        Intent::ToggleSnap => {
            host.state.ui.snap = !host.state.ui.snap;
        }
        Intent::SetZoom(z) => {
            host.state.ui.zoom = z;
        }
        Intent::OpenCommandPalette => {
            host.state.ui.modal = Some(Modal::CommandPalette);
        }
        Intent::OpenAbout => {
            host.state.ui.modal = Some(Modal::About);
        }
        Intent::CloseModal => {
            host.state.ui.modal = None;
        }
        Intent::SetSplashStart(now) => {
            // Stamp once: the splash overlay pushes this every active frame until the
            // clock is set, so guard against re-stamping and resetting the timer.
            if let SplashPhase::Active { since: since @ None } = &mut host.state.ui.splash {
                *since = Some(now);
            }
        }
        Intent::DismissSplash => {
            host.state.ui.splash = SplashPhase::Done;
        }
        Intent::SetThemeVariant(v) => {
            host.theme = Theme::for_variant(v, host.theme.accent_seed());
            apply_to_visuals(&host.theme, ctx);
        }
        Intent::ToggleI18nKeys => {
            i18n::set_show_keys(!i18n::show_keys());
        }
        Intent::RunAction(a) => {
            host.state.session.jobs.push(JobStub::queued(a));
            tracing::debug!(?a, "ActionDispatched");
        }
        Intent::Command(cmd) => {
            match host.edit.history.execute(&mut host.edit.document, cmd) {
                Ok(()) => host.state.session.dirty = true,
                Err(error) => tracing::warn!(%error, "command failed"),
            }
            // The document changed in the post-frame drain; ask for another frame so
            // the canvas recomposites (egui would otherwise go idle until next input).
            ctx.request_repaint();
        }
        Intent::Undo => {
            if host.edit.history.undo(&mut host.edit.document).is_ok() {
                host.state.session.dirty = true;
            }
            ctx.request_repaint();
        }
        Intent::Redo => {
            if host.edit.history.redo(&mut host.edit.document).is_ok() {
                host.state.session.dirty = true;
            }
            ctx.request_repaint();
        }
        Intent::SelectResult(index) => {
            host.edit.results.select(index);
            host.state.session.selected_result = host.edit.results.selected_index();
        }
        Intent::InsertSelectedResultAsSprite => {
            insert_selected_result(host);
            ctx.request_repaint();
        }
        Intent::InsertSelectedAsAnimatedSprite => {
            insert_selected_as_animated_sprite(host);
            ctx.request_repaint();
        }
        Intent::SubmitAnchorJob { prompt } => {
            submit_anchor_job(host, prompt);
            // Show the "Working" status immediately; the result lands via the job
            // channel `drain_background` drains (which repaints again on completion).
            ctx.request_repaint();
        }
        Intent::SubmitIdleAnimationJob {
            prompt,
            from_result,
            cols,
            rows,
            fps,
            clip_name,
        } => {
            submit_idle_animation_job(host, prompt, from_result, cols, rows, fps, clip_name);
            ctx.request_repaint();
        }
        Intent::TogglePlayback => {
            host.state.ui.playback.playing = !host.state.ui.playback.playing;
            ctx.request_repaint(); // wake the loop so the clock starts advancing now
        }
        Intent::StopPlayback => {
            host.state.ui.playback.playing = false;
            host.state.ui.playback.playhead_seconds = 0.0;
            ctx.request_repaint();
        }
        Intent::ScrubToFrame(offset) => {
            host.state.ui.playback.playing = false;
            let fps = f32::from(playback_fps(host).max(1));
            // Bias to the centre of the target frame's time slice (`+0.5`), so the
            // canvas's `floor(seconds * fps)` round-trips back to exactly `offset`
            // even at fps values where `offset / fps` is not f32-exact.
            #[allow(clippy::cast_precision_loss)]
            let seconds = (offset as f32 + 0.5) / fps;
            host.state.ui.playback.playhead_seconds = seconds;
            ctx.request_repaint();
        }
        Intent::SelectClip(clip) => {
            host.state.ui.playback.clip = clip;
            host.state.ui.playback.playhead_seconds = 0.0; // restart at the new range's start
            ctx.request_repaint();
        }
    }
}

/// The fps of the active sprite's resolved play range, for scrub math (>= 1). Falls
/// back to the default rate when there is no active sprite.
fn playback_fps(host: &Host) -> u16 {
    host.edit
        .document
        .active_sprite()
        .and_then(|id| host.edit.document.sprite(id))
        .map_or(crate::playback::DEFAULT_PLAYBACK_FPS, |sprite| {
            crate::playback::resolve_range(sprite, host.state.ui.playback.clip).fps
        })
}

/// Builds an [`ApplyGeneratedAsset`] from the selected result and executes it through
/// the history, marking the session dirty (a no-op if no result is selected).
fn insert_selected_result(host: &mut Host) {
    let Some(asset) = host.edit.results.selected() else {
        return;
    };
    let command = ApplyGeneratedAsset::new(asset.provenance.prompt.clone(), asset.width, asset.height, asset.stride, asset.rgba);
    match host.edit.history.execute(&mut host.edit.document, Box::new(command)) {
        Ok(()) => host.state.session.dirty = true,
        Err(error) => tracing::warn!(%error, "failed to insert generated asset"),
    }
}

/// Dispatches an anchor generation job through a provider offering anchor
/// generation, if one is registered. The result returns over the job channel
/// `drain_background` drains.
fn submit_anchor_job(host: &mut Host, prompt: String) {
    let Some(provider) = host.edit.providers.first_with(ProviderCapability::GenerateAnchor) else {
        tracing::warn!("no provider offers anchor generation");
        return;
    };
    host.state.session.ai_status = AiStatus::Working;
    // Remember the prompt so "Generate more" can resubmit it from the Results panel.
    host.state.session.last_prompt.clone_from(&prompt);
    // Vary the seed by result count so "Generate more" differs, without a random
    // source (the mock hashes prompt+seed deterministically).
    let seed = host.edit.results.len() as u64;
    let input = GenerationJobInput {
        prompt,
        seed,
        size: pixhaus_core::DEFAULT_CANVAS_SIZE,
        context: GenerationContext::NewAsset,
        kind: GenerationKind::Anchor,
    };
    let results = host.edit.results.clone();
    host.edit.jobs.submit(provider, input, results);
}

/// Dispatches an idle-animation job conditioned on the anchor result at
/// `from_result`, if a provider offers idle-animation generation and that result is
/// a still anchor. The anchor's pixels travel as owned bytes (a [`ReferenceImage`]),
/// never as a live document handle (bible 13.6).
#[allow(clippy::too_many_arguments)]
fn submit_idle_animation_job(host: &mut Host, prompt: String, from_result: usize, cols: u32, rows: u32, fps: u16, clip_name: String) {
    let Some(provider) = host.edit.providers.first_with(ProviderCapability::GenerateIdleAnimation) else {
        tracing::warn!("no provider offers idle-animation generation");
        return;
    };
    let Some(anchor) = host.edit.results.asset_at(from_result) else {
        tracing::warn!(from_result, "anchor result missing or not a still image");
        return;
    };
    host.state.session.ai_status = AiStatus::Working;
    let seed = host.edit.results.len() as u64;
    let reference = ReferenceImage {
        width: anchor.width,
        height: anchor.height,
        stride: anchor.stride,
        rgba: anchor.rgba,
    };
    let input = GenerationJobInput {
        prompt,
        seed,
        size: (anchor.width, anchor.height),
        context: GenerationContext::NewAsset,
        kind: GenerationKind::IdleAnimation {
            reference,
            animation_id: clip_name,
            grid: Grid { cols, rows },
            fps,
        },
    };
    let results = host.edit.results.clone();
    host.edit.jobs.submit(provider, input, results);
}

/// Builds an [`ApplyGeneratedAnimation`] from the selected animation result and
/// executes it through the history (a no-op if the selection is not an animation).
fn insert_selected_as_animated_sprite(host: &mut Host) {
    let Some(animation) = host.edit.results.selected_animation() else {
        return;
    };
    let frames = animation.frames;
    let (width, height) = match frames.first() {
        Some(frame) => (frame.width, frame.height),
        None => return,
    };
    let data: Vec<GeneratedFrameData> = frames
        .into_iter()
        .map(|f| GeneratedFrameData {
            stride: f.stride,
            rgba: f.rgba,
        })
        .collect();
    let command = ApplyGeneratedAnimation::new(
        animation.clip_name.clone(),
        animation.clip_name,
        width,
        height,
        animation.fps,
        animation.loop_mode,
        data,
    );
    match host.edit.history.execute(&mut host.edit.document, Box::new(command)) {
        Ok(()) => host.state.session.dirty = true,
        Err(error) => tracing::warn!(%error, "failed to insert generated animation"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::contrib_api::ids::{ActionId, PanelId, ToolId, WorkspaceId};
    use crate::state::Host;
    use crate::state::session::JobState;
    use crate::state::ui_state::{GridMode, Modal, SplashPhase};
    use crate::theme::{Theme, ThemeVariant};

    fn host() -> Host {
        Host::new(&Theme::dark())
    }

    fn ctx() -> egui::Context {
        // A headless Context: no event loop, no GPU. apply_intent's theme path only
        // touches ctx.style_mut, which a default Context fully supports.
        egui::Context::default()
    }

    #[test]
    fn push_appends_intents_in_order() {
        let mut sink = IntentSink::default();
        sink.push(Intent::SelectWorkspace(WorkspaceId("draw")));
        sink.push(Intent::OpenCommandPalette);
        assert_eq!(sink.0.len(), 2, "both intents are queued");
        assert!(
            matches!(sink.0[0], Intent::SelectWorkspace(WorkspaceId("draw"))),
            "first pushed intent stays first",
        );
        assert!(matches!(sink.0[1], Intent::OpenCommandPalette), "second pushed intent stays second");
    }

    #[test]
    fn select_workspace_flips_active_workspace() {
        let mut host = host();
        apply_intent(&mut host, Intent::SelectWorkspace(WorkspaceId("animate")), &ctx());
        assert_eq!(host.state.session.active_workspace, WorkspaceId("animate"));
    }

    #[test]
    fn select_tool_flips_active_tool() {
        let mut host = host();
        apply_intent(&mut host, Intent::SelectTool(ToolId("eraser")), &ctx());
        assert_eq!(host.state.session.active_tool, ToolId("eraser"));
    }

    #[test]
    fn select_tray_tab_updates_the_active_workspaces_tab() {
        let mut host = host();
        // Default workspace is Draw; the tab should be recorded under "draw".
        apply_intent(&mut host, Intent::SelectTrayTab(PanelId("assets")), &ctx());
        assert_eq!(
            host.state.ui.tray_tab.get(&WorkspaceId("draw")).copied(),
            Some(PanelId("assets")),
            "the tray tab is recorded for the active workspace only",
        );
    }

    #[test]
    fn toggle_panel_collapsed_flips_then_flips_back() {
        let mut host = host();
        let p = PanelId("layers");
        apply_intent(&mut host, Intent::TogglePanelCollapsed(p), &ctx());
        assert_eq!(host.state.ui.collapsed.get(&p).copied(), Some(true), "first toggle collapses");
        apply_intent(&mut host, Intent::TogglePanelCollapsed(p), &ctx());
        assert_eq!(host.state.ui.collapsed.get(&p).copied(), Some(false), "second toggle expands");
    }

    #[test]
    fn set_theme_variant_swaps_the_variant() {
        let mut host = host();
        apply_intent(&mut host, Intent::SetThemeVariant(ThemeVariant::Light), &ctx());
        assert_eq!(host.theme.variant, ThemeVariant::Light, "the variant is swapped on the host theme");
    }

    #[test]
    fn open_command_palette_sets_the_modal() {
        let mut host = host();
        apply_intent(&mut host, Intent::OpenCommandPalette, &ctx());
        assert_eq!(host.state.ui.modal, Some(Modal::CommandPalette));
    }

    #[test]
    fn close_modal_clears_the_modal() {
        let mut host = host();
        apply_intent(&mut host, Intent::OpenCommandPalette, &ctx());
        apply_intent(&mut host, Intent::CloseModal, &ctx());
        assert!(host.state.ui.modal.is_none(), "CloseModal clears whatever was open");
    }

    #[test]
    fn open_about_sets_modal() {
        let mut host = host();
        apply_intent(&mut host, Intent::OpenAbout, &ctx());
        assert_eq!(host.state.ui.modal, Some(Modal::About), "OpenAbout opens the About modal");
    }

    #[test]
    fn close_modal_clears_about() {
        let mut host = host();
        apply_intent(&mut host, Intent::OpenAbout, &ctx());
        apply_intent(&mut host, Intent::CloseModal, &ctx());
        assert!(host.state.ui.modal.is_none(), "CloseModal clears the About modal");
    }

    #[test]
    fn dismiss_splash_sets_done() {
        let mut host = host();
        apply_intent(&mut host, Intent::DismissSplash, &ctx());
        assert_eq!(host.state.ui.splash, SplashPhase::Done, "DismissSplash advances the splash to Done");
    }

    #[test]
    fn set_splash_start_stamps_once() {
        let mut host = host();
        apply_intent(&mut host, Intent::SetSplashStart(1.5), &ctx());
        assert_eq!(
            host.state.ui.splash,
            SplashPhase::Active { since: Some(1.5) },
            "the first stamp records the start time",
        );
        // A second stamp must not reset the clock, or the timer never elapses.
        apply_intent(&mut host, Intent::SetSplashStart(9.0), &ctx());
        assert_eq!(
            host.state.ui.splash,
            SplashPhase::Active { since: Some(1.5) },
            "a later stamp is ignored - the start time is set exactly once",
        );
    }

    #[test]
    fn set_grid_changes_the_grid_mode() {
        let mut host = host();
        apply_intent(&mut host, Intent::SetGrid(GridMode::Px16), &ctx());
        assert_eq!(host.state.ui.grid, GridMode::Px16);
    }

    #[test]
    fn toggle_onion_skin_and_snap_flip_their_flags() {
        let mut host = host();
        let snap0 = host.state.ui.snap;
        apply_intent(&mut host, Intent::ToggleOnionSkin, &ctx());
        apply_intent(&mut host, Intent::ToggleSnap, &ctx());
        assert!(host.state.ui.onion_skin, "onion skin starts false and toggles on");
        assert_eq!(host.state.ui.snap, !snap0, "snap flips from its default");
    }

    #[test]
    fn set_zoom_records_the_zoom() {
        let mut host = host();
        apply_intent(&mut host, Intent::SetZoom(16.0), &ctx());
        assert_eq!(host.state.ui.zoom, 16.0);
    }

    #[test]
    fn toggle_i18n_keys_flips_the_service_flag() {
        let mut host = host();
        let before = i18n::show_keys();
        apply_intent(&mut host, Intent::ToggleI18nKeys, &ctx());
        assert_eq!(i18n::show_keys(), !before, "the toggle flips the dev key-display flag");
        // Restore the process-global flag so it does not leak into other tests.
        apply_intent(&mut host, Intent::ToggleI18nKeys, &ctx());
        assert_eq!(i18n::show_keys(), before, "the flag returns to its original state");
    }

    #[test]
    fn run_action_pushes_a_queued_job_and_never_mutates_session_dirty() {
        let mut host = host();
        let was_dirty = host.state.session.dirty;
        apply_intent(&mut host, Intent::RunAction(ActionId("ai.fill")), &ctx());
        assert_eq!(host.state.session.jobs.len(), 1, "RunAction pushes exactly one JobStub");
        assert_eq!(host.state.session.jobs[0].state, JobState::Queued, "the job is queued");
        assert_eq!(
            host.state.session.dirty, was_dirty,
            "RunAction is a mock UI affordance and must never mutate project state (spec invariant)",
        );
    }

    fn red_pixel_command() -> Box<dyn pixhaus_core::Command> {
        Box::new(pixhaus_core::commands::ApplyGeneratedAsset::new("x".to_owned(), 1, 1, 4, vec![10, 20, 30, 255]))
    }

    #[test]
    fn command_executes_against_the_document_and_marks_dirty() {
        let mut host = host();
        apply_intent(&mut host, Intent::Command(red_pixel_command()), &ctx());
        assert_eq!(host.edit.document.sprites().len(), 1, "the command added a sprite");
        assert!(host.state.session.dirty, "a command marks the session dirty");
    }

    #[test]
    fn undo_then_redo_round_trips_through_intents() {
        let mut host = host();
        apply_intent(&mut host, Intent::Command(red_pixel_command()), &ctx());
        apply_intent(&mut host, Intent::Undo, &ctx());
        assert_eq!(host.edit.document.sprites().len(), 0, "Undo removes the sprite");
        apply_intent(&mut host, Intent::Redo, &ctx());
        assert_eq!(host.edit.document.sprites().len(), 1, "Redo re-adds the sprite");
    }

    #[test]
    fn toggle_playback_flips_playing() {
        let mut host = host();
        apply_intent(&mut host, Intent::TogglePlayback, &ctx());
        assert!(host.state.ui.playback.playing, "toggling from stopped starts playback");
        apply_intent(&mut host, Intent::TogglePlayback, &ctx());
        assert!(!host.state.ui.playback.playing, "toggling again pauses");
    }

    #[test]
    fn stop_playback_clears_playing_and_clock() {
        let mut host = host();
        host.state.ui.playback.playing = true;
        host.state.ui.playback.playhead_seconds = 1.5;
        apply_intent(&mut host, Intent::StopPlayback, &ctx());
        assert!(!host.state.ui.playback.playing, "Stop pauses");
        assert_eq!(host.state.ui.playback.playhead_seconds, 0.0, "Stop snaps the playhead to the start");
    }

    #[test]
    fn scrub_pauses_and_sets_the_clock() {
        let mut host = host();
        host.state.ui.playback.playing = true;
        // No active sprite -> the default 12 fps. Frame 6 biases to the centre of its
        // slice: (6 + 0.5) / 12, which floors back to frame 6.
        apply_intent(&mut host, Intent::ScrubToFrame(6), &ctx());
        assert!(!host.state.ui.playback.playing, "scrubbing pauses");
        let expected = (6.0_f32 + 0.5) / 12.0;
        assert!(
            (host.state.ui.playback.playhead_seconds - expected).abs() < 1e-6,
            "the clock lands at the centre of frame 6's slice"
        );
    }

    #[test]
    fn select_clip_resets_the_clock() {
        let mut host = host();
        host.state.ui.playback.playhead_seconds = 2.0;
        apply_intent(&mut host, Intent::SelectClip(Some(ClipId(3))), &ctx());
        assert_eq!(host.state.ui.playback.clip, Some(ClipId(3)), "the clip is selected");
        assert_eq!(host.state.ui.playback.playhead_seconds, 0.0, "selecting a clip restarts the clock");
    }

    #[test]
    fn switching_workspace_pauses_playback() {
        let mut host = host();
        host.state.ui.playback.playing = true;
        apply_intent(&mut host, Intent::SelectWorkspace(WorkspaceId("draw")), &ctx());
        assert!(!host.state.ui.playback.playing, "leaving for another workspace pauses playback");
    }
}
