//! The coverage service (bible 15).
//!
//! Coverage makes the Codex production-oriented: a [`CoverageTemplate`] names the
//! slots an entry should fill, and the Codex tracks a per-slot
//! [`CoverageItemStatus`]. This service reads the Codex and an entry and produces a
//! [`CoverageReport`] — every slot the entry's applied templates and custom slots
//! require, its current status, and which slots are still missing — without mutating
//! anything. Applying a template or setting a status is a `core` command; this is the
//! read side that drives the coverage checklist and the "generate missing" action
//! (bible 15.4).
//!
//! Coverage is per entry, never global: the report is the union of the entry's
//! [`applied_templates`](pixhaus_core::CodexEntry::applied_templates) (resolved via
//! [`Codex::coverage_template`]) plus its
//! [`custom_slots`](pixhaus_core::CodexEntry::custom_slots), de-duped by slot key. A
//! template applied to one entry never bleeds onto another.
//!
//! [`CoverageTemplate`]: pixhaus_core::CoverageTemplate
//! [`Codex::coverage_template`]: pixhaus_core::Codex::coverage_template

use std::collections::HashSet;

use pixhaus_core::{Codex, CodexEntryId, CoverageItemStatus, CoverageLabel, CoverageTemplateId};

/// One row of a coverage report: a slot, its label, its status, and where it came from.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct CoverageItem {
    /// The stable slot key (e.g. `idle`).
    pub slot: String,
    /// The slot's label: a localization [`Key`](CoverageLabel::Key) the UI resolves via
    /// `tr()`, or a [`Literal`](CoverageLabel::Literal) it renders as-is.
    pub label: CoverageLabel,
    /// The slot's current status.
    pub status: CoverageItemStatus,
    /// The template the slot came from, or `None` when it is a per-entry custom slot.
    pub template: Option<CoverageTemplateId>,
}

impl CoverageItem {
    /// Whether this slot still needs an asset (status [`CoverageItemStatus::Missing`]).
    pub fn is_missing(&self) -> bool {
        self.status == CoverageItemStatus::Missing
    }

    /// Whether this slot is finished (approved or manually finalized).
    pub fn is_complete(&self) -> bool {
        matches!(self.status, CoverageItemStatus::Approved | CoverageItemStatus::ManuallyFinalized)
    }
}

/// A coverage report for one entry across the templates and custom slots that apply to
/// it (bible 15).
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct CoverageReport {
    /// The entry the report is for.
    pub entry: CodexEntryId,
    /// Every required slot, in applied-template-then-custom-slot order, with its status.
    pub items: Vec<CoverageItem>,
}

impl CoverageReport {
    /// The number of slots still missing an asset.
    pub fn missing_count(&self) -> usize {
        self.items.iter().filter(|i| i.is_missing()).count()
    }

    /// The number of finished slots (approved or manually finalized).
    pub fn complete_count(&self) -> usize {
        self.items.iter().filter(|i| i.is_complete()).count()
    }

    /// The total number of required slots.
    pub fn total(&self) -> usize {
        self.items.len()
    }

    /// The fraction of slots that are complete, in `0.0..=1.0`. An empty report is
    /// fully complete (there is nothing left to do).
    pub fn completion_ratio(&self) -> f32 {
        let total = self.total();
        if total == 0 {
            return 1.0;
        }
        // Counts are small (slots per entry), so the u16 round-trip is exact and
        // keeps the conversion lossless under clippy's cast lints.
        let complete = u16::try_from(self.complete_count()).unwrap_or(u16::MAX);
        let total = u16::try_from(total).unwrap_or(u16::MAX);
        f32::from(complete) / f32::from(total)
    }

    /// The slot keys still missing an asset, in report order — the input to the
    /// "generate missing" action (bible 15.4).
    pub fn missing_slots(&self) -> Vec<&str> {
        self.items.iter().filter(|i| i.is_missing()).map(|i| i.slot.as_str()).collect()
    }
}

