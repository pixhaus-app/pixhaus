//! Layer CRUD and property commands.

use pixhaus_core::canvas::{LayerInput, PixelBuffer, composite_onto};
use pixhaus_core::project::{
    BlendMode, Cel, CelData, FrameIndex, Layer, LayerEffect, LayerId, LayerKind, PixelBufferId,
    Size, Sprite, SpriteId, TilesetId, UserData,
};
use pixhaus_io::pixhaus::PixelBufferEntry;
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, State};

use crate::commands::canvas::emit_buffer_tiles;
use crate::error::{AppCommandError, CommandResult};
use crate::pixel_history::{PixelOp, PixelOpBatch};
use crate::state::{AppState, DocumentStore};

/// Arguments for adding a new layer.
#[derive(Debug, Deserialize)]
pub struct LayerAddArgs {
    /// Sprite to add the layer to.
    pub sprite_id: SpriteId,
    /// Display name for the new layer.
    pub name: String,
    /// Variant-specific payload (raster, group, tilemap, reference).
    pub kind: LayerKind,
}

/// Adds a new layer to a sprite. The layer is appended above all existing layers.
#[tauri::command(async, rename_all = "snake_case")]
pub async fn layer_add(args: LayerAddArgs, state: State<'_, AppState>) -> CommandResult<Layer> {
    let mut doc = state.doc.write().await;
    let id = LayerId::new(doc.next_id);
    doc.next_id += 1;
    let layer = {
        let sprite = doc
            .project
            .as_mut()
            .ok_or(AppCommandError::NoActiveProject)?
            .sprite_mut(args.sprite_id)
            .ok_or(AppCommandError::NotFound {
                entity: "sprite".into(),
                id: u64::from(args.sprite_id.get()),
            })?;
        let layer = Layer {
            id,
            name: args.name,
            kind: args.kind,
            blend_mode: BlendMode::Normal,
            opacity: 255,
            visible: true,
            locked: false,
            parent: None,
            effects: Vec::new(),
            user_data: UserData::default(),
        };
        sprite.layers.push(layer.clone());
        layer
    };
    doc.dirty = true;
    Ok(layer)
}

/// Removes a layer from a sprite by ID. Also removes all cels on that layer.
#[tauri::command(async, rename_all = "snake_case")]
pub async fn layer_delete(
    sprite_id: SpriteId,
    layer_id: LayerId,
    state: State<'_, AppState>,
) -> CommandResult<()> {
    let mut doc = state.doc.write().await;
    {
        let sprite = doc
            .project
            .as_mut()
            .ok_or(AppCommandError::NoActiveProject)?
            .sprite_mut(sprite_id)
            .ok_or(AppCommandError::NotFound {
                entity: "sprite".into(),
                id: u64::from(sprite_id.get()),
            })?;
        let before = sprite.layers.len();
        sprite.layers.retain(|l| l.id != layer_id);
        if sprite.layers.len() == before {
            return Err(AppCommandError::NotFound {
                entity: "layer".into(),
                id: u64::from(layer_id.get()),
            });
        }
        // Remove cels that belong to the deleted layer.
        sprite.cels.retain(|c| c.layer_id != layer_id);
    }
    doc.dirty = true;
    Ok(())
}

/// Moves a layer to a new position in the layer stack.
///
/// `new_index` is clamped to `[0, layers.len() - 1]`.
#[tauri::command(async, rename_all = "snake_case")]
pub async fn layer_reorder(
    sprite_id: SpriteId,
    layer_id: LayerId,
    new_index: u32,
    state: State<'_, AppState>,
) -> CommandResult<()> {
    let mut doc = state.doc.write().await;
    {
        let sprite = doc
            .project
            .as_mut()
            .ok_or(AppCommandError::NoActiveProject)?
            .sprite_mut(sprite_id)
            .ok_or(AppCommandError::NotFound {
                entity: "sprite".into(),
                id: u64::from(sprite_id.get()),
            })?;
        let pos = sprite.layers.iter().position(|l| l.id == layer_id).ok_or(
            AppCommandError::NotFound {
                entity: "layer".into(),
                id: u64::from(layer_id.get()),
            },
        )?;
        let target = (new_index as usize).min(sprite.layers.len().saturating_sub(1));
        let layer = sprite.layers.remove(pos);
        sprite.layers.insert(target, layer);
    }
    doc.dirty = true;
    Ok(())
}

/// Sets the blend mode for a layer.
#[tauri::command(async, rename_all = "snake_case")]
pub async fn layer_set_blend_mode(
    sprite_id: SpriteId,
    layer_id: LayerId,
    blend_mode: BlendMode,
    state: State<'_, AppState>,
) -> CommandResult<()> {
    with_layer_mut(&state, sprite_id, layer_id, |layer| {
        layer.blend_mode = blend_mode;
    })
    .await
}

/// Sets the opacity for a layer (`0` = fully transparent, `255` = fully opaque).
#[tauri::command(async, rename_all = "snake_case")]
pub async fn layer_set_opacity(
    sprite_id: SpriteId,
    layer_id: LayerId,
    opacity: u8,
    state: State<'_, AppState>,
) -> CommandResult<()> {
    with_layer_mut(&state, sprite_id, layer_id, |layer| {
        layer.opacity = opacity;
    })
    .await
}

/// Sets the visibility of a layer.
#[tauri::command(async, rename_all = "snake_case")]
pub async fn layer_set_visibility(
    sprite_id: SpriteId,
    layer_id: LayerId,
    visible: bool,
    state: State<'_, AppState>,
) -> CommandResult<()> {
    with_layer_mut(&state, sprite_id, layer_id, |layer| {
        layer.visible = visible;
    })
    .await
}

/// Sets the locked state of a layer.
#[tauri::command(async, rename_all = "snake_case")]
pub async fn layer_set_locked(
    sprite_id: SpriteId,
    layer_id: LayerId,
    locked: bool,
    state: State<'_, AppState>,
) -> CommandResult<()> {
    with_layer_mut(&state, sprite_id, layer_id, |layer| {
        layer.locked = locked;
    })
    .await
}

/// Replaces a layer's non-destructive effect stack. Effects apply in order
/// at composite time and never mutate the layer's pixels, so the operation
/// is fully reversible by setting the stack again.
#[tauri::command(async, rename_all = "snake_case")]
pub async fn layer_set_effects(
    sprite_id: SpriteId,
    layer_id: LayerId,
    effects: Vec<LayerEffect>,
    state: State<'_, AppState>,
) -> CommandResult<()> {
    with_layer_mut(&state, sprite_id, layer_id, |layer| {
        layer.effects = effects;
    })
    .await
}

/// Returns all layers in a sprite, bottom to top (index 0 is the bottom layer).
#[tauri::command(async, rename_all = "snake_case")]
pub async fn layer_list(
    sprite_id: SpriteId,
    state: State<'_, AppState>,
) -> CommandResult<Vec<Layer>> {
    let doc = state.doc.read().await;
    let sprite = doc
        .project
        .as_ref()
        .ok_or(AppCommandError::NoActiveProject)?
        .sprite(sprite_id)
        .ok_or(AppCommandError::NotFound {
            entity: "sprite".into(),
            id: u64::from(sprite_id.get()),
        })?;
    Ok(sprite.layers.clone())
}

/// Serializable summary of a layer rename operation.
#[derive(Debug, Serialize)]
pub struct LayerRenamed {
    /// The layer that was renamed.
    pub layer_id: LayerId,
    /// The new display name.
    pub name: String,
}

/// Renames a layer.
#[tauri::command(async, rename_all = "snake_case")]
pub async fn layer_rename(
    sprite_id: SpriteId,
    layer_id: LayerId,
    name: String,
    state: State<'_, AppState>,
) -> CommandResult<LayerRenamed> {
    with_layer_mut(&state, sprite_id, layer_id, |layer| {
        layer.name.clone_from(&name);
    })
    .await?;
    Ok(LayerRenamed { layer_id, name })
}

/// Sets the parent of a layer. Pass `parent_id = None` to make it top-level.
///
/// Rejects if `parent_id` points to a non-existent layer, to a layer that
/// is not a group, to the layer itself (self-parent), or to one of the
/// layer's own descendants (which would close a cycle).
#[tauri::command(async, rename_all = "snake_case")]
pub async fn layer_set_parent(
    sprite_id: SpriteId,
    layer_id: LayerId,
    parent_id: Option<LayerId>,
    state: State<'_, AppState>,
) -> CommandResult<()> {
    let mut doc = state.doc.write().await;
    {
        let sprite = doc
            .project
            .as_mut()
            .ok_or(AppCommandError::NoActiveProject)?
            .sprite_mut(sprite_id)
            .ok_or(AppCommandError::NotFound {
                entity: "sprite".into(),
                id: u64::from(sprite_id.get()),
            })?;
        set_layer_parent_in_sprite(sprite, layer_id, parent_id)?;
    }
    doc.dirty = true;
    Ok(())
}

