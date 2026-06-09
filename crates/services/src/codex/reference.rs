//! The `@`-reference parser and resolver (bible 6.5, 6.6, 6.7, 13.2).
//!
//! Prompt text carries `@`-mentions in three forms: loose (`@bit`), typed with an
//! explicit namespace (`@character.bit`, `@palette.moonlit_ruins`), and
//! field-scoped (`@bit.animation.jump`, the leading segment is the entry handle and
//! the trailing segments are a scope path the compiler may use). Parsing extracts
//! the tokens; resolution turns each into a stable [`CodexEntryId`] against a
//! [`Codex`], reporting typed problems — unresolved, ambiguous, or deprecated — so
//! the UI can offer relink / replace actions.
//!
//! This is a pure read over `&Codex`: it mints no ids and mutates nothing.

use thiserror::Error;

use pixhaus_core::{Codex, CodexEntryId, CodexHandle, EntryStatus, EntryType, HandleError};

/// Why a raw `@`-token could not be parsed into a [`ParsedReference`].
#[derive(Debug, Error, PartialEq, Eq)]
pub enum ParseError {
    /// The token after `@` was empty (a bare `@` with no handle).
    #[error("reference is empty")]
    Empty,
    /// The namespace segment of a typed reference named no known [`EntryType`].
    #[error("unknown namespace {0:?}")]
    UnknownNamespace(String),
    /// The handle segment was not a valid [`CodexHandle`].
    #[error("invalid handle: {0}")]
    BadHandle(#[from] HandleError),
}

/// A parsed but not-yet-resolved `@`-reference (bible 6.3, 6.8).
///
/// The byte range locates the token in the source text so the UI can replace it
/// with a chip. The optional `entry_type` is present only for the typed form. The
/// `scope` path holds any trailing field-scope segments (e.g. `["animation",
/// "jump"]` for `@bit.animation.jump`); it is empty for plain references.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct ParsedReference {
    /// The entry handle the reference points at.
    pub handle: CodexHandle,
    /// The namespace, when the reference was written in typed form.
    pub entry_type: Option<EntryType>,
    /// Trailing field-scope segments, in order; empty for a plain reference.
    pub scope: Vec<String>,
    /// Byte offset of the leading `@` in the source text.
    pub start: usize,
    /// Byte offset one past the last character of the token.
    pub end: usize,
    /// The raw token text, including the leading `@`.
    pub raw: String,
}

/// A successfully resolved reference: a parsed token plus the entry it names.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct ResolvedReference {
    /// The parsed token this resolves.
    pub parsed: ParsedReference,
    /// The stable id of the entry the token resolves to.
    pub entry: CodexEntryId,
    /// The resolved entry's status, so the compiler can weight deprecated entries.
    pub status: EntryStatus,
}

/// A typed resolution problem for a parsed reference (bible 6.6, 6.7, 14.4).
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum ResolutionProblem {
    /// No entry matched the handle (optionally constrained by namespace).
    Unresolved {
        /// The token that did not resolve.
        reference: ParsedReference,
    },
    /// More than one entry matched a loose handle; the candidates are listed.
    Ambiguous {
        /// The token that matched more than one entry.
        reference: ParsedReference,
        /// The entries that matched, in id order.
        candidates: Vec<CodexEntryId>,
    },
    /// The reference resolved, but the entry is deprecated.
    Deprecated {
        /// The token that resolved to a deprecated entry.
        reference: ParsedReference,
        /// The entry it resolved to.
        entry: CodexEntryId,
        /// A suggested replacement (the first non-deprecated entry of the same
        /// type, if one exists), so the UI can offer a relink.
        suggested_replacement: Option<CodexEntryId>,
    },
}

/// The result of resolving every `@`-token in a span of text (bible 6.5).
#[derive(Clone, PartialEq, Eq, Debug, Default)]
pub struct ResolutionReport {
    /// The references that resolved cleanly to a non-deprecated entry.
    pub resolved: Vec<ResolvedReference>,
    /// The references that could not be resolved cleanly.
    pub problems: Vec<ResolutionProblem>,
}

impl ResolutionReport {
    /// Whether every parsed reference resolved without a problem.
    pub fn is_clean(&self) -> bool {
        self.problems.is_empty()
    }
}

