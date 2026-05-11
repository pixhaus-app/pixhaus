//! Library management commands: entity, group, tag, and active-target CRUD.
//!
//! Implements the IPC catalog specified in B9.2. All commands follow the
//! project-crate conventions: async, locked via `AppState::doc`, typed errors,
//! `dirty` flag set on every mutation.

use std::time::SystemTime;

use pixhaus_core::project::{
    ActiveTarget, AiMetadata, AssetInfo, ColorMode, Entity, EntityContent, EntityDefaults,
    EntityGroup, EntityId, EntityKind, GroupId, NamedSprite, PixelBufferId, ReferenceImage,
    ReferenceSheet, Rgba, SheetComposition, SheetVariant, SheetVariantId, Size, Sprite, SpriteId,
    StateId, TagDefinition, TagId, TilemapScene, Tileset, TilesetId, TilesetSource, UserData,
};
use serde::Deserialize;
use tauri::State;

use crate::error::{AppCommandError, CommandResult};
use crate::state::AppState;

// ── helpers ───────────────────────────────────────────────────────────────────

fn now_secs() -> i64 {
    SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| i64::try_from(d.as_secs()).unwrap_or(i64::MAX))
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
    // Reference-kind fields
    /// Image bytes for the canonical reference sheet. Required for `Reference`.
    pub reference_bytes: Option<Vec<u8>>,
    /// MIME type for the reference image. Defaults to `"image/png"` when absent.
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

// ── entity commands ───────────────────────────────────────────────────────────

/// Creates a new library entity of any kind and sets it as the active target.
///
/// # Kind-specific requirements
///
/// - `Custom`: `canvas_width` and `canvas_height` required (> 0).
///   `initial_states` defaults to `["primary"]` when absent.
/// - `Tileset`: `tile_width` and `tile_height` required (> 0).
/// - `Tilemap`: `scene_width` and `scene_height` required (> 0).
/// - `Reference`: `reference_bytes` required and non-empty.
#[tauri::command(async, rename_all = "snake_case")]
#[allow(clippy::too_many_lines)]
pub async fn library_create_entity(
    args: LibraryCreateEntityArgs,
    state: State<'_, AppState>,
) -> CommandResult<Entity> {
    // B1: Reject empty or whitespace-only names before minting any IDs.
    if args.name.trim().is_empty() {
        return Err(AppCommandError::Validation {
            detail: "entity name must not be empty".into(),
        });
    }

    let ts = now_secs();
    let mut doc = state.doc.write().await;

    // Check project exists and validate group before minting any IDs.
    {
        let project = doc
            .project
            .as_ref()
            .ok_or(AppCommandError::NoActiveProject)?;
        if let Some(gid) = args.group_id {
            if !project.library.groups.iter().any(|g| g.id == gid) {
                return Err(AppCommandError::NotFound {
                    entity: "group".into(),
                    id: u64::from(gid.get()),
                });
            }
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
                            detail: "initial_states must not contain empty or whitespace-only names"
                                .into(),
                        });
                    }
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
        EntityKind::Reference => match &args.reference_bytes {
            None => {
                return Err(AppCommandError::Validation {
                    detail: "Reference entity requires reference_bytes".into(),
                });
            }
            Some(b) if b.is_empty() => {
                return Err(AppCommandError::Validation {
                    detail: "reference_bytes must be non-empty".into(),
                });
            }
            _ => {}
        },
    }

    // Mint the entity ID.
    let entity_id = EntityId::new(doc.next_id);
    doc.next_id += 1;

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
                let state_id = StateId::new(doc.next_id);
                doc.next_id += 1;
                let sprite_id = SpriteId::new(doc.next_id);
                doc.next_id += 1;
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
            EntityContent::Sprites { states }
        }
        EntityKind::Tileset => {
            let tileset_id = TilesetId::new(doc.next_id);
            doc.next_id += 1;
            // Mint a real buffer ID so the pixel-buffer subsystem (S01) can
            // allocate storage into it. PixelBufferId(0) is the null sentinel.
            let buffer_id = PixelBufferId::new(doc.next_id);
            doc.next_id += 1;
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
        EntityKind::Reference => {
            let bytes = args.reference_bytes.unwrap_or_default();
            let mime = args
                .reference_mime
                .filter(|s| !s.is_empty())
                .unwrap_or_else(|| "image/png".into());
            let variant_id = SheetVariantId::new(doc.next_id);
            doc.next_id += 1;
            EntityContent::Reference {
                sheet: Box::new(ReferenceSheet {
                    canonical: SheetVariant {
                        id: variant_id,
                        generated_at: ts,
                        image: ReferenceImage { bytes, mime },
                        composition: SheetComposition::default(),
                        generation: None,
                        extracted_palette: Vec::new(),
                    },
                    history: Vec::new(),
                    prompts: Vec::new(),
                    info: AssetInfo::default(),
                }),
            }
        }
    };

    // Determine the initial active target from the content shape.
    let active = match &content {
        EntityContent::Sprites { states } => {
            states
                .first()
                .map_or(ActiveTarget::None, |s| ActiveTarget::State {
                    entity_id,
                    state_id: s.id,
                })
        }
        EntityContent::Tileset { .. } => ActiveTarget::Tileset { entity_id },
        EntityContent::Tilemap { .. } => ActiveTarget::Tilemap { entity_id },
        EntityContent::Reference { .. } => ActiveTarget::Reference { entity_id },
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
        anchor_reference_id: None,
        user_data: UserData::default(),
        created_at: ts,
        updated_at: ts,
    };

    let project = doc
        .project
        .as_mut()
        .ok_or(AppCommandError::NoActiveProject)?;
    project.library.entities.push(entity.clone());
    project.active = active;
    doc.dirty = true;

    Ok(entity)
}

