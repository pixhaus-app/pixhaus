//! The Create-mode **creation cockpit**: the AI front door.
//!
//! One surface — context, a subject prompt and dials, the always-visible
//! composed prompt, a single Generate, and a variant gallery of results below.
//! Generation is `generate -> preview -> accept or reject -> refine -> branch`,
//! anchored to the artist's work, with the active palette and the project's
//! `style_notes` folded in by default. The library (saved prompts/structures and
//! the asset shelf) is reached *from* here, never a screen you must visit first.
//!
//! Results carry the provenance the verb already emits — the exact composed
//! prompt sent, the user prompt, backend, model, seed, and cost — and persist
//! into the core [`ReferenceSheet`] model so history survives a save and reopen.

use eframe::egui;
use pixhaus_ai::plugin::AnchorPayload;
use pixhaus_ai::verbs::reference_sheet::{ReferenceInput, SheetVariantOutput};
use pixhaus_core::project::library::composition::VarControl;
use pixhaus_core::project::{
    EntityContent, GenerationProvenance, ReferenceImage, ReferenceRole, ReferenceSheet, RefinementKind, SheetVariant, SheetVariantId, VariantOrigin,
};

use crate::ai;
use crate::app::{JobStatus, ShellApp};

/// One generated candidate in the gallery, with its decoded image, provenance,
/// lineage, and lazily-built preview texture.
pub(crate) struct CockpitCandidate {
    /// Decoded PNG bytes — for the canvas preview and texture upload.
    pub png: Vec<u8>,
    /// Full verb output: composed prompt, user prompt, provenance, composition.
    pub output: SheetVariantOutput,
    /// Reported run cost in USD, if any.
    pub cost_usd: Option<f64>,
    /// Index of the candidate this one descends from, for lineage display.
    pub parent: Option<usize>,
    /// How this candidate came to be (fresh, re-roll, refinement, branch).
    pub origin: VariantOrigin,
    /// The id this candidate persisted under in the core reference sheet.
    pub core_id: SheetVariantId,
    /// Lazily-built preview texture.
    pub texture: Option<egui::TextureHandle>,
    /// Whether the provenance panel is expanded on the card.
    pub expanded: bool,
    /// Editable name draft used when saving this candidate as a character card
    /// or style swatch. Pre-filled from the subject; the artist renames before
    /// saving instead of accepting a `first_words(...)` auto-name.
    pub name: String,
}

/// A staged drag-in reference: an anchor image with a role and weight, fed to
/// the next generation as `reference_images` / `style_image`.
pub(crate) struct CockpitReference {
    /// Short label for the chip ("result #2", "hero sheet").
    pub label: String,
    /// PNG bytes.
    pub png: Vec<u8>,
    /// What this reference contributes.
    pub role: ReferenceRole,
    /// Relative influence.
    pub weight: f32,
    /// Lazily-built thumbnail.
    pub texture: Option<egui::TextureHandle>,
}

/// A pending generation's lineage, captured at Generate and applied when the
/// results land so each result records where it came from.
pub(crate) struct PendingLineage {
    /// In-gallery parent index, or `None` for a from-scratch generation.
    pub parent: Option<usize>,
    /// Parent's persisted id, threaded into the core variant's `parent_variant_id`.
    pub parent_core: Option<SheetVariantId>,
    /// Origin tag stamped on the results.
    pub origin: VariantOrigin,
    /// Whether the results replace the gallery (from-scratch) or append to it.
    pub replace: bool,
}

impl Default for PendingLineage {
    fn default() -> Self {
        Self {
            parent: None,
            parent_core: None,
            origin: VariantOrigin::FreshGeneration,
            replace: true,
        }
    }
}

impl ShellApp {
    /// The Anchor stage's right inspector: the composition controls — context,
    /// subject prompt + dials, drag-in references, the composed prompt, Generate,
    /// and recent prompts. The studio hosts this; the results gallery is the
    /// center surface ([`Self::anchor_gallery`]).
    pub(crate) fn anchor_inspector(&mut self, ui: &mut egui::Ui) {
        if !self.backend_ready {
            self.key_entry(ui);
            ui.separator();
        }
        self.cockpit_context(ui);
        ui.add_space(8.0);
        self.cockpit_inputs(ui);
        ui.add_space(8.0);
        self.cockpit_dials(ui);
        ui.add_space(8.0);
        self.cockpit_references(ui);
        ui.add_space(8.0);
        self.cockpit_composed_prompt(ui);
        ui.add_space(10.0);
        self.cockpit_generate_row(ui);
        ui.add_space(6.0);
        self.cockpit_history(ui);
    }

    /// The Anchor stage's results gallery (cards with re-roll / more-like-this /
    /// refine / branch / save-as-card / approve / provenance). Rendered in the
    /// inspector, which already scrolls; clicking a card selects it for the
    /// center viewport.
    pub(crate) fn anchor_gallery(&mut self, ui: &mut egui::Ui) {
        self.cockpit_gallery(ui);
    }

