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
use pixhaus_ai::plugin::context::VerbContextBuilder;
use pixhaus_ai::plugin::descriptor::BackendCapabilities;
use pixhaus_ai::plugin::descriptor::VerbId;
use pixhaus_ai::plugin::inputs::VerbInputs;
use pixhaus_ai::plugin::output::VerbEffect;
use pixhaus_ai::plugin::context::PixelData;
use pixhaus_ai::plugin::progress::{VerbProgress, VerbProgressEvent};
use pixhaus_ai::plugin::runtime::VerbRuntime;
use pixhaus_ai::plugin::{AnchorPayload, DEFAULT_ANCHOR_STRENGTH};
use pixhaus_ai::verbs::critique::{CritiqueInputs, CritiqueMode};
use pixhaus_ai::verbs::reference_sheet::GenerateSheetPayload;
use pixhaus_core::color::extraction::{ExtractionOptions, extract_palette_from_image_bytes};
use pixhaus_core::project::approval::{ApprovalError, approve_sheet_variant};
use pixhaus_core::project::{
    ActiveTarget, AiMetadata, AssetId, AssetInfo, AssetLibrary, CharacterCard, ChatTranscript,
    ChatTurn, ColorMode, Entity, EntityContent, EntityDefaults, EntityGroup, EntityId, EntityKind,
    GroupId, LoraAsset, LoraKind, ModelId, NamedSprite, OperationKind, PixelBufferId, Quality,
    ReferenceAsset, ReferenceImage, ReferenceRole, ReferenceSheet,
    ReferenceSheetTemplateDefinition, ReferenceSheetTemplateId, ReferenceSlot, RefinementKind,
    Rgba, SheetVariant, SheetVariantId, Size, Sprite, SpriteId, StateId, StyleSwatch,
    TagDefinition, TagId, TilemapScene, Tileset, TilesetId, TilesetSource, TrainingJob,
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
        if !project.library.groups.iter().any(|g| g.id == gid) {
            return Err(AppCommandError::NotFound {
                entity: "group".into(),
                id: u64::from(gid.get()),
            });
        }
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
    let entity = project
        .library
        .entities
        .iter_mut()
        .find(|e| e.id == entity_id)
        .ok_or(AppCommandError::NotFound {
            entity: "entity".into(),
            id: u64::from(entity_id.get()),
        })?;
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
    let entity = project
        .library
        .entities
        .iter_mut()
        .find(|e| e.id == entity_id)
        .ok_or(AppCommandError::NotFound {
            entity: "entity".into(),
            id: u64::from(entity_id.get()),
        })?;

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

    let entity = project
        .library
        .entities
        .iter_mut()
        .find(|e| e.id == entity_id)
        .ok_or(AppCommandError::NotFound {
            entity: "entity".into(),
            id: u64::from(entity_id.get()),
        })?;

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
        if !project.library.groups.iter().any(|g| g.id == gid) {
            return Err(AppCommandError::NotFound {
                entity: "group".into(),
                id: u64::from(gid.get()),
            });
        }
    }

    let entity = project
        .library
        .entities
        .iter_mut()
        .find(|e| e.id == entity_id)
        .ok_or(AppCommandError::NotFound {
            entity: "entity".into(),
            id: u64::from(entity_id.get()),
        })?;
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
        if !project.library.groups.iter().any(|g| g.id == pid) {
            return Err(AppCommandError::NotFound {
                entity: "group".into(),
                id: u64::from(pid.get()),
            });
        }
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
    if !project.library.groups.iter().any(|g| g.id == group_id) {
        return Err(AppCommandError::NotFound {
            entity: "group".into(),
            id: u64::from(group_id.get()),
        });
    }

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
        if !project.library.groups.iter().any(|g| g.id == pid) {
            return Err(AppCommandError::NotFound {
                entity: "group".into(),
                id: u64::from(pid.get()),
            });
        }
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
            let entity = project
                .library
                .entities
                .iter()
                .find(|e| e.id == entity_id)
                .ok_or(AppCommandError::NotFound {
                    entity: "entity".into(),
                    id: u64::from(entity_id.get()),
                })?;
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

    let entity = project
        .library
        .entities
        .iter_mut()
        .find(|e| e.id == entity_id)
        .ok_or(AppCommandError::NotFound {
            entity: "entity".into(),
            id: u64::from(entity_id.get()),
        })?;

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
    let entity = project
        .library
        .entities
        .iter_mut()
        .find(|e| e.id == entity_id)
        .ok_or(AppCommandError::NotFound {
            entity: "entity".into(),
            id: u64::from(entity_id.get()),
        })?;

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

// ── embedded reference sheet commands ────────────────────────────────────────

/// Arguments for approving a history variant as canonical.
#[derive(Debug, Deserialize)]
pub struct LibraryApproveSheetVariantArgs {
    /// Target sprite entity. Must own an embedded reference sheet.
    pub entity_id: EntityId,
    /// The variant to approve. Must be present in the entity's `history`.
    pub variant_id: SheetVariantId,
}

/// Arguments for updating a sprite entity's embedded reference-sheet info.
#[derive(Debug, Deserialize)]
pub struct LibraryUpdateAssetInfoArgs {
    /// Target sprite entity. Must own an embedded reference sheet.
    pub entity_id: EntityId,
    /// Replacement asset info. Overwrites the existing value.
    pub info: AssetInfo,
}

/// Arguments for generating reference-sheet draft candidates.
#[derive(Debug, Deserialize)]
pub struct LibraryGenerateReferenceSheetArgs {
    /// Target sprite entity that will receive generated draft variants.
    pub entity_id: EntityId,
    /// User description of the subject.
    pub prompt: String,
    /// Sheet template.
    pub template: ReferenceSheetTemplateId,
    /// Output width in pixels.
    pub width: u32,
    /// Output height in pixels.
    pub height: u32,
    /// Requested chroma-key background color.
    pub chroma_color: Rgba,
    /// Backend quality hint.
    #[serde(default)]
    pub quality: Quality,
    /// Number of candidates to request. Clamped by the verb.
    pub candidate_count: u8,
    /// Optional explicit model override. `None`/`Auto` uses routing.
    #[serde(default)]
    pub model: Option<ModelId>,
    /// Ordered generation references.
    #[serde(default)]
    pub references: Vec<ReferenceSlot>,
    /// Search/grounding hint for Google image models.
    #[serde(default)]
    pub real_world_grounding: bool,
    /// Optional Flux LoRA asset to apply.
    #[serde(default)]
    pub applied_lora: Option<AssetId>,
    /// LoRA strength when a Flux LoRA is applied.
    #[serde(default = "default_request_lora_weight")]
    pub lora_weight: f32,
}

fn default_request_lora_weight() -> f32 {
    1.0
}

/// Arguments for importing a reference-sheet image as an unapproved draft.
#[derive(Debug, Deserialize)]
pub struct LibraryImportReferenceSheetArgs {
    /// Target sprite entity that will receive the draft.
    pub entity_id: EntityId,
    /// Image bytes to store.
    pub bytes: Vec<u8>,
    /// MIME type for `bytes`. Defaults to `image/png` when empty.
    pub mime: Option<String>,
}

/// Arguments for removing a non-canonical reference-sheet variant.
#[derive(Debug, Deserialize)]
pub struct LibraryRemoveReferenceSheetVariantArgs {
    /// Target sprite entity.
    pub entity_id: EntityId,
    /// Draft/history variant to remove.
    pub variant_id: SheetVariantId,
}

/// Generates draft reference-sheet variants for a sprite entity.
///
/// The generated candidates are persisted in `ReferenceSheet::history`
/// and never become canonical until the user approves one.
#[tauri::command(async, rename_all = "snake_case")]
pub async fn library_generate_reference_sheet(
    args: LibraryGenerateReferenceSheetArgs,
    app: AppHandle,
    state: State<'_, AppState>,
) -> CommandResult<RequestId> {
    if args.prompt.trim().is_empty() {
        return Err(AppCommandError::Validation {
            detail: "reference sheet prompt must not be empty".into(),
        });
    }

    let (style_notes, model, lora_asset) = {
        let doc = state.doc.read().await;
        let project = doc
            .project
            .as_ref()
            .ok_or(AppCommandError::NoActiveProject)?;
        let entity = project
            .library
            .entities
            .iter()
            .find(|e| e.id == args.entity_id)
            .ok_or_else(|| AppCommandError::NotFound {
                entity: "entity".into(),
                id: u64::from(args.entity_id.get()),
            })?;
        if !matches!(entity.content, EntityContent::Sprites { .. }) {
            return Err(AppCommandError::Validation {
                detail: format!(
                    "entity {} is not a sprite entity; reference sheets belong to sprites",
                    args.entity_id.get()
                ),
            });
        }
        let preferred = selected_model(args.model, OperationKind::FreshGeneration, project);
        let model = configured_execution_model(preferred, &state);
        let lora_asset = find_lora_asset(project, args.applied_lora);
        (project.library.ai.style_notes.clone(), model, lora_asset)
    };

    let (request_id, token, lease) =
        start_sheet_request_with_lease(&state, args.entity_id, true, None)?;
    let spec = VariantCommitSpec {
        provider: SheetProviderRequest {
            operation: OperationKind::FreshGeneration,
            model,
            quality: args.quality,
            prompt: args.prompt,
            template: args.template,
            width: args.width,
            height: args.height,
            chroma_color: args.chroma_color,
            references: args.references,
            source_image: None,
            mask: None,
            candidate_count: args.candidate_count,
            applied_lora: lora_asset,
            lora_weight: args.lora_weight,
            real_world_grounding: args.real_world_grounding,
        },
        build: VariantBuildSpec {
            origin: VariantOrigin::FreshGeneration,
            parent_variant_id: None,
            refinement: None,
            promotion: false,
            chat_transcript: None,
            applied_lora: args.applied_lora,
            lora_weight: args.lora_weight,
            real_world_grounding: args.real_world_grounding,
        },
        stream_index: 0,
    };
    spawn_sheet_provider_job(
        app,
        request_id,
        args.entity_id,
        token,
        lease,
        style_notes,
        spec,
    );
    Ok(request_id)
}

/// Imports a reference image as a draft candidate on a sprite entity.
#[tauri::command(async, rename_all = "snake_case")]
pub async fn library_import_reference_sheet(
    args: LibraryImportReferenceSheetArgs,
    state: State<'_, AppState>,
) -> CommandResult<Entity> {
    if args.bytes.is_empty() {
        return Err(AppCommandError::Validation {
            detail: "reference sheet image bytes must not be empty".into(),
        });
    }

    let ts = now_secs();
    let mut doc = state.doc.write().await;
    let mut next_id = doc.next_id;
    let project = doc
        .project
        .as_mut()
        .ok_or(AppCommandError::NoActiveProject)?;
    let updated = import_reference_sheet_draft(project, &mut next_id, args, ts)?;
    doc.next_id = next_id;
    doc.dirty = true;
    Ok(updated)
}

/// Approves a [`SheetVariant`] as the canonical embedded reference sheet of
/// a sprite entity.
///
/// Moves the variant from `history` into `canonical`, demotes the previous
/// canonical to `history[0]` when one exists, runs eyedropper palette
/// extraction over the new canonical's image bytes (skipped when the
/// variant already carries an extracted palette), bumps `updated_at`, and
/// invalidates any cached [`AnchorPayload`] for the entity. Returns the
/// updated entity so the UI can refresh local state without a separate
/// `library_get_entity`.
#[tauri::command(async, rename_all = "snake_case")]
pub async fn library_approve_sheet_variant(
    args: LibraryApproveSheetVariantArgs,
    state: State<'_, AppState>,
) -> CommandResult<Entity> {
    let ts = now_secs();
    let mut doc = state.doc.write().await;
    let project = doc
        .project
        .as_mut()
        .ok_or(AppCommandError::NoActiveProject)?;
    // Run the full B10.3 approval flow (variant swap + palette extraction).
    approve_sheet_variant(
        project,
        args.entity_id,
        args.variant_id,
        ExtractionOptions::default(),
    )?;
    // Bump `updated_at` so the UI refresh observes the entity as changed.
    let entity = project
        .library
        .entities
        .iter_mut()
        .find(|e| e.id == args.entity_id)
        .ok_or_else(|| AppCommandError::NotFound {
            entity: "entity".into(),
            id: u64::from(args.entity_id.get()),
        })?;
    entity.updated_at = ts;
    let updated = entity.clone();
    doc.dirty = true;
    drop(doc);

    state.anchor_cache.remove(&args.entity_id.get());

    Ok(updated)
}

/// Returns the current [`AnchorPayload`] for an entity, building it lazily
/// and caching the result.
#[tauri::command(async, rename_all = "snake_case")]
pub async fn library_get_anchor_payload(
    entity_id: EntityId,
    state: State<'_, AppState>,
) -> CommandResult<Option<AnchorPayload>> {
    let doc = state.doc.read().await;
    let project = doc
        .project
        .as_ref()
        .ok_or(AppCommandError::NoActiveProject)?;
    let entity = project
        .library
        .entities
        .iter()
        .find(|e| e.id == entity_id)
        .ok_or(AppCommandError::NotFound {
            entity: "entity".into(),
            id: u64::from(entity_id.get()),
        })?;

    let sheet = match &entity.content {
        EntityContent::Sprites {
            reference_sheet: Some(sheet),
            ..
        } => sheet.as_ref(),
        _ => return Ok(None),
    };
    let Some(canonical) = sheet.canonical.as_ref() else {
        return Ok(None);
    };
    let live_hash = pixhaus_ai::plugin::anchor::stable_hash(&canonical.image.bytes);
    let lora_path = crate::commands::verbs::resolve_lora_path(
        entity,
        project.library.ai.project_lora_path.as_deref(),
    );

    if let Some(cached) = state.anchor_cache.get(&entity.id.get()) {
        if cached.canonical_hash == live_hash && cached.lora_path == lora_path {
            return Ok(Some(cached.clone()));
        }
    }

    let payload = AnchorPayload::from_sprite_entity(entity, DEFAULT_ANCHOR_STRENGTH, lora_path);

    if let Some(p) = &payload {
        state
            .anchor_cache
            .insert(p.reference_entity_id.get(), p.clone());
    }

    Ok(payload)
}

/// Updates the asset info (name, age, species, personality notes) for a
/// sprite entity's embedded reference sheet.
#[tauri::command(async, rename_all = "snake_case")]
pub async fn library_update_asset_info(
    args: LibraryUpdateAssetInfoArgs,
    state: State<'_, AppState>,
) -> CommandResult<()> {
    let ts = now_secs();
    let mut doc = state.doc.write().await;
    let project = doc
        .project
        .as_mut()
        .ok_or(AppCommandError::NoActiveProject)?;
    update_asset_info_in_project(project, args.entity_id, args.info, ts)?;
    doc.dirty = true;
    Ok(())
}