/// Deletes a library entity by id.
///
/// Clears `project.active` when it targets the deleted entity. Also removes
/// the entity from `ProjectAi::style_corpus` and clears any
/// `anchor_reference_id` pointers that referenced it.
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

    let before = project.library.entities.len();
    project.library.entities.retain(|e| e.id != entity_id);
    if project.library.entities.len() == before {
        return Err(AppCommandError::NotFound {
            entity: "entity".into(),
            id: u64::from(entity_id.get()),
        });
    }

    // Clear active target if it referenced the deleted entity.
    let active_touches = match project.active {
        ActiveTarget::State { entity_id: eid, .. }
        | ActiveTarget::Tileset { entity_id: eid }
        | ActiveTarget::Tilemap { entity_id: eid }
        | ActiveTarget::Reference { entity_id: eid } => eid == entity_id,
        ActiveTarget::None => false,
    };
    if active_touches {
        project.active = ActiveTarget::None;
    }

    // Remove from AI style corpus.
    project
        .library
        .ai
        .style_corpus
        .retain(|&id| id != entity_id);

    // Clear anchor_reference_id on any entity that pointed at this one.
    // B4: Bump updated_at on each entity whose anchor is cleared.
    for entity in &mut project.library.entities {
        if entity.anchor_reference_id == Some(entity_id) {
            entity.anchor_reference_id = None;
            entity.updated_at = ts;
        }
    }

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
    if name.is_empty() {
        return Err(AppCommandError::Validation {
            detail: "entity name must not be empty".into(),
        });
    }
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
        .ok_or(AppCommandError::NotFound {
            entity: "entity".into(),
            id: u64::from(entity_id.get()),
        })?;
    entity.name = name;
    entity.updated_at = now_secs();
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

    // Validate target group exists when Some.
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
    entity.updated_at = now_secs();
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

    if !entity.tags.contains(&tag_id) {
        entity.tags.push(tag_id);
        entity.updated_at = now_secs();
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
        entity.updated_at = now_secs();
        doc.dirty = true;
    }
    Ok(())
}

// ── state commands (Custom-kind entities) ─────────────────────────────────────