    /// Context band: the active palette and the project style notes, shown and
    /// folded into the prompt automatically so the artist sees the constraints
    /// without re-entering them.
    fn cockpit_context(&mut self, ui: &mut egui::Ui) {
        let palette = crate::theme::Palette::for_theme(ui.ctx().theme());
        let mut notes_changed = false;
        egui::Frame::group(ui.style()).fill(palette.bg_elevated).show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.label(egui::RichText::new(format!("{} Context", crate::icons::PALETTE)).strong());
                ui.label(egui::RichText::new("folded into every prompt").small().weak());
            });
            // Active palette swatches.
            if let Some(pal) = self.doc.active_palette() {
                ui.horizontal_wrapped(|ui| {
                    for entry in pal.colors.iter().take(24) {
                        let c = entry.color;
                        let color = egui::Color32::from_rgba_unmultiplied(c.r, c.g, c.b, c.a);
                        let (rect, _) = ui.allocate_exact_size(egui::vec2(14.0, 14.0), egui::Sense::hover());
                        ui.painter().rect_filled(rect, 2.0, color);
                        ui.painter()
                            .rect_stroke(rect, 2.0, egui::Stroke::new(1.0, palette.border), egui::StrokeKind::Inside);
                    }
                });
            }
            ui.add_space(4.0);
            ui.label(egui::RichText::new("Style notes").small().weak());
            let notes = &mut self.doc.project.library.ai.style_notes;
            notes_changed = ui
                .add(
                    egui::TextEdit::multiline(notes)
                        .hint_text("project house style — palette, era, line weight…")
                        .desired_rows(2)
                        .desired_width(f32::INFINITY),
                )
                .changed();
        });
        // Style notes lead the composed prompt, so an edit re-syncs the preview.
        if notes_changed {
            self.ck_dirty = true;
        }
    }

    /// Prompt + dials: the subject box, the structure picker (free-form by
    /// default), the variant count, and the seed control.
    fn cockpit_inputs(&mut self, ui: &mut egui::Ui) {
        ui.label(egui::RichText::new("Subject").strong());
        let subject = ui.add(
            egui::TextEdit::multiline(&mut self.rs_prompt)
                .hint_text("a small knight with a round shield")
                .desired_rows(2)
                .desired_width(f32::INFINITY),
        );
        if subject.changed() {
            self.ck_dirty = true;
        }

        ui.add_space(4.0);
        ui.horizontal(|ui| {
            ui.label("Output");
            let options = ai::structure_options(&self.doc.project.library.ai);
            let current = options
                .iter()
                .find(|(id, _)| *id == self.ck_structure)
                .map_or_else(|| "Single image".to_owned(), |(_, name)| name.clone());
            egui::ComboBox::from_id_salt("cockpit_structure").selected_text(current).show_ui(ui, |ui| {
                for (id, name) in &options {
                    if ui.selectable_label(self.ck_structure == *id, name).clicked() {
                        self.ck_structure.clone_from(id);
                        self.ck_dirty = true;
                    }
                }
            });
        });

        // Resolution applies to free-form output only; a Paneled structure fixes
        // its own canvas from the layout, so the picker would be a no-op there.
        if self.ck_structure == ai::SINGLE_STRUCTURE_ID {
            self.cockpit_resolution(ui);
        }

        ui.add(egui::Slider::new(&mut self.rs_num_variants, 1..=4).text("variants"));

        ui.horizontal(|ui| {
            ui.checkbox(&mut self.ck_seed_fixed, "Fixed seed")
                .on_hover_text("Pin the seed for a reproducible result; off draws a fresh seed each run");
            ui.add_enabled(self.ck_seed_fixed, egui::DragValue::new(&mut self.ck_seed));
        });
    }

    /// Resolution + aspect picker for a free-form anchor, with a resolved-size
    /// hint. Drives [`Self::ck_aspect`] / [`Self::ck_resolution`] / custom dims.
    fn cockpit_resolution(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            ui.label("Size");
            egui::ComboBox::from_id_salt("ck_resolution")
                .selected_text(format!("{}px", self.ck_resolution))
                .show_ui(ui, |ui| {
                    for px in [1024_u32, 1536, 2048] {
                        ui.selectable_value(&mut self.ck_resolution, px, format!("{px}px"));
                    }
                });
            egui::ComboBox::from_id_salt("ck_aspect")
                .selected_text(self.ck_aspect.label())
                .show_ui(ui, |ui| {
                    for aspect in AnchorAspect::ALL {
                        ui.selectable_value(&mut self.ck_aspect, aspect, aspect.label());
                    }
                });
        });
        if self.ck_aspect == AnchorAspect::Custom {
            ui.horizontal(|ui| {
                ui.label("Custom");
                ui.add(egui::DragValue::new(&mut self.ck_custom.0).range(256..=2048).suffix(" w"));
                ui.add(egui::DragValue::new(&mut self.ck_custom.1).range(256..=2048).suffix(" h"));
            });
        }
        let (w, h) = anchor_target_dims(self.ck_aspect, self.ck_resolution, self.ck_custom);
        ui.label(egui::RichText::new(format!("{w} \u{00d7} {h}")).small().weak());
    }

    /// Creative dials: an optional saved-prompt template, its typed-control
    /// variables, and the serendipity controls (lock, per-dial randomize, and
    /// "surprise me"). With no template the cockpit stays a blank subject box.
    fn cockpit_dials(&mut self, ui: &mut egui::Ui) {
        // Template picker.
        let options = ai::prompt_options(&self.doc.project.library.ai);
        ui.horizontal(|ui| {
            ui.label("Template");
            let current = self
                .ck_prompt_id
                .as_ref()
                .and_then(|id| options.iter().find(|(i, _)| i == id))
                .map_or_else(|| "None (free-form)".to_owned(), |(_, n)| n.clone());
            egui::ComboBox::from_id_salt("ck_template").selected_text(current).show_ui(ui, |ui| {
                if ui.selectable_label(self.ck_prompt_id.is_none(), "None (free-form)").clicked() {
                    self.set_template(None);
                }
                for (id, name) in &options {
                    if ui.selectable_label(self.ck_prompt_id.as_deref() == Some(id), name).clicked() {
                        self.set_template(Some(id.clone()));
                    }
                }
            });
        });

        let Some(prompt_id) = self.ck_prompt_id.clone() else {
            return;
        };
        let vars = ai::prompt_variables(&self.doc.project.library.ai, &prompt_id);
        if vars.is_empty() {
            return;
        }

        ui.horizontal(|ui| {
            ui.label(egui::RichText::new("Dials").strong());
            if ui
                .small_button(format!("{} Surprise me", crate::icons::DICE))
                .on_hover_text("Randomize every unlocked dial")
                .clicked()
            {
                self.surprise_me(&vars);
            }
        });

        // Render each dial against a local value, then write it back — avoids a
        // long-lived borrow of `ck_vars` while other dial state mutates.
        let mut toggle_lock: Option<String> = None;
        let mut randomize: Option<String> = None;
        let mut dirty = false;
        for var in &vars {
            let locked = self.ck_var_locked.get(&var.key).copied().unwrap_or(false);
            let mut value = self.ck_vars.get(&var.key).cloned().unwrap_or_else(|| var.default.clone());
            ui.horizontal(|ui| {
                let lock_icon = if locked { crate::icons::LOCK } else { crate::icons::UNLOCK };
                if ui.small_button(lock_icon).on_hover_text("Lock this dial against randomize").clicked() {
                    toggle_lock = Some(var.key.clone());
                }
                ui.label(&var.label);
                match &var.control {
                    VarControl::Text => {
                        dirty |= ui.add(egui::TextEdit::singleline(&mut value).desired_width(130.0)).changed();
                    }
                    VarControl::Select { choices } | VarControl::Wildcard { choices } => {
                        egui::ComboBox::from_id_salt(("ck_dial", var.key.as_str()))
                            .selected_text(value.clone())
                            .show_ui(ui, |ui| {
                                for c in choices {
                                    if ui.selectable_label(&value == c, c).clicked() {
                                        value.clone_from(c);
                                        dirty = true;
                                    }
                                }
                            });
                    }
                    VarControl::Number { min, max, step } => {
                        let mut n: f64 = value.parse().unwrap_or(*min);
                        if ui.add(egui::Slider::new(&mut n, *min..=*max).step_by(*step)).changed() {
                            value = format_num(n);
                            dirty = true;
                        }
                    }
                    VarControl::Color => {
                        let mut col = parse_hex(&value);
                        if ui.color_edit_button_srgb(&mut col).changed() {
                            value = to_hex(col);
                            dirty = true;
                        }
                    }
                }
                if matches!(var.control, VarControl::Select { .. } | VarControl::Wildcard { .. } | VarControl::Number { .. })
                    && ui.small_button(crate::icons::DICE).on_hover_text("Randomize this dial").clicked()
                {
                    randomize = Some(var.key.clone());
                }
                if matches!(var.control, VarControl::Wildcard { .. }) {
                    ui.label(egui::RichText::new("random each run").small().weak());
                }
            });
            self.ck_vars.insert(var.key.clone(), value);
        }

        if let Some(key) = toggle_lock {
            let now = self.ck_var_locked.get(&key).copied().unwrap_or(false);
            self.ck_var_locked.insert(key, !now);
        }
        if let Some(key) = randomize {
            if let Some(var) = vars.iter().find(|v| v.key == key) {
                if let Some(val) = random_value(&var.control) {
                    self.ck_vars.insert(key, val);
                    dirty = true;
                }
            }
        }
        if dirty {
            self.ck_dirty = true;
        }
    }

    /// Switches the active template: pre-fills the dials from the template's
    /// variable defaults (or clears them for free-form), and recomposes.
    fn set_template(&mut self, prompt_id: Option<String>) {
        self.ck_vars.clear();
        self.ck_var_locked.clear();
        if let Some(id) = &prompt_id {
            for var in ai::prompt_variables(&self.doc.project.library.ai, id) {
                self.ck_vars.insert(var.key, var.default);
            }
        }
        self.ck_prompt_id = prompt_id;
        self.ck_prompt_edited = false;
        self.ck_dirty = true;
    }

    /// Randomizes every unlocked dial that has a choice list or numeric range.
    fn surprise_me(&mut self, vars: &[pixhaus_core::project::library::composition::PromptVariable]) {
        for var in vars {
            if self.ck_var_locked.get(&var.key).copied().unwrap_or(false) {
                continue;
            }
            if let Some(val) = random_value(&var.control) {
                self.ck_vars.insert(var.key.clone(), val);
            }
        }
        self.ck_prompt_edited = false;
        self.ck_dirty = true;
    }

    /// The staged references strip — drag-in anchors with a role and weight.
    /// "More like this" on a result drops it here as a Subject reference.
    fn cockpit_references(&mut self, ui: &mut egui::Ui) {
        if self.ck_references.is_empty() {
            return;
        }
        ui.label(egui::RichText::new(format!("{} References", crate::icons::IMAGE)).strong());
        let ctx = ui.ctx().clone();
        for r in &mut self.ck_references {
            if r.texture.is_none() {
                r.texture = load_png_texture(&ctx, "ck_ref", &r.png);
            }
        }
        let mut remove: Option<usize> = None;
        egui::ScrollArea::horizontal().id_salt("ck_refs").show(ui, |ui| {
            ui.horizontal(|ui| {
                for (i, r) in self.ck_references.iter_mut().enumerate() {
                    egui::Frame::group(ui.style()).show(ui, |ui| {
                        ui.vertical(|ui| {
                            if let Some(tex) = &r.texture {
                                let size = tex.size_vec2();
                                let scale = (72.0 / size.x.max(1.0)).min(1.0);
                                ui.add(egui::Image::new((tex.id(), size * scale)));
                            }
                            ui.label(egui::RichText::new(&r.label).small());
                            egui::ComboBox::from_id_salt(("ck_ref_role", i))
                                .selected_text(role_label(r.role))
                                .show_ui(ui, |ui| {
                                    for role in [
                                        ReferenceRole::Subject,
                                        ReferenceRole::Style,
                                        ReferenceRole::Pose,
                                        ReferenceRole::Outfit,
                                        ReferenceRole::Context,
                                    ] {
                                        ui.selectable_value(&mut r.role, role, role_label(role));
                                    }
                                });
                            if ui.small_button(format!("{} remove", crate::icons::REMOVE)).clicked() {
                                remove = Some(i);
                            }
                        });
                    });
                }
            });
        });
        if let Some(i) = remove {
            self.ck_references.remove(i);
        }
    }

    /// The composed prompt — always visible and editable, not behind an advanced
    /// toggle. Recomputed live from the inputs until the artist edits it; once
    /// edited, it is sent verbatim and a reset re-syncs it to the inputs.
    fn cockpit_composed_prompt(&mut self, ui: &mut egui::Ui) {
        // A look Style picker, next to the structure picker conceptually: its
        // modifiers and look negatives fold into the live preview below.
        ui.horizontal(|ui| {
            ui.label("Style");
            let options = ai::style_options(&self.doc.project.library.ai);
            let current = self
                .ck_style_id
                .as_ref()
                .and_then(|id| options.iter().find(|(i, _)| i == id))
                .map_or_else(|| "(none)".to_owned(), |(_, n)| n.clone());
            egui::ComboBox::from_id_salt("ck_style").selected_text(current).show_ui(ui, |ui| {
                if ui.selectable_label(self.ck_style_id.is_none(), "(none)").clicked() {
                    self.ck_style_id = None;
                    self.ck_dirty = true;
                }
                for (id, name) in &options {
                    if ui.selectable_label(self.ck_style_id.as_deref() == Some(id), name).clicked() {
                        self.ck_style_id = Some(id.clone());
                        self.ck_dirty = true;
                    }
                }
            });
        });

        // Recompute the preview from the current inputs unless the artist has
        // taken the wheel by hand-editing the text. The preview is the authority:
        // every compose input (subject, structure, template, variables, style,
        // and the project style notes) marks the cockpit dirty, so this stays in
        // sync with what Generate will send.
        if self.ck_dirty && !self.ck_prompt_edited {
            let (pos, neg) = ai::compose_preview(
                &self.doc.project.library.ai,
                &self.ck_structure,
                &self.rs_prompt,
                &self.doc.project.library.ai.style_notes,
                self.ck_prompt_id.as_deref(),
                self.ck_style_id.as_deref(),
                &self.ck_vars,
            );
            self.ck_positive = pos;
            self.ck_negative = neg;
            self.ck_dirty = false;
        }

        let mut save_template = false;
        ui.horizontal(|ui| {
            ui.label(egui::RichText::new("The words we send").strong());
            if self.ck_prompt_edited {
                ui.label(egui::RichText::new("edited").small().color(ui.visuals().warn_fg_color));
                if ui
                    .small_button(format!("{} re-sync", crate::icons::UNDO))
                    .on_hover_text("Discard edits and recompose from the inputs")
                    .clicked()
                {
                    self.ck_prompt_edited = false;
                    self.ck_dirty = true;
                }
            } else {
                ui.label(egui::RichText::new("yours to change").small().weak());
            }
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui
                    .small_button(format!("{} Save as template", crate::icons::CARD))
                    .on_hover_text("Save these inputs as a reusable template in the library")
                    .clicked()
                {
                    save_template = true;
                }
            });
        });
        if save_template {
            self.save_prompt_as_template();
        }
        let pos = ui.add(
            egui::TextEdit::multiline(&mut self.ck_positive)
                .desired_rows(3)
                .desired_width(f32::INFINITY)
                .font(egui::TextStyle::Monospace),
        );
        ui.label(egui::RichText::new("Negative").small().weak());
        let neg = ui.add(
            egui::TextEdit::multiline(&mut self.ck_negative)
                .desired_rows(1)
                .desired_width(f32::INFINITY)
                .font(egui::TextStyle::Monospace),
        );
        if pos.changed() || neg.changed() {
            self.ck_prompt_edited = true;
        }
    }

    /// The single Generate action plus the run status line.
    fn cockpit_generate_row(&mut self, ui: &mut egui::Ui) {
        let running = matches!(self.rs_status, JobStatus::Running(_));
        let can_generate = self.backend_ready && !running && self.doc.active_entity_id().is_some();
        let palette = crate::theme::Palette::for_theme(ui.ctx().theme());

        let button = egui::Button::new(egui::RichText::new(format!("{}  Generate", crate::icons::SPARKLE)).color(palette.text_on_accent))
            .fill(palette.accent)
            .min_size(egui::vec2(ui.available_width(), 34.0));
        if ui.add_enabled(can_generate, button).clicked() {
            self.cockpit_generate(PendingLineage::default(), Vec::new());
        }

        match &self.rs_status {
            JobStatus::Running(msg) => {
                ui.horizontal(|ui| {
                    ui.spinner();
                    ui.label(msg);
                });
            }
            JobStatus::Failed(err) => {
                ui.colored_label(palette.error, format!("failed: {err}"));
            }
            JobStatus::Idle => {}
        }
        if self.doc.active_anchor().is_some() {
            ui.colored_label(palette.success, format!("{} anchor approved", crate::icons::CHECK));
            self.cockpit_anchor_strength(ui);
        }
    }

    /// Anchor-influence control, shown beneath the approved-anchor indicator.
    /// Tunes how strongly the approved variant conditions downstream
    /// generations (its bytes become an animation/refine anchor). The value
    /// feeds [`AnchorPayload::from_sprite_entity`] as the IP-Adapter strength.
    /// The anchor's resolution is fixed to the approved variant's own
    /// resolution; this slider governs influence, not size. The cockpit size
    /// picker ([`Self::cockpit_resolution`]) sets the generation request size
    /// that produced the variant, surfaced here as a read-only caption.
    /// Transient UI state — no undo.
    fn cockpit_anchor_strength(&mut self, ui: &mut egui::Ui) {
        ui.add(
            egui::Slider::new(&mut self.ck_anchor_strength, 0.0..=1.0)
                .text("anchor strength")
                .fixed_decimals(2),
        )
        .on_hover_text("How strongly the approved anchor conditions new generations (0 = loose, 1 = rigid)");
        // Resolving the payload at the chosen strength confirms an anchor is in
        // hand; its resolution is the canonical variant's own, not the size
        // picker above. Decode-free — the dimensions are stored on the variant.
        if self.active_anchor_payload().is_some() {
            if let Some((w, h)) = self.active_anchor_resolution() {
                ui.label(egui::RichText::new(format!("anchor resolution {w} \u{00d7} {h}")).small().weak());
            }
        }
    }

    /// The approved canonical variant's stored `(width, height)`, read without
    /// decoding the image bytes. This is the anchor's resolution — the size the
    /// approved variant was generated at, not the cockpit's current size picker.
    fn active_anchor_resolution(&self) -> Option<(u32, u32)> {
        let entity_id = self.doc.active_entity_id()?;
        let entity = self.doc.project.library.entities.iter().find(|e| e.id == entity_id)?;
        let EntityContent::Sprites {
            reference_sheet: Some(sheet),
            ..
        } = &entity.content
        else {
            return None;
        };
        let canonical = sheet.canonical.as_ref()?;
        Some((canonical.width, canonical.height))
    }

    /// Builds the [`AnchorPayload`] for the active entity's approved canonical
    /// variant at the cockpit's chosen [`Self::ck_anchor_strength`]. Returns
    /// `None` when no entity is active or its sheet has no canonical variant.
    /// `LoRA` is out of scope, so `lora_path` is always `None`. The payload's
    /// resolution is the canonical variant's own resolution; the cockpit size
    /// picker ([`Self::cockpit_resolution`]) governs the generation request
    /// size that produced that variant, not the anchor here.
    pub(crate) fn active_anchor_payload(&self) -> Option<AnchorPayload> {
        let entity_id = self.doc.active_entity_id()?;
        let entity = self.doc.project.library.entities.iter().find(|e| e.id == entity_id)?;
        anchor_payload_for(entity, self.ck_anchor_strength)
    }

    /// Recent prompts, recallable from the cockpit — past prompts are starting
    /// points, not lost keystrokes.
    fn cockpit_history(&mut self, ui: &mut egui::Ui) {
        let recent: Vec<(String, i64)> = self
            .doc
            .project
            .library
            .ai
            .prompt_history
            .iter()
            .rev()
            .take(6)
            .map(|e| (e.prompt.clone(), e.timestamp))
            .collect();
        if recent.is_empty() {
            return;
        }
        let now = now_secs();
        let mut recall: Option<String> = None;
        egui::CollapsingHeader::new(format!("{} Recent prompts", crate::icons::UNDO))
            .id_salt("ck_history")
            .show(ui, |ui| {
                for (prompt, timestamp) in &recent {
                    let label = truncate(prompt, 64);
                    ui.horizontal(|ui| {
                        if ui
                            .add(egui::Label::new(egui::RichText::new(label).small()).sense(egui::Sense::click()))
                            .on_hover_text(prompt)
                            .clicked()
                        {
                            recall = Some(prompt.clone());
                        }
                        let rel = relative_time(*timestamp, now);
                        if !rel.is_empty() {
                            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                ui.label(egui::RichText::new(rel).small().weak());
                            });
                        }
                    });
                }
            });
        if let Some(prompt) = recall {
            self.rs_prompt = prompt;
            self.ck_prompt_edited = false;
            self.ck_dirty = true;
        }
    }

    /// The variant gallery: every generation lands as a card with a thumbnail,
    /// per-card actions, and an expandable provenance panel.
    fn cockpit_gallery(&mut self, ui: &mut egui::Ui) {
        if self.rs_candidates.is_empty() {
            ui.label(egui::RichText::new("Results land here. Type a subject and Generate.").small().weak());
            if ui
                .button(format!("{} Import image…", crate::icons::UPLOAD))
                .on_hover_text("Bring an existing image in as a candidate")
                .clicked()
            {
                self.import_image_as_variant();
            }
            return;
        }
        let ctx = ui.ctx().clone();
        for cand in &mut self.rs_candidates {
            if cand.texture.is_none() {
                cand.texture = load_png_texture(&ctx, "ck_candidate", &cand.png);
            }
        }

        let mut import = false;
        ui.horizontal(|ui| {
            ui.label(egui::RichText::new(format!("Results · {}", self.rs_candidates.len())).strong());
            if ui.small_button("clear").clicked() {
                self.rs_candidates.clear();
                self.studio.anchor_selected = None;
            }
            if ui
                .small_button(format!("{} Import", crate::icons::UPLOAD))
                .on_hover_text("Bring an existing image in as a candidate")
                .clicked()
            {
                import = true;
            }
        });
        if import {
            self.import_image_as_variant();
        }
        ui.add_space(4.0);

        let accent = crate::theme::Palette::for_theme(ui.ctx().theme()).accent;
        let mut action: Option<(usize, CardAction)> = None;
        for i in 0..self.rs_candidates.len() {
            let selected = self.studio.anchor_selected == Some(i);
            let mut frame = egui::Frame::group(ui.style());
            if selected {
                frame = frame.stroke(egui::Stroke::new(2.0, accent));
            }
            frame.show(ui, |ui| {
                if let Some(act) = self.cockpit_card(ui, i) {
                    action = Some((i, act));
                }
            });
            ui.add_space(6.0);
        }

        if let Some((i, act)) = action {
            self.run_card_action(i, act);
        }
    }

    /// One result card. Returns an action the gallery applies after the borrow
    /// ends (so the action can mutate `self.rs_candidates`).
    fn cockpit_card(&mut self, ui: &mut egui::Ui, i: usize) -> Option<CardAction> {
        let mut action = None;
        let cand = &self.rs_candidates[i];
        let lineage = lineage_label(cand);

        ui.horizontal(|ui| {
            if let Some(tex) = &cand.texture {
                let size = tex.size_vec2();
                let scale = (96.0 / size.x.max(1.0)).min(1.0);
                let image = egui::Image::new((tex.id(), size * scale)).sense(egui::Sense::click());
                if ui.add(image).on_hover_text("Click to view large in the center").clicked() {
                    action = Some(CardAction::Select);
                }
            } else {
                ui.label("(decode failed)");
            }
            ui.vertical(|ui| {
                ui.label(egui::RichText::new(lineage).small().weak());
                let subject = truncate(&cand.output.user_prompt, 48);
                ui.label(egui::RichText::new(subject).small());
            });
        });

        // Action row.
        ui.horizontal_wrapped(|ui| {
            if ui
                .button(format!("{} Re-roll", crate::icons::DICE))
                .on_hover_text("Same inputs, a new seed")
                .clicked()
            {
                action = Some(CardAction::Reroll);
            }
            if ui
                .button(format!("{} More like this", crate::icons::IMAGE))
                .on_hover_text("Use this result as a Subject reference")
                .clicked()
            {
                action = Some(CardAction::MoreLikeThis);
            }
            if ui
                .button(format!("{} Refine", crate::icons::PENCIL))
                .on_hover_text("Re-run with an adjusted prompt")
                .clicked()
            {
                action = Some(CardAction::Refine);
            }
            if ui
                .button(format!("{} Branch", crate::icons::BRANCH))
                .on_hover_text("Start a new line from this result")
                .clicked()
            {
                action = Some(CardAction::Branch);
            }
            if ui
                .button(format!("{} Card", crate::icons::CARD))
                .on_hover_text("Save as a named character card in the asset library")
                .clicked()
            {
                action = Some(CardAction::SaveCard);
            }
            if ui
                .button(format!("{} Swatch", crate::icons::PALETTE))
                .on_hover_text("Save as a named style swatch in the asset library")
                .clicked()
            {
                action = Some(CardAction::SaveSwatch);
            }
            if ui
                .button(format!("{} Promote", crate::icons::PROMOTE))
                .on_hover_text("Re-render at high quality as the final, flagged as a promotion")
                .clicked()
            {
                action = Some(CardAction::Promote);
            }
            let approve = egui::Button::new(format!("{} Approve as anchor", crate::icons::ANCHOR));
            if ui.add(approve).clicked() {
                action = Some(CardAction::Approve);
            }
            // Guard the approved canonical: the gallery only holds non-canonical
            // candidates (approval moves them to `canonical`), so every card here
            // is safe to delete.
            if ui
                .button(format!("{} Delete", crate::icons::TRASH))
                .on_hover_text("Remove this candidate from the reference sheet")
                .clicked()
            {
                action = Some(CardAction::Delete);
            }
        });

        // Name for a saved card / swatch — editable before saving, so the
        // artist names the asset instead of accepting an auto-name.
        ui.horizontal(|ui| {
            ui.label(egui::RichText::new("Name").small().weak());
            ui.add(
                egui::TextEdit::singleline(&mut self.rs_candidates[i].name)
                    .hint_text("name for a saved card or swatch")
                    .desired_width(f32::INFINITY),
            );
        });

        // Provenance — on demand.
        let expanded = self.rs_candidates[i].expanded;
        let header = if expanded {
            format!("{} Hide provenance", crate::icons::INFO)
        } else {
            format!("{} Provenance", crate::icons::INFO)
        };
        if ui.small_button(header).clicked() {
            self.rs_candidates[i].expanded = !expanded;
            action = action.or(Some(CardAction::None));
        }
        if self.rs_candidates[i].expanded {
            self.cockpit_provenance(ui, i);
        }
        action
    }

    /// The provenance panel: the exact words sent, the user prompt, and the
    /// backend/model/seed/cost that produced this result.
    fn cockpit_provenance(&mut self, ui: &mut egui::Ui, i: usize) {
        let cand = &self.rs_candidates[i];
        let g = &cand.output.generation;
        egui::Frame::group(ui.style()).show(ui, |ui| {
            kv(ui, "backend", &g.backend);
            kv(ui, "model", &g.model);
            kv(ui, "seed", &g.seed.map_or_else(|| "—".to_owned(), |s| s.to_string()));
            kv(ui, "cost", &cand.cost_usd.map_or_else(|| "—".to_owned(), |c| format!("${c:.4}")));
            ui.add_space(2.0);
            ui.label(egui::RichText::new("composed prompt").small().weak());
            ui.label(egui::RichText::new(&g.prompt).small().monospace());
            if let Some(neg) = &g.negative_prompt {
                ui.label(egui::RichText::new("negative").small().weak());
                ui.label(egui::RichText::new(neg).small().monospace());
            }
            if ui.small_button(format!("{} Copy prompt", crate::icons::COPY)).clicked() {
                ui.ctx().copy_text(g.prompt.clone());
            }
        });
    }

    /// Applies a card action after the gallery's immutable borrow has ended.
    fn run_card_action(&mut self, i: usize, action: CardAction) {
        match action {
            CardAction::None => {}
            CardAction::Select => {
                self.studio.anchor_selected = Some(i);
                self.studio.anchor_view.reset();
            }
            CardAction::Reroll => {
                let parent_core = self.rs_candidates.get(i).map(|c| c.core_id);
                self.cockpit_generate(
                    PendingLineage {
                        parent: Some(i),
                        parent_core,
                        origin: VariantOrigin::FreshGeneration,
                        replace: false,
                    },
                    Vec::new(),
                );
            }
            CardAction::MoreLikeThis => {
                if let Some(cand) = self.rs_candidates.get(i) {
                    self.ck_references.push(CockpitReference {
                        label: format!("result #{}", i + 1),
                        png: cand.png.clone(),
                        role: ReferenceRole::Subject,
                        weight: 1.0,
                        texture: None,
                    });
                    let parent_core = Some(cand.core_id);
                    // The reference is now staged; cockpit_generate reads the
                    // staged set, so pass no extras to avoid duplicating it.
                    self.cockpit_generate(
                        PendingLineage {
                            parent: Some(i),
                            parent_core,
                            origin: VariantOrigin::Refinement,
                            replace: false,
                        },
                        Vec::new(),
                    );
                }
            }
            CardAction::Refine => {
                // Prompt-only refine: pre-fill the subject from this result and
                // let the artist adjust, then Generate records it as a child.
                if let Some(cand) = self.rs_candidates.get(i) {
                    self.rs_prompt = cand.output.user_prompt.clone();
                    self.ck_prompt_edited = false;
                    self.ck_dirty = true;
                    self.ck_refine_parent = Some(i);
                }
            }
            CardAction::Branch => {
                let parent_core = self.rs_candidates.get(i).map(|c| c.core_id);
                self.cockpit_generate(
                    PendingLineage {
                        parent: Some(i),
                        parent_core,
                        origin: VariantOrigin::FreshGeneration,
                        replace: false,
                    },
                    Vec::new(),
                );
            }
            CardAction::SaveCard => self.save_candidate_as_card(i),
            CardAction::SaveSwatch => self.save_candidate_as_swatch(i),
            CardAction::Approve => self.approve_candidate(i),
            CardAction::Promote => self.promote_candidate(i),
            CardAction::Delete => self.delete_candidate(i),
        }
    }

    /// Maps the staged references into verb inputs (base64 PNG + role + weight).
    fn references_as_inputs(&self) -> Vec<ReferenceInput> {
        use base64::Engine as _;
        self.ck_references
            .iter()
            .map(|r| ReferenceInput {
                image_b64: base64::engine::general_purpose::STANDARD.encode(&r.png),
                role: r.role,
                weight: r.weight,
            })
            .collect()
    }

    /// Kicks off a generation on the tokio runtime, capturing the lineage so the
    /// landing results record where they came from. Appends the subject to the
    /// project's prompt history.
    fn cockpit_generate(&mut self, lineage: PendingLineage, extra_refs: Vec<ReferenceInput>) {
        let Some(entity_id) = self.doc.active_entity_id() else {
            return;
        };
        // A from-scratch refine carries its parent from the Refine button.
        let lineage = match self.ck_refine_parent.take() {
            Some(parent) if lineage.parent.is_none() => PendingLineage {
                parent: Some(parent),
                parent_core: self.rs_candidates.get(parent).map(|c| c.core_id),
                origin: VariantOrigin::Refinement,
                replace: false,
            },
            _ => lineage,
        };

        let mut references = self.references_as_inputs();
        references.extend(extra_refs);

        let (prompt_override, negative_override) = if self.ck_prompt_edited {
            (Some(self.ck_positive.clone()), Some(self.ck_negative.clone()))
        } else {
            (None, None)
        };
        // Re-roll/branch want a fresh seed; otherwise honour the fixed-seed dial.
        let seed = if lineage.origin == VariantOrigin::FreshGeneration && lineage.parent.is_some() {
            None
        } else {
            self.ck_seed_fixed.then_some(self.ck_seed)
        };

        self.record_prompt_history();

        // Wildcard dials sample a fresh choice each run unless the artist locked
        // them — the serendipity layer.
        let mut variable_values = self.ck_vars.clone();
        if let Some(prompt_id) = &self.ck_prompt_id {
            for var in ai::prompt_variables(&self.doc.project.library.ai, prompt_id) {
                let locked = self.ck_var_locked.get(&var.key).copied().unwrap_or(false);
                if matches!(var.control, VarControl::Wildcard { .. }) && !locked {
                    if let Some(val) = random_value(&var.control) {
                        variable_values.insert(var.key, val);
                    }
                }
            }
        }

        // A promotion is a final, high-quality re-render; everything else lets
        // the router/provider pick the tier. A single candidate suffices for the
        // promotion — it is a one-shot final render of the source.
        let promotion = lineage.origin == VariantOrigin::Promotion;
        let quality = promotion.then_some(ai::ImageQuality::High);
        let num_variants = if promotion { 1 } else { self.rs_num_variants };

        let target = anchor_target_dims(self.ck_aspect, self.ck_resolution, self.ck_custom);
        let job = ai::SheetJob {
            meta: self.doc.project.metadata.clone(),
            entity_id,
            structure_id: self.ck_structure.clone(),
            prompt: self.rs_prompt.clone(),
            prompt_id: self.ck_prompt_id.clone(),
            variable_values,
            num_variants,
            style_notes: self.doc.project.library.ai.style_notes.clone(),
            structures: self.doc.project.library.ai.structures.clone(),
            styles: self.doc.project.library.ai.styles.clone(),
            prompts: self.doc.project.library.ai.prompts.clone(),
            references,
            prompt_override,
            negative_override,
            seed,
            target_size: Some(target),
            quality,
        };
        self.ck_pending = lineage;
        self.rs_status = JobStatus::Running("starting".into());
        if self.ck_pending.replace {
            self.exit_sheet_preview();
            self.rs_candidates.clear();
        }
        ai::spawn_reference_sheet(self.runtime.handle(), self.verb_runtime.clone(), self.egui_ctx.clone(), self.tx.clone(), job);
        // Play the reveal in the Anchor center viewport at the chosen output size.
        if self.reveal_effect_enabled {
            let now = self.egui_ctx.input(|i| i.time);
            self.studio.anchor_reveal.begin(target, now);
        }
    }

    /// Lands generated candidates: stamps lineage, persists them into the core
    /// reference sheet (so provenance survives a save), and adds them to the
    /// gallery. Called from the results drain.
    pub(crate) fn cockpit_on_done(&mut self, variants: Vec<ai::GeneratedVariant>, cost_usd: Option<f64>) {
        self.rs_status = JobStatus::Idle;
        // The studio shows results in the center viewport, not on the sprite
        // canvas; clear any leftover streamed-partial flag.
        self.rs_partial_preview = false;
        let lineage = std::mem::take(&mut self.ck_pending);
        if lineage.replace {
            self.rs_candidates.clear();
        }

        // Mint a core id per variant and persist as a candidate under the active
        // entity's reference sheet.
        let mut core_variants: Vec<SheetVariant> = Vec::with_capacity(variants.len());
        let mut new_cards: Vec<CockpitCandidate> = Vec::with_capacity(variants.len());
        for gv in variants {
            let core_id = SheetVariantId::new(self.doc.alloc_id());
            let mut variant = SheetVariant::from_image(
                core_id,
                gv.output.generated_at,
                ReferenceImage {
                    bytes: gv.png.clone(),
                    mime: "image/png".to_owned(),
                },
            );
            variant.user_prompt.clone_from(&gv.output.user_prompt);
            variant.composed_prompt.clone_from(&gv.output.generation.prompt);
            variant.composition = gv.output.composition.clone();
            variant.generation = Some(gv.output.generation.clone());
            variant.origin = lineage.origin;
            variant.parent_variant_id = lineage.parent_core;
            variant.cost_usd = cost_usd;
            // A promotion lands flagged and tiered as the final render so the
            // gallery and provenance can mark it, and a save preserves it.
            stamp_if_promotion(&mut variant, lineage.origin);
            core_variants.push(variant);

            let name = first_words(&gv.output.user_prompt, 4);
            new_cards.push(CockpitCandidate {
                png: gv.png,
                output: gv.output,
                cost_usd,
                parent: lineage.parent,
                origin: lineage.origin,
                core_id,
                texture: None,
                expanded: false,
                name,
            });
        }

        self.persist_variants(core_variants);
        let first_new = self.rs_candidates.len();
        self.rs_candidates.extend(new_cards);
        // Auto-select the first new result so it appears in the center viewport.
        if first_new < self.rs_candidates.len() {
            self.studio.anchor_selected = Some(first_new);
            self.studio.anchor_view.reset();
        }
    }

    /// Persists `variants` as candidates under the active entity's reference
    /// sheet, recorded as a single undoable [`crate::commands::EntityEdit`].
    fn persist_variants(&mut self, variants: Vec<SheetVariant>) {
        let Some(entity_id) = self.doc.active_entity_id() else {
            return;
        };
        crate::commands::push_library_edit(&mut self.editor, &mut self.doc, "Generate reference sheet", entity_id, move |entity| {
            if let EntityContent::Sprites { reference_sheet, .. } = &mut entity.content {
                let sheet = reference_sheet.get_or_insert_with(|| Box::new(ReferenceSheet::default()));
                // Newest first, matching the gallery order.
                for v in variants.into_iter().rev() {
                    sheet.variants.insert(0, v);
                }
            }
        });
    }

    /// Lands an inpaint-refined anchor image as a new result linked to `parent`,
    /// inheriting the parent's provenance and persisting it like a fresh variant.
    /// Stamps the structured [`RefinementKind`] captured at the start of the run
    /// (a painted mask -> `Masked`, no mask -> `PromptOnly`) onto the variant so
    /// provenance can show the refinement kind and it survives a save/reload.
    pub(crate) fn land_anchor_refine(&mut self, parent: usize, image: Vec<u8>) {
        self.rs_status = JobStatus::Idle;
        if image::load_from_memory(&image).is_err() {
            self.rs_status = JobStatus::Failed("the refined image could not be decoded".to_owned());
            return;
        }
        let (output, parent_core) = match self.rs_candidates.get(parent) {
            Some(c) => (c.output.clone(), c.core_id),
            None => return,
        };
        // A painted mask was staged at start_anchor_inpaint; an empty stage means
        // a prompt-only re-run. Either way the metadata travels on the variant.
        let refinement = self.rs_pending_refinement.take().or(Some(RefinementKind::PromptOnly));
        let core_id = SheetVariantId::new(self.doc.alloc_id());
        let variant = refined_variant(core_id, parent_core, &image, &output, refinement);
        self.persist_variants(vec![variant]);

        let name = first_words(&output.user_prompt, 4);
        let idx = self.rs_candidates.len();
        self.rs_candidates.push(CockpitCandidate {
            png: image,
            output,
            cost_usd: None,
            parent: Some(parent),
            origin: VariantOrigin::Refinement,
            core_id,
            texture: None,
            expanded: false,
            name,
        });
        self.studio.anchor_selected = Some(idx);
        self.studio.anchor_view.reset();
    }

    /// Re-renders candidate `i` at high quality as the final, flagged promotion.
    /// Uses the candidate's image as a Subject reference and runs at
    /// [`ai::ImageQuality::High`]; the landed variant is flagged
    /// `promotion = true` with `origin = VariantOrigin::Promotion`, its parent
    /// set, and `quality = Quality::High`. Raster-only — no upscale, no vector.
    /// The render lands through [`Self::cockpit_on_done`] like any generation.
    fn promote_candidate(&mut self, i: usize) {
        use base64::Engine as _;
        let Some(cand) = self.rs_candidates.get(i) else {
            return;
        };
        let png = cand.png.clone();
        // Condition the high-quality run on the source image as a Subject
        // reference so the promotion stays on-model.
        let extra = vec![ReferenceInput {
            image_b64: base64::engine::general_purpose::STANDARD.encode(&png),
            role: ReferenceRole::Subject,
            weight: 1.0,
        }];
        self.cockpit_generate(
            PendingLineage {
                parent: Some(i),
                parent_core: Some(cand.core_id),
                origin: VariantOrigin::Promotion,
                replace: false,
            },
            extra,
        );
    }

    /// Removes candidate `i` from the active entity's reference sheet and drops
    /// its gallery card. Recorded as one [`crate::commands::EntityEdit`] (the
    /// removed variant's bytes live in the `before` snapshot, so undo restores
    /// it). The gallery never holds the approved canonical — approval moves a
    /// variant into `canonical` — so this only ever retains non-canonical
    /// variants and leaves the canonical untouched.
    fn delete_candidate(&mut self, i: usize) {
        let Some(cand) = self.rs_candidates.get(i) else {
            return;
        };
        let core_id = cand.core_id;
        let Some(entity_id) = self.doc.active_entity_id() else {
            return;
        };
        crate::commands::push_library_edit(&mut self.editor, &mut self.doc, "Delete variant", entity_id, move |entity| {
            if let EntityContent::Sprites {
                reference_sheet: Some(sheet),
                ..
            } = &mut entity.content
            {
                sheet.variants.retain(|v| v.id != core_id);
            }
        });
        self.rs_candidates.remove(i);
        // Keep the selection and parent links coherent after the removal shifts
        // every later index down by one.
        fixup_selection_after_remove(&mut self.studio.anchor_selected, i);
        for cand in &mut self.rs_candidates {
            if let Some(p) = cand.parent {
                cand.parent = fixup_parent_after_remove(p, i);
            }
        }
    }

    /// Imports an existing image file as a [`VariantOrigin::ManualImport`] sheet
    /// variant under the active entity, persisted as one
    /// [`crate::commands::EntityEdit`] and surfaced as a gallery candidate.
    /// Decode-validates the bytes before minting an id; a non-image file or a
    /// cancelled dialog leaves the document unchanged.
    fn import_image_as_variant(&mut self) {
        // A brief modal pick, matching the palette import path. The bounded
        // decode below is the only real work and it is fast.
        let Some(path) = rfd::FileDialog::new()
            .set_title("Import image")
            .add_filter("Image", &["png", "jpg", "jpeg", "webp"])
            .pick_file()
        else {
            return;
        };
        let bytes = match std::fs::read(&path) {
            Ok(b) => b,
            Err(e) => {
                self.rs_status = JobStatus::Failed(format!("could not read the image: {e}"));
                return;
            }
        };
        let mime = mime_for_path(&path);
        let id = SheetVariantId::new(self.doc.alloc_id());
        let Some(variant) = imported_variant(id, now_secs(), &bytes, mime) else {
            self.rs_status = JobStatus::Failed("the file is not a supported image".to_owned());
            return;
        };
        let output = manual_import_output(variant.created_at);
        self.persist_variants(vec![variant]);

        let name = first_words(&output.user_prompt, 4);
        let idx = self.rs_candidates.len();
        self.rs_candidates.push(CockpitCandidate {
            png: bytes,
            output,
            cost_usd: None,
            parent: None,
            origin: VariantOrigin::ManualImport,
            core_id: id,
            texture: None,
            expanded: false,
            name,
        });
        self.studio.anchor_selected = Some(idx);
        self.studio.anchor_view.reset();
        self.rs_status = JobStatus::Idle;
    }

    /// Approves candidate `i` as the canonical anchor: sets it canonical in the
    /// core reference sheet and as the animation pipeline's PNG anchor.
    fn approve_candidate(&mut self, i: usize) {
        let Some(cand) = self.rs_candidates.get(i) else {
            return;
        };
        let png = cand.png.clone();
        let core_id = cand.core_id;
        let Some(entity_id) = self.doc.active_entity_id() else {
            return;
        };
        crate::commands::push_library_edit(&mut self.editor, &mut self.doc, "Approve anchor", entity_id, move |entity| {
            if let EntityContent::Sprites { reference_sheet, .. } = &mut entity.content {
                let sheet = reference_sheet.get_or_insert_with(|| Box::new(ReferenceSheet::default()));
                if let Some(pos) = sheet.variants.iter().position(|v| v.id == core_id) {
                    sheet.canonical = Some(sheet.variants.remove(pos));
                }
            }
        });
        self.exit_sheet_preview();
        self.doc.set_active_anchor(png);
        // Already in the studio (the Anchor stage); advance to the first frame,
        // which the approved anchor now conditions.
        self.studio.stage = crate::studio::StudioStage::FirstFrame;
    }

    /// Saves candidate `i` into the project asset library as a character card,
    /// building the asset shelf without leaving the cockpit. The card and its
    /// reference image land in one [`crate::commands::AiLibraryEdit`], so a
    /// single undo removes both. The artist names the card on the gallery card
    /// before saving; an empty name falls back to the subject's first words.
    fn save_candidate_as_card(&mut self, i: usize) {
        use pixhaus_core::project::AssetId;
        let Some(cand) = self.rs_candidates.get(i) else {
            return;
        };
        let png = cand.png.clone();
        let name = card_name(&cand.name, &cand.output.user_prompt);
        let style_notes = self.doc.project.library.ai.style_notes.clone();
        let asset_id = AssetId::new(self.doc.alloc_id());
        let card_id = AssetId::new(self.doc.alloc_id());
        let created_at = cand.output.generated_at;
        // Card and reference both live on `ProjectAi`, so one AiLibraryEdit
        // closure keeps the pair atomic: a single undo drops both records.
        crate::commands::push_ai_library_edit(&mut self.editor, &mut self.doc, "Save character card", move |ai| {
            push_card_records(&mut ai.asset_library, asset_id, card_id, png, name, style_notes, created_at);
        });
        self.rs_status = JobStatus::Idle;
    }

    /// Saves candidate `i` into the project asset library as a style swatch: the
    /// variant image as a [`pixhaus_core::project::ReferenceAsset`] plus a
    /// [`pixhaus_core::project::StyleSwatch`] referencing it. Both land in one
    /// [`crate::commands::AiLibraryEdit`], so a single undo removes both. `LoRA`
    /// is out of scope, so `associated_lora` is always `None`.
    fn save_candidate_as_swatch(&mut self, i: usize) {
        use pixhaus_core::project::AssetId;
        let Some(cand) = self.rs_candidates.get(i) else {
            return;
        };
        let png = cand.png.clone();
        let name = card_name(&cand.name, &cand.output.user_prompt);
        let style_notes = self.doc.project.library.ai.style_notes.clone();
        let asset_id = AssetId::new(self.doc.alloc_id());
        let swatch_id = AssetId::new(self.doc.alloc_id());
        let created_at = cand.output.generated_at;
        crate::commands::push_ai_library_edit(&mut self.editor, &mut self.doc, "Save style swatch", move |ai| {
            push_swatch_records(&mut ai.asset_library, asset_id, swatch_id, png, name, style_notes, created_at);
        });
        self.rs_status = JobStatus::Idle;
    }

    /// Appends the current subject to the project's prompt history (dedup against
    /// the most recent), stamped with the real UTC second it was issued, so it
    /// is recallable later with a meaningful time. Caps the history so it cannot
    /// grow without bound.
    fn record_prompt_history(&mut self) {
        // Cheap, non-blocking clock read on the UI thread — no worker, no lock.
        let timestamp = now_secs();
        push_prompt_history(&mut self.doc.project.library.ai.prompt_history, &self.rs_prompt, timestamp);
    }
}

