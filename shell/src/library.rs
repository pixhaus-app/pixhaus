//! The composition library: a central Create-mode browser over saved prompt
//! templates, structures, and styles, plus the right-inspector editor for the
//! selected record.
//!
//! Built-ins (`BuiltinLibrary`) are read-only and layer *under* project records;
//! a project record with the same id shadows its built-in. Editing a built-in
//! therefore clones it into the project first. Every mutation routes through
//! [`crate::commands::push_ai_library_edit`] so add / edit / delete land on the
//! undo stack and persist with the project.

use eframe::egui;
use pixhaus_ai::compose::builtins::BuiltinLibrary;
use pixhaus_core::project::ProjectAi;
use pixhaus_core::project::library::ai::{ModelId, Quality};
use pixhaus_core::project::library::composition::{
    Dimensions, PanelRect, PanelSlot, PromptId, PromptTemplate, PromptVariable, Structure, StructureId, StructureOutput, StructurePanel, Style, StyleId,
    VarControl,
};
use pixhaus_core::project::library::pixstyle::{ConflictPolicy, StylePack, merge_pack, read_pack, write_pack};

use crate::app::{ShellApp, ShellMsg};
use crate::commands::push_ai_library_edit;

/// Which kind of composition record the library browser is showing.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum LibraryTab {
    Templates,
    Structures,
    Styles,
}

/// A composition record open in the editor. Editing mutates this owned draft;
/// Save upserts it into the project library by id, so unsaved edits never touch
/// the live project until committed.
pub(crate) enum LibraryDraft {
    Template(PromptTemplate),
    Structure(Structure),
    Style(Style),
}

impl LibraryDraft {
    /// The record's stable id, used to highlight the matching browser row.
    fn id(&self) -> &str {
        match self {
            LibraryDraft::Template(p) => &p.id.0,
            LibraryDraft::Structure(s) => &s.id.0,
            LibraryDraft::Style(s) => &s.id.0,
        }
    }
}

/// One browser row: a resolved record plus whether it is a project record
/// (editable / deletable) or a read-only built-in.
struct Row {
    id: String,
    name: String,
    is_project: bool,
}

