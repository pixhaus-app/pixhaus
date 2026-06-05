//! The application shell: per-frame region composition, the command palette,
//! shortcut routing, and the menu structure (architecture bible section 8).
//!
//! `Shell::run` is called from `App::ui`; `drain_background` from `App::logic`.

pub mod about;
pub mod command_palette;
pub mod menus;
pub mod regions;
pub mod runtime;
pub mod shortcuts;
pub mod splash;

pub use runtime::Shell;

use pixhaus_services::{JobMsg, JobStatus};

use crate::state::session::{AiError, AiStatus};
use crate::state::{BackgroundMsg, Host};

/// Drain background-channel and job results into session state, from `App::logic`.
///
/// This is the single mpsc-drain front door (spec "Region composition and the shell
/// runtime"). It drains two channels: the bootstrap `BackgroundMsg` channel, and the
/// `EditSession`'s job channel — a completed generation job's asset is already in the
/// `ResultStore`, so a [`JobMsg`] only refreshes the read-mirror, AI status, and
/// requests a repaint. It runs in `logic`, not `ui`, because `logic` runs even when
/// the window is occluded but a repaint was requested; if anything landed it requests
/// a repaint so the new state shows immediately.
pub fn drain_background(host: &mut Host, ctx: &egui::Context) {
    let mut landed = false;

    // `try_recv` returns `Err` on both an empty and a disconnected channel, so a
    // `while let Ok` cleanly stops draining in either case.
    while let Ok(msg) = host.bg.rx.try_recv() {
        match msg {
            BackgroundMsg::AiStatusChanged(status) => {
                host.state.session.ai_status = status;
                landed = true;
            }
        }
    }

    // Drain job notifications into a buffer first so the channel borrow ends before we
    // mutate the job manager / session in the match.
    let mut job_msgs = Vec::new();
    while let Ok(msg) = host.edit.job_rx.try_recv() {
        job_msgs.push(msg);
    }
    for msg in job_msgs {
        landed = true;
        match msg {
            JobMsg::Status { job, status } => {
                let cancelled = matches!(status, JobStatus::Cancelled);
                if cancelled {
                    host.state.session.ai_status = AiStatus::Ready;
                }
                host.edit.jobs.set_status(job, status);
                if cancelled {
                    // Terminal: release the dead cancel token so the manager's maps
                    // don't grow for the host's lifetime.
                    host.edit.jobs.finish(job);
                }
            }
            JobMsg::Completed { job } => {
                host.state.session.ai_status = AiStatus::Ready;
                host.edit.jobs.finish(job);
            }
            JobMsg::Failed { job, key, detail } => {
                // Resolve the stable key to user-facing text here (the UI lane), at the
                // moment the message is drained, interpolating the non-localized detail.
                // `services` shipped a key, never English, so the message honors the
                // active language; the raw key + detail still go to the log untranslated.
                let message = match &detail {
                    Some(detail) => pixhaus_services::i18n::tr_args(key, &[("detail", detail.as_str())]),
                    None => pixhaus_services::i18n::tr(key),
                };
                tracing::warn!(?job, key, ?detail, %message, "generation job failed");
                host.state.session.ai_status = AiStatus::Ready;
                // Surface the failure to the artist, not just the log. Carry the raw
                // key + detail (B2's split), not the resolved `message`, so a language
                // switch re-renders it correctly; the status bar resolves at render time.
                host.state.session.last_error = Some(AiError { key, detail });
                host.edit.jobs.finish(job);
            }
        }
    }

    if landed {
        // Copy these document- and result-derived values into the read-only
        // SessionState mirror each frame. Panels are deferred-intent and read-only
        // (&self): they render SessionState and never reach the live EditSession, so
        // anything a panel shows must be mirrored here. The live document and services
        // stay out of the state panels see, keeping mutation on the command path.
        host.state.session.result_count = host.edit.results.len();
        host.state.session.selected_result = host.edit.results.selected_index();
        host.state.session.result_kinds = host.edit.results.kinds_summary();
        ctx.request_repaint();
    }
}