/// Maximum prompt-history entries kept; older prompts age out on append.
const MAX_PROMPT_HISTORY: usize = 100;

/// Appends a trimmed, non-empty prompt to `history` stamped with `timestamp`,
/// deduping against the most recent entry and capping the history length. Pure
/// so the dedup/timestamp/cap rules are unit-testable without a full app.
fn push_prompt_history(history: &mut Vec<pixhaus_core::project::PromptHistoryEntry>, prompt: &str, timestamp: i64) {
    use pixhaus_core::project::PromptHistoryEntry;
    let prompt = prompt.trim().to_owned();
    if prompt.is_empty() {
        return;
    }
    if history.last().is_some_and(|e| e.prompt == prompt) {
        return;
    }
    history.push(PromptHistoryEntry {
        verb_name: "generate_reference_sheet".to_owned(),
        prompt,
        timestamp,
    });
    if history.len() > MAX_PROMPT_HISTORY {
        let drop = history.len() - MAX_PROMPT_HISTORY;
        history.drain(0..drop);
    }
}

/// UTC seconds since the epoch, for stamping prompt-history and other
/// UI-thread-issued records. A cheap `SystemTime::now` read; returns 0 if the
/// clock is before the epoch.
fn now_secs() -> i64 {
    use std::time::{SystemTime, UNIX_EPOCH};
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| i64::try_from(d.as_secs()).unwrap_or(i64::MAX))
}