/// Parses every `@`-token in `text` into a [`ParsedReference`].
///
/// Tokens that fail to parse (a bare `@`, an unknown namespace, a malformed handle)
/// are skipped rather than erroring the whole pass, so one bad token does not hide
/// the rest. A leading `@` starts a token; the token runs until whitespace or a
/// character that cannot appear in a reference. The supported forms are loose
/// (`@bit`), typed (`@character.bit`), and field-scoped (`@bit.animation.jump`,
/// `@character.bit.animation.jump`).
#[tracing::instrument(level = "debug", skip(text), fields(len = text.len()))]
pub fn parse_references(text: &str) -> Vec<ParsedReference> {
    let bytes = text.as_bytes();
    let mut out = Vec::new();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] != b'@' {
            i += 1;
            continue;
        }
        let start = i;
        let mut j = i + 1;
        while j < bytes.len() && is_reference_byte(bytes[j]) {
            j += 1;
        }
        // The token is the text after `@`, up to but not including position `j`.
        let raw_body = &text[start + 1..j];
        if let Ok(parsed) = parse_token(raw_body, start, j, &text[start..j]) {
            out.push(parsed);
        }
        i = j.max(start + 1);
    }
    out
}

/// Whether `b` can be part of a reference token body (`a-z`, `0-9`, `_`, `.`).
fn is_reference_byte(b: u8) -> bool {
    b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'_' || b == b'.'
}

/// Parses one token body (the text after `@`) into a [`ParsedReference`].
fn parse_token(body: &str, start: usize, end: usize, raw: &str) -> Result<ParsedReference, ParseError> {
    if body.is_empty() {
        return Err(ParseError::Empty);
    }
    // Trim a trailing dot so `@bit.` parses as `@bit` (a dangling separator).
    let body = body.trim_end_matches('.');
    let mut segments = body.split('.');
    let first = segments.next().ok_or(ParseError::Empty)?;
    let rest: Vec<&str> = segments.collect();

    // If the first segment names a namespace and there is at least one more
    // segment, the reference is typed: `@<namespace>.<handle>[.scope...]`.
    if let Some(entry_type) = namespace_to_type(first) {
        let handle_str = rest.first().copied().ok_or_else(|| ParseError::UnknownNamespace(first.to_owned()))?;
        let handle = CodexHandle::new(handle_str)?;
        let scope = rest.iter().skip(1).map(|s| (*s).to_owned()).collect();
        return Ok(ParsedReference {
            handle,
            entry_type: Some(entry_type),
            scope,
            start,
            end,
            raw: raw.to_owned(),
        });
    }

    // Otherwise it is a loose reference whose first segment is the handle and whose
    // remaining segments are a field scope: `@<handle>[.scope...]`.
    let handle = CodexHandle::new(first)?;
    let scope = rest.iter().map(|s| (*s).to_owned()).collect();
    Ok(ParsedReference {
        handle,
        entry_type: None,
        scope,
        start,
        end,
        raw: raw.to_owned(),
    })
}

/// Maps a namespace string back to its [`EntryType`], or `None` if unknown.
fn namespace_to_type(namespace: &str) -> Option<EntryType> {
    EntryType::all().iter().copied().find(|t| t.namespace() == namespace)
}