/// Refresh the read-only playback mirror the Animate panels render from (they cannot
/// reach the document). Rebuilds the document-derived shape — frame count, clips, the
/// resolved play range — and re-derives the playhead offset from the transient
/// playback clock. Runs once per frame in [`Shell::run`]; the rebuilt clip rows are a
/// handful of small allocations (the Animate UI, not the pixel hot path).
pub fn sync_playback_mirror(host: &mut Host) {
    use crate::state::session::{ClipRow, PlaybackMirror};

    let clip = host.state.ui.playback.clip;
    let seconds = host.state.ui.playback.playhead_seconds;

    let mirror = host.edit.document.active_sprite().and_then(|id| host.edit.document.sprite(id)).map(|sprite| {
        let range = crate::playback::resolve_range(sprite, clip);
        let frame_count = u32::try_from(sprite.frames().len()).unwrap_or(u32::MAX);
        let clips = sprite
            .clips()
            .iter()
            .map(|clip| ClipRow {
                id: clip.id,
                name: clip.name.clone(),
                start: clip.start,
                end: clip.end,
                fps: clip.fps,
            })
            .collect();
        PlaybackMirror {
            frame_count,
            clips,
            range_start: range.start,
            range_fps: range.fps,
            playhead_offset: crate::playback::playhead_index(seconds, range.fps, range.frame_count, range.loop_mode),
            // A single-frame sprite has nothing to play; transport stays disabled.
            playable: frame_count > 1,
        }
    });
    host.state.session.playback = mirror.unwrap_or_default();

    // Self-heal: if the active sprite can no longer play (e.g. an undo took the
    // document back to a still or empty sprite), clear the transient playing flag.
    // Otherwise the loop would keep requesting repaints with the transport disabled
    // (it gates on `playable`), and there would be no in-UI way to stop it.
    if !host.state.session.playback.playable {
        host.state.ui.playback.playing = false;
    }
}

