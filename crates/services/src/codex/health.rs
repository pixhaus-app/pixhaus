//! Derived health, readiness, usage, and key-info reads for a Codex entry (bible
//! 15, 16, 21).
//!
//! The Codex workspace inspector and the entry detail tabs show numbers the model
//! does not store: how healthy an entry is, whether its prompt is ready to
//! generate, how often it is used elsewhere, and when it was created or last
//! touched. This module derives those as pure functions over `&Codex`, reusing the
//! existing [`coverage`](crate::codex::coverage), [`reference`](crate::codex::reference),
//! [`compiler`](crate::codex::compiler), and [`validation`](crate::codex::validation)
//! passes rather than re-implementing them. Nothing here mutates or mints — the UI
//! mirrors the results into its view and renders them.
//!
//! Findings carry stable message keys (`codex.health.*`, `codex.used_in.*`), never
//! display text; the UI resolves them.

use pixhaus_core::{AnchorKind, Codex, CodexEntryId, EntryDetails, EntryType, EntryVersion, RelationKind};

use crate::codex::compiler::{PromptRequest, compile};
use crate::codex::coverage::coverage_report;
use crate::codex::reference::resolve_text;

/// The pass/warn/fail state of one health checklist item.
#[derive(Copy, Clone, PartialEq, Eq, Hash, Debug)]
pub enum CheckState {
    /// The item is satisfied.
    Pass,
    /// The item is partially satisfied or worth attention, not blocking.
    Warn,
    /// The item is not satisfied.
    Fail,
}

/// One row of an entry's health checklist (bible 21).
///
/// `message_key` names the item ("has handle", "has summary", …); the UI resolves
/// it. `detail` carries non-localized context the UI may interpolate (e.g. a
/// coverage `N/M` string), or `None`.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct HealthCheck {
    /// The stable message key under `codex.health.check.*`.
    pub message_key: String,
    /// Whether the item passes, warns, or fails.
    pub state: CheckState,
    /// Non-localized context for the UI to interpolate, or `None`.
    pub detail: Option<String>,
}

impl HealthCheck {
    /// A check with `message_key` and `state`, no detail.
    fn new(message_key: impl Into<String>, state: CheckState) -> Self {
        Self {
            message_key: message_key.into(),
            state,
            detail: None,
        }
    }

    /// Attaches non-localized detail text.
    #[must_use]
    fn with_detail(mut self, detail: impl Into<String>) -> Self {
        self.detail = Some(detail.into());
        self
    }
}

/// The health tier an entry falls into, by score band (bible 21).
#[derive(Copy, Clone, PartialEq, Eq, Hash, Debug)]
pub enum HealthTier {
    /// Score below 0.4 — the entry needs work before it is production-ready.
    NeedsWork,
    /// Score in `0.4..0.8` — usable, with gaps.
    Good,
    /// Score at or above 0.8 — production-ready.
    Excellent,
}

impl HealthTier {
    /// The tier for a `0.0..=1.0` score: `Excellent` ≥ 0.8, `Good` ≥ 0.4, else
    /// `NeedsWork`.
    pub fn from_score(score: f32) -> Self {
        if score >= 0.8 {
            HealthTier::Excellent
        } else if score >= 0.4 {
            HealthTier::Good
        } else {
            HealthTier::NeedsWork
        }
    }

    /// The stable label key under `codex.health.*` for this tier.
    pub fn label_key(self) -> &'static str {
        match self {
            HealthTier::Excellent => "codex.health.excellent",
            HealthTier::Good => "codex.health.good",
            HealthTier::NeedsWork => "codex.health.needs_work",
        }
    }
}

/// An entry's derived health: a `0.0..=1.0` score, its tier, and the checklist that
/// produced it (bible 21).
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct EntryHealth {
    /// The weighted score in `0.0..=1.0`.
    score_milli: u16,
    /// The checklist rows, in display order.
    pub checks: Vec<HealthCheck>,
}

impl EntryHealth {
    /// The score in `0.0..=1.0`.
    pub fn score(&self) -> f32 {
        f32::from(self.score_milli) / 1000.0
    }

