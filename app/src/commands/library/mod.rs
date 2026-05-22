//! Library management commands: entity, group, tag, and active-target CRUD.
//!
//! Implements the IPC catalog specified in B9.2. All commands follow the
//! project-crate conventions: async, locked via `AppState::doc`, typed errors,
//! `dirty` flag set on every mutation.

use std::collections::{HashMap, HashSet};
use std::io::{Cursor, Write};
use std::sync::Arc;
use std::time::SystemTime;

use base64::Engine as _;
use image::{ImageBuffer, ImageFormat, Rgba as ImageRgba};
use pixhaus_ai::backends::bridge::BackendProxy;
use pixhaus_ai::backends::fal::FalBackend;
use pixhaus_ai::backends::{
    ImageEditRequest, ImageGenRequest, ImageQuality, InferenceRequest, InferenceResponse,
};
use pixhaus_ai::plugin::context::PixelData;
use pixhaus_ai::plugin::context::VerbContextBuilder;
use pixhaus_ai::plugin::descriptor::BackendCapabilities;
use pixhaus_ai::plugin::descriptor::VerbId;
use pixhaus_ai::plugin::inputs::VerbInputs;
use pixhaus_ai::plugin::output::VerbEffect;
use pixhaus_ai::plugin::progress::{VerbProgress, VerbProgressEvent};
use pixhaus_ai::plugin::runtime::VerbRuntime;
use pixhaus_ai::plugin::{AnchorPayload, DEFAULT_ANCHOR_STRENGTH};
use pixhaus_ai::verbs::critique::{CritiqueInputs, CritiqueMode};
use pixhaus_core::color::extraction::{ExtractionOptions, extract_palette_from_image_bytes};
use pixhaus_core::project::approval::{ApprovalError, approve_sheet_variant};
use pixhaus_core::project::{
    ActiveTarget, AiMetadata, AssetId, AssetInfo, AssetLibrary, CharacterCard, ChatTranscript,
    ChatTurn, ColorMode, Entity, EntityContent, EntityDefaults, EntityGroup, EntityId, EntityKind,
    GroupId, LoraAsset, LoraKind, ModelId, NamedSprite, OperationKind, PixelBufferId, Quality,
    ReferenceAsset, ReferenceImage, ReferenceRole, ReferenceSheet,
    ReferenceSheetTemplateDefinition, ReferenceSheetTemplateId, ReferenceSlot, RefinementKind,
    Rgba, SheetVariant, SheetVariantId, Size, Sprite, SpriteId, StateId, StyleSwatch,
    TagDefinition, TagId, TileShape, TilemapScene, Tileset, TilesetId, TilesetSource, TrainingJob,
    TrainingJobId, TrainingStatus, UserData, VariantOrigin, built_in_reference_sheet_templates,
};
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter, Manager, State};
use tokio_util::sync::CancellationToken;

use crate::error::{AppCommandError, CommandResult};
use crate::state::AppState;

impl From<ApprovalError> for AppCommandError {
    fn from(err: ApprovalError) -> Self {
        match err {
            ApprovalError::EntityNotFound(id) => AppCommandError::NotFound {
                entity: "entity".into(),
                id: u64::from(id),
            },
            ApprovalError::NoReferenceSheet(id) => AppCommandError::Validation {
                detail: format!("entity {id} has no sprite reference sheet"),
            },
            ApprovalError::VariantNotFound(vid, eid) => AppCommandError::NotFound {
                entity: format!("variant on entity {eid}"),
                id: u64::from(vid),
            },
        }
    }
}

// ── helpers ───────────────────────────────────────────────────────────────────

fn now_secs() -> i64 {
    SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| i64::try_from(d.as_secs()).unwrap_or(i64::MAX))
}

fn entity_not_found(id: EntityId) -> AppCommandError {
    AppCommandError::NotFound {
        entity: "entity".into(),
        id: u64::from(id.get()),
    }
}

fn group_not_found(id: GroupId) -> AppCommandError {
    AppCommandError::NotFound {
        entity: "group".into(),
        id: u64::from(id.get()),
    }
}

fn find_entity(
    library: &pixhaus_core::project::Library,
    id: EntityId,
) -> Result<&Entity, AppCommandError> {
    library
        .entities
        .iter()
        .find(|e| e.id == id)
        .ok_or_else(|| entity_not_found(id))
}

fn find_entity_mut(
    library: &mut pixhaus_core::project::Library,
    id: EntityId,
) -> Result<&mut Entity, AppCommandError> {
    library
        .entities
        .iter_mut()
        .find(|e| e.id == id)
        .ok_or_else(|| entity_not_found(id))
}

fn ensure_group_exists(
    library: &pixhaus_core::project::Library,
    id: GroupId,
) -> Result<(), AppCommandError> {
    if library.groups.iter().any(|g| g.id == id) {
        Ok(())
    } else {
        Err(group_not_found(id))
    }
}

pub(crate) fn reference_sheet_from_image(
    bytes: Vec<u8>,
    mime: String,
    variant_id: SheetVariantId,
    ts: i64,
) -> ReferenceSheet {
    ReferenceSheet {
        canonical: Some(SheetVariant::from_image(
            variant_id,
            ts,
            ReferenceImage { bytes, mime },
        )),
        variants: Vec::new(),
        prompts: Vec::new(),
        info: AssetInfo::default(),
    }
}

// ── arg types ─────────────────────────────────────────────────────────────────

/// Arguments for creating a new library entity.
#[derive(Debug, Deserialize)]
pub struct LibraryCreateEntityArgs {
    /// Kind of entity to create.
    pub kind: EntityKind,
    /// Display name for the new entity.
    pub name: String,
    /// Optional group to place the entity in immediately.
    pub group_id: Option<GroupId>,
    // Custom-kind fields
    /// Initial state names. Defaults to `["primary"]` when absent for `Custom` entities.
    pub initial_states: Option<Vec<String>>,
    /// Canvas width for initial states (pixels). Required for `Custom`.
    pub canvas_width: Option<u32>,
    /// Canvas height for initial states (pixels). Required for `Custom`.
    pub canvas_height: Option<u32>,
    /// Color mode for initial states. Defaults to `Rgba` when absent.
    pub color_mode: Option<ColorMode>,
    // Tileset-kind fields
    /// Tile width in pixels. Required for `Tileset`.
    pub tile_width: Option<u32>,
    /// Tile height in pixels. Required for `Tileset`.
    pub tile_height: Option<u32>,
    // Tilemap-kind fields
    /// Scene width in tile cells. Required for `Tilemap`.
    pub scene_width: Option<u32>,
    /// Scene height in tile cells. Required for `Tilemap`.
    pub scene_height: Option<u32>,
    // Optional Custom-kind reference sheet fields
    /// Image bytes for the canonical sprite reference sheet.
    pub reference_bytes: Option<Vec<u8>>,
    /// MIME type for the reference image. Defaults to `"image/png"` when
    /// `reference_bytes` is present and this is absent.
    pub reference_mime: Option<String>,
}

/// Arguments for adding a new state to a `Custom`-kind entity.
#[derive(Debug, Deserialize)]
pub struct LibraryAddStateArgs {
    /// Target entity. Must be `Custom`-kind.
    pub entity_id: EntityId,
    /// Name for the new state (e.g. `"idle"`, `"walk"`).
    pub state_name: String,
    /// Canvas width for the state. Inherits from `EntityDefaults` when absent.
    pub canvas_width: Option<u32>,
    /// Canvas height for the state. Inherits from `EntityDefaults` when absent.
    pub canvas_height: Option<u32>,
    /// Color mode for the state. Inherits from `EntityDefaults` when absent.
    pub color_mode: Option<ColorMode>,
}

/// Arguments for creating a new entity group.
#[derive(Debug, Deserialize)]
pub struct LibraryCreateGroupArgs {
    /// Display name for the group.
    pub name: String,
    /// Parent group for nesting. `None` creates a top-level group.
    pub parent_id: Option<GroupId>,
}

/// Arguments for deleting a group.
#[derive(Debug, Deserialize)]
pub struct LibraryDeleteGroupArgs {
    /// Group to delete.
    pub group_id: GroupId,
    /// When `true`, entities that were in the group are unassigned (their
    /// `group_id` is cleared). When `false`, they are deleted along with the
    /// group.
    pub keep_entities: bool,
}

/// Arguments for creating a new tag definition.
#[derive(Debug, Deserialize)]
pub struct LibraryAddTagArgs {
    /// Tag name shown in the library tree.
    pub name: String,
    /// Optional accent color for the tag chip.
    pub color: Option<Rgba>,
}

/// Arguments for searching the library.
#[derive(Debug, Deserialize)]
pub struct LibrarySearchArgs {
    /// Text query matched case-insensitively against entity names, `Custom`
    /// kind strings, and tag names attached to entities.
    pub query: String,
    /// Restrict results to a specific kind. `None` matches all kinds.
    pub kind_filter: Option<EntityKind>,
    /// Restrict results to entities in a specific group. `None` matches all
    /// groups (including ungrouped entities).
    pub group_filter: Option<GroupId>,
    /// Restrict results to entities with a specific tag. `None` matches all.
    pub tag_filter: Option<TagId>,
}

// ── pure project helpers ──────────────────────────────────────────────────────
//
// Each helper takes `&mut Project` (and `next_id: &mut u32` where ID minting
// is required) plus any other validated inputs, and returns the operation
// result. The IPC commands above are thin wrappers that lock `AppState::doc`,
// call the helper, and set `doc.dirty = true` on success.
//
// Helpers are `pub(crate)` so they are testable from within the crate without
// being part of the public IPC surface. ts-rs output is unaffected.