/// Resolves one already-parsed reference against `codex`.
///
/// A typed reference constrains the match to the named [`EntryType`]; a loose
/// reference matches any type and reports [`ResolutionProblem::Ambiguous`] when more
/// than one entry claims the handle. A resolved-but-deprecated entry is reported as
/// [`ResolutionProblem::Deprecated`] with a suggested replacement rather than as a
/// clean hit, so the compiler and UI treat it as needing attention.
pub fn resolve_reference(codex: &Codex, parsed: &ParsedReference) -> Result<ResolvedReference, ResolutionProblem> {
    // Collect every entry whose primary handle or alias matches, in id order.
    let matches: Vec<CodexEntryId> = codex
        .entries()
        .iter()
        .filter(|(_, e)| {
            // An @-reference resolves to an entry's stable id here, never binding to the handle text.
            // That decouples the display vocabulary from the reference graph: renaming or aliasing a
            // handle changes only the token an author types, while every resolved reference keeps
            // pointing at the same entry. Match against the primary handle or any alias, then hand back
            // the id — downstream callers store and compare the id, not this string.
            let handle_matches = e.handle == parsed.handle || e.aliases.contains(&parsed.handle);
            let type_matches = parsed.entry_type.is_none_or(|t| e.entry_type == t);
            handle_matches && type_matches
        })
        .map(|(id, _)| *id)
        .collect();

    match matches.as_slice() {
        [] => Err(ResolutionProblem::Unresolved { reference: parsed.clone() }),
        [only] => {
            // One match. A deprecated hit is a problem, not a clean resolution.
            let Some(entry) = codex.entry(*only) else {
                return Err(ResolutionProblem::Unresolved { reference: parsed.clone() });
            };
            if entry.status == EntryStatus::Deprecated {
                let suggested = suggest_replacement(codex, *only);
                return Err(ResolutionProblem::Deprecated {
                    reference: parsed.clone(),
                    entry: *only,
                    suggested_replacement: suggested,
                });
            }
            Ok(ResolvedReference {
                parsed: parsed.clone(),
                entry: *only,
                status: entry.status,
            })
        }
        _ => Err(ResolutionProblem::Ambiguous {
            reference: parsed.clone(),
            candidates: matches,
        }),
    }
}

/// The first non-deprecated entry of the same type as `deprecated`, as a relink
/// suggestion (bible 6.7). Returns `None` when no candidate exists.
fn suggest_replacement(codex: &Codex, deprecated: CodexEntryId) -> Option<CodexEntryId> {
    let target_type = codex.entry(deprecated)?.entry_type;
    codex
        .entries()
        .iter()
        .find(|(id, e)| **id != deprecated && e.entry_type == target_type && e.status != EntryStatus::Deprecated)
        .map(|(id, _)| *id)
}

/// Parses and resolves every `@`-reference in `text` against `codex`.
///
/// This is the staged pipeline of bible 6.5: parse, resolve aliases and handles to
/// stable ids, and surface typed problems. Clean hits land in
/// [`ResolutionReport::resolved`]; unresolved, ambiguous, and deprecated tokens land
/// in [`ResolutionReport::problems`].
#[tracing::instrument(level = "debug", skip(codex, text))]
pub fn resolve_text(codex: &Codex, text: &str) -> ResolutionReport {
    let mut report = ResolutionReport::default();
    for parsed in parse_references(text) {
        match resolve_reference(codex, &parsed) {
            Ok(resolved) => report.resolved.push(resolved),
            Err(problem) => report.problems.push(problem),
        }
    }
    report
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::disallowed_methods, clippy::panic)]
mod tests {
    use super::*;
    use pixhaus_core::{AddCodexEntry, CodexEntryProto, Command, Document, SetEntryStatus};

    fn handle(s: &str) -> CodexHandle {
        CodexHandle::new(s).unwrap()
    }

    /// Builds a document with one entry of `entry_type` under `h`, returning its id.
    fn add(doc: &mut Document, h: &str, entry_type: EntryType) -> CodexEntryId {
        let proto = CodexEntryProto {
            handle: handle(h),
            name: h.to_owned(),
            entry_type,
        };
        let mut cmd = AddCodexEntry::new(proto);
        cmd.apply(doc).unwrap();
        cmd.inserted_id().expect("apply sets the id")
    }

    #[test]
    fn parses_loose_typed_and_scoped_forms() {
        let refs = parse_references("@bit jumping in @location.fungal_catacombs doing @bit.animation.jump");
        assert_eq!(refs.len(), 3);

        assert_eq!(refs[0].handle.as_str(), "bit");
        assert_eq!(refs[0].entry_type, None);
        assert!(refs[0].scope.is_empty());

        assert_eq!(refs[1].handle.as_str(), "fungal_catacombs");
        assert_eq!(refs[1].entry_type, Some(EntryType::Location));
        assert!(refs[1].scope.is_empty());

        assert_eq!(refs[2].handle.as_str(), "bit");
        assert_eq!(refs[2].entry_type, None);
        assert_eq!(refs[2].scope, vec!["animation".to_owned(), "jump".to_owned()]);
    }

    #[test]
    fn parses_typed_form_with_trailing_scope() {
        let refs = parse_references("@character.bit.animation.jump");
        assert_eq!(refs.len(), 1);
        assert_eq!(refs[0].entry_type, Some(EntryType::Character));
        assert_eq!(refs[0].handle.as_str(), "bit");
        assert_eq!(refs[0].scope, vec!["animation".to_owned(), "jump".to_owned()]);
    }