/// Computes the coverage report for `entry` from its own applied templates and custom
/// slots (bible 15) — never from the project's other templates.
///
/// The report is the union of the slots in `entry.applied_templates` (each resolved via
/// [`Codex::coverage_template`]) followed by `entry.custom_slots`, de-duped by slot key:
/// a key that appears in two applied templates (or in a template and a custom slot)
/// reports once, at its first occurrence. The per-slot status comes from the Codex
/// coverage state, defaulting to [`CoverageItemStatus::Missing`]. The report is empty
/// when `entry` does not exist or the entry has no templates or custom slots. An applied
/// template id that no longer resolves is skipped (logged at `debug`), not an error.
///
/// [`Codex::coverage_template`]: pixhaus_core::Codex::coverage_template
#[tracing::instrument(level = "debug", skip(codex))]
pub fn coverage_report(codex: &Codex, entry: CodexEntryId) -> CoverageReport {
    let mut report = CoverageReport { entry, items: Vec::new() };
    let Some(entry_data) = codex.entry(entry) else {
        return report;
    };

    // Slots are unique by key across the union; the first occurrence wins.
    let mut seen: HashSet<String> = HashSet::new();

    for template_id in &entry_data.applied_templates {
        let Some(template) = codex.coverage_template(*template_id) else {
            tracing::debug!(target: "pixhaus::coverage", ?template_id, "applied template id does not resolve; skipping");
            continue;
        };
        for slot in &template.slots {
            if !seen.insert(slot.key.clone()) {
                continue;
            }
            report.items.push(CoverageItem {
                slot: slot.key.clone(),
                label: slot.label.clone(),
                status: codex.coverage_status(entry, &slot.key),
                template: Some(*template_id),
            });
        }
    }

    for slot in &entry_data.custom_slots {
        if !seen.insert(slot.key.clone()) {
            continue;
        }
        report.items.push(CoverageItem {
            slot: slot.key.clone(),
            label: slot.label.clone(),
            status: codex.coverage_status(entry, &slot.key),
            template: None,
        });
    }

    report
}

/// A picker-facing summary of one project coverage template: its id, display name, and
/// slot count. Borrows nothing — owns the name so the UI can hold it across frames.
pub fn coverage_template_summaries(codex: &Codex) -> Vec<(CoverageTemplateId, String, usize)> {
    codex.coverage_templates().map(|t| (t.id, t.name.clone(), t.slots.len())).collect()
}

/// One slot of a coverage template, mirrored for the UI: its stable key and its label.
///
/// The same shape as a [`CoverageItem`] minus the per-entry status — a template's slots
/// exist independent of any entry, so editing them (add, remove, rename, reorder) must be
/// reachable whether or not the template is applied to the selected entry. The key is the
/// stable id a rename never touches; the label is a [`CoverageLabel`]
/// the layers above resolve (a [`Key`](CoverageLabel::Key) variant through `tr()`, a
/// [`Literal`](CoverageLabel::Literal) variant as-is).
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct CoverageSlotView {
    /// The stable slot key (e.g. `idle`); set once, never changed by a rename.
    pub key: String,
    /// The slot's label; the layers above resolve it.
    pub label: CoverageLabel,
}

/// One project coverage template with its full slot list, for the template-management
/// surface (bible 15.2).
///
/// [`coverage_template_summaries`] gives the picker an id, a name, and a slot count —
/// enough to attach a template to an entry. It is not enough to *edit* a template: the
/// four slot commands ([`AddCoverageSlot`](pixhaus_core::AddCoverageSlot),
/// [`RemoveCoverageSlot`](pixhaus_core::RemoveCoverageSlot),
/// [`RenameCoverageSlotLabel`](pixhaus_core::RenameCoverageSlotLabel),
/// [`ReorderCoverageSlots`](pixhaus_core::ReorderCoverageSlots)) act on a template by id,
/// so the UI needs every template's slots to render and drive them. This detail carries
/// that list, owned so the UI can hold it across frames, for *every* project template —
/// not just the ones applied to the selected entry.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct CoverageTemplateDetail {
    /// Stable id; the slot intents echo it back.
    pub id: CoverageTemplateId,
    /// Display name (project content; may change without breaking references).
    pub name: String,
    /// The template's slots, in order.
    pub slots: Vec<CoverageSlotView>,
}

/// Every project coverage template with its full slot list, for the template-management
/// surface (bible 15.2).
///
/// The richer companion to [`coverage_template_summaries`]: the summary drives the picker
/// (name + count), this drives the slot editor. It exposes the slots of *every* template,
/// so a freshly created or unapplied template's slots are editable too — closing the gap
/// where slot edits were only reachable through a template already applied to the selected
/// entry. Borrows nothing; owns the names and labels so the UI can hold them across frames.
pub fn coverage_template_details(codex: &Codex) -> Vec<CoverageTemplateDetail> {
    codex
        .coverage_templates()
        .map(|t| CoverageTemplateDetail {
            id: t.id,
            name: t.name.clone(),
            slots: t
                .slots
                .iter()
                .map(|s| CoverageSlotView {
                    key: s.key.clone(),
                    label: s.label.clone(),
                })
                .collect(),
        })
        .collect()
}
#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::disallowed_methods, clippy::panic)]
mod tests {
    use super::*;
    use pixhaus_core::{
        AddCodexEntry, AddEntryCustomSlot, ApplyCoverageTemplate, CodexEntryProto, CodexHandle, Command, CoverageSlot, CoverageTemplate,
        CreateCoverageTemplate, Document, EntryType, SetCoverageStatus,
    };

