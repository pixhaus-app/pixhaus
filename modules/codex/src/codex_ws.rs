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

/// Resolve an i18n key to display text at render time (the shell's `tr()` contract).
/// A thin alias so the panel code reads `tr("codex.field.name")` rather than the full
/// path on every call.
fn tr(key: &str) -> String {
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

/// The i18n key for an entry status (`codex.status.*`), resolved to display text at
/// render time. The Codex stores the enum; the shell localizes it.
fn status_key(status: EntryStatus) -> &'static str {
    match status {
        EntryStatus::Draft => "codex.status.draft",
        EntryStatus::Candidate => "codex.status.candidate",
        EntryStatus::Canonical => "codex.status.canonical",
        EntryStatus::Deprecated => "codex.status.deprecated",
        EntryStatus::Archived => "codex.status.archived",
        EntryStatus::Rejected => "codex.status.rejected",
    }
}

/// The i18n key for a relationship kind (`codex.relation.*`).
fn relation_key(kind: RelationKind) -> &'static str {
    match kind {
        RelationKind::Uses => "codex.relation.uses",
        RelationKind::BelongsTo => "codex.relation.belongs_to",
        RelationKind::AppearsIn => "codex.relation.appears_in",
        RelationKind::CompatibleWith => "codex.relation.compatible_with",
        RelationKind::IncompatibleWith => "codex.relation.incompatible_with",
        RelationKind::InheritsFrom => "codex.relation.inherits_from",
        RelationKind::VariantOf => "codex.relation.variant_of",
        RelationKind::Requires => "codex.relation.requires",
        RelationKind::Contains => "codex.relation.contains",
        RelationKind::Replaces => "codex.relation.replaces",
        RelationKind::InspiredBy => "codex.relation.inspired_by",
    }
}

/// The i18n key for an entry type (`codex.entry_type.*`).
fn entry_type_key(entry_type: EntryType) -> &'static str {
    match entry_type {
        EntryType::Character => "codex.entry_type.character",
        EntryType::Enemy => "codex.entry_type.enemy",
        EntryType::Npc => "codex.entry_type.npc",
        EntryType::Creature => "codex.entry_type.creature",
        EntryType::Prop => "codex.entry_type.prop",
        EntryType::Item => "codex.entry_type.item",
        EntryType::Weapon => "codex.entry_type.weapon",
        EntryType::Material => "codex.entry_type.material",
        EntryType::Palette => "codex.entry_type.palette",
        EntryType::Style => "codex.entry_type.style",
        EntryType::Vibe => "codex.entry_type.vibe",
        EntryType::Location => "codex.entry_type.location",
        EntryType::Biome => "codex.entry_type.biome",
        EntryType::Faction => "codex.entry_type.faction",
        EntryType::Animation => "codex.entry_type.animation",
        EntryType::Pose => "codex.entry_type.pose",
        EntryType::Vfx => "codex.entry_type.vfx",
        EntryType::UiElement => "codex.entry_type.ui",
        EntryType::Rule => "codex.entry_type.rule",
        EntryType::Recipe => "codex.entry_type.recipe",
        EntryType::ReferenceBoard => "codex.entry_type.board",
    }
}

/// The i18n key for an inclusion priority (`codex.priority.*`), for the prompt
/// composer's per-fragment priority chip.
fn priority_key(priority: InclusionPriority) -> &'static str {
    match priority {
        InclusionPriority::Critical => "codex.priority.critical",
        InclusionPriority::Important => "codex.priority.important",
        InclusionPriority::Normal => "codex.priority.normal",
        InclusionPriority::Optional => "codex.priority.optional",
        InclusionPriority::NeverInPrompt => "codex.priority.never_in_prompt",
    }
}

/// The i18n key for an anchor kind (`codex.anchor.kind.*`).
fn anchor_kind_key(kind: AnchorKind) -> &'static str {
    match kind {
        AnchorKind::Identity => "codex.anchor.kind.identity",
        AnchorKind::Visual => "codex.anchor.kind.visual",
        AnchorKind::Palette => "codex.anchor.kind.palette",
        AnchorKind::Style => "codex.anchor.kind.style",
        AnchorKind::Animation => "codex.anchor.kind.animation",
        AnchorKind::Scale => "codex.anchor.kind.scale",
        AnchorKind::Lore => "codex.anchor.kind.lore",
        AnchorKind::Negative => "codex.anchor.kind.negative",
    }
}

/// The i18n key for an anchor strength (`codex.anchor.strength.*`).
fn anchor_strength_key(strength: AnchorStrength) -> &'static str {
    match strength {
        AnchorStrength::Loose => "codex.anchor.strength.loose",
        AnchorStrength::Normal => "codex.anchor.strength.normal",
        AnchorStrength::Strong => "codex.anchor.strength.strong",
        AnchorStrength::Locked => "codex.anchor.strength.locked",
    }
}

/// The i18n key for a palette color role (`codex.color_role.*`).
fn color_role_key(role: ColorRole) -> &'static str {
    match role {
        ColorRole::Shadow => "codex.color_role.shadow",
        ColorRole::Midtone => "codex.color_role.midtone",
        ColorRole::Highlight => "codex.color_role.highlight",
        ColorRole::Outline => "codex.color_role.outline",
        ColorRole::Skin => "codex.color_role.skin",
        ColorRole::Cloth => "codex.color_role.cloth",
        ColorRole::Metal => "codex.color_role.metal",
        ColorRole::MagicGlow => "codex.color_role.magic_glow",
        ColorRole::Danger => "codex.color_role.danger",
        ColorRole::Healing => "codex.color_role.healing",
        ColorRole::UiAccent => "codex.color_role.ui_accent",
    }
}

/// A Navigator type-group family: a label key, its representative glyph, and the entry
/// types it gathers. Groups the long type list into the human families the mockup's
/// WORLD tree shows.
struct TypeGroup {
    /// The localized group-label key.
    label_key: &'static str,
    /// The glyph the group header reads with.
    glyph: char,
    /// The entry types this family gathers.
    members: &'static [EntryType],
}

/// The Navigator's WORLD type-group families, in display order (spec 2.1).
const TYPE_GROUPS: &[TypeGroup] = &[
    TypeGroup {
        label_key: "codex.nav.group.characters",
        glyph: icons::CHARACTER,
        members: &[EntryType::Character, EntryType::Npc, EntryType::Enemy, EntryType::Creature],
    },
    TypeGroup {
        label_key: "codex.nav.group.locations",
        glyph: icons::LOCATION,
        members: &[EntryType::Location, EntryType::Biome],
    },
    TypeGroup {
        label_key: "codex.nav.group.props",
        glyph: icons::ASSETS,
        members: &[EntryType::Prop, EntryType::Item, EntryType::Weapon, EntryType::UiElement],
    },
    TypeGroup {
        label_key: "codex.nav.group.materials",
        glyph: icons::MATERIAL,
        members: &[EntryType::Material],
    },
    TypeGroup {
        label_key: "codex.nav.group.palettes",
        glyph: icons::PALETTE,
        members: &[EntryType::Palette],
    },
    TypeGroup {
        label_key: "codex.nav.group.styles",
        glyph: icons::SPARKLE,
        members: &[EntryType::Style, EntryType::Vibe, EntryType::Vfx],
    },
    TypeGroup {
        label_key: "codex.nav.group.animations",
        glyph: icons::TIMELINE,
        members: &[EntryType::Animation, EntryType::Pose],
    },
    TypeGroup {
        label_key: "codex.nav.group.factions",
        glyph: icons::TAG,
        members: &[EntryType::Faction],
    },
    TypeGroup {
        label_key: "codex.nav.group.rules",
        glyph: icons::RULE,
        members: &[EntryType::Rule],
    },
    TypeGroup {
        label_key: "codex.nav.group.recipes",
        glyph: icons::RECIPE,
        members: &[EntryType::Recipe],
    },
    TypeGroup {
        label_key: "codex.nav.group.boards",
        glyph: icons::BOARD,
        members: &[EntryType::ReferenceBoard],
    },
];

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

// ===========================================================================
// Navigator (left dock)
// ===========================================================================

/// The Navigator panel (left dock): a search field, a primary `+ New entry` control
/// with a type-picker, a Pinned section, a World type-group tree with counts, the
/// folder tree, and a Collections section of smart filters (spec 2).
pub struct NavigatorPanel;

impl Panel for NavigatorPanel {
    fn id(&self) -> PanelId {
        NAVIGATOR
    }

    fn meta(&self) -> PanelMeta {
        PanelMeta {
            title: MsgKey("panel.codex-navigator.title"),
            icon: icons::CODEX,
            default_region: Region::LeftDock,
            default_open: true,
        }
    }

    #[allow(clippy::too_many_lines)]
    fn ui(&self, ui: &mut egui::Ui, scope: &mut PanelScope<'_>) {
        let theme = scope.ctx.theme;
        let view = &scope.ctx.session.codex;

        // Search field: the scratch carve-out bound to a single-line TextEdit. When it
        // changes, push CodexSearch so the suggestion mirror rebuilds.
        let resp = ui.add(
            egui::TextEdit::singleline(scope.scratch)
                .hint_text(format!("{} {}", icons::ZOOM, tr("codex.navigator.search.placeholder")))
                .desired_width(f32::INFINITY),
        );
        if resp.changed() {
            scope.ctx.intents.push(Intent::CodexSearch(scope.scratch.clone()));
        }

        ui.add_space(theme.spacing.sm);

        // The primary + New entry button (a filled accent button - an allowed accent
        // use) opening a type picker, plus a secondary + Folder ghost button.
        ui.horizontal(|ui| {
            ui.menu_button(
                egui::RichText::new(format!("{} {}", icons::ADD, tr("codex.action.new_entry")))
                    .size(theme.type_scale.label)
                    .color(theme.accent.on_accent),
                |ui| {
                    for entry_type in EntryType::all() {
                        let label = tr(entry_type_key(*entry_type));
                        if ui.button(format!("{} {label}", widgets::type_icon(*entry_type))).clicked() {
                            scope.ctx.intents.push(Intent::CreateCodexEntry {
                                entry_type: *entry_type,
                                name: label.clone(),
                            });
                            ui.close();
                        }
                    }
                },
            );
            if ui
                .add(
                    egui::Button::new(
                        egui::RichText::new(format!("{} {}", icons::FOLDER, tr("codex.action.new_folder_short")))
                            .size(theme.type_scale.label)
                            .color(theme.roles.text_secondary),
                    )
                    .frame(true),
                )
                .clicked()
            {
                scope.ctx.intents.push(Intent::CreateCodexFolder {
                    parent: None,
                    name: tr("codex.folder.untitled"),
                });
            }
        });

        ui.separator();

        let mut to_select = None;
        egui::ScrollArea::vertical().show(ui, |ui| {
            if !view.search.trim().is_empty() {
                // Search mode: a flat ranked list, folders and groups ignored (matches
                // the @-autocomplete behavior).
                for summary in view.entries.iter().filter(|e| view.suggestions.iter().any(|s| s.id == e.id)) {
                    nav_entry_row(ui, theme, view, summary, 1, &mut to_select);
                }
                return;
            }

            // --- PINNED section. ---
            if nav_section(ui, theme, "codex-nav-pinned", icons::PIN, &tr("codex.nav.section.pinned")) {
                if view.context.is_empty() {
                    nav_empty_line(ui, theme, &tr("codex.nav.empty.pinned"));
                } else {
                    let mut to_unpin = None;
                    for c in &view.context {
                        ui.horizontal(|ui| {
                            let data = widgets::NavNodeData {
                                glyph: widgets::type_icon(c.entry_type),
                                label: &format!("@{}", c.handle),
                                count: None,
                                depth: 1,
                                selected: view.selected == Some(c.entry),
                                expandable: false,
                                expanded: false,
                                accent_glyph: true,
                            };
                            if widgets::nav_tree_node(ui, theme, &data).clicked {
                                to_select = Some(c.entry);
                            }
                            if ui
                                .add(
                                    egui::Button::new(
                                        egui::RichText::new(icons::CLOSE.to_string())
                                            .size(theme.type_scale.label)
                                            .color(theme.roles.text_secondary),
                                    )
                                    .frame(false),
                                )
                                .clicked()
                            {
                                to_unpin = Some(c.entry);
                            }
                        });
                    }
                    if let Some(id) = to_unpin {
                        scope.ctx.intents.push(Intent::UnpinCodexEntry(id));
                    }
                }
            }

            ui.add_space(theme.spacing.xs);

            // --- WORLD type-group tree, then the folder tree and root entries. ---
            if nav_section(ui, theme, "codex-nav-world", icons::WORLD, &tr("codex.nav.section.world")) {
                for group in TYPE_GROUPS {
                    let members: Vec<&pixhaus_ui::state::session::CodexEntrySummary> =
                        view.entries.iter().filter(|e| group.members.contains(&e.entry_type)).collect();
                    if members.is_empty() {
                        continue;
                    }
                    let exp_id = ui.make_persistent_id(("codex-nav-group", group.label_key));
                    let mut expanded = ui.data_mut(|d| d.get_temp::<bool>(exp_id).unwrap_or(false));
                    let data = widgets::NavNodeData {
                        glyph: group.glyph,
                        label: &tr(group.label_key),
                        count: Some(members.len()),
                        depth: 1,
                        selected: false,
                        expandable: true,
                        expanded,
                        accent_glyph: false,
                    };
                    let resp = widgets::nav_tree_node(ui, theme, &data);
                    if resp.toggled || resp.clicked {
                        expanded = !expanded;
                        ui.data_mut(|d| d.insert_temp(exp_id, expanded));
                    }
                    if expanded {
                        for summary in &members {
                            nav_entry_row(ui, theme, view, summary, 2, &mut to_select);
                        }
                    }
                }

                ui.add_space(theme.spacing.xs);
                // The user-made folder tree, then root entries.
                for node in &view.folder_tree {
                    render_folder_node(ui, theme, scope.ctx.intents, view, node, 1, &mut to_select);
                }
                for summary in &view.root_entries {
                    nav_entry_row(ui, theme, view, summary, 1, &mut to_select);
                }
            }

            ui.add_space(theme.spacing.xs);

            // --- COLLECTIONS smart filters. ---
            if nav_section(ui, theme, "codex-nav-collections", icons::BOARD, &tr("codex.nav.section.collections")) {
                let missing = view.entries.iter().filter(|e| e.coverage_incomplete).count();
                let broken = view.entries.iter().filter(|e| e.broken_ref_count > 0).count();
                let filter_row = |ui: &mut egui::Ui, glyph, label_key, count, active| {
                    let data = widgets::NavNodeData {
                        glyph,
                        label: &tr(label_key),
                        count: Some(count),
                        depth: 1,
                        selected: active,
                        expandable: false,
                        expanded: false,
                        accent_glyph: false,
                    };
                    widgets::nav_tree_node(ui, theme, &data).clicked
                };
                if filter_row(
                    ui,
                    icons::COVERAGE,
                    "codex.nav.filter.missing_coverage",
                    missing,
                    view.nav_filter == NavFilter::MissingCoverage,
                ) {
                    let next = if view.nav_filter == NavFilter::MissingCoverage {
                        NavFilter::All
                    } else {
                        NavFilter::MissingCoverage
                    };
                    scope.ctx.intents.push(Intent::SetCodexNavFilter(next));
                }
                if filter_row(
                    ui,
                    icons::WARN,
                    "codex.nav.filter.broken_refs",
                    broken,
                    view.nav_filter == NavFilter::BrokenReferences,
                ) {
                    let next = if view.nav_filter == NavFilter::BrokenReferences {
                        NavFilter::All
                    } else {
                        NavFilter::BrokenReferences
                    };
                    scope.ctx.intents.push(Intent::SetCodexNavFilter(next));
                }
                // Trash is a placeholder this pass (no soft-delete model); inert.
                let trash = widgets::NavNodeData {
                    glyph: icons::TRASH,
                    label: &tr("codex.nav.filter.trash"),
                    count: Some(0),
                    depth: 1,
                    selected: false,
                    expandable: false,
                    expanded: false,
                    accent_glyph: false,
                };
                let _ = widgets::nav_tree_node(ui, theme, &trash);
            }
        });
        if let Some(id) = to_select {
            scope.ctx.intents.push(Intent::SelectCodexEntry(id));
        }
    }
}

/// A collapsible Navigator section header, expansion persisted in egui temp memory keyed
/// by `key`. Returns whether the section is expanded (draw its body when `true`).
fn nav_section(ui: &mut egui::Ui, theme: &Theme, key: &'static str, glyph: char, label: &str) -> bool {
    let exp_id = ui.make_persistent_id(("codex-nav-section", key));
    let mut expanded = ui.data_mut(|d| d.get_temp::<bool>(exp_id).unwrap_or(true));
    let caret = if expanded { icons::CARET_DOWN } else { icons::CARET_RIGHT };
    let header = ui.add(
        egui::Button::new(
            egui::RichText::new(format!("{caret} {glyph} {}", label.to_uppercase()))
                .size(theme.type_scale.label)
                .color(theme.roles.text_secondary),
        )
        .frame(false),
    );
    if header.clicked() {
        expanded = !expanded;
        ui.data_mut(|d| d.insert_temp(exp_id, expanded));
    }
    expanded
}

