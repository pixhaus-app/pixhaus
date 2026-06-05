//! The prompt compiler (bible 7.1-7.5, 14.5).
//!
//! A final generation request is composed from layers: the user request, the
//! resolved Codex references (their prompt fragments), the anchors those references
//! carry, project-wide rules, and negative constraints. The compiler assembles them
//! into an inspectable [`CompiledPrompt`] honoring two budgets — the field's
//! [`InclusionPriority`] and a caller-set character budget — dropping the lowest
//! priority first when over budget.
//!
//! Anchors fold in by strength: a [`Locked`](pixhaus_core::AnchorStrength::Locked) or
//! [`Strong`](pixhaus_core::AnchorStrength::Strong) anchor is treated as critical and
//! never dropped; weaker anchors compete for budget like any normal fragment.
//!
//! The compiler is a pure read over `&Codex` and the resolved references; it mints
//! nothing and mutates nothing.

use pixhaus_core::{AnchorKind, AnchorStrength, Codex, CodexEntryId, EntryType, InclusionPriority};

use crate::codex::reference::{ParsedReference, ResolvedReference};

/// The layers a final prompt is built from (bible 7.2), in the request the caller
/// hands the compiler.
#[derive(Clone, PartialEq, Eq, Debug, Default)]
pub struct PromptRequest {
    /// The raw user request text (with its `@`-mentions left in place).
    pub user_request: String,
    /// The references resolved from the user request, in order.
    pub references: Vec<ResolvedReference>,
    /// Project-wide positive rules to append (e.g. a house style line).
    pub project_rules: Vec<String>,
    /// Project-wide negative constraints to append.
    pub project_negatives: Vec<String>,
    /// The character budget for the positive prompt. `None` means no limit.
    pub budget: Option<usize>,
}

/// One fragment that survived compilation, with where it came from (bible 14.5).
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct IncludedFragment {
    /// The fragment text.
    pub text: String,
    /// The entry it came from, or `None` for the user request and project rules.
    pub source: Option<CodexEntryId>,
    /// The priority the fragment was weighed at.
    pub priority: InclusionPriority,
}

/// One anchor folded into the prompt (bible 7.4 "included anchors").
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct IncludedAnchor {
    /// The entry the anchor belongs to.
    pub source: CodexEntryId,
    /// What the anchor pins down.
    pub kind: AnchorKind,
    /// How firmly it holds.
    pub strength: AnchorStrength,
    /// The anchor's statement, as emitted into the prompt.
    pub statement: String,
}

/// A fragment that was dropped to fit the budget (bible 14.5 "excluded optional
/// fields").
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct ExcludedFragment {
    /// The fragment text that did not make it in.
    pub text: String,
    /// The entry it came from, or `None`.
    pub source: Option<CodexEntryId>,
    /// The priority it was weighed at.
    pub priority: InclusionPriority,
}

/// The inspectable result of compiling a [`PromptRequest`] (bible 7.3, 14.5).
#[derive(Clone, PartialEq, Eq, Debug, Default)]
pub struct CompiledPrompt {
    /// The final positive prompt, ready to send to a provider.
    pub positive: String,
    /// The final negative prompt.
    pub negative: String,
    /// The references that contributed fragments, in id order.
    pub references_used: Vec<CodexEntryId>,
    /// Every positive fragment that survived, in emission order.
    pub included: Vec<IncludedFragment>,
    /// The anchors that were folded in.
    pub included_anchors: Vec<IncludedAnchor>,
    /// The fragments dropped to fit the budget.
    pub excluded: Vec<ExcludedFragment>,
}

/// An internal scored candidate fragment before budgeting.
struct Candidate {
    text: String,
    source: Option<CodexEntryId>,
    priority: InclusionPriority,
    /// Order index so the budget pass keeps a stable, author-meaningful order.
    order: usize,
}