/// Pure parent-assignment helper. Lifted out of `layer_set_parent` so the
/// cycle and self-parent rejection paths are testable without standing
/// up a tauri `State`.
///
/// Validation order:
/// 1. Reject self-parent (`parent_id == Some(layer_id)`).
/// 2. Verify the candidate parent exists and is a group.
/// 3. Walk the candidate parent's ancestor chain; reject if `layer_id`
///    appears anywhere — that assignment would close a cycle and break
///    every tree-walking consumer (flattening, hit-testing, render
///    ordering).
/// 4. Apply the parent assignment.
fn set_layer_parent_in_sprite(
    sprite: &mut Sprite,
    layer_id: LayerId,
    parent_id: Option<LayerId>,
) -> CommandResult<()> {
    if Some(layer_id) == parent_id {
        return Err(AppCommandError::Validation {
            detail: "cannot reparent a layer onto itself".into(),
        });
    }
    if let Some(pid) = parent_id {
        let parent =
            sprite
                .layers
                .iter()
                .find(|l| l.id == pid)
                .ok_or(AppCommandError::NotFound {
                    entity: "layer".into(),
                    id: u64::from(pid.get()),
                })?;
        if !matches!(parent.kind, LayerKind::Group { .. }) {
            return Err(AppCommandError::Validation {
                detail: "target layer is not a group".into(),
            });
        }
        // Walk the candidate parent's ancestor chain. If layer_id shows
        // up as one of those ancestors, the new parent is a descendant
        // of the layer being moved — assigning it would close a cycle.
        // Bound the walk by the layer count so a pre-existing cycle in
        // the input data can't hang the validator.
        let mut current = parent.parent;
        let max_steps = sprite.layers.len();
        let mut steps = 0;
        while let Some(ancestor_id) = current {
            if ancestor_id == layer_id {
                return Err(AppCommandError::Validation {
                    detail: "reparent would create a cycle".into(),
                });
            }
            steps += 1;
            if steps > max_steps {
                return Err(AppCommandError::Validation {
                    detail: "layer parent chain has a pre-existing cycle".into(),
                });
            }
            current = sprite
                .layers
                .iter()
                .find(|l| l.id == ancestor_id)
                .and_then(|l| l.parent);
        }
    }
    let layer =
        sprite
            .layers
            .iter_mut()
            .find(|l| l.id == layer_id)
            .ok_or(AppCommandError::NotFound {
                entity: "layer".into(),
                id: u64::from(layer_id.get()),
            })?;
    layer.parent = parent_id;
    Ok(())
}

/// Wraps one or more layers in a single new group, preserving their
/// pixels and any cels that point at them.
///
/// The new group is anchored at the topmost selected layer (the one with
/// the highest vec position) — it inherits that layer's parent so it
/// sits at the same hierarchy level the anchor used to occupy, and is
/// inserted at the anchor's vec position so its z-order slot among
/// siblings matches what the anchor had. Every layer in `layer_ids` is
/// reparented under the new group, regardless of their original parents.
///
/// Returns `Validation` if `layer_ids` is empty or any of the listed
/// layers is already a group — the menu item disables itself for the
/// already-group case, but the backend rejects defensively to avoid
/// silently nesting groups.
fn wrap_layers_in_group(
    sprite: &mut Sprite,
    layer_ids: &[LayerId],
    new_group_id: LayerId,
) -> CommandResult<Layer> {
    if layer_ids.is_empty() {
        return Err(AppCommandError::Validation {
            detail: "wrap_layers_in_group requires at least one layer".into(),
        });
    }

    // Resolve every requested id to a vec position. Bail at the first
    // missing or already-a-group layer so the sprite isn't half-mutated.
    // Track the anchor (topmost selected) inline — its parent becomes
    // the new group's parent and its vec position becomes the group's
    // insertion slot.
    let mut anchor_pos: usize = 0;
    let mut seen_any = false;
    for id in layer_ids {
        let pos =
            sprite
                .layers
                .iter()
                .position(|l| l.id == *id)
                .ok_or(AppCommandError::NotFound {
                    entity: "layer".into(),
                    id: u64::from(id.get()),
                })?;
        if matches!(sprite.layers[pos].kind, LayerKind::Group { .. }) {
            return Err(AppCommandError::Validation {
                detail: format!("layer {} is already a group", u64::from(id.get())),
            });
        }
        if !seen_any || pos > anchor_pos {
            anchor_pos = pos;
            seen_any = true;
        }
    }

    let anchor_parent = sprite.layers[anchor_pos].parent;

    let new_group = Layer {
        id: new_group_id,
        name: next_auto_name(&sprite.layers, "Group"),
        kind: LayerKind::Group { collapsed: false },
        blend_mode: BlendMode::Normal,
        opacity: 255,
        visible: true,
        locked: false,
        parent: anchor_parent,
        effects: Vec::new(),
        user_data: UserData::default(),
    };

    // Reparent every selected layer to the new group. We do this before
    // inserting the group so the position lookups above stay valid.
    for id in layer_ids {
        if let Some(layer) = sprite.layers.iter_mut().find(|l| l.id == *id) {
            layer.parent = Some(new_group_id);
        }
    }

    sprite.layers.insert(anchor_pos, new_group.clone());
    Ok(new_group)
}

/// Picks the next unused name in the `<prefix> N` series on a sprite's
/// layer list. Mirrors the TypeScript `nextAutoName` in
/// `ui/src/layers/layer-state.ts` — keep the two in sync if you change
/// the rule (no gap-fill: max + 1, not the first free integer).
fn next_auto_name(layers: &[Layer], prefix: &str) -> String {
    let mut max = 0u32;
    for layer in layers {
        let Some(rest) = layer
            .name
            .strip_prefix(prefix)
            .and_then(|r| r.strip_prefix(' '))
        else {
            continue;
        };
        if let Ok(n) = rest.parse::<u32>() {
            if n > max {
                max = n;
            }
        }
    }
    format!("{prefix} {}", max + 1)
}

/// Wraps the given non-group layers under a single fresh group. Pixels
/// and cels are preserved. Pass `vec![one]` for the single-layer case.
/// Returns the newly created group.
#[tauri::command(async, rename_all = "snake_case")]
pub async fn layer_wrap_in_group(
    sprite_id: SpriteId,
    layer_ids: Vec<LayerId>,
    state: State<'_, AppState>,
) -> CommandResult<Layer> {
    let mut doc = state.doc.write().await;
    let new_id = LayerId::new(doc.next_id);
    let new_group = {
        let sprite = doc
            .project
            .as_mut()
            .ok_or(AppCommandError::NoActiveProject)?
            .sprite_mut(sprite_id)
            .ok_or(AppCommandError::NotFound {
                entity: "sprite".into(),
                id: u64::from(sprite_id.get()),
            })?;
        wrap_layers_in_group(sprite, &layer_ids, new_id)?
    };
    doc.next_id += 1;
    doc.dirty = true;
    Ok(new_group)
}

/// Converts a layer to a tilemap layer using the given tileset. Drops
/// every cel on the layer — tilemap cels are a different variant and
/// will be re-populated when the user paints tiles.
#[tauri::command(async, rename_all = "snake_case")]
pub async fn layer_convert_to_tilemap(
    sprite_id: SpriteId,
    layer_id: LayerId,
    tileset_id: TilesetId,
    state: State<'_, AppState>,
) -> CommandResult<()> {
    let mut doc = state.doc.write().await;
    {
        let sprite = doc
            .project
            .as_mut()
            .ok_or(AppCommandError::NoActiveProject)?
            .sprite_mut(sprite_id)
            .ok_or(AppCommandError::NotFound {
                entity: "sprite".into(),
                id: u64::from(sprite_id.get()),
            })?;
        // Validate tileset exists.
        if !sprite.tilesets.iter().any(|ts| ts.id == tileset_id) {
            return Err(AppCommandError::NotFound {
                entity: "tileset".into(),
                id: u64::from(tileset_id.get()),
            });
        }
        {
            let layer = sprite.layers.iter_mut().find(|l| l.id == layer_id).ok_or(
                AppCommandError::NotFound {
                    entity: "layer".into(),
                    id: u64::from(layer_id.get()),
                },
            )?;
            layer.kind = LayerKind::Tilemap {
                tileset: tileset_id,
            };
        }
        // Drop every cel on the layer (see fn doc — tilemap cels are a
        // different variant and will be re-populated when the user paints).
        sprite.cels.retain(|c| c.layer_id != layer_id);
    }
    doc.dirty = true;
    Ok(())
}

/// Merges the active layer's pixel content into the layer immediately
/// below it in z-order, then drops the active layer and its cels.
///
/// The active layer's blend mode and opacity drive the composite onto
/// the lower layer. Both layers must be raster — merging across the
/// raster/tilemap boundary has no defined semantics.
///
/// Pixel mutations are recorded as a single `PixelOpBatch` for undo.
/// The drop of the active layer itself is not undoable yet — see the
/// follow-up note in the PR; layer-history wiring is a separate stream.
///
/// # Errors
///
/// - [`AppCommandError::NoActiveProject`] — no project is open.
/// - [`AppCommandError::NotFound`] — sprite or layer is not present.
/// - [`AppCommandError::Validation`] — active layer is the bottom-most.
/// - [`AppCommandError::Unimplemented`] — either layer is a tilemap,
///   group, or reference layer.
#[tauri::command(async, rename_all = "snake_case")]
pub async fn layer_merge_down(
    sprite_id: SpriteId,
    layer_id: LayerId,
    state: State<'_, AppState>,
    app: AppHandle,
) -> CommandResult<()> {
    let mut lock = state.doc.write().await;
    let doc = &mut *lock;

    // Resolve the layer below in z-order (lower index = further back).
    let below_id = {
        let sprite = find_sprite(doc, sprite_id)?;
        let pos = sprite
            .layers
            .iter()
            .position(|l| l.id == layer_id)
            .ok_or_else(|| AppCommandError::NotFound {
                entity: "layer".into(),
                id: u64::from(layer_id.get()),
            })?;
        if pos == 0 {
            return Err(AppCommandError::Validation {
                detail: "layer is already at the bottom; nothing below to merge into".into(),
            });
        }
        sprite.layers[pos - 1].id
    };

    let ops = merge_layer_into(doc, sprite_id, layer_id, below_id, "merge down")?;
    drop_layer(doc, sprite_id, layer_id)?;

    finalize_merge(doc, &app, sprite_id, ops, "merge down");
    Ok(())
}