/// A short relative-time caption for a UTC-second timestamp, e.g. "2m ago".
/// Falls back to "just now" for non-positive deltas and to a day count for
/// older entries. Returns an empty string for a zero (unstamped) timestamp.
fn relative_time(timestamp: i64, now: i64) -> String {
    if timestamp <= 0 {
        return String::new();
    }
    let delta = now.saturating_sub(timestamp);
    if delta < 60 {
        "just now".to_owned()
    } else if delta < 3600 {
        format!("{}m ago", delta / 60)
    } else if delta < 86_400 {
        format!("{}h ago", delta / 3600)
    } else {
        format!("{}d ago", delta / 86_400)
    }
}

/// A per-card action, resolved after the gallery's borrow ends.
#[derive(Clone, Copy)]
enum CardAction {
    None,
    Select,
    Reroll,
    MoreLikeThis,
    Refine,
    Branch,
    SaveCard,
    /// Save this candidate as a reusable style swatch in the asset library.
    SaveSwatch,
    Approve,
    /// Re-render this candidate at high quality as a flagged promotion.
    Promote,
    /// Remove this candidate from the reference sheet.
    Delete,
}

/// A short lineage caption for a card ("fresh", "re-roll of #1", …).
/// Output aspect ratio for a free-form anchor. The cockpit pairs this with a
/// long-edge resolution to resolve the request size (see [`anchor_target_dims`]).
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub(crate) enum AnchorAspect {
    /// 1:1.
    Square,
    /// 4:3 landscape.
    Landscape,
    /// 3:4 portrait.
    Portrait,
    /// 16:9 widescreen.
    Wide,
    /// 9:16 tall.
    Tall,
    /// Explicit width/height dragged by the artist.
    Custom,
}