/// Rebuild the read-only Codex mirror the Codex-workspace panels render from (they
/// cannot reach the document). Snapshots every entry as a Navigator summary, resolves
/// the selected entry's detail (relations, used-by, anchors) and the search
/// suggestions, and mirrors the context stack and compiled-prompt preview from the
/// shell-owned Codex UI state. Runs once per frame in [`Shell::run`]; the Codex is a
/// metadata graph (a few hundred entries at most), not the pixel hot path.
// The mirror builder snapshots every entry plus the selected detail, suggestions, and
// context stack; its length tracks the view's field count, so the line-count lint does
// not apply (the same carve-out `apply_intent` takes).
#[allow(clippy::too_many_lines)]
pub fn sync_codex_view(host: &mut Host) {
    use crate::state::session::{
        AnchorBadge, CodexContextEntry, CodexEntryDetail, CodexEntrySummary, CodexFolderNode, CodexRelation, CodexSuggestion, CodexView, CoverageRow,
        CoverageSlotRow, CoverageTemplateSummary, EntryVersionRow, HealthCheckRow,
    };
    use pixhaus_services::codex::compiler::preview_entry;
    use pixhaus_services::codex::folder::folder_tree;
    use pixhaus_services::codex::{
        CheckState, SearchContext, broken_reference_count, coverage_report, coverage_template_details, entry_health, generation_readiness, key_info,
        linked_assets, suggest, used_in_counts,
    };

    let codex = host.edit.document.codex();
    let ui_codex = &host.state.ui.codex;

    // The display label for an entry: its name, falling back to its handle.
    let label_of = |id: pixhaus_core::CodexEntryId| -> String {
        match codex.entry(id) {
            Some(e) if !e.name.is_empty() => e.name.clone(),
            Some(e) => e.handle.as_str().to_owned(),
            None => String::new(),
        }
    };

    // One summary per entry, in id order (BTreeMap iteration is sorted).
    let summarize = |id: pixhaus_core::CodexEntryId, entry: &pixhaus_core::CodexEntry| -> CodexEntrySummary {
        let anchors = entry
            .anchors
            .iter()
            .map(|a| AnchorBadge {
                kind: a.kind,
                strength: a.strength,
            })
            .collect();
        let report = coverage_report(codex, id);
        let coverage_ratio = report.completion_ratio();
        let coverage_incomplete = report.missing_count() > 0;
        CodexEntrySummary {
            id,
            handle: entry.handle.as_str().to_owned(),
            name: entry.name.clone(),
            entry_type: entry.entry_type,
            status: entry.status,
            anchors,
            coverage_ratio,
            coverage_incomplete,
            broken_ref_count: broken_reference_count(codex, id),
        }
    };

    let entries: Vec<CodexEntrySummary> = codex.entries().iter().map(|(id, entry)| summarize(*id, entry)).collect();

    // Resolve the selection against the live set: a deleted selection clears.
    let selected = ui_codex.selected.filter(|id| codex.entry(*id).is_some());

    let detail = selected.and_then(|id| {
        codex.entry(id).map(|entry| {
            // Outgoing edges from this entry, and incoming "used by" edges.
            let mut relations = Vec::new();
            let mut used_by = Vec::new();
            for rel in codex.relationships() {
                if rel.from == id {
                    relations.push(CodexRelation {
                        kind: rel.kind,
                        other: rel.to,
                        other_label: label_of(rel.to),
                    });
                }
                if rel.to == id {
                    used_by.push(CodexRelation {
                        kind: rel.kind,
                        other: rel.from,
                        other_label: label_of(rel.from),
                    });
                }
            }
            // Derived health, readiness, usage, key-info, and per-slot coverage from the
            // services passes - computed here (the one mirror builder) so panels read a
            // number, never recompute. The center cards and the inspector checklist read
            // the same coverage rows, so they always agree.
            let health = entry_health(codex, id);
            let health_checks = health
                .checks
                .iter()
                .map(|c| HealthCheckRow {
                    label_key: c.message_key.clone(),
                    state: match c.state {
                        CheckState::Pass => Some(true),
                        CheckState::Fail => Some(false),
                        CheckState::Warn => None,
                    },
                    detail: c.detail.clone(),
                })
                .collect();
            let readiness = generation_readiness(codex, id);
            let report = coverage_report(codex, id);
            let coverage_items = report
                .items
                .iter()
                .map(|item| CoverageRow {
                    slot: item.slot.clone(),
                    label: item.label.clone(),
                    status: item.status,
                    template: item.template,
                })
                .collect();
            let info = key_info(codex, id);
            let version_history = entry
                .version_history
                .iter()
                .map(|v| EntryVersionRow {
                    version: v.version,
                    timestamp_ms: v.timestamp_ms,
                    author: v.author.clone(),
                    summary: v.summary.clone(),
                })
                .collect();
            // Notes reuse the generic `notes` field (spec §8: no new model state).
            let notes = match &entry.details {
                pixhaus_core::codex::EntryDetails::Generic(g) => g.fields.iter().find(|f| f.key == "notes").map(|f| f.value.clone()).unwrap_or_default(),
                _ => String::new(),
            };
            CodexEntryDetail {
                summary: summarize(id, entry),
                aliases: entry.aliases.iter().map(|h| h.as_str().to_owned()).collect(),
                folder_id: entry.folder_id,
                description: entry.description.clone(),
                lore: entry.lore.clone(),
                visual_description: entry.visual_description.clone(),
                tags: entry.tags.clone(),
                prompt_fragments: entry.prompt_fragments.clone(),
                negative_fragments: entry.negative_fragments.clone(),
                anchors: entry.anchors.clone(),
                details: entry.details.clone(),
                relations,
                used_by,
                // The per-entry preview: what this entry alone contributes to a prompt,
                // so the inspector reflects the selection (not a stale global compile).
                prompt_preview: preview_entry(codex, id, None),
                notes,
                created_ms: info.created_ms,
                updated_ms: info.updated_ms,
                author: info.author,
                version: info.version,
                version_history,
                health: health.score(),
                health_percent: health.percent(),
                health_checks,
                readiness: readiness.score(),
                readiness_percent: readiness.percent(),
                coverage_items,
                applied_templates: entry.applied_templates.clone(),
                used_in: used_in_counts(codex, id),
                linked_asset_count: linked_assets(codex, id).handle_count,
            }
        })
    });

    // Suggestions for the current search query (empty query lists all candidates).
    let suggestions: Vec<CodexSuggestion> = suggest(
        codex,
        &ui_codex.search,
        None,
        &SearchContext::default(),
        pixhaus_services::codex::DEFAULT_SUGGESTION_LIMIT,
    )
    .into_iter()
    .map(|s| CodexSuggestion {
        id: s.entry,
        handle: s.handle,
        name: s.name,
        entry_type: s.entry_type,
        status: s.status,
    })
    .collect();

    // The context stack, with display data resolved; drop any pinned entry that no
    // longer exists (an undo could have removed it).
    let context: Vec<CodexContextEntry> = ui_codex
        .context
        .iter()
        .filter_map(|c| {
            codex.entry(c.entry).map(|entry| CodexContextEntry {
                entry: c.entry,
                strength: c.strength,
                handle: entry.handle.as_str().to_owned(),
                entry_type: entry.entry_type,
            })
        })
        .collect();

    // The folder tree, mapped from the services tree into owned Navigator nodes. Each
    // node carries its entries as summaries (drawn from the by-id summary map) and its
    // child folders, recursively. Root-level entries are the summaries with no folder.
    let by_id: std::collections::HashMap<pixhaus_core::CodexEntryId, CodexEntrySummary> = entries.iter().map(|s| (s.id, s.clone())).collect();
    let summaries_for = |ids: &[pixhaus_core::CodexEntryId]| -> Vec<CodexEntrySummary> { ids.iter().filter_map(|id| by_id.get(id).cloned()).collect() };
    let tree = folder_tree(codex);
    let folder_tree_nodes: Vec<CodexFolderNode> = tree.roots.iter().map(|n| map_folder_node(n, codex, &by_id)).collect();
    let root_entries = summaries_for(&tree.root_entries);

    // Project-wide coverage: sum complete/total over every entry, for the top-bar
    // Codex status item. A pure read over the same per-entry reports.
    let project_coverage = codex.entries().keys().fold((0_usize, 0_usize), |(complete, total), id| {
        let report = coverage_report(codex, *id);
        (complete + report.complete_count(), total + report.total())
    });

    // Every project coverage template with its full slot list, for the picker and the
    // template-management surface. Carrying the slots (not just a count) makes a template's
    // slots editable whether or not it is applied to the selected entry.
    let coverage_templates: Vec<CoverageTemplateSummary> = coverage_template_details(codex)
        .into_iter()
        .map(|detail| CoverageTemplateSummary {
            id: detail.id,
            name: detail.name,
            slots: detail.slots.into_iter().map(|s| CoverageSlotRow { key: s.key, label: s.label }).collect(),
        })
        .collect();

    // Commit the whole derived mirror in one assignment. Every derived number
    // (coverage, readiness, suggestions, detail, folder tree) is computed once
    // above and published together so the center cards and inspector checklist
    // always read the same values and can't disagree mid-frame; panels render
    // from this plain owned data and echo ids back as intents, never touching
    // the live document.
    host.state.session.codex = CodexView {
        entries,
        mode: ui_codex.mode,
        detail_tab: ui_codex.detail_tab,
        nav_filter: ui_codex.nav_filter,
        project_coverage,
        selected,
        detail,
        search: ui_codex.search.clone(),
        suggestions,
        context,
        compiled: ui_codex.compiled.clone(),
        folder_tree: folder_tree_nodes,
        root_entries,
        coverage_templates,
    };

    // Reload the editor draft when the selection changed (or a deleted selection
    // cleared it). The shell owns the draft so the editor panel can bind its
    // `TextEdit`s to a `&mut` to it; reloading here keeps it in step with the
    // selection without the panel reaching into the document.
    let draft = &mut host.codex_draft;
    match host.state.session.codex.detail.as_ref() {
        Some(detail) if draft.loaded_id != Some(detail.summary.id) => draft.load_from(detail),
        None if draft.loaded_id.is_some() => *draft = crate::state::ui_state::CodexEditorDraft::default(),
        _ => {}
    }
}

