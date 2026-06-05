//! Session state: the per-session, non-durable model the shell owns.
//!
//! Minimal this round. `active_document` / `selection` / `undo_stack` are reserved
//! seams that arrive with `core` (spec "Owners, no overlap"); they are intentionally
//! absent, not forgotten.

use pixhaus_core::ClipId;
use pixhaus_core::codex::{
    Anchor, AnchorKind, AnchorStrength, CodexEntryId, CodexFolderId, CoverageItemStatus, CoverageLabel, CoverageTemplateId, EntryDetails, EntryStatus,
    EntryType, PromptFragment, RelationKind,
};
use pixhaus_services::ResultKind;
use pixhaus_services::codex::{CompiledPrompt, UsedInCounts};

use crate::contrib_api::ids::{ActionId, ToolId, WorkspaceId};
use crate::state::ui_state::CodexMode;

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
    /// Read-only mirror of the Codex, for the Codex-workspace panels (which cannot
    /// reach the document). Rebuilt post-frame from `Document.codex()` plus the
    /// shell-owned Codex UI state, the same way `result_kinds` is rebuilt.
    pub codex: CodexView,
}

/// The read-only Codex mirror the Codex-workspace panels render from. Built from
/// `Document.codex()` and the shell-owned [`CodexUi`](crate::state::ui_state::CodexUi)
/// by [`sync_codex_view`](crate::shell::sync_codex_view) each frame. Holds owned
/// snapshots only - panels never reach into the document.
#[derive(Clone, Debug, Default)]
pub struct CodexView {
    /// Every entry as a Navigator summary, in id order.
    pub entries: Vec<CodexEntrySummary>,
    /// The active center mode, mirrored from the Codex UI state.
    pub mode: CodexMode,
    /// The active center detail tab, mirrored from the Codex UI state.
    pub detail_tab: crate::state::ui_state::CodexDetailTab,
    /// The active Navigator smart filter, mirrored from the Codex UI state.
    pub nav_filter: crate::state::ui_state::NavFilter,
    /// Project-wide coverage as `(complete, total)` summed over every entry (DERIVED).
    /// The top-bar Codex status item reads this.
    pub project_coverage: (usize, usize),
    /// The selected entry's id, mirrored from the Codex UI state (resolved against the
    /// live entry set, so a deleted selection clears).
    pub selected: Option<CodexEntryId>,
    /// The selected entry's detail snapshot, for the editor and inspector. `None` when
    /// nothing is selected.
    pub detail: Option<CodexEntryDetail>,
    /// The current Navigator search query, mirrored from the Codex UI state.
    pub search: String,
    /// The `@`-autocomplete / Navigator suggestions for the current query.
    pub suggestions: Vec<CodexSuggestion>,
    /// The generation context stack: pinned references with strengths and display data.
    pub context: Vec<CodexContextEntry>,
    /// The latest compiled-prompt preview, when a prompt has been compiled in the Codex
    /// test view. `None` until the user compiles one.
    pub compiled: Option<CompiledPrompt>,
    /// The folder tree the Navigator renders: the root folders and the entries that sit
    /// at the codex root (entries inside a folder hang off their node).
    pub folder_tree: Vec<CodexFolderNode>,
    /// The entries that sit at the codex root (no folder), as Navigator summaries.
    pub root_entries: Vec<CodexEntrySummary>,
    /// Every project-level coverage template, in id order, for the template picker and
    /// the slot editor. Built from the services summaries each frame.
    pub coverage_templates: Vec<CoverageTemplateSummary>,
}

impl CodexView {
    /// The number of entries in the Codex.
    pub fn entry_count(&self) -> usize {
        self.entries.len()
    }

    /// The summary for `id`, if present.
    pub fn summary(&self, id: CodexEntryId) -> Option<&CodexEntrySummary> {
        self.entries.iter().find(|e| e.id == id)
    }

    /// Whether `id` is already pinned in the context stack.
    pub fn is_pinned(&self, id: CodexEntryId) -> bool {
        self.context.iter().any(|c| c.entry == id)
    }

    /// Every folder in the tree as a flat `(id, name)` list, depth-first, for the
    /// "move to folder" picker (which wants a flat menu, not the nested tree).
    pub fn flat_folders(&self) -> Vec<(CodexFolderId, String)> {
        fn walk(nodes: &[CodexFolderNode], out: &mut Vec<(CodexFolderId, String)>) {
            for node in nodes {
                out.push((node.id, node.name.clone()));
                walk(&node.children, out);
            }
        }
        let mut out = Vec::new();
        walk(&self.folder_tree, &mut out);
        out
    }
}

/// One folder in the Navigator tree: identity, the entries directly inside it, and its
/// child folders (each a node). Plain owned data the Navigator renders without the
/// document.
#[derive(Clone, Debug, PartialEq)]
pub struct CodexFolderNode {
    /// Stable id, echoed back into the folder intents.
    pub id: CodexFolderId,
    /// Display name (project content).
    pub name: String,
    /// The entries that live directly in this folder, as summaries.
    pub entries: Vec<CodexEntrySummary>,
    /// Child folders, each fully expanded.
    pub children: Vec<CodexFolderNode>,
}

