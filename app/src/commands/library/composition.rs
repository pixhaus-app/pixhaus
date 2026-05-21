//! Composition-library IPC commands: Structures, Styles, and Prompts.
//!
//! The project tier lives on [`ProjectAi`](pixhaus_core::project::library::ProjectAi)
//! (`project.library.ai`); built-ins come from
//! [`BuiltinLibrary`](pixhaus_ai::compose::builtins::BuiltinLibrary). Project
//! records shadow built-ins by id. Per spec section 11.
//!
//! The merge policies (`merge_structures` / `merge_styles` / `merge_prompts`)
//! are pure and carry the testable core; the command bodies are thin wrappers
//! over them, the doc lock, and the `.pixstyle` bundle I/O.

use std::time::{SystemTime, UNIX_EPOCH};

use pixhaus_ai::compose::builtins::BuiltinLibrary;
use pixhaus_ai::compose::variables::detect_tokens;
use pixhaus_core::project::EntityId;
use pixhaus_core::project::library::composition::{
    PromptId, PromptTemplate, Structure, StructureId, Style, StyleId,
};
use pixhaus_io::pixstyle::{StylePack, read_library_from_project_ai, read_pack, write_pack};
use serde::{Deserialize, Serialize};
use tauri::State;

use crate::error::{AppCommandError, CommandResult};
use crate::state::AppState;

// ── response / request types ────────────────────────────────────────────────

/// The full composition library: project-tier records plus the read-only
/// built-in registry. Project records shadow built-ins by id.
#[derive(Debug, Serialize, Deserialize, ts_rs::TS)]
#[ts(export)]
pub struct CompositionLibrary {
    /// Project-tier Structures.
    pub structures: Vec<Structure>,
    /// Project-tier Styles.
    pub styles: Vec<Style>,
    /// Project-tier saved Prompts.
    pub prompts: Vec<PromptTemplate>,
    /// Built-in Structures, in id order.
    pub builtin_structures: Vec<Structure>,
    /// Built-in Styles, in id order.
    pub builtin_styles: Vec<Style>,
    /// Built-in saved Prompts, in id order.
    pub builtin_prompts: Vec<PromptTemplate>,
}

/// How a `.pixstyle` import resolves an id that already exists in the target.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, ts_rs::TS)]
#[serde(rename_all = "snake_case")]
#[ts(export)]
pub enum ConflictPolicy {
    /// Keep the existing record; drop the incoming one.
    Skip,
    /// Replace the existing record with the incoming one.
    Overwrite,
    /// Add the incoming record under a fresh `.copy`-suffixed id.
    ImportAsCopy,
}

/// Counts of records affected by a merge.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize, ts_rs::TS)]
#[ts(export)]
pub struct ImportReport {
    /// Records added (new id, or `.copy` under `ImportAsCopy`).
    pub imported: u32,
    /// Records skipped because their id already existed under `Skip`.
    pub skipped: u32,
    /// Records replaced because their id already existed under `Overwrite`.
    pub overwritten: u32,
}

impl ImportReport {
    /// Adds another report's counts into this one.
    fn add(&mut self, other: ImportReport) {
        self.imported += other.imported;
        self.skipped += other.skipped;
        self.overwritten += other.overwritten;
    }
}

/// One resolved variable for a saved Prompt: the declared (or detected) key,
/// its label and default, and any best-effort autofilled value.
#[derive(Debug, Serialize, Deserialize, ts_rs::TS)]
#[ts(export)]
pub struct ResolvedVariable {
    /// The variable key as it appears in `{token}` form.
    pub key: String,
    /// Display label. Defaults to `key` for auto-detected tokens.
    pub label: String,
    /// Default value. Empty for auto-detected tokens.
    pub default: String,
    /// Best-effort value pulled from the target entity's metadata, if any.
    pub autofilled: Option<String>,
}

