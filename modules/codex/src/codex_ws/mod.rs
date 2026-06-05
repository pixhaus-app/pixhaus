//! The Codex workspace and its panels - the production-cockpit pass.
//!
//! The Codex is a library-like, non-canvas workspace (Codex bible section 8): a left
//! Navigator (a Pinned section, a World type-group tree, the folder tree, and
//! Collections smart filters), a full-center entry page (a rich header + a detail tab
//! bar over Overview / Visual / Anchors / Prompt / Coverage / Relations / History), a
//! right Inspector (entry health, linked assets, used-in counts, quick actions), and a
//! bottom strip of Coverage / Test Generation / History panels. Every panel is a
//! `&self` unit struct reading the read-only `CodexView` mirror and pushing `Intent`s;
//! the center editor also edits a shell-owned `CodexEditorDraft`. No panel mutates the
//! document - the Codex remembers, AI proposes, the artist decides.
//!
//! This module is split into per-area files (one per panel/area) along the comment-banner
//! seams of the original single module. The split is pure reorganization: the registered
//! panels, the workspace, and `register` keep identical entry points. Shared items are
//! declared here or in `keys`/`editor`/`coverage` and reached through `super::*` so each
//! area file sees the same namespace the single module had.

use egui::{Key, KeyboardShortcut, Modifiers};
use pixhaus_core::CodexEntryId;
use pixhaus_core::codex::details::{ColorRole, PaletteColor, PaletteRamp};
use pixhaus_core::codex::{
    AnchorKind, AnchorStrength, CoverageItemStatus, CoverageSlot, EntryStatus, EntryType, InclusionPriority, PromptFragment, RelationKind,
};
use pixhaus_core::commands::BuiltinCoveragePreset;
use pixhaus_ui::contrib_api::{
    ActionDesc, ActionId, CenterSurface, HostRegistrar, MsgKey, Panel, PanelId, PanelMeta, PanelScope, StatusItem, Workspace, WorkspaceId, WorkspaceLayout,
    WorkspaceMeta,
};
use pixhaus_ui::region::Region;
use pixhaus_ui::state::intent::Intent;
use pixhaus_ui::state::session::{CodexEntryDetail, CodexFolderNode, CodexView};
use pixhaus_ui::state::ui_state::{CodexDetailTab, CodexEditorDraft, NavFilter};
use pixhaus_ui::theme::Theme;
use pixhaus_ui::{icons, widgets};

mod coverage;
mod details;
mod editor;
mod inspector;
mod keys;
mod navigator;
mod tray;

// Pull each area's cross-area items into the module namespace by name (not `*`: the repo
// denies `clippy::wildcard_imports` outside test modules). These are exactly the items
// `register`, the test module, and the sibling areas reach through `super::`; an area's
// fully private helpers stay in their own file. Re-exporting by name keeps the call sites
// unchanged across the split while naming the seam each item crosses.
use coverage::{CoveragePanel, coverage_body};
use details::{details_editor, notes_field};
use editor::{EditorPanel, field_row, history_body};
use inspector::InspectorPanel;
use keys::{RELATION_KINDS, anchor_kind_key, anchor_strength_key, color_role_key, coverage_status_key, entry_type_key, priority_key, relation_key, status_key};
use navigator::NavigatorPanel;
use tray::{CodexHistoryPanel, TestGenerationPanel};

// These three are reached only from the test module below, so they are pulled in under
// `cfg(test)`: a plain `use` would read as unused in the non-test build (`-D warnings`).
// The test module's `use super::*` re-exports them, keeping `codex_ws::tests::*` valid.
#[cfg(test)]
use coverage::{coverage_complete, next_free_slot_key};
#[cfg(test)]
use navigator::TYPE_GROUPS;

/// Resolve an i18n key to display text at render time (the shell's `tr()` contract).
/// A thin alias so the panel code reads `tr("codex.field.name")` rather than the full
/// path on every call.
pub(crate) fn tr(key: &str) -> String {
    pixhaus_services::i18n::tr(key)
}