/// Deletes a history variant from a sprite entity's embedded reference sheet.
#[tauri::command(async, rename_all = "snake_case")]
pub async fn library_delete_sheet_variant(
    entity_id: EntityId,
    variant_id: SheetVariantId,
    state: State<'_, AppState>,
) -> CommandResult<()> {
    let ts = now_secs();
    let mut doc = state.doc.write().await;
    let project = doc
        .project
        .as_mut()
        .ok_or(AppCommandError::NoActiveProject)?;
    delete_sheet_variant_in_project(project, entity_id, variant_id, ts)?;
    doc.dirty = true;
    Ok(())
}

/// Removes a non-canonical reference-sheet draft/history variant.
#[tauri::command(async, rename_all = "snake_case")]
pub async fn library_remove_reference_sheet_variant(
    args: LibraryRemoveReferenceSheetVariantArgs,
    state: State<'_, AppState>,
) -> CommandResult<Entity> {
    let ts = now_secs();
    let mut doc = state.doc.write().await;
    let project = doc
        .project
        .as_mut()
        .ok_or(AppCommandError::NoActiveProject)?;
    delete_sheet_variant_in_project(project, args.entity_id, args.variant_id, ts)?;
    let updated = project
        .library
        .entities
        .iter()
        .find(|e| e.id == args.entity_id)
        .ok_or_else(|| AppCommandError::NotFound {
            entity: "entity".into(),
            id: u64::from(args.entity_id.get()),
        })?
        .clone();
    doc.dirty = true;
    Ok(updated)
}

/// Request id returned by v1 reference-sheet operations.
pub type RequestId = u64;

const SHEET_REQUEST_PROGRESS_EVENT: &str = "SheetRequestProgress";
const SHEET_REQUEST_CANDIDATE_COMPLETE_EVENT: &str = "SheetRequestCandidateComplete";
const SHEET_REQUEST_COMPLETE_EVENT: &str = "SheetRequestComplete";
const SHEET_REQUEST_CANCELLED_EVENT: &str = "SheetRequestCancelled";
const SHEET_REQUEST_ERROR_EVENT: &str = "SheetRequestError";
const TRAINING_JOB_PROGRESS_EVENT: &str = "TrainingJobProgress";
const TRAINING_JOB_COMPLETE_EVENT: &str = "TrainingJobComplete";
const TRAINING_JOB_FAILED_EVENT: &str = "TrainingJobFailed";

/// Progress event emitted for reference-sheet jobs.
#[allow(missing_docs)]
#[derive(Clone, Debug, Serialize)]
pub struct SheetRequestProgressPayload {
    pub request_id: RequestId,
    pub stream_index: u8,
    pub candidate_index: u8,
    pub partial_index: u8,
    pub partial_image: Option<ReferenceImage>,
    pub elapsed_ms: u32,
}

/// Candidate-complete event emitted after a draft variant is committed.
#[allow(missing_docs)]
#[derive(Clone, Debug, Serialize)]
pub struct SheetRequestCandidateCompletePayload {
    pub request_id: RequestId,
    pub stream_index: u8,
    pub candidate_index: u8,
    pub variant: SheetVariant,
}

/// Completion event emitted after all candidates for a request are committed.
#[allow(missing_docs)]
#[derive(Clone, Debug, Serialize)]
pub struct SheetRequestCompletePayload {
    pub request_id: RequestId,
    pub entity_id: EntityId,
    pub sprite: Entity,
    pub total_cost_usd: f64,
}

/// Cancellation event emitted when a job exits through its cancellation token.
#[allow(missing_docs)]
#[derive(Clone, Debug, Serialize)]
pub struct SheetRequestCancelledPayload {
    pub request_id: RequestId,
}

/// Error event emitted for provider/app failures after a request id is minted.
#[allow(missing_docs)]
#[derive(Clone, Debug, Serialize)]
pub struct SheetRequestErrorPayload {
    pub request_id: RequestId,
    pub error: AppCommandError,
}

/// LoRA training progress event.
#[allow(missing_docs)]
#[derive(Clone, Debug, Serialize)]
pub struct TrainingJobProgressPayload {
    pub job_id: TrainingJobId,
    pub status: TrainingStatus,
    pub message: String,
}

/// LoRA training completion event.
#[allow(missing_docs)]
#[derive(Clone, Debug, Serialize)]
pub struct TrainingJobCompletePayload {
    pub job_id: TrainingJobId,
    pub lora_asset: LoraAsset,
}

/// LoRA training failure event.
#[allow(missing_docs)]
#[derive(Clone, Debug, Serialize)]
pub struct TrainingJobFailedPayload {
    pub job_id: TrainingJobId,
    pub error: AppCommandError,
}

/// Model/quality pair used by cross-model comparison.
#[allow(missing_docs)]
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ModelQualityPair {
    pub model: ModelId,
    pub quality: Quality,
}

/// Arguments for refining a sheet variant.
#[allow(missing_docs)]
#[derive(Debug, Deserialize)]
pub struct LibraryRefineReferenceSheetVariantArgs {
    pub entity_id: EntityId,
    pub parent_variant_id: SheetVariantId,
    pub refinement: RefinementKind,
    pub prompt: String,
    pub quality: Quality,
    pub candidate_count: u8,
    pub model: Option<ModelId>,
    #[serde(default)]
    pub additional_references: Vec<ReferenceSlot>,
}

/// Arguments for a conversational sheet edit.
#[allow(missing_docs)]
#[derive(Debug, Deserialize)]
pub struct LibrarySubmitChatTurnArgs {
    pub entity_id: EntityId,
    pub variant_id: SheetVariantId,
    pub user_message: String,
    #[serde(default)]
    pub mask: Option<ReferenceImage>,
    pub model: Option<ModelId>,
}

/// Arguments for "promote to final".
#[allow(missing_docs)]
#[derive(Debug, Deserialize)]
pub struct LibraryPromoteVariantToFinalArgs {
    pub entity_id: EntityId,
    pub source_variant_id: SheetVariantId,
    pub target_quality: Quality,
    pub target_model: Option<ModelId>,
}

/// Arguments for cross-model comparison.
#[allow(missing_docs)]
#[derive(Debug, Deserialize)]
pub struct LibraryStartCrossModelGridArgs {
    pub entity_id: EntityId,
    pub prompt: String,
    pub template: ReferenceSheetTemplateId,
    pub width: u32,
    pub height: u32,
    pub chroma_color: Rgba,
    #[serde(default)]
    pub references: Vec<ReferenceSlot>,
    pub combinations: Vec<ModelQualityPair>,
    pub candidate_count_per_combo: u8,
}

/// Arguments for vector export.
#[allow(missing_docs)]
#[derive(Debug, Deserialize)]
pub struct LibraryExportVariantAsVectorArgs {
    pub entity_id: EntityId,
    pub variant_id: SheetVariantId,
}

/// Arguments for saving a raw reference asset.
#[allow(missing_docs)]
#[derive(Debug, Deserialize)]
pub struct LibrarySaveReferenceToLibraryArgs {
    pub image: ReferenceImage,
    #[serde(default)]
    pub role: ReferenceRole,
    #[serde(default)]
    pub tags: Vec<String>,
}

/// Arguments for saving a variant as a card/swatch.
#[allow(missing_docs)]
#[derive(Debug, Deserialize)]
pub struct LibrarySaveVariantCardArgs {
    pub entity_id: EntityId,
    pub variant_id: SheetVariantId,
    pub name: String,
    #[serde(default)]
    pub style_notes: String,
}

/// Arguments for updating reference asset tags.
#[allow(missing_docs)]
#[derive(Debug, Deserialize)]
pub struct LibraryUpdateAssetTagsArgs {
    pub asset_id: AssetId,
    pub tags: Vec<String>,
}

/// Arguments for LoRA training.
#[allow(missing_docs)]
#[derive(Debug, Deserialize)]
pub struct LibraryTrainLoraArgs {
    pub name: String,
    pub kind: LoraKind,
    pub target_model: ModelId,
    pub trigger_word: String,
    pub training_data: Vec<AssetId>,
}

/// Arguments for project operation model preferences.
#[allow(missing_docs)]
#[derive(Debug, Deserialize)]
pub struct ProjectSetOperationModelPrefArgs {
    pub operation: OperationKind,
    pub model: ModelId,
}

struct SheetRequestLease {
    manager: Arc<crate::state::ReferenceSheetRequestManager>,
    request_id: RequestId,
    entity_id: EntityId,
    counts_toward_sprite_cap: bool,
    chat_variant_id: Option<SheetVariantId>,
}

impl Drop for SheetRequestLease {
    fn drop(&mut self) {
        self.manager.finish(
            self.request_id,
            self.entity_id.get(),
            self.counts_toward_sprite_cap,
            self.chat_variant_id.map(SheetVariantId::get),
        );
    }
}

#[derive(Clone)]
struct SheetProviderRequest {
    operation: OperationKind,
    model: ModelId,
    quality: Quality,
    prompt: String,
    template: ReferenceSheetTemplateId,
    width: u32,
    height: u32,
    chroma_color: Rgba,
    references: Vec<ReferenceSlot>,
    source_image: Option<ReferenceImage>,
    mask: Option<ReferenceImage>,
    candidate_count: u8,
    applied_lora: Option<LoraAsset>,
    lora_weight: f32,
    real_world_grounding: bool,
}

#[derive(Clone)]
struct VariantBuildSpec {
    origin: VariantOrigin,
    parent_variant_id: Option<SheetVariantId>,
    refinement: Option<RefinementKind>,
    promotion: bool,
    chat_transcript: Option<ChatTranscript>,
    applied_lora: Option<AssetId>,
    lora_weight: f32,
    real_world_grounding: bool,
}

#[derive(Clone)]
struct VariantCommitSpec {
    provider: SheetProviderRequest,
    build: VariantBuildSpec,
    stream_index: u8,
}

#[derive(Clone)]
struct GeneratedSheetImage {
    image: ReferenceImage,
    model: ModelId,
    cost_usd: Option<f64>,
}

fn start_sheet_request_with_lease(
    state: &AppState,
    entity_id: EntityId,
    counts_toward_sprite_cap: bool,
    chat_variant_id: Option<SheetVariantId>,
) -> CommandResult<(RequestId, CancellationToken, SheetRequestLease)> {
    let (request_id, token) = state
        .reference_sheet_requests
        .start(
            entity_id.get(),
            counts_toward_sprite_cap,
            chat_variant_id.map(SheetVariantId::get),
        )
        .map_err(|detail| AppCommandError::Validation {
            detail: detail.into(),
        })?;
    let lease = SheetRequestLease {
        manager: state.reference_sheet_requests.clone(),
        request_id,
        entity_id,
        counts_toward_sprite_cap,
        chat_variant_id,
    };
    Ok((request_id, token, lease))
}

fn find_variant(sheet: &ReferenceSheet, variant_id: SheetVariantId) -> Option<&SheetVariant> {
    sheet
        .canonical
        .as_ref()
        .filter(|variant| variant.id == variant_id)
        .or_else(|| {
            sheet
                .variants
                .iter()
                .find(|variant| variant.id == variant_id)
        })
}

fn emit_sheet_event<T: Serialize>(app: &AppHandle, event: &str, payload: &T) {
    if let Err(err) = app.emit(event, payload) {
        tracing::warn!(event, error = %err, "failed to emit reference-sheet event");
    }
}

fn emit_sheet_error(app: &AppHandle, request_id: RequestId, error: AppCommandError) {
    emit_sheet_event(
        app,
        SHEET_REQUEST_ERROR_EVENT,
        &SheetRequestErrorPayload { request_id, error },
    );
}

fn emit_sheet_cancelled(app: &AppHandle, request_id: RequestId) {
    emit_sheet_event(
        app,
        SHEET_REQUEST_CANCELLED_EVENT,
        &SheetRequestCancelledPayload { request_id },
    );
}

fn elapsed_ms_since(started: SystemTime) -> u32 {
    started
        .elapsed()
        .map_or(0, |elapsed| u32::try_from(elapsed.as_millis()).unwrap_or(u32::MAX))
}

fn pixel_data_to_reference_image(pixels: &PixelData) -> Option<ReferenceImage> {
    if pixels.bytes_per_pixel != 4 || pixels.width == 0 || pixels.height == 0 {
        return None;
    }
    let row_len = usize::try_from(pixels.width).ok()?.checked_mul(4)?;
    let stride = usize::try_from(pixels.stride).ok()?;
    if stride < row_len {
        return None;
    }
    let mut packed = Vec::with_capacity(row_len.checked_mul(usize::try_from(pixels.height).ok()?)?);
    for y in 0..usize::try_from(pixels.height).ok()? {
        let start = y.checked_mul(stride)?;
        let end = start.checked_add(row_len)?;
        packed.extend_from_slice(pixels.bytes.get(start..end)?);
    }
    let image = ImageBuffer::<ImageRgba<u8>, Vec<u8>>::from_raw(pixels.width, pixels.height, packed)?;
    let bytes = encode_rgba_png(&image).ok()?;
    Some(ReferenceImage {
        bytes,
        mime: "image/png".into(),
    })
}

fn spawn_provider_progress_bridge(
    app: AppHandle,
    request_id: RequestId,
    stream_index: u8,
    started: SystemTime,
    mut progress_rx: tokio::sync::mpsc::Receiver<VerbProgressEvent>,
) {
    tauri::async_runtime::spawn(async move {
        let mut partial_index: u8 = 0;
        while let Some(event) = progress_rx.recv().await {
            match event {
                VerbProgressEvent::PartialPixels {
                    effect_index,
                    pixels,
                } => {
                    partial_index = partial_index.saturating_add(1);
                    emit_sheet_event(
                        &app,
                        SHEET_REQUEST_PROGRESS_EVENT,
                        &SheetRequestProgressPayload {
                            request_id,
                            stream_index,
                            candidate_index: u8::try_from(effect_index).unwrap_or(u8::MAX),
                            partial_index,
                            partial_image: pixel_data_to_reference_image(&pixels),
                            elapsed_ms: elapsed_ms_since(started),
                        },
                    );
                }
                VerbProgressEvent::Started { .. }
                | VerbProgressEvent::Step { .. }
                | VerbProgressEvent::Cost(_)
                | VerbProgressEvent::Log { .. }
                | VerbProgressEvent::Eta { .. } => {
                    emit_sheet_event(
                        &app,
                        SHEET_REQUEST_PROGRESS_EVENT,
                        &SheetRequestProgressPayload {
                            request_id,
                            stream_index,
                            candidate_index: 0,
                            partial_index,
                            partial_image: None,
                            elapsed_ms: elapsed_ms_since(started),
                        },
                    );
                }
            }
        }
    });
}

fn emit_training_event<T: Serialize>(app: &AppHandle, event: &str, payload: &T) {
    if let Err(err) = app.emit(event, payload) {
        tracing::warn!(event, error = %err, "failed to emit training event");
    }
}

fn model_provider(model: ModelId) -> &'static str {
    match model {
        ModelId::Auto | ModelId::OpenAiGptImage2 => "openai",
        ModelId::GoogleNanoBananaPro | ModelId::GoogleGeminiFlashImage => "google_ai",
        ModelId::FalFluxKontext
        | ModelId::FalFluxDev
        | ModelId::FalRecraftVectorize
        | ModelId::FalRealEsrgan => "fal",
    }
}

