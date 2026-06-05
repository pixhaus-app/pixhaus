//! Intents and events: the one write channel and the post-frame notification bus.
//!
//! An [`Intent`] is a requested change; a contributor pushes intents into an
//! [`IntentSink`] and the shell applies them after the frame's region borrows drop
//! (`apply_intent`, defined alongside [`crate::state::Host`]). An [`Event`] is
//! "something happened", produced only inside `apply_intent` and consumed on the
//! next frame - never read by panels during render, so there is no intra-frame event
//! bus and the borrow guarantee has no hole (spec bible 21.1).

use pixhaus_core::ClipId;
use pixhaus_core::codex::{
    AnchorKind, AnchorStrength, AnimationDetails, CharacterDetails, CodexEntryId, CodexFolderId, CodexHandle, CoverageItemStatus, CoverageLabel, CoverageSlot,
    CoverageTemplateId, EntryStatus, EntryType, GenericDetails, PaletteDetails, PromptFragment, RelationKind, StyleDetails,
};
use pixhaus_core::commands::{ApplyGeneratedAnimation, ApplyGeneratedAsset, BuiltinCoveragePreset, GeneratedFrameData};
use pixhaus_services::{GenerationContext, GenerationJobInput, GenerationKind, Grid, ProviderCapability, ReferenceImage, i18n};