impl ShellApp {
    /// The central composition-library browser: a tab selector, a New button,
    /// and the record list for the active tab.
    pub(crate) fn library_view(&mut self, ui: &mut egui::Ui) {
        ui.add_space(4.0);
        ui.heading(format!("{} Composition library", crate::icons::LIBRARY));
        ui.label(
            egui::RichText::new("Saved prompts, structures, and styles for this project. Built-ins are read-only — duplicate one to make it yours.")
                .small()
                .weak(),
        );
        ui.add_space(6.0);

        ui.horizontal(|ui| {
            ui.selectable_value(&mut self.library_tab, LibraryTab::Templates, "Templates");
            ui.selectable_value(&mut self.library_tab, LibraryTab::Structures, "Structures");
            ui.selectable_value(&mut self.library_tab, LibraryTab::Styles, "Styles");
            ui.add_space(8.0);
            if ui.button(format!("{} New", crate::icons::ADD)).clicked() {
                self.library_new();
            }
            if ui
                .button(format!("{} Export pack", crate::icons::DOWNLOAD))
                .on_hover_text("Save this project's prompts, structures, and styles to a .pixstyle pack")
                .clicked()
            {
                self.library_export_pack();
            }
            if ui
                .button(format!("{} Import pack", crate::icons::UPLOAD))
                .on_hover_text("Merge composition records from a .pixstyle pack into this project")
                .clicked()
            {
                self.library_import_pack();
            }
        });
        ui.separator();

        let rows = self.library_rows();
        let selected = self.library_draft.as_ref().map(|d| d.id().to_owned());
        let mut open: Option<String> = None;
        let mut duplicate: Option<String> = None;
        let mut delete: Option<String> = None;

        egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
            if rows.is_empty() {
                ui.label(egui::RichText::new("No records yet. Click New to add one.").weak());
            }
            for row in &rows {
                ui.horizontal(|ui| {
                    let is_sel = selected.as_deref() == Some(row.id.as_str());
                    if ui.selectable_label(is_sel, &row.name).clicked() {
                        open = Some(row.id.clone());
                    }
                    if !row.is_project {
                        ui.label(egui::RichText::new("built-in").small().weak());
                    }
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if row.is_project && ui.small_button(crate::icons::TRASH).on_hover_text("Delete this record").clicked() {
                            delete = Some(row.id.clone());
                        }
                        if ui
                            .small_button(crate::icons::COPY)
                            .on_hover_text("Duplicate into a new project record")
                            .clicked()
                        {
                            duplicate = Some(row.id.clone());
                        }
                    });
                });
            }
        });

        if let Some(id) = open {
            self.library_open(&id);
        }
        if let Some(id) = duplicate {
            self.library_duplicate(&id);
        }
        if let Some(id) = delete {
            self.library_delete(&id);
        }
    }

    /// The merged record list for the active tab: project records first, then
    /// any built-ins they don't shadow, sorted by display name.
    fn library_rows(&self) -> Vec<Row> {
        let builtins = BuiltinLibrary::load();
        let ai = &self.doc.project.library.ai;
        let mut rows: Vec<Row> = Vec::new();
        match self.library_tab {
            LibraryTab::Templates => {
                for p in &ai.prompts {
                    rows.push(Row {
                        id: p.id.0.clone(),
                        name: p.name.clone(),
                        is_project: true,
                    });
                }
                for (id, p) in &builtins.prompts {
                    if !ai.prompts.iter().any(|x| x.id == *id) {
                        rows.push(Row {
                            id: id.0.clone(),
                            name: p.name.clone(),
                            is_project: false,
                        });
                    }
                }
            }
            LibraryTab::Structures => {
                for s in &ai.structures {
                    rows.push(Row {
                        id: s.id.0.clone(),
                        name: s.name.clone(),
                        is_project: true,
                    });
                }
                for (id, s) in &builtins.structures {
                    if !ai.structures.iter().any(|x| x.id == *id) {
                        rows.push(Row {
                            id: id.0.clone(),
                            name: s.name.clone(),
                            is_project: false,
                        });
                    }
                }
            }
            LibraryTab::Styles => {
                for s in &ai.styles {
                    rows.push(Row {
                        id: s.id.0.clone(),
                        name: s.name.clone(),
                        is_project: true,
                    });
                }
                for (id, s) in &builtins.styles {
                    if !ai.styles.iter().any(|x| x.id == *id) {
                        rows.push(Row {
                            id: id.0.clone(),
                            name: s.name.clone(),
                            is_project: false,
                        });
                    }
                }
            }
        }
        rows.sort_by(|a, b| a.name.cmp(&b.name));
        rows
    }

    /// Loads the record `id` (project copy if present, else the built-in) into
    /// the editor draft for the active tab.
    fn library_open(&mut self, id: &str) {
        let builtins = BuiltinLibrary::load();
        let ai = &self.doc.project.library.ai;
        self.library_draft = match self.library_tab {
            LibraryTab::Templates => ai
                .prompts
                .iter()
                .find(|p| p.id.0 == id)
                .cloned()
                .or_else(|| builtins.prompts.get(&PromptId(id.to_owned())).cloned())
                .map(LibraryDraft::Template),
            LibraryTab::Structures => ai
                .structures
                .iter()
                .find(|s| s.id.0 == id)
                .cloned()
                .or_else(|| builtins.structures.get(&StructureId(id.to_owned())).cloned())
                .map(LibraryDraft::Structure),
            LibraryTab::Styles => ai
                .styles
                .iter()
                .find(|s| s.id.0 == id)
                .cloned()
                .or_else(|| builtins.styles.get(&StyleId(id.to_owned())).cloned())
                .map(LibraryDraft::Style),
        };
    }

    /// Opens a blank draft of the active tab's type with a fresh project id.
    fn library_new(&mut self) {
        let n = self.doc.alloc_id();
        self.library_draft = Some(match self.library_tab {
            LibraryTab::Templates => LibraryDraft::Template(PromptTemplate {
                id: PromptId(format!("project.prompt.{n}")),
                name: "New template".to_owned(),
                text: String::new(),
                variables: Vec::new(),
                default_style: None,
                default_structure: None,
            }),
            LibraryTab::Structures => LibraryDraft::Structure(Structure {
                id: StructureId(format!("project.structure.{n}")),
                name: "New structure".to_owned(),
                output: StructureOutput::Single,
                layout_negatives: String::new(),
            }),
            LibraryTab::Styles => LibraryDraft::Style(Style {
                id: StyleId(format!("project.style.{n}")),
                name: "New style".to_owned(),
                modifiers: String::new(),
                look_negatives: String::new(),
                model_pref: None,
                quality: None,
            }),
        });
    }

    /// Loads a copy of record `id` under a fresh project id into the draft, so
    /// Save inserts a new record rather than shadowing the original.
    fn library_duplicate(&mut self, id: &str) {
        self.library_open(id);
        let n = self.doc.alloc_id();
        match self.library_draft.as_mut() {
            Some(LibraryDraft::Template(p)) => {
                p.id = PromptId(format!("project.prompt.{n}"));
                p.name = format!("{} copy", p.name);
            }
            Some(LibraryDraft::Structure(s)) => {
                s.id = StructureId(format!("project.structure.{n}"));
                s.name = format!("{} copy", s.name);
            }
            Some(LibraryDraft::Style(s)) => {
                s.id = StyleId(format!("project.style.{n}"));
                s.name = format!("{} copy", s.name);
            }
            None => {}
        }
    }

    /// Removes the project record `id` from the active tab's collection.
    fn library_delete(&mut self, id: &str) {
        let tab = self.library_tab;
        let owned = id.to_owned();
        push_ai_library_edit(&mut self.editor, &mut self.doc, "Delete library record", |ai| match tab {
            LibraryTab::Templates => ai.prompts.retain(|p| p.id.0 != owned),
            LibraryTab::Structures => ai.structures.retain(|s| s.id.0 != owned),
            LibraryTab::Styles => ai.styles.retain(|s| s.id.0 != owned),
        });
        if self.library_draft.as_ref().is_some_and(|d| d.id() == id) {
            self.library_draft = None;
        }
    }

    /// Upserts the current draft into the project library by id.
    fn library_save(&mut self) {
        let Some(draft) = self.library_draft.as_ref() else {
            return;
        };
        match draft {
            LibraryDraft::Template(p) => {
                let value = p.clone();
                push_ai_library_edit(&mut self.editor, &mut self.doc, "Save prompt template", |ai| {
                    upsert(&mut ai.prompts, value, |x| &x.id);
                });
            }
            LibraryDraft::Structure(s) => {
                let value = s.clone();
                push_ai_library_edit(&mut self.editor, &mut self.doc, "Save structure", |ai| {
                    upsert(&mut ai.structures, value, |x| &x.id);
                });
            }
            LibraryDraft::Style(s) => {
                let value = s.clone();
                push_ai_library_edit(&mut self.editor, &mut self.doc, "Save style", |ai| upsert(&mut ai.styles, value, |x| &x.id));
            }
        }
    }

    /// The right-inspector editor for the open draft. Edits an owned copy; Save
    /// upserts it, Revert reloads from the library, Delete removes a project
    /// record. Renders a hint when nothing is selected.
    pub(crate) fn library_editor(&mut self, ui: &mut egui::Ui) {
        let Some(mut draft) = self.library_draft.take() else {
            ui.add_space(8.0);
            ui.label(egui::RichText::new("Select a record on the left, or click New.").weak());
            return;
        };
        // Id/name choices for the template dropdowns, read before the draft edit.
        let structure_choices = self.library_choices(LibraryTab::Structures);
        let style_choices = self.library_choices(LibraryTab::Styles);
        let id = draft.id().to_owned();
        let is_project = self.library_record_is_project(&draft);

        let mut save = false;
        let mut revert = false;
        let mut delete = false;

        egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
            match &mut draft {
                LibraryDraft::Template(p) => edit_template(ui, p, &structure_choices, &style_choices),
                LibraryDraft::Structure(s) => edit_structure(ui, s),
                LibraryDraft::Style(s) => edit_style(ui, s),
            }
            ui.add_space(10.0);
            ui.separator();
            ui.horizontal(|ui| {
                if ui.button(format!("{} Save", crate::icons::CHECK)).clicked() {
                    save = true;
                }
                if ui.button(format!("{} Revert", crate::icons::UNDO)).clicked() {
                    revert = true;
                }
                if is_project && ui.button(format!("{} Delete", crate::icons::TRASH)).clicked() {
                    delete = true;
                }
            });
        });

        // Restore the (edited) draft, then act on the captured intent.
        self.library_draft = Some(draft);
        if save {
            self.library_save();
        } else if revert {
            self.library_open(&id);
        } else if delete {
            self.library_delete(&id);
        }
    }

    /// Whether a project record (not just a built-in) exists for `draft`'s id.
    fn library_record_is_project(&self, draft: &LibraryDraft) -> bool {
        let ai = &self.doc.project.library.ai;
        match draft {
            LibraryDraft::Template(p) => ai.prompts.iter().any(|x| x.id == p.id),
            LibraryDraft::Structure(s) => ai.structures.iter().any(|x| x.id == s.id),
            LibraryDraft::Style(s) => ai.styles.iter().any(|x| x.id == s.id),
        }
    }

    /// `(id, name)` of every record of `tab`'s type (project + unshadowed
    /// built-ins), name-sorted, for the template default-structure/style pickers.
    fn library_choices(&self, tab: LibraryTab) -> Vec<(String, String)> {
        let builtins = BuiltinLibrary::load();
        let ai = &self.doc.project.library.ai;
        let mut out: Vec<(String, String)> = Vec::new();
        match tab {
            LibraryTab::Templates => {
                for p in &ai.prompts {
                    out.push((p.id.0.clone(), p.name.clone()));
                }
                for (id, p) in &builtins.prompts {
                    if !ai.prompts.iter().any(|x| x.id == *id) {
                        out.push((id.0.clone(), p.name.clone()));
                    }
                }
            }
            LibraryTab::Structures => {
                for s in &ai.structures {
                    out.push((s.id.0.clone(), s.name.clone()));
                }
                for (id, s) in &builtins.structures {
                    if !ai.structures.iter().any(|x| x.id == *id) {
                        out.push((id.0.clone(), s.name.clone()));
                    }
                }
            }
            LibraryTab::Styles => {
                for s in &ai.styles {
                    out.push((s.id.0.clone(), s.name.clone()));
                }
                for (id, s) in &builtins.styles {
                    if !ai.styles.iter().any(|x| x.id == *id) {
                        out.push((id.0.clone(), s.name.clone()));
                    }
                }
            }
        }
        out.sort_by(|a, b| a.1.cmp(&b.1));
        out
    }

    /// Captures the cockpit's current subject, structure, and dial values as a
    /// new project prompt template, then opens the library on it to name and
    /// refine. The library is reached *from* creating, not visited up front.
    pub(crate) fn save_prompt_as_template(&mut self) {
        let n = self.doc.alloc_id();
        let trimmed = self.rs_prompt.trim();
        let name = if trimmed.is_empty() {
            "Saved template".to_owned()
        } else {
            trimmed.chars().take(40).collect::<String>()
        };
        let default_structure = (self.ck_structure != crate::ai::SINGLE_STRUCTURE_ID).then(|| StructureId(self.ck_structure.clone()));
        let variables = self
            .ck_vars
            .iter()
            .map(|(key, value)| PromptVariable {
                key: key.clone(),
                label: key.clone(),
                default: value.clone(),
                control: VarControl::Text,
            })
            .collect();
        let template = PromptTemplate {
            id: PromptId(format!("project.prompt.{n}")),
            name,
            text: self.rs_prompt.clone(),
            variables,
            default_style: None,
            default_structure,
        };
        let value = template.clone();
        push_ai_library_edit(&mut self.editor, &mut self.doc, "Save prompt as template", |ai| {
            upsert(&mut ai.prompts, value, |x| &x.id);
        });
        self.library_tab = LibraryTab::Templates;
        self.library_draft = Some(LibraryDraft::Template(template));
        self.studio_library_open = true;
    }

    /// Exports this project's composition records to a `.pixstyle` pack: every
    /// project record plus the built-ins the project does not shadow, so the
    /// pack carries the artist's complete working library and not just their
    /// edits. The save dialog and the encode/write run on a worker — the rfd
    /// dialog blocks, and v2's egui loop is synchronous, so doing it inline
    /// would freeze the window. The gathered pack is owned (no borrow crosses
    /// the dialog); the result lands as a status line off the channel. Export
    /// never mutates the document.
    pub(crate) fn library_export_pack(&mut self) {
        let pack = gather_export_pack(&self.doc.project.library.ai, &BuiltinLibrary::load());
        if pack.structures.is_empty() && pack.styles.is_empty() && pack.prompts.is_empty() {
            self.set_status("Nothing to export: the library is empty");
            return;
        }
        let tx = self.tx.clone();
        let ctx = self.egui_ctx.clone();
        // spawn_blocking: the rfd dialog and the file write are blocking I/O,
        // kept off the egui thread. The pack is moved in, so no document borrow
        // outlives the dialog.
        self.runtime.handle().spawn_blocking(move || {
            let result = run_export(&pack);
            let _ = tx.send(ShellMsg::PackExportDone { path: result });
            ctx.request_repaint();
        });
    }

    /// Opens the conflict-policy modal for a `.pixstyle` import. The artist picks
    /// how colliding ids resolve (skip / overwrite / import-as-copy) before any
    /// file is read; confirming spawns the decode worker. Seeds the modal with
    /// [`ConflictPolicy::Skip`], the non-destructive default.
    pub(crate) fn library_import_pack(&mut self) {
        self.pack_import_policy = Some(ConflictPolicy::Skip);
    }

    /// The conflict-policy modal shown before a `.pixstyle` import. Renders the
    /// three policies as radio choices; Import spawns the decode worker with the
    /// chosen policy and closes the modal, Cancel just closes it.
    pub(crate) fn show_pack_import_dialog(&mut self, ctx: &egui::Context) {
        let Some(mut policy) = self.pack_import_policy else {
            return;
        };
        let mut open = true;
        let mut confirm = false;
        egui::Window::new(format!("{} Import composition pack", crate::icons::UPLOAD))
            .collapsible(false)
            .resizable(false)
            .open(&mut open)
            .show(ctx, |ui| {
                ui.label(egui::RichText::new("When a pack record shares an id with this project:").small().weak());
                ui.add_space(4.0);
                for (option, label, hint) in CONFLICT_POLICY_CHOICES {
                    ui.radio_value(&mut policy, option, label).on_hover_text(hint);
                }
                ui.add_space(8.0);
                ui.separator();
                ui.horizontal(|ui| {
                    if ui.button(format!("{} Import", crate::icons::CHECK)).clicked() {
                        confirm = true;
                    }
                    if ui.button("Cancel").clicked() {
                        self.pack_import_policy = None;
                    }
                });
            });
        // Keep live radio edits even before Import, until the modal closes.
        if self.pack_import_policy.is_some() {
            self.pack_import_policy = Some(policy);
        }
        if !open {
            self.pack_import_policy = None;
        }
        if confirm {
            self.pack_import_policy = None;
            self.spawn_pack_import(policy);
        }
    }

    /// Spawns the `.pixstyle` import worker: the open dialog, read, and decode
    /// run off the egui thread (rfd blocks; v2's loop is synchronous). The
    /// decoded pack returns over the channel and merges on the UI thread, so the
    /// merge is one undoable [`crate::commands::AiLibraryEdit`].
    fn spawn_pack_import(&mut self, policy: ConflictPolicy) {
        let tx = self.tx.clone();
        let ctx = self.egui_ctx.clone();
        self.runtime.handle().spawn_blocking(move || {
            let pack = run_import_decode();
            let _ = tx.send(ShellMsg::PackImportDecoded { pack, policy });
            ctx.request_repaint();
        });
    }

    /// Merges a decoded pack into the project's composition tier under `policy`,
    /// as one undoable [`crate::commands::AiLibraryEdit`], and surfaces the
    /// imported / skipped / overwritten counts. Called on the UI thread after the
    /// import worker decodes the pack.
    pub(crate) fn apply_imported_pack(&mut self, pack: StylePack, policy: ConflictPolicy) {
        let report = merge_pack_into_project(&mut self.editor, &mut self.doc, pack, policy);
        self.set_status(format!(
            "Imported pack: {} added, {} skipped, {} overwritten",
            report.imported, report.skipped, report.overwritten
        ));
    }
}