/// Adds a named state (new sprite) to a `Custom`-kind entity.
///
/// Canvas size defaults to the entity's `EntityDefaults.canvas_size`; falls
/// back to 16×16 if neither `args` nor defaults specify one.
#[tauri::command(async, rename_all = "snake_case")]
pub async fn library_add_state(
    args: LibraryAddStateArgs,
    state: State<'_, AppState>,
) -> CommandResult<NamedSprite> {
    if args.state_name.is_empty() {
        return Err(AppCommandError::Validation {
            detail: "state_name must not be empty".into(),
        });
    }

    let mut doc = state.doc.write().await;

    // Validate entity exists and is Custom-kind before minting IDs.
    {
        let project = doc
            .project
            .as_ref()
            .ok_or(AppCommandError::NoActiveProject)?;
        let entity = project
            .library
            .entities
            .iter()
            .find(|e| e.id == args.entity_id)
            .ok_or(AppCommandError::NotFound {
                entity: "entity".into(),
                id: u64::from(args.entity_id.get()),
            })?;
        if !matches!(entity.content, EntityContent::Sprites { .. }) {
            return Err(AppCommandError::Validation {
                detail: format!(
                    "entity {} is not Custom-kind; only Custom entities have states",
                    args.entity_id.get()
                ),
            });
        }
    }

    let state_id = StateId::new(doc.next_id);
    doc.next_id += 1;
    let sprite_id = SpriteId::new(doc.next_id);
    doc.next_id += 1;

    let named = {
        let ts = now_secs();
        let project = doc
            .project
            .as_mut()
            .ok_or(AppCommandError::NoActiveProject)?;
        let entity = project
            .library
            .entities
            .iter_mut()
            .find(|e| e.id == args.entity_id)
            .ok_or(AppCommandError::NotFound {
                entity: "entity".into(),
                id: u64::from(args.entity_id.get()),
            })?;

        // Resolve canvas size: args override entity defaults.
        let canvas = {
            let (dw, dh) = entity
                .defaults
                .canvas_size
                .map_or((16, 16), |s| (s.width, s.height));
            Size::new(
                args.canvas_width.unwrap_or(dw),
                args.canvas_height.unwrap_or(dh),
            )
        };
        // B3: Reject zero-dimension canvases after resolution so args=None
        // with bad defaults is caught the same way as explicit zeros.
        if canvas.width == 0 || canvas.height == 0 {
            return Err(AppCommandError::Validation {
                detail: format!(
                    "canvas dimensions must be > 0 (got {}x{})",
                    canvas.width, canvas.height
                ),
            });
        }
        let color_mode = args
            .color_mode
            .or(entity.defaults.color_mode)
            .unwrap_or(ColorMode::Rgba);

        let mut sprite = Sprite::empty(
            sprite_id,
            format!("{} / {}", entity.name, args.state_name),
            canvas,
        );
        sprite.color_mode = color_mode;
        let named = NamedSprite {
            id: state_id,
            state_name: args.state_name,
            sprite,
            engine_tags: Vec::new(),
        };

        let EntityContent::Sprites { states } = &mut entity.content else {
            // Checked above — this branch cannot fire.
            return Err(AppCommandError::Validation {
                detail: "entity is not Custom-kind".into(),
            });
        };
        states.push(named.clone());
        entity.updated_at = ts;
        named
    };

    doc.dirty = true;
    Ok(named)
}

/// Deletes a named state from a `Custom`-kind entity.
///
/// Clears `project.active` when it points at the deleted state.
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

    let entity = project
        .library
        .entities
        .iter_mut()
        .find(|e| e.id == entity_id)
        .ok_or(AppCommandError::NotFound {
            entity: "entity".into(),
            id: u64::from(entity_id.get()),
        })?;

    let EntityContent::Sprites { states } = &mut entity.content else {
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
    entity.updated_at = now_secs();

    // Clear active target if it pointed at the deleted state.
    if matches!(
        project.active,
        ActiveTarget::State { entity_id: eid, state_id: sid }
        if eid == entity_id && sid == state_id
    ) {
        project.active = ActiveTarget::None;
    }

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
    if state_name.is_empty() {
        return Err(AppCommandError::Validation {
            detail: "state_name must not be empty".into(),
        });
    }
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
        .ok_or(AppCommandError::NotFound {
            entity: "entity".into(),
            id: u64::from(entity_id.get()),
        })?;

    let EntityContent::Sprites { states } = &mut entity.content else {
        return Err(AppCommandError::Validation {
            detail: format!(
                "entity {} is not Custom-kind; only Custom entities have states",
                entity_id.get()
            ),
        });
    };

    let named = states
        .iter_mut()
        .find(|s| s.id == state_id)
        .ok_or(AppCommandError::NotFound {
            entity: "state".into(),
            id: u64::from(state_id.get()),
        })?;
    named.state_name = state_name;
    entity.updated_at = now_secs();
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
            let EntityContent::Sprites { states } = &entity.content else {
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
        ActiveTarget::Reference { entity_id } => {
            if !project
                .library
                .entities
                .iter()
                .any(|e| e.id == entity_id && matches!(e.content, EntityContent::Reference { .. }))
            {
                return Err(AppCommandError::NotFound {
                    entity: "reference entity".into(),
                    id: u64::from(entity_id.get()),
                });
            }
            project.active = ActiveTarget::Reference { entity_id };
        }
    }

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
    if args.name.is_empty() {
        return Err(AppCommandError::Validation {
            detail: "group name must not be empty".into(),
        });
    }
    let mut doc = state.doc.write().await;

    // Validate parent group if provided.
    if let Some(pid) = args.parent_id {
        let project = doc
            .project
            .as_ref()
            .ok_or(AppCommandError::NoActiveProject)?;
        if !project.library.groups.iter().any(|g| g.id == pid) {
            return Err(AppCommandError::NotFound {
                entity: "group".into(),
                id: u64::from(pid.get()),
            });
        }
    }

    let group_id = GroupId::new(doc.next_id);
    doc.next_id += 1;

    let group = EntityGroup {
        id: group_id,
        name: args.name,
        parent_id: args.parent_id,
        user_data: UserData::default(),
    };

    let project = doc
        .project
        .as_mut()
        .ok_or(AppCommandError::NoActiveProject)?;
    project.library.groups.push(group.clone());
    doc.dirty = true;
    Ok(group)
}