/// Compiles `request` against `codex` into an inspectable [`CompiledPrompt`].
///
/// The user request leads (always critical). Each resolved reference contributes its
/// entry's positive prompt fragments and its anchors; an anchor of `Strong` or
/// `Locked` strength is promoted to [`InclusionPriority::Critical`] so the budget
/// pass never drops it. Project rules append at `Normal`. When a budget is set, the
/// compiler keeps fragments by ascending priority (critical first) and drops the
/// rest into [`CompiledPrompt::excluded`]. Negatives never count against the positive
/// budget.
#[tracing::instrument(level = "debug", skip(codex, request), fields(refs = request.references.len(), budget = ?request.budget))]
pub fn compile(codex: &Codex, request: &PromptRequest) -> CompiledPrompt {
    let mut candidates: Vec<Candidate> = Vec::new();
    let mut negatives: Vec<String> = Vec::new();
    let mut anchors: Vec<IncludedAnchor> = Vec::new();
    let mut references_used: Vec<CodexEntryId> = Vec::new();
    let mut order = 0usize;

    // Layer 1: the user request, always critical and never dropped.
    let user = request.user_request.trim();
    if !user.is_empty() {
        candidates.push(Candidate {
            text: user.to_owned(),
            source: None,
            priority: InclusionPriority::Critical,
            order,
        });
        order += 1;
    }

    // Layer 2: resolved Codex references — their fragments, negatives, and anchors.
    for resolved in &request.references {
        let Some(entry) = codex.entry(resolved.entry) else {
            continue;
        };
        let mut contributed = false;

        for fragment in &entry.prompt_fragments {
            if fragment.priority == InclusionPriority::NeverInPrompt {
                continue;
            }
            candidates.push(Candidate {
                text: fragment.text.clone(),
                source: Some(entry.id),
                priority: fragment.priority,
                order,
            });
            order += 1;
            contributed = true;
        }

        for negative in &entry.negative_fragments {
            negatives.push(negative.clone());
            contributed = true;
        }

        for anchor in &entry.anchors {
            // A Negative anchor steers the negative prompt; others steer positive.
            if anchor.kind == AnchorKind::Negative {
                negatives.push(anchor.statement.clone());
            } else {
                let priority = anchor_priority(anchor.strength);
                candidates.push(Candidate {
                    text: anchor.statement.clone(),
                    source: Some(entry.id),
                    priority,
                    order,
                });
                order += 1;
                anchors.push(IncludedAnchor {
                    source: entry.id,
                    kind: anchor.kind,
                    strength: anchor.strength,
                    statement: anchor.statement.clone(),
                });
            }
            contributed = true;
        }

        if contributed && !references_used.contains(&entry.id) {
            references_used.push(entry.id);
        }
    }

    // Layer 3: project-wide rules append at Normal priority.
    for rule in &request.project_rules {
        candidates.push(Candidate {
            text: rule.clone(),
            source: None,
            priority: InclusionPriority::Normal,
            order,
        });
        order += 1;
    }

    // Layer 4: project negatives.
    negatives.extend(request.project_negatives.iter().cloned());

    // Budget pass: keep by ascending priority, then author order, within budget.
    let (included, excluded) = apply_budget(candidates, request.budget);

    let positive = included.iter().map(|f| f.text.as_str()).collect::<Vec<_>>().join(", ");
    let negative = dedup_join(&negatives);

    // references_used reflects only references that actually contributed surviving
    // content: a positive fragment that was kept, or an anchor that was folded in. A
    // reference whose only fragment was dropped to budget no longer counts as used.
    let surviving: Vec<CodexEntryId> = references_used
        .into_iter()
        .filter(|id| included.iter().any(|f| f.source == Some(*id)) || anchors.iter().any(|a| a.source == *id))
        .collect();

    CompiledPrompt {
        positive,
        negative,
        references_used: surviving,
        included,
        included_anchors: anchors,
        excluded,
    }
}

/// Maps an anchor strength to the inclusion priority the budget pass weighs it at.
/// Strong and Locked anchors are critical (never dropped); weaker ones compete.
fn anchor_priority(strength: AnchorStrength) -> InclusionPriority {
    match strength {
        AnchorStrength::Locked | AnchorStrength::Strong => InclusionPriority::Critical,
        AnchorStrength::Normal => InclusionPriority::Important,
        AnchorStrength::Loose => InclusionPriority::Optional,
    }
}

/// Splits scored candidates into kept and dropped, honoring `budget`.
///
/// Candidates are sorted by priority (critical first) then author order. When a
/// budget is set, fragments are added in that order while the running character
/// total (joined with `, `) stays within budget; anything that would overflow is
/// excluded. Critical fragments are always kept even if they alone exceed the
/// budget, because dropping them changes the result's identity (bible 7.5).
fn apply_budget(mut candidates: Vec<Candidate>, budget: Option<usize>) -> (Vec<IncludedFragment>, Vec<ExcludedFragment>) {
    candidates.sort_by(|a, b| a.priority.cmp(&b.priority).then_with(|| a.order.cmp(&b.order)));

    let mut included: Vec<IncludedFragment> = Vec::new();
    let mut excluded: Vec<ExcludedFragment> = Vec::new();
    let mut used = 0usize;

    for c in candidates {
        let cost = if included.is_empty() { c.text.len() } else { c.text.len() + 2 };
        let fits = match budget {
            Some(b) => used + cost <= b,
            None => true,
        };
        let is_critical = c.priority == InclusionPriority::Critical;
        if fits || is_critical {
            used += cost;
            included.push(IncludedFragment {
                text: c.text,
                source: c.source,
                priority: c.priority,
            });
        } else {
            excluded.push(ExcludedFragment {
                text: c.text,
                source: c.source,
                priority: c.priority,
            });
        }
    }

    // Re-sort the kept fragments back into author order for a readable prompt.
    included.sort_by_key(|f| {
        // Critical (user request) should still lead; rely on the fact the user
        // request is the only None-source Critical and was added first.
        priority_emit_rank(f.priority)
    });
    (included, excluded)
}