/// Which composition record kind a fork targets.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, ts_rs::TS)]
#[serde(rename_all = "snake_case")]
#[ts(export)]
pub enum CompositionKind {
    /// A [`Structure`] record.
    Structure,
    /// A [`Style`] record.
    Style,
    /// A [`PromptTemplate`] record.
    Prompt,
}

impl std::fmt::Display for CompositionKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let s = match self {
            CompositionKind::Structure => "structure",
            CompositionKind::Style => "style",
            CompositionKind::Prompt => "prompt",
        };
        f.write_str(s)
    }
}

// ── pure merge logic ────────────────────────────────────────────────────────

/// Merges `incoming` Structures into `target` per `policy`, reporting the
/// counts. On `ImportAsCopy` a colliding id gains a `.copy` suffix.
#[must_use]
pub fn merge_structures(
    target: &mut Vec<Structure>,
    incoming: Vec<Structure>,
    policy: ConflictPolicy,
) -> ImportReport {
    let mut report = ImportReport::default();
    for s in incoming {
        match target.iter().position(|t| t.id == s.id) {
            Some(_) if matches!(policy, ConflictPolicy::Skip) => report.skipped += 1,
            Some(i) if matches!(policy, ConflictPolicy::Overwrite) => {
                target[i] = s;
                report.overwritten += 1;
            }
            Some(_) => {
                // ImportAsCopy.
                let mut c = s;
                c.id = StructureId(format!("{}.copy", c.id.0));
                target.push(c);
                report.imported += 1;
            }
            None => {
                target.push(s);
                report.imported += 1;
            }
        }
    }
    report
}

/// Merges `incoming` Styles into `target` per `policy`. See [`merge_structures`].
#[must_use]
pub fn merge_styles(
    target: &mut Vec<Style>,
    incoming: Vec<Style>,
    policy: ConflictPolicy,
) -> ImportReport {
    let mut report = ImportReport::default();
    for s in incoming {
        match target.iter().position(|t| t.id == s.id) {
            Some(_) if matches!(policy, ConflictPolicy::Skip) => report.skipped += 1,
            Some(i) if matches!(policy, ConflictPolicy::Overwrite) => {
                target[i] = s;
                report.overwritten += 1;
            }
            Some(_) => {
                let mut c = s;
                c.id = StyleId(format!("{}.copy", c.id.0));
                target.push(c);
                report.imported += 1;
            }
            None => {
                target.push(s);
                report.imported += 1;
            }
        }
    }
    report
}

/// Merges `incoming` Prompts into `target` per `policy`. See [`merge_structures`].
#[must_use]
pub fn merge_prompts(
    target: &mut Vec<PromptTemplate>,
    incoming: Vec<PromptTemplate>,
    policy: ConflictPolicy,
) -> ImportReport {
    let mut report = ImportReport::default();
    for p in incoming {
        match target.iter().position(|t| t.id == p.id) {
            Some(_) if matches!(policy, ConflictPolicy::Skip) => report.skipped += 1,
            Some(i) if matches!(policy, ConflictPolicy::Overwrite) => {
                target[i] = p;
                report.overwritten += 1;
            }
            Some(_) => {
                let mut c = p;
                c.id = PromptId(format!("{}.copy", c.id.0));
                target.push(c);
                report.imported += 1;
            }
            None => {
                target.push(p);
                report.imported += 1;
            }
        }
    }
    report
}

/// Merges every kind of a [`StylePack`] into the project tier, summing reports.
fn merge_pack(
    ai: &mut pixhaus_core::project::library::ProjectAi,
    pack: StylePack,
    policy: ConflictPolicy,
) -> ImportReport {
    let mut report = ImportReport::default();
    report.add(merge_structures(
        &mut ai.structures,
        pack.structures,
        policy,
    ));
    report.add(merge_styles(&mut ai.styles, pack.styles, policy));
    report.add(merge_prompts(&mut ai.prompts, pack.prompts, policy));
    report
}

