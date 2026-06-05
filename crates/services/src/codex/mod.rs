//! Codex services: the read-side behavior over a project's creative bible.
//!
//! `core` owns the Codex data model and the commands that mutate it; this module
//! owns the pure functions that read it (architecture bible 12.3, 13-16). None of
//! these touch a document handle, a job, or egui — each is a function or small struct
//! over `&Codex`:
//!
//! - [`reference`](mod@self::reference) parses `@`-mentions and resolves them to
//!   stable entry ids.
//! - [`search`] ranks entries for `@`-autocomplete with a hand-rolled fuzzy matcher.
//! - [`compiler`] composes a final, inspectable generation prompt from layers.
//! - [`coverage`] computes a per-entry coverage report from the templates.
//! - [`validation`] checks entries and a compiled prompt, returning severities.
//! - [`health`] derives an entry's health, generation readiness, used-in counts, and
//!   key-info from the passes above.
//!
//! The UI builds the `core` commands and the existing [`History`](crate::History)
//! executes them; this module never mutates the model.

pub mod compiler;
pub mod coverage;
pub mod demo;
pub mod folder;
pub mod health;
pub mod reference;
pub mod search;
pub mod validation;

#[cfg(test)]
mod test_support;

pub use compiler::{CompiledPrompt, ExcludedFragment, IncludedAnchor, IncludedFragment, PromptRequest, compile, preview_entry, reference_type};
pub use coverage::{
    CoverageItem, CoverageReport, CoverageSlotView, CoverageTemplateDetail, coverage_report, coverage_template_details, coverage_template_summaries,
};
pub use health::{
    CheckState, EntryHealth, GenerationReadiness, HealthCheck, HealthTier, KeyInfo, LinkedAssets, UsedInCounts, broken_reference_count, entry_health,
    generation_readiness, key_info, linked_assets, used_in_counts,
};
pub use reference::{ParseError, ParsedReference, ResolutionProblem, ResolutionReport, ResolvedReference, parse_references, resolve_reference, resolve_text};
pub use search::{DEFAULT_SUGGESTION_LIMIT, SearchContext, Suggestion, fuzzy_score, suggest};
pub use validation::{Diagnostic, Severity, ValidationReport, check_readiness, validate_codex};