fn is_openai_model(model: ModelId) -> bool {
    matches!(model, ModelId::Auto | ModelId::OpenAiGptImage2)
}

fn model_label(model: ModelId) -> &'static str {
    match model {
        ModelId::Auto => "auto",
        ModelId::OpenAiGptImage2 => "gpt-image-2",
        ModelId::GoogleNanoBananaPro => "gemini-3-pro-image-preview",
        ModelId::GoogleGeminiFlashImage => "gemini-3.1-flash-image-preview",
        ModelId::FalFluxKontext => "fal-ai/flux-pro/kontext",
        ModelId::FalFluxDev => "fal-ai/flux-lora",
        ModelId::FalRecraftVectorize => "fal-ai/recraft/vectorize",
        ModelId::FalRealEsrgan => "fal-ai/real-esrgan",
    }
}

fn image_quality(quality: Quality) -> ImageQuality {
    match quality {
        Quality::Auto => ImageQuality::Auto,
        Quality::Low => ImageQuality::Low,
        Quality::Medium => ImageQuality::Medium,
        Quality::High => ImageQuality::High,
    }
}

fn chroma_hex(color: Rgba) -> String {
    format!("#{:02X}{:02X}{:02X}", color.r, color.g, color.b)
}

fn reference_role_name(role: ReferenceRole) -> &'static str {
    match role {
        ReferenceRole::Subject => "subject",
        ReferenceRole::Style => "style",
        ReferenceRole::Pose => "pose",
        ReferenceRole::Outfit => "outfit",
        ReferenceRole::Context => "context",
        ReferenceRole::Generic => "generic",
    }
}

fn reference_data_uri(image: &ReferenceImage) -> String {
    format!(
        "data:{};base64,{}",
        image.mime,
        base64::engine::general_purpose::STANDARD.encode(&image.bytes)
    )
}

fn compose_sheet_prompt(request: &SheetProviderRequest, style_notes: &str) -> String {
    let mut parts = Vec::new();
    if !style_notes.trim().is_empty() {
        parts.push(format!("Project style notes:\n{}", style_notes.trim()));
    }
    parts.push(format!(
        "Create a sprite reference sheet using template {:?} at {}x{}.",
        request.template, request.width, request.height
    ));
    parts.push(format!(
        "Background must be a flat solid chroma key color {} with no shadows, gradients, or background props.",
        chroma_hex(request.chroma_color)
    ));
    if !request.references.is_empty() {
        let hints = request
            .references
            .iter()
            .enumerate()
            .map(|(i, slot)| {
                format!(
                    "Reference {} is {} guidance with weight {:.2}.",
                    i + 1,
                    reference_role_name(slot.role),
                    slot.weight
                )
            })
            .collect::<Vec<_>>()
            .join(" ");
        parts.push(hints);
    }
    if request.real_world_grounding
        && matches!(
            request.model,
            ModelId::GoogleNanoBananaPro | ModelId::GoogleGeminiFlashImage
        )
    {
        parts.push("Use accurate real-world references for named places, objects, and scenes when composing this image.".into());
    }
    if let Some(lora) = &request.applied_lora {
        parts.push(format!(
            "Apply the Flux LoRA trigger word `{}` at weight {:.2}.",
            lora.trigger_word, request.lora_weight
        ));
    }
    match request.operation {
        OperationKind::MaskedRefinement | OperationKind::RegionalRefinement => {
            parts.push("Preserve everything outside the edited region.".into());
        }
        OperationKind::PromptOnlyRefinement | OperationKind::ChatTurn => {
            parts.push("Preserve the character identity, proportions, palette, and sheet layout unless the user specifically asks to change them.".into());
        }
        OperationKind::Promotion => {
            parts.push(
                "Re-render this approved direction as a polished final reference sheet.".into(),
            );
        }
        _ => {}
    }
    parts.push(request.prompt.trim().to_owned());
    parts.join("\n\n")
}

fn selected_model(
    explicit: Option<ModelId>,
    operation: OperationKind,
    project: &pixhaus_core::project::Project,
) -> ModelId {
    explicit
        .filter(|model| *model != ModelId::Auto)
        .or_else(|| {
            project
                .library
                .ai
                .per_operation_model_prefs
                .get(&operation)
                .copied()
                .filter(|model| *model != ModelId::Auto)
        })
        .unwrap_or_else(|| match operation {
            OperationKind::FreshGeneration => ModelId::GoogleNanoBananaPro,
            OperationKind::MaskedRefinement
            | OperationKind::RegionalRefinement
            | OperationKind::Promotion => ModelId::OpenAiGptImage2,
            OperationKind::PromptOnlyRefinement | OperationKind::ChatTurn => {
                ModelId::GoogleNanoBananaPro
            }
            OperationKind::CrossModelGrid => ModelId::OpenAiGptImage2,
            OperationKind::VectorExport => ModelId::FalRecraftVectorize,
            OperationKind::Upscale => ModelId::FalRealEsrgan,
            OperationKind::LoraTraining => ModelId::FalFluxDev,
        })
}

fn find_lora_asset(
    project: &pixhaus_core::project::Project,
    asset_id: Option<AssetId>,
) -> Option<LoraAsset> {
    asset_id.and_then(|asset_id| {
        project
            .library
            .ai
            .asset_library
            .loras
            .iter()
            .find(|asset| asset.id == asset_id)
            .cloned()
    })
}

fn configured_execution_model(preferred: ModelId, state: &AppState) -> ModelId {
    if is_openai_model(preferred) {
        return preferred;
    }
    let backends = state.verb_runtime.list_backends();
    let preferred_ready = backends
        .iter()
        .any(|backend| backend.id == model_provider(preferred) && backend.available);
    if preferred_ready {
        return preferred;
    }
    let openai_ready = backends
        .iter()
        .any(|backend| backend.id == "openai" && backend.available);
    if openai_ready {
        ModelId::OpenAiGptImage2
    } else {
        preferred
    }
}

fn mint_sheet_variant_id(next_id: &mut u32) -> SheetVariantId {
    let id = SheetVariantId::new(*next_id);
    *next_id += 1;
    id
}

fn mint_asset_id(next_id: &mut u32) -> AssetId {
    let id = AssetId::new(*next_id);
    *next_id += 1;
    id
}

async fn invoke_provider_images(
    app: AppHandle,
    request_id: RequestId,
    stream_index: u8,
    started: SystemTime,
    runtime: Arc<VerbRuntime>,
    request: &SheetProviderRequest,
    style_notes: &str,
    cancel: CancellationToken,
) -> CommandResult<Vec<GeneratedSheetImage>> {
    if cancel.is_cancelled() {
        return Err(AppCommandError::VerbError {
            message: "reference-sheet request cancelled".into(),
        });
    }
    let capability = if request.source_image.is_some() {
        if request.mask.is_some() {
            BackendCapabilities::IMAGE_INPAINT
        } else {
            BackendCapabilities::IMAGE_EDIT
        }
    } else {
        BackendCapabilities::IMAGE_GENERATION
    };
    let backend_id = model_provider(request.model);
    let backend = runtime
        .select_backend_by_id(
            backend_id,
            capability,
            &VerbId::new("pixhaus.reference_sheet.dispatch"),
        )
        .map_err(|err| AppCommandError::VerbError {
            message: err.to_string(),
        })?;
    let proxy = backend
        .as_any()
        .downcast_ref::<BackendProxy>()
        .ok_or_else(|| AppCommandError::VerbError {
            message: "selected backend does not expose the image execution surface".into(),
        })?;

    let prompt = compose_sheet_prompt(request, style_notes);
    let model = if request.model == ModelId::Auto {
        None
    } else {
        Some(model_label(request.model).to_owned())
    };
    let (progress, progress_rx) = VerbProgress::channel();
    spawn_provider_progress_bridge(app, request_id, stream_index, started, progress_rx);
    let reference_images = request
        .references
        .iter()
        .map(|slot| slot.image.bytes.clone())
        .collect::<Vec<_>>();
    let response = if backend_id == "fal" {
        proxy
            .fat()
            .invoke(
                InferenceRequest::Replicate(pixhaus_ai::backends::ReplicateRequest {
                    model: model_label(request.model).into(),
                    version: None,
                    input: build_fal_sheet_input(request, &prompt),
                }),
                progress,
                cancel,
            )
            .await
            .map_err(|err| AppCommandError::VerbError {
                message: err.to_string(),
            })?
    } else if let Some(source) = &request.source_image {
        let edit = ImageEditRequest {
            model,
            image: source.bytes.clone(),
            mask: request.mask.as_ref().map(|mask| mask.bytes.clone()),
            prompt,
            negative_prompt: None,
            num_images: u32::from(request.candidate_count.clamp(1, 4)),
            style_image: request
                .references
                .first()
                .map(|slot| slot.image.bytes.clone()),
            reference_images,
        };
        let req = if request.mask.is_some() {
            InferenceRequest::ImageInpaint(edit)
        } else {
            InferenceRequest::ImageEdit(edit)
        };
        proxy
            .fat()
            .invoke(req, progress, cancel)
            .await
            .map_err(|err| AppCommandError::VerbError {
                message: err.to_string(),
            })?
    } else {
        let image_gen = ImageGenRequest {
            model,
            prompt,
            negative_prompt: None,
            width: request.width,
            height: request.height,
            steps: None,
            seed: None,
            num_images: u32::from(request.candidate_count.clamp(1, 4)),
            quality: Some(image_quality(request.quality)),
            style_image: request
                .references
                .first()
                .map(|slot| slot.image.bytes.clone()),
            reference_images,
        };
        proxy
            .fat()
            .invoke(
                InferenceRequest::ImageGeneration(image_gen),
                progress,
                cancel,
            )
            .await
            .map_err(|err| AppCommandError::VerbError {
                message: err.to_string(),
            })?
    };

    let images = match response {
        InferenceResponse::Image(output) => output.images,
        _ => {
            return Err(AppCommandError::VerbError {
                message: "provider returned a non-image response for reference-sheet operation"
                    .into(),
            });
        }
    };
    if images.is_empty() {
        return Err(AppCommandError::VerbError {
            message: "provider returned zero reference-sheet images".into(),
        });
    }
    Ok(images
        .into_iter()
        .map(|bytes| GeneratedSheetImage {
            image: ReferenceImage {
                bytes,
                mime: "image/png".into(),
            },
            model: request.model,
            cost_usd: None,
        })
        .collect())
}

fn region_bounds(region: &pixhaus_core::project::RegionDefinition) -> Option<(u32, u32, u32, u32)> {
    let min_x = region.polygon.iter().map(|p| p.x).min()?;
    let min_y = region.polygon.iter().map(|p| p.y).min()?;
    let max_x = region.polygon.iter().map(|p| p.x).max()?;
    let max_y = region.polygon.iter().map(|p| p.y).max()?;
    if max_x <= min_x || max_y <= min_y {
        return None;
    }
    Some((
        u32::try_from(min_x.max(0)).unwrap_or(0),
        u32::try_from(min_y.max(0)).unwrap_or(0),
        u32::try_from(max_x.max(0)).unwrap_or(0),
        u32::try_from(max_y.max(0)).unwrap_or(0),
    ))
}

fn validate_non_overlapping_regions(
    regions: &[pixhaus_core::project::RegionDefinition],
) -> CommandResult<Vec<(u32, u32, u32, u32)>> {
    let mut bounds = Vec::with_capacity(regions.len());
    for region in regions {
        let Some(next) = region_bounds(region) else {
            return Err(AppCommandError::Validation {
                detail: "regional refinement region must have a non-empty polygon".into(),
            });
        };
        if bounds.iter().any(|existing| rects_overlap(*existing, next)) {
            return Err(AppCommandError::Validation {
                detail: "regional refinement regions must not overlap".into(),
            });
        }
        bounds.push(next);
    }
    Ok(bounds)
}

fn rects_overlap(a: (u32, u32, u32, u32), b: (u32, u32, u32, u32)) -> bool {
    let (ax0, ay0, ax1, ay1) = a;
    let (bx0, by0, bx1, by1) = b;
    ax0 < bx1 && ax1 > bx0 && ay0 < by1 && ay1 > by0
}

fn region_mask_png(
    width: u32,
    height: u32,
    bounds: (u32, u32, u32, u32),
) -> CommandResult<ReferenceImage> {
    let (x0, y0, x1, y1) = bounds;
    let mut mask: ImageBuffer<ImageRgba<u8>, Vec<u8>> =
        ImageBuffer::from_pixel(width, height, ImageRgba([0, 0, 0, 255]));
    let clipped_x1 = x1.min(width);
    let clipped_y1 = y1.min(height);
    for y in y0.min(height)..clipped_y1 {
        for x in x0.min(width)..clipped_x1 {
            mask.put_pixel(x, y, ImageRgba([255, 255, 255, 255]));
        }
    }
    let mut bytes = Vec::new();
    mask.write_to(&mut Cursor::new(&mut bytes), ImageFormat::Png)
        .map_err(|err| AppCommandError::Validation {
            detail: format!("failed to encode regional mask: {err}"),
        })?;
    Ok(ReferenceImage {
        bytes,
        mime: "image/png".into(),
    })
}

fn composite_region(
    base: &mut ImageBuffer<ImageRgba<u8>, Vec<u8>>,
    edited: &[u8],
    bounds: (u32, u32, u32, u32),
) -> CommandResult<()> {
    let edited = image::load_from_memory(edited)
        .map_err(|err| AppCommandError::Validation {
            detail: format!("failed to decode regional refinement output: {err}"),
        })?
        .into_rgba8();
    let (x0, y0, x1, y1) = bounds;
    let width = base.width().min(edited.width());
    let height = base.height().min(edited.height());
    for y in y0.min(height)..y1.min(height) {
        for x in x0.min(width)..x1.min(width) {
            let pixel = *edited.get_pixel(x, y);
            base.put_pixel(x, y, pixel);
        }
    }
    Ok(())
}

fn encode_rgba_png(image: &ImageBuffer<ImageRgba<u8>, Vec<u8>>) -> CommandResult<Vec<u8>> {
    let mut bytes = Vec::new();
    image
        .write_to(&mut Cursor::new(&mut bytes), ImageFormat::Png)
        .map_err(|err| AppCommandError::Validation {
            detail: format!("failed to encode regional composite: {err}"),
        })?;
    Ok(bytes)
}

fn encode_reference_assets_zip(assets: &[ReferenceAsset]) -> CommandResult<Vec<u8>> {
    let buf = Cursor::new(Vec::new());
    let mut zip = zip::ZipWriter::new(buf);
    let options = zip::write::SimpleFileOptions::default()
        .compression_method(zip::CompressionMethod::Deflated);
    for (i, asset) in assets.iter().enumerate() {
        let ext = if asset.image.mime.contains("jpeg") || asset.image.mime.contains("jpg") {
            "jpg"
        } else if asset.image.mime.contains("webp") {
            "webp"
        } else {
            "png"
        };
        zip.start_file(format!("{i:04}.{ext}"), options)
            .map_err(|err| AppCommandError::Validation {
                detail: format!("failed to create LoRA training archive: {err}"),
            })?;
        zip.write_all(&asset.image.bytes)
            .map_err(|err| AppCommandError::Validation {
                detail: format!("failed to write LoRA training archive: {err}"),
            })?;
    }
    zip.finish()
        .map(|buf| buf.into_inner())
        .map_err(|err| AppCommandError::Validation {
            detail: format!("failed to finish LoRA training archive: {err}"),
        })
}