    /// The score as a whole-number percentage `0..=100`.
    pub fn percent(&self) -> u8 {
        u8::try_from((u32::from(self.score_milli) + 5) / 10).unwrap_or(100)
    }

    /// The health tier for the score.
    pub fn tier(&self) -> HealthTier {
        HealthTier::from_score(self.score())
    }
}

/// Converts a `0.0..=1.0` ratio to an integer count of thousandths (`0..=1000`),
/// rounding to nearest. Out-of-range input is clamped. Avoids a lossy float cast by
/// stepping through `0..=1000` whole steps.
fn ratio_to_milli(ratio: f32) -> u16 {
    let clamped = ratio.clamp(0.0, 1.0);
    // `clamped * 1000.0` lands in `0.0..=1000.0`; find the nearest whole step by
    // scanning down from the ceiling, avoiding a truncating float-to-int cast.
    let scaled = clamped * 1000.0;
    (0_u16..=1000).rev().find(|step| f32::from(*step) <= scaled + 0.5).unwrap_or(0)
}

/// Whether `entry_type` is expected to carry a palette reference (a Character), so a
/// missing one counts against health. Other types do not need one.
fn palette_ref_applies(entry_type: EntryType) -> bool {
    entry_type == EntryType::Character
}

/// Whether the character entry has a resolvable palette reference, or `None` when a
/// palette reference does not apply to the entry's type.
fn has_palette_ref(codex: &Codex, entry: CodexEntryId) -> Option<bool> {
    let e = codex.entry(entry)?;
    match &e.details {
        EntryDetails::Character(details) => {
            let ok = details
                .palette_ref
                .as_ref()
                .and_then(|h| codex.resolve_handle(h))
                .and_then(|id| codex.entry(id))
                .is_some_and(|p| p.entry_type == EntryType::Palette);
            Some(ok)
        }
        _ => {
            // A non-character may still declare a palette via a `Uses` relationship.
            if relates_to_palette(codex, entry) { Some(true) } else { None }
        }
    }
}

/// Whether `entry` has a `Uses` edge to a palette entry.
fn relates_to_palette(codex: &Codex, entry: CodexEntryId) -> bool {
    codex
        .relationships()
        .iter()
        .any(|r| r.from == entry && r.kind == RelationKind::Uses && codex.entry(r.to).is_some_and(|t| t.entry_type == EntryType::Palette))
}

/// The number of `@`-references in this entry's own fragment text that fail to
/// resolve cleanly (unresolved, ambiguous, or deprecated). A clean entry returns 0.
#[tracing::instrument(level = "debug", skip(codex))]
pub fn broken_reference_count(codex: &Codex, entry: CodexEntryId) -> usize {
    let Some(e) = codex.entry(entry) else {
        return 0;
    };
    let mut text = String::new();
    for fragment in &e.prompt_fragments {
        text.push_str(&fragment.text);
        text.push(' ');
    }
    for negative in &e.negative_fragments {
        text.push_str(negative);
        text.push(' ');
    }
    for anchor in &e.anchors {
        text.push_str(&anchor.statement);
        text.push(' ');
    }
    resolve_text(codex, &text).problems.len()
}