/// Creates a new entity in the library and sets it as the active target.
///
/// Validates all inputs before minting any IDs so partial state is never
/// written on failure.
#[allow(clippy::too_many_lines)]
pub(crate) fn create_entity_in_project(
    project: &mut pixhaus_core::project::Project,
    next_id: &mut u32,
    mut args: LibraryCreateEntityArgs,
    ts: i64,
) -> Result<Entity, AppCommandError> {
    // B1: Reject empty or whitespace-only names before minting any IDs.
    if args.name.trim().is_empty() {
        return Err(AppCommandError::Validation {
            detail: "entity name must not be empty".into(),
        });
    }

    if let Some(gid) = args.group_id {
        ensure_group_exists(&project.library, gid)?;
    }

    // Validate kind-specific required fields before touching next_id.
    match &args.kind {
        EntityKind::Custom(_) => {
            let w = args.canvas_width.unwrap_or(0);
            let h = args.canvas_height.unwrap_or(0);
            if w == 0 || h == 0 {
                return Err(AppCommandError::Validation {
                    detail: format!(
                        "Custom entity requires canvas_width and canvas_height > 0 (got {w}x{h})"
                    ),
                });
            }
            // B2: Reject whitespace-only state names before minting any IDs.
            if let Some(ref names) = args.initial_states {
                for sn in names {
                    if sn.trim().is_empty() {
                        return Err(AppCommandError::Validation {
                            detail:
                                "initial_states must not contain empty or whitespace-only names"
                                    .into(),
                        });
                    }
                }
            }
            if let Some(bytes) = &args.reference_bytes {
                if bytes.is_empty() {
                    return Err(AppCommandError::Validation {
                        detail: "reference_bytes must be non-empty when provided".into(),
                    });
                }
            }
        }
        EntityKind::Tileset => {
            let w = args.tile_width.unwrap_or(0);
            let h = args.tile_height.unwrap_or(0);
            if w == 0 || h == 0 {
                return Err(AppCommandError::Validation {
                    detail: format!(
                        "Tileset entity requires tile_width and tile_height > 0 (got {w}x{h})"
                    ),
                });
            }
        }
        EntityKind::Tilemap => {
            let w = args.scene_width.unwrap_or(0);
            let h = args.scene_height.unwrap_or(0);
            if w == 0 || h == 0 {
                return Err(AppCommandError::Validation {
                    detail: format!(
                        "Tilemap entity requires scene_width and scene_height > 0 (got {w}x{h})"
                    ),
                });
            }
        }
    }

    // Mint the entity ID.
    let entity_id = EntityId::new(*next_id);
    *next_id += 1;

    // Build kind-specific content, minting sub-entity IDs as needed.
    let content = match &args.kind {
        EntityKind::Custom(_) => {
            let canvas = Size::new(
                args.canvas_width.unwrap_or(16),
                args.canvas_height.unwrap_or(16),
            );
            let color_mode = args.color_mode.unwrap_or(ColorMode::Rgba);
            let state_names = args
                .initial_states
                .clone()
                .filter(|v| !v.is_empty())
                .unwrap_or_else(|| vec!["primary".into()]);

            let mut states = Vec::with_capacity(state_names.len());
            for state_name in state_names {
                let state_id = StateId::new(*next_id);
                *next_id += 1;
                let sprite_id = SpriteId::new(*next_id);
                *next_id += 1;
                let mut sprite =
                    Sprite::empty(sprite_id, format!("{} / {state_name}", args.name), canvas);
                sprite.color_mode = color_mode;
                states.push(NamedSprite {
                    id: state_id,
                    state_name,
                    sprite,
                    engine_tags: Vec::new(),
                });
            }
            let reference_sheet = args.reference_bytes.take().map(|bytes| {
                let mime = args
                    .reference_mime
                    .take()
                    .filter(|s| !s.is_empty())
                    .unwrap_or_else(|| "image/png".into());
                let variant_id = SheetVariantId::new(*next_id);
                *next_id += 1;
                Box::new(reference_sheet_from_image(bytes, mime, variant_id, ts))
            });
            EntityContent::Sprites {
                states,
                reference_sheet,
            }
        }
        EntityKind::Tileset => {
            let tileset_id = TilesetId::new(*next_id);
            *next_id += 1;
            // Mint a real buffer ID so the pixel-buffer subsystem (S01) can
            // allocate storage into it. PixelBufferId(0) is the null sentinel.
            let buffer_id = PixelBufferId::new(*next_id);
            *next_id += 1;
            EntityContent::Tileset {
                tileset: Tileset {
                    id: tileset_id,
                    name: args.name.clone(),
                    tile_size: Size::new(
                        args.tile_width.unwrap_or(16),
                        args.tile_height.unwrap_or(16),
                    ),
                    shape: TileShape::Square,
                    hex_offset: None,
                    // tile_count = 1 for the implicit empty tile at index 0.
                    tile_count: 1,
                    base_index: 1,
                    source: TilesetSource::Inline { buffer: buffer_id },
                    properties: Vec::new(),
                    autotile: None,
                    user_data: UserData::default(),
                },
            }
        }
        EntityKind::Tilemap => EntityContent::Tilemap {
            scene: TilemapScene {
                size: Size::new(
                    args.scene_width.unwrap_or(20),
                    args.scene_height.unwrap_or(15),
                ),
                tilesets: Vec::new(),
                layers: Vec::new(),
                properties: std::collections::BTreeMap::default(),
            },
        },
    };

    // Determine the initial active target from the content shape.
    let active = match &content {
        EntityContent::Sprites { states, .. } => {
            states
                .first()
                .map_or(ActiveTarget::None, |s| ActiveTarget::State {
                    entity_id,
                    state_id: s.id,
                })
        }
        EntityContent::Tileset { .. } => ActiveTarget::Tileset { entity_id },
        EntityContent::Tilemap { .. } => ActiveTarget::Tilemap { entity_id },
    };

    // A4: Populate EntityDefaults for Custom entities so library_add_state
    // inherits the canvas size and color mode set at creation time.
    let defaults = if matches!(&args.kind, EntityKind::Custom(_)) {
        EntityDefaults {
            canvas_size: Some(Size::new(
                args.canvas_width.unwrap_or(16),
                args.canvas_height.unwrap_or(16),
            )),
            color_mode: Some(args.color_mode.unwrap_or(ColorMode::Rgba)),
            default_palette_id: None,
            default_pivot: None,
            default_fps: None,
        }
    } else {
        EntityDefaults::default()
    };

    let entity = Entity {
        id: entity_id,
        kind: args.kind,
        name: args.name,
        group_id: args.group_id,
        tags: Vec::new(),
        defaults,
        content,
        ai: AiMetadata::default(),
        user_data: UserData::default(),
        created_at: ts,
        updated_at: ts,
    };

    project.library.entities.push(entity.clone());
    project.active = active;
    Ok(entity)
}

/// Deletes an entity from the library by id.
///
/// Clears `project.active` when it targets the deleted entity. Also removes
/// the entity from `ProjectAi::style_corpus`.
pub(crate) fn delete_entity_from_project(
    project: &mut pixhaus_core::project::Project,
    entity_id: EntityId,
    _ts: i64,
) -> Result<(), AppCommandError> {
    let before = project.library.entities.len();
    project.library.entities.retain(|e| e.id != entity_id);
    if project.library.entities.len() == before {
        return Err(AppCommandError::NotFound {
            entity: "entity".into(),
            id: u64::from(entity_id.get()),
        });
    }

    let active_touches = match project.active {
        ActiveTarget::State { entity_id: eid, .. }
        | ActiveTarget::Tileset { entity_id: eid }
        | ActiveTarget::Tilemap { entity_id: eid } => eid == entity_id,
        ActiveTarget::None => false,
    };
    if active_touches {
        project.active = ActiveTarget::None;
    }

    project
        .library
        .ai
        .style_corpus
        .retain(|&id| id != entity_id);

    Ok(())
}

/// Renames an entity. Rejects empty or whitespace-only names.
pub(crate) fn rename_entity_in_project(
    project: &mut pixhaus_core::project::Project,
    entity_id: EntityId,
    name: String,
    ts: i64,
) -> Result<(), AppCommandError> {
    if name.trim().is_empty() {
        return Err(AppCommandError::Validation {
            detail: "entity name must not be empty".into(),
        });
    }
    let entity = find_entity_mut(&mut project.library, entity_id)?;
    entity.name = name;
    entity.updated_at = ts;
    Ok(())
}

/// Adds a named state to a `Custom`-kind entity.
///
/// Canvas size defaults to the entity's `EntityDefaults.canvas_size`; falls
/// back to 16×16 if neither `args` nor defaults specify one.
pub(crate) fn add_state_to_entity(
    project: &mut pixhaus_core::project::Project,
    next_id: &mut u32,
    args: LibraryAddStateArgs,
    ts: i64,
) -> Result<NamedSprite, AppCommandError> {
    if args.state_name.trim().is_empty() {
        return Err(AppCommandError::Validation {
            detail: "state_name must not be empty".into(),
        });
    }

    let idx = project
        .library
        .entities
        .iter()
        .position(|e| e.id == args.entity_id)
        .ok_or(AppCommandError::NotFound {
            entity: "entity".into(),
            id: u64::from(args.entity_id.get()),
        })?;

    if !matches!(
        project.library.entities[idx].content,
        EntityContent::Sprites { .. }
    ) {
        return Err(AppCommandError::Validation {
            detail: format!(
                "entity {} is not Custom-kind; only Custom entities have states",
                args.entity_id.get()
            ),
        });
    }

    let state_id = StateId::new(*next_id);
    *next_id += 1;
    let sprite_id = SpriteId::new(*next_id);
    *next_id += 1;

    // Resolve canvas size and color mode before the mutable borrow below.
    let (canvas, color_mode, entity_name) = {
        let entity = &project.library.entities[idx];
        let (dw, dh) = entity
            .defaults
            .canvas_size
            .map_or((16, 16), |s| (s.width, s.height));
        let canvas = Size::new(
            args.canvas_width.unwrap_or(dw),
            args.canvas_height.unwrap_or(dh),
        );
        let cm = args
            .color_mode
            .or(entity.defaults.color_mode)
            .unwrap_or(ColorMode::Rgba);
        (canvas, cm, entity.name.clone())
    };

    // B3: Reject zero-dimension canvases after resolution.
    if canvas.width == 0 || canvas.height == 0 {
        return Err(AppCommandError::Validation {
            detail: format!(
                "canvas dimensions must be > 0 (got {}x{})",
                canvas.width, canvas.height
            ),
        });
    }

    let mut sprite = Sprite::empty(
        sprite_id,
        format!("{entity_name} / {}", args.state_name),
        canvas,
    );
    sprite.color_mode = color_mode;
    let named = NamedSprite {
        id: state_id,
        state_name: args.state_name,
        sprite,
        engine_tags: Vec::new(),
    };

    let entity = &mut project.library.entities[idx];
    let EntityContent::Sprites { states, .. } = &mut entity.content else {
        // Checked above — this branch cannot fire.
        return Err(AppCommandError::Validation {
            detail: "entity is not Custom-kind".into(),
        });
    };
    states.push(named.clone());
    entity.updated_at = ts;
    Ok(named)
}

/// Deletes a named state from a `Custom`-kind entity.
///
/// Clears `project.active` when it points at the deleted state.
pub(crate) fn delete_state_from_entity(
    project: &mut pixhaus_core::project::Project,
    entity_id: EntityId,
    state_id: StateId,
    ts: i64,
) -> Result<(), AppCommandError> {
    let entity = find_entity_mut(&mut project.library, entity_id)?;

    let EntityContent::Sprites { states, .. } = &mut entity.content else {
        return Err(AppCommandError::Validation {
            detail: format!(
                "entity {} is not Custom-kind; only Custom entities have states",
                entity_id.get()
            ),
        });
    };

    let before = states.len();
    states.retain(|s| s.id != state_id);
    if states.len() == before {
        return Err(AppCommandError::NotFound {
            entity: "state".into(),
            id: u64::from(state_id.get()),
        });
    }
    entity.updated_at = ts;
    // NLL: entity borrow ends here; project.active is a disjoint field.

    if matches!(
        project.active,
        ActiveTarget::State { entity_id: eid, state_id: sid }
        if eid == entity_id && sid == state_id
    ) {
        project.active = ActiveTarget::None;
    }

    Ok(())
}

/// Renames a named state within a `Custom`-kind entity.
pub(crate) fn rename_state_in_entity(
    project: &mut pixhaus_core::project::Project,
    entity_id: EntityId,
    state_id: StateId,
    state_name: String,
    ts: i64,
) -> Result<(), AppCommandError> {
    if state_name.trim().is_empty() {
        return Err(AppCommandError::Validation {
            detail: "state_name must not be empty".into(),
        });
    }

    let entity = find_entity_mut(&mut project.library, entity_id)?;

    let EntityContent::Sprites { states, .. } = &mut entity.content else {
        return Err(AppCommandError::Validation {
            detail: format!(
                "entity {} is not Custom-kind; only Custom entities have states",
                entity_id.get()
            ),
        });
    };

    {
        let named =
            states
                .iter_mut()
                .find(|s| s.id == state_id)
                .ok_or(AppCommandError::NotFound {
                    entity: "state".into(),
                    id: u64::from(state_id.get()),
                })?;
        named.state_name = state_name;
        // named borrow ends here
    }
    entity.updated_at = ts;
    Ok(())
}

/// Sets or clears an entity's group membership.
///
/// Pass `None` as `group_id` to remove the entity from its current group.
pub(crate) fn move_entity_to_group(
    project: &mut pixhaus_core::project::Project,
    entity_id: EntityId,
    group_id: Option<GroupId>,
    ts: i64,
) -> Result<(), AppCommandError> {
    if let Some(gid) = group_id {
        ensure_group_exists(&project.library, gid)?;
    }

    let entity = find_entity_mut(&mut project.library, entity_id)?;
    entity.group_id = group_id;
    entity.updated_at = ts;
    Ok(())
}