/// A muted empty-state line for an empty Navigator section.
fn nav_empty_line(ui: &mut egui::Ui, theme: &Theme, text: &str) {
    ui.horizontal(|ui| {
        ui.add_space(theme.spacing.md);
        ui.label(egui::RichText::new(text).size(theme.type_scale.label).color(theme.roles.text_disabled));
    });
}

/// Render one entry as a Navigator tree row (handle + type glyph + status dot), at
/// `depth`. A click selects it; the row carries a trailing move-to menu.
fn nav_entry_row(
    ui: &mut egui::Ui,
    theme: &Theme,
    view: &CodexView,
    summary: &pixhaus_ui::state::session::CodexEntrySummary,
    depth: usize,
    to_select: &mut Option<CodexEntryId>,
) {
    let data = widgets::NavNodeData {
        glyph: widgets::type_icon(summary.entry_type),
        label: &format!("@{}", summary.handle),
        count: None,
        depth,
        selected: view.selected == Some(summary.id),
        expandable: false,
        expanded: false,
        accent_glyph: false,
    };
    if widgets::nav_tree_node(ui, theme, &data).clicked {
        *to_select = Some(summary.id);
    }
}

/// Render one folder subtree: the folder header row (with rename/new-child/delete
/// affordances), then its entries and child folders when expanded. Expansion is tracked
/// in egui memory keyed by the folder id, so it persists across frames without shell
/// state.
// `depth` is the folder nesting level, a tiny integer; the usize -> f32 indent cast
// cannot lose precision in any realistic tree.
#[allow(clippy::cast_precision_loss)]
fn render_folder_node(
    ui: &mut egui::Ui,
    theme: &Theme,
    intents: &mut pixhaus_ui::state::intent::IntentSink,
    view: &CodexView,
    node: &CodexFolderNode,
    depth: usize,
    to_select: &mut Option<pixhaus_core::CodexEntryId>,
) {
    let exp_id = ui.make_persistent_id(("codex-folder", node.id.0));
    let rename_id = ui.make_persistent_id(("codex-folder-rename", node.id.0));
    let mut expanded = ui.data_mut(|d| d.get_temp::<bool>(exp_id).unwrap_or(true));
    let renaming = ui.data(|d| d.get_temp::<String>(rename_id));

    if let Some(mut buf) = renaming {
        // Inline rename field: commit on lost-focus when changed, then close the field.
        ui.horizontal(|ui| {
            ui.add_space(theme.spacing.md * depth as f32);
            ui.label(
                egui::RichText::new(icons::FOLDER.to_string())
                    .size(theme.type_scale.label)
                    .color(theme.accent.base),
            );
            let resp = ui.add(
                egui::TextEdit::singleline(&mut buf)
                    .hint_text(tr("codex.folder.name.placeholder"))
                    .desired_width(140.0),
            );
            if resp.changed() {
                ui.data_mut(|d| d.insert_temp(rename_id, buf.clone()));
            }
            if resp.lost_focus() {
                if !buf.trim().is_empty() && buf != node.name {
                    intents.push(Intent::RenameCodexFolder {
                        id: node.id,
                        name: buf.trim().to_owned(),
                    });
                }
                ui.data_mut(|d| d.remove_temp::<String>(rename_id));
            }
        });
    } else if let Some(action) = widgets::folder_row(ui, theme, &node.name, expanded, depth) {
        match action {
            widgets::FolderRowAction::ToggleExpanded => {
                expanded = !expanded;
                ui.data_mut(|d| d.insert_temp(exp_id, expanded));
            }
            widgets::FolderRowAction::Rename => {
                // Open the inline rename field seeded with the current name.
                ui.data_mut(|d| d.insert_temp(rename_id, node.name.clone()));
            }
            widgets::FolderRowAction::NewChild => intents.push(Intent::CreateCodexFolder {
                parent: Some(node.id),
                name: tr("codex.folder.untitled"),
            }),
            widgets::FolderRowAction::Delete => intents.push(Intent::DeleteCodexFolder { id: node.id }),
        }
    }
    if expanded {
        for summary in &node.entries {
            nav_folder_entry_row(ui, theme, intents, view, summary, depth + 1, to_select);
        }
        for child in &node.children {
            render_folder_node(ui, theme, intents, view, child, depth + 1, to_select);
        }
    }
}

/// Render one folder-resident entry row with a trailing "move to folder" menu. A click
/// selects the entry; a menu pick pushes `SetCodexEntryFolder`.
fn nav_folder_entry_row(
    ui: &mut egui::Ui,
    theme: &Theme,
    intents: &mut pixhaus_ui::state::intent::IntentSink,
    view: &CodexView,
    summary: &pixhaus_ui::state::session::CodexEntrySummary,
    depth: usize,
    to_select: &mut Option<pixhaus_core::CodexEntryId>,
) {
    ui.horizontal(|ui| {
        let data = widgets::NavNodeData {
            glyph: widgets::type_icon(summary.entry_type),
            label: &format!("@{}", summary.handle),
            count: None,
            depth,
            selected: view.selected == Some(summary.id),
            expandable: false,
            expanded: false,
            accent_glyph: false,
        };
        if widgets::nav_tree_node(ui, theme, &data).clicked {
            *to_select = Some(summary.id);
        }
        ui.menu_button(
            egui::RichText::new(icons::MOVE_TO.to_string())
                .size(theme.type_scale.label)
                .color(theme.roles.text_secondary),
            |ui| {
                if ui.button(tr("codex.action.move_to_root")).clicked() {
                    intents.push(Intent::SetCodexEntryFolder {
                        entry: summary.id,
                        folder: None,
                    });
                    ui.close();
                }
                for (fid, name) in view.flat_folders() {
                    if ui.button(&name).clicked() {
                        intents.push(Intent::SetCodexEntryFolder {
                            entry: summary.id,
                            folder: Some(fid),
                        });
                        ui.close();
                    }
                }
            },
        );
    });
}

// ===========================================================================
// Center: header + detail tab bar + tab bodies (the entry page)
// ===========================================================================

/// The Entry Editor panel (full center): a rich entry header, a detail tab bar, and the
/// active tab's body (Overview / Visual / Anchors / Prompt / Coverage / Relations /
/// History). The header keeps inline rename and handle editing working; the tabs
/// reorganize the former big edit form (spec 3-4).
pub struct EditorPanel;

impl Panel for EditorPanel {
    fn id(&self) -> PanelId {
        EDITOR
    }

    fn meta(&self) -> PanelMeta {
        PanelMeta {
            title: MsgKey("panel.codex-editor.title"),
            icon: icons::CODEX,
            default_region: Region::Center,
            default_open: true,
        }
    }

    fn ui(&self, ui: &mut egui::Ui, scope: &mut PanelScope<'_>) {
        let theme = scope.ctx.theme;
        let view = &scope.ctx.session.codex;
        let Some(detail) = view.detail.as_ref() else {
            ui.add_space(theme.spacing.xl);
            ui.vertical_centered(|ui| {
                ui.label(egui::RichText::new(icons::CODEX.to_string()).size(48.0).color(theme.roles.text_disabled));
                ui.label(
                    egui::RichText::new(tr("codex.editor.empty"))
                        .size(theme.type_scale.body)
                        .color(theme.roles.text_secondary),
                );
            });
            return;
        };

        // Clone the detail out so the editor draft borrow and the read borrow do not
        // overlap. The detail is a small metadata snapshot, not the pixel hot path.
        let detail = detail.clone();
        let tab = view.detail_tab;

        egui::ScrollArea::vertical().show(ui, |ui| {
            // The header is shell-owned-draft-aware (inline rename), so it takes the draft.
            header(ui, theme, scope.ctx.intents, scope.draft.as_deref_mut(), &detail);
            ui.add_space(theme.spacing.sm);

            // The detail tab bar.
            let tabs = [
                widgets::DetailTab {
                    label: &tr("codex.tab.overview"),
                    glyph: icons::INSPECTOR,
                    value: CodexDetailTab::Overview,
                },
                widgets::DetailTab {
                    label: &tr("codex.tab.visual"),
                    glyph: icons::BOARD,
                    value: CodexDetailTab::Visual,
                },
                widgets::DetailTab {
                    label: &tr("codex.tab.anchors"),
                    glyph: icons::ANCHOR,
                    value: CodexDetailTab::Anchors,
                },
                widgets::DetailTab {
                    label: &tr("codex.tab.prompt"),
                    glyph: icons::SPARKLE,
                    value: CodexDetailTab::Prompt,
                },
                widgets::DetailTab {
                    label: &tr("codex.tab.coverage"),
                    glyph: icons::COVERAGE,
                    value: CodexDetailTab::Coverage,
                },
                widgets::DetailTab {
                    label: &tr("codex.tab.relations"),
                    glyph: icons::GRAPH,
                    value: CodexDetailTab::Relations,
                },
                widgets::DetailTab {
                    label: &tr("codex.tab.history"),
                    glyph: icons::HISTORY,
                    value: CodexDetailTab::History,
                },
            ];
            if let Some(next) = widgets::detail_tab_bar(ui, theme, &tabs, tab) {
                scope.ctx.intents.push(Intent::SetCodexDetailTab(next));
            }
            ui.separator();
            ui.add_space(theme.spacing.sm);

            match tab {
                CodexDetailTab::Overview => {
                    if let Some(draft) = scope.draft.as_deref_mut() {
                        overview_tab(ui, theme, scope.ctx.intents, draft, &detail);
                    }
                }
                CodexDetailTab::Visual => {
                    if let Some(draft) = scope.draft.as_deref_mut() {
                        visual_tab(ui, theme, scope.ctx.intents, draft, &detail);
                    }
                }
                CodexDetailTab::Anchors => anchors_tab(ui, theme, scope.ctx.intents, &detail),
                CodexDetailTab::Prompt => prompt_tab(ui, theme, scope.ctx.intents, &detail),
                CodexDetailTab::Coverage => coverage_body(ui, theme, scope.ctx.intents, view, detail.summary.id),
                CodexDetailTab::Relations => relations_tab(ui, theme, scope.ctx.intents, view, &detail),
                CodexDetailTab::History => history_body(ui, theme, &detail),
            }
        });
    }
}

/// The entry header: a type-colored portrait placeholder, the name + status badge +
/// `@handle` chip + type chip, the description, tag chips, and the action cluster (Test
/// generate / Duplicate / overflow). Inline rename (name + handle) edits the shell-owned
/// draft and commits on lost-focus. The Type/Status/Created/Updated/Author/ID/Version
/// metadata is NOT repeated here - it lives only in the Overview Key Info card.
#[allow(clippy::too_many_lines)]
fn header(
    ui: &mut egui::Ui,
    theme: &Theme,
    intents: &mut pixhaus_ui::state::intent::IntentSink,
    mut draft: Option<&mut CodexEditorDraft>,
    detail: &CodexEntryDetail,
) {
    let id = detail.summary.id;
    ui.horizontal(|ui| {
        // The portrait placeholder: a type-colored blob on the inset surface (MOCK -
        // no real key-visual store yet).
        let (rect, _) = ui.allocate_exact_size(egui::Vec2::splat(96.0), egui::Sense::hover());
        if ui.is_rect_visible(rect) {
            let painter = ui.painter();
            painter.rect_filled(rect, 3.0, theme.surface(pixhaus_ui::theme::tokens::SurfaceTier::Inset));
            let blob = egui::Rect::from_center_size(rect.center(), egui::Vec2::splat(56.0));
            painter.rect_filled(blob, 3.0, widgets::type_color(theme, detail.summary.entry_type));
        }

        ui.add_space(theme.spacing.md);
        ui.vertical(|ui| {
            // Name + status + identity chips.
            ui.horizontal(|ui| {
                ui.label(
                    egui::RichText::new(&detail.summary.name)
                        .size(theme.type_scale.title)
                        .color(theme.roles.text_primary)
                        .strong(),
                );
                widgets::status_badge(ui, theme, detail.summary.status, &tr(status_key(detail.summary.status)));
                widgets::handle_chip(ui, theme, &detail.summary.handle);
                widgets::type_chip(ui, theme, detail.summary.entry_type, &tr(entry_type_key(detail.summary.entry_type)));
            });

            // Description line (truncated to one line by the layout width).
            if !detail.description.is_empty() {
                ui.label(
                    egui::RichText::new(&detail.description)
                        .size(theme.type_scale.body)
                        .color(theme.roles.text_secondary),
                );
            }

            // Tag chips.
            if !detail.tags.is_empty() {
                ui.horizontal_wrapped(|ui| {
                    for tag in &detail.tags {
                        widgets::chip(ui, theme, tag);
                    }
                });
            }

            // Action cluster: Test generate (primary AI), Duplicate, overflow.
            ui.horizontal(|ui| {
                let test = egui::Button::new(
                    egui::RichText::new(format!("{} {}", icons::SPARKLE, tr("codex.header.action.test_generate")))
                        .size(theme.type_scale.label)
                        .color(theme.accent.ai),
                )
                .frame(true);
                if ui.add(test).clicked() {
                    intents.push(Intent::SetCodexDetailTab(CodexDetailTab::Prompt));
                }
                if ui
                    .add(
                        egui::Button::new(
                            egui::RichText::new(format!("{} {}", icons::DUPLICATE, tr("codex.action.duplicate")))
                                .size(theme.type_scale.label)
                                .color(theme.roles.text_secondary),
                        )
                        .frame(true),
                    )
                    .clicked()
                {
                    intents.push(Intent::DuplicateCodexEntry(id));
                }
                ui.menu_button(
                    egui::RichText::new(icons::SLIDERS.to_string())
                        .size(theme.type_scale.label)
                        .color(theme.roles.text_secondary),
                    |ui| {
                        if ui.button(format!("{} {}", icons::PROMOTE, tr("codex.action.promote"))).clicked() {
                            intents.push(Intent::PromoteCodexEntry(id));
                            ui.close();
                        }
                        if ui.button(format!("{} {}", icons::ARCHIVE, tr("codex.action.archive"))).clicked() {
                            intents.push(Intent::ArchiveCodexEntry(id));
                            ui.close();
                        }
                        if ui.button(format!("{} {}", icons::RENAME, tr("codex.action.rename"))).clicked() {
                            if let Some(draft) = draft.as_mut() {
                                draft.renaming = !draft.renaming;
                            }
                            ui.close();
                        }
                        if ui.button(format!("{} {}", icons::TRASH, tr("codex.action.delete_entry"))).clicked() {
                            intents.push(Intent::DeleteCodexEntry(id));
                            ui.close();
                        }
                    },
                );
            });
        });
    });

    // Inline rename (name + handle), when the draft's rename flag is open.
    if let Some(draft) = draft.as_mut().filter(|d| d.renaming) {
        field_row(ui, theme, &tr("codex.field.name"), |ui| {
            let resp = ui.add(egui::TextEdit::singleline(&mut draft.name).desired_width(f32::INFINITY));
            if resp.lost_focus() && draft.name != detail.summary.name {
                intents.push(Intent::UpdateCodexEntryField {
                    id,
                    name: Some(draft.name.clone()),
                    description: None,
                    lore: None,
                    visual_description: None,
                    tags: None,
                });
            }
        });
        field_row(ui, theme, &tr("codex.field.handle"), |ui| {
            let resp = ui.add(
                egui::TextEdit::singleline(&mut draft.handle)
                    .hint_text(tr("codex.field.handle.placeholder"))
                    .desired_width(f32::INFINITY),
            );
            if resp.lost_focus() && draft.handle != detail.summary.handle {
                intents.push(Intent::SetCodexHandle {
                    id,
                    handle: draft.handle.clone(),
                });
            }
        });
    }
}

/// Build the header key-info rows (type / status / created / updated / author / id /
/// version). Timestamps and author come from the version-history mirror; absent values
/// render the localized dash.
fn key_info_rows(detail: &CodexEntryDetail) -> Vec<(String, String)> {
    let none = tr("codex.keyinfo.none");
    let ms_label = |ms: Option<u64>| ms.map_or_else(|| none.clone(), |v| v.to_string());
    let author = if detail.author.is_empty() { none.clone() } else { detail.author.clone() };
    vec![
        (tr("codex.field.type"), tr(entry_type_key(detail.summary.entry_type))),
        (tr("codex.field.status"), tr(status_key(detail.summary.status))),
        (tr("codex.keyinfo.created"), ms_label(detail.created_ms)),
        (tr("codex.keyinfo.updated"), ms_label(detail.updated_ms)),
        (tr("codex.keyinfo.author"), author),
        (tr("codex.keyinfo.id"), detail.summary.id.0.to_string()),
        (tr("codex.keyinfo.version"), detail.version.to_string()),
    ]
}

