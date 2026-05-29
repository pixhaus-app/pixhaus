//! The asset-library browser: a Create-mode view over the project's saved AI
//! assets — reference images, character cards, and style swatches.
//!
//! The cockpit *writes* these (save-as-card, save-as-swatch); this is the read
//! side that lets the artist browse, rename, delete, and reuse them. The data
//! lives on `project.library.ai.asset_library`. Deletes and renames route
//! through [`crate::commands::push_ai_library_edit`] so they land on the undo
//! stack and persist with the project, matching the composition-library idioms
//! in [`crate::library`].
//!
//! `LoRA` is out of scope: `AssetLibrary::loras` is never read or rendered here.
//!
//! Thumbnails decode once and are cached by [`crate::app::ShellApp`] keyed on the
//! asset id, so a scroll never re-decodes a PNG.

use eframe::egui;
use pixhaus_core::project::library::ai::ProjectAi;
use pixhaus_core::project::{AssetId, Rgba, ReferenceRole, TagDefinition, TagId};

use crate::app::ShellApp;
use crate::cockpit::CockpitReference;
use crate::commands::{push_ai_library_edit, push_tag_edit};
use crate::document::DocumentStore;
use crate::editor::EditorState;

/// Side length, in points, of a reference thumbnail in the browser grid.
const THUMB: f32 = 72.0;