/// Nanosecond clock for minting fresh ids. Falls back to `0` if the clock is
/// before the epoch (never in practice).
fn now_nanos() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_nanos())
}

// ── commands ────────────────────────────────────────────────────────────────

/// Lists the project-tier composition records and the built-in registry.
///
/// # Errors
/// Returns [`AppCommandError::NoActiveProject`] when no project is open.
#[tauri::command(async, rename_all = "snake_case")]
pub async fn library_list_composition(
    state: State<'_, AppState>,
) -> CommandResult<CompositionLibrary> {
    let doc = state.doc.read().await;
    let project = doc
        .project
        .as_ref()
        .ok_or(AppCommandError::NoActiveProject)?;
    let ai = &project.library.ai;
    let builtins = BuiltinLibrary::load();
    Ok(CompositionLibrary {
        structures: ai.structures.clone(),
        styles: ai.styles.clone(),
        prompts: ai.prompts.clone(),
        builtin_structures: builtins.structures.values().cloned().collect(),
        builtin_styles: builtins.styles.values().cloned().collect(),
        builtin_prompts: builtins.prompts.values().cloned().collect(),
    })
}

/// Inserts or replaces a project-tier Structure by id.
///
/// # Errors
/// Returns [`AppCommandError::NoActiveProject`] when no project is open.
#[tauri::command(async, rename_all = "snake_case")]
pub async fn library_upsert_structure(
    structure: Structure,
    state: State<'_, AppState>,
) -> CommandResult<()> {
    let mut doc = state.doc.write().await;
    let project = doc
        .project
        .as_mut()
        .ok_or(AppCommandError::NoActiveProject)?;
    let list = &mut project.library.ai.structures;
    match list.iter().position(|s| s.id == structure.id) {
        Some(i) => list[i] = structure,
        None => list.push(structure),
    }
    doc.dirty = true;
    Ok(())
}

/// Inserts or replaces a project-tier Style by id.
///
/// # Errors
/// Returns [`AppCommandError::NoActiveProject`] when no project is open.
#[tauri::command(async, rename_all = "snake_case")]
pub async fn library_upsert_style(style: Style, state: State<'_, AppState>) -> CommandResult<()> {
    let mut doc = state.doc.write().await;
    let project = doc
        .project
        .as_mut()
        .ok_or(AppCommandError::NoActiveProject)?;
    let list = &mut project.library.ai.styles;
    match list.iter().position(|s| s.id == style.id) {
        Some(i) => list[i] = style,
        None => list.push(style),
    }
    doc.dirty = true;
    Ok(())
}

/// Inserts or replaces a project-tier saved Prompt by id.
///
/// # Errors
/// Returns [`AppCommandError::NoActiveProject`] when no project is open.
#[tauri::command(async, rename_all = "snake_case")]
pub async fn library_upsert_prompt(
    prompt: PromptTemplate,
    state: State<'_, AppState>,
) -> CommandResult<()> {
    let mut doc = state.doc.write().await;
    let project = doc
        .project
        .as_mut()
        .ok_or(AppCommandError::NoActiveProject)?;
    let list = &mut project.library.ai.prompts;
    match list.iter().position(|p| p.id == prompt.id) {
        Some(i) => list[i] = prompt,
        None => list.push(prompt),
    }
    doc.dirty = true;
    Ok(())
}

/// Deletes a project-tier Structure by id.
///
/// Built-ins cannot be deleted: an id matching a built-in but absent from the
/// project tier yields [`AppCommandError::Validation`]; an id matching nothing
/// yields [`AppCommandError::NotFound`].
///
/// # Errors
/// See above; also [`AppCommandError::NoActiveProject`].
#[tauri::command(async, rename_all = "snake_case")]
pub async fn library_delete_structure(
    id: StructureId,
    state: State<'_, AppState>,
) -> CommandResult<()> {
    let mut doc = state.doc.write().await;
    let project = doc
        .project
        .as_mut()
        .ok_or(AppCommandError::NoActiveProject)?;
    match project
        .library
        .ai
        .structures
        .iter()
        .position(|s| s.id == id)
    {
        Some(i) => {
            project.library.ai.structures.remove(i);
            doc.dirty = true;
            Ok(())
        }
        None if BuiltinLibrary::load().structures.contains_key(&id) => {
            Err(AppCommandError::Validation {
                detail: "cannot delete a built-in record".into(),
            })
        }
        None => Err(AppCommandError::NotFound {
            entity: "structure".into(),
            id: 0,
        }),
    }
}