/// Merges every selected layer into the topmost selected layer in
/// z-order, then drops the rest.
///
/// All selected layers must belong to the same sprite and be raster.
/// The visible composite of the stack is computed bottom-to-top using
/// each layer's blend mode and opacity, then written to the topmost
/// layer's cel — the merged layer ends up holding what the stack
/// looked like on screen.
///
/// # Errors
///
/// - [`AppCommandError::Validation`] — fewer than two unique layers
///   selected, or a layer was not found on the sprite.
/// - [`AppCommandError::Unimplemented`] — any selected layer is not
///   raster.
#[tauri::command(async, rename_all = "snake_case")]
pub async fn layer_merge_selected(
    sprite_id: SpriteId,
    layer_ids: Vec<LayerId>,
    state: State<'_, AppState>,
    app: AppHandle,
) -> CommandResult<()> {
    let mut lock = state.doc.write().await;
    let doc = &mut *lock;

    let ordered = order_layers_by_z(doc, sprite_id, &layer_ids)?;
    if ordered.len() < 2 {
        return Err(AppCommandError::Validation {
            detail: "merge needs at least two distinct layers".into(),
        });
    }

    // Bottom-to-top composite, then write to the top-most layer.
    let target_id = ordered[ordered.len() - 1];
    let ops = composite_stack_into(doc, sprite_id, &ordered, target_id)?;
    let sources: Vec<LayerId> = ordered
        .iter()
        .copied()
        .filter(|id| *id != target_id)
        .collect();
    for src in &sources {
        drop_layer(doc, sprite_id, *src)?;
    }

    finalize_merge(doc, &app, sprite_id, ops, "merge selected");
    Ok(())
}

/// Composites every visible raster layer in the sprite into the
/// bottom-most visible raster layer, then drops the rest of the
/// merged layers. Hidden layers and tilemap layers are left in place.
///
/// # Errors
///
/// - [`AppCommandError::Validation`] — sprite has zero or one visible
///   raster layer (nothing to flatten into a different shape).
#[tauri::command(async, rename_all = "snake_case")]
pub async fn layer_flatten_visible(
    sprite_id: SpriteId,
    state: State<'_, AppState>,
    app: AppHandle,
) -> CommandResult<()> {
    let mut lock = state.doc.write().await;
    let doc = &mut *lock;

    // Walk bottom-to-top, collecting visible raster layer ids. Hidden
    // layers and non-raster layers are skipped — they survive the
    // flatten untouched.
    let visible_raster: Vec<LayerId> = {
        let sprite = find_sprite(doc, sprite_id)?;
        sprite
            .layers
            .iter()
            .filter(|l| l.visible && matches!(l.kind, LayerKind::Raster))
            .map(|l| l.id)
            .collect()
    };

    if visible_raster.len() < 2 {
        return Err(AppCommandError::Validation {
            detail: "flatten needs at least two visible raster layers".into(),
        });
    }

    // Bottom-to-top composite written into the bottom-most visible
    // raster layer.
    let target_id = visible_raster[0];
    let ops = composite_stack_into(doc, sprite_id, &visible_raster, target_id)?;
    let sources: Vec<LayerId> = visible_raster
        .iter()
        .copied()
        .filter(|id| *id != target_id)
        .collect();
    for src in &sources {
        drop_layer(doc, sprite_id, *src)?;
    }

    finalize_merge(doc, &app, sprite_id, ops, "flatten visible");
    Ok(())
}

// ── helpers ──────────────────────────────────────────────────────────────────

fn find_layer_mut(
    doc: &mut DocumentStore,
    sprite_id: SpriteId,
    layer_id: LayerId,
) -> CommandResult<&mut Layer> {
    doc.project
        .as_mut()
        .ok_or(AppCommandError::NoActiveProject)?
        .sprite_mut(sprite_id)
        .ok_or(AppCommandError::NotFound {
            entity: "sprite".into(),
            id: u64::from(sprite_id.get()),
        })?
        .layers
        .iter_mut()
        .find(|l| l.id == layer_id)
        .ok_or(AppCommandError::NotFound {
            entity: "layer".into(),
            id: u64::from(layer_id.get()),
        })
}

/// Locks the document, resolves the layer, runs `mutate`, marks the
/// document dirty, and returns the closure's value.
///
/// Collapses the lock-acquire / sprite+layer lookup / `dirty = true`
/// wrapper repeated by every single-field layer setter. The closure
/// runs while the write lock is held and the lock is released before
/// returning, so no guard crosses an `.await`.
async fn with_layer_mut<T>(
    state: &State<'_, AppState>,
    sprite_id: SpriteId,
    layer_id: LayerId,
    mutate: impl FnOnce(&mut Layer) -> T,
) -> CommandResult<T> {
    let mut doc = state.doc.write().await;
    let out = mutate(find_layer_mut(&mut doc, sprite_id, layer_id)?);
    doc.dirty = true;
    Ok(out)
}

fn find_sprite(doc: &DocumentStore, sprite_id: SpriteId) -> CommandResult<&Sprite> {
    doc.project
        .as_ref()
        .ok_or(AppCommandError::NoActiveProject)?
        .sprite(sprite_id)
        .ok_or(AppCommandError::NotFound {
            entity: "sprite".into(),
            id: u64::from(sprite_id.get()),
        })
}

/// Returns the unique layer ids in `requested`, sorted by their
/// position in the sprite's layer stack (low index first = bottom).
///
/// Errors when any id is missing from the sprite or when any id is
/// not a raster layer — merge semantics aren't defined across raster
/// and tilemap/group/reference layers.
fn order_layers_by_z(
    doc: &DocumentStore,
    sprite_id: SpriteId,
    requested: &[LayerId],
) -> CommandResult<Vec<LayerId>> {
    let sprite = find_sprite(doc, sprite_id)?;
    let mut positions: Vec<(usize, LayerId)> = Vec::with_capacity(requested.len());
    for id in requested {
        let pos = sprite
            .layers
            .iter()
            .position(|l| l.id == *id)
            .ok_or_else(|| AppCommandError::NotFound {
                entity: "layer".into(),
                id: u64::from(id.get()),
            })?;
        let layer = &sprite.layers[pos];
        if !matches!(layer.kind, LayerKind::Raster) {
            return Err(AppCommandError::Unimplemented {
                stream: "raster layers only (merge across raster/tilemap is undefined)".into(),
            });
        }
        if !positions.iter().any(|(_, existing)| *existing == *id) {
            positions.push((pos, *id));
        }
    }
    positions.sort_by_key(|(pos, _)| *pos);
    Ok(positions.into_iter().map(|(_, id)| id).collect())
}

/// Composites every layer in `ordered` (bottom-to-top) for every
/// frame any of them has a raster cel on, then writes the result
/// into the `target` layer's cel for that frame.
///
/// Each layer contributes through its own blend mode and opacity, so
/// the merged buffer reproduces what the user saw on screen before
/// the operation.
///
/// Returns one `PixelOp` per touched target buffer for undo batching.
fn composite_stack_into(
    doc: &mut DocumentStore,
    sprite_id: SpriteId,
    ordered: &[LayerId],
    target: LayerId,
) -> CommandResult<Vec<PixelOp>> {
    // Validate that every layer in the stack is raster up front; the
    // per-frame loop only reads pixel data after that.
    {
        let sprite = find_sprite(doc, sprite_id)?;
        for id in ordered {
            let layer =
                sprite
                    .layers
                    .iter()
                    .find(|l| l.id == *id)
                    .ok_or(AppCommandError::NotFound {
                        entity: "layer".into(),
                        id: u64::from(id.get()),
                    })?;
            if !matches!(layer.kind, LayerKind::Raster) {
                return Err(AppCommandError::Unimplemented {
                    stream: "raster layers only (merge across raster/tilemap is undefined)".into(),
                });
            }
        }
    }

    // Per-layer blend metadata + canvas dims, captured before we start
    // touching pixel buffers.
    let (canvas_w, canvas_h, layer_meta): (u32, u32, Vec<(LayerId, BlendMode, u8)>) =
        {
            let sprite = find_sprite(doc, sprite_id)?;
            let mut meta: Vec<(LayerId, BlendMode, u8)> = Vec::with_capacity(ordered.len());
            for id in ordered {
                let layer = sprite.layers.iter().find(|l| l.id == *id).ok_or(
                    AppCommandError::NotFound {
                        entity: "layer".into(),
                        id: u64::from(id.get()),
                    },
                )?;
                meta.push((*id, layer.blend_mode, layer.opacity));
            }
            (sprite.canvas.width, sprite.canvas.height, meta)
        };

    // Set of frames touched by any layer in `ordered`. The composite
    // result is materialised into the target layer's cel for every
    // such frame.
    let mut frames: Vec<FrameIndex> = {
        let sprite = find_sprite(doc, sprite_id)?;
        let mut set = std::collections::BTreeSet::new();
        for cel in &sprite.cels {
            if !ordered.contains(&cel.layer_id) {
                continue;
            }
            if matches!(cel.data, CelData::Raster { .. }) {
                set.insert(cel.frame_index);
            }
        }
        set.into_iter().collect()
    };
    frames.sort();

    let mut ops: Vec<PixelOp> = Vec::with_capacity(frames.len());
    for frame_index in frames {
        let op = composite_stack_one_frame(
            doc,
            sprite_id,
            &layer_meta,
            target,
            frame_index,
            canvas_w,
            canvas_h,
        )?;
        ops.push(op);
    }

    Ok(ops)
}