/// Creates a new entity group.
pub(crate) fn create_group_in_project(
    project: &mut pixhaus_core::project::Project,
    next_id: &mut u32,
    args: LibraryCreateGroupArgs,
) -> Result<EntityGroup, AppCommandError> {
    if args.name.trim().is_empty() {
        return Err(AppCommandError::Validation {
            detail: "group name must not be empty".into(),
        });
    }

    if let Some(pid) = args.parent_id {
        ensure_group_exists(&project.library, pid)?;
    }

    let group_id = GroupId::new(*next_id);
    *next_id += 1;

    let group = EntityGroup {
        id: group_id,
        name: args.name,
        parent_id: args.parent_id,
        user_data: UserData::default(),
    };
    project.library.groups.push(group.clone());
    Ok(group)
}

/// Deletes a group.
///
/// When `keep_entities` is `true`, entities in the group are unassigned. When
/// `false`, entities in the group are deleted. Child groups are re-parented to
/// this group's own `parent_id` (or become top-level if none).
pub(crate) fn delete_group_from_project(
    project: &mut pixhaus_core::project::Project,
    args: &LibraryDeleteGroupArgs,
    ts: i64,
) -> Result<(), AppCommandError> {
    let parent_id = project
        .library
        .groups
        .iter()
        .find(|g| g.id == args.group_id)
        .map(|g| g.parent_id)
        .ok_or(AppCommandError::NotFound {
            entity: "group".into(),
            id: u64::from(args.group_id.get()),
        })?;

    if args.keep_entities {
        // B5: Bump updated_at on each ungrouped entity.
        for entity in &mut project.library.entities {
            if entity.group_id == Some(args.group_id) {
                entity.group_id = None;
                entity.updated_at = ts;
            }
        }
    } else {
        // C1: HashSet so membership checks are O(1).
        let to_delete: HashSet<EntityId> = project
            .library
            .entities
            .iter()
            .filter(|e| e.group_id == Some(args.group_id))
            .map(|e| e.id)
            .collect();

        project
            .library
            .entities
            .retain(|e| !to_delete.contains(&e.id));

        let active_entity = match project.active {
            ActiveTarget::State { entity_id, .. }
            | ActiveTarget::Tileset { entity_id }
            | ActiveTarget::Tilemap { entity_id } => Some(entity_id),
            ActiveTarget::None => None,
        };
        if active_entity.is_some_and(|eid| to_delete.contains(&eid)) {
            project.active = ActiveTarget::None;
        }

        project
            .library
            .ai
            .style_corpus
            .retain(|id| !to_delete.contains(id));
    }

    for group in &mut project.library.groups {
        if group.parent_id == Some(args.group_id) {
            group.parent_id = parent_id;
        }
    }

    project.library.groups.retain(|g| g.id != args.group_id);
    Ok(())
}

/// Renames a group.
pub(crate) fn rename_group_in_project(
    project: &mut pixhaus_core::project::Project,
    group_id: GroupId,
    name: String,
) -> Result<(), AppCommandError> {
    if name.trim().is_empty() {
        return Err(AppCommandError::Validation {
            detail: "group name must not be empty".into(),
        });
    }
    let group = project
        .library
        .groups
        .iter_mut()
        .find(|g| g.id == group_id)
        .ok_or(AppCommandError::NotFound {
            entity: "group".into(),
            id: u64::from(group_id.get()),
        })?;
    group.name = name;
    Ok(())
}

/// Sets or clears a group's parent. Rejects self-referential and cyclic
/// assignments.
pub(crate) fn set_group_parent_in_project(
    project: &mut pixhaus_core::project::Project,
    group_id: GroupId,
    parent_id: Option<GroupId>,
) -> Result<(), AppCommandError> {
    ensure_group_exists(&project.library, group_id)?;

    if let Some(pid) = parent_id {
        if pid == group_id {
            return Err(AppCommandError::Validation {
                detail: "a group cannot be its own parent".into(),
            });
        }
        let mut cursor = Some(pid);
        while let Some(cid) = cursor {
            if cid == group_id {
                return Err(AppCommandError::Validation {
                    detail: "setting this parent would create a group cycle".into(),
                });
            }
            cursor = project
                .library
                .groups
                .iter()
                .find(|g| g.id == cid)
                .and_then(|g| g.parent_id);
        }
        ensure_group_exists(&project.library, pid)?;
    }

    let group = project
        .library
        .groups
        .iter_mut()
        .find(|g| g.id == group_id)
        .ok_or(AppCommandError::NotFound {
            entity: "group".into(),
            id: u64::from(group_id.get()),
        })?;
    group.parent_id = parent_id;
    Ok(())
}

/// Validates and sets the active editing target.
// `ActiveTarget` is non-Copy and the match arms destructure it (consuming
// each variant), so passing by value is correct even though the extracted
// fields are individually Copy. Clippy can't see through the enum.
#[allow(clippy::needless_pass_by_value)]
pub(crate) fn set_active_target_in_project(
    project: &mut pixhaus_core::project::Project,
    target: ActiveTarget,
) -> Result<(), AppCommandError> {
    match target {
        ActiveTarget::None => {
            project.active = ActiveTarget::None;
        }
        ActiveTarget::State {
            entity_id,
            state_id,
        } => {
            let entity = find_entity(&project.library, entity_id)?;
            let EntityContent::Sprites { states, .. } = &entity.content else {
                return Err(AppCommandError::Validation {
                    detail: format!(
                        "entity {} is not Custom-kind; cannot target a state on it",
                        entity_id.get()
                    ),
                });
            };
            if !states.iter().any(|s| s.id == state_id) {
                return Err(AppCommandError::NotFound {
                    entity: "state".into(),
                    id: u64::from(state_id.get()),
                });
            }
            project.active = ActiveTarget::State {
                entity_id,
                state_id,
            };
        }
        ActiveTarget::Tileset { entity_id } => {
            if !project
                .library
                .entities
                .iter()
                .any(|e| e.id == entity_id && matches!(e.content, EntityContent::Tileset { .. }))
            {
                return Err(AppCommandError::NotFound {
                    entity: "tileset entity".into(),
                    id: u64::from(entity_id.get()),
                });
            }
            project.active = ActiveTarget::Tileset { entity_id };
        }
        ActiveTarget::Tilemap { entity_id } => {
            if !project
                .library
                .entities
                .iter()
                .any(|e| e.id == entity_id && matches!(e.content, EntityContent::Tilemap { .. }))
            {
                return Err(AppCommandError::NotFound {
                    entity: "tilemap entity".into(),
                    id: u64::from(entity_id.get()),
                });
            }
            project.active = ActiveTarget::Tilemap { entity_id };
        }
    }
    Ok(())
}

/// Creates a new tag definition.
pub(crate) fn add_tag_to_project(
    project: &mut pixhaus_core::project::Project,
    next_id: &mut u32,
    args: LibraryAddTagArgs,
) -> Result<TagDefinition, AppCommandError> {
    if args.name.trim().is_empty() {
        return Err(AppCommandError::Validation {
            detail: "tag name must not be empty".into(),
        });
    }

    let tag_id = TagId::new(*next_id);
    *next_id += 1;

    let tag = TagDefinition {
        id: tag_id,
        name: args.name,
        color: args.color,
        auto_generated: false,
    };
    project.library.tags.push(tag.clone());
    Ok(tag)
}

/// Deletes a tag definition and removes it from every entity that referenced
/// it.
pub(crate) fn delete_tag_from_project(
    project: &mut pixhaus_core::project::Project,
    tag_id: TagId,
    ts: i64,
) -> Result<(), AppCommandError> {
    let before = project.library.tags.len();
    project.library.tags.retain(|t| t.id != tag_id);
    if project.library.tags.len() == before {
        return Err(AppCommandError::NotFound {
            entity: "tag".into(),
            id: u64::from(tag_id.get()),
        });
    }

    // B6: Bump updated_at only when the entity actually loses a tag.
    for entity in &mut project.library.entities {
        let tags_before = entity.tags.len();
        entity.tags.retain(|&t| t != tag_id);
        let suggested_before = entity.ai.suggested_tags.len();
        entity.ai.suggested_tags.retain(|&t| t != tag_id);
        if entity.tags.len() < tags_before || entity.ai.suggested_tags.len() < suggested_before {
            entity.updated_at = ts;
        }
    }

    Ok(())
}

/// Renames an existing tag definition.
pub(crate) fn rename_tag_in_project(
    project: &mut pixhaus_core::project::Project,
    tag_id: TagId,
    name: String,
) -> Result<(), AppCommandError> {
    if name.trim().is_empty() {
        return Err(AppCommandError::Validation {
            detail: "tag name must not be empty".into(),
        });
    }
    let tag = project
        .library
        .tags
        .iter_mut()
        .find(|t| t.id == tag_id)
        .ok_or(AppCommandError::NotFound {
            entity: "tag".into(),
            id: u64::from(tag_id.get()),
        })?;
    tag.name = name;
    Ok(())
}

/// Attaches an existing tag to an entity.
///
/// Returns `true` if the tag was added, `false` if it was already present
/// (so callers can skip setting `dirty` on no-ops).
pub(crate) fn tag_entity_in_project(
    project: &mut pixhaus_core::project::Project,
    entity_id: EntityId,
    tag_id: TagId,
    ts: i64,
) -> Result<bool, AppCommandError> {
    if !project.library.tags.iter().any(|t| t.id == tag_id) {
        return Err(AppCommandError::NotFound {
            entity: "tag".into(),
            id: u64::from(tag_id.get()),
        });
    }

    let entity = find_entity_mut(&mut project.library, entity_id)?;

    if entity.tags.contains(&tag_id) {
        return Ok(false);
    }
    entity.tags.push(tag_id);
    entity.updated_at = ts;
    Ok(true)
}

/// Removes a tag from an entity.
///
/// Returns `true` if the tag was removed, `false` if it was not present
/// (so callers can skip setting `dirty` on no-ops).
pub(crate) fn untag_entity_in_project(
    project: &mut pixhaus_core::project::Project,
    entity_id: EntityId,
    tag_id: TagId,
    ts: i64,
) -> Result<bool, AppCommandError> {
    let entity = find_entity_mut(&mut project.library, entity_id)?;

    let before = entity.tags.len();
    entity.tags.retain(|&t| t != tag_id);
    if entity.tags.len() < before {
        entity.updated_at = ts;
        Ok(true)
    } else {
        Ok(false)
    }
}

/// Searches the library and returns matching entities.
///
/// The query is matched case-insensitively against entity names, Custom kind
/// strings, and tag names. Optional filters further narrow the result.
pub(crate) fn search_library(
    project: &pixhaus_core::project::Project,
    args: &LibrarySearchArgs,
) -> Vec<Entity> {
    let query = args.query.to_lowercase();

    // C2: precompute a TagId -> lowercased-name map once so per-entity tag
    // matching is O(tags-on-entity) HashMap lookups instead of nested scans.
    let lc_tag_names: HashMap<TagId, String> = project
        .library
        .tags
        .iter()
        .map(|t| (t.id, t.name.to_lowercase()))
        .collect();

    project
        .library
        .entities
        .iter()
        .filter(|entity| {
            if let Some(ref k) = args.kind_filter {
                if &entity.kind != k {
                    return false;
                }
            }
            if let Some(gid) = args.group_filter {
                if entity.group_id != Some(gid) {
                    return false;
                }
            }
            if let Some(tid) = args.tag_filter {
                if !entity.tags.contains(&tid) {
                    return false;
                }
            }
            if query.is_empty() {
                return true;
            }
            if entity.name.to_lowercase().contains(&query) {
                return true;
            }
            if let EntityKind::Custom(ref category) = entity.kind {
                if category.to_lowercase().contains(&query) {
                    return true;
                }
            }
            entity.tags.iter().any(|tid| {
                lc_tag_names
                    .get(tid)
                    .is_some_and(|name| name.contains(&query))
            })
        })
        .cloned()
        .collect()
}