/// Merges `pack` into a project's composition tier under `policy` as one
/// undoable [`crate::commands::AiLibraryEdit`], returning the merge report. The
/// document-touching half of the import, split out so the undo wiring is
/// testable without a full [`ShellApp`].
fn merge_pack_into_project(
    editor: &mut crate::editor::EditorState,
    doc: &mut crate::document::DocumentStore,
    pack: StylePack,
    policy: ConflictPolicy,
) -> pixhaus_core::project::ImportReport {
    let mut report = pixhaus_core::project::ImportReport::default();
    push_ai_library_edit(editor, doc, "Import composition pack", |ai| {
        report = merge_pack(ai, pack, policy);
    });
    report
}

/// Gathers a project's composition records into an exportable [`StylePack`]:
/// every project record, plus the built-ins the project does not shadow. Pure
/// so the "project + unshadowed built-ins" rule is testable without a worker or
/// a dialog.
fn gather_export_pack(ai: &ProjectAi, builtins: &BuiltinLibrary) -> StylePack {
    let mut structures = ai.structures.clone();
    for (id, s) in &builtins.structures {
        if !ai.structures.iter().any(|x| x.id == *id) {
            structures.push(s.clone());
        }
    }
    let mut styles = ai.styles.clone();
    for (id, s) in &builtins.styles {
        if !ai.styles.iter().any(|x| x.id == *id) {
            styles.push(s.clone());
        }
    }
    let mut prompts = ai.prompts.clone();
    for (id, p) in &builtins.prompts {
        if !ai.prompts.iter().any(|x| x.id == *id) {
            prompts.push(p.clone());
        }
    }
    StylePack {
        format_version: 1,
        structures,
        styles,
        prompts,
    }
}