impl AnchorAspect {
    /// The pickable aspects, in display order.
    pub(crate) const ALL: [Self; 6] = [Self::Square, Self::Landscape, Self::Portrait, Self::Wide, Self::Tall, Self::Custom];

    /// The dropdown label.
    pub(crate) fn label(self) -> &'static str {
        match self {
            Self::Square => "1:1",
            Self::Landscape => "4:3",
            Self::Portrait => "3:4",
            Self::Wide => "16:9",
            Self::Tall => "9:16",
            Self::Custom => "Custom",
        }
    }
}

/// Resolves the anchor target `(width, height)` from the aspect, the long-edge
/// resolution, and the custom dims. Non-custom aspects scale `long_edge` to the
/// ratio; the backend rounds to /16 and clamps the result, so this stays exact.
fn anchor_target_dims(aspect: AnchorAspect, long_edge: u32, custom: (u32, u32)) -> (u32, u32) {
    let short = |num: u32, den: u32| long_edge * num / den;
    match aspect {
        AnchorAspect::Square => (long_edge, long_edge),
        AnchorAspect::Landscape => (long_edge, short(3, 4)),
        AnchorAspect::Portrait => (short(3, 4), long_edge),
        AnchorAspect::Wide => (long_edge, short(9, 16)),
        AnchorAspect::Tall => (short(9, 16), long_edge),
        AnchorAspect::Custom => custom,
    }
}