/// Map one services [`FolderNode`](pixhaus_services::codex::folder::FolderNode) into the
/// owned [`CodexFolderNode`](crate::state::session::CodexFolderNode) the Navigator
/// renders, drawing each folder's entries from the by-id summary map and recursing into
/// child folders.
fn map_folder_node(
    node: &pixhaus_services::codex::folder::FolderNode,
    codex: &pixhaus_core::Codex,
    by_id: &std::collections::HashMap<pixhaus_core::CodexEntryId, crate::state::session::CodexEntrySummary>,
) -> crate::state::session::CodexFolderNode {
    let summaries_for = |ids: &[pixhaus_core::CodexEntryId]| ids.iter().filter_map(|id| by_id.get(id).cloned()).collect();
    crate::state::session::CodexFolderNode {
        id: node.folder,
        name: codex.folder(node.folder).map_or_else(String::new, |f| f.name.clone()),
        entries: summaries_for(&node.entries),
        children: node.children.iter().map(|c| map_folder_node(c, codex, by_id)).collect(),
    }
}

#[cfg(test)]
mod tests {
    use super::{drain_background, sync_codex_view};
    use crate::state::Host;
    use crate::state::intent::{Intent, apply_intent};
    use crate::theme::Theme;
    use pixhaus_core::codex::EntryType;
    use pixhaus_services::{JobId, JobManager, JobMsg};
    use std::sync::mpsc;