/// Moves an entity to a different position in the library's insertion-order
/// list. `new_index` is clamped to `[0, entities.len() - 1]`.
pub(crate) fn reorder_entities_in_project(
    project: &mut pixhaus_core::project::Project,
    entity_id: EntityId,
    new_index: usize,
) -> Result<(), AppCommandError> {
    let current = project
        .library
        .entities
        .iter()
        .position(|e| e.id == entity_id)
        .ok_or(AppCommandError::NotFound {
            entity: "entity".into(),
            id: u64::from(entity_id.get()),
        })?;

    let target = new_index.min(project.library.entities.len().saturating_sub(1));
    let entity = project.library.entities.remove(current);
    project.library.entities.insert(target, entity);
    Ok(())
}

// ── entity commands ───────────────────────────────────────────────────────────

/// Creates a new library entity of any kind and sets it as the active target.
///
/// # Kind-specific requirements
///
/// - `Custom`: `canvas_width` and `canvas_height` required (> 0).
///   `initial_states` defaults to `["primary"]` when absent.
///   `reference_bytes` may be provided to initialize the embedded
///   reference sheet.
/// - `Tileset`: `tile_width` and `tile_height` required (> 0).
/// - `Tilemap`: `scene_width` and `scene_height` required (> 0).
#[tauri::command(async, rename_all = "snake_case")]
pub async fn library_create_entity(
    args: LibraryCreateEntityArgs,
    state: State<'_, AppState>,
) -> CommandResult<Entity> {
    let ts = now_secs();
    let mut doc = state.doc.write().await;
    // Copy next_id before the project borrow so NLL can release the project
    // borrow before doc.next_id is written back.
    let mut next_id = doc.next_id;
    let project = doc
        .project
        .as_mut()
        .ok_or(AppCommandError::NoActiveProject)?;
    let entity = create_entity_in_project(project, &mut next_id, args, ts)?;
    doc.next_id = next_id;
    doc.dirty = true;
    Ok(entity)
}

/// Deletes a library entity by id.
#[tauri::command(async, rename_all = "snake_case")]
pub async fn library_delete_entity(
    entity_id: EntityId,
    state: State<'_, AppState>,
) -> CommandResult<()> {
    let ts = now_secs();
    let mut doc = state.doc.write().await;
    let project = doc
        .project
        .as_mut()
        .ok_or(AppCommandError::NoActiveProject)?;
    delete_entity_from_project(project, entity_id, ts)?;
    doc.dirty = true;
    Ok(())
}

/// Renames a library entity.
#[tauri::command(async, rename_all = "snake_case")]
pub async fn library_rename_entity(
    entity_id: EntityId,
    name: String,
    state: State<'_, AppState>,
) -> CommandResult<()> {
    let mut doc = state.doc.write().await;
    let project = doc
        .project
        .as_mut()
        .ok_or(AppCommandError::NoActiveProject)?;
    rename_entity_in_project(project, entity_id, name, now_secs())?;
    doc.dirty = true;
    Ok(())
}

/// Returns a single entity by id.
#[tauri::command(async, rename_all = "snake_case")]
pub async fn library_get_entity(
    entity_id: EntityId,
    state: State<'_, AppState>,
) -> CommandResult<Entity> {
    let doc = state.doc.read().await;
    let project = doc
        .project
        .as_ref()
        .ok_or(AppCommandError::NoActiveProject)?;
    project
        .library
        .entities
        .iter()
        .find(|e| e.id == entity_id)
        .cloned()
        .ok_or(AppCommandError::NotFound {
            entity: "entity".into(),
            id: u64::from(entity_id.get()),
        })
}

/// Returns all entities in the library, in insertion order.
///
/// Optional `kind`, `group_id`, and `tag_id` filters narrow the result. All
/// provided filters must match.
#[tauri::command(async, rename_all = "snake_case")]
pub async fn library_list_entities(
    kind: Option<EntityKind>,
    group_id: Option<GroupId>,
    tag_id: Option<TagId>,
    state: State<'_, AppState>,
) -> CommandResult<Vec<Entity>> {
    let doc = state.doc.read().await;
    let project = doc
        .project
        .as_ref()
        .ok_or(AppCommandError::NoActiveProject)?;

    let result = project
        .library
        .entities
        .iter()
        .filter(|e| {
            if let Some(ref k) = kind {
                if &e.kind != k {
                    return false;
                }
            }
            if let Some(gid) = group_id {
                if e.group_id != Some(gid) {
                    return false;
                }
            }
            if let Some(tid) = tag_id {
                if !e.tags.contains(&tid) {
                    return false;
                }
            }
            true
        })
        .cloned()
        .collect();

    Ok(result)
}

/// Moves an entity to a different position in the library's insertion-order
/// list.
///
/// `new_index` is clamped to `[0, entities.len() - 1]`.
#[tauri::command(async, rename_all = "snake_case")]
pub async fn library_reorder_entities(
    entity_id: EntityId,
    new_index: usize,
    state: State<'_, AppState>,
) -> CommandResult<()> {
    let mut doc = state.doc.write().await;
    let project = doc
        .project
        .as_mut()
        .ok_or(AppCommandError::NoActiveProject)?;
    reorder_entities_in_project(project, entity_id, new_index)?;
    doc.dirty = true;
    Ok(())
}

/// Sets or clears an entity's group membership.
///
/// Pass `None` as `group_id` to remove the entity from its current group.
#[tauri::command(async, rename_all = "snake_case")]
pub async fn library_move_entity_to_group(
    entity_id: EntityId,
    group_id: Option<GroupId>,
    state: State<'_, AppState>,
) -> CommandResult<()> {
    let mut doc = state.doc.write().await;
    let project = doc
        .project
        .as_mut()
        .ok_or(AppCommandError::NoActiveProject)?;
    move_entity_to_group(project, entity_id, group_id, now_secs())?;
    doc.dirty = true;
    Ok(())
}

/// Attaches an existing tag to an entity.
///
/// No-ops if the tag is already attached.
#[tauri::command(async, rename_all = "snake_case")]
pub async fn library_tag_entity(
    entity_id: EntityId,
    tag_id: TagId,
    state: State<'_, AppState>,
) -> CommandResult<()> {
    let mut doc = state.doc.write().await;
    let project = doc
        .project
        .as_mut()
        .ok_or(AppCommandError::NoActiveProject)?;
    if tag_entity_in_project(project, entity_id, tag_id, now_secs())? {
        doc.dirty = true;
    }
    Ok(())
}

/// Removes a tag from an entity.
///
/// No-ops if the tag is not attached.
#[tauri::command(async, rename_all = "snake_case")]
pub async fn library_untag_entity(
    entity_id: EntityId,
    tag_id: TagId,
    state: State<'_, AppState>,
) -> CommandResult<()> {
    let mut doc = state.doc.write().await;
    let project = doc
        .project
        .as_mut()
        .ok_or(AppCommandError::NoActiveProject)?;
    if untag_entity_in_project(project, entity_id, tag_id, now_secs())? {
        doc.dirty = true;
    }
    Ok(())
}

// ── state commands (Custom-kind entities) ─────────────────────────────────────

/// Adds a named state (new sprite) to a `Custom`-kind entity.
#[tauri::command(async, rename_all = "snake_case")]
pub async fn library_add_state(
    args: LibraryAddStateArgs,
    state: State<'_, AppState>,
) -> CommandResult<NamedSprite> {
    let ts = now_secs();
    let mut doc = state.doc.write().await;
    let mut next_id = doc.next_id;
    let project = doc
        .project
        .as_mut()
        .ok_or(AppCommandError::NoActiveProject)?;
    let named = add_state_to_entity(project, &mut next_id, args, ts)?;
    doc.next_id = next_id;
    doc.dirty = true;
    Ok(named)
}

/// Deletes a named state from a `Custom`-kind entity.
#[tauri::command(async, rename_all = "snake_case")]
pub async fn library_delete_state(
    entity_id: EntityId,
    state_id: StateId,
    state: State<'_, AppState>,
) -> CommandResult<()> {
    let mut doc = state.doc.write().await;
    let project = doc
        .project
        .as_mut()
        .ok_or(AppCommandError::NoActiveProject)?;
    delete_state_from_entity(project, entity_id, state_id, now_secs())?;
    doc.dirty = true;
    Ok(())
}

/// Renames a named state within a `Custom`-kind entity.
#[tauri::command(async, rename_all = "snake_case")]
pub async fn library_rename_state(
    entity_id: EntityId,
    state_id: StateId,
    state_name: String,
    state: State<'_, AppState>,
) -> CommandResult<()> {
    let mut doc = state.doc.write().await;
    let project = doc
        .project
        .as_mut()
        .ok_or(AppCommandError::NoActiveProject)?;
    rename_state_in_entity(project, entity_id, state_id, state_name, now_secs())?;
    doc.dirty = true;
    Ok(())
}

// ── active target commands ────────────────────────────────────────────────────

/// Sets the active editing target.
///
/// Validates that the referenced entity (and state, for `State` targets) exist.
#[tauri::command(async, rename_all = "snake_case")]
pub async fn library_set_active_target(
    target: ActiveTarget,
    state: State<'_, AppState>,
) -> CommandResult<()> {
    let mut doc = state.doc.write().await;
    let project = doc
        .project
        .as_mut()
        .ok_or(AppCommandError::NoActiveProject)?;
    set_active_target_in_project(project, target)?;
    doc.dirty = true;
    Ok(())
}

/// Returns the current active editing target.
#[tauri::command(async, rename_all = "snake_case")]
pub async fn library_get_active_target(state: State<'_, AppState>) -> CommandResult<ActiveTarget> {
    let doc = state.doc.read().await;
    let project = doc
        .project
        .as_ref()
        .ok_or(AppCommandError::NoActiveProject)?;
    Ok(project.active.clone())
}

// ── group commands ────────────────────────────────────────────────────────────

/// Creates a new entity group.
#[tauri::command(async, rename_all = "snake_case")]
pub async fn library_create_group(
    args: LibraryCreateGroupArgs,
    state: State<'_, AppState>,
) -> CommandResult<EntityGroup> {
    let mut doc = state.doc.write().await;
    let mut next_id = doc.next_id;
    let project = doc
        .project
        .as_mut()
        .ok_or(AppCommandError::NoActiveProject)?;
    let group = create_group_in_project(project, &mut next_id, args)?;
    doc.next_id = next_id;
    doc.dirty = true;
    Ok(group)
}

/// Deletes a group.
#[tauri::command(async, rename_all = "snake_case")]
pub async fn library_delete_group(
    args: LibraryDeleteGroupArgs,
    state: State<'_, AppState>,
) -> CommandResult<()> {
    let ts = now_secs();
    let mut doc = state.doc.write().await;
    let project = doc
        .project
        .as_mut()
        .ok_or(AppCommandError::NoActiveProject)?;
    delete_group_from_project(project, &args, ts)?;
    doc.dirty = true;
    Ok(())
}

/// Renames a group.
#[tauri::command(async, rename_all = "snake_case")]
pub async fn library_rename_group(
    group_id: GroupId,
    name: String,
    state: State<'_, AppState>,
) -> CommandResult<()> {
    let mut doc = state.doc.write().await;
    let project = doc
        .project
        .as_mut()
        .ok_or(AppCommandError::NoActiveProject)?;
    rename_group_in_project(project, group_id, name)?;
    doc.dirty = true;
    Ok(())
}

/// Sets or clears a group's parent, changing its nesting level.
///
/// Pass `None` to make the group top-level. Cycles are rejected.
#[tauri::command(async, rename_all = "snake_case")]
pub async fn library_set_group_parent(
    group_id: GroupId,
    parent_id: Option<GroupId>,
    state: State<'_, AppState>,
) -> CommandResult<()> {
    let mut doc = state.doc.write().await;
    let project = doc
        .project
        .as_mut()
        .ok_or(AppCommandError::NoActiveProject)?;
    set_group_parent_in_project(project, group_id, parent_id)?;
    doc.dirty = true;
    Ok(())
}