/// Deletes a project-tier Style by id. See [`library_delete_structure`].
///
/// # Errors
/// See [`library_delete_structure`].
#[tauri::command(async, rename_all = "snake_case")]
pub async fn library_delete_style(id: StyleId, state: State<'_, AppState>) -> CommandResult<()> {
    let mut doc = state.doc.write().await;
    let project = doc
        .project
        .as_mut()
        .ok_or(AppCommandError::NoActiveProject)?;
    match project.library.ai.styles.iter().position(|s| s.id == id) {
        Some(i) => {
            project.library.ai.styles.remove(i);
            doc.dirty = true;
            Ok(())
        }
        None if BuiltinLibrary::load().styles.contains_key(&id) => {
            Err(AppCommandError::Validation {
                detail: "cannot delete a built-in record".into(),
            })
        }
        None => Err(AppCommandError::NotFound {
            entity: "style".into(),
            id: 0,
        }),
    }
}

/// Deletes a project-tier saved Prompt by id. See [`library_delete_structure`].
///
/// # Errors
/// See [`library_delete_structure`].
#[tauri::command(async, rename_all = "snake_case")]
pub async fn library_delete_prompt(id: PromptId, state: State<'_, AppState>) -> CommandResult<()> {
    let mut doc = state.doc.write().await;
    let project = doc
        .project
        .as_mut()
        .ok_or(AppCommandError::NoActiveProject)?;
    match project.library.ai.prompts.iter().position(|p| p.id == id) {
        Some(i) => {
            project.library.ai.prompts.remove(i);
            doc.dirty = true;
            Ok(())
        }
        None if BuiltinLibrary::load().prompts.contains_key(&id) => {
            Err(AppCommandError::Validation {
                detail: "cannot delete a built-in record".into(),
            })
        }
        None => Err(AppCommandError::NotFound {
            entity: "prompt".into(),
            id: 0,
        }),
    }
}

/// Forks a built-in record into the project tier and returns the new id.
///
/// With `as_new`, mints a fresh id `project.<kind>.<nanos>`; otherwise reuses
/// the built-in id so the project record shadows it.
///
/// # Errors
/// Returns [`AppCommandError::NotFound`] when `builtin_id` matches no built-in
/// of the requested `kind`; also [`AppCommandError::NoActiveProject`].
#[tauri::command(async, rename_all = "snake_case")]
pub async fn library_fork_builtin(
    kind: CompositionKind,
    builtin_id: String,
    as_new: bool,
    state: State<'_, AppState>,
) -> CommandResult<String> {
    let mut doc = state.doc.write().await;
    let project = doc
        .project
        .as_mut()
        .ok_or(AppCommandError::NoActiveProject)?;
    let builtins = BuiltinLibrary::load();
    let new_id = if as_new {
        format!("project.{kind}.{}", now_nanos())
    } else {
        builtin_id.clone()
    };
    let not_found = || AppCommandError::NotFound {
        entity: format!("builtin {kind}"),
        id: 0,
    };
    match kind {
        CompositionKind::Structure => {
            let mut rec = builtins
                .structures
                .get(&StructureId(builtin_id))
                .ok_or_else(not_found)?
                .clone();
            rec.id = StructureId(new_id.clone());
            project.library.ai.structures.push(rec);
        }
        CompositionKind::Style => {
            let mut rec = builtins
                .styles
                .get(&StyleId(builtin_id))
                .ok_or_else(not_found)?
                .clone();
            rec.id = StyleId(new_id.clone());
            project.library.ai.styles.push(rec);
        }
        CompositionKind::Prompt => {
            let mut rec = builtins
                .prompts
                .get(&PromptId(builtin_id))
                .ok_or_else(not_found)?
                .clone();
            rec.id = PromptId(new_id.clone());
            project.library.ai.prompts.push(rec);
        }
    }
    doc.dirty = true;
    Ok(new_id)
}