fn build_fal_sheet_input(request: &SheetProviderRequest, prompt: &str) -> serde_json::Value {
    let mut input = serde_json::json!({
        "prompt": prompt,
        "num_images": request.candidate_count.clamp(1, 4),
        "sync_mode": true,
    });
    if request.source_image.is_none() {
        input["image_size"] = serde_json::json!({
            "width": request.width,
            "height": request.height,
        });
    }
    if let Some(source) = &request.source_image {
        input["image_url"] = serde_json::json!(reference_data_uri(source));
    }
    if let Some(mask) = &request.mask {
        input["mask_url"] = serde_json::json!(reference_data_uri(mask));
    }
    let reference_urls = request
        .references
        .iter()
        .map(|slot| reference_data_uri(&slot.image))
        .collect::<Vec<_>>();
    if let Some(first) = reference_urls.first() {
        input["reference_image_url"] = serde_json::json!(first);
        input["image_url"] = input
            .get("image_url")
            .cloned()
            .unwrap_or_else(|| serde_json::json!(first));
    }
    if !reference_urls.is_empty() {
        input["reference_image_urls"] = serde_json::json!(reference_urls);
    }
    if let Some(lora) = &request.applied_lora {
        input["loras"] = serde_json::json!([
            {
                "path": lora.fal_lora_url,
                "scale": request.lora_weight,
            }
        ]);
        input["lora_url"] = serde_json::json!(lora.fal_lora_url);
        input["lora_scale"] = serde_json::json!(request.lora_weight);
    }
    input
}

async fn invoke_regional_refinement(
    app: AppHandle,
    request_id: RequestId,
    stream_index: u8,
    started: SystemTime,
    runtime: Arc<VerbRuntime>,
    source: &SheetVariant,
    request: &SheetProviderRequest,
    regions: &[pixhaus_core::project::RegionDefinition],
    style_notes: &str,
    cancel: CancellationToken,
) -> CommandResult<Vec<GeneratedSheetImage>> {
    let bounds = validate_non_overlapping_regions(regions)?;
    let mut outputs = Vec::new();
    for _ in 0..request.candidate_count.clamp(1, 4) {
        let mut composite = image::load_from_memory(&source.image.bytes)
            .map_err(|err| AppCommandError::Validation {
                detail: format!("failed to decode source sheet for regional refinement: {err}"),
            })?
            .into_rgba8();
        for (region, bounds) in regions.iter().zip(bounds.iter().copied()) {
            let mask = region_mask_png(source.width, source.height, bounds)?;
            let mut refs = request.references.clone();
            refs.extend(region.region_references.clone());
            let mut region_request = request.clone();
            region_request.prompt = format!(
                "{}\n\nRegion instruction: {}",
                request.prompt, region.prompt
            );
            region_request.references = refs;
            region_request.source_image = Some(ReferenceImage {
                bytes: encode_rgba_png(&composite)?,
                mime: "image/png".into(),
            });
            region_request.mask = Some(mask);
            region_request.candidate_count = 1;
            let images = invoke_provider_images(
                app.clone(),
                request_id,
                stream_index,
                started,
                runtime.clone(),
                &region_request,
                style_notes,
                cancel.clone(),
            )
            .await?;
            if cancel.is_cancelled() {
                return Err(AppCommandError::VerbError {
                    message: "reference-sheet request cancelled".into(),
                });
            }
            let Some(image) = images.into_iter().next() else {
                return Err(AppCommandError::VerbError {
                    message: "regional refinement provider returned no image".into(),
                });
            };
            composite_region(&mut composite, &image.image.bytes, bounds)?;
        }
        outputs.push(GeneratedSheetImage {
            image: ReferenceImage {
                bytes: encode_rgba_png(&composite)?,
                mime: "image/png".into(),
            },
            model: request.model,
            cost_usd: None,
        });
    }
    Ok(outputs)
}

async fn invoke_vector_export(
    runtime: Arc<VerbRuntime>,
    image: &ReferenceImage,
    cancel: CancellationToken,
) -> CommandResult<ReferenceImage> {
    let backend = runtime
        .select_backend_by_id(
            "fal",
            BackendCapabilities::IMAGE_GENERATION,
            &VerbId::new("pixhaus.reference_sheet.vector_export"),
        )
        .map_err(|err| AppCommandError::VerbError {
            message: err.to_string(),
        })?;
    let proxy = backend
        .as_any()
        .downcast_ref::<BackendProxy>()
        .ok_or_else(|| AppCommandError::VerbError {
            message: "selected fal backend does not expose the image execution surface".into(),
        })?;
    let data_uri = format!(
        "data:{};base64,{}",
        image.mime,
        base64::engine::general_purpose::STANDARD.encode(&image.bytes)
    );
    let response = proxy
        .fat()
        .invoke(
            InferenceRequest::Replicate(pixhaus_ai::backends::ReplicateRequest {
                model: model_label(ModelId::FalRecraftVectorize).into(),
                version: None,
                input: serde_json::json!({ "image_url": data_uri }),
            }),
            VerbProgress::discard(),
            cancel,
        )
        .await
        .map_err(|err| AppCommandError::VerbError {
            message: err.to_string(),
        })?;
    let InferenceResponse::Image(output) = response else {
        return Err(AppCommandError::VerbError {
            message: "fal vector export returned a non-image response".into(),
        });
    };
    let Some(bytes) = output.images.into_iter().next() else {
        return Err(AppCommandError::VerbError {
            message: "fal vector export returned no SVG".into(),
        });
    };
    Ok(ReferenceImage {
        bytes,
        mime: "image/svg+xml".into(),
    })
}

fn build_variant_from_provider_image(
    id: SheetVariantId,
    ts: i64,
    image: GeneratedSheetImage,
    spec: &VariantCommitSpec,
) -> SheetVariant {
    let mut variant = SheetVariant::from_image(id, ts, image.image);
    variant.template = spec.provider.template;
    variant.width = spec.provider.width;
    variant.height = spec.provider.height;
    variant.chroma_color = spec.provider.chroma_color;
    variant.user_prompt = spec.provider.prompt.clone();
    variant.composed_prompt = compose_sheet_prompt(&spec.provider, "");
    variant.references = spec.provider.references.clone();
    variant.model = image.model;
    variant.quality = spec.provider.quality;
    variant.parent_variant_id = spec.build.parent_variant_id;
    variant.origin = spec.build.origin;
    variant.refinement = spec.build.refinement.clone();
    variant.chat_transcript = spec.build.chat_transcript.clone().map(|mut transcript| {
        if let Some(last) = transcript.turns.last_mut() {
            last.resulting_variant_id = id;
        }
        transcript
    });
    variant.promotion = spec.build.promotion;
    variant.applied_lora = spec.build.applied_lora;
    variant.lora_weight = spec.build.lora_weight;
    variant.real_world_grounding = spec.build.real_world_grounding;
    variant.cost_usd = image.cost_usd;
    variant.extracted_palette =
        extract_palette_from_image_bytes(&variant.image.bytes, ExtractionOptions::default())
            .unwrap_or_default();
    variant
}

fn embedded_reference_sheet_or_create_mut(
    entity: &mut Entity,
) -> Result<&mut ReferenceSheet, AppCommandError> {
    match &mut entity.content {
        EntityContent::Sprites {
            reference_sheet, ..
        } => Ok(reference_sheet
            .get_or_insert_with(|| {
                Box::new(ReferenceSheet {
                    canonical: None,
                    variants: Vec::new(),
                    prompts: Vec::new(),
                    info: AssetInfo::default(),
                })
            })
            .as_mut()),
        _ => Err(AppCommandError::Validation {
            detail: format!(
                "entity {} is not a sprite entity; reference sheets belong to sprites",
                entity.id.get()
            ),
        }),
    }
}

async fn commit_provider_variants(
    app: &AppHandle,
    request_id: RequestId,
    entity_id: EntityId,
    specs: Vec<(VariantCommitSpec, Vec<GeneratedSheetImage>)>,
) -> CommandResult<Entity> {
    let ts = now_secs();
    let state = app.state::<AppState>();
    let mut doc = state.doc.write().await;
    let mut next_id = doc.next_id;
    let project = doc
        .project
        .as_mut()
        .ok_or(AppCommandError::NoActiveProject)?;
    let entity = project
        .library
        .entities
        .iter_mut()
        .find(|e| e.id == entity_id)
        .ok_or_else(|| AppCommandError::NotFound {
            entity: "entity".into(),
            id: u64::from(entity_id.get()),
        })?;
    let sheet = embedded_reference_sheet_or_create_mut(entity)?;

    let mut new_variants = Vec::new();
    for (spec, images) in specs {
        for (candidate_index, image) in images.into_iter().enumerate() {
            let variant = build_variant_from_provider_image(
                mint_sheet_variant_id(&mut next_id),
                ts,
                image,
                &spec,
            );
            emit_sheet_event(
                app,
                SHEET_REQUEST_CANDIDATE_COMPLETE_EVENT,
                &SheetRequestCandidateCompletePayload {
                    request_id,
                    stream_index: spec.stream_index,
                    candidate_index: u8::try_from(candidate_index).unwrap_or(u8::MAX),
                    variant: variant.clone(),
                },
            );
            new_variants.push(variant);
        }
    }
    new_variants.append(&mut sheet.variants);
    sheet.variants = new_variants;
    entity.updated_at = ts;
    let updated = entity.clone();
    doc.next_id = next_id;
    doc.dirty = true;
    drop(doc);

    state.anchor_cache.remove(&entity_id.get());
    emit_sheet_event(
        app,
        SHEET_REQUEST_COMPLETE_EVENT,
        &SheetRequestCompletePayload {
            request_id,
            entity_id,
            sprite: updated.clone(),
            total_cost_usd: 0.0,
        },
    );
    Ok(updated)
}

fn spawn_sheet_provider_job(
    app: AppHandle,
    request_id: RequestId,
    entity_id: EntityId,
    token: CancellationToken,
    lease: SheetRequestLease,
    style_notes: String,
    spec: VariantCommitSpec,
) {
    tauri::async_runtime::spawn(async move {
        let _lease = lease;
        let started = SystemTime::now();
        if token.is_cancelled() {
            emit_sheet_cancelled(&app, request_id);
            return;
        }
        let _permit = app
            .state::<AppState>()
            .reference_sheet_requests
            .acquire_sprite_permit(entity_id.get(), true, &token)
            .await;
        if token.is_cancelled() {
            emit_sheet_cancelled(&app, request_id);
            return;
        }
        emit_sheet_event(
            &app,
            SHEET_REQUEST_PROGRESS_EVENT,
            &SheetRequestProgressPayload {
                request_id,
                stream_index: spec.stream_index,
                candidate_index: 0,
                partial_index: 0,
                partial_image: None,
                elapsed_ms: 0,
            },
        );
        let runtime = app.state::<AppState>().verb_runtime.clone();
        let result = invoke_provider_images(
            app.clone(),
            request_id,
            spec.stream_index,
            started,
            runtime,
            &spec.provider,
            &style_notes,
            token.clone(),
        )
        .await;
        if token.is_cancelled() {
            emit_sheet_cancelled(&app, request_id);
            return;
        }
        match result {
            Ok(images) => {
                let elapsed_ms = started.elapsed().map_or(0, |elapsed| {
                    u32::try_from(elapsed.as_millis()).unwrap_or(u32::MAX)
                });
                emit_sheet_event(
                    &app,
                    SHEET_REQUEST_PROGRESS_EVENT,
                    &SheetRequestProgressPayload {
                        request_id,
                        stream_index: spec.stream_index,
                        candidate_index: 0,
                        partial_index: 1,
                        partial_image: None,
                        elapsed_ms,
                    },
                );
                if let Err(err) =
                    commit_provider_variants(&app, request_id, entity_id, vec![(spec, images)])
                        .await
                {
                    emit_sheet_error(&app, request_id, err);
                }
            }
            Err(err) => emit_sheet_error(&app, request_id, err),
        }
    });
}

fn spawn_regional_sheet_job(
    app: AppHandle,
    request_id: RequestId,
    entity_id: EntityId,
    token: CancellationToken,
    lease: SheetRequestLease,
    style_notes: String,
    source: SheetVariant,
    spec: VariantCommitSpec,
    regions: Vec<pixhaus_core::project::RegionDefinition>,
) {
    tauri::async_runtime::spawn(async move {
        let _lease = lease;
        let started = SystemTime::now();
        if token.is_cancelled() {
            emit_sheet_cancelled(&app, request_id);
            return;
        }
        let _permit = app
            .state::<AppState>()
            .reference_sheet_requests
            .acquire_sprite_permit(entity_id.get(), true, &token)
            .await;
        if token.is_cancelled() {
            emit_sheet_cancelled(&app, request_id);
            return;
        }
        let runtime = app.state::<AppState>().verb_runtime.clone();
        let result = invoke_regional_refinement(
            app.clone(),
            request_id,
            spec.stream_index,
            started,
            runtime,
            &source,
            &spec.provider,
            &regions,
            &style_notes,
            token.clone(),
        )
        .await;
        if token.is_cancelled() {
            emit_sheet_cancelled(&app, request_id);
            return;
        }
        match result {
            Ok(images) => {
                if let Err(err) =
                    commit_provider_variants(&app, request_id, entity_id, vec![(spec, images)])
                        .await
                {
                    emit_sheet_error(&app, request_id, err);
                }
            }
            Err(err) => emit_sheet_error(&app, request_id, err),
        }
    });
}