    /// The `JobMsg::Failed` arm surfaces the failure to the artist: it populates
    /// `session.last_error` with the carried key + detail (not just a log line) and
    /// resets the AI status to Ready. A failure that is only logged is invisible; this
    /// is the regression guard that it reaches the status bar.
    #[test]
    fn failed_job_populates_last_error() {
        use crate::state::session::AiStatus;

        let mut host = Host::new(&Theme::dark());
        let ctx = egui::Context::default();
        // Rewire the job channel so the test can post a `Failed` message the drain reads.
        let (tx, rx) = mpsc::channel();
        host.edit.jobs = JobManager::new(tx.clone());
        host.edit.job_rx = rx;
        host.state.session.ai_status = AiStatus::Working;

        let send = tx.send(JobMsg::Failed {
            job: JobId(0),
            key: "provider.error.unavailable",
            detail: Some("backend down".to_owned()),
        });
        assert!(send.is_ok(), "the test channel accepts the failure message");

        drain_background(&mut host, &ctx);

        let Some(error) = host.state.session.last_error.as_ref() else {
            panic!("the failure arm populates last_error");
        };
        assert_eq!(error.key, "provider.error.unavailable", "the stable key is carried for render-time resolution");
        assert_eq!(error.detail.as_deref(), Some("backend down"), "the non-localized detail is carried");
        assert_eq!(host.state.session.ai_status, AiStatus::Ready, "a failed job returns the status to Ready");
    }

