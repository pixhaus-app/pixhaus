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

use std::collections::BTreeMap;

use eframe::egui;
use pixhaus_core::project::library::ai::ProjectAi;
use pixhaus_core::project::{AssetId, EntityContent, EntityId, ModelId, OperationKind, Quality, ReferenceRole, Rgba, TagDefinition, TagId};

use crate::app::ShellApp;
use crate::cockpit::CockpitReference;
use crate::commands::{push_ai_library_edit, push_tag_edit};
use crate::document::DocumentStore;
use crate::editor::EditorState;

/// Side length, in points, of a reference thumbnail in the browser grid. The
/// gallery now owns the full panel width, so the thumbnails are large enough to
/// read the pixel art at a glance rather than the cramped 72 px of the old
/// single-column layout.
const THUMB: f32 = 112.0;

/// Width, in points, of a character-card or style-swatch tile. Bounded so the
/// cards flow as a wrapped grid across the gallery instead of one stretched-out
/// row per card.
const CARD_W: f32 = 250.0;

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

        // Two panes share the full panel width. The left rail holds project
        // configuration — AI defaults, the style corpus, and tags — at a fixed,
        // resizable width; the central pane is the asset gallery proper
        // (references, character cards, style swatches) and takes whatever width
        // is left. Each pane scrolls on its own so a long gallery never pushes
        // the settings out of reach. Order matters: the side panel is declared
        // before the central panel so the gallery fills the remainder.
        egui::Panel::left("asset_settings_rail")
            .resizable(true)
            .default_size(320.0)
            .show_inside(ui, |ui| {
                ui.add_space(6.0);
                ui.label(egui::RichText::new(format!("{} Project settings", crate::icons::AI_DEFAULTS)).strong());
                ui.add_space(6.0);
                egui::ScrollArea::vertical()
                    .auto_shrink([false, false])
                    .id_salt("asset_settings_scroll")
                    .show(ui, |ui| {
                        self.project_ai_section(ui);
                        ui.add_space(10.0);
                        ui.separator();
                        ui.add_space(8.0);
                        self.style_corpus_section(ui);
                        ui.add_space(10.0);
                        ui.separator();
                        ui.add_space(8.0);
                        self.tag_manager_section(ui);
                    });
            });

        egui::CentralPanel::default().show_inside(ui, |ui| {
            ui.add_space(6.0);
            ui.label(egui::RichText::new(format!("{} Assets", crate::icons::LIBRARY)).strong());
            ui.add_space(6.0);
            egui::ScrollArea::vertical()
                .auto_shrink([false, false])
                .id_salt("asset_gallery_scroll")
                .show(ui, |ui| {
                    self.asset_references_section(ui);
                    ui.add_space(12.0);
                    ui.separator();
                    ui.add_space(8.0);
                    self.asset_cards_section(ui);
                    ui.add_space(12.0);
                    ui.separator();
                    ui.add_space(8.0);
                    self.asset_swatches_section(ui);
                });
        });
    }

    /// The Project AI section: per-project generation defaults — style notes,
    /// the chroma-key color, the quality tier, the candidate count, and a
    /// per-operation model preference. Every change routes through
    /// [`push_ai_library_edit`] so it is one undoable step and persists with the
    /// project. These are the defaults a fresh cockpit run inherits.
    fn project_ai_section(&mut self, ui: &mut egui::Ui) {
        ui.label(egui::RichText::new(format!("{} Project AI defaults", crate::icons::AI_DEFAULTS)).strong());
        ui.label(
            egui::RichText::new("Generation defaults for this project. A new Create run inherits the quality tier and candidate count set here.")
                .small()
                .weak(),
        );
        ui.add_space(4.0);

        // Read the editable values into locals, render against them, then write
        // back through `push_ai_library_edit` only when something changed — so
        // the undo stack gets one entry per committed edit, not one per frame.
        let ai = &self.doc.project.library.ai;
        let mut style_notes = ai.style_notes.clone();
        let mut chroma = ai.default_chroma;
        let mut quality = ai.default_quality;
        let mut count = ai.default_candidate_count;
        let mut prefs = ai.per_operation_model_prefs.clone();
        let before = (style_notes.clone(), chroma, quality, count, prefs.clone());

        egui::Frame::group(ui.style()).show(ui, |ui| {
            ui.label(egui::RichText::new("Style notes").small().weak());
            ui.add(
                egui::TextEdit::multiline(&mut style_notes)
                    .hint_text("project house style — palette, era, line weight…")
                    .desired_rows(2)
                    .desired_width(f32::INFINITY),
            );

            ui.add_space(4.0);
            ui.horizontal(|ui| {
                ui.label("Chroma key");
                let mut col = egui::Color32::from_rgba_unmultiplied(chroma.r, chroma.g, chroma.b, chroma.a);
                if ui
                    .color_edit_button_srgba(&mut col)
                    .on_hover_text("Background key color for new generations")
                    .changed()
                {
                    chroma = Rgba::new(col.r(), col.g(), col.b(), col.a());
                }
                if ui.small_button("Magenta").on_hover_text("Reset to the default magenta key").clicked() {
                    chroma = pixhaus_core::project::default_reference_chroma();
                }
            });

            ui.add_space(4.0);
            ui.horizontal(|ui| {
                ui.label("Default quality");
                egui::ComboBox::from_id_salt("ai_default_quality")
                    .selected_text(crate::library::quality_label(quality))
                    .show_ui(ui, |ui| {
                        for q in [Quality::Auto, Quality::Low, Quality::Medium, Quality::High] {
                            ui.selectable_value(&mut quality, q, crate::library::quality_label(q));
                        }
                    });
            });

            ui.horizontal(|ui| {
                ui.label("Default candidates");
                ui.add(egui::DragValue::new(&mut count).range(1..=4));
            });

            ui.add_space(4.0);
            ui.label(egui::RichText::new("Per-operation model").small().weak());
            for op in USER_OPERATIONS {
                operation_model_picker(ui, op, &mut prefs);
            }
        });

        let after = (style_notes, chroma, quality, count, prefs);
        if after != before {
            let (style_notes, chroma, quality, count, prefs) = after;
            push_ai_library_edit(&mut self.editor, &mut self.doc, "Edit project AI defaults", |ai| {
                ai.style_notes = style_notes;
                ai.default_chroma = chroma;
                ai.default_quality = quality;
                ai.default_candidate_count = count;
                ai.per_operation_model_prefs = prefs;
            });
        }
    }

    /// The style-corpus section: curate which sprite entities define the
    /// project's look. Each project sprite entity gets an include/exclude
    /// checkbox that writes [`ProjectAi::style_corpus`]; the corpus biases
    /// future generations through style notes and composition — it does not
    /// train a `LoRA` (out of scope). The shared `style_notes` editor lives in
    /// [`Self::project_ai_section`] above, so this section is the entity-set
    /// half of project style learning. Toggles route through
    /// [`push_ai_library_edit`] so each is one undoable step.
    fn style_corpus_section(&mut self, ui: &mut egui::Ui) {
        // Snapshot the project sprite entities and the current corpus before
        // borrowing `self` mutably to apply a toggle.
        let entities: Vec<(EntityId, String)> = self
            .doc
            .project
            .library
            .entities
            .iter()
            .filter(|e| matches!(e.content, EntityContent::Sprites { .. }))
            .map(|e| (e.id, e.name.clone()))
            .collect();
        let corpus = self.doc.project.library.ai.style_corpus.clone();

        ui.label(egui::RichText::new(format!("{} Style corpus ({})", crate::icons::SPARKLE, corpus.len())).strong());
        ui.label(
            egui::RichText::new(
                "Pick the sprite entities whose canonical sheets define this project's look. The corpus biases future generations; it does not train a model.",
            )
            .small()
            .weak(),
        );
        ui.add_space(4.0);

        if entities.is_empty() {
            ui.label(egui::RichText::new("No sprite entities to include yet.").small().weak());
            return;
        }

        let mut toggle: Option<(EntityId, bool)> = None;
        egui::Frame::group(ui.style()).show(ui, |ui| {
            for (id, name) in &entities {
                let mut included = corpus.contains(id);
                if ui
                    .checkbox(&mut included, name)
                    .on_hover_text("Include this entity in the project style corpus")
                    .changed()
                {
                    toggle = Some((*id, included));
                }
            }
        });

        if let Some((id, included)) = toggle {
            self.set_corpus_membership(id, included);
        }
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
            let resp = ui.add(
                egui::TextEdit::singleline(&mut self.tag_new_name)
                    .hint_text("New tag name")
                    .desired_width(160.0),
            );
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
                        let next = if col.a() == 0 {
                            None
                        } else {
                            Some(Rgba::new(col.r(), col.g(), col.b(), col.a()))
                        };
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
        let tiles: Vec<(AssetId, ReferenceRole, Vec<String>, Vec<u8>)> =
            refs.iter().map(|r| (r.id, r.default_role, r.tags.clone(), r.image.bytes.clone())).collect();

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
                        // Role on the left, an icon-only delete hugging the right
                        // — the verbose "Delete" button per tile was most of the
                        // old grid's visual noise.
                        ui.horizontal(|ui| {
                            ui.label(egui::RichText::new(role_label(*role)).small().strong());
                            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                if ui.small_button(crate::icons::TRASH).on_hover_text("Delete reference").clicked() {
                                    delete = Some(*id);
                                }
                            });
                        });
                        if !tags.is_empty() {
                            ui.horizontal_wrapped(|ui| {
                                for tag in tags {
                                    ui.label(egui::RichText::new(format!("#{tag}")).small().weak());
                                }
                            });
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
        ui.add_space(4.0);
        ui.horizontal_wrapped(|ui| {
            for (id, name, member_ids, style_notes) in &cards {
                egui::Frame::group(ui.style()).show(ui, |ui| {
                    ui.vertical(|ui| {
                        ui.set_width(CARD_W);
                        let editing = self.asset_rename.as_ref().is_some_and(|(rid, _)| *rid == id.get());
                        ui.horizontal(|ui| {
                            if editing {
                                if let Some((_, draft)) = self.asset_rename.as_mut() {
                                    let resp = ui.add(egui::TextEdit::singleline(draft).desired_width(CARD_W - 28.0));
                                    if resp.lost_focus() || ui.button(crate::icons::CHECK).clicked() {
                                        rename = Some((*id, draft.clone()));
                                    }
                                }
                            } else {
                                ui.label(egui::RichText::new(name).strong());
                                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                    if ui.small_button(crate::icons::RENAME).on_hover_text("Rename").clicked() {
                                        self.asset_rename = Some((id.get(), name.clone()));
                                    }
                                });
                            }
                        });
                        if !style_notes.is_empty() {
                            ui.label(egui::RichText::new(style_notes).small().weak());
                        }
                        self.asset_member_thumbs(ui, member_ids);
                        ui.horizontal(|ui| {
                            if ui
                                .button(format!("{} Use", crate::icons::SPARKLE))
                                .on_hover_text("Stage the card's first reference in the cockpit")
                                .clicked()
                            {
                                reuse = Some(*id);
                            }
                            if ui.small_button(crate::icons::TRASH).on_hover_text("Delete card").clicked() {
                                delete = Some(*id);
                            }
                        });
                    });
                });
            }
        });

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
        ui.add_space(4.0);
        ui.horizontal_wrapped(|ui| {
            for (id, name, member_ids, style_notes) in &swatches {
                egui::Frame::group(ui.style()).show(ui, |ui| {
                    ui.vertical(|ui| {
                        ui.set_width(CARD_W);
                        let editing = self.asset_rename.as_ref().is_some_and(|(rid, _)| *rid == id.get());
                        ui.horizontal(|ui| {
                            if editing {
                                if let Some((_, draft)) = self.asset_rename.as_mut() {
                                    let resp = ui.add(egui::TextEdit::singleline(draft).desired_width(CARD_W - 28.0));
                                    if resp.lost_focus() || ui.button(crate::icons::CHECK).clicked() {
                                        rename = Some((*id, draft.clone()));
                                    }
                                }
                            } else {
                                ui.label(egui::RichText::new(name).strong());
                                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                    if ui.small_button(crate::icons::RENAME).on_hover_text("Rename").clicked() {
                                        self.asset_rename = Some((id.get(), name.clone()));
                                    }
                                });
                            }
                        });
                        if !style_notes.is_empty() {
                            ui.label(egui::RichText::new(style_notes).small().weak());
                        }
                        self.asset_member_thumbs(ui, member_ids);
                        ui.horizontal(|ui| {
                            if ui
                                .button(format!("{} Apply", crate::icons::SPARKLE))
                                .on_hover_text("Stage the swatch's first reference as a style reference")
                                .clicked()
                            {
                                apply = Some(*id);
                            }
                            if ui.small_button(crate::icons::TRASH).on_hover_text("Delete swatch").clicked() {
                                delete = Some(*id);
                            }
                        });
                    });
                });
            }
        });

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

    /// Includes (`included = true`) or excludes entity `id` from the project's
    /// style corpus as one undoable edit. A no-op when the membership already
    /// matches `included`.
    pub(crate) fn set_corpus_membership(&mut self, id: EntityId, included: bool) {
        set_corpus_membership(&mut self.editor, &mut self.doc, id, included);
    }

    /// Accepts a suggested tag on an entity: promotes `tag` into the entity's
    /// confirmed tags and drops it from `suggested_tags`, as one undoable edit.
    pub(crate) fn accept_suggested_tag(&mut self, entity_id: EntityId, tag: TagId) {
        accept_suggested_tag(&mut self.editor, &mut self.doc, entity_id, tag);
    }

    /// Rejects a suggested tag on an entity: drops `tag` from `suggested_tags`
    /// without confirming it, as one undoable edit.
    pub(crate) fn reject_suggested_tag(&mut self, entity_id: EntityId, tag: TagId) {
        reject_suggested_tag(&mut self.editor, &mut self.doc, entity_id, tag);
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

/// Accepts the suggested `tag` on entity `entity_id`: moves it from the entity's
/// `suggested_tags` into its confirmed `tags` (no duplicate if already
/// confirmed), as one undoable [`crate::commands::LibraryTagEdit`]. A no-op when
/// the tag was not actually suggested on that entity.
fn accept_suggested_tag(editor: &mut EditorState, doc: &mut DocumentStore, entity_id: EntityId, tag: TagId) {
    push_tag_edit(editor, doc, "Accept suggested tag", |doc| {
        if let Some(entity) = doc.project.library.entities.iter_mut().find(|e| e.id == entity_id) {
            if entity.ai.suggested_tags.contains(&tag) {
                entity.ai.suggested_tags.retain(|t| *t != tag);
                if !entity.tags.contains(&tag) {
                    entity.tags.push(tag);
                }
            }
        }
    });
}

/// Rejects the suggested `tag` on entity `entity_id`: drops it from
/// `suggested_tags` without confirming it, as one undoable edit.
fn reject_suggested_tag(editor: &mut EditorState, doc: &mut DocumentStore, entity_id: EntityId, tag: TagId) {
    push_tag_edit(editor, doc, "Reject suggested tag", |doc| {
        if let Some(entity) = doc.project.library.entities.iter_mut().find(|e| e.id == entity_id) {
            entity.ai.suggested_tags.retain(|t| *t != tag);
        }
    });
}

/// Includes or excludes entity `id` in the project style corpus as one
/// undoable [`crate::commands::AiLibraryEdit`]. Including is idempotent — an
/// entity already in the corpus is not duplicated — and excluding drops every
/// matching id. A toggle that does not change the set produces no undo entry
/// (`push_ai_library_edit` skips a no-op edit).
fn set_corpus_membership(editor: &mut EditorState, doc: &mut DocumentStore, id: EntityId, included: bool) {
    push_ai_library_edit(editor, doc, "Edit style corpus", |ai| {
        if included {
            if !ai.style_corpus.contains(&id) {
                ai.style_corpus.push(id);
            }
        } else {
            ai.style_corpus.retain(|e| *e != id);
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

/// User-facing operations a project may pin a model preference for, in display
/// order. Omits `LoraTraining` (`LoRA` is out of scope) and the non-user-facing
/// routing-only kinds (`CrossModelGrid`, `VectorExport`, `Upscale`) whose
/// surfaces are descoped in v2.
const USER_OPERATIONS: [OperationKind; 5] = [
    OperationKind::FreshGeneration,
    OperationKind::MaskedRefinement,
    OperationKind::PromptOnlyRefinement,
    OperationKind::RegionalRefinement,
    OperationKind::Promotion,
];

/// Human label for a user-facing [`OperationKind`].
fn operation_label(op: OperationKind) -> &'static str {
    match op {
        OperationKind::FreshGeneration => "Fresh generation",
        OperationKind::MaskedRefinement => "Masked refinement",
        OperationKind::PromptOnlyRefinement => "Prompt-only refinement",
        OperationKind::RegionalRefinement => "Regional refinement",
        OperationKind::Promotion => "Promote to final",
        OperationKind::ChatTurn => "Chat turn",
        OperationKind::CrossModelGrid => "Cross-model grid",
        OperationKind::VectorExport => "Vector export",
        OperationKind::Upscale => "Upscale",
        OperationKind::LoraTraining => "LoRA training",
    }
}

/// A `(none)`-plus-look-models dropdown that pins a model preference for `op`.
/// `(none)` clears the entry, meaning "use the router default for this op". The
/// model list reuses the library's [`crate::library::LOOK_MODELS`], excluding
/// the upscale/vectorize models per task 1.
fn operation_model_picker(ui: &mut egui::Ui, op: OperationKind, prefs: &mut BTreeMap<OperationKind, ModelId>) {
    ui.horizontal(|ui| {
        ui.label(operation_label(op));
        let text = prefs.get(&op).copied().map_or("(none)", crate::library::model_label);
        egui::ComboBox::from_id_salt(("ai_op_model", op)).selected_text(text).show_ui(ui, |ui| {
            if ui.selectable_label(!prefs.contains_key(&op), "(none)").clicked() {
                prefs.remove(&op);
            }
            for m in crate::library::LOOK_MODELS {
                if ui.selectable_label(prefs.get(&op) == Some(&m), crate::library::model_label(m)).clicked() {
                    prefs.insert(op, m);
                }
            }
        });
    });
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

    use super::{
        accept_suggested_tag, add_tag, card_first_reference, delete_card, delete_reference, delete_swatch, delete_tag, recolor_tag, reject_suggested_tag,
        rename_card, rename_swatch, rename_tag, set_corpus_membership, swatch_first_reference,
    };
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

    // ── Task 16: project AI defaults ────────────────────────────────────────

    #[test]
    fn user_operations_omit_lora_training() {
        use pixhaus_core::project::OperationKind;
        assert!(
            !super::USER_OPERATIONS.contains(&OperationKind::LoraTraining),
            "LoRA is out of scope, so its training op never appears in the picker list"
        );
    }

    #[test]
    fn editing_ai_defaults_round_trips_under_one_undo() {
        use pixhaus_core::project::{ModelId, OperationKind, Quality};

        let mut doc = DocumentStore::new();
        let mut editor = EditorState::default();

        let before = doc.project.library.ai.clone();
        assert_eq!(before.default_quality, Quality::Medium, "a fresh project defaults to Medium");
        assert_eq!(before.default_candidate_count, 2, "a fresh project defaults to two candidates");
        assert_eq!(
            before.default_chroma,
            pixhaus_core::project::default_reference_chroma(),
            "the default chroma is magenta"
        );
        assert!(before.per_operation_model_prefs.is_empty(), "no per-operation overrides to start");

        // Edit every default through the same path the Project AI section uses.
        push_ai_library_edit(&mut editor, &mut doc, "Edit project AI defaults", |ai| {
            ai.style_notes = "muted SNES palette".to_owned();
            ai.default_chroma = Rgba::opaque(0, 255, 0);
            ai.default_quality = Quality::High;
            ai.default_candidate_count = 4;
            ai.per_operation_model_prefs.insert(OperationKind::FreshGeneration, ModelId::FalFluxDev);
        });

        let after = &doc.project.library.ai;
        assert_eq!(after.style_notes, "muted SNES palette");
        assert_eq!(after.default_chroma, Rgba::opaque(0, 255, 0));
        assert_eq!(after.default_quality, Quality::High);
        assert_eq!(after.default_candidate_count, 4);
        assert_eq!(
            after.per_operation_model_prefs.get(&OperationKind::FreshGeneration),
            Some(&ModelId::FalFluxDev),
            "the fresh-generation model override is pinned"
        );

        editor.history.undo(&mut doc).expect("undo the defaults edit");
        assert_eq!(doc.project.library.ai, before, "one undo restores every default to its prior value");
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

    // ── Task 14: suggested-tag accept / reject ──────────────────────────────

    /// Creates an entity carrying `tag` as a suggested-only tag (not yet
    /// confirmed), the state auto-tagging leaves behind for the artist to judge.
    fn entity_with_suggestion(doc: &mut DocumentStore, tag: TagId) -> EntityId {
        let entity_id = doc.create_sprite("Hero", pixhaus_core::project::Size::new(16, 16)).entity_id;
        let entity = doc.project.library.entities.iter_mut().find(|e| e.id == entity_id).expect("entity exists");
        entity.ai.suggested_tags.push(tag);
        entity_id
    }

    #[test]
    fn accept_promotes_a_suggested_tag_under_one_undo() {
        let mut doc = DocumentStore::new();
        let mut editor = EditorState::default();
        let tag_id = TagId::new(doc.alloc_id());
        add_tag(&mut editor, &mut doc, tag_id, "armored");
        let entity_id = entity_with_suggestion(&mut doc, tag_id);

        let entity = |doc: &DocumentStore| doc.project.library.entities.iter().find(|e| e.id == entity_id).cloned().expect("entity");
        assert!(entity(&doc).tags.is_empty(), "the tag starts suggested, not confirmed");
        assert_eq!(entity(&doc).ai.suggested_tags, vec![tag_id], "the tag starts in the suggestion list");

        accept_suggested_tag(&mut editor, &mut doc, entity_id, tag_id);
        assert_eq!(entity(&doc).tags, vec![tag_id], "accept promotes the tag into confirmed tags");
        assert!(entity(&doc).ai.suggested_tags.is_empty(), "accept drops the tag from the suggestion list");

        editor.history.undo(&mut doc).expect("undo accept");
        assert!(entity(&doc).tags.is_empty(), "one undo restores the unconfirmed state");
        assert_eq!(entity(&doc).ai.suggested_tags, vec![tag_id], "one undo restores the suggestion");
    }

    #[test]
    fn reject_drops_a_suggested_tag_without_confirming() {
        let mut doc = DocumentStore::new();
        let mut editor = EditorState::default();
        let tag_id = TagId::new(doc.alloc_id());
        add_tag(&mut editor, &mut doc, tag_id, "armored");
        let entity_id = entity_with_suggestion(&mut doc, tag_id);

        let entity = |doc: &DocumentStore| doc.project.library.entities.iter().find(|e| e.id == entity_id).cloned().expect("entity");

        reject_suggested_tag(&mut editor, &mut doc, entity_id, tag_id);
        assert!(entity(&doc).ai.suggested_tags.is_empty(), "reject drops the suggestion");
        assert!(entity(&doc).tags.is_empty(), "reject never confirms the tag");

        editor.history.undo(&mut doc).expect("undo reject");
        assert_eq!(entity(&doc).ai.suggested_tags, vec![tag_id], "one undo restores the suggestion");
        assert!(entity(&doc).tags.is_empty(), "undo of a reject still does not confirm");
    }

    #[test]
    fn accept_does_not_duplicate_an_already_confirmed_tag() {
        let mut doc = DocumentStore::new();
        let mut editor = EditorState::default();
        let tag_id = TagId::new(doc.alloc_id());
        add_tag(&mut editor, &mut doc, tag_id, "armored");
        let entity_id = entity_with_suggestion(&mut doc, tag_id);
        // The same tag is already confirmed when its suggestion is accepted.
        doc.project
            .library
            .entities
            .iter_mut()
            .find(|e| e.id == entity_id)
            .expect("entity")
            .tags
            .push(tag_id);

        accept_suggested_tag(&mut editor, &mut doc, entity_id, tag_id);
        let entity = doc.project.library.entities.iter().find(|e| e.id == entity_id).expect("entity");
        assert_eq!(entity.tags, vec![tag_id], "accept never duplicates an already-confirmed tag");
        assert!(entity.ai.suggested_tags.is_empty(), "accept still clears the suggestion");
    }

    // ── Task 13: style-corpus management ────────────────────────────────────

    fn corpus(doc: &DocumentStore) -> &[EntityId] {
        &doc.project.library.ai.style_corpus
    }

    #[test]
    fn toggling_an_entity_into_the_corpus_updates_and_undoes() {
        let mut doc = DocumentStore::new();
        let mut editor = EditorState::default();
        let sprite = doc.create_sprite("Hero", pixhaus_core::project::Size::new(16, 16));
        let entity_id = sprite.entity_id;
        assert!(corpus(&doc).is_empty(), "a fresh project has an empty style corpus");

        // Including the entity adds its id once.
        set_corpus_membership(&mut editor, &mut doc, entity_id, true);
        assert_eq!(corpus(&doc), [entity_id], "including the entity adds it to the corpus");

        // Including an already-included entity is idempotent and a no-op edit.
        set_corpus_membership(&mut editor, &mut doc, entity_id, true);
        assert_eq!(corpus(&doc), [entity_id], "re-including does not duplicate the id");

        // Excluding drops it back out.
        set_corpus_membership(&mut editor, &mut doc, entity_id, false);
        assert!(corpus(&doc).is_empty(), "excluding removes the entity from the corpus");

        // One undo restores the prior include; the redundant re-include made no entry.
        editor.history.undo(&mut doc).expect("undo the exclude");
        assert_eq!(corpus(&doc), [entity_id], "undo restores the included entity");

        // A second undo unwinds the initial include back to empty.
        editor.history.undo(&mut doc).expect("undo the include");
        assert!(corpus(&doc).is_empty(), "undo restores the empty corpus");
    }
}
