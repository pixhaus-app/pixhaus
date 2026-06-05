//! Codex validation and generation-readiness checks (bible 16).
//!
//! Two read-only passes over the Codex: [`validate_codex`] checks every entry for
//! missing required fields, duplicate handles, and broken or deprecated references,
//! and [`check_readiness`] checks a [`CompiledPrompt`] before generation —
//! unresolved references and an empty positive prompt block, a deprecated reference
//! warns. Each finding is a [`Diagnostic`] with a [`Severity`]; the UI shows them in
//! the prompt linter (bible 14.4) and the validation panel.
//!
//! Findings carry stable message keys (`codex.validation.*`), never display text —
//! `core`/`services` store keys and the UI resolves them.

use pixhaus_core::{Codex, CodexEntryId, EntryStatus, EntryType};

use crate::codex::compiler::CompiledPrompt;
use crate::codex::reference::{ResolutionProblem, ResolutionReport};

/// How serious a validation finding is (bible 16.4). Ordered least-to-most severe.
#[derive(Copy, Clone, PartialEq, Eq, Hash, Debug, PartialOrd, Ord)]
pub enum Severity {
    /// Informational; nothing to fix.
    Info,
    /// A problem worth attention; generation may still proceed with caution.
    Warning,
    /// A real error; generation should be discouraged.
    Error,
    /// A blocking error; generation must not proceed.
    Blocking,
}

/// One validation finding (bible 16).
///
/// `message_key` is a stable localization key; `subject` ties the finding to an entry
/// when one applies; `detail` carries non-localized context (a handle, a slot key)
/// the UI can interpolate. No display text lives here.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Diagnostic {
    /// How serious the finding is.
    pub severity: Severity,
    /// The stable message key under `codex.validation.*`.
    pub message_key: String,
    /// The entry the finding is about, if any.
    pub subject: Option<CodexEntryId>,
    /// Non-localized context (e.g. the offending handle), for the UI to interpolate.
    pub detail: Option<String>,
}

impl Diagnostic {
    /// A diagnostic with `severity` and `message_key`, no subject or detail.
    pub fn new(severity: Severity, message_key: impl Into<String>) -> Self {
        Self {
            severity,
            message_key: message_key.into(),
            subject: None,
            detail: None,
        }
    }

    /// Attaches a subject entry.
    #[must_use]
    pub fn with_subject(mut self, subject: CodexEntryId) -> Self {
        self.subject = Some(subject);
        self
    }

    /// Attaches non-localized detail text.
    #[must_use]
    pub fn with_detail(mut self, detail: impl Into<String>) -> Self {
        self.detail = Some(detail.into());
        self
    }
}

/// The outcome of a validation pass: the findings, in discovery order.
#[derive(Clone, PartialEq, Eq, Debug, Default)]
pub struct ValidationReport {
    /// The findings, in the order they were discovered.
    pub diagnostics: Vec<Diagnostic>,
}

impl ValidationReport {
    /// The most severe finding's severity, or `None` for a clean report.
    pub fn worst(&self) -> Option<Severity> {
        self.diagnostics.iter().map(|d| d.severity).max()
    }

    /// Whether any finding is [`Severity::Blocking`].
    pub fn has_blocking(&self) -> bool {
        self.diagnostics.iter().any(|d| d.severity == Severity::Blocking)
    }

    /// Whether the report is clean (no findings).
    pub fn is_clean(&self) -> bool {
        self.diagnostics.is_empty()
    }

    /// The count of findings at exactly `severity`.
    pub fn count(&self, severity: Severity) -> usize {
        self.diagnostics.iter().filter(|d| d.severity == severity).count()
    }
}

/// Message keys the validation findings use. Stable ids; the UI resolves them.
pub mod keys {
    /// An entry has no name.
    pub const MISSING_NAME: &str = "codex.validation.missing_name";
    /// An entry has no description.
    pub const MISSING_DESCRIPTION: &str = "codex.validation.missing_description";
    /// A character entry has no visual anchor.
    pub const MISSING_VISUAL_ANCHOR: &str = "codex.validation.missing_visual_anchor";
    /// Two entries claim the same handle (a model bug, but checked defensively).
    pub const DUPLICATE_HANDLE: &str = "codex.validation.duplicate_handle";
    /// A character's palette reference does not resolve.
    pub const BROKEN_PALETTE_REF: &str = "codex.validation.broken_palette_ref";
    /// A reference points at a deprecated entry.
    pub const DEPRECATED_REFERENCE: &str = "codex.validation.deprecated_reference";
    /// The compiled prompt has an unresolved `@`-reference.
    pub const UNRESOLVED_REFERENCE: &str = "codex.validation.unresolved_reference";
    /// The compiled prompt has an ambiguous `@`-reference.
    pub const AMBIGUOUS_REFERENCE: &str = "codex.validation.ambiguous_reference";
    /// The compiled positive prompt is empty.
    pub const EMPTY_PROMPT: &str = "codex.validation.empty_prompt";
}