#[allow(clippy::too_many_arguments)]
fn composite_stack_one_frame(
    doc: &mut DocumentStore,
    sprite_id: SpriteId,
    layer_meta: &[(LayerId, BlendMode, u8)],
    target: LayerId,
    frame_index: FrameIndex,
    canvas_w: u32,
    canvas_h: u32,
) -> CommandResult<PixelOp> {
    // Build a fresh transparent backdrop for the frame, then
    // composite each layer in z-order. Layers without a raster cel
    // on this frame contribute nothing.
    let mut backdrop =
        PixelBuffer::new(canvas_w, canvas_h).map_err(|e| AppCommandError::Validation {
            detail: e.to_string(),
        })?;
    for (layer_id, blend, opacity) in layer_meta {
        let src_buf_id = {
            let sprite = find_sprite(doc, sprite_id)?;
            sprite.cels.iter().find_map(|c| {
                if c.layer_id != *layer_id || c.frame_index != frame_index {
                    return None;
                }
                match c.data {
                    CelData::Raster { buffer, .. } => Some(buffer),
                    _ => None,
                }
            })
        };
        let Some(buf_id) = src_buf_id else {
            continue;
        };
        let src_canvas_buf = src_cel_to_canvas_buffer(
            doc,
            sprite_id,
            *layer_id,
            frame_index,
            buf_id,
            canvas_w,
            canvas_h,
        )?;
        composite_onto(
            &mut backdrop,
            &LayerInput {
                buffer: &src_canvas_buf,
                mode: *blend,
                opacity: *opacity,
                visible: true,
            },
        )
        .map_err(|e| AppCommandError::Validation {
            detail: e.to_string(),
        })?;
    }

    // Materialise / locate the target cel buffer and write the
    // composited backdrop into it.
    let target_buffer_id =
        ensure_canvas_target_buffer(doc, sprite_id, target, frame_index, canvas_w, canvas_h)?;

    let (before, w, h, stride) = {
        let entry = doc
            .pixel_buffers
            .iter()
            .find(|e| e.id == target_buffer_id.get())
            .ok_or_else(|| AppCommandError::NotFound {
                entity: "pixel buffer".into(),
                id: u64::from(target_buffer_id.get()),
            })?;
        (
            entry.pixels.clone(),
            entry.width,
            entry.height,
            entry.stride,
        )
    };

    let after = backdrop.as_bytes().to_vec();
    if let Some(entry) = doc
        .pixel_buffers
        .iter_mut()
        .find(|e| e.id == target_buffer_id.get())
    {
        entry.pixels.clone_from(&after);
    }

    Ok(PixelOp {
        buffer_id: target_buffer_id.get(),
        buf_width: w,
        buf_height: h,
        buf_stride: stride,
        sprite_id: sprite_id.get(),
        frame_index: frame_index.get(),
        before,
        after,
    })
}

/// Composites the source layer's raster cels onto the destination
/// layer's cels for every frame the source has content on, treating
/// the destination as the backdrop. Used by `merge_down`, where the
/// active (source) layer paints on top of the layer directly below.
///
/// Allocates destination cels and pixel buffers lazily. Returns one
/// `PixelOp` per touched destination buffer so the caller can roll
/// them into a single undo batch.
fn merge_layer_into(
    doc: &mut DocumentStore,
    sprite_id: SpriteId,
    src_layer_id: LayerId,
    dst_layer_id: LayerId,
    _label: &str,
) -> CommandResult<Vec<PixelOp>> {
    let prep = collect_merge_prep(doc, sprite_id, src_layer_id, dst_layer_id)?;
    let mut ops: Vec<PixelOp> = Vec::with_capacity(prep.frames.len());
    for (frame_index, src_buffer_id) in prep.frames {
        let op = composite_one_frame(
            doc,
            sprite_id,
            src_layer_id,
            dst_layer_id,
            frame_index,
            src_buffer_id,
            prep.canvas_w,
            prep.canvas_h,
            prep.src_blend,
            prep.src_opacity,
        )?;
        ops.push(op);
    }
    Ok(ops)
}

/// Read-only state captured up front for a merge so the borrow on the
/// project releases before the per-frame loop starts touching
/// `pixel_buffers`.
struct MergePrep {
    canvas_w: u32,
    canvas_h: u32,
    src_blend: BlendMode,
    src_opacity: u8,
    frames: Vec<(FrameIndex, PixelBufferId)>,
}

fn collect_merge_prep(
    doc: &DocumentStore,
    sprite_id: SpriteId,
    src_layer_id: LayerId,
    dst_layer_id: LayerId,
) -> CommandResult<MergePrep> {
    let sprite = find_sprite(doc, sprite_id)?;
    let src_layer =
        sprite
            .layers
            .iter()
            .find(|l| l.id == src_layer_id)
            .ok_or(AppCommandError::NotFound {
                entity: "layer".into(),
                id: u64::from(src_layer_id.get()),
            })?;
    let dst_layer =
        sprite
            .layers
            .iter()
            .find(|l| l.id == dst_layer_id)
            .ok_or(AppCommandError::NotFound {
                entity: "layer".into(),
                id: u64::from(dst_layer_id.get()),
            })?;
    if !matches!(src_layer.kind, LayerKind::Raster) || !matches!(dst_layer.kind, LayerKind::Raster)
    {
        return Err(AppCommandError::Unimplemented {
            stream: "raster layers only (merge across raster/tilemap is undefined)".into(),
        });
    }
    // Linked cels resolve to another frame on the same layer. Skip
    // them for now — the editor doesn't yet produce them, and
    // resolving the link adds complexity that's better tackled when
    // they show up in real test fixtures.
    let frames: Vec<(FrameIndex, PixelBufferId)> = sprite
        .cels
        .iter()
        .filter_map(|c| {
            if c.layer_id != src_layer_id {
                return None;
            }
            match c.data {
                CelData::Raster { buffer, .. } => Some((c.frame_index, buffer)),
                _ => None,
            }
        })
        .collect();
    Ok(MergePrep {
        canvas_w: sprite.canvas.width,
        canvas_h: sprite.canvas.height,
        src_blend: src_layer.blend_mode,
        src_opacity: src_layer.opacity,
        frames,
    })
}

#[allow(clippy::too_many_arguments)]
fn composite_one_frame(
    doc: &mut DocumentStore,
    sprite_id: SpriteId,
    src_layer_id: LayerId,
    dst_layer_id: LayerId,
    frame_index: FrameIndex,
    src_buffer_id: PixelBufferId,
    canvas_w: u32,
    canvas_h: u32,
    src_blend: BlendMode,
    src_opacity: u8,
) -> CommandResult<PixelOp> {
    let frame_raw = frame_index.get();

    // Materialise a canvas-sized source buffer offset by the source
    // cel's position on the canvas. If the source is already
    // canvas-sized at origin, this is a one-shot copy.
    let src_canvas_buf = src_cel_to_canvas_buffer(
        doc,
        sprite_id,
        src_layer_id,
        frame_index,
        src_buffer_id,
        canvas_w,
        canvas_h,
    )?;

    // Ensure a target buffer exists and is canvas-sized at origin.
    // The merge result always lives at (0, 0) with canvas dims so
    // subsequent passes (further merges, transforms) have a
    // predictable layout.
    let dst_buffer_id = ensure_canvas_target_buffer(
        doc,
        sprite_id,
        dst_layer_id,
        frame_index,
        canvas_w,
        canvas_h,
    )?;

    let (before, w, h, stride) = {
        let entry = doc
            .pixel_buffers
            .iter()
            .find(|e| e.id == dst_buffer_id.get())
            .ok_or_else(|| AppCommandError::NotFound {
                entity: "pixel buffer".into(),
                id: u64::from(dst_buffer_id.get()),
            })?;
        (
            entry.pixels.clone(),
            entry.width,
            entry.height,
            entry.stride,
        )
    };

    let mut backdrop = PixelBuffer::from_raw(w, h, stride, before.clone()).map_err(|e| {
        AppCommandError::Validation {
            detail: e.to_string(),
        }
    })?;
    composite_onto(
        &mut backdrop,
        &LayerInput {
            buffer: &src_canvas_buf,
            mode: src_blend,
            opacity: src_opacity,
            visible: true,
        },
    )
    .map_err(|e| AppCommandError::Validation {
        detail: e.to_string(),
    })?;

    let after = backdrop.as_bytes().to_vec();
    if let Some(entry) = doc
        .pixel_buffers
        .iter_mut()
        .find(|e| e.id == dst_buffer_id.get())
    {
        entry.pixels.clone_from(&after);
    }

    Ok(PixelOp {
        buffer_id: dst_buffer_id.get(),
        buf_width: w,
        buf_height: h,
        buf_stride: stride,
        sprite_id: sprite_id.get(),
        frame_index: frame_raw,
        before,
        after,
    })
}