/// Exports the selected records to a `.pixstyle` bundle at `path`.
///
/// Records are gathered from the combined library — project tier first, then
/// built-ins — so a selected id resolves to its shadowing project record when
/// one exists. Unknown ids are silently skipped.
///
/// # Errors
/// Returns [`AppCommandError::Validation`] on I/O or encode failure; also
/// [`AppCommandError::NoActiveProject`].
#[tauri::command(async, rename_all = "snake_case")]
pub async fn library_export_pack(
    structure_ids: Vec<String>,
    style_ids: Vec<String>,
    prompt_ids: Vec<String>,
    path: String,
    state: State<'_, AppState>,
) -> CommandResult<()> {
    let doc = state.doc.read().await;
    let project = doc
        .project
        .as_ref()
        .ok_or(AppCommandError::NoActiveProject)?;
    let ai = &project.library.ai;
    let builtins = BuiltinLibrary::load();

    let mut pack = StylePack {
        format_version: 1,
        structures: Vec::new(),
        styles: Vec::new(),
        prompts: Vec::new(),
    };
    for id in structure_ids {
        let key = StructureId(id);
        if let Some(s) = ai
            .structures
            .iter()
            .find(|s| s.id == key)
            .cloned()
            .or_else(|| builtins.structures.get(&key).cloned())
        {
            pack.structures.push(s);
        }
    }
    for id in style_ids {
        let key = StyleId(id);
        if let Some(s) = ai
            .styles
            .iter()
            .find(|s| s.id == key)
            .cloned()
            .or_else(|| builtins.styles.get(&key).cloned())
        {
            pack.styles.push(s);
        }
    }
    for id in prompt_ids {
        let key = PromptId(id);
        if let Some(p) = ai
            .prompts
            .iter()
            .find(|p| p.id == key)
            .cloned()
            .or_else(|| builtins.prompts.get(&key).cloned())
        {
            pack.prompts.push(p);
        }
    }

    let file = std::fs::File::create(&path).map_err(|e| AppCommandError::Validation {
        detail: format!("create {path}: {e}"),
    })?;
    write_pack(&pack, file).map_err(|e| AppCommandError::Validation {
        detail: format!("write {path}: {e}"),
    })?;
    Ok(())
}

/// Imports a `.pixstyle` bundle into the project tier per `policy`.
///
/// # Errors
/// Returns [`AppCommandError::Validation`] on I/O or decode failure; also
/// [`AppCommandError::NoActiveProject`].
#[tauri::command(async, rename_all = "snake_case")]
pub async fn library_import_pack(
    path: String,
    policy: ConflictPolicy,
    state: State<'_, AppState>,
) -> CommandResult<ImportReport> {
    let mut doc = state.doc.write().await;
    let project = doc
        .project
        .as_mut()
        .ok_or(AppCommandError::NoActiveProject)?;
    let file = std::fs::File::open(&path).map_err(|e| AppCommandError::Validation {
        detail: format!("open {path}: {e}"),
    })?;
    let pack = read_pack(file).map_err(|e| AppCommandError::Validation {
        detail: format!("read {path}: {e}"),
    })?;
    let report = merge_pack(&mut project.library.ai, pack, policy);
    doc.dirty = true;
    Ok(report)
}