/// The three conflict policies, with their modal labels and hover hints, in the
/// order the import modal lists them. `Skip` leads as the non-destructive
/// default.
const CONFLICT_POLICY_CHOICES: [(ConflictPolicy, &str, &str); 3] = [
    (ConflictPolicy::Skip, "Skip", "Keep this project's record; drop the incoming one"),
    (ConflictPolicy::Overwrite, "Overwrite", "Replace this project's record with the incoming one"),
    (ConflictPolicy::ImportAsCopy, "Import as copy", "Add the incoming record under a fresh .copy id"),
];

/// Opens a `.pixstyle` save dialog and writes `pack` to the chosen path. Runs on
/// a worker. Returns the written path, `None` for a cancelled dialog, or an
/// error string. Never touches the document.
fn run_export(pack: &StylePack) -> Result<Option<std::path::PathBuf>, String> {
    let Some(path) = rfd::FileDialog::new()
        .set_title("Export composition pack")
        .set_file_name("composition.pixstyle")
        .add_filter("Composition pack", &["pixstyle"])
        .save_file()
    else {
        return Ok(None);
    };
    let file = std::fs::File::create(&path).map_err(|e| format!("could not create the file: {e}"))?;
    write_pack(pack, std::io::BufWriter::new(file)).map_err(|e| e.to_string())?;
    Ok(Some(path))
}