impl ShellApp {
    /// The asset-library browser: References, Character cards, and Style swatches,
    /// each a section with thumbnails and reuse / rename / delete actions. No
    /// `LoRA` section. Reuse and apply actions feed the cockpit's staged references.
    pub(crate) fn asset_library_view(&mut self, ui: &mut egui::Ui) {
        ui.add_space(4.0);
        ui.heading(format!("{} Asset library", crate::icons::CARD));
        ui.label(
            egui::RichText::new("Saved references, character cards, and style swatches for this project. Reuse one in the cockpit, rename it, or delete it.")
                .small()
                .weak(),
        );
        ui.add_space(6.0);
        ui.separator();

        egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
            self.asset_references_section(ui);
            ui.add_space(10.0);
            self.asset_cards_section(ui);
            ui.add_space(10.0);
            self.asset_swatches_section(ui);
            ui.add_space(10.0);
            self.tag_manager_section(ui);
        });
    }

    /// The Tags section: list every library tag with an optional color swatch,
    /// an auto-generated indicator, inline rename, and delete. Adding a tag mints
    /// a fresh [`TagId`]; deleting one prunes it from every entity's confirmed and
    /// suggested lists. All edits are one undoable [`crate::commands::LibraryTagEdit`].
    fn tag_manager_section(&mut self, ui: &mut egui::Ui) {
        let tags: Vec<TagDefinition> = self.doc.project.library.tags.clone();
        ui.label(egui::RichText::new(format!("{} Tags ({})", crate::icons::TAG, tags.len())).strong());
        ui.label(
            egui::RichText::new("Library tags label entities. Delete removes a tag from every entity that carries it.")
                .small()
                .weak(),
        );
        ui.add_space(4.0);

        ui.horizontal(|ui| {
            let resp = ui.add(egui::TextEdit::singleline(&mut self.tag_new_name).hint_text("New tag name").desired_width(160.0));
            let submit = resp.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
            if (ui.button(format!("{} Add", crate::icons::ADD)).clicked() || submit) && !self.tag_new_name.trim().is_empty() {
                let name = std::mem::take(&mut self.tag_new_name);
                self.add_library_tag(&name);
            }
        });
        ui.add_space(4.0);

        if tags.is_empty() {
            ui.label(egui::RichText::new("No tags yet.").small().weak());
            return;
        }

        let mut delete: Option<TagId> = None;
        let mut rename: Option<(TagId, String)> = None;
        let mut recolor: Option<(TagId, Option<Rgba>)> = None;
        for tag in &tags {
            egui::Frame::group(ui.style()).show(ui, |ui| {
                ui.horizontal(|ui| {
                    // Color swatch toggling between a set color and `(none)`.
                    let mut col = tag
                        .color
                        .map_or(egui::Color32::TRANSPARENT, |c| egui::Color32::from_rgba_unmultiplied(c.r, c.g, c.b, c.a));
                    if ui.color_edit_button_srgba(&mut col).on_hover_text("Tag color").changed() {
                        let next = if col.a() == 0 { None } else { Some(Rgba::new(col.r(), col.g(), col.b(), col.a())) };
                        recolor = Some((tag.id, next));
                    }

                    let editing = self.tag_rename.as_ref().is_some_and(|(rid, _)| *rid == tag.id.get());
                    if editing {
                        if let Some((_, draft)) = self.tag_rename.as_mut() {
                            let resp = ui.add(egui::TextEdit::singleline(draft).desired_width(140.0));
                            if resp.lost_focus() || ui.button(crate::icons::CHECK).clicked() {
                                rename = Some((tag.id, draft.clone()));
                            }
                        }
                    } else {
                        ui.label(egui::RichText::new(&tag.name).strong());
                        if tag.auto_generated {
                            ui.label(egui::RichText::new(format!("{} auto", crate::icons::SPARKLE)).small().weak())
                                .on_hover_text("Suggested by auto-tagging");
                        }
                        if ui.small_button(crate::icons::RENAME).on_hover_text("Rename").clicked() {
                            self.tag_rename = Some((tag.id.get(), tag.name.clone()));
                        }
                    }
                    if ui.small_button(format!("{} Delete", crate::icons::TRASH)).clicked() {
                        delete = Some(tag.id);
                    }
                });
            });
        }

        if let Some((id, name)) = rename {
            self.rename_library_tag(id, &name);
            self.tag_rename = None;
        }
        if let Some((id, color)) = recolor {
            self.recolor_library_tag(id, color);
        }
        if let Some(id) = delete {
            self.delete_library_tag(id);
        }
    }

    /// The References section: a thumbnail grid, each tile showing its role and
    /// tag chips plus a delete button.
    fn asset_references_section(&mut self, ui: &mut egui::Ui) {
        let refs = &self.doc.project.library.ai.asset_library.references;
        ui.label(egui::RichText::new(format!("{} References ({})", crate::icons::IMAGE, refs.len())).strong());
        if refs.is_empty() {
            ui.label(egui::RichText::new("No saved references yet.").small().weak());
            return;
        }
        // Read the tiles we need before borrowing the cache mutably for textures.
        let tiles: Vec<(AssetId, ReferenceRole, Vec<String>, Vec<u8>)> = refs
            .iter()
            .map(|r| (r.id, r.default_role, r.tags.clone(), r.image.bytes.clone()))
            .collect();

        let mut delete: Option<AssetId> = None;
        ui.add_space(4.0);
        ui.horizontal_wrapped(|ui| {
            for (id, role, tags, bytes) in &tiles {
                let tex = self.asset_texture(ui.ctx(), *id, bytes);
                egui::Frame::group(ui.style()).show(ui, |ui| {
                    ui.vertical(|ui| {
                        ui.set_width(THUMB);
                        if let Some(tex) = tex {
                            ui.add(egui::Image::new(&tex).fit_to_exact_size(egui::vec2(THUMB, THUMB)));
                        } else {
                            ui.add_sized([THUMB, THUMB], egui::Label::new(egui::RichText::new("?").weak()));
                        }
                        ui.label(egui::RichText::new(role_label(*role)).small());
                        for tag in tags {
                            ui.label(egui::RichText::new(format!("# {tag}")).small().weak());
                        }
                        if ui.small_button(format!("{} Delete", crate::icons::TRASH)).clicked() {
                            delete = Some(*id);
                        }
                    });
                });
            }
        });

        if let Some(id) = delete {
            self.delete_reference(id);
        }
    }

    /// The Character cards section: each card shows its name (renamable), the
    /// thumbnails of its member references, its style notes, and reuse / delete.
    fn asset_cards_section(&mut self, ui: &mut egui::Ui) {
        let cards: Vec<(AssetId, String, Vec<AssetId>, String)> = self
            .doc
            .project
            .library
            .ai
            .asset_library
            .character_cards
            .iter()
            .map(|c| (c.id, c.name.clone(), c.references.clone(), c.style_notes.clone()))
            .collect();
        ui.label(egui::RichText::new(format!("{} Character cards ({})", crate::icons::CARD, cards.len())).strong());
        if cards.is_empty() {
            ui.label(egui::RichText::new("No saved character cards yet.").small().weak());
            return;
        }

        let mut delete: Option<AssetId> = None;
        let mut reuse: Option<AssetId> = None;
        let mut rename: Option<(AssetId, String)> = None;
        for (id, name, member_ids, style_notes) in &cards {
            egui::Frame::group(ui.style()).show(ui, |ui| {
                let editing = self.asset_rename.as_ref().is_some_and(|(rid, _)| *rid == id.get());
                ui.horizontal(|ui| {
                    if editing {
                        if let Some((_, draft)) = self.asset_rename.as_mut() {
                            let resp = ui.add(egui::TextEdit::singleline(draft).desired_width(160.0));
                            if resp.lost_focus() || ui.button(crate::icons::CHECK).clicked() {
                                rename = Some((*id, draft.clone()));
                            }
                        }
                    } else {
                        ui.label(egui::RichText::new(name).strong());
                        if ui.small_button(crate::icons::RENAME).on_hover_text("Rename").clicked() {
                            self.asset_rename = Some((id.get(), name.clone()));
                        }
                    }
                });
                if !style_notes.is_empty() {
                    ui.label(egui::RichText::new(style_notes).small().weak());
                }
                self.asset_member_thumbs(ui, member_ids);
                ui.horizontal(|ui| {
                    if ui
                        .button(format!("{} Use as reference", crate::icons::SPARKLE))
                        .on_hover_text("Stage the card's first reference in the cockpit")
                        .clicked()
                    {
                        reuse = Some(*id);
                    }
                    if ui.button(format!("{} Delete", crate::icons::TRASH)).clicked() {
                        delete = Some(*id);
                    }
                });
            });
        }

        if let Some((id, name)) = rename {
            self.rename_card(id, &name);
            self.asset_rename = None;
        }
        if let Some(id) = reuse {
            self.use_card_in_cockpit(id);
        }
        if let Some(id) = delete {
            self.delete_card(id);
        }
    }

    /// The Style swatches section: each swatch shows its name (renamable), its
    /// member-reference thumbnails, its style notes, and apply / delete.
    fn asset_swatches_section(&mut self, ui: &mut egui::Ui) {
        let swatches: Vec<(AssetId, String, Vec<AssetId>, String)> = self
            .doc
            .project
            .library
            .ai
            .asset_library
            .style_swatches
            .iter()
            .map(|s| (s.id, s.name.clone(), s.references.clone(), s.style_notes.clone()))
            .collect();
        ui.label(egui::RichText::new(format!("{} Style swatches ({})", crate::icons::PALETTE, swatches.len())).strong());
        if swatches.is_empty() {
            ui.label(egui::RichText::new("No saved style swatches yet.").small().weak());
            return;
        }

        let mut delete: Option<AssetId> = None;
        let mut apply: Option<AssetId> = None;
        let mut rename: Option<(AssetId, String)> = None;
        for (id, name, member_ids, style_notes) in &swatches {
            egui::Frame::group(ui.style()).show(ui, |ui| {
                let editing = self.asset_rename.as_ref().is_some_and(|(rid, _)| *rid == id.get());
                ui.horizontal(|ui| {
                    if editing {
                        if let Some((_, draft)) = self.asset_rename.as_mut() {
                            let resp = ui.add(egui::TextEdit::singleline(draft).desired_width(160.0));
                            if resp.lost_focus() || ui.button(crate::icons::CHECK).clicked() {
                                rename = Some((*id, draft.clone()));
                            }
                        }
                    } else {
                        ui.label(egui::RichText::new(name).strong());
                        if ui.small_button(crate::icons::RENAME).on_hover_text("Rename").clicked() {
                            self.asset_rename = Some((id.get(), name.clone()));
                        }
                    }
                });
                if !style_notes.is_empty() {
                    ui.label(egui::RichText::new(style_notes).small().weak());
                }
                self.asset_member_thumbs(ui, member_ids);
                ui.horizontal(|ui| {
                    if ui
                        .button(format!("{} Apply to cockpit", crate::icons::SPARKLE))
                        .on_hover_text("Stage the swatch's first reference as a style reference")
                        .clicked()
                    {
                        apply = Some(*id);
                    }
                    if ui.button(format!("{} Delete", crate::icons::TRASH)).clicked() {
                        delete = Some(*id);
                    }
                });
            });
        }

        if let Some((id, name)) = rename {
            self.rename_swatch(id, &name);
            self.asset_rename = None;
        }
        if let Some(id) = apply {
            self.apply_swatch_to_cockpit(id);
        }
        if let Some(id) = delete {
            self.delete_swatch(id);
        }
    }

    /// Renders a row of small thumbnails for the references a card or swatch
    /// names, resolving each [`AssetId`] against the project's reference assets.
    fn asset_member_thumbs(&mut self, ui: &mut egui::Ui, member_ids: &[AssetId]) {
        let resolved: Vec<(AssetId, Vec<u8>)> = member_ids
            .iter()
            .filter_map(|aid| {
                self.doc
                    .project
                    .library
                    .ai
                    .asset_library
                    .references
                    .iter()
                    .find(|r| r.id == *aid)
                    .map(|r| (*aid, r.image.bytes.clone()))
            })
            .collect();
        if resolved.is_empty() {
            return;
        }
        ui.horizontal_wrapped(|ui| {
            for (aid, bytes) in &resolved {
                if let Some(tex) = self.asset_texture(ui.ctx(), *aid, bytes) {
                    ui.add(egui::Image::new(&tex).fit_to_exact_size(egui::vec2(THUMB * 0.6, THUMB * 0.6)));
                }
            }
        });
    }

    /// Returns the cached thumbnail texture for asset `id`, decoding `png` once
    /// on the first call and caching it keyed by the asset id. Never decodes on
    /// a later frame for the same id.
    fn asset_texture(&mut self, ctx: &egui::Context, id: AssetId, png: &[u8]) -> Option<egui::TextureHandle> {
        if let Some(tex) = self.asset_tex_cache.get(&id.get()) {
            return Some(tex.clone());
        }
        let rgba = image::load_from_memory(png).ok()?.to_rgba8();
        let size = [rgba.width() as usize, rgba.height() as usize];
        let color = egui::ColorImage::from_rgba_unmultiplied(size, rgba.as_raw());
        let tex = ctx.load_texture(format!("asset-{}", id.get()), color, egui::TextureOptions::NEAREST);
        self.asset_tex_cache.insert(id.get(), tex.clone());
        Some(tex)
    }

    /// Removes the reference asset `id` from the library as one undoable edit.
    /// Drops its cached thumbnail so the slot does not leak GPU memory.
    pub(crate) fn delete_reference(&mut self, id: AssetId) {
        delete_reference(&mut self.editor, &mut self.doc, id);
        self.asset_tex_cache.remove(&id.get());
    }

    /// Removes the character card `id` from the library as one undoable edit.
    pub(crate) fn delete_card(&mut self, id: AssetId) {
        delete_card(&mut self.editor, &mut self.doc, id);
    }

    /// Renames the character card `id` as one undoable edit.
    pub(crate) fn rename_card(&mut self, id: AssetId, name: &str) {
        rename_card(&mut self.editor, &mut self.doc, id, name);
    }

    /// Removes the style swatch `id` from the library as one undoable edit.
    pub(crate) fn delete_swatch(&mut self, id: AssetId) {
        delete_swatch(&mut self.editor, &mut self.doc, id);
    }

    /// Renames the style swatch `id` as one undoable edit.
    pub(crate) fn rename_swatch(&mut self, id: AssetId, name: &str) {
        rename_swatch(&mut self.editor, &mut self.doc, id, name);
    }

    /// Stages the card's first reference image in the cockpit as a Subject
    /// reference, so the next generation conditions on it. A no-op for a card
    /// with no resolvable references.
    pub(crate) fn use_card_in_cockpit(&mut self, id: AssetId) {
        let ai = &self.doc.project.library.ai;
        let Some((label, bytes)) = card_first_reference(ai, id) else {
            return;
        };
        self.ck_references.push(CockpitReference {
            label,
            png: bytes,
            role: ReferenceRole::Subject,
            weight: 1.0,
            texture: None,
        });
        self.ck_dirty = true;
    }

    /// Stages the swatch's first reference image in the cockpit as a Style
    /// reference, so the next generation folds its look in. A no-op for a swatch
    /// with no resolvable references.
    pub(crate) fn apply_swatch_to_cockpit(&mut self, id: AssetId) {
        let ai = &self.doc.project.library.ai;
        let Some((label, bytes)) = swatch_first_reference(ai, id) else {
            return;
        };
        self.ck_references.push(CockpitReference {
            label,
            png: bytes,
            role: ReferenceRole::Style,
            weight: 1.0,
            texture: None,
        });
        self.ck_dirty = true;
    }

    /// Creates a tag definition named `name` (trimmed) as one undoable edit,
    /// minting a fresh [`TagId`]. A blank name is ignored.
    pub(crate) fn add_library_tag(&mut self, name: &str) {
        let name = name.trim().to_owned();
        if name.is_empty() {
            return;
        }
        let id = TagId::new(self.doc.alloc_id());
        add_tag(&mut self.editor, &mut self.doc, id, &name);
    }

    /// Renames tag `id` as one undoable edit. A blank name is ignored.
    pub(crate) fn rename_library_tag(&mut self, id: TagId, name: &str) {
        rename_tag(&mut self.editor, &mut self.doc, id, name);
    }

    /// Sets (or clears) the accent color of tag `id` as one undoable edit.
    pub(crate) fn recolor_library_tag(&mut self, id: TagId, color: Option<Rgba>) {
        recolor_tag(&mut self.editor, &mut self.doc, id, color);
    }

    /// Deletes tag `id` as one undoable edit, pruning it from every entity's
    /// confirmed and suggested tag lists.
    pub(crate) fn delete_library_tag(&mut self, id: TagId) {
        delete_tag(&mut self.editor, &mut self.doc, id);
    }
}