    /// The Codex mirror reflects a created entry, its selection, and the mode. Drives a
    /// real `CreateCodexEntry` intent, then rebuilds the mirror and reads it back.
    #[test]
    fn sync_codex_view_mirrors_a_created_entry() {
        let mut host = Host::new(&Theme::dark());
        let ctx = egui::Context::default();
        apply_intent(
            &mut host,
            Intent::CreateCodexEntry {
                entry_type: EntryType::Character,
                name: "Bit".to_owned(),
            },
            &ctx,
        );
        sync_codex_view(&mut host);

        let view = &host.state.session.codex;
        assert_eq!(view.entry_count(), 1, "the mirror lists the new entry");
        let Some(summary) = view.entries.first() else {
            panic!("one entry summarized");
        };
        assert_eq!(summary.name, "Bit");
        assert_eq!(summary.entry_type, EntryType::Character);
        assert!(view.selected.is_some(), "the created entry is selected");
        assert!(view.detail.is_some(), "the selected entry has a detail snapshot");
    }

    /// Deleting the selected entry empties the mirror and clears the detail.
    #[test]
    fn sync_codex_view_drops_a_deleted_entry() {
        let mut host = Host::new(&Theme::dark());
        let ctx = egui::Context::default();
        apply_intent(
            &mut host,
            Intent::CreateCodexEntry {
                entry_type: EntryType::Palette,
                name: "Moonlit".to_owned(),
            },
            &ctx,
        );
        let Some(id) = host.state.ui.codex.selected else {
            panic!("created entry selected");
        };
        apply_intent(&mut host, Intent::DeleteCodexEntry(id), &ctx);
        sync_codex_view(&mut host);
        assert_eq!(host.state.session.codex.entry_count(), 0, "the deleted entry is gone from the mirror");
        assert!(host.state.session.codex.detail.is_none(), "no detail after the selection was deleted");
    }

    /// The selected entry's detail carries a per-entry prompt preview, so the inspector
    /// reflects the selection (not a stale global compile).
    #[test]
    fn sync_codex_view_builds_a_per_entry_preview() {
        let mut host = Host::new(&Theme::dark());
        let ctx = egui::Context::default();
        apply_intent(
            &mut host,
            Intent::CreateCodexEntry {
                entry_type: EntryType::Character,
                name: "Bit".to_owned(),
            },
            &ctx,
        );
        let Some(id) = host.state.ui.codex.selected else {
            panic!("created entry selected");
        };
        // Give it a positive prompt fragment so the preview is non-empty.
        apply_intent(
            &mut host,
            Intent::SetCodexPromptFragments {
                id,
                fragments: vec![pixhaus_core::codex::PromptFragment::new(
                    "round head",
                    pixhaus_core::codex::InclusionPriority::Critical,
                )],
            },
            &ctx,
        );
        sync_codex_view(&mut host);
        let preview = host.state.session.codex.detail.as_ref().and_then(|d| d.prompt_preview.clone());
        assert!(
            preview.is_some_and(|p| p.positive.contains("round head")),
            "the per-entry preview reflects the selected entry"
        );
    }

    /// The editor draft reloads from the selection: selecting A then B shows B's name,
    /// so edits land on the right entry (the central bug-1 fix).
    #[test]
    fn draft_reloads_on_selection_change() {
        let mut host = Host::new(&Theme::dark());
        let ctx = egui::Context::default();
        apply_intent(
            &mut host,
            Intent::CreateCodexEntry {
                entry_type: EntryType::Character,
                name: "Alpha".to_owned(),
            },
            &ctx,
        );
        let Some(a) = host.state.ui.codex.selected else {
            panic!("alpha selected");
        };
        apply_intent(
            &mut host,
            Intent::CreateCodexEntry {
                entry_type: EntryType::Character,
                name: "Beta".to_owned(),
            },
            &ctx,
        );
        let Some(b) = host.state.ui.codex.selected else {
            panic!("beta selected");
        };
        sync_codex_view(&mut host);
        assert_eq!(host.codex_draft.loaded_id, Some(b), "the draft loaded the second entry");
        assert_eq!(host.codex_draft.name, "Beta", "the draft shows the selected entry's name");

        // Reselect A; the draft must reload to A's values, not keep B's.
        apply_intent(&mut host, Intent::SelectCodexEntry(a), &ctx);
        sync_codex_view(&mut host);
        assert_eq!(host.codex_draft.loaded_id, Some(a), "the draft reloaded the first entry");
        assert_eq!(host.codex_draft.name, "Alpha", "selecting back shows the first entry, not the second");
    }