fn spawn_cross_model_job(
    app: AppHandle,
    request_id: RequestId,
    entity_id: EntityId,
    token: CancellationToken,
    lease: SheetRequestLease,
    style_notes: String,
    specs: Vec<VariantCommitSpec>,
) {
    tauri::async_runtime::spawn(async move {
        let _lease = lease;
        let started = SystemTime::now();
        let runtime = app.state::<AppState>().verb_runtime.clone();
        let mut tasks = tokio::task::JoinSet::new();
        for spec in specs {
            emit_sheet_event(
                &app,
                SHEET_REQUEST_PROGRESS_EVENT,
                &SheetRequestProgressPayload {
                    request_id,
                    stream_index: spec.stream_index,
                    candidate_index: 0,
                    partial_index: 0,
                    partial_image: None,
                    elapsed_ms: 0,
                },
            );
            let runtime = runtime.clone();
            let style_notes = style_notes.clone();
            let token = token.clone();
            let app_for_task = app.clone();
            tasks.spawn(async move {
                let result = invoke_provider_images(
                    app_for_task,
                    request_id,
                    spec.stream_index,
                    started,
                    runtime,
                    &spec.provider,
                    &style_notes,
                    token,
                )
                .await;
                (spec, result)
            });
        }
        let mut completed = Vec::new();
        while let Some(joined) = tasks.join_next().await {
            if token.is_cancelled() {
                tasks.abort_all();
                emit_sheet_cancelled(&app, request_id);
                return;
            }
            let (spec, result) = match joined {
                Ok(result) => result,
                Err(err) => {
                    emit_sheet_error(
                        &app,
                        request_id,
                        AppCommandError::VerbError {
                            message: format!("cross-model stream task failed: {err}"),
                        },
                    );
                    continue;
                }
            };
            match result {
                Ok(images) => completed.push((spec, images)),
                Err(err) => {
                    emit_sheet_error(&app, request_id, err);
                }
            }
        }
        if completed.is_empty() {
            emit_sheet_error(
                &app,
                request_id,
                AppCommandError::VerbError {
                    message: "cross-model grid produced no successful candidates".into(),
                },
            );
            return;
        }
        if let Err(err) = commit_provider_variants(&app, request_id, entity_id, completed).await {
            emit_sheet_error(&app, request_id, err);
        }
    });
}

/// Built-in reference-sheet templates for the UI.
#[tauri::command(async, rename_all = "snake_case")]
pub async fn library_list_reference_sheet_templates()
-> CommandResult<Vec<ReferenceSheetTemplateDefinition>> {
    Ok(built_in_reference_sheet_templates())
}

/// Cancels a reference-sheet request if it is still in flight.
#[tauri::command(async, rename_all = "snake_case")]
pub async fn library_cancel_reference_sheet_request(
    request_id: RequestId,
    state: State<'_, AppState>,
) -> CommandResult<()> {
    if state.reference_sheet_requests.cancel(request_id) {
        Ok(())
    } else {
        Err(AppCommandError::NotFound {
            entity: "reference sheet request".into(),
            id: request_id,
        })
    }
}

/// Refines a variant by creating one or more derived draft variants.
#[tauri::command(async, rename_all = "snake_case")]
pub async fn library_refine_reference_sheet_variant(
    args: LibraryRefineReferenceSheetVariantArgs,
    app: AppHandle,
    state: State<'_, AppState>,
) -> CommandResult<RequestId> {
    if args.prompt.trim().is_empty() {
        return Err(AppCommandError::Validation {
            detail: "refinement prompt must not be empty".into(),
        });
    }
    let (source, style_notes, model, lora_asset) = {
        let doc = state.doc.read().await;
        let project = doc
            .project
            .as_ref()
            .ok_or(AppCommandError::NoActiveProject)?;
        let entity = project
            .library
            .entities
            .iter()
            .find(|e| e.id == args.entity_id)
            .ok_or_else(|| AppCommandError::NotFound {
                entity: "entity".into(),
                id: u64::from(args.entity_id.get()),
            })?;
        let sheet = match &entity.content {
            EntityContent::Sprites {
                reference_sheet: Some(sheet),
                ..
            } => sheet.as_ref(),
            _ => {
                return Err(AppCommandError::Validation {
                    detail: "entity has no sprite reference sheet".into(),
                });
            }
        };
        let source = find_variant(sheet, args.parent_variant_id)
            .cloned()
            .ok_or_else(|| AppCommandError::NotFound {
                entity: "sheet variant".into(),
                id: u64::from(args.parent_variant_id.get()),
            })?;
        let operation = match &args.refinement {
            RefinementKind::Masked { .. } => OperationKind::MaskedRefinement,
            RefinementKind::PromptOnly => OperationKind::PromptOnlyRefinement,
            RefinementKind::Regional { .. } => OperationKind::RegionalRefinement,
        };
        let lora_asset = find_lora_asset(project, source.applied_lora);
        (
            source,
            project.library.ai.style_notes.clone(),
            configured_execution_model(selected_model(args.model, operation, project), &state),
            lora_asset,
        )
    };
    let operation = match &args.refinement {
        RefinementKind::Masked { .. } => OperationKind::MaskedRefinement,
        RefinementKind::PromptOnly => OperationKind::PromptOnlyRefinement,
        RefinementKind::Regional { .. } => OperationKind::RegionalRefinement,
    };
    let mask = match &args.refinement {
        RefinementKind::Masked { mask_png } => Some(mask_png.clone()),
        RefinementKind::PromptOnly | RefinementKind::Regional { .. } => None,
    };
    let regional_regions = match &args.refinement {
        RefinementKind::Regional { regions } => Some(regions.clone()),
        RefinementKind::Masked { .. } | RefinementKind::PromptOnly => None,
    };
    let (request_id, token, lease) =
        start_sheet_request_with_lease(&state, args.entity_id, true, None)?;
    let mut references = source.references.clone();
    references.extend(args.additional_references.clone());
    let spec = VariantCommitSpec {
        provider: SheetProviderRequest {
            operation,
            model,
            quality: args.quality,
            prompt: args.prompt,
            template: source.template,
            width: source.width,
            height: source.height,
            chroma_color: source.chroma_color,
            references,
            source_image: Some(source.image.clone()),
            mask,
            candidate_count: args.candidate_count.clamp(1, 4),
            applied_lora: lora_asset,
            lora_weight: source.lora_weight,
            real_world_grounding: source.real_world_grounding,
        },
        build: VariantBuildSpec {
            origin: VariantOrigin::Refinement,
            parent_variant_id: Some(source.id),
            refinement: Some(args.refinement),
            promotion: false,
            chat_transcript: None,
            applied_lora: source.applied_lora,
            lora_weight: source.lora_weight,
            real_world_grounding: source.real_world_grounding,
        },
        stream_index: 0,
    };
    if let Some(regions) = regional_regions {
        spawn_regional_sheet_job(
            app,
            request_id,
            args.entity_id,
            token,
            lease,
            style_notes,
            source,
            spec,
            regions,
        );
    } else {
        spawn_sheet_provider_job(
            app,
            request_id,
            args.entity_id,
            token,
            lease,
            style_notes,
            spec,
        );
    }
    Ok(request_id)
}

/// Adds a conversational edit turn and persists the resulting draft variant.
#[tauri::command(async, rename_all = "snake_case")]
pub async fn library_submit_chat_turn(
    args: LibrarySubmitChatTurnArgs,
    app: AppHandle,
    state: State<'_, AppState>,
) -> CommandResult<RequestId> {
    if args.user_message.trim().is_empty() {
        return Err(AppCommandError::Validation {
            detail: "chat message must not be empty".into(),
        });
    }
    let (source, style_notes, model, lora_asset) = {
        let doc = state.doc.read().await;
        let project = doc
            .project
            .as_ref()
            .ok_or(AppCommandError::NoActiveProject)?;
        let entity = project
            .library
            .entities
            .iter()
            .find(|e| e.id == args.entity_id)
            .ok_or_else(|| AppCommandError::NotFound {
                entity: "entity".into(),
                id: u64::from(args.entity_id.get()),
            })?;
        let sheet = match &entity.content {
            EntityContent::Sprites {
                reference_sheet: Some(sheet),
                ..
            } => sheet.as_ref(),
            _ => {
                return Err(AppCommandError::Validation {
                    detail: "entity has no sprite reference sheet".into(),
                });
            }
        };
        let source = find_variant(sheet, args.variant_id)
            .cloned()
            .ok_or_else(|| AppCommandError::NotFound {
                entity: "sheet variant".into(),
                id: u64::from(args.variant_id.get()),
            })?;
        let lora_asset = find_lora_asset(project, source.applied_lora);
        (
            source,
            project.library.ai.style_notes.clone(),
            configured_execution_model(
                selected_model(args.model, OperationKind::ChatTurn, project),
                &state,
            ),
            lora_asset,
        )
    };
    let (request_id, token, lease) =
        start_sheet_request_with_lease(&state, args.entity_id, true, Some(args.variant_id))?;
    let mut transcript = source.chat_transcript.clone().unwrap_or_default();
    transcript.turns.push(ChatTurn {
        timestamp: now_secs(),
        user_message: args.user_message.clone(),
        mask: args.mask.clone(),
        // The committed provider variant receives its durable id after
        // completion. The final transcript is patched with that id during
        // the commit helper.
        resulting_variant_id: SheetVariantId::new(0),
    });
    let refinement = args
        .mask
        .clone()
        .map(|mask_png| RefinementKind::Masked { mask_png });
    let spec = VariantCommitSpec {
        provider: SheetProviderRequest {
            operation: OperationKind::ChatTurn,
            model,
            quality: Quality::Medium,
            prompt: args.user_message,
            template: source.template,
            width: source.width,
            height: source.height,
            chroma_color: source.chroma_color,
            references: source.references.clone(),
            source_image: Some(source.image.clone()),
            mask: args.mask,
            candidate_count: 1,
            applied_lora: lora_asset,
            lora_weight: source.lora_weight,
            real_world_grounding: source.real_world_grounding,
        },
        build: VariantBuildSpec {
            origin: VariantOrigin::ChatTurn,
            parent_variant_id: Some(source.id),
            refinement,
            promotion: false,
            chat_transcript: Some(transcript),
            applied_lora: source.applied_lora,
            lora_weight: source.lora_weight,
            real_world_grounding: source.real_world_grounding,
        },
        stream_index: 0,
    };
    spawn_sheet_provider_job(
        app,
        request_id,
        args.entity_id,
        token,
        lease,
        style_notes,
        spec,
    );
    Ok(request_id)
}

/// Creates a high-quality promoted draft derived from a source variant.
#[tauri::command(async, rename_all = "snake_case")]
pub async fn library_promote_variant_to_final(
    args: LibraryPromoteVariantToFinalArgs,
    app: AppHandle,
    state: State<'_, AppState>,
) -> CommandResult<RequestId> {
    let (source, style_notes, model, lora_asset) = {
        let doc = state.doc.read().await;
        let project = doc
            .project
            .as_ref()
            .ok_or(AppCommandError::NoActiveProject)?;
        let entity = project
            .library
            .entities
            .iter()
            .find(|e| e.id == args.entity_id)
            .ok_or_else(|| AppCommandError::NotFound {
                entity: "entity".into(),
                id: u64::from(args.entity_id.get()),
            })?;
        let sheet = match &entity.content {
            EntityContent::Sprites {
                reference_sheet: Some(sheet),
                ..
            } => sheet.as_ref(),
            _ => {
                return Err(AppCommandError::Validation {
                    detail: "entity has no sprite reference sheet".into(),
                });
            }
        };
        let source = find_variant(sheet, args.source_variant_id)
            .cloned()
            .ok_or_else(|| AppCommandError::NotFound {
                entity: "sheet variant".into(),
                id: u64::from(args.source_variant_id.get()),
            })?;
        let lora_asset = find_lora_asset(project, source.applied_lora);
        (
            source,
            project.library.ai.style_notes.clone(),
            configured_execution_model(
                selected_model(args.target_model, OperationKind::Promotion, project),
                &state,
            ),
            lora_asset,
        )
    };
    let (request_id, token, lease) =
        start_sheet_request_with_lease(&state, args.entity_id, true, None)?;
    let spec = VariantCommitSpec {
        provider: SheetProviderRequest {
            operation: OperationKind::Promotion,
            model,
            quality: args.target_quality,
            prompt: source.user_prompt.clone(),
            template: source.template,
            width: source.width,
            height: source.height,
            chroma_color: source.chroma_color,
            references: source.references.clone(),
            source_image: Some(source.image.clone()),
            mask: None,
            candidate_count: 1,
            applied_lora: lora_asset,
            lora_weight: source.lora_weight,
            real_world_grounding: source.real_world_grounding,
        },
        build: VariantBuildSpec {
            origin: VariantOrigin::Promotion,
            parent_variant_id: Some(source.id),
            refinement: None,
            promotion: true,
            chat_transcript: source.chat_transcript.clone(),
            applied_lora: source.applied_lora,
            lora_weight: source.lora_weight,
            real_world_grounding: source.real_world_grounding,
        },
        stream_index: 0,
    };
    spawn_sheet_provider_job(
        app,
        request_id,
        args.entity_id,
        token,
        lease,
        style_notes,
        spec,
    );
    Ok(request_id)
}

/// Creates comparison-grid draft placeholders for each model/quality pair.
#[tauri::command(async, rename_all = "snake_case")]
pub async fn library_start_cross_model_grid(
    args: LibraryStartCrossModelGridArgs,
    app: AppHandle,
    state: State<'_, AppState>,
) -> CommandResult<RequestId> {
    if args.prompt.trim().is_empty() {
        return Err(AppCommandError::Validation {
            detail: "cross-model prompt must not be empty".into(),
        });
    }
    if !(2..=4).contains(&args.combinations.len()) {
        return Err(AppCommandError::Validation {
            detail: "cross-model grid requires 2 to 4 model combinations".into(),
        });
    }
    let style_notes = {
        let doc = state.doc.read().await;
        let project = doc
            .project
            .as_ref()
            .ok_or(AppCommandError::NoActiveProject)?;
        let entity = project
            .library
            .entities
            .iter()
            .find(|e| e.id == args.entity_id)
            .ok_or_else(|| AppCommandError::NotFound {
                entity: "entity".into(),
                id: u64::from(args.entity_id.get()),
            })?;
        if !matches!(entity.content, EntityContent::Sprites { .. }) {
            return Err(AppCommandError::Validation {
                detail: format!(
                    "entity {} is not a sprite entity; reference sheets belong to sprites",
                    args.entity_id.get()
                ),
            });
        }
        project.library.ai.style_notes.clone()
    };
    let (request_id, token, lease) =
        start_sheet_request_with_lease(&state, args.entity_id, false, None)?;
    let per_combo = args.candidate_count_per_combo.clamp(1, 2);
    let specs = args
        .combinations
        .iter()
        .enumerate()
        .map(|(stream_index, combo)| VariantCommitSpec {
            provider: SheetProviderRequest {
                operation: OperationKind::CrossModelGrid,
                model: combo.model,
                quality: combo.quality,
                prompt: args.prompt.clone(),
                template: args.template,
                width: args.width,
                height: args.height,
                chroma_color: args.chroma_color,
                references: args.references.clone(),
                source_image: None,
                mask: None,
                candidate_count: per_combo,
                applied_lora: None,
                lora_weight: 1.0,
                real_world_grounding: false,
            },
            build: VariantBuildSpec {
                origin: VariantOrigin::CrossModelGrid,
                parent_variant_id: None,
                refinement: None,
                promotion: false,
                chat_transcript: None,
                applied_lora: None,
                lora_weight: 1.0,
                real_world_grounding: false,
            },
            stream_index: u8::try_from(stream_index).unwrap_or(u8::MAX),
        })
        .collect();
    spawn_cross_model_job(
        app,
        request_id,
        args.entity_id,
        token,
        lease,
        style_notes,
        specs,
    );
    Ok(request_id)
}