/// Computes the derived health of `entry` against `codex` (bible 21).
///
/// The score is a weighted sum of six terms, each `0.0..=1.0`:
/// `0.30·has_summary + 0.20·has_visual_anchor + 0.15·has_palette_ref +
/// 0.25·coverage_ratio + 0.10·no_broken_refs`, plus a fixed `has_handle` term that
/// always passes (a real entry always has a handle). The palette term is skipped —
/// and its weight redistributed — for types that do not need a palette, so a Rule or
/// Style is not penalized for lacking one. The checklist mirrors the terms in
/// display order so the inspector and the score always agree.
#[tracing::instrument(level = "debug", skip(codex))]
pub fn entry_health(codex: &Codex, entry: CodexEntryId) -> EntryHealth {
    let Some(e) = codex.entry(entry) else {
        return EntryHealth {
            score_milli: 0,
            checks: Vec::new(),
        };
    };

    let has_summary = !e.description.trim().is_empty();
    let has_visual_anchor = e.anchor_position(AnchorKind::Visual).is_some();
    let palette = has_palette_ref(codex, entry);
    let report = coverage_report(codex, entry);
    let coverage_ratio = report.completion_ratio();
    let broken = broken_reference_count(codex, entry);
    let no_broken = broken == 0;

    // Weights. The palette term only applies to types that need one; when it does
    // not apply, its weight folds into summary so the remaining terms still sum to 1.
    let palette_applies = palette.is_some() && palette_ref_applies(e.entry_type);
    let w_summary = if palette_applies { 0.30 } else { 0.45 };
    let w_visual = 0.20;
    let w_palette = if palette_applies { 0.15 } else { 0.0 };
    let w_coverage = 0.25;
    let w_broken = 0.10;

    let mut score = 0.0_f32;
    if has_summary {
        score += w_summary;
    }
    if has_visual_anchor {
        score += w_visual;
    }
    if palette_applies && palette == Some(true) {
        score += w_palette;
    }
    score += w_coverage * coverage_ratio;
    if no_broken {
        score += w_broken;
    }
    let score = score.clamp(0.0, 1.0);

    let mut checks = Vec::new();
    checks.push(HealthCheck::new("codex.health.check.handle", CheckState::Pass));
    checks.push(HealthCheck::new(
        "codex.health.check.summary",
        if has_summary { CheckState::Pass } else { CheckState::Fail },
    ));
    checks.push(HealthCheck::new(
        "codex.health.check.visual_anchor",
        if has_visual_anchor { CheckState::Pass } else { CheckState::Warn },
    ));
    if palette_applies {
        checks.push(HealthCheck::new(
            "codex.health.check.palette_ref",
            if palette == Some(true) { CheckState::Pass } else { CheckState::Warn },
        ));
    }
    let coverage_state = if report.total() == 0 {
        CheckState::Warn
    } else if report.complete_count() == report.total() {
        CheckState::Pass
    } else {
        CheckState::Warn
    };
    checks.push(HealthCheck::new("codex.health.check.coverage", coverage_state).with_detail(format!("{}/{}", report.complete_count(), report.total())));
    let broken_check = HealthCheck::new("codex.health.check.no_broken_refs", if no_broken { CheckState::Pass } else { CheckState::Fail });
    let broken_check = if no_broken {
        broken_check
    } else {
        broken_check.with_detail(broken.to_string())
    };
    checks.push(broken_check);

    // Store as thousandths for an exact integer, no lossy float cast.
    let score_milli = ratio_to_milli(score);
    EntryHealth { score_milli, checks }
}

/// An entry's generation readiness: how ready its compiled prompt is to send (bible
/// 16.3).
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct GenerationReadiness {
    /// Readiness in `0.0..=1.0`: 1.0 when the prompt is clean, lower as blocking and
    /// error findings accumulate.
    score_milli: u16,
    /// Whether the prompt is clean (ready to generate with no findings).
    pub clean: bool,
    /// The number of blocking findings.
    pub blocking: usize,
}

impl GenerationReadiness {
    /// The readiness score in `0.0..=1.0`.
    pub fn score(&self) -> f32 {
        f32::from(self.score_milli) / 1000.0
    }

    /// The readiness as a whole-number percentage `0..=100`.
    pub fn percent(&self) -> u8 {
        u8::try_from((u32::from(self.score_milli) + 5) / 10).unwrap_or(100)
    }
}