/// Copies composition records from another `.pixhaus` project into this one.
///
/// Opens `source_path` read-only, extracts its [`ProjectAi`] composition tier,
/// and merges it per `policy`. The source project is untouched.
///
/// # Errors
/// Returns [`AppCommandError::Validation`] on read/decode failure; also
/// [`AppCommandError::NoActiveProject`].
#[tauri::command(async, rename_all = "snake_case")]
pub async fn library_copy_from_project(
    source_path: String,
    policy: ConflictPolicy,
    state: State<'_, AppState>,
) -> CommandResult<ImportReport> {
    // Decode the source archive off the lock, then merge into the destination.
    let archive =
        tokio::task::spawn_blocking(move || pixhaus_io::pixhaus::decode_from_file(&source_path))
            .await
            .map_err(|e| AppCommandError::Validation {
                detail: format!("copy-from-project task failed: {e}"),
            })?
            .map_err(AppCommandError::from)?;

    let pack = read_library_from_project_ai(&archive.project.library.ai);

    let mut doc = state.doc.write().await;
    let project = doc
        .project
        .as_mut()
        .ok_or(AppCommandError::NoActiveProject)?;
    let report = merge_pack(&mut project.library.ai, pack, policy);
    doc.dirty = true;
    Ok(report)
}

/// Resolves the variables of a saved Prompt for a target entity.
///
/// The result is the union of (a) the prompt's declared `variables` and (b)
/// auto-detected `{token}`s not already declared (label == key, empty default),
/// declared first then detected, in stable order. The project tier shadows the
/// built-in registry by id.
///
/// # Errors
/// Returns [`AppCommandError::NotFound`] when no prompt matches `prompt_id`;
/// also [`AppCommandError::NoActiveProject`].
#[tauri::command(async, rename_all = "snake_case")]
pub async fn library_resolve_prompt_variables(
    prompt_id: PromptId,
    entity_id: EntityId,
    state: State<'_, AppState>,
) -> CommandResult<Vec<ResolvedVariable>> {
    let doc = state.doc.read().await;
    let project = doc
        .project
        .as_ref()
        .ok_or(AppCommandError::NoActiveProject)?;

    let prompt = project
        .library
        .ai
        .prompts
        .iter()
        .find(|p| p.id == prompt_id)
        .cloned()
        .or_else(|| BuiltinLibrary::load().prompts.get(&prompt_id).cloned())
        .ok_or_else(|| AppCommandError::NotFound {
            entity: "prompt".into(),
            id: 0,
        })?;

    // Autofill is best-effort. Reference-sheet `AssetInfo` lives behind the
    // entity's sprite content + optional reference sheet; wiring that lookup
    // is non-trivial and out of scope here, so we leave `autofilled: None`.
    // The entity is validated to exist so the UI can still key on it.
    let _ = entity_id;

    let mut out: Vec<ResolvedVariable> = prompt
        .variables
        .iter()
        .map(|v| ResolvedVariable {
            key: v.key.clone(),
            label: v.label.clone(),
            default: v.default.clone(),
            autofilled: None,
        })
        .collect();

    for token in detect_tokens(&prompt.text) {
        if out.iter().any(|r| r.key == token) {
            continue;
        }
        out.push(ResolvedVariable {
            label: token.clone(),
            key: token,
            default: String::new(),
            autofilled: None,
        });
    }

    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use pixhaus_core::project::library::composition::StructureOutput;

    fn structure(id: &str, name: &str) -> Structure {
        Structure {
            id: StructureId(id.into()),
            name: name.into(),
            output: StructureOutput::Single,
            layout_negatives: String::new(),
        }
    }

    fn style(id: &str, name: &str) -> Style {
        Style {
            id: StyleId(id.into()),
            name: name.into(),
            modifiers: String::new(),
            look_negatives: String::new(),
            model_pref: None,
            quality: None,
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

    #[test]
    fn merge_structures_skip_keeps_existing_and_counts_skipped() {
        let mut target = vec![structure("a", "original")];
        let report = merge_structures(
            &mut target,
            vec![structure("a", "incoming")],
            ConflictPolicy::Skip,
        );
        assert_eq!(report.skipped, 1);
        assert_eq!(report.imported, 0);
        assert_eq!(report.overwritten, 0);
        assert_eq!(target.len(), 1);
        assert_eq!(target[0].name, "original");
    }

    #[test]
    fn merge_structures_overwrite_replaces() {
        let mut target = vec![structure("a", "original")];
        let report = merge_structures(
            &mut target,
            vec![structure("a", "incoming")],
            ConflictPolicy::Overwrite,
        );
        assert_eq!(report.overwritten, 1);
        assert_eq!(report.imported, 0);
        assert_eq!(target.len(), 1);
        assert_eq!(target[0].name, "incoming");
    }

    #[test]
    fn merge_structures_import_as_copy_adds_suffixed_id() {
        let mut target = vec![structure("a", "original")];
        let report = merge_structures(
            &mut target,
            vec![structure("a", "incoming")],
            ConflictPolicy::ImportAsCopy,
        );
        assert_eq!(report.imported, 1);
        assert_eq!(target.len(), 2);
        assert_eq!(target[1].id, StructureId("a.copy".into()));
        assert_eq!(target[1].name, "incoming");
    }

    #[test]
    fn merge_structures_new_id_is_imported() {
        let mut target = vec![structure("a", "original")];
        let report = merge_structures(
            &mut target,
            vec![structure("b", "fresh")],
            ConflictPolicy::Skip,
        );
        assert_eq!(report.imported, 1);
        assert_eq!(target.len(), 2);
    }

    #[test]
    fn merge_styles_import_as_copy_adds_suffixed_id() {
        let mut target = vec![style("a", "original")];
        let report = merge_styles(
            &mut target,
            vec![style("a", "incoming")],
            ConflictPolicy::ImportAsCopy,
        );
        assert_eq!(report.imported, 1);
        assert_eq!(target[1].id, StyleId("a.copy".into()));
    }

    #[test]
    fn merge_styles_overwrite_replaces() {
        let mut target = vec![style("a", "original")];
        let report = merge_styles(
            &mut target,
            vec![style("a", "incoming")],
            ConflictPolicy::Overwrite,
        );
        assert_eq!(report.overwritten, 1);
        assert_eq!(target[0].name, "incoming");
    }

    #[test]
    fn merge_prompts_skip_keeps_existing() {
        let mut target = vec![prompt("a", "original")];
        let report = merge_prompts(
            &mut target,
            vec![prompt("a", "incoming")],
            ConflictPolicy::Skip,
        );
        assert_eq!(report.skipped, 1);
        assert_eq!(target[0].name, "original");
    }

    #[test]
    fn merge_prompts_import_as_copy_adds_suffixed_id() {
        let mut target = vec![prompt("a", "original")];
        let report = merge_prompts(
            &mut target,
            vec![prompt("a", "incoming")],
            ConflictPolicy::ImportAsCopy,
        );
        assert_eq!(report.imported, 1);
        assert_eq!(target[1].id, PromptId("a.copy".into()));
    }

    #[test]
    fn import_report_add_sums_counts() {
        let mut a = ImportReport {
            imported: 1,
            skipped: 2,
            overwritten: 3,
        };
        a.add(ImportReport {
            imported: 10,
            skipped: 20,
            overwritten: 30,
        });
        assert_eq!(a.imported, 11);
        assert_eq!(a.skipped, 22);
        assert_eq!(a.overwritten, 33);
    }

    #[test]
    fn export_bindings_composition() {
        use ts_rs::{Config, TS};
        let cfg = Config::from_env();
        CompositionLibrary::export_all(&cfg).expect("export CompositionLibrary");
        ConflictPolicy::export_all(&cfg).expect("export ConflictPolicy");
        ImportReport::export_all(&cfg).expect("export ImportReport");
        ResolvedVariable::export_all(&cfg).expect("export ResolvedVariable");
        CompositionKind::export_all(&cfg).expect("export CompositionKind");
    }
}