/// Stores an SVG vector export on a variant.
#[tauri::command(async, rename_all = "snake_case")]
pub async fn library_export_variant_as_vector(
    args: LibraryExportVariantAsVectorArgs,
    app: AppHandle,
    state: State<'_, AppState>,
) -> CommandResult<RequestId> {
    let source_image = {
        let doc = state.doc.read().await;
        let project = doc
            .project
            .as_ref()
            .ok_or(AppCommandError::NoActiveProject)?;
        let entity = project
            .library
            .entities
            .iter()
            .find(|e| e.id == args.entity_id)
            .ok_or_else(|| AppCommandError::NotFound {
                entity: "entity".into(),
                id: u64::from(args.entity_id.get()),
            })?;
        let sheet = match &entity.content {
            EntityContent::Sprites {
                reference_sheet: Some(sheet),
                ..
            } => sheet.as_ref(),
            _ => {
                return Err(AppCommandError::Validation {
                    detail: "entity has no sprite reference sheet".into(),
                });
            }
        };
        find_variant(sheet, args.variant_id)
            .map(|variant| variant.image.clone())
            .ok_or_else(|| AppCommandError::NotFound {
                entity: "sheet variant".into(),
                id: u64::from(args.variant_id.get()),
            })?
    };
    let (request_id, token, lease) =
        start_sheet_request_with_lease(&state, args.entity_id, true, None)?;
    tauri::async_runtime::spawn(async move {
        let _lease = lease;
        if token.is_cancelled() {
            emit_sheet_cancelled(&app, request_id);
            return;
        }
        let _permit = app
            .state::<AppState>()
            .reference_sheet_requests
            .acquire_sprite_permit(args.entity_id.get(), true, &token)
            .await;
        if token.is_cancelled() {
            emit_sheet_cancelled(&app, request_id);
            return;
        }
        let runtime = app.state::<AppState>().verb_runtime.clone();
        let result = invoke_vector_export(runtime, &source_image, token.clone()).await;
        if token.is_cancelled() {
            emit_sheet_cancelled(&app, request_id);
            return;
        }
        let svg = match result {
            Ok(svg) => svg,
            Err(err) => {
                emit_sheet_error(&app, request_id, err);
                return;
            }
        };
        let state = app.state::<AppState>();
        let updated = {
            let ts = now_secs();
            let mut doc = state.doc.write().await;
            let Some(project) = doc.project.as_mut() else {
                emit_sheet_error(&app, request_id, AppCommandError::NoActiveProject);
                return;
            };
            let Some(entity) = project
                .library
                .entities
                .iter_mut()
                .find(|e| e.id == args.entity_id)
            else {
                emit_sheet_error(
                    &app,
                    request_id,
                    AppCommandError::NotFound {
                        entity: "entity".into(),
                        id: u64::from(args.entity_id.get()),
                    },
                );
                return;
            };
            let sheet = match embedded_reference_sheet_mut(entity) {
                Ok(sheet) => sheet,
                Err(err) => {
                    emit_sheet_error(&app, request_id, err);
                    return;
                }
            };
            let Some(variant) = sheet
                .canonical
                .as_mut()
                .filter(|variant| variant.id == args.variant_id)
                .or_else(|| {
                    sheet
                        .variants
                        .iter_mut()
                        .find(|variant| variant.id == args.variant_id)
                })
            else {
                emit_sheet_error(
                    &app,
                    request_id,
                    AppCommandError::NotFound {
                        entity: "sheet variant".into(),
                        id: u64::from(args.variant_id.get()),
                    },
                );
                return;
            };
            variant.vector_image = Some(svg);
            entity.updated_at = ts;
            let updated = entity.clone();
            doc.dirty = true;
            updated
        };
        emit_sheet_event(
            &app,
            SHEET_REQUEST_COMPLETE_EVENT,
            &SheetRequestCompletePayload {
                request_id,
                entity_id: args.entity_id,
                sprite: updated,
                total_cost_usd: 0.0,
            },
        );
    });
    Ok(request_id)
}

/// Saves a reference image into the project asset library.
#[tauri::command(async, rename_all = "snake_case")]
pub async fn library_save_reference_to_library(
    args: LibrarySaveReferenceToLibraryArgs,
    state: State<'_, AppState>,
) -> CommandResult<ReferenceAsset> {
    if args.image.bytes.is_empty() {
        return Err(AppCommandError::Validation {
            detail: "reference image bytes must not be empty".into(),
        });
    }
    let ts = now_secs();
    let mut doc = state.doc.write().await;
    let mut next_id = doc.next_id;
    let project = doc
        .project
        .as_mut()
        .ok_or(AppCommandError::NoActiveProject)?;
    let asset = ReferenceAsset {
        id: mint_asset_id(&mut next_id),
        image: args.image,
        default_role: args.role,
        tags: args.tags,
        source_variant_id: None,
        created_at: ts,
    };
    project
        .library
        .ai
        .asset_library
        .references
        .push(asset.clone());
    doc.next_id = next_id;
    doc.dirty = true;
    Ok(asset)
}

/// Saves a variant image as a character card plus a reference asset.
#[tauri::command(async, rename_all = "snake_case")]
pub async fn library_save_variant_as_character_card(
    args: LibrarySaveVariantCardArgs,
    state: State<'_, AppState>,
) -> CommandResult<CharacterCard> {
    if args.name.trim().is_empty() {
        return Err(AppCommandError::Validation {
            detail: "character card name must not be empty".into(),
        });
    }
    let ts = now_secs();
    let mut doc = state.doc.write().await;
    let mut next_id = doc.next_id;
    let ref_id = mint_asset_id(&mut next_id);
    let card_id = mint_asset_id(&mut next_id);
    let project = doc
        .project
        .as_mut()
        .ok_or(AppCommandError::NoActiveProject)?;
    let entity = project
        .library
        .entities
        .iter_mut()
        .find(|e| e.id == args.entity_id)
        .ok_or_else(|| AppCommandError::NotFound {
            entity: "entity".into(),
            id: u64::from(args.entity_id.get()),
        })?;
    let sheet = embedded_reference_sheet_mut(entity)?;
    let variant = find_variant(sheet, args.variant_id)
        .cloned()
        .ok_or_else(|| AppCommandError::NotFound {
            entity: "sheet variant".into(),
            id: u64::from(args.variant_id.get()),
        })?;
    project
        .library
        .ai
        .asset_library
        .references
        .push(ReferenceAsset {
            id: ref_id,
            image: variant.image,
            default_role: ReferenceRole::Subject,
            tags: Vec::new(),
            source_variant_id: Some(args.variant_id),
            created_at: ts,
        });
    let card = CharacterCard {
        id: card_id,
        name: args.name,
        references: vec![ref_id],
        style_notes: args.style_notes,
        associated_lora: None,
        created_at: ts,
    };
    project
        .library
        .ai
        .asset_library
        .character_cards
        .push(card.clone());
    doc.next_id = next_id;
    doc.dirty = true;
    Ok(card)
}

/// Saves a variant image as a style swatch plus a reference asset.
#[tauri::command(async, rename_all = "snake_case")]
pub async fn library_save_variant_as_style_swatch(
    args: LibrarySaveVariantCardArgs,
    state: State<'_, AppState>,
) -> CommandResult<StyleSwatch> {
    if args.name.trim().is_empty() {
        return Err(AppCommandError::Validation {
            detail: "style swatch name must not be empty".into(),
        });
    }
    let ts = now_secs();
    let mut doc = state.doc.write().await;
    let mut next_id = doc.next_id;
    let ref_id = mint_asset_id(&mut next_id);
    let swatch_id = mint_asset_id(&mut next_id);
    let project = doc
        .project
        .as_mut()
        .ok_or(AppCommandError::NoActiveProject)?;
    let entity = project
        .library
        .entities
        .iter_mut()
        .find(|e| e.id == args.entity_id)
        .ok_or_else(|| AppCommandError::NotFound {
            entity: "entity".into(),
            id: u64::from(args.entity_id.get()),
        })?;
    let sheet = embedded_reference_sheet_mut(entity)?;
    let variant = find_variant(sheet, args.variant_id)
        .cloned()
        .ok_or_else(|| AppCommandError::NotFound {
            entity: "sheet variant".into(),
            id: u64::from(args.variant_id.get()),
        })?;
    project
        .library
        .ai
        .asset_library
        .references
        .push(ReferenceAsset {
            id: ref_id,
            image: variant.image,
            default_role: ReferenceRole::Style,
            tags: Vec::new(),
            source_variant_id: Some(args.variant_id),
            created_at: ts,
        });
    let swatch = StyleSwatch {
        id: swatch_id,
        name: args.name,
        references: vec![ref_id],
        style_notes: args.style_notes,
        associated_lora: None,
        created_at: ts,
    };
    project
        .library
        .ai
        .asset_library
        .style_swatches
        .push(swatch.clone());
    doc.next_id = next_id;
    doc.dirty = true;
    Ok(swatch)
}

/// Returns the project-scoped asset library.
#[tauri::command(async, rename_all = "snake_case")]
pub async fn library_browse_assets(
    _filters: Option<serde_json::Value>,
    state: State<'_, AppState>,
) -> CommandResult<AssetLibrary> {
    let doc = state.doc.read().await;
    let project = doc
        .project
        .as_ref()
        .ok_or(AppCommandError::NoActiveProject)?;
    Ok(project.library.ai.asset_library.clone())
}

/// Gets a single asset by id.
#[tauri::command(async, rename_all = "snake_case")]
pub async fn library_get_asset(
    asset_id: AssetId,
    state: State<'_, AppState>,
) -> CommandResult<serde_json::Value> {
    let doc = state.doc.read().await;
    let project = doc
        .project
        .as_ref()
        .ok_or(AppCommandError::NoActiveProject)?;
    let library = &project.library.ai.asset_library;
    if let Some(asset) = library.references.iter().find(|asset| asset.id == asset_id) {
        return Ok(serde_json::json!({ "kind": "reference", "value": asset }));
    }
    if let Some(asset) = library
        .character_cards
        .iter()
        .find(|asset| asset.id == asset_id)
    {
        return Ok(serde_json::json!({ "kind": "character_card", "value": asset }));
    }
    if let Some(asset) = library
        .style_swatches
        .iter()
        .find(|asset| asset.id == asset_id)
    {
        return Ok(serde_json::json!({ "kind": "style_swatch", "value": asset }));
    }
    if let Some(asset) = library.loras.iter().find(|asset| asset.id == asset_id) {
        return Ok(serde_json::json!({ "kind": "lora", "value": asset }));
    }
    Err(AppCommandError::NotFound {
        entity: "asset".into(),
        id: u64::from(asset_id.get()),
    })
}

/// Removes an asset from the project asset library.
#[tauri::command(async, rename_all = "snake_case")]
pub async fn library_remove_asset(
    asset_id: AssetId,
    state: State<'_, AppState>,
) -> CommandResult<()> {
    let mut doc = state.doc.write().await;
    let project = doc
        .project
        .as_mut()
        .ok_or(AppCommandError::NoActiveProject)?;
    let library = &mut project.library.ai.asset_library;
    let before = (
        library.references.len(),
        library.character_cards.len(),
        library.style_swatches.len(),
        library.loras.len(),
    );
    library.references.retain(|asset| asset.id != asset_id);
    library.character_cards.retain(|asset| asset.id != asset_id);
    library.style_swatches.retain(|asset| asset.id != asset_id);
    library.loras.retain(|asset| asset.id != asset_id);
    let after = (
        library.references.len(),
        library.character_cards.len(),
        library.style_swatches.len(),
        library.loras.len(),
    );
    if before == after {
        return Err(AppCommandError::NotFound {
            entity: "asset".into(),
            id: u64::from(asset_id.get()),
        });
    }
    doc.dirty = true;
    Ok(())
}

/// Updates tags on a reference asset.
#[tauri::command(async, rename_all = "snake_case")]
pub async fn library_update_asset_tags(
    args: LibraryUpdateAssetTagsArgs,
    state: State<'_, AppState>,
) -> CommandResult<()> {
    let mut doc = state.doc.write().await;
    let project = doc
        .project
        .as_mut()
        .ok_or(AppCommandError::NoActiveProject)?;
    let Some(asset) = project
        .library
        .ai
        .asset_library
        .references
        .iter_mut()
        .find(|asset| asset.id == args.asset_id)
    else {
        return Err(AppCommandError::NotFound {
            entity: "reference asset".into(),
            id: u64::from(args.asset_id.get()),
        });
    };
    asset.tags = args.tags;
    doc.dirty = true;
    Ok(())
}