/// Creates a tag definition with id `id` and the (already-trimmed, non-empty)
/// `name` as one undoable [`crate::commands::LibraryTagEdit`].
fn add_tag(editor: &mut EditorState, doc: &mut DocumentStore, id: TagId, name: &str) {
    let name = name.to_owned();
    push_tag_edit(editor, doc, "Add tag", |doc| {
        doc.project.library.tags.push(TagDefinition {
            id,
            name,
            color: None,
            auto_generated: false,
        });
    });
}

/// Renames tag `id`. A blank name is ignored so a tag never loses its label.
fn rename_tag(editor: &mut EditorState, doc: &mut DocumentStore, id: TagId, name: &str) {
    let name = name.trim().to_owned();
    if name.is_empty() {
        return;
    }
    push_tag_edit(editor, doc, "Rename tag", |doc| {
        if let Some(tag) = doc.project.library.tags.iter_mut().find(|t| t.id == id) {
            tag.name = name;
        }
    });
}

/// Sets or clears the accent color of tag `id`.
fn recolor_tag(editor: &mut EditorState, doc: &mut DocumentStore, id: TagId, color: Option<Rgba>) {
    push_tag_edit(editor, doc, "Recolor tag", |doc| {
        if let Some(tag) = doc.project.library.tags.iter_mut().find(|t| t.id == id) {
            tag.color = color;
        }
    });
}