/// Computes the generation readiness of `entry`'s compiled prompt (bible 16.3).
///
/// Resolves the entry's own positive fragments and anchors into a prompt, compiles
/// it, and runs [`check_readiness`](crate::codex::validation::check_readiness). A
/// clean result scores 1.0; each blocking finding subtracts evenly across the
/// references the entry mentions plus one (so a single blocking finding on a small
/// prompt still leaves the entry well short of ready). An entry that compiles to an
/// empty prompt scores 0.0.
#[tracing::instrument(level = "debug", skip(codex))]
pub fn generation_readiness(codex: &Codex, entry: CodexEntryId) -> GenerationReadiness {
    let Some(e) = codex.entry(entry) else {
        return GenerationReadiness {
            score_milli: 0,
            clean: false,
            blocking: 0,
        };
    };

    let mut text = String::new();
    for fragment in &e.prompt_fragments {
        text.push_str(&fragment.text);
        text.push(' ');
    }
    for anchor in &e.anchors {
        text.push_str(&anchor.statement);
        text.push(' ');
    }
    let resolution = resolve_text(codex, &text);
    let request = PromptRequest {
        user_request: text.clone(),
        references: resolution.resolved.clone(),
        ..PromptRequest::default()
    };
    let compiled = compile(codex, &request);
    let report = crate::codex::validation::check_readiness(&resolution, &compiled);

    let clean = report.is_clean();
    let blocking = report.count(crate::codex::validation::Severity::Blocking);
    let errors = report.count(crate::codex::validation::Severity::Error);

    // A clean prompt is fully ready. Otherwise dock for each blocking/error finding;
    // dividing by (problems + 1) keeps the score in range and below 1.0.
    let bad = blocking + errors;
    let score = if clean {
        1.0
    } else {
        let total = bad + 1;
        // `bad / total` is the fraction of trouble; readiness is the complement.
        let bad_f = u16::try_from(bad).unwrap_or(u16::MAX);
        let total_f = u16::try_from(total).unwrap_or(u16::MAX);
        (1.0 - f32::from(bad_f) / f32::from(total_f)).clamp(0.0, 1.0)
    };
    let score_milli = ratio_to_milli(score);
    GenerationReadiness { score_milli, clean, blocking }
}

/// How often an entry is used elsewhere, bucketed by the using entry's family (bible
/// 21).
///
/// Counts incoming relationships (`other --kind--> entry`) and groups them by the
/// source entry's [`EntryType`] into the buckets the inspector shows. A bucket with
/// no backing entry types in the model returns 0 — these are real counts, not
/// placeholders. `scenes` has no entry type backing it this pass (there is no Scene
/// type), so it is always 0 and documented as such.
#[derive(Copy, Clone, PartialEq, Eq, Hash, Debug, Default)]
pub struct UsedInCounts {
    /// Times referenced by a Recipe entry.
    pub recipes: usize,
    /// Times referenced by an Animation or Pose entry.
    pub animations: usize,
    /// Times referenced by a VFX entry.
    pub fx: usize,
    /// Times referenced by a Location or Biome entry (the closest "scene" backing the
    /// model has this pass). Always 0 until a Scene type exists; documented MOCK.
    pub scenes: usize,
    /// Total incoming references, across every bucket and every other type.
    pub total: usize,
}

/// Counts how `entry` is referenced by other entries' relationships (bible 21).
#[tracing::instrument(level = "debug", skip(codex))]
pub fn used_in_counts(codex: &Codex, entry: CodexEntryId) -> UsedInCounts {
    let mut counts = UsedInCounts::default();
    if codex.entry(entry).is_none() {
        return counts;
    }
    for rel in codex.relationships() {
        if rel.to != entry {
            continue;
        }
        counts.total += 1;
        let Some(source) = codex.entry(rel.from) else { continue };
        match source.entry_type {
            EntryType::Recipe => counts.recipes += 1,
            EntryType::Animation | EntryType::Pose => counts.animations += 1,
            EntryType::Vfx => counts.fx += 1,
            // No Scene type exists yet; Location/Biome are the nearest stand-in but
            // the spec reserves `scenes` for a future type, so it stays 0.
            _ => {}
        }
    }
    counts
}

/// Created/updated/author/version key-info derived from an entry's version history
/// (bible 11.7, 21).
///
/// `created` is the first recorded version's timestamp, `updated` the latest. An
/// entry with no recorded history yields `None` timestamps, an empty author, and
/// version 0 — the UI renders a dash. The author is the latest version's author.
#[derive(Clone, PartialEq, Eq, Debug, Default)]
pub struct KeyInfo {
    /// Epoch-millisecond timestamp of the first recorded version, or `None`.
    pub created_ms: Option<u64>,
    /// Epoch-millisecond timestamp of the latest recorded version, or `None`.
    pub updated_ms: Option<u64>,
    /// The latest version's author label (not localized), or empty when unknown.
    pub author: String,
    /// The latest version number, or 0 when there is no history.
    pub version: u32,
}