/// Opens a `.pixstyle` open dialog and decodes the picked pack. Runs on a
/// worker. Returns the decoded pack, `None` for a cancelled dialog, or an error
/// string. The decode caps guard against an oversized or malicious bundle.
fn run_import_decode() -> Result<Option<StylePack>, String> {
    let Some(path) = rfd::FileDialog::new()
        .set_title("Import composition pack")
        .add_filter("Composition pack", &["pixstyle"])
        .pick_file()
    else {
        return Ok(None);
    };
    let file = std::fs::File::open(&path).map_err(|e| format!("could not open the file: {e}"))?;
    let pack = read_pack(std::io::BufReader::new(file)).map_err(|e| e.to_string())?;
    Ok(Some(pack))
}

/// Edits a prompt template: name, text, default structure/style, and variables.
fn edit_template(ui: &mut egui::Ui, p: &mut PromptTemplate, structures: &[(String, String)], styles: &[(String, String)]) {
    ui.label("Name");
    ui.add(egui::TextEdit::singleline(&mut p.name).desired_width(f32::INFINITY));
    ui.add_space(4.0);
    ui.label("Prompt text — use {key} for each variable");
    ui.add(egui::TextEdit::multiline(&mut p.text).desired_rows(3).desired_width(f32::INFINITY));
    ui.add_space(6.0);

    let mut structure = p.default_structure.as_ref().map(|s| s.0.clone());
    if optional_picker(ui, "Default structure", &mut structure, structures) {
        p.default_structure = structure.map(StructureId);
    }
    let mut style = p.default_style.as_ref().map(|s| s.0.clone());
    if optional_picker(ui, "Default style", &mut style, styles) {
        p.default_style = style.map(StyleId);
    }

    ui.add_space(8.0);
    ui.label(egui::RichText::new("Variables").strong());
    let mut remove: Option<usize> = None;
    for (i, var) in p.variables.iter_mut().enumerate() {
        egui::Frame::group(ui.style()).show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.label("key");
                ui.add(egui::TextEdit::singleline(&mut var.key).desired_width(80.0));
                ui.label("label");
                ui.add(egui::TextEdit::singleline(&mut var.label).desired_width(110.0));
                if ui.small_button(crate::icons::TRASH).on_hover_text("Remove variable").clicked() {
                    remove = Some(i);
                }
            });
            ui.horizontal(|ui| {
                ui.label("default");
                ui.add(egui::TextEdit::singleline(&mut var.default).desired_width(120.0));
                control_kind_picker(ui, &mut var.control, i);
            });
            match &mut var.control {
                VarControl::Select { choices } | VarControl::Wildcard { choices } => {
                    ui.label(egui::RichText::new("choices — one per line").small().weak());
                    let mut joined = choices.join("\n");
                    if ui
                        .add(egui::TextEdit::multiline(&mut joined).desired_rows(2).desired_width(f32::INFINITY))
                        .changed()
                    {
                        *choices = joined.lines().map(|s| s.trim().to_owned()).filter(|s| !s.is_empty()).collect();
                    }
                }
                VarControl::Number { min, max, step } => {
                    ui.horizontal(|ui| {
                        ui.label("min");
                        ui.add(egui::DragValue::new(min));
                        ui.label("max");
                        ui.add(egui::DragValue::new(max));
                        ui.label("step");
                        ui.add(egui::DragValue::new(step).speed(0.1));
                    });
                }
                VarControl::Text | VarControl::Color => {}
            }
        });
    }
    if let Some(i) = remove {
        p.variables.remove(i);
    }
    if ui.button(format!("{} Add variable", crate::icons::ADD)).clicked() {
        p.variables.push(PromptVariable {
            key: format!("var{}", p.variables.len() + 1),
            label: "Variable".to_owned(),
            default: String::new(),
            control: VarControl::Text,
        });
    }
}

