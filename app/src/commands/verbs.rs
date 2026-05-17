//! AI verb invocation commands.
//!
//! `verb_list` returns the descriptors of all registered verbs, including
//! plugin-registered verbs (the plugin registry shares the same runtime).
//! `verb_invoke` runs a verb and returns its output plus an invocation
//! handle that `verb_cancel` can use to interrupt long-running calls.
//! In-flight invocations live in `AppState::invocations`, keyed by the
//! runtime's `PreviewId` (rendered as a string at the IPC boundary so
//! the JS side never has to think about u64 precision).

use serde::{Deserialize, Serialize};
use tauri::State;

use pixhaus_ai::plugin::AnchorPayload;
use pixhaus_ai::plugin::DEFAULT_ANCHOR_STRENGTH;
use pixhaus_ai::plugin::context::VerbContext;
use pixhaus_ai::plugin::descriptor::VerbId;
use pixhaus_ai::plugin::inputs::VerbInputs;
use pixhaus_ai::plugin::output::VerbOutput;
use pixhaus_core::project::{ActiveTarget, EntityContent, EntityId, Project, ProjectMetadata};

use crate::error::{AppCommandError, CommandResult};
use crate::state::AppState;

/// Arguments for invoking a verb.
#[derive(Debug, Deserialize)]
pub struct VerbInvokeArgs {
    /// Stable verb ID (e.g. `"pixhaus.builtin.critique"`).
    pub verb_id: String,
    /// JSON payload whose schema is defined by the verb's descriptor.
    pub inputs: serde_json::Value,
    /// Optional override for the entity to use when resolving the
    /// embedded reference sheet.
    ///
    /// When `None`, the anchor target is derived from the project's
    /// [`ActiveTarget`]: a Custom-state target uses its parent entity,
    /// and a Tileset / Tilemap target uses the named entity itself. Pass
    /// `Some(entity_id)` from verbs that target an entity other than the
    /// active one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub target_entity_id: Option<EntityId>,
}

/// Result of a verb invocation: the output the host commits, plus the
/// opaque handle the UI hands back to `verb_cancel` if the user
/// interrupts a still-running invocation.
#[derive(Debug, Serialize)]
pub struct VerbInvocationResult {
    /// Per-invocation handle. Stringified `PreviewId` so the JS side
    /// doesn't lose precision on values above 2^53.
    pub invocation_id: String,
    /// The verb's output, ready for preview / commit.
    pub output: VerbOutput,
}

/// Metadata about an available verb.
#[derive(Debug, Serialize)]
pub struct VerbInfo {
    /// Stable verb ID, used in `verb_invoke`.
    pub id: String,
    /// Display name for menus and the command palette.
    pub display_name: String,
    /// One-line description shown in the command palette.
    pub description: String,
    /// Whether the verb can be cancelled mid-run.
    pub cancellable: bool,
    /// Backend capabilities required to run this verb.
    pub required_capabilities: u32,
    /// JSON Schema for the verb's input payload — surfaced so the UI
    /// can render an input form without baking per-verb knowledge.
    pub input_schema: serde_json::Value,
}

/// Lists all verbs registered with the runtime, sorted by ID.
#[tauri::command(async, rename_all = "snake_case")]
pub async fn verb_list(state: State<'_, AppState>) -> CommandResult<Vec<VerbInfo>> {
    let descriptors = state.verb_runtime.list();
    Ok(descriptors
        .into_iter()
        .map(|d| VerbInfo {
            id: d.id.as_str().to_owned(),
            display_name: d.display_name,
            description: d.description,
            cancellable: d.cancellable,
            required_capabilities: d.required_capabilities.0,
            input_schema: d.input_schema,
        })
        .collect())
}