/// Deletes tag `id` and prunes it from every entity's confirmed and suggested
/// tag lists, all as one undoable edit.
fn delete_tag(editor: &mut EditorState, doc: &mut DocumentStore, id: TagId) {
    push_tag_edit(editor, doc, "Delete tag", |doc| {
        doc.project.library.tags.retain(|t| t.id != id);
        for entity in &mut doc.project.library.entities {
            entity.tags.retain(|t| *t != id);
            entity.ai.suggested_tags.retain(|t| *t != id);
        }
    });
}

/// Removes the reference asset `id` from the library as one undoable edit.
fn delete_reference(editor: &mut EditorState, doc: &mut DocumentStore, id: AssetId) {
    push_ai_library_edit(editor, doc, "Delete reference", |ai| {
        ai.asset_library.references.retain(|r| r.id != id);
    });
}

/// Removes the character card `id` from the library as one undoable edit. The
/// card's member references are left in place; only the card is dropped.
fn delete_card(editor: &mut EditorState, doc: &mut DocumentStore, id: AssetId) {
    push_ai_library_edit(editor, doc, "Delete character card", |ai| {
        ai.asset_library.character_cards.retain(|c| c.id != id);
    });
}

/// Renames the character card `id` as one undoable edit. A blank name is ignored
/// so the card never loses its label to an empty string.
fn rename_card(editor: &mut EditorState, doc: &mut DocumentStore, id: AssetId, name: &str) {
    let name = name.trim().to_owned();
    if name.is_empty() {
        return;
    }
    push_ai_library_edit(editor, doc, "Rename character card", |ai| {
        if let Some(card) = ai.asset_library.character_cards.iter_mut().find(|c| c.id == id) {
            card.name = name;
        }
    });
}