    #[test]
    fn skips_bare_at_and_records_byte_range() {
        let refs = parse_references("hi @ there @bit");
        assert_eq!(refs.len(), 1);
        assert_eq!(refs[0].handle.as_str(), "bit");
        let r = &refs[0];
        assert_eq!(&"hi @ there @bit"[r.start..r.end], "@bit");
    }

    #[test]
    fn resolves_loose_handle_and_alias() {
        let mut doc = Document::new();
        let id = add(&mut doc, "bit", EntryType::Character);
        // Resolve by primary handle.
        let report = resolve_text(doc.codex(), "@bit jumps");
        assert!(report.is_clean());
        assert_eq!(report.resolved[0].entry, id);
    }

    #[test]
    fn typed_reference_constrains_to_type() {
        let mut doc = Document::new();
        let _palette = add(&mut doc, "moonlit_ruins", EntryType::Palette);
        // A character-typed reference to a palette handle does not resolve.
        let report = resolve_text(doc.codex(), "@character.moonlit_ruins");
        assert_eq!(report.resolved.len(), 0);
        assert!(matches!(report.problems[0], ResolutionProblem::Unresolved { .. }));
        // The palette-typed form resolves.
        let ok = resolve_text(doc.codex(), "@palette.moonlit_ruins");
        assert!(ok.is_clean());
    }

    #[test]
    fn unresolved_reference_is_reported() {
        let doc = Document::new();
        let report = resolve_text(doc.codex(), "@nobody");
        assert!(!report.is_clean());
        assert!(matches!(report.problems[0], ResolutionProblem::Unresolved { .. }));
    }

    #[test]
    fn ambiguous_loose_reference_lists_candidates() {
        use crate::codex::test_support::with_alias;
        let mut doc = Document::new();
        // Two entries with distinct primary handles; the second also claims `slime`
        // as an alias, so a loose `@slime` matches both and is ambiguous.
        let a = add(&mut doc, "slime", EntryType::Enemy);
        let b = add(&mut doc, "blob", EntryType::Creature);
        let codex = with_alias(doc.codex(), b, "slime");
        let report = resolve_text(&codex, "@slime");
        match &report.problems[0] {
            ResolutionProblem::Ambiguous { candidates, .. } => {
                assert_eq!(candidates, &vec![a, b]);
            }
            other => panic!("expected ambiguous, got {other:?}"),
        }
    }

    #[test]
    fn resolves_by_alias() {
        use crate::codex::test_support::with_alias;
        let mut doc = Document::new();
        let bit = add(&mut doc, "bit", EntryType::Character);
        let codex = with_alias(doc.codex(), bit, "mascot");
        let report = resolve_text(&codex, "@mascot");
        assert!(report.is_clean());
        assert_eq!(report.resolved[0].entry, bit);
    }

    #[test]
    fn deprecated_reference_suggests_replacement() {
        let mut doc = Document::new();
        let old = add(&mut doc, "old_style", EntryType::Style);
        let new = add(&mut doc, "new_style", EntryType::Style);
        let mut dep = SetEntryStatus::new(old, EntryStatus::Deprecated);
        dep.apply(&mut doc).unwrap();
        let report = resolve_text(doc.codex(), "@old_style");
        match &report.problems[0] {
            ResolutionProblem::Deprecated {
                entry, suggested_replacement, ..
            } => {
                assert_eq!(*entry, old);
                assert_eq!(*suggested_replacement, Some(new));
            }
            other => panic!("expected deprecated, got {other:?}"),
        }
    }

    #[test]
    fn parse_token_rejects_empty_and_unknown_namespace() {
        assert_eq!(parse_token("", 0, 0, "@"), Err(ParseError::Empty));
        // A single unknown segment is treated as a loose handle, not a namespace.
        assert!(parse_token("bit", 0, 4, "@bit").is_ok());
        // `nope.bit` has an unknown leading segment, so it is a loose handle with a
        // scope, not a typed reference.
        let p = parse_token("nope.bit", 0, 9, "@nope.bit").unwrap();
        assert_eq!(p.handle.as_str(), "nope");
        assert_eq!(p.scope, vec!["bit".to_owned()]);
    }
}