/// A labelled field row: a small caption then the field-building closure.
fn field_row(ui: &mut egui::Ui, theme: &Theme, label: &str, add: impl FnOnce(&mut egui::Ui)) {
    ui.add_space(theme.spacing.xs);
    ui.label(egui::RichText::new(label).size(theme.type_scale.label).color(theme.roles.text_secondary));
    add(ui);
}

/// A multiline `TextEdit` bound to a draft buffer that commits its diff against
/// `current` on lost-focus by pushing the intent `make` builds from the new text.
fn commit_multiline(
    ui: &mut egui::Ui,
    buffer: &mut String,
    current: &str,
    make: impl FnOnce(String) -> Intent,
    intents: &mut pixhaus_ui::state::intent::IntentSink,
) {
    let resp = ui.add(egui::TextEdit::multiline(buffer).desired_rows(3).desired_width(f32::INFINITY));
    if resp.lost_focus() && buffer != current {
        intents.push(make(buffer.clone()));
    }
}

// ---------------------------------------------------------------------------
// Overview tab: a multi-card grid.
// ---------------------------------------------------------------------------

/// The Overview cards, in their fixed home order. `distribute` round-robins this order
/// into the chosen column count, so at three columns the spec grouping falls out:
/// `[Summary, Role, Notes]` | `[KeyInfo, Tags, Details]` | `[Lore, QuickLinks]`. The
/// `Details` card (the rich per-type editor) extends the list past the seven spec cards
/// so per-type fields stay editable in the grid.
const OVERVIEW_CARDS: [OverviewCard; 8] = [
    OverviewCard::Summary,
    OverviewCard::KeyInfo,
    OverviewCard::Lore,
    OverviewCard::Role,
    OverviewCard::Tags,
    OverviewCard::QuickLinks,
    OverviewCard::Notes,
    OverviewCard::Details,
];

/// One Overview card kind. Placement is decided by [`OVERVIEW_CARDS`] + `distribute`;
/// the body each kind draws is unchanged from the former single-stack version.
#[derive(Clone, Copy)]
enum OverviewCard {
    /// The editable description (Summary).
    Summary,
    /// The read-only metadata stat rows + handle/aliases (Key Info).
    KeyInfo,
    /// The editable lore.
    Lore,
    /// The DERIVED role-in-project stat rows.
    Role,
    /// The editable tag list.
    Tags,
    /// Chips to related entries.
    QuickLinks,
    /// The editable notes field.
    Notes,
    /// The rich per-type details editor.
    Details,
}

/// The Overview tab: Summary, Key info, Lore, Role in project, Tags, Quick links, Notes,
/// and the per-type details, laid into 1/2/3 responsive columns by pane width. Editable
/// cards commit through the existing `Update*`/`Set*` intents and the editor draft; only
/// the placement changed, not the card content or the intents it pushes.
fn overview_tab(
    ui: &mut egui::Ui,
    theme: &Theme,
    intents: &mut pixhaus_ui::state::intent::IntentSink,
    draft: &mut CodexEditorDraft,
    detail: &CodexEntryDetail,
) {
    let columns = widgets::column_count(ui.available_width());
    // Responsive Overview grid: column_count picks 1/2/3 columns from pane width and
    // distribute round-robins the cards, both pure so the layout is tested without a
    // live egui frame. The cards render in sequential bodies below, not a stored
    // Vec<closure>, so each can borrow &mut intents / &mut draft; folding this nested
    // loop into a closure vec would reintroduce that borrow conflict.
    let layout = widgets::distribute(OVERVIEW_CARDS.len(), columns);
    ui.columns(columns, |cols| {
        for (c, idxs) in layout.iter().enumerate() {
            for (slot, &i) in idxs.iter().enumerate() {
                // A consistent small gap between stacked cards within a column (the gap
                // precedes every card after the column's first).
                if slot > 0 {
                    cols[c].add_space(theme.spacing.sm);
                }
                render_overview_card(&mut cols[c], theme, intents, draft, detail, OVERVIEW_CARDS[i]);
            }
        }
    });
}

/// Draw one Overview card into `ui`. The bodies are unchanged from the former
/// single-stack `overview_tab`; this is the per-kind dispatch the column grid calls.
#[allow(clippy::too_many_lines)]
fn render_overview_card(
    ui: &mut egui::Ui,
    theme: &Theme,
    intents: &mut pixhaus_ui::state::intent::IntentSink,
    draft: &mut CodexEditorDraft,
    detail: &CodexEntryDetail,
    card: OverviewCard,
) {
    let id = detail.summary.id;
    match card {
        OverviewCard::Summary => {
            widgets::info_card(ui, theme, icons::INSPECTOR, &tr("codex.overview.card.summary"), false, |ui| {
                commit_multiline(
                    ui,
                    &mut draft.description,
                    &detail.description,
                    |text| Intent::UpdateCodexEntryField {
                        id,
                        name: None,
                        description: Some(text),
                        lore: None,
                        visual_description: None,
                        tags: None,
                    },
                    intents,
                );
            });
        }
        OverviewCard::KeyInfo => {
            // Key info (read-only stat rows) plus the editable aliases list. The full
            // metadata lives here only (the header no longer repeats it); a collapsing
            // "View all metadata" reveals the less-glanceable id/version fields.
            widgets::info_card(ui, theme, icons::TAG, &tr("codex.overview.card.key_info"), false, |ui| {
                let rows = key_info_rows(detail);
                // The four glanceable fields stay visible; the id/version (and any extra)
                // hide behind the expander.
                let glance = rows.len().min(KEY_INFO_GLANCE_ROWS);
                for (label, value) in rows.iter().take(glance) {
                    widgets::stat_row(ui, theme, label, value, false);
                }
                widgets::stat_row(ui, theme, &tr("codex.field.handle"), &format!("@{}", detail.summary.handle), false);
                if rows.len() > glance {
                    egui::CollapsingHeader::new(
                        egui::RichText::new(tr("codex.overview.keyinfo.view_all"))
                            .size(theme.type_scale.label)
                            .color(theme.roles.text_secondary),
                    )
                    .id_salt(("codex-keyinfo-viewall", id.0))
                    .default_open(false)
                    .show(ui, |ui| {
                        for (label, value) in rows.iter().skip(glance) {
                            widgets::stat_row(ui, theme, label, value, false);
                        }
                    });
                }
                ui.add_space(theme.spacing.xs);
                ui.label(
                    egui::RichText::new(tr("codex.field.aliases"))
                        .size(theme.type_scale.label)
                        .color(theme.roles.text_secondary),
                );
                if let Some(action) = widgets::editable_list(ui, theme, &detail.aliases, &mut draft.alias_add, &tr("codex.field.alias.placeholder")) {
                    match action {
                        widgets::ListAction::Add(alias) => {
                            intents.push(Intent::AddCodexAlias { id, alias });
                            draft.alias_add.clear();
                        }
                        widgets::ListAction::Remove(i) => {
                            if let Some(alias) = detail.aliases.get(i) {
                                intents.push(Intent::RemoveCodexAlias { id, alias: alias.clone() });
                            }
                        }
                    }
                }
            });
        }
        OverviewCard::Lore => {
            widgets::info_card(ui, theme, icons::RULE, &tr("codex.overview.card.lore"), false, |ui| {
                commit_multiline(
                    ui,
                    &mut draft.lore,
                    &detail.lore,
                    |text| Intent::UpdateCodexEntryField {
                        id,
                        name: None,
                        description: None,
                        lore: Some(text),
                        visual_description: None,
                        tags: None,
                    },
                    intents,
                );
            });
        }
        OverviewCard::Role => {
            widgets::info_card(ui, theme, icons::LINK, &tr("codex.overview.card.role"), false, |ui| {
                // The coverage ratio is clamped 0..1, so `*100` lands in 0..=100 - a u8
                // holds it exactly; the rounded value cannot truncate or lose a sign.
                #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
                let pct = (detail.summary.coverage_ratio.clamp(0.0, 1.0) * 100.0).round() as u8;
                widgets::stat_row(ui, theme, &tr("codex.tab.relations"), &detail.relations.len().to_string(), false);
                widgets::stat_row(ui, theme, &tr("codex.inspector.used_in"), &detail.used_by.len().to_string(), false);
                widgets::stat_row(ui, theme, &tr("codex.tab.coverage"), &format!("{pct}%"), false);
            });
        }
        OverviewCard::Tags => {
            widgets::info_card(ui, theme, icons::TAG, &tr("codex.overview.card.tags"), false, |ui| {
                if let Some(action) = widgets::editable_list(ui, theme, &detail.tags, &mut draft.tag_add, &tr("codex.field.tag.placeholder")) {
                    let mut tags = detail.tags.clone();
                    match action {
                        widgets::ListAction::Add(tag) => {
                            tags.push(tag);
                            draft.tag_add.clear();
                        }
                        widgets::ListAction::Remove(i) => {
                            if i < tags.len() {
                                tags.remove(i);
                            }
                        }
                    }
                    intents.push(Intent::UpdateCodexEntryField {
                        id,
                        name: None,
                        description: None,
                        lore: None,
                        visual_description: None,
                        tags: Some(tags),
                    });
                }
            });
        }
        OverviewCard::QuickLinks => {
            widgets::info_card(ui, theme, icons::GRAPH, &tr("codex.overview.card.quick_links"), false, |ui| {
                let links: Vec<&pixhaus_ui::state::session::CodexRelation> = detail.relations.iter().chain(detail.used_by.iter()).collect();
                if links.is_empty() {
                    ui.label(
                        egui::RichText::new(tr("codex.keyinfo.none"))
                            .size(theme.type_scale.label)
                            .color(theme.roles.text_disabled),
                    );
                } else {
                    let mut to_select = None;
                    ui.horizontal_wrapped(|ui| {
                        for rel in links {
                            // The chip uses the entry-type color; the relation list does
                            // not carry the other end's type, so colorize by the current
                            // entry's type.
                            if widgets::quick_link_chip(ui, theme, &rel.other_label, detail.summary.entry_type) {
                                to_select = Some(rel.other);
                            }
                        }
                    });
                    if let Some(other) = to_select {
                        intents.push(Intent::SelectCodexEntry(other));
                    }
                }
            });
        }
        OverviewCard::Notes => {
            // Notes (editable; reuses the GenericDetails `notes` key, populated for
            // Generic entries only - read-only display for other types, with a MOCK note).
            widgets::info_card(ui, theme, icons::RENAME, &tr("codex.overview.card.notes"), false, |ui| {
                notes_field(ui, theme, intents, detail);
            });
        }
        OverviewCard::Details => {
            // Type details (the rich type-specific editor, reorganized from the former
            // big form into the Overview grid so per-type fields stay editable).
            widgets::info_card(ui, theme, icons::MATERIAL, &tr("codex.editor.section.details"), false, |ui| {
                details_editor(ui, theme, intents, detail);
            });
        }
    }
}

/// How many Key Info rows stay always-visible; the rest fold behind the "View all
/// metadata" expander. `key_info_rows` yields type/status/created/updated then
/// author/id/version, so the first four are the glanceable set.
const KEY_INFO_GLANCE_ROWS: usize = 4;

/// The type-specific details editor: rich editable fields for Character / Palette /
/// Style / Animation, a key/value list for Generic. Each edit commits the whole body as
/// the matching `Set*Details` intent.
fn details_editor(ui: &mut egui::Ui, theme: &Theme, intents: &mut pixhaus_ui::state::intent::IntentSink, detail: &CodexEntryDetail) {
    use pixhaus_core::codex::EntryDetails;
    let id = detail.summary.id;
    match &detail.details {
        EntryDetails::Palette(body) => palette_details(ui, theme, intents, id, body),
        EntryDetails::Character(body) => character_details(ui, theme, intents, id, body),
        EntryDetails::Style(body) => style_details(ui, theme, intents, id, body),
        EntryDetails::Animation(body) => animation_details(ui, theme, intents, id, body),
        EntryDetails::Generic(body) => generic_details(ui, theme, intents, id, body),
    }
}

/// The Notes field: for Generic entries it edits the reserved `notes` key through
/// `SetGenericDetails`; for other types the model has no notes slot, so it shows the
/// current notes (always empty) as a disabled placeholder (a MOCK stand-in).
fn notes_field(ui: &mut egui::Ui, theme: &Theme, intents: &mut pixhaus_ui::state::intent::IntentSink, detail: &CodexEntryDetail) {
    use pixhaus_core::codex::{EntryDetails, GenericField};
    let id = detail.summary.id;
    match &detail.details {
        EntryDetails::Generic(body) => {
            let buf_id = ui.make_persistent_id(("codex-notes", id.0));
            let mut text = ui.data(|d| d.get_temp::<String>(buf_id)).unwrap_or_else(|| detail.notes.clone());
            let resp = ui.add(egui::TextEdit::multiline(&mut text).desired_rows(3).desired_width(f32::INFINITY));
            if resp.changed() {
                ui.data_mut(|d| d.insert_temp(buf_id, text.clone()));
            }
            if resp.lost_focus() && text != detail.notes {
                let mut next = body.clone();
                if let Some(field) = next.fields.iter_mut().find(|f| f.key == "notes") {
                    field.value = text;
                } else {
                    next.fields.push(GenericField {
                        key: "notes".to_owned(),
                        value: text,
                    });
                }
                intents.push(Intent::SetGenericDetails { id, body: next });
            }
        }
        _ => {
            ui.label(
                egui::RichText::new(tr("codex.keyinfo.none"))
                    .size(theme.type_scale.label)
                    .color(theme.roles.text_disabled),
            );
        }
    }
}

// ---------------------------------------------------------------------------
// Visual tab.
// ---------------------------------------------------------------------------

/// The Visual cards in their fixed home order. `distribute` round-robins these into the
/// chosen column count exactly as the Overview grid does.
const VISUAL_CARDS: [VisualCard; 6] = [
    VisualCard::Description,
    VisualCard::KeyVisual,
    VisualCard::Palette,
    VisualCard::Silhouette,
    VisualCard::QuickAnchors,
    VisualCard::Readiness,
];

/// One Visual card kind. Placement is decided by [`VISUAL_CARDS`] + `distribute`; each
/// body is the former Visual section, now wrapped in an `info_card` so it content-sizes.
#[derive(Clone, Copy)]
enum VisualCard {
    /// The editable visual-identity field.
    Description,
    /// The MOCK key-visual thumbnail strip.
    KeyVisual,
    /// Palette swatches (real for a Palette entry, else a MOCK strip).
    Palette,
    /// Silhouette placeholders + the character's silhouette caption.
    Silhouette,
    /// The quick-anchor badges.
    QuickAnchors,
    /// The DERIVED generation-readiness ring.
    Readiness,
}

/// The Visual tab: an editable visual-identity field, a MOCK key-visual thumbnail strip,
/// palette swatches (real for a Palette entry), silhouette placeholders, quick anchors,
/// and the generation-readiness ring (spec 4.2), laid into 1/2/3 responsive columns by
/// pane width. The card content and intents are unchanged; only the placement is.
fn visual_tab(ui: &mut egui::Ui, theme: &Theme, intents: &mut pixhaus_ui::state::intent::IntentSink, draft: &mut CodexEditorDraft, detail: &CodexEntryDetail) {
    let columns = widgets::column_count(ui.available_width());
    let layout = widgets::distribute(VISUAL_CARDS.len(), columns);
    ui.columns(columns, |cols| {
        for (c, idxs) in layout.iter().enumerate() {
            for (slot, &i) in idxs.iter().enumerate() {
                if slot > 0 {
                    cols[c].add_space(theme.spacing.sm);
                }
                render_visual_card(&mut cols[c], theme, intents, draft, detail, VISUAL_CARDS[i]);
            }
        }
    });
}