    fn add(doc: &mut Document, h: &str) -> CodexEntryId {
        let proto = CodexEntryProto {
            handle: CodexHandle::new(h).unwrap(),
            name: h.to_owned(),
            entry_type: EntryType::Character,
        };
        let mut cmd = AddCodexEntry::new(proto);
        cmd.apply(doc).unwrap();
        cmd.inserted_id().unwrap()
    }

    fn create(doc: &mut Document, template: CoverageTemplate) -> CoverageTemplateId {
        let mut cmd = CreateCoverageTemplate::new(template.name.clone(), template.slots);
        cmd.apply(doc).unwrap();
        cmd.inserted_id().unwrap()
    }

    fn apply(doc: &mut Document, entry: CodexEntryId, template: CoverageTemplateId) {
        ApplyCoverageTemplate::new(entry, template).apply(doc).unwrap();
    }

    #[test]
    fn report_for_unknown_entry_is_empty() {
        let doc = Document::new();
        let report = coverage_report(doc.codex(), CodexEntryId(99));
        assert!(report.items.is_empty());
        assert_eq!(report.total(), 0);
        // An empty report counts as fully complete.
        assert!((report.completion_ratio() - 1.0).abs() < f32::EPSILON);
    }

    #[test]
    fn applied_template_seeds_missing_slots() {
        let mut doc = Document::new();
        let bit = add(&mut doc, "bit");
        let t = create(&mut doc, CoverageTemplate::platformer_character());
        apply(&mut doc, bit, t);
        let report = coverage_report(doc.codex(), bit);
        assert_eq!(report.total(), 9);
        assert_eq!(report.missing_count(), 9);
        assert_eq!(report.complete_count(), 0);
        assert!(report.completion_ratio().abs() < f32::EPSILON);
        assert_eq!(
            report.missing_slots(),
            vec!["idle", "walk", "run", "jump", "fall", "land", "attack", "hurt", "death"]
        );
    }

    #[test]
    fn setting_a_status_updates_the_report() {
        let mut doc = Document::new();
        let bit = add(&mut doc, "bit");
        let t = create(&mut doc, CoverageTemplate::platformer_character());
        apply(&mut doc, bit, t);
        let mut set = SetCoverageStatus::new(bit, "idle", CoverageItemStatus::Approved);
        set.apply(&mut doc).unwrap();
        let report = coverage_report(doc.codex(), bit);
        assert_eq!(report.complete_count(), 1);
        assert_eq!(report.missing_count(), 8);
        let idle = report.items.iter().find(|i| i.slot == "idle").unwrap();
        assert!(idle.is_complete());
        assert_eq!(idle.label, CoverageLabel::Key("codex.coverage.slot.idle".to_owned()));
        assert_eq!(idle.template, Some(t));
    }

    #[test]
    fn entry_without_template_has_no_items() {
        let mut doc = Document::new();
        let bit = add(&mut doc, "bit");
        let report = coverage_report(doc.codex(), bit);
        assert!(report.items.is_empty());
    }

    #[test]
    fn per_entry_reports_are_disjoint() {
        // Template X on bit, Y on mossy. Neither report carries the other's slots.
        let mut doc = Document::new();
        let bit = add(&mut doc, "bit");
        let mossy = add(&mut doc, "mossy");
        let x = create(&mut doc, CoverageTemplate::platformer_character());
        let y = create(&mut doc, CoverageTemplate::ui_button_states());
        apply(&mut doc, bit, x);
        apply(&mut doc, mossy, y);

        let bit_report = coverage_report(doc.codex(), bit);
        let mossy_report = coverage_report(doc.codex(), mossy);

        let bit_slots: Vec<&str> = bit_report.items.iter().map(|i| i.slot.as_str()).collect();
        let mossy_slots: Vec<&str> = mossy_report.items.iter().map(|i| i.slot.as_str()).collect();
        assert_eq!(bit_slots, ["idle", "walk", "run", "jump", "fall", "land", "attack", "hurt", "death"]);
        assert_eq!(mossy_slots, ["normal", "hover", "pressed", "disabled"]);
        assert!(bit_slots.iter().all(|s| !mossy_slots.contains(s)));
    }