fn lineage_label(cand: &CockpitCandidate) -> String {
    match (cand.origin, cand.parent) {
        (VariantOrigin::FreshGeneration, Some(p)) => format!("branch of #{}", p + 1),
        (VariantOrigin::FreshGeneration, None) => "fresh".to_owned(),
        (VariantOrigin::Refinement, Some(p)) => format!("refine of #{}", p + 1),
        (VariantOrigin::Refinement, None) => "refine".to_owned(),
        _ => "result".to_owned(),
    }
}

/// Human label for a reference role.
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

/// A muted-key / value row used in the provenance panel.
fn kv(ui: &mut egui::Ui, key: &str, value: &str) {
    ui.horizontal(|ui| {
        ui.label(egui::RichText::new(key).small().weak());
        ui.label(egui::RichText::new(value).small().monospace());
    });
}

/// Truncates `s` to at most `max` chars, appending an ellipsis when cut.
fn truncate(s: &str, max: usize) -> String {
    if s.chars().count() <= max {
        return s.to_owned();
    }
    let cut: String = s.chars().take(max.saturating_sub(1)).collect();
    format!("{cut}…")
}

/// The first `n` words of `s`, for a default card name.
fn first_words(s: &str, n: usize) -> String {
    let name: Vec<&str> = s.split_whitespace().take(n).collect();
    if name.is_empty() { "Untitled card".to_owned() } else { name.join(" ") }
}

/// Resolves the name to save an asset under: the artist's typed `name` when it
/// has content, otherwise the subject's first words. Pure so the fallback rule
/// is unit-testable.
fn card_name(name: &str, subject: &str) -> String {
    let trimmed = name.trim();
    if trimmed.is_empty() { first_words(subject, 4) } else { trimmed.to_owned() }
}

/// Writes a character card and its source reference image into `lib`, with the
/// card referencing the asset. Pure so the card/reference pairing is testable
/// without a full app; the caller wraps it in one undo entry.
fn push_card_records(
    lib: &mut pixhaus_core::project::AssetLibrary,
    asset_id: pixhaus_core::project::AssetId,
    card_id: pixhaus_core::project::AssetId,
    png: Vec<u8>,
    name: String,
    style_notes: String,
    created_at: i64,
) {
    use pixhaus_core::project::{CharacterCard, ReferenceAsset};
    lib.references.push(ReferenceAsset {
        id: asset_id,
        image: ReferenceImage {
            bytes: png,
            mime: "image/png".to_owned(),
        },
        default_role: ReferenceRole::Subject,
        tags: Vec::new(),
        source_variant_id: None,
        created_at,
    });
    lib.character_cards.push(CharacterCard {
        id: card_id,
        name,
        references: vec![asset_id],
        style_notes,
        associated_lora: None,
        created_at,
    });
}

/// Writes a style swatch and its source reference image into `lib`, with the
/// swatch referencing the asset. `LoRA` is out of scope, so `associated_lora`
/// is `None`. Pure so the swatch/reference pairing is testable without a full
/// app; the caller wraps it in one undo entry.
fn push_swatch_records(
    lib: &mut pixhaus_core::project::AssetLibrary,
    asset_id: pixhaus_core::project::AssetId,
    swatch_id: pixhaus_core::project::AssetId,
    png: Vec<u8>,
    name: String,
    style_notes: String,
    created_at: i64,
) {
    use pixhaus_core::project::{ReferenceAsset, StyleSwatch};
    lib.references.push(ReferenceAsset {
        id: asset_id,
        image: ReferenceImage {
            bytes: png,
            mime: "image/png".to_owned(),
        },
        default_role: ReferenceRole::Style,
        tags: Vec::new(),
        source_variant_id: None,
        created_at,
    });
    lib.style_swatches.push(StyleSwatch {
        id: swatch_id,
        name,
        references: vec![asset_id],
        style_notes,
        associated_lora: None,
        created_at,
    });
}

/// A random value for a typed control: a choice for Select/Wildcard, a snapped
/// value in range for Number. `None` for controls with nothing to randomize.
fn random_value(control: &VarControl) -> Option<String> {
    match control {
        VarControl::Select { choices } | VarControl::Wildcard { choices } => {
            if choices.is_empty() {
                return None;
            }
            Some(choices[rand_index(choices.len())].clone())
        }
        VarControl::Number { min, max, step } => {
            let span = (max - min).max(0.0);
            let steps = if *step > 0.0 { (span / step).floor() as usize + 1 } else { 1 };
            let n = min + (rand_index(steps.max(1)) as f64) * step.max(f64::EPSILON);
            Some(format_num(n.min(*max)))
        }
        VarControl::Text | VarControl::Color => None,
    }
}

/// A cheap, dependency-free pseudo-random index in `0..len`. Good enough for
/// dice rolls in the cockpit — not for anything that needs real randomness.
fn rand_index(len: usize) -> usize {
    use std::cell::Cell;
    use std::time::{SystemTime, UNIX_EPOCH};
    thread_local!(static STATE: Cell<u64> = const { Cell::new(0x9e37_79b9_7f4a_7c15) });
    let seed = SystemTime::now().duration_since(UNIX_EPOCH).map_or(0, |d| d.as_nanos() as u64);
    STATE.with(|s| {
        // xorshift mixed with the clock so consecutive calls differ.
        let mut x = s.get() ^ seed;
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        s.set(x);
        if len == 0 { 0 } else { (x % len as u64) as usize }
    })
}

/// Formats a dial number compactly: integral values drop the decimal point.
fn format_num(n: f64) -> String {
    if (n.fract()).abs() < f64::EPSILON {
        format!("{}", n as i64)
    } else {
        format!("{n:.2}")
    }
}

/// Parses a `#rrggbb` (or `rrggbb`) hex string to an sRGB triple, defaulting to
/// mid-grey on a parse failure.
fn parse_hex(s: &str) -> [u8; 3] {
    let h = s.trim().trim_start_matches('#');
    if h.len() == 6 {
        if let Ok(v) = u32::from_str_radix(h, 16) {
            return [(v >> 16) as u8, (v >> 8) as u8, v as u8];
        }
    }
    [128, 128, 128]
}

/// Formats an sRGB triple as a `#rrggbb` hex string.
fn to_hex(c: [u8; 3]) -> String {
    format!("#{:02x}{:02x}{:02x}", c[0], c[1], c[2])
}

/// Decodes PNG bytes into a NEAREST-sampled egui texture (pixel art).
fn load_png_texture(ctx: &egui::Context, name: &str, png: &[u8]) -> Option<egui::TextureHandle> {
    let rgba = image::load_from_memory(png).ok()?.to_rgba8();
    let size = [rgba.width() as usize, rgba.height() as usize];
    let color = egui::ColorImage::from_rgba_unmultiplied(size, rgba.as_raw());
    Some(ctx.load_texture(name, color, egui::TextureOptions::NEAREST))
}

/// Builds a refined [`SheetVariant`] from a repainted image, inheriting `parent`'s
/// provenance and stamping the structured `refinement` metadata. Pure so the
/// refinement-kind wiring is unit-testable without a full app.
fn refined_variant(id: SheetVariantId, parent: SheetVariantId, image: &[u8], output: &SheetVariantOutput, refinement: Option<RefinementKind>) -> SheetVariant {
    let mut variant = SheetVariant::from_image(
        id,
        output.generated_at,
        ReferenceImage {
            bytes: image.to_vec(),
            mime: "image/png".to_owned(),
        },
    );
    variant.user_prompt.clone_from(&output.user_prompt);
    variant.composed_prompt.clone_from(&output.generation.prompt);
    variant.composition = output.composition.clone();
    variant.generation = Some(output.generation.clone());
    variant.origin = VariantOrigin::Refinement;
    variant.parent_variant_id = Some(parent);
    variant.refinement = refinement;
    variant.cost_usd = None;
    variant
}

/// Builds a [`VariantOrigin::ManualImport`] variant from image bytes, returning
/// `None` when the bytes are not a decodable image. Pure and tested; the caller
/// owns id minting and persistence.
fn imported_variant(id: SheetVariantId, created_at: i64, bytes: &[u8], mime: &str) -> Option<SheetVariant> {
    // Validate before committing: a non-image file must not land as a variant.
    image::load_from_memory(bytes).ok()?;
    Some(SheetVariant::from_image(
        id,
        created_at,
        ReferenceImage {
            bytes: bytes.to_vec(),
            mime: mime.to_owned(),
        },
    ))
}

/// A minimal gallery output for an imported image: no prompt, empty composition,
/// and a `manual-import` provenance so the card and provenance panel render.
fn manual_import_output(created_at: i64) -> SheetVariantOutput {
    SheetVariantOutput {
        id: SheetVariantId::new(0),
        generated_at: created_at,
        image_b64: String::new(),
        user_prompt: "imported image".to_owned(),
        composition: pixhaus_core::project::SheetComposition::default(),
        generation: GenerationProvenance {
            backend: "manual-import".to_owned(),
            model: String::new(),
            prompt: String::new(),
            seed: None,
            negative_prompt: None,
        },
    }
}