/// Draw one Visual card into `ui`. The bodies match the former single-stack `visual_tab`,
/// wrapped in `info_card` frames so they content-size in the column grid.
fn render_visual_card(
    ui: &mut egui::Ui,
    theme: &Theme,
    intents: &mut pixhaus_ui::state::intent::IntentSink,
    draft: &mut CodexEditorDraft,
    detail: &CodexEntryDetail,
    card: VisualCard,
) {
    use pixhaus_core::codex::EntryDetails;
    let id = detail.summary.id;
    match card {
        VisualCard::Description => {
            widgets::info_card(ui, theme, icons::BOARD, &tr("codex.field.visual_description"), false, |ui| {
                commit_multiline(
                    ui,
                    &mut draft.visual_description,
                    &detail.visual_description,
                    |text| Intent::UpdateCodexEntryField {
                        id,
                        name: None,
                        description: None,
                        lore: None,
                        visual_description: Some(text),
                        tags: None,
                    },
                    intents,
                );
            });
        }
        VisualCard::KeyVisual => {
            widgets::info_card(ui, theme, icons::ASSETS, &tr("codex.inspector.linked_assets"), false, |ui| {
                widgets::mock_thumbnail_grid(ui, theme, 4);
            });
        }
        VisualCard::Palette => {
            widgets::info_card(ui, theme, icons::PALETTE, &tr("codex.palette.colors"), false, |ui| match &detail.details {
                EntryDetails::Palette(body) if !body.colors.is_empty() => {
                    ui.horizontal_wrapped(|ui| {
                        for color in &body.colors {
                            let (rect, _) = ui.allocate_exact_size(egui::Vec2::splat(theme.type_scale.body + 8.0), egui::Sense::hover());
                            if ui.is_rect_visible(rect) {
                                let [r, g, b, a] = color.rgba;
                                ui.painter().rect_filled(rect, 2.0, egui::Color32::from_rgba_unmultiplied(r, g, b, a));
                            }
                        }
                    });
                }
                _ => widgets::asset_thumbnail_strip(ui, theme, 5, 5),
            });
        }
        VisualCard::Silhouette => {
            widgets::info_card(ui, theme, icons::CHARACTER, &tr("codex.character.silhouette"), false, |ui| {
                widgets::asset_thumbnail_strip(ui, theme, 3, 3);
                if let EntryDetails::Character(body) = &detail.details
                    && !body.silhouette_notes.is_empty()
                {
                    ui.label(
                        egui::RichText::new(&body.silhouette_notes)
                            .size(theme.type_scale.label)
                            .color(theme.roles.text_secondary),
                    );
                }
            });
        }
        VisualCard::QuickAnchors => {
            widgets::info_card(ui, theme, icons::ANCHOR, &tr("codex.editor.section.anchors"), false, |ui| {
                ui.horizontal_wrapped(|ui| {
                    for anchor in &detail.anchors {
                        widgets::anchor_badge(
                            ui,
                            theme,
                            anchor.strength,
                            &tr(anchor_kind_key(anchor.kind)),
                            &tr(anchor_strength_key(anchor.strength)),
                        );
                    }
                });
            });
        }
        VisualCard::Readiness => {
            widgets::info_card(ui, theme, icons::READINESS, &tr("codex.readiness.title"), false, |ui| {
                widgets::score_ring(
                    ui,
                    theme,
                    icons::READINESS,
                    detail.readiness,
                    detail.readiness_percent,
                    &tr("codex.readiness.title"),
                );
            });
        }
    }
}

// ---------------------------------------------------------------------------
// Anchors tab.
// ---------------------------------------------------------------------------

/// The Anchors tab: every anchor with a kind badge, a 4-step strength selector (the
/// discrete reconciliation of the mockup's continuous AI-weight slider), an editable
/// statement, and a remove; an add-anchor picker; and the positive/negative rule lists
/// (spec 4.3).
/// One card in the Anchors tab grid: a single anchor, the add-anchor control, or one of
/// the positive/negative fragment lists. The anchor count is dynamic, so the card list
/// is a `Vec` rather than a fixed array.
enum AnchorCard {
    /// The anchor at this index in `detail.anchors`.
    Anchor(usize),
    /// The add-anchor picker.
    Add,
    /// The positive prompt fragments.
    Positive,
    /// The negative prompt fragments.
    Negative,
}

fn anchors_tab(ui: &mut egui::Ui, theme: &Theme, intents: &mut pixhaus_ui::state::intent::IntentSink, detail: &CodexEntryDetail) {
    // One card per anchor, then the add-anchor card, then the positive- and
    // negative-fragment cards. The dynamic anchor count means the card list is built at
    // render time rather than a fixed array.
    let mut cards: Vec<AnchorCard> = (0..detail.anchors.len()).map(AnchorCard::Anchor).collect();
    cards.push(AnchorCard::Add);
    cards.push(AnchorCard::Positive);
    cards.push(AnchorCard::Negative);

    let columns = widgets::column_count(ui.available_width());
    let layout = widgets::distribute(cards.len(), columns);
    ui.columns(columns, |cols| {
        for (c, idxs) in layout.iter().enumerate() {
            for (slot, &i) in idxs.iter().enumerate() {
                if slot > 0 {
                    cols[c].add_space(theme.spacing.sm);
                }
                match &cards[i] {
                    AnchorCard::Anchor(ai) => render_anchor_card(&mut cols[c], theme, intents, detail, *ai),
                    AnchorCard::Add => render_add_anchor_card(&mut cols[c], theme, intents, detail),
                    AnchorCard::Positive => render_positive_fragments_card(&mut cols[c], theme, intents, detail),
                    AnchorCard::Negative => render_negative_fragments_card(&mut cols[c], theme, intents, detail),
                }
            }
        }
    });
}

/// One anchor card: the kind badge, the 4-step strength selector, a remove control, and
/// the editable statement. Unchanged behavior, wrapped in an `info_card` for the grid.
fn render_anchor_card(ui: &mut egui::Ui, theme: &Theme, intents: &mut pixhaus_ui::state::intent::IntentSink, detail: &CodexEntryDetail, anchor_index: usize) {
    let Some(anchor) = detail.anchors.get(anchor_index) else {
        return;
    };
    let id = detail.summary.id;
    widgets::info_card(ui, theme, icons::ANCHOR, &tr(anchor_kind_key(anchor.kind)), false, |ui| {
        ui.horizontal(|ui| {
            widgets::anchor_badge(
                ui,
                theme,
                anchor.strength,
                &tr(anchor_kind_key(anchor.kind)),
                &tr(anchor_strength_key(anchor.strength)),
            );
            if let Some(strength) = widgets::strength_selector(ui, theme, anchor.strength, |s| tr(anchor_strength_key(s))) {
                intents.push(Intent::SetCodexAnchor {
                    id,
                    kind: anchor.kind,
                    strength,
                    statement: anchor.statement.clone(),
                });
            }
            if ui
                .add(
                    egui::Button::new(
                        egui::RichText::new(icons::TRASH.to_string())
                            .size(theme.type_scale.label)
                            .color(theme.roles.warning),
                    )
                    .frame(false),
                )
                .clicked()
            {
                intents.push(Intent::RemoveCodexAnchor { id, kind: anchor.kind });
            }
        });
        // The editable statement: a temp-buffer field committing through SetCodexAnchor
        // with the same kind/strength on lost-focus.
        temp_anchor_statement(ui, theme, intents, id, anchor.kind, anchor.strength, &anchor.statement);
    });
}

/// The add-anchor card: a picker over the not-yet-used anchor kinds.
fn render_add_anchor_card(ui: &mut egui::Ui, theme: &Theme, intents: &mut pixhaus_ui::state::intent::IntentSink, detail: &CodexEntryDetail) {
    let id = detail.summary.id;
    widgets::info_card(ui, theme, icons::ADD, &tr("codex.editor.section.anchors"), false, |ui| {
        let existing: Vec<AnchorKind> = detail.anchors.iter().map(|a| a.kind).collect();
        if let Some(kind) = widgets::add_anchor_picker(ui, theme, &existing, |k| tr(anchor_kind_key(k))) {
            let statement = if kind == AnchorKind::Visual {
                detail.visual_description.clone()
            } else {
                String::new()
            };
            intents.push(Intent::SetCodexAnchor {
                id,
                kind,
                strength: AnchorStrength::Normal,
                statement,
            });
        }
    });
}

/// The positive-fragment card (the entry's prompt fragments).
fn render_positive_fragments_card(ui: &mut egui::Ui, theme: &Theme, intents: &mut pixhaus_ui::state::intent::IntentSink, detail: &CodexEntryDetail) {
    let id = detail.summary.id;
    widgets::info_card(ui, theme, icons::SPARKLE, &tr("codex.field.prompt_fragments"), false, |ui| {
        let fragment_texts: Vec<String> = detail.prompt_fragments.iter().map(|f| f.text.clone()).collect();
        fragment_list(ui, theme, intents, id, "anchors_pos", &fragment_texts, &detail.prompt_fragments);
    });
}

/// The negative-fragment card (the entry's negative rules).
fn render_negative_fragments_card(ui: &mut egui::Ui, theme: &Theme, intents: &mut pixhaus_ui::state::intent::IntentSink, detail: &CodexEntryDetail) {
    let id = detail.summary.id;
    widgets::info_card(ui, theme, icons::WARN, &tr("codex.field.negative_fragments"), false, |ui| {
        negative_list(ui, theme, intents, id, "anchors_neg", &detail.negative_fragments);
    });
}

/// An editable anchor-statement row, bound to a temp buffer keyed by entry+kind, that
/// commits a `SetCodexAnchor` intent on lost-focus when the text changed. The buffer is
/// dropped on commit so it re-seeds from the committed value next frame.
fn temp_anchor_statement(
    ui: &mut egui::Ui,
    theme: &Theme,
    intents: &mut pixhaus_ui::state::intent::IntentSink,
    id: CodexEntryId,
    kind: AnchorKind,
    strength: AnchorStrength,
    current: &str,
) {
    let buf_id = ui.make_persistent_id(("codex-anchor-stmt", id.0, kind));
    let mut text = ui.data(|d| d.get_temp::<String>(buf_id)).unwrap_or_else(|| current.to_owned());
    field_row(ui, theme, &tr(anchor_kind_key(kind)), |ui| {
        let resp = ui.add(egui::TextEdit::singleline(&mut text).desired_width(f32::INFINITY));
        if resp.changed() {
            ui.data_mut(|d| d.insert_temp(buf_id, text.clone()));
        }
        if resp.lost_focus() {
            if text != current {
                intents.push(Intent::SetCodexAnchor {
                    id,
                    kind,
                    strength,
                    statement: text.clone(),
                });
            }
            ui.data_mut(|d| d.remove_temp::<String>(buf_id));
        }
    });
}

// ---------------------------------------------------------------------------
// Prompt tab.
// ---------------------------------------------------------------------------

/// The Prompt tab: the positive fragments with priority chips, the negative fragments,
/// and the compiled per-entry preview, with a Test generate button (spec 4.4).
/// One card in the Prompt tab grid.
#[derive(Clone, Copy)]
enum PromptCard {
    /// The positive prompt fragments (priority chips + editable list).
    Positive,
    /// The negative prompt fragments.
    Negative,
    /// The compiled per-entry preview.
    Preview,
}

fn prompt_tab(ui: &mut egui::Ui, theme: &Theme, intents: &mut pixhaus_ui::state::intent::IntentSink, detail: &CodexEntryDetail) {
    let id = detail.summary.id;

    // Positive, negative, and the compiled preview as three cards. The compiled text
    // reads better with width, so cap the prompt grid at two columns. The Test-generate
    // button stays full-width below the grid.
    let cards = [PromptCard::Positive, PromptCard::Negative, PromptCard::Preview];
    let columns = widgets::column_count(ui.available_width()).min(2);
    let layout = widgets::distribute(cards.len(), columns);
    ui.columns(columns, |cols| {
        for (c, idxs) in layout.iter().enumerate() {
            for (slot, &i) in idxs.iter().enumerate() {
                if slot > 0 {
                    cols[c].add_space(theme.spacing.sm);
                }
                match cards[i] {
                    PromptCard::Positive => {
                        widgets::info_card(&mut cols[c], theme, icons::SPARKLE, &tr("codex.field.prompt_fragments"), false, |ui| {
                            // Each fragment shows a priority chip ahead of its text.
                            for fragment in &detail.prompt_fragments {
                                ui.horizontal(|ui| {
                                    widgets::chip(ui, theme, &tr(priority_key(fragment.priority)));
                                    ui.label(egui::RichText::new(&fragment.text).size(theme.type_scale.label).color(theme.roles.text_primary));
                                });
                            }
                            let fragment_texts: Vec<String> = detail.prompt_fragments.iter().map(|f| f.text.clone()).collect();
                            fragment_list(ui, theme, intents, id, "prompt_pos", &fragment_texts, &detail.prompt_fragments);
                        });
                    }
                    PromptCard::Negative => {
                        widgets::info_card(&mut cols[c], theme, icons::WARN, &tr("codex.field.negative_fragments"), false, |ui| {
                            negative_list(ui, theme, intents, id, "prompt_neg", &detail.negative_fragments);
                        });
                    }
                    PromptCard::Preview => {
                        widgets::info_card(&mut cols[c], theme, icons::INSPECTOR, &tr("codex.inspector.preview.title"), false, |ui| {
                            prompt_preview_body(ui, theme, detail);
                        });
                    }
                }
            }
        }
    });

    ui.add_space(theme.spacing.sm);
    if ui
        .add(egui::Button::new(
            egui::RichText::new(format!("{} {}", icons::SPARKLE, tr("codex.header.action.test_generate")))
                .size(theme.type_scale.label)
                .color(theme.accent.ai),
        ))
        .clicked()
    {
        intents.push(Intent::CompileCodexPrompt {
            user_text: format!("@{}", detail.summary.handle),
        });
    }
}

/// The compiled per-entry prompt preview body (positive then negative, or an empty hint).
fn prompt_preview_body(ui: &mut egui::Ui, theme: &Theme, detail: &CodexEntryDetail) {
    match detail.prompt_preview.as_ref() {
        Some(compiled) if !compiled.positive.is_empty() => {
            ui.label(
                egui::RichText::new(tr("codex.inspector.preview.positive"))
                    .size(theme.type_scale.label)
                    .color(theme.roles.text_disabled),
            );
            ui.label(
                egui::RichText::new(&compiled.positive)
                    .size(theme.type_scale.label)
                    .color(theme.roles.text_secondary),
            );
            if !compiled.negative.is_empty() {
                ui.label(
                    egui::RichText::new(tr("codex.inspector.preview.negative"))
                        .size(theme.type_scale.label)
                        .color(theme.roles.text_disabled),
                );
                ui.label(
                    egui::RichText::new(&compiled.negative)
                        .size(theme.type_scale.label)
                        .color(theme.roles.text_secondary),
                );
            }
        }
        _ => {
            ui.label(
                egui::RichText::new(tr("codex.inspector.preview.empty"))
                    .size(theme.type_scale.label)
                    .color(theme.roles.text_disabled),
            );
        }
    }
}

/// An editable positive-fragment list (add appends a Normal-priority fragment; remove
/// drops by index), committing the whole list through `SetCodexPromptFragments`. The add
/// buffer lives in temp memory keyed by `field`.
fn fragment_list(
    ui: &mut egui::Ui,
    theme: &Theme,
    intents: &mut pixhaus_ui::state::intent::IntentSink,
    id: CodexEntryId,
    field: &'static str,
    texts: &[String],
    fragments: &[PromptFragment],
) {
    let buf_id = ui.make_persistent_id(("codex-frag-add", field, id.0));
    let mut add_buf = ui.data(|d| d.get_temp::<String>(buf_id)).unwrap_or_default();
    if let Some(action) = widgets::editable_list(ui, theme, texts, &mut add_buf, &tr("codex.field.fragment.placeholder")) {
        let mut next = fragments.to_vec();
        match action {
            widgets::ListAction::Add(text) => {
                next.push(PromptFragment::new(text, InclusionPriority::Normal));
                add_buf.clear();
            }
            widgets::ListAction::Remove(i) => {
                if i < next.len() {
                    next.remove(i);
                }
            }
        }
        intents.push(Intent::SetCodexPromptFragments { id, fragments: next });
    }
    ui.data_mut(|d| d.insert_temp(buf_id, add_buf));
}

/// An editable negative-fragment list, committing through `SetCodexNegativeFragments`.
fn negative_list(
    ui: &mut egui::Ui,
    theme: &Theme,
    intents: &mut pixhaus_ui::state::intent::IntentSink,
    id: CodexEntryId,
    field: &'static str,
    negatives: &[String],
) {
    let buf_id = ui.make_persistent_id(("codex-neg-add", field, id.0));
    let mut add_buf = ui.data(|d| d.get_temp::<String>(buf_id)).unwrap_or_default();
    if let Some(action) = widgets::editable_list(ui, theme, negatives, &mut add_buf, &tr("codex.field.negative.placeholder")) {
        let mut next = negatives.to_vec();
        match action {
            widgets::ListAction::Add(text) => {
                next.push(text);
                add_buf.clear();
            }
            widgets::ListAction::Remove(i) => {
                if i < next.len() {
                    next.remove(i);
                }
            }
        }
        intents.push(Intent::SetCodexNegativeFragments { id, fragments: next });
    }
    ui.data_mut(|d| d.insert_temp(buf_id, add_buf));
}

// ---------------------------------------------------------------------------
// Relations tab.
// ---------------------------------------------------------------------------