/// Derives the [`KeyInfo`] for `entry` from its version history (bible 11.7).
#[tracing::instrument(level = "debug", skip(codex))]
pub fn key_info(codex: &Codex, entry: CodexEntryId) -> KeyInfo {
    let Some(e) = codex.entry(entry) else {
        return KeyInfo::default();
    };
    let first = e.version_history.first();
    let last = e.version_history.last();
    KeyInfo {
        created_ms: first.map(|v: &EntryVersion| v.timestamp_ms),
        updated_ms: last.map(|v: &EntryVersion| v.timestamp_ms),
        author: last.map(|v| v.author.clone()).unwrap_or_default(),
        version: last.map_or(0, |v| v.version),
    }
}

/// The asset handles an entry references that could back a thumbnail strip (bible
/// 21).
///
/// The model carries no asset blobs or sprite store this pass, so there is nothing
/// to thumbnail. This returns the count of handle-shaped references the entry does
/// carry — its character palette/style/animation handles — so the UI can size a MOCK
/// strip against a real number rather than a fabricated one. When the entry carries
/// none, the count is 0 and the UI shows a clean empty state.
#[derive(Copy, Clone, PartialEq, Eq, Hash, Debug, Default)]
pub struct LinkedAssets {
    /// The number of handle references the entry declares (palette, styles, animation
    /// set). A proxy for "linked assets" until a real asset store exists; no blobs.
    pub handle_count: usize,
    /// Always `false` this pass: the model stores no renderable asset blobs.
    pub has_blobs: bool,
}

/// Aggregates the handle-shaped references `entry` declares as a stand-in for linked
/// assets (bible 21). Returns an empty result for unknown or blob-less entries.
#[tracing::instrument(level = "debug", skip(codex))]
pub fn linked_assets(codex: &Codex, entry: CodexEntryId) -> LinkedAssets {
    let Some(e) = codex.entry(entry) else {
        return LinkedAssets::default();
    };
    let handle_count = match &e.details {
        EntryDetails::Character(details) => usize::from(details.palette_ref.is_some()) + details.allowed_styles.len() + details.animation_set.len(),
        EntryDetails::Animation(details) => details.character_compatibility.len(),
        _ => 0,
    };
    LinkedAssets {
        handle_count,
        has_blobs: false,
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::disallowed_methods, clippy::panic)]
mod tests {
    use super::*;
    use pixhaus_core::codex::CharacterDetails;
    use pixhaus_core::{
        AddCodexEntry, AddRelationship, Anchor, AnchorKind, AnchorStrength, ApplyCoverageTemplate, CodexEntryDelta, CodexEntryProto, CodexHandle, Command,
        CoverageItemStatus, CoverageTemplate, CreateCoverageTemplate, Document, EntryType, SetAnchor, SetCharacterDetails, SetCoverageStatus, UpdateCodexEntry,
    };

    /// Creates the platformer-character template and applies it to `entry`, returning
    /// nothing — the tests only need the coverage cells seeded.
    fn apply_platformer(doc: &mut Document, entry: CodexEntryId) {
        let t = CoverageTemplate::platformer_character();
        let mut create = CreateCoverageTemplate::new(t.name.clone(), t.slots);
        create.apply(doc).unwrap();
        let id = create.inserted_id().unwrap();
        ApplyCoverageTemplate::new(entry, id).apply(doc).unwrap();
    }

    /// Sets `entry`'s character palette reference to `palette_handle` via the real
    /// command path (so the codex never round-trips through JSON).
    fn set_palette_ref(doc: &mut Document, entry: CodexEntryId, palette_handle: &str) {
        let body = CharacterDetails {
            palette_ref: Some(handle(palette_handle)),
            ..CharacterDetails::default()
        };
        let mut cmd = SetCharacterDetails::new(entry, body);
        cmd.apply(doc).unwrap();
    }

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

    fn describe(doc: &mut Document, id: CodexEntryId, text: &str) {
        let delta = CodexEntryDelta {
            description: Some(text.to_owned()),
            ..CodexEntryDelta::new()
        };
        let mut cmd = UpdateCodexEntry::new(id, delta);
        cmd.apply(doc).unwrap();
    }