    /// Applying a built-in preset to one entry surfaces a project template in the mirror
    /// and per-entry coverage rows on that entry alone - the cross-entry bleed fix, read
    /// at the UI layer.
    #[test]
    fn sync_codex_view_mirrors_templates_and_is_per_entry() {
        use pixhaus_core::commands::BuiltinCoveragePreset;

        let mut host = Host::new(&Theme::dark());
        let ctx = egui::Context::default();
        apply_intent(
            &mut host,
            Intent::CreateCodexEntry {
                entry_type: EntryType::Character,
                name: "Alpha".to_owned(),
            },
            &ctx,
        );
        let Some(a) = host.state.ui.codex.selected else {
            panic!("alpha selected");
        };
        apply_intent(
            &mut host,
            Intent::CreateCodexEntry {
                entry_type: EntryType::Character,
                name: "Beta".to_owned(),
            },
            &ctx,
        );
        let Some(b) = host.state.ui.codex.selected else {
            panic!("beta selected");
        };
        // Apply a preset to A only.
        apply_intent(
            &mut host,
            Intent::ApplyBuiltinCoverageTemplate {
                id: a,
                preset: BuiltinCoveragePreset::PlatformerCharacter,
            },
            &ctx,
        );
        sync_codex_view(&mut host);

        let view = &host.state.session.codex;
        assert_eq!(view.coverage_templates.len(), 1, "the project template is mirrored for the picker");

        // A's detail carries the seeded rows and the applied template id; B's does not.
        apply_intent(&mut host, Intent::SelectCodexEntry(a), &ctx);
        sync_codex_view(&mut host);
        let a_items = host.state.session.codex.detail.as_ref().map_or(0, |d| d.coverage_items.len());
        let a_applied = host.state.session.codex.detail.as_ref().map_or(0, |d| d.applied_templates.len());
        assert!(a_items > 0, "entry A has its seeded coverage rows");
        assert_eq!(a_applied, 1, "entry A has the template applied");

        apply_intent(&mut host, Intent::SelectCodexEntry(b), &ctx);
        sync_codex_view(&mut host);
        let b_items = host.state.session.codex.detail.as_ref().map_or(0, |d| d.coverage_items.len());
        let b_applied = host.state.session.codex.detail.as_ref().map_or(0, |d| d.applied_templates.len());
        assert_eq!(b_items, 0, "entry B shows no coverage - the bleed is fixed");
        assert_eq!(b_applied, 0, "entry B has no template applied");
    }

    /// The mirror carries every project template's full slot list, even for a template
    /// applied to no entry, so the slot editor can reach its slots (GAP 2).
    #[test]
    fn sync_codex_view_mirrors_unapplied_template_slots() {
        use pixhaus_core::CoverageSlot;

        let mut host = Host::new(&Theme::dark());
        let ctx = egui::Context::default();
        apply_intent(
            &mut host,
            Intent::CreateCoverageTemplate {
                name: "states".to_owned(),
                slots: vec![CoverageSlot::custom("idle", "Idle"), CoverageSlot::custom("walk", "Walk")],
            },
            &ctx,
        );
        sync_codex_view(&mut host);
        let Some(tpl) = host.state.session.codex.coverage_templates.first() else {
            panic!("the template is mirrored");
        };
        assert_eq!(tpl.slot_count(), 2, "the unapplied template's slot count is mirrored");
        assert!(
            tpl.slots.iter().any(|s| s.key == "idle"),
            "the unapplied template's slot keys are reachable for the editor",
        );
    }
}