/// Edits a structure: name, layout negatives, and (when paneled) a canvas size
/// and a minimal panel list.
fn edit_structure(ui: &mut egui::Ui, s: &mut Structure) {
    ui.label("Name");
    ui.add(egui::TextEdit::singleline(&mut s.name).desired_width(f32::INFINITY));
    ui.add_space(4.0);
    ui.label("Layout negatives");
    ui.add(egui::TextEdit::multiline(&mut s.layout_negatives).desired_rows(2).desired_width(f32::INFINITY));
    ui.add_space(6.0);

    let mut paneled = matches!(s.output, StructureOutput::Paneled { .. });
    if ui.checkbox(&mut paneled, "Paneled (multi-panel sheet)").changed() {
        s.output = if paneled {
            StructureOutput::Paneled {
                canvas: Dimensions { width: 1024, height: 1024 },
                panels: Vec::new(),
            }
        } else {
            StructureOutput::Single
        };
    }

    if let StructureOutput::Paneled { canvas, panels } = &mut s.output {
        ui.horizontal(|ui| {
            ui.label("canvas w");
            ui.add(egui::DragValue::new(&mut canvas.width));
            ui.label("h");
            ui.add(egui::DragValue::new(&mut canvas.height));
        });
        let mut remove: Option<usize> = None;
        for (i, panel) in panels.iter_mut().enumerate() {
            egui::Frame::group(ui.style()).show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.label("label");
                    ui.add(egui::TextEdit::singleline(&mut panel.label).desired_width(100.0));
                    if ui.small_button(crate::icons::TRASH).on_hover_text("Remove panel").clicked() {
                        remove = Some(i);
                    }
                });
                ui.horizontal(|ui| {
                    ui.label("x");
                    ui.add(egui::DragValue::new(&mut panel.rect.x));
                    ui.label("y");
                    ui.add(egui::DragValue::new(&mut panel.rect.y));
                    ui.label("w");
                    ui.add(egui::DragValue::new(&mut panel.rect.w));
                    ui.label("h");
                    ui.add(egui::DragValue::new(&mut panel.rect.h));
                });
                ui.label("prose");
                ui.add(
                    egui::TextEdit::multiline(&mut panel.prose_fragment)
                        .desired_rows(2)
                        .desired_width(f32::INFINITY),
                );
                slot_picker(ui, &mut panel.slot, i);
            });
        }
        if let Some(i) = remove {
            panels.remove(i);
        }
        if ui.button(format!("{} Add panel", crate::icons::ADD)).clicked() {
            panels.push(StructurePanel {
                label: "panel".to_owned(),
                rect: PanelRect { x: 0, y: 0, w: 256, h: 256 },
                prose_fragment: String::new(),
                slot: PanelSlot::Generic,
            });
        }
    }
}

/// Edits a style: name, modifiers, look negatives, and the optional per-style
/// model and quality preferences. `model_pref` / `quality` default to `(none)`,
/// meaning "inherit the project / router default".
fn edit_style(ui: &mut egui::Ui, s: &mut Style) {
    ui.label("Name");
    ui.add(egui::TextEdit::singleline(&mut s.name).desired_width(f32::INFINITY));
    ui.add_space(4.0);
    ui.label("Modifiers — look description folded into the prompt");
    ui.add(egui::TextEdit::multiline(&mut s.modifiers).desired_rows(2).desired_width(f32::INFINITY));
    ui.add_space(4.0);
    ui.label("Look negatives");
    ui.add(egui::TextEdit::multiline(&mut s.look_negatives).desired_rows(2).desired_width(f32::INFINITY));
    ui.add_space(4.0);
    model_pref_picker(ui, &mut s.model_pref);
    quality_picker(ui, &mut s.quality);
}

/// The look models a Style may prefer, in display order. Excludes the upscale
/// (`FalRealEsrgan`) and vectorize (`FalRecraftVectorize`) models — they are not
/// general look generators, so they never belong on a style.
pub(crate) const LOOK_MODELS: [ModelId; 6] = [
    ModelId::Auto,
    ModelId::OpenAiGptImage2,
    ModelId::GoogleNanoBananaPro,
    ModelId::GoogleGeminiFlashImage,
    ModelId::FalFluxKontext,
    ModelId::FalFluxDev,
];

/// Human label for a look [`ModelId`].
pub(crate) fn model_label(model: ModelId) -> &'static str {
    match model {
        ModelId::Auto => "Auto (router default)",
        ModelId::OpenAiGptImage2 => "OpenAI gpt-image-2",
        ModelId::GoogleNanoBananaPro => "Google Nano Banana Pro",
        ModelId::GoogleGeminiFlashImage => "Google Gemini Flash Image",
        ModelId::FalFluxKontext => "fal Flux Kontext",
        ModelId::FalFluxDev => "fal Flux.1 dev",
        ModelId::FalRecraftVectorize => "fal Recraft vectorize",
        ModelId::FalRealEsrgan => "fal Real-ESRGAN",
    }
}

/// A `(none)`-plus-look-models dropdown over `Option<ModelId>`. `(none)` means
/// inherit the project / router default.
fn model_pref_picker(ui: &mut egui::Ui, model: &mut Option<ModelId>) {
    ui.horizontal(|ui| {
        ui.label("Model preference");
        let text = model.map_or("(none)", model_label);
        egui::ComboBox::from_id_salt("style_model_pref").selected_text(text).show_ui(ui, |ui| {
            ui.selectable_value(model, None, "(none)");
            for m in LOOK_MODELS {
                ui.selectable_value(model, Some(m), model_label(m));
            }
        });
    });
}

/// Human label for a [`Quality`] tier.
pub(crate) fn quality_label(quality: Quality) -> &'static str {
    match quality {
        Quality::Auto => "Auto",
        Quality::Low => "Low",
        Quality::Medium => "Medium",
        Quality::High => "High",
    }
}

/// A `(none)`-plus-tiers dropdown over `Option<Quality>`. `(none)` means inherit
/// the project default quality.
fn quality_picker(ui: &mut egui::Ui, quality: &mut Option<Quality>) {
    ui.horizontal(|ui| {
        ui.label("Quality");
        let text = quality.map_or("(none)", quality_label);
        egui::ComboBox::from_id_salt("style_quality").selected_text(text).show_ui(ui, |ui| {
            ui.selectable_value(quality, None, "(none)");
            for q in [Quality::Auto, Quality::Low, Quality::Medium, Quality::High] {
                ui.selectable_value(quality, Some(q), quality_label(q));
            }
        });
    });
}

