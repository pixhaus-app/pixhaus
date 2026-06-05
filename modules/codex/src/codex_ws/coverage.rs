//! The Coverage panel (bottom tray) and the shared coverage body reused by the center
//! Coverage tab.
//!
//! Coverage is customizable per project: a template picker applies built-in presets and
//! project templates by id, a slot editor reorders and removes a template's slots, and a
//! per-entry add-custom-slot affordance covers one-off needs (bible 8.7, 15). Every edit
//! routes through a command via an `Intent`.

// Explicit imports rather than `use super::*`: the repo denies `clippy::wildcard_imports`
// outside test modules. The set is the panel-trait surface, the view type, the coverage
// model types these controls build, the built-in preset enum, the intent enum, the
// design-system theme/glyphs/widgets, the i18n helper, this panel's id, and the
// coverage-status key mapper from the keys area.
use super::{
    BuiltinCoveragePreset, COVERAGE, CodexView, CoverageItemStatus, CoverageSlot, Intent, MsgKey, Panel, PanelId, PanelMeta, PanelScope, Region, Theme,
    coverage_status_key, icons, tr, widgets,
};

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
pub(super) fn coverage_complete(status: CoverageItemStatus) -> bool {
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
pub(super) fn coverage_body(
    ui: &mut egui::Ui,
    theme: &Theme,
    intents: &mut pixhaus_ui::state::intent::IntentSink,
    view: &CodexView,
    id: pixhaus_core::CodexEntryId,
) {
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
        // Scope the add-field buffer to this surface ("applied"): the same template can also
        // appear in the manage-templates surface below, and an unscoped buffer id keyed on the
        // template alone would make the two add fields share one temp buffer.
        add_slot_field(
            ui,
            theme,
            intents,
            "applied",
            *template,
            detail.coverage_items.iter().map(|item| item.slot.as_str()),
        );
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
            // stable for the add, its label the typed literal. The "manage" surface
            // discriminator keeps this add field's buffer disjoint from the applied-template
            // add field above when the same template appears in both surfaces.
            add_slot_field(ui, theme, intents, "manage", tpl.id, tpl.slots.iter().map(|s| s.key.as_str()));
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

/// Mints a slot key of the form `<prefix>_<n>` that does not already exist among
/// `existing` keys. Counting current rows is not enough: removing an interior slot and
/// adding again can re-derive a key that still exists, and the add then silently fails the
/// command's duplicate-key guard. Scanning from 1 for the first free index keeps the add
/// real after any sequence of removes.
pub(super) fn next_free_slot_key<'a>(prefix: &str, existing: impl Iterator<Item = &'a str>) -> String {
    let taken: std::collections::HashSet<&str> = existing.collect();
    // The first free index is at most `taken.len() + 1` (pigeonhole), so this range
    // always yields a key not in `taken` — the bound keeps the search finite.
    (1..=taken.len() + 1)
        .map(|n| format!("{prefix}_{n}"))
        .find(|key| !taken.contains(key.as_str()))
        .unwrap_or_else(|| format!("{prefix}_1"))
}

/// A per-template add-slot row: an inline label field plus an Add button. The buffer
/// lives in egui temp data keyed to the `surface` plus the template, so each template's
/// field is disjoint - and the same template shown in two surfaces ("applied" vs "manage")
/// does not collide on one buffer. A submit appends a slot whose key is the first free
/// `slot_<n>` not already present (so an add stays real after any interior remove) and whose
/// label is the typed literal.
fn add_slot_field<'a>(
    ui: &mut egui::Ui,
    theme: &Theme,
    intents: &mut pixhaus_ui::state::intent::IntentSink,
    // A surface discriminator ("applied" vs "manage") so the same template id rendered in two
    // surfaces does not share one temp buffer for its in-progress slot label.
    surface: &'static str,
    template: pixhaus_core::codex::CoverageTemplateId,
    existing_keys: impl Iterator<Item = &'a str>,
) {
    let key = next_free_slot_key("slot", existing_keys);
    let buf_id = ui.make_persistent_id(("codex-coverage-add-slot", surface, template.0));
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