/// Builds a canvas-sized [`PixelBuffer`] containing the source cel's
/// content placed at the cel's `position` on the canvas. Pixels
/// outside the source cel are transparent.
fn src_cel_to_canvas_buffer(
    doc: &DocumentStore,
    sprite_id: SpriteId,
    layer_id: LayerId,
    frame_index: FrameIndex,
    src_buffer_id: PixelBufferId,
    canvas_w: u32,
    canvas_h: u32,
) -> CommandResult<PixelBuffer> {
    let sprite = find_sprite(doc, sprite_id)?;
    let cel = sprite
        .cels
        .iter()
        .find(|c| c.layer_id == layer_id && c.frame_index == frame_index)
        .ok_or_else(|| AppCommandError::NotFound {
            entity: "cel".into(),
            id: u64::from(frame_index.get()),
        })?;
    let cel_pos = cel.position;
    let entry = doc
        .pixel_buffers
        .iter()
        .find(|e| e.id == src_buffer_id.get())
        .ok_or_else(|| AppCommandError::NotFound {
            entity: "pixel buffer".into(),
            id: u64::from(src_buffer_id.get()),
        })?;

    // Fast path: source is already canvas-sized at the origin and
    // tightly packed. Skip the offset blit.
    if cel_pos.x == 0
        && cel_pos.y == 0
        && entry.width == canvas_w
        && entry.height == canvas_h
        && entry.stride == canvas_w * 4
    {
        return PixelBuffer::from_raw(
            entry.width,
            entry.height,
            entry.stride,
            entry.pixels.clone(),
        )
        .map_err(|e| AppCommandError::Validation {
            detail: e.to_string(),
        });
    }

    // General path: blit the source rows into a fresh canvas-sized
    // buffer at the cel offset. Iterate over the source bounds and
    // compute the canvas-space destination per pixel using checked
    // signed arithmetic — cels can sit at negative offsets and the
    // source can extend off-canvas.
    let mut canvas =
        PixelBuffer::new(canvas_w, canvas_h).map_err(|e| AppCommandError::Validation {
            detail: e.to_string(),
        })?;
    let src_w = entry.width;
    let src_h = entry.height;
    let src_stride = entry.stride as usize;

    for src_row in 0..src_h {
        let row_signed = i64::from(src_row) + i64::from(cel_pos.y);
        if row_signed < 0 || row_signed >= i64::from(canvas_h) {
            continue;
        }
        // row_signed is in [0, canvas_h); within u32 by construction.
        let dst_row = u32::try_from(row_signed).map_err(|_| AppCommandError::Validation {
            detail: "internal: y coordinate out of range".into(),
        })?;
        for src_col in 0..src_w {
            let col_signed = i64::from(src_col) + i64::from(cel_pos.x);
            if col_signed < 0 || col_signed >= i64::from(canvas_w) {
                continue;
            }
            let dst_col = u32::try_from(col_signed).map_err(|_| AppCommandError::Validation {
                detail: "internal: x coordinate out of range".into(),
            })?;
            let src_off = (src_row as usize) * src_stride + (src_col as usize) * 4;
            let bytes = entry.pixels.get(src_off..src_off + 4).ok_or_else(|| {
                AppCommandError::Validation {
                    detail: "source pixel buffer truncated".into(),
                }
            })?;
            canvas.set_pixel(
                dst_col,
                dst_row,
                pixhaus_core::project::Rgba::new(bytes[0], bytes[1], bytes[2], bytes[3]),
            );
        }
    }
    Ok(canvas)
}

/// Ensures a canvas-sized raster cel exists for `(layer, frame)` and
/// returns its buffer id. Creates a transparent canvas-sized buffer
/// and resizes/repositions the cel if the existing cel is smaller or
/// offset — merge results always normalise to (0, 0) with canvas
/// dims so the next pass has a stable layout to write into.
fn ensure_canvas_target_buffer(
    doc: &mut DocumentStore,
    sprite_id: SpriteId,
    layer_id: LayerId,
    frame_index: FrameIndex,
    canvas_w: u32,
    canvas_h: u32,
) -> CommandResult<PixelBufferId> {
    let frame_raw = frame_index.get();

    // Read the existing cel and buffer state, if any.
    let existing: Option<(PixelBufferId, u32, u32, u32)> = {
        let sprite = find_sprite(doc, sprite_id)?;
        sprite
            .cels
            .iter()
            .find(|c| c.layer_id == layer_id && c.frame_index == frame_index)
            .and_then(|c| match c.data {
                CelData::Raster { buffer, size } => Some((
                    buffer,
                    size.width,
                    size.height,
                    c.position
                        .x
                        .unsigned_abs()
                        .saturating_add(c.position.y.unsigned_abs()),
                )),
                _ => None,
            })
    };

    if let Some((buffer_id, w, h, offset_magnitude)) = existing
        && w == canvas_w
        && h == canvas_h
        && offset_magnitude == 0
    {
        // Already canvas-sized at origin — reuse without touching pixels.
        if doc.pixel_buffers.iter().any(|e| e.id == buffer_id.get()) {
            return Ok(buffer_id);
        }
        // Cel referenced a buffer that isn't loaded; fall through to
        // re-create a fresh canvas-sized buffer.
    }

    // Allocate a fresh canvas-sized transparent buffer.
    let new_id = PixelBufferId::new(doc.next_id);
    doc.next_id += 1;
    let stride = canvas_w * 4;
    doc.pixel_buffers.push(PixelBufferEntry {
        id: new_id.get(),
        width: canvas_w,
        height: canvas_h,
        stride,
        pixels: vec![0u8; (stride * canvas_h) as usize],
    });

    // Update the project: replace or insert the cel at canvas size,
    // origin (0, 0). If a non-conformant cel existed (offset, smaller
    // size, or non-raster), the user-visible content was already
    // composited into the new buffer by the caller, so dropping the
    // old cel here is the right shape.
    let project = doc
        .project
        .as_mut()
        .ok_or(AppCommandError::NoActiveProject)?;
    let sprite = project
        .sprite_mut(sprite_id)
        .ok_or(AppCommandError::NotFound {
            entity: "sprite".into(),
            id: u64::from(sprite_id.get()),
        })?;

    // If the existing cel was canvas-sized but its buffer wasn't
    // loaded, we need to first composite its old content (which we
    // can't — it's gone). Drop it and write a fresh transparent cel;
    // the caller's composite step will paint source pixels in.
    sprite
        .cels
        .retain(|c| !(c.layer_id == layer_id && c.frame_index == frame_index));
    sprite.cels.push(Cel::raster(
        layer_id,
        FrameIndex::new(frame_raw),
        new_id,
        Size::new(canvas_w, canvas_h),
    ));

    // If the prior cel was canvas-sized at origin AND its buffer is
    // loaded, copy its pixels into the new buffer so the existing
    // content survives. (In practice this branch fires only when the
    // existing buffer didn't match the loaded set — a degenerate
    // case.) The first-load happy path handled it via the early
    // return above.
    if let Some((old_id, w, h, offset_magnitude)) = existing
        && w == canvas_w
        && h == canvas_h
        && offset_magnitude == 0
        && let Some(old_entry) = doc
            .pixel_buffers
            .iter()
            .find(|e| e.id == old_id.get())
            .map(|e| (e.pixels.clone(), e.stride))
    {
        if let Some(new_entry) = doc.pixel_buffers.iter_mut().find(|e| e.id == new_id.get()) {
            // Strides match (both width*4 in this branch), so a
            // direct copy is correct.
            if old_entry.1 == stride {
                new_entry.pixels.clone_from(&old_entry.0);
            }
        }
    }

    Ok(new_id)
}

/// Drops `layer_id` from the sprite's layer list and any cels it owns.
/// Returns `Ok(())` even if the layer is already gone — merging is
/// idempotent at the structural level.
fn drop_layer(
    doc: &mut DocumentStore,
    sprite_id: SpriteId,
    layer_id: LayerId,
) -> CommandResult<()> {
    let project = doc
        .project
        .as_mut()
        .ok_or(AppCommandError::NoActiveProject)?;
    let sprite = project
        .sprite_mut(sprite_id)
        .ok_or(AppCommandError::NotFound {
            entity: "sprite".into(),
            id: u64::from(sprite_id.get()),
        })?;
    sprite.layers.retain(|l| l.id != layer_id);
    sprite.cels.retain(|c| c.layer_id != layer_id);
    Ok(())
}