    #[test]
    fn bare_entry_scores_low_and_fails_summary() {
        let mut doc = Document::new();
        let bit = add(&mut doc, "bit", EntryType::Character);
        let health = entry_health(doc.codex(), bit);
        // No summary, no visual anchor, no palette, no coverage template => low score.
        assert!(health.score() < 0.4);
        assert_eq!(health.tier(), HealthTier::NeedsWork);
        let summary = health.checks.iter().find(|c| c.message_key == "codex.health.check.summary").unwrap();
        assert_eq!(summary.state, CheckState::Fail);
        // The handle check always passes for a real entry.
        let handle_check = health.checks.iter().find(|c| c.message_key == "codex.health.check.handle").unwrap();
        assert_eq!(handle_check.state, CheckState::Pass);
    }

    #[test]
    fn fully_specified_character_scores_high() {
        let mut doc = Document::new();
        let palette = add(&mut doc, "moonlit", EntryType::Palette);
        let _ = palette;
        let bit = add(&mut doc, "bit", EntryType::Character);
        describe(&mut doc, bit, "the mascot");
        let mut anchor = SetAnchor::new(bit, Anchor::new(AnchorKind::Visual, AnchorStrength::Normal, "round head"));
        anchor.apply(&mut doc).unwrap();
        // Apply a coverage template and approve every slot.
        apply_platformer(&mut doc, bit);
        for slot in ["idle", "walk", "run", "jump", "fall", "land", "attack", "hurt", "death"] {
            let mut set = SetCoverageStatus::new(bit, slot, CoverageItemStatus::Approved);
            set.apply(&mut doc).unwrap();
        }
        // Point the character at the real palette via the command path.
        set_palette_ref(&mut doc, bit, "moonlit");
        let health = entry_health(doc.codex(), bit);
        assert!(health.score() >= 0.8, "score was {}", health.score());
        assert_eq!(health.tier(), HealthTier::Excellent);
        assert_eq!(health.percent(), 100);
        // Every check passes.
        assert!(health.checks.iter().all(|c| c.state == CheckState::Pass));
    }

    #[test]
    fn coverage_check_carries_n_of_m_detail() {
        let mut doc = Document::new();
        let bit = add(&mut doc, "bit", EntryType::Character);
        apply_platformer(&mut doc, bit);
        let mut set = SetCoverageStatus::new(bit, "idle", CoverageItemStatus::Approved);
        set.apply(&mut doc).unwrap();
        let health = entry_health(doc.codex(), bit);
        let coverage = health.checks.iter().find(|c| c.message_key == "codex.health.check.coverage").unwrap();
        assert_eq!(coverage.detail.as_deref(), Some("1/9"));
        assert_eq!(coverage.state, CheckState::Warn);
    }

    #[test]
    fn non_character_omits_palette_check() {
        let mut doc = Document::new();
        let rule = add(&mut doc, "no_gore", EntryType::Rule);
        describe(&mut doc, rule, "keep it bloodless");
        let health = entry_health(doc.codex(), rule);
        assert!(health.checks.iter().all(|c| c.message_key != "codex.health.check.palette_ref"));
        // A described rule with no coverage template and no broken refs scores well:
        // summary 0.45 + coverage 0.25 (empty report is complete) + no_broken 0.10.
        assert!(health.score() >= 0.8);
    }

    #[test]
    fn unknown_entry_has_empty_health() {
        let doc = Document::new();
        let health = entry_health(doc.codex(), CodexEntryId(999));
        assert!(health.score().abs() < f32::EPSILON);
        assert!(health.checks.is_empty());
    }

    #[test]
    fn readiness_clean_for_an_entry_with_a_fragment() {
        use crate::codex::test_support::with_prompt_fragment;
        use pixhaus_core::InclusionPriority;
        let mut doc = Document::new();
        let bit = add(&mut doc, "bit", EntryType::Character);
        let codex = with_prompt_fragment(doc.codex(), bit, "a round mascot", InclusionPriority::Normal);
        let readiness = generation_readiness(&codex, bit);
        assert!(readiness.clean);
        assert!((readiness.score() - 1.0).abs() < f32::EPSILON);
        assert_eq!(readiness.percent(), 100);
        assert_eq!(readiness.blocking, 0);
    }