/// Validates every entry in `codex` (bible 16.1).
///
/// Checks per entry: a missing name (error), a missing description (warning), a
/// missing visual anchor on a character (warning), and a character palette reference
/// that does not resolve to a palette entry (error). Across entries: a duplicate
/// primary handle (blocking — the model should prevent it, but a corrupt project
/// might not). A deprecated entry is reported as info, since it still resolves.
#[tracing::instrument(level = "debug", skip(codex))]
pub fn validate_codex(codex: &Codex) -> ValidationReport {
    let mut report = ValidationReport::default();

    // Cross-entry: detect duplicate primary handles.
    let mut seen: Vec<&str> = Vec::new();
    for entry in codex.entries().values() {
        let h = entry.handle.as_str();
        if seen.contains(&h) {
            report.diagnostics.push(
                Diagnostic::new(Severity::Blocking, keys::DUPLICATE_HANDLE)
                    .with_subject(entry.id)
                    .with_detail(h),
            );
        } else {
            seen.push(h);
        }
    }

    for entry in codex.entries().values() {
        if entry.name.trim().is_empty() {
            report
                .diagnostics
                .push(Diagnostic::new(Severity::Error, keys::MISSING_NAME).with_subject(entry.id));
        }
        if entry.description.trim().is_empty() {
            report
                .diagnostics
                .push(Diagnostic::new(Severity::Warning, keys::MISSING_DESCRIPTION).with_subject(entry.id));
        }
        if entry.status == EntryStatus::Deprecated {
            report
                .diagnostics
                .push(Diagnostic::new(Severity::Info, keys::DEPRECATED_REFERENCE).with_subject(entry.id));
        }
        // A character with no visual anchor is hard to keep on-model.
        if entry.entry_type == EntryType::Character && entry.anchor_position(pixhaus_core::AnchorKind::Visual).is_none() {
            report
                .diagnostics
                .push(Diagnostic::new(Severity::Warning, keys::MISSING_VISUAL_ANCHOR).with_subject(entry.id));
        }
        // A character's palette reference must resolve to a palette entry.
        if let pixhaus_core::EntryDetails::Character(details) = &entry.details {
            if let Some(palette_ref) = &details.palette_ref {
                let resolves_to_palette = codex
                    .resolve_handle(palette_ref)
                    .and_then(|id| codex.entry(id))
                    .is_some_and(|e| e.entry_type == EntryType::Palette);
                if !resolves_to_palette {
                    report.diagnostics.push(
                        Diagnostic::new(Severity::Error, keys::BROKEN_PALETTE_REF)
                            .with_subject(entry.id)
                            .with_detail(palette_ref.as_str()),
                    );
                }
            }
        }
    }

    report
}

/// Checks whether a compiled prompt is ready to generate (bible 16.3).
///
/// Takes the [`ResolutionReport`] from resolving the prompt text and the
/// [`CompiledPrompt`] the compiler produced. An unresolved reference blocks; an
/// ambiguous reference blocks; an empty positive prompt blocks; a deprecated
/// reference warns. A clean report means generation may proceed.
#[tracing::instrument(level = "debug", skip(resolution, compiled))]
pub fn check_readiness(resolution: &ResolutionReport, compiled: &CompiledPrompt) -> ValidationReport {
    let mut report = ValidationReport::default();

    for problem in &resolution.problems {
        match problem {
            ResolutionProblem::Unresolved { reference } => {
                report
                    .diagnostics
                    .push(Diagnostic::new(Severity::Blocking, keys::UNRESOLVED_REFERENCE).with_detail(reference.raw.clone()));
            }
            ResolutionProblem::Ambiguous { reference, .. } => {
                report
                    .diagnostics
                    .push(Diagnostic::new(Severity::Blocking, keys::AMBIGUOUS_REFERENCE).with_detail(reference.raw.clone()));
            }
            ResolutionProblem::Deprecated { reference, entry, .. } => {
                report.diagnostics.push(
                    Diagnostic::new(Severity::Warning, keys::DEPRECATED_REFERENCE)
                        .with_subject(*entry)
                        .with_detail(reference.raw.clone()),
                );
            }
        }
    }

    if compiled.positive.trim().is_empty() {
        report.diagnostics.push(Diagnostic::new(Severity::Blocking, keys::EMPTY_PROMPT));
    }

    report
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::disallowed_methods, clippy::panic)]
mod tests {
    use super::*;
    use crate::codex::compiler::{PromptRequest, compile};
    use crate::codex::reference::resolve_text;
    use pixhaus_core::{
        AddCodexEntry, Anchor, AnchorKind, AnchorStrength, CodexEntryDelta, CodexEntryProto, CodexHandle, Command, Document, SetAnchor, SetEntryStatus,
        UpdateCodexEntry,
    };