/// Removes the style swatch `id` from the library as one undoable edit.
fn delete_swatch(editor: &mut EditorState, doc: &mut DocumentStore, id: AssetId) {
    push_ai_library_edit(editor, doc, "Delete style swatch", |ai| {
        ai.asset_library.style_swatches.retain(|s| s.id != id);
    });
}

/// Renames the style swatch `id` as one undoable edit. A blank name is ignored.
fn rename_swatch(editor: &mut EditorState, doc: &mut DocumentStore, id: AssetId, name: &str) {
    let name = name.trim().to_owned();
    if name.is_empty() {
        return;
    }
    push_ai_library_edit(editor, doc, "Rename style swatch", |ai| {
        if let Some(swatch) = ai.asset_library.style_swatches.iter_mut().find(|s| s.id == id) {
            swatch.name = name;
        }
    });
}

/// Resolves the card `id`'s first reference to its label and PNG bytes, or
/// `None` when the card or its first reference is missing.
fn card_first_reference(ai: &ProjectAi, id: AssetId) -> Option<(String, Vec<u8>)> {
    let card = ai.asset_library.character_cards.iter().find(|c| c.id == id)?;
    let first = card.references.first()?;
    let asset = ai.asset_library.references.iter().find(|r| r.id == *first)?;
    Some((card.name.clone(), asset.image.bytes.clone()))
}