    #[test]
    fn a_custom_slot_appears_only_on_its_entry() {
        let mut doc = Document::new();
        let bit = add(&mut doc, "bit");
        let mossy = add(&mut doc, "mossy");
        let mut cmd = AddEntryCustomSlot::new(bit, CoverageSlot::custom("crouch", "Crouch"));
        cmd.apply(&mut doc).unwrap();

        let bit_report = coverage_report(doc.codex(), bit);
        let crouch = bit_report.items.iter().find(|i| i.slot == "crouch").unwrap();
        assert_eq!(crouch.label, CoverageLabel::Literal("Crouch".to_owned()));
        assert_eq!(crouch.template, None);

        let mossy_report = coverage_report(doc.codex(), mossy);
        assert!(mossy_report.items.iter().all(|i| i.slot != "crouch"));
    }

    #[test]
    fn a_slot_in_two_applied_templates_appears_once() {
        // Both presets define `idle`; applying both to one entry reports `idle` once.
        let mut doc = Document::new();
        let bit = add(&mut doc, "bit");
        let platformer = create(&mut doc, CoverageTemplate::platformer_character());
        let top_down = create(&mut doc, CoverageTemplate::top_down_four_direction());
        apply(&mut doc, bit, platformer);
        apply(&mut doc, bit, top_down);

        let report = coverage_report(doc.codex(), bit);
        let idle_count = report.items.iter().filter(|i| i.slot == "idle").count();
        assert_eq!(idle_count, 1);
        // The first occurrence wins, so `idle` is attributed to the platformer template.
        let idle = report.items.iter().find(|i| i.slot == "idle").unwrap();
        assert_eq!(idle.template, Some(platformer));
    }

    #[test]
    fn template_summaries_list_id_name_and_slot_count() {
        let mut doc = Document::new();
        let platformer = create(&mut doc, CoverageTemplate::platformer_character());
        let ui = create(&mut doc, CoverageTemplate::ui_button_states());
        let summaries = coverage_template_summaries(doc.codex());
        assert_eq!(
            summaries,
            vec![(platformer, "platformer_character".to_owned(), 9), (ui, "ui_button_states".to_owned(), 4)]
        );
    }

    #[test]
    fn summaries_are_empty_with_no_templates() {
        let doc = Document::new();
        assert!(coverage_template_summaries(doc.codex()).is_empty());
    }

    #[test]
    fn template_details_carry_every_slot_for_every_template() {
        let mut doc = Document::new();
        let platformer = create(&mut doc, CoverageTemplate::platformer_character());
        let ui = create(&mut doc, CoverageTemplate::ui_button_states());
        let details = coverage_template_details(doc.codex());

        assert_eq!(details.len(), 2);

        let p = &details[0];
        assert_eq!(p.id, platformer);
        assert_eq!(p.name, "platformer_character");
        assert_eq!(
            p.slots.iter().map(|s| s.key.as_str()).collect::<Vec<_>>(),
            vec!["idle", "walk", "run", "jump", "fall", "land", "attack", "hurt", "death"]
        );
        // A preset slot carries a localization-key label, never a literal.
        assert_eq!(p.slots[0].label, CoverageLabel::Key("codex.coverage.slot.idle".to_owned()));

        let u = &details[1];
        assert_eq!(u.id, ui);
        assert_eq!(
            u.slots.iter().map(|s| s.key.as_str()).collect::<Vec<_>>(),
            vec!["normal", "hover", "pressed", "disabled"]
        );
    }

    #[test]
    fn template_details_expose_an_unapplied_templates_slots() {
        // The whole point of the detail helper: a template that is NOT applied to any entry
        // still exposes its slots, so the slot editor is reachable for it.
        let mut doc = Document::new();
        let bit = add(&mut doc, "bit");
        let template = create(&mut doc, CoverageTemplate::platformer_character());

        // The entry has no applied templates, so its report is empty...
        assert!(coverage_report(doc.codex(), bit).items.is_empty());
        // ...but the template's slots are still reachable for editing.
        let details = coverage_template_details(doc.codex());
        let detail = details.iter().find(|d| d.id == template).unwrap();
        assert_eq!(detail.slots.len(), 9);
    }

    #[test]
    fn template_details_reflect_a_custom_slot_label() {
        // A user-authored slot carries a literal label, surfaced verbatim (never localized).
        let mut doc = Document::new();
        let template = create(&mut doc, CoverageTemplate::new("custom", vec![CoverageSlot::custom("crouch", "Crouch")]));
        let details = coverage_template_details(doc.codex());
        let detail = details.iter().find(|d| d.id == template).unwrap();
        assert_eq!(
            detail.slots,
            vec![CoverageSlotView {
                key: "crouch".to_owned(),
                label: CoverageLabel::Literal("Crouch".to_owned())
            }]
        );
    }

    #[test]
    fn template_details_are_empty_with_no_templates() {
        let doc = Document::new();
        assert!(coverage_template_details(doc.codex()).is_empty());
    }
}
