//! Codex search and `@`-autocomplete ranking (bible 13.1, 13.3, 26).
//!
//! Given a query, an optional type filter, and a small context hint, this ranks the
//! Codex entries that match by a fixed precedence: exact handle match first, then
//! fuzzy subsequence score, then canonical status, then type relevance to the
//! context. The fuzzy matcher is a hand-rolled subsequence scorer — no new crate —
//! that rewards contiguous runs and word-start hits the way an editor's quick-open
//! does.
//!
//! This is the services-side index the bible (12.3) calls for: it is computed from
//! `&Codex` on demand, never stored in `core`.

use std::cmp::Ordering;

use pixhaus_core::{Codex, CodexEntryId, EntryStatus, EntryType};

/// A small hint about where the search is happening, used only for tie-breaking
/// (bible 13.1 "entry type expected by field"). All fields are optional.
#[derive(Clone, PartialEq, Eq, Debug, Default)]
pub struct SearchContext {
    /// The entry type the field expects, if any. A matching entry ranks above a
    /// non-matching one when scores are otherwise equal.
    pub expected_type: Option<EntryType>,
}

/// One ranked autocomplete suggestion (bible 13.1).
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Suggestion {
    /// The matched entry.
    pub entry: CodexEntryId,
    /// The entry's primary handle, for inserting the chip text.
    pub handle: String,
    /// The entry's display name, for the suggestion row.
    pub name: String,
    /// The entry's type, for the type badge and color.
    pub entry_type: EntryType,
    /// The entry's status, so the UI can mark deprecated suggestions.
    pub status: EntryStatus,
    /// The fuzzy score that ranked this suggestion; higher is a better text match.
    pub score: i32,
    /// Whether the query exactly equals the handle (the top rank).
    pub exact: bool,
}

/// The maximum number of suggestions [`suggest`] returns by default.
pub const DEFAULT_SUGGESTION_LIMIT: usize = 8;

/// Ranks Codex entries for `query`, returning up to `limit` suggestions.
///
/// `query` is matched case-insensitively against each entry's handle, aliases, and
/// name. `type_filter` restricts the candidate set (the typed `@character:` form of
/// bible 13.2). `context` only breaks ties. An empty query returns every candidate
/// ranked by status then name, so an opened-but-empty autocomplete still lists the
/// project's entries.
#[tracing::instrument(level = "debug", skip(codex), fields(query, ?type_filter, limit))]
pub fn suggest(codex: &Codex, query: &str, type_filter: Option<EntryType>, context: &SearchContext, limit: usize) -> Vec<Suggestion> {
    let needle = query.trim().to_ascii_lowercase();
    let mut scored: Vec<Suggestion> = codex
        .entries()
        .values()
        .filter(|e| type_filter.is_none_or(|t| e.entry_type == t))
        .filter_map(|e| {
            let handle = e.handle.as_str();
            let exact = !needle.is_empty() && handle == needle;
            // The best text match across the handle, aliases, and name.
            let score = if needle.is_empty() {
                0
            } else {
                let mut best = fuzzy_score(handle, &needle);
                for alias in &e.aliases {
                    best = best.max(fuzzy_score(alias.as_str(), &needle));
                }
                best = best.max(fuzzy_score(&e.name.to_ascii_lowercase(), &needle));
                best?
            };
            Some(Suggestion {
                entry: e.id,
                handle: handle.to_owned(),
                name: e.name.clone(),
                entry_type: e.entry_type,
                status: e.status,
                score,
                exact,
            })
        })
        .collect();

    scored.sort_by(|a, b| rank(a, b, context));
    scored.truncate(limit);
    scored
}

