//! The Navigator panel (left dock) and its tree rows.
//!
//! A search field, a `+ New entry` type-picker, a Pinned section, the WORLD type-group
//! tree, the folder tree, and the Collections smart filters (spec 2). The panel is a
//! `&self` unit struct that reads the read-only `CodexView` mirror and pushes `Intent`s.

// Explicit imports rather than `use super::*`: the repo denies `clippy::wildcard_imports`
// outside test modules. The set is the panel-trait surface, the view/session and ui-state
// types these rows read, the intent enum, the design-system theme/glyphs/widgets, the i18n
// helper, this panel's id, and the `entry_type_key` mapper from the keys area.
use super::{
    CodexEntryId, CodexFolderNode, CodexView, EntryType, Intent, MsgKey, NAVIGATOR, NavFilter, Panel, PanelId, PanelMeta, PanelScope, Region, Theme,
    entry_type_key, icons, tr, widgets,
};

/// A Navigator type-group family: a label key, its representative glyph, and the entry
/// types it gathers. Groups the long type list into the human families the mockup's
/// WORLD tree shows.
pub(super) struct TypeGroup {
    /// The localized group-label key.
    pub(super) label_key: &'static str,
    /// The glyph the group header reads with.
    pub(super) glyph: char,
    /// The entry types this family gathers.
    pub(super) members: &'static [EntryType],
}

/// The Navigator's WORLD type-group families, in display order (spec 2.1).
pub(super) const TYPE_GROUPS: &[TypeGroup] = &[
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