/// A stable emit-order key so critical content leads the joined positive prompt.
fn priority_emit_rank(priority: InclusionPriority) -> u8 {
    match priority {
        InclusionPriority::Critical => 0,
        InclusionPriority::Important => 1,
        InclusionPriority::Normal => 2,
        InclusionPriority::Optional => 3,
        InclusionPriority::NeverInPrompt => 4,
    }
}

/// Joins strings with `, `, dropping empties and exact duplicates while preserving
/// first-seen order.
fn dedup_join(items: &[String]) -> String {
    let mut seen: Vec<&str> = Vec::new();
    for item in items {
        let t = item.trim();
        if !t.is_empty() && !seen.contains(&t) {
            seen.push(t);
        }
    }
    seen.join(", ")
}

/// The entry type a resolved reference points at, for callers that need to weigh a
/// reference by type. A convenience over the resolved id and `codex`.
pub fn reference_type(codex: &Codex, reference: &ResolvedReference) -> Option<EntryType> {
    codex.entry(reference.entry).map(|e| e.entry_type)
}

/// Compiles the prompt for a single entry as the inspector preview (bible 14.5).
///
/// The Editor and Inspector show what *this* entry alone would contribute to a
/// generation: its own prompt fragments and anchors, with the entry referenced as if
/// the user had typed `@<handle>`. This builds that one-reference [`PromptRequest`]
/// and runs it through [`compile`], so the preview reflects the selected entry and
/// stays in lockstep with the real compiler — no second code path to drift.
///
/// Returns `None` when `entry` is not in `codex`. `budget` bounds the positive prompt
/// exactly as in [`compile`]; pass `None` for the full preview.
#[tracing::instrument(level = "debug", skip(codex), fields(?entry, ?budget))]
pub fn preview_entry(codex: &Codex, entry: CodexEntryId, budget: Option<usize>) -> Option<CompiledPrompt> {
    let target = codex.entry(entry)?;
    let raw = format!("@{}", target.handle.as_str());
    let parsed = ParsedReference {
        handle: target.handle.clone(),
        entry_type: Some(target.entry_type),
        scope: Vec::new(),
        start: 0,
        end: raw.len(),
        raw,
    };
    let reference = ResolvedReference {
        parsed,
        entry,
        status: target.status,
    };
    let request = PromptRequest {
        // The preview has no user request text; the entry is the whole request.
        user_request: String::new(),
        references: vec![reference],
        project_rules: Vec::new(),
        project_negatives: Vec::new(),
        budget,
    };
    Some(compile(codex, &request))
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::disallowed_methods, clippy::panic)]
mod tests {
    use super::*;
    use crate::codex::reference::resolve_text;
    use crate::codex::test_support::{with_negative_fragment, with_prompt_fragment};
    use pixhaus_core::{AddCodexEntry, Anchor, Codex, CodexEntryProto, CodexHandle, Command, Document, SetAnchor};

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

    fn request(codex: &Codex, text: &str) -> PromptRequest {
        let report = resolve_text(codex, text);
        PromptRequest {
            user_request: text.to_owned(),
            references: report.resolved,
            project_rules: Vec::new(),
            project_negatives: Vec::new(),
            budget: None,
        }
    }

    #[test]
    fn user_request_leads_and_fragments_follow() {
        let mut doc = Document::new();
        let bit = add(&mut doc, "bit", EntryType::Character);
        let codex = with_prompt_fragment(doc.codex(), bit, "round blue mascot", InclusionPriority::Important);
        let req = request(&codex, "@bit jumping");
        let out = compile(&codex, &req);
        assert!(out.positive.starts_with("@bit jumping"));
        assert!(out.positive.contains("round blue mascot"));
        assert_eq!(out.references_used, vec![bit]);
    }