/// The Relations tab: a graph/list toggle (the list view this pass), outgoing edges
/// (removable, target navigates), the used-by edges, and an add-relationship control
/// (spec 4.6).
fn relations_tab(ui: &mut egui::Ui, theme: &Theme, intents: &mut pixhaus_ui::state::intent::IntentSink, view: &CodexView, detail: &CodexEntryDetail) {
    let from = detail.summary.id;
    // Graph / List toggle (the graph view is a later increment; List is the current
    // edge list). The toggle persists in temp memory.
    let toggle_id = ui.make_persistent_id(("codex-rel-view", from.0));
    let mut as_list = ui.data_mut(|d| d.get_temp::<bool>(toggle_id).unwrap_or(true));
    ui.horizontal(|ui| {
        if widgets::workspace_tab(ui, theme, &format!("{} {}", icons::COVERAGE, tr("codex.relations.toggle.list")), as_list).clicked() {
            as_list = true;
            ui.data_mut(|d| d.insert_temp(toggle_id, as_list));
        }
        if widgets::workspace_tab(ui, theme, &format!("{} {}", icons::GRAPH, tr("codex.relations.toggle.graph")), !as_list).clicked() {
            as_list = false;
            ui.data_mut(|d| d.insert_temp(toggle_id, as_list));
        }
    });
    ui.separator();

    widgets::section_header(ui, theme, icons::GRAPH, &tr("codex.editor.section.relationships"));

    // Outgoing edges (both views render the same rows this pass).
    for rel in &detail.relations {
        ui.horizontal(|ui| {
            ui.label(egui::RichText::new(format!("@{}", detail.summary.handle)).color(theme.accent.base));
            // The kind reads as a menu button: picking a different kind retypes the edge in
            // place (one undo step), so a relationship's kind is editable without a manual
            // remove + re-add.
            ui.menu_button(egui::RichText::new(tr(relation_key(rel.kind))).color(theme.roles.text_secondary), |ui| {
                for k in RELATION_KINDS {
                    if k != rel.kind && ui.button(tr(relation_key(k))).clicked() {
                        intents.push(Intent::ChangeRelationshipKind {
                            from,
                            old_kind: rel.kind,
                            to: rel.other,
                            new_kind: k,
                        });
                        ui.close();
                    }
                }
            })
            .response
            .on_hover_text(tr("codex.action.change_relationship_kind"));
            if ui
                .add(egui::Button::new(egui::RichText::new(&rel.other_label).color(theme.roles.text_primary)).frame(false))
                .clicked()
            {
                intents.push(Intent::SelectCodexEntry(rel.other));
            }
            if ui
                .add(
                    egui::Button::new(
                        egui::RichText::new(icons::TRASH.to_string())
                            .size(theme.type_scale.label)
                            .color(theme.roles.warning),
                    )
                    .frame(false),
                )
                .clicked()
            {
                intents.push(Intent::RemoveCodexRelationship {
                    from,
                    kind: rel.kind,
                    to: rel.other,
                });
            }
        });
    }

    // Incoming edges (used by), clickable to navigate.
    if !detail.used_by.is_empty() {
        ui.add_space(theme.spacing.sm);
        widgets::section_header(ui, theme, icons::REFERENCE, &tr("codex.inspector.references_used"));
        for rel in &detail.used_by {
            ui.horizontal(|ui| {
                if ui
                    .add(egui::Button::new(egui::RichText::new(&rel.other_label).color(theme.roles.text_primary)).frame(false))
                    .clicked()
                {
                    intents.push(Intent::SelectCodexEntry(rel.other));
                }
                ui.label(egui::RichText::new(tr(relation_key(rel.kind))).color(theme.roles.text_secondary));
            });
        }
    }

    // Add a relationship.
    ui.add_space(theme.spacing.md);
    add_relationship_control(ui, theme, intents, view, from);
}

// ---------------------------------------------------------------------------
// History tab/body.
// ---------------------------------------------------------------------------

/// The History body: a version timeline from the entry's version-history mirror. Empty
/// history shows a muted empty-state line (the former hardcoded mock log is gone).
fn history_body(ui: &mut egui::Ui, theme: &Theme, detail: &CodexEntryDetail) {
    widgets::section_header(ui, theme, icons::HISTORY, &tr("codex.tab.history"));
    if detail.version_history.is_empty() {
        ui.label(
            egui::RichText::new(tr("codex.history.empty"))
                .size(theme.type_scale.label)
                .color(theme.roles.text_disabled),
        );
        return;
    }
    // Newest first.
    for v in detail.version_history.iter().rev() {
        let author = if v.author.is_empty() {
            tr("codex.history.author_unknown")
        } else {
            v.author.clone()
        };
        widgets::history_timeline_row(ui, theme, v.version, &author, &v.summary);
    }
}

// ===========================================================================
// Type-specific details editors (reused under the tabs / inspector).
// ===========================================================================

/// A temp-memory string buffer keyed by `(entry_id, field)`, seeded once from `current`.
/// Used for the type-specific text fields, which have no shell draft slot.
fn temp_buffer(ui: &mut egui::Ui, key: (&'static str, u64), current: &str) -> egui::Id {
    let buf_id = ui.make_persistent_id(("codex-details", key.0, key.1));
    if ui.data(|d| d.get_temp::<String>(buf_id)).is_none() {
        ui.data_mut(|d| d.insert_temp(buf_id, current.to_owned()));
    }
    buf_id
}

/// A single-line text field bound to a temp buffer that commits on lost-focus when it
/// differs from `current`, calling `commit` with the new text.
fn temp_text_row(ui: &mut egui::Ui, theme: &Theme, id: CodexEntryId, field: &'static str, label: &str, current: &str, commit: impl FnOnce(String)) {
    let buf_id = temp_buffer(ui, (field, id.0), current);
    field_row(ui, theme, label, |ui| {
        let mut text = ui.data(|d| d.get_temp::<String>(buf_id)).unwrap_or_default();
        let resp = ui.add(egui::TextEdit::singleline(&mut text).desired_width(f32::INFINITY));
        if resp.changed() {
            ui.data_mut(|d| d.insert_temp(buf_id, text.clone()));
        }
        if resp.lost_focus() && text != current {
            commit(text);
        }
    });
}

/// Join handles into the display list the `editable_list` widget shows (one per row).
fn handle_display(handles: &[pixhaus_core::codex::CodexHandle]) -> Vec<String> {
    handles.iter().map(|h| h.as_str().to_owned()).collect()
}

/// The Character details editor.
fn character_details(
    ui: &mut egui::Ui,
    theme: &Theme,
    intents: &mut pixhaus_ui::state::intent::IntentSink,
    id: CodexEntryId,
    body: &pixhaus_core::codex::CharacterDetails,
) {
    use pixhaus_core::codex::CodexHandle;
    temp_text_row(ui, theme, id, "char_prop", &tr("codex.character.proportions"), &body.proportions, |text| {
        let mut next = body.clone();
        next.proportions = text;
        intents.push(Intent::SetCharacterDetails { id, body: next });
    });
    temp_text_row(ui, theme, id, "char_sil", &tr("codex.character.silhouette"), &body.silhouette_notes, |text| {
        let mut next = body.clone();
        next.silhouette_notes = text;
        intents.push(Intent::SetCharacterDetails { id, body: next });
    });
    let palette_ref = body.palette_ref.as_ref().map(|h| h.as_str().to_owned()).unwrap_or_default();
    temp_text_row(ui, theme, id, "char_pal", &tr("codex.character.palette_ref"), &palette_ref, |text| {
        let mut next = body.clone();
        next.palette_ref = if text.trim().is_empty() {
            None
        } else {
            CodexHandle::new(text.trim().to_lowercase()).ok()
        };
        intents.push(Intent::SetCharacterDetails { id, body: next });
    });
    handle_list_field(
        ui,
        theme,
        id,
        "char_allow",
        &tr("codex.character.allowed_styles"),
        &body.allowed_styles,
        |next_handles| {
            let mut next = body.clone();
            next.allowed_styles = next_handles;
            intents.push(Intent::SetCharacterDetails { id, body: next });
        },
    );
    handle_list_field(
        ui,
        theme,
        id,
        "char_forbid",
        &tr("codex.character.forbidden_styles"),
        &body.forbidden_styles,
        |next_handles| {
            let mut next = body.clone();
            next.forbidden_styles = next_handles;
            intents.push(Intent::SetCharacterDetails { id, body: next });
        },
    );
    handle_list_field(
        ui,
        theme,
        id,
        "char_anim",
        &tr("codex.character.animation_set"),
        &body.animation_set,
        |next_handles| {
            let mut next = body.clone();
            next.animation_set = next_handles;
            intents.push(Intent::SetCharacterDetails { id, body: next });
        },
    );
}

/// An editable list of `CodexHandle`s.
fn handle_list_field(
    ui: &mut egui::Ui,
    theme: &Theme,
    id: CodexEntryId,
    field: &'static str,
    label: &str,
    handles: &[pixhaus_core::codex::CodexHandle],
    commit: impl FnOnce(Vec<pixhaus_core::codex::CodexHandle>),
) {
    use pixhaus_core::codex::CodexHandle;
    let buf_id = ui.make_persistent_id(("codex-handle-add", field, id.0));
    let mut add_buf = ui.data(|d| d.get_temp::<String>(buf_id)).unwrap_or_default();
    let display = handle_display(handles);
    field_row(ui, theme, label, |ui| {
        if let Some(action) = widgets::editable_list(ui, theme, &display, &mut add_buf, &tr("codex.field.alias.placeholder")) {
            let mut next = handles.to_vec();
            match action {
                widgets::ListAction::Add(text) => {
                    if let Ok(h) = CodexHandle::new(text.trim().to_lowercase()) {
                        next.push(h);
                    }
                    add_buf.clear();
                    commit(next);
                }
                widgets::ListAction::Remove(i) => {
                    if i < next.len() {
                        next.remove(i);
                        commit(next);
                    }
                }
            }
        }
    });
    ui.data_mut(|d| d.insert_temp(buf_id, add_buf));
}

/// The Palette details editor: per-color lock/optional/remove rows, ramps, and the
/// allow-generated toggle.
fn palette_details(
    ui: &mut egui::Ui,
    theme: &Theme,
    intents: &mut pixhaus_ui::state::intent::IntentSink,
    id: CodexEntryId,
    body: &pixhaus_core::codex::PaletteDetails,
) {
    ui.label(
        egui::RichText::new(tr("codex.palette.colors"))
            .size(theme.type_scale.label)
            .color(theme.roles.text_secondary),
    );
    for (i, color) in body.colors.iter().enumerate() {
        if let Some(action) = widgets::palette_color_row(
            ui,
            theme,
            *color,
            &tr(color_role_key(color.role)),
            &tr("codex.palette.optional_short"),
            |role| tr(color_role_key(role)),
        ) {
            let mut next = body.clone();
            match action {
                widgets::ColorRowAction::ToggleLocked => {
                    if let Some(c) = next.colors.get_mut(i) {
                        c.locked = !c.locked;
                    }
                }
                widgets::ColorRowAction::ToggleOptional => {
                    if let Some(c) = next.colors.get_mut(i) {
                        c.optional = !c.optional;
                    }
                }
                widgets::ColorRowAction::SetRgba(rgba) => {
                    if let Some(c) = next.colors.get_mut(i) {
                        c.rgba = rgba;
                    }
                }
                widgets::ColorRowAction::SetRole(role) => {
                    if let Some(c) = next.colors.get_mut(i) {
                        c.role = role;
                    }
                }
                widgets::ColorRowAction::Remove => {
                    if i < next.colors.len() {
                        next.colors.remove(i);
                    }
                }
            }
            intents.push(Intent::SetPaletteDetails { id, body: next });
        }
    }
    // Add a color: a new midtone the artist then recolors and re-roles in its row.
    if ui
        .add(egui::Button::new(
            egui::RichText::new(format!("{} {}", icons::ADD, tr("codex.palette.add_color")))
                .size(theme.type_scale.label)
                .color(theme.accent.base),
        ))
        .clicked()
    {
        let mut next = body.clone();
        next.colors.push(PaletteColor::new([128, 128, 128, 255], ColorRole::Midtone));
        intents.push(Intent::SetPaletteDetails { id, body: next });
    }
    palette_ramps_editor(ui, theme, intents, id, body);
    let mut allow = body.allow_generated_colors;
    if ui.checkbox(&mut allow, tr("codex.palette.allow_generated")).changed() {
        let mut next = body.clone();
        next.allow_generated_colors = allow;
        intents.push(Intent::SetPaletteDetails { id, body: next });
    }
}

/// The palette-ramps editor: a heading, one editor row per ramp, and an add-ramp field.
/// A ramp is named and structured, so it gets full add/remove/rename/edit-indices - this
/// closes the add-only ramp gap. Every edit commits the whole body through
/// `SetPaletteDetails`.
fn palette_ramps_editor(
    ui: &mut egui::Ui,
    theme: &Theme,
    intents: &mut pixhaus_ui::state::intent::IntentSink,
    id: CodexEntryId,
    body: &pixhaus_core::codex::PaletteDetails,
) {
    ui.add_space(theme.spacing.xs);
    ui.label(
        egui::RichText::new(tr("codex.palette.ramps"))
            .size(theme.type_scale.label)
            .color(theme.roles.text_secondary),
    );
    for (i, ramp) in body.ramps.iter().enumerate() {
        palette_ramp_row(ui, theme, intents, id, body, i, ramp);
    }
    // Add a ramp: an inline name field plus an Add button (the folder-rename pattern - the
    // buffer lives in egui temp data). A submit appends an empty ramp the artist then names
    // and fills.
    let ramp_name_id = ui.make_persistent_id(("codex-palette-add-ramp", id.0));
    let mut ramp_buf = ui.data(|d| d.get_temp::<String>(ramp_name_id).unwrap_or_default());
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = theme.spacing.xs;
        let field = ui.add(
            egui::TextEdit::singleline(&mut ramp_buf)
                .hint_text(tr("codex.palette.ramp_name.placeholder"))
                .desired_width(140.0),
        );
        if field.changed() {
            ui.data_mut(|d| d.insert_temp(ramp_name_id, ramp_buf.clone()));
        }
        let submitted = field.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
        let add = ui
            .add(egui::Button::new(
                egui::RichText::new(format!("{} {}", icons::ADD, tr("codex.palette.add_ramp")))
                    .size(theme.type_scale.label)
                    .color(theme.accent.base),
            ))
            .clicked();
        if (submitted || add) && !ramp_buf.trim().is_empty() {
            let mut next = body.clone();
            next.ramps.push(PaletteRamp {
                name: ramp_buf.trim().to_owned(),
                color_indices: Vec::new(),
            });
            intents.push(Intent::SetPaletteDetails { id, body: next });
            ui.data_mut(|d| d.remove_temp::<String>(ramp_name_id));
        }
    });
}

/// One palette-ramp editor row: an inline name rename, the ramp's color indices as
/// removable chips, an add-index field, and a remove-ramp control. Every edit commits the
/// whole `PaletteDetails` body through `SetPaletteDetails`. A ramp carries a NAME and a
/// structured index list, so both must be editable (not just removable) - this closes the
/// add-only ramp gap. An added index is clamped to the palette's color count, so a ramp
/// never points past its colors.
fn palette_ramp_row(
    ui: &mut egui::Ui,
    theme: &Theme,
    intents: &mut pixhaus_ui::state::intent::IntentSink,
    id: CodexEntryId,
    body: &pixhaus_core::codex::PaletteDetails,
    i: usize,
    ramp: &PaletteRamp,
) {
    let rename_id = ui.make_persistent_id(("codex-palette-ramp-rename", id.0, i));
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = theme.spacing.xs;
        if let Some(mut buf) = ui.data(|d| d.get_temp::<String>(rename_id)) {
            // Inline name rename: commit on lost-focus when non-empty and changed.
            let resp = ui.add(
                egui::TextEdit::singleline(&mut buf)
                    .hint_text(tr("codex.palette.ramp_name.placeholder"))
                    .desired_width(140.0),
            );
            if resp.changed() {
                ui.data_mut(|d| d.insert_temp(rename_id, buf.clone()));
            }
            if resp.lost_focus() {
                if !buf.trim().is_empty() && buf.trim() != ramp.name {
                    let mut next = body.clone();
                    if let Some(r) = next.ramps.get_mut(i) {
                        buf.trim().clone_into(&mut r.name);
                    }
                    intents.push(Intent::SetPaletteDetails { id, body: next });
                }
                ui.data_mut(|d| d.remove_temp::<String>(rename_id));
            }
        } else {
            ui.label(egui::RichText::new(&ramp.name).size(theme.type_scale.label).color(theme.roles.text_primary));
            if ui
                .add(
                    egui::Button::new(
                        egui::RichText::new(icons::RENAME.to_string())
                            .size(theme.type_scale.label)
                            .color(theme.roles.text_secondary),
                    )
                    .frame(false),
                )
                .on_hover_text(tr("codex.palette.ramp_name"))
                .clicked()
            {
                ui.data_mut(|d| d.insert_temp(rename_id, ramp.name.clone()));
            }
        }
        // The ramp's color indices as removable chips.
        for (j, index) in ramp.color_indices.iter().enumerate() {
            if ui
                .add(egui::Button::new(
                    egui::RichText::new(format!("{index} {}", icons::CLOSE))
                        .size(theme.type_scale.label)
                        .color(theme.roles.text_secondary),
                ))
                .clicked()
            {
                let mut next = body.clone();
                if let Some(r) = next.ramps.get_mut(i)
                    && j < r.color_indices.len()
                {
                    r.color_indices.remove(j);
                }
                intents.push(Intent::SetPaletteDetails { id, body: next });
            }
        }
        // Add an index pointing at an existing color (clamped to the color count).
        if !body.colors.is_empty()
            && ui
                .add(egui::Button::new(
                    egui::RichText::new(format!("{} {}", icons::ADD, tr("codex.palette.add_index")))
                        .size(theme.type_scale.label)
                        .color(theme.accent.base),
                ))
                .clicked()
        {
            let mut next = body.clone();
            if let Some(r) = next.ramps.get_mut(i) {
                // Point at the last existing color; the artist edits the run from there.
                r.color_indices.push(body.colors.len().saturating_sub(1));
            }
            intents.push(Intent::SetPaletteDetails { id, body: next });
        }
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if ui
                .add(
                    egui::Button::new(
                        egui::RichText::new(icons::TRASH.to_string())
                            .size(theme.type_scale.label)
                            .color(theme.roles.warning),
                    )
                    .frame(false),
                )
                .clicked()
            {
                let mut next = body.clone();
                if i < next.ramps.len() {
                    next.ramps.remove(i);
                }
                intents.push(Intent::SetPaletteDetails { id, body: next });
            }
        });
    });
}