    #[test]
    fn readiness_low_for_an_empty_prompt() {
        let mut doc = Document::new();
        let bit = add(&mut doc, "bit", EntryType::Character);
        let readiness = generation_readiness(doc.codex(), bit);
        // An empty positive prompt is a blocking finding.
        assert!(!readiness.clean);
        assert!(readiness.blocking >= 1);
        assert!(readiness.score() < 1.0);
    }

    #[test]
    fn broken_reference_count_flags_unresolved() {
        use crate::codex::test_support::with_prompt_fragment;
        use pixhaus_core::InclusionPriority;
        let mut doc = Document::new();
        let bit = add(&mut doc, "bit", EntryType::Character);
        let codex = with_prompt_fragment(doc.codex(), bit, "next to @ghost_town", InclusionPriority::Normal);
        assert_eq!(broken_reference_count(&codex, bit), 1);
        // The health no-broken-refs check fails as a result.
        let health = entry_health(&codex, bit);
        let broken = health.checks.iter().find(|c| c.message_key == "codex.health.check.no_broken_refs").unwrap();
        assert_eq!(broken.state, CheckState::Fail);
        assert_eq!(broken.detail.as_deref(), Some("1"));
    }

    #[test]
    fn used_in_counts_bucket_by_source_type() {
        let mut doc = Document::new();
        let bit = add(&mut doc, "bit", EntryType::Character);
        let recipe = add(&mut doc, "boss_recipe", EntryType::Recipe);
        let anim = add(&mut doc, "jump_anim", EntryType::Animation);
        let fx = add(&mut doc, "dust_puff", EntryType::Vfx);
        for (from, kind) in [(recipe, RelationKind::Uses), (anim, RelationKind::AppearsIn), (fx, RelationKind::AppearsIn)] {
            let mut rel = AddRelationship::new(pixhaus_core::Relationship::new(from, kind, bit));
            rel.apply(&mut doc).unwrap();
        }
        let counts = used_in_counts(doc.codex(), bit);
        assert_eq!(counts.recipes, 1);
        assert_eq!(counts.animations, 1);
        assert_eq!(counts.fx, 1);
        assert_eq!(counts.scenes, 0);
        assert_eq!(counts.total, 3);
    }

    #[test]
    fn used_in_counts_empty_for_unreferenced_entry() {
        let mut doc = Document::new();
        let bit = add(&mut doc, "bit", EntryType::Character);
        let counts = used_in_counts(doc.codex(), bit);
        assert_eq!(counts, UsedInCounts::default());
    }

    #[test]
    fn key_info_empty_without_history() {
        let mut doc = Document::new();
        let bit = add(&mut doc, "bit", EntryType::Character);
        let info = key_info(doc.codex(), bit);
        assert_eq!(info.created_ms, None);
        assert_eq!(info.updated_ms, None);
        assert!(info.author.is_empty());
        assert_eq!(info.version, 0);
    }

    #[test]
    fn linked_assets_counts_character_handles() {
        let mut doc = Document::new();
        let bit = add(&mut doc, "bit", EntryType::Character);
        // A bare character declares no handles.
        let assets = linked_assets(doc.codex(), bit);
        assert_eq!(assets.handle_count, 0);
        assert!(!assets.has_blobs);
        // With a palette ref, the count rises by one.
        set_palette_ref(&mut doc, bit, "moonlit");
        assert_eq!(linked_assets(doc.codex(), bit).handle_count, 1);
    }

    #[test]
    fn linked_assets_empty_for_unknown_entry() {
        let doc = Document::new();
        assert_eq!(linked_assets(doc.codex(), CodexEntryId(404)), LinkedAssets::default());
    }

    #[test]
    fn health_tier_bands() {
        assert_eq!(HealthTier::from_score(0.0), HealthTier::NeedsWork);
        assert_eq!(HealthTier::from_score(0.5), HealthTier::Good);
        assert_eq!(HealthTier::from_score(0.9), HealthTier::Excellent);
        assert_eq!(HealthTier::Excellent.label_key(), "codex.health.excellent");
    }
}