/// Compares two suggestions by the bible 13.1 precedence: exact handle, then fuzzy
/// score (higher first), then canonical status, then type relevance, then name.
fn rank(a: &Suggestion, b: &Suggestion, context: &SearchContext) -> Ordering {
    // 1. Exact handle match wins.
    b.exact.cmp(&a.exact).then_with(|| {
        // 2. Higher fuzzy score first.
        b.score.cmp(&a.score).then_with(|| {
            // 3. Canonical before everything else, deprecated last.
            status_rank(a.status).cmp(&status_rank(b.status)).then_with(|| {
                // 4. Type relevance to the context.
                let a_rel = context.expected_type == Some(a.entry_type);
                let b_rel = context.expected_type == Some(b.entry_type);
                b_rel.cmp(&a_rel).then_with(|| {
                    // 5. Stable final tie-break by name then id.
                    a.name.cmp(&b.name).then_with(|| a.entry.0.cmp(&b.entry.0))
                })
            })
        })
    })
}

/// A sort key for status: canonical ranks first, deprecated last. Lower is better.
fn status_rank(status: EntryStatus) -> u8 {
    match status {
        EntryStatus::Canonical => 0,
        EntryStatus::Candidate => 1,
        EntryStatus::Draft => 2,
        EntryStatus::Archived => 3,
        EntryStatus::Rejected => 4,
        EntryStatus::Deprecated => 5,
    }
}

/// Scores how well `needle` matches `haystack` as a subsequence, or `None` if it is
/// not a subsequence at all.
///
/// Both inputs are expected lowercase. The score rewards a shorter haystack, a
/// contiguous run of matched characters, and matches at word starts (after `_` or a
/// digit boundary) — the heuristics that make `fung cat` rank `fungal_catacombs`.
/// Spaces in the needle are treated as gaps, so a multi-word query still matches.
pub fn fuzzy_score(haystack: &str, needle: &str) -> Option<i32> {
    if needle.is_empty() {
        return Some(0);
    }
    let hay: Vec<char> = haystack.chars().collect();
    // Drop spaces from the needle; they are author gaps, not characters to match.
    let need: Vec<char> = needle.chars().filter(|c| !c.is_whitespace()).collect();
    if need.is_empty() {
        return Some(0);
    }

    let mut score = 0i32;
    let mut h = 0usize;
    let mut prev_match: Option<usize> = None;
    for &nc in &need {
        // Advance through the haystack to the next occurrence of `nc`.
        let mut found = None;
        while h < hay.len() {
            if hay[h] == nc {
                found = Some(h);
                h += 1;
                break;
            }
            h += 1;
        }
        let pos = found?;
        // Base point for a matched character.
        score += 1;
        // Reward a contiguous run (the previous match sat immediately before this).
        if pos > 0 && prev_match == Some(pos - 1) {
            score += 4;
        }
        // Reward a word-start match (string start or after a separator).
        if pos == 0 || hay.get(pos - 1).is_some_and(|c| *c == '_' || c.is_whitespace()) {
            score += 3;
        }
        prev_match = Some(pos);
    }
    // Reward a tighter haystack so a short exact-ish handle beats a long one.
    // `32usize.saturating_sub(len)` is provably in 0..=32, so the i32 conversion never fails;
    // the `unwrap_or(0)` is an unreachable defensive fallback, not a real branch.
    score += i32::try_from(32usize.saturating_sub(hay.len())).unwrap_or(0);
    // A full prefix match is worth a strong bonus.
    if haystack.starts_with(&need.iter().collect::<String>()) {
        score += 20;
    }
    Some(score)
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::disallowed_methods, clippy::panic)]
mod tests {
    use super::*;
    use pixhaus_core::{AddCodexEntry, CodexEntryProto, CodexHandle, Command, Document, SetEntryStatus};

    fn add(doc: &mut Document, h: &str, name: &str, entry_type: EntryType) -> CodexEntryId {
        let proto = CodexEntryProto {
            handle: CodexHandle::new(h).unwrap(),
            name: name.to_owned(),
            entry_type,
        };
        let mut cmd = AddCodexEntry::new(proto);
        cmd.apply(doc).unwrap();
        cmd.inserted_id().unwrap()
    }

