//! The center Entry Editor panel: the rich header, the detail tab bar, and the tab
//! bodies (Overview / Visual / Anchors / Prompt / Coverage / Relations / History).
//!
//! The header keeps inline rename and handle editing working against the shell-owned
//! `CodexEditorDraft`; the tabs reorganize the former big edit form (spec 3-4). No panel
//! mutates the document - it reads `CodexEntryDetail` and pushes `Intent`s.

// Explicit imports rather than `use super::*`: the repo denies `clippy::wildcard_imports`
// outside test modules. The set is the panel-trait surface, the view/detail/draft and
// ui-state types the tabs read and edit, the enum types and prompt-fragment type the cards
// build, the intent enum, the design-system theme/glyphs/widgets, the i18n helper, this
// panel's id, the `RELATION_KINDS` table and key mappers from the keys area, and the shared
// `coverage_body`/`details_editor`/`notes_field` bodies from the coverage and details areas.
use super::{
    AnchorKind, AnchorStrength, CodexDetailTab, CodexEditorDraft, CodexEntryDetail, CodexEntryId, CodexView, EDITOR, InclusionPriority, Intent, MsgKey, Panel,
    PanelId, PanelMeta, PanelScope, PromptFragment, RELATION_KINDS, Region, RelationKind, Theme, anchor_kind_key, anchor_strength_key, coverage_body,
    details_editor, entry_type_key, icons, notes_field, priority_key, relation_key, status_key, tr, widgets,
};

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
// header() is one cohesive visual block whose sub-sections (portrait, identity, status/
// handle/type chips, description, tags, action cluster) all thread the same borrow set
// (ui, theme, intents, draft, summary); splitting them into helpers would re-plumb those
// args through each call for no readability gain. The allow is deliberate, like coverage_body.
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
pub(super) fn field_row(ui: &mut egui::Ui, theme: &Theme, label: &str, add: impl FnOnce(&mut egui::Ui)) {
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
                                // Color32 built from artist palette data, not theme chrome - exempt from the
                                // design-system token rule (the same exception palette_color_row documents). This
                                // is a non-interactive hover-only swatch, so it paints locally rather than routing
                                // through a widgets:: helper.
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

// ---------------------------------------------------------------------------
// History tab/body.
// ---------------------------------------------------------------------------

/// The History body: a version timeline from the entry's version-history mirror. Empty
/// history shows a muted empty-state line (the former hardcoded mock log is gone). Shared
/// by the center History tab and the tray History panel.
pub(super) fn history_body(ui: &mut egui::Ui, theme: &Theme, detail: &CodexEntryDetail) {
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