/// The Style details editor.
fn style_details(
    ui: &mut egui::Ui,
    theme: &Theme,
    intents: &mut pixhaus_ui::state::intent::IntentSink,
    id: CodexEntryId,
    body: &pixhaus_core::codex::StyleDetails,
) {
    use pixhaus_core::codex::{AntiAliasingRule, DetailLevel, LineTreatment};
    temp_text_row(ui, theme, id, "style_rules", &tr("codex.style.shading"), &body.rendering_rules, |text| {
        let mut next = body.clone();
        next.rendering_rules = text;
        intents.push(Intent::SetStyleDetails { id, body: next });
    });
    enum_picker(
        ui,
        theme,
        &tr("codex.style.line"),
        body.line_treatment,
        &[LineTreatment::None, LineTreatment::Clean, LineTreatment::Bold, LineTreatment::Selective],
        |chosen| {
            let mut next = body.clone();
            next.line_treatment = chosen;
            intents.push(Intent::SetStyleDetails { id, body: next });
        },
    );
    enum_picker(
        ui,
        theme,
        &tr("codex.style.outline"),
        body.detail_level,
        &[DetailLevel::Minimal, DetailLevel::Low, DetailLevel::Medium, DetailLevel::High],
        |chosen| {
            let mut next = body.clone();
            next.detail_level = chosen;
            intents.push(Intent::SetStyleDetails { id, body: next });
        },
    );
    enum_picker(
        ui,
        theme,
        &tr("codex.style.dithering"),
        body.anti_aliasing,
        &[AntiAliasingRule::None, AntiAliasingRule::Manual, AntiAliasingRule::Allowed],
        |chosen| {
            let mut next = body.clone();
            next.anti_aliasing = chosen;
            intents.push(Intent::SetStyleDetails { id, body: next });
        },
    );
    field_row(ui, theme, &tr("codex.field.negative_fragments"), |ui| {
        let buf_id = ui.make_persistent_id(("codex-style-neg", id.0));
        let mut add_buf = ui.data(|d| d.get_temp::<String>(buf_id)).unwrap_or_default();
        if let Some(action) = widgets::editable_list(ui, theme, &body.negative_rules, &mut add_buf, &tr("codex.field.negative.placeholder")) {
            let mut next = body.clone();
            match action {
                widgets::ListAction::Add(text) => {
                    next.negative_rules.push(text);
                    add_buf.clear();
                }
                widgets::ListAction::Remove(i) => {
                    if i < next.negative_rules.len() {
                        next.negative_rules.remove(i);
                    }
                }
            }
            intents.push(Intent::SetStyleDetails { id, body: next });
        }
        ui.data_mut(|d| d.insert_temp(buf_id, add_buf));
    });
}

/// A row of selectable labels, one per enum variant, with the current one in the accent.
fn enum_picker<T: Copy + PartialEq + std::fmt::Debug>(ui: &mut egui::Ui, theme: &Theme, label: &str, current: T, variants: &[T], commit: impl FnOnce(T)) {
    let mut chosen = None;
    field_row(ui, theme, label, |ui| {
        ui.horizontal_wrapped(|ui| {
            for &v in variants {
                let active = v == current;
                let color = if active { theme.accent.base } else { theme.roles.text_secondary };
                if ui
                    .selectable_label(active, egui::RichText::new(format!("{v:?}")).size(theme.type_scale.label).color(color))
                    .clicked()
                    && !active
                {
                    chosen = Some(v);
                }
            }
        });
    });
    if let Some(v) = chosen {
        commit(v);
    }
}

/// The pose-beat editor: one row per beat exposing both the beat's NAME (label) and its
/// structured description, plus a remove control, then an add-beat field. A pose beat
/// carries a label and a description, so both must be editable (not just add/remove) -
/// this surfaces the description, which the prior `editable_list` left permanently empty.
/// Every edit commits the whole body through the existing `SetAnimationDetails` command.
fn pose_beats_editor(
    ui: &mut egui::Ui,
    theme: &Theme,
    intents: &mut pixhaus_ui::state::intent::IntentSink,
    id: CodexEntryId,
    body: &pixhaus_core::codex::AnimationDetails,
) {
    for (i, beat) in body.pose_beats.iter().enumerate() {
        let label_id = ui.make_persistent_id(("codex-anim-beat-label", id.0, i));
        let desc_id = ui.make_persistent_id(("codex-anim-beat-desc", id.0, i));
        let mut label = ui.data(|d| d.get_temp::<String>(label_id)).unwrap_or_else(|| beat.label.clone());
        let mut desc = ui.data(|d| d.get_temp::<String>(desc_id)).unwrap_or_else(|| beat.description.clone());
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = theme.spacing.xs;
            let lr = ui.add(
                egui::TextEdit::singleline(&mut label)
                    .hint_text(tr("codex.animation.pose_beat_label.placeholder"))
                    .desired_width(110.0),
            );
            let dr = ui.add(
                egui::TextEdit::singleline(&mut desc)
                    .hint_text(tr("codex.animation.pose_beat_description.placeholder"))
                    .desired_width(180.0),
            );
            ui.data_mut(|d| {
                d.insert_temp(label_id, label.clone());
                d.insert_temp(desc_id, desc.clone());
            });
            if (lr.lost_focus() && label != beat.label) || (dr.lost_focus() && desc != beat.description) {
                let mut next = body.clone();
                if let Some(b) = next.pose_beats.get_mut(i) {
                    b.label.clone_from(&label);
                    b.description.clone_from(&desc);
                }
                intents.push(Intent::SetAnimationDetails { id, body: next });
            }
            if ui
                .add(
                    egui::Button::new(
                        egui::RichText::new(icons::TRASH.to_string())
                            .size(theme.type_scale.label)
                            .color(theme.roles.warning),
                    )
                    .frame(false),
                )
                .clicked()
            {
                let mut next = body.clone();
                if i < next.pose_beats.len() {
                    next.pose_beats.remove(i);
                }
                intents.push(Intent::SetAnimationDetails { id, body: next });
            }
        });
    }
    // Add a beat: an inline label field plus an Add button (temp-data buffer).
    let buf_id = ui.make_persistent_id(("codex-anim-beat-add", id.0));
    let mut add_buf = ui.data(|d| d.get_temp::<String>(buf_id)).unwrap_or_default();
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = theme.spacing.xs;
        let field = ui.add(
            egui::TextEdit::singleline(&mut add_buf)
                .hint_text(tr("codex.animation.pose_beat_label.placeholder"))
                .desired_width(110.0),
        );
        if field.changed() {
            ui.data_mut(|d| d.insert_temp(buf_id, add_buf.clone()));
        }
        let submitted = field.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
        let add = ui
            .add(egui::Button::new(
                egui::RichText::new(format!("{} {}", icons::ADD, tr("codex.animation.add_pose_beat")))
                    .size(theme.type_scale.label)
                    .color(theme.accent.base),
            ))
            .clicked();
        if (submitted || add) && !add_buf.trim().is_empty() {
            let mut next = body.clone();
            next.pose_beats.push(pixhaus_core::codex::PoseBeat {
                label: add_buf.trim().to_owned(),
                description: String::new(),
            });
            intents.push(Intent::SetAnimationDetails { id, body: next });
            ui.data_mut(|d| d.remove_temp::<String>(buf_id));
        }
    });
}

/// The Animation details editor.
fn animation_details(
    ui: &mut egui::Ui,
    theme: &Theme,
    intents: &mut pixhaus_ui::state::intent::IntentSink,
    id: CodexEntryId,
    body: &pixhaus_core::codex::AnimationDetails,
) {
    use pixhaus_core::codex::LoopBehavior;
    temp_text_row(ui, theme, id, "anim_purpose", &tr("codex.animation.purpose"), &body.purpose, |text| {
        let mut next = body.clone();
        next.purpose = text;
        intents.push(Intent::SetAnimationDetails { id, body: next });
    });
    enum_picker(
        ui,
        theme,
        &tr("codex.animation.loops"),
        body.loop_behavior,
        &[LoopBehavior::Loop, LoopBehavior::Once, LoopBehavior::PingPong],
        |chosen| {
            let mut next = body.clone();
            next.loop_behavior = chosen;
            intents.push(Intent::SetAnimationDetails { id, body: next });
        },
    );
    temp_text_row(
        ui,
        theme,
        id,
        "anim_frames",
        &tr("codex.animation.frames"),
        &body.recommended_frame_count.to_string(),
        |text| {
            if let Ok(n) = text.trim().parse::<u32>() {
                let mut next = body.clone();
                next.recommended_frame_count = n;
                intents.push(Intent::SetAnimationDetails { id, body: next });
            }
        },
    );
    temp_text_row(ui, theme, id, "anim_fps", &tr("codex.animation.fps"), &body.fps.to_string(), |text| {
        if let Ok(n) = text.trim().parse::<u16>() {
            let mut next = body.clone();
            next.fps = n;
            intents.push(Intent::SetAnimationDetails { id, body: next });
        }
    });
    field_row(ui, theme, &tr("codex.animation.pose_beats"), |ui| {
        pose_beats_editor(ui, theme, intents, id, body);
    });
    handle_list_field(
        ui,
        theme,
        id,
        "anim_compat",
        &tr("codex.animation.compat"),
        &body.character_compatibility,
        |next_handles| {
            let mut next = body.clone();
            next.character_compatibility = next_handles;
            intents.push(Intent::SetAnimationDetails { id, body: next });
        },
    );
}

/// The Generic details editor: a key/value row per field, each removable, plus an add
/// control. Commits the whole body.
fn generic_details(
    ui: &mut egui::Ui,
    theme: &Theme,
    intents: &mut pixhaus_ui::state::intent::IntentSink,
    id: CodexEntryId,
    body: &pixhaus_core::codex::GenericDetails,
) {
    use pixhaus_core::codex::GenericField;
    ui.label(
        egui::RichText::new(tr("codex.generic.notes"))
            .size(theme.type_scale.label)
            .color(theme.roles.text_secondary),
    );
    let mut commit_body: Option<pixhaus_core::codex::GenericDetails> = None;
    for (i, f) in body.fields.iter().enumerate() {
        let key_id = ui.make_persistent_id(("codex-gen-k", id.0, i));
        let val_id = ui.make_persistent_id(("codex-gen-v", id.0, i));
        let mut key = ui.data(|d| d.get_temp::<String>(key_id)).unwrap_or_else(|| f.key.clone());
        let mut val = ui.data(|d| d.get_temp::<String>(val_id)).unwrap_or_else(|| f.value.clone());
        ui.horizontal(|ui| {
            let kr = ui.add(
                egui::TextEdit::singleline(&mut key)
                    .desired_width(100.0)
                    .hint_text(tr("codex.field.fragment_text")),
            );
            let vr = ui.add(egui::TextEdit::singleline(&mut val).desired_width(160.0));
            ui.data_mut(|d| {
                d.insert_temp(key_id, key.clone());
                d.insert_temp(val_id, val.clone());
            });
            if (kr.lost_focus() && key != f.key) || (vr.lost_focus() && val != f.value) {
                let mut next = body.clone();
                if let Some(field) = next.fields.get_mut(i) {
                    field.key.clone_from(&key);
                    field.value.clone_from(&val);
                }
                commit_body = Some(next);
            }
            if ui
                .add(
                    egui::Button::new(
                        egui::RichText::new(icons::CLOSE.to_string())
                            .size(theme.type_scale.label)
                            .color(theme.roles.warning),
                    )
                    .frame(false),
                )
                .clicked()
            {
                let mut next = body.clone();
                if i < next.fields.len() {
                    next.fields.remove(i);
                }
                commit_body = Some(next);
            }
        });
    }
    let new_key_id = ui.make_persistent_id(("codex-gen-addk", id.0));
    let new_val_id = ui.make_persistent_id(("codex-gen-addval", id.0));
    let mut new_key = ui.data(|d| d.get_temp::<String>(new_key_id)).unwrap_or_default();
    let mut new_val = ui.data(|d| d.get_temp::<String>(new_val_id)).unwrap_or_default();
    ui.horizontal(|ui| {
        ui.add(
            egui::TextEdit::singleline(&mut new_key)
                .desired_width(100.0)
                .hint_text(tr("codex.field.fragment_text")),
        );
        ui.add(
            egui::TextEdit::singleline(&mut new_val)
                .desired_width(160.0)
                .hint_text(tr("codex.field.fragment.placeholder")),
        );
        if ui
            .add(egui::Button::new(
                egui::RichText::new(format!("{} {}", icons::ADD, tr("codex.action.add")))
                    .size(theme.type_scale.label)
                    .color(theme.accent.base),
            ))
            .clicked()
            && !new_key.trim().is_empty()
        {
            let mut next = body.clone();
            next.fields.push(GenericField {
                key: new_key.trim().to_owned(),
                value: new_val.clone(),
            });
            new_key.clear();
            new_val.clear();
            commit_body = Some(next);
        }
    });
    ui.data_mut(|d| {
        d.insert_temp(new_key_id, new_key);
        d.insert_temp(new_val_id, new_val);
    });
    if let Some(next) = commit_body {
        intents.push(Intent::SetGenericDetails { id, body: next });
    }
}

/// The add-relationship control: a kind menu and a target-entry menu. Picking a target
/// (with a kind already chosen, held in temp memory) pushes `AddCodexRelationship`.
fn add_relationship_control(ui: &mut egui::Ui, theme: &Theme, intents: &mut pixhaus_ui::state::intent::IntentSink, view: &CodexView, from: CodexEntryId) {
    let kind_id = ui.make_persistent_id(("codex-rel-kind", from.0));
    let mut kind = ui.data(|d| d.get_temp::<RelationKind>(kind_id)).unwrap_or(RelationKind::Uses);
    ui.horizontal(|ui| {
        ui.label(
            egui::RichText::new(format!("{} ", icons::ADD))
                .size(theme.type_scale.label)
                .color(theme.accent.base),
        );
        ui.menu_button(
            egui::RichText::new(tr(relation_key(kind)))
                .size(theme.type_scale.label)
                .color(theme.accent.base),
            |ui| {
                for k in RELATION_KINDS {
                    if ui.button(tr(relation_key(k))).clicked() {
                        kind = k;
                        ui.data_mut(|d| d.insert_temp(kind_id, k));
                        ui.close();
                    }
                }
            },
        );
        ui.menu_button(
            egui::RichText::new(format!("{} {}", icons::REFERENCE, tr("codex.action.add_relationship"))).size(theme.type_scale.label),
            |ui| {
                for summary in view.entries.iter().filter(|e| e.id != from) {
                    if ui.button(format!("@{}", summary.handle)).clicked() {
                        intents.push(Intent::AddCodexRelationship { from, kind, to: summary.id });
                        ui.close();
                    }
                }
            },
        );
    });
}

/// Every relationship kind, for the add-relationship picker.
const RELATION_KINDS: [RelationKind; 11] = [
    RelationKind::Uses,
    RelationKind::BelongsTo,
    RelationKind::AppearsIn,
    RelationKind::CompatibleWith,
    RelationKind::IncompatibleWith,
    RelationKind::InheritsFrom,
    RelationKind::VariantOf,
    RelationKind::Requires,
    RelationKind::Contains,
    RelationKind::Replaces,
    RelationKind::InspiredBy,
];

// ===========================================================================
// Inspector (right dock)
// ===========================================================================