/// One entry's Navigator summary: identity, classification, anchor badges, and a
/// coverage ratio. Plain owned data so the Navigator reads it without the document.
#[derive(Clone, Debug, PartialEq)]
pub struct CodexEntrySummary {
    /// Stable id, echoed back into the Codex intents.
    pub id: CodexEntryId,
    /// Primary handle, without the leading `@` (project content, not a key).
    pub handle: String,
    /// Display name (project content).
    pub name: String,
    /// The entry's type, for the type-tree grouping and the type-color chip.
    pub entry_type: EntryType,
    /// The entry's lifecycle status, for the status badge.
    pub status: EntryStatus,
    /// One badge per anchor: the kind and its strength, for the strength badges.
    pub anchors: Vec<AnchorBadge>,
    /// Coverage completion in `0.0..=1.0` (1.0 when no coverage template applies).
    pub coverage_ratio: f32,
    /// Whether any coverage slot is still missing - the Navigator "missing coverage"
    /// filter and the card indicator read this.
    pub coverage_incomplete: bool,
    /// The count of broken `@`-references in this entry's own fragment text - the
    /// Navigator "broken references" filter reads this (DERIVED in `sync_codex_view`).
    pub broken_ref_count: usize,
}

/// A compact anchor badge: the kind and its strength. The widget colors it by strength.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct AnchorBadge {
    /// What the anchor pins.
    pub kind: AnchorKind,
    /// How firmly it holds (drives the badge color, Locked vs Loose).
    pub strength: AnchorStrength,
}

/// One mirrored coverage slot: its stable key, its label (an i18n key or a literal,
/// resolved in the UI), its status, and its source template (`None` for a per-entry
/// custom slot). The center Coverage cards and the inspector checklist read this so
/// they show true per-slot state instead of a coarse all-or-nothing stand-in.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CoverageRow {
    /// The stable slot key (project content, e.g. "idle").
    pub slot: String,
    /// The slot label: a [`CoverageLabel::Key`] the UI resolves via `tr()`, or a
    /// [`CoverageLabel::Literal`] rendered as-is. Never pass a literal to `tr()`.
    pub label: CoverageLabel,
    /// The slot's current status.
    pub status: CoverageItemStatus,
    /// The source template this slot came from, or `None` for a per-entry custom slot.
    /// The slot editor uses this to route a remove to the right command.
    pub template: Option<CoverageTemplateId>,
}

/// One project coverage template summarized for the picker and the template-management
/// surface: its stable id, its display name (project content, not a key), and its full
/// slot list. Built by
/// [`coverage_template_details`](pixhaus_services::codex::coverage_template_details) and
/// mirrored each frame in [`sync_codex_view`](crate::shell::sync_codex_view). Carrying the
/// slots (not just a count) lets the slot editor reach *any* template's slots - add,
/// remove, rename, reorder - whether or not the template is applied to the selected entry.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CoverageTemplateSummary {
    /// Stable id, echoed back into the coverage intents.
    pub id: CoverageTemplateId,
    /// Display name (project content).
    pub name: String,
    /// The template's slots, in order: each a stable key plus a resolvable label.
    pub slots: Vec<CoverageSlotRow>,
}

impl CoverageTemplateSummary {
    /// How many slots the template defines.
    pub fn slot_count(&self) -> usize {
        self.slots.len()
    }
}

/// One template slot mirrored for the management surface: its stable key (a rename never
/// touches it) and its label (a [`CoverageLabel::Key`] the UI resolves via `tr()`, or a
/// [`CoverageLabel::Literal`] rendered as-is). A thin mirror of the services
/// [`CoverageSlotView`](pixhaus_services::codex::CoverageSlotView), owned for the session.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CoverageSlotRow {
    /// The stable slot key (project content, e.g. "idle"); never changed by a rename.
    pub key: String,
    /// The slot label, resolved in the UI through `resolve_coverage_label`.
    pub label: CoverageLabel,
}

/// One mirrored version-history entry, for the History tab's timeline. A thin mirror
/// of core's `EntryVersion` owned for the session.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EntryVersionRow {
    /// Monotonic version number.
    pub version: u32,
    /// Epoch-millisecond timestamp.
    pub timestamp_ms: u64,
    /// Who or what made the change (a source label, not localized).
    pub author: String,
    /// A short summary of what changed (project content).
    pub summary: String,
}

