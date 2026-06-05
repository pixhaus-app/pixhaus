//! The type-specific details editors (Character / Palette / Style / Animation / Generic)
//! and the Notes field, reused under the Overview tab.
//!
//! Each edit commits the whole body as the matching `Set*Details` intent; no editor
//! writes the model directly. Text fields without a shell draft slot bind to egui temp
//! buffers keyed by `(entry_id, field)`.

// Explicit imports rather than `use super::*`: the repo denies `clippy::wildcard_imports`
// outside test modules. The set is the entry-id and detail-mirror types, the palette types
// these editors build, the intent enum, the design-system theme/glyphs/widgets, the i18n
// helper, the `color_role_key` mapper from the keys area, and the shared `field_row` from
// the editor area. The per-type body structs are imported locally inside each editor fn.
use super::{CodexEntryDetail, CodexEntryId, ColorRole, Intent, PaletteColor, PaletteRamp, Theme, color_role_key, field_row, icons, tr, widgets};

/// The type-specific details editor: rich editable fields for Character / Palette /
/// Style / Animation, a key/value list for Generic. Each edit commits the whole body as
/// the matching `Set*Details` intent.
pub(super) fn details_editor(ui: &mut egui::Ui, theme: &Theme, intents: &mut pixhaus_ui::state::intent::IntentSink, detail: &CodexEntryDetail) {
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
pub(super) fn notes_field(ui: &mut egui::Ui, theme: &Theme, intents: &mut pixhaus_ui::state::intent::IntentSink, detail: &CodexEntryDetail) {
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