/// Invokes a registered verb synchronously and returns its output.
///
/// Builds a [`VerbContext`] from the current document state, dispatches
/// through the [`pixhaus_ai::plugin::runtime::VerbRuntime`], awaits
/// completion, and returns the [`VerbOutput`].
///
/// Returns an error when:
/// - the verb ID is not registered,
/// - the inputs fail schema validation,
/// - no backend satisfies the verb's required capabilities,
/// - the verb itself returns an error.
#[tauri::command(async, rename_all = "snake_case")]
pub async fn verb_invoke(
    args: VerbInvokeArgs,
    state: State<'_, AppState>,
) -> CommandResult<VerbInvocationResult> {
    let verb_id = VerbId::new(&args.verb_id);
    let inputs = VerbInputs::new(args.inputs);

    // Build a minimal VerbContext from the current document state. The
    // full context (sprite, palette, references) requires a read guard on
    // the document; we release it before awaiting the verb so we never
    // hold a lock across an I/O suspension.
    //
    // The anchor is resolved here too: the host looks at the active or
    // override sprite entity's embedded reference sheet and builds an
    // `AnchorPayload`. The result lands on `ctx.anchor`; verbs that
    // ignore it pay nothing.
    let ctx = {
        let doc = state.doc.read().await;
        let project_meta = doc.project.as_ref().map_or_else(
            || ProjectMetadata {
                name: "untitled".into(),
                description: None,
                author: None,
                created_at: 0,
                updated_at: 0,
                editor_version: env!("CARGO_PKG_VERSION").into(),
            },
            |p| p.metadata.clone(),
        );
        let anchor = doc
            .project
            .as_ref()
            .and_then(|p| resolve_anchor(p, args.target_entity_id, &state.anchor_cache));
        let mut ctx = VerbContext::empty(project_meta);
        ctx.anchor = anchor;
        ctx
        // doc guard drops here
    };

    let invocation = state
        .verb_runtime
        .invoke(&verb_id, ctx, inputs)
        .map_err(|e| AppCommandError::VerbError {
            message: e.to_string(),
        })?;

    // Register the cancel token before awaiting the verb body so a
    // concurrent `verb_cancel` IPC call can find and fire it.
    let preview_id = invocation.preview_id().get();
    state
        .invocations
        .insert(preview_id, invocation.cancellation());

    let result = invocation.finish().await;

    // Always remove the entry — successful, cancelled, or errored.
    state.invocations.remove(&preview_id);

    let preview = result.map_err(|e| AppCommandError::VerbError {
        message: e.to_string(),
    })?;

    Ok(VerbInvocationResult {
        invocation_id: preview_id.to_string(),
        output: preview.output,
    })
}

/// Cancels an in-progress verb invocation by its opaque ID.
///
/// `invocation_id` is the value returned by `verb_invoke` (a stringified
/// `PreviewId`). Cancellation is cooperative: the verb observes the
/// token between expensive operations and returns
/// [`pixhaus_ai::plugin::error::VerbError::Cancelled`] when it sees the
/// fire. Idempotent — a missing id (already finished or never seen) is
/// not an error.
#[tauri::command(async, rename_all = "snake_case")]
pub async fn verb_cancel(invocation_id: String, state: State<'_, AppState>) -> CommandResult<()> {
    let id: u64 = invocation_id
        .parse()
        .map_err(|_| AppCommandError::Validation {
            detail: format!("invocation_id is not a valid u64: {invocation_id:?}"),
        })?;
    if let Some((_, token)) = state.invocations.remove(&id) {
        token.cancel();
    }
    Ok(())
}

/// Resolves the anchor payload for a verb invocation.
///
/// The active entity is determined as follows:
///
/// 1. If `target_override` is supplied, that entity wins.
/// 2. Otherwise the project's [`ActiveTarget`] picks the entity:
///    - `State { entity_id, .. }` → the parent Custom entity.
///    - `Tileset { entity_id }` / `Tilemap { entity_id }` → the named
///      entity itself.
///    - `None` → no anchor.
///
/// Once the active entity is in hand, build the payload from its
/// embedded sprite reference sheet. Entities without a sheet return
/// `None`.
///
/// The cache (keyed by sprite entity id) returns a hit when the stored
/// `canonical_hash` and resolved `LoRA` path both match the live project
/// state; stale entries are rebuilt and reinserted.
pub(crate) fn resolve_anchor(
    project: &Project,
    target_override: Option<EntityId>,
    cache: &dashmap::DashMap<u32, AnchorPayload>,
) -> Option<AnchorPayload> {
    let active_entity_id = target_override.or_else(|| active_target_entity_id(&project.active))?;
    let active = project
        .library
        .entities
        .iter()
        .find(|e| e.id == active_entity_id)?;

    // Cheap hash of the live canonical bytes — no base64 alloc.
    // Cache hits skip the AnchorPayload::from_sprite_entity build
    // entirely, which would otherwise clone the bytes and base64-encode
    // a ~megabyte payload on every verb invocation.
    let sheet = match &active.content {
        EntityContent::Sprites {
            reference_sheet: Some(sheet),
            ..
        } => sheet.as_ref(),
        _ => return None,
    };
    let canonical = sheet.canonical.as_ref()?;
    let live_hash = pixhaus_ai::plugin::anchor::stable_hash(&canonical.image.bytes);
    let lora_path = resolve_lora_path(active, project.library.ai.project_lora_path.as_deref());

    if let Some(cached) = cache.get(&active.id.get()) {
        if cached.canonical_hash == live_hash && cached.lora_path == lora_path {
            return Some(cached.clone());
        }
    }

    let live = AnchorPayload::from_sprite_entity(active, DEFAULT_ANCHOR_STRENGTH, lora_path)?;
    cache.insert(live.reference_entity_id.get(), live.clone());
    Some(live)
}