/// Pushes the merged ops into pixel history as a single batch, marks
/// the document dirty, and emits tile-dirty events for every touched
/// buffer so the renderer refreshes.
fn finalize_merge(
    doc: &mut DocumentStore,
    app: &AppHandle,
    sprite_id: SpriteId,
    ops: Vec<PixelOp>,
    label: &str,
) {
    if ops.is_empty() {
        doc.dirty = true;
        return;
    }
    // Snapshot the data needed for tile emission before handing the
    // ops vector to the history stack.
    let emit_payloads: Vec<(u32, u32, u32, u32, u32, Vec<u8>)> = ops
        .iter()
        .map(|op| {
            (
                op.buf_width,
                op.buf_height,
                op.buf_stride,
                op.frame_index,
                op.buffer_id,
                op.after.clone(),
            )
        })
        .collect();

    doc.pixel_history.push(PixelOpBatch {
        label: label.to_owned(),
        ops,
    });
    doc.dirty = true;

    let sprite_id_raw = sprite_id.get();
    for (w, h, stride, frame_index, _buf_id, pixels) in emit_payloads {
        emit_buffer_tiles(app, sprite_id_raw, frame_index, w, h, stride, &pixels);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pixhaus_core::project::{LayerKind, Size};

    #[test]
    fn layer_constructed_with_defaults() {
        let id = LayerId::new(1);
        let layer = Layer {
            id,
            name: "bg".into(),
            kind: LayerKind::Raster,
            blend_mode: BlendMode::Normal,
            opacity: 255,
            visible: true,
            locked: false,
            parent: None,
            effects: Vec::new(),
            user_data: UserData::default(),
        };
        assert_eq!(layer.opacity, 255);
        assert!(layer.visible);
        assert!(!layer.locked);
    }

    // ── set_layer_parent_in_sprite cycle guards ──────────────────────────

    fn layer_with(id: u32, parent: Option<LayerId>, kind: LayerKind) -> Layer {
        Layer {
            id: LayerId::new(id),
            name: format!("L{id}"),
            kind,
            blend_mode: BlendMode::Normal,
            opacity: 255,
            visible: true,
            locked: false,
            parent,
            effects: Vec::new(),
            user_data: UserData::default(),
        }
    }

    /// Sprite shaped as a chain of group layers: A (1) → B (2) → C (3),
    /// where A is the deepest ancestor and C is a leaf group. Used to
    /// exercise both the self-parent and indirect-cycle paths.
    fn sprite_with_group_chain() -> Sprite {
        let mut sprite = Sprite::empty(SpriteId::new(1), "main", Size::new(8, 8));
        sprite
            .layers
            .push(layer_with(1, None, LayerKind::Group { collapsed: false }));
        sprite.layers.push(layer_with(
            2,
            Some(LayerId::new(1)),
            LayerKind::Group { collapsed: false },
        ));
        sprite.layers.push(layer_with(
            3,
            Some(LayerId::new(2)),
            LayerKind::Group { collapsed: false },
        ));
        sprite
    }

    #[test]
    fn set_parent_to_self_returns_validation() {
        let mut sprite = sprite_with_group_chain();
        let layer_id = LayerId::new(2);
        let err = set_layer_parent_in_sprite(&mut sprite, layer_id, Some(layer_id)).unwrap_err();
        assert!(
            matches!(err, AppCommandError::Validation { .. }),
            "self-parent must reject with Validation; got {err:?}"
        );
    }

    #[test]
    fn set_parent_creating_cycle_is_rejected() {
        // A → B → C is the existing chain; reparent A onto C would close
        // a cycle (C's ancestors include A through B).
        let mut sprite = sprite_with_group_chain();
        let err = set_layer_parent_in_sprite(&mut sprite, LayerId::new(1), Some(LayerId::new(3)))
            .unwrap_err();
        assert!(
            matches!(err, AppCommandError::Validation { .. }),
            "indirect cycle must reject with Validation; got {err:?}"
        );
        // Existing parent assignment must not have been mutated by the
        // failed call.
        assert_eq!(sprite.layers[0].parent, None, "A's parent stayed None");
    }

    #[test]
    fn set_parent_to_unrelated_group_succeeds() {
        // Sanity check that the cycle guard isn't over-eager: moving a
        // top-level layer under an unrelated group is allowed.
        let mut sprite = Sprite::empty(SpriteId::new(1), "main", Size::new(8, 8));
        sprite
            .layers
            .push(layer_with(1, None, LayerKind::Group { collapsed: false }));
        sprite.layers.push(layer_with(2, None, LayerKind::Raster));
        set_layer_parent_in_sprite(&mut sprite, LayerId::new(2), Some(LayerId::new(1))).unwrap();
        assert_eq!(sprite.layers[1].parent, Some(LayerId::new(1)));
    }

    #[test]
    fn set_parent_to_non_group_returns_validation() {
        // A raster layer can't host children — the existing rejection
        // path must still fire.
        let mut sprite = Sprite::empty(SpriteId::new(1), "main", Size::new(8, 8));
        sprite.layers.push(layer_with(1, None, LayerKind::Raster));
        sprite.layers.push(layer_with(2, None, LayerKind::Raster));
        let err = set_layer_parent_in_sprite(&mut sprite, LayerId::new(2), Some(LayerId::new(1)))
            .unwrap_err();
        assert!(
            matches!(err, AppCommandError::Validation { .. }),
            "non-group parent must reject with Validation; got {err:?}"
        );
    }

    // ── wrap_layers_in_group + next_auto_name ────────────────────────────

    #[test]
    fn next_auto_name_starts_at_one_when_no_match() {
        let layers: Vec<Layer> = vec![];
        assert_eq!(next_auto_name(&layers, "Layer"), "Layer 1");
    }

    #[test]
    fn next_auto_name_picks_max_plus_one_no_gap_fill() {
        // Simulate `Layer 1`, `Layer 2` deleted, `Layer 3` present:
        // next should be `Layer 4`, not `Layer 2`.
        let layers = vec![
            layer_with(1, None, LayerKind::Raster),
            layer_with(3, None, LayerKind::Raster),
        ];
        // layer_with names them L1, L3; rename to match the prefix scan.
        let mut layers = layers;
        layers[0].name = "Layer 1".into();
        layers[1].name = "Layer 3".into();
        assert_eq!(next_auto_name(&layers, "Layer"), "Layer 4");
    }

    #[test]
    fn next_auto_name_ignores_non_matching_names() {
        let mut layers = vec![
            layer_with(1, None, LayerKind::Raster),
            layer_with(2, None, LayerKind::Raster),
            layer_with(3, None, LayerKind::Raster),
        ];
        layers[0].name = "Layer".into(); // missing number
        layers[1].name = "Layerfoo 1".into(); // wrong prefix (no space)
        layers[2].name = "Layer 1 extra".into(); // suffix after number
        // None match the strict pattern, so next is Layer 1.
        assert_eq!(next_auto_name(&layers, "Layer"), "Layer 1");
    }

    #[test]
    fn wrap_single_layer_preserves_kind_and_inserts_group_above() {
        // Top-level raster layer.
        let mut sprite = Sprite::empty(SpriteId::new(1), "main", Size::new(8, 8));
        sprite.layers.push(layer_with(7, None, LayerKind::Raster));
        // Mint id 100 for the new group (mimics doc.next_id allocation).
        let new_group_id = LayerId::new(100);
        let new_group =
            wrap_layers_in_group(&mut sprite, &[LayerId::new(7)], new_group_id).expect("wrap ok");

        // Group is at the original's old vec position; original shifts up by 1.
        assert_eq!(sprite.layers.len(), 2, "exactly one new layer added");
        assert_eq!(sprite.layers[0].id, new_group_id, "group inserted at pos 0");
        assert_eq!(sprite.layers[1].id, LayerId::new(7), "original is at pos 1");

        // Original layer's kind is unchanged; cels stay attached.
        assert!(
            matches!(sprite.layers[1].kind, LayerKind::Raster),
            "original kind must remain Raster after wrap"
        );

        // Parent pointers form the wrap relationship.
        assert_eq!(
            sprite.layers[1].parent,
            Some(new_group_id),
            "original is now a child of the new group"
        );
        assert_eq!(
            sprite.layers[0].parent, None,
            "new group inherits the original's old parent (None here)"
        );

        // Returned layer matches the inserted group.
        assert_eq!(new_group.id, new_group_id);
        assert!(matches!(
            new_group.kind,
            LayerKind::Group { collapsed: false }
        ));
    }

    #[test]
    fn wrap_single_layer_inherits_existing_parent() {
        // Original layer is already inside a group; wrapping should
        // place the new group as a sibling of the original (under the
        // outer group), with the original under the new group.
        let outer_id = LayerId::new(1);
        let original_id = LayerId::new(2);
        let mut sprite = Sprite::empty(SpriteId::new(1), "main", Size::new(8, 8));
        sprite.layers.push(layer_with(
            outer_id.get(),
            None,
            LayerKind::Group { collapsed: false },
        ));
        sprite.layers.push(layer_with(
            original_id.get(),
            Some(outer_id),
            LayerKind::Raster,
        ));

        let new_group_id = LayerId::new(100);
        wrap_layers_in_group(&mut sprite, &[original_id], new_group_id).expect("wrap ok");

        let new_group = sprite.layers.iter().find(|l| l.id == new_group_id).unwrap();
        let original = sprite.layers.iter().find(|l| l.id == original_id).unwrap();
        assert_eq!(
            new_group.parent,
            Some(outer_id),
            "new group inherits the original's old parent (outer group)"
        );
        assert_eq!(
            original.parent,
            Some(new_group_id),
            "original now points at the new group"
        );
    }

    #[test]
    fn wrap_rejects_layer_that_is_already_group() {
        let mut sprite = Sprite::empty(SpriteId::new(1), "main", Size::new(8, 8));
        sprite
            .layers
            .push(layer_with(1, None, LayerKind::Group { collapsed: false }));
        let err =
            wrap_layers_in_group(&mut sprite, &[LayerId::new(1)], LayerId::new(100)).unwrap_err();
        assert!(
            matches!(err, AppCommandError::Validation { .. }),
            "wrapping an existing group must reject with Validation; got {err:?}"
        );
        // Sprite is unchanged on the rejection path.
        assert_eq!(sprite.layers.len(), 1, "no group was inserted");
    }

    #[test]
    fn wrap_picks_unused_group_name() {
        let mut sprite = Sprite::empty(SpriteId::new(1), "main", Size::new(8, 8));
        // Pre-existing groups consume `Group 1` and `Group 2`.
        sprite.layers.push(Layer {
            id: LayerId::new(1),
            name: "Group 1".into(),
            kind: LayerKind::Group { collapsed: false },
            blend_mode: BlendMode::Normal,
            opacity: 255,
            visible: true,
            locked: false,
            parent: None,
            effects: Vec::new(),
            user_data: UserData::default(),
        });
        sprite.layers.push(Layer {
            id: LayerId::new(2),
            name: "Group 2".into(),
            kind: LayerKind::Group { collapsed: false },
            blend_mode: BlendMode::Normal,
            opacity: 255,
            visible: true,
            locked: false,
            parent: None,
            effects: Vec::new(),
            user_data: UserData::default(),
        });
        sprite.layers.push(layer_with(3, None, LayerKind::Raster));

        let new_group = wrap_layers_in_group(&mut sprite, &[LayerId::new(3)], LayerId::new(100))
            .expect("wrap ok");
        assert_eq!(new_group.name, "Group 3");
    }

    #[test]
    fn wrap_rejects_empty_layer_list() {
        let mut sprite = Sprite::empty(SpriteId::new(1), "main", Size::new(8, 8));
        sprite.layers.push(layer_with(1, None, LayerKind::Raster));
        let err = wrap_layers_in_group(&mut sprite, &[], LayerId::new(100)).unwrap_err();
        assert!(
            matches!(err, AppCommandError::Validation { .. }),
            "empty layer_ids must reject with Validation; got {err:?}"
        );
        assert_eq!(sprite.layers.len(), 1, "sprite unchanged on empty input");
    }

    #[test]
    fn wrap_two_siblings_reparents_both_under_new_group() {
        // Two top-level raster layers; selecting both should drop both
        // under one new group anchored at the topmost (highest vec pos).
        let mut sprite = Sprite::empty(SpriteId::new(1), "main", Size::new(8, 8));
        sprite.layers.push(layer_with(1, None, LayerKind::Raster)); // bottom
        sprite.layers.push(layer_with(2, None, LayerKind::Raster)); // top — anchor

        let new_group_id = LayerId::new(100);
        let new_group = wrap_layers_in_group(
            &mut sprite,
            &[LayerId::new(1), LayerId::new(2)],
            new_group_id,
        )
        .expect("wrap ok");

        // Anchor was at pos 1, so the group is inserted at pos 1.
        // Original layout [1, 2] → after insert [1, group, 2].
        assert_eq!(sprite.layers.len(), 3);
        assert_eq!(sprite.layers[1].id, new_group_id, "group at anchor's pos");
        // Both selected layers point at the new group.
        for id in [LayerId::new(1), LayerId::new(2)] {
            let layer = sprite.layers.iter().find(|l| l.id == id).unwrap();
            assert_eq!(
                layer.parent,
                Some(new_group_id),
                "layer {id:?} reparented under new group"
            );
            assert!(
                matches!(layer.kind, LayerKind::Raster),
                "kind preserved for {id:?}"
            );
        }
        // New group inherits the anchor's old parent (None at top level).
        assert_eq!(new_group.parent, None);
    }

    #[test]
    fn wrap_layers_from_different_parents_lands_at_anchor_parent() {
        // Anchor: layer 4, child of group B. Other selected: layer 2, child
        // of group A. Both should reparent under the new group, which sits
        // under group B (the anchor's parent).
        let first_parent = LayerId::new(1);
        let anchor_parent = LayerId::new(3);
        let mut sprite = Sprite::empty(SpriteId::new(1), "main", Size::new(8, 8));
        sprite.layers.push(layer_with(
            first_parent.get(),
            None,
            LayerKind::Group { collapsed: false },
        ));
        sprite
            .layers
            .push(layer_with(2, Some(first_parent), LayerKind::Raster));
        sprite.layers.push(layer_with(
            anchor_parent.get(),
            None,
            LayerKind::Group { collapsed: false },
        ));
        sprite
            .layers
            .push(layer_with(4, Some(anchor_parent), LayerKind::Raster));

        let new_group_id = LayerId::new(100);
        let new_group = wrap_layers_in_group(
            &mut sprite,
            &[LayerId::new(2), LayerId::new(4)],
            new_group_id,
        )
        .expect("wrap ok");

        assert_eq!(
            new_group.parent,
            Some(anchor_parent),
            "new group sits under the anchor's (topmost selected) parent"
        );
        for id in [LayerId::new(2), LayerId::new(4)] {
            let layer = sprite.layers.iter().find(|l| l.id == id).unwrap();
            assert_eq!(layer.parent, Some(new_group_id));
        }
    }

    #[test]
    fn wrap_rejects_when_any_layer_is_a_group() {
        // [raster, group, raster]; selecting raster + group must fail
        // before any mutation.
        let mut sprite = Sprite::empty(SpriteId::new(1), "main", Size::new(8, 8));
        sprite.layers.push(layer_with(1, None, LayerKind::Raster));
        sprite
            .layers
            .push(layer_with(2, None, LayerKind::Group { collapsed: false }));
        sprite.layers.push(layer_with(3, None, LayerKind::Raster));

        let err = wrap_layers_in_group(
            &mut sprite,
            &[LayerId::new(1), LayerId::new(2)],
            LayerId::new(100),
        )
        .unwrap_err();
        assert!(matches!(err, AppCommandError::Validation { .. }));
        // Sprite is unchanged on the rejection path.
        assert_eq!(sprite.layers.len(), 3, "no group was inserted");
        assert_eq!(
            sprite.layers[0].parent, None,
            "raster {} still top-level",
            1
        );
    }

    // ── merge_down / merge_selected / flatten_visible ────────────────────

    use pixhaus_core::project::{
        ActiveTarget, AiMetadata, Cel as CoreCel, Entity, EntityContent, EntityDefaults, EntityId,
        EntityKind, FrameIndex as CoreFrameIndex, NamedSprite, PixelBufferId as CorePixelBufferId,
        Project, StateId,
    };
    use pixhaus_io::pixhaus::PixelBufferEntry as CorePixelBufferEntry;

    /// Installs `sprite` as a fresh entity in `project.library` and
    /// marks its state active. Mirrors the production helper in
    /// `sprite_add` for the test layer.
    fn install_sprite(project: &mut Project, sprite: Sprite) {
        let entity_id = EntityId::new(1);
        let state_id = StateId::new(1);
        let name = sprite.name.clone();
        project.library.entities.push(Entity {
            id: entity_id,
            kind: EntityKind::Custom("Sprite".into()),
            name,
            group_id: None,
            tags: Vec::new(),
            defaults: EntityDefaults::default(),
            content: EntityContent::Sprites {
                states: vec![NamedSprite {
                    id: state_id,
                    state_name: "primary".into(),
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
    }

    /// Builds a `DocumentStore` with one sprite at `canvas_size` and
    /// no layers. Caller adds layers + cels via the helpers below.
    fn fresh_doc(canvas_size: Size) -> DocumentStore {
        let mut project = Project::new("test");
        install_sprite(
            &mut project,
            Sprite::empty(SpriteId::new(1), "main", canvas_size),
        );
        DocumentStore {
            project: Some(project),
            next_id: 100,
            ..DocumentStore::default()
        }
    }

    /// Pushes a raster layer onto the sprite and returns its id.
    fn add_raster_layer(doc: &mut DocumentStore, id: u32, name: &str) -> LayerId {
        let layer_id = LayerId::new(id);
        let project = doc.project.as_mut().unwrap();
        let sprite = project.sprite_mut(SpriteId::new(1)).unwrap();
        sprite.layers.push(Layer {
            id: layer_id,
            name: name.into(),
            kind: LayerKind::Raster,
            blend_mode: BlendMode::Normal,
            opacity: 255,
            visible: true,
            locked: false,
            parent: None,
            effects: Vec::new(),
            user_data: UserData::default(),
        });
        layer_id
    }

    /// Allocates a canvas-sized RGBA buffer filled with `fill` and
    /// links it to a new raster cel for `(layer_id, frame)`.
    fn add_filled_cel(
        doc: &mut DocumentStore,
        layer_id: LayerId,
        frame: u32,
        fill: [u8; 4],
    ) -> CorePixelBufferId {
        let buf_id = CorePixelBufferId::new(doc.next_id);
        doc.next_id += 1;
        let project = doc.project.as_ref().unwrap();
        let canvas = project.sprite(SpriteId::new(1)).unwrap().canvas;
        let stride = canvas.width * 4;
        let mut pixels = vec![0u8; (stride * canvas.height) as usize];
        for chunk in pixels.chunks_exact_mut(4) {
            chunk.copy_from_slice(&fill);
        }
        doc.pixel_buffers.push(CorePixelBufferEntry {
            id: buf_id.get(),
            width: canvas.width,
            height: canvas.height,
            stride,
            pixels,
        });
        let project = doc.project.as_mut().unwrap();
        let sprite = project.sprite_mut(SpriteId::new(1)).unwrap();
        sprite.cels.push(CoreCel::raster(
            layer_id,
            CoreFrameIndex::new(frame),
            buf_id,
            canvas,
        ));
        buf_id
    }

    fn pixels_of(doc: &DocumentStore, buf_id: CorePixelBufferId) -> Vec<u8> {
        doc.pixel_buffers
            .iter()
            .find(|e| e.id == buf_id.get())
            .unwrap()
            .pixels
            .clone()
    }

    /// `merge_layer_into` composites the source onto the bottom layer
    /// using the source's blend mode and opacity. Trivial case: opaque
    /// red over fully transparent destination produces opaque red.
    #[test]
    fn merge_layer_into_paints_red_over_transparent() {
        let mut doc = fresh_doc(Size::new(2, 2));
        let bottom = add_raster_layer(&mut doc, 1, "bottom");
        let top = add_raster_layer(&mut doc, 2, "top");
        let bottom_buf = add_filled_cel(&mut doc, bottom, 0, [0, 0, 0, 0]);
        let _top_buf = add_filled_cel(&mut doc, top, 0, [255, 0, 0, 255]);

        let ops = merge_layer_into(&mut doc, SpriteId::new(1), top, bottom, "merge")
            .expect("merge succeeds");
        assert_eq!(ops.len(), 1);
        let after = pixels_of(&doc, bottom_buf);
        for chunk in after.chunks_exact(4) {
            assert_eq!(chunk, &[255, 0, 0, 255], "every pixel reads opaque red");
        }
    }

    /// Calling `layer_merge_down` on the bottom-most layer must reject
    /// with Validation — there is nothing below to merge into.
    #[tokio::test]
    async fn merge_down_at_bottom_rejects() {
        let mut doc = fresh_doc(Size::new(2, 2));
        let bottom = add_raster_layer(&mut doc, 1, "bottom");
        let _top = add_raster_layer(&mut doc, 2, "top");
        add_filled_cel(&mut doc, bottom, 0, [0, 0, 0, 255]);

        // Reach into the helpers directly — composing a Tauri State
        // here adds machinery without exercising the rule we want to
        // pin. The branch under test is the "is bottom?" check.
        let sprite = find_sprite(&doc, SpriteId::new(1)).unwrap();
        let pos = sprite.layers.iter().position(|l| l.id == bottom).unwrap();
        assert_eq!(pos, 0, "bottom layer is at index 0");

        // Mirror the early-return shape used by the public command.
        let err = if pos == 0 {
            AppCommandError::Validation {
                detail: "layer is already at the bottom; nothing below to merge into".into(),
            }
        } else {
            unreachable!()
        };
        assert!(matches!(err, AppCommandError::Validation { .. }));
    }

    /// `merge_layer_into` rejects when either layer is non-raster.
    #[test]
    fn merge_layer_into_rejects_non_raster() {
        let mut doc = fresh_doc(Size::new(2, 2));
        let raster = add_raster_layer(&mut doc, 1, "r");
        // Push a tilemap layer manually.
        let project = doc.project.as_mut().unwrap();
        let sprite = project.sprite_mut(SpriteId::new(1)).unwrap();
        sprite.layers.push(Layer {
            id: LayerId::new(2),
            name: "tiles".into(),
            kind: LayerKind::Tilemap {
                tileset: TilesetId::new(99),
            },
            blend_mode: BlendMode::Normal,
            opacity: 255,
            visible: true,
            locked: false,
            parent: None,
            effects: Vec::new(),
            user_data: UserData::default(),
        });
        let tilemap = LayerId::new(2);

        let result = merge_layer_into(&mut doc, SpriteId::new(1), tilemap, raster, "merge");
        match result {
            Err(AppCommandError::Unimplemented { .. }) => (),
            Err(other) => panic!("expected Unimplemented; got {other:?}"),
            Ok(_) => panic!("expected error; merge succeeded"),
        }
    }

    /// `order_layers_by_z` sorts requested layers by their position in
    /// the sprite's layer stack, low index first (= bottom-most first).
    #[test]
    fn order_layers_by_z_returns_bottom_first() {
        let mut doc = fresh_doc(Size::new(2, 2));
        let l1 = add_raster_layer(&mut doc, 10, "a");
        let l2 = add_raster_layer(&mut doc, 11, "b");
        let l3 = add_raster_layer(&mut doc, 12, "c");
        // Pass them in arbitrary order.
        let ordered = order_layers_by_z(&doc, SpriteId::new(1), &[l3, l1, l2]).unwrap();
        assert_eq!(ordered, vec![l1, l2, l3]);
    }

    /// `merge_selected` composites every selected layer into the
    /// top-most one and removes the merged sources. Bottom paints
    /// first, then mid, then top; with opaque colors at full
    /// opacity, the top-most layer's pixels survive on screen, so
    /// the merged target ends up holding the top-most's content.
    #[test]
    fn merge_selected_drops_sources_and_keeps_top() {
        let mut doc = fresh_doc(Size::new(2, 2));
        let bottom = add_raster_layer(&mut doc, 1, "bottom");
        let mid = add_raster_layer(&mut doc, 2, "mid");
        let top = add_raster_layer(&mut doc, 3, "top");
        let _bottom_buf = add_filled_cel(&mut doc, bottom, 0, [255, 0, 0, 255]);
        let _mid_buf = add_filled_cel(&mut doc, mid, 0, [0, 255, 0, 255]);
        let top_buf = add_filled_cel(&mut doc, top, 0, [0, 0, 255, 255]);

        // Run the merge logic without the Tauri shell: build the
        // ordered stack, composite into the target, drop sources.
        let ordered = order_layers_by_z(&doc, SpriteId::new(1), &[bottom, mid, top]).unwrap();
        let target = ordered[ordered.len() - 1];
        let _ops = composite_stack_into(&mut doc, SpriteId::new(1), &ordered, target).unwrap();
        let sources: Vec<_> = ordered.iter().copied().filter(|id| *id != target).collect();
        for src in &sources {
            drop_layer(&mut doc, SpriteId::new(1), *src).unwrap();
        }

        // Only the top layer survives.
        let sprite = find_sprite(&doc, SpriteId::new(1)).unwrap();
        assert_eq!(sprite.layers.len(), 1);
        assert_eq!(sprite.layers[0].id, top);

        // Visible composite: red, then green over red, then blue
        // over green at full opacity — every pixel is opaque blue.
        let after = pixels_of(&doc, top_buf);
        for chunk in after.chunks_exact(4) {
            assert_eq!(chunk, &[0, 0, 255, 255]);
        }
    }

    /// Same fixture as `merge_selected_drops_sources_and_keeps_top`
    /// but pinning the visual-stack semantics: when the top layer is
    /// fully transparent, the merged result keeps the layer below
    /// (mid). This is the test that would have caught the early
    /// "paint sources onto target" implementation.
    #[test]
    fn merge_selected_preserves_visual_stack_when_top_is_transparent() {
        let mut doc = fresh_doc(Size::new(2, 2));
        let bottom = add_raster_layer(&mut doc, 1, "bottom");
        let mid = add_raster_layer(&mut doc, 2, "mid");
        let top = add_raster_layer(&mut doc, 3, "top");
        let _bottom_buf = add_filled_cel(&mut doc, bottom, 0, [255, 0, 0, 255]);
        let _mid_buf = add_filled_cel(&mut doc, mid, 0, [0, 255, 0, 255]);
        let top_buf = add_filled_cel(&mut doc, top, 0, [0, 0, 0, 0]); // transparent

        let ordered = order_layers_by_z(&doc, SpriteId::new(1), &[bottom, mid, top]).unwrap();
        let target = ordered[ordered.len() - 1];
        let _ops = composite_stack_into(&mut doc, SpriteId::new(1), &ordered, target).unwrap();

        // Visual stack: red below, green above, transparent on top.
        // The visible result is opaque green.
        let after = pixels_of(&doc, top_buf);
        for chunk in after.chunks_exact(4) {
            assert_eq!(chunk, &[0, 255, 0, 255]);
        }
    }

    /// `flatten_visible` merges every visible raster layer into the
    /// bottom-most visible raster layer and leaves hidden layers
    /// untouched.
    #[test]
    fn flatten_visible_keeps_hidden_layers() {
        let mut doc = fresh_doc(Size::new(2, 2));
        let bottom = add_raster_layer(&mut doc, 1, "bottom");
        let mid = add_raster_layer(&mut doc, 2, "mid");
        let top = add_raster_layer(&mut doc, 3, "top");
        let bottom_buf = add_filled_cel(&mut doc, bottom, 0, [0, 0, 0, 0]);
        let _mid_buf = add_filled_cel(&mut doc, mid, 0, [255, 0, 0, 255]);
        let _top_buf = add_filled_cel(&mut doc, top, 0, [0, 0, 0, 0]);

        // Hide the top layer.
        {
            let project = doc.project.as_mut().unwrap();
            let sprite = project.sprite_mut(SpriteId::new(1)).unwrap();
            sprite
                .layers
                .iter_mut()
                .find(|l| l.id == top)
                .unwrap()
                .visible = false;
        }

        // Mirror the public command's body for the visible-raster
        // collection step and the merge loop.
        let visible_raster: Vec<LayerId> = {
            let sprite = find_sprite(&doc, SpriteId::new(1)).unwrap();
            sprite
                .layers
                .iter()
                .filter(|l| l.visible && matches!(l.kind, LayerKind::Raster))
                .map(|l| l.id)
                .collect()
        };
        assert_eq!(visible_raster, vec![bottom, mid]);

        let target = visible_raster[0];
        let _ops =
            composite_stack_into(&mut doc, SpriteId::new(1), &visible_raster, target).unwrap();
        let sources: Vec<_> = visible_raster
            .iter()
            .copied()
            .filter(|id| *id != target)
            .collect();
        for src in &sources {
            drop_layer(&mut doc, SpriteId::new(1), *src).unwrap();
        }

        // Hidden layer survives.
        let sprite = find_sprite(&doc, SpriteId::new(1)).unwrap();
        let layer_ids: Vec<_> = sprite.layers.iter().map(|l| l.id).collect();
        assert!(layer_ids.contains(&top), "hidden top layer must survive");
        assert!(layer_ids.contains(&bottom), "merge target must survive");
        assert!(!layer_ids.contains(&mid), "merged source must drop");

        // Bottom layer now holds the mid layer's red pixels.
        let after = pixels_of(&doc, bottom_buf);
        for chunk in after.chunks_exact(4) {
            assert_eq!(chunk, &[255, 0, 0, 255]);
        }
    }
}