/// The Codex workspace id.
pub const CODEX: WorkspaceId = WorkspaceId("codex");

/// The Navigator panel id (left dock).
pub const NAVIGATOR: PanelId = PanelId("codex-navigator");
/// The Entry Editor / Board / Graph panel id (full center).
pub const EDITOR: PanelId = PanelId("codex-editor");
/// The Inspector panel id (right dock).
pub const INSPECTOR: PanelId = PanelId("codex-inspector");
/// The Coverage panel id (bottom tray).
pub const COVERAGE: PanelId = PanelId("codex-coverage");
/// The Test Generation panel id (bottom tray).
pub const TEST_GENERATION: PanelId = PanelId("codex-test-generation");
/// The History panel id (bottom tray).
pub const HISTORY: PanelId = PanelId("codex-history");

// The `codex.*` actions the panels dispatch, namespaced so they never collide.
const CODEX_NEW_ENTRY: ActionId = ActionId("codex.new-entry");
const CODEX_COMPILE: ActionId = ActionId("codex.compile-prompt");

/// The Codex workspace: a non-canvas creative-bible layout (Codex bible 8.2). Owns
/// layout only, no data.
pub struct CodexWorkspace;

impl Workspace for CodexWorkspace {
    fn id(&self) -> WorkspaceId {
        CODEX
    }

    fn meta(&self) -> WorkspaceMeta {
        WorkspaceMeta {
            name: MsgKey("workspace.codex.title"),
            icon: icons::CODEX,
            purpose: MsgKey("workspace.codex.purpose"),
            shortcut: KeyboardShortcut::new(Modifiers::COMMAND, Key::Num6),
        }
    }

    fn layout(&self) -> WorkspaceLayout {
        WorkspaceLayout {
            left_dock: vec![NAVIGATOR],
            right_dock: vec![INSPECTOR],
            bottom_tray: vec![COVERAGE, TEST_GENERATION, HISTORY],
            center: CenterSurface::Panel(EDITOR),
            // The Codex has no pixel tools; the rail is empty (no default tool read).
            primary_tools: Vec::new(),
            default_tool: pixhaus_ui::contrib_api::ToolId(""),
            // The project-wide coverage count is DERIVED into `CodexView.project_coverage`
            // each frame; the status-item label here is the static caption the shell
            // resolves at layout-build time (it has no view access). See the return note.
            status_items: vec![StatusItem {
                icon: icons::COVERAGE,
                text: MsgKey("workspace.codex.status.coverage").tr(),
            }],
        }
    }
}