/// Returns the `LoRA` path to thread through the anchor payload.
///
/// Per-entity weights (`Entity.ai.lora_path`, populated by the B10.5
/// train-entity-lora verb) override the project-wide weights when both
/// are present — generations against this entity should be conditioned
/// on the entity's own sheet rather than the broader project style.
/// Falls back to the project-wide path when the entity has none.
pub(crate) fn resolve_lora_path(
    reference: &pixhaus_core::project::Entity,
    project_lora_path: Option<&str>,
) -> Option<String> {
    reference
        .ai
        .lora_path
        .clone()
        .or_else(|| project_lora_path.map(ToOwned::to_owned))
}

/// Maps an [`ActiveTarget`] back to the entity id it implicates.
fn active_target_entity_id(active: &ActiveTarget) -> Option<EntityId> {
    match *active {
        ActiveTarget::None => None,
        ActiveTarget::State { entity_id, .. }
        | ActiveTarget::Tileset { entity_id }
        | ActiveTarget::Tilemap { entity_id } => Some(entity_id),
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use dashmap::DashMap;
    use pixhaus_core::project::{
        AiMetadata, AssetInfo, Entity, EntityContent, EntityDefaults, EntityKind, ReferenceImage,
        ReferenceSheet, SheetComposition, SheetVariant, SheetVariantId, StateId, UserData,
    };

    use super::*;

    fn sprite_entity(id: u32, bytes: Option<Vec<u8>>) -> Entity {
        let reference_sheet = bytes.map(|bytes| {
            Box::new(ReferenceSheet {
                canonical: Some(SheetVariant {
                    id: SheetVariantId::new(1),
                    generated_at: 0,
                    image: ReferenceImage {
                        bytes,
                        mime: "image/png".into(),
                    },
                    composition: SheetComposition::default(),
                    generation: None,
                    extracted_palette: Vec::new(),
                }),
                history: Vec::new(),
                prompts: Vec::new(),
                info: AssetInfo {
                    fields: BTreeMap::new(),
                    notes: Vec::new(),
                },
            })
        });
        Entity {
            id: EntityId::new(id),
            kind: EntityKind::Custom("Hero".into()),
            name: format!("Hero {id}"),
            group_id: None,
            tags: Vec::new(),
            defaults: EntityDefaults::default(),
            content: EntityContent::Sprites {
                states: Vec::new(),
                reference_sheet,
            },
            ai: AiMetadata::default(),
            user_data: UserData::default(),
            created_at: 0,
            updated_at: 0,
        }
    }

    #[test]
    fn verb_info_fields_are_present() {
        let info = VerbInfo {
            id: "pixhaus.builtin.critique".into(),
            display_name: "Critique".into(),
            description: "VLM quality analysis".into(),
            cancellable: true,
            required_capabilities: 0b10,
            input_schema: serde_json::json!({"type": "object"}),
        };
        assert!(!info.id.is_empty());
        assert!(info.cancellable);
    }

    #[test]
    fn resolve_anchor_returns_none_for_no_target() {
        let project = Project::new("none");
        let cache = DashMap::new();
        assert!(resolve_anchor(&project, None, &cache).is_none());
    }

    #[test]
    fn resolve_anchor_uses_target_override() {
        let mut project = Project::new("override");
        project
            .library
            .entities
            .push(sprite_entity(7, Some(vec![1, 2, 3])));
        let cache = DashMap::new();
        let p = resolve_anchor(&project, Some(EntityId::new(7)), &cache).unwrap();
        assert_eq!(p.reference_entity_id, EntityId::new(7));
        assert_eq!(p.image_bytes, vec![1, 2, 3]);
    }

    #[test]
    fn resolve_anchor_uses_active_state_entity_sheet() {
        let mut project = Project::new("anchored");
        project
            .library
            .entities
            .push(sprite_entity(9, Some(vec![1, 2, 3])));
        project.active = ActiveTarget::State {
            entity_id: EntityId::new(9),
            state_id: StateId::new(1),
        };
        let cache = DashMap::new();
        let p = resolve_anchor(&project, None, &cache).unwrap();
        assert_eq!(p.reference_entity_id, EntityId::new(9));
    }

    #[test]
    fn resolve_anchor_returns_none_for_unanchored_custom() {
        let mut project = Project::new("unanchored");
        project.library.entities.push(sprite_entity(9, None));
        project.active = ActiveTarget::State {
            entity_id: EntityId::new(9),
            state_id: StateId::new(1),
        };
        let cache = DashMap::new();
        assert!(resolve_anchor(&project, None, &cache).is_none());
    }

    #[test]
    fn resolve_anchor_returns_none_for_draft_only_sheet() {
        let mut project = Project::new("draft-only");
        project
            .library
            .entities
            .push(sprite_entity(9, Some(vec![1, 2, 3])));
        if let EntityContent::Sprites {
            reference_sheet: Some(sheet),
            ..
        } = &mut project.library.entities[0].content
        {
            let canonical = sheet.canonical.take().expect("canonical fixture");
            sheet.history.push(canonical);
        }
        let cache = DashMap::new();
        assert!(resolve_anchor(&project, Some(EntityId::new(9)), &cache).is_none());
    }

    #[test]
    fn resolve_anchor_caches_payload_and_serves_from_cache() {
        let mut project = Project::new("cache");
        project
            .library
            .entities
            .push(sprite_entity(7, Some(vec![1, 2, 3])));
        let cache = DashMap::new();

        let p1 = resolve_anchor(&project, Some(EntityId::new(7)), &cache).unwrap();
        assert_eq!(cache.len(), 1);

        // Second call with the same unchanged project: the cache hit
        // path returns the same payload (same hash). The companion
        // `resolve_anchor_invalidates_cache_when_canonical_changes`
        // test below exercises the mutation case.
        let p2 = resolve_anchor(&project, Some(EntityId::new(7)), &cache).unwrap();
        assert_eq!(p2.canonical_hash, p1.canonical_hash);
    }

    #[test]
    fn resolve_anchor_invalidates_cache_when_canonical_changes() {
        let mut project = Project::new("cache-invalidate");
        project
            .library
            .entities
            .push(sprite_entity(7, Some(vec![1, 2, 3])));
        let cache = DashMap::new();

        let p1 = resolve_anchor(&project, Some(EntityId::new(7)), &cache).unwrap();

        // Change the canonical bytes.
        if let EntityContent::Sprites {
            reference_sheet: Some(sheet),
            ..
        } = &mut project.library.entities[0].content
        {
            if let Some(canonical) = &mut sheet.canonical {
                canonical.image.bytes = vec![9, 9, 9];
            }
        }
        let p2 = resolve_anchor(&project, Some(EntityId::new(7)), &cache).unwrap();
        assert_ne!(
            p1.canonical_hash, p2.canonical_hash,
            "hash must change when bytes change"
        );
        assert_eq!(p2.image_bytes, vec![9, 9, 9]);
    }

    #[test]
    fn resolve_anchor_invalidates_cache_when_entity_lora_changes() {
        let mut project = Project::new("cache-lora");
        project
            .library
            .entities
            .push(sprite_entity(7, Some(vec![1, 2, 3])));
        let cache = DashMap::new();

        let p1 = resolve_anchor(&project, Some(EntityId::new(7)), &cache).unwrap();
        assert_eq!(p1.lora_path, None);

        project.library.entities[0].ai.lora_path = Some("entity.safetensors".into());
        let p2 = resolve_anchor(&project, Some(EntityId::new(7)), &cache).unwrap();
        assert_eq!(p2.canonical_hash, p1.canonical_hash);
        assert_eq!(p2.lora_path.as_deref(), Some("entity.safetensors"));
    }

    #[test]
    fn resolve_anchor_invalidates_cache_when_project_lora_changes() {
        let mut project = Project::new("cache-project-lora");
        project
            .library
            .entities
            .push(sprite_entity(7, Some(vec![1, 2, 3])));
        project.library.ai.project_lora_path = Some("project-a.safetensors".into());
        let cache = DashMap::new();

        let p1 = resolve_anchor(&project, Some(EntityId::new(7)), &cache).unwrap();
        assert_eq!(p1.lora_path.as_deref(), Some("project-a.safetensors"));

        project.library.ai.project_lora_path = Some("project-b.safetensors".into());
        let p2 = resolve_anchor(&project, Some(EntityId::new(7)), &cache).unwrap();
        assert_eq!(p2.canonical_hash, p1.canonical_hash);
        assert_eq!(p2.lora_path.as_deref(), Some("project-b.safetensors"));
    }

    #[test]
    fn resolve_anchor_returns_none_for_missing_override() {
        let mut project = Project::new("stale");
        project.library.entities.push(sprite_entity(9, None));
        project.active = ActiveTarget::State {
            entity_id: EntityId::new(9),
            state_id: StateId::new(1),
        };
        let cache = DashMap::new();
        // Should not panic, returns None.
        assert!(resolve_anchor(&project, Some(EntityId::new(99)), &cache).is_none());
    }
}