use crate::contrib_api::ids::{ActionId, PanelId, ToolId, WorkspaceId};
use crate::state::Host;
use crate::state::session::{AiStatus, JobStub};
use crate::state::ui_state::{CodexDetailTab, CodexMode, ContextRef, GridMode, Modal, NavFilter, SplashPhase};
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
    /// Set canvas zoom to an absolute scale (screen points per sprite pixel). Clamped.
    SetZoom(f32),
    /// Step the canvas zoom one notch in (`true`) or out (`false`), about the board
    /// center, honoring the pixel-perfect mode. The geometry-free zoom path for menus,
    /// the floating zoom control, and the `+`/`-` keys (the cursor-anchored wheel path
    /// lives in the canvas region, which has the pointer position).
    ZoomStep {
        /// Zoom in when `true`, out when `false`.
        zoom_in: bool,
    },
    /// Set the canvas pan offset in points. The canvas re-clamps it each frame.
    SetPan(egui::Vec2),
    /// Request a fit-to-window: clears the recorded fit so the canvas re-fits the
    /// active sprite to the stage next frame (where the stage geometry is known).
    FitView,
    /// Toggle pixel-perfect zoom (integer-snap) vs. continuous zoom.
    ToggleZoomMode,
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

    // --- Codex workspace ---
    /// Create a new Codex entry of `entry_type` named `name`. Mints a handle from the
    /// name, executes `AddCodexEntry`, and selects the new entry on success.
    CreateCodexEntry {
        /// The entry type to create.
        entry_type: EntryType,
        /// The display name (project content); the handle is derived from it.
        name: String,
    },
    /// Select a Codex entry as the Navigator/editor focus.
    SelectCodexEntry(CodexEntryId),
    /// Delete a Codex entry (an undoable command). Clears the selection if it matched.
    DeleteCodexEntry(CodexEntryId),
    /// Update editable text fields on a Codex entry (an undoable command). A `None`
    /// field leaves the entry's value unchanged.
    UpdateCodexEntryField {
        /// The entry to edit.
        id: CodexEntryId,
        /// New name, or `None` to leave it.
        name: Option<String>,
        /// New description, or `None` to leave it.
        description: Option<String>,
        /// New lore, or `None` to leave it.
        lore: Option<String>,
        /// New visual description, or `None` to leave it.
        visual_description: Option<String>,
        /// New tags, or `None` to leave them.
        tags: Option<Vec<String>>,
    },
    /// Set a Codex entry's lifecycle status (an undoable command).
    SetCodexEntryStatus {
        /// The entry to update.
        id: CodexEntryId,
        /// The new status.
        status: EntryStatus,
    },
    /// Add or replace a Codex entry's anchor of `kind` (an undoable command). One
    /// anchor per kind; setting an existing kind replaces it.
    SetCodexAnchor {
        /// The entry to anchor.
        id: CodexEntryId,
        /// What the anchor pins.
        kind: AnchorKind,
        /// How firmly it holds.
        strength: AnchorStrength,
        /// The anchor statement (project content).
        statement: String,
    },
    /// Remove a Codex entry's anchor of `kind` (an undoable command).
    RemoveCodexAnchor {
        /// The entry to update.
        id: CodexEntryId,
        /// The anchor kind to remove.
        kind: AnchorKind,
    },
    /// Add a relationship edge between two Codex entries (an undoable command).
    AddCodexRelationship {
        /// The source entry.
        from: CodexEntryId,
        /// The relationship kind.
        kind: RelationKind,
        /// The target entry.
        to: CodexEntryId,
    },
    /// Attach a project coverage template to a Codex entry by id, seeding its vacant
    /// slots (an undoable command).
    ApplyCoverageTemplate {
        /// The entry to seed coverage for.
        id: CodexEntryId,
        /// The project template to attach, by id.
        template: CoverageTemplateId,
    },
    /// Apply a built-in coverage preset to a Codex entry: create the matching project
    /// template if absent, then attach it, in one undo step.
    ApplyBuiltinCoverageTemplate {
        /// The entry to seed coverage for.
        id: CodexEntryId,
        /// Which built-in preset to apply.
        preset: BuiltinCoveragePreset,
    },
    /// Create a new project coverage template with `name` and `slots` (an undoable
    /// command). Mints a stable id.
    CreateCoverageTemplate {
        /// The template's display name (project content).
        name: String,
        /// The template's initial slots.
        slots: Vec<CoverageSlot>,
    },
    /// Rename a project coverage template's display name (an undoable command).
    RenameCoverageTemplate {
        /// The template to rename.
        template: CoverageTemplateId,
        /// The new display name (project content).
        name: String,
    },
    /// Delete a project coverage template, detaching it from every entry (an undoable
    /// command). Does not touch coverage-status cells.
    DeleteCoverageTemplate {
        /// The template to delete.
        template: CoverageTemplateId,
    },
    /// Add a slot to a project coverage template (an undoable command). A duplicate key
    /// is rejected by the command.
    AddCoverageSlot {
        /// The template to add the slot to.
        template: CoverageTemplateId,
        /// The new slot (a stable key plus a label).
        slot: CoverageSlot,
    },
    /// Remove a slot from a project coverage template by its key (an undoable command).
    RemoveCoverageSlot {
        /// The template to remove the slot from.
        template: CoverageTemplateId,
        /// The stable key of the slot to remove.
        key: String,
    },
    /// Rename a slot's label in a project coverage template, never its key (an undoable
    /// command). The key stays stable so coverage-status cells survive.
    RenameCoverageSlotLabel {
        /// The template that owns the slot.
        template: CoverageTemplateId,
        /// The stable key of the slot to relabel.
        key: String,
        /// The new label (a key or a literal).
        label: CoverageLabel,
    },
    /// Reorder a slot within a project coverage template (an undoable command). An
    /// out-of-range index is rejected by the command.
    ReorderCoverageSlots {
        /// The template whose slots to reorder.
        template: CoverageTemplateId,
        /// The current slot index.
        from: usize,
        /// The destination index.
        to: usize,
    },
    /// Add a per-entry custom coverage slot, seeding its cell (an undoable command). A
    /// duplicate key is rejected by the command.
    AddEntryCustomSlot {
        /// The entry to add the custom slot to.
        id: CodexEntryId,
        /// The new slot (a stable key plus a literal label).
        slot: CoverageSlot,
    },
    /// Remove a per-entry custom coverage slot by its key (an undoable command).
    RemoveEntryCustomSlot {
        /// The entry to remove the custom slot from.
        id: CodexEntryId,
        /// The stable key of the slot to remove.
        key: String,
    },
    /// Rename a per-entry custom coverage slot's label, never its key (an undoable
    /// command). The key stays stable so the coverage-status cell keyed on it survives.
    RenameEntryCustomSlotLabel {
        /// The entry that owns the custom slot.
        id: CodexEntryId,
        /// The stable key of the slot to relabel.
        key: String,
        /// The new label (a literal for a user-created slot).
        label: CoverageLabel,
    },
    /// Set one coverage slot's status on a Codex entry (an undoable command).
    SetCoverageStatus {
        /// The entry.
        id: CodexEntryId,
        /// The coverage slot key (project content, e.g. "idle").
        slot: String,
        /// The new slot status.
        status: CoverageItemStatus,
    },
    /// Switch the Codex center mode (editor / board / graph / coverage / test).
    SetCodexMode(CodexMode),
    /// Set the Navigator search query; the suggestion mirror rebuilds from it.
    CodexSearch(String),
    /// Pin a Codex entry to the generation context stack at the default strength.
    AddReferenceToContext(CodexEntryId),
    /// Unpin a Codex entry from the generation context stack.
    RemoveReferenceFromContext(CodexEntryId),
    /// Change the strength a pinned reference is weighted at.
    SetReferenceStrength {
        /// The pinned entry.
        id: CodexEntryId,
        /// The new strength.
        strength: AnchorStrength,
    },
    /// Compile a Codex-aware prompt from `user_text` plus the context stack, storing
    /// the inspectable preview. Never submits a job - the artist decides.
    CompileCodexPrompt {
        /// The user's prompt text (with `@`-mentions left in place).
        user_text: String,
    },
    /// Generate a sample for a missing coverage slot: compile a Codex prompt from the
    /// entry's references and submit it through the existing generation job pathway.
    GenerateFromCoverage {
        /// The entry whose coverage slot to fill.
        entry: CodexEntryId,
        /// The coverage slot key (project content, e.g. "attack").
        slot: String,
    },

    // --- Codex CRUD (second pass): identity, fragments, type details, coverage,
    // relationships, and folders ---
    /// Rename an entry's primary `@`-handle (an undoable command). The string is parsed
    /// into a [`CodexHandle`] first; an invalid handle is dropped with a warning.
    SetCodexHandle {
        /// The entry to rename.
        id: CodexEntryId,
        /// The new handle text, without the leading `@` (parsed and validated).
        handle: String,
    },
    /// Add an alias handle to an entry (an undoable command). Parsed and validated.
    AddCodexAlias {
        /// The entry to alias.
        id: CodexEntryId,
        /// The new alias text, without `@`.
        alias: String,
    },
    /// Remove an alias handle from an entry (an undoable command).
    RemoveCodexAlias {
        /// The entry to update.
        id: CodexEntryId,
        /// The alias to remove, without `@`.
        alias: String,
    },
    /// Replace an entry's positive prompt fragments wholesale (an undoable command).
    SetCodexPromptFragments {
        /// The entry to update.
        id: CodexEntryId,
        /// The new fragment list (each fragment carries its inclusion priority).
        fragments: Vec<PromptFragment>,
    },
    /// Replace an entry's negative prompt fragments wholesale (an undoable command).
    SetCodexNegativeFragments {
        /// The entry to update.
        id: CodexEntryId,
        /// The new negative-fragment list.
        fragments: Vec<String>,
    },
    /// Replace a Character entry's type-specific body (an undoable command).
    SetCharacterDetails {
        /// The entry to update.
        id: CodexEntryId,
        /// The full character body.
        body: CharacterDetails,
    },
    /// Replace a Palette entry's type-specific body (an undoable command).
    SetPaletteDetails {
        /// The entry to update.
        id: CodexEntryId,
        /// The full palette body.
        body: PaletteDetails,
    },
    /// Replace a Style entry's type-specific body (an undoable command).
    SetStyleDetails {
        /// The entry to update.
        id: CodexEntryId,
        /// The full style body.
        body: StyleDetails,
    },
    /// Replace an Animation entry's type-specific body (an undoable command).
    SetAnimationDetails {
        /// The entry to update.
        id: CodexEntryId,
        /// The full animation body.
        body: AnimationDetails,
    },
    /// Replace a generic entry's key/value body (an undoable command).
    SetGenericDetails {
        /// The entry to update.
        id: CodexEntryId,
        /// The full generic body.
        body: GenericDetails,
    },
    /// Clear an entry's coverage (an undoable command).
    ClearCoverage {
        /// The entry to clear coverage for.
        id: CodexEntryId,
    },
    /// Remove a relationship edge between two entries (an undoable command). Routes to
    /// the core `RemoveRelationship` command; the inverse of `AddCodexRelationship`.
    RemoveCodexRelationship {
        /// The source entry.
        from: CodexEntryId,
        /// The relationship kind.
        kind: RelationKind,
        /// The target entry.
        to: CodexEntryId,
    },
    /// Change an existing relationship edge's kind in place (an undoable command). Routes
    /// to the core `ChangeRelationshipKind` command, which retypes the edge `from
    /// --old_kind--> to` to `new_kind` in one undo step (no remove + re-add).
    ChangeRelationshipKind {
        /// The source entry.
        from: CodexEntryId,
        /// The kind currently on the edge.
        old_kind: RelationKind,
        /// The target entry.
        to: CodexEntryId,
        /// The kind to retype the edge to.
        new_kind: RelationKind,
    },
    /// Create a folder under `parent` (the codex root when `None`); selects nothing.
    CreateCodexFolder {
        /// The parent folder, or `None` for the codex root.
        parent: Option<CodexFolderId>,
        /// The new folder's display name (project content).
        name: String,
    },
    /// Rename a folder (an undoable command).
    RenameCodexFolder {
        /// The folder to rename.
        id: CodexFolderId,
        /// The new name (project content).
        name: String,
    },
    /// Delete a folder (an undoable command). Its child folders and entries reparent to
    /// the deleted folder's parent.
    DeleteCodexFolder {
        /// The folder to delete.
        id: CodexFolderId,
    },
    /// Move a folder under a new parent (an undoable command). A cycle is rejected by
    /// the command.
    SetCodexFolderParent {
        /// The folder to move.
        id: CodexFolderId,
        /// The new parent, or `None` for the codex root.
        parent: Option<CodexFolderId>,
    },
    /// Move an entry into a folder (the codex root when `None`) (an undoable command).
    SetCodexEntryFolder {
        /// The entry to move.
        entry: CodexEntryId,
        /// The destination folder, or `None` for the codex root.
        folder: Option<CodexFolderId>,
    },

    // --- Codex production-cockpit (the polish pass) ---
    /// Switch the center detail tab for the selected entry.
    SetCodexDetailTab(CodexDetailTab),
    /// Set the Navigator smart filter (none / missing-coverage / broken-refs).
    SetCodexNavFilter(NavFilter),
    /// Pin a Codex entry to the generation context stack (the inspector's pin control;
    /// behaves like `AddReferenceToContext`).
    PinCodexEntry(CodexEntryId),
    /// Unpin a Codex entry from the generation context stack (behaves like
    /// `RemoveReferenceFromContext`).
    UnpinCodexEntry(CodexEntryId),
    /// Duplicate a Codex entry under a fresh unique handle (an undoable command) and
    /// select the clone.
    DuplicateCodexEntry(CodexEntryId),
    /// Promote a Codex entry to canonical status (sugar over `SetCodexEntryStatus`).
    PromoteCodexEntry(CodexEntryId),
    /// Archive a Codex entry (sugar over `SetCodexEntryStatus`).
    ArchiveCodexEntry(CodexEntryId),
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
            host.state.ui.zoom = crate::canvas::view::clamp_scale(z);
        }
        Intent::ZoomStep { zoom_in } => {
            host.state.ui.zoom = crate::canvas::view::zoom_step(host.state.ui.zoom, zoom_in, host.state.ui.pixel_perfect_zoom);
        }
        Intent::SetPan(pan) => {
            host.state.ui.pan = pan;
        }
        Intent::FitView => {
            // Defer the actual fit to the canvas region, which has the stage rect and
            // sprite size; clearing the record makes its "dimensions changed?" check fire.
            host.state.ui.last_fit_size = None;
            ctx.request_repaint();
        }
        Intent::ToggleZoomMode => {
            host.state.ui.pixel_perfect_zoom = !host.state.ui.pixel_perfect_zoom;
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
        Intent::CreateCodexEntry { entry_type, name } => {
            create_codex_entry(host, entry_type, name);
            ctx.request_repaint();
        }
        Intent::SelectCodexEntry(id) => {
            host.state.ui.codex.selected = Some(id);
        }
        Intent::DeleteCodexEntry(id) => {
            execute_codex(host, Box::new(pixhaus_core::commands::DeleteCodexEntry::new(id)));
            if host.state.ui.codex.selected == Some(id) {
                host.state.ui.codex.selected = None;
            }
            host.state.ui.codex.context.retain(|c| c.entry != id);
            ctx.request_repaint();
        }
        Intent::UpdateCodexEntryField {
            id,
            name,
            description,
            lore,
            visual_description,
            tags,
        } => {
            let delta = pixhaus_core::commands::CodexEntryDelta {
                name,
                description,
                lore,
                visual_description,
                tags,
            };
            execute_codex(host, Box::new(pixhaus_core::commands::UpdateCodexEntry::new(id, delta)));
            ctx.request_repaint();
        }
        Intent::SetCodexEntryStatus { id, status } => {
            execute_codex(host, Box::new(pixhaus_core::commands::SetEntryStatus::new(id, status)));
            ctx.request_repaint();
        }
        Intent::SetCodexAnchor { id, kind, strength, statement } => {
            let anchor = pixhaus_core::codex::Anchor::new(kind, strength, statement);
            execute_codex(host, Box::new(pixhaus_core::commands::SetAnchor::new(id, anchor)));
            ctx.request_repaint();
        }
        Intent::RemoveCodexAnchor { id, kind } => {
            execute_codex(host, Box::new(pixhaus_core::commands::RemoveAnchor::new(id, kind)));
            ctx.request_repaint();
        }
        Intent::AddCodexRelationship { from, kind, to } => {
            let rel = pixhaus_core::codex::Relationship::new(from, kind, to);
            execute_codex(host, Box::new(pixhaus_core::commands::AddRelationship::new(rel)));
            ctx.request_repaint();
        }
        Intent::ApplyCoverageTemplate { id, template } => {
            execute_codex(host, Box::new(pixhaus_core::commands::ApplyCoverageTemplate::new(id, template)));
            ctx.request_repaint();
        }
        Intent::ApplyBuiltinCoverageTemplate { id, preset } => {
            execute_codex(host, Box::new(pixhaus_core::commands::ApplyBuiltinCoverageTemplate::new(id, preset)));
            ctx.request_repaint();
        }
        Intent::CreateCoverageTemplate { name, slots } => {
            execute_codex(host, Box::new(pixhaus_core::commands::CreateCoverageTemplate::new(name, slots)));
            ctx.request_repaint();
        }
        Intent::RenameCoverageTemplate { template, name } => {
            execute_codex(host, Box::new(pixhaus_core::commands::RenameCoverageTemplate::new(template, name)));
            ctx.request_repaint();
        }
        Intent::DeleteCoverageTemplate { template } => {
            execute_codex(host, Box::new(pixhaus_core::commands::DeleteCoverageTemplate::new(template)));
            ctx.request_repaint();
        }
        Intent::AddCoverageSlot { template, slot } => {
            execute_codex(host, Box::new(pixhaus_core::commands::AddCoverageSlot::new(template, slot)));
            ctx.request_repaint();
        }
        Intent::RemoveCoverageSlot { template, key } => {
            execute_codex(host, Box::new(pixhaus_core::commands::RemoveCoverageSlot::new(template, key)));
            ctx.request_repaint();
        }
        Intent::RenameCoverageSlotLabel { template, key, label } => {
            execute_codex(host, Box::new(pixhaus_core::commands::RenameCoverageSlotLabel::new(template, key, label)));
            ctx.request_repaint();
        }
        Intent::ReorderCoverageSlots { template, from, to } => {
            execute_codex(host, Box::new(pixhaus_core::commands::ReorderCoverageSlots::new(template, from, to)));
            ctx.request_repaint();
        }
        Intent::AddEntryCustomSlot { id, slot } => {
            execute_codex(host, Box::new(pixhaus_core::commands::AddEntryCustomSlot::new(id, slot)));
            ctx.request_repaint();
        }
        Intent::RemoveEntryCustomSlot { id, key } => {
            execute_codex(host, Box::new(pixhaus_core::commands::RemoveEntryCustomSlot::new(id, key)));
            ctx.request_repaint();
        }
        Intent::RenameEntryCustomSlotLabel { id, key, label } => {
            execute_codex(host, Box::new(pixhaus_core::commands::RenameEntryCustomSlotLabel::new(id, key, label)));
            ctx.request_repaint();
        }
        Intent::SetCoverageStatus { id, slot, status } => {
            execute_codex(host, Box::new(pixhaus_core::commands::SetCoverageStatus::new(id, slot, status)));
            ctx.request_repaint();
        }
        Intent::SetCodexMode(mode) => {
            host.state.ui.codex.mode = mode;
        }
        Intent::CodexSearch(query) => {
            host.state.ui.codex.search = query;
        }
        Intent::AddReferenceToContext(id) => {
            if !host.state.ui.codex.context.iter().any(|c| c.entry == id) {
                host.state.ui.codex.context.push(ContextRef {
                    entry: id,
                    strength: AnchorStrength::default(),
                });
            }
        }
        Intent::RemoveReferenceFromContext(id) => {
            host.state.ui.codex.context.retain(|c| c.entry != id);
        }
        Intent::SetReferenceStrength { id, strength } => {
            if let Some(c) = host.state.ui.codex.context.iter_mut().find(|c| c.entry == id) {
                c.strength = strength;
            }
        }
        Intent::CompileCodexPrompt { user_text } => {
            host.state.ui.codex.compiled = Some(compile_codex_prompt(host, &user_text));
        }
        Intent::GenerateFromCoverage { entry, slot } => {
            generate_from_coverage(host, entry, &slot);
            ctx.request_repaint();
        }
        Intent::SetCodexHandle { id, handle } => {
            match CodexHandle::new(handle) {
                Ok(h) => execute_codex(host, Box::new(pixhaus_core::commands::SetCodexHandle::new(id, h))),
                Err(error) => tracing::warn!(%error, "rejected an invalid codex handle"),
            }
            ctx.request_repaint();
        }
        Intent::AddCodexAlias { id, alias } => {
            match CodexHandle::new(alias) {
                Ok(h) => execute_codex(host, Box::new(pixhaus_core::commands::AddCodexAlias::new(id, h))),
                Err(error) => tracing::warn!(%error, "rejected an invalid codex alias"),
            }
            ctx.request_repaint();
        }
        Intent::RemoveCodexAlias { id, alias } => {
            match CodexHandle::new(alias) {
                Ok(h) => execute_codex(host, Box::new(pixhaus_core::commands::RemoveCodexAlias::new(id, h))),
                Err(error) => tracing::warn!(%error, "rejected an invalid codex alias"),
            }
            ctx.request_repaint();
        }
        Intent::SetCodexPromptFragments { id, fragments } => {
            execute_codex(host, Box::new(pixhaus_core::commands::SetPromptFragments::new(id, fragments)));
            ctx.request_repaint();
        }
        Intent::SetCodexNegativeFragments { id, fragments } => {
            execute_codex(host, Box::new(pixhaus_core::commands::SetNegativeFragments::new(id, fragments)));
            ctx.request_repaint();
        }
        Intent::SetCharacterDetails { id, body } => {
            execute_codex(host, Box::new(pixhaus_core::commands::SetCharacterDetails::new(id, body)));
            ctx.request_repaint();
        }
        Intent::SetPaletteDetails { id, body } => {
            execute_codex(host, Box::new(pixhaus_core::commands::SetPaletteDetails::new(id, body)));
            ctx.request_repaint();
        }
        Intent::SetStyleDetails { id, body } => {
            execute_codex(host, Box::new(pixhaus_core::commands::SetStyleDetails::new(id, body)));
            ctx.request_repaint();
        }
        Intent::SetAnimationDetails { id, body } => {
            execute_codex(host, Box::new(pixhaus_core::commands::SetAnimationDetails::new(id, body)));
            ctx.request_repaint();
        }
        Intent::SetGenericDetails { id, body } => {
            execute_codex(host, Box::new(pixhaus_core::commands::SetGenericDetails::new(id, body)));
            ctx.request_repaint();
        }
        Intent::ClearCoverage { id } => {
            execute_codex(host, Box::new(pixhaus_core::commands::ClearCoverage::new(id)));
            ctx.request_repaint();
        }
        Intent::RemoveCodexRelationship { from, kind, to } => {
            let rel = pixhaus_core::codex::Relationship::new(from, kind, to);
            execute_codex(host, Box::new(pixhaus_core::commands::RemoveRelationship::new(rel)));
            ctx.request_repaint();
        }
        Intent::ChangeRelationshipKind { from, old_kind, to, new_kind } => {
            execute_codex(
                host,
                Box::new(pixhaus_core::commands::ChangeRelationshipKind::new(from, old_kind, to, new_kind)),
            );
            ctx.request_repaint();
        }
        Intent::CreateCodexFolder { parent, name } => {
            execute_codex(host, Box::new(pixhaus_core::commands::CreateCodexFolder::new(parent, name)));
            ctx.request_repaint();
        }
        Intent::RenameCodexFolder { id, name } => {
            execute_codex(host, Box::new(pixhaus_core::commands::RenameCodexFolder::new(id, name)));
            ctx.request_repaint();
        }
        Intent::DeleteCodexFolder { id } => {
            execute_codex(host, Box::new(pixhaus_core::commands::DeleteCodexFolder::new(id)));
            ctx.request_repaint();
        }
        Intent::SetCodexFolderParent { id, parent } => {
            execute_codex(host, Box::new(pixhaus_core::commands::SetCodexFolderParent::new(id, parent)));
            ctx.request_repaint();
        }
        Intent::SetCodexEntryFolder { entry, folder } => {
            execute_codex(host, Box::new(pixhaus_core::commands::SetCodexEntryFolder::new(entry, folder)));
            ctx.request_repaint();
        }
        Intent::SetCodexDetailTab(tab) => {
            host.state.ui.codex.detail_tab = tab;
        }
        Intent::SetCodexNavFilter(filter) => {
            host.state.ui.codex.nav_filter = filter;
        }
        Intent::PinCodexEntry(id) => {
            if !host.state.ui.codex.context.iter().any(|c| c.entry == id) {
                host.state.ui.codex.context.push(ContextRef {
                    entry: id,
                    strength: AnchorStrength::default(),
                });
            }
        }
        Intent::UnpinCodexEntry(id) => {
            host.state.ui.codex.context.retain(|c| c.entry != id);
        }
        Intent::DuplicateCodexEntry(id) => {
            duplicate_codex_entry(host, id);
            ctx.request_repaint();
        }
        Intent::PromoteCodexEntry(id) => {
            execute_codex(host, Box::new(pixhaus_core::commands::SetEntryStatus::new(id, EntryStatus::Canonical)));
            ctx.request_repaint();
        }
        Intent::ArchiveCodexEntry(id) => {
            execute_codex(host, Box::new(pixhaus_core::commands::SetEntryStatus::new(id, EntryStatus::Archived)));
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
    // A generated result reaches the canvas only as an undoable command run through
    // the history, never by mutating the live document directly. This keeps generation
    // results on the same undo/redo path as every other edit and means the job worker
    // only ever produces immutable input the artist later chooses to apply.
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

/// Execute a Codex command through the history, marking the session dirty on success
/// and warning on a typed failure. The single Codex command-path seam (bible rules 3,
/// 4): every Codex model mutation routes through here.
fn execute_codex(host: &mut Host, command: Box<dyn pixhaus_core::Command>) {
    match host.edit.history.execute(&mut host.edit.document, command) {
        Ok(()) => host.state.session.dirty = true,
        Err(error) => tracing::warn!(%error, "codex command failed"),
    }
}

/// Derive a stable handle from a display name: lowercase, non-alphanumerics to
/// underscores, leading digits stripped (a handle's first char must be a letter).
/// Falls back to "entry" when the name has no usable characters.
fn handle_from_name(name: &str) -> String {
    let mut out = String::new();
    for ch in name.chars() {
        if ch.is_ascii_alphanumeric() {
            out.push(ch.to_ascii_lowercase());
        } else if (ch.is_whitespace() || ch == '-' || ch == '_') && !out.ends_with('_') && !out.is_empty() {
            out.push('_');
        }
    }
    let trimmed = out.trim_matches('_');
    // The handle must start with a letter; strip a leading digit run.
    let body: String = trimmed.chars().skip_while(char::is_ascii_digit).collect();
    let body = body.trim_matches('_');
    if body.is_empty() { "entry".to_owned() } else { body.to_owned() }
}

/// Create a Codex entry: mint a unique handle from the name, execute `AddCodexEntry`,
/// and select the new entry on success. The handle gains a numeric suffix if the base
/// is already taken, so two "Hero" entries do not collide.
fn create_codex_entry(host: &mut Host, entry_type: EntryType, name: String) {
    let base = handle_from_name(&name);
    // Find a free handle: base, base_2, base_3, ...
    let mut candidate = base.clone();
    let mut n = 2u32;
    let handle = loop {
        match pixhaus_core::codex::CodexHandle::new(&candidate) {
            Ok(h) if !host.edit.document.codex().handle_in_use(&h) => break h,
            _ => {
                candidate = format!("{base}_{n}");
                n += 1;
                if n > 9999 {
                    tracing::warn!("could not mint a free codex handle");
                    return;
                }
            }
        }
    };
    // Keep a copy of the handle to re-resolve the new entry's id after the command is
    // boxed and executed (the history owns the command, so we cannot read its
    // `inserted_id` back out).
    let lookup = handle.clone();
    let proto = pixhaus_core::commands::CodexEntryProto { handle, name, entry_type };
    match host
        .edit
        .history
        .execute(&mut host.edit.document, Box::new(pixhaus_core::commands::AddCodexEntry::new(proto)))
    {
        Ok(()) => {
            host.state.session.dirty = true;
            if let Some(id) = host.edit.document.codex().resolve_handle(&lookup) {
                host.state.ui.codex.selected = Some(id);
            }
        }
        Err(error) => tracing::warn!(%error, "failed to add codex entry"),
    }
}

/// Duplicate a Codex entry: execute the core `DuplicateCodexEntry` command through the
/// history and select the clone. The history owns the boxed command after execution, so
/// the clone's id is recovered by diffing the entry-id set before and after (the same
/// re-resolve pattern `create_codex_entry` uses for a freshly minted entry).
fn duplicate_codex_entry(host: &mut Host, source: CodexEntryId) {
    let before: std::collections::HashSet<CodexEntryId> = host.edit.document.codex().entries().keys().copied().collect();
    execute_codex(host, Box::new(pixhaus_core::commands::DuplicateCodexEntry::new(source)));
    let clone = host.edit.document.codex().entries().keys().copied().find(|id| !before.contains(id));
    if let Some(id) = clone {
        host.state.ui.codex.selected = Some(id);
    }
}

/// Compile a Codex-aware prompt from `user_text` plus the current context stack. The
/// pinned references are appended as `@handle` mentions, the whole text is resolved
/// against the live Codex, and the resolved references feed the compiler. A pure read
/// over the document - it submits nothing.
fn compile_codex_prompt(host: &Host, user_text: &str) -> pixhaus_services::codex::CompiledPrompt {
    let codex = host.edit.document.codex();
    let mut text = user_text.to_owned();
    for c in &host.state.ui.codex.context {
        if let Some(entry) = codex.entry(c.entry) {
            text.push_str(" @");
            text.push_str(entry.handle.as_str());
        }
    }
    let report = pixhaus_services::codex::resolve_text(codex, &text);
    let request = pixhaus_services::codex::PromptRequest {
        user_request: text,
        references: report.resolved,
        project_rules: Vec::new(),
        project_negatives: Vec::new(),
        budget: None,
    };
    pixhaus_services::codex::compile(codex, &request)
}

/// Generate a sample for a missing coverage slot: compile a Codex prompt seeded with
/// the entry's handle and the slot name, then submit it through the existing anchor
/// generation pathway. AI proposes; promoting the result stays a separate command.
fn generate_from_coverage(host: &mut Host, entry: CodexEntryId, slot: &str) {
    let user_text = {
        let Some(e) = host.edit.document.codex().entry(entry) else {
            tracing::warn!(?entry, "coverage generation target is gone");
            return;
        };
        format!("@{} {slot}", e.handle.as_str())
    };
    let compiled = compile_codex_prompt(host, &user_text);
    if compiled.positive.is_empty() {
        tracing::warn!(slot, "compiled coverage prompt is empty; not submitting");
        return;
    }
    submit_anchor_job(host, compiled.positive);
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
    fn set_zoom_clamps_out_of_range_values() {
        let mut host = host();
        apply_intent(&mut host, Intent::SetZoom(1000.0), &ctx());
        assert_eq!(host.state.ui.zoom, crate::canvas::view::MAX_SCALE, "an over-large zoom clamps to the ceiling");
        apply_intent(&mut host, Intent::SetZoom(0.0), &ctx());
        assert_eq!(host.state.ui.zoom, crate::canvas::view::MIN_SCALE, "a zero zoom clamps to the floor");
    }

    #[test]
    fn zoom_step_moves_one_notch_and_saturates() {
        let mut host = host();
        host.state.ui.zoom = 2.0; // pixel-perfect mode is the default
        apply_intent(&mut host, Intent::ZoomStep { zoom_in: true }, &ctx());
        assert_eq!(host.state.ui.zoom, 3.0, "a pixel-perfect step lands on the next integer scale");
        for _ in 0..200 {
            apply_intent(&mut host, Intent::ZoomStep { zoom_in: true }, &ctx());
        }
        assert_eq!(
            host.state.ui.zoom,
            crate::canvas::view::MAX_SCALE,
            "repeated zoom-in saturates, never overflows"
        );
    }

    #[test]
    fn set_pan_records_the_pan() {
        let mut host = host();
        apply_intent(&mut host, Intent::SetPan(egui::vec2(12.0, -7.0)), &ctx());
        assert_eq!(host.state.ui.pan, egui::vec2(12.0, -7.0));
    }

    #[test]
    fn fit_view_clears_the_recorded_fit() {
        let mut host = host();
        host.state.ui.last_fit_size = Some((64, 64));
        apply_intent(&mut host, Intent::FitView, &ctx());
        assert!(host.state.ui.last_fit_size.is_none(), "FitView clears the record so the canvas re-fits");
    }

    #[test]
    fn toggle_zoom_mode_flips_pixel_perfect() {
        let mut host = host();
        let before = host.state.ui.pixel_perfect_zoom;
        apply_intent(&mut host, Intent::ToggleZoomMode, &ctx());
        assert_eq!(host.state.ui.pixel_perfect_zoom, !before, "the mode flips");
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

    // --- Codex intents ---

    /// The handle deriver lowercases, underscores separators, strips leading digits,
    /// and falls back to "entry" for an unusable name.
    #[test]
    fn handle_from_name_normalizes() {
        assert_eq!(handle_from_name("Mossy Stone"), "mossy_stone");
        assert_eq!(handle_from_name("  Bit!! "), "bit");
        assert_eq!(handle_from_name("123abc"), "abc");
        assert_eq!(handle_from_name("!!!"), "entry");
    }

    /// Creating a Codex entry adds it to the document, marks the session dirty, and
    /// selects the new entry.
    #[test]
    fn create_codex_entry_adds_and_selects() {
        let mut host = host();
        apply_intent(
            &mut host,
            Intent::CreateCodexEntry {
                entry_type: EntryType::Character,
                name: "Bit".to_owned(),
            },
            &ctx(),
        );
        assert_eq!(host.edit.document.codex().entries().len(), 1, "the entry was added");
        assert!(host.state.session.dirty, "creating an entry marks the session dirty");
        assert!(host.state.ui.codex.selected.is_some(), "the new entry is selected");
    }

    /// Two entries with the same name get distinct handles (the second gains a suffix),
    /// so the second add does not fail on a duplicate handle.
    #[test]
    fn create_codex_entry_dedupes_handles() {
        let mut host = host();
        let make = |host: &mut Host| {
            apply_intent(
                host,
                Intent::CreateCodexEntry {
                    entry_type: EntryType::Prop,
                    name: "Crate".to_owned(),
                },
                &ctx(),
            );
        };
        make(&mut host);
        make(&mut host);
        assert_eq!(host.edit.document.codex().entries().len(), 2, "both entries added under distinct handles");
    }

    /// Selecting, then deleting the selection clears the selection and removes the entry.
    #[test]
    fn delete_codex_entry_clears_selection() {
        let mut host = host();
        apply_intent(
            &mut host,
            Intent::CreateCodexEntry {
                entry_type: EntryType::Character,
                name: "Bit".to_owned(),
            },
            &ctx(),
        );
        let Some(id) = host.state.ui.codex.selected else {
            panic!("the created entry should be selected");
        };
        apply_intent(&mut host, Intent::DeleteCodexEntry(id), &ctx());
        assert_eq!(host.edit.document.codex().entries().len(), 0, "the entry was removed");
        assert!(host.state.ui.codex.selected.is_none(), "deleting the selected entry clears the selection");
    }

    /// Setting an entry's status routes through a command and changes the model.
    #[test]
    fn set_status_updates_the_entry() {
        let mut host = host();
        apply_intent(
            &mut host,
            Intent::CreateCodexEntry {
                entry_type: EntryType::Style,
                name: "Painterly".to_owned(),
            },
            &ctx(),
        );
        let Some(id) = host.state.ui.codex.selected else {
            panic!("created entry should be selected");
        };
        apply_intent(
            &mut host,
            Intent::SetCodexEntryStatus {
                id,
                status: EntryStatus::Canonical,
            },
            &ctx(),
        );
        let status = host.edit.document.codex().entry(id).map(|e| e.status);
        assert_eq!(status, Some(EntryStatus::Canonical), "the status command applied");
    }

    /// Setting then removing an anchor round-trips through commands.
    #[test]
    fn set_then_remove_anchor() {
        let mut host = host();
        apply_intent(
            &mut host,
            Intent::CreateCodexEntry {
                entry_type: EntryType::Character,
                name: "Bit".to_owned(),
            },
            &ctx(),
        );
        let Some(id) = host.state.ui.codex.selected else {
            panic!("created entry should be selected");
        };
        apply_intent(
            &mut host,
            Intent::SetCodexAnchor {
                id,
                kind: AnchorKind::Visual,
                strength: AnchorStrength::Locked,
                statement: "round head, two eyes".to_owned(),
            },
            &ctx(),
        );
        assert!(
            host.edit
                .document
                .codex()
                .entry(id)
                .is_some_and(|e| e.anchor_position(AnchorKind::Visual).is_some()),
            "the anchor was set",
        );
        apply_intent(&mut host, Intent::RemoveCodexAnchor { id, kind: AnchorKind::Visual }, &ctx());
        assert!(
            host.edit
                .document
                .codex()
                .entry(id)
                .is_some_and(|e| e.anchor_position(AnchorKind::Visual).is_none()),
            "the anchor was removed",
        );
    }

    /// Codex mode and search are pure UI-state writes, no command.
    #[test]
    fn set_mode_and_search_write_ui_state() {
        let mut host = host();
        apply_intent(&mut host, Intent::SetCodexMode(CodexMode::Graph), &ctx());
        assert_eq!(host.state.ui.codex.mode, CodexMode::Graph);
        apply_intent(&mut host, Intent::CodexSearch("bit".to_owned()), &ctx());
        assert_eq!(host.state.ui.codex.search, "bit");
    }

    /// Pinning, restrengthening, and unpinning a context reference.
    #[test]
    fn context_stack_pin_strength_unpin() {
        let mut host = host();
        apply_intent(
            &mut host,
            Intent::CreateCodexEntry {
                entry_type: EntryType::Palette,
                name: "Moonlit".to_owned(),
            },
            &ctx(),
        );
        let Some(id) = host.state.ui.codex.selected else {
            panic!("created entry should be selected");
        };
        apply_intent(&mut host, Intent::AddReferenceToContext(id), &ctx());
        assert_eq!(host.state.ui.codex.context.len(), 1, "the reference was pinned");
        // Pinning again does not duplicate.
        apply_intent(&mut host, Intent::AddReferenceToContext(id), &ctx());
        assert_eq!(host.state.ui.codex.context.len(), 1, "pinning is idempotent");
        apply_intent(
            &mut host,
            Intent::SetReferenceStrength {
                id,
                strength: AnchorStrength::Strong,
            },
            &ctx(),
        );
        assert_eq!(
            host.state.ui.codex.context.first().map(|c| c.strength),
            Some(AnchorStrength::Strong),
            "the strength was set",
        );
        apply_intent(&mut host, Intent::RemoveReferenceFromContext(id), &ctx());
        assert!(host.state.ui.codex.context.is_empty(), "the reference was unpinned");
    }

    /// Compiling a Codex prompt stores an inspectable preview without submitting a job.
    #[test]
    fn compile_codex_prompt_stores_preview() {
        let mut host = host();
        apply_intent(
            &mut host,
            Intent::CompileCodexPrompt {
                user_text: "a knight idle".to_owned(),
            },
            &ctx(),
        );
        assert!(host.state.ui.codex.compiled.is_some(), "the compiled preview is stored");
        let positive = host.state.ui.codex.compiled.as_ref().map(|c| c.positive.clone()).unwrap_or_default();
        assert!(positive.contains("knight"), "the user request leads the positive prompt");
    }

    /// Seed an entry and return its id, for the CRUD intent tests.
    fn seed_entry(host: &mut Host, entry_type: EntryType, name: &str) -> CodexEntryId {
        apply_intent(
            host,
            Intent::CreateCodexEntry {
                entry_type,
                name: name.to_owned(),
            },
            &ctx(),
        );
        match host.state.ui.codex.selected {
            Some(id) => id,
            None => panic!("the created entry should be selected"),
        }
    }

    /// Renaming an entry's handle routes through `SetCodexHandle` and changes the model.
    #[test]
    fn set_handle_renames_the_entry() {
        let mut host = host();
        let id = seed_entry(&mut host, EntryType::Character, "Bit");
        apply_intent(
            &mut host,
            Intent::SetCodexHandle {
                id,
                handle: "bitter".to_owned(),
            },
            &ctx(),
        );
        let handle = host.edit.document.codex().entry(id).map(|e| e.handle.as_str().to_owned());
        assert_eq!(handle.as_deref(), Some("bitter"), "the handle command applied");
    }

    /// An invalid handle is rejected (no command runs, the handle is unchanged).
    #[test]
    fn set_handle_rejects_an_invalid_handle() {
        let mut host = host();
        let id = seed_entry(&mut host, EntryType::Character, "Bit");
        apply_intent(
            &mut host,
            Intent::SetCodexHandle {
                id,
                handle: "9bad space".to_owned(),
            },
            &ctx(),
        );
        let handle = host.edit.document.codex().entry(id).map(|e| e.handle.as_str().to_owned());
        assert_eq!(handle.as_deref(), Some("bit"), "an invalid handle leaves the entry unchanged");
    }

    /// Adding then removing an alias round-trips through the alias commands.
    #[test]
    fn add_then_remove_alias() {
        let mut host = host();
        let id = seed_entry(&mut host, EntryType::Character, "Bit");
        apply_intent(&mut host, Intent::AddCodexAlias { id, alias: "byte".to_owned() }, &ctx());
        assert!(
            host.edit
                .document
                .codex()
                .entry(id)
                .is_some_and(|e| e.aliases.iter().any(|a| a.as_str() == "byte")),
            "the alias was added",
        );
        apply_intent(&mut host, Intent::RemoveCodexAlias { id, alias: "byte".to_owned() }, &ctx());
        assert!(
            host.edit.document.codex().entry(id).is_some_and(|e| e.aliases.is_empty()),
            "the alias was removed",
        );
    }

    /// Setting prompt fragments and negatives routes through the fragment commands.
    #[test]
    fn set_fragments_applies() {
        let mut host = host();
        let id = seed_entry(&mut host, EntryType::Character, "Bit");
        apply_intent(
            &mut host,
            Intent::SetCodexPromptFragments {
                id,
                fragments: vec![PromptFragment::new("round head", pixhaus_core::codex::InclusionPriority::Critical)],
            },
            &ctx(),
        );
        apply_intent(
            &mut host,
            Intent::SetCodexNegativeFragments {
                id,
                fragments: vec!["extra limbs".to_owned()],
            },
            &ctx(),
        );
        let entry = host.edit.document.codex().entry(id).cloned();
        assert_eq!(entry.as_ref().map(|e| e.prompt_fragments.len()), Some(1));
        assert_eq!(entry.as_ref().map(|e| e.negative_fragments.len()), Some(1));
    }

    /// Editing palette details routes through `SetPaletteDetails`.
    #[test]
    fn set_palette_details_applies() {
        let mut host = host();
        let id = seed_entry(&mut host, EntryType::Palette, "Moonlit");
        let body = PaletteDetails {
            allow_generated_colors: true,
            ..PaletteDetails::default()
        };
        apply_intent(&mut host, Intent::SetPaletteDetails { id, body }, &ctx());
        assert!(
            host.edit
                .document
                .codex()
                .entry(id)
                .is_some_and(|e| matches!(&e.details, pixhaus_core::codex::EntryDetails::Palette(p) if p.allow_generated_colors)),
            "the palette body was replaced",
        );
    }

    /// The detail-tab and nav-filter intents are pure UI-state writes, no command.
    #[test]
    fn set_detail_tab_and_nav_filter_write_ui_state() {
        use crate::state::ui_state::{CodexDetailTab, NavFilter};
        let mut host = host();
        apply_intent(&mut host, Intent::SetCodexDetailTab(CodexDetailTab::Anchors), &ctx());
        assert_eq!(host.state.ui.codex.detail_tab, CodexDetailTab::Anchors);
        apply_intent(&mut host, Intent::SetCodexNavFilter(NavFilter::MissingCoverage), &ctx());
        assert_eq!(host.state.ui.codex.nav_filter, NavFilter::MissingCoverage);
    }

    /// Pin/unpin behave like the context-stack add/remove and stay idempotent.
    #[test]
    fn pin_and_unpin_codex_entry() {
        let mut host = host();
        let id = seed_entry(&mut host, EntryType::Palette, "Moonlit");
        apply_intent(&mut host, Intent::PinCodexEntry(id), &ctx());
        assert_eq!(host.state.ui.codex.context.len(), 1, "the entry was pinned");
        apply_intent(&mut host, Intent::PinCodexEntry(id), &ctx());
        assert_eq!(host.state.ui.codex.context.len(), 1, "pinning is idempotent");
        apply_intent(&mut host, Intent::UnpinCodexEntry(id), &ctx());
        assert!(host.state.ui.codex.context.is_empty(), "the entry was unpinned");
    }

    /// Duplicating an entry adds a second entry and selects the clone.
    #[test]
    fn duplicate_codex_entry_clones_and_selects() {
        let mut host = host();
        let id = seed_entry(&mut host, EntryType::Character, "Bit");
        apply_intent(&mut host, Intent::DuplicateCodexEntry(id), &ctx());
        assert_eq!(host.edit.document.codex().entries().len(), 2, "the clone was added");
        let selected = host.state.ui.codex.selected;
        assert!(selected.is_some() && selected != Some(id), "the clone is selected, not the source");
    }

    /// Promote/archive route through the status command path.
    #[test]
    fn promote_and_archive_set_status() {
        let mut host = host();
        let id = seed_entry(&mut host, EntryType::Character, "Bit");
        apply_intent(&mut host, Intent::PromoteCodexEntry(id), &ctx());
        assert_eq!(
            host.edit.document.codex().entry(id).map(|e| e.status),
            Some(EntryStatus::Canonical),
            "promote sets canonical",
        );
        apply_intent(&mut host, Intent::ArchiveCodexEntry(id), &ctx());
        assert_eq!(
            host.edit.document.codex().entry(id).map(|e| e.status),
            Some(EntryStatus::Archived),
            "archive sets archived",
        );
    }

    /// Folder create / set-parent / set-entry-folder / delete all route through commands.
    #[test]
    fn folder_crud_round_trips() {
        let mut host = host();
        let id = seed_entry(&mut host, EntryType::Character, "Bit");
        apply_intent(
            &mut host,
            Intent::CreateCodexFolder {
                parent: None,
                name: "Heroes".to_owned(),
            },
            &ctx(),
        );
        let Some(folder) = host.edit.document.codex().child_folders(None).first().copied() else {
            panic!("a root folder should exist");
        };
        apply_intent(
            &mut host,
            Intent::SetCodexEntryFolder {
                entry: id,
                folder: Some(folder),
            },
            &ctx(),
        );
        assert_eq!(
            host.edit.document.codex().entry(id).and_then(|e| e.folder_id),
            Some(folder),
            "the entry moved into the folder"
        );
        apply_intent(&mut host, Intent::DeleteCodexFolder { id: folder }, &ctx());
        assert!(host.edit.document.codex().folder(folder).is_none(), "the folder was deleted");
        assert_eq!(
            host.edit.document.codex().entry(id).and_then(|e| e.folder_id),
            None,
            "its entry reparented to the root"
        );
    }

    // --- Coverage intents ---

    /// Applying a built-in preset creates the project template and attaches it to the
    /// entry, so the entry's coverage is no longer empty.
    #[test]
    fn apply_builtin_coverage_template_attaches() {
        let mut host = host();
        let id = seed_entry(&mut host, EntryType::Character, "Bit");
        apply_intent(
            &mut host,
            Intent::ApplyBuiltinCoverageTemplate {
                id,
                preset: BuiltinCoveragePreset::PlatformerCharacter,
            },
            &ctx(),
        );
        let attached = host.edit.document.codex().entry(id).map(|e| e.applied_templates.len());
        assert_eq!(attached, Some(1), "the entry has one applied template");
        assert_eq!(host.edit.document.codex().coverage_templates().count(), 1, "the project gained one template");
    }

    /// Creating a project template, then attaching it to an entry by id, seeds the
    /// entry's coverage. Round-trips the create + apply intents.
    #[test]
    fn create_then_apply_coverage_template() {
        let mut host = host();
        let id = seed_entry(&mut host, EntryType::Character, "Bit");
        apply_intent(
            &mut host,
            Intent::CreateCoverageTemplate {
                name: "states".to_owned(),
                slots: vec![CoverageSlot::custom("idle", "Idle"), CoverageSlot::custom("walk", "Walk")],
            },
            &ctx(),
        );
        let Some(template) = host.edit.document.codex().coverage_templates().next().map(|t| t.id) else {
            panic!("the template should exist");
        };
        apply_intent(&mut host, Intent::ApplyCoverageTemplate { id, template }, &ctx());
        assert!(
            host.edit.document.codex().entry(id).is_some_and(|e| e.applied_templates.contains(&template)),
            "the template is attached to the entry",
        );
    }

    /// A per-entry custom slot is added and removed through its intents, never touching
    /// any template.
    #[test]
    fn add_then_remove_entry_custom_slot() {
        let mut host = host();
        let id = seed_entry(&mut host, EntryType::Character, "Bit");
        apply_intent(
            &mut host,
            Intent::AddEntryCustomSlot {
                id,
                slot: CoverageSlot::custom("crouch", "Crouch"),
            },
            &ctx(),
        );
        assert!(
            host.edit
                .document
                .codex()
                .entry(id)
                .is_some_and(|e| e.custom_slots.iter().any(|s| s.key == "crouch")),
            "the custom slot was added",
        );
        apply_intent(&mut host, Intent::RemoveEntryCustomSlot { id, key: "crouch".to_owned() }, &ctx());
        assert!(
            host.edit.document.codex().entry(id).is_some_and(|e| e.custom_slots.is_empty()),
            "the custom slot was removed",
        );
    }

    /// Renaming a per-entry custom slot's label routes through the command path and keeps
    /// the stable slot key, so a coverage-status cell keyed on it survives the rename.
    #[test]
    fn rename_entry_custom_slot_label_keeps_the_key() {
        let mut host = host();
        let id = seed_entry(&mut host, EntryType::Character, "Bit");
        apply_intent(
            &mut host,
            Intent::AddEntryCustomSlot {
                id,
                slot: CoverageSlot::custom("crouch", "Crouch"),
            },
            &ctx(),
        );
        apply_intent(
            &mut host,
            Intent::RenameEntryCustomSlotLabel {
                id,
                key: "crouch".to_owned(),
                label: CoverageLabel::Literal("Sneak".to_owned()),
            },
            &ctx(),
        );
        let slot = host
            .edit
            .document
            .codex()
            .entry(id)
            .and_then(|e| e.custom_slots.iter().find(|s| s.key == "crouch").cloned());
        assert_eq!(
            slot.map(|s| s.label),
            Some(CoverageLabel::Literal("Sneak".to_owned())),
            "the custom slot's label changed but the key stayed stable",
        );
    }

    /// Changing a relationship's kind in place retypes the edge through the command path,
    /// without a remove + re-add.
    #[test]
    fn change_relationship_kind_retypes_the_edge() {
        let mut host = host();
        let bit = seed_entry(&mut host, EntryType::Character, "Bit");
        let style = seed_entry(&mut host, EntryType::Style, "Crisp");
        apply_intent(
            &mut host,
            Intent::AddCodexRelationship {
                from: bit,
                kind: RelationKind::Uses,
                to: style,
            },
            &ctx(),
        );
        apply_intent(
            &mut host,
            Intent::ChangeRelationshipKind {
                from: bit,
                old_kind: RelationKind::Uses,
                to: style,
                new_kind: RelationKind::Requires,
            },
            &ctx(),
        );
        let kinds: Vec<RelationKind> = host
            .edit
            .document
            .codex()
            .relationships()
            .iter()
            .filter(|r| r.from == bit && r.to == style)
            .map(|r| r.kind)
            .collect();
        assert_eq!(kinds, vec![RelationKind::Requires], "the single edge was retyped, not duplicated");
    }

    /// Template-edit intents (create, add slot, rename a slot's label, reorder, delete)
    /// all route through commands and change the model. A slot rename keeps the key.
    #[test]
    fn template_slot_edits_round_trip() {
        let mut host = host();
        seed_entry(&mut host, EntryType::Character, "Bit");
        apply_intent(
            &mut host,
            Intent::CreateCoverageTemplate {
                name: "states".to_owned(),
                slots: vec![CoverageSlot::custom("idle", "Idle")],
            },
            &ctx(),
        );
        let Some(template) = host.edit.document.codex().coverage_templates().next().map(|t| t.id) else {
            panic!("the template should exist");
        };
        apply_intent(
            &mut host,
            Intent::AddCoverageSlot {
                template,
                slot: CoverageSlot::custom("walk", "Walk"),
            },
            &ctx(),
        );
        apply_intent(
            &mut host,
            Intent::RenameCoverageSlotLabel {
                template,
                key: "idle".to_owned(),
                label: CoverageLabel::Literal("Standing".to_owned()),
            },
            &ctx(),
        );
        apply_intent(&mut host, Intent::ReorderCoverageSlots { template, from: 0, to: 1 }, &ctx());
        let slots = host
            .edit
            .document
            .codex()
            .coverage_template(template)
            .map(|t| t.slots.clone())
            .unwrap_or_default();
        assert_eq!(slots.len(), 2, "the template has two slots");
        // The "idle" slot kept its key after the relabel, and now sits second after the reorder.
        assert!(slots.iter().any(|s| s.key == "idle"), "the slot key survived the rename");
        assert!(
            slots.iter().any(|s| s.label == CoverageLabel::Literal("Standing".to_owned())),
            "the slot label changed",
        );
        apply_intent(&mut host, Intent::DeleteCoverageTemplate { template }, &ctx());
        assert!(host.edit.document.codex().coverage_template(template).is_none(), "the template was deleted");
    }

    /// Renaming a project template changes its display name through the command path.
    #[test]
    fn rename_coverage_template_applies() {
        let mut host = host();
        apply_intent(
            &mut host,
            Intent::CreateCoverageTemplate {
                name: "states".to_owned(),
                slots: vec![CoverageSlot::custom("idle", "Idle")],
            },
            &ctx(),
        );
        let Some(template) = host.edit.document.codex().coverage_templates().next().map(|t| t.id) else {
            panic!("the template should exist");
        };
        apply_intent(
            &mut host,
            Intent::RenameCoverageTemplate {
                template,
                name: "animation states".to_owned(),
            },
            &ctx(),
        );
        assert_eq!(
            host.edit.document.codex().coverage_template(template).map(|t| t.name.clone()),
            Some("animation states".to_owned()),
            "the template name changed",
        );
    }
}
