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
use pixhaus_core::project::library::composition::{
    Dimensions, PanelRect, PanelSlot, PromptId, PromptTemplate, PromptVariable, Structure, StructureId, StructureOutput, StructurePanel, Style, StyleId,
    VarControl,
};

use crate::app::ShellApp;
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

/// Edits a style: name, modifiers, and look negatives. `model_pref` and
/// `quality` are advanced fields preserved across edits but not exposed yet.
fn edit_style(ui: &mut egui::Ui, s: &mut Style) {
    ui.label("Name");
    ui.add(egui::TextEdit::singleline(&mut s.name).desired_width(f32::INFINITY));
    ui.add_space(4.0);
    ui.label("Modifiers — look description folded into the prompt");
    ui.add(egui::TextEdit::multiline(&mut s.modifiers).desired_rows(2).desired_width(f32::INFINITY));
    ui.add_space(4.0);
    ui.label("Look negatives");
    ui.add(egui::TextEdit::multiline(&mut s.look_negatives).desired_rows(2).desired_width(f32::INFINITY));
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