/// Resolves the swatch `id`'s first reference to its label and PNG bytes, or
/// `None` when the swatch or its first reference is missing.
fn swatch_first_reference(ai: &ProjectAi, id: AssetId) -> Option<(String, Vec<u8>)> {
    let swatch = ai.asset_library.style_swatches.iter().find(|s| s.id == id)?;
    let first = swatch.references.first()?;
    let asset = ai.asset_library.references.iter().find(|r| r.id == *first)?;
    Some((swatch.name.clone(), asset.image.bytes.clone()))
}

/// Human label for a [`ReferenceRole`], mirroring the cockpit's role labels.
fn role_label(role: ReferenceRole) -> &'static str {
    match role {
        ReferenceRole::Subject => "Subject",
        ReferenceRole::Style => "Style",
        ReferenceRole::Pose => "Pose",
        ReferenceRole::Outfit => "Outfit",
        ReferenceRole::Context => "Context",
        ReferenceRole::Generic => "Generic",
    }
}

#[cfg(test)]
mod tests {
    use pixhaus_core::project::library::ai::ProjectAi;
    use pixhaus_core::project::{AssetId, CharacterCard, EntityId, ReferenceAsset, ReferenceImage, ReferenceRole, Rgba, StyleSwatch, TagId};

    use super::{add_tag, card_first_reference, delete_card, delete_reference, delete_swatch, delete_tag, recolor_tag, rename_card, rename_swatch, rename_tag, swatch_first_reference};
    use crate::commands::push_ai_library_edit;
    use crate::document::DocumentStore;
    use crate::editor::EditorState;

    /// Stand-in PNG bytes; the model layer never decodes them (texture decode is
    /// an incidental UI concern, covered by the live app, not these tests).
    fn tiny_png() -> Vec<u8> {
        vec![0x89, b'P', b'N', b'G']
    }

    fn ai(doc: &DocumentStore) -> &ProjectAi {
        &doc.project.library.ai
    }

    fn card_count(doc: &DocumentStore) -> usize {
        doc.project.library.ai.asset_library.character_cards.len()
    }

    fn swatch_count(doc: &DocumentStore) -> usize {
        doc.project.library.ai.asset_library.style_swatches.len()
    }

    fn ref_count(doc: &DocumentStore) -> usize {
        doc.project.library.ai.asset_library.references.len()
    }

    /// Saves a card with one reference through the undoable edit path, the same
    /// way the cockpit's save-as-card writes it. Returns `(card_id, asset_id)`.
    fn save_card(editor: &mut EditorState, doc: &mut DocumentStore) -> (AssetId, AssetId) {
        let asset_id = AssetId::new(doc.alloc_id());
        let card_id = AssetId::new(doc.alloc_id());
        push_ai_library_edit(editor, doc, "Save character card", |ai| {
            ai.asset_library.references.push(ReferenceAsset {
                id: asset_id,
                image: ReferenceImage {
                    bytes: tiny_png(),
                    mime: "image/png".to_owned(),
                },
                default_role: ReferenceRole::Subject,
                tags: vec!["hero".to_owned()],
                source_variant_id: None,
                created_at: 0,
            });
            ai.asset_library.character_cards.push(CharacterCard {
                id: card_id,
                name: "Hero".to_owned(),
                references: vec![asset_id],
                style_notes: "16-bit".to_owned(),
                associated_lora: None,
                created_at: 0,
            });
        });
        (card_id, asset_id)
    }