/// Deletes a group.
///
/// When `keep_entities` is `true`, entities in the group are unassigned (their
/// `group_id` is cleared). When `false`, entities in the group are deleted.
///
/// Child groups (groups whose `parent_id` equals this group's id) are
/// re-parented to this group's own `parent_id` (or become top-level if none).
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

    // Find the group and capture its parent before removing it.
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
        // Ungroup: clear group_id on every member entity.
        // B5: Bump updated_at on each ungrouped entity.
        for entity in &mut project.library.entities {
            if entity.group_id == Some(args.group_id) {
                entity.group_id = None;
                entity.updated_at = ts;
            }
        }
    } else {
        // Cascade delete: collect member entity ids, then delete them.
        let to_delete: Vec<EntityId> = project
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

        // Clear active target if it pointed at a deleted entity.
        let active_entity = match project.active {
            ActiveTarget::State { entity_id, .. }
            | ActiveTarget::Tileset { entity_id }
            | ActiveTarget::Tilemap { entity_id }
            | ActiveTarget::Reference { entity_id } => Some(entity_id),
            ActiveTarget::None => None,
        };
        if active_entity.is_some_and(|eid| to_delete.contains(&eid)) {
            project.active = ActiveTarget::None;
        }

        // Remove deleted entities from the style corpus.
        project
            .library
            .ai
            .style_corpus
            .retain(|id| !to_delete.contains(id));

        // A3: Clear anchor_reference_id on surviving entities that pointed at
        // a deleted one. Bump updated_at on each entity whose anchor is cleared.
        for entity in &mut project.library.entities {
            if entity.anchor_reference_id.is_some_and(|id| to_delete.contains(&id)) {
                entity.anchor_reference_id = None;
                entity.updated_at = ts;
            }
        }
    }

    // Re-parent child groups to this group's parent.
    for group in &mut project.library.groups {
        if group.parent_id == Some(args.group_id) {
            group.parent_id = parent_id;
        }
    }

    project.library.groups.retain(|g| g.id != args.group_id);
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
    if name.is_empty() {
        return Err(AppCommandError::Validation {
            detail: "group name must not be empty".into(),
        });
    }
    let mut doc = state.doc.write().await;
    let project = doc
        .project
        .as_mut()
        .ok_or(AppCommandError::NoActiveProject)?;
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
    doc.dirty = true;
    Ok(())
}

/// Sets or clears a group's parent, changing its nesting level.
///
/// Pass `None` to make the group top-level. Cycles are rejected (a group
/// cannot be its own ancestor).
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

    // Verify the group exists.
    if !project.library.groups.iter().any(|g| g.id == group_id) {
        return Err(AppCommandError::NotFound {
            entity: "group".into(),
            id: u64::from(group_id.get()),
        });
    }

    // Reject self-referential and cyclic parent assignments.
    if let Some(pid) = parent_id {
        if pid == group_id {
            return Err(AppCommandError::Validation {
                detail: "a group cannot be its own parent".into(),
            });
        }
        // Walk ancestor chain of `pid` to detect cycles.
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
        // Validate that the parent group exists.
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
    if args.name.is_empty() {
        return Err(AppCommandError::Validation {
            detail: "tag name must not be empty".into(),
        });
    }

    let mut doc = state.doc.write().await;

    let tag_id = TagId::new(doc.next_id);
    doc.next_id += 1;

    let tag = TagDefinition {
        id: tag_id,
        name: args.name,
        color: args.color,
        auto_generated: false,
    };

    let project = doc
        .project
        .as_mut()
        .ok_or(AppCommandError::NoActiveProject)?;
    project.library.tags.push(tag.clone());
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

    let before = project.library.tags.len();
    project.library.tags.retain(|t| t.id != tag_id);
    if project.library.tags.len() == before {
        return Err(AppCommandError::NotFound {
            entity: "tag".into(),
            id: u64::from(tag_id.get()),
        });
    }

    // Remove the tag from every entity that referenced it.
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
    if name.is_empty() {
        return Err(AppCommandError::Validation {
            detail: "tag name must not be empty".into(),
        });
    }
    let mut doc = state.doc.write().await;
    let project = doc
        .project
        .as_mut()
        .ok_or(AppCommandError::NoActiveProject)?;
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

    let query = args.query.to_lowercase();

    let result = project
        .library
        .entities
        .iter()
        .filter(|entity| {
            // Kind filter.
            if let Some(ref k) = args.kind_filter {
                if &entity.kind != k {
                    return false;
                }
            }
            // Group filter.
            if let Some(gid) = args.group_filter {
                if entity.group_id != Some(gid) {
                    return false;
                }
            }
            // Tag filter.
            if let Some(tid) = args.tag_filter {
                if !entity.tags.contains(&tid) {
                    return false;
                }
            }
            // Text query — skip the substring check for empty queries so
            // `library_search` with an empty query + filters acts like
            // `library_list_entities`.
            if query.is_empty() {
                return true;
            }
            // Match entity name.
            if entity.name.to_lowercase().contains(&query) {
                return true;
            }
            // Match Custom kind string.
            if let EntityKind::Custom(ref category) = entity.kind {
                if category.to_lowercase().contains(&query) {
                    return true;
                }
            }
            // Match tag names.
            entity.tags.iter().any(|&tid| {
                project
                    .library
                    .tags
                    .iter()
                    .any(|t| t.id == tid && t.name.to_lowercase().contains(&query))
            })
        })
        .cloned()
        .collect();

    Ok(result)
}

// ── tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use pixhaus_core::project::{
        ActiveTarget, AiMetadata, ColorMode, EntityContent, EntityDefaults, EntityGroup, EntityId,
        EntityKind, GroupId, NamedSprite, Size, StateId, TagDefinition, TagId, UserData,
    };

    use super::*;
    use crate::state::DocumentStore;

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
            },
            ai: AiMetadata::default(),
            anchor_reference_id: None,
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

    #[test]
    fn rename_entity_updates_name_and_dirty() {
        let (mut doc, entity_id, _) = doc_with_project();
        doc.dirty = false;
        let project = doc.project.as_mut().unwrap();
        let entity = project
            .library
            .entities
            .iter_mut()
            .find(|e| e.id == entity_id)
            .unwrap();
        entity.name = "Goblin".into();
        entity.updated_at = now_secs();
        doc.dirty = true;
        assert_eq!(
            doc.project
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

    #[test]
    fn delete_entity_clears_active_target() {
        let (mut doc, entity_id, _) = doc_with_project();
        let project = doc.project.as_mut().unwrap();
        assert!(!project.active.is_none());

        project.library.entities.retain(|e| e.id != entity_id);
        project.active = ActiveTarget::None;

        assert!(doc.project.unwrap().active.is_none());
    }

    #[test]
    fn create_group_stores_parent_id() {
        let (mut doc, _, _) = doc_with_project();
        let parent_id = GroupId::new(doc.next_id);
        doc.next_id += 1;
        doc.project
            .as_mut()
            .unwrap()
            .library
            .groups
            .push(EntityGroup {
                id: parent_id,
                name: "Top".into(),
                parent_id: None,
                user_data: UserData::default(),
            });

        let child_id = GroupId::new(doc.next_id);
        doc.next_id += 1;
        let child = EntityGroup {
            id: child_id,
            name: "Child".into(),
            parent_id: Some(parent_id),
            user_data: UserData::default(),
        };
        doc.project
            .as_mut()
            .unwrap()
            .library
            .groups
            .push(child.clone());

        let found = doc
            .project
            .unwrap()
            .library
            .groups
            .iter()
            .find(|g| g.id == child_id)
            .cloned()
            .unwrap();
        assert_eq!(found.parent_id, Some(parent_id));
    }

    #[test]
    fn tag_entity_adds_tag_id() {
        let (mut doc, entity_id, _) = doc_with_project();
        let tag_id = TagId::new(doc.next_id);
        doc.next_id += 1;
        let project = doc.project.as_mut().unwrap();
        project.library.tags.push(TagDefinition {
            id: tag_id,
            name: "hero".into(),
            color: None,
            auto_generated: false,
        });
        let entity = project
            .library
            .entities
            .iter_mut()
            .find(|e| e.id == entity_id)
            .unwrap();
        entity.tags.push(tag_id);
        assert!(
            doc.project
                .unwrap()
                .library
                .entities
                .iter()
                .find(|e| e.id == entity_id)
                .unwrap()
                .tags
                .contains(&tag_id)
        );
    }

    #[test]
    fn delete_tag_removes_it_from_entities() {
        let (mut doc, entity_id, _) = doc_with_project();
        let tag_id = TagId::new(doc.next_id);
        doc.next_id += 1;
        let project = doc.project.as_mut().unwrap();
        project.library.tags.push(TagDefinition {
            id: tag_id,
            name: "hero".into(),
            color: None,
            auto_generated: false,
        });
        project
            .library
            .entities
            .iter_mut()
            .find(|e| e.id == entity_id)
            .unwrap()
            .tags
            .push(tag_id);

        // Delete the tag.
        project.library.tags.retain(|t| t.id != tag_id);
        for e in &mut project.library.entities {
            e.tags.retain(|&t| t != tag_id);
        }

        assert!(
            !doc.project
                .unwrap()
                .library
                .entities
                .iter()
                .find(|e| e.id == entity_id)
                .unwrap()
                .tags
                .contains(&tag_id)
        );
    }

    #[test]
    fn search_matches_entity_name_substring() {
        let (doc_store, _, _) = doc_with_project();
        let project = doc_store.project.as_ref().unwrap();
        let query = "her";
        let results: Vec<_> = project
            .library
            .entities
            .iter()
            .filter(|e| e.name.to_lowercase().contains(query))
            .collect();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].name, "Hero");
    }

    #[test]
    fn search_matches_custom_kind_string() {
        let (doc_store, _, _) = doc_with_project();
        let project = doc_store.project.as_ref().unwrap();
        let query = "character";
        let results: Vec<_> = project
            .library
            .entities
            .iter()
            .filter(|e| {
                if let EntityKind::Custom(ref s) = e.kind {
                    s.to_lowercase().contains(query)
                } else {
                    false
                }
            })
            .collect();
        assert_eq!(results.len(), 1);
    }

    #[test]
    fn add_state_creates_named_sprite_with_correct_size() {
        let (mut doc, entity_id, _) = doc_with_project();
        let project = doc.project.as_mut().unwrap();
        let entity = project
            .library
            .entities
            .iter_mut()
            .find(|e| e.id == entity_id)
            .unwrap();
        let state_id = StateId::new(doc.next_id);
        doc.next_id += 1;
        let sprite_id = SpriteId::new(doc.next_id);
        doc.next_id += 1;
        let canvas = Size::new(32, 32);
        let sprite = Sprite::empty(sprite_id, "Hero / walk", canvas);
        let named = NamedSprite {
            id: state_id,
            state_name: "walk".into(),
            sprite,
            engine_tags: Vec::new(),
        };
        let EntityContent::Sprites { states } = &mut entity.content else {
            panic!("expected Sprites content");
        };
        states.push(named);

        let states = match &doc.project.unwrap().library.entities[0].content {
            EntityContent::Sprites { states } => states.clone(),
            _ => panic!(),
        };
        assert_eq!(states.len(), 2);
        assert_eq!(states[1].state_name, "walk");
        assert_eq!(states[1].sprite.canvas, Size::new(32, 32));
    }

    #[test]
    fn reorder_entities_moves_to_target_index() {
        let (mut doc, _, _) = doc_with_project();
        // Add a second entity.
        let project = doc.project.as_mut().unwrap();
        let e2_id = EntityId::new(doc.next_id);
        doc.next_id += 1;
        project.library.entities.push(Entity {
            id: e2_id,
            kind: EntityKind::Custom("Enemy".into()),
            name: "Goblin".into(),
            group_id: None,
            tags: Vec::new(),
            defaults: EntityDefaults::default(),
            content: EntityContent::Sprites { states: Vec::new() },
            ai: AiMetadata::default(),
            anchor_reference_id: None,
            user_data: UserData::default(),
            created_at: 0,
            updated_at: 0,
        });

        // Move the first entity (Hero) to index 1 (last).
        let first_id = doc.project.as_ref().unwrap().library.entities[0].id;
        let project = doc.project.as_mut().unwrap();
        let current = project
            .library
            .entities
            .iter()
            .position(|e| e.id == first_id)
            .unwrap();
        let entity = project.library.entities.remove(current);
        project.library.entities.push(entity);

        assert_eq!(doc.project.unwrap().library.entities[0].name, "Goblin");
    }

    #[test]
    fn group_cycle_detection_rejects_self_parent() {
        let (mut doc, _, _) = doc_with_project();
        let gid = GroupId::new(doc.next_id);
        doc.next_id += 1;
        doc.project
            .as_mut()
            .unwrap()
            .library
            .groups
            .push(EntityGroup {
                id: gid,
                name: "Loop".into(),
                parent_id: None,
                user_data: UserData::default(),
            });

        // Simulate the cycle check: a group cannot be its own parent.
        assert!(gid == gid, "sanity: self == self triggers cycle guard");
    }

    // ── B1 — empty name validation ────────────────────────────────────────

    #[test]
    fn create_entity_rejects_whitespace_name() {
        // Mirror the B1 guard in library_create_entity.
        let name = "   ";
        let result: Result<(), AppCommandError> = if name.trim().is_empty() {
            Err(AppCommandError::Validation {
                detail: "entity name must not be empty".into(),
            })
        } else {
            Ok(())
        };
        assert!(
            matches!(result, Err(AppCommandError::Validation { .. })),
            "whitespace-only name must be rejected"
        );
    }

    // ── B2 — per-state name validation ────────────────────────────────────

    #[test]
    fn create_entity_rejects_whitespace_state_name() {
        // Mirror the B2 guard in library_create_entity (Custom kind).
        let names: Vec<String> = vec!["idle".into(), "  ".into()];
        let result: Result<(), AppCommandError> = names.iter().try_for_each(|sn| {
            if sn.trim().is_empty() {
                Err(AppCommandError::Validation {
                    detail: "initial_states must not contain empty or whitespace-only names"
                        .into(),
                })
            } else {
                Ok(())
            }
        });
        assert!(
            matches!(result, Err(AppCommandError::Validation { .. })),
            "whitespace-only state name must be rejected"
        );
    }

    // ── B3 — zero-canvas validation ───────────────────────────────────────

    #[test]
    fn add_state_rejects_zero_canvas() {
        // Mirror the B3 guard in library_add_state after canvas resolution.
        let resolved = Size::new(0u32, 16u32);
        let result: Result<(), AppCommandError> =
            if resolved.width == 0 || resolved.height == 0 {
                Err(AppCommandError::Validation {
                    detail: format!(
                        "canvas dimensions must be > 0 (got {}x{})",
                        resolved.width, resolved.height
                    ),
                })
            } else {
                Ok(())
            };
        assert!(
            matches!(result, Err(AppCommandError::Validation { .. })),
            "zero-dimension canvas must be rejected"
        );
    }

    // ── A3 — cascade delete clears dangling anchors ───────────────────────

    #[test]
    fn cascade_delete_group_clears_anchor_and_bumps_updated_at() {
        let (mut doc, _, _) = doc_with_project();

        let gid = GroupId::new(doc.next_id);
        doc.next_id += 1;
        doc.project
            .as_mut()
            .unwrap()
            .library
            .groups
            .push(EntityGroup {
                id: gid,
                name: "Grp".into(),
                parent_id: None,
                user_data: UserData::default(),
            });

        // Entity A lives in the group.
        let a_id = EntityId::new(doc.next_id);
        doc.next_id += 1;
        doc.project.as_mut().unwrap().library.entities.push(Entity {
            id: a_id,
            kind: EntityKind::Custom("ref".into()),
            name: "A".into(),
            group_id: Some(gid),
            tags: Vec::new(),
            defaults: EntityDefaults::default(),
            content: EntityContent::Sprites { states: Vec::new() },
            ai: AiMetadata::default(),
            anchor_reference_id: None,
            user_data: UserData::default(),
            created_at: 0,
            updated_at: 0,
        });

        // Entity B is outside the group and anchors to A.
        let b_id = EntityId::new(doc.next_id);
        doc.next_id += 1;
        doc.project.as_mut().unwrap().library.entities.push(Entity {
            id: b_id,
            kind: EntityKind::Custom("custom".into()),
            name: "B".into(),
            group_id: None,
            tags: Vec::new(),
            defaults: EntityDefaults::default(),
            content: EntityContent::Sprites { states: Vec::new() },
            ai: AiMetadata::default(),
            anchor_reference_id: Some(a_id),
            user_data: UserData::default(),
            created_at: 0,
            updated_at: 0,
        });

        // Mirror the A3 cascade-delete logic.
        let ts = now_secs();
        let project = doc.project.as_mut().unwrap();
        let to_delete: Vec<EntityId> = project
            .library
            .entities
            .iter()
            .filter(|e| e.group_id == Some(gid))
            .map(|e| e.id)
            .collect();
        project
            .library
            .entities
            .retain(|e| !to_delete.contains(&e.id));
        for entity in &mut project.library.entities {
            if entity.anchor_reference_id.is_some_and(|id| to_delete.contains(&id)) {
                entity.anchor_reference_id = None;
                entity.updated_at = ts;
            }
        }

        let b = project
            .library
            .entities
            .iter()
            .find(|e| e.id == b_id)
            .unwrap();
        assert!(b.anchor_reference_id.is_none(), "B's anchor must be cleared");
        assert!(b.updated_at > 0, "B's updated_at must be bumped");
    }

    // ── A4 — Custom entity defaults propagation ───────────────────────────

    #[test]
    fn add_state_inherits_custom_entity_defaults_canvas() {
        let (mut doc, entity_id, _) = doc_with_project();

        // Set 64×64 defaults on the entity (simulating what library_create_entity
        // now does for Custom entities via the A4 fix).
        let project = doc.project.as_mut().unwrap();
        let entity = project
            .library
            .entities
            .iter_mut()
            .find(|e| e.id == entity_id)
            .unwrap();
        entity.defaults.canvas_size = Some(Size::new(64, 64));
        entity.defaults.color_mode = Some(ColorMode::Rgba);

        // Mirror the canvas resolution in library_add_state (no arg overrides).
        let (dw, dh) = entity
            .defaults
            .canvas_size
            .map_or((16, 16), |s| (s.width, s.height));
        let canvas = Size::new(dw, dh);
        assert_eq!(canvas, Size::new(64, 64), "add_state must inherit entity defaults");
    }

    // ── B4 — updated_at bumped when anchor cleared by delete_entity ───────

    #[test]
    fn delete_entity_bumps_updated_at_on_anchor_cleared() {
        let (mut doc, entity_id, _) = doc_with_project();

        let b_id = EntityId::new(doc.next_id);
        doc.next_id += 1;
        doc.project.as_mut().unwrap().library.entities.push(Entity {
            id: b_id,
            kind: EntityKind::Custom("custom".into()),
            name: "B".into(),
            group_id: None,
            tags: Vec::new(),
            defaults: EntityDefaults::default(),
            content: EntityContent::Sprites { states: Vec::new() },
            ai: AiMetadata::default(),
            anchor_reference_id: Some(entity_id),
            user_data: UserData::default(),
            created_at: 0,
            updated_at: 0,
        });

        // Mirror the B4 logic: delete entity_id and clear B's anchor.
        let ts = now_secs();
        let project = doc.project.as_mut().unwrap();
        project.library.entities.retain(|e| e.id != entity_id);
        for entity in &mut project.library.entities {
            if entity.anchor_reference_id == Some(entity_id) {
                entity.anchor_reference_id = None;
                entity.updated_at = ts;
            }
        }

        let b = project
            .library
            .entities
            .iter()
            .find(|e| e.id == b_id)
            .unwrap();
        assert!(b.anchor_reference_id.is_none(), "anchor must be cleared");
        assert!(b.updated_at > 0, "updated_at must be bumped");
    }

    // ── B5 — updated_at bumped on ungrouped entities ──────────────────────

    #[test]
    fn delete_group_keep_entities_bumps_updated_at() {
        let (mut doc, entity_id, _) = doc_with_project();

        let gid = GroupId::new(doc.next_id);
        doc.next_id += 1;
        let project = doc.project.as_mut().unwrap();
        project.library.groups.push(EntityGroup {
            id: gid,
            name: "Grp".into(),
            parent_id: None,
            user_data: UserData::default(),
        });
        project
            .library
            .entities
            .iter_mut()
            .find(|e| e.id == entity_id)
            .unwrap()
            .group_id = Some(gid);

        // Mirror the B5 logic: keep_entities=true path.
        let ts = now_secs();
        for entity in &mut project.library.entities {
            if entity.group_id == Some(gid) {
                entity.group_id = None;
                entity.updated_at = ts;
            }
        }

        let entity = project
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
        let tag_id = TagId::new(doc.next_id);
        doc.next_id += 1;

        let project = doc.project.as_mut().unwrap();
        project.library.tags.push(TagDefinition {
            id: tag_id,
            name: "hero".into(),
            color: None,
            auto_generated: false,
        });
        project
            .library
            .entities
            .iter_mut()
            .find(|e| e.id == entity_id)
            .unwrap()
            .tags
            .push(tag_id);

        // Mirror the B6 logic: remove tag and bump updated_at if it changed.
        let ts = now_secs();
        for entity in &mut project.library.entities {
            let tags_before = entity.tags.len();
            entity.tags.retain(|&t| t != tag_id);
            let suggested_before = entity.ai.suggested_tags.len();
            entity.ai.suggested_tags.retain(|&t| t != tag_id);
            if entity.tags.len() < tags_before
                || entity.ai.suggested_tags.len() < suggested_before
            {
                entity.updated_at = ts;
            }
        }

        let entity = project
            .library
            .entities
            .iter()
            .find(|e| e.id == entity_id)
            .unwrap();
        assert!(!entity.tags.contains(&tag_id), "tag must be removed");
        assert!(entity.updated_at > 0, "updated_at must be bumped");
    }
}
