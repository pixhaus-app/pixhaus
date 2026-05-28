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
use pixhaus_ai::verbs::reference_sheet::{ReferenceInput, SheetVariantOutput};
use pixhaus_core::project::library::composition::VarControl;
use pixhaus_core::project::{EntityContent, ReferenceImage, ReferenceRole, ReferenceSheet, SheetVariant, SheetVariantId, VariantOrigin};

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
            ui.add(
                egui::TextEdit::multiline(notes)
                    .hint_text("project house style — palette, era, line weight…")
                    .desired_rows(2)
                    .desired_width(f32::INFINITY),
            );
        });
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

        ui.add(egui::Slider::new(&mut self.rs_num_variants, 1..=4).text("variants"));

        ui.horizontal(|ui| {
            ui.checkbox(&mut self.ck_seed_fixed, "Fixed seed")
                .on_hover_text("Pin the seed for a reproducible result; off draws a fresh seed each run");
            ui.add_enabled(self.ck_seed_fixed, egui::DragValue::new(&mut self.ck_seed));
        });
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
        // Recompute the preview from the current inputs unless the artist has
        // taken the wheel by hand-editing the text.
        if self.ck_dirty && !self.ck_prompt_edited {
            let (pos, neg) = ai::compose_preview(
                &self.doc.project.library.ai,
                &self.ck_structure,
                &self.rs_prompt,
                &self.doc.project.library.ai.style_notes,
                self.ck_prompt_id.as_deref(),
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
        }
    }

    /// Recent prompts, recallable from the cockpit — past prompts are starting
    /// points, not lost keystrokes.
    fn cockpit_history(&mut self, ui: &mut egui::Ui) {
        let recent: Vec<String> = self
            .doc
            .project
            .library
            .ai
            .prompt_history
            .iter()
            .rev()
            .take(6)
            .map(|e| e.prompt.clone())
            .collect();
        if recent.is_empty() {
            return;
        }
        egui::CollapsingHeader::new(format!("{} Recent prompts", crate::icons::UNDO))
            .id_salt("ck_history")
            .show(ui, |ui| {
                for prompt in recent {
                    let label = truncate(&prompt, 64);
                    if ui
                        .add(egui::Label::new(egui::RichText::new(label).small()).sense(egui::Sense::click()))
                        .on_hover_text(&prompt)
                        .clicked()
                    {
                        self.rs_prompt = prompt;
                        self.ck_prompt_edited = false;
                        self.ck_dirty = true;
                    }
                }
            });
    }

    /// The variant gallery: every generation lands as a card with a thumbnail,
    /// per-card actions, and an expandable provenance panel.
    fn cockpit_gallery(&mut self, ui: &mut egui::Ui) {
        if self.rs_candidates.is_empty() {
            ui.label(egui::RichText::new("Results land here. Type a subject and Generate.").small().weak());
            return;
        }
        let ctx = ui.ctx().clone();
        for cand in &mut self.rs_candidates {
            if cand.texture.is_none() {
                cand.texture = load_png_texture(&ctx, "ck_candidate", &cand.png);
            }
        }

        ui.horizontal(|ui| {
            ui.label(egui::RichText::new(format!("Results · {}", self.rs_candidates.len())).strong());
            if ui.small_button("clear").clicked() {
                self.rs_candidates.clear();
                self.studio.anchor_selected = None;
            }
        });
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
                .on_hover_text("Save as a character card in the asset library")
                .clicked()
            {
                action = Some(CardAction::SaveCard);
            }
            let approve = egui::Button::new(format!("{} Approve as anchor", crate::icons::ANCHOR));
            if ui.add(approve).clicked() {
                action = Some(CardAction::Approve);
            }
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
            CardAction::Approve => self.approve_candidate(i),
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

        let job = ai::SheetJob {
            meta: self.doc.project.metadata.clone(),
            entity_id,
            structure_id: self.ck_structure.clone(),
            prompt: self.rs_prompt.clone(),
            prompt_id: self.ck_prompt_id.clone(),
            variable_values,
            num_variants: self.rs_num_variants,
            style_notes: self.doc.project.library.ai.style_notes.clone(),
            structures: self.doc.project.library.ai.structures.clone(),
            styles: self.doc.project.library.ai.styles.clone(),
            prompts: self.doc.project.library.ai.prompts.clone(),
            references,
            prompt_override,
            negative_override,
            seed,
        };
        self.ck_pending = lineage;
        self.rs_status = JobStatus::Running("starting".into());
        if self.ck_pending.replace {
            self.exit_sheet_preview();
            self.rs_candidates.clear();
        }
        ai::spawn_reference_sheet(self.runtime.handle(), self.verb_runtime.clone(), self.egui_ctx.clone(), self.tx.clone(), job);
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
            core_variants.push(variant);

            new_cards.push(CockpitCandidate {
                png: gv.png,
                output: gv.output,
                cost_usd,
                parent: lineage.parent,
                origin: lineage.origin,
                core_id,
                texture: None,
                expanded: false,
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
        let core_id = SheetVariantId::new(self.doc.alloc_id());
        let mut variant = SheetVariant::from_image(
            core_id,
            output.generated_at,
            ReferenceImage {
                bytes: image.clone(),
                mime: "image/png".to_owned(),
            },
        );
        variant.user_prompt.clone_from(&output.user_prompt);
        variant.composed_prompt.clone_from(&output.generation.prompt);
        variant.composition = output.composition.clone();
        variant.generation = Some(output.generation.clone());
        variant.origin = VariantOrigin::Refinement;
        variant.parent_variant_id = Some(parent_core);
        variant.cost_usd = None;
        self.persist_variants(vec![variant]);

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
        });
        self.studio.anchor_selected = Some(idx);
        self.studio.anchor_view.reset();
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
    /// building the asset shelf without leaving the cockpit.
    fn save_candidate_as_card(&mut self, i: usize) {
        use pixhaus_core::project::{AssetId, CharacterCard, ReferenceAsset};
        let Some(cand) = self.rs_candidates.get(i) else {
            return;
        };
        let png = cand.png.clone();
        let name = first_words(&cand.output.user_prompt, 4);
        let Some(entity_id) = self.doc.active_entity_id() else {
            return;
        };
        let asset_id = AssetId::new(self.doc.alloc_id());
        let card_id = AssetId::new(self.doc.alloc_id());
        let created_at = cand.output.generated_at;
        crate::commands::push_library_edit(&mut self.editor, &mut self.doc, "Save character card", entity_id, move |_entity| {});
        // Card and reference live at the project level, not on the entity, so we
        // write them directly (no per-entity undo needed for the asset shelf).
        let lib = &mut self.doc.project.library.ai.asset_library;
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
            style_notes: self.doc.project.library.ai.style_notes.clone(),
            associated_lora: None,
            created_at,
        });
        self.rs_status = JobStatus::Idle;
    }

    /// Appends the current subject to the project's prompt history (dedup against
    /// the most recent), so it is recallable later.
    fn record_prompt_history(&mut self) {
        use pixhaus_core::project::PromptHistoryEntry;
        let prompt = self.rs_prompt.trim().to_owned();
        if prompt.is_empty() {
            return;
        }
        let history = &mut self.doc.project.library.ai.prompt_history;
        if history.last().is_some_and(|e| e.prompt == prompt) {
            return;
        }
        history.push(PromptHistoryEntry {
            verb_name: "generate_reference_sheet".to_owned(),
            prompt,
            timestamp: 0,
        });
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
    Approve,
}

/// A short lineage caption for a card ("fresh", "re-roll of #1", …).
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