    fn save_swatch(editor: &mut EditorState, doc: &mut DocumentStore) -> (AssetId, AssetId) {
        let asset_id = AssetId::new(doc.alloc_id());
        let swatch_id = AssetId::new(doc.alloc_id());
        push_ai_library_edit(editor, doc, "Save style swatch", |ai| {
            ai.asset_library.references.push(ReferenceAsset {
                id: asset_id,
                image: ReferenceImage {
                    bytes: tiny_png(),
                    mime: "image/png".to_owned(),
                },
                default_role: ReferenceRole::Style,
                tags: Vec::new(),
                source_variant_id: None,
                created_at: 0,
            });
            ai.asset_library.style_swatches.push(StyleSwatch {
                id: swatch_id,
                name: "SNES".to_owned(),
                references: vec![asset_id],
                style_notes: "warm palette".to_owned(),
                associated_lora: None,
                created_at: 0,
            });
        });
        (swatch_id, asset_id)
    }

    #[test]
    fn saved_card_is_visible_and_delete_removes_it_under_undo() {
        let mut doc = DocumentStore::new();
        let mut editor = EditorState::default();
        assert_eq!(card_count(&doc), 0, "no cards to start");

        let (card_id, _asset_id) = save_card(&mut editor, &mut doc);
        assert_eq!(card_count(&doc), 1, "the saved card appears in the browser model");
        assert_eq!(
            doc.project.library.ai.asset_library.character_cards[0].name, "Hero",
            "the saved card carries its name"
        );

        delete_card(&mut editor, &mut doc, card_id);
        assert_eq!(card_count(&doc), 0, "delete removes the card");

        editor.history.undo(&mut doc).expect("undo delete");
        assert_eq!(card_count(&doc), 1, "undo restores the deleted card");
    }

    #[test]
    fn rename_card_persists_and_undoes() {
        let mut doc = DocumentStore::new();
        let mut editor = EditorState::default();
        let (card_id, _asset_id) = save_card(&mut editor, &mut doc);

        rename_card(&mut editor, &mut doc, card_id, "  Renamed  ");
        assert_eq!(
            doc.project.library.ai.asset_library.character_cards[0].name, "Renamed",
            "rename trims and applies"
        );

        // A blank rename is ignored, leaving the prior name and undo stack intact.
        rename_card(&mut editor, &mut doc, card_id, "   ");
        assert_eq!(
            doc.project.library.ai.asset_library.character_cards[0].name, "Renamed",
            "a blank rename is a no-op"
        );

        editor.history.undo(&mut doc).expect("undo rename");
        assert_eq!(
            doc.project.library.ai.asset_library.character_cards[0].name, "Hero",
            "undo restores the original name"
        );
    }

    #[test]
    fn card_first_reference_resolves_label_and_bytes() {
        let mut doc = DocumentStore::new();
        let mut editor = EditorState::default();
        let (card_id, _asset_id) = save_card(&mut editor, &mut doc);

        let resolved = card_first_reference(ai(&doc), card_id);
        assert_eq!(resolved, Some(("Hero".to_owned(), tiny_png())), "the card's first reference resolves");

        // An unknown card id resolves to nothing.
        assert_eq!(card_first_reference(ai(&doc), AssetId::new(9999)), None);
    }

    #[test]
    fn save_swatch_is_visible_and_delete_removes_it_under_undo() {
        let mut doc = DocumentStore::new();
        let mut editor = EditorState::default();
        let (swatch_id, _asset_id) = save_swatch(&mut editor, &mut doc);
        assert_eq!(swatch_count(&doc), 1, "the saved swatch appears in the browser model");

        delete_swatch(&mut editor, &mut doc, swatch_id);
        assert_eq!(swatch_count(&doc), 0, "delete removes the swatch");

        editor.history.undo(&mut doc).expect("undo delete");
        assert_eq!(swatch_count(&doc), 1, "undo restores the deleted swatch");
    }

    #[test]
    fn rename_swatch_persists_and_undoes() {
        let mut doc = DocumentStore::new();
        let mut editor = EditorState::default();
        let (swatch_id, _asset_id) = save_swatch(&mut editor, &mut doc);

        rename_swatch(&mut editor, &mut doc, swatch_id, "Game Boy");
        assert_eq!(doc.project.library.ai.asset_library.style_swatches[0].name, "Game Boy", "rename applies");

        editor.history.undo(&mut doc).expect("undo rename");
        assert_eq!(doc.project.library.ai.asset_library.style_swatches[0].name, "SNES", "undo restores the name");
    }

    #[test]
    fn swatch_first_reference_resolves_label_and_bytes() {
        let mut doc = DocumentStore::new();
        let mut editor = EditorState::default();
        let (swatch_id, _asset_id) = save_swatch(&mut editor, &mut doc);

        let resolved = swatch_first_reference(ai(&doc), swatch_id);
        assert_eq!(resolved, Some(("SNES".to_owned(), tiny_png())), "the swatch's first reference resolves");
    }