/// The image MIME hint for a picked file, by extension. Defaults to PNG, which
/// `SheetVariant::from_image` only uses as a hint — the real bytes drive decode.
fn mime_for_path(path: &std::path::Path) -> &'static str {
    match path.extension().and_then(|e| e.to_str()).map(str::to_ascii_lowercase).as_deref() {
        Some("jpg" | "jpeg") => "image/jpeg",
        Some("webp") => "image/webp",
        _ => "image/png",
    }
}

/// Adjusts a gallery selection index after the candidate at `removed` is dropped.
/// The selected card shifts down by one past the removal; the removed card itself
/// clears the selection. Pure so the index bookkeeping is testable.
fn fixup_selection_after_remove(selected: &mut Option<usize>, removed: usize) {
    match *selected {
        Some(sel) if sel == removed => *selected = None,
        Some(sel) if sel > removed => *selected = Some(sel - 1),
        _ => {}
    }
}

/// Adjusts a card's parent index after the candidate at `removed` is dropped.
/// A child of the removed card loses its parent link; later parents shift down.
fn fixup_parent_after_remove(parent: usize, removed: usize) -> Option<usize> {
    match parent {
        p if p == removed => None,
        p if p > removed => Some(p - 1),
        p => Some(p),
    }
}

/// Flags a variant as a final promotion when its origin is
/// [`VariantOrigin::Promotion`]: sets `promotion` and the high quality tier so
/// the gallery, provenance, and a save all reflect it. A no-op for any other
/// origin. Pure so the promotion stamping is testable without a full app.
fn stamp_if_promotion(variant: &mut SheetVariant, origin: VariantOrigin) {
    if origin == VariantOrigin::Promotion {
        variant.promotion = true;
        variant.quality = pixhaus_core::project::Quality::High;
    }
}

/// Builds the anchor payload for `entity` at the chosen `strength`, passing
/// `None` for `lora_path` (`LoRA` is out of scope). Returns `None` when the
/// entity has no approved canonical variant. Pure so the strength wiring is
/// unit-testable without a full app; `from_sprite_entity` itself clamps the
/// value to `0.0..=1.0`.
fn anchor_payload_for(entity: &pixhaus_core::project::Entity, strength: f32) -> Option<AnchorPayload> {
    AnchorPayload::from_sprite_entity(entity, strength, None)
}

#[cfg(test)]
mod tests {
    use super::{
        AnchorAspect, anchor_target_dims, fixup_parent_after_remove, fixup_selection_after_remove, imported_variant, manual_import_output, now_secs,
        push_prompt_history, refined_variant, relative_time, stamp_if_promotion,
    };

    #[test]
    fn anchor_target_dims_scales_each_aspect_to_the_long_edge() {
        assert_eq!(anchor_target_dims(AnchorAspect::Square, 1536, (0, 0)), (1536, 1536));
        assert_eq!(anchor_target_dims(AnchorAspect::Landscape, 1536, (0, 0)), (1536, 1152));
        assert_eq!(anchor_target_dims(AnchorAspect::Portrait, 1536, (0, 0)), (1152, 1536));
        assert_eq!(anchor_target_dims(AnchorAspect::Wide, 1536, (0, 0)), (1536, 864));
        assert_eq!(anchor_target_dims(AnchorAspect::Tall, 1536, (0, 0)), (864, 1536));
    }

    #[test]
    fn anchor_target_dims_passes_custom_through_verbatim() {
        assert_eq!(anchor_target_dims(AnchorAspect::Custom, 1536, (640, 1280)), (640, 1280));
    }

    #[test]
    fn now_secs_is_a_real_post_epoch_timestamp() {
        // Pixhaus v2's first commit is well after 2020, so any sane clock yields
        // a value past this floor. Guards against the old hardcoded 0.
        assert!(now_secs() > 1_577_836_800, "now_secs returns real UTC seconds");
    }

    #[test]
    fn record_prompt_history_stamps_a_real_timestamp() {
        let mut history = Vec::new();
        push_prompt_history(&mut history, "a small knight", 1_700_000_000);
        assert_eq!(history.len(), 1);
        assert_eq!(history[0].prompt, "a small knight");
        assert_ne!(history[0].timestamp, 0, "the entry carries a real timestamp, not 0");
        assert_eq!(history[0].timestamp, 1_700_000_000);
    }

    #[test]
    fn record_prompt_history_dedups_consecutive_identical_prompts() {
        let mut history = Vec::new();
        push_prompt_history(&mut history, "a knight", 1);
        push_prompt_history(&mut history, "  a knight  ", 2);
        assert_eq!(history.len(), 1, "the trimmed identical prompt is not appended again");
        push_prompt_history(&mut history, "a dragon", 3);
        assert_eq!(history.len(), 2, "a different prompt appends");
    }

    #[test]
    fn record_prompt_history_skips_blank_prompts() {
        let mut history = Vec::new();
        push_prompt_history(&mut history, "   ", 1);
        assert!(history.is_empty());
    }

    #[test]
    fn record_prompt_history_caps_the_length() {
        let mut history = Vec::new();
        for i in 0..super::MAX_PROMPT_HISTORY + 50 {
            push_prompt_history(&mut history, &format!("prompt {i}"), i as i64);
        }
        assert_eq!(history.len(), super::MAX_PROMPT_HISTORY, "history is capped");
        // The oldest entries aged out; the newest survives.
        assert_eq!(history.last().map(|e| e.prompt.as_str()), Some("prompt 149"));
    }

    #[test]
    fn relative_time_reads_as_human_deltas() {
        assert_eq!(relative_time(0, 1_000), "", "an unstamped entry shows no time");
        assert_eq!(relative_time(1_000, 1_010), "just now");
        assert_eq!(relative_time(1_000, 1_000 + 120), "2m ago");
        assert_eq!(relative_time(1_000, 1_000 + 7_200), "2h ago");
        assert_eq!(relative_time(1_000, 1_000 + 2 * 86_400), "2d ago");
    }

    use pixhaus_core::project::{
        EntityContent, GenerationProvenance, Quality, ReferenceImage, ReferenceSheet, RefinementKind, SheetVariant, SheetVariantId, Size, VariantOrigin,
    };

    use crate::commands::push_library_edit;
    use crate::document::DocumentStore;
    use crate::editor::EditorState;

    /// A tiny valid PNG (`w` x `h`, solid red) for decode-validating helpers.
    fn tiny_png(w: u32, h: u32) -> Vec<u8> {
        use std::io::Cursor;
        let img = image::RgbaImage::from_pixel(w, h, image::Rgba([200, 0, 0, 255]));
        let mut out = Vec::new();
        image::DynamicImage::ImageRgba8(img)
            .write_to(&mut Cursor::new(&mut out), image::ImageFormat::Png)
            .expect("encode test png");
        out
    }

    /// A gallery output standing in for the refine source's provenance.
    fn fake_output(prompt: &str, created_at: i64) -> super::SheetVariantOutput {
        super::SheetVariantOutput {
            id: SheetVariantId::new(0),
            generated_at: created_at,
            image_b64: String::new(),
            user_prompt: prompt.to_owned(),
            composition: pixhaus_core::project::SheetComposition::default(),
            generation: GenerationProvenance {
                backend: "openai".to_owned(),
                model: "gpt-image-2".to_owned(),
                prompt: format!("{prompt}, pixel art"),
                seed: Some(7),
                negative_prompt: None,
            },
        }
    }

    // ── Task 5: structured masked / regional refinement metadata ──────────────

    #[test]
    fn refined_variant_records_a_masked_refinement_with_round_tripping_mask_bytes() {
        let parent = SheetVariantId::new(1);
        let id = SheetVariantId::new(2);
        let image = tiny_png(32, 32);
        let output = fake_output("a knight", 1_700_000_000);
        let mask_bytes = vec![9, 8, 7, 6];
        let refinement = Some(RefinementKind::Masked {
            mask_png: ReferenceImage {
                bytes: mask_bytes.clone(),
                mime: "image/png".to_owned(),
            },
        });

        let variant = refined_variant(id, parent, &image, &output, refinement);

        assert_eq!(variant.origin, VariantOrigin::Refinement);
        assert_eq!(variant.parent_variant_id, Some(parent));
        assert_eq!(variant.composed_prompt, "a knight, pixel art", "inherits the parent's composed prompt");
        match variant.refinement {
            Some(RefinementKind::Masked { mask_png }) => assert_eq!(mask_png.bytes, mask_bytes, "the mask bytes round-trip onto the variant"),
            other => panic!("expected a masked refinement, got {other:?}"),
        }
    }

    #[test]
    fn refined_variant_with_no_mask_records_prompt_only() {
        let variant = refined_variant(
            SheetVariantId::new(2),
            SheetVariantId::new(1),
            &tiny_png(16, 16),
            &fake_output("a dragon", 1),
            Some(RefinementKind::PromptOnly),
        );
        assert_eq!(variant.refinement, Some(RefinementKind::PromptOnly));
    }

    // ── Task 6: promote variant to final ──────────────────────────────────────

    #[test]
    fn stamp_if_promotion_flags_only_promotion_variants() {
        let mut promoted = SheetVariant::from_image(
            SheetVariantId::new(3),
            0,
            ReferenceImage {
                bytes: tiny_png(8, 8),
                mime: "image/png".to_owned(),
            },
        );
        promoted.origin = VariantOrigin::Promotion;
        promoted.parent_variant_id = Some(SheetVariantId::new(1));
        stamp_if_promotion(&mut promoted, VariantOrigin::Promotion);
        assert!(promoted.promotion, "a promotion is flagged");
        assert_eq!(promoted.quality, Quality::High, "a promotion renders at the high tier");
        assert_eq!(promoted.origin, VariantOrigin::Promotion);
        assert_eq!(promoted.parent_variant_id, Some(SheetVariantId::new(1)), "the source link is kept");

        let mut fresh = SheetVariant::from_image(
            SheetVariantId::new(4),
            0,
            ReferenceImage {
                bytes: tiny_png(8, 8),
                mime: "image/png".to_owned(),
            },
        );
        stamp_if_promotion(&mut fresh, VariantOrigin::FreshGeneration);
        assert!(!fresh.promotion, "a fresh generation is not flagged a promotion");
    }

    // ── Task 7: import existing image as a sheet variant ──────────────────────

    #[test]
    fn imported_variant_decodes_dimensions_and_defaults_to_manual_import() {
        let variant = imported_variant(SheetVariantId::new(5), 1_700_000_000, &tiny_png(48, 24), "image/png").expect("valid image imports");
        assert_eq!((variant.width, variant.height), (48, 24), "real encoded dimensions are read");
        assert_eq!(variant.origin, VariantOrigin::ManualImport);
        assert_eq!(variant.created_at, 1_700_000_000);
    }

    #[test]
    fn imported_variant_rejects_non_image_bytes() {
        assert!(
            imported_variant(SheetVariantId::new(6), 0, b"not an image at all", "image/png").is_none(),
            "garbage bytes do not import"
        );
    }