    #[test]
    fn strong_anchor_is_critical_and_survives_budget() {
        let mut doc = Document::new();
        let bit = add(&mut doc, "bit", EntryType::Character);
        let mut set = SetAnchor::new(bit, Anchor::new(AnchorKind::Visual, AnchorStrength::Strong, "keep the round head"));
        set.apply(&mut doc).unwrap();
        let mut req = request(doc.codex(), "@bit");
        // A tiny budget that only fits the short user request.
        req.budget = Some(4);
        let out = compile(doc.codex(), &req);
        // The strong anchor is critical, so it is kept despite the budget.
        assert!(out.included_anchors.iter().any(|a| a.statement == "keep the round head"));
        assert!(out.positive.contains("keep the round head"));
    }

    #[test]
    fn optional_fragment_is_dropped_under_budget() {
        let mut doc = Document::new();
        let bit = add(&mut doc, "bit", EntryType::Character);
        let codex = with_prompt_fragment(doc.codex(), bit, "CRITICAL_KEEP", InclusionPriority::Critical);
        let codex = with_prompt_fragment(&codex, bit, "optional_extra_detail_text", InclusionPriority::Optional);
        let mut req = request(&codex, "@bit");
        req.budget = Some(20);
        let out = compile(&codex, &req);
        assert!(out.included.iter().any(|f| f.text == "CRITICAL_KEEP"));
        assert!(out.excluded.iter().any(|f| f.text == "optional_extra_detail_text"));
    }

    #[test]
    fn never_in_prompt_fragment_is_skipped() {
        let mut doc = Document::new();
        let bit = add(&mut doc, "bit", EntryType::Character);
        let codex = with_prompt_fragment(doc.codex(), bit, "internal note", InclusionPriority::NeverInPrompt);
        let req = request(&codex, "@bit");
        let out = compile(&codex, &req);
        assert!(!out.positive.contains("internal note"));
    }

    #[test]
    fn negative_anchor_and_negatives_assemble() {
        let mut doc = Document::new();
        let bit = add(&mut doc, "bit", EntryType::Character);
        let mut set = SetAnchor::new(bit, Anchor::new(AnchorKind::Negative, AnchorStrength::Normal, "no extra limbs"));
        set.apply(&mut doc).unwrap();
        let codex = with_negative_fragment(doc.codex(), bit, "blurry");
        let mut req = request(&codex, "@bit");
        req.project_negatives.push("watermark".to_owned());
        req.project_negatives.push("blurry".to_owned()); // duplicate, should dedup
        let out = compile(&codex, &req);
        assert!(out.negative.contains("blurry"));
        assert!(out.negative.contains("no extra limbs"));
        assert!(out.negative.contains("watermark"));
        // The duplicate "blurry" appears once.
        assert_eq!(out.negative.matches("blurry").count(), 1);
        // A Negative anchor is not listed as an included (positive) anchor.
        assert!(out.included_anchors.is_empty());
    }

    #[test]
    fn project_rules_append() {
        let mut doc = Document::new();
        add(&mut doc, "bit", EntryType::Character);
        let mut req = request(doc.codex(), "@bit");
        req.project_rules.push("32x32 pixel art".to_owned());
        let out = compile(doc.codex(), &req);
        assert!(out.positive.contains("32x32 pixel art"));
    }

    #[test]
    fn reference_type_reports_the_entry_type() {
        let mut doc = Document::new();
        add(&mut doc, "moonlit", EntryType::Palette);
        let report = resolve_text(doc.codex(), "@moonlit");
        assert_eq!(reference_type(doc.codex(), &report.resolved[0]), Some(EntryType::Palette));
    }

    #[test]
    fn preview_entry_compiles_the_entry_alone() {
        let mut doc = Document::new();
        let bit = add(&mut doc, "bit", EntryType::Character);
        let codex = with_prompt_fragment(doc.codex(), bit, "round blue mascot", InclusionPriority::Important);
        let out = preview_entry(&codex, bit, None).expect("entry exists");
        assert!(out.positive.contains("round blue mascot"));
        assert_eq!(out.references_used, vec![bit]);
    }

    #[test]
    fn preview_entry_folds_in_anchors() {
        let mut doc = Document::new();
        let bit = add(&mut doc, "bit", EntryType::Character);
        let mut set = SetAnchor::new(bit, Anchor::new(AnchorKind::Visual, AnchorStrength::Strong, "keep the round head"));
        set.apply(&mut doc).unwrap();
        let out = preview_entry(doc.codex(), bit, None).expect("entry exists");
        assert!(out.included_anchors.iter().any(|a| a.statement == "keep the round head"));
        assert!(out.positive.contains("keep the round head"));
    }

    #[test]
    fn preview_entry_returns_none_for_unknown_entry() {
        let doc = Document::new();
        assert!(preview_entry(doc.codex(), CodexEntryId(999), None).is_none());
    }
}