/// A `(none)`-plus-choices dropdown over an optional id. Returns whether the
/// selection changed.
fn optional_picker(ui: &mut egui::Ui, label: &str, current: &mut Option<String>, choices: &[(String, String)]) -> bool {
    let mut changed = false;
    ui.horizontal(|ui| {
        ui.label(label);
        let text = current
            .as_ref()
            .and_then(|id| choices.iter().find(|(cid, _)| cid == id))
            .map_or_else(|| "(none)".to_owned(), |(_, name)| name.clone());
        egui::ComboBox::from_id_salt(("lib_opt", label)).selected_text(text).show_ui(ui, |ui| {
            if ui.selectable_label(current.is_none(), "(none)").clicked() {
                *current = None;
                changed = true;
            }
            for (id, name) in choices {
                if ui.selectable_label(current.as_deref() == Some(id.as_str()), name).clicked() {
                    *current = Some(id.clone());
                    changed = true;
                }
            }
        });
    });
    changed
}

/// A dropdown over the [`VarControl`] kinds. Switching kind resets to a default
/// of that kind (e.g. empty choices), since the kinds carry different data.
fn control_kind_picker(ui: &mut egui::Ui, control: &mut VarControl, salt: usize) {
    let label = match control {
        VarControl::Text => "Text",
        VarControl::Select { .. } => "Select",
        VarControl::Number { .. } => "Number",
        VarControl::Color => "Color",
        VarControl::Wildcard { .. } => "Wildcard",
    };
    egui::ComboBox::from_id_salt(("var_kind", salt)).selected_text(label).show_ui(ui, |ui| {
        if ui.selectable_label(matches!(control, VarControl::Text), "Text").clicked() {
            *control = VarControl::Text;
        }
        if ui.selectable_label(matches!(control, VarControl::Select { .. }), "Select").clicked() && !matches!(control, VarControl::Select { .. }) {
            *control = VarControl::Select { choices: Vec::new() };
        }
        if ui.selectable_label(matches!(control, VarControl::Number { .. }), "Number").clicked() && !matches!(control, VarControl::Number { .. }) {
            *control = VarControl::Number {
                min: 0.0,
                max: 10.0,
                step: 1.0,
            };
        }
        if ui.selectable_label(matches!(control, VarControl::Color), "Color").clicked() {
            *control = VarControl::Color;
        }
        if ui.selectable_label(matches!(control, VarControl::Wildcard { .. }), "Wildcard").clicked() && !matches!(control, VarControl::Wildcard { .. }) {
            *control = VarControl::Wildcard { choices: Vec::new() };
        }
    });
}

/// A dropdown over the [`PanelSlot`] kinds.
fn slot_picker(ui: &mut egui::Ui, slot: &mut PanelSlot, salt: usize) {
    let label = match slot {
        PanelSlot::View => "View",
        PanelSlot::Expression => "Expression",
        PanelSlot::Callout => "Callout",
        PanelSlot::Outfit => "Outfit",
        PanelSlot::PaletteSwatch => "Palette swatch",
        PanelSlot::Generic => "Generic",
    };
    egui::ComboBox::from_id_salt(("panel_slot", salt)).selected_text(label).show_ui(ui, |ui| {
        for (variant, name) in [
            (PanelSlot::View, "View"),
            (PanelSlot::Expression, "Expression"),
            (PanelSlot::Callout, "Callout"),
            (PanelSlot::Outfit, "Outfit"),
            (PanelSlot::PaletteSwatch, "Palette swatch"),
            (PanelSlot::Generic, "Generic"),
        ] {
            if ui.selectable_label(*slot == variant, name).clicked() {
                *slot = variant;
            }
        }
    });
}

/// Replaces the record with the same id, or appends it if absent.
fn upsert<T, I: PartialEq>(list: &mut Vec<T>, value: T, id_of: impl Fn(&T) -> &I) {
    let id = id_of(&value);
    if let Some(slot) = list.iter_mut().find(|x| id_of(x) == id) {
        *slot = value;
    } else {
        list.push(value);
    }
}

#[cfg(test)]
mod tests {
    use super::{ConflictPolicy, ModelId, Quality, Structure, StructureId, Style, StyleId, gather_export_pack, read_pack, upsert, write_pack};
    use crate::commands::push_ai_library_edit;
    use crate::document::DocumentStore;
    use crate::editor::EditorState;
    use pixhaus_ai::compose::builtins::BuiltinLibrary;
    use pixhaus_core::project::ProjectAi;
    use pixhaus_core::project::library::composition::{PromptId, PromptTemplate, StructureOutput};
    use std::collections::BTreeMap;

    fn structure(id: &str, name: &str) -> Structure {
        Structure {
            id: StructureId(id.into()),
            name: name.into(),
            output: StructureOutput::Single,
            layout_negatives: String::new(),
        }
    }

    fn prompt(id: &str, name: &str) -> PromptTemplate {
        PromptTemplate {
            id: PromptId(id.into()),
            name: name.into(),
            text: String::new(),
            variables: Vec::new(),
            default_style: None,
            default_structure: None,
        }
    }

    /// A hermetic built-in registry with one structure, so the gather rule is
    /// tested against a known set rather than the real (growing) built-ins.
    fn one_builtin_structure(id: &str) -> BuiltinLibrary {
        let mut structures = BTreeMap::new();
        structures.insert(StructureId(id.into()), structure(id, "Builtin"));
        BuiltinLibrary {
            structures,
            styles: BTreeMap::new(),
            prompts: BTreeMap::new(),
        }
    }

    #[test]
    fn gather_export_pack_includes_project_and_unshadowed_builtins() {
        let mut ai = ProjectAi::default();
        ai.structures.push(structure("project.s", "Project"));
        // The project shadows the built-in `shared` and adds its own; the
        // unshadowed built-in `extra` must still come along.
        ai.structures.push(structure("shared", "Project override"));
        let mut builtins = one_builtin_structure("extra");
        builtins.structures.insert(StructureId("shared".into()), structure("shared", "Builtin shared"));

        let pack = gather_export_pack(&ai, &builtins);
        let ids: Vec<&str> = pack.structures.iter().map(|s| s.id.0.as_str()).collect();
        assert!(ids.contains(&"project.s"), "project record is exported");
        assert!(ids.contains(&"extra"), "unshadowed built-in is exported");
        // The shadowed id appears once, carrying the project's override.
        assert_eq!(ids.iter().filter(|i| **i == "shared").count(), 1, "shadowed id is not duplicated");
        let shared = pack.structures.iter().find(|s| s.id.0 == "shared").expect("shared present");
        assert_eq!(shared.name, "Project override", "the project record wins over the shadowed built-in");
    }