    #[test]
    fn fuzzy_matches_subsequence_and_rejects_non_subsequence() {
        assert!(fuzzy_score("fungal_catacombs", "fungcat").is_some());
        assert!(fuzzy_score("fungal_catacombs", "xyz").is_none());
    }

    #[test]
    fn fuzzy_prefers_contiguous_and_word_start() {
        let contiguous = fuzzy_score("jumping", "jump").unwrap();
        let scattered = fuzzy_score("jacuzzi_pump", "jump").unwrap();
        assert!(contiguous > scattered, "contiguous {contiguous} should beat scattered {scattered}");
    }

    #[test]
    fn exact_handle_ranks_first() {
        let mut doc = Document::new();
        let _jumper = add(&mut doc, "jumper", "Jumper", EntryType::Animation);
        let jump = add(&mut doc, "jump", "Jump", EntryType::Animation);
        let out = suggest(doc.codex(), "jump", None, &SearchContext::default(), DEFAULT_SUGGESTION_LIMIT);
        assert_eq!(out[0].entry, jump);
        assert!(out[0].exact);
    }

    #[test]
    fn type_filter_restricts_candidates() {
        let mut doc = Document::new();
        let _anim = add(&mut doc, "jump", "Jump", EntryType::Animation);
        let _loc = add(&mut doc, "jungle", "Jungle", EntryType::Location);
        let out = suggest(
            doc.codex(),
            "ju",
            Some(EntryType::Location),
            &SearchContext::default(),
            DEFAULT_SUGGESTION_LIMIT,
        );
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].handle, "jungle");
    }

    #[test]
    fn canonical_outranks_draft_on_equal_score() {
        let mut doc = Document::new();
        let draft = add(&mut doc, "walk_a", "Walk", EntryType::Animation);
        let canon = add(&mut doc, "walk_b", "Walk", EntryType::Animation);
        let mut set = SetEntryStatus::new(canon, EntryStatus::Canonical);
        set.apply(&mut doc).unwrap();
        // Query "walk" matches both names equally; canonical should sort first.
        let out = suggest(doc.codex(), "walk", None, &SearchContext::default(), DEFAULT_SUGGESTION_LIMIT);
        let canon_pos = out.iter().position(|s| s.entry == canon).unwrap();
        let draft_pos = out.iter().position(|s| s.entry == draft).unwrap();
        assert!(canon_pos < draft_pos);
    }

    #[test]
    fn context_type_breaks_ties() {
        let mut doc = Document::new();
        // Two entries with identical handles-as-text is impossible, so make their
        // fuzzy scores equal via identical handles in different types is also
        // impossible; instead use the same name to force equal scoring on name.
        let anim = add(&mut doc, "dash_x", "Dash", EntryType::Animation);
        let pose = add(&mut doc, "dash_y", "Dash", EntryType::Pose);
        let ctx = SearchContext {
            expected_type: Some(EntryType::Pose),
        };
        let out = suggest(doc.codex(), "dash", None, &ctx, DEFAULT_SUGGESTION_LIMIT);
        let pose_pos = out.iter().position(|s| s.entry == pose).unwrap();
        let anim_pos = out.iter().position(|s| s.entry == anim).unwrap();
        assert!(pose_pos < anim_pos);
    }

    #[test]
    fn empty_query_lists_everything() {
        let mut doc = Document::new();
        add(&mut doc, "bit", "Bit", EntryType::Character);
        add(&mut doc, "byte", "Byte", EntryType::Character);
        let out = suggest(doc.codex(), "", None, &SearchContext::default(), DEFAULT_SUGGESTION_LIMIT);
        assert_eq!(out.len(), 2);
    }

    #[test]
    fn limit_truncates() {
        let mut doc = Document::new();
        for i in 0..10 {
            add(&mut doc, &format!("e_{i}"), "E", EntryType::Prop);
        }
        let out = suggest(doc.codex(), "", None, &SearchContext::default(), 3);
        assert_eq!(out.len(), 3);
    }
}