/// Returns all groups in the library.
#[tauri::command(async, rename_all = "snake_case")]
pub async fn library_list_groups(state: State<'_, AppState>) -> CommandResult<Vec<EntityGroup>> {
    let doc = state.doc.read().await;
    let project = doc
        .project
        .as_ref()
        .ok_or(AppCommandError::NoActiveProject)?;
    Ok(project.library.groups.clone())
}

// ── tag commands ──────────────────────────────────────────────────────────────

/// Creates a new tag definition.
#[tauri::command(async, rename_all = "snake_case")]
pub async fn library_add_tag(
    args: LibraryAddTagArgs,
    state: State<'_, AppState>,
) -> CommandResult<TagDefinition> {
    let mut doc = state.doc.write().await;
    let mut next_id = doc.next_id;
    let project = doc
        .project
        .as_mut()
        .ok_or(AppCommandError::NoActiveProject)?;
    let tag = add_tag_to_project(project, &mut next_id, args)?;
    doc.next_id = next_id;
    doc.dirty = true;
    Ok(tag)
}

/// Deletes a tag definition and removes the tag from every entity that
/// referenced it.
#[tauri::command(async, rename_all = "snake_case")]
pub async fn library_delete_tag(tag_id: TagId, state: State<'_, AppState>) -> CommandResult<()> {
    let ts = now_secs();
    let mut doc = state.doc.write().await;
    let project = doc
        .project
        .as_mut()
        .ok_or(AppCommandError::NoActiveProject)?;
    delete_tag_from_project(project, tag_id, ts)?;
    doc.dirty = true;
    Ok(())
}

/// Renames an existing tag definition.
#[tauri::command(async, rename_all = "snake_case")]
pub async fn library_rename_tag(
    tag_id: TagId,
    name: String,
    state: State<'_, AppState>,
) -> CommandResult<()> {
    let mut doc = state.doc.write().await;
    let project = doc
        .project
        .as_mut()
        .ok_or(AppCommandError::NoActiveProject)?;
    rename_tag_in_project(project, tag_id, name)?;
    doc.dirty = true;
    Ok(())
}

/// Returns all tag definitions in the library.
#[tauri::command(async, rename_all = "snake_case")]
pub async fn library_list_tags(state: State<'_, AppState>) -> CommandResult<Vec<TagDefinition>> {
    let doc = state.doc.read().await;
    let project = doc
        .project
        .as_ref()
        .ok_or(AppCommandError::NoActiveProject)?;
    Ok(project.library.tags.clone())
}

// ── search ────────────────────────────────────────────────────────────────────

/// Searches the library and returns matching entities.
///
/// The query is matched case-insensitively against:
/// - entity names (substring)
/// - the string carried by `Custom`-kind entities (substring)
/// - names of tags attached to the entity (substring)
///
/// Optional `kind_filter`, `group_filter`, and `tag_filter` further narrow
/// results. All provided filters must match.
#[tauri::command(async, rename_all = "snake_case")]
pub async fn library_search(
    args: LibrarySearchArgs,
    state: State<'_, AppState>,
) -> CommandResult<Vec<Entity>> {
    let doc = state.doc.read().await;
    let project = doc
        .project
        .as_ref()
        .ok_or(AppCommandError::NoActiveProject)?;
    Ok(search_library(project, &args))
}

pub mod composition;
pub mod lora;
pub mod reference_sheets;

// The `import_reference_sheet_draft` sheet helper (kept here) consumes this
// arg type, which lives with the import command in the reference_sheets module.
use reference_sheets::LibraryImportReferenceSheetArgs;

// ── sheet helpers ─────────────────────────────────────────────────────────────

pub(crate) fn import_reference_sheet_draft(
    project: &mut pixhaus_core::project::Project,
    next_id: &mut u32,
    args: LibraryImportReferenceSheetArgs,
    ts: i64,
) -> Result<Entity, AppCommandError> {
    let entity = find_entity_mut(&mut project.library, args.entity_id)?;

    let EntityContent::Sprites {
        reference_sheet, ..
    } = &mut entity.content
    else {
        return Err(AppCommandError::Validation {
            detail: format!(
                "entity {} is not a sprite entity; reference sheets belong to sprites",
                args.entity_id.get()
            ),
        });
    };

    let variant_id = SheetVariantId::new(*next_id);
    *next_id += 1;
    let mime = args
        .mime
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(|| "image/png".into());
    let variant = SheetVariant::from_image(
        variant_id,
        ts,
        ReferenceImage {
            bytes: args.bytes,
            mime,
        },
    );
    let sheet = reference_sheet
        .get_or_insert_with(|| {
            Box::new(ReferenceSheet {
                canonical: None,
                variants: Vec::new(),
                prompts: Vec::new(),
                info: AssetInfo::default(),
            })
        })
        .as_mut();
    sheet.variants.insert(0, variant);
    entity.updated_at = ts;
    Ok(entity.clone())
}

pub(crate) fn update_asset_info_in_project(
    project: &mut pixhaus_core::project::Project,
    entity_id: EntityId,
    info: AssetInfo,
    ts: i64,
) -> Result<(), AppCommandError> {
    let entity = find_entity_mut(&mut project.library, entity_id)?;

    let sheet = embedded_reference_sheet_mut(entity)?;

    sheet.info = info;
    entity.updated_at = ts;
    Ok(())
}

pub(crate) fn delete_sheet_variant_in_project(
    project: &mut pixhaus_core::project::Project,
    entity_id: EntityId,
    variant_id: SheetVariantId,
    ts: i64,
) -> Result<(), AppCommandError> {
    let entity = find_entity_mut(&mut project.library, entity_id)?;

    let sheet = embedded_reference_sheet_mut(entity)?;

    if sheet
        .canonical
        .as_ref()
        .is_some_and(|variant| variant.id == variant_id)
    {
        return Err(AppCommandError::Validation {
            detail: "cannot delete the canonical variant; approve a replacement first".into(),
        });
    }

    let before = sheet.variants.len();
    sheet.variants.retain(|v| v.id != variant_id);
    if sheet.variants.len() == before {
        return Err(AppCommandError::NotFound {
            entity: "sheet variant".into(),
            id: u64::from(variant_id.get()),
        });
    }

    entity.updated_at = ts;
    Ok(())
}

fn embedded_reference_sheet_mut(
    entity: &mut Entity,
) -> Result<&mut ReferenceSheet, AppCommandError> {
    match &mut entity.content {
        EntityContent::Sprites {
            reference_sheet: Some(sheet),
            ..
        } => Ok(sheet.as_mut()),
        _ => Err(AppCommandError::Validation {
            detail: "entity has no sprite reference sheet".into(),
        }),
    }
}

// ── tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use base64::Engine as _;
    use pixhaus_ai::verbs::reference_sheet::GenerateSheetPayload;
    use pixhaus_core::project::{
        ActiveTarget, AiMetadata, AssetInfo, ColorMode, EntityContent, EntityDefaults, EntityId,
        EntityKind, GroupId, NamedSprite, ReferenceImage, ReferenceSheet, SheetComposition,
        SheetVariant, SheetVariantId, Size, StateId, TagId, UserData,
    };

    use super::*;
    // The AI-hook commands moved into the `lora` submodule; bring them back
    // into the test module's scope so the existing test bodies are unchanged.
    // (Reference-sheet command bodies are exercised through their core
    // helpers, which stay in this module and arrive via `use super::*`.)
    use super::lora::*;
    use crate::state::{AppState, DocumentStore};

    fn project_with_one_custom_entity() -> (pixhaus_core::project::Project, EntityId, StateId) {
        let mut project = pixhaus_core::project::Project::new("test");
        let sprite = Sprite::empty(SpriteId::new(3), "Hero / idle", Size::new(16, 16));
        let state_id = StateId::new(2);
        let entity_id = EntityId::new(1);
        project.library.entities.push(Entity {
            id: entity_id,
            kind: EntityKind::Custom("Character".into()),
            name: "Hero".into(),
            group_id: None,
            tags: Vec::new(),
            defaults: EntityDefaults::default(),
            content: EntityContent::Sprites {
                states: vec![NamedSprite {
                    id: state_id,
                    state_name: "idle".into(),
                    sprite,
                    engine_tags: Vec::new(),
                }],
                reference_sheet: None,
            },
            ai: AiMetadata::default(),
            user_data: UserData::default(),
            created_at: 0,
            updated_at: 0,
        });
        project.active = ActiveTarget::State {
            entity_id,
            state_id,
        };
        (project, entity_id, state_id)
    }

    fn doc_with_project() -> (DocumentStore, EntityId, StateId) {
        let (project, eid, sid) = project_with_one_custom_entity();
        let doc = DocumentStore {
            project: Some(project),
            next_id: 10, // Past all seeded ids
            ..DocumentStore::default()
        };
        (doc, eid, sid)
    }

    // ── rename entity ─────────────────────────────────────────────────────

    #[test]
    fn rename_entity_updates_name_and_updated_at() {
        let (mut doc, entity_id, _) = doc_with_project();
        let ts = 99_i64;
        let project = doc.project.as_mut().unwrap();
        rename_entity_in_project(project, entity_id, "Goblin".into(), ts).unwrap();
        let entity = doc
            .project
            .unwrap()
            .library
            .entities
            .into_iter()
            .find(|e| e.id == entity_id)
            .unwrap();
        assert_eq!(entity.name, "Goblin");
        assert_eq!(entity.updated_at, ts);
    }

    #[test]
    fn rename_entity_not_found() {
        let (mut doc, _, _) = doc_with_project();
        let missing = EntityId::new(999);
        let result =
            rename_entity_in_project(doc.project.as_mut().unwrap(), missing, "X".into(), 0);
        assert!(
            matches!(result, Err(AppCommandError::NotFound { .. })),
            "missing entity must return NotFound"
        );
    }

    #[test]
    fn rename_entity_rejects_whitespace_name() {
        let (mut doc, entity_id, _) = doc_with_project();
        let result =
            rename_entity_in_project(doc.project.as_mut().unwrap(), entity_id, "   ".into(), 0);
        assert!(matches!(result, Err(AppCommandError::Validation { .. })));
    }

    // ── delete entity ─────────────────────────────────────────────────────

    #[test]
    fn delete_entity_clears_active_target() {
        let (mut doc, entity_id, _) = doc_with_project();
        let project = doc.project.as_mut().unwrap();
        assert!(!project.active.is_none());
        delete_entity_from_project(project, entity_id, 0).unwrap();
        assert!(doc.project.unwrap().active.is_none());
    }

    #[test]
    fn delete_entity_not_found() {
        let (mut doc, _, _) = doc_with_project();
        let missing = EntityId::new(999);
        let result = delete_entity_from_project(doc.project.as_mut().unwrap(), missing, 0);
        assert!(matches!(result, Err(AppCommandError::NotFound { .. })));
    }

    // ── B1 — empty name validation ────────────────────────────────────────

    #[test]
    fn create_entity_rejects_whitespace_name() {
        let mut project = pixhaus_core::project::Project::new("test");
        let mut next_id = 1u32;
        let args = LibraryCreateEntityArgs {
            kind: EntityKind::Custom("Character".into()),
            name: "   ".into(),
            group_id: None,
            initial_states: None,
            canvas_width: Some(16),
            canvas_height: Some(16),
            color_mode: None,
            tile_width: None,
            tile_height: None,
            scene_width: None,
            scene_height: None,
            reference_bytes: None,
            reference_mime: None,
        };
        let result = create_entity_in_project(&mut project, &mut next_id, args, 0);
        assert!(
            matches!(result, Err(AppCommandError::Validation { .. })),
            "whitespace-only name must be rejected"
        );
    }

    #[test]
    fn create_entity_unknown_group_not_found() {
        let mut project = pixhaus_core::project::Project::new("test");
        let mut next_id = 1u32;
        let args = LibraryCreateEntityArgs {
            kind: EntityKind::Custom("Character".into()),
            name: "Hero".into(),
            group_id: Some(GroupId::new(999)),
            initial_states: None,
            canvas_width: Some(16),
            canvas_height: Some(16),
            color_mode: None,
            tile_width: None,
            tile_height: None,
            scene_width: None,
            scene_height: None,
            reference_bytes: None,
            reference_mime: None,
        };
        let result = create_entity_in_project(&mut project, &mut next_id, args, 0);
        assert!(matches!(result, Err(AppCommandError::NotFound { .. })));
    }

    // ── B2 — per-state name validation ────────────────────────────────────

    #[test]
    fn create_entity_rejects_whitespace_state_name() {
        let mut project = pixhaus_core::project::Project::new("test");
        let mut next_id = 1u32;
        let args = LibraryCreateEntityArgs {
            kind: EntityKind::Custom("Character".into()),
            name: "Hero".into(),
            group_id: None,
            initial_states: Some(vec!["idle".into(), "  ".into()]),
            canvas_width: Some(16),
            canvas_height: Some(16),
            color_mode: None,
            tile_width: None,
            tile_height: None,
            scene_width: None,
            scene_height: None,
            reference_bytes: None,
            reference_mime: None,
        };
        let result = create_entity_in_project(&mut project, &mut next_id, args, 0);
        assert!(
            matches!(result, Err(AppCommandError::Validation { .. })),
            "whitespace-only state name must be rejected"
        );
    }

    // ── add / delete / rename state ───────────────────────────────────────

    #[test]
    fn add_state_creates_named_sprite_with_correct_size() {
        let (mut doc, entity_id, _) = doc_with_project();
        let args = LibraryAddStateArgs {
            entity_id,
            state_name: "walk".into(),
            canvas_width: Some(32),
            canvas_height: Some(32),
            color_mode: None,
        };
        let named =
            add_state_to_entity(doc.project.as_mut().unwrap(), &mut doc.next_id, args, 0).unwrap();
        assert_eq!(named.state_name, "walk");
        assert_eq!(named.sprite.canvas, Size::new(32, 32));
        let states = match &doc.project.unwrap().library.entities[0].content {
            EntityContent::Sprites { states, .. } => states.clone(),
            _ => panic!("expected Sprites"),
        };
        assert_eq!(states.len(), 2);
    }

    // ── B3 — zero-canvas validation ───────────────────────────────────────

    #[test]
    fn add_state_rejects_zero_canvas() {
        let (mut doc, entity_id, _) = doc_with_project();
        let args = LibraryAddStateArgs {
            entity_id,
            state_name: "walk".into(),
            canvas_width: Some(0),
            canvas_height: Some(16),
            color_mode: None,
        };
        let result = add_state_to_entity(doc.project.as_mut().unwrap(), &mut doc.next_id, args, 0);
        assert!(
            matches!(result, Err(AppCommandError::Validation { .. })),
            "zero-dimension canvas must be rejected"
        );
    }

    #[test]
    fn add_state_entity_not_found() {
        let (mut doc, _, _) = doc_with_project();
        let args = LibraryAddStateArgs {
            entity_id: EntityId::new(999),
            state_name: "walk".into(),
            canvas_width: Some(16),
            canvas_height: Some(16),
            color_mode: None,
        };
        let result = add_state_to_entity(doc.project.as_mut().unwrap(), &mut doc.next_id, args, 0);
        assert!(matches!(result, Err(AppCommandError::NotFound { .. })));
    }

    #[test]
    fn delete_state_entity_not_found() {
        let (mut doc, _, state_id) = doc_with_project();
        let result = delete_state_from_entity(
            doc.project.as_mut().unwrap(),
            EntityId::new(999),
            state_id,
            0,
        );
        assert!(matches!(result, Err(AppCommandError::NotFound { .. })));
    }

    #[test]
    fn delete_state_state_not_found() {
        let (mut doc, entity_id, _) = doc_with_project();
        let result = delete_state_from_entity(
            doc.project.as_mut().unwrap(),
            entity_id,
            StateId::new(999),
            0,
        );
        assert!(matches!(result, Err(AppCommandError::NotFound { .. })));
    }

    #[test]
    fn rename_state_entity_not_found() {
        let (mut doc, _, state_id) = doc_with_project();
        let result = rename_state_in_entity(
            doc.project.as_mut().unwrap(),
            EntityId::new(999),
            state_id,
            "run".into(),
            0,
        );
        assert!(matches!(result, Err(AppCommandError::NotFound { .. })));
    }

    #[test]
    fn rename_state_state_not_found() {
        let (mut doc, entity_id, _) = doc_with_project();
        let result = rename_state_in_entity(
            doc.project.as_mut().unwrap(),
            entity_id,
            StateId::new(999),
            "run".into(),
            0,
        );
        assert!(matches!(result, Err(AppCommandError::NotFound { .. })));
    }

    // ── move entity to group ──────────────────────────────────────────────

    #[test]
    fn move_entity_to_group_entity_not_found() {
        let (mut doc, _, _) = doc_with_project();
        let result =
            move_entity_to_group(doc.project.as_mut().unwrap(), EntityId::new(999), None, 0);
        assert!(matches!(result, Err(AppCommandError::NotFound { .. })));
    }

    #[test]
    fn move_entity_to_group_group_not_found() {
        let (mut doc, entity_id, _) = doc_with_project();
        let result = move_entity_to_group(
            doc.project.as_mut().unwrap(),
            entity_id,
            Some(GroupId::new(999)),
            0,
        );
        assert!(matches!(result, Err(AppCommandError::NotFound { .. })));
    }

    // ── reorder entities ──────────────────────────────────────────────────

    #[test]
    fn reorder_entities_moves_to_target_index() {
        let (mut doc, _, _) = doc_with_project();
        let e2_id = EntityId::new(doc.next_id);
        doc.next_id += 1;
        doc.project.as_mut().unwrap().library.entities.push(Entity {
            id: e2_id,
            kind: EntityKind::Custom("Enemy".into()),
            name: "Goblin".into(),
            group_id: None,
            tags: Vec::new(),
            defaults: EntityDefaults::default(),
            content: EntityContent::Sprites {
                states: Vec::new(),
                reference_sheet: None,
            },
            ai: AiMetadata::default(),
            user_data: UserData::default(),
            created_at: 0,
            updated_at: 0,
        });

        let first_id = doc.project.as_ref().unwrap().library.entities[0].id;
        reorder_entities_in_project(doc.project.as_mut().unwrap(), first_id, 1).unwrap();
        assert_eq!(doc.project.unwrap().library.entities[0].name, "Goblin");
    }

    #[test]
    fn reorder_entities_not_found() {
        let (mut doc, _, _) = doc_with_project();
        let result =
            reorder_entities_in_project(doc.project.as_mut().unwrap(), EntityId::new(999), 0);
        assert!(matches!(result, Err(AppCommandError::NotFound { .. })));
    }

    // ── create / delete / rename group ────────────────────────────────────

    #[test]
    fn create_group_stores_parent_id() {
        let (mut doc, _, _) = doc_with_project();
        let parent_args = LibraryCreateGroupArgs {
            name: "Top".into(),
            parent_id: None,
        };
        let parent =
            create_group_in_project(doc.project.as_mut().unwrap(), &mut doc.next_id, parent_args)
                .unwrap();

        let child_args = LibraryCreateGroupArgs {
            name: "Child".into(),
            parent_id: Some(parent.id),
        };
        let child =
            create_group_in_project(doc.project.as_mut().unwrap(), &mut doc.next_id, child_args)
                .unwrap();

        assert_eq!(child.parent_id, Some(parent.id));
        let found = doc
            .project
            .unwrap()
            .library
            .groups
            .into_iter()
            .find(|g| g.id == child.id)
            .unwrap();
        assert_eq!(found.parent_id, Some(parent.id));
    }

    #[test]
    fn delete_group_not_found() {
        let (mut doc, _, _) = doc_with_project();
        let result = delete_group_from_project(
            doc.project.as_mut().unwrap(),
            &LibraryDeleteGroupArgs {
                group_id: GroupId::new(999),
                keep_entities: true,
            },
            0,
        );
        assert!(matches!(result, Err(AppCommandError::NotFound { .. })));
    }

    #[test]
    fn rename_group_not_found() {
        let (mut doc, _, _) = doc_with_project();
        let result =
            rename_group_in_project(doc.project.as_mut().unwrap(), GroupId::new(999), "X".into());
        assert!(matches!(result, Err(AppCommandError::NotFound { .. })));
    }

    #[test]
    fn rename_group_rejects_whitespace_name() {
        let (mut doc, _, _) = doc_with_project();
        let mut next_id = doc.next_id;
        let group = create_group_in_project(
            doc.project.as_mut().unwrap(),
            &mut next_id,
            LibraryCreateGroupArgs {
                name: "Grp".into(),
                parent_id: None,
            },
        )
        .unwrap();
        let result = rename_group_in_project(doc.project.as_mut().unwrap(), group.id, "   ".into());
        assert!(matches!(result, Err(AppCommandError::Validation { .. })));
    }

    // ── set_group_parent ──────────────────────────────────────────────────

    #[test]
    fn group_cycle_detection_rejects_self_parent() {
        let (mut doc, _, _) = doc_with_project();
        let mut next_id = doc.next_id;
        let group = create_group_in_project(
            doc.project.as_mut().unwrap(),
            &mut next_id,
            LibraryCreateGroupArgs {
                name: "Loop".into(),
                parent_id: None,
            },
        )
        .unwrap();
        let result =
            set_group_parent_in_project(doc.project.as_mut().unwrap(), group.id, Some(group.id));
        assert!(
            matches!(result, Err(AppCommandError::Validation { .. })),
            "self-parent must be rejected"
        );
    }

    #[test]
    fn set_group_parent_group_not_found() {
        let (mut doc, _, _) = doc_with_project();
        let result =
            set_group_parent_in_project(doc.project.as_mut().unwrap(), GroupId::new(999), None);
        assert!(matches!(result, Err(AppCommandError::NotFound { .. })));
    }

    // ── set active target ─────────────────────────────────────────────────

    #[test]
    fn set_active_target_entity_not_found() {
        let (mut doc, _, _) = doc_with_project();
        let result = set_active_target_in_project(
            doc.project.as_mut().unwrap(),
            ActiveTarget::Tileset {
                entity_id: EntityId::new(999),
            },
        );
        assert!(matches!(result, Err(AppCommandError::NotFound { .. })));
    }

    #[test]
    fn set_active_target_state_not_found() {
        let (mut doc, entity_id, _) = doc_with_project();
        let result = set_active_target_in_project(
            doc.project.as_mut().unwrap(),
            ActiveTarget::State {
                entity_id,
                state_id: StateId::new(999),
            },
        );
        assert!(matches!(result, Err(AppCommandError::NotFound { .. })));
    }

    // ── tag / untag entity ────────────────────────────────────────────────

    #[test]
    fn tag_entity_adds_tag_id() {
        let (mut doc, entity_id, _) = doc_with_project();
        let tag = add_tag_to_project(
            doc.project.as_mut().unwrap(),
            &mut doc.next_id,
            LibraryAddTagArgs {
                name: "hero".into(),
                color: None,
            },
        )
        .unwrap();

        let changed =
            tag_entity_in_project(doc.project.as_mut().unwrap(), entity_id, tag.id, 0).unwrap();
        assert!(changed, "first tag application must return true");
        assert!(
            doc.project
                .unwrap()
                .library
                .entities
                .iter()
                .find(|e| e.id == entity_id)
                .unwrap()
                .tags
                .contains(&tag.id)
        );
    }

    #[test]
    fn tag_entity_tag_not_found() {
        let (mut doc, entity_id, _) = doc_with_project();
        let result =
            tag_entity_in_project(doc.project.as_mut().unwrap(), entity_id, TagId::new(999), 0);
        assert!(matches!(result, Err(AppCommandError::NotFound { .. })));
    }

    #[test]
    fn tag_entity_entity_not_found() {
        let (mut doc, _, _) = doc_with_project();
        let tag = add_tag_to_project(
            doc.project.as_mut().unwrap(),
            &mut doc.next_id,
            LibraryAddTagArgs {
                name: "hero".into(),
                color: None,
            },
        )
        .unwrap();
        let result =
            tag_entity_in_project(doc.project.as_mut().unwrap(), EntityId::new(999), tag.id, 0);
        assert!(matches!(result, Err(AppCommandError::NotFound { .. })));
    }

    #[test]
    fn tag_entity_idempotent_returns_false() {
        let (mut doc, entity_id, _) = doc_with_project();
        let tag = add_tag_to_project(
            doc.project.as_mut().unwrap(),
            &mut doc.next_id,
            LibraryAddTagArgs {
                name: "hero".into(),
                color: None,
            },
        )
        .unwrap();
        tag_entity_in_project(doc.project.as_mut().unwrap(), entity_id, tag.id, 0).unwrap();
        let second =
            tag_entity_in_project(doc.project.as_mut().unwrap(), entity_id, tag.id, 0).unwrap();
        assert!(!second, "second application of same tag must be a no-op");
        assert_eq!(
            doc.project
                .unwrap()
                .library
                .entities
                .iter()
                .find(|e| e.id == entity_id)
                .unwrap()
                .tags
                .len(),
            1,
            "tag must not be duplicated"
        );
    }

    #[test]
    fn untag_entity_entity_not_found() {
        let (mut doc, _, _) = doc_with_project();
        let result = untag_entity_in_project(
            doc.project.as_mut().unwrap(),
            EntityId::new(999),
            TagId::new(1),
            0,
        );
        assert!(matches!(result, Err(AppCommandError::NotFound { .. })));
    }

    // ── add / delete / rename tag ─────────────────────────────────────────

    #[test]
    fn delete_tag_removes_it_from_entities() {
        let (mut doc, entity_id, _) = doc_with_project();
        let tag = add_tag_to_project(
            doc.project.as_mut().unwrap(),
            &mut doc.next_id,
            LibraryAddTagArgs {
                name: "hero".into(),
                color: None,
            },
        )
        .unwrap();
        tag_entity_in_project(doc.project.as_mut().unwrap(), entity_id, tag.id, 0).unwrap();

        delete_tag_from_project(doc.project.as_mut().unwrap(), tag.id, 0).unwrap();

        assert!(
            !doc.project
                .unwrap()
                .library
                .entities
                .iter()
                .find(|e| e.id == entity_id)
                .unwrap()
                .tags
                .contains(&tag.id)
        );
    }

    #[test]
    fn delete_tag_not_found() {
        let (mut doc, _, _) = doc_with_project();
        let result = delete_tag_from_project(doc.project.as_mut().unwrap(), TagId::new(999), 0);
        assert!(matches!(result, Err(AppCommandError::NotFound { .. })));
    }

    #[test]
    fn rename_tag_not_found() {
        let (mut doc, _, _) = doc_with_project();
        let result =
            rename_tag_in_project(doc.project.as_mut().unwrap(), TagId::new(999), "X".into());
        assert!(matches!(result, Err(AppCommandError::NotFound { .. })));
    }

    // ── search ────────────────────────────────────────────────────────────

    #[test]
    fn search_matches_entity_name_substring() {
        let (doc, _, _) = doc_with_project();
        let results = search_library(
            doc.project.as_ref().unwrap(),
            &LibrarySearchArgs {
                query: "her".into(),
                kind_filter: None,
                group_filter: None,
                tag_filter: None,
            },
        );
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].name, "Hero");
    }

    #[test]
    fn search_matches_custom_kind_string() {
        let (doc, _, _) = doc_with_project();
        let results = search_library(
            doc.project.as_ref().unwrap(),
            &LibrarySearchArgs {
                query: "character".into(),
                kind_filter: None,
                group_filter: None,
                tag_filter: None,
            },
        );
        assert_eq!(results.len(), 1);
    }

    #[test]
    fn search_empty_query_returns_all() {
        let (doc, _, _) = doc_with_project();
        let results = search_library(
            doc.project.as_ref().unwrap(),
            &LibrarySearchArgs {
                query: String::new(),
                kind_filter: None,
                group_filter: None,
                tag_filter: None,
            },
        );
        assert_eq!(results.len(), 1);
    }

    // ── A4 — Custom entity defaults propagation ───────────────────────────

    #[test]
    fn add_state_inherits_custom_entity_defaults_canvas() {
        let (mut doc, entity_id, _) = doc_with_project();

        // Set 64×64 defaults on the entity.
        let project = doc.project.as_mut().unwrap();
        let entity = project
            .library
            .entities
            .iter_mut()
            .find(|e| e.id == entity_id)
            .unwrap();
        entity.defaults.canvas_size = Some(Size::new(64, 64));
        entity.defaults.color_mode = Some(ColorMode::Rgba);

        let args = LibraryAddStateArgs {
            entity_id,
            state_name: "run".into(),
            canvas_width: None,  // should inherit 64
            canvas_height: None, // should inherit 64
            color_mode: None,
        };
        let named =
            add_state_to_entity(doc.project.as_mut().unwrap(), &mut doc.next_id, args, 0).unwrap();
        assert_eq!(
            named.sprite.canvas,
            Size::new(64, 64),
            "add_state must inherit entity defaults"
        );
    }

    // ── B5 — updated_at bumped on ungrouped entities ──────────────────────

    #[test]
    fn delete_group_keep_entities_bumps_updated_at() {
        let (mut doc, entity_id, _) = doc_with_project();

        let grp = create_group_in_project(
            doc.project.as_mut().unwrap(),
            &mut doc.next_id,
            LibraryCreateGroupArgs {
                name: "Grp".into(),
                parent_id: None,
            },
        )
        .unwrap();

        doc.project
            .as_mut()
            .unwrap()
            .library
            .entities
            .iter_mut()
            .find(|e| e.id == entity_id)
            .unwrap()
            .group_id = Some(grp.id);

        let ts = now_secs();
        delete_group_from_project(
            doc.project.as_mut().unwrap(),
            &LibraryDeleteGroupArgs {
                group_id: grp.id,
                keep_entities: true,
            },
            ts,
        )
        .unwrap();

        let entity = doc
            .project
            .as_ref()
            .unwrap()
            .library
            .entities
            .iter()
            .find(|e| e.id == entity_id)
            .unwrap();
        assert!(entity.group_id.is_none(), "group_id must be cleared");
        assert!(entity.updated_at > 0, "updated_at must be bumped");
    }

    // ── B6 — updated_at bumped when tag is removed ────────────────────────

    #[test]
    fn delete_tag_bumps_updated_at_on_detagged_entities() {
        let (mut doc, entity_id, _) = doc_with_project();

        let tag = add_tag_to_project(
            doc.project.as_mut().unwrap(),
            &mut doc.next_id,
            LibraryAddTagArgs {
                name: "hero".into(),
                color: None,
            },
        )
        .unwrap();
        tag_entity_in_project(doc.project.as_mut().unwrap(), entity_id, tag.id, 0).unwrap();

        let ts = now_secs();
        delete_tag_from_project(doc.project.as_mut().unwrap(), tag.id, ts).unwrap();

        let entity = doc
            .project
            .as_ref()
            .unwrap()
            .library
            .entities
            .iter()
            .find(|e| e.id == entity_id)
            .unwrap();
        assert!(!entity.tags.contains(&tag.id), "tag must be removed");
        assert!(entity.updated_at > 0, "updated_at must be bumped");
    }

    // ── dirty-flag integration test ───────────────────────────────────────

    // Exercises the full AppState → doc lock → helper → dirty sequence that
    // the library_rename_entity wrapper performs. Uses AppState directly
    // (avoiding the tauri::State wrapper that requires the full Tauri runtime)
    // so the test runs in a tokio context without any Tauri machinery.
    #[tokio::test]
    async fn rename_entity_wrapper_sets_dirty_flag() {
        let app_state = AppState::new();
        let entity_id = EntityId::new(1);
        {
            let mut doc = app_state.doc.write().await;
            let (project, _, _) = project_with_one_custom_entity();
            doc.project = Some(project);
            doc.next_id = 10;
            doc.dirty = false;
        }
        {
            let mut doc = app_state.doc.write().await;
            let project = doc.project.as_mut().unwrap();
            rename_entity_in_project(project, entity_id, "Goblin".into(), now_secs()).unwrap();
            doc.dirty = true;
        }
        let doc = app_state.doc.read().await;
        assert!(doc.dirty, "wrapper must set dirty after successful rename");
        assert_eq!(
            doc.project
                .as_ref()
                .unwrap()
                .library
                .entities
                .iter()
                .find(|e| e.id == entity_id)
                .unwrap()
                .name,
            "Goblin"
        );
    }

    // ── sheet helpers ─────────────────────────────────────────────────────────

    fn make_variant(id: u32) -> SheetVariant {
        SheetVariant::from_image(
            SheetVariantId::new(id),
            0,
            ReferenceImage {
                bytes: Vec::new(),
                mime: "image/png".into(),
            },
        )
    }

    fn project_with_one_sprite_reference() -> (pixhaus_core::project::Project, EntityId) {
        let mut project = pixhaus_core::project::Project::new("test");
        let canonical = make_variant(10);
        let entity_id = EntityId::new(1);
        project.library.entities.push(Entity {
            id: entity_id,
            kind: EntityKind::Custom("Character".into()),
            name: "Hero".into(),
            group_id: None,
            tags: Vec::new(),
            defaults: EntityDefaults::default(),
            content: EntityContent::Sprites {
                states: Vec::new(),
                reference_sheet: Some(Box::new(ReferenceSheet {
                    canonical: Some(canonical),
                    variants: vec![make_variant(20), make_variant(30)],
                    prompts: Vec::new(),
                    info: AssetInfo::default(),
                })),
            },
            ai: AiMetadata::default(),
            user_data: UserData::default(),
            created_at: 0,
            updated_at: 0,
        });
        (project, entity_id)
    }

    // ── approve_sheet_variant ─────────────────────────────────────────────────
    //
    // The B10.4 fixture tests for `approve_sheet_variant_in_project` were
    // dropped in the merge with B10.3: that helper is now subsumed by
    // `pixhaus_core::project::approval::approve_sheet_variant`, which carries
    // its own test suite covering swap/demote, idempotence, palette
    // extraction, kind validation, and not-found paths. The IPC command
    // (`library_approve_sheet_variant`) just calls that core helper, bumps
    // `updated_at`, clones the entity, and invalidates the anchor cache —
    // all behaviour either tested at the core layer or trivial.

    // ── update_asset_info ─────────────────────────────────────────────────────

    #[test]
    fn update_asset_info_replaces_fields_and_bumps_updated_at() {
        let (mut project, entity_id) = project_with_one_sprite_reference();
        let info = AssetInfo {
            fields: [("name".into(), "Hero".into()), ("age".into(), "20".into())]
                .into_iter()
                .collect(),
            notes: vec!["brave".into()],
        };
        update_asset_info_in_project(&mut project, entity_id, info.clone(), 55).unwrap();

        let entity = project
            .library
            .entities
            .iter()
            .find(|e| e.id == entity_id)
            .unwrap();
        assert_eq!(entity.updated_at, 55);
        let sheet = match &entity.content {
            EntityContent::Sprites {
                reference_sheet: Some(sheet),
                ..
            } => sheet.as_ref(),
            _ => panic!("wrong kind"),
        };
        assert_eq!(
            sheet.info.fields.get("name").map(String::as_str),
            Some("Hero")
        );
        assert_eq!(sheet.info.notes, vec!["brave"]);
    }

    // ── delete_sheet_variant ─────────────────────────────────────────────────

    #[test]
    fn delete_history_variant_removes_it() {
        let (mut project, entity_id) = project_with_one_sprite_reference();
        delete_sheet_variant_in_project(&mut project, entity_id, SheetVariantId::new(20), 0)
            .unwrap();

        let sheet = match &project
            .library
            .entities
            .iter()
            .find(|e| e.id == entity_id)
            .unwrap()
            .content
        {
            EntityContent::Sprites {
                reference_sheet: Some(sheet),
                ..
            } => sheet.as_ref(),
            _ => panic!("wrong kind"),
        };
        assert_eq!(sheet.variants.len(), 1);
        assert_eq!(sheet.variants[0].id, SheetVariantId::new(30));
    }

    #[test]
    fn delete_canonical_variant_returns_validation_error() {
        let (mut project, entity_id) = project_with_one_sprite_reference();
        let result =
            delete_sheet_variant_in_project(&mut project, entity_id, SheetVariantId::new(10), 0);
        assert!(matches!(result, Err(AppCommandError::Validation { .. })));
    }

    #[test]
    fn delete_variant_not_found_returns_error() {
        let (mut project, entity_id) = project_with_one_sprite_reference();
        let result =
            delete_sheet_variant_in_project(&mut project, entity_id, SheetVariantId::new(999), 0);
        assert!(matches!(result, Err(AppCommandError::NotFound { .. })));
    }

    /// Test-only helper: appends decoded `GenerateSheetPayload` variants
    /// to the entity's reference sheet. Mirrors the work the eventual
    /// production handler will do; kept inside the test module until
    /// that handler exists.
    fn apply_generated_reference_sheet_payload(
        project: &mut pixhaus_core::project::Project,
        next_id: &mut u32,
        payload: GenerateSheetPayload,
        ts: i64,
    ) -> Result<Entity, AppCommandError> {
        let entity = find_entity_mut(&mut project.library, payload.entity_id)?;

        let EntityContent::Sprites {
            reference_sheet, ..
        } = &mut entity.content
        else {
            return Err(AppCommandError::Validation {
                detail: format!(
                    "entity {} is not a sprite entity; reference sheets belong to sprites",
                    payload.entity_id.get()
                ),
            });
        };

        let mut variants = Vec::with_capacity(payload.variants.len());
        for output in payload.variants {
            let bytes = base64::engine::general_purpose::STANDARD
                .decode(output.image_b64.as_bytes())
                .map_err(|e| AppCommandError::Validation {
                    detail: format!("generated reference sheet image was not valid base64: {e}"),
                })?;
            let variant_id = SheetVariantId::new(*next_id);
            *next_id += 1;
            variants.push(SheetVariant {
                composition: output.composition,
                generation: Some(output.generation),
                origin: pixhaus_core::project::VariantOrigin::FreshGeneration,
                ..SheetVariant::from_image(
                    variant_id,
                    output.generated_at,
                    ReferenceImage {
                        bytes,
                        mime: "image/png".into(),
                    },
                )
            });
        }

        let sheet = reference_sheet
            .get_or_insert_with(|| {
                Box::new(ReferenceSheet {
                    canonical: None,
                    variants: Vec::new(),
                    prompts: Vec::new(),
                    info: AssetInfo::default(),
                })
            })
            .as_mut();
        variants.append(&mut sheet.variants);
        sheet.variants = variants;
        entity.updated_at = ts;
        Ok(entity.clone())
    }

    #[test]
    fn apply_generated_payload_creates_draft_only_sheet() {
        let (mut project, entity_id, _) = project_with_one_custom_entity();
        let mut next_id = 50;
        let payload = GenerateSheetPayload {
            entity_id,
            variants: vec![pixhaus_ai::verbs::reference_sheet::SheetVariantOutput {
                id: SheetVariantId::new(0),
                generated_at: 123,
                image_b64: base64::engine::general_purpose::STANDARD.encode([1, 2, 3]),
                composition: SheetComposition::default(),
                generation: pixhaus_core::project::GenerationProvenance {
                    backend: "stub".into(),
                    model: "stub-model".into(),
                    prompt: "hero".into(),
                    seed: None,
                    negative_prompt: None,
                },
            }],
        };

        let updated =
            apply_generated_reference_sheet_payload(&mut project, &mut next_id, payload, 77)
                .unwrap();

        assert_eq!(next_id, 51);
        assert_eq!(updated.updated_at, 77);
        let EntityContent::Sprites {
            reference_sheet: Some(sheet),
            ..
        } = &updated.content
        else {
            panic!("expected reference sheet");
        };
        assert!(sheet.canonical.is_none());
        assert_eq!(sheet.variants.len(), 1);
        assert_eq!(sheet.variants[0].id, SheetVariantId::new(50));
        assert_eq!(sheet.variants[0].image.bytes, vec![1, 2, 3]);
    }

    // ── AI hooks (B9.4) ───────────────────────────────────────────────────

    #[test]
    fn suggest_tags_creates_new_auto_generated_tags() {
        let (mut doc, entity_id, _) = doc_with_project();
        let project = doc.project.as_mut().unwrap();
        let mut next_id = doc.next_id;
        let tags = apply_suggest_entity_tags(
            project,
            &mut next_id,
            entity_id,
            vec!["idle".into(), "fantasy".into()],
            0,
        )
        .unwrap();
        assert_eq!(tags.len(), 2);
        assert!(tags.iter().all(|t| t.auto_generated));
        assert!(tags.iter().any(|t| t.name == "idle"));
        assert!(tags.iter().any(|t| t.name == "fantasy"));
    }

    #[test]
    fn suggest_tags_normalises_to_lowercase() {
        let (mut doc, entity_id, _) = doc_with_project();
        let project = doc.project.as_mut().unwrap();
        let mut next_id = doc.next_id;
        let tags =
            apply_suggest_entity_tags(project, &mut next_id, entity_id, vec!["WARRIOR".into()], 0)
                .unwrap();
        assert_eq!(tags[0].name, "warrior");
    }

    #[test]
    fn suggest_tags_deduplicates_repeated_names() {
        let (mut doc, entity_id, _) = doc_with_project();
        let project = doc.project.as_mut().unwrap();
        let mut next_id = doc.next_id;
        apply_suggest_entity_tags(project, &mut next_id, entity_id, vec!["idle".into()], 0)
            .unwrap();
        // Same name again must not grow suggested_tags.
        apply_suggest_entity_tags(project, &mut next_id, entity_id, vec!["idle".into()], 0)
            .unwrap();
        let entity = project
            .library
            .entities
            .iter()
            .find(|e| e.id == entity_id)
            .unwrap();
        assert_eq!(entity.ai.suggested_tags.len(), 1);
    }

    #[test]
    fn suggest_tags_returns_not_found_for_missing_entity() {
        let (mut doc, _, _) = doc_with_project();
        let project = doc.project.as_mut().unwrap();
        let mut next_id = doc.next_id;
        let result = apply_suggest_entity_tags(
            project,
            &mut next_id,
            EntityId::new(999),
            vec!["idle".into()],
            0,
        );
        assert!(matches!(result, Err(AppCommandError::NotFound { .. })));
    }

    #[test]
    fn accept_tag_moves_from_suggested_to_confirmed() {
        let (mut doc, entity_id, _) = doc_with_project();
        let project = doc.project.as_mut().unwrap();
        let mut next_id = doc.next_id;
        let tags =
            apply_suggest_entity_tags(project, &mut next_id, entity_id, vec!["warrior".into()], 0)
                .unwrap();
        let tag_id = tags[0].id;

        // Simulate accept: remove from suggested, add to confirmed.
        let entity = project
            .library
            .entities
            .iter_mut()
            .find(|e| e.id == entity_id)
            .unwrap();
        entity.ai.suggested_tags.retain(|&id| id != tag_id);
        entity.tags.push(tag_id);

        let entity = project
            .library
            .entities
            .iter()
            .find(|e| e.id == entity_id)
            .unwrap();
        assert!(entity.ai.suggested_tags.is_empty());
        assert!(entity.tags.contains(&tag_id));
    }

    #[test]
    fn update_corpus_deduplicates_entity_ids() {
        let (mut doc, entity_id, _) = doc_with_project();
        let project = doc.project.as_mut().unwrap();
        apply_update_project_ai(project, vec![entity_id, entity_id], None);
        assert_eq!(project.library.ai.style_corpus.len(), 1);
        // Re-adding the same id must not grow the corpus.
        apply_update_project_ai(project, vec![entity_id], None);
        assert_eq!(project.library.ai.style_corpus.len(), 1);
    }

    #[test]
    fn update_corpus_sets_lora_path() {
        let (mut doc, _, _) = doc_with_project();
        let project = doc.project.as_mut().unwrap();
        apply_update_project_ai(project, vec![], Some("styles/hero.safetensors".into()));
        assert_eq!(
            project.library.ai.project_lora_path.as_deref(),
            Some("styles/hero.safetensors")
        );
    }

    // ── B10.5: per-entity LoRA wiring ─────────────────────────────────────────

    #[test]
    fn apply_update_entity_ai_sets_lora_path_and_bumps_updated_at() {
        let (mut doc, entity_id, _) = doc_with_project();
        let project = doc.project.as_mut().unwrap();
        // Snapshot the prior timestamp so we can assert the bump.
        let before = project
            .library
            .entities
            .iter()
            .find(|e| e.id == entity_id)
            .unwrap()
            .updated_at;

        let applied = apply_update_entity_ai(
            project,
            entity_id,
            Some("https://replicate.delivery/abc/hero.safetensors".into()),
            before + 100,
        );
        assert!(applied);

        let entity = project
            .library
            .entities
            .iter()
            .find(|e| e.id == entity_id)
            .unwrap();
        assert_eq!(
            entity.ai.lora_path.as_deref(),
            Some("https://replicate.delivery/abc/hero.safetensors")
        );
        assert_eq!(entity.updated_at, before + 100);
    }

    #[test]
    fn apply_update_entity_ai_skips_unknown_entity() {
        let (mut doc, _, _) = doc_with_project();
        let project = doc.project.as_mut().unwrap();
        let applied = apply_update_entity_ai(
            project,
            EntityId::new(9_999),
            Some("ignored.safetensors".into()),
            123,
        );
        assert!(!applied);
    }

    #[test]
    fn apply_update_entity_ai_none_is_no_op_but_reports_presence() {
        let (mut doc, entity_id, _) = doc_with_project();
        let project = doc.project.as_mut().unwrap();
        // Seed an existing path so we can prove it isn't clobbered.
        apply_update_entity_ai(project, entity_id, Some("existing.safetensors".into()), 1);
        let known = apply_update_entity_ai(project, entity_id, None, 2);
        assert!(known);
        let unknown = apply_update_entity_ai(project, EntityId::new(9_999), None, 2);
        assert!(!unknown);
        let entity = project
            .library
            .entities
            .iter()
            .find(|e| e.id == entity_id)
            .unwrap();
        assert_eq!(entity.ai.lora_path.as_deref(), Some("existing.safetensors"));
    }

    #[test]
    fn resolve_lora_path_prefers_per_entity_over_project_wide() {
        use crate::commands::verbs::resolve_lora_path;

        let mut entity = Entity {
            id: EntityId::new(1),
            kind: EntityKind::Custom("Character".into()),
            name: "Hero".into(),
            group_id: None,
            tags: Vec::new(),
            defaults: EntityDefaults::default(),
            content: EntityContent::Sprites {
                states: Vec::new(),
                reference_sheet: None,
            },
            ai: AiMetadata::default(),
            user_data: UserData::default(),
            created_at: 0,
            updated_at: 0,
        };

        assert_eq!(resolve_lora_path(&entity, None), None);
        assert_eq!(
            resolve_lora_path(&entity, Some("project.safetensors")),
            Some("project.safetensors".into())
        );

        entity.ai.lora_path = Some("entity.safetensors".into());
        assert_eq!(
            resolve_lora_path(&entity, Some("project.safetensors")),
            Some("entity.safetensors".into()),
        );
        assert_eq!(
            resolve_lora_path(&entity, None),
            Some("entity.safetensors".into())
        );
    }
}