    #[test]
    fn export_then_import_round_trips_records_into_a_fresh_project() {
        // Gather a source project's pack, encode and decode it through the
        // .pixstyle bytes, then merge into a fresh project — the export/import
        // acceptance path.
        let mut source = ProjectAi::default();
        source.structures.push(structure("s1", "One"));
        source.prompts.push(prompt("p1", "Warrior"));
        let builtins = BuiltinLibrary {
            structures: BTreeMap::new(),
            styles: BTreeMap::new(),
            prompts: BTreeMap::new(),
        };
        let pack = gather_export_pack(&source, &builtins);

        let mut bytes = Vec::new();
        write_pack(&pack, &mut bytes).expect("write");
        let decoded = read_pack(&bytes[..]).expect("read");

        let mut doc = DocumentStore::new();
        let mut editor = EditorState::default();
        doc.project.library.ai = ProjectAi::default();
        super::merge_pack_into_project(&mut editor, &mut doc, decoded, ConflictPolicy::Skip);

        let ai = &doc.project.library.ai;
        assert!(ai.structures.iter().any(|s| s.id.0 == "s1"), "structure imported into the fresh project");
        assert!(ai.prompts.iter().any(|p| p.id.0 == "p1"), "prompt imported into the fresh project");
    }

    #[test]
    fn re_import_with_skip_reports_skipped_without_duplicating() {
        // First import adds the record; a second Skip import of the same pack
        // reports it skipped and leaves the project unchanged.
        let mut doc = DocumentStore::new();
        let mut editor = EditorState::default();
        doc.project.library.ai = ProjectAi::default();

        let pack = super::StylePack {
            format_version: 1,
            structures: vec![structure("s1", "One")],
            styles: Vec::new(),
            prompts: Vec::new(),
        };

        super::merge_pack_into_project(&mut editor, &mut doc, pack.clone(), ConflictPolicy::Skip);
        let after_first = doc.project.library.ai.structures.len();
        assert_eq!(after_first, 1, "first import adds the record");

        let report = super::merge_pack(&mut doc.project.library.ai, pack, ConflictPolicy::Skip);
        assert_eq!(report.skipped, 1, "the colliding id is skipped");
        assert_eq!(report.imported, 0, "nothing new is added");
        assert_eq!(doc.project.library.ai.structures.len(), 1, "no duplicate record");
    }

    #[test]
    fn import_pack_is_one_undoable_edit() {
        let mut doc = DocumentStore::new();
        let mut editor = EditorState::default();
        doc.project.library.ai = ProjectAi::default();

        let pack = super::StylePack {
            format_version: 1,
            structures: vec![structure("s1", "One")],
            styles: Vec::new(),
            prompts: vec![prompt("p1", "Warrior")],
        };

        super::merge_pack_into_project(&mut editor, &mut doc, pack, ConflictPolicy::Skip);
        assert_eq!(doc.project.library.ai.structures.len(), 1);
        assert_eq!(doc.project.library.ai.prompts.len(), 1);

        // One undo removes the whole import — structures and prompts together.
        editor.history.undo(&mut doc).expect("undo");
        assert!(doc.project.library.ai.structures.is_empty(), "undo removes the imported structure");
        assert!(doc.project.library.ai.prompts.is_empty(), "undo removes the imported prompt in the same step");
    }

    fn style_with_prefs() -> Style {
        Style {
            id: StyleId("project.style.snes".into()),
            name: "SNES".into(),
            modifiers: "16-bit palette".into(),
            look_negatives: "blurry".into(),
            model_pref: Some(ModelId::FalFluxKontext),
            quality: Some(Quality::High),
        }
    }

    #[test]
    fn upsert_appends_then_replaces_by_id() {
        let mut styles = vec![style_with_prefs()];
        // Same id, different prefs: upsert replaces in place rather than appending.
        let mut edited = style_with_prefs();
        edited.model_pref = Some(ModelId::FalFluxDev);
        upsert(&mut styles, edited, |x| &x.id);
        assert_eq!(styles.len(), 1, "same id replaces");
        assert_eq!(styles[0].model_pref, Some(ModelId::FalFluxDev));
    }

    #[test]
    fn style_model_and_quality_edit_survives_undo_redo() {
        let mut doc = DocumentStore::new();
        let mut editor = EditorState::default();

        // Save a style carrying a model preference and quality, as the editor's
        // Save does via push_ai_library_edit + upsert.
        let value = style_with_prefs();
        push_ai_library_edit(&mut editor, &mut doc, "Save style", |ai| upsert(&mut ai.styles, value, |x| &x.id));

        let prefs = |doc: &DocumentStore| {
            doc.project
                .library
                .ai
                .styles
                .iter()
                .find(|s| s.id == StyleId("project.style.snes".into()))
                .map(|s| (s.model_pref, s.quality))
        };
        assert_eq!(prefs(&doc), Some((Some(ModelId::FalFluxKontext), Some(Quality::High))), "prefs persisted");

        editor.history.undo(&mut doc).expect("undo");
        assert_eq!(prefs(&doc), None, "undo removes the style");

        editor.history.redo(&mut doc).expect("redo");
        assert_eq!(
            prefs(&doc),
            Some((Some(ModelId::FalFluxKontext), Some(Quality::High))),
            "redo restores the style with its prefs"
        );
    }
}