/// Starts a fal LoRA training job record.
#[tauri::command(async, rename_all = "snake_case")]
pub async fn library_train_lora(
    args: LibraryTrainLoraArgs,
    app: AppHandle,
    state: State<'_, AppState>,
) -> CommandResult<TrainingJobId> {
    if args.name.trim().is_empty() || args.trigger_word.trim().is_empty() {
        return Err(AppCommandError::Validation {
            detail: "LoRA name and trigger word must not be empty".into(),
        });
    }
    if args.training_data.len() < 10 {
        return Err(AppCommandError::Validation {
            detail: "LoRA training requires at least 10 saved reference assets".into(),
        });
    }
    let ts = now_secs();
    let mut doc = state.doc.write().await;
    let mut next_id = doc.next_id;
    let project = doc
        .project
        .as_mut()
        .ok_or(AppCommandError::NoActiveProject)?;
    let available_refs: HashMap<AssetId, ReferenceAsset> = project
        .library
        .ai
        .asset_library
        .references
        .iter()
        .map(|asset| (asset.id, asset.clone()))
        .collect();
    if let Some(missing) = args
        .training_data
        .iter()
        .find(|asset_id| !available_refs.contains_key(asset_id))
    {
        return Err(AppCommandError::NotFound {
            entity: "reference asset".into(),
            id: u64::from(missing.get()),
        });
    }
    let training_assets = args
        .training_data
        .iter()
        .filter_map(|asset_id| available_refs.get(asset_id).cloned())
        .collect::<Vec<_>>();
    let id = TrainingJobId::new(next_id);
    next_id += 1;
    project.library.ai.training_jobs.push(TrainingJob {
        id,
        asset_name: args.name.clone(),
        kind: args.kind,
        target_model: args.target_model,
        trigger_word: args.trigger_word.clone(),
        training_data: args.training_data.clone(),
        fal_job_id: format!("pending-fal-{}", id.get()),
        status: TrainingStatus::Running,
        created_at: ts,
        completed_at: None,
        result_lora_id: None,
        error: None,
    });
    doc.next_id = next_id;
    doc.dirty = true;
    drop(doc);

    emit_training_event(
        &app,
        TRAINING_JOB_PROGRESS_EVENT,
        &TrainingJobProgressPayload {
            job_id: id,
            status: TrainingStatus::Running,
            message: "submitting fal LoRA training job".into(),
        },
    );
    let asset_name = args.name;
    let kind = args.kind;
    let target_model = args.target_model;
    let trigger_word = args.trigger_word;
    tauri::async_runtime::spawn(async move {
        let result = async {
            let archive = encode_reference_assets_zip(&training_assets)?;
            let backend =
                FalBackend::from_keychain().map_err(|err| AppCommandError::VerbError {
                    message: err.to_string(),
                })?;
            backend
                .train_lora_archive(
                    archive,
                    &trigger_word,
                    matches!(kind, LoraKind::Style),
                    CancellationToken::new(),
                )
                .await
                .map_err(|err| AppCommandError::VerbError {
                    message: err.to_string(),
                })
        }
        .await;
        let state = app.state::<AppState>();
        match result {
            Ok(training) => {
                let maybe_asset = {
                    let mut doc = state.doc.write().await;
                    let mut next_id = doc.next_id;
                    let Some(project) = doc.project.as_mut() else {
                        emit_training_event(
                            &app,
                            TRAINING_JOB_FAILED_EVENT,
                            &TrainingJobFailedPayload {
                                job_id: id,
                                error: AppCommandError::NoActiveProject,
                            },
                        );
                        return;
                    };
                    let Some(job) = project
                        .library
                        .ai
                        .training_jobs
                        .iter_mut()
                        .find(|job| job.id == id)
                    else {
                        emit_training_event(
                            &app,
                            TRAINING_JOB_FAILED_EVENT,
                            &TrainingJobFailedPayload {
                                job_id: id,
                                error: AppCommandError::NotFound {
                                    entity: "training job".into(),
                                    id: u64::from(id.get()),
                                },
                            },
                        );
                        return;
                    };
                    if job.status == TrainingStatus::Cancelled {
                        return;
                    }
                    let lora_id = mint_asset_id(&mut next_id);
                    let asset = LoraAsset {
                        id: lora_id,
                        name: asset_name.clone(),
                        kind,
                        trigger_word: trigger_word.clone(),
                        target_model,
                        fal_lora_url: training.lora_url,
                        training_data_thumbnails: training_assets
                            .iter()
                            .take(12)
                            .map(|asset| asset.image.clone())
                            .collect(),
                        created_at: now_secs(),
                    };
                    project.library.ai.asset_library.loras.push(asset.clone());
                    job.status = TrainingStatus::Completed;
                    job.completed_at = Some(now_secs());
                    job.result_lora_id = Some(lora_id);
                    doc.next_id = next_id;
                    doc.dirty = true;
                    Some(asset)
                };
                if let Some(asset) = maybe_asset {
                    emit_training_event(
                        &app,
                        TRAINING_JOB_COMPLETE_EVENT,
                        &TrainingJobCompletePayload {
                            job_id: id,
                            lora_asset: asset,
                        },
                    );
                }
            }
            Err(err) => {
                {
                    let mut doc = state.doc.write().await;
                    if let Some(project) = doc.project.as_mut()
                        && let Some(job) = project
                            .library
                            .ai
                            .training_jobs
                            .iter_mut()
                            .find(|job| job.id == id)
                    {
                        job.status = TrainingStatus::Failed;
                        job.completed_at = Some(now_secs());
                        job.error = Some(err.to_string());
                        doc.dirty = true;
                    }
                }
                emit_training_event(
                    &app,
                    TRAINING_JOB_FAILED_EVENT,
                    &TrainingJobFailedPayload {
                        job_id: id,
                        error: err,
                    },
                );
            }
        }
    });
    Ok(id)
}

/// Returns one LoRA training job.
#[tauri::command(async, rename_all = "snake_case")]
pub async fn library_get_training_job_status(
    job_id: TrainingJobId,
    state: State<'_, AppState>,
) -> CommandResult<TrainingJob> {
    let doc = state.doc.read().await;
    let project = doc
        .project
        .as_ref()
        .ok_or(AppCommandError::NoActiveProject)?;
    project
        .library
        .ai
        .training_jobs
        .iter()
        .find(|job| job.id == job_id)
        .cloned()
        .ok_or_else(|| AppCommandError::NotFound {
            entity: "training job".into(),
            id: u64::from(job_id.get()),
        })
}

/// Lists all LoRA training jobs.
#[tauri::command(async, rename_all = "snake_case")]
pub async fn library_list_training_jobs(
    state: State<'_, AppState>,
) -> CommandResult<Vec<TrainingJob>> {
    let doc = state.doc.read().await;
    let project = doc
        .project
        .as_ref()
        .ok_or(AppCommandError::NoActiveProject)?;
    Ok(project.library.ai.training_jobs.clone())
}

/// Cancels a queued/running LoRA training job record.
#[tauri::command(async, rename_all = "snake_case")]
pub async fn library_cancel_training_job(
    job_id: TrainingJobId,
    state: State<'_, AppState>,
) -> CommandResult<()> {
    let mut doc = state.doc.write().await;
    let project = doc
        .project
        .as_mut()
        .ok_or(AppCommandError::NoActiveProject)?;
    let Some(job) = project
        .library
        .ai
        .training_jobs
        .iter_mut()
        .find(|job| job.id == job_id)
    else {
        return Err(AppCommandError::NotFound {
            entity: "training job".into(),
            id: u64::from(job_id.get()),
        });
    };
    job.status = TrainingStatus::Cancelled;
    job.completed_at = Some(now_secs());
    doc.dirty = true;
    Ok(())
}

/// Project style notes getter.
#[tauri::command(async, rename_all = "snake_case")]
pub async fn project_get_style_notes(state: State<'_, AppState>) -> CommandResult<String> {
    let doc = state.doc.read().await;
    let project = doc
        .project
        .as_ref()
        .ok_or(AppCommandError::NoActiveProject)?;
    Ok(project.library.ai.style_notes.clone())
}

/// Project style notes setter.
#[tauri::command(async, rename_all = "snake_case")]
pub async fn project_set_style_notes(
    notes: String,
    state: State<'_, AppState>,
) -> CommandResult<()> {
    let mut doc = state.doc.write().await;
    let project = doc
        .project
        .as_mut()
        .ok_or(AppCommandError::NoActiveProject)?;
    project.library.ai.style_notes = notes;
    doc.dirty = true;
    Ok(())
}

/// Stores a per-operation model preference.
#[tauri::command(async, rename_all = "snake_case")]
pub async fn project_set_operation_model_pref(
    args: ProjectSetOperationModelPrefArgs,
    state: State<'_, AppState>,
) -> CommandResult<()> {
    let mut doc = state.doc.write().await;
    let project = doc
        .project
        .as_mut()
        .ok_or(AppCommandError::NoActiveProject)?;
    project
        .library
        .ai
        .per_operation_model_prefs
        .insert(args.operation, args.model);
    doc.dirty = true;
    Ok(())
}

/// Clears all per-operation model preferences.
#[tauri::command(async, rename_all = "snake_case")]
pub async fn project_clear_operation_model_prefs(state: State<'_, AppState>) -> CommandResult<()> {
    let mut doc = state.doc.write().await;
    let project = doc
        .project
        .as_mut()
        .ok_or(AppCommandError::NoActiveProject)?;
    project.library.ai.per_operation_model_prefs.clear();
    doc.dirty = true;
    Ok(())
}

/// Sets the default chroma color.
#[tauri::command(async, rename_all = "snake_case")]
pub async fn project_set_default_chroma(
    color: Rgba,
    state: State<'_, AppState>,
) -> CommandResult<()> {
    let mut doc = state.doc.write().await;
    let project = doc
        .project
        .as_mut()
        .ok_or(AppCommandError::NoActiveProject)?;
    project.library.ai.default_chroma = color;
    doc.dirty = true;
    Ok(())
}

/// Sets the default quality.
#[tauri::command(async, rename_all = "snake_case")]
pub async fn project_set_default_quality(
    quality: Quality,
    state: State<'_, AppState>,
) -> CommandResult<()> {
    let mut doc = state.doc.write().await;
    let project = doc
        .project
        .as_mut()
        .ok_or(AppCommandError::NoActiveProject)?;
    project.library.ai.default_quality = quality;
    doc.dirty = true;
    Ok(())
}

/// Sets the default candidate count.
#[tauri::command(async, rename_all = "snake_case")]
pub async fn project_set_default_candidate_count(
    n: u8,
    state: State<'_, AppState>,
) -> CommandResult<()> {
    if !(1..=4).contains(&n) {
        return Err(AppCommandError::Validation {
            detail: "default candidate count must be between 1 and 4".into(),
        });
    }
    let mut doc = state.doc.write().await;
    let project = doc
        .project
        .as_mut()
        .ok_or(AppCommandError::NoActiveProject)?;
    project.library.ai.default_candidate_count = n;
    doc.dirty = true;
    Ok(())
}

// ── sheet helpers ─────────────────────────────────────────────────────────────