/// The Inspector panel (right dock): entry health (ring + checklist), linked assets,
/// used-in counts, and quick actions. The former anchors/relations/preview moved to the
/// center tabs (spec 5).
pub struct InspectorPanel;

impl Panel for InspectorPanel {
    fn id(&self) -> PanelId {
        INSPECTOR
    }

    fn meta(&self) -> PanelMeta {
        PanelMeta {
            title: MsgKey("panel.codex-inspector.title"),
            icon: icons::INSPECTOR,
            default_region: Region::RightDock,
            default_open: true,
        }
    }

    #[allow(clippy::too_many_lines)]
    fn ui(&self, ui: &mut egui::Ui, scope: &mut PanelScope<'_>) {
        let theme = scope.ctx.theme;
        let view = &scope.ctx.session.codex;
        let Some(detail) = view.detail.as_ref() else {
            ui.label(
                egui::RichText::new(tr("codex.inspector.empty"))
                    .size(theme.type_scale.label)
                    .color(theme.roles.text_secondary),
            );
            return;
        };
        let id = detail.summary.id;

        // --- Entry health: a ring with the tier label, then the checklist. ---
        widgets::section_header(ui, theme, icons::HEALTH, &tr("codex.health.title"));
        let tier_key = if detail.health >= 0.8 {
            "codex.health.excellent"
        } else if detail.health >= 0.4 {
            "codex.health.good"
        } else {
            "codex.health.needs_work"
        };
        widgets::score_ring(ui, theme, icons::HEALTH, detail.health, detail.health_percent, &tr(tier_key));
        ui.add_space(theme.spacing.xs);
        for check in &detail.health_checks {
            widgets::status_check_row(ui, theme, check.state, &tr(&check.label_key), check.detail.as_deref());
        }

        // --- Linked assets (MOCK strip sized by the real handle count). ---
        ui.add_space(theme.spacing.sm);
        widgets::section_header(ui, theme, icons::ASSETS, &tr("codex.inspector.linked_assets"));
        widgets::asset_thumbnail_strip(ui, theme, detail.linked_asset_count, 4);

        // --- Used in (DERIVED counts). ---
        ui.add_space(theme.spacing.sm);
        widgets::section_header(ui, theme, icons::LINK, &tr("codex.inspector.used_in"));
        widgets::used_in_row(ui, theme, icons::RECIPE, &tr("codex.used_in.recipes"), detail.used_in.recipes);
        widgets::used_in_row(ui, theme, icons::TIMELINE, &tr("codex.used_in.animations"), detail.used_in.animations);
        widgets::used_in_row(ui, theme, icons::SPARKLE, &tr("codex.used_in.fx"), detail.used_in.fx);
        widgets::used_in_row(ui, theme, icons::SCENE, &tr("codex.used_in.scenes"), detail.used_in.scenes);

        // --- Quick actions. ---
        ui.add_space(theme.spacing.sm);
        widgets::section_header(ui, theme, icons::SLIDERS, &tr("codex.editor.section.identity"));
        ui.horizontal_wrapped(|ui| {
            // Pin / unpin.
            if view.is_pinned(id) {
                if ui
                    .add(
                        egui::Button::new(
                            egui::RichText::new(format!("{} {}", icons::PIN, tr("codex.action.remove")))
                                .size(theme.type_scale.label)
                                .color(theme.roles.text_secondary),
                        )
                        .frame(true),
                    )
                    .clicked()
                {
                    scope.ctx.intents.push(Intent::UnpinCodexEntry(id));
                }
            } else if ui
                .add(
                    egui::Button::new(
                        egui::RichText::new(format!("{} {}", icons::PIN, tr("codex.action.pin_selected")))
                            .size(theme.type_scale.label)
                            .color(theme.accent.base),
                    )
                    .frame(true),
                )
                .clicked()
            {
                scope.ctx.intents.push(Intent::PinCodexEntry(id));
            }
            // Duplicate.
            if ui
                .add(
                    egui::Button::new(
                        egui::RichText::new(format!("{} {}", icons::DUPLICATE, tr("codex.action.duplicate")))
                            .size(theme.type_scale.label)
                            .color(theme.roles.text_secondary),
                    )
                    .frame(true),
                )
                .clicked()
            {
                scope.ctx.intents.push(Intent::DuplicateCodexEntry(id));
            }
            // Promote to canonical.
            if ui
                .add(
                    egui::Button::new(
                        egui::RichText::new(format!("{} {}", icons::PROMOTE, tr("codex.action.promote")))
                            .size(theme.type_scale.label)
                            .color(theme.roles.text_secondary),
                    )
                    .frame(true),
                )
                .clicked()
            {
                scope.ctx.intents.push(Intent::PromoteCodexEntry(id));
            }
            // Archive.
            if ui
                .add(
                    egui::Button::new(
                        egui::RichText::new(format!("{} {}", icons::ARCHIVE, tr("codex.action.archive")))
                            .size(theme.type_scale.label)
                            .color(theme.roles.text_secondary),
                    )
                    .frame(true),
                )
                .clicked()
            {
                scope.ctx.intents.push(Intent::ArchiveCodexEntry(id));
            }
            // Delete (the destructive action, in the error role).
            if ui
                .add(
                    egui::Button::new(
                        egui::RichText::new(format!("{} {}", icons::TRASH, tr("codex.action.delete_entry")))
                            .size(theme.type_scale.label)
                            .color(theme.roles.error),
                    )
                    .frame(true),
                )
                .clicked()
            {
                scope.ctx.intents.push(Intent::DeleteCodexEntry(id));
            }
        });
    }
}

// ===========================================================================
// Bottom tray: Coverage / Test Generation / History
// ===========================================================================

/// The Coverage panel (bottom tray): the selected entry's coverage checklist, with
/// per-slot frame cards driven by the per-slot mirror, an apply-template control, a
/// clear control, and a Generate button per missing slot (bible 8.7, 15).
pub struct CoveragePanel;

impl Panel for CoveragePanel {
    fn id(&self) -> PanelId {
        COVERAGE
    }

    fn meta(&self) -> PanelMeta {
        PanelMeta {
            title: MsgKey("panel.codex-coverage.title"),
            icon: icons::COVERAGE,
            default_region: Region::BottomTray,
            default_open: true,
        }
    }

    fn ui(&self, ui: &mut egui::Ui, scope: &mut PanelScope<'_>) {
        let theme = scope.ctx.theme;
        let view = &scope.ctx.session.codex;
        let Some(detail) = view.detail.as_ref() else {
            ui.label(
                egui::RichText::new(tr("codex.coverage.empty"))
                    .size(theme.type_scale.label)
                    .color(theme.roles.text_secondary),
            );
            return;
        };
        coverage_body(ui, theme, scope.ctx.intents, view, detail.summary.id);
    }
}

/// Whether a coverage status counts as complete (an asset is approved or finalized).
fn coverage_complete(status: CoverageItemStatus) -> bool {
    matches!(status, CoverageItemStatus::Approved | CoverageItemStatus::ManuallyFinalized)
}

/// The built-in coverage presets offered in the picker, each with the action key for
/// its button label.
const BUILTIN_PRESETS: [(BuiltinCoveragePreset, &str); 3] = [
    (BuiltinCoveragePreset::PlatformerCharacter, "codex.coverage.preset.platformer_character"),
    (BuiltinCoveragePreset::TopDownFourDirection, "codex.coverage.preset.top_down_four_direction"),
    (BuiltinCoveragePreset::UiButtonStates, "codex.coverage.preset.ui_button_states"),
];

/// The shared coverage body, used by both the center Coverage tab and the tray panel.
/// Coverage is customizable per project: a template picker applies built-in presets and
/// existing project templates by id, a slot editor reorders and removes a template's
/// slots, and a per-entry add-custom-slot affordance covers one-off needs. The per-slot
/// checklist reads the entry's own coverage only (no cross-entry bleed) and renders each
/// label through the [`resolve_coverage_label`](widgets::resolve_coverage_label) rule.
// The body composes the picker, the per-slot checklist, the slot editor, and the status
// cycle; its length tracks those sections, so the line-count lint does not apply.
#[allow(clippy::too_many_lines)]
fn coverage_body(ui: &mut egui::Ui, theme: &Theme, intents: &mut pixhaus_ui::state::intent::IntentSink, view: &CodexView, id: pixhaus_core::CodexEntryId) {
    let Some(detail) = view.detail.as_ref().filter(|d| d.summary.id == id) else {
        return;
    };

    // The picker: built-in presets (create-if-absent + apply in one step) and any
    // project template applied by id. Clear stays a status-only reset.
    ui.label(
        egui::RichText::new(tr("codex.action.apply_builtin"))
            .size(theme.type_scale.label)
            .color(theme.roles.text_secondary),
    );
    ui.horizontal_wrapped(|ui| {
        for (preset, label_key) in BUILTIN_PRESETS {
            if ui
                .add(egui::Button::new(
                    egui::RichText::new(format!("{} {}", icons::COVERAGE, tr(label_key)))
                        .size(theme.type_scale.label)
                        .color(theme.accent.base),
                ))
                .clicked()
            {
                intents.push(Intent::ApplyBuiltinCoverageTemplate { id, preset });
            }
        }
    });
    if !view.coverage_templates.is_empty() {
        ui.add_space(theme.spacing.xs);
        ui.label(
            egui::RichText::new(tr("codex.action.apply_template"))
                .size(theme.type_scale.label)
                .color(theme.roles.text_secondary),
        );
        ui.horizontal_wrapped(|ui| {
            for tpl in &view.coverage_templates {
                let attached = detail.applied_templates.contains(&tpl.id);
                let color = if attached { theme.roles.text_disabled } else { theme.accent.base };
                let resp = ui.add_enabled(
                    !attached,
                    egui::Button::new(
                        egui::RichText::new(format!("{} ({})", tpl.name, tpl.slot_count()))
                            .size(theme.type_scale.label)
                            .color(color),
                    ),
                );
                if resp.clicked() {
                    intents.push(Intent::ApplyCoverageTemplate { id, template: tpl.id });
                }
            }
        });
    }

    // The new-template creator: an inline name field (the established folder-rename
    // pattern - the buffer lives in egui temp data keyed to this widget, so the shared
    // body needs no shell draft). Enter or the Add button mints a project template seeded
    // with one default slot, so it reads as a real template the artist then applies and
    // extends. A new template's first slot key is stable (`slot_1`).
    new_template_field(ui, theme, intents);

    // The template-management surface: edit any project template's slots - add, remove,
    // rename, reorder - and rename or delete the template itself, whether or not it is
    // applied to the selected entry. Without this, a freshly created or unapplied
    // template's slots were unreachable (the slot editor below only walks the entry's own
    // applied templates and custom slots).
    manage_templates_section(ui, theme, intents, view);

    ui.add_space(theme.spacing.xs);
    ui.horizontal(|ui| {
        // Add a per-entry custom slot. The key is the first free `custom_<n>` not already
        // present (so the add stays real after any interior remove); the label is a literal
        // the artist renames later.
        if ui
            .add(egui::Button::new(
                egui::RichText::new(format!("{} {}", icons::ADD, tr("codex.action.add_custom_slot")))
                    .size(theme.type_scale.label)
                    .color(theme.accent.base),
            ))
            .clicked()
        {
            let key = next_free_slot_key("custom", detail.coverage_items.iter().map(|item| item.slot.as_str()));
            intents.push(Intent::AddEntryCustomSlot {
                id,
                slot: CoverageSlot::custom(key, tr("codex.coverage.custom_slot_default")),
            });
        }
        if ui
            .add(egui::Button::new(
                egui::RichText::new(format!("{} {}", icons::TRASH, tr("codex.action.clear_coverage")))
                    .size(theme.type_scale.label)
                    .color(theme.roles.warning),
            ))
            .clicked()
        {
            intents.push(Intent::ClearCoverage { id });
        }
    });
    ui.add_space(theme.spacing.sm);

    // The per-slot checklist: the entry's own coverage only. Empty until a template or a
    // custom slot lands, so the empty state guides the artist to the picker above.
    if detail.coverage_items.is_empty() {
        ui.label(
            egui::RichText::new(tr("codex.coverage.empty_hint"))
                .size(theme.type_scale.label)
                .color(theme.roles.text_secondary),
        );
        return;
    }
    let rows: Vec<(String, bool)> = detail
        .coverage_items
        .iter()
        .map(|item| (widgets::resolve_coverage_label(&item.label), coverage_complete(item.status)))
        .collect();
    if let Some(slot_index) = widgets::coverage_checklist(ui, theme, &rows, true, &tr("codex.coverage.generate")) {
        if let Some(item) = detail.coverage_items.get(slot_index) {
            intents.push(Intent::GenerateFromCoverage {
                entry: id,
                slot: item.slot.clone(),
            });
        }
    }

    // The slot editor: rename, remove, or reorder a slot. A template slot's edit targets
    // its template; a custom slot's edit targets the entry. Reorder is template-only (the
    // command operates on a template's slot list), so its carets show on template slots.
    // Per-template add-slot and rename/delete-template controls are grouped under each
    // applied template heading so the artist edits a template in place.
    ui.add_space(theme.spacing.sm);
    ui.label(
        egui::RichText::new(tr("codex.coverage.slots_header"))
            .size(theme.type_scale.label)
            .color(theme.roles.text_secondary),
    );
    let last = detail.coverage_items.len().saturating_sub(1);
    let mut last_template = None;
    for (i, item) in detail.coverage_items.iter().enumerate() {
        // A template-heading row with rename/delete controls precedes that template's
        // first slot, so each applied template is editable in place. Custom slots carry no
        // template and fall under no heading.
        if item.template != last_template {
            if let Some(template) = item.template {
                template_heading_row(ui, theme, intents, view, template);
            }
            last_template = item.template;
        }
        let slot_rename_id = ui.make_persistent_id(("codex-coverage-slot-rename", &item.slot));
        if let Some(mut buf) = ui.data(|d| d.get_temp::<String>(slot_rename_id)) {
            // Inline label rename: commit on lost-focus as a literal (user content), never
            // touching the slot key, so coverage-status cells survive the rename.
            ui.horizontal(|ui| {
                ui.add_space(theme.spacing.md);
                let resp = ui.add(
                    egui::TextEdit::singleline(&mut buf)
                        .hint_text(tr("codex.coverage.slot_label.placeholder"))
                        .desired_width(160.0),
                );
                if resp.changed() {
                    ui.data_mut(|d| d.insert_temp(slot_rename_id, buf.clone()));
                }
                if resp.lost_focus() {
                    if !buf.trim().is_empty() {
                        let label = pixhaus_core::codex::CoverageLabel::Literal(buf.trim().to_owned());
                        // A template slot relabels its template; a custom slot relabels the
                        // entry. Both keep the stable slot key, so the status cell survives.
                        match item.template {
                            Some(template) => intents.push(Intent::RenameCoverageSlotLabel {
                                template,
                                key: item.slot.clone(),
                                label,
                            }),
                            None => intents.push(Intent::RenameEntryCustomSlotLabel {
                                id,
                                key: item.slot.clone(),
                                label,
                            }),
                        }
                    }
                    ui.data_mut(|d| d.remove_temp::<String>(slot_rename_id));
                }
            });
            continue;
        }
        let label_text = widgets::resolve_coverage_label(&item.label);
        // Reorder only applies to a template slot, and only between siblings of the same
        // template; gate the carets to template slots that are not at the list ends.
        let reorderable = item.template.is_some();
        let can_up = reorderable && i > 0;
        let can_down = reorderable && i < last;
        if let Some(action) = widgets::slot_editor_row(ui, theme, &label_text, can_up, can_down) {
            match action {
                widgets::SlotEditAction::Remove => match item.template {
                    Some(template) => intents.push(Intent::RemoveCoverageSlot {
                        template,
                        key: item.slot.clone(),
                    }),
                    None => intents.push(Intent::RemoveEntryCustomSlot { id, key: item.slot.clone() }),
                },
                widgets::SlotEditAction::MoveUp => {
                    if let Some(template) = item.template {
                        intents.push(Intent::ReorderCoverageSlots {
                            template,
                            from: i,
                            to: i.saturating_sub(1),
                        });
                    }
                }
                widgets::SlotEditAction::MoveDown => {
                    if let Some(template) = item.template {
                        intents.push(Intent::ReorderCoverageSlots { template, from: i, to: i + 1 });
                    }
                }
                // Open the inline rename field, seeded with the current resolved label.
                // Both template slots and custom slots relabel through the same field; the
                // lost-focus commit routes to the right command by `item.template`.
                widgets::SlotEditAction::Rename => {
                    ui.data_mut(|d| d.insert_temp(slot_rename_id, label_text.clone()));
                }
            }
        }
    }

    // Add a slot to each applied template: one add field per template, keyed off the
    // template id so the buffers stay disjoint. A new slot's key is auto-derived and
    // stable for the add; its label is a literal the artist edits.
    for template in &detail.applied_templates {
        add_slot_field(ui, theme, intents, *template, detail.coverage_items.iter().map(|item| item.slot.as_str()));
    }

    // Per-slot status cycling: a button per slot steps Missing -> Generated -> Approved
    // -> Missing, pushing SetCoverageStatus from the slot's true current status.
    ui.add_space(theme.spacing.sm);
    ui.horizontal_wrapped(|ui| {
        for item in &detail.coverage_items {
            let next = match item.status {
                CoverageItemStatus::Missing | CoverageItemStatus::Draft => CoverageItemStatus::Generated,
                CoverageItemStatus::Generated | CoverageItemStatus::NeedsReview => CoverageItemStatus::Approved,
                _ => CoverageItemStatus::Missing,
            };
            if ui
                .add(egui::Button::new(egui::RichText::new(widgets::resolve_coverage_label(&item.label)).size(theme.type_scale.label)).frame(true))
                .on_hover_text(tr(coverage_status_key(next)))
                .clicked()
            {
                intents.push(Intent::SetCoverageStatus {
                    id,
                    slot: item.slot.clone(),
                    status: next,
                });
            }
        }
    });
}