    fn handle(s: &str) -> CodexHandle {
        CodexHandle::new(s).unwrap()
    }

    fn add(doc: &mut Document, h: &str, entry_type: EntryType) -> CodexEntryId {
        let proto = CodexEntryProto {
            handle: handle(h),
            name: h.to_owned(),
            entry_type,
        };
        let mut cmd = AddCodexEntry::new(proto);
        cmd.apply(doc).unwrap();
        cmd.inserted_id().unwrap()
    }

    fn describe(doc: &mut Document, id: CodexEntryId) {
        let delta = CodexEntryDelta {
            description: Some("a thing".to_owned()),
            ..CodexEntryDelta::new()
        };
        let mut cmd = UpdateCodexEntry::new(id, delta);
        cmd.apply(doc).unwrap();
    }

    #[test]
    fn flags_missing_description_as_warning() {
        let mut doc = Document::new();
        add(&mut doc, "prop_a", EntryType::Prop);
        let report = validate_codex(doc.codex());
        assert!(
            report
                .diagnostics
                .iter()
                .any(|d| d.message_key == keys::MISSING_DESCRIPTION && d.severity == Severity::Warning)
        );
    }

    #[test]
    fn character_without_visual_anchor_warns() {
        let mut doc = Document::new();
        let bit = add(&mut doc, "bit", EntryType::Character);
        describe(&mut doc, bit);
        let report = validate_codex(doc.codex());
        assert!(report.diagnostics.iter().any(|d| d.message_key == keys::MISSING_VISUAL_ANCHOR));
        // Adding a visual anchor clears the warning.
        let mut set = SetAnchor::new(bit, Anchor::new(AnchorKind::Visual, AnchorStrength::Normal, "round head"));
        set.apply(&mut doc).unwrap();
        let report = validate_codex(doc.codex());
        assert!(!report.diagnostics.iter().any(|d| d.message_key == keys::MISSING_VISUAL_ANCHOR));
    }

    #[test]
    fn broken_palette_ref_is_an_error() {
        use crate::codex::test_support::with_palette_ref;
        let mut doc = Document::new();
        let bit = add(&mut doc, "bit", EntryType::Character);
        describe(&mut doc, bit);
        // Point the character at a palette handle that does not exist.
        let codex = with_palette_ref(doc.codex(), bit, "ghost_palette");
        let report = validate_codex(&codex);
        let broken = report.diagnostics.iter().find(|d| d.message_key == keys::BROKEN_PALETTE_REF).unwrap();
        assert_eq!(broken.severity, Severity::Error);
        assert_eq!(broken.detail.as_deref(), Some("ghost_palette"));
    }

    #[test]
    fn deprecated_entry_is_info() {
        let mut doc = Document::new();
        let old = add(&mut doc, "old", EntryType::Style);
        describe(&mut doc, old);
        let mut dep = SetEntryStatus::new(old, EntryStatus::Deprecated);
        dep.apply(&mut doc).unwrap();
        let report = validate_codex(doc.codex());
        assert!(
            report
                .diagnostics
                .iter()
                .any(|d| d.message_key == keys::DEPRECATED_REFERENCE && d.severity == Severity::Info)
        );
    }

    #[test]
    fn readiness_blocks_on_unresolved_and_empty() {
        let doc = Document::new();
        // An empty prompt with an unresolved reference.
        let resolution = resolve_text(doc.codex(), "@ghost");
        let req = PromptRequest {
            user_request: String::new(),
            references: resolution.resolved.clone(),
            ..PromptRequest::default()
        };
        let compiled = compile(doc.codex(), &req);
        let report = check_readiness(&resolution, &compiled);
        assert!(report.has_blocking());
        assert!(report.diagnostics.iter().any(|d| d.message_key == keys::UNRESOLVED_REFERENCE));
        assert!(report.diagnostics.iter().any(|d| d.message_key == keys::EMPTY_PROMPT));
    }

    #[test]
    fn readiness_clean_for_a_good_prompt() {
        let mut doc = Document::new();
        add(&mut doc, "bit", EntryType::Character);
        let resolution = resolve_text(doc.codex(), "@bit jumping");
        let req = PromptRequest {
            user_request: "@bit jumping".to_owned(),
            references: resolution.resolved.clone(),
            ..PromptRequest::default()
        };
        let compiled = compile(doc.codex(), &req);
        let report = check_readiness(&resolution, &compiled);
        assert!(report.is_clean());
    }

    #[test]
    fn worst_and_severity_order() {
        let mut report = ValidationReport::default();
        report.diagnostics.push(Diagnostic::new(Severity::Info, "a"));
        report.diagnostics.push(Diagnostic::new(Severity::Error, "b"));
        assert_eq!(report.worst(), Some(Severity::Error));
        assert_eq!(report.count(Severity::Info), 1);
        assert!(Severity::Info < Severity::Blocking);
    }
}