/// Register the Codex workspace, its Navigator/Editor/Inspector panels, the
/// Coverage/Test/History tray panels, and the `codex.*` actions.
pub fn register(host: &mut dyn HostRegistrar) {
    host.add_workspace(Box::new(CodexWorkspace));
    host.add_panel(Box::new(NavigatorPanel));
    host.add_panel(Box::new(EditorPanel));
    host.add_panel(Box::new(InspectorPanel));
    host.add_panel(Box::new(CoveragePanel));
    host.add_panel(Box::new(TestGenerationPanel));
    host.add_panel(Box::new(CodexHistoryPanel));

    // These ActionIds are surfaced in the command palette but their RunAction handler is
    // currently the shell-wide mock JobStub - the panels do not dispatch them. The live
    // create/compile path is the direct Intent the panels push (Intent::CreateCodexEntry /
    // Intent::CompileCodexPrompt); the palette wiring of ActionId -> Intent arrives with the
    // real services job dispatch. Keeping them registered preserves palette discoverability
    // uniformly with the other five modules rather than making Codex the lone exception.
    for (id, label) in [
        (CODEX_NEW_ENTRY, MsgKey("command.codex.add_entry")),
        (CODEX_COMPILE, MsgKey("command.codex.compile_prompt")),
    ] {
        host.add_action(ActionDesc {
            id,
            label,
            icon: icons::CODEX,
            palette_visible: true,
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn codex_layout_uses_left_dock_and_a_panel_center() {
        let layout = CodexWorkspace.layout();
        assert_eq!(layout.left_dock, vec![NAVIGATOR]);
        assert_eq!(layout.center, CenterSurface::Panel(EDITOR));
        assert_eq!(layout.right_dock, vec![INSPECTOR]);
        assert_eq!(layout.bottom_tray, vec![COVERAGE, TEST_GENERATION, HISTORY]);
        assert!(layout.primary_tools.is_empty(), "the Codex has no pixel tools");
    }

    #[test]
    fn codex_meta_uses_cmd_6() {
        assert_eq!(CodexWorkspace.id(), CODEX);
        assert_eq!(CodexWorkspace.meta().name, MsgKey("workspace.codex.title"));
        assert_eq!(CodexWorkspace.meta().shortcut, KeyboardShortcut::new(Modifiers::COMMAND, Key::Num6));
    }

    #[test]
    fn panel_ids_and_regions() {
        assert_eq!(NavigatorPanel.id(), NAVIGATOR);
        assert_eq!(NavigatorPanel.meta().default_region, Region::LeftDock);
        assert_eq!(EditorPanel.meta().default_region, Region::Center);
        assert_eq!(InspectorPanel.meta().default_region, Region::RightDock);
        assert_eq!(CoveragePanel.meta().default_region, Region::BottomTray);
        assert_eq!(TestGenerationPanel.meta().default_region, Region::BottomTray);
        assert_eq!(CodexHistoryPanel.meta().default_region, Region::BottomTray);
    }

    #[test]
    fn type_groups_cover_every_entry_type() {
        // Every EntryType must land in exactly one Navigator family, so no entry is
        // unreachable in the WORLD tree.
        for t in EntryType::all() {
            let count = TYPE_GROUPS.iter().filter(|g| g.members.contains(t)).count();
            assert_eq!(count, 1, "{t:?} must belong to exactly one type group");
        }
    }

    #[test]
    fn coverage_complete_only_for_finalized_states() {
        assert!(coverage_complete(CoverageItemStatus::Approved));
        assert!(coverage_complete(CoverageItemStatus::ManuallyFinalized));
        assert!(!coverage_complete(CoverageItemStatus::Missing));
        assert!(!coverage_complete(CoverageItemStatus::Generated));
    }

    #[test]
    fn relation_kinds_cover_all_eleven() {
        assert_eq!(RELATION_KINDS.len(), 11);
    }

    #[test]
    fn next_free_slot_key_starts_at_one_when_empty() {
        assert_eq!(next_free_slot_key("slot", std::iter::empty()), "slot_1");
    }

    #[test]
    fn next_free_slot_key_skips_past_a_dense_run() {
        assert_eq!(next_free_slot_key("slot", ["slot_1", "slot_2"].into_iter()), "slot_3");
    }

    #[test]
    fn next_free_slot_key_fills_an_interior_gap() {
        // The whole point of scanning (not counting): an interior remove leaves slot_2 free, so
        // the next add must reuse it rather than re-mint slot_3 and collide on the next round.
        assert_eq!(next_free_slot_key("slot", ["slot_1", "slot_3"].into_iter()), "slot_2");
    }

    #[test]
    fn next_free_slot_key_honors_a_custom_prefix() {
        assert_eq!(next_free_slot_key("custom", ["custom_1"].into_iter()), "custom_2");
    }

    // The enum-to-i18n-key mappers must each return a UNIQUE key per variant under the right
    // namespace prefix, so a renamed or duplicated key fails the build rather than drifting
    // silently. We assert prefix + uniqueness, not literal equality - asserting each literal
    // would just mirror the source and rot. Variant lists are spelled out locally because only
    // EntryType (via EntryType::all) and RelationKind (via RELATION_KINDS) carry an
    // enumeration helper; the rest have neither all() nor a strum derive.
    fn assert_keys_unique_with_prefix(prefix: &str, keys: &[&'static str]) {
        for key in keys {
            assert!(key.starts_with(prefix), "{key:?} must start with {prefix:?}");
        }
        let unique: std::collections::HashSet<&&str> = keys.iter().collect();
        assert_eq!(unique.len(), keys.len(), "{prefix} keys must be unique across variants");
    }

    #[test]
    fn status_keys_are_unique_and_namespaced() {
        let keys: Vec<&'static str> = [
            EntryStatus::Draft,
            EntryStatus::Candidate,
            EntryStatus::Canonical,
            EntryStatus::Deprecated,
            EntryStatus::Archived,
            EntryStatus::Rejected,
        ]
        .into_iter()
        .map(status_key)
        .collect();
        assert_keys_unique_with_prefix("codex.status.", &keys);
    }

    #[test]
    fn relation_keys_are_unique_and_namespaced() {
        let keys: Vec<&'static str> = RELATION_KINDS.into_iter().map(relation_key).collect();
        assert_keys_unique_with_prefix("codex.relation.", &keys);
    }

    #[test]
    fn entry_type_keys_are_unique_and_namespaced() {
        let keys: Vec<&'static str> = EntryType::all().iter().copied().map(entry_type_key).collect();
        assert_keys_unique_with_prefix("codex.entry_type.", &keys);
    }

    #[test]
    fn priority_keys_are_unique_and_namespaced() {
        let keys: Vec<&'static str> = [
            InclusionPriority::Critical,
            InclusionPriority::Important,
            InclusionPriority::Normal,
            InclusionPriority::Optional,
            InclusionPriority::NeverInPrompt,
        ]
        .into_iter()
        .map(priority_key)
        .collect();
        assert_keys_unique_with_prefix("codex.priority.", &keys);
    }

    #[test]
    fn anchor_kind_keys_are_unique_and_namespaced() {
        let keys: Vec<&'static str> = [
            AnchorKind::Identity,
            AnchorKind::Visual,
            AnchorKind::Palette,
            AnchorKind::Style,
            AnchorKind::Animation,
            AnchorKind::Scale,
            AnchorKind::Lore,
            AnchorKind::Negative,
        ]
        .into_iter()
        .map(anchor_kind_key)
        .collect();
        assert_keys_unique_with_prefix("codex.anchor.kind.", &keys);
    }

    #[test]
    fn anchor_strength_keys_are_unique_and_namespaced() {
        let keys: Vec<&'static str> = [AnchorStrength::Loose, AnchorStrength::Normal, AnchorStrength::Strong, AnchorStrength::Locked]
            .into_iter()
            .map(anchor_strength_key)
            .collect();
        assert_keys_unique_with_prefix("codex.anchor.strength.", &keys);
    }

    #[test]
    fn color_role_keys_are_unique_and_namespaced() {
        let keys: Vec<&'static str> = [
            ColorRole::Shadow,
            ColorRole::Midtone,
            ColorRole::Highlight,
            ColorRole::Outline,
            ColorRole::Skin,
            ColorRole::Cloth,
            ColorRole::Metal,
            ColorRole::MagicGlow,
            ColorRole::Danger,
            ColorRole::Healing,
            ColorRole::UiAccent,
        ]
        .into_iter()
        .map(color_role_key)
        .collect();
        assert_keys_unique_with_prefix("codex.color_role.", &keys);
    }

    #[test]
    fn coverage_status_keys_are_unique_and_namespaced() {
        let keys: Vec<&'static str> = [
            CoverageItemStatus::Missing,
            CoverageItemStatus::Draft,
            CoverageItemStatus::Generated,
            CoverageItemStatus::NeedsReview,
            CoverageItemStatus::Approved,
            CoverageItemStatus::ManuallyFinalized,
            CoverageItemStatus::Deprecated,
        ]
        .into_iter()
        .map(coverage_status_key)
        .collect();
        assert_keys_unique_with_prefix("codex.coverage.status.", &keys);
    }
}