/// The new-template creator row: an inline name field plus an Add button. The buffer
/// lives in egui temp data (the folder-rename pattern), so the shared body needs no shell
/// draft. A submit mints a project template named from the field, seeded with one default
/// literal slot (`slot_1`) so it reads as a real, non-empty template the artist applies
/// and then extends.
fn new_template_field(ui: &mut egui::Ui, theme: &Theme, intents: &mut pixhaus_ui::state::intent::IntentSink) {
    let buf_id = ui.make_persistent_id("codex-coverage-new-template");
    let mut buf = ui.data(|d| d.get_temp::<String>(buf_id).unwrap_or_default());
    ui.add_space(theme.spacing.xs);
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = theme.spacing.xs;
        let field = ui.add(
            egui::TextEdit::singleline(&mut buf)
                .hint_text(tr("codex.coverage.template_name.placeholder"))
                .desired_width(160.0),
        );
        if field.changed() {
            ui.data_mut(|d| d.insert_temp(buf_id, buf.clone()));
        }
        let submitted = field.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
        let add = ui
            .add(egui::Button::new(
                egui::RichText::new(format!("{} {}", icons::ADD, tr("codex.action.new_template")))
                    .size(theme.type_scale.label)
                    .color(theme.accent.base),
            ))
            .clicked();
        if (submitted || add) && !buf.trim().is_empty() {
            intents.push(Intent::CreateCoverageTemplate {
                name: buf.trim().to_owned(),
                slots: vec![CoverageSlot::custom("slot_1", tr("codex.coverage.custom_slot_default"))],
            });
            ui.data_mut(|d| d.remove_temp::<String>(buf_id));
        }
    });
}

/// The template-management surface: a collapsing section listing every project coverage
/// template with its full slot list, so a template's slots are editable - add, remove,
/// rename, reorder - and the template itself renamable/deletable, whether or not it is
/// applied to the selected entry. This is the reachability fix for GAP 2: the per-entry
/// slot editor below only walks the entry's applied templates and custom slots, so an
/// unapplied (or freshly created) template's slots had no editor. Drives the same four
/// slot commands as the per-entry editor, addressed by template id.
fn manage_templates_section(ui: &mut egui::Ui, theme: &Theme, intents: &mut pixhaus_ui::state::intent::IntentSink, view: &CodexView) {
    if view.coverage_templates.is_empty() {
        return;
    }
    ui.add_space(theme.spacing.xs);
    egui::CollapsingHeader::new(
        egui::RichText::new(format!("{} {}", icons::COVERAGE, tr("codex.coverage.manage_templates")))
            .size(theme.type_scale.label)
            .color(theme.roles.text_secondary),
    )
    .id_salt("codex-manage-templates")
    .show(ui, |ui| {
        ui.label(
            egui::RichText::new(tr("codex.coverage.manage_templates_hint"))
                .size(theme.type_scale.label)
                .color(theme.roles.text_disabled),
        );
        for tpl in &view.coverage_templates {
            ui.add_space(theme.spacing.xs);
            template_heading_row(ui, theme, intents, view, tpl.id);
            manage_template_slots(ui, theme, intents, tpl);
            // Add a slot to this template, addressed by id; the key is auto-derived and
            // stable for the add, its label the typed literal.
            add_slot_field(ui, theme, intents, tpl.id, tpl.slots.iter().map(|s| s.key.as_str()));
        }
    });
}

/// The slot rows for one template in the management surface: a rename/remove/reorder row
/// per slot, addressed by the template's id. A rename opens an inline label field (egui
/// temp data) and commits a literal, never touching the stable slot key. Reorder carets
/// are hidden at the ends so a slot cannot move out of range.
fn manage_template_slots(
    ui: &mut egui::Ui,
    theme: &Theme,
    intents: &mut pixhaus_ui::state::intent::IntentSink,
    tpl: &pixhaus_ui::state::session::CoverageTemplateSummary,
) {
    let last = tpl.slots.len().saturating_sub(1);
    for (i, slot) in tpl.slots.iter().enumerate() {
        let rename_id = ui.make_persistent_id(("codex-manage-slot-rename", tpl.id.0, &slot.key));
        if let Some(mut buf) = ui.data(|d| d.get_temp::<String>(rename_id)) {
            ui.horizontal(|ui| {
                ui.add_space(theme.spacing.md);
                let resp = ui.add(
                    egui::TextEdit::singleline(&mut buf)
                        .hint_text(tr("codex.coverage.slot_label.placeholder"))
                        .desired_width(160.0),
                );
                if resp.changed() {
                    ui.data_mut(|d| d.insert_temp(rename_id, buf.clone()));
                }
                if resp.lost_focus() {
                    if !buf.trim().is_empty() {
                        intents.push(Intent::RenameCoverageSlotLabel {
                            template: tpl.id,
                            key: slot.key.clone(),
                            label: pixhaus_core::codex::CoverageLabel::Literal(buf.trim().to_owned()),
                        });
                    }
                    ui.data_mut(|d| d.remove_temp::<String>(rename_id));
                }
            });
            continue;
        }
        let label_text = widgets::resolve_coverage_label(&slot.label);
        if let Some(action) = widgets::slot_editor_row(ui, theme, &label_text, i > 0, i < last) {
            match action {
                widgets::SlotEditAction::Remove => intents.push(Intent::RemoveCoverageSlot {
                    template: tpl.id,
                    key: slot.key.clone(),
                }),
                widgets::SlotEditAction::MoveUp => intents.push(Intent::ReorderCoverageSlots {
                    template: tpl.id,
                    from: i,
                    to: i.saturating_sub(1),
                }),
                widgets::SlotEditAction::MoveDown => intents.push(Intent::ReorderCoverageSlots {
                    template: tpl.id,
                    from: i,
                    to: i + 1,
                }),
                widgets::SlotEditAction::Rename => {
                    ui.data_mut(|d| d.insert_temp(rename_id, label_text.clone()));
                }
            }
        }
    }
}

/// A per-applied-template heading: the template's name with trailing rename and delete
/// controls. Rename opens an inline name field (egui temp data); delete detaches the
/// template from every entry (an undoable command) without touching status cells. Drawn
/// above that template's first slot in the editor list.
fn template_heading_row(
    ui: &mut egui::Ui,
    theme: &Theme,
    intents: &mut pixhaus_ui::state::intent::IntentSink,
    view: &CodexView,
    template: pixhaus_core::codex::CoverageTemplateId,
) {
    let name = view
        .coverage_templates
        .iter()
        .find(|t| t.id == template)
        .map_or_else(String::new, |t| t.name.clone());
    let rename_id = ui.make_persistent_id(("codex-coverage-template-rename", template.0));
    if let Some(mut buf) = ui.data(|d| d.get_temp::<String>(rename_id)) {
        ui.horizontal(|ui| {
            let resp = ui.add(
                egui::TextEdit::singleline(&mut buf)
                    .hint_text(tr("codex.coverage.template_name.placeholder"))
                    .desired_width(160.0),
            );
            if resp.changed() {
                ui.data_mut(|d| d.insert_temp(rename_id, buf.clone()));
            }
            if resp.lost_focus() {
                if !buf.trim().is_empty() && buf.trim() != name {
                    intents.push(Intent::RenameCoverageTemplate {
                        template,
                        name: buf.trim().to_owned(),
                    });
                }
                ui.data_mut(|d| d.remove_temp::<String>(rename_id));
            }
        });
        return;
    }
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = theme.spacing.xs;
        ui.label(
            egui::RichText::new(format!("{} {name}", icons::COVERAGE))
                .size(theme.type_scale.label)
                .color(theme.roles.text_secondary)
                .strong(),
        );
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if ui
                .add(
                    egui::Button::new(
                        egui::RichText::new(icons::TRASH.to_string())
                            .size(theme.type_scale.label)
                            .color(theme.roles.warning),
                    )
                    .frame(false),
                )
                .on_hover_text(tr("codex.action.delete_template"))
                .clicked()
            {
                intents.push(Intent::DeleteCoverageTemplate { template });
            }
            if ui
                .add(
                    egui::Button::new(
                        egui::RichText::new(icons::RENAME.to_string())
                            .size(theme.type_scale.label)
                            .color(theme.roles.text_secondary),
                    )
                    .frame(false),
                )
                .on_hover_text(tr("codex.action.rename_template"))
                .clicked()
            {
                ui.data_mut(|d| d.insert_temp(rename_id, name.clone()));
            }
        });
    });
}

/// The localization key for a coverage status, used to label the per-slot status-cycle
/// button's tooltip with the status the click will move to.
fn coverage_status_key(status: CoverageItemStatus) -> &'static str {
    match status {
        CoverageItemStatus::Missing => "codex.coverage.status.missing",
        CoverageItemStatus::Draft => "codex.coverage.status.draft",
        CoverageItemStatus::Generated => "codex.coverage.status.generated",
        CoverageItemStatus::NeedsReview => "codex.coverage.status.needs_review",
        CoverageItemStatus::Approved => "codex.coverage.status.approved",
        CoverageItemStatus::ManuallyFinalized => "codex.coverage.status.manually_finalized",
        CoverageItemStatus::Deprecated => "codex.coverage.status.deprecated",
    }
}

/// Mints a slot key of the form `<prefix>_<n>` that does not already exist among
/// `existing` keys. Counting current rows is not enough: removing an interior slot and
/// adding again can re-derive a key that still exists, and the add then silently fails the
/// command's duplicate-key guard. Scanning from 1 for the first free index keeps the add
/// real after any sequence of removes.
fn next_free_slot_key<'a>(prefix: &str, existing: impl Iterator<Item = &'a str>) -> String {
    let taken: std::collections::HashSet<&str> = existing.collect();
    // The first free index is at most `taken.len() + 1` (pigeonhole), so this range
    // always yields a key not in `taken` — the bound keeps the search finite.
    (1..=taken.len() + 1)
        .map(|n| format!("{prefix}_{n}"))
        .find(|key| !taken.contains(key.as_str()))
        .unwrap_or_else(|| format!("{prefix}_1"))
}

/// A per-template add-slot row: an inline label field plus an Add button. The buffer
/// lives in egui temp data keyed to the template, so each template's field is disjoint. A
/// submit appends a slot whose key is the first free `slot_<n>` not already present (so an
/// add stays real after any interior remove) and whose label is the typed literal.
fn add_slot_field<'a>(
    ui: &mut egui::Ui,
    theme: &Theme,
    intents: &mut pixhaus_ui::state::intent::IntentSink,
    template: pixhaus_core::codex::CoverageTemplateId,
    existing_keys: impl Iterator<Item = &'a str>,
) {
    let key = next_free_slot_key("slot", existing_keys);
    let buf_id = ui.make_persistent_id(("codex-coverage-add-slot", template.0));
    let mut buf = ui.data(|d| d.get_temp::<String>(buf_id).unwrap_or_default());
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = theme.spacing.xs;
        let field = ui.add(
            egui::TextEdit::singleline(&mut buf)
                .hint_text(tr("codex.coverage.add_slot.placeholder"))
                .desired_width(160.0),
        );
        if field.changed() {
            ui.data_mut(|d| d.insert_temp(buf_id, buf.clone()));
        }
        let submitted = field.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
        let add = ui
            .add(egui::Button::new(
                egui::RichText::new(format!("{} {}", icons::ADD, tr("codex.action.add_slot")))
                    .size(theme.type_scale.label)
                    .color(theme.accent.base),
            ))
            .clicked();
        if (submitted || add) && !buf.trim().is_empty() {
            intents.push(Intent::AddCoverageSlot {
                template,
                slot: CoverageSlot::custom(key.clone(), buf.trim().to_owned()),
            });
            ui.data_mut(|d| d.remove_temp::<String>(buf_id));
        }
    });
}

/// The Test Generation panel (bottom tray): a prompt field (the scratch carve-out),
/// the context stack as removable reference chips, a Compile button, and the compiled
/// preview - test entries without leaving the workspace (bible 8.8, 14).
pub struct TestGenerationPanel;

impl Panel for TestGenerationPanel {
    fn id(&self) -> PanelId {
        TEST_GENERATION
    }

    fn meta(&self) -> PanelMeta {
        PanelMeta {
            title: MsgKey("panel.codex-test-generation.title"),
            icon: icons::SPARKLE,
            default_region: Region::BottomTray,
            default_open: true,
        }
    }

    fn ui(&self, ui: &mut egui::Ui, scope: &mut PanelScope<'_>) {
        let theme = scope.ctx.theme;
        let view = &scope.ctx.session.codex;

        // The context stack: pinned references as removable chips (bible 9.2).
        if view.context.is_empty() {
            ui.label(
                egui::RichText::new(tr("codex.test.empty"))
                    .size(theme.type_scale.label)
                    .color(theme.roles.text_secondary),
            );
        } else {
            ui.horizontal_wrapped(|ui| {
                let mut to_remove = None;
                for c in &view.context {
                    if widgets::reference_chip(ui, theme, &format!("@{}", c.handle), c.entry_type, true) {
                        to_remove = Some(c.entry);
                    }
                }
                if let Some(id) = to_remove {
                    scope.ctx.intents.push(Intent::RemoveReferenceFromContext(id));
                }
            });
        }

        // Add the selected entry to the context stack.
        if let Some(selected) = view.selected
            && !view.is_pinned(selected)
            && ui
                .add(
                    egui::Button::new(
                        egui::RichText::new(format!("{} {}", icons::PIN, tr("codex.action.pin_selected")))
                            .size(theme.type_scale.label)
                            .color(theme.accent.base),
                    )
                    .frame(false),
                )
                .clicked()
        {
            scope.ctx.intents.push(Intent::AddReferenceToContext(selected));
        }

        ui.add_space(theme.spacing.sm);

        // The prompt field: the scratch carve-out.
        ui.add(
            egui::TextEdit::multiline(scope.scratch)
                .desired_rows(2)
                .desired_width(f32::INFINITY)
                .hint_text(tr("codex.test.placeholder")),
        );

        ui.horizontal(|ui| {
            if ui
                .button(egui::RichText::new(format!("{} {}", icons::SPARKLE, tr("codex.action.compile"))).color(theme.accent.ai))
                .clicked()
            {
                scope.ctx.intents.push(Intent::CompileCodexPrompt {
                    user_text: scope.scratch.clone(),
                });
            }
        });

        // The compiled preview.
        if let Some(compiled) = view.compiled.as_ref()
            && !compiled.positive.is_empty()
        {
            ui.add_space(theme.spacing.xs);
            widgets::section_header(ui, theme, icons::CODEX, &tr("codex.test.compiled"));
            ui.label(
                egui::RichText::new(&compiled.positive)
                    .size(theme.type_scale.label)
                    .color(theme.roles.text_secondary),
            );
        }
    }
}

/// The History panel (bottom tray): the selected entry's version timeline (or a hint to
/// select an entry). Reads the version-history mirror; no hardcoded log.
pub struct CodexHistoryPanel;

impl Panel for CodexHistoryPanel {
    fn id(&self) -> PanelId {
        HISTORY
    }

    fn meta(&self) -> PanelMeta {
        PanelMeta {
            title: MsgKey("panel.codex-history.title"),
            icon: icons::HISTORY,
            default_region: Region::BottomTray,
            default_open: true,
        }
    }

    fn ui(&self, ui: &mut egui::Ui, scope: &mut PanelScope<'_>) {
        let theme = scope.ctx.theme;
        let view = &scope.ctx.session.codex;
        match view.detail.as_ref() {
            Some(detail) => history_body(ui, theme, detail),
            None => {
                ui.label(
                    egui::RichText::new(tr("codex.inspector.empty"))
                        .size(theme.type_scale.label)
                        .color(theme.roles.text_secondary),
                );
            }
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
}