#[allow(dead_code)]
pub(crate) fn apply_generated_reference_sheet_payload(
    project: &mut pixhaus_core::project::Project,
    next_id: &mut u32,
    payload: GenerateSheetPayload,
    ts: i64,
) -> Result<Entity, AppCommandError> {
    let entity = project
        .library
        .entities
        .iter_mut()
        .find(|e| e.id == payload.entity_id)
        .ok_or_else(|| AppCommandError::NotFound {
            entity: "entity".into(),
            id: u64::from(payload.entity_id.get()),
        })?;

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

pub(crate) fn import_reference_sheet_draft(
    project: &mut pixhaus_core::project::Project,
    next_id: &mut u32,
    args: LibraryImportReferenceSheetArgs,
    ts: i64,
) -> Result<Entity, AppCommandError> {
    let entity = project
        .library
        .entities
        .iter_mut()
        .find(|e| e.id == args.entity_id)
        .ok_or_else(|| AppCommandError::NotFound {
            entity: "entity".into(),
            id: u64::from(args.entity_id.get()),
        })?;

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
    let entity = project
        .library
        .entities
        .iter_mut()
        .find(|e| e.id == entity_id)
        .ok_or_else(|| AppCommandError::NotFound {
            entity: "entity".into(),
            id: u64::from(entity_id.get()),
        })?;

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
    let entity = project
        .library
        .entities
        .iter_mut()
        .find(|e| e.id == entity_id)
        .ok_or_else(|| AppCommandError::NotFound {
            entity: "entity".into(),
            id: u64::from(entity_id.get()),
        })?;

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

// ── AI hooks (B9.4) ───────────────────────────────────────────────────────────

/// Resolves an existing [`TagDefinition`] by name (case-insensitive) or mints
/// a new auto-generated one. Returns the `TagId`.
fn find_or_create_tag(
    library: &mut pixhaus_core::project::Library,
    next_id: &mut u32,
    name: &str,
) -> TagId {
    let normalized = name.to_lowercase();
    if let Some(existing) = library
        .tags
        .iter()
        .find(|t| t.name.to_lowercase() == normalized)
    {
        return existing.id;
    }
    let id = TagId::new(*next_id);
    *next_id += 1;
    library.tags.push(TagDefinition {
        id,
        name: normalized,
        color: None,
        auto_generated: true,
    });
    id
}

/// Applies a [`VerbEffect::SuggestEntityTags`] to the project: resolves or
/// creates [`TagDefinition`]s and adds their IDs to
/// `entity.ai.suggested_tags`. Returns the updated suggested-tag list so the
/// caller can surface it to the UI without a second read.
pub(crate) fn apply_suggest_entity_tags(
    project: &mut pixhaus_core::project::Project,
    next_id: &mut u32,
    entity_id: EntityId,
    tag_names: Vec<String>,
    ts: i64,
) -> Result<Vec<TagDefinition>, AppCommandError> {
    let entity_idx = project
        .library
        .entities
        .iter()
        .position(|e| e.id == entity_id)
        .ok_or_else(|| AppCommandError::NotFound {
            entity: "entity".into(),
            id: u64::from(entity_id.get()),
        })?;

    let mut new_ids = Vec::with_capacity(tag_names.len());
    for name in tag_names {
        let trimmed = name.trim().to_owned();
        if trimmed.is_empty() {
            continue;
        }
        let id = find_or_create_tag(&mut project.library, next_id, &trimmed);
        new_ids.push(id);
    }

    // Scope the mutable entity borrow so the immutable tags borrow below compiles.
    let suggested_ids = {
        let entity = &mut project.library.entities[entity_idx];
        for id in new_ids {
            if !entity.ai.suggested_tags.contains(&id) {
                entity.ai.suggested_tags.push(id);
            }
        }
        entity.updated_at = ts;
        entity.ai.suggested_tags.clone()
    };

    let definitions = project
        .library
        .tags
        .iter()
        .filter(|t| suggested_ids.contains(&t.id))
        .cloned()
        .collect();
    Ok(definitions)
}

/// Applies a [`VerbEffect::UpdateProjectAi`]: deduplicates and appends entity
/// IDs to `ProjectAi.style_corpus`, and optionally overwrites the `LoRA` path.
///
/// IDs that don't currently exist in the project library are silently
/// dropped. Stale references in `style_corpus` would force every later
/// corpus consumer to defend against them; filtering at the write boundary
/// keeps the invariant local.
pub(crate) fn apply_update_project_ai(
    project: &mut pixhaus_core::project::Project,
    add_entity_ids: Vec<EntityId>,
    lora_path: Option<String>,
) {
    // Collect known entity ids once so the inner loop is O(n + m).
    let known: HashSet<EntityId> = project.library.entities.iter().map(|e| e.id).collect();

    for id in add_entity_ids {
        if !known.contains(&id) {
            continue; // skip unknown
        }
        if !project.library.ai.style_corpus.contains(&id) {
            project.library.ai.style_corpus.push(id);
        }
    }
    if let Some(path) = lora_path {
        project.library.ai.project_lora_path = Some(path);
    }
}

/// Applies a [`VerbEffect::UpdateEntityAi`]: overwrites
/// `Entity.ai.lora_path` for the named entity and bumps `updated_at`.
///
/// Returns `true` when the entity was found and updated, `false` when
/// the entity id is unknown (silently dropped — same invariant as
/// [`apply_update_project_ai`], unknown ids would be a stale write).
///
/// `None` for `lora_path` is a no-op: the verb returns `None` until
/// the host downloads the safetensors and supplies the real path.
pub(crate) fn apply_update_entity_ai(
    project: &mut pixhaus_core::project::Project,
    entity_id: EntityId,
    lora_path: Option<String>,
    ts: i64,
) -> bool {
    let Some(path) = lora_path else {
        return project.library.entities.iter().any(|e| e.id == entity_id);
    };
    let Some(entity) = project
        .library
        .entities
        .iter_mut()
        .find(|e| e.id == entity_id)
    else {
        return false;
    };
    entity.ai.lora_path = Some(path);
    entity.updated_at = ts;
    true
}

/// Renders an entity's name, kind, and existing tag names into a short
/// plaintext block the auto-tag VLM can use as grounding when no sprite
/// reference is attached.
fn build_auto_tag_metadata(entity: &Entity, tags: &[TagDefinition]) -> String {
    let kind = match &entity.kind {
        EntityKind::Tileset => "Tileset".to_owned(),
        EntityKind::Tilemap => "Tilemap".to_owned(),
        EntityKind::Custom(category) => format!("Custom({category})"),
    };
    let existing: Vec<&str> = entity
        .tags
        .iter()
        .filter_map(|tid| tags.iter().find(|t| t.id == *tid).map(|t| t.name.as_str()))
        .collect();
    let existing_str = if existing.is_empty() {
        "none".to_owned()
    } else {
        existing.join(", ")
    };
    format!(
        "Entity name: {}\nKind: {kind}\nExisting tags: {existing_str}",
        entity.name
    )
}

/// Invokes the Critique verb in `LibraryAutoTag` mode for the given entity.
#[tauri::command(async, rename_all = "snake_case")]
pub async fn library_auto_tag_entity(
    entity_id: EntityId,
    state: State<'_, AppState>,
) -> CommandResult<Vec<TagDefinition>> {
    // Phase 1: read lock — build context, validate entity exists, and
    // construct the metadata summary the VLM will see (since we don't
    // attach a sprite reference today, the metadata is the only
    // grounding the model has).
    let (ctx, entity_metadata) = {
        let doc = state.doc.read().await;
        let project = doc
            .project
            .as_ref()
            .ok_or(AppCommandError::NoActiveProject)?;
        let entity = project
            .library
            .entities
            .iter()
            .find(|e| e.id == entity_id)
            .ok_or_else(|| AppCommandError::NotFound {
                entity: "entity".into(),
                id: u64::from(entity_id.get()),
            })?;
        let metadata = build_auto_tag_metadata(entity, &project.library.tags);
        let ctx = VerbContextBuilder::new(project.metadata.clone())
            .with_library_entity(entity_id)
            .build();
        (ctx, metadata)
        // read guard drops here
    };

    // Phase 2: invoke verb — no document lock held during the async call.
    let verb_id = VerbId::new("pixhaus.builtin.critique");
    let inputs = VerbInputs::from_struct(&CritiqueInputs {
        mode: CritiqueMode::LibraryAutoTag {
            entity_id,
            entity_metadata: Some(entity_metadata),
        },
        checks: vec![],
        notes: None,
    })
    .map_err(|e| AppCommandError::VerbError {
        message: e.to_string(),
    })?;

    let invocation = state
        .verb_runtime
        .invoke(&verb_id, ctx, inputs)
        .map_err(|e| AppCommandError::VerbError {
            message: e.to_string(),
        })?;

    let preview_id = invocation.preview_id().get();
    state
        .invocations
        .insert(preview_id, invocation.cancellation());

    let result = invocation.finish().await;
    state.invocations.remove(&preview_id);

    let preview = result.map_err(|e| AppCommandError::VerbError {
        message: e.to_string(),
    })?;

    let tag_names = preview
        .output
        .effects
        .into_iter()
        .find_map(|e| {
            if let VerbEffect::SuggestEntityTags { tag_names, .. } = e {
                Some(tag_names)
            } else {
                None
            }
        })
        .unwrap_or_default();

    // Phase 3: write lock — persist suggested tags to the entity.
    let mut doc = state.doc.write().await;
    let mut next_id = doc.next_id;
    let project = doc
        .project
        .as_mut()
        .ok_or(AppCommandError::NoActiveProject)?;
    let suggestions =
        apply_suggest_entity_tags(project, &mut next_id, entity_id, tag_names, now_secs())?;
    doc.next_id = next_id;
    doc.dirty = true;
    Ok(suggestions)
}

/// Moves a tag from `entity.ai.suggested_tags` to `entity.tags`, confirming
/// the VLM suggestion.
#[tauri::command(async, rename_all = "snake_case")]
pub async fn library_accept_suggested_tag(
    entity_id: EntityId,
    tag_id: TagId,
    state: State<'_, AppState>,
) -> CommandResult<()> {
    let ts = now_secs();
    let mut doc = state.doc.write().await;
    let project = doc
        .project
        .as_mut()
        .ok_or(AppCommandError::NoActiveProject)?;

    // Validate: the tag must currently be in the entity's suggested set —
    // accepting an arbitrary tag id (one nobody suggested, or one that
    // doesn't even exist) would silently corrupt entity.tags.
    let entity = project
        .library
        .entities
        .iter_mut()
        .find(|e| e.id == entity_id)
        .ok_or_else(|| AppCommandError::NotFound {
            entity: "entity".into(),
            id: u64::from(entity_id.get()),
        })?;
    if !entity.ai.suggested_tags.contains(&tag_id) {
        return Err(AppCommandError::Validation {
            detail: format!(
                "tag {} is not in entity {}'s suggested_tags",
                tag_id.get(),
                entity_id.get(),
            ),
        });
    }
    entity.ai.suggested_tags.retain(|&id| id != tag_id);

    // Canonical attach path: validates the tag definition exists and
    // bumps `updated_at`. Returns `Ok(false)` if the tag was already on
    // the entity — fine, we want idempotence on the accept.
    tag_entity_in_project(project, entity_id, tag_id, ts)?;

    doc.dirty = true;
    Ok(())
}

/// Removes a tag from `entity.ai.suggested_tags`, dismissing the suggestion.
#[tauri::command(async, rename_all = "snake_case")]
pub async fn library_reject_suggested_tag(
    entity_id: EntityId,
    tag_id: TagId,
    state: State<'_, AppState>,
) -> CommandResult<()> {
    let ts = now_secs();
    let mut doc = state.doc.write().await;
    let project = doc
        .project
        .as_mut()
        .ok_or(AppCommandError::NoActiveProject)?;
    let entity = project
        .library
        .entities
        .iter_mut()
        .find(|e| e.id == entity_id)
        .ok_or_else(|| AppCommandError::NotFound {
            entity: "entity".into(),
            id: u64::from(entity_id.get()),
        })?;
    entity.ai.suggested_tags.retain(|&id| id != tag_id);
    entity.updated_at = ts;
    doc.dirty = true;
    Ok(())
}

/// Adds entity IDs to `ProjectAi.style_corpus`, deduplicating against what is
/// already there. Corpus management is separate from verb invocation — this
/// command does not trigger training.
#[tauri::command(async, rename_all = "snake_case")]
pub async fn library_update_corpus(
    entity_ids: Vec<EntityId>,
    state: State<'_, AppState>,
) -> CommandResult<()> {
    let mut doc = state.doc.write().await;
    let project = doc
        .project
        .as_mut()
        .ok_or(AppCommandError::NoActiveProject)?;
    apply_update_project_ai(project, entity_ids, None);
    doc.dirty = true;
    Ok(())
}

// ── B10.5: per-entity LoRA training ───────────────────────────────────────────

/// Inputs surfaced to the UI for [`library_train_entity_lora`].
///
/// Mirrors the verb's parameters minus the training images — the IPC
/// command extracts those from the entity's canonical sheet so the UI
/// never has to ship the bytes back to Rust.
#[derive(Debug, Deserialize)]
pub struct TrainEntityLoraOptions {
    /// `LoRA` rank (4-32). `None` falls back to the verb default.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub lora_rank: Option<u32>,
    /// Training step count (200-2000). `None` falls back to the verb
    /// default.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub steps: Option<u32>,
    /// Trigger word / label. `None` lets the verb derive one from the
    /// entity name.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub label: Option<String>,
    /// Override Replicate model. `None` uses the verb default.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,
}

/// Result returned by [`library_train_entity_lora`] on a successful train.
#[derive(Debug, serde::Serialize)]
pub struct LibraryTrainEntityLoraResult {
    /// Entity the weights are bound to.
    pub entity_id: EntityId,
    /// The `LoRA` path now stored on `Entity.ai.lora_path`. Currently
    /// the Replicate weights URL — downloading the safetensors into
    /// the project directory is a follow-up shared with the
    /// project-wide style training flow.
    pub lora_path: String,
    /// Trigger word the trainer used.
    pub label: String,
    /// Replicate training job ID, retained so the UI can surface it
    /// in audit traces.
    pub training_id: String,
    /// Per-invocation handle. Stringified `PreviewId` so the JS side
    /// doesn't lose precision on values above 2^53.
    pub invocation_id: String,
}

/// Decodes a PNG byte buffer into a tightly-packed RGBA8
/// [`pixhaus_ai::plugin::context::PixelData`].
///
/// The reference sheet ships as `image/png` (B10.1); this helper sits in
/// front of the verb's `PixelData`-typed inputs so the host never moves
/// PNG bytes through the protocol unnecessarily.
fn decode_reference_sheet_png(
    bytes: &[u8],
) -> Result<pixhaus_ai::plugin::context::PixelData, AppCommandError> {
    use image::ImageReader;
    use std::io::Cursor;

    let dyn_image = ImageReader::new(Cursor::new(bytes))
        .with_guessed_format()
        .map_err(|e| AppCommandError::Validation {
            detail: format!("failed to read reference sheet image header: {e}"),
        })?
        .decode()
        .map_err(|e| AppCommandError::Validation {
            detail: format!("failed to decode reference sheet image: {e}"),
        })?;
    let rgba = dyn_image.to_rgba8();
    let (w, h) = rgba.dimensions();
    Ok(pixhaus_ai::plugin::context::PixelData::rgba8(
        w,
        h,
        rgba.into_raw(),
    ))
}

/// Prepares the verb context and the decoded canonical sheet image for
/// [`library_train_entity_lora`]. Holds the read lock only for the brief
/// window needed to clone what the verb invocation will consume.
async fn build_train_entity_lora_context(
    entity_id: EntityId,
    state: &AppState,
) -> CommandResult<(
    pixhaus_ai::plugin::context::VerbContext,
    pixhaus_ai::plugin::context::PixelData,
    String,
)> {
    let doc = state.doc.read().await;
    let project = doc
        .project
        .as_ref()
        .ok_or(AppCommandError::NoActiveProject)?;
    let entity = project
        .library
        .entities
        .iter()
        .find(|e| e.id == entity_id)
        .ok_or_else(|| AppCommandError::NotFound {
            entity: "entity".into(),
            id: u64::from(entity_id.get()),
        })?;
    let sheet = match &entity.content {
        EntityContent::Sprites {
            reference_sheet: Some(sheet),
            ..
        } => sheet.as_ref(),
        _ => {
            return Err(AppCommandError::Validation {
                detail: format!(
                    "entity {} has no sprite reference sheet; train_entity_lora requires one",
                    entity_id.get(),
                ),
            });
        }
    };
    let Some(canonical) = sheet.canonical.as_ref() else {
        return Err(AppCommandError::Validation {
            detail: format!(
                "entity {} has no approved canonical reference sheet; train_entity_lora requires one",
                entity_id.get(),
            ),
        });
    };
    let decoded = decode_reference_sheet_png(&canonical.image.bytes)?;
    let ctx = VerbContextBuilder::new(project.metadata.clone())
        .with_library_entity(entity_id)
        .build();
    Ok((ctx, decoded, entity.name.clone()))
}

/// Trains a per-entity `LoRA` from a sprite entity's canonical reference sheet
/// and persists the weights URL on `Entity.ai.lora_path`.
///
/// Three-phase flow:
///
/// 1. Read lock: extract the canonical sheet bytes and decode to
///    [`pixhaus_ai::plugin::context::PixelData`].
/// 2. No lock held: dispatch the
///    `pixhaus.builtin.train_entity_lora` verb via the runtime. The
///    cancel token is registered with the in-flight invocation table on
///    `AppState` so `verb_cancel` can interrupt the 15-30 minute
///    training run.
/// 3. Write lock: apply the
///    [`pixhaus_ai::plugin::output::VerbEffect::UpdateEntityAi`] effect
///    using the verb's `weights_url`, invalidate the anchor cache for
///    the entity, and bump `entity.updated_at`.
#[tauri::command(async, rename_all = "snake_case")]
pub async fn library_train_entity_lora(
    entity_id: EntityId,
    options: Option<TrainEntityLoraOptions>,
    state: State<'_, AppState>,
) -> CommandResult<LibraryTrainEntityLoraResult> {
    use pixhaus_ai::verbs::train_entity_lora::{
        EntityLoraResult, TRAIN_ENTITY_LORA_EFFECT_NAME, TRAIN_ENTITY_LORA_VERB_ID,
        TrainEntityLoraInputs,
    };

    let opts = options.unwrap_or(TrainEntityLoraOptions {
        lora_rank: None,
        steps: None,
        label: None,
        model: None,
    });

    let (ctx, decoded, entity_label) = build_train_entity_lora_context(entity_id, &state).await?;

    let label = opts.label.clone().unwrap_or(entity_label);
    let inputs = VerbInputs::from_struct(&TrainEntityLoraInputs {
        entity_id,
        training_images: vec![decoded],
        lora_rank: opts.lora_rank,
        steps: opts.steps,
        label: Some(label.clone()),
        model: opts.model,
    })
    .map_err(|e| AppCommandError::VerbError {
        message: format!("failed to build train_entity_lora inputs: {e}"),
    })?;

    let verb_id = VerbId::new(TRAIN_ENTITY_LORA_VERB_ID);
    let invocation = state
        .verb_runtime
        .invoke(&verb_id, ctx, inputs)
        .map_err(|e| AppCommandError::VerbError {
            message: e.to_string(),
        })?;

    let preview_id = invocation.preview_id().get();
    state
        .invocations
        .insert(preview_id, invocation.cancellation());
    let result = invocation.finish().await;
    state.invocations.remove(&preview_id);

    let preview = result.map_err(|e| AppCommandError::VerbError {
        message: e.to_string(),
    })?;
    let lora_result = preview
        .output
        .effects
        .iter()
        .find_map(|e| match e {
            VerbEffect::Custom { name, payload } if name == TRAIN_ENTITY_LORA_EFFECT_NAME => {
                serde_json::from_value::<EntityLoraResult>(payload.clone()).ok()
            }
            _ => None,
        })
        .ok_or_else(|| AppCommandError::VerbError {
            message: format!(
                "train_entity_lora verb did not return a {TRAIN_ENTITY_LORA_EFFECT_NAME} effect"
            ),
        })?;

    {
        let mut doc = state.doc.write().await;
        let project = doc
            .project
            .as_mut()
            .ok_or(AppCommandError::NoActiveProject)?;
        let applied = apply_update_entity_ai(
            project,
            entity_id,
            Some(lora_result.weights_url.clone()),
            now_secs(),
        );
        if !applied {
            return Err(AppCommandError::NotFound {
                entity: "entity".into(),
                id: u64::from(entity_id.get()),
            });
        }
        doc.dirty = true;
    }
    state.anchor_cache.remove(&entity_id.get());

    Ok(LibraryTrainEntityLoraResult {
        entity_id,
        lora_path: lora_result.weights_url,
        label: lora_result.label,
        training_id: lora_result.training_id,
        invocation_id: preview_id.to_string(),
    })
}

// ── tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use base64::Engine as _;
    use pixhaus_core::project::{
        ActiveTarget, AiMetadata, AssetInfo, ColorMode, EntityContent, EntityDefaults, EntityId,
        EntityKind, GroupId, NamedSprite, ReferenceImage, ReferenceSheet, SheetComposition,
        SheetVariant, SheetVariantId, Size, StateId, TagId, UserData,
    };

    use super::*;
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