    #[test]
    fn delete_reference_removes_it_under_undo() {
        let mut doc = DocumentStore::new();
        let mut editor = EditorState::default();
        let (_card_id, asset_id) = save_card(&mut editor, &mut doc);
        assert_eq!(ref_count(&doc), 1, "one reference saved");

        delete_reference(&mut editor, &mut doc, asset_id);
        assert_eq!(ref_count(&doc), 0, "delete removes the reference");

        editor.history.undo(&mut doc).expect("undo delete");
        assert_eq!(ref_count(&doc), 1, "undo restores the reference");
    }

    // ── tag-definition CRUD ─────────────────────────────────────────────────

    fn tags(doc: &DocumentStore) -> &[pixhaus_core::project::TagDefinition] {
        &doc.project.library.tags
    }

    /// Attaches `tag` to an entity's confirmed and suggested lists so a later
    /// delete can be observed to prune both.
    fn attach_tag(doc: &mut DocumentStore, entity_id: EntityId, tag: TagId) {
        let entity = doc.project.library.entities.iter_mut().find(|e| e.id == entity_id).expect("entity exists");
        entity.tags.push(tag);
        entity.ai.suggested_tags.push(tag);
    }

    #[test]
    fn add_rename_recolor_tag_round_trip_under_undo() {
        let mut doc = DocumentStore::new();
        let mut editor = EditorState::default();
        assert!(tags(&doc).is_empty(), "no tags to start");

        let tag_id = TagId::new(doc.alloc_id());
        add_tag(&mut editor, &mut doc, tag_id, "hero");
        assert_eq!(tags(&doc).len(), 1, "add creates one tag");
        assert_eq!(tags(&doc)[0].name, "hero");
        assert_eq!(tags(&doc)[0].color, None, "a new tag has no color");

        rename_tag(&mut editor, &mut doc, tag_id, "  Protagonist  ");
        assert_eq!(tags(&doc)[0].name, "Protagonist", "rename trims and applies");

        // A blank rename is a no-op and leaves the undo stack untouched.
        rename_tag(&mut editor, &mut doc, tag_id, "   ");
        assert_eq!(tags(&doc)[0].name, "Protagonist", "a blank rename is ignored");

        recolor_tag(&mut editor, &mut doc, tag_id, Some(Rgba::opaque(10, 20, 30)));
        assert_eq!(tags(&doc)[0].color, Some(Rgba::opaque(10, 20, 30)), "recolor applies");

        editor.history.undo(&mut doc).expect("undo recolor");
        assert_eq!(tags(&doc)[0].color, None, "undo restores no color");
        editor.history.undo(&mut doc).expect("undo rename");
        assert_eq!(tags(&doc)[0].name, "hero", "undo restores the original name");
        editor.history.undo(&mut doc).expect("undo add");
        assert!(tags(&doc).is_empty(), "undo removes the added tag");
    }

    #[test]
    fn delete_tag_prunes_entities_and_undoes() {
        let mut doc = DocumentStore::new();
        let mut editor = EditorState::default();
        let sprite = doc.create_sprite("Hero", pixhaus_core::project::Size::new(16, 16));
        let entity_id = sprite.entity_id;

        let tag_id = TagId::new(doc.alloc_id());
        add_tag(&mut editor, &mut doc, tag_id, "hero");
        attach_tag(&mut doc, entity_id, tag_id);

        let entity = |doc: &DocumentStore| doc.project.library.entities.iter().find(|e| e.id == entity_id).cloned().expect("entity");
        assert_eq!(entity(&doc).tags, vec![tag_id], "entity carries the confirmed tag");
        assert_eq!(entity(&doc).ai.suggested_tags, vec![tag_id], "entity carries the suggested tag");

        delete_tag(&mut editor, &mut doc, tag_id);
        assert!(tags(&doc).is_empty(), "delete removes the tag definition");
        assert!(entity(&doc).tags.is_empty(), "delete prunes the confirmed tag from the entity");
        assert!(entity(&doc).ai.suggested_tags.is_empty(), "delete prunes the suggested tag from the entity");

        editor.history.undo(&mut doc).expect("undo delete");
        assert_eq!(tags(&doc).len(), 1, "undo restores the tag definition");
        assert_eq!(entity(&doc).tags, vec![tag_id], "undo restores the entity's confirmed tag");
        assert_eq!(entity(&doc).ai.suggested_tags, vec![tag_id], "undo restores the entity's suggested tag");
    }
}