/// The selected entry's full detail snapshot, for the center editor and the inspector.
#[derive(Clone, Debug, PartialEq)]
pub struct CodexEntryDetail {
    /// The summary fields (identity, type, status, anchors, coverage).
    pub summary: CodexEntrySummary,
    /// Alias handles, without the leading `@` (project content).
    pub aliases: Vec<String>,
    /// The folder this entry lives in, or `None` for the codex root.
    pub folder_id: Option<CodexFolderId>,
    /// Free-text description (project content).
    pub description: String,
    /// Lore / backstory text (project content).
    pub lore: String,
    /// Visual-identity text (project content).
    pub visual_description: String,
    /// Tags (project content).
    pub tags: Vec<String>,
    /// Positive prompt fragments with their inclusion priorities (project content).
    pub prompt_fragments: Vec<PromptFragment>,
    /// Negative prompt fragments, things to avoid (project content).
    pub negative_fragments: Vec<String>,
    /// The full anchors, with statements, for the inspector's anchor list.
    pub anchors: Vec<Anchor>,
    /// Type-specific detail body (Character/Palette/Style/Animation rich, else generic).
    pub details: EntryDetails,
    /// Outgoing relationships from this entry: the relation kind and the target's
    /// display name + id.
    pub relations: Vec<CodexRelation>,
    /// Incoming relationships (the "used by" list): the relation kind and the source's
    /// display name + id.
    pub used_by: Vec<CodexRelation>,
    /// The compiled prompt for THIS entry alone (a one-reference preview), so the
    /// inspector shows what the selected entry contributes to a prompt. `None` when the
    /// preview could not be built. Live: rebuilt each frame in `sync_codex_view`.
    pub prompt_preview: Option<CompiledPrompt>,
    /// Notes text (project content). Read from the entry's `GenericDetails` `notes`
    /// key when present, else empty. Edited through `SetGenericDetails`.
    pub notes: String,
    /// Epoch-millisecond timestamp of the first recorded version, or `None`.
    pub created_ms: Option<u64>,
    /// Epoch-millisecond timestamp of the latest recorded version, or `None`.
    pub updated_ms: Option<u64>,
    /// The latest version's author label (project content), or empty when unknown.
    pub author: String,
    /// The latest version number, or 0 when there is no history.
    pub version: u32,
    /// The full version history, newest last, for the History tab's timeline.
    pub version_history: Vec<EntryVersionRow>,
    /// The entry's derived health score in `0.0..=1.0` (DERIVED in `sync_codex_view`).
    pub health: f32,
    /// The entry's derived health percentage `0..=100`.
    pub health_percent: u8,
    /// The health checklist rows (pass/warn/fail), for the inspector. `(label_key,
    /// state, detail)` where `state` is `Some(true)` pass, `Some(false)` fail, `None`
    /// warn, and `detail` is optional interpolation context (e.g. a coverage `N/M`).
    pub health_checks: Vec<HealthCheckRow>,
    /// The entry's generation-readiness score in `0.0..=1.0` (DERIVED).
    pub readiness: f32,
    /// The readiness percentage `0..=100`.
    pub readiness_percent: u8,
    /// Per-slot coverage rows (DERIVED), replacing the coarse all-or-nothing stand-in.
    /// The union of the entry's applied templates and its custom slots, per-entry only.
    pub coverage_items: Vec<CoverageRow>,
    /// The project templates this entry has applied, in apply order. The coverage tab
    /// reads it to show which templates are attached and to offer a detach.
    pub applied_templates: Vec<CoverageTemplateId>,
    /// How often this entry is used elsewhere, bucketed by family (DERIVED).
    pub used_in: UsedInCounts,
    /// The count of handle-shaped references the entry declares (a MOCK stand-in for a
    /// linked-asset count until an asset store exists).
    pub linked_asset_count: usize,
}

/// One mirrored health-checklist row, owned for the session. Mirrors a services
/// [`HealthCheck`](pixhaus_services::codex::HealthCheck) so the inspector renders the
/// pass/warn/fail row without recomputing health.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct HealthCheckRow {
    /// The stable label key under `codex.health.check.*` (resolved in the UI).
    pub label_key: String,
    /// `Some(true)` pass, `Some(false)` fail, `None` warn.
    pub state: Option<bool>,
    /// Non-localized interpolation context (e.g. a coverage `N/M`), or `None`.
    pub detail: Option<String>,
}

/// One relationship edge from the selected entry's perspective, with the other end's
/// display name resolved for the inspector.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CodexRelation {
    /// The relationship kind.
    pub kind: RelationKind,
    /// The other endpoint's id (echoed back into intents).
    pub other: CodexEntryId,
    /// The other endpoint's display name, or its handle if the name is empty.
    pub other_label: String,
}

/// One `@`-autocomplete / Navigator suggestion (a thin mirror of the services
/// [`Suggestion`](pixhaus_services::codex::Suggestion), owned for the session).
#[derive(Clone, Debug, PartialEq)]
pub struct CodexSuggestion {
    /// The suggested entry's id.
    pub id: CodexEntryId,
    /// The entry's handle (without `@`).
    pub handle: String,
    /// The entry's display name.
    pub name: String,
    /// The entry's type.
    pub entry_type: EntryType,
    /// The entry's status.
    pub status: EntryStatus,
}

/// One pinned reference in the generation context stack, with display data resolved.
#[derive(Clone, Debug, PartialEq)]
pub struct CodexContextEntry {
    /// The pinned entry's id (echoed back into intents).
    pub entry: CodexEntryId,
    /// The strength the compiler weights it at.
    pub strength: AnchorStrength,
    /// The entry's handle (without `@`), for the reference chip.
    pub handle: String,
    /// The entry's type, for the chip's type color.
    pub entry_type: EntryType,
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