    #[test]
    fn manual_import_output_carries_manual_import_provenance() {
        let output = manual_import_output(42);
        assert_eq!(output.generated_at, 42);
        assert_eq!(output.generation.backend, "manual-import");
        assert!(output.image_b64.is_empty());
    }

    #[test]
    fn importing_bytes_appends_a_manual_import_variant_under_one_undo() {
        let mut doc = DocumentStore::new();
        doc.create_sprite("hero", Size::new(16, 16));
        let entity_id = doc.active_entity_id().expect("active entity");
        let mut editor = EditorState::default();

        let id = SheetVariantId::new(doc.alloc_id());
        let variant = imported_variant(id, 1_700_000_000, &tiny_png(20, 20), "image/png").expect("import");
        // Mirror persist_variants: insert newest-first under the active entity.
        push_library_edit(&mut editor, &mut doc, "Generate reference sheet", entity_id, move |entity| {
            if let EntityContent::Sprites { reference_sheet, .. } = &mut entity.content {
                let sheet = reference_sheet.get_or_insert_with(|| Box::new(ReferenceSheet::default()));
                sheet.variants.insert(0, variant);
            }
        });

        assert_eq!(sheet_variants(&doc, entity_id).len(), 1, "the import appended one variant");
        assert_eq!(sheet_variants(&doc, entity_id)[0].origin, VariantOrigin::ManualImport);
        editor.history.undo(&mut doc).expect("undo");
        assert!(sheet_variants(&doc, entity_id).is_empty(), "undo removes the imported variant");
    }

    // ── Task 8: delete / remove sheet variant ─────────────────────────────────

    #[test]
    fn deleting_a_variant_removes_it_and_undo_restores_it() {
        let mut doc = DocumentStore::new();
        doc.create_sprite("hero", Size::new(16, 16));
        let entity_id = doc.active_entity_id().expect("active entity");
        let mut editor = EditorState::default();

        let keep = SheetVariantId::new(doc.alloc_id());
        let drop = SheetVariantId::new(doc.alloc_id());
        let keep_variant = imported_variant(keep, 1, &tiny_png(8, 8), "image/png").expect("keep");
        let drop_variant = imported_variant(drop, 2, &tiny_png(8, 8), "image/png").expect("drop");
        push_library_edit(&mut editor, &mut doc, "Generate reference sheet", entity_id, move |entity| {
            if let EntityContent::Sprites { reference_sheet, .. } = &mut entity.content {
                let sheet = reference_sheet.get_or_insert_with(|| Box::new(ReferenceSheet::default()));
                sheet.variants = vec![keep_variant, drop_variant];
            }
        });
        assert_eq!(sheet_variants(&doc, entity_id).len(), 2);

        // The delete path: retain variants by id, dropping the target.
        push_library_edit(&mut editor, &mut doc, "Delete variant", entity_id, move |entity| {
            if let EntityContent::Sprites {
                reference_sheet: Some(sheet),
                ..
            } = &mut entity.content
            {
                sheet.variants.retain(|v| v.id != drop);
            }
        });

        let after = sheet_variants(&doc, entity_id);
        assert_eq!(after.len(), 1, "the targeted variant is removed");
        assert_eq!(after[0].id, keep, "the other variant survives");

        editor.history.undo(&mut doc).expect("undo");
        let restored = sheet_variants(&doc, entity_id);
        assert_eq!(restored.len(), 2, "undo restores the deleted variant");
        assert!(restored.iter().any(|v| v.id == drop), "the dropped variant is back by id");
    }

    // ── Task 9: anchor payload strength control ───────────────────────────────

    #[test]
    fn chosen_anchor_strength_reaches_the_payload_constructor() {
        use super::anchor_payload_for;

        let mut doc = DocumentStore::new();
        doc.create_sprite("hero", Size::new(16, 16));
        let entity_id = doc.active_entity_id().expect("active entity");
        let mut editor = EditorState::default();

        // Approve a canonical variant so the entity carries an anchor.
        let canonical = SheetVariant::from_image(
            SheetVariantId::new(doc.alloc_id()),
            1,
            ReferenceImage {
                bytes: tiny_png(24, 24),
                mime: "image/png".to_owned(),
            },
        );
        push_library_edit(&mut editor, &mut doc, "Approve anchor", entity_id, move |entity| {
            if let EntityContent::Sprites { reference_sheet, .. } = &mut entity.content {
                let sheet = reference_sheet.get_or_insert_with(|| Box::new(ReferenceSheet::default()));
                sheet.canonical = Some(canonical);
            }
        });

        let entity = doc.project.library.entities.iter().find(|e| e.id == entity_id).expect("entity");

        // The cockpit's chosen strength threads straight through to the payload.
        let payload = anchor_payload_for(entity, 0.42).expect("canonical anchor yields a payload");
        assert!((payload.strength - 0.42).abs() < f32::EPSILON, "the chosen strength reaches the constructor");
        assert!(payload.lora_path.is_none(), "LoRA is out of scope, so lora_path is None");

        // The constructor clamps out-of-range values to the unit interval.
        let clamped = anchor_payload_for(entity, 5.0).expect("payload");
        assert!((clamped.strength - 1.0).abs() < f32::EPSILON, "an over-range strength clamps to 1.0");
    }

    #[test]
    fn anchor_payload_for_returns_none_without_a_canonical() {
        use super::anchor_payload_for;

        let mut doc = DocumentStore::new();
        doc.create_sprite("hero", Size::new(16, 16));
        let entity_id = doc.active_entity_id().expect("active entity");
        let entity = doc.project.library.entities.iter().find(|e| e.id == entity_id).expect("entity");
        assert!(anchor_payload_for(entity, 0.7).is_none(), "no approved canonical means no anchor payload");
    }

    // ── Tasks 11 & 12: save-as-card / save-as-swatch, undoable as a unit ───────

    use super::{card_name, push_card_records, push_swatch_records};
    use crate::commands::push_ai_library_edit;
    use pixhaus_core::project::{AssetId, ReferenceRole};

    #[test]
    fn card_name_prefers_the_typed_name_and_falls_back_to_the_subject() {
        assert_eq!(card_name("Hero knight", "a small knight"), "Hero knight", "a typed name wins");
        assert_eq!(card_name("  spaced  ", "ignored"), "spaced", "the typed name is trimmed");
        assert_eq!(card_name("   ", "a small knight"), "a small knight", "a blank name falls back to the subject's first words");
        assert_eq!(card_name("", ""), "Untitled card", "no name and no subject falls back to the default");
    }

    #[test]
    fn saving_a_card_adds_one_card_and_one_reference_under_one_undo() {
        let mut doc = DocumentStore::new();
        doc.create_sprite("hero", Size::new(16, 16));
        let mut editor = EditorState::default();

        let asset_id = AssetId::new(doc.alloc_id());
        let card_id = AssetId::new(doc.alloc_id());
        let png = tiny_png(20, 20);
        push_ai_library_edit(&mut editor, &mut doc, "Save character card", move |ai| {
            push_card_records(&mut ai.asset_library, asset_id, card_id, png, "Hero knight".to_owned(), "house style".to_owned(), 1_700_000_000);
        });

        let lib = &doc.project.library.ai.asset_library;
        assert_eq!(lib.character_cards.len(), 1, "the save adds one character card");
        assert_eq!(lib.references.len(), 1, "the save adds one reference image");
        let card = &lib.character_cards[0];
        assert_eq!(card.name, "Hero knight");
        assert_eq!(card.references, vec![asset_id], "the card references its source image");
        assert_eq!(card.style_notes, "house style");
        assert!(card.associated_lora.is_none(), "LoRA is out of scope");
        assert_eq!(lib.references[0].id, asset_id);
        assert_eq!(lib.references[0].default_role, ReferenceRole::Subject);

        editor.history.undo(&mut doc).expect("undo");
        let lib = &doc.project.library.ai.asset_library;
        assert!(lib.character_cards.is_empty(), "one undo removes the card");
        assert!(lib.references.is_empty(), "the same undo removes the reference");
    }

    #[test]
    fn saving_a_swatch_adds_one_swatch_and_one_reference_under_one_undo() {
        let mut doc = DocumentStore::new();
        doc.create_sprite("hero", Size::new(16, 16));
        let mut editor = EditorState::default();

        let asset_id = AssetId::new(doc.alloc_id());
        let swatch_id = AssetId::new(doc.alloc_id());
        let png = tiny_png(20, 20);
        push_ai_library_edit(&mut editor, &mut doc, "Save style swatch", move |ai| {
            push_swatch_records(&mut ai.asset_library, asset_id, swatch_id, png, "Dusk palette".to_owned(), "muted dusk".to_owned(), 1_700_000_000);
        });

        let lib = &doc.project.library.ai.asset_library;
        assert_eq!(lib.style_swatches.len(), 1, "the save adds one style swatch");
        assert_eq!(lib.references.len(), 1, "the save adds one reference image");
        let swatch = &lib.style_swatches[0];
        assert_eq!(swatch.name, "Dusk palette");
        assert_eq!(swatch.references, vec![asset_id], "the swatch references its source image");
        assert_eq!(swatch.style_notes, "muted dusk");
        assert!(swatch.associated_lora.is_none(), "LoRA is out of scope");
        assert!(lib.character_cards.is_empty(), "a swatch save does not touch cards");
        assert_eq!(lib.references[0].id, asset_id);
        assert_eq!(lib.references[0].default_role, ReferenceRole::Style);

        editor.history.undo(&mut doc).expect("undo");
        let lib = &doc.project.library.ai.asset_library;
        assert!(lib.style_swatches.is_empty(), "one undo removes the swatch");
        assert!(lib.references.is_empty(), "the same undo removes the reference");
    }

    #[test]
    fn fixup_selection_after_remove_shifts_or_clears() {
        let mut sel = Some(2);
        fixup_selection_after_remove(&mut sel, 0);
        assert_eq!(sel, Some(1), "a later selection shifts down by one");

        let mut sel = Some(1);
        fixup_selection_after_remove(&mut sel, 1);
        assert_eq!(sel, None, "removing the selected card clears the selection");

        let mut sel = Some(0);
        fixup_selection_after_remove(&mut sel, 2);
        assert_eq!(sel, Some(0), "an earlier selection is unchanged");
    }

    #[test]
    fn fixup_parent_after_remove_shifts_or_orphans() {
        assert_eq!(fixup_parent_after_remove(3, 1), Some(2), "a later parent shifts down");
        assert_eq!(fixup_parent_after_remove(1, 1), None, "a child of the removed card is orphaned");
        assert_eq!(fixup_parent_after_remove(0, 2), Some(0), "an earlier parent is unchanged");
    }

    /// The active entity's reference-sheet variants, for the persistence tests.
    fn sheet_variants(doc: &DocumentStore, entity_id: pixhaus_core::project::EntityId) -> Vec<SheetVariant> {
        doc.project
            .library
            .entities
            .iter()
            .find(|e| e.id == entity_id)
            .and_then(|e| match &e.content {
                EntityContent::Sprites { reference_sheet, .. } => reference_sheet.as_ref().map(|s| s.variants.clone()),
                _ => None,
            })
            .unwrap_or_default()
    }
}
